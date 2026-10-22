//! The market (DESIGN.md 6): founded early, it grows with the colony.
//!
//! - **Its houses** are the ones whose middle lies on its site or `trade_range` tiles around it (`trade_area`) – each
//!   house belongs to one market only, the oldest one that reaches it. They pay `house_bonus_percent` more gold.
//! - **It sells** what the stock has too much of: everything above `sell_from_percent` of the storage, one unit of
//!   each good in turn, at `sell_prices`. How much per payout: one good for every `colonists_per_good` colonists living
//!   in its houses – more houses, bigger houses, more it sells. Q on the market switches selling off (and on again).
//! - Both happen at the payout (`economy_payout`): the bonus when the houses pay, the selling right after – before the
//!   upkeep, so the gold already helps there, and after the houses ate, so it never sells their food.
//!
//! Switched off (Y) or infected it does neither.

use super::World;
use crate::data;
use crate::kinds::{Area, BuildingKind, Resource};

impl World {
    /// Does building `m` work as a market right now – a market, not switched off, not infected?
    fn runs_market(&self, m: usize) -> bool {
        let b = &self.buildings[m];
        b.kind == BuildingKind::Market && !b.off && !b.infected
    }

    /// Is building `h` a house (colonists live in it) whose middle lies in `area`? A middle on the edge counts: the
    /// house stands half in it.
    fn house_in(&self, h: usize, area: Area) -> bool {
        let house = &self.buildings[h];
        let ((x0, y0), (x1, y1)) = area;
        let c = house.center();
        let inside = c.x >= x0 as f32 && c.y >= y0 as f32 && c.x <= x1 as f32 && c.y <= y1 as f32;
        house.stats().colonists > 0 && !house.infected && inside
    }

    /// The houses (indices) a market with this `area` would have: the ones in it that no running market further up the
    /// list (index below `before`) has already. A market that isn't built yet: `before` = the length of the list.
    pub fn market_houses(&self, area: Area, before: usize) -> Vec<usize> {
        let older: Vec<Area> = (0..before.min(self.buildings.len()))
            .filter(|&m| self.runs_market(m))
            .map(|m| BuildingKind::Market.trade_area(self.buildings[m].tile))
            .collect();
        (0..self.buildings.len())
            .filter(|&h| self.house_in(h, area) && !older.iter().any(|&a| self.house_in(h, a)))
            .collect()
    }

    /// The market house `h` belongs to: the oldest running one that reaches it – `None`: no market.
    pub fn market_of(&self, h: usize) -> Option<usize> {
        (0..self.buildings.len())
            .filter(|&m| self.runs_market(m))
            .find(|&m| self.house_in(h, BuildingKind::Market.trade_area(self.buildings[m].tile)))
    }

    /// How many percent more gold house `h` pays – its market's `house_bonus_percent`, 0 without a market.
    pub fn market_bonus(&self, h: usize) -> u32 {
        self.market_of(h).map_or(0, |m| self.buildings[m].stats().house_bonus_percent)
    }

    /// The customers of a market with these houses: everybody living in them.
    pub fn customers(&self, houses: &[usize]) -> u32 {
        houses.iter().map(|&h| self.buildings[h].stats().colonists).sum()
    }

    /// How many goods a market with these houses sells per payout at most: one for every `colonists_per_good`.
    pub fn sell_limit(&self, houses: &[usize]) -> u32 {
        self.customers(houses) / BuildingKind::Market.stats().colonists_per_good.max(1)
    }

    /// At the payout, after the houses paid: every running market that sells takes what lies above the mark – one unit
    /// of each good in turn, until its limit is reached or nothing is above it anymore – and gets its price in gold.
    /// Markets further up the list first. What each one sold is kept for the panel (`sold_goods`, `sold_gold`).
    pub(super) fn sell_surplus(&mut self) {
        let buildings = &data::get().buildings;
        for m in 0..self.buildings.len() {
            if self.buildings[m].kind != BuildingKind::Market {
                continue;
            }
            self.buildings[m].sold_goods = 0;
            self.buildings[m].sold_gold = 0;
            if !self.runs_market(m) || !self.buildings[m].selling {
                continue;
            }
            let houses = self.market_houses(BuildingKind::Market.trade_area(self.buildings[m].tile), m);
            let mut left = self.sell_limit(&houses);
            let (mut goods, mut gold) = (0, 0);
            // Round after round, one of each good – until a whole round sold nothing.
            let mut sold_any = true;
            while left > 0 && sold_any {
                sold_any = false;
                for r in Resource::ALL {
                    let mark = self.storage(r) * buildings.sell_from_percent / 100;
                    if left > 0 && self.stock.get(r) > mark {
                        self.stock.take(r, 1);
                        left -= 1;
                        goods += 1;
                        gold += price(r);
                        sold_any = true;
                    }
                }
            }
            self.earn(gold);
            self.buildings[m].sold_goods = goods;
            self.buildings[m].sold_gold = gold;
        }
    }
}

/// What a market gets for one unit of `r` (`sell_prices`).
pub fn price(r: Resource) -> u32 {
    let prices = &data::get().buildings.sell_prices;
    match r {
        Resource::Wood => prices.wood,
        Resource::Stone => prices.stone,
        Resource::Iron => prices.iron,
        Resource::Coal => prices.coal,
        Resource::Food => prices.food,
    }
}
