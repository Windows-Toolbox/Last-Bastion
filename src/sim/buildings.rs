//! Buildings: placing and replacing them, production queues, towers with units inside, and what happens when one
//! falls. *Where* a building may go is in `placement.rs`.

use std::collections::VecDeque;

use macroquad::math::{vec2, Vec2};

use super::commands::Command;
use super::terrain::free_point_near;

use super::{payout_ticks, World, TICKS_PER_SECOND};
use crate::data;
use crate::kinds::{BuildingKind, BuildingStats, Resource, UnitKind};

/// This many units fit into a building's production queue (like in StarCraft).
pub const MAX_PRODUCTION: usize = 5;
/// "No building on this tile" in `building_at`.
pub(super) const NO_BUILDING: u32 = u32::MAX;

#[derive(Clone)]
pub struct Building {
    pub id: u32,
    pub kind: BuildingKind,
    /// The top left tile (smallest x and y).
    pub tile: (i32, i32),
    /// Turned by 90°: width and depth swap (only matters for the gate).
    pub turned: bool,
    pub hp: u32,
    /// What's left of the shield – zombies hit it first (`BuildingStats::max_shield`).
    pub shield: u32,
    /// The shield broke: its colonists came out as zombies, it pays no more gold, everyone walks through it.
    pub infected: bool,
    /// Switched off (Y, like in TAB – `BuildingKind::can_switch_off`): it does nothing and costs nothing – no upkeep,
    /// no workers, no power, and what it `requires` for others doesn't count either.
    pub off: bool,
    /// Ticks until it's finished (`World::start_construction`) – 0: it's done. Until then it stands and can be hit,
    /// takes its workers, and its hit points grow – but it does nothing yet (`works`).
    pub construction: u32,
    /// Points (hit points, then shield) that are paid for and still come back – a little every tick (`repair`).
    pub repair_left: u32,
    /// Production queue: in front what's being made right now – a unit, or a barrel in the munitions factory.
    pub production: VecDeque<Product>,
    /// How many ticks the front unit has been built for.
    pub progress: u32,
    /// Thousandths of a tick of building, collected while the power isn't enough: every tick it gets its power share
    /// (`Power::share`), and every full 1000 the front unit is built one tick further.
    charge: u32,
    /// A charcoal burner: did it get its wood for this hour (`World::fuel`)? Without it, it makes no power.
    pub fuelled: bool,
    /// A foundry: the iron it smelted at the last payout (`World::smelt`) – 0: no ore or coal, and it stands still.
    pub smelted: u32,
    /// A tower that shoots by itself (`sim/towers.rs`): thousandths of a tick until it can shoot again – every tick
    /// takes off its power share, so short of power it reloads slower.
    pub reload: u32,
    /// A house: did its colonists get their food at the last payout – and the gold they paid then (`economy_payout`;
    /// before the first payout what they will pay, `expect_first_payout`).
    pub fed: bool,
    pub paid: u32,
    /// Where new units go (right click with the building selected).
    pub rally: Option<Rally>,
    /// A barrel that went off (`blast.rs`) – it's gone at the end of the tick. Damage that takes all of a barrel's
    /// hit points at once must still set it off: that's why this is kept apart from `hp`.
    pub blown: bool,
    /// A market: does it sell (Q, `market.rs`) – and what it sold at the last payout: goods, and the gold for them.
    pub selling: bool,
    pub sold_goods: u32,
    pub sold_gold: u32,
}

/// What a production queue makes: a unit (barracks, inn) or a barrel (munitions factory, `blast.rs`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Product {
    Unit(UnitKind),
    Barrel,
}

impl Product {
    /// Its price in gold – paid when it goes into the queue, back in full if it's cancelled.
    pub fn cost(self) -> u32 {
        match self {
            Product::Unit(kind) => kind.stats().cost,
            Product::Barrel => data::get().buildings.blast.cost,
        }
    }

    /// The workers it takes, from the queue on (a barrel: none).
    pub fn worker_cost(self) -> u32 {
        match self {
            Product::Unit(kind) => kind.stats().worker_cost,
            Product::Barrel => 0,
        }
    }

