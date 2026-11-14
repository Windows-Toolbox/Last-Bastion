//! The map drawn with TAB's own landscape pictures – like TAB's map drawer (`ZX.GameSystems\ZXMapDrawer.cs`). Only
//! drawing: the simulation never sees any of it.
//!
//! **The ground** works like TAB's: one earth texture over the whole map, the grass texture over it where there's
//! grass, the water where there's water. Which tile is what, a **mask** says – a small picture, one pixel per tile (red:
//! grass, green: water), smoothed between the tiles (TAB: `CCustomMask` with linear filtering). A little program on the
//! graphics card (a *shader*) mixes the three textures by the mask, pixel by pixel – for the whole visible area at once,
//! two triangles.
//!
//! **On the ground** (`Decor`): a rock piece on every 2 × 2 block of mountain – bigger deep in a range, one big
//! picture where the editor put a big rock (`map::BigRock`); one tree per
//! forest tile – other trees on grass than on bare earth, bigger deep in a forest; herbs on the grass; four lumps per
//! ore tile (coal too – TAB has none, ours is made of its stone). Where they stand and which picture, decided like TAB (`CreateEntities`), with a "random" number from the
//! tile – the same every time the map is drawn.
//!
//! **The cliffs round the water** (`cliffs`): TAB's rock faces on every shore tile, hanging down into the water – which
//! piece by the water around the tile (`ZXMapDrawer.CreateCliffsFor`), drawn over the water's shimmer.
//!
//! **The floor under the buildings** (`Floor`): TAB paves the ground around every building but the farm
//! (`ZX.Components\CFloorUndeBuilding.cs`) – its stone pavement `Floor A`, a soft mask over it so the edge frays, one
//! tile further than the building, but never onto forest, rock, water or ore. Herbs under it don't grow.
//!
//! The pictures come from `tools\tab_terrain.py` (`assets\terrain\<look>\`, only on this PC). Not there: the map is
//! drawn as before (`draw.rs`, coloured diamonds).

use std::collections::{HashMap, VecDeque};

use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation, MipmapFilterMode, TextureWrap};
use macroquad::prelude::*;
use serde::Deserialize;

use crate::data;
use crate::iso::{Camera, TILE_W};
use crate::map::{Map, Ore, Region, Terrain, BIG_ROCK_SIZES};

/// The floor under a building reaches this many tiles past it on every side – TAB's 1 (2 from 3 tiles on was too far).
const FLOOR_REACH: i32 = 1;
/// The landscape a map without one is drawn as.
const DEFAULT_LOOK: &str = "BR";
/// The world is cut into squares of this many tiles – drawing only looks at the squares on screen.
const CHUNK: i32 = 16;
/// Deep in a mountain range a rock piece is drawn bigger (BR, TM, AL, DS) – the first row that fits: `(every, square,
/// scale)`: on every `every`-th block (counted along x + y), if the `square` × `square` tiles around the block are all
/// mountain. TAB's squares are one step bigger – 12, 10, 8, 6 (DESIGN.md 9): there only wide ranges got big rocks.
const SCALES: [(i32, i32, f32); 4] = [(16, 10, 2.5), (8, 8, 2.25), (4, 6, 2.0), (1, 4, 1.5)];
/// FA, VO: a taller piece instead – `(every, square, height)`. TAB's squares: 18, 14, 10, 6.
const HEIGHTS: [(i32, i32, usize); 4] = [(12, 14, 5), (6, 10, 4), (1, 6, 3), (1, 4, 2)];

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
    /// TAB's big rocks per size (3, 4, 5 tiles) – each a list of variants (`map::BigRock`). Older files without them:
    /// none, their tiles get small pieces.
    #[serde(default)]
    big_rocks: Vec<Vec<usize>>,
    /// Coal – TAB has none, ours: its stone lumps blackened and broken (`tools\tab_terrain.py`). Older files without
    /// it: none.
    #[serde(default)]
    coal: Vec<usize>,
    /// The pavement under the buildings, and its masks side by side (small, medium, large – `Floor`). Older files
    /// without them: no floor.
    #[serde(default)]
    floor: Option<Texture>,
    #[serde(default)]
    floor_masks: Option<String>,
    /// TAB's cliffs round the water (`cliffs`): per kind of piece (`"B"`, `"TR"`, `"IBL"` …) its pictures. Older files
    /// without them: no cliffs.
    #[serde(default)]
    cliffs: HashMap<String, Vec<usize>>,
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
    /// The pavement and its masks (`Sheet::floor`).
    floor: Option<(Texture2D, Texture2D)>,
}

