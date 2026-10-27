//! The crowd moves like TAB's physics (DESIGN.md 8.1): every zombie is a smooth circle with a weight (`mass` in
//! `data\zombies.json`). First each one takes the step it wants (`zombies.rs`). Then, a few rounds over, everyone is
//! pushed out of everyone it overlaps – each by its share of the overlap, the lighter one further – and fully out of
//! walls, buildings, water and mountains. Where that leaves it is where it stands this tick.
//!
//! TAB uses a real physics engine for it (12 steps a second, 10 rounds of pushing apart each). This is our own,
//! smaller one – but the same on every PC, so co-op stays in step (DESIGN.md 12.8). 20 ticks a second with `ROUNDS`
//! rounds each, every round pushing `RELAX` of the way, push about as hard per second as TAB's.
//!
//! **Why the mass flows like a liquid:** the circles are smooth – one pressed against another at a slant slides off it
//! to the side instead of stopping. Every round passes the push on by one more row, so the press of the ones behind
//! reaches the front; the front gives way to the sides, along the wall, around a corner.
//!
//! **Rocks** (TAB's `CMovable`): whoever has just stopped – for `ROCK_SECONDS` – weighs `ROCK` times as much: the crowd
//! flows around it instead of shoving it away. A zombie that just got to the wall it hits stands firm, the ones behind
//! slide past it along the wall. Your units too, while they hold or aim at a zombie (`Unit::is_rock`).
//!
//! **The same on every PC and on all cores at once ("Jacobi"):** in every round each zombie works out where it goes only
//! from where everyone stood after the round before – only reading, nobody writes. So it doesn't matter who goes first:
//! with many of them it's worked out on all cores side by side (`per_zombie`), collected in the order of the list.

use macroquad::math::{vec2, Vec2};
use rayon::prelude::*;

use super::zombies::{PER_CORE, SIDE_BY_SIDE_FROM};
use super::{slide, World, TICKS_PER_SECOND, UNIT_RADIUS};

/// Rounds of pushing apart per tick. Each round the press goes one row deeper into the crowd. With 6, 600 zombies
/// pressing against a ring of walls stepped back and forth 600 times in 3 s and 157 of them stood more than a third
/// inside another; with 8 (and `RELAX`) 18 times and 5 of them. 10 were hardly calmer.
const ROUNDS: usize = 8;
/// Each round a zombie goes only this share of the way out of the others. All of it, two pushed from both sides
/// overshoot – back and forth from round to round, from tick to tick (with 8 rounds: 59 visible steps back in 3 s
/// instead of 18).
const RELAX: f32 = 0.8;
/// Who has stopped stands firm for this long (TAB: 5 s) …
pub(super) const ROCK_SECONDS: f32 = 5.0;
/// … this many times as heavy (TAB) – the others flow around it.
const ROCK: f32 = 500.0;
/// How heavy one of your units is, like a walker. A rock: `ROCK` times that.
const UNIT_MASS: f32 = 1.0;
/// Neighbours a zombie may get pushed into this tick are looked for this far beyond touching (tiles) – further than
/// two of them get closer in one tick, both walking and pushed.
const MARGIN: f32 = 0.3;
/// At most this far per round (tiles) – one squeezed in the middle of a pile doesn't jump out of it.
const MAX_FIX: f32 = 0.1;
/// A neighbour in the list of one of your units (`UNIT | index`) instead of a zombie (`index`).
const UNIT: u32 = 1 << 31;

/// Ticks a rock stands firm (`ROCK_SECONDS`).
pub(super) fn rock_ticks() -> u32 {
    (ROCK_SECONDS * TICKS_PER_SECOND) as u32
}

/// Has it stopped `still` ticks ago, short enough to stand firm? 0: it walks.
pub(super) fn is_rock(still: u32) -> bool {
    still > 0 && still <= rock_ticks()
}

