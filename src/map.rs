//! The map: a square grid of tiles. Every tile has a terrain (grass, dirt, mountain, forest, water), and
//! buildings can stand on it (they come and go), or a house of a village of doom (`sim/doom.rs`). Everything except
//! bare ground – grass and dirt – blocks the way, and forests also block the view: nobody sees or shoots
//! through them. Farms only grow on grass (`Terrain::is_fertile`).
//!
//! Around the playing field lies a ring outside the **world border**: your units can't go there and in the
//! game view you can't see it – that's where the zombies come from. The editor shows the whole map.
//!
//! Like in TAB the border runs **straight on screen** (DESIGN.md 12.3): the playing field and the ring are rectangles
//! on screen (`Region`), so in the square grid they are diamonds around the middle. The four corners of the grid
//! outside them are off the map – nothing lives there, nobody walks there.

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};


/// Width of the ring outside the world border, in steps of the screen axes (`to_frame` – on screen 32 pixels across,
/// 16 pixels up and down; a straight distance of 24 / √2 ≈ 17 tiles). Room for the spawners at its outer edge and,
/// between them and the border, for track where a train comes in unseen. How big the playing field is, is chosen per
/// map – `mapgen::Common::size`; the map is that much bigger.
pub const BORDER: usize = 24;
/// The playing field is at least this many steps of the screen axes across and down (`Map::set_play_area`).
pub const MIN_PLAY: i32 = 8;

// What a tile is like, as bits of one byte (`Map::flags`): one look into memory answers "can a zombie go here?",
// "can a unit go here?", "can you see through it?". The terrain bits are worked out from `terrain` and kept in step
// with it, buildings set and clear theirs, oil its own (`OIL`). All eight bits are taken – power coverage (DESIGN.md 5.5)
// needs a wider type.
/// Solid terrain – everything but grass and dirt: nobody walks here, and zombies can't tear it down.
const SOLID: u8 = 1 << 0;
/// Forest: nobody sees or shoots through it.
const BLOCKS_SIGHT: u8 = 1 << 1;
/// A building stands here.
const BUILT: u8 = 1 << 2;
/// The door of a gate: open for your units, closed for zombies. Only ever together with `BUILT`.
const DOOR: u8 = 1 << 3;
/// Outside the world border: zombies come from here, your units can't go there.
const OUTSIDE: u8 = 1 << 4;
/// An infected building stands here: it belongs to the zombies now – nothing to tear down, and zombies and units alike
/// walk right through it (its walls are broken). Only ever together with `BUILT`.
const TAKEN: u8 = 1 << 5;
/// A house of a village of doom stands here (`sim/doom.rs`): like a mountain for everyone – nobody walks through it,
/// the zombies don't tear it down, nobody builds on it. Only shooting it down frees the tiles again.
const DOOM: u8 = 1 << 6;
/// An oil pool lies here (a 2 × 2 node): nobody walks into it. Ways go round the whole node (`is_blocked`,
/// `blocks_unit`), the bodies only keep out of a circle in its middle (`out_of_oil`) – its corners are free.
const OIL: u8 = 1 << 7;
/// Off the map altogether: like a mountain outside the border.
const OFF_MAP: u8 = SOLID | OUTSIDE;
/// The oil pool: a circle this many tiles round the middle of its node – a body's middle keeps its own radius further
/// off, but never more than 1 (the edge of the node).
pub const OIL_POOL: f32 = 0.75;

/// What the ground of a tile is. Grass and dirt are open ground – everything else blocks walking (DESIGN.md 9.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Terrain {
    Grass,
    /// Bare ground: walk and build on it like on grass – only no farm grows here (`is_fertile`).
    Dirt,
    Mountain,
    /// Also a single tile on its own – it looks like any forest, there is no separate "tree".
    Forest,
    Water,
}

impl Terrain {
    /// Can nobody walk here – units and zombies alike, and zombies can't tear it down either?
    pub fn is_solid(self) -> bool {
        !matches!(self, Terrain::Grass | Terrain::Dirt)
    }

    /// Does a farm grow here (`kinds::Source::Fertile`)? Only on grass.
    pub fn is_fertile(self) -> bool {
        self == Terrain::Grass
    }

    /// Can nobody see (and so shoot) through it? Forests – over mountains and water you can look.
    pub fn blocks_sight(self) -> bool {
        self == Terrain::Forest
    }

    pub fn name(self) -> &'static str {
        match self {
            Terrain::Grass => "Grass",
            Terrain::Dirt => "Dirt",
            Terrain::Mountain => "Mountain",
            Terrain::Forest => "Forest",
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

/// What can lie in the ground (DESIGN.md 5.1): ore patches like in Factorio. They lie **on open ground** (grass, dirt) –
/// you walk and build on them; a mine on top gets the ore out (DESIGN.md 5). Wood comes from forests.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Ore {
    Iron,
    Coal,
    Stone,
    /// Not a patch but nodes of exactly 2 × 2 tiles, in groups (`mapgen/ores.rs`) – later an oil pump (2 × 2) goes
    /// on top of one. Two nodes never touch, so every node can be told apart.
    Oil,
    /// TAB's gold (`mapgen/tab.rs`): a mine on it digs gold, straight into the colony's gold (`World::gold_rate`).
    Gold,
}

impl Ore {
    /// The ores that lie in patches – oil lies in nodes. Stone first: the start area places it first and closest.
    pub const PATCHES: [Ore; 3] = [Ore::Stone, Ore::Iron, Ore::Coal];