/// Something standing on the ground: picture `picture` with its feet at `at`, `scale` times its size.
#[derive(Clone, Copy)]
pub struct Decor {
    picture: u16,
    pub at: Vec2,
    scale: f32,
    /// Tall (trees, rocks): sorted in among the figures – by `depth`. Otherwise flat on the ground (herbs, ore) –
    /// drawn with it.
    pub tall: bool,
    /// Where it's sorted in (x + y): the middle of the ground it stands on, like TAB's position – not its front corner.
    /// Sorted by that, a tree at the forest's edge tied with a wall on the next tile and covered it.
    pub depth: f32,
    /// A herb: the tile it grows on – a building's floor hides it (TAB's `CInvisibleUnderBuilding`).
    herb_on: Option<(i32, i32)>,
}

impl Decor {
    /// A herb with a floor over it (`Floor`) – not drawn.
    pub fn is_paved(&self, floors: &[Floor]) -> bool {
        self.herb_on.is_some_and(|tile| floors.iter().any(|f| f.covers(tile)))
    }

    /// A herb – part of the ground; the other flat things are lumps of ore (the editor's layers, `draw::Layers`).
    pub fn is_herb(&self) -> bool {
        self.herb_on.is_some()
    }
}

/// The paved ground under one building (TAB's `CFloorUndeBuilding`): `area` in tiles, `mask` which of the three.
#[derive(Clone, Copy)]
pub struct Floor {
    area: Rect,
    mask: usize,
}

impl Floor {
    /// The floor under a building at `tile`, `size` tiles big – like TAB: a mask by its smaller side (1, 2, 3+ tiles),
    /// one tile further on every side (TAB: two from 3 tiles on – with its nearly solid big mask that paved too far,
    /// `FLOOR_REACH`); but where the ring around the building meets forest, rock, water or ore (anything but open
    /// ground), the floor stops before it on that side – and where one of the corners beyond it does, it stops before
    /// that above or below. Off the map it goes on. A building standing on ore itself (defence, the mines,
    /// `placement.rs`) gets none: the ore stays to be seen.
    pub fn under(map: &Map, tile: (i32, i32), size: (i32, i32)) -> Option<Floor> {
        let (x, y, w, h) = (tile.0, tile.1, size.0, size.1);
        if (y..y + h).any(|cy| (x..x + w).any(|cx| map.deposit(cx, cy).is_some())) {
            return None;
        }
        let side = w.min(h);
        let mask = (side.max(1) - 1).min(2) as usize;
        let reach = FLOOR_REACH;
        let (mut left, mut top, mut right, mut bottom) = (x - reach, y - reach, x + w + reach, y + h + reach);
        let n = map.size as i32;
        let open = |cx: i32, cy: i32| {
            !(0..n).contains(&cx)
                || !(0..n).contains(&cy)
                || (matches!(map.terrain(cx, cy), Terrain::Grass | Terrain::Dirt) && map.deposit(cx, cy).is_none())
        };
        // The rings around the building, each one tile further out, without their corners (TAB's `GetAdjacentCells`).
        for i in 0..reach {
            let (l, t, r, b) = (x - i, y - i, x + w + i, y + h + i);
            for cy in t..b {
                if !open(l - 1, cy) {
                    left = left.max(l);
                }
                if !open(r, cy) {
                    right = right.min(r);
                }
            }
            for cx in l..r {
                if !open(cx, t - 1) {
                    top = top.max(t);
                }
                if !open(cx, b) {
                    bottom = bottom.min(b);
                }
            }
        }
        // The corners the rings left out: something closed in one, and the floor above (or below) the building stops
        // at its edge – it keeps its width.
        let corner_closed = |xs: std::ops::Range<i32>, ys: std::ops::Range<i32>| ys.into_iter().any(|cy| xs.clone().any(|cx| !open(cx, cy)));
        if corner_closed(left..x, top..y) || corner_closed(x + w..right, top..y) {
            top = y;
        }
        if corner_closed(left..x, y + h..bottom) || corner_closed(x + w..right, y + h..bottom) {
            bottom = y + h;
        }
        let area = Rect::new(left as f32, top as f32, (right - left) as f32, (bottom - top) as f32);
        Some(Floor { area, mask })
    }

