//! The page "Key mapping" (Esc menu and home screen, DESIGN.md 11.2): which key does what (`keys.rs`), in tabs like
//! the hotkeys of TAB's options. Click a key, press the new one – right click empties it. A key that's twice on one
//! card (or twice among the keys that work everywhere) turns red.
//!
//! The changes go straight into the settings and are saved like every other setting (`Settings::apply_changes`).

use macroquad::prelude::*;

use crate::hud::{self, Button, BARRACKS_BUTTONS, COMMAND_CENTER_BUTTONS, UNIT_BUTTONS};
use crate::kinds::BuildingKind;
use crate::keys::{self, Action, Bindings};
use crate::widgets::{button, button_lit, key_box, mouse, Column, BAD, DIM, GAP, ROW};

const WIDTH: f32 = 760.0;
/// The height of one line of the list.
const LINE: f32 = 30.0;
const KEY_W: f32 = 130.0;
const TABS: [&str; 5] = ["Camera and time", "Selection", "Units", "Command center", "Buildings"];

/// One line of the list: a heading between the actions, or an action with its two keys.
enum Line {
    Heading(String),
    Bind(Action),
}

pub struct KeyMap {
    tab: usize,
    /// How many lines of the list are scrolled past.
    scroll: usize,
    /// The key being set – which action, which of its two places. The next key pressed goes there.
    listening: Option<(Action, usize)>,
    /// While waiting: a key came that can't go on an action – said under the list, so the page doesn't just seem stuck.
    refused: bool,
}

impl KeyMap {
    pub fn new() -> KeyMap {
        KeyMap { tab: 0, scroll: 0, listening: None, refused: false }
    }

    /// Esc while a key is being set: only that stops. Returns `true` if Esc was used for it – then the page stays.
    pub fn escape(&mut self) -> bool {
        self.listening.take().is_some()
    }

