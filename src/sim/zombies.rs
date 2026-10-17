//! The horde: what a zombie notices, where it walks, whom it hits (DESIGN.md 8.1) – and how the crowd stays a
//! crowd instead of merging into one spot.
//!
//! The numbers (sight, hearing, how long a shot zombie storms, who looks for a gap …) are in `data\zombies.json`.
//! Here only what isn't a game value but how the crowd is computed.

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use super::{clear_line, push_apart, slide, World, TICK, UNIT_RADIUS};
use crate::data::{self, BreedStats};
use crate::vision::Vision;

/// How often per tick overlapping zombies are pushed apart. More passes = harder crowd, but more work.
const CROWD_PASSES: u32 = 2;
/// At most this far per pass – so a zombie in the middle of a tight crowd doesn't jump.
const MAX_UNPILE: f32 = 0.1;
/// A building tile on the way of a storming zombie costs this much instead of 1 step: their way goes around water
/// and mountains, but almost straight through walls – they tear them down instead of looking for the gap.
pub(super) const CHARGE_THROUGH: u16 = 2;
/// A zombie reaches this far past its own edge: it hits a unit or a building that is this close to its body.
/// (Its body: `radius`, per breed – a fat one reaches further.)
const ARM: f32 = 0.15;
/// This many steps of detour is a building tile worth to zombies. If the way around is longer,
/// they go right through and tear the building down (DESIGN.md 8.1).
pub(super) const BUILDING_DETOUR: u32 = 10;

pub struct Zombie {
    pub pos: Vec2,
    /// Position in the previous tick – for drawing between two ticks.
    pub prev: Vec2,
    /// Fixed number that never changes – not even when the index in the list changes.
    /// An attack order remembers this number, not the index.
    pub id: u32,
    pub(super) hp: u32,
    /// Its own speed: that of its breed, a bit faster or slower at random – so a crowd doesn't walk in step.
    pub(super) speed: f32,
    /// How big it is (tiles) – that of its breed. Kept here like `speed`: the crowd asks for it thousands of times
    /// per tick.
    pub radius: f32,
    pub(super) cooldown: u32,
    /// Ticks left in which it storms at your units because it has been shot or heard a shot (0: calm).
    pub(super) alert: u32,
    /// Ticks left in which it looks for a gap to your units instead of tearing the wall down (0: doesn't).
    pub(super) seek: u32,
    pub breed: Breed,
    /// Placed as a wanderer (editor): strolls around where it was put instead of heading for the command center –
    /// until it notices one of your units. `None`: a normal zombie.
    pub(super) wander: Option<Wander>,
}

/// What kind of zombie (DESIGN.md 8.1) – for now three. All behave the same, only their numbers differ
/// (`data\zombies.json`). `Serialize, Deserialize`: can be written into a map file (`mapfile.rs`) and named in
/// `data\spawning.json`.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum Breed {
    /// The weakest and slowest: lies scattered over the map from the start, and dead units rise as one.
    Walker,
    /// Fast and weak: the waves, everything out of the spawners. `alias`: map files from before 2026-10-16 still
    /// have the hunter in them – it is a runner now.
    #[serde(alias = "Hunter")]
    Runner,
    /// Like TAB's "Chubby": big (bigger `radius`), lots of hit points and armor, hits hard.
    Fat,
}

impl Breed {
    pub const ALL: [Breed; 3] = [Breed::Walker, Breed::Runner, Breed::Fat];

    /// Speed, hit points, armor, damage, sight and size of this breed.
    pub fn stats(self) -> &'static BreedStats {
        data::get().breed(self)
    }

    /// Its place in `ALL` – for lists with one entry per breed.
    pub fn index(self) -> usize {
        self as usize
    }
}

/// A wandering zombie: where it was put, where it's strolling to right now, and a countdown.
#[derive(Clone, Copy)]
pub(super) struct Wander {
    home: Vec2,
    /// `None`: standing around until `timer` runs out.
    pub(super) goal: Option<Vec2>,
    /// Ticks until it stops standing around – or, walking, gives up on a goal it can't reach.
    timer: u32,
}

impl Zombie {
    pub fn is_alive(&self) -> bool {
        self.hp > 0
    }

    /// Is it looking for a gap right now (shot, or standing next to one that was)?
    pub fn is_seeking(&self) -> bool {
        self.seek > 0
    }
}

/// Whom a zombie hits this tick (index in the respective list).
enum Hit {
    Unit(usize),
    Building(usize),
}