impl World {
    /// Moves the crowd: everyone with `solving` set takes its step (`wants`, tiles) and is pushed out of the others, out
    /// of your units and out of blocked tiles (see the top of the file). Everyone else (resting, in the air) is already
    /// where it ends up – a resting one walks on with its step and stays where it was decided. `wants`, `solving`: one
    /// entry per zombie, in the order of the list. The grid (`grid`) has to hold everyone where the tick began.
    /// Your units give way too, by their weight: how far the crowd pushes each one goes into its `shove` – it walks
    /// that along with its own step (`units.rs`), sliding along walls and the world border.
    pub(super) fn move_crowd(&mut self, wants: &[Vec2], solving: &[bool]) {
        let map = &self.map;
        // Resting ones walk on – sliding along walls, nobody pushes them.
        for ((zombie, &want), _) in self.zombies.iter_mut().zip(wants).zip(solving).filter(|&(_, &s)| !s) {
            if want != Vec2::ZERO && zombie.leap.is_none() {
                zombie.pos = slide(zombie.pos, want, |x, y| map.is_blocked(x, y));
            }
        }
        for unit in &mut self.units {
            unit.shove = Vec2::ZERO;
        }
        let busy = solving.iter().filter(|&&s| s).count();
        if busy == 0 {
            return;
        }
        // How light everyone is (1 / mass): only the light give way. One that doesn't take part this tick doesn't
        // give way at all (0) – the others are pushed off it.
        let weight: Vec<f32> = self
            .zombies
            .iter()
            .zip(solving)
            .map(|(z, &s)| if s { 1.0 / (z.breed.stats().mass * if is_rock(z.still) { ROCK } else { 1.0 }) } else { 0.0 })
            .collect();
        let radius: Vec<f32> = self.zombies.iter().map(|z| z.radius).collect();
        let unit_weight: Vec<f32> = self.units.iter().map(|u| 1.0 / (UNIT_MASS * if u.is_rock() { ROCK } else { 1.0 })).collect();
        let (starts, near) = self.neighbours(solving);
        // And per unit on the ground the zombies taking part it may get pushed by.
        let unit_near: Vec<Vec<u32>> = self.units.iter().map(|u| self.zombies_near_unit(u, solving)).collect();

        // Where everyone is: the solving ones with their step taken, the others where they are now.
        let mut at: Vec<Vec2> =
            self.zombies.iter().zip(wants).zip(solving).map(|((z, &want), &s)| if s { z.pos + want } else { z.pos }).collect();
        let mut unit_at: Vec<Vec2> = self.units.iter().map(|u| u.pos).collect();
        for _ in 0..ROUNDS {
            let (before, units_before) = (&at, &unit_at);
            let next = self.per_zombie(busy, |(i, zombie)| {
                let me = before[i];
                if !solving[i] {
                    return me;
                }
                let others = near[starts[i] as usize..starts[i + 1] as usize].iter().map(|&other| {
                    if other & UNIT != 0 {
                        let u = (other & !UNIT) as usize;
                        (units_before[u], UNIT_RADIUS, unit_weight[u], !self.units[u].id)
                    } else {
                        let j = other as usize;
                        (before[j], radius[j], weight[j], self.zombies[j].id)
                    }
                });
                let fix = push_out((me, zombie.radius, weight[i], zombie.id), others);
                out_of_blocked(me + fix, zombie.radius, |x, y| map.is_blocked(x, y))
            });
            // Your units – only a few, one after the other.
            let units_next: Vec<Vec2> = self
                .units
                .iter()
                .enumerate()
                .map(|(u, unit)| {
                    let me = units_before[u];
                    if unit_near[u].is_empty() {
                        return me;
                    }
                    let others = unit_near[u].iter().map(|&z| (before[z as usize], radius[z as usize], weight[z as usize], self.zombies[z as usize].id));
                    let fix = push_out((me, UNIT_RADIUS, unit_weight[u], !unit.id), others);
                    out_of_blocked(me + fix, UNIT_RADIUS, |x, y| map.blocks_unit(x, y))
                })
                .collect();
            at = next;
            unit_at = units_next;
        }
        // There – but never with its middle into a blocked tile, nor through the corner between two walls standing
        // diagonally (`slide` goes along x and y one after the other).
        for ((zombie, to), _) in self.zombies.iter_mut().zip(at).zip(solving).filter(|&(_, &s)| s) {
            zombie.pos = slide(zombie.pos, to - zombie.pos, |x, y| map.is_blocked(x, y));
        }
        for (unit, to) in self.units.iter_mut().zip(unit_at) {
            unit.shove = to - unit.pos;
        }
    }

