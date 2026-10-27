//! The map editor's panel on the right (only in `LastBastionEditor.exe` – the game has none, `available`). At the top
//! two buttons, **Edit** and **Test** (F10 switches): in Edit no time passes, you change the map; in Test the round
//! runs from the map as it is, with all tools still at hand – back in Edit the map is as it was before the test.
//! Either way you see the whole map, including the ring outside the border where the zombies come from.
//!
//! The panel has tabs like TAB's map editor (DESIGN.md 12.6):
//! - **Map** – name, save, close, a new map – the round: how many days, the gold and wood at the start (TAB's
//!   "Info") – and where the playing field lies in the whole map (SC2's map bounds and map size).
//! - **Terrain** – paint mountains, forest, water, erase, ore (TAB's painter).
//! - **Entities** – units, buildings (without the rules of the game), zombies, spawners, the eraser.
//! - **Waves** – send waves now, the **timeline** (waves that come by themselves on a given day and hour, like TAB's
//!   events "32d 12h", the final wave among them – each with groups of several breeds, from sides or from chosen
//!   spawners) and the **constant spawn**.
//!
//! A right click on a spawner opens its properties in a small window of their own (`properties_window`).
//!
//! It's made with **egui** (DESIGN.md 12.9, step 1): fields to type numbers and names into (or drag them), drop-down
//! lists, sections that fold, a list that scrolls. egui works like macroquad's drawing – *immediate mode*: every frame
//! `show` describes the whole panel anew, and what was clicked or typed comes back right there. The panel only reads
//! the world; whatever it changes goes out as an order (`Order`), so it stays deterministic. What the left mouse button
//! does on the map with a tool in hand is decided by `ui.rs`, because that's where the mouse is read anyway. Saving,
//! closing and switching between Edit and Test only `game.rs` can do – the panel asks for it (`request`).

use egui_macroquad::egui::{
    self, Align2, CollapsingHeader, Color32, ComboBox, DragValue, FontId, Frame, RichText, ScrollArea, Slider, TextEdit,
    TextStyle, Ui,
};
use macroquad::math::Vec2;

use crate::data::{ConstantSpecial, Stage};
use crate::hud::Layout;
use crate::kinds::{BuildingKind, Resource, UnitKind};
use crate::map::{Ore, Terrain};
use crate::map::BORDER;
use crate::sim::{
    day_and_hour, Axes, Breed, BreedCount, Order, SpawnMode, Spawner, TimedWave, WaveFrom, WaveGroup, World, DAY_TICKS,
    HOUR_TICKS, MAP_SIZES, MOST_PER_TILE,
};

