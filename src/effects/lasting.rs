//! Effects that last as long as what causes them – drawn from the world as it is in every frame, not from something
//! that happened in a tick (`sim::Fx`):
//! - fire, smoke and the fire's glow on badly damaged buildings (ours – TAB's buildings only burn from fire damage),
//! - green smoke over infected ones (TAB's `CInfected`: `Infection_A` as big as the building),
//! - the weather of a landscape (TAB's `ZXMapThemeAL` / `ZXMapThemeVO`: snow on the Alpine, rain on the Volcanic
//!   landscape), leaves falling from trees (`ZXMapThemeFA.AddLeave`) and the shimmer on water,
//! - the range circle of a tower (`CVisualAttackRange`).
//!
//! Like the rest of `effects.rs` only the look: the little randomness comes from a building's `id` or a tile's place,
//! never from the simulation's numbers.

use super::*;
use crate::map::{to_frame, Region, Terrain};

/// Below this share of its hit points a building burns: one fire …
const BURNS: f32 = 0.5;
/// … below this one up to three, bigger.
const BLAZES: f32 = 0.25;
/// A fire this big (`size` of the effect – TAB's burning things: 1.3 × their width, for a figure 0.4).
const FIRE_SIZE: f32 = 0.7;
/// The orange glow round a fire (TAB's `Fire._OBright`: `LightWhite_A`, orange, added, 20 % opaque).
const FIRE_GLOW: Color = Color::new(1.0, 0.65, 0.0, 0.2);
/// TAB's weather (`EntityTemplates.Water.Snow_B` / `Rain_A`): turned this much on screen (degrees, TAB's `Angle`).
const SNOW_ANGLE: f32 = 320.0;
const RAIN_ANGLE: f32 = 15.0;
/// Every this many forest tiles of a hundred drop leaves (TAB: 10 %) …
const LEAF_TREES: u32 = 10;
/// … each falls 4 × 1 s (4 bends), waits 1–8 s, then falls again; it fades in and out over 2 s.
const LEAF_FALL: f32 = 4.0;
const LEAF_FADE: f32 = 2.0;
/// TAB's scene unit, in tile widths: a tile is √2 of them wide (see `ARC`).
const SCENE: f32 = std::f32::consts::FRAC_1_SQRT_2;
/// How strong the shimmer on water is – TAB's water pictures, added.
const WATER_SHINE: f32 = 0.12;
/// How far the water's shimmer picture reaches: this many tiles (TAB's `Water_A`: an entity 5 tiles wide).
const WATER_TILES: f32 = 5.0;
/// The range circle turns this many degrees per second (TAB: 90 in 20 s).
const RANGE_TURN: f32 = 90.0 / 20.0;
/// At most this many quads in one mesh – macroquad draws at most 5000 indices at once (6 per quad).
const QUADS_PER_MESH: usize = 800;

/// The effects this file draws – loaded in `Effects::update`, since drawing only looks them up.
pub(super) const NAMES: [&str; 9] = [
    "effects/fire",
    "effects/smoke_b",
    "effects/light_white_a",
    "effects/infection",
    "effects/snow_b",
    "effects/rain_a",
    "effects/leaf",
    "effects/water_a",
    "effects/range_circle",
];

/// A number 0..1 out of two others – the same every frame (for where a fire sits, which tree drops leaves).
fn hash(a: u32, b: u32) -> f32 {
    let mut x = a.wrapping_mul(0x9E37_79B9) ^ b.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x >> 8) as f32 / (1 << 24) as f32
}

impl Effects {
    fn sheet(&self, name: &str) -> Option<&Effect> {
        self.sheets.get(name).and_then(Option::as_ref)
    }

    /// One looping effect standing at `at` (tiles) – its feet like TAB's (`Playing`), `lift` tile widths above the
    /// ground. `phase`: so two of them don't flicker in step.
    #[allow(clippy::too_many_arguments)]
    fn draw_looping(&self, camera: &Camera, name: &str, at: Vec2, lift: f32, size: f32, now: f32, phase: f32, color: Color) {
        let Some(effect) = self.sheet(name) else {
            return;
        };
        let Some((frame, _)) = effect.at_age(now + phase * effect.sheet.duration) else {
            return;
        };
        let color = Color { a: color.a * effect.sheet.opacity.unwrap_or(1.0), ..color };
        let feet = camera.world_to_screen(at + Vec2::splat(size / 2.0)) - vec2(0.0, lift * TILE_W * camera.zoom);
        effect.draw_frame(camera, frame, feet, size, 0.0, color);
    }

