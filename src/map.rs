//! The map: a square grid of tiles. Every tile has a terrain (grass, mountain, forest, tree, water), and buildings
//! can stand on it (they come and go). Everything except bare grass blocks the way – and forests and trees also
//! block the view: nobody sees or shoots through them.
//!
//! Around the playing field lies a ring outside the **world border**: your units can't go there and in the
//! game view you can't see it – that's where the zombies come from. The editor shows the whole map.

use macroquad::math::{vec2, Vec2};


/// Width of the ring outside the world border, in tiles. (How big the playing field is, is chosen per map –
/// `mapgen::Common::size`.)
pub const BORDER: usize = 16;

// What a tile is like, as bits of one byte (`Map::flags`): one look into memory answers "can a zombie go here?",
// "can a unit go here?", "can you see through it?". The terrain bits are worked out from `terrain` and kept in step
// with it, buildings set and clear theirs. Three bits are still free (power coverage, DESIGN.md 5.5, will be one).
/// Solid terrain – everything but grass: nobody walks here, and zombies can't tear it down.
const SOLID: u8 = 1 << 0;
/// Forest or tree: nobody sees or shoots through it.
const BLOCKS_SIGHT: u8 = 1 << 1;
/// A building stands here.
const BUILT: u8 = 1 << 2;
/// The door of a gate: open for your units, closed for zombies. Only ever together with `BUILT`.
const DOOR: u8 = 1 << 3;
/// Outside the world border: zombies come from here, your units can't go there.
const OUTSIDE: u8 = 1 << 4;
/// Off the map altogether: like a mountain outside the border.
const OFF_MAP: u8 = SOLID | OUTSIDE;

/// What the ground of a tile is. For now everything but grass simply blocks walking – later forests give wood and
/// water may be shot across (DESIGN.md 9.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Terrain {
    Grass,
    Mountain,
    Forest,
    /// A single tree on grass: blocks like a forest tile, only drawn as one tree.
    Tree,
    Water,
}

impl Terrain {
    /// Can nobody walk here – units and zombies alike, and zombies can't tear it down either?
    pub fn is_solid(self) -> bool {
        self != Terrain::Grass
    }

    /// Can nobody see (and so shoot) through it? Forests and trees – over mountains and water you can look.
    pub fn blocks_sight(self) -> bool {
        matches!(self, Terrain::Forest | Terrain::Tree)
    }

    pub fn name(self) -> &'static str {
        match self {
            Terrain::Grass => "Grass",
            Terrain::Mountain => "Mountain",
            Terrain::Forest => "Forest",
            Terrain::Tree => "Tree",
            Terrain::Water => "Water",
        }
    }

    /// The bits of `Map::flags` that come from the terrain.
    fn flags(self) -> u8 {
        let solid = if self.is_solid() { SOLID } else { 0 };
        let sight = if self.blocks_sight() { BLOCKS_SIGHT } else { 0 };
        solid | sight
    }
}

/// A square piece of the map in world coordinates: from `min`, `size` tiles wide and high.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Region {
    pub min: Vec2,
    pub size: f32,
}

impl Region {
    pub fn max(&self) -> Vec2 {
        self.min + Vec2::splat(self.size)
    }

    pub fn contains(&self, p: Vec2) -> bool {
        let max = self.max();
        p.x >= self.min.x && p.y >= self.min.y && p.x < max.x && p.y < max.y
    }

    /// The nearest point inside, at least `margin` away from the edge.
    pub fn clamp(&self, p: Vec2, margin: f32) -> Vec2 {
        p.clamp(self.min + Vec2::splat(margin), self.max() - Vec2::splat(margin))
    }
}

pub struct Map {
    pub size: usize,
    /// Width of the ring outside the world border.
    pub border: usize,
    /// One row after the other: tile (x, y) is at index y * size + x.
    terrain: Vec<Terrain>,
    /// Per tile the bits from the top of the file (`SOLID`, `BUILT`, `DOOR` …), in the same order as `terrain`.
    /// `BUILT` and `DOOR` are set and cleared by the simulation.
    flags: Vec<u8>,
    /// Counts every change of the terrain – whoever keeps a picture of the map (the minimap) sees from it
    /// whether the picture is still right.
    pub revision: u32,
}

