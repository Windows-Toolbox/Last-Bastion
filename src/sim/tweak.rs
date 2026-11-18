//! The combat values of **one** unit, zombie or building that differ from its kind's (`data\`): what the editor's
//! Objects module sets on a single thing (`Order::SetTweak`), and what upgrades like StarCraft 2's will change later.
//! Damage, attack speed, range, sight and speed are percents of what the kind has – 100 changes nothing; hit points,
//! shield and the two armors are numbers of their own, the kind's when left out. Only the values that make a fight. (A map can change *all* values of a *kind* – the Data
//! module, `data::Overrides`.)
//!
//! Deterministic like everything in the simulation: whole percents, `u16`.

use serde::{Deserialize, Serialize};

/// The least and the most of a percent (10 % … 1000 %) – so nothing is zero or silly.
pub const PERCENT: std::ops::RangeInclusive<u16> = 10..=1000;
/// The most hit points or shield a number may give – so a sum never overflows.
pub const MOST_LIFE: u32 = 10_000_000;

/// Percents of the kind's values – see the module. Hit points, shield and armor are numbers of their own. Order of the
/// fields is the order of the editor's fields.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Tweak {
    /// Hit points as a number, not a percent – the kind's when `None`. A building that has only a shield (a wall) has 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hp: Option<u32>,
    /// A building's shield as a number – zombies hit it first; hit points and shield together are its life, it is
    /// destroyed when both are 0. The kind's when `None`; units have none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shield: Option<u32>,
    pub damage: u16,
    /// 150 shoots one and a half times as often: every time of the weapon (raise, aim, wind-up, cooldown …) is divided by it.
    pub attack_speed: u16,
    pub range: u16,
    pub sight: u16,
    pub speed: u16,
    /// Armor points as a number – the kind's when `None` (a walker with 3 is a tough one). The file names it
    /// `armor_points`: before, it was a percent.
    #[serde(rename = "armor_points", skip_serializing_if = "Option::is_none")]
    pub armor: Option<u32>,
    /// A building's shield armor points as a number – the kind's when `None`.
    #[serde(rename = "shield_armor_points", skip_serializing_if = "Option::is_none")]
    pub shield_armor: Option<u32>,
}

impl Default for Tweak {
    fn default() -> Tweak {
        Tweak { hp: None, shield: None, damage: 100, attack_speed: 100, range: 100, sight: 100, speed: 100, armor: None, shield_armor: None }
    }
}

/// The most armor points a number may give – so a sum never overflows.
pub const MOST_ARMOR: u32 = 1000;

impl Tweak {
    /// Nothing changed – the map file leaves it out.
    pub fn is_default(&self) -> bool {
        *self == Tweak::default()
    }

    /// Every percent inside `PERCENT` – what comes from a file or a field.
    pub fn clamped(self) -> Tweak {
        let c = |p: u16| p.clamp(*PERCENT.start(), *PERCENT.end());
        Tweak { hp: self.hp.map(|h| h.min(MOST_LIFE)), shield: self.shield.map(|h| h.min(MOST_LIFE)), damage: c(self.damage), attack_speed: c(self.attack_speed), range: c(self.range), sight: c(self.sight), speed: c(self.speed), armor: self.armor.map(|a| a.min(MOST_ARMOR)), shield_armor: self.shield_armor.map(|a| a.min(MOST_ARMOR)) }
    }

    /// A whole number scaled by a percent, rounded to the nearest (`hp`, `damage`).
    pub fn scale(value: u32, percent: u16) -> u32 {
        (value as u64 * percent as u64 + 50).min(u32::MAX as u64 * 100) as u32 / 100
    }

    /// A time (ticks) that gets shorter the faster it attacks: `ticks × 100 / (attack speed + extra)`, rounded up so a
    /// time that was there never becomes 0 – and a 0 stays 0. `extra`: percent points the upgrades add (`upgrades.rs`).
    pub fn quicker_with(&self, ticks: u32, extra: u32) -> u32 {
        (ticks as u64 * 100).div_ceil((self.attack_speed as u64 + extra as u64).max(1)) as u32
    }

    /// The armor points: its own number, or the kind's.
    pub fn armor_of(&self, kind: u32) -> u32 {
        self.armor.unwrap_or(kind)
    }

    /// The shield armor points: its own number, or the kind's.
    pub fn shield_armor_of(&self, kind: u32) -> u32 {
        self.shield_armor.unwrap_or(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_tweak_changes_nothing() {
        let t = Tweak::default();
        assert!(t.is_default());
        assert_eq!((Tweak::scale(120, 100), t.quicker_with(11, 0), t.quicker_with(0, 0), t.armor_of(3)), (120, 11, 0, 3));
    }

    #[test]
    fn percents_scale_and_round_sensibly() {
        let fast = Tweak { attack_speed: 200, ..Tweak::default() };
        assert_eq!(fast.quicker_with(11, 0), 6, "half the time, rounded up: 5.5 → 6");
        assert_eq!(fast.quicker_with(1, 0), 1, "a time that was there never becomes 0");
        assert_eq!(Tweak::scale(16, 150), 24);
        assert_eq!(Tweak::scale(15, 150), 23, "22.5 rounds up");
        let tough = Tweak { armor: Some(5), ..Tweak::default() };
        assert_eq!((tough.armor_of(3), Tweak::default().armor_of(3)), (5, 3), "its own armor, or the kind's");
        assert_eq!(Tweak { armor: Some(0), ..Tweak::default() }.armor_of(3), 0, "0 is a number too");
        assert_eq!(Tweak { hp: Some(u32::MAX), armor: Some(u32::MAX), speed: 5000, ..Tweak::default() }.clamped(), Tweak { hp: Some(MOST_LIFE), armor: Some(MOST_ARMOR), speed: 1000, ..Tweak::default() });
        let file: Tweak = serde_json::from_str("{\"damage\": 150}").unwrap();
        assert_eq!((file.damage, file.hp), (150, None), "a file names only what differs");
    }
}
