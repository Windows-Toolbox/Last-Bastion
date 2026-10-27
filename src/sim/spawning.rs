//! Where zombies come from: walkers scattered over the map at the start, the constant spawn of the map (a new map
//! gets it from `data\spawning.json`), spawners (placed in the editor), waves (now or on the timeline), and your dead
//! units rising again. A killed zombie stays dead – a cleared part of the map stays clear.

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use super::terrain::free_point_near;
use super::zombies::{values, Breed};
use super::{World, DAY_TICKS, HOUR_TICKS, TICKS_PER_SECOND};
use crate::data::{self, ConstantSpawn, Stage};
use crate::kinds::UnitKind;
use crate::map::{from_frame, to_frame, Map, Region};

/// So many random spots in the spawners are tried for a new zombie before giving up
/// (e.g. when the base has been walled off with water and is unreachable from everywhere).
const SPAWN_TRIES: u32 = 1000;
/// A spawner placed in the editor starts this big (steps of the screen axes – on screen twice as wide as high).
pub(super) const NEW_SPAWNER_SIZE: i32 = 8;

/// An area on the map where zombies come from – the only place they come from. Placed, renamed and resized
/// in the editor; players don't see spawners. A rectangle: straight on screen like the world border, or slanted like
/// the tiles (`axes`) – `min` and `size` count in those axes, in whole steps.
#[derive(Clone)]
pub struct Spawner {
    pub id: u32,
    pub name: String,
    /// The corner with the smallest values – on screen the top left one, slanted the top one.
    pub min: (i32, i32),
    /// Width and height.
    pub size: (i32, i32),
    pub axes: Axes,
    /// What it sends all by itself (waves come on top).
    pub mode: SpawnMode,
    /// Periodic: ticks since it last sent zombies.
    pub(super) timer: u32,
}

/// The axes a spawner's corner and size count in (editor: right click on it).
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub enum Axes {
    /// The screen's (`map::to_frame`): straight on screen, like the world border – a step across is half a tile wide.
    /// `default`: every spawner in map files before format 4.
    #[default]
    Screen,
    /// The tiles': slanted on screen like every tile – a step is one tile.
    Tiles,
}

impl Axes {
    /// A world point in these axes.
    pub fn of(self, p: Vec2) -> Vec2 {
        match self {
            Axes::Screen => to_frame(p),
            Axes::Tiles => p,
        }
    }

    /// The other way: a point in these axes as a world point.
    pub fn back(self, a: Vec2) -> Vec2 {
        match self {
            Axes::Screen => from_frame(a),
            Axes::Tiles => a,
        }
    }
}

/// The four corners (world points, in order around it) of a rectangle from `min`, `size` big, in `axes` – for drawing
/// a spawner, also one that is being dragged.
pub fn corners(axes: Axes, min: (i32, i32), size: (i32, i32)) -> [Vec2; 4] {
    let (x0, y0) = (min.0 as f32, min.1 as f32);
    let (x1, y1) = (x0 + size.0 as f32, y0 + size.1 as f32);
    [vec2(x0, y0), vec2(x1, y0), vec2(x1, y1), vec2(x0, y1)].map(|a| axes.back(a))
}

/// The tiles such a rectangle covers (straight on screen: those whose middle lies inside) – for the pictures of maps.
pub fn tiles(axes: Axes, min: (i32, i32), size: (i32, i32)) -> Vec<(i32, i32)> {
    match axes {
        Axes::Screen => {
            let (x0, y0) = (min.0 as f32, min.1 as f32);
            let region = Region { min: vec2(x0, y0), max: vec2(x0 + size.0 as f32, y0 + size.1 as f32) };
            // `move`: the closure takes `y` along – it lives on after this line, the loop variable wouldn't.
            region.rows(0.0).flat_map(|(y, xs)| xs.map(move |x| (x, y))).collect()
        }
        Axes::Tiles => (min.1..min.1 + size.1).flat_map(|y| (min.0..min.0 + size.0).map(move |x| (x, y))).collect(),
    }
}

/// How a spawner sends zombies by itself (set in the editor).
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum SpawnMode {
    /// Takes part in the constant spawn of the round (`data\spawning.json`): each group comes out of one of the
    /// spawners set to this, picked at random.
    Constant,
    /// Every `every` seconds `count` new zombies.
    Periodic { every: u32, count: u32 },
    /// Nothing by itself – only waves.
    Off,
}

