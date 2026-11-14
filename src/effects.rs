//! TAB's effects (DESIGN.md 12.3): blood when a zombie is hit, a flash at the muzzle, arrows flying, the blasts of
//! barrels and of falling buildings, stains on the ground – and where the guns of the ballista and the machine gun
//! point. Only drawing: the simulation says what happened (`sim::Fx`), the game hands it over after every tick
//! (`take`), this file shows it the way TAB does (`ZX\VisualEffects.cs`, `CShotEffect`, `CRotatingBuilding`).
//!
//! An effect is a JSON file and its PNG in `assets\effects\` (`tools\tab_effects.py`, explained in
//! `assets\README.md`): frames side by side, played once in `duration` seconds of game time. It's drawn
//! `size × width` tiles wide, its feet on the lowest corner of a `size` square around its spot – like TAB, which is
//! why a hit gets size 0.4 and a barrel's blast 5. Missing files: that effect is just not shown (TAB's pictures only
//! lie on the PC that converted them) – shots are then drawn as lines again (`draw.rs`).

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, TAU};

use macroquad::prelude::*;
use serde::Deserialize;

use crate::building_sprites::glow;
use crate::data;
use crate::iso::{Camera, TILE_W};
use crate::kinds::{BuildingKind, UnitKind};
use crate::sim::{Fallen, Fx, Nest, World, MUZZLE, TICK};
use crate::sprites::smooth;

mod lasting;

/// `assets\effects\<name>.json`.
#[derive(Deserialize)]
struct Sheet {
    image: String,
    /// Width and height of one frame (pixels), and how many frames lie side by side before the next row starts.
    frame: [f32; 2],
    columns: u32,
    frames: u32,
    /// Where in a frame the effect stands (pixels from its top left corner).
    feet: [f32; 2],
    /// This many pixels are one tile width on screen at zoom 1 – for an effect of size 1.
    pixels_per_tile: f32,
    /// Seconds of game time for all its frames; 0: a still picture (a stain, an arrow).
    duration: f32,
    /// Over and over instead of once.
    #[serde(default, rename = "loop")]
    looping: bool,
    /// From this share of its time on it fades out (TAB's `VanishFactorTime`).
    vanish: Option<f32>,
    /// Added to what's below – it glows (`building_sprites::glow`).
    #[serde(default)]
    add: bool,
    opacity: Option<f32>,
    /// Drawn turned the way it points: a muzzle flash (pointing right in its picture) or an arrow (pointing up).
    #[serde(default)]
    turns: bool,
    /// A muzzle flash: its height times this, how far in front of the shooter and how high (tile widths) it is.
    #[serde(default = "one")]
    squash: f32,
    #[serde(default)]
    ahead: f32,
    #[serde(default)]
    height: f32,
}

fn one() -> f32 {
    1.0
}

struct Effect {
    sheet: Sheet,
    texture: Texture2D,
}

impl Effect {
    fn load(name: &str) -> Result<Option<Effect>, String> {
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
        let rows = sheet.frames.div_ceil(sheet.columns.max(1));
        let fits = sheet.columns as f32 * sheet.frame[0] <= image.width as f32 && rows as f32 * sheet.frame[1] <= image.height as f32;
        if sheet.frames == 0 || sheet.columns == 0 || sheet.pixels_per_tile <= 0.0 || !fits {
            return Err(at(&"frames, columns or frame don't fit the picture"));
        }
        let texture = Texture2D::from_image(&image);
        smooth(&texture);
        Ok(Some(Effect { sheet, texture }))
    }

    /// Where frame `i` lies in the picture.
    fn source(&self, i: u32) -> Rect {
        let [w, h] = self.sheet.frame;
        Rect::new((i % self.sheet.columns) as f32 * w, (i / self.sheet.columns) as f32 * h, w, h)
    }

    /// The frame `age` seconds after it started, and how visible it is then – `None`: it's over.
    fn at_age(&self, age: f32) -> Option<(u32, f32)> {
        let s = &self.sheet;
        if s.duration <= 0.0 {
            return Some((0, 1.0));
        }
        let mut share = age / s.duration;
        if s.looping {
            share = share.fract();
        } else if share >= 1.0 {
            return None;
        }
        let frame = ((share * s.frames as f32) as u32).min(s.frames - 1);
        let fade = match s.vanish {
            Some(v) if share > v && v < 1.0 => (1.0 - share) / (1.0 - v),
            _ => 1.0,
        };
        Some((frame, fade))
    }
}

