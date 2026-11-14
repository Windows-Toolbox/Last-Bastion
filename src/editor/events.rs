//! The **Events** module (`docs\editor.md`, step 6): TAB's events – the waves, the final wave, notices, the victory.
//!
//! The list of boxes, one per event, and "Send now" on the right; the groups of a wave (where it comes out – a random
//! side, chosen spawners or chosen regions – and how many of which breed).

use egui_macroquad::egui::{self, Align2, Color32, ComboBox, DragValue, FontId, Frame, RichText, ScrollArea, Sense, Ui};

use super::{breed_box, section_from, settled_name, Editor, Origin, Places, Wave, FINAL, MAX_AMOUNT, MAX_DAYS};
use crate::sim::{Breed, BreedCount, Event, Order, Spawner, TriggerEvent, WaveFrom, WaveGroup, World, Zone};

/// Where a group of a wave can come from at random (besides chosen spawners).
const WAVE_FROM: [WaveFrom; 5] = [WaveFrom::OneSpawner, WaveFrom::Sides(1), WaveFrom::Sides(2), WaveFrom::Sides(3), WaveFrom::Sides(4)];
/// What the Events page keeps between frames (the world has no use for it).
#[derive(Default)]
pub struct EventsPage {
    /// The event whose values are shown below the timeline (its place in `World::events`).
    selected: Option<usize>,
    /// A bar being dragged: the event and which time it comes.
    drag: Option<(usize, u32)>,
    /// The whole round on the timeline – or only its first `FIRST_DAYS`.
    all: bool,
    /// "Plan again" asked once – a second click does it.
    confirm_plan: bool,
    /// The difficulty whose zombie population "Plan again" takes – none: the map's.
    plan_difficulty: Option<String>,
}

/// The days the timeline shows without "All".
const FIRST_DAYS: u32 = 30;
/// A bar is picked this close to the mouse (pixels).
const GRAB: f32 = 8.0;
/// The timeline's rows, top to bottom, with their heights; the waves' row is the tall one.
const ROWS: [(Row, f32, &str); 5] =
    [(Row::Wave, 120.0, "Waves"), (Row::Final, 26.0, "Final"), (Row::Notice, 22.0, "Notices"), (Row::Reveal, 22.0, "Reveal"), (Row::Victory, 22.0, "Victory")];
/// The width left of the timeline for the names of its rows.
const GUTTER: f32 = 64.0;
/// The colour of the selected one's frame and texts.
const PICKED: Color32 = Color32::from_rgb(250, 220, 90);

/// The rows of the timeline.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Row {
    Wave,
    Final,
    Notice,
    Reveal,
    Victory,
}

/// A bar or dot of the timeline: the `time`-th coming of event `event` (its notice: when it's announced), at `hour`,
/// with about so many zombies.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Mark {
    event: usize,
    time: u32,
    hour: u32,
    row: Row,
    zombies: u32,
}

/// Everything the events do up to hour `last` (the round's end or the end of what's shown): per coming a bar in the
/// waves' row – the final wave in its own –, a dot for its notice, for revealing the map and for the victory. An
/// event that brings nothing and does nothing of these still gets a thin bar, so it can be clicked.
fn marks(events: &[Event], last: u32) -> Vec<Mark> {
    let mut all = Vec::new();
    for (e, event) in events.iter().enumerate() {
        for time in 0..event.repeats().unwrap_or(u32::MAX) {
            let hour = event.hour + time * event.every_hours;
            if hour > last {
                break;
            }
            let zombies = event.at_time(time);
            let bar = if event.final_wave { Row::Final } else { Row::Wave };
            if !event.groups.is_empty() || event.final_wave || !(event.reveal || event.won) {
                all.push(Mark { event: e, time, hour, row: bar, zombies });
            }
            if let Some(before) = event.notify_hours {
                all.push(Mark { event: e, time, hour: hour.saturating_sub(before), row: Row::Notice, zombies });
            }
            if event.reveal {
                all.push(Mark { event: e, time, hour, row: Row::Reveal, zombies: 0 });
            }
            if event.won {
                all.push(Mark { event: e, time, hour, row: Row::Victory, zombies: 0 });
            }
            if event.every_hours == 0 {
                break;
            }
        }
    }
    all
}

