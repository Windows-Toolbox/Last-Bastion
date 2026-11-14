//! The **Regions** module (`docs\editor.md`, step 4): named regions and points like SC2's (`sim\regions.rs`).
//!
//! **Left:** the tools "+ Rectangle", "+ Circle", "+ Paint", "+ Point", then everything there is to find again – the
//! playing field and the base (special: the map has them anyway), the regions, the points, and the spawners, which
//! are places with a name too.
//!
//! **On the map** (`ui.rs` reads the mouse and asks here): a tool dragged makes a new region; with **Ctrl** it adds
//! its shape to the selected region, with **Alt** it cuts it out. Without a tool a click selects a region or a point,
//! dragging moves it, and the edges of the selected region's rectangles can be dragged like a spawner's.
//!
//! **Right:** name, colour (only for the editor) and the shapes with their numbers; locate, duplicate, delete.
//!
//! Every change is an order (`AddZone`, `SetZone`, `PaintZone` …) – undo, redo and the map file come by themselves.
//! Who uses a region (triggers, events) is listed here once something can use one.

use egui_macroquad::egui::{self, Color32, DragValue, RichText, Sense, TextEdit, Ui};
use macroquad::color::{Color, WHITE};
use macroquad::math::{vec2, Vec2};
use macroquad::shapes::{draw_circle, draw_circle_lines, draw_line};
use macroquad::text::{draw_text, measure_text};

use super::{section, section_from, settled, settled_name, Editor, EditorRequest, Origin, Tool};
use crate::draw::draw_diamond;
use crate::iso::Camera;
use crate::sim::{Area, Form, Order, Shape, Spot, World, Zone, MOST_SHAPES, ZONE_COLORS};

/// The colours regions and points can have (`Zone::color` counts through them) – told apart by lightness too.
const COLORS: [(u8, u8, u8); ZONE_COLORS as usize] =
    [(224, 160, 58), (192, 80, 77), (106, 159, 216), (139, 192, 106), (176, 127, 208), (79, 184, 176), (216, 127, 168), (214, 214, 214)];
/// One rectangle of the selected region: which shape it is, its corner and its size.
pub type PickedRect = (usize, (i32, i32), (i32, i32));
/// A point is picked this close to the mouse (pixels on screen).
const POINT_GRAB: f32 = 12.0;

/// What's selected in the Regions module (its number).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Picked {
    Zone(u32),
    Spot(u32),
}

fn color32(index: u8) -> Color32 {
    let (r, g, b) = COLORS[(index % ZONE_COLORS) as usize];
    Color32::from_rgb(r, g, b)
}

/// The same colour for macroquad, which draws the map.
fn map_color(index: u8) -> Color {
    let (r, g, b) = COLORS[(index % ZONE_COLORS) as usize];
    Color::from_rgba(r, g, b, 255)
}

/// How a region is drawn: its tiles as runs along the rows (one see-through strip each instead of a diamond per
/// tile) and its edge as lines between tile corners. Worked out once and kept until the region changes (`zone` is
/// the region it was made from).
pub(super) struct Outline {
    zone: Zone,
    /// (row, first column, the column after the last).
    runs: Vec<(i32, i32, i32)>,
    edges: Vec<((i32, i32), (i32, i32))>,
    tiles: usize,
}