    /// Ticks to make it, at full power.
    pub fn build_time(self) -> u32 {
        match self {
            Product::Unit(kind) => kind.stats().build_time,
            Product::Barrel => data::get().buildings.blast.build_time,
        }
    }
}

impl Building {
    pub fn stats(&self) -> &'static BuildingStats {
        self.kind.stats()
    }

    /// Finished building it (`construction` 0)? Until then it gives nothing: no workers, gold, power, storage …
    pub fn is_built(&self) -> bool {
        self.construction == 0
    }

    /// Does it do its job right now – finished, switched on, not infected? Gathering, power, gold, upkeep, unlocking.
    pub fn works(&self) -> bool {
        self.is_built() && !self.off && !self.infected
    }

    /// Width and depth in tiles, as it stands (turned or not).
    pub fn footprint(&self) -> (i32, i32) {
        self.kind.footprint(self.turned)
    }

    /// The top left corner in world coordinates.
    pub fn corner(&self) -> Vec2 {
        vec2(self.tile.0 as f32, self.tile.1 as f32)
    }

    /// The bottom right corner in world coordinates.
    pub fn far_corner(&self) -> Vec2 {
        let (w, h) = self.footprint();
        self.corner() + vec2(w as f32, h as f32)
    }

    pub fn center(&self) -> Vec2 {
        (self.corner() + self.far_corner()) / 2.0
    }

    /// How far a point is from the edge of the building (0 if it lies on it).
    pub fn distance_to(&self, p: Vec2) -> f32 {
        let (min, max) = (self.corner(), self.far_corner());
        (min - p).max(p - max).max(Vec2::ZERO).length()
    }

    /// All tiles the building stands on.
    pub fn tiles(&self) -> Vec<(i32, i32)> {
        let (w, h) = self.footprint();
        let (x0, y0) = self.tile;
        (y0..y0 + h).flat_map(|y| (x0..x0 + w).map(move |x| (x, y))).collect()
    }

    /// A zombie hits it – minus its `armor`, at least 1: first the shield, once that is gone the hit points. `true` if this hit broke the shield –
    /// then the building is infected now (`World::infect` lets the zombies out). What's left over of a hit that
    /// breaks the shield is lost, like in TAB a hit only ever goes to one of the two.
    pub fn take_damage(&mut self, damage: u32) -> bool {
        let damage = damage.saturating_sub(self.stats().armor).max(1);
        if self.shield == 0 {
            self.hp = self.hp.saturating_sub(damage);
            return false;
        }
        self.shield = self.shield.saturating_sub(damage);
        // The command center is never infected – its shield is just more life, it falls when its hit points are gone.
        let broke = self.shield == 0 && self.stats().colonists > 0 && self.kind != BuildingKind::CommandCenter;
        self.infected |= broke;
        broke
    }

    /// Hit points and shield together, when it's whole.
    fn full(&self) -> u32 {
        self.stats().hp + self.stats().max_shield()
    }

    /// What's missing to whole and isn't paid for yet – what a repair would fix now. Nothing while it's being built:
    /// it grows by itself, what a hit took then can be repaired once it's finished.
    pub fn unpaid_damage(&self) -> u32 {
        if !self.is_built() {
            return 0;
        }
        // `saturating_sub`: never below 0 – even if `data\` was changed to fewer hit points than it has.
        self.full().saturating_sub(self.hp + self.shield + self.repair_left)
    }

    /// One tick of building: its hit points grow towards full – what was built up to now, minus what was built up to
    /// the last tick, so the steps add up to exactly full and what a hit took stays missing. When it's finished the
    /// shield is there. Returns `true` on the tick it's finished.
    fn build_step(&mut self) -> bool {
        if self.construction == 0 {
            return false;
        }
        let stats = self.stats();
        let start = stats.hp * data::get().buildings.build_start_percent / 100;
        let total = stats.build_time.max(1);
        // Hit points built when `left` ticks are left – a share of what's between the start and full.
        let grown = |left: u32| stats.hp.saturating_sub(start) * total.saturating_sub(left) / total;
        let before = grown(self.construction);
        self.construction -= 1;
        self.hp = (self.hp + grown(self.construction) - before).min(stats.hp);
        if self.construction == 0 {
            self.shield = stats.max_shield();
            return true;
        }
        false
    }

    /// What repairing it costs right now: its share of `repair_cost`, rounded up – a scratch never costs nothing.
    /// `u64` in between: the product of two `u32` could get too big for one.
    pub fn repair_price(&self) -> u32 {
        let damage = self.unpaid_damage() as u64;
        (self.stats().repair_cost as u64 * damage).div_ceil(self.full() as u64) as u32
    }

    /// One tick of repairing: back come the hit points first, then the shield – so much per tick that a full repair
    /// takes `repair_seconds`. Returns `true` if this made an infected building whole: then it's cured.
    fn repair(&mut self) -> bool {
        if self.repair_left == 0 {
            return false;
        }
        let ticks = (data::get().buildings.repair_seconds * TICKS_PER_SECOND).max(1.0) as u32;
        let mut step = self.full().div_ceil(ticks).min(self.repair_left);
        self.repair_left -= step;
        let hp = step.min(self.stats().hp - self.hp);
        self.hp += hp;
        step -= hp;
        self.shield = (self.shield + step).min(self.stats().max_shield());
        if self.hp + self.shield == self.full() {
            self.repair_left = 0; // whole – nothing left to do (it can only be left over if it was hit meanwhile)
            return std::mem::take(&mut self.infected);
        }
        false
    }

    /// Does it have people in it – what zombies go for when they see it (TAB's `HasPeople`)? The command center, a
    /// house, a building with workers or one that gives them, a tower with units inside (`holds_units`) – not an
    /// infected one.
    pub fn has_people(&self, holds_units: bool) -> bool {
        let s = self.stats();
        let people = s.colonists > 0 || s.workers > 0 || s.worker_cost > 0;
        !self.zombies_ignore() && (self.kind == BuildingKind::CommandCenter || people || holds_units)
    }

    /// Do the zombies leave it alone – infected (it's theirs) or a trap (they walk over it)? Then they don't notice
    /// it, don't hit it, and everyone walks through (`Map::set_taken`).
    pub fn zombies_ignore(&self) -> bool {
        self.infected || self.kind.is_trap()
    }

    /// Gold per payout when it's fed and has all its power: its `gold` – as long as the building isn't infected. What it really paid last hour: `paid`.
    pub fn full_income(&self) -> u32 {
        if self.infected { 0 } else { self.stats().gold }
    }
}

