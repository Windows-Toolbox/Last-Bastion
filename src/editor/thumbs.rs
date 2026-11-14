//! Small pictures for the editor's lists (`objects.rs`): TAB's portraits (`assets\ui\portraits\`) and, for what has
//! none, the piece's own picture out of `assets\decor\`.
//!
//! egui draws its own textures, not macroquad's – so a picture is read once more for it: cut to what counts (a
//! portrait to a square, a sheet of several pictures to its first one), shrunk to `SIDE` pixels and handed to egui
//! (`Context::load_texture`). Each is made the first time its row is on screen and then kept; one that isn't there is
//! `None` – the row then shows a plain square with the first letter.

use std::collections::HashMap;

use egui_macroquad::egui::{self, Color32, ColorImage, TextureHandle, TextureOptions};
use macroquad::texture::Image;

use crate::data;

/// A thumbnail's longer edge, in pixels – shown at 22 and at 44 points.
const SIDE: usize = 64;

#[derive(Default)]
pub struct Thumbs {
    /// By the picture's path inside `assets\`, without `.png`.
    made: HashMap<String, Option<TextureHandle>>,
}

impl Thumbs {
    /// The thumbnail of `assets\<path>.png` – read and shrunk the first time.
    pub fn get(&mut self, ctx: &egui::Context, path: &str) -> Option<&TextureHandle> {
        if !self.made.contains_key(path) {
            let texture = load(path).map(|image| ctx.load_texture(path, image, TextureOptions::LINEAR));
            self.made.insert(path.to_owned(), texture);
        }
        self.made.get(path)?.as_ref()
    }

    /// Draws the thumbnail into `rect`, as big as fits (its shape kept) – or a square with `name`'s first letter.
    pub fn paint(&mut self, ui: &egui::Ui, rect: egui::Rect, path: &str, name: &str) {
        let painter = ui.painter();
        match self.get(ui.ctx(), path) {
            Some(texture) => {
                let size = texture.size_vec2();
                let fitted = size * (rect.width() / size.x).min(rect.height() / size.y);
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                painter.image(texture.id(), egui::Rect::from_center_size(rect.center(), fitted), uv, Color32::WHITE);
            }
            None => {
                painter.rect_filled(rect, 3.0, Color32::from_gray(70));
                let letter: String = name.chars().take(1).collect();
                let font = egui::FontId::proportional(rect.height() * 0.6);
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, letter, font, Color32::from_gray(190));
            }
        }
    }
}

/// Reads `assets\<path>.png`, cuts it and shrinks it.
fn load(path: &str) -> Option<ColorImage> {
    let file = data::folder("assets")?.join(format!("{path}.png"));
    let image = Image::from_file_with_format(&std::fs::read(&file).ok()?, None).ok()?;
    let (w, h) = (image.width as usize, image.height as usize);
    // What part of it: the first picture of a sheet (`pictures` in the JSON next to it: x, y, width, height, …) – or,
    // without a JSON, a portrait: the square at its top, like the bar draws it (`hud::draw_face`).
    let (x0, y0, cw, ch) = match sheet_picture(&file.with_extension("json")) {
        Some((x, y, cw, ch)) if x + cw <= w && y + ch <= h && cw > 0 && ch > 0 => (x, y, cw, ch),
        _ => {
            let side = w.min(h);
            ((w - side) / 2, 0, side, side)
        }
    };
    // Shrunk by a whole factor: every new pixel is the mean of `f` × `f` old ones – weighted by how solid each is, so
    // see-through pixels don't darken the edges.
    let f = cw.max(ch).div_ceil(SIDE).max(1);
    let (tw, th) = ((cw / f).max(1), (ch / f).max(1));
    let mut rgba = Vec::with_capacity(tw * th * 4);
    for ty in 0..th {
        for tx in 0..tw {
            let (mut sum, mut alpha, mut n) = ([0u32; 3], 0u32, 0u32);
            for sy in (y0 + ty * f)..(y0 + (ty + 1) * f).min(y0 + ch) {
                for sx in (x0 + tx * f)..(x0 + (tx + 1) * f).min(x0 + cw) {
                    let i = (sy * w + sx) * 4;
                    let a = image.bytes[i + 3] as u32;
                    for (c, total) in sum.iter_mut().enumerate() {
                        *total += image.bytes[i + c] as u32 * a;
                    }
                    alpha += a;
                    n += 1;
                }
            }
            let colour = |total: u32| total.checked_div(alpha).unwrap_or(0) as u8;
            rgba.extend([colour(sum[0]), colour(sum[1]), colour(sum[2]), (alpha / n.max(1)) as u8]);
        }
    }
    Some(ColorImage::from_rgba_unmultiplied([tw, th], &rgba))
}

/// The first picture of a sheet: x, y, width, height (pixels) – from the JSON `tools\tab_*.py` write next to it.
fn sheet_picture(json: &std::path::Path) -> Option<(usize, usize, usize, usize)> {
    let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(json).ok()?).ok()?;
    let first = value.get("pictures")?.get(0)?;
    let number = |k: usize| first.get(k)?.as_f64().map(|v| v.max(0.0) as usize);
    Some((number(0)?, number(1)?, number(2)?, number(3)?))
}
