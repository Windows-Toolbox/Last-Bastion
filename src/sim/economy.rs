//! The colony's economy (DESIGN.md 5): the stock of goods, the power grids, what the gathering buildings bring in,
//! what the charcoal burners burn and what the houses eat.
//!
//! Every payout (every `economy_every_seconds`, 28 s like TAB's `CResourceGenerator`), in this order
//! (`economy_payout`): the burners take their wood, the gathering buildings and the command center put what they got
//! into the stock, the units eat, the houses eat and pay their gold (the command center pays without eating), the
//! upkeep is paid (`debt.rs`). Every "per payout" value in `data\` counts for one of them. A building
//! that's switched off (`Building::off`) does none of it – and isn't in any grid. Power is worked out anew whenever it's needed
//! (`power`) – it only depends on what stands where, and on which burner has wood.
//!
//! The stock counts in thousandths, so a gathering building that brings 2.5 wood per payout really brings 2.5 – no
//! float in the simulation's numbers (DESIGN.md 12.8). Shown and paid are whole units.

use macroquad::math::Vec2;

use super::World;
use crate::data;
use crate::kinds::{in_area, Area, BuildingKind, Resource, Source, UnitKind};
use crate::map::{Ore, Terrain};

/// What the colony has in store – wood, stone, iron ore, coal, food. Gold is `World::gold`.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Stock {
    /// Thousandths of a unit, one per `Resource` (`Resource::index`).
    milli: [u64; Resource::COUNT],
}

impl Stock {
    /// Whole units in store.
    pub fn get(&self, r: Resource) -> u32 {
        (self.milli[r.index()] / 1000).min(u32::MAX as u64) as u32
    }

    /// Puts `milli` more in, up to `limit` whole units – what doesn't fit is lost. More than that in it already (the
    /// editor, the tests): it stays, nothing comes in.
    fn add_milli(&mut self, r: Resource, milli: u64, limit: u32) {
        let have = &mut self.milli[r.index()];
        *have = (*have + milli).min((*have).max(limit as u64 * 1000));
    }

    /// Puts `units` more into the stock, up to `limit` (loot that was picked up).
    pub(super) fn add(&mut self, r: Resource, units: u32, limit: u32) {
        self.add_milli(r, units as u64 * 1000, limit);
    }

    /// Takes `units` out – or all there is, if there are fewer.
    pub(super) fn take_up_to(&mut self, r: Resource, units: u32) {
        let milli = &mut self.milli[r.index()];
        *milli = milli.saturating_sub(units as u64 * 1000);
    }

    /// Takes `units` out if there are that many – otherwise nothing, and `false`.
    pub fn take(&mut self, r: Resource, units: u32) -> bool {
        let milli = units as u64 * 1000;
        let ok = self.milli[r.index()] >= milli;
        if ok {
            self.milli[r.index()] -= milli;
        }
        ok
    }

    /// Sets the whole units of `r` (the editor's start wood) – what was in it is gone.
    pub fn set(&mut self, r: Resource, units: u32) {
        self.milli[r.index()] = units as u64 * 1000;
    }

    /// For the checksum.
    pub(super) fn all_milli(&self) -> [u64; Resource::COUNT] {
        self.milli
    }
}

/// The power grids right now (DESIGN.md 5.5). A grid starts at the command center; a tesla tower inside it joins it
/// and carries the power further. Every building that makes or takes power belongs to the grid whose command center
/// or tesla tower reaches its middle. A grid shares what its buildings make among those that take: not enough, and
/// they all work that much slower.
pub struct Power {
    /// Per building (index in `World::buildings`): its grid – `None` outside every grid, or nothing to do with power.
    pub grid_of: Vec<Option<usize>>,
    /// Per grid: the power its buildings make …
    pub made: Vec<u32>,
    /// … and take.
    pub used: Vec<u32>,
}

