//! The panel at the bottom, laid out like in They Are Billions (DESIGN.md 11.1): minimap, the clock with the pause
//! button, the control groups, the selection, the command card with the build menus, the colony's stock. Here is
//! where everything lies (`Layout`), what it looks like, and what is under the mouse (`hit`). What a click does is
//! decided by `ui.rs`.

use macroquad::prelude::*;

use crate::draw::{breed_color, building_color, kind_color, ZOMBIE};
use crate::formation::Shape;
use crate::iso::{iso, iso_inverse, Camera, TILE_W};
use crate::editor::Sight;
use crate::data;
use crate::kinds::{BuildingKind, UnitKind};
use crate::map::Region;
use crate::picture::MapPicture;
use crate::sim::{Building, RepairPlan, World, Zombie, MAX_PRODUCTION, TICKS_PER_SECOND};
use crate::ui::{Pick, Sub, Ui};

pub const PANEL_H: f32 = 200.0;
const PAD: f32 = 8.0;
/// Between two parts of the panel.
const GAP: f32 = 8.0;
const MINIMAP_W: f32 = 260.0; // the minimap is a diamond, twice as wide as high
const CLOCK_W: f32 = 104.0;
const GROUPS_W: f32 = 46.0;
const STOCK_W: f32 = 224.0;
const ICON: f32 = 36.0;
const QUEUE_ICON: f32 = 40.0;
const SUMMARY_H: f32 = 20.0;
const PORTRAIT: f32 = 84.0;
const CELL_W: f32 = 58.0;
const CELL_H: f32 = 38.0;
const BUTTON: f32 = 58.0;
const CARD_COLS: usize = 5;
const CARD_ROWS: usize = 3;

// Brass and dark wood like TAB's steampunk panel, the values glowing green like its icons.
const PANEL_BG: Color = Color::new(0.09, 0.07, 0.05, 0.97);
const SECTION_BG: Color = Color::new(0.05, 0.04, 0.03, 1.0);
const BOX_BG: Color = Color::new(0.15, 0.11, 0.07, 1.0);
const HOVER_BG: Color = Color::new(0.25, 0.19, 0.11, 1.0);
const WAIT_BG: Color = Color::new(0.22, 0.32, 0.14, 1.0);
const BORDER: Color = Color::new(0.45, 0.32, 0.16, 1.0);
const BRASS: Color = Color::new(0.80, 0.60, 0.30, 1.0);
const DIM: Color = Color::new(0.72, 0.64, 0.50, 1.0);
const GLOW: Color = Color::new(0.62, 0.95, 0.40, 1.0);
const KEY: Color = Color::new(1.0, 0.85, 0.40, 1.0);
const TOO_DEAR: Color = Color::new(1.0, 0.35, 0.3, 1.0);
const FINAL: Color = Color::new(1.0, 0.6, 0.5, 1.0);

/// The buttons of the command card. Which ones are visible depends on the selection (`Ui::card`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Button {
    // Units
    Move,
    Stop,
    Hold,
    Patrol,
    Attack,
    /// Formations (like in Diplomacy is Not an Option) – only choose the shape, the right mouse button draws it.
    FormLine,
    FormBox,
    FormCircle,
    // Barracks
    TrainSoldier,
    TrainRanger,
    TrainSniper,
    TrainMedic,
    Rally,
    // Command center
    /// Open one of the build menus – the card then shows its buildings (`Ui::submenu`).
    Open(Submenu),
    /// Back from a build menu to the main buttons of its building – B (its spot in the grid) or Esc.
    Back,
    PlaceTent,
    PlaceBarracks,
    PlaceWall,
    PlaceGate,
    PlaceTower,
    /// A building that's planned but not in the game yet – the index in `PLANNED`. Its button already sits where
    /// it will be, greyed out.
    Planned(usize),
    // Tower
    Unload,
    /// Every building: repair the selected buildings (F, like "fix").
    Repair,
}

/// The build menus of the command center – the keys on the left of the keyboard, like TAB's build categories.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Submenu {
    /// Q: houses for the colonists.
    Colonists,
    /// W: gathering buildings.
    Resources,
    /// E: power plants and the grid (DESIGN.md 5.5).
    Energy,
    /// A: industry and trade.
    Industry,
    /// S: military.
    Military,
    /// D: walls, gates, towers, traps.
    Defense,
    /// Y: wonders – the most expensive buildings, with very strong buffs.
    Wonders,
}

impl Submenu {
    /// Its spot on the command center's card: Q W E / A S D / Y.
    fn slot(self) -> usize {
        match self {
            Submenu::Colonists => 0,
            Submenu::Resources => 1,
            Submenu::Energy => 2,
            Submenu::Industry => 5,
            Submenu::Military => 6,
            Submenu::Defense => 7,
            Submenu::Wonders => 10,
        }
    }
}

/// A building that's planned but not in the game yet (DESIGN.md 6): which menu, which spot, its name on the button
/// (short), its full name and what it will do.
pub struct Planned {
    pub menu: Submenu,
    pub slot: usize,
    pub short: &'static str,
    pub name: &'static str,
    pub help: &'static str,
}

/// Shorter to write the table below.
const fn plan(menu: Submenu, slot: usize, short: &'static str, name: &'static str, help: &'static str) -> Planned {
    Planned { menu, slot, short, name, help }
}

/// All planned buildings, menu by menu. When one gets built, its line goes and a real button takes its spot.
pub const PLANNED: [Planned; 20] = [
    plan(Submenu::Colonists, 1, "Stone h.", "Stone house", "Tier 2 house: more colonists and gold, needs wood, stone and tools (DESIGN.md 5.3)."),
    plan(Submenu::Resources, 0, "Hunter", "Hunter cottage", "1 x 1. Food from the game in the forest around it."),
    plan(Submenu::Resources, 1, "Fishing", "Fishing hut", "1 x 1, at the water. Food from the fish."),
    plan(Submenu::Resources, 2, "Saw mill", "Saw mill", "2 x 2, next to the forest. Wood."),
    plan(Submenu::Resources, 3, "Quarry", "Quarry", "2 x 2. Stone."),
    plan(Submenu::Resources, 4, "Oil pump", "Oil pump", "2 x 2, right on an oil node (the dark pools - one next to the base, groups further out). Oil."),
    plan(Submenu::Energy, 0, "Tesla", "Tesla tower", "Carries the electricity further: more of the base is covered."),
    plan(Submenu::Energy, 1, "Windmill", "Windmill", "Quiet, but little power."),
    plan(Submenu::Energy, 2, "Generator", "Generator", "Burns wood: cheap, inefficient, loud - the first power after the command center's."),
    plan(Submenu::Energy, 3, "Oil gen.", "Oil burning generator", "Burns oil: a lot of power."),
    plan(Submenu::Industry, 0, "Wood sh.", "Wood workshop", "Works wood into goods (recipes come with the economy)."),
    plan(Submenu::Industry, 1, "Stone sh.", "Stone workshop", "Works stone into goods (recipes come with the economy)."),
    plan(Submenu::Industry, 2, "Foundry", "Foundry", "Iron ore and coal become iron."),
    plan(Submenu::Industry, 3, "Inn", "The Inn", "Like the inn in They Are Billions - what it does here is still open."),
    plan(Submenu::Military, 1, "Munitions", "Munitions factory", "Makes ammunition from resources - if units need ammunition instead of only gold (open question 6: ammo is free for now)."),
    plan(Submenu::Military, 2, "Siege", "Siege factory", "Vehicles and siege weapons."),
    plan(Submenu::Defense, 3, "Mines", "Mines", "Go off under zombies - and under your own units: area damage hits everyone."),
    plan(Submenu::Defense, 4, "Trip wire", "Trip wire", "Stretched across a way: whoever walks into it sets it off."),
    plan(Submenu::Defense, 5, "Ballista", "Ballista tower", "Heavy tower: a big bolt, slow and strong."),
    plan(Submenu::Defense, 6, "MG tower", "Machine gun tower", "Fast fire against crowds - needs power."),
];

