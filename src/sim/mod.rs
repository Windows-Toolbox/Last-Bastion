//! The simulation: everything that "really happens" in the game. Runs in fixed ticks and is
//! deterministic (DESIGN.md 12.5, 12.8): same seed + same orders = same result.
//! Nothing is drawn here and no key is read.
//!
//! Floats: only `+ - * /`, `sqrt`, `abs`, `floor`, `min`/`max` – those are exact on every PC. No `mul_add`;
//! `sin`, `cos`, `atan2` & co. only from the `libm` crate (DESIGN.md 12.8). That also counts for code the
//! simulation calls (`formation.rs`, `flow.rs`, `map.rs` …).
//!
//! One file per part of the game – each one is an `impl World` block, like `placement.rs`:
//! - `orders.rs`: the players' orders – the first part of every tick.
//! - `spawning.rs`: spawners, waves, killed zombies coming back.
//! - `zombies.rs`: what a zombie notices, where it walks, whom it hits.
//! - `units.rs`: your units – walking, arriving, pushing, climbing into towers.
//! - `commands.rs`: commands as the player gives them, and the units' command queues.
//! - `combat.rs`: whom units aim at, and shooting.
//! - `buildings.rs`: buildings, production queues, towers.
//! - `terrain.rs`: when the map changes – recompute what depends on it, push everyone off new terrain.
//!
//! This file: the `World` itself, the order of a tick (`step`) and small helpers that all of them use.

mod buildings;
mod combat;
mod commands;
mod orders;
mod spawning;
mod terrain;
mod units;
mod zombies;
#[cfg(test)]
mod tests;

use macroquad::math::{vec2, Vec2};

use crate::aggro::AggroMap;
use crate::flow::FlowField;
use crate::grid::SpatialGrid;
use crate::hunt::{HuntMap, Through};
use crate::kinds::{BuildingKind, UnitKind};
use crate::map::Map;
use crate::mapgen::{self, MapSetup};
use crate::rng::Rng;
use crate::vision::Vision;

// What the rest of the game sees of the simulation – the same names as when it was all one file `sim.rs`.
pub use buildings::{Building, Rally, MAX_PRODUCTION};
pub use commands::{Command, Mark};
pub use orders::Order;
pub use spawning::{SpawnMode, Spawner, TimedWave, WaveFrom};
pub use units::Unit;
pub use zombies::{Breed, Zombie};

use buildings::NO_BUILDING;
use zombies::CHARGE_THROUGH;

pub const TICKS_PER_SECOND: f32 = 20.0;
pub const TICK: f32 = 1.0 / TICKS_PER_SECOND;
/// One day of the round in ticks: 3 minutes (DESIGN.md 4.1 – fewer but longer days than TAB; open question 18).
pub const DAY_TICKS: u64 = 3 * 60 * 20;
/// One hour of such a day (a day has 24, like in TAB's "32d 12h").
pub const HOUR_TICKS: u64 = DAY_TICKS / 24;

const START_ZOMBIES: usize = 500;
const START_SOLDIERS: usize = 8;
const START_SNIPERS: usize = 4;

/// How big units and zombies are (radius in tiles) – here, because both sides need both.
pub const UNIT_RADIUS: f32 = 0.35;
const ZOMBIE_RADIUS: f32 = 0.25;

pub struct World {
    pub map: Map,
    pub base: Vec2,
    pub zombies: Vec<Zombie>,
    pub units: Vec<Unit>,
    pub buildings: Vec<Building>,
    /// Where the zombies come from.
    pub spawners: Vec<Spawner>,
    /// What your units and buildings see right now, and what they have ever seen (fog of war).
    pub vision: Vision,
    /// Shots of this tick (from, to) – only for drawing.
    pub shots: Vec<(Vec2, Vec2)>,
    /// Medic beams of this tick (from, to) – only for drawing.
    pub beams: Vec<(Vec2, Vec2)>,
    pub tick: u64,
    pub seed: u64,
    pub respawn: bool,
    /// The timeline of waves (editor) – each comes by itself when the clock reaches it.
    pub waves: Vec<TimedWave>,
    rng: Rng,
    to_base: FlowField,
    grid: SpatialGrid,
    unit_grid: SpatialGrid,
    /// For every tile the nearest unit (straight line) within hearing – new every tick.
    aggro: AggroMap,
    /// For every tile the nearest building (straight line) – new when the buildings change.
    building_aggro: AggroMap,
    /// For every tile the nearest unit on foot, around obstacles – for the hunters.
    hunt: HuntMap,
    /// The same, but straight through buildings – the way of the storming zombies.
    charge: HuntMap,
    /// Per tile: would a zombie standing here see one of your units? Seeing goes both ways, so it's computed
    /// from the few units looking out, not from the thousands of zombies.
    zombie_sight: Vision,
    /// Per tile: index of the building in `buildings`, or `NO_BUILDING`.
    building_at: Vec<u32>,
    /// Was something built or painted this tick? Then everything that depends on the map is recomputed
    /// once after the orders – not after every single wall of a dragged line.
    map_changed: bool,
    next_zombie_id: u32,
    next_unit_id: u32,
    next_building_id: u32,
    next_spawner_id: u32,
}

