//! The two buttons in the top right – **Game View** and **Editor Mode** (F10 switches) – and the editor panel
//! below them. In the game view you only see what lies inside the world border, and there are no tools.
//! In editor mode you see the whole map, including the ring outside the border where the zombies come from.
//!
//! The panel has tabs like TAB's map editor (DESIGN.md 12.6):
//! - **Map** – name, save, a new map from the setup screen (TAB's "Info" tab).
//! - **Terrain** – paint mountains, forest, trees, water, erase (TAB's painter).
//! - **Entities** – units, buildings (without the rules of the game), zombies, spawners.
//! - **Waves** – send waves now, and the **timeline**: waves that come by themselves on a given day and hour
//!   (TAB's events "32d 12h").
//!
//! Like the bottom panel (`hud.rs`) it's split in two: here is where everything lies (`hit`), what it looks
//! like (`draw`) and what a button does (`click`). What the left mouse button does on the map with a tool
//! in hand is decided by `ui.rs`, because that's where the mouse is read anyway.

use macroquad::prelude::*;

use crate::kinds::{BuildingKind, UnitKind};
use crate::map::Terrain;
use crate::mapfile::MapFile;
use crate::sim::{day_and_hour, Breed, Order, SpawnMode, TimedWave, WaveFrom, World, DAY_TICKS, HOUR_TICKS};
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
/// A spawner name and a map name have at most this many letters.
const MAX_NAME: usize = 24;

/// What you're looking at: the game as a player sees it, or the editor with the whole map.
#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Game,
    Editor,
}

/// The tabs of the panel (like TAB's map editor: Info, Painter, Entities, Events).
#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Map,
    Terrain,
    Entities,
    Waves,
}

/// Whose eyes the editor looks through. In the game view it's always the player's.
#[derive(Clone, Copy, PartialEq)]
pub enum Sight {
    /// No fog – everything, everywhere.
    All,
    /// Fog of war like in the game view: what your units and buildings see (in co-op both players share it,
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
            Tool::ZombieBrush => "Brush",
            Tool::ZombieDrop => "Drop",
            Tool::Unit(kind) => kind.stats().name,
            Tool::Spawner => "+ Spawner",
            Tool::Building(BuildingKind::CommandCenter) => "CC",
            Tool::Building(kind) => kind.stats().name,
        }
    }
}

/// What the editor wants from outside, because it can't do it itself (`main.rs` takes it).
#[derive(Clone, Copy, PartialEq)]
pub enum EditorRequest {
    /// "New map…": go to the setup screen.
    NewMap,
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
    /// Only shows what map this is (generator, seed) – does nothing when clicked.
    Info,
    SaveMap,
    NewMap,
    /// Only shows the result of the last save.
    Status,