impl World {
    /// Is this zombie a wanderer (placed in the editor to stroll around)? For saving the map.
    pub fn is_wanderer(&self, zombie: &Zombie) -> bool {
        zombie.wander.is_some()
    }

    /// What the zombies see right now – only for the editor ("View: Zombies"), not used by the simulation.
    /// Expensive with many zombies, so it's only computed while someone looks at it. Looked at once per tile
    /// from its middle – many zombies share a tile, and the simulation also asks "does a unit see this tile?".
    pub fn zombie_view(&self, vision: &mut Vision) {
        // Per tile and breed – breeds see differently far.
        let mut tiles: Vec<(i32, i32, usize)> =
            self.zombies.iter().map(|z| (z.pos.x.floor() as i32, z.pos.y.floor() as i32, z.breed.index())).collect();
        tiles.sort_unstable();
        tiles.dedup();
        let eyes = tiles.into_iter().map(|(x, y, b)| (vec2(x as f32 + 0.5, y as f32 + 0.5), Breed::ALL[b].stats().sight));
        vision.update(&self.map, eyes);
    }

    /// A shot rang out at `at`, `radius` tiles loud (`noise` in `data\units.json`): every zombie that hears it comes
    /// for your units for `noise_seconds` – like in TAB, quiet rangers draw few, loud snipers many. Sound isn't
    /// stopped by forests or walls. Called right after the shot, when the grid matches the zombies.
    pub(super) fn make_noise(&mut self, at: Vec2, radius: f32) {
        let r = radius.ceil() as i32;
        let (cx, cy) = (at.x.floor() as i32, at.y.floor() as i32);
        for dy in -r..=r {
            for dx in -r..=r {
                for &z in self.grid.cell(cx + dx, cy + dy) {
                    let zombie = &mut self.zombies[z as usize];
                    if zombie.pos.distance(at) <= radius {
                        zombie.alert = zombie.alert.max(values().noise);
                    }
                }
            }
        }
    }

    /// A runner storming the command center. Only the tests use it.
    #[cfg(test)]
    pub(super) fn spawn_zombie(&mut self, pos: Vec2) {
        self.spawn_zombie_as(pos, Breed::Runner, false);
    }

    /// A zombie of this breed, wandering around `pos` or not.
    pub(super) fn spawn_zombie_as(&mut self, pos: Vec2, breed: Breed, wander: bool) {
        let stats = breed.stats();
        let speed = stats.speed * (0.8 + 0.4 * self.rng.unit_f32());
        let id = self.new_zombie_id();
        let wander = wander.then_some(Wander { home: pos, goal: None, timer: 0 });
        let radius = stats.radius;
        self.zombies.push(Zombie { pos, prev: pos, id, hp: stats.hp, speed, radius, cooldown: 0, alert: 0, seek: 0, breed, wander });
    }

