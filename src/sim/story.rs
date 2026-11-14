//! The map's story, like TAB's "Game Narrative" (`ZXLevelExtension`, `ZXScript`, `ZXAction_*`): a **script** is a list
//! of steps, done one after the other in a single tick. A step is an action from a fixed set, a condition (when it's
//! false, the action and its code are skipped) and code that runs after the action – both in the small typed language
//! of `expr.rs`, as in TAB.
//! Scripts hang on **triggers** (SC2's way): a trigger has an event – the round starts, is won or lost, a map event
//! comes or is done (`events.rs`), a neutral thing joins the colony (`neutral.rs`), the player presses the command
//! center's own button –, conditions that must all hold, and the script (`fire`). **Goals** (TAB's victory and defeat
//! conditions) win or lose the round; **variables** (TAB's user variables: a number or yes/no) let scripts count and
//! triggers and goals ask.
//!
//! Everything is deterministic like the rest of the simulation (DESIGN.md 12.8): the dice of a random action are the
//! world's, and a choice of the player comes as an order (`Order::Choose`), not straight from the mouse.

use macroquad::math::vec2;
use serde::{Deserialize, Serialize};

use super::events::{Notice, Request};
use super::expr::{Assign, Code, Expr, Op, Stmt, Value};
use super::neutral::{Tag, ThingRef};
use super::spawning::WaveGroup;
use super::terrain::free_point_near;
use super::zombies::Breed;
use super::{Weather, World, DAY_TICKS};
use crate::kinds::{BuildingKind, Resource, UnitKind};

/// `true` – for values that are on when they're left out.
fn yes() -> bool {
    true
}

/// A script: steps one after the other (TAB's `ZXScript`).
pub type Script = Vec<Step>;

/// One step of a script (TAB's `ZXAction` with its two ZXCode fields): `when` is a condition – false: the action and
/// the code are skipped; the action; then the code (for "Random" after the option's script, for "Choose" after the
/// player's pick and its script).
///
/// In the file a step without condition and code is just the action – as it was before there were any; older files
/// read as they are.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(from = "StepRepr", into = "StepRepr")]
pub struct Step {
    pub action: Action,
    pub when: Option<Expr>,
    pub code: Code,
}

impl Step {
    pub fn new(action: Action) -> Step {
        Step { action, when: None, code: Vec::new() }
    }

    /// A step that only runs this code.
    pub fn code(code: Code) -> Step {
        Step { action: Action::Code, when: None, code }
    }
}

impl From<Action> for Step {
    fn from(action: Action) -> Step {
        Step::new(action)
    }
}

/// How a step is written in the file.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum StepRepr {
    Full {
        action: Action,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<Expr>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        code: Code,
    },
    /// Before format 9 the action "Variable" (first called "Counter") set a number: now it is code.
    OldVariable {
        #[serde(rename = "Variable", alias = "Counter")]
        variable: OldVariable,
    },
    Bare(Action),
}

#[derive(Serialize, Deserialize)]
struct OldVariable {
    name: String,
    add: bool,
    value: i32,
}

impl From<StepRepr> for Step {
    fn from(repr: StepRepr) -> Step {
        match repr {
            StepRepr::Full { action, when, code } => Step { action, when, code },
            StepRepr::OldVariable { variable: v } => {
                let how = if v.add { Assign::Add } else { Assign::Set };
                Step::code(vec![Stmt::Set { name: v.name, how, value: Expr::number(v.value) }])
            }
            StepRepr::Bare(action) => Step::new(action),
        }
    }
}

impl From<Step> for StepRepr {
    fn from(step: Step) -> StepRepr {
        if step.when.is_none() && step.code.is_empty() {
            StepRepr::Bare(step.action)
        } else {
            StepRepr::Full { action: step.action, when: step.when, code: step.code }
        }
    }
}