    /// Over the buildings: fire and smoke where one is badly damaged, green smoke where one is infected. `shows`: is
    /// that spot seen.
    pub fn draw_buildings(&self, world: &World, camera: &Camera, now: f32, shows: impl Fn(Vec2) -> bool) {
        for building in world.buildings.iter().filter(|b| shows(b.center())) {
            let (w, h) = building.footprint();
            let stats = building.stats();
            if building.infected {
                let size = w.max(h) as f32;
                let phase = hash(building.id, 1);
                self.draw_looping(camera, "effects/infection", building.center(), 0.0, size, now, phase, WHITE);
                continue;
            }
            let share = building.hp as f32 / stats.hp.max(1) as f32;
            if !building.is_built() || share >= BURNS || stats.trap_damage > 0 {
                continue;
            }
            let (fires, size) = if share < BLAZES { ((w * h).min(3) as u32, FIRE_SIZE * 1.3) } else { (1, FIRE_SIZE) };
            for k in 0..fires {
                // Somewhere on its site, not right at the edge – the same spot every frame.
                let spot = |axis: u32, tiles: i32| 0.25 + hash(building.id, k * 2 + axis) * (tiles as f32 - 0.5);
                let at = building.corner() + vec2(spot(0, w), spot(1, h));
                let lift = stats.height * 0.5;
                let phase = hash(building.id, 10 + k);
                // The glow: the light's middle on the fire's foot.
                if let Some(light) = self.sheet("effects/light_white_a") {
                    let feet = camera.world_to_screen(at) - vec2(0.0, lift * TILE_W * camera.zoom);
                    light.draw_frame(camera, 0, feet, size * 2.0, 0.0, FIRE_GLOW);
                }
                self.draw_looping(camera, "effects/fire", at, lift, size, now, phase, WHITE);
                self.draw_looping(camera, "effects/smoke_b", at, lift + size * 0.6, size, now, phase, WHITE);
            }
        }
    }

    /// Zombies on fire (`sim/fire.rs`): TAB's fire on them while they burn – only where you see them.
    pub fn draw_burning(&self, world: &World, camera: &Camera, now: f32, shows: impl Fn(Vec2) -> bool) {
        for zombie in world.zombies.iter().filter(|z| z.burning > 0 && shows(z.pos)) {
            let size = zombie.radius * 3.0;
            self.draw_looping(camera, "effects/fire", zombie.pos, 0.05, size, now, hash(zombie.id, 3), WHITE);
        }
    }

    /// The range circle of a tower: flat on the ground, `range` tiles round `at`, slowly turning (TAB's
    /// `CVisualAttackRange`). Without the picture: a thin ring.
    pub fn draw_range(&self, camera: &Camera, at: Vec2, range: f32, now: f32) {
        let Some(effect) = self.sheet("effects/range_circle") else {
            crate::draw::draw_ring(camera, at, range, 1.5, Color::new(0.4, 1.0, 0.4, 0.6));
            return;
        };
        let turn = (now * RANGE_TURN).to_radians();
        effect.draw_flat(camera, at, range, turn, WHITE);
    }

    /// The weather of a landscape (`look`, like `World::look`): its picture over and over across the screen, turned
    /// like TAB's, fixed to the map – it moves when you scroll. Only TAB's snowy and rainy landscapes have one.
    pub fn draw_weather(&self, look: &str, camera: &Camera, now: f32) {
        let (name, degrees) = match look {
            "AL" => ("effects/snow_b", SNOW_ANGLE),
            "VO" => ("effects/rain_a", RAIN_ANGLE),
            _ => return,
        };
        let Some(effect) = self.sheet(name) else {
            return;
        };
        let Some((frame, _)) = effect.at_age(now) else {
            return;
        };
        let s = &effect.sheet;
        let scale = TILE_W / s.pixels_per_tile * camera.zoom;
        let side = vec2(s.frame[0], s.frame[1]) * scale;
        if side.x < 4.0 || side.y < 4.0 {
            return;
        }
        // A grid of pictures, turned as a whole: picture (i, j) has its middle at `origin + turn(i × width, j × height)`
        // and is itself turned by the same angle – so they lie edge to edge.
        let angle = degrees.to_radians();
        let (sin, cos) = angle.sin_cos();
        let turn = |v: Vec2| vec2(v.x * cos - v.y * sin, v.x * sin + v.y * cos);
        let unturn = |v: Vec2| vec2(v.x * cos + v.y * sin, -v.x * sin + v.y * cos);
        let origin = camera.world_to_screen(Vec2::ZERO);
        // Which (i, j) reach the screen: its corners, turned back into the grid.
        let corners = [vec2(0.0, 0.0), vec2(screen_width(), 0.0), vec2(0.0, screen_height()), vec2(screen_width(), screen_height())];
        let grid: Vec<Vec2> = corners.iter().map(|&c| unturn(c - origin) / side).collect();
        let lo = grid.iter().fold(Vec2::splat(f32::MAX), |a, &b| a.min(b)).floor() - 1.0;
        let hi = grid.iter().fold(Vec2::splat(f32::MIN), |a, &b| a.max(b)).ceil() + 1.0;
        if (hi.x - lo.x) * (hi.y - lo.y) > 2000.0 {
            return;
        }
        let color = Color { a: s.opacity.unwrap_or(1.0), ..WHITE };
        let draw = || {
            for i in lo.x as i32..=hi.x as i32 {
                for j in lo.y as i32..=hi.y as i32 {
                    let middle = origin + turn(vec2(i as f32, j as f32) * side);
                    let params = DrawTextureParams {
                        dest_size: Some(side + 1.0),
                        source: Some(effect.source(frame)),
                        rotation: angle,
                        pivot: Some(middle),
                        ..Default::default()
                    };
                    let corner = middle - side / 2.0;
                    draw_texture_ex(&effect.texture, corner.x, corner.y, color, params);
                }
            }
        };
        if s.add { glow(draw) } else { draw() }
    }

