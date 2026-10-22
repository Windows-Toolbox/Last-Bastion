//! Ore patches (DESIGN.md 5.1, 9.1 steps 2–3) – placed like Factorio's "spot noise" (Factorio Friday Facts #258):
//!
//! 1. **Start area**: one small patch of each ore just outside the free space around the base – like Factorio's
//!    starting area, so every map can start the same economy.
//! 2. **The rest of the map** is cut into square regions. Each gets a few random candidate points, sorted by how
//!    "favourable" they are (a slow noise – so patches cluster a little, like ore fields), and the best ones become
//!    patches until the region has as many as the density asks for. Spots on water, rock or too close to another
//!    patch are skipped.
//! 3. **Further out = bigger and richer**, like in Factorio: going out there for ore is worth the risk.
//! 4. **Oil** lies in nodes of 2 × 2 tiles, in groups: one node alone in the start area, further out groups of more
//!    and more nodes (2 next to the start area, 6 at the edge). The same region scheme as for the patches places them.
//!
//! A patch is a frayed disc (noise on its edge) with the most ore in its middle. Ore only lies on grass and only
//! inside the world border – your units can't go beyond it. Only `+ - * /` and `sqrt` (DESIGN.md 12.8).

use macroquad::math::{vec2, Vec2};

use super::tools::{self, Tiles};
use super::Context;
use crate::map::{Deposit, Ore, Region, Terrain};
use crate::noise::Noise;
use crate::rng::Rng;

/// Edge of a region (tiles): each one gets its share of patches.
const REGION: i32 = 48;
/// Random candidate points per region – the most favourable ones become patches.
const CANDIDATES: usize = 8;
/// Units of ore in the middle of a patch next to the base at 100 % richness. At the edge of the map 3 times as much.
const CENTER_AMOUNT: f32 = 1200.0;
/// The start patches: radius (tiles) and richness (share of `CENTER_AMOUNT`).
const START_RADIUS: f32 = 4.0;
const START_RICHNESS: f32 = 0.6;
/// At least this many free tiles between two patches.
const GAP: f32 = 4.0;
/// Outside the start area: no patch closer to the base than its free space plus this (tiles).
const AWAY_FROM_BASE: f32 = 20.0;
/// Edge of a region for oil (tiles) – bigger than for ore: there are fewer oil groups than patches.
const OIL_REGION: i32 = 64;
/// Oil nodes in a group right outside the start area – at the edge of the map `OIL_GROUP_EDGE`.
const OIL_GROUP_NEAR: f32 = 2.0;
const OIL_GROUP_EDGE: f32 = 6.0;
/// At least this many tiles between the middles of two oil groups.
const OIL_GROUP_GAP: f32 = 16.0;
/// Units of oil in every tile of a node next to the base at 100 % richness – at the edge twice as much.
const OIL_AMOUNT: f32 = 2000.0;

/// What the setup screen decides about ore (`Common`).
pub struct OreSettings {
    /// Patches per 10,000 tiles outside the start area.
    pub per_10k: f32,
    /// Average radius of a patch near the base (tiles).
    pub size: f32,
    /// 1.0 = normal. Also for oil.
    pub richness: f32,
    /// Groups of oil nodes per 10,000 tiles outside the start area.
    pub oil_per_10k: f32,
}

struct Patch {
    center: Vec2,
    radius: f32,
    ore: Ore,
    /// Share of `CENTER_AMOUNT` in its middle.
    richness: f32,
}