impl SpawnMode {
    /// What "periodic" starts with when it's chosen in the editor.
    pub const PERIODIC: SpawnMode = SpawnMode::Periodic { every: 10, count: 10 };
}

impl Spawner {
    pub fn contains(&self, p: Vec2) -> bool {
        let a = self.axes.of(p);
        let (x0, y0) = (self.min.0 as f32, self.min.1 as f32);
        a.x > x0 && a.y > y0 && a.x < x0 + self.size.0 as f32 && a.y < y0 + self.size.1 as f32
    }

    /// The middle (world point).
    pub fn center(&self) -> Vec2 {
        self.point_at(0.5, 0.5)
    }

    /// The point `u` of the way across and `v` of the way down (0 to 1, in its axes).
    fn point_at(&self, u: f32, v: f32) -> Vec2 {
        let min = vec2(self.min.0 as f32, self.min.1 as f32);
        self.axes.back(min + vec2(u * self.size.0 as f32, v * self.size.1 as f32))
    }

    /// Which side of the map it is closest to, as you see it: 0 top, 1 bottom, 2 left, 3 right.
    pub(super) fn side(&self, map: &Map) -> u32 {
        map.full_area().nearest_side(self.center())
    }

    /// The same spawner in the other axes (`Axes`): around the same middle, about as many tiles, as long and thin –
    /// turned by 45°. A step of the screen axes is `1 / √2` of a tile edge, so the sides grow or shrink by `√2`.
    pub(super) fn turn_to(&mut self, axes: Axes) {
        if axes == self.axes {
            return;
        }
        let factor = if axes == Axes::Tiles { std::f32::consts::FRAC_1_SQRT_2 } else { std::f32::consts::SQRT_2 };
        let side = |n: i32| ((n as f32 * factor).round() as i32).max(1);
        let size = (side(self.size.0), side(self.size.1));
        let center = axes.of(self.center());
        self.min = ((center.x - size.0 as f32 / 2.0).round() as i32, (center.y - size.1 as f32 / 2.0).round() as i32);
        self.size = size;
        self.axes = axes;
    }
}

/// Where a group of a wave comes from (editor).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum WaveFrom {
    /// All of them from one random spawner.
    OneSpawner,
    /// This many different random sides of the map – on each side the full count, spread over its spawners.
    Sides(u32),
    /// These spawners (their `id`): the count once, spread over all of them – "5000 runners out of C, D, F and G". A
    /// spawner that is removed leaves the list (`World::forget_spawner`); with none left nobody comes.
    Spawners(Vec<u32>),
}

/// A wave on the timeline of the map (editor, "Waves" tab – like TAB's events "on day 12, 6:00 a wave from the
/// north"). It comes all by itself when the clock gets there. A new round plans its waves itself (`plan_waves`).
/// `from = "WaveInFile"`: a file is read through that type – it also knows the waves of older files.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(from = "WaveInFile")]
pub struct TimedWave {
    /// At this tick (`DAY_TICKS`, `HOUR_TICKS`).
    pub at: u64,
    /// Part of the final wave: when every final wave has come and none of its zombies is left, the round is won
    /// (`round.rs`). It also calls every zombie on the map – they all belong to it then (`World::call_everyone`).
    pub final_wave: bool,
    /// When it comes, the whole map is shown from then on (`World::revealed`).
    pub reveal: bool,
    /// Its groups: each from its own place, with its own breeds – "5000 runners out of C, D, F, G and 200 spitters out
    /// of B" is one wave of two groups.
    pub groups: Vec<WaveGroup>,
}

/// One group of a wave: where it comes out – picked once, all its breeds come out there together – and how many of
/// each breed. (`breeds`, not `zombies`: in the map file the waves come before the map's own field `zombies`, and
/// `mapfile::pretty` finds that one by where it first appears.)
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct WaveGroup {
    pub from: WaveFrom,
    pub breeds: Vec<BreedCount>,
}

/// So many zombies of one breed – from every side the group comes from (`WaveFrom::Sides`), or in total.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct BreedCount {
    pub count: u32,
    pub breed: Breed,
}

impl TimedWave {
    /// A wave of one group with one breed.
    pub fn new(at: u64, from: WaveFrom, breed: Breed, count: u32) -> TimedWave {
        TimedWave { at, final_wave: false, reveal: false, groups: vec![WaveGroup { from, breeds: vec![BreedCount { count, breed }] }] }
    }

