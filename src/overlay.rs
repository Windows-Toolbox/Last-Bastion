//! The overlay in the top left (F3): measurements, switches and the key help – in sections that fold open and shut
//! with a click on their title. Which sections are folded is kept in the settings (`overlay_folded`), so it stays that
//! way after a restart.
//!
//! Immediate mode like `widgets.rs`: `game.rs` builds the overlay anew every frame from what it knows, then asks it
//! what was clicked (`click`) and draws it (`draw`). Both walk the same rows (`rows`) – what you see is what you click.

use macroquad::prelude::*;

use crate::settings::Settings;
use crate::widgets::{mouse, ACCENT, DIM, HOVER};

/// Its corner: this far in from the corner of the map's part of the window (`view.rs`).
const LEFT: f32 = 8.0;
const TOP: f32 = 8.0;
const LINE: f32 = 20.0;
const SIZE: u16 = 20;
const PAD: f32 = 8.0;

/// What a click on a row does.
#[derive(Clone, Copy, PartialEq)]
pub enum Action {
    /// Fold the section with this key open or shut.
    Fold(&'static str),
    /// Switch a setting or the pause.
    Switch(Switch),
    /// Copy the seed to the clipboard (to play the map again: `--seed`).
    CopySeed,
}

/// The switches in the overlay – `game.rs` knows where each one lives.
#[derive(Clone, Copy, PartialEq)]
pub enum Switch {
    Pause,
    EdgeScroll,
    ConfineMouse,
    HealthBars,
    FullQueue,
    Grid,
}

/// One line: its text, what a click does (`None`: only text), and for a switch whether it's on.
struct Row {
    text: String,
    action: Option<Action>,
    on: Option<bool>,
}

/// A section: a key (in the settings file), the title, the rows below it.
struct Section {
    key: &'static str,
    title: &'static str,
    rows: Vec<Row>,
}

/// What a visible line is: a section title or a row – borrowed from the overlay, for one frame.
enum Line<'a> {
    Title(&'a Section, bool),
    Row(&'a Row),
}

pub struct Overlay {
    sections: Vec<Section>,
}

impl Overlay {
    pub fn new() -> Overlay {
        Overlay { sections: Vec::new() }
    }

    /// A new section – the rows added after it go into it. Returns `&mut Self`, so the calls can be chained.
    pub fn section(&mut self, key: &'static str, title: &'static str) -> &mut Self {
        self.sections.push(Section { key, title, rows: Vec::new() });
        self
    }

    /// A row of text.
    pub fn text(&mut self, text: impl Into<String>) -> &mut Self {
        self.push(Row { text: text.into(), action: None, on: None })
    }

    /// A row that does something when clicked.
    pub fn button(&mut self, text: impl Into<String>, action: Action) -> &mut Self {
        self.push(Row { text: text.into(), action: Some(action), on: None })
    }

    /// A switch: `[x]` or `[ ]` in front, a click flips it.
    pub fn switch(&mut self, text: &str, on: bool, switch: Switch) -> &mut Self {
        self.push(Row { text: text.to_owned(), action: Some(Action::Switch(switch)), on: Some(on) })
    }

    fn push(&mut self, row: Row) -> &mut Self {
        if let Some(section) = self.sections.last_mut() {
            section.rows.push(row);
        }
        self
    }

    /// Every visible line with its text, in order: each title, and below it the rows if the section is open.
    fn lines(&self, settings: &Settings) -> Vec<(Line<'_>, String)> {
        let mut lines = Vec::new();
        for section in &self.sections {
            let folded = is_folded(settings, section.key);
            let sign = if folded { "+" } else { "-" };
            lines.push((Line::Title(section, folded), format!("[{sign}] {}", section.title)));
            if !folded {
                for row in &section.rows {
                    let text = match row.on {
                        Some(true) => format!("    [x] {}", row.text),
                        Some(false) => format!("    [ ] {}", row.text),
                        None => format!("    {}", row.text),
                    };
                    lines.push((Line::Row(row), text));
                }
            }
        }
        lines
    }

    /// The lines with their rectangles on the screen – all as wide as the longest one.
    fn rows(&self, settings: &Settings) -> Vec<(Rect, Line<'_>, String)> {
        let lines = self.lines(settings);
        let width = lines.iter().map(|(_, text)| measure_text(text, None, SIZE, 1.0).width).fold(0.0, f32::max) + 2.0 * PAD;
        let corner = corner();
        lines
            .into_iter()
            .enumerate()
            .map(|(i, (line, text))| (Rect::new(corner.x, corner.y + PAD / 2.0 + i as f32 * LINE, width, LINE), line, text))
            .collect()
    }

    /// The box the overlay covers – clicks there are for the overlay, not for the map.
    pub fn area(&self, settings: &Settings) -> Rect {
        let rows = self.rows(settings);
        let width = rows.first().map_or(0.0, |(r, _, _)| r.w);
        let corner = corner();
        Rect::new(corner.x, corner.y, width, rows.len() as f32 * LINE + PAD)
    }

    /// What was clicked in this frame – `None` if nothing (or nothing that does something).
    pub fn click(&self, settings: &Settings) -> Option<Action> {
        if !is_mouse_button_pressed(MouseButton::Left) {
            return None;
        }
        let at = mouse();
        self.rows(settings).into_iter().find(|(r, _, _)| r.contains(at)).and_then(|(_, line, _)| match line {
            Line::Title(section, _) => Some(Action::Fold(section.key)),
            Line::Row(row) => row.action,
        })
    }

    pub fn draw(&self, settings: &Settings) {
        let area = self.area(settings);
        draw_rectangle(area.x, area.y, area.w, area.h, Color::new(0.0, 0.0, 0.0, 0.6));
        let at = mouse();
        for (r, line, text) in self.rows(settings) {
            let clickable = match line {
                Line::Title(..) => true,
                Line::Row(row) => row.action.is_some(),
            };
            if clickable && r.contains(at) {
                draw_rectangle(r.x, r.y, r.w, r.h, HOVER);
            }
            let color = match line {
                Line::Title(_, folded) => if folded { DIM } else { ACCENT },
                Line::Row(_) => WHITE,
            };
            draw_text(&text, r.x + PAD, r.y + LINE - 5.0, SIZE as f32, color);
        }
    }
}

/// The overlay's top left corner on the screen. In the editor (the map doesn't start at the window's top) a bit lower:
/// the bar "Sight" floats in the map's corner there.
fn corner() -> Vec2 {
    let view = crate::view::rect();
    let below_bar = if view.y > 0.0 { 40.0 } else { 0.0 };
    view.point() + vec2(LEFT, TOP + below_bar)
}

fn is_folded(settings: &Settings, key: &str) -> bool {
    settings.overlay_folded.iter().any(|k| k == key)
}

/// Folds the section open if it's shut, shut if it's open.
pub fn toggle_fold(settings: &mut Settings, key: &str) {
    match settings.overlay_folded.iter().position(|k| k == key) {
        Some(i) => {
            settings.overlay_folded.remove(i);
        }
        None => settings.overlay_folded.push(key.to_owned()),
    }
}