    pub fn name(self) -> &'static str {
        match self {
            Ore::Iron => "Iron ore",
            Ore::Coal => "Coal",
            Ore::Stone => "Stone",
            Ore::Oil => "Oil",
            Ore::Gold => "Gold",
        }
    }
}

/// One tile of an ore patch: which ore and how much of it (units of ore – the middle of a patch has the most).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Deposit {
    pub ore: Ore,
    pub amount: u32,
}

/// World point → the screen axes: `a = x − y` runs to the right on screen, `b = x + y` downwards (`iso.rs` only
/// stretches them into pixels). A rectangle in `a`, `b` is a straight rectangle on screen.
pub fn to_frame(p: Vec2) -> Vec2 {
    vec2(p.x - p.y, p.x + p.y)
}

/// The inverse of `to_frame`.
pub fn from_frame(f: Vec2) -> Vec2 {
    vec2((f.x + f.y) / 2.0, (f.y - f.x) / 2.0)
}

/// A piece of the map that is a straight rectangle on screen – from `min` to `max` in the screen axes (`to_frame`).
/// In the grid it's a diamond: the playing field, the whole map, a spawner.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Region {
    pub min: Vec2,
    pub max: Vec2,
}

impl Region {
    /// `half` steps of the screen axes to every side of the world point `center` – on screen `TILE_W` : `TILE_H`.
    pub fn around(center: Vec2, half: f32) -> Region {
        let c = to_frame(center);
        Region { min: c - Vec2::splat(half), max: c + Vec2::splat(half) }
    }

    pub fn contains(&self, p: Vec2) -> bool {
        let f = to_frame(p);
        f.x > self.min.x && f.y > self.min.y && f.x < self.max.x && f.y < self.max.y
    }

    /// Does the middle of this tile lie inside?
    pub fn contains_tile(&self, x: i32, y: i32) -> bool {
        self.contains(vec2(x as f32 + 0.5, y as f32 + 0.5))
    }

    /// Does the whole disc lie inside? (Its straight distance to a side is `√2` times less than in the screen axes.)
    pub fn contains_disc(&self, center: Vec2, radius: f32) -> bool {
        let r = radius * std::f32::consts::SQRT_2;
        let f = to_frame(center);
        f.x - r >= self.min.x && f.y - r >= self.min.y && f.x + r <= self.max.x && f.y + r <= self.max.y
    }

    /// The nearest point inside, at least `margin` steps of the screen axes away from the edge. A point more than 1
    /// away from it lies on a tile whose middle is inside too (`contains_tile`).
    pub fn clamp(&self, p: Vec2, margin: f32) -> Vec2 {
        from_frame(to_frame(p).clamp(self.min + Vec2::splat(margin), self.max - Vec2::splat(margin)))
    }

    /// The four corners as world points, clockwise as you see them: top left, top right, bottom right, bottom left.
    pub fn corners(&self) -> [Vec2; 4] {
        let (min, max) = (self.min, self.max);
        [min, vec2(max.x, min.y), max, vec2(min.x, max.y)].map(from_frame)
    }

    /// The middle, as a world point.
    pub fn center(&self) -> Vec2 {
        from_frame((self.min + self.max) / 2.0)
    }

    /// How many tiles it covers (one step of both screen axes is half a tile).
    pub fn area(&self) -> f32 {
        let size = self.max - self.min;
        size.x * size.y / 2.0
    }

    /// Which side the point lies closest to (0 top, 1 bottom, 2 left, 3 right – as you see it on screen) – seen from the middle, the larger of the
    /// two screen axes decides.
    pub fn nearest_side(&self, p: Vec2) -> u32 {
        let d = to_frame(p) - (self.min + self.max) / 2.0;
        let half = (self.max - self.min) / 2.0;
        let (a, b) = (d.x / half.x, d.y / half.y);
        if b.abs() >= a.abs() {
            if b < 0.0 { 0 } else { 1 }
        } else if a < 0.0 {
            2
        } else {
            3
        }
    }

    /// Row by row (y), the tiles (x from … to, without the end) whose middle lies inside the region made `grow`
    /// steps bigger to every side. `grow` 1: every tile that shows at least a bit of itself inside.
    pub fn rows(&self, grow: f32) -> impl Iterator<Item = (i32, std::ops::Range<i32>)> {
        let (min, max) = (self.min - Vec2::splat(grow), self.max + Vec2::splat(grow));
        // Middle of tile (x, y) in the screen axes: a = x − y, b = x + y + 1 – strictly between min and max.
        let (top, bottom) = (((min.y - max.x - 1.0) / 2.0).floor() as i32, ((max.y - min.x - 1.0) / 2.0).ceil() as i32);
        (top..=bottom).filter_map(move |y| {
            let yf = y as f32;
            let lo = (min.x + yf).max(min.y - 1.0 - yf);
            let hi = (max.x + yf).min(max.y - 1.0 - yf);
            let (from, to) = (lo.floor() as i32 + 1, hi.ceil() as i32);
            (from < to).then_some((y, from..to))
        })
    }
}

