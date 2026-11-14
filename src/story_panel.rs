//! The editor's story tools (TAB's "Game Narrative", "Custom Commands", "Variables" – `sim/story.rs`): the fields of a
//! trigger (SC2's: an event, conditions, actions as sentences – `sentences.rs`), of the goals, the command center's own
//! button and the variables. The Triggers page (`editor\triggers.rs`) puts them together; an event's and a neutral
//! thing's own triggers are edited where they are too (`linked_triggers`).
//!
//! Like the rest of the editor it only reads the world: everything works on a copy, and if the copy differs at the end
//! of the frame, one order sets it (`Order::SetStory`).

mod sentences;

use egui_macroquad::egui::{self, Color32, CollapsingHeader, ComboBox, DragValue, Frame, RichText, TextEdit, Ui};

use crate::editor::{settled_text, Places};
use crate::kinds::BuildingKind;
use crate::sim::{
    parse_condition, parse_expr, signature, CustomCommand, Expr, ExprError, Goal, Order, Scope, Story, Trigger, TriggerEvent, Ty, Value,
    VarKind, Variable, World, FUNCS,
};

pub use sentences::{action_help, add_from_palette, mark_top, script_ui, GROUPS};

/// "if" and a field for an optional condition (empty: none), checked as it is typed.
fn when_field(ui: &mut Ui, id: egui::Id, when: &mut Option<Expr>, scope: Scope, hint: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("if").weak());
        let shown = when.as_ref().map_or(String::new(), |e| e.to_string());
        let parse = |t: &str| if t.trim().is_empty() { Ok(None) } else { parse_condition(t, scope).map(Some) };
        if let Some(new) = code_field(ui, id, &shown, hint, false, parse) {
            *when = new;
        }
    });
}

/// A text field for an expression or code, checked as it is typed (`parse`): a mistake is shown under the field with
/// its place; the text stays as typed until it's right. Only a text that parses ends the editing – losing the focus
/// gives the parsed result back. `shown`: the text of what's stored now (what the field starts with).
pub(crate) fn code_field<T>(ui: &mut Ui, id: egui::Id, shown: &str, hint: &str, multiline: bool, parse: impl Fn(&str) -> Result<T, ExprError>) -> Option<T> {
    /// What is typed and not yet taken over – kept by egui between frames.
    #[derive(Clone)]
    struct Draft {
        /// The stored text it was started from: if that changed meanwhile (another step moved here), it's dropped.
        base: String,
        text: String,
        error: Option<ExprError>,
    }
    let mut draft = ui
        .data(|data| data.get_temp::<Draft>(id))
        .filter(|d| d.base == shown)
        .unwrap_or_else(|| Draft { base: shown.to_owned(), text: shown.to_owned(), error: None });
    // In a column of its own: the message goes under the field, not beside it (that would widen the panel).
    ui.vertical(|ui| {
        let edit = if multiline { TextEdit::multiline(&mut draft.text).desired_rows(1) } else { TextEdit::singleline(&mut draft.text) };
        let response = ui.add(edit.hint_text(hint).font(egui::TextStyle::Monospace).desired_width(ui.available_width()));
        if response.changed() {
            draft.error = parse(&draft.text).err();
        }
        let mut result = None;
        if response.lost_focus() {
            match parse(&draft.text) {
                Ok(t) => result = Some(t),
                Err(e) => draft.error = Some(e),
            }
        }
        if let Some(e) = &draft.error {
            ui.label(RichText::new(e.to_string()).small().color(Color32::LIGHT_RED));
        }
        if result.is_some() {
            ui.data_mut(|data| data.remove::<Draft>(id));
        } else if draft.text != draft.base || draft.error.is_some() {
            ui.data_mut(|data| data.insert_temp(id, draft));
        } else {
            ui.data_mut(|data| data.remove::<Draft>(id));
        }
        result
    })
    .inner
}

/// A text of an action, changed once typing ends (`settled_text`).
pub(crate) fn text_field(ui: &mut Ui, text: &mut String, multiline: bool) {
    if let Some(new) = settled_text(ui, text, multiline) {
        *text = new;
    }
}

