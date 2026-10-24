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
//! - `spawning.rs`: walkers at the start, the constant spawn, spawners, waves, dead units rising.
//! - `zombies.rs`: what a zombie notices, where it walks, whom it hits.
//! - `sleep.rs`: the zombies far from everything – asleep as plain entries until something wakes them.
//! - `ranged.rs`: the zombies that attack from afar – spitters and infested – and the acid in flight.
//! - `doom.rs`: the villages of doom and the loot lying around – zombies waiting until you come close.
//! - `sound.rs`: noise like in TAB – what makes how much, where it adds up, who hears it and comes.
//! - `units.rs`: your units – walking, arriving, pushing, climbing into towers.
//! - `commands.rs`: commands as the player gives them, and the units' command queues.
//! - `combat.rs`: whom units aim at, and shooting.
//! - `buildings.rs`: buildings, production queues, towers.
//! - `economy.rs`: the stock, the power grids, gathering, burning, feeding the houses.
//! - `market.rs`: markets – the houses around them pay more, and they sell what the stock has too much of.
//! - `debt.rs`: upkeep and debt – what debt does to the army, deserters, the banks' credit.
//! - `blast.rs`: explosive barrels – making them, carrying them, and what happens when one goes off.
//! - `scan.rs`: the command center's scan – a circle of the map shown for a while.
//! - `terrain.rs`: when the map changes – recompute what depends on it, push everyone off new terrain.
//! - `round.rs`: the round as a whole – its days and waves, how it ends (won, lost), what happened in it.
//!
//! This file: the `World` itself, the order of a tick (`step`) and small helpers that all of them use.

mod blast;
mod buildings;
mod combat;
mod commands;
mod debt;
mod doom;
mod economy;
mod market;
mod orders;
mod ranged;
mod round;
mod scan;
mod sleep;
mod sound;
mod spawning;
mod terrain;
mod units;
mod veterans;
mod zombies;
#[cfg(test)]
mod tests;

use macroquad::math::{vec2, Vec2};

use crate::aggro::AggroMap;
use crate::data::{self, ConstantSpawn, Difficulty, KeepOut};
use crate::flow::FlowField;
use crate::grid::SpatialGrid;
use crate::hunt::{HuntMap, Through};
use crate::kinds::{BuildingKind, Resource, UnitKind};
use crate::lure::LureField;
use crate::map::Map;
use crate::mapgen::{self, MapSetup};
use crate::rng::Rng;
use crate::vision::Vision;

// What the rest of the game sees of the simulation – the same names as when it was all one file `sim.rs`.
pub use blast::Blast;
pub use buildings::{Building, Product, Rally, RepairPlan, SwitchBlock, MAX_PRODUCTION};
pub use commands::{Command, Mark};
pub use debt::DebtStage;
pub use doom::{DoomHouse, Loot, LootKind};
pub use economy::{Power, Stock};
pub use ranged::Spit;
pub use orders::Order;
pub use round::{plan_waves, Outcome, Score};
pub use scan::Scanned;
pub use sleep::Sleep;
pub use spawning::{scaled_groups, stage_start, Corpse, SpawnMode, Spawner, TimedWave, WaveFrom};
pub use units::Unit;
pub use zombies::{Breed, Zombie};

use buildings::NO_BUILDING;
use sound::Sound;
use zombies::CHARGE_THROUGH;

pub const TICKS_PER_SECOND: f32 = 20.0;
pub const TICK: f32 = 1.0 / TICKS_PER_SECOND;
/// One day of the round in ticks: 3 minutes (DESIGN.md 4.1 – fewer but longer days than TAB; open question 18).
pub const DAY_TICKS: u64 = 3 * 60 * 20;
/// One hour of such a day (a day has 24, like in TAB's "32d 12h").
pub const HOUR_TICKS: u64 = DAY_TICKS / 24;

/// Ticks from one payout of the economy to the next (`economy_every_hours` in `data\colony.json`, like TAB's 8-hour
/// tick): then houses pay, gatherers deliver, burners burn and the upkeep is due (`economy.rs`).
pub fn payout_ticks() -> u64 {
    HOUR_TICKS * data::get().colony.economy_every_hours.max(1)
}

/// How big units are (radius in tiles) – here, because the zombies need it too. Zombies: per breed (`radius` in
/// `data\zombies.json`).
pub const UNIT_RADIUS: f32 = 0.175;

