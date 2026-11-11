//! Pictures for zombies and units (DESIGN.md 12.3): pre-rendered sprite sheets like in TAB. Only drawing – the
//! simulation never sees them.
//!
//! A sprite is a JSON file in `assets\` and its picture (PNG) – explained in `assets\README.md`. A kind names its
//! **skins** in `data\` (`"skins": ["zombies/walker_aged", …]` → `assets\zombies\walker_aged.json` …): every unit and
//! every zombie wears one of them, for life – the number `look` picks it (`Sprites::unit`, `Sprites::breed`). A skin
//! whose JSON file isn't there is left out without a word: TAB's pictures only lie on the PC that converted them
//! (`tools\tab_skins.py`). None of them there: the kind's `sprite` (the grey test figure). Without that either, or if a
//! file is broken, it stays a box (`draw.rs`) and `problem` says why.

use std::collections::HashMap;
use std::f32::consts::TAU;

use macroquad::miniquad::MipmapFilterMode;
use macroquad::prelude::*;
use serde::Deserialize;

use crate::building_sprites::BuildingSprite;
use crate::data;
use crate::iso::{Camera, TILE_W};
use crate::kinds::{BuildingKind, UnitKind};
use crate::sim::{Breed, Nest};

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
    /// towards +y (clockwise on screen) – like `Unit::facing`. With `mirror` only half of them, see there.
    directions: u32,
    /// `true`: only the directions facing right on screen are in the picture, like in TAB – the first row half a step
    /// clockwise from straight up, the last one half a step before straight down. Facing left, a figure is drawn as
    /// the mirror image of facing right. Left out: `false`, every direction has its row.
    #[serde(default)]
    mirror: bool,
    /// `true`: drawn in the colour of its kind (`draw.rs`) – for a grey test picture that several kinds share.
    #[serde(default)]
    tint: bool,
    idle: Animation,
    /// Missing: idle instead.
    walk: Option<Animation>,
    /// Running – walking at `run_from` tiles per second or faster (TAB's zombies walk when they roam, run when they
    /// charge). Missing: always `walk`.
    run: Option<Animation>,
    #[serde(default)]
    run_from: f32,
    /// One attack: a unit's shot (TAB's `Attack` – starts and ends with the weapon up), a zombie's hit.
    attack: Option<Animation>,
    /// Bringing the weapon up (TAB's `LoadAttack`; down again: backwards). Its last frame is the pose of a unit that
    /// holds its weapon up between shots. Missing: the first frame of `attack` once it's all the way up.
    load: Option<Animation>,
    /// Growling (TAB's "special" animation: an idle zombie shakes and growls, a giant roars). Missing: idle.
    special: Option<Animation>,
    /// Dying (TAB's "die"): played once at `fps`, then its last frame held – the body lying there. Missing: idle.
    die: Option<Animation>,
    /// A second attack from close by (Thanatos hits with its launcher, TAB's "Fight"). Missing: `attack`.
    fight: Option<Animation>,
    /// Only the raven: taking off (TAB's "PrepareFly" – backwards: landing) and flying. Missing: idle.
    take_off: Option<Animation>,
    fly: Option<Animation>,
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
    /// Attacking. `Some`: this far through one swing (0..1 – a zombie's hit). `None`: over and over (the medic's beam).
    Attack(Option<f32>),
    /// A unit with its weapon, standing (`sim/combat.rs`): `up` how far it's up (0..1, `load`), `shot` seconds since
    /// its shot's animation started (`attack`, played once at `fps`) – after that, the weapon held up.
    Weapon { up: f32, shot: Option<f32> },
    /// Growling for this many seconds now – played once at `fps`, then its last frame held.
    Special(f32),
    /// Dead for this many seconds – like `Special`.
    Die(f32),
    /// The close attack (`fight`), this far through one hit (0..1).
    Fight(f32),
    /// Taking off for this many seconds (played once), landing (the same backwards), flying (over and over).
    TakeOff(f32),
    Land(f32),
    Fly,
}