/// What keeps a switched off building from going on again (`World::switch_on_blocked`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SwitchBlock {
    Workers,
    /// Not enough spare power in its grid – or no grid reaches it.
    Power,
    /// The houses' gold doesn't cover the upkeep with it.
    Income,
}

/// What a repair of some buildings would do right now (`World::repair_plan`) – the buttons show it, the UI sends it.
pub struct RepairPlan {
    /// The buildings it repairs (indices): cheapest first, as many as the gold pays for.
    pub chosen: Vec<usize>,
    /// What all of them that may be repaired would cost together – more than the gold: not all get repaired.
    pub price: u32,
    /// How many have anything to repair at all, and how many of those may not be repaired now (`can_repair`).
    pub damaged: usize,
    pub blocked: usize,
}

/// Rally point of a building: a point, or a unit that new units follow (DESIGN.md 7.1).
#[derive(Clone, Copy)]
pub enum Rally {
    Point(Vec2),
    Unit(u32),
}

impl World {
    /// The units in this tower (indices into `units`).
    pub fn garrison(&self, building: u32) -> Vec<usize> {
        (0..self.units.len()).filter(|&i| self.units[i].inside == Some(building)).collect()
    }

    /// Where is the building with this `id` in the list? `None` if it's destroyed.
    pub fn find_building(&self, id: u32) -> Option<usize> {
        self.buildings.iter().position(|b| b.id == id)
    }

    /// The building whose tiles this point lies on.
    pub fn building_at_point(&self, p: Vec2) -> Option<usize> {
        let (x, y) = (p.x.floor() as i32, p.y.floor() as i32);
        self.building_on_tile(x, y)
    }

