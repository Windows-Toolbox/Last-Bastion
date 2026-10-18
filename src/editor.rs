//! The map editor's panel on the right (only in `LastBastionEditor.exe` – the game has none, `available`). At the top
//! two buttons, **Edit** and **Test** (F10 switches): in Edit no time passes, you change the map; in Test the round
//! runs from the map as it is, with all tools still at hand – back in Edit the map is as it was before the test.
//! Either way you see the whole map, including the ring outside the border where the zombies come from.
//!
//! The panel has tabs like TAB's map editor (DESIGN.md 12.6):
//! - **Map** – name, save, close, a new map – and the round: how many days, the gold at the start (TAB's "Info").
//! - **Terrain** – paint mountains, forest, trees, water, erase, ore (TAB's painter).
//! - **Entities** – units, buildings (without the rules of the game), zombies, spawners, the eraser.
//! - **Waves** – send waves now, the **timeline** (waves that come by themselves on a given day and hour, like TAB's
//!   events "32d 12h", the final wave among them) and the **constant spawn**.
//!
//! Like the bottom panel (`hud.rs`) it's split in two: here is where everything lies (`hit`), what it looks
//! like (`draw`) and what a button does (`click`). What the left mouse button does on the map with a tool
//! in hand is decided by `ui.rs`, because that's where the mouse is read anyway. Saving, closing and switching between
//! Edit and Test only `game.rs` can do – the panel asks for it (`request`).

use macroquad::prelude::*;

use crate::data::Stage;
use crate::kinds::{BuildingKind, UnitKind};
use crate::map::{Ore, Terrain};
use crate::sim::{day_and_hour, stage_start, Breed, Order, SpawnMode, TimedWave, WaveFrom, World, DAY_TICKS, HOUR_TICKS};
use crate::widgets::type_into;

const WIDTH: f32 = 270.0;
const PAD: f32 = 8.0;
const MODE_H: f32 = 32.0;
const ROW_H: f32 = 30.0;
const HEADING_H: f32 = 24.0;
/// Room for the hint at the bottom of the panel.
const HINT_H: f32 = 30.0;

const BG: Color = Color::new(0.08, 0.09, 0.11, 0.92);
const BOX: Color = Color::new(0.15, 0.16, 0.20, 1.0);
const HOVER: Color = Color::new(0.22, 0.24, 0.30, 1.0);
const ACTIVE: Color = Color::new(0.25, 0.35, 0.20, 1.0);
const BORDER: Color = Color::new(0.30, 0.32, 0.38, 1.0);
const DIM: Color = Color::new(0.55, 0.57, 0.62, 1.0);
/// The final wave stands out on the timeline.
const FINAL: Color = Color::new(0.95, 0.45, 0.35, 1.0);
/// A spawner name and a map name have at most this many letters.
const MAX_NAME: usize = 24;

/// Editing the map (no time passes) or testing it (the round runs).
#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Edit,
    Test,
}

/// The tabs of the panel (like TAB's map editor: Info, Painter, Entities, Events).
#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Map,
    Terrain,
    Entities,
    Waves,
}

/// Whose eyes the editor looks through. In the game it's always the player's.
#[derive(Clone, Copy, PartialEq)]
pub enum Sight {
    /// No fog – everything, everywhere.
    All,
    /// Fog of war like in the game: what your units and buildings see (in co-op both players share it,
    /// DESIGN.md 12.8 – so there is only one).
    Player,
    /// What the zombies see: lit where at least one zombie looks, dark elsewhere. Your units in the dark are
    /// safe from being spotted.
    Zombies,
}

/// What the left mouse button does on the map while the editor has a tool in hand.
#[derive(Clone, Copy, PartialEq)]
pub enum Tool {
    /// Paint this terrain (hold and drag). `Grass` = rubber.
    Paint(Terrain),
    /// Paint ore on grass (hold and drag). `None` = take the ore away.
    Ore(Option<Ore>),
    /// The eraser: everything on the brush area goes – buildings, units, zombies (hold and drag).
    Remove,
    /// Zombie brush: while the left mouse button is held, every tick a few zombies on the brush area.
    ZombieBrush,
    /// A whole crowd of zombies at once at the click.
    ZombieDrop,
    /// One unit at the click.
    Unit(UnitKind),
    /// A new spawner at the click.
    Spawner,
    /// A building, without the rules of the game. Not really held by the editor: `ui.rs` hangs it on the mouse
    /// like a building of the command center (`placing`), so dragging lines and turning (R) work the same.
    Building(BuildingKind),
}

impl Tool {
    pub fn name(self) -> &'static str {
        match self {
            Tool::Paint(Terrain::Grass) => "Erase",
            Tool::Paint(terrain) => terrain.name(),
            Tool::Ore(Some(ore)) => ore.name(),
            Tool::Ore(None) => "No ore",
            Tool::Remove => "Remove all on it",
            Tool::ZombieBrush => "Brush",
            Tool::ZombieDrop => "Drop",
            Tool::Unit(kind) => &kind.stats().name,
            Tool::Spawner => "+ Spawner",
            Tool::Building(BuildingKind::CommandCenter) => "CC",
            Tool::Building(kind) => &kind.stats().name,
        }
    }

    /// Does it paint along the way the mouse is dragged (terrain, ore, the eraser)?
    pub fn paints(self) -> bool {
        matches!(self, Tool::Paint(_) | Tool::Ore(_) | Tool::Remove)
    }
}

/// What the editor wants from outside, because it can't do it itself (`game.rs` takes it).
#[derive(Clone, Copy, PartialEq)]
pub enum EditorRequest {
    /// "New map…": to the setup screen.
    NewMap,
    /// "Save" (or Ctrl+S).
    Save,
    /// "Close": back to the editor's home screen.
    Close,
    /// Edit ↔ Test (the two buttons at the top, F10).
    Mode(Mode),
}

/// The kinds of waves the editor can send right now – all of them from the spawners.
#[derive(Clone, Copy, PartialEq)]
pub enum Wave {
    /// 150 zombies from one random spawner.
    Small,
    /// 500 zombies from the spawners of one random side.
    Side,
    /// 500 zombies from the spawners of each of two random sides.
    TwoSides,
}

impl Wave {
    fn order(self) -> Order {
        match self {
            Wave::Small => Order::SpawnWave { count: 150, from: WaveFrom::OneSpawner },
            Wave::Side => Order::SpawnWave { count: 500, from: WaveFrom::Sides(1) },
            Wave::TwoSides => Order::SpawnWave { count: 500, from: WaveFrom::Sides(2) },
        }
    }
}

