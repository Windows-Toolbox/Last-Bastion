//! The **Triggers** page (`docs\editor.md`, step 5): SC2's trigger editor with TAB's game narrative in it. Three columns:
//! on the left the tree (the story's goals, button and variables; the triggers in folders; the things with scripts), in
//! the middle the chosen trigger – its event, its conditions, its actions as sentences (`story_panel\sentences.rs`) –
//! on the right the palette of actions: a click adds one at the end of the script last asked for, dragging puts it
//! between two steps.
//!
//! The page works on a copy of the story; if the copy differs at the end of the frame, one order sets it
//! (`Order::SetStory`), so every change is one step of undo.

use egui_macroquad::egui::{self, Align2, CollapsingHeader, Color32, DragAndDrop, Frame, RichText, ScrollArea, Sense, Ui};

use super::{settled_text, Editor, Places};
use crate::sim::{ExprError, Order, Script, Story, Trigger, TriggerEvent, World};
use crate::story_panel::{
    action_help, add_from_palette, code_field, command_section, event_label, functions_help, goals_ui, text_field, trigger_body, trigger_event_ui,
    variables_section, GROUPS,
};

/// What the Triggers page keeps between frames.
#[derive(Default)]
pub struct TriggersPage {
    /// What the middle shows (none yet: the first trigger, or the goals).
    node: Option<Node>,
    /// The tree's search field and the palette's.
    search: String,
    palette_search: String,
    /// The middle shows the trigger as text instead of sentences.
    text: bool,
}

/// What the tree can point at.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Node {
    Trigger(usize),
    Goals,
    Button,
    Variables,
    Functions,
}

/// How many steps a script has, the ones inside included.
fn steps_in(script: &Script) -> usize {
    script
        .iter()
        .map(|step| {
            let mut inner = 0;
            let mut k = 0;
            while let Some(child) = step.action.child(k) {
                inner += steps_in(child);
                k += 1;
            }
            1 + inner
        })
        .sum()
}

/// What is wrong with a trigger, if anything: an event or a thing it names that isn't on the map, an expression that
/// no longer fits the variables.
pub fn problems(world: &World, t: &Trigger) -> Vec<String> {
    let mut found = Vec::new();
    match &t.event {
        TriggerEvent::EventComes { event } | TriggerEvent::EventDone { event } if !world.events.iter().any(|e| e.name == *event) => {
            found.push(format!("No event is named \"{event}\" (Events)."));
        }
        TriggerEvent::Join { name_id } if !has_thing(world, name_id) => found.push(format!("No neutral thing has the name ID \"{name_id}\" (Objects).")),
        _ => {}
    }
    if world.story.misfits().contains(&format!("trigger \"{}\"", t.name)) {
        found.push("An expression doesn't fit the variables any more (Variables).".to_owned());
    }
    found
}

/// Does a neutral thing with this name ID stand on the map?
fn has_thing(world: &World, name_id: &str) -> bool {
    world.neutral.iter().any(|n| n.tag.as_ref().is_some_and(|t| t.name_id == name_id))
}

