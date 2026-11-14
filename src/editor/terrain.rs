//! The **Terrain** module (`docs\editor.md`, step 3): how the painting tools go over the map, which pictures they
//! paint with, the area tools, the landscape – and on the right the minimap, the layers and "Under the mouse".
//!
//! **Shapes.** Whatever paints (ground, ore, smoothing, the eraser, the zombie brush) can go over the map four ways:
//! as a **brush** along the mouse, as a **line** or a **rectangle** from where the button went down to where it's let
//! go, or as a **fill** of the patch that was clicked. All four only pick tiles – `paint_orders` turns them into
//! orders, so undo and the map file know nothing of shapes. `ui.rs` reads the mouse and asks here.
//!
//! **Density** below 100 % leaves tiles out, scattered (TAB's painter: forest with clearings, ore in crumbs). Which
//! ones is decided by the tile and the stroke, not by dice thrown anew every frame – so dragging over a tile again
//! in the same stroke doesn't fill it up.
//!
//! **Pictures.** Forest, rocks and ore each have a choice "mixed, A, B, C": the landscape's pictures in three groups
//! (`Map::picture`, drawn by `ground.rs`). Painting sets it along with the ground (`Order::PaintPicture`).
//!
//! **Area.** "Select area" drags a rectangle of tiles. Its ground can be copied and pasted elsewhere
//! (`World::copy_terrain`, `Order::PasteTerrain`), and TAB's generator can make a forest, a lake, mountains, a field
//! of ore or a village of doom in it (`Order::Generate`, `sim\landscaping.rs`).
//!
//! **Minimap and layers.** The minimap is the whole map as one small picture for egui (one pixel per tile, turned
//! like the map on screen), with the buildings, the border and the camera's frame on it – a click moves the camera.
//! "Show" switches parts of the map off and on (`draw::Layers`).

use egui_macroquad::egui::{self, Color32, ColorImage, RichText, Slider, TextureOptions, Ui};
use macroquad::math::{vec2, Vec2};

use super::{section_from, Editor, EditorRequest, Mode, Origin, Sight, Tool};
use crate::draw::{building_color, ore_color, terrain_color};
use crate::iso::{Camera, TILE_H, TILE_W};
use crate::map::{from_frame, to_frame, Ore, Terrain, BIG_ROCK_SIZES, PICTURES};
use crate::sim::{Area, Order, Patch, World, LOOKS};

/// The minimap is at most this high (points) – a map far higher than wide would push everything else off the panel.
const MINIMAP_MAX_H: f32 = 260.0;
/// On the minimap: the houses of doom, the world border, the camera's frame.
const MINI_DOOM: Color32 = Color32::from_rgb(217, 31, 26);
const MINI_BORDER: Color32 = Color32::from_rgb(235, 200, 90);

/// How a painting tool goes over the map.
#[derive(Clone, Copy, PartialEq)]
pub enum PaintShape {
    /// Along the mouse while the button is held.
    Brush,
    /// From the tile where the button went down to the one where it's let go, as thick as the brush.
    Line,
    /// Every tile of the box between those two.
    Rect,
    /// The whole patch of the same ground (or the same ore) around the tile clicked.
    Fill,
}

/// The painter's layers that have pictures to choose from – the index into `Editor::pictures`.
#[derive(Clone, Copy)]
enum Layer {
    Forest = 0,
    Rocks = 1,
    Ore = 2,
}

impl Editor {
    /// The shape `tool` paints in right now. Oil is always stamped (a node of 2 × 2), and only ground and ore can
    /// fill – smoothing, the eraser and the zombie brush have no patch to follow.
    pub fn shape_of(&self, tool: Tool) -> PaintShape {
        match (self.shape, tool) {
            _ if !tool.paints() || tool == Tool::Ore(Some(Ore::Oil)) => PaintShape::Brush,
            (PaintShape::Fill, Tool::Paint(_) | Tool::Ore(_)) => PaintShape::Fill,
            (PaintShape::Fill, _) => PaintShape::Brush,
            (shape, _) => shape,
        }
    }

