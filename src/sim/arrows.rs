//! Arrows in flight – the ranger's, like TAB's `Proyectile`. A bullet hits the moment it's fired; an arrow is shot at
//! where the zombie will be and hits when it gets there (`projectile_speed` in `data\units\`, TAB: 12 tiles a second).
//!
//! Like TAB, an arrow always hits the one it was shot at – if that one is still alive when it lands. It doesn't hit
//! others on the way, and stepping aside doesn't dodge it: where it flies to is only for the picture. Meanwhile it
//! counts as damage already on its way, so nobody else wastes a shot on a zombie that's as good as dead (TAB's
//! `AddFutureDamage`). The picture draws it in an arc with its shadow below (`effects.rs`) – the simulation only needs
//! the time.

use macroquad::math::Vec2;

use super::areas::Blow;
use super::combat::Aim;
use super::fx::Fx;
use super::{after_armor, zombies, World, TICKS_PER_SECOND};
use crate::attrs::ExtraDamage;

/// TAB lets the arrow leave a bit in front of the shooter, not out of its middle (tiles).
pub const MUZZLE: f32 = 0.3;
/// TAB leads a running zombie only this far (tiles) – further, and it shoots at where it stands (`ZXCommand`).
const MAX_LEAD: f32 = 2.0;
/// Slower than this (tiles per second), a zombie counts as standing – TAB's 0.1.
const STANDING: f32 = 0.1;

/// One arrow on its way.
#[derive(Clone)]
pub struct Arrow {
    /// Whom it hits when it lands: a zombie, a house of doom or one of your own (attacked on purpose), by `id`.
    target: Aim,
    /// Who shot it (`id`) – gets the kill – and whom the zombie hit comes for (the unit, or its tower).
    shooter: u32,
    by: zombies::Aim,
    /// Damage before the target's armor (a veteran's more, less in debt – as it was when it was shot).
    damage: u32,
    /// Ticks until it lands.
    left: u32,
    /// Thanatos's missile: it blows up where it lands and hits everyone within this many tiles (`areas.rs`) – 0: only
    /// its target. `edge`: the share at the rim; `to`: where it was shot at (if the target is gone by then).
    splash: f32,
    edge: f32,
    to: Vec2,
    /// The missile's blow: the damage without the extra, the extra against attributes and the debt's share (`areas.rs`).
    plain: u32,
    extra: ExtraDamage,
    keep: u32,
}

impl Arrow {
    pub(super) fn new(target: Aim, shooter: u32, by: zombies::Aim, damage: u32, flight: u32) -> Arrow {
        Arrow { target, shooter, by, damage, left: flight, splash: 0.0, edge: 1.0, to: Vec2::ZERO, plain: damage, extra: ExtraDamage::default(), keep: 1000 }
    }

    /// The same, as a missile that blows up (`splash`).
    pub(super) fn with_splash(self, splash: f32, edge: f32, to: Vec2, blow: (u32, ExtraDamage, u32)) -> Arrow {
        Arrow { splash, edge, to, plain: blow.0, extra: blow.1, keep: blow.2, ..self }
    }
}

/// Ticks an arrow at `speed` tiles per second takes from `from` to `to` – at least 1. Speed 0: no arrow, the shot hits
/// at once (0 ticks).
pub(super) fn flight_ticks(speed: f32, from: Vec2, to: Vec2) -> u32 {
    if speed <= 0.0 {
        return 0;
    }
    let way = (from.distance(to) - MUZZLE).max(0.0);
    ((way / speed * TICKS_PER_SECOND).ceil() as u32).max(1)
}

impl World {
    /// Where to shoot an arrow (`speed` tiles per second) from `from` at zombie `z`, which stands at `to`: where it will
    /// be when the arrow gets there – its pace times the flight, like TAB. Only if that's still within `range` and at
    /// most `MAX_LEAD` from where it is now; otherwise (and standing) right at it.
    pub(super) fn lead(&self, z: usize, from: Vec2, to: Vec2, speed: f32, range: f32) -> Vec2 {
        let zombie = &self.zombies[z];
        let pace = (zombie.pos - zombie.prev) * TICKS_PER_SECOND;
        if pace.length() < STANDING {
            return to;
        }
        let ahead = to + pace * (from.distance(to) / speed);
        if ahead.distance(from) <= range && ahead.distance(to) <= MAX_LEAD {
            ahead
        } else {
            to
        }
    }

    /// One tick of the arrows: those that arrive hit what they were shot at, if it's still there.
    pub(super) fn fly_arrows(&mut self) {
        let mut landed = Vec::new();
        self.arrows.retain_mut(|arrow| {
            arrow.left = arrow.left.saturating_sub(1);
            if arrow.left == 0 {
                landed.push(arrow.clone());
            }
            arrow.left > 0
        });
        for arrow in landed {
            if arrow.splash > 0.0 {
                // A missile: it blows up on its target – or where it was shot at, if that one is gone.
                let at = match arrow.target {
                    Aim::Zombie(id) => self.find_zombie(id).map_or(arrow.to, |z| self.zombies[z].pos),
                    _ => arrow.to,
                };
                let blow = Blow {
                    damage: arrow.plain,
                    edge: arrow.edge,
                    shooter: arrow.shooter,
                    by: arrow.by,
                    fire: false,
                    burn_chance: 0.0,
                    friendly: false,
                    extra: arrow.extra,
                    keep: arrow.keep,
                };
                self.hit_splash(at, arrow.splash, &blow);
                let by = self.find_unit(arrow.shooter).map(|u| self.units[u].kind);
                self.fx.push(Fx::Blast { at, radius: arrow.splash, by });
                continue;
            }
            match arrow.target {
                Aim::Zombie(id) => {
                    if let Some(z) = self.find_zombie(id) {
                        self.hit_zombie(z, arrow.damage, arrow.shooter, arrow.by);
                    }
                }
                // Medics don't shoot arrows – `Ally` never comes here.
                other => self.hit_thing(other, arrow.damage),
            }
        }
    }

    /// Adds to `aimed` (per zombie, by index) the damage of the arrows on their way to it, after its armor – so the
    /// others pick a zombie that isn't as good as dead already (`choose_targets`).
    pub(super) fn count_arrows_on_the_way(&self, aimed: &mut [u32]) {
        // The arrows' targets sorted by `id`, then each zombie looks itself up – one pass over all zombies instead of
        // one per arrow. (A `Vec`, not a `HashMap`: the same order on every PC, DESIGN.md 12.8.)
        let mut on_the_way: Vec<(u32, u32)> = self
            .arrows
            .iter()
            .filter_map(|a| match a.target {
                Aim::Zombie(id) => Some((id, a.damage)),
                _ => None,
            })
            .collect();
        if on_the_way.is_empty() {
            return;
        }
        on_the_way.sort_unstable();
        for (z, zombie) in self.zombies.iter().enumerate() {
            let first = on_the_way.partition_point(|&(id, _)| id < zombie.id);
            let armor = zombie.armor();
            for &(_, damage) in on_the_way[first..].iter().take_while(|&&(id, _)| id == zombie.id) {
                aimed[z] += after_armor(damage, armor);
            }
        }
    }

    /// How many arrows are on their way – for the checksum.
    pub(super) fn arrows_left(&self) -> impl Iterator<Item = u32> + '_ {
        self.arrows.iter().map(|a| a.left)
    }
}
