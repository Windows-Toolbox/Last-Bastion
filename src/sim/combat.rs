//! Combat of your units: whom they aim at (the nearest zombie in sight, with none in range a house of a village of
//! doom, for medics the nearest hurt unit) and shooting. Zombies hitting back is part of their walking (`zombies.rs`).
//!
//! A shot goes like in StarCraft, with the times from `data\units.json`:
//! 1. **turn** to face the target (`turn_around_seconds` for 180°),
//! 2. wait until the weapon is **reloaded** (`cooldown_seconds`, counted from the last shot),
//! 3. **aim** – draw the bow, raise the rifle (`aim_seconds`),
//! 4. shoot – and the reloading starts.
//!
//! Another target, or none, and the aiming starts over. That's why a unit keeps its target (`kept_target`) instead of
//! jumping to whichever zombie is a bit closer right now.

use macroquad::math::Vec2;

use super::units::Intent;
use super::zombies::{self, Breed};
use super::{slide, World, TICKS_PER_SECOND};

/// The noise of one shot (`noise` in `data\units.json`), as a whole number for the sound grid.
fn shot_noise(noise: f32) -> u32 {
    (noise + 0.5).floor().max(0.0) as u32
}

/// What a unit aims at this tick: a zombie (shoot) or – medics – one of its own (beam).
/// The number is the index in the respective list.
#[derive(Clone, Copy)]
pub(super) enum Target {
    Zombie(usize),
    Ally(usize),
    /// A house of a village of doom (`doom.rs`).
    Doom(usize),
}

/// What a unit has been aiming at – by `id`, because it's kept from tick to tick and the indices change when the
/// dead are taken out of the lists.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Aim {
    Zombie(u32),
    Ally(u32),
    Doom(u32),
}

impl World {
    /// Turn, reload, aim, shoot (see the top of the file) – or, for medics, beam – at what each unit wants to aim at
    /// this tick (`intent`).
    pub(super) fn shoot(&mut self, intents: &[Intent]) {
        // In debt the units shoot slower and weaker (`debt.rs`) – read before `unit` borrows `self`.
        let keep = self.army_keep;
        for (i, intent) in intents.iter().enumerate() {
            // Reloading goes on whatever the unit does – walking, turning, nothing.
            self.units[i].cooldown = self.units[i].cooldown.saturating_sub(1);
            let (aim, to) = match intent.target {
                Some(Target::Zombie(z)) if self.zombies[z].is_alive() => {
                    (Aim::Zombie(self.zombies[z].id), self.zombies[z].pos)
                }
                Some(Target::Ally(a)) => (Aim::Ally(self.units[a].id), self.units[a].pos),
                Some(Target::Doom(d)) if self.doom[d].hp > 0 => {
                    (Aim::Doom(self.doom[d].id), self.doom[d].closest_point(self.units[i].pos))
                }
                // No target (or already finished off by another unit this tick): stop aiming.
                _ => {
                    let unit = &mut self.units[i];
                    unit.aim_at = None;
                    unit.aim = 0;
                    continue;
                }
            };
            let unit = &mut self.units[i];
            let stats = unit.stats();
            if unit.aim_at != Some(aim) {
                unit.aim_at = Some(aim);
                unit.aim = 0; // a new target: aiming starts over
            }
            // 1. Turn. Not facing it yet: no aiming.
            if !unit.turn_toward(to - unit.pos) {
                unit.aim = 0;
                continue;
            }
            // 2. Reload.
            if unit.cooldown > 0 {
                continue;
            }
            // 3. Aim.
            if unit.aim < stats.aim {
                unit.aim += 1;
                continue;
            }
            // 4. Shoot.
            unit.aim = 0;
            unit.cooldown = stats.cooldown * 1000 / keep;
            let damage = (unit.damage() * keep / 1000).max(1);
            match intent.target {
                Some(Target::Zombie(z)) => {
                    self.shots.push((unit.pos, to));
                    let zombie = &mut self.zombies[z];
                    // The same as `shot_damage` – here `self` is already borrowed through `unit` and `zombie`.
                    let damage = damage.saturating_sub(zombie.breed.stats().armor).max(1);
                    zombie.hp = zombie.hp.saturating_sub(damage);
                    if !zombie.is_alive() {
                        unit.kills += 1;
                    } else if stats.knockback > 0.0 {
                        // The spear pushes it back – a big one less: as big as a walker the whole way, a fat one about
                        // half, a giant a quarter. It slides along what's in the way, like walking.
                        let away = (zombie.pos - unit.pos).normalize_or_zero();
                        let size = Breed::Walker.stats().radius / zombie.breed.stats().radius;
                        let push = stats.knockback * size.min(1.0);
                        zombie.pos = slide(zombie.pos, away * push, |x, y| self.map.is_blocked(x, y));
                    }
                    // It comes for the shooter – a unit in a tower: the tower – and so do the ones right around it
                    // that aren't after anything yet (`zombies.rs`). They noticed even if it fell.
                    let from = unit.pos;
                    let by = unit.inside.map_or(zombies::Aim::Unit(unit.id), zombies::Aim::Building);
                    self.zombie_hit_by(z, by);
                    // And the shot makes noise where it was fired – it adds up (TAB's noise, `sound.rs`).
                    self.make_colony_sound(from, shot_noise(stats.noise));
                }
                // The beam always shows whom it's on – it heals only out of the fight (`heal`).
                Some(Target::Ally(a)) => {
                    self.beams.push((unit.pos, to));
                    self.heal(i, a);
                }
                // A house of doom: hit – a batch comes out of it (`doom.rs`) – and the shot makes noise like any other.
                Some(Target::Doom(d)) => {
                    self.shots.push((unit.pos, to));
                    let from = unit.pos;
                    self.hit_house(d, damage);
                    self.make_colony_sound(from, shot_noise(stats.noise));
                }
                None => {}
            }
        }
    }