/// Width of the panel. egui counts in points – here the same as macroquad's pixels.
const WIDTH: f32 = 320.0;
/// Room between the panel and the edge of the window or the bottom panel.
const PAD: f32 = 8.0;
/// Room for the hint at the bottom of the panel and the panel's own edge.
const HINT_H: f32 = 44.0;
/// A spawner name and a map name have at most this many letters.
const MAX_NAME: usize = 24;
/// The round lasts at most this many days.
const MAX_DAYS: u32 = 200;
/// The most a number field takes (gold, wood, zombies) – far more than a map needs, only so nothing overflows.
const MAX_AMOUNT: u32 = 10_000_000;
/// The final wave stands out on the timeline – and so does a group with no spawner ticked.
const FINAL: Color32 = Color32::from_rgb(242, 115, 89);
/// Where a group of a wave can come from at random (besides chosen spawners).
const WAVE_FROM: [WaveFrom; 5] = [WaveFrom::OneSpawner, WaveFrom::Sides(1), WaveFrom::Sides(2), WaveFrom::Sides(3), WaveFrom::Sides(4)];
/// What the zombie brush and drop can place: every breed but the giant – it has a tool of its own (`Tool::Giant`),
/// one at a time. A new breed in `Breed::ALL` gets its slider by itself.
fn small_breeds() -> Vec<Breed> {
    Breed::ALL.into_iter().filter(|&b| b != Breed::Giant).collect()
}
/// The buildings of the "Entities" tab, in this order.
const BUILDINGS: &[BuildingKind] = &[
    BuildingKind::CommandCenter,
    BuildingKind::Tent,
    BuildingKind::Wall,
    BuildingKind::Gate,
    BuildingKind::Tower,
    BuildingKind::Barracks,
    BuildingKind::Sawmill,
    BuildingKind::Mine,
    BuildingKind::HunterCottage,
    BuildingKind::FishingHut,
    BuildingKind::Windmill,
    BuildingKind::CharcoalBurner,
    BuildingKind::WoodWorkshop,
    BuildingKind::StoneWorkshop,
    BuildingKind::MunitionsFactory,
    BuildingKind::TeslaTower,
    BuildingKind::StoneHouse,
    BuildingKind::Stakes,
    BuildingKind::StoneWall,
    BuildingKind::StoneTower,
    BuildingKind::Farm,
    BuildingKind::Market,
    BuildingKind::Bank,
    BuildingKind::Inn,
    BuildingKind::Foundry,
    BuildingKind::Ballista,
    BuildingKind::MachineGunTower,
    BuildingKind::Barrel,
];

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
    /// Zombie brush, like TAB: on every tile of the brush area so many of each breed (`Editor::per_tile`) – hold and
    /// drag, painting over a tile again only fills it up.
    ZombieBrush,
    /// A whole crowd of zombies at once at the click.
    ZombieDrop,
    /// One giant on the tile clicked.
    Giant,
    /// The playing field's edges (the world border) can be grabbed and dragged – inside it, it moves as a whole.
    PlayArea,
    /// One unit at the click.
    Unit(UnitKind),
    /// A new spawner at the click.
    Spawner,
    /// A house of a village of doom (3 × 3) at the click – with as many zombies inside as the generator would put there.
    Doom,
    /// A pile of loot at the click – what lies in it and its hidden guards rolled like the generator does.
    Loot,
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
            Tool::Giant => "Giant",
            Tool::PlayArea => "Drag the border",
            Tool::Unit(kind) => &kind.stats().name,
            Tool::Spawner => "+ Spawner",
            Tool::Doom => "Doom house",
            Tool::Loot => "Loot",
            Tool::Building(BuildingKind::CommandCenter) => "CC",
            Tool::Building(kind) => &kind.stats().name,
        }
    }

    /// Does it paint along the way the mouse is dragged (terrain, ore, the eraser, the zombie brush)?
    pub fn paints(self) -> bool {
        matches!(self, Tool::Paint(_) | Tool::Ore(_) | Tool::Remove | Tool::ZombieBrush)
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
    /// "Undo" (Ctrl+Z) and "Redo" (Ctrl+Y).
    Undo,
    Redo,
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

pub struct Editor {
    /// Is there an editor at all? Only in `LastBastionEditor.exe` – in the game nothing of it is drawn or clickable,
    /// and the whole map is never shown.
    pub available: bool,
    pub mode: Mode,
    pub tab: Tab,
    /// The tool in hand – `None`: the left mouse button selects as usual.
    pub tool: Option<Tool>,
    /// Brush size in tiles (edge length, odd).
    pub brush: i32,
    /// The selected spawner (its `id`) – its edge glows and can be dragged.
    pub spawner: Option<u32>,
    /// The selected spawner's properties are open in a window of their own (right click on it) – and where it goes
    /// when it's shown next (screen, once: then it stays where it's dragged to).
    properties: bool,
    properties_at: Option<Vec2>,
    /// "Grid" while it's being changed – until "Apply" (`None`: the size the map has).
    grid_size: Option<usize>,
    /// While the selected spawner's name field has the keyboard: the text so far (`ui.rs` shows it on the map).
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
    /// How many steps undo and redo can go – set by `game.rs` (`undo.rs`).
    pub steps: (usize, usize),
    /// For `ui.rs`: a tool or building button was clicked – hang this building on the mouse (`Some(kind)`), or take
    /// whatever is on it off (`None`).
    pub place: Option<Option<BuildingKind>>,
    /// The building `ui.rs` has on the mouse right now (copied every frame, `sync`) – its button is lit.
    placing: Option<BuildingKind>,
    /// Whose view the editor shows (stays when switching back and forth).
    sight: Sight,
    /// Wandering (asleep, like the zombies a new map starts with) or storming – brush, drop and giant.
    pub wander: bool,
    /// The brush: how many of each breed on every tile (0 to `MOST_PER_TILE`).
    pub per_tile: Vec<(Breed, u32)>,
    /// The drop: which breed, how many.
    pub breed: Breed,
    pub drop: u32,
    /// Is the mouse on the panel, or is one of its fields being dragged? Then the map doesn't get it (`show`).
    on_panel: bool,
    /// Does a field of the panel have the keyboard – now, or until the end of the last frame (`show`)?
    typing: bool,
    /// Did one have it at the end of the last frame?
    typed_before: bool,
    /// Are the panel's fonts set yet (`style`)?
    styled: bool,
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
            properties: false,
            properties_at: None,
            grid_size: None,
            name_edit: None,
            map_name_edit: None,
            map_name: "Untitled".to_owned(),
            map_info: String::new(),
            status: None,
            request: None,
            steps: (0, 0),
            place: None,
            placing: None,
            sight: Sight::All,
            // Like the zombies a new map starts with – asleep, strolling around.
            wander: true,
            // A walker on every tile to begin with – the other sliders at 0.
            per_tile: small_breeds().into_iter().map(|b| (b, u32::from(b == Breed::Walker))).collect(),
            breed: Breed::Walker,
            drop: 100,
            on_panel: false,
            typing: false,
            typed_before: false,
            styled: false,
        }
    }

    /// Every frame: the building `ui.rs` has on the mouse – its button is lit.
    pub fn sync(&mut self, placing: Option<BuildingKind>) {
        self.placing = placing;
    }

    /// The drop: `drop` zombies around `center`, in the radius of the brush – at least as big as the crowd needs.
    pub fn drop_order(&self, center: Vec2) -> Order {
        let radius = self.drop_radius();
        Order::PlaceZombies { center, radius, count: self.drop as usize, breed: self.breed, wander: self.wander }
    }

    /// The radius (tiles) a drop lands in: the brush – at least as big as its crowd needs.
    pub fn drop_radius(&self) -> f32 {
        (self.brush as f32 / 2.0).max(0.5 + (self.drop as f32).sqrt() * 0.3)
    }

    /// The zombie brush on these tiles (`Order::PaintZombies`) – only the breeds whose slider isn't at 0.
    pub fn brush_order(&self, tiles: Vec<(i32, i32)>) -> Order {
        let per_tile = self.per_tile.iter().copied().filter(|&(_, n)| n > 0).collect();
        Order::PaintZombies { tiles, per_tile, wander: self.wander }
    }

    /// What the brush puts on a tile, as a short text on the mouse: "2 Walker, 1 Runner".
    pub fn brush_text(&self) -> String {
        let parts: Vec<String> = self.per_tile.iter().filter(|&&(_, n)| n > 0).map(|&(b, n)| format!("{n} {}", b.name())).collect();
        if parts.is_empty() { "nothing - push a slider".to_owned() } else { parts.join(", ") + " per tile" }
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
        self.properties = false;
    }

    /// Selects a spawner (or none). A name being typed for the previous one was already saved by `show` – the click
    /// that selects took the keyboard away from the field. Its properties window closes.
    pub fn select_spawner(&mut self, spawner: Option<u32>) {
        if spawner != self.spawner {
            self.name_edit = None;
            self.properties = false;
        }
        self.spawner = spawner;
    }

    /// Right click on a spawner: select it and open its properties next to the mouse (`mouse`: on screen).
    pub fn open_properties(&mut self, spawner: u32, mouse: Vec2) {
        self.select_spawner(Some(spawner));
        self.properties = true;
        self.properties_at = Some(mouse + Vec2::new(16.0, 8.0));
    }

    /// Is something being typed into the panel (a name, a number)? Then the keyboard belongs to it and no key does
    /// anything else.
    pub fn is_typing(&self) -> bool {
        self.typing
    }

    /// Is the mouse on the panel (or dragging one of its fields)? Then clicks and the wheel don't go to the map.
    pub fn has_mouse(&self) -> bool {
        self.available && self.on_panel
    }

    /// F10: the other mode – asked for, `game.rs` switches.
    pub fn toggle(&mut self) {
        if self.available {
            let other = if self.mode == Mode::Edit { Mode::Test } else { Mode::Edit };
            self.request = Some(EditorRequest::Mode(other));
        }
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

    /// Every frame in the editor, before `ui.rs` reads the mouse and the keys: the whole panel, built anew. `ctx` is
    /// egui's context – it remembers between frames what egui needs (which section is folded, how far the list is
    /// scrolled, which field has the keyboard). `enabled`: `false` while the menu or a question is in front – then the
    /// panel is only shown, greyed out.
    pub fn show(&mut self, ctx: &egui::Context, world: &World, enabled: bool, orders: &mut Vec<Order>) {
        if !self.styled {
            style(ctx);
            self.styled = true;
        }
        // From the top down to the bottom panel (`hud.rs`) – the command card and the stock stay free.
        let height = Layout::new().panel.y - 2.0 * PAD;
        egui::Window::new("Map editor")
            .title_bar(false)
            .resizable(false)
            .anchor(Align2::RIGHT_TOP, [-PAD, PAD])
            .min_width(WIDTH)
            .max_width(WIDTH)
            .show(ctx, |ui| {
                if !enabled {
                    ui.disable();
                }
                self.top(ui);
                ui.separator();
                // `min_rect`: what the panel has used so far. The tab gets the rest – more scrolls.
                let room = (height - ui.min_rect().height() - HINT_H).max(60.0);
                ScrollArea::vertical().max_height(room).auto_shrink([false, true]).show(ui, |ui| match self.tab {
                    Tab::Map => self.map_tab(ui, world, orders),
                    Tab::Terrain => self.terrain_tab(ui),
                    Tab::Entities => self.entities_tab(ui, world, orders),
                    Tab::Waves => waves_tab(ui, world, orders),
                });
                ui.separator();
                ui.label(RichText::new(self.hint()).small().weak());
            });
        self.properties_window(ctx, world, enabled, orders);
        keep_focus_for_text(ctx);
        // Esc and Enter take the keyboard away from a field before `show` even starts – but in that frame the key
        // was still meant for the field (not for the menu or the selection). So "typing" lasts one frame longer.
        let typing = text_has_focus(ctx);
        self.typing = typing || self.typed_before;
        self.typed_before = typing;
        // `wants_pointer_input`: also while a number is dragged and the mouse has left the panel.
        self.on_panel = ctx.wants_pointer_input() || ctx.is_pointer_over_area();
    }

    /// What's always there: Edit / Test, the tabs, whose view.
    fn top(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let w = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
            for (mode, text) in [(Mode::Edit, "Edit"), (Mode::Test, "Test (F10)")] {
                let button = egui::Button::new(RichText::new(text).strong()).selected(self.mode == mode);
                if ui.add_sized([w, 28.0], button).clicked() && mode != self.mode {
                    self.request = Some(EditorRequest::Mode(mode));
                }
            }
        });
        // Undo and redo only in Edit – a test runs on a copy of the map anyway. `add_enabled`: greyed out if not.
        ui.horizontal(|ui| {
            let (back, forward) = self.steps;
            let edit = self.mode == Mode::Edit;
            if ui.add_enabled(edit && back > 0, egui::Button::new(format!("Undo ({back})"))).on_hover_text("Ctrl+Z").clicked() {
                self.request = Some(EditorRequest::Undo);
            }
            if ui.add_enabled(edit && forward > 0, egui::Button::new(format!("Redo ({forward})"))).on_hover_text("Ctrl+Y").clicked() {
                self.request = Some(EditorRequest::Redo);
            }
        });
        ui.horizontal(|ui| {
            for (tab, text) in [(Tab::Map, "Map"), (Tab::Terrain, "Terrain"), (Tab::Entities, "Entities"), (Tab::Waves, "Waves")] {
                ui.selectable_value(&mut self.tab, tab, text);
            }
        });
        ui.horizontal(|ui| {
            ui.label("View:");
            for (sight, text) in [(Sight::All, "All"), (Sight::Player, "Player"), (Sight::Zombies, "Zombies")] {
                ui.selectable_value(&mut self.sight, sight, text);
            }
        });
    }

    /// The line at the bottom of the panel.
    fn hint(&self) -> &'static str {
        if self.testing() {
            "Test - Edit (F10): the map as before the test"
        } else {
            match self.tab {
                _ if self.tool == Some(Tool::PlayArea) => "Grab an edge of the border - inside: move it",
                Tab::Map | Tab::Waves => "Numbers: drag them, or click and type",
                _ => "Right click or Esc: put the tool away",
            }
        }
    }

    fn map_tab(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        section(ui, "Map", "", |ui| {
            ui.horizontal(|ui| {
                ui.label("Name");
                let shown = self.map_name.clone();
                if let Some(name) = name_field(ui, &mut self.map_name_edit, &shown) {
                    self.map_name = name;
                }
            });
            ui.label(RichText::new(&self.map_info).weak());
            ui.horizontal_wrapped(|ui| {
                if ui.button("Save (Ctrl+S)").clicked() {
                    self.request = Some(EditorRequest::Save);
                }
                if ui.button("Close").clicked() {
                    self.request = Some(EditorRequest::Close);
                }
                if ui.button("New map...").clicked() {
                    self.request = Some(EditorRequest::NewMap);
                }
            });
            if let Some(status) = &self.status {
                ui.label(RichText::new(status).weak());
            }
        });
        section(ui, "Round", "", |ui| {
            // `Grid`: a table – the names in one column, the fields in the next, `end_row` starts a new line.
            egui::Grid::new("round").num_columns(2).show(ui, |ui| {
                ui.label("Days");
                let mut days = world.days;
                if ui.add(DragValue::new(&mut days).range(1..=MAX_DAYS)).changed() {
                    orders.push(Order::SetDays { days });
                }
                ui.end_row();
                ui.label("Gold at the start");
                let mut gold = world.gold;
                if ui.add(DragValue::new(&mut gold).range(0..=MAX_AMOUNT).speed(10)).changed() {
                    orders.push(Order::SetGold { gold });
                }
                ui.end_row();
                ui.label("Wood at the start");
                let mut wood = world.stock.get(Resource::Wood);
                if ui.add(DragValue::new(&mut wood).range(0..=MAX_AMOUNT)).changed() {
                    orders.push(Order::SetWood { wood });
                }
                ui.end_row();
            });
        });
        self.play_section(ui, world, orders);
        self.grid_section(ui, world, orders);
    }

    /// "Playing field": the world border – where units may go, the camera and the minimap. Drag its edges on the map
    /// (`Tool::PlayArea`, `ui.rs`) or type its size: across and down in steps of the screen axes (a step across is
    /// half a tile wide), around the same middle. Nothing on the map moves.
    fn play_section(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        section(ui, "Playing field", "the world border - units stay inside, zombies come from outside", |ui| {
            let (min, size) = world.map.play_steps();
            ui.horizontal(|ui| {
                self.tool_button(ui, Tool::PlayArea);
                // As many tiles as a square of this edge: one step across and down is half a tile.
                let tiles = ((size.0 * size.1) as f32 / 2.0).sqrt().round();
                ui.label(RichText::new(format!("like {tiles} x {tiles} tiles")).weak());
            });
            ui.horizontal(|ui| {
                ui.label("across");
                // `settled`: one order when the number is let go – every new border recomputes the zombies' ways.
                let w = settled(ui, size.0, |v| DragValue::new(v).range(crate::map::MIN_PLAY..=world.map.size as i32));
                ui.label("down");
                let h = settled(ui, size.1, |v| DragValue::new(v).range(crate::map::MIN_PLAY..=world.map.size as i32));
                if w.is_some() || h.is_some() {
                    let new = (w.unwrap_or(size.0), h.unwrap_or(size.1));
                    // Around the same middle.
                    let min = (min.0 + (size.0 - new.0) / 2, min.1 + (size.1 - new.1) / 2);
                    orders.push(Order::SetPlayArea { min, size: new });
                }
                ui.label("steps");
            });
            if ui.button("Like a new map").on_hover_text("In the middle, the ring of a new map around it").clicked() {
                let region = crate::map::Map::play_area_of(world.map.size, world.map.border.max(BORDER));
                let (min, extent) = (region.min.round(), (region.max - region.min).round());
                orders.push(Order::SetPlayArea { min: (min.x as i32, min.y as i32), size: (extent.x as i32, extent.y as i32) });
            }
        });
    }

    /// "Grid": the whole map the playing field lies in (SC2's map size) – more room around it, or less. It only changes
    /// with "Apply": the map is loaded anew then (`World::resize`), and smaller, what lies outside is gone (undo brings
    /// it back). Not during a test.
    fn grid_section(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        section(ui, "Grid", "the whole map - grows or shrinks on every side", |ui| {
            let now = world.map.size;
            let mut size = self.grid_size.unwrap_or(now);
            ui.horizontal(|ui| {
                // `speed(2)` and the rounding below: always even, the map has a middle tile pair.
                ui.add(DragValue::new(&mut size).range(MAP_SIZES).speed(2).suffix(" tiles"));
                size = size / 2 * 2;
                let apply = egui::Button::new("Apply");
                if ui.add_enabled(size != now && self.mode == Mode::Edit, apply).clicked() {
                    orders.push(Order::ResizeMap { size });
                    size = now;
                }
            });
            // `then_some`: `Some(size)` if the condition holds, otherwise `None`.
            self.grid_size = (size != now).then_some(size);
            ui.label(RichText::new("Everything stays where it is, new ground is grass. Then drag the border.").small().weak());
        });
    }

    fn terrain_tab(&mut self, ui: &mut Ui) {
        section(ui, "Paint", "hold and drag", |ui| {
            ui.horizontal_wrapped(|ui| {
                for terrain in [Terrain::Mountain, Terrain::Forest, Terrain::Water, Terrain::Dirt, Terrain::Grass] {
                    self.tool_button(ui, Tool::Paint(terrain));
                }
            });
        });
        section(ui, "Ore", "only on open ground - oil: one 2 x 2 node", |ui| {
            ui.horizontal_wrapped(|ui| {
                for ore in [Some(Ore::Iron), Some(Ore::Coal), Some(Ore::Stone), Some(Ore::Gold), Some(Ore::Oil), None] {
                    self.tool_button(ui, Tool::Ore(ore));
                }
            });
        });
        self.brush_size(ui);
    }

    fn entities_tab(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        section(ui, "Units", "click inside the border", |ui| {
            ui.horizontal_wrapped(|ui| {
                for kind in UnitKind::ALL {
                    self.tool_button(ui, Tool::Unit(kind));
                }
            });
        });
        section(ui, "Buildings", "free, R turns", |ui| {
            ui.horizontal_wrapped(|ui| {
                for &kind in BUILDINGS {
                    self.tool_button(ui, Tool::Building(kind));
                }
            });
        });
        section(ui, "Remove", "hold and drag (brush)", |ui| self.tool_button(ui, Tool::Remove));
        section(ui, "Zombies", "brush: per tile, hold and drag - drop: a crowd - giant: one", |ui| {
            ui.horizontal(|ui| {
                self.tool_button(ui, Tool::ZombieBrush);
                self.tool_button(ui, Tool::ZombieDrop);
                self.tool_button(ui, Tool::Giant);
            });
            ui.horizontal(|ui| {
                ui.label("They");
                ui.radio_value(&mut self.wander, true, "sleep and wander");
                ui.radio_value(&mut self.wander, false, "storm the CC");
            });
            ui.label(format!("Brush - on every tile, at most {MOST_PER_TILE} of each (like TAB):"));
            // `&mut (breed, n)`: each pair of the list lent to change `n` – the breed stays.
            for (breed, n) in &mut self.per_tile {
                ui.add(Slider::new(n, 0..=MOST_PER_TILE).text(breed.name()));
            }
            ui.horizontal(|ui| {
                ui.label("Drop");
                ui.add(DragValue::new(&mut self.drop).range(1..=10_000).suffix(" zombies"));
                breed_list_box(ui, "drop breed", &mut self.breed, &small_breeds());
            });
        });
        section(ui, "Villages of doom and loot", "click", |ui| {
            ui.horizontal(|ui| {
                self.tool_button(ui, Tool::Doom);
                self.tool_button(ui, Tool::Loot);
            });
        });
        self.brush_size(ui);
        section(ui, "Spawners", "click to place or select", |ui| self.tool_button(ui, Tool::Spawner));
        self.spawner_section(ui, world, orders);
    }

    /// The selected spawner in the panel – unless its properties window is open (right click): the same fields twice
    /// would fight over the name being typed (`name_edit`).
    fn spawner_section(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        let Some(s) = self.spawner.and_then(|id| world.find_spawner(id)) else {
            return;
        };
        section(ui, "Selected spawner", "drag its edge - right click: its window", |ui| {
            if self.properties {
                ui.label(RichText::new("Its properties are open in their own window.").weak());
            } else {
                self.spawner_fields(ui, &world.spawners[s], orders);
            }
        });
    }

    /// The selected spawner's properties in a window of their own, next to where it was right-clicked – like TAB's
    /// entity properties. Its X, Esc or selecting something else closes it.
    fn properties_window(&mut self, ctx: &egui::Context, world: &World, enabled: bool, orders: &mut Vec<Order>) {
        if !self.properties {
            return;
        }
        let Some(s) = self.spawner.and_then(|id| world.find_spawner(id)) else {
            self.properties = false;
            return;
        };
        let mut open = true;
        // `id`: egui knows the window by it (and where it was dragged to) – not by its title, which shows the name.
        let mut window = egui::Window::new(format!("Spawner - {}", world.spawners[s].name))
            .id(egui::Id::new("spawner properties"))
            .collapsible(false)
            .resizable(false)
            .min_width(250.0)
            .open(&mut open);
        // `take`: only the first time after the right click – after that it stays where it was dragged.
        if let Some(at) = self.properties_at.take() {
            window = window.current_pos([at.x, at.y]);
        }
        window.show(ctx, |ui| {
            if !enabled {
                ui.disable();
            }
            self.spawner_fields(ui, &world.spawners[s], orders);
        });
        if !open {
            self.properties = false;
        }
    }

    /// What can be set on a spawner: its name, straight or slanted, how it sends zombies by itself – and remove it.
    fn spawner_fields(&mut self, ui: &mut Ui, spawner: &Spawner, orders: &mut Vec<Order>) {
        let (id, name, mode) = (spawner.id, &spawner.name, spawner.mode);
        ui.horizontal(|ui| {
            ui.label("Name");
            if let Some(name) = name_field(ui, &mut self.name_edit, name) {
                orders.push(Order::RenameSpawner { spawner: id, name });
            }
        });
        ui.horizontal(|ui| {
            ui.label("Lies");
            for (axes, text, hint) in [
                (Axes::Screen, "straight", "A rectangle straight on screen, like the world border"),
                (Axes::Tiles, "slanted", "Slanted like the tiles - turned by 45 degrees, about as big"),
            ] {
                if ui.selectable_label(spawner.axes == axes, text).on_hover_text(hint).clicked() && spawner.axes != axes {
                    orders.push(Order::SetSpawnerAxes { spawner: id, axes });
                }
            }
            let (w, h) = spawner.size;
            let unit = if spawner.axes == Axes::Tiles { "tiles" } else { "steps" };
            ui.label(RichText::new(format!("{w} x {h} {unit}")).weak());
        });
        let spawner = id;
        {
            ui.label("Sends zombies by itself:");
            let mut new = mode;
            ui.horizontal(|ui| {
                let periodic = matches!(mode, SpawnMode::Periodic { .. });
                if ui.selectable_label(mode == SpawnMode::Constant, "Constant").clicked() {
                    new = SpawnMode::Constant;
                }
                if ui.selectable_label(periodic, "Periodic").clicked() && !periodic {
                    new = SpawnMode::PERIODIC;
                }
                if ui.selectable_label(mode == SpawnMode::Off, "Off").clicked() {
                    new = SpawnMode::Off;
                }
            });
            // `if let` with a pattern that also takes the values out – `mut`: as copies that may change.
            if let SpawnMode::Periodic { mut every, mut count } = mode {
                ui.horizontal(|ui| {
                    ui.label("every");
                    ui.add(DragValue::new(&mut every).range(1..=3600).suffix(" s"));
                    ui.add(DragValue::new(&mut count).range(1..=10_000).suffix(" zombies"));
                });
                new = SpawnMode::Periodic { every, count };
            }
            if new != mode {
                orders.push(Order::SetSpawnMode { spawner, mode: new });
            }
            if ui.button("Remove spawner (Del)").clicked() {
                self.select_spawner(None);
                orders.push(Order::RemoveSpawner { spawner });
            }
        }
    }

    fn brush_size(&mut self, ui: &mut Ui) {
        section(ui, "Brush size", "", |ui| {
            ui.add(Slider::new(&mut self.brush, 1..=41).suffix(" tiles"));
            // Odd, so the brush has a middle tile – an even size would paint like the next smaller one
            // (`brush_tiles`). `|= 1`: sets the lowest bit, 4 becomes 5.
            self.brush |= 1;
        });
    }

    /// A tool's button: lit while it's in hand – clicked: take it, clicked again: put it away. A building isn't held
    /// by the editor: `ui.rs` hangs it on the mouse (`place`).
    fn tool_button(&mut self, ui: &mut Ui, tool: Tool) {
        let held = match tool {
            Tool::Building(kind) => self.placing == Some(kind),
            _ => self.tool == Some(tool),
        };
        if !ui.selectable_label(held, tool.name()).clicked() {
            return;
        }
        match tool {
            Tool::Building(kind) => {
                self.tool = None;
                self.place = Some(if held { None } else { Some(kind) });
            }
            _ => {
                self.tool = if held { None } else { Some(tool) };
                // Whatever else is on the mouse comes off it: a building, a command waiting for its target.
                self.place = Some(None);
            }
        }
    }
}

