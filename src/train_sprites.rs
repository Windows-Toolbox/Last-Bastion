//! Pictures for the trains (`sim/trains.rs`): TAB's locomotive, its three cars and their wheels, put together by
//! `tools\tab_trains.py` (explained in `assets\README.md`). Only drawing – the simulation never sees them. Loaded once
//! by `Sprites` (`sprites.rs`), like the figures.
//!
//! How TAB draws a car (`TrainCarriage.OnUpdateSceneObjectParallel`): one of 91 pictures all the way round – the one
//! for its angle, picked with TAB's own formula (kept in the JSON as `angles`) and turned by what's left between two of
//! them; behind it its shadow (the same picture in black, a bit to the left and bigger) and its six wheels, each its own
//! picture turning with the way it went.

use macroquad::prelude::*;
use serde::Deserialize;

use crate::data;
use crate::iso::{Camera, TILE_W};
use crate::sim::CarKind;
use crate::sprites::smooth;

/// `assets\trains\<car>.json`.
#[derive(Deserialize)]
struct CarSheet {
    image: String,
    /// This many pixels are one tile width on screen at zoom 1.
    pixels_per_tile: f32,
    /// Every picture: `[x, y, width, height, middle x, middle y]` in pixels.
    pictures: Vec<[f32; 6]>,
    /// For angle number n (n × 360 / 91 degrees): picture `angles[n]`.
    angles: Vec<usize>,
    /// The picture's middle stands this far from the car's middle (tiles on the map, not turned) and this high (tile
    /// widths).
    offset: [f32; 2],
    height: f32,
    shadow: Shadow,
    /// Where its six wheels sit: tiles along it and across, turned with it.
    wheels: Vec<[f32; 2]>,
}

#[derive(Deserialize)]
struct Shadow {
    opacity: f32,
    /// Tile widths to the left on screen, and how much bigger.
    left: f32,
    scale: f32,
}

/// `assets\trains\wheel.json`: per angle number `frames` pictures of it turning.
#[derive(Deserialize)]
struct WheelSheet {
    image: String,
    pixels_per_tile: f32,
    pictures: Vec<[f32; 6]>,
    frames: usize,
    /// It turns this many pictures per tile the car went, and sits this many tile widths up.
    per_tile_travelled: f32,
    up: f32,
}

struct Car {
    sheet: CarSheet,
    texture: Texture2D,
}

/// The trains' pictures, loaded.
pub struct TrainSprites {
    /// In the order of `CarKind`: locomotive, colonists', armoured, goods car.
    cars: Vec<Car>,
    wheel: WheelSheet,
    wheel_texture: Texture2D,
}

/// A JSON sheet and its PNG out of `assets\trains\`.
fn load_sheet<T: for<'de> Deserialize<'de>>(name: &str, image: impl Fn(&T) -> &str) -> Result<(T, Texture2D), String> {
    let folder = data::folder("assets").ok_or("no assets\\ folder")?.join("trains");
    let json = folder.join(format!("{name}.json"));
    let at = |e: &dyn std::fmt::Display| format!("{}: {e}", json.display());
    let text = std::fs::read_to_string(&json).map_err(|e| at(&e))?;
    let sheet: T = serde_json::from_str(&text).map_err(|e| at(&e))?;
    let png = folder.join(image(&sheet));
    let bytes = std::fs::read(&png).map_err(|e| format!("{}: {e}", png.display()))?;
    let picture = Image::from_file_with_format(&bytes, None).map_err(|e| format!("{}: {e}", png.display()))?;
    let texture = Texture2D::from_image(&picture);
    smooth(&texture);
    Ok((sheet, texture))
}

