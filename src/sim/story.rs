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
//! A script is a tree: **block**, **loop** (with **exit loop**), **random** and **choose** have scripts of their own,
//! **wait** lets time pass. A running script is only data (`script.rs`: `Run` – which trigger, where in its tree, what it
//! waits for), so it can be saved in the middle, replayed and joined again.
//!
//! Everything is deterministic like the rest of the simulation (DESIGN.md 12.8): the dice of a random action are the
//! world's, and a choice of the player comes as an order (`Order::Choose`), not straight from the mouse.

use macroquad::math::vec2;
use serde::{Deserialize, Serialize};

use super::events::{Notice, Request};
use super::expr::{code_text, parse_code, parse_condition, parse_expr, Assign, Code, Expr, Op, Scope, Stmt, Ty, Value};
use super::neutral::ThingRef;
use super::spawning::{WaveFrom, WaveGroup};
use super::watch::Watched;
use super::terrain::free_point_near;
use super::zombies::Breed;
use super::{Weather, World};
use crate::kinds::{BuildingKind, Resource, UnitKind};

/// `true` – for values that are on when they're left out.
fn yes() -> bool {
    true
}

fn one() -> u32 {
    1
}

/// Is it what a value left out of the file would be? (Such fields aren't written.)
fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// The loudest noise an action makes (TAB: 10,000).
pub const MOST_NOISE: u32 = 10_000;

/// The colour of a notification's text (TAB's `ZXAction_ShowNotification.TColor`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Tint {
    #[default]
    White,
    Green,
    Orange,
    Gold,
    Red,
    Blue,
}

impl Tint {
    pub const ALL: [Tint; 6] = [Tint::White, Tint::Green, Tint::Orange, Tint::Gold, Tint::Red, Tint::Blue];

    pub fn name(self) -> &'static str {
        match self {
            Tint::White => "White",
            Tint::Green => "Green",
            Tint::Orange => "Orange",
            Tint::Gold => "Gold",
            Tint::Red => "Red",
            Tint::Blue => "Blue",
        }
    }

    /// TAB's colours (`YellowGreen`, `Orange`, `Gold`, `Red`, `Aqua`).
    pub fn rgb(self) -> [u8; 3] {
        match self {
            Tint::White => [255, 255, 255],
            Tint::Green => [154, 205, 50],
            Tint::Orange => [255, 165, 0],
            Tint::Gold => [255, 215, 0],
            Tint::Red => [255, 0, 0],
            Tint::Blue => [0, 255, 255],
        }
    }
}

/// The picture of a notification (TAB's `TIcon`): the file `assets\ui\events\<file>.png`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Icon {
    #[default]
    Info,
    Warning,
    Reward,
    Swarm,
    Fire,
    GoalCompleted,
}

impl Icon {
    pub const ALL: [Icon; 6] = [Icon::Info, Icon::Warning, Icon::Reward, Icon::Swarm, Icon::Fire, Icon::GoalCompleted];

    pub fn name(self) -> &'static str {
        match self {
            Icon::Info => "Info",
            Icon::Warning => "Warning",
            Icon::Reward => "Reward",
            Icon::Swarm => "Swarm",
            Icon::Fire => "Fire",
            Icon::GoalCompleted => "Goal completed",
        }
    }

    /// The file's name.
    pub fn file(self) -> &'static str {
        match self {
            Icon::Info => "EventInfo",
            Icon::Warning => "EventWarning",
            Icon::Reward => "EventReward",
            Icon::Swarm => "EventSwarm",
            Icon::Fire => "EventFire",
            Icon::GoalCompleted => "EventGoalCompleted",
        }
    }
}

/// The sound of a window or a "Sound" action: one of TAB's custom-event pieces (`Music\CustomEvent_<name>N.ogg`,
/// `ZXCustomEventSoundType`) – or none.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Mood {
    #[default]
    Informative,
    Apocalyptic,
    Celebration,
    Crazy,
    Fun,
    Mystery,
    Negative,
    Positive,
    Sad,
    Scary,
    Tension,
    Silent,
}