/// The buttons for units – top row like in StarCraft, the formations at the bottom.
pub const UNIT_BUTTONS: [Button; 8] = [
    Button::Move,
    Button::Stop,
    Button::Hold,
    Button::Patrol,
    Button::Attack,
    Button::FormLine,
    Button::FormBox,
    Button::FormCircle,
];
/// The buttons of the command center: one per build menu, Q W E / A S D / Y – and F repair. It builds, the
/// barracks trains.
pub const COMMAND_CENTER_BUTTONS: [Button; 8] = [
    Button::Open(Submenu::Colonists),
    Button::Open(Submenu::Resources),
    Button::Open(Submenu::Energy),
    Button::Open(Submenu::Industry),
    Button::Open(Submenu::Military),
    Button::Open(Submenu::Defense),
    Button::Open(Submenu::Wonders),
    Button::Repair,
];
/// The buttons of the barracks.
pub const BARRACKS_BUTTONS: [Button; 6] =
    [Button::TrainSoldier, Button::TrainRanger, Button::TrainSniper, Button::TrainMedic, Button::Rally, Button::Repair];
/// The buttons of a tower.
pub const TOWER_BUTTONS: [Button; 3] = [Button::Unload, Button::Rally, Button::Repair];
/// Buildings that do nothing else (tent, wall, gate): only repair.
pub const REPAIR_BUTTONS: [Button; 1] = [Button::Repair];

/// What a build menu shows: the buildings that are in the game, the planned ones (greyed out) and "back".
pub fn menu_buttons(menu: Submenu) -> Vec<Button> {
    let built: &[Button] = match menu {
        Submenu::Colonists => &[Button::PlaceTent],
        Submenu::Military => &[Button::PlaceBarracks],
        Submenu::Defense => &[Button::PlaceWall, Button::PlaceGate, Button::PlaceTower],
        _ => &[],
    };
    let planned = (0..PLANNED.len()).filter(|&i| PLANNED[i].menu == menu).map(Button::Planned);
    built.iter().copied().chain(planned).chain([Button::Back]).collect()
}

/// StarCraft's grid layout: the keys of the 15 spots of the command card, row by row – the left part of the
/// keyboard. macroquad reads keys by their place on a US keyboard: on a German one the first key of the bottom row
/// is Y.
const GRID_KEYS: [KeyCode; 15] = [
    KeyCode::Q, KeyCode::W, KeyCode::E, KeyCode::R, KeyCode::T,
    KeyCode::A, KeyCode::S, KeyCode::D, KeyCode::F, KeyCode::G,
    KeyCode::Z, KeyCode::X, KeyCode::C, KeyCode::V, KeyCode::B,
];
const GRID_LABELS: [&str; 15] = ["Q", "W", "E", "R", "T", "A", "S", "D", "F", "G", "Y", "X", "C", "V", "B"];

impl Button {
    /// The name on the button.
    pub fn name(self) -> &'static str {
        match self {
            Button::Move => "Move",
            Button::Stop => "Stop",
            Button::Hold => "Hold",
            Button::Patrol => "Patrol",
            Button::Attack => "Attack",
            Button::FormLine => "Line",
            Button::FormBox => "Box",
            Button::FormCircle => "Circle",
            Button::TrainSoldier => "Soldier",
            Button::TrainRanger => "Ranger",
            Button::TrainSniper => "Sniper",
            Button::TrainMedic => "Medic",
            Button::Rally => "Rally",
            Button::Open(Submenu::Colonists) => "Colonists",
            Button::Open(Submenu::Resources) => "Resources",
            Button::Open(Submenu::Energy) => "Energy",
            Button::Open(Submenu::Industry) => "Industry",
            Button::Open(Submenu::Military) => "Military",
            Button::Open(Submenu::Defense) => "Defense",
            Button::Open(Submenu::Wonders) => "Wonders",
            Button::Back => "Back",
            Button::PlaceTent => "Tent",
            Button::PlaceBarracks => "Barracks",
            Button::PlaceWall => "Wall",
            Button::PlaceGate => "Gate",
            Button::PlaceTower => "Tower",
            Button::Planned(i) => PLANNED[i].short,
            Button::Unload => "Unload",
            Button::Repair => "Repair",
        }
    }

    /// The full name – in the tooltip and in messages.
    pub fn title(self) -> &'static str {
        match self {
            Button::Planned(i) => PLANNED[i].name,
            Button::Open(Submenu::Industry) => "Industry and trade",
            _ => self.name(),
        }
    }

    /// The key: for units their own letters (M = move, S = stop …), for buildings the grid (`GRID_KEYS`).
    pub fn key(self) -> KeyCode {
        match self {
            Button::Move => KeyCode::M,
            Button::Stop => KeyCode::S,
            Button::Hold => KeyCode::H,
            Button::Patrol => KeyCode::P,
            Button::Attack => KeyCode::T,
            Button::FormLine => KeyCode::X,
            Button::FormBox => KeyCode::C,
            Button::FormCircle => KeyCode::V,
            _ => GRID_KEYS[self.slot()],
        }
    }

    fn key_label(self) -> &'static str {
        match self {
            Button::Move => "M",
            Button::Stop => "S",
            Button::Hold => "H",
            Button::Patrol => "P",
            Button::Attack => "T",
            // Not Z: macroquad reads keys by their place on a US keyboard, and there a German keyboard has Y.
            Button::FormLine => "X",
            Button::FormBox => "C",
            Button::FormCircle => "V",
            _ => GRID_LABELS[self.slot()],
        }
    }

    /// Spot on the command card: 0–4 top row, 5–9 middle, 10–14 bottom. For buildings this also decides the key.
    fn slot(self) -> usize {
        match self {
            Button::Move | Button::TrainSoldier | Button::Unload => 0,
            Button::PlaceTent | Button::PlaceBarracks | Button::PlaceWall => 0,
            Button::Stop | Button::TrainRanger | Button::PlaceGate => 1,
            Button::Hold | Button::PlaceTower => 2,
            Button::Patrol | Button::TrainSniper => 3,
            Button::Attack | Button::TrainMedic => 4,
            Button::Open(menu) => menu.slot(),
            Button::Planned(i) => PLANNED[i].slot,
            Button::Repair => 8,
            Button::Rally => 9,
            Button::FormLine => 10,
            Button::FormBox => 11,
            Button::FormCircle => 12,
            Button::Back => 14,
        }
    }

    /// Which formation shape this button chooses – `None` for all other buttons.
    pub fn formation(self) -> Option<Shape> {
        match self {
            Button::FormLine => Some(Shape::Line),
            Button::FormBox => Some(Shape::Box),
            Button::FormCircle => Some(Shape::Circle),
            _ => None,
        }
    }

    /// Which unit this button trains – `None` for all other buttons.
    pub fn trains(self) -> Option<UnitKind> {
        match self {
            Button::TrainSoldier => Some(UnitKind::Soldier),
            Button::TrainRanger => Some(UnitKind::Ranger),
            Button::TrainSniper => Some(UnitKind::Sniper),
            Button::TrainMedic => Some(UnitKind::Medic),
            _ => None,
        }
    }

    /// What the button costs in gold – the unit it trains or the building it places (`None`: it costs nothing).
    pub fn cost(self) -> Option<u32> {
        match (self.trains(), self.places()) {
            (Some(kind), _) => Some(kind.stats().cost),
            (_, Some(kind)) => Some(kind.stats().cost),
            _ => None,
        }
    }

    /// Which building this button puts on the grid – `None` for all other buttons.
    pub fn places(self) -> Option<BuildingKind> {
        match self {
            Button::PlaceBarracks => Some(BuildingKind::Barracks),
            Button::PlaceTent => Some(BuildingKind::Tent),
            Button::PlaceWall => Some(BuildingKind::Wall),
            Button::PlaceGate => Some(BuildingKind::Gate),
            Button::PlaceTower => Some(BuildingKind::Tower),
            _ => None,
        }
    }

    fn help(self) -> &'static str {
        match self {
            Button::Move => "Walk there without shooting. Drag: in formation. On a unit: follow it.",
            Button::Stop => "Cancel all commands.",
            Button::Hold => "Stand still and only shoot what comes into range.",
            Button::Patrol => "Walk back and forth, fight everything on the way. Drag: in formation.",
            Button::Attack => "Walk there and fight everything on the way. Drag: in formation. On a zombie: exactly that one. Medics: on one of your units: beam at it.",
            Button::FormLine => "Formation: rows along the line you drag with the right mouse button. Soldiers in front.",
            Button::FormBox => "Formation: a square block, the dragged line is its front. Soldiers in front.",
            Button::FormCircle => "Formation: a ring around where you start dragging, the drag is the radius. Soldiers outside.",
            Button::TrainSoldier => "Tough, short range, aims quickly, loud. Build time: 5 s. Shift: 5 at once.",
            Button::TrainRanger => "Fast and weak, draws the bow a while before every arrow, quiet. Build time: 4 s. Shift: 5 at once.",
            Button::TrainSniper => "Long range, one big shot - very loud. Build time: 8 s. Shift: 5 at once.",
            Button::TrainMedic => "No attack. Beams at hurt units nearby (healing comes later). Build time: 6 s.",
            Button::Rally => "Where new units go (tower: where units go when unloaded). On a unit: follow it. Also with right click.",
            Button::Open(Submenu::Colonists) => "Houses for your colonists: the tent - later the stone house.",
            Button::Open(Submenu::Resources) => "Gathering - all still planned: hunter cottage, fishing hut, saw mill, quarry, oil pump.",
            Button::Open(Submenu::Energy) => "Power - all still planned: tesla tower, windmill, generator, oil burning generator.",
            Button::Open(Submenu::Industry) => "All still planned: wood workshop, stone workshop, foundry, the inn.",
            Button::Open(Submenu::Military) => "The barracks - later the munitions factory and the siege factory.",
            Button::Open(Submenu::Defense) => "Wall, gate, tower - later mines, trip wires, ballista and machine gun towers.",
            Button::Open(Submenu::Wonders) => "The most expensive buildings, with very strong buffs - none designed yet.",
            Button::Back => "Back to the main buttons (B or Esc).",
            Button::PlaceTent => "Tent (2 x 2). 6 colonists pay gold; shield down = infected. Drag: a line. Right click / Esc: stop building.",
            Button::PlaceBarracks => "Barracks (3 x 3, 800 HP). Trains soldiers, rangers, snipers and medics - the only building that does. Right click / Esc: stop building.",
            Button::PlaceWall => "Wall (1 x 1, 400 HP). Zombies have to tear it down. Drag: a line, diagonal too. Right click / Esc: stop building.",
            Button::PlaceGate => "Gate (3 x 1, 400 HP). Your units walk through the middle, zombies don't. Can go on walls (cheaper there, but walls + gate still cost more than the gate alone). R: turn. Right click / Esc: stop building.",
            Button::PlaceTower => "Tower (1 x 1, 600 HP). Right click it with units: up to 4 go in, shoot and see 2 tiles further and can't be hit. Needs 1 free tile to the next tower. Drag: a line.",
            Button::Planned(i) => PLANNED[i].help,
            Button::Unload => "Everyone out of the tower - on the side of the rally point, then they walk there.",
            Button::Repair => "Repair all selected buildings: pay gold for what's missing, it comes back over a few seconds. An infected building is cured once it's whole - it's yours again and pays gold.",
        }
    }
}