    /// How many zombies it brings in all – a group from several sides brings its count from each of them (as long as
    /// there are spawners on that many sides).
    pub fn zombies(&self) -> u32 {
        let times = |from: &WaveFrom| match from {
            WaveFrom::Sides(n) => (*n).min(4),
            _ => 1,
        };
        self.groups.iter().map(|g| g.breeds.iter().map(|z| z.count).sum::<u32>() * times(&g.from)).sum()
    }
}

/// A wave as a map file has it. `untagged`: the file doesn't say which of the two it is – serde tries them in order,
/// and the first one whose fields fit is it.
#[derive(Deserialize)]
#[serde(untagged)]
enum WaveInFile {
    /// Since map format 4: with groups.
    Groups {
        at: u64,
        #[serde(default)]
        final_wave: bool,
        #[serde(default)]
        reveal: bool,
        groups: Vec<WaveGroup>,
    },
    /// Before: one breed per wave – several of them at the same time came out of the same random sides together
    /// (`merge_old_waves` makes one wave of them). Files from before 2026-10-17 have no breed (their waves were
    /// runners) and no final wave.
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

/// What a wave without a breed in its file sends – for `#[serde(default = "runner")]` above, which wants a function.
fn runner() -> Breed {
    Breed::Runner
}

/// `impl From`: how serde makes a `TimedWave` out of what it read (`#[serde(from = …)]`).
impl From<WaveInFile> for TimedWave {
    fn from(wave: WaveInFile) -> TimedWave {
        match wave {
            WaveInFile::Groups { at, final_wave, reveal, groups } => TimedWave { at, final_wave, reveal, groups },
            WaveInFile::Single { at, count, from, breed, final_wave, reveal } => {
                TimedWave { final_wave, reveal, ..TimedWave::new(at, from, breed, count) }
            }
        }
    }
}

/// The waves of a map file before format 4 had one breed each; at the same time and from the same kind of place they
/// came out of the same random spawners. Now that's one wave with one group per kind of place – it comes the same.
/// In the order of the list: the first wave at a time takes in the later ones.
pub fn merge_old_waves(waves: Vec<TimedWave>) -> Vec<TimedWave> {
    let mut merged: Vec<TimedWave> = Vec::new();
    for wave in waves {
        let Some(into) = merged.iter_mut().find(|w| w.at == wave.at && w.final_wave == wave.final_wave) else {
            merged.push(wave);
            continue;
        };
        into.reveal |= wave.reveal;
        for group in wave.groups {
            match into.groups.iter_mut().find(|g| g.from == group.from) {
                Some(same) => same.breeds.extend(group.breeds),
                None => into.groups.push(group),
            }
        }
    }
    merged
}

/// What the constant spawn sends right now: a group of `min` to `max` zombies of this breed every `every` ticks –
/// `specials` of them other breeds (breed, share), `unrest` of them more because of the colony's noise. Out of these
/// spawners (`id`s) – `None`: out of those set to "Constant".
#[derive(Debug, PartialEq)]
pub struct Group {
    pub breed: Breed,
    pub every: u64,
    pub min: u32,
    pub max: u32,
    pub specials: Vec<(Breed, f32)>,
    pub unrest: u32,
    pub spawners: Option<Vec<u32>>,
}

/// The constant spawn `spawning` with every group `percent` as big (`constant_spawn_percent` of the difficulty) – the
/// group sizes rounded, at least 1 where there was one before, and the growth after the last stage too.
pub fn scaled_groups(spawning: &ConstantSpawn, percent: u32) -> ConstantSpawn {
    let scale = |count: u32| if count == 0 { 0 } else { ((count * percent + 50) / 100).max(1) };
    let mut scaled = spawning.clone();
    for stage in &mut scaled.stages {
        stage.group = stage.group.map(scale);
    }
    scaled.extra_per_day = spawning.extra_per_day * percent as f32 / 100.0;
    scaled
}

/// The tick a stage of the constant spawn starts at (day 1, hour 0 = tick 0).
pub fn stage_start(stage: &Stage) -> u64 {
    stage.day.saturating_sub(1) * DAY_TICKS + stage.hour * HOUR_TICKS
}

/// What the constant spawn `spawning` sends at this tick – `None` before the first stage. Within a stage the values
/// go over smoothly to those of the next one, as long as that one has the same breed (walkers: more and more often,
/// bigger groups). Next stage another breed: the values stay until it starts. After the last stage the groups keep
/// growing by `extra_per_day`.
pub(super) fn group_at(spawning: &ConstantSpawn, tick: u64) -> Option<Group> {
    let i = spawning.stages.iter().rposition(|s| stage_start(s) <= tick)?;
    let stage = &spawning.stages[i];
    let (mut every, mut min, mut max) = (stage.every_seconds, stage.group[0] as f32, stage.group[1] as f32);
    let since = (tick - stage_start(stage)) as f32;
    match spawning.stages.get(i + 1) {
        Some(next) if next.breed == stage.breed && stage_start(next) > stage_start(stage) => {
            let share = since / (stage_start(next) - stage_start(stage)) as f32;
            every += (next.every_seconds - every) * share;
            min += (next.group[0] as f32 - min) * share;
            max += (next.group[1] as f32 - max) * share;
        }
        Some(_) => {}
        None => {
            let days = since / DAY_TICKS as f32;
            min += spawning.extra_per_day * days;
            max += spawning.extra_per_day * days;
        }
    }
    // Rounded – `floor` instead of `round`, the list of allowed float operations stays short (`sim/mod.rs`).
    let round = |x: f32| (x + 0.5).floor().max(0.0) as u32;
    let every = (round(every * TICKS_PER_SECOND) as u64).max(1);
    let min = round(min);
    // The specials whose day has come.
    let specials = spawning
        .specials
        .iter()
        .filter(|s| s.day.saturating_sub(1) * DAY_TICKS <= tick)
        .map(|s| (s.breed, s.share))
        .collect();
    Some(Group { breed: stage.breed, every, min, max: round(max).max(min), specials, unrest: 0, spawners: stage.spawners.clone() })
}

/// One of your units has fallen: it lies there and rises as a walker when `rise_in` runs out.
#[derive(Clone)]
pub struct Corpse {
    pub pos: Vec2,
    /// What it was – only for drawing.
    pub kind: UnitKind,
    /// Ticks until it gets up.
    pub(super) rise_in: u32,
}

impl World {
    /// The start of a round, like TAB: zombies almost everywhere on the playing field, in small packs on one tile each
    /// (`pack` in `data\roaming.json`: 1–3) – so never more than that on a tile. They idle where they are until they see
    /// one of your units or buildings with people. `per_thousand`: how many per 1,000 tiles they can stand on (open
    /// ground in the playing field, out of the ring around the command center no zombie lies in at the start – the
    /// smallest `keep_out`) – at most one pack per tile, so with packs of 1–3 about 2,000 fill the map.
    /// Where: a little more further out (`edge_weight`). The balance is in the breeds, not in how many (`roll_breed`):
    /// near the base nearly only walkers, further out runners, then the armored ones, the infested and the spitters,
    /// at the edge also fat ones – none closer than its `keep_out`.
    pub(super) fn scatter_roamers(&mut self, per_thousand: f32) {
        let roaming = &data::get().roaming;
        let size = self.map.size as i32;
        let [smallest, biggest] = roaming.pack;
        let biggest = biggest.max(smallest).max(1);
        // The tiles they can stand on – the whole tile out of the ring (its middle half a diagonal further).
        let ring = self.keep_out.least();
        let open: Vec<(i32, i32)> = (0..size)
            .flat_map(|y| (0..size).map(move |x| (x, y)))
            .filter(|&(x, y)| self.map.in_play(x, y) && !self.map.is_blocked(x, y))
            .filter(|&(x, y)| vec2(x as f32 + 0.5, y as f32 + 0.5).distance(self.base) >= ring + 0.75)
            .collect();
        if open.is_empty() {
            return;
        }
        let count = ((per_thousand * open.len() as f32 / 1000.0) as usize).min(open.len() * biggest as usize);
        // A tile with a pack on it takes no second one.
        let mut taken = vec![false; (size * size) as usize];
        let mut placed = 0;
        // Taken tiles and the ones thrown away near the middle are tried for nothing – enough tries, but never endless.
        for _ in 0..count * 20 {
            if placed >= count {
                break;
            }
            let (x, y) = open[self.rng.below(open.len() as u32) as usize];
            let tile = (y * size + x) as usize;
            let middle = vec2(x as f32 + 0.5, y as f32 + 0.5);
            let out = self.way_out(middle);
            // Toward the ring they thin out (`ring_ramp`): 0 right at it, 1 from `ring_ramp` tiles further out.
            let ramp = if roaming.ring_ramp > 0.0 { ((middle.distance(self.base) - ring) / roaming.ring_ramp).clamp(0.0, 1.0) } else { 1.0 };
            if taken[tile] || self.rng.unit_f32() >= libm::powf(out, roaming.edge_weight) * ramp {
                continue;
            }
            taken[tile] = true;
            // The packs too: right at the ring the smallest, the biggest only from `ring_ramp` tiles further out.
            let most = smallest + ((biggest - smallest) as f32 * ramp).round() as u32;
            let pack = (smallest + self.rng.below(most - smallest + 1)).min((count - placed) as u32);
            for _ in 0..pack {
                // Somewhere on the tile, not quite at its edge – asleep until something wakes it (`sleep.rs`).
                let pos = vec2(x as f32 + 0.15 + 0.7 * self.rng.unit_f32(), y as f32 + 0.15 + 0.7 * self.rng.unit_f32());
                let breed = self.roll_breed(pos);
                // Only when walkers have to stay further out than another breed: this one stays away.
                if pos.distance(self.base) >= self.keep_out.of(breed) {
                    self.lay_to_sleep(pos, breed);
                }
            }
            placed += pack as usize;
        }
    }

