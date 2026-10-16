//! Spatial grid (spatial hash, DESIGN.md 12.1): remembers which zombies stand in which tile.
//! To find neighbours you only look into the tiles around you instead of checking everyone.

use macroquad::math::Vec2;

pub struct SpatialGrid {
    size: usize,
    /// Per tile the indices of the zombies currently standing in it.
    cells: Vec<Vec<u32>>,
}

impl SpatialGrid {
    pub fn new(size: usize) -> SpatialGrid {
        SpatialGrid {
            size,
            cells: vec![Vec::new(); size * size],
        }
    }

    /// Empties all tiles and enters the positions again (once per tick).
    pub fn rebuild(&mut self, positions: impl Iterator<Item = Vec2>) {
        for cell in &mut self.cells {
            cell.clear();
        }
        for (i, pos) in positions.enumerate() {
            let (x, y) = (pos.x.floor() as i32, pos.y.floor() as i32);
            if self.inside(x, y) {
                self.cells[y as usize * self.size + x as usize].push(i as u32);
            }
        }
    }

    /// The zombies in tile (x, y) – an empty list outside the map.
    pub fn cell(&self, x: i32, y: i32) -> &[u32] {
        if !self.inside(x, y) {
            return &[];
        }
        &self.cells[y as usize * self.size + x as usize]
    }

    fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.size as i32 && y < self.size as i32
    }
}