    pub(super) fn building_on_tile(&self, x: i32, y: i32) -> Option<usize> {
        let size = self.map.size as i32;
        if x < 0 || y < 0 || x >= size || y >= size {
            return None;
        }
        match self.building_at[(y * size + x) as usize] {
            NO_BUILDING => None,
            b => Some(b as usize),
        }
    }

    pub(super) fn add_building(&mut self, kind: BuildingKind, tile: (i32, i32), turned: bool) {
        self.next_building_id += 1;
        self.buildings.push(Building {
            id: self.next_building_id,
            kind,
            tile,
            turned,
            hp: kind.stats().hp,
            shield: kind.stats().max_shield(),
            infected: false,
            off: false,
            construction: 0,
            repair_left: 0,
            production: VecDeque::new(),
            progress: 0,
            charge: 0,
            fuelled: false,
            smelted: 0,
            reload: 0,
            fed: true,
            paid: 0,
            rally: None,
            blown: false,
            selling: true,
            sold_goods: 0,
            sold_gold: 0,
        });
        // Block the map right away – the next building in the same tick must not go on the same spot.
        let size = kind.footprint(turned);
        self.map.set_occupied(tile.0, tile.1, size, true);
        // The gate: its middle tile is the door – open for your units.
        if kind == BuildingKind::Gate {
            self.map.set_door(tile.0 + size.0 / 2, tile.1 + size.1 / 2);
        }
        // A trap lies on the ground: everyone walks over it, like over an infected building.
        if kind.is_trap() {
            self.map.set_taken(tile.0, tile.1, size, true);
        }
        self.map_changed = true;
    }

    /// The building just placed (`b`, by a player – the editor's and the generator's stand at once) builds itself
    /// first, like in TAB: `build_seconds` long (`data\buildings.json`), from `build_start_percent` of its hit points
    /// up, without a shield. Built every tick in `update_buildings`, power or not.
    pub(super) fn start_construction(&mut self, b: usize) {
        #[cfg(test)]
        if self.instant_build {
            return;
        }
        let building = &mut self.buildings[b];
        let stats = building.stats();
        if stats.build_time == 0 {
            return;
        }
        building.construction = stats.build_time;
        building.hp = (stats.hp * data::get().buildings.build_start_percent / 100).max(1);
        building.shield = 0;
    }

    /// Before building: whatever the new building replaces (a gate on walls, `placement::replaces`) is torn down –
    /// its tiles are free again. `remove` instead of `swap_remove`, so the order of the list stays the same on every PC.
    pub(super) fn replace_buildings(&mut self, kind: BuildingKind, tile: (i32, i32), turned: bool) {
        for id in self.replaced_by(kind, tile, turned) {
            self.take_down(id);
        }
    }

    /// The building with this `id` goes away (built over, or the editor's eraser): its tiles are free again, whoever
    /// was in it stands on the ground. Recomputed after the orders of the tick (`map_changed`).
    pub(super) fn take_down(&mut self, id: u32) {
        if let Some(b) = self.find_building(id) {
            let old = self.buildings.remove(b);
            self.map.set_occupied(old.tile.0, old.tile.1, old.footprint(), false);
            for unit in self.units.iter_mut().filter(|u| u.inside == Some(old.id)) {
                unit.inside = None;
            }
            self.map_changed = true;
        }
    }

    /// May the player tear building `b` down (Del)? Not the command center – losing it loses the round – and not an
    /// infected one: it belongs to the zombies now (repair it or shoot it).
    pub fn can_demolish(&self, b: usize) -> bool {
        let building = &self.buildings[b];
        building.kind != BuildingKind::CommandCenter && !building.infected
    }

    /// What tearing building `b` down gives back: `demolish_refund_percent` of its price (`data\buildings.json`), of
    /// one still being built `demolish_unfinished_refund_percent` – gold, then wood, stone, coal, iron (rounded down).
    pub fn demolish_refund(&self, b: usize) -> [u32; 5] {
        let building = &self.buildings[b];
        let data = &data::get().buildings;
        let percent = if building.is_built() { data.demolish_refund_percent } else { data.demolish_unfinished_refund_percent };
        let s = building.stats();
        [s.cost, s.cost_wood, s.cost_stone, s.cost_coal, s.cost_iron].map(|n| n * percent / 100)
    }