    /// The mouse button went down with a painting tool: a new stroke – the density leaves out other tiles than the
    /// last one did.
    pub fn new_stroke(&mut self) {
        self.stroke = self.stroke.wrapping_add(1);
    }

    /// Leaves out tiles until only `density` percent are left – always the same ones within a stroke.
    pub fn thin(&self, tiles: &mut Vec<(i32, i32)>) {
        if self.density < 100 {
            tiles.retain(|&(x, y)| mix(x, y, self.stroke) % 100 < self.density);
        }
    }

    /// The pictures the painter gives what `tool` paints: 0 mixed, 1–3 the group A, B, C – `None` for a tool that
    /// paints nothing with pictures (water, bare earth, oil, the eraser).
    fn picture_of(&self, tool: Tool) -> Option<u8> {
        let layer = match tool {
            Tool::Paint(Terrain::Forest) => Layer::Forest,
            Tool::Paint(Terrain::Mountain) => Layer::Rocks,
            Tool::Ore(Some(ore)) if ore != Ore::Oil => Layer::Ore,
            _ => return None,
        };
        Some(self.pictures[layer as usize])
    }

    /// A painting tool on these tiles, as orders: first the density leaves out its tiles (`thin`), then – for ground
    /// nobody can walk on and for smoothing – "Keep buildings and tracks free" its own (`World::spare`). Ground and
    /// ore with pictures get a second order for them; both belong to the same stroke, so undo takes them back as one.
    pub fn paint_orders(&self, world: &World, tool: Tool, mut tiles: Vec<(i32, i32)>, orders: &mut Vec<Order>) {
        self.thin(&mut tiles);
        let solid = matches!(tool, Tool::Paint(terrain) if terrain.is_solid()) || tool == Tool::Smooth;
        if self.spare && solid {
            world.spare(&mut tiles);
        }
        if tiles.is_empty() {
            return;
        }
        let picture = self.picture_of(tool);
        // `tiles.clone()`: the first order takes the list for itself (it *moves* into the order) – the second one
        // needs a copy of its own.
        match (tool, picture) {
            (Tool::Paint(terrain), Some(picture)) => {
                orders.push(Order::PaintTerrain { terrain, tiles: tiles.clone() });
                orders.push(Order::PaintPicture { picture, tiles });
            }
            (Tool::Paint(terrain), None) => orders.push(Order::PaintTerrain { terrain, tiles }),
            (Tool::Ore(ore), Some(picture)) => {
                orders.push(Order::PaintOre { ore, tiles: tiles.clone() });
                orders.push(Order::PaintPicture { picture, tiles });
            }
            (Tool::Ore(ore), None) => orders.push(Order::PaintOre { ore, tiles }),
            (Tool::Smooth, _) => orders.push(Order::SmoothTerrain { tiles }),
            (Tool::Remove, _) => orders.push(Order::RemoveAt { tiles }),
            (Tool::ZombieBrush, _) => orders.push(self.brush_order(tiles)),
            _ => {}
        }
    }

    /// "Copy" (or Ctrl+C in the Terrain module): the ground of the selected area into the editor's clipboard for
    /// ground. Copying only reads the world – no order.
    pub fn copy_area(&mut self, world: &World) {
        if let Some(area) = self.area {
            self.terrain_clip = Some(world.copy_terrain(area));
            self.ground_copied = true;
        }
    }

    /// "Paste" (or Ctrl+V): the copied ground onto the mouse.
    pub fn take_paste(&mut self) {
        if self.terrain_clip.is_some() {
            self.take(Tool::PasteArea);
        }
    }

    /// Where the copied ground would land with the mouse on `tile` – its middle under the mouse: the corner tile and
    /// the size. `None`: nothing copied.
    pub fn paste_spot(&self, tile: (i32, i32)) -> Option<((i32, i32), (i32, i32))> {
        let (w, h) = self.terrain_clip.as_ref()?.size;
        Some(((tile.0 - w / 2, tile.1 - h / 2), (w, h)))
    }

