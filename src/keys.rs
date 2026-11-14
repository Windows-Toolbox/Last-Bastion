//! Which key does what – the key mapping (DESIGN.md 11.2). Set on the page "Key mapping" (Esc menu, home screen –
//! `keymap.rs`) and saved with the settings (`settings.json`, 11.4). Every action has up to two keys, like StarCraft 2's
//! first and second key. Only what differs from the defaults goes into the file: a better default in a new version
//! reaches you, unless you changed that key yourself.
//!
//! Not in here: **Shift** and **Ctrl** always mean the same (queue, on credit, 5 at once, set a group …), **Esc**
//! always cancels and opens the menu, **Ctrl+S** saves in the editor, **Ctrl+Z** / **Ctrl+Y** undo and redo there.
//!
//! macroquad reports a key by its place on the keyboard (that of a US one), not by its letter: `KeyCode::Z` is the key
//! left of X – on a German keyboard Y. The names shown (`name`) are those of a German keyboard.
//!
//! Mice with many buttons: the side buttons 4 and 5 can go on an action themselves (`Key::Mouse4`, asked from Windows
//! in `cursor.rs`). Every further button only reaches a game through the mouse's own software, which sends a key for
//! it – F13 to F24 are made for that (no keyboard has them, so they never clash), and they can all be put on actions.
//! A key macroquad doesn't know (some media keys) arrives as nothing at all; the page "Key mapping" says so.

use std::collections::{BTreeMap, HashMap};

use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

use crate::cursor;
use crate::hud::{self, Button};
use crate::kinds::UnitKind;

/// The keys of one action: the first and the second. `None`: no key in that place.
pub type Keys = [Option<Key>; 2];

/// What can go on an action: a key of the keyboard, or one of the mouse's side buttons.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Key {
    Board(KeyCode),
    /// The side buttons, usually "back" (4) and "forward" (5) – Windows' X buttons.
    Mouse4,
    Mouse5,
}

impl From<KeyCode> for Key {
    fn from(key: KeyCode) -> Key {
        Key::Board(key)
    }
}

impl Key {
    /// Pressed in this frame (the first press – holding doesn't repeat it)?
    pub fn pressed(self) -> bool {
        match self {
            Key::Board(key) => is_key_pressed(key),
            Key::Mouse4 => cursor::side_pressed(0),
            Key::Mouse5 => cursor::side_pressed(1),
        }
    }

    /// Held right now?
    pub fn down(self) -> bool {
        match self {
            Key::Board(key) => is_key_down(key),
            Key::Mouse4 => cursor::side_down(0),
            Key::Mouse5 => cursor::side_down(1),
        }
    }
}

/// The first key pressed in this frame that can be put on an action – for the page "Key mapping". `Err(())`: only a key
/// macroquad doesn't know came (`KeyCode::Unknown` – some media keys, odd buttons of a mouse). Shift, Ctrl and the like
/// are left out without a word.
pub fn caught() -> Option<Result<Key, ()>> {
    let pressed = get_keys_pressed();
    if let Some(&key) = pressed.iter().find(|&&key| bindable(key)) {
        return Some(Ok(Key::Board(key)));
    }
    for side in [Key::Mouse4, Key::Mouse5] {
        if side.pressed() {
            return Some(Ok(side));
        }
    }
    pressed.contains(&KeyCode::Unknown).then_some(Err(()))
}

/// Everything a key can do. The buttons of the command card are one action each (`Card`) – their keys are their
/// letters there.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Action {
    CameraUp,
    CameraLeft,
    CameraDown,
    CameraRight,
    /// Camera to the base (like StarCraft's jump to the main building).
    Base,
    /// Camera spot 0–3: jump there – with Ctrl: remember where the camera is.
    Bookmark(usize),
    Pause,
    Faster,
    Slower,
    /// The overlay in the top left: measurements, switches, key help.
    Overlay,
    /// The lines between the tiles on the ground, on or off.
    GridView,
    ConfineMouse,
    /// Editor: Edit ↔ Test.
    EditTest,
    SelectArmy,
    /// The next type in the selection – with Shift the one before.
    NextType,
    /// Control group 0–9 (keys 1 … 9, then 0).
    Group(usize),
    RepairAll,
    /// Turn the gate while it's on the mouse (like Factorio's R) – this key comes first then, before the card.
    TurnGate,
    /// A button of the command card.
    Card(Button),
}

