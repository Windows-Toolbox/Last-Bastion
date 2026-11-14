//! The editor's copy, cut and paste (Ctrl+C, Ctrl+X, Ctrl+V – TAB's editor copies one entity, this copies whatever is
//! selected). A copy (`Clip`) keeps every thing the way the map file does (`mapfile::BuildingData` …, `of` / `add_to`):
//! its kind, where it is and everything the editor gave it – hit points, infected or veteran, owner, tag (name ID, name,
//! text, minimap mark), the script it runs when it joins; a spawner its size, axes and mode; a house of doom how many
//! are still inside. So whatever the file learns to keep later, a copy keeps too.
//!
//! Copying only reads the world (`World::copy`); pasting and cutting are orders (`Order::Paste`, `Order::RemovePieces`),
//! so undo and redo take them back like every other change.

use macroquad::math::{vec2, Vec2};

use super::{Axes, Ornament, Thing, ThingRef, World};
use crate::kinds::BuildingKind;
use crate::map::{BigRock, Terrain};
use crate::mapfile::{BuildingData, DoomData, LootData, NeutralData, SpawnerData, UnitData, ZombieData};

/// Something on the map the editor can select, copy and remove.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Piece {
    Building(u32),
    Unit(u32),
    Neutral(u32),
    /// An awake zombie (its `id`).
    Zombie(u32),
    /// A sleeping zombie (`sleep.rs`) – sleepers have no number, it's known by the spot it lies on. In Edit no time
    /// passes, so it stays there.
    Sleeper(Vec2),
    Doom(u32),
    Loot(u32),
    Spawner(u32),
    /// The decoration: TAB's big rocks …
    BigRock(BigRock),
    /// … and the ruins and power line towers.
    Ornament(Ornament),
}

/// A copy of things on the map, with everything they have.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clip {
    pub buildings: Vec<BuildingData>,
    pub units: Vec<UnitData>,
    pub neutral: Vec<NeutralData>,
    pub zombies: Vec<ZombieData>,
    pub doom: Vec<DoomData>,
    pub loot: Vec<LootData>,
    pub spawners: Vec<SpawnerData>,
    pub big_rocks: Vec<BigRock>,
    pub ornaments: Vec<Ornament>,
    /// The tile in the middle of it all – pasted, that one lands on the tile under the mouse, the rest around it as
    /// it was.
    pub middle: (i32, i32),
}

impl Clip {
    /// "2 buildings, 1 unit, 40 zombies" – or "nothing".
    pub fn describe(&self) -> String {
        let counts = [
            (self.buildings.len(), "building", "buildings"),
            (self.units.len(), "unit", "units"),
            (self.neutral.len(), "neutral thing", "neutral things"),
            (self.zombies.len(), "zombie", "zombies"),
            (self.doom.len(), "house of doom", "houses of doom"),
            (self.loot.len(), "loot", "loot"),
            (self.spawners.len(), "spawner", "spawners"),
            (self.big_rocks.len() + self.ornaments.len(), "decoration", "decorations"),
        ];
        counted(&counts)
    }

    pub fn is_empty(&self) -> bool {
        self.buildings.is_empty()
            && self.units.is_empty()
            && self.neutral.is_empty()
            && self.zombies.is_empty()
            && self.doom.is_empty()
            && self.loot.is_empty()
            && self.spawners.is_empty()
            && self.big_rocks.is_empty()
            && self.ornaments.is_empty()
    }
}

/// "3 units, 1 house of doom" from (how many, one, more than one) – those with none are left out.
fn counted(counts: &[(usize, &str, &str)]) -> String {
    let parts: Vec<String> =
        counts.iter().filter(|(n, ..)| *n > 0).map(|&(n, one, many)| format!("{n} {}", if n == 1 { one } else { many })).collect();
    if parts.is_empty() { "nothing".to_owned() } else { parts.join(", ") }
}