/// What lies under the mouse.
#[derive(Clone, Copy)]
pub enum HudHit {
    /// A point on the minimap – already converted to world coordinates.
    Minimap(Vec2),
    /// The pause button of the clock.
    Pause,
    /// A control group (0 = key 1 … 9 = key 0).
    Group(usize),
    /// An icon in the selection – the index in `Ui::selection_view` (units and buildings).
    Icon(usize),
    /// An entry in the production queue of the selected building.
    Queue(usize),
    /// "Repair all" in the stock (X).
    RepairAll,
    Button(Button),
    /// Anywhere else on a panel.
    Panel,
}

/// Where everything lies on screen, from left to right. Computed anew every frame, in case the window changes size.
pub struct Layout {
    pub panel: Rect,
    /// The diamond of the minimap (its frame is as high as the panel).
    minimap: Rect,
    clock: Rect,
    groups: Rect,
    /// The selection: one thing in detail, or an icon for each of several.
    info: Rect,
    card: Rect,
    /// The colony's stock: colonists, gold, resources.
    stock: Rect,
}

impl Layout {
    pub fn new() -> Layout {
        let (w, h) = (screen_width(), screen_height());
        let panel = Rect::new(0.0, h - PANEL_H, w, PANEL_H);
        let (top, inner_h) = (panel.y + PAD, PANEL_H - 2.0 * PAD);
        let minimap_h = MINIMAP_W / 2.0;
        let minimap = Rect::new(PAD, panel.y + (PANEL_H - minimap_h) / 2.0, MINIMAP_W, minimap_h);
        let clock = Rect::new(minimap.right() + GAP, top, CLOCK_W, inner_h);
        let groups = Rect::new(clock.right() + GAP, top, GROUPS_W, inner_h);
        let stock = Rect::new(w - PAD - STOCK_W, top, STOCK_W, inner_h);
        let (card_w, card_h) = (CARD_COLS as f32 * BUTTON, CARD_ROWS as f32 * BUTTON);
        let card = Rect::new(stock.x - GAP - card_w, panel.y + (PANEL_H - card_h) / 2.0, card_w, card_h);
        let info_x = groups.right() + GAP;
        let info = Rect::new(info_x, top, (card.x - GAP - info_x).max(0.0), inner_h);
        Layout { panel, minimap, clock, groups, info, card, stock }
    }

    /// Is this screen point on the map (so not on the panel)?
    pub fn in_game_view(&self, p: Vec2) -> bool {
        p.x >= 0.0 && p.y >= 0.0 && p.x < screen_width() && p.y < self.panel.y
    }

    /// What lies under the mouse? `None` = nothing of the panel, so the map.
    /// `area`: the part of the map the minimap shows (game view: inside the border, editor: everything).
    /// `selected`: how many units are selected, `queue`: how long the production queue of the
    /// selected building is, `card`: which buttons the command card shows right now.
    pub fn hit(&self, mouse: Vec2, area: Region, selected: usize, queue: usize, card: &[Button]) -> Option<HudHit> {
        if !self.panel.contains(mouse) {
            return None;
        }
        if self.minimap.contains(mouse) {
            // Also in the corners next to the diamond: limit to the area.
            let p = self.minimap_to_world(mouse, area);
            return Some(HudHit::Minimap(area.clamp(p, 0.01)));
        }
        if self.pause_rect().contains(mouse) {
            return Some(HudHit::Pause);
        }
        for g in 0..10 {
            if self.group_rect(g).contains(mouse) {
                return Some(HudHit::Group(g));
            }
        }
        if selected > 1 {
            for i in 0..selected.min(self.icon_capacity()) {
                if self.icon_rect(i).contains(mouse) {
                    return Some(HudHit::Icon(i));
                }
            }
        }
        for i in 0..queue {
            if self.queue_rect(i).contains(mouse) {
                return Some(HudHit::Queue(i));
            }
        }
        for &button in card {
            if self.button_rect(button.slot()).contains(mouse) {
                return Some(HudHit::Button(button));
            }
        }
        if self.repair_all_rect().contains(mouse) {
            return Some(HudHit::RepairAll);
        }
        Some(HudHit::Panel)
    }

    // --- Minimap: the same isometric diamond as the map, only small – showing `area` ---

    fn minimap_scale(&self, area: Region) -> f32 {
        self.minimap.w / (area.size * TILE_W)
    }

