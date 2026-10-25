//! All values of the game in one place: the JSON files in `data\` (DESIGN.md 12.4) – units, buildings, zombies, sound,
//! the roaming zombies, the villages of doom, loot, the colony, the constant spawn and the waves of a new round, and
//! what you pick before a round (like TAB): the difficulties, the durations and the maps. What each value means is in
//! `data\README.md`.
//!
//! The files are built into the program (`include_str!`), so the game also runs without the folder (`dist\`). If a
//! folder `data\` lies next to where the game is started (or next to the exe), `reload` reads it instead – before
//! every new round, so a changed value counts after "Restart" without building anew. A broken file never crashes the
//! game: the old values stay and `problem` says what's wrong.
//!
//! The constant spawn and the waves are an exception: a new map gets a copy of them, and from then on they belong to
//! its map file (one game = one file, `mapfile.rs`) – the editor can change them there. `data\spawning.json` only
//! decides what new maps start with.
//!
//! Co-op (DESIGN.md 12.8): both PCs need the same values, or they compute different games. Checking that comes with
//! the network code.

use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, RwLock};

use serde::{Deserialize, Deserializer, Serialize};

use crate::kinds::{BuildingKind, BuildingStats, Stats, UnitKind};
use crate::mapgen::MapSetup;
use crate::sim::{Breed, TICKS_PER_SECOND};

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
    /// Veterans (DESIGN.md 7.3, `sim/veterans.rs`).
    pub veterans: Veterans,
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

/// The command center's scan, like StarCraft's: it shows a circle of the map for a while, wherever you like.
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
    /// … and while one charges, the command center takes this much power from its grid.
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
    pub screamer: BreedStats,
    pub giant: BreedStats,
    /// Time between two hits (read in seconds, kept in ticks) – for every breed without its own `attack_seconds`.
    #[serde(rename = "hit_every_seconds", deserialize_with = "seconds")]
    pub hit_cooldown: u32,
    /// This close they notice a unit even without seeing it. (How far they see: per breed, `sight`.)
    pub hearing: f32,
    /// A building this close (straight line) is noticed and torn down.
    pub notice_buildings: f32,
    /// A shot zombie storms this long, even without seeing the shooter.
    #[serde(rename = "alert_seconds", deserialize_with = "seconds")]
    pub alert: u32,
    /// Storming zombies find a way to your units up to this many steps (a building tile counts double).
    pub charge_steps: u16,
    /// The shot zombie and this many of the closest around it look for a gap …
    pub seek_helpers: usize,
    /// … if they stand this close to it (tiles) …
    pub seek_radius: f32,
    /// … for this long …
    #[serde(rename = "seek_seconds", deserialize_with = "seconds")]
    pub seek: u32,
    /// … up to this many steps on foot around walls.
    pub seek_steps: u16,
    /// Wanderers stroll at this share of their speed (how far per trip: per breed, `stroll`).
    pub wander_pace: f32,
    /// A dead unit gets up as a walker after this many seconds – somewhere between min and max.
    #[serde(rename = "rise_min_seconds", deserialize_with = "seconds")]
    pub rise_min: u32,
    #[serde(rename = "rise_max_seconds", deserialize_with = "seconds")]
    pub rise_max: u32,
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
    /// A zombie inside the ring its breed keeps out of (`keep_out` of the difficulty – lured in there, your bad luck)
    /// strolls on at random from there: on every trip it may get at most this many tiles closer to the command center.
    /// Trips further in are dropped, so on the whole it drifts back out – nothing is kept per zombie for it.
    pub inside_step: f32,
    /// Which breeds besides walkers, from how far out and how many of them. Whatever is left over is walkers. The
    /// villages of doom and the guards of loot are mixed the same way. Closer than its `keep_out` (difficulty) a breed
    /// never lies – there it's a walker.
    pub mix: Vec<Mix>,
    /// The giants (`giants` of the difficulty) lie at least this far out – share of the way, like `from`.
    pub giants_from: f32,
}

