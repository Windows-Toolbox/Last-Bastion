//! The **Map** page (`docs\editor.md`, step 7): a page of its own in three columns, like SC2's Map Settings – it covers
//! the map, so nothing on the map can be clicked while it is open.
//!
//! **Left:** the map's name, TAB's Info (and how the list of custom levels shows it), the world – landscape, grid, the
//! playing field. **Middle:** the round – days, gold, wood, the difficulty – and the players. **Right:** the goals,
//! victory and defeat.
//!
//! Left out because the game has nothing behind them yet: weather and music per map, rules, strings, allied AIs.

use egui_macroquad::egui::{self, Color32, ComboBox, DragValue, RichText, ScrollArea, Ui};

use super::{section, section_from, Editor, Mode, Module, Origin, Tool, MAX_AMOUNT, MAX_DAYS};
use crate::data;
use crate::kinds::Resource;
use crate::sim::{Order, World, LOOKS};

impl Editor {
    /// The whole page – three columns that scroll together.
    pub(super) fn map_page(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        ScrollArea::vertical().id_salt("map page").auto_shrink([false, false]).show(ui, |ui| {
            // `columns`: three equal columns; the closure gets all three `Ui`s at once.
            ui.columns(3, |cols| {
                self.world_column(&mut cols[0], world, orders);
                self.round_column(&mut cols[1], world, orders);
                goals_column(&mut cols[2], world, orders);
            });
        });
    }

    fn world_column(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        section(ui, "Map", "", |ui| {
            ui.horizontal(|ui| {
                ui.label("Name");
                let shown = self.map_name.clone();
                if let Some(name) = super::name_field(ui, &mut self.map_name_edit, &shown) {
                    self.map_name = name;
                }
            });
            ui.label(RichText::new(&self.map_info).weak());
            if let Some(status) = &self.status {
                ui.label(RichText::new(status).weak());
            }
        });
        self.info_section(ui);
        self.preview_section(ui, world);
        section_from(ui, "World", Some(Origin::Sc2), "", |ui| {
            ui.label("Landscape");
            ui.horizontal_wrapped(|ui| {
                // A map without a landscape of its own is drawn as BR.
                let now = if world.look.is_empty() { "BR" } else { world.look.as_str() };
                for (look, name) in LOOKS {
                    if ui.selectable_label(now == look, look).on_hover_text(name).clicked() && now != look {
                        orders.push(Order::SetLook { look: look.to_owned() });
                    }
                }
            });
            let name = LOOKS.iter().find(|(look, _)| *look == world.look).map_or("", |(_, name)| *name);
            ui.label(RichText::new(name).small().weak());
        });
        self.play_section(ui, world, orders);
        self.grid_section(ui, world, orders);
    }

    /// "Shown in Custom levels like this": the card the list of custom levels makes of the map (name, who made it,
    /// how long, how hard, the description) – so it can be judged while it's written.
    fn preview_section(&mut self, ui: &mut Ui, world: &World) {
        section_from(ui, "In the list of custom levels", Some(Origin::Tab), "", |ui| {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(&self.map_name).strong().size(16.0));
                let by = if self.info.author.trim().is_empty() { String::new() } else { format!("by {} - ", self.info.author.trim()) };
                let days = if world.days == 0 { "no set length".to_owned() } else { format!("{} days", world.days) };
                ui.label(RichText::new(format!("{by}{days} - {}", data::get().difficulty(&world.difficulty).name)).weak());
                if !self.info.description.trim().is_empty() {
                    let text: String = self.info.description.trim().chars().take(160).collect();
                    let cut = if self.info.description.trim().chars().count() > 160 { "..." } else { "" };
                    ui.label(format!("{text}{cut}"));
                }
            });
        });
    }

    fn round_column(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        section_from(ui, "Round", Some(Origin::Tab), "", |ui| {
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
        self.difficulty_section(ui, world, orders);
        section_from(ui, "Players", Some(Origin::Sc2), "who takes part", |ui| {
            egui::Grid::new("players").num_columns(3).striped(true).show(ui, |ui| {
                for (name, control, about) in [
                    ("Colony", "Human", "the command center and what you build"),
                    ("Neutral", "none", "stands about; joins the colony when a unit comes near (Objects: Owner)"),
                    ("Zombies", "Game", "the horde, by the events and what lies on the map"),
                ] {
                    ui.label(RichText::new(name).strong());
                    ui.label(control);
                    ui.label(RichText::new(about).small().weak());
                    ui.end_row();
                }
            });
        });
    }

    /// TAB's zombie population, our difficulties: the choice, and what it means. A new choice changes how close each
    /// breed may come and what new villages of doom hold - the zombies already on the map stay, and the events keep
    /// their numbers ("Plan again" in Events makes them anew).
    fn difficulty_section(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        section_from(ui, "Difficulty", Some(Origin::Tab), "TAB's infected population - and more", |ui| {
            let list = &data::get().difficulties.difficulties;
            let now = data::get().difficulty(&world.difficulty);
            ComboBox::from_id_salt("difficulty").width(160.0).selected_text(now.name.as_str()).show_ui(ui, |ui| {
                for d in list {
                    if ui.selectable_label(d.name == now.name, &d.name).on_hover_text(&d.about).clicked() && d.name != now.name {
                        orders.push(Order::SetDifficulty { name: d.name.clone() });
                    }
                }
            });
            ui.label(RichText::new(&now.about).small().weak());
            egui::Grid::new("difficulty facts").num_columns(2).show(ui, |ui| {
                ui.label("Infected population");
                ui.label(format!("{} %", (now.population * 100.0).round()));
                ui.end_row();
                ui.label("Score");
                ui.label(format!("{} %", now.score_percent));
                ui.end_row();
                ui.label("Villages of doom");
                ui.label(format!("{} % as many, {} % as big", now.villages_percent, now.village_houses_percent));
                ui.end_row();
            });
            ui.label(RichText::new("No zombie comes closer to the base at the start than (tiles):").small().weak());
            ui.horizontal_wrapped(|ui| {
                for breed in crate::sim::Breed::ALL {
                    ui.label(RichText::new(format!("{} {}", breed.name(), now.keep_out.of(breed))).small());
                }
            });
            ui.label(RichText::new("The zombies already placed stay. The events change with \"Plan again\" (Events).").small().weak());
        });
    }

    /// "Playing field" on this page: its size by number, and a button to the tool on the map (the Regions module).
    pub(super) fn drag_border_button(&mut self, ui: &mut Ui) {
        if ui.add_enabled(self.mode == Mode::Edit, egui::Button::new("Edit in Regions")).on_hover_text("Drag the border on the map").clicked() {
            self.module = Module::Regions;
            self.take(Tool::PlayArea);
        }
    }
}

/// "Victory / Defeat": the goals, as in the Triggers module - the same list, edited in either.
fn goals_column(ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
    section_from(ui, "Victory / Defeat", Some(Origin::Tab), "all victory goals reached: won - any defeat goal reached: lost", |ui| {
        crate::story_panel::goals_section(ui, world, orders);
    });
    ui.label(RichText::new("Scripts, counters and the command center's button: module Triggers.").small().weak().color(Color32::GRAY));
}