use Action::*;

impl Action {
    /// The action of a card button. The mercenaries are on the keys of their kind in the barracks – one action.
    pub fn of(button: Button) -> Action {
        Card(match button {
            Button::Train(UnitKind::MercSoldier) => Button::Train(UnitKind::Soldier),
            Button::Train(UnitKind::MercRanger) => Button::Train(UnitKind::Ranger),
            Button::Train(UnitKind::MercSniper) => Button::Train(UnitKind::Sniper),
            other => other,
        })
    }

    /// The keys it has until you change them.
    pub fn defaults(self) -> Keys {
        let one = |key: KeyCode| [Some(key.into()), None];
        let two = |first: KeyCode, second: KeyCode| [Some(first.into()), Some(second.into())];
        match self {
            CameraUp => two(KeyCode::W, KeyCode::Up),
            CameraLeft => two(KeyCode::A, KeyCode::Left),
            CameraDown => two(KeyCode::S, KeyCode::Down),
            CameraRight => two(KeyCode::D, KeyCode::Right),
            Base => one(KeyCode::Backspace),
            Bookmark(b) => one([KeyCode::F5, KeyCode::F6, KeyCode::F7, KeyCode::F8][b]),
            Pause => one(KeyCode::Space),
            // On a German keyboard "+" is where a US one has "]", and "-" where it has "/".
            Faster => two(KeyCode::RightBracket, KeyCode::KpAdd),
            Slower => two(KeyCode::Slash, KeyCode::KpSubtract),
            Overlay => one(KeyCode::F3),
            // TAB's own key for its view with the grid (flat mode).
            GridView => one(KeyCode::F4),
            ConfineMouse => one(KeyCode::F12),
            EditTest => one(KeyCode::F10),
            SelectArmy => one(KeyCode::F2),
            NextType => one(KeyCode::Tab),
            Group(g) => one(GROUP_KEYS[g]),
            RepairAll => one(KeyCode::X),
            TurnGate => one(KeyCode::R),
            Card(button) => one(button.default_key()),
        }
    }

    /// Its name in `settings.json`: `"camera_up"`, `"group_1"`, `"stop"`, `"build_tent"`, `"train_soldier"` …
    fn id(self) -> String {
        match self {
            Bookmark(b) => format!("bookmark_{}", b + 1),
            Group(g) => format!("group_{}", (g + 1) % 10),
            Card(Button::Open(menu)) => format!("menu_{}", snake(&format!("{menu:?}"))),
            Card(Button::Place(kind)) => format!("build_{}", snake(&format!("{kind:?}"))),
            Card(Button::Train(kind)) => format!("train_{}", snake(&format!("{kind:?}"))),
            Card(Button::Delay(d)) => format!("mine_delay_{d}"),
            Card(Button::Research(t)) => {
                let id = &crate::sim::tech(t).id;
                format!("research_{}", snake(id.strip_prefix("Tech").unwrap_or(id)))
            }
            Card(button) => snake(&format!("{button:?}")),
            other => snake(&format!("{other:?}")),
        }
    }

    /// Its name on the page "Key mapping".
    pub fn name(self) -> String {
        match self {
            CameraUp => "Camera up".to_owned(),
            CameraLeft => "Camera left".to_owned(),
            CameraDown => "Camera down".to_owned(),
            CameraRight => "Camera right".to_owned(),
            Base => "Camera to the base".to_owned(),
            Bookmark(b) => format!("Camera spot {} (Ctrl: remember it)", b + 1),
            Pause => "Pause".to_owned(),
            Faster => "Faster".to_owned(),
            Slower => "Slower".to_owned(),
            Overlay => "Stats and key help (top left)".to_owned(),
            GridView => "Grid view (lines between the tiles)".to_owned(),
            ConfineMouse => "Keep the mouse in the window".to_owned(),
            EditTest => "Editor: Edit / Test".to_owned(),
            SelectArmy => "Select all units".to_owned(),
            NextType => "Next type in the selection (Shift: back)".to_owned(),
            Group(g) => format!("Group {} (Ctrl: set, Shift: add)", (g + 1) % 10),
            RepairAll => "Repair all buildings".to_owned(),
            TurnGate => "Turn the gate (while placing it)".to_owned(),
            Card(Button::Open(menu)) => format!("{} menu", Button::Open(menu).title()),
            Card(Button::Repair) => "Repair (every building)".to_owned(),
            Card(Button::Back) => "Back (out of a build menu)".to_owned(),
            Card(Button::Unload) => "Unload (tower)".to_owned(),
            Card(Button::MakeBarrel) => "Barrel (munitions factory)".to_owned(),
            Card(Button::Sell) => "Selling on/off (market)".to_owned(),
            Card(Button::Delay(d)) => format!("Delay {d} (mine trap)"),
            Card(Button::Research(t)) => format!("Research {}", crate::sim::tech(t).name.to_lowercase()),
            Card(Button::Switch) => "On/Off (buildings that run)".to_owned(),
            Card(Button::Demolish) => "Demolish (buildings; editor: remove)".to_owned(),
            Card(button) => button.title().to_owned(),
        }
    }