/// The buttons of the panel.
#[derive(Clone, Copy, PartialEq)]
pub enum EditorButton {
    /// One of the two buttons at the top.
    Mode(Mode),
    Tab(Tab),
    /// Whose view: all, the player's, the zombies'.
    Sight(Sight),
    /// Take a tool in hand (clicked again: put it away).
    Tool(Tool),
    /// Brush size in tiles (edge length).
    Brush(i32),

    // --- Map tab ---
    /// The name of the map – click to type a new one.
    MapName,
    /// Only shows what map this is (generator, seed, days) – does nothing when clicked.
    Info,
    SaveMap,
    CloseMap,
    NewMap,
    /// Only shows the result of the last save.
    Status,
    /// The round: a day less (-1) or more (+1) – with Shift 10 …
    Days(i32),
    /// … how many (shown) …
    DaysShow,
    /// … and the gold at the start.
    Gold(i32),
    GoldShow,

    // --- Waves tab ---
    /// 1,000 zombies spread over all spawners.
    EdgeZombies,
    Wave(Wave),
    ClearZombies,
    /// Only shows the clock of the round.
    Clock,
    /// A new wave at the end of the timeline.
    AddTimed,
    /// Wave number `.0` of the timeline: earlier (-1) / later (+1) by 6 hours, with Shift by a day …
    TimedTime(usize, i32),
    /// … its time (shown) …
    TimedShow(usize),
    /// … part of the final wave or not …
    TimedFinal(usize),
    /// … fewer / more zombies …
    TimedCount(usize, i32),
    /// … how many (shown) …
    TimedCountShow(usize),
    /// … which breed (click: the next one) …
    TimedBreed(usize),
    /// … where from (click: the next choice) …
    TimedFrom(usize),
    /// … and off the timeline.
    TimedRemove(usize),
    /// Stage number `.0` of the constant spawn: earlier / later (like the waves), its time (shown), breed, remove …
    StageTime(usize, i32),
    StageShow(usize),
    StageBreed(usize),
    StageRemove(usize),
    /// … every how many seconds a group comes (step down / up, shown) …
    StageEvery(usize, i32),
    StageEveryShow(usize),
    /// … and how big the group is (step down / up, shown).
    StageGroup(usize, i32),
    StageGroupShow(usize),
    /// A new stage at the end, a day after the last one.
    AddStage,
    /// After the last stage the groups grow by this much per day (less / more, shown).
    Extra(i32),
    ExtraShow,

    // --- Selected spawner ---
    /// The name of the selected spawner – click to type a new one.
    SpawnerName,
    /// How the selected spawner sends zombies by itself.
    Sends(Sends),
    /// Periodic: the time between two sends one step shorter (-1) or longer (+1) …
    Every(i32),
    /// … and how many come each time.
    Count(i32),
    /// Only shows a value (the time, the count) – does nothing when clicked.
    Show(Value),
    RemoveSpawner,

    // --- Zombies ---
    /// Which zombies the brush and drop place.
    Breed(Breed),
    /// Placed zombies wander around there (true) or storm the command center (false).
    Wander(bool),
    /// Zombies per tick of the brush one step less (-1) or more (+1) …
    Rate(i32),
    /// … and how many a drop places.
    Drop(i32),
}

/// The three ways of sending (the buttons for `sim::SpawnMode`).
#[derive(Clone, Copy, PartialEq)]
pub enum Sends {
    Constant,
    Periodic,
    Off,
}

/// Which value a `Show` field shows.
#[derive(Clone, Copy, PartialEq)]
pub enum Value {
    Every,
    Count,
    Rate,
    Drop,
}

/// The steps for "every … seconds" and "… zombies" – the buttons jump from one to the next.
const EVERY_STEPS: [u32; 12] = [1, 2, 3, 5, 10, 15, 20, 30, 45, 60, 90, 120];
const COUNT_STEPS: [u32; 9] = [1, 2, 5, 10, 20, 50, 100, 200, 500];
/// Zombies per tick of the brush, zombies per drop.
const RATE_STEPS: [u32; 6] = [1, 2, 3, 5, 10, 20];
const DROP_STEPS: [u32; 7] = [1, 10, 50, 100, 200, 500, 1000];
/// Zombies of a wave on the timeline.
const WAVE_STEPS: [u32; 15] = [5, 10, 15, 25, 50, 100, 150, 200, 300, 400, 500, 750, 1000, 1500, 2000];
/// Where a wave on the timeline comes from – the button goes round these.
const WAVE_FROM: [WaveFrom; 5] = [WaveFrom::OneSpawner, WaveFrom::Sides(1), WaveFrom::Sides(2), WaveFrom::Sides(3), WaveFrom::Sides(4)];
/// The gold a map starts with.
const GOLD_STEPS: [u32; 16] = [0, 50, 100, 150, 200, 300, 400, 500, 750, 1000, 1500, 2000, 3000, 5000, 10000, 50000];
/// How big a group of the constant spawn is: from … to … zombies.
const GROUP_STEPS: [[u32; 2]; 13] = [[1, 1], [1, 2], [1, 3], [2, 4], [3, 5], [4, 5], [5, 8], [8, 10], [10, 15], [15, 20], [20, 30], [30, 50], [50, 100]];
/// How much the groups grow per day after the last stage.
const EXTRA_STEPS: [f32; 7] = [0.0, 0.5, 1.0, 2.0, 3.0, 5.0, 10.0];

/// The next step up (+1) or down (-1) from `now` in `steps` – stays at the ends.
fn next_step(steps: &[u32], now: u32, dir: i32) -> u32 {
    let at = steps.iter().position(|&s| s >= now).unwrap_or(steps.len() - 1) as i32;
    steps[(at + dir).clamp(0, steps.len() as i32 - 1) as usize]
}

/// The same for any list whose entries can be compared (`PartialOrd`: numbers with a fraction, pairs) – a *generic*
/// function: `T` stands for whatever type the list has. `Copy`: an entry can be handed out as a copy.
fn next_in<T: PartialOrd + Copy>(steps: &[T], now: T, dir: i32) -> T {
    let at = steps.iter().position(|&s| s >= now).unwrap_or(steps.len() - 1) as i32;
    steps[(at + dir).clamp(0, steps.len() as i32 - 1) as usize]
}

/// The breed after this one (the breed buttons go round).
fn next_breed(breed: Breed) -> Breed {
    Breed::ALL[(breed.index() + 1) % Breed::ALL.len()]
}