/// One effect playing: which, where its spot is (tiles), how far above the ground (tile widths), its size, since
/// when (seconds of game time), which way it points on screen (radians – only those that turn), its colour.
struct Playing {
    name: &'static str,
    at: Vec2,
    lift: f32,
    size: f32,
    start: f32,
    angle: f32,
    color: Color,
}

/// An arrow on its way: from, to, the second it was shot, how long it flies – and what it does when it lands.
struct Flight {
    name: &'static str,
    from: Vec2,
    to: Vec2,
    start: f32,
    time: f32,
    bleeds: bool,
    /// In an arc (the ranger's arrow) or straight (the ballista's bolt) – TAB's `FactorHeight` 0.1 or 0.
    arc: bool,
}

impl Flight {
    /// How far along its way at `now`: 0 = just shot, 1 = landed.
    fn share(&self, now: f32) -> f32 {
        ((now - self.start) / self.time.max(0.01)).clamp(0.0, 1.0)
    }

    /// How high above the ground it is (tile widths) that far along – TAB's `Proyectile.OnUpdate`: from chest height
    /// to the target's chest, an arrow in an arc above that line (highest in the middle, the longer the way the higher),
    /// a bolt straight down from a bit higher up.
    fn height(&self, t: f32) -> f32 {
        if self.arc {
            CHEST + 4.0 * t * (1.0 - t) * self.from.distance(self.to) * ARC
        } else {
            CHEST + BOLT_LIFT * (1.0 - t)
        }
    }
}

/// A ray from a point to another (TAB's `ShowRay`): the effect's frames stretched along the way.
struct Ray {
    name: &'static str,
    from: Vec2,
    to: Vec2,
    start: f32,
    /// The building it comes from – gone with it.
    building: u32,
}

/// A ray is this much longer than the way (TAB's `lengthFactor`) and starts this high (tile widths – TAB's 1.5 scene
/// units).
const RAY_LENGTH: f32 = 1.3;
const RAY_HEIGHT: f32 = 1.5 * std::f32::consts::FRAC_1_SQRT_2;

/// A stain lying on the ground (TAB's `DieUnit`): it grows for a second, lies there, fades out over ten.
struct Stain {
    name: &'static str,
    at: Vec2,
    size: f32,
    turn: f32,
    start: f32,
    color: Color,
}

/// Where a turning gun points, where it wants to, and when it last shot.
#[derive(Clone, Copy)]
struct Gun {
    degrees: f32,
    target: f32,
    shot: f32,
}

/// TAB's guns turn 8 degrees per update, 30 updates a second (`CRotatingBuilding.OnUpdate`).
const GUN_TURN: f32 = 240.0;
/// A stain grows this long, then fades out over this long (seconds) – TAB's `DieUnit`.
const STAIN_GROWS: f32 = 1.0;
const STAIN_FADES: f32 = 10.0;
/// How far above the ground a hit bleeds and a shot leaves (tile widths) – about a figure's chest.
const CHEST: f32 = 0.3;
/// TAB's ballista bolt flies this fast (tiles per second, `ProyectileSpeed`). The ranger's arrow: `sim/arrows.rs`.
const BOLT_SPEED: f32 = 16.0;
/// How high an arrow's arc goes in its middle, per tile of its way (tile widths): TAB's `FactorHeight` 0.1 – its peak
/// is 0.1 × the way in TAB's scene units, and a tile is √2 of those wide.
const ARC: f32 = 0.1 / std::f32::consts::SQRT_2;
/// The ballista's bolt flies this much higher than an arrow starts (tile widths) – TAB's 0.8 scene units.
const BOLT_LIFT: f32 = 0.8 / std::f32::consts::SQRT_2;
/// An arrow's shadow on the ground: black, half see-through – TAB's `_OShadow`.
const SHADOW: Color = Color::new(0.0, 0.0, 0.0, 0.5);
/// A zombie's blood is a bit darker (TAB: brightness 0.8, its stain dark red).
const ZOMBIE_BLOOD: Color = Color::new(0.8, 0.8, 0.8, 1.0);
const ZOMBIE_STAIN: Color = Color::new(0.55, 0.08, 0.06, 0.8);
const HUMAN_STAIN: Color = Color::new(0.75, 0.75, 0.75, 0.8);
const VENOM_POOL: Color = Color::new(1.0, 1.0, 1.0, 0.5);

