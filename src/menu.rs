//! The menu on Esc (DESIGN.md 7.1) with the settings (11.4). In single player it stops
//! everything: while it's open, no simulation runs and no orders go out.
//! The game and the editor have their own buttons (`Program`).
//! The buttons and sliders come from `widgets.rs` (shared with the home screen).

use macroquad::prelude::*;

use crate::app::Program;
use crate::keymap::KeyMap;
use crate::settings::{self, Settings, WindowMode};
use crate::widgets::{button, choice, slider, toggle, Column, GAP};

const WIDTH: f32 = 460.0;
/// The settings page: two columns side by side (`settings_page`).
pub const SETTINGS_WIDTH: f32 = 2.0 * WIDTH;

/// What the player triggered in the menu – `game.rs` carries it out.
pub enum MenuAction {
    Nothing,
    /// Game: the same map from the start. Editor: back to the map as it was saved ("Revert").
    Restart,
    /// Game: a new map with the same setup and a new seed.
    NewMap,
    /// Editor: save the map.
    Save,
    /// Game: back to the home screen. Editor: close the map (to the editor's home screen).
    MainMenu,
    Quit,
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Main,
    Settings,
    Keys,
}

pub struct Menu {
    pub open: bool,
    page: Page,
    program: Program,
    /// The page "Key mapping" – kept, so it opens again on the tab it was left on.
    keymap: KeyMap,
}

impl Menu {
    pub fn new(program: Program) -> Menu {
        Menu { open: false, page: Page::Main, program, keymap: KeyMap::new() }
    }

    /// Esc: open the menu – inside the menu one page back – on the first page close it. While a new key is being
    /// set, Esc only stops that.
    pub fn escape(&mut self) {
        if !self.open {
            self.open = true;
            self.page = Page::Main;
        } else if self.page == Page::Keys && self.keymap.escape() {
            // Stayed on the page.
        } else if self.page != Page::Main {
            self.page = Page::Main;
        } else {
            self.open = false;
        }
    }

    /// Draws the menu and evaluates the clicks. Settings are changed directly in `settings`.
    pub fn show(&mut self, settings: &mut Settings) -> MenuAction {
        if !self.open {
            return MenuAction::Nothing;
        }
        // Darken the game behind it.
        draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.55));
        match self.page {
            Page::Main => self.main_page(),
            Page::Settings => {
                let mut col = Column::centered("Settings", 13, SETTINGS_WIDTH);
                if settings_page(&mut col, settings) {
                    self.page = Page::Main;
                }
                MenuAction::Nothing
            }
            Page::Keys => {
                if self.keymap.show(&mut settings.keys) {
                    self.page = Page::Main;
                }
                MenuAction::Nothing
            }
        }
    }

    fn main_page(&mut self) -> MenuAction {
        let editor = self.program == Program::Editor;
        let mut col = Column::centered(if editor { "Editor menu" } else { "Game menu" }, 9, WIDTH);
        if button(col.row(), "Resume", true) {
            self.open = false;
        }
        if button(col.row(), "Settings", true) {
            self.page = Page::Settings;
        }
        if button(col.row(), "Key mapping", true) {
            self.page = Page::Keys;
        }
        // `bool::then_some`: `Some(action)` if the button was clicked, otherwise `None`. Every `button` call is made (each
        // one draws its button), `or` keeps the first `Some`.
        let mut clicked = if editor {
            let save = button(col.row(), "Save map (Ctrl+S)", true).then_some(MenuAction::Save);
            let revert = button(col.row(), "Revert to saved", true).then_some(MenuAction::Restart);
            save.or(revert)
        } else {
            let restart = button(col.row(), "Restart (same map)", true).then_some(MenuAction::Restart);
            let new_map = button(col.row(), "New map (same setup, new seed)", true).then_some(MenuAction::NewMap);
            button(col.row(), "Save / Load game (coming later)", false);
            restart.or(new_map)
        };
        col.gap();
        let leave = if editor { "Close map" } else { "Main menu" };
        clicked = clicked.or(button(col.row(), leave, true).then_some(MenuAction::MainMenu));
        clicked = clicked.or(button(col.row(), if editor { "Quit editor" } else { "Quit game" }, true).then_some(MenuAction::Quit));
        match clicked {
            Some(action) => {
                self.open = false;
                action
            }
            None => MenuAction::Nothing,
        }
    }
}

/// The settings, as rows in `col` – the same page in the Esc menu and on the home screen: on the left controls and
/// display, on the right TAB's seven volumes. `col` is as wide as both halves (`SETTINGS_WIDTH`). Returns `true` when
/// "Back" was clicked.
pub fn settings_page(col: &mut Column, s: &mut Settings) -> bool {
    let half = (col.width - 2.0 * GAP) / 2.0;
    let mut right = Column { x: col.x + half + 2.0 * GAP, y: col.y, width: half };
    col.width = half;
    right.heading("Sound");
    let v = &mut s.volume;
    // The master volume turns everything down at once – each kind below is a share of it.
    slider(right.row(), "Master volume", &mut v.global, 0.0, 100.0);
    right.gap();
    for (label, value) in [
        ("Music", &mut v.music),
        ("Effects", &mut v.effects),
        ("Unit voices", &mut v.voices),
        ("Alerts", &mut v.notifications),
        ("Surroundings", &mut v.ambient),
        ("Interface", &mut v.interface),
    ] {
        slider(right.row(), label, value, 0.0, 100.0);
    }
    col.heading("Controls");
    toggle(col.row(), "Scroll at the window edge", &mut s.edge_scroll);
    slider(col.row(), "Scroll speed", &mut s.scroll_speed, 300.0, 2500.0);
    toggle(col.row(), "Keep mouse in window (F12)", &mut s.confine_mouse);

    col.heading("Display");
    let mode = match s.window_mode {
        WindowMode::Window => "Window",
        WindowMode::Borderless => "Borderless",
    };
    if choice(col.row(), "Window mode", mode) {
        s.window_mode = match s.window_mode {
            WindowMode::Window => WindowMode::Borderless,
            WindowMode::Borderless => WindowMode::Window,
        };
    }
    if choice(col.row(), "Health bars", s.health_bars.name()) {
        s.health_bars = s.health_bars.next();
    }
    toggle(col.row(), "Always show the whole command queue", &mut s.always_full_queue);
    toggle(col.row(), "Show the grid", &mut s.show_grid);
    toggle(col.row(), "Stats in the top left (F3)", &mut s.dev_overlay);

    col.gap();
    if button(col.row(), "Open settings folder", true) {
        settings::open_folder();
    }
    let back = button(col.row(), "Back", true);
    col.note("Changes apply right away and are saved.");
    back
}
