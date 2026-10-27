//! Lure field: the way on foot to a place zombies are heading for – a sound they heard, where they last saw one of
//! your units, a building they're after (`sim/sound.rs`, `sim/zombies.rs`). Computed like the hunt map (`hunt.rs`), only from this one place
//! outwards: around water and mountains, straight through your walls (a building tile costs a little extra – they
//! tear it down on the way, like storming zombies).
//!
//! Only in a square window around the place: a lured zombie heard or saw it from not far away. That keeps it small and
//! quick, so the simulation can keep one per place zombies are heading for. Places are whole cells
//! (`sound::CELL` tiles a side) – everybody lured into the same cell shares one field.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use macroquad::math::{vec2, Vec2};

use crate::flow::NEIGHBORS;
use crate::hunt::step_cost;
use crate::map::Map;

const UNSEEN: u16 = u16::MAX;

#[derive(Clone)]
pub struct LureField {
    /// The cell it leads to – its key in the simulation's list.
    pub cell: (i32, i32),
    /// The window: top left tile, and its edge in tiles.
    x0: i32,
    y0: i32,
    w: i32,
    /// Per tile of the window: steps to the cell, or `UNSEEN` (too far, no way).
    dist: Vec<u16>,
    /// What a step onto a building tile costs (`hunt::step_cost`).
    through: u16,
    /// The tick a zombie last needed it – unused for a while, it's thrown away.
    pub used: u64,
}

impl LureField {
    /// The way to `cell` (cells of `cell_size` tiles) from up to `reach` tiles around it.
    pub fn new(map: &Map, cell: (i32, i32), cell_size: i32, reach: i32, through: u16, tick: u64) -> LureField {
        let (x0, y0) = (cell.0 * cell_size - reach, cell.1 * cell_size - reach);
        let w = cell_size + 2 * reach;
        let mut field = LureField { cell, x0, y0, w, dist: vec![UNSEEN; (w * w) as usize], through, used: tick };
        // The goals: every tile of the cell a zombie can stand on. All of them taken (a building fills the cell): the
        // building tiles themselves – the zombies come and tear it down.
        let tiles: Vec<(i32, i32)> =
            (0..cell_size * cell_size).map(|k| (cell.0 * cell_size + k % cell_size, cell.1 * cell_size + k / cell_size)).collect();
        let open: Vec<(i32, i32)> = tiles.iter().copied().filter(|&(x, y)| !map.is_blocked(x, y)).collect();
        let goals = if open.is_empty() { tiles.into_iter().filter(|&(x, y)| step_cost(map, through, x, y).is_some()).collect() } else { open };
        let mut heap = BinaryHeap::new();
        for (x, y) in goals {
            if let Some(i) = field.index(x, y) {
                field.dist[i] = 0;
                heap.push(Reverse((0u16, x, y)));
            }
        }
        // Dijkstra outwards, like `HuntMap::rebuild` – in the order of the queue, the same on every PC.
        while let Some(Reverse((d, x, y))) = heap.pop() {
            if field.index(x, y).is_none_or(|i| d > field.dist[i]) {
                continue;
            }
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                let (Some(next), Some(step)) = (field.index(nx, ny), step_cost(map, through, nx, ny)) else {
                    continue;
                };
                if d + step < field.dist[next] {
                    field.dist[next] = d + step;
                    heap.push(Reverse((d + step, nx, ny)));
                }
            }
        }
        field
    }

    /// The direction of the next step from `pos` towards the cell – `ZERO` once it's in the cell. `None` outside the
    /// window or without a way: then the zombie walks straight at it. The step may lead onto a building – that's the
    /// one to tear down.
    pub fn step(&self, map: &Map, pos: Vec2) -> Option<Vec2> {
        let (x, y) = (pos.x.floor() as i32, pos.y.floor() as i32);
        let here = self.dist[self.index(x, y)?];
        if here == UNSEEN {
            return None;
        }
        let mut best = (here, Vec2::ZERO);
        for (dx, dy) in NEIGHBORS {
            let (nx, ny) = (x + dx, y + dy);
            let Some(i) = self.index(nx, ny) else {
                continue;
            };
            // Diagonal only onto free ground and if no corner is in the way – like the flow field and the hunt map.
            let diagonal = dx != 0 && dy != 0;
            if diagonal && (map.is_blocked(nx, ny) || map.is_blocked(x + dx, y) || map.is_blocked(x, y + dy)) {
                continue;
            }
            if self.dist[i] < best.0 && step_cost(map, self.through, nx, ny).is_some() {
                best = (self.dist[i], vec2(dx as f32, dy as f32).normalize());
            }
        }
        Some(best.1)
    }

    /// Index of a tile in the window – `None` outside it.
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let (lx, ly) = (x - self.x0, y - self.y0);
        (lx >= 0 && ly >= 0 && lx < self.w && ly < self.w).then(|| (ly * self.w + lx) as usize)
    }
}