/// The command center's button: its texts, how often, and the three expressions asked every tick.
pub(crate) fn command_section(ui: &mut Ui, command: &mut Option<CustomCommand>, places: Places) {
    let mut on = command.is_some();
    if ui.checkbox(&mut on, "The command center has it").changed() {
        *command = on.then(|| CustomCommand { label: "Command".into(), ..CustomCommand::default() });
    }
    let Some(command) = command else {
        return;
    };
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
    // TAB's three expressions, asked every tick - each empty: always yes / no number.
    let scope = Scope { variables: places.variables };
    let condition = |ui: &mut Ui, id: &str, label: &str, field: &mut Option<Expr>, hint: &str| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).weak());
            let shown = field.as_ref().map_or(String::new(), |e| e.to_string());
            let parse = |t: &str| if t.trim().is_empty() { Ok(None) } else { parse_condition(t, scope).map(Some) };
            if let Some(new) = code_field(ui, egui::Id::new(("command", id)), &shown, hint, false, parse) {
                *field = new;
            }
        });
    };
    condition(ui, "visible", "visible if", &mut command.visible, "condition - empty: always");
    condition(ui, "enabled", "enabled if", &mut command.enabled, "condition - empty: always");
    if command.enabled.is_some() {
        ui.horizontal(|ui| {
            ui.label(RichText::new("when not").weak());
            text_field(ui, &mut command.disabled_text, false);
        });
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new("counter").weak());
        let shown = command.counter.as_ref().map_or(String::new(), |e| e.to_string());
        let parse = |t: &str| if t.trim().is_empty() { Ok(None) } else { parse_expr(t, scope, Ty::Number).map(Some) };
        if let Some(new) = code_field(ui, egui::Id::new(("command", "counter")), &shown, "a number shown on the button, e.g. keys - empty: none", false, parse) {
            command.counter = new;
        }
    });
}

/// The variables: name, kind, value at the start – and what they hold now (in a test).
pub(crate) fn variables_section(ui: &mut Ui, story: &mut Story, world: &World) {
    let mut gone = None;
    for (k, v) in story.variables.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            text_field(ui, &mut v.name, false);
            ui.selectable_value(&mut v.kind, VarKind::Number, "No.").on_hover_text("A number");
            ui.selectable_value(&mut v.kind, VarKind::Flag, "Y/N").on_hover_text("Yes or no");
            ui.selectable_value(&mut v.kind, VarKind::Text, "Txt").on_hover_text("A text");
            // The start value in the kind it must have (an older file may have a number for a yes/no one).
            let mut start = v.start.as_ty(v.kind.ty());
            match &mut start {
                Value::Number(n) => {
                    ui.add(DragValue::new(n).range(-1_000_000..=1_000_000));
                }
                Value::Flag(yes) => {
                    ui.checkbox(yes, "yes");
                }
                Value::Text(t) => text_field(ui, t, false),
            }
            if start != v.start {
                v.start = start;
            }
            if ui.small_button("x").clicked() {
                gone = Some(k);
            }
        });
    }
    if let Some(k) = gone {
        story.variables.remove(k);
    }
    if ui.small_button("+ variable").clicked() {
        story.variables.push(Variable { name: format!("variable {}", story.variables.len() + 1), kind: VarKind::Number, start: Value::Number(0) });
    }
    // A variable that changed its kind (or went) while code still uses it as the old kind: the round would turn the value.
    if !story.variables.is_empty() {
        let misfits = story.misfits();
        if !misfits.is_empty() {
            ui.label(RichText::new(format!("Doesn't fit the variables any more: {}", misfits.join(", "))).small().color(Color32::LIGHT_RED));
        }
    }
    if !world.variables.is_empty() {
        let now: Vec<String> = world.variables.iter().map(|(n, v)| format!("{n} = {}", v.text())).collect();
        ui.label(RichText::new(format!("Now: {}", now.join(", "))).small().weak());
    }
}

/// The list of everything the language knows (`funcs.rs`): the way to write it, what it gives back, what it does.
pub(crate) fn functions_help(ui: &mut Ui) {
    ui.label(RichText::new("Operators: | & = != < <= > >= + - * / ! - and ( ). Text in \"quotes\"; true / false. Code: x = 1; x += 2; GiveGold(5)").small().weak());
    for f in FUNCS {
        let gives = f.returns.map_or(String::new(), |t: Ty| format!(" -> {}", t.name()));
        ui.label(RichText::new(format!("{}{gives}", signature(f))).monospace()).on_hover_text(f.help);
        ui.label(RichText::new(f.help).small().weak());
    }
}

