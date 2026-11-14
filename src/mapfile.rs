//! One game = one file, like in TAB (2026-10-17): every map is one JSON file under `%APPDATA%\Last Bastion\maps\`.
//! Starting a new round writes the file of its map there – the game plays from that file, "Restart" reads it anew,
//! and the map editor (`LastBastionEditor.exe`) opens it, changes it and saves it again. Maps made in the editor land
//! in the same folder. JSON like the settings (DESIGN.md 12.4, question 14): readable, and you can change it by hand.
//!
//! What's in it: the round (how many days, the gold at the start, the difficulty), the events (TAB's: the waves, the
//! stragglers, the final wave, the victory), the spawners, buildings, units, the zombies on the map, the houses of the villages
//! of doom and the loot, the named regions and points, the ore deposits and the terrain (one letter per tile, one line of text per row – you can
//! almost see the map in the file). Not in it: what happens during a round (hit points, commands, the clock) – the file is the round as it starts. Saving a round half-way through
//! (DESIGN.md 11.5) comes later; then it goes into the same file.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use crate::data::{self, Modifiers};
use crate::kinds::{BuildingKind, Resource, UnitKind};
use crate::map::{BigRock, Deposit, Map, Ore, Terrain};
use crate::mapgen::MapSetup;
use crate::settings;
use crate::sim::{
    Axes, Breed, Building, DoomHouse, Event, Loot, LootKind, Neutral, Ornament, Sleeper, SpawnMode, Spawner, Story, Tag,
    Thing, ThingRef, Unit, WaveFrom, WaveGroup, World, Zombie, Spot, Zone,
};

/// Which version of the file format – if it ever changes, old files can still be recognised.
/// 2 (2026-10-17): the round, the constant spawn, the setup, and waves with breed and final wave.
/// 3 (2026-10-20): dirt (`D`), the villages of doom, the loot – an older game can't read that, so it refuses the file.
/// 4: waves with groups (several breeds, chosen spawners), spawners with their number and axes, custom levels, the
/// playing field anywhere on the map.
/// 5: TAB's events instead of the timeline of waves and the constant spawn; days of 60 seconds.
/// 6: named regions and points (`sim/regions.rs`).
/// 7: a wave's group can come out of regions (`WaveFrom::Zones`) – an older game can't read that.
/// 8: triggers and variables (`sim/story.rs`) instead of scripts on the start, the end, the events, the neutral things
/// and the command center's button, and counters – `triggers_from_scripts` turns an older file around when it's read.
/// 9: a small typed language (`sim/expr.rs`): steps of a script have a condition and code, triggers have expressions
/// as conditions, variables can be yes/no or text. Older files read as they are (`Story::declare_unknown`).
/// 10: scripts as trees – block, loop, exit loop, wait, conditions on the options of a random action and a choice
/// (`sim/story.rs`, `sim/script.rs`); goals and the command center's button with expressions. Older files read as they
/// are (a goal's old condition becomes its expressions).
/// 11: more events for triggers (time, buildings, kills, regions, things reached) and the actions Camera and Noise
/// (`sim/watch.rs`) – an older game can't read them.
const VERSION: u32 = 11;

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
    /// TAB's "Info": who made it, what it's about, and a picture for the list of custom levels – the name of a PNG file
    /// next to the map file. Empty: none (and not written).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub picture: String,
    /// TAB's `ForceRandomGeneration`: every time the game starts it (and at "Restart"), the map is made anew from `setup`
    /// with a new seed – its round and its events stay the file's (`world_to_play`). The editor shows the saved one.
    #[serde(default)]
    pub regenerate: bool,
    pub seed: u64,
    pub size: usize,
    pub border: usize,
    /// The playing field (the world border) – files before format 4: in the middle, `border` inside the edge.
    #[serde(default)]
    pub play_area: Option<PlayArea>,
    #[serde(default)]
    pub round: RoundData,
    /// What the landscape does to the round (`data::Modifiers`, from `game_maps.json`) – not written when nothing
    /// changes. Also with `regenerate`: the file's, not the setup's.
    #[serde(default, skip_serializing_if = "Modifiers::is_default")]
    pub modifiers: Modifiers,
    /// Which of TAB's landscapes it's drawn as (`MapSetup::look`) – not written when it's BR's (`""`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub look: String,
    /// The events of this map (`sim/events.rs`).
    #[serde(default)]
    pub events: Vec<Event>,
    /// Files before format 5: the timeline of waves – read, turned into events (`old_events`), never written. (Their
    /// constant spawn is left out: TAB's stragglers are events now.)
    #[serde(default, skip_serializing)]
    pub waves: Vec<OldWave>,
    /// The map's story (`sim/story.rs`): triggers, goals, the command center's button, variables – not written if empty.
    #[serde(default, skip_serializing_if = "Story::is_empty")]
    pub story: Story,
    /// How the map was generated – for the info line and "New map" (same setup, new seed). `None`: not known (files
    /// from before 2026-10-17).
    #[serde(default)]
    pub setup: Option<MapSetup>,
    pub spawners: Vec<SpawnerData>,
    /// The named regions and points (`sim/regions.rs`), with their numbers – not written when there are none. Files
    /// before format 6 have none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<Zone>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<Spot>,
    pub buildings: Vec<BuildingData>,
    pub units: Vec<UnitData>,
    pub zombies: Vec<ZombieData>,
    /// Neutral buildings and units – nobody's, maybe joining the colony (`sim/neutral.rs`).
    #[serde(default)]
    pub neutral: Vec<NeutralData>,
    /// The houses of the villages of doom and the loot – files from before 2026-10-20 have none.
    #[serde(default)]
    pub doom: Vec<DoomData>,
    #[serde(default)]
    pub loot: Vec<LootData>,
    /// Treasure ruins and power line towers (`sim/ornaments.rs`) – only the look. Not written when there are none;
    /// an older game skips the field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ornaments: Vec<Ornament>,
    /// Every tile with ore on it. `default`: files from before 2026-10-17 have none.
    #[serde(default)]
    pub deposits: Vec<DepositData>,
    /// The big rocks put down in the editor (`Map::place_big_rock`) – their tiles are `M` in `terrain` as well. Not
    /// written when there are none; an older game skips the field and draws small pieces there.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub big_rocks: Vec<BigRock>,
    /// Which pictures the forest, rock or ore of a tile is drawn with, where the editor's painter chose them
    /// (`Map::picture`) – in runs along a row: `[x, y, how many tiles, picture]`. Only the look: not written when
    /// everything is mixed, and an older game skips the field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pictures: Vec<(i32, i32, i32, u8)>,
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
    let awake = world.zombies.iter().map(|z| ZombieData::awake(world, z));
    let asleep = world.sleep.iter().map(ZombieData::asleep);
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