/// One action of a script – TAB's actions (their names in the comments), without free code, sounds and technologies
/// (we have none of those).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Action {
    /// Zombies out of the spawners, now ("Generate Units"): groups like an event's, every side a few at a time;
    /// storming the command center ("Go to Command Center?") or idling where they come out.
    Zombies { groups: Vec<WaveGroup>, dispersion: u32, storm: bool },
    /// Gold and goods for the colony – or, below 0, taken away ("Give / Remove Resources").
    Resources { gold: i32, goods: Vec<(Resource, i32)> },
    /// Units for the colony next to the command center ("Give Bonus Entities").
    Units { units: Vec<(UnitKind, u32)> },
    /// So many houses with colonists, picked at random, are infected ("Infect Random Dwellings").
    InfectHouses { count: u32 },
    /// A line at the top of the screen ("Display Notification").
    Notify { text: String },
    /// A window with a title and a text – the game pauses until it's closed ("Display Window Message").
    Window { title: String, text: String },
    /// One of these scripts, each as likely as its weight ("Execute Random Action").
    Random { options: Vec<(u32, Script)> },
    /// The player picks one of these in a window – its script runs ("Choose Option").
    Choose { text: String, options: Vec<(String, Script)> },
    /// Nothing but the step's code ("Execute Code" in TAB): `keys += 1; GiveGold(100)`.
    Code,
    /// The neutral thing with this name ID joins the colony.
    Join { name_id: String },
    /// The whole map is shown from now on.
    Reveal,
    /// Snow, rain or clear from now on (TAB's map conditions Snowy / Rainy) – the picture fades over a few seconds.
    Weather { weather: Weather },
    /// A train comes along the railway ("Send Train", `trains.rs`): on the line closest to the command center, from
    /// its end nearer the edge of the map – `reverse`: from the other one. `stop` ("Go to Command Center?"): it stops
    /// next to the command center for a while, and `on_arrival` runs then (TAB unloads colonists there).
    Train {
        #[serde(default = "yes")]
        stop: bool,
        #[serde(default)]
        reverse: bool,
        #[serde(default)]
        on_arrival: Script,
    },
    /// The round is won – or lost – now.
    Win,
    Lose,
}

/// When a goal is reached (TAB's conditions, as a fixed list). Each one also says how far along it is.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Condition {
    /// At least this many colonists live in your houses.
    Colonists(u32),
    /// The clock has reached this day (from 1).
    Day(u32),
    /// At least this many zombies killed.
    ZombiesKilled(u32),
    /// At least this many buildings of this kind stand, finished.
    Built { kind: BuildingKind, count: u32 },
    /// The building or unit with this name ID is gone (destroyed, dead – or never there).
    Destroyed { name_id: String },
    /// A variable is at least this (yes/no ones: 1 is yes; texts: their length). Files before format 8 call it
    /// `Counter`.
    #[serde(alias = "Counter")]
    Variable { name: String, at_least: i32 },
    /// The event with this name has come every time it will.
    EventDone { name: String },
}

/// A goal (TAB's `ZXLevelCondition`): its text for the player, when it's reached, and whether reaching it wins (all
/// victory goals reached) or loses (any defeat goal reached) the round.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Goal {
    pub text: String,
    pub condition: Condition,
    #[serde(default)]
    pub defeat: bool,
}

/// The command center's own button (TAB's "Command Center – Custom Command"): its name, its tooltip, and how often it
/// can be pressed (0: as often as you like). What it does: the triggers with the event `Button`.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct CustomCommand {
    pub label: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub uses: u32,
}

/// When a trigger fires – each event is checked in its own place of the simulation (`fire`).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum TriggerEvent {
    /// The first moment of the round (TAB's "On Game Start").
    Start,
    /// The round is won / lost (TAB's "On Game Won" / "On Game Lost").
    Won,
    Lost,
    /// The map event with this name comes – every time (TAB's event `Script`) …
    EventComes { event: String },
    /// … or has come every time it will, once (`ScriptOnFinished` – a final wave only once none of it is left).
    EventDone { event: String },
    /// The neutral thing with this name ID joins the colony (TAB's "Script On Join").
    Join { name_id: String },
    /// The player presses the command center's own button.
    Button,
}

fn is_yes(b: &bool) -> bool {
    *b
}

/// SC2's trigger: an event, conditions that must all hold, and the actions then (a script). Triggers of one event
/// run in the order of the list. `folder` only groups them in the editor.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Trigger {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub folder: String,
    /// Off: it never fires.
    #[serde(default = "yes", skip_serializing_if = "is_yes")]
    pub on: bool,
    pub event: TriggerEvent,
    /// All of these must be true when the event comes – none: always. Before format 9 they were the goals' kind of
    /// condition (`Condition`), read as the expression it stands for.
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "conditions")]
    pub conditions: Vec<Expr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Script,
}

/// A trigger's conditions from the file: expressions – or, from before format 9, goal conditions.
fn conditions<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Expr>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Either {
        New(Expr),
        Old(Condition),
    }
    let list = Vec::<Either>::deserialize(d)?;
    Ok(list
        .into_iter()
        .map(|c| match c {
            Either::New(e) => e,
            Either::Old(c) => c.to_expr(),
        })
        .collect())
}

impl Trigger {
    pub fn new(name: &str, event: TriggerEvent) -> Trigger {
        Trigger { name: name.to_owned(), folder: String::new(), on: true, event, conditions: Vec::new(), actions: Vec::new() }
    }
}