/// One of TAB's big rocks (`BRMountainC2/C3/C4`), put down in the editor: `size` × `size` tiles of mountain from
/// `tile` (its corner with the smallest x and y), drawn as one big picture (`ground.rs`) instead of small pieces.
/// `variant`: which of its pictures (0–2). TAB's generator never makes them, only its hand-made maps.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BigRock {
    pub tile: (i32, i32),
    pub size: i32,
    pub variant: u8,
}

/// The picture groups a tile can ask for (`Map::picture`): A, B, C – 0 is "mixed".
pub const PICTURES: [(u8, &str); 3] = [(1, "A"), (2, "B"), (3, "C")];

/// The sizes of the big rocks (tiles) – TAB's `BRMountainC2`, `C3`, `C4`.
pub const BIG_ROCK_SIZES: [i32; 3] = [3, 4, 5];

impl BigRock {
    /// Does it lie on tile (x, y)?
    pub fn covers(&self, x: i32, y: i32) -> bool {
        (self.tile.0..self.tile.0 + self.size).contains(&x) && (self.tile.1..self.tile.1 + self.size).contains(&y)
    }

    /// All its tiles.
    pub fn tiles(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        (0..self.size).flat_map(move |dy| (0..self.size).map(move |dx| (self.tile.0 + dx, self.tile.1 + dy)))
    }

    /// Do the two share a tile?
    fn overlaps(&self, other: &BigRock) -> bool {
        let apart = |a: i32, a_size: i32, b: i32, b_size: i32| a + a_size <= b || b + b_size <= a;
        !apart(self.tile.0, self.size, other.tile.0, other.size) && !apart(self.tile.1, self.size, other.tile.1, other.size)
    }
}

#[derive(Clone)]
pub struct Map {
    pub size: usize,
    /// Width of the ring outside the world border a new map gets – where its playing field lies until it's moved.
    pub border: usize,
    /// The playing field inside the world border: a straight rectangle on screen, anywhere on the map (`set_play_area`)
    /// – like SC2's map bounds inside its map size. A new map has it in the middle, `border` steps inside the edge.
    play: Region,
    /// One row after the other: tile (x, y) is at index y * size + x.
    terrain: Vec<Terrain>,
    /// Per tile the bits from the top of the file (`SOLID`, `BUILT`, `DOOR` …), in the same order as `terrain`.
    /// `BUILT` and `DOOR` are set and cleared by the simulation.
    flags: Vec<u8>,
    /// Counts every change of the terrain – whoever keeps a picture of the map (the minimap) sees from it
    /// whether the picture is still right.
    pub revision: u32,
    /// Per tile: an ore deposit lying on it (`None`: nothing). Only ever on open ground.
    deposits: Vec<Option<Deposit>>,
    /// The big rocks put down in the editor – their tiles are mountain (`place_big_rock`).
    big_rocks: Vec<BigRock>,
    /// Per tile: which pictures the forest, the rock or the ore on it is drawn with (`PICTURES`: 0 mixed, 1–3 the
    /// landscape's group A, B, C – `ground.rs`). Only the look: the simulation never asks. The editor's painter sets
    /// it; ground that becomes something else is mixed again.
    pictures: Vec<u8>,
}

impl Map {
    /// A map from finished terrain (`mapgen` makes it, or a saved map file): `size` × `size` tiles, row by row.
    pub fn from_terrain(size: usize, border: usize, terrain: Vec<Terrain>) -> Map {
        assert_eq!(terrain.len(), size * size, "terrain doesn't fit the size");
        let play = Map::play_area_of(size, border);
        let mut map = Map {
            size,
            border,
            play,
            terrain,
            flags: vec![0; size * size],
            revision: 0,
            deposits: vec![None; size * size],
            big_rocks: Vec::new(),
            pictures: vec![0; size * size],
        };
        for i in 0..size * size {
            let (x, y) = ((i % size) as i32, (i / size) as i32);
            let outside = if map.in_play(x, y) { 0 } else { OUTSIDE };
            map.flags[i] = map.terrain[i].flags() | outside;
        }
        map
    }

    /// The playing field gets new bounds – `min` (the top left corner on screen) and `size` in whole steps of the
    /// screen axes, like a straight spawner. Always on the map and at least `MIN_PLAY` steps each way. The world border
    /// lies there from now on: outside of it is outside (`OUTSIDE` – zombies come from there, units can't go there).
    /// The ground and whatever stands on it stay as they are.
    pub fn set_play_area(&mut self, min: (i32, i32), size: (i32, i32)) {
        // The whole map in the screen axes: across from −size / 2 to size / 2, down from size / 2 to 3 · size / 2.
        let n = self.size as i32;
        let (left, top) = (-n / 2, n - n / 2);
        let (w, h) = (size.0.clamp(MIN_PLAY.min(n), n), size.1.clamp(MIN_PLAY.min(n), n));
        let min = (min.0.clamp(left, left + n - w), min.1.clamp(top, top + n - h));
        let corner = vec2(min.0 as f32, min.1 as f32);
        self.play = Region { min: corner, max: corner + vec2(w as f32, h as f32) };
        for i in 0..self.size * self.size {
            let (x, y) = ((i % self.size) as i32, (i / self.size) as i32);
            if self.play.contains_tile(x, y) {
                self.flags[i] &= !OUTSIDE;
            } else {
                self.flags[i] |= OUTSIDE;
            }
        }
        self.revision += 1;
    }