/// All effects – one per game, thrown away on "Restart" like the sprites.
#[derive(Default)]
pub struct Effects {
    /// Loaded by name (`"effects/blood_splash"`); `None`: tried, not there or broken.
    sheets: HashMap<&'static str, Option<Effect>>,
    playing: Vec<Playing>,
    flights: Vec<Flight>,
    stains: Vec<Stain>,
    rays: Vec<Ray>,
    /// The guns of the ballistas and machine guns, by building `id`.
    guns: HashMap<u32, Gun>,
    /// For the little randomness of the look (where a hit bleeds) – not the simulation's numbers.
    seed: u32,
    /// Every other big building falling shows wood, the others stone (TAB's `WoodAndStoneBig`).
    falls: u32,
    /// How strong the snow and the rain are drawn (0..1, `lasting.rs`) – they fade towards `World::weather`. `None`:
    /// not yet looked – the first frame takes the round's weather at once.
    weather: Option<[f32; 2]>,
    /// The smoke out of the locomotives' chimneys (TAB's `Locomotive.AddParticleSmoke`).
    puffs: Vec<Puff>,
    problem: Option<String>,
}

/// A puff of a locomotive's smoke: one of TAB's nine `SmokeParticle` pictures, where it left the chimney and when,
/// how long it lives, where it drifts (screen, tile widths over its life), how far it turns (radians) and how big it
/// starts (tile widths).
struct Puff {
    name: &'static str,
    at: Vec2,
    start: f32,
    life: f32,
    drift: Vec2,
    turn: f32,
    size: f32,
}

/// TAB's chimney: the locomotive picture's middle (its `ImageOffset` and `Height`, `tools\tab_trains.py`) and 1.1
/// scene units above it – in tiles on the map, and in tile widths up.
const CHIMNEY_OFFSET: Vec2 = vec2(-0.45, -0.47);
const CHIMNEY_LIFT: f32 = 0.163 + 1.1 * std::f32::consts::FRAC_1_SQRT_2;
/// A puff: 0.5 scene units big, growing from 1.5 to 16 times that over its 4–5 s, fading in over 0.1 s.
const PUFF_SIZE: f32 = 0.5 * std::f32::consts::FRAC_1_SQRT_2;
const PUFF_GROWS: (f32, f32) = (1.5, 16.0);
const PUFF_IN: f32 = 0.1;
/// At most this opaque – 20 a second lie over each other at first.
const PUFF_OPACITY: f32 = 0.45;
const PUFFS: [&str; 9] = [
    "effects/smoke_puff_1",
    "effects/smoke_puff_2",
    "effects/smoke_puff_3",
    "effects/smoke_puff_4",
    "effects/smoke_puff_5",
    "effects/smoke_puff_6",
    "effects/smoke_puff_7",
    "effects/smoke_puff_8",
    "effects/smoke_puff_9",
];