    /// Leaves falling from the trees of the autumn landscape (`look` "FA"): one tree in ten drops one now and then –
    /// TAB's `ZXMapThemeFA.AddLeave`: from above the tree, four swings to the side and down, fading in and out.
    pub fn draw_leaves(&self, world: &World, area: Region, camera: &Camera, now: f32, shows: impl Fn(Vec2) -> bool) {
        if world.look != "FA" {
            return;
        }
        let Some(effect) = self.sheet("effects/leaf") else {
            return;
        };
        let area = screen_part(camera, area);
        let scene = TILE_W * SCENE * camera.zoom;
        // TAB: 0.8 scene units at most – the picture at size 1 is `frame / pixels_per_tile` tiles wide.
        let s = &effect.sheet;
        let size = 0.8 * SCENE / (s.frame[0].max(s.frame[1]) / s.pixels_per_tile);
        for (y, xs) in area.rows(1.0) {
            for x in xs {
                if world.map.terrain(x, y) != Terrain::Forest || hash(x as u32, y as u32) * 100.0 >= LEAF_TREES as f32 {
                    continue;
                }
                let middle = vec2(x as f32 + 0.5, y as f32 + 0.5);
                if !shows(middle) {
                    continue;
                }
                // Each leaf waits a while of its own between its falls.
                let wait = 1.0 + 7.0 * hash(x as u32, y as u32 ^ 0x55);
                let cycle = LEAF_FALL + wait;
                let t = (now + hash(y as u32, x as u32) * cycle).rem_euclid(cycle);
                if t >= LEAF_FALL {
                    continue;
                }
                // Four bends, each a second long: to one side and down, smoothly (TAB's curved trajectory).
                let mut at = camera.world_to_screen(middle) - vec2(0.0, 1.5 * scene);
                let fall = (x as u32).wrapping_mul(31).wrapping_add(y as u32);
                for bend in 0..4 {
                    let side = if hash(fall, bend) < 0.5 { -1.0 } else { 1.0 };
                    let step = vec2(side * (0.7 + 0.3 * hash(fall, bend + 8)), 0.6 + 0.2 * (hash(fall, bend + 16) - 0.5)) * scene;
                    let share = (t - bend as f32).clamp(0.0, 1.0);
                    at += step * (share * share * (3.0 - 2.0 * share));
                }
                let fade = (t / LEAF_FADE).min((LEAF_FALL - t) / LEAF_FADE).clamp(0.0, 1.0) * 0.8;
                let Some((frame, _)) = effect.at_age(now + hash(fall, 99) * s.duration) else {
                    continue;
                };
                let color = Color::new(0.9, 0.9, 0.9, fade);
                // `at` is the leaf's middle – `draw_frame` wants its feet.
                let feet = at + (Vec2::from(s.feet) - Vec2::from(s.frame) / 2.0) * TILE_W / s.pixels_per_tile * camera.zoom * size;
                effect.draw_frame(camera, frame, feet, size, 0.0, color);
            }
        }
    }

