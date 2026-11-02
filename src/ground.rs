//! The map drawn with TAB's own landscape pictures – like TAB's map drawer (`ZX.GameSystems\ZXMapDrawer.cs`). Only
//! drawing: the simulation never sees any of it.
//!
//! **The ground** works like TAB's: one earth texture over the whole map, the grass texture over it where there's
//! grass, the water where there's water. Which tile is what, a **mask** says – a small picture, one pixel per tile (red:
//! grass, green: water), smoothed between the tiles (TAB: `CCustomMask` with linear filtering). A little program on the
//! graphics card (a *shader*) mixes the three textures by the mask, pixel by pixel – for the whole visible area at once,
//! two triangles.
//!
//! **On the ground** (`Decor`): a rock piece on every 2 × 2 block of mountain – bigger deep in a range; one tree per
//! forest tile – other trees on grass than on bare earth, bigger deep in a forest; herbs on the grass; four lumps per
//! ore tile (coal too – TAB has none, ours is made of its stone). Where they stand and which picture, decided like TAB (`CreateEntities`), with a "random" number from the
//! tile – the same every time the map is drawn.
//!
//! The pictures come from `tools\tab_terrain.py` (`assets\terrain\<look>\`, only on this PC). Not there: the map is
//! drawn as before (`draw.rs`, coloured diamonds).

use std::collections::{HashMap, VecDeque};

use macroquad::miniquad::{MipmapFilterMode, TextureWrap};
use macroquad::prelude::*;
use serde::Deserialize;

use crate::data;
use crate::iso::{Camera, TILE_W};
use crate::map::{Map, Ore, Region, Terrain};

/// The landscape a map without one is drawn as.
const DEFAULT_LOOK: &str = "BR";
/// The world is cut into squares of this many tiles – drawing only looks at the squares on screen.
const CHUNK: i32 = 16;

/// `assets\terrain\<look>\terrain.json`: what `tools\tab_terrain.py` wrote.
#[derive(Deserialize)]
struct Sheet {
    earth: Texture,
    grass: Texture,
    water: Texture,
    /// The atlas: all pictures standing on the ground, in one picture.
    objects: String,
    pictures: Vec<Picture>,
    /// The mountain pieces per height (1–5) – each a list of variants. One height only: it's scaled instead
    /// (`scale_pieces`).
    pieces: Vec<Vec<usize>>,
    scale_pieces: bool,
    /// The trees on bare earth and on grass: `[picture, weight]`.
    trees_dirt: Vec<(usize, f32)>,
    trees_grass: Vec<(usize, f32)>,
    /// Deep in a forest a tree is drawn up to this many times as big (TAB's `MaxScaleFactorForTrees`).
    max_tree: f32,
    herbs: Vec<usize>,
    stone: Vec<usize>,
    iron: Vec<usize>,
    gold: Vec<usize>,
    /// TAB's oil source (`OilSource`): one picture for a whole 2 × 2 node. Older files without it: none.
    #[serde(default)]
    oil: Vec<usize>,
    /// Coal – TAB has none, ours: its stone lumps blackened and broken (`tools	ab_terrain.py`). Older files without
    /// it: none.
    #[serde(default)]
    coal: Vec<usize>,
}

/// A ground texture: its picture and how many tiles it covers before it repeats.
#[derive(Deserialize)]
struct Texture {
    image: String,
    tiles: f32,
}

/// One picture in the atlas.
#[derive(Deserialize)]
struct Picture {
    /// x, y, width, height in the atlas (pixels).
    at: [f32; 4],
    /// Where it stands on the ground (pixels from its top left corner).
    feet: [f32; 2],
    /// This many pixels are one tile width at scale 1.
    pixels_per_tile: f32,
}

/// A landscape, loaded.
struct Look {
    sheet: Sheet,
    earth: Texture2D,
    grass: Texture2D,
    water: Texture2D,
    objects: Texture2D,
}

