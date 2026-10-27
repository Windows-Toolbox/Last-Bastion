//! Input: mouse and keyboard → selection, camera and orders – like in StarCraft 2 (DESIGN.md 7.1, 11.2).
//! The simulation is never changed directly here – only through the order list (DESIGN.md 12.8).
//! Selection and control groups remember the `id`s of units and buildings, because both can die. Like in StarCraft a
//! selection can hold units and buildings together; Tab switches which type is "in front" (`Sub`).

use macroquad::prelude::*;

use crate::cursor::Cursor;
use crate::data;
use crate::draw::{draw_diamond, draw_ring, mark_color, ore_color, terrain_color, SpawnerShape, TESLA_GLOW, ZOMBIE};
use crate::editor::{Editor, Sight, Tool};
use crate::formation::{self, Formation, Shape, MIN_DRAG};
use crate::hud::{building_buttons, menu_buttons, Button, HudHit, Layout, Submenu, COMMAND_CENTER_BUTTONS, UNIT_BUTTONS};
use crate::iso::{Camera, TILE_H, TILE_W};
use crate::keys::Action;
use crate::kinds::{BuildingKind, Resource, UnitKind};
use crate::map::{to_frame, Ore, Region, Terrain};
use crate::placement::{grid_line, placement_line, placement_tile};
use crate::settings::Settings;
use crate::sim::{spawner_corners, Axes, Breed, Command, Mark, Order, Rally, SwitchBlock, World, MAX_PRODUCTION, UNIT_RADIUS};

/// One entry of the selection, as an index into the world's list of units or buildings.
#[derive(Clone, Copy, PartialEq)]
pub enum Pick {
    Unit(usize),
    Building(usize),
}

/// A type in the selection – what Tab switches between (StarCraft's subgroups). Units come first (in `UnitKind`
/// order), then buildings. The command card shows the buttons of the active one.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Sub {
    Unit(UnitKind),
    Building(BuildingKind),
}

impl Sub {
    pub fn name(self) -> &'static str {
        match self {
            Sub::Unit(kind) => &kind.stats().name,
            Sub::Building(kind) => &kind.stats().name,
        }
    }
}

/// A control group: units and buildings (their `id`s).
#[derive(Clone, Default, PartialEq)]
pub struct Group {
    pub units: Vec<u32>,
    pub buildings: Vec<u32>,
}

impl Group {
    pub fn is_empty(&self) -> bool {
        self.units.is_empty() && self.buildings.is_empty()
    }

    pub fn len(&self) -> usize {
        self.units.len() + self.buildings.len()
    }
}

/// What a line of buildings would really build (`Ui::affordable`).
struct LinePlan {
    /// The pieces that get built (their top left tiles).
    built: Vec<(i32, i32)>,
    /// What they cost together: gold and workers.
    cost: u32,
    workers: u32,
    /// Some fit, but there wasn't enough of this for them.
    short: Option<Short>,
}

/// What ran out – or what has to stand first.
#[derive(Clone, Copy)]
enum Short {
    Gold,
    /// Not enough gold – but the banks' credit would pay for it, with Shift.
    GoldCredit,
    /// No barrel in the munitions factories' store.
    Barrels,
    Wood,
    Stone,
    Coal,
    Iron,
    Workers,
    /// Not enough spare power – switching a building on again (Y).
    Power,
    /// The gold coming in doesn't cover the upkeep – switching a building on again (Y).
    Income,
    /// This building has to stand first (`requires`).
    Locked(BuildingKind),
}

impl Short {
    fn alert(self) -> String {
        match self {
            Short::Gold => "Not enough gold".to_owned(),
            Short::GoldCredit => "Not enough gold - hold Shift to buy on the banks' credit".to_owned(),
            Short::Barrels => "No barrel in store - make some in the munitions factory".to_owned(),
            Short::Wood => "Not enough wood - build a sawmill".to_owned(),
            Short::Stone => "Not enough stone - build a mine on a stone patch".to_owned(),
            Short::Coal => "Not enough coal - build a mine on a coal patch".to_owned(),
            Short::Iron => "Not enough iron - the foundry smelts it from iron ore and coal".to_owned(),
            Short::Workers => "Not enough workers - build tents".to_owned(),
            Short::Power => "Not enough power - build a windmill or a charcoal burner".to_owned(),
            Short::Income => "Not enough gold income for its upkeep - build houses".to_owned(),
            Short::Locked(needed) => format!("Build a {} first", needed.stats().name.to_lowercase()),
        }
    }
}

/// This close to the window edge (pixels) the mouse moves the camera.
const EDGE: f32 = 8.0;
/// The same key or unit twice within this time (seconds) counts as a double press.
const DOUBLE_TAP: f64 = 0.4;
/// A held key counts as pressed again after this many seconds …
const REPEAT_DELAY: f64 = 0.4;
/// … and from then on every this many seconds – about Windows' own key repeat (≈ 30 per second at most), which
/// StarCraft 2 uses to queue many units by holding a key.
const REPEAT_EVERY: f64 = 0.04;

/// Holding a key presses it again and again, like typing "aaaa" by holding A. macroquad's `is_key_pressed` only
/// reports the first press (it leaves out Windows' repeats), so the repeat is counted here – only for the buttons
/// that want it (training units): a held Q mustn't open the build menu and then press Q in it too.
#[derive(Default)]
struct KeyRepeat {
    /// The key that was pressed last, and when it counts as pressed the next time.
    held: Option<(KeyCode, f64)>,
}

impl KeyRepeat {
    /// Was `key` pressed this frame – the first time, or again because it's held long enough?
    fn pressed(&mut self, key: KeyCode) -> bool {
        let now = get_time();
        if is_key_pressed(key) {
            self.held = Some((key, now + REPEAT_DELAY));
            return true;
        }
        match self.held {
            Some((held, next)) if held == key && is_key_down(key) && now >= next => {
                // `max`: after a slow frame not several presses to catch up – one per frame at most.
                self.held = Some((key, (next + REPEAT_EVERY).max(now)));
                true
            }
            _ => false,
        }
    }
}

/// The camera keys and where each one moves it (on screen).
const CAMERA: [(Action, Vec2); 4] = [
    (Action::CameraUp, Vec2::new(0.0, -1.0)),
    (Action::CameraLeft, Vec2::new(-1.0, 0.0)),
    (Action::CameraDown, Vec2::new(0.0, 1.0)),
    (Action::CameraRight, Vec2::new(1.0, 0.0)),
];
/// The edge of a spawner can be grabbed this many pixels away from it.
const GRAB_PIXELS: f32 = 10.0;

/// A formation being drawn: the mouse button is down, on release the command goes out.
#[derive(Clone, Copy)]
struct FormDrag {
    /// Where the mouse button went down (world).
    from: Vec2,
    /// Which command: Move, Attack (attack-move) or Patrol.
    button: Button,
    /// Which mouse button draws it: the right one for moving, the left one after T, P or M.
    mouse: MouseButton,
}

/// Which edges of a spawner move while it's dragged. All four: the whole spawner moves.
#[derive(Clone, Copy, PartialEq)]
struct Edges {
    x_min: bool,
    x_max: bool,
    y_min: bool,
    y_max: bool,
}

impl Edges {
    const ALL: Edges = Edges { x_min: true, x_max: true, y_min: true, y_max: true };
}

/// What is being dragged in the editor.
#[derive(Clone, Copy, PartialEq)]
enum Dragged {
    /// A spawner (its `id`).
    Spawner(u32),
    /// The playing field – the world border (`Tool::PlayArea`).
    PlayArea,
}

/// A rectangle being dragged in the editor – a spawner or the playing field: one or two of its edges, or the whole of
/// it. In its axes (`Axes`): straight on screen x across, y down – slanted along the tiles. More things to drag later
/// (regions, DESIGN.md 12.9 step 5) are one more `Dragged`.
#[derive(Clone, Copy)]
struct RectDrag {
    what: Dragged,
    edges: Edges,
    /// Where the mouse went down (world), and where the rectangle lay at that moment.
    start: Vec2,
    min: (i32, i32),
    size: (i32, i32),
    axes: Axes,
}

impl RectDrag {
    /// Where the rectangle lies with the mouse at `point` (world): the grabbed edges snap to the nearest whole step.
    /// It never gets smaller than 1 × 1 – an edge can't be pulled across the opposite one.
    fn rect_at(&self, point: Vec2) -> ((i32, i32), (i32, i32)) {
        let (x0, y0) = self.min;
        let (x1, y1) = (x0 + self.size.0, y0 + self.size.1);
        let point = self.axes.of(point);
        if self.edges == Edges::ALL {
            let d = (point - self.axes.of(self.start)).round();
            return ((x0 + d.x as i32, y0 + d.y as i32), self.size);
        }
        let (px, py) = (point.x.round() as i32, point.y.round() as i32);
        let e = self.edges;
        let nx0 = if e.x_min { px.min(x1 - 1) } else { x0 };
        let nx1 = if e.x_max { px.max(x0 + 1) } else { x1 };
        let ny0 = if e.y_min { py.min(y1 - 1) } else { y0 };
        let ny1 = if e.y_max { py.max(y0 + 1) } else { y1 };
        ((nx0, ny0), (nx1 - nx0, ny1 - ny0))
    }
}

/// A house of a village of doom or a pile of loot clicked on (its `id`) – only to look at it, like a zombie.
#[derive(Clone, Copy, PartialEq)]
pub enum Place {
    Doom(u32),
    Loot(u32),
}

pub struct Ui {
    /// The selected units (their `id`).
    pub selected: Vec<u32>,
    /// The selected buildings (their `id`) – alone or together with units.
    pub selected_buildings: Vec<u32>,
    /// A zombie clicked on (its `id`) – only to look at its values, like an enemy in StarCraft. Never together with
    /// your units or buildings, and gone once it's dead or out of sight.
    pub selected_zombie: Option<u32>,
    /// The same for a house of doom or a pile of loot.
    pub selected_place: Option<Place>,
    /// The Tab subgroup: which type of the selection is "in front" right now – its buttons are on the card.
    pub active: Option<Sub>,
    /// The build menu of the command center that is open (D, E, A) – `None`: its main buttons.
    pub submenu: Option<Submenu>,
    pub paused: bool,
    pub speed: u32,
    /// The player's settings (menu → settings, DESIGN.md 11.4).
    pub settings: Settings,
    /// Game view or editor mode (top right, F10) – with the developer tools in editor mode.
    pub editor: Editor,
    /// The real mouse from Windows (focus, position, confining).
    cursor: Cursor,
    /// Where the mouse really is – `None` if the window has no focus or the mouse is outside.
    real_mouse: Option<Vec2>,
    /// What lies under the mouse on the map (for highlighting like in StarCraft).
    pub hover_unit: Option<u32>,
    pub hover_zombie: Option<u32>,
    pub hover_building: Option<u32>,
    /// Middle mouse button held: where the mouse was in the last frame (dragging the map).
    pan_drag: Option<Vec2>,
    /// After T, P, M or R: the next left click gives this command instead of selecting.
    pub targeting: Option<Button>,
    /// The waiting command was given with Shift at least once (waypoints) – letting go of Shift then takes it off the
    /// mouse and puts it here, and pressing Shift again puts it back. Gone with the next click, command or selection.
    chained: bool,
    parked: Option<Button>,
    /// The overlay in the top left (F3), if it's shown: clicks there are for it, not for the map (`game.rs`).
    pub overlay: Option<Rect>,
    /// After B, V, G or T: a building hangs on the mouse and waits for its spot. It stays there after building –
    /// until right click or Esc.
    pub placing: Option<BuildingKind>,
    /// The building on the mouse came from the editor's panel: it goes anywhere and costs nothing – even in a test.
    /// (From the command center it follows the rules of the game while a test runs.)
    free_placing: bool,
    /// A short message in the middle at the top ("Not enough gold", a wave is coming) and how many seconds it stays.
    alert: Option<(String, f32)>,
    /// The building on the mouse is turned by 90° (R – only the gate is not square).
    turned: bool,
    /// The formation units line up in when they're sent somewhere (buttons on the command card).
    pub formation: Shape,
    /// A formation being drawn with the mouse.
    form_drag: Option<FormDrag>,
    /// A spawner or the playing field being dragged in the editor.
    rect_drag: Option<RectDrag>,
    /// While building with the left mouse button down: the tile where it started. On release the whole
    /// line from there to the mouse is built.
    place_start: Option<(i32, i32)>,
    /// While painting terrain with the left mouse button down: the tile painted last. If the mouse moves
    /// quickly, the line in between is painted too, so the stroke has no holes.
    paint_last: Option<(i32, i32)>,
    /// Control groups: index 0 belongs to key 1, …, index 9 to key 0.
    pub groups: [Group; 10],
    /// Which group key was pressed last, and when – for the double press.
    last_group: Option<(usize, f64)>,
    /// Which unit was clicked last, and when – for the double click. The same for buildings.
    last_click: Option<(u32, f64)>,
    last_building_click: Option<(u32, f64)>,
    bookmarks: [Option<Vec2>; 4],
    /// Where the left mouse button was pressed on the map (for the selection box).
    drag_start: Option<Vec2>,
    /// Left mouse button pressed and held on the minimap: the camera follows the mouse.
    minimap_drag: bool,
    /// Marker after a command: world position, seconds left, kind (for the colour).
    marker: Option<(Vec2, f32, Mark)>,
    /// A held key of the command card presses its button again and again (training units).
    repeat: KeyRepeat,
}