pub struct Sprite {
    sheet: Sheet,
    texture: Texture2D,
}

impl Sprite {
    /// Reads `assets\<name>.json` and its picture. `Ok(None)`: there is no such JSON file – a skin that wasn't made on
    /// this PC. `Err`: what's wrong, with the file's name.
    fn load(name: &str) -> Result<Option<Sprite>, String> {
        let folder = data::folder("assets").ok_or("no assets\\ folder")?;
        let json = folder.join(format!("{name}.json"));
        if !json.is_file() {
            return Ok(None);
        }
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
        Ok(Some(Sprite { sheet, texture }))
    }

    /// Drawn in the colour of its kind (`tint` in the JSON)?
    pub fn tinted(&self) -> bool {
        self.sheet.tint
    }

    /// Does it have a death (`die`)? Without one a dead figure isn't drawn as one.
    pub fn dies(&self) -> bool {
        self.sheet.die.is_some()
    }

    /// Draws the figure standing at `at` (tiles), facing `facing` (radians, like `Unit::facing`). `clock`: seconds of
    /// game time – a bit different for every figure, so a crowd doesn't move in step. `color`: white = as painted.
    pub fn draw(&self, camera: &Camera, at: Vec2, facing: f32, pose: Pose, clock: f32, color: Color) {
        let sheet = &self.sheet;
        let scale = TILE_W / sheet.pixels_per_tile * camera.zoom;
        let size = Vec2::from(sheet.frame) * scale;
        let [w, h] = sheet.frame;
        let (direction, flip) = self.direction(facing);
        // Mirrored, the feet are as far from the right edge as they were from the left one.
        let feet = vec2(if flip { w - sheet.feet[0] } else { sheet.feet[0] }, sheet.feet[1]);
        let corner = camera.world_to_screen(at) - feet * scale;
        // Off the screen: not drawn at all.
        if corner.x > screen_width() || corner.y > screen_height() || corner.x + size.x < 0.0 || corner.y + size.y < 0.0 {
            return;
        }
        let (animation, frame) = self.frame(pose, clock);
        let source = Rect::new((animation.at[0] + frame) as f32 * w, (animation.at[1] + direction) as f32 * h, w, h);
        let params = DrawTextureParams { dest_size: Some(size), source: Some(source), flip_x: flip, ..Default::default() };
        draw_texture_ex(&self.texture, corner.x, corner.y, color, params);
    }

    /// Which row of an animation shows a figure facing `facing` (radians) – and whether it's drawn mirrored.
    fn direction(&self, facing: f32) -> (u32, bool) {
        let n = self.sheet.directions as f32;
        if !self.sheet.mirror {
            // The closest of the directions there are (`round`), counted from 0 again after a full turn (`rem_euclid`).
            return ((facing / TAU * n).round().rem_euclid(n) as u32, false);
        }
        // TAB's way (`DXAtlasWithActionsAndAngles`), in degrees: 0 faces +x (lower right on screen), 45 straight down,
        // 225 straight up. Facing left on screen (more than 45, up to 225) is the mirror image of facing right – mirrored
        // across the line from up to down, the 45° line.
        let degrees = facing.to_degrees().rem_euclid(360.0);
        let (degrees, flip) = if degrees > 45.0 && degrees <= 225.0 { (90.0 - degrees, true) } else { (degrees, false) };
        // Now from straight up (-135) to straight down (45), turning clockwise – the first row is half a step past -135.
        let turned = (degrees + 180.0).rem_euclid(360.0) - 180.0;
        let step = 180.0 / n;
        let row = ((turned + 135.0 - step / 2.0) / step).round().clamp(0.0, n - 1.0);
        (row as u32, flip)
    }

