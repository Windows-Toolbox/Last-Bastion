//! Research like TAB (`ZX.Commands\Technology.cs`, the `Tech…` rows of its `Commands`, our list in
//! `data\research.json`):
//! - **Where:** each research belongs to one workshop – the wood workshop, the stone workshop or the foundry. Its
//!   button sits on that workshop's card, at TAB's spot.
//! - **Once:** done, it's done for the rest of the round – even if the workshop falls. Its button goes away.
//! - **One at a time** per workshop (TAB's `CanBeQueued` false), and the same research not in two workshops at once.
//! - **Price:** gold (wonders also wood, stone, iron, oil), paid when it starts, all back when it's cancelled (a click on
//!   it in the queue, like a unit).
//! - **Time:** 70 s × its factor (TAB: 1.4 × factor × 50) – while the workshop has power, slower on a short grid, not
//!   at all when it's switched off. It goes through the workshop's production queue like a unit in the barracks.
//! - **What it unlocks** can be built or trained from then on – if what that `requires` stands too (`unlocked`).
//! - Thanatos and Titan only show once the engineering center is researched and one stands (`needs_tech`,
//!   `needs_building`).

use super::{Building, Notice, Product, World};
use crate::data::{self, Tech};
use crate::kinds::{BuildingKind, UnitKind};

/// Research `t` of the list.
pub fn tech(t: u16) -> &'static Tech {
    &data::get().research.techs[t as usize]
}

/// The researches done in workshop `kind`, in the order of the list.
pub fn researched_in(kind: BuildingKind) -> impl Iterator<Item = u16> {
    let techs = &data::get().research.techs;
    (0..techs.len() as u16).filter(move |&t| techs[t as usize].researched_in == kind)
}

impl World {
    /// Is research `t` done?
    pub fn researched(&self, t: u16) -> bool {
        self.researched.contains(&t)
    }

    /// Is every research done that unlocks building `kind`? (Most need one, many none.)
    pub fn building_researched(&self, kind: BuildingKind) -> bool {
        let techs = &data::get().research.techs;
        (0..techs.len() as u16).filter(|&t| techs[t as usize].buildings.contains(&kind)).all(|t| self.researched(t))
    }

    /// The same for training unit `kind`.
    pub fn unit_researched(&self, kind: UnitKind) -> bool {
        let techs = &data::get().research.techs;
        (0..techs.len() as u16).filter(|&t| techs[t as usize].units.contains(&kind)).all(|t| self.researched(t))
    }

    /// The first research still missing for building `kind` – for "research it first".
    pub fn missing_research(&self, kind: BuildingKind) -> Option<u16> {
        let techs = &data::get().research.techs;
        (0..techs.len() as u16).find(|&t| techs[t as usize].buildings.contains(&kind) && !self.researched(t))
    }

    /// The same for unit `kind`.
    pub fn missing_unit_research(&self, kind: UnitKind) -> Option<u16> {
        let techs = &data::get().research.techs;
        (0..techs.len() as u16).find(|&t| techs[t as usize].units.contains(&kind) && !self.researched(t))
    }

    /// Is research `t` going on in some workshop right now?
    pub fn researching(&self, t: u16) -> bool {
        self.buildings.iter().any(|b| b.production.contains(&Product::Research(t)))
    }

    /// Does research `t` show on its workshop's card: not blocked by the map, not done, not going on, and what it
    /// `needs` is there (TAB's `IsVisibleFor`).
    pub fn tech_offered(&self, t: u16) -> bool {
        let tech = tech(t);
        let research = &data::get().research;
        let needs_tech = tech.needs_tech.as_deref().and_then(|id| research.index(id)).is_none_or(|n| self.researched(n));
        // A research of an upgrade track comes in order: it is offered when the track is one level below.
        let in_order = tech.upgrade.as_ref().is_none_or(|u| self.upgrade_level(&u.track) + 1 == u.level);
        !tech.blocked && !self.researched(t) && !self.researching(t) && needs_tech && in_order && self.stands(tech.needs_building)
    }

    /// Are wood, stone, iron and oil enough for research `t`?
    pub fn can_afford_tech_goods(&self, t: u16) -> bool {
        tech(t).goods().iter().all(|&(r, n)| self.stock.get(r) >= n)
    }

    /// `Order::Research`: workshop `building` starts research `t` – if it's that research's workshop, finished, idle,
    /// the research is offered and the gold (with `Order::Credit` also the credit) and the goods are there.
    pub(super) fn start_research(&mut self, building: u32, t: u16) {
        let Some(b) = self.find_building(building) else {
            return;
        };
        if t as usize >= data::get().research.techs.len() {
            return;
        }
        let tech = tech(t);
        let workshop: &Building = &self.buildings[b];
        let idle = workshop.kind == tech.researched_in && workshop.is_built() && !workshop.infected && workshop.production.is_empty();
        if !idle || !self.tech_offered(t) || self.spendable() < tech.gold || !self.can_afford_tech_goods(t) {
            return;
        }
        self.pay_gold(tech.gold);
        for (r, n) in tech.goods() {
            self.stock.take(r, n);
        }
        self.score.gold_spent += tech.gold;
        self.buildings[b].production.push_back(Product::Research(t));
    }

    /// Research `t` is done (the production queue, `buildings.rs`): from now on, and a notice.
    pub(super) fn finish_research(&mut self, t: u16) {
        if !self.researched(t) {
            self.researched.push(t);
        }
        if let Some(u) = &tech(t).upgrade {
            self.raise_upgrade(&u.track, u.level);
        }
        self.notices.push(Notice::Researched { name: tech(t).name.clone() });
    }

    /// Tests: everything researched – so tests of buildings and units don't have to research first.
    #[cfg(test)]
    pub(super) fn research_all(&mut self) {
        self.researched = (0..data::get().research.techs.len() as u16).collect();
    }
}