/// Something standing on the ground: picture `picture` with its feet at `at`, `scale` times its size.
#[derive(Clone, Copy)]
pub struct Decor {
    picture: u16,
    pub at: Vec2,
    scale: f32,
    /// Tall (trees, rocks): sorted in among the figures. Otherwise flat on the ground (herbs, ore) – drawn with it.
    pub tall: bool,
}

/// What lies under every tile – TAB's terrain layer: forests and mountains are things *on* the ground there.
#[derive(Clone, Copy, PartialEq)]
enum Ground {
    Dirt,
    Grass,
    Water,
}

/// TAB's landscapes, drawn. Loaded when first needed; `refresh` before drawing.
#[derive(Default)]
pub struct Landscape {
    /// Per look: loaded – or `None`: no such pictures (or broken), not tried again every frame.
    looks: HashMap<String, Option<Look>>,
    /// The shader that mixes the ground. `None` until made (it needs the window), or if it couldn't be.
    material: Option<Material>,
    /// The mask: one pixel per tile.
    mask: Option<Texture2D>,
    /// What the mask and the things on the ground were made for: the map's revision, its size and the look.
    made_for: Option<(u32, usize, String)>,
    /// The things on the ground, per square of `CHUNK` tiles, row after row.
    chunks: Vec<Vec<Decor>>,
    chunks_wide: i32,
    /// What went wrong with the last look that didn't load.
    problem: Option<String>,
}

impl Landscape {
    /// Makes the mask and the things on the ground anew if the map or its look changed (a new round, the editor's
    /// brush). `false`: there are no pictures for this look – draw the map the old way.
    pub fn refresh(&mut self, map: &Map, look: &str) -> bool {
        let look = if look.is_empty() { DEFAULT_LOOK } else { look };
        if !self.looks.contains_key(look) {
            let loaded = match Look::load(look) {
                Ok(l) => l,
                Err(e) => {
                    self.problem = Some(e);
                    None
                }
            };
            self.looks.insert(look.to_string(), loaded);
        }
        if self.looks[look].is_none() {
            return false;
        }
        if self.material.is_none() {
            match make_material() {
                Ok(m) => self.material = Some(m),
                Err(e) => {
                    self.problem = Some(e);
                    self.looks.insert(look.to_string(), None);
                    return false;
                }
            }
        }
        let wanted = (map.revision, map.size, look.to_string());
        if self.made_for.as_ref() != Some(&wanted) {
            let ground = ground_of(map);
            self.paint_mask(map, &ground);
            self.place(map, &ground, look);
            self.made_for = Some(wanted);
        }
        true
    }

    /// Does this look draw oil itself (TAB's oil source)? Otherwise `draw.rs` keeps its dark pool.
    pub fn draws_oil(&self) -> bool {
        self.look().is_some_and(|l| !l.sheet.oil.is_empty())
    }

    /// Does this look draw coal itself (our lumps)? Otherwise `draw.rs` keeps its dark diamond.
    pub fn draws_coal(&self) -> bool {
        self.look().is_some_and(|l| !l.sheet.coal.is_empty())
    }

    /// What was wrong with the last look that didn't load.
    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    fn look(&self) -> Option<&Look> {
        let (_, _, look) = self.made_for.as_ref()?;
        self.looks.get(look)?.as_ref()
    }

    /// The mask: red where there's grass, green where there's water.
    fn paint_mask(&mut self, map: &Map, ground: &[Ground]) {
        let n = map.size;
        let mut bytes = vec![0u8; n * n * 4];
        for (i, g) in ground.iter().enumerate() {
            bytes[i * 4] = if *g == Ground::Grass { 255 } else { 0 };
            bytes[i * 4 + 1] = if *g == Ground::Water { 255 } else { 0 };
            bytes[i * 4 + 3] = 255;
        }
        let texture = Texture2D::from_rgba8(n as u16, n as u16, &bytes);
        // Smoothed between the tiles – the soft edge of the grass.
        texture.set_filter(FilterMode::Linear);
        self.mask = Some(texture);
    }

