//! The editor's "Story" tab and its script editor (TAB's "Game Narrative", "Custom Commands", "Variables" –
//! `sim/story.rs`): the triggers (SC2's: an event, conditions, actions); the goals; the command center's own button; the
//! variables. An event's and a neutral thing's own triggers are edited where they are too (`linked_triggers`).
//!
//! Like the rest of the panel it only reads the world: everything works on a copy, and if the copy differs at the end
//! of the frame, one order sets it (`Order::SetStory`).

use egui_macroquad::egui::{self, Color32, CollapsingHeader, ComboBox, DragValue, Frame, RichText, TextEdit, Ui};

use crate::editor::{section, settled_text, wave_group, Places, BUILDINGS, MAX_AMOUNT};
use crate::kinds::{BuildingKind, Resource, UnitKind};
use crate::sim::{
    code_text, parse_code, parse_condition, signature, Action, BreedCount, Condition, CustomCommand, Expr, ExprError, Goal, Order, Scope, Script,
    Step, Story, Trigger, TriggerEvent, Ty, Value, VarKind, Variable, WaveFrom, WaveGroup, Weather, World, FUNCS,
};

/// The kinds of actions, for "+ action" – in TAB's order.
const ACTIONS: [&str; 15] = [
    "Zombies", "Resources", "Units", "Infect houses", "Notify", "Window", "Random", "Choose", "Code", "Join", "Reveal", "Weather", "Train",
    "Win", "Lose",
];

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
        "Code" => Action::Code,
        "Join" => Action::Join { name_id: String::new() },
        "Reveal" => Action::Reveal,
        "Weather" => Action::Weather { weather: Weather::Snow },
        "Train" => Action::Train { stop: true, reverse: false, on_arrival: Vec::new() },
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
        Action::Code => "Code",
        Action::Join { .. } => "Join",
        Action::Reveal => "Reveal",
        Action::Weather { .. } => "Weather",
        Action::Train { .. } => "Train",
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
        "Code" => "Nothing but the code below: set variables, call functions (keys += 1; GiveGold(100))",
        "Join" => "The neutral thing with this name ID joins the colony",
        "Reveal" => "The whole map is shown from now on",
        "Weather" => "Snow, rain or clear from now on - it fades in over a few seconds. A round starts with its landscape's (snow on Frozen Highlands, rain on Caustic Lands)",
        "Train" => "A train comes along the railway (Objects > Railway): on the line closest to the command center, from its end nearer the edge of the map. It runs over everyone in its way",
        "Win" => "The round is won",
        _ => "The round is lost",
    }
}

/// A script in the panel: one box per action – its fields, x to remove it, ^ to move it up – and "+ action". Works on
/// `script` in place. `id`: egui tells the fields of different scripts apart by it.
pub fn script_ui(ui: &mut Ui, id: egui::Id, script: &mut Script, places: Places) {
    let mut gone = None;
    let mut up = None;
    let scope = Scope { variables: places.variables };
    for (k, step) in script.iter_mut().enumerate() {
        let id = id.with(k);
        Frame::group(ui.style()).inner_margin(4.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let name = action_name(&step.action);
                ui.label(RichText::new(name).strong()).on_hover_text(action_help(name));
                if ui.small_button("x").on_hover_text("Remove the action").clicked() {
                    gone = Some(k);
                }
                if k > 0 && ui.small_button("^").on_hover_text("Earlier").clicked() {
                    up = Some(k);
                }
            });
            // TAB's two ZXCode fields: the condition (false: the action and the code are skipped) …
            ui.horizontal(|ui| {
                ui.label(RichText::new("if").weak());
                let shown = step.when.as_ref().map_or(String::new(), |e| e.to_string());
                let parse = |t: &str| if t.trim().is_empty() { Ok(None) } else { parse_condition(t, scope).map(Some) };
                if let Some(new) = code_field(ui, id.with("if"), &shown, "condition - empty: always", false, parse) {
                    step.when = new;
                }
            });
            action_ui(ui, id, &mut step.action, places);
            // … and the code that runs after the action.
            ui.horizontal(|ui| {
                ui.label(RichText::new("code").weak());
                let shown = code_text(&step.code);
                if let Some(new) = code_field(ui, id.with("code"), &shown, "code, e.g. keys += 1; GiveGold(100)", true, |t| parse_code(t, scope)) {
                    step.code = new;
                }
            });
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
                script.push(Step::new(new_action(name)));
            }
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