impl Map {
    /// A map from finished terrain (`mapgen` makes it, or a saved map file): `size` × `size` tiles, row by row.
    pub fn from_terrain(size: usize, border: usize, terrain: Vec<Terrain>) -> Map {
        assert_eq!(terrain.len(), size * size, "terrain doesn't fit the size");
        let mut map = Map { size, border, terrain, flags: vec![0; size * size], revision: 0 };
        for i in 0..size * size {
            let (x, y) = ((i % size) as i32, (i / size) as i32);
            let outside = if map.in_play(x, y) { 0 } else { OUTSIDE };
            map.flags[i] = map.terrain[i].flags() | outside;
        }
        map
    }

    /// Only grass – for tests.
    #[cfg(test)]
    pub fn blank(size: usize, border: usize) -> Map {
        Map::from_terrain(size, border, vec![Terrain::Grass; size * size])
    }

    /// Exactly the same terrain on every tile? (Buildings don't count.)
    #[cfg(test)]
    pub fn same_terrain(&self, other: &Map) -> bool {
        self.size == other.size && self.terrain == other.terrain
    }

    /// The playing field inside the world border.
    pub fn play_area(&self) -> Region {
        Region { min: Vec2::splat(self.border as f32), size: (self.size - 2 * self.border) as f32 }
    }

    /// The whole map, including the ring outside the border.
    pub fn full_area(&self) -> Region {
        Region { min: vec2(0.0, 0.0), size: self.size as f32 }
    }

    /// Does this tile lie inside the world border?
    pub fn in_play(&self, x: i32, y: i32) -> bool {
        let (lo, hi) = (self.border as i32, (self.size - self.border) as i32);
        x >= lo && y >= lo && x < hi && y < hi
    }

    /// Solid terrain, a building, or outside the map? (That's what blocks zombies.)
    pub fn is_blocked(&self, x: i32, y: i32) -> bool {
        self.has(x, y, SOLID | BUILT)
    }

    /// Blocked for your units: like `is_blocked`, plus everything outside the world border – but a gate's door
    /// lets them through.
    pub fn blocks_unit(&self, x: i32, y: i32) -> bool {
        let flags = self.flags(x, y);
        let blocked = (flags & (SOLID | BUILT)) != 0 && (flags & DOOR) == 0;
        blocked || (flags & OUTSIDE) != 0
    }

    /// Is there a building on this tile?
    pub fn is_built(&self, x: i32, y: i32) -> bool {
        self.has(x, y, BUILT)
    }

    /// Is this tile the door of a gate?
    pub fn is_door(&self, x: i32, y: i32) -> bool {
        self.has(x, y, DOOR)
    }

    /// Can a building go on this tile – grass, nothing built on it yet, inside the world border?
    /// (Not the same as `blocks_unit`: the door of a gate is open for units, but taken.)
    pub fn is_buildable(&self, x: i32, y: i32) -> bool {
        !self.has(x, y, SOLID | BUILT | OUTSIDE)
    }

    /// The terrain of a tile. Outside the map counts as mountain (solid).
    pub fn terrain(&self, x: i32, y: i32) -> Terrain {
        self.index(x, y).map_or(Terrain::Mountain, |i| self.terrain[i])
    }

    /// Only solid terrain – not buildings. Zombies can break through buildings, never through this.
    pub fn is_solid(&self, x: i32, y: i32) -> bool {
        self.has(x, y, SOLID)
    }

    /// Forest or tree on this tile – you can't see through it.
    pub fn blocks_sight(&self, x: i32, y: i32) -> bool {
        self.has(x, y, BLOCKS_SIGHT)
    }

    /// Can you see from `from` to `to` – is there no forest or tree **between** them? The tiles of the two
    /// points themselves don't count: you can see the edge of a forest, just not into it.
    pub fn clear_sight(&self, from: Vec2, to: Vec2) -> bool {
        self.line_is_clear(from, to, |x, y| self.blocks_sight(x, y))
    }