    /// Which animation, and which of its frames. One the sheet doesn't have: idle instead.
    fn frame(&self, pose: Pose, clock: f32) -> (&Animation, u32) {
        let sheet = &self.sheet;
        if let Pose::Weapon { up, shot } = pose {
            if let Some(found) = self.weapon_frame(up, shot) {
                return found;
            }
            return (&sheet.idle, sheet.idle.looping(clock));
        }
        let wanted = match pose {
            Pose::Idle => None,
            // Fast enough to run, and there is a running animation: that one – otherwise walking, or running if it has
            // only that (TAB's units).
            Pose::Walk(speed) if speed >= sheet.run_from && sheet.run.is_some() => sheet.run.as_ref(),
            Pose::Walk(_) => sheet.walk.as_ref().or(sheet.run.as_ref()),
            Pose::Attack(_) => sheet.attack.as_ref(),
            Pose::Special(_) => sheet.special.as_ref(),
            Pose::Die(_) => sheet.die.as_ref(),
            Pose::Fight(_) => sheet.fight.as_ref().or(sheet.attack.as_ref()),
            Pose::TakeOff(_) | Pose::Land(_) => sheet.take_off.as_ref(),
            Pose::Fly => sheet.fly.as_ref(),
            Pose::Weapon { .. } => unreachable!("handled above"),
        };
        let Some(animation) = wanted else {
            return (&sheet.idle, sheet.idle.looping(clock));
        };
        let frame = match pose {
            Pose::Walk(speed) if animation.tiles > 0.0 => animation.at_share(clock * speed / animation.tiles),
            Pose::Attack(Some(share)) => animation.at_share(share),
            Pose::Fight(share) => animation.at_share(share.min(0.999)),
            Pose::Special(seconds) | Pose::Die(seconds) | Pose::TakeOff(seconds) => ((seconds.max(0.0) * animation.fps) as u32).min(animation.frames - 1),
            Pose::Land(seconds) => animation.frames - 1 - ((seconds.max(0.0) * animation.fps) as u32).min(animation.frames - 1),
            _ => animation.looping(clock),
        };
        (animation, frame)
    }

    /// `Pose::Weapon`: the shot's animation while it plays, otherwise `load` at how far the weapon is up. `None`: the
    /// weapon is down (or the sheet has neither) – standing idle.
    fn weapon_frame(&self, up: f32, shot: Option<f32>) -> Option<(&Animation, u32)> {
        let sheet = &self.sheet;
        if let (Some(seconds), Some(attack)) = (shot, sheet.attack.as_ref()) {
            let frame = (seconds.max(0.0) * attack.fps) as u32;
            if frame < attack.frames {
                return Some((attack, frame));
            }
        }
        if up <= 0.0 {
            return None;
        }
        match (sheet.load.as_ref(), sheet.attack.as_ref()) {
            // Frame 0 is the weapon down, the last one up – a share of the way up is that share of the frames.
            (Some(load), _) => Some((load, ((up * load.frames as f32) as u32).min(load.frames - 1))),
            // No `load` (older sheets, the test figure): the first frame of the shot once it's up.
            (None, Some(attack)) if up >= 1.0 => Some((attack, 0)),
            _ => None,
        }
    }
}

