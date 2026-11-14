//! Where zombies come from: walkers scattered over the map at the start, spawners (placed in the editor) – the
//! events of the map send their zombies out of them (`events.rs`), the editor sends waves at once – and your dead
//! units rising again. A killed zombie stays dead – a cleared part of the map stays clear.

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use super::fx::Fx;
use super::fallen::Fling;
use super::terrain::free_point_near;
use super::zombies::{values, Breed};
use super::{World, TICKS_PER_SECOND};
use crate::data;
use crate::kinds::UnitKind;
use crate::map::{from_frame, to_frame, Map, Region};

/// So many random spots in the spawners are tried for a new zombie before giving up
/// (e.g. when the base has been walled off with water and is unreachable from everywhere).
const SPAWN_TRIES: u32 = 1000;
/// A spawner placed in the editor starts this big (steps of the screen axes – on screen √2 times as wide as high).
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
    /// What it sends all by itself (the events come on top).
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
    /// Every `every` seconds `count` new zombies.
    Periodic { every: u32, count: u32 },
    /// Nothing by itself – only the events. `alias`: map files from before TAB's events have spawners set to the
    /// constant spawn, which is gone – TAB's stragglers are events now.
    #[serde(alias = "Constant")]
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
    /// Somewhere in these regions (their `id`, `sim/regions.rs`): a random free tile of one of them, from which the base
    /// can be reached – the count once, spread over all of them. A region that is removed leaves the list
    /// (`World::forget_zone`); with none left nobody comes.
    Zones(Vec<u32>),
}

/// One group of an event: where it comes out – picked once, all its breeds come out there together – and how many of
/// each breed. (`breeds`, not `zombies`: in the map file the events come before the map's own field `zombies`, and
/// `mapfile::pretty` finds that one by where it first appears.)
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct WaveGroup {
    pub from: WaveFrom,
    pub breeds: Vec<BreedCount>,
}

/// So many zombies of one breed – from every side the group comes from (`WaveFrom::Sides`), or in total: `count`, or
/// a random number from `count` to `up_to` (TAB's "4-6") – `up_to` 0: just `count`.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct BreedCount {
    pub count: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub up_to: u32,
    pub breed: Breed,
}

impl BreedCount {
    pub fn new(count: u32, breed: Breed) -> BreedCount {
        BreedCount { count, up_to: 0, breed }
    }
}

/// For `skip_serializing_if`: no range isn't written.
fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// One of your units has fallen: it lies there and rises as a walker when `rise_in` runs out. A blow that threw it
/// (`fallen.rs`): it slides on a bit first.
#[derive(Clone)]
pub struct Corpse {
    pub pos: Vec2,
    /// Where it was the tick before – drawn in between.
    pub prev: Vec2,
    /// What it was, its number and which way it faced – only for drawing (its skin, its death).
    pub kind: UnitKind,
    pub id: u32,
    pub facing: f32,
    /// The tick it fell.
    pub died: u64,
    /// Ticks until it gets up.
    pub(super) rise_in: u32,
    pub(super) fling: Fling,
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
                let unit = &self.units[i];
                let (pos, kind, id, facing, fling) = (unit.pos, unit.kind, unit.id, unit.facing, unit.fling);
                self.corpses.push(Corpse { pos, prev: pos, kind, id, facing, died: self.tick, rise_in, fling });
                self.fx.push(Fx::UnitDied { at: pos, kind });
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

    /// Spawner `id` is gone: the events that named it don't any more.
    pub(super) fn forget_spawner(&mut self, id: u32) {
        for group in self.events.iter_mut().flat_map(|e| e.groups.iter_mut()) {
            if let WaveFrom::Spawners(ids) = &mut group.from {
                ids.retain(|&s| s != id);
            }
        }
    }

    /// Region `id` is gone: the events that named it don't any more.
    pub(super) fn forget_zone(&mut self, id: u32) {
        for group in self.events.iter_mut().flat_map(|e| e.groups.iter_mut()) {
            if let WaveFrom::Zones(ids) = &mut group.from {
                ids.retain(|&z| z != id);
            }
        }
    }

