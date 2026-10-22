//! Flow field (DESIGN.md 12.1): computed once per target, after that every zombie (or unit) only reads its own
//! tile. It's a **line-of-sight flow field** (the idea of Theta*, an "any-angle" path finder):
//!
//! 1. **Distance** from every tile to the target (Dijkstra with 8 neighbours: a straight step costs 10, a diagonal
//!    one 14 ≈ 10 · √2) – so the way around an obstacle really is the shortest one, not just any staircase.
//! 2. **Anchor** instead of an arrow: every tile remembers the furthest point on its way that it can still see in a
//!    straight line. If it sees the anchor of the next tile on its way, it takes that one over – otherwise the next
//!    tile itself becomes its anchor. Whoever follows the field walks straight at the anchor: in any direction,
//!    taut from corner to corner, instead of only in 8 directions (first diagonally, then straight – the old
//!    staircase).
//!
//! **Anchors are only worked out when someone stands on the tile** (and then kept). Working them out for all
//! 83,000 tiles of a map took most of the time of a command (2026-10-17: 95 ms in the debug build, a visible
//! hitch) – and a group walks over a few hundred of them. The result is the same, only computed later.
//!
//! Who walks the field decides the rules (`Walker`): for units buildings and the world border are walls.
//! Not for zombies: they come from outside the border, and to them a building tile only costs a lot of extra
//! steps – if the detour is longer, their path goes right through it, and they tear the building down.

use std::sync::OnceLock;

use macroquad::math::{vec2, Vec2};

use crate::map::Map;

const UNREACHABLE: u32 = u32::MAX;
/// What a step costs: straight 10, diagonal 14 (≈ 10 · √2). Whole numbers, so it's exactly the same on every PC.
const STRAIGHT: u32 = 10;
const DIAGONAL: u32 = 14;
/// "No next tile" in `FlowField::next` (goal tiles and unreachable ones).
const NONE: u32 = u32::MAX;

/// The 8 neighbours of a tile. The fixed order decides ties.
pub const NEIGHBORS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

/// Who walks a flow field – units and zombies follow different rules.
#[derive(Clone, Copy)]
pub enum Walker {
    /// Your units: buildings are walls, and they stay inside the world border.
    Unit,
    /// Zombies: the world border doesn't matter to them, and a building tile costs `detour` extra steps.
    Zombie { detour: u32 },
}

pub struct FlowField {
    size: usize,
    /// The exact target point (not just the tile).
    target: Vec2,
    /// Cost to the target, per tile (a straight step costs `STRAIGHT`).
    dist: Vec<u32>,
    /// Per tile: the neighbour its shortest way leads through (`NONE` for goal tiles and unreachable ones).
    next: Vec<u32>,
    /// Per tile: could the walker go there freely when the field was made? The anchors need it later – and the map
    /// may have changed by then (a field is renewed when it does, but it shouldn't depend on that).
    free: Vec<bool>,
    /// Per tile: the point to walk straight at (see the top of the file) – empty until someone asks.
    /// `OnceLock`: it's filled in while the field is only being read (`&self`) – even by several threads at once (the
    /// zombies think side by side, `sim/zombies.rs`). Each of them works out the same point; the first one keeps it.
    anchor: Vec<OnceLock<Vec2>>,
}

impl FlowField {
    /// For a unit, to a point. If it lies on a blocked tile, nothing is reachable.
    pub fn new(map: &Map, target: Vec2) -> FlowField {
        let tile = (target.x.floor() as i32, target.y.floor() as i32);
        let goals: Vec<(i32, i32)> = if map.blocks_unit(tile.0, tile.1) { Vec::new() } else { vec![tile] };
        FlowField::build(map, &goals, target, Walker::Unit)
    }

    /// To a whole area, e.g. all tiles of a building – even if they are blocked themselves.
    /// Whoever arrives stands right in front of it. `target`: the point the goal tiles lead to.
    pub fn to_area(map: &Map, goals: &[(i32, i32)], target: Vec2, walker: Walker) -> FlowField {
        FlowField::build(map, goals, target, walker)
    }

    /// The point this field leads to.
    pub fn target(&self) -> Vec2 {
        self.target
    }

