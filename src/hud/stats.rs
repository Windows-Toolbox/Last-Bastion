//! The two icons StarCraft 2 shows for a selected thing – its **weapon** and its **armor** (a building with a shield
//! has a **shield armor** too) – as boxes in the selection panel, and the tooltip over each: damage with the upgrades'
//! share, extra damage against attributes, attack speed and damage per second, range – or armor, what it is, hit points,
//! speed and which upgrades count. Only text is made here (testable, no drawing); `hud.rs` draws the boxes and shows the
//! tooltip for the one under the mouse.

use std::collections::BTreeMap;

use crate::attrs::{self, Extra};
use crate::data;
use crate::kinds::{UpgradeBonus, Weapon};
use crate::sim::{Building, Unit, World, Zombie, TICKS_PER_SECOND};

/// One box of the panel: a label above, the number below – `up` in green behind it (what the upgrades give) – and the
/// tooltip lines (the first is the title). No lines: no tooltip.
pub struct Stat<'a> {
    pub label: &'a str,
    pub value: String,
    pub up: u32,
    pub tip: Vec<String>,
}

impl<'a> Stat<'a> {
    /// A plain box without a tooltip.
    pub fn plain(label: &'a str, value: impl Into<String>) -> Stat<'a> {
        Stat { label, value: value.into(), up: 0, tip: Vec::new() }
    }
}

/// "1.5" for 1.5, "2" for 2 – no float noise.
fn tidy(value: f32) -> String {
    format!("{}", (value * 100.0).round() / 100.0)
}

/// Shots (or blows) per second of a weapon that needs `ticks` for one.
fn per_second(ticks: u32) -> f32 {
    TICKS_PER_SECOND / ticks.max(1) as f32
}

/// The levels of the tracks a thing lists, as lines: "Infantry weapons 2/3: +4 damage". `show` says what `level` of the
/// track gives *for this box* (empty: nothing, no line). A track at level 0 says what each level would give.
fn upgrade_lines(world: &World, listed: &BTreeMap<String, UpgradeBonus>, show: impl Fn(&UpgradeBonus, u32) -> Vec<String>) -> Vec<String> {
    let mut lines = Vec::new();
    for (id, per_level) in listed {
        let Some(track) = data::get().upgrades.tracks.iter().find(|t| &t.id == id) else {
            continue;
        };
        let level = world.upgrade_level(id).min(track.levels);
        let gains = show(per_level, level);
        if !gains.is_empty() {
            lines.push(format!("{} {}/{}: {}", track.name, level, track.levels, gains.join(", ")));
        } else if level == 0 {
            // Not researched yet: say what one level would give.
            let next = show(per_level, 1);
            if !next.is_empty() {
                lines.push(format!("{} 0/{} (each level: {})", track.name, track.levels, next.join(", ")));
            }
        }
    }
    lines
}

/// "+4 damage" – only if there is something.
fn plus(amount: u32, what: &str) -> Option<String> {
    (amount > 0).then(|| format!("+{amount} {what}"))
}

/// What the levels of a track give a *weapon*, at `level`.
fn weapon_gains(b: &UpgradeBonus, level: u32) -> Vec<String> {
    [plus(b.damage * level, "damage"), plus(b.bonus_damage * level, "extra damage"), plus(b.attack_speed * level, "% attack speed"), (b.range > 0.0).then(|| format!("+{} range", tidy(b.range * level as f32)))]
        .into_iter()
        .flatten()
        .collect()
}

/// What the levels of a track give an *armor*, at `level`.
fn armor_gains(b: &UpgradeBonus, level: u32, shield: bool) -> Vec<String> {
    let points = if shield { b.shield_armor } else { b.armor };
    let mut gains = vec![plus(points * level, if shield { "shield armor" } else { "armor" })];
    if !shield {
        gains.extend([plus(b.hp * level, "hit points"), (b.speed > 0.0).then(|| format!("+{} speed", tidy(b.speed * level as f32)))]);
    } else {
        gains.push(plus(b.shield * level, "shield"));
    }
    gains.into_iter().flatten().collect()
}

/// The extra damage lines: "+10 vs Armored" – the upgrades' share already in it.
fn extra_lines(extra: &Extra, per_level: u32) -> Vec<String> {
    extra.lines(per_level).into_iter().map(|(a, n)| format!("+{n} vs {}", a.name())).collect()
}

/// The damage per second against a target with the extra damage of `attribute` as well as without.
fn dps_lines(damage: u32, extra: &Extra, per_level: u32, shots_per_second: f32) -> Vec<String> {
    let mut lines = vec![format!("Damage per second: {:.1}", damage as f32 * shots_per_second)];
    for (attribute, n) in extra.lines(per_level) {
        lines.push(format!("  vs {}: {:.1}", attribute.name(), (damage + n) as f32 * shots_per_second));
    }
    lines
}

/// The panel's weapon box of a unit: `None` for one that doesn't shoot (the medic).
pub fn unit_weapon<'a>(world: &World, unit: &Unit) -> Option<Stat<'a>> {
    let stats = unit.stats();
    if stats.heals {
        return None;
    }
    let total = world.army_damage(unit.damage());
    let up = unit.up.damage.min(total);
    let ticks = unit.quicker(stats.shot_ticks());
    let shots = per_second(ticks);
    let range = unit.base_range();
    let mut tip = vec![format!("Weapon - {}", stats.name)];
    tip.push(if up > 0 { format!("Damage: {total} ({} + {up} from upgrades)", total - up) } else { format!("Damage: {total}") });
    let extra = &stats.bonus;
    tip.extend(extra_lines(extra, unit.up.bonus_damage));
    tip.push(format!("Attack speed: {shots:.2} shots per second ({:.2} s each)", seconds(ticks)));
    tip.extend(dps_lines(total, extra, unit.up.bonus_damage, shots));
    tip.push(format!("Range: {}", tidy(range)));
    if stats.splash > 0.0 {
        tip.push(format!("Hits an area of {} tiles round the target", tidy(stats.splash)));
    }
    if stats.cone_degrees > 0.0 {
        tip.push(format!("Hits everyone in a cone of {} degrees", tidy(stats.cone_degrees)));
    }
    if stats.knockback > 0.0 {
        tip.push(format!("Pushes the target back {} tiles", tidy(stats.knockback)));
    }
    tip.extend(upgrade_lines(world, &stats.upgrades, |b, level| weapon_gains(b, level)));
    Some(Stat { label: "Weapon", value: (total - up).to_string(), up, tip })
}

/// The panel's armor box of a unit.
pub fn unit_armor<'a>(world: &World, unit: &Unit) -> Stat<'a> {
    let stats = unit.stats();
    let (armor, up) = (unit.armor(), unit.up.armor);
    let mut tip = vec![format!("Armor - {}", stats.name)];
    tip.push(if up > 0 { format!("Armor: {armor} ({} + {up} from upgrades)", armor - up) } else { format!("Armor: {armor}") });
    tip.push(format!("Every hit loses {armor} damage (at least 1 gets through)"));
    tip.push(format!("Is: {}", attrs::names(unit.attributes())));
    tip.push(format!("Hit points: {}/{}", unit.hp, unit.max_hp()));
    tip.push(format!("Move speed: {}", tidy(unit.speed())));
    tip.push(format!("Takes {} % of venom, {} % of fire", (stats.venom_factor * 100.0).round(), (stats.fire_factor * 100.0).round()));
    tip.extend(upgrade_lines(world, &stats.upgrades, |b, level| armor_gains(b, level, false)));
    Stat { label: "Armor", value: (armor - up).to_string(), up, tip }
}

/// The weapon box of a zombie.
pub fn zombie_weapon<'a>(world: &World, zombie: &Zombie) -> Stat<'a> {
    let stats = zombie.breed.stats();
    let total = zombie.damage();
    let up = zombie.up.damage.min(total);
    let ticks = zombie.quicker(zombie.breed.attack_ticks());
    let shots = per_second(ticks);
    let mut tip = vec![format!("Weapon - {}", zombie.breed.name())];
    tip.push(if up > 0 { format!("Damage: {total} ({} + {up} from upgrades)", total - up) } else { format!("Damage: {total}") });
    tip.extend(extra_lines(&stats.bonus, zombie.up.bonus_damage));
    tip.push(format!("Attack speed: {shots:.2} hits per second ({:.2} s each)", seconds(ticks)));
    tip.extend(dps_lines(total, &stats.bonus, zombie.up.bonus_damage, shots));
    tip.push(if zombie.breed.is_ranged() { format!("Range: {}", tidy(stats.range)) } else { "Range: melee".to_owned() });
    if stats.splash > 0.0 {
        tip.push(format!("Hits an area of {} tiles", tidy(stats.splash)));
    }
    if stats.sweep_radius > 0.0 {
        tip.push(format!("Sweeps {} tiles in an arc of {} degrees", tidy(stats.sweep_radius), tidy(stats.sweep_degrees)));
    }
    if stats.venom {
        tip.push("Venom: armor doesn't stop it".to_owned());
    }
    tip.extend(upgrade_lines(world, &stats.upgrades, |b, level| weapon_gains(b, level)));
    Stat { label: "Weapon", value: (total - up).to_string(), up, tip }
}

/// The armor box of a zombie.
pub fn zombie_armor<'a>(world: &World, zombie: &Zombie) -> Stat<'a> {
    let stats = zombie.breed.stats();
    let (armor, up) = (zombie.armor(), zombie.up.armor);
    let mut tip = vec![format!("Armor - {}", zombie.breed.name())];
    tip.push(if up > 0 { format!("Armor: {armor} ({} + {up} from upgrades)", armor - up) } else { format!("Armor: {armor}") });
    tip.push(format!("Every shot loses {armor} damage (at least 1 gets through)"));
    tip.push(format!("Is: {}", attrs::names(&stats.attributes)));
    tip.push(format!("Hit points: {}/{}", zombie.hp(), zombie.max_hp()));
    tip.push(format!("Move speed: {}", tidy(zombie.speed())));
    tip.push(format!("Fire hurts it {} %", (stats.fire_factor * 100.0).round()));
    tip.extend(upgrade_lines(world, &stats.upgrades, |b, level| armor_gains(b, level, false)));
    Stat { label: "Armor", value: (armor - up).to_string(), up, tip }
}

/// The weapon box of a building that shoots by itself (a tower); `None` for the others.
pub fn building_weapon<'a>(world: &World, building: &Building) -> Option<Stat<'a>> {
    let stats = building.stats();
    let weapon: &Weapon = stats.weapon.as_ref()?;
    let damage = crate::sim::Tweak::scale(weapon.damage + building.up.damage, building.tweak.damage);
    let up = building.up.damage.min(damage);
    let ticks = building.tweak.quicker_with(weapon.reload, building.up.attack);
    let shots = per_second(ticks);
    let range = (weapon.range + building.up.range) * building.tweak.range as f32 / 100.0;
    let mut tip = vec![format!("Weapon - {}", stats.name)];
    tip.push(if up > 0 { format!("Damage: {damage} ({} + {up} from upgrades)", damage - up) } else { format!("Damage: {damage}") });
    tip.extend(extra_lines(&weapon.bonus, building.up.bonus_damage));
    tip.push(format!("Attack speed: {shots:.2} shots per second ({:.2} s each)", seconds(ticks)));
    tip.extend(dps_lines(damage, &weapon.bonus, building.up.bonus_damage, shots));
    tip.push(format!("Range: {}", tidy(range)));
    if weapon.all_in_range {
        tip.push("Hits every zombie in range at once".to_owned());
    }
    if weapon.splash > 0.0 {
        tip.push(format!("Hits an area of {} tiles round the target", tidy(weapon.splash)));
    }
    tip.extend(upgrade_lines(world, &stats.upgrades, |b, level| weapon_gains(b, level)));
    Some(Stat { label: "Weapon", value: (damage - up).to_string(), up, tip })
}

/// The armor box of a building: the hit points' armor.
pub fn building_armor<'a>(world: &World, building: &Building) -> Stat<'a> {
    let stats = building.stats();
    let (armor, up) = (building.armor(), building.up.armor);
    let mut tip = vec![format!("Armor - {}", stats.name)];
    tip.push(if up > 0 { format!("Armor: {armor} ({} + {up} from upgrades)", armor - up) } else { format!("Armor: {armor}") });
    tip.push(format!("Every hit on the hit points loses {armor} damage (at least 1 gets through)"));
    tip.push(format!("Is: {}", attrs::names(building.attributes())));
    tip.push(format!("Hit points: {}/{}", building.hp, building.max_hp()));
    tip.extend(upgrade_lines(world, &stats.upgrades, |b, level| armor_gains(b, level, false)));
    Stat { label: "Armor", value: (armor - up).to_string(), up, tip }
}

/// The shield armor box of a building with a shield: `None` for one without.
pub fn building_shield_armor<'a>(world: &World, building: &Building) -> Option<Stat<'a>> {
    if building.max_shield() == 0 {
        return None;
    }
    let stats = building.stats();
    let (armor, up) = (building.shield_armor(), building.up.shield_armor);
    let mut tip = vec![format!("Shield armor - {}", stats.name)];
    tip.push(if up > 0 { format!("Shield armor: {armor} ({} + {up} from upgrades)", armor - up) } else { format!("Shield armor: {armor}") });
    tip.push(format!("Every hit on the shield loses {armor} damage (at least 1 gets through)"));
    tip.push(format!("Shield: {}/{} - zombies hit it first, broken it lets them infect the building", building.shield, building.max_shield()));
    tip.extend(upgrade_lines(world, &stats.upgrades, |b, level| armor_gains(b, level, true)));
    Some(Stat { label: "Shield armor", value: (armor - up).to_string(), up, tip })
}

fn seconds(ticks: u32) -> f32 {
    ticks as f32 / TICKS_PER_SECOND
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::{BuildingKind, UnitKind};
    use crate::sim::{Breed, Order};
    use macroquad::math::vec2;

    fn has(tip: &[String], part: &str) -> bool {
        tip.iter().any(|line| line.contains(part))
    }

    /// The sniper's weapon tooltip says its extra damage (none shipped – the map gives it one), its speed and what the
    /// upgrades add; its armor tooltip what it is.
    #[test]
    fn a_unit_shows_weapon_and_armor_like_sc2() {
        let mut world = World::new(42);
        world.units.clear();
        let at = world.base + vec2(0.0, 6.0);
        world.step(&[
            Order::SetData { key: "units.sniper.bonus.armored".into(), value: Some(serde_json::json!(25)) },
            Order::SetData { key: "units.sniper.upgrades.infantry_weapons.bonus_damage".into(), value: Some(serde_json::json!(3)) },
        ]);
        world.step(&[Order::SpawnUnit { kind: UnitKind::Sniper, point: at, facing: None }]);
        world.step(&[Order::SetUpgrade { track: "infantry_weapons".into(), level: 2 }, Order::SetUpgrade { track: "infantry_armor".into(), level: 1 }]);
        let unit = &world.units[0];
        let weapon = unit_weapon(&world, unit).expect("a sniper shoots");
        let base = UnitKind::Sniper.stats().damage;
        assert_eq!((weapon.value.as_str(), weapon.up), (base.to_string().as_str(), 20), "the number, and the upgrades' 2 x 10 in green");
        assert!(has(&weapon.tip, "+31 vs Armored"), "25 and 3 a level: {:?}", weapon.tip);
        assert!(has(&weapon.tip, "Infantry weapons 2/3: +20 damage, +6 extra damage"), "{:?}", weapon.tip);
        assert!(has(&weapon.tip, "Damage per second") && has(&weapon.tip, "Range: 8"), "{:?}", weapon.tip);
        let armor = unit_armor(&world, unit);
        assert_eq!((armor.value.as_str(), armor.up), ("0", 1));
        assert!(has(&armor.tip, "Is: Light, Biological") && has(&armor.tip, "Infantry armor 1/3: +1 armor"), "{:?}", armor.tip);
        // A medic has no weapon.
        world.step(&[Order::SpawnUnit { kind: UnitKind::Medic, point: at + vec2(1.0, 0.0), facing: None }]);
        let medic = world.units.iter().find(|u| u.kind == UnitKind::Medic).expect("the medic is there");
        assert!(unit_weapon(&world, medic).is_none());
    }

    /// A tower has a weapon, every building an armor, and a shield armor where it has a shield; a house without a gun none.
    #[test]
    fn a_building_shows_armor_and_shield_armor() {
        let mut world = World::new(42);
        let (bx, by) = (world.base.x as i32, world.base.y as i32);
        world.step(&[
            Order::PlaceBuilding { kind: BuildingKind::StoneHouse, tile: (bx + 14, by), turned: false, editor: true },
            Order::PlaceBuilding { kind: BuildingKind::Wall, tile: (bx + 14, by + 6), turned: false, editor: true },
        ]);
        let house = world.buildings.iter().find(|b| b.kind == BuildingKind::StoneHouse).expect("the house stands");
        assert!(building_weapon(&world, house).is_none());
        let shield = building_shield_armor(&world, house).expect("a stone house has a shield");
        assert!(has(&shield.tip, "Shield armor: 0") && has(&shield.tip, "Structure armor 0/3 (each level: +1 shield armor)"), "{:?}", shield.tip);
        let wall = world.buildings.iter().find(|b| b.kind == BuildingKind::Wall).expect("the wall stands");
        assert!(building_shield_armor(&world, wall).is_none(), "a wall has no shield");
        assert!(has(&building_armor(&world, wall).tip, "Is: Armored, Structure"));
    }

    /// A zombie's boxes: Fat is armored, with no points of its own (TAB's share is in its hit points); the tooltip says so.
    #[test]
    fn a_zombie_shows_weapon_and_armor_too() {
        let mut world = World::new(42);
        world.zombies.clear();
        let center = world.base + vec2(30.0, 0.0);
        world.step(&[Order::PlaceZombies { center, radius: 0.0, count: 1, breed: Breed::Fat, wander: true }]);
        world.step(&[Order::SetUpgrade { track: "zombie_armor".into(), level: 3 }]);
        let zombie = world.zombies.iter().find(|z| z.breed == Breed::Fat).expect("the fat one stands");
        let armor = zombie_armor(&world, zombie);
        assert_eq!((armor.value.as_str(), armor.up), ("0", 3));
        assert!(has(&armor.tip, "Is: Armored, Biological") && has(&armor.tip, "Zombie armor 3/3: +3 armor"), "{:?}", armor.tip);
        let weapon = zombie_weapon(&world, zombie);
        assert_eq!(weapon.value, Breed::Fat.stats().damage.to_string());
        assert!(has(&weapon.tip, "Range: melee") && has(&weapon.tip, "Zombie weapons 0/3 (each level: +4 damage)"), "{:?}", weapon.tip);
    }
}
