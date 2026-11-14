//! The editor's terrain tools that work on more than the tiles under the brush (`docs\editor.md`, step 3):
//! - **Smooth** (`Order::SmoothTerrain`): every tile takes the ground most of its neighbours have – single specks go,
//!   ragged edges get round.
//! - **Copy and paste an area** (`World::copy_terrain`, `Order::PasteTerrain`): the ground of a rectangle of tiles
//!   with what belongs to it – forest, rock, water, ore, big rocks and which pictures they're drawn with. Not what
//!   stands on it: that's the selection's copy (`clipboard.rs`).
//! - **Generate in an area** (`Order::Generate`): a patch of forest, a lake, mountains, a field of ore or a village of
//!   doom, made the way TAB's generator makes them (`mapgen/tab.rs`) – only inside the rectangle, from a seed of its
//!   own: the same seed, the same patch.
//! - **Keep buildings and tracks free** (`World::spare`): the tiles a painting tool leaves alone when that's ticked.
//!
//! Like everything of the generator: only `+ - * /` on decimal numbers, so the same order gives the same map on every
//! PC (DESIGN.md 12.8).

use std::collections::HashSet;

use super::{rail_pieces, World};
use crate::data;
use crate::map::{BigRock, Deposit, Ore, Terrain};
use crate::mapgen::spiral;
use crate::noise::simplex_octaves;
use crate::rng::Rng;

/// How much ore a generated field puts on a tile – like the editor's brush (`orders.rs`).
const FIELD_ORE: u32 = 1000;
/// How much the noise of a patch loses towards the edge of its area: at the middle of an edge this much, in a corner
/// twice as much – so a patch is a blob in the middle of its rectangle, not cut off straight at its edges.
const EDGE: f64 = 0.6;
/// A tile is smoothed if at least this many of its 8 neighbours have the same other ground.
const SMOOTH_FROM: u32 = 5;
const NEIGHBOURS: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// A rectangle of tiles: its corner with the smallest x and y, and the one with the biggest – both belong to it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Area {
    pub min: (i32, i32),
    pub max: (i32, i32),
}

impl Area {
    /// The rectangle between two tiles, whichever way round they are.
    pub fn between(a: (i32, i32), b: (i32, i32)) -> Area {
        Area { min: (a.0.min(b.0), a.1.min(b.1)), max: (a.0.max(b.0), a.1.max(b.1)) }
    }

    /// Width and depth in tiles.
    pub fn size(self) -> (i32, i32) {
        (self.max.0 - self.min.0 + 1, self.max.1 - self.min.1 + 1)
    }

    pub fn contains(self, (x, y): (i32, i32)) -> bool {
        (self.min.0..=self.max.0).contains(&x) && (self.min.1..=self.max.1).contains(&y)
    }

    /// All its tiles, row by row.
    pub fn tiles(self) -> impl Iterator<Item = (i32, i32)> {
        (self.min.1..=self.max.1).flat_map(move |y| (self.min.0..=self.max.0).map(move |x| (x, y)))
    }
}

/// What "Generate in the area" can make.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Patch {
    /// Trees on the open ground of the area – water and rock stay.
    Forest,
    Lake,
    /// In blocks of 2 × 2 tiles, like TAB's.
    Mountains,
    /// A field of this ore on the open ground that has none yet (not oil – that lies in nodes).
    Ore(Ore),
    /// A village of doom: a circle cleared, a house in its middle, houses in a spiral around it.
    Village,
}

impl Patch {
    /// The ones the editor offers, in the order of its list.
    pub const ALL: [Patch; 8] = [
        Patch::Forest,
        Patch::Lake,
        Patch::Mountains,
        Patch::Ore(Ore::Stone),
        Patch::Ore(Ore::Iron),
        Patch::Ore(Ore::Coal),
        Patch::Ore(Ore::Gold),
        Patch::Village,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Patch::Forest => "Forest",
            Patch::Lake => "Lake",
            Patch::Mountains => "Mountains",
            Patch::Ore(Ore::Stone) => "Stone field",
            Patch::Ore(Ore::Iron) => "Iron field",
            Patch::Ore(Ore::Coal) => "Coal field",
            Patch::Ore(_) => "Gold field",
            Patch::Village => "Village of doom",
        }
    }

    /// Does "how much of the area" mean anything for it? A village always takes the circle that fits.
    pub fn has_amount(self) -> bool {
        self != Patch::Village
    }
}

