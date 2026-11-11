//! Combat of your units: whom they aim at (the nearest zombie in sight, with none in range a house of a village of
//! doom, for medics the nearest hurt unit) and shooting. Zombies hitting back is part of their walking (`zombies.rs`).
//!
//! A shot goes like in TAB (`ZXBehaviour.ExecuteCurrentAction`), with the times from `data\units.json`:
//! 1. **raise** the weapon (`raise_seconds`, TAB's `LoadAttack`) – only if it's down; it stays up while the unit keeps
//!    shooting. Meanwhile it **turns** to face the target (`turn_around_seconds` for 180°).
//! 2. wait until the weapon is **reloaded** (`cooldown_seconds`, counted from the last shot),
//! 3. **aim** – hold still on the target (`aim_seconds`, only the sniper),
//! 4. **wind up** – the shot's animation starts (`windup_seconds`), and at its end:
//! 5. shoot – and the reloading starts.
//!
//! **Animation cancelling** (like StarCraft): walking off during the wind-up cancels the shot – no damage, no reloading.
//! Walking off after the shot costs nothing: the reloading goes on while it walks. The weapon goes down while it
//! walks (`lower_seconds`), so it has to come up again when it stops – the price of a step aside.
//!
//! Another target, or none, and the aiming starts over. That's why a unit keeps its target (`kept_target`) instead of
//! jumping to whichever zombie is a bit closer right now. A new one it looks for only every `search_seconds`, like TAB –
//! except on attack-move and patrol: without a target it walks on, so it looks every tick (`hunts`).
//!
//! A new command starts the shot over too (`restart_shot`) – also the same attack again, like StarCraft.
//!
//! Your own units and buildings are only shot on purpose – an attack on one (T + click, `Prey`). Nobody picks them by
//! themselves.

use macroquad::math::Vec2;

use super::arrows::{self, Arrow};
use super::commands::{Prey, Task};
use super::areas::Blow;
use super::fx::Fx;
use super::units::{Intent, Unit, WEAPON_UP};
use crate::data;
use super::zombies::{self, Breed};
use super::{after_armor, slide, World, TICKS_PER_SECOND};

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
    /// One of your own units, attacked on purpose (shot, not beamed).
    Friend(usize),
    /// One of your own buildings, attacked on purpose.
    Building(usize),
}

/// What a unit has been aiming at – by `id`, because it's kept from tick to tick and the indices change when the
/// dead are taken out of the lists.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Aim {
    Zombie(u32),
    Ally(u32),
    Doom(u32),
    Friend(u32),
    Building(u32),
}

