//! **Ways in**, like the open approaches of a TAB map: before a generator paints anything, a few open lanes are
//! planned from every side of the map to the base. The generators let their features fade out before them – the
//! same way they fade out before the base (`Context::fade`) – and afterwards `generate` clears whatever still lies in
//! them (a river gets a ford there, a wall between two regions a pass). So every side has its way to the base from
//! the start and the terrain grows around it, instead of a straight line being cut through it afterwards
//! (`tools::connect_pockets` is only the safety net now).
//!
//! A lane starts at the world border, evenly spread along its side, and walks towards the base, bending left and
//! right like a river (`tools::river`) and getting a bit wider and narrower on the way. When it runs into a lane
//! planned before it, it ends there – that one already leads to the base. So lanes join like the branches of a tree
//! and fewer of them reach the base: those are the chokepoints. Only `+ - * /` and `sqrt` (DESIGN.md 12.8).

use macroquad::math::{vec2, Vec2};

use crate::noise::Noise;
use crate::rng::Rng;

/// How far outside a lane its distance is worked out (tiles) – at least as far as any generator fades (Canyons: 10).
const REACH: f32 = 12.0;
/// How strongly a lane bends: the sideways push at the most, relative to "straight to the base".
const BEND: f32 = 3.0;
/// A lane walks at least this many steps (tiles) before it may end in another one – so two lanes from the same spot
/// don't end right at the border.
const MIN_STEPS: usize = 12;

/// The planned lanes, as a distance for every tile.
pub struct Lanes {
    size: i32,
    /// Per tile: how far it is to the edge of the nearest lane (tiles) – 0 or less inside a lane. Only worked out
    /// up to `REACH` tiles outside; further away it stays infinite.
    clearance: Vec<f32>,
}

impl Lanes {
    /// No lanes at all (Rings, Blank, or 0 per side).
    pub fn none(size: i32) -> Lanes {
        Lanes { size, clearance: vec![f32::INFINITY; (size * size) as usize] }
    }

    /// `per_side` lanes from every side of a map `size` tiles across to the base at `center`, `width` tiles wide on
    /// average. They start at the world border (`border` tiles from the map edge) and end at `stop` tiles from the
    /// base – the free space there is open anyway.
    pub fn plan(rng: &mut Rng, size: i32, border: i32, center: Vec2, stop: f32, per_side: u32, width: f32) -> Lanes {
        let mut lanes = Lanes::none(size);
        if per_side == 0 {
            return lanes;
        }
        let (bend, thickness) = (Noise::new(rng.next_u64() as u32), Noise::new(rng.next_u64() as u32));
        let (lo, n) = (border as f32, size as f32);
        let piece = (n - 2.0 * lo) / per_side as f32;
        let mut number = 0.0;
        for side in 0..4 {
            for k in 0..per_side {
                // The middle of the k-th piece of the side, give or take a quarter piece.
                let along = lo + piece * (k as f32 + 0.5 + (rng.unit_f32() - 0.5) * 0.5);
                let start = match side {
                    0 => vec2(along, lo),
                    1 => vec2(along, n - lo),
                    2 => vec2(lo, along),
                    _ => vec2(n - lo, along),
                };
                // Each lane reads the noise in its own row, so they all bend differently.
                lanes.walk(start, center, stop, width, &bend, &thickness, number * 100.0);
                number += 1.0;
            }
        }
        lanes
    }

    /// How far the tile (x, y) is from the edge of the nearest lane – 0 or less inside one, infinite far away and
    /// off the map.
    pub fn clearance(&self, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 || x >= self.size || y >= self.size {
            return f32::INFINITY;
        }
        self.clearance[(y * self.size + x) as usize]
    }

    /// Does the tile lie in a lane – must it stay open?
    pub fn is_open(&self, x: i32, y: i32) -> bool {
        self.clearance(x, y) <= 0.0
    }

