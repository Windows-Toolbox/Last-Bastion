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
//! - `spawning.rs`: walkers at the start, spawners, waves sent at once, dead units rising.
//! - `events.rs`: the events of the map, like TAB's – stragglers, swarms, the final wave, the victory, by the clock.
//! - `zombies.rs`: what a zombie notices, where it walks, whom it hits.
//! - `crowd.rs`: how the crowd pushes and flows – smooth circles pushed out of each other, like TAB's physics.
//! - `sleep.rs`: the zombies far from everything – asleep as plain entries until something wakes them.
//! - `ranged.rs`: the zombies that attack from afar – spitters and infested – and the acid in flight.
//! - `neutral.rs`: what the editor gives a placed thing – a tag, an owner (neutral, joinable) – and the neutral ones.
//! - `clipboard.rs`: the editor's copy, cut and paste – things the way the map file keeps them.
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
//! - `scan.rs`: the radar's scan – a circle of the map shown for a while.
//! - `terrain.rs`: when the map changes – recompute what depends on it, push everyone off new terrain.
//! - `rails.rs`: the railway – pieces of track and the lines they make, like TAB's.
//! - `trains.rs`: the train a script sends along a line – it stops at the command center and runs over who's in its way.
//! - `story.rs`: the map's story like TAB's – scripts of actions, goals, counters, the command center's own button.
//! - `regen.rs`: units and zombies healing themselves, like TAB's.
//! - `fallen.rs`: the dead – zombies dying, units lying there, and how a giant's blow throws them.
//! - `fx.rs`: what happened in a tick that only the picture shows (shots, deaths, explosions).
//! - `landscape.rs`: what the map's landscape does to the round – hearing, sight, speed (`data::Modifiers`).
//! - `round.rs`: the round as a whole – how it ends (won, lost), what happened in it.
//!
//! This file: the `World` itself, the order of a tick (`step`) and small helpers that all of them use.

mod areas;
mod arrows;
mod blast;
mod buildings;
mod clipboard;
mod combat;
mod commands;
mod crowd;
mod debt;
mod doom;
mod economy;
mod events;
mod expr;
mod fallen;
mod fire;
mod fx;
mod funcs;
mod landscape;
mod landscaping;
mod market;
mod neutral;
mod orders;
mod ornaments;
mod rails;
mod ranged;
mod regions;
mod regen;
mod research;
mod round;
mod scan;
mod script;
mod sleep;
mod sound;
mod spawning;
mod story;
mod terrain;
mod towers;
mod trains;
mod units;
mod veterans;
mod watch;
mod weather;
mod zombies;
#[cfg(test)]
mod tests;

use macroquad::math::{vec2, Vec2};

use crate::aggro::AggroMap;
use crate::data::{self, Difficulty, KeepOut, Modifiers};
use crate::flow::FlowField;
use crate::grid::SpatialGrid;
use crate::hunt::HuntMap;
use crate::kinds::{BuildingKind, Resource, UnitKind};
use crate::lure::LureField;
use crate::map::Map;
use crate::mapgen::{self, MapSetup};
use crate::rng::Rng;
use crate::vision::Vision;

