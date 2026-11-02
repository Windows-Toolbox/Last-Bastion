//! What the map's landscape does to the whole round (`data::Modifiers`, like TAB's landscapes' `Mods`): how well the
//! zombies hear and see, how fast everyone walks, how far your eyes reach. The power buildings need is in
//! `economy.rs` (`energy_use`), the hunters' game too (`hunting_percent`). Everything here asks `World::modifiers` –
//! the stats in `data\` stay as they are.

use super::zombies::Breed;
use super::World;
use crate::data::{self, Modifiers};

impl World {
    /// How far a zombie of this breed sees (tiles): its `sight`, changed by the map.
    pub fn zombie_sight(&self, breed: Breed) -> f32 {
        breed.stats().sight * Modifiers::factor(self.modifiers.zombie_sight)
    }

    /// How far it hears: `hearing` times its sight – a landscape where they see further, they hear further too.
    pub(super) fn hearing_range(&self, breed: Breed) -> f32 {
        data::get().sound.hearing * self.zombie_sight(breed)
    }

    /// What a sound's level is multiplied with for this breed (`sound.rs`): TAB's 20 × its `awareness`, changed by the
    /// map (TAB's `Zombie ActivityAwarenessFactor`).
    pub(super) fn hear_factor(&self, breed: Breed) -> u32 {
        super::sound::hear_factor(breed.stats().awareness * Modifiers::factor(self.modifiers.zombie_hearing))
    }

    /// The zombies' speed times this – rolled into every zombie's own pace when it comes onto the map.
    pub(super) fn zombie_pace(&self) -> f32 {
        Modifiers::factor(self.modifiers.zombie_speed)
    }

    /// Your units' speed times this.
    pub(super) fn unit_pace(&self) -> f32 {
        Modifiers::factor(self.modifiers.unit_speed)
    }

    /// How far one of your units or buildings that sees `sight` tiles sees on this map.
    pub fn eye_range(&self, sight: f32) -> f32 {
        sight * Modifiers::factor(self.modifiers.unit_sight)
    }
}
