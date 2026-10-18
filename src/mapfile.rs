//! One game = one file, like in TAB (2026-10-17): every map is one JSON file under `%APPDATA%\Last Bastion\maps\`.
//! Starting a new round writes the file of its map there – the game plays from that file, "Restart" reads it anew,
//! and the map editor (`LastBastionEditor.exe`) opens it, changes it and saves it again. Maps made in the editor land
//! in the same folder. JSON like the settings (DESIGN.md 12.4, question 14): readable, and you can change it by hand.
//!
//! What's in it: the round (how many days, the gold at the start), the constant spawn and the timeline of waves (with
//! the final wave), the spawners, buildings, units, the zombies on the map, the ore deposits and the terrain (one
//! letter per tile, one line of text per row – you can almost see the map in the file). Not in it: what happens during
//! a round (hit points, commands, the clock) – the file is the round as it starts. Saving a round half-way through
//! (DESIGN.md 11.5) comes later; then it goes into the same file.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use macroquad::math::vec2;
use serde::{Deserialize, Serialize};

use crate::data::{self, ConstantSpawn};
use crate::kinds::{BuildingKind, UnitKind};
use crate::map::{Deposit, Map, Ore, Terrain};
use crate::mapgen::MapSetup;
use crate::settings;
use crate::sim::{Breed, SpawnMode, TimedWave, World};

/// Which version of the file format – if it ever changes, old files can still be recognised.
/// 2 (2026-10-17): the round, the constant spawn, the setup, and waves with breed and final wave.
const VERSION: u32 = 2;

/// The fields in the order they are in the file: the short ones on top, the terrain at the end.
/// `#[serde(default …)]`: a field a file from version 1 doesn't have yet gets this value instead of an error.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MapFile {
    pub version: u32,
    pub name: String,
    pub seed: u64,
    pub size: usize,
    pub border: usize,
    #[serde(default)]
    pub round: RoundData,
    /// The constant spawn of this map (`sim/spawning.rs`) – older files: what `data\spawning.json` says now.
    #[serde(default = "default_constant")]
    pub constant_spawn: ConstantSpawn,
    pub waves: Vec<TimedWave>,
    /// How the map was generated – for the info line and "New map" (same setup, new seed). `None`: not known (files
    /// from before 2026-10-17).
    #[serde(default)]
    pub setup: Option<MapSetup>,
    pub spawners: Vec<SpawnerData>,
    pub buildings: Vec<BuildingData>,
    pub units: Vec<UnitData>,
    pub zombies: Vec<ZombieData>,
    /// Every tile with ore on it. `default`: files from before 2026-10-17 have none.
    #[serde(default)]
    pub deposits: Vec<DepositData>,
    /// One string per row, one letter per tile (`letter`).
    pub terrain: Vec<String>,
}

/// The round on this map.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
#[serde(default)]
pub struct RoundData {
    /// How many days the round lasts – the final wave comes on the last one. 0: no set length.
    pub days: u32,
    /// The colony's gold at the start.
    pub gold: u32,
}

impl Default for RoundData {
    /// What a file without a round gets: no set length, the start gold of `data\colony.json`.
    fn default() -> RoundData {
        RoundData { days: 0, gold: data::get().colony.start_gold }
    }
}