/// What a variable holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum VarKind {
    #[default]
    Number,
    /// Yes or no.
    Flag,
    Text,
}

/// A variable of the map (TAB's user variables): its name, what it holds, its value when the round starts.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    #[serde(default)]
    pub kind: VarKind,
    /// A number, `true`/`false` or a text – turned into its kind when the round starts (`Value::as_ty`).
    #[serde(default)]
    pub start: Value,
}

/// The map's story: its triggers, goals, the command center's button, the variables.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Story {
    pub triggers: Vec<Trigger>,
    pub goals: Vec<Goal>,
    pub command: Option<CustomCommand>,
    /// The variables and their values when the round starts – in this order (no `HashMap`: the order must be the same
    /// on every PC).
    pub variables: Vec<Variable>,
}

impl Story {
    pub fn is_empty(&self) -> bool {
        *self == Story::default()
    }

    /// Every variable the triggers use gets declared (as a number) if it isn't – files before format 9 let a script
    /// set a counter nobody declared.
    pub fn declare_unknown(&mut self) {
        let mut names = Vec::new();
        for t in &self.triggers {
            t.conditions.iter().for_each(|c| c.variables(&mut names));
            names_in_script(&t.actions, &mut names);
        }
        for name in names {
            if !self.variables.iter().any(|v| v.name == name) {
                self.variables.push(Variable { name, kind: VarKind::Number, start: Value::Number(0) });
            }
        }
    }
}

/// The names of the variables a script reads or sets, children included.
fn names_in_script(script: &Script, names: &mut Vec<String>) {
    for step in script {
        if let Some(w) = &step.when {
            w.variables(names);
        }
        for s in &step.code {
            match s {
                Stmt::Set { name, value, .. } => {
                    names.push(name.clone());
                    value.variables(names);
                }
                Stmt::Do(e) => e.variables(names),
            }
        }
        match &step.action {
            Action::Random { options } => options.iter().for_each(|o| names_in_script(&o.1, names)),
            Action::Choose { options, .. } => options.iter().for_each(|o| names_in_script(&o.1, names)),
            Action::Train { on_arrival, .. } => names_in_script(on_arrival, names),
            _ => {}
        }
    }
}

impl Condition {
    /// The expression that says the same (a goal's condition in a trigger's list).
    pub fn to_expr(&self) -> Expr {
        let at_least = |value: Expr, n: i64| Expr::bin(Op::Ge, value, Expr::number(n.clamp(i32::MIN as i64, i32::MAX as i64) as i32));
        match self {
            Condition::Colonists(n) => at_least(Expr::call("Colonists", Vec::new()), *n as i64),
            Condition::Day(n) => at_least(Expr::call("Day", Vec::new()), *n as i64),
            Condition::ZombiesKilled(n) => at_least(Expr::call("Kills", Vec::new()), *n as i64),
            Condition::Built { kind, count } => at_least(Expr::call("Count", vec![Expr::text(&kind.stats().name)]), *count as i64),
            Condition::Destroyed { name_id } => Expr::call("IsDead", vec![Expr::text(name_id)]),
            Condition::Variable { name, at_least: n } => at_least(Expr::var(name), *n as i64),
            Condition::EventDone { name } => Expr::call("EventDone", vec![Expr::text(name)]),
        }
    }
}

/// A choice the player has to make (`Action::Choose`), waiting for `Order::Choose`.
#[derive(Clone, PartialEq, Debug)]
pub struct Choice {
    pub text: String,
    pub options: Vec<(String, Script)>,
    /// The step's code – it runs after the option the player picked.
    pub then: Code,
}

/// What a script asked to happen to the round – checked with the rest of the end of the round (`round.rs`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Ending {
    Won,
    Lost,
}

/// At most this many actions per script run – a script can't call itself, but a careless one could be very long.
const MOST_ACTIONS: usize = 10_000;

impl World {
    /// The round starts (its first tick): the variables get their starting values, the start triggers fire.
    pub(super) fn start_story(&mut self) {
        self.variables = self.story.variables.iter().map(|v| (v.name.clone(), v.start.as_ty(v.kind.ty()))).collect();
        self.fire(&TriggerEvent::Start);
    }

    /// An event of the round: every trigger for it that is on and whose conditions hold runs its actions, in the
    /// order of the list – one trigger's actions can decide the next one's conditions.
    pub fn fire(&mut self, event: &TriggerEvent) {
        for t in 0..self.story.triggers.len() {
            let trigger = &self.story.triggers[t];
            if !trigger.on || trigger.event != *event {
                continue;
            }
            let (conditions, script) = (trigger.conditions.clone(), trigger.actions.clone());
            if conditions.iter().all(|c| self.holds(c)) {
                self.run(&script);
            }
        }
    }

