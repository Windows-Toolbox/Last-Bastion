//! Sight and fog of war (DESIGN.md 9.2): which tiles your units and buildings see right now, and which ones
//! have ever been seen. Like StarCraft 2 there are three states for the picture:
//! - **visible**: someone sees it right now – terrain, zombies, everything,
//! - **explored**: seen before, but nobody is looking right now – the terrain shows darkened, zombies don't,
//! - **unexplored**: never seen – black.
//!
//! Everyone looks around in a circle (sight range), but not through forest (`Map::clear_sight`).
//! Computed in the simulation, so it's the same on every PC – in co-op both players share the view.

use macroquad::math::{vec2, Vec2};

use crate::map::Map;

#[derive(Clone)]
pub struct Vision {
    size: usize,
    /// Per tile: does someone see it right now? Computed anew every tick.
    visible: Vec<bool>,
    /// Per tile: has someone ever seen it? Never gets reset.
    explored: Vec<bool>,
}

impl Vision {
    pub fn new(size: usize) -> Vision {
        Vision { size, visible: vec![false; size * size], explored: vec![false; size * size] }
    }

    /// Forgets what is visible and looks again from every eye: (position, sight range in tiles).
    pub fn update(&mut self, map: &Map, eyes: impl Iterator<Item = (Vec2, f32)>) {
        self.visible.fill(false);
        for (eye, range) in eyes {
            let r = range.ceil() as i32;
            let (cx, cy) = (eye.x.floor() as i32, eye.y.floor() as i32);
            for y in cy - r..=cy + r {
                for x in cx - r..=cx + r {
                    let Some(i) = self.index(x, y) else {
                        continue;
                    };
                    if self.visible[i] {
                        continue; // someone else already sees it – saves the line of sight
                    }
                    let center = vec2(x as f32 + 0.5, y as f32 + 0.5);
                    if center.distance(eye) <= range && map.clear_sight(eye, center) {
                        self.visible[i] = true;
                        self.explored[i] = true;
                    }
                }
            }
        }
    }

    /// A circle visible – and so explored – on top of what the eyes see, over forests too: the command center's scan
    /// (`sim/scan.rs`).
    pub fn reveal_circle(&mut self, at: Vec2, radius: f32) {
        let r = radius.ceil() as i32;
        let (cx, cy) = (at.x.floor() as i32, at.y.floor() as i32);
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                let center = vec2(x as f32 + 0.5, y as f32 + 0.5);
                if let Some(i) = self.index(x, y).filter(|_| center.distance(at) <= radius) {
                    self.visible[i] = true;
                    self.explored[i] = true;
                }
            }
        }
    }

    /// Everything visible – and so explored – at once: the map is shown (a wave that reveals it, `World::revealed`).
    pub fn reveal_all(&mut self) {
        self.visible.fill(true);
        self.explored.fill(true);
    }

    /// Does someone see this tile right now? Outside the map: no.
    pub fn is_visible(&self, x: i32, y: i32) -> bool {
        self.index(x, y).is_some_and(|i| self.visible[i])
    }

    /// Has someone ever seen this tile?
    pub fn is_explored(&self, x: i32, y: i32) -> bool {
        self.index(x, y).is_some_and(|i| self.explored[i])
    }

    /// Does someone see the tile this point lies on? (For zombies: may it be drawn, clicked?)
    pub fn sees(&self, p: Vec2) -> bool {
        self.is_visible(p.x.floor() as i32, p.y.floor() as i32)
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let inside = x >= 0 && y >= 0 && x < self.size as i32 && y < self.size as i32;
        inside.then(|| y as usize * self.size + x as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Terrain;

    /// Within the sight range you see everything – except what lies behind a forest. What has been seen
    /// stays explored after the eye is gone.
    #[test]
    fn sight_stops_at_forests_and_explored_stays() {
        let mut map = Map::blank(160, crate::map::BORDER);
        let c = map.size as i32 / 2;
        for y in c - 2..=c + 2 {
            map.set_terrain(c + 4, y, Terrain::Forest);
        }
        let mut vision = Vision::new(map.size);
        let eye = vec2(c as f32 + 0.5, c as f32 + 0.5);
        vision.update(&map, [(eye, 8.0)].into_iter());
        assert!(vision.is_visible(c - 6, c), "open ground in range");
        assert!(vision.is_visible(c + 4, c), "the edge of the forest");
        assert!(!vision.is_visible(c + 6, c), "behind the forest");
        assert!(!vision.is_visible(c - 9, c), "out of range");

        vision.update(&map, std::iter::empty());
        assert!(!vision.is_visible(c - 6, c));
        assert!(vision.is_explored(c - 6, c), "seen once = explored for good");
        assert!(!vision.is_explored(c + 6, c));
    }
}