impl World {
    /// A new round: the map from `setup` (same seed + same values = same map, `mapgen`), its spawners, the
    /// command center on the base, the start units and the start horde.
    pub fn generate(seed: u64, setup: &MapSetup) -> World {
        let generated = mapgen::generate(seed, setup);
        let mut world = World::empty(seed, generated.map);
        for spot in generated.spawners {
            world.add_spawner(spot.name, spot.min, spot.size, SpawnMode::Constant);
        }
        // The command center (3 × 3) right on the base.
        let center = world.map.size as i32 / 2;
        world.add_building(BuildingKind::CommandCenter, (center - 1, center - 1), false);
        world.refresh_map();

        // Two rows of 6 below the base: soldiers in front, the rest of the soldiers and the snipers behind.
        for i in 0..START_SOLDIERS + START_SNIPERS {
            let kind = if i < START_SOLDIERS { UnitKind::Soldier } else { UnitKind::Sniper };
            let pos = world.base + vec2((i % 6) as f32 - 2.5, (i / 6) as f32 + 2.5);
            world.spawn_unit(kind, pos);
        }
        world.apply(&Order::SpawnZombies { count: START_ZOMBIES });
        world.update_vision();
        world
    }

    /// For the tests: the default "Wild" map, but at the old size of 128 tiles – smaller is faster, and the tests
    /// were written for it.
    #[cfg(test)]
    pub fn new(seed: u64) -> World {
        let mut setup = MapSetup::default();
        setup.common.size = 128.0;
        World::generate(seed, &setup)
    }

    /// A world with this map and nothing on it yet – no spawner, no building, nobody. The base is the middle.
    /// `generate` and loading a map file (`mapfile.rs`) fill it.
    pub fn empty(seed: u64, map: Map) -> World {
        let center = map.size as i32 / 2;
        let base = vec2(center as f32 + 0.5, center as f32 + 0.5);
        let size = map.size;
        World {
            to_base: FlowField::new(&map, base),
            grid: SpatialGrid::new(size),
            unit_grid: SpatialGrid::new(size),
            aggro: AggroMap::new(size),
            building_aggro: AggroMap::new(size),
            hunt: HuntMap::new(size, Through::Never),
            charge: HuntMap::new(size, Through::Cost(CHARGE_THROUGH)),
            zombie_sight: Vision::new(size),
            building_at: vec![NO_BUILDING; size * size],
            vision: Vision::new(size),
            map,
            base,
            zombies: Vec::new(),
            units: Vec::new(),
            buildings: Vec::new(),
            spawners: Vec::new(),
            shots: Vec::new(),
            beams: Vec::new(),
            tick: 0,
            seed,
            respawn: true,
            waves: Vec::new(),
            // Not `Rng::new(seed)`: the map generator already uses that one – this way the two don't repeat each other.
            rng: Rng::new(seed ^ 0x5157_4F52_4C44),
            map_changed: false,
            next_zombie_id: 0,
            next_unit_id: 0,
            next_building_id: 0,
            next_spawner_id: 0,
        }
    }

    /// A building without the rules of the game (loading a map file). Ignored where another building stands.
    pub fn add_building_anywhere(&mut self, kind: BuildingKind, tile: (i32, i32), turned: bool) {
        self.apply(&Order::PlaceBuilding { kind, tile, turned, editor: true });
    }