/// "Send now" on the right: send waves now – to try how the defence holds (it works in Edit too: they stand where
/// they came out until the test starts).
pub(super) fn send_now(ui: &mut Ui, orders: &mut Vec<Order>) {
    section_from(ui, "Send now", Some(Origin::New), "from the spawners", |ui| {
        // `horizontal_wrapped`: side by side, and on to the next line where the panel ends.
        ui.horizontal_wrapped(|ui| {
            for (wave, text) in [(Wave::Small, "Small"), (Wave::Side, "One side"), (Wave::TwoSides, "Two sides")] {
                if ui.button(text).clicked() {
                    orders.push(wave.order());
                }
            }
            if ui.button("+1000 from all spawners").clicked() {
                orders.push(Order::SpawnZombies { count: 1000 });
            }
            if ui.button("Remove all zombies").clicked() {
                orders.push(Order::ClearZombies);
            }
        });
    });
}

impl Editor {
    /// The Events page, TAB's list of events as a timeline: the head (how many days, events and zombies, and the
    /// buttons to add one), the timeline of the round, and below it three columns – the selected event's values, its
    /// groups, and "Plan from rules" with "Send now".
    pub(super) fn events_page(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        // A removed event can't stay selected.
        if self.events_page.selected.is_some_and(|i| i >= world.events.len()) {
            self.events_page.selected = None;
        }
        ScrollArea::vertical().id_salt("events page").auto_shrink([false, false]).show(ui, |ui| {
            self.events_head(ui, world, orders);
            self.timeline(ui, world, orders);
            ui.add_space(6.0);
            ui.columns(3, |cols| {
                match self.events_page.selected.and_then(|i| world.events.get(i).map(|e| (i, e))) {
                    Some((i, event)) => selected_event(cols, i, event, world, orders),
                    None => {
                        cols[0].label(RichText::new("Click a bar on the timeline - or add an event.").weak());
                    }
                }
                self.plan_section(&mut cols[2], world, orders);
                send_now(&mut cols[2], orders);
            });
        });
    }

    /// "N days - M events - Z zombies in all", the buttons that add one, and what the timeline shows.
    fn events_head(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        ui.horizontal_wrapped(|ui| {
            let last = world.days * 24;
            let zombies: u32 = world.events.iter().map(|e| e.total(last)).sum();
            ui.label(RichText::new(format!("{} days - {} events - about {} zombies in all", world.days, world.events.len(), zombies)).strong());
            let (day, hour) = world.clock();
            ui.label(RichText::new(format!("Now: day {day}, {hour:02}:00")).weak());
            ui.separator();
            // A day after the last one that isn't the final wave or the victory (or after now).
            let now = (world.tick / crate::sim::HOUR_TICKS) as u32;
            let next = world.events.iter().filter(|e| !e.final_wave && !e.won).map(|e| e.hour).max().unwrap_or(0).max(now) + 24;
            let new = world.events.len();
            let name = |what: &str| format!("{what} {}", new + 1);
            let plain = |name: String, from: WaveFrom, count: u32| Event::new(&name, next, from, Breed::Runner, count);
            let mut add = None;
            if ui.button("+ Wave").on_hover_text("200 runners from a random side, announced 8 hours before").clicked() {
                add = Some(Event { notify_hours: Some(8), ..plain(name("Wave"), WaveFrom::Sides(1), 200) });
            }
            if ui.button("+ Notice").on_hover_text("Only a message on the clock - no zombies; give it a script for more").clicked() {
                add = Some(Event { notify_hours: Some(0), groups: Vec::new(), ..plain(name("Notice"), WaveFrom::Sides(1), 0) });
            }
            if ui.button("+ Reveal").on_hover_text("Shows the whole map when it comes").clicked() {
                add = Some(Event { reveal: true, groups: Vec::new(), ..plain(name("Reveal"), WaveFrom::Sides(1), 0) });
            }
            if let Some(event) = add {
                // One behind the shown days would be invisible – show everything then.
                if event.hour >= FIRST_DAYS * 24 {
                    self.events_page.all = true;
                }
                orders.push(Order::AddEvent(event));
                self.events_page.selected = Some(new);
            }
            ui.separator();
            ui.selectable_value(&mut self.events_page.all, false, format!("Days 1-{FIRST_DAYS}"));
            ui.selectable_value(&mut self.events_page.all, true, "All");
        });
    }