impl Mood {
    pub const ALL: [Mood; 12] = [
        Mood::Informative,
        Mood::Apocalyptic,
        Mood::Celebration,
        Mood::Crazy,
        Mood::Fun,
        Mood::Mystery,
        Mood::Negative,
        Mood::Positive,
        Mood::Sad,
        Mood::Scary,
        Mood::Tension,
        Mood::Silent,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Mood::Informative => "Informative",
            Mood::Apocalyptic => "Apocalyptic",
            Mood::Celebration => "Celebration",
            Mood::Crazy => "Crazy",
            Mood::Fun => "Fun",
            Mood::Mystery => "Mystery",
            Mood::Negative => "Negative",
            Mood::Positive => "Positive",
            Mood::Sad => "Sad",
            Mood::Scary => "Scary",
            Mood::Tension => "Tension",
            Mood::Silent => "No sound",
        }
    }

    /// The list of sound files this mood plays one of (`audio.rs`) – none: it is silent.
    pub fn sounds(self) -> Option<String> {
        (self != Mood::Silent).then(|| format!("CustomEvent_{}", self.name()))
    }
}

/// A script: steps one after the other (TAB's `ZXScript`).
pub type Script = Vec<Step>;

/// One step of a script (TAB's `ZXAction` with its two ZXCode fields): `when` is a condition – false: the action, its
/// children and the code are skipped; the action; then the code – **after all the children** (a block's steps, a
/// loop's last round, the option of a random action or the player's pick and its script, the wait).
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
    /// A line at the top of the screen ("Display Notification"), with a colour and a picture. `{expression}` in the
    /// text is worked out when it is shown (`World::render`).
    Notify {
        text: String,
        #[serde(default, skip_serializing_if = "is_default")]
        color: Tint,
        #[serde(default, skip_serializing_if = "is_default")]
        icon: Icon,
    },
    /// A window with a title and a text – the game pauses until it's closed ("Display Window Message"). `mood` is the
    /// sound that plays when it opens, `picture` the name of a PNG next to the map file that is shown above the text.
    Window {
        title: String,
        text: String,
        #[serde(default, skip_serializing_if = "is_default")]
        mood: Mood,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        picture: String,
    },
    /// A sound of TAB's custom events plays ("Play Sound").
    Sound { mood: Mood },
    /// Steps one after the other, as one ("Block") – with its own condition and code.
    Block { steps: Script },
    /// The steps again and again until an exit loop leaves it ("Loop"). Without one it never ends: put a wait or a
    /// choice in it.
    Loop { steps: Script },
    /// Leaves the innermost loop – or, with `loops` above 1, that many ("Exit Loop"). Outside any loop it does nothing.
    ExitLoop {
        #[serde(default = "one")]
        loops: u32,
    },
    /// Time passes before the next step: this many seconds (`Wait(5)` in a loop makes a timer).
    Wait { seconds: Expr },
    /// One of these scripts, each as likely as its weight – only those whose condition holds ("Execute Random Action").
    Random { options: Vec<Weighted> },
    /// The player picks one of these in a window – only those whose condition holds; none: no window, on with the next
    /// step. The pick's script runs ("Choose Option").
    Choose { text: String, options: Vec<Pick> },
    /// Nothing but the step's code ("Execute Code" in TAB): `keys += 1; GiveGold(100)`.
    Code,
    /// The neutral thing with this name ID joins the colony.
    Join { name_id: String },
    /// The colony gets these researches (TAB's `Tech…` names, `data\esearch.json`) without a workshop – at once
    /// ("Grant Technologies"). One it has is skipped; one a workshop is working on is done when that one is.
    Research { ids: Vec<String> },
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
    /// The camera goes there ("Move Camera"): the screen takes it from the notices.
    Camera { to: Site },
    /// A noise there that the zombies hear, 0 to 10,000 (TAB's noise of "On Enter").
    Noise { at: Site, volume: u32 },
    /// The playing field becomes the smallest straight rectangle on screen that covers this region ("Playing field to
    /// region", SC2's missions in parts): units outside it are moved in, the border moves, nothing else changes.
    PlayArea { to: u32 },
    /// The round is won – or lost – now.
    Win,
    Lose,
}