impl Effects {
    /// After every tick: what the simulation says happened (`World::fx`) starts playing.
    pub fn take(&mut self, world: &World) {
        let now = world.tick as f32 * TICK;
        for fx in &world.fx {
            match *fx {
                Fx::UnitShot { from, to, kind, bleeds, flight } => self.unit_shot(now, from, to, kind, bleeds, flight),
                Fx::TowerShot { building, kind, from, to } => self.tower_shot(now, building, kind, from, to),
                Fx::ZombieDied { at, .. } => {
                    self.on_body("effects/blood_zombie", now, at, 0.4, ZOMBIE_BLOOD);
                    self.stain("effects/blood_stain", now, at, 0.4, ZOMBIE_STAIN);
                }
                Fx::UnitDied { at, .. } => {
                    self.on_body("effects/blood_human", now, at, 0.4, WHITE);
                    self.stain("effects/blood_stain", now, at, 0.4, HUMAN_STAIN);
                }
                Fx::Acid { at } => {
                    self.play("effects/venom", now, at, 0.0, 0.6, 0.0, WHITE);
                    self.stain("effects/venom_pool", now, at, 0.4, VENOM_POOL);
                }
                // TAB's big fire blast for a barrel or a mine, a small one for a shell or a missile – as wide as it hits.
                Fx::Blast { at, radius, .. } => {
                    let name = if radius >= 1.5 { "effects/fire_blast" } else { "effects/small_fire" };
                    self.play(name, now, at, 0.0, 2.0 * radius, 0.0, WHITE);
                }
                Fx::Flame { from, to } => self.flame(now, from, to),
                Fx::Shock { building, from, to } => self.shock(now, building, from, to),
                Fx::Fell { what, tile, size } => self.fell(now, what, tile, size),
                Fx::Infected { tile, size, .. } => {
                    let middle = vec2(tile.0 as f32 + size.0 as f32 / 2.0, tile.1 as f32 + size.1 as f32 / 2.0);
                    let s = 1.0 + (size.0.max(size.1) - 1) as f32 * 0.2;
                    self.play("effects/crystal", now, middle, 0.0, s, 0.0, WHITE);
                }
                // Only heard, not seen (`audio.rs`).
                Fx::ZombieStruck { .. }
                | Fx::Placed { .. }
                | Fx::Built { .. }
                | Fx::Trained { .. }
                | Fx::Veteran
                | Fx::Picked { .. }
                | Fx::Deserted { .. }
                | Fx::TrainStart { .. } => {}
            }
        }
        // Every locomotive that moved lets out a puff (TAB: every update its position changed).
        for train in world.trains.iter().filter(|t| t.offset != t.prev_offset) {
            let at = train.line.point_at(train.offset) + CHIMNEY_OFFSET + self.spread(0.07);
            let name = PUFFS[(self.random() * PUFFS.len() as f32) as usize % PUFFS.len()];
            self.load(name);
            let life = 4.0 + self.random();
            // Up 5 scene units, give or take 2 each way.
            let drift = (vec2(0.0, -5.0) + vec2(self.random() * 4.0 - 2.0, self.random() * 4.0 - 2.0)) * std::f32::consts::FRAC_1_SQRT_2;
            let turn = (180.0 + 90.0 * self.random()).to_radians();
            let size = PUFF_SIZE * (0.8 + 0.4 * self.random());
            self.puffs.push(Puff { name, at, start: now, life, drift, turn, size });
        }
    }

    /// A unit shot: the soldier's and sniper's flash at the muzzle, an arrow on its way – and whom it hits bleeds (an
    /// arrow's when it gets there). `flight`: the arrow's ticks in the simulation (`sim/arrows.rs`), so it lands in the
    /// picture just when it hits; 0: a bullet.
    fn unit_shot(&mut self, now: f32, from: Vec2, to: Vec2, kind: UnitKind, bleeds: bool, flight: u32) {
        let angle = (to - from).to_angle();
        if flight > 0 {
            // It leaves a bit in front of the shooter, like TAB's. Thanatos's missile flies straight, with a flash at
            // the launcher – it blows up where it lands (`Fx::Blast`).
            let from = from + (to - from).normalize_or_zero() * MUZZLE;
            let time = flight as f32 * TICK;
            if kind == UnitKind::Thanatos {
                self.flash("effects/flash_thanatos", now, from, angle);
                self.fly(Flight { name: "projectiles/missile", from, to, start: now, time, bleeds: false, arc: false });
                return;
            }
            self.fly(Flight { name: "projectiles/arrow", from, to, start: now, time, bleeds, arc: true });
            return;
        }
        match kind {
            UnitKind::Soldier | UnitKind::MercSoldier => self.flash("effects/flash_soldier", now, from, angle),
            UnitKind::Sniper | UnitKind::MercSniper => self.flash("effects/flash_sniper", now, from, angle),
            UnitKind::Titan => self.flash("effects/flash_thanatos", now, from, angle),
            _ => {}
        }
        if bleeds {
            self.on_body("effects/blood_splash", now, to, 0.4, ZOMBIE_BLOOD);
        }
    }

    /// A tower shot: its gun turns there – the ballista plays its shot and lets a bolt fly, the machine gun flashes and
    /// its bullets spark where they hit.
    fn tower_shot(&mut self, now: f32, building: u32, kind: BuildingKind, from: Vec2, to: Vec2) {
        let target = (to - from).to_angle().to_degrees().rem_euclid(360.0);
        let gun = self.guns.entry(building).or_insert(Gun { degrees: target, target, shot: f32::MIN });
        gun.target = target;
        gun.shot = now;
        match kind {
            BuildingKind::Ballista => {
                let time = from.distance(to) / BOLT_SPEED;
                self.fly(Flight { name: "projectiles/big_arrow", from, to, start: now, time, bleeds: true, arc: false });
            }
            _ => {
                self.flash("effects/flash_gun", now, from, (to - from).to_angle());
                let name = if self.random() < 0.5 { "effects/gun_impact_a" } else { "effects/gun_impact_b" };
                let spot = to + self.spread(0.15);
                let lift = self.random() * CHEST;
                self.play(name, now, spot, lift, 0.5, 0.0, WHITE);
                self.on_body("effects/blood_splash", now, to, 0.4, ZOMBIE_BLOOD);
            }
        }
    }

