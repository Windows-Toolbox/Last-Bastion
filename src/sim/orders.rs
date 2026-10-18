//! The players' orders – the only way into the simulation, also in singleplayer (DESIGN.md 12.8). They are
//! carried out right at the start of a tick, before time passes.

use macroquad::math::{vec2, Vec2};

use super::buildings::{Rally, MAX_PRODUCTION};
use super::commands::Command;
use super::spawning::{SpawnMode, TimedWave, WaveFrom, NEW_SPAWNER_SIZE};
use super::zombies::Breed;
use super::World;
use crate::data::Stage;
use crate::formation::Formation;
use crate::kinds::{BuildingKind, UnitKind};
use crate::map::{Deposit, Ore, Terrain};

/// How much ore the editor's brush puts on a tile – about the middle of a patch near the base (`mapgen/ores.rs`).
const PAINTED_ORE: u32 = 1000;

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
    /// Runners as a crowd around a point, storming. Only the tests use it – it only exists in the test build
    /// (`#[cfg(test)]`); the editor has `PlaceZombies`.
    #[cfg(test)]
    SpawnZombiesAt { count: usize, point: Vec2 },
    /// Editor brush: `count` zombies of one breed at random spots within `radius` of `center`, storming the command
    /// center or wandering around there.
    PlaceZombies { center: Vec2, radius: f32, count: usize, breed: Breed, wander: bool },
    /// A wave of `count` zombies from the spawners.
    SpawnWave { count: usize, from: WaveFrom },
    /// A wave onto the timeline, change one (`index` in `World::waves`), take one off.
    AddTimedWave(TimedWave),
    SetTimedWave { index: usize, wave: TimedWave },
    RemoveTimedWave { index: usize },
    /// All zombies gone.
    ClearZombies,
    /// A new unit at a point (only inside the world border).
    SpawnUnit { kind: UnitKind, point: Vec2 },
    /// Editor: these tiles get this terrain (grass = erase). Tiles with buildings stay as they are,
    /// whoever stands on new solid ground is pushed out.
    PaintTerrain { terrain: Terrain, tiles: Vec<(i32, i32)> },
    /// Editor: ore on these tiles (only where there's grass) – `None` takes it away.
    PaintOre { ore: Option<Ore>, tiles: Vec<(i32, i32)> },
    /// A new spawner centred on this tile (named "Spawner" + its number).
    PlaceSpawner { center: (i32, i32) },
    /// Move a spawner or change its size – it always stays on the map and at least 1 × 1.
    ResizeSpawner { spawner: u32, min: (i32, i32), size: (i32, i32) },
    RenameSpawner { spawner: u32, name: String },
    /// Constantly / periodically / off. Periodic starts counting from 0 again.
    SetSpawnMode { spawner: u32, mode: SpawnMode },
    RemoveSpawner { spawner: u32 },
    /// Editor eraser: everything on these tiles goes – buildings standing on one of them, units, zombies, the dead.
    RemoveAt { tiles: Vec<(i32, i32)> },

    // --- The round (editor) ---
    /// The round lasts this many days (at least 1) – the final waves move to the new last day.
    SetDays { days: u32 },
    /// The colony's gold right now – in the editor that's the gold the map starts with.
    SetGold { gold: u32 },
    /// The constant spawn: a stage onto it, change one (`index` in `World::constant.stages`), take one off – the
    /// stages stay in order of time. And how much bigger every group gets per day after the last stage.
    AddStage(Stage),
    SetStage { index: usize, stage: Stage },
    RemoveStage { index: usize },
    SetExtraPerDay { extra: f32 },

    // --- Buildings ---
    /// Build a building, `tile` = top left tile, `turned` by 90° (gate). Invalid spots are ignored (`can_place`),
    /// so is a building you can't pay for (`build_cost`).
    /// `editor`: without the rules of the game and for free – anywhere on the map that no other building takes
    /// (`can_place_anywhere`).
    PlaceBuilding { kind: BuildingKind, tile: (i32, i32), turned: bool, editor: bool },
    /// Everyone out of this tower.
    Unload { building: u32 },
    /// Put a unit at the end of a building's production queue – paid right away, if there's enough gold.
    Train { building: u32, kind: UnitKind },
    /// Take an entry out of the production queue (`index`: position in the queue). Its gold comes back.
    CancelTrain { building: u32, index: usize },
    SetRally { building: u32, rally: Rally },
    /// Repair these buildings – each one only if it may be repaired now and the gold is still enough for it
    /// (`start_repair`), in this order.
    Repair { buildings: Vec<u32> },
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
                self.spawn_from(&all, *count, Breed::Runner, false);
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
            Order::SpawnWave { count, from } => self.send_wave(*count, *from, Breed::Runner, false),
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
            Order::PaintOre { ore, tiles } => {
                // Ore doesn't change who may walk where – nothing to recompute.
                let deposit = ore.map(|ore| Deposit { ore, amount: PAINTED_ORE });
                for &(x, y) in tiles {
                    self.map.set_deposit(x, y, deposit);
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
            Order::RemoveAt { tiles } => self.remove_at(tiles),
            Order::SetDays { days } => self.set_days(*days),
            Order::SetGold { gold } => self.gold = *gold,
            Order::AddStage(stage) => {
                self.constant.stages.push(*stage);
                self.constant_spawn_changed();
            }
            Order::SetStage { index, stage } => {
                if let Some(s) = self.constant.stages.get_mut(*index) {
                    *s = *stage;
                    self.constant_spawn_changed();
                }
            }
            Order::RemoveStage { index } => {
                if *index < self.constant.stages.len() {
                    self.constant.stages.remove(*index);
                    self.constant_spawn_changed();
                }
            }
            Order::SetExtraPerDay { extra } => self.constant.extra_per_day = extra.max(0.0),
            Order::PlaceBuilding { kind, tile, turned, editor: false } => {
                // The price is worked out before building: built over walls, the gate gets part of theirs back.
                let cost = self.build_cost(*kind, *tile, *turned);
                if self.can_place(*kind, *tile, *turned) && self.gold >= cost {
                    self.gold -= cost;
                    self.score.gold_spent += cost;
                    self.score.buildings_built += 1;
                    self.replace_buildings(*kind, *tile, *turned);
                    self.add_building(*kind, *tile, *turned);
                }
            }
            Order::PlaceBuilding { kind, tile, turned, editor: true } => {
                if self.can_place_anywhere(*kind, *tile, *turned) {
                    self.replace_buildings(*kind, *tile, *turned);
                    // The terrain under it stays as it is (water stays water) – it shows again when the building
                    // falls. Whoever stands there is pushed out (`push_out_of_terrain`).
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
                    let cost = kind.stats().cost;
                    // Two different fields of `self` at the same time – `buildings` and `gold`: the compiler allows
                    // that, because it can see they don't overlap.
                    let building = &mut self.buildings[b];
                    // Only the barracks trains units (`BuildingKind::trains`).
                    let room = building.kind.trains() && building.production.len() < MAX_PRODUCTION;
                    if room && self.gold >= cost {
                        building.production.push_back(*kind);
                        self.gold -= cost;
                        self.score.gold_spent += cost;
                    }
                }
            }
            Order::Repair { buildings } => {
                for &id in buildings {
                    if let Some(b) = self.find_building(id) {
                        self.start_repair(b);
                    }
                }
            }
            Order::CancelTrain { building, index } => {
                if let Some(b) = self.find_building(*building) {
                    let building = &mut self.buildings[b];
                    if let Some(kind) = building.production.remove(*index) {
                        if *index == 0 {
                            building.progress = 0; // the front one had been started – from the beginning
                        }
                        // The gold comes back in full, like in StarCraft.
                        let cost = kind.stats().cost;
                        self.gold += cost;
                        self.score.gold_spent = self.score.gold_spent.saturating_sub(cost);
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

    /// The editor's eraser: everything on these tiles goes – every building that stands on at least one of them,
    /// units (also those that were in a tower that goes), zombies and the dead lying there. Like the terrain brush it
    /// gets all the tiles of a stroke at once.
    fn remove_at(&mut self, tiles: &[(i32, i32)]) {
        let n = self.map.size as i32;
        // One yes/no per tile of the map: is it under the eraser? Quicker to ask than searching the list every time.
        let mut hit = vec![false; self.map.size * self.map.size];
        for &(x, y) in tiles {
            if x >= 0 && y >= 0 && x < n && y < n {
                hit[(y * n + x) as usize] = true;
            }
        }
        let on_tile = |x: i32, y: i32| x >= 0 && y >= 0 && x < n && y < n && hit[(y * n + x) as usize];
        let on = |p: Vec2| on_tile(p.x.floor() as i32, p.y.floor() as i32);
        let gone: Vec<u32> = self.buildings.iter().filter(|b| b.tiles().into_iter().any(|(x, y)| on_tile(x, y))).map(|b| b.id).collect();
        for id in gone {
            self.take_down(id);
        }
        self.units.retain(|u| !on(u.pos));
        self.zombies.retain(|z| !on(z.pos));
        self.corpses.retain(|c| !on(c.pos));
    }
}
