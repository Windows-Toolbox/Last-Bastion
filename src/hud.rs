//! The panel at the bottom, like in StarCraft (DESIGN.md 11.1): minimap on the left, control groups and selection
//! in the middle, command card on the right. Here is where everything lies (`Layout`), what it looks like, and what
//! is under the mouse (`hit`). What a click does is decided by `ui.rs`.

use macroquad::prelude::*;

use crate::draw::{building_color, kind_color, ZOMBIE};
use crate::formation::Shape;
use crate::iso::{iso, iso_inverse, Camera, TILE_W};
use crate::editor::Sight;
use crate::data;
use crate::kinds::{BuildingKind, UnitKind};
use crate::map::Region;
use crate::picture::MapPicture;
use crate::sim::{Building, World, MAX_PRODUCTION, TICKS_PER_SECOND};
use crate::ui::{Pick, Sub, Ui};

pub const PANEL_H: f32 = 200.0;
const PAD: f32 = 8.0;
const MINIMAP_W: f32 = 320.0; // the minimap is a diamond, twice as wide as high
const GROUP_H: f32 = 26.0;
const GROUP_W: f32 = 58.0;
const ICON: f32 = 40.0;
const QUEUE_ICON: f32 = 44.0;
const SUMMARY_H: f32 = 22.0;
const BUTTON: f32 = 64.0;
const CARD_COLS: usize = 5;
const CARD_ROWS: usize = 3;

const PANEL_BG: Color = Color::new(0.08, 0.09, 0.11, 0.95);
const BOX_BG: Color = Color::new(0.13, 0.14, 0.17, 1.0);
const BORDER: Color = Color::new(0.30, 0.32, 0.38, 1.0);
const DIM: Color = Color::new(0.55, 0.57, 0.62, 1.0);

/// The buttons of the command card. Which ones are visible depends on the selection (`Ui::card`).
#[derive(Clone, Copy, PartialEq)]
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
    // Command center
    TrainSoldier,
    TrainRanger,
    TrainSniper,
    TrainMedic,
    Rally,
    PlaceTent,
    /// Open one of the build menus – the card then shows its buildings (`Ui::submenu`).
    Open(Submenu),
    /// Back from a build menu to the command center's buttons (Esc).
    Back,
    PlaceWall,
    PlaceGate,
    PlaceTower,
    // Tower
    Unload,
}