/// `data\sound.json`: sound like in TAB (`sim/sound.rs`): every event adds noise where it happens; it adds up while it
/// goes on and fades once it's quiet. A zombie comes when it hears enough of it. The shots of your units: `noise` in `data\units.json`.
#[derive(Deserialize)]
pub struct SoundValues {
    /// A zombie needs this much noise divided by its `alertness` to come (TAB: 1000 / 2 for the slow ones …).
    pub threshold: u32,
    /// It hears this many times as far as it sees (TAB: 4).
    pub hearing: f32,
    /// Once a spot has been quiet this long (read in seconds, kept in ticks) its noise halves …
    #[serde(rename = "quiet_seconds", deserialize_with = "seconds")]
    pub quiet: u32,
    /// … and then again every this long.
    #[serde(rename = "halve_seconds", deserialize_with = "seconds")]
    pub halve: u32,
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
    /// group of the constant spawn brings one zombie more – for good.
    pub unrest_per_zombie: u64,
}

/// One line of the mix: from `from` of the way out, `share` of the zombies there are this breed.
#[derive(Deserialize)]
pub struct Mix {
    pub breed: Breed,
    pub from: f32,
    pub share: f32,
}

/// `data\doom.json`: the houses of a village of doom, like TAB's: full of sleeping zombies until one of your units comes close.
#[derive(Deserialize)]
pub struct Doom {
    pub hp: u32,
    /// Subtracted from every shot, like a zombie's. How many zombies sleep in a house: `doom_zombies` of the difficulty.
    pub armor: u32,
    /// A unit this close (tiles from its walls) lets a few out (`near_zombies`) – again every `near_every_seconds`
    /// while it stays.
    pub wake_radius: f32,
    pub near_zombies: u32,
    #[serde(rename = "near_every_seconds", deserialize_with = "seconds")]
    pub near_every: u32,
    /// A sound at least `hear_threshold` loud within `hearing` tiles rouses it: it lets out `release_per_second`
    /// zombies while the sound goes on – they go where the sound was.
    pub hearing: f32,
    pub hear_threshold: u32,
    pub release_per_second: f32,
    /// Every hit lets out this many more – on top of the ones inside, as long as it isn't empty.
    pub hit_zombies: u32,
    /// A destroyed house drops loot: this much gold – or this much wood, stone, iron ore or coal.
    pub loot_gold: u32,
    pub loot_goods: u32,
}

/// `data\loot.json`: loot lying around, like TAB's: picked up by walking onto it. Zombies lie hidden around it.
#[derive(Deserialize)]
pub struct LootValues {
    /// A unit this close (tiles) – or a sound at least `hear_threshold` loud within `hearing` tiles – and the guards
    /// come out of the ground.
    pub wake_radius: f32,
    pub hearing: f32,
    pub hear_threshold: u32,
    /// One pile: this much gold – or this much wood, stone, iron ore or coal …
    pub gold: u32,
    pub goods: u32,
    /// … gold in this share of the piles.
    pub gold_share: f32,
}

/// The numbers of one zombie breed.
#[derive(Deserialize)]
pub struct BreedStats {
    /// Shown in the game. Only here – the breed's name in the files (`"Armored"`) stays, so a new name is one line.
    pub name: String,
    /// How easily it hears a sound: it comes at `threshold` / `alertness` of noise (`SoundValues`). TAB: slow ones 2,
    /// fast ones and fat ones 3, venoms 4.
    pub alertness: u32,
    /// Tiles per second.
    pub speed: f32,
    pub hp: u32,
    /// Subtracted from every shot that hits it (at least 1 damage always gets through) – like the units' armor.
    pub armor: u32,
    /// Per hit – a spitter: on everyone in its splash.
    pub damage: u32,
    /// How far it sees your units (tiles) – not through forest.
    pub sight: f32,
    /// A wanderer strolls at most this far per trip (tiles, in x and y), from wherever it stands – it has no home it
    /// keeps to, so over time it may get anywhere but the ring around the command center its difficulty keeps it out
    /// of (`keep_out`). The rarer the breed, the further.
    pub stroll: f32,
    /// How big it is (tiles): zombies, units and buildings keep this far from its middle.
    pub radius: f32,
    /// A ranged zombie: it attacks from this far (tiles). Left out: 0 – it has to touch you.
    #[serde(default)]
    pub range: f32,
    /// Its shot hits everyone within this many tiles of where it lands (spitter). Left out: 0 – only its target.
    #[serde(default)]
    pub splash: f32,
    /// Time between two attacks (read in seconds, kept in ticks). Left out: `hit_every_seconds` of all zombies.
    #[serde(default, rename = "attack_seconds", deserialize_with = "some_seconds")]
    pub attack: Option<u32>,
    /// Its shot flies this many tiles per second – you can step out of the way. Left out: 0 – it hits at once.
    #[serde(default)]
    pub projectile_speed: f32,
    /// Experience it gives when it dies, split among your units nearby (`sim/veterans.rs`). Left out: 1.
    #[serde(default = "one")]
    pub xp: u32,
    /// A screamer: seeing one of your units it screams – this much noise where it stands (`sim/sound.rs`) …
    #[serde(default)]
    pub scream: u32,
    /// … and again at the earliest after this long (read in seconds, kept in ticks).
    #[serde(default, rename = "scream_every_seconds", deserialize_with = "seconds")]
    pub scream_every: u32,
    /// Its picture: a sprite in `assets\` (`sprites.rs`), e.g. `"test/figure"`. Missing: a box.
    pub sprite: Option<String>,
}