    fn covers(&self, (x, y): (i32, i32)) -> bool {
        self.area.contains(vec2(x as f32 + 0.5, y as f32 + 0.5))
    }
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
    /// The shader that draws the floor under a building – `None` until made, or if it couldn't be.
    floor_material: Option<Material>,
    /// The shader that draws the cliffs round the water (`draw_cliffs`) – `None` until made, or if it couldn't be.
    cliff_material: Option<Material>,
    /// What the mask and the things on the ground were made for: the map's revision, its size and the look.
    made_for: Option<(u32, usize, String)>,
    /// The things on the ground, per square of `CHUNK` tiles, row after row.
    chunks: Vec<Vec<Decor>>,
    /// The cliffs round the water, the same way – drawn on their own, over the water's shimmer (`draw_cliffs`).
    cliff_chunks: Vec<Vec<Decor>>,
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
            // Without it there's just no floor.
            match make_floor_material() {
                Ok(m) => self.floor_material = Some(m),
                Err(e) => self.problem = Some(e),
            }
            // Without it the cliffs are drawn as painted.
            match make_cliff_material() {
                Ok(m) => self.cliff_material = Some(m),
                Err(e) => self.problem = Some(e),
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
        let squares = (self.chunks_wide * self.chunks_wide) as usize;
        self.chunks = vec![Vec::new(); squares];
        self.cliff_chunks = vec![Vec::new(); squares];
        let Some(sheet) = self.looks.get(look).and_then(|l| l.as_ref()).map(|l| &l.sheet) else {
            return;
        };
        let mut all = Vec::new();
        mountains(map, sheet, &mut all);
        trees(map, ground, sheet, &mut all);
        herbs(map, ground, sheet, &mut all);
        ores(map, sheet, &mut all);
        let mut edges = Vec::new();
        cliffs(map, sheet, &mut edges);
        let wide = self.chunks_wide;
        for (list, chunks) in [(all, &mut self.chunks), (edges, &mut self.cliff_chunks)] {
            for decor in list {
                let (cx, cy) = ((decor.at.x as i32 / CHUNK).clamp(0, wide - 1), (decor.at.y as i32 / CHUNK).clamp(0, wide - 1));
                chunks[(cy * wide + cx) as usize].push(decor);
            }
            // Within a square from the back to the front – the flat ones are drawn in this order. (`sort_by` keeps
            // the order of equals: a tile's cliff pieces stay in TAB's order.)
            for chunk in chunks.iter_mut() {
                chunk.sort_by(|a, b| (a.at.x + a.at.y).total_cmp(&(b.at.x + b.at.y)));
            }
        }
    }