impl Editor {
    /// The page: tree, trigger, palette – in one frame, on a copy of the story.
    pub(super) fn triggers_page(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        let mut story: Story = world.story.clone();
        let places = Places::of(world);
        let page = &mut self.triggers_page;
        // A removed trigger can't stay chosen; nothing chosen yet: the first trigger, or the goals.
        let node = match page.node {
            Some(Node::Trigger(i)) if i >= story.triggers.len() => None,
            node => node,
        };
        let node = node.unwrap_or(if story.triggers.is_empty() { Node::Goals } else { Node::Trigger(0) });
        page.node = Some(node);

        egui::SidePanel::left("trigger tree").default_width(270.0).width_range(200.0..=520.0).show_inside(ui, |ui| {
            ScrollArea::vertical().id_salt("trigger tree").auto_shrink([false, false]).show(ui, |ui| tree(ui, page, &mut story, world));
        });
        egui::SidePanel::right("trigger palette").default_width(220.0).width_range(160.0..=400.0).show_inside(ui, |ui| {
            palette(ui, page, matches!(page.node, Some(Node::Trigger(_))));
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ScrollArea::vertical().id_salt("trigger page").auto_shrink([false, false]).show(ui, |ui| middle(ui, page, &mut story, world, places));
        });
        // The palette's click was meant for the script that took it; if none did (no trigger shown), it goes.
        ui.ctx().data_mut(|data| data.remove_temp::<&'static str>(egui::Id::new("palette pending")));
        dragged_action(ui.ctx());
        if story != world.story {
            orders.push(Order::SetStory { story });
        }
    }

    /// The line for the status bar: the chosen trigger, its conditions and actions, and whether anything is wrong.
    pub(super) fn trigger_status(&self, world: &World) -> String {
        match self.triggers_page.node {
            Some(Node::Trigger(i)) => match world.story.triggers.get(i) {
                Some(t) => {
                    let wrong = problems(world, t);
                    let state = if wrong.is_empty() { "No errors".to_owned() } else { format!("{} problem(s)", wrong.len()) };
                    format!("Trigger: {} - {} conditions - {} actions - {state}", t.name, t.conditions.len(), steps_in(&t.actions))
                }
                None => "Trigger: -".to_owned(),
            },
            _ => format!("{} triggers", world.story.triggers.len()),
        }
    }
}

/// The left column: search, "+ Trigger", the story's own parts, the triggers in folders, the things with scripts.
fn tree(ui: &mut Ui, page: &mut TriggersPage, story: &mut Story, world: &World) {
    ui.add(egui::TextEdit::singleline(&mut page.search).hint_text("Search").desired_width(f32::INFINITY));
    let search = page.search.trim().to_lowercase();
    if ui.button("+ Trigger").on_hover_text("A new trigger - it fires when the round starts, change its event").clicked() {
        story.triggers.push(Trigger::new(&format!("Trigger {}", story.triggers.len() + 1), TriggerEvent::Start));
        page.node = Some(Node::Trigger(story.triggers.len() - 1));
    }
    ui.add_space(4.0);
    ui.label(RichText::new("Story").strong());
    let command = if story.command.is_some() { "Command center button (on)" } else { "Command center button" };
    for (node, text) in [
        (Node::Goals, format!("Goals ({})", story.goals.len())),
        (Node::Button, command.to_owned()),
        (Node::Variables, format!("Variables ({})", story.variables.len())),
        (Node::Functions, "Functions".to_owned()),
    ] {
        if (search.is_empty() || text.to_lowercase().contains(&search)) && ui.selectable_label(page.node == Some(node), text).clicked() {
            page.node = Some(node);
        }
    }
    ui.add_space(4.0);
    ui.label(RichText::new("Triggers").strong());
    let shown = |t: &Trigger| search.is_empty() || [t.name.as_str(), t.folder.as_str(), &event_label(&t.event)].iter().any(|s| s.to_lowercase().contains(&search));
    let mut folders: Vec<&str> = Vec::new();
    for t in &story.triggers {
        if !folders.contains(&t.folder.as_str()) {
            folders.push(&t.folder);
        }
    }
    // Triggers without a folder first.
    folders.sort_by_key(|f| !f.is_empty());
    let mut chosen = None;
    for folder in folders {
        let mut rows = |ui: &mut Ui| {
            for (i, t) in story.triggers.iter().enumerate().filter(|(_, t)| t.folder == folder && shown(t)) {
                let mark = if world_problem(world, t) { " !" } else { "" };
                let text = format!("{}{}{mark}", if t.on { "" } else { "(off) " }, t.name);
                if ui.selectable_label(page.node == Some(Node::Trigger(i)), text).on_hover_text(event_label(&t.event)).clicked() {
                    chosen = Some(Node::Trigger(i));
                }
            }
        };
        if folder.is_empty() {
            rows(ui);
        } else {
            let count = story.triggers.iter().filter(|t| t.folder == folder).count();
            CollapsingHeader::new(format!("{folder} ({count})")).id_salt(("trigger folder", folder)).default_open(true).show(ui, rows);
        }
    }
    if story.triggers.is_empty() {
        ui.label(RichText::new("None yet.").weak());
    }
    // Every neutral thing with a name ID, and every name ID a trigger waits for: TAB's scripts on objects, found again.
    ui.add_space(4.0);
    ui.label(RichText::new("Object scripts").strong()).on_hover_text("Neutral things (Objects) that have a name ID - and the triggers that wait for them to join");
    let mut ids: Vec<String> = world.neutral.iter().filter_map(|n| n.tag.as_ref()).map(|t| t.name_id.clone()).filter(|id| !id.is_empty()).collect();
    for t in &story.triggers {
        if let TriggerEvent::Join { name_id } = &t.event
            && !ids.contains(name_id)
        {
            ids.push(name_id.clone());
        }
    }
    for id in ids.iter().filter(|id| search.is_empty() || id.to_lowercase().contains(&search)) {
        let mine: Vec<usize> =
            story.triggers.iter().enumerate().filter(|(_, t)| matches!(&t.event, TriggerEvent::Join { name_id } if name_id == id)).map(|(i, _)| i).collect();
        let gone = !has_thing(world, id);
        let text = if gone { RichText::new(format!("{id} ({}) - no such thing", mine.len())).color(Color32::LIGHT_RED) } else { RichText::new(format!("{id} ({})", mine.len())) };
        if ui.selectable_label(mine.iter().any(|&i| page.node == Some(Node::Trigger(i))), text).on_hover_text("Click: its trigger when it joins - made if there is none").clicked() {
            chosen = Some(match mine.first() {
                Some(&i) => Node::Trigger(i),
                None => {
                    story.triggers.push(Trigger::new(&format!("{id} joins"), TriggerEvent::Join { name_id: id.clone() }));
                    Node::Trigger(story.triggers.len() - 1)
                }
            });
        }
    }
    if let Some(node) = chosen {
        page.node = Some(node);
    }
}

/// Is something wrong with the trigger (for the "!" in the tree)?
fn world_problem(world: &World, t: &Trigger) -> bool {
    !problems(world, t).is_empty()
}

/// The middle: what the tree points at.
fn middle(ui: &mut Ui, page: &mut TriggersPage, story: &mut Story, world: &World, places: Places) {
    match page.node.unwrap_or(Node::Goals) {
        Node::Trigger(i) => trigger_page(ui, page, story, i, world, places),
        Node::Goals => {
            ui.heading("Goals");
            ui.label(RichText::new("All victory goals reached: the round is won - any defeat goal reached: lost").small().weak());
            goals_ui(ui, &mut story.goals, places);
        }
        Node::Button => {
            ui.heading("Command center button");
            ui.label(RichText::new("A button of its own on the command center (V) - what it does: the triggers with the event Button").small().weak());
            command_section(ui, &mut story.command, places);
        }
        Node::Variables => {
            ui.heading("Variables");
            ui.label(RichText::new("Numbers, yes/no or texts the triggers change and ask - their value at the start").small().weak());
            variables_section(ui, story, world);
        }
        Node::Functions => {
            ui.heading("Functions");
            ui.label(RichText::new("What the conditions and the code can use - names in any case").small().weak());
            functions_help(ui);
        }
    }
}

/// One trigger: the head (on, name, buttons), when, folder, then the sentences – or the text.
fn trigger_page(ui: &mut Ui, page: &mut TriggersPage, story: &mut Story, i: usize, world: &World, places: Places) {
    let Some(t) = story.triggers.get_mut(i) else {
        return;
    };
    let id = egui::Id::new(("trigger", i));
    let (mut remove, mut duplicate, mut shift) = (false, false, 0i32);
    ui.horizontal(|ui| {
        ui.checkbox(&mut t.on, "").on_hover_text("Off: it never fires");
        if let Some(name) = settled_text(ui, &t.name, false).filter(|n| !n.is_empty()) {
            t.name = name;
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut page.text, false, "Sentences");
        ui.selectable_value(&mut page.text, true, "Text").on_hover_text("The trigger the way the map file keeps it - change it and click elsewhere");
        ui.separator();
        shift -= i32::from(ui.small_button("Earlier").on_hover_text("It fires before the one above (among those of the same event)").clicked());
        shift += i32::from(ui.small_button("Later").on_hover_text("It fires after the one below").clicked());
        duplicate = ui.small_button("Duplicate").clicked();
        remove = ui.small_button("Delete").on_hover_text("Undo brings it back").clicked();
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("When").weak());
        trigger_event_ui(ui, id, t);
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("Folder").weak());
        text_field(ui, &mut t.folder, false);
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("Note").weak());
        text_field(ui, &mut t.comment, false);
    });
    for problem in problems(world, t) {
        ui.label(RichText::new(problem).small().color(Color32::LIGHT_RED));
    }
    ui.separator();
    if page.text {
        let shown = serde_json::to_string_pretty(t).unwrap_or_default();
        let hint = "{ \"name\": ... }";
        let parse = |text: &str| serde_json::from_str::<Trigger>(text).map_err(|e| ExprError { at: e.column().saturating_sub(1), msg: e.to_string() });
        if let Some(new) = code_field(ui, id.with("text"), &shown, hint, true, parse) {
            *t = new;
        }
    } else {
        trigger_body(ui, id, t, places);
    }
    // Done after the fields: the list can't change while they hold a trigger.
    if remove {
        story.triggers.remove(i);
        page.node = None;
    } else if duplicate {
        let mut copy = story.triggers[i].clone();
        copy.name = format!("{} copy", copy.name);
        story.triggers.insert(i + 1, copy);
        page.node = Some(Node::Trigger(i + 1));
    } else if shift != 0 {
        let to = (i as i32 + shift).clamp(0, story.triggers.len() as i32 - 1) as usize;
        story.triggers.swap(i, to);
        page.node = Some(Node::Trigger(to));
    }
}