/// A building of the colony – with what the editor gave it (TAB's entity properties): its hit points in percent,
/// infected, a tag. Left out in the file when they're as usual.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct BuildingData {
    pub kind: BuildingKind,
    pub tile: (i32, i32),
    pub turned: bool,
    #[serde(default = "hundred", skip_serializing_if = "is_hundred")]
    pub hp_percent: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub infected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<Tag>,
}

/// A unit of the colony – the same, and a veteran or not.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct UnitData {
    pub kind: UnitKind,
    pub pos: (f32, f32),
    #[serde(default = "hundred", skip_serializing_if = "is_hundred")]
    pub hp_percent: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub veteran: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<Tag>,
}

/// A neutral building or unit (`sim/neutral.rs`): what it is, whether it joins the colony when a unit comes close, and
/// how it will be then.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct NeutralData {
    pub thing: Thing,
    #[serde(default)]
    pub joinable: bool,
    #[serde(default = "hundred", skip_serializing_if = "is_hundred")]
    pub hp_percent: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub veteran: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<Tag>,
}

/// 100 – full hit points, when a file says nothing (`#[serde(default = "hundred")]`) …
fn hundred() -> u32 {
    100
}

/// … and then it isn't written either.
fn is_hundred(n: &u32) -> bool {
    *n == 100
}

