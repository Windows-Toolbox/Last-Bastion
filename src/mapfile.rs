//! One game = one file, like in TAB (2026-10-17): every map is one JSON file under `%APPDATA%\Last Bastion\maps\`.
//! Starting a new round writes the file of its map there – the game plays from that file, "Restart" reads it anew,
//! and the map editor (`LastBastionEditor.exe`) opens it, changes it and saves it again. Maps made in the editor land
//! in the same folder. JSON like the settings (DESIGN.md 12.4, question 14): readable, and you can change it by hand.
//!
//! What's in it: the round (how many days, the gold at the start, the difficulty), the events (TAB's: the waves, the
//! stragglers, the final wave, the victory), the spawners, buildings, units, the zombies on the map, the houses of the villages
//! of doom and the loot, the ore deposits and the terrain (one letter per tile, one line of text per row – you can
//! almost see the map in the file). Not in it: what happens during a round (hit points, commands, the clock) – the file is the round as it starts. Saving a round half-way through
//! (DESIGN.md 11.5) comes later; then it goes into the same file.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use crate::data;
use crate::kinds::{BuildingKind, Resource, UnitKind};
use crate::map::{Deposit, Map, Ore, Terrain};
use crate::mapgen::MapSetup;
use crate::settings;
use crate::sim::{Axes, Breed, Event, LootKind, SpawnMode, WaveFrom, WaveGroup, World};

/// Which version of the file format – if it ever changes, old files can still be recognised.
/// 2 (2026-10-17): the round, the constant spawn, the setup, and waves with breed and final wave.
/// 3 (2026-10-20): dirt (`D`), the villages of doom, the loot – an older game can't read that, so it refuses the file.
/// 4: waves with groups (several breeds, chosen spawners), spawners with their number and axes, custom levels, the
/// playing field anywhere on the map.
/// 5: TAB's events instead of the timeline of waves and the constant spawn; days of 60 seconds.
const VERSION: u32 = 5;

/// The fields in the order they are in the file: the short ones on top, the terrain at the end.
/// `#[serde(default …)]`: a field a file from version 1 doesn't have yet gets this value instead of an error.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MapFile {
    pub version: u32,
    pub name: String,
    /// Saved in the editor: a custom level – the game's "New game" lists it under "Custom levels" (like TAB's).
    /// The file every new round writes for itself isn't one. `default`: files before format 4 aren't.
    #[serde(default)]
    pub custom: bool,
    pub seed: u64,
    pub size: usize,
    pub border: usize,
    /// The playing field (the world border) – files before format 4: in the middle, `border` inside the edge.
    #[serde(default)]
    pub play_area: Option<PlayArea>,
    #[serde(default)]
    pub round: RoundData,
    /// The events of this map (`sim/events.rs`).
    #[serde(default)]
    pub events: Vec<Event>,
    /// Files before format 5: the timeline of waves – read, turned into events (`old_events`), never written. (Their
    /// constant spawn is left out: TAB's stragglers are events now.)
    #[serde(default, skip_serializing)]
    pub waves: Vec<OldWave>,
    /// How the map was generated – for the info line and "New map" (same setup, new seed). `None`: not known (files
    /// from before 2026-10-17).
    #[serde(default)]
    pub setup: Option<MapSetup>,
    pub spawners: Vec<SpawnerData>,
    pub buildings: Vec<BuildingData>,
    pub units: Vec<UnitData>,
    pub zombies: Vec<ZombieData>,
    /// The houses of the villages of doom and the loot – files from before 2026-10-20 have none.
    #[serde(default)]
    pub doom: Vec<DoomData>,
    #[serde(default)]
    pub loot: Vec<LootData>,
    /// Every tile with ore on it. `default`: files from before 2026-10-17 have none.
    #[serde(default)]
    pub deposits: Vec<DepositData>,
    /// One string per row, one letter per tile (`letter`).
    pub terrain: Vec<String>,
}

/// The round on this map.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(default)]
pub struct RoundData {
    /// How many days the round lasts – its events are planned for them. 0: no set length.
    pub days: u32,
    /// The colony's gold at the start …
    pub gold: u32,
    /// … and its wood.
    pub wood: u32,
    /// The difficulty it was made with (`data\game_difficulties.json`): its zombies, waves and start are in this file
    /// already – while the round runs, it says how close every breed may come (`World::keep_out`). Not in the list
    /// (any more), or an older file: the default one.
    pub difficulty: String,
}