    /// Decides what stands where, like TAB's `CreateEntities`.
    fn place(&mut self, map: &Map, ground: &[Ground], look: &str) {
        let n = map.size as i32;
        self.chunks_wide = (n + CHUNK - 1) / CHUNK;
        self.chunks = vec![Vec::new(); (self.chunks_wide * self.chunks_wide) as usize];
        let Some(sheet) = self.looks.get(look).and_then(|l| l.as_ref()).map(|l| &l.sheet) else {
            return;
        };
        let mut all = Vec::new();
        mountains(map, sheet, &mut all);
        trees(map, ground, sheet, &mut all);
        herbs(map, ground, sheet, &mut all);
        ores(map, sheet, &mut all);
        for decor in all {
            let (cx, cy) = ((decor.at.x as i32 / CHUNK).clamp(0, self.chunks_wide - 1), (decor.at.y as i32 / CHUNK).clamp(0, self.chunks_wide - 1));
            self.chunks[(cy * self.chunks_wide + cx) as usize].push(decor);
        }
        // Within a square from the back to the front – the flat ones are drawn in this order.
        for chunk in &mut self.chunks {
            chunk.sort_by(|a, b| (a.at.x + a.at.y).total_cmp(&(b.at.x + b.at.y)));
        }
    }

    /// The ground of `area`: the shader mixes the textures by the mask.
    pub fn draw_ground(&self, camera: &Camera, area: Region) {
        let (Some(look), Some(material), Some(mask)) = (self.look(), &self.material, &self.mask) else {
            return;
        };
        let size = mask.width();
        material.set_texture("Earth", look.earth.clone());
        material.set_texture("Grass", look.grass.clone());
        material.set_texture("Water", look.water.clone());
        material.set_texture("Mask", mask.clone());
        material.set_uniform("Tiles", [look.sheet.earth.tiles, look.sheet.grass.tiles, look.sheet.water.tiles, size]);
        // The four corners of the area: on screen where the camera puts them, and as "texture position" the world point
        // itself – the shader works out from it which tile it's on and where in each texture.
        let vertices: Vec<Vertex> = area
            .corners()
            .iter()
            .map(|&w| {
                let s = camera.world_to_screen(w);
                Vertex::new(s.x, s.y, 0.0, w.x, w.y, WHITE)
            })
            .collect();
        let mesh = Mesh { vertices, indices: vec![0, 1, 2, 0, 2, 3], texture: None };
        gl_use_material(material);
        draw_mesh(&mesh);
        gl_use_default_material();
    }

    /// The things on the ground in `area` – `tall` or flat ones. `each` gets every one that is in it.
    pub fn decor_in(&self, area: Region, tall: bool, mut each: impl FnMut(&Decor)) {
        if self.chunks.is_empty() {
            return;
        }
        // The squares the area touches (a bit more: a big tree stands out of its square).
        let (mut lo, mut hi) = (IVec2::splat(i32::MAX), IVec2::splat(i32::MIN));
        for c in area.corners() {
            let t = IVec2::new(c.x.floor() as i32, c.y.floor() as i32);
            lo = lo.min(t);
            hi = hi.max(t);
        }
        let w = self.chunks_wide;
        let (x0, y0) = (((lo.x - 3) / CHUNK).max(0), ((lo.y - 3) / CHUNK).max(0));
        let (x1, y1) = (((hi.x + 3) / CHUNK).min(w - 1), ((hi.y + 3) / CHUNK).min(w - 1));
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                for decor in self.chunks[(cy * w + cx) as usize].iter().filter(|d| d.tall == tall && area.contains(d.at)) {
                    each(decor);
                }
            }
        }
    }

    /// Draws one of them – `color` multiplied (white: as painted; darker in the fog).
    pub fn draw_decor(&self, camera: &Camera, decor: &Decor, color: Color) {
        let Some(look) = self.look() else {
            return;
        };
        let Some(p) = look.sheet.pictures.get(decor.picture as usize) else {
            return;
        };
        let scale = TILE_W / p.pixels_per_tile * camera.zoom * decor.scale;
        let size = vec2(p.at[2], p.at[3]) * scale;
        let corner = camera.world_to_screen(decor.at) - Vec2::from(p.feet) * scale;
        if corner.x > screen_width() || corner.y > screen_height() || corner.x + size.x < 0.0 || corner.y + size.y < 0.0 {
            return;
        }
        let source = Rect::new(p.at[0], p.at[1], p.at[2], p.at[3]);
        let params = DrawTextureParams { dest_size: Some(size), source: Some(source), ..Default::default() };
        draw_texture_ex(&look.objects, corner.x, corner.y, color, params);
    }
}