    /// The cliffs round the water in `area` – after the water's shimmer, so it doesn't lie on them. Through their
    /// shader (`make_cliff_material`): below its tile a piece shows only over water – a small pond's far cliff hung
    /// down past it onto the shore in front. Without the shader: as painted.
    pub fn draw_cliffs(&self, camera: &Camera, area: Region) {
        let (Some(look), Some(material), Some(mask)) = (self.look(), &self.cliff_material, &self.mask) else {
            in_chunks(&self.cliff_chunks, self.chunks_wide, area, |_| true, |d| self.draw_decor(camera, d, WHITE));
            return;
        };
        material.set_texture("Mask", mask.clone());
        material.set_uniform("Size", mask.width());
        gl_use_material(material);
        let (tw, th) = (look.objects.width(), look.objects.height());
        let (mut vertices, mut indices) = (Vec::new(), Vec::new());
        let flush = |vertices: &mut Vec<Vertex>, indices: &mut Vec<u16>| {
            if !vertices.is_empty() {
                draw_mesh(&Mesh { vertices: std::mem::take(vertices), indices: std::mem::take(indices), texture: Some(look.objects.clone()) });
            }
        };
        in_chunks(&self.cliff_chunks, self.chunks_wide, area, |_| true, |d| {
            let Some((corner, size, p)) = self.placed(look, camera, d) else {
                return;
            };
            // Per corner: where it is on screen, in the atlas – and (`normal`, for the shader) the ground under it
            // and how far that lies in front of the piece's tile (x + y past its feet).
            let k = vertices.len() as u16;
            for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                let s = corner + vec2(dx * size.x, dy * size.y);
                let ground = camera.screen_to_world(s);
                let mut v = Vertex::new(s.x, s.y, 0.0, (p.at[0] + dx * p.at[2]) / tw, (p.at[1] + dy * p.at[3]) / th, WHITE);
                v.normal = vec4(ground.x, ground.y, ground.x + ground.y - d.at.x - d.at.y, 0.0);
                vertices.push(v);
            }
            indices.extend_from_slice(&[k, k + 1, k + 2, k, k + 2, k + 3]);
            if vertices.len() >= 4 * 4000 {
                flush(&mut vertices, &mut indices);
            }
        });
        flush(&mut vertices, &mut indices);
        gl_use_default_material();
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
    /// The floors under the buildings: the pavement, as much as each one's mask lets through – on the ground, before
    /// the herbs and the fog.
    pub fn draw_floors(&self, camera: &Camera, floors: &[Floor]) {
        let (Some(look), Some(material)) = (self.look(), &self.floor_material) else {
            return;
        };
        let (Some((pavement, masks)), Some(sheet)) = (&look.floor, &look.sheet.floor) else {
            return;
        };
        material.set_texture("Floor", pavement.clone());
        material.set_texture("Masks", masks.clone());
        gl_use_material(material);
        for floor in floors {
            let a = floor.area;
            let corners = [vec2(a.x, a.y), vec2(a.right(), a.y), vec2(a.right(), a.bottom()), vec2(a.x, a.bottom())];
            let screen = corners.map(|w| camera.world_to_screen(w));
            let (lo, hi) = screen.iter().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), s| (lo.min(*s), hi.max(*s)));
            if lo.x > screen_width() || lo.y > screen_height() || hi.x < 0.0 || hi.y < 0.0 {
                continue;
            }
            // Per floor where it lies (its mask is stretched over it) and which mask – uniforms, so one draw each
            // (macroquad starts a new batch when a uniform changes).
            material.set_uniform("Area", [a.x, a.y, a.w, a.h]);
            material.set_uniform("Pick", [floor.mask as f32, sheet.tiles]);
            let vertices = corners.iter().zip(screen).map(|(w, s)| Vertex::new(s.x, s.y, 0.0, w.x, w.y, WHITE)).collect();
            draw_mesh(&Mesh { vertices, indices: vec![0, 1, 2, 0, 2, 3], texture: None });
        }
        gl_use_default_material();
    }

    pub fn decor_in(&self, area: Region, tall: bool, each: impl FnMut(&Decor)) {
        in_chunks(&self.chunks, self.chunks_wide, area, |d| d.tall == tall, each);
    }

    /// Draws one of them – `color` multiplied (white: as painted; darker in the fog).
    pub fn draw_decor(&self, camera: &Camera, decor: &Decor, color: Color) {
        let Some(look) = self.look() else {
            return;
        };
        let Some((corner, size, p)) = self.placed(look, camera, decor) else {
            return;
        };
        let source = Rect::new(p.at[0], p.at[1], p.at[2], p.at[3]);
        let params = DrawTextureParams { dest_size: Some(size), source: Some(source), ..Default::default() };
        draw_texture_ex(&look.objects, corner.x, corner.y, color, params);
    }

    /// Where on screen `decor`'s picture goes: its top left corner, its size – `None` off screen.
    fn placed<'a>(&self, look: &'a Look, camera: &Camera, decor: &Decor) -> Option<(Vec2, Vec2, &'a Picture)> {
        let p = look.sheet.pictures.get(decor.picture as usize)?;
        let scale = TILE_W / p.pixels_per_tile * camera.zoom * decor.scale;
        let size = vec2(p.at[2], p.at[3]) * scale;
        let corner = camera.world_to_screen(decor.at) - Vec2::from(p.feet) * scale;
        let off = corner.x > screen_width() || corner.y > screen_height() || corner.x + size.x < 0.0 || corner.y + size.y < 0.0;
        (!off).then_some((corner, size, p))
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
        let floor = match (&sheet.floor, &sheet.floor_masks) {
            (Some(pavement), Some(masks)) => {
                let (pavement, masks) = (picture(&pavement.image)?, picture(masks)?);
                smooth(&pavement, true);
                smooth(&masks, false);
                Some((pavement, masks))
            }
            _ => None,
        };
        Ok(Some(Look { sheet, earth, grass, water, objects, floor }))
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
    // water as much as it's green – that edge a bit sharper (TAB's water has a hard edge), and 0.3 tiles out into the
    // water: TAB lays a patch of earth over every shore tile, reaching that far past it (`Cliff`, 1.6 × 1.6 tiles,
    // round at the corners) – its cliffs hang down from there (`cliffs`). The mask is 0.5 at a tile's edge, 0.8 that
    // far past it.
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
    color = mix(color, water, smoothstep(0.7, 0.9, mask.g));
    gl_FragColor = vec4(color, 1.0);
}"#;
    let params = MaterialParams {
        uniforms: vec![UniformDesc::new("Tiles", UniformType::Float4)],
        textures: vec!["Earth".into(), "Grass".into(), "Water".into(), "Mask".into()],
        ..Default::default()
    };
    load_material(ShaderSource::Glsl { vertex, fragment }, params).map_err(|e| format!("ground shader: {e}"))
}

