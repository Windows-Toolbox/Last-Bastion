//! Zombies that attack from afar (DESIGN.md 8.1): the spitter – TAB's venom – and the infested – StarCraft 2's
//! infested marine. *Whom* they shoot at is decided while they walk (`zombie_intent` in `zombies.rs`): the nearest of
//! your units in range and in sight, otherwise a building in range. Here: finding that unit, and what a shot does.
//!
//! - The infested's bullet hits at once – like your soldiers' (`projectile_speed` 0).
//! - The spitter's acid flies (`projectile_speed`) and lands where the target stood when it spat – whoever steps away
//!   in time is safe – and burns everyone within `splash` of that spot, your units and buildings alike. Never other
//!   zombies, like TAB's venom.

use macroquad::math::Vec2;

use super::fx::Fx;
use crate::attrs::ExtraDamage;
use super::zombies::{Breed, Hit, Zombie};
use super::{World, TICKS_PER_SECOND, UNIT_RADIUS};

/// Acid in flight.
#[derive(Clone)]
pub struct Spit {
    pub from: Vec2,
    pub to: Vec2,
    /// Ticks the whole flight takes, and how many are left.
    flight: u32,
    pub(super) left: u32,
    breed: Breed,
    /// What it does where it lands: its breed's damage, a boss's own – and the extra damage against attributes.
    damage: u32,
    extra: ExtraDamage,
}

impl Spit {
    /// How far along its way it is: 0 = just spat, 1 = landing. `alpha`: how far between two ticks – for drawing.
    pub fn progress(&self, alpha: f32) -> f32 {
        (((self.flight - self.left) as f32 + alpha) / self.flight as f32).min(1.0)
    }
}

impl World {
    /// The nearest of your units on the ground within the zombie's `range` that it can see – forests block its view,
    /// walls don't. Units are few: only the tiles around it in `unit_grid` are looked at. Equally far: the lower index
    /// (DESIGN.md 12.8).
    pub(super) fn ranged_target(&self, zombie: &Zombie) -> Option<usize> {
        let range = zombie.breed.stats().range;
        let r = range.ceil() as i32;
        let (cx, cy) = (zombie.pos.x.floor() as i32, zombie.pos.y.floor() as i32);
        let mut best: Option<(f32, usize)> = None;
        for dy in -r..=r {
            for dx in -r..=r {
                for u in self.unit_grid.cell(cx + dx, cy + dy) {
                    let unit = &self.units[u];
                    let d = unit.pos.distance(zombie.pos);
                    if unit.is_inside() || d > range {
                        continue;
                    }
                    let better = best.is_none_or(|(best_d, best_u)| d < best_d || (d == best_d && u < best_u));
                    // The line of sight last – it's the expensive part.
                    if better && self.map.clear_sight(zombie.pos, unit.pos) {
                        best = Some((d, u));
                    }
                }
            }
        }
        best.map(|(_, u)| u)
    }

    /// A ranged zombie at `from` shoots at `target`: acid goes on its way (`fly_spits`), a bullet hits at once.
    pub(super) fn zombie_shoots(&mut self, from: Vec2, breed: Breed, target: Hit, damage: u32, extra: ExtraDamage) {
        let to = match target {
            Hit::Unit(u) => self.units[u].pos,
            // The spot of the building closest to the zombie.
            Hit::Building(b) => from.clamp(self.buildings[b].corner(), self.buildings[b].far_corner()),
        };
        let stats = breed.stats();
        if stats.projectile_speed > 0.0 {
            // Rounded with `floor` – the list of allowed float operations stays short (`sim/mod.rs`).
            let flight = ((to.distance(from) / stats.projectile_speed * TICKS_PER_SECOND + 0.5).floor() as u32).max(1);
            self.spits.push(Spit { from, to, flight, left: flight, breed, damage, extra });
            return;
        }
        self.zombie_shots.push((from, to));
        if stats.splash > 0.0 {
            self.splash(to, breed, damage, extra);
            return;
        }
        match target {
            Hit::Unit(u) => {
                let damage = damage + extra.against(self.units[u].attributes());
                if stats.venom {
                    self.units[u].take_venom(damage, self.tick);
                } else {
                    self.units[u].take_hit(damage, self.tick);
                }
            }
            Hit::Building(b) => {
                let damage = damage + extra.against(self.buildings[b].attributes());
                if self.buildings[b].take_damage(damage) {
                    self.infect(b);
                }
            }
        }
    }

    /// Every tick: the acid in flight comes a step closer, and what lands now burns (`splash`). In the order of the
    /// list – the same on every PC.
    pub(super) fn fly_spits(&mut self) {
        let mut landed = Vec::new();
        self.spits.retain_mut(|spit| {
            spit.left = spit.left.saturating_sub(1);
            if spit.left == 0 {
                landed.push((spit.to, spit.breed, spit.damage, spit.extra));
            }
            spit.left > 0
        });
        for (at, breed, damage, extra) in landed {
            self.fx.push(Fx::Acid { at });
            self.splash(at, breed, damage, extra);
        }
    }

    /// A shot lands at `at`: every unit on the ground whose body is within `splash` of the spot takes the breed's damage
    /// (after its armor – venom: its `venom_factor` of it), and every building that close (a broken shield infects it,
    /// like a hit). Units in a tower can't be hit – the tower is. Without splash only what stands right on the spot.
    fn splash(&mut self, at: Vec2, breed: Breed, damage: u32, extra: ExtraDamage) {
        let stats = breed.stats();
        let radius = stats.splash.max(0.1);
        let tick = self.tick;
        for unit in self.units.iter_mut().filter(|u| !u.is_inside() && u.pos.distance(at) <= radius + UNIT_RADIUS) {
            let damage = damage + extra.against(unit.attributes());
            if stats.venom {
                unit.take_venom(damage, tick);
            } else {
                unit.take_hit(damage, tick);
            }
        }
        let mut infected = Vec::new();
        for (b, building) in self.buildings.iter_mut().enumerate() {
            if !building.zombies_ignore() && building.distance_to(at) <= radius && building.take_damage(damage + extra.against(building.attributes())) {
                infected.push(b);
            }
        }
        for b in infected {
            self.infect(b);
        }
    }
}
