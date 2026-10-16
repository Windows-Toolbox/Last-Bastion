//! **Regions**: the map is split into regions like in TAB (DESIGN.md 9.1, step 4 "chokepoints"). Between two
//! regions runs a wall of rock, forest or water with one or a few **passes** – the obvious places to wall off.
//! Every region gets a character: open meadow, woods, rocky, or a lake.
//!
//! How the regions come about: a "Voronoi diagram". A few points ("seeds") are scattered over the map, and every
//! tile belongs to the seed closest to it. Where the closest and the second closest seed are almost equally far
//! away, the tile lies on the border between two regions – that's where the wall goes. The tile is looked up a
//! bit shifted (`Warp`), so the borders wind instead of being straight lines.

use std::collections::BTreeMap;

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use super::tools::{self, Tiles, Warp};
use super::{Context, Field, Show, MATERIALS};
use crate::map::Terrain;
use crate::noise::Noise;
use crate::rng::Rng;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    pub regions: f32,
    pub wobble: f32,
    pub wall: f32,
    pub passes: f32,
    pub pass_width: f32,
    /// Index into `MATERIALS`.
    pub material: f32,
    /// Shares of the regions with that character (the rest is open meadow).
    pub woods: f32,
    pub rocky: f32,
    pub lakes: f32,
    pub trees: f32,
}

impl Default for Params {
    fn default() -> Params {
        Params {
            regions: 18.0,
            wobble: 10.0,
            wall: 3.0,
            passes: 1.0,
            pass_width: 5.0,
            material: 2.0,
            woods: 0.3,
            rocky: 0.15,
            lakes: 0.1,
            trees: 0.015,
        }
    }
}

impl Params {
    pub fn fields(&mut self) -> Vec<Field<'_>> {
        vec![
            Field { label: "Regions", value: &mut self.regions, min: 4.0, max: 48.0, step: 1.0, show: Show::Number,
                help: "How many regions the map is split into. Fewer = bigger regions." },
            Field { label: "Border wobble", value: &mut self.wobble, min: 0.0, max: 30.0, step: 1.0, show: Show::Tiles,
                help: "0 = straight borders, more = winding ones." },
            Field { label: "Wall thickness", value: &mut self.wall, min: 1.0, max: 8.0, step: 1.0, show: Show::Tiles,
                help: "How thick the walls between the regions are." },
            Field { label: "Passes per border", value: &mut self.passes, min: 1.0, max: 4.0, step: 1.0, show: Show::Number,
                help: "Openings in every wall between two regions." },
            Field { label: "Pass width", value: &mut self.pass_width, min: 2.0, max: 14.0, step: 1.0, show: Show::Tiles,
                help: "How wide a pass is - narrow passes are easy to wall off." },
            Field { label: "Walls of", value: &mut self.material, min: 0.0, max: 3.0, step: 1.0, show: Show::Choice(MATERIALS),
                help: "What the walls are made of. Mixed: rock or forest, now and then water." },
            Field { label: "Wooded regions", value: &mut self.woods, min: 0.0, max: 1.0, step: 0.05, show: Show::Percent,
                help: "Share of the regions full of forest with clearings." },
            Field { label: "Rocky regions", value: &mut self.rocky, min: 0.0, max: 1.0, step: 0.05, show: Show::Percent,
                help: "Share of the regions with rocks lying around." },
            Field { label: "Lake regions", value: &mut self.lakes, min: 0.0, max: 1.0, step: 0.05, show: Show::Percent,
                help: "Share of the regions with a lake in the middle." },
            tools::trees_field(&mut self.trees),
        ]
    }
}

/// What a region looks like inside.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Meadow,
    Woods,
    Rocky,
    Lake,
}

