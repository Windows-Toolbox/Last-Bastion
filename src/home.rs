//! The home screens (DESIGN.md 10.1, a first version) – of the game and of the map editor (`Program`):
//! - **Main page.** The game: New game, Map editor, Custom maps, Settings, Quit. The editor: New map, Open a map,
//!   Settings, Quit. In front of a map of the current setup.
//! - **Setup page:** pick a generator, change its values, pick a seed, set the round (days, waves) – the preview on
//!   the right shows the map right away. "Start game" plays it, "Open in the editor" edits it.
//! - **Maps:** the files in the maps folder (`mapfile.rs`) – every game that was started and every map saved in the
//!   editor, newest first: play, edit, delete.
//! - **Settings:** the same page as in the Esc menu.
//!
//! Regions, research and records (DESIGN.md 10.1–10.2) come later – they need a whole round with a score first.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use macroquad::prelude::*;

use crate::app::{self, Program};
use crate::iso::{iso, TILE_H, TILE_W};
use crate::map::Region;
use crate::mapfile::{self, Entry, MapFile};
use crate::mapgen::{self, Generator, MapSetup};
use crate::menu::settings_page;
use crate::picture::{Extras, MapPicture};
use crate::rng::Rng;
use crate::settings::{self, Settings};
use crate::widgets::{self, button, button_lit, field_slider, text_box, tooltip, type_into, Column, ACCENT, DIM};

/// What the home screen wants `app.rs` to do.
pub enum HomeAction {
    Nothing,
    /// A new map from this setup and seed – the game plays it, the editor opens it.
    Start { seed: u64, setup: MapSetup },
    /// A map from the maps folder – the game plays it, the editor opens it.
    Open { file: MapFile, path: PathBuf },
    Quit,
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Main,
    Setup,
    Maps,
    Settings,
}

/// This many maps fit on the maps page at once – the mouse wheel scrolls the rest.
const SHOWN: usize = 10;

pub struct Home {
    program: Program,
    page: Page,
    pub settings: Settings,
    pub setup: MapSetup,
    seed: u64,
    /// While a seed is being typed: the text so far.
    seed_text: Option<String>,
    preview: MapPicture,
    /// The setup or seed changed since the preview was painted – and when it was painted last (seconds).
    preview_stale: bool,
    preview_time: f64,
    /// The maps in the folder (maps page), read when the page is opened.
    maps: Vec<Entry>,
    /// How many maps of the list are scrolled past.
    scroll: usize,
    /// "Delete" was clicked once on this map, at this time – a second click within 3 s deletes it.
    delete_armed: Option<(PathBuf, f64)>,
    /// A line for the player (a map that couldn't be loaded, the editor that couldn't be started …).
    pub message: Option<String>,
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
    pub fn new(program: Program, settings: Settings, seed: u64) -> Home {
        Home {
            program,
            page: Page::Main,
            settings,
            setup: load_setup(),
            seed,
            seed_text: None,
            preview: MapPicture::new(),
            preview_stale: true,
            preview_time: 0.0,
            maps: Vec::new(),
            scroll: 0,
            delete_armed: None,
            message: None,
        }
    }

