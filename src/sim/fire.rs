//! Fire – like TAB's (`CInflamable`, `Fire`): Lucifer's flame sets a zombie alight (`burn_chance` of its hits, TAB's
//! `BurnFactor`). It burns for its breed's `burn_seconds` (TAB's `BurningTime`: 3 s, the giant not at all) and every
//! half second takes TAB's fire damage: 5 × its `fire_factor`, armor doesn't help (TAB's `FireDamage`,
//! `AddDamage(applyArmour: false)`). Being set alight again starts the time over.
//!
//! Not like TAB (yet): TAB's fire also spreads – everyone standing in it heats up and catches fire after a while
//! (`InflamableTime`), your units too. Here only the ones Lucifer hits burn.

use super::{World, TICKS_PER_SECOND};

/// TAB's fire hurts every half second (`IsIAFrameModule(60)`) …
const BURN_EVERY: u64 = (TICKS_PER_SECOND / 2.0) as u64;
/// … this much, times the burning one's `fire_factor` (TAB's `FireDamage` = `FireDamageFactor` × 5).
const BURN_DAMAGE: f32 = 5.0;

impl World {
    /// Zombie `z` catches fire – if its breed burns at all.
    pub(super) fn set_alight(&mut self, z: usize) {
        let ticks = self.zombies[z].breed.stats().burn;
        if ticks > 0 {
            self.zombies[z].burning = ticks;
        }
    }

    /// Every tick: the burning burn. Each takes its damage on its own tick of the half second (by its `id`), so a
    /// burning crowd doesn't all lose its hit points at once. Who burns to death counts as killed (nobody's kill).
    pub(super) fn burn(&mut self) {
        let tick = self.tick;
        for zombie in self.zombies.iter_mut().filter(|z| z.burning > 0 && z.is_alive()) {
            zombie.burning -= 1;
            if (tick + zombie.id as u64).is_multiple_of(BURN_EVERY) {
                let damage = (BURN_DAMAGE * zombie.breed.stats().fire_factor).round() as u32;
                zombie.hurt(damage, tick);
            }
        }
    }
}