    /// Is none of the tiles **between** `from` and `to` `blocked`? (The tiles of the two points themselves
    /// don't count.)
    ///
    /// Walks along the line tile by tile, exactly through the tiles the line touches (a "grid walk" like in
    /// old ray casting games): every step goes into the neighbour tile whose border the line crosses next –
    /// never diagonally. So a line can't slip through the corner between two diagonal wall pieces either.
    pub fn line_is_clear(&self, from: Vec2, to: Vec2, blocked: impl Fn(i32, i32) -> bool) -> bool {
        let (mut x, mut y) = (from.x.floor() as i32, from.y.floor() as i32);
        let (end_x, end_y) = (to.x.floor() as i32, to.y.floor() as i32);
        let d = to - from;
        let (step_x, step_y) = (if d.x > 0.0 { 1 } else { -1 }, if d.y > 0.0 { 1 } else { -1 });
        // How far along the line (0 = from, 1 = to) one whole tile in x (or y) is …
        let delta_x = if d.x != 0.0 { 1.0 / d.x.abs() } else { f32::INFINITY };
        let delta_y = if d.y != 0.0 { 1.0 / d.y.abs() } else { f32::INFINITY };
        // … and where along the line it crosses the next tile border in x (or y).
        let mut next_x = delta_x * if d.x > 0.0 { x as f32 + 1.0 - from.x } else { from.x - x as f32 };
        let mut next_y = delta_y * if d.y > 0.0 { y as f32 + 1.0 - from.y } else { from.y - y as f32 };
        // Exactly this many steps lead from the start tile to the end tile.
        let steps = (end_x - x).abs() + (end_y - y).abs();
        for k in 1..=steps {
            // Whichever border comes first – but never past the end tile (rounding errors at the very end).
            let go_x = if x == end_x {
                false
            } else if y == end_y {
                true
            } else {
                next_x < next_y
            };
            if go_x {
                x += step_x;
                next_x += delta_x;
            } else {
                y += step_y;
                next_y += delta_y;
            }
            if k < steps && blocked(x, y) {
                return false;
            }
        }
        true
    }

    /// Changes the terrain of one tile (editor). Tiles with a building stay as they are.
    /// Returns whether anything changed.
    pub fn set_terrain(&mut self, x: i32, y: i32, terrain: Terrain) -> bool {
        match self.index(x, y) {
            Some(i) if (self.flags[i] & BUILT) == 0 && self.terrain[i] != terrain => {
                self.terrain[i] = terrain;
                // Swap the terrain bits, keep the others (outside the border stays outside).
                self.flags[i] = (self.flags[i] & !(SOLID | BLOCKS_SIGHT)) | terrain.flags();
                self.revision += 1;
                true
            }
            _ => false,
        }
    }

    /// Marks a rectangle of tiles (width, depth) as built on – or free again, then a door there is gone too.
    pub fn set_occupied(&mut self, x0: i32, y0: i32, size: (i32, i32), value: bool) {
        for y in y0..y0 + size.1 {
            for x in x0..x0 + size.0 {
                if let Some(i) = self.index(x, y) {
                    if value {
                        self.flags[i] |= BUILT;
                    } else {
                        self.flags[i] &= !(BUILT | DOOR);
                    }
                }
            }
        }
    }

    /// Marks a built-on tile as the door of a gate: open for your units, closed for zombies.
    pub fn set_door(&mut self, x: i32, y: i32) {
        if let Some(i) = self.index(x, y) {
            if (self.flags[i] & BUILT) != 0 {
                self.flags[i] |= DOOR;
            }
        }
    }

    /// The bits of a tile – off the map `OFF_MAP`.
    fn flags(&self, x: i32, y: i32) -> u8 {
        self.index(x, y).map_or(OFF_MAP, |i| self.flags[i])
    }

    /// Does the tile have at least one of these bits?
    fn has(&self, x: i32, y: i32, bits: u8) -> bool {
        (self.flags(x, y) & bits) != 0
    }