impl Outline {
    fn of(zone: &Zone) -> Outline {
        let mut outline = Outline { zone: zone.clone(), runs: Vec::new(), edges: Vec::new(), tiles: 0 };
        let Some(bounds) = zone.bounds() else {
            return outline;
        };
        let (w, h) = bounds.size();
        // One yes/no per tile of the bounds, row by row.
        let grid: Vec<bool> = bounds.tiles().map(|t| zone.contains(t)).collect();
        let inside = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && grid[(y * w + x) as usize];
        let (ox, oy) = bounds.min;
        for y in 0..=h {
            // `start`: where the run (or the piece of edge) going on right now began.
            let (mut run, mut edge): (Option<i32>, Option<i32>) = (None, None);
            for x in 0..=w {
                if inside(x, y) {
                    run.get_or_insert(x);
                    outline.tiles += 1;
                } else if let Some(start) = run.take() {
                    outline.runs.push((oy + y, ox + start, ox + x));
                }
                // The line above this row is edge wherever the tile above and the one below differ.
                if x < w && inside(x, y - 1) != inside(x, y) {
                    edge.get_or_insert(x);
                } else if let Some(start) = edge.take() {
                    outline.edges.push(((ox + start, oy + y), (ox + x, oy + y)));
                }
            }
        }
        // The same down the columns.
        for x in 0..=w {
            let mut edge: Option<i32> = None;
            for y in 0..=h {
                if y < h && inside(x - 1, y) != inside(x, y) {
                    edge.get_or_insert(y);
                } else if let Some(start) = edge.take() {
                    outline.edges.push(((ox + x, oy + start), (ox + x, oy + y)));
                }
            }
        }
        outline
    }
}

impl Editor {
    /// Are the regions drawn on the map? In their module always, elsewhere when "Show" says so.
    pub fn shows_regions(&self) -> bool {
        self.editing() && (self.module == super::Module::Regions || self.layers.regions)
    }

    /// Every frame: an outline for every region – the ones that didn't change keep theirs.
    pub(super) fn refresh_outlines(&mut self, world: &World) {
        // `take`: the old list out, an empty one in its place.
        let mut old = std::mem::take(&mut self.outlines);
        for zone in &world.zones {
            let outline = match old.iter().position(|o| o.zone == *zone) {
                Some(k) => old.swap_remove(k),
                None => Outline::of(zone),
            };
            self.outlines.push(outline);
        }
    }

    /// The left panel.
    pub(super) fn regions_module(&mut self, ui: &mut Ui, world: &World) {
        ui.horizontal_wrapped(|ui| {
            for tool in [Tool::ZoneRect, Tool::ZoneCircle, Tool::ZonePaint, Tool::Spot] {
                self.tool_button(ui, tool);
            }
        });
        ui.label(RichText::new("Drag on the map: a new region. Ctrl: add to the selected one, Alt: cut out of it.").small().weak());
        if self.tool == Some(Tool::ZonePaint) {
            ui.horizontal(|ui| {
                ui.label("Brush");
                ui.add(egui::Slider::new(&mut self.brush, 1..=41).suffix(" tiles"));
                self.brush |= 1;
            });
        }
        ui.add(TextEdit::singleline(&mut self.region_search).hint_text("Find region or point...").desired_width(f32::INFINITY));
        let search = self.region_search.trim().to_lowercase();
        let found = |name: &str| search.is_empty() || name.to_lowercase().contains(&search);
        ui.separator();
        section(ui, "Special", "", |ui| {
            let (_, size) = world.map.play_steps();
            ui.horizontal(|ui| {
                if ui.selectable_label(self.holds(Tool::PlayArea), "Playing field").on_hover_text("The world border - click, then drag its edges on the map").clicked() {
                    self.toggle_tool(Tool::PlayArea);
                }
                ui.label(RichText::new(format!("{} x {} steps", size.0, size.1)).weak());
            });
            ui.horizontal(|ui| {
                if ui.selectable_label(false, "Base (command center)").on_hover_text("Where the command center stands - click: the camera goes there").clicked() {
                    self.request = Some(EditorRequest::Locate(world.base));
                }
                ui.label(RichText::new("auto").weak());
            });
        });
        section_from(ui, "Regions", Some(Origin::Sc2), "", |ui| {
            if world.zones.is_empty() {
                ui.label(RichText::new("None yet - take a tool above and drag on the map.").weak());
            }
            for (k, zone) in world.zones.iter().enumerate().filter(|(_, z)| found(&z.name)) {
                let tiles = self.outlines.get(k).map_or(0, |o| o.tiles);
                let center = zone.center();
                self.list_row(ui, Picked::Zone(zone.id), zone.color, &zone.name, &format!("{tiles} tiles"), center);
            }
        });
        section_from(ui, "Points", Some(Origin::Sc2), "", |ui| {
            if world.spots.is_empty() {
                ui.label(RichText::new("None yet - \"+ Point\", then click on the map.").weak());
            }
            for spot in world.spots.iter().filter(|s| found(&s.name)) {
                self.list_row(ui, Picked::Spot(spot.id), spot.color, &spot.name, &format!("{}, {}", spot.tile.0, spot.tile.1), Some(spot.center()));
            }
        });
        section_from(ui, "Spawners", Some(Origin::New), "already there - where the zombies come from", |ui| {
            for spawner in world.spawners.iter().filter(|s| found(&s.name)) {
                let line = ui.selectable_label(self.spawner == Some(spawner.id), &spawner.name).on_hover_text("Click: select it - double click: the camera goes there");
                if line.clicked() {
                    self.picked = None;
                    self.select_spawner(Some(spawner.id));
                }
                if line.double_clicked() {
                    self.request = Some(EditorRequest::Locate(spawner.center()));
                }
            }
        });
    }