/// The "Waves" tab: send waves now, the timeline, the constant spawn. Only reads the world – a free function, not a
/// method, because it needs nothing of the editor.
fn waves_tab(ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
    section(ui, "Send now", "from the spawners", |ui| {
        // `horizontal_wrapped`: side by side, and on to the next line where the panel ends.
        ui.horizontal_wrapped(|ui| {
            for (wave, text) in [(Wave::Small, "Small"), (Wave::Side, "One side"), (Wave::TwoSides, "Two sides")] {
                if ui.button(text).clicked() {
                    orders.push(wave.order());
                }
            }
            if ui.button("+1000 from all spawners").clicked() {
                orders.push(Order::SpawnZombies { count: 1000 });
            }
            if ui.button("Remove all zombies").clicked() {
                orders.push(Order::ClearZombies);
            }
        });
    });
    section(ui, "Timeline", "comes by itself", |ui| {
        let (day, hour) = world.clock();
        ui.label(format!("Now: day {day}, {hour:02}:00"));
        for (i, wave) in world.waves.iter().enumerate() {
            timed_wave(ui, i, wave, world, orders);
        }
        if ui.button("+ Add wave").clicked() {
            // A day after the last wave (or after now), on the full hour – but before a final wave.
            let last = world.waves.iter().filter(|w| !w.final_wave).map(|w| w.at).max().unwrap_or(0).max(world.tick);
            let at = (last + DAY_TICKS) / HOUR_TICKS * HOUR_TICKS;
            orders.push(Order::AddTimedWave(TimedWave::new(at, WaveFrom::Sides(1), Breed::Runner, 200)));
        }
    });
    section(ui, "Constant spawn", "groups out of the spawners set to Constant - or the stage's own", |ui| {
        let constant = &world.constant;
        for (i, stage) in constant.stages.iter().enumerate() {
            stage_row(ui, i, stage, &world.spawners, orders);
        }
        if ui.button("+ Add stage").clicked() {
            // A day after the last one, with its values – or single walkers on day 2 if there is none yet. `clone`:
            // a stage has a list (its spawners), so it's no longer copied by itself like a number.
            let stage = match constant.stages.last() {
                Some(last) => Stage { day: last.day + 1, ..last.clone() },
                None => Stage { day: 2, hour: 0, breed: Breed::Walker, every_seconds: 30.0, group: [1, 1], spawners: None },
            };
            orders.push(Order::AddStage(stage));
        }
        ui.horizontal(|ui| {
            ui.label("After the last stage");
            let mut extra = constant.extra_per_day;
            if ui.add(DragValue::new(&mut extra).range(0.0..=100.0).speed(0.1).suffix(" more per day")).changed() {
                orders.push(Order::SetExtraPerDay { extra });
            }
        });
        ui.add_space(4.0);
        ui.label("Specials - a share of every group, from their day on:");
        for i in 0..constant.specials.len() {
            special_row(ui, i, &constant.specials, orders);
        }
        if ui.button("+ Add special").clicked() {
            // A day after the last one, with its values – or a few fat ones from day 10.
            let special = match constant.specials.last() {
                Some(&last) => ConstantSpecial { day: last.day + 1, ..last },
                None => ConstantSpecial { day: 10, breed: Breed::Fat, share: 0.1 },
            };
            let mut specials = constant.specials.clone();
            specials.push(special);
            orders.push(Order::SetSpecials { specials });
        }
    });
}