    /// Runs a script, action after action, now (in this tick).
    pub fn run(&mut self, script: &Script) {
        let mut done = 0;
        self.run_counted(script, &mut done);
    }

    fn run_counted(&mut self, script: &Script, done: &mut usize) {
        for step in script {
            *done += 1;
            if *done > MOST_ACTIONS {
                return;
            }
            if step.when.as_ref().is_some_and(|w| !self.holds(w)) {
                continue;
            }
            match &step.action {
                // The code of a choice runs after the pick: with the choice, not now.
                Action::Choose { text, options } if !options.is_empty() => {
                    self.choices.push(Choice { text: text.clone(), options: options.clone(), then: step.code.clone() });
                }
                action => {
                    self.act(action, done);
                    self.exec(&step.code);
                }
            }
        }
    }

    /// One action.
    fn act(&mut self, action: &Action, done: &mut usize) {
        match action {
            Action::Zombies { groups, dispersion, storm } => self.send_script_zombies(groups, *dispersion, *storm),
            Action::Resources { gold, goods } => {
                self.give_gold(*gold);
                for &(r, n) in goods {
                    self.give_good(r, n);
                }
            }
            Action::Units { units } => {
                for &(kind, count) in units {
                    for k in 0..count {
                        // Around the command center's bottom, one after the other – the free spot nearest each.
                        let want = self.base + vec2((k % 6) as f32 - 2.5, 3.0 + (k / 6) as f32);
                        if let Some(at) = free_point_near(&self.map, want) {
                            self.spawn_unit(kind, at);
                        }
                    }
                }
            }
            Action::InfectHouses { count } => {
                for _ in 0..*count {
                    let houses: Vec<usize> = (0..self.buildings.len())
                        .filter(|&b| self.buildings[b].stats().colonists > 0 && !self.buildings[b].infected && self.buildings[b].is_built())
                        .collect();
                    if houses.is_empty() {
                        break;
                    }
                    let b = houses[self.rng.below(houses.len() as u32) as usize];
                    self.infect(b);
                }
            }
            Action::Notify { text } => self.notices.push(Notice::Message { text: text.clone() }),
            Action::Window { title, text } => self.notices.push(Notice::Window { title: title.clone(), text: text.clone() }),
            Action::Random { options } => {
                let total: u32 = options.iter().map(|o| o.0).sum();
                if total == 0 {
                    return;
                }
                let mut roll = self.rng.below(total);
                for (weight, script) in options {
                    if roll < *weight {
                        let script = script.clone();
                        self.run_counted(&script, done);
                        return;
                    }
                    roll -= weight;
                }
            }
            // A choice is made in `run_counted` (and with no option there is nothing to pick).
            Action::Choose { .. } | Action::Code => {}
            Action::Join { name_id } => {
                let found = self.neutral.iter().find(|n| n.tag.as_ref().is_some_and(|t| t.name_id == *name_id)).map(|n| n.id);
                if let Some(id) = found {
                    self.set_owner(ThingRef::Neutral(id), super::neutral::Owner::Colony);
                }
            }
            Action::Reveal => self.revealed = true,
            Action::Weather { weather } => self.weather = *weather,
            Action::Train { stop, reverse, on_arrival } => self.send_train(*stop, *reverse, on_arrival),
            Action::Win => self.ending = self.ending.or(Some(Ending::Won)),
            Action::Lose => self.ending = Some(Ending::Lost),
        }
    }

    /// An event of the map got another name (the editor): the triggers for it follow – unless another event still has
    /// the old name.
    pub(super) fn rename_event_in_triggers(&mut self, old: &str, new: &str) {
        if old.is_empty() || old == new || self.events.iter().any(|e| e.name == old) {
            return;
        }
        for t in &mut self.story.triggers {
            match &mut t.event {
                TriggerEvent::EventComes { event } | TriggerEvent::EventDone { event } if event == old => *event = new.to_owned(),
                _ => {}
            }
        }
    }

    /// A neutral thing got another name ID (the editor): the triggers for its joining follow.
    pub(super) fn rename_name_id_in_triggers(&mut self, old: &str, new: &str) {
        for t in &mut self.story.triggers {
            if let TriggerEvent::Join { name_id } = &mut t.event
                && name_id == old
            {
                *name_id = new.to_owned();
            }
        }
    }

