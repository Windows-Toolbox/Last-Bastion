//! The editor's "Story" tab and its script editor (TAB's "Game Narrative", "Custom Commands", "Variables" –
//! `sim/story.rs`): the scripts when the round starts, is won or lost; the goals; the command center's own button; the
//! counters. The script editor is also used for an event's scripts and a neutral thing's script (`editor.rs`).
//!
//! Like the rest of the panel it only reads the world: everything works on a copy, and if the copy differs at the end
//! of the frame, one order sets it (`Order::SetStory`).

use egui_macroquad::egui::{self, ComboBox, DragValue, Frame, RichText, Ui};

use crate::editor::{section, settled_text, wave_group, BUILDINGS, MAX_AMOUNT};
use crate::kinds::{BuildingKind, Resource, UnitKind};
use crate::sim::{Action, BreedCount, Condition, CustomCommand, Goal, Order, Script, Spawner, Story, WaveFrom, WaveGroup, World};

/// The kinds of actions, for "+ action" – in TAB's order.
const ACTIONS: [&str; 13] =
    ["Zombies", "Resources", "Units", "Infect houses", "Notify", "Window", "Random", "Choose", "Counter", "Join", "Reveal", "Win", "Lose"];

/// A new action of the kind named in `ACTIONS`, with values to start from.
fn new_action(name: &str) -> Action {
    match name {
        "Zombies" => {
            let group = WaveGroup { from: WaveFrom::Sides(1), breeds: vec![BreedCount::new(50, crate::sim::Breed::Runner)] };
            Action::Zombies { groups: vec![group], dispersion: 0, storm: true }
        }
        "Resources" => Action::Resources { gold: 100, goods: Vec::new() },
        "Units" => Action::Units { units: vec![(UnitKind::Soldier, 1)] },
        "Infect houses" => Action::InfectHouses { count: 1 },
        "Notify" => Action::Notify { text: "Something happened.".into() },
        "Window" => Action::Window { title: "News".into(), text: "Something happened.".into() },
        "Random" => Action::Random { options: vec![(1, Vec::new()), (1, Vec::new())] },
        "Choose" => Action::Choose { text: "What now?".into(), options: vec![("Yes".into(), Vec::new()), ("No".into(), Vec::new())] },
        "Counter" => Action::Counter { name: "count".into(), add: true, value: 1 },
        "Join" => Action::Join { name_id: String::new() },
        "Reveal" => Action::Reveal,
        "Win" => Action::Win,
        _ => Action::Lose,
    }
}

/// Its name in `ACTIONS`.
fn action_name(action: &Action) -> &'static str {
    match action {
        Action::Zombies { .. } => "Zombies",
        Action::Resources { .. } => "Resources",
        Action::Units { .. } => "Units",
        Action::InfectHouses { .. } => "Infect houses",
        Action::Notify { .. } => "Notify",
        Action::Window { .. } => "Window",
        Action::Random { .. } => "Random",
        Action::Choose { .. } => "Choose",
        Action::Counter { .. } => "Counter",
        Action::Join { .. } => "Join",
        Action::Reveal => "Reveal",
        Action::Win => "Win",
        Action::Lose => "Lose",
    }
}

/// What each action does, for its tooltip.
fn action_help(name: &str) -> &'static str {
    match name {
        "Zombies" => "Zombies out of the spawners, now - storming the command center or idling where they come out",
        "Resources" => "Gold and goods for the colony - below 0 taken away",
        "Units" => "Units for the colony next to the command center",
        "Infect houses" => "So many houses with colonists, picked at random, are infected",
        "Notify" => "A line at the top of the screen",
        "Window" => "A window with a text - the game pauses until it's closed",
        "Random" => "One of the scripts below, each as likely as its weight",
        "Choose" => "The player picks one in a window - its script runs",
        "Counter" => "A counter set to a value, or changed by it",
        "Join" => "The neutral thing with this name ID joins the colony",
        "Reveal" => "The whole map is shown from now on",
        "Win" => "The round is won",
        _ => "The round is lost",
    }
}

