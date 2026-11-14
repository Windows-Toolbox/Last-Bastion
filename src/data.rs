//! All values of the game in one place: the JSON files in `data\` (DESIGN.md 12.4) – units, buildings, zombies, sound,
//! the roaming zombies, the villages of doom, loot, the colony, the events of a new round (TAB's survival plan), and
//! what you pick before a round (like TAB): the difficulties, the durations and the maps. What each value means is in
//! `data\README.md`.
//!
//! The files are built into the program (`include_str!`), so the game also runs without the folder (`dist\`). If a
//! folder `data\` lies next to where the game is started (or next to the exe), `reload` reads it instead – before
//! every new round, so a changed value counts after "Restart" without building anew. A broken file never crashes the
//! game: the old values stay and `problem` says what's wrong.
//!
//! The events are an exception: a new map gets a copy of them, and from then on they belong to its map file (one
//! game = one file, `mapfile.rs`) – the editor can change them there. `data\spawning.json` only decides what new
//! maps start with.
//!
//! Co-op (DESIGN.md 12.8): both PCs need the same values, or they compute different games. Checking that comes with
//! the network code.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, RwLock};

use serde::{Deserialize, Deserializer, Serialize};

use crate::kinds::{BuildingKind, BuildingStats, Resource, Stats, UnitKind};
use crate::mapgen::MapSetup;
use crate::sim::{Breed, Event, TICKS_PER_SECOND};

/// Everything from the files (`FILES`).
pub struct GameData {
    pub units: Units,
    pub buildings: Buildings,
    pub zombies: Zombies,
    /// `data\sound.json`: what makes how much noise, how the zombies hear it (`sim/sound.rs`).
    pub sound: SoundValues,
    /// `data\roaming.json`: the zombies roaming the map from the start (`World::scatter_roamers`).
    pub roaming: Roaming,
    /// `data\doom.json`: the houses of the villages of doom (`sim/doom.rs`).
    pub doom: Doom,
    /// `data\loot.json`: the loot lying around the map, guarded by zombies hidden around it (`sim/doom.rs`).
    pub loot: LootValues,
    pub colony: Colony,
    pub spawning: Spawning,
    /// `data\game_difficulties.json`, `game_durations.json`, `game_maps.json`: what you pick before a round.
    pub difficulties: Difficulties,
    pub durations: Durations,
    pub maps: GameMaps,
    /// `data\research.json`: TAB's research – what each workshop researches, for how much, what it unlocks
    /// (`sim/research.rs`).
    pub research: Research,
    /// `data\audio.json`: which of TAB's sounds go with which unit, zombie and building (`audio.rs`).
    pub audio: AudioData,
    /// `data\trains.json`: how TAB's train runs (`sim/trains.rs`).
    pub trains: TrainValues,
}

/// `data\trains.json`: TAB's `Locomotive` and `TrainCarriage` – in the file per second, here per tick.
#[derive(Deserialize)]
pub struct TrainValues {
    /// Top speed and how fast it gets there (tiles per tick, per tick and tick) – in the file tiles per second and
    /// per second² (TAB: `MaxSpeed` 8 × 30 / 1000 tiles per update, 0.001 more every update, 30 updates a second).
    #[serde(deserialize_with = "per_second")]
    pub max_speed: f32,
    #[serde(deserialize_with = "per_second_squared")]
    pub acceleration: f32,
    /// It brakes this many tiles before a stop: at most its top speed × the way left / this.
    pub brake_tiles: f32,
    /// But never slower than this (tiles per tick – in the file per second) before it's there.
    #[serde(deserialize_with = "per_second")]
    pub least_speed: f32,
    /// It waits this long at a stop (ticks – in the file seconds, TAB's `TimeInStopPoint`).
    #[serde(rename = "stop_seconds", deserialize_with = "seconds")]
    pub stop: u32,
    /// Room between two cars (tiles, `CarriageSeparation`).
    pub gap: f32,
    /// Running faster than `hits_from_speed` (tiles per tick – in the file per second), it hits everyone in its way
    /// with `damage`, every tick they're in it (before their armor).
    pub damage: u32,
    #[serde(deserialize_with = "per_second")]
    pub hits_from_speed: f32,
    /// How long each car is (tiles).
    pub cars: CarLengths,
}

#[derive(Deserialize)]
pub struct CarLengths {
    pub locomotive: f32,
    pub colonists: f32,
    pub armored: f32,
    pub goods: f32,
}

/// `data\audio.json`: TAB's `SoundOn…` columns of its `Entities` and `Commands`, on our kinds. Every value is a list
/// name: every file of its folder whose name contains it (several names: separated by spaces). Left out: silent.
#[derive(Deserialize)]
pub struct AudioData {
    pub units: HashMap<UnitKind, UnitSounds>,
    pub zombies: HashMap<Breed, ZombieSounds>,
    pub buildings: HashMap<BuildingKind, BuildingSounds>,
    /// A house of doom falling: the small one, the medium and large ones.
    pub doom_small: String,
    pub doom_big: String,
    /// The landscape's own sound, looping (TAB's `ZXMapTheme.AmbientSound`), by `look`. While it rains
    /// (`World::weather`) `RainLoopA` plays on top.
    pub ambient: HashMap<String, String>,
}

/// A unit's sounds. `voice`: the start of its voice files in `UnitVoices\` (TAB: `Ranger_What`, `_Yes`, `_Attack`,
/// `_Trained`, `_Die`, `_Bye`); `shot` its shot, `melee` its close blow, `hit` where its missile lands.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct UnitSounds {
    pub voice: String,
    pub shot: String,
    pub melee: String,
    pub hit: String,
}

/// A zombie's sounds: its roar (idle, running), when it strikes, when it dies – and a giant's own.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ZombieSounds {
    pub roar: String,
    pub attack: String,
    pub died: String,
    pub death: String,
    pub fall: String,
    pub steps: String,
}

/// A building's sounds: placed, selected, falling, infected, its shot and where that hits. `structure`: TAB's walls,
/// gates, towers and traps – no "building completed" when they're done.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct BuildingSounds {
    pub placed: String,
    pub selected: String,
    pub destroyed: String,
    pub infected: String,
    pub shot: String,
    pub hit: String,
    pub structure: bool,
}

/// `data\research.json`: TAB's research (`ZX.Commands\Technology.cs`, the `Tech…` rows of `Commands`).
#[derive(Deserialize)]
pub struct Research {
    /// A research takes this long times its `time_factor` (TAB: 1.4 × factor × 50 – seconds of game time).
    pub seconds_per_factor: f32,
    pub techs: Vec<Tech>,
}

/// One research (TAB's `Tech…`): done once, in its workshop, and from then on what it unlocks can be built or trained
/// – as long as what that `requires` stands, too.
#[derive(Deserialize)]
pub struct Tech {
    /// TAB's name (`TechWoodHouse`) – what the key file and later the scripts call it.
    pub id: String,
    /// On the button.
    pub name: String,
    /// The workshop that researches it.
    pub researched_in: BuildingKind,
    /// Its spot on the workshop's card (0–14, row by row) – TAB's `PanelPosition` − 1, moved where repair (8) and
    /// on/off (10) sit on ours.
    pub slot: usize,
    #[serde(default = "one_f32")]
    pub time_factor: f32,
    pub gold: u32,
    #[serde(default)]
    pub wood: u32,
    #[serde(default)]
    pub stone: u32,
    #[serde(default)]
    pub iron: u32,
    #[serde(default)]
    pub oil: u32,
    /// What it unlocks.
    #[serde(default)]
    pub buildings: Vec<BuildingKind>,
    #[serde(default)]
    pub units: Vec<UnitKind>,
    /// It only shows once this research is done and this building stands (TAB: Thanatos and Titan need the
    /// engineering center).
    #[serde(default)]
    pub needs_tech: Option<String>,
    #[serde(default)]
    pub needs_building: Option<BuildingKind>,
}