impl Look {
    /// Reads `assets\terrain\<name>\`. `Ok(None)`: there's no such folder – not converted on this PC.
    fn load(name: &str) -> Result<Option<Look>, String> {
        let Some(folder) = data::folder("assets").map(|f| f.join("terrain").join(name)) else {
            return Ok(None);
        };
        let json = folder.join("terrain.json");
        if !json.is_file() {
            return Ok(None);
        }
        let at = |e: &dyn std::fmt::Display| format!("{}: {e}", json.display());
        let text = std::fs::read_to_string(&json).map_err(|e| at(&e))?;
        let sheet: Sheet = serde_json::from_str(&text).map_err(|e| at(&e))?;
        let picture = |file: &str| -> Result<Texture2D, String> {
            let path = folder.join(file);
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let image = Image::from_file_with_format(&bytes, None).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(Texture2D::from_image(&image))
        };
        let (earth, grass, water) = (picture(&sheet.earth.image)?, picture(&sheet.grass.image)?, picture(&sheet.water.image)?);
        for texture in [&earth, &grass, &water] {
            smooth(texture, true);
        }
        let objects = picture(&sheet.objects)?;
        smooth(&objects, false);
        Ok(Some(Look { sheet, earth, grass, water, objects }))
    }
}

/// Smooth with mipmaps (like the sprites, `sprites.rs`) – and `repeat`: past its edge the texture starts over.
fn smooth(texture: &Texture2D, repeat: bool) {
    texture.set_filter(FilterMode::Linear);
    let id = texture.raw_miniquad_id();
    // `unsafe`: straight to miniquad, as in `sprites.rs` – it only touches this new texture.
    let gl = unsafe { get_internal_gl() };
    gl.quad_context.texture_generate_mipmaps(id);
    gl.quad_context.texture_set_min_filter(id, FilterMode::Linear, MipmapFilterMode::Linear);
    if repeat {
        gl.quad_context.texture_set_wrap(id, TextureWrap::Repeat, TextureWrap::Repeat);
    }
}

/// The program on the graphics card that mixes the ground. Written in GLSL (OpenGL's shader language): the first part
/// runs once per corner (it only places it), the second once per pixel on screen.
fn make_material() -> Result<Material, String> {
    let vertex = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
varying highp vec2 world;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    world = texcoord;
}"#;
    // Per pixel: its world point (`world`, in tiles) → where in each texture (it repeats every `Tiles` tiles) and in
    // the mask (one pixel per tile, the map `Tiles.w` tiles big). Earth, grass over it as much as the mask is red,
    // water as much as it's green – that edge a bit sharper (TAB's water has a hard edge).
    let fragment = r#"#version 100
precision highp float;
varying highp vec2 world;
uniform sampler2D Earth;
uniform sampler2D Grass;
uniform sampler2D Water;
uniform sampler2D Mask;
uniform vec4 Tiles;
void main() {
    vec3 earth = texture2D(Earth, world / Tiles.x).rgb;
    vec3 grass = texture2D(Grass, world / Tiles.y).rgb;
    vec3 water = texture2D(Water, world / Tiles.z).rgb;
    vec4 mask = texture2D(Mask, world / Tiles.w);
    vec3 color = mix(earth, grass, mask.r);
    color = mix(color, water, smoothstep(0.35, 0.65, mask.g));
    gl_FragColor = vec4(color, 1.0);
}"#;
    let params = MaterialParams {
        uniforms: vec![UniformDesc::new("Tiles", UniformType::Float4)],
        textures: vec!["Earth".into(), "Grass".into(), "Water".into(), "Mask".into()],
        ..Default::default()
    };
    load_material(ShaderSource::Glsl { vertex, fragment }, params).map_err(|e| format!("ground shader: {e}"))
}