// What the rest of the game sees of the simulation – the same names as when it was all one file `sim.rs`.
pub use blast::Blast;
pub use clipboard::{count_pieces, ornament_middle, Clip, Piece};
pub use buildings::{Building, Product, Rally, RepairPlan, SwitchBlock, MAX_PRODUCTION};
pub use commands::{Command, Mark, Prey};
pub use debt::DebtStage;
pub use doom::{DoomHouse, Loot, LootKind, Nest};
pub use economy::{Power, Stock};
pub use events::{plan_events, Event, Notice};
pub use fallen::Dying;
pub use fx::{Fallen, Fx, Struck};
pub use landscaping::{Area, Patch, TerrainClip};
pub use neutral::{Neutral, Owner, Tag, Thing, ThingRef};
pub use regions::{Form, Shape, Spot, Zone, MOST_SHAPES, ZONE_COLORS};
pub use expr::{code_text, parse_code, parse_condition, parse_expr, Expr, ExprError, Scope, Ty, Value};
pub use funcs::{signature, FUNCS};
pub use script::{Place, Run};
pub use story::{
    Action, CommandState, CustomCommand, Goal, GoalState, Pick, Script, Site, Step, Story, Trigger, TriggerEvent, VarKind, Variable, Weighted, MOST_NOISE,
};
pub use weather::{Weather, LOOKS};
pub use ranged::Spit;
pub use research::{researched_in, tech};
use arrows::Arrow;
pub use arrows::MUZZLE;
pub use orders::{Order, MAP_SIZES, MOST_PER_TILE};
pub use ornaments::{Ornament, OrnamentKind};
pub use rails::{extensions, joined, line_start, open_ends, rail_pieces, snap as snap_rail, Connector, Extension, RailPiece, Side, Turn};
#[cfg(test)]
pub use rails::RailShape;
pub use trains::{CarAt, CarKind, Motion, Train};
pub use round::{Outcome, Score};
pub use scan::Scanned;
pub use sound::CELL as SOUND_CELL;
pub use sleep::{Sleep, Sleeper};
pub use spawning::{corners as spawner_corners, tiles as spawner_tiles, Axes, BreedCount, Corpse, SpawnMode, Spawner, WaveFrom, WaveGroup};
pub use units::Unit;
pub use zombies::{Breed, Zombie};

use buildings::NO_BUILDING;
use sound::Sound;
use zombies::CHARGE_THROUGH;

pub const TICKS_PER_SECOND: f32 = 20.0;
pub const TICK: f32 = 1.0 / TICKS_PER_SECOND;
/// One day of the round in ticks: 60 seconds, like TAB's (`ColonyDays`, DESIGN.md 4.1).
pub const DAY_TICKS: u64 = 60 * 20;
/// One hour of such a day (a day has 24, like in TAB's "32d 12h").
pub const HOUR_TICKS: u64 = DAY_TICKS / 24;

/// Ticks from one payout of the economy to the next (`economy_every_seconds` in `data\colony.json`, 28 s like TAB's
/// `CResourceGenerator`): then houses pay, gatherers deliver, burners burn and the upkeep is due (`economy.rs`).
pub fn payout_ticks() -> u64 {
    data::get().colony.economy_every.max(1) as u64
}

/// The time between two payouts as the screen shows it, e.g. "28 s".
pub fn payout_text() -> String {
    format!("{} s", (payout_ticks() as f32 / TICKS_PER_SECOND).round())
}

/// How big units are (radius in tiles) – here, because the zombies need it too. Zombies: per breed (`radius` in
/// `data\zombies.json`). TAB's soldier: a body 0.5 wide (its ranger 0.45, its sniper 0.6).
pub const UNIT_RADIUS: f32 = 0.25;

/// What's left of a hit of `damage` against `armor` – like TAB (`CLife.AddDamage`): armor is a share (0.1 = a tenth
/// less), the rest rounded to the nearest whole point, at least 1. `round_ties_even`: a half goes to the even number,
/// like C#'s `Math.Round` – so 6 against 0.25 is 4 here too, not 5. Only `*` and `-` on `f32`: the same on every PC.
pub fn after_armor(damage: u32, armor: f32) -> u32 {
    ((damage as f32 * (1.0 - armor)).round_ties_even() as u32).max(1)
}