/// One wave of the timeline, in a box: when (day, hour), final, reveal, remove – then its groups (`wave_group`) and
/// "+ group". All fields work on a copy (`changed`); if it differs at the end, one order sets it.
fn timed_wave(ui: &mut Ui, i: usize, wave: &TimedWave, world: &World, orders: &mut Vec<Order>) {
    let mut changed = wave.clone();
    let mut removed = false;
    Frame::group(ui.style()).show(ui, |ui| {
        // Every box as wide as the panel, whatever is in it.
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            // A wave whose time has passed won't come any more – greyed out. The final wave in its colour.
            let color = if wave.at < world.tick {
                ui.visuals().weak_text_color()
            } else if wave.final_wave {
                FINAL
            } else {
                ui.visuals().text_color()
            };
            ui.label(RichText::new("Day").color(color));
            let (mut day, mut hour) = day_and_hour(wave.at);
            let a = ui.add(DragValue::new(&mut day).range(1..=MAX_DAYS as u64));
            let b = ui.add(DragValue::new(&mut hour).range(0..=23).suffix(":00"));
            // `|`, not `||`: both sides are worked out – here only booleans, no difference, but no skipping either.
            if a.changed() | b.changed() {
                changed.at = (day - 1) * DAY_TICKS + hour * HOUR_TICKS;
            }
            ui.toggle_value(&mut changed.final_wave, "Final").on_hover_text("Calls every zombie on the map");
            ui.toggle_value(&mut changed.reveal, "Reveal").on_hover_text("Shows the whole map when it comes");
            removed = ui.small_button("x").on_hover_text("Remove the wave").clicked();
        });
        // The group the "x" of was clicked – taken out after the loop, a list can't change while it's gone through.
        let mut gone = None;
        // `iter_mut`: every group lent to change it in place (`&mut`).
        for (g, group) in changed.groups.iter_mut().enumerate() {
            if wave_group(ui, (i, g), group, &world.spawners) {
                gone = Some(g);
            }
        }
        if let Some(g) = gone {
            changed.groups.remove(g);
        }
        if ui.small_button("+ group").on_hover_text("More zombies at the same time - from somewhere else").clicked() {
            let zombies = vec![BreedCount { count: 100, breed: Breed::Runner }];
            changed.groups.push(WaveGroup { from: WaveFrom::Sides(1), breeds: zombies });
        }
    });
    if removed {
        orders.push(Order::RemoveTimedWave { index: i });
    } else if changed != *wave {
        orders.push(Order::SetTimedWave { index: i, wave: changed });
    }
}