/// The program that draws a floor: the pavement repeating every `Pick.y` tiles (in world coordinates, so floors next
/// to each other run on without a seam), seen as much as the mask says – mask `Pick.x` of the three side by side,
/// stretched over the floor's `Area` (x, y, width, height in tiles).
fn make_floor_material() -> Result<Material, String> {
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
    let fragment = r#"#version 100
precision highp float;
varying highp vec2 world;
uniform sampler2D Floor;
uniform sampler2D Masks;
uniform vec4 Area;
uniform vec2 Pick;
void main() {
    vec2 inside = clamp((world - Area.xy) / Area.zw, 0.0, 1.0);
    float seen = texture2D(Masks, vec2((Pick.x + inside.x) / 3.0, inside.y)).r;
    gl_FragColor = vec4(texture2D(Floor, world / Pick.y).rgb, seen);
}"#;
    let blend = BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::OneMinusValue(BlendValue::SourceAlpha));
    let params = MaterialParams {
        uniforms: vec![UniformDesc::new("Area", UniformType::Float4), UniformDesc::new("Pick", UniformType::Float2)],
        textures: vec!["Floor".into(), "Masks".into()],
        pipeline_params: PipelineParams { color_blend: Some(blend), ..Default::default() },
    };
    load_material(ShaderSource::Glsl { vertex, fragment }, params).map_err(|e| format!("floor shader: {e}"))
}

/// The program that draws the cliffs: the picture as painted – but where the ground under a pixel lies in front of
/// the piece's tile (`ground.z`: x + y past its feet, `draw_cliffs`) and isn't water (the mask's green, one pixel per
/// tile, the map `Size` tiles big), it fades out within half a tile. TAB doesn't: its lakes are big enough that the
/// cliffs never reach across them (a deviation, DESIGN.md 2).
fn make_cliff_material() -> Result<Material, String> {
    let vertex = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
attribute vec4 normal;
varying lowp vec4 color;
varying highp vec2 uv;
varying highp vec3 ground;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    color = color0 / 255.0;
    uv = texcoord;
    ground = normal.xyz;
}"#;
    let fragment = r#"#version 100
