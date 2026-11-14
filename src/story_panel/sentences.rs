//! A script as sentences (SC2's way of showing a trigger): every step is one line of words – "Send 50 Runner zombies,
//! storming the command center" – and the blue ones can be clicked: the field to change it opens right under the line.
//! Block, loop, random and choose have their steps indented under them.
//!
//! One function draws a step (`sentence`), one edits a word of it (`edit_action`): both go by the same word numbers,
//! so what's drawn is what can be changed. The scripts are edited in place (`&mut Script`); the caller compares with
//! the original and sends one order.
//!
//! New steps come from the "+ action" menu at the end of every script, or from the palette of the Triggers page
//! (click: at the end of the script last asked for, drag: between the steps – `PENDING`, `TARGET`).

use egui_macroquad::egui::{self, Color32, CursorIcon, DragAndDrop, DragValue, Frame, Id, RichText, Sense, Ui};

use super::{code_field, text_field, when_field};
use crate::editor::{from_label, wave_group, Places, MAX_AMOUNT};
use crate::kinds::{Resource, UnitKind};
use crate::sim::{
    code_text, parse_code, parse_expr, Action, BreedCount, Expr, Pick, Scope, Script, Step, Ty, Weather, Weighted, WaveFrom, WaveGroup,
};

/// The kinds of actions the palette and the "+ action" menu offer, in groups – each name is one of `action_name`'s.
pub const GROUPS: [(&str, &[&str]); 5] = [
    ("Flow", &["Block", "Loop", "Exit loop", "Wait", "Random", "Choose", "Code"]),
    ("Zombies", &["Zombies"]),
    ("Colony", &["Resources", "Units", "Infect houses", "Join", "Train"]),
    ("Show", &["Notify", "Window", "Reveal", "Weather"]),
    ("End", &["Win", "Lose"]),
];

/// What the Triggers page keeps in egui's memory for the scripts: the action asked for in the palette (taken by the
/// script it belongs to), the script the palette adds to (the last "+ action" clicked), and the first script of the
/// trigger shown (where the palette adds to without any click).
const PENDING: &str = "palette pending";
const TARGET: &str = "palette target";
const TOP: &str = "palette top";

/// The word numbers beyond the actions' own ones: the step's condition and its code.
const WHEN: u8 = 8;
const CODE: u8 = 9;

/// The palette asks for this action in the script it adds to.
pub fn add_from_palette(ctx: &egui::Context, name: &'static str) {
    ctx.data_mut(|data| data.insert_temp(Id::new(PENDING), name));
}

/// The trigger shown now has its first script with this id (the palette's default place).
pub fn mark_top(ui: &Ui, id: Id) {
    ui.data_mut(|data| data.insert_temp(Id::new(TOP), id));
}

/// A new action of the kind named in `GROUPS`, with values to start from.
pub fn new_action(name: &str) -> Action {
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
        "Block" => Action::Block { steps: Vec::new() },
        "Loop" => Action::Loop { steps: Vec::new() },
        "Exit loop" => Action::ExitLoop { loops: 1 },
        "Wait" => Action::Wait { seconds: Expr::number(5) },
        "Random" => Action::Random { options: vec![Weighted::new(1, Vec::new()), Weighted::new(1, Vec::new())] },
        "Choose" => Action::Choose { text: "What now?".into(), options: vec![Pick::new("Yes", Vec::new()), Pick::new("No", Vec::new())] },
        "Code" => Action::Code,
        "Join" => Action::Join { name_id: String::new() },
        "Reveal" => Action::Reveal,
        "Weather" => Action::Weather { weather: Weather::Snow },
        "Train" => Action::Train { stop: true, reverse: false, on_arrival: Vec::new() },
        "Win" => Action::Win,
        _ => Action::Lose,
    }
}