    /// What names region `id`: its events' groups, for the Regions module ("Wave (day 20), group 2") – deleting it
    /// takes it out of those.
    pub fn zone_uses(&self, id: u32) -> Vec<String> {
        let mut uses = Vec::new();
        for event in &self.events {
            for (g, group) in event.groups.iter().enumerate() {
                if matches!(&group.from, WaveFrom::Zones(ids) if ids.contains(&id)) {
                    uses.push(format!("{} (day {}), group {}", event.name, event.hour / 24 + 1, g + 1));
                }
            }
        }
        uses
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

    /// A group of a wave sent at once ("send now" in the editor): where it comes out is picked once (`wave_sides`) –
    /// every breed of it comes out there, one after the other, all of a side one swarm.
    pub(super) fn send_group(&mut self, group: &WaveGroup, final_wave: bool) {
        if let Some(zones) = self.wave_zones(&group.from) {
            let swarm = self.new_swarm();
            for z in &group.breeds {
                self.spawn_in_zones(&zones, z.count as usize, z.breed, final_wave, swarm);
            }
            return;
        }
        for side in self.wave_sides(&group.from) {
            let swarm = self.new_swarm();
            let spawners: Vec<(usize, u32)> = side.into_iter().map(|s| (s, swarm)).collect();
            for z in &group.breeds {
                self.spawn_from(&spawners, z.count as usize, z.breed, final_wave);
            }
        }
    }

    /// Where a group comes out (`WaveFrom`), the sides picked at random: a list of spawners (indices) per side – each
    /// list gets the group's full count (TAB's `N & S`). One random side (TAB's `N | S | E | W`) is `Sides(1)`. A
    /// single random spawner is one list too, the chosen spawners are one list – the count spread over them.
    pub(super) fn wave_sides(&mut self, from: &WaveFrom) -> Vec<Vec<usize>> {
        match from {
            WaveFrom::OneSpawner if self.spawners.is_empty() => Vec::new(),
            WaveFrom::OneSpawner => vec![vec![self.rng.below(self.spawners.len() as u32) as usize]],
            WaveFrom::Sides(sides) => {
                let map = &self.map;
                let mut left: Vec<u32> = (0..4).filter(|&side| self.spawners.iter().any(|s| s.side(map) == side)).collect();
                let mut lists = Vec::new();
                for _ in 0..(*sides).min(left.len() as u32) {
                    let side = left.remove(self.rng.below(left.len() as u32) as usize);
                    lists.push((0..self.spawners.len()).filter(|&s| self.spawners[s].side(&self.map) == side).collect());
                }
                lists
            }
            WaveFrom::Spawners(ids) => {
                let found: Vec<usize> = ids.iter().filter_map(|&id| self.find_spawner(id)).collect();
                if found.is_empty() { Vec::new() } else { vec![found] }
            }
            // Not from spawners at all (`wave_zones`).
            WaveFrom::Zones(_) => Vec::new(),
        }
    }

    /// The regions (`id`) a group comes out of – the ones that still exist; `None`: the group comes from spawners.
    pub(super) fn wave_zones(&self, from: &WaveFrom) -> Option<Vec<u32>> {
        match from {
            WaveFrom::Zones(ids) => Some(ids.iter().copied().filter(|&id| self.find_zone(id).is_some()).collect()),
            _ => None,
        }
    }

    /// A random tile in one of these regions (`id`) that isn't blocked and leads to the base: its middle, as a point –
    /// `None` after `SPAWN_TRIES` tries (all of them gone, covered with water, cut off). Only the world's dice.
    pub(super) fn zone_point(&mut self, zones: &[u32]) -> Option<Vec2> {
        let found: Vec<usize> = zones.iter().filter_map(|&id| self.find_zone(id)).collect();
        if found.is_empty() {
            return None;
        }
        for _ in 0..SPAWN_TRIES {
            let zone = &self.zones[found[self.rng.below(found.len() as u32) as usize]];
            let Some(area) = zone.bounds() else {
                continue;
            };
            let (w, h) = area.size();
            let tile = (area.min.0 + self.rng.below(w as u32) as i32, area.min.1 + self.rng.below(h as u32) as i32);
            if zone.contains(tile) && !self.map.is_blocked(tile.0, tile.1) && self.to_base.reachable(tile.0, tile.1) {
                return Some(vec2(tile.0 as f32 + self.rng.unit_f32(), tile.1 as f32 + self.rng.unit_f32()));
            }
        }
        None
    }

    /// `count` zombies of this breed in these regions, at once (`spawn_from`'s way for regions).
    pub(super) fn spawn_in_zones(&mut self, zones: &[u32], count: usize, breed: Breed, final_wave: bool, swarm: u32) {
        for _ in 0..count {
            let Some(pos) = self.zone_point(zones) else {
                break;
            };
            self.spawn_zombie_as(pos, breed, false);
            if let Some(zombie) = self.zombies.last_mut() {
                zombie.final_wave = final_wave;
                zombie.swarm = swarm;
            }
        }
    }

    /// A random free point in a random one of these spawners (indices), from which the base can be reached – and
    /// which one of the list it is. `None` if there are no spawners, or nothing was found after `SPAWN_TRIES` tries
    /// (base cut off, spawners painted over).
    pub(super) fn spawn_point(&mut self, spawners: &[usize]) -> Option<(Vec2, usize)> {
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

    /// The periodic spawners (editor): count the ticks, and when the time is up send their zombies (runners).
    pub(super) fn update_spawners(&mut self) {
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
