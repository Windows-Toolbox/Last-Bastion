//! Upkeep and debt (DESIGN.md 7.2), like in TAB: every unit and every running building costs gold every payout
//! (`upkeep` in `data\units\` and `data\buildings\`). Not enough gold: the rest is **debt**. Gold that comes in
//! pays it off first – while there is debt the gold stays at 0, so nothing new can be bought.
//!
//! In debt the army suffers, step by step (`data\colony.json`):
//! 1. any debt: less damage, slower shots,
//! 2. more than `deep_debt_payouts` of upkeep: even less, and the units walk slower,
//! 3. more than `desert_debt_payouts`: every `desert_every_seconds` one unit deserts – gone for good. DESIGN.md 7.2:
//!    weighted `upkeep × (3 if not a veteran, else 1)` – the dear ones first, veterans a third as likely
//!    (`veterans.rs`).
//!
//! **Mercenaries** (from the inn) work for gold: they are the first to go, and they go already at stage 2.
//!
//! **Banks** give credit (Dispo): debt up to their `credit` brings none of the penalties – only what's beyond it counts
//! for the stages above. Instead it costs `credit_interest_percent` of it at every payout. Buying never goes into it
//! by itself – only with Shift (`Order::Credit`), and only when gold is all that's missing. A bank that falls takes its
//! share of the debt in the credit with it – and the gold above the smaller storage.
//!
//! The penalties are one number for the whole army (`army_keep`, `army_pace`) – out of debt this costs nothing: the
//! upkeep isn't even added up then.

use super::{Fx, World};
use crate::data;

/// How bad the debt is right now (`World::debt_stage`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum DebtStage {
    None,
    /// Some debt: less damage, slower shots.
    Debt,
    /// More than `deep_debt_payouts` of upkeep: even less, and slower on foot.
    Deep,
    /// More than `desert_debt_payouts`: units desert.
    Desert,
}

impl World {
    /// The credit all banks give together – infected ones none.
    pub fn credit_line(&self) -> u32 {
        self.buildings.iter().filter(|b| !b.infected && b.is_built()).map(|b| b.stats().credit).sum()
    }

    /// The debt beyond the credit – the part that brings the penalties.
    pub fn penalty_debt(&self) -> u32 {
        self.debt.saturating_sub(self.credit_line())
    }

    /// The gold an order may spend: what there is – during an `Order::Credit` plus what's left of the credit.
    pub fn spendable(&self) -> u32 {
        let credit = if self.on_credit { self.credit_line().saturating_sub(self.debt) } else { 0 };
        self.gold + credit
    }

    /// The gold left to spend with Shift: the gold plus what's left of the credit (the panel and its buttons).
    pub fn spendable_on_credit(&self) -> u32 {
        self.gold + self.credit_line().saturating_sub(self.debt)
    }

    /// Pays `cost` (checked before with `spendable`): from the gold, and what's missing becomes debt – in the credit.
    pub(super) fn pay_gold(&mut self, cost: u32) {
        let paid = cost.min(self.gold);
        self.gold -= paid;
        self.debt += cost - paid;
    }

    /// The interest the debt in the credit costs at the next payout – rounded up, a little debt costs a little too.
    pub fn interest_due(&self) -> u32 {
        let in_credit = self.debt.min(self.credit_line());
        (in_credit * data::get().colony.credit_interest_percent).div_ceil(100)
    }

    /// At every payout, first: the interest goes onto the debt.
    pub(super) fn charge_interest(&mut self) {
        self.debt += self.interest_due();
    }

    /// Every tick: did a bank fall (destroyed or infected) since the last one? Then its share of the debt in the
    /// credit is gone with it – its part of the credit used, in proportion – and so is the gold above the smaller
    /// storage.
    fn watch_banks(&mut self) {
        let (line, storage) = (self.credit_line(), self.gold_storage());
        if line < self.credit_seen {
            let in_credit = self.debt.min(self.credit_seen) as u64;
            let gone = in_credit * (self.credit_seen - line) as u64 / self.credit_seen as u64;
            self.debt -= gone as u32;
        }
        if storage < self.storage_seen {
            self.gold = self.gold.min(storage);
        }
        (self.credit_seen, self.storage_seen) = (line, storage);
    }