pub struct World {
    pub map: Map,
    pub base: Vec2,
    pub zombies: Vec<Zombie>,
    /// The zombies far from everything, asleep – they cost nothing until something wakes them (`sleep.rs`).
    pub sleep: Sleep,
    pub units: Vec<Unit>,
    pub buildings: Vec<Building>,
    /// Your fallen units – they rise as walkers (`spawning.rs`).
    pub corpses: Vec<Corpse>,
    /// The houses of the villages of doom, and the loot lying around (`doom.rs`).
    pub doom: Vec<DoomHouse>,
    pub loot: Vec<Loot>,
    /// Spitters' acid in flight (`ranged.rs`).
    pub spits: Vec<Spit>,
    /// The colony's gold: the tents pay it every hour (`buildings.rs`), units and buildings cost it (`orders.rs`).
    pub gold: u32,
    /// Upkeep the gold didn't cover (`debt.rs`) – paid off first by any gold that comes in, so while there is debt the
    /// gold is 0.
    pub debt: u32,
    /// Thousandths of their damage and shot speed your units keep, and of their speed on foot – less in debt.
    army_keep: u32,
    army_pace: u32,
    /// Ticks since the last deserter – while deep in debt.
    desert_timer: u32,
    /// The banks' credit and the gold storage at the last tick – when a bank falls, its share of the debt goes with
    /// it, and so does the gold above the smaller storage (`debt.rs`).
    credit_seen: u32,
    storage_seen: u32,
    /// While an `Order::Credit` is carried out: gold may go below 0, into the credit.
    on_credit: bool,
    /// Barrels in the munitions factories' store (`blast.rs`) – set from the defence menu, carried off by units.
    pub barrels: u32,
    /// Barrels going off this tick and a little after – only for drawing (where, ticks left).
    pub blasts: Vec<Blast>,
    /// The command center's scan (`scan.rs`): charges ready, ticks the next one has charged, scans showing right now.
    pub scan_charges: u32,
    pub scan_charge: u32,
    pub scans: Vec<Scanned>,
    /// Wood, stone, ore, food (`economy.rs`): the gathering buildings bring them in, buildings cost them.
    pub stock: Stock,
    /// Tests: workers that come from no building (`workers` in `buildings.rs`) – like the gold of `World::new`, so
    /// tests that build and train don't depend on how many workers things take.
    #[cfg(test)]
    pub extra_workers: u32,
    /// Tests: the command center's grid reaches this much further and makes this much more power – the same idea,
    /// so tests that build around the base don't depend on its `power_radius` and `energy`.
    #[cfg(test)]
    pub extra_reach: f32,
    #[cfg(test)]
    pub extra_power: u32,
    /// Tests: this much more fits in the storage, of gold and of every good (`World::storage`).
    #[cfg(test)]
    pub extra_storage: u32,
    /// Tests: buildings stand at once, without their build time – the tests about building time switch it off.
    #[cfg(test)]
    pub instant_build: bool,
    /// How many days the round lasts – the final wave comes on the last one (DESIGN.md 4.1). 0: no set length (old map
    /// files).
    pub days: u32,
    /// The round's difficulty (`data\game_difficulties.json`), by name – kept in the map file. What it says about the
    /// zombies, the waves and the start is in the map already; only how close every breed may come is looked up
    /// (`set_difficulty`).
    pub difficulty: String,
    /// Per breed: no zombie of it strolls closer to the command center than this (tiles).
    pub keep_out: KeepOut,
    /// The constant spawn of this map (`spawning.rs`). A new map gets it from `data\spawning.json` (as big as its
    /// difficulty says), then it's part of its map file.
    pub constant: ConstantSpawn,
    /// How the round ended (`round.rs`) – `None` while it's still going.
    pub outcome: Option<Outcome>,
    /// What happened in the round so far – for the end screen.
    pub score: Score,
    /// Where the zombies come from.
    pub spawners: Vec<Spawner>,
    /// How loud it is where (`sound.rs`).
    sound: Sound,
    /// The colony's unrest: all the noise your units and buildings ever made – shots, machines, blasts, building. It
    /// only grows: the constant spawn brings more for good (`sound.rs`).
    pub unrest: u64,
    /// The ways to the places lured zombies are heading for, one per cell, in order of the cell (`zombies.rs`).
    lures: Vec<LureField>,
    /// A wave that shows the whole map has come: from now on everything is visible (`TimedWave::reveal`).
    pub revealed: bool,
    /// What your units and buildings see right now, and what they have ever seen (fog of war).
    pub vision: Vision,
    /// Shots of this tick (from, to) – only for drawing.
    pub shots: Vec<(Vec2, Vec2)>,
    /// Medic beams of this tick (from, to) – only for drawing.
    pub beams: Vec<(Vec2, Vec2)>,
    /// Shots of the infested this tick (from, to) – only for drawing.
    pub zombie_shots: Vec<(Vec2, Vec2)>,
    pub tick: u64,
    pub seed: u64,
    /// The tick the constant spawn sends its next group (`spawning.rs`).
    next_group: u64,
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
    /// For every tile the nearest unit on foot, around obstacles – for the zombies looking for a gap.
    hunt: HuntMap,
    /// The same, but straight through buildings – the way of the storming zombies.
    charge: HuntMap,
    /// Per tile: would a zombie standing here see one of your units? Seeing goes both ways, so it's computed
    /// from the few units looking out, not from the thousands of zombies. One per breed (`Breed::index`) – they see
    /// differently far.
    zombie_sight: Vec<Vision>,
    /// Per tile: index of the building in `buildings`, or `NO_BUILDING`.
    building_at: Vec<u32>,
    /// Was something built or painted this tick? Then everything that depends on the map is recomputed
    /// once after the orders – not after every single wall of a dragged line.
    map_changed: bool,
    next_zombie_id: u32,
    next_unit_id: u32,
    next_building_id: u32,
    next_spawner_id: u32,
    next_doom_id: u32,
    next_loot_id: u32,
}