impl Power {
    /// How much of the power it needs building `b` gets, in thousandths (1000 = all). A building that needs none
    /// always gets all; one outside every grid gets nothing.
    pub fn share(&self, world: &World, b: usize) -> u32 {
        if world.energy_use(world.buildings[b].kind) == 0 {
            return 1000;
        }
        match self.grid_of[b] {
            None => 0,
            Some(g) => (self.made[g] as u64 * 1000 / self.used[g].max(1) as u64).min(1000) as u32,
        }
    }
}

impl World {
    /// How much of `r` fits in the stock (`colony.json`, later more with warehouses).
    pub fn storage(&self, r: Resource) -> u32 {
        let colony = &data::get().colony;
        let limit = if r == Resource::Food { colony.storage_food } else { colony.storage_goods };
        #[cfg(test)]
        let limit = limit.saturating_add(self.extra_storage);
        limit
    }

    /// How much gold fits (`colony.json`) – every finished bank that isn't infected adds its `gold_storage`.
    pub fn gold_storage(&self) -> u32 {
        let banks: u32 = self.buildings.iter().filter(|b| !b.infected && b.is_built()).map(|b| b.stats().gold_storage).sum();
        let limit = data::get().colony.storage_gold + banks;
        #[cfg(test)]
        let limit = limit.saturating_add(self.extra_storage);
        limit
    }

    /// Gold that comes in – the houses, loot – up to `gold_storage`, the rest is lost. More than that already (the
    /// editor, the tests): it stays, nothing comes in. Counted as earned all the same.
    pub(super) fn earn(&mut self, gold: u32) {
        self.gold = (self.gold + gold).min(self.gold.max(self.gold_storage()));
        self.score.gold_earned += gold;
    }

    /// The power grids as they are now (`Power`). Cheap enough to work out whenever it's needed: only the command
    /// center and the tesla towers are compared with each other. Infected buildings are out of every grid.
    pub fn power(&self) -> Power {
        let n = self.buildings.len();
        let nodes: Vec<usize> = (0..n).filter(|&b| self.carries_power(b)).collect();
        // Grid of every node: two are joined if one stands inside the other's reach. Filled from node to node in the
        // order of the list – the same on every PC.
        const NONE: usize = usize::MAX;
        let mut node_grid = vec![NONE; nodes.len()];
        let mut grids = 0;
        for start in 0..nodes.len() {
            if node_grid[start] != NONE {
                continue;
            }
            node_grid[start] = grids;
            let mut todo = vec![start];
            while let Some(k) = todo.pop() {
                for j in 0..nodes.len() {
                    if node_grid[j] == NONE && self.joined(nodes[k], nodes[j]) {
                        node_grid[j] = grids;
                        todo.push(j);
                    }
                }
            }
            grids += 1;
        }
        let mut power = Power { grid_of: vec![None; n], made: vec![0; grids], used: vec![0; grids] };
        for b in 0..n {
            let building = &self.buildings[b];
            let counts = building.kind.on_grid() || building.kind == BuildingKind::CommandCenter;
            if !building.works() || !counts {
                continue;
            }
            let center = building.center();
            let grid = match nodes.iter().position(|&node| node == b) {
                Some(k) => Some(node_grid[k]),
                None => nodes.iter().position(|&node| self.reaches(node, center)).map(|k| node_grid[k]),
            };
            if let Some(g) = grid {
                power.grid_of[b] = Some(g);
                let stats = building.stats();
                if stats.burns_wood == 0 || building.fuelled {
                    power.made[g] += stats.energy;
                }
                #[cfg(test)]
                if building.kind == BuildingKind::CommandCenter {
                    power.made[g] += self.extra_power;
                }
                power.used[g] += self.energy_use(building.kind);
                // A scan charging takes the command center's grid a lot of power (`scan.rs`).
                if self.scan_charger() == Some(b) {
                    power.used[g] += data::get().buildings.scan.power;
                }
            }
        }
        power
    }

    /// The power a building of `kind` takes: its `energy_use`, changed by the map (`Modifiers::building_energy`).
    pub fn energy_use(&self, kind: BuildingKind) -> u32 {
        self.modifiers.energy(kind.stats().energy_use)
    }

