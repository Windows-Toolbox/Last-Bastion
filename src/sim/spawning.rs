//! Where zombies come from: walkers scattered over the map at the start, the constant spawn of the round
//! (`data\spawning.json`), spawners (placed in the editor), waves (now or on the timeline), and your dead units
//! rising again. A killed zombie stays dead – a cleared part of the map stays clear.

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use super::terrain::free_point_near;
use super::zombies::{values, Breed};
use super::{Order, World, DAY_TICKS, HOUR_TICKS, TICKS_PER_SECOND};
use crate::data::{self, Stage};
use crate::kinds::UnitKind;

/// So many random spots in the spawners are tried for a new zombie before giving up
/// (e.g. when the base has been walled off with water and is unreachable from everywhere).
const SPAWN_TRIES: u32 = 1000;
/// A spawner placed in the editor starts this big (square).
pub(super) const NEW_SPAWNER_SIZE: i32 = 6;

/// An area on the map where zombies come from – the only place they come from. Placed, renamed and resized
/// in the editor; players don't see spawners.
pub struct Spawner {
    pub id: u32,
    pub name: String,
    /// The top left tile.
    pub min: (i32, i32),
    /// Width and height in tiles.
    pub size: (i32, i32),
    /// What it sends all by itself (waves come on top).
    pub mode: SpawnMode,
    /// Periodic: ticks since it last sent zombies.
    pub(super) timer: u32,
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
    /// The top left corner and the size in world coordinates.
    pub fn rect(&self) -> (Vec2, Vec2) {
        (vec2(self.min.0 as f32, self.min.1 as f32), vec2(self.size.0 as f32, self.size.1 as f32))
    }

    pub fn contains(&self, p: Vec2) -> bool {
        let (min, size) = self.rect();
        p.x >= min.x && p.y >= min.y && p.x < min.x + size.x && p.y < min.y + size.y
    }

    /// Which side of the map it is closest to: 0 top (y = 0), 1 bottom, 2 left (x = 0), 3 right.
    pub(super) fn side(&self, map_size: usize) -> u32 {
        let (min, size) = self.rect();
        let c = min + size / 2.0;
        let edge = map_size as f32;
        let distances = [c.y, edge - c.y, c.x, edge - c.x];
        (0..4).min_by(|&a, &b| distances[a].total_cmp(&distances[b])).unwrap_or(0) as u32
    }
}

/// Where a wave comes from (editor).
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum WaveFrom {
    /// All of them from one random spawner.
    OneSpawner,
    /// This many different random sides of the map – on each side the full count, spread over its spawners.
    Sides(u32),
}

/// A wave on the timeline of the map (editor, "Waves" tab – like TAB's events "on day 12, 6:00 a wave from the
/// north"). It comes all by itself when the clock gets there.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct TimedWave {
    /// At this tick (`DAY_TICKS`, `HOUR_TICKS`).
    pub at: u64,
    pub count: u32,
    pub from: WaveFrom,
}

/// What the constant spawn sends right now: a group of `min` to `max` zombies of this breed every `every` ticks.
#[derive(Debug, PartialEq)]
pub struct Group {
    pub breed: Breed,
    pub every: u64,
    pub min: u32,
    pub max: u32,
}

/// The tick a stage of `data\spawning.json` starts at (day 1, hour 0 = tick 0).
fn stage_start(stage: &Stage) -> u64 {
    stage.day.saturating_sub(1) * DAY_TICKS + stage.hour * HOUR_TICKS
}