/// One group of a wave, in a box of its own: from where (a random side, … or chosen spawners – then a tick box per
/// spawner), and a line per breed: how many, which one. `id`: (wave, group) – egui tells the lists apart by it.
/// Returns whether its "x" was clicked.
fn wave_group(ui: &mut Ui, id: (usize, usize), group: &mut WaveGroup, spawners: &[Spawner]) -> bool {
    let mut removed = false;
    Frame::group(ui.style()).inner_margin(4.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label("From");
            from_box(ui, ("wave from", id), &mut group.from);
            removed = ui.small_button("x").on_hover_text("Remove this group").clicked();
        });
        if let WaveFrom::Spawners(ids) = &mut group.from {
            spawner_checks(ui, ids, spawners);
        }
        let many = group.breeds.len() > 1;
        let mut gone = None;
        for (k, z) in group.breeds.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(DragValue::new(&mut z.count).range(0..=MAX_AMOUNT).speed(5).suffix(" zombies"));
                // A triple as the list's name – one per breed of every group of every wave.
                breed_box(ui, ("wave breed", id, k), &mut z.breed);
                if many && ui.small_button("x").on_hover_text("Remove this breed").clicked() {
                    gone = Some(k);
                }
            });
        }
        if let Some(k) = gone {
            group.breeds.remove(k);
        }
        if ui.small_button("+ breed").on_hover_text("Another breed out of the same spawners").clicked() {
            // The first breed that isn't in the group yet.
            let breed = Breed::ALL.into_iter().find(|b| group.breeds.iter().all(|z| z.breed != *b)).unwrap_or(Breed::Runner);
            group.breeds.push(BreedCount { count: 50, breed });
        }
    });
    removed
}