    /// Draws the page and reacts to clicks and keys. The keys are changed directly in `keys`. Returns `true` when
    /// "Back" was clicked.
    pub fn show(&mut self, keys: &mut Bindings) -> bool {
        self.catch_key(keys);
        let h = (screen_height() - 20.0).min(820.0);
        let r = Rect::new((screen_width() - WIDTH) / 2.0, ((screen_height() - h) / 2.0).max(10.0), WIDTH, h);
        let mut col = Column::boxed("Key mapping", r);
        let clicked = is_mouse_button_pressed(MouseButton::Left) || is_mouse_button_pressed(MouseButton::Right);
        // A click anywhere but on a key stops the waiting for one (a click on another key waits there instead).
        let mut on_a_key = false;

        for (i, tab) in col.split(TABS.len()).into_iter().enumerate() {
            if button_lit(tab, TABS[i], true, self.tab == i) && self.tab != i {
                self.tab = i;
                self.scroll = 0;
            }
        }

        // The list between the tabs and the three lines at the bottom (what the mouse is on, the hint, the buttons).
        let lines = lines(self.tab);
        let top = col.y + 4.0;
        let bottom = r.bottom() - 20.0 - ROW - GAP - 2.0 * 22.0 - 8.0;
        let shown = (((bottom - top) / LINE).floor() as usize).max(1);
        let max_scroll = lines.len().saturating_sub(shown);
        let wheel = mouse_wheel().1;
        if wheel < 0.0 {
            self.scroll = (self.scroll + 3).min(max_scroll);
        } else if wheel > 0.0 {
            self.scroll = self.scroll.saturating_sub(3);
        }
        self.scroll = self.scroll.min(max_scroll);

        let key_x = [col.x + col.width - 2.0 * KEY_W - 8.0, col.x + col.width - KEY_W];
        let mut info = None;
        for (i, line) in lines.iter().enumerate().skip(self.scroll).take(shown) {
            let y = top + (i - self.scroll) as f32 * LINE;
            let action = match line {
                Line::Heading(text) => {
                    draw_text(text, col.x, y + 22.0, 20.0, DIM);
                    continue;
                }
                Line::Bind(action) => *action,
            };
            let clashes = keys.clashes(action);
            let row = Rect::new(col.x, y, col.width, LINE);
            if row.contains(mouse()) {
                draw_rectangle(row.x, row.y, row.w, row.h, Color::new(1.0, 1.0, 1.0, 0.04));
                info = Some(if clashes.is_empty() {
                    (format!("Default: {}", Bindings::default().label(action)), DIM)
                } else {
                    let names: Vec<String> = clashes.iter().map(|other| other.name()).collect();
                    (format!("Same key as: {}", names.join(", ")), BAD)
                });
            }
            draw_text(action.name(), col.x + 12.0, y + 21.0, 19.0, if clashes.is_empty() { WHITE } else { BAD });
            for (slot, x) in key_x.into_iter().enumerate() {
                let key = keys.keys(action)[slot];
                let waiting = self.listening == Some((action, slot));
                let text = match key {
                    _ if waiting => "press a key",
                    Some(key) => keys::name(key),
                    None => "",
                };
                // Red: only the key that's shared, not the other one.
                let clash = key.is_some_and(|key| clashes.iter().any(|&other| keys.has(other, key)));
                let (left, right) = key_box(Rect::new(x, y + 2.0, KEY_W, LINE - 4.0), text, waiting, clash);
                if left {
                    self.listening = Some((action, slot));
                    self.refused = false;
                    on_a_key = true;
                } else if right {
                    keys.set(action, slot, None);
                    self.listening = None;
                    on_a_key = true;
                }
            }
        }
        if clicked && !on_a_key {
            self.listening = None;
        }
        if max_scroll > 0 {
            let text = format!("{}-{} of {} lines (mouse wheel)", self.scroll + 1, (self.scroll + shown).min(lines.len()), lines.len());
            let w = measure_text(&text, None, 16, 1.0).width;
            draw_text(&text, r.right() - 20.0 - w, r.y + 38.0, 16.0, DIM);
        }

        col.y = bottom + 8.0;
        if self.listening.is_none() {
            self.refused = false;
        }
        let refused = "This key doesn't reach the game - in the mouse's software, put F13-F24 on such buttons.";
        match self.refused.then(|| (refused.to_owned(), BAD)).or(info) {
            Some((text, color)) => {
                draw_text(&text, col.x, col.y + 16.0, 16.0, color);
                col.y += 22.0;
            }
            None => col.note(""),
        }
        col.note("Click a key, then press the new one (or mouse 4/5). Right click: no key. Esc: cancel. Shift, Ctrl fixed.");
        let row = col.split(3);
        if button(row[0], "Defaults for this tab", true) {
            for line in &lines {
                if let Line::Bind(action) = line {
                    keys.reset(*action);
                }
            }
        }
        if button(row[1], "Defaults for all", true) {
            *keys = Bindings::default();
        }
        button(row[2], "Back", true)
    }

    /// While waiting for a key: the first one pressed that can be put on an action goes there – a key, or the mouse's
    /// side button 4 or 5. Shift, Ctrl, and keys macroquad doesn't know (`KeyCode::Unknown`, some media keys) can't – it
    /// says so and waits on (Esc: `escape`).
    fn catch_key(&mut self, keys: &mut Bindings) {
        let Some((action, slot)) = self.listening else {
            return;
        };
        match keys::caught() {
            Some(Ok(key)) => {
                keys.set(action, slot, Some(key));
                self.listening = None;
                self.refused = false;
            }
            Some(Err(())) => self.refused = true,
            None => {}
        }
    }
}

