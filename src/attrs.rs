//! What a unit, zombie or building *is*, like StarCraft 2's attributes (**Light**, **Armored**, **Massive**, …), and the
//! extra damage a weapon does against them (`"bonus": { "armored": 10 }` – SC2's "+10 (vs Armored)"). Which attributes a
//! thing has stands in its own file (`"attributes": ["light", "biological"]`), the weapon's bonus in the weapon's block
//! or file. Nothing here changes the armor: a hit is `damage + extra` minus the armor (`sim::after_armor`).
//!
//! A track of upgrades (`sim/upgrades.rs`) adds `bonus_damage` per level to *every* bonus the weapon has – SC2: a
//! marauder's +1 (+1 vs Armored) per level.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One attribute – the file spells it in lower case (`"armored"`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Attribute {
    /// Small and fragile: walkers, rangers – flamers and machine guns are good against them.
    Light,
    /// Plated: riots, the big zombies, most buildings – sniper bolts and ballista shots are good against them.
    Armored,
    /// Huge: giants, the titan.
    Massive,
    /// Flesh and blood: everything that lives.
    Biological,
    /// Machines: the walking cannons.
    Mechanical,
    /// Buildings.
    Structure,
}

impl Attribute {
    pub const ALL: [Attribute; 6] =
        [Attribute::Light, Attribute::Armored, Attribute::Massive, Attribute::Biological, Attribute::Mechanical, Attribute::Structure];

    /// As shown in the game.
    pub fn name(self) -> &'static str {
        match self {
            Attribute::Light => "Light",
            Attribute::Armored => "Armored",
            Attribute::Massive => "Massive",
            Attribute::Biological => "Biological",
            Attribute::Mechanical => "Mechanical",
            Attribute::Structure => "Structure",
        }
    }
}

/// The attributes of a thing, as its file lists them – a handful at most, so a slice is faster than a set.
pub type Attributes = Vec<Attribute>;

/// The names of a list of attributes, e.g. "Light, Biological" – for the panel.
pub fn names(list: &[Attribute]) -> String {
    list.iter().map(|a| a.name()).collect::<Vec<_>>().join(", ")
}

/// The attributes of a building that doesn't name its own: plated and a structure (SC2: nearly all of them).
pub fn building_default() -> Attributes {
    vec![Attribute::Armored, Attribute::Structure]
}

/// A weapon's extra damage per attribute of the target – `{ "armored": 10, "light": 4 }`. A target with several of the
/// listed attributes takes all their extras (SC2). Left out of a file: no extra.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Extra(pub BTreeMap<Attribute, u32>);

impl Extra {
    /// How much more one hit does to a target with these attributes. `per_level` – the upgrades' `bonus_damage` – is
    /// added to every extra that applies.
    pub fn against(&self, target: &[Attribute], per_level: u32) -> u32 {
        self.0.iter().filter(|(a, _)| target.contains(a)).map(|(_, extra)| extra + per_level).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// "+10 vs Armored, +4 vs Light" with `per_level` put on – for the panel's tooltip. Empty: nothing.
    pub fn lines(&self, per_level: u32) -> Vec<(Attribute, u32)> {
        self.0.iter().map(|(a, extra)| (*a, extra + per_level)).collect()
    }
}

/// A weapon's extra damage as the shot sees it: the weapon's table (from the data, which lives for the whole program) and
/// the upgrades' per-level number. `Copy`, so it travels with a shot or an area attack.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExtraDamage {
    pub table: Option<&'static Extra>,
    pub per_level: u32,
}

impl ExtraDamage {
    pub fn new(table: &'static Extra, per_level: u32) -> ExtraDamage {
        ExtraDamage { table: (!table.is_empty()).then_some(table), per_level }
    }

    /// The extra damage against a target with these attributes.
    pub fn against(&self, target: &[Attribute]) -> u32 {
        self.table.map_or(0, |t| t.against(target, self.per_level))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_damage_counts_every_attribute_the_target_has() {
        let extra: Extra = serde_json::from_str(r#"{ "armored": 10, "massive": 5 }"#).unwrap();
        assert_eq!(extra.against(&[Attribute::Light], 0), 0);
        assert_eq!(extra.against(&[Attribute::Armored, Attribute::Biological], 0), 10);
        assert_eq!(extra.against(&[Attribute::Armored, Attribute::Massive], 0), 15, "both extras");
        assert_eq!(extra.against(&[Attribute::Armored, Attribute::Massive], 2), 19, "each extra grows with the upgrade");
        assert_eq!(extra.against(&[Attribute::Light], 3), 0, "an upgrade adds nothing to a target without a bonus");
    }

    #[test]
    fn attributes_are_written_in_lower_case() {
        let list: Attributes = serde_json::from_str(r#"["light", "biological"]"#).unwrap();
        assert_eq!(list, vec![Attribute::Light, Attribute::Biological]);
        assert_eq!(names(&list), "Light, Biological");
    }
}
