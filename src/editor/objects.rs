//! The **Objects** module (`docs\editor.md`, step 2 – board 1 of the design):
//! - **Left** the catalog: every kind that can be put down, with its picture (`thumbs.rs`), in groups that fold, with
//!   a search field and filters – a click takes it in hand. Below it "Placing": whose the next one will be.
//! - **Below the map** TAB's entity list as a table: everything placed, a line each (kind, name ID, tile, owner, life,
//!   scripts), filtered and searched; a click selects it on the map, a double click moves the camera there.
//! - **Right** the properties of what's selected – of one thing all of them, of several the ones they share: a change
//!   goes to all.
//!
//! The selection is the map's own (`picking.rs`): the list only asks for it (`Editor::select`), and what's selected
//! comes back every frame (`Editor::selection`). So a click on the map and a click in the list are the same thing.

use egui_macroquad::egui::{self, Align, Align2, CollapsingHeader, Color32, DragValue, RichText, ScrollArea, Sense, TextEdit, TextStyle, Ui};
use macroquad::math::Vec2;

use super::{section, section_from, settled, settled_text, Editor, EditorRequest, Filter, Origin, Tool};
use crate::kinds::{BuildingKind, UnitKind};
use crate::sim::{LootKind, Neutral, Nest, Order, OrnamentKind, Owner, Piece, Tag, Thing, ThingRef, Tweak, TriggerEvent, World, MOST_ARMOR, MOST_LIFE, PERCENT};

/// A line of the catalog and its picture (points).
const ITEM_H: f32 = 30.0;
const ICON: f32 = 26.0;
/// A line of the table of placed things.
const ROW_H: f32 = 22.0;
/// The table's columns: heading, and where it starts (share of the table's width).
const COLUMNS: [(&str, f32); 6] = [("Kind", 0.0), ("Name ID", 0.27), ("Tile", 0.46), ("Owner", 0.57), ("Life", 0.68), ("Scripts", 0.77)];
/// The owners a thing can have, with the button's text and hint.
const OWNERS: [(Owner, &str, &str); 3] = [
    (Owner::Colony, "Colony", "Yours"),
    (Owner::Neutral, "Neutral", "Nobody's: it does nothing, the zombies leave it alone"),
    (Owner::Joinable, "Joins", "Nobody's until one of your units comes within a tile - then yours"),
];

/// The catalog's groups, in this order.
#[derive(Clone, Copy, PartialEq)]
enum Group {
    Colony,
    Defense,
    Wonders,
    Units,
    Doom,
    Decor,
    Railway,
    Loot,
}

impl Group {
    const ALL: [Group; 8] = [Group::Colony, Group::Defense, Group::Wonders, Group::Units, Group::Doom, Group::Decor, Group::Railway, Group::Loot];

    fn name(self) -> &'static str {
        match self {
            Group::Colony => "Housing and economy",
            Group::Defense => "Defense",
            Group::Wonders => "Wonders",
            Group::Units => "Units",
            Group::Doom => "Villages of doom",
            Group::Decor => "Decor",
            Group::Railway => "Railway",
            Group::Loot => "Loot",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Group::Colony | Group::Defense | Group::Wonders => "free, without the rules of the game - drag: a line",
            Group::Units => "click inside the border",
            Group::Doom => "TAB's three houses - zombies inside as the generator puts them",
            Group::Decor => "only the look - what lies round TAB's loot",
            Group::Railway => "a line runs from a buffer stop (End) to the next - a script sends the train (Triggers: + action > Train)",
            Group::Loot => "what lies in it and its guards rolled like the generator does",
        }
    }

    /// The filter button that shows this group.
    fn chip(self) -> Chip {
        match self {
            Group::Colony | Group::Wonders => Chip::Colony,
            Group::Defense => Chip::Defense,
            Group::Units => Chip::Units,
            Group::Doom => Chip::Neutral,
            Group::Decor | Group::Railway => Chip::Decor,
            Group::Loot => Chip::Loot,
        }
    }
}

/// The catalog's filter buttons.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Chip {
    All,
    Colony,
    Defense,
    Units,
    Neutral,
    Decor,
    Loot,
}

