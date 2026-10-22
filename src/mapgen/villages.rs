//! Where the villages of doom and the loot lie (DESIGN.md 9.1 step 6, "points of interest" – what they do is in
//! `sim/doom.rs`). Only the spots: how many zombies sleep in a house and what lies in a pile, the world rolls when the
//! round starts (`World::generate`).
//!
//! - **A village**: a spot far from the base (`VILLAGE_FROM`, `VILLAGE_AWAY`) and at least `VILLAGE_GAP` tiles from
//!   every other village, with `houses_near` houses near the base up to `houses_edge` at the edge (the map's setup –
//!   the difficulty makes them more or fewer before), scattered around it: within `VILLAGE_RADIUS`, a big village
//!   further (`village_radius`) – and big villages keep further apart and further from the base. A house is 2 × 2 or
//!   3 × 3 tiles and needs a ring of open ground around it – so it never cuts off a way (whoever walks around it has
//!   the ring), and two houses never touch. On cramped ground a village may get fewer houses than it should: every
//!   house has `TRIES` tries.
//! - **Loot**: single tiles of open ground, away from the base (`LOOT_FROM`, `LOOT_AWAY`), apart from each other and
//!   from the houses.
//!
//! Neither lies on ore or oil; houses don't go into the ways in either (the places to wall off stay as they were).
//! Only `+ - * /` and `sqrt`, like every generator (DESIGN.md 12.8).

use macroquad::math::{vec2, Vec2};

use super::tools::Tiles;
use super::Context;
use crate::map::{from_frame, Deposit, Region};
use crate::rng::Rng;

/// Villages lie at least this far out (share of the way from the base to the edge), and at least `VILLAGE_AWAY` tiles
/// beyond the free space around the base …
const VILLAGE_FROM: f32 = 0.3;
const VILLAGE_AWAY: f32 = 25.0;
/// … and at least this many tiles apart (their middles).
const VILLAGE_GAP: f32 = 30.0;
/// The houses of a village of up to 6 lie this close around its middle (tiles) – a bigger one needs more room.
const VILLAGE_RADIUS: f32 = 8.0;
/// Loot lies at least this far out, and `LOOT_AWAY` tiles beyond the free space around the base – its guards shouldn't
/// jump out next to the command center …
const LOOT_FROM: f32 = 0.15;
const LOOT_AWAY: f32 = 12.0;
/// … at least this many tiles from other loot and from every house.
const LOOT_GAP: f32 = 10.0;
/// Random spots tried per village, per house and per pile before giving up – never endless.
const TRIES: u32 = 30;

/// A house as the generator plans it – the world makes the real one out of it (`World::add_doom`).
#[derive(Clone, Debug, PartialEq)]
pub struct HouseSpot {
    /// The top left tile, width and depth.
    pub tile: (i32, i32),
    pub size: (i32, i32),
}

impl HouseSpot {
    pub fn center(&self) -> Vec2 {
        vec2(self.tile.0 as f32 + self.size.0 as f32 / 2.0, self.tile.1 as f32 + self.size.1 as f32 / 2.0)
    }

    /// The tiles it stands on, and the ring of open ground it needs around it.
    fn tiles_with_ring(&self) -> impl Iterator<Item = (i32, i32)> + use<> {
        let ((x0, y0), (w, h)) = (self.tile, self.size);
        (y0 - 1..=y0 + h).flat_map(move |y| (x0 - 1..=x0 + w).map(move |x| (x, y)))
    }

    /// Would it stand on the other one or on its ring? Then the two aren't at least one free tile apart.
    fn too_close(&self, other: &HouseSpot) -> bool {
        let (ax, ay, aw, ah) = (self.tile.0, self.tile.1, self.size.0, self.size.1);
        let (bx, by, bw, bh) = (other.tile.0 - 1, other.tile.1 - 1, other.size.0 + 2, other.size.1 + 2);
        ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah
    }
}

/// How many villages and piles, how big the villages (from `Common`, the setup).
pub struct Settings {
    /// Villages per 10,000 tiles of the playing field.
    pub villages: f32,
    /// Houses in a village near the base – at the edge `houses_edge`, in between growing with the distance.
    pub houses_near: f32,
    pub houses_edge: f32,
    /// Piles of loot per 10,000 tiles.
    pub loot: f32,
}

/// The spots of all houses and all piles of loot – and the middle of every village.
pub struct Places {
    pub houses: Vec<HouseSpot>,
    pub loot: Vec<(i32, i32)>,
    pub villages: Vec<Vec2>,
}