    /// Does any grid reach this point – may a building that makes, takes or carries power stand here?
    pub fn in_grid(&self, p: Vec2) -> bool {
        (0..self.buildings.len()).any(|b| self.carries_power(b) && self.reaches(b, p))
    }

    /// The command center and the tesla towers – those with a `power_radius` – as long as they aren't infected or
    /// switched off.
    pub fn carries_power(&self, b: usize) -> bool {
        let building = &self.buildings[b];
        building.stats().power_radius > 0.0 && building.works()
    }

    /// The grid reaching point `p` and what it has to spare (made minus used) – `None` if no grid reaches it. For a
    /// building that isn't in a grid yet: a new one, one that's switched off.
    pub fn spare_power_at(&self, power: &Power, p: Vec2) -> Option<(usize, u32)> {
        let grid = (0..self.buildings.len()).filter(|&c| self.carries_power(c) && self.reaches(c, p)).find_map(|c| power.grid_of[c])?;
        Some((grid, power.made[grid].saturating_sub(power.used[grid])))
    }

    /// Does the power of building `b` reach this point?
    pub fn reaches(&self, b: usize, p: Vec2) -> bool {
        let reach = self.buildings[b].stats().power_radius;
        #[cfg(test)]
        let reach = if self.buildings[b].kind == BuildingKind::CommandCenter { reach + self.extra_reach } else { reach };
        self.buildings[b].center().distance(p) <= reach
    }

    /// Two carriers of power are in the same grid if one of them stands inside the other's reach.
    fn joined(&self, a: usize, b: usize) -> bool {
        self.reaches(a, self.buildings[b].center()) || self.reaches(b, self.buildings[a].center())
    }

    /// The tiles a gathering building of `kind` standing at `tile` (top left) works: the fitting ones in its
    /// `gather_area` – its site plus `gather_range` tiles on every side – that nothing else is built on, and that no
    /// building of the same kind further up the list (index below `before`) works already. Two saw mills side by side
    /// share the forest, they don't both get it all (like in TAB). A building that isn't built yet: `before` = the
    /// length of the list.
    /// The tiles it stands on only count with `gather_below` (the mine) – and then always for it, even if an older
    /// mine reaches them. Built or only planned, the same tiles: the preview shows what it will get.
    pub fn worked_tiles(&self, kind: BuildingKind, tile: (i32, i32), before: usize) -> Vec<(i32, i32)> {
        let Some((source, _)) = kind.gathers() else {
            return Vec::new();
        };
        // The areas of the older ones of its kind – what lies in them is theirs.
        let earlier: Vec<Area> = self.buildings[..before.min(self.buildings.len())]
            .iter()
            .filter(|b| b.kind == kind && !b.infected && !b.off)
            .map(|b| kind.gather_area(b.tile))
            .collect();
        let (w, h) = kind.footprint(false);
        let site = (tile, (tile.0 + w, tile.1 + h));
        let ((x0, y0), (x1, y1)) = kind.gather_area(tile);
        let mut tiles = Vec::new();
        for y in y0..y1 {
            for x in x0..x1 {
                let own = in_area(site, (x, y));
                let taken = if own {
                    !kind.stats().gather_below
                } else {
                    self.map.is_built(x, y) || self.map.is_doom(x, y)
                };
                if !self.map.in_play(x, y) || taken {
                    continue;
                }
                let fits = match source {
                    Source::Forest => self.map.terrain(x, y) == Terrain::Forest,
                    Source::Hunting => self.hunting_percent(x, y) > 0,
                    Source::Water => self.map.terrain(x, y) == Terrain::Water,
                    Source::Ore => self.map.deposit(x, y).is_some_and(|d| Resource::from_ore(d.ore).is_some() || d.ore == Ore::Gold),
                    Source::Fertile => self.map.terrain(x, y).is_fertile() && self.map.deposit(x, y).is_none(),
                };
                if fits && (own || !earlier.iter().any(|&area| in_area(area, (x, y)))) {
                    tiles.push((x, y));
                }
            }
        }
        tiles
    }