impl Chip {
    const ALL: [(Chip, &'static str); 7] =
        [(Chip::All, "All"), (Chip::Colony, "Colony"), (Chip::Defense, "Defense"), (Chip::Units, "Units"), (Chip::Neutral, "Neutral"), (Chip::Decor, "Decor"), (Chip::Loot, "Loot")];
}

/// One kind in the catalog: the tool a click takes in hand, its name, its picture (inside `assets\`, without `.png`)
/// and how many tiles it covers (`None`: a figure, a point).
pub(super) struct Entry {
    tool: Tool,
    group: Group,
    name: String,
    art: String,
    size: Option<(i32, i32)>,
}

/// Which group a building belongs to.
fn group_of(kind: BuildingKind) -> Group {
    use BuildingKind::*;
    match kind {
        Wall | Gate | Tower | StoneWall | StoneTower | Ballista | MachineGunTower | Stakes | Barrel | ShockingTower | Executor | BladeTrap | MineTrap | LookoutTower | Radar => {
            Group::Defense
        }
        Spire | Academy | Victorious | Transmutator | CrystalPalace => Group::Wonders,
        _ => Group::Colony,
    }
}

/// Everything that can be put down – every building and unit there is (a new kind in `BuildingKind::ALL` shows up by
/// itself), the houses of doom, the decoration, the railway, loot. `look`: the landscape, for the ruin's picture.
fn catalog(look: &str) -> Vec<Entry> {
    let mut list = Vec::new();
    for kind in BuildingKind::ALL {
        let (name, art) = (kind.stats().name.clone(), format!("ui/portraits/{kind:?}"));
        list.push(Entry { tool: Tool::Building(kind), group: group_of(kind), name, art, size: Some(kind.footprint(false)) });
    }
    for kind in UnitKind::ALL {
        let (name, art) = (kind.stats().name.clone(), format!("ui/portraits/{kind:?}"));
        list.push(Entry { tool: Tool::Unit(kind), group: Group::Units, name, art, size: None });
    }
    for (nest, art) in [(Nest::Small, "DoomSmall"), (Nest::Medium, "DoomMedium"), (Nest::Big, "DoomBig")] {
        let tool = Tool::Doom(nest);
        list.push(Entry { tool, group: Group::Doom, name: tool.name().to_owned(), art: format!("ui/portraits/{art}"), size: Some((nest.side(), nest.side())) });
    }
    let ornament = |kind: OrnamentKind, group: Group| {
        let name = match kind {
            OrnamentKind::PowerTower => "High power tower",
            OrnamentKind::PowerTowerFlip => "High power tower (mirrored)",
            _ => kind.name(),
        };
        let art = match kind.picture() {
            Some(picture) => format!("decor/{picture}"),
            // The ruin in the look of the landscape – the ones without a ruin of their own get the first.
            None => {
                let look = look.to_lowercase();
                format!("decor/treasure_ruin_{}", if ["al", "ds", "tm", "vo"].contains(&look.as_str()) { &look } else { "br" })
            }
        };
        Entry { tool: Tool::Ornament(kind), group, name: name.to_owned(), art, size: Some(kind.footprint()) }
    };
    let towers = [OrnamentKind::PowerTower, OrnamentKind::PowerTowerFlip];
    list.extend(OrnamentKind::SITES.into_iter().chain(towers).map(|kind| ornament(kind, Group::Decor)));
    list.push(Entry { tool: Tool::RailPlanner, group: Group::Railway, name: "Rail planner (like Factorio)".to_owned(), art: "decor/rail_curve_ne".to_owned(), size: None });
    list.extend(OrnamentKind::RAILWAY.into_iter().map(|kind| ornament(kind, Group::Railway)));
    // Every kind of pile – with its usual amount; what it holds can be changed once it lies there (`loot_section`).
    for kind in LootKind::ALL {
        list.push(Entry { tool: Tool::Loot(Some(kind)), group: Group::Loot, name: kind.name().to_owned(), art: format!("ui/portraits/{}", kind.portrait()), size: None });
    }
    list.push(Entry { tool: Tool::Loot(None), group: Group::Loot, name: "Loot (random)".to_owned(), art: "ui/portraits/LootRandom".to_owned(), size: None });
    list
}

impl Editor {
    /// The left panel: search, filters, the catalog in groups – then "Placing" and the eraser.
    pub(super) fn objects_module(&mut self, ui: &mut Ui, world: &World) {
        if self.catalog.is_empty() {
            self.catalog = catalog(&world.look);
        }
        ui.add(TextEdit::singleline(&mut self.catalog_search).hint_text("Search all kinds...").desired_width(f32::INFINITY));
        ui.horizontal_wrapped(|ui| {
            for (chip, text) in Chip::ALL {
                ui.selectable_value(&mut self.chip, chip, text);
            }
        });
        ui.separator();
        let search = self.catalog_search.trim().to_lowercase();
        // The list out of `self` for a moment: its entries are read while `self` changes (the tool in hand, the
        // thumbnails) – `take` leaves an empty list behind, and it's put back below.
        let entries = std::mem::take(&mut self.catalog);
        let mut found = 0;
        for group in Group::ALL {
            if self.chip != Chip::All && group.chip() != self.chip {
                continue;
            }
            let shown: Vec<&Entry> = entries.iter().filter(|e| e.group == group && (search.is_empty() || e.name.to_lowercase().contains(&search))).collect();
            if shown.is_empty() {
                continue;
            }
            found += shown.len();
            let title = RichText::new(format!("{} ({})", group.name(), shown.len())).strong();
            // Every group starts folded – a search or a filter opens what it found.
            let mut header = CollapsingHeader::new(title).id_salt(("catalog", group.name())).default_open(false);
            if !search.is_empty() || self.chip != Chip::All {
                header = header.open(Some(true));
            }
            header.show(ui, |ui| {
                ui.label(RichText::new(group.hint()).small().weak());
                for entry in shown {
                    self.catalog_item(ui, entry);
                }
            });
        }
        self.catalog = entries;
        if found == 0 {
            ui.label(RichText::new("Nothing is called like that.").weak());
        }
        ui.separator();
        section_from(ui, "Placing", Some(Origin::Sc2), "whose the next building or unit is", |ui| {
            ui.horizontal(|ui| {
                ui.label("Owner");
                for (owner, text, hint) in OWNERS {
                    ui.selectable_value(&mut self.place_owner, owner, text).on_hover_text(hint);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Position");
                ui.selectable_value(&mut self.place_on_grid, false, "Free").on_hover_text("Units and loot go exactly where the mouse points");
                ui.selectable_value(&mut self.place_on_grid, true, "Grid").on_hover_text("Units and loot go onto the middle of the tile under the mouse");
            });
            ui.label(RichText::new("Shift: keep it in hand for the next one - R turns a gate - drag: a line of buildings.").small().weak());
        });
        section(ui, "Remove", "hold and drag (brush)", |ui| self.tool_button(ui, Tool::Remove));
        self.brush_size(ui, false);
    }

    /// One line of the catalog: picture and name, lit while it's in hand – a click takes it, another puts it away.
    /// Drawn by hand: `allocate_exact_size` takes the line's room and says what the mouse does there, the painter
    /// then fills it.
    fn catalog_item(&mut self, ui: &mut Ui, entry: &Entry) {
        let held = self.holds(entry.tool);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ITEM_H), Sense::click());
        // The colours egui gives a button in this state (hovered, pressed, selected).
        let visuals = ui.style().interact_selectable(&response, held);
        if held || response.hovered() {
            ui.painter().rect_filled(rect, 3.0, visuals.weak_bg_fill);
        }
        let icon = egui::Rect::from_min_size(rect.min + egui::vec2(4.0, (ITEM_H - ICON) / 2.0), egui::Vec2::splat(ICON));
        self.thumbs.paint(ui, icon, &entry.art, &entry.name);
        let at = egui::pos2(icon.max.x + 7.0, rect.center().y);
        ui.painter().text(at, Align2::LEFT_CENTER, &entry.name, TextStyle::Body.resolve(ui.style()), visuals.text_color());
        let hint = match entry.size {
            Some((w, h)) => format!("{} - {w} x {h} tiles", entry.name),
            None => entry.name.clone(),
        };
        if response.on_hover_text(hint).clicked() {
            self.toggle_tool(entry.tool);
        }
    }

    /// The order that puts a building or unit down, for the owner "Placing" says: the colony's as it is, a neutral
    /// one wrapped (`Order::PlaceAs`).
    pub fn owned(&self, order: Order) -> Order {
        match self.place_owner {
            Owner::Colony => order,
            owner => Order::PlaceAs { owner, order: Box::new(order) },
        }
    }

    /// The panel below the map: TAB's list of the entities created, as a table.
    pub(super) fn placed_table(&mut self, ui: &mut Ui, world: &World) {
        let rows = placed(world, self.filter, &self.placed_search.trim().to_lowercase());
        ui.horizontal(|ui| {
            ui.label(RichText::new("Placed").strong());
            ui.label(RichText::new(rows.len().to_string()).weak());
            ui.separator();
            for (filter, text, hint) in [
                (Filter::All, "All", "Buildings, units, neutral things, houses of doom, loot"),
                (Filter::Buildings, "Buildings", "The colony's"),
                (Filter::Units, "Units", "The colony's"),
                (Filter::Neutral, "Neutral", "Nobody's - and the ones that join"),
                (Filter::Tagged, "Tagged", "With a name ID, a name, a text or a mark"),
                (Filter::Scripted, "With script", "Neutral things that run a script when they join"),
            ] {
                ui.selectable_value(&mut self.filter, filter, text).on_hover_text(hint);
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.add(TextEdit::singleline(&mut self.placed_search).hint_text("Find kind, name or ID...").desired_width(190.0));
            });
        });
        // No gap between the lines: a click between two rows would hit none.
        ui.spacing_mut().item_spacing.y = 0.0;
        table_row(ui, COLUMNS.map(|(heading, _)| heading), false, true);
        ui.separator();
        // `show_rows`: only the lines in view are made – a map has thousands of things.
        ScrollArea::vertical().id_salt("placed rows").auto_shrink([false, false]).show_rows(ui, ROW_H, rows.len(), |ui, range| {
            for &row in &rows[range] {
                let cells = cells(world, row);
                let tile = format!("{}, {}", cells.tile.0, cells.tile.1);
                let selected = self.selection.contains(&cells.piece);
                let response = table_row(ui, [&cells.kind, &cells.name_id, &tile, cells.owner, &cells.life, &cells.scripts], selected, false);
                if response.clicked() {
                    // Shift or Ctrl: in or out of what's selected already.
                    let add = ui.input(|input| input.modifiers.shift || input.modifiers.command);
                    self.select = Some((vec![cells.piece], add));
                }
                if response.double_clicked() {
                    self.request = Some(EditorRequest::Locate(cells.at));
                }
            }
        });
    }

