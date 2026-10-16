//! **Canyons**: a cellular automaton, the classic way to make caves. Every cell starts as rock or open at random
//! (`fill` = share of rock), then a few rounds of one simple rule: a cell with many rock neighbours becomes rock, one
//! with few becomes open. The noise clumps together into smooth rock masses with winding canyons between them.
//! The cells can be bigger than a tile (`grain`), which makes the canyons wider. Afterwards `connect_pockets` (in
//! `generate`) cuts a way into every closed-off cave.

use serde::{Deserialize, Serialize};

use super::tools::{self, Tiles};
use super::{Context, Field, Show, MATERIALS};
use crate::map::Terrain;
use crate::noise::Noise;
use crate::rng::Rng;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    pub fill: f32,
    pub smoothing: f32,
    pub grain: f32,
    /// Index into `MATERIALS`.
    pub material: f32,
    pub ponds: f32,
    pub trees: f32,
}

impl Default for Params {
    fn default() -> Params {
        Params { fill: 0.46, smoothing: 5.0, grain: 2.0, material: 0.0, ponds: 0.15, trees: 0.02 }
    }
}

impl Params {
    pub fn fields(&mut self) -> Vec<Field<'_>> {
        vec![
            Field { label: "Rock", value: &mut self.fill, min: 0.30, max: 0.60, step: 0.01, show: Show::Percent,
                help: "How much starts as rock. Above ~50 % the canyons get narrow and close up." },
            Field { label: "Smoothing", value: &mut self.smoothing, min: 0.0, max: 10.0, step: 1.0, show: Show::Number,
                help: "Rounds of the rule. 0 = noise, more = smooth rock masses." },
            Field { label: "Canyon width", value: &mut self.grain, min: 1.0, max: 5.0, step: 1.0, show: Show::Number,
                help: "Size of a cell in tiles - bigger cells, wider canyons and bigger rocks." },
            Field { label: "Rock is", value: &mut self.material, min: 0.0, max: 3.0, step: 1.0, show: Show::Choice(MATERIALS),
                help: "Mountains, forest (blocks sight too), mixed or water (islands)." },
            Field { label: "Ponds", value: &mut self.ponds, min: 0.0, max: 1.0, step: 0.05, show: Show::Percent,
                help: "Share of the rock that is water instead (you can see and later shoot across it)." },
            tools::trees_field(&mut self.trees),
        ]
    }
}

pub fn generate(tiles: &mut Tiles, p: &Params, rng: &mut Rng, ctx: &Context) {
    let grain = p.grain.round().max(1.0) as i32;
    let cells = (tiles.size + grain - 1) / grain;
    let index = |x: i32, y: i32| (y * cells + x) as usize;

    // Start: rock at random – but not near the base (it's cleared anyway, and this way the canyons lead up to it).
    let mut rock: Vec<bool> = (0..cells * cells)
        .map(|i| {
            let (x, y) = ((i % cells) * grain + grain / 2, (i / cells) * grain + grain / 2);
            rng.unit_f32() < p.fill * ctx.fade(x, y, 10.0)
        })
        .collect();
    for _ in 0..p.smoothing.round() as u32 {
        rock = step(&rock, cells);
    }

    // Cells → tiles, then two rounds on the tiles themselves, which rounds off the stairs of big cells.
    let mut solid: Vec<bool> = tiles.all().map(|(x, y)| rock[index(x / grain, y / grain)]).collect();
    if grain > 1 {
        for _ in 0..2 {
            solid = step(&solid, tiles.size);
        }
    }

    let (kind, wet) = (Noise::new(rng.next_u64() as u32), Noise::new(rng.next_u64() as u32));
    let base_material = tools::material(p.material, rng);
    for (i, (x, y)) in tiles.all().enumerate() {
        if !solid[i] {
            continue;
        }
        let terrain = if p.ponds > 0.0 && wet.fractal(x as f32, y as f32, 20.0, 3) < 0.28 + 0.3 * p.ponds {
            Terrain::Water
        } else if p.material.round() as i32 == 2 {
            // Mixed: patches of rock and forest.
            if kind.fractal(x as f32, y as f32, 24.0, 3) > 0.5 { Terrain::Mountain } else { Terrain::Forest }
        } else {
            base_material
        };
        tiles.set(x, y, terrain);
    }
    tools::scatter_trees(tiles, rng, p.trees);
}

/// One round of the rule ("4-5 rule"): rock with at least 4 rock neighbours stays rock, an open cell with at least 5
/// becomes rock, everything else is open. Outside the grid counts as rock, so the canyons don't run off the edge.
fn step(rock: &[bool], size: i32) -> Vec<bool> {
    let at = |x: i32, y: i32| x < 0 || y < 0 || x >= size || y >= size || rock[(y * size + x) as usize];
    (0..size * size)
        .map(|i| {
            let (x, y) = (i % size, i / size);
            let mut around = 0;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if (dx, dy) != (0, 0) && at(x + dx, y + dy) {
                        around += 1;
                    }
                }
            }
            if at(x, y) { around >= 4 } else { around >= 5 }
        })
        .collect()
}