    /// The order that puts the copied ground down there.
    pub fn paste_order(&self, tile: (i32, i32)) -> Option<Order> {
        let (to, _) = self.paste_spot(tile)?;
        let clip = self.terrain_clip.clone()?;
        Some(Order::PasteTerrain { clip: Box::new(clip), to, spare: self.spare })
    }

    /// The Terrain module's tools, on the left.
    pub(super) fn terrain_module(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        let edit = self.mode == Mode::Edit;
        section_from(ui, "Landscape", Some(Origin::Tab), "which of TAB's six the map is drawn as - with its weather", |ui| {
            ui.horizontal_wrapped(|ui| {
                // A map without a landscape of its own is drawn as BR.
                let now = if world.look.is_empty() { "BR" } else { world.look.as_str() };
                for (look, name) in LOOKS {
                    if ui.selectable_label(now == look, look).on_hover_text(name).clicked() && now != look {
                        orders.push(Order::SetLook { look: look.to_owned() });
                    }
                }
            });
        });
        section_from(ui, "Paint", Some(Origin::Tab), "hold and drag", |ui| {
            ui.horizontal_wrapped(|ui| {
                for terrain in [Terrain::Mountain, Terrain::Forest, Terrain::Water, Terrain::Dirt, Terrain::Grass] {
                    self.tool_button(ui, Tool::Paint(terrain));
                }
            });
            self.picture_row(ui, "Forest", Layer::Forest);
            self.picture_row(ui, "Mountain", Layer::Rocks);
            ui.horizontal(|ui| {
                self.tool_button(ui, Tool::Smooth);
                ui.label(RichText::new("every tile takes what most around it are").small().weak());
            });
        });
        section_from(ui, "Big rocks", Some(Origin::Tab), "click - its tiles become mountain, paint other ground over it to take it away", |ui| {
            ui.horizontal(|ui| {
                for size in BIG_ROCK_SIZES {
                    self.tool_button(ui, Tool::BigRock(size));
                }
            });
            ui.horizontal(|ui| {
                ui.label("Picture");
                ui.radio_value(&mut self.rock_variant, None, "mixed");
                for (variant, name) in [(0, "A"), (1, "B"), (2, "C")] {
                    ui.radio_value(&mut self.rock_variant, Some(variant), name);
                }
            });
        });
        section_from(ui, "Ore", Some(Origin::Tab), "only on open ground - oil: one 2 x 2 node", |ui| {
            ui.horizontal_wrapped(|ui| {
                for ore in [Some(Ore::Iron), Some(Ore::Coal), Some(Ore::Stone), Some(Ore::Gold), Some(Ore::Oil), None] {
                    self.tool_button(ui, Tool::Ore(ore));
                }
            });
            self.picture_row(ui, "Ore", Layer::Ore);
        });
        self.brush_size(ui, true);
        self.area_section(ui, world, edit);
        self.generate_section(ui, orders, edit);
    }