/// What each action does, for its tooltip.
pub fn action_help(name: &str) -> &'static str {
    match name {
        "Zombies" => "Zombies out of the spawners, now - storming the command center or idling where they come out",
        "Resources" => "Gold and goods for the colony - below 0 taken away",
        "Units" => "Units for the colony next to the command center",
        "Infect houses" => "So many houses with colonists, picked at random, are infected",
        "Notify" => "A line at the top of the screen",
        "Window" => "A window with a text - the game pauses until it's closed",
        "Block" => "Steps one after the other, as one - the condition and the code of the block count for all of them",
        "Loop" => "The steps again and again, until an Exit loop leaves it. With Choose and a Back button inside it makes a menu; Wait makes a timer",
        "Exit loop" => "Leaves the innermost loop - or that many loops. Outside a loop it does nothing",
        "Wait" => "Time passes before the next step (seconds)",
        "Random" => "One of the scripts below, each as likely as its weight - only those whose condition holds",
        "Choose" => "The player picks one in a window - its script runs. Options whose condition is false are left out; none left: no window",
        "Code" => "Nothing but the code below: set variables, call functions (keys += 1; GiveGold(100))",
        "Join" => "The neutral thing with this name ID joins the colony",
        "Reveal" => "The whole map is shown from now on",
        "Weather" => "Snow, rain or clear from now on - it fades in over a few seconds. A round starts with its landscape's (snow on Frozen Highlands, rain on Caustic Lands)",
        "Train" => "A train comes along the railway (Objects > Railway): on the line closest to the command center, from its end nearer the edge of the map. It runs over everyone in its way",
        "Win" => "The round is won",
        _ => "The round is lost",
    }
}

/// What a step's "..." menu or the end of a script asked for – done after the loop, a list can't change while it's
/// gone through.
enum Op {
    Remove(usize),
    Swap(usize, usize),
    Duplicate(usize),
    Insert(usize, &'static str),
}

/// A script in the editor: a sentence per step, "+ action" at the end. Works on `script` in place. `id`: egui tells the
/// fields of different scripts apart by it.
pub fn script_ui(ui: &mut Ui, id: Id, script: &mut Script, places: Places) {
    let mut op: Option<Op> = None;
    let dragging = DragAndDrop::has_payload_of_type::<&'static str>(ui.ctx());
    let len = script.len();
    for (k, step) in script.iter_mut().enumerate() {
        // While an action from the palette is dragged, a place to drop it between every two steps.
        if dragging {
            drop_place(ui, k, &mut op);
        }
        step_ui(ui, id.with(k), k, len, step, places, &mut op);
    }
    // The end: drop place and "+ action" in one.
    let target = ui.data(|data| data.get_temp::<Id>(Id::new(TARGET)));
    let top = ui.data(|data| data.get_temp::<Id>(Id::new(TOP)));
    let is_target = target.map_or(top == Some(id), |t| t == id);
    if is_target && let Some(name) = ui.data_mut(|data| data.remove_temp::<&'static str>(Id::new(PENDING))) {
        op = Some(Op::Insert(len, name));
    }
    let (_, dropped) = ui.dnd_drop_zone::<&'static str, _>(Frame::group(ui.style()).inner_margin(2.0), |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            let menu = ui.menu_button(RichText::new("+ action").color(link_color(ui)), |ui| add_menu(ui, len, &mut op));
            if menu.response.clicked() {
                ui.data_mut(|data| data.insert_temp(Id::new(TARGET), id));
            }
            if is_target && !dragging {
                ui.label(RichText::new("< the palette adds here").small().weak());
            } else if len == 0 && !dragging {
                ui.label(RichText::new("nothing here yet").small().weak());
            }
            if dragging {
                ui.label(RichText::new("drop it here").small().weak());
            }
        });
    });
    if let Some(name) = dropped {
        op = Some(Op::Insert(len, *name));
    }
    let Some(op) = op else {
        return;
    };
    // A step moved or went: what was open belongs to the wrong one now.
    for k in 0..len {
        ui.data_mut(|data| data.remove::<Option<u8>>(id.with(k).with("open")));
    }
    match op {
        Op::Remove(k) => {
            script.remove(k);
        }
        Op::Swap(a, b) => script.swap(a, b),
        Op::Duplicate(k) => {
            let copy = script[k].clone();
            script.insert(k + 1, copy);
        }
        Op::Insert(at, name) => script.insert(at.min(len), Step::new(new_action(name))),
    }
}

