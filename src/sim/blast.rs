//! Explosive barrels (DESIGN.md 6, "Defence"). The munitions factory makes them (`Order::MakeBarrel`, its production
//! queue) and keeps them in its store (`World::barrels` – `barrel_storage` per factory). From there:
//! - **set** from the defence menu like a stake trap, around the base – one from the store each,
//! - or **carried**: right click the munitions factory with units, each takes one – slower on foot, and it doesn't
//!   shoot while it carries – and puts it down where it stands (D, `drop_barrels`), wherever that is.
//!
//! A barrel on the ground goes off at the **first damage** – a zombie hitting it, acid, the blast of another barrel.
//! A unit carrying one goes off **when it dies**. A blast hurts everyone within `radius`: zombies, your units,
//! buildings, houses of doom – and it's loud (`noise`, `sound.rs`). One blast sets off the next: a row of barrels goes
//! up all at once. Values in `data\buildings.json` ("blast").

use macroquad::math::Vec2;

use super::buildings::{Product, MAX_PRODUCTION};
use super::World;
use crate::data;
use crate::kinds::{BuildingKind, Resource};

/// How long a blast is drawn (ticks) – 0.6 s.
const SHOWN: u32 = 12;

/// A blast, for drawing: where, and how many ticks it's still shown.
#[derive(Clone, Copy)]
pub struct Blast {
    pub at: Vec2,
    pub left: u32,
}

impl World {
    /// How many barrels the stores hold together: `barrel_storage` of every munitions factory that runs.
    pub fn barrel_room(&self) -> u32 {
        self.buildings.iter().filter(|b| !b.off && !b.infected).map(|b| b.stats().barrel_storage).sum()
    }

    /// A barrel into the queue of munitions factory `id` – if there's room in the queue and the gold and coal pay for
    /// it (the gold also from the credit, with Shift). Paid right away, like a unit.
    pub(super) fn make_barrel(&mut self, id: u32) {
        let Some(b) = self.find_building(id) else {
            return;
        };
        let blast = &data::get().buildings.blast;
        let factory = &self.buildings[b];
        let room = factory.kind.makes_barrels() && !factory.infected && factory.production.len() < MAX_PRODUCTION;
        if room && self.spendable() >= blast.cost && self.stock.get(Resource::Coal) >= blast.cost_coal {
            self.stock.take(Resource::Coal, blast.cost_coal);
            self.score.gold_spent += blast.cost;
            self.pay_gold(blast.cost);
            self.buildings[b].production.push_back(Product::Barrel);
        }
    }

    /// Can unit `i` fetch a barrel at building `b` – a munitions factory with one in the store, and its hands free?
    pub(super) fn can_fetch(&self, i: usize, b: usize) -> bool {
        self.buildings[b].kind.makes_barrels() && !self.buildings[b].infected && self.barrels > 0 && !self.units[i].barrel
    }

    /// Unit `i` takes a barrel from the store.
    pub(super) fn fetch(&mut self, i: usize) {
        self.barrels -= 1;
        self.units[i].barrel = true;
    }

    /// These units put their barrels down on the tile they stand on – where nothing else is (they are pushed aside, like
    /// from any new building). Unlike building there may be zombies close by: that's what they're for.
    pub(super) fn drop_barrels(&mut self, ids: &[u32]) {
        for &id in ids {
            let Some(i) = self.find_unit(id) else {
                continue;
            };
            let unit = &self.units[i];
            let (x, y) = (unit.pos.x.floor() as i32, unit.pos.y.floor() as i32);
            if !unit.barrel || unit.is_inside() || !self.map.is_buildable(x, y) {
                continue;
            }
            self.units[i].barrel = false;
            self.add_building(BuildingKind::Barrel, (x, y), false);
        }
    }

    /// Every tick after the units: barrels that took damage and units with a barrel that died go off – and every blast
    /// can set off more, round after round, until none is left. In the order of the lists – the same on every PC. The
    /// dead are taken away at the end of the tick (`remove_dead`).
    pub(super) fn detonate(&mut self) {
        self.blasts.retain_mut(|blast| {
            blast.left -= 1;
            blast.left > 0
        });
        loop {
            let mut going_off = Vec::new();
            for building in self.buildings.iter_mut().filter(|b| b.kind == BuildingKind::Barrel && !b.blown) {
                if building.hp < building.stats().hp {
                    building.hp = 0;
                    building.blown = true;
                    going_off.push(building.center());
                }
            }
            for unit in self.units.iter_mut().filter(|u| u.barrel && u.hp == 0) {
                unit.barrel = false;
                going_off.push(unit.pos);
            }
            if going_off.is_empty() {
                return;
            }
            for at in going_off {
                self.blast(at);
            }
        }
    }

    /// One barrel goes off at `at`: everyone within `radius` takes `damage` – zombies and your units minus their armor
    /// (at least 1; units in a tower are safe, the tower isn't), buildings like a hit (a broken shield infects), houses
    /// of doom like a shot (more come out). Loud.
    fn blast(&mut self, at: Vec2) {
        let blast = &data::get().buildings.blast;
        let (damage, radius) = (blast.damage, blast.radius);
        // Whoever sleeps there wakes – and gets hit (`sleep.rs`).
        self.wake_area(at, radius);
        for zombie in self.zombies.iter_mut().filter(|z| z.is_alive() && z.pos.distance(at) <= radius) {
            zombie.hp = zombie.hp.saturating_sub(damage.saturating_sub(zombie.breed.stats().armor).max(1));
        }
        for unit in self.units.iter_mut().filter(|u| !u.is_inside() && u.hp > 0 && u.pos.distance(at) <= radius) {
            unit.hp = unit.hp.saturating_sub(damage.saturating_sub(unit.stats().armor).max(1));
        }
        let mut infected = Vec::new();
        for (b, building) in self.buildings.iter_mut().enumerate() {
            if building.hp > 0 && building.distance_to(at) <= radius && building.take_damage(damage) {
                infected.push(b);
            }
        }
        for b in infected {
            self.infect(b);
        }
        let houses: Vec<usize> = (0..self.doom.len())
            .filter(|&d| self.doom[d].hp > 0 && self.doom[d].closest_point(at).distance(at) <= radius)
            .collect();
        for d in houses {
            self.hit_house(d, damage, at);
        }
        self.make_colony_sound(at, blast.noise);
        self.blasts.push(Blast { at, left: SHOWN });
    }
}