    /// A unit where it was saved (loading a map file).
    pub fn add_unit(&mut self, kind: UnitKind, pos: Vec2) {
        self.spawn_unit(kind, pos);
    }

    /// A zombie as placed in the editor (loading a map file).
    pub fn add_zombie(&mut self, pos: Vec2, breed: Breed, wander: bool) {
        self.spawn_zombie_as(pos, Some(breed), wander);
    }

    /// Everything that depends on the map and on what stands on it, computed anew – after loading a map file.
    pub fn settle(&mut self) {
        self.push_out_of_terrain();
        self.refresh_map();
        self.map_changed = false;
        self.update_vision();
    }

    /// Day (from 1) and hour (0–23) of the round right now.
    pub fn clock(&self) -> (u64, u64) {
        day_and_hour(self.tick)
    }

    /// Everyone looks around: units from where they stand (in a tower further), buildings from their middle.
    fn update_vision(&mut self) {
        let units = self.units.iter().map(|u| (u.pos, u.sight()));
        let buildings = self.buildings.iter().map(|b| (b.center(), b.stats().sight));
        self.vision.update(&self.map, units.chain(buildings));
    }

    /// One tick: first the orders, then time passes. The order is fixed – the same on every PC.
    pub fn step(&mut self, orders: &[Order]) {
        self.shots.clear();
        self.beams.clear();
        self.apply_orders(orders); // orders.rs
        self.fire_timed_waves(); // spawning.rs
        self.update_spawners(); // spawning.rs
        self.move_zombies(); // zombies.rs
        self.update_units(); // units.rs, shooting in combat.rs
        self.update_buildings(); // buildings.rs
        self.remove_dead();
        self.update_vision();
        self.tick += 1;
    }

    /// Clean up at the end of the tick: dead units disappear, destroyed buildings fall (`buildings.rs`), killed
    /// zombies (and those at the base) start again in a spawner or disappear too (`spawning.rs`).
    fn remove_dead(&mut self) {
        self.units.retain(|u| u.hp > 0);
        self.remove_destroyed_buildings();
        if self.respawn {
            self.respawn_zombies();
        }
        self.zombies.retain(|z| z.is_alive());
    }

    /// Checksum over all positions and hit points. Same seed + same orders → same number.
    pub fn checksum(&self) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a
        let mut mix = |value: u32| {
            hash ^= value as u64;
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        };
        for z in &self.zombies {
            mix(z.pos.x.to_bits());
            mix(z.pos.y.to_bits());
        }
        for u in &self.units {
            mix(u.pos.x.to_bits());
            mix(u.pos.y.to_bits());
            mix(u.hp);
        }
        for b in &self.buildings {
            mix(b.hp);
        }
        hash
    }
}

/// Tick → day (from 1) and hour (0–23).
pub fn day_and_hour(tick: u64) -> (u64, u64) {
    (tick / DAY_TICKS + 1, tick % DAY_TICKS / HOUR_TICKS)
}

/// How strongly `pos` is pushed away from `other` if both are closer than `min_dist`.
fn push_apart(pos: Vec2, other: Vec2, min_dist: f32) -> Vec2 {
    let away = pos - other;
    let d = away.length();
    if d > 0.0 && d < min_dist { away / d * (1.0 - d / min_dist) } else { Vec2::ZERO }
}

/// Can you get from `from` to `to` in a straight line without touching a blocked tile? Tile by tile
/// (`Map::line_is_clear`), so not through the corner between two diagonal wall pieces either.
fn clear_line(map: &Map, from: Vec2, to: Vec2) -> bool {
    map.line_is_clear(from, to, |x, y| map.is_blocked(x, y))
}

/// Moves by `velocity`, but not into `blocked` tiles: check x and y separately,
/// so you slide along a wall instead of getting stuck.
fn slide(pos: Vec2, velocity: Vec2, blocked: impl Fn(i32, i32) -> bool) -> Vec2 {
    let mut new = pos;
    let x = pos.x + velocity.x;
    if !blocked(x.floor() as i32, new.y.floor() as i32) {
        new.x = x;
    }
    let y = pos.y + velocity.y;
    if !blocked(new.x.floor() as i32, y.floor() as i32) {
        new.y = y;
    }
    new
}
