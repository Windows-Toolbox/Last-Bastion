//! Running scripts (docs/trigger-und-28yl.md step 4): a script that runs is only data – **which trigger, where in its
//! tree, what it waits for**. No stack of the program, no closure: a `Run` is plain serde, so a round can be saved in the
//! middle of a script, replayed, and joined again by a second player.
//!
//! The tree is the trigger's `actions` (`story.rs`): a step may hold scripts of its own (the steps of a block or a loop,
//! the options of a random action or a choice, the arrival of a train – `Action::child`). A run finds its place by the
//! numbers on the way down (`Level`: the step, and which of its scripts) and the next step in the script it is in.
//!
//! The rules are TAB's (`ZXAction.Execute`): a step whose condition is false is skipped with everything below it and
//! its code; otherwise the action runs, then the code – after all its children. Two things can stop a run: a **wait**
//! (until a tick) and a **choice** (until the player's order). A run doesn't pause the game by itself – windows do
//! that, in the screen (`game.rs`).
//!
//! Everything is deterministic: the runs wake in the order they were started, once a tick (`update_runs`), and a pick
//! comes as an order (`Order::Choose`).

use serde::{Deserialize, Serialize};

use super::story::{Action, Script};
use super::{World, TICKS_PER_SECOND};

/// At most this many steps in one go (until a run waits or ends) – a loop with nothing in it that waits would never
/// end; this one is stopped instead.
const MOST_STEPS: usize = 10_000;

/// One step on the way down the tree: the step of the script you are in, and which of its scripts you go into
/// (`Action::child`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Level {
    pub step: usize,
    pub child: usize,
}

/// A script of the story: the trigger and the way down its tree (empty: its own actions) – what a train remembers to
/// run when it arrives.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Place {
    pub trigger: usize,
    pub path: Vec<Level>,
}

/// What a run waits for.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Waiting {
    #[default]
    No,
    /// Until this tick (`Action::Wait`).
    Until(u64),
    /// For the player's pick (`Order::Choose`) of one of the options `shown` of the choice (`Action::Choose`) – the
    /// ones whose condition held when it was asked – with the question's text and the buttons' texts as they were
    /// worked out then (`{expression}` in them, `World::render`).
    Choice { shown: Vec<usize>, text: String, labels: Vec<String> },
}

/// A script that is running: it began at `base` levels down `path` – it ends when that script does – and is at step
/// `next` of the script `path` leads to.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Run {
    pub trigger: usize,
    pub path: Vec<Level>,
    pub base: usize,
    pub next: usize,
    pub waiting: Waiting,
}

impl Run {
    /// A script about to begin with its first step.
    pub fn new(place: Place) -> Run {
        let base = place.path.len();
        Run { trigger: place.trigger, path: place.path, base, next: 0, waiting: Waiting::No }
    }

    /// Into script `child` of the step it is at.
    fn enter(&mut self, child: usize) {
        self.path.push(Level { step: self.next, child });
        self.next = 0;
    }

    /// The script it is in now, if the story still has it.
    fn script<'a>(&self, story: &'a super::Story) -> Option<&'a Script> {
        script_at(&story.triggers.get(self.trigger)?.actions, &self.path)
    }
}

/// The script `path` leads to from the top.
fn script_at<'a>(top: &'a Script, path: &[Level]) -> Option<&'a Script> {
    let mut script = top;
    for level in path {
        script = script.get(level.step)?.action.child(level.child)?;
    }
    Some(script)
}

/// What the player sees of a choice that waits: its text and the buttons.
pub struct ChoiceView {
    pub text: String,
    pub options: Vec<String>,
}

impl World {
    /// A trigger's script begins now (its conditions are not asked – `fire` does that): it runs until it waits or ends.
    pub fn run_trigger(&mut self, trigger: usize) {
        self.begin(Place { trigger, path: Vec::new() });
    }

    /// The script at `place` begins now.
    pub(super) fn begin(&mut self, place: Place) {
        let mut run = Run::new(place);
        if self.go(&mut run) {
            self.runs.push(run);
        }
    }