/// A thing's hit points in percent of its full ones, rounded down – at least 1.
fn percent(hp: u32, full: u32) -> u32 {
    (hp * 100 / full.max(1)).clamp(1, 100)
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

// Every thing of the map as the file keeps it (`of`), and back onto a world (`add_to`). `from_world` and `to_world` go
// through these – and so do the editor's copy and paste (`sim/clipboard.rs`): whatever the file keeps of a thing, a copy
// keeps too.

impl SpawnerData {
    pub fn of(s: &Spawner) -> SpawnerData {
        SpawnerData { name: s.name.clone(), id: s.id, min: s.min, size: s.size, axes: s.axes, mode: s.mode }
    }

    /// With its number – or, if that's 0 or taken, the next free one (`World::add_spawner`).
    pub fn add_to(&self, world: &mut World) {
        world.add_spawner(self.id, self.name.clone(), self.min, self.size, self.axes, self.mode);
    }
}

impl BuildingData {
    pub fn of(b: &Building) -> BuildingData {
        BuildingData {
            kind: b.kind,
            tile: b.tile,
            turned: b.turned,
            hp_percent: percent(b.hp, b.stats().hp),
            infected: b.infected,
            tag: b.tag.as_deref().cloned(),
        }
    }

    /// Only where it fits – `add_building_anywhere` leaves out one that doesn't.
    pub fn add_to(&self, world: &mut World) {
        let before = world.buildings.len();
        world.add_building_anywhere(self.kind, self.tile, self.turned);
        // What the editor gave it – if it's there.
        if let Some(id) = world.buildings.get(before).map(|built| built.id) {
            world.edit_thing(ThingRef::Building(id), self.hp_percent, self.infected, self.tag.as_ref());
        }
    }
}

impl UnitData {
    pub fn of(u: &Unit) -> UnitData {
        UnitData { kind: u.kind, pos: (u.pos.x, u.pos.y), hp_percent: percent(u.hp, u.max_hp()), veteran: u.veteran, tag: u.tag.as_deref().cloned() }
    }

    pub fn add_to(&self, world: &mut World) {
        world.add_unit(self.kind, vec2(self.pos.0, self.pos.1));
        let id = world.units.last().map_or(0, |unit| unit.id);
        world.edit_thing(ThingRef::Unit(id), self.hp_percent, self.veteran, self.tag.as_ref());
    }
}

impl NeutralData {
    pub fn of(n: &Neutral) -> NeutralData {
        NeutralData {
            thing: n.thing,
            joinable: n.joinable,
            hp_percent: n.hp_percent,
            veteran: n.veteran,
            tag: n.tag.as_deref().cloned(),
        }
    }

    /// A building only where it fits (`World::add_neutral`).
    pub fn add_to(&self, world: &mut World) {
        world.add_neutral(self.thing, self.joinable, self.hp_percent, self.veteran, self.tag.clone().map(Box::new));
    }
}

impl ZombieData {
    pub fn awake(world: &World, z: &Zombie) -> ZombieData {
        ZombieData { pos: (z.pos.x, z.pos.y), breed: z.breed, wander: world.is_wanderer(z) }
    }

    /// A sleeper is a wanderer – put on a world, it lies down asleep again.
    pub fn asleep(s: &Sleeper) -> ZombieData {
        ZombieData { pos: (s.pos.x, s.pos.y), breed: s.breed, wander: true }
    }

    /// Wanderers lie down asleep, the others storm.
    pub fn add_to(&self, world: &mut World) {
        let pos = vec2(self.pos.0, self.pos.1);
        if self.wander {
            world.add_sleeper(pos, self.breed);
        } else {
            world.add_zombie(pos, self.breed, false);
        }
    }
}

impl DoomData {
    pub fn of(d: &DoomHouse) -> DoomData {
        DoomData { house: d.tile, size: d.size, zombies: d.inside, factor: d.factor }
    }

    pub fn add_to(&self, world: &mut World) {
        world.add_doom(self.house, self.size, self.zombies, self.factor);
    }
}

impl LootData {
    pub fn of(l: &Loot) -> LootData {
        LootData { loot: l.kind, pos: (l.pos.x, l.pos.y), amount: l.amount, guards: l.guards }
    }

    pub fn add_to(&self, world: &mut World) {
        world.add_loot(vec2(self.pos.0, self.pos.1), self.loot, self.amount, self.guards);
    }
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
            author: String::new(),
            description: String::new(),
            picture: String::new(),
            regenerate: false,
            seed: world.seed,
            size: world.map.size,
            border: world.map.border,
            play_area: Some({
                let (min, size) = world.map.play_steps();
                PlayArea { min, size }
            }),
            round: RoundData { days: world.days, gold: world.gold, wood: world.stock.get(Resource::Wood), difficulty: world.difficulty.clone() },
            modifiers: world.modifiers,
            look: world.look.clone(),
            events: world.events.clone(),
            waves: Vec::new(),
            story: world.story.clone(),
            setup: None,
            spawners: world.spawners.iter().map(SpawnerData::of).collect(),
            regions: world.zones.clone(),
            points: world.spots.clone(),
            buildings: world.buildings.iter().map(BuildingData::of).collect(),
            units: world.units.iter().map(UnitData::of).collect(),
            neutral: world.neutral.iter().map(NeutralData::of).collect(),
            zombies: zombie_data(world),
            doom: world.doom.iter().map(DoomData::of).collect(),
            loot: world.loot.iter().map(LootData::of).collect(),
            ornaments: world.ornaments.clone(),
            deposits: world.map.all_deposits().map(|(x, y, d)| DepositData { tile: (x, y), ore: d.ore, amount: d.amount }).collect(),
            big_rocks: world.map.big_rocks().to_vec(),
            pictures: picture_runs(world.map.all_pictures()),
            terrain,
        }
    }

    /// A world from the file – at tick 0, as the round starts. `Err` with a reason if the file doesn't add up.
    pub fn to_world(&self) -> Result<World, String> {
        let mut world = World::empty(self.seed, self.map()?);
        world.set_difficulty(data::get().difficulty(&self.round.difficulty));
        // Before the zombies: their speed is rolled with it.
        world.modifiers = self.modifiers;
        world.set_look(&self.look);
        world.days = self.round.days;
        world.gold = self.round.gold;
        world.stock.set(Resource::Wood, self.round.wood);
        for s in &self.spawners {
            s.add_to(&mut world);
        }
        self.add_regions(&mut world);
        for b in &self.buildings {
            b.add_to(&mut world);
        }
        // The base is where the command center stands – on a map made in the editor it may stand anywhere, a new map has
        // it in the middle (`World::empty`, the same point). What the zombies keep away from (`keep_out`) and where waves
        // go without a command center.
        if let Some(cc) = world.buildings.iter().find(|b| b.kind == BuildingKind::CommandCenter) {
            world.base = cc.center();
        }
        for u in &self.units {
            u.add_to(&mut world);
        }
        for n in &self.neutral {
            n.add_to(&mut world);
        }
        world.story = self.story.clone();
        // Wanderers lie down asleep – the ones near your eyes wake when the world settles (`World::settle`).
        for z in &self.zombies {
            z.add_to(&mut world);
        }
        for d in &self.doom {
            d.add_to(&mut world);
        }
        for l in &self.loot {
            l.add_to(&mut world);
        }
        world.ornaments = self.ornaments.clone();
        world.events = if self.version < 5 && self.events.is_empty() { old_events(&self.waves) } else { self.events.clone() };
        world.roll_events();
        world.settle();
        Ok(world)
    }

    /// The world the game plays (not the editor): the file's – or, with `regenerate` and a known setup, a new map from
    /// the setup and `seed` with the file's round and events on it (TAB's `ForceRandomGeneration`).
    pub fn world_to_play(&self, seed: u64) -> Result<World, String> {
        let Some(setup) = self.setup.as_ref().filter(|_| self.regenerate) else {
            return self.to_world();
        };
        let mut setup = setup.clone();
        setup.round.days = self.round.days as f32;
        setup.round.difficulty = self.round.difficulty.clone();
        setup.modifiers = self.modifiers;
        setup.look = self.look.clone();
        let mut world = World::generate(seed, &setup);
        world.gold = self.round.gold;
        world.stock.set(Resource::Wood, self.round.wood);
        world.events = self.events.clone();
        world.roll_events();
        world.story = self.story.clone();
        // They lie where they lay on the saved map – on a new one that's somewhere else, but a script may still name them.
        self.add_regions(&mut world);
        Ok(world)
    }

    /// The file's regions and points into the world, each with its number.
    fn add_regions(&self, world: &mut World) {
        for zone in &self.regions {
            world.add_zone(zone.clone());
        }
        for spot in &self.points {
            world.add_spot(spot.clone());
        }
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
        for &rock in &self.big_rocks {
            map.place_big_rock(rock);
        }
        for (x, y, picture) in picture_tiles(&self.pictures) {
            map.set_picture(x, y, picture);
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
        file.big_rocks = self
            .big_rocks
            .iter()
            .map(|r| BigRock { tile: shift(r.tile), ..*r })
            .filter(|r| all_on_map(r.tile, r.size, r.size))
            .collect();
        file.pictures = picture_runs(picture_tiles(&self.pictures).map(|(x, y, p)| (x + d, y + d, p)).filter(|&(x, y, _)| on_map(x, y)));
        file.doom = self.doom.iter().map(|h| DoomData { house: shift(h.house), ..h.clone() }).filter(|h| all_on_map(h.house, h.size.0, h.size.1)).collect();
        file.units = self.units.iter().map(|u| UnitData { pos: moved(u.pos), ..u.clone() }).filter(|u| point_on_map(u.pos)).collect();
        file.neutral = self
            .neutral
            .iter()
            .map(|n| {
                let thing = match n.thing {
                    Thing::Building { kind, tile, turned } => Thing::Building { kind, tile: shift(tile), turned },
                    Thing::Unit { kind, pos } => Thing::Unit { kind, pos: moved(pos) },
                };
                NeutralData { thing, ..n.clone() }
            })
            .filter(|n| match n.thing {
                Thing::Building { kind, tile, turned } => {
                    let (w, h) = kind.footprint(turned);
                    all_on_map(tile, w, h)
                }
                Thing::Unit { pos, .. } => point_on_map(pos),
            })
            .collect();
        file.zombies = self.zombies.iter().map(|z| ZombieData { pos: moved(z.pos), ..z.clone() }).filter(|z| point_on_map(z.pos)).collect();
        file.loot = self.loot.iter().map(|l| LootData { pos: moved(l.pos), ..l.clone() }).filter(|l| point_on_map(l.pos)).collect();
        file.ornaments = self
            .ornaments
            .iter()
            .map(|o| Ornament { tile: shift(o.tile), ..*o })
            .filter(|o| {
                let (w, h) = o.kind.footprint();
                all_on_map(o.tile, w, h)
            })
            .collect();
        // In the screen axes (`map::to_frame`: across x − y, down x + y) d tiles both ways are 0 across and 2 · d down.
        let down = |min: (i32, i32)| (min.0, min.1 + 2 * d);
        file.spawners = self
            .spawners
            .iter()
            .map(|s| SpawnerData { min: if s.axes == Axes::Screen { down(s.min) } else { shift(s.min) }, ..s.clone() })
            .collect();
        // Regions and points move along, also off the map: back on a bigger grid they're where they were.
        file.regions = self.regions.iter().map(|z| z.moved((d, d))).collect();
        file.points = self.points.iter().map(|p| Spot { tile: shift(p.tile), ..p.clone() }).collect();
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
        MapFile::parse(&text)
    }

    /// A map file's text as a `MapFile` – an older format is turned into the newest.
    pub fn parse(text: &str) -> Result<MapFile, String> {
        /// Only the number: an old file is read as plain JSON first, a new one straight into the structs.
        #[derive(Deserialize)]
        struct Head {
            #[serde(default)]
            version: u32,
        }
        let not_a_map = |e: serde_json::Error| format!("not a map file: {e}");
        let head: Head = serde_json::from_str(text).map_err(not_a_map)?;
        let mut file: MapFile = if head.version < 8 {
            let mut value: serde_json::Value = serde_json::from_str(text).map_err(not_a_map)?;
            triggers_from_scripts(&mut value);
            serde_json::from_value(value).map_err(not_a_map)?
        } else {
            serde_json::from_str(text).map_err(not_a_map)?
        };
        if head.version < 9 {
            file.story.declare_unknown();
        }
        file.upgrade();
        Ok(file)
    }
}