/// The ground under every tile: water and grass as they are, everything else bare earth – and under forest and
/// mountains the ground of the nearest open tile (our map has no layer under them, TAB's has).
fn ground_of(map: &Map) -> Vec<Ground> {
    let n = map.size as i32;
    let mut ground = vec![Ground::Dirt; (n * n) as usize];
    let mut known = vec![false; (n * n) as usize];
    let mut queue = VecDeque::new();
    for y in 0..n {
        for x in 0..n {
            let i = (y * n + x) as usize;
            let g = match map.terrain(x, y) {
                Terrain::Grass => Some(Ground::Grass),
                Terrain::Dirt => Some(Ground::Dirt),
                Terrain::Water => Some(Ground::Water),
                Terrain::Forest | Terrain::Mountain => None,
            };
            if let Some(g) = g {
                ground[i] = g;
                known[i] = g != Ground::Water;
                if known[i] {
                    queue.push_back((x, y));
                }
            }
        }
    }
    // Outwards from every open tile, one ring after the other (a "breadth-first search", like a flood fill).
    while let Some((x, y)) = queue.pop_front() {
        let g = ground[(y * n + x) as usize];
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= n || ny >= n {
                continue;
            }
            let j = (ny * n + nx) as usize;
            if !known[j] && map.terrain(nx, ny).is_solid() && map.terrain(nx, ny) != Terrain::Water {
                known[j] = true;
                ground[j] = g;
                queue.push_back((nx, ny));
            }
        }
    }
    ground
}

/// A "random" number from a tile and a salt (0..1) – always the same for the same tile. Only for the picture.
fn roll(x: i32, y: i32, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B9) ^ (y as u32).wrapping_mul(0x85EB_CA6B) ^ salt.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Which of `list` (`[picture, weight]`), by the roll `r` (0..1).
fn by_weight(list: &[(usize, f32)], r: f32) -> Option<usize> {
    let total: f32 = list.iter().map(|(_, w)| w).sum();
    let mut left = r * total;
    for &(picture, weight) in list {
        if left < weight {
            return Some(picture);
        }
        left -= weight;
    }
    list.last().map(|&(p, _)| p)
}

/// Is every tile of the rectangle (`x`, `y`, `w` × `h`) such a tile? (TAB's `IsAllAreaWithValue`.) Off the map counts as
/// mountain, like `Map::terrain` says.
fn all(map: &Map, x: i32, y: i32, w: i32, h: i32, terrain: Terrain) -> bool {
    (y..y + h).all(|ty| (x..x + w).all(|tx| map.terrain(tx, ty) == terrain))
}