    /// The right panel's part for one selected pile of loot: what it holds – kind and amount –, how many zombies are
    /// hidden around it, and the name ID triggers know it by ("When it is picked up"). `false`: no pile is selected.
    pub(super) fn loot_section(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) -> bool {
        let piles: Vec<u32> = self.selection.iter().filter_map(|p| if let Piece::Loot(id) = *p { world.find_loot(id).map(|_| id) } else { None }).collect();
        let [id] = piles[..] else {
            return !piles.is_empty();
        };
        let Some(pile) = world.find_loot(id).map(|l| &world.loot[l]) else {
            return false;
        };
        let art = format!("ui/portraits/{}", pile.kind.portrait());
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(44.0), Sense::hover());
            self.thumbs.paint(ui, rect, &art, pile.kind.name());
            ui.vertical(|ui| {
                ui.label(RichText::new(format!("Loot: {}", pile.kind.name())).strong().size(17.0));
                ui.label(RichText::new(format!("#{id} - tile {}, {}", pile.pos.x.floor() as i32, pile.pos.y.floor() as i32)).small().weak());
            });
        });
        section_from(ui, "Loot", Some(Origin::New), "what lies there, and who guards it", |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Locate").clicked() {
                    self.request = Some(EditorRequest::Locate(pile.pos));
                }
                if ui.button("Delete").clicked() {
                    orders.push(Order::RemovePieces { pieces: vec![Piece::Loot(id)] });
                    self.select = Some((Vec::new(), false));
                }
            });
            let set = |kind: LootKind, amount: u32, guards: u32, name_id: &str| Order::SetLoot { id, kind, amount, guards, name_id: name_id.to_owned() };
            let mut kind = pile.kind;
            egui::ComboBox::from_id_salt(("loot kind", id)).selected_text(kind.name()).show_ui(ui, |ui| {
                for k in LootKind::ALL {
                    ui.selectable_value(&mut kind, k, k.name());
                }
            });
            if kind != pile.kind {
                // The usual amount of the new kind, unless the amount was changed by hand.
                let amount = if pile.amount == pile.kind.usual_amount() { kind.usual_amount() } else { pile.amount };
                orders.push(set(kind, amount, pile.guards, &pile.name_id));
            }
            egui::Grid::new(("loot values", id)).num_columns(2).show(ui, |ui| {
                ui.label("Amount").on_hover_text("Gold, goods - or what the command center makes more per payout, for a supply");
                if let Some(amount) = settled(ui, pile.amount, |v| DragValue::new(v).range(1..=1_000_000).speed(5)) {
                    orders.push(set(pile.kind, amount, pile.guards, &pile.name_id));
                }
                ui.end_row();
                ui.label("Guards").on_hover_text("Zombies hidden around it - they come out when a unit comes close");
                if let Some(guards) = settled(ui, pile.guards, |v| DragValue::new(v).range(0..=500)) {
                    orders.push(set(pile.kind, pile.amount, guards, &pile.name_id));
                }
                ui.end_row();
                ui.label("Name ID").on_hover_text("The name the triggers know it by");
                if let Some(text) = settled_text(ui, &pile.name_id, false) {
                    orders.push(set(pile.kind, pile.amount, pile.guards, &text));
                }
                ui.end_row();
            });
            if pile.name_id.is_empty() {
                ui.label(RichText::new("Give it a name ID to attach triggers.").small().weak());
            } else {
                let picked = TriggerEvent::PickedUp { name_id: pile.name_id.clone() };
                crate::story_panel::linked_triggers(ui, egui::Id::new(("loot triggers", id)), world, orders, &picked, "When it is picked up");
            }
        });
        true
    }

    /// The selected buildings, units and neutral things that are (still) on the map.
    pub(super) fn things(&self, world: &World) -> Vec<ThingRef> {
        let thing = |piece: &Piece| match *piece {
            Piece::Building(id) => Some(ThingRef::Building(id)),
            Piece::Unit(id) => Some(ThingRef::Unit(id)),
            Piece::Neutral(id) => Some(ThingRef::Neutral(id)),
            _ => None,
        };
        self.selection.iter().filter_map(thing).filter(|&t| world.thing_center(t).is_some()).collect()
    }

    /// "Combat values": the selected units' and buildings' own hit points, damage, attack speed, range, sight, speed and armor,
    /// as percents of their kind's (`sim/tweak.rs`, `Order::SetTweak`) – a change goes to all of them. A map's values for a whole
    /// kind are in the Data module.
    fn combat_section(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>, things: &[ThingRef]) {
        let units: Vec<(ThingRef, Tweak)> = things
            .iter()
            .filter_map(|&thing| match thing {
                ThingRef::Unit(id) => world.find_unit(id).map(|i| (thing, world.units[i].tweak)),
                ThingRef::Building(id) => world.find_building(id).map(|i| (thing, world.buildings[i].tweak)),
                ThingRef::Neutral(_) => None,
            })
            .collect();
        if units.is_empty() {
            return;
        }
        // The field's name, where it is in a `Tweak`, and what it does.
        type Get = fn(&Tweak) -> &u16;
        type Put = fn(&mut Tweak) -> &mut u16;
        const FIELDS: [(&str, Get, Put, &str); 5] = [
            ("Damage", |t| &t.damage, |t| &mut t.damage, "Damage of every shot"),
            ("Attack speed", |t| &t.attack_speed, |t| &mut t.attack_speed, "150 % shoots one and a half times as often: all the times of the weapon get shorter"),
            ("Range", |t| &t.range, |t| &mut t.range, "How far it shoots"),
            ("Sight", |t| &t.sight, |t| &mut t.sight, "How far it sees"),
            ("Speed", |t| &t.speed, |t| &mut t.speed, "How fast it walks"),
        ];
        // What a thing can do: only units walk, only units and towers shoot.
        let walks = units.iter().any(|(thing, _)| matches!(thing, ThingRef::Unit(_)));
        let shoots = units.iter().any(|&(thing, _)| match thing {
            ThingRef::Unit(_) => true,
            ThingRef::Building(id) => world.find_building(id).is_some_and(|i| world.buildings[i].stats().weapon.is_some()),
            ThingRef::Neutral(_) => false,
        });
        // A shield only where a thing has or may have one: buildings (a wall has only a shield).
        let shows_shield = units.iter().any(|&(thing, _)| matches!(thing, ThingRef::Building(_)));
        let title = if units.len() == 1 { "Combat values".to_owned() } else { format!("Combat values ({} things)", units.len()) };
        section_from(ui, &title, Some(Origin::New), "life and armor in points, the rest in percent of what its kind has - 100 changes nothing", |ui| {
            egui::Grid::new("combat values").num_columns(2).show(ui, |ui| {
                // Hit points and shield are numbers: 800 and 200 make a life of 1000. A wall has only a shield; it falls
                // when both are 0. What it has now is shown (its kind's if the editor didn't change it).
                let life = |thing: ThingRef| -> (u32, u32) {
                    match thing {
                        ThingRef::Unit(id) => world.find_unit(id).map_or((0, 0), |i| (world.units[i].max_hp(), 0)),
                        ThingRef::Building(id) => world.find_building(id).map_or((0, 0), |i| (world.buildings[i].max_hp(), world.buildings[i].max_shield())),
                        ThingRef::Neutral(_) => (0, 0),
                    }
                };
                // The kind's own, to write `None` when a field is set back to it.
                let kinds = |thing: ThingRef| -> (u32, u32) {
                    match thing {
                        ThingRef::Unit(id) => world.find_unit(id).map_or((0, 0), |i| (world.units[i].stats().hp, 0)),
                        ThingRef::Building(id) => world.find_building(id).map_or((0, 0), |i| (world.buildings[i].stats().hp, world.buildings[i].stats().max_shield())),
                        ThingRef::Neutral(_) => (0, 0),
                    }
                };
                for (name, shield) in [("Hit points", false), ("Shield", true)] {
                    if shield && !shows_shield {
                        continue;
                    }
                    ui.label(name).on_hover_text(if shield { "A building's shield - zombies hit it first" } else { "Hit points - a wall may have 0 and only a shield" });
                    ui.horizontal(|ui| {
                        let pick = |thing: ThingRef| if shield { life(thing).1 } else { life(thing).0 };
                        let first = pick(units[0].0);
                        if let Some(points) = settled(ui, first, |v| DragValue::new(v).range(0..=MOST_LIFE).speed(5.0)) {
                            for &(thing, mut tweak) in &units {
                                let own = if shield { kinds(thing).1 } else { kinds(thing).0 };
                                let value = (points != own).then_some(points);
                                if shield {
                                    tweak.shield = value;
                                } else {
                                    tweak.hp = value;
                                }
                                orders.push(Order::SetTweak { thing, tweak });
                            }
                        }
                        if units.iter().any(|&(t, _)| pick(t) != first) {
                            ui.label(RichText::new("mixed").weak());
                        }
                    });
                    ui.end_row();
                }
                if shows_shield {
                    ui.label("Life").on_hover_text("Hit points and shield together - it falls when both are 0");
                    ui.label(RichText::new(format!("{} in all", life(units[0].0).0 + life(units[0].0).1)).strong());
                    ui.end_row();
                }
                // Armor is a number of points too: they come off every hit. A building's shield has its own (the hit points
                // keep theirs). What it has now is shown (its kind's if the editor didn't change it) – upgrades come on top.
                let armors = |thing: ThingRef, kind: bool| -> (u32, u32) {
                    match thing {
                        ThingRef::Unit(id) => world.find_unit(id).map_or((0, 0), |i| {
                            let u = &world.units[i];
                            (if kind { u.stats().armor } else { u.tweak.armor_of(u.stats().armor) }, 0)
                        }),
                        ThingRef::Building(id) => world.find_building(id).map_or((0, 0), |i| {
                            let b = &world.buildings[i];
                            let s = b.stats();
                            if kind { (s.armor, s.shield_armor) } else { (b.tweak.armor_of(s.armor), b.tweak.shield_armor_of(s.shield_armor)) }
                        }),
                        ThingRef::Neutral(_) => (0, 0),
                    }
                };
                for (name, shield) in [("Armor", false), ("Shield armor", true)] {
                    if shield && !shows_shield {
                        continue;
                    }
                    ui.label(name).on_hover_text(if shield {
                        "Armor points of the shield: they come off every hit that lands on it"
                    } else {
                        "Armor points: they come off every hit (at least 1 gets through)"
                    });
                    ui.horizontal(|ui| {
                        let pick = |thing: ThingRef| if shield { armors(thing, false).1 } else { armors(thing, false).0 };
                        let first = pick(units[0].0);
                        if let Some(points) = settled(ui, first, |v| DragValue::new(v).range(0..=MOST_ARMOR).speed(0.2)) {
                            for &(thing, mut tweak) in &units {
                                let own = if shield { armors(thing, true).1 } else { armors(thing, true).0 };
                                let value = (points != own).then_some(points);
                                if shield {
                                    tweak.shield_armor = value;
                                } else {
                                    tweak.armor = value;
                                }
                                orders.push(Order::SetTweak { thing, tweak });
                            }
                        }
                        if units.iter().any(|&(t, _)| pick(t) != first) {
                            ui.label(RichText::new("mixed").weak());
                        }
                    });
                    ui.end_row();
                }
                for (name, get, put, hint) in FIELDS {
                    if (name == "Speed" && !walks) || (matches!(name, "Damage" | "Attack speed" | "Range") && !shoots) {
                        continue;
                    }
                    ui.label(name).on_hover_text(hint);
                    ui.horizontal(|ui| {
                        let first = *get(&units[0].1);
                        if let Some(percent) = settled(ui, first, |v| DragValue::new(v).range(PERCENT).suffix(" %")) {
                            for &(thing, mut tweak) in &units {
                                *put(&mut tweak) = percent;
                                orders.push(Order::SetTweak { thing, tweak });
                            }
                        }
                        if units.iter().any(|(_, t)| get(t) != get(&units[0].1)) {
                            ui.label(RichText::new("mixed").weak());
                        }
                    });
                    ui.end_row();
                }
            });
            if units.iter().any(|(_, t)| !t.is_default()) && ui.button("Back to its kind's values").clicked() {
                orders.extend(units.iter().map(|&(thing, _)| Order::SetTweak { thing, tweak: Tweak::default() }));
            }
        });
    }

    /// The right panel's part for placed things: one selected – all its properties; several – the ones they share.
    /// `false`: none is selected.
    pub(super) fn things_section(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) -> bool {
        let things = self.things(world);
        match things.as_slice() {
            [] => return false,
            [thing] => self.object_section(ui, world, orders, *thing),
            many => self.many_section(ui, world, orders, many),
        }
        self.combat_section(ui, world, orders, &things);
        true
    }

    /// TAB's entity properties for one thing (`sim/neutral.rs`): locate, clone (its kind into the hand), delete; its
    /// owner – the colony, neutral, or joining the colony when a unit comes close –, its hit points, infected or
    /// veteran, and its tag: the name scripts know it by, a name and a text the player sees, a mark on the minimap.
    fn object_section(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>, thing: ThingRef) {
        let facts = describe(world, thing);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(44.0), Sense::hover());
            self.thumbs.paint(ui, rect, &facts.art, facts.name);
            ui.vertical(|ui| {
                ui.label(RichText::new(facts.name).strong().size(17.0));
                let size = facts.size.map_or(String::new(), |(w, h)| format!(" - {w} x {h}"));
                ui.label(RichText::new(format!("#{}{size} - tile {}, {}", facts.id, facts.tile.0, facts.tile.1)).small().weak());
            });
        });
        section_from(ui, "Object", Some(Origin::Tab), "", |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Locate").clicked() {
                    self.request = Some(EditorRequest::Locate(facts.at));
                }
                if ui.button("Clone").on_hover_text("Its kind into the hand - click to place another").clicked() {
                    self.clone_kind(facts.clone);
                }
                if ui.button("Delete").clicked() {
                    orders.push(Order::RemoveThing { thing });
                    self.select = Some((Vec::new(), false));
                }
            });
            ui.horizontal(|ui| {
                ui.label("Owner");
                for (owner, text, hint) in OWNERS {
                    if ui.selectable_label(facts.owner == owner, text).on_hover_text(hint).clicked() && facts.owner != owner {
                        self.hand_over(world, orders, std::slice::from_ref(&facts), owner);
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Life");
                if let Some(percent) = settled(ui, facts.percent, |v| DragValue::new(v).range(1..=100).suffix(" %")) {
                    orders.push(Order::SetHealth { thing, percent });
                }
                if let Some((text, on)) = facts.mark {
                    let mut now = on;
                    if ui.checkbox(&mut now, text).changed() {
                        orders.push(Order::SetMark { thing, on: now });
                    }
                }
            });
            let tag = world.tag_of(thing).cloned().unwrap_or_default();
            let mut changed = tag.clone();
            egui::Grid::new(("tag", thing_key(thing))).num_columns(2).show(ui, |ui| {
                ui.label("Name ID").on_hover_text("The name the scripts know it by");
                if let Some(text) = settled_text(ui, &tag.name_id, false) {
                    changed.name_id = text;
                }
                ui.end_row();
                ui.label("Name").on_hover_text("Shown instead of its kind's name");
                if let Some(text) = settled_text(ui, &tag.name, false) {
                    changed.name = text;
                }
                ui.end_row();
            });
            ui.label("Text");
            if let Some(text) = settled_text(ui, &tag.text, true) {
                changed.text = text;
            }
            ui.checkbox(&mut changed.minimap, "Mark on the minimap");
            if changed != tag {
                orders.push(Order::SetTag { thing, tag: changed });
            }
            // A neutral one: its triggers when it joins (TAB's "Script On Join") – they know it by its name ID.
            if let ThingRef::Neutral(id) = thing {
                if tag.name_id.is_empty() {
                    ui.label(RichText::new("Give it a name ID to attach triggers.").small().weak());
                } else {
                    let joins = TriggerEvent::Join { name_id: tag.name_id.clone() };
                    crate::story_panel::linked_triggers(ui, egui::Id::new(("join triggers", id)), world, orders, &joins, "When it joins");
                }
            }
        });
    }

    /// Several things selected (SC2's editor does the same): what they all have – owner, life, infected or veteran,
    /// the mark on the minimap – and every change goes to each of them, as one step of undo. A name ID is one thing's
    /// alone, so it isn't here.
    fn many_section(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>, things: &[ThingRef]) {
        let facts: Vec<Facts> = things.iter().map(|&thing| describe(world, thing)).collect();
        let count = |wanted: fn(&ThingRef) -> bool| things.iter().filter(|t| wanted(t)).count();
        let buildings = count(|t| matches!(t, ThingRef::Building(_)));
        let units = count(|t| matches!(t, ThingRef::Unit(_)));
        let neutral = things.len() - buildings - units;
        section_from(ui, "Objects", Some(Origin::Sc2), "a change goes to all of them", |ui| {
            ui.label(RichText::new(format!("{} selected", things.len())).strong().size(17.0));
            ui.label(RichText::new(format!("{buildings} buildings, {units} units, {neutral} neutral")).small().weak());
            if ui.button("Delete all").clicked() {
                orders.push(Order::RemovePieces { pieces: things.iter().map(|&thing| piece_of(thing)).collect() });
                self.select = Some((Vec::new(), false));
            }
            ui.horizontal(|ui| {
                ui.label("Owner");
                // Lit only if all of them have the same.
                let common = facts.iter().all(|f| f.owner == facts[0].owner).then_some(facts[0].owner);
                for (owner, text, hint) in OWNERS {
                    if ui.selectable_label(common == Some(owner), text).on_hover_text(hint).clicked() && common != Some(owner) {
                        self.hand_over(world, orders, &facts, owner);
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Life");
                if let Some(percent) = settled(ui, facts[0].percent, |v| DragValue::new(v).range(1..=100).suffix(" %")) {
                    orders.extend(things.iter().map(|&thing| Order::SetHealth { thing, percent }));
                }
                if facts.iter().any(|f| f.percent != facts[0].percent) {
                    ui.label(RichText::new("mixed").weak());
                }
            });
            // "Infected" for the buildings among them, "Veteran" for the units – ticked if all of those are.
            for text in ["Infected", "Veteran"] {
                let with: Vec<(ThingRef, bool)> = facts.iter().filter_map(|f| f.mark.filter(|&(name, _)| name == text).map(|(_, on)| (f.thing, on))).collect();
                if with.is_empty() {
                    continue;
                }
                let mut on = with.iter().all(|&(_, on)| on);
                if ui.checkbox(&mut on, format!("{text} ({})", with.len())).changed() {
                    orders.extend(with.iter().map(|&(thing, _)| Order::SetMark { thing, on }));
                }
            }
            let tags: Vec<Tag> = things.iter().map(|&thing| world.tag_of(thing).cloned().unwrap_or_default()).collect();
            let mut marked = tags.iter().all(|tag| tag.minimap);
            if ui.checkbox(&mut marked, "Mark on the minimap").changed() {
                for (&thing, tag) in things.iter().zip(tags) {
                    orders.push(Order::SetTag { thing, tag: Tag { minimap: marked, ..tag } });
                }
            }
        });
    }

    /// These things belong to `owner` from now on – and stay selected. A thing that changes hands moves to another
    /// list of the world and gets a new number there: the one the world gives next (`World::next_ids`), counted on
    /// for each of them in the order of the orders.
    fn hand_over(&mut self, world: &World, orders: &mut Vec<Order>, things: &[Facts], owner: Owner) {
        let (mut building, mut unit, mut neutral) = world.next_ids();
        // Hands out the number and counts on – `&mut u32`: the counter lent to change it.
        let next = |counter: &mut u32| {
            *counter += 1;
            *counter - 1
        };
        let mut selected = Vec::new();
        for facts in things {
            if facts.owner != owner {
                orders.push(Order::SetOwner { thing: facts.thing, owner });
            }
            selected.push(piece_of(match (facts.thing, owner) {
                (thing, _) if facts.owner == owner => thing,
                (ThingRef::Neutral(id), Owner::Neutral | Owner::Joinable) => ThingRef::Neutral(id),
                (_, Owner::Neutral | Owner::Joinable) => ThingRef::Neutral(next(&mut neutral)),
                (ThingRef::Neutral(_), Owner::Colony) if matches!(facts.clone, Copies::Building(_)) => ThingRef::Building(next(&mut building)),
                (ThingRef::Neutral(_), Owner::Colony) => ThingRef::Unit(next(&mut unit)),
                (other, Owner::Colony) => other,
            }));
        }
        self.select = Some((selected, false));
    }

    /// "Clone": the thing's kind into the hand.
    fn clone_kind(&mut self, kind: Copies) {
        match kind {
            Copies::Building(kind) => {
                self.tool = None;
                self.place = Some(Some(kind));
            }
            Copies::Unit(kind) => {
                self.place = Some(None);
                self.tool = Some(Tool::Unit(kind));
            }
        }
    }
}

/// One line of the table: its six texts at the columns' places, each cut off where the next begins. `heading`: the
/// line of the headings – grey, and nothing to click.
fn table_row(ui: &mut Ui, texts: [&str; 6], selected: bool, heading: bool) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, ROW_H), if heading { Sense::hover() } else { Sense::click() });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let visuals = ui.style().interact_selectable(&response, selected);
    if !heading && (selected || response.hovered()) {
        ui.painter().rect_filled(rect, 2.0, visuals.weak_bg_fill);
    }
    let color: Color32 = if heading { ui.visuals().weak_text_color() } else { visuals.text_color() };
    let font = TextStyle::Body.resolve(ui.style());
    for (k, text) in texts.iter().enumerate() {
        let left = rect.min.x + COLUMNS[k].1 * width + 6.0;
        let right = COLUMNS.get(k + 1).map_or(rect.max.x, |next| rect.min.x + next.1 * width);
        let cell = egui::Rect::from_min_max(egui::pos2(left, rect.min.y), egui::pos2(right - 4.0, rect.max.y));
        // `with_clip_rect`: a painter that only draws inside the cell.
        ui.painter().with_clip_rect(cell.intersect(ui.clip_rect())).text(egui::pos2(left, rect.center().y), Align2::LEFT_CENTER, *text, font.clone(), color);
    }
    response
}