    fn build(map: &Map, goals: &[(i32, i32)], target: Vec2, walker: Walker) -> FlowField {
        let size = map.size;
        let index = |x: i32, y: i32| y as usize * size + x as usize;
        let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < size as i32 && y < size as i32;

        let mut is_goal = vec![false; size * size];
        for &(x, y) in goals {
            if inside(x, y) {
                is_goal[index(x, y)] = true;
            }
        }
        // Open: walkable for this walker without tearing anything down.
        let open = |x: i32, y: i32| match walker {
            Walker::Unit => !map.blocks_unit(x, y),
            Walker::Zombie { .. } => !map.is_blocked(x, y),
        };
        // Free: open – or a goal (you may go there even if it's blocked). Once per tile, kept for the anchors.
        let free: Vec<bool> = (0..size * size).map(|i| is_goal[i] || open((i % size) as i32, (i / size) as i32)).collect();
        let is_free = |x: i32, y: i32| inside(x, y) && free[index(x, y)];
        // What a straight step onto this tile costs. `None`: you can never get there (solid terrain, the edge, for
        // units also buildings and everything outside the border).
        let detour = match walker {
            Walker::Zombie { detour } => detour,
            Walker::Unit => 0,
        };
        let cost = |x: i32, y: i32| -> Option<u32> {
            if is_free(x, y) {
                Some(STRAIGHT)
            } else if !inside(x, y) {
                None
            } else {
                match walker {
                    // Not open, but no solid terrain either: a building. Zombies may tear their way through. (An
                    // infected one is open anyway – its walls are broken.)
                    Walker::Zombie { .. } if map.can_tear_down(x, y) => Some(STRAIGHT * (1 + detour)),
                    _ => None,
                }
            }
        };

        // 1. Shortest paths from the target (Dijkstra): what does it cost from every tile to the target? The tile
        //    with the lowest cost always goes next. Steps only cost a few whole numbers (10, 14, through a building
        //    more), so instead of a sorted heap there's a "bucket" per cost: all tiles with cost d wait in bucket
        //    d % buckets – and the buckets are emptied in turn (Dial's algorithm). Much faster than a heap.
        //    `next[i]`: the neighbour that tile i's shortest way leads through.
        let max_step = DIAGONAL.max(STRAIGHT * (1 + detour));
        let buckets_n = max_step as usize + 1;
        let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); buckets_n];
        let mut dist = vec![UNREACHABLE; size * size];
        let mut next = vec![NONE; size * size];
        let mut waiting = 0usize;
        for &(x, y) in goals {
            if inside(x, y) && dist[index(x, y)] == UNREACHABLE {
                dist[index(x, y)] = 0;
                buckets[0].push(index(x, y) as u32);
                waiting += 1;
            }
        }
        let mut d = 0u32;
        while waiting > 0 {
            let bucket = std::mem::take(&mut buckets[d as usize % buckets_n]);
            waiting -= bucket.len();
            for i in bucket {
                let i = i as usize;
                if dist[i] != d {
                    continue; // outdated entry: the tile has been reached more cheaply in the meantime
                }
                let (x, y) = ((i % size) as i32, (i / size) as i32);
                for (dx, dy) in NEIGHBORS {
                    let (nx, ny) = (x + dx, y + dy);
                    let Some(mut step) = cost(nx, ny) else {
                        continue;
                    };
                    // Diagonal only between free tiles and if no corner is in the way (not even a building corner) –
                    // nobody slips through the corner between two diagonal wall pieces.
                    if dx != 0 && dy != 0 {
                        if !is_free(nx, ny) || !is_free(x + dx, y) || !is_free(x, y + dy) {
                            continue;
                        }
                        step = DIAGONAL;
                    }
                    let n = index(nx, ny);
                    if d + step < dist[n] {
                        dist[n] = d + step;
                        next[n] = i as u32;
                        buckets[(d + step) as usize % buckets_n].push(n as u32);
                        waiting += 1;
                    }
                }
            }
            d += 1;
        }

        // 2. Anchors: the goal tiles lead to the target point itself – the rest is worked out when asked (`anchor`).
        let anchor = (0..size * size)
            .map(|i| match dist[i] {
                0 => OnceLock::from(target),
                _ => OnceLock::new(),
            })
            .collect();
        FlowField { size, target, dist, next, free, anchor }
    }

    /// The anchor of tile `i` (reachable): kept if it was worked out before. Otherwise follow the way to the first
    /// tile that already has one, and work back from there – each tile sees the anchor of the next one, or the next
    /// tile itself becomes its anchor. Always the same result, whoever asks first.
    fn anchor(&self, i: usize) -> Vec2 {
        if let Some(&a) = self.anchor[i].get() {
            return a;
        }
        let mut chain = vec![i];
        let mut j = self.next[i] as usize;
        while self.anchor[j].get().is_none() {
            chain.push(j);
            j = self.next[j] as usize;
        }
        let center = |k: usize| vec2((k % self.size) as f32 + 0.5, (k / self.size) as f32 + 0.5);
        let blocked = |x: i32, y: i32| {
            let inside = x >= 0 && y >= 0 && x < self.size as i32 && y < self.size as i32;
            !inside || !self.free[y as usize * self.size + x as usize]
        };
        for &k in chain.iter().rev() {
            let n = self.next[k] as usize;
            let after = self.anchor[n].get().copied().unwrap_or(self.target);
            // A building on the way (zombies): walk at the building – nothing can be seen through it.
            let a = if self.free[n] && Map::line_is_clear(center(k), after, blocked) { after } else { center(n) };
            // Another thread was quicker: it has put the same point there already.
            let _ = self.anchor[k].set(a);
        }
        self.anchor[i].get().copied().unwrap_or(self.target)
    }

    /// Where to go from `pos`: straight at the anchor of its tile. Length 1 – or 0 when standing on the anchor
    /// or where there's no way.
    pub fn direction(&self, pos: Vec2) -> Vec2 {
        let (x, y) = (pos.x.floor() as i32, pos.y.floor() as i32);
        if !self.reachable(x, y) {
            return Vec2::ZERO;
        }
        (self.anchor(y as usize * self.size + x as usize) - pos).normalize_or_zero()
    }

    /// Is there any way at all from this tile to the target?
    pub fn reachable(&self, x: i32, y: i32) -> bool {
        self.inside(x, y) && self.dist[y as usize * self.size + x as usize] != UNREACHABLE
    }

    fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.size as i32 && y < self.size as i32
    }
}