    /// What a gathering building of `kind` at `tile` brings in per payout, in thousandths per resource
    /// (`Resource::index`) – with `share` thousandths of the power it needs (`Power::share`).
    pub fn gather_rate(&self, kind: BuildingKind, tile: (i32, i32), before: usize, share: u32) -> [u64; Resource::COUNT] {
        let mut rate = [0; Resource::COUNT];
        let Some((_, gives)) = kind.gathers() else {
            return rate;
        };
        // Rounded once from the file's number – the same on every PC.
        let per_tile = (kind.stats().gather_per_tile * 1000.0).round() as u64;
        let hunts = matches!(kind.gathers(), Some((Source::Hunting, _)));
        for (x, y) in self.worked_tiles(kind, tile, before) {
            let resource = gives.or_else(|| self.map.deposit(x, y).and_then(|d| Resource::from_ore(d.ore)));
            // The hunter: each tile its own share by what grows there.
            let of_tile = if hunts { per_tile * self.hunting_percent(x, y) as u64 / 100 } else { per_tile };
            if let Some(r) = resource {
                rate[r.index()] += of_tile * share as u64 / 1000;
            }
        }
        rate
    }

    /// How much game a tile has for a hunter cottage, in percent of its `gather_per_tile` – like TAB's: forest, grass
    /// and dirt each the map's own share (`Modifiers::hunting_*`), nothing on an ore patch, water or rock.
    fn hunting_percent(&self, x: i32, y: i32) -> u32 {
        if self.map.deposit(x, y).is_some() {
            return 0;
        }
        let m = &self.modifiers;
        match self.map.terrain(x, y) {
            Terrain::Forest => m.hunting_forest,
            Terrain::Grass => m.hunting_grass,
            Terrain::Dirt => m.hunting_dirt,
            Terrain::Water | Terrain::Mountain => 0,
        }
    }

    /// The gold a gathering building of `kind` at `tile` digs per payout, in thousandths – like TAB's mine: every tile of
    /// gold it works brings `gold_per_tile`, as much of it as the power allows (`share`, thousandths). Straight into
    /// the colony's gold, not the stock.
    pub fn gold_rate(&self, kind: BuildingKind, tile: (i32, i32), before: usize, share: u32) -> u64 {
        let per_tile = (kind.stats().gold_per_tile * 1000.0).round() as u64;
        if per_tile == 0 {
            return 0;
        }
        let gold = |&(x, y): &(i32, i32)| self.map.deposit(x, y).is_some_and(|d| d.ore == Ore::Gold);
        self.worked_tiles(kind, tile, before).iter().filter(|t| gold(t)).count() as u64 * per_tile * share as u64 / 1000
    }

    /// What comes into the stock per payout and what goes out of it, in thousandths per resource: the gathering
    /// buildings and the command center's food minus the wood the burners burn and the food the units and houses eat –
    /// what the HUD shows next to the stock.
    pub fn flow(&self, power: &Power) -> [i64; Resource::COUNT] {
        let mut flow = [0i64; Resource::COUNT];
        for (b, building) in self.buildings.iter().enumerate().filter(|(_, b)| b.works()) {
            let rate = self.gather_rate(building.kind, building.tile, b, power.share(self, b));
            for r in Resource::ALL {
                flow[r.index()] += rate[r.index()] as i64;
            }
            flow[Resource::Wood.index()] -= building.stats().burns_wood as i64 * 1000;
            flow[Resource::Food.index()] += (building.stats().food_made as i64 - building.stats().food as i64) * 1000;
        }
        flow[Resource::Food.index()] -= self.units_food() as i64 * 1000;
        flow
    }

    /// Food per payout for every living unit (not the ones in a production queue) – like their upkeep.
    fn units_food(&self) -> u32 {
        self.units.iter().map(|u| u.stats().food).sum()
    }

    /// Does a building of this kind stand – or is nothing `needed`? What `requires` asks for (like TAB's workshops).
    /// Switched off it doesn't count: otherwise a workshop could unlock everything without ever costing upkeep.
    pub fn stands(&self, needed: Option<BuildingKind>) -> bool {
        needed.is_none_or(|needed| self.buildings.iter().any(|b| b.kind == needed && !b.off && b.is_built()))
    }