    /// Where the corner `area.min` lies on the minimap: at the top in the middle.
    fn minimap_origin(&self) -> Vec2 {
        vec2(self.minimap.x + self.minimap.w / 2.0, self.minimap.y)
    }

    fn world_to_minimap(&self, p: Vec2, area: Region) -> Vec2 {
        self.minimap_origin() + iso(p - area.min) * self.minimap_scale(area)
    }

    fn minimap_to_world(&self, m: Vec2, area: Region) -> Vec2 {
        iso_inverse((m - self.minimap_origin()) / self.minimap_scale(area)) + area.min
    }

    // --- Where the individual fields lie ---

    /// The frame around the minimap: as high as the other parts, the diamond in its middle.
    fn minimap_frame(&self) -> Rect {
        Rect::new(self.minimap.x, self.clock.y, self.minimap.w, self.clock.h)
    }

    /// "Repair all" in the stock: the fifth row of the people column is free.
    fn repair_all_rect(&self) -> Rect {
        let r = self.stock;
        let row_h = (r.h - 12.0) / 5.0;
        Rect::new(r.x + 10.0, r.y + 6.0 + 4.0 * row_h + 2.0, 74.0, row_h - 4.0)
    }

    fn pause_rect(&self) -> Rect {
        let w = 56.0;
        Rect::new(self.clock.x + (self.clock.w - w) / 2.0, self.clock.y + 72.0, w, 40.0)
    }

    /// The control groups one below the other, like TAB's numbers next to the selection.
    fn group_rect(&self, g: usize) -> Rect {
        let h = self.groups.h / 10.0;
        Rect::new(self.groups.x, self.groups.y + g as f32 * h, self.groups.w, h - 2.0)
    }

    /// The selection without its frame.
    fn info_inner(&self) -> Rect {
        Rect::new(self.info.x + 6.0, self.info.y + 6.0, (self.info.w - 12.0).max(0.0), self.info.h - 12.0)
    }

    fn portrait(&self) -> Rect {
        let inner = self.info_inner();
        Rect::new(inner.x, inner.y, PORTRAIT, PORTRAIT)
    }

    /// Right of the portrait: name, life bar, a few lines of text (or the production queue).
    fn text_column(&self) -> Rect {
        let inner = self.info_inner();
        let x = inner.x + PORTRAIT + 10.0;
        Rect::new(x, inner.y, (inner.right() - x).max(0.0), PORTRAIT)
    }

    /// Below the portrait: the values, one small box each (like TAB's icons with a number).
    fn cells(&self) -> Rect {
        let inner = self.info_inner();
        let y = inner.y + PORTRAIT + 8.0;
        Rect::new(inner.x, y, inner.w, inner.bottom() - y)
    }

    fn icon_columns(&self) -> usize {
        ((self.info_inner().w / ICON) as usize).max(1)
    }

    /// How many icons fit into the field.
    fn icon_capacity(&self) -> usize {
        self.icon_columns() * ((self.info_inner().h - SUMMARY_H) / ICON) as usize
    }

    fn icon_rect(&self, i: usize) -> Rect {
        let inner = self.info_inner();
        let cols = self.icon_columns();
        let (col, row) = ((i % cols) as f32, (i / cols) as f32);
        Rect::new(inner.x + col * ICON, inner.y + row * ICON, ICON - 4.0, ICON - 4.0)
    }

    /// The production queue (and a tower's garrison) lies right of the portrait, below the life bar.
    fn queue_rect(&self, i: usize) -> Rect {
        let text = self.text_column();
        Rect::new(text.x + i as f32 * QUEUE_ICON, text.y + 44.0, QUEUE_ICON - 4.0, QUEUE_ICON - 4.0)
    }

    fn button_rect(&self, b: usize) -> Rect {
        let (col, row) = ((b % CARD_COLS) as f32, (b / CARD_COLS) as f32);
        Rect::new(self.card.x + col * BUTTON, self.card.y + row * BUTTON, BUTTON - 4.0, BUTTON - 4.0)
    }
}

/// Draws the whole panel. `minimap`: the picture of the map (`picture.rs`), kept up to date by `game.rs`.
pub fn draw(layout: &Layout, world: &World, camera: &Camera, ui: &Ui, minimap: &MapPicture) {
    let p = layout.panel;
    draw_rectangle(p.x, p.y, p.w, p.h, PANEL_BG);
    draw_line(p.x, p.y + 1.5, p.right(), p.y + 1.5, 3.0, BRASS);
    draw_line(p.x, p.y + 4.0, p.right(), p.y + 4.0, 1.0, BORDER);
    for r in [layout.minimap_frame(), layout.clock, layout.info, layout.stock] {
        frame(r);
    }

    draw_minimap(layout, world, camera, ui, minimap);
    draw_clock(layout, world, ui);
    draw_groups(layout, world, ui);
    draw_selection(layout, world, ui);
    draw_card(layout, world, ui, &ui.card(world));
    draw_stock(layout, world);
    draw_repair_all(layout, world);
}

/// A part of the panel: dark inside, a brass edge with a rivet in every corner.
fn frame(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, SECTION_BG);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, BORDER);
    draw_rectangle_lines(r.x + 3.0, r.y + 3.0, r.w - 6.0, r.h - 6.0, 1.0, Color { a: 0.35, ..BORDER });
    for (x, y) in [(r.x, r.y), (r.right(), r.y), (r.x, r.bottom()), (r.right(), r.bottom())] {
        draw_circle(x, y, 3.0, BRASS);
    }
}

fn draw_minimap(layout: &Layout, world: &World, camera: &Camera, ui: &Ui, minimap: &MapPicture) {
    let area = ui.view_area(world);
    let to_mini = |p: Vec2| layout.world_to_minimap(p, area);
    let diamond = |corner: Vec2, size: Vec2, color: Color| {
        let a = to_mini(corner);
        let b = to_mini(corner + vec2(size.x, 0.0));
        let c = to_mini(corner + size);
        let d = to_mini(corner + vec2(0.0, size.y));
        draw_triangle(a, b, c, color);
        draw_triangle(a, c, d, color);
    };

    // Terrain and fog: one picture instead of a diamond per tile. Fog only in the player's view – in the editor's
    // "View: Zombies" the map stays readable (`game.rs` paints the picture that way).
    minimap.draw(area, to_mini);
    let fog = ui.sight() == Sight::Player;
    for building in &world.buildings {
        let color = if ui.selected_buildings.contains(&building.id) { WHITE } else { building_color(building.kind) };
        diamond(building.corner(), building.far_corner() - building.corner(), color);
    }

    // Zombies only where someone sees them right now (in the editor all of them).
    for zombie in world.zombies.iter().filter(|z| area.contains(z.pos) && (!fog || world.vision.sees(z.pos))) {
        let m = to_mini(zombie.pos);
        draw_rectangle(m.x - 1.0, m.y - 1.0, 2.0, 2.0, ZOMBIE);
    }
    for unit in world.units.iter().filter(|u| !u.is_inside()) {
        let m = to_mini(unit.pos);
        let color = if ui.selected.contains(&unit.id) { WHITE } else { kind_color(unit.kind) };
        draw_rectangle(m.x - 1.5, m.y - 1.5, 3.0, 3.0, color);
    }

    // What the camera shows right now: a rectangle, because the map and the minimap are rotated the same way.
    let top_left = to_mini(camera.screen_to_world(Vec2::ZERO)).max(layout.minimap.point());
    let bottom_right = to_mini(camera.screen_to_world(vec2(screen_width(), layout.panel.y)))
        .min(layout.minimap.point() + layout.minimap.size());
    let size = bottom_right - top_left;
    draw_rectangle_lines(top_left.x, top_left.y, size.x, size.y, 1.5, WHITE);
}

