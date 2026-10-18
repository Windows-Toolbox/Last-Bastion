//! Building blocks the generators share: the grid of tiles they paint on, rivers, single trees, and the step that
//! connects every open place to the base.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

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

    /// Grass on every tile outside the square `lo` … `hi` (on both axes, `hi` itself is outside) – the ring outside
    /// the world border.
    pub fn clear_outside(&mut self, lo: i32, hi: i32) {
        for (x, y) in self.all() {
            if x < lo || y < lo || x >= hi || y >= hi {
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

/// A random direction: a random point in the square -1 … 1, but only if it lies in the circle (otherwise the
/// corners would be twice as likely), shortened to length 1. No angles, so no `sin`/`cos`.
pub fn random_direction(rng: &mut Rng) -> Vec2 {
    loop {
        let v = vec2(rng.unit_f32() * 2.0 - 1.0, rng.unit_f32() * 2.0 - 1.0);
        let len = v.length();
        if len > 0.1 && len <= 1.0 {
            return v / len;
        }
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

/// Like `material`, but for "mixed" (2) per tile from a noise: patches of rock and forest along the same wall, now
/// and then a bit of water – a wall that looks grown, not painted with one brush. The others stay one material.
pub fn material_at(choice: f32, fixed: Terrain, mix: &Noise, x: i32, y: i32) -> Terrain {
    if choice.round() as i32 != 2 {
        return fixed;
    }
    let v = mix.fractal(x as f32, y as f32, 18.0, 3);
    if v < 0.28 {
        Terrain::Water
    } else if v < 0.55 {
        Terrain::Mountain
    } else {
        Terrain::Forest
    }
}

/// Blobs of rock, forest or water smaller than `max` tiles become grass again – crumbs that only get in the way.
/// Single trees stay (they are meant to be single).
pub fn remove_specks(tiles: &mut Tiles, max: usize) {
    let n = tiles.size;
    let mut seen = vec![false; (n * n) as usize];
    for (x, y) in tiles.all() {
        let t = tiles.get(x, y);
        if seen[(y * n + x) as usize] || !t.is_solid() || t == Terrain::Tree {
            continue;
        }
        // All connected tiles of the same terrain.
        let mut blob = vec![(x, y)];
        let mut stack = vec![(x, y)];
        seen[(y * n + x) as usize] = true;
        while let Some((bx, by)) = stack.pop() {
            for (nx, ny) in [(bx + 1, by), (bx - 1, by), (bx, by + 1), (bx, by - 1)] {
                if tiles.inside(nx, ny) && !seen[(ny * n + nx) as usize] && tiles.get(nx, ny) == t {
                    seen[(ny * n + nx) as usize] = true;
                    stack.push((nx, ny));
                    blob.push((nx, ny));
                }
            }
        }
        if blob.len() < max {
            for (bx, by) in blob {
                tiles.set(bx, by, Terrain::Grass);
            }
        }
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

/// Makes sure every open place (grass) can be reached from `from` on foot, in two passes:
/// 1. **Inside** the world border (`inside`), only through tiles inside it – that's where your units walk.
/// 2. **Everywhere**, the ring outside the border too – the zombies' way in. Since the ring is open ground (`generate`)
///    and every way in starts at the border, there is normally nothing left to do here; it's the guarantee.
///
/// In this order: before 2026-10-17 it was the other way round, and a meadow closed in by forest got two cuts – first
/// the cheapest one out to the ring, then a second one inside for your units. Filling a small pocket is safe in both
/// passes now: the ring is open all around, so no pocket inside is anyone's only way in.
///
/// A cut-off place gets the way that cuts through **as little as possible** – a ford through a river, a gap in a
/// ridge, a trail through a forest – 3 tiles wide where it cuts. How much a blocked tile "costs" changes slowly over
/// the map (`rough`), so the way winds along the thinner, cheaper parts instead of running along a grid line (cut
/// with a ruler, it looked like one). Open places smaller than `min_pocket` tiles are filled with forest instead.
/// Since the ways in (`lanes.rs`) this is only the safety net for what the generators still close off. Returns how many
/// blocked tiles it opened.
///
/// Walking counts only straight steps (4 neighbours): stricter than the units, who also go diagonally – so whatever
/// is connected here is certainly connected in the game.
pub fn connect_pockets(tiles: &mut Tiles, from: (i32, i32), min_pocket: usize, rough: &Noise, inside: impl Fn(i32, i32) -> bool) -> usize {
    connect_area(tiles, from, min_pocket, rough, &inside) + connect_area(tiles, from, min_pocket, rough, &|_, _| true)
}

/// One pass of `connect_pockets`: everything open within `area` reachable from `from`, ways only within `area`.
/// Returns how many blocked tiles it opened.
fn connect_area(tiles: &mut Tiles, from: (i32, i32), min_pocket: usize, rough: &Noise, area: &dyn Fn(i32, i32) -> bool) -> usize {
    let n = tiles.size;
    let index = |x: i32, y: i32| (y * n + x) as usize;
    let open = |tiles: &Tiles, x: i32, y: i32| tiles.inside(x, y) && area(x, y) && !tiles.get(x, y).is_solid();
    let mut reached = vec![false; (n * n) as usize];
    flood(tiles, from, &mut reached, &open);
    let mut opened = 0;
    let mut pocket_seen = vec![false; (n * n) as usize];
    for (x, y) in tiles.all() {
        let i = index(x, y);
        if reached[i] || pocket_seen[i] || !open(tiles, x, y) {
            continue;
        }
        // A new open place that isn't connected. Which tiles belong to it?
        let mut pocket = vec![false; (n * n) as usize];
        let cells = flood(tiles, (x, y), &mut pocket, &open);
        for &(px, py) in &cells {
            pocket_seen[index(px, py)] = true;
        }
        if cells.len() < min_pocket {
            for &(px, py) in &cells {
                tiles.set(px, py, Terrain::Forest);
            }
            continue;
        }
        // From all of its tiles at once until a reached tile turns up – the cheapest way. Dijkstra: always go on from
        // the cheapest tile so far (`BinaryHeap` + `Reverse` = smallest cost first). A step onto open ground costs
        // nothing, a step through a blocked tile 6 to 26 (the slow noise `rough`).
        let mut cost = vec![u32::MAX; (n * n) as usize];
        let mut came_from: Vec<u32> = vec![u32::MAX; (n * n) as usize];
        let mut queue: BinaryHeap<Reverse<(u32, u32)>> = BinaryHeap::new();
        for &(px, py) in &cells {
            cost[index(px, py)] = 0;
            came_from[index(px, py)] = index(px, py) as u32;
            queue.push(Reverse((0, index(px, py) as u32)));
        }
        let mut found = None;
        while let Some(Reverse((here, i))) = queue.pop() {
            if here > cost[i as usize] {
                continue; // an old entry – this tile was reached cheaper in the meantime
            }
            let (cx, cy) = (i as i32 % n, i as i32 / n);
            if reached[i as usize] {
                found = Some((cx, cy));
                break;
            }
            for (nx, ny) in [(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)] {
                if !tiles.inside(nx, ny) || !area(nx, ny) {
                    continue;
                }
                let step = if open(tiles, nx, ny) { 0 } else { 6 + (rough.fractal(nx as f32, ny as f32, 8.0, 2) * 20.0) as u32 };
                if here + step < cost[index(nx, ny)] {
                    cost[index(nx, ny)] = here + step;
                    came_from[index(nx, ny)] = i;
                    queue.push(Reverse((here + step, index(nx, ny) as u32)));
                }
            }
        }
        let Some(mut at) = found else {
            continue;
        };
        // Walk back along the way and clear it where it goes through something: the tile and its 8 neighbours (a
        // 1-tile cut would be too easy to miss and to block). Never out of the area.
        let mut path = Vec::new();
        loop {
            if tiles.get(at.0, at.1).is_solid() {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        opened += open_up(tiles, area, at.0 + dx, at.1 + dy) as usize;
                    }
                }
            }
            path.push(at);
            let prev = came_from[index(at.0, at.1)] as i32;
            let prev = (prev % n, prev / n);
            if prev == at {
                break;
            }
            at = prev;
        }
        // Now the way, this pocket and whatever else the way opened belong to the reached part. (Flooding from
        // `from` again wouldn't do: it is marked already, so the flood would stop right there.)
        for (px, py) in path {
            flood(tiles, (px, py), &mut reached, &open);
        }
    }
    opened
}

/// Grass on a blocked tile of the area – returns whether there was something to open.
fn open_up(tiles: &mut Tiles, area: &dyn Fn(i32, i32) -> bool, x: i32, y: i32) -> bool {
    let blocked = tiles.inside(x, y) && area(x, y) && tiles.get(x, y).is_solid();
    if blocked {
        tiles.set(x, y, Terrain::Grass);
    }
    blocked
}

/// Marks every `open` tile reachable from `start` (straight steps) in `seen` and returns the newly marked ones.
fn flood(tiles: &Tiles, start: (i32, i32), seen: &mut [bool], open: &dyn Fn(&Tiles, i32, i32) -> bool) -> Vec<(i32, i32)> {
    let n = tiles.size;
    let mut found = Vec::new();
    if !open(tiles, start.0, start.1) || seen[(start.1 * n + start.0) as usize] {
        return found;
    }
    let mut stack = vec![start];
    seen[(start.1 * n + start.0) as usize] = true;
    while let Some((x, y)) = stack.pop() {
        found.push((x, y));
        for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if open(tiles, nx, ny) && !seen[(ny * n + nx) as usize] {
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
        connect_pockets(&mut tiles, (10, 10), 5, &Noise::new(1), |_, _| true);
        let mut seen = vec![false; 1600];
        flood(&tiles, (10, 10), &mut seen, &|t: &Tiles, x, y| t.inside(x, y) && !t.get(x, y).is_solid());
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
