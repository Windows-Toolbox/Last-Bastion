//! The players' orders – the only way into the simulation, also in singleplayer (DESIGN.md 12.8). They are
//! carried out right at the start of a tick, before time passes.

use macroquad::math::{vec2, Vec2};

use super::buildings::{Product, Rally, MAX_PRODUCTION};
use super::clipboard::{Clip, Piece};
use super::commands::Command;
use super::doom::LootKind;
use super::events::{plan_events, Event};
use super::landscaping::{Area, Patch, TerrainClip};
use super::neutral::{Owner, Tag, ThingRef};
use super::rails::RailPiece;
use super::regions::{Spot, Zone};
use super::{Ornament, OrnamentKind};
use super::story::Story;
use super::spawning::{Axes, BreedCount, SpawnMode, WaveFrom, WaveGroup, NEW_SPAWNER_SIZE};
use super::zombies::Breed;
use super::World;
use crate::data;
use crate::formation::Formation;
use crate::kinds::{BuildingKind, Resource, UnitKind};
use crate::map::{BigRock, Deposit, Ore, Terrain};

/// How much ore the editor's brush puts on a tile – about the middle of a patch near the base (`mapgen/ores.rs`).
const PAINTED_ORE: u32 = 1000;
/// The editor's "Grid" makes the whole map at least and at most this big (`Map::size`, edge in tiles) – room for a
/// playing field of about 512 × 512, twice TAB's 256 across. Every tile of the grid costs memory and time, also outside
/// the playing field.
pub const MAP_SIZES: std::ops::RangeInclusive<usize> = 64..=760;
/// The editor's zombie brush puts at most this many of each breed on a tile (like TAB: never more than 3).
pub const MOST_PER_TILE: u32 = 3;

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
    /// Send these units away (Del, DESIGN.md 7.2): they leave the colony – no more upkeep, their workers are free
    /// again. No corpse, no gold back, but half their goods (`dismiss_refund_percent`, rounded up); a barrel one of
    /// them carries is put down first.
    Dismiss { units: Vec<u32> },

    // --- Developer tools (DESIGN.md 12.6) – these are orders too, so it stays deterministic ---
    /// Zombies at random spots in random spawners.
    SpawnZombies { count: usize },
    /// Runners as a crowd around a point, storming. Only the tests use it – it only exists in the test build
    /// (`#[cfg(test)]`); the editor has `PlaceZombies`.
    #[cfg(test)]
    SpawnZombiesAt { count: usize, point: Vec2 },
    /// Editor "Drop": `count` zombies of one breed at random spots within `radius` of `center`, storming the command
    /// center or wandering around there.
    PlaceZombies { center: Vec2, radius: f32, count: usize, breed: Breed, wander: bool },
    /// The editor's zombie brush, like TAB: on each of these tiles so many of each breed (at most `MOST_PER_TILE`) –
    /// what stands there already counts, painting over a tile again only fills it up. `wander`: asleep and strolling
    /// like the zombies a new map starts with, otherwise awake and storming the command center. A giant is one tile
    /// with one giant.
    PaintZombies { tiles: Vec<(i32, i32)>, per_tile: Vec<(Breed, u32)>, wander: bool },
    /// A wave of `count` runners from the spawners.
    SpawnWave { count: usize, from: WaveFrom },
    /// An event onto the map's list, change one (`index` in `World::events`), take one off. An added or changed one
    /// starts anew (its random offset rolled again) – if its hour has passed, it comes at once.
    AddEvent(Event),
    SetEvent { index: usize, event: Event },
    RemoveEvent { index: usize },
    /// An event's zombies, now, once – like "Send now" but with the event's groups and numbers (the middle of a range is
    /// rolled); the event itself keeps its schedule.
    SendEvent { index: usize },
    /// All events anew from TAB's survival plan (`plan_events`) for the round's days and this difficulty's zombie
    /// population – what was there is replaced, the offsets are rolled again.
    PlanEvents { difficulty: String },
    /// All zombies gone.
    ClearZombies,
    /// A new unit at a point (only inside the world border).
    SpawnUnit { kind: UnitKind, point: Vec2, facing: Option<f32> },
    /// Editor: these tiles get this terrain (grass = erase). Tiles with buildings stay as they are,
    /// whoever stands on new solid ground is pushed out.
    PaintTerrain { terrain: Terrain, tiles: Vec<(i32, i32)> },
    /// Editor: the forest, rock or ore on these tiles is drawn with this group of pictures – 1–3: the landscape's A,
    /// B, C, 0: mixed again (`Map::set_picture`). Only the look.
    PaintPicture { picture: u8, tiles: Vec<(i32, i32)> },
    /// Editor "Smooth": each of these tiles takes the ground most of its neighbours have (`landscaping.rs`).
    SmoothTerrain { tiles: Vec<(i32, i32)> },
    /// Editor: a copied area's ground back onto the map, its corner on the tile `to`. `spare`: not on or next to
    /// buildings and the railway (`World::spare`). `Box`: the copy lies somewhere else in memory, so the order itself
    /// stays small – like `Paste`.
    PasteTerrain { clip: Box<TerrainClip>, to: (i32, i32), spare: bool },
    /// Editor "Generate in the area": a patch of forest, a lake, mountains, an ore field or a village of doom inside
    /// `area`, made from `seed`. `percent`: how much of the area it takes.
    Generate { what: Patch, area: Area, percent: u32, seed: u64, spare: bool },
    /// Editor: the map is drawn as this one of TAB's landscapes from now on ("BR", "FA" … – `World::set_look`), with
    /// its weather. What the landscape does to the round (`modifiers`) stays as it is.
    SetLook { look: String },
    /// Editor: one of TAB's big rocks – its tiles become mountain like painted ones (`Map::place_big_rock`).
    PlaceBigRock(BigRock),
    /// Editor: ore on these tiles (only where there's grass) – `None` takes it away.
    PaintOre { ore: Option<Ore>, tiles: Vec<(i32, i32)> },
    /// A new spawner centred on this point of the screen axes (`map::to_frame`), named "Spawner" + its number.
    PlaceSpawner { center: (i32, i32) },
    /// Move a spawner or change its size – it always stays on the map and at least 1 × 1.
    ResizeSpawner { spawner: u32, min: (i32, i32), size: (i32, i32) },
    RenameSpawner { spawner: u32, name: String },
    /// Straight on screen or slanted like the tiles – around the same middle, about as big (`Spawner::turn_to`).
    SetSpawnerAxes { spawner: u32, axes: Axes },
    /// Constantly / periodically / off. Periodic starts counting from 0 again.
    SetSpawnMode { spawner: u32, mode: SpawnMode },
    /// Gone – and out of every wave and stage that named it.
    RemoveSpawner { spawner: u32 },
    /// Editor "Regions" (`regions.rs`): a new named region – its number is given here, and its name if it's empty
    /// ("Region 7").
    AddZone { zone: Zone },
    /// The region with this number (`zone.id`) is this one now: name, colour, shapes.
    SetZone { zone: Zone },
    /// Tiles painted into a region by hand – or, `cut`, out of it.
    PaintZone { zone: u32, tiles: Vec<(i32, i32)>, cut: bool },
    RemoveZone { zone: u32 },
    /// The same for a named point.
    AddSpot { spot: Spot },
    SetSpot { spot: Spot },
    RemoveSpot { spot: u32 },
    /// Editor: a house of a village of doom (top left tile, width and depth) with as many zombies inside as the generator
    /// would put there (`World::doom_zombies`). Only inside the world border, on open ground nothing else takes.
    PlaceDoom { tile: (i32, i32), size: (i32, i32) },
    /// Editor (Ctrl+V): a copy again, its middle on this tile (`clipboard.rs`) – what doesn't fit is left out.
    Paste { clip: Box<Clip>, to: (i32, i32) },
    /// Editor (Ctrl+X, Del): these off the map – buildings, units, zombies, houses of doom, loot, spawners, decoration.
    RemovePieces { pieces: Vec<Piece> },
    /// Editor: a pile of loot at this point – of this kind (`None`: rolled), the usual amount and as many guards as the
    /// generator would hide there.
    PlaceLoot { point: Vec2, kind: Option<LootKind> },
    /// Editor: what a pile holds – its kind, how much, how many guards lie hidden around it, and the name ID triggers
    /// know it by.
    SetLoot { id: u32, kind: LootKind, amount: u32, guards: u32, name_id: String },
    /// Editor: an ornament – a piece of the railway (`ornaments.rs`). Only where all of it is on the map, and not twice
    /// the same on the same tile.
    PlaceOrnament(Ornament),
    /// Editor eraser: everything on these tiles goes – buildings standing on one of them, houses of doom, units,
    /// zombies, loot, the dead.
    RemoveAt { tiles: Vec<(i32, i32)> },
    /// Editor (Del): these buildings go, whatever they are – nothing comes back.
    RemoveBuildings { buildings: Vec<u32> },
    /// Editor, TAB's entity properties (`neutral.rs`): a placed thing's tag (empty: none), its owner, its hit points in
    /// percent, infected (a building) or a veteran (a unit) – and the thing removed.
    SetTag { thing: ThingRef, tag: Tag },
    SetOwner { thing: ThingRef, owner: Owner },
    /// Editor ("Placing: owner"): carry out the order inside – a building or a unit put down – and what it made belongs
    /// to `owner` right away. If it made nothing (no room), nothing happens. `Box`: like `Credit`.
    PlaceAs { owner: Owner, order: Box<Order> },
    SetHealth { thing: ThingRef, percent: u32 },
    SetMark { thing: ThingRef, on: bool },
    /// One thing's combat values as percents of its kind's (`tweak.rs`): hit points, damage, attack speed, range, sight,
    /// speed, armor. Its life stays the same share of the new maximum. Only units so far.
    SetTweak { thing: ThingRef, tweak: super::Tweak },
    /// The editor's Boss tool: one zombie of this breed with its own hit points, damage, attack speed, speed and armor
    /// (`tweak.rs`) – asleep (`wander`) or storming the command center.
    PlaceBoss { pos: Vec2, breed: Breed, wander: bool, tweak: super::Tweak },
    /// A track of upgrades is at this level (`data\upgrades.json`; the editor's start level of a map, a campaign).
    SetUpgrade { track: String, level: u32 },
    /// The boss at `pos` gets other values (the Zombies module's list, or a click on it).
    SetBoss { pos: Vec2, tweak: super::Tweak },
    RemoveThing { thing: ThingRef },
    /// Editor: the map's story – triggers, goals, the command center's button, variables (`story.rs`).
    SetStory { story: Story },
    /// The player picked option `index` of the oldest waiting choice of a script (`Action::Choose`).
    Choose { index: usize },
    /// The player pressed the command center's own button (`Story::command`).
    CustomCommand,
    /// Editor "Grid": the whole map gets this edge length (`Map::size`, even) – it grows or shrinks on every side,
    /// everything on it stays where it is, new ground is grass (`mapfile::MapFile::resized`).
    ResizeMap { size: usize },
    /// Editor (later triggers too): the playing field – the world border – lies here now, in whole steps of the screen
    /// axes (`Map::set_play_area`). The ground and what stands on it stay; units outside go to the nearest free spot
    /// inside (`push_out_of_terrain`), buildings outside simply stand outside.
    SetPlayArea { min: (i32, i32), size: (i32, i32) },

    // --- The round (editor) ---
    /// The round lasts this many days (at least 1) – the final waves move to the new last day.
    SetDays { days: u32 },
    /// The round's difficulty, by name (`data\game_difficulties.json`; an unknown name is the default one): how close
    /// each breed may come (`keep_out`) and what new villages of doom hold. The gold, wood and everything placed stay.
    SetDifficulty { name: String },
    /// The map changes one value of `data` for itself ("units.soldier.hp" = 150, `data::Overrides`) – `None` puts it
    /// back. Every part of the game reads the new value from now on; what already exists keeps what it was made with
    /// (a unit's life now, not its maximum).
    SetData { key: String, value: Option<serde_json::Value> },
    /// Every value of the map back to what `data` says.
    ClearData,
    /// The colony's gold right now – in the editor that's the gold the map starts with.
    SetGold { gold: u32 },
    /// The wood in the stock right now – in the editor the wood the map starts with.
    SetWood { wood: u32 },

    // --- Buildings ---
    /// Build a building, `tile` = top left tile, `turned` by 90° (gate). Invalid spots are ignored (`can_place`),
    /// so is a building you can't pay for (`build_cost`, `can_afford_goods`), that there aren't enough free workers
    /// (`build_workers`) or spare power (`has_power_for`, like in TAB) for, or whose `requires` doesn't stand yet
    /// (`unlocked`).
    /// `editor`: without the rules of the game and for free – anywhere on the map that no other building takes
    /// (`can_place_anywhere`).
    PlaceBuilding { kind: BuildingKind, tile: (i32, i32), turned: bool, editor: bool },
    /// Everyone out of this tower.
    Unload { building: u32 },
    /// Put a unit at the end of a building's production queue – paid right away, if there's enough gold and a free
    /// worker for it, what it `requires` stands (firearms: the munitions factory) and it's this building that trains
    /// it (`trained_in`: barracks or inn).
    Train { building: u32, kind: UnitKind },
    /// A barrel at the end of a munitions factory's queue – paid right away, gold and coal (`blast.rs`).
    MakeBarrel { building: u32 },
    /// Take an entry out of the production queue (`index`: position in the queue). Its gold (and coal) and its worker
    /// come back.
    CancelTrain { building: u32, index: usize },
    /// Markets: sell what's above the mark (`on`) – or not (`market.rs`).
    SetSelling { buildings: Vec<u32>, on: bool },
    /// Mine traps: wait this many steps (0–3) before going off (`blast.rs`, TAB's `DelayActivation`).
    SetMineDelay { buildings: Vec<u32>, delay: u8 },
    /// This workshop starts research `tech` (`research.rs`, the index in `data\research.json`).
    Research { building: u32, tech: u16 },
    /// The same order, but gold may go below 0 into the banks' credit (Shift, `debt.rs`) – everything else it needs
    /// has to be there as always. `Box`: an order inside an order needs a fixed size.
    Credit(Box<Order>),
    /// The command center's scan: show a circle of the map around this point for a while (`scan.rs`).
    Scan { at: Vec2 },
    /// These units put their barrels down where they stand (`blast.rs`).
    DropBarrels { units: Vec<u32> },
    SetRally { building: u32, rally: Rally },
    /// Repair these buildings – each one only if it may be repaired now and the gold is still enough for it
    /// (`start_repair`), in this order.
    Repair { buildings: Vec<u32> },
    /// Tear these buildings down (Del, like TAB): part of their price comes back (`demolish`). Not the command center,
    /// not an infected building.
    Demolish { buildings: Vec<u32> },
    /// Switch these buildings off (`off`) or on again (Y) – only those that can be (`can_switch_off`), not infected
    /// ones. On again only if there are enough free workers for it, in this order.
    SwitchOff { buildings: Vec<u32>, off: bool },
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
        // Overrides may have changed – or the world is a different one than the last (undo): the values follow.
        self.use_data();
        // Levels of upgrades changed, or something new stands: its sums are made now.
        self.refresh_upgrades();
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
                        self.units[i].restart_shot();
                    }
                }
            }
            Order::Dismiss { units } => {
                self.drop_barrels(units);
                self.refund_dismissed(units);
                // `retain`, not `swap_remove`: the order of the list stays the same on every PC.
                let before = self.units.len();
                self.units.retain(|u| !units.contains(&u.id));
                self.score.units_dismissed += (before - self.units.len()) as u32;
            }
            Order::SpawnZombies { count } => {
                let all: Vec<(usize, u32)> = (0..self.spawners.len()).map(|s| (s, 0)).collect();
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
            Order::SpawnWave { count, from } => {
                let group = WaveGroup { from: from.clone(), breeds: vec![BreedCount::new(*count as u32, Breed::Runner)] };
                self.send_group(&group, false);
            }
            Order::AddEvent(event) => {
                self.events.push(event.clone());
                self.event_changed(self.events.len() - 1);
            }
            Order::SetEvent { index, event } => {
                if *index < self.events.len() {
                    let old = std::mem::replace(&mut self.events[*index], event.clone()).name;
                    self.rename_event_in_triggers(&old, &event.name);
                    self.event_changed(*index);
                }
            }
            Order::RemoveEvent { index } => {
                if *index < self.events.len() {
                    self.events.remove(*index);
                }
            }
            Order::SendEvent { index } => {
                if let Some(event) = self.events.get(*index).cloned() {
                    for group in &event.groups {
                        let breeds = group
                            .breeds
                            .iter()
                            .map(|b| {
                                let (lo, hi) = (b.count, b.up_to.max(b.count));
                                BreedCount::new(lo + self.rng.below(hi - lo + 1), b.breed)
                            })
                            .collect();
                        self.send_group(&WaveGroup { from: group.from.clone(), breeds }, false);
                    }
                }
            }
            Order::PlanEvents { difficulty } => {
                self.events = plan_events(self.days, data::get().difficulty(difficulty).population);
                self.roll_events();
            }
            Order::ClearZombies => {
                self.zombies.clear();
                self.sleep.clear();
                self.spits.clear();
                self.arrows.clear();
            }
            Order::SpawnUnit { kind, point, facing } => {
                if !self.map.blocks_unit(point.x.floor() as i32, point.y.floor() as i32) {
                    self.spawn_unit(*kind, *point);
                    if let (Some(facing), Some(unit)) = (facing, self.units.last_mut()) {
                        unit.facing = *facing;
                    }
                }
            }
            Order::PaintTerrain { terrain, tiles } => {
                for &(x, y) in tiles {
                    if self.map.set_terrain(x, y, *terrain) {
                        self.map_changed = true;
                    }
                }
            }
            Order::PaintPicture { picture, tiles } => self.paint_picture(*picture, tiles),
            Order::SmoothTerrain { tiles } => self.smooth(tiles),
            Order::PasteTerrain { clip, to, spare } => self.paste_terrain(clip, *to, *spare),
            Order::Generate { what, area, percent, seed, spare } => self.generate_patch(*what, *area, *percent, *seed, *spare),
            Order::SetLook { look } => self.set_look(look),
            Order::PlaceBigRock(rock) => {
                if self.map.place_big_rock(*rock) {
                    self.map_changed = true;
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
                self.add_spawner(0, name, min, size, Axes::Screen, SpawnMode::Off);
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
            Order::SetSpawnerAxes { spawner, axes } => {
                if let Some(s) = self.find_spawner(*spawner) {
                    self.spawners[s].turn_to(*axes);
                    self.keep_spawner_on_map(s);
                }
            }
            Order::RemoveSpawner { spawner } => {
                self.spawners.retain(|s| s.id != *spawner);
                self.forget_spawner(*spawner);
            }
            Order::AddZone { zone } => self.add_zone(zone.clone()),
            Order::SetZone { zone } => self.set_zone(zone),
            Order::PaintZone { zone, tiles, cut } => self.paint_zone(*zone, tiles, *cut),
            Order::RemoveZone { zone } => {
                self.zones.retain(|z| z.id != *zone);
                self.forget_zone(*zone);
            }
            Order::AddSpot { spot } => self.add_spot(spot.clone()),
            Order::SetSpot { spot } => self.set_spot(spot),
            Order::RemoveSpot { spot } => self.spots.retain(|s| s.id != *spot),
            Order::PlaceDoom { tile, size } => {
                if self.doom_fits(*tile, *size) {
                    // As full as the generator makes it at the map's difficulty.
                    let population = data::get().difficulty(&self.difficulty).population;
                    self.add_doom(*tile, *size, World::doom_zombies(*size, population), population);
                }
            }
            Order::PlaceLoot { point, kind } => {
                if self.map.in_play(point.x.floor() as i32, point.y.floor() as i32) && !self.map.is_blocked(point.x.floor() as i32, point.y.floor() as i32) {
                    self.place_loot(*point, *kind);
                }
            }
            Order::SetLoot { id, kind, amount, guards, name_id } => {
                if let Some(l) = self.find_loot(*id) {
                    let pile = &mut self.loot[l];
                    (pile.kind, pile.amount, pile.guards, pile.name_id) = (*kind, *amount, *guards, name_id.clone());
                }
            }
            Order::PlaceOrnament(ornament) => {
                // Every tile it lies on on the map – not off it, nor in a corner of the grid past its edge.
                let (w, h) = ornament.kind.footprint();
                let (x, y) = ornament.tile;
                let covers = |t: (i32, i32)| match ornament.kind {
                    OrnamentKind::Rail(shape) => RailPiece { shape, tile: ornament.tile }.covers(t),
                    _ => true,
                };
                let on_map = (y..y + h).all(|ty| (x..x + w).all(|tx| !covers((tx, ty)) || self.map.on_map(tx, ty)));
                if on_map && !self.ornaments.contains(ornament) {
                    self.ornaments.push(*ornament);
                }
            }
            Order::RemoveAt { tiles } => self.remove_at(tiles),
            Order::Paste { clip, to } => self.paste(clip, *to),
            Order::RemovePieces { pieces } => self.remove_pieces(pieces),
            Order::RemoveBuildings { buildings } => {
                for &id in buildings {
                    self.take_down(id);
                }
            }
            Order::SetTag { thing, tag } => self.set_tag(*thing, tag),
            Order::SetOwner { thing, owner } => {
                self.set_owner(*thing, *owner);
                self.expect_payout_before_start();
            }
            Order::SetHealth { thing, percent } => self.set_health(*thing, *percent),
            Order::SetMark { thing, on } => self.set_mark(*thing, *on),
            Order::SetTweak { thing, tweak } => self.set_tweak(*thing, *tweak),
            Order::PlaceBoss { pos, breed, wander, tweak } => self.add_boss(*pos, *breed, *wander, *tweak),
            Order::SetBoss { pos, tweak } => self.set_boss(*pos, *tweak),
            Order::SetUpgrade { track, level } => self.set_upgrade_level(track, *level),
            Order::RemoveThing { thing } => self.remove_thing(*thing),
            Order::SetStory { story } => {
                self.story = story.clone();
                self.drop_lost_runs();
            }
            Order::Choose { index } => self.choose(*index),
            Order::CustomCommand => self.custom_command(),
            Order::ResizeMap { size } => self.resize(*size),
            Order::SetPlayArea { min, size } => {
                self.map.set_play_area(*min, *size);
                self.map_changed = true;
            }
            Order::PaintZombies { tiles, per_tile, wander } => self.paint_zombies(tiles, per_tile, *wander),
            Order::SetDays { days } => self.set_days(*days),
            Order::SetDifficulty { name } => self.set_difficulty(data::get().difficulty(name)),
            Order::SetData { key, value } => self.overrides.set(key, value.clone()),
            Order::ClearData => self.overrides = data::Overrides::default(),
            Order::SetGold { gold } => {
                self.gold = *gold;
                self.debt = 0;
            }
            Order::SetWood { wood } => self.stock.set(Resource::Wood, *wood),
            Order::PlaceBuilding { kind, tile, turned, editor: false } => {
                // The price is worked out before building: built over walls, the gate gets part of theirs back.
                let cost = self.build_cost(*kind, *tile, *turned);
                let workers = self.build_workers(*kind, *tile, *turned) <= self.free_workers();
                let paid = self.spendable() >= cost && self.can_afford_goods(*kind) && workers && self.has_power_for(*kind, *tile, *turned);
                if paid && self.unlocked(*kind) && self.can_place(*kind, *tile, *turned) {
                    self.pay_gold(cost);
                    self.pay_goods(*kind);
                    self.score.gold_spent += cost;
                    self.score.buildings_built += 1;
                    self.replace_buildings(*kind, *tile, *turned);
                    self.add_building(*kind, *tile, *turned);
                    // It builds itself first (`build_seconds`). One that stands at once: a charcoal burner lights up
                    // right away if there's wood – not only at the next full hour.
                    self.start_construction(self.buildings.len() - 1);
                    self.fuel(self.buildings.len() - 1);
                    // Building is loud work (`sound.rs`).
                    let center = self.buildings[self.buildings.len() - 1].center();
                    self.make_colony_sound(center, data::get().sound.building_placed);
                    self.fx.push(super::Fx::Placed { kind: *kind });
                }
            }
            Order::PlaceBuilding { kind, tile, turned, editor: true } => {
                if self.can_place_anywhere(*kind, *tile, *turned) {
                    self.replace_buildings(*kind, *tile, *turned);
                    // The terrain under it stays as it is (water stays water) – it shows again when the building
                    // falls. Whoever stands there is pushed out (`push_out_of_terrain`).
                    self.add_building(*kind, *tile, *turned);
                    self.expect_payout_before_start();
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
                    // Before `&mut self.buildings[b]`: while that borrow lives, `self` can't be read as a whole.
                    let workers = kind.stats().worker_cost <= self.free_workers() && self.unit_unlocked(*kind);
                    let gold = self.spendable() >= cost && self.can_afford_unit_goods(*kind);
                    let building = &mut self.buildings[b];
                    // Only where it's trained: soldiers in the barracks, mercenaries in the inn (`trained_in`) – once
                    // it's finished.
                    let room = building.kind == kind.stats().trained_in && building.is_built() && building.production.len() < MAX_PRODUCTION;
                    if room && gold && workers {
                        building.production.push_back(Product::Unit(*kind));
                        self.score.gold_spent += cost;
                        self.pay_gold(cost);
                        self.pay_unit_goods(*kind);
                    }
                }
            }
            Order::MakeBarrel { building } => self.make_barrel(*building),
            Order::SetSelling { buildings, on } => {
                for &id in buildings {
                    if let Some(b) = self.find_building(id).filter(|&b| self.buildings[b].kind == BuildingKind::Market) {
                        self.buildings[b].selling = *on;
                    }
                }
            }
            Order::Research { building, tech } => self.start_research(*building, *tech),
            Order::SetMineDelay { buildings, delay } => {
                for &id in buildings {
                    if let Some(b) = self.find_building(id).filter(|&b| self.buildings[b].kind == BuildingKind::MineTrap) {
                        self.buildings[b].delay = (*delay).min(3);
                    }
                }
            }
            Order::PlaceAs { owner, order } => {
                // The numbers the next building and unit get: whichever is taken afterwards is the new one.
                let (building, unit, _) = self.next_ids();
                self.apply(order);
                let (next_building, next_unit, _) = self.next_ids();
                if next_building > building {
                    self.set_owner(ThingRef::Building(building), *owner);
                } else if next_unit > unit {
                    self.set_owner(ThingRef::Unit(unit), *owner);
                }
                self.expect_payout_before_start();
            }
            Order::Credit(order) => {
                self.on_credit = true;
                self.apply(order);
                self.on_credit = false;
            }
            Order::Scan { at } => self.scan(*at),
            Order::DropBarrels { units } => self.drop_barrels(units),
            Order::Repair { buildings } => {
                for &id in buildings {
                    if let Some(b) = self.find_building(id) {
                        self.start_repair(b);
                    }
                }
            }
            Order::Demolish { buildings } => {
                for &id in buildings {
                    self.demolish(id);
                }
            }
            Order::SwitchOff { buildings, off } => {
                for &id in buildings {
                    if let Some(b) = self.find_building(id) {
                        self.switch_off(b, *off);
                    }
                }
            }
            Order::CancelTrain { building, index } => {
                if let Some(b) = self.find_building(*building) {
                    let building = &mut self.buildings[b];
                    if let Some(product) = building.production.remove(*index) {
                        if *index == 0 {
                            building.progress = 0; // the front one had been started – from the beginning
                        }
                        // The gold comes back in full, like in StarCraft – the worker too (`workers` counts the
                        // queue, so it's free again by itself), the wood and iron of a unit and the coal of a barrel.
                        let cost = product.cost();
                        self.gold += cost;
                        self.score.gold_spent = self.score.gold_spent.saturating_sub(cost);
                        let goods = match product {
                            Product::Unit(kind) => {
                                let s = kind.stats();
                                vec![(Resource::Wood, s.cost_wood), (Resource::Iron, s.cost_iron), (Resource::Oil, s.cost_oil)]
                            }
                            Product::Barrel => vec![(Resource::Coal, data::get().buildings.blast.cost_coal)],
                            Product::Research(t) => super::tech(t).goods().to_vec(),
                        };
                        for (r, n) in goods {
                            self.stock.add(r, n, self.storage(r));
                        }
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
        // Houses of doom on the tiles: gone, their tiles open again.
        let map = &mut self.map;
        let before = self.doom.len();
        self.doom.retain(|d| {
            let hit = (d.tile.1..d.tile.1 + d.size.1).any(|y| (d.tile.0..d.tile.0 + d.size.0).any(|x| on_tile(x, y)));
            if hit {
                map.set_doom(d.tile.0, d.tile.1, d.size, false);
            }
            !hit
        });
        self.map_changed |= self.doom.len() != before;
        self.units.retain(|u| !on(u.pos));
        self.zombies.retain(|z| !on(z.pos));
        self.sleep.remove(on);
        self.loot.retain(|l| !on(l.pos));
        self.corpses.retain(|c| !on(c.pos));
        self.dying.retain(|d| !on(d.pos));
        self.remove_neutral_at(on_tile);
        // Ornaments with a tile under it – a piece of track only where it lies.
        self.ornaments.retain(|o| {
            let (w, h) = o.kind.footprint();
            let tiles = (o.tile.1..o.tile.1 + h).flat_map(|y| (o.tile.0..o.tile.0 + w).map(move |x| (x, y)));
            let covered = |t: (i32, i32)| match o.kind {
                OrnamentKind::Rail(shape) => RailPiece { shape, tile: o.tile }.covers(t),
                _ => true,
            };
            !tiles.filter(|&t| covered(t)).any(|(x, y)| on_tile(x, y))
        });
    }

    /// The zombie brush (`Order::PaintZombies`). Who stands on a painted tile already is counted first – awake or asleep
    /// – so a tile never gets more than it should have of a breed. Then the rest, each at a random spot of its tile, not
    /// quite at its edge (like `scatter_roamers`). Tiles that are blocked (water, a building) or off the map get none.
    fn paint_zombies(&mut self, tiles: &[(i32, i32)], per_tile: &[(Breed, u32)], wander: bool) {
        let n = self.map.size as i32;
        let tiles: Vec<(i32, i32)> = tiles.iter().copied().filter(|&(x, y)| !self.map.is_blocked(x, y)).collect();
        // Per tile of the map: its place in `tiles` plus 1 – 0 for one that isn't painted. A list, no `HashMap`:
        // nothing in the simulation may depend on the order of one (DESIGN.md 12.8).
        let mut slot = vec![0usize; (n * n) as usize];
        for (k, &(x, y)) in tiles.iter().enumerate() {
            slot[(y * n + x) as usize] = k + 1;
        }
        // How many of each breed of `per_tile` stand on every painted tile already.
        let mut there = vec![vec![0u32; per_tile.len()]; tiles.len()];
        let standing = self.zombies.iter().map(|z| (z.pos, z.breed)).chain(self.sleep.iter().map(|s| (s.pos, s.breed)));
        for (pos, breed) in standing {
            let (x, y) = (pos.x.floor() as i32, pos.y.floor() as i32);
            if x < 0 || y < 0 || x >= n || y >= n {
                continue;
            }
            let k = slot[(y * n + x) as usize];
            if let (true, Some(b)) = (k > 0, per_tile.iter().position(|&(other, _)| other == breed)) {
                there[k - 1][b] += 1;
            }
        }
        for (k, &(x, y)) in tiles.iter().enumerate() {
            for (b, &(breed, wanted)) in per_tile.iter().enumerate() {
                for _ in there[k][b]..wanted.min(MOST_PER_TILE) {
                    let pos = vec2(x as f32 + 0.15 + 0.7 * self.rng.unit_f32(), y as f32 + 0.15 + 0.7 * self.rng.unit_f32());
                    if wander {
                        self.lay_to_sleep(pos, breed);
                    } else {
                        self.spawn_zombie_as(pos, breed, false);
                    }
                }
            }
        }
    }

    /// The editor's "Grid" (`Order::ResizeMap`): the map gets an edge of `size` tiles (even, within `MAP_SIZES`).
    /// Done through the map file – as if it were saved, made bigger or smaller there (`MapFile::resized`) and loaded
    /// again: everything a map is (terrain, buildings, zombies, spawners …) is in it, and loading builds everything that
    /// depends on the size anew (ways, sight, the zombies' grid). Like loading, the spawners and units get new numbers.
    /// `sim` uses `mapfile` here, the other way round than usual – Rust doesn't mind modules that use each other.
    fn resize(&mut self, size: usize) {
        let size = size.clamp(*MAP_SIZES.start(), *MAP_SIZES.end()) / 2 * 2;
        if size == self.map.size {
            return;
        }
        let file = crate::mapfile::MapFile::from_world(self, "").resized(size);
        // It can't fail – `from_world` writes what `to_world` reads; if it ever did, the map stays as it is.
        if let Ok(world) = file.to_world() {
            *self = world;
        }
    }
}