/// The clock like in TAB: day and hour in big numbers, the pause button between them and below, the speed, and how
/// long until the next wave (the final wave in its colour).
fn draw_clock(layout: &Layout, world: &World, ui: &Ui) {
    let r = layout.clock;
    let (day, hour) = world.clock();
    let (left, right) = (r.x + 10.0, r.right() - 10.0);
    draw_text("Day", left, r.y + 18.0, 15.0, DIM);
    draw_right("Hour", right, r.y + 18.0, 15.0, DIM);
    draw_text(day.to_string(), left, r.y + 48.0, 32.0, GLOW);
    draw_right(&format!("{hour:02}"), right, r.y + 48.0, 32.0, GLOW);
    if world.days > 0 {
        draw_text(format!("of {}", world.days), left, r.y + 64.0, 14.0, DIM);
    }

    let pause = layout.pause_rect();
    let hover = pause.contains(Vec2::from(mouse_position()));
    draw_rectangle(pause.x, pause.y, pause.w, pause.h, if hover { HOVER_BG } else { BOX_BG });
    draw_rectangle_lines(pause.x, pause.y, pause.w, pause.h, 2.0, if ui.paused { GLOW } else { BRASS });
    let c = pause.center();
    if ui.paused {
        // A "play" triangle: click to go on.
        draw_triangle(vec2(c.x - 7.0, c.y - 10.0), vec2(c.x - 7.0, c.y + 10.0), vec2(c.x + 10.0, c.y), GLOW);
    } else {
        draw_rectangle(c.x - 8.0, c.y - 10.0, 5.0, 20.0, BRASS);
        draw_rectangle(c.x + 3.0, c.y - 10.0, 5.0, 20.0, BRASS);
    }
    let speed = if ui.paused { "PAUSED".to_owned() } else { format!("Speed x{}", ui.speed) };
    draw_centered(&speed, c.x, pause.bottom() + 18.0, 15.0, if ui.paused { GLOW } else { DIM });

    if let Some(next) = world.waves.iter().filter(|w| w.at > world.tick).min_by_key(|w| w.at) {
        let seconds = (next.at - world.tick) / TICKS_PER_SECOND as u64;
        let final_wave = world.waves.iter().any(|w| w.at == next.at && w.final_wave);
        let (what, color) = if final_wave { ("FINAL WAVE", FINAL) } else { ("next wave", DIM) };
        draw_centered(what, c.x, r.bottom() - 26.0, 14.0, color);
        draw_centered(&format!("{}:{:02}", seconds / 60, seconds % 60), c.x, r.bottom() - 9.0, 18.0, if final_wave { FINAL } else { WHITE });
    }
    if hover {
        draw_tooltip(&["Pause (Space)", "Orders still work while paused. + / - : faster, slower."], pause.x, layout.panel.y - 4.0);
    }
}

fn draw_groups(layout: &Layout, world: &World, ui: &Ui) {
    for (g, group) in ui.groups.iter().enumerate() {
        let r = layout.group_rect(g);
        let selected = !group.is_empty() && group.units == ui.selected && group.buildings == ui.selected_buildings;
        draw_rectangle(r.x, r.y, r.w, r.h, if group.is_empty() { SECTION_BG } else { BOX_BG });
        draw_rectangle_lines(r.x, r.y, r.w, r.h, if selected { 2.0 } else { 1.0 }, if selected { WHITE } else { BORDER });

        let key = if g == 9 { "0".to_owned() } else { (g + 1).to_string() };
        draw_text(&key, r.x + 4.0, r.y + r.h - 4.0, 14.0, if group.is_empty() { BORDER } else { KEY });
        if group.is_empty() {
            continue;
        }
        // Colour: the most common unit type in the group – only buildings: the first building – plus the count.
        let count = |kind: UnitKind| group.units.iter().filter(|&&id| unit_kind(world, id) == Some(kind)).count();
        let main = UnitKind::ALL.into_iter().filter(|&kind| count(kind) > 0).max_by_key(|&kind| count(kind));
        let building = group.buildings.iter().find_map(|&id| world.find_building(id)).map(|b| world.buildings[b].kind);
        let color = match (main, building) {
            (Some(kind), _) => kind_color(kind),
            (None, Some(kind)) => building_color(kind),
            (None, None) => BORDER,
        };
        draw_rectangle(r.x + 14.0, r.y + 3.0, 6.0, r.h - 6.0, color);
        draw_right(&group.len().to_string(), r.right() - 3.0, r.y + r.h - 4.0, 14.0, WHITE);
    }
}

fn draw_selection(layout: &Layout, world: &World, ui: &Ui) {
    let view = ui.selection_view(world);
    match view.as_slice() {
        [] => match ui.selected_zombie.and_then(|id| world.find_zombie(id)) {
            Some(z) => draw_zombie(layout, world, &world.zombies[z]),
            None => {
                let inner = layout.info_inner();
                draw_text("Nothing selected", inner.x + 4.0, inner.y + 20.0, 20.0, DIM);
                let hint = "Click the command center to build: Q houses, W resources, E energy, A industry, S military, D defense, Y wonders.";
                draw_wrapped(hint, inner.x + 4.0, inner.y + 46.0, inner.w - 8.0, 15.0, 4, DIM);
            }
        },
        [Pick::Unit(i)] => draw_single(layout, world, *i),
        [Pick::Building(b)] => draw_building(layout, world, &world.buildings[*b]),
        _ => draw_icons(layout, world, ui, &view),
    }
}

/// Ticks → seconds, for showing the times of `data\units.json` the way they are written there.
fn seconds(ticks: u32) -> f32 {
    ticks as f32 / TICKS_PER_SECOND
}

/// The portrait: a coloured field with the letter of the type, `edge` around it.
fn draw_portrait(layout: &Layout, color: Color, letter: &str, edge: Color) {
    let r = layout.portrait();
    draw_rectangle(r.x, r.y, r.w, r.h, color);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, edge);
    draw_centered(letter, r.x + r.w / 2.0, r.y + r.h / 2.0 + 12.0, 36.0, BLACK);
}

/// Name and life bar at the top of the text column. Returns where the next line goes.
fn draw_title(layout: &Layout, name: &str, hp: (u32, u32), shield: (u32, u32)) -> f32 {
    let text = layout.text_column();
    draw_text(name, text.x, text.y + 18.0, 22.0, BRASS);
    let bar = Rect::new(text.x, text.y + 26.0, text.w.min(220.0), 10.0);
    draw_life(bar, hp, shield, BOX_BG, Some(BORDER));
    text.y + 54.0
}

/// The values below the portrait, one box each: a small label above, the number below. As many as fit.
fn draw_cells(layout: &Layout, cells: &[(&str, String)]) {
    let area = layout.cells();
    let cols = ((area.w / CELL_W) as usize).max(1);
    let rows = (area.h / CELL_H) as usize;
    for (k, (label, value)) in cells.iter().take(cols * rows).enumerate() {
        let (x, y) = (area.x + (k % cols) as f32 * CELL_W, area.y + (k / cols) as f32 * CELL_H);
        let r = Rect::new(x, y, CELL_W - 4.0, CELL_H - 4.0);
        draw_rectangle(r.x, r.y, r.w, r.h, BOX_BG);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, BORDER);
        draw_centered(label, r.x + r.w / 2.0, r.y + 12.0, 12.0, DIM);
        draw_fitted(value, r.x + r.w / 2.0, r.bottom() - 5.0, r.w - 4.0, 17.0, GLOW);
    }
}

