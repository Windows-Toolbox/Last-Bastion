//! The players' orders – the only way into the simulation, also in singleplayer (DESIGN.md 12.8). They are
//! carried out right at the start of a tick, before time passes.

use macroquad::math::{vec2, Vec2};

use super::buildings::{Rally, MAX_PRODUCTION};
use super::commands::Command;
use super::spawning::{SpawnMode, TimedWave, WaveFrom, NEW_SPAWNER_SIZE};
use super::zombies::Breed;
use super::World;
use crate::formation::Formation;
use crate::kinds::{BuildingKind, UnitKind};
use crate::map::Terrain;

/// The players' orders. They only reach the simulation through the order list of a tick.
/// Units are named by their `id`, not by their index in the list.
#[derive(Clone)]
pub enum Order {
    /// `queue`: given with Shift → add to the end of the command queue instead of replacing it.
    /// `formation`: the units line up in this formation at the target (only for commands to a point).
    /// `None`: everyone walks to the same point.
    Command { units: Vec<u32>, command: Command, queue: bool, formation: Option<Formation> },
    /// Cancel everything (S).
    Stop { units: Vec<u32> },

    // --- Developer tools (DESIGN.md 12.6) – these are orders too, so it stays deterministic ---
    /// Zombies at random spots in random spawners.
    SpawnZombies { count: usize },
    /// Zombies as a crowd around a point (the usual mix, storming). Only the tests use it – it only exists in the
    /// test build (`#[cfg(test)]`); the editor has `PlaceZombies`.
    #[cfg(test)]
    SpawnZombiesAt { count: usize, point: Vec2 },
    /// Editor brush: `count` zombies at random spots within `radius` of `center` – of one breed (`None`: the usual
    /// mix), storming the command center or wandering around there.
    PlaceZombies { center: Vec2, radius: f32, count: usize, breed: Option<Breed>, wander: bool },
    /// A wave of `count` zombies from the spawners.
    SpawnWave { count: usize, from: WaveFrom },
    /// A wave onto the timeline, change one (`index` in `World::waves`), take one off.
    AddTimedWave(TimedWave),
    SetTimedWave { index: usize, wave: TimedWave },
    RemoveTimedWave { index: usize },
    /// All zombies gone.
    ClearZombies,
    /// Do killed zombies come back at the edge (on) or stay gone (off)?
    SetRespawn(bool),
    /// A new unit at a point (only inside the world border).
    SpawnUnit { kind: UnitKind, point: Vec2 },
    /// Editor: these tiles get this terrain (grass = erase). Tiles with buildings stay as they are,
    /// whoever stands on new solid ground is pushed out.
    PaintTerrain { terrain: Terrain, tiles: Vec<(i32, i32)> },
    /// A new spawner centred on this tile (named "Spawner" + its number).
    PlaceSpawner { center: (i32, i32) },
    /// Move a spawner or change its size – it always stays on the map and at least 1 × 1.
    ResizeSpawner { spawner: u32, min: (i32, i32), size: (i32, i32) },
    RenameSpawner { spawner: u32, name: String },
    /// Constantly / periodically / off. Periodic starts counting from 0 again.
    SetSpawnMode { spawner: u32, mode: SpawnMode },
    RemoveSpawner { spawner: u32 },

    // --- Buildings ---
    /// Build a building, `tile` = top left tile, `turned` by 90° (gate). Invalid spots are ignored (`can_place`).
    /// `editor`: without the rules of the game – anywhere on the map that no other building takes (`can_place_anywhere`).
    PlaceBuilding { kind: BuildingKind, tile: (i32, i32), turned: bool, editor: bool },
    /// Everyone out of this tower.
    Unload { building: u32 },
    /// Put a unit at the end of a building's production queue.
    Train { building: u32, kind: UnitKind },
    /// Take an entry out of the production queue (`index`: position in the queue).
    CancelTrain { building: u32, index: usize },
    SetRally { building: u32, rally: Rally },
}