impl World {
    /// Raise, turn, reload, aim, wind up, shoot (see the top of the file) – or, for medics, beam – at what each unit
    /// wants to aim at this tick (`intent`).
    pub(super) fn shoot(&mut self, intents: &[Intent]) {
        // In debt the units shoot slower and weaker (`debt.rs`) – read before `unit` borrows `self`.
        let keep = self.army_keep;
        let search = data::get().units.search;
        let tick = self.tick;
        for (i, intent) in intents.iter().enumerate() {
            // Reloading goes on whatever the unit does – walking, turning, nothing. That's why walking off right after
            // the shot costs no time.
            self.units[i].cooldown = self.units[i].cooldown.saturating_sub(1);
            let (aim, to) = match intent.target {
                Some(Target::Zombie(z)) if self.zombies[z].is_alive() => {
                    (Aim::Zombie(self.zombies[z].id), self.zombies[z].pos)
                }
                Some(Target::Ally(a)) => (Aim::Ally(self.units[a].id), self.units[a].pos),
                Some(Target::Doom(d)) if self.doom[d].hp > 0 => {
                    (Aim::Doom(self.doom[d].id), self.doom[d].closest_point(self.units[i].pos))
                }
                Some(Target::Friend(u)) if self.units[u].hp > 0 => (Aim::Friend(self.units[u].id), self.units[u].pos),
                Some(Target::Building(b)) if self.buildings[b].hp > 0 => {
                    (Aim::Building(self.buildings[b].id), self.buildings[b].closest_point(self.units[i].pos))
                }
                // No target (or already finished off by another unit this tick): stop aiming.
                _ => {
                    let unit = &mut self.units[i];
                    unit.since_target = unit.since_target.saturating_add(1);
                    unit.cancel_windup();
                    unit.aim_at = None;
                    unit.aim = 0;
                    // Walking, or nothing found for a whole search: the weapon goes down. Between two searches (a kill,
                    // the next one not looked for yet) it stays up, like TAB's.
                    if intent.walking || unit.since_target > search {
                        unit.lower_weapon();
                    }
                    if intent.walking {
                        unit.swing_from = None; // the rest of the shot's animation is cancelled too
                    }
                    continue;
                }
            };
            let unit = &mut self.units[i];
            let stats = unit.stats();
            unit.since_target = 0;
            if unit.aim_at != Some(aim) {
                unit.aim_at = Some(aim);
                unit.cancel_windup();
                unit.aim = 0; // a new target: aiming starts over
            }
            // Thanatos close up: its blow instead of its missile – with the blow's times (`Melee`).
            let close = stats.melee.as_ref().filter(|m| unit.pos.distance(to) <= m.range);
            let (aim_time, windup, cooldown) = match close {
                Some(m) => (0, m.windup, m.cooldown),
                None => (stats.aim, stats.windup, stats.cooldown),
            };
            // 1. Raise the weapon – while it turns, too.
            unit.raise_weapon();
            // 2. Turn. Not facing it yet: no aiming.
            if !unit.turn_toward(to - unit.pos) {
                unit.cancel_windup();
                unit.aim = 0;
                continue;
            }
            // 3. Weapon up and reloaded?
            if unit.weapon < WEAPON_UP || unit.cooldown > 0 {
                continue;
            }
            // 4. Aim, then wind up – the shot's animation starts with the wind-up (the picture plays it from here).
            if unit.aim == aim_time {
                unit.swing_from = Some(tick);
                unit.fighting = close.is_some();
            }
            if unit.aim < aim_time + windup {
                unit.aim += 1;
                continue;
            }
            // 5. Shoot.
            unit.aim = 0;
            unit.cooldown = cooldown * 1000 / keep;
            let damage = (unit.damage() * keep / 1000).max(1);
            match intent.target {
                // Thanatos's blow: everyone in a wide cone in front of it (`areas.rs`), as hard as its missile is
                // made stronger (veteran) or weaker (debt).
                Some(Target::Zombie(_)) if close.is_some() => {
                    let m = close.expect("checked");
                    let (from, id, kind) = (unit.pos, unit.id, unit.kind);
                    let by = unit.inside.map_or(zombies::Aim::Unit(id), zombies::Aim::Building);
                    let damage = (m.damage * damage / stats.damage.max(1)).max(1);
                    let blow = Blow { damage, edge: m.edge_factor, shooter: id, by, fire: false, burn_chance: 0.0, friendly: false };
                    self.hit_cone(from, to, m.range, m.cone_degrees, &blow);
                    self.fx.push(Fx::UnitShot { from, to, kind, bleeds: true, flight: 0 });
                    self.make_colony_sound(from, shot_noise(stats.noise.min(3.0)));
                }
                // Lucifer's flame: everyone in its cone burns (`areas.rs`, `fire.rs`) – your own units too.
                Some(Target::Zombie(_)) if stats.cone_degrees > 0.0 => {
                    let (from, id, range) = (unit.pos, unit.id, unit.range());
                    let by = unit.inside.map_or(zombies::Aim::Unit(id), zombies::Aim::Building);
                    let blow = Blow {
                        damage,
                        edge: stats.edge_factor,
                        shooter: id,
                        by,
                        fire: stats.burn_chance > 0.0,
                        burn_chance: stats.burn_chance,
                        friendly: stats.friendly_fire,
                    };
                    self.hit_cone(from, to, range, stats.cone_degrees, &blow);
                    self.fx.push(Fx::Flame { from, to: from + (to - from).normalize_or_zero() * range });
                    self.make_colony_sound(from, shot_noise(stats.noise));
                }
                Some(Target::Zombie(z)) => {
                    // Copied out of `unit`: from here on `self` is needed whole, for the methods below.
                    let (from, id, kind, range) = (unit.pos, unit.id, unit.kind, unit.range());
                    // The zombie hit comes for the shooter – a unit in a tower: the tower (`zombies.rs`).
                    let by = unit.inside.map_or(zombies::Aim::Unit(id), zombies::Aim::Building);
                    // An arrow is shot at where the zombie will be and hits when it gets there (`arrows.rs`); a bullet
                    // or a spear hits at once.
                    let speed = stats.projectile_speed;
                    let to = if speed > 0.0 { self.lead(z, from, to, speed, range) } else { to };
                    let flight = arrows::flight_ticks(speed, from, to);
                    self.shots.push((from, to));
                    self.fx.push(Fx::UnitShot { from, to, kind, bleeds: true, flight });
                    // A shell or missile with a splash (`areas.rs`): everyone round where it lands.
                    let splash = (stats.splash > 0.0).then(|| Blow {
                        damage,
                        edge: stats.edge_factor,
                        shooter: id,
                        by,
                        fire: false,
                        burn_chance: 0.0,
                        friendly: stats.friendly_fire,
                    });
                    if flight > 0 {
                        let arrow = Arrow::new(Aim::Zombie(self.zombies[z].id), id, by, damage, flight);
                        self.arrows.push(if splash.is_some() { arrow.with_splash(stats.splash, stats.edge_factor, to) } else { arrow });
                    } else if let Some(blow) = splash {
                        self.hit_splash(to, stats.splash, &blow);
                        self.fx.push(Fx::Blast { at: to, radius: stats.splash });
                    } else {
                        self.hit_zombie(z, damage, id, by);
                        if stats.knockback > 0.0 && self.zombies[z].is_alive() {
                            self.push_back(z, from, stats.knockback);
                        }
                    }
                    // And the shot makes noise where it was fired – it adds up (TAB's noise, `sound.rs`).
                    self.make_colony_sound(from, shot_noise(stats.noise));
                }
                // The beam always shows whom it's on – it heals only out of the fight (`heal`).
                Some(Target::Ally(a)) => {
                    self.beams.push((unit.pos, to));
                    self.heal(i, a);
                }
                // A house of doom, or one of your own: hit (`hit_thing`) – and the shot makes noise like any other.
                Some(Target::Doom(_) | Target::Friend(_) | Target::Building(_)) => {
                    let (from, id, kind) = (unit.pos, unit.id, unit.kind);
                    let by = unit.inside.map_or(zombies::Aim::Unit(id), zombies::Aim::Building);
                    let flight = arrows::flight_ticks(stats.projectile_speed, from, to);
                    self.shots.push((from, to));
                    let bleeds = matches!(aim, Aim::Friend(_));
                    self.fx.push(Fx::UnitShot { from, to, kind, bleeds, flight });
                    if flight > 0 {
                        self.arrows.push(Arrow::new(aim, id, by, damage, flight));
                    } else {
                        self.hit_thing(aim, damage);
                    }
                    self.make_colony_sound(from, shot_noise(stats.noise));
                }
                None => {}
            }
        }
    }

