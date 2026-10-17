//! Hunt map: how far is it from every tile **on foot** to the nearest unit – around solid terrain? The threat map
//! (`aggro.rs`) measures straight-line distance; there a unit across a lake is just as close as one in front of
//! you. Not here: the way leads around the water.
//!
//! There are two of them, and they differ in what a building is (`Through`):
//! - for the few zombies **looking for a gap** (shot, or next to one that was – DESIGN.md 8.1) buildings are walls –
//!   whoever is walled in can only be reached through the gap,
//! - for the **charge** of all the other zombies a building only costs a little extra: their way goes around
//!   water and mountains, but straight through walls – so they storm the wall instead of looking for the gap.
//!
//! It's computed like the flow field, just the other way round: Dijkstra starts at all units at once and stops
//! at `max_cost`. That's cheap because there are only a few units and the radius is small.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use macroquad::math::{vec2, Vec2};

use crate::flow::NEIGHBORS;
use crate::map::Map;

const UNSEEN: u16 = u16::MAX;

/// What a building tile is on the way to a unit.
#[derive(Clone, Copy, PartialEq)]
pub enum Through {
    /// A wall: no way through (looking for a gap).
    Never,
    /// Passable, a step onto it costs this much instead of 1 (charge: tear it down on the way).
    Cost(u16),
}

pub struct HuntMap {
    size: usize,
    through: Through,
    /// Per tile: cost to the nearest unit, or `UNSEEN` (too far / unreachable).
    dist: Vec<u16>,
    /// Per tile: which unit that is (index in the unit list).
    unit: Vec<u32>,
    /// The queue of the search – reused every tick instead of created anew.
    heap: BinaryHeap<Reverse<(u16, i32, i32)>>,
}

impl HuntMap {
    pub fn new(size: usize, through: Through) -> HuntMap {
        HuntMap { size, through, dist: vec![UNSEEN; size * size], unit: vec![0; size * size], heap: BinaryHeap::new() }
    }

    /// Recompute (once per tick): search from all units, at most `max_cost` far.
    pub fn rebuild(&mut self, map: &Map, positions: impl Iterator<Item = Vec2>, max_cost: u16) {
        self.dist.fill(UNSEEN);
        self.heap.clear();
        for (i, pos) in positions.enumerate() {
            let (x, y) = (pos.x.floor() as i32, pos.y.floor() as i32);
            // Two units on one tile: the one entered first stays (DESIGN.md 12.8).
            // A unit in a tower stands on a building: only the charge can get there.
            if self.cost(map, x, y).is_none() || self.dist[self.index(x, y)] == 0 {
                continue;
            }
            let tile = self.index(x, y);
            self.dist[tile] = 0;
            self.unit[tile] = i as u32;
            self.heap.push(Reverse((0, x, y)));
        }
        while let Some(Reverse((d, x, y))) = self.heap.pop() {
            let here = self.index(x, y);
            if d > self.dist[here] || d >= max_cost {
                continue; // outdated entry, or far enough
            }
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                let Some(step) = self.cost(map, nx, ny) else {
                    continue;
                };
                let next = self.index(nx, ny);
                if d + step < self.dist[next] {
                    self.dist[next] = d + step;
                    self.unit[next] = self.unit[here];
                    self.heap.push(Reverse((d + step, nx, ny)));
                }
            }
        }
    }

    /// The unit that is closest on foot from here, and the direction of the next step towards it.
    /// Direction 0: already on the same tile. `None`: no unit within range (or no way).
    /// For the charge the step can lead onto a building – that's the one to tear down.
    pub fn track(&self, map: &Map, pos: Vec2) -> Option<(usize, Vec2)> {
        let (x, y) = (pos.x.floor() as i32, pos.y.floor() as i32);
        if map.is_blocked(x, y) {
            return None;
        }
        let tile = self.index(x, y);
        let here = self.dist[tile];
        if here == UNSEEN {
            return None;
        }
        let mut best = (here, Vec2::ZERO);
        for (dx, dy) in NEIGHBORS {
            let (nx, ny) = (x + dx, y + dy);
            if self.cost(map, nx, ny).is_none() {
                continue;
            }
            // Diagonal only onto free ground and if no corner is in the way – like in the flow field.
            let diagonal = dx != 0 && dy != 0;
            if diagonal && (map.is_blocked(nx, ny) || map.is_blocked(x + dx, y) || map.is_blocked(x, y + dy)) {
                continue;
            }
            let d = self.dist[self.index(nx, ny)];
            if d < best.0 {
                best = (d, vec2(dx as f32, dy as f32).normalize());
            }
        }
        Some((self.unit[tile] as usize, best.1))
    }

    /// What a step onto this tile costs – `None` if you can't go there at all (solid terrain, off the map,
    /// when looking for a gap also buildings).
    fn cost(&self, map: &Map, x: i32, y: i32) -> Option<u16> {
        if !map.is_blocked(x, y) {
            return Some(1);
        }
        match self.through {
            // Blocked, but no solid terrain and on the map: a building.
            Through::Cost(cost) if !map.is_solid(x, y) => Some(cost),
            _ => None,
        }
    }

    /// Only call for tiles on the map (`cost` is `None` outside, that protects us).
    fn index(&self, x: i32, y: i32) -> usize {
        y as usize * self.size + x as usize
    }
}