#[derive(Clone)]
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
    /// Killed zombies playing their death – only for the picture (`fallen.rs`).
    pub dying: Vec<Dying>,
    /// The houses of the villages of doom, and the loot lying around (`doom.rs`).
    pub doom: Vec<DoomHouse>,
    /// Neutral buildings and units (`neutral.rs`) – nobody's, until a joinable one joins the colony.
    pub neutral: Vec<Neutral>,
    pub loot: Vec<Loot>,
    /// Treasure ruins and power line towers – only to be looked at (`ornaments.rs`).
    pub ornaments: Vec<Ornament>,
    /// Trains on the railway (`trains.rs`) – a script sends them.
    pub trains: Vec<Train>,
    /// Spitters' acid in flight (`ranged.rs`).
    pub spits: Vec<Spit>,
    /// Your rangers' arrows in flight (`arrows.rs`).
    arrows: Vec<Arrow>,
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
    /// The researches done (`research.rs`, indices into `data\research.json`), in the order they were done.
    pub researched: Vec<u16>,
    /// Barrels going off this tick and a little after – only for drawing (where, ticks left).
    pub blasts: Vec<Blast>,
    /// The radar's scan (`scan.rs`): charges ready, ticks the next one has charged, scans showing right now.
    pub scan_charges: u32,
    pub scan_charge: u32,
    pub scans: Vec<Scanned>,
    /// Wood, stone, ore, food (`economy.rs`): the gathering buildings bring them in, buildings cost them.
    pub stock: Stock,
    /// The supplies of food and energy picked up (`LootKind::Food`, `Energy`): the command center makes this much more
    /// food per payout and power, for the rest of the round (TAB's `CommandCenterExtraFood` / `…Energy`).
    pub found_food: u32,
    pub found_power: u32,
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
    /// How many days the round lasts – its events are planned for them (DESIGN.md 4.1). 0: no set length (old map files).
    pub days: u32,
    /// The round's difficulty (`data\game_difficulties.json`), by name – kept in the map file. What it says about the
    /// zombies, the waves and the start is in the map already; only how close every breed may come is looked up
    /// (`set_difficulty`).
    pub difficulty: String,
    /// Per breed: no zombie of it lies closer to the command center than this when the round is made (tiles).
    pub keep_out: KeepOut,
    /// What the map's landscape does to the whole round (`data::Modifiers`) – kept in the map file.
    pub modifiers: Modifiers,
    /// Which of TAB's landscapes the map is drawn as (`MapSetup::look`) – the simulation never looks at it. Set with
    /// `set_look`, which brings the landscape's weather.
    pub look: String,
    /// Snow, rain or clear right now (`weather.rs`) – only for the picture and the sound; scripts change it.
    pub weather: Weather,
    /// How the round ended (`round.rs`) – `None` while it's still going.
    pub outcome: Option<Outcome>,
    /// What happened in the round so far – for the end screen.
    pub score: Score,
    /// Where the zombies come from.
    pub spawners: Vec<Spawner>,
    /// The map's named regions and points (`regions.rs`) – what triggers and events talk about. Players don't see them.
    pub zones: Vec<Zone>,
    pub spots: Vec<Spot>,
    /// How loud it is where (`sound.rs`).
    sound: Sound,
    /// The colony's unrest: all the noise your units and buildings ever made – shots, machines, blasts, building. It
    /// only grows: the silent events (TAB's stragglers) bring more for good (`sound.rs`, `events.rs`).
    pub unrest: u64,
    /// The ways to the places lured zombies are heading for, one per cell, in order of the cell (`zombies.rs`).
    lures: Vec<LureField>,
    /// An event that shows the whole map has come: from now on everything is visible (`Event::reveal`).
    pub revealed: bool,
    /// What your units and buildings see right now, and what they have ever seen (fog of war).
    pub vision: Vision,
    /// Shots of this tick (from, to) – only for drawing.
    pub shots: Vec<(Vec2, Vec2)>,
    /// What happened this tick that only the picture shows (`fx.rs`) – the game takes it after every tick.
    pub fx: Vec<Fx>,
    /// Medic beams of this tick (from, to) – only for drawing.
    pub beams: Vec<(Vec2, Vec2)>,
    /// Shots of the infested this tick (from, to) – only for drawing.
    pub zombie_shots: Vec<(Vec2, Vec2)>,
    pub tick: u64,
    pub seed: u64,
    /// The events of the map, like TAB's (`events.rs`) – each comes by itself when the clock reaches it.
    pub events: Vec<Event>,
    /// Zombies the events still have to let out of their spawners, a few at a time (`events.rs`).
    requests: Vec<events::Request>,
    /// What the screen should say about the events – the game takes them (`std::mem::take`).
    pub notices: Vec<Notice>,
    /// The map's story (`story.rs`): triggers, goals, the command center's button, the variables' starting values.
    pub story: Story,
    /// While the round runs: the variables, the scripts that run (`script.rs`: they wait for a time or for the
    /// player's pick – oldest first), how the goals and the command center's button look (`story.rs`), what a script
    /// decided about the round, how often the button was pressed.
    pub variables: Vec<(String, Value)>,
    pub runs: Vec<Run>,
    pub goal_states: Vec<GoalState>,
    pub command_state: CommandState,
    ending: Option<story::Ending>,
    command_used: u32,
    /// What the triggers watch between ticks (`watch.rs`).
    watched: watch::Watched,
    rng: Rng,
    to_base: FlowField,
    grid: SpatialGrid,
    unit_grid: SpatialGrid,
    /// For every tile the nearest unit (straight line) within hearing – new every tick.
    aggro: AggroMap,
    /// For every tile the nearest building with people in it (TAB's `HasPeople`) within sight of a zombie – by its
    /// number (`id`), so it stays right when other buildings fall. Stamped anew only when they change (`mark_manned`).
    manned: AggroMap,
    /// Those buildings' numbers, as last stamped.
    manned_ids: Vec<u32>,
    /// For every tile the nearest unit on foot, around water and mountains but straight through buildings – the way
    /// of the storming zombies.
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
    /// The last swarm of a wave (`Zombie::swarm`) – the next one gets the number after it.
    next_swarm: u32,
    next_unit_id: u32,
    next_building_id: u32,
    next_spawner_id: u32,
    /// Regions and points count together.
    next_zone_id: u32,
    next_doom_id: u32,
    next_loot_id: u32,
    next_neutral_id: u32,
}