    /// A shot of `damage` (before its armor) lands on zombie `z` – a bullet at once, an arrow when it gets there
    /// (`arrows.rs`): it's hurt, the shooter (`id`, if it still lives) gets the kill, and it comes for the shooter (`by`) –
    /// so do the ones right around it that aren't after anything yet (`zombies.rs`). They noticed even if it fell.
    pub(super) fn hit_zombie(&mut self, z: usize, damage: u32, shooter: u32, by: zombies::Aim) {
        let zombie = &mut self.zombies[z];
        zombie.hurt(after_armor(damage, zombie.breed.stats().armor), self.tick);
        if !zombie.is_alive() {
            if let Some(s) = self.find_unit(shooter) {
                self.units[s].kills += 1;
            }
        }
        self.zombie_hit_by(z, by);
    }

    /// A shot at something other than a zombie lands – a bullet at once, an arrow when it gets there (`arrows.rs`):
    /// a house of doom (a batch comes out of it, `doom.rs`), or one of your own. Your building: shield, then hit points
    /// – it never infects it, but without its shield the next zombie hit does (`Building::take_friendly_fire`). What is
    /// gone by then (dead, fallen, a unit gone into a tower) isn't hit.
    pub(super) fn hit_thing(&mut self, aim: Aim, damage: u32) {
        match aim {
            Aim::Doom(id) => {
                if let Some(d) = self.find_doom(id).filter(|&d| self.doom[d].hp > 0) {
                    self.hit_house(d, damage);
                }
            }
            Aim::Friend(id) => {
                if let Some(u) = self.find_unit(id).filter(|&u| !self.units[u].is_inside()) {
                    let tick = self.tick;
                    self.units[u].take_hit(damage, tick);
                }
            }
            Aim::Building(id) => {
                if let Some(b) = self.find_building(id).filter(|&b| self.buildings[b].hp > 0) {
                    self.buildings[b].take_friendly_fire(damage);
                }
            }
            Aim::Zombie(_) | Aim::Ally(_) => {}
        }
    }

    /// Where the prey of an attack is, seen from `from`: what to aim at, and the spot a shot lands – on a building the
    /// edge closest to `from`. `None` if it's gone (dead, fallen, or a unit that went into a tower).
    pub(super) fn prey_spot(&self, prey: Prey, from: Vec2) -> Option<(Target, Vec2)> {
        match prey {
            Prey::Zombie(id) => self.find_zombie(id).map(|z| (Target::Zombie(z), self.zombies[z].pos)),
            Prey::Unit(id) => self
                .find_unit(id)
                .filter(|&u| self.units[u].hp > 0 && !self.units[u].is_inside())
                .map(|u| (Target::Friend(u), self.units[u].pos)),
            Prey::Building(id) => self
                .find_building(id)
                .filter(|&b| self.buildings[b].hp > 0)
                .map(|b| (Target::Building(b), self.buildings[b].closest_point(from))),
        }
    }