pub fn generate(tiles: &mut Tiles, p: &Params, rng: &mut Rng, ctx: &Context) {
    let n = tiles.size as f32;
    let count = p.regions.round().max(2.0) as usize;

    // The seeds: the first one on the base (so the base lies in the middle of its region), the others scattered
    // with some distance to each other – otherwise there would be tiny regions.
    let min_gap = n / (count as f32).sqrt() * 0.6;
    let mut seeds: Vec<Vec2> = vec![ctx.center];
    for _ in 1..count {
        let mut best = vec2(rng.unit_f32() * n, rng.unit_f32() * n);
        for _ in 0..30 {
            let p = vec2(rng.unit_f32() * n, rng.unit_f32() * n);
            if seeds.iter().all(|s| s.distance(p) >= min_gap) {
                best = p;
                break;
            }
        }
        seeds.push(best);
    }
    let kinds: Vec<Kind> = (0..count)
        .map(|i| {
            if i == 0 {
                return Kind::Meadow; // the base's region
            }
            let roll = rng.unit_f32();
            if roll < p.woods {
                Kind::Woods
            } else if roll < p.woods + p.rocky {
                Kind::Rocky
            } else if roll < p.woods + p.rocky + p.lakes {
                Kind::Lake
            } else {
                Kind::Meadow
            }
        })
        .collect();

    // Per tile: which region, and how far it is to the border (second closest minus closest seed).
    let warp = Warp::new(rng, 40.0, p.wobble);
    let size = tiles.size;
    let mut region = vec![0u16; (size * size) as usize];
    let mut to_border = vec![0f32; (size * size) as usize];
    // Every wall tile, sorted by the pair of regions it separates. A `BTreeMap` and not a `HashMap`: a `HashMap`
    // goes through its entries in a different order on every start (on purpose, against attacks) – then the same
    // seed would give different maps. A `BTreeMap` is always sorted.
    let mut borders: BTreeMap<(u16, u16), Vec<(i32, i32)>> = BTreeMap::new();
    for (x, y) in tiles.all() {
        let w = warp.apply(x as f32 + 0.5, y as f32 + 0.5);
        let (mut a, mut da, mut b, mut db) = (0, f32::MAX, 0, f32::MAX);
        for (i, s) in seeds.iter().enumerate() {
            let d = s.distance(w);
            if d < da {
                (b, db) = (a, da);
                (a, da) = (i, d);
            } else if d < db {
                (b, db) = (i, d);
            }
        }
        let i = (y * size + x) as usize;
        region[i] = a as u16;
        to_border[i] = db - da;
        if db - da < p.wall {
            borders.entry((a.min(b) as u16, a.max(b) as u16)).or_default().push((x, y));
        }
    }

    // Inside the regions: forest, rocks or a lake from noise – but not too close to the border, so the passes
    // stay open on both sides.
    let (detail, lake) = (Noise::new(rng.next_u64() as u32), Noise::new(rng.next_u64() as u32));
    for (x, y) in tiles.all() {
        let i = (y * size + x) as usize;
        let inner = to_border[i] - p.wall - 3.0;
        if inner <= 0.0 {
            continue;
        }
        let v = detail.fractal(x as f32, y as f32, 12.0, 3);
        let terrain = match kinds[region[i] as usize] {
            Kind::Meadow => continue,
            Kind::Woods if v > 0.42 => Terrain::Forest,
            Kind::Rocky if v > 0.62 => Terrain::Mountain,
            // A lake fills the middle of its region: the deeper inside, the likelier.
            Kind::Lake if inner > 6.0 && lake.fractal(x as f32, y as f32, 16.0, 3) + inner * 0.03 > 0.8 => Terrain::Water,
            _ => continue,
        };
        tiles.set(x, y, terrain);
    }

    // The walls, each border its own material.
    for cells in borders.values() {
        let terrain = tools::material(p.material, rng);
        for &(x, y) in cells {
            tiles.set(x, y, terrain);
        }
    }
    // The passes: on every border a few random spots where its wall is cleared. Only borders long enough to be a
    // real wall – the tiny corners where three regions meet are left as they are.
    for cells in borders.values() {
        if cells.len() < 6 {
            continue;
        }
        for _ in 0..p.passes.round() as u32 {
            let (x, y) = cells[rng.below(cells.len() as u32) as usize];
            let center = vec2(x as f32 + 0.5, y as f32 + 0.5);
            // The spot may lie anywhere across the wall: a circle this big is still `pass_width` wide where it
            // leaves the wall on the far side (Pythagoras).
            let half = p.pass_width / 2.0;
            let radius = (half * half + p.wall * p.wall).sqrt();
            for &(cx, cy) in cells {
                let along = vec2(cx as f32 + 0.5, cy as f32 + 0.5).distance(center);
                if along <= radius {
                    tiles.set(cx, cy, Terrain::Grass);
                }
            }
        }
    }
    tools::scatter_trees(tiles, rng, p.trees);
}
