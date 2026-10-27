//! What the editor gives a placed building or unit, like TAB's entity properties (`FEditObject`): a **tag** – the name
//! scripts know it by, a name and a text the player sees, a mark on the minimap – and its **owner**, TAB's `Team`:
//! the colony, nobody (neutral), or nobody until one of your units comes close (TAB's `NeutralJoinable`).
//!
//! A neutral building or unit isn't one of the colony's: it waits in a list of its own (`World::neutral`) and takes
//! part in nothing – nobody selects it, the zombies leave it alone, it costs, pays and sees nothing. A building blocks
//! its tiles like a house of doom (`Map::set_doom`). A joinable one joins the colony as soon as one of your units is
//! within a tile of it – TAB checks every 1/3 s (`CNeutralJoinable`) – and from then on it's an ordinary building or
//! unit.

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use super::events::Notice;
use super::World;
use crate::kinds::{BuildingKind, UnitKind};

/// A joinable one looks for your units every this many ticks (TAB: `IsIAFrameModule(20)`, 1/3 s).
const JOIN_EVERY: u64 = 7;

/// A tag of a placed building or unit (TAB: "Name ID", "Custom Name", "Custom Description", "Display Icon on
/// Minimap"). Empty fields aren't written into the map file.
#[derive(Clone, Default, PartialEq, Debug, Serialize, Deserialize)]
pub struct Tag {
    /// The name the scripts know it by (`name_id`, unique per map – the editor doesn't check).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name_id: String,
    /// Shown instead of its kind's name, and a text under it when it's selected.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// A mark on the minimap, always – also in the fog.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub minimap: bool,
}

impl Tag {
    /// Nothing set – then a thing has no tag at all.
    pub fn is_empty(&self) -> bool {
        *self == Tag::default()
    }
}

/// Who a placed thing belongs to (TAB's `Team`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Owner {
    Colony,
    Neutral,
    /// Neutral until one of your units comes within a tile – then the colony's.
    Joinable,
}

/// What a neutral thing is.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum Thing {
    Building { kind: BuildingKind, tile: (i32, i32), turned: bool },
    Unit { kind: UnitKind, pos: (f32, f32) },
}

/// A neutral building or unit, as it waits (`World::neutral`). How it will be once it joins: `hp_percent` of its hit
/// points, a veteran or not, its tag.
#[derive(Clone, Debug)]
pub struct Neutral {
    pub id: u32,
    pub thing: Thing,
    pub joinable: bool,
    pub hp_percent: u32,
    pub veteran: bool,
    pub tag: Option<Box<Tag>>,
}

impl Neutral {
    /// Its middle (world point).
    pub fn center(&self) -> Vec2 {
        match self.thing {
            Thing::Building { kind, tile, turned } => {
                let (w, h) = kind.footprint(turned);
                vec2(tile.0 as f32 + w as f32 / 2.0, tile.1 as f32 + h as f32 / 2.0)
            }
            Thing::Unit { pos, .. } => vec2(pos.0, pos.1),
        }
    }

    /// The name the player sees: its tag's, else its kind's.
    pub fn name(&self) -> &str {
        match (&self.tag, self.thing) {
            (Some(tag), _) if !tag.name.is_empty() => &tag.name,
            (_, Thing::Building { kind, .. }) => &kind.stats().name,
            (_, Thing::Unit { kind, .. }) => &kind.stats().name,
        }
    }

    /// Is one of these points within a tile of it (TAB: its cells grown by 1)?
    fn touched(&self, units: impl Iterator<Item = Vec2>) -> bool {
        let (min, max) = match self.thing {
            Thing::Building { kind, tile, turned } => {
                let (w, h) = kind.footprint(turned);
                (vec2(tile.0 as f32, tile.1 as f32), vec2((tile.0 + w) as f32, (tile.1 + h) as f32))
            }
            Thing::Unit { pos, .. } => {
                let cell = vec2(pos.0.floor(), pos.1.floor());
                (cell, cell + Vec2::ONE)
            }
        };
        let (min, max) = (min - Vec2::ONE, max + Vec2::ONE);
        units.into_iter().any(|p| p.x >= min.x && p.y >= min.y && p.x < max.x && p.y < max.y)
    }
}

