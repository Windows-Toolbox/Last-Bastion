//! Ore patches (DESIGN.md 5.1, 9.1 steps 2–3) – placed like Factorio's "spot noise" (Factorio Friday Facts #258):
//!
//! 1. **Start area**: one small patch of each ore just outside the free space around the base – like Factorio's
//!    starting area, so every map can start the same economy.
//! 2. **The rest of the map** is cut into square regions. Each gets a few random candidate points, sorted by how
//!    "favourable" they are (a slow noise – so patches cluster a little, like ore fields), and the best ones become
//!    patches until the region has as many as the density asks for. Spots on water, rock or too close to another
//!    patch are skipped.
//! 3. **Further out = bigger and richer**, like in Factorio: going out there for ore is worth the risk.
//!
//! A patch is a frayed disc (noise on its edge) with the most ore in its middle. Ore only lies on grass and only
//! inside the world border – your units can't go beyond it. Only `+ - * /` and `sqrt` (DESIGN.md 12.8).

use macroquad::math::{vec2, Vec2};

use super::tools::{self, Tiles};
use super::Context;
use crate::map::{Deposit, Ore, Terrain};
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

/// What the setup screen decides about ore (`Common`).
pub struct OreSettings {
    /// Patches per 10,000 tiles outside the start area.
    pub per_10k: f32,
    /// Average radius of a patch near the base (tiles).
    pub size: f32,
    /// 1.0 = normal.
    pub richness: f32,
}

struct Patch {
    center: Vec2,
    radius: f32,
    ore: Ore,
    /// Share of `CENTER_AMOUNT` in its middle.
    richness: f32,
}

/// Where the ore goes: (x, y, deposit) for every tile of every patch. `play`: the playing field inside the world
/// border, as (first tile, one past the last tile) on both axes.
pub fn place(tiles: &Tiles, rng: &mut Rng, ctx: &Context, play: (i32, i32), settings: &OreSettings) -> Vec<(i32, i32, Deposit)> {
    let (lo, hi) = play;
    let half = (hi - lo) as f32 / 2.0;
    let shape = Noise::new(rng.next_u64() as u32);
    let favour = Noise::new(rng.next_u64() as u32);
    let mut patches: Vec<Patch> = Vec::new();

    // Is the disc inside the world border, mostly on grass, and far enough from every patch so far?
    let fits = |patches: &[Patch], c: Vec2, r: f32, grass_needed: f32| {
        let inside = c.x - r >= lo as f32 && c.y - r >= lo as f32 && c.x + r <= hi as f32 && c.y + r <= hi as f32;
        inside && patches.iter().all(|p| p.center.distance(c) >= p.radius + r + GAP) && grass_share(tiles, c, r) >= grass_needed
    };

    // 1. Start area: one patch of each ore, a few tiles outside the free space, in random directions.
    for ore in Ore::ALL {
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
                let wanted = settings.per_10k * (w * h) as f32 / 10_000.0;
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
                if x < lo || y < lo || x >= hi || y >= hi || tiles.get(x, y) != Terrain::Grass || taken[(y * n + x) as usize] {
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
    deposits
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