    /// A line of the lists: its colour, its name, something about it in grey. A click selects, a double click also
    /// moves the camera there.
    fn list_row(&mut self, ui: &mut Ui, what: Picked, color: u8, name: &str, about: &str, at: Option<Vec2>) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), Sense::hover());
            // A region is a square, a point a dot.
            let rounding = if matches!(what, Picked::Spot(_)) { 6.0 } else { 2.0 };
            ui.painter().rect_filled(rect, rounding, color32(color));
            let line = ui.selectable_label(self.picked == Some(what), name).on_hover_text("Click: select it - double click: the camera goes there");
            if line.clicked() {
                self.picked = Some(what);
                self.select_spawner(None);
            }
            if let (true, Some(at)) = (line.double_clicked(), at) {
                self.request = Some(EditorRequest::Locate(at));
            }
            ui.label(RichText::new(about).weak());
        });
    }

    /// The right panel: the selected region or point.
    pub(super) fn region_section(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        match self.picked {
            Some(Picked::Zone(id)) => match world.find_zone(id) {
                Some(z) => self.zone_fields(ui, world, &world.zones[z], orders),
                // Gone (undo, Del): nothing selected any more.
                None => self.picked = None,
            },
            Some(Picked::Spot(id)) => match world.find_spot(id) {
                Some(s) => self.spot_fields(ui, world, &world.spots[s], orders),
                None => self.picked = None,
            },
            None => {}
        }
        if self.picked.is_none() && self.spawner.is_none() {
            section(ui, "Selection", "", |ui| {
                ui.label(RichText::new("Nothing selected. Click a region or a point - on the map or in the list on the left.").weak());
            });
        }
    }

    fn zone_fields(&mut self, ui: &mut Ui, world: &World, zone: &Zone, orders: &mut Vec<Order>) {
        // A copy the fields change – if it differs from the region afterwards, it goes out as one order.
        let mut new = zone.clone();
        let mut remove = false;
        section_from(ui, "Region", Some(Origin::Sc2), "", |ui| {
            ui.horizontal(|ui| {
                ui.label("Name");
                if let Some(name) = settled_name(ui, &zone.name) {
                    new.name = name;
                }
            });
            color_row(ui, &mut new.color);
            let tiles = self.outlines.iter().find(|o| o.zone.id == zone.id).map_or(0, |o| o.tiles);
            ui.label(RichText::new(format!("{tiles} tiles - shapes, each added to or cut out of the ones before:")).small().weak());
            let mut gone = None;
            let several = new.shapes.len() > 1;
            for (k, shape) in new.shapes.iter_mut().enumerate() {
                // `push_id`: the fields of every shape get ids of their own, whatever their number.
                ui.push_id(k, |ui| {
                    ui.horizontal(|ui| {
                        let sign = if shape.cut { "-" } else { "+" };
                        if ui.small_button(sign).on_hover_text("+ added, - cut out - click: the other one").clicked() {
                            shape.cut = !shape.cut;
                        }
                        shape_fields(ui, &mut shape.form);
                        if several && ui.small_button("x").on_hover_text("Take this shape away").clicked() {
                            gone = Some(k);
                        }
                    });
                });
            }
            if let Some(k) = gone {
                new.shapes.remove(k);
            }
            ui.horizontal(|ui| {
                if ui.button("Locate").clicked() {
                    self.request = zone.center().map(EditorRequest::Locate);
                }
                if ui.button("Duplicate").on_hover_text("The same shapes two tiles further, under a new name").clicked() {
                    let copy = Zone { name: format!("{} copy", zone.name), id: 0, color: self.next_color(world), ..zone.moved((2, 2)) };
                    self.picked = Some(Picked::Zone(new_id(world, orders)));
                    orders.push(Order::AddZone { zone: copy });
                }
                // Named by events: ask first – "Delete anyway" (Undo brings it back, with its place in the events).
                let used = !world.zone_uses(zone.id).is_empty();
                if ui.button("Delete").on_hover_text("Del").clicked() {
                    if used {
                        self.zone_asked = Some(zone.id);
                    } else {
                        remove = true;
                    }
                }
            });
        });
        let uses = world.zone_uses(zone.id);
        if !uses.is_empty() {
            section_from(ui, "Used by", Some(Origin::New), "events that bring zombies out of it", |ui| {
                for line in &uses {
                    ui.label(line);
                }
                if self.zone_asked == Some(zone.id) {
                    ui.label(RichText::new("Deleting it takes it out of these.").color(Color32::from_rgb(242, 115, 89)));
                    ui.horizontal(|ui| {
                        remove = ui.button("Delete anyway").clicked();
                        if ui.button("Cancel").clicked() {
                            self.zone_asked = None;
                        }
                    });
                }
            });
        }
        if remove {
            self.zone_asked = None;
            orders.extend(self.remove_picked());
        } else if new != *zone {
            orders.push(Order::SetZone { zone: new });
        }
    }

    fn spot_fields(&mut self, ui: &mut Ui, _world: &World, spot: &Spot, orders: &mut Vec<Order>) {
        let mut new = spot.clone();
        let mut remove = false;
        section_from(ui, "Point", Some(Origin::Sc2), "", |ui| {
            ui.horizontal(|ui| {
                ui.label("Name");
                if let Some(name) = settled_name(ui, &spot.name) {
                    new.name = name;
                }
            });
            color_row(ui, &mut new.color);
            ui.horizontal(|ui| {
                ui.label("Tile");
                number(ui, &mut new.tile.0, i32::MIN..=i32::MAX);
                number(ui, &mut new.tile.1, i32::MIN..=i32::MAX);
            });
            ui.horizontal(|ui| {
                if ui.button("Locate").clicked() {
                    self.request = Some(EditorRequest::Locate(spot.center()));
                }
                remove = ui.button("Delete").on_hover_text("Del").clicked();
            });
        });
        if remove {
            orders.extend(self.remove_picked());
        } else if new != *spot {
            orders.push(Order::SetSpot { spot: new });
        }
    }

    /// The colour a new region or point gets: the next of the row.
    fn next_color(&self, world: &World) -> u8 {
        ((world.zones.len() + world.spots.len()) % ZONE_COLORS as usize) as u8
    }

    /// Del, "Delete": the selected region or point goes.
    pub fn remove_picked(&mut self) -> Option<Order> {
        Some(match self.picked.take()? {
            Picked::Zone(zone) => Order::RemoveZone { zone },
            Picked::Spot(spot) => Order::RemoveSpot { spot },
        })
    }

    /// The shape a rectangle or circle tool makes dragged from `start` to `end`: the box between the two tiles, or
    /// the circle around the first one that reaches the second.
    pub fn region_form(&self, tool: Tool, start: (i32, i32), end: (i32, i32)) -> Form {
        if tool == Tool::ZoneCircle {
            let (dx, dy) = ((end.0 - start.0) as f32, (end.1 - start.1) as f32);
            return Form::Circle { center: start, radius: (dx * dx + dy * dy).sqrt().round() as i32 };
        }
        let area = Area::between(start, end);
        Form::Rect { min: area.min, size: area.size() }
    }

    /// A shape dragged on the map: a new region of it – or, with a region selected, added to it (`add`, Ctrl) or cut
    /// out of it (`cut`, Alt).
    pub fn add_shape(&mut self, world: &World, form: Form, add: bool, cut: bool, orders: &mut Vec<Order>) {
        let picked = match self.picked {
            Some(Picked::Zone(id)) if add || cut => world.find_zone(id).map(|z| &world.zones[z]),
            _ => None,
        };
        match picked {
            Some(zone) if zone.shapes.len() < MOST_SHAPES => {
                let mut zone = zone.clone();
                zone.shapes.push(Shape { form, cut });
                orders.push(Order::SetZone { zone });
            }
            // Full: nothing happens rather than a new region nobody asked for.
            Some(_) => {}
            // Cutting needs a region to cut from.
            None if cut => {}
            None => {
                self.picked = Some(Picked::Zone(new_id(world, orders)));
                self.select_spawner(None);
                orders.push(Order::AddZone { zone: Zone::new("", self.next_color(world), form) });
            }
        }
    }

    /// "+ Paint" pressed on the map: the brush goes into the selected region (`cut`, Alt: out of it) – with none
    /// selected it starts a new one. The strokes after it follow with `paint_region`.
    pub fn start_region_paint(&mut self, world: &World, tiles: Vec<(i32, i32)>, cut: bool, orders: &mut Vec<Order>) {
        self.region_stroke = None;
        let picked = match self.picked {
            Some(Picked::Zone(id)) => world.find_zone(id).map(|_| id),
            _ => None,
        };
        match picked {
            Some(zone) => {
                self.region_stroke = Some((zone, cut));
                orders.push(Order::PaintZone { zone, tiles, cut });
            }
            None if cut => {}
            None => {
                let id = new_id(world, orders);
                self.picked = Some(Picked::Zone(id));
                self.select_spawner(None);
                self.region_stroke = Some((id, false));
                orders.push(Order::AddZone { zone: Zone::new("", self.next_color(world), Form::Tiles(tiles)) });
            }
        }
    }

    /// The brush dragged on: these tiles too.
    pub fn paint_region(&self, tiles: Vec<(i32, i32)>, orders: &mut Vec<Order>) {
        if let Some((zone, cut)) = self.region_stroke {
            orders.push(Order::PaintZone { zone, tiles, cut });
        }
    }

    /// "+ Point" clicked on the map.
    pub fn add_point(&mut self, world: &World, tile: (i32, i32), orders: &mut Vec<Order>) {
        self.picked = Some(Picked::Spot(new_id(world, orders)));
        self.select_spawner(None);
        orders.push(Order::AddSpot { spot: Spot { name: String::new(), id: 0, color: self.next_color(world), tile } });
    }

    /// What a click at `point` (world) selects: a point close to the mouse, otherwise the region the tile lies in –
    /// of several the selected one first, so it can be dragged out from under another.
    pub fn pick_at(&self, world: &World, camera: &Camera, point: Vec2) -> Option<Picked> {
        let mouse = camera.world_to_screen(point);
        let close = |s: &&Spot| camera.world_to_screen(s.center()).distance(mouse) < POINT_GRAB;
        if let Some(spot) = world.spots.iter().rev().find(close) {
            return Some(Picked::Spot(spot.id));
        }
        let tile = (point.x.floor() as i32, point.y.floor() as i32);
        if let Some(Picked::Zone(id)) = self.picked
            && world.find_zone(id).is_some_and(|z| world.zones[z].contains(tile))
        {
            return self.picked;
        }
        world.zone_at(tile).map(|z| Picked::Zone(z.id))
    }

    /// The rectangles of the selected region – (which shape, corner, size) – whose edges can be dragged.
    pub fn picked_rects(&self, world: &World) -> Vec<PickedRect> {
        let Some(Picked::Zone(id)) = self.picked else {
            return Vec::new();
        };
        let Some(z) = world.find_zone(id) else {
            return Vec::new();
        };
        let rect = |(k, shape): (usize, &Shape)| match shape.form {
            Form::Rect { min, size } => Some((k, min, size)),
            _ => None,
        };
        world.zones[z].shapes.iter().enumerate().filter_map(rect).collect()
    }

    /// A rectangle of a region let go after dragging its edge: shape `k` lies here now.
    pub fn resize_order(&self, world: &World, id: u32, k: usize, min: (i32, i32), size: (i32, i32)) -> Option<Order> {
        let mut zone = world.zones[world.find_zone(id)?].clone();
        zone.shapes.get_mut(k)?.form = Form::Rect { min, size };
        Some(Order::SetZone { zone })
    }

    /// A region or a point let go after dragging it `d` tiles.
    pub fn move_order(&self, world: &World, what: Picked, d: (i32, i32)) -> Option<Order> {
        if d == (0, 0) {
            return None;
        }
        Some(match what {
            Picked::Zone(id) => Order::SetZone { zone: world.zones[world.find_zone(id)?].moved(d) },
            Picked::Spot(id) => {
                let spot = &world.spots[world.find_spot(id)?];
                Order::SetSpot { spot: Spot { tile: (spot.tile.0 + d.0, spot.tile.1 + d.1), ..spot.clone() } }
            }
        })
    }

    /// The name of the region the mouse is over, and how many regions and points there are – for the status line.
    pub(super) fn region_status(&self, world: &World) -> (Option<&str>, String) {
        let name = self.hover.and_then(|tile| self.outlines.iter().rev().find(|o| o.zone.contains(tile))).map(|o| o.zone.name.as_str());
        (name, format!("{} regions, {} points", world.zones.len(), world.spots.len()))
    }

    /// Draws the regions and points on the map (macroquad, after the map itself). `drag`: one of them is being
    /// dragged this many tiles – it's shown where it would land.
    pub fn draw_regions(&self, world: &World, camera: &Camera, drag: Option<(Picked, (i32, i32))>) {
        if !self.shows_regions() {
            return;
        }
        let offset = |what: Picked| match drag {
            Some((dragged, d)) if dragged == what => vec2(d.0 as f32, d.1 as f32),
            _ => Vec2::ZERO,
        };
        let corner = |(x, y): (i32, i32)| vec2(x as f32, y as f32);
        for outline in &self.outlines {
            let zone = &outline.zone;
            let picked = self.picked == Some(Picked::Zone(zone.id));
            let d = offset(Picked::Zone(zone.id));
            let color = map_color(zone.color);
            for &(y, x0, x1) in &outline.runs {
                draw_diamond(camera, corner((x0, y)) + d, vec2((x1 - x0) as f32, 1.0), Color { a: if picked { 0.34 } else { 0.2 }, ..color });
            }
            for &(a, b) in &outline.edges {
                let (p, q) = (camera.world_to_screen(corner(a) + d), camera.world_to_screen(corner(b) + d));
                draw_line(p.x, p.y, q.x, q.y, if picked { 3.0 } else { 2.0 }, if picked { WHITE } else { color });
            }
            // The corners of its rectangles: their edges can be dragged.
            if picked {
                for shape in &zone.shapes {
                    if let Form::Rect { min, size } = shape.form {
                        for c in [min, (min.0 + size.0, min.1), (min.0 + size.0, min.1 + size.1), (min.0, min.1 + size.1)] {
                            let s = camera.world_to_screen(corner(c) + d);
                            draw_circle(s.x, s.y, 4.0, WHITE);
                        }
                    }
                }
            }
            if let Some(center) = zone.center() {
                label(camera.world_to_screen(center + d), &zone.name, if picked { 20.0 } else { 17.0 });
            }
        }
        for spot in &world.spots {
            let picked = self.picked == Some(Picked::Spot(spot.id));
            let s = camera.world_to_screen(spot.center() + offset(Picked::Spot(spot.id)));
            draw_circle(s.x, s.y, 7.0, map_color(spot.color));
            draw_circle_lines(s.x, s.y, if picked { 10.0 } else { 7.0 }, 2.0, WHITE);
            // Above the point: a region's name sits in its middle, and a point in it would be under that.
            label(s - vec2(0.0, 18.0), &spot.name, 16.0);
        }
    }
}