    /// The zombies taking part (`solving`) close enough to unit `unit` to push it this tick – none for one in a tower.
    fn zombies_near_unit(&self, unit: &super::Unit, solving: &[bool]) -> Vec<u32> {
        let mut near = Vec::new();
        if unit.is_inside() {
            return near;
        }
        let (cx, cy) = (unit.pos.x.floor() as i32, unit.pos.y.floor() as i32);
        for dy in -1..=1 {
            for dx in -1..=1 {
                for zombie in self.grid.bodies(cx + dx, cy + dy) {
                    let near_enough = UNIT_RADIUS + zombie.radius + MARGIN;
                    if solving[zombie.index as usize] && (zombie.pos - unit.pos).length_squared() < near_enough * near_enough {
                        near.push(zombie.index);
                    }
                }
            }
        }
        near
    }

    /// For every zombie taking part (`solving`): who it may get pushed into this tick – the zombies (by index) and your
    /// units on the ground (`UNIT | index`) closer than touching plus `MARGIN`, not one in the air. All lists one after
    /// the other in one long list; zombie i's part is `near[starts[i]..starts[i + 1]]`. Only those few are looked at
    /// in the rounds, not the hundreds of a thick crowd's tiles over and over.
    fn neighbours(&self, solving: &[bool]) -> (Vec<u32>, Vec<u32>) {
        let biggest = self.zombies.iter().map(|z| z.radius).fold(0.0, f32::max);
        let flying: Vec<bool> = self.zombies.iter().map(|z| z.leap.is_some()).collect();
        let list = |i: usize, near: &mut Vec<u32>| {
            let zombie = &self.zombies[i];
            if !solving[i] {
                return;
            }
            let (pos, radius) = (zombie.pos, zombie.radius);
            let (cx, cy) = (pos.x.floor() as i32, pos.y.floor() as i32);
            // A walker looks into the 3 × 3 tiles around, a giant next to another giant further.
            let reach = (radius + biggest + MARGIN).ceil() as i32;
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    for other in self.grid.bodies(cx + dx, cy + dy) {
                        let near_enough = radius + other.radius + MARGIN;
                        if other.index as usize != i
                            && (other.pos - pos).length_squared() < near_enough * near_enough
                            && !flying[other.index as usize]
                        {
                            near.push(other.index);
                        }
                    }
                }
            }
            for dy in -1..=1 {
                for dx in -1..=1 {
                    for u in self.unit_grid.cell(cx + dx, cy + dy) {
                        let near_enough = radius + UNIT_RADIUS + MARGIN;
                        if !self.units[u].is_inside() && (self.units[u].pos - pos).length_squared() < near_enough * near_enough {
                            near.push(UNIT | u as u32);
                        }
                    }
                }
            }
        };
        // In blocks of `PER_CORE` zombies – with many busy ones side by side – each block its own list, put together
        // in the order of the blocks.
        let block = |(b, zombies): (usize, &[super::Zombie])| {
            let (mut lengths, mut near) = (Vec::with_capacity(zombies.len()), Vec::new());
            for k in 0..zombies.len() {
                let before = near.len();
                list(b * PER_CORE + k, &mut near);
                lengths.push((near.len() - before) as u32);
            }
            (lengths, near)
        };
        let blocks: Vec<(Vec<u32>, Vec<u32>)> = if solving.iter().filter(|&&s| s).count() >= SIDE_BY_SIDE_FROM {
            self.zombies.par_chunks(PER_CORE).enumerate().map(block).collect()
        } else {
            self.zombies.chunks(PER_CORE).enumerate().map(block).collect()
        };
        let mut starts = Vec::with_capacity(self.zombies.len() + 1);
        let mut near = Vec::new();
        starts.push(0);
        for (lengths, part) in blocks {
            for length in lengths {
                starts.push(starts[starts.len() - 1] + length);
            }
            near.extend(part);
        }
        (starts, near)
    }
}