impl World {
    /// A new round: the map from `setup` (same seed + same values = same map, `mapgen`), its spawners, the
    /// command center on the base, the start units, the zombies roaming the map, and the round – its days and the
    /// waves planned for them (`round.rs`). How many zombies, how strong the waves, what the colony starts with: its
    /// difficulty (`data\game_difficulties.json`).
    pub fn generate(seed: u64, setup: &MapSetup) -> World {
        World::generate_with(seed, setup, data::get().difficulty(&setup.round.difficulty))
    }

    /// `generate` with this difficulty – the tests change one to try something out.
    fn generate_with(seed: u64, setup: &MapSetup, difficulty: &Difficulty) -> World {
        let generated = mapgen::generate(seed, &with_villages(setup, difficulty));
        let mut world = World::empty(seed, generated.map);
        world.set_difficulty(difficulty);
        world.days = setup.round.days.round() as u32;
        world.gold = difficulty.start_gold;
        world.stock.set(Resource::Wood, difficulty.start_wood);
        // On which days the waves come: the duration with these days (or the closest one).
        let duration = data::get().duration(world.days);
        world.waves = plan_waves(world.days, duration, difficulty.waves_percent as f32 / 100.0);
        world.constant = scaled_groups(&data::get().spawning.constant, difficulty.constant_spawn_percent);
        for spot in generated.spawners {
            world.add_spawner(spot.name, spot.min, spot.size, SpawnMode::Constant);
        }
        // The villages of doom and the loot where the generator put them (`doom.rs`) – every house as full as the
        // difficulty says; what lies in each pile is rolled here.
        for house in generated.houses {
            world.add_doom(house.tile, house.size, difficulty.doom_zombies);
        }
        for tile in generated.loot {
            let pos = vec2(tile.0 as f32 + 0.5, tile.1 as f32 + 0.5);
            let (kind, amount, guards) = world.roll_loot(pos, difficulty.loot_guards);
            world.add_loot(pos, kind, amount, guards);
        }
        // The command center right on the base, its middle tile on it – however big `data\buildings.json` makes it.
        let center = world.map.size as i32 / 2;
        let (w, h) = BuildingKind::CommandCenter.footprint(false);
        world.add_building(BuildingKind::CommandCenter, (center - w / 2, center - h / 2), false);
        world.refresh_map();

        // Rows of 6 below it, a tile away from its edge: soldiers in front, then the snipers, then the rangers.
        let start = [
            (UnitKind::Soldier, difficulty.start_soldiers),
            (UnitKind::Sniper, difficulty.start_snipers),
            (UnitKind::Ranger, difficulty.start_rangers),
        ];
        let below = h as f32 / 2.0 + 1.0;
        let kinds = start.into_iter().flat_map(|(kind, count)| std::iter::repeat_n(kind, count));
        for (i, kind) in kinds.enumerate() {
            let pos = world.base + vec2((i % 6) as f32 - 2.5, (i / 6) as f32 + below);
            world.spawn_unit(kind, pos);
        }
        world.scatter_roamers(difficulty.roaming_zombies);
        world.place_giants(difficulty.giants);
        world.wake_near_eyes();
        world.update_vision();
        world
    }