/// Format 8: a file before it has its scripts on the story (`on_start`, `on_won`, `on_lost`, the command center's
/// button), on the events (`script`, `script_done`) and on the neutral things (`on_join`) – they become triggers, in
/// this order. A neutral thing without a name ID gets one (`join 1` …), the triggers know it by that. Counters
/// (`[name, value]`) become variables.
fn triggers_from_scripts(file: &mut serde_json::Value) {
    use serde_json::{json, Map as Json, Value};

    /// The non-empty list under `key`, taken out of `object`.
    fn take_script(object: &mut Json<String, Value>, key: &str) -> Option<Value> {
        match object.remove(key) {
            Some(Value::Array(list)) if !list.is_empty() => Some(Value::Array(list)),
            _ => None,
        }
    }
    let trigger = |name: String, event: Value, actions: Value| json!({ "name": name, "event": event, "actions": actions });

    let Some(root) = file.as_object_mut() else {
        return;
    };
    let mut triggers = Vec::new();
    if let Some(Value::Object(story)) = root.get_mut("story") {
        for (key, name, event) in [("on_start", "On game start", "Start"), ("on_won", "On victory", "Won"), ("on_lost", "On defeat", "Lost")] {
            if let Some(actions) = take_script(story, key) {
                triggers.push(trigger(name.to_owned(), json!(event), actions));
            }
        }
        if let Some(Value::Object(command)) = story.get_mut("command")
            && let Some(actions) = take_script(command, "script")
        {
            triggers.push(trigger("Command center button".to_owned(), json!("Button"), actions));
        }
        if let Some(Value::Array(counters)) = story.remove("counters") {
            let variables: Vec<Value> = counters.iter().filter_map(|c| Some(json!({ "name": c.get(0)?, "start": c.get(1)? }))).collect();
            if !variables.is_empty() {
                story.insert("variables".to_owned(), Value::Array(variables));
            }
        }
    }
    if let Some(Value::Array(events)) = root.get_mut("events") {
        for event in events.iter_mut().filter_map(Value::as_object_mut) {
            let name = event.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
            if let Some(actions) = take_script(event, "script") {
                triggers.push(trigger(format!("{name}: it comes"), json!({ "EventComes": { "event": name } }), actions));
            }
            if let Some(actions) = take_script(event, "script_done") {
                triggers.push(trigger(format!("{name}: it's done"), json!({ "EventDone": { "event": name } }), actions));
            }
        }
    }
    if let Some(Value::Array(neutral)) = root.get_mut("neutral") {
        for (k, thing) in neutral.iter_mut().filter_map(Value::as_object_mut).enumerate() {
            let Some(actions) = take_script(thing, "on_join") else {
                continue;
            };
            let tag = thing.entry("tag").or_insert(Value::Null);
            if !tag.is_object() {
                *tag = json!({});
            }
            let tag = tag.as_object_mut().expect("just made one");
            let mut name_id = tag.get("name_id").and_then(Value::as_str).unwrap_or_default().to_owned();
            if name_id.is_empty() {
                name_id = format!("join {}", k + 1);
                tag.insert("name_id".to_owned(), json!(name_id));
            }
            triggers.push(trigger(format!("{name_id}: it joins"), json!({ "Join": { "name_id": name_id } }), actions));
        }
    }
    if !triggers.is_empty() {
        let story = root.entry("story").or_insert_with(|| json!({}));
        if let Some(story) = story.as_object_mut() {
            story.insert("triggers".to_owned(), Value::Array(triggers));
        }
    }
}