    /// Opens the setup page directly – the editor's "New map…" comes here with the values of the current map.
    pub fn open_setup(&mut self, setup: MapSetup, seed: u64) {
        self.setup = setup;
        self.seed = seed;
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

    /// To the maps page – the list is read from the folder anew.
    fn open_maps(&mut self) {
        self.maps = mapfile::list();
        self.scroll = 0;
        self.message = None;
        self.page = Page::Maps;
    }

    fn main_page(&mut self) -> HomeAction {
        self.draw_backdrop(0.6);
        let x = 80.0;
        draw_text("LAST BASTION", x, 150.0, 84.0, WHITE);
        let subtitle = match self.program {
            Program::Game => "Hold the line. Build the factory. Survive the billions.",
            Program::Editor => "Map editor - every map is one file",
        };
        draw_text(subtitle, x + 4.0, 188.0, 22.0, DIM);
        let mut col = Column { x, y: 240.0, width: 340.0 };
        let mut action = HomeAction::Nothing;
        match self.program {
            Program::Game => {
                if button(col.row(), "New game", true) {
                    self.page = Page::Setup;
                }
                button(col.row(), "Continue (saving a round comes later)", false);
                if button(col.row(), "Map editor", true) {
                    // `err()`: the error as `Some`, or `None` if it worked – exactly what the message line wants.
                    self.message = app::launch_editor(None).err();
                }
                if button(col.row(), "Custom maps", true) {
                    self.open_maps();
                }
                button(col.row(), "Research (comes later)", false);
            }
            Program::Editor => {
                if button(col.row(), "New map", true) {
                    self.page = Page::Setup;
                }
                if button(col.row(), "Open a map", true) {
                    self.open_maps();
                }
                if button(col.row(), "Open maps folder", true) {
                    mapfile::open_folder();
                }
            }
        }
        if button(col.row(), "Settings", true) {
            self.page = Page::Settings;
        }
        col.gap();
        if button(col.row(), "Quit", true) {
            action = HomeAction::Quit;
        }
        if let Some(message) = &self.message {
            draw_text(message, x, col.y + 24.0, 20.0, ACCENT);
        }
        draw_text("Prototype - DESIGN.md 13", x, screen_height() - 24.0, 18.0, DIM);
        action
    }

    fn setup_page(&mut self, new_seed: impl Fn() -> u64) -> HomeAction {
        let h = screen_height();
        let left = Rect::new(16.0, 16.0, 440.0, h - 32.0);
        let (title, start) = match self.program {
            Program::Game => ("New game", "Start game"),
            Program::Editor => ("New map", "Open in the editor"),
        };
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

        // Both lists of sliders have to fit into the box: in a 720 px window (Wild: 19 sliders) the rows get lower.
        let count = self.setup.common_fields().len() + self.setup.generator_fields().len();
        let room = left.bottom() - col.y - 2.0 * 30.0 - 8.0;
        let height = (room / count as f32 - 3.0).clamp(19.0, 28.0);
        col.heading("Map");
        for mut field in self.setup.common_fields() {
            let (c, h) = field_slider(col.row_of(height), &mut field);
            changed |= c;
            help = help.or(h);
        }
        col.heading(&format!("{} - values", self.setup.generator.name()));
        for mut field in self.setup.generator_fields() {
            let (c, h) = field_slider(col.row_of(height), &mut field);
            changed |= c;
            help = help.or(h);
        }

        // Right side: preview, the round under it, then the buttons.
        let right = Rect::new(left.right() + 16.0, 16.0, screen_width() - left.right() - 32.0, h - 32.0);
        self.draw_preview(Rect::new(right.x, right.y, right.w, right.h - 200.0));
        let n = self.setup.map_size();
        let info = format!("{n} x {n} tiles ({} inside the border)   seed {}", n - 2 * crate::map::BORDER, self.seed);
        draw_text(&info, right.x, right.bottom() - 184.0, 18.0, DIM);
        draw_text("Round", right.x, right.bottom() - 156.0, 22.0, DIM);
        let mut y = right.bottom() - 148.0;
        // The round doesn't change the map – no new preview needed.
        for mut field in self.setup.round_fields() {
            let (_, h) = field_slider(Rect::new(right.x, y, right.w.min(520.0), 28.0), &mut field);
            help = help.or(h);
            y += 31.0;
        }
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
        if button(Rect::new(right.x + half + 8.0, row2, half, 40.0), start, true) {
            save_setup(&self.setup);
            action = HomeAction::Start { seed: self.seed, setup: self.setup.clone() };
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
        let editor = self.program == Program::Editor;
        let title = if editor { "Open a map" } else { "Custom maps" };
        let mut col = Column::centered(title, SHOWN + 3, 760.0);
        col.note("Every new game and every map saved in the editor is one file in %APPDATA%\\Last Bastion\\maps.");
        if self.maps.is_empty() {
            col.note("No maps yet. Start a new game, or make one in the map editor.");
        }
        // The mouse wheel scrolls the list.
        let wheel = mouse_wheel().1;
        let max_scroll = self.maps.len().saturating_sub(SHOWN);
        if wheel < 0.0 {
            self.scroll = (self.scroll + 1).min(max_scroll);
        } else if wheel > 0.0 {
            self.scroll = self.scroll.saturating_sub(1);
        }
        let now = get_time();
        // What was clicked – carried out after the list, because inside the loop `self.maps` is borrowed.
        let (mut play, mut edit, mut delete): (Option<PathBuf>, Option<PathBuf>, Option<(PathBuf, bool)>) = (None, None, None);
        for entry in self.maps.iter().skip(self.scroll).take(SHOWN) {
            let r = col.row();
            draw_text(&entry.name, r.x + 8.0, r.y + 17.0, 20.0, WHITE);
            draw_text(&details(entry), r.x + 8.0, r.y + 33.0, 15.0, DIM);
            // The buttons from the right edge to the left: `slot` hands out the next place. A closure that changes a
            // variable outside of it (`right`) has to be `mut` itself.
            let mut right = r.right();
            let mut slot = |w: f32| {
                right -= w;
                let place = Rect::new(right, r.y, w - 4.0, r.h);
                right -= 4.0;
                place
            };
            let armed = self.delete_armed.as_ref().is_some_and(|(path, at)| *path == entry.path && now - at < 3.0);
            if button(slot(84.0), if armed { "Sure?" } else { "Delete" }, true) {
                delete = Some((entry.path.clone(), armed));
            }
            if button(slot(76.0), "Edit", true) {
                edit = Some(entry.path.clone());
            }
            if !editor && button(slot(76.0), "Play", true) {
                play = Some(entry.path.clone());
            }
        }
        if self.maps.len() > SHOWN {
            let text = format!("{} - {} of {} maps (mouse wheel)", self.scroll + 1, (self.scroll + SHOWN).min(self.maps.len()), self.maps.len());
            col.note(&text);
        }
        let mut action = HomeAction::Nothing;
        // The game hands a map to edit over to the editor program; the editor opens it itself.
        if let (Some(path), false) = (&edit, editor) {
            self.message = app::launch_editor(Some(path)).err();
        }
        if let Some(path) = play.or(edit.filter(|_| editor)) {
            match MapFile::load(&path) {
                Ok(file) => action = HomeAction::Open { file, path },
                Err(e) => self.message = Some(format!("Couldn't load it: {e}")),
            }
        }
        match delete {
            Some((path, true)) => {
                self.message = fs::remove_file(&path).err().map(|e| format!("Couldn't delete it: {e}"));
                self.delete_armed = None;
                self.maps = mapfile::list();
                self.scroll = self.scroll.min(self.maps.len().saturating_sub(SHOWN));
            }
            Some((path, false)) => self.delete_armed = Some((path, now)),
            None => {}
        }
        if let Some(message) = &self.message {
            col.note(message);
        }
        col.gap();
        let row = col.split(2);
        if button(row[0], "Open maps folder", true) {
            mapfile::open_folder();
        }
        if button(row[1], "Back", true) {
            self.page = Page::Main;
        }
        action
    }
}

/// The small line under a map's name: days, size, how long ago it was changed.
fn details(entry: &Entry) -> String {
    let mut parts = Vec::new();
    match entry.days {
        Some(0) => parts.push("no set length".to_owned()),
        Some(days) => parts.push(format!("{days} days")),
        None => parts.push("can't be read".to_owned()),
    }
    if let Some(field) = entry.field {
        parts.push(format!("{field} x {field} tiles"));
    }
    if let Some(age) = entry.age {
        parts.push(ago(age));
    }
    parts.join(" - ")
}

/// "just now", "5 min ago", "3 h ago", "2 days ago". (`0..60` in a pattern: from 0 up to, not including, 60.)
fn ago(age: Duration) -> String {
    let s = age.as_secs();
    match s {
        0..60 => "just now".to_owned(),
        60..3600 => format!("{} min ago", s / 60),
        3600..86400 => format!("{} h ago", s / 3600),
        _ => format!("{} days ago", s / 86400),
    }
}