impl Ui {
    /// `editor`: in the map editor (with its panel and tools) or in the game.
    pub fn new(settings: Settings, editor: bool) -> Ui {
        Ui {
            selected: Vec::new(),
            selected_buildings: Vec::new(),
            selected_zombie: None,
            selected_place: None,
            active: None,
            submenu: None,
            paused: false,
            speed: 1,
            settings,
            editor: Editor::new(editor),
            cursor: Cursor::new(),
            real_mouse: None,
            hover_unit: None,
            hover_zombie: None,
            hover_building: None,
            pan_drag: None,
            targeting: None,
            chained: false,
            parked: None,
            overlay: None,
            placing: None,
            free_placing: false,
            alert: None,
            turned: false,
            formation: Shape::Line,
            form_drag: None,
            rect_drag: None,
            place_start: None,
            paint_last: None,
            groups: Default::default(),
            last_group: None,
            last_click: None,
            last_building_click: None,
            bookmarks: [None; 4],
            drag_start: None,
            minimap_drag: false,
            marker: None,
            repeat: KeyRepeat::default(),
        }
    }

    /// The selection as it's shown in the panel at the bottom: first the units, then the buildings, each sorted
    /// by type.
    pub fn selection_view(&self, world: &World) -> Vec<Pick> {
        let mut units: Vec<usize> = self.selected.iter().filter_map(|&id| world.find_unit(id)).collect();
        units.sort_by_key(|&i| (world.units[i].kind, world.units[i].id));
        let mut buildings: Vec<usize> = self.selected_buildings.iter().filter_map(|&id| world.find_building(id)).collect();
        buildings.sort_by_key(|&b| (world.buildings[b].kind, world.buildings[b].id));
        units.into_iter().map(Pick::Unit).chain(buildings.into_iter().map(Pick::Building)).collect()
    }

    /// Is anything selected at all?
    fn has_selection(&self) -> bool {
        !self.selected.is_empty() || !self.selected_buildings.is_empty()
    }

    /// The selected buildings of the active type (indices) – the ones the command card speaks for.
    fn active_buildings(&self, world: &World) -> Vec<usize> {
        let Some(Sub::Building(kind)) = self.active else {
            return Vec::new();
        };
        self.selected_buildings.iter().filter_map(|&id| world.find_building(id)).filter(|&b| world.buildings[b].kind == kind).collect()
    }

    /// The only selected thing, if it is one building – then the panel shows it in full (production queue …).
    pub fn single_building(&self, world: &World) -> Option<usize> {
        match self.selection_view(world).as_slice() {
            [Pick::Building(b)] => Some(*b),
            _ => None,
        }
    }

    /// What the camera may show: in the game view only the playing field inside the world border,
    /// in editor mode the whole map.
    pub fn view_area(&self, world: &World) -> Region {
        if self.editor.editing() { world.map.full_area() } else { world.map.play_area() }
    }

    /// Whose eyes the picture shows: the player's in the game view, in the editor the chosen view.
    pub fn sight(&self) -> Sight {
        self.editor.sight()
    }

    /// Does the building on the mouse go without the rules of the game (anywhere, for free)? While editing always; in a
    /// test only if it came from the editor's panel – from the command center the test is the real game.
    fn rule_free(&self) -> bool {
        self.editor.editing() && (self.free_placing || !self.editor.testing())
    }

    /// Can the building on the mouse go here? By the rules (`can_place`), or almost anywhere (`rule_free`).
    fn fits(&self, world: &World, kind: BuildingKind, tile: (i32, i32)) -> bool {
        if self.rule_free() {
            world.can_place_anywhere(kind, tile, self.turned)
        } else {
            world.can_place(kind, tile, self.turned)
        }
    }

    /// The pieces of a building line that would really be built: those that fit and – by the rules – that the gold,
    /// the wood, the stone and the free workers pay for, one after the other. `credit` (Shift): the gold may go into
    /// the banks' credit. `world` is only read: the simulation pays when the orders arrive (`orders.rs`).
    fn affordable(&self, world: &World, kind: BuildingKind, tiles: &[(i32, i32)], credit: bool) -> LinePlan {
        let free = self.rule_free();
        let (mut gold, mut workers) = (if credit { world.spendable_on_credit() } else { world.gold }, world.free_workers());
        let mut barrels = world.barrels;
        // Without Shift, but the credit would pay: say so.
        let gold_short = if !credit && world.spendable_on_credit() > world.gold { Short::GoldCredit } else { Short::Gold };
        let (mut wood, mut stone) = (world.stock.get(Resource::Wood), world.stock.get(Resource::Stone));
        let (mut coal, mut iron) = (world.stock.get(Resource::Coal), world.stock.get(Resource::Iron));
        let stats = kind.stats();
        let mut plan = LinePlan { built: Vec::new(), cost: 0, workers: 0, short: None };
        // Power: what the grids have to spare, and what the pieces planned so far take of it, per grid.
        let power = world.power();
        let mut taken = vec![0u32; power.made.len()];
        let (w, h) = kind.footprint(self.turned);
        if let Some(needed) = stats.requires.filter(|_| !free && !world.unlocked(kind)) {
            plan.short = Some(Short::Locked(needed));
            return plan;
        }
        for &tile in tiles {
            if !self.fits(world, kind, tile) {
                continue;
            }
            let (price, needs) = if free {
                (0, 0)
            } else {
                (world.build_cost(kind, tile, self.turned), world.build_workers(kind, tile, self.turned))
            };
            // `get_or_insert`: only the first thing that runs out is remembered.
            if price > gold {
                plan.short.get_or_insert(gold_short);
                continue;
            }
            if needs > workers {
                plan.short.get_or_insert(Short::Workers);
                continue;
            }
            let energy = if free { 0 } else { world.build_energy(kind, tile, self.turned) };
            let center = vec2(tile.0 as f32 + w as f32 / 2.0, tile.1 as f32 + h as f32 / 2.0);
            let grid = world.spare_power_at(&power, center);
            if energy > 0 && grid.is_none_or(|(g, spare)| taken[g] + energy > spare) {
                plan.short.get_or_insert(Short::Power);
                continue;
            }
            if let Some((g, _)) = grid {
                taken[g] += energy;
            }
            if !free && stats.cost_wood > wood {
                plan.short.get_or_insert(Short::Wood);
                continue;
            }
            if !free && stats.cost_stone > stone {
                plan.short.get_or_insert(Short::Stone);
                continue;
            }
            if !free && stats.cost_coal > coal {
                plan.short.get_or_insert(Short::Coal);
                continue;
            }
            if !free && stats.cost_iron > iron {
                plan.short.get_or_insert(Short::Iron);
                continue;
            }
            if !free && kind == BuildingKind::Barrel && barrels == 0 {
                plan.short.get_or_insert(Short::Barrels);
                continue;
            }
            if !free {
                wood -= stats.cost_wood;
                stone -= stats.cost_stone;
                coal -= stats.cost_coal;
                iron -= stats.cost_iron;
                if kind == BuildingKind::Barrel {
                    barrels -= 1;
                }
            }
            gold -= price;
            workers -= needs;
            plan.cost += price;
            plan.workers += needs;
            plan.built.push(tile);
        }
        plan
    }

    /// A short message in the middle at the top, for a few seconds.
    pub fn alert(&mut self, text: &str) {
        self.alert = Some((text.to_owned(), 2.5));
    }

    /// Nothing selected anymore, the control groups empty – the editor's test runs on a copy of the map, whose units
    /// and buildings have other numbers.
    pub fn forget_selection(&mut self) {
        self.selected.clear();
        self.selected_buildings.clear();
        self.selected_zombie = None;
        self.selected_place = None;
        self.groups = Default::default();
        self.targeting = None;
        self.parked = None;
        self.placing = None;
        self.marker = None;
    }

    /// Can this zombie be seen (drawn, clicked)? In the player's view only where your units look.
    pub fn shows_zombie(&self, world: &World, p: Vec2) -> bool {
        self.view_area(world).contains(p) && (self.sight() != Sight::Player || world.vision.sees(p))
    }

    /// While a building is on the mouse (by the rules of the game): if it needs power, the reach of every grid – the
    /// circles of the command center and the tesla towers; if it gathers, its circle, the tiles it would work and
    /// what it would bring in per payout (at full power).
    fn draw_economy_hints(&self, world: &World, camera: &Camera, kind: BuildingKind, tile: (i32, i32), mouse: Vec2) {
        if self.rule_free() {
            return;
        }
        if kind.on_grid() {
            for (b, building) in world.buildings.iter().enumerate() {
                if world.carries_power(b) {
                    let radius = building.stats().power_radius;
                    draw_ground_circle(camera, building.center(), radius, 2.0, Color { a: 0.55, ..TESLA_GLOW });
                }
            }
        }
        // A market: its area, the houses that would be its own, and what they'd bring (`sim/market.rs`).
        if kind.stats().trade_range > 0 {
            let (min, max) = kind.trade_area(tile);
            draw_ground_frame(camera, tile_corner(min), tile_corner(max), 1.5, Color::new(1.0, 0.8, 0.4, 0.7));
            let houses = world.market_houses((min, max), world.buildings.len());
            for &h in &houses {
                let house = &world.buildings[h];
                draw_diamond(camera, house.corner(), house.far_corner() - house.corner(), Color::new(1.0, 0.85, 0.3, 0.35));
            }
            let stats = kind.stats();
            let bonus: u32 = houses.iter().map(|&h| world.buildings[h].stats().gold * stats.house_bonus_percent / 100).sum();
            let every = data::get().colony.economy_every_hours;
            let text = format!(
                "{} houses: +{bonus} gold, sells up to {} goods per payout ({every} h)",
                houses.len(),
                world.sell_limit(&houses)
            );
            draw_text(&text, mouse.x + 14.0, mouse.y + 14.0, 18.0, Color::new(1.0, 1.0, 0.7, 1.0));
            return;
        }
        if kind.gathers().is_none() {
            return;
        }
        let (min, max) = kind.gather_area(tile);
        draw_ground_frame(camera, tile_corner(min), tile_corner(max), 1.5, Color::new(1.0, 1.0, 1.0, 0.5));
        for (x, y) in world.worked_tiles(kind, tile, world.buildings.len()) {
            draw_diamond(camera, vec2(x as f32 + 0.3, y as f32 + 0.3), Vec2::splat(0.4), Color::new(1.0, 1.0, 0.6, 0.6));
        }
        let rate = world.gather_rate(kind, tile, world.buildings.len(), 1000);
        let mut parts: Vec<String> = Resource::ALL
            .into_iter()
            .filter(|r| rate[r.index()] > 0)
            .map(|r| format!("+{:.1} {}", rate[r.index()] as f32 / 1000.0, r.name().to_lowercase()))
            .collect();
        let gold = world.gold_rate(kind, tile, world.buildings.len(), 1000);
        if gold > 0 {
            parts.push(format!("+{} gold", gold / 1000));
        }
        let every = data::get().colony.economy_every_hours;
        let text = if parts.is_empty() { "Nothing to gather here".to_owned() } else { parts.join(", ") + &format!(" per payout ({every} h)") };
        draw_text(&text, mouse.x + 14.0, mouse.y + 14.0, 18.0, Color::new(1.0, 1.0, 0.7, 1.0));
    }

    /// Is a command waiting for its target, a building for its spot, or is an editor tool in hand?
    /// Then right click and Esc cancel that.
    fn is_waiting(&self) -> bool {
        self.targeting.is_some() || self.placing.is_some() || self.editor.tool.is_some()
    }

    /// Does Esc have something to do here – cancel, put away, deselect the spawner, stop typing, clear the selection?
    /// Then it doesn't open the menu.
    pub fn is_busy(&self) -> bool {
        self.has_pending() || self.has_selection() || self.selected_zombie.is_some() || self.selected_place.is_some()
    }

    /// Everything Esc cancels before it gets to the selection: a waiting command or tool, a formation being drawn,
    /// a build menu, the selected spawner, a name being typed.
    fn has_pending(&self) -> bool {
        self.is_waiting()
            || self.form_drag.is_some()
            || self.submenu.is_some()
            || self.editor.spawner.is_some()
            || self.editor.is_typing()
    }

    /// Nothing selected anymore (Esc, a click or a box on nothing) – then WASD moves the camera again.
    fn deselect(&mut self) {
        self.set_selection(Vec::new(), Vec::new());
        self.targeting = None;
    }

