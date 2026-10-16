//! The home screen (DESIGN.md 10.1, a first version): what you see after starting the game.
//! - **Main page:** New game, Map editor, Custom maps, Settings, Quit – in front of a map of the current setup.
//! - **Setup page:** pick a generator, change its values, pick a seed – the preview on the right shows the map
//!   right away. Start plays it (or opens it in the editor).
//! - **Custom maps:** the maps saved in the editor (`mapfile.rs`) – play or edit.
//! - **Settings:** the same page as in the Esc menu.
//!
//! Regions, research and records (DESIGN.md 10.1–10.2) come later – they need a whole round with a score first.

use std::fs;
use std::path::PathBuf;

use macroquad::prelude::*;

use crate::iso::{iso, TILE_H, TILE_W};
use crate::map::Region;
use crate::mapfile::{self, MapFile};
use crate::mapgen::{self, Generator, MapSetup};
use crate::menu::settings_page;
use crate::picture::{Extras, MapPicture};
use crate::rng::Rng;
use crate::settings::{self, Settings};
use crate::widgets::{self, button, button_lit, field_slider, text_box, tooltip, type_into, Column, ACCENT, DIM};

/// What the home screen wants `main.rs` to do.
pub enum HomeAction {
    Nothing,
    /// A new map from this setup and seed – in the game view or right in the editor.
    Start { seed: u64, setup: MapSetup, editor: bool },
    /// A saved map.
    Open { file: MapFile, editor: bool },
    Quit,
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Main,
    Setup,
    Maps,
    Settings,
}

pub struct Home {
    page: Page,
    pub settings: Settings,
    pub setup: MapSetup,
    seed: u64,
    /// While a seed is being typed: the text so far.
    seed_text: Option<String>,
    /// Does "Start" open the map in the editor (came from "Map editor") or play it?
    for_editor: bool,
    preview: MapPicture,
    /// The setup or seed changed since the preview was painted – and when it was painted last (seconds).
    preview_stale: bool,
    preview_time: f64,
    /// Saved maps (Custom maps page), read when the page is opened.
    maps: Vec<(String, PathBuf)>,
    /// A line for the player (a map that couldn't be loaded …).
    message: Option<String>,
}

/// The values of the last setup are kept here, so the setup screen starts where you left it.
fn setup_file() -> Option<PathBuf> {
    settings::folder().map(|dir| dir.join("map_setup.json"))
}