impl Default for RoundData {
    /// What a file without a round gets: no set length, the start gold and wood of the default difficulty.
    fn default() -> RoundData {
        let difficulty = data::get().difficulty("");
        RoundData { days: 0, gold: difficulty.start_gold, wood: difficulty.start_wood, difficulty: String::new() }
    }
}

/// The zombies of a map, awake and asleep (`sim/sleep.rs`) – the sleeping ones as wanderers, they lie down asleep again
/// when the map is loaded. In order of where they stand, row by row: the same file whoever was awake or asleep when it
/// was saved.
fn zombie_data(world: &World) -> Vec<ZombieData> {
    let awake = world.zombies.iter().map(|z| ZombieData { pos: (z.pos.x, z.pos.y), breed: z.breed, wander: world.is_wanderer(z) });
    let asleep = world.sleep.iter().map(|s| ZombieData { pos: (s.pos.x, s.pos.y), breed: s.breed, wander: true });
    let mut zombies: Vec<ZombieData> = awake.chain(asleep).collect();
    zombies.sort_by(|a, b| a.pos.1.total_cmp(&b.pos.1).then(a.pos.0.total_cmp(&b.pos.0)));
    zombies
}

/// A wave as files before format 5 have it. `untagged`: the file doesn't say which of the two it is – serde tries them
/// in order, and the first one whose fields fit is it.
#[derive(Deserialize, Debug, PartialEq, Clone)]
#[serde(untagged)]
pub enum OldWave {
    /// Format 4: with groups. `at` is a tick of the old days of 3 minutes.
    Groups {
        at: u64,
        #[serde(default)]
        final_wave: bool,
        #[serde(default)]
        reveal: bool,
        groups: Vec<WaveGroup>,
    },
    /// Before: one breed per wave – several of them at the same time came out of the same random sides together, so
    /// they become one event. Files from before 2026-10-17 have no breed (their waves were runners).
    Single {
        at: u64,
        count: u32,
        from: WaveFrom,
        #[serde(default = "runner")]
        breed: Breed,
        #[serde(default)]
        final_wave: bool,
        #[serde(default)]
        reveal: bool,
    },
}

/// What an old wave without a breed sent – for `#[serde(default = "runner")]`, which wants a function.
fn runner() -> Breed {
    Breed::Runner
}

/// An hour of the old days of 3 minutes, in ticks – the old waves' `at` counts in them.
const OLD_HOUR_TICKS: u64 = 150;