impl Tech {
    /// Ticks the research takes at full power.
    pub fn ticks(&self, research: &Research) -> u32 {
        (research.seconds_per_factor * self.time_factor * TICKS_PER_SECOND + 0.5).floor() as u32
    }

    /// The goods it costs besides the gold.
    pub fn goods(&self) -> [(Resource, u32); 4] {
        [(Resource::Wood, self.wood), (Resource::Stone, self.stone), (Resource::Iron, self.iron), (Resource::Oil, self.oil)]
    }
}

impl Research {
    /// The research with TAB's name `id`.
    pub fn index(&self, id: &str) -> Option<u16> {
        self.techs.iter().position(|t| t.id == id).map(|i| i as u16)
    }

    /// Something wrong with the list – a name twice, an unknown `needs_tech`, a spot off the card or taken twice.
    fn check(&self) -> Result<(), String> {
        for (i, tech) in self.techs.iter().enumerate() {
            let twice = self.techs[..i].iter().find(|t| t.id == tech.id || (t.researched_in == tech.researched_in && t.slot == tech.slot));
            if let Some(other) = twice {
                return Err(format!("data\\research.json: {} and {} - same name or same spot", other.id, tech.id));
            }
            if tech.slot >= 15 || [9, 10, 14].contains(&tech.slot) {
                return Err(format!("data\\research.json: {} - slot {} isn't free on the card", tech.id, tech.slot));
            }
            if let Some(needs) = tech.needs_tech.as_deref().filter(|&n| self.index(n).is_none()) {
                return Err(format!("data\\research.json: {} needs {needs}, which isn't in the list", tech.id));
            }
        }
        Ok(())
    }
}

fn one_f32() -> f32 {
    1.0
}

/// `data\units.json`
#[derive(Deserialize)]
pub struct Units {
    pub soldier: Stats,
    pub ranger: Stats,
    pub sniper: Stats,
    pub medic: Stats,
    pub spearman: Stats,
    pub merc_soldier: Stats,
    pub merc_ranger: Stats,
    pub merc_sniper: Stats,
    pub lucifer: Stats,
    pub thanatos: Stats,
    pub titan: Stats,
    /// Veterans (DESIGN.md 7.3, `sim/veterans.rs`).
    pub veterans: Veterans,
    /// Healing themselves (every unit's and zombie's `regen`), like TAB's `TimeForLifeRegeneration` and
    /// `MinTimeIdleForLifeRegeneration`: every this long (read in seconds, kept in ticks) …
    #[serde(rename = "regen_every_seconds", deserialize_with = "seconds")]
    pub regen_every: u32,
    /// … once it hasn't been hit for this long – units and zombies alike.
    #[serde(rename = "regen_after_seconds", deserialize_with = "seconds")]
    pub regen_after: u32,
    /// Dismissed (`Order::Dismiss`), a unit gives this many percent of its goods back – wood and iron, not its gold –
    /// rounded up: half of 3 wood is 2.
    pub dismiss_refund_percent: u32,
    /// A unit without a target looks for a new one only this often (read in seconds, kept in ticks) – like TAB's
    /// `EntityWatchInterval` (20 frames, every 2nd counts: 1/3 s). After a kill it stands a moment, weapon up.
    #[serde(rename = "search_seconds", deserialize_with = "seconds")]
    pub search: u32,
}

/// How a unit becomes a veteran: every killed zombie gives its `xp` (`data\zombies.json`), split among your units
/// within `share_radius` tiles – a veteran takes half a share. `xp` of it and the unit is a veteran: `hp_percent` more hit
/// points, `damage_percent` more damage. Mercenaries don't take a share – they are better than veterans already.
#[derive(Deserialize)]
pub struct Veterans {
    pub xp: u32,
    pub share_radius: f32,
    pub hp_percent: u32,
    pub damage_percent: u32,
}

/// `data\buildings.json`
#[derive(Deserialize)]
pub struct Buildings {
    pub command_center: BuildingStats,
    pub barracks: BuildingStats,
    pub tent: BuildingStats,
    pub wall: BuildingStats,
    pub gate: BuildingStats,
    pub tower: BuildingStats,
    pub sawmill: BuildingStats,
    pub mine: BuildingStats,
    pub hunter_cottage: BuildingStats,
    pub fishing_hut: BuildingStats,
    pub farm: BuildingStats,
    pub windmill: BuildingStats,
    pub charcoal_burner: BuildingStats,
    pub wood_workshop: BuildingStats,
    pub tesla_tower: BuildingStats,
    pub stone_workshop: BuildingStats,
    pub munitions_factory: BuildingStats,
    pub stone_house: BuildingStats,
    pub stone_wall: BuildingStats,
    pub stone_tower: BuildingStats,
    pub stakes: BuildingStats,
    pub market: BuildingStats,
    pub bank: BuildingStats,
    pub inn: BuildingStats,
    pub barrel: BuildingStats,
    pub foundry: BuildingStats,
    pub ballista: BuildingStats,
    pub machine_gun_tower: BuildingStats,
    pub wood_house: BuildingStats,
    pub power_plant: BuildingStats,
    pub oil_platform: BuildingStats,
    pub quarry: BuildingStats,
    pub big_quarry: BuildingStats,
    pub big_farm: BuildingStats,
    pub big_mill: BuildingStats,
    pub warehouse: BuildingStats,
    pub engineering_center: BuildingStats,
    pub shocking_tower: BuildingStats,
    pub executor: BuildingStats,
    pub blade_trap: BuildingStats,
    pub mine_trap: BuildingStats,
    pub radar: BuildingStats,
    pub lookout_tower: BuildingStats,
    pub spire: BuildingStats,
    pub academy: BuildingStats,
    pub victorious: BuildingStats,
    pub transmutator: BuildingStats,
    pub crystal_palace: BuildingStats,
    /// Markets sell only what lies above this many percent of the storage (`sim/market.rs`) …
    pub sell_from_percent: u32,
    /// … at these prices (gold per unit).
    pub sell_prices: Prices,
    /// Making, carrying and blowing up barrels (`sim/blast.rs`).
    pub blast: Blast,
    /// The command center's scan (`sim/scan.rs`).
    pub scan: Scan,
    /// Traps hit the zombies on them this often (read in seconds, kept in ticks).
    #[serde(rename = "trap_every_seconds", deserialize_with = "seconds")]
    pub trap_every: u32,
    /// How many units fit into a tower.
    pub tower_slots: usize,
    /// Units in a tower shoot and see this many tiles further.
    pub tower_bonus: f32,
    /// No zombie may be this close to a building site (tiles) – otherwise you can't build there.
    pub zombie_free_radius: i32,
    /// Built over another building (a gate on walls), this many percent of its price come back.
    pub replace_refund_percent: u32,
    /// Torn down by the player (Del): this many percent of its price come back – gold, wood, stone, coal, iron –
    /// of one that is still being built this many (`sim/buildings.rs`, `demolish`).
    pub demolish_refund_percent: u32,
    pub demolish_unfinished_refund_percent: u32,
    /// A repair from nothing to full takes this many seconds – a smaller one less (`BuildingStats::repair_cost`).
    pub repair_seconds: f32,
    /// A building being built starts with this many percent of its hit points and grows to full until it's finished,
    /// like in TAB – its shield comes when it's finished (`build_seconds` of every building).
    pub build_start_percent: u32,
}

