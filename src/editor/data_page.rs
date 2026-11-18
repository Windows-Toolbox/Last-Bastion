//! The **Data** page (`docs\editor.md`, "Data"): SC2's data editor, light. Every value of `data\` – units, buildings,
//! zombies, sound, loot, the colony, research, trains – can be changed for this map alone (`data::Overrides`, kept in
//! the map file). The standard stays in `data\`; the page shows both and marks what the map changes.
//!
//! **Left:** the kinds, by file (Soldier, Barracks, Walker …). **Middle:** the chosen one's values – every number and
//! yes/no the file has, found by walking its JSON, so a new value in `data\` shows up here without any code. Units,
//! zombies and towers have one **Attack speed** on top that stands for all their times (raise, aim, wind-up, cooldown …)
//! at once. **Right:** everything this map changes, each with a way back.
//!
//! A change is an order (`Order::SetData`) – undo and redo work, and a co-op game plays it too. What already exists
//! keeps what it was made with (a unit's life now, not its maximum).

use egui_macroquad::egui::{self, Color32, RichText, ScrollArea, Ui};
use serde_json::Value;

use super::{section_from, settled, Editor, Origin};
use crate::data::{self, Overrides, OVERRIDABLE};
use crate::sim::{Order, World};

/// What the Data page keeps between frames.
#[derive(Default)]
pub struct DataPage {
    /// The kind shown, as a key prefix: `units.soldier`; a file alone (`colony`) shows its loose values.
    pick: String,
    search: String,
    /// The standard values, by file – read once (`data::base_json`), not every frame.
    tree: Vec<(&'static str, Value)>,
    /// "Reset all" asked once – a second click does it.
    confirm_clear: bool,
}

/// The times of a weapon, in the order they are listed: scaled together by "Attack speed".
const TIMES: [&str; 7] = ["raise_seconds", "lower_seconds", "aim_seconds", "windup_seconds", "cooldown_seconds", "attack_seconds", "reload_seconds"];
/// The colour of a value this map changes.
const CHANGED: Color32 = Color32::from_rgb(250, 190, 90);

impl Editor {
    pub(super) fn data_page(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        let page = &mut self.data_page;
        if page.tree.is_empty() {
            page.tree = OVERRIDABLE.iter().filter_map(|file| data::base_json(file).map(|t| (*file, t))).collect();
        }
        if page.pick.is_empty() {
            page.pick = "units.soldier".to_owned();
        }
        egui::SidePanel::left("data kinds").default_width(240.0).width_range(180.0..=420.0).show_inside(ui, |ui| {
            ui.add(egui::TextEdit::singleline(&mut page.search).hint_text("Search").desired_width(f32::INFINITY));
            ScrollArea::vertical().id_salt("data kinds").auto_shrink([false, false]).show(ui, |ui| kinds(ui, page, world));
        });
        egui::SidePanel::right("data changes").default_width(320.0).width_range(220.0..=520.0).show_inside(ui, |ui| {
            ScrollArea::vertical().id_salt("data changes").auto_shrink([false, false]).show(ui, |ui| changes(ui, page, world, orders));
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ScrollArea::vertical().id_salt("data values").auto_shrink([false, false]).show(ui, |ui| values(ui, page, world, orders));
        });
    }
}

/// The text of a value's name: `cooldown_seconds` → "cooldown (s)".
fn pretty(key: &str) -> String {
    match key.strip_suffix("_seconds") {
        Some(rest) => format!("{} (s)", rest.replace('_', " ")),
        None => key.replace('_', " "),
    }
}

/// What a kind is called in the list: its `name` (or `id`) if it has one, else its key.
fn label_of(key: &str, value: &Value) -> String {
    value.get("name").or_else(|| value.get("id")).and_then(Value::as_str).map_or_else(|| pretty(key), str::to_owned)
}

/// The kinds of one file: `(key prefix, label)` – the objects at its top (and the objects in a list at its top), and
/// "General" for the loose values.
fn entries(file: &str, root: &Value) -> Vec<(String, String)> {
    let mut list = Vec::new();
    let Some(map) = root.as_object() else {
        return list;
    };
    if file == "upgrades" || map.values().any(|v| v.is_number() || v.is_boolean() || v.as_array().is_some_and(|a| a.iter().all(|x| x.is_number()))) {
        list.push((file.to_owned(), match file { "research" => "All researches", "upgrades" => "Upgrade levels", _ => "General" }.to_owned()));
    }
    for (key, value) in map {
        match value {
            Value::Object(_) => list.push((format!("{file}.{key}"), label_of(key, value))),
            Value::Array(items) if items.iter().all(Value::is_object) && !items.is_empty() => {
                for (i, item) in items.iter().enumerate() {
                    list.push((format!("{file}.{key}.{i}"), label_of(&format!("{key} {i}"), item)));
                }
            }
            _ => {}
        }
    }
    list
}

fn kinds(ui: &mut Ui, page: &mut DataPage, world: &World) {
    let search = page.search.trim().to_lowercase();
    let lists: Vec<(&str, Vec<(String, String)>)> = page.tree.iter().map(|(file, root)| (*file, entries(file, root))).collect();
    for (file, list) in lists {
        let list: Vec<(String, String)> = list.into_iter().filter(|(_, label)| search.is_empty() || label.to_lowercase().contains(&search)).collect();
        if list.is_empty() {
            continue;
        }
        let title = pretty(file);
        let title = format!("{}{}", title[..1].to_uppercase(), &title[1..]);
        egui::CollapsingHeader::new(RichText::new(title).strong()).id_salt(("data file", file)).default_open(file == "units" || !search.is_empty()).show(ui, |ui| {
            for (key, label) in list {
                let changed = world.overrides.under(&key).count();
                let text = if changed > 0 { format!("{label}  ({changed})") } else { label };
                let text = if changed > 0 { RichText::new(text).color(CHANGED) } else { RichText::new(text) };
                if ui.selectable_label(page.pick == key, text).clicked() {
                    page.pick = key;
                }
            }
        });
    }
}

/// The tree at `key` (`units.soldier`, `research.techs.3`, a file alone: its root).
fn node_at<'a>(page: &'a DataPage, key: &str) -> Option<&'a Value> {
    let mut parts = key.split('.');
    let file = parts.next()?;
    let mut at = &page.tree.iter().find(|(name, _)| *name == file)?.1;
    for part in parts {
        at = match at {
            Value::Object(map) => map.get(part)?,
            Value::Array(list) => list.get(part.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(at)
}

fn values(ui: &mut Ui, page: &mut DataPage, world: &World, orders: &mut Vec<Order>) {
    let key = page.pick.clone();
    let Some(node) = node_at(page, &key) else {
        ui.label(RichText::new("Choose a kind on the left.").weak());
        return;
    };
    let is_file = !key.contains('.');
    let title = if is_file { format!("{} - general values", pretty(&key)) } else { label_of(key.rsplit('.').next().unwrap_or(&key), node) };
    ui.horizontal(|ui| {
        ui.heading(title);
        ui.label(RichText::new(&key).small().weak());
    });
    ui.label(
        RichText::new("What a value is called is its name in the file (data\\README.md says what each does). Orange: this map changes it - the standard is in the tooltip.")
            .small()
            .weak(),
    );
    let used = world.overrides.under(&key).count();
    if used > 0 && ui.button(format!("Back to standard ({used})")).on_hover_text("Every value of this kind").clicked() {
        for (k, _) in world.overrides.under(&key) {
            orders.push(Order::SetData { key: k.clone(), value: None });
        }
    }
    ui.add_space(4.0);
    if key == "research" {
        research_table(ui, world, orders, node);
    }
    if key == "upgrades" {
        upgrade_table(ui, world, orders, node);
    }
    node_values(ui, world, orders, &key, node, is_file, 0);
    // What the others of this kind have and this one leaves out – a map may give it a value.
    let missing = key.rsplit_once('.').map(|(parent, _)| left_out(page, parent, node)).unwrap_or_default();
    if !missing.is_empty() {
        ui.add_space(4.0);
        egui::CollapsingHeader::new(RichText::new(format!("Not set here ({})", missing.len())).weak()).id_salt(("data missing", &key)).default_open(false).show(ui, |ui| {
            for (name, zero) in &missing {
                value_row(ui, world, orders, &format!("{key}.{name}"), &pretty(name), zero, true);
            }
        });
    }
}

/// The values of one object (or list): numbers and yes/no as fields, objects and lists as folding parts.
fn node_values(ui: &mut Ui, world: &World, orders: &mut Vec<Order>, key: &str, node: &Value, loose_only: bool, depth: usize) {
    match node {
        Value::Object(map) => {
            attack_speed(ui, world, orders, key, map);
            for (name, value) in map {
                let sub = format!("{key}.{name}");
                match value {
                    Value::Number(_) | Value::Bool(_) => value_row(ui, world, orders, &sub, &pretty(name), value, false),
                    // The file's top: its objects are kinds of their own (left), only the loose values are here.
                    Value::Object(_) | Value::Array(_) if loose_only && value.is_object() => {}
                    Value::Object(_) => part(ui, world, orders, &sub, &pretty(name), value, depth),
                    Value::Array(items) if items.iter().all(Value::is_number) => part(ui, world, orders, &sub, &pretty(name), value, depth),
                    Value::Array(items) if loose_only && items.iter().all(Value::is_object) => {}
                    Value::Array(items) if items.iter().any(|i| i.is_object() || i.is_number()) => part(ui, world, orders, &sub, &pretty(name), value, depth),
                    Value::Array(_) | Value::String(_) | Value::Null => {}
                }
            }
        }
        Value::Array(items) => {
            for (i, value) in items.iter().enumerate() {
                let sub = format!("{key}.{i}");
                match value {
                    Value::Number(_) | Value::Bool(_) => value_row(ui, world, orders, &sub, &format!("[{i}]"), value, false),
                    _ => part(ui, world, orders, &sub, &label_of(&format!("[{i}]"), value), value, depth),
                }
            }
        }
        _ => {}
    }
}

/// A folding part with its own values – a nested object or a list.
fn part(ui: &mut Ui, world: &World, orders: &mut Vec<Order>, key: &str, label: &str, value: &Value, depth: usize) {
    let changed = world.overrides.under(key).count();
    let title = if changed > 0 { RichText::new(format!("{label}  ({changed})")).color(CHANGED) } else { RichText::new(label) };
    egui::CollapsingHeader::new(title).id_salt(("data part", key)).default_open(depth < 1 || changed > 0).show(ui, |ui| {
        node_values(ui, world, orders, key, value, false, depth + 1);
    });
}

/// One number or yes/no: its field, and – when this map changes it – the way back. `absent`: the file leaves this value
/// out for this thing (`base` is then 0 or no, in the kind of the others' values) – the map may give it one.
fn value_row(ui: &mut Ui, world: &World, orders: &mut Vec<Order>, key: &str, label: &str, base: &Value, absent: bool) {
    let over = world.overrides.get(key);
    let standard = if absent { "not set".to_owned() } else { show(base) };
    ui.horizontal(|ui| {
        let name = if over.is_some() { RichText::new(label).color(CHANGED) } else { RichText::new(label) };
        ui.label(name).on_hover_text(format!("{key}\nstandard: {standard}"));
        field(ui, orders, key, base, over);
        if over.is_some() {
            ui.label(RichText::new(format!("(standard {standard})")).small().weak());
            if ui.small_button("↺").on_hover_text("Back to the standard value").clicked() {
                orders.push(Order::SetData { key: key.to_owned(), value: None });
            }
        }
    });
}

/// The field of one value: a checkbox for a yes/no, a drag field for a number (whole where the standard is whole). The
/// order comes when the mouse is let go; the standard value itself takes the change away.
fn field(ui: &mut Ui, orders: &mut Vec<Order>, key: &str, base: &Value, over: Option<&Value>) {
    let shown = over.unwrap_or(base);
    match (base, shown) {
        (Value::Bool(_), Value::Bool(now)) => {
            let mut on = *now;
            if ui.checkbox(&mut on, "").changed() {
                orders.push(set(key, Value::Bool(on), base));
            }
        }
        (Value::Number(b), Value::Number(now)) if !b.is_f64() => {
            let (now, base_int) = (now.as_i64().or_else(|| now.as_f64().map(|f| f as i64)).unwrap_or(0), b.as_i64().unwrap_or(0));
            let speed = (base_int.unsigned_abs() as f64 * 0.02).max(0.2);
            let low = if base_int >= 0 { 0 } else { i64::MIN / 2 };
            if let Some(new) = settled(ui, now, |v| egui::DragValue::new(v).speed(speed).range(low..=i64::MAX / 2)) {
                orders.push(set(key, Value::from(new), base));
            }
        }
        (Value::Number(b), Value::Number(now)) => {
            let (now, base_f) = (now.as_f64().unwrap_or(0.0), b.as_f64().unwrap_or(0.0));
            let speed = (base_f.abs() * 0.01).max(0.005);
            let low = if base_f >= 0.0 { 0.0 } else { f64::MIN / 2.0 };
            if let Some(new) = settled(ui, now, |v| egui::DragValue::new(v).speed(speed).range(low..=f64::MAX / 2.0).max_decimals(4)) {
                let new = (new * 10_000.0).round() / 10_000.0;
                if let Some(n) = serde_json::Number::from_f64(new) {
                    orders.push(set(key, Value::Number(n), base));
                }
            }
        }
        _ => {}
    }
}

/// The order that gives `key` this value – the standard itself takes the change away.
fn set(key: &str, value: Value, base: &Value) -> Order {
    let same = match (&value, base) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (a, b) => a == b,
    };
    Order::SetData { key: key.to_owned(), value: (!same).then_some(value) }
}

fn show(value: &Value) -> String {
    match value {
        Value::Bool(b) => if *b { "yes" } else { "no" }.to_owned(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// The weapon times of an object: `(key, standard value)`.
fn times(map: &serde_json::Map<String, Value>) -> Vec<(&'static str, f64)> {
    TIMES.iter().filter_map(|k| map.get(*k).and_then(Value::as_f64).map(|v| (*k, v))).collect()
}

/// The times that make one shot: aim + wind-up + cooldown for a unit, the attack of a zombie, the reload of a tower.
fn cycle(map: &serde_json::Map<String, Value>) -> Vec<&'static str> {
    let has = |k: &str| map.get(k).is_some_and(Value::is_number);
    if has("cooldown_seconds") {
        ["aim_seconds", "windup_seconds", "cooldown_seconds"].into_iter().filter(|k| has(k)).collect()
    } else if has("attack_seconds") {
        vec!["attack_seconds"]
    } else if has("reload_seconds") {
        vec!["reload_seconds"]
    } else {
        Vec::new()
    }
}

/// "Attack speed": one number in percent for all the times of a weapon – 150 % shoots one and a half times as often.
/// It writes the times (raise, lower, aim, wind-up, cooldown, attack, reload) as the standard divided by that – the
/// single times stay editable below.
fn attack_speed(ui: &mut Ui, world: &World, orders: &mut Vec<Order>, key: &str, map: &serde_json::Map<String, Value>) {
    let all = times(map);
    let shot = cycle(map);
    let base_shot: f64 = shot.iter().filter_map(|k| map.get(*k).and_then(Value::as_f64)).sum();
    if shot.is_empty() || base_shot <= 0.0 {
        return;
    }
    let value_of = |k: &str| -> f64 {
        let place = format!("{key}.{k}");
        world.overrides.get(&place).and_then(Value::as_f64).or_else(|| map.get(k).and_then(Value::as_f64)).unwrap_or(0.0)
    };
    let now_shot: f64 = shot.iter().map(|k| value_of(k)).sum();
    let percent = if now_shot > 0.0 { (base_shot / now_shot * 100.0).round() as i64 } else { 1000 };
    ui.horizontal(|ui| {
        ui.label(RichText::new("Attack speed").strong()).on_hover_text("One number for all of its times: raise, lower, aim, wind-up, cooldown, attack, reload");
        if let Some(new) = settled(ui, percent, |v| egui::DragValue::new(v).range(10..=1000).speed(1).suffix(" %")) {
            for (k, base) in &all {
                let scaled = ((base * 100.0 / new as f64) * 1000.0).round() / 1000.0;
                // The standard itself (also a time of 0 – nothing to scale) is no change.
                let value = if new == 100 || scaled == *base { None } else { serde_json::Number::from_f64(scaled).map(Value::Number) };
                orders.push(Order::SetData { key: format!("{key}.{k}"), value });
            }
        }
        ui.label(RichText::new(format!("one shot every {now_shot:.2} s (standard {base_shot:.2} s)")).small().weak());
    });
}

fn changes(ui: &mut Ui, page: &mut DataPage, world: &World, orders: &mut Vec<Order>) {
    section_from(ui, "Changed on this map", Some(Origin::Sc2), "", |ui| {
        let all: &Overrides = &world.overrides;
        if all.is_empty() {
            ui.label(RichText::new("Nothing - this map plays with the standard values of data\\.").weak());
            return;
        }
        ui.horizontal(|ui| {
            ui.label(format!("{} values", all.0.len()));
            let text = if page.confirm_clear { "Sure? Reset all" } else { "Reset all" };
            if ui.button(text).clicked() {
                if page.confirm_clear {
                    orders.push(Order::ClearData);
                    page.confirm_clear = false;
                } else {
                    page.confirm_clear = true;
                }
            }
        });
        for (key, value) in &all.0 {
            ui.horizontal(|ui| {
                if ui.small_button("↺").on_hover_text("Back to the standard value").clicked() {
                    orders.push(Order::SetData { key: key.clone(), value: None });
                }
                let base = data::base_value(key).map_or_else(|| "?".to_owned(), |b| show(&b));
                let go = ui.link(RichText::new(key.as_str()).small()).on_hover_text("Show it");
                if go.clicked() {
                    // The kind it belongs to: everything before the last name (`units.soldier`).
                    if let Some((kind, _)) = key.rsplit_once('.') {
                        let mut kind = kind.to_owned();
                        // A nested value (`units.soldier.explosion.x`): up to the kind the list shows.
                        while node_at(page, &kind).is_some() && !is_kind(page, &kind) && kind.contains('.') {
                            kind = kind.rsplit_once('.').map_or(kind.clone(), |(k, _)| k.to_owned());
                        }
                        page.pick = kind;
                    }
                }
                ui.label(RichText::new(format!("{base} -> {}", show(value))).small().color(CHANGED));
            });
        }
    });
    if world.overrides.is_empty() {
        page.confirm_clear = false;
    }
}

/// Does the list on the left have this key?
fn is_kind(page: &DataPage, key: &str) -> bool {
    page.tree.iter().any(|(file, root)| entries(file, root).iter().any(|(k, _)| k == key))
}

/// The numbers and yes/no the other things of this kind have but this one leaves out – with the zero or no they stand
/// for, in the kind of the others' values (whole or fractional). `parent`: the key of the list the thing is in.
fn left_out(page: &DataPage, parent: &str, node: &Value) -> Vec<(String, Value)> {
    let (Some(own), Some(parent)) = (node.as_object(), node_at(page, parent)) else {
        return Vec::new();
    };
    let siblings: Vec<&Value> = match parent {
        Value::Object(map) => map.values().collect(),
        Value::Array(list) => list.iter().collect(),
        _ => Vec::new(),
    };
    let mut found = std::collections::BTreeMap::new();
    for sibling in siblings.iter().filter_map(|s| s.as_object()) {
        for (name, value) in sibling {
            if own.contains_key(name) || found.contains_key(name) {
                continue;
            }
            let zero = match value {
                Value::Bool(_) => Value::Bool(false),
                Value::Number(n) if n.is_f64() => serde_json::Number::from_f64(0.0).map_or(Value::Null, Value::Number),
                Value::Number(_) => Value::from(0),
                _ => continue,
            };
            found.insert(name.clone(), zero);
        }
    }
    found.into_iter().collect()
}

/// The table of all researches (the Research file's page): per tech its workshop, whether this map **blocks** it, the
/// gold and goods it costs and how long it takes – the numbers a campaign changes from map to map.
fn research_table(ui: &mut Ui, world: &World, orders: &mut Vec<Order>, root: &Value) {
    let Some(techs) = root.get("techs").and_then(Value::as_array) else {
        return;
    };
    let per_factor = |world: &World| world.overrides.get("research.seconds_per_factor").or_else(|| root.get("seconds_per_factor")).and_then(Value::as_f64).unwrap_or(70.0);
    ui.label(RichText::new("Researches").strong());
    ui.label(
        RichText::new("Blocked: not on the workshop's card, can't be started - a trigger's action Research still grants it. The time is in seconds at full power.")
            .small()
            .weak(),
    );
    egui::Grid::new("research table").striped(true).num_columns(9).spacing([10.0, 4.0]).show(ui, |ui| {
        for head in ["Research", "Workshop", "Blocked", "Gold", "Wood", "Stone", "Iron", "Oil", "Time (s)"] {
            ui.label(RichText::new(head).small().weak());
        }
        ui.end_row();
        for (i, tech) in techs.iter().enumerate() {
            let key = format!("research.techs.{i}");
            let changed = world.overrides.under(&key).count() > 0;
            let name = tech.get("name").and_then(Value::as_str).unwrap_or("?");
            ui.label(if changed { RichText::new(name).color(CHANGED) } else { RichText::new(name) }).on_hover_text(&key);
            ui.label(RichText::new(tech.get("researched_in").and_then(Value::as_str).unwrap_or("")).small().weak());
            for what in ["blocked", "gold", "wood", "stone", "iron", "oil"] {
                let place = format!("{key}.{what}");
                let base = tech.get(what).cloned().unwrap_or(if what == "blocked" { Value::Bool(false) } else { Value::from(0) });
                field(ui, orders, &place, &base, world.overrides.get(&place));
            }
            // The time: `seconds_per_factor` × `time_factor` – typed as seconds, kept as the factor.
            let place = format!("{key}.time_factor");
            let base = tech.get("time_factor").cloned().unwrap_or(Value::from(1.0));
            let factor = world.overrides.get(&place).unwrap_or(&base).as_f64().unwrap_or(1.0);
            let seconds = per_factor(world) * factor;
            let new = settled(ui, (seconds * 10.0).round() / 10.0, |v| egui::DragValue::new(v).speed(1.0).range(1.0..=100_000.0).max_decimals(1));
            if let Some(new) = new {
                let factor = ((new / per_factor(world)) * 1000.0).round() / 1000.0;
                if let Some(n) = serde_json::Number::from_f64(factor) {
                    orders.push(set(&place, Value::Number(n), &base));
                }
            }
            ui.end_row();
        }
    });
    ui.add_space(8.0);
    ui.label(RichText::new("Other values of the research").strong());
}

/// The tracks of upgrades (the Upgrades file's page): the level each starts on in this map – a campaign map that begins
/// with level 2 – and what one level gives is in the files of the units and buildings (`upgrades`, below in their page).
fn upgrade_table(ui: &mut Ui, world: &World, orders: &mut Vec<Order>, root: &Value) {
    let Some(tracks) = root.get("tracks").and_then(Value::as_array) else {
        return;
    };
    ui.label(RichText::new("Upgrade levels this map starts on").strong());
    ui.label(
        RichText::new("Like SC2: a research raises a track's level, and every unit and building that lists the track in its own `upgrades` gets its bonus per level - the ones that stand and the ones built later.")
            .small()
            .weak(),
    );
    egui::Grid::new("upgrade tracks").striped(true).num_columns(3).spacing([12.0, 4.0]).show(ui, |ui| {
        for head in ["Track", "Start level", "Of"] {
            ui.label(RichText::new(head).small().weak());
        }
        ui.end_row();
        for track in tracks {
            let id = track.get("id").and_then(Value::as_str).unwrap_or("");
            let levels = track.get("levels").and_then(Value::as_u64).unwrap_or(0) as u32;
            let now = world.upgrade_level(id);
            ui.label(if now > 0 { RichText::new(track.get("name").and_then(Value::as_str).unwrap_or(id)).color(CHANGED) } else { RichText::new(track.get("name").and_then(Value::as_str).unwrap_or(id)) }).on_hover_text(id);
            if let Some(level) = settled(ui, now, |v| egui::DragValue::new(v).range(0..=levels)) {
                orders.push(Order::SetUpgrade { track: id.to_owned(), level });
            }
            ui.label(levels.to_string());
            ui.end_row();
        }
    });
    ui.add_space(8.0);
    ui.label(RichText::new("Other values").strong());
}