/// One unit selected: picture, name, health, order, what it is – and its values below.
fn draw_single(layout: &Layout, world: &World, i: usize) {
    let unit = &world.units[i];
    let stats = unit.stats();
    draw_portrait(layout, kind_color(unit.kind), &stats.letter, BORDER);
    let text = layout.text_column();
    let y = draw_title(layout, &stats.name, (unit.hp, stats.hp), (0, 0));
    let order = match unit.queue_len() {
        0 | 1 => format!("Order: {}", unit.order_name()),
        n => format!("Order: {} (+{} queued)", unit.order_name(), n - 1),
    };
    draw_fitted_left(&order, text.x, y, text.w, 15.0, WHITE);
    draw_wrapped(unit_text(unit.kind), text.x, y + 16.0, text.w, 14.0, 1, DIM);

    let mut cells = vec![("HP", format!("{}/{}", unit.hp, stats.hp)), ("Damage", stats.damage.to_string()), ("Armor", stats.armor.to_string())];
    if stats.heals {
        cells.push(("Beam", format!("{}", stats.range)));
    } else {
        cells.push(("Range", format!("{}", stats.range)));
        cells.push(("Shots/s", format!("{:.1}", TICKS_PER_SECOND / stats.shot_ticks() as f32)));
    }
    cells.extend([
        ("Speed", format!("{}", stats.speed)),
        ("Sight", format!("{}", stats.sight)),
        ("Kills", unit.kills.to_string()),
        ("Aim", format!("{:.2} s", seconds(stats.aim))),
        ("Turn", format!("{:.2} s", seconds(stats.turn_around))),
        ("Noise", format!("{}", stats.noise)),
    ]);
    draw_cells(layout, &cells);
}

/// What a unit type is, in one line.
fn unit_text(kind: UnitKind) -> &'static str {
    match kind {
        UnitKind::Soldier => "Tough, short range, aims quickly - and loud.",
        UnitKind::Ranger => "Fast and weak, draws the bow before every arrow, quiet.",
        UnitKind::Sniper => "Long range, one big shot - very loud.",
        UnitKind::Medic => "No attack. Beams at hurt units (healing comes later).",
    }
}

/// A zombie clicked on: picture, health, its values and what it's up to.
fn draw_zombie(layout: &Layout, world: &World, zombie: &Zombie) {
    let stats = zombie.breed.stats();
    draw_portrait(layout, breed_color(zombie.breed), "Z", RED);
    let text = layout.text_column();
    let title = if zombie.final_wave { format!("{} (final wave)", zombie.breed.name()) } else { zombie.breed.name().to_owned() };
    let y = draw_title(layout, &title, (zombie.hp(), stats.hp), (0, 0));
    draw_wrapped(world.zombie_doing(zombie), text.x, y, text.w, 15.0, 2, WHITE);

    let zombies = &data::get().zombies;
    draw_cells(
        layout,
        &[
            ("HP", format!("{}/{}", zombie.hp(), stats.hp)),
            ("Damage", stats.damage.to_string()),
            ("Armor", stats.armor.to_string()),
            ("Hits", format!("{:.1} s", seconds(zombies.hit_cooldown))),
            ("Speed", format!("{:.2}", zombie.speed())),
            ("Sight", format!("{}", stats.sight)),
            ("Hearing", format!("{}", zombies.hearing)),
        ],
    );
}

/// One building selected: picture, health, what it does – for the barracks the production queue, for a tower who
/// sits in it – and its values below.
fn draw_building(layout: &Layout, world: &World, building: &Building) {
    let stats = building.stats();
    draw_portrait(layout, building_color(building.kind), &stats.letter, BORDER);
    let text = layout.text_column();
    let y = draw_title(layout, &stats.name, (building.hp, stats.hp), (building.shield, stats.max_shield()));

    let mut cells = vec![("HP", format!("{}/{}", building.hp, stats.hp))];
    if stats.max_shield() > 0 {
        cells.push(("Shield", format!("{} %", building.shield * 100 / stats.max_shield())));
    }
    if stats.colonists > 0 {
        cells.push(("Colonists", stats.colonists.to_string()));
        cells.push(("Gold/h", format!("+{}", building.income())));
    }
    cells.push(("Sight", format!("{}", stats.sight)));
    let (w, h) = stats.size;
    cells.push(("Size", format!("{w} x {h}")));
    if stats.cost > 0 {
        cells.push(("Cost", stats.cost.to_string()));
    }
    if building.repair_left > 0 {
        cells.insert(1, ("Repair", format!("+{}", building.repair_left)));
    }

    if building.kind == BuildingKind::Tower {
        // One box per spot: the unit in it, or empty.
        let inside = world.garrison(building.id);
        for k in 0..data::get().buildings.tower_slots {
            let r = layout.queue_rect(k);
            match inside.get(k) {
                Some(&i) => {
                    let unit = &world.units[i];
                    draw_rectangle(r.x, r.y, r.w, r.h, kind_color(unit.kind));
                    draw_centered(&unit.stats().letter, r.x + r.w / 2.0, r.y + r.h / 2.0 + 6.0, 20.0, BLACK);
                }
                None => draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, BORDER),
            }
        }
        cells.insert(1, ("Garrison", format!("{}/{}", inside.len(), data::get().buildings.tower_slots)));
        cells.insert(2, ("Rally", if building.rally.is_some() { "set" } else { "none" }.to_owned()));
        draw_cells(layout, &cells);
        return;
    }
    if !building.kind.trains() {
        draw_wrapped(&building_text(building), text.x, y, text.w, 15.0, 3, DIM);
        draw_cells(layout, &cells);
        return;
    }
    // Production queue: 5 spots, the front one with a progress bar. A click cancels.
    for i in 0..MAX_PRODUCTION {
        let r = layout.queue_rect(i);
        match building.production.get(i) {
            Some(&kind) => {
                draw_rectangle(r.x, r.y, r.w, r.h, kind_color(kind));
                draw_centered(&kind.stats().letter, r.x + r.w / 2.0, r.y + r.h / 2.0 + 6.0, 20.0, BLACK);
                if i == 0 {
                    let t = building.progress as f32 / kind.stats().build_time as f32;
                    draw_rectangle(r.x, r.bottom() - 5.0, r.w * t, 5.0, GREEN);
                }
                if r.contains(Vec2::from(mouse_position())) {
                    draw_tooltip(&[&format!("{} - click to cancel", kind.stats().name)], r.x, layout.panel.y - 4.0);
                }
            }
            None => draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, BORDER),
        }
    }
    cells.insert(1, ("Queue", format!("{}/{MAX_PRODUCTION}", building.production.len())));
    cells.insert(2, ("Rally", if building.rally.is_some() { "set" } else { "none" }.to_owned()));
    draw_cells(layout, &cells);
}

/// What a building does, in a sentence or two – like TAB's text next to the portrait.
fn building_text(building: &Building) -> String {
    let stats = building.stats();
    match building.kind {
        BuildingKind::CommandCenter => "The heart of the colony - it builds everything. Lose it and the round is lost.".to_owned(),
        BuildingKind::Wall => "Holds zombies back - they have to tear it down.".to_owned(),
        BuildingKind::Gate => "Your units walk through the middle - zombies have to tear it down.".to_owned(),
        _ if building.infected => "INFECTED - its colonists came out as zombies, everyone walks through it. Repair (F) cures it.".to_owned(),
        _ if stats.colonists > 0 => format!("{} colonists live here and pay gold. Shield down = infected.", stats.colonists),
        _ => "Does nothing yet - but blocks the way.".to_owned(),
    }
}

/// What an icon in the selection shows: type (for Tab), colour, letter, name, hit points, shield.
struct IconInfo {
    sub: Sub,
    color: Color,
    letter: &'static str,
    name: &'static str,
    hp: (u32, u32),
    shield: (u32, u32),
}

fn icon_info(world: &World, pick: Pick) -> IconInfo {
    match pick {
        Pick::Unit(i) => {
            let unit = &world.units[i];
            let stats = unit.stats();
            let (letter, name) = (stats.letter.as_str(), stats.name.as_str());
            IconInfo { sub: Sub::Unit(unit.kind), color: kind_color(unit.kind), letter, name, hp: (unit.hp, stats.hp), shield: (0, 0) }
        }
        Pick::Building(b) => {
            let building = &world.buildings[b];
            let stats = building.stats();
            let (letter, name) = (stats.letter.as_str(), stats.name.as_str());
            let (hp, shield) = ((building.hp, stats.hp), (building.shield, stats.max_shield()));
            IconInfo { sub: Sub::Building(building.kind), color: building_color(building.kind), letter, name, hp, shield }
        }
    }
}