    /// Every action there is – for the file and to look for a key on a card (`on_any_card`). The planned buildings
    /// aren't in it: their buttons only show where they will be and keep the key of their spot.
    pub fn all() -> Vec<Action> {
        let mut all = vec![CameraUp, CameraLeft, CameraDown, CameraRight, Base];
        all.extend((0..4).map(Bookmark));
        all.extend([Pause, Faster, Slower, Overlay, GridView, ConfineMouse, EditTest, SelectArmy, NextType]);
        all.extend((0..10).map(Group));
        all.extend([RepairAll, TurnGate]);
        for button in hud::all_cards().into_iter().flatten() {
            let action = Action::of(button);
            if !matches!(button, Button::Planned(_)) && !all.contains(&action) {
                all.push(action);
            }
        }
        all
    }

    /// The actions that work everywhere, without a card – they may not share a key with each other. The gate isn't
    /// among them: its key only counts while the gate is on the mouse.
    fn global() -> Vec<Action> {
        Action::all().into_iter().filter(|a| !matches!(a, Card(_) | TurnGate)).collect()
    }
}

const GROUP_KEYS: [KeyCode; 10] = [
    KeyCode::Key1,
    KeyCode::Key2,
    KeyCode::Key3,
    KeyCode::Key4,
    KeyCode::Key5,
    KeyCode::Key6,
    KeyCode::Key7,
    KeyCode::Key8,
    KeyCode::Key9,
    KeyCode::Key0,
];