/// The tiles with a chosen picture (`Map::all_pictures`: row by row, left to right) as runs along a row: `(x, y, how
/// many tiles, picture)` – a forest painted with one picture is a few hundred runs instead of thousands of tiles.
fn picture_runs(tiles: impl Iterator<Item = (i32, i32, u8)>) -> Vec<(i32, i32, i32, u8)> {
    let mut runs: Vec<(i32, i32, i32, u8)> = Vec::new();
    for (x, y, picture) in tiles {
        match runs.last_mut() {
            // Right behind the last run, in the same row, the same picture: that run gets one tile longer.
            Some(run) if run.1 == y && run.0 + run.2 == x && run.3 == picture => run.2 += 1,
            _ => runs.push((x, y, 1, picture)),
        }
    }
    runs
}

/// The runs as single tiles again: (x, y, picture).
fn picture_tiles(runs: &[(i32, i32, i32, u8)]) -> impl Iterator<Item = (i32, i32, u8)> + '_ {
    runs.iter().flat_map(|&(x, y, len, picture)| (0..len).map(move |k| (x + k, y, picture)))
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
    for key in ["name", "custom", "author", "description", "picture", "regenerate", "seed", "size", "border", "play_area", "round", "modifiers", "look", "events", "story", "setup", "spawners", "regions", "points", "buildings", "units", "zombies", "neutral", "doom", "loot", "ornaments", "deposits", "big_rocks", "pictures", "terrain"] {
        text = text.replacen(&format!(",\"{key}\":"), &format!(",\n\"{key}\":"), 1);
    }
    // One entry per line: events and spawners start with "name", the groups of events with "from", buildings and units
    // with "kind", houses of doom with "house", piles of loot with "loot".
    for first in ["name", "from", "kind", "thing", "house", "loot"] {
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
    use crate::sim::TriggerEvent;
    use crate::map::Ore;
    use crate::sim::{BreedCount, Order};

    /// Saved and loaded again, everything is still there: terrain, ore, spawners (a slanted one too), buildings, units,
    /// zombies, events (groups out of chosen spawners too) – and the round.
    #[test]
    fn a_map_comes_back_the_same() {
        let mut world = World::new(4);
        let c = world.map.size as i32 / 2;
        let (first, last) = (world.spawners[0].id, world.spawners[11].id);
        let (unit, other, cc) = (world.units[0].id, world.units[1].id, world.buildings[0].id);
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
            Order::SetTag { thing: ThingRef::Unit(unit), tag: Tag { name_id: "hero".into(), text: "A long text".into(), ..Tag::default() } },
            Order::SetHealth { thing: ThingRef::Unit(unit), percent: 30 },
            Order::SetMark { thing: ThingRef::Building(cc), on: true },
            // Neutral, not joinable: the start units right next to it would take it in at once.
            Order::SetOwner { thing: ThingRef::Unit(other), owner: crate::sim::Owner::Neutral },
        ]);
        let mut file = MapFile::from_world(&world, "Test: map?");
        file.setup = Some(MapSetup::default());
        (file.author, file.description, file.picture, file.regenerate) = ("Me".into(), "A test, \"quoted\".".into(), "pic.png".into(), true);
        file.story.triggers = vec![crate::sim::Trigger {
            comment: "Greets the player.".into(),
            actions: vec![crate::sim::Action::Window { title: "Hi".into(), text: "Welcome.".into() }.into()],
            ..crate::sim::Trigger::new("Welcome", crate::sim::TriggerEvent::Start)
        }];
        file.story.variables = vec![crate::sim::Variable { name: "keys".into(), kind: crate::sim::VarKind::Number, start: crate::sim::Value::Number(2) }];
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
        assert_eq!(loaded.story, file.story, "the story isn't loaded");
        // What the editor gave the things: a tag, hit points, infected, a neutral unit.
        assert_eq!(loaded.units[0].tag.as_ref().map(|t| t.name_id.as_str()), Some("hero"));
        assert_eq!(loaded.units[0].hp, loaded.units[0].max_hp() * 30 / 100);
        assert!(loaded.buildings[0].infected);
        assert!(loaded.neutral.len() == 1 && !loaded.neutral[0].joinable);
        assert_eq!((loaded.days, loaded.gold, loaded.stock.get(Resource::Wood)), (25, world.gold, 55));
        let again = MapFile {
            setup: file.setup.clone(),
            author: file.author.clone(),
            description: file.description.clone(),
            picture: file.picture.clone(),
            regenerate: true,
            ..MapFile::from_world(&loaded, "Test: map?")
        };
        assert_eq!(again, file, "saving the loaded map gives the same file");
        assert_eq!(file_stem("Test: map?"), "Test_ map_");
    }

    /// TAB's random generation: with `regenerate` the game gets a new map from the file's setup for every seed – with the
    /// file's round and events. Without it, the saved map.
    #[test]
    fn a_regenerated_level_keeps_its_round_and_events() {
        let mut setup = MapSetup::default();
        setup.common.size = 128.0;
        let world = World::generate(4, &setup);
        let mut file = MapFile::from_world(&world, "Random");
        file.setup = Some(setup);
        file.round.gold = 777;
        file.events.truncate(2);
        assert!(file.world_to_play(9).unwrap().map.same_terrain(&world.map), "without regenerate it isn't the saved map");
        file.regenerate = true;
        let (a, b) = (file.world_to_play(9).unwrap(), file.world_to_play(10).unwrap());
        assert!(!a.map.same_terrain(&b.map), "the same map for two seeds");
        assert_eq!((a.gold, a.days), (777, file.round.days));
        assert_eq!(a.events, file.events);
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

    /// A file before format 8 has scripts on the start, the end, the command center's button, the events and the neutral
    /// things, and counters: reading it turns them into triggers (in that order) and variables, and a neutral thing
    /// without a name ID gets one. The result is what format 8 would have written – nothing of the old stays.
    #[test]
    fn old_scripts_become_triggers() {
        let old = r#"{"version":7,"name":"Old","seed":7,"size":2,"border":0,"terrain":["..",".."],
            "events":[{"name":"Raid","hour":5,"script":[{"Notify":{"text":"coming"}}],"script_done":[{"Win":null}]},{"name":"Quiet","hour":9}],
            "story":{"on_start":[{"Counter":{"name":"keys","add":false,"value":1}}],"on_lost":[{"Notify":{"text":"lost"}}],
                "command":{"label":"Dig","script":[{"Counter":{"name":"keys","add":true,"value":1}}],"uses":2},
                "goals":[{"text":"keys","condition":{"Counter":{"name":"keys","at_least":3}}}],"counters":[["keys",0]]},
            "spawners":[],"buildings":[],"units":[],"zombies":[],
            "neutral":[{"thing":{"Unit":{"kind":"Soldier","pos":[0.5,0.5]}},"on_join":[{"Reveal":null}]},
                {"thing":{"Unit":{"kind":"Soldier","pos":[1.5,1.5]}},"tag":{"name_id":"boss"},"on_join":[{"Notify":{"text":"hi"}}]},
                {"thing":{"Unit":{"kind":"Soldier","pos":[1.5,0.5]}}}]}"#;
        let file = MapFile::parse(old).unwrap();
        assert_eq!(file.version, VERSION);
        let story = &file.story;
        let names: Vec<(&str, &TriggerEvent)> = story.triggers.iter().map(|t| (t.name.as_str(), &t.event)).collect();
        assert_eq!(
            names,
            [
                ("On game start", &TriggerEvent::Start),
                ("On defeat", &TriggerEvent::Lost),
                ("Command center button", &TriggerEvent::Button),
                ("Raid: it comes", &TriggerEvent::EventComes { event: "Raid".into() }),
                ("Raid: it's done", &TriggerEvent::EventDone { event: "Raid".into() }),
                ("join 1: it joins", &TriggerEvent::Join { name_id: "join 1".into() }),
                ("boss: it joins", &TriggerEvent::Join { name_id: "boss".into() }),
            ]
        );
        assert_eq!(story.variables, [crate::sim::Variable { name: "keys".into(), kind: crate::sim::VarKind::Number, start: crate::sim::Value::Number(0) }]);
        assert_eq!(story.goals[0].when.to_string(), "keys >= 3", "a goal's Counter isn't read as a variable");
        assert_eq!(story.goals[0].progress.as_ref().map(|p| p.to_string()).as_deref(), Some("keys * 100 / 3"));
        assert_eq!(story.command, Some(crate::sim::CustomCommand { label: "Dig".into(), text: String::new(), uses: 2, ..Default::default() }));
        assert_eq!(file.neutral[0].tag.as_ref().map(|t| t.name_id.as_str()), Some("join 1"));
        assert!(file.neutral[2].tag.is_none(), "a thing without a script got a tag");
        // It plays: the start trigger sets the variable.
        let mut world = file.to_world().unwrap();
        world.step(&[]);
        assert_eq!(world.variable("keys"), 1);
        // And what's written reads back the same.
        let again = MapFile::parse(&pretty(&serde_json::to_string(&file).unwrap())).unwrap();
        assert_eq!(again, file);
    }

    /// Format 9: steps may have a condition and code, triggers have expressions as conditions. A step without either
    /// is still just its action in the file – and a file of format 8 with the goals' kind of conditions, the action
    /// "Variable" and an undeclared counter reads as the same in the new kind.
    #[test]
    fn steps_and_conditions_are_written_and_old_ones_are_read() {
        use crate::sim::{Action, Step, Trigger};
        let mut file = MapFile::parse(r#"{"version":9,"name":"N","seed":7,"size":2,"border":0,"terrain":["..",".."],"spawners":[],"buildings":[],"units":[],"zombies":[]}"#).unwrap();
        let variables = vec![crate::sim::Variable { name: "keys".into(), kind: crate::sim::VarKind::Number, start: crate::sim::Value::Number(0) }];
        let scope = crate::sim::Scope { variables: &variables };
        let mut trigger = Trigger::new("t", TriggerEvent::Start);
        trigger.conditions = vec![crate::sim::parse_condition("Day >= 3 & keys < 2", scope).unwrap()];
        trigger.actions = vec![
            Step::new(Action::Reveal),
            Step {
                action: Action::Notify { text: "hi".into() },
                when: Some(crate::sim::parse_condition("keys = 0", scope).unwrap()),
                code: crate::sim::parse_code("keys += 1", scope).unwrap(),
            },
        ];
        file.story.variables = variables;
        file.story.triggers = vec![trigger];
        let text = serde_json::to_string(&file).unwrap();
        assert!(text.contains(r#"["Reveal",{"action":{"Notify":{"text":"hi"}},"when":"#), "a plain step isn't just its action: {text}");
        assert_eq!(MapFile::parse(&text).unwrap(), file);

        let old = r#"{"version":8,"name":"Old","seed":7,"size":2,"border":0,"terrain":["..",".."],"spawners":[],"buildings":[],"units":[],"zombies":[],
            "story":{"triggers":[{"name":"t","event":"Start","conditions":[{"Day":3},{"Variable":{"name":"keys","at_least":2}}],
                "actions":[{"Variable":{"name":"keys","add":true,"value":1}},"Reveal"]}],"variables":[{"name":"met","kind":"Flag","start":1}]}}"#;
        let old = MapFile::parse(old).unwrap();
        let t = &old.story.triggers[0];
        assert_eq!(t.conditions.iter().map(|c| c.to_string()).collect::<Vec<_>>(), ["Day >= 3", "keys >= 2"]);
        assert_eq!(t.actions.len(), 2);
        assert_eq!(crate::sim::code_text(&t.actions[0].code), "keys += 1");
        assert_eq!(t.actions[1], Step::new(Action::Reveal));
        let declared: Vec<&str> = old.story.variables.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(declared, ["met", "keys"], "a counter nobody declared is declared");
        let mut world = old.to_world().unwrap();
        world.step(&[]);
        assert_eq!(world.variable("keys"), 0, "Day >= 3 doesn't hold on the first day");
    }

    /// Format 10: scripts are trees (block, loop, wait, options with conditions), goals and the button have expressions.
    /// A file of format 9 – options as `[weight, steps]` / `[label, steps]`, a goal with one of the fixed conditions –
    /// reads as the same in the new kind, and what is written reads back the same.
    #[test]
    fn scripts_as_trees_are_written_and_format_9_is_read() {
        use crate::sim::Action;
        let old = r#"{"version":9,"name":"Old","seed":7,"size":2,"border":0,"terrain":["..",".."],"spawners":[],"buildings":[],"units":[],"zombies":[],
            "story":{"triggers":[{"name":"t","event":"Start","actions":[
                {"Random":{"options":[[2,["Reveal"]],[1,[]]]}},
                {"Choose":{"text":"?","options":[["Yes",["Win"]],["No",[]]]}}]}],
              "goals":[{"text":"Hold","condition":{"Day":5},"defeat":false},{"text":"Tents","condition":{"Built":{"kind":"Tent","count":4}},"defeat":true},
                       {"text":"Boss","condition":{"Destroyed":{"name_id":"boss"}}}],
              "command":{"label":"Dig","text":"","uses":0}}}"#;
        let file = MapFile::parse(old).unwrap();
        assert_eq!(file.version, VERSION);
        let story = &file.story;
        let goals: Vec<(String, Option<String>, bool)> =
            story.goals.iter().map(|g| (g.when.to_string(), g.progress.as_ref().map(|p| p.to_string()), g.defeat)).collect();
        assert_eq!(
            goals,
            [
                ("Day >= 5".to_owned(), Some("Day * 100 / 5".to_owned()), false),
                ("Count(\"Tent\") >= 4".to_owned(), Some("Count(\"Tent\") * 100 / 4".to_owned()), true),
                ("IsDead(\"boss\")".to_owned(), None, false),
            ]
        );
        let Action::Random { options } = &story.triggers[0].actions[0].action else {
            panic!("not a random action");
        };
        assert_eq!((options[0].weight, options[0].steps.len(), options[1].weight), (2, 1, 1));
        let Action::Choose { options, .. } = &story.triggers[0].actions[1].action else {
            panic!("not a choice");
        };
        assert_eq!((options[0].label.as_str(), options[1].label.as_str(), options[0].when.is_none()), ("Yes", "No", true));

        // New things: a loop with a wait and a choice whose button has a condition; the goal and the button with expressions.
        use crate::sim::{Expr, Pick, Step, Trigger};
        let mut file = file;
        let back = Pick { label: "Back".into(), when: Some(Expr::yes()), steps: vec![Step::new(Action::ExitLoop { loops: 1 })] };
        let menu = Action::Choose { text: "Again?".into(), options: vec![back] };
        let steps = vec![Step::new(Action::Wait { seconds: Expr::number(3) }), Step::new(menu)];
        let mut trigger = Trigger::new("menu", TriggerEvent::Start);
        trigger.actions = vec![Step::new(Action::Loop { steps }), Step::new(Action::Block { steps: vec![Step::new(Action::Reveal)] })];
        file.story.triggers = vec![trigger];
        file.story.command = Some(crate::sim::CustomCommand { label: "Dig".into(), visible: Some(Expr::yes()), counter: Some(Expr::number(4)), ..Default::default() });
        let text = serde_json::to_string(&file).unwrap();
        assert!(!text.contains("\"condition\""), "the old kind of condition is not written any more: {text}");
        let again = MapFile::parse(&pretty(&text)).unwrap();
        assert_eq!(again, file);
        // And it plays: the loop waits, the block after it waits for the loop.
        let mut world = again.to_world().unwrap();
        world.step(&[]);
        assert_eq!(world.runs.len(), 1);
        assert!(!world.revealed);
    }

    /// Format 11: the new events (time, buildings, kills, regions, a thing reached) and the actions Camera and Noise are
    /// written and read back the same.
    #[test]
    fn the_events_of_format_11_are_written_and_read_back() {
        use crate::sim::{Action, Site, Step, Trigger};
        let mut file = MapFile::from_world(&World::new(4), "x");
        let events = [
            TriggerEvent::Every { seconds: 30 },
            TriggerEvent::Clock { day: 3, hour: 12 },
            TriggerEvent::Built { kind: None },
            TriggerEvent::Built { kind: Some(BuildingKind::Tent) },
            TriggerEvent::Fell { kind: Some(BuildingKind::Wall) },
            TriggerEvent::Kills { count: 500 },
            TriggerEvent::Enters { zone: 2 },
            TriggerEvent::Leaves { zone: 2 },
            TriggerEvent::Reached { name_id: "chest".into() },
        ];
        file.story.triggers = events
            .iter()
            .map(|e| {
                let mut t = Trigger::new("t", e.clone());
                t.actions = vec![Step::new(Action::Camera { to: Site::Region(2) }), Step::new(Action::Noise { at: Site::Point(1), volume: 800 })];
                t
            })
            .collect();
        let text = pretty(&serde_json::to_string(&file).unwrap());
        let again = MapFile::parse(&text).unwrap();
        assert_eq!(again.story, file.story);
        assert_eq!(again.version, VERSION);
    }

    /// A round from its file starts with the command center's gold as the income – nobody has paid yet, but it will at
    /// the first payout. Before, the HUD showed only the upkeep there.
    #[test]
    fn the_income_shows_before_the_first_payout() {
        let world = MapFile::from_world(&World::new(4), "x").to_world().unwrap();
        assert_eq!(world.tick, 0);
        assert_eq!(world.income(), BuildingKind::CommandCenter.stats().gold);
    }

    /// The pictures the painter chose are kept, in runs along a row – and move with the grid. A map with none doesn't
    /// have the field, and a file without it (every file so far) reads as "all mixed".
    #[test]
    fn chosen_pictures_are_saved_in_runs_and_move_with_the_grid() {
        let mut world = World::new(4);
        let c = world.map.size as i32 / 2;
        let row: Vec<(i32, i32)> = (0..5).map(|k| (c + 20 + k, c + 3)).collect();
        let below: Vec<(i32, i32)> = (0..2).map(|k| (c + 20 + k, c + 4)).collect();
        let all: Vec<(i32, i32)> = row.iter().chain(&below).copied().collect();
        world.apply_orders(&[
            Order::RemoveAt { tiles: all.clone() },
            Order::PaintOre { ore: None, tiles: all.clone() },
            Order::PaintTerrain { terrain: Terrain::Forest, tiles: all },
            Order::PaintPicture { picture: 2, tiles: row },
            Order::PaintPicture { picture: 3, tiles: below },
        ]);
        let file = MapFile::from_world(&world, "x");
        assert_eq!(file.pictures, vec![(c + 20, c + 3, 5, 2), (c + 20, c + 4, 2, 3)]);
        let text = pretty(&serde_json::to_string(&file).unwrap());
        let back: MapFile = serde_json::from_str(&text).unwrap();
        let map = back.to_world().unwrap().map;
        assert_eq!((map.picture(c + 24, c + 3), map.picture(c + 21, c + 4), map.picture(c + 25, c + 3)), (2, 3, 0));
        // 16 tiles more on every side: everything 16 further.
        let grown = file.resized(file.size + 32);
        assert_eq!(grown.pictures, vec![(c + 36, c + 19, 5, 2), (c + 36, c + 20, 2, 3)]);
        // Ground painted over loses its pictures; a map without any doesn't write the field – and reads without it.
        world.apply_orders(&[Order::PaintTerrain { terrain: Terrain::Grass, tiles: vec![(c + 22, c + 3)] }]);
        assert_eq!(MapFile::from_world(&world, "x").pictures[..2], [(c + 20, c + 3, 2, 2), (c + 23, c + 3, 2, 2)]);
        let plain = serde_json::to_string(&MapFile::from_world(&World::new(4), "None")).unwrap();
        assert!(!plain.contains("pictures"));
        assert!(serde_json::from_str::<MapFile>(&plain).unwrap().pictures.is_empty());
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

    /// The big rocks are saved, come back, and move with the rest when the grid grows.
    #[test]
    fn big_rocks_are_saved_and_move_with_the_grid() {
        let mut world = World::new(4);
        let c = world.map.size as i32 / 2;
        let rock = BigRock { tile: (c + 10, c - 12), size: 5, variant: 2 };
        world.step(&[Order::PlaceBigRock(rock)]);
        let file = MapFile::from_world(&world, "Rocks");
        assert_eq!(file.big_rocks, vec![rock]);
        let text = pretty(&serde_json::to_string(&file).unwrap());
        assert!(text.contains("
\"big_rocks\""), "not in a line of its own");
        let back: MapFile = serde_json::from_str(&text).unwrap();
        assert_eq!(back.to_world().unwrap().map.big_rocks(), &[rock]);
        // Two tiles more on every side: it moves along.
        let grown = file.resized(file.size + 4);
        let moved = BigRock { tile: (rock.tile.0 + 2, rock.tile.1 + 2), ..rock };
        assert_eq!(grown.to_world().unwrap().map.big_rocks(), &[moved]);
        // A file without big rocks doesn't get the field.
        assert!(!serde_json::to_string(&MapFile::from_world(&World::new(4), "None")).unwrap().contains("big_rocks"));
    }
}