/// The ground of one tile, as a copy keeps it.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Ground {
    terrain: Terrain,
    picture: u8,
    deposit: Option<Deposit>,
}

/// A copy of the ground of an area (`World::copy_terrain`).
#[derive(Clone, PartialEq, Debug)]
pub struct TerrainClip {
    /// Width and depth in tiles.
    pub size: (i32, i32),
    /// Row by row – `None`: that tile was off the map, pasting leaves alone what lies there.
    tiles: Vec<Option<Ground>>,
    /// The big rocks that lay in it with all their tiles – `tile` counted from the area's corner.
    big_rocks: Vec<BigRock>,
}

impl World {
    /// Editor, "Keep buildings and tracks free": takes out of `tiles` every tile on or right next to a building or a
    /// house of doom (corners count – a door stays reachable from every side), and every tile on railway track or
    /// beside it (corners don't count, like the rule for building there – `placement.rs`).
    pub fn spare(&self, tiles: &mut Vec<(i32, i32)>) {
        // A `HashSet`: a set of values that answers "is this one in it?" at once, however many there are – like
        // Python's `set`.
        let mut track: HashSet<(i32, i32)> = HashSet::new();
        for piece in rail_pieces(&self.ornaments) {
            let side = piece.shape.size();
            for (dx, dy) in (0..side).flat_map(|dy| (0..side).map(move |dx| (dx, dy))) {
                let tile = (piece.tile.0 + dx, piece.tile.1 + dy);
                if piece.covers(tile) {
                    track.extend([tile, (tile.0 + 1, tile.1), (tile.0 - 1, tile.1), (tile.0, tile.1 + 1), (tile.0, tile.1 - 1)]);
                }
            }
        }
        let map = &self.map;
        let taken = |x: i32, y: i32| map.on_map(x, y) && (map.is_built(x, y) || map.is_doom(x, y));
        tiles.retain(|&(x, y)| !track.contains(&(x, y)) && !taken(x, y) && !NEIGHBOURS.iter().any(|&(dx, dy)| taken(x + dx, y + dy)));
    }

    /// `Order::SmoothTerrain`: each of these tiles takes the ground at least `SMOOTH_FROM` of its 8 neighbours share.
    /// All of them look at the map as it was before – the order of the tiles doesn't matter.
    pub(super) fn smooth(&mut self, tiles: &[(i32, i32)]) {
        const KINDS: [Terrain; 5] = [Terrain::Grass, Terrain::Dirt, Terrain::Mountain, Terrain::Forest, Terrain::Water];
        let map = &self.map;
        let changes: Vec<((i32, i32), Terrain)> = tiles
            .iter()
            .filter_map(|&(x, y)| {
                if !map.on_map(x, y) {
                    return None;
                }
                // How many neighbours of each kind – off the map none.
                let mut counts = [0u32; KINDS.len()];
                for (dx, dy) in NEIGHBOURS {
                    if map.on_map(x + dx, y + dy) {
                        let kind = map.terrain(x + dx, y + dy);
                        counts[KINDS.iter().position(|&k| k == kind).unwrap_or(0)] += 1;
                    }
                }
                // At least 5 of 8: there can only be one such kind.
                let most = (0..KINDS.len()).find(|&k| counts[k] >= SMOOTH_FROM)?;
                (KINDS[most] != map.terrain(x, y)).then_some(((x, y), KINDS[most]))
            })
            .collect();
        for ((x, y), terrain) in changes {
            if self.map.set_terrain(x, y, terrain) {
                self.map_changed = true;
            }
        }
    }

