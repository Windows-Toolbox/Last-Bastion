//! Buildings: placing and replacing them, production queues, towers with units inside, and what happens when one
//! falls. *Where* a building may go is in `placement.rs`.

use std::collections::VecDeque;

use macroquad::math::{vec2, Vec2};

use super::commands::Command;
use super::terrain::free_point_near;
use super::zombies::Breed;
use super::{World, HOUR_TICKS, TICKS_PER_SECOND};
use crate::data;
use crate::kinds::{BuildingKind, BuildingStats, UnitKind};

/// This many units fit into a building's production queue (like in StarCraft).
pub const MAX_PRODUCTION: usize = 5;
/// "No building on this tile" in `building_at`.
pub(super) const NO_BUILDING: u32 = u32::MAX;

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
    /// Points (hit points, then shield) that are paid for and still come back – a little every tick (`repair`).
    pub repair_left: u32,
    /// Production queue: in front the unit currently being built.
    pub production: VecDeque<UnitKind>,
    /// How many ticks the front unit has been built for.
    pub progress: u32,
    /// Where new units go (right click with the building selected).
    pub rally: Option<Rally>,
}

impl Building {
    pub fn stats(&self) -> &'static BuildingStats {
        self.kind.stats()
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

    /// A zombie hits it: first the shield, once that is gone the hit points. `true` if this hit broke the shield –
    /// then the building is infected now (`World::infect` lets the zombies out). What's left over of a hit that
    /// breaks the shield is lost, like in TAB a hit only ever goes to one of the two.
    pub fn take_damage(&mut self, damage: u32) -> bool {
        if self.shield == 0 {
            self.hp = self.hp.saturating_sub(damage);
            return false;
        }
        self.shield = self.shield.saturating_sub(damage);
        let broke = self.shield == 0 && self.stats().colonists > 0;
        self.infected |= broke;
        broke
    }

    /// Hit points and shield together, when it's whole.
    fn full(&self) -> u32 {
        self.stats().hp + self.stats().max_shield()
    }

    /// What's missing to whole and isn't paid for yet – what a repair would fix now.
    pub fn unpaid_damage(&self) -> u32 {
        // `saturating_sub`: never below 0 – even if `data\` was changed to fewer hit points than it has.
        self.full().saturating_sub(self.hp + self.shield + self.repair_left)
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

    /// Gold per hour: every colonist pays – as long as the building isn't infected.
    pub fn income(&self) -> u32 {
        if self.infected { 0 } else { self.stats().colonists * data::get().colony.gold_per_colonist_per_hour }
    }
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
            repair_left: 0,
            production: VecDeque::new(),
            progress: 0,
            rally: None,
        });
        // Block the map right away – the next building in the same tick must not go on the same spot.
        let size = kind.footprint(turned);
        self.map.set_occupied(tile.0, tile.1, size, true);
        // The gate: its middle tile is the door – open for your units.
        if kind == BuildingKind::Gate {
            self.map.set_door(tile.0 + size.0 / 2, tile.1 + size.1 / 2);
        }
        self.map_changed = true;
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

    /// Destroyed buildings: free their tiles, then recompute the index and the zombies' path.
    /// Whoever was in a fallen tower now stands on the ground where it was. Returns whether a command center fell –
    /// the round may be lost (`round.rs`).
    pub(super) fn remove_destroyed_buildings(&mut self) -> bool {
        let before = self.buildings.len();
        let mut command_center_fell = false;
        for building in &self.buildings {
            if building.hp == 0 {
                self.map.set_occupied(building.tile.0, building.tile.1, building.footprint(), false);
                for unit in self.units.iter_mut().filter(|u| u.inside == Some(building.id)) {
                    unit.inside = None;
                }
                command_center_fell |= building.kind == BuildingKind::CommandCenter;
            }
        }
        self.buildings.retain(|b| b.hp > 0);
        if self.buildings.len() != before {
            self.score.buildings_lost += (before - self.buildings.len()) as u32;
            self.refresh_map();
        }
        command_center_fell
    }

    /// Building `b` has just been infected: its colonists come out as zombies right next to it (DESIGN.md 6). For a
    /// tent with 6: 2–4 runners, the rest walkers. Not wandering – they go for the next target right away.
    /// From now on it belongs to the zombies: they don't hit it anymore, and its broken walls let everyone through
    /// (`Map::set_taken`) – so the threat map and their way to the command center are computed anew.
    pub(super) fn infect(&mut self, b: usize) {
        let (tile, size) = (self.buildings[b].tile, self.buildings[b].footprint());
        self.buildings[b].repair_left = 0; // a repair underway is lost with it
        self.map.set_taken(tile.0, tile.1, size, true);
        self.refresh_map();
        let colonists = self.buildings[b].stats().colonists;
        let runners = colonists.min(2 + self.rng.below(3));
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
        if ring.is_empty() {
            return; // walled in all around: they can't get out
        }
        for i in 0..colonists {
            let (x, y) = ring[self.rng.below(ring.len() as u32) as usize];
            let pos = vec2(x as f32 + self.rng.unit_f32(), y as f32 + self.rng.unit_f32());
            let breed = if i < runners { Breed::Runner } else { Breed::Walker };
            self.spawn_zombie_as(pos, breed, false);
        }
    }

    /// Gold per hour of all buildings together – what the HUD shows next to the gold.
    pub fn income(&self) -> u32 {
        self.buildings.iter().map(|b| b.income()).sum()
    }

    /// Every full hour of the round each building pays its gold (colonists × `gold_per_colonist_per_hour`,
    /// `data\colony.json`). Then repairs go on, and the production queues. Finished units come out next to the
    /// building.
    pub(super) fn update_buildings(&mut self) {
        if (self.tick + 1) % HOUR_TICKS == 0 {
            let income = self.income();
            self.gold += income;
            self.score.gold_earned += income;
        }
        for b in 0..self.buildings.len() {
            if self.buildings[b].repair() {
                self.cure(b);
            }
        }
        for b in 0..self.buildings.len() {
            let Some(&kind) = self.buildings[b].production.front() else {
                continue;
            };
            self.buildings[b].progress += 1;
            if self.buildings[b].progress >= kind.stats().build_time {
                self.buildings[b].production.pop_front();
                self.buildings[b].progress = 0;
                self.finish_unit(b, kind);
            }
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
        building.kind == BuildingKind::Tower && self.garrison(building.id).len() < data::get().buildings.tower_slots
    }
}