/// A line of the table: which thing, by its place in its list of the world (only for this frame).
#[derive(Clone, Copy)]
enum Row {
    Building(usize),
    Unit(usize),
    Neutral(usize),
    Doom(usize),
    Loot(usize),
}

/// The lines the table shows: everything placed that passes the filter and the search (lower case; empty: all).
fn placed(world: &World, filter: Filter, search: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    if matches!(filter, Filter::All | Filter::Buildings | Filter::Tagged) {
        rows.extend((0..world.buildings.len()).map(Row::Building));
    }
    if matches!(filter, Filter::All | Filter::Units | Filter::Tagged) {
        rows.extend((0..world.units.len()).map(Row::Unit));
    }
    if matches!(filter, Filter::All | Filter::Neutral | Filter::Tagged | Filter::Scripted) {
        rows.extend((0..world.neutral.len()).map(Row::Neutral));
    }
    if filter == Filter::All {
        rows.extend((0..world.doom.len()).map(Row::Doom));
        rows.extend((0..world.loot.len()).map(Row::Loot));
    }
    let tag = |row: Row| match row {
        Row::Building(b) => world.buildings[b].tag.as_deref(),
        Row::Unit(u) => world.units[u].tag.as_deref(),
        Row::Neutral(n) => world.neutral[n].tag.as_deref(),
        Row::Doom(_) | Row::Loot(_) => None,
    };
    let found = |row: Row| {
        let in_tag = tag(row).is_some_and(|t| t.name_id.to_lowercase().contains(search) || t.name.to_lowercase().contains(search));
        in_tag || kind_name(world, row).to_lowercase().contains(search)
    };
    rows.retain(|&row| {
        let passes = match filter {
            Filter::Tagged => tag(row).is_some(),
            Filter::Scripted => matches!(row, Row::Neutral(n) if join_triggers(world, &world.neutral[n]) > 0),
            _ => true,
        };
        passes && (search.is_empty() || found(row))
    });
    rows
}