/// Where the ore goes: (x, y, deposit) for every tile of every patch. `play`: the playing field inside the world
/// border.
pub fn place(tiles: &Tiles, rng: &mut Rng, ctx: &Context, play: &Region, settings: &OreSettings) -> Vec<(i32, i32, Deposit)> {
    // The squares of regions cover the diamond of the playing field: from its leftmost to its rightmost tile.
    let corners = play.corners();
    let (lo, hi) = (corners[0].x.floor() as i32, corners[2].x.ceil() as i32);
    // How many tiles of the square (x, y, w, h) lie inside – a region half outside gets half the patches.
    let inside = |x0: i32, y0: i32, w: i32, h: i32| (y0..y0 + h).flat_map(|y| (x0..x0 + w).map(move |x| (x, y))).filter(|&(x, y)| play.contains_tile(x, y)).count();
    // From the base to the middle of a side, in a straight line (the corners count as edge too).
    let half = (play.max.x - play.min.x) / 2.0 * std::f32::consts::FRAC_1_SQRT_2;
    let shape = Noise::new(rng.next_u64() as u32);
    let favour = Noise::new(rng.next_u64() as u32);
    let mut patches: Vec<Patch> = Vec::new();

    // Is the disc inside the world border, mostly on grass, and far enough from every patch so far?
    let fits = |patches: &[Patch], c: Vec2, r: f32, grass_needed: f32| {
        play.contains_disc(c, r) && patches.iter().all(|p| p.center.distance(c) >= p.radius + r + GAP) && grass_share(tiles, c, r) >= grass_needed
    };

    // 1. Start area: one patch of each ore, a few tiles outside the free space, in random directions.
    for ore in Ore::PATCHES {
        for _ in 0..60 {
            let dir = tools::random_direction(rng);
            let dist = ctx.keep_out + START_RADIUS + 2.0 + rng.unit_f32() * 10.0;
            let c = ctx.center + dir * dist;
            if fits(&patches, c, START_RADIUS, 0.6) {
                patches.push(Patch { center: c, radius: START_RADIUS, ore, richness: START_RICHNESS * settings.richness });
                break;
            }
        }
    }

    // 2. Region by region: candidates sorted by favour, the best ones become patches.
    if settings.per_10k > 0.0 {
        for ry in (lo..hi).step_by(REGION as usize) {
            for rx in (lo..hi).step_by(REGION as usize) {
                let (w, h) = ((hi - rx).min(REGION), (hi - ry).min(REGION));
                let wanted = settings.per_10k * inside(rx, ry, w, h) as f32 / 10_000.0;
                // 2.4 wanted: 2 patches, and a 40 % chance of a third.
                let mut count = wanted.floor() as usize + usize::from(rng.unit_f32() < wanted - wanted.floor());
                let mut candidates: Vec<(f32, Vec2)> = (0..CANDIDATES)
                    .map(|_| {
                        let p = vec2(rx as f32 + rng.unit_f32() * w as f32, ry as f32 + rng.unit_f32() * h as f32);
                        (favour.fractal(p.x, p.y, 64.0, 2) + 0.3 * rng.unit_f32(), p)
                    })
                    .collect();
                // Most favourable first. Equal values can't change the order between PCs: the sort is stable.
                candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
                for (_, c) in candidates {
                    if count == 0 {
                        break;
                    }
                    let d = c.distance(ctx.center);
                    if d < ctx.keep_out + AWAY_FROM_BASE {
                        continue;
                    }
                    let out = (d / half).min(1.0);
                    let radius = settings.size * (0.7 + 0.6 * rng.unit_f32()) * (0.8 + 0.6 * out);
                    if !fits(&patches, c, radius, 0.5) {
                        continue;
                    }
                    let roll = rng.unit_f32();
                    let ore = if roll < 0.4 {
                        Ore::Iron
                    } else if roll < 0.75 {
                        Ore::Coal
                    } else {
                        Ore::Stone
                    };
                    patches.push(Patch { center: c, radius, ore, richness: settings.richness * (1.0 + 2.0 * out) });
                    count -= 1;
                }
            }
        }
    }

    // 3. Paint: every grass tile within the frayed edge; the most ore in the middle, a fifth of it at the edge.
    let n = tiles.size;
    let mut taken = vec![false; (n * n) as usize];
    let mut deposits = Vec::new();
    for p in &patches {
        let reach = (p.radius * 1.45).ceil() as i32 + 1;
        let (cx, cy) = (p.center.x.floor() as i32, p.center.y.floor() as i32);
        for y in cy - reach..=cy + reach {
            for x in cx - reach..=cx + reach {
                if !play.contains_tile(x, y) || tiles.get(x, y) != Terrain::Grass || taken[(y * n + x) as usize] {
                    continue;
                }
                let d = vec2(x as f32 + 0.5, y as f32 + 0.5).distance(p.center);
                let edge = p.radius * (0.55 + 0.9 * shape.fractal(x as f32, y as f32, 5.0, 3));
                if d > edge {
                    continue;
                }
                let amount = (CENTER_AMOUNT * p.richness * (1.0 - 0.8 * d / edge)).round().max(1.0) as u32;
                taken[(y * n + x) as usize] = true;
                deposits.push((x, y, Deposit { ore: p.ore, amount }));
            }
        }
    }

    // 4. Oil: one node in the start area, then groups region by region – more nodes per group further out.
    let mut oil = Oil { tiles, play: *play, taken, deposits, groups: Vec::new() };
    for _ in 0..60 {
        let c = ctx.center + tools::random_direction(rng) * (ctx.keep_out + 3.0 + rng.unit_f32() * 10.0);
        if oil.group(rng, c, 1, settings.richness) {
            break;
        }
    }
    if settings.oil_per_10k > 0.0 {
        let favour = Noise::new(rng.next_u64() as u32);
        for ry in (lo..hi).step_by(OIL_REGION as usize) {
            for rx in (lo..hi).step_by(OIL_REGION as usize) {
                let (w, h) = ((hi - rx).min(OIL_REGION), (hi - ry).min(OIL_REGION));
                let wanted = settings.oil_per_10k * inside(rx, ry, w, h) as f32 / 10_000.0;
                let mut count = wanted.floor() as usize + usize::from(rng.unit_f32() < wanted - wanted.floor());
                let mut candidates: Vec<(f32, Vec2)> = (0..CANDIDATES)
                    .map(|_| {
                        let p = vec2(rx as f32 + rng.unit_f32() * w as f32, ry as f32 + rng.unit_f32() * h as f32);
                        (favour.fractal(p.x, p.y, 64.0, 2) + 0.3 * rng.unit_f32(), p)
                    })
                    .collect();
                candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
                for (_, c) in candidates {
                    if count == 0 {
                        break;
                    }
                    let d = c.distance(ctx.center);
                    if d < ctx.keep_out + AWAY_FROM_BASE {
                        continue;
                    }
                    let out = (d / half).min(1.0);
                    let nodes = (OIL_GROUP_NEAR + (OIL_GROUP_EDGE - OIL_GROUP_NEAR) * out).round() as usize;
                    if oil.group(rng, c, nodes, settings.richness * (1.0 + out)) {
                        count -= 1;
                    }
                }
            }
        }
    }
    oil.deposits
}

