//! Building blocks the generators share: the grid of tiles they paint on, rivers, single trees, and the step that
//! connects every open place to the base.

use std::collections::VecDeque;

use macroquad::math::{vec2, Vec2};

use super::{Context, Field, Show};
use crate::map::Terrain;
use crate::noise::Noise;
use crate::rng::Rng;

/// The tiles a generator paints on – a square of terrain, everything grass at the start.
pub struct Tiles {
    pub size: i32,
    terrain: Vec<Terrain>,
}

impl Tiles {
    pub fn new(size: usize) -> Tiles {
        Tiles { size: size as i32, terrain: vec![Terrain::Grass; size * size] }
    }

    pub fn center(&self) -> (i32, i32) {
        (self.size / 2, self.size / 2)
    }

    pub fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.size && y < self.size
    }

    /// Outside the map counts as mountain – like `Map::terrain`.
    pub fn get(&self, x: i32, y: i32) -> Terrain {
        if self.inside(x, y) { self.terrain[(y * self.size + x) as usize] } else { Terrain::Mountain }
    }

    /// Outside the map nothing happens.
    pub fn set(&mut self, x: i32, y: i32, terrain: Terrain) {
        if self.inside(x, y) {
            self.terrain[(y * self.size + x) as usize] = terrain;
        }
    }

    /// All tiles, row by row: (0, 0), (1, 0), … – the same order as the list inside.
    pub fn all(&self) -> impl Iterator<Item = (i32, i32)> + use<> {
        let size = self.size;
        (0..size).flat_map(move |y| (0..size).map(move |x| (x, y)))
    }

    pub fn clear_rect(&mut self, min: (i32, i32), size: (i32, i32)) {
        for y in min.1..min.1 + size.1 {
            for x in min.0..min.0 + size.0 {
                self.set(x, y, Terrain::Grass);
            }
        }
    }

    /// Grass on every tile whose middle lies within `radius` of `center`.
    pub fn clear_circle(&mut self, center: Vec2, radius: f32) {
        self.disc(center, radius, Terrain::Grass);
    }

    /// Paints every tile whose middle lies within `radius` of `center`.
    pub fn disc(&mut self, center: Vec2, radius: f32, terrain: Terrain) {
        let r = radius.ceil() as i32 + 1;
        let (cx, cy) = (center.x.floor() as i32, center.y.floor() as i32);
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                if vec2(x as f32 + 0.5, y as f32 + 0.5).distance(center) <= radius {
                    self.set(x, y, terrain);
                }
            }
        }
    }

    pub fn into_terrain(self) -> Vec<Terrain> {
        self.terrain
    }
}

/// 0 below `lo`, 1 above `hi`, and an "S" in between (smooth at both ends).
pub fn smoothstep(lo: f32, hi: f32, v: f32) -> f32 {
    let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The value at the given share of all values: `level(values, 0.1)` = the value 10 % of them lie below.
/// So "10 % water" stays 10 %, whatever the noise gave this time.
pub fn level(values: &[f32], share: f32) -> f32 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f32::total_cmp);
    sorted[((sorted.len() - 1) as f32 * share.clamp(0.0, 1.0)) as usize]
}

/// Which terrain a wall gets: material 0 mountains, 1 forest, 3 water; 2 (mixed) rolls mountains or forest, now and
/// then water.
pub fn material(choice: f32, rng: &mut Rng) -> Terrain {
    match choice.round() as i32 {
        0 => Terrain::Mountain,
        1 => Terrain::Forest,
        3 => Terrain::Water,
        _ => match rng.below(10) {
            0..=4 => Terrain::Mountain,
            5..=8 => Terrain::Forest,
            _ => Terrain::Water,
        },
    }
}

/// The slider for single trees – nearly every generator has it.
pub fn trees_field(value: &mut f32) -> Field<'_> {
    Field {
        label: "Single trees",
        value,
        min: 0.0,
        max: 0.10,
        step: 0.005,
        show: Show::Percent,
        help: "Chance for a lone tree on open grass - more next to forests. Trees block walking, sight and shots.",
    }
}

/// Single trees on open grass, `density` = chance per tile, three times as likely right next to a forest.
pub fn scatter_trees(tiles: &mut Tiles, rng: &mut Rng, density: f32) {
    if density <= 0.0 {
        return;
    }
    for (x, y) in tiles.all() {
        if tiles.get(x, y) != Terrain::Grass {
            continue;
        }
        let forest_near = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| tiles.get(x + dx, y + dy) == Terrain::Forest);
        let chance = if forest_near { density * 3.0 } else { density };
        if rng.unit_f32() < chance {
            tiles.set(x, y, Terrain::Tree);
        }
    }
}