impl Action {
    /// A line of text, white, with the info picture.
    #[cfg(test)]
    pub fn notify(text: &str) -> Action {
        Action::Notify { text: text.to_owned(), color: Tint::default(), icon: Icon::default() }
    }

    /// A window with this title and text, informative, without a picture.
    #[cfg(test)]
    pub fn window(title: &str, text: &str) -> Action {
        Action::Window { title: title.to_owned(), text: text.to_owned(), mood: Mood::default(), picture: String::new() }
    }

    /// The script of child `k`: the steps of a block or loop (0), an option of a random action or a choice, the arrival
    /// of a train (0). A running script finds its place by these numbers (`script.rs`).
    pub fn child(&self, k: usize) -> Option<&Script> {
        match self {
            Action::Block { steps } | Action::Loop { steps } => (k == 0).then_some(steps),
            Action::Random { options } => options.get(k).map(|o| &o.steps),
            Action::Choose { options, .. } => options.get(k).map(|o| &o.steps),
            Action::Train { on_arrival, .. } => (k == 0).then_some(on_arrival),
            _ => None,
        }
    }

    /// The regions and points this action itself names (not those of the steps inside it): `(regions, points)`.
    pub(super) fn sites(&self) -> (Vec<u32>, Vec<u32>) {
        let (mut zones, mut spots) = (Vec::new(), Vec::new());
        let mut site = |s: &Site| match *s {
            Site::Region(id) => zones.push(id),
            Site::Point(id) => spots.push(id),
        };
        match self {
            Action::Camera { to } => site(to),
            Action::Noise { at, .. } => site(at),
            Action::PlayArea { to } => zones.push(*to),
            Action::Zombies { groups, .. } => {
                for group in groups {
                    if let WaveFrom::Zones(ids) = &group.from {
                        zones.extend(ids);
                    }
                }
            }
            _ => {}
        }
        (zones, spots)
    }

    /// All the scripts inside it, mutable.
    fn children_mut(&mut self) -> Vec<&mut Script> {
        match self {
            Action::Block { steps } | Action::Loop { steps } => vec![steps],
            Action::Random { options } => options.iter_mut().map(|o| &mut o.steps).collect(),
            Action::Choose { options, .. } => options.iter_mut().map(|o| &mut o.steps).collect(),
            Action::Train { on_arrival, .. } => vec![on_arrival],
            _ => Vec::new(),
        }
    }
}

/// An option of a random action: its weight, a condition (false: it's out of the draw) and its steps (TAB: a child
/// action with its weight). In the file without a condition just `[weight, steps]`, as before there was one.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(from = "WeightedRepr", into = "WeightedRepr")]
pub struct Weighted {
    pub weight: u32,
    pub when: Option<Expr>,
    pub steps: Script,
}