    /// A building falls (TAB's `ShowExplosionOnEntity`): the big blasts once for every inner corner of a building wider
    /// than a tile, each about 2 tiles; otherwise one of size 1 in its middle.
    fn fell(&mut self, now: f32, what: Fallen, tile: (i32, i32), size: (i32, i32)) {
        let names: &[String] = match what {
            Fallen::Building(kind) => &kind.stats().explosion,
            Fallen::Doom(nest) => match &data::get().doom.explosion {
                Some(e) => match nest {
                    Nest::Small => &e.small,
                    Nest::Medium => &e.medium,
                    Nest::Big => &e.big,
                },
                None => &[],
            },
        };
        if names.is_empty() {
            return;
        }
        let corner = vec2(tile.0 as f32, tile.1 as f32);
        let big = names.iter().any(|n| n.ends_with("_big"));
        let mut spots = Vec::new();
        if big && size.0 > 1 {
            for i in 1..size.0 {
                for j in 1..size.1.max(2) {
                    spots.push((corner + vec2(i as f32, j as f32), 2.0));
                }
            }
        } else {
            spots.push((corner + vec2(size.0 as f32, size.1 as f32) / 2.0, 1.0));
        }
        for (spot, s) in spots {
            self.falls = self.falls.wrapping_add(1);
            let name = leak(&format!("effects/{}", names[self.falls as usize % names.len()]));
            let s = s * (0.85 + 0.3 * self.random());
            self.play(name, now, spot, 0.0, s, 0.0, WHITE);
        }
    }

    /// Lucifer's flame: a burst at the nozzle (TAB's `FireBurst_A`), fire all along the cone to its end – bigger
    /// further out, the cone widens.
    fn flame(&mut self, now: f32, from: Vec2, to: Vec2) {
        let way = to - from;
        self.flash("effects/fire_burst", now, from, way.to_angle());
        for k in 1..=4 {
            let share = k as f32 / 4.0;
            let spot = from + way * share + self.spread(0.1 * share);
            self.play("effects/small_fire", now, spot, CHEST * (1.0 - share * 0.5), 0.3 + 0.5 * share, 0.0, WHITE);
        }
    }

    /// The shocking tower's ray to one zombie (TAB's `ShowRay`: `ShockRay_A` from its top to the zombie, 1.3 times as
    /// long, 1.5 high) – and the zombie twitches in sparks.
    fn shock(&mut self, now: f32, building: u32, from: Vec2, to: Vec2) {
        if matches!(self.load("effects/shock_ray"), Some(Some(_))) {
            self.rays.push(Ray { name: "effects/shock_ray", from, to, start: now, building });
        }
        self.on_body("effects/gun_impact_a", now, to, 0.5, WHITE);
    }

    /// TAB's `ShowOnUnit`: somewhere on the body, up to chest high.
    fn on_body(&mut self, name: &'static str, now: f32, at: Vec2, size: f32, color: Color) {
        let spot = at + self.spread(0.12);
        let lift = self.random() * CHEST;
        self.play(name, now, spot, lift, size, 0.0, color);
    }

    /// A muzzle flash: in front of the shooter, at its gun's height, pointing where it shot.
    fn flash(&mut self, name: &'static str, now: f32, from: Vec2, world_angle: f32) {
        let Some(Some(effect)) = self.load(name) else {
            return;
        };
        let (ahead, height) = (effect.sheet.ahead, effect.sheet.height);
        let at = from + Vec2::from_angle(world_angle) * ahead;
        // On screen it points where the world direction points – the angle there is a different one.
        let dir = Vec2::from_angle(world_angle);
        let screen = crate::iso::iso(dir);
        self.play(name, now, at, height * 0.5, 1.0, screen.y.atan2(screen.x), WHITE);
    }

    /// An arrow or bolt sets off. Its picture is loaded first – `draw` only looks it up, it doesn't load. Without a
    /// picture it still flies (unseen): the zombie bleeds when it lands.
    fn fly(&mut self, flight: Flight) {
        self.load(flight.name);
        self.flights.push(flight);
    }