/// A script in the panel: one box per action – its fields, x to remove it, ^ to move it up – and "+ action". Works on
/// `script` in place. `id`: egui tells the fields of different scripts apart by it.
pub fn script_ui(ui: &mut Ui, id: egui::Id, script: &mut Script, spawners: &[Spawner]) {
    let mut gone = None;
    let mut up = None;
    for (k, action) in script.iter_mut().enumerate() {
        let id = id.with(k);
        Frame::group(ui.style()).inner_margin(4.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let name = action_name(action);
                ui.label(RichText::new(name).strong()).on_hover_text(action_help(name));
                if ui.small_button("x").on_hover_text("Remove the action").clicked() {
                    gone = Some(k);
                }
                if k > 0 && ui.small_button("^").on_hover_text("Earlier").clicked() {
                    up = Some(k);
                }
            });
            action_ui(ui, id, action, spawners);
        });
    }
    if let Some(k) = gone {
        script.remove(k);
    } else if let Some(k) = up {
        script.swap(k - 1, k);
    }
    ComboBox::from_id_salt(id.with("add")).width(120.0).selected_text("+ action").show_ui(ui, |ui| {
        for name in ACTIONS {
            if ui.selectable_label(false, name).on_hover_text(action_help(name)).clicked() {
                script.push(new_action(name));
            }
        }
    });
}

/// The fields of one action.
fn action_ui(ui: &mut Ui, id: egui::Id, action: &mut Action, spawners: &[Spawner]) {
    match action {
        Action::Zombies { groups, dispersion, storm } => {
            let mut gone = None;
            for (g, group) in groups.iter_mut().enumerate() {
                if wave_group(ui, (id, g), group, spawners) {
                    gone = Some(g);
                }
            }
            if let Some(g) = gone {
                groups.remove(g);
            }
            ui.horizontal_wrapped(|ui| {
                if ui.small_button("+ group").clicked() {
                    groups.push(WaveGroup { from: WaveFrom::Sides(1), breeds: vec![BreedCount::new(50, crate::sim::Breed::Runner)] });
                }
                ui.add(DragValue::new(dispersion).range(0..=200).prefix("spread ").suffix(" tiles"));
                ui.checkbox(storm, "storm the CC");
            });
        }
        Action::Resources { gold, goods } => {
            ui.horizontal(|ui| {
                ui.label("Gold");
                ui.add(DragValue::new(gold).range(-(MAX_AMOUNT as i32)..=MAX_AMOUNT as i32).speed(10));
            });
            let mut gone = None;
            for (k, (resource, n)) in goods.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ComboBox::from_id_salt(id.with(("good", k))).width(90.0).selected_text(resource.name()).show_ui(ui, |ui| {
                        for r in Resource::ALL {
                            ui.selectable_value(resource, r, r.name());
                        }
                    });
                    ui.add(DragValue::new(n).range(-100_000..=100_000));
                    if ui.small_button("x").clicked() {
                        gone = Some(k);
                    }
                });
            }
            if let Some(k) = gone {
                goods.remove(k);
            }
            if ui.small_button("+ good").clicked() {
                goods.push((Resource::Wood, 10));
            }
        }
        Action::Units { units } => {
            let mut gone = None;
            for (k, (kind, n)) in units.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(n).range(1..=100));
                    ComboBox::from_id_salt(id.with(("unit", k))).width(110.0).selected_text(kind.stats().name.as_str()).show_ui(ui, |ui| {
                        for u in UnitKind::ALL {
                            ui.selectable_value(kind, u, u.stats().name.as_str());
                        }
                    });
                    if ui.small_button("x").clicked() {
                        gone = Some(k);
                    }
                });
            }
            if let Some(k) = gone {
                units.remove(k);
            }
            if ui.small_button("+ unit").clicked() {
                units.push((UnitKind::Soldier, 1));
            }
        }
        Action::InfectHouses { count } => {
            ui.add(DragValue::new(count).range(1..=100).suffix(" houses"));
        }
        Action::Notify { text } => text_field(ui, text, false),
        Action::Window { title, text } => {
            text_field(ui, title, false);
            text_field(ui, text, true);
        }
        Action::Random { options } => {
            let mut gone = None;
            for (k, (weight, script)) in options.iter_mut().enumerate() {
                Frame::group(ui.style()).inner_margin(4.0).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(DragValue::new(weight).range(0..=1000).prefix("weight "));
                        if ui.small_button("x").on_hover_text("Remove this option").clicked() {
                            gone = Some(k);
                        }
                    });
                    script_ui(ui, id.with(("random", k)), script, spawners);
                });
            }
            if let Some(k) = gone {
                options.remove(k);
            }
            if ui.small_button("+ option").clicked() {
                options.push((1, Vec::new()));
            }
        }
        Action::Choose { text, options } => {
            text_field(ui, text, true);
            let mut gone = None;
            for (k, (label, script)) in options.iter_mut().enumerate() {
                Frame::group(ui.style()).inner_margin(4.0).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Button");
                        if let Some(new) = settled_text(ui, label, false) {
                            *label = new;
                        }
                        if ui.small_button("x").on_hover_text("Remove this option").clicked() {
                            gone = Some(k);
                        }
                    });
                    script_ui(ui, id.with(("choose", k)), script, spawners);
                });
            }
            if let Some(k) = gone {
                options.remove(k);
            }
            if ui.small_button("+ option").clicked() {
                options.push(("Option".into(), Vec::new()));
            }
        }
        Action::Counter { name, add, value } => {
            ui.horizontal(|ui| {
                if let Some(new) = settled_text(ui, name, false) {
                    *name = new;
                }
            });
            ui.horizontal(|ui| {
                ui.selectable_value(add, false, "set to");
                ui.selectable_value(add, true, "add");
                ui.add(DragValue::new(value).range(-1_000_000..=1_000_000));
            });
        }
        Action::Join { name_id } => {
            ui.horizontal(|ui| {
                ui.label("Name ID");
                if let Some(new) = settled_text(ui, name_id, false) {
                    *name_id = new;
                }
            });
        }
        Action::Reveal | Action::Win | Action::Lose => {}
    }
}

