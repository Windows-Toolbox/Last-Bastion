//! Spatial grid (spatial hash, DESIGN.md 12.1): remembers which zombies stand in which tile.
//! To find neighbours you only look into the tiles around you instead of checking everyone.
//!
//! Kept flat, like a sorted list: everyone tile after tile in one array, and where each tile's part starts (`start`).
//! Filled anew in two rounds – count per tile, then put each one in its place ("counting sort"). Within a tile they
//! stay in the order they were given (by index) – the same on every PC.
//!
//! What's kept is a `Body` – the index, where it stands, how big it is, its number. The crowd only needs those when it
//! looks at its neighbours, and finds them right there, one after the other, instead of fetching every whole zombie
//! from somewhere in memory. With a hundred thousand zombies that's what makes it fast.

use macroquad::math::Vec2;

/// "Not on the map" in `tile_of`.
const OFF_MAP: u32 = u32::MAX;

/// What the crowd needs of a neighbour: its index in the list, its number, where it stands and how big it is.
#[derive(Clone, Copy)]
pub struct Body {
    pub index: u32,
    pub id: u32,
    pub pos: Vec2,
    pub radius: f32,
}

pub struct SpatialGrid {
    size: usize,
    /// Per tile where its entries start – tile k: `bodies[start[k]..start[k + 1]]`. One more than there are tiles.
    start: Vec<u32>,
    /// Everyone's body, tile after tile.
    bodies: Vec<Body>,
    /// Per index where its body is in `bodies` (`OFF_MAP` if it's off the map) – to move a body (`moved`).
    slot_of: Vec<u32>,
    /// Kept between two fillings so it needs no new memory: per tile where the next entry goes.
    next: Vec<u32>,
}

impl SpatialGrid {
    pub fn new(size: usize) -> SpatialGrid {
        SpatialGrid { size, start: vec![0; size * size + 1], bodies: Vec::new(), slot_of: Vec::new(), next: Vec::new() }
    }

    /// Empties all tiles and enters everyone again: (where, how big, number) – their index is their place in the list.
    /// Gone through twice (`Clone`): first counted per tile, then each one put in its place.
    pub fn rebuild(&mut self, bodies: impl Iterator<Item = (Vec2, f32, u32)> + Clone) {
        // 1. How many per tile (counted one place further on).
        self.start.fill(0);
        let mut count = 0;
        for (pos, _, _) in bodies.clone() {
            if let Some(tile) = self.tile(pos) {
                self.start[tile + 1] += 1;
            }
            count += 1;
        }
        // 2. Add up: now every tile knows where its part starts.
        for k in 1..self.start.len() {
            self.start[k] += self.start[k - 1];
        }
        // 3. Everyone into its place – in the order of the list.
        self.next.clear();
        self.next.extend_from_slice(&self.start[..self.start.len() - 1]);
        self.bodies.clear();
        self.bodies.resize(self.start[self.start.len() - 1] as usize, Body { index: 0, id: 0, pos: Vec2::ZERO, radius: 0.0 });
        self.slot_of.clear();
        self.slot_of.resize(count, OFF_MAP);
        for (i, (pos, radius, id)) in bodies.enumerate() {
            if let Some(tile) = self.tile(pos) {
                let at = self.next[tile];
                self.next[tile] += 1;
                self.bodies[at as usize] = Body { index: i as u32, id, pos, radius };
                self.slot_of[i] = at;
            }
        }
    }

    /// The indices of everyone in tile (x, y) – none outside the map.
    pub fn cell(&self, x: i32, y: i32) -> impl Iterator<Item = usize> + '_ {
        self.bodies(x, y).iter().map(|b| b.index as usize)
    }

    /// The bodies of everyone in tile (x, y) – in the order of the list.
    pub fn bodies(&self, x: i32, y: i32) -> &[Body] {
        if !self.inside(x, y) {
            return &[];
        }
        let k = y as usize * self.size + x as usize;
        &self.bodies[self.start[k] as usize..self.start[k + 1] as usize]
    }

    /// Index `i` has moved a little: its body stands at `pos` now. It stays listed in its old tile until the next
    /// `rebuild` – fine for small steps, the neighbours look into the tiles around anyway.
    pub fn moved(&mut self, i: usize, pos: Vec2) {
        if let Some(&slot) = self.slot_of.get(i).filter(|&&s| s != OFF_MAP) {
            self.bodies[slot as usize].pos = pos;
        }
    }

    /// The tile (its number) a point lies on – `None` off the map.
    fn tile(&self, pos: Vec2) -> Option<usize> {
        let (x, y) = (pos.x.floor() as i32, pos.y.floor() as i32);
        self.inside(x, y).then(|| y as usize * self.size + x as usize)
    }

    fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.size as i32 && y < self.size as i32
    }
}