/// `"CameraUp"` → `"camera_up"`.
fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// The keys that can be put on an action, and their names – those of a German keyboard (see the top of the file).
/// Not in it: Shift, Ctrl, Alt and the Windows key (modifiers), Esc.
const NAMES: &[(KeyCode, &str)] = &[
    (KeyCode::A, "A"), (KeyCode::B, "B"), (KeyCode::C, "C"), (KeyCode::D, "D"), (KeyCode::E, "E"),
    (KeyCode::F, "F"), (KeyCode::G, "G"), (KeyCode::H, "H"), (KeyCode::I, "I"), (KeyCode::J, "J"),
    (KeyCode::K, "K"), (KeyCode::L, "L"), (KeyCode::M, "M"), (KeyCode::N, "N"), (KeyCode::O, "O"),
    (KeyCode::P, "P"), (KeyCode::Q, "Q"), (KeyCode::R, "R"), (KeyCode::S, "S"), (KeyCode::T, "T"),
    (KeyCode::U, "U"), (KeyCode::V, "V"), (KeyCode::W, "W"), (KeyCode::X, "X"),
    // Swapped on a German keyboard.
    (KeyCode::Y, "Z"), (KeyCode::Z, "Y"),
    (KeyCode::Key1, "1"), (KeyCode::Key2, "2"), (KeyCode::Key3, "3"), (KeyCode::Key4, "4"), (KeyCode::Key5, "5"),
    (KeyCode::Key6, "6"), (KeyCode::Key7, "7"), (KeyCode::Key8, "8"), (KeyCode::Key9, "9"), (KeyCode::Key0, "0"),
    (KeyCode::Space, "Space"), (KeyCode::Tab, "Tab"), (KeyCode::Enter, "Enter"), (KeyCode::Backspace, "Backspace"),
    (KeyCode::Insert, "Ins"), (KeyCode::Delete, "Del"), (KeyCode::Home, "Home"), (KeyCode::End, "End"),
    (KeyCode::PageUp, "PgUp"), (KeyCode::PageDown, "PgDn"),
    (KeyCode::Up, "Up"), (KeyCode::Down, "Down"), (KeyCode::Left, "Left"), (KeyCode::Right, "Right"),
    (KeyCode::CapsLock, "Caps"), (KeyCode::ScrollLock, "Scroll"), (KeyCode::NumLock, "Num"),
    (KeyCode::PrintScreen, "Print"), (KeyCode::Pause, "Break"), (KeyCode::Menu, "Menu"),
    (KeyCode::Minus, "ß"), (KeyCode::Equal, "´"), (KeyCode::LeftBracket, "Ü"), (KeyCode::RightBracket, "+"),
    (KeyCode::Semicolon, "Ö"), (KeyCode::Apostrophe, "Ä"), (KeyCode::Backslash, "#"), (KeyCode::GraveAccent, "^"),
    (KeyCode::Comma, ","), (KeyCode::Period, "."), (KeyCode::Slash, "-"), (KeyCode::World1, "<"),
    (KeyCode::F1, "F1"), (KeyCode::F2, "F2"), (KeyCode::F3, "F3"), (KeyCode::F4, "F4"), (KeyCode::F5, "F5"),
    (KeyCode::F6, "F6"), (KeyCode::F7, "F7"), (KeyCode::F8, "F8"), (KeyCode::F9, "F9"), (KeyCode::F10, "F10"),
    (KeyCode::F11, "F11"), (KeyCode::F12, "F12"),
    // No keyboard has these – the software of mice with many buttons sends them (top of the file).
    (KeyCode::F13, "F13"), (KeyCode::F14, "F14"), (KeyCode::F15, "F15"), (KeyCode::F16, "F16"), (KeyCode::F17, "F17"),
    (KeyCode::F18, "F18"), (KeyCode::F19, "F19"), (KeyCode::F20, "F20"), (KeyCode::F21, "F21"), (KeyCode::F22, "F22"),
    (KeyCode::F23, "F23"), (KeyCode::F24, "F24"),
    (KeyCode::Kp0, "Num 0"), (KeyCode::Kp1, "Num 1"), (KeyCode::Kp2, "Num 2"), (KeyCode::Kp3, "Num 3"),
    (KeyCode::Kp4, "Num 4"), (KeyCode::Kp5, "Num 5"), (KeyCode::Kp6, "Num 6"), (KeyCode::Kp7, "Num 7"),
    (KeyCode::Kp8, "Num 8"), (KeyCode::Kp9, "Num 9"), (KeyCode::KpDecimal, "Num ,"), (KeyCode::KpDivide, "Num /"),
    (KeyCode::KpMultiply, "Num *"), (KeyCode::KpSubtract, "Num -"), (KeyCode::KpAdd, "Num +"),
    (KeyCode::KpEnter, "Num Enter"), (KeyCode::KpEqual, "Num ="),
];

/// The name of a key as it's written on a German keyboard. `"?"`: one that can't be put on an action.
pub fn name(key: Key) -> &'static str {
    match key {
        Key::Board(key) => NAMES.iter().find(|(k, _)| *k == key).map_or("?", |(_, name)| name),
        Key::Mouse4 => "Mouse 4",
        Key::Mouse5 => "Mouse 5",
    }
}

/// Can this key be put on an action (not Shift, Ctrl, Esc …)?
fn bindable(key: KeyCode) -> bool {
    NAMES.iter().any(|(k, _)| *k == key)
}

/// A key as it's written in `settings.json`: macroquad's name for it (`"Z"`, `"Key1"`, `"RightBracket"`) – by its place
/// on a US keyboard, like macroquad reads it – or `"Mouse4"` / `"Mouse5"`.
fn file_name(key: Key) -> String {
    match key {
        Key::Board(key) => format!("{key:?}"),
        other => format!("{other:?}"),
    }
}

fn from_file_name(name: &str) -> Option<Key> {
    let mut all = NAMES.iter().map(|&(key, _)| Key::Board(key)).chain([Key::Mouse4, Key::Mouse5]);
    all.find(|&key| file_name(key) == name)
}

