//! Towers that shoot by themselves (DESIGN.md decision 33): the ballista and the machine gun tower – no units inside,
//! a `weapon` in `data\buildings\` instead: damage, range, reload, noise per shot, and whom it aims at. Unlike your
//! units it doesn't turn or aim: once reloaded it shoots at what's in range and in sight (not through forest).
//!
//! It shoots while it works (`Building::works` – finished, switched on, not infected). One that takes power reloads
//! slower when its grid is short of it, like training in the barracks. A shot zombie – and the ones right around it –
//! goes for the tower like one hit by your units (`zombie_hit_by`), and every shot makes noise – the machine gun tower a
//! lot of it.

use macroquad::math::Vec2;

use super::areas::Blow;
use super::zombies::Aim;
use super::{after_armor, Fx, Tweak, World};
use crate::attrs::ExtraDamage;

impl World {
    /// Every tick: towers reload – and shoot when they're ready and something is in range.
    pub(super) fn fire_towers(&mut self) {
        // The grids only if a shooting tower takes power – most don't.
        let needs_power = self.buildings.iter().any(|b| b.stats().weapon.is_some() && b.stats().energy_use > 0);
        let power = needs_power.then(|| self.power());
        for b in 0..self.buildings.len() {
            let building = &self.buildings[b];
            let Some(weapon) = building.stats().weapon.as_ref() else {
                continue;
            };
            if !building.works() {
                continue;
            }
            let share = match &power {
                Some(power) if building.stats().energy_use > 0 => power.share(self, b),
                _ => 1000,
            };
            if building.reload > 0 {
                self.buildings[b].reload = building.reload.saturating_sub(share);
                continue;
            }
            let from = building.center();
            let (id, kind) = (building.id, building.kind);
            let tweak = building.tweak;
            let up_attack = building.up.attack;
            let (range, damage) = ((weapon.range + building.up.range) * tweak.range as f32 / 100.0, Tweak::scale(weapon.damage + building.up.damage, tweak.damage));
            let extra = ExtraDamage::new(&weapon.bonus, building.up.bonus_damage);
            // The shocking tower: every zombie in range at once. The others: one.
            let targets: Vec<usize> = if weapon.all_in_range {
                self.zombies_in_shot(from, range)
            } else if weapon.toughest {
                self.toughest_zombie(from, range).into_iter().collect()
            } else {
                self.nearest_zombie(from, range, |_| true).into_iter().collect()
            };
            // Nothing in range: it stays loaded and shoots the moment something comes.
            if targets.is_empty() {
                continue;
            }
            let by = Aim::Building(id);
            for z in targets {
                let to = self.zombies[z].pos;
                self.shots.push((from, to));
                if weapon.all_in_range {
                    self.fx.push(Fx::Shock { building: id, from, to });
                } else {
                    self.fx.push(Fx::TowerShot { building: id, kind, from, to });
                }
                if weapon.splash > 0.0 {
                    // The executor: everyone round its target, all of it (`areas.rs`).
                    let blow = Blow { damage, edge: 1.0, shooter: 0, by, fire: false, burn_chance: 0.0, friendly: false, extra, keep: 1000 };
                    self.hit_splash(to, weapon.splash, &blow);
                    continue;
                }
                let zombie = &mut self.zombies[z];
                let damage = after_armor(damage + extra.against(zombie.attributes()), zombie.armor());
                zombie.hurt(damage, self.tick);
                self.zombie_hit_by(z, by);
            }
            self.make_colony_sound(from, weapon.noise);
            self.buildings[b].reload = tweak.quicker_with(weapon.reload, up_attack) * 1000;
        }
    }

    /// The living zombie within `range` with the most hit points that's in sight – the ballista's target: its bolt is
    /// wasted on a walker. Equally tough: the nearer one, then the lower index (DESIGN.md 12.8).
    fn toughest_zombie(&self, pos: Vec2, range: f32) -> Option<usize> {
        let r = range.ceil() as i32;
        let (cx, cy) = (pos.x.floor() as i32, pos.y.floor() as i32);
        // (hit points, distance, index) of the best so far.
        let mut best: Option<(u32, f32, usize)> = None;
        for dy in -r..=r {
            for dx in -r..=r {
                for z in self.grid.cell(cx + dx, cy + dy) {
                    let zombie = &self.zombies[z];
                    let d = zombie.pos.distance(pos);
                    if !zombie.is_alive() || d > range {
                        continue;
                    }
                    let better = match best {
                        None => true,
                        Some((hp, best_d, best_z)) => {
                            zombie.hp > hp || (zombie.hp == hp && (d < best_d || (d == best_d && z < best_z)))
                        }
                    };
                    // The line of sight only for one that would be better – it's the expensive part.
                    if better && self.map.clear_sight(pos, zombie.pos) {
                        best = Some((zombie.hp, d, z));
                    }
                }
            }
        }
        best.map(|(_, _, z)| z)
    }

    /// Every living zombie within `range` of `from` and in sight – the shocking tower's targets, in the order of the
    /// grid (the same on every PC).
    fn zombies_in_shot(&self, from: Vec2, range: f32) -> Vec<usize> {
        let r = range.ceil() as i32;
        let (cx, cy) = (from.x.floor() as i32, from.y.floor() as i32);
        let mut found = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                for z in self.grid.cell(cx + dx, cy + dy) {
                    let zombie = &self.zombies[z];
                    if zombie.is_alive() && zombie.pos.distance(from) <= range && self.map.clear_sight(from, zombie.pos) {
                        found.push(z);
                    }
                }
            }
        }
        found
    }

    /// How much of its reload a tower still has to go, as a share (0 = ready) – for the panel.
    pub fn reload_share(&self, b: usize) -> f32 {
        let building = &self.buildings[b];
        building.stats().weapon.as_ref().map_or(0.0, |w| building.reload as f32 / (building.tweak.quicker_with(w.reload, building.up.attack).max(1) * 1000) as f32)
    }
}