/// `data\colony.json` – what the colony starts with is up to the difficulty (`Difficulty`).
#[derive(Deserialize)]
pub struct Colony {
    /// The storage (`World::storage`): this much gold at most, like TAB's command center …
    pub storage_gold: u32,
    /// … this much of every good – wood, stone, iron ore, coal …
    pub storage_goods: u32,
    /// … and this much food (TAB doesn't store food, here it piles up between payouts).
    pub storage_food: u32,
    /// The economy pays out every this many hours of the round, like TAB's 8-hour tick (`sim/economy.rs`): houses
    /// pay, gatherers deliver, burners burn, upkeep is due – every value "per payout" in `data\` means this.
    pub economy_every_hours: u64,
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

/// `data\spawning.json`: what a new map starts with – its constant spawn, and how the waves of its round are planned
/// (`sim/spawning.rs`, `sim/round.rs`).
#[derive(Deserialize)]
pub struct Spawning {
    /// `flatten`: `stages`, `extra_per_day` and `specials` stand directly in the file, not inside an extra `{ }` – the
    /// file stays as it was, only the program keeps them together (they go into every map file together).
    #[serde(flatten)]
    pub constant: ConstantSpawn,
    pub waves: WavePlan,
}

/// The constant spawn of a map: groups of zombies out of the spawners set to "Constant", all round long – each group
/// crowded on one spot. A copy of it is in every map file (`Serialize`: it can be written, not only read).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct ConstantSpawn {
    /// In order of time. Between two stages of the same breed the values go over smoothly.
    pub stages: Vec<Stage>,
    /// After the last stage every group gets this many zombies more per day.
    pub extra_per_day: f32,
    /// The special breeds that come along in every group from their day on. `default`: map files from before
    /// 2026-10-22 have none.
    #[serde(default)]
    pub specials: Vec<ConstantSpecial>,
}

/// From this day on, `share` of every group of the constant spawn are this breed instead of the stage's – like a
/// wave's specials, only by day.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub struct ConstantSpecial {
    pub day: u64,
    pub breed: Breed,
    pub share: f32,
}

/// How the waves of a new round are planned from its number of days (`sim::plan_waves`, DESIGN.md 8.2). The counts
/// are per side the wave comes from – times `waves_percent` of the difficulty. On which days they come is up to the
/// duration (`GameDuration`).
#[derive(Deserialize)]
pub struct WavePlan {
    /// Every wave comes at this hour.
    pub hour: u32,
    /// The first wave has this many runners, the last one before the final wave this many – in between they grow evenly.
    pub first: u32,
    pub last: u32,
    /// The other breeds that come along – the first waves only runners, later more and more of these.
    pub specials: Vec<Special>,
    /// The final wave on the last day, at this hour, from all four sides: this many runners per side, the specials in
    /// full. It also calls every zombie on the map and shows the whole map.
    pub final_hour: u32,
    pub final_runners: u32,
}