/// A placed thing the editor works on, by its number: one of the colony's buildings or units, or a neutral one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThingRef {
    Building(u32),
    Unit(u32),
    Neutral(u32),
}

impl World {
    /// A neutral building or unit (loading a map file, the editor). A building blocks its tiles from now on – where it
    /// fits; one that doesn't fit isn't added.
    pub fn add_neutral(&mut self, thing: Thing, joinable: bool, hp_percent: u32, veteran: bool, tag: Option<Box<Tag>>) {
        if let Thing::Building { kind, tile, turned } = thing {
            if !self.can_place_anywhere(kind, tile, turned) {
                return;
            }
            self.map.set_doom(tile.0, tile.1, kind.footprint(turned), true);
            self.map_changed = true;
        }
        self.next_neutral_id += 1;
        let hp_percent = hp_percent.clamp(1, 100);
        self.neutral.push(Neutral { id: self.next_neutral_id, thing, joinable, hp_percent, veteran, tag });
    }

    /// The number the next neutral thing gets – the editor selects it right after making one.
    pub fn next_ids(&self) -> (u32, u32, u32) {
        (self.next_building_id + 1, self.next_unit_id + 1, self.next_neutral_id + 1)
    }

    /// Where is the neutral thing with this `id` in the list?
    pub fn find_neutral(&self, id: u32) -> Option<usize> {
        self.neutral.iter().position(|n| n.id == id)
    }

    /// Every `JOIN_EVERY` ticks: every joinable thing one of your units (on the ground) is within a tile of joins the
    /// colony – in the order of the list.
    pub(super) fn join_neutrals(&mut self) {
        if !self.tick.is_multiple_of(JOIN_EVERY) || !self.neutral.iter().any(|n| n.joinable) {
            return;
        }
        let units: Vec<Vec2> = self.units.iter().filter(|u| !u.is_inside()).map(|u| u.pos).collect();
        let joining: Vec<u32> = self.neutral.iter().filter(|n| n.joinable && n.touched(units.iter().copied())).map(|n| n.id).collect();
        for id in joining {
            if let Some(n) = self.find_neutral(id) {
                let name = self.neutral[n].name().to_owned();
                self.to_colony(n);
                self.notices.push(Notice::Joined { name });
            }
        }
    }

    /// Neutral thing `n` becomes the colony's: an ordinary building (standing at once, like the editor's) or unit,
    /// with its hit points, veteran and tag.
    fn to_colony(&mut self, n: usize) {
        let neutral = self.neutral.remove(n);
        match neutral.thing {
            Thing::Building { kind, tile, turned } => {
                self.map.set_doom(tile.0, tile.1, kind.footprint(turned), false);
                self.add_building(kind, tile, turned);
                let building = self.buildings.last_mut().expect("just added");
                building.hp = (building.hp * neutral.hp_percent / 100).max(1);
                building.tag = neutral.tag;
            }
            Thing::Unit { kind, pos } => {
                self.spawn_unit(kind, vec2(pos.0, pos.1));
                let unit = self.units.last_mut().expect("just added");
                if neutral.veteran {
                    unit.make_veteran();
                }
                unit.hp = (unit.max_hp() * neutral.hp_percent / 100).max(1);
                unit.tag = neutral.tag;
            }
        }
        self.map_changed = true;
    }

