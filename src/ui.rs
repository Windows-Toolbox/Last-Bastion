//! Input: mouse and keyboard → selection, camera and orders – like in StarCraft 2 (DESIGN.md 7.1, 11.2).
//! The simulation is never changed directly here – only through the order list (DESIGN.md 12.8).
//! Selection and control groups remember the units' `id`, because units can die.

use macroquad::prelude::*;

use crate::cursor::Cursor;
use crate::draw::{draw_diamond, mark_color, terrain_color, SpawnerShape, ZOMBIE};
use crate::editor::{Editor, Sight, Tool};
use crate::formation::{self, Formation, Shape, MIN_DRAG};
use crate::hud::{Button, HudHit, Layout, COMMAND_CENTER_BUTTONS, TOWER_BUTTONS, UNIT_BUTTONS};
use crate::iso::{Camera, TILE_H, TILE_W};
use crate::kinds::{BuildingKind, UnitKind};
use crate::map::{Region, Terrain};
use crate::placement::{grid_line, placement_line, placement_tile};
use crate::settings::Settings;
use crate::sim::{Command, Mark, Order, Rally, Spawner, World};

/// This close to the window edge (pixels) the mouse moves the camera.
const EDGE: f32 = 8.0;
/// The same key or unit twice within this time (seconds) counts as a double press.
const DOUBLE_TAP: f64 = 0.4;
/// The keys of the control groups, in keyboard order: 1 … 9, then 0.
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
/// Camera with WASD – but only as long as the selection doesn't need the key itself.
const WASD: [(KeyCode, Vec2); 4] = [
    (KeyCode::W, Vec2::new(0.0, -1.0)),
    (KeyCode::A, Vec2::new(-1.0, 0.0)),
    (KeyCode::S, Vec2::new(0.0, 1.0)),
    (KeyCode::D, Vec2::new(1.0, 0.0)),
];
/// Camera bookmarks like in StarCraft 2: Ctrl + F5–F8 to set, F5–F8 to jump.
const BOOKMARK_KEYS: [KeyCode; 4] = [KeyCode::F5, KeyCode::F6, KeyCode::F7, KeyCode::F8];
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

/// A spawner being dragged in the editor (its edge or the whole spawner).
#[derive(Clone, Copy)]
struct SpawnerDrag {
    spawner: u32,
    edges: Edges,
    /// Where the mouse went down (world), and where the spawner lay at that moment.
    start: Vec2,
    min: (i32, i32),
    size: (i32, i32),
}