/// A breed that comes along with the waves: from `from` of the waves on (0 = the first, 1 = the last before the final
/// wave) – a quarter of `share` per runner there, growing to the whole `share` by the last one.
#[derive(Deserialize)]
pub struct Special {
    pub breed: Breed,
    pub from: f32,
    pub share: f32,
}

/// `data\game_difficulties.json`: the difficulties to pick before a round (like TAB's zombie population, but more):
/// how many zombies, how close the hard breeds come, how strong the waves are, what the colony starts with. A new round
/// takes them over into its map file (`World::generate`) – only `keep_out` is looked up while it runs, by the name the
/// map file keeps.
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
    /// Per breed: none lies closer to the command center at the start, none strolls closer (tiles).
    pub keep_out: KeepOut,
    /// The villages of doom: this many percent as many as the map says, this many percent of its houses in each …
    pub villages_percent: u32,
    pub village_houses_percent: u32,
    /// … and this many zombies asleep in every house – the same in each.
    pub doom_zombies: u32,
    /// Giants on a map of TAB's size (256) – more on a bigger one, fewer on a smaller one, far out (`place_giants`).
    #[serde(default)]
    pub giants: u32,
    /// Zombies hidden around a pile of loot: `[near the base, at the edge]` – in between it grows with the distance.
    pub loot_guards: [u32; 2],
    /// Every wave this many percent of the counts in `spawning.json` …
    pub waves_percent: u32,
    /// … every group of the constant spawn this many percent as big.
    pub constant_spawn_percent: u32,
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
    pub screamer: f32,
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
            Breed::Screamer => self.screamer,
            Breed::Giant => self.giant,
        }
    }

    /// The smallest of them – no zombie at all comes closer.
    pub fn least(&self) -> f32 {
        Breed::ALL.iter().map(|&b| self.of(b)).fold(f32::INFINITY, f32::min)
    }
}

/// `data\game_durations.json`: how long a round can last (like TAB's 80 / 100 / 120 / 150 days) – and how often its
/// waves come.
#[derive(Deserialize)]
pub struct Durations {
    /// Picked when nothing else is.
    pub default_days: u32,
    pub durations: Vec<GameDuration>,
}

/// One duration.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct GameDuration {
    /// The final wave comes on the last day. 1 day = 3 minutes.
    pub days: u32,
    /// Like the difficulty's: fewer days are harder, so they count more.
    pub score_percent: u32,
    /// The first wave comes on this day, then one every `wave_every_days` until the day before the last.
    pub first_wave_day: u32,
    pub wave_every_days: u32,
}

/// `data\game_maps.json`: the maps to pick before a round, like TAB's. For now a map is one of the generators with
/// its values – it looks no different.
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
}

/// From this time on: every `every_seconds` a group of `group[0]` to `group[1]` zombies of this breed.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub struct Stage {
    /// Day of the round (from 1) and hour (0–23).
    pub day: u64,
    pub hour: u64,
    pub breed: Breed,
    pub every_seconds: f32,
    pub group: [u32; 2],
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
        }
    }

    /// The difficulty with this name – an unknown one (or `""`): the default.
    pub fn difficulty(&self, name: &str) -> &Difficulty {
        let list = &self.difficulties.difficulties;
        // `parse` made sure the list isn't empty – `list[0]` is always there.
        list.iter().find(|d| d.name == name).or_else(|| list.iter().find(|d| d.name == self.difficulties.default)).unwrap_or(&list[0])
    }

    /// The duration with this many days – or the closest one (a map with another number of days): its waves come as
    /// often.
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
            Breed::Screamer => &self.zombies.screamer,
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
        };
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
const FILES: [&str; 12] = [
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
        assert_eq!(data.units.soldier.cooldown, 8, "0.4 s should be 8 ticks");
        let stages = &data.spawning.constant.stages;
        assert!(stages.windows(2).all(|w| (w[0].day, w[0].hour) <= (w[1].day, w[1].hour)), "stages out of order");
        assert_eq!(data.spawning.waves.hour, 12, "the waves aren't read");
        // Every map's setup is read – a typo in a generator's name would be an error, a missing one the default.
        assert!(data.maps.maps.iter().any(|m| m.setup.generator != MapSetup::default().generator), "the maps' setups aren't read");
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