/// One row of the panel: a heading or a few buttons side by side. The buttons are a `Vec` (not a fixed list),
/// because the timeline has as many rows as there are waves.
#[derive(Clone)]
enum Row {
    Heading(&'static str),
    Buttons(Vec<EditorButton>),
}

use EditorButton as B;

/// Shorter to write: a row of buttons.
fn buttons(list: &[EditorButton]) -> Row {
    Row::Buttons(list.to_vec())
}

pub struct Editor {
    /// Is there an editor at all? Only in `LastBastionEditor.exe` – in the game nothing of it is drawn or clickable,
    /// and the whole map is never shown.
    pub available: bool,
    pub mode: Mode,
    pub tab: Tab,
    /// The tool in hand – `None`: the left mouse button selects as usual.
    pub tool: Option<Tool>,
    /// Brush size for painting terrain.
    pub brush: i32,
    /// The selected spawner (its `id`) – its edge glows and can be dragged.
    pub spawner: Option<u32>,
    /// While a new name is being typed for the selected spawner: the text so far. Then the keyboard
    /// belongs to the name field and no key does anything else.
    pub name_edit: Option<String>,
    /// The same for the map name.
    map_name_edit: Option<String>,
    /// Name of the map – the file name when saving.
    pub map_name: String,
    /// What map this is (generator, seed, days) – set by `game.rs`.
    pub map_info: String,
    /// What came of the last "Save" – set by `game.rs`.
    pub status: Option<String>,
    /// For `game.rs`: something only it can do (`EditorRequest`).
    pub request: Option<EditorRequest>,
    /// For `ui.rs`: a building button was clicked – hang this building on the mouse (`Some(kind)`) or take it off
    /// (`None`).
    pub place: Option<Option<BuildingKind>>,
    /// The building `ui.rs` has on the mouse right now (copied every frame, `sync`) – its button is lit.
    placing: Option<BuildingKind>,
    /// How many waves the timeline has and how many stages the constant spawn (copied every frame) – that many rows
    /// in the "Waves" tab.
    wave_count: usize,
    stage_count: usize,
    /// How far the panel is scrolled down (pixels) – mouse wheel on the panel.
    scroll: f32,
    /// Whose view the editor shows (stays when switching back and forth).
    sight: Sight,
    /// What the zombie brush and drop place: breed, wandering or storming, how many.
    pub breed: Breed,
    pub wander: bool,
    /// Zombies per tick of the brush.
    pub rate: u32,
    /// Zombies per drop.
    pub drop: u32,
    /// Sections folded shut (by their heading) – so the panel fits on the screen.
    folded: Vec<&'static str>,
}

impl Editor {
    /// `available`: the editor program (`true`) or the game (`false`).
    pub fn new(available: bool) -> Editor {
        Editor {
            available,
            mode: Mode::Edit,
            tab: Tab::Map,
            tool: None,
            brush: 3,
            spawner: None,
            name_edit: None,
            map_name_edit: None,
            map_name: "Untitled".to_owned(),
            map_info: String::new(),
            status: None,
            request: None,
            place: None,
            placing: None,
            wave_count: 0,
            stage_count: 0,
            scroll: 0.0,
            sight: Sight::All,
            breed: Breed::Walker,
            wander: false,
            rate: 2,
            drop: 100,
            folded: Vec::new(),
        }
    }

    /// Every frame, before anything else: what the panel needs to know from outside.
    pub fn sync(&mut self, world: &World, placing: Option<BuildingKind>) {
        self.wave_count = world.waves.len();
        self.stage_count = world.constant.stages.len();
        self.placing = placing;
    }

    /// Zombie brush or drop: the order for `count` zombies around `center`, in the radius of the brush – a drop at
    /// least as big as the crowd needs.
    pub fn zombie_order(&self, center: Vec2, count: u32) -> Order {
        let radius = self.zombie_radius(count);
        Order::PlaceZombies { center, radius, count: count as usize, breed: self.breed, wander: self.wander }
    }

    /// The radius (tiles) zombies are placed in: the brush – a drop at least as big as its crowd needs.
    pub fn zombie_radius(&self, count: u32) -> f32 {
        let crowd = if self.tool == Some(Tool::ZombieDrop) { 0.5 + (count as f32).sqrt() * 0.3 } else { 0.0 };
        (self.brush as f32 / 2.0).max(crowd)
    }

    /// The editor's tools and the whole map – only in the editor program, in both modes.
    pub fn editing(&self) -> bool {
        self.available
    }

    /// Is a test running (the round runs from the edited map)?
    pub fn testing(&self) -> bool {
        self.available && self.mode == Mode::Test
    }

    /// Whose view is shown right now: in the game always the player's, in the editor the chosen one.
    pub fn sight(&self) -> Sight {
        if self.editing() { self.sight } else { Sight::Player }
    }

    /// Switches to the given mode – `game.rs` does that when it has swapped the worlds (`EditorRequest::Mode`). The
    /// test runs on a copy of the map: its spawners have other numbers, so the selected one and a name being typed go.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.spawner = None;
        self.name_edit = None;
    }

    /// Selects a spawner (or none). A name still being typed for the previous one is saved first.
    pub fn select_spawner(&mut self, spawner: Option<u32>, orders: &mut Vec<Order>) {
        if spawner != self.spawner {
            self.finish_name(orders);
        }
        self.spawner = spawner;
    }

    /// Is a name being typed (spawner or map)? Then the keyboard belongs to the name field.
    pub fn is_typing(&self) -> bool {
        self.name_edit.is_some() || self.map_name_edit.is_some()
    }

    /// Every frame while a name field is active: letters, Backspace, Enter (keep) or Esc (cancel).
    pub fn type_text(&mut self, orders: &mut Vec<Order>) {
        if let Some(text) = &mut self.map_name_edit {
            match type_into(text, MAX_NAME, |_| true) {
                Some(true) => self.finish_name(orders),
                Some(false) => self.map_name_edit = None,
                None => {}
            }
        } else if let Some(text) = &mut self.name_edit {
            match type_into(text, MAX_NAME, |_| true) {
                Some(true) => self.finish_name(orders),
                Some(false) => self.name_edit = None,
                None => {}
            }
        }
    }

    /// Ends typing: renames the spawner (an empty name is ignored by the simulation) or the map.
    pub fn finish_name(&mut self, orders: &mut Vec<Order>) {
        if let Some(name) = self.map_name_edit.take() {
            if !name.trim().is_empty() {
                self.map_name = name.trim().to_owned();
            }
        }
        if let (Some(name), Some(spawner)) = (self.name_edit.take(), self.spawner) {
            orders.push(Order::RenameSpawner { spawner, name });
        }
    }

    /// F10: the other mode – asked for, `game.rs` switches.
    pub fn toggle(&mut self) {
        if self.available {
            let other = if self.mode == Mode::Edit { Mode::Test } else { Mode::Edit };
            self.request = Some(EditorRequest::Mode(other));
        }
    }

    /// Is this screen point on the panel (or on the mode buttons)? Then the click doesn't go to the map.
    pub fn contains(&self, p: Vec2) -> bool {
        self.available && self.panel().contains(p)
    }

