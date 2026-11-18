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

/// Which units and buildings get a health bar above them.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")] // in the file: "always" / "damaged" / "selected" / "never"
pub enum HealthBars {
    /// Above everything.
    Always,
    /// Above the hurt and the selected ones.
    Damaged,
    /// Only above the selected ones.
    Selected,
    Never,
}

impl HealthBars {
    pub const ALL: [HealthBars; 4] = [HealthBars::Always, HealthBars::Damaged, HealthBars::Selected, HealthBars::Never];

    pub fn name(self) -> &'static str {
        match self {
            HealthBars::Always => "Always",
            HealthBars::Damaged => "Damaged",
            HealthBars::Selected => "Selected",
            HealthBars::Never => "Never",
        }
    }

    /// The next one in `ALL` – a click on the setting goes round them.
    pub fn next(self) -> HealthBars {
        let i = HealthBars::ALL.iter().position(|&b| b == self).unwrap_or(0);
        HealthBars::ALL[(i + 1) % HealthBars::ALL.len()]
    }

    /// Does something with this health (`hurt`) and selection get a bar?
    pub fn shows(self, hurt: bool, selected: bool) -> bool {
        match self {
            HealthBars::Always => true,
            HealthBars::Damaged => hurt || selected,
            HealthBars::Selected => selected,
            HealthBars::Never => false,
        }
    }
}

/// `#[serde(default)]`: if a value is missing in the file, it comes from `Default` below.
/// Unknown values in the file are simply skipped.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub window_mode: WindowMode,
    /// Which monitor the window is on, or the borderless one covers (`screen.rs`): 0 is the main one, then the others
    /// from left to right. One that isn't there: the main one.
    pub monitor: usize,
    /// Move the camera when the mouse is at the window edge.
    pub edge_scroll: bool,
    /// Pixels per second, for the edge and the arrow keys.
    pub scroll_speed: f32,
    /// Hold the mouse inside the window while the game is in front (like StarCraft 2).
    pub confine_mouse: bool,
    /// Which units and buildings show a health bar.
    pub health_bars: HealthBars,
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
    /// How loud (`audio.rs`) – TAB's seven sliders.
    pub volume: Volumes,
}

/// TAB's volumes (`ZXGameConfiguration`), in percent: everything, and each kind of sound on its own – each kind is
/// played at its own share of the whole.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Volumes {
    pub global: f32,
    pub music: f32,
    pub interface: f32,
    pub effects: f32,
    pub ambient: f32,
    pub voices: f32,
    pub notifications: f32,
}

impl Default for Volumes {
    fn default() -> Volumes {
        Volumes { global: 100.0, music: 100.0, interface: 100.0, effects: 100.0, ambient: 100.0, voices: 100.0, notifications: 100.0 }
    }
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            window_mode: WindowMode::Window,
            monitor: 0,
            edge_scroll: true,
            scroll_speed: 900.0,
            confine_mouse: true,
            health_bars: HealthBars::Damaged,
            always_full_queue: true,
            show_grid: false,
            dev_overlay: true,
            // At first only the measurements and the switches are open – the key help is there, one click away.
            overlay_folded: ["select", "commands", "buildings", "groups", "keys"].map(String::from).to_vec(),
            keys: Bindings::default(),
            volume: Volumes::default(),
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
            let borderless = self.window_mode == WindowMode::Borderless;
            if self.window_mode != before.window_mode {
                macroquad::window::set_fullscreen(borderless);
            }
            if self.window_mode != before.window_mode || self.monitor != before.monitor {
                crate::screen::set_target(borderless, self.monitor, true);
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

    /// The health bar setting goes round all four and is saved by name.
    #[test]
    fn health_bars_cycle_and_save() {
        let mut b = HealthBars::Damaged;
        for _ in 0..HealthBars::ALL.len() {
            b = b.next();
        }
        assert!(b == HealthBars::Damaged);
        let s = Settings::parse(r#"{ "health_bars": "never" }"#);
        assert!(s.health_bars == HealthBars::Never);
        assert!(HealthBars::Selected.shows(false, true) && !HealthBars::Selected.shows(true, false));
    }

    /// A changed key is saved and read back with the rest.
    #[test]
    fn keys_round_trip() {
        use crate::keys::Action;
        use macroquad::prelude::KeyCode;
        let mut s = Settings::default();
        s.keys.set(Action::Pause, 0, Some(KeyCode::P.into()));
        let text = serde_json::to_string_pretty(&s).unwrap();
        assert!(Settings::parse(&text).keys.has(Action::Pause, KeyCode::P.into()));
    }
}
