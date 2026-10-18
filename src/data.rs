//! All values of the game in one place: the JSON files in `data\` (DESIGN.md 12.4) – units, buildings, zombies, the
//! colony, the constant spawn and the waves of a new round. What each value means is in `data\README.md`.
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
use crate::sim::{Breed, TICKS_PER_SECOND};

/// Everything from the five files.
pub struct GameData {
    pub units: Units,
    pub buildings: Buildings,
    pub zombies: Zombies,
    pub colony: Colony,
    pub spawning: Spawning,
}

/// `data\units.json`
#[derive(Deserialize)]
pub struct Units {
    pub soldier: Stats,
    pub ranger: Stats,
    pub sniper: Stats,
    pub medic: Stats,
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
}

/// `data\zombies.json`
#[derive(Deserialize)]
pub struct Zombies {
    pub walker: BreedStats,
    pub runner: BreedStats,
    pub fat: BreedStats,
    /// Time between two hits (read in seconds, kept in ticks).
    #[serde(rename = "hit_every_seconds", deserialize_with = "seconds")]
    pub hit_cooldown: u32,
    /// This close they notice a unit even without seeing it. (How far they see: per breed, `sight`.)
    pub hearing: f32,
    /// A building this close (straight line) is noticed and torn down.
    pub notice_buildings: f32,
    /// A shot zombie storms this long, even without seeing the shooter.
    #[serde(rename = "alert_seconds", deserialize_with = "seconds")]
    pub alert: u32,
    /// A zombie that heard a shot (within the shooter's `noise`, `data\units.json`) storms this long.
    #[serde(rename = "noise_seconds", deserialize_with = "seconds")]
    pub noise: u32,
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
    /// Wanderers stroll at most this far from where they were put (tiles) …
    pub wander_radius: f32,
    /// … at this share of their speed.
    pub wander_pace: f32,
    /// A dead unit gets up as a walker after this many seconds – somewhere between min and max.
    #[serde(rename = "rise_min_seconds", deserialize_with = "seconds")]
    pub rise_min: u32,
    #[serde(rename = "rise_max_seconds", deserialize_with = "seconds")]
    pub rise_max: u32,
    /// The zombies roaming the map from the start (`World::scatter_roamers`).
    pub roaming: Roaming,
}

/// Where the roaming zombies lie at the start and which breed they are. How many: the map setup ("Roaming zombies").
/// Distances as a share of the way from the command center to the edge of the playing field (0 = centre, 1 = edge).
#[derive(Deserialize)]
pub struct Roaming {
    /// None closer to the command center than this (tiles).
    pub keep_out: f32,
    /// How much more of them lie further out: the chance to keep a spot is (share of the way) ^ `edge_weight`.
    /// 0 = evenly everywhere, 1 = growing evenly with the distance, 2 = mostly at the edge.
    pub edge_weight: f32,
    /// From this far out runners roam too …
    pub runners_from: f32,
    /// … this share of the zombies there.
    pub runner_share: f32,
    /// From this far out (near the edge) fat ones roam too …
    pub fat_from: f32,
    /// … this share of the zombies there.
    pub fat_share: f32,
}

/// The numbers of one zombie breed.
#[derive(Deserialize)]
pub struct BreedStats {
    /// Tiles per second.
    pub speed: f32,
    pub hp: u32,
    /// Subtracted from every shot that hits it (at least 1 damage always gets through) – like the units' armor.
    pub armor: u32,
    /// Per hit.
    pub damage: u32,
    /// How far it sees your units (tiles) – not through forests and trees.
    pub sight: f32,
    /// How big it is (tiles): zombies, units and buildings keep this far from its middle.
    pub radius: f32,
}

/// `data\colony.json`
#[derive(Deserialize)]
pub struct Colony {
    pub start_gold: u32,
    /// Every colonist pays this much per hour of the round (1 hour = 7.5 s).
    pub gold_per_colonist_per_hour: u32,
    pub start_soldiers: usize,
    pub start_snipers: usize,
    pub start_rangers: usize,
}

/// `data\spawning.json`: what a new map starts with – its constant spawn, and how the waves of its round are planned
/// (`sim/spawning.rs`, `sim/round.rs`).
#[derive(Deserialize)]
pub struct Spawning {
    /// `flatten`: `stages` and `extra_per_day` stand directly in the file, not inside an extra `{ }` – the file stays
    /// as it was, only the program keeps the two together (they go into every map file together).
    #[serde(flatten)]
    pub constant: ConstantSpawn,
    pub waves: WavePlan,
}