    /// Gold per payout for everything that costs upkeep right now: every unit (not the ones in a production queue), every
    /// building that runs – not switched off, not infected.
    pub fn upkeep(&self) -> u32 {
        let units: u32 = self.units.iter().map(|u| u.stats().upkeep).sum();
        let buildings: u32 = self.buildings.iter().filter(|b| b.works()).map(|b| b.stats().upkeep).sum();
        units + buildings
    }

    /// Every payout (`economy_payout`, after the houses paid): the upkeep. What the gold doesn't cover becomes debt.
    pub(super) fn pay_upkeep(&mut self) {
        let due = self.upkeep();
        let paid = due.min(self.gold);
        self.gold -= paid;
        self.debt += due - paid;
    }

    /// How bad the debt is: measured in payouts of the upkeep right now – the same debt is worse for a small colony.
    /// Only what's beyond the banks' credit counts.
    pub fn debt_stage(&self) -> DebtStage {
        let colony = &data::get().colony;
        let payout = self.upkeep().max(1);
        match self.penalty_debt() {
            0 => DebtStage::None,
            d if d > payout * colony.desert_debt_payouts => DebtStage::Desert,
            d if d > payout * colony.deep_debt_payouts => DebtStage::Deep,
            _ => DebtStage::Debt,
        }
    }

    /// Every tick, after the buildings: gold that came in pays off the debt, the army's penalties follow the debt, and
    /// deep in debt units desert.
    pub(super) fn update_debt(&mut self) {
        self.watch_banks();
        let pay = self.gold.min(self.debt);
        self.gold -= pay;
        self.debt -= pay;
        if self.penalty_debt() == 0 {
            (self.army_keep, self.army_pace, self.desert_timer) = (1000, 1000, 0);
            return;
        }
        let colony = &data::get().colony;
        let stage = self.debt_stage();
        // Thousandths of their damage and shot speed the units keep, thousandths of their speed on foot.
        (self.army_keep, self.army_pace) = match stage {
            DebtStage::None => (1000, 1000),
            DebtStage::Debt => (1000 - colony.debt_penalty_percent * 10, 1000),
            DebtStage::Deep | DebtStage::Desert => {
                (1000 - colony.deep_debt_penalty_percent * 10, 1000 - colony.deep_debt_slow_percent * 10)
            }
        };
        // Mercenaries already leave at the stage before – the others only at the last one.
        let mercenaries = self.units.iter().any(|u| u.stats().mercenary);
        if stage < DebtStage::Deep || (stage < DebtStage::Desert && !mercenaries) {
            self.desert_timer = 0;
            return;
        }
        self.desert_timer += 1;
        if self.desert_timer >= colony.desert_every.max(1) {
            self.desert_timer = 0;
            self.desert();
        }
    }

    /// One unit leaves – picked at random, weighted by its upkeep (one without upkeep still counts a little), a veteran
    /// a third as likely: it stays loyal longer. It just goes: no corpse, it doesn't rise. Its workers are free again by
    /// themselves (`workers` counts the living). While there are mercenaries, one of them goes – they work for gold.
    fn desert(&mut self) {
        let mercenaries = self.units.iter().any(|u| u.stats().mercenary);
        let weight = |i: usize| {
            let unit = &self.units[i];
            let loyal = if unit.veteran { 1 } else { 3 };
            if mercenaries && !unit.stats().mercenary { 0 } else { unit.stats().upkeep.max(1) * loyal }
        };
        let total: u32 = (0..self.units.len()).map(weight).sum();
        if total == 0 {
            return;
        }
        let mut roll = self.rng.below(total);
        for i in 0..self.units.len() {
            if roll < weight(i) {
                // `remove`, not `swap_remove`: the order of the list stays the same on every PC.
                let gone = self.units.remove(i);
                self.fx.push(Fx::Deserted { kind: gone.kind });
                self.score.units_deserted += 1;
                return;
            }
            roll -= weight(i);
        }
    }

    /// A unit's damage per shot with the debt's penalty – at least 1.
    pub fn army_damage(&self, damage: u32) -> u32 {
        (damage * self.army_keep / 1000).max(1)
    }
}