    /// The giants (DESIGN.md 8.1, like TAB's): only a few per map – `giants` of the map (times the difficulty's
    /// `giants_percent`) on a map of TAB's size (256 × 256 tiles inside the border), more on a bigger one, fewer on a
    /// smaller one (at least one, if any). Far out (`giants_from` in `data\roaming.json`, and never inside their
    /// `keep_out`), asleep like the roamers: they stroll and wake like any zombie – and the final wave calls them too.
    pub(super) fn place_giants(&mut self, giants: f32) {
        if giants <= 0.0 {
            return;
        }
        let size = self.map.size as i32;
        let from = data::get().roaming.giants_from;
        let keep_out = self.keep_out.of(Breed::Giant);
        let open: Vec<Vec2> = (0..size)
            .flat_map(|y| (0..size).map(move |x| (x, y)))
            .filter(|&(x, y)| self.map.in_play(x, y) && !self.map.is_blocked(x, y))
            .map(|(x, y)| vec2(x as f32 + 0.5, y as f32 + 0.5))
            .filter(|&p| self.way_out(p) >= from && p.distance(self.base) >= keep_out)
            .collect();
        if open.is_empty() {
            return;
        }
        let area = self.map.play_area().area();
        let count = ((giants * area / (256.0 * 256.0)).round() as u32).max(1);
        for _ in 0..count {
            let pos = open[self.rng.below(open.len() as u32) as usize];
            self.lay_to_sleep(pos, Breed::Giant);
        }
    }