impl World {
    /// Carries out orders **without** time passing: command queues, production queues, rally points, buildings are
    /// set – but nobody moves, nothing gets built. The first part of every tick; while paused it's called on its own,
    /// so you see what you've ordered (lines, queues, rally points) and it starts when the game goes on.
    /// That's the same as giving all these orders with the next tick – so it stays deterministic.
    pub fn apply_orders(&mut self, orders: &[Order]) {
        for order in orders {
            self.apply(order);
        }
        if self.map_changed {
            self.push_out_of_terrain();
            self.reroute_units();
            self.refresh_map();
        }
    }

    pub(super) fn apply(&mut self, order: &Order) {
        match order {
            Order::Command { units, command, queue, formation } => self.command(units, *command, *queue, *formation),
            Order::Stop { units } => {
                for &id in units {
                    if let Some(i) = self.find_unit(id) {
                        self.units[i].queue.clear();
                    }
                }
            }
            Order::SpawnZombies { count } => {
                let all: Vec<usize> = (0..self.spawners.len()).collect();
                self.spawn_from(&all, *count);
            }
            #[cfg(test)]
            Order::SpawnZombiesAt { count, point } => self.spawn_cluster(*count, *point),
            Order::PlaceZombies { center, radius, count, breed, wander } => {
                for _ in 0..*count {
                    // Evenly in the circle: the root makes sure there aren't more near the middle.
                    for _ in 0..10 {
                        let angle = self.rng.unit_f32() * std::f32::consts::TAU;
                        let r = radius * self.rng.unit_f32().sqrt();
                        let pos = *center + vec2(libm::cosf(angle), libm::sinf(angle)) * r;
                        if !self.map.is_blocked(pos.x.floor() as i32, pos.y.floor() as i32) {
                            self.spawn_zombie_as(pos, *breed, *wander);
                            break;
                        }
                    }
                }
            }
            Order::SpawnWave { count, from: WaveFrom::OneSpawner } => {
                if !self.spawners.is_empty() {
                    let s = self.rng.below(self.spawners.len() as u32) as usize;
                    self.spawn_from(&[s], *count);
                }
            }
            Order::SpawnWave { count, from: WaveFrom::Sides(sides) } => {
                // `sides` different sides at random – only sides that have spawners at all.
                let size = self.map.size;
                let mut left: Vec<u32> =
                    (0..4).filter(|&side| self.spawners.iter().any(|s| s.side(size) == side)).collect();
                for _ in 0..(*sides).min(left.len() as u32) {
                    let side = left.remove(self.rng.below(left.len() as u32) as usize);
                    let on_side: Vec<usize> = (0..self.spawners.len()).filter(|&s| self.spawners[s].side(size) == side).collect();
                    self.spawn_from(&on_side, *count);
                }
            }
            Order::AddTimedWave(wave) => self.waves.push(*wave),
            Order::SetTimedWave { index, wave } => {
                if let Some(w) = self.waves.get_mut(*index) {
                    *w = *wave;
                }
            }
            Order::RemoveTimedWave { index } => {
                if *index < self.waves.len() {
                    self.waves.remove(*index);
                }
            }
            Order::ClearZombies => self.zombies.clear(),
            Order::SetRespawn(on) => self.respawn = *on,
            Order::SpawnUnit { kind, point } => {
                if !self.map.blocks_unit(point.x.floor() as i32, point.y.floor() as i32) {
                    self.spawn_unit(*kind, *point);
                }
            }
            Order::PaintTerrain { terrain, tiles } => {
                for &(x, y) in tiles {
                    if self.map.set_terrain(x, y, *terrain) {
                        self.map_changed = true;
                    }
                }
            }
            Order::PlaceSpawner { center } => {
                let half = NEW_SPAWNER_SIZE / 2;
                let name = format!("Spawner {}", self.next_spawner_id + 1);
                let (min, size) = ((center.0 - half, center.1 - half), (NEW_SPAWNER_SIZE, NEW_SPAWNER_SIZE));
                self.add_spawner(name, min, size, SpawnMode::Constant);
            }
            Order::ResizeSpawner { spawner, min, size } => {
                if let Some(s) = self.find_spawner(*spawner) {
                    self.spawners[s].min = *min;
                    self.spawners[s].size = *size;
                    self.keep_spawner_on_map(s);
                }
            }
            Order::RenameSpawner { spawner, name } => {
                let name = name.trim();
                if let (Some(s), false) = (self.find_spawner(*spawner), name.is_empty()) {
                    self.spawners[s].name = name.to_owned();
                }
            }
            Order::SetSpawnMode { spawner, mode } => {
                if let Some(s) = self.find_spawner(*spawner) {
                    self.spawners[s].mode = *mode;
                    self.spawners[s].timer = 0;
                }
            }
            Order::RemoveSpawner { spawner } => self.spawners.retain(|s| s.id != *spawner),
            Order::PlaceBuilding { kind, tile, turned, editor: false } => {
                if self.can_place(*kind, *tile, *turned) {
                    self.replace_buildings(*kind, *tile, *turned);
                    self.add_building(*kind, *tile, *turned);
                }
            }
            Order::PlaceBuilding { kind, tile, turned, editor: true } => {
                if self.can_place_anywhere(*kind, *tile, *turned) {
                    self.replace_buildings(*kind, *tile, *turned);
                    // Terrain under it becomes grass; whoever stands there is pushed out (`push_out_of_terrain`).
                    let (w, h) = kind.footprint(*turned);
                    for y in tile.1..tile.1 + h {
                        for x in tile.0..tile.0 + w {
                            self.map.set_terrain(x, y, Terrain::Grass);
                        }
                    }
                    self.add_building(*kind, *tile, *turned);
                }
            }
            Order::Unload { building } => {
                if let Some(b) = self.find_building(*building) {
                    // Out on the side of the rally point, then on to it – like units leaving the command center.
                    // Without one: on the side of the base, in a wall that's usually the inside.
                    let rally = self.buildings[b].rally;
                    let toward = match rally {
                        Some(Rally::Point(p)) => p,
                        Some(Rally::Unit(id)) => self.find_unit(id).map_or(self.base, |u| self.units[u].pos),
                        None => self.base,
                    };
                    let pos = self.exit_point(b, toward).unwrap_or(self.buildings[b].center());
                    let garrison = self.garrison(*building);
                    let ids: Vec<u32> = garrison.iter().map(|&i| self.units[i].id).collect();
                    for i in garrison {
                        let unit = &mut self.units[i];
                        unit.inside = None;
                        unit.pos = pos;
                        unit.prev = pos;
                    }
                    match rally {
                        Some(Rally::Point(p)) => self.command(&ids, Command::Move(p), false, None),
                        Some(Rally::Unit(target)) => self.command(&ids, Command::Follow(target), false, None),
                        None => {}
                    }
                }
            }
            Order::Train { building, kind } => {
                if let Some(b) = self.find_building(*building) {
                    let building = &mut self.buildings[b];
                    // For testing, the command center builds what the barracks will build later.
                    if building.kind == BuildingKind::CommandCenter && building.production.len() < MAX_PRODUCTION {
                        building.production.push_back(*kind);
                    }
                }
            }
            Order::CancelTrain { building, index } => {
                if let Some(b) = self.find_building(*building) {
                    let building = &mut self.buildings[b];
                    if building.production.remove(*index).is_some() && *index == 0 {
                        building.progress = 0; // the front one had been started – from the beginning
                    }
                }
            }
            Order::SetRally { building, rally } => {
                if let Some(b) = self.find_building(*building) {
                    self.buildings[b].rally = Some(*rally);
                }
            }
        }
    }
}