    /// The mouse wheel turned on the panel: scroll it (instead of zooming the map).
    pub fn scroll_by(&mut self, wheel: f32) {
        self.scroll = (self.scroll - wheel.signum() * 40.0).clamp(0.0, self.max_scroll());
    }

    /// The tiles the brush covers around `center`: a square for small brushes, a rough circle for big ones.
    pub fn brush_tiles(&self, center: (i32, i32)) -> Vec<(i32, i32)> {
        let r = self.brush / 2;
        let mut tiles = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                // r · r + r: a circle that still contains the corners of a 3 × 3 square.
                if dx * dx + dy * dy <= r * r + r {
                    tiles.push((center.0 + dx, center.1 + dy));
                }
            }
        }
        tiles
    }

    /// A click on the panel. Clicking anywhere but a name field ends typing.
    pub fn click(&mut self, p: Vec2, world: &World, orders: &mut Vec<Order>) {
        if let Some(text) = self.heading_at(p) {
            match self.folded.iter().position(|&t| t == text) {
                Some(at) => {
                    self.folded.remove(at);
                }
                None => self.folded.push(text),
            }
            return;
        }
        let button = self.hit(p);
        if !matches!(button, Some(EditorButton::SpawnerName | EditorButton::MapName)) {
            self.finish_name(orders);
        }
        let Some(button) = button else {
            return;
        };
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        match button {
            EditorButton::Mode(mode) => {
                if mode != self.mode {
                    self.request = Some(EditorRequest::Mode(mode));
                }
            }
            EditorButton::Tab(tab) => {
                self.tab = tab;
                self.scroll = 0.0;
            }
            EditorButton::Sight(sight) => self.sight = sight,
            EditorButton::Tool(Tool::Building(kind)) => {
                self.tool = None;
                self.place = Some(if self.placing == Some(kind) { None } else { Some(kind) });
            }
            EditorButton::Tool(tool) => {
                self.tool = if self.tool == Some(tool) { None } else { Some(tool) };
                if self.placing.is_some() {
                    self.place = Some(None);
                }
            }
            EditorButton::Brush(size) => self.brush = size,
            EditorButton::MapName => {
                if self.map_name_edit.is_none() {
                    self.finish_name(orders);
                    self.map_name_edit = Some(self.map_name.clone());
                }
            }
            EditorButton::SaveMap => self.request = Some(EditorRequest::Save),
            EditorButton::CloseMap => self.request = Some(EditorRequest::Close),
            EditorButton::NewMap => self.request = Some(EditorRequest::NewMap),
            EditorButton::Days(dir) => {
                let step = if shift { 10 } else { 1 };
                let days = (world.days as i32 + dir * step).clamp(1, 200) as u32;
                orders.push(Order::SetDays { days });
            }
            EditorButton::Gold(dir) => orders.push(Order::SetGold { gold: next_step(&GOLD_STEPS, world.gold, dir) }),
            EditorButton::Info | EditorButton::Status | EditorButton::Clock | EditorButton::DaysShow | EditorButton::GoldShow => {}
            EditorButton::EdgeZombies => orders.push(Order::SpawnZombies { count: 1000 }),
            EditorButton::Wave(wave) => orders.push(wave.order()),
            EditorButton::ClearZombies => orders.push(Order::ClearZombies),
            EditorButton::AddTimed => {
                // A day after the last wave (or after now), on the full hour – but before a final wave.
                let last = world.waves.iter().filter(|w| !w.final_wave).map(|w| w.at).max().unwrap_or(0).max(world.tick);
                let at = (last + DAY_TICKS) / HOUR_TICKS * HOUR_TICKS;
                orders.push(Order::AddTimedWave(TimedWave { at, count: 200, from: WaveFrom::Sides(1), breed: Breed::Runner, final_wave: false }));
            }
            // In a pattern with `|` every alternative has to bind the same names – here only `i`, the number of the wave.
            EditorButton::TimedTime(i, _)
            | EditorButton::TimedCount(i, _)
            | EditorButton::TimedFinal(i)
            | EditorButton::TimedBreed(i)
            | EditorButton::TimedFrom(i)
            | EditorButton::TimedRemove(i) => self.timed_click(button, i, world, shift, orders),
            EditorButton::TimedShow(_) | EditorButton::TimedCountShow(_) => {}
            EditorButton::StageTime(..)
            | EditorButton::StageBreed(_)
            | EditorButton::StageRemove(_)
            | EditorButton::StageEvery(..)
            | EditorButton::StageGroup(..)
            | EditorButton::AddStage
            | EditorButton::Extra(_) => self.stage_click(button, world, shift, orders),
            EditorButton::StageShow(_) | EditorButton::StageEveryShow(_) | EditorButton::StageGroupShow(_) | EditorButton::ExtraShow => {}
            EditorButton::SpawnerName => {
                if self.name_edit.is_none() {
                    self.finish_name(orders);
                    self.name_edit = Some(spawner_name(world, self.spawner).unwrap_or_default());
                }
            }
            EditorButton::RemoveSpawner => {
                if let Some(spawner) = self.spawner.take() {
                    self.name_edit = None;
                    orders.push(Order::RemoveSpawner { spawner });
                }
            }
            EditorButton::Sends(_) | EditorButton::Every(_) | EditorButton::Count(_) => {
                // A new mode for the selected spawner – the time and count buttons switch it to "periodic".
                let (Some(spawner), Some(now)) = (self.spawner, spawn_mode(world, self.spawner)) else {
                    return;
                };
                let (every, count) = periodic_values(now);
                let mode = match button {
                    EditorButton::Sends(Sends::Constant) => SpawnMode::Constant,
                    EditorButton::Sends(Sends::Off) => SpawnMode::Off,
                    EditorButton::Every(dir) => SpawnMode::Periodic { every: next_step(&EVERY_STEPS, every, dir), count },
                    EditorButton::Count(dir) => SpawnMode::Periodic { every, count: next_step(&COUNT_STEPS, count, dir) },
                    _ => SpawnMode::Periodic { every, count },
                };
                if mode != now {
                    orders.push(Order::SetSpawnMode { spawner, mode });
                }
            }
            EditorButton::Show(_) => {}
            EditorButton::Breed(breed) => self.breed = breed,
            EditorButton::Wander(wander) => self.wander = wander,
            EditorButton::Rate(dir) => self.rate = next_step(&RATE_STEPS, self.rate, dir),
            EditorButton::Drop(dir) => self.drop = next_step(&DROP_STEPS, self.drop, dir),
        }
    }

    /// A button of wave `i` on the timeline: one order that changes it (or takes it off).
    fn timed_click(&self, button: EditorButton, i: usize, world: &World, shift: bool, orders: &mut Vec<Order>) {
        let Some(&wave) = world.waves.get(i) else {
            return;
        };
        let changed = match button {
            B::TimedTime(_, dir) => {
                let step = (if shift { DAY_TICKS } else { 6 * HOUR_TICKS }) as i64;
                TimedWave { at: (wave.at as i64 + step * dir as i64).max(0) as u64, ..wave }
            }
            B::TimedCount(_, dir) => TimedWave { count: next_step(&WAVE_STEPS, wave.count, dir), ..wave },
            B::TimedFinal(_) => TimedWave { final_wave: !wave.final_wave, ..wave },
            B::TimedBreed(_) => TimedWave { breed: next_breed(wave.breed), ..wave },
            B::TimedFrom(_) => {
                let at = WAVE_FROM.iter().position(|&f| f == wave.from).unwrap_or(0);
                TimedWave { from: WAVE_FROM[(at + 1) % WAVE_FROM.len()], ..wave }
            }
            B::TimedRemove(_) => {
                orders.push(Order::RemoveTimedWave { index: i });
                return;
            }
            _ => return,
        };
        orders.push(Order::SetTimedWave { index: i, wave: changed });
    }

    /// A button of the constant spawn: change a stage, add one, take one off, the growth after the last stage.
    fn stage_click(&self, button: EditorButton, world: &World, shift: bool, orders: &mut Vec<Order>) {
        let stages = &world.constant.stages;
        let change = |i: usize, change: &dyn Fn(Stage) -> Stage, orders: &mut Vec<Order>| {
            if let Some(&stage) = stages.get(i) {
                orders.push(Order::SetStage { index: i, stage: change(stage) });
            }
        };
        match button {
            B::StageTime(i, dir) => change(
                i,
                &|stage| {
                    let step = (if shift { DAY_TICKS } else { 6 * HOUR_TICKS }) as i64;
                    let at = (stage_start(&stage) as i64 + step * dir as i64).max(0) as u64;
                    let (day, hour) = day_and_hour(at);
                    Stage { day, hour, ..stage }
                },
                orders,
            ),
            B::StageBreed(i) => change(i, &|stage| Stage { breed: next_breed(stage.breed), ..stage }, orders),
            B::StageEvery(i, dir) => change(
                i,
                &|stage| Stage { every_seconds: next_step(&EVERY_STEPS, stage.every_seconds.round() as u32, dir) as f32, ..stage },
                orders,
            ),
            B::StageGroup(i, dir) => change(i, &|stage| Stage { group: next_in(&GROUP_STEPS, stage.group, dir), ..stage }, orders),
            B::StageRemove(i) => orders.push(Order::RemoveStage { index: i }),
            B::AddStage => {
                // A day after the last one, with its values – or single walkers on day 2 if there is none yet.
                let stage = match stages.last() {
                    Some(&last) => Stage { day: last.day + 1, ..last },
                    None => Stage { day: 2, hour: 0, breed: Breed::Walker, every_seconds: 30.0, group: [1, 1] },
                };
                orders.push(Order::AddStage(stage));
            }
            B::Extra(dir) => orders.push(Order::SetExtraPerDay { extra: next_in(&EXTRA_STEPS, world.constant.extra_per_day, dir) }),
            _ => {}
        }
    }

    pub fn draw(&self, world: &World) {
        if !self.available {
            return;
        }
        let panel = self.panel();
        draw_rectangle(panel.x, panel.y, panel.w, panel.h, BG);
        draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 1.5, BORDER);

        let mouse = Vec2::from(mouse_position());
        for (y, text) in self.headings() {
            let fold = if self.folded.contains(&text) { "[+]" } else { "[-]" };
            draw_text(&format!("{fold} {text}"), panel.x + PAD, y + 17.0, 17.0, DIM);
        }
        for (r, button) in self.buttons() {
            let active = match button {
                EditorButton::Mode(mode) => self.mode == mode,
                EditorButton::Tab(tab) => self.tab == tab,
                EditorButton::Sight(sight) => self.sight == sight,
                EditorButton::Tool(Tool::Building(kind)) => self.placing == Some(kind),
                EditorButton::Tool(tool) => self.tool == Some(tool),
                EditorButton::Brush(size) => self.brush == size,
                EditorButton::SpawnerName => self.name_edit.is_some(),
                EditorButton::MapName => self.map_name_edit.is_some(),
                EditorButton::Sends(sends) => spawn_mode(world, self.spawner).is_some_and(|mode| sends_of(mode) == sends),
                EditorButton::Breed(breed) => self.breed == breed,
                EditorButton::Wander(wander) => self.wander == wander,
                EditorButton::TimedFinal(i) => world.waves.get(i).is_some_and(|w| w.final_wave),
                _ => false,
            };
            // Fields that only show something have no box.
            let plain = matches!(
                button,
                B::Info | B::Status | B::Clock | B::TimedShow(_) | B::TimedCountShow(_) | B::Show(_) | B::DaysShow | B::GoldShow
                    | B::StageShow(_) | B::StageEveryShow(_) | B::StageGroupShow(_) | B::ExtraShow
            );
            if !plain {
                let bg = if active {
                    ACTIVE
                } else if r.contains(mouse) {
                    HOVER
                } else {
                    BOX
                };
                draw_rectangle(r.x, r.y, r.w, r.h, bg);
                let edge = match button {
                    B::TimedFinal(_) if active => FINAL,
                    _ if active => GREEN,
                    _ => BORDER,
                };
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, edge);
            }
            let label = self.label(button, world);
            let color = match button {
                // A wave whose time has passed won't come any more – a final wave in its colour.
                B::TimedShow(i) if world.waves.get(i).is_some_and(|w| w.at < world.tick) => DIM,
                B::TimedShow(i) if world.waves.get(i).is_some_and(|w| w.final_wave) => FINAL,
                B::Info | B::Status => DIM,
                _ => WHITE,
            };
            let w = measure_text(&label, None, 17, 1.0).width;
            let x = if plain && matches!(button, B::Info | B::Status) { r.x + 2.0 } else { r.x + (r.w - w) / 2.0 };
            draw_text(&label, x, r.y + r.h / 2.0 + 5.0, 17.0, color);
        }
        let hint = if self.testing() {
            "Test - Edit (F10): the map as before the test"
        } else {
            match self.tab {
                Tab::Waves => "Shift + time button: a whole day",
                Tab::Map => "Ctrl+S saves   Shift + days: 10",
                _ => "Right click or Esc: put the tool away",
            }
        };
        draw_text(hint, panel.x + PAD, panel.bottom() - 10.0, 15.0, DIM);
        if self.max_scroll() > 0.0 {
            draw_text("wheel: scroll", panel.right() - 82.0, panel.bottom() - 10.0, 15.0, DIM);
        }
    }

    // --- Where everything lies. Computed anew every frame, in case the window changes size ---

    /// The rows that are always there: the tabs and whose view.
    fn fixed_rows(&self) -> Vec<Row> {
        vec![
            buttons(&[B::Tab(Tab::Map), B::Tab(Tab::Terrain), B::Tab(Tab::Entities), B::Tab(Tab::Waves)]),
            buttons(&[B::Sight(Sight::All), B::Sight(Sight::Player), B::Sight(Sight::Zombies)]),
        ]
    }

    /// The rows of the open tab. The rows of a folded section are left out – only its heading stays.
    fn tab_rows(&self) -> Vec<Row> {
        let brush = [
            Row::Heading("Brush size"),
            buttons(&[B::Brush(1), B::Brush(3), B::Brush(5), B::Brush(9)]),
            buttons(&[B::Brush(15), B::Brush(25), B::Brush(41)]),
        ];
        let mut rows = match self.tab {
            Tab::Map => {
                let mut rows = vec![
                    Row::Heading("Map"),
                    buttons(&[B::MapName]),
                    buttons(&[B::Info]),
                    buttons(&[B::SaveMap, B::CloseMap]),
                    buttons(&[B::NewMap]),
                ];
                if self.status.is_some() {
                    rows.push(buttons(&[B::Status]));
                }
                rows.extend([
                    Row::Heading("Round - days, start gold"),
                    buttons(&[B::Days(-1), B::DaysShow, B::Days(1)]),
                    buttons(&[B::Gold(-1), B::GoldShow, B::Gold(1)]),
                ]);
                rows
            }
            Tab::Terrain => {
                let mut rows = vec![
                    Row::Heading("Paint - hold and drag"),
                    buttons(&[B::Tool(Tool::Paint(Terrain::Mountain)), B::Tool(Tool::Paint(Terrain::Forest)), B::Tool(Tool::Paint(Terrain::Tree))]),
                    buttons(&[B::Tool(Tool::Paint(Terrain::Water)), B::Tool(Tool::Paint(Terrain::Grass))]),
                    Row::Heading("Ore - only on grass (oil: one 2 x 2 node)"),
                    buttons(&[B::Tool(Tool::Ore(Some(Ore::Iron))), B::Tool(Tool::Ore(Some(Ore::Coal))), B::Tool(Tool::Ore(Some(Ore::Stone)))]),
                    buttons(&[B::Tool(Tool::Ore(Some(Ore::Oil))), B::Tool(Tool::Ore(None))]),
                ];
                rows.extend(brush);
                rows
            }
            Tab::Entities => {
                let mut rows = vec![
                    Row::Heading(UNITS),
                    buttons(&[B::Tool(Tool::Unit(UnitKind::Soldier)), B::Tool(Tool::Unit(UnitKind::Ranger))]),
                    buttons(&[B::Tool(Tool::Unit(UnitKind::Sniper)), B::Tool(Tool::Unit(UnitKind::Medic))]),
                    Row::Heading("Buildings - free, R turns"),
                    buttons(&[
                        B::Tool(Tool::Building(BuildingKind::CommandCenter)),
                        B::Tool(Tool::Building(BuildingKind::Tent)),
                        B::Tool(Tool::Building(BuildingKind::Wall)),
                    ]),
                    buttons(&[
                        B::Tool(Tool::Building(BuildingKind::Gate)),
                        B::Tool(Tool::Building(BuildingKind::Tower)),
                        B::Tool(Tool::Building(BuildingKind::Barracks)),
                    ]),
                    Row::Heading("Remove - hold and drag (brush)"),
                    buttons(&[B::Tool(Tool::Remove)]),
                    Row::Heading("Zombies - brush: hold, drop: click"),
                    buttons(&[B::Tool(Tool::ZombieBrush), B::Tool(Tool::ZombieDrop)]),
                    buttons(&[B::Breed(Breed::Walker), B::Breed(Breed::Runner), B::Breed(Breed::Fat)]),
                    buttons(&[B::Wander(false), B::Wander(true)]),
                    buttons(&[B::Rate(-1), B::Show(Value::Rate), B::Rate(1)]),
                    buttons(&[B::Drop(-1), B::Show(Value::Drop), B::Drop(1)]),
                ];
                rows.extend(brush);
                rows.extend([Row::Heading("Spawners - click to place or select"), buttons(&[B::Tool(Tool::Spawner)])]);
                if self.spawner.is_some() {
                    rows.extend([
                        Row::Heading("Selected spawner - drag its edge"),
                        buttons(&[B::SpawnerName]),
                        Row::Heading("Sends zombies by itself"),
                        buttons(&[B::Sends(Sends::Constant), B::Sends(Sends::Periodic), B::Sends(Sends::Off)]),
                        buttons(&[B::Every(-1), B::Show(Value::Every), B::Every(1)]),
                        buttons(&[B::Count(-1), B::Show(Value::Count), B::Count(1)]),
                        buttons(&[B::RemoveSpawner]),
                    ]);
                }
                rows
            }
            Tab::Waves => {
                let mut rows = vec![
                    Row::Heading("Send now - from the spawners"),
                    buttons(&[B::Wave(Wave::Small), B::Wave(Wave::Side), B::Wave(Wave::TwoSides)]),
                    buttons(&[B::EdgeZombies]),
                    buttons(&[B::ClearZombies]),
                    Row::Heading("Timeline - comes by itself"),
                    buttons(&[B::Clock]),
                ];
                for i in 0..self.wave_count {
                    rows.push(buttons(&[B::TimedTime(i, -1), B::TimedShow(i), B::TimedTime(i, 1), B::TimedFinal(i)]));
                    rows.push(buttons(&[
                        B::TimedCount(i, -1),
                        B::TimedCountShow(i),
                        B::TimedCount(i, 1),
                        B::TimedBreed(i),
                        B::TimedFrom(i),
                        B::TimedRemove(i),
                    ]));
                }
                rows.push(buttons(&[B::AddTimed]));
                rows.push(Row::Heading("Constant spawn"));
                for i in 0..self.stage_count {
                    rows.push(buttons(&[B::StageTime(i, -1), B::StageShow(i), B::StageTime(i, 1), B::StageBreed(i), B::StageRemove(i)]));
                    rows.push(buttons(&[
                        B::StageEvery(i, -1),
                        B::StageEveryShow(i),
                        B::StageEvery(i, 1),
                        B::StageGroup(i, -1),
                        B::StageGroupShow(i),
                        B::StageGroup(i, 1),
                    ]));
                }
                rows.push(buttons(&[B::AddStage]));
                rows.push(buttons(&[B::Extra(-1), B::ExtraShow, B::Extra(1)]));
                rows
            }
        };
        let mut folded = false;
        rows.retain(|row| match row {
            Row::Heading(text) => {
                folded = self.folded.contains(text);
                true
            }
            Row::Buttons(_) => !folded,
        });
        rows
    }

    /// Top of the rows that scroll (under the mode buttons, tabs and view).
    fn content_top(&self) -> f32 {
        PAD + MODE_H + 8.0 + self.fixed_rows().iter().map(row_height).sum::<f32>()
    }

    /// How high the scrolling rows are together, and how much room there is for them on the screen.
    fn content_heights(&self) -> (f32, f32) {
        let total: f32 = self.tab_rows().iter().map(row_height).sum();
        let room = (screen_height() - PAD - HINT_H - self.content_top()).max(ROW_H);
        (total, room)
    }

    fn max_scroll(&self) -> f32 {
        let (total, room) = self.content_heights();
        (total - room).max(0.0)
    }

    fn panel(&self) -> Rect {
        let (x, y) = (screen_width() - WIDTH - PAD, PAD);
        let (total, room) = self.content_heights();
        Rect::new(x, y, WIDTH, self.content_top() - PAD + total.min(room) + HINT_H)
    }

    /// The heading under the mouse, if any – a click on it folds its section shut or open.
    fn heading_at(&self, p: Vec2) -> Option<&'static str> {
        let x = self.panel().x;
        self.headings().into_iter().find(|&(y, _)| Rect::new(x, y, WIDTH, HEADING_H).contains(p)).map(|(_, text)| text)
    }

    /// The rows one below the other: the top edge of every row together with the row. The rows of the tab are
    /// shifted by the scrolling, and only those fully in the panel are kept.
    fn rows(&self) -> Vec<(f32, Row)> {
        let mut rows = Vec::new();
        let mut y = PAD + MODE_H + 8.0;
        for row in self.fixed_rows() {
            let h = row_height(&row);
            rows.push((y, row));
            y += h;
        }
        let (top, (_, room)) = (y, self.content_heights());
        let scroll = self.scroll.min(self.max_scroll());
        y -= scroll;
        for row in self.tab_rows() {
            let h = row_height(&row);
            if y >= top - 0.5 && y + h <= top + room + 0.5 {
                rows.push((y, row.clone()));
            }
            y += h;
        }
        rows
    }

    fn headings(&self) -> Vec<(f32, &'static str)> {
        self.rows()
            .into_iter()
            .filter_map(|(y, row)| match row {
                Row::Heading(text) => Some((y, text)),
                Row::Buttons(_) => None,
            })
            .collect()
    }

    /// Every button with its rectangle: first the two mode buttons, then the rest. The buttons of a row share its
    /// width by their `weight` – a "−" needs less room than "Day 12, 06:00".
    fn buttons(&self) -> Vec<(Rect, EditorButton)> {
        let x0 = self.panel().x + PAD;
        let width = WIDTH - 2.0 * PAD;
        let mut buttons = Vec::new();
        let mut add_row = |y: f32, h: f32, row: &[EditorButton]| {
            let unit = width / row.iter().map(|&b| weight(b)).sum::<f32>();
            let mut x = x0;
            for &button in row {
                let w = weight(button) * unit;
                buttons.push((Rect::new(x, y + 2.0, w - 4.0, h - 4.0), button));
                x += w;
            }
        };
        add_row(self.panel().y + 4.0, MODE_H, &[B::Mode(Mode::Edit), B::Mode(Mode::Test)]);
        for (y, row) in self.rows() {
            if let Row::Buttons(row) = row {
                add_row(y, ROW_H, &row);
            }
        }
        buttons
    }

    fn hit(&self, p: Vec2) -> Option<EditorButton> {
        self.buttons().into_iter().find(|(r, _)| r.contains(p)).map(|(_, button)| button)
    }

    /// The text on a button.
    fn label(&self, button: EditorButton, world: &World) -> String {
        // While typing with a blinking line at the end, like a text cursor.
        let typed = |text: &String| if (get_time() * 2.0) as i64 % 2 == 0 { format!("{text}|") } else { text.clone() };
        let stage = |i: usize| world.constant.stages.get(i).copied();
        match button {
            EditorButton::SpawnerName => match &self.name_edit {
                Some(text) => typed(text),
                None => format!("{}  (rename)", spawner_name(world, self.spawner).unwrap_or_default()),
            },
            EditorButton::MapName => match &self.map_name_edit {
                Some(text) => typed(text),
                None => format!("{}  (rename)", self.map_name),
            },
            EditorButton::Info => self.map_info.clone(),
            EditorButton::Status => self.status.clone().unwrap_or_default(),
            EditorButton::DaysShow => match world.days {
                1 => "1 day".to_owned(),
                days => format!("{days} days"),
            },
            EditorButton::GoldShow => format!("{} gold at the start", world.gold),
            EditorButton::Clock => {
                let (day, hour) = world.clock();
                format!("Now: day {day}, {hour:02}:00")
            }
            EditorButton::TimedShow(i) => world.waves.get(i).map_or(String::new(), |w| {
                let (day, hour) = day_and_hour(w.at);
                format!("Day {day}, {hour:02}:00")
            }),
            EditorButton::TimedCountShow(i) => world.waves.get(i).map_or(String::new(), |w| w.count.to_string()),
            EditorButton::TimedBreed(i) => world.waves.get(i).map_or(String::new(), |w| w.breed.name().to_owned()),
            EditorButton::TimedFrom(i) => world.waves.get(i).map_or(String::new(), |w| from_label(w.from).to_owned()),
            EditorButton::StageShow(i) => stage(i).map_or(String::new(), |s| format!("Day {}, {:02}:00", s.day, s.hour)),
            EditorButton::StageBreed(i) => stage(i).map_or(String::new(), |s| s.breed.name().to_owned()),
            EditorButton::StageEveryShow(i) => stage(i).map_or(String::new(), |s| format!("every {} s", s.every_seconds)),
            EditorButton::StageGroupShow(i) => stage(i).map_or(String::new(), |s| format!("{}-{}", s.group[0], s.group[1])),
            EditorButton::ExtraShow => format!("+{} per day after", world.constant.extra_per_day),
            // The values of "periodic" – greyed out with a dash while the spawner sends some other way.
            EditorButton::Show(Value::Rate) => format!("{} per tick", self.rate),
            EditorButton::Show(Value::Drop) => format!("{} per drop", self.drop),
            EditorButton::Show(value) => {
                let mode = spawn_mode(world, self.spawner);
                let (every, count) = periodic_values(mode.unwrap_or(SpawnMode::Off));
                match (mode, value) {
                    (Some(SpawnMode::Periodic { .. }), Value::Every) => format!("every {every} s"),
                    (Some(SpawnMode::Periodic { .. }), Value::Count) => format!("{count} zombies"),
                    _ => "-".to_owned(),
                }
            }
            _ => label(button),
        }
    }
}

