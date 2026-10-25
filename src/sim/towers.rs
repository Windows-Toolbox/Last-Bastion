//! Towers that shoot by themselves (DESIGN.md decision 33): the ballista and the machine gun tower – no units inside,
//! a `weapon` in `data\buildings.json` instead: damage, range, reload, noise per shot, and whom it aims at. Unlike your
//! units it doesn't turn or aim: once reloaded it shoots at what's in range and in sight (not through forest).
//!
//! It shoots while it works (`Building::works` – finished, switched on, not infected). One that takes power reloads
//! slower when its grid is short of it, like training in the barracks. A shot zombie storms like one hit by your units
//! (`start_seeking`), and every shot makes noise – the machine gun tower a lot of it.

use macroquad::math::Vec2;

use super::World;

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
            let target = if weapon.toughest {
                self.toughest_zombie(from, weapon.range)
            } else {
                self.nearest_zombie(from, weapon.range, |_| true)
            };
            // Nothing in range: it stays loaded and shoots the moment something comes.
            let Some(z) = target else {
                continue;
            };
            let zombie = &mut self.zombies[z];
            let damage = weapon.damage.saturating_sub(zombie.breed.stats().armor).max(1);
            zombie.hp = zombie.hp.saturating_sub(damage);
            let to = zombie.pos;
            self.shots.push((from, to));
            self.start_seeking(z);
            self.make_colony_sound(from, weapon.noise);
            self.buildings[b].reload = weapon.reload * 1000;
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

    /// How much of its reload a tower still has to go, as a share (0 = ready) – for the panel.
    pub fn reload_share(&self, b: usize) -> f32 {
        let building = &self.buildings[b];
        building.stats().weapon.as_ref().map_or(0.0, |w| building.reload as f32 / (w.reload.max(1) * 1000) as f32)
    }
}