impl World {
    /// A new round: the map from `setup` (same seed + same values = same map, `mapgen`), its spawners, the
    /// command center on the base, the start units, the zombies roaming the map, and the round – its days and the
    /// events planned for them (`events.rs`). How many zombies, how strong the waves, what the colony starts with: its
    /// difficulty (`data\game_difficulties.json`).
    pub fn generate(seed: u64, setup: &MapSetup) -> World {
        World::generate_with(seed, setup, data::get().difficulty(&setup.round.difficulty))
    }

    /// `generate` with this difficulty – the tests change one to try something out.
    fn generate_with(seed: u64, setup: &MapSetup, difficulty: &Difficulty) -> World {
        let generated = mapgen::generate(seed, &with_villages(setup, difficulty));
        let mut world = World::empty(seed, generated.map);
        // The base where the generator put it – TAB's moves it off the middle to the nearest free spot.
        world.base = vec2(generated.base.0 as f32 + 0.5, generated.base.1 as f32 + 0.5);
        world.set_difficulty(difficulty);
        // Before the first zombie: their speed is rolled with it.
        world.modifiers = setup.modifiers;
        world.set_look(&setup.look);
        world.days = setup.round.days.round() as u32;
        world.gold = difficulty.start_gold;
        world.stock.set(Resource::Wood, difficulty.start_wood);
        // TAB's survival plan, stretched to the days and as big as the zombie population.
        world.events = plan_events(world.days, difficulty.population);
        world.roll_events();
        for spot in generated.spawners {
            world.add_spawner(0, spot.name, spot.min, spot.size, Axes::Screen, SpawnMode::Off);
        }
        // The villages of doom and the loot where the generator put them (`doom.rs`) – every house lets out as many as
        // its size and the difficulty's population say, in batches as big as the population and the landscape's
        // `nests` say (TAB's `InfectedFactor`, `InfectionNestsFactor`); what lies in each pile is rolled here (or the
        // generator says it).
        let nests = if generated.zombies.is_some() { setup.tab.nests } else { 1.0 };
        for house in generated.houses {
            let zombies = World::doom_zombies(house.size, difficulty.population);
            world.add_doom(house.tile, house.size, zombies, difficulty.population * nests);
        }
        world.ornaments = generated.ornaments;
        for spot in generated.loot {
            let pos = vec2(spot.tile.0 as f32 + 0.5, spot.tile.1 as f32 + 0.5);
            let (kind, amount, guards) = world.roll_loot(pos, difficulty.loot_guards);
            // What lies in it: the generator's (TAB's ruins), else what was rolled.
            let (kind, amount) = spot.haul.unwrap_or((kind, amount));
            world.add_loot(pos, kind, amount, guards);
        }
        // The command center right on the base, its middle tile on it – however big `data\buildings.json` makes it.
        let (w, h) = BuildingKind::CommandCenter.footprint(false);
        let (bx, by) = generated.base;
        world.add_building(BuildingKind::CommandCenter, (bx - w / 2, by - h / 2), false);
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
        // The zombies: the ones TAB's generator laid out – asleep, like the roamers – or scattered by the difficulty.
        match generated.zombies {
            Some(spots) => {
                for spot in spots {
                    world.lay_to_sleep(spot.pos, spot.breed);
                }
            }
            None => {
                world.scatter_roamers(difficulty.roaming_zombies);
                world.place_giants(setup.common.giants * difficulty.giants_percent as f32 / 100.0);
            }
        }
        world.wake_near_eyes();
        world.update_vision();
        world
    }