    /// For the tests: the default "Wild" map, but at the old size of 128 tiles – smaller is faster, and the tests
    /// were written for it. With plenty of gold, so tests that build and train don't depend on the prices – the
    /// tests about gold set it themselves. Plenty of workers, goods and power too, for the same reason – and buildings
    /// stand at once, without their build time. And the start units the tests were written for: 8 soldiers, 4 snipers,
    /// 4 rangers – whatever the default difficulty says now.
    #[cfg(test)]
    pub fn new(seed: u64) -> World {
        let mut setup = MapSetup::default();
        setup.common.size = 128.0;
        let mut difficulty = data::get().difficulty("").clone();
        (difficulty.start_soldiers, difficulty.start_snipers, difficulty.start_rangers) = (8, 4, 4);
        let mut world = World::generate_with(seed, &setup, &difficulty);
        world.gold = 100_000;
        for r in Resource::ALL {
            world.stock.set(r, 100_000);
        }
        world.extra_workers = 100_000;
        world.extra_reach = 10.0;
        world.extra_power = 100_000;
        world.extra_storage = 1_000_000;
        world.instant_build = true;
        world
    }

    /// A world with this map and nothing on it yet – no spawner, no building, nobody. The base is the middle.
    /// `generate` and loading a map file (`mapfile.rs`) fill it.
    pub fn empty(seed: u64, map: Map) -> World {
        let center = map.size as i32 / 2;
        let base = vec2(center as f32 + 0.5, center as f32 + 0.5);
        let size = map.size;
        // Until it's set (`set_difficulty`): the default one.
        let difficulty = data::get().difficulty("");
        World {
            to_base: FlowField::new(&map, base),
            grid: SpatialGrid::new(size),
            unit_grid: SpatialGrid::new(size),
            aggro: AggroMap::new(size),
            building_aggro: AggroMap::new(size),
            hunt: HuntMap::new(size, Through::Never),
            charge: HuntMap::new(size, Through::Cost(CHARGE_THROUGH)),
            zombie_sight: Breed::ALL.iter().map(|_| Vision::new(size)).collect(),
            building_at: vec![NO_BUILDING; size * size],
            vision: Vision::new(size),
            map,
            base,
            zombies: Vec::new(),
            sleep: Sleep::new(size),
            units: Vec::new(),
            buildings: Vec::new(),
            corpses: Vec::new(),
            doom: Vec::new(),
            loot: Vec::new(),
            spits: Vec::new(),
            gold: difficulty.start_gold,
            debt: 0,
            army_keep: 1000,
            army_pace: 1000,
            desert_timer: 0,
            credit_seen: 0,
            storage_seen: data::get().colony.storage_gold,
            on_credit: false,
            barrels: 0,
            blasts: Vec::new(),
            scan_charges: data::get().buildings.scan.charges,
            scan_charge: 0,
            scans: Vec::new(),
            stock: {
                let mut stock = Stock::default();
                stock.set(Resource::Wood, difficulty.start_wood);
                stock
            },
            #[cfg(test)]
            extra_workers: 0,
            #[cfg(test)]
            extra_reach: 0.0,
            #[cfg(test)]
            extra_power: 0,
            #[cfg(test)]
            extra_storage: 0,
            #[cfg(test)]
            instant_build: false,
            days: 0,
            difficulty: difficulty.name.clone(),
            keep_out: difficulty.keep_out,
            constant: data::get().spawning.constant.clone(),
            outcome: None,
            score: Score::default(),
            spawners: Vec::new(),
            sound: Sound::new(size),
            unrest: 0,
            lures: Vec::new(),
            revealed: false,
            shots: Vec::new(),
            beams: Vec::new(),
            zombie_shots: Vec::new(),
            tick: 0,
            seed,
            next_group: 0,
            waves: Vec::new(),
            // Not `Rng::new(seed)`: the map generator already uses that one – this way the two don't repeat each other.
            rng: Rng::new(seed ^ 0x5157_4F52_4C44),
            map_changed: false,
            next_zombie_id: 0,
            next_unit_id: 0,
            next_building_id: 0,
            next_spawner_id: 0,
            next_doom_id: 0,
            next_loot_id: 0,
        }
    }

