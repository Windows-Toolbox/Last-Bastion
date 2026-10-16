//! Combat of your units: whom they aim at (the nearest zombie in sight, for medics the nearest hurt unit) and
//! shooting. Zombies hitting back is part of their walking (`zombies.rs`).

use macroquad::math::Vec2;

use super::units::Intent;
use super::zombies::ALERT_TICKS;
use super::World;

/// What a unit aims at this tick: a zombie (shoot) or – medics – one of its own (beam).
/// The number is the index in the respective list.
#[derive(Clone, Copy)]
pub(super) enum Target {
    Zombie(usize),
    Ally(usize),
}

impl World {
    /// Shoot – or, for medics, beam – at what each unit aimed at this tick (`intent`). Whose weapon is still
    /// cooling down only counts down.
    pub(super) fn shoot(&mut self, intents: &[Intent]) {
        for (i, intent) in intents.iter().enumerate() {
            if self.units[i].cooldown > 0 {
                self.units[i].cooldown -= 1;
                continue;
            }
            match intent.target {
                Some(Target::Zombie(z)) => {
                    if !self.zombies[z].is_alive() {
                        continue; // already finished off by another unit this tick
                    }
                    let unit = &mut self.units[i];
                    let stats = unit.stats();
                    self.shots.push((unit.pos, self.zombies[z].pos));
                    unit.cooldown = stats.cooldown;
                    let zombie = &mut self.zombies[z];
                    zombie.hp = zombie.hp.saturating_sub(stats.damage);
                    zombie.alert = ALERT_TICKS; // whoever survives the shot comes for you
                    if !zombie.is_alive() {
                        unit.kills += 1;
                    }
                }
                Some(Target::Ally(a)) => {
                    // For now only the beam – healing comes later.
                    let to = self.units[a].pos;
                    let unit = &mut self.units[i];
                    self.beams.push((unit.pos, to));
                    unit.cooldown = unit.stats().cooldown;
                }
                None => {}
            }
        }
    }

    pub(super) fn nearest_zombie(&self, pos: Vec2, range: f32) -> Option<usize> {
        let r = range.ceil() as i32;
        let (cx, cy) = (pos.x.floor() as i32, pos.y.floor() as i32);
        let mut best: Option<(f32, usize)> = None;
        for dy in -r..=r {
            for dx in -r..=r {
                for &z in self.grid.cell(cx + dx, cy + dy) {
                    let z = z as usize;
                    if !self.zombies[z].is_alive() {
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