/// Gold per unit of every good – what a market gets for it.
#[derive(Deserialize)]
pub struct Prices {
    pub wood: u32,
    pub stone: u32,
    pub iron_ore: u32,
    pub coal: u32,
    pub food: u32,
    /// Iron from the foundry – worth more than the ore and coal in it.
    pub iron: u32,
    /// Oil from the oil platform – as much as iron.
    pub oil: u32,
}

/// Explosive barrels (`sim/blast.rs`): what one costs in the munitions factory and how long it takes …
#[derive(Deserialize)]
pub struct Blast {
    pub cost: u32,
    pub cost_coal: u32,
    #[serde(rename = "build_seconds", deserialize_with = "seconds")]
    pub build_time: u32,
    /// … how hard it hits everyone this close (tiles) when it goes off, and how loud that is …
    pub damage: u32,
    pub radius: f32,
    pub noise: u32,
    /// … and how much slower a unit carrying one walks (it doesn't shoot either).
    pub carry_slow_percent: u32,
}

/// The radar's scan, like StarCraft's: it shows a circle of the map for a while, wherever you like.
#[derive(Deserialize)]
pub struct Scan {
    /// How far (tiles) and how long …
    pub radius: f32,
    #[serde(rename = "seconds", deserialize_with = "seconds")]
    pub time: u32,
    /// … this many charges at most, a new one every this long …
    pub charges: u32,
    #[serde(rename = "every_seconds", deserialize_with = "seconds")]
    pub every: u32,
    /// … and while one charges, the radar takes this much power from its grid.
    pub power: u32,
}

/// `data\zombies.json`
#[derive(Deserialize)]
pub struct Zombies {
    pub walker: BreedStats,
    pub runner: BreedStats,
    pub fat: BreedStats,
    pub armored: BreedStats,
    pub spitter: BreedStats,
    pub infested: BreedStats,
    pub harpy: BreedStats,
    pub giant: BreedStats,
    /// Time between two hits (read in seconds, kept in ticks) – for every breed without its own `attack_seconds`.
    #[serde(rename = "hit_every_seconds", deserialize_with = "seconds")]
    pub hit_cooldown: u32,
    /// Storming zombies find a way to your units up to this many steps (a building tile counts double).
    pub charge_steps: u16,
    /// What an idle zombie does next, rolled with these weights (TAB 1300 / 150 / 150, `sim/zombies.rs`) …
    pub idle: IdleWeights,
    /// … standing this long (TAB: as long as its standing animation) …
    #[serde(rename = "stand_seconds", deserialize_with = "seconds")]
    pub stand: u32,
    /// … growling this long (its special animation) – a giant roars as long. Both only for a breed without its own
    /// `stand_seconds` / `growl_seconds`.
    #[serde(rename = "growl_seconds", deserialize_with = "seconds")]
    pub growl: u32,
    /// Every this long every zombie that isn't with a wave looks around its tile (TAB: 238 frames, a little under 4 s):
    /// at least `contagion_count` wave zombies of its `level` or higher there – it joins them.
    #[serde(rename = "contagion_seconds", deserialize_with = "seconds")]
    pub contagion: u32,
    pub contagion_count: usize,
    /// What comes out of an infected building, and what your fallen units rise as: one of these at random each –
    /// TAB's eight (WeakA/B/C, DressedA, WorkerA/B, MediumA/B).
    pub infected: Vec<Breed>,
    /// Fewer zombies than this left on the map (awake and asleep): every `cull_every_seconds` each one that has no way
    /// to the command center dies (TAB) – so a round doesn't hang on the last zombie on an island.
    pub cull_below: usize,
    #[serde(rename = "cull_every_seconds", deserialize_with = "seconds")]
    pub cull_every: u32,
    /// A dead unit gets up as a zombie (one of `infected`) after this many seconds – somewhere between min and max.
    #[serde(rename = "rise_min_seconds", deserialize_with = "seconds")]
    pub rise_min: u32,
    #[serde(rename = "rise_max_seconds", deserialize_with = "seconds")]
    pub rise_max: u32,
    /// A body thrown by a blow (`push` of a breed) slides this long until it lies still (read in seconds, kept in
    /// ticks). TAB's damping is in its physics library – estimated.
    #[serde(rename = "push_seconds", deserialize_with = "seconds")]
    pub push: u32,
    /// A killed zombie plays its death this long, then it's gone (TAB's `TimeAniDie`: 1 s for every zombie).
    #[serde(rename = "die_seconds", deserialize_with = "seconds")]
    pub die: u32,
}

/// The weights of what an idle zombie does next (`Zombies::idle`).
#[derive(Deserialize)]
pub struct IdleWeights {
    pub stand: u32,
    pub growl: u32,
    pub roam: u32,
}

/// `data\roaming.json`: where the roaming zombies lie at the start and which breed they are. How many: the map setup
/// ("Roaming zombies"). Distances as a share of the way from the command center to the edge of the playing field
/// (0 = centre, 1 = edge).
#[derive(Deserialize)]
pub struct Roaming {
    /// How much more of them lie further out: the chance to keep a spot is (share of the way) ^ `edge_weight`.
    /// 0 = evenly everywhere, 1 = growing evenly with the distance, 2 = mostly at the edge.
    pub edge_weight: f32,
    /// They lie in packs on one tile each, `[fewest, most]` zombies per pack – so never more than `most` on a tile at
    /// the start (TAB: up to 3).
    pub pack: [u32; 2],
    /// Over this many tiles beyond the ring around the command center (the smallest `keep_out`) they thin out toward
    /// it: right at the ring hardly any, one per tile; this far out as many as anywhere, in full packs.
    pub ring_ramp: f32,
    /// Which breeds besides walkers, from how far out and how many of them. Whatever is left over is walkers. The
    /// villages of doom and the guards of loot are mixed the same way. Closer than its `keep_out` (difficulty) a breed
    /// never lies – there it's a walker.
    pub mix: Vec<Mix>,
    /// The giants (`giants` of the map) lie at least this far out – share of the way, like `from`.
    pub giants_from: f32,
}