precision highp float;
varying lowp vec4 color;
varying highp vec2 uv;
varying highp vec3 ground;
uniform sampler2D Texture;
uniform sampler2D Mask;
uniform float Size;
void main() {
    vec4 c = texture2D(Texture, uv) * color;
    float land = 1.0 - smoothstep(0.35, 0.6, texture2D(Mask, ground.xy / Size).g);
    c.a *= 1.0 - land * smoothstep(0.0, 0.5, ground.z);
    gl_FragColor = c;
}"#;
    let blend = BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::OneMinusValue(BlendValue::SourceAlpha));
    let params = MaterialParams {
        uniforms: vec![UniformDesc::new("Size", UniformType::Float1)],
        textures: vec!["Mask".into()],
        pipeline_params: PipelineParams { color_blend: Some(blend), ..Default::default() },
    };
    load_material(ShaderSource::Glsl { vertex, fragment }, params).map_err(|e| format!("cliff shader: {e}"))
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

/// Every one of `chunks` (squares of `CHUNK` tiles, `wide` per row) in `area` that `keep` keeps goes to `each`.
fn in_chunks(chunks: &[Vec<Decor>], wide: i32, area: Region, keep: impl Fn(&Decor) -> bool, mut each: impl FnMut(&Decor)) {
    if chunks.is_empty() {
        return;
    }
    // The squares the area touches (a bit more: a big tree stands out of its square).
    let (mut lo, mut hi) = (IVec2::splat(i32::MAX), IVec2::splat(i32::MIN));
    for c in area.corners() {
        let t = IVec2::new(c.x.floor() as i32, c.y.floor() as i32);
        lo = lo.min(t);
        hi = hi.max(t);
    }
    let (x0, y0) = (((lo.x - 3) / CHUNK).max(0), ((lo.y - 3) / CHUNK).max(0));
    let (x1, y1) = (((hi.x + 3) / CHUNK).min(wide - 1), ((hi.y + 3) / CHUNK).min(wide - 1));
    for cy in y0..=y1 {
        for cx in x0..=x1 {
            for decor in chunks[(cy * wide + cx) as usize].iter().filter(|d| keep(d) && area.contains(d.at)) {
                each(decor);
            }
        }
    }
}

