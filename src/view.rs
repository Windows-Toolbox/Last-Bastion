//! The part of the window the map is shown in. In the game that's the whole window; in the editor only the room left
//! between its panels (menu and module bar at the top, tools on the left, properties on the right, the status line at
//! the bottom – `editor.rs`). Everything that asks "where is the screen?" for the map asks here instead of macroquad's
//! `screen_width()`: the camera's middle (`iso.rs`), the bar at the bottom with its minimap (`hud.rs`), scrolling at
//! the edge (`ui.rs`), the messages at the top.
//!
//! `thread_local!` with a `Cell`, like `hud.rs`'s `DRAWING`: one value for the whole program that can be changed
//! without `&mut` – the editor sets it once a frame, before anything reads it (`Game::frame`). In Python it would be a
//! module-level variable.

use std::cell::Cell;

use macroquad::prelude::*;

thread_local! {
    /// `None`: the whole window.
    static VIEW: Cell<Option<Rect>> = const { Cell::new(None) };
}

/// The map's part of the window, in screen pixels.
pub fn rect() -> Rect {
    VIEW.get().unwrap_or_else(|| Rect::new(0.0, 0.0, screen_width(), screen_height()))
}

/// Sets it – `None`: the whole window again (the game, or the editor while its window has no size).
pub fn set(rect: Option<Rect>) {
    VIEW.set(rect);
}

/// Its middle – the camera looks at it.
pub fn center() -> Vec2 {
    rect().center()
}
