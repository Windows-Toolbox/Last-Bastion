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
//! every tick again. And with a busy one right next to it, it moves every tick too – it may be walked into and pushed
//! (`next_to_busy`). That's what makes tens of thousands of them possible.
//!
//! **Thinking side by side:** what every zombie wants (`zombie_intent`), its step in the crowd (`crowd_step`) and how
//! far it is pushed apart from the others (`unpile_zombies`) are worked out on all cores of the processor at once
//! (`rayon`, `per_zombie`) – each zombie only reads, nobody writes. Every one comes out exactly as if worked out one
//! after the other, collected in the order of the list – the same on every PC, however many cores it has.

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
/// A zombie's hard core – this share of the touching distance (both radii together): nobody walks closer to another
/// than that, and whoever is closer is pushed out to it (`unpile_zombies`). So a crowd stands a little inside itself,
/// like in TAB – not merged into one, and not body to body in rows either.
const CORE: f32 = 0.75;
/// This share of the step another's core blocks, a zombie takes sideways instead – around it. The ones behind flow
/// around a pile instead of queuing up behind it. With half (0.5) more of them still queued up behind it; with all of it
/// (1.0) more of them stood inside each other around your units.
const SIDESTEP: f32 = 0.75;
/// Overlapping zombies push each other away softly – making room to the sides – at this share of their pace. Much
/// stronger (1.5) and a crowd jitters: the push and the walking back in swing against each other from tick to tick.
const SOFT_PUSH: f32 = 0.3;
/// Who walks into another's core pushes it along: the other one moves by this share of the step that went into it –
/// between two of the same size; a heavier one pushes more (at most all of it), a lighter one less (`push_share`). In
/// the same tick, and it doesn't walk into anybody's core either – where nobody can make room, nobody moves, so the
/// crowd stands still. Half as strong (0.5) a walker was pushed only half as far as before the cores; 1.0 shook more.
/// Who is pushed presses on with it the next tick (`shove`) – so the whole mass pushes, not only the ones right behind.
const PUSH: f32 = 0.75;
/// This share of its last step a zombie keeps (`crowd_step`) – it doesn't turn on the spot or stop dead. Without it a
/// crowd streaming into a gap zigzagged, pushed this way one tick and that way the next; and someone pushed moved in
/// jerks.
const INERTIA: f32 = 0.5;
/// A zombie's step is checked against at most this many cores close by (`crowd_step`) – even in the thickest crowd
/// around a fat one there are fewer; more would only be left to the pushing apart (`unpile_zombies`).
const CLOSE: usize = 32;
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
/// A leap over a wall (`leap_over`) takes this many ticks in the air – 0.6 s …
const LEAP_TICKS: u32 = 12;
/// … and goes this high at its top (tile widths) – only the look (`leap_height`).
const LEAP_HEIGHT: f32 = 0.8;
/// Looking for where to land behind a wall: a point every this many tiles along the way.
const LEAP_LOOK: f32 = 0.1;
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

#[derive(Clone)]
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
    /// The swarm of a wave it came out with – everyone of one wave out of one side has the same number (0: none). The
    /// minimap marks the middle of every swarm (`swarms`).
    pub swarm: u32,
    /// Its step per tick as it last decided – a calm zombie walks on with it until it thinks again (`is_calm`), a busy
    /// one keeps a part of it (`INERTIA`) and steps aside to the side it points to (`crowd_step`).
    pub(super) step: Vec2,
    /// Ticks it has been calm in a row – long enough, and far from your eyes, it falls asleep (`sleep.rs`).
    pub(super) calm_for: u32,
    /// How far the others pushed it this tick (`crowd_step`). The next tick it presses on with it against the ones in
    /// front of it – so a push goes on through the crowd (`move_zombies`).
    pub(super) shove: Vec2,
    /// In the air over a wall (a harpy, `leap_over`) – nobody pushes it, it pushes nobody, it hits nothing.
    pub(super) leap: Option<Leap>,
}

