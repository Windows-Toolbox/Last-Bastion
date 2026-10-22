//! Isometric camera (DESIGN.md 12.3, 12.5): the simulation works on a straight grid, it only becomes
//! a diamond when drawn. And the other way round for the mouse.

use macroquad::prelude::*;

use crate::map::Region;

/// On screen a tile is a diamond, twice as wide as it is high (like in TAB).
pub const TILE_W: f32 = 64.0;
pub const TILE_H: f32 = 32.0;

pub struct Camera {
    /// The world point (in tiles) shown in the middle of the screen.
    pub center: Vec2,
    pub zoom: f32,
}

impl Camera {
    pub fn new(center: Vec2) -> Camera {
        Camera { center, zoom: 0.3 }
    }

    /// World (tiles) → screen (pixels).
    pub fn world_to_screen(&self, p: Vec2) -> Vec2 {
        (iso(p) - iso(self.center)) * self.zoom + screen_center()
    }

    /// Screen (pixels) → world (tiles). Needed for the mouse.
    pub fn screen_to_world(&self, s: Vec2) -> Vec2 {
        iso_inverse((s - screen_center()) / self.zoom + iso(self.center))
    }

    /// Moves the camera by this many pixels on screen – but never with its centre outside `area`.
    pub fn pan(&mut self, screen_delta: Vec2, area: Region) {
        self.center += iso_inverse(screen_delta / self.zoom);
        self.keep_inside(area);
    }

    /// The camera stops at the edge of `area` (in the game view: the world border).
    pub fn keep_inside(&mut self, area: Region) {
        self.center = area.clamp(self.center, 0.0);
    }

    pub fn zoom_by(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(0.15, 3.0);
    }
}

fn screen_center() -> Vec2 {
    vec2(screen_width(), screen_height()) / 2.0
}

/// Straight grid → diamond: x goes to the lower right, y to the lower left.
pub fn iso(p: Vec2) -> Vec2 {
    vec2((p.x - p.y) * TILE_W / 2.0, (p.x + p.y) * TILE_H / 2.0)
}

/// The inverse of `iso`.
pub fn iso_inverse(s: Vec2) -> Vec2 {
    let a = s.x / (TILE_W / 2.0); // = x - y
    let b = s.y / (TILE_H / 2.0); // = x + y
    vec2((a + b) / 2.0, (b - a) / 2.0)
}
