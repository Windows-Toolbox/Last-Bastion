//! The building blocks of the menus (Esc menu, home screen, setup screen): buttons, switches, sliders – drawn
//! "immediate mode": every frame anew, and the same call that draws a button tells whether it was just clicked.
//! There are no button objects to keep track of – `if button(…) { … }` is the whole thing.
//! (Moved here from `menu.rs`, so the home screen can use them too.)

use macroquad::prelude::*;

use crate::mapgen::Field;

pub const ROW: f32 = 36.0;
pub const GAP: f32 = 6.0;

pub const BG: Color = Color::new(0.08, 0.09, 0.11, 0.97);
pub const BOX: Color = Color::new(0.15, 0.16, 0.20, 1.0);
pub const HOVER: Color = Color::new(0.22, 0.24, 0.30, 1.0);
pub const BORDER: Color = Color::new(0.30, 0.32, 0.38, 1.0);
pub const DIM: Color = Color::new(0.55, 0.57, 0.62, 1.0);
pub const ON: Color = Color::new(0.30, 0.65, 0.35, 1.0);
/// The picked one of a group of buttons.
pub const LIT: Color = Color::new(0.25, 0.35, 0.20, 1.0);
pub const ACCENT: Color = Color::new(0.95, 0.62, 0.20, 1.0);

/// A column of rows: every call of `row()` gives the rectangle of the next row.
pub struct Column {
    pub x: f32,
    pub y: f32,
    pub width: f32,
}

impl Column {
    /// A box in the middle of the screen with a title. `rows`: roughly how many rows will come (for the height).
    pub fn centered(title: &str, rows: usize, width: f32) -> Column {
        let h = 60.0 + rows as f32 * (ROW + GAP) + 40.0;
        let x = (screen_width() - width) / 2.0;
        let y = ((screen_height() - h) / 2.0).max(10.0);
        Column::boxed(title, Rect::new(x, y, width, h))
    }

    /// A box at the given place with a title.
    pub fn boxed(title: &str, r: Rect) -> Column {
        draw_rectangle(r.x, r.y, r.w, r.h, BG);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, BORDER);
        draw_text(title, r.x + 20.0, r.y + 38.0, 30.0, WHITE);
        Column { x: r.x + 20.0, y: r.y + 60.0, width: r.w - 40.0 }
    }

    pub fn row(&mut self) -> Rect {
        let r = Rect::new(self.x, self.y, self.width, ROW);
        self.y += ROW + GAP;
        r
    }

    /// A lower row of this height (sliders in long lists) – squeezed together when a list is long and the window low.
    pub fn row_of(&mut self, height: f32) -> Rect {
        let r = Rect::new(self.x, self.y, self.width, height);
        self.y += height + 3.0;
        r
    }

    /// The next row, split into `n` equal parts side by side.
    pub fn split(&mut self, n: usize) -> Vec<Rect> {
        let r = self.row();
        let w = (r.w - GAP * (n - 1) as f32) / n as f32;
        (0..n).map(|k| Rect::new(r.x + k as f32 * (w + GAP), r.y, w, r.h)).collect()
    }

    pub fn gap(&mut self) {
        self.y += ROW / 2.0;
    }

    pub fn heading(&mut self, text: &str) {
        draw_text(text, self.x, self.y + 22.0, 22.0, DIM);
        self.y += 30.0;
    }

    pub fn note(&mut self, text: &str) {
        draw_text(text, self.x, self.y + 16.0, 16.0, DIM);
        self.y += 22.0;
    }

    /// A note as long as it needs to be – broken into lines as wide as the column (texts from `data\`, which can be
    /// any length).
    pub fn wrap(&mut self, text: &str) {
        let mut line = String::new();
        for word in text.split_whitespace() {
            let longer = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if !line.is_empty() && measure_text(&longer, None, 16, 1.0).width > self.width {
                self.note(&line);
                line = word.to_owned();
            } else {
                line = longer;
            }
        }
        if !line.is_empty() {
            self.note(&line);
        }
    }
}

pub fn mouse() -> Vec2 {
    Vec2::from(mouse_position())
}

/// A button. Returns `true` if it was clicked in this frame.
pub fn button(r: Rect, label: &str, enabled: bool) -> bool {
    button_lit(r, label, enabled, false)
}