/// How many triggers fire when this neutral thing joins (they know it by its name ID).
fn join_triggers(world: &World, n: &Neutral) -> usize {
    let Some(tag) = n.tag.as_ref().filter(|t| !t.name_id.is_empty()) else {
        return 0;
    };
    world.story.triggers.iter().filter(|t| matches!(&t.event, TriggerEvent::Join { name_id } if *name_id == tag.name_id)).count()
}

/// The name of a line's kind: "Tent", "Soldier", "House of doom", "Loot".
fn kind_name(world: &World, row: Row) -> &str {
    match row {
        Row::Building(b) => &world.buildings[b].stats().name,
        Row::Unit(u) => &world.units[u].stats().name,
        Row::Neutral(n) => match world.neutral[n].thing {
            Thing::Building { kind, .. } => &kind.stats().name,
            Thing::Unit { kind, .. } => &kind.stats().name,
        },
        Row::Doom(_) => "House of doom",
        Row::Loot(_) => "Loot",
    }
}

/// What a line of the table shows – made only for the lines in view.
struct Cells {
    piece: Piece,
    kind: String,
    name_id: String,
    /// Where the camera goes on a double click, and the tile shown.
    at: Vec2,
    tile: (i32, i32),
    owner: &'static str,
    life: String,
    scripts: String,
}