    /// Medic `m` beams at unit `a`: it heals `heal_per_second` – but only if `a` hasn't been hit for
    /// `heal_after_seconds`. Not like StarCraft's medic, who keeps a whole army standing in the middle of a fight: here
    /// the wounded have to step back first. Up to its full hit points (a veteran's more).
    fn heal(&mut self, m: usize, a: usize) {
        let stats = self.units[m].stats();
        if self.tick.saturating_sub(self.units[a].hurt_at) < stats.heal_after as u64 {
            return;
        }
        let medic = &mut self.units[m];
        medic.heal += (stats.heal_per_second * 1000.0 / TICKS_PER_SECOND) as u32;
        let whole = medic.heal / 1000;
        medic.heal %= 1000;
        let other = &mut self.units[a];
        other.hp = (other.hp + whole).min(other.max_hp());
    }

    /// The target the unit has been aiming at, if it's still worth it: alive, in range and in sight (and for medics
    /// still hurt). Like in StarCraft a unit keeps its target – jumping to one that is a bit closer would start the
    /// aiming over, and a ranger in front of a crowd would never get an arrow off.
    pub(super) fn kept_target(&self, i: usize) -> Option<Target> {
        let unit = &self.units[i];
        match unit.aim_at? {
            Aim::Zombie(id) => self
                .find_zombie(id)
                .filter(|&z| self.in_shot(unit.pos, self.zombies[z].pos, unit.range()))
                .map(Target::Zombie),
            Aim::Ally(id) => self
                .find_unit(id)
                .filter(|&a| {
                    let other = &self.units[a];
                    other.hp < other.max_hp() && !other.is_inside() && self.in_shot(unit.pos, other.pos, unit.range())
                })
                .map(Target::Ally),
            Aim::Doom(id) => self
                .find_doom(id)
                .filter(|&d| self.doom[d].hp > 0 && self.in_shot(unit.pos, self.doom[d].closest_point(unit.pos), unit.range()))
                .map(Target::Doom),
        }
    }