/// How far a body (`me`: where, radius, how light, its number) is pushed out of the `others` it overlaps (each the
/// same) this round: by its share of every overlap – all of it against one that doesn't give way (lightness 0), half
/// against one as heavy, hardly any against a giant –, `RELAX` of that, at most `MAX_FIX`.
fn push_out((pos, radius, light, id): (Vec2, f32, f32, u32), others: impl Iterator<Item = (Vec2, f32, f32, u32)>) -> Vec2 {
    let mut fix = Vec2::ZERO;
    for (q, r, other_light, other_id) in others {
        let touching = radius + r;
        let away = pos - q;
        // Most don't touch: the squared distance tells that without the slow square root.
        let d2 = away.length_squared();
        if d2 >= touching * touching {
            continue;
        }
        let d = d2.sqrt();
        // Exactly on top of each other: no direction to push in. Then the numbers decide.
        let dir = if d > 0.0001 { away / d } else { overlap_direction(id, other_id) };
        fix += dir * (touching - d) * (light / (light + other_light));
    }
    (fix * RELAX).clamp_length_max(MAX_FIX)
}

/// Pushes a body (middle `p`, radius `r`) out of every `blocked` tile it overlaps – walls, buildings, water,
/// mountains don't give way (TAB: they don't move). Each tile is a square: out from the point of it closest to the
/// middle. With its middle inside the tile (only if pushed in from two sides at once), out over the nearest edge.
fn out_of_blocked(mut p: Vec2, r: f32, blocked: impl Fn(i32, i32) -> bool) -> Vec2 {
    let (x0, x1) = ((p.x - r).floor() as i32, (p.x + r).floor() as i32);
    let (y0, y1) = ((p.y - r).floor() as i32, (p.y + r).floor() as i32);
    for y in y0..=y1 {
        for x in x0..=x1 {
            if !blocked(x, y) {
                continue;
            }
            let (min, max) = (vec2(x as f32, y as f32), vec2(x as f32 + 1.0, y as f32 + 1.0));
            let away = p - p.clamp(min, max);
            let d2 = away.length_squared();
            if d2 >= r * r {
                continue;
            }
            if d2 > 0.0 {
                let d = d2.sqrt();
                p += away / d * (r - d);
            } else {
                let edges = [
                    (p.x - min.x, vec2(-1.0, 0.0)),
                    (max.x - p.x, vec2(1.0, 0.0)),
                    (p.y - min.y, vec2(0.0, -1.0)),
                    (max.y - p.y, vec2(0.0, 1.0)),
                ];
                let (d, out) = edges.into_iter().fold(edges[0], |best, e| if e.0 < best.0 { e } else { best });
                p += out * (d + r);
            }
        }
    }
    p
}

/// Direction for two bodies standing exactly on top of each other: one of 8 directions, picked from both numbers, and
/// the one with the smaller number gets the opposite – so they move apart instead of together. A table instead of
/// `sin`/`cos`: those may round differently on another PC, and co-op needs the same result.
fn overlap_direction(me: u32, other: u32) -> Vec2 {
    const D: f32 = std::f32::consts::FRAC_1_SQRT_2;
    const DIRS: [Vec2; 8] = [
        vec2(1.0, 0.0), vec2(D, D), vec2(0.0, 1.0), vec2(-D, D),
        vec2(-1.0, 0.0), vec2(-D, -D), vec2(0.0, -1.0), vec2(D, -D),
    ];
    let (low, high) = (me.min(other), me.max(other));
    let dir = DIRS[(low.wrapping_mul(7919) ^ high.wrapping_mul(104_729)) as usize % 8];
    if me == low { -dir } else { dir }
}