/// A smooth random shift of a point by up to `amount` tiles ("domain warping"): look up the noise somewhere else
/// than where you stand, and round blobs become frayed, stretched, organic shapes.
pub struct Warp {
    x: Noise,
    y: Noise,
    scale: f32,
    amount: f32,
}

impl Warp {
    pub fn new(rng: &mut Rng, scale: f32, amount: f32) -> Warp {
        Warp { x: Noise::new(rng.next_u64() as u32), y: Noise::new(rng.next_u64() as u32), scale, amount }
    }

    pub fn apply(&self, x: f32, y: f32) -> Vec2 {
        if self.amount <= 0.0 {
            return vec2(x, y);
        }
        let dx = (self.x.fractal(x, y, self.scale, 3) - 0.5) * 2.0 * self.amount;
        let dy = (self.y.fractal(x, y, self.scale, 3) - 0.5) * 2.0 * self.amount;
        vec2(x + dx, y + dy)
    }
}

/// A river from one edge of the map to another: it starts at a random point on a random side and flows towards a
/// point on another side, bending left and right (`meander` 0 = nearly straight, 1 = big bends). Near the base it
/// is pushed outwards, so it flows around the free space instead of being cut off by it.
///
/// No angles: the direction is a vector – "towards the end" plus a bit of "sideways", then shortened to length 1
/// (`normalize`, only `sqrt`). That keeps it exact on every PC.
pub fn river(tiles: &mut Tiles, rng: &mut Rng, ctx: &Context, width: f32, meander: f32) {
    let n = tiles.size as f32;
    let edge_point = |side: u32, t: f32| match side {
        0 => vec2(t * n, 0.0),
        1 => vec2(t * n, n),
        2 => vec2(0.0, t * n),
        _ => vec2(n, t * n),
    };
    let from_side = rng.below(4);
    // Mostly to the opposite side (0 ↔ 1, 2 ↔ 3), sometimes round the corner.
    let opposite = from_side ^ 1;
    let to_side = if rng.below(3) == 0 { (from_side + 2) % 4 } else { opposite };
    let mut pos = edge_point(from_side, 0.15 + 0.7 * rng.unit_f32());
    let end = edge_point(to_side, 0.15 + 0.7 * rng.unit_f32());
    let bend = Noise::new(rng.next_u64() as u32);
    let avoid = ctx.keep_out + 10.0;
    let step = 0.5;
    let max_steps = (n * 8.0) as usize;
    for k in 0..max_steps {
        let to_end = (end - pos).normalize_or_zero();
        let side = vec2(-to_end.y, to_end.x);
        // The bend: slow noise along the way, -1 … 1.
        let wiggle = (bend.fractal(k as f32 * step, 0.0, 40.0, 3) - 0.5) * 4.0 * meander;
        let mut dir = to_end + side * wiggle;
        // Pushed away from the base, the closer the stronger.
        let from_base = pos - ctx.center;
        let d = from_base.length();
        if d < avoid && d > 0.0 {
            dir += from_base / d * (3.0 * (avoid - d) / avoid);
        }
        pos += dir.normalize_or_zero() * step;
        tiles.disc(pos, width / 2.0, Terrain::Water);
        let outside = pos.x < -1.0 || pos.y < -1.0 || pos.x > n + 1.0 || pos.y > n + 1.0;
        if pos.distance(end) < 1.0 || (k > 20 && outside) {
            break;
        }
    }
}