    /// "Forest  mixed A B C": which group of the landscape's pictures the painter gives this layer.
    fn picture_row(&mut self, ui: &mut Ui, name: &str, layer: Layer) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{name} pictures")).weak());
            let picture = &mut self.pictures[layer as usize];
            ui.radio_value(picture, 0, "mixed");
            for (group, letter) in PICTURES {
                ui.radio_value(picture, group, letter);
            }
        })
        .response
        .on_hover_text("The landscape's pictures in three groups - painting over what's there changes its pictures");
    }

    /// The section "Brush" under the painting tools: shape, round or square, size, density – and, in the Terrain
    /// module (`terrain`), what the painter leaves alone.
    pub(super) fn brush_size(&mut self, ui: &mut Ui, terrain: bool) {
        section_from(ui, "Brush", None, "how the painting tools go over the map", |ui| {
            ui.horizontal(|ui| {
                for (shape, text, hint) in [
                    (PaintShape::Brush, "Brush", "Hold and drag"),
                    (PaintShape::Line, "Line", "Press, drag, let go: a straight line as thick as the brush"),
                    (PaintShape::Rect, "Rectangle", "Press, drag, let go: every tile of the box"),
                    (PaintShape::Fill, "Fill", "Click: the whole patch of the same ground (or the same ore) - only ground and ore"),
                ] {
                    ui.selectable_value(&mut self.shape, shape, text).on_hover_text(hint);
                }
            });
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.square_brush, false, "Round");
                ui.selectable_value(&mut self.square_brush, true, "Square");
            });
            ui.add(Slider::new(&mut self.brush, 1..=41).suffix(" tiles"));
            // Odd, so the brush has a middle tile – an even size would paint like the next smaller one
            // (`brush_tiles`). `|= 1`: sets the lowest bit, 4 becomes 5.
            self.brush |= 1;
            ui.add(Slider::new(&mut self.density, 1..=100).suffix(" %").text("density"))
                .on_hover_text("Below 100 %: only so many of the tiles, scattered - forest with clearings, ore in crumbs");
            if terrain {
                ui.checkbox(&mut self.spare, "Keep buildings and tracks free")
                    .on_hover_text("Mountain, forest and water aren't painted, pasted or generated on or right next to buildings, houses of doom and the railway");
            }
        });
    }

    /// "Area": select a rectangle of tiles, copy its ground, paste it.
    fn area_section(&mut self, ui: &mut Ui, world: &World, edit: bool) {
        section_from(ui, "Area", Some(Origin::Sc2), "drag a rectangle on the map - copy its ground, or generate in it", |ui| {
            ui.horizontal(|ui| {
                self.tool_button(ui, Tool::Area);
                match self.area {
                    Some(area) => {
                        let (w, h) = area.size();
                        ui.label(format!("{w} x {h} tiles from {}, {}", area.min.0, area.min.1));
                    }
                    None => {
                        ui.label(RichText::new("none yet").weak());
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(self.area.is_some(), egui::Button::new("Copy")).on_hover_text("Ctrl+C - ground, forest, rocks, ore and their pictures; not what stands on it").clicked() {
                    self.copy_area(world);
                }
                let paste = egui::Button::new("Paste").selected(self.tool == Some(Tool::PasteArea));
                if ui.add_enabled(edit && self.terrain_clip.is_some(), paste).on_hover_text("Ctrl+V - the copy on the mouse, a click puts it down").clicked() {
                    self.toggle_tool(Tool::PasteArea);
                }
                if ui.add_enabled(self.area.is_some(), egui::Button::new("Deselect")).clicked() {
                    self.area = None;
                }
            });
            if let Some(clip) = &self.terrain_clip {
                ui.label(RichText::new(format!("Copied: {} x {} tiles", clip.size.0, clip.size.1)).small().weak());
            }
        });
    }

    /// "Generate in the area": a patch made the way TAB's generator makes them, from a seed.
    fn generate_section(&mut self, ui: &mut Ui, orders: &mut Vec<Order>, edit: bool) {
        section_from(ui, "Generate in the area", Some(Origin::Tab), "TAB's generator, only inside the selected area", |ui| {
            egui::Grid::new("generate").num_columns(2).show(ui, |ui| {
                ui.label("What");
                egui::ComboBox::from_id_salt("patch").selected_text(self.patch.name()).show_ui(ui, |ui| {
                    for patch in Patch::ALL {
                        ui.selectable_value(&mut self.patch, patch, patch.name());
                    }
                });
                ui.end_row();
                if self.patch.has_amount() {
                    ui.label("Amount");
                    ui.add(Slider::new(&mut self.patch_percent, 5..=100).suffix(" %")).on_hover_text("How much of the area it takes - 100 %: all of it");
                    ui.end_row();
                }
                ui.label("Seed");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut self.patch_seed));
                    // `rand()`: macroquad's own dice – only for a new number here, the patch itself comes from the seed.
                    if ui.button("New").on_hover_text("Another seed: another patch").clicked() {
                        self.patch_seed = macroquad::rand::rand() % 100_000;
                    }
                });
                ui.end_row();
            });
            let button = ui.add_enabled(edit && self.area.is_some(), egui::Button::new("Generate"));
            if button.on_hover_text("The same seed in the same area makes the same patch - Ctrl+Z takes it back").clicked()
                && let Some(area) = self.area
            {
                orders.push(Order::Generate { what: self.patch, area, percent: self.patch_percent, seed: self.patch_seed as u64, spare: self.spare });
            }
            if self.area.is_none() {
                ui.label(RichText::new("Select an area first.").small().weak());
            }
        });
    }

    /// On the right: the whole map small. A click or a drag on it moves the camera there.
    pub(super) fn minimap_section(&mut self, ui: &mut Ui, world: &World, camera: &Camera) {
        section_from(ui, "Minimap", Some(Origin::Sc2), "click or drag: the camera goes there", |ui| {
            // The picture: painted anew only when the ground changed (`Map::revision`) or the map is another size.
            let n = world.map.size;
            let fresh = matches!(&self.minimap, Some((_, revision, size)) if *revision == world.map.revision && *size == n);
            if !fresh {
                // `NEAREST`: sharp pixels – every tile stays a clear little square.
                let texture = ui.ctx().load_texture("minimap", minimap_image(world), TextureOptions::NEAREST);
                self.minimap = Some((texture, world.map.revision, n));
            }
            let Some((texture, ..)) = &self.minimap else {
                return;
            };
            // The whole map is a straight rectangle on screen (`Map::full_area`, in the screen axes): as wide as the
            // panel, as high as its shape on screen asks – a step across is `TILE_W / 2` pixels, one down `TILE_H / 2`.
            let area = world.map.full_area();
            let extent = area.max - area.min;
            let width = ui.available_width();
            let height = width * (extent.y * TILE_H) / (extent.x * TILE_W);
            let scale = (MINIMAP_MAX_H / height).min(1.0);
            // `allocate_exact_size`: room of this size in the panel – and what the mouse does in it (`response`).
            let (rect, response) = ui.allocate_exact_size(egui::vec2(width * scale, height * scale), egui::Sense::click_and_drag());
            let to_mini = |p: Vec2| {
                let f = (to_frame(p) - area.min) / extent;
                egui::pos2(rect.min.x + f.x * rect.width(), rect.min.y + f.y * rect.height())
            };
            // Nothing is drawn outside the minimap's room.
            let painter = ui.painter().with_clip_rect(rect);
            painter.rect_filled(rect, 0.0, Color32::BLACK);
            // The picture lies turned on it: its four corners are the map's four corners on screen. A `Mesh` is a
            // shape of triangles with a picture on it – here two, like `picture::draw_tiles` does it for the game.
            let mut mesh = egui::Mesh::with_texture(texture.id());
            for corner in area.corners() {
                mesh.vertices.push(egui::epaint::Vertex { pos: to_mini(corner), uv: egui::pos2(corner.x / n as f32, corner.y / n as f32), color: Color32::WHITE });
            }
            mesh.indices = vec![0, 1, 2, 0, 2, 3];
            painter.add(egui::Shape::mesh(mesh));
            // A tile-shaped patch: its four corners as a filled polygon.
            let diamond = |corner: Vec2, size: Vec2, color: Color32| {
                let points = [corner, corner + vec2(size.x, 0.0), corner + size, corner + vec2(0.0, size.y)].map(to_mini).to_vec();
                painter.add(egui::Shape::convex_polygon(points, color, egui::Stroke::NONE));
            };
            if self.layers.things {
                for building in &world.buildings {
                    diamond(building.corner(), building.far_corner() - building.corner(), color32(building_color(building.kind)));
                }
                for house in &world.doom {
                    diamond(house.corner() - Vec2::splat(0.5), house.far_corner() - house.corner() + Vec2::ONE, MINI_DOOM);
                }
            }
            // The world border, and what the camera shows right now – both straight rectangles here.
            let frame = |a: Vec2, b: Vec2, color: Color32| {
                let (a, b) = (to_mini(a), to_mini(b));
                painter.rect_stroke(egui::Rect::from_two_pos(a, b), 0.0, egui::Stroke::new(1.0_f32, color), egui::StrokeKind::Inside);
            };
            let play = world.map.play_area();
            frame(from_frame(play.min), from_frame(play.max), MINI_BORDER);
            let view = crate::view::rect();
            frame(camera.screen_to_world(view.point()), camera.screen_to_world(vec2(view.right(), view.bottom())), Color32::WHITE);
            // `interact_pointer_pos`: where the mouse is while it clicks or drags here.
            if let Some(at) = response.interact_pointer_pos() {
                let f = vec2((at.x - rect.min.x) / rect.width(), (at.y - rect.min.y) / rect.height());
                self.request = Some(EditorRequest::Locate(area.clamp(from_frame(area.min + f * extent), 0.01)));
            }
        });
    }

    /// "Show": what of the map is drawn – SC2's layers.
    pub(super) fn layers_section(&mut self, ui: &mut Ui, grid: &mut bool) {
        section_from(ui, "Show", Some(Origin::Sc2), "what of the map is drawn - also in the View menu", |ui| self.layer_boxes(ui, grid));
    }

    /// The switches of "Show" – here and in the View menu. The grid is a setting of the game (`grid`), the fog one of
    /// the three sights (the bar over the map).
    pub(super) fn layer_boxes(&mut self, ui: &mut Ui, grid: &mut bool) {
        ui.checkbox(&mut self.layers.ground, "Ground");
        ui.checkbox(&mut self.layers.cover, "Forest, rocks, ore");
        ui.checkbox(&mut self.layers.things, "Buildings and units");
        ui.checkbox(&mut self.layers.zombies, "Zombies");
        ui.checkbox(&mut self.layers.regions, "Regions and points").on_hover_text("In the Regions module they're always shown");
        ui.checkbox(grid, "Grid").on_hover_text("TAB's grid view: the tiles, and where nothing can be built");
        let mut fog = self.sight == Sight::Player;
        if ui.checkbox(&mut fog, "Fog of war").on_hover_text("What the player sees - like \"Sight: Player\" over the map").changed() {
            self.sight = if fog { Sight::Player } else { Sight::All };
        }
        ui.checkbox(&mut self.layers.paths, "Paths of the zombies")
            .on_hover_text("On every tile the way a zombie walks from there to the command center - a red dot: no way, shut in");
        ui.checkbox(&mut self.layers.noise, "Noise as a heat map")
            .on_hover_text("How loud it is where, yellow to red, in cells of 4 x 4 tiles - noise is only made while a test runs");
    }

    /// In "Terrain" on the right: what lies on the tile the mouse was on last – SC2's status of the cell.
    pub(super) fn under_mouse_section(&self, ui: &mut Ui, world: &World) {
        section_from(ui, "Under the mouse", Some(Origin::Sc2), "", |ui| {
            let Some((x, y)) = self.hover.filter(|&(x, y)| world.map.on_map(x, y)) else {
                ui.label(RichText::new("Move the mouse over the map.").weak());
                return;
            };
            let on_it = if let Some(b) = world.building_at_point(vec2(x as f32 + 0.5, y as f32 + 0.5)) {
                world.buildings[b].stats().name.clone()
            } else if let Some(house) = world.doom.iter().find(|d| (d.tile.0..d.tile.0 + d.size.0).contains(&x) && (d.tile.1..d.tile.1 + d.size.1).contains(&y)) {
                format!("House of doom {} x {}", house.size.0, house.size.1)
            } else if world.map.is_doom(x, y) {
                "A neutral building".to_owned()
            } else if world.map.big_rocks().iter().any(|rock| rock.covers(x, y)) {
                "Big rock".to_owned()
            } else if let Some(ornament) = world.ornaments.iter().find(|o| o.covers((x, y))) {
                ornament.kind.name().to_owned()
            } else {
                "-".to_owned()
            };
            let yes = |on: bool| if on { "yes" } else { "no" };
            // The pictures chosen for what's on the tile: " (B)" – nothing while they're mixed. They belong to the ore
            // if there is some, else to the forest or the rock.
            let picture = PICTURES.iter().find(|&&(group, _)| group == world.map.picture(x, y)).map_or(String::new(), |(_, letter)| format!(" ({letter})"));
            let deposit = world.map.deposit(x, y);
            let ground = format!("{}{}", world.map.terrain(x, y).name(), if deposit.is_none() { picture.as_str() } else { "" });
            let ore = deposit.map_or("-".to_owned(), |deposit| format!("{}{picture} ({})", deposit.ore.name(), deposit.amount));
            egui::Grid::new("under the mouse").num_columns(2).show(ui, |ui| {
                for (name, value) in [
                    ("Tile", format!("{x}, {y}")),
                    ("Ground", ground),
                    ("Ore", ore),
                    ("On it", on_it),
                    ("Walkable", yes(!world.map.blocks_unit(x, y)).to_owned()),
                    ("Buildable", yes(world.map.is_buildable(x, y)).to_owned()),
                    ("Playing field", if world.map.in_play(x, y) { "inside" } else { "outside - the zombies' ring" }.to_owned()),
                ] {
                    ui.label(RichText::new(name).weak());
                    ui.label(value);
                    ui.end_row();
                }
            });
        });
    }
}