/// The number the next region or point gets: the world's next one – plus those of this frame still waiting.
fn new_id(world: &World, orders: &[Order]) -> u32 {
    let waiting = orders.iter().filter(|o| matches!(o, Order::AddZone { .. } | Order::AddSpot { .. })).count() as u32;
    world.next_zone_id() + waiting
}

/// A name on the map, its middle at `at` (screen): white on a dark shadow, so it reads on grass and on snow.
fn label(at: Vec2, text: &str, size: f32) {
    let width = measure_text(text, None, size as u16, 1.0).width;
    let (x, y) = (at.x - width / 2.0, at.y + size * 0.3);
    draw_text(text, x + 1.0, y + 1.0, size, Color::new(0.0, 0.0, 0.0, 0.85));
    draw_text(text, x, y, size, WHITE);
}

/// "Colour": the row of colours to click – only for the editor, the game never shows a region.
fn color_row(ui: &mut Ui, color: &mut u8) {
    ui.horizontal(|ui| {
        ui.label("Colour");
        for k in 0..ZONE_COLORS {
            let (rect, response) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), Sense::click());
            ui.painter().rect_filled(rect, 3.0, color32(k));
            if *color == k {
                ui.painter().rect_stroke(rect, 3.0, egui::Stroke::new(2.0_f32, Color32::WHITE), egui::StrokeKind::Inside);
            }
            if response.clicked() {
                *color = k;
            }
        }
    });
}