    /// The timeline: the round's hours left to right, a row each for waves (the bar's height is the number of
    /// zombies, on a square-root scale so small ones show), the final wave, notices, revealing the map and the
    /// victory. A click picks the nearest bar, dragging one moves its event to another hour (one order when let go).
    fn timeline(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        let shown_days = if self.events_page.all { world.days } else { world.days.min(FIRST_DAYS) };
        let last = (shown_days * 24).max(24);
        let all = marks(&world.events, last);
        let most = all.iter().map(|m| m.zombies).max().unwrap_or(1).max(1) as f32;
        let height: f32 = ROWS.iter().map(|r| r.1).sum::<f32>() + 22.0;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let (left, right) = (rect.min.x + GUTTER, rect.max.x - 8.0);
        let x_of = |hour: u32| left + (right - left) * hour as f32 / last as f32;
        let hour_at = |x: f32| (((x - left) / (right - left)).clamp(0.0, 1.0) * last as f32).round() as u32;
        painter.rect_filled(rect, 4.0, Color32::from_gray(24));
        // The rows' bands, and where each starts (top).
        let mut tops = Vec::new();
        let mut y = rect.min.y + 20.0;
        for (k, (_, h, name)) in ROWS.iter().enumerate() {
            if k % 2 == 0 {
                painter.rect_filled(egui::Rect::from_min_size(egui::pos2(rect.min.x, y), egui::vec2(rect.width(), *h)), 0.0, Color32::from_gray(30));
            }
            painter.text(egui::pos2(rect.min.x + 6.0, y + h / 2.0), Align2::LEFT_CENTER, name, FontId::proportional(13.0), Color32::from_gray(150));
            tops.push(y);
            y += h;
        }
        // The days: a line every 5 (10 on a long round), with its number on top.
        let step = if shown_days > 60 { 10 } else { 5 };
        for day in (0..=shown_days).step_by(step) {
            let x = x_of(day * 24);
            painter.line_segment([egui::pos2(x, rect.min.y + 18.0), egui::pos2(x, rect.max.y)], egui::Stroke::new(1.0_f32, Color32::from_gray(48)));
            painter.text(egui::pos2(x + 3.0, rect.min.y + 3.0), Align2::LEFT_TOP, format!("{}", day + 1), FontId::proportional(12.0), Color32::from_gray(130));
        }
        // Where "now" is in a test.
        let now = (world.tick / crate::sim::HOUR_TICKS) as u32;
        if now > 0 && now <= last {
            let x = x_of(now);
            painter.line_segment([egui::pos2(x, rect.min.y + 18.0), egui::pos2(x, rect.max.y)], egui::Stroke::new(1.0_f32, Color32::from_rgb(110, 170, 230)));
        }
        let selected = self.events_page.selected;
        let place = |m: &Mark| {
            let k = ROWS.iter().position(|r| r.0 == m.row).unwrap_or(0);
            (tops[k], ROWS[k].1)
        };
        for m in &all {
            let (top, h) = place(m);
            let x = x_of(m.hour);
            let picked = selected == Some(m.event);
            let color = match m.row {
                Row::Wave => Color32::from_rgb(126, 176, 224),
                Row::Final => FINAL,
                Row::Notice => Color32::from_rgb(230, 200, 110),
                Row::Reveal => Color32::from_rgb(150, 210, 150),
                Row::Victory => Color32::from_rgb(190, 150, 230),
            };
            match m.row {
                Row::Wave | Row::Final => {
                    // The square root: a swarm of 100 is still a third of one of 1000.
                    let bar = ((m.zombies as f32 / most).sqrt() * (h - 6.0)).clamp(4.0, h - 6.0);
                    let r = egui::Rect::from_min_max(egui::pos2(x - 2.0, top + h - 3.0 - bar), egui::pos2(x + 2.0, top + h - 3.0));
                    painter.rect_filled(r, 1.0, color);
                    if picked {
                        painter.rect_stroke(r.expand(2.0), 2.0, egui::Stroke::new(1.5_f32, PICKED), egui::StrokeKind::Outside);
                    }
                }
                _ => {
                    painter.circle_filled(egui::pos2(x, top + h / 2.0), if picked { 6.0 } else { 4.5 }, color);
                    if picked {
                        painter.circle_stroke(egui::pos2(x, top + h / 2.0), 7.5, egui::Stroke::new(1.5_f32, PICKED));
                    }
                }
            }
        }
        // The selected one's day and zombies, next to its first bar.
        if let Some(m) = all.iter().find(|m| Some(m.event) == selected && matches!(m.row, Row::Wave | Row::Final)) {
            let (top, _) = place(m);
            let text = format!("day {} - {}", m.hour / 24 + 1, m.zombies);
            painter.text(egui::pos2(x_of(m.hour) + 8.0, top + 4.0), Align2::LEFT_TOP, text, FontId::proportional(13.0), PICKED);
        }
        // Picking: the mark across nearest to the mouse – among those, the one whose row the mouse is closest to.
        let nearest = |at: egui::Pos2| -> Option<Mark> {
            let row_gap = |m: &Mark| {
                let (top, h) = place(m);
                if at.y < top {
                    top - at.y
                } else if at.y > top + h {
                    at.y - top - h
                } else {
                    0.0
                }
            };
            let key = |m: &Mark| (row_gap(m), (x_of(m.hour) - at.x).abs());
            all.iter()
                .filter(|m| (x_of(m.hour) - at.x).abs() <= GRAB)
                .min_by(|a, b| key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal))
                .copied()
        };
        if response.drag_started() || response.clicked() {
            if let Some(m) = response.interact_pointer_pos().and_then(nearest) {
                self.events_page.selected = Some(m.event);
                self.events_page.drag = (response.drag_started() && m.row != Row::Notice).then_some((m.event, m.time));
            }
        }
        if let (Some((e, time)), Some(pointer)) = (self.events_page.drag, response.interact_pointer_pos()) {
            if response.dragged() {
                let hour = hour_at(pointer.x);
                painter.line_segment([egui::pos2(pointer.x, rect.min.y + 18.0), egui::pos2(pointer.x, rect.max.y)], egui::Stroke::new(1.5_f32, PICKED));
                let text = format!("day {}, {:02}:00", hour / 24 + 1, hour % 24);
                painter.text(egui::pos2(pointer.x + 6.0, rect.max.y - 16.0), Align2::LEFT_TOP, text, FontId::proportional(13.0), PICKED);
            }
            if response.drag_stopped() {
                if let Some(event) = world.events.get(e) {
                    // This time of the event is at the mouse – so its first one is `time` repetitions earlier.
                    let hour = hour_at(pointer.x).saturating_sub(time * event.every_hours);
                    if hour != event.hour {
                        orders.push(Order::SetEvent { index: e, event: Event { hour, ..event.clone() } });
                    }
                }
            }
        }
        if response.drag_stopped() {
            self.events_page.drag = None;
        }
    }

    /// "Plan from rules": TAB's survival plan for this round's days, with a difficulty's zombie population – replaces
    /// all the events, after a second click.
    fn plan_section(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        section_from(ui, "Plan from rules", Some(Origin::Tab), "TAB's plan stretched to the days (Map), as many zombies as a difficulty says", |ui| {
            let list = &crate::data::get().difficulties.difficulties;
            let current = self.events_page.plan_difficulty.clone().unwrap_or_else(|| crate::data::get().difficulty(&world.difficulty).name.clone());
            ui.horizontal(|ui| {
                ui.label("Population of");
                ComboBox::from_id_salt("plan difficulty").width(110.0).selected_text(&current).show_ui(ui, |ui| {
                    for d in list {
                        if ui.selectable_label(d.name == current, &d.name).clicked() {
                            self.events_page.plan_difficulty = Some(d.name.clone());
                        }
                    }
                });
            });
            if self.events_page.confirm_plan {
                ui.label(RichText::new(format!("Replaces all {} events - also the ones you changed.", world.events.len())).color(FINAL));
                ui.horizontal(|ui| {
                    if ui.button("Yes, plan again").clicked() {
                        orders.push(Order::PlanEvents { difficulty: current.clone() });
                        self.events_page.confirm_plan = false;
                        self.events_page.selected = None;
                    }
                    if ui.button("Cancel").clicked() {
                        self.events_page.confirm_plan = false;
                    }
                });
            } else if ui.button(format!("Plan again (replaces all {})", world.events.len())).clicked() {
                self.events_page.confirm_plan = true;
            }
        });
    }
}