/// A button that can be lit (the chosen one of a group).
pub fn button_lit(r: Rect, label: &str, enabled: bool, lit: bool) -> bool {
    let hover = enabled && r.contains(mouse());
    let bg = if lit {
        LIT
    } else if hover {
        HOVER
    } else {
        BOX
    };
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, if lit { GREEN } else { BORDER });
    let w = measure_text(label, None, 20, 1.0).width;
    draw_text(label, r.x + (r.w - w) / 2.0, r.y + r.h / 2.0 + 6.0, 20.0, if enabled { WHITE } else { DIM });
    hover && is_mouse_button_pressed(MouseButton::Left)
}

/// One of a list to pick from (the maps of a new game): its name on the left, a detail on the right (what it's worth) –
/// lit when it's the picked one. Returns `true` if it was clicked in this frame.
pub fn pick(r: Rect, label: &str, detail: &str, lit: bool) -> bool {
    let hover = r.contains(mouse());
    let bg = if lit {
        LIT
    } else if hover {
        HOVER
    } else {
        BOX
    };
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, if lit { GREEN } else { BORDER });
    draw_text(label, r.x + 12.0, r.y + r.h / 2.0 + 6.0, 20.0, WHITE);
    let w = measure_text(detail, None, 18, 1.0).width;
    draw_text(detail, r.right() - w - 12.0, r.y + r.h / 2.0 + 6.0, 18.0, DIM);
    hover && is_mouse_button_pressed(MouseButton::Left)
}

/// Text on the left, a box "On"/"Off" on the right. A click on the row switches.
pub fn toggle(r: Rect, label: &str, value: &mut bool) {
    let hover = r.contains(mouse());
    if hover && is_mouse_button_pressed(MouseButton::Left) {
        *value = !*value;
    }
    if hover {
        draw_rectangle(r.x, r.y, r.w, r.h, HOVER);
    }
    draw_text(label, r.x + 8.0, r.y + 24.0, 20.0, WHITE);
    let b = Rect::new(r.right() - 70.0, r.y + 6.0, 62.0, r.h - 12.0);
    draw_rectangle(b.x, b.y, b.w, b.h, if *value { ON } else { BOX });
    draw_rectangle_lines(b.x, b.y, b.w, b.h, 1.0, BORDER);
    let text = if *value { "On" } else { "Off" };
    let w = measure_text(text, None, 18, 1.0).width;
    draw_text(text, b.x + (b.w - w) / 2.0, b.y + 17.0, 18.0, WHITE);
}

/// Text on the left, a bar on the right. While the mouse is held down on the bar, the value follows it.
pub fn slider(r: Rect, label: &str, value: &mut f32, min: f32, max: f32) {
    draw_text(label, r.x + 8.0, r.y + 24.0, 20.0, WHITE);
    let bar = Rect::new(r.right() - 200.0, r.y + 12.0, 192.0, r.h - 24.0);
    if bar.contains(mouse()) && is_mouse_button_down(MouseButton::Left) {
        let t = ((mouse().x - bar.x) / bar.w).clamp(0.0, 1.0);
        *value = min + t * (max - min);
    }
    let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);
    draw_rectangle(bar.x, bar.y, bar.w, bar.h, BOX);
    draw_rectangle(bar.x, bar.y, bar.w * t, bar.h, ON);
    draw_rectangle_lines(bar.x, bar.y, bar.w, bar.h, 1.0, BORDER);
    draw_text(&format!("{:.0}", *value), bar.x - 50.0, r.y + 24.0, 18.0, DIM);
}