    /// The player picked option `index` of the oldest waiting choice (`Order::Choose`): its script runs. A wrong
    /// index, or no choice waiting: nothing.
    pub(super) fn choose(&mut self, index: usize) {
        if self.choices.is_empty() {
            return;
        }
        let choice = self.choices.remove(0);
        if let Some((_, script)) = choice.options.get(index) {
            let script = script.clone();
            self.run(&script);
            self.exec(&choice.then);
        }
    }

    /// The command center's own button was pressed (`Order::CustomCommand`): the triggers for it fire – as long as it
    /// may still be pressed and a command center stands.
    pub(super) fn custom_command(&mut self) {
        let Some(command) = self.story.command.clone() else {
            return;
        };
        if (command.uses > 0 && self.command_used >= command.uses) || !self.buildings.iter().any(|b| b.kind == BuildingKind::CommandCenter) {
            return;
        }
        self.command_used += 1;
        self.fire(&TriggerEvent::Button);
    }

    /// Can the command center's own button still be pressed?
    pub fn command_ready(&self) -> bool {
        self.story.command.as_ref().is_some_and(|c| c.uses == 0 || self.command_used < c.uses)
    }

    /// Zombies of a script (`Action::Zombies`): like an event's, every group's sides picked now, per breed a count in
    /// its range – out a few at a time (`events.rs`).
    fn send_script_zombies(&mut self, groups: &[WaveGroup], dispersion: u32, storm: bool) {
        for group in groups {
            let counts: Vec<(Breed, u32)> = group
                .breeds
                .iter()
                .map(|b| {
                    let (lo, hi) = (b.count, b.up_to.max(b.count));
                    (b.breed, lo + self.rng.below(hi - lo + 1))
                })
                .collect();
            if let Some(zones) = self.wave_zones(&group.from) {
                self.request_zones(&zones, &counts, false, false);
                if !storm {
                    for r in self.requests.iter_mut().rev().take(counts.iter().filter(|c| c.1 > 0).count()) {
                        Request::idle(r);
                    }
                }
            }
            for side in self.wave_sides(&group.from) {
                let ids: Vec<u32> = side.into_iter().map(|s| self.spawners[s].id).collect();
                self.request(&ids, &counts, dispersion, false, false);
                if !storm {
                    for r in self.requests.iter_mut().rev().take(counts.iter().filter(|c| c.1 > 0).count()) {
                        Request::idle(r);
                    }
                }
            }
        }
    }

    /// How far along goal `g` is: 0 to 100 (100: reached).
    pub fn goal_progress(&self, g: usize) -> u32 {
        self.condition_progress(&self.story.goals[g].condition)
    }

    /// How far along a condition (a goal's, a trigger's) is: 0 to 100 (100: reached).
    pub fn condition_progress(&self, condition: &Condition) -> u32 {
        // 0 to 100 – a target of 0 is reached at once.
        let share = |now: u64, target: u64| (now.min(target) * 100).checked_div(target).map_or(100, |p| p as u32);
        match condition {
            Condition::Colonists(n) => {
                let colonists: u32 = self.buildings.iter().filter(|b| !b.infected && b.is_built()).map(|b| b.stats().colonists).sum();
                share(colonists as u64, *n as u64)
            }
            Condition::Day(day) => share(self.tick / DAY_TICKS + 1, *day as u64),
            Condition::ZombiesKilled(n) => share(self.score.zombies_killed as u64, *n as u64),
            Condition::Built { kind, count } => {
                let standing = self.buildings.iter().filter(|b| b.kind == *kind && b.is_built() && !b.infected).count();
                share(standing as u64, *count as u64)
            }
            Condition::Destroyed { name_id } => {
                let tagged = |tag: &Option<Box<Tag>>| tag.as_ref().is_some_and(|t| t.name_id == *name_id);
                let there = self.buildings.iter().any(|b| tagged(&b.tag)) || self.units.iter().any(|u| tagged(&u.tag));
                if there { 0 } else { 100 }
            }
            Condition::Variable { name, at_least } => {
                let now = self.variable(name);
                match () {
                    _ if now >= *at_least => 100,
                    _ if *at_least <= 0 => 0,
                    _ => share(now.max(0) as u64, *at_least as u64),
                }
            }
            Condition::EventDone { name } => match self.events.iter().find(|e| e.name == *name) {
                Some(e) if e.is_done() => 100,
                Some(e) => e.repeats().map_or(0, |n| share(e.fired() as u64, n as u64)),
                None => 0,
            },
        }
    }

    /// Is goal `g` reached?
    pub fn goal_reached(&self, g: usize) -> bool {
        self.goal_progress(g) >= 100
    }
}