/// `data\sound.json`: sound like in TAB (`sim/sound.rs`): every event adds noise where it happens; it adds up and
/// halves every second. A zombie comes by chance – the louder and the closer, the surer. The shots of your units:
/// `noise` in `data\units.json`.
#[derive(Deserialize)]
pub struct SoundValues {
    /// It hears this many times as far as it sees (TAB: 4).
    pub hearing: f32,
    /// How sure a listener is to come, in percent: noise × this × its `awareness` / distance² (tiles). TAB: 20.
    pub per_awareness: u32,
    /// Every this long (read in seconds, kept in ticks) every spot's noise halves (TAB: every second).
    #[serde(rename = "halve_seconds", deserialize_with = "seconds")]
    pub halve: u32,
    /// Every this long every zombie listens, each on another tick (TAB: 4 s).
    #[serde(rename = "listen_seconds", deserialize_with = "seconds")]
    pub listen: u32,
    /// … and every this long the houses of doom and the guards of loot (TAB's nests: every second).
    #[serde(rename = "places_listen_seconds", deserialize_with = "seconds")]
    pub places_listen: u32,
    /// How loud: a zombie hitting a unit or a building, a unit dying, a building placed, a building destroyed …
    pub zombie_hit: u32,
    pub unit_dies: u32,
    pub building_placed: u32,
    pub building_destroyed: u32,
    /// … a building infected – plus this much for every zombie that comes out of it.
    pub building_infected: u32,
    pub per_infected: u32,
    /// Every this long (read in seconds, kept in ticks) every working building makes its `noise` – all at once.
    #[serde(rename = "work_every_seconds", deserialize_with = "seconds")]
    pub work_every: u32,
    /// The colony's unrest (`World::unrest`, everything your units and buildings make): for every this much of it, every
    /// side of a silent event (TAB's stragglers) brings one zombie more – for good.
    pub unrest_per_zombie: u64,
}

/// One line of the mix: from `from` of the way out, `share` of the zombies there are this breed.
#[derive(Deserialize)]
pub struct Mix {
    pub breed: Breed,
    pub from: f32,
    pub share: f32,
}

/// `data\doom.json`: the houses of a village of doom, like TAB's (`CInfectionNest`): their zombies come into being in
/// batches when something happens – `sim/doom.rs` says when and how many.
#[derive(Deserialize)]
pub struct Doom {
    pub hp: PerNest<u32>,
    /// This share of every shot doesn't get through, like a zombie's armor (TAB's houses: none).
    pub armor: f32,
    /// How many a house lets out in all (TAB's `InfectionNestMaxUnits`), times the difficulty's population.
    pub max_units: PerNest<u32>,
    /// At most one batch every this long per house.
    #[serde(rename = "batch_every_seconds", deserialize_with = "seconds")]
    pub batch_every: u32,
    /// It hears sounds within `hearing` tiles (by its size – TAB: 4 × its `WatchRange`), as surely as a zombie with
    /// this `awareness` (`sim/sound.rs`). Heard: a batch × `heard_batch` goes there.
    pub hearing: PerNest<f32>,
    pub awareness: f32,
    pub heard_batch: f32,
    /// Every `guard_every_hours` (each house at its own hour): fewer than `guards_per_size` × its size of zombies
    /// around it – a batch × `guard_batch` stays there.
    pub guard_every_hours: u64,
    pub guards_per_size: u32,
    pub guard_batch: f32,
    /// From day `raids_from_day` every `raid_every_days` days (each house on its own day): a batch × `raid_batch`
    /// marches on the command center.
    pub raids_from_day: u64,
    pub raid_every_days: u64,
    pub raid_batch: f32,
    /// Hit: a batch × `hit_batch` goes for the closest of your units within `hit_reach` tiles, and every house nearby
    /// sends a batch × `neighbour_batch` along.
    pub hit_batch: f32,
    pub hit_reach: f32,
    pub neighbour_batch: f32,
    /// Called by the final wave: a batch × `final_batch` every time it may, until it's empty.
    pub final_batch: f32,
    /// Which breeds come out of a house of each size, and how likely (TAB's weights).
    pub breeds: PerNest<Vec<Weighted>>,
    /// A destroyed house drops loot: this much gold – or this much wood, stone, iron ore or coal.
    pub loot_gold: u32,
    pub loot_goods: u32,
    /// Their looks by size, like the buildings' `skins` (`building_sprites.rs`): each house wears one of them. Left
    /// out, or none of them there: drawn as a block.
    #[serde(default)]
    pub skins: Option<PerNest<Vec<String>>>,
    /// What a house looks like when it falls, by size – like the buildings' `explosion`.
    #[serde(default)]
    pub explosion: Option<PerNest<Vec<String>>>,
}

/// One value for each size of house of doom (TAB's `ZXInfectionNestSize`).
#[derive(Deserialize)]
pub struct PerNest<T> {
    pub small: T,
    pub medium: T,
    pub big: T,
}

/// A breed and how likely it is next to the others in its list.
#[derive(Deserialize)]
pub struct Weighted {
    pub breed: Breed,
    pub weight: u32,
}

/// `data\loot.json`: loot lying around, like TAB's: picked up by walking onto it. Zombies lie hidden around it.
#[derive(Deserialize)]
pub struct LootValues {
    /// A unit this close (tiles) – or a sound within `hearing` tiles, heard as surely as a zombie with this `awareness`
    /// would (`sim/sound.rs`) – and the guards come out of the ground.
    pub wake_radius: f32,
    pub hearing: f32,
    pub awareness: f32,
    /// One pile: this much gold – or this much wood, stone, iron ore or coal …
    pub gold: u32,
    pub goods: u32,
    /// … gold in this share of the piles.
    pub gold_share: f32,
    /// A supply of food / energy (TAB's `PickableFood`, `PickableEnergy`: `FoodSupply`, `EnergySupply`): the command
    /// center makes this much more food per payout / power from then on.
    pub food: u32,
    pub energy: u32,
}

