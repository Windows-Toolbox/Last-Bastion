//! Attacks that hit more than their target – TAB's `AttackAreaType`:
//! - a **cone** in front of the attacker (Lucifer's flame, 20° wide and as long as its range; Thanatos's blow, 120°),
//! - a **splash** round where a shot lands (Thanatos's missile 1.8 tiles, the Titan's shell 0.8, the executor's 0.6).
//!
//! Everyone in it takes the damage – the further from the middle (the cone: from the attacker), the less, down to
//! `edge` at the rim (TAB's `MinFactorDamageInArea`; 1: everyone all of it). A zombie hit like this comes for the one
//! who hit it, like any shot (`hit_zombie`). Fire (Lucifer) hurts every zombie by its `fire_factor` and may set it
//! alight (`fire.rs`); with `friendly` your own units in it are hurt too (TAB's `FriendlyDamage`) – never the attacker.
//!
//! Only plain arithmetic, no `sin`/`cos` of the standard library: those may round differently on another PC, and the
//! simulation has to come out bit for bit the same everywhere (DESIGN.md 12.8). The cone's width is checked with a
//! cosine of our own (`cos_degrees`).

use macroquad::math::Vec2;

use super::{zombies, World};
use crate::attrs::ExtraDamage;

/// What one area attack does.
pub(super) struct Blow {
    /// Damage in the middle, before armor.
    pub damage: u32,
    /// At the rim only this share of it.
    pub edge: f32,
    /// Who did it (`id`, gets the kills) and whom the zombies hit come for.
    pub shooter: u32,
    pub by: zombies::Aim,
    /// Fire: every zombie takes its `fire_factor` of it, every unit its own – and `burn_chance` of the zombies catch fire.
    pub fire: bool,
    pub burn_chance: f32,
    /// Your units in it are hurt too.
    pub friendly: bool,
    /// The weapon's extra damage against attributes (`attrs.rs`) – worked out for every zombie in it – and the debt's
    /// share of it in thousandths (1000: all). `damage` is without the extra.
    pub extra: ExtraDamage,
    pub keep: u32,
}

impl World {
    /// A cone from `from` towards `to`, `range` tiles long, `degrees` wide.
    pub(super) fn hit_cone(&mut self, from: Vec2, to: Vec2, range: f32, degrees: f32, blow: &Blow) {
        let toward = (to - from).normalize_or_zero();
        let wide = cos_degrees(degrees / 2.0);
        let inside = |p: Vec2| {
            let way = p - from;
            let d = way.length();
            d <= range && (d < 0.05 || way.dot(toward) >= d * wide)
        };
        self.hit_area(from, range, inside, blow);
    }

    /// A splash of `radius` tiles round `at`.
    pub(super) fn hit_splash(&mut self, at: Vec2, radius: f32, blow: &Blow) {
        self.hit_area(at, radius, |p| p.distance(at) <= radius, blow);
    }

    /// Everyone within `radius` of `middle` that `inside` takes: the damage less towards the rim.
    fn hit_area(&mut self, middle: Vec2, radius: f32, inside: impl Fn(Vec2) -> bool, blow: &Blow) {
        let share = |p: Vec2| 1.0 - (1.0 - blow.edge) * (p.distance(middle) / radius.max(0.01)).min(1.0);
        // Who's in it, found first (in the order of the grid – the same on every PC), then hit: hitting changes the
        // zombies, and the grid may not be read meanwhile.
        let r = radius.ceil() as i32 + 1;
        let (cx, cy) = (middle.x.floor() as i32, middle.y.floor() as i32);
        let mut hit: Vec<usize> = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                hit.extend(self.grid.cell(cx + dx, cy + dy).filter(|&z| self.zombies[z].is_alive() && inside(self.zombies[z].pos)));
            }
        }
        hit.sort_unstable();
        for z in hit {
            let factor = if blow.fire { self.zombies[z].breed.stats().fire_factor } else { 1.0 };
            let full = blow.damage + blow.extra.against(self.zombies[z].attributes()) * blow.keep / 1000;
            let damage = (full as f32 * share(self.zombies[z].pos) * factor).round() as u32;
            if damage > 0 {
                self.hit_zombie(z, damage, blow.shooter, blow.by);
            }
            if blow.fire && self.zombies[z].is_alive() && self.rng.unit_f32() < blow.burn_chance {
                self.set_alight(z);
            }
        }
        if !blow.friendly {
            return;
        }
        let tick = self.tick;
        for unit in self.units.iter_mut().filter(|u| u.id != blow.shooter && u.hp > 0 && !u.is_inside() && inside(u.pos)) {
            let factor = if blow.fire { unit.stats().fire_factor } else { 1.0 };
            let full = blow.damage + blow.extra.against(unit.attributes()) * blow.keep / 1000;
            let damage = (full as f32 * share(unit.pos) * factor).round() as u32;
            if damage > 0 {
                unit.take_hit(damage, tick);
            }
        }
    }
}

/// The cosine of an angle in degrees (0 to 180), with nothing but multiplying and adding – the same on every PC.
/// Taylor's series to the 14th power: off by less than a millionth up to 180°.
pub(super) fn cos_degrees(degrees: f32) -> f32 {
    let x = degrees.clamp(0.0, 180.0) * std::f32::consts::PI / 180.0;
    let x2 = x * x;
    let (mut term, mut sum) = (1.0f32, 1.0f32);
    for k in 1..=7 {
        term *= -x2 / ((2 * k - 1) * (2 * k)) as f32;
        sum += term;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::cos_degrees;

    #[test]
    fn our_cosine_is_close_to_the_real_one() {
        for d in [0.0f32, 10.0, 45.0, 60.0, 90.0, 120.0, 180.0] {
            assert!((cos_degrees(d) - d.to_radians().cos()).abs() < 1e-4, "{d}");
        }
    }
}
