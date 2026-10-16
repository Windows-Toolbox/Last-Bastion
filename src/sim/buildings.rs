//! Buildings: placing and replacing them, production queues, towers with units inside, and what happens when one
//! falls. *Where* a building may go is in `placement.rs`.

use std::collections::VecDeque;

use macroquad::math::{vec2, Vec2};

use super::commands::Command;
use super::terrain::free_point_near;
use super::World;
use crate::kinds::{BuildingKind, BuildingStats, UnitKind, TOWER_SLOTS};

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
            if let Some(b) = self.find_building(id) {
                let old = self.buildings.remove(b);
                self.map.set_occupied(old.tile.0, old.tile.1, old.footprint(), false);
                for unit in self.units.iter_mut().filter(|u| u.inside == Some(old.id)) {
                    unit.inside = None;
                }
                self.map_changed = true;
            }
        }
    }

    /// Destroyed buildings: free their tiles, then recompute the index and the zombies' path.
    /// Whoever was in a fallen tower now stands on the ground where it was.
    pub(super) fn remove_destroyed_buildings(&mut self) {
        let before = self.buildings.len();
        for building in &self.buildings {
            if building.hp == 0 {
                self.map.set_occupied(building.tile.0, building.tile.1, building.footprint(), false);
                for unit in self.units.iter_mut().filter(|u| u.inside == Some(building.id)) {
                    unit.inside = None;
                }
            }
        }
        self.buildings.retain(|b| b.hp > 0);
        if self.buildings.len() != before {
            self.refresh_map();
        }
    }

    /// Continue the production queues. Finished units come out next to the building.
    pub(super) fn update_buildings(&mut self) {
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
        building.kind == BuildingKind::Tower && self.garrison(building.id).len() < TOWER_SLOTS
    }
}