    /// The spawners as the editor shows them – a dragged one with the size it has right now, a renamed one with
    /// the name being typed. In the game view there are none: players don't see spawners.
    pub fn spawner_shapes(&self, world: &World, camera: &Camera) -> Vec<SpawnerShape> {
        if !self.editor.editing() {
            return Vec::new();
        }
        let point = camera.screen_to_world(Vec2::from(mouse_position()));
        world
            .spawners
            .iter()
            .map(|s| {
                let selected = self.editor.spawner == Some(s.id);
                let (min, size) = match self.rect_drag {
                    Some(drag) if drag.what == Dragged::Spawner(s.id) => drag.rect_at(point),
                    _ => (s.min, s.size),
                };
                let name = match &self.editor.name_edit {
                    Some(text) if selected => text.clone(),
                    _ => s.name.clone(),
                };
                SpawnerShape { corners: spawner_corners(s.axes, min, size), name, selected }
            })
            .collect()
    }

    /// Which buttons the command card shows right now – and which keys are taken by them.
    /// Was a key of `action` pressed in this frame – one the command card doesn't have right now? The card comes
    /// first: X is the Line formation while units are selected, otherwise "repair all" (`keys.rs`).
    fn global_pressed(&self, action: Action, card: &[Button]) -> bool {
        let keys = &self.settings.keys;
        let on_card = |key| card.iter().any(|&button| keys.has(Action::of(button), key));
        keys.keys(action).into_iter().flatten().any(|key| is_key_pressed(key) && !on_card(key))
    }

    pub fn card(&self, world: &World) -> Vec<Button> {
        if !self.has_selection() || self.selection_view(world).is_empty() {
            return Vec::new();
        }
        match self.active {
            Some(Sub::Unit(_)) => {
                let mut card = UNIT_BUTTONS.to_vec();
                // Somebody carries a barrel: D puts it down.
                if self.selected.iter().any(|&id| world.find_unit(id).is_some_and(|u| world.units[u].barrel)) {
                    card.push(Button::Drop);
                }
                card
            }
            Some(Sub::Building(BuildingKind::CommandCenter)) => match self.submenu {
                // In the editor the command center can be removed like anything else.
                None if self.editor.editing() && !self.editor.testing() => [COMMAND_CENTER_BUTTONS.as_slice(), &[Button::Demolish]].concat(),
                None => COMMAND_CENTER_BUTTONS.to_vec(),
                Some(menu) => menu_buttons(menu),
            },
            Some(Sub::Building(kind)) => building_buttons(kind),
            None => Vec::new(),
        }
    }

    /// A new selection: these units and buildings, nothing on the mouse anymore. A build menu of the command center
    /// closes – selecting it again (click, group key) shows its main buttons.
    fn set_selection(&mut self, units: Vec<u32>, buildings: Vec<u32>) {
        self.selected = units;
        self.selected_buildings = buildings;
        self.selected_zombie = None;
        self.selected_place = None;
        self.placing = None;
        self.parked = None;
        self.submenu = None;
    }

    fn select_units(&mut self, ids: Vec<u32>) {
        self.set_selection(ids, Vec::new());
    }

    /// Every frame, even with the menu open: query the real mouse, confine or release it.
    pub fn update_cursor(&mut self) {
        self.real_mouse = self.cursor.update(self.settings.confine_mouse);
    }

    pub fn handle_input(&mut self, world: &World, camera: &mut Camera, orders: &mut Vec<Order>, dt: f32) {
        self.forget_dead(world);
        self.editor.sync(self.placing, tile_of(camera.screen_to_world(Vec2::from(mouse_position()))));
        // What the editor's panel did this frame (it ran before, `Game::frame`): a building button hangs the building
        // on the mouse; a tool taken in hand takes off whatever is on it – a building, a command waiting for its target.
        if let Some(place) = self.editor.place.take() {
            self.targeting = None;
            self.placing = place;
            if place.is_some() {
                self.free_placing = true;
                self.turned = false;
                self.selected_buildings.clear();
            }
        }
        let layout = Layout::new();
        let mouse = Vec2::from(mouse_position());
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);

        // Waypoints with Shift (like StarCraft): Shift let go, and the command comes off the mouse – no right click
        // needed. Shift pressed again before anything else happened, and it's back on the mouse.
        if self.chained && !shift {
            self.parked = self.targeting.take();
            self.chained = false;
        } else if shift && self.targeting.is_none() {
            if let Some(button) = self.parked.take() {
                self.targeting = Some(button);
                self.chained = true;
            }
        }
        if is_mouse_button_pressed(MouseButton::Left) || is_mouse_button_pressed(MouseButton::Right) {
            self.parked = None;
        }

        // While something is typed into the editor's panel (a name, a number), the keyboard belongs to that field: no
        // hotkeys, no WASD.
        let typing = self.editor.is_typing();
        // Esc with nothing else to cancel clears the selection – the next Esc opens the menu. Decided now, before
        // anything below cancels something: one press does one thing.
        let esc_deselects = !typing && is_key_pressed(KeyCode::Escape) && !self.has_pending();
        if !typing && is_key_pressed(KeyCode::Escape) {
            // Esc: first put the tool away, then deselect the spawner.
            if self.editor.tool.is_some() {
                self.editor.tool = None;
            } else {
                self.editor.select_spawner(None);
            }
            self.form_drag = None;
        }
        if !typing && is_key_pressed(KeyCode::Delete) {
            if let Some(spawner) = self.editor.spawner.take() {
                orders.push(Order::RemoveSpawner { spawner });
            }
        }
        let card = self.card(world);
        self.mouse_buttons(world, camera, orders, &layout, mouse, shift, ctrl);
        // X: repair all (like TAB) – unless the card has a button on X right now (units: the Line formation).
        if !typing && self.global_pressed(Action::RepairAll, &card) {
            self.repair(world, orders, (0..world.buildings.len()).collect());
        }
        if !typing {
            self.command_keys(world, orders, &card, shift, ctrl);
            self.selection_keys(world, camera, &card, shift, ctrl);
            self.other_keys(&card);
        }
        if esc_deselects {
            self.deselect();
        }
        self.move_camera(world, camera, mouse, dt, !typing);
        self.fix_active(world);
        self.update_hover(world, camera, &layout, mouse);
        // Whatever moved the camera (minimap, bookmark, group, mode switch): it stops at the edge of the view.
        camera.keep_inside(self.view_area(world));