/// The whole grid as a picture for egui, one pixel per tile: the ore's colour where there is some, else the ground's
/// – like the game's own minimap (`picture.rs`).
fn minimap_image(world: &World) -> ColorImage {
    let n = world.map.size;
    let mut rgba = Vec::with_capacity(n * n * 4);
    for y in 0..n as i32 {
        for x in 0..n as i32 {
            let color = match world.map.deposit(x, y) {
                Some(deposit) => ore_color(deposit.ore),
                None => terrain_color(world.map.terrain(x, y)),
            };
            let c = color32(color);
            rgba.extend([c.r(), c.g(), c.b(), 255]);
        }
    }
    ColorImage::from_rgba_unmultiplied([n, n], &rgba)
}

/// macroquad's colour (four decimal numbers from 0 to 1) as egui's (four bytes).
fn color32(c: macroquad::color::Color) -> Color32 {
    Color32::from_rgb((c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8)
}

/// Every tile of the box between two tiles.
pub fn rect_tiles(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    Area::between(a, b).tiles().collect()
}

/// The patch a fill covers: every tile reached from `start` over neighbours (not diagonal) that are like it – the same
/// ground for a ground tool, the same ground and the same ore (or none) for an ore tool.
pub fn fill_tiles(world: &World, tool: Tool, start: (i32, i32)) -> Vec<(i32, i32)> {
    let map = &world.map;
    if !map.on_map(start.0, start.1) {
        return Vec::new();
    }
    let with_ore = matches!(tool, Tool::Ore(_));
    let key = |(x, y): (i32, i32)| (map.terrain(x, y), map.deposit(x, y).filter(|_| with_ore).map(|deposit| deposit.ore));
    let wanted = key(start);
    let n = map.size as i32;
    // A flood fill: a stack of tiles to look at, and for every tile whether it was put on it already.
    let mut seen = vec![false; (n * n) as usize];
    let mut todo = vec![start];
    let mut tiles = Vec::new();
    seen[(start.1 * n + start.0) as usize] = true;
    while let Some((x, y)) = todo.pop() {
        tiles.push((x, y));
        for next in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            if map.on_map(next.0, next.1) && !seen[(next.1 * n + next.0) as usize] && key(next) == wanted {
                seen[(next.1 * n + next.0) as usize] = true;
                todo.push(next);
            }
        }
    }
    tiles
}