impl Sheet {
    /// Does every animation fit into the picture (`width` × `height` pixels)?
    fn check(&self, width: f32, height: f32) -> Result<(), String> {
        if self.directions == 0 || self.pixels_per_tile <= 0.0 || self.frame[0] <= 0.0 || self.frame[1] <= 0.0 {
            return Err("frame, pixels_per_tile and directions have to be more than 0".to_string());
        }
        let animations = [
            ("idle", Some(&self.idle)),
            ("walk", self.walk.as_ref()),
            ("run", self.run.as_ref()),
            ("attack", self.attack.as_ref()),
            ("load", self.load.as_ref()),
            ("special", self.special.as_ref()),
            ("die", self.die.as_ref()),
            ("fight", self.fight.as_ref()),
            ("take_off", self.take_off.as_ref()),
            ("fly", self.fly.as_ref()),
        ];
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
pub(crate) fn smooth(texture: &Texture2D) {
    texture.set_filter(FilterMode::Linear);
    let id = texture.raw_miniquad_id();
    // `unsafe`: Rust can't check this call itself – macroquad's drawing state could get out of step with the graphics
    // card. Safe here: it only touches this new texture, nothing else is being drawn with it yet.
    let gl = unsafe { get_internal_gl() };
    gl.quad_context.texture_generate_mipmaps(id);
    gl.quad_context.texture_set_min_filter(id, FilterMode::Linear, MipmapFilterMode::Linear);
}

/// TAB's animals (`critters.rs`).
const ANIMALS: [&str; 2] = ["animals/raven", "animals/rat"];

/// The pictures of `assets\decor\` the game draws: wheat on a farm's fields, loot, the treasure ruins of every
/// landscape, the power line towers.
const DECOR: [&str; 17] = [
    "wheat",
    "loot_gold",
    "loot_wood",
    "loot_stone",
    "loot_iron",
    "loot_oil_1",
    "loot_food",
    "loot_energy",
    "loot_science",
    "treasure_ruin_br",
    "treasure_ruin_tm",
    "treasure_ruin_al",
    "treasure_ruin_ds",
    "treasure_ruin_vo",
    "power_tower_high",
    "power_tower_high_flip",
    "loot_oil_2",
];

/// The sprites in use, each loaded once – by the name `data\` gives it – and which of them every kind wears.
#[derive(Default)]
pub struct Sprites {
    /// `None`: tried, but there is no such file, or it's broken – not tried again every frame.
    loaded: HashMap<String, Option<Sprite>>,
    /// Per unit type (in the order of `UnitKind::ALL`) and per breed (`Breed::ALL`): the names of its skins that
    /// loaded – or just its `sprite` if none did. Empty: it's drawn as a box. Both empty: not loaded yet.
    units: Vec<Vec<String>>,
    breeds: Vec<Vec<String>>,
    /// The same for buildings (`building_sprites.rs`): per type (`BuildingKind::ALL`) and per size of house of doom
    /// (small, medium, big) the names of its skins that loaded. Empty: a block.
    buildings_loaded: HashMap<String, Option<BuildingSprite>>,
    buildings: Vec<Vec<String>>,
    doom: Vec<Vec<String>>,
    /// What was wrong with the last one that didn't load.
    problem: Option<String>,
}

impl Sprites {
    /// Loads the skins and sprites that `data\` names, once. Before drawing – a texture needs the window. Throw the
    /// whole `Sprites` away to read changed files again (`game.rs`, "Restart").
    pub fn load_missing(&mut self) {
        if !self.units.is_empty() {
            return;
        }
        // `map … collect`: one list per kind, in the order of `ALL` – like a list comprehension in Python.
        self.units = UnitKind::ALL.iter().map(|k| self.looks(&k.stats().skins, k.stats().sprite.as_deref())).collect();
        self.breeds = Breed::ALL.iter().map(|b| self.looks(&b.stats().skins, b.stats().sprite.as_deref())).collect();
        self.buildings = BuildingKind::ALL.iter().map(|k| self.building_looks(&k.stats().skins)).collect();
        for name in ANIMALS {
            self.try_load(name, false);
        }
        let decor: Vec<String> = DECOR.iter().map(|name| format!("decor/{name}")).collect();
        self.building_looks(&decor);
        self.doom = match &data::get().doom.skins {
            Some(skins) => [&skins.small, &skins.medium, &skins.big].map(|s| self.building_looks(s)).to_vec(),
            None => vec![Vec::new(); 3],
        };
    }

    /// The building skins of this list that load.
    fn building_looks(&mut self, skins: &[String]) -> Vec<String> {
        let mut found = Vec::new();
        for name in skins {
            if !self.buildings_loaded.contains_key(name) {
                let sprite = BuildingSprite::load(name).unwrap_or_else(|e| {
                    self.problem = Some(e);
                    None
                });
                self.buildings_loaded.insert(name.clone(), sprite);
            }
            if self.buildings_loaded[name].is_some() {
                found.push(name.clone());
            }
        }
        found
    }

    /// The picture of a building of this type, skin number `look`. `None`: it's drawn as blocks.
    pub fn building(&self, kind: BuildingKind, look: u32) -> Option<&BuildingSprite> {
        self.pick_building(&self.buildings, kind as usize, look)
    }

    /// A picture of `assets\decor\` (`DECOR`, same format as a building): `"wheat"`, `"loot_gold"` … `None`: not there.
    pub fn decor(&self, name: &str) -> Option<&BuildingSprite> {
        self.buildings_loaded.get(&format!("decor/{name}"))?.as_ref()
    }

    /// A sprite of `ANIMALS` by its name (`"animals/raven"`) – `None`: not there.
    pub fn animal(&self, name: &str) -> Option<&Sprite> {
        self.loaded.get(name)?.as_ref()
    }

    /// The picture of a house of doom of this size.
    pub fn doom(&self, nest: Nest, look: u32) -> Option<&BuildingSprite> {
        let index = match nest {
            Nest::Small => 0,
            Nest::Medium => 1,
            Nest::Big => 2,
        };
        self.pick_building(&self.doom, index, look)
    }

    fn pick_building(&self, lists: &[Vec<String>], index: usize, look: u32) -> Option<&BuildingSprite> {
        let names = lists.get(index)?;
        if names.is_empty() {
            return None;
        }
        self.buildings_loaded.get(&names[look as usize % names.len()])?.as_ref()
    }

    /// The skins of one kind that load – or its `sprite` if none does (`fallback`, `None`: there is none).
    fn looks(&mut self, skins: &[String], fallback: Option<&str>) -> Vec<String> {
        let found: Vec<String> = skins.iter().filter(|name| self.try_load(name, false)).cloned().collect();
        if !found.is_empty() {
            return found;
        }
        // `into_iter … collect`: an `Option` is a list of zero or one – this is the fallback alone, or nothing.
        fallback.filter(|name| self.try_load(name, true)).map(str::to_owned).into_iter().collect()
    }

    /// Loads the sprite of this name unless that was tried already. Did it load? `missing_counts`: a missing JSON
    /// file is a problem worth the red line too (the fallback), not just a skin made on another PC.
    fn try_load(&mut self, name: &str, missing_counts: bool) -> bool {
        if !self.loaded.contains_key(name) {
            let sprite = match Sprite::load(name) {
                Ok(Some(sprite)) => Some(sprite),
                Ok(None) => {
                    if missing_counts {
                        self.problem = Some(format!("assets\\{name}.json is missing"));
                    }
                    None
                }
                Err(e) => {
                    self.problem = Some(e);
                    None
                }
            };
            self.loaded.insert(name.to_string(), sprite);
        }
        self.loaded[name].is_some()
    }

    /// The picture of a unit of this type – the skin number `look` (counting round, so any number will do).
    /// `None`: it's drawn as a box.
    pub fn unit(&self, kind: UnitKind, look: u32) -> Option<&Sprite> {
        // `kind as usize`: its place in the enum – the same order as `UnitKind::ALL`.
        self.pick(&self.units, kind as usize, look)
    }

    /// The picture of a zombie of this breed, wearing skin number `look`.
    pub fn breed(&self, breed: Breed, look: u32) -> Option<&Sprite> {
        self.pick(&self.breeds, breed.index(), look)
    }

    fn pick(&self, lists: &[Vec<String>], kind: usize, look: u32) -> Option<&Sprite> {
        let names = lists.get(kind)?;
        if names.is_empty() {
            return None;
        }
        let name = &names[look as usize % names.len()];
        self.loaded.get(name)?.as_ref()
    }

    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }
}