/// The "+ action" menu: every kind of action, by group.
fn add_menu(ui: &mut Ui, at: usize, op: &mut Option<Op>) {
    for (group, names) in GROUPS {
        ui.label(RichText::new(group).small().weak());
        for &name in names {
            if ui.button(name).on_hover_text(action_help(name)).clicked() {
                *op = Some(Op::Insert(at, name));
                ui.close_menu();
            }
        }
    }
}

/// A place between two steps where a dragged action can be dropped.
fn drop_place(ui: &mut Ui, at: usize, op: &mut Option<Op>) {
    let (_, dropped) = ui.dnd_drop_zone::<&'static str, _>(Frame::group(ui.style()).inner_margin(1.0), |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new("insert here").small().weak());
    });
    if let Some(name) = dropped {
        *op = Some(Op::Insert(at, *name));
    }
}

/// The colour of the clickable words.
fn link_color(ui: &Ui) -> Color32 {
    ui.visuals().hyperlink_color
}

/// A clickable word: `open` is the number of the word whose field is open (none: no field) – a click opens this
/// one's, or closes it.
fn word(ui: &mut Ui, open: &mut Option<u8>, number: u8, text: &str) {
    let on = *open == Some(number);
    let mut shown = RichText::new(text).color(link_color(ui));
    if on {
        shown = shown.underline().strong();
    }
    let response = ui.add(egui::Label::new(shown).sense(Sense::click())).on_hover_cursor(CursorIcon::PointingHand);
    if response.clicked() {
        *open = if on { None } else { Some(number) };
    }
}

/// A text for a word, never longer than `most` letters.
fn short(text: &str, most: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= most {
        text.to_owned()
    } else {
        format!("{}...", text.chars().take(most.saturating_sub(3)).collect::<String>())
    }
}

/// `"text"` for a word – "(empty)" for none.
fn quote(text: &str) -> String {
    if text.trim().is_empty() { "(empty)".to_owned() } else { format!("\"{}\"", short(text, 40)) }
}

/// "50 Runner, 10-20 Jumper from 1 random side" – and for several groups, one after the other.
fn groups_text(groups: &[WaveGroup]) -> String {
    if groups.is_empty() {
        return "no".to_owned();
    }
    let one = |g: &WaveGroup| {
        let breeds: Vec<String> = g
            .breeds
            .iter()
            .map(|b| if b.up_to > b.count { format!("{}-{} {}", b.count, b.up_to, b.breed.name()) } else { format!("{} {}", b.count, b.breed.name()) })
            .collect();
        format!("{} from {}", breeds.join(" + "), from_label(&g.from))
    };
    short(&groups.iter().map(one).collect::<Vec<_>>().join(", and "), 70)
}

