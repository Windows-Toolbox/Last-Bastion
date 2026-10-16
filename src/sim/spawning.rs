//! Where zombies come from: spawners (placed in the editor), waves (now or on the timeline), and killed zombies
//! that come back.

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use super::zombies::{Breed, ZOMBIE_HP};
use super::{Order, World, TICKS_PER_SECOND};

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
    /// What it sends all by itself (waves and the start horde come on top).
    pub mode: SpawnMode,
    /// Periodic: ticks since it last sent zombies.
    pub(super) timer: u32,
}

/// How a spawner sends zombies by itself (set in the editor).
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum SpawnMode {
    /// Constantly: every killed zombie comes back – in one of the spawners set to this (if "Respawn" is on).
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

impl World {
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

    /// `count` zombies, each from a random one of these spawners (indices into `spawners`).
    pub(super) fn spawn_from(&mut self, spawners: &[usize], count: usize) {
        for _ in 0..count {
            let Some(pos) = self.spawn_point(spawners) else {
                break; // no spot in these spawners leads to the base: nobody can come
            };
            self.spawn_zombie(pos);
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

    /// Periodic spawners: count the ticks, and when the time is up send their zombies.
    pub(super) fn update_spawners(&mut self) {
        for s in 0..self.spawners.len() {
            let SpawnMode::Periodic { every, count } = self.spawners[s].mode else {
                continue;
            };
            self.spawners[s].timer += 1;
            if self.spawners[s].timer as f32 >= every as f32 * TICKS_PER_SECOND {
                self.spawners[s].timer = 0;
                self.spawn_from(&[s], count as usize);
            }
        }
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

    /// Dead zombies (and those at the base) start again in a spawner – as a new zombie with a new `id`.
    pub(super) fn respawn_zombies(&mut self) {
        // Killed zombies come back only in the spawners that send constantly.
        let constant: Vec<usize> = (0..self.spawners.len()).filter(|&s| self.spawners[s].mode == SpawnMode::Constant).collect();
        for i in 0..self.zombies.len() {
            if self.zombies[i].is_alive() {
                continue;
            }
            // No such spawner, or no spot in them leads to the base anymore: this one stays dead.
            let Some(pos) = self.spawn_point(&constant) else {
                continue;
            };
            let id = self.new_zombie_id();
            let zombie = &mut self.zombies[i];
            zombie.pos = pos;
            zombie.prev = pos;
            zombie.id = id;
            zombie.hp = ZOMBIE_HP;
            zombie.cooldown = 0;
            zombie.alert = 0;
            // Comes back as a normal zombie of the usual mix.
            zombie.breed = Breed::mixed(id);
            zombie.wander = None;
        }
    }
}