/// The kinds of events, for the drop-down list of a trigger.
const EVENTS: [&str; 15] = [
    "Round starts",
    "Round won",
    "Round lost",
    "Event comes",
    "Event done",
    "Thing joins",
    "Button",
    "Every few seconds",
    "Day and hour",
    "Building finished",
    "Building falls",
    "Zombies killed",
    "Unit enters region",
    "Unit leaves region",
    "Unit reaches thing",
];

fn event_name(event: &TriggerEvent) -> &'static str {
    match event {
        TriggerEvent::Start => EVENTS[0],
        TriggerEvent::Won => EVENTS[1],
        TriggerEvent::Lost => EVENTS[2],
        TriggerEvent::EventComes { .. } => EVENTS[3],
        TriggerEvent::EventDone { .. } => EVENTS[4],
        TriggerEvent::Join { .. } => EVENTS[5],
        TriggerEvent::Button => EVENTS[6],
        TriggerEvent::Every { .. } => EVENTS[7],
        TriggerEvent::Clock { .. } => EVENTS[8],
        TriggerEvent::Built { .. } => EVENTS[9],
        TriggerEvent::Fell { .. } => EVENTS[10],
        TriggerEvent::Kills { .. } => EVENTS[11],
        TriggerEvent::Enters { .. } => EVENTS[12],
        TriggerEvent::Leaves { .. } => EVENTS[13],
        TriggerEvent::Reached { .. } => EVENTS[14],
    }
}

fn event_help(name: &str) -> &'static str {
    match name {
        "Round starts" => "In the first moment of the round",
        "Round won" => "When the round is won",
        "Round lost" => "When the round is lost",
        "Event comes" => "Every time the event (Events) with this name comes",
        "Event done" => "Once, when the event with this name has come every time - a final wave once none of it is left",
        "Thing joins" => "When the neutral thing with this name ID (Objects) joins the colony",
        "Every few seconds" => "Again and again: every so many seconds of the round, the first time after that many",
        "Day and hour" => "Once, when the clock reaches this day and hour",
        "Building finished" => "When a building (of this kind, or any) is finished",
        "Building falls" => "When a building (of this kind, or any) is destroyed - not torn down, not a barrel that went off",
        "Zombies killed" => "Once, when the zombies killed so far reach this many",
        "Unit enters region" => "When one of your units comes into the region (Regions) - a unit that starts in it doesn't count. Once per look, however many come",
        "Unit leaves region" => "When one of your units goes out of the region (or dies in it)",
        "Unit reaches thing" => "Once, when one of your units comes within a tile of the neutral thing with this name ID (Objects) - TAB's \"On Enter\"",
        _ => "When the player presses the command center's button (below)",
    }
}

/// A new event of the kind named in `EVENTS` – the name of an event or thing stays when the kind changes between
/// those that have one.
fn new_event(name: &str, old: &TriggerEvent) -> TriggerEvent {
    let kept = match old {
        TriggerEvent::EventComes { event } | TriggerEvent::EventDone { event } => event.clone(),
        TriggerEvent::Join { name_id } | TriggerEvent::Reached { name_id } => name_id.clone(),
        _ => String::new(),
    };
    match name {
        "Round starts" => TriggerEvent::Start,
        "Round won" => TriggerEvent::Won,
        "Round lost" => TriggerEvent::Lost,
        "Event comes" => TriggerEvent::EventComes { event: kept },
        "Event done" => TriggerEvent::EventDone { event: kept },
        "Thing joins" => TriggerEvent::Join { name_id: kept },
        "Every few seconds" => TriggerEvent::Every { seconds: 60 },
        "Day and hour" => TriggerEvent::Clock { day: 2, hour: 12 },
        "Building finished" => TriggerEvent::Built { kind: None },
        "Building falls" => TriggerEvent::Fell { kind: None },
        "Zombies killed" => TriggerEvent::Kills { count: 100 },
        "Unit enters region" => TriggerEvent::Enters { zone: 0 },
        "Unit leaves region" => TriggerEvent::Leaves { zone: 0 },
        "Unit reaches thing" => TriggerEvent::Reached { name_id: kept },
        _ => TriggerEvent::Button,
    }
}