    /// Once a tick: the runs whose wait is over go on, in the order they were started. What they start meanwhile comes
    /// after them.
    pub(super) fn update_runs(&mut self) {
        if self.runs.is_empty() {
            return;
        }
        let runs = std::mem::take(&mut self.runs);
        let mut kept = Vec::with_capacity(runs.len());
        for mut run in runs {
            let due = matches!(run.waiting, Waiting::Until(t) if self.tick >= t);
            if !due || self.go(&mut run) {
                kept.push(run);
            }
        }
        kept.append(&mut self.runs);
        self.runs = kept;
    }

    /// Is a choice waiting for the player?
    pub fn choice_waiting(&self) -> bool {
        self.runs.iter().any(|r| matches!(r.waiting, Waiting::Choice { .. }))
    }

    /// The oldest choice that waits for the player – the one `Order::Choose` answers.
    pub fn choice(&self) -> Option<ChoiceView> {
        let run = self.runs.iter().find(|r| matches!(r.waiting, Waiting::Choice { .. }))?;
        let Waiting::Choice { text, labels, .. } = &run.waiting else {
            return None;
        };
        Some(ChoiceView { text: text.clone(), options: labels.clone() })
    }

    /// The player picked button `index` of the oldest waiting choice (`Order::Choose`): its script runs – unless its
    /// condition has turned false since the window opened (TAB asks again): then it is skipped, the choice is done. A
    /// wrong index, or no choice waiting: nothing.
    pub(super) fn choose(&mut self, index: usize) {
        let Some(at) = self.runs.iter().position(|r| matches!(r.waiting, Waiting::Choice { .. })) else {
            return;
        };
        let Waiting::Choice { shown, .. } = &self.runs[at].waiting else {
            return;
        };
        let Some(&child) = shown.get(index) else {
            return;
        };
        let run = &self.runs[at];
        let when = match run.script(&self.story).and_then(|s| s.get(run.next)).map(|s| &s.action) {
            Some(Action::Choose { options, .. }) => options.get(child).and_then(|o| o.when.clone()),
            _ => None,
        };
        let still = when.is_none_or(|w| self.holds(&w));
        let mut run = self.runs.remove(at);
        run.waiting = Waiting::No;
        run.path.push(Level { step: run.next, child });
        // Past the end of the option's script: out of it at once, on with the choice's code and the steps after.
        run.next = if still { 0 } else { usize::MAX };
        if self.go(&mut run) {
            self.runs.insert(at.min(self.runs.len()), run);
        }
    }

    /// After the story was changed in the editor: the runs whose place is gone are dropped.
    pub(super) fn drop_lost_runs(&mut self) {
        let story = &self.story;
        self.runs.retain(|r| r.script(story).is_some());
    }