impl Weighted {
    pub fn new(weight: u32, steps: Script) -> Weighted {
        Weighted { weight, when: None, steps }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum WeightedRepr {
    Full {
        weight: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<Expr>,
        steps: Script,
    },
    Plain((u32, Script)),
}

impl From<WeightedRepr> for Weighted {
    fn from(repr: WeightedRepr) -> Weighted {
        match repr {
            WeightedRepr::Full { weight, when, steps } => Weighted { weight, when, steps },
            WeightedRepr::Plain((weight, steps)) => Weighted { weight, when: None, steps },
        }
    }
}

impl From<Weighted> for WeightedRepr {
    fn from(o: Weighted) -> WeightedRepr {
        match o.when {
            None => WeightedRepr::Plain((o.weight, o.steps)),
            when => WeightedRepr::Full { weight: o.weight, when, steps: o.steps },
        }
    }
}

/// An option of a choice: the text of its button, a condition (false: the button isn't shown) and its steps. In the
/// file without a condition just `[label, steps]`, as before there was one.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(from = "PickRepr", into = "PickRepr")]
pub struct Pick {
    pub label: String,
    pub when: Option<Expr>,
    pub steps: Script,
}

impl Pick {
    pub fn new(label: &str, steps: Script) -> Pick {
        Pick { label: label.to_owned(), when: None, steps }
    }

    /// The text of its button: empty, it is TAB's standard text – "Back" for an option that leaves a loop.
    pub fn shown(&self, k: usize) -> String {
        match (self.label.trim(), self.steps.first().map(|s| &s.action)) {
            ("", Some(Action::ExitLoop { .. })) => "Back".to_owned(),
            ("", _) => format!("Option {}", k + 1),
            (label, _) => label.to_owned(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum PickRepr {
    Full {
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        when: Option<Expr>,
        steps: Script,
    },
    Plain((String, Script)),
}

impl From<PickRepr> for Pick {
    fn from(repr: PickRepr) -> Pick {
        match repr {
            PickRepr::Full { label, when, steps } => Pick { label, when, steps },
            PickRepr::Plain((label, steps)) => Pick { label, when: None, steps },
        }
    }
}

impl From<Pick> for PickRepr {
    fn from(o: Pick) -> PickRepr {
        match o.when {
            None => PickRepr::Plain((o.label, o.steps)),
            when => PickRepr::Full { label: o.label, when, steps: o.steps },
        }
    }
}

/// The goals' kind of condition up to format 8 (a fixed list), kept to read older files: a goal reads one as its
/// expression (`to_expr`, `progress_expr`).
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

/// A goal (TAB's `ZXLevelCondition`): its text for the player, when it's reached (a yes/no expression), how far along
/// it is (a number from 0 to 100 – left out: 0, and 100 once reached), and whether reaching it wins (all victory goals
/// reached) or loses (any defeat goal reached) the round.
///
/// In a file before format 10 the condition was one of a fixed list (`Condition`): it is read as the expressions that
/// say the same.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(from = "GoalRepr", into = "GoalRepr")]
pub struct Goal {
    pub text: String,
    pub when: Expr,
    pub progress: Option<Expr>,
    pub defeat: bool,
}

#[derive(Serialize, Deserialize)]
struct GoalRepr {
    text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    when: Option<Expr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    progress: Option<Expr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    condition: Option<Condition>,
    #[serde(default)]
    defeat: bool,
}

impl From<GoalRepr> for Goal {
    fn from(r: GoalRepr) -> Goal {
        let (when, progress) = match (r.when, r.condition) {
            (Some(when), _) => (when, r.progress),
            (None, Some(old)) => (old.to_expr(), r.progress.or_else(|| old.progress_expr())),
            (None, None) => (Expr::yes(), r.progress),
        };
        Goal { text: r.text, when, progress, defeat: r.defeat }
    }
}

impl From<Goal> for GoalRepr {
    fn from(g: Goal) -> GoalRepr {
        GoalRepr { text: g.text, when: Some(g.when), progress: g.progress, condition: None, defeat: g.defeat }
    }
}

/// The command center's own button (TAB's "Command Center – Custom Command"): its name, its tooltip, and how often it
/// can be pressed (0: as often as you like). What it does: the triggers with the event `Button`. Three expressions
/// are asked every tick like a goal's: is it visible, is it enabled (not: grey, with `disabled_text` in the tooltip),
/// and a number shown on it (a counter) – each left out: always yes, no number. (TAB's resource cost is missing:
/// `GiveGold(-100)` in the trigger and `Gold >= 100` as the condition do it.)
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct CustomCommand {
    pub label: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub uses: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<Expr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<Expr>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub disabled_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter: Option<Expr>,
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
    /// Every this many seconds of the round (SC2's periodic timer) – the first time after that many.
    Every { seconds: u32 },
    /// The clock reaches this day (from 1) and hour (0–23), once.
    Clock { day: u32, hour: u32 },
    /// A building of this kind (none: any kind) is finished …
    Built { kind: Option<BuildingKind> },
    /// … or falls (destroyed – not torn down, not a barrel that went off).
    Fell { kind: Option<BuildingKind> },
    /// The zombies killed so far reach this many, once.
    Kills { count: u32 },
    /// One of your units comes into / goes out of the region with this number (`watch.rs`).
    Enters { zone: u32 },
    Leaves { zone: u32 },
    /// One of your units comes within a tile of the neutral thing with this name ID, once (TAB's "On Enter",
    /// `CScriptOnEnter`) – a joinable one joins at the same time (`Join` first).
    Reached { name_id: String },
    /// One of your units picks up the pile of loot with this name ID (the editor's Name ID on a pile of loot).
    PickedUp { name_id: String },
}

impl TriggerEvent {
    /// Does a trigger for this event answer to what just `happened`? The same event – or, for "a building is
    /// finished / falls", the one without a kind answers to every kind.
    pub fn answers(&self, happened: &TriggerEvent) -> bool {
        match (self, happened) {
            (TriggerEvent::Built { kind: None }, TriggerEvent::Built { .. }) | (TriggerEvent::Fell { kind: None }, TriggerEvent::Fell { .. }) => true,
            _ => self == happened,
        }
    }
}

/// A place a script names: a region (its middle) or a point, by number – they keep it when they are renamed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Site {
    Region(u32),
    Point(u32),
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
    /// A note for whoever edits the map – the round ignores it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,
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
        Trigger { name: name.to_owned(), folder: String::new(), on: true, comment: String::new(), event, conditions: Vec::new(), actions: Vec::new() }
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
        for g in &self.goals {
            g.when.variables(&mut names);
            g.progress.iter().for_each(|p| p.variables(&mut names));
        }
        for e in self.command.iter().flat_map(|c| [&c.visible, &c.enabled, &c.counter]).flatten() {
            e.variables(&mut names);
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
            Action::Block { steps } | Action::Loop { steps } => names_in_script(steps, names),
            Action::Wait { seconds } => seconds.variables(names),
            Action::Random { options } => {
                for o in options {
                    o.when.iter().for_each(|w| w.variables(names));
                    names_in_script(&o.steps, names);
                }
            }
            Action::Choose { options, .. } => {
                for o in options {
                    o.when.iter().for_each(|w| w.variables(names));
                    names_in_script(&o.steps, names);
                }
            }
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

    /// How far along it was, as an expression (`value * 100 / target`; shown as 0 to 100): what the old condition
    /// showed on the goals list. None for the ones that were only reached or not.
    pub fn progress_expr(&self) -> Option<Expr> {
        let share = |value: Expr, n: i64| (n > 0).then(|| Expr::bin(Op::Div, Expr::bin(Op::Mul, value, Expr::number(100)), Expr::number(n.min(i32::MAX as i64) as i32)));
        match self {
            Condition::Colonists(n) => share(Expr::call("Colonists", Vec::new()), *n as i64),
            Condition::Day(n) => share(Expr::call("Day", Vec::new()), *n as i64),
            Condition::ZombiesKilled(n) => share(Expr::call("Kills", Vec::new()), *n as i64),
            Condition::Built { kind, count } => share(Expr::call("Count", vec![Expr::text(&kind.stats().name)]), *count as i64),
            Condition::Variable { name, at_least } => share(Expr::var(name), *at_least as i64),
            Condition::Destroyed { .. } | Condition::EventDone { .. } => None,
        }
    }
}

/// How far a goal is: reached, and a number from 0 to 99 – 100 once it is reached. Worked out every tick
/// (`refresh_story_view`) so the screen only reads it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct GoalState {
    pub reached: bool,
    pub progress: u32,
}

/// What the command center's button looks like right now (`CustomCommand`'s expressions, worked out every tick).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CommandState {
    pub visible: bool,
    pub enabled: bool,
    pub counter: Option<i32>,
}

impl Default for CommandState {
    fn default() -> CommandState {
        CommandState { visible: true, enabled: true, counter: None }
    }
}

/// What a script asked to happen to the round – checked with the rest of the end of the round (`round.rs`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Ending {
    Won,
    Lost,
}

/// One part of the story that holds an expression, code or an action: what `Story::walk` hands out.
pub enum Part<'a> {
    /// A yes/no expression: a condition.
    Flag(&'a mut Expr),
    /// A number: how long to wait, a goal's progress, a button's counter.
    Number(&'a mut Expr),
    Code(&'a mut Code),
    Action(&'a mut Action),
}

impl Part<'_> {
    /// Each expression in it (the ones in code too).
    pub fn exprs(&mut self, f: &mut dyn FnMut(&mut Expr)) {
        match self {
            Part::Flag(e) | Part::Number(e) => f(e),
            Part::Code(code) => {
                for s in code.iter_mut() {
                    match s {
                        Stmt::Set { value, .. } => f(value),
                        Stmt::Do(e) => f(e),
                    }
                }
            }
            Part::Action(_) => {}
        }
    }
}

fn walk_script(script: &mut Script, f: &mut dyn FnMut(Part)) {
    for step in script {
        if let Some(w) = &mut step.when {
            f(Part::Flag(w));
        }
        f(Part::Action(&mut step.action));
        f(Part::Code(&mut step.code));
        match &mut step.action {
            Action::Wait { seconds } => f(Part::Number(seconds)),
            Action::Random { options } => options.iter_mut().filter_map(|o| o.when.as_mut()).for_each(|w| f(Part::Flag(w))),
            Action::Choose { options, .. } => options.iter_mut().filter_map(|o| o.when.as_mut()).for_each(|w| f(Part::Flag(w))),
            _ => {}
        }
        for child in step.action.children_mut() {
            walk_script(child, f);
        }
    }
}

impl Trigger {
    pub fn walk(&mut self, f: &mut dyn FnMut(Part)) {
        self.conditions.iter_mut().for_each(|c| f(Part::Flag(c)));
        walk_script(&mut self.actions, f);
    }
}

impl Goal {
    pub fn walk(&mut self, f: &mut dyn FnMut(Part)) {
        f(Part::Flag(&mut self.when));
        if let Some(p) = &mut self.progress {
            f(Part::Number(p));
        }
    }
}

impl CustomCommand {
    pub fn walk(&mut self, f: &mut dyn FnMut(Part)) {
        self.visible.iter_mut().chain(self.enabled.iter_mut()).for_each(|e| f(Part::Flag(e)));
        self.counter.iter_mut().for_each(|e| f(Part::Number(e)));
    }
}

impl Story {
    /// Every expression, code and action of the story, in the order of the file (to rename things in them, to check
    /// them).
    pub fn walk(&mut self, f: &mut dyn FnMut(Part)) {
        self.triggers.iter_mut().for_each(|t| t.walk(f));
        self.goals.iter_mut().for_each(|g| g.walk(f));
        if let Some(c) = &mut self.command {
            c.walk(f);
        }
    }

    /// Where an expression or code no longer fits the variables – a variable that changed its kind (number to text) or
    /// went: it would be turned into the other kind when the round runs (`Value::as_ty`), probably not as meant. The
    /// places, for the editor to warn about.
    pub fn misfits(&self) -> Vec<String> {
        let scope = Scope { variables: &self.variables };
        let bad = |part: Part| match part {
            Part::Flag(e) => parse_condition(&e.to_string(), scope).is_err(),
            Part::Number(e) => parse_expr(&e.to_string(), scope, Ty::Number).is_err(),
            Part::Code(c) => parse_code(&code_text(c), scope).is_err(),
            Part::Action(_) => false,
        };
        let mut places = Vec::new();
        for t in &self.triggers {
            let (mut copy, mut found) = (t.clone(), false);
            copy.walk(&mut |p| found |= bad(p));
            if found {
                places.push(format!("trigger \"{}\"", t.name));
            }
        }
        for g in &self.goals {
            let (mut copy, mut found) = (g.clone(), false);
            copy.walk(&mut |p| found |= bad(p));
            if found {
                places.push(format!("goal \"{}\"", g.text));
            }
        }
        if let Some(c) = &self.command {
            let (mut copy, mut found) = (c.clone(), false);
            copy.walk(&mut |p| found |= bad(p));
            if found {
                places.push("the command center's button".to_owned());
            }
        }
        places
    }
}

impl Expr {
    /// In the calls of these functions (any case) whose first value is the text `old`: `new` instead – a thing was
    /// renamed in the editor.
    fn rename_text(&mut self, funcs: &[&str], old: &str, new: &str) {
        match self {
            Expr::Call { name, args } => {
                if funcs.iter().any(|f| f.eq_ignore_ascii_case(name)) && matches!(args.first(), Some(Expr::Lit(Value::Text(t))) if t == old) {
                    args[0] = Expr::text(new);
                }
                args.iter_mut().for_each(|a| a.rename_text(funcs, old, new));
            }
            Expr::Not(e) | Expr::Neg(e) => e.rename_text(funcs, old, new),
            Expr::Bin(_, l, r) => {
                l.rename_text(funcs, old, new);
                r.rename_text(funcs, old, new);
            }
            Expr::Lit(_) | Expr::Var(_) => {}
        }
    }
}

impl World {
    /// The round starts (its first tick): the variables get their starting values, the start triggers fire.
    pub(super) fn start_story(&mut self) {
        self.variables = self.story.variables.iter().map(|v| (v.name.clone(), v.start.as_ty(v.kind.ty()))).collect();
        self.runs.clear();
        self.dead.clear();
        self.watched = Watched::default();
        self.fire(&TriggerEvent::Start);
        self.refresh_story_view();
    }

    /// An event of the round: every trigger for it that is on and whose conditions hold runs its actions, in the
    /// order of the list – one trigger's actions can decide the next one's conditions.
    pub fn fire(&mut self, event: &TriggerEvent) {
        for t in 0..self.story.triggers.len() {
            let trigger = &self.story.triggers[t];
            if !trigger.on || !trigger.event.answers(event) {
                continue;
            }
            let conditions = trigger.conditions.clone();
            if conditions.iter().all(|c| self.holds(c)) {
                self.run_trigger(t);
            }
        }
    }

    /// One action that happens at once (`script.rs` takes care of the ones with steps of their own, of time and of
    /// the player's choice).
    pub(super) fn act(&mut self, action: &Action) {
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
                    self.give_units(kind, count);
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
            Action::Notify { text, color, icon } => {
                let text = self.render(text);
                self.notices.push(Notice::Message { text, color: *color, icon: *icon });
            }
            Action::Window { title, text, mood, picture } => {
                let (title, text) = (self.render(title), self.render(text));
                self.notices.push(Notice::Window { title, text, mood: *mood, picture: picture.clone() });
            }
            Action::Sound { mood } => self.notices.push(Notice::Sound { mood: *mood }),
            Action::Join { name_id } => {
                let found = self.neutral.iter().find(|n| n.tag.as_ref().is_some_and(|t| t.name_id == *name_id)).map(|n| n.id);
                if let Some(id) = found {
                    self.set_owner(ThingRef::Neutral(id), super::neutral::Owner::Colony);
                }
            }
            Action::Research { ids } => {
                for id in ids {
                    if let Some(t) = crate::data::get().research.index(id)
                        && !self.researched(t)
                    {
                        self.finish_research(t);
                    }
                }
            }
            Action::Reveal => self.revealed = true,
            Action::Weather { weather } => self.weather = *weather,
            Action::Camera { to } => {
                if let Some(at) = self.site_center(*to) {
                    self.notices.push(Notice::Camera { at });
                }
            }
            Action::Noise { at, volume } => {
                if let Some(at) = self.site_center(*at) {
                    self.make_sound(at, (*volume).min(MOST_NOISE));
                }
            }
            Action::PlayArea { to } => {
                if let Some(bounds) = self.find_zone(*to).and_then(|i| self.zones.get(i)).and_then(|z| z.bounds()) {
                    // A tile (x, y) is at a = x − y, b = x + y; the corners of the tile rectangle reach furthest.
                    let (x0, y0, x1, y1) = (bounds.min.0, bounds.min.1, bounds.max.0 + 1, bounds.max.1 + 1);
                    self.map.set_play_area((x0 - y1, x0 + y0), (x1 - y0 - (x0 - y1), x1 + y1 - (x0 + y0)));
                    self.map_changed = true;
                }
            }
            Action::Win => self.ending = self.ending.or(Some(Ending::Won)),
            Action::Lose => self.ending = Some(Ending::Lost),
            // These are steps of their own (`script.rs`); the code of a step runs there too.
            Action::Code
            | Action::Block { .. }
            | Action::Loop { .. }
            | Action::ExitLoop { .. }
            | Action::Wait { .. }
            | Action::Random { .. }
            | Action::Choose { .. }
            | Action::Train { .. } => {}
        }
    }

    /// `count` units for the colony, around the command center's bottom, one after the other – the free spot nearest
    /// each.
    pub(super) fn give_units(&mut self, kind: UnitKind, count: u32) {
        for k in 0..count {
            let want = self.base + vec2((k % 6) as f32 - 2.5, 3.0 + (k / 6) as f32);
            if let Some(at) = free_point_near(&self.map, want) {
                self.spawn_unit(kind, at);
            }
        }
    }

    /// An event of the map got another name (the editor): the triggers for it follow, and so do the texts in the
    /// expressions that ask for it (`EventDone("old")`) – unless another event still has the old name.
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
        self.story.walk(&mut |mut part| part.exprs(&mut |e| e.rename_text(&["EventDone"], old, new)));
    }

    /// A neutral thing got another name ID (the editor): the triggers for its joining follow, and the action "Join"
    /// and the expressions that ask for it (`IsDead("old")`).
    pub(super) fn rename_name_id_in_triggers(&mut self, old: &str, new: &str) {
        for t in &mut self.story.triggers {
            if let TriggerEvent::Join { name_id } = &mut t.event
                && name_id == old
            {
                *name_id = new.to_owned();
            }
        }
        self.story.walk(&mut |mut part| {
            part.exprs(&mut |e| e.rename_text(&["IsDead"], old, new));
            if let Part::Action(Action::Join { name_id }) = &mut part
                && name_id == old
            {
                *name_id = new.to_owned();
            }
        });
    }

    /// The command center's own button was pressed (`Order::CustomCommand`): the triggers for it fire – as long as it
    /// may still be pressed and a command center stands.
    pub(super) fn custom_command(&mut self) {
        if !self.command_ready() || !self.buildings.iter().any(|b| b.kind == BuildingKind::CommandCenter) {
            return;
        }
        self.command_used += 1;
        self.fire(&TriggerEvent::Button);
    }

    /// Can the command center's own button be pressed? It must be there, visible and enabled (as of the last tick) and
    /// not used up.
    pub fn command_ready(&self) -> bool {
        let state = &self.command_state;
        self.story.command.as_ref().is_some_and(|c| (c.uses == 0 || self.command_used < c.uses) && state.visible && state.enabled)
    }

    /// The goals' and the button's expressions, asked once a tick (the screen only reads the answers: `goal_progress`,
    /// `command_state`). Before the end of the tick's check of the round (`check_outcome`).
    pub(super) fn refresh_story_view(&mut self) {
        let mut goals = Vec::with_capacity(self.story.goals.len());
        for g in 0..self.story.goals.len() {
            let (when, progress) = (self.story.goals[g].when.clone(), self.story.goals[g].progress.clone());
            let reached = self.holds(&when);
            let progress = if reached { 100 } else { progress.map_or(0, |p| self.eval(&p).number().clamp(0, 99) as u32) };
            goals.push(GoalState { reached, progress });
        }
        self.goal_states = goals;
        self.command_state = match self.story.command.clone() {
            None => CommandState::default(),
            Some(c) => CommandState {
                visible: c.visible.as_ref().is_none_or(|e| self.holds(e)),
                enabled: c.enabled.as_ref().is_none_or(|e| self.holds(e)),
                counter: c.counter.as_ref().map(|e| self.eval(e).number()),
            },
        };
    }

    /// How far along goal `g` is: 0 to 100 (100: reached).
    pub fn goal_progress(&self, g: usize) -> u32 {
        self.goal_states.get(g).map_or(0, |s| s.progress)
    }

    /// Is goal `g` reached?
    pub fn goal_reached(&self, g: usize) -> bool {
        self.goal_states.get(g).is_some_and(|s| s.reached)
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
}
