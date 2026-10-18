//! Combat of your units: whom they aim at (the nearest zombie in sight, for medics the nearest hurt unit) and
//! shooting. Zombies hitting back is part of their walking (`zombies.rs`).
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
use super::World;

/// What a unit aims at this tick: a zombie (shoot) or – medics – one of its own (beam).
/// The number is the index in the respective list.
#[derive(Clone, Copy)]
pub(super) enum Target {
    Zombie(usize),
    Ally(usize),
}

/// What a unit has been aiming at – by `id`, because it's kept from tick to tick and the indices change when the
/// dead are taken out of the lists.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Aim {
    Zombie(u32),
    Ally(u32),
}

impl World {
    /// Turn, reload, aim, shoot (see the top of the file) – or, for medics, beam – at what each unit wants to aim at
    /// this tick (`intent`).
    pub(super) fn shoot(&mut self, intents: &[Intent]) {
        for (i, intent) in intents.iter().enumerate() {
            // Reloading goes on whatever the unit does – walking, turning, nothing.
            self.units[i].cooldown = self.units[i].cooldown.saturating_sub(1);
            let (aim, to) = match intent.target {
                Some(Target::Zombie(z)) if self.zombies[z].is_alive() => {
                    (Aim::Zombie(self.zombies[z].id), self.zombies[z].pos)
                }
                Some(Target::Ally(a)) => (Aim::Ally(self.units[a].id), self.units[a].pos),
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
            unit.cooldown = stats.cooldown;
            match intent.target {
                Some(Target::Zombie(z)) => {
                    self.shots.push((unit.pos, to));
                    let zombie = &mut self.zombies[z];
                    // The same as `shot_damage` – here `self` is already borrowed through `unit` and `zombie`.
                    let damage = stats.damage.saturating_sub(zombie.breed.stats().armor).max(1);
                    zombie.hp = zombie.hp.saturating_sub(damage);
                    if !zombie.is_alive() {
                        unit.kills += 1;
                    }
                    // Whoever survives the shot comes for you – it and a few around it through the gap, if there
                    // is one (`zombies.rs`). The ones around it noticed even if it fell.
                    let from = unit.pos;
                    self.start_seeking(z);
                    // And everyone who heard the shot comes too (TAB's noise).
                    self.make_noise(from, stats.noise);
                }
                // For now only the beam – healing comes later.
                Some(Target::Ally(_)) => self.beams.push((unit.pos, to)),
                None => {}
            }
        }
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
                    other.hp < other.stats().hp && !other.is_inside() && self.in_shot(unit.pos, other.pos, unit.range())
                })
                .map(Target::Ally),
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
            targets[i] = pick.map(Target::Zombie);
        }
        targets
    }

    /// What one shot of unit `i` does to zombie `z`: the damage minus the zombie's armor, at least 1.
    fn shot_damage(&self, i: usize, z: usize) -> u32 {
        self.units[i].stats().damage.saturating_sub(self.zombies[z].breed.stats().armor).max(1)
    }

    /// The nearest living zombie within `range` that's in sight and that `accept` takes (by its index).
    pub(super) fn nearest_zombie(&self, pos: Vec2, range: f32, accept: impl Fn(usize) -> bool) -> Option<usize> {
        let r = range.ceil() as i32;
        let (cx, cy) = (pos.x.floor() as i32, pos.y.floor() as i32);
        let mut best: Option<(f32, usize)> = None;
        for dy in -r..=r {
            for dx in -r..=r {
                for &z in self.grid.cell(cx + dx, cy + dy) {
                    let z = z as usize;
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
            if i == me || other.hp >= other.stats().hp || d > range || other.is_inside() {
                continue;
            }
            if best.is_none_or(|(best_d, _)| d < best_d) && self.map.clear_sight(pos, other.pos) {
                best = Some((d, i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// Can a unit at `from` shoot (or beam) at `to`: in range and no forest or tree in between?
    pub(super) fn in_shot(&self, from: Vec2, to: Vec2, range: f32) -> bool {
        from.distance(to) <= range && self.map.clear_sight(from, to)
    }
}
