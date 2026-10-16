//! **Wild**: noise like in Factorio (DESIGN.md 9.1, step 1). The old generator, repaired:
//! - **Wobble** (domain warping) instead of plain value noise – frayed, stretched shapes instead of round blobs that
//!   follow the noise grid.
//! - **Mountain ridges**: "ridged" noise makes long chains of rock (walls with gaps – chokepoints), not just lumps.
//! - **Rivers** are real paths from one edge to another (`tools::river`) instead of a thin band of noise – those
//!   broke into pieces, made loops and ended at the clearing.
//! - Around the base everything **fades out** smoothly instead of a square cut out with scissors.

use serde::{Deserialize, Serialize};

use super::tools::{self, level, Tiles, Warp};
use super::{Context, Field, Show};
use crate::map::Terrain;
use crate::noise::Noise;
use crate::rng::Rng;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    /// How many tiles across the big features are.
    pub scale: f32,
    /// Domain warping: how far (tiles) the noise is shifted – 0 = round blobs.
    pub wobble: f32,
    /// Shares of the map.
    pub water: f32,
    pub mountains: f32,
    /// 0 = mountains as round massifs, 1 = as long thin ridges.
    pub ridges: f32,
    pub forest: f32,
    pub trees: f32,
    pub rivers: f32,
    pub river_width: f32,
    pub meander: f32,
}

impl Default for Params {
    fn default() -> Params {
        Params {
            scale: 32.0,
            wobble: 14.0,
            water: 0.06,
            mountains: 0.08,
            ridges: 0.6,
            forest: 0.16,
            trees: 0.015,
            rivers: 1.0,
            river_width: 2.0,
            meander: 0.5,
        }
    }
}

impl Params {
    pub fn fields(&mut self) -> Vec<Field<'_>> {
        vec![
            Field { label: "Feature size", value: &mut self.scale, min: 12.0, max: 64.0, step: 1.0, show: Show::Tiles,
                help: "How big lakes, forests and mountain ranges are." },
            Field { label: "Wobble", value: &mut self.wobble, min: 0.0, max: 40.0, step: 1.0, show: Show::Tiles,
                help: "Bends the shapes: 0 = round blobs, more = frayed, stretched, organic." },
            Field { label: "Lakes", value: &mut self.water, min: 0.0, max: 0.30, step: 0.01, show: Show::Percent,
                help: "Share of the map under water (the lowest ground)." },
            Field { label: "Mountains", value: &mut self.mountains, min: 0.0, max: 0.30, step: 0.01, show: Show::Percent,
                help: "Share of the map that is rock. Nobody walks over it, but you can see over it." },
            Field { label: "Ridges", value: &mut self.ridges, min: 0.0, max: 1.0, step: 0.05, show: Show::Percent,
                help: "0 = mountains as round massifs, 100 % = long thin ridges with gaps - chokepoints." },
            Field { label: "Forest", value: &mut self.forest, min: 0.0, max: 0.45, step: 0.01, show: Show::Percent,
                help: "Share of the map that is forest. Blocks walking and sight." },
            tools::trees_field(&mut self.trees),
            Field { label: "Rivers", value: &mut self.rivers, min: 0.0, max: 5.0, step: 1.0, show: Show::Number,
                help: "Rivers from one edge of the map to another, around the base. Fords appear where needed." },
            Field { label: "River width", value: &mut self.river_width, min: 1.0, max: 7.0, step: 1.0, show: Show::Tiles,
                help: "How wide the rivers are." },
            Field { label: "River bends", value: &mut self.meander, min: 0.0, max: 1.0, step: 0.05, show: Show::Percent,
                help: "0 = nearly straight, 100 % = big loops." },
        ]
    }
}

pub fn generate(tiles: &mut Tiles, p: &Params, rng: &mut Rng, ctx: &Context) {
    let mut seed = || rng.next_u64() as u32;
    let (height, rock, rock_ridge, moisture) = (Noise::new(seed()), Noise::new(seed()), Noise::new(seed()), Noise::new(seed()));
    let warp = Warp::new(rng, p.scale * 1.5, p.wobble);

    // Per tile: height (→ water), rock (→ mountains), moisture (→ forest). Near the base each fades towards a value
    // that is none of these, so features shrink smoothly before the clearing instead of being cut off.
    let n = (tiles.size * tiles.size) as usize;
    let (mut h, mut r, mut m) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
    for (x, y) in tiles.all() {
        let w = warp.apply(x as f32, y as f32);
        let fade = ctx.fade(x, y, 14.0);
        let height_here = height.fractal(w.x, w.y, p.scale, 5);
        // Ridged noise: 1 where the noise crosses its middle – that's a thin winding line, a ridge.
        let ridge = 1.0 - (rock_ridge.fractal(w.x, w.y, p.scale * 1.6, 4) - 0.5).abs() * 2.0;
        let lump = rock.fractal(w.x, w.y, p.scale, 4);
        let rock_here = lump + (ridge - lump) * p.ridges;
        h.push(0.5 + (height_here - 0.5) * fade);
        r.push(rock_here * fade);
        m.push(moisture.fractal(w.x, w.y, p.scale * 0.8, 4) * fade);
    }
    let water = if p.water > 0.0 { level(&h, p.water) } else { f32::MIN };
    let mountain = if p.mountains > 0.0 { level(&r, 1.0 - p.mountains) } else { f32::MAX };
    let forest = if p.forest > 0.0 { level(&m, 1.0 - p.forest) } else { f32::MAX };

    for (i, (x, y)) in tiles.all().enumerate() {
        let terrain = if h[i] <= water {
            Terrain::Water
        } else if r[i] >= mountain {
            Terrain::Mountain
        } else if m[i] >= forest {
            Terrain::Forest
        } else {
            Terrain::Grass
        };
        tiles.set(x, y, terrain);
    }
    for _ in 0..p.rivers.round() as u32 {
        tools::river(tiles, rng, ctx, p.river_width, p.meander);
    }
    tools::scatter_trees(tiles, rng, p.trees);
}