/// One step: the sentence (with the "..." menu), the field of the word that's open, the step's code, and the steps of
/// its children below.
#[allow(clippy::too_many_arguments)]
fn step_ui(ui: &mut Ui, id: Id, k: usize, len: usize, step: &mut Step, places: Places, op: &mut Option<Op>) {
    let open_id = id.with("open");
    let mut open = ui.data(|data| data.get_temp::<Option<u8>>(open_id)).flatten();
    ui.horizontal_wrapped(|ui| {
        ui.menu_button("...", |ui| {
            if k > 0 && ui.button("Move up").clicked() {
                *op = Some(Op::Swap(k - 1, k));
                ui.close_menu();
            }
            if k + 1 < len && ui.button("Move down").clicked() {
                *op = Some(Op::Swap(k, k + 1));
                ui.close_menu();
            }
            if ui.button("Duplicate").clicked() {
                *op = Some(Op::Duplicate(k));
                ui.close_menu();
            }
            if step.when.is_none() && ui.button("+ condition").on_hover_text("False: the action and its code are skipped").clicked() {
                step.when = Some(Expr::yes());
                open = Some(WHEN);
                ui.close_menu();
            }
            if step.code.is_empty() && ui.button("+ code").on_hover_text("Runs after the action").clicked() {
                open = Some(CODE);
                ui.close_menu();
            }
            if ui.button("Remove").clicked() {
                *op = Some(Op::Remove(k));
                ui.close_menu();
            }
        });
        ui.label(RichText::new(format!("{}.", k + 1)).weak());
        sentence(ui, &step.action, &mut open);
        if let Some(when) = &step.when {
            word(ui, &mut open, WHEN, &format!("if {}", short(&when.to_string(), 50)));
        }
    });
    let scope = Scope { variables: places.variables };
    if let Some(w) = open {
        Frame::group(ui.style()).inner_margin(4.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            match w {
                WHEN => when_field(ui, id.with("if"), &mut step.when, scope, "condition - empty: always"),
                CODE => code_row(ui, id, step, scope),
                _ => edit_action(ui, id, &mut step.action, w, places),
            }
        });
    }
    // The code that runs after the action (and after the children).
    let is_code = matches!(step.action, Action::Code);
    ui.data_mut(|data| data.insert_temp(open_id, open));
    ui.indent(id.with("indent"), |ui| {
        children(ui, id, &mut step.action, places);
        if !step.code.is_empty() || is_code {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("then").weak());
                let text = if step.code.is_empty() { "(write the code)".to_owned() } else { short(&code_text(&step.code), 70) };
                let mut open_code = open;
                word(ui, &mut open_code, CODE, &text);
                if open_code != open {
                    ui.data_mut(|data| data.insert_temp(open_id, open_code));
                }
            });
        }
    });
}

/// The field of a step's code.
fn code_row(ui: &mut Ui, id: Id, step: &mut Step, scope: Scope) {
    let shown = code_text(&step.code);
    if let Some(new) = code_field(ui, id.with("code"), &shown, "code, e.g. keys += 1; GiveGold(100)", true, |t| parse_code(t, scope)) {
        step.code = new;
    }
}

/// The sentence of an action; its blue words are numbered for `edit_action`.
fn sentence(ui: &mut Ui, action: &Action, open: &mut Option<u8>) {
    match action {
        Action::Zombies { groups, dispersion, storm } => {
            ui.label("Send");
            word(ui, open, 0, &groups_text(groups));
            ui.label("zombies,");
            word(ui, open, 1, if *storm { "storming the command center" } else { "idling where they come out" });
            word(ui, open, 2, &format!("spread {dispersion} tiles"));
        }
        Action::Resources { gold, goods } => {
            ui.label("Give");
            word(ui, open, 0, &format!("{gold} gold"));
            ui.label("and");
            let goods = goods.iter().map(|(r, n)| format!("{n} {}", r.name())).collect::<Vec<_>>().join(", ");
            word(ui, open, 1, if goods.is_empty() { "no goods" } else { &goods });
        }
        Action::Units { units } => {
            ui.label("Give");
            let units = units.iter().map(|(u, n)| format!("{n} {}", u.stats().name)).collect::<Vec<_>>().join(", ");
            word(ui, open, 0, if units.is_empty() { "no units" } else { &units });
        }
        Action::InfectHouses { count } => {
            ui.label("Infect");
            word(ui, open, 0, &format!("{count} houses"));
        }
        Action::Notify { text } => {
            ui.label("Notify");
            word(ui, open, 0, &quote(text));
        }
        Action::Window { title, text } => {
            ui.label("Window");
            word(ui, open, 0, &quote(title));
            ui.label(":");
            word(ui, open, 1, &quote(text));
        }
        Action::ExitLoop { loops } => {
            ui.label("Exit");
            word(ui, open, 0, &format!("{loops} loop(s)"));
        }
        Action::Wait { seconds } => {
            ui.label("Wait");
            word(ui, open, 0, &short(&seconds.to_string(), 30));
            ui.label("seconds");
        }
        Action::Choose { text, .. } => {
            ui.label("Ask");
            word(ui, open, 0, &quote(text));
        }
        Action::Join { name_id } => {
            word(ui, open, 0, &if name_id.is_empty() { "(name ID)".to_owned() } else { name_id.clone() });
            ui.label("joins the colony");
        }
        Action::Weather { weather } => {
            ui.label("Weather becomes");
            word(ui, open, 0, weather.name());
        }
        Action::Train { stop, reverse, .. } => {
            ui.label("A train comes");
            word(ui, open, 0, if *reverse { "from the other end" } else { "from the usual end" });
            word(ui, open, 1, if *stop { "and stops at the command center" } else { "and drives through" });
        }
        Action::Block { .. } => {
            ui.label("Block:");
        }
        Action::Loop { .. } => {
            ui.label("Loop:");
        }
        Action::Random { .. } => {
            ui.label("Random, one of:");
        }
        Action::Code => {
            ui.label("Run code");
        }
        Action::Reveal => {
            ui.label("Reveal the whole map");
        }
        Action::Win => {
            ui.label("Win the round");
        }
        Action::Lose => {
            ui.label("Lose the round");
        }
    }
}