    /// `Order::PaintPicture`: the forest, rock or ore on these tiles is drawn with this group of pictures (0: mixed).
    pub(super) fn paint_picture(&mut self, picture: u8, tiles: &[(i32, i32)]) {
        for &(x, y) in tiles {
            self.map.set_picture(x, y, picture);
        }
    }

    /// A copy of the ground of `area` – only reads the world, like `copy`.
    pub fn copy_terrain(&self, area: Area) -> TerrainClip {
        let map = &self.map;
        let is_oil = |x: i32, y: i32| map.deposit(x, y).is_some_and(|d| d.ore == Ore::Oil);
        let tiles = area
            .tiles()
            .map(|(x, y)| {
                // An oil node cut by the edge of the area stays behind: half a node is no node (its tiles are all
                // neighbours of each other, so each of them sees the one outside).
                let cut = is_oil(x, y) && NEIGHBOURS.iter().any(|&(dx, dy)| is_oil(x + dx, y + dy) && !area.contains((x + dx, y + dy)));
                map.on_map(x, y).then(|| Ground { terrain: map.terrain(x, y), picture: map.picture(x, y), deposit: map.deposit(x, y).filter(|_| !cut) })
            })
            .collect();
        let big_rocks = map
            .big_rocks()
            .iter()
            .filter(|rock| rock.tiles().all(|tile| area.contains(tile)))
            .map(|rock| BigRock { tile: (rock.tile.0 - area.min.0, rock.tile.1 - area.min.1), ..*rock })
            .collect();
        TerrainClip { size: area.size(), tiles, big_rocks }
    }

    /// `Order::PasteTerrain`: the copy's ground onto the map, its corner on the tile `to`. Tiles with a building or a
    /// house of doom stay as they are (`Map::set_terrain`) – with `spare` the tiles around them and the railway too.
    pub(super) fn paste_terrain(&mut self, clip: &TerrainClip, to: (i32, i32), spare: bool) {
        let (w, h) = clip.size;
        let area = Area { min: to, max: (to.0 + w - 1, to.1 + h - 1) };
        let mut tiles: Vec<(i32, i32)> = area.tiles().collect();
        if spare {
            self.spare(&mut tiles);
        }
        for (x, y) in tiles {
            let Some(ground) = clip.tiles[((y - to.1) * w + (x - to.0)) as usize] else {
                continue;
            };
            if !self.map.on_map(x, y) || self.map.is_built(x, y) || self.map.is_doom(x, y) {
                continue;
            }
            self.map.set_terrain(x, y, ground.terrain);
            self.map.set_deposit(x, y, ground.deposit);
            self.map.set_picture(x, y, ground.picture);
        }
        for rock in &clip.big_rocks {
            let rock = BigRock { tile: (to.0 + rock.tile.0, to.1 + rock.tile.1), ..*rock };
            // With `spare`: only a rock none of whose tiles is to be left alone.
            let mut on: Vec<(i32, i32)> = rock.tiles().collect();
            if spare {
                self.spare(&mut on);
            }
            if on.len() == (rock.size * rock.size) as usize {
                self.map.place_big_rock(rock);
            }
        }
        self.map_changed = true;
    }