    /// One lane: first the whole way (checking against the lanes before it, not against itself), then painted.
    #[allow(clippy::too_many_arguments)]
    fn walk(&mut self, from: Vec2, to: Vec2, stop: f32, width: f32, bend: &Noise, thickness: &Noise, row: f32) {
        let mut points: Vec<(Vec2, f32)> = Vec::new();
        let mut pos = from;
        for k in 0..(self.size * 3) as usize {
            let t = k as f32;
            // Half the width here: about 70 % to 130 % of the average, changing slowly along the way.
            let half = width / 2.0 * (0.4 + 1.2 * thickness.fractal(t, row, 30.0, 2));
            points.push((pos, half));
            if pos.distance(to) <= stop {
                break;
            }
            // Ran well into a lane planned before: that one leads on to the base.
            if k >= MIN_STEPS && self.clearance(pos.x.floor() as i32, pos.y.floor() as i32) < -1.0 {
                break;
            }
            // Towards the base, plus a bit sideways – how much, a slow noise along the way decides (no angles: a
            // vector made length 1, like the rivers).
            let ahead = (to - pos).normalize_or_zero();
            let sideways = vec2(-ahead.y, ahead.x) * (bend.fractal(t, row, 40.0, 3) - 0.5) * BEND;
            pos += (ahead + sideways).normalize_or_zero();
        }
        for (center, half) in points {
            self.paint(center, half);
        }
    }

    /// Every tile around `center`: its distance to the edge of a disc of radius `half` – kept if it's the smallest yet.
    fn paint(&mut self, center: Vec2, half: f32) {
        let r = (half + REACH).ceil() as i32;
        let (cx, cy) = (center.x.floor() as i32, center.y.floor() as i32);
        for y in (cy - r).max(0)..=(cy + r).min(self.size - 1) {
            for x in (cx - r).max(0)..=(cx + r).min(self.size - 1) {
                let d = vec2(x as f32 + 0.5, y as f32 + 0.5).distance(center) - half;
                let i = (y * self.size + x) as usize;
                if d < self.clearance[i] {
                    self.clearance[i] = d;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// From the base along the lanes alone (plus the free space around the base) you get to the border on every
    /// side – the ways in are there before any terrain is painted. Wider lanes, more open tiles.
    #[test]
    fn lanes_lead_from_every_side_to_the_base() {
        let (size, border, stop) = (288, 16, 14.0);
        let center = vec2(144.5, 144.5);
        for seed in 1..=8u64 {
            let lanes = Lanes::plan(&mut Rng::new(seed), size, border, center, stop, 2, 6.0);
            let open = |x: i32, y: i32| lanes.is_open(x, y) || vec2(x as f32 + 0.5, y as f32 + 0.5).distance(center) <= stop;
            let mut seen = vec![false; (size * size) as usize];
            let mut stack = vec![(144, 144)];
            seen[144 * size as usize + 144] = true;
            while let Some((x, y)) = stack.pop() {
                for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                    if nx >= 0 && ny >= 0 && nx < size && ny < size && open(nx, ny) && !seen[(ny * size + nx) as usize] {
                        seen[(ny * size + nx) as usize] = true;
                        stack.push((nx, ny));
                    }
                }
            }
            let reached = |x: i32, y: i32| seen[(y * size + x) as usize];
            let (lo, hi) = (border, size - border - 1);
            assert!((lo..=hi).any(|x| reached(x, lo)), "seed {seed}: no way in from the top");
            assert!((lo..=hi).any(|x| reached(x, hi)), "seed {seed}: no way in from the bottom");
            assert!((lo..=hi).any(|y| reached(lo, y)), "seed {seed}: no way in from the left");
            assert!((lo..=hi).any(|y| reached(hi, y)), "seed {seed}: no way in from the right");
        }
        let count = |width: f32| {
            let lanes = Lanes::plan(&mut Rng::new(3), size, border, center, stop, 2, width);
            (0..size).flat_map(|y| (0..size).map(move |x| (x, y))).filter(|&(x, y)| lanes.is_open(x, y)).count()
        };
        assert!(count(10.0) > count(4.0) * 2, "the width doesn't change the lanes");
        assert_eq!(count(6.0), count(6.0), "not repeatable");
        let none = Lanes::plan(&mut Rng::new(3), size, border, center, stop, 0, 6.0);
        assert!(!none.is_open(144, 20) && none.clearance(144, 20).is_infinite());
    }
}