/// For `#[serde(default = "default_constant")]` – it wants a function, not a value.
fn default_constant() -> ConstantSpawn {
    data::get().spawning.constant.clone()
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DepositData {
    pub tile: (i32, i32),
    pub ore: Ore,
    pub amount: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SpawnerData {
    pub name: String,
    pub min: (i32, i32),
    pub size: (i32, i32),
    pub mode: SpawnMode,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct BuildingData {
    pub kind: BuildingKind,
    pub tile: (i32, i32),
    pub turned: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct UnitData {
    pub kind: UnitKind,
    pub pos: (f32, f32),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ZombieData {
    pub pos: (f32, f32),
    pub breed: Breed,
    pub wander: bool,
}

/// The letter of a terrain in the file.
fn letter(terrain: Terrain) -> char {
    match terrain {
        Terrain::Grass => '.',
        Terrain::Mountain => 'M',
        Terrain::Forest => 'F',
        Terrain::Tree => 'T',
        Terrain::Water => 'W',
    }
}

fn from_letter(c: char) -> Option<Terrain> {
    match c {
        '.' => Some(Terrain::Grass),
        'M' => Some(Terrain::Mountain),
        'F' => Some(Terrain::Forest),
        'T' => Some(Terrain::Tree),
        'W' => Some(Terrain::Water),
        _ => None,
    }
}

impl MapFile {
    /// Everything that makes up the map in `world` right now. `setup` stays empty – only the caller knows it.
    pub fn from_world(world: &World, name: &str) -> MapFile {
        let n = world.map.size as i32;
        let terrain = (0..n).map(|y| (0..n).map(|x| letter(world.map.terrain(x, y))).collect()).collect();
        MapFile {
            version: VERSION,
            name: name.to_owned(),
            seed: world.seed,
            size: world.map.size,
            border: world.map.border,
            round: RoundData { days: world.days, gold: world.gold },
            constant_spawn: world.constant.clone(),
            waves: world.waves.clone(),
            setup: None,
            spawners: world
                .spawners
                .iter()
                .map(|s| SpawnerData { name: s.name.clone(), min: s.min, size: s.size, mode: s.mode })
                .collect(),
            buildings: world.buildings.iter().map(|b| BuildingData { kind: b.kind, tile: b.tile, turned: b.turned }).collect(),
            units: world.units.iter().map(|u| UnitData { kind: u.kind, pos: (u.pos.x, u.pos.y) }).collect(),
            zombies: world
                .zombies
                .iter()
                .map(|z| ZombieData { pos: (z.pos.x, z.pos.y), breed: z.breed, wander: world.is_wanderer(z) })
                .collect(),
            deposits: world.map.all_deposits().map(|(x, y, d)| DepositData { tile: (x, y), ore: d.ore, amount: d.amount }).collect(),
            terrain,
        }
    }

    /// A world from the file – at tick 0, as the round starts. `Err` with a reason if the file doesn't add up.
    pub fn to_world(&self) -> Result<World, String> {
        if self.version > VERSION {
            return Err(format!("made by a newer version of the game (format {})", self.version));
        }
        if self.terrain.len() != self.size {
            return Err(format!("{} rows of terrain, but the map is {} big", self.terrain.len(), self.size));
        }
        let mut terrain = Vec::with_capacity(self.size * self.size);
        for (y, row) in self.terrain.iter().enumerate() {
            let tiles: Option<Vec<Terrain>> = row.chars().map(from_letter).collect();
            match tiles {
                Some(tiles) if tiles.len() == self.size => terrain.extend(tiles),
                _ => return Err(format!("row {y} of the terrain is broken")),
            }
        }
        let mut map = Map::from_terrain(self.size, self.border, terrain);
        for d in &self.deposits {
            map.set_deposit(d.tile.0, d.tile.1, Some(Deposit { ore: d.ore, amount: d.amount }));
        }
        let mut world = World::empty(self.seed, map);
        world.days = self.round.days;
        world.gold = self.round.gold;
        world.constant = self.constant_spawn.clone();
        for s in &self.spawners {
            world.add_spawner(s.name.clone(), s.min, s.size, s.mode);
        }
        for b in &self.buildings {
            world.add_building_anywhere(b.kind, b.tile, b.turned);
        }
        for u in &self.units {
            world.add_unit(u.kind, vec2(u.pos.0, u.pos.1));
        }
        for z in &self.zombies {
            world.add_zombie(vec2(z.pos.0, z.pos.1), z.breed, z.wander);
        }
        world.waves = self.waves.clone();
        world.settle();
        Ok(world)
    }

    /// One line about the map: how it was made and how long its round is – for the editor's "Map" tab.
    pub fn info(&self) -> String {
        let made = match &self.setup {
            Some(setup) => format!("{} map, seed {}", setup.generator.name(), self.seed),
            None => format!("Map from seed {}", self.seed),
        };
        match self.round.days {
            0 => made,
            days => format!("{made} - {days} days"),
        }
    }

    /// Writes the file to `path` (the folder is made if it's missing).
    pub fn write(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string(self).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        fs::write(path, pretty(&text)).map_err(|e| e.to_string())
    }

    pub fn load(path: &Path) -> Result<MapFile, String> {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| format!("not a map file: {e}"))
    }
}

/// `serde_json` writes everything in one long line. This puts in line breaks where they help reading and changing
/// the file by hand: every field of the map on its own line, every wave, stage of the constant spawn, spawner, building
/// and unit too – and one row of terrain per line, so the map can be seen in the file. (The thousands of zombies and
/// ore tiles stay in one line each.)
///
/// It's safe because a line break between two parts of JSON is always allowed – and the patterns can only stand
/// between parts, never inside a text: inside a JSON string every `"` is written as `\"`, so `,"name":` with a plain
/// `"` can't be in there.
fn pretty(json: &str) -> String {
    let mut text = json.to_owned();
    // The fields of `MapFile` – `replacen(…, 1)`: only the first time, that's the field of the map (they come first, in
    // the order of the struct; spawners and the setup further down have fields with the same names).
    for key in ["name", "seed", "size", "border", "round", "constant_spawn", "waves", "setup", "spawners", "buildings", "units", "zombies", "deposits", "terrain"] {
        text = text.replacen(&format!(",\"{key}\":"), &format!(",\n\"{key}\":"), 1);
    }
    // One entry per line: waves start with "at", stages with "day", spawners with "name", buildings and units with "kind".
    for first in ["at", "day", "name", "kind"] {
        text = text.replace(&format!("{{\"{first}\":"), &format!("\n  {{\"{first}\":"));
    }
    // The terrain: rows only have the letters `. M F T W`, so between two rows there's always `","`.
    if let Some(start) = text.find("\"terrain\":[") {
        let rows = text.split_off(start);
        text += &rows.replace("\",\"", "\",\n\"").replacen("[\"", "[\n\"", 1);
    }
    text.replace("{\"version\"", "{\n\"version\"") + "\n"
}

/// `%APPDATA%\Last Bastion\maps`.
pub fn folder() -> Option<PathBuf> {
    settings::folder().map(|dir| dir.join("maps"))
}

/// Where the file of a map with this name goes.
pub fn path_for(name: &str) -> Option<PathBuf> {
    folder().map(|dir| dir.join(format!("{}.json", file_stem(name))))
}

/// A name no map in the folder has yet: "Wild 1", "Wild 2" … – for the file of a new round.
pub fn free_name(base: &str) -> String {
    (1..)
        .map(|n| format!("{base} {n}"))
        .find(|name| path_for(name).is_none_or(|path| !path.exists()))
        .unwrap_or_else(|| base.to_owned())
}

/// A map in the folder, for the lists on the home screens.
pub struct Entry {
    /// The file name without ".json".
    pub name: String,
    pub path: PathBuf,
    /// How long ago it was last changed – `None` if Windows doesn't say.
    pub age: Option<Duration>,
    /// Days of its round (0: no set length) and the edge of the playing field inside the border – `None` if the file
    /// can't be read.
    pub days: Option<u32>,
    pub field: Option<usize>,
}

/// Only what the list shows of a file. Everything else in it is skipped (serde ignores fields it doesn't know).
#[derive(Deserialize)]
struct Summary {
    size: usize,
    border: usize,
    #[serde(default)]
    round: RoundData,
}

/// All maps in the folder, the one changed last on top.
pub fn list() -> Vec<Entry> {
    let Some(dir) = folder() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let now = SystemTime::now();
    let mut maps: Vec<Entry> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| {
            let name = path.file_stem()?.to_string_lossy().into_owned();
            let age = fs::metadata(&path).and_then(|m| m.modified()).ok().and_then(|t| now.duration_since(t).ok());
            let summary: Option<Summary> = fs::read_to_string(&path).ok().and_then(|text| serde_json::from_str(&text).ok());
            let (days, field) = match summary {
                Some(s) => (Some(s.round.days), Some(s.size.saturating_sub(2 * s.border))),
                None => (None, None),
            };
            Some(Entry { name, path, age, days, field })
        })
        .collect();
    // Newest first; without an age at the end.
    maps.sort_by_key(|entry| entry.age.unwrap_or(Duration::MAX));
    maps
}

/// Opens the maps folder in Explorer (made first if it's missing).
pub fn open_folder() {
    if let Some(dir) = folder() {
        let _ = fs::create_dir_all(&dir);
        let _ = std::process::Command::new("explorer").arg(dir).spawn();
    }
}

/// A map name as a file name: letters, digits, space, `-` and `_` stay, everything else becomes `_`.
/// (Windows doesn't allow `\ / : * ? " < > |` in file names.)
pub fn file_stem(name: &str) -> String {
    let stem: String = name.trim().chars().map(|c| if c.is_alphanumeric() || " -_".contains(c) { c } else { '_' }).collect();
    if stem.is_empty() { "Untitled".to_owned() } else { stem }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Ore;
    use crate::sim::{Order, WaveFrom};

    /// Saved and loaded again, everything is still there: terrain, ore, spawners, buildings, units, zombies, waves –
    /// and the round with its constant spawn.
    #[test]
    fn a_map_comes_back_the_same() {
        let mut world = World::new(4);
        let c = world.map.size as i32 / 2;
        world.step(&[
            Order::PaintTerrain { terrain: Terrain::Water, tiles: vec![(c + 20, c + 20), (c + 21, c + 20)] },
            Order::PlaceBuilding { kind: BuildingKind::Wall, tile: (c + 6, c), turned: false, editor: true },
            Order::AddTimedWave(TimedWave { at: 1234, count: 300, from: WaveFrom::Sides(2), breed: Breed::Fat, final_wave: true }),
            Order::PaintOre { ore: Some(Ore::Coal), tiles: vec![(c + 3, c + 9)] },
            Order::SetDays { days: 25 },
            Order::SetGold { gold: 777 },
            Order::SetExtraPerDay { extra: 2.5 },
        ]);
        let mut file = MapFile::from_world(&world, "Test: map?");
        file.setup = Some(MapSetup::default());
        assert!(file.deposits.len() > 20, "the generated ore isn't saved");
        assert!(file.deposits.iter().any(|d| d.tile == (c + 3, c + 9) && d.ore == Ore::Coal), "the painted ore isn't saved");
        assert_eq!(file.round, RoundData { days: 25, gold: world.gold });
        let text = pretty(&serde_json::to_string(&file).unwrap());
        let back: MapFile = serde_json::from_str(&text).unwrap();
        assert_eq!(back, file);

        let loaded = back.to_world().unwrap();
        assert!(loaded.map.same_terrain(&world.map));
        assert_eq!(loaded.spawners.len(), world.spawners.len());
        assert_eq!(loaded.buildings.len(), world.buildings.len());
        assert_eq!(loaded.units.len(), world.units.len());
        assert_eq!(loaded.zombies.len(), world.zombies.len());
        assert_eq!(loaded.waves, world.waves);
        assert_eq!((loaded.days, loaded.gold), (25, world.gold));
        assert_eq!(loaded.constant, world.constant);
        let again = MapFile { setup: file.setup.clone(), ..MapFile::from_world(&loaded, "Test: map?") };
        assert_eq!(again, file, "saving the loaded map gives the same file");
        assert_eq!(file_stem("Test: map?"), "Test_ map_");
    }

    /// The file is readable by hand: the round in its own line, one row of terrain per line – the map is in there like
    /// a picture.
    #[test]
    fn the_file_shows_the_map_row_by_row() {
        let world = World::new(4);
        let file = MapFile::from_world(&world, "Rows \"quoted\", too");
        let text = pretty(&serde_json::to_string(&file).unwrap());
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.iter().any(|l| l.starts_with("\"round\":{\"days\":10")), "no line for the round");
        let rows = lines.iter().filter(|l| l.len() >= world.map.size && l.trim_matches(|c| c == '"' || c == ',' || c == ']' || c == '}').len() == world.map.size).count();
        assert_eq!(rows, world.map.size, "not one line per row of terrain");
        let back: MapFile = serde_json::from_str(&text).unwrap();
        assert_eq!(back.name, "Rows \"quoted\", too", "a name with quotes and commas got broken");
    }

    /// A file of version 1 (before 2026-10-17: no round, no constant spawn, waves without breed) still loads: no set
    /// length, the start gold and the constant spawn of `data\`, runners in its waves.
    #[test]
    fn old_files_still_load() {
        let old = r#"{"version":1,"name":"Old","seed":7,"size":2,"border":0,"terrain":[".M",".."],
            "spawners":[],"buildings":[],"units":[],"zombies":[],"waves":[{"at":100,"count":20,"from":"OneSpawner"}]}"#;
        let file: MapFile = serde_json::from_str(old).unwrap();
        assert_eq!(file.round, RoundData { days: 0, gold: data::get().colony.start_gold });
        assert_eq!(file.constant_spawn, data::get().spawning.constant);
        assert_eq!((file.waves[0].breed, file.waves[0].final_wave), (Breed::Runner, false));
        let world = file.to_world().unwrap();
        assert_eq!(world.map.terrain(1, 0), Terrain::Mountain);
    }

    /// A broken file gives a reason, not a crash.
    #[test]
    fn broken_files_are_refused() {
        let mut file = MapFile::from_world(&World::new(4), "x");
        file.terrain[3].push('?');
        assert!(file.to_world().is_err());
        file.terrain.pop();
        assert!(file.to_world().is_err());
    }

    /// Written to a file and read back – in a folder of its own under the temp folder, not in the real maps folder.
    #[test]
    fn write_and_load_a_file() {
        let dir = std::env::temp_dir().join(format!("last_bastion_test_{}", std::process::id()));
        let path = dir.join("A map.json");
        let file = MapFile::from_world(&World::new(5), "A map");
        file.write(&path).unwrap();
        assert_eq!(MapFile::load(&path).unwrap(), file);
        fs::remove_dir_all(&dir).unwrap();
    }
}