    /// How far out a point lies: its share of the way from the base to the middle of a side of the playing field, in a
    /// straight line – 0 at the base, 1 at the edge (the corners count as edge too).
    pub(super) fn way_out(&self, pos: Vec2) -> f32 {
        let play = self.map.play_area();
        let half = (play.max.x - play.min.x) / 2.0 * std::f32::consts::FRAC_1_SQRT_2;
        (pos.distance(self.base) / half).min(1.0)
    }

    /// Which breed a zombie at `pos` is – one random number decides (`mix` in `data\roaming.json`): every breed of the
    /// mix that starts at or before how far out it lies (`way_out`) – and may come this close to the command center
    /// (`keep_out`) – gets its share of the numbers, in the order of the list. The rest are walkers.
    pub(super) fn roll_breed(&mut self, pos: Vec2) -> Breed {
        let (out, near) = (self.way_out(pos), pos.distance(self.base));
        let keep_out = self.keep_out;
        let roll = self.rng.unit_f32();
        let mut sum = 0.0;
        for mix in data::get().roaming.mix.iter().filter(|m| out >= m.from && near >= keep_out.of(m.breed)) {
            sum += mix.share;
            if roll < sum {
                return mix.breed;
            }
        }
        Breed::Walker
    }

    /// Units that died this tick lie down – they rise after `rise_min_seconds` to `rise_max_seconds`
    /// (`data\zombies.json`). Before they're removed (`remove_dead`).
    pub(super) fn lay_down_dead_units(&mut self) {
        let (min, max) = (values().rise_min, values().rise_max);
        for i in 0..self.units.len() {
            if self.units[i].hp == 0 {
                let rise_in = min + self.rng.below(max.saturating_sub(min) + 1);
                let pos = self.units[i].pos;
                self.corpses.push(Corpse { pos, kind: self.units[i].kind, rise_in });
                self.score.units_lost += 1;
                // Dying is loud (`sound.rs`).
                self.make_sound(pos, data::get().sound.unit_dies);
            }
        }
    }