    /// `Order::Generate`: a patch of `what` in `area`. `percent`: how much of the area it takes (not a village).
    /// `spare`: leave buildings, houses of doom and the railway free (`spare`).
    pub(super) fn generate_patch(&mut self, what: Patch, area: Area, percent: u32, seed: u64, spare: bool) {
        let mut rng = Rng::new(seed);
        if what == Patch::Village {
            self.generate_village(area, &mut rng, spare);
            return;
        }
        // How fine the noise is and in how many layers: a forest's edge is ragged, a lake's shore smooth, ore lies in
        // crumbs (TAB's ore noise is the finest of its layers).
        let (scale, octaves) = match what {
            Patch::Forest => (0.15, 3),
            Patch::Lake | Patch::Mountains => (0.08, 2),
            _ => (0.2, 1),
        };
        let mut tiles = blob(area, &mut rng, percent, scale, octaves);
        if what == Patch::Mountains {
            tiles = in_blocks(area, &tiles);
        }
        if spare {
            self.spare(&mut tiles);
        }
        for (x, y) in tiles {
            let open = !self.map.terrain(x, y).is_solid();
            match what {
                Patch::Forest if open => self.map_changed |= self.map.set_terrain(x, y, Terrain::Forest),
                Patch::Lake => self.map_changed |= self.map.set_terrain(x, y, Terrain::Water),
                Patch::Mountains => self.map_changed |= self.map.set_terrain(x, y, Terrain::Mountain),
                Patch::Ore(ore) if ore != Ore::Oil && self.map.deposit(x, y).is_none() => {
                    self.map.set_deposit(x, y, Some(Deposit { ore, amount: FIELD_ORE }));
                }
                _ => {}
            }
        }
    }

    /// A village of doom like TAB's generator makes one (`mapgen/tab.rs`, step 5) in the circle that fits into `area`:
    /// the circle cleared (water, forest and rock become bare earth), a house in the middle – a big one in a big
    /// village –, then houses in a spiral around it: every fifth spot left out, two of three small, each with a tile
    /// of free ground around it. As full as the map's difficulty makes them (`doom_zombies`).
    fn generate_village(&mut self, area: Area, rng: &mut Rng, spare: bool) {
        let (w, h) = area.size();
        let r = w.min(h) / 2;
        let c = (area.min.0 + w / 2, area.min.1 + h / 2);
        let in_circle = |(x, y): (i32, i32)| (x - c.0) * (x - c.0) + (y - c.1) * (y - c.1) < (r * r).max(1);
        let mut circle: Vec<(i32, i32)> = area.tiles().filter(|&t| in_circle(t)).collect();
        if spare {
            self.spare(&mut circle);
        }
        for &(x, y) in &circle {
            if self.map.terrain(x, y).is_solid() && self.map.set_terrain(x, y, Terrain::Dirt) {
                self.map_changed = true;
            }
        }
        let population = data::get().difficulty(&self.difficulty).population;
        // A house of `side` tiles around `at` (TAB's middle tile) – if it and the ring around it are free and cleared.
        let cleared: HashSet<(i32, i32)> = circle.into_iter().collect();
        let build = |world: &mut World, at: (i32, i32), side: i32, ring: i32| {
            let tile = (at.0 - (side - 1) / 2, at.1 - (side - 1) / 2);
            let on_cleared = (0..side).all(|dy| (0..side).all(|dx| cleared.contains(&(tile.0 + dx, tile.1 + dy))));
            let fits = on_cleared && world.doom_fits((tile.0 - ring, tile.1 - ring), (side + 2 * ring, side + 2 * ring));
            if fits {
                world.add_doom(tile, (side, side), World::doom_zombies((side, side), population), population);
            }
            fits
        };
        let middle = match r {
            r if r > 8 => 5,
            r if r >= 3 => 3,
            _ => 2,
        };
        build(self, c, middle, 0);
        let mut k = 0;
        for (x, y) in spiral(c.0, c.1, false) {
            if (x - c.0).abs() > r {
                break;
            }
            if !in_circle((x, y)) || rng.below(5) > 3 {
                continue;
            }
            let side = if r > 12 && k % 8 == 0 {
                5
            } else if k % 3 > 0 {
                2
            } else {
                3
            };
            if build(self, (x, y), side, 1) {
                k += 1;
            }
        }
    }
}