/// What the constant spawn sends at this tick – `None` before the first stage. Within a stage the values go over
/// smoothly to those of the next one, as long as that one has the same breed (walkers: more and more often, bigger
/// groups). Next stage another breed: the values stay until it starts. After the last stage the groups keep
/// growing by `extra_per_day`.
pub(super) fn group_at(tick: u64) -> Option<Group> {
    let spawning = &data::get().spawning;
    let i = spawning.stages.iter().rposition(|s| stage_start(s) <= tick)?;
    let stage = spawning.stages[i];
    let (mut every, mut min, mut max) = (stage.every_seconds, stage.group[0] as f32, stage.group[1] as f32);
    let since = (tick - stage_start(&stage)) as f32;
    match spawning.stages.get(i + 1) {
        Some(next) if next.breed == stage.breed && stage_start(next) > stage_start(&stage) => {
            let share = since / (stage_start(next) - stage_start(&stage)) as f32;
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
    Some(Group { breed: stage.breed, every, min, max: round(max).max(min) })
}

/// One of your units has fallen: it lies there and rises as a walker when `rise_in` runs out.
pub struct Corpse {
    pub pos: Vec2,
    /// What it was – only for drawing.
    pub kind: UnitKind,
    /// Ticks until it gets up.
    pub(super) rise_in: u32,
}

impl World {
    /// The start of a round: zombies scattered over the playing field, wandering around where they are until they
    /// notice one of your units. `per_thousand`: how many per 1,000 tiles. Where and which breed: `roaming` in
    /// `data\zombies.json` – fewer near the command center, more further out; walkers everywhere, further out runners
    /// too, at the edge also fat ones.
    pub(super) fn scatter_roamers(&mut self, per_thousand: f32) {
        let roaming = &values().roaming;
        let play = self.map.play_area();
        let count = (per_thousand * play.size * play.size / 1000.0) as usize;
        let half = play.size / 2.0;
        let mut placed = 0;
        // Many random spots are thrown away near the middle – enough tries for that, but never endless.
        for _ in 0..count * 20 {
            if placed == count {
                break;
            }
            let pos = play.min + vec2(self.rng.unit_f32(), self.rng.unit_f32()) * play.size;
            let d = pos.distance(self.base);
            // Share of the way to the edge (the corners count as edge too).
            let out = (d / half).min(1.0);
            let keep = libm::powf(out, roaming.edge_weight);
            if d < roaming.keep_out || self.rng.unit_f32() >= keep || self.map.is_blocked(pos.x.floor() as i32, pos.y.floor() as i32) {
                continue;
            }
            // Which breed: one number decides – fat ones first (at the edge), then runners (further out), else walkers.
            let roll = self.rng.unit_f32();
            let fat = if out >= roaming.fat_from { roaming.fat_share } else { 0.0 };
            let runner = if out >= roaming.runners_from { roaming.runner_share } else { 0.0 };
            let breed = if roll < fat {
                Breed::Fat
            } else if roll < fat + runner {
                Breed::Runner
            } else {
                Breed::Walker
            };
            self.spawn_zombie_as(pos, breed, true);
            placed += 1;
        }
    }

    /// Units that died this tick lie down – they rise after `rise_min_seconds` to `rise_max_seconds`
    /// (`data\zombies.json`). Before they're removed (`remove_dead`).
    pub(super) fn lay_down_dead_units(&mut self) {
        let (min, max) = (values().rise_min, values().rise_max);
        for i in 0..self.units.len() {
            if self.units[i].hp == 0 {
                let rise_in = min + self.rng.below(max.saturating_sub(min) + 1);
                self.corpses.push(Corpse { pos: self.units[i].pos, kind: self.units[i].kind, rise_in });
            }
        }
    }

    /// The dead count down, and whoever's time has come gets up as a walker – storming, not wandering. Something
    /// built on the spot in the meantime: it gets up next to it.
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
                self.spawn_zombie_as(pos, Breed::Walker, false);
            }
        }
    }

    pub fn add_spawner(&mut self, name: String, min: (i32, i32), size: (i32, i32), mode: SpawnMode) {
        self.next_spawner_id += 1;
        self.spawners.push(Spawner { id: self.next_spawner_id, name, min, size, mode, timer: 0 });
        self.keep_spawner_on_map(self.spawners.len() - 1);
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

    /// `count` zombies of this breed, each from a random one of these spawners (indices into `spawners`). They storm
    /// the command center.
    pub(super) fn spawn_from(&mut self, spawners: &[usize], count: usize, breed: Breed) {
        for _ in 0..count {
            let Some(pos) = self.spawn_point(spawners) else {
                break; // no spot in these spawners leads to the base: nobody can come
            };
            self.spawn_zombie_as(pos, breed, false);
        }
    }

    /// A random free point in a random one of these spawners, from which the base can be reached.
    /// `None` if there are no spawners, or nothing was found after `SPAWN_TRIES` tries (base cut off,
    /// spawners painted over).
    fn spawn_point(&mut self, spawners: &[usize]) -> Option<Vec2> {
        if spawners.is_empty() {
            return None;
        }
        for _ in 0..SPAWN_TRIES {
            let s = &self.spawners[spawners[self.rng.below(spawners.len() as u32) as usize]];
            let (min, size) = (s.min, s.size);
            let x = min.0 + self.rng.below(size.0 as u32) as i32;
            let y = min.1 + self.rng.below(size.1 as u32) as i32;
            if !self.map.is_blocked(x, y) && self.to_base.reachable(x, y) {
                return Some(vec2(x as f32 + self.rng.unit_f32(), y as f32 + self.rng.unit_f32()));
            }
        }
        None
    }

    /// The waves of the timeline whose time has come. In the order of the list – the same on every PC.
    pub(super) fn fire_timed_waves(&mut self) {
        let due: Vec<TimedWave> = self.waves.iter().filter(|w| w.at == self.tick).copied().collect();
        for wave in due {
            self.apply(&Order::SpawnWave { count: wave.count as usize, from: wave.from });
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
                self.spawn_from(&[s], count as usize, Breed::Runner);
            }
        }
    }

    /// What the constant spawn sends right now – for the developer overlay (F3).
    pub fn spawn_group(&self) -> Option<Group> {
        group_at(self.tick)
    }

    /// The constant spawn (`group_at`): when its time has come, one group out of one random spawner set to
    /// "constant". Then the next group is due `every` ticks later – how often is read anew each time, so it gets
    /// more frequent over the round.
    fn constant_spawn(&mut self) {
        if self.tick < self.next_group {
            return;
        }
        let Some(group) = group_at(self.tick) else {
            // Before the first stage: nothing until it starts (no stages at all: never).
            let first = data::get().spawning.stages.iter().map(stage_start).min();
            self.next_group = first.filter(|&t| t > self.tick).unwrap_or(u64::MAX);
            return;
        };
        self.next_group = self.tick + group.every;
        let constant: Vec<usize> = (0..self.spawners.len()).filter(|&s| self.spawners[s].mode == SpawnMode::Constant).collect();
        if constant.is_empty() {
            return;
        }
        let s = constant[self.rng.below(constant.len() as u32) as usize];
        let count = group.min + self.rng.below(group.max - group.min + 1);
        self.spawn_from(&[s], count as usize, group.breed);
    }

    /// A spawner always lies completely on the map and is at least 1 × 1 tile.
    pub(super) fn keep_spawner_on_map(&mut self, s: usize) {
        let map = self.map.size as i32;
        let spawner = &mut self.spawners[s];
        let w = spawner.size.0.clamp(1, map);
        let h = spawner.size.1.clamp(1, map);
        spawner.size = (w, h);
        spawner.min = (spawner.min.0.clamp(0, map - w), spawner.min.1.clamp(0, map - h));
    }
}
