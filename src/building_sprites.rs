//! Pictures for buildings and the houses of the villages of doom: TAB's, put together by `tools\tab_buildings.py`
//! (explained in `assets\README.md`). Only drawing – the simulation never sees them. Loaded and handed out by
//! `Sprites` (`sprites.rs`), like the figures.
//!
//! A building sheet is a JSON file and one PNG with all its pictures. Each picture knows where its **feet** are: the
//! lowest corner of the building's site on screen (its far corner on the map, `Building::far_corner`) – TAB puts its
//! buildings there too. Every building comes in three looks, like TAB's: **working** (lights on, the engine turning),
//! **idle** (switched off: no lights, standing still, paler) and **being built** (TAB's green ghost).
//!
//! A look is a list of **parts**, drawn back to front like TAB's layers: what stands still is one picture, every moving
//! thing (a saw, the sails, a flag, smoke) a part with its own frames and speed, every light a part that is **added** to
//! what's below (`glow`) – so it shines. The ballista's and the machine gun's gun is a part whose frames go once round
//! the circle: the one facing the target is drawn (`Aim`).

use std::cell::OnceCell;
use std::collections::HashMap;

use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation, PipelineParams};
use macroquad::prelude::*;
use serde::Deserialize;

use crate::data;
use crate::iso::{Camera, TILE_W};
use crate::sprites::smooth;

/// `assets\<name>.json`.
#[derive(Deserialize)]
struct Sheet {
    /// The picture, next to the JSON file.
    image: String,
    /// This many pixels are one tile width on screen at zoom 1 …
    pixels_per_tile: f32,
    /// … for a building this many tiles wide. One of another width (a house of doom bigger than TAB's) is drawn that
    /// much bigger or smaller.
    size: f32,
    /// Every picture: `[x, y, width, height, feet x, feet y]` in pixels – where it is in the PNG, and where in it the
    /// feet are.
    pictures: Vec<[f32; 6]>,
    /// The whole building – of a tower: its body, behind the units inside.
    body: Option<Appearance>,
    /// A tower's front wall, in front of the units inside …
    front: Option<Appearance>,
    /// … who stand this many tile widths above the middle of the site.
    #[serde(default)]
    roof: f32,
    /// Walls and the gate: the pieces they're put together of (`pillar`, `left`, `right`, `horizontal`, `vertical`,
    /// the stone wall's `pillar_straight`, the gate's `door_x` and `door_y`). `HashMap` is fine here: only the picture.
    #[serde(default)]
    pieces: HashMap<String, Appearance>,
}

/// One thing in its three looks – each a list of parts, back to front.
#[derive(Deserialize)]
pub struct Appearance {
    working: Vec<Part>,
    idle: Vec<Part>,
    building: Vec<Part>,
}

/// One part of a look: numbers in `pictures`.
#[derive(Deserialize)]
struct Part {
    /// Its frames, over and over at `fps` – one frame: it stands still.
    frames: Vec<usize>,
    #[serde(default)]
    fps: f32,
    /// A light: added to what's below instead of covering it (TAB's `BlendMode.Add`).
    #[serde(default)]
    add: bool,
    /// A gun that turns: its frames go once round the circle – the first one faces `turn_from` degrees (like
    /// `Unit::facing`: 0 lower right on screen, 90 lower left), each next one `turn` degrees further.
    turn_from: Option<f32>,
    #[serde(default)]
    turn: f32,
    /// The ballista's shot: for every 18 degrees from 0 the frames of one shot, played once in `shot_ms`.
    #[serde(default)]
    shot: Vec<Vec<usize>>,
    #[serde(default)]
    shot_ms: f32,
}

/// Which look a building shows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Finished, switched on, not infected – it does its job.
    Working,
    /// Switched off, infected, neutral.
    Idle,
    /// Still being built.
    Building,
}

/// Where a turning gun points (degrees, like `Unit::facing`) – and how far through a shot it is (0..1), if it shoots.
#[derive(Clone, Copy, Default)]
pub struct Aim {
    pub degrees: f32,
    pub shot: Option<f32>,
}

impl Appearance {
    fn parts(&self, state: State) -> &[Part] {
        match state {
            State::Working => &self.working,
            State::Idle => &self.idle,
            State::Building => &self.building,
        }
    }

