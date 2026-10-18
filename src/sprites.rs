//! Pictures for zombies and units (DESIGN.md 12.3): pre-rendered sprite sheets like in TAB. Only drawing – the
//! simulation never sees them.
//!
//! A sprite is a JSON file in `assets\` and its picture (PNG) – explained in `assets\README.md`. A kind names its
//! sprite in `data\` (`"sprite": "test/figure"` → `assets\test\figure.json`). Without one, or if its files are missing
//! or broken, it stays a box (`draw.rs`) and `problem` says why.

use std::collections::HashMap;
use std::f32::consts::TAU;

use macroquad::miniquad::MipmapFilterMode;
use macroquad::prelude::*;
use serde::Deserialize;

use crate::data;
use crate::iso::{Camera, TILE_W};
use crate::kinds::UnitKind;
use crate::sim::Breed;

/// `assets\<name>.json`: what's in the picture and where.
#[derive(Deserialize)]
struct Sheet {
    /// The picture, next to the JSON file.
    image: String,
    /// Width and height of one frame (pixels).
    frame: [f32; 2],
    /// Where the feet are in a frame (pixels from its top left corner) – this point stands on the figure's position.
    feet: [f32; 2],
    /// This many pixels of the picture are one tile width on screen (`TILE_W` at zoom 1) – how big it is drawn.
    pixels_per_tile: f32,
    /// Each animation has one row per direction: the first one facing +x (on screen to the lower right), then turning
    /// towards +y (clockwise on screen) – like `Unit::facing`.
    directions: u32,
    /// `true`: drawn in the colour of its kind (`draw.rs`) – for a grey test picture that several kinds share.
    #[serde(default)]
    tint: bool,
    idle: Animation,
    /// Missing: idle instead.
    walk: Option<Animation>,
    attack: Option<Animation>,
}

/// One animation: its frames side by side, its directions one below the other.
#[derive(Deserialize)]
struct Animation {
    /// Column and row of its first frame (direction 0) in the picture, counted in frames.
    at: [u32; 2],
    frames: u32,
    /// Frames per second …
    #[serde(default)]
    fps: f32,
    /// … or, for walking: one loop of it goes this many tiles – a faster figure steps faster, the feet don't slide.
    #[serde(default)]
    tiles: f32,
}

/// What a figure is doing – which animation, and how far along it is.
#[derive(Clone, Copy)]
pub enum Pose {
    Idle,
    /// Walking at this speed (tiles per second).
    Walk(f32),
    /// Attacking. `Some`: this far through one swing (0..1 – a zombie's hit). `None`: over and over (a unit aiming).
    Attack(Option<f32>),
}

pub struct Sprite {
    sheet: Sheet,
    texture: Texture2D,
}

impl Sprite {
    /// Reads `assets\<name>.json` and its picture. `Err`: what's wrong, with the file's name.
    fn load(name: &str) -> Result<Sprite, String> {
        let folder = data::folder("assets").ok_or("no assets\\ folder")?;
        let json = folder.join(format!("{name}.json"));
        let at = |e: &dyn std::fmt::Display| format!("{}: {e}", json.display());
        let text = std::fs::read_to_string(&json).map_err(|e| at(&e))?;
        let sheet: Sheet = serde_json::from_str(&text).map_err(|e| at(&e))?;
        // `parent`: the folder the JSON file is in – there is always one, it was just read.
        let png = json.parent().unwrap().join(&sheet.image);
        let bytes = std::fs::read(&png).map_err(|e| format!("{}: {e}", png.display()))?;
        let image = Image::from_file_with_format(&bytes, None).map_err(|e| format!("{}: {e}", png.display()))?;
        sheet.check(image.width as f32, image.height as f32).map_err(|e| at(&e))?;
        let texture = Texture2D::from_image(&image);
        smooth(&texture);
        Ok(Sprite { sheet, texture })
    }

    /// Drawn in the colour of its kind (`tint` in the JSON)?
    pub fn tinted(&self) -> bool {
        self.sheet.tint
    }

    /// Draws the figure standing at `at` (tiles), facing `facing` (radians, like `Unit::facing`). `clock`: seconds of
    /// game time – a bit different for every figure, so a crowd doesn't move in step. `color`: white = as painted.
    pub fn draw(&self, camera: &Camera, at: Vec2, facing: f32, pose: Pose, clock: f32, color: Color) {
        let sheet = &self.sheet;
        let scale = TILE_W / sheet.pixels_per_tile * camera.zoom;
        let size = Vec2::from(sheet.frame) * scale;
        let corner = camera.world_to_screen(at) - Vec2::from(sheet.feet) * scale;
        // Off the screen: not drawn at all.
        if corner.x > screen_width() || corner.y > screen_height() || corner.x + size.x < 0.0 || corner.y + size.y < 0.0 {
            return;
        }
        let (animation, frame) = self.frame(pose, clock);
        // The closest of the directions there are (`round`), counted from 0 again after a full turn (`rem_euclid`).
        let n = sheet.directions as f32;
        let direction = (facing / TAU * n).round().rem_euclid(n) as u32;
        let [w, h] = sheet.frame;
        let source = Rect::new((animation.at[0] + frame) as f32 * w, (animation.at[1] + direction) as f32 * h, w, h);
        let params = DrawTextureParams { dest_size: Some(size), source: Some(source), ..Default::default() };
        draw_texture_ex(&self.texture, corner.x, corner.y, color, params);
    }