    /// The player tears building `id` down (`Order::Demolish`, like TAB): it goes at once, part of its price comes back
    /// (`demolish_refund`) – as much as the stock and the gold storage take, like any income. A barrel goes back into
    /// the store if there is room. Its workers are free again (they're counted from what stands), units in a tower
    /// stand on the ground.
    pub(super) fn demolish(&mut self, id: u32) {
        let Some(b) = self.find_building(id).filter(|&b| self.can_demolish(b)) else {
            return;
        };
        let [gold, wood, stone, coal, iron] = self.demolish_refund(b);
        self.gold = (self.gold + gold).min(self.gold.max(self.gold_storage()));
        for (r, n) in [(Resource::Wood, wood), (Resource::Stone, stone), (Resource::Coal, coal), (Resource::Iron, iron)] {
            self.stock.add(r, n, self.storage(r));
        }
        if self.buildings[b].kind == BuildingKind::Barrel && self.barrels < self.barrel_room() {
            self.barrels += 1;
        }
        self.take_down(id);
    }

    /// Destroyed buildings: free their tiles, then recompute the index and the zombies' path.
    /// Whoever was in a fallen tower now stands on the ground where it was. Every fallen building makes noise
    /// (`building_destroyed`), and one with workers in it is infected as it falls, like in TAB: they come out as zombies
    /// where it stood – and that is loud (`infected_come_out`). An overrun hunter cottage far out brings the horde.
    /// Returns whether a command center fell – the round may be lost (`round.rs`).
    pub(super) fn remove_destroyed_buildings(&mut self) -> bool {
        let before = self.buildings.len();
        // A barrel that went off did its job – it isn't counted as lost.
        let blown = self.buildings.iter().filter(|b| b.hp == 0 && b.blown).count();
        let mut command_center_fell = false;
        // (middle, its tiles, its workers) of every fallen building – they come out after the list is cleaned up.
        let mut fallen = Vec::new();
        for building in &self.buildings {
            if building.hp == 0 {
                self.map.set_occupied(building.tile.0, building.tile.1, building.footprint(), false);
                for unit in self.units.iter_mut().filter(|u| u.inside == Some(building.id)) {
                    unit.inside = None;
                }
                command_center_fell |= building.kind == BuildingKind::CommandCenter;
                // Switched off or not finished yet, nobody works in it – nobody comes out.
                let workers = if building.works() { building.stats().worker_cost } else { 0 };
                fallen.push((building.center(), building.tiles(), workers));
            }
        }
        self.buildings.retain(|b| b.hp > 0);
        if self.buildings.len() != before {
            self.score.buildings_lost += (before - self.buildings.len() - blown) as u32;
            self.refresh_map();
        }
        for (center, tiles, workers) in fallen {
            self.make_sound(center, data::get().sound.building_destroyed);
            let free: Vec<(i32, i32)> = tiles.into_iter().filter(|&(x, y)| !self.map.is_blocked(x, y)).collect();
            self.infected_come_out(center, &free, workers);
        }
        command_center_fell
    }

    /// Building `b` has just been infected: its people come out as zombies right next to it (DESIGN.md 6) – like TAB's,
    /// as many as live and work in it beyond the workers it gives, plus its `extra_infected`. From now on it belongs to
    /// the zombies: they don't hit it anymore, and its broken walls let everyone through (`Map::set_taken`) – so their
    /// way to the command center is computed anew. Still being built, it falls (TAB).
    pub(super) fn infect(&mut self, b: usize) {
        let (tile, size) = (self.buildings[b].tile, self.buildings[b].footprint());
        self.buildings[b].repair_left = 0; // a repair underway is lost with it
        self.map.set_taken(tile.0, tile.1, size, true);
        self.refresh_map();
        // Free tiles in the ring around the building – for zombies, so not on walls or water.
        let building = &self.buildings[b];
        let (w, h) = building.footprint();
        let (x0, y0) = building.tile;
        let mut ring = Vec::new();
        for y in y0 - 1..=y0 + h {
            for x in x0 - 1..=x0 + w {
                let on_ring = x == x0 - 1 || x == x0 + w || y == y0 - 1 || y == y0 + h;
                if on_ring && !self.map.is_blocked(x, y) {
                    ring.push((x, y));
                }
            }
        }
        let s = building.stats();
        let people = (s.worker_cost + s.colonists).saturating_sub(s.workers) + s.extra_infected;
        let (center, unfinished) = (building.center(), !building.is_built());
        self.infected_come_out(center, &ring, people);
        if unfinished {
            self.buildings[b].hp = 0;
        }
    }