    /// Every picture it names.
    fn all(&self) -> impl Iterator<Item = usize> + '_ {
        let parts = self.working.iter().chain(&self.idle).chain(&self.building);
        parts.flat_map(|p| p.frames.iter().chain(p.shot.iter().flatten())).copied()
    }

    /// How long one shot of its gun takes (seconds) – 0: it has no shot of its own.
    pub fn shot_seconds(&self) -> f32 {
        self.working.iter().find(|p| !p.shot.is_empty()).map_or(0.0, |p| p.shot_ms / 1000.0)
    }
}

impl Part {
    /// The picture to draw now – moving ones with `clock` (seconds of game time), a gun by its `aim`.
    fn picture(&self, clock: f32, aim: Aim) -> usize {
        let n = self.frames.len();
        if let Some(from) = self.turn_from {
            // The ballista only aims in steps of 18 degrees (TAB's `UseFixedAngles`) – the step its shot is drawn for.
            let degrees = if self.shot.is_empty() { aim.degrees } else { (aim.degrees / 18.0).floor() * 18.0 };
            if let (Some(share), false) = (aim.shot, self.shot.is_empty()) {
                let frames = &self.shot[(degrees / 18.0).rem_euclid(20.0) as usize % self.shot.len()];
                if !frames.is_empty() {
                    return frames[((share * frames.len() as f32) as usize).min(frames.len() - 1)];
                }
            }
            let step = if self.turn == 0.0 { 360.0 / n as f32 } else { self.turn };
            return self.frames[((degrees - from) / step).round().rem_euclid(n as f32) as usize % n];
        }
        if n > 1 && self.fps > 0.0 {
            return self.frames[((clock * self.fps) as usize) % n];
        }
        self.frames[0]
    }
}

pub struct BuildingSprite {
    sheet: Sheet,
    texture: Texture2D,
}

impl BuildingSprite {
    /// Reads `assets\<name>.json` and its picture. `Ok(None)`: there is no such JSON file – a skin made on another PC.
    pub fn load(name: &str) -> Result<Option<BuildingSprite>, String> {
        let folder = data::folder("assets").ok_or("no assets\\ folder")?;
        let json = folder.join(format!("{name}.json"));
        if !json.is_file() {
            return Ok(None);
        }
        let at = |e: &dyn std::fmt::Display| format!("{}: {e}", json.display());
        let text = std::fs::read_to_string(&json).map_err(|e| at(&e))?;
        let sheet: Sheet = serde_json::from_str(&text).map_err(|e| at(&e))?;
        let png = json.parent().unwrap().join(&sheet.image);
        let bytes = std::fs::read(&png).map_err(|e| format!("{}: {e}", png.display()))?;
        let image = Image::from_file_with_format(&bytes, None).map_err(|e| format!("{}: {e}", png.display()))?;
        sheet.check(image.width as f32, image.height as f32).map_err(|e| at(&e))?;
        let texture = Texture2D::from_image(&image);
        smooth(&texture);
        Ok(Some(BuildingSprite { sheet, texture }))
    }

    /// How many tiles wide its pictures are made for (`size` in the JSON).
    pub fn size(&self) -> f32 {
        self.sheet.size
    }

    pub fn body(&self) -> Option<&Appearance> {
        self.sheet.body.as_ref()
    }

    pub fn front(&self) -> Option<&Appearance> {
        self.sheet.front.as_ref()
    }

    /// How high the units inside a tower stand (tile widths above the middle of its site).
    pub fn roof(&self) -> f32 {
        self.sheet.roof
    }

    pub fn piece(&self, name: &str) -> Option<&Appearance> {
        self.sheet.pieces.get(name)
    }

    /// Does it have the pieces of a wall (or a gate)?
    pub fn has_pieces(&self) -> bool {
        !self.sheet.pieces.is_empty()
    }

    /// One of its pictures for this look, as the first part of `looks` shows it – a carried barrel (`draw.rs`).
    pub fn first_picture(&self, looks: &Appearance, state: State) -> Option<usize> {
        looks.parts(state).first().map(|p| p.frames[0])
    }