/// The numbers of one shape, to type or drag.
fn shape_fields(ui: &mut Ui, form: &mut Form) {
    match form {
        Form::Rect { min, size } => {
            ui.label("Rect").on_hover_text("A rectangle: its corner tile, then its width and depth");
            number(ui, &mut min.0, i32::MIN..=i32::MAX);
            number(ui, &mut min.1, i32::MIN..=i32::MAX);
            ui.label("size");
            number(ui, &mut size.0, 1..=i32::MAX);
            number(ui, &mut size.1, 1..=i32::MAX);
        }
        Form::Circle { center, radius } => {
            ui.label("Circle").on_hover_text("A circle: its middle tile, then its radius");
            number(ui, &mut center.0, i32::MIN..=i32::MAX);
            number(ui, &mut center.1, i32::MIN..=i32::MAX);
            ui.label("r");
            number(ui, radius, 0..=i32::MAX);
        }
        Form::Tiles(tiles) => {
            ui.label(format!("{} painted tiles", tiles.len()));
        }
    }
}

/// A number field that only counts when it's let go (`settled`) – one order, not one per step of the drag.
fn number(ui: &mut Ui, value: &mut i32, range: std::ops::RangeInclusive<i32>) {
    if let Some(new) = settled(ui, *value, |v| DragValue::new(v).range(range).speed(0.2)) {
        *value = new;
    }
}