    /// The dead count down, and whoever's time has come gets up as one of TAB's infected (`infected_breed`) – heading
    /// for the command center like a wave, not wandering. Something built on the spot in the meantime: it gets up next
    /// to it.
    pub(super) fn raise_corpses(&mut self) {
        let mut rising = Vec::new();
        self.corpses.retain_mut(|corpse| {
            corpse.rise_in = corpse.rise_in.saturating_sub(1);
            if corpse.rise_in == 0 {
                rising.push(corpse.pos);
            }
            corpse.rise_in > 0
        });
        for pos in rising {
            if let Some(pos) = free_point_near(&self.map, pos) {
                let breed = self.infected_breed();
                self.spawn_zombie_as(pos, breed, false);
            }
        }
    }

    /// A new spawner. `id`: the number it had in its map file – waves name their spawners by it (`WaveFrom::Spawners`).
    /// 0, or one that's taken already: the next free number.
    pub fn add_spawner(&mut self, id: u32, name: String, min: (i32, i32), size: (i32, i32), axes: Axes, mode: SpawnMode) {
        let id = if id == 0 || self.find_spawner(id).is_some() { self.next_spawner_id + 1 } else { id };
        self.next_spawner_id = self.next_spawner_id.max(id);
        self.spawners.push(Spawner { id, name, min, size, axes, mode, timer: 0 });
        self.keep_spawner_on_map(self.spawners.len() - 1);
    }

    /// Spawner `id` is gone: the waves and stages that named it don't any more.
    pub(super) fn forget_spawner(&mut self, id: u32) {
        for group in self.waves.iter_mut().flat_map(|w| w.groups.iter_mut()) {
            if let WaveFrom::Spawners(ids) = &mut group.from {
                ids.retain(|&s| s != id);
            }
        }
        for ids in self.constant.stages.iter_mut().filter_map(|s| s.spawners.as_mut()) {
            ids.retain(|&s| s != id);
        }
    }

    /// The number the next spawner will get – the editor selects a spawner right after placing it.
    pub fn next_spawner_id(&self) -> u32 {
        self.next_spawner_id + 1
    }

    /// Where is the spawner with this `id` in the list? `None` if it has been removed.
    pub fn find_spawner(&self, id: u32) -> Option<usize> {
        self.spawners.iter().position(|s| s.id == id)
    }

    /// `count` zombies scattered around `point` – the more, the bigger the crowd. Only for the tests.
    #[cfg(test)]
    pub(super) fn spawn_cluster(&mut self, count: usize, point: Vec2) {
        let radius = 0.5 + (count as f32).sqrt() * 0.3;
        for _ in 0..count {
            // A few tries to find a spot that isn't blocked.
            for _ in 0..10 {
                let offset = vec2(self.rng.unit_f32() * 2.0 - 1.0, self.rng.unit_f32() * 2.0 - 1.0);
                let pos = point + offset * radius;
                if !self.map.is_blocked(pos.x.floor() as i32, pos.y.floor() as i32) {
                    self.spawn_zombie(pos);
                    break;
                }
            }
        }
    }

    /// `count` zombies of this breed, each from a random one of these spawners – `(index into spawners, swarm)`: the
    /// swarm of a wave it then belongs to (`new_swarm`), 0: none. They storm the command center. `final_wave`: they
    /// belong to the final wave – the round is won once they're all dead.
    pub(super) fn spawn_from(&mut self, spawners: &[(usize, u32)], count: usize, breed: Breed, final_wave: bool) {
        let indices: Vec<usize> = spawners.iter().map(|&(s, _)| s).collect();
        for _ in 0..count {
            let Some((pos, k)) = self.spawn_point(&indices) else {
                break; // no spot in these spawners leads to the base: nobody can come
            };
            self.spawn_zombie_as(pos, breed, false);
            if let Some(zombie) = self.zombies.last_mut() {
                zombie.final_wave = final_wave;
                zombie.swarm = spawners[k].1;
            }
        }
    }

    /// A group of a wave (or "send now" in the editor): where it comes out is picked once (`wave_spawners`) – every
    /// breed of it comes out there, one after the other.
    pub(super) fn send_group(&mut self, group: &WaveGroup, final_wave: bool) {
        for spawners in self.wave_spawners(&group.from) {
            for z in &group.breeds {
                self.spawn_from(&spawners, z.count as usize, z.breed, final_wave);
            }
        }
    }