    /// Whom every unit aims at this tick – for all of them together, so a crowd doesn't all pick the same zombie
    /// (overkill prevention, 2026-10-17). Without it, a blob of soldiers aimed at the one zombie in front: the first
    /// shots killed it, all the others had to turn and aim anew – at the next one in front, again all together. 160
    /// soldiers fired like 15.
    ///
    /// Two rounds, in the order of the list (the same on every PC):
    /// 1. Whoever keeps its target (`kept_target`) keeps it – what its next shot does to it is counted.
    /// 2. Whoever needs a new one takes the nearest zombie that isn't already as good as dead from the shots aimed at
    ///    it – and counts its own shot. Is every zombie in range covered: simply the nearest. Medics: the nearest hurt
    ///    unit, as before.
    ///
    /// Only for units that pick their targets themselves (standing, holding, attack-move, patrol, in a tower) – one
    /// that walks (move, follow) or has a zombie of its own to attack gets `None` here (`units.rs` decides for it).
    pub(super) fn choose_targets(&self) -> Vec<Option<Target>> {
        let picks = |i: usize| self.units[i].picks_targets();
        let mut targets: Vec<Option<Target>> =
            (0..self.units.len()).map(|i| if picks(i) { self.kept_target(i) } else { None }).collect();
        // Per zombie (by index): the damage of the shots already aimed at it.
        let mut aimed = vec![0u32; self.zombies.len()];
        for (i, target) in targets.iter().enumerate() {
            if let Some(Target::Zombie(z)) = *target {
                aimed[z] += self.shot_damage(i, z);
            }
        }
        for i in 0..self.units.len() {
            if targets[i].is_some() || !picks(i) {
                continue;
            }
            let unit = &self.units[i];
            if unit.stats().heals {
                targets[i] = self.nearest_hurt_ally(i, unit.range()).map(Target::Ally);
                continue;
            }
            // A zombie is still open if the shots aimed at it don't kill it yet.
            let open = self.nearest_zombie(unit.pos, unit.range(), |z| aimed[z] < self.zombies[z].hp);
            let pick = open.or_else(|| self.nearest_zombie(unit.pos, unit.range(), |_| true));
            if let Some(z) = pick {
                aimed[z] += self.shot_damage(i, z);
            }
            // No zombie in range: a house of a village of doom, if one is.
            targets[i] = pick.map(Target::Zombie).or_else(|| self.nearest_doom(unit.pos, unit.range()).map(Target::Doom));
        }
        targets
    }

    /// The nearest house of doom still standing whose walls are within `range` and in sight. There are only a few
    /// dozen – all of them are checked. Equally far: the lower index.
    fn nearest_doom(&self, pos: Vec2, range: f32) -> Option<usize> {
        let mut best: Option<(f32, usize)> = None;
        for (d, house) in self.doom.iter().enumerate() {
            let d_to = house.distance_to(pos);
            if house.hp == 0 || d_to > range || best.is_some_and(|(best_d, _)| best_d <= d_to) {
                continue;
            }
            if self.map.clear_sight(pos, house.closest_point(pos)) {
                best = Some((d_to, d));
            }
        }
        best.map(|(_, d)| d)
    }

    /// What one shot of unit `i` does to zombie `z`: the damage (a veteran's more, less in debt) minus the zombie's
    /// armor, at least 1.
    fn shot_damage(&self, i: usize, z: usize) -> u32 {
        self.army_damage(self.units[i].damage()).saturating_sub(self.zombies[z].breed.stats().armor).max(1)
    }

    /// The nearest living zombie within `range` that's in sight and that `accept` takes (by its index).
    pub(super) fn nearest_zombie(&self, pos: Vec2, range: f32, accept: impl Fn(usize) -> bool) -> Option<usize> {
        let r = range.ceil() as i32;
        let (cx, cy) = (pos.x.floor() as i32, pos.y.floor() as i32);
        let mut best: Option<(f32, usize)> = None;
        for dy in -r..=r {
            for dx in -r..=r {
                for z in self.grid.cell(cx + dx, cy + dy) {
                    if !self.zombies[z].is_alive() || !accept(z) {
                        continue;
                    }
                    let d = self.zombies[z].pos.distance(pos);
                    if d > range {
                        continue;
                    }
                    // Equally far: the lower index wins (DESIGN.md 12.8).
                    let better = match best {
                        None => true,
                        Some((best_d, best_z)) => d < best_d || (d == best_d && z < best_z),
                    };
                    // Only then the line of sight – it's the expensive part. Behind a tree: not a target.
                    if better && self.map.clear_sight(pos, self.zombies[z].pos) {
                        best = Some((d, z));
                    }
                }
            }
        }
        best.map(|(_, z)| z)
    }

    /// For medics: the nearest other unit within `range` that is hurt. There are only few units,
    /// so simply all of them are checked. Equally far: the lower index wins.
    pub(super) fn nearest_hurt_ally(&self, me: usize, range: f32) -> Option<usize> {
        let pos = self.units[me].pos;
        let mut best: Option<(f32, usize)> = None;
        for (i, other) in self.units.iter().enumerate() {
            let d = other.pos.distance(pos);
            if i == me || other.hp >= other.max_hp() || d > range || other.is_inside() {
                continue;
            }
            if best.is_none_or(|(best_d, _)| d < best_d) && self.map.clear_sight(pos, other.pos) {
                best = Some((d, i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// Can a unit at `from` shoot (or beam) at `to`: in range and no forest in between?
    pub(super) fn in_shot(&self, from: Vec2, to: Vec2, range: f32) -> bool {
        from.distance(to) <= range && self.map.clear_sight(from, to)
    }
}