    /// `count` infected people of a building at `center` come out as zombies, on random spots of these free `tiles` –
    /// each one of TAB's infected (`infected_breed`), and all of them head for the command center like a wave (TAB's
    /// `AttackCommandCenter`). And it's loud, like in TAB: `building_infected` plus `per_infected` for every one of them
    /// (`sound.rs`). No tile free (walled in all around): they can't get out – but it's still heard.
    fn infected_come_out(&mut self, center: Vec2, tiles: &[(i32, i32)], count: u32) {
        if count == 0 {
            return;
        }
        let sound = &data::get().sound;
        self.make_sound(center, sound.building_infected + sound.per_infected * count);
        if tiles.is_empty() {
            return;
        }
        for _ in 0..count {
            let (x, y) = tiles[self.rng.below(tiles.len() as u32) as usize];
            let pos = vec2(x as f32 + self.rng.unit_f32(), y as f32 + self.rng.unit_f32());
            let breed = self.infected_breed();
            self.spawn_zombie_as(pos, breed, false);
        }
    }

    /// The gold all buildings paid at the last payout – what the HUD shows next to the gold. Infected since then:
    /// nothing.
    pub fn income(&self) -> u32 {
        self.buildings.iter().filter(|b| !b.infected).map(|b| b.paid).sum()
    }

    /// Workers like in TAB (DESIGN.md 5.6): houses and the command center give them (`workers`), every other building
    /// and every unit takes some (`worker_cost`) – for as long as it stands or lives. Units in a production queue
    /// already count: they take them when they go in, like their gold. Infected buildings give none, switched off
    /// ones take none. A building still being built takes them already, a house gives them only once it's finished.
    /// Counted anew every time instead of kept in a number – that way they can't get out of step with what stands on
    /// the map, and map files don't need them. Returns (in use, all there are).
    pub fn workers(&self) -> (u32, u32) {
        let buildings = self.buildings.iter().filter(|b| !b.off).map(|b| b.stats().worker_cost);
        let queued = self.buildings.iter().flat_map(|b| b.production.iter().map(|p| p.worker_cost()));
        let units = self.units.iter().map(|u| u.stats().worker_cost);
        let used = buildings.chain(queued).chain(units).sum();
        let all: u32 = self.buildings.iter().filter(|b| !b.infected && b.is_built()).map(|b| b.stats().workers).sum();
        #[cfg(test)]
        let all = all + self.extra_workers;
        (used, all)
    }

    /// Workers nobody uses – what a new building or unit may take. 0 if more are in use than there are (a house was
    /// infected): then nothing new until it's repaired or another house stands.
    pub fn free_workers(&self) -> u32 {
        let (used, all) = self.workers();
        all.saturating_sub(used)
    }