fn cells(world: &World, row: Row) -> Cells {
    let name_id = |tag: Option<&Tag>| tag.map_or(String::new(), |t| t.name_id.clone());
    let floor = |p: Vec2| (p.x.floor() as i32, p.y.floor() as i32);
    let percent = |hp: u32, full: u32| format!("{} %", (hp * 100 / full.max(1)).clamp(1, 100));
    let kind = kind_name(world, row);
    match row {
        Row::Building(b) => {
            let b = &world.buildings[b];
            let kind = if b.infected { format!("{kind} (infected)") } else { kind.to_owned() };
            Cells { piece: Piece::Building(b.id), kind, name_id: name_id(b.tag.as_deref()), at: b.center(), tile: b.tile, owner: "Colony", life: percent(b.hp, b.max_hp()), scripts: String::new() }
        }
        Row::Unit(u) => {
            let u = &world.units[u];
            let kind = if u.veteran { format!("{kind} (veteran)") } else { kind.to_owned() };
            Cells { piece: Piece::Unit(u.id), kind, name_id: name_id(u.tag.as_deref()), at: u.pos, tile: floor(u.pos), owner: "Colony", life: percent(u.hp, u.max_hp()), scripts: String::new() }
        }
        Row::Neutral(n) => {
            let n = &world.neutral[n];
            let tile = match n.thing {
                Thing::Building { tile, .. } => tile,
                Thing::Unit { pos, .. } => (pos.0.floor() as i32, pos.1.floor() as i32),
            };
            let count = join_triggers(world, n);
            let scripts = if count == 0 { String::new() } else { format!("on join ({count})") };
            let owner = if n.joinable { "Joins" } else { "Neutral" };
            Cells { piece: Piece::Neutral(n.id), kind: kind.to_owned(), name_id: name_id(n.tag.as_deref()), at: n.center(), tile, owner, life: format!("{} %", n.hp_percent), scripts }
        }
        Row::Doom(d) => {
            let d = &world.doom[d];
            let kind = format!("{kind} {} x {}", d.size.0, d.size.1);
            Cells { piece: Piece::Doom(d.id), kind, name_id: String::new(), at: d.center(), tile: d.tile, owner: "Zombies", life: format!("{} hp", d.hp), scripts: format!("nest: {} zombies", d.inside) }
        }
        Row::Loot(l) => {
            let l = &world.loot[l];
            let kind = format!("{kind} - {:?}", l.kind);
            Cells { piece: Piece::Loot(l.id), kind, name_id: l.name_id.clone(), at: l.pos, tile: floor(l.pos), owner: "-", life: "-".to_owned(), scripts: format!("{} in it, {} guards", l.amount, l.guards) }
        }
    }
}

