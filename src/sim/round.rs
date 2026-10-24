//! The round as a whole (DESIGN.md 4): how many days it lasts and which waves a new round gets (`plan_waves`), how it
//! ends – lost when the last command center falls, won when the final wave is dead (`check_outcome`) – and what
//! happened in it along the way (`Score`, for the end screen).

use super::spawning::{TimedWave, WaveFrom};
use super::zombies::{Breed, Zombie};
use super::{World, DAY_TICKS, HOUR_TICKS};
use crate::data::{self, GameDuration};
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

/// The tick at which day `day` (from 1) reaches hour `hour`.
pub fn tick_of(day: u32, hour: u32) -> u64 {
    (day.max(1) as u64 - 1) * DAY_TICKS + hour as u64 * HOUR_TICKS
}

/// The waves of a new round of `days` days (DESIGN.md 8.2, the numbers are `waves` in `data\spawning.json`): from
/// `first_wave_day` of the `duration` on one every `wave_every_days` days, growing from `first` to `last` runners per
/// side – in the first half of the round from one side, then from two. The first waves are only runners; the
/// `specials` (riots, infested, spitters, fat ones) come along from their share of the waves on, more and more of them – one entry per breed at the same time,
/// from the same sides (`fire_timed_waves`). On the last day the final wave from all four sides with every breed in
/// full: it calls every zombie on the map and shows the whole map. `strength`: every count times this
/// (`waves_percent` of the difficulty, 1 = 100 %). 0 days: no waves – a map without a set length.
pub fn plan_waves(days: u32, duration: &GameDuration, strength: f32) -> Vec<TimedWave> {
    let plan = &data::get().spawning.waves;
    let mut waves = Vec::new();
    if days == 0 {
        return waves;
    }
    // Rounded with `floor` – the list of float operations allowed in the simulation stays short (`sim/mod.rs`).
    let scaled = |count: f32| (count * strength + 0.5).floor() as u32;
    let wave = |at: u64, count: u32, sides: u32, breed: Breed, last: bool| TimedWave {
        at,
        count,
        from: WaveFrom::Sides(sides),
        breed,
        final_wave: last,
        reveal: last,
    };
    // The days with a wave: from the first one every few days, but not on the last day – that one is the final wave's.
    let wave_days: Vec<u32> = (duration.first_wave_day.max(1)..days).step_by(duration.wave_every_days.max(1) as usize).collect();
    let n = wave_days.len();
    for (i, &day) in wave_days.iter().enumerate() {
        // 0 for the first wave, 1 for the last one before the final wave.
        let share = if n > 1 { i as f32 / (n - 1) as f32 } else { 0.0 };
        let runners = plan.first as f32 + (plan.last as f32 - plan.first as f32) * share;
        let sides = if 2 * i < n { 1 } else { 2 };
        let at = tick_of(day, plan.hour);
        waves.push(wave(at, scaled(runners).max(1), sides, Breed::Runner, false));
        for special in plan.specials.iter().filter(|s| share >= s.from) {
            // A quarter of its share where it starts, all of it by the last wave.
            let grown = if special.from < 1.0 { (share - special.from) / (1.0 - special.from) } else { 1.0 };
            let count = scaled(runners * special.share * (0.25 + 0.75 * grown));
            if count > 0 {
                waves.push(wave(at, count, sides, special.breed, false));
            }
        }
    }
    let at = tick_of(days, plan.final_hour);
    let finals = std::iter::once((Breed::Runner, 1.0)).chain(plan.specials.iter().map(|s| (s.breed, s.share)));
    for (breed, share) in finals {
        let count = scaled(plan.final_runners as f32 * share);
        if count > 0 {
            waves.push(wave(at, count, 4, breed, true));
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
        // The dead are already gone (`remove_dead`): every zombie left is alive. One that can never reach the command
        // center (shut in somewhere on a map made in the editor) doesn't keep the round from being won.
        let coming = |z: &&Zombie| z.final_wave && self.to_base.reachable(z.pos.x.floor() as i32, z.pos.y.floor() as i32);
        // Sleepers still waking to join the final wave (`sleep.rs`) count as coming too.
        if all_came && !self.zombies.iter().any(|z| coming(&z)) && !self.sleep.still_waking() {
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