impl TrainSprites {
    /// All of them – `Ok(None)`: there is no `assets\trains\` (the trains are drawn as blocks then).
    pub fn load() -> Result<Option<TrainSprites>, String> {
        let there = data::folder("assets").is_some_and(|a| a.join("trains").join("locomotive.json").is_file());
        if !there {
            return Ok(None);
        }
        let mut cars = Vec::new();
        for name in ["locomotive", "colonist_carriage", "armored_carriage", "goods_carriage"] {
            let (sheet, texture) = load_sheet::<CarSheet>(name, |s| &s.image)?;
            if sheet.angles.is_empty() || sheet.angles.iter().any(|&a| a >= sheet.pictures.len()) {
                return Err(format!("assets\\trains\\{name}.json: an angle names a picture that isn't there"));
            }
            cars.push(Car { sheet, texture });
        }
        let (wheel, wheel_texture) = load_sheet::<WheelSheet>("wheel", |s| &s.image)?;
        Ok(Some(TrainSprites { cars, wheel, wheel_texture }))
    }

    /// Draws car `kind` with its middle on `pos` (tiles), pointing along `dir` (length 1), its wheels turned as far as
    /// `travelled` tiles take them – shadow, wheels, then the car.
    pub fn draw(&self, camera: &Camera, kind: CarKind, pos: Vec2, dir: Vec2, travelled: f32, color: Color) {
        let car = &self.cars[kind as usize];
        let s = &car.sheet;
        let unit = TILE_W * camera.zoom;
        let degrees = dir.y.atan2(dir.x).to_degrees().rem_euclid(360.0);
        let n = s.angles.len();
        let number = (degrees * n as f32 / 360.0).round() as usize % n;
        let picture = s.angles[number];
        // What's left to the exact angle, turned on screen (TAB: none at a right angle).
        let rest = if degrees % 90.0 == 0.0 { 0.0 } else { (degrees - number as f32 * 360.0 / n as f32).to_radians() };
        let middle = camera.world_to_screen(pos + Vec2::from(s.offset)) - vec2(0.0, s.height * unit);
        let scale = TILE_W / s.pixels_per_tile * camera.zoom;
        let [_, _, w, h, _, _] = s.pictures[picture];
        let reach = w.max(h) * scale;
        if middle.x < -reach || middle.y < -reach || middle.x > screen_width() + reach || middle.y > screen_height() + reach {
            return;
        }
        let shadow = Color::new(0.0, 0.0, 0.0, s.shadow.opacity * color.a);
        draw_picture(&car.texture, s.pictures[picture], middle - vec2(s.shadow.left * unit, 0.0), scale * s.shadow.scale, rest, shadow);
        let wheel = &self.wheel;
        let turned = (travelled * wheel.per_tile_travelled).rem_euclid(wheel.frames as f32) as usize % wheel.frames.max(1);
        if let Some(&frame) = wheel.pictures.get(picture * wheel.frames + turned) {
            let side = vec2(-dir.y, dir.x);
            let wheel_scale = TILE_W / wheel.pixels_per_tile * camera.zoom;
            for &[along, across] in &s.wheels {
                let at = camera.world_to_screen(pos + dir * along + side * across) - vec2(0.0, wheel.up * unit);
                draw_picture(&self.wheel_texture, frame, at, wheel_scale, 0.0, color);
            }
        }
        draw_picture(&car.texture, s.pictures[picture], middle, scale, rest, color);
    }
}

/// One picture `[x, y, w, h, middle x, middle y]` of a sheet with its middle on `middle` (screen), `scale` screen
/// pixels per pixel, turned by `angle` (radians) round its middle.
fn draw_picture(texture: &Texture2D, [x, y, w, h, mx, my]: [f32; 6], middle: Vec2, scale: f32, angle: f32, color: Color) {
    let corner = middle - vec2(mx, my) * scale;
    let params = DrawTextureParams {
        dest_size: Some(vec2(w, h) * scale),
        source: Some(Rect::new(x, y, w, h)),
        rotation: angle,
        pivot: Some(middle),
        ..Default::default()
    };
    draw_texture_ex(texture, corner.x, corner.y, color, params);
}