/// Placing oil nodes: what's taken so far (ore and oil) and the middles of the groups so far.
struct Oil<'a> {
    tiles: &'a Tiles,
    play: Region,
    taken: Vec<bool>,
    deposits: Vec<(i32, i32, Deposit)>,
    groups: Vec<Vec2>,
}

impl Oil<'_> {
    /// A group of up to `nodes` nodes around `c`. The first node must fit right at `c` (otherwise there's no group
    /// and nothing happens); the others are tried at random spots around it. Returns whether the group was placed.
    fn group(&mut self, rng: &mut Rng, c: Vec2, nodes: usize, richness: f32) -> bool {
        if self.groups.iter().any(|g| g.distance(c) < OIL_GROUP_GAP) || !self.node(c, richness) {
            return false;
        }
        self.groups.push(c);
        // The more nodes, the wider the group: 2 nodes within 4 tiles, 6 within about 6.
        let spread = 2.0 + 1.6 * (nodes as f32).sqrt();
        let mut placed = 1;
        for _ in 0..nodes * 20 {
            if placed == nodes {
                break;
            }
            let p = c + tools::random_direction(rng) * (spread * rng.unit_f32());
            if self.node(p, richness) {
                placed += 1;
            }
        }
        true
    }

    /// One node: the 2 × 2 tiles whose top corner is the tile of `p` – if all four are grass inside the border and
    /// nothing lies on them or right next to them (so two nodes never touch, and none touches an ore patch).
    fn node(&mut self, p: Vec2, richness: f32) -> bool {
        let (x0, y0) = (p.x.floor() as i32, p.y.floor() as i32);
        let n = self.tiles.size;
        if !(0..4).all(|k| self.play.contains_tile(x0 + k % 2, y0 + k / 2)) {
            return false;
        }
        let grass = (0..4).all(|k| self.tiles.get(x0 + k % 2, y0 + k / 2) == Terrain::Grass);
        let free = (y0 - 1..y0 + 3).all(|y| (x0 - 1..x0 + 3).all(|x| x < 0 || y < 0 || x >= n || y >= n || !self.taken[(y * n + x) as usize]));
        if !grass || !free {
            return false;
        }
        let amount = (OIL_AMOUNT * richness).round().max(1.0) as u32;
        for k in 0..4 {
            let (x, y) = (x0 + k % 2, y0 + k / 2);
            self.taken[(y * n + x) as usize] = true;
            self.deposits.push((x, y, Deposit { ore: Ore::Oil, amount }));
        }
        true
    }
}

/// Which share of the tiles within `r` of `c` are grass.
fn grass_share(tiles: &Tiles, c: Vec2, r: f32) -> f32 {
    let reach = r.ceil() as i32;
    let (cx, cy) = (c.x.floor() as i32, c.y.floor() as i32);
    let (mut all, mut grass) = (0, 0);
    for y in cy - reach..=cy + reach {
        for x in cx - reach..=cx + reach {
            if vec2(x as f32 + 0.5, y as f32 + 0.5).distance(c) <= r {
                all += 1;
                if tiles.get(x, y) == Terrain::Grass {
                    grass += 1;
                }
            }
        }
    }
    if all == 0 { 0.0 } else { grass as f32 / all as f32 }
}
