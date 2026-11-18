//! Bosses (`docs\editor.md`, Zombies module): single zombies with values of their own – more hit points, harder blows,
//! a slower or faster attack, speed, armor (`sim/tweak.rs`). The Boss tool puts one at the click with the values set
//! here; below, the list of the bosses on the map: click one (or click it on the map) to change its values, locate it,
//! delete it. A whole breed's values are the Data module's.

use egui_macroquad::egui::{self, DragValue, RichText, Ui};
use macroquad::math::{vec2, Vec2};

use super::{breed_list_box, section_from, settled, Editor, EditorRequest, Origin, Tool};
use crate::sim::{Breed, Order, Tweak, World, MOST_ARMOR, MOST_LIFE, PERCENT};

/// A boss's values as fields: hit points as a number (the breed's are the start), the rest in percent. `Some`: a field
/// was let go with a new value – the whole new set.
fn boss_fields(ui: &mut Ui, id: &str, breed: Breed, tweak: &Tweak) -> Option<Tweak> {
    let mut now = *tweak;
    let mut changed = false;
    egui::Grid::new(id).num_columns(2).show(ui, |ui| {
        ui.label("Hit points").on_hover_text("As a number - the breed's are the start");
        let kind = breed.stats().hp;
        if let Some(points) = settled(ui, tweak.hp.unwrap_or(kind), |v| DragValue::new(v).range(1..=MOST_LIFE).speed(10.0)) {
            now.hp = (points != kind).then_some(points);
            changed = true;
        }
        ui.end_row();
        ui.label("Armor").on_hover_text("Armor points as a number - they come off every hit; the breed's are the start");
        let armor = breed.stats().armor;
        if let Some(points) = settled(ui, tweak.armor.unwrap_or(armor), |v| DragValue::new(v).range(0..=MOST_ARMOR).speed(0.2)) {
            now.armor = (points != armor).then_some(points);
            changed = true;
        }
        ui.end_row();
        for (name, hint, get) in [
            ("Damage", "Of every blow and shot", (|t: &mut Tweak| &mut t.damage) as fn(&mut Tweak) -> &mut u16),
            ("Attack speed", "150 % hits one and a half times as often", |t| &mut t.attack_speed),
            ("Speed", "How fast it walks", |t| &mut t.speed),
        ] {
            ui.label(name).on_hover_text(hint);
            let mut copy = *tweak;
            let value = *get(&mut copy);
            if let Some(percent) = settled(ui, value, |v| DragValue::new(v).range(PERCENT).suffix(" %")) {
                *get(&mut now) = percent;
                changed = true;
            }
            ui.end_row();
        }
    });
    changed.then_some(now)
}

impl Editor {
    /// The Boss section of the Zombies module: the tool, the breed, the values, and the bosses on the map.
    pub(super) fn boss_section(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        section_from(ui, "Boss", Some(Origin::New), "one zombie with values of its own - click to place", |ui| {
            ui.horizontal(|ui| {
                self.tool_button(ui, Tool::Boss);
                breed_list_box(ui, "boss breed", &mut self.boss_breed, &Breed::ALL);
            });
            if let Some(new) = boss_fields(ui, "boss values", self.boss_breed, &self.boss_tweak) {
                self.boss_tweak = new;
            }
            ui.label(RichText::new("Asleep or storming as set under \"They\". Life and armor as numbers, the rest in percent of the breed's values - 100 changes nothing.").small().weak());
        });
        self.boss_list(ui, world, orders);
    }

    /// Every boss on the map, asleep or awake – where it is, what it is; the picked one with its values to change.
    fn boss_list(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        let mut bosses: Vec<(Vec2, Breed, Tweak, bool)> = Vec::new();
        for z in &world.zombies {
            if let Some(t) = z.boss.as_deref() {
                bosses.push((z.pos, z.breed, *t, false));
            }
        }
        for s in world.sleep.iter().filter(|s| s.boss > 0) {
            bosses.push((s.pos, s.breed, world.bosses[s.boss as usize - 1], true));
        }
        // A boss that is gone can't stay picked.
        if self.boss_pick.is_some_and(|p| !bosses.iter().any(|(at, ..)| at.distance(vec2(p.0, p.1)) < 0.05)) {
            self.boss_pick = None;
        }
        section_from(ui, &format!("Bosses on the map ({})", bosses.len()), Some(Origin::New), "click one to change its values", |ui| {
            if bosses.is_empty() {
                ui.label(RichText::new("None yet.").weak());
            }
            for (at, breed, t, asleep) in &bosses {
                let picked = self.boss_pick.is_some_and(|p| at.distance(vec2(p.0, p.1)) < 0.05);
                ui.horizontal(|ui| {
                    let sleeping = if *asleep { ", asleep" } else { "" };
                    if ui.selectable_label(picked, format!("{} at {}, {}{sleeping}", breed.name(), at.x.floor(), at.y.floor())).clicked() {
                        self.boss_pick = if picked { None } else { Some((at.x, at.y)) };
                    }
                    if ui.small_button("Locate").clicked() {
                        self.request = Some(EditorRequest::Locate(*at));
                    }
                    if ui.small_button("Delete").on_hover_text("Everything on its tile goes").clicked() {
                        orders.push(Order::RemoveAt { tiles: vec![(at.x.floor() as i32, at.y.floor() as i32)] });
                    }
                });
                if picked {
                    if let Some(new) = boss_fields(ui, "picked boss", *breed, t) {
                        orders.push(Order::SetBoss { pos: *at, tweak: new });
                    }
                } else {
                    let life = t.hp.map_or("as breed".to_owned(), |h| h.to_string());
                    let armor = t.armor.map_or("as breed".to_owned(), |a| a.to_string());
                    ui.label(RichText::new(format!("life {life}, armor {armor}, damage {} %, attack {} %, speed {} %", t.damage, t.attack_speed, t.speed)).small().weak());
                }
            }
        });
    }
}