    /// The run goes on from where it is until it waits (`true`) or ends (`false`, also when its place is no longer in
    /// the story or it did more than `MOST_STEPS`). The tree is copied once: the actions need the whole world.
    fn go(&mut self, run: &mut Run) -> bool {
        let Some(tree) = self.story.triggers.get(run.trigger).map(|t| t.actions.clone()) else {
            return false;
        };
        match run.waiting {
            Waiting::Choice { .. } => return true,
            Waiting::Until(t) if self.tick < t => return true,
            Waiting::Until(_) => {
                // The wait is over: the step is done (its code runs).
                run.waiting = Waiting::No;
                if !self.finish(&tree, run) {
                    return false;
                }
            }
            Waiting::No => {}
        }
        for _ in 0..MOST_STEPS {
            let Some(script) = script_at(&tree, &run.path) else {
                return false;
            };
            // The end of a script: out of the step that holds it – a loop starts again.
            let Some(step) = script.get(run.next) else {
                if run.path.len() <= run.base {
                    return false;
                }
                let level = run.path.pop().expect("longer than base");
                let Some(holder) = script_at(&tree, &run.path).and_then(|s| s.get(level.step)) else {
                    return false;
                };
                if matches!(holder.action, Action::Loop { .. }) && !script.is_empty() {
                    run.path.push(level);
                    run.next = 0;
                } else {
                    self.exec(&holder.code);
                    run.next = level.step + 1;
                }
                continue;
            };
            if step.when.as_ref().is_some_and(|w| !self.holds(w)) {
                run.next += 1;
                continue;
            }
            match &step.action {
                Action::Block { steps } | Action::Loop { steps } if !steps.is_empty() => run.enter(0),
                Action::ExitLoop { loops } => {
                    self.exec(&step.code);
                    self.exit_loops(&tree, run, *loops);
                }
                Action::Wait { seconds } => {
                    let seconds = self.eval(seconds).number();
                    if seconds > 0 {
                        run.waiting = Waiting::Until(self.tick.saturating_add((seconds as u64).saturating_mul(TICKS_PER_SECOND as u64)));
                        return true;
                    }
                    self.exec(&step.code);
                    run.next += 1;
                }
                Action::Random { options } => {
                    // Those whose condition holds, each as likely as its weight.
                    let mut live = Vec::new();
                    for (k, o) in options.iter().enumerate() {
                        if o.when.as_ref().is_none_or(|w| self.holds(w)) {
                            live.push((k, o.weight));
                        }
                    }
                    let total: u32 = live.iter().map(|o| o.1).sum();
                    if total == 0 {
                        self.exec(&step.code);
                        run.next += 1;
                        continue;
                    }
                    let mut roll = self.rng.below(total);
                    let picked = live.iter().find(|o| {
                        let hit = roll < o.1;
                        roll = roll.saturating_sub(o.1);
                        hit
                    });
                    match picked {
                        Some(&(k, _)) => run.enter(k),
                        None => run.next += 1,
                    }
                }
                Action::Choose { text, options } => {
                    let shown: Vec<usize> = (0..options.len()).filter(|&k| options[k].when.as_ref().is_none_or(|w| self.holds(w))).collect();
                    if shown.is_empty() {
                        // No button to show: no window, on with the next step.
                        self.exec(&step.code);
                        run.next += 1;
                        continue;
                    }
                    let labels = shown.iter().map(|&k| self.render(&options[k].shown(k))).collect();
                    run.waiting = Waiting::Choice { shown, text: self.render(text), labels };
                    return true;
                }
                Action::Train { stop, reverse, .. } => {
                    let mut path = run.path.clone();
                    path.push(Level { step: run.next, child: 0 });
                    self.send_train(*stop, *reverse, Place { trigger: run.trigger, path });
                    self.exec(&step.code);
                    run.next += 1;
                }
                action => {
                    self.act(action);
                    self.exec(&step.code);
                    run.next += 1;
                }
            }
        }
        false
    }

    /// The step the run is at is done: its code runs, the next step is next. `false`: the step isn't there.
    fn finish(&mut self, tree: &Script, run: &mut Run) -> bool {
        let Some(step) = script_at(tree, &run.path).and_then(|s| s.get(run.next)) else {
            return false;
        };
        self.exec(&step.code);
        run.next += 1;
        true
    }

    /// `loops` loops around the run end: what is inside is left at once (the steps after the exit, in a block too, are
    /// skipped), the code of every step left runs – inside out – and the run goes on after the last loop. Fewer loops
    /// around: it leaves those; none: nothing happens (the step after it is next).
    fn exit_loops(&mut self, tree: &Script, run: &mut Run, loops: u32) {
        let mut found = 0;
        let mut cut = None;
        for i in (run.base..run.path.len()).rev() {
            let holder = script_at(tree, &run.path[..i]).and_then(|s| s.get(run.path[i].step));
            if holder.is_some_and(|h| matches!(h.action, Action::Loop { .. })) {
                found += 1;
                cut = Some(i);
                if found >= loops.max(1) {
                    break;
                }
            }
        }
        let Some(cut) = cut else {
            run.next += 1;
            return;
        };
        while run.path.len() > cut {
            let level = run.path.pop().expect("longer than cut");
            if let Some(holder) = script_at(tree, &run.path).and_then(|s| s.get(level.step)) {
                self.exec(&holder.code);
            }
            run.next = level.step + 1;
        }
    }
}
