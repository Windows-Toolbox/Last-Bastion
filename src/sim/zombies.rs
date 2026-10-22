//! The horde: what a zombie notices, where it walks, whom it hits (DESIGN.md 8.1) – and how the crowd stays a
//! crowd instead of merging into one spot.
//!
//! A zombie remembers where it was lured to (`Lure`): a sound it heard (`sound.rs`), or where it last saw one of your
//! units. It goes there; arrived and nobody there, it stays and strolls around – only the waves (`siege`) head on for
//! the command center.
//!
//! The numbers (sight, hearing, how long a shot zombie storms, who looks for a gap …) are in `data\zombies.json`.
//! Here only what isn't a game value but how the crowd is computed.
//!
//! **Calm zombies are cheap** (`is_calm`): most of the horde only strolls around its place, far from everything. Such a
//! zombie thinks anew only every `THINK_EVERY` ticks – each on another tick, by its number – and walks on with what it
//! decided in between. It still looks every tick whether it notices anything: then it isn't calm anymore and thinks
//! every tick again. That's what makes tens of thousands of them possible.
//!
//! **Thinking side by side:** what every zombie wants (`zombie_intent`) and how far it is pushed apart from the others
//! (`unpile_zombies`) are worked out on all cores of the processor at once (`rayon`) – each zombie only reads, nobody
//! writes. Every one comes out exactly as if worked out one after the other, collected in the order of the list – the
//! same on every PC, however many cores it has.

use macroquad::math::{vec2, Vec2};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::sound::{cell_xy, CELL};
use super::{clear_line, keeps_out, push_apart, slide, World, TICK, TICKS_PER_SECOND, UNIT_RADIUS};
use crate::data::{self, BreedStats};
use crate::hunt::Through;
use crate::lure::LureField;
use crate::vision::Vision;

/// How often per tick overlapping zombies are pushed apart. More passes = harder crowd, but more work.
const CROWD_PASSES: u32 = 2;
/// A calm zombie (`is_calm`) thinks anew only every this many ticks – 0.4 s; in between it walks on as it decided.
const THINK_EVERY: u64 = 8;
/// Every this many ticks the list of zombies is sorted by where they stand (`sort_by_place`) – 2 s.
pub(super) const SORT_EVERY: u64 = 40;
/// Thinking side by side: each core takes at least this many zombies at a time – fewer, and handing them out would
/// cost more than it saves …
const PER_CORE: usize = 1024;
/// … and only with at least this many busy ones (not resting) at all – waking the other cores costs more than a few
/// thousand zombies' thinking. One core or many: the result is the same.
const SIDE_BY_SIDE_FROM: usize = 4096;
/// At most this far per pass – so a zombie in the middle of a tight crowd doesn't jump.
const MAX_UNPILE: f32 = 0.1;
/// A building tile on the way of a storming zombie costs this much instead of 1 step: their way goes around water
/// and mountains, but almost straight through walls – they tear them down instead of looking for the gap.
pub(super) const CHARGE_THROUGH: u16 = 2;
/// A zombie reaches this far past its own edge: it hits a unit or a building that is this close to its body.
/// (Its body: `radius`, per breed – a fat one reaches further.)
const ARM: f32 = 0.075;
/// This many steps of detour is a building tile worth to zombies. If the way around is longer,
/// they go right through and tear the building down (DESIGN.md 8.1).
pub(super) const BUILDING_DETOUR: u32 = 10;
/// A lured zombie this close to where it was lured to has arrived (tiles).
const LURE_ARRIVED: f32 = 1.5;
/// It gives up after this many seconds if it isn't there yet (stuck somewhere) – and stays where it is.
const LURE_PATIENCE: f32 = 60.0;
/// The way to a lure place is worked out this far around it (tiles, `LureField`) – lured zombies heard or saw it from
/// closer than that. Further away they walk straight at it.
const LURE_REACH: i32 = 40;
/// At most this many new ways to lure places per tick – the others walk straight until it's their turn.
const NEW_LURE_FIELDS: usize = 8;
/// A way nobody needed for this long (seconds) is thrown away.
const LURE_FIELD_KEPT: f32 = 10.0;

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
    /// Ticks left in which it storms at your units because it has been shot (0: calm).
    pub(super) alert: u32,
    /// Ticks left in which it looks for a gap to your units instead of tearing the wall down (0: doesn't).
    pub(super) seek: u32,
    pub breed: Breed,
    /// Strolls around a place: a roamer where it lay at the start, anyone who arrived where it was lured to. `None`:
    /// on its way somewhere.
    pub(super) wander: Option<Wander>,
    /// Where it was lured to – a sound, or where it last saw one of your units – and is heading for now.
    pub(super) lure: Option<Lure>,
    /// With nothing else to do it heads for the command center (the waves, the dead rising, the colonists of an infected
    /// house). Otherwise it stays where it ends up.
    pub(super) siege: bool,
    /// Came with the final wave (or was called by it) – the round is won once none of these is left (`round.rs`).
    pub final_wave: bool,
    /// Its step per tick as it last decided – a calm zombie walks on with it until it thinks again (`is_calm`).
    pub(super) step: Vec2,
    /// Ticks it has been calm in a row – long enough, and far from your eyes, it falls asleep (`sleep.rs`).
    pub(super) calm_for: u32,
}