/// What a trigger's event is called, short: "round starts", "event Wave 1", "Gate joins", "button" …
pub(crate) fn event_label(event: &TriggerEvent) -> String {
    match event {
        TriggerEvent::Start => "round starts".to_owned(),
        TriggerEvent::Won => "round won".to_owned(),
        TriggerEvent::Lost => "round lost".to_owned(),
        TriggerEvent::EventComes { event } => format!("event {event} comes"),
        TriggerEvent::EventDone { event } => format!("event {event} done"),
        TriggerEvent::Join { name_id } => format!("{name_id} joins"),
        TriggerEvent::Button => "button".to_owned(),
        TriggerEvent::Every { seconds } => format!("every {seconds} s"),
        TriggerEvent::Clock { day, hour } => format!("day {day}, {hour:02}:00"),
        TriggerEvent::Built { kind } => format!("{} finished", kind.map_or("building", |k| k.stats().name.as_str())),
        TriggerEvent::Fell { kind } => format!("{} falls", kind.map_or("building", |k| k.stats().name.as_str())),
        TriggerEvent::Kills { count } => format!("{count} zombies killed"),
        TriggerEvent::Enters { zone } => format!("unit enters region {zone}"),
        TriggerEvent::Leaves { zone } => format!("unit leaves region {zone}"),
        TriggerEvent::Reached { name_id } => format!("{name_id} reached"),
    }
}

/// "When": the kind of event and the name it needs, as words on one line.
pub(crate) fn trigger_event_ui(ui: &mut Ui, id: egui::Id, t: &mut Trigger, places: Places) {
    ui.horizontal_wrapped(|ui| {
        let shown = event_name(&t.event);
        ComboBox::from_id_salt(id.with("event")).width(120.0).selected_text(shown).show_ui(ui, |ui| {
            for name in EVENTS {
                if ui.selectable_label(name == shown, name).on_hover_text(event_help(name)).clicked() && name != shown {
                    t.event = new_event(name, &t.event);
                }
            }
        });
        match &mut t.event {
            TriggerEvent::EventComes { event } | TriggerEvent::EventDone { event } => {
                ui.label("named");
                text_field(ui, event, false);
            }
            TriggerEvent::Join { name_id } | TriggerEvent::Reached { name_id } => {
                ui.label("name ID");
                text_field(ui, name_id, false);
            }
            TriggerEvent::Every { seconds } => {
                ui.add(DragValue::new(seconds).range(1..=36_000).suffix(" s"));
            }
            TriggerEvent::Clock { day, hour } => {
                ui.add(DragValue::new(day).range(1..=999).prefix("day "));
                ui.add(DragValue::new(hour).range(0..=23).prefix("hour "));
            }
            TriggerEvent::Built { kind } | TriggerEvent::Fell { kind } => {
                let shown = kind.map_or("any building", |k| k.stats().name.as_str());
                ComboBox::from_id_salt(id.with("kind")).width(150.0).selected_text(shown).show_ui(ui, |ui| {
                    ui.selectable_value(kind, None, "any building");
                    for k in BuildingKind::ALL {
                        ui.selectable_value(kind, Some(k), k.stats().name.as_str());
                    }
                });
            }
            TriggerEvent::Kills { count } => {
                ui.add(DragValue::new(count).range(1..=10_000_000).speed(10));
            }
            TriggerEvent::Enters { zone } | TriggerEvent::Leaves { zone } => {
                ui.label("region");
                zone_box(ui, id.with("zone"), zone, places);
            }
            _ => {}
        }
    });
}

/// A drop-down list of the map's regions, by name - the number is what's kept.
pub(crate) fn zone_box(ui: &mut Ui, id: egui::Id, zone: &mut u32, places: Places) {
    let shown = places.zones.iter().find(|z| z.id == *zone).map_or("(choose one)", |z| z.name.as_str());
    ComboBox::from_id_salt(id).width(150.0).selected_text(shown).show_ui(ui, |ui| {
        for z in places.zones {
            ui.selectable_value(zone, z.id, z.name.as_str());
        }
    });
    if places.zones.is_empty() {
        ui.label(RichText::new("No regions yet - make some in the Regions module").small().weak());
    }
}