    /// For the tests: a "Wild" map (not TAB's generator, the default since), at the old size of 128 tiles – smaller is
    /// faster, and the tests were written for it. With plenty of gold, so tests that build and train don't depend on the prices – the
    /// tests about gold set it themselves. Plenty of workers, goods and power too, for the same reason – and buildings
    /// stand at once, without their build time. And the start units the tests were written for: 8 soldiers, 4 snipers,
    /// 4 rangers – whatever the default difficulty says now.
    #[cfg(test)]
    pub fn new(seed: u64) -> World {
        let mut setup = MapSetup { generator: mapgen::Generator::Blank, ..MapSetup::default() };
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
            manned: AggroMap::new(size),
            manned_ids: Vec::new(),
            charge: HuntMap::new(size, CHARGE_THROUGH),
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
            dying: Vec::new(),
            doom: Vec::new(),
            neutral: Vec::new(),
            loot: Vec::new(),
            ornaments: Vec::new(),
            trains: Vec::new(),
            spits: Vec::new(),
            arrows: Vec::new(),
            gold: difficulty.start_gold,
            debt: 0,
            army_keep: 1000,
            army_pace: 1000,
            desert_timer: 0,
            credit_seen: 0,
            storage_seen: data::get().colony.storage_gold,
            on_credit: false,
            barrels: 0,
            researched: Vec::new(),
            blasts: Vec::new(),
            scan_charges: 0,
            scan_charge: 0,
            scans: Vec::new(),
            stock: {
                let mut stock = Stock::default();
                stock.set(Resource::Wood, difficulty.start_wood);
                stock
            },
            found_food: 0,
            found_power: 0,
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
            modifiers: Modifiers::default(),
            look: String::new(),
            weather: Weather::Clear,
            outcome: None,
            score: Score::default(),
            spawners: Vec::new(),
            zones: Vec::new(),
            spots: Vec::new(),
            sound: Sound::new(size),
            unrest: 0,
            lures: Vec::new(),
            revealed: false,
            shots: Vec::new(),
            fx: Vec::new(),
            beams: Vec::new(),
            zombie_shots: Vec::new(),
            tick: 0,
            seed,
            events: Vec::new(),
            requests: Vec::new(),
            notices: Vec::new(),
            story: Story::default(),
            variables: Vec::new(),
            runs: Vec::new(),
            goal_states: Vec::new(),
            command_state: CommandState::default(),
            ending: None,
            command_used: 0,
            watched: watch::Watched::default(),
            // Not `Rng::new(seed)`: the map generator already uses that one – this way the two don't repeat each other.
            rng: Rng::new(seed ^ 0x5157_4F52_4C44),
            map_changed: false,
            next_zombie_id: 0,
            next_swarm: 0,
            next_unit_id: 0,
            next_building_id: 0,
            next_spawner_id: 0,
            next_zone_id: 0,
            next_doom_id: 0,
            next_loot_id: 0,
            next_neutral_id: 0,
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
        self.expect_first_payout();
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
        // How far everyone sees on this map (`eye_range`) – worked out first, `vision` is changed with it.
        let reach = Modifiers::factor(self.modifiers.unit_sight);
        let units = self.units.iter().map(|u| (u.pos, u.sight() * reach));
        // Radar, lookout tower and the wonders see over forest (TAB's `WatchThroughOpaque`) – finished and not infected;
        // they're added on top below, like the scans.
        let over_forest = |b: &Building| b.stats().sees_through_forest && b.is_built() && !b.infected;
        let buildings = self.buildings.iter().filter(|b| !over_forest(b)).map(|b| (b.center(), b.stats().sight * reach));
        self.vision.update(&self.map, units.chain(buildings));
        for b in self.buildings.iter().filter(|b| over_forest(b)) {
            self.vision.reveal_circle(b.center(), b.stats().sight * reach);
        }
        // The scans on top – they see over forests too, like StarCraft's.
        let radius = scan::scan_reach();
        for scan in &self.scans {
            self.vision.reveal_circle(scan.at, radius);
        }
    }