/// The selected event on the page: in the first column its values (name, when, how often, how it grows, announced or
/// silent, what it does besides) and scripts, in the second its groups. All fields work on a copy (`changed`); if it
/// differs at the end, one order sets it.
fn selected_event(cols: &mut [Ui], i: usize, event: &Event, world: &World, orders: &mut Vec<Order>) {
    let mut changed = event.clone();
    let mut removed = false;
    let mut again = false;
    let ui = &mut cols[0];
    section_from(ui, "Event", Some(Origin::Tab), "", |ui| {
        ui.horizontal(|ui| {
            // One that won't come any more is greyed out, the final wave in its colour.
            let color = if event.next_hour().is_none() {
                ui.visuals().weak_text_color()
            } else if event.final_wave {
                FINAL
            } else {
                ui.visuals().text_color()
            };
            ui.label(RichText::new(format!("{}.", i + 1)).color(color));
            if let Some(name) = settled_name(ui, &event.name) {
                changed.name = name;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Day");
            let (mut day, mut hour) = (event.hour / 24 + 1, event.hour % 24);
            let a = ui.add(DragValue::new(&mut day).range(1..=MAX_DAYS));
            let b = ui.add(DragValue::new(&mut hour).range(0..=23).suffix(":00"));
            // `|`, not `||`: both sides are worked out – here only booleans, no difference, but no skipping either.
            if a.changed() | b.changed() {
                changed.hour = (day - 1) * 24 + hour;
            }
            ui.add(DragValue::new(&mut changed.random_hours).range(0..=24 * MAX_DAYS).prefix("+ up to ").suffix(" h"))
                .on_hover_text("TAB's random offset: rolled when the round starts");
        });
        ui.horizontal_wrapped(|ui| {
            ui.add(DragValue::new(&mut changed.every_hours).range(0..=24 * MAX_DAYS).prefix("every ").suffix(" h"))
                .on_hover_text("Again every this many hours - 0: once");
            if changed.every_hours > 0 {
                ui.add(DragValue::new(&mut changed.times).range(0..=1000).suffix(" times"))
                    .on_hover_text("At most this many times - 0: as long as the round lasts");
                ui.add(DragValue::new(&mut changed.growth).range(0.0..=100.0).speed(0.05).prefix("+").suffix(" each"))
                    .on_hover_text("The n-th time brings 1 + this x (n - 1) times as many");
                ui.add(DragValue::new(&mut changed.growth_max).range(0.0..=1000.0).speed(0.1).prefix("max +"))
                    .on_hover_text("The growth stops here - 0: never");
            }
        });
        ui.horizontal_wrapped(|ui| {
            let mut announced = changed.notify_hours.is_some();
            if ui.toggle_value(&mut announced, "Announce").on_hover_text("Countdown on the clock, its swarms on the minimap - off: it comes silently").changed() {
                changed.notify_hours = announced.then_some(8);
            }
            if let Some(hours) = changed.notify_hours.as_mut() {
                ui.add(DragValue::new(hours).range(0..=240).suffix(" h before"));
            }
            ui.add(DragValue::new(&mut changed.dispersion).range(0..=200).prefix("spread ").suffix(" tiles"))
                .on_hover_text("Every side's zombies come out of a square this big at a random spot - 0: the whole side");
        });
        ui.horizontal_wrapped(|ui| {
            ui.toggle_value(&mut changed.final_wave, "Final").on_hover_text("Calls every zombie on the map");
            ui.toggle_value(&mut changed.reveal, "Reveal").on_hover_text("Shows the whole map when it comes");
            ui.toggle_value(&mut changed.won, "Wins").on_hover_text("The round is won when it comes (TAB's victory at the end of the last day)");
            ui.toggle_value(&mut changed.won_when_dead, "Wins when dead").on_hover_text("The round is won once it came every time and none of the final wave is left");
        });
        ui.horizontal(|ui| {
            again = ui.button("Send now").on_hover_text("Its zombies come at once, once - the event itself stays as it is").clicked();
            removed = ui.button("Delete").on_hover_text("Remove the event").clicked();
        });
        // Its triggers (TAB's event "Script" and "Script On Finished", `story_panel.rs`) – folded away until needed.
        // An event without a name has none: they know it by the name.
        if event.name.is_empty() {
            ui.label(RichText::new("Give it a name to attach triggers.").small().weak());
        } else {
            let comes = TriggerEvent::EventComes { event: event.name.clone() };
            let done = TriggerEvent::EventDone { event: event.name.clone() };
            crate::story_panel::linked_triggers(ui, egui::Id::new(("event comes", i)), world, orders, &comes, "When it comes");
            crate::story_panel::linked_triggers(ui, egui::Id::new(("event done", i)), world, orders, &done, "When it's done");
        }
    });
    let ui = &mut cols[1];
    section_from(ui, "Groups", Some(Origin::Tab), "where its zombies come out - and how many of which breed", |ui| {
        // The group the "x" of was clicked – taken out after the loop, a list can't change while it's gone through.
        let mut gone = None;
        // `iter_mut`: every group lent to change it in place (`&mut`).
        for (g, group) in changed.groups.iter_mut().enumerate() {
            if wave_group(ui, (i, g), group, Places::of(world)) {
                gone = Some(g);
            }
        }
        if let Some(g) = gone {
            changed.groups.remove(g);
        }
        if ui.small_button("+ group").on_hover_text("More zombies at the same time - from somewhere else").clicked() {
            let zombies = vec![BreedCount::new(100, Breed::Runner)];
            changed.groups.push(WaveGroup { from: WaveFrom::Sides(1), breeds: zombies });
        }
    });
    if removed {
        orders.push(Order::RemoveEvent { index: i });
    } else if changed != *event {
        orders.push(Order::SetEvent { index: i, event: changed });
    }
    if again {
        orders.push(Order::SendEvent { index: i });
    }
}

/// One group of an event, in a box of its own: from where (a random side, … or chosen spawners – then a tick box per
/// spawner), and a line per breed: how many (from … to, TAB's "4-6"), which one. `id`: (event, group) – egui tells the
/// lists apart by it. Returns whether its "x" was clicked.
pub(crate) fn wave_group(ui: &mut Ui, id: impl std::hash::Hash + Copy, group: &mut WaveGroup, places: Places) -> bool {
    let mut removed = false;
    Frame::group(ui.style()).inner_margin(4.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label("From");
            from_box(ui, ("wave from", id), &mut group.from);
            removed = ui.small_button("x").on_hover_text("Remove this group").clicked();
        });
        match &mut group.from {
            WaveFrom::Spawners(ids) => spawner_checks(ui, ids, places.spawners),
            WaveFrom::Zones(ids) => zone_checks(ui, ids, places.zones),
            _ => {}
        }
        let many = group.breeds.len() > 1;
        let mut gone = None;
        for (k, z) in group.breeds.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(DragValue::new(&mut z.count).range(0..=MAX_AMOUNT).speed(5));
                // "to" never below "from": the field starts at it; 0 or the same: just `count`.
                let mut to = z.up_to.max(z.count);
                if ui.add(DragValue::new(&mut to).range(z.count..=MAX_AMOUNT).speed(5).prefix("to ")).changed() {
                    z.up_to = if to > z.count { to } else { 0 };
                }
                // A triple as the list's name – one per breed of every group of every wave.
                breed_box(ui, ("wave breed", id, k), &mut z.breed);
                if many && ui.small_button("x").on_hover_text("Remove this breed").clicked() {
                    gone = Some(k);
                }
            });
        }
        if let Some(k) = gone {
            group.breeds.remove(k);
        }
        if ui.small_button("+ breed").on_hover_text("Another breed out of the same spawners").clicked() {
            // The first breed that isn't in the group yet.
            let breed = Breed::ALL.into_iter().find(|b| group.breeds.iter().all(|z| z.breed != *b)).unwrap_or(Breed::Runner);
            group.breeds.push(BreedCount::new(50, breed));
        }
    });
    removed
}