/// A text of an action, changed once typing ends (`settled_text`).
fn text_field(ui: &mut Ui, text: &mut String, multiline: bool) {
    if let Some(new) = settled_text(ui, text, multiline) {
        *text = new;
    }
}

/// The kinds of goals, for the drop-down list.
const CONDITIONS: [&str; 7] = ["Colonists", "Day", "Zombies killed", "Built", "Destroyed", "Counter", "Event done"];

fn condition_name(condition: &Condition) -> &'static str {
    match condition {
        Condition::Colonists(_) => "Colonists",
        Condition::Day(_) => "Day",
        Condition::ZombiesKilled(_) => "Zombies killed",
        Condition::Built { .. } => "Built",
        Condition::Destroyed { .. } => "Destroyed",
        Condition::Counter { .. } => "Counter",
        Condition::EventDone { .. } => "Event done",
    }
}

fn new_condition(name: &str) -> Condition {
    match name {
        "Colonists" => Condition::Colonists(100),
        "Day" => Condition::Day(20),
        "Zombies killed" => Condition::ZombiesKilled(1000),
        "Built" => Condition::Built { kind: BuildingKind::Tent, count: 10 },
        "Destroyed" => Condition::Destroyed { name_id: String::new() },
        "Counter" => Condition::Counter { name: "count".into(), at_least: 1 },
        _ => Condition::EventDone { name: String::new() },
    }
}