    /// One tick: first the orders, then time passes. The order is fixed – the same on every PC.
    pub fn step(&mut self, orders: &[Order]) {
        self.shots.clear();
        self.fx.clear();
        self.beams.clear();
        self.zombie_shots.clear();
        if self.tick == 0 {
            self.start_story(); // story.rs
        }
        self.apply_orders(orders); // orders.rs
        self.update_events(); // events.rs
        self.update_runs(); // script.rs – scripts whose wait is over go on
        self.update_spawners(); // spawning.rs
        self.wake_sleepers(); // sleep.rs – near your eyes, and after the final wave's call
        self.move_fallen(); // fallen.rs – the thrown dead slide on, the dying zombies' death ends
        self.raise_corpses(); // spawning.rs
        self.update_doom(); // doom.rs
        self.listen(); // sound.rs – zombies, houses of doom, guards of loot hear the noise
        self.move_zombies(); // zombies.rs, infected buildings in buildings.rs, shots of the ranged ones in ranged.rs
        self.fly_spits(); // ranged.rs
        self.fly_arrows(); // arrows.rs – before the units choose: a zombie an arrow killed is no target
        self.update_units(); // units.rs, shooting in combat.rs
        self.run_trains(); // trains.rs – after everyone moved: whoever is in a train's way now is hit or pushed out
        self.join_neutrals(); // neutral.rs
        self.watch_triggers(); // watch.rs – time, kills, regions, things reached: their triggers
        self.fire_towers(); // towers.rs – the ballista, the machine gun tower, the shocking tower, the executor
        self.burn(); // fire.rs – zombies Lucifer set alight
        self.detonate(); // blast.rs – barrels hit or carried by the dead go off
        self.update_buildings(); // buildings.rs, gold too
        self.academy(); // veterans.rs – the Academy makes every unit a veteran
        self.update_scan(); // scan.rs
        self.update_debt(); // debt.rs
        self.regenerate(); // regen.rs – units and zombies heal themselves
        let command_center_fell = self.remove_dead();
        self.fade_sound(); // sound.rs
        self.refresh_story_view(); // story.rs – the goals and the command center's button
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
        self.start_dying(); // fallen.rs
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
            mix(u.facing.to_bits()); // decides when it can shoot – and so does how far its weapon is up
            mix(u.weapon);
            mix(u.hp);
            mix(u.barrel as u32);
            mix(u.xp);
        }
        for b in &self.buildings {
            mix(b.hp);
            mix(b.shield);
            mix(b.off as u32);
            mix(b.construction);
            mix(b.reload);
        }
        for &t in &self.researched {
            mix(t as u32);
        }
        for c in &self.corpses {
            mix(c.pos.x.to_bits());
            mix(c.pos.y.to_bits());
            mix(c.rise_in);
        }
        for d in &self.dying {
            mix(d.pos.x.to_bits());
            mix(d.pos.y.to_bits());
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
        for t in &self.trains {
            mix(t.offset.to_bits());
        }
        for left in self.arrows_left() {
            mix(left);
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
/// `village_houses_percent` as big – and TAB's generator with the difficulty's zombie population (`population`). The
/// generator itself never reads `data\` – it only gets the numbers. The terrain stays the same: houses only go on
/// open ground with a ring around them.
fn with_villages(setup: &MapSetup, difficulty: &Difficulty) -> MapSetup {
    let (many, big) = (difficulty.villages_percent as f32 / 100.0, difficulty.village_houses_percent as f32 / 100.0);
    let mut setup = setup.clone();
    setup.tab.population = difficulty.population;
    setup.common.villages *= many;
    setup.common.houses_near *= big;
    setup.common.houses_edge *= big;
    setup
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