/// Where a group of a wave comes from: at random (one spawner, 1–4 sides) or chosen spawners. Chosen ones start with
/// none ticked – the group says so in red until one is.
fn from_box(ui: &mut Ui, id: impl std::hash::Hash, from: &mut WaveFrom) {
    let chosen = matches!(from, WaveFrom::Spawners(_));
    ComboBox::from_id_salt(id).width(130.0).selected_text(from_label(from)).show_ui(ui, |ui| {
        for option in WAVE_FROM {
            let label = from_label(&option);
            ui.selectable_value(from, option, label);
        }
        if ui.selectable_label(chosen, "chosen spawners").clicked() && !chosen {
            *from = WaveFrom::Spawners(Vec::new());
        }
    });
}

/// A tick box for every spawner of the map, by name – the ticked ones are in `ids`, in the order of the map's list.
fn spawner_checks(ui: &mut Ui, ids: &mut Vec<u32>, spawners: &[Spawner]) {
    ui.horizontal_wrapped(|ui| {
        for s in spawners {
            let mut on = ids.contains(&s.id);
            if ui.checkbox(&mut on, s.name.as_str()).changed() {
                // Anew from the map's list: in its order, and without spawners that are gone.
                let ticked = |t: &Spawner| if t.id == s.id { on } else { ids.contains(&t.id) };
                *ids = spawners.iter().filter(|t| ticked(t)).map(|t| t.id).collect();
            }
        }
    });
    if ids.is_empty() {
        ui.label(RichText::new("No spawner ticked - nobody comes").color(FINAL));
    }
}