/// The numbers of one zombie breed.
#[derive(Deserialize)]
pub struct BreedStats {
    /// Shown in the game. Only here – the breed's name in the files (`"Armored"`) stays, so a new name is one line.
    pub name: String,
    /// How easily it hears a sound – TAB's `ActivityAwarenessFactor`: how sure it is to come grows with it
    /// (`per_awareness` of `SoundValues`, `sim/sound.rs`).
    pub awareness: f32,
    /// Tiles per second when it runs – charging, with a wave, to a sound (TAB's `RunSpeed`).
    pub speed: f32,
    /// Tiles per second when it walks – roaming about, idle (TAB's `WalkSpeed`).
    pub walk_speed: f32,
    /// How long it stands and how long it growls when it idles (TAB: as long as its animations, `TimeAniNormal` and
    /// `TimeAniSpecial`; read in seconds, kept in ticks). Left out: `stand_seconds` / `growl_seconds` of all zombies.
    #[serde(default, rename = "stand_seconds", deserialize_with = "some_seconds")]
    pub stand: Option<u32>,
    #[serde(default, rename = "growl_seconds", deserialize_with = "some_seconds")]
    pub growl: Option<u32>,
    pub hp: u32,
    /// Armor like TAB's: this share of every shot doesn't get through (`after_armor`, at least 1 always does).
    pub armor: f32,
    /// It heals itself like TAB's zombies (`LifeRegenFactor`): this share of its full hit points (rounded up) every
    /// `regen_every_seconds` of `data\units.json`, once it hasn't been hit for `regen_after_seconds` – a zombie shot at
    /// and then let go is whole again soon.
    pub regen: f32,
    /// How much fire hurts it (TAB's `FireDamageFactor` – the spitter 1.5) …
    pub fire_factor: f32,
    /// … and how long it burns once it's set on fire (TAB's `BurningTime`; the giant: 0, it doesn't catch fire). Read in
    /// seconds, kept in ticks (`sim/fire.rs`).
    #[serde(rename = "burn_seconds", deserialize_with = "seconds")]
    pub burn: u32,
    /// Per hit – a spitter: on everyone in its splash.
    pub damage: u32,
    /// How far it sees your units and the buildings with people in them (tiles) – not through forest.
    pub sight: f32,
    /// It looks around for them every this long (TAB's `EntityWatchInterval`, read in seconds, kept in ticks) – twice
    /// as long while it's after something, three times out of your sight, five times where you've never been.
    #[serde(rename = "watch_seconds", deserialize_with = "seconds")]
    pub watch: u32,
    /// Its rank (TAB's `Level`): it joins a wave only if enough of its zombies stand on its tile with this rank or a
    /// higher one (`contagion_seconds`).
    pub level: u32,
    /// Roaming, it goes `[at least, at most]` this far per trip (tiles, on each axis on its own – a square, like TAB),
    /// from wherever it stands. No home it keeps to: over time it may get anywhere. The rarer the breed, the further.
    pub stroll: [f32; 2],
    /// Roaming, it never goes into this ring around the command center (tiles) – unless it's in it already. TAB's
    /// giant: 54. Left out: 0 – no ring.
    #[serde(default)]
    pub roam_ring: f32,
    /// Every this long it stops and roars for `growl_seconds`, whatever it's doing (TAB's giant, read in seconds, kept
    /// in ticks). A breed that roars thinks like TAB's giant: it never goes to where it lost sight of you and never
    /// joins a wave. Left out: it doesn't.
    #[serde(default, rename = "roar_every_seconds", deserialize_with = "some_seconds")]
    pub roar: Option<u32>,
    /// How big it is (tiles): zombies, units and buildings keep this far from its middle.
    pub radius: f32,
    /// How heavy it is (a walker: 1) – in the crowd the lighter one gives way (`sim/crowd.rs`): a giant walks through
    /// walkers, they hardly move it. TAB's `Mass` (unknown, estimated).
    pub mass: f32,
    /// Its hits are venom, like TAB's venom (the spitter): armor doesn't stop them, a unit takes its `venom_factor` of
    /// them (`data\units.json`). Left out: `false` – armor counts.
    #[serde(default)]
    pub venom: bool,
    /// A ranged zombie: it attacks from this far (tiles). Left out: 0 – it has to touch you.
    #[serde(default)]
    pub range: f32,
    /// Its shot hits everyone within this many tiles of where it lands (spitter). Left out: 0 – only its target.
    #[serde(default)]
    pub splash: f32,
    /// Its blow sweeps (TAB's giant: `Arc120`, radius 3.5): every hit lands on everyone in front of it – your units on
    /// the ground, your buildings – within `sweep_radius` tiles and `sweep_degrees` wide, aimed at its target, which
    /// it always hits. Left out: 0 – only its target.
    #[serde(default)]
    pub sweep_radius: f32,
    #[serde(default)]
    pub sweep_degrees: f32,
    /// … and other zombies in it too (TAB's `FriendlyDamage`). Left out: `false`.
    #[serde(default)]
    pub sweep_hits_zombies: bool,
    /// A hit of it that kills throws the body away (TAB's `FactorPush`, giant 150): `push / mass / 12` tiles per
    /// second, slowing down over `push_seconds` (`sim/fallen.rs`). Left out: 0 – the dead fall where they stand.
    #[serde(default)]
    pub push: f32,
    /// Time between two attacks (read in seconds, kept in ticks). Left out: `hit_every_seconds` of all zombies.
    #[serde(default, rename = "attack_seconds", deserialize_with = "some_seconds")]
    pub attack: Option<u32>,
    /// Its swing (read in seconds, kept in ticks): the attack's animation starts, the blow lands this long after – TAB's
    /// `TimeAction × TimeFactorAction` (a walker: 1 s × 0.5). A unit that got out of its reach meanwhile isn't hit.
    /// Left out: 0 – it hits at once.
    #[serde(default, rename = "windup_seconds", deserialize_with = "seconds")]
    pub windup: u32,
    /// Its shot flies this many tiles per second – you can step out of the way. Left out: 0 – it hits at once.
    #[serde(default)]
    pub projectile_speed: f32,
    /// Experience it gives when it dies, split among your units nearby (`sim/veterans.rs`). Left out: 1.
    #[serde(default = "one")]
    pub xp: u32,
    /// Leaps over walls (and gates) up to this many tiles thick instead of tearing them down (TAB's harpy, `leap_over`)
    /// – a thicker wall it hits. Left out: 0 – it never leaps.
    #[serde(default)]
    pub jump: f32,
    /// Every hit it lands makes this much noise where it stands (`sim/sound.rs`) – the zombies' noise, not counted
    /// in the unrest. Left out: `zombie_hit` of `data\sound.json`.
    #[serde(default)]
    pub hit_noise: Option<u32>,
    /// Its looks, like the units' `skins`: TAB's walker has three (Weak A, B, C) – every zombie wears one of them.
    #[serde(default)]
    pub skins: Vec<String>,
    /// Its picture if none of its `skins` is there, e.g. `"test/figure"`. Missing too: a box.
    pub sprite: Option<String>,
}

/// `data\colony.json` – what the colony starts with is up to the difficulty (`Difficulty`).
#[derive(Deserialize)]
pub struct Colony {
    /// Your eyes look this many forest tiles deep into a forest (TAB's `MaxOpaqueCellsVisible`) – not past it.
    pub sight_into_forest: u32,
    /// The storage (`World::storage`): this much gold at most, like TAB's command center …
    pub storage_gold: u32,
    /// … this much of every good – wood, stone, iron ore, coal …
    pub storage_goods: u32,
    /// … and this much food (TAB doesn't store food, here it piles up between payouts).
    pub storage_food: u32,
    /// The economy pays out every this many ticks – in the file seconds, 28 like TAB's `CResourceGenerator`
    /// (`sim/economy.rs`): houses pay, gatherers deliver, burners burn, upkeep is due – every value "per payout" in
    /// `data\` means this.
    #[serde(rename = "economy_every_seconds", deserialize_with = "seconds")]
    pub economy_every: u32,
    /// Debt (DESIGN.md 7.2, `sim/debt.rs`): in debt at all, your units do this many percent less damage and shoot
    /// that much slower …
    pub debt_penalty_percent: u32,
    /// … more than this many payouts of upkeep in debt: this much less, and they walk this much slower …
    pub deep_debt_payouts: u32,
    pub deep_debt_penalty_percent: u32,
    pub deep_debt_slow_percent: u32,
    /// … more than this many payouts: every so often one of them deserts – gone for good.
    pub desert_debt_payouts: u32,
    #[serde(rename = "desert_every_seconds", deserialize_with = "seconds")]
    pub desert_every: u32,
    /// Debt inside the banks' credit (Dispo) brings none of the penalties – it costs this many percent of it every
    /// payout instead.
    pub credit_interest_percent: u32,
}

/// `data\spawning.json`: TAB's survival plan – the events a new round gets (`sim::plan_events`, DESIGN.md 8.2), at
/// TAB's 100 days and population 1 – and how fast a side lets out an event's zombies.
#[derive(Deserialize)]
pub struct Spawning {
    /// Every this long each side lets out a burst of an event's zombies (TAB's spawners: every 8/60 s) …
    #[serde(rename = "stream_every_seconds", deserialize_with = "seconds")]
    pub stream_every: u32,
    /// … about this many (TAB's `GenerationSpeed`).
    pub stream_speed: u32,
    pub events: Vec<EventPlan>,
}