    /// The spear pushes zombie `z` back, away from `from` – a big one less: as big as a walker the whole way, a fat one
    /// about half, a giant a quarter. It slides along what's in the way, like walking.
    fn push_back(&mut self, z: usize, from: Vec2, knockback: f32) {
        let zombie = &mut self.zombies[z];
        let away = (zombie.pos - from).normalize_or_zero();
        let size = Breed::Walker.stats().radius / zombie.breed.stats().radius;
        let push = knockback * size.min(1.0);
        zombie.pos = slide(zombie.pos, away * push, |x, y| self.map.is_blocked(x, y));
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
                .filter(|&z| self.in_shot(unit.pos, self.zombies[z].pos, unit.range()) && unit.reaches(self.zombies[z].pos))
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
            // Your own only as long as the attack on it lasts – after it, nobody keeps shooting them.
            Aim::Friend(_) | Aim::Building(_) => None,
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
        // Arrows on their way count too (`arrows.rs`, TAB's `AddFutureDamage`).
        self.count_arrows_on_the_way(&mut aimed);
        for i in 0..self.units.len() {
            // A new target only when it's this unit's turn to look around (`searches`) – or it would walk on (`hunts`).
            if targets[i].is_some() || !picks(i) || !(self.searches(i) || self.units[i].hunts()) {
                continue;
            }
            let unit = &self.units[i];
            if unit.stats().heals {
                targets[i] = self.nearest_hurt_ally(i, unit.range()).map(Target::Ally);
                continue;
            }
            // A zombie is still open if the shots aimed at it don't kill it yet.
            // Thanatos: nothing closer than its missile's `min_range` – unless its blow reaches it (`reaches`).
            let reach = |z: usize| unit.reaches(self.zombies[z].pos);
            let open = self.nearest_zombie(unit.pos, unit.range(), |z| aimed[z] < self.zombies[z].hp && reach(z));
            let pick = open.or_else(|| self.nearest_zombie(unit.pos, unit.range(), reach));
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

    /// What one shot of unit `i` does to zombie `z`: the damage (a veteran's more, less in debt) after the zombie's
    /// armor, at least 1.
    fn shot_damage(&self, i: usize, z: usize) -> u32 {
        after_armor(self.army_damage(self.units[i].damage()), self.zombies[z].breed.stats().armor)
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

    /// Does unit `i` look around for a new target this tick? Every `search_seconds` – each on its own tick (by its
    /// `id`), so a crowd doesn't all look at once. TAB does the same with its `IsIAFrameModule`.
    fn searches(&self, i: usize) -> bool {
        let every = data::get().units.search.max(1) as u64;
        (self.tick + self.units[i].id as u64).is_multiple_of(every)
    }
}

/// The weapon and the shot's animation – only `shoot` above changes them (and a new command, `restart_shot`).
impl Unit {
    /// On attack-move or patrol – without a target it walks on. So it looks for one every tick: waiting for its turn
    /// (`searches`), it walked a few steps past a zombie in range first, and after every kill a step forward – and down
    /// went the weapon.
    fn hunts(&self) -> bool {
        !self.is_inside() && matches!(self.queue.front(), Some(Task::AttackMove(_) | Task::Patrol { .. }))
    }

    /// A new command (or stop): the shot it's winding up is broken off and the aiming starts over – also when the new
    /// command is the same attack again (StarCraft's animation cancelling). The weapon stays up, the reloading goes on.
    pub(super) fn restart_shot(&mut self) {
        self.cancel_windup();
        self.aim = 0;
    }

    /// One tick of raising the weapon: all the way up in `raise` ticks (0: at once).
    fn raise_weapon(&mut self) {
        let step = WEAPON_UP.div_ceil(self.stats().raise.max(1));
        self.weapon = (self.weapon + step).min(WEAPON_UP);
    }

    /// One tick of taking it down: all the way in `lower` ticks. Walked only a bit, it's only partly down – coming up
    /// again takes the same share of `raise`.
    fn lower_weapon(&mut self) {
        let step = WEAPON_UP.div_ceil(self.stats().lower.max(1));
        self.weapon = self.weapon.saturating_sub(step);
    }

    /// The wind-up is broken off (walked off, turned away, another target): the shot doesn't happen, and its animation
    /// isn't played on. Already shot: nothing to cancel – the rest of the animation goes on.
    fn cancel_windup(&mut self) {
        if self.aim > self.stats().aim {
            self.swing_from = None;
        }
    }
}