/// All keys: only the actions whose keys differ from the defaults are kept – that's also exactly what goes into the
/// file. `serde(from, into)`: in the file it's a plain list `"stop": ["Q", ""]` (`""`: no key in that place), read and
/// written through the two `From`s below.
#[derive(Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(from = "BTreeMap<String, Vec<String>>", into = "BTreeMap<String, Vec<String>>")]
pub struct Bindings {
    changed: HashMap<Action, Keys>,
}

impl Bindings {
    /// The keys of an action right now.
    pub fn keys(&self, action: Action) -> Keys {
        self.changed.get(&action).copied().unwrap_or_else(|| action.defaults())
    }

    /// Was one of its keys pressed in this frame?
    pub fn pressed(&self, action: Action) -> bool {
        self.keys(action).into_iter().flatten().any(Key::pressed)
    }

    /// Does `action` have this key (first or second)?
    pub fn has(&self, action: Action, key: Key) -> bool {
        self.keys(action).contains(&Some(key))
    }

    /// Its keys as text: `"Q"`, `"W / Up"`, `"-"` without any.
    pub fn label(&self, action: Action) -> String {
        let names: Vec<&str> = self.keys(action).into_iter().flatten().map(name).collect();
        if names.is_empty() { "-".to_owned() } else { names.join(" / ") }
    }

    /// Only its first key (or the second, if the first place is empty) – on a button of the card, where there's only
    /// room for one. `""` without any.
    pub fn short(&self, action: Action) -> &'static str {
        self.keys(action).into_iter().flatten().next().map_or("", name)
    }

    /// Puts `key` in place `slot` (0 or 1) of `action` – `None` empties it.
    pub fn set(&mut self, action: Action, slot: usize, key: Option<Key>) {
        let mut keys = self.keys(action);
        keys[slot] = key;
        self.set_keys(action, keys);
    }

    /// Back to its default keys.
    pub fn reset(&mut self, action: Action) {
        self.changed.remove(&action);
    }

    /// Keys the same as the defaults are not kept – so the file only holds what was really changed.
    fn set_keys(&mut self, action: Action, keys: Keys) {
        if keys == action.defaults() {
            self.changed.remove(&action);
        } else {
            self.changed.insert(action, keys);
        }
    }

    /// Is the key on any button of any card? Then it moves the camera only while nothing is selected (`ui.rs`) – a key
    /// that moves the camera with one selection and does something else with another one is a trap.
    pub fn on_any_card(&self, key: Key) -> bool {
        Action::all().into_iter().any(|a| matches!(a, Card(_)) && self.has(a, key))
    }

    /// The actions that share a key with `action` where both work at the same time: on one card, or both everywhere.
    /// A key that's on the card and also works everywhere is not a clash – the card comes first (X: the Line
    /// formation for units, otherwise "repair all").
    pub fn clashes(&self, action: Action) -> Vec<Action> {
        let mut groups: Vec<Vec<Action>> = vec![Action::global()];
        for card in hud::all_cards() {
            groups.push(card.into_iter().filter(|b| !matches!(b, Button::Planned(_))).map(Action::of).collect());
        }
        let mine: Vec<Key> = self.keys(action).into_iter().flatten().collect();
        let mut clashes = Vec::new();
        for group in groups.iter().filter(|g| g.contains(&action)) {
            for &other in group {
                if other != action && !clashes.contains(&other) && mine.iter().any(|&key| self.has(other, key)) {
                    clashes.push(other);
                }
            }
        }
        clashes
    }
}

/// Reading the file: start from the defaults, take every action whose name is known. An unknown action or key name
/// (from an older or newer version) is skipped – like the unknown values of the settings.
impl From<BTreeMap<String, Vec<String>>> for Bindings {
    fn from(file: BTreeMap<String, Vec<String>>) -> Bindings {
        let mut bindings = Bindings::default();
        for action in Action::all() {
            if let Some(names) = file.get(&action.id()) {
                let key = |slot: usize| names.get(slot).and_then(|name| from_file_name(name));
                bindings.set_keys(action, [key(0), key(1)]);
            }
        }
        bindings
    }
}

