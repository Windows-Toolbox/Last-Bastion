//! A picture of the whole map – one pixel per tile, kept as a texture on the graphics card. Drawn as a diamond
//! (like the map itself) on the minimap, in the preview of the setup screen and behind the home screen.
//!
//! Before, the minimap drew one small diamond per tile, every frame. On a 256 map that's up to 80,000 diamonds
//! (160,000 triangles) per frame just for the minimap. Now the picture is painted only when something changed
//! (`Map::revision`, or the fog a few times per second) and then drawn as two triangles with the picture on them.

use macroquad::prelude::*;

use crate::draw::{terrain_color, UNEXPLORED};
use crate::map::{Map, Region};
use crate::vision::Vision;

/// Spawners in the preview.
const SPAWNER: Color = Color::new(0.85, 0.25, 0.55, 1.0);
const BASE: Color = Color::new(0.32, 0.45, 0.70, 1.0);
/// The ring outside the world border is drawn a bit darker in previews.
const RING_SHADE: f32 = 0.7;

pub struct MapPicture {
    image: Image,
    /// `None` until painted for the first time (and while there is no graphics card – in the tests).
    texture: Option<Texture2D>,
    /// What the picture shows right now: the map's revision and whether it had fog – so `refresh` knows when to
    /// paint again.
    painted: Option<(u32, bool)>,
    /// Seconds since the fog was painted last.
    fog_age: f32,
}

/// What goes into a picture besides the terrain.
pub struct Extras<'a> {
    /// Fog of war: unexplored tiles black.
    pub fog: Option<&'a Vision>,
    /// Spawners (top left tile, size) – shown in previews.
    pub spawners: &'a [((i32, i32), (i32, i32))],
    /// Mark the base in the middle (previews).
    pub base: bool,
    /// Darken the ring outside the world border (previews).
    pub shade_ring: bool,
}

impl MapPicture {
    pub fn new() -> MapPicture {
        MapPicture { image: Image { bytes: Vec::new(), width: 0, height: 0 }, texture: None, painted: None, fog_age: 0.0 }
    }

    /// Paints the picture anew if the map changed – with fog also every quarter second (what's explored grows).
    pub fn refresh(&mut self, map: &Map, extras: &Extras, dt: f32) {
        self.fog_age += dt;
        let state = (map.revision, extras.fog.is_some());
        let fog_due = extras.fog.is_some() && self.fog_age >= 0.25;
        if self.painted != Some(state) || fog_due {
            self.paint(map, extras);
        }
    }

    /// Paints the picture now: every tile one pixel.
    pub fn paint(&mut self, map: &Map, extras: &Extras) {
        let n = map.size;
        if self.image.width as usize != n {
            self.image = Image { bytes: vec![0; n * n * 4], width: n as u16, height: n as u16 };
        }
        let rgba = |c: Color| [(c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8, 255];
        for y in 0..n as i32 {
            for x in 0..n as i32 {
                let mut c = terrain_color(map.terrain(x, y));
                if extras.fog.is_some_and(|fog| !fog.is_explored(x, y)) {
                    c = UNEXPLORED;
                } else if extras.shade_ring && !map.in_play(x, y) {
                    c = Color::new(c.r * RING_SHADE, c.g * RING_SHADE, c.b * RING_SHADE, 1.0);
                }
                let i = (y as usize * n + x as usize) * 4;
                self.image.bytes[i..i + 4].copy_from_slice(&rgba(c));
            }
        }
        let mut dot = |x: i32, y: i32, c: Color| {
            if x >= 0 && y >= 0 && (x as usize) < n && (y as usize) < n {
                let i = (y as usize * n + x as usize) * 4;
                self.image.bytes[i..i + 4].copy_from_slice(&rgba(c));
            }
        };
        for &(min, size) in extras.spawners {
            for y in min.1..min.1 + size.1 {
                for x in min.0..min.0 + size.0 {
                    dot(x, y, SPAWNER);
                }
            }
        }
        if extras.base {
            let c = n as i32 / 2;
            for y in c - 2..=c + 2 {
                for x in c - 2..=c + 2 {
                    dot(x, y, BASE);
                }
            }
        }
        match &self.texture {
            Some(texture) if texture.width() as usize == n => texture.update(&self.image),
            _ => {
                let texture = Texture2D::from_image(&self.image);
                // Sharp pixels instead of blurred ones – every tile stays a clear little square.
                texture.set_filter(FilterMode::Nearest);
                self.texture = Some(texture);
            }
        }
        self.painted = Some((map.revision, extras.fog.is_some()));
        self.fog_age = 0.0;
    }

    /// Draws the part `area` of the map. `to_screen` turns a world point into a screen point – the four corners of
    /// `area` become the four corners of a diamond, and the picture is stretched over it.
    pub fn draw(&self, area: Region, to_screen: impl Fn(Vec2) -> Vec2) {
        let Some(texture) = &self.texture else {
            return;
        };
        let n = texture.width();
        let corners = [area.min, vec2(area.max().x, area.min.y), area.max(), vec2(area.min.x, area.max().y)];
        let vertices = corners
            .iter()
            .map(|&c| {
                let s = to_screen(c);
                Vertex::new(s.x, s.y, 0.0, c.x / n, c.y / n, WHITE)
            })
            .collect();
        draw_mesh(&Mesh { vertices, indices: vec![0, 1, 2, 0, 2, 3], texture: Some(texture.clone()) });
    }
}
