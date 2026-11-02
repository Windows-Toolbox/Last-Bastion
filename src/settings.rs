//! The player's settings – a JSON file at `%APPDATA%\Last Bastion\settings.json` (DESIGN.md 11.4).
//! Other programs (e.g. the Windows toolbox) can read and change it.
//! A broken or missing file never crashes the game: then the defaults apply.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::keys::Bindings;

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")] // in the file: "window" / "borderless"
pub enum WindowMode {
    Window,
    /// Borderless fullscreen on the main monitor – macroquad can't do real fullscreen (DESIGN.md 12.2).
    Borderless,
}

/// `#[serde(default)]`: if a value is missing in the file, it comes from `Default` below.
/// Unknown values in the file are simply skipped.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub window_mode: WindowMode,
    /// Move the camera when the mouse is at the window edge.
    pub edge_scroll: bool,
    /// Pixels per second, for the edge and the arrow keys.
    pub scroll_speed: f32,
    /// Hold the mouse inside the window while the game is in front (like StarCraft 2).
    pub confine_mouse: bool,
    /// Health bars above all units – otherwise only above hurt and selected ones.
    pub always_health_bars: bool,
    /// Lines through the whole command queue of the selected units. Off: only the current target, the whole
    /// queue only while Shift is held (like StarCraft 2).
    pub always_full_queue: bool,
    /// The lines between the tiles on the ground (F4, the button next to the pause button).
    pub show_grid: bool,
    /// Stats and key help in the top left (F3).
    pub dev_overlay: bool,
    /// The sections of that overlay that are folded shut (`overlay.rs`) – by their key.
    pub overlay_folded: Vec<String>,
    /// Which key does what (`keys.rs`) – only the changed ones are in the file.
    pub keys: Bindings,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            window_mode: WindowMode::Window,
            edge_scroll: true,
            scroll_speed: 900.0,
            confine_mouse: true,
            always_health_bars: false,
            always_full_queue: true,
            show_grid: false,
            dev_overlay: true,
            // At first only the measurements and the switches are open – the key help is there, one click away.
            overlay_folded: ["select", "commands", "buildings", "groups", "keys"].map(String::from).to_vec(),
            keys: Bindings::default(),
        }
    }
}

impl Settings {
    /// Reads the file. Missing or broken: defaults.
    pub fn load() -> Settings {
        let Some(path) = file_path() else {
            return Settings::default();
        };
        match fs::read_to_string(&path) {
            Ok(text) => Settings::parse(&text),
            Err(_) => Settings::default(),
        }
    }

    /// JSON text → settings. Broken text: defaults.
    fn parse(text: &str) -> Settings {
        serde_json::from_str(text).unwrap_or_default()
    }

    /// Writes the file (and creates the folder if it's missing). Errors are not a big deal:
    /// then the settings simply only last until the game ends.
    pub fn save(&self) {
        let (Some(dir), Some(path)) = (folder(), file_path()) else {
            return;
        };
        let Ok(text) = serde_json::to_string_pretty(self) else {
            return;
        };
        if fs::create_dir_all(&dir).and_then(|_| fs::write(&path, text)).is_err() {
            eprintln!("Could not save the settings: {}", path.display());
        }
    }

    /// Changed since `before` (one frame earlier)? Then apply it right away and save it (DESIGN.md 11.4).
    pub fn apply_changes(&self, before: &Settings) {
        if self != before {
            if self.window_mode != before.window_mode {
                macroquad::window::set_fullscreen(self.window_mode == WindowMode::Borderless);
            }
            self.save();
        }
    }
}

/// `%APPDATA%\Last Bastion` – `None` if Windows doesn't tell us `APPDATA`.
pub fn folder() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|appdata| PathBuf::from(appdata).join("Last Bastion"))
}

fn file_path() -> Option<PathBuf> {
    folder().map(|dir| dir.join("settings.json"))
}

/// Opens the folder in Explorer (DESIGN.md 11.4: button "Open settings folder").
pub fn open_folder() {
    if let Some(dir) = folder() {
        let _ = fs::create_dir_all(&dir);
        let _ = std::process::Command::new("explorer").arg(dir).spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Missing values come from `Default`, unknown ones are skipped (DESIGN.md 11.4).
    #[test]
    fn missing_values_use_defaults() {
        let s = Settings::parse(r#"{ "edge_scroll": false, "does_not_exist": 5 }"#);
        assert!(!s.edge_scroll);
        assert!(s.confine_mouse);
    }

    /// A broken file doesn't crash the game.
    #[test]
    fn broken_file_gives_defaults() {
        assert!(Settings::parse("{ broken") == Settings::default());
    }

    /// What gets saved comes back exactly the same when loading.
    #[test]
    fn save_and_load_round_trip() {
        let mut s = Settings::default();
        s.window_mode = WindowMode::Borderless;
        s.scroll_speed = 1234.0;
        let text = serde_json::to_string_pretty(&s).unwrap();
        assert!(text.contains(r#""window_mode": "borderless""#));
        assert!(Settings::parse(&text) == s);
    }

    /// A changed key is saved and read back with the rest.
    #[test]
    fn keys_round_trip() {
        use crate::keys::Action;
        use macroquad::prelude::KeyCode;
        let mut s = Settings::default();
        s.keys.set(Action::Pause, 0, Some(KeyCode::P));
        let text = serde_json::to_string_pretty(&s).unwrap();
        assert!(Settings::parse(&text).keys.has(Action::Pause, KeyCode::P));
    }
}