    /// Where a group of a wave comes out (`WaveFrom`), the sides picked at random: a list of spawners (indices, each
    /// with its swarm) per side – each list gets the group's full count. A single random spawner is one list too. The
    /// chosen spawners are one list – the count spread over them – with one swarm per side they lie on.
    fn wave_spawners(&mut self, from: &WaveFrom) -> Vec<Vec<(usize, u32)>> {
        match from {
            WaveFrom::OneSpawner if self.spawners.is_empty() => Vec::new(),
            WaveFrom::OneSpawner => {
                let s = self.rng.below(self.spawners.len() as u32) as usize;
                vec![vec![(s, self.new_swarm())]]
            }
            WaveFrom::Sides(sides) => {
                let map = &self.map;
                let mut left: Vec<u32> = (0..4).filter(|&side| self.spawners.iter().any(|s| s.side(map) == side)).collect();
                let mut groups = Vec::new();
                for _ in 0..(*sides).min(left.len() as u32) {
                    let side = left.remove(self.rng.below(left.len() as u32) as usize);
                    let swarm = self.new_swarm();
                    groups.push((0..self.spawners.len()).filter(|&s| self.spawners[s].side(&self.map) == side).map(|s| (s, swarm)).collect());
                }
                groups
            }
            WaveFrom::Spawners(ids) => {
                // (side, swarm) of every side so far.
                let mut swarms: Vec<(u32, u32)> = Vec::new();
                let mut list = Vec::new();
                let found: Vec<usize> = ids.iter().filter_map(|&id| self.find_spawner(id)).collect();
                for s in found {
                    let side = self.spawners[s].side(&self.map);
                    let swarm = match swarms.iter().find(|(other, _)| *other == side) {
                        Some(&(_, swarm)) => swarm,
                        None => {
                            let swarm = self.new_swarm();
                            swarms.push((side, swarm));
                            swarm
                        }
                    };
                    list.push((s, swarm));
                }
                if list.is_empty() { Vec::new() } else { vec![list] }
            }
        }
    }

    /// A random free point in a random one of these spawners (indices), from which the base can be reached – and
    /// which one of the list it is. `None` if there are no spawners, or nothing was found after `SPAWN_TRIES` tries
    /// (base cut off, spawners painted over).
    fn spawn_point(&mut self, spawners: &[usize]) -> Option<(Vec2, usize)> {
        if spawners.is_empty() {
            return None;
        }
        for _ in 0..SPAWN_TRIES {
            let k = self.rng.below(spawners.len() as u32) as usize;
            let (u, v) = (self.rng.unit_f32(), self.rng.unit_f32());
            let pos = self.spawners[spawners[k]].point_at(u, v);
            let (x, y) = (pos.x.floor() as i32, pos.y.floor() as i32);
            // A slanted spawner may stick out of the map at a corner (`Map::is_blocked`: off the map is blocked).
            if !self.map.is_blocked(x, y) && self.to_base.reachable(x, y) {
                return Some((pos, k));
            }
        }
        None
    }

    /// The waves of the timeline whose time has come, group by group (`send_group`). A final wave first calls every
    /// zombie on the map (`call_everyone`); a wave that shows the map shows it from now on. In the order of the list –
    /// the same on every PC.
    pub(super) fn fire_timed_waves(&mut self) {
        let due: Vec<TimedWave> = self.waves.iter().filter(|w| w.at == self.tick).cloned().collect();
        if due.iter().any(|w| w.final_wave) {
            self.call_everyone();
        }
        if due.iter().any(|w| w.reveal) {
            self.revealed = true;
        }
        for wave in &due {
            for group in &wave.groups {
                self.send_group(group, wave.final_wave);
            }
        }
    }

    /// The constant spawn of the round, then the periodic spawners (editor): count the ticks, and when the time is up
    /// send their zombies (runners).
    pub(super) fn update_spawners(&mut self) {
        self.constant_spawn();
        for s in 0..self.spawners.len() {
            let SpawnMode::Periodic { every, count } = self.spawners[s].mode else {
                continue;
            };
            self.spawners[s].timer += 1;
            if self.spawners[s].timer as f32 >= every as f32 * TICKS_PER_SECOND {
                self.spawners[s].timer = 0;
                let swarm = self.new_swarm();
                self.spawn_from(&[(s, swarm)], count as usize, Breed::Runner, false);
            }
        }
    }