/// What a placed thing's "Clone" puts into the hand.
#[derive(Clone, Copy)]
enum Copies {
    Building(BuildingKind),
    Unit(UnitKind),
}

/// A placed thing as a piece of the selection.
fn piece_of(thing: ThingRef) -> Piece {
    match thing {
        ThingRef::Building(id) => Piece::Building(id),
        ThingRef::Unit(id) => Piece::Unit(id),
        ThingRef::Neutral(id) => Piece::Neutral(id),
    }
}

/// A number per placed thing, for egui to tell its fields apart.
fn thing_key(thing: ThingRef) -> (u8, u32) {
    match thing {
        ThingRef::Building(id) => (0, id),
        ThingRef::Unit(id) => (1, id),
        ThingRef::Neutral(id) => (2, id),
    }
}

/// What the properties of a placed thing start from.
#[derive(Clone)]
pub(super) struct Facts {
    thing: ThingRef,
    /// Its number in its list, its kind's name and picture.
    id: u32,
    pub(super) name: &'static str,
    art: String,
    owner: Owner,
    /// Its hit points in percent.
    percent: u32,
    /// Its mark: the tick box's text and whether it's set – a neutral building has none.
    mark: Option<(&'static str, bool)>,
    /// What "Clone" takes into the hand.
    clone: Copies,
    /// Its middle, its tile, and how many tiles a building covers.
    at: Vec2,
    tile: (i32, i32),
    size: Option<(i32, i32)>,
}

/// Looks a placed thing up. It has to be on the map (`Editor::things` only hands out those).
pub(super) fn describe(world: &World, thing: ThingRef) -> Facts {
    let percent = |hp: u32, full: u32| (hp * 100 / full.max(1)).clamp(1, 100);
    let floor = |p: Vec2| (p.x.floor() as i32, p.y.floor() as i32);
    let building = |kind: BuildingKind| (kind.stats().name.as_str(), format!("ui/portraits/{kind:?}"), Copies::Building(kind));
    let unit = |kind: UnitKind| (kind.stats().name.as_str(), format!("ui/portraits/{kind:?}"), Copies::Unit(kind));
    match thing {
        ThingRef::Building(id) => {
            let b = world.buildings.iter().find(|b| b.id == id).expect("looked up before");
            let (name, art, clone) = building(b.kind);
            let (percent, mark) = (percent(b.hp, b.max_hp()), Some(("Infected", b.infected)));
            Facts { thing, id, name, art, owner: Owner::Colony, percent, mark, clone, at: b.center(), tile: b.tile, size: Some(b.kind.footprint(b.turned)) }
        }
        ThingRef::Unit(id) => {
            let u = world.units.iter().find(|u| u.id == id).expect("looked up before");
            let (name, art, clone) = unit(u.kind);
            let (percent, mark) = (percent(u.hp, u.max_hp()), Some(("Veteran", u.veteran)));
            Facts { thing, id, name, art, owner: Owner::Colony, percent, mark, clone, at: u.pos, tile: floor(u.pos), size: None }
        }
        ThingRef::Neutral(id) => {
            let n = world.neutral.iter().find(|n| n.id == id).expect("looked up before");
            let owner = if n.joinable { Owner::Joinable } else { Owner::Neutral };
            let at = n.center();
            match n.thing {
                Thing::Building { kind, tile, turned } => {
                    let (name, art, clone) = building(kind);
                    Facts { thing, id, name, art, owner, percent: n.hp_percent, mark: None, clone, at, tile, size: Some(kind.footprint(turned)) }
                }
                Thing::Unit { kind, .. } => {
                    let (name, art, clone) = unit(kind);
                    Facts { thing, id, name, art, owner, percent: n.hp_percent, mark: Some(("Veteran", n.veteran)), clone, at, tile: floor(at), size: None }
                }
            }
        }
    }
}