/// The "Story" tab: the scripts when the round starts, is won or lost, the goals, the command center's button, the
/// counters – on a copy of the story; changed, one order sets it.
pub fn story_tab(ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
    let mut story: Story = world.story.clone();
    let spawners = &world.spawners;
    let id = egui::Id::new("story");
    for (title, hint, script) in [
        ("On game start", "runs in the first moment of the round", &mut story.on_start),
        ("On victory", "runs when the round is won", &mut story.on_won),
        ("On defeat", "runs when the round is lost", &mut story.on_lost),
    ] {
        section(ui, title, hint, |ui| script_ui(ui, id.with(title), script, spawners));
    }
    section(ui, "Goals", "all victory goals reached: won - any defeat goal reached: lost", |ui| goals_ui(ui, &mut story.goals));
    section(ui, "Command center button", "a button of its own on the command center (V)", |ui| {
        let mut on = story.command.is_some();
        if ui.checkbox(&mut on, "The command center has it").changed() {
            story.command = on.then(|| CustomCommand { label: "Command".into(), ..CustomCommand::default() });
        }
        if let Some(command) = &mut story.command {
            egui::Grid::new("command").num_columns(2).show(ui, |ui| {
                ui.label("Name");
                text_field(ui, &mut command.label, false);
                ui.end_row();
                ui.label("Tooltip");
                text_field(ui, &mut command.text, false);
                ui.end_row();
                ui.label("Times");
                ui.add(DragValue::new(&mut command.uses).range(0..=1000)).on_hover_text("How often it can be pressed - 0: as often as you like");
                ui.end_row();
            });
            script_ui(ui, id.with("command"), &mut command.script, spawners);
        }
    });
    section(ui, "Counters", "numbers the scripts change and the goals ask - their value at the start", |ui| {
        let mut gone = None;
        for (k, (name, value)) in story.counters.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                text_field(ui, name, false);
                ui.add(DragValue::new(value).range(-1_000_000..=1_000_000));
                if ui.small_button("x").clicked() {
                    gone = Some(k);
                }
            });
        }
        if let Some(k) = gone {
            story.counters.remove(k);
        }
        if ui.small_button("+ counter").clicked() {
            story.counters.push((format!("counter {}", story.counters.len() + 1), 0));
        }
        if !world.counters.is_empty() {
            let now: Vec<String> = world.counters.iter().map(|(n, v)| format!("{n} = {v}")).collect();
            ui.label(RichText::new(format!("Now: {}", now.join(", "))).small().weak());
        }
    });
    if story != world.story {
        orders.push(Order::SetStory { story });
    }
}

/// The goals: per goal its text, victory or defeat, and when it's reached.
fn goals_ui(ui: &mut Ui, goals: &mut Vec<Goal>) {
    let mut gone = None;
    for (k, goal) in goals.iter_mut().enumerate() {
        Frame::group(ui.style()).inner_margin(4.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                text_field(ui, &mut goal.text, false);
                if ui.small_button("x").clicked() {
                    gone = Some(k);
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut goal.defeat, false, "Victory");
                ui.selectable_value(&mut goal.defeat, true, "Defeat");
                let shown = condition_name(&goal.condition);
                ComboBox::from_id_salt(("goal", k)).width(110.0).selected_text(shown).show_ui(ui, |ui| {
                    for name in CONDITIONS {
                        if ui.selectable_label(name == shown, name).clicked() && name != shown {
                            goal.condition = new_condition(name);
                        }
                    }
                });
            });
            condition_ui(ui, k, &mut goal.condition);
        });
    }
    if let Some(k) = gone {
        goals.remove(k);
    }
    if ui.small_button("+ goal").clicked() {
        goals.push(Goal { text: "Survive".into(), condition: Condition::Day(20), defeat: false });
    }
}

/// The fields of a goal's condition.
fn condition_ui(ui: &mut Ui, k: usize, condition: &mut Condition) {
    ui.horizontal_wrapped(|ui| match condition {
        Condition::Colonists(n) => {
            ui.add(DragValue::new(n).range(1..=100_000).prefix("at least ").suffix(" colonists"));
        }
        Condition::Day(day) => {
            ui.add(DragValue::new(day).range(1..=1000).prefix("day "));
        }
        Condition::ZombiesKilled(n) => {
            ui.add(DragValue::new(n).range(1..=10_000_000).prefix("at least ").suffix(" killed"));
        }
        Condition::Built { kind, count } => {
            ui.add(DragValue::new(count).range(1..=1000).prefix("at least "));
            ComboBox::from_id_salt(("goal kind", k)).width(120.0).selected_text(kind.stats().name.as_str()).show_ui(ui, |ui| {
                for &b in BUILDINGS {
                    ui.selectable_value(kind, b, b.stats().name.as_str());
                }
            });
        }
        Condition::Destroyed { name_id } => {
            ui.label("Name ID");
            text_field(ui, name_id, false);
        }
        Condition::Counter { name, at_least } => {
            text_field(ui, name, false);
            ui.add(DragValue::new(at_least).range(-1_000_000..=1_000_000).prefix("at least "));
        }
        Condition::EventDone { name } => {
            ui.label("Event");
            text_field(ui, name, false);
        }
    });
}

