//! The round as a whole (DESIGN.md 4): how many days it lasts and which waves a new round gets (`plan_waves`), how it
//! ends – lost when the last command center falls, won when the final wave is dead (`check_outcome`) – and what
//! happened in it along the way (`Score`, for the end screen).

use super::spawning::{TimedWave, WaveFrom};
use super::zombies::Breed;
use super::{World, DAY_TICKS, HOUR_TICKS};
use crate::data;
use crate::kinds::BuildingKind;

/// How a round ended, and at which tick.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Outcome {
    /// Every final wave has come and none of its zombies is left – while a command center still stands.
    Won { tick: u64 },
    /// The last command center fell.
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
    pub buildings_built: u32,
    pub buildings_lost: u32,
    pub gold_earned: u32,
    pub gold_spent: u32,
}

/// The tick at which day `day` (from 1) reaches hour `hour`.
pub fn tick_of(day: u32, hour: u32) -> u64 {
    (day.max(1) as u64 - 1) * DAY_TICKS + hour as u64 * HOUR_TICKS
}

/// The waves of a new round of `days` days (DESIGN.md 8.2, the numbers are `waves` in `data\spawning.json`): from
/// `first_day` on one every `every_days` days, growing from `first` to `last` runners per side – in the first half of
/// the round from one side, then from two. On the last day the final wave from all four sides: runners and fat ones,
/// two entries at the same time. `strength`: every count times this (the setup's "Waves", 1 = 100 %).
/// 0 days: no waves – a map without a set length.
pub fn plan_waves(days: u32, strength: f32) -> Vec<TimedWave> {
    let plan = &data::get().spawning.waves;
    let mut waves = Vec::new();
    if days == 0 {
        return waves;
    }
    // Rounded with `floor` – the list of float operations allowed in the simulation stays short (`sim/mod.rs`).
    let scaled = |count: f32| ((count * strength + 0.5).floor() as u32).max(1);
    // The days with a wave: from the first one every few days, but not on the last day – that one is the final wave's.
    let wave_days: Vec<u32> = (plan.first_day.max(1)..days).step_by(plan.every_days.max(1) as usize).collect();
    let n = wave_days.len();
    for (i, &day) in wave_days.iter().enumerate() {
        // 0 for the first wave, 1 for the last one before the final wave.
        let share = if n > 1 { i as f32 / (n - 1) as f32 } else { 0.0 };
        let count = plan.first as f32 + (plan.last as f32 - plan.first as f32) * share;
        let sides = if 2 * i < n { 1 } else { 2 };
        waves.push(TimedWave { at: tick_of(day, plan.hour), count: scaled(count), from: WaveFrom::Sides(sides), breed: Breed::Runner, final_wave: false });
    }
    let at = tick_of(days, plan.final_hour);
    for (breed, count) in [(Breed::Runner, plan.final_runners), (Breed::Fat, plan.final_fat)] {
        if count > 0 {
            waves.push(TimedWave { at, count: scaled(count as f32), from: WaveFrom::Sides(4), breed, final_wave: true });
        }
    }
    waves
}

impl World {
    /// The end of the round, checked at the end of every tick. Lost: the last command center fell this tick
    /// (`command_center_fell` – a map without one from the start can't be lost that way). Won: every final wave has
    /// come and none of its zombies is alive any more. Decided once – then it stays.
    pub(super) fn check_outcome(&mut self, command_center_fell: bool) {
        if self.outcome.is_some() {
            return;
        }
        let tick = self.tick;
        if command_center_fell && !self.buildings.iter().any(|b| b.kind == BuildingKind::CommandCenter) {
            self.outcome = Some(Outcome::Lost { tick });
            return;
        }
        let finals: Vec<u64> = self.waves.iter().filter(|w| w.final_wave).map(|w| w.at).collect();
        let all_came = !finals.is_empty() && finals.iter().all(|&at| at <= tick);
        // The dead are already gone (`remove_dead`): every zombie left is alive.
        if all_came && !self.zombies.iter().any(|z| z.final_wave) {
            self.outcome = Some(Outcome::Won { tick });
        }
    }

    /// The round lasts `days` days (at least 1) – the final waves move along to the new last day, at the same hour.
    pub(super) fn set_days(&mut self, days: u32) {
        let days = days.max(1);
        self.days = days;
        for wave in self.waves.iter_mut().filter(|w| w.final_wave) {
            wave.at = tick_of(days, 0) + wave.at % DAY_TICKS;
        }
    }
}
