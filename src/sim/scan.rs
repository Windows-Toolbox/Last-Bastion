//! The radar's scan, like StarCraft's: click anywhere on the map, and a circle of it is shown for a while – zombies,
//! villages, loot – over forests too. It makes no noise. Only with a radar that works (TAB's, researched in the
//! foundry) – the command center has none.
//!
//! `charges` at most, a new one every `every_seconds` – none at the start, a radar charges them – and while one
//! charges, the radar takes `power` from its grid (`World::power`), so a scan costs the colony something: you don't
//! scan all the time. Values in `data\buildings.json` ("scan").

use macroquad::math::Vec2;

use super::World;
use crate::data;
use crate::kinds::BuildingKind;

/// How far a scan shows (tiles): its `radius` – the ring its picture draws – and `SCAN_MARGIN` more, so the soft edge
/// of the light lies outside the ring and nothing inside it is half dark.
pub fn scan_reach() -> f32 {
    data::get().buildings.scan.radius + SCAN_MARGIN
}

/// See `scan_reach`.
const SCAN_MARGIN: f32 = 2.0;

/// A scan being shown right now: around where, until which tick.
#[derive(Clone, Copy)]
pub struct Scanned {
    pub at: Vec2,
    pub until: u64,
}

impl World {
    /// The radar charging the next scan (its index) – the first one that works. `None` while all charges are there,
    /// or no radar works.
    pub fn scan_charger(&self) -> Option<usize> {
        if self.scan_charges >= data::get().buildings.scan.charges {
            return None;
        }
        self.scanner()
    }

    /// The first radar that works (`Building::works`) – it scans and charges. `None`: no scan.
    pub fn scanner(&self) -> Option<usize> {
        self.buildings.iter().position(|b| b.kind == BuildingKind::Radar && b.works())
    }

    /// Seconds until the next charge is there – `None` while all of them are.
    pub fn next_scan_in(&self) -> Option<u32> {
        self.scan_charger()?;
        let every = data::get().buildings.scan.every;
        Some(every.saturating_sub(self.scan_charge).div_ceil(super::TICKS_PER_SECOND as u32))
    }

    /// A scan around `at` – if there's a charge and a radar works. Seen at once, also while paused.
    pub(super) fn scan(&mut self, at: Vec2) {
        if self.scan_charges == 0 || self.scanner().is_none() {
            return;
        }
        self.scan_charges -= 1;
        let until = self.tick + data::get().buildings.scan.time as u64;
        self.scans.push(Scanned { at, until });
        // Shown: whoever sleeps there is awake now (`sleep.rs`) – and fades in.
        self.wake_area(at, scan_reach());
        self.update_vision();
    }

    /// Every tick: scans that are over go, and the next charge charges – always at the same speed, but it costs the
    /// radar's grid power meanwhile.
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
