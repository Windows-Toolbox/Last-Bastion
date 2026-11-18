//! Upgrades like StarCraft 2's: a **track** ("Infantry weapons") has a level; a research (or a map, or a script's
//! action Research) raises it. Nothing is handed out to units: every unit and building lists in its own file what *one
//! level* of a track gives *it* (`"upgrades": { "infantry_weapons": { "damage": 1 } }` – flat, like SC2: a marine +1, a
//! marauder +2), and its values are worked out from the track's level. So an upgrade counts for what already stands and
//! for what is built later.
//!
//! The levels are `World::upgrade_levels` (kept in the map file: a campaign map can start on level 2). A unit's or
//! building's sums are cached (`Bonus`) and made again when the levels change (`World::refresh_upgrades`, every tick –
//! a number compare). `data\upgrades.json` names the tracks and their number of levels.

use std::collections::BTreeMap;

use super::World;
use crate::data;
use crate::kinds::UpgradeBonus;

/// What the levels give one unit or building, all together – a flat number each.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Bonus {
    pub hp: u32,
    pub shield: u32,
    pub damage: u32,
    /// Armor points; a building's shield armor points.
    pub armor: u32,
    pub shield_armor: u32,
    /// Added to every extra damage of the weapon (`attrs.rs`).
    pub bonus_damage: u32,
    pub range: f32,
    pub sight: f32,
    /// Tiles per second.
    pub speed: f32,
    /// Percent points added to its attack speed.
    pub attack: u32,
}

impl Bonus {
    /// The sum of every track's bonus times its level. A track that doesn't exist (any more) counts nothing.
    pub fn of(listed: &BTreeMap<String, UpgradeBonus>, levels: &BTreeMap<String, u32>) -> Bonus {
        let mut sum = Bonus::default();
        for (track, per_level) in listed {
            let level = levels.get(track).copied().unwrap_or(0);
            if level == 0 {
                continue;
            }
            sum.hp += per_level.hp * level;
            sum.shield += per_level.shield * level;
            sum.damage += per_level.damage * level;
            sum.armor += per_level.armor * level;
            sum.shield_armor += per_level.shield_armor * level;
            sum.bonus_damage += per_level.bonus_damage * level;
            sum.range += per_level.range * level as f32;
            sum.sight += per_level.sight * level as f32;
            sum.speed += per_level.speed * level as f32;
            sum.attack += per_level.attack_speed * level;
        }
        sum
    }
}

impl World {
    /// The level of a track (0: none yet – or no such track).
    pub fn upgrade_level(&self, track: &str) -> u32 {
        self.upgrade_levels.get(track).copied().unwrap_or(0)
    }

    /// A track is at this level now – at least 0 and at most the track's number of levels; a track that isn't in
    /// `data\upgrades.json` is ignored. Level 0 takes the entry out.
    pub fn set_upgrade_level(&mut self, track: &str, level: u32) {
        let Some(found) = data::get().upgrades.tracks.iter().find(|t| t.id == track) else {
            return;
        };
        let level = level.min(found.levels);
        if level == 0 {
            self.upgrade_levels.remove(track);
        } else {
            self.upgrade_levels.insert(track.to_owned(), level);
        }
        self.upgrade_rev += 1;
    }

    /// Research done raises its track (`finish_research`): never lowers it.
    pub(super) fn raise_upgrade(&mut self, track: &str, level: u32) {
        if level > self.upgrade_level(track) {
            self.set_upgrade_level(track, level);
        }
    }

    /// Every unit and building whose sums are from before the last change of a level (or new): worked out again. A unit
    /// that gets more hit points gets them right away, like in SC2; fewer: it can't have more than its maximum.
    /// Every tick, at its end – so what was built this tick has its bonus at once.
    pub(super) fn refresh_upgrades(&mut self) {
        let rev = self.upgrade_rev;
        let levels = &self.upgrade_levels;
        for unit in self.units.iter_mut().filter(|u| u.up_rev != rev) {
            let before = unit.max_hp();
            unit.up = Bonus::of(&unit.stats().upgrades, levels);
            unit.up_rev = rev;
            let after = unit.max_hp();
            unit.hp = if after >= before { unit.hp + (after - before) } else { unit.hp.min(after).max(1) };
        }
        // The zombies' own tracks (`zombie_weapons`, `zombie_armor` …) – a zombie asleep has none until it wakes.
        for zombie in self.zombies.iter_mut().filter(|z| z.up_rev != rev) {
            zombie.up = Bonus::of(&zombie.breed.stats().upgrades, levels);
            zombie.up_rev = rev;
        }
        for building in self.buildings.iter_mut().filter(|b| b.up_rev != rev) {
            let (hp, shield) = (building.max_hp(), building.max_shield());
            building.up = Bonus::of(&building.stats().upgrades, levels);
            building.up_rev = rev;
            let (new_hp, new_shield) = (building.max_hp(), building.max_shield());
            // Still being built, it grows towards the new maximum by itself (`build_step`).
            if !building.is_built() {
                continue;
            }
            building.hp = if new_hp >= hp { building.hp + (new_hp - hp) } else { building.hp.min(new_hp) };
            building.shield = if new_shield >= shield { building.shield + (new_shield - shield) } else { building.shield.min(new_shield) };
        }
    }
}