/// The slider of a map generator value (`mapgen::Field`): label, bar, value as text. Click or drag on the bar, or
/// the small `<` / `>` at its ends for one step. Returns `true` if the value changed, and the help text while the
/// mouse is on the row.
pub fn field_slider(r: Rect, field: &mut Field) -> (bool, Option<&'static str>) {
    let before = *field.value;
    let hover = r.contains(mouse());
    if hover {
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(1.0, 1.0, 1.0, 0.04));
    }
    draw_text(field.label, r.x + 6.0, r.y + r.h / 2.0 + 6.0, 19.0, WHITE);
    let value_w = 92.0;
    let step_w = 18.0;
    let bar = Rect::new(r.x + r.w * 0.47, r.y + 8.0, r.w * 0.53 - value_w - 2.0 * step_w - 8.0, r.h - 16.0);
    let less = Rect::new(bar.x - step_w - 2.0, r.y + 4.0, step_w, r.h - 8.0);
    let more = Rect::new(bar.right() + 2.0, r.y + 4.0, step_w, r.h - 8.0);
    if bar.contains(mouse()) && is_mouse_button_down(MouseButton::Left) {
        field.set_fraction((mouse().x - bar.x) / bar.w);
    }
    if is_mouse_button_pressed(MouseButton::Left) {
        let step = field.step / (field.max - field.min);
        if less.contains(mouse()) {
            field.set_fraction(field.fraction() - step);
        } else if more.contains(mouse()) {
            field.set_fraction(field.fraction() + step);
        }
    }
    for (b, text) in [(less, "<"), (more, ">")] {
        let lit = b.contains(mouse());
        draw_rectangle(b.x, b.y, b.w, b.h, if lit { HOVER } else { BOX });
        draw_text(text, b.x + 5.0, b.y + b.h / 2.0 + 5.0, 18.0, WHITE);
    }
    draw_rectangle(bar.x, bar.y, bar.w, bar.h, BOX);
    draw_rectangle(bar.x, bar.y, bar.w * field.fraction(), bar.h, ON);
    draw_rectangle_lines(bar.x, bar.y, bar.w, bar.h, 1.0, BORDER);
    draw_text(&field.text(), r.right() - value_w + 4.0, r.y + r.h / 2.0 + 6.0, 18.0, WHITE);
    (*field.value != before, hover.then_some(field.help))
}

/// Text on the left, the current value on the right. Returns `true` if clicked (then pick the next value).
pub fn choice(r: Rect, label: &str, current: &str) -> bool {
    let hover = r.contains(mouse());
    if hover {
        draw_rectangle(r.x, r.y, r.w, r.h, HOVER);
    }
    draw_text(label, r.x + 8.0, r.y + 24.0, 20.0, WHITE);
    let b = Rect::new(r.right() - 110.0, r.y + 6.0, 102.0, r.h - 12.0);
    draw_rectangle(b.x, b.y, b.w, b.h, BOX);
    draw_rectangle_lines(b.x, b.y, b.w, b.h, 1.0, BORDER);
    let w = measure_text(current, None, 18, 1.0).width;
    draw_text(current, b.x + (b.w - w) / 2.0, b.y + 17.0, 18.0, WHITE);
    hover && is_mouse_button_pressed(MouseButton::Left)
}

/// A text field: shows `text`; while `active` with a blinking line at the end. Returns `true` if it was clicked.
pub fn text_box(r: Rect, label: &str, text: &str, active: bool) -> bool {
    let hover = r.contains(mouse());
    draw_rectangle(r.x, r.y, r.w, r.h, if hover || active { HOVER } else { BOX });
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, if active { GREEN } else { BORDER });
    draw_text(label, r.x + 8.0, r.y + r.h / 2.0 + 6.0, 19.0, DIM);
    let cursor = if active && (get_time() * 2.0) as i64 % 2 == 0 { "|" } else { "" };
    let lw = measure_text(label, None, 19, 1.0).width;
    draw_text(&format!("{text}{cursor}"), r.x + lw + 18.0, r.y + r.h / 2.0 + 6.0, 20.0, WHITE);
    hover && is_mouse_button_pressed(MouseButton::Left)
}

/// Typing into a text field, every frame while it's active: letters (as the keyboard layout means them – ä, ß,
/// Y on a German keyboard), Backspace. `allowed`: which characters may go in. Returns `Some(true)` on Enter,
/// `Some(false)` on Esc, `None` while typing goes on.
pub fn type_into(text: &mut String, max: usize, allowed: impl Fn(char) -> bool) -> Option<bool> {
    while let Some(c) = get_char_pressed() {
        if !c.is_control() && allowed(c) && text.chars().count() < max {
            text.push(c);
        }
    }
    if is_key_pressed(KeyCode::Backspace) {
        text.pop();
    }
    if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
        Some(true)
    } else if is_key_pressed(KeyCode::Escape) {
        Some(false)
    } else {
        None
    }
}

/// A tooltip box above the point `at` (the mouse), kept on the screen.
pub fn tooltip(text: &str, at: Vec2) {
    let w = measure_text(text, None, 17, 1.0).width + 16.0;
    let x = (at.x + 14.0).min(screen_width() - w - 4.0);
    let y = (at.y - 34.0).max(4.0);
    draw_rectangle(x, y, w, 26.0, Color::new(0.05, 0.05, 0.07, 0.95));
    draw_rectangle_lines(x, y, w, 26.0, 1.0, BORDER);
    draw_text(text, x + 8.0, y + 18.0, 17.0, WHITE);
}