    /// Index into the lists – `None` outside the map.
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let inside = x >= 0 && y >= 0 && x < self.size as i32 && y < self.size as i32;
        inside.then(|| y as usize * self.size + x as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A map made with the default values of "Wild" has water, mountains, forests and single trees.
    #[test]
    fn default_map_has_everything() {
        use crate::mapgen::{self, MapSetup};
        for seed in 1..=5 {
            let map = mapgen::generate(seed, &MapSetup::default()).map;
            let count = |t: Terrain| (0..map.size as i32).flat_map(|y| (0..map.size as i32).map(move |x| (x, y)))
                .filter(|&(x, y)| map.terrain(x, y) == t)
                .count();
            for (t, at_least) in [(Terrain::Water, 1000), (Terrain::Mountain, 1000), (Terrain::Forest, 2000), (Terrain::Tree, 300)] {
                assert!(count(t) > at_least, "seed {seed}: only {} {}", count(t), t.name());
            }
        }
    }

    /// A tree between two points blocks the view – a tree at either end doesn't (you see the edge of the forest).
    #[test]
    fn trees_block_the_view_only_in_between() {
        let mut map = Map::blank(160, BORDER);
        let c = map.size as i32 / 2;
        let p = |x: i32, y: i32| vec2(x as f32 + 0.5, y as f32 + 0.5);
        assert!(map.clear_sight(p(c, c), p(c + 6, c + 3)));
        map.set_terrain(c + 3, c, Terrain::Tree);
        assert!(!map.clear_sight(p(c, c), p(c + 6, c)), "straight through the tree");
        assert!(!map.clear_sight(p(c + 6, c), p(c, c)), "the same the other way round");
        assert!(map.clear_sight(p(c, c), p(c + 3, c)), "the tree itself is visible");
        assert!(map.clear_sight(p(c, c), p(c + 6, c + 4)), "past the tree");
        // Mountains don't block the view.
        map.set_terrain(c, c + 3, Terrain::Mountain);
        assert!(map.clear_sight(p(c, c), p(c, c + 6)));
    }

    /// The bits of a tile follow terrain and buildings: who may walk, build and look where.
    #[test]
    fn tile_bits_follow_terrain_and_buildings() {
        let mut map = Map::blank(64, BORDER);
        let c = map.size as i32 / 2;
        assert!(map.is_buildable(c, c) && !map.blocks_unit(c, c));

        // Forest: blocks everyone and the view. Back to grass: everything open again.
        map.set_terrain(c, c, Terrain::Forest);
        assert!(map.is_blocked(c, c) && map.is_solid(c, c) && map.blocks_sight(c, c) && !map.is_buildable(c, c));
        map.set_terrain(c, c, Terrain::Grass);
        assert!(!map.is_blocked(c, c) && !map.blocks_sight(c, c) && map.is_buildable(c, c));

        // A gate (3 × 1) with its door in the middle: zombies blocked everywhere, units only beside the door.
        map.set_occupied(c - 1, c, (3, 1), true);
        map.set_door(c, c);
        assert!(map.is_blocked(c, c) && !map.blocks_unit(c, c) && map.is_door(c, c));
        assert!(map.blocks_unit(c - 1, c) && !map.is_door(c - 1, c));
        assert!(!map.is_buildable(c, c) && !map.is_solid(c, c));
        // Built on: painting doesn't change it.
        assert!(!map.set_terrain(c, c, Terrain::Water));
        // Torn down: the door is gone with it, and a door only goes on a building.
        map.set_occupied(c - 1, c, (3, 1), false);
        assert!(!map.is_door(c, c) && map.is_buildable(c, c));
        map.set_door(c, c);
        assert!(!map.is_door(c, c));

        // Outside the world border: zombies walk there, units don't, nobody builds. Off the map: blocked for all.
        let out = BORDER as i32 - 1;
        assert!(!map.is_blocked(out, out) && map.blocks_unit(out, out) && !map.is_buildable(out, out));
        assert!(map.is_blocked(-1, 0) && map.blocks_unit(-1, 0) && map.is_solid(-1, 0) && !map.blocks_sight(-1, 0));
    }
}