        if let Some((_, time_left, _)) = &mut self.marker {
            *time_left -= dt;
            if *time_left <= 0.0 {
                self.marker = None;
            }
        }
        if let Some((_, time_left)) = &mut self.alert {
            *time_left -= dt;
            if *time_left <= 0.0 {
                self.alert = None;
            }
        }
    }

    /// Strike dead units from the selection and the control groups. Units in a tower can't be selected (like in
    /// a StarCraft bunker) – they stay in their control groups, though, and are back after unloading.
    fn forget_dead(&mut self, world: &World) {
        self.selected.retain(|&id| world.find_unit(id).is_some_and(|i| !world.units[i].is_inside()));
        for group in &mut self.groups {
            group.units.retain(|&id| world.find_unit(id).is_some());
            group.buildings.retain(|&id| world.find_building(id).is_some());
        }
        let before = self.selected_buildings.len();
        self.selected_buildings.retain(|&id| world.find_building(id).is_some());
        if self.selected_buildings.len() < before {
            self.targeting = None;
            self.placing = None;
        }
        // The zombie looked at: gone when it's dead, out of sight – or when units or buildings got selected some
        // other way (control group, F2 …).
        let zombie_gone = |id: u32| world.find_zombie(id).is_none_or(|z| !self.shows_zombie(world, world.zombies[z].pos));
        if self.selected_zombie.is_some_and(zombie_gone) || self.has_selection() {
            self.selected_zombie = None;
        }
        // The house shot down, the loot picked up.
        let place_gone = |place: Place| match place {
            Place::Doom(id) => world.find_doom(id).is_none(),
            Place::Loot(id) => world.find_loot(id).is_none(),
        };
        if self.selected_place.is_some_and(place_gone) || self.has_selection() {
            self.selected_place = None;
        }
        // A removed spawner – but not one that has just been placed and isn't in the world yet.
        if let Some(id) = self.editor.spawner {
            if id < world.next_spawner_id() && world.find_spawner(id).is_none() {
                self.editor.spawner = None;
                self.editor.name_edit = None;
                self.rect_drag = None;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn mouse_buttons(
        &mut self,
        world: &World,
        camera: &mut Camera,
        orders: &mut Vec<Order>,
        layout: &Layout,
        mouse: Vec2,
        shift: bool,
        ctrl: bool,
    ) {
        let view = self.selection_view(world);
        // The production queue is only shown (and clickable) when one building alone is selected.
        let building = self.single_building(world);
        let queue = building.map_or(0, |b| world.buildings[b].production.len());
        // The editor panel and the overlay lie on top of the map: there it counts like the bottom panel (not the map).
        // A click on the editor's panel egui has already taken (`Editor::show`).
        let on_editor = self.editor.has_mouse();
        let on_overlay = self.overlay.is_some_and(|r| r.contains(mouse));
        let hit = if on_editor || on_overlay {
            Some(HudHit::Panel)
        } else {
            layout.hit(mouse, self.view_area(world), view.len(), queue, &self.card(world))
        };
        // The world point under the mouse – on the minimap the point it shows.
        let point = match hit {
            Some(HudHit::Minimap(p)) => p,
            _ => camera.screen_to_world(mouse),
        };

        if is_mouse_button_pressed(MouseButton::Left) {
            if let (Some(tool), None) = (self.editor.tool, hit) {
                self.use_tool(tool, world, camera, orders, point);
            } else if let (Some(kind), None) = (self.placing, hit) {
                // Building on the mouse: the line starts here. It's built on release.
                self.place_start = Some(placement_tile(kind, self.turned, point));
            } else if let (Some(Button::Scan), None | Some(HudHit::Minimap(_))) = (self.targeting, hit) {
                // The scan waits for its spot – on the map or on the minimap.
                orders.push(Order::Scan { at: point });
                self.targeting = None;
            } else if let (Some(Button::Rally), None | Some(HudHit::Minimap(_))) = (self.targeting, hit) {
                let rallying = self.rally_buildings(world, true);
                self.set_rally(&rallying, world, camera, orders, mouse, point, hit.is_none());
                self.targeting = None;
            } else if let (Some(button), None | Some(HudHit::Minimap(_))) = (self.targeting, hit) {
                // If a command (T, P, M) waits for its target, the click gives it – on the map or on the minimap.
                // On a zombie or a unit right away; on the ground it waits for the release: dragged = formation.
                let on_field = hit.is_none();
                match self.aimed_at(button, world, camera, mouse).filter(|_| on_field) {
                    Some(command) => {
                        self.give(orders, command, shift, point, None);
                        self.keep_targeting(shift);
                    }
                    None if on_field => self.form_drag = Some(FormDrag { from: point, button, mouse: MouseButton::Left }),
                    None => {
                        // On the minimap: a click, so no formation – everyone to the point.
                        self.give(orders, point_command(button, point), shift, point, None);
                        self.keep_targeting(shift);
                    }
                }
            } else {
                match hit {
                    // In the editor a click can grab a spawner – otherwise it starts the selection box.
                    None if self.editor.editing() && self.spawner_press(world, camera, point) => {}
                    None => self.drag_start = Some(mouse),
                    Some(HudHit::Minimap(p)) => {
                        camera.center = p;
                        self.minimap_drag = true;
                    }
                    Some(HudHit::Group(g)) => self.group_action(g, world, camera, shift, ctrl),
                    Some(HudHit::Icon(k)) => self.click_icon(world, view[k], shift, ctrl),
                    Some(HudHit::Queue(index)) => {
                        if let Some(b) = building {
                            orders.push(Order::CancelTrain { building: world.buildings[b].id, index });
                        }
                    }
                    Some(HudHit::Button(button)) => self.press(button, world, orders, shift, ctrl),
                    Some(HudHit::Pause) => self.paused = !self.paused,
                    Some(HudHit::RepairAll) => self.repair(world, orders, (0..world.buildings.len()).collect()),
                    Some(HudHit::Panel) => {}
                }
            }
        }
        if is_mouse_button_down(MouseButton::Left) {
            if self.minimap_drag {
                if let Some(HudHit::Minimap(p)) = hit {
                    camera.center = p;
                }
            }
            // Painting terrain or ore, erasing: every frame the mouse is on another tile, paint the way there.
            if let (Some(tool), Some(last), None) = (self.editor.tool, self.paint_last, hit) {
                let now = tile_of(point);
                if tool.paints() && now != last {
                    self.paint(tool, last, now, orders);
                }
            }
        }
        if is_mouse_button_released(MouseButton::Left) {
            self.minimap_drag = false;
            self.paint_last = None;
            if let (Some(start), Some(kind)) = (self.place_start.take(), self.placing) {
                // Build the whole line – every piece that has room and that the gold pays for. The building stays on
                // the mouse for the next line, until right click or Esc. In the editor without the rules of the game.
                let end = placement_tile(kind, self.turned, camera.screen_to_world(mouse));
                let (turned, editor) = (self.turned, self.rule_free());
                let plan = self.affordable(world, kind, &placement_line(kind, start, end), shift);
                for tile in plan.built {
                    let order = Order::PlaceBuilding { kind, tile, turned, editor };
                    // With Shift the gold may go into the banks' credit.
                    orders.push(if shift && !editor { Order::Credit(Box::new(order)) } else { order });
                }
                if let Some(short) = plan.short {
                    self.alert(&short.alert());
                }
            }
            if let Some(start) = self.drag_start.take() {
                self.select(world, camera, layout, start, mouse, shift, ctrl);
            }
            // A spawner or the border let go: put it there for real (one order, not one per frame).
            if let Some(drag) = self.rect_drag.take() {
                let (min, size) = drag.rect_at(point);
                if (min, size) != (drag.min, drag.size) {
                    orders.push(match drag.what {
                        Dragged::Spawner(spawner) => Order::ResizeSpawner { spawner, min, size },
                        Dragged::PlayArea => Order::SetPlayArea { min, size },
                    });
                }
            }
        }

        // The mouse button that draws a formation is let go: off goes the command.
        if let Some(drag) = self.form_drag {
            if is_mouse_button_released(drag.mouse) {
                self.form_drag = None;
                self.finish_formation(drag, camera.screen_to_world(mouse), orders, shift);
            }
        }

        if is_mouse_button_pressed(MouseButton::Right) {
            if self.is_waiting() || self.form_drag.is_some() {
                // Right click cancels the waiting command or puts the editor tool away.
                self.targeting = None;
                self.placing = None;
                self.editor.tool = None;
                self.form_drag = None;
                return;
            }
            // Editor: right click on a spawner, with nothing selected that would take it as an order – its properties
            // (the one on top, placed last).
            if self.editor.editing() && hit.is_none() && !self.has_selection() {
                if let Some(s) = world.spawners.iter().rposition(|s| s.contains(point)) {
                    self.editor.open_properties(world.spawners[s].id, mouse);
                    return;
                }
            }
            // Selected buildings set their rally point (like in StarCraft) – selected units still get their command.
            let rallying = self.rally_buildings(world, false);
            if !rallying.is_empty() {
                if let None | Some(HudHit::Minimap(_)) = hit {
                    self.set_rally(&rallying, world, camera, orders, mouse, point, hit.is_none());
                }
            }
            if self.selected.is_empty() {
                return;
            }
            match hit {
                // Right click on the map: the "smart" command, depending on what is under the mouse. On the ground
                // it waits for the release – dragged, the units line up along the line (formation).
                None => {
                    // A tower to go in, or a munitions factory to fetch a barrel from – walking there is the same.
                    let tower = building_under_mouse(world, camera, mouse)
                        .filter(|&b| world.buildings[b].kind.is_tower() || world.buildings[b].kind.makes_barrels());
                    if let Some(zombie) = self.zombie_at(world, camera, mouse) {
                        self.give(orders, Command::Attack(zombie), shift, point, None);
                    } else if let Some(i) = unit_under_mouse(world, camera, mouse) {
                        self.give(orders, Command::Follow(world.units[i].id), shift, point, None);
                    } else if let Some(b) = tower {
                        // On a tower: go in (like loading a StarCraft bunker). On a munitions factory: each takes a
                        // barrel.
                        self.give(orders, Command::Enter(world.buildings[b].id), shift, point, None);
                    } else if let Some(place) = self.place_at(world, camera, mouse) {
                        // On a house of doom: attack-move there – they shoot it once no zombie is in range. On loot:
                        // walk right onto it, that picks it up.
                        let (command, at) = match place {
                            Place::Doom(id) => {
                                let center = world.find_doom(id).map_or(point, |d| world.doom[d].center());
                                (Command::AttackMove(center), center)
                            }
                            Place::Loot(id) => {
                                let pos = world.find_loot(id).map_or(point, |l| world.loot[l].pos);
                                (Command::Move(pos), pos)
                            }
                        };
                        self.give(orders, command, shift, at, None);
                    } else if !self.selected.is_empty() {
                        self.form_drag = Some(FormDrag { from: point, button: Button::Move, mouse: MouseButton::Right });
                    }
                }
                // Right click on the minimap: everyone walks to the point (a click, not a drag: no formation).
                Some(HudHit::Minimap(p)) => self.give(orders, Command::Move(p), shift, p, None),
                _ => {}
            }
        }
    }

    /// Released after drawing a formation: the command goes out. Barely dragged counts as a click – then there is no
    /// formation, everyone goes to (or attacks towards) the clicked point.
    fn finish_formation(&mut self, drag: FormDrag, to: Vec2, orders: &mut Vec<Order>, shift: bool) {
        let command = point_command(drag.button, drag.from);
        if to.distance(drag.from) < MIN_DRAG {
            self.give(orders, command, shift, drag.from, None);
        } else {
            let formation = Formation { shape: self.formation, from: drag.from, to };
            // The marker goes into the middle of the formation (for the circle: the centre where the drag started).
            let middle = if self.formation == Shape::Circle { drag.from } else { (drag.from + to) / 2.0 };
            self.give(orders, command, shift, middle, Some(formation));
        }
        if drag.mouse == MouseButton::Left {
            self.keep_targeting(shift);
        }
    }

    /// After a command went to its target: without Shift the waiting ends, with Shift it stays – that's how you set
    /// several waypoints in a row (until Shift is let go).
    fn keep_targeting(&mut self, shift: bool) {
        if shift {
            self.chained = true;
        } else {
            self.targeting = None;
        }
    }

    /// Left click in the editor (no tool): grab the edge of the selected spawner, or the inside of a spawner to
    /// move it. Returns whether a spawner was hit – if not, the selection box starts as usual.
    fn spawner_press(&mut self, world: &World, camera: &Camera, point: Vec2) -> bool {
        let selected = self.editor.spawner.and_then(|id| world.find_spawner(id));
        let grab_spawner = |s: usize| grab(world.spawners[s].min, world.spawners[s].size, world.spawners[s].axes, point, camera);
        let grabbed = match selected.and_then(|s| grab_spawner(s).map(|e| (s, e))) {
            Some(hit) => Some(hit),
            // Another spawner (the one on top, i.e. placed last), unless there's a unit under the mouse.
            None if unit_under_mouse(world, camera, camera.world_to_screen(point)).is_none() => {
                world.spawners.iter().rposition(|s| s.contains(point)).map(|s| (s, Edges::ALL))
            }
            None => None,
        };
        let Some((s, edges)) = grabbed else {
            self.editor.select_spawner(None);
            return false;
        };
        let spawner = &world.spawners[s];
        self.editor.select_spawner(Some(spawner.id));
        let what = Dragged::Spawner(spawner.id);
        self.rect_drag = Some(RectDrag { what, edges, start: point, min: spawner.min, size: spawner.size, axes: spawner.axes });
        true
    }

    /// Left click on the map with an editor tool in hand.
    fn use_tool(&mut self, tool: Tool, world: &World, camera: &Camera, orders: &mut Vec<Order>, point: Vec2) {
        match tool {
            Tool::Paint(_) | Tool::Ore(_) | Tool::Remove | Tool::ZombieBrush => {
                let tile = tile_of(point);
                self.paint(tool, tile, tile, orders);
            }
            Tool::ZombieDrop => orders.push(self.editor.drop_order(point)),
            // One giant on the tile: the brush with one giant on one tile.
            Tool::Giant => {
                let wander = self.editor.wander;
                orders.push(Order::PaintZombies { tiles: vec![tile_of(point)], per_tile: vec![(Breed::Giant, 1)], wander });
            }
            // An edge of the border (or two at a corner) – or, inside, all of it.
            Tool::PlayArea => {
                let (min, size) = world.map.play_steps();
                if let Some(edges) = grab(min, size, Axes::Screen, point, camera) {
                    self.rect_drag = Some(RectDrag { what: Dragged::PlayArea, edges, start: point, min, size, axes: Axes::Screen });
                }
            }
            Tool::Unit(kind) => orders.push(Order::SpawnUnit { kind, point }),
            // A 3 × 3 house with its middle tile under the mouse.
            Tool::Doom => {
                let (x, y) = tile_of(point);
                orders.push(Order::PlaceDoom { tile: (x - 1, y - 1), size: (3, 3) });
            }
            Tool::Loot => orders.push(Order::PlaceLoot { point }),
            Tool::Spawner => {
                // Select the new spawner right away, so its edge can be dragged. Which number it will get:
                // the next free one – plus the spawners placed while the game is paused (orders still waiting).
                let waiting = orders.iter().filter(|o| matches!(o, Order::PlaceSpawner { .. })).count() as u32;
                let center = to_frame(point).round();
                orders.push(Order::PlaceSpawner { center: (center.x as i32, center.y as i32) });
                self.editor.select_spawner(Some(world.next_spawner_id() + waiting));
                self.editor.tool = None;
            }
            // Never in the editor's hand – it goes onto the mouse as `placing` right away.
            Tool::Building(_) => {}
        }
    }

    /// Paints with the brush along the line from `from` to `to` – one order for the whole stroke.
    /// `tool`: terrain, ore or the eraser – anything else doesn't paint.
    fn paint(&mut self, tool: Tool, from: (i32, i32), to: (i32, i32), orders: &mut Vec<Order>) {
        // Oil isn't painted but stamped: one node of 2 × 2 tiles, whatever the brush. Dragged, the next one only comes
        // where it doesn't touch the last one – nodes never touch (`mapgen/ores.rs`).
        if tool == Tool::Ore(Some(Ore::Oil)) {
            if self.paint_last.is_some() && (to.0 - from.0).abs().max((to.1 - from.1).abs()) < 3 {
                return;
            }
            orders.push(Order::PaintOre { ore: Some(Ore::Oil), tiles: oil_node(to) });
            self.paint_last = Some(to);
            return;
        }
        let mut tiles: Vec<(i32, i32)> = grid_line(from, to).into_iter().flat_map(|t| self.editor.brush_tiles(t)).collect();
        tiles.sort();
        tiles.dedup(); // neighbouring brush squares overlap
        match tool {
            Tool::Paint(terrain) => orders.push(Order::PaintTerrain { terrain, tiles }),
            Tool::Ore(ore) => orders.push(Order::PaintOre { ore, tiles }),
            Tool::Remove => orders.push(Order::RemoveAt { tiles }),
            Tool::ZombieBrush => orders.push(self.editor.brush_order(tiles)),
            _ => {}
        }
        self.paint_last = Some(to);
    }

    /// The selected buildings that have a rally point (barracks, tower) – `active`: only those of the active
    /// type (the Rally button), otherwise all of them (right click).
    fn rally_buildings(&self, world: &World, active: bool) -> Vec<u32> {
        let picked = if active { self.active_buildings(world) } else { self.selection_view(world).iter().filter_map(|p| match p {
            Pick::Building(b) => Some(*b),
            Pick::Unit(_) => None,
        }).collect() };
        picked
            .into_iter()
            .filter(|&b| world.buildings[b].kind == BuildingKind::Barracks || world.buildings[b].kind.is_tower())
            .map(|b| world.buildings[b].id)
            .collect()
    }

    /// Rally point of these buildings: on a unit (follow) or on a point.
    #[allow(clippy::too_many_arguments)]
    fn set_rally(&mut self, buildings: &[u32], world: &World, camera: &Camera, orders: &mut Vec<Order>, mouse: Vec2, point: Vec2, on_field: bool) {
        if buildings.is_empty() {
            return;
        }
        let rally = match unit_under_mouse(world, camera, mouse).filter(|_| on_field) {
            Some(i) => Rally::Unit(world.units[i].id),
            None => Rally::Point(point),
        };
        for &building in buildings {
            orders.push(Order::SetRally { building, rally });
        }
        self.marker = Some((point, 0.6, Mark::Move));
    }

    /// What T or M means on a zombie or a unit under the mouse – `None` if there is none (or it's P): then the
    /// command goes to the ground.
    fn aimed_at(&self, button: Button, world: &World, camera: &Camera, mouse: Vec2) -> Option<Command> {
        let unit = || unit_under_mouse(world, camera, mouse).map(|i| world.units[i].id);
        match button {
            // Attack (T) on a zombie = attack exactly that one (like in StarCraft). On one of your own units:
            // medics aim their beam at it, everyone else attack-moves there.
            Button::Attack => self.zombie_at(world, camera, mouse).map(Command::Attack).or_else(|| unit().map(Command::Heal)),
            // M-click on a unit = follow it.
            Button::Move => unit().map(Command::Follow),
            _ => None,
        }
    }

    /// The zombie under the mouse – only one you can see (in the game view not in the fog, not outside the border).
    fn zombie_at(&self, world: &World, camera: &Camera, mouse: Vec2) -> Option<u32> {
        zombie_under_mouse(world, camera, mouse, |p| self.shows_zombie(world, p))
    }

    /// The house of doom or the pile of loot under the mouse – only one that has been seen (in the game view they stay
    /// on the map once explored, like the terrain). Loot first: it lies in front of a house.
    fn place_at(&self, world: &World, camera: &Camera, mouse: Vec2) -> Option<Place> {
        let seen = |p: Vec2| {
            self.view_area(world).contains(p) && (self.sight() != Sight::Player || world.vision.is_explored(p.x.floor() as i32, p.y.floor() as i32))
        };
        let reach = 12.0 * camera.zoom;
        let loot = world.loot.iter().filter(|l| seen(l.pos)).find(|l| camera.world_to_screen(l.pos).distance(mouse) <= reach);
        if let Some(loot) = loot {
            return Some(Place::Loot(loot.id));
        }
        // Like `building_under_mouse`: the point on the ground below the mouse, and a bit lower – the house stands up.
        for step in 0..=3 {
            let ground = camera.screen_to_world(mouse + vec2(0.0, step as f32 * 0.25 * TILE_W * camera.zoom));
            if let Some(d) = world.doom_at_point(ground).filter(|&d| seen(world.doom[d].center())) {
                return Some(Place::Doom(world.doom[d].id));
            }
        }
        None
    }

    /// Sends a command to the selection and sets a marker at the target point. `formation`: the units line up in
    /// it at the target – it travels with the order, so it's the same on both PCs in co-op.
    fn give(&mut self, orders: &mut Vec<Order>, command: Command, queue: bool, point: Vec2, formation: Option<Formation>) {
        if self.selected.is_empty() {
            return;
        }
        orders.push(Order::Command { units: self.selected.clone(), command, queue, formation });
        let mark = match command {
            Command::Move(_) | Command::Follow(_) => Mark::Move,
            Command::Patrol(_) => Mark::Patrol,
            Command::Heal(_) => Mark::Heal,
            _ => Mark::Attack,
        };
        self.marker = Some((point, 0.6, mark));
    }

    /// A button of the command card – by key or by click. T, P, M, R wait for a target, B and V for a spot.
    /// The formation buttons only choose the shape – a command that is waiting keeps waiting.
    /// Training and barrels: `ctrl` 5 at once, `shift` the gold may go into the banks' credit.
    fn press(&mut self, button: Button, world: &World, orders: &mut Vec<Order>, shift: bool, ctrl: bool) {
        if let Some(shape) = button.formation() {
            self.formation = shape;
            return;
        }
        self.targeting = None;
        self.chained = false;
        self.parked = None;
        self.placing = None;
        self.editor.tool = None;
        let buildings = self.active_buildings(world);
        match button {
            Button::Stop => orders.push(Order::Stop { units: self.selected.clone() }),
            Button::Hold => {
                let units = self.selected.clone();
                orders.push(Order::Command { units, command: Command::Hold, queue: shift, formation: None });
            }
            Button::Move | Button::Patrol | Button::Attack | Button::Rally => self.targeting = Some(button),
            Button::Scan => match world.next_scan_in().filter(|_| world.scan_charges == 0) {
                Some(seconds) => self.alert(&format!("No scan ready - the next one in {}:{:02}", seconds / 60, seconds % 60)),
                None => self.targeting = Some(button),
            },
            Button::Drop => orders.push(Order::DropBarrels { units: self.selected.clone() }),
            Button::Dismiss => orders.push(Order::Dismiss { units: self.selected.clone() }),
            // All selected buildings, like repairing. In the editor's Edit mode they're only removed: no time passes
            // there, nothing is paid – and nothing comes back.
            Button::Demolish => {
                let buildings = self.selected_buildings.clone();
                let editing = self.editor.editing() && !self.editor.testing();
                orders.push(if editing { Order::RemoveBuildings { buildings } } else { Order::Demolish { buildings } });
            }
            // All selected markets: if one of them sells, all stop – otherwise all sell again.
            Button::Sell => {
                let on = !self.selling(world);
                let buildings = buildings.iter().map(|&b| world.buildings[b].id).collect();
                orders.push(Order::SetSelling { buildings, on });
            }
            // Like training: each barrel to the factory with the shortest queue, Ctrl 5 at once, as many as the gold
            // (with Shift also the credit) and the coal pay for.
            Button::MakeBarrel => {
                if self.editor.editing() && !self.editor.testing() {
                    self.alert(&format!("No time passes while editing - make barrels in a test ({})", self.settings.keys.label(Action::EditTest)));
                    return;
                }
                let blast = &data::get().buildings.blast;
                let mut queued: Vec<(usize, u32)> =
                    buildings.iter().map(|&b| (world.buildings[b].production.len(), world.buildings[b].id)).collect();
                let mut gold = if shift { world.spendable_on_credit() } else { world.gold };
                let mut coal = world.stock.get(Resource::Coal);
                for _ in 0..if ctrl { 5 } else { 1 } {
                    if gold < blast.cost {
                        let credit = !shift && world.spendable_on_credit() >= blast.cost;
                        self.alert(&if credit { Short::GoldCredit } else { Short::Gold }.alert());
                        return;
                    }
                    if coal < blast.cost_coal {
                        self.alert(&Short::Coal.alert());
                        return;
                    }
                    let Some(shortest) = queued.iter_mut().filter(|(len, _)| *len < MAX_PRODUCTION).min_by_key(|(len, _)| *len) else {
                        return;
                    };
                    let order = Order::MakeBarrel { building: shortest.1 };
                    orders.push(if shift { Order::Credit(Box::new(order)) } else { order });
                    shortest.0 += 1;
                    gold -= blast.cost;
                    coal -= blast.cost_coal;
                }
            }
            Button::Place(_) => {
                self.placing = button.places();
                self.free_placing = false;
            }
            Button::Open(menu) => self.submenu = Some(menu),
            Button::Back => self.submenu = None,
            Button::Planned(_) => self.alert(&format!("{} - planned, not in the game yet", button.title())),
            Button::Repair => {
                let selected = self.selected_building_indices(world);
                self.repair(world, orders, selected);
            }
            // All selected buildings that can be switched off: if one of them still runs, all go off – otherwise all
            // on again (the simulation checks the workers once more, `switch_off`).
            Button::Switch => {
                let switchable: Vec<usize> = self.switchable(world);
                let off = !self.switching_on(world);
                // Why one of them stays off (each on its own – the simulation switches them on one after another).
                let blocked = switchable.iter().filter(|_| !off).find_map(|&b| world.switch_on_blocked(b));
                if let Some(block) = blocked {
                    let short = match block {
                        SwitchBlock::Workers => Short::Workers,
                        SwitchBlock::Power => Short::Power,
                        SwitchBlock::Income => Short::Income,
                    };
                    self.alert(&short.alert());
                }
                let buildings = switchable.iter().map(|&b| world.buildings[b].id).collect();
                orders.push(Order::SwitchOff { buildings, off });
            }
            Button::Unload => {
                for b in buildings {
                    orders.push(Order::Unload { building: world.buildings[b].id });
                }
            }
            // With Ctrl 5 at once (StarCraft has Shift – here Shift buys on the banks' credit). Several barracks
            // selected: each unit goes to the one with the shortest queue. Only as many as the gold and the free workers
            // pay for (the simulation pays, `orders.rs`). Holding the key presses it again and again (`KeyRepeat`).
            Button::Train(_) => {
                let Some(kind) = button.trains() else {
                    return;
                };
                if self.editor.editing() && !self.editor.testing() {
                    self.alert(&format!("No time passes while editing - train units in a test ({})", self.settings.keys.label(Action::EditTest)));
                    return;
                }
                if let Some(needed) = kind.stats().requires.filter(|_| !world.unit_unlocked(kind)) {
                    self.alert(&Short::Locked(needed).alert());
                    return;
                }
                let mut queued: Vec<(usize, u32)> =
                    buildings.iter().map(|&b| (world.buildings[b].production.len(), world.buildings[b].id)).collect();
                let gold = if shift { world.spendable_on_credit() } else { world.gold };
                let (mut gold, mut workers) = (gold, world.free_workers());
                let stats = kind.stats();
                for _ in 0..if ctrl { 5 } else { 1 } {
                    if gold < stats.cost {
                        let credit = !shift && world.spendable_on_credit() >= stats.cost;
                        self.alert(&if credit { Short::GoldCredit } else { Short::Gold }.alert());
                        return;
                    }
                    if workers < stats.worker_cost {
                        self.alert(&Short::Workers.alert());
                        return;
                    }
                    let Some(shortest) = queued.iter_mut().min_by_key(|(len, _)| *len) else {
                        return;
                    };
                    let order = Order::Train { building: shortest.1, kind };
                    orders.push(if shift { Order::Credit(Box::new(order)) } else { order });
                    shortest.0 += 1;
                    gold -= stats.cost;
                    workers -= stats.worker_cost;
                }
            }
            Button::FormLine | Button::FormBox | Button::FormCircle => {} // handled above
        }
    }

    /// Do all selected markets sell (Q)? Then their button is lit.
    pub fn selling(&self, world: &World) -> bool {
        let markets: Vec<usize> = self.active_buildings(world);
        !markets.is_empty() && markets.iter().all(|&b| world.buildings[b].selling)
    }

    /// The selected buildings that can be switched off (Y) – not infected ones.
    fn switchable(&self, world: &World) -> Vec<usize> {
        let can = |&b: &usize| world.buildings[b].kind.can_switch_off() && !world.buildings[b].infected && world.buildings[b].is_built();
        self.selected_building_indices(world).into_iter().filter(can).collect()
    }

    /// Would Y switch the selection on again – is every one of them that can be switched off already off? Then its
    /// button is lit, like a command waiting for its target.
    pub fn switching_on(&self, world: &World) -> bool {
        let switchable = self.switchable(world);
        !switchable.is_empty() && switchable.iter().all(|&b| world.buildings[b].off)
    }

    /// The selected buildings (all types, not only the active one) – what F repairs.
    pub fn selected_building_indices(&self, world: &World) -> Vec<usize> {
        self.selected_buildings.iter().filter_map(|&id| world.find_building(id)).collect()
    }

    /// Repair these buildings (indices) – F: the selected ones, X: all. Cheapest first, as many as the gold pays for
    /// (`repair_plan`), and a message if not all of them get repaired. The simulation checks it all again when the
    /// order arrives (`start_repair`); `world.gold` is only read here.
    fn repair(&mut self, world: &World, orders: &mut Vec<Order>, candidates: Vec<usize>) {
        if self.editor.editing() && !self.editor.testing() {
            self.alert(&format!("No time passes while editing - repair in a test ({})", self.settings.keys.label(Action::EditTest)));
            return;
        }
        let plan = world.repair_plan(candidates);
        if plan.damaged == 0 {
            self.alert("Nothing to repair");
            return;
        }
        if plan.chosen.len() < plan.damaged {
            let short = plan.damaged - plan.blocked > plan.chosen.len();
            let why = match (plan.blocked > 0, short) {
                (true, true) => "zombies too close, not enough gold",
                (true, false) => "zombies too close",
                _ => "not enough gold",
            };
            let (done, of) = (plan.chosen.len(), plan.damaged);
            self.alert(&if done == 0 { format!("Nothing repaired - {why}") } else { format!("Repairing {done} of {of} - {why}") });
        }
        if !plan.chosen.is_empty() {
            let buildings = plan.chosen.iter().map(|&b| world.buildings[b].id).collect();
            orders.push(Order::Repair { buildings });
        }
    }

    /// Keys of the command card. Only the buttons that are visible right now react.
    fn command_keys(&mut self, world: &World, orders: &mut Vec<Order>, card: &[Button], shift: bool, ctrl: bool) {
        // Esc: first the waiting command or the building on the mouse, then the build menu (back to the main buttons).
        if is_key_pressed(KeyCode::Escape) {
            self.parked = None;
            if self.targeting.is_some() || self.placing.is_some() {
                self.targeting = None;
                self.placing = None;
            } else {
                self.submenu = None;
            }
            return;
        }
        if card.is_empty() {
            // Nothing selected that has these commands any more – but a building from the editor's panel hangs on no
            // selection (`free_placing`), it stays on the mouse.
            self.targeting = None;
            if !self.free_placing {
                self.placing = None;
            }
            return;
        }
        // R turns the gate on the mouse (like in Factorio) – while placing it, R isn't the stakes of the menu.
        if self.placing == Some(BuildingKind::Gate) && self.settings.keys.pressed(Action::TurnGate) {
            self.turned = !self.turned;
            return;
        }
        for &button in card {
            let repeats = button.trains().is_some() || button == Button::MakeBarrel;
            // `into_iter().flatten()`: the keys that are there (the `Some`s of the two places).
            let keys = self.settings.keys.keys(Action::of(button));
            let pressed = keys.into_iter().flatten().any(|key| if repeats { self.repeat.pressed(key) } else { is_key_pressed(key) });
            if pressed {
                self.press(button, world, orders, shift, ctrl);
                break; // the card may change (a build menu opens) – one button per key press
            }
        }
    }

    /// Click on an icon in the selection panel (like in StarCraft):
    /// only this unit / Shift: this one out / Ctrl: only this type / Ctrl+Shift: this type out.
    fn click_icon(&mut self, world: &World, pick: Pick, shift: bool, ctrl: bool) {
        match pick {
            Pick::Unit(i) => {
                let (id, kind) = (world.units[i].id, world.units[i].kind);
                let is_kind = |other: u32| world.find_unit(other).is_some_and(|j| world.units[j].kind == kind);
                match (shift, ctrl) {
                    (false, false) => self.select_units(vec![id]),
                    (true, false) => self.selected.retain(|&other| other != id),
                    (false, true) => {
                        self.selected.retain(|&other| is_kind(other));
                        self.selected_buildings.clear();
                    }
                    (true, true) => self.selected.retain(|&other| !is_kind(other)),
                }
            }
            Pick::Building(b) => {
                let (id, kind) = (world.buildings[b].id, world.buildings[b].kind);
                let is_kind = |other: u32| world.find_building(other).is_some_and(|j| world.buildings[j].kind == kind);
                match (shift, ctrl) {
                    (false, false) => self.set_selection(Vec::new(), vec![id]),
                    (true, false) => self.selected_buildings.retain(|&other| other != id),
                    (false, true) => {
                        self.selected_buildings.retain(|&other| is_kind(other));
                        self.selected.clear();
                    }
                    (true, true) => self.selected_buildings.retain(|&other| !is_kind(other)),
                }
            }
        }
    }

    /// A click (barely moved) selects one unit, dragging selects everyone in the box.
    /// Shift: add, or remove again by clicking. Ctrl-click or double click: all of this type on screen.
    /// A click or box on nothing clears the selection like in TAB (StarCraft keeps it – but then there was no quick
    /// way to get rid of it, and WASD only moves the camera with nothing selected). With Shift or Ctrl it stays.
    #[allow(clippy::too_many_arguments)]
    fn select(&mut self, world: &World, camera: &Camera, layout: &Layout, start: Vec2, end: Vec2, shift: bool, ctrl: bool) {
        let rect = Rect::new(
            start.x.min(end.x),
            start.y.min(end.y),
            (start.x - end.x).abs(),
            (start.y - end.y).abs(),
        );

        if rect.w >= 5.0 || rect.h >= 5.0 {
            let found: Vec<u32> = world
                .units
                .iter()
                .filter(|unit| !unit.is_inside() && rect.contains(camera.world_to_screen(unit.pos)))
                .map(|unit| unit.id)
                .collect();
            if !found.is_empty() {
                self.add_or_replace(found, shift);
                return;
            }
            // No unit in the box: the buildings in it (like in StarCraft – units always win).
            let buildings: Vec<u32> = world
                .buildings
                .iter()
                .filter(|b| rect.contains(camera.world_to_screen(b.center())))
                .map(|b| b.id)
                .collect();
            if buildings.is_empty() && !shift && !ctrl {
                self.deselect();
            }
            self.add_or_replace_buildings(buildings, shift);
            return;
        }

        let Some(i) = unit_under_mouse(world, camera, end) else {
            // No unit, but a zombie: show its values (the selection goes – like clicking an enemy in StarCraft). With
            // Shift or Ctrl a zombie can't be added to your units, so then it's ignored.
            if let Some(zombie) = self.zombie_at(world, camera, end).filter(|_| !shift && !ctrl) {
                self.deselect();
                self.selected_zombie = Some(zombie);
                return;
            }
            // The same for a house of doom or a pile of loot.
            if let Some(place) = self.place_at(world, camera, end).filter(|_| !shift && !ctrl) {
                self.deselect();
                self.selected_place = Some(place);
                return;
            }
            // No unit, but a building: select it. Shift: add or take out. Ctrl or double click: all of its type on
            // the screen. Nothing at all: the selection goes.
            match building_under_mouse(world, camera, end) {
                Some(b) => self.click_building(world, camera, layout, b, shift, ctrl),
                None if !shift && !ctrl => self.deselect(),
                None => {}
            }
            return;
        };
        let (id, kind) = (world.units[i].id, world.units[i].kind);
        let now = get_time();
        let double = matches!(self.last_click, Some((last, time)) if last == id && now - time < DOUBLE_TAP);
        self.last_click = Some((id, now));

        if ctrl || double {
            let same_kind: Vec<u32> = world
                .units
                .iter()
                .filter(|unit| unit.kind == kind && !unit.is_inside() && layout.in_game_view(camera.world_to_screen(unit.pos)))
                .map(|unit| unit.id)
                .collect();
            self.add_or_replace(same_kind, shift);
        } else if shift {
            match self.selected.iter().position(|&s| s == id) {
                Some(already) => {
                    self.selected.remove(already);
                }
                None => self.selected.push(id),
            }
        } else {
            self.select_units(vec![id]);
        }
    }

    fn add_or_replace(&mut self, found: Vec<u32>, shift: bool) {
        if found.is_empty() {
            return;
        }
        if !shift {
            self.set_selection(Vec::new(), Vec::new());
        }
        for id in found {
            if !self.selected.contains(&id) {
                self.selected.push(id);
            }
        }
    }

    /// The same for buildings.
    fn add_or_replace_buildings(&mut self, found: Vec<u32>, shift: bool) {
        if found.is_empty() {
            return;
        }
        if !shift {
            self.set_selection(Vec::new(), Vec::new());
        }
        for id in found {
            if !self.selected_buildings.contains(&id) {
                self.selected_buildings.push(id);
            }
        }
    }

    /// A click on a building: select it alone – Shift: add it or take it out again – Ctrl or double click: all
    /// buildings of its type on the screen.
    fn click_building(&mut self, world: &World, camera: &Camera, layout: &Layout, b: usize, shift: bool, ctrl: bool) {
        let (id, kind) = (world.buildings[b].id, world.buildings[b].kind);
        let now = get_time();
        let double = matches!(self.last_building_click, Some((last, time)) if last == id && now - time < DOUBLE_TAP);
        self.last_building_click = Some((id, now));
        self.targeting = None;
        if ctrl || double {
            let same_kind: Vec<u32> = world
                .buildings
                .iter()
                .filter(|other| other.kind == kind && layout.in_game_view(camera.world_to_screen(other.center())))
                .map(|other| other.id)
                .collect();
            self.add_or_replace_buildings(same_kind, shift);
        } else if shift {
            match self.selected_buildings.iter().position(|&s| s == id) {
                Some(already) => {
                    self.selected_buildings.remove(already);
                }
                None => self.selected_buildings.push(id),
            }
        } else {
            self.set_selection(Vec::new(), vec![id]);
        }
    }

    /// Control groups, F2, Tab, camera bookmarks, Backspace.
    fn selection_keys(&mut self, world: &World, camera: &mut Camera, card: &[Button], shift: bool, ctrl: bool) {
        for g in 0..10 {
            if self.global_pressed(Action::Group(g), card) {
                self.group_action(g, world, camera, shift, ctrl);
            }
        }

        // F2: the whole army (like in StarCraft 2).
        if self.global_pressed(Action::SelectArmy, card) {
            self.select_units(world.units.iter().filter(|u| !u.is_inside()).map(|unit| unit.id).collect());
        }

        // Tab: to the next type in the selection, Shift+Tab: to the previous one.
        if self.global_pressed(Action::NextType, card) {
            let subs = self.subs_in_selection(world);
            if let Some(pos) = subs.iter().position(|&k| Some(k) == self.active) {
                let next = (if shift { pos + subs.len() - 1 } else { pos + 1 }) % subs.len();
                self.active = Some(subs[next]);
                self.submenu = None;
            }
        }

        for b in 0..4 {
            if self.global_pressed(Action::Bookmark(b), card) {
                if ctrl {
                    self.bookmarks[b] = Some(camera.center);
                } else if let Some(center) = self.bookmarks[b] {
                    camera.center = center;
                }
            }
        }
        // Backspace: camera to the base (like StarCraft's jump to the main building).
        if self.global_pressed(Action::Base, card) {
            camera.center = world.base;
        }
    }

    /// A control group – by number key or by click on the panel, with the same modifier keys.
    fn group_action(&mut self, g: usize, world: &World, camera: &mut Camera, shift: bool, ctrl: bool) {
        // Nothing selected: nothing to save.
        if (ctrl || shift) && !self.has_selection() {
            return;
        }
        let selection = Group { units: self.selected.clone(), buildings: self.selected_buildings.clone() };
        match (ctrl, shift) {
            // Ctrl: set the group anew.
            (true, false) => self.groups[g] = selection,
            // Ctrl+Shift: set anew and take the units and buildings out of all other groups ("steal").
            (true, true) => {
                for (other, group) in self.groups.iter_mut().enumerate() {
                    if other != g {
                        group.units.retain(|id| !selection.units.contains(id));
                        group.buildings.retain(|id| !selection.buildings.contains(id));
                    }
                }
                self.groups[g] = selection;
            }
            // Shift: add the selection to the group.
            (false, true) => {
                let group = &mut self.groups[g];
                for id in selection.units {
                    if !group.units.contains(&id) {
                        group.units.push(id);
                    }
                }
                for id in selection.buildings {
                    if !group.buildings.contains(&id) {
                        group.buildings.push(id);
                    }
                }
            }
            (false, false) => self.recall_group(g, world, camera),
        }
    }

    /// Select the group – twice quickly in a row: the camera jumps there.
    fn recall_group(&mut self, g: usize, world: &World, camera: &mut Camera) {
        if self.groups[g].is_empty() {
            return;
        }
        let now = get_time();
        let double = matches!(self.last_group, Some((last, time)) if last == g && now - time < DOUBLE_TAP);
        // Units in a tower stay in the group, but aren't selected.
        let units: Vec<u32> =
            self.groups[g].units.iter().copied().filter(|&id| world.find_unit(id).is_some_and(|i| !world.units[i].is_inside())).collect();
        let buildings = self.groups[g].buildings.clone();
        self.set_selection(units, buildings);
        if double {
            if let Some(center) = center_of(world, &self.selected, &self.selected_buildings) {
                camera.center = center;
            }
        }
        self.last_group = Some((g, now));
    }

    /// The types in the selection, without duplicates, in a fixed order (units first).
    fn subs_in_selection(&self, world: &World) -> Vec<Sub> {
        let mut subs: Vec<Sub> = self
            .selection_view(world)
            .iter()
            .map(|pick| match *pick {
                Pick::Unit(i) => Sub::Unit(world.units[i].kind),
                Pick::Building(b) => Sub::Building(world.buildings[b].kind),
            })
            .collect();
        subs.dedup(); // works because `selection_view` is sorted by type
        subs
    }

    /// If the active type isn't in the selection anymore, the first type becomes active. A build menu only stays
    /// open while a command center is active.
    fn fix_active(&mut self, world: &World) {
        let subs = self.subs_in_selection(world);
        if !self.active.is_some_and(|k| subs.contains(&k)) {
            self.active = subs.first().copied();
        }
        if self.active != Some(Sub::Building(BuildingKind::CommandCenter)) {
            self.submenu = None;
        }
    }

    /// Time, overlay, editor. The developer tools are buttons in the editor now (F10) –
    /// so the keys Insert, Home, Delete, End, Page Up/Down, F4 and F11 are free again.
    fn other_keys(&mut self, card: &[Button]) {
        if self.global_pressed(Action::Pause, card) {
            self.paused = !self.paused;
        }
        if self.global_pressed(Action::Faster, card) {
            self.speed = (self.speed * 2).min(8);
        }
        if self.global_pressed(Action::Slower, card) {
            self.speed = (self.speed / 2).max(1);
        }
        if self.global_pressed(Action::Overlay, card) {
            self.settings.dev_overlay = !self.settings.dev_overlay;
        }
        // Game view ↔ editor mode (the two buttons in the top right).
        if self.global_pressed(Action::EditTest, card) {
            self.editor.toggle();
        }
        // Emergency exit if the confined mouse gets in the way (the setting is also in the menu).
        if self.global_pressed(Action::ConfineMouse, card) {
            self.settings.confine_mouse = !self.settings.confine_mouse;
        }
    }

    /// Camera: arrow keys, mouse at the window edge, mouse wheel. `keys`: may the keyboard move it (not while
    /// a name is being typed)?
    fn move_camera(&mut self, world: &World, camera: &mut Camera, mouse: Vec2, dt: f32, keys: bool) {
        let mut pan = Vec2::ZERO;
        // A camera key that's also on a card (W A S D) only while nothing is selected – units and buildings have their
        // hotkeys right there (S = stop, Q, W, E …), and a key that sometimes moves the camera and sometimes doesn't
        // is a trap. Ctrl held: not either (Ctrl+S, Ctrl+A … should never move the camera). The arrows always.
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
        let selected = self.has_selection();
        for (action, dir) in CAMERA {
            for key in self.settings.keys.keys(action).into_iter().flatten() {
                let free = !self.settings.keys.on_any_card(key) || (!ctrl && !selected);
                if keys && is_key_down(key) && free {
                    pan += dir;
                }
            }
        }
        // At the edge only the real mouse counts: if the window has no focus or the mouse is outside
        // (e.g. on the second monitor), nothing scrolls. Also not while a selection box is being dragged
        // (it hangs on the screen, not on the world) or the camera is moved another way.
        let busy = self.drag_start.is_some() || self.minimap_drag || self.pan_drag.is_some() || self.rect_drag.is_some();
        if let Some(mouse) = self.real_mouse.filter(|_| self.settings.edge_scroll && !busy) {
            if mouse.x < EDGE {
                pan.x -= 1.0;
            }
            if mouse.x > screen_width() - EDGE {
                pan.x += 1.0;
            }
            if mouse.y < EDGE {
                pan.y -= 1.0;
            }
            if mouse.y > screen_height() - EDGE {
                pan.y += 1.0;
            }
        }
        // Length 1, so diagonal isn't faster and arrow + edge don't count twice.
        let area = self.view_area(world);
        camera.pan(pan.normalize_or_zero() * self.settings.scroll_speed * dt, area);

        // Hold the middle mouse button and drag: "grab" the map and move it.
        if is_mouse_button_down(MouseButton::Middle) {
            if let Some(last) = self.pan_drag {
                camera.pan(last - mouse, area);
            }
            self.pan_drag = Some(mouse);
        } else {
            self.pan_drag = None;
        }

        let (_, wheel) = mouse_wheel();
        // On the editor panel the wheel scrolls the panel (egui does that), not the map.
        if !self.editor.has_mouse() {
            if wheel > 0.0 {
                camera.zoom_by(1.15);
            } else if wheel < 0.0 {
                camera.zoom_by(1.0 / 1.15);
            }
        }
    }

    /// What lies on the map under the mouse? Not if the mouse is on a panel or a tool is in hand.
    fn update_hover(&mut self, world: &World, camera: &Camera, layout: &Layout, mouse: Vec2) {
        self.hover_unit = None;
        self.hover_zombie = None;
        self.hover_building = None;
        let busy = self.drag_start.is_some() || self.placing.is_some() || self.editor.tool.is_some();
        if !layout.in_game_view(mouse) || self.editor.has_mouse() || busy {
            return;
        }
        self.hover_zombie = self.zombie_at(world, camera, mouse);
        if self.hover_zombie.is_none() {
            self.hover_unit = unit_under_mouse(world, camera, mouse).map(|i| world.units[i].id);
        }
        if self.hover_zombie.is_none() && self.hover_unit.is_none() {
            self.hover_building = building_under_mouse(world, camera, mouse).map(|b| world.buildings[b].id);
        }
    }

    /// Selection box, highlights, target marker and the waiting command or tool on the mouse.
    pub fn draw(&self, world: &World, camera: &Camera) {
        let mouse = Vec2::from(mouse_position());

        if let Some(start) = self.drag_start {
            let min = start.min(mouse);
            let size = (start - mouse).abs();
            draw_rectangle_lines(min.x, min.y, size.x, size.y, 1.5, GREEN);
            // Preview like in StarCraft: whoever stands in the box is already highlighted.
            let rect = Rect::new(min.x, min.y, size.x, size.y);
            for unit in &world.units {
                if rect.contains(camera.world_to_screen(unit.pos)) {
                    draw_ring(camera, unit.pos, UNIT_RADIUS, 1.5, WHITE);
                }
            }
        }
        // Under the mouse: zombie red (right click = attack), own unit white.
        if let Some(z) = self.hover_zombie.and_then(|id| world.find_zombie(id)) {
            draw_ring(camera, world.zombies[z].pos, world.zombies[z].radius, 1.5, RED);
        }
        // The zombie clicked on: a thick red ring. A house of doom or loot: a red frame on the ground around it.
        if let Some(z) = self.selected_zombie.and_then(|id| world.find_zombie(id)) {
            let s = camera.world_to_screen(world.zombies[z].pos);
            draw_ring(camera, world.zombies[z].pos, world.zombies[z].radius, 2.5, RED);
            // Where it was lured to: a thin line there and a small cross.
            if let Some(to) = world.zombies[z].lured_to() {
                let t = camera.world_to_screen(to);
                draw_line(s.x, s.y, t.x, t.y, 1.0, Color { a: 0.6, ..RED });
                draw_line(t.x - 5.0, t.y - 5.0, t.x + 5.0, t.y + 5.0, 2.0, RED);
                draw_line(t.x - 5.0, t.y + 5.0, t.x + 5.0, t.y - 5.0, 2.0, RED);
            }
        }
        let frame = match self.selected_place {
            Some(Place::Doom(id)) => world.find_doom(id).map(|d| (world.doom[d].corner(), world.doom[d].far_corner())),
            Some(Place::Loot(id)) => world.find_loot(id).map(|l| (world.loot[l].pos - Vec2::splat(0.4), world.loot[l].pos + Vec2::splat(0.4))),
            None => None,
        };
        if let Some((c, f)) = frame {
            let corners = [c, vec2(f.x, c.y), f, vec2(c.x, f.y)].map(|p| camera.world_to_screen(p));
            for k in 0..4 {
                draw_line(corners[k].x, corners[k].y, corners[(k + 1) % 4].x, corners[(k + 1) % 4].y, 2.5, RED);
            }
        }
        if let Some(i) = self.hover_unit.and_then(|id| world.find_unit(id)) {
            draw_ring(camera, world.units[i].pos, UNIT_RADIUS, 1.5, WHITE);
        }
        // Buildings: outline on the ground – white under the mouse, green when selected, plus the rally point.
        let outline = |b: usize, color: Color| {
            let building = &world.buildings[b];
            let (c, f) = (building.corner(), building.far_corner());
            let corners = [c, vec2(f.x, c.y), f, vec2(c.x, f.y)];
            for k in 0..4 {
                let (p, q) = (camera.world_to_screen(corners[k]), camera.world_to_screen(corners[(k + 1) % 4]));
                draw_line(p.x, p.y, q.x, q.y, 2.0, color);
            }
        };
        if let Some(b) = self.hover_building.and_then(|id| world.find_building(id)) {
            outline(b, WHITE);
        }
        for b in self.selected_buildings.iter().filter_map(|&id| world.find_building(id)) {
            outline(b, GREEN);
            let building = &world.buildings[b];
            let target = match building.rally {
                Some(Rally::Point(p)) => Some(p),
                Some(Rally::Unit(id)) => world.find_unit(id).map(|u| world.units[u].pos),
                None => None,
            };
            if let Some(target) = target {
                let (from, to) = (camera.world_to_screen(building.center()), camera.world_to_screen(target));
                draw_line(from.x, from.y, to.x, to.y, 1.5, Color { a: 0.6, ..GREEN });
                draw_circle(to.x, to.y, 4.0, GREEN);
            }
        }
        // Building on the mouse – while dragging the whole line: green where there's room, red otherwise.
        // Around the building under the mouse the zone that must be free of zombies (4 tiles all around):
        // a thin frame, red while a zombie is in it.
        if let Some(kind) = self.placing {
            let turned = self.turned;
            let here = placement_tile(kind, turned, camera.screen_to_world(mouse));
            let (min, max) = World::build_zone(kind, here, turned);
            let frame = if world.zombies_in_build_zone(kind, here, turned) {
                Color::new(1.0, 0.25, 0.2, 0.9)
            } else {
                Color::new(1.0, 1.0, 1.0, 0.35)
            };
            // The zone free of zombies only counts by the rules of the game – the editor builds anywhere.
            let free = self.rule_free();
            if !free {
                draw_ground_frame(camera, min, max, 1.5, frame);
            }
            let tiles = match self.place_start {
                Some(start) => placement_line(kind, start, here),
                None => vec![here],
            };
            let (w, h) = kind.footprint(turned);
            // Green: it will be built – red: no room there, or the gold or the workers don't reach that far.
            let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
            let plan = self.affordable(world, kind, &tiles, shift);
            for tile in tiles {
                let color = if plan.built.contains(&tile) {
                    Color::new(0.3, 0.9, 0.3, 0.45)
                } else {
                    Color::new(0.9, 0.2, 0.2, 0.45)
                };
                draw_diamond(camera, tile_corner(tile), vec2(w as f32, h as f32), color);
                // The gate: its door (the middle) lighter – that's where your units walk through.
                if kind == BuildingKind::Gate {
                    let door = tile_corner(tile) + vec2((w / 2) as f32, (h / 2) as f32);
                    draw_diamond(camera, door + 0.2, Vec2::splat(0.6), Color::new(1.0, 1.0, 1.0, 0.35));
                }
            }
            // On walls the gate is cheaper – the text says so, so it doesn't look like a mistake.
            let on_walls = world.replaced_by(kind, here, turned).len();
            let stats = kind.stats();
            let mut text = if free { "Free (editor)".to_owned() } else { format!("Cost {}", plan.cost) };
            let pieces = plan.built.len().max(1) as u32;
            if !free && stats.cost_wood > 0 {
                text += &format!(", {} wood", stats.cost_wood * pieces);
            }
            if !free && stats.cost_stone > 0 {
                text += &format!(", {} stone", stats.cost_stone * pieces);
            }
            if !free && stats.cost_coal > 0 {
                text += &format!(", {} coal", stats.cost_coal * pieces);
            }
            if !free && stats.cost_iron > 0 {
                text += &format!(", {} iron", stats.cost_iron * pieces);
            }
            if plan.workers > 0 {
                text += &format!(", {} worker{}", plan.workers, if plan.workers == 1 { "" } else { "s" });
            }
            if on_walls > 0 && self.place_start.is_none() && !free {
                text += &format!(" (on {on_walls} wall{})", if on_walls == 1 { "" } else { "s" });
            }
            match plan.short {
                Some(Short::Gold) => text += &format!(" - not enough gold ({})", world.gold),
                Some(Short::GoldCredit) => text += &format!(" - not enough gold ({}) - Shift: on credit", world.gold),
                Some(Short::Barrels) => text += " - no barrel in store",
                Some(Short::Wood) => text += &format!(" - not enough wood ({})", world.stock.get(Resource::Wood)),
                Some(Short::Stone) => text += &format!(" - not enough stone ({})", world.stock.get(Resource::Stone)),
                Some(Short::Coal) => text += &format!(" - not enough coal ({})", world.stock.get(Resource::Coal)),
                Some(Short::Iron) => text += &format!(" - not enough iron ({})", world.stock.get(Resource::Iron)),
                Some(Short::Workers) => text += &format!(" - not enough workers ({} free)", world.free_workers()),
                Some(short @ (Short::Locked(_) | Short::Power | Short::Income)) => text += &format!(" - {}", short.alert().to_lowercase()),
                None => {}
            }
            if kind == BuildingKind::Gate {
                text += "   R: turn";
            }
            let color = if plan.short.is_some() { Color::new(1.0, 0.45, 0.4, 1.0) } else { WHITE };
            draw_text(&text, mouse.x + 14.0, mouse.y - 8.0, 20.0, color);
            self.draw_economy_hints(world, camera, kind, here, mouse);
        }
        // Editor tool on the mouse: the brush in the colour of the terrain, or the name of what gets spawned.
        if let Some(tool) = self.editor.tool.filter(|_| !self.editor.has_mouse()) {
            match tool {
                Tool::Paint(_) | Tool::Ore(_) | Tool::Remove | Tool::ZombieBrush => {
                    let color = match tool {
                        Tool::Paint(Terrain::Grass) | Tool::Ore(None) => Color::new(1.0, 1.0, 1.0, 0.25),
                        Tool::Paint(terrain) => Color { a: 0.6, ..terrain_color(terrain) },
                        Tool::Ore(Some(ore)) => Color { a: 0.6, ..ore_color(ore) },
                        Tool::Remove => Color::new(0.9, 0.15, 0.15, 0.35),
                        Tool::ZombieBrush => Color { a: 0.35, ..ZOMBIE },
                        _ => WHITE,
                    };
                    let tile = tile_of(camera.screen_to_world(mouse));
                    let tiles = if tool == Tool::Ore(Some(Ore::Oil)) { oil_node(tile) } else { self.editor.brush_tiles(tile) };
                    for (x, y) in tiles {
                        draw_diamond(camera, vec2(x as f32, y as f32), Vec2::ONE, color);
                    }
                    if tool == Tool::ZombieBrush {
                        draw_text(self.editor.brush_text(), mouse.x + 14.0, mouse.y - 8.0, 20.0, ZOMBIE);
                    }
                }
                Tool::ZombieDrop => {
                    // The area where they land: a circle on the ground – on screen a flat ellipse (like the tiles).
                    let w = self.editor.drop_radius() * TILE_W * std::f32::consts::FRAC_1_SQRT_2 * camera.zoom;
                    draw_ellipse_lines(mouse.x, mouse.y, w, w / 2.0, 0.0, 2.0, ZOMBIE);
                    draw_text(format!("{} zombies", self.editor.drop), mouse.x + 14.0, mouse.y - 8.0, 20.0, ZOMBIE);
                }
                Tool::Unit(_) | Tool::Spawner | Tool::Doom | Tool::Loot | Tool::Giant => {
                    draw_circle_lines(mouse.x, mouse.y, 10.0, 2.0, YELLOW);
                    draw_text(tool.name(), mouse.x + 14.0, mouse.y - 8.0, 20.0, YELLOW);
                }
                // Its edges light up – below.
                Tool::PlayArea | Tool::Building(_) => {}
            }
        }
        // Formation being drawn: the dragged line and a ring at every spot – soldiers in front. Only once it's really
        // dragged: a click sends everyone to the point.
        let dragged = self.form_drag.filter(|drag| camera.screen_to_world(mouse).distance(drag.from) >= MIN_DRAG);
        if let Some(drag) = dragged {
            let to = camera.screen_to_world(mouse);
            let units: Vec<(Vec2, UnitKind)> =
                self.selection_view(world)
                    .iter()
                    .filter_map(|pick| match *pick {
                        Pick::Unit(i) => Some((world.units[i].pos, world.units[i].kind)),
                        Pick::Building(_) => None,
                    })
                    .collect();
            let color = mark_color(button_mark(drag.button));
            let (a, b) = (camera.world_to_screen(drag.from), camera.world_to_screen(to));
            draw_line(a.x, a.y, b.x, b.y, 2.0, Color { a: 0.6, ..color });
            let formation = Formation { shape: self.formation, from: drag.from, to };
            for (spot, (_, kind)) in formation::spots(formation, &units).into_iter().zip(&units) {
                let ring = if *kind == UnitKind::Soldier { color } else { Color { a: 0.6, ..color } };
                draw_ring(camera, spot, UNIT_RADIUS, 1.5, ring);
            }
        }
        // Editor: the edge under the mouse (or being dragged) lights up white – of the selected spawner, or of the border
        // while its tool is in hand. The border being dragged shows where it will lie.
        if self.editor.editing() {
            let point = camera.screen_to_world(mouse);
            let free = !self.editor.has_mouse();
            let spawner = self.editor.spawner.and_then(|id| world.find_spawner(id)).map(|s| &world.spawners[s]);
            // (axes, corner, size, the edges under the mouse)
            let lit = match (self.rect_drag, self.editor.tool, spawner) {
                (Some(drag), _, _) => {
                    let (min, size) = drag.rect_at(point);
                    if drag.what == Dragged::PlayArea {
                        draw_edges(camera, drag.axes, min, size, Edges::ALL, 2.0);
                    }
                    Some((drag.axes, min, size, Some(drag.edges)))
                }
                (None, Some(Tool::PlayArea), _) if free => {
                    let (min, size) = world.map.play_steps();
                    Some((Axes::Screen, min, size, grab(min, size, Axes::Screen, point, camera)))
                }
                (None, None, Some(s)) if free => Some((s.axes, s.min, s.size, grab(s.min, s.size, s.axes, point, camera))),
                _ => None,
            };
            if let Some((axes, min, size, Some(edges))) = lit.filter(|l| l.3 != Some(Edges::ALL)) {
                draw_edges(camera, axes, min, size, edges, 4.0);
            }
        }
        if let Some((target, time_left, mark)) = self.marker {
            let s = camera.world_to_screen(target);
            let w = 24.0 * camera.zoom * (time_left / 0.6 + 0.5);
            draw_ellipse_lines(s.x, s.y, w, w / 2.0, 0.0, 2.0, mark_color(mark));
        }
        if let Some(button) = self.targeting {
            let color = mark_color(button_mark(button));
            draw_circle_lines(mouse.x, mouse.y, 10.0, 2.0, color);
            draw_text(button.name(), mouse.x + 14.0, mouse.y - 8.0, 20.0, color);
        }
        // The message at the top (`alert`) – it fades out in its last half second.
        if let Some((text, time_left)) = &self.alert {
            let fade = (time_left * 2.0).min(1.0);
            let w = measure_text(text, None, 24, 1.0).width + 32.0;
            let (x, y) = ((screen_width() - w) / 2.0, 48.0);
            draw_rectangle(x, y, w, 38.0, Color::new(0.08, 0.09, 0.11, 0.85 * fade));
            draw_rectangle_lines(x, y, w, 38.0, 1.0, Color::new(0.95, 0.62, 0.20, fade));
            draw_text(text, x + 16.0, y + 26.0, 24.0, Color::new(1.0, 1.0, 1.0, fade));
        }
    }
}

/// The command T, P or M gives on the ground.
fn point_command(button: Button, point: Vec2) -> Command {
    match button {
        Button::Attack => Command::AttackMove(point),
        Button::Patrol => Command::Patrol(point),
        _ => Command::Move(point),
    }
}

/// The colour of a waiting command (T red, P yellow, everything else green).
fn button_mark(button: Button) -> Mark {
    match button {
        Button::Attack => Mark::Attack,
        Button::Patrol => Mark::Patrol,
        _ => Mark::Move,
    }
}

/// A tile (or a size in tiles) as a point in world coordinates.
fn tile_corner(tile: (i32, i32)) -> Vec2 {
    vec2(tile.0 as f32, tile.1 as f32)
}

/// The outline of a rectangle on the ground, from corner `min` to corner `max` (world) – on screen a diamond, like the
/// tiles.
fn draw_ground_frame(camera: &Camera, min: Vec2, max: Vec2, thickness: f32, color: Color) {
    let corners = [min, vec2(max.x, min.y), max, vec2(min.x, max.y)];
    for k in 0..4 {
        let (p, q) = (camera.world_to_screen(corners[k]), camera.world_to_screen(corners[(k + 1) % 4]));
        draw_line(p.x, p.y, q.x, q.y, thickness, color);
    }
}

/// A circle on the ground around `center` (world), `radius` tiles – on screen a flat ellipse, like the tiles.
fn draw_ground_circle(camera: &Camera, center: Vec2, radius: f32, thickness: f32, color: Color) {
    let s = camera.world_to_screen(center);
    let w = radius * TILE_W * std::f32::consts::FRAC_1_SQRT_2 * camera.zoom;
    draw_ellipse_lines(s.x, s.y, w, w / 2.0, 0.0, thickness, color);
}

/// How close to its edge (in steps of its axes) a spawner can be grabbed: `GRAB_PIXELS` on screen. Straight: a step
/// across the screen is twice as wide as a step down. Slanted: one tile – two lines a tile apart along a tile's edge lie
/// `TILE_W · TILE_H / 2 / edge` pixels apart (the area of a tile's diamond over the length of its edge).
fn grab_tolerance(camera: &Camera, axes: Axes) -> Vec2 {
    match axes {
        Axes::Screen => {
            let down = (GRAB_PIXELS / (TILE_H / 2.0 * camera.zoom)).clamp(0.4, 3.0);
            vec2(down / 2.0, down)
        }
        Axes::Tiles => {
            let apart = TILE_W * TILE_H / 2.0 / vec2(TILE_W / 2.0, TILE_H / 2.0).length() * camera.zoom;
            Vec2::splat((GRAB_PIXELS / apart).clamp(0.2, 1.5))
        }
    }
}

/// These edges of a rectangle (`min`, `size` in `axes`, like a spawner) as white lines on the ground, `width` pixels.
fn draw_edges(camera: &Camera, axes: Axes, min: (i32, i32), size: (i32, i32), edges: Edges, width: f32) {
    let (min, max) = (tile_corner(min), tile_corner(min) + tile_corner(size));
    let line = |p: Vec2, q: Vec2| {
        let (p, q) = (camera.world_to_screen(axes.back(p)), camera.world_to_screen(axes.back(q)));
        draw_line(p.x, p.y, q.x, q.y, width, WHITE);
    };
    if edges.x_min {
        line(min, vec2(min.x, max.y));
    }
    if edges.x_max {
        line(vec2(max.x, min.y), max);
    }
    if edges.y_min {
        line(min, vec2(max.x, min.y));
    }
    if edges.y_max {
        line(vec2(min.x, max.y), max);
    }
}

/// Which edges of a rectangle (`min`, `size` in `axes` – a spawner, the border) lie close to the world point `point`
/// (`grab_tolerance`) – at a corner two. Inside, away from the edges: all four (it moves as a whole). `None` if the
/// point lies outside.
fn grab(min: (i32, i32), size: (i32, i32), axes: Axes, point: Vec2, camera: &Camera) -> Option<Edges> {
    let (min, max) = (tile_corner(min), tile_corner(min) + tile_corner(size));
    let t = grab_tolerance(camera, axes);
    let p = axes.of(point);
    if p.x < min.x - t.x || p.y < min.y - t.y || p.x > max.x + t.x || p.y > max.y + t.y {
        return None;
    }
    let near = |a: f32, b: f32, t: f32| (a - b).abs() < t;
    let mut e = Edges { x_min: near(p.x, min.x, t.x), x_max: near(p.x, max.x, t.x), y_min: near(p.y, min.y, t.y), y_max: near(p.y, max.y, t.y) };
    // A small spawner: both edges close – take the closer one.
    if e.x_min && e.x_max {
        let left_closer = (p.x - min.x).abs() < (p.x - max.x).abs();
        (e.x_min, e.x_max) = (left_closer, !left_closer);
    }
    if e.y_min && e.y_max {
        let top_closer = (p.y - min.y).abs() < (p.y - max.y).abs();
        (e.y_min, e.y_max) = (top_closer, !top_closer);
    }
    if e.x_min || e.x_max || e.y_min || e.y_max {
        Some(e)
    } else if p.x > min.x && p.y > min.y && p.x < max.x && p.y < max.y {
        Some(Edges::ALL)
    } else {
        None
    }
}

/// The tile a world point lies on.
fn tile_of(p: Vec2) -> (i32, i32) {
    (p.x.floor() as i32, p.y.floor() as i32)
}

/// The middle of a group of units (by their `id`) – `None` if none of them is alive anymore.
fn center_of(world: &World, units: &[u32], buildings: &[u32]) -> Option<Vec2> {
    let units = units.iter().filter_map(|&id| world.find_unit(id)).map(|i| world.units[i].pos);
    let buildings = buildings.iter().filter_map(|&id| world.find_building(id)).map(|b| world.buildings[b].center());
    let positions: Vec<Vec2> = units.chain(buildings).collect();
    if positions.is_empty() {
        return None;
    }
    let sum: Vec2 = positions.iter().sum();
    Some(sum / positions.len() as f32)
}

/// The unit whose foot point is closest to the mouse (within a small radius).
fn unit_under_mouse(world: &World, camera: &Camera, mouse: Vec2) -> Option<usize> {
    let mut best: Option<(f32, usize)> = None;
    for (i, unit) in world.units.iter().enumerate().filter(|(_, u)| !u.is_inside()) {
        // The figure stands above its foot point, so check a bit higher.
        let body = camera.world_to_screen(unit.pos) - vec2(0.0, 12.0 * camera.zoom);
        let d = body.distance(mouse);
        if d < 20.0 * camera.zoom.max(0.5) && best.map_or(true, |(best_d, _)| d < best_d) {
            best = Some((d, i));
        }
    }
    best.map(|(_, i)| i)
}

/// The zombie under the mouse – its `id`, not its index in the list. Only zombies for which `visible` says yes
/// count (in the game view you can't see, and so can't click, the ones in the fog or outside the world border).
fn zombie_under_mouse(world: &World, camera: &Camera, mouse: Vec2, visible: impl Fn(Vec2) -> bool) -> Option<u32> {
    let mut best: Option<(f32, u32)> = None;
    for zombie in world.zombies.iter().filter(|z| visible(z.pos)) {
        let body = camera.world_to_screen(zombie.pos) - vec2(0.0, 11.0 * camera.zoom);
        let d = body.distance(mouse);
        if d < 14.0 * camera.zoom.max(0.5) && best.map_or(true, |(best_d, _)| d < best_d) {
            best = Some((d, zombie.id));
        }
    }
    best.map(|(_, id)| id)
}

/// The building under the mouse. Buildings are drawn tall, so don't only check the ground
/// but also a bit above it (walls and roof).
fn building_under_mouse(world: &World, camera: &Camera, mouse: Vec2) -> Option<usize> {
    for step in 0..=4 {
        let lift = step as f32 * 0.25 * TILE_W * camera.zoom;
        let ground = camera.screen_to_world(mouse + vec2(0.0, lift));
        if let Some(b) = world.building_at_point(ground) {
            let height = world.buildings[b].stats().height * TILE_W * camera.zoom;
            if lift <= height + 1.0 {
                return Some(b);
            }
        }
    }
    None
}

/// The 2 × 2 tiles of an oil node whose top corner is `tile` – the editor's oil stamp.
fn oil_node(tile: (i32, i32)) -> Vec<(i32, i32)> {
    vec![tile, (tile.0 + 1, tile.1), (tile.0, tile.1 + 1), (tile.0 + 1, tile.1 + 1)]
}