    /// The round's difficulty: its name (for the map file) and how close every breed may come – the rest of it a new
    /// round takes over when it's made (`generate`).
    pub fn set_difficulty(&mut self, difficulty: &Difficulty) {
        self.difficulty = difficulty.name.clone();
        self.keep_out = difficulty.keep_out;
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
        self.spawn_zombie_as(pos, breed, wander);
    }

    /// A wanderer as it lies in a map file: asleep (`sleep.rs`) – the ones near your eyes wake right away (`settle`).
    pub fn add_sleeper(&mut self, pos: Vec2, breed: Breed) {
        self.lay_to_sleep(pos, breed);
    }

    /// Everything that depends on the map and on what stands on it, computed anew – after loading a map file.
    pub fn settle(&mut self) {
        self.push_out_of_terrain();
        self.refresh_map();
        self.map_changed = false;
        self.wake_near_eyes();
        self.update_vision();
    }

    /// Day (from 1) and hour (0–23) of the round right now.
    pub fn clock(&self) -> (u64, u64) {
        day_and_hour(self.tick)
    }

    /// Everyone looks around: units from where they stand (in a tower further), buildings from their middle. Every
    /// tick – and from outside in the editor, where orders are carried out without time passing: a unit placed there
    /// should lift the fog right away.
    pub fn update_vision(&mut self) {
        if self.revealed {
            self.vision.reveal_all();
            return;
        }
        let units = self.units.iter().map(|u| (u.pos, u.sight()));
        let buildings = self.buildings.iter().map(|b| (b.center(), b.stats().sight));
        self.vision.update(&self.map, units.chain(buildings));
        // The scans on top – they see over forests too, like StarCraft's.
        let radius = data::get().buildings.scan.radius;
        for scan in &self.scans {
            self.vision.reveal_circle(scan.at, radius);
        }
    }

    /// One tick: first the orders, then time passes. The order is fixed – the same on every PC.
    pub fn step(&mut self, orders: &[Order]) {
        self.shots.clear();
        self.beams.clear();
        self.zombie_shots.clear();
        self.apply_orders(orders); // orders.rs
        self.fire_timed_waves(); // spawning.rs
        self.update_spawners(); // spawning.rs
        self.wake_sleepers(); // sleep.rs – near your eyes, and after the final wave's call
        self.raise_corpses(); // spawning.rs
        self.update_doom(); // doom.rs
        self.listen(); // sound.rs – zombies, houses of doom, guards of loot hear the noise
        self.move_zombies(); // zombies.rs, infected buildings in buildings.rs, shots of the ranged ones in ranged.rs
        self.fly_spits(); // ranged.rs
        self.update_units(); // units.rs, shooting in combat.rs
        self.detonate(); // blast.rs – barrels hit or carried by the dead go off
        self.update_buildings(); // buildings.rs, gold too
        self.update_scan(); // scan.rs
        self.update_debt(); // debt.rs
        let command_center_fell = self.remove_dead();
        self.fade_sound(); // sound.rs
        self.check_outcome(command_center_fell); // round.rs
        self.update_vision();
        self.tick += 1;
    }