/// The right column: the kinds of actions in groups, with a search field.
fn palette(ui: &mut Ui, page: &mut TriggersPage, on: bool) {
    ui.label(RichText::new("Actions").strong());
    ui.add(egui::TextEdit::singleline(&mut page.palette_search).hint_text("Search").desired_width(f32::INFINITY));
    ui.label(RichText::new(if on { "Click: at the end - drag: between the steps" } else { "Choose a trigger first" }).small().weak());
    let search = page.palette_search.trim().to_lowercase();
    ScrollArea::vertical().id_salt("palette").auto_shrink([false, false]).show(ui, |ui| {
        ui.add_enabled_ui(on, |ui| {
            for (group, names) in GROUPS {
                let names: Vec<&&str> = names.iter().filter(|n| search.is_empty() || n.to_lowercase().contains(&search) || group.to_lowercase().contains(&search)).collect();
                if names.is_empty() {
                    continue;
                }
                ui.add_space(2.0);
                ui.label(RichText::new(group).small().weak());
                for &&name in &names {
                    let button = egui::Button::new(name).sense(Sense::click_and_drag()).min_size(egui::vec2(ui.available_width(), 0.0));
                    let response = ui.add(button).on_hover_text(action_help(name));
                    response.dnd_set_drag_payload(name);
                    if response.clicked() {
                        add_from_palette(ui.ctx(), name);
                    }
                }
            }
        });
    });
}

/// The action being dragged, a small box at the mouse.
fn dragged_action(ctx: &egui::Context) {
    let Some(name) = DragAndDrop::payload::<&'static str>(ctx) else {
        return;
    };
    let Some(at) = ctx.pointer_latest_pos() else {
        return;
    };
    egui::Area::new(egui::Id::new("dragged action")).order(egui::Order::Tooltip).fixed_pos(at + egui::vec2(12.0, 6.0)).pivot(Align2::LEFT_TOP).interactable(false).show(ctx, |ui| {
        Frame::popup(ui.style()).show(ui, |ui| ui.label(*name));
    });
}