/// The `percent` of the area's tiles where TAB's noise is highest – less of it the further from the middle (`EDGE`),
/// so the patch is one blob with a ragged edge. Where in the endless noise it lies, the seed says.
fn blob(area: Area, rng: &mut Rng, percent: u32, scale: f64, octaves: u32) -> Vec<(i32, i32)> {
    // Three random offsets, like TAB's generator rolls them for a map.
    let mut offset = || (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64 * 1000.0;
    let (dx, dy, z) = (offset(), offset(), offset());
    let (w, h) = area.size();
    let (cx, cy) = (area.min.0 as f64 + (w - 1) as f64 / 2.0, area.min.1 as f64 + (h - 1) as f64 / 2.0);
    let mut valued: Vec<(f64, (i32, i32))> = area
        .tiles()
        .map(|(x, y)| {
            // 0 in the middle, 1 at the middle of an edge, 2 in a corner.
            let (ex, ey) = ((x as f64 - cx) / (w as f64 / 2.0), (y as f64 - cy) / (h as f64 / 2.0));
            let noise = simplex_octaves(dx + scale * x as f64, dy + scale * y as f64, z, octaves) as f64;
            (noise - EDGE * (ex * ex + ey * ey), (x, y))
        })
        .collect();
    // The highest first. `total_cmp`: decimal numbers have no order Rust trusts by itself (a NaN fits nowhere) – this
    // one puts every value somewhere. A stable sort: equal values keep the order of the tiles.
    valued.sort_by(|a, b| b.0.total_cmp(&a.0));
    let count = valued.len() * percent.min(100) as usize / 100;
    valued.into_iter().take(count).map(|(_, tile)| tile).collect()
}

/// TAB's mountains stand in blocks of 2 × 2 tiles (at even x and y – `ground.rs` puts a rock piece on each): every
/// block of the area with at least two of its tiles in `tiles`, whole.
fn in_blocks(area: Area, tiles: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let picked: HashSet<(i32, i32)> = tiles.iter().copied().collect();
    let mut blocks = Vec::new();
    // `rem_euclid(2)`: 0 for even, 1 for odd – also for negative numbers. The first even tile in the area.
    let (x0, y0) = (area.min.0 + area.min.0.rem_euclid(2), area.min.1 + area.min.1.rem_euclid(2));
    for y in (y0..area.max.1).step_by(2) {
        for x in (x0..area.max.0).step_by(2) {
            let block = [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)];
            if block.iter().filter(|t| picked.contains(t)).count() >= 2 {
                blocks.extend(block);
            }
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{Order, Weather};

    /// A world with nothing on it – all grass – and an area of 30 × 30 tiles in its playing field.
    fn bare(seed: u64) -> (World, Area) {
        let mut world = World::new(seed);
        let n = world.map.size as i32;
        let all: Vec<(i32, i32)> = (0..n).flat_map(|y| (0..n).map(move |x| (x, y))).collect();
        world.apply_orders(&[
            Order::RemoveAt { tiles: all.clone() },
            Order::PaintOre { ore: None, tiles: all.clone() },
            Order::PaintTerrain { terrain: Terrain::Grass, tiles: all },
        ]);
        let (bx, by) = (world.base.x as i32, world.base.y as i32);
        (world, Area::between((bx - 20, by - 10), (bx + 9, by + 19)))
    }

    fn count(world: &World, terrain: Terrain) -> usize {
        let n = world.map.size as i32;
        (0..n).flat_map(|y| (0..n).map(move |x| (x, y))).filter(|&(x, y)| world.map.on_map(x, y) && world.map.terrain(x, y) == terrain).count()
    }

    /// Smoothing takes a single speck away and rounds a corner, a straight edge and the inside stay.
    #[test]
    fn smoothing_takes_specks_and_rounds_corners() {
        let (mut world, area) = bare(5);
        let (x, y) = area.min;
        let block = Area::between((x + 5, y + 5), (x + 14, y + 14));
        world.apply_orders(&[
            Order::PaintTerrain { terrain: Terrain::Forest, tiles: block.tiles().collect() },
            Order::PaintTerrain { terrain: Terrain::Water, tiles: vec![(x + 22, y + 22)] },
            Order::SmoothTerrain { tiles: area.tiles().collect() },
        ]);
        assert_eq!(world.map.terrain(x + 22, y + 22), Terrain::Grass, "the speck stayed");
        assert_eq!(world.map.terrain(x + 5, y + 5), Terrain::Grass, "the corner stayed");
        assert_eq!(world.map.terrain(x + 5, y + 9), Terrain::Forest, "the straight edge went");
        assert_eq!(world.map.terrain(x + 9, y + 9), Terrain::Forest);
        assert_eq!(count(&world, Terrain::Forest), 96, "more than the four corners went");
    }

    /// A patch takes its share of the area, stays inside it, and is the same for the same seed.
    #[test]
    fn patches_stay_in_their_area_and_come_from_their_seed() {
        let (mut world, area) = bare(5);
        let forest = |seed: u64| Order::Generate { what: Patch::Forest, area, percent: 60, seed, spare: false };
        world.apply_orders(&[forest(7)]);
        assert_eq!(count(&world, Terrain::Forest), 540, "60 % of 900 tiles");
        assert_eq!(area.tiles().filter(|&(x, y)| world.map.terrain(x, y) == Terrain::Forest).count(), 540, "forest outside the area");
        let (mut again, _) = bare(5);
        again.apply_orders(&[forest(7)]);
        assert!(world.map.same_terrain(&again.map), "the same seed made another forest");
        let (mut other, _) = bare(5);
        other.apply_orders(&[forest(8)]);
        assert!(!world.map.same_terrain(&other.map), "another seed made the same forest");

        // A lake goes over the forest; mountains stand in whole blocks of 2 × 2 at even tiles.
        world.apply_orders(&[Order::Generate { what: Patch::Lake, area, percent: 40, seed: 3, spare: false }]);
        assert_eq!(count(&world, Terrain::Water), 360);
        let (mut rocky, _) = bare(5);
        rocky.apply_orders(&[Order::Generate { what: Patch::Mountains, area, percent: 50, seed: 3, spare: false }]);
        let rocks: Vec<(i32, i32)> = area.tiles().filter(|&(x, y)| rocky.map.terrain(x, y) == Terrain::Mountain).collect();
        assert!(rocks.len() > 200 && rocks.len() == count(&rocky, Terrain::Mountain), "{} tiles of mountain", rocks.len());
        for (x, y) in rocks {
            let (bx, by) = (x - x.rem_euclid(2), y - y.rem_euclid(2));
            assert!([(0, 0), (1, 0), (0, 1), (1, 1)].iter().all(|(dx, dy)| rocky.map.terrain(bx + dx, by + dy) == Terrain::Mountain), "half a block at {x}, {y}");
        }

        // Ore only on open ground: none in the lake.
        world.apply_orders(&[Order::Generate { what: Patch::Ore(Ore::Iron), area, percent: 100, seed: 1, spare: false }]);
        let ore = world.map.all_deposits().count();
        assert!(ore > 100 && ore <= 900 - 360, "{ore} tiles of ore");
        assert!(world.map.all_deposits().all(|(x, y, d)| d.ore == Ore::Iron && d.amount == FIELD_ORE && !world.map.terrain(x, y).is_solid()));
    }

    /// A village of doom: the circle cleared, a big house in the middle of a big one, houses around it – all inside
    /// the area, a tile apart.
    #[test]
    fn a_village_clears_its_circle_and_stands_inside_the_area() {
        let (mut world, area) = bare(5);
        let village = Order::Generate { what: Patch::Village, area, percent: 100, seed: 11, spare: false };
        world.apply_orders(&[Order::PaintTerrain { terrain: Terrain::Forest, tiles: area.tiles().collect() }, village.clone()]);
        assert!(world.doom.len() > 4, "only {} houses", world.doom.len());
        assert_eq!(world.doom[0].size, (5, 5), "no big house in the middle");
        assert!(world.doom.iter().all(|house| house.inside > 0), "an empty house");
        for house in &world.doom {
            let tiles = Area { min: house.tile, max: (house.tile.0 + house.size.0 - 1, house.tile.1 + house.size.1 - 1) };
            assert!(tiles.tiles().all(|t| area.contains(t)), "a house outside the area");
            // No other house on it or right next to it.
            let ring = Area { min: (tiles.min.0 - 1, tiles.min.1 - 1), max: (tiles.max.0 + 1, tiles.max.1 + 1) };
            let others = world.doom.iter().filter(|other| other.id != house.id);
            let mut their_tiles = others.flat_map(|o| Area { min: o.tile, max: (o.tile.0 + o.size.0 - 1, o.tile.1 + o.size.1 - 1) }.tiles());
            assert!(their_tiles.all(|t| !ring.contains(t)), "two houses touch");
        }
        // The corners of the area lie outside the circle: still forest.
        assert_eq!(world.map.terrain(area.min.0, area.min.1), Terrain::Forest);
        assert!(count(&world, Terrain::Dirt) > 300, "the circle is still forest");
        let (mut again, _) = bare(5);
        again.apply_orders(&[Order::PaintTerrain { terrain: Terrain::Forest, tiles: area.tiles().collect() }, village]);
        let houses = |world: &World| world.doom.iter().map(|h| (h.tile, h.size)).collect::<Vec<_>>();
        assert_eq!(houses(&world), houses(&again), "the same seed built another village");
    }

    /// A copied area brings its ground, its ore, its pictures and its big rocks – not an oil node its edge cuts.
    #[test]
    fn a_copy_keeps_ground_ore_pictures_and_rocks() {
        let (mut world, area) = bare(5);
        let (x, y) = area.min;
        let node = |x: i32, y: i32| vec![(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)];
        let trees: Vec<(i32, i32)> = (0..6).map(|k| (x + 20 + k, y + 3)).collect();
        let rock = BigRock { tile: (x + 2, y + 2), size: 3, variant: 1 };
        world.apply_orders(&[
            Order::PaintTerrain { terrain: Terrain::Forest, tiles: trees.clone() },
            Order::PaintPicture { picture: 3, tiles: trees },
            Order::PaintOre { ore: Some(Ore::Oil), tiles: node(x + 10, y + 10) },
            Order::PaintOre { ore: Some(Ore::Oil), tiles: node(area.max.0, y + 20) },
            Order::PlaceBigRock(rock),
        ]);
        let clip = world.copy_terrain(area);
        assert_eq!(clip.size, (30, 30));
        let to = (x + 31, y - 5);
        world.apply_orders(&[Order::PasteTerrain { clip: Box::new(clip), to, spare: false }]);
        let (dx, dy) = (to.0 - x, to.1 - y);
        assert_eq!(world.map.terrain(x + 22 + dx, y + 3 + dy), Terrain::Forest);
        assert_eq!(world.map.picture(x + 22 + dx, y + 3 + dy), 3, "the pictures stayed behind");
        assert!(node(x + 10 + dx, y + 10 + dy).iter().all(|&(x, y)| world.map.deposit(x, y).is_some_and(|d| d.ore == Ore::Oil)), "the oil node stayed behind");
        assert!(world.map.deposit(area.max.0 + dx, y + 20 + dy).is_none(), "half an oil node came along");
        assert!(world.map.big_rocks().contains(&BigRock { tile: (rock.tile.0 + dx, rock.tile.1 + dy), ..rock }), "the big rock stayed behind");
    }

    /// The landscape changes how the map is drawn, and brings its weather.
    #[test]
    fn a_new_landscape_brings_its_weather() {
        let (mut world, _) = bare(5);
        world.apply_orders(&[Order::SetLook { look: "AL".to_owned() }]);
        assert_eq!((world.look.as_str(), world.weather), ("AL", Weather::Snow));
        world.apply_orders(&[Order::SetLook { look: "DS".to_owned() }]);
        assert_eq!((world.look.as_str(), world.weather), ("DS", Weather::Clear));
    }
}