/// Where a zombie was lured to.
#[derive(Clone, Copy)]
pub(super) struct Lure {
    pub(super) to: Vec2,
    /// How loud the sound was there – a louder one may draw it elsewhere, a quieter one doesn't. Seen with its own eyes:
    /// the most.
    pub(super) loud: u32,
    /// It gives up at this tick, if it isn't there yet.
    until: u64,
}

impl Lure {
    pub(super) fn new(to: Vec2, loud: u32, tick: u64) -> Lure {
        Lure { to, loud, until: tick + (LURE_PATIENCE * TICKS_PER_SECOND) as u64 }
    }

    /// Where one of your units was seen – nothing draws a zombie more.
    pub(super) fn seen(to: Vec2, tick: u64) -> Lure {
        Lure::new(to, u32::MAX, tick)
    }
}

/// What kind of zombie (DESIGN.md 8.1). Most of them only differ in their numbers (`data\zombies.json`); the ranged
/// ones (`range`) attack from afar (`ranged.rs`). `Serialize, Deserialize`: can be written into a map file
/// (`mapfile.rs`) and named in `data\spawning.json` – by these names, whatever `name` the game shows.
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
    /// TAB's armored runner: as fast as a runner, but armor eats most of an arrow. Shown as "Riot" (`name`).
    Armored,
    /// Like TAB's venom: spits acid a few tiles far – it lands a moment later and burns everyone around the spot.
    Spitter,
    /// Like StarCraft 2's infested marine: shoots back, just a little less far than your soldiers.
    Infested,
}

impl Breed {
    pub const ALL: [Breed; 6] = [Breed::Walker, Breed::Runner, Breed::Fat, Breed::Armored, Breed::Spitter, Breed::Infested];

    /// Speed, hit points, armor, damage, sight and size of this breed.
    pub fn stats(self) -> &'static BreedStats {
        data::get().breed(self)
    }

    /// Its place in `ALL` – for lists with one entry per breed.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The name the game shows – from `data\zombies.json`. `&'static`: the values live as long as the program
    /// (`data::get`).
    pub fn name(self) -> &'static str {
        &self.stats().name
    }

    /// Does it attack from afar (`range` in `data\zombies.json`)?
    pub fn is_ranged(self) -> bool {
        self.stats().range > 0.0
    }

    /// Ticks between two of its attacks: its own `attack_seconds`, otherwise `hit_every_seconds` of all zombies.
    pub fn attack_ticks(self) -> u32 {
        self.stats().attack.unwrap_or(values().hit_cooldown).max(1)
    }
}

/// A wandering zombie: where it's strolling to right now, and a countdown. No home: every trip starts where it stands.
#[derive(Clone, Copy)]
pub(super) struct Wander {
    /// `None`: standing around until `timer` runs out.
    pub(super) goal: Option<Vec2>,
    /// Ticks until it stops standing around – or, walking, gives up on a goal it can't reach.
    timer: u32,
}

impl Wander {
    /// Strolling, starting by standing where it is.
    fn new() -> Wander {
        Wander { goal: None, timer: 0 }
    }

    /// Strolling, on its way to `goal` – it gives up after `timer` ticks (a sleeper that wakes on a trip, `sleep.rs`).
    pub(super) fn towards(goal: Vec2, timer: u32) -> Wander {
        Wander { goal: Some(goal), timer }
    }
}