    #[allow(clippy::too_many_arguments)]
    fn play(&mut self, name: &'static str, now: f32, at: Vec2, lift: f32, size: f32, angle: f32, color: Color) {
        if matches!(self.load(name), Some(Some(_))) {
            self.playing.push(Playing { name, at, lift, size, start: now, angle, color });
        }
    }

    fn stain(&mut self, name: &'static str, now: f32, at: Vec2, width: f32, color: Color) {
        if matches!(self.load(name), Some(Some(_))) {
            let size = width * (2.2 + 0.6 * self.random());
            let turn = self.random() * TAU;
            self.stains.push(Stain { name, at, size, turn, start: now, color });
        }
    }

    /// The effect of this name, loaded the first time it's asked for. `Some(None)`: not there.
    fn load(&mut self, name: &'static str) -> Option<&Option<Effect>> {
        if !self.sheets.contains_key(name) {
            let effect = Effect::load(name).unwrap_or_else(|e| {
                self.problem = Some(e);
                None
            });
            self.sheets.insert(name, effect);
        }
        self.sheets.get(name)
    }

    /// Does it have the muzzle flashes – otherwise shots are still drawn as lines.
    pub fn shows_shots(&self) -> bool {
        matches!(self.sheets.get("effects/flash_soldier"), Some(Some(_)))
    }

    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    /// Every frame: guns turn towards their target, what's over is forgotten. `now`: seconds of game time.
    pub fn update(&mut self, world: &World, now: f32, dt: f32) {
        self.load("effects/flash_soldier"); // for `shows_shots`
        for name in lasting::NAMES {
            self.load(name);
        }
        for gun in self.guns.values_mut() {
            let diff = (gun.target - gun.degrees + 180.0).rem_euclid(360.0) - 180.0;
            gun.degrees = (gun.degrees + diff.clamp(-GUN_TURN * dt, GUN_TURN * dt)).rem_euclid(360.0);
        }
        // Torn down, fallen: its gun goes with it.
        self.guns.retain(|id, _| world.buildings.iter().any(|b| b.id == *id));
        self.fade_weather(world.weather, dt);
        let sheets = &self.sheets;
        let over = |name: &str, start: f32| match sheets.get(name) {
            Some(Some(e)) => e.at_age(now - start).is_none(),
            _ => true,
        };
        self.playing.retain(|p| !over(p.name, p.start));
        self.rays.retain(|r| !over(r.name, r.start) && world.buildings.iter().any(|b| b.id == r.building));
        self.flights.retain(|f| now < f.start + f.time + 0.05);
        self.stains.retain(|s| now < s.start + STAIN_GROWS + STAIN_FADES);
        self.puffs.retain(|p| now < p.start + p.life);
        // Arrows that arrive: the zombie bleeds.
        let landed: Vec<(Vec2, f32)> = self
            .flights
            .iter()
            .filter(|f| f.bleeds && now >= f.start + f.time && now - dt < f.start + f.time)
            .map(|f| (f.to, f.start + f.time))
            .collect();
        for (at, when) in landed {
            self.on_body("effects/blood_splash", when, at, 0.4, ZOMBIE_BLOOD);
        }
    }

    /// Where the gun of building `id` points (degrees, like `Unit::facing`), and when it last shot – `None`: it hasn't
    /// shot yet.
    pub fn gun(&self, id: u32) -> Option<(f32, f32)> {
        self.guns.get(&id).map(|g| (g.degrees, g.shot))
    }