/// Where a group of a wave comes from: at random (one spawner, 1–4 sides) or chosen spawners. Chosen ones start with
/// none ticked – the group says so in red until one is.
fn from_box(ui: &mut Ui, id: impl std::hash::Hash, from: &mut WaveFrom) {
    let chosen = matches!(from, WaveFrom::Spawners(_));
    ComboBox::from_id_salt(id).width(130.0).selected_text(from_label(from)).show_ui(ui, |ui| {
        for option in WAVE_FROM {
            let label = from_label(&option);
            ui.selectable_value(from, option, label);
        }
        if ui.selectable_label(chosen, "chosen spawners").clicked() && !chosen {
            *from = WaveFrom::Spawners(Vec::new());
        }
        let in_regions = matches!(from, WaveFrom::Zones(_));
        if ui.selectable_label(in_regions, "chosen regions").on_hover_text("Anywhere in these regions (module Regions)").clicked() && !in_regions {
            *from = WaveFrom::Zones(Vec::new());
        }
    });
}

/// A tick box for every spawner of the map, by name – the ticked ones are in `ids`, in the order of the map's list.
fn spawner_checks(ui: &mut Ui, ids: &mut Vec<u32>, spawners: &[Spawner]) {
    ui.horizontal_wrapped(|ui| {
        for s in spawners {
            let mut on = ids.contains(&s.id);
            if ui.checkbox(&mut on, s.name.as_str()).changed() {
                // Anew from the map's list: in its order, and without spawners that are gone.
                let ticked = |t: &Spawner| if t.id == s.id { on } else { ids.contains(&t.id) };
                *ids = spawners.iter().filter(|t| ticked(t)).map(|t| t.id).collect();
            }
        }
    });
    if ids.is_empty() {
        ui.label(RichText::new("No spawner ticked - nobody comes").color(FINAL));
    }
}