/// An event of the plan: the event as a map file has it (`flatten`: its fields stand right in the entry) – and its
/// timing below population 0.5 (TAB's stragglers come later and less often there).
#[derive(Deserialize)]
pub struct EventPlan {
    #[serde(flatten)]
    pub event: Event,
    #[serde(default)]
    pub low_population: Option<Timing>,
}

/// When an event comes: its hour, every how many hours again, how often.
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Timing {
    pub hour: u32,
    pub every_hours: u32,
    pub times: u32,
}

/// `data\game_difficulties.json`: the difficulties to pick before a round (like TAB's zombie population, but more):
/// how many zombies, how close the hard breeds come, how strong the waves are, what the colony starts with. A new round
/// takes them over into its map file (`World::generate`) – only `keep_out` is looked up by the name the map file keeps.
#[derive(Deserialize)]
pub struct Difficulties {
    /// Picked when nothing else is – and for a map that names a difficulty that isn't in the list (any more).
    pub default: String,
    pub difficulties: Vec<Difficulty>,
}

/// One difficulty.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Difficulty {
    pub name: String,
    /// One line for the home screen.
    pub about: String,
    /// The score counts this many percent (TAB's score factor) – only shown so far, there's no score yet.
    pub score_percent: u32,
    /// What the colony starts with: gold, wood (the rest of the stock is empty) and units.
    pub start_gold: u32,
    pub start_wood: u32,
    pub start_soldiers: usize,
    pub start_rangers: usize,
    pub start_snipers: usize,
    /// Zombies roaming the map at the start, per 1,000 tiles they can stand on (`World::scatter_roamers`) – in packs
    /// of `pack` (`roaming.json`), so about 2,000 put a pack on nearly every tile.
    pub roaming_zombies: f32,
    /// Per breed: none lies closer to the command center at the start (tiles). Roaming they may come closer, like in TAB
    /// – only the giant keeps its `roam_ring`.
    pub keep_out: KeepOut,
    /// TAB's zombie population (`FactorZombiePopulation`): on maps of TAB's generator how many zombies it lays out
    /// (times the map's `infected`), half the villages below 0.5, the giants (`mapgen/tab.rs`); the other maps go by
    /// `roaming_zombies`, `keep_out` and the percentages. On every map: how many a house of doom lets out in all and
    /// how big its batches are (`sim/doom.rs`).
    pub population: f32,
    /// The villages of doom on our maps: this many percent as many as the map says, this many percent of its houses in
    /// each.
    pub villages_percent: u32,
    pub village_houses_percent: u32,
    /// The giants: this many percent as many as the map says (`giants` of its setup, `place_giants`).
    pub giants_percent: u32,
    /// Zombies hidden around a pile of loot: `[near the base, at the edge]` – in between it grows with the distance.
    pub loot_guards: [u32; 2],
}

/// One number of tiles per breed – named like the blocks in `zombies.json`.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct KeepOut {
    pub walker: f32,
    pub runner: f32,
    pub fat: f32,
    pub armored: f32,
    pub spitter: f32,
    pub infested: f32,
    pub harpy: f32,
    pub giant: f32,
}

impl KeepOut {
    pub fn of(&self, breed: Breed) -> f32 {
        match breed {
            Breed::Walker => self.walker,
            Breed::Runner => self.runner,
            Breed::Fat => self.fat,
            Breed::Armored => self.armored,
            Breed::Spitter => self.spitter,
            Breed::Infested => self.infested,
            Breed::Harpy => self.harpy,
            Breed::Giant => self.giant,
        }
    }

    /// The smallest of them – no zombie at all comes closer.
    pub fn least(&self) -> f32 {
        Breed::ALL.iter().map(|&b| self.of(b)).fold(f32::INFINITY, f32::min)
    }
}

/// `data\game_durations.json`: how long a round can last (TAB's 80 / 100 / 120 / 150 days, and a short one to try
/// things) – its events are stretched to it (`sim::plan_events`).
#[derive(Deserialize)]
pub struct Durations {
    /// Picked when nothing else is.
    pub default_days: u32,
    pub durations: Vec<GameDuration>,
}

/// One duration.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct GameDuration {
    /// Won at the end of the last day, the final wave at 92 % (TAB). 1 day = 60 seconds.
    pub days: u32,
    /// Like the difficulty's: fewer days are harder, so they count more.
    pub score_percent: u32,
}

/// `data\game_maps.json`: the maps to pick before a round, like TAB's. For now a map is one of the generators with
/// its values – and its landscape's look (`MapSetup::look`).
#[derive(Deserialize)]
pub struct GameMaps {
    pub default: String,
    pub maps: Vec<GameMap>,
}

/// One map.
#[derive(Deserialize, Clone)]
pub struct GameMap {
    pub name: String,
    pub about: String,
    pub score_percent: u32,
    /// How it's generated: everything the setup page has (`mapgen::MapSetup` – generator, map size, ways in, ore …).
    /// What's left out keeps its default; the round (difficulty, days) is picked apart.
    pub setup: MapSetup,
    /// What the landscape does to the whole round (`Modifiers`) – left out: nothing, and the food of TAB's BR.
    #[serde(default)]
    pub modifiers: Modifiers,
}

/// What a map's landscape does to the whole round, like TAB's six landscapes (their `Mods` and food factors in
/// `MapThemes`): the Frozen Highlands need more power and slow everyone down, in the Desolated Wasteland the zombies
/// are livelier. For now one set for the whole map – later maybe per region. A new round takes them from
/// `game_maps.json` into its setup and its map file (`MapFile::modifiers`); the simulation reads them from
/// `World::modifiers`. In percent of the normal value: 100 = unchanged.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct Modifiers {
    /// How easily the zombies hear a sound – their `awareness` (TAB's `Zombie ActivityAwarenessFactor`).
    pub zombie_hearing: u32,
    /// How far the zombies see – and so how far they hear, `hearing` times their sight (`sim/sound.rs`).
    pub zombie_sight: u32,
    /// How fast the zombies walk and run (TAB's `Unit RunSpeed` counts for them too).
    pub zombie_speed: u32,
    /// How fast your units walk.
    pub unit_speed: u32,
    /// How far your units and buildings see (TAB's `ZXEntity WatchRange`; the zombies' is `zombie_sight`).
    pub unit_sight: u32,
    /// How much power every building that takes some needs (TAB's `Building EnergyNeeded`) – rounded like TAB:
    /// `+ round(need × (percent − 100) / 100)`, a half to the even number. A tent's 1 stays 1 at 130 %, 4 become 5.
    pub building_energy: u32,
    /// What a hunter cottage gets from a tile, in percent of its `gather_per_tile` – forest, grass, dirt (TAB's
    /// `TreesFoodFactor`, `GrassFoodFactor`, `EarthFoodFactor`). Left out: TAB's BR (60, 30, 15).
    pub hunting_forest: u32,
    pub hunting_grass: u32,
    pub hunting_dirt: u32,
}

impl Default for Modifiers {
    fn default() -> Modifiers {
        Modifiers {
            zombie_hearing: 100,
            zombie_sight: 100,
            zombie_speed: 100,
            unit_speed: 100,
            unit_sight: 100,
            building_energy: 100,
            hunting_forest: 60,
            hunting_grass: 30,
            hunting_dirt: 15,
        }
    }
}

