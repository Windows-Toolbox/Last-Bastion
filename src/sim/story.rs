//! The map's story, like TAB's "Game Narrative" (`ZXLevelExtension`, `ZXScript`, `ZXAction_*`) – without TAB's free
//! script language: a **script** is a list of actions from a fixed set, done one after the other in a single tick.
//! Scripts run when the round starts, is won or lost, when an event comes or is done (`events.rs`), when a neutral thing
//! joins the colony (`neutral.rs`), and when the player presses the command center's own button. **Goals** (TAB's
//! victory and defeat conditions) win or lose the round; **counters** (TAB's user variables, numbers only) let
//! scripts count and goals ask.
//!
//! Everything is deterministic like the rest of the simulation (DESIGN.md 12.8): the dice of a random action are the
//! world's, and a choice of the player comes as an order (`Order::Choose`), not straight from the mouse.

use macroquad::math::vec2;
use serde::{Deserialize, Serialize};

use super::events::{Notice, Request};
use super::neutral::{Tag, ThingRef};
use super::spawning::WaveGroup;
use super::terrain::free_point_near;
use super::zombies::Breed;
use super::{World, DAY_TICKS};
use crate::kinds::{BuildingKind, Resource, UnitKind};

/// A script: actions one after the other (TAB's `ZXScript`).
pub type Script = Vec<Action>;

/// One action of a script – TAB's actions (their names in the comments), without free code, sounds, trains and
/// technologies (we have none of those).
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
    /// A counter set to a value, or changed by it (TAB's user variables).
    Counter { name: String, add: bool, value: i32 },
    /// The neutral thing with this name ID joins the colony.
    Join { name_id: String },
    /// The whole map is shown from now on.
    Reveal,
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
    /// A counter is at least this.
    Counter { name: String, at_least: i32 },
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

/// The command center's own button (TAB's "Command Center – Custom Command"): its name, its tooltip, what it does –
/// and how often it can be pressed (0: as often as you like).
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct CustomCommand {
    pub label: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub script: Script,
    #[serde(default)]
    pub uses: u32,
}

/// The map's story: its scripts, goals, the command center's button, the counters' starting values.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Story {
    pub on_start: Script,
    pub on_won: Script,
    pub on_lost: Script,
    pub goals: Vec<Goal>,
    pub command: Option<CustomCommand>,
    /// Counters and their values when the round starts – in this order (no `HashMap`: the order must be the same on
    /// every PC).
    pub counters: Vec<(String, i32)>,
}

impl Story {
    pub fn is_empty(&self) -> bool {
        *self == Story::default()
    }
}

/// A choice the player has to make (`Action::Choose`), waiting for `Order::Choose`.
#[derive(Clone, PartialEq, Debug)]
pub struct Choice {
    pub text: String,
    pub options: Vec<(String, Script)>,
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
    /// The round starts (its first tick): the counters get their starting values, the start script runs.
    pub(super) fn start_story(&mut self) {
        self.counters = self.story.counters.clone();
        let script = self.story.on_start.clone();
        self.run(&script);
    }

    /// Runs a script, action after action, now (in this tick).
    pub fn run(&mut self, script: &Script) {
        let mut done = 0;
        self.run_counted(script, &mut done);
    }

    fn run_counted(&mut self, script: &Script, done: &mut usize) {
        for action in script {
            *done += 1;
            if *done > MOST_ACTIONS {
                return;
            }
            self.act(action, done);
        }
    }

    /// One action.
    fn act(&mut self, action: &Action, done: &mut usize) {
        match action {
            Action::Zombies { groups, dispersion, storm } => self.send_script_zombies(groups, *dispersion, *storm),
            Action::Resources { gold, goods } => {
                if *gold >= 0 {
                    self.earn(*gold as u32);
                } else {
                    self.gold = self.gold.saturating_sub(gold.unsigned_abs());
                }
                for &(r, n) in goods {
                    if n >= 0 {
                        self.stock.add(r, n as u32, self.storage(r));
                    } else {
                        self.stock.take_up_to(r, n.unsigned_abs());
                    }
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
            Action::Choose { text, options } => {
                if !options.is_empty() {
                    self.choices.push(Choice { text: text.clone(), options: options.clone() });
                }
            }
            Action::Counter { name, add, value } => {
                let now = self.counter(name);
                let new = if *add { now.saturating_add(*value) } else { *value };
                match self.counters.iter_mut().find(|c| c.0 == *name) {
                    Some(c) => c.1 = new,
                    None => self.counters.push((name.clone(), new)),
                }
            }
            Action::Join { name_id } => {
                let found = self.neutral.iter().find(|n| n.tag.as_ref().is_some_and(|t| t.name_id == *name_id)).map(|n| n.id);
                if let Some(id) = found {
                    self.set_owner(ThingRef::Neutral(id), super::neutral::Owner::Colony);
                }
            }
            Action::Reveal => self.revealed = true,
            Action::Win => self.ending = self.ending.or(Some(Ending::Won)),
            Action::Lose => self.ending = Some(Ending::Lost),
        }
    }

    /// A counter's value – 0 if no script set it yet.
    pub fn counter(&self, name: &str) -> i32 {
        self.counters.iter().find(|c| c.0 == name).map_or(0, |c| c.1)
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
        }
    }

    /// The command center's own button was pressed (`Order::CustomCommand`): its script runs – as long as it may still
    /// be pressed and a command center stands.
    pub(super) fn custom_command(&mut self) {
        let Some(command) = self.story.command.clone() else {
            return;
        };
        if (command.uses > 0 && self.command_used >= command.uses) || !self.buildings.iter().any(|b| b.kind == BuildingKind::CommandCenter) {
            return;
        }
        self.command_used += 1;
        self.run(&command.script);
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
        // 0 to 100 – a target of 0 is reached at once.
        let share = |now: u64, target: u64| (now.min(target) * 100).checked_div(target).map_or(100, |p| p as u32);
        match &self.story.goals[g].condition {
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
            Condition::Counter { name, at_least } => {
                let now = self.counter(name);
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