    /// May `kind` be built – does what it `requires` stand?
    pub fn unlocked(&self, kind: BuildingKind) -> bool {
        self.stands(kind.stats().requires)
    }

    /// May `kind` be trained – does what it `requires` stand (firearms: the munitions factory)?
    pub fn unit_unlocked(&self, kind: UnitKind) -> bool {
        self.stands(kind.stats().requires)
    }

    /// Are wood, stone, coal and iron enough for `kind` – and for a barrel, is there one in the store (`blast.rs`)?
    /// (The gold is checked with `build_cost` – it can be less on walls.)
    pub fn can_afford_goods(&self, kind: BuildingKind) -> bool {
        let s = kind.stats();
        let enough = |r: Resource, n: u32| self.stock.get(r) >= n;
        let barrel = kind != BuildingKind::Barrel || self.barrels > 0;
        let goods = enough(Resource::Wood, s.cost_wood) && enough(Resource::Stone, s.cost_stone);
        goods && enough(Resource::Coal, s.cost_coal) && enough(Resource::Iron, s.cost_iron) && barrel
    }

    /// Are wood and iron in the stock enough to train `kind` (its gold and workers are checked on their own)?
    pub fn can_afford_unit_goods(&self, kind: UnitKind) -> bool {
        let s = kind.stats();
        self.stock.get(Resource::Wood) >= s.cost_wood && self.stock.get(Resource::Iron) >= s.cost_iron
    }

    /// Pays the wood, stone, coal and iron of `kind` – a barrel comes out of the store (checked before with
    /// `can_afford_goods`).
    pub(super) fn pay_goods(&mut self, kind: BuildingKind) {
        let s = kind.stats();
        self.stock.take(Resource::Wood, s.cost_wood);
        self.stock.take(Resource::Stone, s.cost_stone);
        self.stock.take(Resource::Coal, s.cost_coal);
        self.stock.take(Resource::Iron, s.cost_iron);
        if kind == BuildingKind::Barrel {
            self.barrels = self.barrels.saturating_sub(1);
        }
    }

    /// These units are dismissed (`Order::Dismiss`): `dismiss_refund_percent` of their wood and iron comes back,
    /// rounded up – half of 3 wood is 2 – up to what fits in the stock. Their gold doesn't.
    pub(super) fn refund_dismissed(&mut self, ids: &[u32]) {
        let percent = data::get().units.dismiss_refund_percent;
        let back = |cost: u32| (cost * percent).div_ceil(100);
        let (mut wood, mut iron) = (0, 0);
        for unit in self.units.iter().filter(|u| ids.contains(&u.id)) {
            let s = unit.stats();
            wood += back(s.cost_wood);
            iron += back(s.cost_iron);
        }
        self.stock.add(Resource::Wood, wood, self.storage(Resource::Wood));
        self.stock.add(Resource::Iron, iron, self.storage(Resource::Iron));
    }

    /// A charcoal burner takes the wood until the next payout – without it, it makes no power until then.
    /// Switched off or not finished it takes none.
    pub(super) fn fuel(&mut self, b: usize) {
        let building = &self.buildings[b];
        let burns = building.stats().burns_wood;
        if burns > 0 {
            let runs = building.works();
            self.buildings[b].fuelled = runs && self.stock.take(Resource::Wood, burns);
        }
    }

    /// Every foundry that works smelts iron (DESIGN.md decision 32): `smelts` per payout at full power, less with less –
    /// as much as the iron ore and the coal in the stock are enough for (`smelt_ore` and `smelt_coal` per iron). In
    /// the order of the list: when ore runs short, the older foundry gets it.
    fn smelt(&mut self, power: &Power) {
        for b in 0..self.buildings.len() {
            let building = &self.buildings[b];
            let stats = building.stats();
            if !building.works() || stats.smelts == 0 {
                continue;
            }
            let want = stats.smelts * power.share(self, b) / 1000;
            let ore = self.stock.get(Resource::IronOre) / stats.smelt_ore.max(1);
            let coal = self.stock.get(Resource::Coal) / stats.smelt_coal.max(1);
            let room = self.storage(Resource::Iron).saturating_sub(self.stock.get(Resource::Iron));
            let iron = want.min(ore).min(coal).min(room);
            self.stock.take(Resource::IronOre, iron * stats.smelt_ore);
            self.stock.take(Resource::Coal, iron * stats.smelt_coal);
            self.stock.add(Resource::Iron, iron, self.storage(Resource::Iron));
            self.buildings[b].smelted = iron;
        }
    }