    // --- Waves tab ---
    /// 1,000 zombies spread over all spawners.
    EdgeZombies,
    Wave(Wave),
    ClearZombies,
    /// Do killed zombies come back in a spawner?
    Respawn,
    /// Only shows the clock of the round.
    Clock,
    /// A new wave at the end of the timeline.
    AddTimed,
    /// Wave number `.0` of the timeline: earlier (-1) / later (+1) by 6 hours, with Shift by a day …
    TimedTime(usize, i32),
    /// … its time (shown) …
    TimedShow(usize),
    /// … fewer / more zombies …
    TimedCount(usize, i32),
    /// … how many (shown) …
    TimedCountShow(usize),
    /// … where from (click: the next choice) …
    TimedFrom(usize),
    /// … and off the timeline.
    TimedRemove(usize),

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
    /// Which zombies the brush and drop place: `None` the usual mix, or only this breed.
    Breed(Option<Breed>),
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
const WAVE_STEPS: [u32; 12] = [10, 25, 50, 100, 150, 200, 300, 500, 750, 1000, 1500, 2000];
/// Where a wave on the timeline comes from – the button goes round these.
const WAVE_FROM: [WaveFrom; 5] = [WaveFrom::OneSpawner, WaveFrom::Sides(1), WaveFrom::Sides(2), WaveFrom::Sides(3), WaveFrom::Sides(4)];

/// The next step up (+1) or down (-1) from `now` in `steps` – stays at the ends.
fn next_step(steps: &[u32], now: u32, dir: i32) -> u32 {
    let at = steps.iter().position(|&s| s >= now).unwrap_or(steps.len() - 1) as i32;
    steps[(at + dir).clamp(0, steps.len() as i32 - 1) as usize]
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
    pub mode: Mode,
    pub tab: Tab,
    /// The tool in hand – `None`: the left mouse button selects as usual. Only in editor mode.
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
    /// What map this is (generator and seed, or "saved map") – set by `main.rs`.
    pub map_info: String,
    /// What came of the last "Save map".
    status: Option<String>,
    /// For `main.rs`: something only it can do (`EditorRequest`).
    pub request: Option<EditorRequest>,
    /// For `ui.rs`: a building button was clicked – hang this building on the mouse (`Some(kind)`) or take it off
    /// (`None`).
    pub place: Option<Option<BuildingKind>>,
    /// The building `ui.rs` has on the mouse right now (copied every frame, `sync`) – its button is lit.
    placing: Option<BuildingKind>,
    /// How many waves the timeline has (copied every frame) – that many rows in the "Waves" tab.
    wave_count: usize,
    /// How far the panel is scrolled down (pixels) – mouse wheel on the panel.
    scroll: f32,
    /// Whose view the editor shows (stays when switching back and forth).
    sight: Sight,
    /// What the zombie brush and drop place: breed (`None`: the usual mix), wandering or storming, how many.
    pub breed: Option<Breed>,
    pub wander: bool,
    /// Zombies per tick of the brush.
    pub rate: u32,
    /// Zombies per drop.
    pub drop: u32,
    /// Sections folded shut (by their heading) – so the panel fits on the screen.
    folded: Vec<&'static str>,
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            mode: Mode::Game,
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
            scroll: 0.0,
            sight: Sight::All,
            breed: None,
            wander: false,
            rate: 2,
            drop: 100,
            folded: Vec::new(),
        }
    }

    /// Every frame, before anything else: what the panel needs to know from outside.
    pub fn sync(&mut self, world: &World, placing: Option<BuildingKind>) {
        self.wave_count = world.waves.len();
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

    pub fn editing(&self) -> bool {
        self.mode == Mode::Editor
    }

    /// Whose view is shown right now: in the game view always the player's, in the editor the chosen one.
    pub fn sight(&self) -> Sight {
        if self.editing() { self.sight } else { Sight::Player }
    }

    /// Switches to the given mode. Leaving the editor puts the tool away.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        if mode == Mode::Game {
            self.tool = None;
            self.spawner = None;
            self.name_edit = None;
            self.map_name_edit = None;
        }
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

    /// F10: the other mode.
    pub fn toggle(&mut self) {
        self.set_mode(if self.editing() { Mode::Game } else { Mode::Editor });
    }

    /// Is this screen point on the panel (or on the mode buttons)? Then the click doesn't go to the map.
    pub fn contains(&self, p: Vec2) -> bool {
        self.panel().contains(p)
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
            EditorButton::Mode(mode) => self.set_mode(mode),
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
            EditorButton::SaveMap => {
                self.status = Some(match MapFile::from_world(world, &self.map_name).save() {
                    Ok(path) => format!("Saved: {}", path.file_name().map_or(String::new(), |f| f.to_string_lossy().into_owned())),
                    Err(e) => format!("Not saved: {e}"),
                });
            }
            EditorButton::NewMap => self.request = Some(EditorRequest::NewMap),
            EditorButton::Info | EditorButton::Status | EditorButton::Clock => {}
            EditorButton::EdgeZombies => orders.push(Order::SpawnZombies { count: 1000 }),
            EditorButton::Wave(wave) => orders.push(wave.order()),
            EditorButton::ClearZombies => orders.push(Order::ClearZombies),
            EditorButton::Respawn => orders.push(Order::SetRespawn(!world.respawn)),
            EditorButton::AddTimed => {
                // A day after the last wave (or after now), on the full hour.
                let last = world.waves.iter().map(|w| w.at).max().unwrap_or(0).max(world.tick);
                let at = (last + DAY_TICKS) / HOUR_TICKS * HOUR_TICKS;
                orders.push(Order::AddTimedWave(TimedWave { at, count: 200, from: WaveFrom::Sides(1) }));
            }
            EditorButton::TimedTime(i, dir) | EditorButton::TimedCount(i, dir) => {
                let Some(&wave) = world.waves.get(i) else {
                    return;
                };
                let changed = if let EditorButton::TimedTime(..) = button {
                    let step = (if shift { DAY_TICKS } else { 6 * HOUR_TICKS }) as i64;
                    TimedWave { at: (wave.at as i64 + step * dir as i64).max(0) as u64, ..wave }
                } else {
                    TimedWave { count: next_step(&WAVE_STEPS, wave.count, dir), ..wave }
                };
                orders.push(Order::SetTimedWave { index: i, wave: changed });
            }
            EditorButton::TimedFrom(i) => {
                if let Some(&wave) = world.waves.get(i) {
                    let at = WAVE_FROM.iter().position(|&f| f == wave.from).unwrap_or(0);
                    let from = WAVE_FROM[(at + 1) % WAVE_FROM.len()];
                    orders.push(Order::SetTimedWave { index: i, wave: TimedWave { from, ..wave } });
                }
            }
            EditorButton::TimedRemove(i) => orders.push(Order::RemoveTimedWave { index: i }),
            EditorButton::TimedShow(_) | EditorButton::TimedCountShow(_) => {}
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

    pub fn draw(&self, world: &World) {
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
                EditorButton::Respawn => world.respawn,
                EditorButton::SpawnerName => self.name_edit.is_some(),
                EditorButton::MapName => self.map_name_edit.is_some(),
                EditorButton::Sends(sends) => spawn_mode(world, self.spawner).is_some_and(|mode| sends_of(mode) == sends),
                EditorButton::Breed(breed) => self.breed == breed,
                EditorButton::Wander(wander) => self.wander == wander,
                _ => false,
            };
            // Fields that only show something have no box.
            let plain = matches!(button, B::Info | B::Status | B::Clock | B::TimedShow(_) | B::TimedCountShow(_) | B::Show(_));
            if !plain {
                let bg = if active {
                    ACTIVE
                } else if r.contains(mouse) {
                    HOVER
                } else {
                    BOX
                };
                draw_rectangle(r.x, r.y, r.w, r.h, bg);
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, if active { GREEN } else { BORDER });
            }
            let label = self.label(button, world);
            let color = match button {
                // A wave whose time has passed won't come any more.
                B::TimedShow(i) if world.waves.get(i).is_some_and(|w| w.at < world.tick) => DIM,
                B::Info | B::Status => DIM,
                _ => WHITE,
            };
            let w = measure_text(&label, None, 17, 1.0).width;
            let x = if plain && matches!(button, B::Info | B::Status) { r.x + 2.0 } else { r.x + (r.w - w) / 2.0 };
            draw_text(&label, x, r.y + r.h / 2.0 + 5.0, 17.0, color);
        }
        if self.editing() {
            let hint = match self.tab {
                Tab::Waves => "Shift + time button: a whole day",
                _ => "Right click or Esc: put the tool away",
            };
            draw_text(hint, panel.x + PAD, panel.bottom() - 10.0, 15.0, DIM);
            if self.max_scroll() > 0.0 {
                draw_text("wheel: scroll", panel.right() - 82.0, panel.bottom() - 10.0, 15.0, DIM);
            }
        }
    }

    // --- Where everything lies. Computed anew every frame, in case the window changes size ---

    /// The rows that are always there in editor mode: the tabs and whose view.
    fn fixed_rows(&self) -> Vec<Row> {
        vec![
            buttons(&[B::Tab(Tab::Map), B::Tab(Tab::Terrain), B::Tab(Tab::Entities), B::Tab(Tab::Waves)]),
            buttons(&[B::Sight(Sight::All), B::Sight(Sight::Player), B::Sight(Sight::Zombies)]),
        ]
    }

    /// The rows of the open tab. The rows of a folded section are left out – only its heading stays.
    fn tab_rows(&self) -> Vec<Row> {
        let mut rows = match self.tab {
            Tab::Map => {
                let mut rows = vec![
                    Row::Heading("Map"),
                    buttons(&[B::MapName]),
                    buttons(&[B::Info]),
                    buttons(&[B::SaveMap, B::NewMap]),
                ];
                if self.status.is_some() {
                    rows.push(buttons(&[B::Status]));
                }
                rows
            }
            Tab::Terrain => vec![
                Row::Heading("Paint - hold and drag"),
                buttons(&[B::Tool(Tool::Paint(Terrain::Mountain)), B::Tool(Tool::Paint(Terrain::Forest)), B::Tool(Tool::Paint(Terrain::Tree))]),
                buttons(&[B::Tool(Tool::Paint(Terrain::Water)), B::Tool(Tool::Paint(Terrain::Grass))]),
                Row::Heading("Brush size - terrain and zombies"),
                buttons(&[B::Brush(1), B::Brush(3), B::Brush(5), B::Brush(9)]),
                buttons(&[B::Brush(15), B::Brush(25), B::Brush(41)]),
            ],
            Tab::Entities => {
                let mut rows = vec![
                    Row::Heading(UNITS),
                    buttons(&[B::Tool(Tool::Unit(UnitKind::Soldier)), B::Tool(Tool::Unit(UnitKind::Sniper)), B::Tool(Tool::Unit(UnitKind::Medic))]),
                    Row::Heading("Buildings - no rules, R turns"),
                    buttons(&[
                        B::Tool(Tool::Building(BuildingKind::CommandCenter)),
                        B::Tool(Tool::Building(BuildingKind::Tent)),
                        B::Tool(Tool::Building(BuildingKind::Wall)),
                    ]),
                    buttons(&[B::Tool(Tool::Building(BuildingKind::Gate)), B::Tool(Tool::Building(BuildingKind::Tower))]),
                    Row::Heading("Zombies - brush: hold, drop: click"),
                    buttons(&[B::Tool(Tool::ZombieBrush), B::Tool(Tool::ZombieDrop)]),
                    buttons(&[B::Breed(None), B::Breed(Some(Breed::Walker)), B::Breed(Some(Breed::Hunter))]),
                    buttons(&[B::Wander(false), B::Wander(true)]),
                    buttons(&[B::Rate(-1), B::Show(Value::Rate), B::Rate(1)]),
                    buttons(&[B::Drop(-1), B::Show(Value::Drop), B::Drop(1)]),
                    Row::Heading("Spawners - click to place or select"),
                    buttons(&[B::Tool(Tool::Spawner)]),
                ];
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
                    buttons(&[B::ClearZombies, B::Respawn]),
                    Row::Heading("Timeline - comes by itself"),
                    buttons(&[B::Clock]),
                ];
                for i in 0..self.wave_count {
                    rows.push(buttons(&[B::TimedTime(i, -1), B::TimedShow(i), B::TimedTime(i, 1)]));
                    rows.push(buttons(&[B::TimedCount(i, -1), B::TimedCountShow(i), B::TimedCount(i, 1), B::TimedFrom(i), B::TimedRemove(i)]));
                }
                rows.push(buttons(&[B::AddTimed]));
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
        if !self.editing() {
            return Rect::new(x, y, WIDTH, MODE_H + 8.0);
        }
        let (total, room) = self.content_heights();
        Rect::new(x, y, WIDTH, self.content_top() - PAD + total.min(room) + HINT_H)
    }

    /// The heading under the mouse, if any – a click on it folds its section shut or open.
    fn heading_at(&self, p: Vec2) -> Option<&'static str> {
        let x = self.panel().x;
        self.headings().into_iter().find(|&(y, _)| Rect::new(x, y, WIDTH, HEADING_H).contains(p)).map(|(_, text)| text)
    }

    /// The rows one below the other: the top edge of every row together with the row. The rows of the tab are
    /// shifted by the scrolling, and only those fully in the panel are kept. Only in editor mode – in the game view
    /// there are only the two mode buttons.
    fn rows(&self) -> Vec<(f32, Row)> {
        let mut rows = Vec::new();
        if !self.editing() {
            return rows;
        }
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

    /// Every button with its rectangle: first the two mode buttons, then the rest.
    /// The buttons of a row share its width equally.
    fn buttons(&self) -> Vec<(Rect, EditorButton)> {
        let x0 = self.panel().x + PAD;
        let width = WIDTH - 2.0 * PAD;
        let mut buttons = Vec::new();
        let mut add_row = |y: f32, h: f32, row: &[EditorButton]| {
            let w = width / row.len() as f32;
            for (k, &button) in row.iter().enumerate() {
                buttons.push((Rect::new(x0 + k as f32 * w, y + 2.0, w - 4.0, h - 4.0), button));
            }
        };
        add_row(self.panel().y + 4.0, MODE_H, &[B::Mode(Mode::Game), B::Mode(Mode::Editor)]);
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
            EditorButton::Clock => {
                let (day, hour) = world.clock();
                format!("Now: day {day}, {hour:02}:00")
            }
            EditorButton::TimedShow(i) => world.waves.get(i).map_or(String::new(), |w| {
                let (day, hour) = day_and_hour(w.at);
                format!("Day {day}, {hour:02}:00")
            }),
            EditorButton::TimedCountShow(i) => world.waves.get(i).map_or(String::new(), |w| w.count.to_string()),
            EditorButton::TimedFrom(i) => world.waves.get(i).map_or(String::new(), |w| from_label(w.from).to_owned()),
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
            _ => label(button, world.respawn),
        }
    }
}

