//! Saved maps (the editor's "Save map", the home screen's "Custom maps") – like TAB's map projects, but one plain
//! JSON file per map under `%APPDATA%\Last Bastion\maps\`. JSON like the settings (DESIGN.md 12.4, question 14):
//! readable, and you can change it by hand.
//!
//! What's in it: the terrain (one letter per tile, one line of text per row – you can almost see the map in the
//! file), the spawners, buildings, units, the zombies placed in the editor, the timeline of waves. Not in it: what
//! happens during a round (hit points, commands, the tick) – that would be a save game (DESIGN.md 11.5, later).

use std::fs;
use std::path::{Path, PathBuf};

use macroquad::math::vec2;
use serde::{Deserialize, Serialize};

use crate::kinds::{BuildingKind, UnitKind};
use crate::map::{Map, Terrain};
use crate::settings;
use crate::sim::{Breed, SpawnMode, TimedWave, World};

/// Which version of the file format – if it ever changes, old files can still be recognised.
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct MapFile {
    pub version: u32,
    pub name: String,
    pub seed: u64,
    pub size: usize,
    pub border: usize,
    /// One string per row, one letter per tile (`letter`).
    pub terrain: Vec<String>,
    pub spawners: Vec<SpawnerData>,
    pub buildings: Vec<BuildingData>,
    pub units: Vec<UnitData>,
    pub zombies: Vec<ZombieData>,
    pub waves: Vec<TimedWave>,
    pub respawn: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct SpawnerData {
    pub name: String,
    pub min: (i32, i32),
    pub size: (i32, i32),
    pub mode: SpawnMode,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct BuildingData {
    pub kind: BuildingKind,
    pub tile: (i32, i32),
    pub turned: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct UnitData {
    pub kind: UnitKind,
    pub pos: (f32, f32),
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
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
    /// Everything that makes up the map in `world` right now.
    pub fn from_world(world: &World, name: &str) -> MapFile {
        let n = world.map.size as i32;
        let terrain = (0..n).map(|y| (0..n).map(|x| letter(world.map.terrain(x, y))).collect()).collect();
        MapFile {
            version: VERSION,
            name: name.to_owned(),
            seed: world.seed,
            size: world.map.size,
            border: world.map.border,
            terrain,
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
            waves: world.waves.clone(),
            respawn: world.respawn,
        }
    }

    /// A world from the file – at tick 0, as the editor left it. `Err` with a reason if the file doesn't add up.
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
        let mut world = World::empty(self.seed, Map::from_terrain(self.size, self.border, terrain));
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
        world.respawn = self.respawn;
        world.settle();
        Ok(world)
    }

    /// Writes the file into the maps folder, named after the map. Returns where it went.
    pub fn save(&self) -> Result<PathBuf, String> {
        let dir = folder().ok_or("Windows doesn't say where APPDATA is")?;
        let path = dir.join(format!("{}.json", file_stem(&self.name)));
        let text = serde_json::to_string(self).map_err(|e| e.to_string())?;
        // `serde_json` writes everything in one line; one row of terrain per line is nicer to look at.
        let text = text.replace("\",\"", "\",\n\"");
        fs::create_dir_all(&dir).and_then(|_| fs::write(&path, text)).map_err(|e| e.to_string())?;
        Ok(path)
    }

    pub fn load(path: &Path) -> Result<MapFile, String> {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| format!("not a map file: {e}"))
    }
}

/// `%APPDATA%\Last Bastion\maps`.
pub fn folder() -> Option<PathBuf> {
    settings::folder().map(|dir| dir.join("maps"))
}

/// All saved maps: (name of the file without ".json", path), sorted by name.
pub fn list() -> Vec<(String, PathBuf)> {
    let Some(dir) = folder() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut maps: Vec<(String, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|p| Some((p.file_stem()?.to_string_lossy().into_owned(), p)))
        .collect();
    maps.sort();
    maps
}

/// A map name as a file name: letters, digits, space, `-` and `_` stay, everything else becomes `_`.
/// (Windows doesn't allow `\ / : * ? " < > |` in file names.)
fn file_stem(name: &str) -> String {
    let stem: String = name.trim().chars().map(|c| if c.is_alphanumeric() || " -_".contains(c) { c } else { '_' }).collect();
    if stem.is_empty() { "Untitled".to_owned() } else { stem }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{Order, WaveFrom};

    /// Saved and loaded again, everything is still there: terrain, spawners, buildings, units, zombies, waves.
    #[test]
    fn a_map_comes_back_the_same() {
        let mut world = World::new(4);
        let c = world.map.size as i32 / 2;
        world.step(&[
            Order::PaintTerrain { terrain: Terrain::Water, tiles: vec![(c + 20, c + 20), (c + 21, c + 20)] },
            Order::PlaceBuilding { kind: BuildingKind::Wall, tile: (c + 6, c), turned: false, editor: true },
            Order::AddTimedWave(TimedWave { at: 1234, count: 300, from: WaveFrom::Sides(2) }),
            Order::SetRespawn(false),
        ]);
        let file = MapFile::from_world(&world, "Test: map?");
        let text = serde_json::to_string(&file).unwrap();
        let back: MapFile = serde_json::from_str(&text).unwrap();
        assert_eq!(back, file);

        let loaded = back.to_world().unwrap();
        assert!(loaded.map.same_terrain(&world.map));
        assert_eq!(loaded.spawners.len(), world.spawners.len());
        assert_eq!(loaded.buildings.len(), world.buildings.len());
        assert_eq!(loaded.units.len(), world.units.len());
        assert_eq!(loaded.zombies.len(), world.zombies.len());
        assert_eq!(loaded.waves, world.waves);
        assert!(!loaded.respawn);
        assert_eq!(MapFile::from_world(&loaded, "Test: map?"), file, "saving the loaded map gives the same file");
        assert_eq!(file_stem("Test: map?"), "Test_ map_");
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
}