    /// What the constant spawn sends right now (`group_at`) – with the zombies the colony's unrest adds to every group
    /// (`sound.rs`). Also for the developer overlay (F3).
    pub fn spawn_group(&self) -> Option<Group> {
        let mut group = group_at(&self.constant, self.tick)?;
        group.unrest = self.unrest_zombies();
        group.min += group.unrest;
        group.max += group.unrest;
        Some(group)
    }

    /// The constant spawn (`spawn_group`): when its time has come, one group out of one random spawner of its stage –
    /// those it names, otherwise one set to "constant" – all of them crowded on one random spot in it
    /// (`spawn_crowd`). Then the next group is due `every` ticks later – how often is read anew each time, so it gets
    /// more frequent over the round.
    fn constant_spawn(&mut self) {
        if self.tick < self.next_group {
            return;
        }
        let Some(group) = self.spawn_group() else {
            // Before the first stage: nothing until it starts (no stages at all: never).
            let first = self.constant.stages.iter().map(stage_start).min();
            self.next_group = first.filter(|&t| t > self.tick).unwrap_or(u64::MAX);
            return;
        };
        self.next_group = self.tick + group.every;
        let from: Vec<usize> = match &group.spawners {
            Some(ids) => ids.iter().filter_map(|&id| self.find_spawner(id)).collect(),
            None => (0..self.spawners.len()).filter(|&s| self.spawners[s].mode == SpawnMode::Constant).collect(),
        };
        if from.is_empty() {
            return;
        }
        let s = from[self.rng.below(from.len() as u32) as usize];
        let count = group.min + self.rng.below(group.max - group.min + 1);
        self.spawn_crowd(s, count, &group);
    }

    /// A group of the constant spawn: one random spot in spawner `s` that leads to the base, and `count` zombies
    /// crowded around it – the more, the bigger the crowd – not spread over the whole spawner. Each one is the group's
    /// breed, or one of its specials by their shares (`roll_special`).
    fn spawn_crowd(&mut self, s: usize, count: u32, group: &Group) {
        let Some((center, _)) = self.spawn_point(&[s]) else {
            return; // nothing in this spawner leads to the base
        };
        let radius = 0.5 + (count as f32).sqrt() * 0.4;
        for _ in 0..count {
            let breed = self.roll_special(group);
            // A few tries for a free spot around the middle – otherwise right on it (they push each other apart).
            let mut pos = center;
            for _ in 0..10 {
                let offset = vec2(self.rng.unit_f32() * 2.0 - 1.0, self.rng.unit_f32() * 2.0 - 1.0) * radius;
                let (x, y) = ((center.x + offset.x).floor() as i32, (center.y + offset.y).floor() as i32);
                if !self.map.is_blocked(x, y) && self.to_base.reachable(x, y) {
                    pos = center + offset;
                    break;
                }
            }
            self.spawn_zombie_as(pos, breed, false);
        }
    }

    /// One random number decides: each special of the group gets its share of the numbers, in the order of the list;
    /// the rest are the group's own breed.
    fn roll_special(&mut self, group: &Group) -> Breed {
        let roll = self.rng.unit_f32();
        let mut sum = 0.0;
        for &(breed, share) in &group.specials {
            sum += share;
            if roll < sum {
                return breed;
            }
        }
        group.breed
    }

    /// The constant spawn was changed (editor): keep its stages in order of time – `group_at` relies on that – and
    /// look again at the next tick what comes when, instead of waiting for a group planned with the old values.
    /// `sort_by_key` is stable: two stages at the same time keep their order, the same on every PC.
    pub(super) fn constant_spawn_changed(&mut self) {
        self.constant.stages.sort_by_key(stage_start);
        self.next_group = self.tick;
    }

    /// A spawner always lies completely on the map and is at least 1 × 1 step: straight on screen inside
    /// `Map::full_area`, slanted inside the grid (its corners are off the map – no zombie comes out there).
    pub(super) fn keep_spawner_on_map(&mut self, s: usize) {
        // The whole map in the screen axes: across from −size / 2 to size / 2, down from size / 2 to 3 · size / 2.
        // The grid: from 0 to size both ways.
        let n = self.map.size as i32;
        let spawner = &mut self.spawners[s];
        let (left, top) = match spawner.axes {
            Axes::Screen => (-n / 2, n - n / 2),
            Axes::Tiles => (0, 0),
        };
        let w = spawner.size.0.clamp(1, n);
        let h = spawner.size.1.clamp(1, n);
        spawner.size = (w, h);
        spawner.min = (spawner.min.0.clamp(left, left + n - w), spawner.min.1.clamp(top, top + n - h));
    }
}