/// The old waves as events: each at its day and hour (the days are shorter now, the day stays), announced 8 hours
/// before like TAB's swarms. A final wave wins the round once it's dead, as before. Waves at the same time (and both
/// final or not) are one event, a group per kind of place – they came out of the same random spawners.
fn old_events(waves: &[OldWave]) -> Vec<Event> {
    let mut events: Vec<Event> = Vec::new();
    for wave in waves {
        let (at, final_wave, reveal, groups) = match wave.clone() {
            OldWave::Groups { at, final_wave, reveal, groups } => (at, final_wave, reveal, groups),
            OldWave::Single { at, count, from, breed, final_wave, reveal } => {
                (at, final_wave, reveal, vec![WaveGroup { from, breeds: vec![crate::sim::BreedCount::new(count, breed)] }])
            }
        };
        let hour = (at / OLD_HOUR_TICKS) as u32;
        let Some(into) = events.iter_mut().find(|e| e.hour == hour && e.final_wave == final_wave) else {
            let name = if final_wave { "Final wave".to_owned() } else { format!("Wave {}", events.len() + 1) };
            let mut event = Event::new(&name, hour, WaveFrom::OneSpawner, Breed::Runner, 0);
            (event.groups, event.final_wave, event.reveal, event.won_when_dead, event.notify_hours) =
                (groups, final_wave, reveal, final_wave, Some(8));
            events.push(event);
            continue;
        };
        into.reveal |= reveal;
        for group in groups {
            match into.groups.iter_mut().find(|g| g.from == group.from) {
                Some(same) => same.breeds.extend(group.breeds),
                None => into.groups.push(group),
            }
        }
    }
    events
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DepositData {
    pub tile: (i32, i32),
    pub ore: Ore,
    pub amount: u32,
}

/// Where a new map's playing field lies (`Map::play_area_of`) – for files from before format 4.
fn default_play(size: usize, border: usize) -> PlayArea {
    let region = Map::play_area_of(size, border);
    let (min, extent) = (region.min.round(), (region.max - region.min).round());
    PlayArea { min: (min.x as i32, min.y as i32), size: (extent.x as i32, extent.y as i32) }
}

/// Where the playing field lies: its top left corner and its size, in whole steps of the screen axes (`map::to_frame`) –
/// like a straight spawner (`Map::set_play_area`).
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
pub struct PlayArea {
    pub min: (i32, i32),
    pub size: (i32, i32),
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SpawnerData {
    pub name: String,
    /// Its number – the waves name their spawners by it. 0 in files before format 4: numbered in order (`upgrade`).
    #[serde(default)]
    pub id: u32,
    pub min: (i32, i32),
    pub size: (i32, i32),
    /// Straight on screen or slanted like the tiles – files before format 4: straight.
    #[serde(default)]
    pub axes: Axes,
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

/// A house of a village of doom: its top left tile, width and depth, how many zombies it may still let out, and how
/// big its batches are besides its size (`DoomHouse::factor` – files from before TAB's houses have none: 1).
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DoomData {
    pub house: (i32, i32),
    pub size: (i32, i32),
    pub zombies: u32,
    #[serde(default = "one")]
    pub factor: f32,
}

/// 1 – for `factor`, when a file has none.
fn one() -> f32 {
    1.0
}

/// A pile of loot: what, where, how much, and how many zombies lie hidden around it.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct LootData {
    pub loot: LootKind,
    pub pos: (f32, f32),
    pub amount: u32,
    pub guards: u32,
}

/// The letter of a terrain in the file.
fn letter(terrain: Terrain) -> char {
    match terrain {
        Terrain::Grass => '.',
        Terrain::Dirt => 'D',
        Terrain::Mountain => 'M',
        Terrain::Forest => 'F',
        Terrain::Water => 'W',
    }
}

fn from_letter(c: char) -> Option<Terrain> {
    match c {
        '.' => Some(Terrain::Grass),
        'D' => Some(Terrain::Dirt),
        'M' => Some(Terrain::Mountain),
        // `T` was a single tree in files before 2026-10-24 – now that's simply forest.
        'F' | 'T' => Some(Terrain::Forest),
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
            custom: false,
            seed: world.seed,
            size: world.map.size,
            border: world.map.border,
            play_area: Some({
                let (min, size) = world.map.play_steps();
                PlayArea { min, size }
            }),
            round: RoundData { days: world.days, gold: world.gold, wood: world.stock.get(Resource::Wood), difficulty: world.difficulty.clone() },
            events: world.events.clone(),
            waves: Vec::new(),
            setup: None,
            spawners: world
                .spawners
                .iter()
                .map(|s| SpawnerData { name: s.name.clone(), id: s.id, min: s.min, size: s.size, axes: s.axes, mode: s.mode })
                .collect(),
            buildings: world.buildings.iter().map(|b| BuildingData { kind: b.kind, tile: b.tile, turned: b.turned }).collect(),
            units: world.units.iter().map(|u| UnitData { kind: u.kind, pos: (u.pos.x, u.pos.y) }).collect(),
            zombies: zombie_data(world),
            doom: world.doom.iter().map(|d| DoomData { house: d.tile, size: d.size, zombies: d.inside, factor: d.factor }).collect(),
            loot: world
                .loot
                .iter()
                .map(|l| LootData { loot: l.kind, pos: (l.pos.x, l.pos.y), amount: l.amount, guards: l.guards })
                .collect(),
            deposits: world.map.all_deposits().map(|(x, y, d)| DepositData { tile: (x, y), ore: d.ore, amount: d.amount }).collect(),
            terrain,
        }
    }

    /// A world from the file – at tick 0, as the round starts. `Err` with a reason if the file doesn't add up.
    pub fn to_world(&self) -> Result<World, String> {
        let mut world = World::empty(self.seed, self.map()?);
        world.set_difficulty(data::get().difficulty(&self.round.difficulty));
        world.days = self.round.days;
        world.gold = self.round.gold;
        world.stock.set(Resource::Wood, self.round.wood);
        for s in &self.spawners {
            world.add_spawner(s.id, s.name.clone(), s.min, s.size, s.axes, s.mode);
        }
        for b in &self.buildings {
            world.add_building_anywhere(b.kind, b.tile, b.turned);
        }
        // The base is where the command center stands – on a map made in the editor it may stand anywhere, a new map has
        // it in the middle (`World::empty`, the same point). What the zombies keep away from (`keep_out`) and where waves
        // go without a command center.
        if let Some(cc) = world.buildings.iter().find(|b| b.kind == BuildingKind::CommandCenter) {
            world.base = cc.center();
        }
        for u in &self.units {
            world.add_unit(u.kind, vec2(u.pos.0, u.pos.1));
        }
        // Wanderers lie down asleep – the ones near your eyes wake when the world settles (`World::settle`).
        for z in &self.zombies {
            let pos = vec2(z.pos.0, z.pos.1);
            if z.wander {
                world.add_sleeper(pos, z.breed);
            } else {
                world.add_zombie(pos, z.breed, false);
            }
        }
        for d in &self.doom {
            world.add_doom(d.house, d.size, d.zombies, d.factor);
        }
        for l in &self.loot {
            world.add_loot(vec2(l.pos.0, l.pos.1), l.loot, l.amount, l.guards);
        }
        world.events = if self.version < 5 && self.events.is_empty() { old_events(&self.waves) } else { self.events.clone() };
        world.roll_events();
        world.settle();
        Ok(world)
    }

    /// Only the map – terrain and ore, without anything on it: for `to_world`, and for the preview of a custom level
    /// (`home.rs`), which needs nothing else. `Err` with a reason if the file doesn't add up.
    pub fn map(&self) -> Result<Map, String> {
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
        if let Some(play) = self.play_area {
            map.set_play_area(play.min, play.size);
        }
        for d in &self.deposits {
            map.set_deposit(d.tile.0, d.tile.1, Some(Deposit { ore: d.ore, amount: d.amount }));
        }
        Ok(map)
    }

    /// Every tile one of its spawners covers – for the preview.
    pub fn spawner_tiles(&self) -> Vec<(i32, i32)> {
        self.spawners.iter().flat_map(|s| crate::sim::spawner_tiles(s.axes, s.min, s.size)).collect()
    }

    /// A file of an older format as it would be written now (format 5): its waves become events (`old_events`), its
    /// spawners get their numbers – the same ones `World::add_spawner` gives them. So an old map opened in the editor
    /// doesn't count as changed right away.
    fn upgrade(&mut self) {
        if self.version >= VERSION {
            return;
        }
        if self.events.is_empty() {
            self.events = old_events(&std::mem::take(&mut self.waves));
        }
        let mut last = 0;
        let mut taken = Vec::new();
        for s in &mut self.spawners {
            if s.id == 0 || taken.contains(&s.id) {
                s.id = last + 1;
            }
            last = last.max(s.id);
            taken.push(s.id);
        }
        self.version = VERSION;
    }

    /// The same map on a grid with an edge of `size` tiles (`Map::size`, even – the editor's "Grid"): it grows or shrinks
    /// by the same number of tiles on every side, everything on it stays where it is – the playing field and the
    /// spawners too; drag them where you want them. New ground is grass, what doesn't fit any more goes (the playing field
    /// gets smaller then, `Map::set_play_area`).
    pub fn resized(&self, size: usize) -> MapFile {
        let (old, new) = (self.size as i32, size as i32);
        // On every side this many tiles more (or fewer, if it's negative) – both sizes are even.
        let d = (new - old) / 2;
        let full = Map::full_area_of(size);
        let on_map = |x: i32, y: i32| full.contains_tile(x, y);
        let moved = |p: (f32, f32)| (p.0 + d as f32, p.1 + d as f32);
        let point_on_map = |p: (f32, f32)| full.contains(Vec2::new(p.0, p.1));
        let all_on_map = |tile: (i32, i32), w: i32, h: i32| (0..h).all(|y| (0..w).all(|x| on_map(tile.0 + x, tile.1 + y)));

        // Only what was on the old map comes along: the grid's corners beyond it were never part of it (`Map::index`),
        // whatever the generator left there – grown, they'd be a wall through the new ring.
        let was_on_map = Map::full_area_of(self.size);
        let terrain = (0..new)
            .map(|y| {
                let row: Vec<char> = self.terrain.get((y - d) as usize).map_or(Vec::new(), |r| r.chars().collect());
                let old_letter = |x: i32| was_on_map.contains_tile(x - d, y - d).then(|| row.get((x - d) as usize).copied()).flatten();
                (0..new).map(|x| old_letter(x).unwrap_or('.')).collect()
            })
            .collect();
        let mut file = self.clone();
        file.size = size;
        file.terrain = terrain;
        let shift = |t: (i32, i32)| (t.0 + d, t.1 + d);
        file.deposits = self.deposits.iter().map(|o| DepositData { tile: shift(o.tile), ..o.clone() }).filter(|o| on_map(o.tile.0, o.tile.1)).collect();
        file.buildings = self
            .buildings
            .iter()
            .map(|b| BuildingData { tile: shift(b.tile), ..b.clone() })
            .filter(|b| {
                let (w, h) = b.kind.footprint(b.turned);
                all_on_map(b.tile, w, h)
            })
            .collect();
        file.doom = self.doom.iter().map(|h| DoomData { house: shift(h.house), ..h.clone() }).filter(|h| all_on_map(h.house, h.size.0, h.size.1)).collect();
        file.units = self.units.iter().map(|u| UnitData { pos: moved(u.pos), ..u.clone() }).filter(|u| point_on_map(u.pos)).collect();
        file.zombies = self.zombies.iter().map(|z| ZombieData { pos: moved(z.pos), ..z.clone() }).filter(|z| point_on_map(z.pos)).collect();
        file.loot = self.loot.iter().map(|l| LootData { pos: moved(l.pos), ..l.clone() }).filter(|l| point_on_map(l.pos)).collect();
        // In the screen axes (`map::to_frame`: across x − y, down x + y) d tiles both ways are 0 across and 2 · d down.
        let down = |min: (i32, i32)| (min.0, min.1 + 2 * d);
        file.spawners = self
            .spawners
            .iter()
            .map(|s| SpawnerData { min: if s.axes == Axes::Screen { down(s.min) } else { shift(s.min) }, ..s.clone() })
            .collect();
        let play = self.play_area.unwrap_or_else(|| default_play(self.size, self.border));
        file.play_area = Some(PlayArea { min: down(play.min), ..play });
        file
    }

    /// One line about the map: how it was made, how long its round is and how hard – for the editor's "Map" tab.
    pub fn info(&self) -> String {
        let made = match &self.setup {
            Some(setup) if !setup.map.is_empty() => format!("{}, seed {}", setup.map, self.seed),
            Some(setup) => format!("{} map, seed {}", setup.generator.name(), self.seed),
            None => format!("Map from seed {}", self.seed),
        };
        let difficulty = &data::get().difficulty(&self.round.difficulty).name;
        match self.round.days {
            0 => format!("{made} - {difficulty}"),
            days => format!("{made} - {days} days - {difficulty}"),
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
        let mut file: MapFile = serde_json::from_str(&text).map_err(|e| format!("not a map file: {e}"))?;
        file.upgrade();
        Ok(file)
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
    for key in ["name", "custom", "seed", "size", "border", "play_area", "round", "events", "setup", "spawners", "buildings", "units", "zombies", "doom", "loot", "deposits", "terrain"] {
        text = text.replacen(&format!(",\"{key}\":"), &format!(",\n\"{key}\":"), 1);
    }
    // One entry per line: events and spawners start with "name", the groups of events with "from", buildings and units
    // with "kind", houses of doom with "house", piles of loot with "loot".
    for first in ["name", "from", "kind", "house", "loot"] {
        text = text.replace(&format!("{{\"{first}\":"), &format!("\n  {{\"{first}\":"));
    }
    // The terrain: rows only have the letters `. D M F T W`, so between two rows there's always `","`.
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
    /// Days of its round (0: no set length) and how big the playing field inside the border is (the edge of a square
    /// with as many tiles) – `None` if the file can't be read.
    pub days: Option<u32>,
    pub field: Option<usize>,
    /// Saved in the editor (`MapFile::custom`) – and how hard: the name of its difficulty.
    pub custom: bool,
    pub difficulty: String,
}

/// Only what the list shows of a file. Everything else in it is skipped (serde ignores fields it doesn't know).
#[derive(Deserialize)]
struct Summary {
    size: usize,
    border: usize,
    #[serde(default)]
    play_area: Option<PlayArea>,
    #[serde(default)]
    round: RoundData,
    #[serde(default)]
    custom: bool,
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
            let (days, field, custom, difficulty) = match summary {
                Some(s) => {
                    // As many tiles as the playing field covers – one step across and down is half a tile.
                    let play = s.play_area.unwrap_or_else(|| default_play(s.size, s.border));
                    let field = ((play.size.0 * play.size.1) as f32 / 2.0).sqrt().round() as usize;
                    let difficulty = data::get().difficulty(&s.round.difficulty).name.clone();
                    (Some(s.round.days), Some(field), s.custom, difficulty)
                }
                None => (None, None, false, String::new()),
            };
            Some(Entry { name, path, age, days, field, custom, difficulty })
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
    use crate::sim::{BreedCount, Order};

    /// Saved and loaded again, everything is still there: terrain, ore, spawners (a slanted one too), buildings, units,
    /// zombies, events (groups out of chosen spawners too) – and the round.
    #[test]
    fn a_map_comes_back_the_same() {
        let mut world = World::new(4);
        let c = world.map.size as i32 / 2;
        let (first, last) = (world.spawners[0].id, world.spawners[11].id);
        let mut event = Event { final_wave: true, reveal: true, every_hours: 30, times: 3, growth: 1.5, notify_hours: Some(4), ..Event::new("Big", 123, WaveFrom::Sides(2), Breed::Fat, 300) };
        let zombies = vec![BreedCount { count: 50, up_to: 60, breed: Breed::Harpy }, BreedCount::new(7, Breed::Giant)];
        event.groups.push(WaveGroup { from: WaveFrom::Spawners(vec![first, last]), breeds: zombies });
        world.step(&[
            Order::PaintTerrain { terrain: Terrain::Water, tiles: vec![(c + 20, c + 20), (c + 21, c + 20)] },
            Order::PlaceBuilding { kind: BuildingKind::Wall, tile: (c + 6, c), turned: false, editor: true },
            Order::AddEvent(event),
            Order::RemoveSpawner { spawner: world.spawners[3].id },
            Order::SetSpawnerAxes { spawner: last, axes: Axes::Tiles },
            Order::PaintOre { ore: Some(Ore::Coal), tiles: vec![(c + 3, c + 9)] },
            Order::SetDays { days: 25 },
            Order::SetGold { gold: 777 },
            Order::SetWood { wood: 55 },
        ]);
        let mut file = MapFile::from_world(&world, "Test: map?");
        file.setup = Some(MapSetup::default());
        assert!(file.deposits.len() > 20, "the generated ore isn't saved");
        assert!(file.deposits.iter().any(|d| d.tile == (c + 3, c + 9) && d.ore == Ore::Coal), "the painted ore isn't saved");
        assert_eq!(file.round, RoundData { days: 25, gold: world.gold, wood: 55, difficulty: world.difficulty.clone() });
        let text = pretty(&serde_json::to_string(&file).unwrap());
        let back: MapFile = serde_json::from_str(&text).unwrap();
        assert_eq!(back, file);

        let loaded = back.to_world().unwrap();
        assert!(loaded.map.same_terrain(&world.map));
        assert_eq!(loaded.spawners.len(), world.spawners.len());
        // The numbers stay – the waves name their spawners by them – also with a gap where one was removed.
        let numbered = |w: &World| w.spawners.iter().map(|s| (s.id, s.axes, s.min, s.size)).collect::<Vec<_>>();
        assert_eq!(numbered(&loaded), numbered(&world));
        assert_eq!(loaded.buildings.len(), world.buildings.len());
        assert_eq!(loaded.units.len(), world.units.len());
        assert_eq!(loaded.zombies.len() + loaded.sleep.len(), world.zombies.len() + world.sleep.len());
        assert_eq!(loaded.events, world.events);
        assert_eq!((loaded.days, loaded.gold, loaded.stock.get(Resource::Wood)), (25, world.gold, 55));
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
        assert!(lines.iter().any(|l| l.starts_with("\"round\":{\"days\":")), "no line for the round");
        // Every field of the map in a line of its own – also those whose names are used further up (in the events).
        for key in ["events", "spawners", "units", "zombies", "doom", "terrain"] {
            assert!(lines.iter().any(|l| l.starts_with(&format!("\"{key}\":"))), "no line for {key}");
        }
        let rows = lines.iter().filter(|l| l.len() >= world.map.size && l.trim_matches(|c| c == '"' || c == ',' || c == ']' || c == '}').len() == world.map.size).count();
        assert_eq!(rows, world.map.size, "not one line per row of terrain");
        let back: MapFile = serde_json::from_str(&text).unwrap();
        assert_eq!(back.name, "Rows \"quoted\", too", "a name with quotes and commas got broken");
    }

    /// A file of version 1 (before 2026-10-17: no round, no constant spawn, waves without breed) still loads: no set
    /// length, the start gold and wood of the default difficulty, runners in its waves – an event now.
    #[test]
    fn old_files_still_load() {
        let old = r#"{"version":1,"name":"Old","seed":7,"size":2,"border":0,"terrain":[".M",".."],
            "spawners":[],"buildings":[],"units":[],"zombies":[],"waves":[{"at":100,"count":20,"from":"OneSpawner"}]}"#;
        let file: MapFile = serde_json::from_str(old).unwrap();
        let normal = data::get().difficulty("");
        assert_eq!(file.round, RoundData { days: 0, gold: normal.start_gold, wood: normal.start_wood, difficulty: String::new() });
        let world = file.to_world().unwrap();
        assert_eq!(world.events.len(), 1);
        assert_eq!(world.events[0].groups[0], WaveGroup { from: WaveFrom::OneSpawner, breeds: vec![BreedCount::new(20, Breed::Runner)] });
        assert_eq!(world.map.terrain(1, 0), Terrain::Mountain);
        assert_eq!(world.keep_out, normal.keep_out, "an old file doesn't get the default difficulty");
    }

    /// A file of format 3 had one breed per wave; those at the same time and from the same kind of place came out of the
    /// same random spawners. Now they're one event with one group per kind of place, at the same day and hour – and the
    /// spawners get their numbers in order – the same as when it's loaded, so an old map opened in the editor doesn't
    /// count as changed. A spawner of the old constant spawn sends nothing by itself now.
    #[test]
    fn old_waves_become_groups() {
        let old = r#"{"version":3,"name":"Old","seed":7,"size":2,"border":0,"terrain":["..",".."],
            "spawners":[{"name":"A","min":[0,1],"size":[1,1],"mode":"Constant"},{"name":"B","min":[0,1],"size":[1,1],"mode":"Off"}],
            "buildings":[],"units":[],"zombies":[],"waves":[
            {"at":100,"count":20,"from":{"Sides":1},"breed":"Runner"},
            {"at":100,"count":5,"from":{"Sides":1},"breed":"Armored"},
            {"at":100,"count":3,"from":"OneSpawner","breed":"Fat"},
            {"at":900,"count":50,"from":{"Sides":4},"breed":"Runner","final_wave":true,"reveal":true}]}"#;
        let mut file: MapFile = serde_json::from_str(old).unwrap();
        let world = file.to_world().unwrap();
        file.upgrade();
        assert_eq!(file.version, VERSION);
        assert_eq!(file.events, world.events, "loaded and upgraded give other events");
        assert_eq!(file.events.len(), 2);
        let first = &file.events[0];
        assert_eq!(first.hour, 0, "tick 100 of the old days is hour 0");
        assert_eq!(first.groups.len(), 2, "the runners and riots aren't one group");
        assert_eq!(first.groups[0].breeds, vec![BreedCount::new(20, Breed::Runner), BreedCount::new(5, Breed::Armored)]);
        assert_eq!(first.groups[1].from, WaveFrom::OneSpawner);
        let last = &file.events[1];
        assert!(last.final_wave && last.reveal && last.won_when_dead && last.hour == 6);
        assert_eq!(world.spawners[0].mode, SpawnMode::Off);
        assert_eq!(file.spawners.iter().map(|s| s.id).collect::<Vec<_>>(), world.spawners.iter().map(|s| s.id).collect::<Vec<_>>());
    }

    /// A round from its file starts with the command center's gold as the income – nobody has paid yet, but it will at
    /// the first payout. Before, the HUD showed only the upkeep there.
    #[test]
    fn the_income_shows_before_the_first_payout() {
        let world = MapFile::from_world(&World::new(4), "x").to_world().unwrap();
        assert_eq!(world.tick, 0);
        assert_eq!(world.income(), BuildingKind::CommandCenter.stats().gold);
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