/// How far around its middle the houses of a village of `houses` lie: `VILLAGE_RADIUS` up to 6 houses, then growing
/// – about the same room for every house.
pub(super) fn village_radius(houses: usize) -> f32 {
    (1.5 + 2.6 * (houses as f32).sqrt()).max(VILLAGE_RADIUS)
}

/// Plans the villages and the piles of loot as `settings` says. `deposits`: the ore and oil already placed – nothing
/// goes on them.
pub fn place(
    tiles: &Tiles,
    deposits: &[(i32, i32, Deposit)],
    rng: &mut Rng,
    ctx: &Context,
    play: &Region,
    settings: &Settings,
) -> Places {
    let n = tiles.size;
    // One yes/no per tile: ore or oil on it? Quicker to ask than searching the list every time.
    let mut ore = vec![false; (n * n) as usize];
    for &(x, y, _) in deposits {
        if tiles.inside(x, y) {
            ore[(y * n + x) as usize] = true;
        }
    }
    // Open ground inside the border without ore – where a house or a pile may lie.
    let open = |x: i32, y: i32| play.contains_tile(x, y) && !tiles.get(x, y).is_solid() && !ore[(y * n + x) as usize];
    // From the base to the middle of a side in a straight line – the same as `World::way_out`.
    let half = (play.max.x - play.min.x) / 2.0 * std::f32::consts::FRAC_1_SQRT_2;
    let out = |p: Vec2| (p.distance(ctx.center) / half).min(1.0);
    let away = |p: Vec2, from: f32, beyond: f32| out(p) >= from && p.distance(ctx.center) >= ctx.keep_out + beyond;
    // A random point in the playing field.
    let random_point = |rng: &mut Rng| from_frame(play.min + vec2(rng.unit_f32(), rng.unit_f32()) * (play.max - play.min));

    let mut places = Places { houses: Vec::new(), loot: Vec::new(), villages: Vec::new() };
    let villages = (settings.villages * play.area() / 10_000.0 + 0.5).floor() as usize;
    // How many houses a village this far out gets. The biggest one decides how far apart villages keep (with 6 houses:
    // `VILLAGE_GAP`) and how much further from the base they keep than small ones – so no house comes closer.
    let houses_at = |out: f32| (settings.houses_near + (settings.houses_edge - settings.houses_near) * out + 0.5).floor().max(0.0) as usize;
    let biggest = village_radius(houses_at(0.0).max(houses_at(1.0)));
    let gap = VILLAGE_GAP.max(2.0 * biggest + 14.0);
    let beyond = VILLAGE_AWAY + biggest - VILLAGE_RADIUS;
    for _ in 0..villages {
        let Some(center) = (0..TRIES).map(|_| random_point(rng)).find(|&p| {
            away(p, VILLAGE_FROM, beyond) && places.villages.iter().all(|c| c.distance(p) >= gap) && open(p.x as i32, p.y as i32)
        }) else {
            continue;
        };
        places.villages.push(center);
        let houses = houses_at(out(center));
        let radius = village_radius(houses);
        for _ in 0..houses {
            for _ in 0..TRIES {
                let side = 2 + rng.below(2) as i32;
                let spot = vec2(rng.unit_f32() * 2.0 - 1.0, rng.unit_f32() * 2.0 - 1.0) * radius + center;
                let house = HouseSpot { tile: (spot.x.floor() as i32, spot.y.floor() as i32), size: (side, side) };
                // Its ring stays open for good: a later house may not stand on it either (`too_close`).
                let free = house.tiles_with_ring().all(|(x, y)| open(x, y) && !ctx.lanes.is_open(x, y));
                if free && !places.houses.iter().any(|other| house.too_close(other)) {
                    places.houses.push(house);
                    break;
                }
            }
        }
    }

    let piles = (settings.loot * play.area() / 10_000.0 + 0.5).floor() as usize;
    for _ in 0..piles {
        let spot = (0..TRIES).map(|_| random_point(rng)).find(|&p| {
            let near_house = places.houses.iter().any(|h| h.center().distance(p) < LOOT_GAP);
            let near_loot = places.loot.iter().any(|&(x, y)| vec2(x as f32 + 0.5, y as f32 + 0.5).distance(p) < LOOT_GAP);
            away(p, LOOT_FROM, LOOT_AWAY) && !near_house && !near_loot && open(p.x as i32, p.y as i32)
        });
        if let Some(p) = spot {
            places.loot.push((p.x as i32, p.y as i32));
        }
    }
    places
}