    /// Which animation, and which of its frames. One the sheet doesn't have: idle instead.
    fn frame(&self, pose: Pose, clock: f32) -> (&Animation, u32) {
        let sheet = &self.sheet;
        let wanted = match pose {
            Pose::Idle => None,
            Pose::Walk(_) => sheet.walk.as_ref(),
            Pose::Attack(_) => sheet.attack.as_ref(),
        };
        let Some(animation) = wanted else {
            return (&sheet.idle, sheet.idle.looping(clock));
        };
        let frame = match pose {
            Pose::Walk(speed) if animation.tiles > 0.0 => animation.at_share(clock * speed / animation.tiles),
            Pose::Attack(Some(share)) => animation.at_share(share),
            _ => animation.looping(clock),
        };
        (animation, frame)
    }
}

impl Sheet {
    /// Does every animation fit into the picture (`width` × `height` pixels)?
    fn check(&self, width: f32, height: f32) -> Result<(), String> {
        if self.directions == 0 || self.pixels_per_tile <= 0.0 || self.frame[0] <= 0.0 || self.frame[1] <= 0.0 {
            return Err("frame, pixels_per_tile and directions have to be more than 0".to_string());
        }
        let animations = [("idle", Some(&self.idle)), ("walk", self.walk.as_ref()), ("attack", self.attack.as_ref())];
        for (name, animation) in animations {
            let Some(animation) = animation else {
                continue;
            };
            let right = (animation.at[0] + animation.frames) as f32 * self.frame[0];
            let bottom = (animation.at[1] + self.directions) as f32 * self.frame[1];
            if animation.frames == 0 {
                return Err(format!("\"{name}\" has no frames"));
            }
            if right > width || bottom > height {
                return Err(format!("\"{name}\" reaches out of the picture ({width} × {height} pixels)"));
            }
        }
        Ok(())
    }
}

impl Animation {
    /// The frame at this share of one loop – it goes round (1.25 = a quarter into the next loop).
    fn at_share(&self, share: f32) -> u32 {
        ((share.rem_euclid(1.0) * self.frames as f32) as u32).min(self.frames - 1)
    }

    /// Playing over and over at `fps`.
    fn looping(&self, clock: f32) -> u32 {
        self.at_share(clock * self.fps / self.frames as f32)
    }
}

/// Smooth when drawn smaller or bigger – and with mipmaps: ready-made smaller copies of the picture (½, ¼ …), so a
/// zoomed-out horde doesn't flicker. macroquad has no call for mipmaps, so this goes straight to miniquad, the layer
/// below it.
fn smooth(texture: &Texture2D) {
    texture.set_filter(FilterMode::Linear);
    let id = texture.raw_miniquad_id();
    // `unsafe`: Rust can't check this call itself – macroquad's drawing state could get out of step with the graphics
    // card. Safe here: it only touches this new texture, nothing else is being drawn with it yet.
    let gl = unsafe { get_internal_gl() };
    gl.quad_context.texture_generate_mipmaps(id);
    gl.quad_context.texture_set_min_filter(id, FilterMode::Linear, MipmapFilterMode::Linear);
}

/// The sprites in use, each loaded once – by the name `data\` gives it.
#[derive(Default)]
pub struct Sprites {
    /// `None`: tried, but missing or broken – not tried again every frame.
    loaded: HashMap<String, Option<Sprite>>,
    /// What was wrong with the last one that didn't load.
    problem: Option<String>,
}

impl Sprites {
    /// Loads every sprite that `data\` names and that hasn't been tried yet. Before drawing – a texture needs the
    /// window. Throw the whole `Sprites` away to read changed files again (`game.rs`, "Restart").
    pub fn load_missing(&mut self) {
        let units = UnitKind::ALL.map(|kind| kind.stats().sprite.as_deref());
        let breeds = Breed::ALL.map(|breed| breed.stats().sprite.as_deref());
        // `flatten`: skips the kinds without a sprite (`None`).
        for name in units.into_iter().chain(breeds).flatten() {
            if self.loaded.contains_key(name) {
                continue;
            }
            let sprite = Sprite::load(name).map_err(|e| self.problem = Some(e)).ok();
            self.loaded.insert(name.to_string(), sprite);
        }
    }

    /// The sprite of this name – `None` without a name, or if it didn't load.
    pub fn get(&self, name: Option<&str>) -> Option<&Sprite> {
        self.loaded.get(name?)?.as_ref()
    }

    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }
}