/// The fields of one action.
fn action_ui(ui: &mut Ui, id: egui::Id, action: &mut Action, places: Places) {
    match action {
        Action::Zombies { groups, dispersion, storm } => {
            let mut gone = None;
            for (g, group) in groups.iter_mut().enumerate() {
                if wave_group(ui, (id, g), group, places) {
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
                    script_ui(ui, id.with(("random", k)), script, places);
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
                    script_ui(ui, id.with(("choose", k)), script, places);
                });
            }
            if let Some(k) = gone {
                options.remove(k);
            }
            if ui.small_button("+ option").clicked() {
                options.push(("Option".into(), Vec::new()));
            }
        }
        Action::Join { name_id } => {
            ui.horizontal(|ui| {
                ui.label("Name ID");
                if let Some(new) = settled_text(ui, name_id, false) {
                    *name_id = new;
                }
            });
        }
        Action::Weather { weather } => {
            ui.horizontal(|ui| {
                for w in Weather::ALL {
                    ui.selectable_value(weather, w, w.name());
                }
            });
        }
        Action::Train { stop, reverse, on_arrival } => {
            ui.checkbox(reverse, "from the other end").on_hover_text("Off: it comes from the end of the line nearer the edge of the map");
            ui.checkbox(stop, "stops at the command center").on_hover_text("It waits there 3 s - then the script below runs (TAB unloads colonists there)");
            if *stop {
                ui.label(RichText::new("When it's there:").weak());
                script_ui(ui, id.with("arrival"), on_arrival, places);
            }
        }
        Action::Reveal | Action::Win | Action::Lose | Action::Code => {}
    }
}

/// A text of an action, changed once typing ends (`settled_text`).
fn text_field(ui: &mut Ui, text: &mut String, multiline: bool) {
    if let Some(new) = settled_text(ui, text, multiline) {
        *text = new;
    }
}

/// The kinds of conditions (goals, triggers), for the drop-down list.
const CONDITIONS: [&str; 7] = ["Colonists", "Day", "Zombies killed", "Built", "Destroyed", "Variable", "Event done"];

fn condition_name(condition: &Condition) -> &'static str {
    match condition {
        Condition::Colonists(_) => "Colonists",
        Condition::Day(_) => "Day",
        Condition::ZombiesKilled(_) => "Zombies killed",
        Condition::Built { .. } => "Built",
        Condition::Destroyed { .. } => "Destroyed",
        Condition::Variable { .. } => "Variable",
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
        "Variable" => Condition::Variable { name: "count".into(), at_least: 1 },
        _ => Condition::EventDone { name: String::new() },
    }
}

/// The "Story" tab: the triggers, the goals, the command center's button, the variables – on a copy of the story;
/// changed, one order sets it.
pub fn story_tab(ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
    let mut story: Story = world.story.clone();
    let places = Places::of(world);
    let id = egui::Id::new("story");
    section(ui, "Triggers", "an event, conditions that must all hold, and what happens then - in this order", |ui| {
        triggers_ui(ui, id.with("triggers"), &mut story.triggers, places);
    });
    section(ui, "Goals", "all victory goals reached: won - any defeat goal reached: lost", |ui| goals_ui(ui, &mut story.goals));
    section(ui, "Command center button", "a button of its own on the command center (V) - what it does: the triggers with the event Button", |ui| {
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
        }
    });
    section(ui, "Variables", "numbers, yes/no or texts the triggers change and ask - their value at the start", |ui| {
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
        if !world.variables.is_empty() {
            let now: Vec<String> = world.variables.iter().map(|(n, v)| format!("{n} = {}", v.text())).collect();
            ui.label(RichText::new(format!("Now: {}", now.join(", "))).small().weak());
        }
    });
    section(ui, "Functions", "what the conditions and the code can use - names in any case", functions_help);
    if story != world.story {
        orders.push(Order::SetStory { story });
    }
}

/// The list of everything the language knows (`funcs.rs`): the way to write it, what it gives back, what it does.
fn functions_help(ui: &mut Ui) {
    ui.label(RichText::new("Operators: | & = != < <= > >= + - * / ! - and ( ). Text in \"quotes\"; true / false. Code: x = 1; x += 2; GiveGold(5)").small().weak());
    for f in FUNCS {
        let gives = f.returns.map_or(String::new(), |t: Ty| format!(" -> {}", t.name()));
        ui.label(RichText::new(format!("{}{gives}", signature(f))).monospace()).on_hover_text(f.help);
        ui.label(RichText::new(f.help).small().weak());
    }
}

/// The kinds of events, for the drop-down list of a trigger.
const EVENTS: [&str; 7] = ["Round starts", "Round won", "Round lost", "Event comes", "Event done", "Thing joins", "Button"];

fn event_name(event: &TriggerEvent) -> &'static str {
    match event {
        TriggerEvent::Start => EVENTS[0],
        TriggerEvent::Won => EVENTS[1],
        TriggerEvent::Lost => EVENTS[2],
        TriggerEvent::EventComes { .. } => EVENTS[3],
        TriggerEvent::EventDone { .. } => EVENTS[4],
        TriggerEvent::Join { .. } => EVENTS[5],
        TriggerEvent::Button => EVENTS[6],
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
        _ => "When the player presses the command center's button (below)",
    }
}