impl Modifiers {
    /// Nothing changed – the map file leaves them out.
    pub fn is_default(&self) -> bool {
        *self == Modifiers::default()
    }

    /// A percent as a factor: 80 → 0.8.
    pub fn factor(percent: u32) -> f32 {
        percent as f32 / 100.0
    }

    /// The power a building takes with `building_energy` – like TAB's mods on a whole number.
    pub fn energy(&self, need: u32) -> u32 {
        let change = (need as f32 * (self.building_energy as f32 - 100.0) / 100.0).round_ties_even();
        (need as f32 + change).max(0.0) as u32
    }

    /// The ones that aren't 100 %, for the map's info: "Zombies hear: 50 %".
    pub fn changed(&self) -> Vec<String> {
        let all = [
            ("Zombies hear", self.zombie_hearing),
            ("Zombies see", self.zombie_sight),
            ("Zombie speed", self.zombie_speed),
            ("Unit speed", self.unit_speed),
            ("Your sight", self.unit_sight),
            ("Power needed", self.building_energy),
        ];
        all.iter().filter(|(_, p)| *p != 100).map(|(name, p)| format!("{name}: {p} %")).collect()
    }
}

impl GameData {
    pub fn unit(&self, kind: UnitKind) -> &Stats {
        match kind {
            UnitKind::Soldier => &self.units.soldier,
            UnitKind::Ranger => &self.units.ranger,
            UnitKind::Sniper => &self.units.sniper,
            UnitKind::Medic => &self.units.medic,
            UnitKind::Spearman => &self.units.spearman,
            UnitKind::MercSoldier => &self.units.merc_soldier,
            UnitKind::MercRanger => &self.units.merc_ranger,
            UnitKind::MercSniper => &self.units.merc_sniper,
            UnitKind::Lucifer => &self.units.lucifer,
            UnitKind::Thanatos => &self.units.thanatos,
            UnitKind::Titan => &self.units.titan,
        }
    }

    pub fn building(&self, kind: BuildingKind) -> &BuildingStats {
        match kind {
            BuildingKind::CommandCenter => &self.buildings.command_center,
            BuildingKind::Barracks => &self.buildings.barracks,
            BuildingKind::Tent => &self.buildings.tent,
            BuildingKind::Wall => &self.buildings.wall,
            BuildingKind::Gate => &self.buildings.gate,
            BuildingKind::Tower => &self.buildings.tower,
            BuildingKind::Sawmill => &self.buildings.sawmill,
            BuildingKind::Mine => &self.buildings.mine,
            BuildingKind::HunterCottage => &self.buildings.hunter_cottage,
            BuildingKind::FishingHut => &self.buildings.fishing_hut,
            BuildingKind::Farm => &self.buildings.farm,
            BuildingKind::Windmill => &self.buildings.windmill,
            BuildingKind::CharcoalBurner => &self.buildings.charcoal_burner,
            BuildingKind::WoodWorkshop => &self.buildings.wood_workshop,
            BuildingKind::TeslaTower => &self.buildings.tesla_tower,
            BuildingKind::StoneWorkshop => &self.buildings.stone_workshop,
            BuildingKind::MunitionsFactory => &self.buildings.munitions_factory,
            BuildingKind::StoneHouse => &self.buildings.stone_house,
            BuildingKind::StoneWall => &self.buildings.stone_wall,
            BuildingKind::StoneTower => &self.buildings.stone_tower,
            BuildingKind::Stakes => &self.buildings.stakes,
            BuildingKind::Market => &self.buildings.market,
            BuildingKind::Bank => &self.buildings.bank,
            BuildingKind::Inn => &self.buildings.inn,
            BuildingKind::Barrel => &self.buildings.barrel,
            BuildingKind::Foundry => &self.buildings.foundry,
            BuildingKind::Ballista => &self.buildings.ballista,
            BuildingKind::MachineGunTower => &self.buildings.machine_gun_tower,
            BuildingKind::WoodHouse => &self.buildings.wood_house,
            BuildingKind::PowerPlant => &self.buildings.power_plant,
            BuildingKind::OilPlatform => &self.buildings.oil_platform,
            BuildingKind::Quarry => &self.buildings.quarry,
            BuildingKind::BigQuarry => &self.buildings.big_quarry,
            BuildingKind::BigFarm => &self.buildings.big_farm,
            BuildingKind::BigMill => &self.buildings.big_mill,
            BuildingKind::Warehouse => &self.buildings.warehouse,
            BuildingKind::EngineeringCenter => &self.buildings.engineering_center,
            BuildingKind::ShockingTower => &self.buildings.shocking_tower,
            BuildingKind::Executor => &self.buildings.executor,
            BuildingKind::BladeTrap => &self.buildings.blade_trap,
            BuildingKind::MineTrap => &self.buildings.mine_trap,
            BuildingKind::Radar => &self.buildings.radar,
            BuildingKind::LookoutTower => &self.buildings.lookout_tower,
            BuildingKind::Spire => &self.buildings.spire,
            BuildingKind::Academy => &self.buildings.academy,
            BuildingKind::Victorious => &self.buildings.victorious,
            BuildingKind::Transmutator => &self.buildings.transmutator,
            BuildingKind::CrystalPalace => &self.buildings.crystal_palace,
        }
    }

    /// The difficulty with this name – an unknown one (or `""`): the default.
    pub fn difficulty(&self, name: &str) -> &Difficulty {
        let list = &self.difficulties.difficulties;
        // `parse` made sure the list isn't empty – `list[0]` is always there.
        list.iter().find(|d| d.name == name).or_else(|| list.iter().find(|d| d.name == self.difficulties.default)).unwrap_or(&list[0])
    }

    /// The duration with this many days – or the closest one (a map with another number of days).
    pub fn duration(&self, days: u32) -> &GameDuration {
        let list = &self.durations.durations;
        list.iter().min_by_key(|d| d.days.abs_diff(days)).unwrap_or(&list[0])
    }

    /// The map with this name – an unknown one (or `""`): the default.
    pub fn game_map(&self, name: &str) -> &GameMap {
        let list = &self.maps.maps;
        list.iter().find(|m| m.name == name).or_else(|| list.iter().find(|m| m.name == self.maps.default)).unwrap_or(&list[0])
    }

    pub fn breed(&self, breed: Breed) -> &BreedStats {
        match breed {
            Breed::Walker => &self.zombies.walker,
            Breed::Runner => &self.zombies.runner,
            Breed::Fat => &self.zombies.fat,
            Breed::Armored => &self.zombies.armored,
            Breed::Spitter => &self.zombies.spitter,
            Breed::Infested => &self.zombies.infested,
            Breed::Harpy => &self.zombies.harpy,
            Breed::Giant => &self.zombies.giant,
        }
    }

    /// The values built into the program – the files as they were when it was built.
    fn built_in() -> GameData {
        GameData::parse(BUILT_IN).unwrap_or_else(|e| panic!("the built-in data is broken: {e}"))
    }

