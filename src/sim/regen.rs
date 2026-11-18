//! Healing themselves, like TAB's `CLife`: every `regen_every_seconds` (`data\units\`) every unit and zombie
//! that hasn't been hit for `regen_after_seconds` gets its `regen` share of its full hit points back, rounded up
//! (TAB's `LifeRegenerationPerPeriod`: a walker 3 of 35, a giant 50 of 10,000) – each one on its own tick, by its
//! number. Asleep they heal too: what they would have got is added when they wake (`sleep.rs`).

use super::World;
use crate::data;

/// What one with `max` hit points and this `regen` share gets back every time – rounded up, like TAB.
pub(super) fn regen_amount(max: u32, regen: f32) -> u32 {
    (max as f32 * regen).ceil() as u32
}

impl World {
    /// Once per tick, after everyone was hit.
    pub(super) fn regenerate(&mut self) {
        let values = &data::get().units;
        let every = values.regen_every.max(1) as u64;
        let after = values.regen_after as u64;
        let tick = self.tick;
        let due = |id: u32, hurt_at: u64| (tick + id as u64).is_multiple_of(every) && tick >= hurt_at + after;
        for unit in self.units.iter_mut().filter(|u| u.hp > 0 && due(u.id, u.hurt_at)) {
            let max = unit.max_hp();
            unit.hp = (unit.hp + regen_amount(max, unit.stats().regen)).min(max);
        }
        for zombie in self.zombies.iter_mut().filter(|z| z.is_alive() && due(z.id, z.hurt_at)) {
            let stats = zombie.breed.stats();
            let max = zombie.max_hp();
            zombie.hp = (zombie.hp + regen_amount(max, stats.regen)).min(max);
        }
    }
}