/// A number that looks random, from a tile and a stroke – the same every time for the same three.
fn mix(x: i32, y: i32, stroke: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77) ^ stroke.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Terrain;
    use crate::sim::Order;

    /// A fill takes the whole patch and stops at other ground; half density leaves about half of the tiles, the same
    /// ones every time within a stroke and others in the next.
    #[test]
    fn fill_follows_the_patch_and_density_thins_it() {
        let mut world = World::new(3);
        let (x, y) = (world.base.x as i32 + 20, world.base.y as i32 + 20);
        // A lake of 6 × 4 in a frame of dirt.
        world.apply_orders(&[
            Order::PaintTerrain { terrain: Terrain::Dirt, tiles: rect_tiles((x - 1, y - 1), (x + 6, y + 4)) },
            Order::PaintTerrain { terrain: Terrain::Water, tiles: rect_tiles((x, y), (x + 5, y + 3)) },
        ]);
        let mut lake = fill_tiles(&world, Tool::Paint(Terrain::Grass), (x + 2, y + 2));
        lake.sort();
        let mut expected = rect_tiles((x, y), (x + 5, y + 3));
        expected.sort();
        assert_eq!(lake, expected);

        let mut editor = Editor::new(true);
        editor.density = 50;
        let all = rect_tiles((0, 0), (99, 99));
        let (mut first, mut again) = (all.clone(), all.clone());
        editor.thin(&mut first);
        editor.thin(&mut again);
        assert_eq!(first, again, "the same stroke left out other tiles");
        assert!((4_000..6_000).contains(&first.len()), "half density left {} of 10000", first.len());
        editor.new_stroke();
        let mut next = all;
        editor.thin(&mut next);
        assert_ne!(first, next, "a new stroke left out the same tiles");
    }

    /// The painter's orders: forest with a picture chosen is the ground and its pictures, water only the ground; with
    /// "Keep buildings and tracks free" solid ground stays off the command center and the tiles around it; a copied
    /// area lands with its middle under the mouse.
    #[test]
    fn the_painter_sets_pictures_spares_buildings_and_pastes_at_the_mouse() {
        let mut world = World::new(3);
        let (x, y) = (world.base.x as i32 + 20, world.base.y as i32 + 20);
        let mut editor = Editor::new(true);
        editor.pictures[Layer::Forest as usize] = 2;
        let tiles = rect_tiles((x, y), (x + 3, y + 3));
        // Whatever the generator put here and where the copy goes: gone, all grass.
        let ground = rect_tiles((x - 2, y - 32), (x + 6, y + 6));
        world.apply_orders(&[
            Order::RemoveAt { tiles: ground.clone() },
            Order::PaintOre { ore: None, tiles: ground.clone() },
            Order::PaintTerrain { terrain: Terrain::Grass, tiles: ground },
        ]);
        let mut orders = Vec::new();
        editor.paint_orders(&world, Tool::Paint(Terrain::Forest), tiles.clone(), &mut orders);
        assert_eq!(orders.len(), 2, "forest with pictures: the ground and the pictures");
        world.apply_orders(&orders);
        assert!(tiles.iter().all(|&(x, y)| world.map.terrain(x, y) == Terrain::Forest && world.map.picture(x, y) == 2));
        let mut orders = Vec::new();
        editor.paint_orders(&world, Tool::Paint(Terrain::Water), tiles.clone(), &mut orders);
        assert_eq!(orders.len(), 1, "water has no pictures");

        // The command center and a tile around it stay free.
        editor.spare = true;
        let (bx, by) = (world.base.x as i32, world.base.y as i32);
        let around = rect_tiles((bx - 6, by - 6), (bx + 6, by + 6));
        let mut orders = Vec::new();
        editor.paint_orders(&world, Tool::Paint(Terrain::Water), around.clone(), &mut orders);
        world.apply_orders(&orders);
        let wet = around.iter().filter(|&&(x, y)| world.map.terrain(x, y) == Terrain::Water).count();
        assert!(wet > 0 && wet < around.len(), "{wet} of {} tiles wet", around.len());
        let cc = &world.buildings[0];
        let (corner, far) = (cc.corner(), cc.far_corner());
        let ring = rect_tiles((corner.x as i32 - 1, corner.y as i32 - 1), (far.x as i32, far.y as i32));
        assert!(ring.iter().all(|&(x, y)| world.map.terrain(x, y) != Terrain::Water), "water on or right next to the command center");

        // Copy the forest, paste it with the mouse 30 tiles further up.
        editor.area = Some(Area::between((x, y), (x + 3, y + 3)));
        editor.copy_area(&world);
        assert_eq!(editor.paste_spot((x + 2, y - 28)), Some(((x, y - 30), (4, 4))));
        world.apply_orders(&[editor.paste_order((x + 2, y - 28)).unwrap()]);
        assert!(rect_tiles((x, y - 30), (x + 3, y - 27)).iter().all(|&(x, y)| world.map.terrain(x, y) == Terrain::Forest && world.map.picture(x, y) == 2));
    }
}