impl SpawnerDrag {
    /// Where the spawner lies with the mouse at `point`: the grabbed edges snap to the nearest tile border.
    /// It never gets smaller than 1 × 1 – an edge can't be pulled across the opposite one.
    fn rect_at(&self, point: Vec2) -> ((i32, i32), (i32, i32)) {
        let (x0, y0) = self.min;
        let (x1, y1) = (x0 + self.size.0, y0 + self.size.1);
        if self.edges == Edges::ALL {
            let d = (point - self.start).round();
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

pub struct Ui {
    /// The selected units (their `id`).
    pub selected: Vec<u32>,
    /// Or one selected building (its `id`) – never both at once.
    pub selected_building: Option<u32>,
    /// The Tab subgroup: which type of the selection is "in front" right now.
    pub active_kind: Option<UnitKind>,
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
    /// After B, V, G or T: a building hangs on the mouse and waits for its spot. It stays there after building –
    /// until right click or Esc.
    pub placing: Option<BuildingKind>,
    /// The building on the mouse is turned by 90° (R – only the gate is not square).
    turned: bool,
    /// The formation units line up in when they're sent somewhere (buttons on the command card).
    pub formation: Shape,
    /// A formation being drawn with the mouse.
    form_drag: Option<FormDrag>,
    /// A spawner being dragged in the editor.
    spawner_drag: Option<SpawnerDrag>,
    /// While building with the left mouse button down: the tile where it started. On release the whole
    /// line from there to the mouse is built.
    place_start: Option<(i32, i32)>,
    /// While the zombie brush is held on the map: where it is (world). Every tick zombies go there.
    brush_at: Option<Vec2>,
    /// While painting terrain with the left mouse button down: the tile painted last. If the mouse moves
    /// quickly, the line in between is painted too, so the stroke has no holes.
    paint_last: Option<(i32, i32)>,
    /// Control groups: index 0 belongs to key 1, …, index 9 to key 0.
    pub groups: [Vec<u32>; 10],
    /// Which group key was pressed last, and when – for the double press.
    last_group: Option<(usize, f64)>,
    /// Which unit was clicked last, and when – for the double click.
    last_click: Option<(u32, f64)>,
    bookmarks: [Option<Vec2>; 4],
    /// Where the left mouse button was pressed on the map (for the selection box).
    drag_start: Option<Vec2>,
    /// Left mouse button pressed and held on the minimap: the camera follows the mouse.
    minimap_drag: bool,
    /// Marker after a command: world position, seconds left, kind (for the colour).
    marker: Option<(Vec2, f32, Mark)>,
}

impl Ui {
    pub fn new(settings: Settings) -> Ui {
        Ui {
            selected: Vec::new(),
            selected_building: None,
            active_kind: None,
            paused: false,
            speed: 1,
            settings,
            editor: Editor::new(),
            cursor: Cursor::new(),
            real_mouse: None,
            hover_unit: None,
            hover_zombie: None,
            hover_building: None,
            pan_drag: None,
            targeting: None,
            placing: None,
            turned: false,
            formation: Shape::Line,
            form_drag: None,
            spawner_drag: None,
            place_start: None,
            brush_at: None,
            paint_last: None,
            groups: Default::default(),
            last_group: None,
            last_click: None,
            bookmarks: [None; 4],
            drag_start: None,
            minimap_drag: false,
            marker: None,
        }
    }

    /// The selection as indices into `world.units`, sorted by type (like the panel at the bottom).
    pub fn selection_view(&self, world: &World) -> Vec<usize> {
        let mut view: Vec<usize> = self.selected.iter().filter_map(|&id| world.find_unit(id)).collect();
        view.sort_by_key(|&i| (world.units[i].kind, world.units[i].id));
        view
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

    /// Once per tick (from the tick loop in `main`, so "per tick" really means per tick – and nothing while paused):
    /// the zombies of the brush, if it's held on the map.
    pub fn brush_order(&self) -> Option<Order> {
        let at = self.brush_at.filter(|_| self.editor.tool == Some(Tool::ZombieBrush))?;
        Some(self.editor.zombie_order(at, self.editor.rate))
    }

    /// Can the building on the mouse go here? In the game by the rules (`can_place`), in editor mode almost anywhere.
    fn fits(&self, world: &World, kind: BuildingKind, tile: (i32, i32)) -> bool {
        if self.editor.editing() {
            world.can_place_anywhere(kind, tile, self.turned)
        } else {
            world.can_place(kind, tile, self.turned)
        }
    }

    /// Can this zombie be seen (drawn, clicked)? In the player's view only where your units look.
    pub fn shows_zombie(&self, world: &World, p: Vec2) -> bool {
        self.view_area(world).contains(p) && (self.sight() != Sight::Player || world.vision.sees(p))
    }

    /// Is a command waiting for its target, a building for its spot, or is an editor tool in hand?
    /// Then right click and Esc cancel that.
    fn is_waiting(&self) -> bool {
        self.targeting.is_some() || self.placing.is_some() || self.editor.tool.is_some()
    }

    /// Does Esc have something to do here – cancel, put away, deselect the spawner, stop typing?
    /// Then it doesn't open the menu.
    pub fn is_busy(&self) -> bool {
        self.is_waiting() || self.form_drag.is_some() || self.editor.spawner.is_some() || self.editor.is_typing()
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
                let (min, size) = match self.spawner_drag {
                    Some(drag) if drag.spawner == s.id => drag.rect_at(point),
                    _ => (s.min, s.size),
                };
                let name = match &self.editor.name_edit {
                    Some(text) if selected => text.clone(),
                    _ => s.name.clone(),
                };
                SpawnerShape { min: tile_corner(min), size: tile_corner(size), name, selected }
            })
            .collect()
    }

    /// Which buttons the command card shows right now – and which keys are taken by them.
    pub fn card(&self, world: &World) -> Vec<Button> {
        if let Some(b) = self.selected_building.and_then(|id| world.find_building(id)) {
            return match world.buildings[b].kind {
                BuildingKind::CommandCenter => COMMAND_CENTER_BUTTONS.to_vec(),
                BuildingKind::Tower => TOWER_BUTTONS.to_vec(),
                BuildingKind::Tent | BuildingKind::Wall | BuildingKind::Gate => Vec::new(),
            };
        }
        if self.selected.is_empty() { Vec::new() } else { UNIT_BUTTONS.to_vec() }
    }

    /// Selecting units means: no building selected anymore (and vice versa).
    fn select_units(&mut self, ids: Vec<u32>) {
        self.selected = ids;
        self.selected_building = None;
        self.placing = None;
    }

    /// Every frame, even with the menu open: query the real mouse, confine or release it.
    pub fn update_cursor(&mut self) {
        self.real_mouse = self.cursor.update(self.settings.confine_mouse);
    }

    pub fn handle_input(&mut self, world: &World, camera: &mut Camera, orders: &mut Vec<Order>, dt: f32) {
        self.forget_dead(world);
        self.editor.sync(world, self.placing);
        let layout = Layout::new();
        let mouse = Vec2::from(mouse_position());
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);

        // While a name is being typed (spawner, map), the keyboard belongs to the name field: no hotkeys, no WASD.
        let typing = self.editor.is_typing();
        if typing {
            self.editor.type_text(orders);
        } else if is_key_pressed(KeyCode::Escape) {
            // Esc: first put the tool away, then deselect the spawner.
            if self.editor.tool.is_some() {
                self.editor.tool = None;
            } else {
                self.editor.select_spawner(None, orders);
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
        if !typing {
            self.command_keys(orders, &card, shift);
            self.selection_keys(world, camera, shift, ctrl);
            self.other_keys();
        }
        self.move_camera(world, camera, mouse, &card, dt, !typing);
        self.fix_active_kind(world);
        self.update_hover(world, camera, &layout, mouse);
        // Whatever moved the camera (minimap, bookmark, group, mode switch): it stops at the edge of the view.
        camera.keep_inside(self.view_area(world));

        if let Some((_, time_left, _)) = &mut self.marker {
            *time_left -= dt;
            if *time_left <= 0.0 {
                self.marker = None;
            }
        }
    }

    /// Strike dead units from the selection and the control groups. Units in a tower can't be selected (like in
    /// a StarCraft bunker) – they stay in their control groups, though, and are back after unloading.
    fn forget_dead(&mut self, world: &World) {
        self.selected.retain(|&id| world.find_unit(id).is_some_and(|i| !world.units[i].is_inside()));
        for group in &mut self.groups {
            group.retain(|&id| world.find_unit(id).is_some());
        }
        if self.selected_building.is_some_and(|id| world.find_building(id).is_none()) {
            self.selected_building = None;
            self.targeting = None;
            self.placing = None;
        }
        // A removed spawner – but not one that has just been placed and isn't in the world yet.
        if let Some(id) = self.editor.spawner {
            if id < world.next_spawner_id() && world.find_spawner(id).is_none() {
                self.editor.spawner = None;
                self.editor.name_edit = None;
                self.spawner_drag = None;
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
        let building = self.selected_building.and_then(|id| world.find_building(id));
        let queue = building.map_or(0, |b| world.buildings[b].production.len());
        // The editor panel lies on top of the map: there it counts like the bottom panel (not the map).
        let on_editor = self.editor.contains(mouse);
        let hit = if on_editor {
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
            if !on_editor {
                self.editor.finish_name(orders); // a click outside the name field ends typing
            }
            if on_editor {
                self.editor.click(mouse, world, orders);
                if self.editor.tool.is_some() {
                    self.targeting = None;
                    self.placing = None;
                }
                // A building button of the editor: hang the building on the mouse (or take it off again).
                if let Some(place) = self.editor.place.take() {
                    self.targeting = None;
                    self.placing = place;
                    self.turned = false;
                    self.selected_building = None;
                }
            } else if let (Some(tool), None) = (self.editor.tool, hit) {
                self.use_tool(tool, world, orders, point);
            } else if let (Some(kind), None) = (self.placing, hit) {
                // Building on the mouse: the line starts here. It's built on release.
                self.place_start = Some(placement_tile(kind, self.turned, point));
            } else if let (Some(Button::Rally), None | Some(HudHit::Minimap(_))) = (self.targeting, hit) {
                self.set_rally(world, camera, orders, mouse, point, hit.is_none());
                self.targeting = None;
            } else if let (Some(button), None | Some(HudHit::Minimap(_))) = (self.targeting, hit) {
                // If a command (T, P, M) waits for its target, the click gives it – on the map or on the minimap.
                // On a zombie or a unit right away; on the ground it waits for the release: dragged = formation.
                let on_field = hit.is_none();
                match self.aimed_at(button, world, camera, mouse).filter(|_| on_field) {
                    Some(command) => {
                        self.give(orders, command, shift, point, None);
                        // With Shift the mode stays – that's how you set several waypoints in a row.
                        if !shift {
                            self.targeting = None;
                        }
                    }
                    None if on_field => self.form_drag = Some(FormDrag { from: point, button, mouse: MouseButton::Left }),
                    None => {
                        // On the minimap: a click, so no formation – everyone to the point.
                        self.give(orders, point_command(button, point), shift, point, None);
                        if !shift {
                            self.targeting = None;
                        }
                    }
                }
            } else {
                match hit {
                    // In the editor a click can grab a spawner – otherwise it starts the selection box.
                    None if self.editor.editing() && self.spawner_press(world, camera, point, orders) => {}
                    None => self.drag_start = Some(mouse),
                    Some(HudHit::Minimap(p)) => {
                        camera.center = p;
                        self.minimap_drag = true;
                    }
                    Some(HudHit::Group(g)) => self.group_action(g, world, camera, shift, ctrl),
                    Some(HudHit::Icon(k)) => self.click_icon(world, view[k], shift, ctrl),
                    Some(HudHit::Queue(index)) => {
                        if let Some(id) = self.selected_building {
                            orders.push(Order::CancelTrain { building: id, index });
                        }
                    }
                    Some(HudHit::Button(button)) => self.press(button, orders, shift),
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
            // Painting terrain: every frame the mouse is on another tile, paint the way there.
            if let (Some(Tool::Paint(terrain)), Some(last), None) = (self.editor.tool, self.paint_last, hit) {
                let now = tile_of(point);
                if now != last {
                    self.paint(terrain, last, now, orders);
                }
            }
            // The zombie brush follows the mouse – but only while it's held on the map.
            if self.brush_at.is_some() {
                self.brush_at = if hit.is_none() && self.editor.tool == Some(Tool::ZombieBrush) { Some(point) } else { None };
            }
        } else {
            self.brush_at = None;
        }
        if is_mouse_button_released(MouseButton::Left) {
            self.minimap_drag = false;
            self.paint_last = None;
            if let (Some(start), Some(kind)) = (self.place_start.take(), self.placing) {
                // Build the whole line – every piece that has room. The building stays on the mouse for the next
                // line, until right click or Esc.
                // In editor mode without the rules of the game.
                let end = placement_tile(kind, self.turned, camera.screen_to_world(mouse));
                let (turned, editor) = (self.turned, self.editor.editing());
                for tile in placement_line(kind, start, end) {
                    if self.fits(world, kind, tile) {
                        orders.push(Order::PlaceBuilding { kind, tile, turned, editor });
                    }
                }
            }
            if let Some(start) = self.drag_start.take() {
                self.select(world, camera, layout, start, mouse, shift, ctrl);
            }
            // A spawner let go: move it there for real (one order, not one per frame).
            if let Some(drag) = self.spawner_drag.take() {
                let (min, size) = drag.rect_at(point);
                if (min, size) != (drag.min, drag.size) {
                    orders.push(Order::ResizeSpawner { spawner: drag.spawner, min, size });
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
            // Building selected: right click sets the rally point (like in StarCraft).
            if self.selected_building.is_some() {
                if let None | Some(HudHit::Minimap(_)) = hit {
                    self.set_rally(world, camera, orders, mouse, point, hit.is_none());
                }
                return;
            }
            match hit {
                // Right click on the map: the "smart" command, depending on what is under the mouse. On the ground
                // it waits for the release – dragged, the units line up along the line (formation).
                None => {
                    let tower = building_under_mouse(world, camera, mouse).filter(|&b| world.buildings[b].kind == BuildingKind::Tower);
                    if let Some(zombie) = self.zombie_at(world, camera, mouse) {
                        self.give(orders, Command::Attack(zombie), shift, point, None);
                    } else if let Some(i) = unit_under_mouse(world, camera, mouse) {
                        self.give(orders, Command::Follow(world.units[i].id), shift, point, None);
                    } else if let Some(b) = tower {
                        // On a tower: go in (like loading a StarCraft bunker).
                        self.give(orders, Command::Enter(world.buildings[b].id), shift, point, None);
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
        if drag.mouse == MouseButton::Left && !shift {
            self.targeting = None;
        }
    }

    /// Left click in the editor (no tool): grab the edge of the selected spawner, or the inside of a spawner to
    /// move it. Returns whether a spawner was hit – if not, the selection box starts as usual.
    fn spawner_press(&mut self, world: &World, camera: &Camera, point: Vec2, orders: &mut Vec<Order>) -> bool {
        let tolerance = (GRAB_PIXELS / (TILE_H * camera.zoom)).clamp(0.2, 1.5);
        let selected = self.editor.spawner.and_then(|id| world.find_spawner(id));
        let grabbed = match selected.and_then(|s| grab(&world.spawners[s], point, tolerance).map(|e| (s, e))) {
            Some(hit) => Some(hit),
            // Another spawner (the one on top, i.e. placed last), unless there's a unit under the mouse.
            None if unit_under_mouse(world, camera, camera.world_to_screen(point)).is_none() => {
                world.spawners.iter().rposition(|s| s.contains(point)).map(|s| (s, Edges::ALL))
            }
            None => None,
        };
        let Some((s, edges)) = grabbed else {
            self.editor.select_spawner(None, orders);
            return false;
        };
        let spawner = &world.spawners[s];
        self.editor.select_spawner(Some(spawner.id), orders);
        self.spawner_drag = Some(SpawnerDrag { spawner: spawner.id, edges, start: point, min: spawner.min, size: spawner.size });
        true
    }

    /// Left click on the map with an editor tool in hand.
    fn use_tool(&mut self, tool: Tool, world: &World, orders: &mut Vec<Order>, point: Vec2) {
        match tool {
            Tool::Paint(terrain) => {
                let tile = tile_of(point);
                self.paint(terrain, tile, tile, orders);
            }
            Tool::ZombieDrop => orders.push(self.editor.zombie_order(point, self.editor.drop)),
            // The brush places its zombies tick by tick while the button is held (`brush_order`).
            Tool::ZombieBrush => self.brush_at = Some(point),
            Tool::Unit(kind) => orders.push(Order::SpawnUnit { kind, point }),
            Tool::Spawner => {
                // Select the new spawner right away, so its edge can be dragged. Which number it will get:
                // the next free one – plus the spawners placed while the game is paused (orders still waiting).
                let waiting = orders.iter().filter(|o| matches!(o, Order::PlaceSpawner { .. })).count() as u32;
                orders.push(Order::PlaceSpawner { center: tile_of(point) });
                self.editor.select_spawner(Some(world.next_spawner_id() + waiting), orders);
                self.editor.tool = None;
            }
            // Never in the editor's hand – it goes onto the mouse as `placing` right away.
            Tool::Building(_) => {}
        }
    }

    /// Paints with the brush along the line from `from` to `to` – one order for the whole stroke.
    fn paint(&mut self, terrain: Terrain, from: (i32, i32), to: (i32, i32), orders: &mut Vec<Order>) {
        let mut tiles: Vec<(i32, i32)> = grid_line(from, to).into_iter().flat_map(|t| self.editor.brush_tiles(t)).collect();
        tiles.sort();
        tiles.dedup(); // neighbouring brush squares overlap
        orders.push(Order::PaintTerrain { terrain, tiles });
        self.paint_last = Some(to);
    }

    /// Rally point of the selected building: on a unit (follow) or on a point.
    fn set_rally(&mut self, world: &World, camera: &Camera, orders: &mut Vec<Order>, mouse: Vec2, point: Vec2, on_field: bool) {
        let (Some(building), Some(b)) = (self.selected_building, self.selected_building.and_then(|id| world.find_building(id)))
        else {
            return;
        };
        if !matches!(world.buildings[b].kind, BuildingKind::CommandCenter | BuildingKind::Tower) {
            return;
        }
        let rally = match unit_under_mouse(world, camera, mouse).filter(|_| on_field) {
            Some(i) => Rally::Unit(world.units[i].id),
            None => Rally::Point(point),
        };
        orders.push(Order::SetRally { building, rally });
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
    fn press(&mut self, button: Button, orders: &mut Vec<Order>, shift: bool) {
        if let Some(shape) = button.formation() {
            self.formation = shape;
            return;
        }
        self.targeting = None;
        self.placing = None;
        self.editor.tool = None;
        let building = self.selected_building;
        match (button, building) {
            (Button::Stop, _) => orders.push(Order::Stop { units: self.selected.clone() }),
            (Button::Hold, _) => {
                let units = self.selected.clone();
                orders.push(Order::Command { units, command: Command::Hold, queue: shift, formation: None });
            }
            (Button::Move | Button::Patrol | Button::Attack | Button::Rally, _) => self.targeting = Some(button),
            (Button::PlaceTent | Button::PlaceWall | Button::PlaceGate | Button::PlaceTower, _) => {
                self.placing = button.places();
            }
            (Button::Unload, Some(building)) => orders.push(Order::Unload { building }),
            (Button::Unload, None) => {}
            // With Shift 5 at once into the queue, like in StarCraft.
            (Button::TrainSoldier | Button::TrainSniper | Button::TrainMedic, Some(building)) => {
                let Some(kind) = button.trains() else {
                    return;
                };
                for _ in 0..if shift { 5 } else { 1 } {
                    orders.push(Order::Train { building, kind });
                }
            }
            (Button::TrainSoldier | Button::TrainSniper | Button::TrainMedic, None) => {}
            (Button::FormLine | Button::FormBox | Button::FormCircle, _) => {} // handled above
        }
    }

    /// Keys of the command card. Only the buttons that are visible right now react.
    fn command_keys(&mut self, orders: &mut Vec<Order>, card: &[Button], shift: bool) {
        if is_key_pressed(KeyCode::Escape) || card.is_empty() {
            self.targeting = None;
            self.placing = None;
            return;
        }
        // R turns the gate on the mouse (like in Factorio) – while placing it, R isn't "rally".
        if self.placing == Some(BuildingKind::Gate) && is_key_pressed(KeyCode::R) {
            self.turned = !self.turned;
            return;
        }
        for &button in card {
            if is_key_pressed(button.key()) {
                self.press(button, orders, shift);
            }
        }
    }

    /// Click on an icon in the selection panel (like in StarCraft):
    /// only this unit / Shift: this one out / Ctrl: only this type / Ctrl+Shift: this type out.
    fn click_icon(&mut self, world: &World, i: usize, shift: bool, ctrl: bool) {
        let (id, kind) = (world.units[i].id, world.units[i].kind);
        let is_kind = |other: u32| world.find_unit(other).is_some_and(|j| world.units[j].kind == kind);
        match (shift, ctrl) {
            (false, false) => self.select_units(vec![id]),
            (true, false) => self.selected.retain(|&other| other != id),
            (false, true) => self.selected.retain(|&other| is_kind(other)),
            (true, true) => self.selected.retain(|&other| !is_kind(other)),
        }
    }

    /// A click (barely moved) selects one unit, dragging selects everyone in the box.
    /// Shift: add, or remove again by clicking. Ctrl-click or double click: all of this type on screen.
    /// Like in StarCraft the old selection stays if the click or box hits nothing.
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
            self.add_or_replace(found, shift);
            return;
        }

        let Some(i) = unit_under_mouse(world, camera, end) else {
            // No unit, but a building: select that (buildings always one at a time).
            if let Some(b) = building_under_mouse(world, camera, end) {
                self.selected.clear();
                self.selected_building = Some(world.buildings[b].id);
                self.targeting = None;
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
            self.selected_building = None;
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
        self.selected_building = None;
        if !shift {
            self.selected.clear();
        }
        for id in found {
            if !self.selected.contains(&id) {
                self.selected.push(id);
            }
        }
    }

    /// Control groups, F2, Tab, camera bookmarks, Backspace.
    fn selection_keys(&mut self, world: &World, camera: &mut Camera, shift: bool, ctrl: bool) {
        for (g, &key) in GROUP_KEYS.iter().enumerate() {
            if is_key_pressed(key) {
                self.group_action(g, world, camera, shift, ctrl);
            }
        }

        // F2: the whole army (like in StarCraft 2).
        if is_key_pressed(KeyCode::F2) {
            self.select_units(world.units.iter().filter(|u| !u.is_inside()).map(|unit| unit.id).collect());
        }

        // Tab: to the next type in the selection, Shift+Tab: to the previous one.
        if is_key_pressed(KeyCode::Tab) {
            let kinds = self.kinds_in_selection(world);
            if let Some(pos) = kinds.iter().position(|&k| Some(k) == self.active_kind) {
                let next = (if shift { pos + kinds.len() - 1 } else { pos + 1 }) % kinds.len();
                self.active_kind = Some(kinds[next]);
            }
        }

        for (b, &key) in BOOKMARK_KEYS.iter().enumerate() {
            if is_key_pressed(key) {
                if ctrl {
                    self.bookmarks[b] = Some(camera.center);
                } else if let Some(center) = self.bookmarks[b] {
                    camera.center = center;
                }
            }
        }
        // Backspace: camera to the base (like StarCraft's jump to the main building).
        if is_key_pressed(KeyCode::Backspace) {
            camera.center = world.base;
        }
    }

    /// A control group – by number key or by click on the panel, with the same modifier keys.
    fn group_action(&mut self, g: usize, world: &World, camera: &mut Camera, shift: bool, ctrl: bool) {
        // Groups only hold units: with a building in the selection there's nothing to save.
        if (ctrl || shift) && self.selected.is_empty() {
            return;
        }
        match (ctrl, shift) {
            // Ctrl: set the group anew.
            (true, false) => self.groups[g] = self.selected.clone(),
            // Ctrl+Shift: set anew and take the units out of all other groups ("steal").
            (true, true) => {
                for (other, group) in self.groups.iter_mut().enumerate() {
                    if other != g {
                        group.retain(|id| !self.selected.contains(id));
                    }
                }
                self.groups[g] = self.selected.clone();
            }
            // Shift: add the selection to the group.
            (false, true) => {
                for &id in &self.selected {
                    if !self.groups[g].contains(&id) {
                        self.groups[g].push(id);
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
        let group: Vec<u32> =
            self.groups[g].iter().copied().filter(|&id| world.find_unit(id).is_some_and(|i| !world.units[i].is_inside())).collect();
        self.select_units(group);
        if double {
            if let Some(center) = center_of(world, &self.selected) {
                camera.center = center;
            }
        }
        self.last_group = Some((g, now));
    }

    /// The types in the selection, without duplicates, in a fixed order.
    fn kinds_in_selection(&self, world: &World) -> Vec<UnitKind> {
        let mut kinds: Vec<UnitKind> = self.selection_view(world).iter().map(|&i| world.units[i].kind).collect();
        kinds.dedup(); // works because `selection_view` is sorted by type
        kinds
    }

    /// If the active type isn't in the selection anymore, the first type becomes active.
    fn fix_active_kind(&mut self, world: &World) {
        let kinds = self.kinds_in_selection(world);
        if !self.active_kind.is_some_and(|k| kinds.contains(&k)) {
            self.active_kind = kinds.first().copied();
        }
    }

    /// Time, overlay, editor. The developer tools are buttons in the editor now (F10) –
    /// so the keys Insert, Home, Delete, End, Page Up/Down, F4 and F11 are free again.
    fn other_keys(&mut self) {
        if is_key_pressed(KeyCode::Space) {
            self.paused = !self.paused;
        }
        // macroquad reports keys by their position on a US keyboard. On a German keyboard
        // "+" is where US has "]", and "-" is where US has "/" – so check both.
        if any_pressed(&[KeyCode::KpAdd, KeyCode::RightBracket, KeyCode::Equal]) {
            self.speed = (self.speed * 2).min(8);
        }
        if any_pressed(&[KeyCode::KpSubtract, KeyCode::Slash, KeyCode::Minus]) {
            self.speed = (self.speed / 2).max(1);
        }
        if is_key_pressed(KeyCode::F3) {
            self.settings.dev_overlay = !self.settings.dev_overlay;
        }
        // Game view ↔ editor mode (the two buttons in the top right).
        if is_key_pressed(KeyCode::F10) {
            self.editor.toggle();
        }
        // Emergency exit if the confined mouse gets in the way (the setting is also in the menu).
        if is_key_pressed(KeyCode::F12) {
            self.settings.confine_mouse = !self.settings.confine_mouse;
        }
    }

    /// Camera: arrow keys, mouse at the window edge, mouse wheel. `keys`: may the keyboard move it (not while
    /// a name is being typed)?
    fn move_camera(&mut self, world: &World, camera: &mut Camera, mouse: Vec2, card: &[Button], dt: f32, keys: bool) {
        let mut pan = Vec2::ZERO;
        // WASD – unless the selection needs the key itself (e.g. S = stop for units).
        // Ctrl held: not either (Ctrl+S, Ctrl+A … should never move the camera).
        // A building selected: not at all – its keys (Q, W, E, R …) sit right next to them.
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
        let building = self.selected_building.is_some();
        for (key, dir) in WASD {
            if keys && is_key_down(key) && !ctrl && !building && !card.iter().any(|b| b.key() == key) {
                pan += dir;
            }
        }
        let arrows = [(KeyCode::Left, vec2(-1.0, 0.0)), (KeyCode::Right, vec2(1.0, 0.0)), (KeyCode::Up, vec2(0.0, -1.0)), (KeyCode::Down, vec2(0.0, 1.0))];
        for (key, dir) in arrows {
            if keys && is_key_down(key) {
                pan += dir;
            }
        }
        // At the edge only the real mouse counts: if the window has no focus or the mouse is outside
        // (e.g. on the second monitor), nothing scrolls. Also not while a selection box is being dragged
        // (it hangs on the screen, not on the world) or the camera is moved another way.
        let busy = self.drag_start.is_some() || self.minimap_drag || self.pan_drag.is_some() || self.spawner_drag.is_some();
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
        // On the editor panel the wheel scrolls the panel, not the map.
        if wheel != 0.0 && self.editor.contains(mouse) {
            self.editor.scroll_by(wheel);
        } else if wheel > 0.0 {
            camera.zoom_by(1.15);
        } else if wheel < 0.0 {
            camera.zoom_by(1.0 / 1.15);
        }
    }

    /// What lies on the map under the mouse? Not if the mouse is on a panel or a tool is in hand.
    fn update_hover(&mut self, world: &World, camera: &Camera, layout: &Layout, mouse: Vec2) {
        self.hover_unit = None;
        self.hover_zombie = None;
        self.hover_building = None;
        let busy = self.drag_start.is_some() || self.placing.is_some() || self.editor.tool.is_some();
        if !layout.in_game_view(mouse) || self.editor.contains(mouse) || busy {
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
        let ring = |pos: Vec2, color: Color| {
            let s = camera.world_to_screen(pos);
            let w = 0.8 * TILE_W * camera.zoom;
            draw_ellipse_lines(s.x, s.y, w, w / 2.0, 0.0, 1.5, color);
        };

        if let Some(start) = self.drag_start {
            let min = start.min(mouse);
            let size = (start - mouse).abs();
            draw_rectangle_lines(min.x, min.y, size.x, size.y, 1.5, GREEN);
            // Preview like in StarCraft: whoever stands in the box is already highlighted.
            let rect = Rect::new(min.x, min.y, size.x, size.y);
            for unit in &world.units {
                if rect.contains(camera.world_to_screen(unit.pos)) {
                    ring(unit.pos, WHITE);
                }
            }
        }
        // Under the mouse: zombie red (right click = attack), own unit white.
        if let Some(z) = self.hover_zombie.and_then(|id| world.zombies.iter().find(|z| z.id == id)) {
            ring(z.pos, RED);
        }
        if let Some(i) = self.hover_unit.and_then(|id| world.find_unit(id)) {
            ring(world.units[i].pos, WHITE);
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
        if let Some(b) = self.selected_building.and_then(|id| world.find_building(id)) {
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
            // The zone free of zombies only counts in the game – the editor builds anywhere.
            let corners = [min, vec2(max.x, min.y), max, vec2(min.x, max.y)];
            for k in (0..4).filter(|_| !self.editor.editing()) {
                let (p, q) = (camera.world_to_screen(corners[k]), camera.world_to_screen(corners[(k + 1) % 4]));
                draw_line(p.x, p.y, q.x, q.y, 1.5, frame);
            }
            let tiles = match self.place_start {
                Some(start) => placement_line(kind, start, here),
                None => vec![here],
            };
            let (w, h) = kind.footprint(turned);
            // What the pieces that have room cost together (only shown – nothing is paid yet).
            let mut cost = 0;
            for tile in tiles {
                let fits = self.fits(world, kind, tile);
                if fits {
                    cost += world.build_cost(kind, tile, turned);
                }
                let color = if fits {
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
            let mut text = format!("Cost {cost}");
            if on_walls > 0 && self.place_start.is_none() {
                text += &format!(" (on {on_walls} wall{})", if on_walls == 1 { "" } else { "s" });
            }
            if kind == BuildingKind::Gate {
                text += "   R: turn";
            }
            draw_text(&text, mouse.x + 14.0, mouse.y - 8.0, 20.0, WHITE);
        }
        // Editor tool on the mouse: the brush in the colour of the terrain, or the name of what gets spawned.
        if let Some(tool) = self.editor.tool.filter(|_| !self.editor.contains(mouse)) {
            match tool {
                Tool::Paint(terrain) => {
                    let color = match terrain {
                        Terrain::Grass => Color::new(1.0, 1.0, 1.0, 0.25),
                        _ => Color { a: 0.6, ..terrain_color(terrain) },
                    };
                    for (x, y) in self.editor.brush_tiles(tile_of(camera.screen_to_world(mouse))) {
                        draw_diamond(camera, vec2(x as f32, y as f32), Vec2::ONE, color);
                    }
                }
                Tool::ZombieBrush | Tool::ZombieDrop => {
                    // The area where they land: a circle on the ground – on screen a flat ellipse (like the tiles).
                    let radius = self.editor.zombie_radius(if tool == Tool::ZombieDrop { self.editor.drop } else { 1 });
                    let s = mouse;
                    let w = radius * TILE_W * std::f32::consts::FRAC_1_SQRT_2 * camera.zoom;
                    draw_ellipse_lines(s.x, s.y, w, w / 2.0, 0.0, 2.0, ZOMBIE);
                    let text = match tool {
                        Tool::ZombieBrush => format!("{} per tick", self.editor.rate),
                        _ => format!("{} zombies", self.editor.drop),
                    };
                    draw_text(&text, mouse.x + 14.0, mouse.y - 8.0, 20.0, ZOMBIE);
                }
                Tool::Unit(_) | Tool::Spawner => {
                    draw_circle_lines(mouse.x, mouse.y, 10.0, 2.0, YELLOW);
                    draw_text(tool.name(), mouse.x + 14.0, mouse.y - 8.0, 20.0, YELLOW);
                }
                Tool::Building(_) => {}
            }
        }
        // Formation being drawn: the dragged line and a ring at every spot – soldiers in front. Only once it's really
        // dragged: a click sends everyone to the point.
        let dragged = self.form_drag.filter(|drag| camera.screen_to_world(mouse).distance(drag.from) >= MIN_DRAG);
        if let Some(drag) = dragged {
            let to = camera.screen_to_world(mouse);
            let units: Vec<(Vec2, UnitKind)> =
                self.selection_view(world).iter().map(|&i| (world.units[i].pos, world.units[i].kind)).collect();
            let color = mark_color(button_mark(drag.button));
            let (a, b) = (camera.world_to_screen(drag.from), camera.world_to_screen(to));
            draw_line(a.x, a.y, b.x, b.y, 2.0, Color { a: 0.6, ..color });
            let formation = Formation { shape: self.formation, from: drag.from, to };
            let w = 0.6 * TILE_W * camera.zoom;
            for (spot, (_, kind)) in formation::spots(formation, &units).into_iter().zip(&units) {
                let s = camera.world_to_screen(spot);
                let ring = if *kind == UnitKind::Soldier { color } else { Color { a: 0.6, ..color } };
                draw_ellipse_lines(s.x, s.y, w, w / 2.0, 0.0, 1.5, ring);
            }
        }
        // Editor: the edge of the selected spawner under the mouse (or being dragged) lights up white.
        if let Some(s) = self.editor.spawner.and_then(|id| world.find_spawner(id)).filter(|_| self.editor.editing()) {
            let point = camera.screen_to_world(mouse);
            let tolerance = (GRAB_PIXELS / (TILE_H * camera.zoom)).clamp(0.2, 1.5);
            let edges = match self.spawner_drag {
                Some(drag) => Some(drag.edges),
                None if !self.editor.contains(mouse) && self.editor.tool.is_none() => grab(&world.spawners[s], point, tolerance),
                None => None,
            };
            if let Some(edges) = edges.filter(|&e| e != Edges::ALL) {
                let (min, size) = match self.spawner_drag {
                    Some(drag) => drag.rect_at(point),
                    None => (world.spawners[s].min, world.spawners[s].size),
                };
                let (min, max) = (tile_corner(min), tile_corner(min) + tile_corner(size));
                let line = |p: Vec2, q: Vec2| {
                    let (p, q) = (camera.world_to_screen(p), camera.world_to_screen(q));
                    draw_line(p.x, p.y, q.x, q.y, 4.0, WHITE);
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
    }
}

fn any_pressed(keys: &[KeyCode]) -> bool {
    keys.iter().any(|&key| is_key_pressed(key))
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

/// Which edges of the spawner lie within `tolerance` tiles of `point` – at a corner two. Inside, away from the
/// edges: all four (the whole spawner moves). `None` if the point lies outside.
fn grab(spawner: &Spawner, point: Vec2, tolerance: f32) -> Option<Edges> {
    let (min, size) = spawner.rect();
    let max = min + size;
    let t = tolerance;
    if point.x < min.x - t || point.y < min.y - t || point.x > max.x + t || point.y > max.y + t {
        return None;
    }
    let near = |a: f32, b: f32| (a - b).abs() < t;
    let mut e = Edges { x_min: near(point.x, min.x), x_max: near(point.x, max.x), y_min: near(point.y, min.y), y_max: near(point.y, max.y) };
    // A small spawner: both edges close – take the closer one.
    if e.x_min && e.x_max {
        let left_closer = (point.x - min.x).abs() < (point.x - max.x).abs();
        (e.x_min, e.x_max) = (left_closer, !left_closer);
    }
    if e.y_min && e.y_max {
        let top_closer = (point.y - min.y).abs() < (point.y - max.y).abs();
        (e.y_min, e.y_max) = (top_closer, !top_closer);
    }
    if e.x_min || e.x_max || e.y_min || e.y_max {
        Some(e)
    } else if spawner.contains(point) {
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
fn center_of(world: &World, ids: &[u32]) -> Option<Vec2> {
    let positions: Vec<Vec2> = ids.iter().filter_map(|&id| world.find_unit(id)).map(|i| world.units[i].pos).collect();
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