    /// Clean up at the end of the tick: dead units lie down and disappear, destroyed buildings fall (`buildings.rs`),
    /// destroyed houses of the villages of doom too (`doom.rs`), killed zombies are gone for good (and counted).
    /// Returns whether a command center fell.
    fn remove_dead(&mut self) -> bool {
        self.lay_down_dead_units();
        self.units.retain(|u| u.hp > 0);
        let command_center_fell = self.remove_destroyed_buildings();
        if self.remove_razed_houses() {
            self.refresh_map();
        }
        self.share_experience(); // veterans.rs – the units around the fallen learn
        self.score.zombies_killed += self.zombies.iter().filter(|z| !z.is_alive()).count() as u32;
        self.zombies.retain(|z| z.is_alive());
        // Now and then: the calm ones far from everything fall asleep (`sleep.rs`), and the rest are put in order of
        // where they stand – quicker with many (`zombies.rs`).
        if self.tick.is_multiple_of(zombies::SORT_EVERY) {
            self.fall_asleep();
            self.sort_by_place();
        }
        command_center_fell
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
        for s in self.sleep.iter() {
            mix(s.pos.x.to_bits());
            mix(s.pos.y.to_bits());
        }
        for u in &self.units {
            mix(u.pos.x.to_bits());
            mix(u.pos.y.to_bits());
            mix(u.facing.to_bits()); // decides when it can shoot
            mix(u.hp);
            mix(u.barrel as u32);
            mix(u.xp);
        }
        for b in &self.buildings {
            mix(b.hp);
            mix(b.shield);
            mix(b.off as u32);
            mix(b.construction);
        }
        for c in &self.corpses {
            mix(c.pos.x.to_bits());
            mix(c.pos.y.to_bits());
            mix(c.rise_in);
        }
        for d in &self.doom {
            mix(d.hp);
            mix(d.inside);
        }
        for l in &self.loot {
            mix(l.amount);
            mix(l.guards);
        }
        for s in &self.spits {
            mix(s.left);
        }
        for (_, level) in self.sound.loud() {
            mix(level);
        }
        mix(self.gold);
        mix(self.debt);
        mix(self.barrels);
        mix(self.scan_charges);
        mix(self.unrest as u32);
        for milli in self.stock.all_milli() {
            mix(milli as u32);
        }
        hash
    }
}

/// Tick → day (from 1) and hour (0–23).
pub fn day_and_hour(tick: u64) -> (u64, u64) {
    (tick / DAY_TICKS + 1, tick % DAY_TICKS / HOUR_TICKS)
}

/// How strongly `pos` is pushed away from `other` if both are closer than `min_dist`.
/// Most pairs don't touch: the squared distance tells that without a square root – the slow part.
fn push_apart(pos: Vec2, other: Vec2, min_dist: f32) -> Vec2 {
    let away = pos - other;
    let d2 = away.length_squared();
    if d2 == 0.0 || d2 >= min_dist * min_dist {
        return Vec2::ZERO;
    }
    let d = d2.sqrt();
    away / d * (1.0 - d / min_dist)
}

/// The map's setup with its villages of doom as the difficulty makes them: `villages_percent` as many,
/// `village_houses_percent` as big. The generator itself never reads `data\` – it only gets the numbers. The terrain
/// stays the same: houses only go on open ground with a ring around them.
fn with_villages(setup: &MapSetup, difficulty: &Difficulty) -> MapSetup {
    let (many, big) = (difficulty.villages_percent as f32 / 100.0, difficulty.village_houses_percent as f32 / 100.0);
    let mut setup = setup.clone();
    setup.common.villages *= many;
    setup.common.houses_near *= big;
    setup.common.houses_edge *= big;
    setup
}

/// May a stroller go from `from` straight to `to`? Outside its ring (`keep_out` tiles around the command center at
/// `base`) the whole way has to stay outside – the goal alone isn't enough, a fat one strolls up to 30 tiles and would
/// cut right through the ring. Inside it already (lured there – your bad luck), the way may go at most `inside_step`
/// tiles deeper than where it starts: it strolls on at random, now and then a little closer, but never far – and as
/// every trip further in is dropped, on the whole it drifts back out. Only from where it stands, nothing is kept.
fn keeps_out(from: Vec2, to: Vec2, base: Vec2, keep_out: f32, inside_step: f32) -> bool {
    let start = from.distance(base);
    let limit = if start >= keep_out { keep_out } else { start - inside_step };
    // The point of the way closest to the base: how far along it (0 = `from`, 1 = `to`), kept on the way. The goal is
    // on the way too – if that point is far enough, so is the goal.
    let way = to - from;
    let along = ((base - from).dot(way) / way.length_squared().max(1e-6)).clamp(0.0, 1.0);
    (from + way * along).distance(base) >= limit
}

/// Can you get from `from` to `to` in a straight line without touching a blocked tile? Tile by tile
/// (`Map::line_is_clear`), so not through the corner between two diagonal wall pieces either.
fn clear_line(map: &Map, from: Vec2, to: Vec2) -> bool {
    Map::line_is_clear(from, to, |x, y| map.is_blocked(x, y))
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
