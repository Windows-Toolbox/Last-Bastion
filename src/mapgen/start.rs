//! The start area (DESIGN.md 9.1, step 2): what the first minutes need lies within reach of the command center on
//! every map – whatever the noise did.
//!
//! Factorio does it per resource: besides the spots of the whole map, a second "starting area" spot noise puts one
//! patch of every basic ore near the spawn, and there is always a lake close to it. Here the ore does the same
//! (`ores.rs`, step 1), and for the wood there is this one rule: **first look, then help**. Only if the best spot for
//! a sawmill inside the command center's grid works too little forest does a grove grow at the edge of the grid –
//! and another, until it's enough. A map whose noise already put a forest there stays untouched. The clearing stays
//! free: if it's wider than the grid, the forest lies one tesla tower further out.
//!
//! How far the grid reaches and how big a sawmill is and how far it works come from `data\buildings\` – change
//! them there and the start area follows.

use macroquad::math::vec2;

use super::tools::{self, Tiles};
use super::Context;
use crate::kinds::BuildingKind;
use crate::map::Terrain;
use crate::noise::Noise;
use crate::rng::Rng;

/// Forest tiles the best sawmill inside the command center's grid works at least (TAB-like: a sawmill at the start
/// is worth building).
pub const MIN_FOREST: usize = 20;
/// At most this many groves – then the map is as it is (the start area is full of water or rock).
const MAX_GROVES: usize = 6;
/// Radius of one grove (tiles), give or take: 3 to 4.5.
const GROVE_RADIUS: f32 = 3.0;

/// Groves at the edge of the command center's grid until a sawmill there works `MIN_FOREST` tiles.
pub fn ensure_forest(tiles: &mut Tiles, rng: &mut Rng, ctx: &Context) {
    for _ in 0..MAX_GROVES {
        if best_sawmill(tiles, ctx) >= MIN_FOREST {
            return;
        }
        plant_grove(tiles, rng, ctx);
    }
}

/// How far from the command center the middle of a sawmill may stand: inside its grid – or, if the clearing is wider
/// than that (no forest grows in it), one tesla tower further.
pub fn sawmill_reach(ctx: &Context) -> f32 {
    if ctx.keep_out + 2.0 <= ctx.grid { ctx.grid } else { ctx.grid + ctx.relay }
}

/// How many forest tiles the best sawmill within `sawmill_reach` would work – standing on open ground, counting the
/// forest in its square (its site + `gather_range` around it).
pub fn best_sawmill(tiles: &Tiles, ctx: &Context) -> usize {
    let stats = BuildingKind::Sawmill.stats();
    let (w, h) = BuildingKind::Sawmill.footprint(false);
    let range = stats.gather_range;
    let allowed = sawmill_reach(ctx);
    let reach = allowed.ceil() as i32 + w.max(h);
    let (cx, cy) = (ctx.center.x.floor() as i32, ctx.center.y.floor() as i32);
    let mut best = 0;
    for ty in cy - reach..=cy + reach {
        for tx in cx - reach..=cx + reach {
            let middle = vec2(tx as f32 + w as f32 / 2.0, ty as f32 + h as f32 / 2.0);
            let open = (ty..ty + h).all(|y| (tx..tx + w).all(|x| !tiles.get(x, y).is_solid()));
            if middle.distance(ctx.center) > allowed || !open {
                continue;
            }
            let forest = (ty - range..ty + h + range)
                .flat_map(|y| (tx - range..tx + w + range).map(move |x| (x, y)))
                .filter(|&(x, y)| tiles.get(x, y) == Terrain::Forest && tiles.inside(x, y))
                .count();
            best = best.max(forest);
        }
    }
    best
}

/// One grove – a frayed disc of forest like an ore patch – just at the edge of the grid (or the clearing, if that's
/// wider), where a sawmill reaches it. Only on open ground (water and rock stay), never in the clearing or a way in,
/// and not where a way in passes close by.
fn plant_grove(tiles: &mut Tiles, rng: &mut Rng, ctx: &Context) {
    let shape = Noise::new(rng.next_u64() as u32);
    let radius = GROVE_RADIUS + rng.unit_f32() * 1.5;
    let distance = ctx.grid.max(ctx.keep_out + 2.0);
    let center = ctx.center + tools::random_direction(rng) * (distance + rng.unit_f32() * 2.5);
    let reach = (radius * 1.45).ceil() as i32 + 1;
    let (cx, cy) = (center.x.floor() as i32, center.y.floor() as i32);
    for y in cy - reach..=cy + reach {
        for x in cx - reach..=cx + reach {
            let middle = vec2(x as f32 + 0.5, y as f32 + 0.5);
            let edge = radius * (0.55 + 0.9 * shape.fractal(x as f32, y as f32, 5.0, 3));
            let in_clearing = middle.distance(ctx.center) <= ctx.keep_out;
            if middle.distance(center) <= edge && !in_clearing && !tiles.get(x, y).is_solid() {
                tiles.set(x, y, Terrain::Forest);
            }
        }
    }
}