/// How much of its row's width a button gets (`Editor::buttons`).
fn weight(button: EditorButton) -> f32 {
    match button {
        // − + x
        B::TimedTime(..)
        | B::TimedCount(..)
        | B::TimedRemove(_)
        | B::StageTime(..)
        | B::StageEvery(..)
        | B::StageGroup(..)
        | B::StageRemove(_)
        | B::Every(_)
        | B::Count(_)
        | B::Rate(_)
        | B::Drop(_)
        | B::Days(_)
        | B::Gold(_)
        | B::Extra(_) => 1.0,
        // The value between a − and a +.
        B::TimedShow(_) | B::StageShow(_) => 4.0,
        B::Show(_) | B::DaysShow | B::GoldShow | B::ExtraShow | B::StageEveryShow(_) => 3.5,
        B::TimedCountShow(_) | B::StageGroupShow(_) => 2.0,
        _ => 2.5,
    }
}

/// Where a wave of the timeline comes from, as a short text.
fn from_label(from: WaveFrom) -> &'static str {
    match from {
        WaveFrom::OneSpawner => "1 spawner",
        WaveFrom::Sides(1) => "1 side",
        WaveFrom::Sides(2) => "2 sides",
        WaveFrom::Sides(3) => "3 sides",
        WaveFrom::Sides(_) => "4 sides",
    }
}