    /// The stains – on the ground, before the figures. `shows`: is that spot seen.
    pub fn draw_stains(&self, camera: &Camera, now: f32, shows: impl Fn(Vec2) -> bool) {
        // The shadows of arrows and bolts in flight: straight along the ground below them – that's what makes the arc
        // visible (TAB's `_OShadow`).
        for flight in self.flights.iter().filter(|f| shows(f.from) || shows(f.to)) {
            let Some(Some(effect)) = self.sheets.get(flight.name) else {
                continue;
            };
            let here = flight.from.lerp(flight.to, flight.share(now));
            let way = camera.world_to_screen(flight.to) - camera.world_to_screen(flight.from);
            // The arrow points up in its picture.
            let angle = way.y.atan2(way.x) + FRAC_PI_2;
            effect.draw_at(camera, camera.world_to_screen(here), 1.0, angle, SHADOW, true);
        }
        for stain in self.stains.iter().filter(|s| shows(s.at)) {
            let Some(Some(effect)) = self.sheets.get(stain.name) else {
                continue;
            };
            let age = now - stain.start;
            let grown = (age / STAIN_GROWS).clamp(0.1, 1.0);
            let fade = 1.0 - ((age - STAIN_GROWS) / STAIN_FADES).clamp(0.0, 1.0);
            let half = stain.size * grown / 2.0;
            // Its four corners, turned on the ground, then onto the screen: it lies flat like TAB's.
            let (s, c) = stain.turn.sin_cos();
            let corner = |x: f32, y: f32| camera.world_to_screen(stain.at + vec2(x * c - y * s, x * s + y * c) * half);
            let color = Color { a: stain.color.a * fade, ..stain.color };
            let r = effect.source(0);
            let (tw, th) = (effect.texture.width(), effect.texture.height());
            let uv = |u: f32, v: f32| vec2((r.x + u * r.w) / tw, (r.y + v * r.h) / th);
            let points = [(corner(-1.0, -1.0), uv(0.0, 0.0)), (corner(1.0, -1.0), uv(1.0, 0.0)), (corner(1.0, 1.0), uv(1.0, 1.0)), (corner(-1.0, 1.0), uv(0.0, 1.0))];
            let vertices = points.iter().map(|(p, t)| Vertex::new(p.x, p.y, 0.0, t.x, t.y, color)).collect();
            draw_mesh(&Mesh { vertices, indices: vec![0, 1, 2, 0, 2, 3], texture: Some(effect.texture.clone()) });
        }
    }

    /// Everything else – over the figures and buildings, like TAB's effects.
    pub fn draw(&self, camera: &Camera, now: f32, shows: impl Fn(Vec2) -> bool) {
        // The locomotives' smoke: rising, growing, turning, fading in fast and out slowly, a bit greyer as it goes.
        for puff in self.puffs.iter().filter(|p| shows(p.at)) {
            let Some(Some(effect)) = self.sheets.get(puff.name) else {
                continue;
            };
            let t = ((now - puff.start) / puff.life).clamp(0.0, 1.0);
            let age = now - puff.start;
            let fade = if age < PUFF_IN { age / PUFF_IN } else { (1.0 - t) * (1.0 - t) };
            let grey = 1.0 - 0.15 * t;
            let size = puff.size * (PUFF_GROWS.0 + (PUFF_GROWS.1 - PUFF_GROWS.0) * t);
            let unit = TILE_W * camera.zoom;
            // Fast away from the chimney, then slower (TAB's transition with an easing of its own) – so the puffs
            // don't pile up into one bright ball.
            let away = 1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t);
            let at = camera.world_to_screen(puff.at) + (vec2(0.0, -CHIMNEY_LIFT) + puff.drift * away) * unit;
            effect.draw_at(camera, at, size, puff.turn * t, Color::new(grey, grey, grey, fade * PUFF_OPACITY), true);
        }
        for flight in self.flights.iter().filter(|f| shows(f.from) || shows(f.to)) {
            let Some(Some(effect)) = self.sheets.get(flight.name) else {
                continue;
            };
            let t = flight.share(now);
            let height = |t: f32| flight.height(t);
            let point = |t: f32| camera.world_to_screen(flight.from.lerp(flight.to, t)) - vec2(0.0, height(t) * TILE_W * camera.zoom);
            let (here, next) = (point(t), point((t + 0.02).min(1.0)) - point((t - 0.02).max(0.0)));
            // The arrow points up in its picture.
            let angle = next.y.atan2(next.x) + FRAC_PI_2;
            effect.draw_at(camera, here, 1.0, angle, WHITE, true);
        }
        for ray in self.rays.iter().filter(|r| shows(r.from) || shows(r.to)) {
            let Some(Some(effect)) = self.sheets.get(ray.name) else {
                continue;
            };
            let Some((frame, fade)) = effect.at_age(now - ray.start) else {
                continue;
            };
            let lift = vec2(0.0, RAY_HEIGHT * TILE_W * camera.zoom);
            let (a, b) = (camera.world_to_screen(ray.from) - lift, camera.world_to_screen(ray.to) - lift * 0.3);
            let way = (b - a) * RAY_LENGTH;
            // As long as the way, one scene unit thick, its middle half way, turned along it.
            let size = vec2(way.length(), TILE_W * std::f32::consts::FRAC_1_SQRT_2 * camera.zoom);
            let middle = a + way / 2.0;
            let params = DrawTextureParams {
                dest_size: Some(size),
                source: Some(effect.source(frame)),
                rotation: way.y.atan2(way.x),
                pivot: Some(middle),
                ..Default::default()
            };
            let color = Color { a: fade, ..WHITE };
            let corner = middle - size / 2.0;
            glow(|| draw_texture_ex(&effect.texture, corner.x, corner.y, color, params.clone()));
        }
        for p in self.playing.iter().filter(|p| shows(p.at)) {
            let Some(Some(effect)) = self.sheets.get(p.name) else {
                continue;
            };
            let Some((frame, fade)) = effect.at_age(now - p.start) else {
                continue;
            };
            let color = Color { a: p.color.a * fade * effect.sheet.opacity.unwrap_or(1.0), ..p.color };
            if effect.sheet.turns {
                let at = camera.world_to_screen(p.at) - vec2(0.0, p.lift * TILE_W * camera.zoom);
                effect.draw_frame(camera, frame, at, p.size, p.angle, color);
                continue;
            }
            // TAB: the bottom middle on the lowest corner of the square of `size` around the spot.
            let feet = camera.world_to_screen(p.at + Vec2::splat(p.size / 2.0)) - vec2(0.0, p.lift * TILE_W * camera.zoom);
            effect.draw_frame(camera, frame, feet, p.size, 0.0, color);
        }
    }
}

