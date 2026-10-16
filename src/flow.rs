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
//! Who walks the field decides the rules (`Walker`): for units buildings and the world border are walls.
//! Not for zombies: they come from outside the border, and to them a building tile only costs a lot of extra
//! steps – if the detour is longer, their path goes right through it, and they tear the building down.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use macroquad::math::{vec2, Vec2};

use crate::map::Map;

const UNREACHABLE: u32 = u32::MAX;
/// What a step costs: straight 10, diagonal 14 (≈ 10 · √2). Whole numbers, so it's exactly the same on every PC.
const STRAIGHT: u32 = 10;
const DIAGONAL: u32 = 14;

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
    /// Per tile: the point to walk straight at (see the top of the file). Unreachable tiles: unused.
    anchor: Vec<Vec2>,
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
        let center = |i: usize| vec2((i % size) as f32 + 0.5, (i / size) as f32 + 0.5);

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
        // Free: open – or a goal (you may go there even if it's blocked).
        let free = |x: i32, y: i32| inside(x, y) && (is_goal[index(x, y)] || open(x, y));
        // What a straight step onto this tile costs. `None`: you can never get there (solid terrain, the edge, for
        // units also buildings and everything outside the border).
        let cost = |x: i32, y: i32| -> Option<u32> {
            if free(x, y) {
                Some(STRAIGHT)
            } else if !inside(x, y) {
                None
            } else {
                match walker {
                    // Not open, but no solid terrain either: a building. Zombies may go through.
                    Walker::Zombie { detour } if !map.is_solid(x, y) => Some(STRAIGHT * (1 + detour)),
                    _ => None,
                }
            }
        };

        // 1. Shortest paths from the target (Dijkstra): what does it cost from every tile to the target? The tile
        //    with the lowest cost always goes next – a "heap" (priority queue) takes care of that. `next[i]`: the
        //    neighbour that tile i's shortest way leads through.
        let mut dist = vec![UNREACHABLE; size * size];
        let mut next = vec![usize::MAX; size * size];
        let mut heap = BinaryHeap::new();
        for &(x, y) in goals {
            if inside(x, y) && dist[index(x, y)] == UNREACHABLE {
                dist[index(x, y)] = 0;
                heap.push(Reverse((0, x, y)));
            }
        }
        while let Some(Reverse((d, x, y))) = heap.pop() {
            if d > dist[index(x, y)] {
                continue; // outdated entry: the tile has been reached more cheaply in the meantime
            }
            for (dx, dy) in NEIGHBORS {
                let (nx, ny) = (x + dx, y + dy);
                let Some(mut step) = cost(nx, ny) else {
                    continue;
                };
                // Diagonal only between free tiles and if no corner is in the way (not even a building corner) –
                // nobody slips through the corner between two diagonal wall pieces.
                if dx != 0 && dy != 0 {
                    if !free(nx, ny) || !free(x + dx, y) || !free(x, y + dy) {
                        continue;
                    }
                    step = DIAGONAL;
                }
                if d + step < dist[index(nx, ny)] {
                    dist[index(nx, ny)] = d + step;
                    next[index(nx, ny)] = index(x, y);
                    heap.push(Reverse((d + step, nx, ny)));
                }
            }
        }

        // 2. Anchors, from the target outwards (by distance) – so the next tile on the way is always done already.
        //    The goal tiles lead to the target point itself.
        let mut order: Vec<usize> = (0..size * size).filter(|&i| dist[i] != UNREACHABLE).collect();
        order.sort_by_key(|&i| (dist[i], i));
        let mut anchor = vec![target; size * size];
        let sees = |from: Vec2, to: Vec2| map.line_is_clear(from, to, |x, y| !free(x, y));
        for i in order {
            if dist[i] == 0 {
                continue; // a goal tile: its anchor is the target
            }
            let (here, n) = (center(i), next[i]);
            // A building on the way (zombies): walk at the building – nothing can be seen through it.
            let n_is_free = free((n % size) as i32, (n / size) as i32);
            anchor[i] = if n_is_free && sees(here, anchor[n]) { anchor[n] } else { center(n) };
        }

        FlowField { size, target, dist, anchor }
    }

    /// Where to go from `pos`: straight at the anchor of its tile. Length 1 – or 0 when standing on the anchor
    /// or where there's no way.
    pub fn direction(&self, pos: Vec2) -> Vec2 {
        let (x, y) = (pos.x.floor() as i32, pos.y.floor() as i32);
        if !self.reachable(x, y) {
            return Vec2::ZERO;
        }
        (self.anchor[y as usize * self.size + x as usize] - pos).normalize_or_zero()
    }

    /// Is there any way at all from this tile to the target?
    pub fn reachable(&self, x: i32, y: i32) -> bool {
        self.inside(x, y) && self.dist[y as usize * self.size + x as usize] != UNREACHABLE
    }

    fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.size as i32 && y < self.size as i32
    }
}