impl Zombie {
    pub fn is_alive(&self) -> bool {
        self.hp > 0
    }

    /// Hit points left – for the panel when it's clicked.
    pub fn hp(&self) -> u32 {
        self.hp
    }

    /// Its own speed (tiles per second) – for the panel when it's clicked.
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// Right after a hit: how far it is through the pause until the next one (0 = it just hit, towards 1 = ready
    /// again). For the picture: one swing of the attack animation per hit. `None`: it hasn't hit for a while.
    pub fn since_hit(&self) -> Option<f32> {
        (self.cooldown > 0).then(|| 1.0 - self.cooldown as f32 / self.breed.attack_ticks() as f32)
    }

    /// Is it looking for a gap right now (shot, or standing next to one that was)?
    pub fn is_seeking(&self) -> bool {
        self.seek > 0
    }

    /// Where it was lured to – for the panel and the editor.
    pub fn lured_to(&self) -> Option<Vec2> {
        self.lure.map(|lure| lure.to)
    }
}

/// Whom a zombie hits this tick (index in the respective list) – a ranged one shoots at it instead (`ranged.rs`).
#[derive(Clone, Copy)]
pub(super) enum Hit {
    Unit(usize),
    Building(usize),
}

impl World {
    /// Is this zombie a wanderer – does it stay where it ends up instead of heading for the command center? For saving
    /// the map.
    pub fn is_wanderer(&self, zombie: &Zombie) -> bool {
        !zombie.siege
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

    /// A runner storming the command center. Only the tests use it.
    #[cfg(test)]
    pub(super) fn spawn_zombie(&mut self, pos: Vec2) {
        self.spawn_zombie_as(pos, Breed::Runner, false);
    }

    /// A zombie of this breed: wandering around `pos` and staying there (a roamer), or heading for the command center
    /// (`siege` – the waves).
    pub(super) fn spawn_zombie_as(&mut self, pos: Vec2, breed: Breed, wander: bool) {
        let stats = breed.stats();
        let speed = stats.speed * (0.8 + 0.4 * self.rng.unit_f32());
        let id = self.new_zombie_id();
        let siege = !wander;
        let wander = wander.then_some(Wander::new());
        let radius = stats.radius;
        self.zombies.push(Zombie {
            pos,
            prev: pos,
            id,
            hp: stats.hp,
            speed,
            radius,
            cooldown: 0,
            alert: 0,
            seek: 0,
            breed,
            wander,
            lure: None,
            siege,
            final_wave: false,
            step: Vec2::ZERO,
            calm_for: 0,
        });
    }

    /// A zombie lured to `to` right away (out of a house of doom, out of the ground around loot): it goes there and
    /// stays.
    pub(super) fn spawn_lured(&mut self, pos: Vec2, breed: Breed, lure: Lure) {
        self.spawn_zombie_as(pos, breed, false);
        if let Some(zombie) = self.zombies.last_mut() {
            zombie.siege = false;
            zombie.lure = Some(lure);
        }
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
                for other in self.grid.cell(cx + dx, cy + dy) {
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

    /// Sorts the list of zombies by where they stand: block of 8 × 8 tiles after block, inside one tile after tile,
    /// equal ones by number – the same order on every PC. Then neighbours on the map are neighbours in memory too, and
    /// looking at the zombies around one is much quicker with a hundred thousand of them: the processor finds them in
    /// its cache instead of fetching each one from far away. Every `SORT_EVERY` ticks, at the end of a tick – an index
    /// into the list only counts within one tick (everything kept longer goes by `id`).
    pub(super) fn sort_by_place(&mut self) {
        let n = self.map.size as i32;
        let blocks = (n + 7) / 8;
        // `cached`: each zombie's place is worked out once, and every zombie is moved only once.
        self.zombies.sort_by_cached_key(|z| {
            let (x, y) = ((z.pos.x.floor() as i32).clamp(0, n - 1), (z.pos.y.floor() as i32).clamp(0, n - 1));
            let block = (y / 8 * blocks + x / 8) as u32;
            let inside = ((y % 8) * 8 + x % 8) as u32;
            (block, inside, z.id)
        });
    }

    /// Fills the zombie grid anew from where they stand now – with their bodies (`grid.rs`).
    pub(super) fn fill_zombie_grid(&mut self) {
        self.grid.rebuild(self.zombies.iter().map(|z| (z.pos, z.radius, z.id)));
    }

    pub(super) fn new_zombie_id(&mut self) -> u32 {
        self.next_zombie_id += 1;
        self.next_zombie_id
    }

    /// Where is the zombie with this `id` in the list? `None` if it doesn't exist anymore.
    pub fn find_zombie(&self, id: u32) -> Option<usize> {
        self.zombies.iter().position(|z| z.id == id && z.is_alive())
    }

    pub(super) fn move_zombies(&mut self) {
        // 1. Fill the grid and the maps anew, so that neighbours and targets are found quickly.
        self.fill_zombie_grid();
        self.unit_grid.rebuild(self.units.iter().map(|u| (u.pos, UNIT_RADIUS, u.id)));
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
        // Who has noticed one of your units – asked once per zombie, the answer is used again and again below. The way
        // to storm along is only needed if one has.
        let noticing: Vec<bool> = self.zombies.iter().map(|z| self.notices(z)).collect();
        if noticing.contains(&true) {
            self.charge.rebuild(&self.map, self.units.iter().map(|u| u.pos), values().charge_steps);
        }
        self.update_minds(&noticing);
        self.prepare_lure_fields();
        // Who is calm (`is_calm`) – and who of them rests this tick: it isn't its tick to think.
        let calm: Vec<bool> = self.zombies.iter().zip(&noticing).map(|(z, &n)| !n && self.is_calm(z)).collect();
        let resting: Vec<bool> = self.zombies.iter().zip(&calm).map(|(z, &c)| c && !self.thinks_now(z)).collect();

        // 2. For every zombie compute where it walks and whether it hits – only reading, so with many busy ones on all
        // cores at once (`par_iter`, see the top of the file). A resting one walks on as it decided.
        let world = &*self;
        let think = |(i, zombie): (usize, &Zombie)| {
            if resting[i] {
                return (zombie.step, None, None);
            }
            let (dir, hit, seen) = world.zombie_intent(zombie, noticing[i]);
            let hit = hit.filter(|_| zombie.cooldown == 0);
            let push = world.zombie_separation(i, zombie);
            ((dir + push) * zombie.speed * TICK, hit, seen)
        };
        let busy = resting.iter().filter(|&&r| !r).count();
        let moves: Vec<(Vec2, Option<Hit>, Option<Vec2>)> = if busy >= SIDE_BY_SIDE_FROM {
            world.zombies.par_iter().enumerate().with_min_len(PER_CORE).map(think).collect()
        } else {
            world.zombies.iter().enumerate().map(think).collect()
        };

        // 3. Only now move everyone and deal damage – writing. The world border doesn't stop zombies.
        let map = &self.map;
        let tick = self.tick;
        // Buildings whose shield breaks this tick – they let their zombies out after the loop (`infect`).
        let mut infected = Vec::new();
        // The shots of the ranged zombies – fired after the loop (`ranged.rs`), they need all of `self`.
        let mut shots = Vec::new();
        // Every hit makes a little noise (`sound.rs`) – a crowd tearing at a wall gets loud.
        let mut hits = Vec::new();
        for (i, (zombie, (velocity, hit, seen))) in self.zombies.iter_mut().zip(moves).enumerate() {
            // It sees one of your units: it remembers where – and goes there when it loses sight of it.
            if let Some(at) = seen {
                zombie.lure = Some(Lure::seen(at, tick));
                zombie.wander = None;
            }
            zombie.prev = zombie.pos;
            zombie.step = velocity;
            zombie.calm_for = if calm.get(i) == Some(&true) { zombie.calm_for.saturating_add(1) } else { 0 };
            if velocity != Vec2::ZERO {
                zombie.pos = slide(zombie.pos, velocity, |x, y| map.is_blocked(x, y));
                // Its body in the grid too – for pushing the crowd apart below.
                self.grid.moved(i, zombie.pos);
            }
            zombie.cooldown = zombie.cooldown.saturating_sub(1);
            zombie.alert = zombie.alert.saturating_sub(1);
            zombie.seek = zombie.seek.saturating_sub(1);
            if hit.is_some() {
                zombie.cooldown = zombie.breed.attack_ticks();
                hits.push(zombie.pos);
            }
            match hit {
                Some(hit) if zombie.breed.is_ranged() => shots.push((zombie.pos, zombie.breed, hit)),
                Some(Hit::Unit(u)) => {
                    let unit = &mut self.units[u];
                    let damage = zombie.breed.stats().damage.saturating_sub(unit.stats().armor).max(1);
                    unit.hp = unit.hp.saturating_sub(damage);
                }
                Some(Hit::Building(b)) => {
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
        for (from, breed, hit) in shots {
            self.zombie_shoots(from, breed, hit);
        }
        for at in hits {
            self.make_sound(at, data::get().sound.zombie_hit);
        }

        // 4. Nobody stands inside anybody else: whoever overlaps is pushed apart, like a liquid. Only the busy ones – a
        // calm crowd sorts itself out slowly by its soft push (`zombie_separation`). The grid still has everyone in the
        // tile where the tick began, their bodies where they stand now (`moved`) – nobody moves more than a step.
        for _ in 0..CROWD_PASSES {
            self.unpile_zombies(&calm);
        }
        // The grid now matches the new positions – the units look up their zombies in it afterwards.
        self.fill_zombie_grid();

    }

    /// What every zombie has in mind before it moves. One that arrived where it was lured to – or gave up getting there
    /// – forgets about it: a wave zombie (`siege`) heads on for the command center, every other one stays and strolls
    /// around there. Strollers stand around a while, then stroll to a random spot near their place. Here, not in
    /// `zombie_intent`, because it draws random numbers – and that changes the generator (writing).
    /// A stroller plans only on its thinking tick (`thinks_now`) – its timer counts `THINK_EVERY` ticks at once then.
    /// `noticing`: per zombie, has it noticed one of your units (`notices`).
    fn update_minds(&mut self, noticing: &[bool]) {
        for i in 0..self.zombies.len() {
            let zombie = &self.zombies[i];
            if let Some(lure) = zombie.lure {
                let there = zombie.pos.distance(lure.to) < LURE_ARRIVED || self.tick >= lure.until;
                if there && !noticing[i] {
                    let zombie = &mut self.zombies[i];
                    zombie.lure = None;
                    if !zombie.siege {
                        zombie.wander = Some(Wander::new());
                    }
                }
                continue;
            }
            // Storming one of your units: no strolling – it remembers where it saw it (`move_zombies`).
            let Some(mut wander) = zombie.wander.filter(|_| !noticing[i] && self.thinks_now(zombie)) else {
                continue;
            };
            wander.timer = wander.timer.saturating_sub(THINK_EVERY as u32);
            let arrived = wander.goal.is_some_and(|goal| goal.distance(zombie.pos) < 0.3);
            if wander.goal.is_some() && (arrived || wander.timer == 0) {
                // There (or stuck): stand around for 1–5 s.
                wander.goal = None;
                wander.timer = 20 + self.rng.below(80);
            } else if wander.goal.is_none() && wander.timer == 0 {
                // On to a random free spot around where it stands – how far: its breed's `stroll`, but never into the
                // ring around the command center its difficulty keeps the breed out of (`keep_out`). No home it keeps to: trip after
                // trip it may get anywhere. It gets the time it needs at its pace and half as much again for detours,
                // then it gives up.
                let radius = zombie.breed.stats().stroll;
                let pace = (zombie.speed * values().wander_pace).max(0.05);
                let (keep_out, inside_step) = (self.keep_out.of(zombie.breed), data::get().roaming.inside_step);
                for _ in 0..5 {
                    let offset = vec2(self.rng.unit_f32() * 2.0 - 1.0, self.rng.unit_f32() * 2.0 - 1.0) * radius;
                    let spot = zombie.pos + offset;
                    let free = !self.map.is_blocked(spot.x.floor() as i32, spot.y.floor() as i32);
                    if free && keeps_out(zombie.pos, spot, self.base, keep_out, inside_step) {
                        wander.goal = Some(spot);
                        wander.timer = (spot.distance(zombie.pos) / pace * 1.5 * TICKS_PER_SECOND) as u32 + 40;
                        break;
                    }
                }
            }
            self.zombies[i].wander = Some(wander);
        }
    }

    /// The ways to the places lured zombies are heading for (`LureField`, one per cell of the sound grid) – made when a
    /// zombie first needs one, at most `NEW_LURE_FIELDS` per tick (the others walk straight until then), kept while
    /// they're used. Who storms one of your units doesn't need one. In the order of the cells – the same on every PC.
    fn prepare_lure_fields(&mut self) {
        let mut cells: Vec<(i32, i32)> = self
            .zombies
            .iter()
            .filter(|z| z.lure.is_some() && !self.notices(z))
            .filter_map(|z| z.lure.map(|lure| cell_xy(lure.to)))
            .collect();
        cells.sort_unstable();
        cells.dedup();
        let mut made = 0;
        for cell in cells {
            match self.lures.binary_search_by_key(&cell, |f| f.cell) {
                Ok(k) => self.lures[k].used = self.tick,
                Err(k) if made < NEW_LURE_FIELDS => {
                    let field = LureField::new(&self.map, cell, CELL, LURE_REACH, Through::Cost(CHARGE_THROUGH), self.tick);
                    self.lures.insert(k, field);
                    made += 1;
                }
                Err(_) => {}
            }
        }
        let (tick, kept) = (self.tick, (LURE_FIELD_KEPT * TICKS_PER_SECOND) as u64);
        self.lures.retain(|f| tick - f.used < kept);
    }

    /// What a zombie is up to right now, in a few words – for the panel when it's clicked. The same order as in
    /// `zombie_intent`, only roughly (it doesn't look whether the way is really there).
    pub fn zombie_doing(&self, zombie: &Zombie) -> &'static str {
        if zombie.breed.is_ranged() && self.notices(zombie) && self.ranged_target(zombie).is_some() {
            "Shooting at your units"
        } else if zombie.is_seeking() {
            "Looking for a gap to your units"
        } else if self.notices(zombie) {
            "Storming your units"
        } else if self.building_aggro.near(zombie.pos).is_some() {
            "Tearing down a building"
        } else if let Some(lure) = zombie.lure {
            if lure.loud == u32::MAX { "Going where it last saw your units" } else { "Lured by a sound" }
        } else if zombie.wander.is_some() {
            "Wandering around"
        } else if zombie.siege {
            "Heading for the command center"
        } else {
            "Standing around"
        }
    }

    /// Is this zombie calm – a stroller with nothing else to do? Nobody lured it, no building is near, it isn't busy
    /// hitting, looking for a gap or with the final wave – and (asked apart, `move_zombies`) it notices none of your
    /// units. Then it only has to think every `THINK_EVERY` ticks. Asked every tick – the lookups are cheap.
    fn is_calm(&self, zombie: &Zombie) -> bool {
        zombie.wander.is_some()
            && zombie.lure.is_none()
            && zombie.cooldown == 0
            && zombie.seek == 0
            && !zombie.final_wave
            && self.building_aggro.near(zombie.pos).is_none()
    }

    /// Is it this calm zombie's tick to think? Every `THINK_EVERY` ticks, each on another one – by its number, so they
    /// don't all think at once (the same on every PC).
    fn thinks_now(&self, zombie: &Zombie) -> bool {
        (self.tick + zombie.id as u64).is_multiple_of(THINK_EVERY)
    }

    /// Has this zombie noticed one of your units – sees one, hears one close by, or has just been shot?
    pub(super) fn notices(&self, zombie: &Zombie) -> bool {
        zombie.alert > 0
            || self.aggro.near(zombie.pos).is_some()
            || self.zombie_sight[zombie.breed.index()].sees(zombie.pos)
    }

    /// What a zombie wants this tick: where to walk (length 1, slower: shorter, standing: 0), whom to hit – and where it
    /// sees one of your units right now (it remembers that place, `Lure::seen`). In this order (DESIGN.md 8.1):
    /// 1. A unit within reach: hit it – everyone does that. A ranged zombie: one in range and in sight – shoot it.
    /// 2. Looking for a gap (shot, or next to one that was) with a unit in reach on foot, around walls: walk the way
    ///    to it.
    /// 3. Noticed a unit (sees it, hears it, has been shot): storm at it – around water and mountains, but straight
    ///    through walls. If the next step is a building, tear it down.
    /// 4. A building nearby: tear it down (a ranged zombie: shoot at it from where it is in range).
    /// 5. Lured somewhere (a sound, the place it last saw a unit): go there – the same way through walls as 3.
    /// 6. A wanderer: stroll around its place.
    /// 7. A wave zombie: follow the flow field to the command center – right through buildings if the detour is too
    ///    long. Then the building soon becomes the nearest target (point 4). Anyone else stands where it is.
    /// `noticing`: has it noticed one of your units (`notices`, asked once per tick in `move_zombies`)?
    fn zombie_intent(&self, zombie: &Zombie, noticing: bool) -> (Vec2, Option<Hit>, Option<Vec2>) {
        let pos = zombie.pos;
        if let Some(u) = self.aggro.near(pos) {
            if self.units[u].pos.distance(pos) <= UNIT_RADIUS + zombie.radius + ARM {
                return (Vec2::ZERO, Some(Hit::Unit(u)), Some(self.units[u].pos));
            }
        }
        // A ranged zombie that has noticed your units: one of them in range and in sight – stand still and shoot. Over
        // walls too, like TAB's venom: only forests block its view.
        if zombie.breed.is_ranged() && noticing {
            if let Some(u) = self.ranged_target(zombie) {
                return (Vec2::ZERO, Some(Hit::Unit(u)), Some(self.units[u].pos));
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
                return (dir, None, Some(target));
            }
        }

        if noticing {
            if let Some((u, step)) = self.charge.track(&self.map, pos) {
                let target = self.units[u].pos;
                // The unit sits in a tower that the zombie touches: hit the tower.
                let tower = self.units[u].inside.and_then(|id| self.find_building(id));
                if let Some(b) = tower.filter(|&b| self.buildings[b].distance_to(pos) <= zombie.radius + ARM) {
                    return (Vec2::ZERO, Some(Hit::Building(b)), Some(target));
                }
                // Clear way: straight there. Otherwise one step along the way.
                if step == Vec2::ZERO || clear_line(&self.map, pos, target) {
                    return ((target - pos).normalize_or_zero(), None, Some(target));
                }
                // The step leads onto a building (a wall in the way, or the tower the unit sits in): hit it as soon
                // as you touch it.
                let (dir, hit) = self.step_or_tear_down(zombie, step);
                return (dir, hit, Some(target));
            }
        }

        if let Some(b) = self.building_aggro.near(pos) {
            // Already standing at a wall: hit that one – otherwise to the nearest spot of the building. A ranged one
            // doesn't have to touch it: in range is enough.
            if let Some(touching) = self.touching_building(zombie) {
                return (Vec2::ZERO, Some(Hit::Building(touching)), None);
            }
            if zombie.breed.is_ranged() && self.buildings[b].distance_to(pos) <= zombie.breed.stats().range {
                return (Vec2::ZERO, Some(Hit::Building(b)), None);
            }
            let b = &self.buildings[b];
            let closest = pos.clamp(b.corner(), b.far_corner());
            // Straight there only if nothing is in between (water, a mountain – the building's own tiles don't count:
            // just short of its edge). Otherwise it goes on with whatever else it's up to – a lure, the way to the
            // command center – and finds the building from there instead of pushing against the water.
            let short_of_it = closest + (pos - closest).normalize_or_zero() * 0.05;
            if clear_line(&self.map, pos, short_of_it) {
                return ((closest - pos).normalize_or_zero(), None, None);
            }
        }

        // Lured somewhere: along the way to its cell (`prepare_lure_fields`), the last bit – or without a way yet –
        // straight. A building in the way: tear it down.
        if let Some(lure) = zombie.lure {
            let field = self.lures.binary_search_by_key(&cell_xy(lure.to), |f| f.cell).ok().map(|k| &self.lures[k]);
            return match field.and_then(|f| f.step(&self.map, pos)) {
                Some(step) if step != Vec2::ZERO && !clear_line(&self.map, pos, lure.to) => {
                    let (dir, hit) = self.step_or_tear_down(zombie, step);
                    (dir, hit, None)
                }
                _ => ((lure.to - pos).normalize_or_zero(), None, None),
            };
        }

        // A wanderer strolls slowly to its spot – or just stands there.
        if let Some(wander) = zombie.wander {
            let dir = wander.goal.map_or(Vec2::ZERO, |goal| (goal - pos).normalize_or_zero());
            return (dir * values().wander_pace, None, None);
        }
        if !zombie.siege {
            return (Vec2::ZERO, None, None);
        }

        let dir = self.to_base.direction(pos);
        let dir = if dir == Vec2::ZERO { (self.base - pos).normalize_or_zero() } else { dir };
        (dir, None, None)
    }

    /// One step along a zombie's way – unless the step leads onto a building (a wall in the way, the tower a unit sits
    /// in) and the zombie touches it: then it stands and tears it down.
    fn step_or_tear_down(&self, zombie: &Zombie, step: Vec2) -> (Vec2, Option<Hit>) {
        let pos = zombie.pos;
        let next = (pos.x.floor() as i32 + step.x.round() as i32, pos.y.floor() as i32 + step.y.round() as i32);
        match self.building_on_tile(next.0, next.1).and_then(|_| self.touching_building(zombie)) {
            Some(touching) => (Vec2::ZERO, Some(Hit::Building(touching))),
            None => (step, None),
        }
    }

    /// Does this zombie touch a building it would hit – one of yours, not an infected one or a trap? Only looks into the tiles
    /// around it.
    fn touching_building(&self, zombie: &Zombie) -> Option<usize> {
        let pos = zombie.pos;
        let (cx, cy) = (pos.x.floor() as i32, pos.y.floor() as i32);
        for dy in -1..=1 {
            for dx in -1..=1 {
                if let Some(b) = self.building_on_tile(cx + dx, cy + dy) {
                    if !self.buildings[b].zombies_ignore() && self.buildings[b].distance_to(pos) <= zombie.radius + ARM {
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
    /// A calm zombie (`calm`, see `move_zombies`) isn't pushed – the others are still pushed away from it. The grid has
    /// to be filled before, with the bodies where they stand now (`move_zombies`). Zombies that came out during this
    /// tick (an infected house) are in neither yet – they aren't calm, and the others find them next tick.
    fn unpile_zombies(&mut self, calm: &[bool]) {
        // Only reading – with many busy ones on all cores at once (see the top of the file).
        let grid = &self.grid;
        let push = |(me, zombie): (usize, &Zombie)| {
            if calm.get(me).copied().unwrap_or(false) {
                return Vec2::ZERO;
            }
            let pos = zombie.pos;
            let (cx, cy) = (pos.x.floor() as i32, pos.y.floor() as i32);
            let mut fix = Vec2::ZERO;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    for other in grid.bodies(cx + dx, cy + dy) {
                        if other.index as usize == me {
                            continue;
                        }
                        // Bodies touch at the sum of both radii. Even two fat ones touch closer than one tile, so
                        // looking into the 3 × 3 tiles around is still enough.
                        let min_dist = zombie.radius + other.radius;
                        let away = pos - other.pos;
                        // Most don't touch: the squared distance tells that without the slow square root.
                        let d2 = away.length_squared();
                        if d2 >= min_dist * min_dist {
                            continue;
                        }
                        let d = d2.sqrt();
                        // Exactly on top of each other: no direction to push in. Then the numbers decide –
                        // the two get opposite directions, the same ones every time (deterministic).
                        let dir = if d > 0.0001 { away / d } else { overlap_direction(zombie.id, other.id) };
                        fix += dir * (min_dist - d) * 0.5;
                    }
                }
            }
            fix.clamp_length_max(MAX_UNPILE)
        };
        let busy = self.zombies.len() - calm.iter().filter(|&&c| c).count();
        let fixes: Vec<Vec2> = if busy >= SIDE_BY_SIDE_FROM {
            self.zombies.par_iter().enumerate().with_min_len(PER_CORE).map(push).collect()
        } else {
            self.zombies.iter().enumerate().map(push).collect()
        };
        // Moved: its body in the grid too, so the next pass sees it where it is now.
        let map = &self.map;
        for (i, (zombie, fix)) in self.zombies.iter_mut().zip(fixes).enumerate().filter(|(_, (_, fix))| *fix != Vec2::ZERO) {
            zombie.pos = slide(zombie.pos, fix, |x, y| map.is_blocked(x, y));
            self.grid.moved(i, zombie.pos);
        }
    }

    /// Pushes overlapping zombies apart – and away from units.
    fn zombie_separation(&self, me: usize, zombie: &Zombie) -> Vec2 {
        let (pos, radius) = (zombie.pos, zombie.radius);
        let mut push = Vec2::ZERO;
        let (cx, cy) = (pos.x.floor() as i32, pos.y.floor() as i32);
        for dy in -1..=1 {
            for dx in -1..=1 {
                for other in self.grid.bodies(cx + dx, cy + dy).iter().filter(|b| b.index as usize != me) {
                    push += push_apart(pos, other.pos, radius + other.radius);
                }
                for u in self.unit_grid.cell(cx + dx, cy + dy) {
                    let unit = &self.units[u];
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
