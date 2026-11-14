//! The round as a whole (DESIGN.md 4): how it ends – lost when the last command center falls or is infected (like
//! TAB's: its shield broken, half its life), won like TAB when its
//! victory event comes, or when the final wave is dead – or as the story says: its goals, a script that wins or loses
//! (`check_outcome`, `story.rs`) – and what happened in it along the way (`Score`, for the end screen). Its events are
//! in `events.rs`.

use super::story::{Ending, TriggerEvent};
use super::zombies::Zombie;
use super::World;
use crate::kinds::BuildingKind;

/// How a round ended, and at which tick.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Outcome {
    /// The victory event came, or the final wave is dead – while a command center still stands.
    Won { tick: u64 },
    /// The last command center fell or was infected.
    Lost { tick: u64 },
}

impl Outcome {
    pub fn tick(self) -> u64 {
        match self {
            Outcome::Won { tick } | Outcome::Lost { tick } => tick,
        }
    }
}

/// What happened in the round – counted along the way, shown on the end screen. Only counts, it changes nothing,
/// so it's not in the checksum either.
#[derive(Clone, Copy, Default, Debug)]
pub struct Score {
    pub zombies_killed: u32,
    pub units_trained: u32,
    pub units_lost: u32,
    /// Units that left because of debt (`debt.rs`) – and the ones you sent away (`Order::Dismiss`).
    pub units_deserted: u32,
    pub units_dismissed: u32,
    pub buildings_built: u32,
    pub buildings_lost: u32,
    pub gold_earned: u32,
    pub gold_spent: u32,
    /// Houses of the villages of doom destroyed, piles of loot picked up (`doom.rs`).
    pub doom_razed: u32,
    pub loot_found: u32,
}

impl World {
    /// The end of the round, checked at the end of every tick. Lost: the last command center fell this tick
    /// (`command_center_fell` – a map without one from the start can't be lost that way) or none is left that isn't
    /// infected (TAB's `EndGameIfInfected`). Won: an event that wins
    /// (`Event::won`, TAB's victory at the end of the last day) came – or every event that wins once its swarm is dead
    /// (`won_when_dead`) has come every time and none of the final wave is alive or still to come out. Decided once –
    /// then it stays.
    pub(super) fn check_outcome(&mut self, command_center_fell: bool) {
        if self.outcome.is_some() {
            return;
        }
        let tick = self.tick;
        let goals = 0..self.story.goals.len();
        let lost_goal = goals.clone().any(|g| self.story.goals[g].defeat && self.goal_reached(g));
        let cc = |b: &&super::Building| b.kind == BuildingKind::CommandCenter;
        let lost_one = command_center_fell || self.buildings.iter().filter(cc).any(|b| b.infected);
        let fell = lost_one && !self.buildings.iter().filter(cc).any(|b| !b.infected);
        if fell || lost_goal || self.ending == Some(Ending::Lost) {
            self.decide(Outcome::Lost { tick });
            return;
        }
        let mut victories = goals.filter(|&g| !self.story.goals[g].defeat).peekable();
        let goals_won = victories.peek().is_some() && victories.all(|g| self.goal_reached(g));
        if self.ending == Some(Ending::Won) || goals_won || self.events.iter().any(|e| e.won && e.state.fired > 0) {
            self.decide(Outcome::Won { tick });
            return;
        }
        let mut finals = self.events.iter().filter(|e| e.won_when_dead).peekable();
        let all_came = finals.peek().is_some() && finals.all(|e| e.is_done());
        // The dead are already gone (`remove_dead`): every zombie left is alive. One that can never reach the command
        // center (shut in somewhere on a map made in the editor) doesn't keep the round from being won.
        let coming = |z: &&Zombie| z.final_wave && self.to_base.reachable(z.pos.x.floor() as i32, z.pos.y.floor() as i32);
        // Sleepers still waking to join the final wave (`sleep.rs`) count as coming too.
        if all_came && !self.final_wave_pending() && !self.zombies.iter().any(|z| coming(&z)) && !self.sleep.still_waking() {
            self.decide(Outcome::Won { tick });
        }
    }

    /// The round ends like this – and the story's script for it runs (TAB's "On Game Won" / "On Game Lost").
    fn decide(&mut self, outcome: Outcome) {
        self.outcome = Some(outcome);
        self.fire(&match outcome {
            Outcome::Won { .. } => TriggerEvent::Won,
            Outcome::Lost { .. } => TriggerEvent::Lost,
        });
    }

    /// The round lasts `days` days (at least 1) – the events that win the round move to the end of the new last day.
    pub(super) fn set_days(&mut self, days: u32) {
        let days = days.max(1);
        self.days = days;
        for e in 0..self.events.len() {
            if self.events[e].won {
                self.events[e].hour = days * 24;
                self.roll_event(e);
            }
        }
    }
}