/// Makes sure every open place (grass) can be reached from `from` on foot: an open place that is cut off – by a lake,
/// a wall of rock, the ring of a fortress – gets the shortest possible path through whatever is in the way, 2 tiles
/// wide. Those paths are natural chokepoints (a ford through a river, a gap in a ridge). Open places smaller than
/// `min_pocket` tiles are filled with forest instead.
///
/// Walking counts only straight steps (4 neighbours): stricter than the units, who also go diagonally – so whatever
/// is connected here is certainly connected in the game.
pub fn connect_pockets(tiles: &mut Tiles, from: (i32, i32), min_pocket: usize) {
    let n = tiles.size;
    let index = |x: i32, y: i32| (y * n + x) as usize;
    let mut reached = vec![false; (n * n) as usize];
    flood(tiles, from, &mut reached);
    let mut pocket_seen = vec![false; (n * n) as usize];
    for (x, y) in tiles.all() {
        let i = index(x, y);
        if reached[i] || pocket_seen[i] || tiles.get(x, y).is_solid() {
            continue;
        }
        // A new open place that isn't connected. Which tiles belong to it?
        let mut pocket = vec![false; (n * n) as usize];
        let cells = flood(tiles, (x, y), &mut pocket);
        for &(px, py) in &cells {
            pocket_seen[index(px, py)] = true;
        }
        if cells.len() < min_pocket {
            for &(px, py) in &cells {
                tiles.set(px, py, Terrain::Forest);
            }
            continue;
        }
        // From all of its tiles at once, through anything, until a reached tile turns up: that's the shortest way.
        let mut came_from: Vec<u32> = vec![u32::MAX; (n * n) as usize];
        let mut queue: VecDeque<(i32, i32)> = cells.iter().copied().collect();
        for &(px, py) in &cells {
            came_from[index(px, py)] = index(px, py) as u32;
        }
        let mut found = None;
        while let Some((cx, cy)) = queue.pop_front() {
            if reached[index(cx, cy)] {
                found = Some((cx, cy));
                break;
            }
            for (nx, ny) in [(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)] {
                if tiles.inside(nx, ny) && came_from[index(nx, ny)] == u32::MAX {
                    came_from[index(nx, ny)] = index(cx, cy) as u32;
                    queue.push_back((nx, ny));
                }
            }
        }
        let Some(mut at) = found else {
            continue;
        };
        // Walk back and clear the way, 2 tiles wide (a 1-tile path would be too easy to miss and to block).
        let mut path = Vec::new();
        loop {
            for tile in [at, (at.0 + 1, at.1), (at.0, at.1 + 1)] {
                tiles.set(tile.0, tile.1, Terrain::Grass);
                path.push(tile);
            }
            let prev = came_from[index(at.0, at.1)] as i32;
            let prev = (prev % n, prev / n);
            if prev == at {
                break;
            }
            at = prev;
        }
        // Now the path, this pocket and whatever else the path opened belong to the reached part. (Flooding from
        // `from` again wouldn't do: it is marked already, so the flood would stop right there.)
        for (px, py) in path {
            if tiles.inside(px, py) {
                flood(tiles, (px, py), &mut reached);
            }
        }
    }
}

/// Marks every open tile reachable from `start` (straight steps) in `seen` and returns the newly marked ones.
fn flood(tiles: &Tiles, start: (i32, i32), seen: &mut [bool]) -> Vec<(i32, i32)> {
    let n = tiles.size;
    let mut found = Vec::new();
    if tiles.get(start.0, start.1).is_solid() || seen[(start.1 * n + start.0) as usize] {
        return found;
    }
    let mut stack = vec![start];
    seen[(start.1 * n + start.0) as usize] = true;
    while let Some((x, y)) = stack.pop() {
        found.push((x, y));
        for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if tiles.inside(nx, ny) && !tiles.get(nx, ny).is_solid() && !seen[(ny * n + nx) as usize] {
                seen[(ny * n + nx) as usize] = true;
                stack.push((nx, ny));
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grass room walled in by rock gets a path to the base; a tiny hole gets filled.
    #[test]
    fn closed_rooms_get_a_path_tiny_holes_get_filled() {
        let mut tiles = Tiles::new(40);
        for y in 0..40 {
            for x in 0..40 {
                // A wall all around the room 25..35 × 25..35, and a one-tile hole at (5, 5).
                let wall = (24..=35).contains(&x) && (24..=35).contains(&y) && !((25..35).contains(&x) && (25..35).contains(&y));
                if wall || (4..=6).contains(&x) && (4..=6).contains(&y) && (x, y) != (5, 5) {
                    tiles.set(x, y, Terrain::Mountain);
                }
            }
        }
        connect_pockets(&mut tiles, (10, 10), 5);
        let mut seen = vec![false; 1600];
        flood(&tiles, (10, 10), &mut seen);
        assert!(seen[30 * 40 + 30], "the room is connected");
        assert_eq!(tiles.get(5, 5), Terrain::Forest, "the hole is filled");
        // The path is short: only a few wall tiles were opened.
        let opened = (24..=35).flat_map(|y| (24..=35).map(move |x| (x, y))).filter(|&(x, y)| {
            let wall = !((25..35).contains(&x) && (25..35).contains(&y));
            wall && tiles.get(x, y) == Terrain::Grass
        });
        assert!(opened.count() <= 4);
    }
}