/// One stage of the constant spawn, in a box: from when, which breed, remove – every how many seconds, how big a group
/// – out of which spawners: those set to "Constant", or its own (a tick box per spawner).
fn stage_row(ui: &mut Ui, i: usize, stage: &Stage, spawners: &[Spawner], orders: &mut Vec<Order>) {
    let mut changed = stage.clone();
    let mut removed = false;
    Frame::group(ui.style()).show(ui, |ui| {
        // Every box as wide as the panel, whatever is in it.
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label("Day");
            // `settled`: the stages stay in order of time – see there.
            if let Some(day) = settled(ui, stage.day, |v| DragValue::new(v).range(1..=MAX_DAYS as u64)) {
                changed.day = day;
            }
            if let Some(hour) = settled(ui, stage.hour, |v| DragValue::new(v).range(0..=23).suffix(":00")) {
                changed.hour = hour;
            }
            breed_box(ui, ("stage breed", i), &mut changed.breed);
            removed = ui.small_button("x").on_hover_text("Remove").clicked();
        });
        ui.horizontal(|ui| {
            ui.label("every");
            ui.add(DragValue::new(&mut changed.every_seconds).range(0.1..=3600.0).speed(0.1).suffix(" s"));
            // "From" never above "to": each one's range ends at the other.
            let [from, to] = &mut changed.group;
            let to_now = *to;
            ui.add(DragValue::new(from).range(1..=to_now));
            ui.label("to");
            ui.add(DragValue::new(to).range(*from..=MAX_AMOUNT));
            ui.label("zombies");
        });
        ui.horizontal(|ui| {
            ui.label("From");
            let chosen = changed.spawners.is_some();
            let shown = if chosen { "chosen spawners" } else { "those set to Constant" };
            ComboBox::from_id_salt(("stage from", i)).width(160.0).selected_text(shown).show_ui(ui, |ui| {
                if ui.selectable_label(!chosen, "those set to Constant").clicked() {
                    changed.spawners = None;
                }
                if ui.selectable_label(chosen, "chosen spawners").clicked() && !chosen {
                    // Starting with those set to "Constant" – nothing changes until one is ticked or unticked.
                    changed.spawners = Some(spawners.iter().filter(|s| s.mode == SpawnMode::Constant).map(|s| s.id).collect());
                }
            });
        });
        // `as_mut`: the list inside the `Option`, lent to change it.
        if let Some(ids) = changed.spawners.as_mut() {
            spawner_checks(ui, ids, spawners);
        }
    });
    if removed {
        orders.push(Order::RemoveStage { index: i });
    } else if changed != *stage {
        orders.push(Order::SetStage { index: i, stage: changed });
    }
}