/// The lines of a tab.
fn lines(tab: usize) -> Vec<Line> {
    let mut lines = Vec::new();
    match tab {
        0 => {
            heading(&mut lines, "Camera");
            binds(&mut lines, [Action::CameraUp, Action::CameraLeft, Action::CameraDown, Action::CameraRight, Action::Base]);
            binds(&mut lines, (0..4).map(Action::Bookmark));
            heading(&mut lines, "Time");
            binds(&mut lines, [Action::Pause, Action::Faster, Action::Slower]);
            heading(&mut lines, "Window and tools");
            binds(&mut lines, [Action::Overlay, Action::GridView, Action::ConfineMouse, Action::EditTest]);
        }
        1 => {
            binds(&mut lines, [Action::SelectArmy, Action::NextType, Action::RepairAll]);
            heading(&mut lines, "Control groups");
            binds(&mut lines, (0..10).map(Action::Group));
        }
        2 => {
            // `partition`: two lists – the buttons that choose a formation, and the rest.
            let (shapes, commands): (Vec<Button>, Vec<Button>) =
                UNIT_BUTTONS.into_iter().chain([Button::Drop]).partition(|b| b.formation().is_some());
            heading(&mut lines, "Commands");
            binds(&mut lines, commands.into_iter().map(Action::of));
            heading(&mut lines, "Formations - then drag with the right mouse button");
            binds(&mut lines, shapes.into_iter().map(Action::of));
        }
        3 => {
            heading(&mut lines, "Command center");
            // The repair button is on every building – it's in the last tab.
            let main = COMMAND_CENTER_BUTTONS.into_iter().filter(|&b| b != Button::Repair).map(Action::of);
            // The map's own button (`sim/story.rs`) – only on maps whose story has one.
            binds(&mut lines, main.chain([Action::of(Button::Custom), Action::of(Button::Back), Action::TurnGate]));
            for menu in hud::build_menus() {
                let buildings: Vec<Button> = hud::menu_buttons(menu).into_iter().filter(|b| b.places().is_some()).collect();
                if !buildings.is_empty() {
                    heading(&mut lines, &format!("{} menu", Button::Open(menu).title()));
                    binds(&mut lines, buildings.into_iter().map(Action::of));
                }
            }
        }
        _ => {
            heading(&mut lines, "Barracks - and the inn: a mercenary has the key of its kind");
            let trains = BARRACKS_BUTTONS.into_iter().filter(|b| b.trains().is_some()).map(Action::of);
            binds(&mut lines, trains.chain([Action::of(Button::Rally)]));
            heading(&mut lines, "Engineering center");
            binds(&mut lines, hud::ENGINEERING_BUTTONS.into_iter().filter(|b| b.trains().is_some()).map(Action::of));
            for kind in [BuildingKind::WoodWorkshop, BuildingKind::StoneWorkshop, BuildingKind::Foundry] {
                heading(&mut lines, &format!("{} - research", kind.stats().name));
                binds(&mut lines, crate::sim::researched_in(kind).map(|t| Action::of(Button::Research(t))));
            }
            heading(&mut lines, "Other buildings");
            let others = [Button::Repair, Button::Switch, Button::Demolish, Button::Unload, Button::MakeBarrel, Button::Sell, Button::Scan, Button::Delay(0), Button::Delay(1), Button::Delay(2), Button::Delay(3)];
            binds(&mut lines, others.into_iter().map(Action::of));
        }
    }
    lines
}

fn heading(lines: &mut Vec<Line>, text: &str) {
    lines.push(Line::Heading(text.to_owned()));
}

/// `impl IntoIterator`: anything that can be gone through – an array, a range turned into actions, a chain of both.
fn binds(lines: &mut Vec<Line>, actions: impl IntoIterator<Item = Action>) {
    lines.extend(actions.into_iter().map(Line::Bind));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every action can be set on one of the tabs – exactly once.
    #[test]
    fn every_action_on_one_tab() {
        let mut listed = Vec::new();
        for tab in 0..TABS.len() {
            for line in lines(tab) {
                if let Line::Bind(action) = line {
                    assert!(!listed.contains(&action), "{action:?} twice");
                    listed.push(action);
                }
            }
        }
        for action in Action::all() {
            assert!(listed.contains(&action), "{action:?} on no tab");
        }
    }
}