/// "If" (the conditions, all of them) and "Then" (the actions as sentences) of a trigger.
pub(crate) fn trigger_body(ui: &mut Ui, id: egui::Id, t: &mut Trigger, places: Places) {
    ui.label(RichText::new("If (all of them):").weak());
    let mut gone = None;
    let scope = Scope { variables: places.variables };
    for (k, condition) in t.conditions.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            if ui.small_button("x").on_hover_text("Remove the condition").clicked() {
                gone = Some(k);
            }
            let shown = condition.to_string();
            if let Some(new) = code_field(ui, id.with(("if", k)), &shown, "e.g. Day >= 3 & Gold > 200", false, |t| parse_condition(t, scope)) {
                *condition = new;
            }
        });
    }
    if let Some(k) = gone {
        t.conditions.remove(k);
    }
    if ui.small_button("+ condition").on_hover_text("A yes/no expression, e.g. Day >= 3 & Gold > 200 - see Functions").clicked() {
        t.conditions.push(Expr::yes());
    }
    ui.label(RichText::new("Then:").weak());
    let actions = id.with("actions");
    mark_top(ui, actions);
    script_ui(ui, actions, &mut t.actions, places);
}

/// One trigger in a box (an event's window, a neutral thing's properties): on/off, name, x, the conditions, the
/// actions. The event is not shown – the trigger is shown where its event is.
fn trigger_ui(ui: &mut Ui, id: egui::Id, t: &mut Trigger, places: Places) -> bool {
    let mut remove = false;
    Frame::group(ui.style()).inner_margin(4.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.checkbox(&mut t.on, "").on_hover_text("Off: it never fires");
            text_field(ui, &mut t.name, false);
            remove = ui.small_button("x").on_hover_text("Remove the trigger").clicked();
        });
        trigger_body(ui, id, t, places);
    });
    remove
}

/// The triggers with this event, edited where the event is (an event's window, a neutral thing's properties): the
/// same box as in the Story tab without the event, and "+ trigger". On a copy of the story; changed, one order sets it.
pub fn linked_triggers(ui: &mut Ui, id: egui::Id, world: &World, orders: &mut Vec<Order>, event: &TriggerEvent, title: &str) {
    let mut story = world.story.clone();
    let places = Places::of(world);
    let mut gone = None;
    let count = story.triggers.iter().filter(|t| t.event == *event).count();
    let shown = if count == 0 { title.to_owned() } else { format!("{title} ({count})") };
    CollapsingHeader::new(shown).id_salt(id).show(ui, |ui| {
        for k in 0..story.triggers.len() {
            if story.triggers[k].event != *event {
                continue;
            }
            if trigger_ui(ui, id.with(k), &mut story.triggers[k], places) {
                gone = Some(k);
            }
        }
        if ui.small_button("+ trigger").clicked() {
            story.triggers.push(Trigger::new(&format!("Trigger {}", story.triggers.len() + 1), event.clone()));
        }
    });
    if let Some(k) = gone {
        story.triggers.remove(k);
    }
    if story != world.story {
        orders.push(Order::SetStory { story });
    }
}

/// The goals alone (the Map page): a copy of the story's goals; changed, one order sets the story.
pub fn goals_section(ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
    let mut story = world.story.clone();
    goals_ui(ui, &mut story.goals, Places::of(world));
    if story != world.story {
        orders.push(Order::SetStory { story });
    }
}

/// The goals: per goal its text, victory or defeat, when it's reached and how far along it is (TAB's two ZXCode
/// fields: a condition, and a number from 0 to 100).
pub(crate) fn goals_ui(ui: &mut Ui, goals: &mut Vec<Goal>, places: Places) {
    let mut gone = None;
    let scope = Scope { variables: places.variables };
    for (k, goal) in goals.iter_mut().enumerate() {
        let id = egui::Id::new(("goal", k));
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
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("reached if").weak());
                let shown = goal.when.to_string();
                if let Some(new) = code_field(ui, id.with("when"), &shown, "e.g. Day >= 20", false, |t| parse_condition(t, scope)) {
                    goal.when = new;
                }
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("progress").weak());
                let shown = goal.progress.as_ref().map_or(String::new(), |e| e.to_string());
                let parse = |t: &str| if t.trim().is_empty() { Ok(None) } else { parse_expr(t, scope, Ty::Number).map(Some) };
                if let Some(new) = code_field(ui, id.with("progress"), &shown, "0 to 100, e.g. Day * 100 / 20 - empty: none", false, parse) {
                    goal.progress = new;
                }
            });
        });
    }
    if let Some(k) = gone {
        goals.remove(k);
    }
    if ui.small_button("+ goal").clicked() {
        let when = parse_condition("Day >= 20", scope).unwrap_or_else(|_| Expr::yes());
        let progress = parse_expr("Day * 100 / 20", scope, Ty::Number).ok();
        goals.push(Goal { text: "Survive".into(), when, progress, defeat: false });
    }
}