    /// The playing field in whole steps of the screen axes: top left corner and size (`set_play_area`).
    pub fn play_steps(&self) -> ((i32, i32), (i32, i32)) {
        let (min, size) = (self.play.min.round(), (self.play.max - self.play.min).round());
        ((min.x as i32, min.y as i32), (size.x as i32, size.y as i32))
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
        self.play
    }

    /// The whole map, including the ring outside the border – the diamond touching the edges of the grid.
    pub fn full_area(&self) -> Region {
        Map::full_area_of(self.size)
    }

    /// Where the playing field of a new map lies (`mapgen`): in the middle, `border` steps inside the edge.
    pub fn play_area_of(size: usize, border: usize) -> Region {
        Region::around(Vec2::splat(size as f32 / 2.0), (size as f32 / 2.0 - border as f32).max(0.0))
    }

    /// `full_area` of a map that doesn't exist yet (`mapgen`).
    pub fn full_area_of(size: usize) -> Region {
        Region::around(Vec2::splat(size as f32 / 2.0), size as f32 / 2.0)
    }

    /// Is this tile on the map – inside `full_area`, not in a corner of the grid sticking out past it?
    pub fn on_map(&self, x: i32, y: i32) -> bool {
        self.full_area().contains_tile(x, y)
    }

    /// Does this tile lie inside the world border?
    pub fn in_play(&self, x: i32, y: i32) -> bool {
        self.play_area().contains_tile(x, y)
    }

    /// Solid terrain, a house of doom, oil, a building (not an infected one), or outside the map? (That's what blocks
    /// zombies – their ways.)
    pub fn is_blocked(&self, x: i32, y: i32) -> bool {
        self.is_blocked_body(x, y) || self.has(x, y, OIL)
    }

    /// `is_blocked` for a zombie's body: oil doesn't count here – it keeps out of the pool's circle (`out_of_oil`).
    pub fn is_blocked_body(&self, x: i32, y: i32) -> bool {
        let flags = self.flags(x, y);
        (flags & (SOLID | DOOM)) != 0 || (flags & (BUILT | TAKEN)) == BUILT
    }

    /// Blocked for your units: like `is_blocked`, plus everything outside the world border – but a gate's door
    /// and an infected building let them through. Not onto solid terrain, though: in the editor a gate can stand on
    /// water. Their ways go round oil too.
    pub fn blocks_unit(&self, x: i32, y: i32) -> bool {
        self.blocks_unit_body(x, y) || self.has(x, y, OIL)
    }

    /// `blocks_unit` for a unit's body: without oil – it keeps out of the pool's circle (`out_of_oil`).
    pub fn blocks_unit_body(&self, x: i32, y: i32) -> bool {
        let flags = self.flags(x, y);
        let closed_building = (flags & BUILT) != 0 && (flags & (DOOR | TAKEN)) == 0;
        (flags & (SOLID | DOOM | OUTSIDE)) != 0 || closed_building
    }

    /// Where a body of `radius` at `pos` has to stand to keep out of an oil pool (`OIL_POOL`): pushed straight out
    /// from the middle of its node – sideways it slides round. `pos` itself if it isn't in one. A pool lies inside its
    /// node, so only a middle on an oil tile can be in it (nodes never touch).
    pub fn out_of_oil(&self, pos: Vec2, radius: f32) -> Vec2 {
        let (x, y) = (pos.x.floor() as i32, pos.y.floor() as i32);
        if !self.has(x, y, OIL) {
            return pos;
        }
        // The node's top left: oil left of this tile, or above it, is the same node.
        let left = if self.has(x - 1, y, OIL) { x - 1 } else { x };
        let top = if self.has(x, y - 1, OIL) { y - 1 } else { y };
        let middle = vec2(left as f32 + 1.0, top as f32 + 1.0);
        let reach = (OIL_POOL + radius).min(1.0);
        let off = pos - middle;
        let d = off.length();
        if d >= reach {
            return pos;
        }
        // Right in the middle (put there in the editor): out to the right.
        let dir = if d > 1e-4 { off / d } else { vec2(1.0, 0.0) };
        middle + dir * reach
    }

    /// Is there a building on this tile?
    pub fn is_built(&self, x: i32, y: i32) -> bool {
        self.has(x, y, BUILT)
    }

    /// Is this tile the door of a gate?
    pub fn is_door(&self, x: i32, y: i32) -> bool {
        self.has(x, y, DOOR)
    }

    /// Can a building go on this tile – open ground, nothing built on it yet, inside the world border?
    /// (Not the same as `blocks_unit`: the door of a gate is open for units, but taken.)
    pub fn is_buildable(&self, x: i32, y: i32) -> bool {
        !self.has(x, y, SOLID | DOOM | BUILT | OUTSIDE)
    }

    /// Does a house of a village of doom stand on this tile?
    pub fn is_doom(&self, x: i32, y: i32) -> bool {
        self.has(x, y, DOOM)
    }

    /// The terrain of a tile. Outside the map counts as mountain (solid).
    pub fn terrain(&self, x: i32, y: i32) -> Terrain {
        self.index(x, y).map_or(Terrain::Mountain, |i| self.terrain[i])
    }

    /// Only solid terrain or a house of doom – not buildings. Zombies can break through buildings, never through this.
    pub fn is_solid(&self, x: i32, y: i32) -> bool {
        self.has(x, y, SOLID | DOOM)
    }