/// Several things selected: one icon each, units first, sorted by type. The active type (Tab) is bright.
fn draw_icons(layout: &Layout, world: &World, ui: &Ui, view: &[Pick]) {
    let capacity = layout.icon_capacity();
    for (k, &pick) in view.iter().take(capacity).enumerate() {
        let info = icon_info(world, pick);
        let r = layout.icon_rect(k);
        let base = info.color;
        let fill = if ui.active == Some(info.sub) { base } else { Color::new(base.r * 0.4, base.g * 0.4, base.b * 0.4, 1.0) };
        let frac = info.hp.0 as f32 / info.hp.1.max(1) as f32;
        draw_rectangle(r.x, r.y, r.w, r.h, fill);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, hp_color(frac));
        draw_centered(info.letter, r.x + r.w / 2.0, r.y + r.h / 2.0 + 6.0, 20.0, BLACK);
        // small life bar at the bottom of the icon
        draw_life(Rect::new(r.x + 3.0, r.bottom() - 6.0, r.w - 6.0, 3.0), info.hp, info.shield, BLACK, None);
    }

    // Tooltip above the icon under the mouse: name and health.
    let mouse = Vec2::from(mouse_position());
    for (k, &pick) in view.iter().take(capacity).enumerate() {
        let r = layout.icon_rect(k);
        if r.contains(mouse) {
            let info = icon_info(world, pick);
            let text = format!("{}   HP {} / {}", info.name, info.hp.0, info.hp.1);
            draw_tooltip(&[&text], r.x, layout.panel.y - 4.0);
        }
    }

    // Summary: how many, which type is active.
    let mut text = format!("{} selected", view.len());
    if view.len() > capacity {
        text += &format!(" (+{} not shown)", view.len() - capacity);
    }
    if let Some(sub) = ui.active {
        text += &format!("   active: {} (Tab)", sub.name());
    }
    let inner = layout.info_inner();
    draw_fitted_left(&text, inner.x, inner.bottom() - 4.0, inner.w, 16.0, DIM);
}

fn draw_card(layout: &Layout, world: &World, ui: &Ui, card: &[Button]) {
    let mouse = Vec2::from(mouse_position());

    // Empty spots.
    for b in 0..CARD_COLS * CARD_ROWS {
        let r = layout.button_rect(b);
        draw_rectangle(r.x, r.y, r.w, r.h, SECTION_BG);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, Color { a: 0.5, ..BORDER });
    }

    let mut hovered = None;
    for &button in card {
        let r = layout.button_rect(button.slot());
        let hover = r.contains(mouse);
        if hover {
            hovered = Some(button);
        }
        let planned = matches!(button, Button::Planned(_));
        // Highlighted: the command waiting for its target, the building on the mouse, the chosen formation.
        let waiting = ui.targeting == Some(button)
            || (button.places().is_some() && ui.placing == button.places())
            || button.formation().is_some_and(|shape| shape == ui.formation);
        let bg = if waiting {
            WAIT_BG
        } else if hover {
            HOVER_BG
        } else if planned {
            SECTION_BG
        } else {
            BOX_BG
        };
        // Too expensive right now: the name dims and the price turns red (like StarCraft's missing minerals).
        // Planned: everything dim – the button only shows where it will be.
        let cost = shown_cost(button, world, ui);
        let affordable = cost.is_none_or(|cost| cost <= world.gold);
        // Repair with nothing to repair: dim like a planned button.
        let idle = button == Button::Repair && cost.is_none();
        let text = if planned || idle || !affordable { DIM } else { GLOW };
        draw_rectangle(r.x, r.y, r.w, r.h, bg);
        let edge = if waiting { GREEN } else if planned { BORDER } else { BRASS };
        draw_rectangle_lines(r.x, r.y, r.w, r.h, if waiting { 2.0 } else { 1.0 }, edge);
        draw_text(button.key_label(), r.x + 4.0, r.y + 15.0, 16.0, if planned { DIM } else { KEY });
        if let Some(cost) = cost {
            let price = cost.to_string();
            draw_right(&price, r.right() - 3.0, r.y + 14.0, 14.0, if affordable { KEY } else { TOO_DEAR });
        } else if planned {
            draw_right("soon", r.right() - 3.0, r.y + 14.0, 12.0, BORDER);
        } else if matches!(button, Button::Open(_)) {
            draw_right(">", r.right() - 4.0, r.y + 15.0, 16.0, BRASS);
        }
        draw_fitted(button.name(), r.x + r.w / 2.0, r.y + r.h - 9.0, r.w - 6.0, 15.0, text);
    }

    // Tooltip above the command card, like in StarCraft – with the price.
    if let Some(button) = hovered {
        let title = match (shown_cost(button, world, ui), button) {
            (Some(cost), _) => format!("{} ({}) - {cost} gold", button.title(), button.key_label()),
            (None, Button::Repair) => {
                let plan = world.repair_plan(ui.selected_building_indices(world));
                format!("{} ({}) - {}", button.title(), button.key_label(), nothing_to_repair(&plan))
            }
            (None, Button::Planned(_)) => format!("{} ({}) - planned, not in the game yet", button.title(), button.key_label()),
            (None, _) => format!("{} ({})", button.title(), button.key_label()),
        };
        draw_tooltip(&[&title, button.help()], layout.card.x, layout.panel.y - 4.0);
    }
}

/// The price on a button: what it trains or places – for Repair what repairing the selection costs right now
/// (`None`: nothing that may be repaired).
fn shown_cost(button: Button, world: &World, ui: &Ui) -> Option<u32> {
    match button {
        Button::Repair => repair_price(&world.repair_plan(ui.selected_building_indices(world))),
        _ => button.cost(),
    }
}

/// The price of a repair plan – `None` if nothing in it may be repaired.
fn repair_price(plan: &RepairPlan) -> Option<u32> {
    (plan.damaged > plan.blocked).then_some(plan.price)
}

/// Why a repair plan has no price: nothing damaged, or zombies too close.
fn nothing_to_repair(plan: &RepairPlan) -> &'static str {
    if plan.blocked > 0 { "zombies too close" } else { "nothing to repair" }
}

/// "Repair all" (X) in the stock: like a button of the card. The plan (all buildings) is only worked out while the
/// mouse is on it – with thousands of zombies it isn't free.
fn draw_repair_all(layout: &Layout, world: &World) {
    let r = layout.repair_all_rect();
    let hover = r.contains(Vec2::from(mouse_position()));
    draw_rectangle(r.x, r.y, r.w, r.h, if hover { HOVER_BG } else { BOX_BG });
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, BRASS);
    draw_text("X", r.x + 4.0, r.y + 13.0, 14.0, KEY);
    draw_fitted("Repair all", r.x + r.w / 2.0, r.y + r.h - 7.0, r.w - 6.0, 14.0, GLOW);
    if hover {
        let plan = world.repair_plan(0..world.buildings.len());
        let title = match repair_price(&plan) {
            Some(price) => format!("Repair all (X) - {price} gold"),
            None => format!("Repair all (X) - {}", nothing_to_repair(&plan)),
        };
        let help = "Every damaged building without a zombie near it - cheapest first, as many as the gold pays for. \
                    X works while no units are selected (then X is the Line formation).";
        draw_tooltip(&[&title, help], layout.stock.x, layout.panel.y - 4.0);
    }
}

