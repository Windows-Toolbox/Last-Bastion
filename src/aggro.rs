//! Threat map: for every tile the nearest target nearby (if there is one) – a unit or a building.
//! Instead of each of the thousands of zombies searching the area, each of the few targets enters its
//! surroundings into the map once – after that every zombie only reads its own tile.
//! There are two such maps: one for units (new every tick) and one for buildings (only when the buildings
//! change).

use macroquad::math::{vec2, Vec2};

const NONE: u32 = u32::MAX;

#[derive(Clone)]
pub struct AggroMap {
    size: usize,
    /// Per tile: index of the nearest target in its list (units or buildings), or `NONE`.
    target: Vec<u32>,
    /// Per tile: how far that target is from the tile centre.
    dist: Vec<f32>,
}

impl AggroMap {
    pub fn new(size: usize) -> AggroMap {
        AggroMap {
            size,
            target: vec![NONE; size * size],
            dist: vec![f32::MAX; size * size],
        }
    }

    pub fn clear(&mut self) {
        self.target.fill(NONE);
        self.dist.fill(f32::MAX);
    }

    /// Enters the rectangle `min`–`max` (a building – or a point if `min == max`) as target number
    /// `index` into all tiles that are at most `radius` away from its edge.
    pub fn stamp(&mut self, index: usize, min: Vec2, max: Vec2, radius: f32) {
        let r = radius.ceil() as i32;
        for y in min.y.floor() as i32 - r..=max.y.floor() as i32 + r {
            for x in min.x.floor() as i32 - r..=max.x.floor() as i32 + r {
                if !self.inside(x, y) {
                    continue;
                }
                let center = vec2(x as f32 + 0.5, y as f32 + 0.5);
                let d = (min - center).max(center - max).max(Vec2::ZERO).length();
                let tile = y as usize * self.size + x as usize;
                // The closer target wins. Equally far: the one entered first stays (DESIGN.md 12.8).
                if d <= radius && d < self.dist[tile] {
                    self.dist[tile] = d;
                    self.target[tile] = index as u32;
                }
            }
        }
    }

    /// The target closest to this spot – or `None` if there is none nearby.
    pub fn near(&self, pos: Vec2) -> Option<usize> {
        let (x, y) = (pos.x.floor() as i32, pos.y.floor() as i32);
        if !self.inside(x, y) {
            return None;
        }
        match self.target[y as usize * self.size + x as usize] {
            NONE => None,
            i => Some(i as usize),
        }
    }

    fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.size as i32 && y < self.size as i32
    }
}