impl Effect {
    /// Frame `frame` with its feet on `feet` (screen pixels), for size `size`, turned by `angle` (radians) round its
    /// feet. A glowing one is added to the screen.
    fn draw_frame(&self, camera: &Camera, frame: u32, feet: Vec2, size: f32, angle: f32, color: Color) {
        let s = &self.sheet;
        let scale = TILE_W / s.pixels_per_tile * camera.zoom * size;
        let dest = vec2(s.frame[0], s.frame[1] * s.squash) * scale;
        let corner = feet - vec2(s.feet[0], s.feet[1] * s.squash) * scale;
        let margin = dest.x.max(dest.y);
        if feet.x < -margin || feet.y < -margin || feet.x > screen_width() + margin || feet.y > screen_height() + margin {
            return;
        }
        let params = DrawTextureParams {
            dest_size: Some(dest),
            source: Some(self.source(frame)),
            rotation: angle,
            pivot: Some(feet),
            ..Default::default()
        };
        let draw = || draw_texture_ex(&self.texture, corner.x, corner.y, color, params.clone());
        if s.add {
            glow(draw);
        } else {
            draw();
        }
    }

    /// A still picture (an arrow) with its middle on `at`, turned by `angle`.
    fn draw_at(&self, camera: &Camera, at: Vec2, size: f32, angle: f32, color: Color, middle: bool) {
        let s = &self.sheet;
        let scale = TILE_W / s.pixels_per_tile * camera.zoom * size;
        let dest = vec2(s.frame[0], s.frame[1]) * scale;
        let corner = if middle { at - dest / 2.0 } else { at };
        let params = DrawTextureParams { dest_size: Some(dest), source: Some(self.source(0)), rotation: angle, ..Default::default() };
        draw_texture_ex(&self.texture, corner.x, corner.y, color, params);
    }
}

impl Effects {
    /// A random number 0..1 (xorshift) – only for the look.
    fn random(&mut self) -> f32 {
        let mut x = self.seed.wrapping_add(0x9E37_79B9);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / (1 << 24) as f32
    }

    /// A random step of at most `radius` tiles in x and y.
    fn spread(&mut self, radius: f32) -> Vec2 {
        vec2(self.random() * 2.0 - 1.0, self.random() * 2.0 - 1.0) * radius
    }
}

/// The names in `data\` live as long as the game – but they're `String`s there. Each name made `'static` once and kept
/// (a handful of them, so the leak is a few bytes) – then `Playing` needs no `String` of its own.
fn leak(name: &str) -> &'static str {
    thread_local! {
        static NAMES: std::cell::RefCell<Vec<&'static str>> = const { std::cell::RefCell::new(Vec::new()) };
    }
    NAMES.with(|names| {
        let mut names = names.borrow_mut();
        if let Some(n) = names.iter().find(|n| **n == name) {
            return *n;
        }
        let n: &'static str = Box::leak(name.to_string().into_boxed_str());
        names.push(n);
        n
    })
}