    /// One payout: the banks take their interest, burners take their wood, gathering buildings and the command center
    /// bring in, the foundries smelt, the units eat, houses eat and pay (more around a market), the markets sell
    /// (`market.rs`), then the upkeep (`debt.rs`).
    pub(super) fn economy_payout(&mut self) {
        self.charge_interest();
        for b in 0..self.buildings.len() {
            self.fuel(b);
        }
        let power = self.power();
        for b in 0..self.buildings.len() {
            let building = &self.buildings[b];
            if building.kind.gathers().is_none() {
                continue;
            }
            if !building.works() {
                self.buildings[b].paid = 0;
                continue;
            }
            let share = power.share(self, b);
            let rate = self.gather_rate(building.kind, building.tile, b, share);
            for r in Resource::ALL {
                let limit = self.storage(r);
                self.stock.add_milli(r, rate[r.index()], limit);
            }
            // The gold a mine dug – it "pays" it like a house (`Building::paid`, the HUD's income).
            let dug = (self.gold_rate(building.kind, building.tile, b, share) / 1000) as u32;
            self.buildings[b].paid = dug;
            self.earn(dug);
        }
        // The foundries smelt what came in – before the markets could sell the ore.
        self.smelt(&power);
        // The command center's food, out of nothing.
        let made: u32 = self.buildings.iter().filter(|b| !b.infected && b.is_built()).map(|b| b.stats().food_made).sum();
        self.stock.add(Resource::Food, made, self.storage(Resource::Food));
        // The units eat first – what they eat the houses don't get. Not enough: they eat what there is, nothing else
        // happens to them.
        self.stock.take_up_to(Resource::Food, self.units_food());
        // Whoever pays gold: houses – their colonists eat first – and the command center, which eats nothing. Fed, they
        // pay their gold, as much of it as the power is enough for – a house around a market more (`market_bonus`).
        for b in 0..self.buildings.len() {
            let share = power.share(self, b);
            let building = &self.buildings[b];
            if building.infected || !building.is_built() || building.stats().gold == 0 {
                // A gathering building keeps what it dug above.
                if building.kind.gathers().is_none() {
                    self.buildings[b].paid = 0;
                }
                continue;
            }
            let food = building.stats().food;
            let fed = food == 0 || self.stock.take(Resource::Food, food);
            let paid = if fed { self.gold_paid(b, share) } else { 0 };
            let building = &mut self.buildings[b];
            building.fed = fed;
            building.paid = paid;
            self.earn(paid);
        }
        self.sell_surplus();
        self.pay_upkeep();
    }

    /// The gold building `b` pays when it's fed: its `gold`, as much of it as the power is enough for (`share`,
    /// thousandths) – a house around a market more (`market_bonus`).
    fn gold_paid(&self, b: usize, share: u32) -> u32 {
        self.buildings[b].full_income() * share / 1000 * (100 + self.market_bonus(b)) / 100
    }

    /// When the round starts nobody has paid yet – the HUD (gold of the last payout minus the upkeep) would show only
    /// the upkeep, as if the command center paid nothing. So every finished building counts with what it will pay at
    /// the first payout, as if fed.
    pub(super) fn expect_first_payout(&mut self) {
        let power = self.power();
        for b in 0..self.buildings.len() {
            let building = &self.buildings[b];
            let share = power.share(self, b);
            let dug = (self.gold_rate(building.kind, building.tile, b, share) / 1000) as u32;
            self.buildings[b].paid = if self.buildings[b].is_built() { self.gold_paid(b, share) + dug } else { 0 };
        }
    }
}