    /// The shimmer of light on water: TAB's moving water picture, added on every water tile – fixed to the map, so it
    /// runs on from tile to tile. Before the fog, which darkens it like the water below.
    pub fn draw_water(&self, world: &World, area: Region, camera: &Camera, now: f32) {
        let Some(effect) = self.sheet("effects/water_a") else {
            return;
        };
        let Some((frame, _)) = effect.at_age(now) else {
            return;
        };
        let area = screen_part(camera, area);
        let r = effect.source(frame);
        let (tw, th) = (effect.texture.width(), effect.texture.height());
        let color = Color { a: WATER_SHINE, ..WHITE };
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let flush = |vertices: &mut Vec<Vertex>, indices: &mut Vec<u16>| {
            if !vertices.is_empty() {
                let mesh = Mesh { vertices: std::mem::take(vertices), indices: std::mem::take(indices), texture: Some(effect.texture.clone()) };
                glow(|| draw_mesh(&mesh));
            }
        };
        for (y, xs) in area.rows(1.0) {
            for x in xs {
                if world.map.terrain(x, y) != Terrain::Water {
                    continue;
                }
                let corner = vec2(x as f32, y as f32);
                if !on_screen(camera.world_to_screen(corner + 0.5), TILE_W * camera.zoom) {
                    continue;
                }
                // Where in the picture this tile lies: the picture covers `WATER_TILES` × `WATER_TILES` tiles.
                let k = indices.len() / 6 * 4;
                for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                    let p = camera.world_to_screen(corner + vec2(dx, dy));
                    let u = ((x as f32 + dx) / WATER_TILES).rem_euclid(1.0);
                    let v = ((y as f32 + dy) / WATER_TILES).rem_euclid(1.0);
                    // The far edge of a picture's last tile is 1, not 0.
                    let u = if dx > 0.0 && u == 0.0 { 1.0 } else { u };
                    let v = if dy > 0.0 && v == 0.0 { 1.0 } else { v };
                    vertices.push(Vertex::new(p.x, p.y, 0.0, (r.x + u * r.w) / tw, (r.y + v * r.h) / th, color));
                }
                let k = k as u16;
                indices.extend_from_slice(&[k, k + 1, k + 2, k, k + 2, k + 3]);
                if indices.len() / 6 >= QUADS_PER_MESH {
                    flush(&mut vertices, &mut indices);
                }
            }
        }
        flush(&mut vertices, &mut indices);
    }
}

impl Effect {
    /// A picture lying flat on the ground (a range circle): its middle on `at` (tiles), `radius` tiles round it,
    /// turned by `turn` (radians) on the ground.
    fn draw_flat(&self, camera: &Camera, at: Vec2, radius: f32, turn: f32, color: Color) {
        let (s, c) = turn.sin_cos();
        let corner = |x: f32, y: f32| camera.world_to_screen(at + vec2(x * c - y * s, x * s + y * c) * radius);
        let r = self.source(0);
        let (tw, th) = (self.texture.width(), self.texture.height());
        let uv = |u: f32, v: f32| vec2((r.x + u * r.w) / tw, (r.y + v * r.h) / th);
        let points = [(corner(-1.0, -1.0), uv(0.0, 0.0)), (corner(1.0, -1.0), uv(1.0, 0.0)), (corner(1.0, 1.0), uv(1.0, 1.0)), (corner(-1.0, 1.0), uv(0.0, 1.0))];
        let vertices = points.iter().map(|(p, t)| Vertex::new(p.x, p.y, 0.0, t.x, t.y, color)).collect();
        let mesh = Mesh { vertices, indices: vec![0, 1, 2, 0, 2, 3], texture: Some(self.texture.clone()) };
        if self.sheet.add { glow(|| draw_mesh(&mesh)) } else { draw_mesh(&mesh) }
    }
}

/// Is this point (pixels) on the screen, give or take `margin`?
fn on_screen(p: Vec2, margin: f32) -> bool {
    p.x > -margin && p.y > -margin && p.x < screen_width() + margin && p.y < screen_height() + margin
}

/// The part of `area` that is on the screen (a little more) – so only those tiles are looked at, not the whole map.
fn screen_part(camera: &Camera, area: Region) -> Region {
    let (w, h) = (screen_width(), screen_height());
    let corners = [vec2(0.0, 0.0), vec2(w, 0.0), vec2(0.0, h), vec2(w, h)].map(|c| to_frame(camera.screen_to_world(c)));
    let min = corners.iter().fold(Vec2::splat(f32::MAX), |a, &b| a.min(b)) - Vec2::splat(4.0);
    let max = corners.iter().fold(Vec2::splat(f32::MIN), |a, &b| a.max(b)) + Vec2::splat(4.0);
    Region { min: min.max(area.min), max: max.min(area.max) }
}