    /// Can zombies tear down what blocks this tile – one of your buildings, not on solid terrain (in the editor a
    /// building can stand on water) and not one they have taken (infected)?
    pub fn can_tear_down(&self, x: i32, y: i32) -> bool {
        let flags = self.flags(x, y);
        (flags & BUILT) != 0 && (flags & (SOLID | TAKEN)) == 0
    }

    /// Forest on this tile – you can't see through it.
    pub fn blocks_sight(&self, x: i32, y: i32) -> bool {
        self.has(x, y, BLOCKS_SIGHT)
    }

    /// Can you see from `from` to `to` – is there no forest **between** them? The tiles of the two
    /// points themselves don't count: you can see the edge of a forest, just not into it.
    pub fn clear_sight(&self, from: Vec2, to: Vec2) -> bool {
        Map::line_is_clear(from, to, |x, y| self.blocks_sight(x, y))
    }

    /// Can your eyes on tile `from` make out tile `to`? TAB's rule (`DXWorldGrid.IsCellVisibleThoughOpaqueFrom`): they
    /// look up to `depth` forest tiles in a row into a forest – but not past it, nor between two forest tiles standing
    /// corner to corner. Only for what you see (`vision.rs`): a shot still needs `clear_sight`.
    ///
    /// A line like Bresenham's: along its longer side it moves one tile every step, along the shorter one when `gathered`
    /// has come together – then that step is diagonal.
    pub fn sees_into_forest(&self, from: (i32, i32), to: (i32, i32), depth: u32) -> bool {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let (sx, sy) = (dx.signum(), dy.signum());
        let (long, short, straight) =
            if dx.abs() > dy.abs() { (dx.abs(), dy.abs(), (sx, 0)) } else { (dy.abs(), dx.abs(), (0, sy)) };
        let (mut x, mut y) = from;
        let mut forest = 0;
        let mut gathered = long / 2;
        for _ in 0..=long {
            if self.index(x, y).is_none() {
                return false;
            }
            if self.blocks_sight(x, y) {
                if forest >= depth {
                    return false;
                }
                forest += 1;
            } else if forest > 0 {
                return false; // out of the forest again: what lies behind it stays hidden
            }
            gathered += short;
            if gathered >= long {
                gathered -= long;
                if sx != 0 && sy != 0 && self.blocks_sight(x + sx, y) && self.blocks_sight(x, y + sy) {
                    return false;
                }
                x += sx;
                y += sy;
            } else {
                x += straight.0;
                y += straight.1;
            }
        }
        true
    }