/// TAB's cliffs round the water (`ZXMapDrawer.CreateCliffsFor`): on every land tile with water on one of its four
/// sides, the pieces whose rule fits the water around it – a side (B, T, L, R: water along a whole side), an outer
/// corner (BR, BL, TR, TL), an inner corner (IBR, IBL, ITR, ITL), a narrow tongue of land (UT, UB, UL, UR) or a lone
/// tip (X). Several can fit one tile. Which of a kind's pictures: by the tile's x (the sides L and R: by its y), like
/// TAB. They stand on the tile's front corner and hang down into the water.
fn cliffs(map: &Map, sheet: &Sheet, out: &mut Vec<Decor>) {
    if sheet.cliffs.is_empty() {
        return;
    }
    let n = map.size as i32;
    for y in 0..n {
        for x in 0..n {
            // Water this far from the tile? Off the map there is none (`Map::terrain`: mountain).
            let w = |dx: i32, dy: i32| map.terrain(x + dx, y + dy) == Terrain::Water;
            if w(0, 0) || !(w(1, 0) || w(-1, 0) || w(0, 1) || w(0, -1)) {
                continue;
            }
            // TAB's rules, in its order – `(kind, fits, number that picks the picture)`.
            let rules = [
                ("X", w(0, 1) && !w(0, 2) && !w(-1, 1) && !w(1, 1), x),
                ("UT", w(1, 0) && !w(1, -1) && !w(2, 0) && w(1, 1), x),
                ("UB", w(1, 0) && !w(1, 1) && !w(2, 0) && w(1, -1), x),
                ("UL", w(0, 1) && !w(0, 2) && !w(-1, 1) && w(1, 1), x),
                ("UR", w(0, 1) && !w(0, 2) && w(-1, 1) && !w(1, 1), x),
                ("B", w(0, 1) && w(-1, 1) && w(1, 1), x),
                ("T", w(0, -1) && w(-1, -1) && w(1, -1), x),
                ("R", w(1, 0) && w(1, 1) && w(1, -1), y),
                ("L", w(-1, 0) && w(-1, 1) && w(-1, -1), y),
                ("TR", w(0, -1) && w(1, -1) && w(1, 0), x),
                ("TL", w(0, -1) && w(-1, -1) && w(-1, 0), x),
                ("BR", w(0, 1) && w(1, 1) && w(1, 0), x),
                ("BL", w(0, 1) && w(-1, 1) && w(-1, 0), x),
                ("ITR", !w(-1, -1) && w(0, -1) && w(1, -1) && w(0, -2), x),
                ("ITL", !w(1, -1) && w(0, -1) && w(0, -2) && w(-1, -1), x),
                ("IBR", !w(1, -1) && w(2, 0) && w(1, 0) && w(1, 1), x),
                ("IBL", !w(-1, -1) && w(-1, 0) && w(-1, 1) && w(-2, 0), x),
            ];
            let at = vec2(x as f32 + 1.0, y as f32 + 1.0);
            for (kind, fits, by) in rules {
                let Some(list) = sheet.cliffs.get(kind).filter(|l| fits && !l.is_empty()) else {
                    continue;
                };
                let picture = list[by.rem_euclid(list.len() as i32) as usize] as u16;
                out.push(Decor { picture, at, scale: 1.0, tall: false, depth: at.x + at.y, herb_on: None });
            }
        }
    }
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

/// The pictures of a list of `count` that belong to the group a tile asks for (`Map::picture`: 1–3 = A, B, C): the
/// list cut into three parts, as even as it goes – TAB's lists hold their pictures in such runs (its trees: three
/// kinds, each in a few shapes). `None`: mixed (0), or the list is too short to have that group – then all of them.
/// A `Range` (`a..b`): the numbers from `a` up to, not including, `b` – a slice takes it as its part (`&list[a..b]`).
fn group(count: usize, picture: u8) -> Option<std::ops::Range<usize>> {
    let g = (picture as usize).checked_sub(1)?;
    // `div_ceil`: divided and rounded up.
    let among = (g * count).div_ceil(3)..((g + 1) * count).div_ceil(3);
    (!among.is_empty() && among.end <= count).then_some(among)
}

/// Is every tile of the rectangle (`x`, `y`, `w` × `h`) such a tile? (TAB's `IsAllAreaWithValue`.) Off the map counts as
/// mountain, like `Map::terrain` says.
fn all(map: &Map, x: i32, y: i32, w: i32, h: i32, terrain: Terrain) -> bool {
    (y..y + h).all(|ty| (x..x + w).all(|tx| map.terrain(tx, ty) == terrain))
}

/// A rock piece on every 2 × 2 block of mountain whose corner tile (even x and y) is mountain – TAB's. Deep inside a
/// range bigger (`SCALES`: the same piece up to 2.5 times; `HEIGHTS`: a taller piece). Single mountain tiles off that
/// grid (painted in the editor) get a small piece of their own.
fn mountains(map: &Map, sheet: &Sheet, out: &mut Vec<Decor>) {
    let n = map.size as i32;
    // `covered`: tiles that have their picture already – no small piece there.
    let mut covered = vec![false; (n * n) as usize];
    // First the big rocks: one picture each, standing on its front corner like the pieces. Only if the look has
    // pictures for its size – otherwise its tiles stay uncovered and get small pieces like any mountain.
    for rock in map.big_rocks() {
        let Some(size_index) = BIG_ROCK_SIZES.iter().position(|&s| s == rock.size) else {
            continue;
        };
        let Some(pics) = sheet.big_rocks.get(size_index).filter(|p| !p.is_empty()) else {
            continue;
        };
        let picture = pics[rock.variant as usize % pics.len()] as u16;
        let at = vec2((rock.tile.0 + rock.size) as f32, (rock.tile.1 + rock.size) as f32);
        let depth = at.x + at.y - rock.size as f32;
        out.push(Decor { picture, at, scale: 1.0, tall: true, depth, herb_on: None });
        for (x, y) in rock.tiles() {
            if (0..n).contains(&x) && (0..n).contains(&y) {
                covered[(y * n + x) as usize] = true;
            }
        }
    }
    let is_covered = |x: i32, y: i32, covered: &[bool]| covered[(y * n + x) as usize];
    // A piece on the square of `size` tiles at `x`, `y`, `scale` times as big. The pictures are made for 2 × 2.
    let put = |pics: &[usize], x: i32, y: i32, size: f32, scale: f32, out: &mut Vec<Decor>| {
        if pics.is_empty() {
            return;
        }
        // TAB picks the variant by the block (`GridTwoCells`) – among all its pictures, or among the group the editor's
        // painter chose for the block's first tile (`Map::picture`).
        let among = group(pics.len(), map.picture(x, y)).unwrap_or(0..pics.len());
        let variant = among.start + ((x / 2 + y / 2 * (among.len() as i32 / 2).max(1)) as usize) % among.len();
        // Its feet: the bottom corner of its square – scaled from the middle.
        let middle = vec2(x as f32, y as f32) + size / 2.0;
        let at = middle + Vec2::splat(size * scale / 2.0);
        let depth = middle.x + middle.y;
        out.push(Decor { picture: pics[variant] as u16, at, scale: scale * size / 2.0, tall: true, depth, herb_on: None });
    };
    // The pieces of a height (1–5); a landscape with only one: that one.
    let height = |level: usize| sheet.pieces.get(level - 1).or(sheet.pieces.first()).map(|p| p.as_slice()).unwrap_or(&[]);
    let full = map.full_area();
    for y in (0..n - 1).step_by(2) {
        for x in (0..n - 1).step_by(2) {
            if map.terrain(x, y) != Terrain::Mountain || !full.contains_tile(x, y) {
                continue;
            }
            // A block a big rock lies on (even partly): no piece – its tiles left over get small ones below.
            let block = [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)];
            if block.iter().any(|&(tx, ty)| is_covered(tx, ty, &covered)) {
                continue;
            }
            for (tx, ty) in block {
                covered[(ty * n + tx) as usize] = true;
            }
            // Is this row of the table met here: its block, and the square around it (the block in its middle) all mountain?
            let deep = |every: i32, square: i32| {
                let d = (square - 2) / 2;
                (x + y) % every == 0 && all(map, x - d, y - d, square, square, Terrain::Mountain)
            };
            if sheet.scale_pieces {
                let scale = SCALES.iter().find(|&&(every, square, _)| deep(every, square)).map_or(1.0, |row| row.2);
                put(height(1), x, y, 2.0, scale, out);
            } else {
                // TAB: the tallest only every 12 tiles or so – here every block deep enough whose numbers fit.
                let level = HEIGHTS.iter().find(|&&(every, square, _)| deep(every, square)).map_or(1, |row| row.2);
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
            // Among all the trees of this ground, by their weights – or only among the group the painter chose.
            let list = group(list.len(), map.picture(x, y)).map_or(list.as_slice(), |among| &list[among]);
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
            let depth = middle.x + middle.y;
            out.push(Decor { picture: picture as u16, at: middle + Vec2::splat(scale / 2.0), scale, tall: true, depth, herb_on: None });
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
                out.push(Decor { picture: picture as u16, at: middle + Vec2::splat(size / 2.0), scale: size / 0.3, tall: false, depth: 0.0, herb_on: Some((x, y)) });
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
                        out.push(Decor { picture: picture as u16, at: vec2(x as f32 + 2.0, y as f32 + 2.0), scale: 1.0, tall: false, depth: 0.0, herb_on: None });
                    }
                    continue;
                }
            };
            if pics.is_empty() {
                continue;
            }
            let pics = group(pics.len(), map.picture(x, y)).map_or(pics.as_slice(), |among| &pics[among]);
            for k in 0..4 {
                let salt = 40 + k as u32 * 4;
                let quarter = vec2(0.25 + (k % 2) as f32 * 0.5, 0.25 + (k / 2) as f32 * 0.5);
                let off = vec2(roll(x, y, salt) - 0.5, roll(x, y, salt + 1) - 0.5) * 0.44;
                let middle = vec2(x as f32, y as f32) + quarter + off;
                let size = 0.6 + (roll(x, y, salt + 2) - 0.5) * 0.3;
                let picture = pics[(roll(x, y, salt + 3) * pics.len() as f32) as usize % pics.len()];
                out.push(Decor { picture: picture as u16, at: middle + Vec2::splat(size / 2.0), scale: size, tall: false, depth: 0.0, herb_on: None });
            }
        }
    }
}
