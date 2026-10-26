//! The home screens (DESIGN.md 10.1, a first version) – of the game and of the map editor (`Program`):
//! - **Main page.** The game: New game, Map editor, Custom maps, Settings, Quit. The editor: New map, Open a map,
//!   Settings, Quit. In front of a map of the current setup.
//! - **New game** (the game, like TAB's): pick a map (`data\game_maps.json`), a difficulty (`game_difficulties.json`)
//!   and how long the round lasts (`game_durations.json`) – the preview shows the map, the score factor what the
//!   round is worth. "Custom setup" goes on to the setup page with the picked map's values. Or, like TAB's "Custom
//!   Levels", one of the maps saved in the editor – its round is in its file.
//! - **Setup page** (the game's "Custom setup", the editor's "New map"): pick a generator, change its values, pick a
//!   seed, set the round (difficulty, days) – the preview on the right shows the map right away. "Start game" plays
//!   it, "Open in the editor" edits it.
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
use crate::data;
use crate::iso::{iso, TILE_H, TILE_W};
use crate::keymap::KeyMap;
use crate::map::Map;
use crate::mapfile::{self, Entry, MapFile};
use crate::mapgen::{self, Generator, MapSetup, RoundSetup};
use crate::menu::settings_page;
use crate::picture::{Extras, MapPicture};
use crate::rng::Rng;
use crate::settings::{self, Settings};
use crate::sim::{spawner_tiles, Axes, DAY_TICKS, TICKS_PER_SECOND};
use crate::widgets::{self, button, button_lit, field_slider, pick, text_box, tooltip, type_into, Column, ACCENT, DIM, GAP, ROW};

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
    NewGame,
    Setup,
    Maps,
    Settings,
    Keys,
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
    /// New game: the custom levels instead of the game's maps (like TAB's "Custom Levels") – the maps saved in the
    /// editor (read when the list is opened), how many of them are scrolled past, and the picked one with its file.
    custom: bool,
    levels: Vec<Entry>,
    level_scroll: usize,
    level: Option<(PathBuf, MapFile)>,
    /// The edge of the map in the preview right now – the setup's, or the picked level's (`draw_preview`).
    preview_size: usize,
    /// The page "Key mapping" (`keymap.rs`) – the same as in the Esc menu.
    keymap: KeyMap,
}

/// The values of the last setup are kept here, so the setup screen starts where you left it.
fn setup_file() -> Option<PathBuf> {
    settings::folder().map(|dir| dir.join("map_setup.json"))
}