/// The colony's stock on the right, like TAB's: people on the left, goods with a bar on the right. Only colonists
/// and gold exist yet – the rest is shown dim until the economy comes (DESIGN.md 5).
fn draw_stock(layout: &Layout, world: &World) {
    let r = layout.stock;
    let mouse = Vec2::from(mouse_position());
    let row_h = (r.h - 12.0) / 5.0;
    let row = |k: usize| r.y + 6.0 + k as f32 * row_h;
    let (left_x, left_w) = (r.x + 10.0, 74.0);
    let (right_x, right_w) = (left_x + left_w + 8.0, r.right() - 10.0 - (left_x + left_w + 8.0));
    let mut tooltip: Option<[&str; 2]> = None;

    // People: colonists (real), the rest later.
    let colonists: u32 = world.buildings.iter().filter(|b| !b.infected).map(|b| b.stats().colonists).sum();
    let people: [(&str, Option<String>, &str); 4] = [
        ("Colonists", Some(colonists.to_string()), "The colonists living in your houses - each pays gold every hour."),
        ("Workers", None, "Colonists working in buildings - comes with the economy."),
        ("Food", None, "Feeds the houses - comes with the economy."),
        ("Energy", None, "Powers houses, machines and towers - comes with the economy."),
    ];
    for (k, (label, value, help)) in people.into_iter().enumerate() {
        let y = row(k);
        draw_text(label, left_x, y + 13.0, 13.0, DIM);
        let (text, color) = match value {
            Some(v) => (v, GLOW),
            None => ("-".to_owned(), BORDER),
        };
        draw_text(&text, left_x, y + 31.0, 19.0, color);
        if Rect::new(left_x, y, left_w, row_h).contains(mouse) {
            tooltip = Some([label, help]);
        }
    }

    // Goods: gold (real) with what comes in per hour, the rest later.
    let gold_help = "Pays for units and buildings. The houses pay it every hour.";
    let goods: [(&str, Option<u32>, &str); 5] = [
        ("Gold", Some(world.gold), gold_help),
        ("Wood", None, "From the saw mill - comes with the economy."),
        ("Stone", None, "From the quarry - comes with the economy."),
        ("Iron", None, "From the foundry - comes with the economy."),
        ("Oil", None, "From oil pumps on the oil nodes - comes with the economy."),
    ];
    for (k, (label, value, help)) in goods.into_iter().enumerate() {
        let y = row(k);
        draw_text(label, right_x, y + 13.0, 13.0, DIM);
        if k == 0 {
            draw_right(&format!("+{}/h", world.income()), right_x + right_w, y + 13.0, 13.0, GLOW);
        }
        let bar = Rect::new(right_x, y + 17.0, right_w, 16.0);
        draw_rectangle(bar.x, bar.y, bar.w, bar.h, BOX_BG);
        draw_rectangle_lines(bar.x, bar.y, bar.w, bar.h, 1.0, BORDER);
        match value {
            Some(v) => draw_centered(&v.to_string(), bar.x + bar.w / 2.0, bar.bottom() - 3.0, 16.0, WHITE),
            None => draw_centered("-", bar.x + bar.w / 2.0, bar.bottom() - 3.0, 16.0, BORDER),
        }
        if Rect::new(right_x, y, right_w, row_h).contains(mouse) {
            tooltip = Some([label, help]);
        }
    }
    if let Some(lines) = tooltip {
        draw_tooltip(&lines, r.x, layout.panel.y - 4.0);
    }
}

/// A box with text above the panel. `x`: left edge (or right edge if there's not enough room),
/// `bottom`: bottom edge. The first line is bright, the rest grey.
fn draw_tooltip(lines: &[&str], x: f32, bottom: f32) {
    let line_h = 20.0;
    let w = lines.iter().map(|line| measure_text(line, None, 18, 1.0).width).fold(0.0, f32::max) + 16.0;
    let h = lines.len() as f32 * line_h + 8.0;
    let x = if x + w > screen_width() { (screen_width() - w).max(0.0) } else { x };
    let y = bottom - h;
    draw_rectangle(x, y, w, h, PANEL_BG);
    draw_rectangle_lines(x, y, w, h, 1.0, BRASS);
    for (k, line) in lines.iter().enumerate() {
        let color = if k == 0 { WHITE } else { DIM };
        draw_text(line, x + 8.0, y + 20.0 + k as f32 * line_h, 18.0, color);
    }
}

/// The gray-blue of a shield – the part of the life bar behind the hit points.
pub const SHIELD: Color = Color::new(0.50, 0.58, 0.70, 1.0);

/// One life bar for hit points and shield (`(now, max)` each): the whole bar is both together, first the hit points
/// in their colour (green, yellow, red), then what's left of the shield in gray-blue. Without a shield it's a plain
/// health bar. `border`: a frame around it (the panel), or none (above a building).
pub fn draw_life(bar: Rect, hp: (u32, u32), shield: (u32, u32), background: Color, border: Option<Color>) {
    let total = (hp.1 + shield.1).max(1) as f32;
    let hp_w = bar.w * hp.0 as f32 / total;
    let shield_w = bar.w * shield.0 as f32 / total;
    draw_rectangle(bar.x, bar.y, bar.w, bar.h, background);
    draw_rectangle(bar.x, bar.y, hp_w, bar.h, hp_color(hp.0 as f32 / hp.1.max(1) as f32));
    draw_rectangle(bar.x + hp_w, bar.y, shield_w, bar.h, SHIELD);
    if let Some(border) = border {
        draw_rectangle_lines(bar.x, bar.y, bar.w, bar.h, 1.0, border);
    }
}

/// Green at full health, yellow from half, red below a quarter.
pub fn hp_color(frac: f32) -> Color {
    if frac > 0.5 {
        GREEN
    } else if frac > 0.25 {
        YELLOW
    } else {
        RED
    }
}

fn unit_kind(world: &World, id: u32) -> Option<UnitKind> {
    world.find_unit(id).map(|i| world.units[i].kind)
}

fn text_width(text: &str, size: f32) -> f32 {
    measure_text(text, None, size as u16, 1.0).width
}

/// Text centred around x, baseline at y.
fn draw_centered(text: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(text, x - text_width(text, size) / 2.0, y, size, color);
}

/// Text ending at x, baseline at y.
fn draw_right(text: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(text, x - text_width(text, size), y, size, color);
}

/// The font size at which `text` fits into `max_w` – `size`, or smaller.
fn fitted_size(text: &str, max_w: f32, size: f32) -> f32 {
    let w = text_width(text, size);
    if w > max_w { size * max_w / w } else { size }
}

/// Text centred around x – gets smaller if it doesn't fit into `max_w`.
fn draw_fitted(text: &str, x: f32, y: f32, max_w: f32, size: f32, color: Color) {
    draw_centered(text, x, y, fitted_size(text, max_w, size), color);
}

/// Text starting at x – gets smaller if it doesn't fit into `max_w`.
fn draw_fitted_left(text: &str, x: f32, y: f32, max_w: f32, size: f32, color: Color) {
    draw_text(text, x, y, fitted_size(text, max_w, size), color);
}

/// Text broken into lines of at most `max_w`, the first baseline at y – at most `max_lines` lines, the last one
/// ends with "..." if the text goes on.
fn draw_wrapped(text: &str, x: f32, y: f32, max_w: f32, size: f32, max_lines: usize, color: Color) {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if text_width(&format!("{line} {word}"), size) <= max_w => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            last.push_str(" ...");
        }
    }
    for (k, line) in lines.iter().enumerate() {
        draw_text(line, x, y + k as f32 * (size + 2.0), size, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every card: no two buttons on the same spot (they'd share a key and one would hide the other).
    #[test]
    fn no_two_buttons_share_a_spot() {
        let mut cards: Vec<Vec<Button>> =
            vec![UNIT_BUTTONS.to_vec(), COMMAND_CENTER_BUTTONS.to_vec(), BARRACKS_BUTTONS.to_vec(), TOWER_BUTTONS.to_vec()];
        let menus = [Submenu::Colonists, Submenu::Resources, Submenu::Energy, Submenu::Industry, Submenu::Military, Submenu::Defense, Submenu::Wonders];
        cards.extend(menus.map(menu_buttons));
        for card in &cards {
            for (k, a) in card.iter().enumerate() {
                for b in &card[k + 1..] {
                    assert_ne!(a.slot(), b.slot(), "{a:?} and {b:?} on the same spot");
                }
            }
        }
    }
}