impl World {
    /// A copy of these pieces – the ones that are gone already are left out.
    pub fn copy(&self, pieces: &[Piece]) -> Clip {
        let mut clip = Clip::default();
        // Every thing's middle, for the middle of all.
        let mut points: Vec<Vec2> = Vec::new();
        for &piece in pieces {
            match piece {
                Piece::Building(id) => {
                    if let Some(b) = self.find_building(id) {
                        clip.buildings.push(BuildingData::of(&self.buildings[b]));
                        points.push(self.buildings[b].center());
                    }
                }
                Piece::Unit(id) => {
                    if let Some(u) = self.find_unit(id) {
                        clip.units.push(UnitData::of(&self.units[u]));
                        points.push(self.units[u].pos);
                    }
                }
                Piece::Neutral(id) => {
                    if let Some(n) = self.find_neutral(id) {
                        clip.neutral.push(NeutralData::of(&self.neutral[n]));
                        points.push(self.neutral[n].center());
                    }
                }
                Piece::Zombie(id) => {
                    if let Some(z) = self.find_zombie(id) {
                        clip.zombies.push(ZombieData::awake(self, &self.zombies[z]));
                        points.push(self.zombies[z].pos);
                    }
                }
                Piece::Sleeper(pos) => {
                    if let Some(s) = self.sleep.iter().find(|s| s.pos == pos) {
                        clip.zombies.push(ZombieData::asleep(s));
                        points.push(pos);
                    }
                }
                Piece::Doom(id) => {
                    if let Some(d) = self.find_doom(id) {
                        clip.doom.push(DoomData::of(&self.doom[d]));
                        points.push(self.doom[d].center());
                    }
                }
                Piece::Loot(id) => {
                    if let Some(l) = self.find_loot(id) {
                        clip.loot.push(LootData::of(&self.loot[l]));
                        points.push(self.loot[l].pos);
                    }
                }
                Piece::Spawner(id) => {
                    if let Some(s) = self.find_spawner(id) {
                        clip.spawners.push(SpawnerData::of(&self.spawners[s]));
                        points.push(self.spawners[s].center());
                    }
                }
                Piece::BigRock(rock) => {
                    if self.map.big_rocks().contains(&rock) {
                        clip.big_rocks.push(rock);
                        points.push(vec2(rock.tile.0 as f32, rock.tile.1 as f32) + rock.size as f32 / 2.0);
                    }
                }
                Piece::Ornament(ornament) => {
                    if self.ornaments.contains(&ornament) {
                        clip.ornaments.push(ornament);
                        points.push(ornament_middle(ornament));
                    }
                }
            }
        }
        // The middle of the rectangle around them all.
        if let Some(&first) = points.first() {
            let (min, max) = points.iter().fold((first, first), |(min, max), &p| (min.min(p), max.max(p)));
            let middle = (min + max) / 2.0;
            clip.middle = (middle.x.floor() as i32, middle.y.floor() as i32);
        }
        clip
    }

    /// `Order::Paste`: the copy again, its middle on tile `to`. Everything goes where it fits by the same rules as
    /// placing it by hand – what doesn't fit (a building on another one, a zombie in the water, a house of doom outside
    /// the border) is left out. A second command center neither: the colony has one, like in TAB.
    pub(super) fn paste(&mut self, clip: &Clip, to: (i32, i32)) {
        let (dx, dy) = (to.0 - clip.middle.0, to.1 - clip.middle.1);
        let tile = |t: (i32, i32)| (t.0 + dx, t.1 + dy);
        let point = |p: (f32, f32)| (p.0 + dx as f32, p.1 + dy as f32);
        // First the rocks – they're ground, the rest stands on it.
        for rock in &clip.big_rocks {
            if self.map.place_big_rock(BigRock { tile: tile(rock.tile), ..*rock }) {
                self.map_changed = true;
            }
        }
        for b in &clip.buildings {
            let has_cc = self.buildings.iter().any(|other| other.kind == BuildingKind::CommandCenter);
            if b.kind == BuildingKind::CommandCenter && has_cc {
                continue;
            }
            BuildingData { tile: tile(b.tile), ..b.clone() }.add_to(self);
        }
        for d in &clip.doom {
            let moved = DoomData { house: tile(d.house), ..d.clone() };
            if self.doom_fits(moved.house, moved.size) {
                moved.add_to(self);
            }
        }
        for n in &clip.neutral {
            let thing = match n.thing {
                Thing::Building { kind, tile: t, turned } => Thing::Building { kind, tile: tile(t), turned },
                Thing::Unit { kind, pos } => {
                    let pos = point(pos);
                    if self.map.blocks_unit(pos.0.floor() as i32, pos.1.floor() as i32) {
                        continue;
                    }
                    Thing::Unit { kind, pos }
                }
            };
            NeutralData { thing, ..n.clone() }.add_to(self);
        }
        for u in &clip.units {
            let pos = point(u.pos);
            if !self.map.blocks_unit(pos.0.floor() as i32, pos.1.floor() as i32) {
                UnitData { pos, ..u.clone() }.add_to(self);
            }
        }
        // Like the zombie brush: not on blocked tiles (water, a building) and not off the map.
        for z in &clip.zombies {
            let pos = point(z.pos);
            if !self.map.is_blocked(pos.0.floor() as i32, pos.1.floor() as i32) {
                ZombieData { pos, ..z.clone() }.add_to(self);
            }
        }
        // Like "Loot": inside the border, on open ground.
        for l in &clip.loot {
            let pos = point(l.pos);
            let (x, y) = (pos.0.floor() as i32, pos.1.floor() as i32);
            if self.map.in_play(x, y) && !self.map.is_blocked(x, y) {
                LootData { pos, ..l.clone() }.add_to(self);
            }
        }
        for ornament in &clip.ornaments {
            self.ornaments.push(Ornament { tile: tile(ornament.tile), ..*ornament });
        }
        // A new number each (`id` 0); the corner moves in the spawner's own axes – on screen a tile is a step of
        // (1, 1) or (−1, 1) there (`map::to_frame`).
        for s in &clip.spawners {
            let (fx, fy) = match s.axes {
                Axes::Screen => (dx - dy, dx + dy),
                Axes::Tiles => (dx, dy),
            };
            SpawnerData { id: 0, min: (s.min.0 + fx, s.min.1 + fy), ..s.clone() }.add_to(self);
        }
    }