    /// A building or unit of the colony becomes neutral (or joinable): out of the colony's lists into `neutral`,
    /// with what it is.
    fn to_neutral(&mut self, thing: ThingRef, joinable: bool) {
        match thing {
            ThingRef::Building(id) => {
                let Some(b) = self.buildings.iter().position(|b| b.id == id) else {
                    return;
                };
                let building = &self.buildings[b];
                let (kind, tile, turned) = (building.kind, building.tile, building.turned);
                let percent = building.hp * 100 / building.stats().hp.max(1);
                let tag = building.tag.clone();
                self.take_down(id);
                self.refresh_map();
                self.add_neutral(Thing::Building { kind, tile, turned }, joinable, percent, false, tag);
            }
            ThingRef::Unit(id) => {
                let Some(u) = self.units.iter().position(|u| u.id == id) else {
                    return;
                };
                let unit = self.units.remove(u);
                let percent = unit.hp * 100 / unit.max_hp().max(1);
                let thing = Thing::Unit { kind: unit.kind, pos: (unit.pos.x, unit.pos.y) };
                self.add_neutral(thing, joinable, percent, unit.veteran, unit.tag);
            }
            ThingRef::Neutral(_) => {}
        }
    }

    /// Editor: `thing` belongs to `owner` from now on.
    pub(super) fn set_owner(&mut self, thing: ThingRef, owner: Owner) {
        match (thing, owner) {
            (ThingRef::Neutral(id), Owner::Colony) => {
                if let Some(n) = self.find_neutral(id) {
                    self.to_colony(n);
                }
            }
            (ThingRef::Neutral(id), _) => {
                if let Some(n) = self.find_neutral(id) {
                    self.neutral[n].joinable = owner == Owner::Joinable;
                }
            }
            (_, Owner::Colony) => {}
            (thing, owner) => self.to_neutral(thing, owner == Owner::Joinable),
        }
    }