/// The constant spawn of a map: groups of zombies out of the spawners set to "Constant", all round long. A copy of it
/// is in every map file (`Serialize`: it can be written, not only read).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct ConstantSpawn {
    /// In order of time. Between two stages of the same breed the values go over smoothly.
    pub stages: Vec<Stage>,
    /// After the last stage every group gets this many zombies more per day.
    pub extra_per_day: f32,
}

/// How the waves of a new round are planned from its number of days (`sim::plan_waves`, DESIGN.md 8.2). The counts
/// are per side the wave comes from.
#[derive(Deserialize)]
pub struct WavePlan {
    /// The first wave comes on this day …
    pub first_day: u32,
    /// … then one every this many days, until the last day …
    pub every_days: u32,
    /// … always at this hour.
    pub hour: u32,
    /// The first wave has this many runners, the last one before the final wave this many – in between they grow evenly.
    pub first: u32,
    pub last: u32,
    /// The final wave on the last day, at this hour, from all four sides: this many runners and this many fat ones per side.
    pub final_hour: u32,
    pub final_runners: u32,
    pub final_fat: u32,
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
        }
    }

    pub fn breed(&self, breed: Breed) -> &BreedStats {
        match breed {
            Breed::Walker => &self.zombies.walker,
            Breed::Runner => &self.zombies.runner,
            Breed::Fat => &self.zombies.fat,
        }
    }

    /// The values built into the program – the files as they were when it was built.
    fn built_in() -> GameData {
        let texts = [
            include_str!("../data/units.json"),
            include_str!("../data/buildings.json"),
            include_str!("../data/zombies.json"),
            include_str!("../data/colony.json"),
            include_str!("../data/spawning.json"),
        ];
        GameData::parse(texts).unwrap_or_else(|e| panic!("the built-in data is broken: {e}"))
    }

    /// The five files' contents, in the order of `FILES`. The error names the file and the spot.
    fn parse(texts: [&str; 5]) -> Result<GameData, String> {
        fn one<T: for<'de> Deserialize<'de>>(text: &str, file: &str) -> Result<T, String> {
            serde_json::from_str(text).map_err(|e| format!("data\\{file}: {e}"))
        }
        Ok(GameData {
            units: one(texts[0], FILES[0])?,
            buildings: one(texts[1], FILES[1])?,
            zombies: one(texts[2], FILES[2])?,
            colony: one(texts[3], FILES[3])?,
            spawning: one(texts[4], FILES[4])?,
        })
    }
}

const FILES: [&str; 5] = ["units.json", "buildings.json", "zombies.json", "colony.json", "spawning.json"];

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
    let Some(folder) = folder() else {
        return; // no folder: the built-in values
    };
    let read = || -> Result<GameData, String> {
        let mut texts = Vec::new();
        for file in FILES {
            let path = folder.join(file);
            texts.push(std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?);
        }
        GameData::parse([&texts[0], &texts[1], &texts[2], &texts[3], &texts[4]])
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

/// `data\` in the folder the game was started in, otherwise next to the exe.
fn folder() -> Option<PathBuf> {
    let here = std::env::current_dir().ok().map(|d| d.join("data"));
    let exe = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("data")));
    [here, exe].into_iter().flatten().find(|d| d.is_dir())
}

/// For `deserialize_with` (also in `kinds.rs`): a time in seconds in the file becomes ticks in the game (20 per
/// second), rounded.
pub fn seconds<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let s = f32::deserialize(d)?;
    Ok((s * TICKS_PER_SECOND + 0.5).floor() as u32)
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
        assert_eq!(data.spawning.waves.first_day, 4, "the waves aren't read");
    }

    /// `reload` finds `data\` (tests run in the project folder) and reads it without a problem. The same files as the
    /// built-in ones – so the other tests running at the same time see the same values.
    #[test]
    fn reload_reads_the_folder() {
        assert!(folder().is_some(), "no data folder found");
        reload();
        assert_eq!(problem(), None);
    }

    /// A broken file is reported with its name, not a crash.
    #[test]
    fn broken_file_names_itself() {
        let texts = [
            include_str!("../data/units.json"),
            "{ \"tent\": 5 }",
            include_str!("../data/zombies.json"),
            include_str!("../data/colony.json"),
            include_str!("../data/spawning.json"),
        ];
        let error = GameData::parse(texts).err().unwrap();
        assert!(error.contains("buildings.json"), "{error}");
    }
}