/// The very first time (no file yet): the default setup, as long as the default duration says.
pub fn load_setup() -> MapSetup {
    setup_file().and_then(|path| fs::read_to_string(path).ok()).and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_else(|| {
        let mut setup = MapSetup::default();
        setup.round.days = data::get().durations.default_days as f32;
        setup
    })
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
            custom: false,
            levels: Vec::new(),
            level_scroll: 0,
            level: None,
            preview_size: 0,
            keymap: KeyMap::new(),
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
            // While a new key is being set, Esc only stops that.
            if !(self.page == Page::Keys && self.keymap.escape()) {
                self.page = Page::Main;
            }
            return HomeAction::Nothing;
        }
        match self.page {
            Page::Main => self.main_page(),
            Page::NewGame => self.new_game_page(new_seed),
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
            Page::Keys => {
                self.draw_backdrop(0.8);
                if self.keymap.show(&mut self.settings.keys) {
                    self.page = Page::Main;
                }
                HomeAction::Nothing
            }
        }
    }

    /// Paints the preview anew when something changed – but at most every 0.15 s while a slider is being dragged,
    /// a big map takes up to 30 ms.
    /// On the new game page with a custom level picked, the preview shows that one – from its file, nothing to generate.
    fn refresh_preview(&mut self) {
        let dragging = is_mouse_button_down(MouseButton::Left);
        if !self.preview_stale || (dragging && get_time() - self.preview_time < 0.15) {
            return;
        }
        let level = self.level.as_ref().filter(|_| self.custom && self.page == Page::NewGame);
        let (map, spawners) = match level.map(|(_, file)| (file.map(), file.spawner_tiles())) {
            Some((Ok(map), spawners)) => (map, spawners),
            // `Some((Err(_), _))`: a level whose map is broken – it can't be started either, the setup's map is shown.
            _ => {
                let generated = mapgen::generate(self.seed, &self.setup);
                // `flat_map`: every spawner gives a list of tiles, one after the other they make one list.
                let spawners = generated.spawners.iter().flat_map(|s| spawner_tiles(Axes::Screen, s.min, s.size)).collect();
                (generated.map, spawners)
            }
        };
        let extras = Extras { fog: None, spawners: &spawners, base: true, shade_ring: true };
        self.preview.paint(&map, &extras);
        self.preview_size = map.size;
        self.preview_stale = false;
        self.preview_time = get_time();
    }

    /// The preview of the whole map (turned like on screen, twice as wide as high), as big as fits into `r`.
    fn draw_preview(&self, r: Rect) {
        let n = self.preview_size.max(1);
        let area = Map::full_area_of(n);
        let size = vec2(n as f32 * TILE_W, n as f32 * TILE_H) / 2.0;
        let scale = (r.w / size.x).min(r.h / size.y);
        let top_left = vec2(r.x, r.y) + (vec2(r.w, r.h) - size * scale) / 2.0;
        let origin = iso(area.corners()[0]);
        self.preview.draw(area, |p| top_left + (iso(p) - origin) * scale);
    }

    /// The map of the current setup over the whole screen, darkened – behind the main page.
    fn draw_backdrop(&self, dark: f32) {
        self.draw_preview(Rect::new(-60.0, 0.0, screen_width() + 120.0, screen_height() * 1.15));
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.03, 0.04, 0.05, dark));
    }

    /// To the new game page: the picks of last time, with fresh values from `data\` – a map changed in
    /// `game_maps.json` shows as it is now. A difficulty that isn't in the list any more becomes the default one, a
    /// number of days the closest duration.
    fn open_new_game(&mut self) {
        data::reload();
        let data = data::get();
        self.setup.round.difficulty = data.difficulty(&self.setup.round.difficulty).name.clone();
        self.setup.round.days = data.duration(self.setup.round.days.round() as u32).days as f32;
        self.setup = picked_setup(&self.setup.map, &self.setup.round);
        self.preview_stale = true;
        self.page = Page::NewGame;
    }

    /// To the setup page, with fresh values from `data\` (the difficulties and durations to pick).
    fn open_setup_page(&mut self) {
        data::reload();
        self.preview_stale = true;
        self.page = Page::Setup;
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
                    self.open_new_game();
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
                    self.open_setup_page();
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
        if button(col.row(), "Key mapping", true) {
            self.page = Page::Keys;
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

    /// The new game page, like TAB's: a map, a difficulty, how long the round lasts – and what the round is worth (the
    /// score factor: the three multiplied). "Custom setup" goes on to every value of the picked map (the setup page).
    fn new_game_page(&mut self, new_seed: impl Fn() -> u64) -> HomeAction {
        let h = screen_height();
        let left = Rect::new(16.0, 16.0, 440.0, h - 32.0);
        let mut col = Column::boxed("New game", left);
        // `&'static`: the values live as long as the program (`data::get`) – borrowing them doesn't hold up `self`.
        let data = data::get();
        // Like TAB's new game: the game's maps – or the custom levels, made in the editor.
        let tabs = col.split(2);
        if button_lit(tabs[0], "Maps", true, !self.custom) && self.custom {
            self.custom = false;
            self.preview_stale = true;
        }
        if button_lit(tabs[1], "Custom levels", true, self.custom) && !self.custom {
            self.open_levels();
        }
        if self.custom {
            return self.levels_page(col, left);
        }

        for map in &data.maps.maps {
            let picked = map.name == self.setup.map;
            if pick(col.row(), &map.name, &format!("{} %", map.score_percent), picked) && !picked {
                self.setup = picked_setup(&map.name, &self.setup.round);
                self.preview_stale = true;
            }
        }
        col.wrap(&data.game_map(&self.setup.map).about);
        self.round_picks(&mut col, true);
        col.gap();
        if self.seed_row(col.row(), &new_seed) {
            self.preview_stale = true;
        }

        // Right side: the preview, what was picked and its score factor, the buttons.
        let right = Rect::new(left.right() + 16.0, 16.0, screen_width() - left.right() - 32.0, h - 32.0);
        self.draw_preview(Rect::new(right.x, right.y, right.w, right.h - 130.0));
        let map = data.game_map(&self.setup.map);
        let difficulty = data.difficulty(&self.setup.round.difficulty);
        let duration = data.duration(self.setup.round.days.round() as u32);
        let picked = format!("{} - {} - {} days ({})", map.name, difficulty.name, duration.days, play_time(duration.days));
        draw_text(&picked, right.x, right.bottom() - 98.0, 24.0, WHITE);
        let factor = map.score_percent as u64 * difficulty.score_percent as u64 * duration.score_percent as u64 / 10_000;
        let score = format!("Score factor {factor} %  (map {} % x difficulty {} % x duration {} %)", map.score_percent, difficulty.score_percent, duration.score_percent);
        draw_text(&score, right.x, right.bottom() - 70.0, 19.0, ACCENT);

        let third = (right.w - 16.0) / 3.0;
        let row = right.bottom() - 44.0;
        let mut action = HomeAction::Nothing;
        if button(Rect::new(right.x, row, third, 40.0), "Back", true) {
            self.page = Page::Main;
        }
        if button(Rect::new(right.x + third + 8.0, row, third, 40.0), "Custom setup...", true) {
            self.open_setup_page();
        }
        let start = Rect::new(right.x + 2.0 * (third + 8.0), row, third, 40.0);
        if button(start, "Start game", true) {
            save_setup(&self.setup);
            action = HomeAction::Start { seed: self.seed, setup: self.setup.clone() };
        }
        draw_rectangle_lines(start.x, start.y, start.w, start.h, 2.0, ACCENT);
        action
    }

    /// "Custom levels" on the new game page: the list of maps saved in the editor (newest first) is read anew, the first
    /// one picked – or the one picked before, if it's still there.
    fn open_levels(&mut self) {
        self.custom = true;
        self.levels = mapfile::list().into_iter().filter(|entry| entry.custom).collect();
        self.level_scroll = 0;
        let before = self.level.take().map(|(path, _)| path);
        let pick = before.filter(|path| self.levels.iter().any(|e| e.path == *path)).or(self.levels.first().map(|e| e.path.clone()));
        if let Some(path) = pick {
            self.pick_level(path);
        }
        self.preview_stale = true;
    }

    /// Picks a custom level: its file is read (for the preview and what it says about its round).
    fn pick_level(&mut self, path: PathBuf) {
        match MapFile::load(&path) {
            Ok(file) => self.level = Some((path, file)),
            Err(e) => {
                self.level = None;
                self.message = Some(format!("Couldn't load it: {e}"));
            }
        }
        self.preview_stale = true;
    }

    /// The new game page with "Custom levels": on the left the levels, on the right the picked one. Its round is in its
    /// file – days, difficulty, waves, what the colony starts with – so there's nothing else to pick, and no score
    /// factor: a level made by hand isn't measured against the game's maps.
    fn levels_page(&mut self, mut col: Column, left: Rect) -> HomeAction {
        col.heading("Custom levels");
        if self.levels.is_empty() {
            col.wrap("None yet. Every map saved in the map editor is a custom level - it shows up here.");
        }
        // As many rows as fit into the box, the mouse wheel scrolls the rest.
        let fit = (((left.bottom() - col.y - 40.0) / (ROW + GAP)) as usize).max(1);
        let wheel = mouse_wheel().1;
        if wheel < 0.0 {
            self.level_scroll = (self.level_scroll + 1).min(self.levels.len().saturating_sub(fit));
        } else if wheel > 0.0 {
            self.level_scroll = self.level_scroll.saturating_sub(1);
        }
        let mut picked = None;
        for entry in self.levels.iter().skip(self.level_scroll).take(fit) {
            let lit = self.level.as_ref().is_some_and(|(path, _)| *path == entry.path);
            let detail = match entry.days {
                Some(0) | None => entry.difficulty.clone(),
                Some(days) => format!("{days} days - {}", entry.difficulty),
            };
            if pick(col.row(), &entry.name, &detail, lit) && !lit {
                picked = Some(entry.path.clone());
            }
        }
        if let Some(path) = picked {
            self.pick_level(path);
        }
        if self.levels.len() > fit {
            let shown = format!("{} - {} of {} (mouse wheel)", self.level_scroll + 1, (self.level_scroll + fit).min(self.levels.len()), self.levels.len());
            col.note(&shown);
        }

        // Right side: the preview, what the level is, the buttons.
        let h = screen_height();
        let right = Rect::new(left.right() + 16.0, 16.0, screen_width() - left.right() - 32.0, h - 32.0);
        self.draw_preview(Rect::new(right.x, right.y, right.w, right.h - 130.0));
        let level = self.level.as_ref().map(|(_, file)| file);
        if let Some(file) = level {
            let difficulty = &data::get().difficulty(&file.round.difficulty).name;
            let length = match file.round.days {
                0 => "no set length".to_owned(),
                days => format!("{days} days ({})", play_time(days)),
            };
            draw_text(format!("{} - {difficulty} - {length}", file.name), right.x, right.bottom() - 98.0, 24.0, WHITE);
            draw_text("Custom level - its round is in the file: days, difficulty, waves, the start.", right.x, right.bottom() - 70.0, 19.0, ACCENT);
        } else if let Some(message) = &self.message {
            draw_text(message, right.x, right.bottom() - 70.0, 19.0, ACCENT);
        }
        let third = (right.w - 16.0) / 3.0;
        let row = right.bottom() - 44.0;
        let mut action = HomeAction::Nothing;
        if button(Rect::new(right.x, row, third, 40.0), "Back", true) {
            self.page = Page::Main;
            self.preview_stale = true;
        }
        let path = self.level.as_ref().map(|(path, _)| path.clone());
        if button(Rect::new(right.x + third + 8.0, row, third, 40.0), "Edit in the editor", path.is_some()) {
            self.message = path.as_ref().and_then(|path| app::launch_editor(Some(path)).err());
        }
        let start = Rect::new(right.x + 2.0 * (third + 8.0), row, third, 40.0);
        if button(start, "Start game", path.is_some()) {
            // Played from its own file, like "Custom maps - Play": "Restart" reads it anew.
            if let Some((path, file)) = self.level.clone() {
                action = HomeAction::Open { file, path };
            }
        }
        draw_rectangle_lines(start.x, start.y, start.w, start.h, 2.0, ACCENT);
        action
    }

    /// The round, both pages: a row of difficulties and a row of durations. `notes`: what the picked ones mean under
    /// each row (the new game page has room for it).
    fn round_picks(&mut self, col: &mut Column, notes: bool) {
        let data = data::get();
        let round = &mut self.setup.round;
        col.heading("Difficulty");
        let list = &data.difficulties.difficulties;
        let current = data.difficulty(&round.difficulty).name.clone();
        for (r, d) in col.split(list.len()).into_iter().zip(list) {
            if button_lit(r, &d.name, true, d.name == current) {
                round.difficulty = d.name.clone();
            }
        }
        if notes {
            let d = data.difficulty(&round.difficulty);
            col.wrap(&format!("{} - score {} %", d.about, d.score_percent));
        }
        col.heading("Duration");
        let list = &data.durations.durations;
        let days = round.days.round() as u32;
        for (r, d) in col.split(list.len()).into_iter().zip(list) {
            if button_lit(r, &format!("{} days", d.days), true, d.days == days) {
                round.days = d.days as f32;
            }
        }
        if notes {
            let d = data.duration(round.days.round() as u32);
            let waves = format!("first wave on day {}, then every {} days", d.first_wave_day, d.wave_every_days);
            col.wrap(&format!("{} at speed x1 - {waves} - score {} %", play_time(d.days), d.score_percent));
        }
    }

    /// The seed: click the box to type one, or roll a new one. `true` if it changed.
    fn seed_row(&mut self, row: Rect, new_seed: &impl Fn() -> u64) -> bool {
        let mut changed = false;
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
        changed
    }

    fn setup_page(&mut self, new_seed: impl Fn() -> u64) -> HomeAction {
        let h = screen_height();
        let left = Rect::new(16.0, 16.0, 440.0, h - 32.0);
        let (title, start) = match self.program {
            Program::Game => ("Custom setup", "Start game"),
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
        changed |= self.seed_row(col.row(), &new_seed);

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
        self.draw_preview(Rect::new(right.x, right.y, right.w, right.h - 262.0));
        let inside = Map::play_area_of(self.setup.map_size(), crate::map::BORDER).area();
        let info = format!("{:.0} tiles inside the border (like {:.0} x {:.0})   seed {}", inside, inside.sqrt(), inside.sqrt(), self.seed);
        draw_text(&info, right.x, right.bottom() - 244.0, 18.0, DIM);
        // The round doesn't change the map – no new preview needed.
        let mut round = Column { x: right.x, y: right.bottom() - 236.0, width: right.w.min(560.0) };
        self.round_picks(&mut round, false);
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
            // The game came here from "New game" – back there (the picked map's values again).
            match self.program {
                Program::Game => self.open_new_game(),
                Program::Editor => self.page = Page::Main,
            }
        }
        if button(Rect::new(right.x + half + 8.0, row2, half, 40.0), start, true) {
            save_setup(&self.setup);
            // Still the picked map? Only with all of its values – one changed by hand, and it's a map of its own.
            let mut setup = self.setup.clone();
            if !setup.map.is_empty() && picked_setup(&setup.map, &setup.round) != setup {
                setup.map.clear();
            }
            action = HomeAction::Start { seed: self.seed, setup };
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

/// A map of `data\game_maps.json` (an unknown name: the default map) with this round – the setup a new game starts
/// with. `..`: every other field from the map's setup.
fn picked_setup(map: &str, round: &RoundSetup) -> MapSetup {
    let picked = data::get().game_map(map);
    MapSetup { map: picked.name.clone(), round: round.clone(), ..picked.setup.clone() }
}

/// How long a round of `days` days takes at speed x1: "30 min", "2 h", "1 h 30 min".
fn play_time(days: u32) -> String {
    let minutes = (days as u64 * DAY_TICKS) as f32 / TICKS_PER_SECOND / 60.0;
    let minutes = minutes.round() as u32;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
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
        parts.push(format!("like {field} x {field} tiles"));
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