/// One special of the constant spawn: from which day, which breed, what share of every group. The order sets the
/// whole list anew.
fn special_row(ui: &mut Ui, i: usize, specials: &[ConstantSpecial], orders: &mut Vec<Order>) {
    let special = specials[i];
    let mut changed = special;
    let mut removed = false;
    ui.horizontal(|ui| {
        ui.label("From day");
        ui.add(DragValue::new(&mut changed.day).range(1..=MAX_DAYS as u64));
        breed_box(ui, ("special breed", i), &mut changed.breed);
        // In the file a share (0.15), here in percent (15 %).
        let mut percent = special.share * 100.0;
        if ui.add(DragValue::new(&mut percent).range(0.0..=100.0).speed(0.5).max_decimals(1).suffix(" %")).changed() {
            changed.share = percent / 100.0;
        }
        removed = ui.small_button("x").on_hover_text("Remove").clicked();
    });
    // `to_vec`: a copy of the list that can be changed – the world's own list only changes through the order.
    let mut list = specials.to_vec();
    if removed {
        list.remove(i);
    } else if changed != special {
        list[i] = changed;
    } else {
        return;
    }
    orders.push(Order::SetSpecials { specials: list });
}

/// A section with a heading that folds shut with a click (egui remembers it by the heading) and a grey hint under it.
/// `add`: what's in it – a *closure*, like a `lambda` in Python, only that it may have many lines. `impl FnOnce(&mut
/// Ui)`: any closure that takes the section's `Ui` and is called once.
fn section(ui: &mut Ui, title: &str, hint: &str, add: impl FnOnce(&mut Ui)) {
    CollapsingHeader::new(RichText::new(title).strong()).default_open(true).show(ui, |ui| {
        if !hint.is_empty() {
            ui.label(RichText::new(hint).small().weak());
        }
        add(ui);
    });
}

/// A name field: shows `name`; while it has the keyboard the text so far is in `edit` (the map shows a spawner's new
/// name while it's typed). Hands out the new name when typing ends – with Enter or a click elsewhere, not with Esc,
/// and not if it's empty or unchanged.
fn name_field(ui: &mut Ui, edit: &mut Option<String>, name: &str) -> Option<String> {
    let mut text = edit.clone().unwrap_or_else(|| name.to_owned());
    let field = ui.add(TextEdit::singleline(&mut text).char_limit(MAX_NAME).desired_width(f32::INFINITY));
    if field.has_focus() {
        *edit = Some(text);
        return None;
    }
    *edit = None;
    let escaped = ui.input(|input| input.key_pressed(egui::Key::Escape));
    let text = text.trim();
    (field.lost_focus() && !escaped && !text.is_empty() && text != name).then(|| text.to_owned())
}

/// A number field whose new value only counts once it's settled: let go after dragging, Enter or a click elsewhere
/// after typing. Until then it shows its own copy, kept in egui's memory. For the time of a stage of the constant
/// spawn: the simulation keeps the stages in order of time, so a stage dragged past another would swap rows under the
/// mouse – and the drag would go on with the wrong one.
///
/// `field` makes the field for a value: `impl FnOnce(&mut T) -> DragValue<'_>` – a closure that gets the value
/// lent (`&mut`) and gives back a field that borrows it (`'_`: as long as the value is lent).
///
/// `<T>`: a *generic* function – it works for any type `T` that fulfils what stands after `where`: a number egui can
/// drag (`Numeric`), and one egui's memory may keep (`Send + Sync`: safe across threads). Here the days of a stage
/// (`u64`) and the size of the playing field (`i32`) – the compiler makes one copy of the function for each.
fn settled<T>(ui: &mut Ui, value: T, field: impl FnOnce(&mut T) -> DragValue<'_>) -> Option<T>
where
    T: egui::emath::Numeric + Send + Sync,
{
    // The id the field is going to get – `with`: one of its own, derived from it.
    let id = ui.next_auto_id().with("settled");
    let mut shown = ui.data(|data| data.get_temp::<T>(id)).unwrap_or(value);
    let response = ui.add(field(&mut shown).update_while_editing(false));
    if response.dragged() || response.has_focus() {
        ui.data_mut(|data| data.insert_temp(id, shown));
        return None;
    }
    ui.data_mut(|data| data.remove::<T>(id));
    (shown != value).then_some(shown)
}

/// A drop-down list of the zombie breeds. `id`: egui tells the lists apart by it.
fn breed_box(ui: &mut Ui, id: impl std::hash::Hash, breed: &mut Breed) {
    breed_list_box(ui, id, breed, &Breed::ALL);
}

/// A drop-down list of these breeds.
fn breed_list_box(ui: &mut Ui, id: impl std::hash::Hash, breed: &mut Breed, breeds: &[Breed]) {
    ComboBox::from_id_salt(id).width(90.0).selected_text(breed.name()).show_ui(ui, |ui| {
        for &b in breeds {
            ui.selectable_value(breed, b, b.name());
        }
    });
}

/// Where a group of a wave comes from, as a short text.
fn from_label(from: &WaveFrom) -> &'static str {
    match from {
        WaveFrom::OneSpawner => "1 random spawner",
        WaveFrom::Sides(1) => "1 random side",
        WaveFrom::Sides(2) => "2 random sides",
        WaveFrom::Sides(3) => "3 random sides",
        WaveFrom::Sides(_) => "all 4 sides",
        WaveFrom::Spawners(_) => "chosen spawners",
    }
}

/// The panel's fonts a bit bigger than egui's own (12.5 points) – near the 17 pixels of the rest of the game.
fn style(ctx: &egui::Context) {
    ctx.style_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Body, FontId::proportional(15.0)),
            (TextStyle::Button, FontId::proportional(15.0)),
            (TextStyle::Heading, FontId::proportional(19.0)),
            (TextStyle::Monospace, FontId::monospace(14.0)),
        ]
        .into();
    });
}

/// Tab hands egui's keyboard focus on to the next button – and then Space or Enter would press it. In this game Tab
/// and Space are keys of their own (`keys.rs`), so only a field you type into keeps the focus.
fn keep_focus_for_text(ctx: &egui::Context) {
    let button = ctx.memory(|memory| memory.focused()).filter(|&id| TextEdit::load_state(ctx, id).is_none());
    if let Some(id) = button {
        ctx.memory_mut(|memory| memory.surrender_focus(id));
    }
}

/// Is something being typed into a field (a name, or a number clicked to type it)? Only those keep a text cursor in
/// egui's memory (`TextEdit::load_state`) – a button that has the focus doesn't.
fn text_has_focus(ctx: &egui::Context) -> bool {
    ctx.memory(|memory| memory.focused()).is_some_and(|id| TextEdit::load_state(ctx, id).is_some())
}