/// A rock piece on every 2 × 2 block of mountain whose corner tile (even x and y) is mountain – TAB's. Deep inside a
/// range bigger (BR, TM, AL, DS: the same piece up to 2.5 times; FA, VO: a taller piece). Single mountain tiles off
/// that grid (painted in the editor) get a small piece of their own.
fn mountains(map: &Map, sheet: &Sheet, out: &mut Vec<Decor>) {
    let n = map.size as i32;
    let mut covered = vec![false; (n * n) as usize];
    // A piece on the square of `size` tiles at `x`, `y`, `scale` times as big. The pictures are made for 2 × 2.
    let put = |pics: &[usize], x: i32, y: i32, size: f32, scale: f32, out: &mut Vec<Decor>| {
        if pics.is_empty() {
            return;
        }
        // TAB picks the variant by the block (`GridTwoCells`).
        let variant = ((x / 2 + y / 2 * (pics.len() as i32 / 2).max(1)) as usize) % pics.len();
        // Its feet: the bottom corner of its square – scaled from the middle.
        let middle = vec2(x as f32, y as f32) + size / 2.0;
        let at = middle + Vec2::splat(size * scale / 2.0);
        out.push(Decor { picture: pics[variant] as u16, at, scale: scale * size / 2.0, tall: true });
    };
    // The pieces of a height (1–5); a landscape with only one: that one.
    let height = |level: usize| sheet.pieces.get(level - 1).or(sheet.pieces.first()).map(|p| p.as_slice()).unwrap_or(&[]);
    let full = map.full_area();
    for y in (0..n - 1).step_by(2) {
        for x in (0..n - 1).step_by(2) {
            if map.terrain(x, y) != Terrain::Mountain || !full.contains_tile(x, y) {
                continue;
            }
            for (tx, ty) in [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)] {
                covered[(ty * n + tx) as usize] = true;
            }
            let m = |d: i32, w: i32| all(map, x - d, y - d, w, w, Terrain::Mountain);
            if sheet.scale_pieces {
                let scale = if (x + y) % 16 == 0 && m(5, 12) {
                    2.5
                } else if (x + y) % 8 == 0 && m(4, 10) {
                    2.25
                } else if (x + y) % 4 == 0 && m(3, 8) {
                    2.0
                } else if m(2, 6) {
                    1.5
                } else {
                    1.0
                };
                put(height(1), x, y, 2.0, scale, out);
            } else {
                // TAB: the tallest only every 12 tiles or so – here every block deep enough whose numbers fit.
                let level = if (x + y) % 12 == 0 && m(8, 18) {
                    5
                } else if (x + y) % 6 == 0 && m(6, 14) {
                    4
                } else if m(4, 10) {
                    3
                } else if m(2, 6) {
                    2
                } else {
                    1
                };
                put(height(level), x, y, 2.0, 1.0, out);
            }
        }
    }
    for y in 0..n {
        for x in 0..n {
            if map.terrain(x, y) == Terrain::Mountain && !covered[(y * n + x) as usize] && full.contains_tile(x, y) {
                put(height(1), x, y, 1.0, 1.0, out);
            }
        }
    }
}

/// One tree per forest tile (TAB: `TemplateDicForTreesOnGround` / `…OnGrass`). Deep in a forest it's bigger and stands
/// a bit off the middle of its tile; at the edge right in it, as painted.
fn trees(map: &Map, ground: &[Ground], sheet: &Sheet, out: &mut Vec<Decor>) {
    let n = map.size as i32;
    let max = sheet.max_tree;
    for y in 0..n {
        for x in 0..n {
            if map.terrain(x, y) != Terrain::Forest {
                continue;
            }
            let list = if ground[(y * n + x) as usize] == Ground::Grass { &sheet.trees_grass } else { &sheet.trees_dirt };
            let Some(picture) = by_weight(list, roll(x, y, 1)) else {
                continue;
            };
            // TAB's rectangles, as they are (a bit lopsided: 5 wide from 3 to the left).
            let f = |d: i32, w: i32| all(map, x - d, y - d, w, w, Terrain::Forest);
            let r = roll(x, y, 2);
            let scale = if f(3, 5) {
                Some((1.7 + r * (max - 1.7)).min(max))
            } else if f(2, 4) {
                Some(1.3 + r * ((1.0 + (max - 1.0) / 2.0) - 1.3))
            } else if f(1, 3) {
                Some(1.1 + r * ((1.0 + (max - 1.0) / 4.0) - 1.1))
            } else {
                None
            };
            let middle = vec2(x as f32 + 0.5, y as f32 + 0.5);
            let (middle, scale) = match scale {
                // TAB's `GetRandom(0.3)`: up to 0.3 tiles off, either way.
                Some(s) => (middle + vec2(roll(x, y, 3) - 0.5, roll(x, y, 4) - 0.5) * 0.6, s.min(max)),
                None => (middle, 1.0),
            };
            // Its feet: the bottom corner of its tile-sized square, scaled from the middle.
            out.push(Decor { picture: picture as u16, at: middle + Vec2::splat(scale / 2.0), scale, tall: true });
        }
    }
}