/// A leap over a wall: from where it took off to where it lands.
#[derive(Clone, Copy)]
pub(super) struct Leap {
    from: Vec2,
    to: Vec2,
    /// Ticks in the air so far – it lands after `LEAP_TICKS`.
    flown: u32,
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
    /// Like TAB's harpy: weak, but fast and very alert – it hears a fight from much further than the others and comes
    /// first. Every hit it lands is loud (`hit_noise`), louder the longer it goes on: kill it before the rest come.
    /// `alias`: map files from before 2026-10-25 still have the screamer in them – it is a harpy now.
    #[serde(alias = "Screamer")]
    Harpy,
    /// Like TAB's giant, a bit smaller and weaker: huge, slow, smashes walls in a few hits. Only a few per map, far out
    /// (`giants` of the map, `place_giants`) – and in the final wave, which calls everyone.
    Giant,
}

impl Breed {
    pub const ALL: [Breed; 8] = [
        Breed::Walker,
        Breed::Runner,
        Breed::Fat,
        Breed::Armored,
        Breed::Spitter,
        Breed::Infested,
        Breed::Harpy,
        Breed::Giant,
    ];

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

    /// How much noise one of its hits makes: its own `hit_noise`, otherwise `zombie_hit` of `data\sound.json`.
    pub fn hit_noise(self) -> u32 {
        self.stats().hit_noise.unwrap_or(data::get().sound.zombie_hit)
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

    /// How high it is in the air right now (tile widths, 0: on the ground) – a harpy leaping over a wall goes up in an
    /// arc. `alpha`: how far between the last tick and the next, like the position (`draw.rs`). Only the look.
    pub fn leap_height(&self, alpha: f32) -> f32 {
        self.leap.map_or(0.0, |leap| {
            let t = ((leap.flown as f32 - 1.0 + alpha) / LEAP_TICKS as f32).clamp(0.0, 1.0);
            LEAP_HEIGHT * (t * std::f32::consts::PI).sin()
        })
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
            swarm: 0,
            step: Vec2::ZERO,
            calm_for: 0,
            shove: Vec2::ZERO,
            leap: None,
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

    /// The number of a new swarm of a wave (`Zombie::swarm`).
    pub(super) fn new_swarm(&mut self) -> u32 {
        self.next_swarm += 1;
        self.next_swarm
    }

    /// The middle of every swarm of a wave still alive, and how many it still has – one mark each on the minimap,
    /// wherever the swarm is. In the order the swarms came.
    pub fn swarms(&self) -> Vec<(Vec2, usize)> {
        // (number, sum of the positions, how many) – only a handful of swarms at a time, so a plain list will do.
        let mut swarms: Vec<(u32, Vec2, usize)> = Vec::new();
        for zombie in self.zombies.iter().filter(|z| z.swarm != 0 && z.is_alive()) {
            match swarms.iter_mut().find(|s| s.0 == zombie.swarm) {
                Some(swarm) => {
                    swarm.1 += zombie.pos;
                    swarm.2 += 1;
                }
                None => swarms.push((zombie.swarm, zombie.pos, 1)),
            }
        }
        swarms.sort_by_key(|s| s.0);
        swarms.into_iter().map(|(_, sum, n)| (sum / n as f32, n)).collect()
    }

    /// Where the giants are – awake or asleep. The minimap always shows them, fog or not.
    pub fn giants(&self) -> impl Iterator<Item = Vec2> + '_ {
        let awake = self.zombies.iter().filter(|z| z.breed == Breed::Giant && z.is_alive()).map(|z| z.pos);
        awake.chain(self.sleep.iter().filter(|s| s.breed == Breed::Giant).map(|s| s.pos))
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
        // Who is calm (`is_calm`) – and who of them rests this tick: it isn't its tick to think, and no busy one is next
        // to it. Whoever doesn't rest may be pushed (`crowd_step`) – a calm one only next to a busy one, so it moves
        // every tick then; otherwise it would only be pushed every 8th tick, in jerks.
        let calm: Vec<bool> = self.zombies.iter().zip(&noticing).map(|(z, &n)| !n && self.is_calm(z)).collect();
        let near_busy = self.next_to_busy(&calm);
        let every_tick: Vec<bool> = calm.iter().zip(&near_busy).map(|(&c, &near)| !c || near).collect();
        let resting: Vec<bool> = self.zombies.iter().zip(&every_tick).map(|(z, &e)| !e && !self.thinks_now(z)).collect();

        // 2. For every zombie compute where it wants to walk this tick (tiles) and whether it hits – only reading, so with
        // many busy ones on all cores at once (see the top of the file). A resting one walks on as it decided.
        let world = &*self;
        let busy = resting.iter().filter(|&&r| !r).count();
        // A harpy in the air flies on; one at a wall that can leap over it takes off (the last: where it lands).
        let intents: Vec<(Vec2, Option<Hit>, Option<Vec2>, Option<Vec2>)> = world.per_zombie(busy, |(i, zombie)| {
            if resting[i] {
                return (zombie.step, None, None, None);
            }
            if zombie.leap.is_some() {
                return (Vec2::ZERO, None, None, None);
            }
            let (dir, hit, seen) = world.zombie_intent(zombie, noticing[i]);
            if let Some(to) = world.leap_over(zombie, hit, seen) {
                return (Vec2::ZERO, None, seen, Some(to));
            }
            (dir * zombie.speed * TICK, hit.filter(|_| zombie.cooldown == 0), seen, None)
        });
        // Then its step in the crowd (`crowd_step`) – only now, it needs to know what the others want: who walks into
        // it pushes it. How hard each one presses on the ones in front of it: what it wants and how far it was pushed
        // last tick (`shove`) – so the mass behind pushes through to the front. But never harder than it walks itself:
        // added up through a deep crowd, one was shoved almost twice as fast as a zombie walks.
        let wants: Vec<Vec2> = intents.iter().map(|&(want, _, _, _)| want).collect();
        let presses: Vec<Vec2> =
            wants.iter().zip(&world.zombies).map(|(&want, z)| (want + z.shove).clamp_length_max(z.speed * TICK)).collect();
        let steps = world.per_zombie(busy, |(i, zombie)| {
            if resting[i] {
                (zombie.step, Vec2::ZERO)
            } else if zombie.leap.is_some() || intents[i].3.is_some() {
                (Vec2::ZERO, Vec2::ZERO)
            } else {
                world.crowd_step(i, zombie, &wants, &presses, every_tick[i])
            }
        });
        let moves = steps.into_iter().zip(intents).map(|((step, shove), (_, hit, seen, leap))| (step, shove, hit, seen, leap));

        // 3. Only now move everyone and deal damage – writing. The world border doesn't stop zombies.
        let map = &self.map;
        let tick = self.tick;
        // Buildings whose shield breaks this tick – they let their zombies out after the loop (`infect`).
        let mut infected = Vec::new();
        // The shots of the ranged zombies – fired after the loop (`ranged.rs`), they need all of `self`.
        let mut shots = Vec::new();
        // Every hit makes noise (`sound.rs`) – a crowd tearing at a wall gets loud, a harpy alone does.
        let mut hits = Vec::new();
        for (i, (zombie, (velocity, shove, hit, seen, leap))) in self.zombies.iter_mut().zip(moves).enumerate() {
            zombie.shove = shove;
            // It sees one of your units: it remembers where – and goes there when it loses sight of it.
            if let Some(at) = seen {
                zombie.lure = Some(Lure::seen(at, tick));
                zombie.wander = None;
            }
            // Over a wall: takes off, flies on in a straight line – over the wall and whoever stands there – and lands
            // after `LEAP_TICKS`. Nothing else happens to it in the air.
            if let Some(to) = leap {
                zombie.leap = Some(Leap { from: zombie.pos, to, flown: 0 });
            }
            if let Some(leap) = zombie.leap.as_mut() {
                leap.flown += 1;
                let at = leap.from.lerp(leap.to, leap.flown as f32 / LEAP_TICKS as f32);
                if leap.flown >= LEAP_TICKS {
                    zombie.leap = None;
                }
                zombie.prev = zombie.pos;
                zombie.pos = at;
                zombie.step = Vec2::ZERO;
                zombie.calm_for = 0;
                zombie.cooldown = zombie.cooldown.saturating_sub(1);
                zombie.alert = zombie.alert.saturating_sub(1);
                zombie.seek = zombie.seek.saturating_sub(1);
                self.grid.moved(i, at);
                continue;
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
                hits.push((zombie.pos, zombie.breed.hit_noise()));
            }
            match hit {
                Some(hit) if zombie.breed.is_ranged() => shots.push((zombie.pos, zombie.breed, hit)),
                Some(Hit::Unit(u)) => self.units[u].take_hit(zombie.breed.stats().damage, tick),
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
        // The zombies' noise: it brings them and wakes the sleepers around like any sound, but it doesn't add to your
        // colony's unrest.
        for (at, loud) in hits {
            self.make_sound(at, loud);
        }

        // 4. Nobody stands deeper inside anybody else than its core: whoever does is pushed apart. Only the busy ones – a
        // calm crowd sorts itself out slowly by its soft push (`crowd_step`). The grid still has everyone in the
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
            && zombie.leap.is_none()
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
                    arrive(zombie, target, zombie.radius + UNIT_RADIUS)
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
                    return (arrive(zombie, target, zombie.radius + UNIT_RADIUS), None, Some(target));
                }
                // The step leads onto a building (a wall in the way, or the tower the unit sits in): hit it as soon
                // as you touch it.
                let (dir, hit) = self.step_or_tear_down(zombie, step);
                return (dir, hit, Some(target));
            }
        }

        // (A harpy leaves alone the walls that aren't in its way – the one it just leaped over, `leaves_alone`.)
        if let Some(b) = self.building_aggro.near(pos).filter(|&b| !self.leaves_alone(zombie, b)) {
            // Already standing at a wall: hit that one – otherwise to the nearest spot of the building. A ranged one
            // doesn't have to touch it: in range is enough.
            if let Some(touching) = self.touching_building(zombie).filter(|&t| !self.leaves_alone(zombie, t)) {
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
                return (arrive(zombie, closest, zombie.radius + ARM), None, None);
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

    /// A harpy (`jump` in `data\zombies.json`) about to hit a wall or a gate leaps over it instead – if what it is after
    /// lies behind it: the unit it sees, otherwise where it was lured to, otherwise the command center. Straight towards
    /// that it looks for free ground behind the wall, at most `jump` tiles of wall away: where it lands (the middle of
    /// that tile). Not over anything else – a house, a tower, water, a mountain, a forest, a wall too thick: it hits.
    fn leap_over(&self, zombie: &Zombie, hit: Option<Hit>, seen: Option<Vec2>) -> Option<Vec2> {
        let jump = zombie.breed.stats().jump;
        let Some(Hit::Building(b)) = hit else {
            return None;
        };
        if jump <= 0.0 || !self.buildings[b].kind.is_wall() {
            return None;
        }
        let pos = zombie.pos;
        let goal = seen.or(zombie.lure.map(|lure| lure.to)).unwrap_or(self.base);
        let dir = (goal - pos).normalize_or_zero();
        if dir == Vec2::ZERO {
            return None;
        }
        // From its body's edge over at most `jump` tiles of wall.
        let reach = zombie.radius + ARM + jump + LEAP_LOOK;
        let mut over_wall = false;
        let mut s = 0.0;
        while s < reach {
            s += LEAP_LOOK;
            let p = pos + dir * s;
            let (x, y) = (p.x.floor() as i32, p.y.floor() as i32);
            match self.building_on_tile(x, y).filter(|&w| !self.buildings[w].zombies_ignore()) {
                Some(w) if self.buildings[w].kind.is_wall() => over_wall = true,
                Some(_) => return None,
                None if self.map.is_blocked(x, y) => return None,
                None if over_wall => {
                    let land = vec2(x as f32 + 0.5, y as f32 + 0.5);
                    return (land.distance(goal) < pos.distance(goal)).then_some(land);
                }
                None => {}
            }
        }
        None
    }

    /// Does a harpy leave this building alone? A wall that isn't between it and where it's heading (where it was lured
    /// to, otherwise the command center) – it doesn't turn back to tear down the wall it just leaped over. Walls in
    /// its way it leaps over (`leap_over`) – or hits, if too thick.
    fn leaves_alone(&self, zombie: &Zombie, b: usize) -> bool {
        let building = &self.buildings[b];
        if zombie.breed.stats().jump <= 0.0 || !building.kind.is_wall() {
            return false;
        }
        let goal = zombie.lure.map_or(self.base, |lure| lure.to);
        let closest = zombie.pos.clamp(building.corner(), building.far_corner());
        (closest - zombie.pos).dot(goal - zombie.pos) <= 0.0
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

    /// The crowd can't be squeezed: every pair of zombies closer than their cores (`CORE`) is pushed apart by half of
    /// that each. Walking, nobody goes into another's core (`crowd_step`) – but two stepping into the same gap at once,
    /// or one pushed out of a wall or knocked back, still end up deeper inside each other. This correction moves the
    /// positions directly.
    /// First all corrections are computed (only reading), then applied – so the order in the list doesn't matter.
    /// A calm zombie (`calm`, see `move_zombies`) isn't pushed – the others are still pushed away from it. The grid has
    /// to be filled before, with the bodies where they stand now (`move_zombies`). Zombies that came out during this
    /// tick (an infected house) are in neither yet – they aren't calm, and the others find them next tick.
    fn unpile_zombies(&mut self, calm: &[bool]) {
        // Only reading – with many busy ones on all cores at once (see the top of the file).
        let (grid, zombies) = (&self.grid, &self.zombies);
        let push = |(me, zombie): (usize, &Zombie)| {
            if calm.get(me).copied().unwrap_or(false) || zombie.leap.is_some() {
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
                        // Bodies touch at the sum of both radii, their cores a little closer. Even two fat ones touch
                        // closer than one tile, so looking into the 3 × 3 tiles around is still enough.
                        let min_dist = (zombie.radius + other.radius) * CORE;
                        let away = pos - other.pos;
                        // Most don't touch: the squared distance tells that without the slow square root.
                        let d2 = away.length_squared();
                        if d2 >= min_dist * min_dist {
                            continue;
                        }
                        if zombies[other.index as usize].leap.is_some() {
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
        let fixes = self.per_zombie(busy, push);
        // Moved: its body in the grid too, so the next pass sees it where it is now.
        let map = &self.map;
        for (i, (zombie, fix)) in self.zombies.iter_mut().zip(fixes).enumerate().filter(|(_, (_, fix))| *fix != Vec2::ZERO) {
            zombie.pos = slide(zombie.pos, fix, |x, y| map.is_blocked(x, y));
            self.grid.moved(i, zombie.pos);
        }
    }

    /// Per zombie: is there a busy one (not `calm`) on its tile or a tile around it? Only then can somebody walk into a
    /// calm one and push it – touching, even two giants are less than a tile apart. Marked tile by tile around the busy
    /// ones, so a calm crowd far from them costs nothing more.
    fn next_to_busy(&self, calm: &[bool]) -> Vec<bool> {
        // Nobody calm (the final wave called them all) or nobody busy: nothing to look for.
        if !calm.contains(&true) || !calm.contains(&false) {
            return vec![false; calm.len()];
        }
        let n = self.map.size as i32;
        let tile = |x: i32, y: i32| (x >= 0 && y >= 0 && x < n && y < n).then(|| (y * n + x) as usize);
        let mut near = vec![false; (n * n) as usize];
        for (zombie, _) in self.zombies.iter().zip(calm).filter(|&(_, &c)| !c) {
            let (x, y) = (zombie.pos.x.floor() as i32, zombie.pos.y.floor() as i32);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if let Some(k) = tile(x + dx, y + dy) {
                        near[k] = true;
                    }
                }
            }
        }
        self.zombies.iter().map(|z| tile(z.pos.x.floor() as i32, z.pos.y.floor() as i32).is_some_and(|k| near[k])).collect()
    }

    /// Works `f` out for every zombie, in the order of the list – with at least `SIDE_BY_SIDE_FROM` busy ones on all
    /// cores at once (see the top of the file). One core or many: the same result.
    fn per_zombie<T: Send>(&self, busy: usize, f: impl Fn((usize, &Zombie)) -> T + Sync + Send) -> Vec<T> {
        if busy >= SIDE_BY_SIDE_FROM {
            self.zombies.par_iter().enumerate().with_min_len(PER_CORE).map(f).collect()
        } else {
            self.zombies.iter().enumerate().map(f).collect()
        }
    }

    /// Its step this tick (tiles): what it wants (its entry in `wants`, everyone's step as they want it) and how the
    /// others push it (`PUSH`, by how hard they press: `presses` – if it moves every tick: `pushable`), half of it what
    /// it walked last tick (`INERTIA`) – but toward a zombie or one of your units close by only up to its core (`CORE`),
    /// what's left slides along it, and three quarters of what's blocked it takes sideways (`SIDESTEP`). On top of that a
    /// soft push away from the zombies it overlaps (`SOFT_PUSH`) and from units. Also how far it was pushed (`shove`).
    /// Walking into each other as far as they liked, three of four stood more than half inside each other in front of a
    /// wall; only up to touching and without the sidestep, a crowd stood in rows; with a soft push five times as strong,
    /// it jittered. Pushing by walking into each other and being pushed apart afterwards (`unpile_zombies`), like before
    /// the cores, 600 zombies at a wall stepped back and forth 23,000 times in 3 s – now about 70 times.
    fn crowd_step(&self, me: usize, zombie: &Zombie, wants: &[Vec2], presses: &[Vec2], pushable: bool) -> (Vec2, Vec2) {
        let (pos, radius) = (zombie.pos, zombie.radius);
        let want = wants[me];
        let length = want.length();
        let (cx, cy) = (pos.x.floor() as i32, pos.y.floor() as i32);
        // 1. The soft push, the sidestep around every core in the way – to the side it already moves to (its last step,
        // so it doesn't zigzag), straight behind one by its number –, the pushes it gets, and which cores its step could
        // reach: with the sidestep it gets at most twice as long. Of a few hundred zombies around in a thick crowd, only
        // these few are looked at again below.
        let (mut push, mut aside, mut shove) = (Vec2::ZERO, Vec2::ZERO, Vec2::ZERO);
        let mut strongest = 0.0f32;
        let mut close = [(Vec2::ZERO, 0.0); CLOSE];
        let mut count = 0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                for other in self.grid.bodies(cx + dx, cy + dy).iter().filter(|b| b.index as usize != me) {
                    let touching = radius + other.radius;
                    let closest = touching * CORE;
                    let toward = other.pos - pos;
                    let d2 = toward.length_squared();
                    let reach = touching.max(closest + 2.0 * length);
                    if d2 >= reach * reach || d2 == 0.0 {
                        continue;
                    }
                    // One in the air over a wall isn't in the way.
                    if self.zombies[other.index as usize].leap.is_some() {
                        continue;
                    }
                    let d = d2.sqrt();
                    let n = toward / d;
                    if d < touching {
                        push -= n * (1.0 - d / touching);
                    }
                    let gap = (d - closest).max(0.0);
                    let blocked = want.dot(n) - gap;
                    if blocked > 0.0 {
                        let side = vec2(-n.y, n.x);
                        let lean = zombie.step.dot(side);
                        let leans = lean.abs() > 0.05 * zombie.speed * TICK;
                        let sign = if leans { lean.signum() } else if zombie.id.is_multiple_of(2) { 1.0 } else { -1.0 };
                        aside += side * sign * blocked * SIDESTEP;
                    }
                    // Pushed by as much as the other presses further into its core than its own step goes into the
                    // other's: two walking into each other head-on push nobody, one walking away from a faster one is
                    // pushed.
                    let pressed = -presses[other.index as usize].dot(n) - gap - blocked.max(0.0);
                    if pushable && pressed > 0.0 {
                        let share = push_share(other.radius, radius);
                        shove -= n * pressed * share;
                        strongest = strongest.max(pressed * share);
                    }
                    if count < CLOSE {
                        close[count] = (n, gap);
                        count += 1;
                    }
                }
            }
        }
        // Your units: a soft push away from them – and not into their cores either. Pushed into them by a crowd, more
        // zombies stood deep inside each other around them, and the units were shoved around further.
        let mut from_units = Vec2::ZERO;
        for dy in -1..=1 {
            for dx in -1..=1 {
                for u in self.unit_grid.cell(cx + dx, cy + dy) {
                    let unit = &self.units[u];
                    let toward = unit.pos - pos;
                    let d = toward.length();
                    if unit.is_inside() || d == 0.0 {
                        continue;
                    }
                    from_units += push_apart(pos, unit.pos, radius + UNIT_RADIUS);
                    if count < CLOSE {
                        close[count] = (toward / d, (d - (radius + UNIT_RADIUS) * CORE).max(0.0));
                        count += 1;
                    }
                }
            }
        }
        // 2. What it wants, the sidestep and the pushes – several push it no further than the strongest of them – and a
        // part of its last step (`INERTIA`). Into nobody's core: what's left slides along it.
        let shove = shove.clamp_length_max(strongest);
        let wish = want + aside.clamp_length_max(length) + shove;
        let mut step = wish.lerp(zombie.step, INERTIA);
        for &(n, gap) in &close[..count] {
            let into = step.dot(n) - gap;
            if into > 0.0 {
                step -= n * into;
            }
        }
        (step + (push * SOFT_PUSH + from_units) * zombie.speed * TICK, shove)
    }
}

/// The zombies' values from `data\zombies.json`.
pub(super) fn values() -> &'static data::Zombies {
    &data::get().zombies
}

/// How much of its step into another's core a zombie of size `pusher` (radius, tiles) passes on to one of size `pushed`
/// (`PUSH`): by their weight – the size squared, like the area of their bodies. A fat one or a giant shoves a walker the
/// whole way, a walker hardly moves a giant.
fn push_share(pusher: f32, pushed: f32) -> f32 {
    let (a, b) = (pusher * pusher, pushed * pushed);
    (PUSH * 2.0 * a / (a + b)).min(1.0)
}

/// Where a zombie walks to get to `to` (length 1 = its full step): straight there – but its last step only as long as it
/// needs to come within `stop` (tiles). One walking to a wall to hit it stops right where it reaches it, instead of a
/// step too far: then the crowd pushed it back out of reach, it walked in again – in front of a wall dozens stepped back
/// and forth every few ticks. To one of your units it walks until its body touches – within its reach with an arm's
/// length to spare, so it still hits one that steps back a little.
fn arrive(zombie: &Zombie, to: Vec2, stop: f32) -> Vec2 {
    let need = (zombie.pos.distance(to) - stop) / (zombie.speed * TICK).max(f32::EPSILON);
    (to - zombie.pos).normalize_or_zero() * need.clamp(0.0, 1.0)
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