pub fn load_setup() -> MapSetup {
    setup_file()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_setup(setup: &MapSetup) {
    if let (Some(dir), Some(path), Ok(text)) = (settings::folder(), setup_file(), serde_json::to_string_pretty(setup)) {
        let _ = fs::create_dir_all(dir).and_then(|_| fs::write(path, text));
    }
}

impl Home {
    pub fn new(settings: Settings, seed: u64) -> Home {
        Home {
            page: Page::Main,
            settings,
            setup: load_setup(),
            seed,
            seed_text: None,
            for_editor: false,
            preview: MapPicture::new(),
            preview_stale: true,
            preview_time: 0.0,
            maps: Vec::new(),
            message: None,
        }
    }

    /// Opens the setup page directly – the editor's "New map…" comes here with the values of the current map.
    pub fn open_setup(&mut self, setup: MapSetup, seed: u64, for_editor: bool) {
        self.setup = setup;
        self.seed = seed;
        self.for_editor = for_editor;
        self.page = Page::Setup;
        self.preview_stale = true;
    }

    /// One frame of the home screen: draw it, react to clicks.
    pub fn frame(&mut self, new_seed: impl Fn() -> u64) -> HomeAction {
        clear_background(Color::new(0.06, 0.07, 0.09, 1.0));
        self.refresh_preview();
        if self.seed_text.is_none() && is_key_pressed(KeyCode::Escape) && self.page != Page::Main {
            self.page = Page::Main;
            return HomeAction::Nothing;
        }
        match self.page {
            Page::Main => self.main_page(),
            Page::Setup => self.setup_page(new_seed),
            Page::Maps => self.maps_page(),
            Page::Settings => {
                self.draw_backdrop(0.8);
                let mut col = Column::centered("Settings", 13, 460.0);
                if settings_page(&mut col, &mut self.settings) {
                    self.page = Page::Main;
                }
                HomeAction::Nothing
            }
        }
    }

    /// Paints the preview anew when something changed – but at most every 0.15 s while a slider is being dragged,
    /// a big map takes up to 30 ms.
    fn refresh_preview(&mut self) {
        let dragging = is_mouse_button_down(MouseButton::Left);
        if !self.preview_stale || (dragging && get_time() - self.preview_time < 0.15) {
            return;
        }
        let generated = mapgen::generate(self.seed, &self.setup);
        let spawners: Vec<((i32, i32), (i32, i32))> = generated.spawners.iter().map(|s| (s.min, s.size)).collect();
        let extras = Extras { fog: None, spawners: &spawners, base: true, shade_ring: true };
        self.preview.paint(&generated.map, &extras);
        self.preview_stale = false;
        self.preview_time = get_time();
    }

    /// The preview of the map as a diamond, as big as fits into `r`.
    fn draw_preview(&self, r: Rect) {
        let n = self.setup.map_size() as f32;
        let scale = (r.w / (n * TILE_W)).min(r.h / (n * TILE_H));
        let top = vec2(r.x + r.w / 2.0, r.y + (r.h - n * TILE_H * scale) / 2.0);
        self.preview.draw(Region { min: Vec2::ZERO, size: n }, |p| top + iso(p) * scale);
    }

    /// The map of the current setup over the whole screen, darkened – behind the main page.
    fn draw_backdrop(&self, dark: f32) {
        self.draw_preview(Rect::new(-60.0, 0.0, screen_width() + 120.0, screen_height() * 1.15));
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.03, 0.04, 0.05, dark));
    }

    fn main_page(&mut self) -> HomeAction {
        self.draw_backdrop(0.6);
        let x = 80.0;
        draw_text("LAST BASTION", x, 150.0, 84.0, WHITE);
        draw_text("Hold the line. Build the factory. Survive the billions.", x + 4.0, 188.0, 22.0, DIM);
        let mut col = Column { x, y: 240.0, width: 340.0 };
        let mut action = HomeAction::Nothing;
        if button(col.row(), "New game", true) {
            self.for_editor = false;
            self.page = Page::Setup;
        }
        button(col.row(), "Continue (save games come later)", false);
        if button(col.row(), "Map editor", true) {
            self.for_editor = true;
            self.page = Page::Setup;
        }
        if button(col.row(), "Custom maps", true) {
            self.maps = mapfile::list();
            self.message = None;
            self.page = Page::Maps;
        }
        button(col.row(), "Research (comes later)", false);
        if button(col.row(), "Settings", true) {
            self.page = Page::Settings;
        }
        col.gap();
        if button(col.row(), "Quit", true) {
            action = HomeAction::Quit;
        }
        draw_text("Prototype - DESIGN.md 13", x, screen_height() - 24.0, 18.0, DIM);
        action
    }

    fn setup_page(&mut self, new_seed: impl Fn() -> u64) -> HomeAction {
        let h = screen_height();
        let left = Rect::new(16.0, 16.0, 440.0, h - 32.0);
        let title = if self.for_editor { "New map for the editor" } else { "New game" };
        let mut col = Column::boxed(title, left);
        let mut changed = false;
        let mut help: Option<&'static str> = None;

        // Which generator.
        let gens = col.split(Generator::ALL.len());
        for (r, g) in gens.into_iter().zip(Generator::ALL) {
            if button_lit(r, g.name(), true, self.setup.generator == g) && self.setup.generator != g {
                self.setup.generator = g;
                changed = true;
            }
        }
        col.note(self.setup.generator.about());

        // The seed: click the box to type one, or roll a new one.
        let row = col.row();
        let (seed_box, roll) = (Rect::new(row.x, row.y, row.w * 0.62, row.h), Rect::new(row.x + row.w * 0.64, row.y, row.w * 0.36, row.h));
        let shown = self.seed_text.clone().unwrap_or_else(|| self.seed.to_string());
        if text_box(seed_box, "Seed", &shown, self.seed_text.is_some()) && self.seed_text.is_none() {
            self.seed_text = Some(String::new());
        }
        if let Some(text) = &mut self.seed_text {
            if let Some(keep) = type_into(text, 19, |c| c.is_ascii_digit()) {
                if let (true, Ok(seed)) = (keep, text.parse::<u64>()) {
                    self.seed = seed;
                    changed = true;
                }
                self.seed_text = None;
            }
        }
        if button(roll, "New seed", true) {
            self.seed = new_seed();
            self.seed_text = None;
            changed = true;
        }

        col.heading("Map");
        for mut field in self.setup.common_fields() {
            let (c, h) = field_slider(col.thin_row(), &mut field);
            changed |= c;
            help = help.or(h);
        }
        col.heading(&format!("{} - values", self.setup.generator.name()));
        for mut field in self.setup.generator_fields() {
            let (c, h) = field_slider(col.thin_row(), &mut field);
            changed |= c;
            help = help.or(h);
        }

        // Right side: preview, the buttons under it.
        let right = Rect::new(left.right() + 16.0, 16.0, screen_width() - left.right() - 32.0, h - 32.0);
        self.draw_preview(Rect::new(right.x, right.y, right.w, right.h - 120.0));
        let n = self.setup.map_size();
        let info = format!("{n} x {n} tiles ({} inside the border)   seed {}", n - 2 * crate::map::BORDER, self.seed);
        draw_text(&info, right.x, right.bottom() - 104.0, 18.0, DIM);
        let half = (right.w - 8.0) / 2.0;
        let row1 = right.bottom() - 90.0;
        if button(Rect::new(right.x, row1, half, 36.0), "Random values", true) {
            self.setup.randomize_generator(&mut Rng::new(new_seed()));
            changed = true;
        }
        if button(Rect::new(right.x + half + 8.0, row1, half, 36.0), "Defaults", true) {
            self.setup.reset_generator();
            changed = true;
        }
        let row2 = right.bottom() - 44.0;
        let mut action = HomeAction::Nothing;
        if button(Rect::new(right.x, row2, half, 40.0), "Back", true) {
            self.page = Page::Main;
        }
        let start = if self.for_editor { "Open in the editor" } else { "Start game" };
        if button(Rect::new(right.x + half + 8.0, row2, half, 40.0), start, true) {
            save_setup(&self.setup);
            action = HomeAction::Start { seed: self.seed, setup: self.setup.clone(), editor: self.for_editor };
        }
        draw_rectangle_lines(right.x + half + 8.0, row2, half, 40.0, 2.0, ACCENT);
        if changed {
            self.preview_stale = true;
        }
        if let Some(text) = help {
            tooltip(text, widgets::mouse());
        }
        action
    }

    fn maps_page(&mut self) -> HomeAction {
        self.draw_backdrop(0.8);
        let rows = self.maps.len().clamp(1, 12) + 4;
        let mut col = Column::centered("Custom maps", rows, 560.0);
        let mut action = HomeAction::Nothing;
        if self.maps.is_empty() {
            col.note("No maps yet. Make one in the Map editor and press \"Save map\" (tab Map).");
        }
        let mut open: Option<(PathBuf, bool)> = None;
        for (name, path) in self.maps.iter().take(12) {
            let r = col.row();
            draw_text(name, r.x + 8.0, r.y + 24.0, 20.0, WHITE);
            if button(Rect::new(r.right() - 190.0, r.y, 90.0, r.h), "Play", true) {
                open = Some((path.clone(), false));
            }
            if button(Rect::new(r.right() - 94.0, r.y, 94.0, r.h), "Edit", true) {
                open = Some((path.clone(), true));
            }
        }
        if let Some((path, editor)) = open {
            match MapFile::load(&path) {
                Ok(file) => action = HomeAction::Open { file, editor },
                Err(e) => self.message = Some(format!("Couldn't load it: {e}")),
            }
        }
        if let Some(message) = &self.message {
            col.note(message);
        }
        col.gap();
        if button(col.row(), "Open maps folder", true) {
            if let Some(dir) = mapfile::folder() {
                let _ = fs::create_dir_all(&dir);
                let _ = std::process::Command::new("explorer").arg(dir).spawn();
            }
        }
        if button(col.row(), "Back", true) {
            self.page = Page::Main;
        }
        action
    }
}