    /// Is none of the tiles **between** `from` and `to` `blocked`? (The tiles of the two points themselves
    /// don't count.)
    ///
    /// Walks along the line tile by tile, exactly through the tiles the line touches (a "grid walk" like in
    /// old ray casting games): every step goes into the neighbour tile whose border the line crosses next –
    /// never diagonally. So a line can't slip through the corner between two diagonal wall pieces either.
    /// Needs nothing of the map itself (`blocked` says what blocks) – so a flow field can use it with what it kept.
    pub fn line_is_clear(from: Vec2, to: Vec2, blocked: impl Fn(i32, i32) -> bool) -> bool {
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

    /// The ore deposit on a tile, if there is one.
    pub fn deposit(&self, x: i32, y: i32) -> Option<Deposit> {
        self.index(x, y).and_then(|i| self.deposits[i])
    }

    /// Puts ore on a tile – or takes it away (`None`). Only on open ground (grass, dirt): on anything else nothing
    /// happens. Oil taken away takes its whole node with it. Returns whether anything changed.
    pub fn set_deposit(&mut self, x: i32, y: i32, deposit: Option<Deposit>) -> bool {
        match self.index(x, y) {
            Some(i) if !self.terrain[i].is_solid() && self.deposits[i] != deposit => {
                if deposit.is_none_or(|d| d.ore != Ore::Oil) {
                    self.clear_oil_node(i);
                }
                self.deposits[i] = deposit;
                if deposit.is_some_and(|d| d.ore == Ore::Oil) {
                    self.flags[i] |= OIL;
                }
                if deposit.is_none() {
                    self.pictures[i] = 0;
                }
                self.revision += 1;
                true
            }
            _ => false,
        }
    }

    /// Oil only makes sense as a whole 2 × 2 node: if tile `i` has oil, all the oil connected to it goes (half a node
    /// would be drawn as a broken pool). Nothing happens on any other tile.
    fn clear_oil_node(&mut self, i: usize) {
        let mut todo = vec![i];
        while let Some(i) = todo.pop() {
            if !self.deposits[i].is_some_and(|d| d.ore == Ore::Oil) {
                continue;
            }
            self.deposits[i] = None;
            self.flags[i] &= !OIL;
            let (x, y) = ((i % self.size) as i32, (i / self.size) as i32);
            todo.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)].into_iter().filter_map(|(x, y)| self.index(x, y)));
        }
    }

    /// Every tile with ore on it: (x, y, deposit) – for saving.
    pub fn all_deposits(&self) -> impl Iterator<Item = (i32, i32, Deposit)> + '_ {
        self.deposits.iter().enumerate().filter_map(|(i, d)| d.map(|d| ((i % self.size) as i32, (i / self.size) as i32, d)))
    }

    /// Changes the terrain of one tile (editor). Tiles with a building or a house of doom stay as they are. Solid terrain
    /// takes the ore away with it. Returns whether anything changed.
    pub fn set_terrain(&mut self, x: i32, y: i32, terrain: Terrain) -> bool {
        match self.index(x, y) {
            Some(i) if (self.flags[i] & (BUILT | DOOM)) == 0 && self.terrain[i] != terrain => {
                // Other ground painted over a big rock: the rock goes – its other tiles stay mountain (small pieces then).
                if terrain != Terrain::Mountain {
                    self.big_rocks.retain(|r| !r.covers(x, y));
                }
                self.terrain[i] = terrain;
                // Whatever stood here had its picture – the new ground starts mixed (ore on open ground keeps its own).
                if terrain.is_solid() || self.deposits[i].is_none() {
                    self.pictures[i] = 0;
                }
                if terrain.is_solid() {
                    self.clear_oil_node(i);
                    self.deposits[i] = None;
                }
                // Swap the terrain bits, keep the others (outside the border stays outside).
                self.flags[i] = (self.flags[i] & !(SOLID | BLOCKS_SIGHT)) | terrain.flags();
                self.revision += 1;
                true
            }
            _ => false,
        }
    }

    /// Which pictures the forest, rock or ore on a tile is drawn with: 0 mixed, 1–3 the group A, B, C (`PICTURES`).
    pub fn picture(&self, x: i32, y: i32) -> u8 {
        self.index(x, y).map_or(0, |i| self.pictures[i])
    }

    /// Sets it (editor) – only where something stands that has pictures: forest, mountain or ore. Anything above C
    /// counts as mixed. Returns whether anything changed.
    pub fn set_picture(&mut self, x: i32, y: i32, picture: u8) -> bool {
        let picture = if PICTURES.iter().any(|&(p, _)| p == picture) { picture } else { 0 };
        match self.index(x, y) {
            Some(i) if self.pictures[i] != picture && (matches!(self.terrain[i], Terrain::Forest | Terrain::Mountain) || self.deposits[i].is_some()) => {
                self.pictures[i] = picture;
                self.revision += 1;
                true
            }
            _ => false,
        }
    }

    /// Every tile that isn't mixed: (x, y, picture) – for saving.
    pub fn all_pictures(&self) -> impl Iterator<Item = (i32, i32, u8)> + '_ {
        self.pictures.iter().enumerate().filter(|&(_, &p)| p != 0).map(|(i, &p)| ((i % self.size) as i32, (i / self.size) as i32, p))
    }

    /// The big rocks on the map.
    pub fn big_rocks(&self) -> &[BigRock] {
        &self.big_rocks
    }

    /// Puts a big rock down: all its tiles become mountain, a big rock it overlaps goes. Not if a tile of it is off the
    /// map, built on or a house of doom – `false` then, nothing changes.
    pub fn place_big_rock(&mut self, rock: BigRock) -> bool {
        let free = |(x, y): (i32, i32)| self.index(x, y).is_some_and(|i| self.flags[i] & (BUILT | DOOM) == 0);
        if !BIG_ROCK_SIZES.contains(&rock.size) || !rock.tiles().all(free) {
            return false;
        }
        self.big_rocks.retain(|r| !r.overlaps(&rock));
        for (x, y) in rock.tiles() {
            self.set_terrain(x, y, Terrain::Mountain);
        }
        self.big_rocks.push(rock);
        // Even if every tile was mountain already: the picture changes (`ground.rs` looks at the revision).
        self.revision += 1;
        true
    }

    /// Marks a rectangle of tiles (width, depth) as built on – or free again, then a door there is gone too.
    pub fn set_occupied(&mut self, x0: i32, y0: i32, size: (i32, i32), value: bool) {
        for y in y0..y0 + size.1 {
            for x in x0..x0 + size.0 {
                if let Some(i) = self.index(x, y) {
                    if value {
                        self.flags[i] |= BUILT;
                    } else {
                        self.flags[i] &= !(BUILT | DOOR | TAKEN);
                    }
                }
            }
        }
    }

    /// Marks the built-on tiles of a rectangle as taken by the zombies (an infected building, `TAKEN`) – or, `false`,
    /// as yours again (repaired: cured).
    pub fn set_taken(&mut self, x0: i32, y0: i32, size: (i32, i32), taken: bool) {
        for y in y0..y0 + size.1 {
            for x in x0..x0 + size.0 {
                if let Some(i) = self.index(x, y) {
                    if (self.flags[i] & BUILT) != 0 {
                        if taken {
                            self.flags[i] |= TAKEN;
                        } else {
                            self.flags[i] &= !TAKEN;
                        }
                    }
                }
            }
        }
    }

    /// Marks a rectangle of tiles as a house of a village of doom – or, `false`, as open again (shot down).
    pub fn set_doom(&mut self, x0: i32, y0: i32, size: (i32, i32), doom: bool) {
        for y in y0..y0 + size.1 {
            for x in x0..x0 + size.0 {
                if let Some(i) = self.index(x, y) {
                    if doom {
                        self.flags[i] |= DOOM;
                    } else {
                        self.flags[i] &= !DOOM;
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

    /// Index into the lists – `None` off the map: outside the grid, or in one of its corners beyond the ring
    /// (`full_area`).
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        // `full_area().contains_tile` without the floats: the middle of the tile is less than size / 2 steps of the
        // screen axes from the middle of the map – |2x + 1 − size| + |2y + 1 − size| < size, in halves.
        let n = self.size as i32;
        let inside = x >= 0 && y >= 0 && x < n && y < n && (2 * x + 1 - n).abs() + (2 * y + 1 - n).abs() < n;
        inside.then(|| y as usize * self.size + x as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A map of TAB's generator (the default) has water, mountains and forests.
    #[test]
    fn default_map_has_everything() {
        use crate::mapgen::{self, MapSetup};
        for seed in 1..=5 {
            let map = mapgen::generate(seed, &MapSetup::default()).map;
            let tiles = || (0..map.size as i32).flat_map(|y| (0..map.size as i32).map(move |x| (x, y)));
            let count = |t: Terrain| tiles().filter(|&(x, y)| map.terrain(x, y) == t).count();
            for (t, at_least) in [(Terrain::Water, 1000), (Terrain::Mountain, 1000), (Terrain::Forest, 2000)] {
                assert!(count(t) > at_least, "seed {seed}: only {} {}", count(t), t.name());
            }
        }
    }

    /// A forest tile between two points blocks the view – one at either end doesn't (you see the edge of the forest).
    #[test]
    fn forest_blocks_the_view_only_in_between() {
        let mut map = Map::blank(160, BORDER);
        let c = map.size as i32 / 2;
        let p = |x: i32, y: i32| vec2(x as f32 + 0.5, y as f32 + 0.5);
        assert!(map.clear_sight(p(c, c), p(c + 6, c + 3)));
        map.set_terrain(c + 3, c, Terrain::Forest);
        assert!(!map.clear_sight(p(c, c), p(c + 6, c)), "straight through the forest");
        assert!(!map.clear_sight(p(c + 6, c), p(c, c)), "the same the other way round");
        assert!(map.clear_sight(p(c, c), p(c + 3, c)), "the forest tile itself is visible");
        assert!(map.clear_sight(p(c, c), p(c + 6, c + 4)), "past the forest tile");
        // Mountains don't block the view.
        map.set_terrain(c, c + 3, Terrain::Mountain);
        assert!(map.clear_sight(p(c, c), p(c, c + 6)));
    }

    /// Your eyes reach three forest tiles deep, like TAB's – not further, and not past the forest.
    #[test]
    fn eyes_look_into_a_forest_but_not_through_it() {
        let mut map = Map::blank(160, BORDER);
        let c = map.size as i32 / 2;
        // A wall of forest: three wide across the line, six deep.
        for x in c + 3..c + 9 {
            for y in c - 1..=c + 1 {
                map.set_terrain(x, y, Terrain::Forest);
            }
        }
        let from = (c, c);
        assert!(map.sees_into_forest(from, (c + 2, c), 3), "open ground");
        for deep in 3..6 {
            assert!(map.sees_into_forest(from, (c + deep, c), 3), "{deep} tiles away, inside the forest");
        }
        assert!(!map.sees_into_forest(from, (c + 6, c), 3), "the fourth forest tile");
        assert!(!map.sees_into_forest(from, (c + 10, c), 3), "behind the forest");
        // One deep is just the edge – what `clear_sight` calls visible.
        assert!(map.sees_into_forest(from, (c + 3, c), 1));
        assert!(!map.sees_into_forest(from, (c + 4, c), 1));
        // A single tree: you see it, but not what's behind it – like TAB's.
        let mut map = Map::blank(160, BORDER);
        map.set_terrain(c + 3, c, Terrain::Forest);
        assert!(map.sees_into_forest(from, (c + 3, c), 3));
        assert!(!map.sees_into_forest(from, (c + 4, c), 3));
        // Not between two trees standing corner to corner.
        let mut map = Map::blank(160, BORDER);
        map.set_terrain(c + 1, c, Terrain::Forest);
        map.set_terrain(c, c + 1, Terrain::Forest);
        assert!(!map.sees_into_forest(from, (c + 1, c + 1), 3));
        // Looking at your own tile always works.
        assert!(map.sees_into_forest(from, from, 3));
    }

    /// The geometry tests below: a 64 map with a ring of 16 steps – a playing field 32 steps across.
    const RING: usize = 16;

    /// The bits of a tile follow terrain and buildings: who may walk, build and look where.
    #[test]
    fn tile_bits_follow_terrain_and_buildings() {
        let mut map = Map::blank(64, RING);
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

        // Built on and taken by the zombies (infected): open for everyone, still built on, nothing to tear down. Cured:
        // closed again. Torn down: gone.
        map.set_occupied(c, c, (1, 1), true);
        assert!(map.can_tear_down(c, c));
        map.set_taken(c, c, (1, 1), true);
        assert!(!map.is_blocked(c, c) && !map.blocks_unit(c, c) && !map.can_tear_down(c, c) && !map.is_buildable(c, c));
        map.set_taken(c, c, (1, 1), false);
        assert!(map.is_blocked(c, c) && map.blocks_unit(c, c) && map.can_tear_down(c, c));
        map.set_taken(c, c, (1, 1), true);
        map.set_occupied(c, c, (1, 1), false);
        map.set_occupied(c, c, (1, 1), true);
        assert!(map.can_tear_down(c, c));
        map.set_occupied(c, c, (1, 1), false);

        // Outside the world border: zombies walk there, units don't, nobody builds. Off the map – outside the grid or
        // in a corner of it beyond the ring: blocked for all.
        let (x, y) = (c, c - map.size as i32 / 2 + RING as i32 / 2);
        assert!(!map.in_play(x, y) && !map.is_blocked(x, y) && map.blocks_unit(x, y) && !map.is_buildable(x, y));
        for (x, y) in [(-1, 0), (0, 0), (RING as i32 - 1, RING as i32 - 1)] {
            assert!(map.is_blocked(x, y) && map.blocks_unit(x, y) && map.is_solid(x, y) && !map.blocks_sight(x, y));
        }
    }

    /// The border runs straight on screen: the playing field is a diamond in the grid, as wide as high, and
    /// `Region::rows` walks exactly over the tiles `contains_tile` says are inside.
    #[test]
    fn the_playing_field_is_a_diamond_in_the_grid() {
        let map = Map::blank(64, RING);
        let play = map.play_area();
        let c = 32;
        // Half the width of the diamond: 16 steps of the screen axes – the middle of a tile less than that from the
        // middle of the map (|dx| + |dy| < 16).
        assert!(map.in_play(c + 14, c) && !map.in_play(c + 15, c) && map.in_play(c, c - 15) && !map.in_play(c, c - 16));
        assert!(!map.in_play(c + 8, c + 8), "a corner of the old square");
        let mut counted = 0;
        for (y, xs) in play.rows(0.0) {
            for x in xs {
                assert!(play.contains_tile(x, y), "({x}, {y})");
                counted += 1;
            }
        }
        let inside = (0..64).flat_map(|y| (0..64).map(move |x| (x, y))).filter(|&(x, y)| map.in_play(x, y)).count();
        assert_eq!(counted, inside);
        assert!((inside as f32 - play.area()).abs() < 40.0, "{inside} tiles, area {}", play.area());
        // Clamped with a margin above 1, a point lands on a tile inside.
        let p = play.clamp(vec2(0.3, 50.7), 1.05);
        assert!(map.in_play(p.x.floor() as i32, p.y.floor() as i32));
    }

    /// Oil goes as a whole node: erasing one tile, painting other ore or terrain on one takes all four – a node
    /// a tile away stays.
    #[test]
    fn oil_goes_as_a_whole_node() {
        let mut map = Map::blank(64, BORDER);
        let c = map.size as i32 / 2;
        let oil = Some(Deposit { ore: Ore::Oil, amount: 100 });
        let stamp = |map: &mut Map, x: i32, y: i32| {
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                map.set_deposit(x + dx, y + dy, oil);
            }
        };
        let oil_left = |map: &Map| map.all_deposits().filter(|(_, _, d)| d.ore == Ore::Oil).count();
        stamp(&mut map, c, c);
        stamp(&mut map, c + 3, c);
        map.set_deposit(c + 1, c + 1, None);
        assert_eq!(oil_left(&map), 4, "erased");
        stamp(&mut map, c, c);
        map.set_deposit(c, c + 1, Some(Deposit { ore: Ore::Coal, amount: 100 }));
        assert_eq!(oil_left(&map), 4, "coal painted over it");
        assert_eq!(map.deposit(c, c + 1).map(|d| d.ore), Some(Ore::Coal));
        map.set_terrain(c + 4, c, Terrain::Water);
        assert_eq!(oil_left(&map), 0, "water painted over it");
    }

    /// A big rock makes its tiles mountain; one it overlaps goes; other ground painted over it takes it away – and it
    /// never goes on a building or off the map.
    #[test]
    fn big_rocks_are_mountain_and_can_be_painted_away() {
        let mut map = Map::blank(160, BORDER);
        let c = 80;
        let rock = BigRock { tile: (c, c), size: 4, variant: 1 };
        assert!(map.place_big_rock(rock));
        assert!(rock.tiles().all(|(x, y)| map.terrain(x, y) == Terrain::Mountain));
        assert_eq!(rock.tiles().count(), 16);
        assert_eq!(map.big_rocks(), &[rock]);
        // Overlapping: the new one wins, the old one's tiles stay mountain.
        let other = BigRock { tile: (c + 2, c + 2), size: 3, variant: 0 };
        assert!(map.place_big_rock(other));
        assert_eq!(map.big_rocks(), &[other]);
        assert_eq!(map.terrain(c, c), Terrain::Mountain);
        // Grass on one of its tiles: gone.
        map.set_terrain(c + 3, c + 3, Terrain::Grass);
        assert!(map.big_rocks().is_empty());
        // Not on a building, not off the map, not in a size TAB doesn't have.
        map.set_occupied(c + 20, c, (2, 2), true);
        assert!(!map.place_big_rock(BigRock { tile: (c + 19, c - 1), size: 3, variant: 0 }));
        assert!(!map.place_big_rock(BigRock { tile: (-2, -2), size: 3, variant: 0 }));
        assert!(!map.place_big_rock(BigRock { tile: (c - 20, c), size: 6, variant: 0 }));
        assert_eq!(map.terrain(c + 19, c - 1), Terrain::Grass, "a refused rock changed the ground");
        assert!(map.big_rocks().is_empty());
    }
}