/// The field of word `w` of an action.
fn edit_action(ui: &mut Ui, id: Id, action: &mut Action, w: u8, places: Places) {
    let scope = Scope { variables: places.variables };
    match (action, w) {
        (Action::Zombies { groups, .. }, 0) => {
            let mut gone = None;
            for (g, group) in groups.iter_mut().enumerate() {
                if wave_group(ui, (id, g), group, places) {
                    gone = Some(g);
                }
            }
            if let Some(g) = gone {
                groups.remove(g);
            }
            if ui.small_button("+ group").clicked() {
                groups.push(WaveGroup { from: WaveFrom::Sides(1), breeds: vec![BreedCount::new(50, crate::sim::Breed::Runner)] });
            }
        }
        (Action::Zombies { storm, .. }, 1) => {
            ui.radio_value(storm, true, "storm the command center");
            ui.radio_value(storm, false, "idle where they come out");
        }
        (Action::Zombies { dispersion, .. }, 2) => {
            ui.add(DragValue::new(dispersion).range(0..=200).suffix(" tiles"));
            ui.label(RichText::new("How far apart along the edge they come out.").small().weak());
        }
        (Action::Resources { gold, .. }, 0) => {
            ui.add(DragValue::new(gold).range(-(MAX_AMOUNT as i32)..=MAX_AMOUNT as i32).speed(10).suffix(" gold"));
        }
        (Action::Resources { goods, .. }, 1) => {
            let mut gone = None;
            for (k, (resource, n)) in goods.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt(id.with(("good", k))).width(90.0).selected_text(resource.name()).show_ui(ui, |ui| {
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
        (Action::Units { units }, 0) => {
            let mut gone = None;
            for (k, (kind, n)) in units.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(n).range(1..=100));
                    egui::ComboBox::from_id_salt(id.with(("unit", k))).width(110.0).selected_text(kind.stats().name.as_str()).show_ui(ui, |ui| {
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
        (Action::InfectHouses { count }, 0) => {
            ui.add(DragValue::new(count).range(1..=100).suffix(" houses"));
        }
        (Action::Notify { text }, 0) => text_field(ui, text, false),
        (Action::Window { title, .. }, 0) => text_field(ui, title, false),
        (Action::Window { text, .. }, 1) => text_field(ui, text, true),
        (Action::ExitLoop { loops }, 0) => {
            ui.add(DragValue::new(loops).range(1..=100).suffix(" loop(s)")).on_hover_text("How many loops to leave - 1: the innermost");
        }
        (Action::Wait { seconds }, 0) => {
            let shown = seconds.to_string();
            if let Some(new) = code_field(ui, id.with("seconds"), &shown, "a number, e.g. 5 or Random(3, 8)", false, |t| parse_expr(t, scope, Ty::Number)) {
                *seconds = new;
            }
        }
        (Action::Choose { text, .. }, 0) => text_field(ui, text, true),
        (Action::Join { name_id }, 0) => {
            text_field(ui, name_id, false);
            ui.label(RichText::new("The name ID of a neutral thing (Objects).").small().weak());
        }
        (Action::Weather { weather }, 0) => {
            ui.horizontal(|ui| {
                for w in Weather::ALL {
                    ui.selectable_value(weather, w, w.name());
                }
            });
        }
        (Action::Train { reverse, .. }, 0) => {
            ui.radio_value(reverse, false, "from the end of the line nearer the edge of the map");
            ui.radio_value(reverse, true, "from the other end");
        }
        (Action::Train { stop, .. }, 1) => {
            ui.radio_value(stop, true, "stops at the command center").on_hover_text("It waits there 3 s - then the script below runs (TAB unloads colonists there)");
            ui.radio_value(stop, false, "drives through");
        }
        _ => {}
    }
}

/// The scripts inside an action, indented under its sentence.
fn children(ui: &mut Ui, id: Id, action: &mut Action, places: Places) {
    let scope = Scope { variables: places.variables };
    match action {
        Action::Block { steps } => script_ui(ui, id.with("steps"), steps, places),
        Action::Loop { steps } => {
            script_ui(ui, id.with("steps"), steps, places);
            if steps.is_empty() {
                ui.label(RichText::new("An empty loop does nothing.").small().weak());
            }
        }
        Action::Random { options } => {
            let mut gone = None;
            for (k, option) in options.iter_mut().enumerate() {
                let oid = id.with(("random", k));
                let mut open = ui.data(|data| data.get_temp::<Option<u8>>(oid.with("open"))).flatten();
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("x").on_hover_text("Remove this option").clicked() {
                        gone = Some(k);
                    }
                    ui.label("Option, weight");
                    word(ui, &mut open, 0, &option.weight.to_string());
                    let when = option.when.as_ref().map_or("+ condition".to_owned(), |w| format!("if {}", short(&w.to_string(), 50)));
                    word(ui, &mut open, 1, &when);
                });
                if let Some(w) = open {
                    Frame::group(ui.style()).inner_margin(4.0).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        match w {
                            0 => {
                                ui.add(DragValue::new(&mut option.weight).range(0..=1000).prefix("weight "));
                            }
                            _ => when_field(ui, oid.with("if"), &mut option.when, scope, "condition - empty: always in the draw"),
                        }
                    });
                }
                ui.data_mut(|data| data.insert_temp(oid.with("open"), open));
                ui.indent(oid.with("indent"), |ui| script_ui(ui, oid, &mut option.steps, places));
            }
            if let Some(k) = gone {
                options.remove(k);
            }
            if ui.small_button("+ option").clicked() {
                options.push(Weighted::new(1, Vec::new()));
            }
        }
        Action::Choose { options, .. } => {
            let mut gone = None;
            for (k, option) in options.iter_mut().enumerate() {
                let oid = id.with(("choose", k));
                let mut open = ui.data(|data| data.get_temp::<Option<u8>>(oid.with("open"))).flatten();
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("x").on_hover_text("Remove this option").clicked() {
                        gone = Some(k);
                    }
                    ui.label("Button");
                    word(ui, &mut open, 0, &quote(&option.shown(k)));
                    let when = option.when.as_ref().map_or("+ condition".to_owned(), |w| format!("if {}", short(&w.to_string(), 50)));
                    word(ui, &mut open, 1, &when);
                });
                if let Some(w) = open {
                    Frame::group(ui.style()).inner_margin(4.0).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        match w {
                            0 => {
                                text_field(ui, &mut option.label, false);
                                ui.label(RichText::new("Empty, with Exit loop as its first step: the Back button.").small().weak());
                            }
                            _ => when_field(ui, oid.with("if"), &mut option.when, scope, "condition - empty: always shown"),
                        }
                    });
                }
                ui.data_mut(|data| data.insert_temp(oid.with("open"), open));
                ui.indent(oid.with("indent"), |ui| script_ui(ui, oid, &mut option.steps, places));
            }
            if let Some(k) = gone {
                options.remove(k);
            }
            if ui.small_button("+ option").on_hover_text("An option with an empty button text whose script starts with Exit loop is the Back button").clicked() {
                options.push(Pick::new("Option", Vec::new()));
            }
        }
        Action::Train { stop: true, on_arrival, .. } => {
            ui.label(RichText::new("When it's there:").weak());
            script_ui(ui, id.with("arrival"), on_arrival, places);
        }
        _ => {}
    }
}
