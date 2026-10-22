//! The command center's scan, like StarCraft's: click anywhere on the map, and a circle of it is shown for a while –
//! zombies, villages, loot – over forests too. It makes no noise.
//!
//! `charges` at most, a new one every `every_seconds` – and while one charges, the command center takes `power` from
//! its grid (`World::power`). Early on that's nearly all the power there is, so a scan costs the colony something: you
//! don't scan all the time. Values in `data\buildings.json` ("scan").

use macroquad::math::Vec2;

use super::World;
use crate::data;
use crate::kinds::BuildingKind;

/// A scan being shown right now: around where, until which tick.
#[derive(Clone, Copy)]
pub struct Scanned {
    pub at: Vec2,
    pub until: u64,
}

impl World {
    /// The command center charging the next scan (its index) – `None` while all charges are there, or none stands.
    pub fn scan_charger(&self) -> Option<usize> {
        if self.scan_charges >= data::get().buildings.scan.charges {
            return None;
        }
        self.buildings.iter().position(|b| b.kind == BuildingKind::CommandCenter && !b.infected)
    }

    /// Seconds until the next charge is there – `None` while all of them are.
    pub fn next_scan_in(&self) -> Option<u32> {
        self.scan_charger()?;
        let every = data::get().buildings.scan.every;
        Some(every.saturating_sub(self.scan_charge).div_ceil(super::TICKS_PER_SECOND as u32))
    }

    /// A scan around `at` – if there's a charge and a command center. Seen at once, also while paused.
    pub(super) fn scan(&mut self, at: Vec2) {
        let standing = self.buildings.iter().any(|b| b.kind == BuildingKind::CommandCenter && !b.infected);
        if self.scan_charges == 0 || !standing {
            return;
        }
        self.scan_charges -= 1;
        let until = self.tick + data::get().buildings.scan.time as u64;
        self.scans.push(Scanned { at, until });
        // Shown: whoever sleeps there is awake now (`sleep.rs`) – and fades in.
        self.wake_area(at, data::get().buildings.scan.radius);
        self.update_vision();
    }

    /// Every tick: scans that are over go, and the next charge charges – always at the same speed, but it costs the
    /// command center's grid power meanwhile.
    pub(super) fn update_scan(&mut self) {
        let tick = self.tick;
        self.scans.retain(|s| s.until > tick);
        if self.scan_charger().is_none() {
            self.scan_charge = 0;
            return;
        }
        self.scan_charge += 1;
        if self.scan_charge >= data::get().buildings.scan.every.max(1) {
            self.scan_charge = 0;
            self.scan_charges += 1;
        }
    }
}