    /// Editor: the tag of `thing` – an empty one takes it off.
    pub(super) fn set_tag(&mut self, thing: ThingRef, tag: &Tag) {
        let tag = (!tag.is_empty()).then(|| Box::new(tag.clone()));
        match thing {
            ThingRef::Building(id) => {
                if let Some(b) = self.buildings.iter_mut().find(|b| b.id == id) {
                    b.tag = tag;
                }
            }
            ThingRef::Unit(id) => {
                if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
                    u.tag = tag;
                }
            }
            ThingRef::Neutral(id) => {
                if let Some(n) = self.neutral.iter_mut().find(|n| n.id == id) {
                    n.tag = tag;
                }
            }
        }
    }

    /// Editor: `thing` has `percent` of its full hit points (1–100).
    pub(super) fn set_health(&mut self, thing: ThingRef, percent: u32) {
        let percent = percent.clamp(1, 100);
        match thing {
            ThingRef::Building(id) => {
                if let Some(b) = self.buildings.iter_mut().find(|b| b.id == id) {
                    b.hp = (b.stats().hp * percent / 100).max(1);
                }
            }
            ThingRef::Unit(id) => {
                if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
                    u.hp = (u.max_hp() * percent / 100).max(1);
                }
            }
            ThingRef::Neutral(id) => {
                if let Some(n) = self.neutral.iter_mut().find(|n| n.id == id) {
                    n.hp_percent = percent;
                }
            }
        }
    }

    /// Editor: a building of the colony infected or healed (TAB: "Is Infected?") – nobody comes out, it's only its
    /// state; a unit a veteran or not ("Is Veteran?"), a neutral one too.
    pub(super) fn set_mark(&mut self, thing: ThingRef, on: bool) {
        match thing {
            ThingRef::Building(id) => {
                if let Some(b) = self.buildings.iter_mut().find(|b| b.id == id) {
                    b.infected = on;
                    b.shield = if on { 0 } else { b.stats().max_shield() };
                }
            }
            ThingRef::Unit(id) => {
                if let Some(u) = self.units.iter_mut().find(|u| u.id == id) {
                    if on && !u.veteran {
                        u.make_veteran();
                    } else if !on && u.veteran {
                        u.veteran = false;
                        u.xp = 0;
                        u.hp = u.hp.min(u.max_hp());
                    }
                }
            }
            ThingRef::Neutral(id) => {
                if let Some(n) = self.neutral.iter_mut().find(|n| n.id == id) {
                    n.veteran = on;
                }
            }
        }
    }

    /// Editor: `thing` goes – nothing comes back.
    pub(super) fn remove_thing(&mut self, thing: ThingRef) {
        match thing {
            ThingRef::Building(id) => self.take_down(id),
            ThingRef::Unit(id) => self.units.retain(|u| u.id != id),
            ThingRef::Neutral(id) => {
                if let Some(n) = self.find_neutral(id) {
                    let neutral = self.neutral.remove(n);
                    if let Thing::Building { kind, tile, turned } = neutral.thing {
                        self.map.set_doom(tile.0, tile.1, kind.footprint(turned), false);
                        self.map_changed = true;
                    }
                }
            }
        }
    }

    /// The neutral things on these tiles (the editor's eraser) – a building if any of its tiles is hit.
    pub(super) fn remove_neutral_at(&mut self, on_tile: impl Fn(i32, i32) -> bool) {
        let gone: Vec<u32> = self
            .neutral
            .iter()
            .filter(|n| match n.thing {
                Thing::Building { kind, tile, turned } => {
                    let (w, h) = kind.footprint(turned);
                    (tile.1..tile.1 + h).any(|y| (tile.0..tile.0 + w).any(|x| on_tile(x, y)))
                }
                Thing::Unit { pos, .. } => on_tile(pos.0.floor() as i32, pos.1.floor() as i32),
            })
            .map(|n| n.id)
            .collect();
        for id in gone {
            self.remove_thing(ThingRef::Neutral(id));
        }
    }

    /// What a map file says about a building or unit of the colony: its hit points in percent, infected (a building) or
    /// a veteran (a unit), its tag (`mapfile.rs`).
    pub fn edit_thing(&mut self, thing: ThingRef, hp_percent: u32, mark: bool, tag: Option<&Tag>) {
        if mark {
            self.set_mark(thing, true);
        }
        if hp_percent < 100 {
            self.set_health(thing, hp_percent);
        }
        if let Some(tag) = tag {
            self.set_tag(thing, tag);
        }
    }

    /// The tag of `thing`, if it has one.
    pub fn tag_of(&self, thing: ThingRef) -> Option<&Tag> {
        match thing {
            ThingRef::Building(id) => self.buildings.iter().find(|b| b.id == id)?.tag.as_deref(),
            ThingRef::Unit(id) => self.units.iter().find(|u| u.id == id)?.tag.as_deref(),
            ThingRef::Neutral(id) => self.neutral.iter().find(|n| n.id == id)?.tag.as_deref(),
        }
    }

    /// Where the things the editor marked for the minimap are (`Tag::minimap`) – buildings, units, neutral ones.
    pub fn minimap_marks(&self) -> Vec<Vec2> {
        let marked = |tag: &Option<Box<Tag>>| tag.as_ref().is_some_and(|t| t.minimap);
        let buildings = self.buildings.iter().filter(|b| marked(&b.tag)).map(|b| b.center());
        let units = self.units.iter().filter(|u| marked(&u.tag)).map(|u| u.pos);
        let neutral = self.neutral.iter().filter(|n| marked(&n.tag)).map(|n| n.center());
        buildings.chain(units).chain(neutral).collect()
    }

    /// Where `thing` is (its middle) – `None` if it's gone.
    pub fn thing_center(&self, thing: ThingRef) -> Option<Vec2> {
        match thing {
            ThingRef::Building(id) => self.buildings.iter().find(|b| b.id == id).map(|b| b.center()),
            ThingRef::Unit(id) => self.units.iter().find(|u| u.id == id).map(|u| u.pos),
            ThingRef::Neutral(id) => self.neutral.iter().find(|n| n.id == id).map(|n| n.center()),
        }
    }
}