/// Writing the file: the changed actions by their names. `BTreeMap`: sorted by name – the same order every time,
/// easy to compare in git or by hand.
impl From<Bindings> for BTreeMap<String, Vec<String>> {
    fn from(bindings: Bindings) -> BTreeMap<String, Vec<String>> {
        let names = |keys: Keys| keys.iter().map(|key| key.map(file_name).unwrap_or_default()).collect();
        bindings.changed.into_iter().map(|(action, keys)| (action.id(), names(keys))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::BuildingKind;

    /// Out of the box no key does two things at once – on no card and not among the keys that work everywhere.
    #[test]
    fn defaults_dont_clash() {
        let bindings = Bindings::default();
        for action in Action::all() {
            let clashes = bindings.clashes(action);
            assert!(clashes.is_empty(), "{action:?} clashes with {clashes:?}");
        }
    }

    /// Every action has its own name in the file – otherwise two would read the same line.
    #[test]
    fn ids_are_unique() {
        let ids: Vec<String> = Action::all().into_iter().map(Action::id).collect();
        for (i, id) in ids.iter().enumerate() {
            assert!(!ids[i + 1..].contains(id), "{id} twice");
        }
        assert!(ids.contains(&"build_hunter_cottage".to_owned()));
        assert!(ids.contains(&"group_0".to_owned()));
    }

    /// Only what was changed goes into the file, and it comes back the same.
    #[test]
    fn file_round_trip() {
        let mut bindings = Bindings::default();
        bindings.set(Card(Button::Stop), 0, Some(KeyCode::Q.into()));
        bindings.set(CameraUp, 1, None);
        bindings.set(Pause, 0, Some(KeyCode::Space.into())); // the default: not kept
        let text = serde_json::to_string(&bindings).unwrap();
        assert_eq!(text, r#"{"camera_up":["W",""],"stop":["Q",""]}"#);
        let back: Bindings = serde_json::from_str(&text).unwrap();
        assert!(back == bindings);
        assert_eq!(back.label(CameraUp), "W");
    }

    /// Unknown names from another version are skipped, the rest is read.
    #[test]
    fn unknown_names_are_skipped() {
        let back: Bindings = serde_json::from_str(r#"{"no_such_action":["Q"],"pause":["NoSuchKey","P"]}"#).unwrap();
        assert_eq!(back.keys(Pause), [None, Some(KeyCode::P.into())]);
    }

    /// The side buttons of the mouse and F13–F24 (what a mouse's software sends for its other buttons) go into the
    /// file and come back.
    #[test]
    fn mouse_buttons_round_trip() {
        let mut bindings = Bindings::default();
        bindings.set(Base, 0, Some(Key::Mouse4));
        bindings.set(Base, 1, Some(Key::Mouse5));
        bindings.set(Pause, 0, Some(KeyCode::F13.into()));
        bindings.set(Pause, 1, Some(KeyCode::F24.into()));
        let text = serde_json::to_string(&bindings).unwrap();
        assert_eq!(text, r#"{"base":["Mouse4","Mouse5"],"pause":["F13","F24"]}"#);
        let back: Bindings = serde_json::from_str(&text).unwrap();
        assert!(back == bindings);
        assert_eq!(back.label(Base), "Mouse 4 / Mouse 5");
        assert_eq!(back.label(Pause), "F13 / F24");
    }

    /// A mercenary is trained with the key of its kind in the barracks.
    #[test]
    fn mercenaries_use_the_key_of_their_kind() {
        let mut bindings = Bindings::default();
        bindings.set(Action::of(Button::Train(UnitKind::Soldier)), 0, Some(KeyCode::K.into()));
        assert!(bindings.has(Action::of(Button::Train(UnitKind::MercSoldier)), KeyCode::K.into()));
    }

    /// Two buildings of one build menu on the same key: both are told.
    #[test]
    fn clash_on_one_card() {
        let mut bindings = Bindings::default();
        let (wall, gate) = (Card(Button::Place(BuildingKind::Wall)), Card(Button::Place(BuildingKind::Gate)));
        bindings.set(gate, 0, Some(KeyCode::Q.into()));
        assert_eq!(bindings.clashes(wall), vec![gate]);
        assert_eq!(bindings.clashes(gate), vec![wall]);
        // The tent's menu is another card: Q there is no clash.
        assert!(bindings.clashes(Card(Button::Place(BuildingKind::Tent))).is_empty());
    }
}