/// Herbs on the grass – and on bare earth right next to it (TAB, its "medium" grass quality: 0 to 4 per tile). Not
/// next to mountains or trees.
fn herbs(map: &Map, ground: &[Ground], sheet: &Sheet, out: &mut Vec<Decor>) {
    if sheet.herbs.is_empty() {
        return;
    }
    let n = map.size as i32;
    let open = |x: i32, y: i32| !matches!(map.terrain(x, y), Terrain::Forest | Terrain::Mountain);
    let grass = |x: i32, y: i32| map.terrain(x, y) == Terrain::Grass;
    for y in 2..n - 3 {
        for x in 2..n - 3 {
            let here = ground[(y * n + x) as usize];
            let fits = match map.terrain(x, y) {
                Terrain::Grass => true,
                Terrain::Dirt => grass(x + 1, y) || grass(x - 1, y) || grass(x, y + 1) || grass(x, y - 1),
                _ => false,
            };
            if !fits || here == Ground::Water || map.deposit(x, y).is_some() {
                continue;
            }
            if !(open(x + 1, y) && open(x + 1, y + 1) && open(x + 1, y - 1) && open(x, y + 1) && open(x, y + 2) && open(x + 2, y) && open(x + 2, y + 1)) {
                continue;
            }
            let count = ((roll(x, y, 5) * 7.0) as i32 - 2).max(0);
            for k in 0..count {
                let salt = 10 + k as u32 * 4;
                let picture = sheet.herbs[(roll(x, y, salt) * sheet.herbs.len() as f32) as usize % sheet.herbs.len()];
                let middle = vec2(x as f32 + 0.5, y as f32 + 0.5) + vec2(roll(x, y, salt + 1) - 0.5, roll(x, y, salt + 2) - 0.5) * 0.6;
                // 0.35 tiles big, give or take 0.1 – the picture is made for 0.3.
                let size = 0.35 + (roll(x, y, salt + 3) - 0.5) * 0.2;
                out.push(Decor { picture: picture as u16, at: middle + Vec2::splat(size / 2.0), scale: size / 0.3, tall: false });
            }
        }
    }
}

/// Four lumps on every tile of stone, iron, gold and coal (TAB's `MineralStone` …; coal is ours): in its quarters, a
/// bit off, 0.6 tiles big give or take 0.15. On every oil node (2 × 2, like TAB's) its oil source.
fn ores(map: &Map, sheet: &Sheet, out: &mut Vec<Decor>) {
    let n = map.size as i32;
    for y in 0..n {
        for x in 0..n {
            let Some(deposit) = map.deposit(x, y) else {
                continue;
            };
            let pics = match deposit.ore {
                Ore::Stone => &sheet.stone,
                Ore::Iron => &sheet.iron,
                Ore::Gold => &sheet.gold,
                Ore::Coal => &sheet.coal,
                // Once per node: at its top left tile (nodes never touch – no oil above or left of it). Its feet: the
                // bottom corner of the 2 × 2.
                Ore::Oil => {
                    let is_oil = |x: i32, y: i32| map.deposit(x, y).is_some_and(|d| d.ore == Ore::Oil);
                    if let Some(&picture) = sheet.oil.first().filter(|_| !is_oil(x - 1, y) && !is_oil(x, y - 1)) {
                        out.push(Decor { picture: picture as u16, at: vec2(x as f32 + 2.0, y as f32 + 2.0), scale: 1.0, tall: false });
                    }
                    continue;
                }
            };
            if pics.is_empty() {
                continue;
            }
            for k in 0..4 {
                let salt = 40 + k as u32 * 4;
                let quarter = vec2(0.25 + (k % 2) as f32 * 0.5, 0.25 + (k / 2) as f32 * 0.5);
                let off = vec2(roll(x, y, salt) - 0.5, roll(x, y, salt + 1) - 0.5) * 0.44;
                let middle = vec2(x as f32, y as f32) + quarter + off;
                let size = 0.6 + (roll(x, y, salt + 2) - 0.5) * 0.3;
                let picture = pics[(roll(x, y, salt + 3) * pics.len() as f32) as usize % pics.len()];
                out.push(Decor { picture: picture as u16, at: middle + Vec2::splat(size / 2.0), scale: size, tall: false });
            }
        }
    }
}