/// A new event of the kind named in `EVENTS` – the name of an event or thing stays when the kind changes between
/// those that have one.
fn new_event(name: &str, old: &TriggerEvent) -> TriggerEvent {
    let kept = match old {
        TriggerEvent::EventComes { event } | TriggerEvent::EventDone { event } => event.clone(),
        TriggerEvent::Join { name_id } => name_id.clone(),
        _ => String::new(),
    };
    match name {
        "Round starts" => TriggerEvent::Start,
        "Round won" => TriggerEvent::Won,
        "Round lost" => TriggerEvent::Lost,
        "Event comes" => TriggerEvent::EventComes { event: kept },
        "Event done" => TriggerEvent::EventDone { event: kept },
        "Thing joins" => TriggerEvent::Join { name_id: kept },
        _ => TriggerEvent::Button,
    }
}

/// All triggers of the map, grouped by folder: the ones without a folder on top, one fold-away box per folder.
fn triggers_ui(ui: &mut Ui, id: egui::Id, triggers: &mut Vec<Trigger>, places: Places) {
    let mut folders: Vec<String> = Vec::new();
    for t in triggers.iter() {
        if !folders.contains(&t.folder) {
            folders.push(t.folder.clone());
        }
    }
    // The empty folder name first (`false` sorts before `true`; the sort keeps the rest in order).
    folders.sort_by_key(|f| !f.is_empty());
    let (mut gone, mut up) = (None, None);
    for folder in &folders {
        let count = triggers.iter().filter(|t| t.folder == *folder).count();
        let mut list = |ui: &mut Ui| {
            for k in 0..triggers.len() {
                if triggers[k].folder != *folder {
                    continue;
                }
                let earlier = triggers[..k].iter().rposition(|t| t.folder == *folder);
                let act = trigger_ui(ui, id.with(k), &mut triggers[k], places, true, earlier.is_some());
                if act.remove {
                    gone = Some(k);
                } else if act.up {
                    up = earlier.map(|e| (e, k));
                }
            }
        };
        if folder.is_empty() {
            list(ui);
        } else {
            CollapsingHeader::new(format!("{folder} ({count})")).id_salt(id.with(("folder", folder))).default_open(true).show(ui, list);
        }
    }
    if let Some(k) = gone {
        triggers.remove(k);
    } else if let Some((a, b)) = up {
        triggers.swap(a, b);
    }
    if ui.small_button("+ trigger").on_hover_text("A new trigger - it fires when the round starts, change its event").clicked() {
        triggers.push(Trigger::new(&format!("Trigger {}", triggers.len() + 1), TriggerEvent::Start));
    }
}

/// What the buttons of a trigger asked for.
struct TriggerAct {
    remove: bool,
    up: bool,
}

/// One trigger: on/off, name, (the event and folder,) the conditions, the actions. `with_event`: the event is chosen
/// here; otherwise the trigger is shown where its event is (an event's window, a neutral thing's properties).
fn trigger_ui(ui: &mut Ui, id: egui::Id, t: &mut Trigger, places: Places, with_event: bool, can_move_up: bool) -> TriggerAct {
    let mut act = TriggerAct { remove: false, up: false };
    Frame::group(ui.style()).inner_margin(4.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.checkbox(&mut t.on, "").on_hover_text("Off: it never fires");
            ui.label(RichText::new("Trigger").strong());
            act.remove = ui.small_button("x").on_hover_text("Remove the trigger").clicked();
            act.up = can_move_up && ui.small_button("^").on_hover_text("Earlier (it fires before the next one)").clicked();
        });
        ui.horizontal(|ui| {
            ui.label("Name");
            text_field(ui, &mut t.name, false);
        });
        if with_event {
            ui.horizontal_wrapped(|ui| {
                ui.label("Event");
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
                    TriggerEvent::Join { name_id } => {
                        ui.label("name ID");
                        text_field(ui, name_id, false);
                    }
                    _ => {}
                }
            });
            ui.horizontal(|ui| {
                ui.label("Folder");
                text_field(ui, &mut t.folder, false);
            });
        }
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
        if ui.small_button("+ condition").on_hover_text("A yes/no expression, e.g. Day >= 3 & Gold > 200 - see Functions in the Story tab").clicked() {
            t.conditions.push(Expr::yes());
        }
        ui.label(RichText::new("Then:").weak());
        script_ui(ui, id.with("actions"), &mut t.actions, places);
    });
    act
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
            if trigger_ui(ui, id.with(k), &mut story.triggers[k], places, false, false).remove {
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
    goals_ui(ui, &mut story.goals);
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
            condition_ui(ui, egui::Id::new(("goal", k)), &mut goal.condition);
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
fn condition_ui(ui: &mut Ui, id: egui::Id, condition: &mut Condition) {
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
            ComboBox::from_id_salt(id.with("kind")).width(120.0).selected_text(kind.stats().name.as_str()).show_ui(ui, |ui| {
                for &b in BUILDINGS {
                    ui.selectable_value(kind, b, b.stats().name.as_str());
                }
            });
        }
        Condition::Destroyed { name_id } => {
            ui.label("Name ID");
            text_field(ui, name_id, false);
        }
        Condition::Variable { name, at_least } => {
            text_field(ui, name, false);
            ui.add(DragValue::new(at_least).range(-1_000_000..=1_000_000).prefix("at least "));
        }
        Condition::EventDone { name } => {
            ui.label("Event");
            text_field(ui, name, false);
        }
    });
}