/// Where a wave of the timeline comes from, as a short text.
fn from_label(from: WaveFrom) -> &'static str {
    match from {
        WaveFrom::OneSpawner => "1 spawner",
        WaveFrom::Sides(1) => "1 side",
        WaveFrom::Sides(2) => "2 sides",
        WaveFrom::Sides(3) => "3 sides",
        WaveFrom::Sides(_) => "All sides",
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
fn label(button: EditorButton, respawn: bool) -> String {
    match button {
        EditorButton::Mode(Mode::Game) => "Game View".to_owned(),
        EditorButton::Mode(Mode::Editor) => "Editor Mode".to_owned(),
        EditorButton::Tab(Tab::Map) => "Map".to_owned(),
        EditorButton::Tab(Tab::Terrain) => "Terrain".to_owned(),
        EditorButton::Tab(Tab::Entities) => "Entities".to_owned(),
        EditorButton::Tab(Tab::Waves) => "Waves".to_owned(),
        EditorButton::Sight(Sight::All) => "All".to_owned(),
        EditorButton::Sight(Sight::Player) => "Player".to_owned(),
        EditorButton::Sight(Sight::Zombies) => "Zombies".to_owned(),
        EditorButton::Tool(tool) => tool.name().to_owned(),
        EditorButton::Brush(size) => size.to_string(),
        EditorButton::SaveMap => "Save map".to_owned(),
        EditorButton::NewMap => "New map...".to_owned(),
        EditorButton::EdgeZombies => "+1000 from all spawners".to_owned(),
        EditorButton::Wave(Wave::Small) => "Small".to_owned(),
        EditorButton::Wave(Wave::Side) => "One side".to_owned(),
        EditorButton::Wave(Wave::TwoSides) => "Two sides".to_owned(),
        EditorButton::ClearZombies => "Remove all".to_owned(),
        EditorButton::Respawn => format!("Respawn: {}", if respawn { "on" } else { "off" }),
        EditorButton::AddTimed => "+ Add wave".to_owned(),
        EditorButton::TimedTime(_, dir) | EditorButton::TimedCount(_, dir) => if dir < 0 { "-" } else { "+" }.to_owned(),
        EditorButton::TimedRemove(_) => "x".to_owned(),
        EditorButton::Sends(Sends::Constant) => "Constantly".to_owned(),
        EditorButton::Sends(Sends::Periodic) => "Periodic".to_owned(),
        EditorButton::Sends(Sends::Off) => "Off".to_owned(),
        EditorButton::Every(dir) | EditorButton::Count(dir) | EditorButton::Rate(dir) | EditorButton::Drop(dir) => {
            if dir < 0 { "-" } else { "+" }.to_owned()
        }
        EditorButton::Breed(None) => "Mixed".to_owned(),
        EditorButton::Breed(Some(Breed::Walker)) => "Walker".to_owned(),
        EditorButton::Breed(Some(Breed::Hunter)) => "Hunter".to_owned(),
        EditorButton::Wander(false) => "Storm the CC".to_owned(),
        EditorButton::Wander(true) => "Wander".to_owned(),
        EditorButton::RemoveSpawner => "Remove spawner (Del)".to_owned(),
        // These show a value – `Editor::label` writes them.
        EditorButton::SpawnerName
        | EditorButton::MapName
        | EditorButton::Info
        | EditorButton::Status
        | EditorButton::Clock
        | EditorButton::TimedShow(_)
        | EditorButton::TimedCountShow(_)
        | EditorButton::TimedFrom(_)
        | EditorButton::Show(_) => String::new(),
    }
}