    /// `Order::RemovePieces`: these pieces off the map (Ctrl+X, Del). A big rock's tiles become grass.
    pub(super) fn remove_pieces(&mut self, pieces: &[Piece]) {
        // The sleepers by their spot – sorted, so each one is found quickly (a cut can take thousands).
        let key = |p: Vec2| (p.x.to_bits(), p.y.to_bits());
        let mut sleepers: Vec<(u32, u32)> = pieces.iter().filter_map(|p| if let Piece::Sleeper(pos) = p { Some(key(*pos)) } else { None }).collect();
        sleepers.sort_unstable();
        if !sleepers.is_empty() {
            self.sleep.remove(|p| sleepers.binary_search(&key(p)).is_ok());
        }
        for &piece in pieces {
            match piece {
                Piece::Building(id) => self.take_down(id),
                Piece::Unit(id) => self.remove_thing(ThingRef::Unit(id)),
                Piece::Neutral(id) => self.remove_thing(ThingRef::Neutral(id)),
                Piece::Zombie(id) => self.zombies.retain(|z| z.id != id),
                Piece::Sleeper(_) => {}
                Piece::Doom(id) => {
                    if let Some(d) = self.find_doom(id) {
                        let house = self.doom.remove(d);
                        self.map.set_doom(house.tile.0, house.tile.1, house.size, false);
                        self.map_changed = true;
                    }
                }
                Piece::Loot(id) => self.loot.retain(|l| l.id != id),
                Piece::Spawner(id) => {
                    if self.find_spawner(id).is_some() {
                        self.spawners.retain(|s| s.id != id);
                        self.forget_spawner(id);
                    }
                }
                Piece::BigRock(rock) => {
                    if self.map.big_rocks().contains(&rock) {
                        for (x, y) in rock.tiles() {
                            self.map.set_terrain(x, y, Terrain::Grass);
                        }
                        self.map_changed = true;
                    }
                }
                Piece::Ornament(ornament) => self.ornaments.retain(|o| *o != ornament),
            }
        }
    }
}

/// What these pieces are, counted: "3 units, 40 zombies, 1 house of doom" – for the status line, without looking at
/// the world.
pub fn count_pieces(pieces: &[Piece]) -> String {
    let mut n = [0usize; 8];
    for piece in pieces {
        n[match piece {
            Piece::Building(_) => 0,
            Piece::Unit(_) => 1,
            Piece::Neutral(_) => 2,
            Piece::Zombie(_) | Piece::Sleeper(_) => 3,
            Piece::Doom(_) => 4,
            Piece::Loot(_) => 5,
            Piece::Spawner(_) => 6,
            Piece::BigRock(_) | Piece::Ornament(_) => 7,
        }] += 1;
    }
    counted(&[
        (n[0], "building", "buildings"),
        (n[1], "unit", "units"),
        (n[2], "neutral thing", "neutral things"),
        (n[3], "zombie", "zombies"),
        (n[4], "house of doom", "houses of doom"),
        (n[5], "loot", "loot"),
        (n[6], "spawner", "spawners"),
        (n[7], "decoration", "decorations"),
    ])
}

/// The middle of an ornament on the ground: of the tiles it covers (a ruin's 4 × 4, a tower's one).
pub fn ornament_middle(ornament: Ornament) -> Vec2 {
    let (w, h) = ornament.kind.footprint();
    vec2(ornament.tile.0 as f32 + w as f32 / 2.0, ornament.tile.1 as f32 + h as f32 / 2.0)
}