/// The mode of the selected spawner – `None` if none is selected (or it's gone).
fn spawn_mode(world: &World, spawner: Option<u32>) -> Option<SpawnMode> {
    let s = world.find_spawner(spawner?)?;
    Some(world.spawners[s].mode)
}

/// Every how many seconds and how many: the spawner's own values if it sends periodically, otherwise those
/// "periodic" starts with.
fn periodic_values(mode: SpawnMode) -> (u32, u32) {
    match mode {
        SpawnMode::Periodic { every, count } => (every, count),
        _ => periodic_values(SpawnMode::PERIODIC),
    }
}

/// Which of the three buttons belongs to a mode.
fn sends_of(mode: SpawnMode) -> Sends {
    match mode {
        SpawnMode::Constant => Sends::Constant,
        SpawnMode::Periodic { .. } => Sends::Periodic,
        SpawnMode::Off => Sends::Off,
    }
}

fn row_height(row: &Row) -> f32 {
    match row {
        Row::Heading(_) => HEADING_H,
        Row::Buttons(_) => ROW_H,
    }
}

/// The name of a spawner – `None` if there is none (anymore).
fn spawner_name(world: &World, spawner: Option<u32>) -> Option<String> {
    let s = world.find_spawner(spawner?)?;
    Some(world.spawners[s].name.clone())
}