    /// Draws one of its things in a look, its feet on `at` (tiles) – for a building `width` tiles wide. `clock`:
    /// seconds of game time for what moves, `aim`: where its gun points. `color`: white = as painted.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, camera: &Camera, looks: &Appearance, state: State, clock: f32, aim: Aim, at: Vec2, width: f32, color: Color) {
        let feet = camera.world_to_screen(at);
        let scale = TILE_W / self.sheet.pixels_per_tile * camera.zoom * width / self.sheet.size;
        for part in looks.parts(state) {
            let picture = part.picture(clock, aim);
            if part.add {
                glow(|| self.draw_picture(picture, feet, scale, color));
            } else {
                self.draw_picture(picture, feet, scale, color);
            }
        }
    }

    /// Draws picture `i` with its feet on `feet` (pixels on screen), `scale` screen pixels per pixel of the sheet.
    /// `squash`: its height times this (a shadow lying on the ground: less than 1).
    pub fn draw_picture(&self, i: usize, feet: Vec2, scale: f32, color: Color) {
        self.draw_squashed(i, feet, scale, 1.0, color);
    }

    pub fn draw_squashed(&self, i: usize, feet: Vec2, scale: f32, squash: f32, color: Color) {
        let [x, y, w, h, fx, fy] = self.sheet.pictures[i];
        let corner = feet - vec2(fx, fy * squash) * scale;
        let size = vec2(w, h * squash) * scale;
        if corner.x > screen_width() || corner.y > screen_height() || corner.x + size.x < 0.0 || corner.y + size.y < 0.0 {
            return;
        }
        let params = DrawTextureParams { dest_size: Some(size), source: Some(Rect::new(x, y, w, h)), ..Default::default() };
        draw_texture_ex(&self.texture, corner.x, corner.y, color, params);
    }

    /// How many screen pixels one pixel of its sheet is, for a thing `width` tiles wide.
    pub fn scale(&self, camera: &Camera, width: f32) -> f32 {
        TILE_W / self.sheet.pixels_per_tile * camera.zoom * width / self.sheet.size
    }
}

impl Sheet {
    /// Does every picture lie in the PNG (`width` × `height`), and does every look name a picture there is?
    fn check(&self, width: f32, height: f32) -> Result<(), String> {
        if self.pixels_per_tile <= 0.0 || self.size <= 0.0 {
            return Err("pixels_per_tile and size have to be more than 0".to_string());
        }
        for (i, [x, y, w, h, ..]) in self.pictures.iter().enumerate() {
            if x + w > width || y + h > height {
                return Err(format!("picture {i} reaches out of the PNG ({width} × {height} pixels)"));
            }
        }
        let looks = self.body.iter().chain(&self.front).chain(self.pieces.values());
        for looks in looks {
            if let Some(i) = looks.all().find(|&i| i >= self.pictures.len()) {
                return Err(format!("there is no picture {i}"));
            }
            let parts = looks.working.iter().chain(&looks.idle).chain(&looks.building);
            if parts.clone().any(|p| p.frames.is_empty()) {
                return Err("a part without frames".to_string());
            }
        }
        if self.body.is_none() && self.pieces.is_empty() {
            return Err("neither \"body\" nor \"pieces\"".to_string());
        }
        Ok(())
    }
}

thread_local! {
    /// The way lights are drawn (`glow`), made once – `None` if the graphics card wouldn't take it: then lights are
    /// drawn like everything else. `thread_local`: macroquad draws on one thread only, a `Material` can't be shared.
    static GLOW: OnceCell<Option<Material>> = const { OnceCell::new() };
}

/// Whatever `draw` draws is **added** to what's already on the screen – its colour times its alpha – instead of
/// covering it (TAB's `BlendMode.Add`): lit windows, a muzzle flash, sparks shine on what's below. A small shader of our
/// own that colours like macroquad's (`texture × colour`), only the blending differs. Also for `effects.rs`.
pub fn glow(draw: impl FnOnce()) {
    GLOW.with(|cell| {
        let material = cell.get_or_init(|| make_glow().map_err(|e| eprintln!("{e}")).ok());
        match material {
            Some(material) => {
                gl_use_material(material);
                draw();
                gl_use_default_material();
            }
            None => draw(),
        }
    });
}

fn make_glow() -> Result<Material, String> {
    let vertex = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    color = color0 / 255.0;
    uv = texcoord;
}"#;
    let fragment = r#"#version 100
varying lowp vec2 uv;
varying lowp vec4 color;
uniform sampler2D Texture;
void main() {
    gl_FragColor = color * texture2D(Texture, uv);
}"#;
    // On screen: what's there + new colour × new alpha. The screen's own alpha stays as it is.
    let pipeline_params = PipelineParams {
        color_blend: Some(BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::One)),
        alpha_blend: Some(BlendState::new(Equation::Add, BlendFactor::Zero, BlendFactor::One)),
        ..Default::default()
    };
    let params = MaterialParams { pipeline_params, ..Default::default() };
    load_material(ShaderSource::Glsl { vertex, fragment }, params).map_err(|e| format!("glow shader: {e}"))
}