    /// Zombie `z` has just been shot: it and up to `seek_helpers` of the ones standing closest to it (within
    /// `seek_radius`) look for a gap to your units for a while. The rest of the crowd doesn't change – it storms the
    /// wall. Only if `z` isn't looking yet: otherwise every shot into a crowd would add three more, and soon the whole
    /// horde would squeeze into the gap.
    pub(super) fn start_seeking(&mut self, z: usize) {
        self.zombies[z].alert = values().alert;
        if self.zombies[z].is_seeking() {
            return;
        }
        let pos = self.zombies[z].pos;
        let r = values().seek_radius.ceil() as i32;
        let (cx, cy) = (pos.x.floor() as i32, pos.y.floor() as i32);
        let mut near: Vec<(f32, usize)> = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                for &other in self.grid.cell(cx + dx, cy + dy) {
                    let other = other as usize;
                    let zombie = &self.zombies[other];
                    let d = zombie.pos.distance(pos);
                    if other != z && zombie.is_alive() && !zombie.is_seeking() && d <= values().seek_radius {
                        near.push((d, other));
                    }
                }
            }
        }
        // Closest first; equally far: the lower index (DESIGN.md 12.8).
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let helpers = near.into_iter().take(values().seek_helpers).map(|(_, i)| i);
        for i in std::iter::once(z).chain(helpers) {
            let zombie = &mut self.zombies[i];
            zombie.alert = values().alert;
            zombie.seek = values().seek;
        }
    }

    pub(super) fn new_zombie_id(&mut self) -> u32 {
        self.next_zombie_id += 1;
        self.next_zombie_id
    }

    /// Where is the zombie with this `id` in the list? `None` if it doesn't exist anymore.
    pub(super) fn find_zombie(&self, id: u32) -> Option<usize> {
        self.zombies.iter().position(|z| z.id == id && z.is_alive())
    }

    pub(super) fn move_zombies(&mut self) {
        // 1. Fill the grid and the maps anew, so that neighbours and targets are found quickly.
        self.grid.rebuild(self.zombies.iter().map(|z| z.pos));
        self.unit_grid.rebuild(self.units.iter().map(|u| u.pos));
        // Hearing (and hitting): only units on the ground – up in a tower they're out of reach.
        self.aggro.clear();
        for (i, unit) in self.units.iter().enumerate().filter(|(_, u)| !u.is_inside()) {
            self.aggro.stamp(i, unit.pos, unit.pos, values().hearing);
        }
        // The way through the gaps is only needed if someone is looking for one.
        if self.zombies.iter().any(|z| z.is_seeking()) {
            self.hunt.rebuild(&self.map, self.units.iter().map(|u| u.pos), values().seek_steps);
        }
        // Sight, once per breed – they see differently far. The units in a tower can be seen too – then the zombies
        // storm the tower.
        for breed in Breed::ALL {
            let sight = breed.stats().sight;
            self.zombie_sight[breed.index()].update(&self.map, self.units.iter().map(|u| (u.pos, sight)));
        }
        // The way to storm along is only needed if a zombie has noticed someone.
        if self.zombies.iter().any(|z| self.notices(z)) {
            self.charge.rebuild(&self.map, self.units.iter().map(|u| u.pos), values().charge_steps);
        }
        self.update_wanderers();

        // 2. For every zombie compute where it walks and whether it hits – only reading.
        let mut moves = Vec::with_capacity(self.zombies.len());
        for (i, zombie) in self.zombies.iter().enumerate() {
            let (dir, hit) = self.zombie_intent(zombie);
            let hit = hit.filter(|_| zombie.cooldown == 0);
            let push = self.zombie_separation(i, zombie);
            moves.push(((dir + push) * zombie.speed * TICK, hit));
        }

        // 3. Only now move everyone and deal damage – writing. The world border doesn't stop zombies.
        let map = &self.map;
        // Buildings whose shield breaks this tick – they let their zombies out after the loop (`infect`).
        let mut infected = Vec::new();
        for (zombie, (velocity, hit)) in self.zombies.iter_mut().zip(moves) {
            zombie.prev = zombie.pos;
            zombie.pos = slide(zombie.pos, velocity, |x, y| map.is_blocked(x, y));
            zombie.cooldown = zombie.cooldown.saturating_sub(1);
            zombie.alert = zombie.alert.saturating_sub(1);
            zombie.seek = zombie.seek.saturating_sub(1);
            match hit {
                Some(Hit::Unit(u)) => {
                    zombie.cooldown = values().hit_cooldown;
                    let unit = &mut self.units[u];
                    let damage = zombie.breed.stats().damage.saturating_sub(unit.stats().armor).max(1);
                    unit.hp = unit.hp.saturating_sub(damage);
                }
                Some(Hit::Building(b)) => {
                    zombie.cooldown = values().hit_cooldown;
                    if self.buildings[b].take_damage(zombie.breed.stats().damage) {
                        infected.push(b);
                    }
                }
                None => {}
            }
        }
        for b in infected {
            self.infect(b);
        }

        // 4. Nobody stands inside anybody else: whoever overlaps is pushed apart, like a liquid.
        for _ in 0..CROWD_PASSES {
            self.unpile_zombies();
        }
        // The grid now matches the new positions – the units look up their zombies in it afterwards.
        self.grid.rebuild(self.zombies.iter().map(|z| z.pos));

    }

    /// Wandering zombies: whoever has noticed a unit becomes a normal zombie for good and storms. The others stand around a while, then stroll to a random spot near where they were put.
    /// Here, not in `zombie_intent`, because it draws random numbers – and that changes the generator (writing).
    fn update_wanderers(&mut self) {
        for i in 0..self.zombies.len() {
            let Some(mut wander) = self.zombies[i].wander else {
                continue;
            };
            let zombie = &self.zombies[i];
            if self.notices(zombie) {
                self.zombies[i].wander = None;
                continue;
            }
            wander.timer = wander.timer.saturating_sub(1);
            let arrived = wander.goal.is_some_and(|goal| goal.distance(zombie.pos) < 0.3);
            if wander.goal.is_some() && (arrived || wander.timer == 0) {
                // There (or stuck): stand around for 1–5 s.
                wander.goal = None;
                wander.timer = 20 + self.rng.below(80);
            } else if wander.goal.is_none() && wander.timer == 0 {
                // On to a random free spot near home – at most 8 s, then it gives up.
                for _ in 0..5 {
                    let offset = vec2(self.rng.unit_f32() * 2.0 - 1.0, self.rng.unit_f32() * 2.0 - 1.0) * values().wander_radius;
                    let spot = wander.home + offset;
                    if !self.map.is_blocked(spot.x.floor() as i32, spot.y.floor() as i32) {
                        wander.goal = Some(spot);
                        wander.timer = 160;
                        break;
                    }
                }
            }
            self.zombies[i].wander = Some(wander);
        }
    }

    /// Has this zombie noticed one of your units – sees one, hears one close by, has just been shot or heard a shot?
    fn notices(&self, zombie: &Zombie) -> bool {
        zombie.alert > 0
            || self.aggro.near(zombie.pos).is_some()
            || self.zombie_sight[zombie.breed.index()].sees(zombie.pos)
    }

    /// What a zombie wants this tick: where to walk (length 1, slower: shorter, standing: 0) and whom to hit.
    /// In this order (DESIGN.md 8.1):
    /// 1. A unit within reach: hit it – everyone does that.
    /// 2. Looking for a gap (shot, or next to one that was) with a unit in reach on foot, around walls: walk the way
    ///    to it.
    /// 3. Noticed a unit (sees it, hears it, has been shot): storm at it – around water and mountains, but straight
    ///    through walls. If the next step is a building, tear it down.
    /// 4. A building nearby: tear it down.
    /// 5. A wanderer: stroll around where it was put.
    /// 6. Nothing nearby: follow the flow field to the command center – right through buildings if the
    ///    detour is too long. Then the building soon becomes the nearest target (point 4).
    fn zombie_intent(&self, zombie: &Zombie) -> (Vec2, Option<Hit>) {
        let pos = zombie.pos;
        if let Some(u) = self.aggro.near(pos) {
            if self.units[u].pos.distance(pos) <= UNIT_RADIUS + zombie.radius + ARM {
                return (Vec2::ZERO, Some(Hit::Unit(u)));
            }
        }

        if zombie.is_seeking() {
            if let Some((u, step)) = self.hunt.track(&self.map, pos) {
                // Clear way: straight there. Otherwise follow the way around the obstacles.
                let target = self.units[u].pos;
                let dir = if step == Vec2::ZERO || clear_line(&self.map, pos, target) {
                    (target - pos).normalize_or_zero()
                } else {
                    step
                };
                return (dir, None);
            }
        }

        if self.notices(zombie) {
            if let Some((u, step)) = self.charge.track(&self.map, pos) {
                // The unit sits in a tower that the zombie touches: hit the tower.
                let tower = self.units[u].inside.and_then(|id| self.find_building(id));
                if let Some(b) = tower.filter(|&b| self.buildings[b].distance_to(pos) <= zombie.radius + ARM) {
                    return (Vec2::ZERO, Some(Hit::Building(b)));
                }
                // Clear way: straight there. Otherwise one step along the way.
                let target = self.units[u].pos;
                if step == Vec2::ZERO || clear_line(&self.map, pos, target) {
                    return ((target - pos).normalize_or_zero(), None);
                }
                // The step leads onto a building (a wall in the way, or the tower the unit sits in): hit it as soon
                // as you touch it.
                let next = (pos.x.floor() as i32 + step.x.round() as i32, pos.y.floor() as i32 + step.y.round() as i32);
                if self.building_on_tile(next.0, next.1).is_some() {
                    if let Some(touching) = self.touching_building(zombie) {
                        return (Vec2::ZERO, Some(Hit::Building(touching)));
                    }
                }
                return (step, None);
            }
        }

        if let Some(b) = self.building_aggro.near(pos) {
            // Already standing at a wall: hit that one – otherwise to the nearest spot of the building.
            if let Some(touching) = self.touching_building(zombie) {
                return (Vec2::ZERO, Some(Hit::Building(touching)));
            }
            let b = &self.buildings[b];
            let closest = pos.clamp(b.corner(), b.far_corner());
            return ((closest - pos).normalize_or_zero(), None);
        }

        // A wanderer strolls slowly to its spot – or just stands there.
        if let Some(wander) = zombie.wander {
            let dir = wander.goal.map_or(Vec2::ZERO, |goal| (goal - pos).normalize_or_zero());
            return (dir * values().wander_pace, None);
        }

        let dir = self.to_base.direction(pos);
        let dir = if dir == Vec2::ZERO { (self.base - pos).normalize_or_zero() } else { dir };
        (dir, None)
    }

    /// Does this zombie touch a building? Only looks into the tiles around it.
    fn touching_building(&self, zombie: &Zombie) -> Option<usize> {
        let pos = zombie.pos;
        let (cx, cy) = (pos.x.floor() as i32, pos.y.floor() as i32);
        for dy in -1..=1 {
            for dx in -1..=1 {
                if let Some(b) = self.building_on_tile(cx + dx, cy + dy) {
                    if self.buildings[b].distance_to(pos) <= zombie.radius + ARM {
                        return Some(b);
                    }
                }
            }
        }
        None
    }

    /// The crowd can't be squeezed: every pair of zombies that overlaps is pushed apart by half the overlap each.
    /// `zombie_separation` is only a soft push – against a zombie walking at full speed it loses, so the ones
    /// behind slid into the ones in front. This correction moves the positions directly. Whoever presses from
    /// behind shoves the front ones sideways – the horde piles up in front of a gap and flows like water.
    /// First all corrections are computed (only reading), then applied – so the order in the list doesn't matter.
    fn unpile_zombies(&mut self) {
        self.grid.rebuild(self.zombies.iter().map(|z| z.pos));
        let mut fixes = Vec::with_capacity(self.zombies.len());
        for (me, zombie) in self.zombies.iter().enumerate() {
            let pos = zombie.pos;
            let (cx, cy) = (pos.x.floor() as i32, pos.y.floor() as i32);
            let mut fix = Vec2::ZERO;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    for &other in self.grid.cell(cx + dx, cy + dy) {
                        if other as usize == me {
                            continue;
                        }
                        let other = &self.zombies[other as usize];
                        // Bodies touch at the sum of both radii. Even two fat ones touch closer than one tile, so
                        // looking into the 3 × 3 tiles around is still enough.
                        let min_dist = zombie.radius + other.radius;
                        let away = pos - other.pos;
                        let d = away.length();
                        if d >= min_dist {
                            continue;
                        }
                        // Exactly on top of each other: no direction to push in. Then the numbers decide –
                        // the two get opposite directions, the same ones every time (deterministic).
                        let dir = if d > 0.0001 { away / d } else { overlap_direction(zombie.id, other.id) };
                        fix += dir * (min_dist - d) * 0.5;
                    }
                }
            }
            fixes.push(fix.clamp_length_max(MAX_UNPILE));
        }
        let map = &self.map;
        for (zombie, fix) in self.zombies.iter_mut().zip(fixes) {
            zombie.pos = slide(zombie.pos, fix, |x, y| map.is_blocked(x, y));
        }
    }

    /// Pushes overlapping zombies apart – and away from units.
    fn zombie_separation(&self, me: usize, zombie: &Zombie) -> Vec2 {
        let (pos, radius) = (zombie.pos, zombie.radius);
        let mut push = Vec2::ZERO;
        let (cx, cy) = (pos.x.floor() as i32, pos.y.floor() as i32);
        for dy in -1..=1 {
            for dx in -1..=1 {
                for &other in self.grid.cell(cx + dx, cy + dy) {
                    let other = other as usize;
                    if other != me {
                        let other = &self.zombies[other];
                        push += push_apart(pos, other.pos, radius + other.radius);
                    }
                }
                for &u in self.unit_grid.cell(cx + dx, cy + dy) {
                    let unit = &self.units[u as usize];
                    if !unit.is_inside() {
                        push += push_apart(pos, unit.pos, radius + UNIT_RADIUS);
                    }
                }
            }
        }
        push
    }
}

/// The zombies' values from `data\zombies.json`.
pub(super) fn values() -> &'static data::Zombies {
    &data::get().zombies
}

/// Direction for two zombies standing exactly on top of each other: one of 8 directions, picked from both
/// numbers, and the one with the smaller number gets the opposite – so they move apart instead of together.
/// A table instead of `sin`/`cos`: those may round differently on another PC, and co-op needs the same result.
fn overlap_direction(me: u32, other: u32) -> Vec2 {
    const D: f32 = std::f32::consts::FRAC_1_SQRT_2;
    const DIRS: [Vec2; 8] = [
        vec2(1.0, 0.0), vec2(D, D), vec2(0.0, 1.0), vec2(-D, D),
        vec2(-1.0, 0.0), vec2(-D, -D), vec2(0.0, -1.0), vec2(D, -D),
    ];
    let (low, high) = (me.min(other), me.max(other));
    let dir = DIRS[(low.wrapping_mul(7919) ^ high.wrapping_mul(104_729)) as usize % 8];
    if me == low { -dir } else { dir }
}