    /// Every payout (`payout_ticks`) the economy does its round (`economy.rs`: gathering, burning, eating, the houses'
    /// gold, the upkeep). Then buildings being built grow, repairs go on, and the production queues – as fast as the
    /// power allows, not at all while switched off. Finished units come out next to the building, finished barrels go
    /// into the store – with the store full, the next one waits.
    pub(super) fn update_buildings(&mut self) {
        for b in 0..self.buildings.len() {
            // Finished: a charcoal burner lights up right away if there's wood, like a new one did before.
            if self.buildings[b].build_step() {
                self.fuel(b);
            }
        }
        if (self.tick + 1).is_multiple_of(payout_ticks()) {
            self.economy_payout();
        }
        if (self.tick + 1).is_multiple_of(data::get().buildings.trap_every.max(1) as u64) {
            self.spring_traps();
        }
        if (self.tick + 1).is_multiple_of(data::get().sound.work_every.max(1) as u64) {
            self.machines_work(); // sound.rs
        }
        for b in 0..self.buildings.len() {
            if self.buildings[b].repair() {
                self.cure(b);
            }
        }
        // The grids only when something is being trained – most ticks nothing is.
        let power = self.buildings.iter().any(|b| !b.production.is_empty()).then(|| self.power());
        for b in 0..self.buildings.len() {
            let Some(&product) = self.buildings[b].production.front().filter(|_| !self.buildings[b].off) else {
                continue;
            };
            if product == Product::Barrel && self.barrels >= self.barrel_room() {
                continue;
            }
            let share = power.as_ref().map_or(1000, |p| p.share(self, b));
            let building = &mut self.buildings[b];
            building.charge += share;
            if building.charge < 1000 {
                continue;
            }
            building.charge -= 1000;
            building.progress += 1;
            if self.buildings[b].progress >= product.build_time() {
                self.buildings[b].production.pop_front();
                self.buildings[b].progress = 0;
                match product {
                    Product::Unit(kind) => self.finish_unit(b, kind),
                    Product::Barrel => self.barrels += 1,
                }
            }
        }
    }

    /// Switches building `b` off or on again (`Order::SwitchOff`). On again only if nothing blocks it
    /// (`switch_on_blocked`) – otherwise it stays off. A charcoal burner switched on lights up right away if there's
    /// wood, like a new one.
    pub(super) fn switch_off(&mut self, b: usize, off: bool) {
        let building = &self.buildings[b];
        if !building.kind.can_switch_off() || building.infected || !building.is_built() || building.off == off {
            return;
        }
        if !off && self.switch_on_blocked(b).is_some() {
            return;
        }
        self.buildings[b].off = off;
        if !off {
            self.fuel(b);
        }
    }

    /// Why switched off building `b` can't go on again right now – `None`: it can. It needs its workers free, the
    /// power it takes spare in the grid where it stands, and as much gold coming in as everything costs with it
    /// (the gold of the last payout ≥ upkeep now + its own).
    pub fn switch_on_blocked(&self, b: usize) -> Option<SwitchBlock> {
        let building = &self.buildings[b];
        let stats = building.stats();
        if stats.worker_cost > self.free_workers() {
            return Some(SwitchBlock::Workers);
        }
        // Switched off it's in no grid – `spare_power_at` finds the one reaching it.
        if stats.energy_use > 0 && self.spare_power_at(&self.power(), building.center()).map_or(0, |(_, spare)| spare) < stats.energy_use {
            return Some(SwitchBlock::Power);
        }
        if stats.upkeep > 0 && self.income() < self.upkeep() + stats.upkeep {
            return Some(SwitchBlock::Income);
        }
        None
    }

    /// Every trap hurts every zombie standing on it (minus its armor, at least 1), and every hit wears the trap down by
    /// `trap_wear` – at 0 hit points it's gone like any destroyed building (`remove_destroyed_buildings`).
    fn spring_traps(&mut self) {
        if !self.buildings.iter().any(|b| b.kind.is_trap()) {
            return;
        }
        for z in 0..self.zombies.len() {
            let pos = self.zombies[z].pos;
            let Some(b) = self.building_at_point(pos) else {
                continue;
            };
            let trap = &mut self.buildings[b];
            let stats = trap.stats();
            if !trap.kind.is_trap() || trap.hp == 0 || !self.zombies[z].is_alive() {
                continue;
            }
            trap.hp = trap.hp.saturating_sub(stats.trap_wear);
            let zombie = &mut self.zombies[z];
            let damage = stats.trap_damage.saturating_sub(zombie.breed.stats().armor).max(1);
            zombie.hp = zombie.hp.saturating_sub(damage);
        }
    }

    /// Building `b` was infected and is whole again: it's yours again – closed for everyone (whoever stands in it is
    /// pushed out), something for the zombies to hit, and it pays gold. Its colonists are back (a repair brings new
    /// ones). Everyone's ways are computed anew.
    fn cure(&mut self, b: usize) {
        let (tile, size) = (self.buildings[b].tile, self.buildings[b].footprint());
        self.map.set_taken(tile.0, tile.1, size, false);
        self.push_out_of_terrain();
        self.reroute_units();
        self.refresh_map();
    }