/// A tick box for every region of the map, by name – the ticked ones are in `ids`, in the order of the map's list.
fn zone_checks(ui: &mut Ui, ids: &mut Vec<u32>, zones: &[Zone]) {
    ui.horizontal_wrapped(|ui| {
        for z in zones {
            let mut on = ids.contains(&z.id);
            if ui.checkbox(&mut on, z.name.as_str()).changed() {
                let ticked = |t: &Zone| if t.id == z.id { on } else { ids.contains(&t.id) };
                *ids = zones.iter().filter(|t| ticked(t)).map(|t| t.id).collect();
            }
        }
    });
    if zones.is_empty() {
        ui.label(RichText::new("No regions yet - make some in the Regions module").weak());
    } else if ids.is_empty() {
        ui.label(RichText::new("No region ticked - nobody comes").color(FINAL));
    }
}

/// Where a group of a wave comes from, as a short text.
pub(crate) fn from_label(from: &WaveFrom) -> &'static str {
    match from {
        WaveFrom::OneSpawner => "1 random spawner",
        WaveFrom::Sides(1) => "1 random side",
        WaveFrom::Sides(2) => "2 random sides",
        WaveFrom::Sides(3) => "3 random sides",
        WaveFrom::Sides(_) => "all 4 sides",
        WaveFrom::Spawners(_) => "chosen spawners",
        WaveFrom::Zones(_) => "chosen regions",
    }
}