/// The build menus of the command center, like StarCraft's "build structure" pages.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Submenu {
    /// D: walls, gates, towers.
    Defense,
    /// E: power plants and the grid (DESIGN.md 5.5) – nothing yet.
    Energy,
    /// A: special buildings (DESIGN.md 6) – nothing yet.
    Special,
}

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
/// The buttons of the command center (StarCraft's grid layout: the key is where the button sits).
pub const COMMAND_CENTER_BUTTONS: [Button; 9] = [
    Button::TrainSoldier,
    Button::TrainRanger,
    Button::Open(Submenu::Energy),
    Button::TrainSniper,
    Button::TrainMedic,
    Button::Open(Submenu::Special),
    Button::PlaceTent,
    Button::Open(Submenu::Defense),
    Button::Rally,
];
/// The defense build menu (D).
pub const DEFENSE_BUTTONS: [Button; 4] = [Button::PlaceWall, Button::PlaceGate, Button::PlaceTower, Button::Back];
/// The buttons of a tower.
pub const TOWER_BUTTONS: [Button; 2] = [Button::Unload, Button::Rally];

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
            Button::PlaceTent => "Tent",
            Button::Open(Submenu::Defense) => "Defense",
            Button::Open(Submenu::Energy) => "Energy",
            Button::Open(Submenu::Special) => "Special",
            Button::Back => "Back",
            Button::PlaceWall => "Wall",
            Button::PlaceGate => "Gate",
            Button::PlaceTower => "Tower",
            Button::Unload => "Unload",
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
            Button::Back => KeyCode::Escape,
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
            Button::Back => "Esc",
            _ => GRID_LABELS[self.slot()],
        }
    }

    /// Spot on the command card: 0–4 top row, 5–9 middle, 10–14 bottom. For buildings this also decides the key.
    fn slot(self) -> usize {
        match self {
            Button::Move | Button::TrainSoldier | Button::Unload | Button::PlaceWall => 0,
            Button::Stop | Button::TrainRanger | Button::PlaceGate => 1,
            Button::Hold | Button::Open(Submenu::Energy) | Button::PlaceTower => 2,
            Button::Patrol | Button::TrainSniper => 3,
            Button::Attack | Button::TrainMedic => 4,
            Button::Open(Submenu::Special) => 5,
            Button::PlaceTent => 6,
            Button::Open(Submenu::Defense) => 7,
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

    /// Which building this button puts on the grid – `None` for all other buttons.
    pub fn places(self) -> Option<BuildingKind> {
        match self {
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
            Button::PlaceTent => "Tent (2 x 2). 6 colonists pay gold; shield down = infected. Drag: a line. Right click / Esc: stop building.",
            Button::Open(Submenu::Defense) => "Defense buildings: wall, gate, tower.",
            Button::Open(Submenu::Energy) => "Energy buildings - none yet (the power grid comes with the economy).",
            Button::Open(Submenu::Special) => "Special buildings - none yet.",
            Button::Back => "Back to the command center.",
            Button::PlaceWall => "Wall (1 x 1, 400 HP). Zombies have to tear it down. Drag: a line, diagonal too. Right click / Esc: stop building.",
            Button::PlaceGate => "Gate (3 x 1, 400 HP). Your units walk through the middle, zombies don't. Can go on walls (cheaper there, but walls + gate still cost more than the gate alone). R: turn. Right click / Esc: stop building.",
            Button::PlaceTower => "Tower (1 x 1, 600 HP). Right click it with units: up to 4 go in, shoot and see 2 tiles further and can't be hit. Needs 1 free tile to the next tower. Drag: a line.",
            Button::Unload => "Everyone out of the tower - on the side of the rally point, then they walk there.",
        }
    }
}

/// What lies under the mouse.
#[derive(Clone, Copy)]
pub enum HudHit {
    /// A point on the minimap – already converted to world coordinates.
    Minimap(Vec2),
    /// A control group (0 = key 1 … 9 = key 0).
    Group(usize),
    /// An icon in the selection – the index in `Ui::selection_view` (units and buildings).
    Icon(usize),
    /// An entry in the production queue of the selected building.
    Queue(usize),
    Button(Button),
    /// Anywhere else on a panel.
    Panel,
}

/// Where everything lies on screen. Computed anew every frame, in case the window changes size.
pub struct Layout {
    pub panel: Rect,
    minimap: Rect,
    groups: Rect,
    icons: Rect,
    summary: Rect,
    card: Rect,
}

impl Layout {
    pub fn new() -> Layout {
        let (w, h) = (screen_width(), screen_height());
        let panel = Rect::new(0.0, h - PANEL_H, w, PANEL_H);
        let minimap_h = MINIMAP_W / 2.0;
        let minimap = Rect::new(PAD, panel.y + (PANEL_H - minimap_h) / 2.0, MINIMAP_W, minimap_h);
        let card_w = CARD_COLS as f32 * BUTTON;
        let card = Rect::new(w - PAD - card_w, panel.y + PAD, card_w, CARD_ROWS as f32 * BUTTON);
        let mid_x = minimap.right() + 2.0 * PAD;
        let mid_w = (card.x - 2.0 * PAD - mid_x).max(0.0);
        let groups = Rect::new(mid_x, panel.y + PAD, mid_w, GROUP_H);
        let info_y = groups.bottom() + PAD;
        let info_h = panel.bottom() - PAD - info_y;
        let icons = Rect::new(mid_x, info_y, mid_w, info_h - SUMMARY_H);
        let summary = Rect::new(mid_x, icons.bottom(), mid_w, SUMMARY_H);
        Layout { panel, minimap, groups, icons, summary, card }
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

    fn group_rect(&self, g: usize) -> Rect {
        let w = (self.groups.w / 10.0).min(GROUP_W);
        Rect::new(self.groups.x + g as f32 * w, self.groups.y, w - 4.0, self.groups.h)
    }

    fn icon_columns(&self) -> usize {
        ((self.icons.w / ICON) as usize).max(1)
    }

    /// How many icons fit into the field.
    fn icon_capacity(&self) -> usize {
        self.icon_columns() * (self.icons.h / ICON) as usize
    }

    fn icon_rect(&self, i: usize) -> Rect {
        let cols = self.icon_columns();
        let (col, row) = ((i % cols) as f32, (i / cols) as f32);
        Rect::new(self.icons.x + col * ICON, self.icons.y + row * ICON, ICON - 4.0, ICON - 4.0)
    }

    /// The production queue lies to the right of the building's portrait.
    fn queue_rect(&self, i: usize) -> Rect {
        let x = self.icons.x + 126.0 + i as f32 * QUEUE_ICON;
        Rect::new(x, self.icons.y + 78.0, QUEUE_ICON - 4.0, QUEUE_ICON - 4.0)
    }

    fn button_rect(&self, b: usize) -> Rect {
        let (col, row) = ((b % CARD_COLS) as f32, (b / CARD_COLS) as f32);
        Rect::new(self.card.x + col * BUTTON, self.card.y + row * BUTTON, BUTTON - 4.0, BUTTON - 4.0)
    }
}

/// The clock of the round at the top right, next to the Game View / Editor Mode buttons – the gold and what comes in
/// per hour, day and hour like in TAB, and how long until the next wave of the editor's timeline.
pub fn draw_clock(world: &World) {
    let (day, hour) = world.clock();
    let mut text = format!("Gold {} (+{}/h)   Day {day}  {hour:02}:00", world.gold, world.income());
    if let Some(next) = world.waves.iter().map(|w| w.at).filter(|&at| at > world.tick).min() {
        let seconds = (next - world.tick) / 20;
        text += &format!("   wave in {}:{:02}", seconds / 60, seconds % 60);
    }
    let w = measure_text(&text, None, 20, 1.0).width + 24.0;
    // Left of the editor panel (270 wide + margins), so it neither covers it nor the stats in the top left.
    let x = screen_width() - 270.0 - 16.0 - 8.0 - w;
    draw_rectangle(x, 6.0, w, 28.0, Color::new(0.08, 0.09, 0.11, 0.85));
    draw_rectangle_lines(x, 6.0, w, 28.0, 1.0, BORDER);
    draw_text(&text, x + 12.0, 26.0, 20.0, WHITE);
}

/// Draws the whole panel. `minimap`: the picture of the map (`picture.rs`), kept up to date by `main.rs`.
pub fn draw(layout: &Layout, world: &World, camera: &Camera, ui: &Ui, minimap: &MapPicture) {
    let p = layout.panel;
    draw_rectangle(p.x, p.y, p.w, p.h, PANEL_BG);
    draw_line(p.x, p.y, p.right(), p.y, 2.0, BORDER);

    draw_minimap(layout, world, camera, ui, minimap);
    draw_groups(layout, world, ui);
    draw_selection(layout, world, ui);
    draw_card(layout, ui, &ui.card(world));
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
    // "View: Zombies" the map stays readable (`main.rs` paints the picture that way).
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

fn draw_groups(layout: &Layout, world: &World, ui: &Ui) {
    for (g, group) in ui.groups.iter().enumerate() {
        let r = layout.group_rect(g);
        let selected = !group.is_empty() && group.units == ui.selected && group.buildings == ui.selected_buildings;
        draw_rectangle(r.x, r.y, r.w, r.h, if group.is_empty() { PANEL_BG } else { BOX_BG });
        draw_rectangle_lines(r.x, r.y, r.w, r.h, if selected { 2.0 } else { 1.0 }, if selected { WHITE } else { BORDER });

        let key = if g == 9 { "0".to_owned() } else { (g + 1).to_string() };
        draw_text(&key, r.x + 4.0, r.y + 12.0, 14.0, DIM);
        if group.is_empty() {
            continue;
        }
        // Icon: the most common unit type in the group – only buildings: the first building – plus the count.
        let count = |kind: UnitKind| group.units.iter().filter(|&&id| unit_kind(world, id) == Some(kind)).count();
        let main = UnitKind::ALL.into_iter().filter(|&kind| count(kind) > 0).max_by_key(|&kind| count(kind));
        let building = group.buildings.iter().find_map(|&id| world.find_building(id)).map(|b| world.buildings[b].kind);
        let color = match (main, building) {
            (Some(kind), _) => kind_color(kind),
            (None, Some(kind)) => building_color(kind),
            (None, None) => BORDER,
        };
        draw_rectangle(r.x + 16.0, r.y + 5.0, 14.0, r.h - 10.0, color);
        draw_text(&group.len().to_string(), r.x + 34.0, r.y + r.h - 7.0, 18.0, WHITE);
    }
}

fn draw_selection(layout: &Layout, world: &World, ui: &Ui) {
    let view = ui.selection_view(world);
    match view.as_slice() {
        [] => {
            draw_text("Nothing selected", layout.icons.x + 8.0, layout.icons.y + 24.0, 20.0, DIM);
        }
        [Pick::Unit(i)] => draw_single(layout, world, *i),
        [Pick::Building(b)] => draw_building(layout, world, &world.buildings[*b]),
        _ => draw_icons(layout, world, ui, &view),
    }
}

/// Ticks → seconds, for showing the times of `data\units.json` the way they are written there.
fn seconds(ticks: u32) -> f32 {
    ticks as f32 / TICKS_PER_SECOND
}

/// One unit selected: big picture and all values.
fn draw_single(layout: &Layout, world: &World, i: usize) {
    let unit = &world.units[i];
    let stats = unit.stats();
    let area = Rect::new(layout.icons.x, layout.icons.y, layout.icons.w, layout.icons.h + SUMMARY_H);

    // Portrait: coloured field with the letter of the type.
    let size = area.h.min(110.0);
    draw_rectangle(area.x, area.y, size, size, kind_color(unit.kind));
    draw_rectangle_lines(area.x, area.y, size, size, 2.0, BORDER);
    draw_centered(&stats.letter, area.x + size / 2.0, area.y + size / 2.0 + 12.0, 40.0, BLACK);

    let x = area.x + size + 16.0;
    let mut y = area.y + 18.0;
    draw_text(&stats.name, x, y, 24.0, WHITE);
    y += 12.0;

    // Health bar
    let bar_w = (area.right() - x).min(220.0);
    let frac = unit.hp as f32 / stats.hp as f32;
    draw_rectangle(x, y, bar_w, 12.0, BOX_BG);
    draw_rectangle(x, y, bar_w * frac, 12.0, hp_color(frac));
    draw_rectangle_lines(x, y, bar_w, 12.0, 1.0, BORDER);
    y += 30.0;
    draw_text(&format!("HP {} / {}", unit.hp, stats.hp), x, y, 18.0, WHITE);

    let lines = [
        format!("Damage {}   Armor {}", stats.damage, stats.armor),
        if stats.heals {
            format!("Beam range {}   (healing comes later)", stats.range)
        } else {
            format!("Range {}   Shots per s {:.1}   Aim {:.2} s", stats.range, 20.0 / stats.shot_ticks() as f32, seconds(stats.aim))
        },
        format!("Speed {}   Turn {:.2} s   Kills {}", stats.speed, seconds(stats.turn_around), unit.kills),
        match unit.queue_len() {
            0 | 1 => format!("Order: {}", unit.order_name()),
            n => format!("Order: {} (+{} queued)", unit.order_name(), n - 1),
        },
    ];
    for line in &lines {
        y += 20.0;
        draw_text(line, x, y, 18.0, DIM);
    }
}

/// One building selected: picture, health and – for the command center – the production queue, for a tower
/// who sits in it.
fn draw_building(layout: &Layout, world: &World, building: &Building) {
    let stats = building.stats();
    let area = Rect::new(layout.icons.x, layout.icons.y, layout.icons.w, layout.icons.h + SUMMARY_H);
    let size = area.h.min(110.0);
    draw_rectangle(area.x, area.y, size, size, building_color(building.kind));
    draw_rectangle_lines(area.x, area.y, size, size, 2.0, BORDER);
    draw_centered(&stats.letter, area.x + size / 2.0, area.y + size / 2.0 + 12.0, 36.0, BLACK);

    let x = area.x + size + 16.0;
    draw_text(&stats.name, x, area.y + 18.0, 24.0, WHITE);
    let bar_w = (area.right() - x).min(220.0);
    let bar = Rect::new(x, area.y + 30.0, bar_w, 12.0);
    draw_life(bar, (building.hp, stats.hp), (building.shield, stats.max_shield()), BOX_BG, Some(BORDER));
    let mut text = format!("HP {} / {}", building.hp, stats.hp);
    if stats.max_shield() > 0 {
        text += &format!("   Shield {} %", building.shield * 100 / stats.max_shield());
    }
    draw_text(&text, x, area.y + 62.0, 18.0, WHITE);

    if building.kind == BuildingKind::Tower {
        // One box per spot: the unit in it, or empty.
        let inside = world.garrison(building.id);
        let text = format!("Garrison {} / {}   (right click with units: go in)", inside.len(), data::get().buildings.tower_slots);
        draw_text(&text, x, area.y + 90.0, 18.0, DIM);
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
        let rally = if building.rally.is_some() { "Rally point set - Q unloads towards it" } else { "No rally point (G or right click)" };
        draw_text(rally, layout.queue_rect(0).x, layout.queue_rect(0).bottom() + 22.0, 18.0, DIM);
        return;
    }
    if building.kind != BuildingKind::CommandCenter {
        let note = match building.kind {
            BuildingKind::Wall => "Holds zombies back - they have to tear it down.".to_owned(),
            BuildingKind::Gate => "Your units walk through the middle - zombies have to tear it down.".to_owned(),
            _ if building.infected => "INFECTED - its colonists came out as zombies.".to_owned(),
            _ if stats.colonists > 0 => format!("{} colonists, +{} gold per hour. Shield down = infected.", stats.colonists, building.income()),
            _ => "Does nothing yet - but blocks the way.".to_owned(),
        };
        draw_text(&note, x, area.y + 90.0, 18.0, DIM);
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
    let rally = if building.rally.is_some() { "Rally point set" } else { "No rally point (G or right click)" };
    draw_text(rally, layout.queue_rect(0).x, layout.queue_rect(0).bottom() + 22.0, 18.0, DIM);
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
        text += &format!("   active: {} (Tab switches)", sub.name());
    }
    draw_text(&text, layout.summary.x, layout.summary.y + 16.0, 18.0, DIM);
}

fn draw_card(layout: &Layout, ui: &Ui, card: &[Button]) {
    let mouse = Vec2::from(mouse_position());

    // Empty spots (later abilities and the build menu).
    for b in 0..CARD_COLS * CARD_ROWS {
        let r = layout.button_rect(b);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, BOX_BG);
    }

    let mut hovered = None;
    for &button in card {
        let r = layout.button_rect(button.slot());
        let hover = r.contains(mouse);
        if hover {
            hovered = Some(button);
        }
        // Highlighted: the command waiting for its target, the building on the mouse, the chosen formation.
        let waiting = ui.targeting == Some(button)
            || (button.places().is_some() && ui.placing == button.places())
            || button.formation().is_some_and(|shape| shape == ui.formation);
        let bg = if waiting {
            Color::new(0.25, 0.35, 0.20, 1.0)
        } else if hover {
            Color::new(0.20, 0.22, 0.27, 1.0)
        } else {
            BOX_BG
        };
        let text = WHITE;
        draw_rectangle(r.x, r.y, r.w, r.h, bg);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, if waiting { 2.0 } else { 1.0 }, if waiting { GREEN } else { BORDER });
        draw_text(button.key_label(), r.x + 5.0, r.y + 16.0, 18.0, YELLOW);
        draw_fitted(button.name(), r.x + r.w / 2.0, r.y + r.h - 9.0, r.w - 6.0, text);
    }

    // Tooltip above the command card, like in StarCraft.
    if let Some(button) = hovered {
        let title = format!("{} ({})", button.name(), button.key_label());
        draw_tooltip(&[&title, button.help()], layout.card.right(), layout.panel.y - 4.0);
    }
}

/// A box with text above the panel. `x`: left edge (or right edge if there's not enough room),
/// `bottom`: bottom edge. The first line is bright, the rest grey.
fn draw_tooltip(lines: &[&str], x: f32, bottom: f32) {
    let line_h = 20.0;
    let w = lines.iter().map(|line| measure_text(line, None, 18, 1.0).width).fold(0.0, f32::max) + 16.0;
    let h = lines.len() as f32 * line_h + 8.0;
    let x = if x + w > screen_width() { (x - w).max(0.0) } else { x };
    let y = bottom - h;
    draw_rectangle(x, y, w, h, PANEL_BG);
    draw_rectangle_lines(x, y, w, h, 1.0, BORDER);
    for (k, line) in lines.iter().enumerate() {
        let color = if k == 0 { WHITE } else { DIM };
        draw_text(line, x + 8.0, y + 20.0 + k as f32 * line_h, 18.0, color);
    }
}

/// Green at full health, yellow from half, red below a quarter.
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

/// Text centred around x, baseline at y.
fn draw_centered(text: &str, x: f32, y: f32, size: f32, color: Color) {
    let w = measure_text(text, None, size as u16, 1.0).width;
    draw_text(text, x - w / 2.0, y, size, color);
}

/// Text centred around x – gets smaller if it doesn't fit into `max_w`.
fn draw_fitted(text: &str, x: f32, y: f32, max_w: f32, color: Color) {
    let mut size = 16.0;
    let w = measure_text(text, None, size as u16, 1.0).width;
    if w > max_w {
        size *= max_w / w;
    }
    draw_centered(text, x, y, size, color);
}