    /// Which of these buildings (indices) a repair fixes right now: the damaged ones that may be repaired
    /// (`can_repair`), cheapest first, as many as the gold pays for – that way the most buildings get repaired.
    pub fn repair_plan(&self, candidates: impl IntoIterator<Item = usize>) -> RepairPlan {
        let (mut damaged, mut blocked) = (0, 0);
        // (price, id, index): sorted, the cheapest comes first, the id decides between equal prices.
        let mut repairable: Vec<(u32, u32, usize)> = Vec::new();
        for b in candidates {
            if self.buildings[b].unpaid_damage() == 0 {
                continue;
            }
            damaged += 1;
            if !self.can_repair(b) {
                blocked += 1;
                continue;
            }
            repairable.push((self.buildings[b].repair_price(), self.buildings[b].id, b));
        }
        repairable.sort_unstable();
        let price = repairable.iter().map(|&(p, _, _)| p).sum();
        let mut gold = self.gold;
        let mut chosen = Vec::new();
        for (p, _, b) in repairable {
            if p <= gold {
                gold -= p;
                chosen.push(b);
            }
        }
        RepairPlan { chosen, price, damaged, blocked }
    }

    /// Pays for repairing building `b`, if it may be repaired now (`can_repair`) and the gold is enough: what's missing
    /// then comes back over the next seconds. Returns whether it was paid (and so whether it repairs now).
    pub(super) fn start_repair(&mut self, b: usize) -> bool {
        let price = self.buildings[b].repair_price();
        let damage = self.buildings[b].unpaid_damage();
        if damage == 0 || price > self.gold || !self.can_repair(b) {
            return false;
        }
        self.gold -= price;
        self.score.gold_spent += price;
        self.buildings[b].repair_left += damage;
        true
    }

    /// The free tile around a building that is closest to `toward` (its middle) – where units come out of it.
    /// Built in all around: just somewhere nearby. `None` if nothing is free far and wide.
    pub(super) fn exit_point(&self, b: usize, toward: Vec2) -> Option<Vec2> {
        let building = &self.buildings[b];
        let (w, h) = building.footprint();
        let (x0, y0) = building.tile;
        let mut exit: Option<(f32, Vec2)> = None;
        for y in y0 - 1..=y0 + h {
            for x in x0 - 1..=x0 + w {
                let ring = x == x0 - 1 || x == x0 + w || y == y0 - 1 || y == y0 + h;
                if !ring || self.map.blocks_unit(x, y) {
                    continue;
                }
                let center = vec2(x as f32 + 0.5, y as f32 + 0.5);
                let d = center.distance(toward);
                if exit.is_none_or(|(best, _)| d < best) {
                    exit = Some((d, center));
                }
            }
        }
        exit.map(|(_, p)| p).or_else(|| free_point_near(&self.map, building.center()))
    }

    fn finish_unit(&mut self, b: usize, kind: UnitKind) {
        let building = &self.buildings[b];
        // Where the unit should go: to the rally point – without one simply out at the bottom.
        let rally = building.rally;
        let toward = match rally {
            Some(Rally::Point(p)) => p,
            Some(Rally::Unit(id)) => self.find_unit(id).map_or(building.center(), |u| self.units[u].pos),
            None => building.center() + vec2(0.0, building.footprint().1 as f32),
        };
        let Some(pos) = self.exit_point(b, toward) else {
            return;
        };
        self.spawn_unit(kind, pos);
        self.score.units_trained += 1;
        let id = self.next_unit_id;
        match rally {
            Some(Rally::Point(p)) => self.command(&[id], Command::Move(p), false, None),
            Some(Rally::Unit(target)) => self.command(&[id], Command::Follow(target), false, None),
            None => {}
        }
    }

    /// Is this building a tower with a free spot?
    pub(super) fn has_room(&self, b: usize) -> bool {
        let building = &self.buildings[b];
        building.kind.is_tower() && building.is_built() && self.garrison(building.id).len() < data::get().buildings.tower_slots
    }
}
