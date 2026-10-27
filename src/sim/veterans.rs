//! Veterans (DESIGN.md 7.3): every zombie that dies gives experience (`xp` in `data\zombies.json` – a walker 1, a fat
//! one 10), split among your units within `share_radius` of where it fell – so the ones that held the line together
//! grow together, whoever fired the last shot. A veteran counts as half a share: fresh units next to veterans still
//! learn at a sensible pace (`xp × own share / sum of shares`). With `xp` of it (`veterans` in `data\units.json`) a unit
//! becomes a veteran – the only level: more hit points and damage (`Unit::max_hp`, `Unit::damage`), a mark over its
//! head, and in debt it deserts a third as likely (`debt.rs`). Mercenaries take no share – they are better already.
//! Kept in thousandths, so a share of a walker among ten units isn't lost to rounding.

use super::World;
use crate::data;

impl World {
    /// The zombies that died this tick give their experience to the units around them – before they are taken out of
    /// the list (`remove_dead`). In the order of the lists, the same on every PC.
    pub(super) fn share_experience(&mut self) {
        let veterans = &data::get().units.veterans;
        let radius = veterans.share_radius;
        for z in 0..self.zombies.len() {
            let (pos, breed) = (self.zombies[z].pos, self.zombies[z].breed);
            if self.zombies[z].is_alive() {
                continue;
            }
            // (unit, its share): 2 for a fresh unit, 1 for a veteran. Worked out first – the loop below changes the
            // units, and while it does, nothing else may read them.
            let near: Vec<(usize, u32)> = (0..self.units.len())
                .filter(|&u| {
                    let unit = &self.units[u];
                    unit.hp > 0 && !unit.stats().mercenary && unit.pos.distance(pos) <= radius
                })
                .map(|u| (u, if self.units[u].veteran { 1 } else { 2 }))
                .collect();
            let total: u32 = near.iter().map(|&(_, share)| share).sum();
            if total == 0 {
                continue;
            }
            let xp = breed.stats().xp * 1000;
            for (u, share) in near {
                let gain = xp * share / total;
                let unit = &mut self.units[u];
                unit.xp += gain;
                if !unit.veteran && unit.xp >= veterans.xp * 1000 {
                    unit.make_veteran();
                }
            }
        }
    }
}