    /// The files' contents, in the order of `FILES`. The error names the file and the spot.
    fn parse(texts: [&str; FILES.len()]) -> Result<GameData, String> {
        fn one<T: for<'de> Deserialize<'de>>(text: &str, file: &str) -> Result<T, String> {
            serde_json::from_str(text).map_err(|e| format!("data\\{file}: {e}"))
        }
        let data = GameData {
            units: one(texts[0], FILES[0])?,
            buildings: one(texts[1], FILES[1])?,
            zombies: one(texts[2], FILES[2])?,
            sound: one(texts[3], FILES[3])?,
            roaming: one(texts[4], FILES[4])?,
            doom: one(texts[5], FILES[5])?,
            loot: one(texts[6], FILES[6])?,
            colony: one(texts[7], FILES[7])?,
            spawning: one(texts[8], FILES[8])?,
            difficulties: one(texts[9], FILES[9])?,
            durations: one(texts[10], FILES[10])?,
            maps: one(texts[11], FILES[11])?,
            research: one(texts[12], FILES[12])?,
            audio: one(texts[13], FILES[13])?,
            trains: one(texts[14], FILES[14])?,
        };
        data.research.check()?;
        // The lookups (`difficulty`, `duration`, `game_map`) need at least one of each.
        let empty = [
            (data.difficulties.difficulties.is_empty(), FILES[9]),
            (data.durations.durations.is_empty(), FILES[10]),
            (data.maps.maps.is_empty(), FILES[11]),
        ];
        match empty.into_iter().find(|(none, _)| *none) {
            Some((_, file)) => Err(format!("data\\{file}: the list is empty")),
            None => Ok(data),
        }
    }
}

/// The files in `data\` – a new one goes here and into `BUILT_IN`, in the same place.
const FILES: [&str; 15] = [
    "units.json",
    "buildings.json",
    "zombies.json",
    "sound.json",
    "roaming.json",
    "doom.json",
    "loot.json",
    "colony.json",
    "spawning.json",
    "game_difficulties.json",
    "game_durations.json",
    "game_maps.json",
    "research.json",
    "audio.json",
    "trains.json",
];

/// The files as they were when the program was built (`include_str!` only takes a written-out path, no `FILES[i]`).
const BUILT_IN: [&str; FILES.len()] = [
    include_str!("../data/units.json"),
    include_str!("../data/buildings.json"),
    include_str!("../data/zombies.json"),
    include_str!("../data/sound.json"),
    include_str!("../data/roaming.json"),
    include_str!("../data/doom.json"),
    include_str!("../data/loot.json"),
    include_str!("../data/colony.json"),
    include_str!("../data/spawning.json"),
    include_str!("../data/game_difficulties.json"),
    include_str!("../data/game_durations.json"),
    include_str!("../data/game_maps.json"),
    include_str!("../data/research.json"),
    include_str!("../data/audio.json"),
    include_str!("../data/trains.json"),
];

/// The values in use. A `&'static` reference to them, so `kind.stats()` can hand out references that live on.
/// `RwLock`: many may read at the same time, `reload` writes now and then.
static DATA: LazyLock<RwLock<&'static GameData>> = LazyLock::new(|| RwLock::new(Box::leak(Box::new(GameData::built_in()))));
/// What was wrong with the files at the last `reload` – shown in the game.
static PROBLEM: Mutex<Option<String>> = Mutex::new(None);

/// The values in use.
pub fn get() -> &'static GameData {
    *DATA.read().unwrap_or_else(|e| e.into_inner())
}

/// What was wrong with the files at the last `reload` (`None`: nothing).
pub fn problem() -> Option<String> {
    PROBLEM.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Read the folder `data\` anew, if there is one (where the game was started, else next to the exe). Called before
/// every new round. Broken: the old values stay, `problem` tells why.
pub fn reload() {
    let Some(folder) = folder("data") else {
        return; // no folder: the built-in values
    };
    let read = || -> Result<GameData, String> {
        let mut texts = Vec::new();
        for file in FILES {
            let path = folder.join(file);
            texts.push(std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?);
        }
        // `from_fn`: an array made by calling this for every place 0, 1, 2 … – here a `&str` borrowed from each `String`.
        GameData::parse(std::array::from_fn(|i| texts[i].as_str()))
    };
    let result = read();
    let mut problem = PROBLEM.lock().unwrap_or_else(|e| e.into_inner());
    match result {
        Ok(data) => {
            // The old values are never freed – a few kilobytes per round, and every reference to them stays valid.
            *DATA.write().unwrap_or_else(|e| e.into_inner()) = Box::leak(Box::new(data));
            *problem = None;
        }
        Err(e) => *problem = Some(e),
    }
}

/// The folder `name` (`data\`, `assets\`) in the folder the game was started in, otherwise next to the exe.
pub fn folder(name: &str) -> Option<PathBuf> {
    let here = std::env::current_dir().ok().map(|d| d.join(name));
    let exe = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(name)));
    [here, exe].into_iter().flatten().find(|d| d.is_dir())
}

/// For `deserialize_with` (also in `kinds.rs`): a time in seconds in the file becomes ticks in the game (20 per
/// second), rounded.
pub fn seconds<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let s = f32::deserialize(d)?;
    Ok((s * TICKS_PER_SECOND + 0.5).floor() as u32)
}

/// A speed per second in the file – per tick here.
fn per_second<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    Ok(f32::deserialize(d)? / TICKS_PER_SECOND)
}

/// A change of speed per second² in the file – per tick² here.
fn per_second_squared<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    Ok(f32::deserialize(d)? / (TICKS_PER_SECOND * TICKS_PER_SECOND))
}

/// 1 – for values that are 1 when they're left out (`#[serde(default = "one")]`).
fn one() -> u32 {
    1
}

/// `seconds` for a value that may be left out (`#[serde(default)]` gives `None` then – this is only called when it's
/// there).
fn some_seconds<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    seconds(d).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The files in `data\` are complete and readable – otherwise the game wouldn't even start.
    #[test]
    fn built_in_data_parses() {
        let data = GameData::built_in();
        // Seconds become ticks, rounded: the soldier's 0.27 + 0.53 s – a shot every 0.8 s, like TAB plays it.
        let soldier = &data.units.soldier;
        assert_eq!((soldier.aim, soldier.windup, soldier.cooldown), (0, 5, 11), "0.27 + 0.53 s should be 5 + 11 ticks");
        assert!(data.spawning.events.iter().any(|e| e.event.won), "no victory in the plan");
        assert!(data.spawning.events.iter().any(|e| e.event.final_wave), "no final wave in the plan");
        // Every map's setup is read – each has its own landscape (`look`), a missing one would be "".
        assert!(data.maps.maps.iter().all(|m| !m.setup.look.is_empty()), "the maps' setups aren't read");
        assert!(data.difficulties.difficulties.iter().any(|d| d.name == data.difficulties.default), "the default difficulty isn't in the list");
        assert!(data.maps.maps.iter().any(|m| m.name == data.maps.default), "the default map isn't in the list");
        assert!(data.durations.durations.iter().any(|d| d.days == data.durations.default_days), "the default duration isn't in the list");
    }

    /// `reload` finds `data\` (tests run in the project folder) and reads it without a problem. The same files as the
    /// built-in ones – so the other tests running at the same time see the same values.
    #[test]
    fn reload_reads_the_folder() {
        assert!(folder("data").is_some(), "no data folder found");
        reload();
        assert_eq!(problem(), None);
    }

    /// A broken file is reported with its name, not a crash.
    #[test]
    fn broken_file_names_itself() {
        let mut texts = BUILT_IN;
        texts[1] = "{ \"tent\": 5 }";
        let error = GameData::parse(texts).err().unwrap();
        assert!(error.contains("buildings.json"), "{error}");
    }
}
