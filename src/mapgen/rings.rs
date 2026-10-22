//! **Rings**: a fortress layout. Rings of rock, forest or water run around the base, each with a few openings.
//! The openings of neighbouring rings don't line up, so the way in zigzags – and each ring is a line of defence
//! you can fall back to. Between the rings lies some clutter (forest patches, ponds).

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use super::tools::{self, Tiles};
use super::{Context, Field, Show, MATERIALS};
use crate::map::{Terrain, BORDER};
use crate::noise::Noise;
use crate::rng::Rng;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    pub rings: f32,
    pub thickness: f32,
    pub openings: f32,
    pub opening_width: f32,
    pub wobble: f32,
    /// 0 = round rings, 1 = square ones (like the map).
    pub square: f32,
    /// Index into `MATERIALS`.
    pub material: f32,
    pub clutter: f32,
    pub trees: f32,
}

impl Default for Params {
    fn default() -> Params {
        Params {
            rings: 3.0,
            thickness: 7.0,
            openings: 3.0,
            opening_width: 6.0,
            wobble: 10.0,
            square: 0.3,
            material: 2.0,
            clutter: 0.5,
            trees: 0.02,
        }
    }
}

impl Params {
    pub fn fields(&mut self) -> Vec<Field<'_>> {
        vec![
            Field { label: "Rings", value: &mut self.rings, min: 1.0, max: 6.0, step: 1.0, show: Show::Number,
                help: "How many rings run around the base, evenly from the base to the border." },
            Field { label: "Ring thickness", value: &mut self.thickness, min: 1.0, max: 8.0, step: 1.0, show: Show::Tiles,
                help: "How thick each ring is." },
            Field { label: "Openings per ring", value: &mut self.openings, min: 1.0, max: 8.0, step: 1.0, show: Show::Number,
                help: "Gaps in every ring. They never line up with the ones of the next ring." },
            Field { label: "Opening width", value: &mut self.opening_width, min: 3.0, max: 16.0, step: 1.0, show: Show::Tiles,
                help: "How wide the gaps are." },
            Field { label: "Wobble", value: &mut self.wobble, min: 0.0, max: 24.0, step: 1.0, show: Show::Tiles,
                help: "0 = perfect rings, more = bumpy, natural ones." },
            Field { label: "Square", value: &mut self.square, min: 0.0, max: 1.0, step: 0.05, show: Show::Percent,
                help: "0 = round rings, 100 % = square ones." },
            Field { label: "Rings of", value: &mut self.material, min: 0.0, max: 3.0, step: 1.0, show: Show::Choice(MATERIALS),
                help: "What the rings are made of. Mixed: each ring rock or forest, now and then a moat." },
            Field { label: "Clutter", value: &mut self.clutter, min: 0.0, max: 1.0, step: 0.05, show: Show::Percent,
                help: "Forest patches and ponds between the rings." },
            tools::trees_field(&mut self.trees),
        ]
    }
}

pub fn generate(tiles: &mut Tiles, p: &Params, rng: &mut Rng, ctx: &Context) {
    let n = tiles.size as f32;
    let count = p.rings.round().max(1.0) as usize;
    // From just outside the free space to just inside the world border: in a straight line from the base to the
    // middle of a side it's (n / 2 − ring) / √2 (the playing field is a diamond in the grid, `map.rs`).
    let to_side = (n / 2.0 - BORDER as f32) * std::f32::consts::FRAC_1_SQRT_2;
    let (inner, outer) = (ctx.keep_out + 6.0, to_side - 4.0);
    let radii: Vec<f32> = (0..count)
        .map(|i| if count == 1 { (inner + outer) / 2.0 } else { inner + (outer - inner) * i as f32 / (count - 1) as f32 })
        .collect();

    // Openings: per ring a few directions (vectors of length 1). Each is checked against the openings of the ring
    // inside it, so they don't line up – the way in zigzags.
    let mut openings: Vec<Vec<Vec2>> = Vec::new();
    for ring in 0..count {
        let mut dirs: Vec<Vec2> = Vec::new();
        for _ in 0..p.openings.round() as usize {
            let mut best = tools::random_direction(rng);
            for _ in 0..40 {
                let dir = tools::random_direction(rng);
                let apart = |others: &[Vec2], min_dot: f32| others.iter().all(|o| o.dot(dir) < min_dot);
                let inner_ok = ring == 0 || apart(&openings[ring - 1], 0.93);
                if inner_ok && apart(&dirs, 0.5) {
                    best = dir;
                    break;
                }
            }
            dirs.push(best);
        }
        openings.push(dirs);
    }
    let materials: Vec<Terrain> = (0..count).map(|_| tools::material(p.material, rng)).collect();
    let mix = Noise::new(rng.next_u64() as u32);

    let (bump, clutter) = (Noise::new(rng.next_u64() as u32), Noise::new(rng.next_u64() as u32));
    let wet = Noise::new(rng.next_u64() as u32);
    for (x, y) in tiles.all() {
        let d = vec2(x as f32 + 0.5, y as f32 + 0.5) - ctx.center;
        // Round (straight-line distance) blended with square – square on screen, like the world border: the larger of
        // the two screen axes (|dx| + |dy|), in tiles.
        let round = d.length();
        let square = (d.x.abs() + d.y.abs()) * std::f32::consts::FRAC_1_SQRT_2;
        let dist = round + (square - round) * p.square + (bump.fractal(x as f32, y as f32, 24.0, 3) - 0.5) * 2.0 * p.wobble;
        let mut terrain = Terrain::Grass;
        for (ring, &r) in radii.iter().enumerate() {
            if (dist - r).abs() >= p.thickness / 2.0 {
                continue;
            }
            // In an opening? Within half its width of the line from the base out in its direction.
            let open = openings[ring].iter().any(|dir| d.dot(*dir) > 0.0 && (d.x * dir.y - d.y * dir.x).abs() < p.opening_width / 2.0);
            if !open {
                terrain = tools::material_at(p.material, materials[ring], &mix, x, y);
            }
        }
        // Clutter between the rings: patches of forest, a few ponds – but not right next to a ring, so its
        // openings stay open.
        let near_ring = radii.iter().any(|&r| (dist - r).abs() < p.thickness / 2.0 + 4.0);
        if terrain == Terrain::Grass && !near_ring && ctx.fade(x, y, 6.0) >= 1.0 && p.clutter > 0.0 {
            let c = clutter.fractal(x as f32, y as f32, 14.0, 3);
            if c > 0.7 - 0.25 * p.clutter {
                terrain = if wet.fractal(x as f32, y as f32, 30.0, 2) > 0.62 { Terrain::Water } else { Terrain::Forest };
            }
        }
        tiles.set(x, y, terrain);
    }
    tools::remove_specks(tiles, 5);
    tools::scatter_trees(tiles, rng, p.trees);
}