const UNITS: &str = "Units - click inside the border";

/// The text on a button (except the ones that show a value).
fn label(button: EditorButton) -> String {
    match button {
        EditorButton::Mode(Mode::Edit) => "Edit".to_owned(),
        EditorButton::Mode(Mode::Test) => "Test (F10)".to_owned(),
        EditorButton::Tab(Tab::Map) => "Map".to_owned(),
        EditorButton::Tab(Tab::Terrain) => "Terrain".to_owned(),
        EditorButton::Tab(Tab::Entities) => "Entities".to_owned(),
        EditorButton::Tab(Tab::Waves) => "Waves".to_owned(),
        EditorButton::Sight(Sight::All) => "All".to_owned(),
        EditorButton::Sight(Sight::Player) => "Player".to_owned(),
        EditorButton::Sight(Sight::Zombies) => "Zombies".to_owned(),
        EditorButton::Tool(tool) => tool.name().to_owned(),
        EditorButton::Brush(size) => size.to_string(),
        EditorButton::SaveMap => "Save".to_owned(),
        EditorButton::CloseMap => "Close".to_owned(),
        EditorButton::NewMap => "New map...".to_owned(),
        EditorButton::EdgeZombies => "+1000 from all spawners".to_owned(),
        EditorButton::Wave(Wave::Small) => "Small".to_owned(),
        EditorButton::Wave(Wave::Side) => "One side".to_owned(),
        EditorButton::Wave(Wave::TwoSides) => "Two sides".to_owned(),
        EditorButton::ClearZombies => "Remove all zombies".to_owned(),
        EditorButton::AddTimed => "+ Add wave".to_owned(),
        EditorButton::AddStage => "+ Add stage".to_owned(),
        EditorButton::TimedFinal(_) => "Final".to_owned(),
        EditorButton::TimedRemove(_) | EditorButton::StageRemove(_) => "x".to_owned(),
        EditorButton::TimedTime(_, dir)
        | EditorButton::TimedCount(_, dir)
        | EditorButton::StageTime(_, dir)
        | EditorButton::StageEvery(_, dir)
        | EditorButton::StageGroup(_, dir)
        | EditorButton::Every(dir)
        | EditorButton::Count(dir)
        | EditorButton::Rate(dir)
        | EditorButton::Drop(dir)
        | EditorButton::Days(dir)
        | EditorButton::Gold(dir)
        | EditorButton::Extra(dir) => if dir < 0 { "-" } else { "+" }.to_owned(),
        EditorButton::Sends(Sends::Constant) => "Constant".to_owned(),
        EditorButton::Sends(Sends::Periodic) => "Periodic".to_owned(),
        EditorButton::Sends(Sends::Off) => "Off".to_owned(),
        EditorButton::Breed(breed) => breed.name().to_owned(),
        EditorButton::Wander(false) => "Storm the CC".to_owned(),
        EditorButton::Wander(true) => "Wander".to_owned(),
        EditorButton::RemoveSpawner => "Remove spawner (Del)".to_owned(),
        // These show a value – `Editor::label` writes them.
        EditorButton::SpawnerName
        | EditorButton::MapName
        | EditorButton::Info
        | EditorButton::Status
        | EditorButton::Clock
        | EditorButton::DaysShow
        | EditorButton::GoldShow
        | EditorButton::TimedShow(_)
        | EditorButton::TimedCountShow(_)
        | EditorButton::TimedBreed(_)
        | EditorButton::TimedFrom(_)
        | EditorButton::StageShow(_)
        | EditorButton::StageBreed(_)
        | EditorButton::StageEveryShow(_)
        | EditorButton::StageGroupShow(_)
        | EditorButton::ExtraShow
        | EditorButton::Show(_) => String::new(),
    }
}
