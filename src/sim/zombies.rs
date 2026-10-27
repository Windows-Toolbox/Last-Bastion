//! The horde: what a zombie is after, where it walks, whom it hits (DESIGN.md 8.1). How the crowd pushes and flows
//! is in `crowd.rs`.
//!
//! **What a zombie has in mind is TAB's decision tree** (`ZXBehaviour`, the trees of the zombie and of the giant):
//! the first point that fits wins.
//! 1. It's after something (`aim`) – one of your units, a building with people in it, a wall in its way: there, and
//!    hit it.
//! 2. It lost sight of it: to where that was, and stay there (`Lure`, `Why::Seen`). Not the giant.
//! 3. With a wave (`siege`): to the command center – what stands in the way it tears down.
//! 4. Every ~4 s: enough wave zombies of its rank or higher on its tile – it joins them (`joins_wave`). Not the giant.
//! 5. A sound it heard (`sound.rs`): there – and while it's loud there, it stays around the place.
//! 6. Idle (`Idle`): it stands, growls or roams, rolled with TAB's weights 1300 / 150 / 150.
//!
//! And the giant stops every 12 s and roars, whatever it's doing.
//!
//! **What it's after** it decides when it looks around (`watch`, TAB's `WatchForEnemies`): every `watch_seconds`,
//! less often while it's after something, out of your sight and where you've never been. Then it takes the nearest of
//! your units and buildings with people in it that it sees. Walls, empty buildings, power poles it never goes for –
//! only when one stands in its way (`wall_in_way`), then it tears that one down. Shot, it and the ones right around it
//! that aren't after anything go for the shooter (`zombie_hit_by`). The delays are TAB's on purpose: a crowd doesn't
//! turn as one.
//!
//! The numbers (sight, how often they look, their ranks, how they idle …) are in `data\zombies.json`. Here only what
//! isn't a game value but how the crowd is computed.
//!
//! **Calm zombies are cheap** (`is_calm`): most of the horde only idles around its place, far from everything. Such a
//! zombie thinks anew only every `THINK_EVERY` ticks – each on another tick, by its number – and walks on with what it
//! decided in between. It still looks around on its own beat (`watches_now`): sees it something, it isn't calm anymore
//! and thinks every tick again. And with a busy one right next to it, it moves every tick too – it may be walked into
//! and pushed (`next_to_busy`). That's what makes tens of thousands of them possible.
//!
//! **Thinking side by side:** what every zombie sees (`watch`), what it wants (`zombie_intent`) and how the crowd pushes
//! it (`crowd.rs`) are worked out on all cores of the processor at once (`rayon`, `per_zombie`) – each zombie only
//! reads, nobody writes. Every one comes out exactly as if
//! worked out one after the other, collected in the order of the list – the same on every PC, however many cores it
//! has.

use macroquad::math::{vec2, Vec2};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::sound::{cell_xy, CELL};
use super::terrain::nearest_free_tile;
use super::crowd::rock_ticks;
use super::{clear_line, World, TICK, TICKS_PER_SECOND, UNIT_RADIUS};
use crate::data::{self, BreedStats};
use crate::kinds::BuildingKind;
use crate::lure::LureField;
use crate::vision::Vision;

/// A calm zombie (`is_calm`) thinks anew only every this many ticks – 0.4 s; in between it walks on as it decided.
const THINK_EVERY: u64 = 8;
/// Walking on, a zombie keeps this share of its last step – it doesn't turn on the spot (TAB decides where to walk only
/// every other tick). Without it, a few squeezed against the corner of a gap stepped to and fro up to 16 times in 3 s:
/// the way from one tile goes to the next tile's middle, from that one straight into the gap.
const TURN: f32 = 0.3;
/// Every this many ticks the list of zombies is sorted by where they stand (`sort_by_place`) – 2 s.
pub(super) const SORT_EVERY: u64 = 40;
/// Thinking side by side: each core takes at least this many zombies at a time – fewer, and handing them out would
/// cost more than it saves …
pub(super) const PER_CORE: usize = 1024;
/// … and only with at least this many busy ones (not resting) at all – waking the other cores costs more than a few
/// thousand zombies' thinking. One core or many: the result is the same.
pub(super) const SIDE_BY_SIDE_FROM: usize = 4096;
/// A building tile on the way of a storming zombie costs this much instead of 1 step: their way goes around water
/// and mountains, but almost straight through walls – they tear them down instead of looking for the gap.
pub(super) const CHARGE_THROUGH: u16 = 2;
/// A zombie reaches this far past its own edge: it hits a unit or a building that is this close to its body.
/// (Its body: `radius`, per breed – a fat one reaches further.)
const ARM: f32 = 0.075;
/// Going for a building, a zombie walks this far (tiles) inside its reach (`go_for_building`).
const TOUCH_MARGIN: f32 = 0.01;
/// Your units are entered into the threat map this far around them (tiles) – further than any zombie reaches, so every
/// zombie finds the unit within its reach on its own tile (`zombie_intent`).
const REACH: f32 = 1.5;
/// This many steps of detour is a building tile worth to zombies. If the way around is longer,
/// they go right through and tear the building down (DESIGN.md 8.1).
pub(super) const BUILDING_DETOUR: u32 = 10;
/// A leap over a wall (`leap_over`) takes this many ticks in the air – 0.6 s …
const LEAP_TICKS: u32 = 12;
/// … and goes this high at its top (tile widths) – only the look (`leap_height`).
const LEAP_HEIGHT: f32 = 0.8;
/// Looking for where to land behind a wall: a point every this many tiles along the way.
const LEAP_LOOK: f32 = 0.1;
/// A zombie this close to where it lost sight of you, or where it was sent, has arrived (tiles).
const LURE_ARRIVED: f32 = 1.5;
/// One lured by a sound this close to the place has got there (TAB: 2 tiles) – it idles around it while it's loud.
const SOUND_REACHED: f32 = 2.0;
/// It gives up after this many seconds if it isn't there yet (stuck somewhere) – and stays where it is.
const LURE_PATIENCE: f32 = 60.0;
/// The way to a place zombies head for is worked out this far around it (tiles, `LureField`) – they heard or saw it
/// from closer than that. Further away they walk straight at it.
const LURE_REACH: i32 = 40;
/// At most this many new ways to such places per tick – the others walk straight until it's their turn.
const NEW_LURE_FIELDS: usize = 8;
/// A way nobody needed for this long (seconds) is thrown away.
const LURE_FIELD_KEPT: f32 = 10.0;
/// The wall rule (`wall_in_way`) only counts for a target further away than this (tiles, TAB) …
const WALL_FROM: f32 = 0.5;
/// … and looks for walls in a square this many tiles around the zombie, plus its own size (TAB: its width + 3).
const WALL_LOOK: i32 = 3;
/// Roaming: this many tries for a free spot (TAB) – none free, it stays and rolls again.
const ROAM_TRIES: usize = 20;
/// Off the playing field (or on blocked ground) an idle zombie goes back onto the nearest free tile within this many
/// tiles first.
const ROAM_BACK: i32 = 16;

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
    pub breed: Breed,
    /// What it's after (TAB's watcher target) – decided when it looks around (`watch`), or when it's shot
    /// (`zombie_hit_by`).
    pub(super) aim: Option<Aim>,
    /// Shot since it last looked around (TAB's `WasAttacked`): it stays on the shooter once more, even out of sight.
    pub(super) attacked: bool,
    /// What it does when it has nothing else in mind – standing, growling, roaming (`Idle`).
    pub(super) idle: Idle,
    /// Where it heads for – a sound, where it lost sight of what it was after, where it was sent.
    pub(super) lure: Option<Lure>,
    /// With a wave (TAB's `AttackCommandCenter`): it heads for the command center – the waves, the dead rising, the
    /// people of an infected house, and whoever joins them (`joins_wave`). Otherwise it stays where it ends up.
    pub(super) siege: bool,
    /// Came with the final wave (or was called by it) – the round is won once none of these is left (`round.rs`).
    pub final_wave: bool,
    /// The swarm of a wave it came out with – everyone of one wave out of one side has the same number (0: none). The
    /// minimap marks the middle of every swarm (`swarms`).
    pub swarm: u32,
    /// Its step per tick as it last decided – a calm zombie walks on with it until it thinks again (`is_calm`).
    pub(super) step: Vec2,
    /// Ticks it has wanted to stand in a row (0: it walks) – just stopped, it stands firm in the crowd (`crowd.rs`).
    pub(super) still: u32,
    /// Ticks it has been calm in a row – long enough, and far from your eyes, it falls asleep (`sleep.rs`).
    pub(super) calm_for: u32,
    /// In the air over a wall (a harpy, `leap_over`) – nobody pushes it, it pushes nobody, it hits nothing.
    pub(super) leap: Option<Leap>,
    /// Ticks left of a giant's roar (`roar_every_seconds`) – it stands still meanwhile.
    pub(super) roar: u32,
}

/// What a zombie is after – by number, so it stays right while the lists change.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Aim {
    /// One of your units.
    Unit(u32),
    /// A building: one with people in it (TAB's `HasPeople`: the command center, a house, one with workers, a tower
    /// with units inside – `Building::has_people`), or a wall in its way (`wall_in_way`).
    Building(u32),
}

/// What an aim is right now: its index in its list.
#[derive(Clone, Copy)]
enum Aimed {
    Unit(usize),
    Building(usize),
}

/// What an idle zombie does (TAB's `ZombieNormalBehaviour`), and for how long still.
#[derive(Clone, Copy)]
pub(super) struct Idle {
    pub(super) doing: Doing,
    /// Ticks until it rolls anew – roaming: until it gives up on a spot it can't get to.
    timer: u32,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Doing {
    Stand,
    /// Its special animation – TAB's zombie shakes and growls.
    Growl,
    /// On its way to this spot, at its walking pace (`wander_pace`).
    Roam(Vec2),
}

impl Idle {
    /// Standing, and rolling what it does next the next time it thinks.
    pub(super) fn new() -> Idle {
        Idle { doing: Doing::Stand, timer: 0 }
    }

    /// Roaming to `goal` – it gives up after `timer` ticks (a sleeper that wakes on a trip, `sleep.rs`).
    pub(super) fn roaming(goal: Vec2, timer: u32) -> Idle {
        Idle { doing: Doing::Roam(goal), timer }
    }
}

/// A leap over a wall: from where it took off to where it lands.
#[derive(Clone, Copy)]
pub(super) struct Leap {
    from: Vec2,
    to: Vec2,
    /// Ticks in the air so far – it lands after `LEAP_TICKS`.
    flown: u32,
}

/// Where a zombie heads for.
#[derive(Clone, Copy)]
pub(super) struct Lure {
    pub(super) to: Vec2,
    pub(super) why: Why,
    /// It gives up at this tick, if it isn't there yet.
    until: u64,
}

/// Why a zombie goes somewhere – TAB keeps these apart, and they come at different places in its mind.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Why {
    /// It lost sight of what it was after there (TAB's `LastEnemyPosition`). On its way there it doesn't listen.
    Seen,
    /// A sound in this cell of the sound grid (`sound.rs`, TAB's `CellActivity`): it goes there as long as it's loud
    /// there.
    Sound(usize),
    /// Sent there – out of a house of doom, out of the ground around loot: it goes, however quiet it is there.
    Sent,
}

impl Lure {
    fn new(to: Vec2, why: Why, tick: u64) -> Lure {
        Lure { to, why, until: tick + (LURE_PATIENCE * TICKS_PER_SECOND) as u64 }
    }

    /// Where it lost sight of what it was after – or where one of your units is (the houses of doom send them).
    pub(super) fn seen(to: Vec2, tick: u64) -> Lure {
        Lure::new(to, Why::Seen, tick)
    }

    /// The middle `to` of cell `cell` of the sound grid, where it heard a sound.
    pub(super) fn sound(cell: usize, to: Vec2, tick: u64) -> Lure {
        Lure::new(to, Why::Sound(cell), tick)
    }

    /// Sent to `to`.
    pub(super) fn sent(to: Vec2, tick: u64) -> Lure {
        Lure::new(to, Why::Sent, tick)
    }
}

/// What kind of zombie (DESIGN.md 8.1). Most of them only differ in their numbers (`data\zombies.json`); the ranged
/// ones (`range`) attack from afar (`ranged.rs`). `Serialize, Deserialize`: can be written into a map file
/// (`mapfile.rs`) and named in `data\` – by these names, whatever `name` the game shows. TAB's breeds are ours like
/// this: WeakA/B/C → walker, WorkerA/B and MediumA/B → runner, DressedA → riot, StrongA (Chubby) → fat, Venom →
/// spitter, Harpy → harpy, Giant → giant.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum Breed {
    /// The weakest and slowest: lies scattered over the map from the start, and comes out of infected houses.
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

    /// Does it roar (`roar_every_seconds`)? Then it thinks like TAB's giant: it never goes to where it lost sight of
    /// you, never joins a wave.
    pub fn roars(self) -> bool {
        self.stats().roar.is_some()
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

    /// Where it heads for – for the panel and the editor.
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

    /// A zombie of this breed: idling around `pos` and staying there (a roamer), or heading for the command center
    /// with a wave (`siege`).
    pub(super) fn spawn_zombie_as(&mut self, pos: Vec2, breed: Breed, wander: bool) {
        let stats = breed.stats();
        let speed = stats.speed * (0.8 + 0.4 * self.rng.unit_f32());
        let id = self.new_zombie_id();
        let radius = stats.radius;
        self.zombies.push(Zombie {
            pos,
            prev: pos,
            id,
            hp: stats.hp,
            speed,
            radius,
            cooldown: 0,
            breed,
            aim: None,
            attacked: false,
            idle: Idle::new(),
            lure: None,
            siege: !wander,
            final_wave: false,
            swarm: 0,
            step: Vec2::ZERO,
            // Like one that stood for long: put down somewhere, it doesn't stand firm.
            still: rock_ticks() + 1,
            calm_for: 0,
            leap: None,
            roar: 0,
        });
    }

    /// A zombie lured to `to` right away (out of a house of doom, out of the ground around loot): it goes there and
    /// stays.
    pub(super) fn spawn_lured(&mut self, pos: Vec2, breed: Breed, lure: Lure) {
        self.spawn_zombie_as(pos, breed, true);
        if let Some(zombie) = self.zombies.last_mut() {
            zombie.lure = Some(lure);
        }
    }

    /// One of TAB's infected at random (`infected` in `data\zombies.json`) – out of an infected building, out of one
    /// of your fallen units.
    pub(super) fn infected_breed(&mut self) -> Breed {
        let breeds = &values().infected;
        if breeds.is_empty() {
            return Breed::Walker;
        }
        breeds[self.rng.below(breeds.len() as u32) as usize]
    }

    /// Zombie `z` has just been hit by `by` – one of your units, or a building (the tower your units sit in, one that
    /// shoots by itself) – like TAB's (`Zombie.OnAttacked`): if it isn't after anything yet, it and every zombie on its
    /// tile and the four next to it that isn't either go for the shooter, once more even out of sight (`attacked`,
    /// `watch`). One that's after something already doesn't turn – and then neither do the ones around it.
    pub(super) fn zombie_hit_by(&mut self, z: usize, by: Aim) {
        if self.zombies[z].aim.is_some() {
            return;
        }
        let (x, y) = tile_of(self.zombies[z].pos);
        let around = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)];
        let near: Vec<usize> = around.iter().flat_map(|&(dx, dy)| self.grid.cell(x + dx, y + dy)).chain(std::iter::once(z)).collect();
        for i in near {
            let zombie = &mut self.zombies[i];
            if zombie.aim.is_none() && zombie.is_alive() {
                zombie.aim = Some(by);
                zombie.attacked = true;
            }
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
        self.grid.rebuild(self.zombies.iter().map(|z| (z.pos, z.radius)));
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

    /// Where the unit with this number is in the list. The list is in the order of the numbers – new ones go at the
    /// end, nobody is moved – so a binary search finds it quickly: thousands of zombies ask every tick.
    fn unit_index(&self, id: u32) -> Option<usize> {
        self.units.binary_search_by_key(&id, |u| u.id).ok()
    }

    /// The same for a building – that list is in order of the numbers too.
    fn building_index(&self, id: u32) -> Option<usize> {
        self.buildings.binary_search_by_key(&id, |b| b.id).ok()
    }

    /// What `aim` is right now. A unit in a tower: the tower. Gone, or infected (it's theirs now): `None`.
    fn aimed(&self, aim: Aim) -> Option<Aimed> {
        match aim {
            Aim::Unit(id) => {
                let u = self.unit_index(id)?;
                match self.units[u].inside {
                    Some(tower) => self.building_index(tower).map(Aimed::Building),
                    None => Some(Aimed::Unit(u)),
                }
            }
            Aim::Building(id) => self.building_index(id).filter(|&b| !self.buildings[b].zombies_ignore()).map(Aimed::Building),
        }
    }

    /// Where what it's after is – a building: its middle (TAB measures to that).
    fn aim_spot(&self, aim: Aim) -> Option<Vec2> {
        match self.aimed(aim)? {
            Aimed::Unit(u) => Some(self.units[u].pos),
            Aimed::Building(b) => Some(self.buildings[b].center()),
        }
    }

    /// Is it after a wall (or a gate)? It keeps on that one when it sees nothing else (`watch`).
    fn aims_at_wall(&self, aim: Aim) -> bool {
        matches!(self.aimed(aim), Some(Aimed::Building(b)) if self.buildings[b].kind.is_wall())
    }

    /// The buildings with people in them (`Building::has_people`) into `manned` – only when they changed: a house
    /// built or infected, a unit into a tower or out of it. Every tile within sight of a zombie gets the nearest.
    fn mark_manned(&mut self) {
        let mut held: Vec<u32> = self.units.iter().filter_map(|u| u.inside).collect();
        held.sort_unstable();
        held.dedup();
        let ids: Vec<u32> =
            self.buildings.iter().filter(|b| b.has_people(held.binary_search(&b.id).is_ok())).map(|b| b.id).collect();
        if ids == self.manned_ids {
            return;
        }
        let sight = Breed::ALL.iter().map(|b| b.stats().sight).fold(0.0, f32::max);
        self.manned.clear();
        for &id in &ids {
            if let Some(b) = self.building_index(id) {
                let building = &self.buildings[b];
                self.manned.stamp(id as usize, building.corner(), building.far_corner(), sight);
            }
        }
        self.manned_ids = ids;
    }

    pub(super) fn move_zombies(&mut self) {
        // 1. Fill the grid and the maps anew, so that neighbours and targets are found quickly.
        self.fill_zombie_grid();
        self.unit_grid.rebuild(self.units.iter().map(|u| (u.pos, UNIT_RADIUS)));
        // Hitting: only units on the ground – up in a tower they're out of reach (the tower isn't).
        self.aggro.clear();
        for (i, unit) in self.units.iter().enumerate().filter(|(_, u)| !u.is_inside()) {
            self.aggro.stamp(i, unit.pos, unit.pos, REACH);
        }
        // Sight, once per breed – they see differently far. The units in a tower can be seen too – then the zombies
        // go for the tower.
        for breed in Breed::ALL {
            let sight = breed.stats().sight;
            self.zombie_sight[breed.index()].update(&self.map, self.units.iter().map(|u| (u.pos, sight)));
        }
        self.mark_manned();

        // 2. Whose turn it is looks around (`watches_now`) – what it sees is worked out only reading, with many of them
        // on all cores at once (see the top of the file). Then their minds are made up – writing (`update_minds`).
        let due: Vec<bool> = self.zombies.iter().map(|z| self.watches_now(z)).collect();
        let watching = due.iter().filter(|&&d| d).count();
        let world = &*self;
        let watched: Vec<Option<Option<Aim>>> = world.per_zombie(watching, |(i, zombie)| due[i].then(|| world.watch(zombie)));
        self.update_minds(&watched);
        // The way to storm along – only needed if one is after one of your units.
        if self.zombies.iter().any(|z| matches!(z.aim, Some(Aim::Unit(_)))) {
            self.charge.rebuild(&self.map, self.units.iter().map(|u| u.pos), values().charge_steps);
        }
        self.prepare_lure_fields();
        // Who is calm (`is_calm`) – and who of them rests this tick: it isn't its tick to think, and no busy one is next
        // to it. Whoever doesn't rest is in the crowd (`crowd.rs`): pushed and pushing. A calm one only next to a busy
        // one, so it moves every tick then; otherwise it would only be pushed every 8th tick, in jerks.
        let calm: Vec<bool> = self.zombies.iter().map(|z| self.is_calm(z)).collect();
        let near_busy = self.next_to_busy(&calm);
        let every_tick: Vec<bool> = calm.iter().zip(&near_busy).map(|(&c, &near)| !c || near).collect();
        let resting: Vec<bool> = self.zombies.iter().zip(&every_tick).map(|(z, &e)| !e && !self.thinks_now(z)).collect();

        // 3. For every zombie compute where it wants to walk this tick (tiles) and whether it hits – only reading, so with
        // many busy ones on all cores at once. A resting one walks on as it decided.
        let world = &*self;
        let busy = resting.iter().filter(|&&r| !r).count();
        // A harpy in the air flies on; one at a wall that can leap over it takes off (the last: where it lands).
        let intents: Vec<(Vec2, Option<Hit>, Option<Vec2>)> = world.per_zombie(busy, |(i, zombie)| {
            if resting[i] {
                return (zombie.step, None, None);
            }
            if zombie.leap.is_some() {
                return (Vec2::ZERO, None, None);
            }
            let (dir, hit, goal) = world.zombie_intent(zombie);
            if let Some(to) = world.leap_over(zombie, hit, goal) {
                return (Vec2::ZERO, None, Some(to));
            }
            (dir * zombie.speed * TICK, hit.filter(|_| zombie.cooldown == 0), None)
        });
        // Who is in the crowd this tick (`move_crowd`): not resting, not in the air, not taking off.
        let solving: Vec<bool> = self
            .zombies
            .iter()
            .zip(&resting)
            .zip(&intents)
            .map(|((z, &r), &(_, _, leap))| !r && z.leap.is_none() && leap.is_none())
            .collect();
        // Walking on, it keeps a part of its last step (`TURN`) – stopping, it stops at once.
        let wants: Vec<Vec2> = intents
            .iter()
            .zip(&self.zombies)
            .zip(&resting)
            .map(|((&(want, _, _), z), &r)| if r || want == Vec2::ZERO || z.step == Vec2::ZERO { want } else { want.lerp(z.step, TURN) })
            .collect();

        // 4. Deal damage and take off – writing. Walking comes after, all at once (`move_crowd`).
        let tick = self.tick;
        // Buildings whose shield breaks this tick – they let their zombies out after the loop (`infect`).
        let mut infected = Vec::new();
        // The shots of the ranged zombies – fired after the loop (`ranged.rs`), they need all of `self`.
        let mut shots = Vec::new();
        // Every hit makes noise (`sound.rs`) – a crowd tearing at a wall gets loud, a harpy alone does.
        let mut hits = Vec::new();
        for (i, (zombie, ((_, hit, leap), &want))) in self.zombies.iter_mut().zip(intents.into_iter().zip(&wants)).enumerate() {
            zombie.roar = zombie.roar.saturating_sub(1);
            zombie.prev = zombie.pos;
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
                zombie.pos = at;
                zombie.step = Vec2::ZERO;
                zombie.calm_for = 0;
                zombie.cooldown = zombie.cooldown.saturating_sub(1);
                self.grid.moved(i, at);
                continue;
            }
            zombie.step = want;
            zombie.still = if want == Vec2::ZERO { zombie.still.saturating_add(1) } else { 0 };
            zombie.calm_for = if calm.get(i) == Some(&true) { zombie.calm_for.saturating_add(1) } else { 0 };
            zombie.cooldown = zombie.cooldown.saturating_sub(1);
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
        // 5. Everyone walks at once, the crowd pushes and flows (`crowd.rs`). Before the infected houses let their people
        // out – those are in none of the lists yet, they walk the next tick.
        self.move_crowd(&wants, &solving);
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
        // The grid now matches the new positions – the units look up their zombies in it afterwards.
        self.fill_zombie_grid();
    }

    /// Is it this zombie's tick to look around (TAB's `WatchForEnemies`)? Every `watch_seconds` of its breed, each on
    /// another tick by its number – twice as long while it's after something; three times out of your sight, five times
    /// where you've never been (TAB: it saves work, and far out nobody sees them react late). Not in the air.
    fn watches_now(&self, zombie: &Zombie) -> bool {
        if !zombie.is_alive() || zombie.leap.is_some() {
            return false;
        }
        let (x, y) = tile_of(zombie.pos);
        let seen = if self.vision.is_visible(x, y) {
            1
        } else if self.vision.is_explored(x, y) {
            3
        } else {
            5
        };
        let after = if zombie.aim.is_some() { 2 } else { 1 };
        let every = zombie.breed.stats().watch.max(1) as u64 * after * seen;
        (self.tick + zombie.id as u64).is_multiple_of(every)
    }

    /// A zombie looks around (TAB's `CEntityWatcher`): what is it after from now on? The nearest of your units and
    /// buildings with people in it that it sees (`nearest_enemy`). Seeing none, but shot since it last looked: still the
    /// shooter, as long as nothing blocks the view to it. Then the wall rule – not for a harpy, it leaps over walls
    /// (`leap_over`):
    /// - After something further than `WALL_FROM`: a wall in the way to it (`wall_in_way`) – then that wall.
    /// - Nothing in sight, but it was tearing down a wall: on with that one.
    /// - With a wave, or on its way to a sound or where it was sent: a wall in the way to its next step.
    pub(super) fn watch(&self, zombie: &Zombie) -> Option<Aim> {
        let pos = zombie.pos;
        let before = zombie.aim.filter(|&a| self.aimed(a).is_some());
        let shooter = before.filter(|_| zombie.attacked).filter(|&a| self.aim_spot(a).is_some_and(|at| self.map.clear_sight(pos, at)));
        let aim = self.nearest_enemy(zombie).or(shooter);
        if zombie.breed.stats().jump > 0.0 {
            return aim;
        }
        let around = WALL_LOOK + (zombie.radius * 2.0) as i32;
        let wall = |to: Vec2, around: i32| self.wall_in_way(zombie, to, around).map(|b| Aim::Building(self.buildings[b].id));
        match aim {
            Some(a) => match self.aim_spot(a) {
                Some(at) if at.distance(pos) > WALL_FROM => wall(at, around).or(aim),
                _ => aim,
            },
            None if before.is_some_and(|a| self.aims_at_wall(a)) => before,
            None => {
                let heading = zombie.siege || zombie.lure.is_some_and(|l| matches!(l.why, Why::Sound(_) | Why::Sent));
                if heading { self.next_waypoint(zombie).and_then(|to| wall(to, 1)) } else { None }
            }
        }
    }

    /// The nearest of your units and buildings with people in them (`manned`) that the zombie sees: within its `sight`
    /// (from their edge), nothing blocking the view. A unit in a tower: the tower. Equally far: the first found –
    /// tile row after row, a unit before a building (the same on every PC).
    fn nearest_enemy(&self, zombie: &Zombie) -> Option<Aim> {
        let (pos, sight) = (zombie.pos, zombie.breed.stats().sight);
        let mut best: Option<(f32, Aim)> = None;
        // Does any unit see this tile? Seeing goes both ways (`zombie_sight`) – most zombies see nobody, and needn't look.
        if self.zombie_sight[zombie.breed.index()].sees(pos) {
            let r = (sight + UNIT_RADIUS).ceil() as i32;
            let (cx, cy) = tile_of(pos);
            for dy in -r..=r {
                for dx in -r..=r {
                    for u in self.unit_grid.cell(cx + dx, cy + dy) {
                        let unit = &self.units[u];
                        let d = unit.pos.distance(pos) - UNIT_RADIUS;
                        // The line of sight last – it's the expensive part.
                        if d <= sight && best.is_none_or(|(b, _)| d < b) && self.map.clear_sight(pos, unit.pos) {
                            let aim = unit.inside.map_or(Aim::Unit(unit.id), Aim::Building);
                            best = Some((d, aim));
                        }
                    }
                }
            }
        }
        if let Some(b) = self.manned.near(pos).and_then(|id| self.building_index(id as u32)) {
            let building = &self.buildings[b];
            let d = building.distance_to(pos);
            if d <= sight && best.is_none_or(|(b, _)| d < b) && self.map.clear_sight(pos, building.center()) {
                best = Some((d, Aim::Building(building.id)));
            }
        }
        best.map(|(_, aim)| aim)
    }

    /// TAB's wall rule (`CEntityWatcher`): on its way from where the zombie stands to `to`, which of your defences
    /// (walls, gates, towers – `is_defence`) stands in the way? Going diagonally between two wall tiles: one of the two
    /// (by its number). Otherwise of the defences in the square `around` tiles around it, the nearest whose tiles the
    /// straight line to `to` crosses. Equally near: the lower index.
    pub(super) fn wall_in_way(&self, zombie: &Zombie, to: Vec2, around: i32) -> Option<usize> {
        let pos = zombie.pos;
        let defence = |x: i32, y: i32| {
            self.building_on_tile(x, y).filter(|&b| self.buildings[b].kind.is_defence() && !self.buildings[b].zombies_ignore())
        };
        let (c, p) = (tile_of(pos), tile_of(to));
        if (p.0 - c.0).abs() == 1 && (p.1 - c.1).abs() == 1 {
            let wall = |x: i32, y: i32| defence(x, y).filter(|&b| self.buildings[b].kind.is_wall());
            if let (Some(a), Some(b)) = (wall(c.0, p.1), wall(p.0, c.1)) {
                return Some(if zombie.id.is_multiple_of(2) { a } else { b });
            }
        }
        let mut near: Vec<usize> = Vec::new();
        for y in c.1 - around..=c.1 + around {
            for x in c.0 - around..=c.0 + around {
                near.extend(defence(x, y));
            }
        }
        near.sort_unstable();
        near.dedup();
        let crossed = near.into_iter().filter(|&b| segment_hits_rect(pos, to, self.buildings[b].corner(), self.buildings[b].far_corner()));
        crossed.min_by(|&a, &b| {
            let (da, db) = (self.buildings[a].center().distance_squared(pos), self.buildings[b].center().distance_squared(pos));
            da.total_cmp(&db).then(a.cmp(&b))
        })
    }

    /// The middle of the next tile on a zombie's way – with a wave: to the command center; lured: to the place. `None`:
    /// it's there.
    fn next_waypoint(&self, zombie: &Zombie) -> Option<Vec2> {
        let pos = zombie.pos;
        let step = match zombie.lure.filter(|_| !zombie.siege) {
            Some(lure) => self.lure_field(lure.to).and_then(|f| f.step(&self.map, pos)).unwrap_or((lure.to - pos).normalize_or_zero()),
            None => self.to_base.direction(pos),
        };
        let (x, y) = tile_of(pos);
        (step != Vec2::ZERO).then(|| vec2((x + step.x.round() as i32) as f32 + 0.5, (y + step.y.round() as i32) as f32 + 0.5))
    }

    /// What every zombie has in mind before it moves – here, not in `zombie_intent`, because it draws random numbers and
    /// writes (that changes the generator). In this order, per zombie:
    /// 1. What it saw when it looked around (`watched`, from `watch`): its aim from now on. It lost what it was after –
    ///    it goes to where that was (not the giant). What it's after is gone (dead, fallen, infected): forgotten.
    /// 2. Where it was heading: arrived (where it lost you, where it was sent) or given up – forgotten, it idles there.
    ///    Lured by a sound it stays near the place until it's quiet there (`listen_zombies`).
    /// 3. It joins a wave (`joins_wave`).
    /// 4. Idle on its thinking tick (`thinks_now`): its timer counts `THINK_EVERY` ticks at once; run out, it rolls
    ///    what it does next (`next_idle`).
    /// 5. A giant's roar is due: it stops and roars.
    /// 6. Few zombies left: one that has no way to the command center dies (TAB, `cull_below`).
    fn update_minds(&mut self, watched: &[Option<Option<Aim>>]) {
        let tick = self.tick;
        let waves = self.zombies.iter().any(|z| z.siege && z.is_alive());
        let has_center = self.buildings.iter().any(|b| b.kind == BuildingKind::CommandCenter);
        let cull = has_center && self.zombies.len() + self.sleep.len() < values().cull_below;
        for (i, &seen) in watched.iter().enumerate() {
            if let Some(aim) = seen {
                let zombie = &self.zombies[i];
                let lost = zombie.aim.filter(|_| aim.is_none() && !zombie.breed.roars()).and_then(|a| self.aim_spot(a));
                let zombie = &mut self.zombies[i];
                zombie.aim = aim;
                zombie.attacked = false;
                if let Some(at) = lost {
                    zombie.lure = Some(Lure::seen(at, tick));
                }
            }
            if self.zombies[i].aim.is_some_and(|a| self.aimed(a).is_none()) {
                self.zombies[i].aim = None;
            }
            let zombie = &mut self.zombies[i];
            if let Some(lure) = zombie.lure.filter(|_| zombie.aim.is_none()) {
                let there = !matches!(lure.why, Why::Sound(_)) && zombie.pos.distance(lure.to) < LURE_ARRIVED;
                if there || tick >= lure.until {
                    zombie.lure = None;
                    zombie.idle = Idle::new();
                }
            }
            if waves && let Some(swarm) = self.joins_wave(i) {
                let zombie = &mut self.zombies[i];
                zombie.siege = true;
                zombie.lure = None;
                zombie.swarm = swarm;
            }
            let zombie = &self.zombies[i];
            if self.idles(zombie) && self.thinks_now(zombie) {
                let (pos, breed, speed) = (zombie.pos, zombie.breed, zombie.speed);
                let mut idle = zombie.idle;
                idle.timer = idle.timer.saturating_sub(THINK_EVERY as u32);
                if matches!(idle.doing, Doing::Roam(goal) if goal.distance(pos) < 0.3) {
                    idle.timer = 0;
                }
                if idle.timer == 0 {
                    idle = self.next_idle(pos, breed, speed);
                }
                self.zombies[i].idle = idle;
            }
            let zombie = &self.zombies[i];
            let roar_due = zombie.breed.stats().roar.is_some_and(|every| (tick + zombie.id as u64).is_multiple_of(every.max(1) as u64));
            let (x, y) = tile_of(zombie.pos);
            let lost = cull && (tick + zombie.id as u64).is_multiple_of(values().cull_every.max(1) as u64) && !self.to_base.reachable(x, y);
            let zombie = &mut self.zombies[i];
            if roar_due && zombie.roar == 0 {
                zombie.roar = values().growl.max(1);
            }
            if lost {
                zombie.hp = 0;
            }
        }
    }

    /// TAB's contagion (`CheckNearZombiesToAttackCommandCenter`): every `contagion_seconds` – each zombie on another
    /// tick by its number – one that isn't with a wave and has nothing else in mind before that (TAB's tree: not after
    /// anything, not on its way to where it lost you or was sent) counts the wave zombies on its tile with its `level` or
    /// a higher one. At least `contagion_count`: it joins them – the swarm of the first of them (returned). Not the
    /// giant.
    fn joins_wave(&self, i: usize) -> Option<u32> {
        let zombie = &self.zombies[i];
        let every = values().contagion.max(1) as u64;
        if zombie.siege || zombie.breed.roars() || zombie.aim.is_some() || !(self.tick + zombie.id as u64).is_multiple_of(every) {
            return None;
        }
        if zombie.lure.is_some_and(|l| matches!(l.why, Why::Seen | Why::Sent)) {
            return None;
        }
        let level = zombie.breed.stats().level;
        let (x, y) = tile_of(zombie.pos);
        let mut wave = self
            .grid
            .cell(x, y)
            .filter(|&o| o != i)
            .map(|o| &self.zombies[o])
            .filter(|o| o.siege && o.is_alive() && o.breed.stats().level >= level);
        let first = wave.next()?;
        (1 + wave.count() >= values().contagion_count).then_some(first.swarm)
    }

    /// Has it nothing else in mind – does it idle (the last point of TAB's tree)? Not after anything, not with a wave,
    /// not on its way somewhere (a sound: arrived there), not in the air, not roaring.
    fn idles(&self, zombie: &Zombie) -> bool {
        zombie.aim.is_none()
            && !zombie.siege
            && zombie.leap.is_none()
            && zombie.roar == 0
            && zombie.lure.is_none_or(|l| matches!(l.why, Why::Sound(_)) && zombie.pos.distance(l.to) < SOUND_REACHED)
    }

    /// What an idle zombie does next, rolled with TAB's weights (`idle`: 1300 stand, 150 growl, 150 roam): stand or
    /// growl for `stand_seconds` / `growl_seconds`, or roam to a spot (`roam_spot`) – at its walking pace, with the time
    /// it needs and half as much again for detours before it gives up. No spot free: it rolls again the next time.
    pub(super) fn next_idle(&mut self, pos: Vec2, breed: Breed, speed: f32) -> Idle {
        let (z, w) = (values(), &values().idle);
        let roll = self.rng.below((w.stand + w.growl + w.roam).max(1));
        if roll < w.stand {
            Idle { doing: Doing::Stand, timer: z.stand }
        } else if roll < w.stand + w.growl {
            Idle { doing: Doing::Growl, timer: z.growl }
        } else {
            match self.roam_spot(pos, breed) {
                Some(goal) => {
                    let pace = (speed * z.wander_pace).max(0.05);
                    Idle::roaming(goal, (goal.distance(pos) / pace * 1.5 * TICKS_PER_SECOND) as u32 + 40)
                }
                None => Idle::new(),
            }
        }
    }

    /// Where an idle zombie roams to (TAB's `GetRandomRoamingPosition`): on each axis on its own `stroll` tiles away
    /// (between its two values, either way) – a square, not a circle. Up to `ROAM_TRIES` tries: the spot has to be free
    /// ground on the playing field, and not in its `roam_ring` around the command center – unless it stands in it
    /// already. Off the playing field (or on blocked ground) it goes back onto the nearest free tile first.
    pub(super) fn roam_spot(&mut self, pos: Vec2, breed: Breed) -> Option<Vec2> {
        let free = |map: &crate::map::Map, x: i32, y: i32| !map.is_blocked(x, y) && map.in_play(x, y);
        let (x, y) = tile_of(pos);
        if !free(&self.map, x, y) {
            let back = nearest_free_tile(pos, ROAM_BACK, |x, y| !free(&self.map, x, y));
            return back.map(|(x, y)| vec2(x as f32 + 0.5, y as f32 + 0.5));
        }
        let stats = breed.stats();
        let [least, most] = stats.stroll;
        let ring = stats.roam_ring;
        let inside = pos.distance(self.base) <= ring;
        for _ in 0..ROAM_TRIES {
            let mut far = || {
                let sign = if self.rng.below(2) == 0 { -1.0 } else { 1.0 };
                sign * (least + self.rng.unit_f32() * (most + 1.0 - least))
            };
            let (dx, dy) = (far(), far());
            let spot = pos + vec2(dx, dy);
            let (sx, sy) = tile_of(spot);
            if free(&self.map, sx, sy) && (ring <= 0.0 || inside || spot.distance(self.base) > ring) {
                return Some(spot);
            }
        }
        None
    }

    /// The way to the place a zombie heads for (`LureField`, one per cell of the sound grid) – if it was made already.
    fn lure_field(&self, to: Vec2) -> Option<&LureField> {
        self.lures.binary_search_by_key(&cell_xy(to), |f| f.cell).ok().map(|k| &self.lures[k])
    }

    /// Where a zombie walks that needs a way there (`prepare_lure_fields`): the building it's after (the spot of it
    /// closest to it), the place it was lured to. Not to one of your units – that's the storm's way (`charge`) – and not
    /// with a wave (the flow field to the command center).
    fn destination(&self, zombie: &Zombie) -> Option<Vec2> {
        match zombie.aim.and_then(|a| self.aimed(a)) {
            Some(Aimed::Building(b)) => Some(zombie.pos.clamp(self.buildings[b].corner(), self.buildings[b].far_corner())),
            Some(Aimed::Unit(_)) => None,
            None if zombie.siege && zombie.lure.is_none_or(|l| matches!(l.why, Why::Sound(_))) => None,
            None => zombie.lure.filter(|l| !matches!(l.why, Why::Sound(_)) || l.to.distance(zombie.pos) >= SOUND_REACHED).map(|l| l.to),
        }
    }

    /// The ways to the places zombies head for (`LureField`, one per cell of the sound grid) – made when a zombie first
    /// needs one, at most `NEW_LURE_FIELDS` per tick (the others walk straight until then), kept while they're used. In
    /// the order of the cells – the same on every PC.
    fn prepare_lure_fields(&mut self) {
        let mut cells: Vec<(i32, i32)> = self.zombies.iter().filter_map(|z| self.destination(z)).map(cell_xy).collect();
        cells.sort_unstable();
        cells.dedup();
        let mut made = 0;
        for cell in cells {
            match self.lures.binary_search_by_key(&cell, |f| f.cell) {
                Ok(k) => self.lures[k].used = self.tick,
                Err(k) if made < NEW_LURE_FIELDS => {
                    let field = LureField::new(&self.map, cell, CELL, LURE_REACH, CHARGE_THROUGH, self.tick);
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
    /// `zombie_intent`, only roughly.
    pub fn zombie_doing(&self, zombie: &Zombie) -> &'static str {
        if zombie.roar > 0 {
            return "Roaring";
        }
        match zombie.aim.and_then(|a| self.aimed(a)) {
            Some(Aimed::Unit(_)) if zombie.breed.is_ranged() && self.ranged_target(zombie).is_some() => "Shooting at your units",
            Some(Aimed::Unit(_)) => "Storming your units",
            Some(Aimed::Building(b)) if self.buildings[b].kind == BuildingKind::CommandCenter => "Attacking the command center",
            Some(Aimed::Building(b)) if self.buildings[b].kind.is_defence() && !self.buildings[b].has_people(false) => {
                "Tearing down a wall in its way"
            }
            Some(Aimed::Building(_)) => "Attacking a building with people in it",
            None => match zombie.lure.map(|l| l.why) {
                Some(Why::Seen) => "Going where it lost sight of your units",
                Some(Why::Sent) => "Sent somewhere",
                _ if zombie.siege => "Heading for the command center with a wave",
                Some(Why::Sound(_)) if !self.idles(zombie) => "Lured by a sound",
                _ => match zombie.idle.doing {
                    Doing::Stand => "Standing around",
                    Doing::Growl => "Growling",
                    Doing::Roam(_) => "Roaming",
                },
            },
        }
    }

    /// Is this zombie calm – idle with nothing else in mind? Not after anything, not with a wave or the final wave, on
    /// its way nowhere, not busy hitting, not in the air or roaring. Then it only has to think every `THINK_EVERY`
    /// ticks. Asked every tick – the lookups are cheap.
    fn is_calm(&self, zombie: &Zombie) -> bool {
        zombie.aim.is_none()
            && !zombie.siege
            && !zombie.final_wave
            && zombie.lure.is_none()
            && zombie.leap.is_none()
            && zombie.cooldown == 0
            && zombie.roar == 0
    }

    /// Is it this calm zombie's tick to think? Every `THINK_EVERY` ticks, each on another one – by its number, so they
    /// don't all think at once (the same on every PC).
    fn thinks_now(&self, zombie: &Zombie) -> bool {
        (self.tick + zombie.id as u64).is_multiple_of(THINK_EVERY)
    }

    /// What a zombie wants this tick: where to walk (length 1, slower: shorter, standing: 0), whom to hit – and where
    /// it's heading (for `leap_over`). TAB's tree (see the top of the file):
    /// 0. A giant roaring stands still and hits nobody (TAB: it pauses).
    /// 1. A unit within reach: hit it – everyone does that.
    /// 2. After something (`aim`): a ranged zombie shoots at one of your units in range and in sight. A unit: storm it
    ///    (`storm`). A building: go there and hit it (`go_for_building`).
    /// 3. To where it lost sight of what it was after, or where it was sent.
    /// 4. With a wave: follow the flow field to the command center – right through buildings if the detour is too
    ///    long: one on its next step it tears down.
    /// 5. A sound: to the place – arrived, it idles there.
    /// 6. Idle: roaming slowly to its spot, or standing (growling).
    fn zombie_intent(&self, zombie: &Zombie) -> (Vec2, Option<Hit>, Option<Vec2>) {
        let pos = zombie.pos;
        if zombie.roar > 0 {
            return (Vec2::ZERO, None, None);
        }
        if let Some(u) = self.aggro.near(pos) {
            if self.units[u].pos.distance(pos) <= UNIT_RADIUS + zombie.radius + ARM {
                return (Vec2::ZERO, Some(Hit::Unit(u)), None);
            }
        }
        if let Some(aimed) = zombie.aim.and_then(|a| self.aimed(a)) {
            // Over walls too, like TAB's venom: only forests block its view.
            if zombie.breed.is_ranged()
                && let Some(u) = self.ranged_target(zombie)
            {
                return (Vec2::ZERO, Some(Hit::Unit(u)), None);
            }
            return match aimed {
                Aimed::Unit(u) => self.storm(zombie, u),
                Aimed::Building(b) => self.go_for_building(zombie, b),
            };
        }
        if let Some(lure) = zombie.lure.filter(|l| matches!(l.why, Why::Seen | Why::Sent)) {
            let (dir, hit) = self.walk_to(zombie, lure.to, 0.0);
            return (dir, hit, Some(lure.to));
        }
        if zombie.siege {
            let dir = self.to_base.direction(pos);
            let dir = if dir == Vec2::ZERO { (self.base - pos).normalize_or_zero() } else { dir };
            let (dir, hit) = self.step_or_tear_down(zombie, dir);
            return (dir, hit, None);
        }
        if let Some(lure) = zombie.lure.filter(|l| l.to.distance(pos) >= SOUND_REACHED) {
            let (dir, hit) = self.walk_to(zombie, lure.to, 0.0);
            return (dir, hit, Some(lure.to));
        }
        match zombie.idle.doing {
            Doing::Roam(goal) => ((goal - pos).normalize_or_zero() * values().wander_pace, None, None),
            Doing::Stand | Doing::Growl => (Vec2::ZERO, None, None),
        }
    }

    /// Storming unit `u` (TAB's `ChaseEnemies`): clear way – straight there, until its body touches. Otherwise along
    /// the way of the storming (`charge`): around water and mountains, straight through walls – a building on the next
    /// step it tears down. Too far for that way: like to any place (`walk_to`).
    fn storm(&self, zombie: &Zombie, u: usize) -> (Vec2, Option<Hit>, Option<Vec2>) {
        let target = self.units[u].pos;
        let stop = zombie.radius + UNIT_RADIUS;
        if clear_line(&self.map, zombie.pos, target) {
            return (arrive(zombie, target, stop), None, Some(target));
        }
        let (dir, hit) = match self.charge.track(&self.map, zombie.pos) {
            Some((_, step)) if step != Vec2::ZERO => self.step_or_tear_down(zombie, step),
            _ => self.walk_to(zombie, target, stop),
        };
        (dir, hit, Some(target))
    }

    /// Going for building `b` – one with people in it, or a wall in the way: touching it (a ranged one: in range) – hit
    /// it. Otherwise to the spot of it closest to the zombie (`walk_to`).
    fn go_for_building(&self, zombie: &Zombie, b: usize) -> (Vec2, Option<Hit>, Option<Vec2>) {
        let (pos, building) = (zombie.pos, &self.buildings[b]);
        let d = building.distance_to(pos);
        if d <= zombie.radius + ARM || (zombie.breed.is_ranged() && d <= zombie.breed.stats().range) {
            return (Vec2::ZERO, Some(Hit::Building(b)), None);
        }
        // At a wall already, it hits the one it touches: pushed along the wall by the crowd, it doesn't push back to the
        // one it went for – the next time it looks around, TAB's wall rule finds the one in its way from here anyway.
        // Walking back, a crowd at a wall stepped to and fro twice as often.
        if building.kind.is_defence()
            && let Some(t) = self.touching_building(zombie).filter(|&t| self.buildings[t].kind.is_defence())
        {
            return (Vec2::ZERO, Some(Hit::Building(t)), None);
        }
        let closest = pos.clamp(building.corner(), building.far_corner());
        // Just short of its edge – its own tiles are blocked, the line to the edge itself would never be clear. It walks
        // up to `TOUCH_MARGIN` within its reach: stopping right at the end of it, a rounding error left it a hair too
        // far – it never hit, and never stepped closer either.
        let short_of_it = closest + (pos - closest).normalize_or_zero() * 0.05;
        let (dir, hit) = self.walk_to(zombie, short_of_it, zombie.radius + ARM - 0.05 - TOUCH_MARGIN);
        (dir, hit, Some(closest))
    }

    /// Where a zombie walks to get to `to` (TAB's `RunToPosition`), until it's within `stop`: straight, if nothing is
    /// in between. Otherwise along the way to the cell of `to` (`prepare_lure_fields`) – a building on the next step
    /// it tears down. No way yet: straight at it.
    fn walk_to(&self, zombie: &Zombie, to: Vec2, stop: f32) -> (Vec2, Option<Hit>) {
        let pos = zombie.pos;
        if clear_line(&self.map, pos, to) {
            return (arrive(zombie, to, stop), None);
        }
        match self.lure_field(to).and_then(|f| f.step(&self.map, pos)) {
            Some(step) if step != Vec2::ZERO => self.step_or_tear_down(zombie, step),
            _ => ((to - pos).normalize_or_zero(), None),
        }
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
    /// lies behind it: `goal`, where it's heading (otherwise the command center). Straight towards that it looks for
    /// free ground behind the wall, at most `jump` tiles of wall away: where it lands (the middle of that tile). Not
    /// over anything else – a house, a tower, water, a mountain, a forest, a wall too thick: it hits.
    fn leap_over(&self, zombie: &Zombie, hit: Option<Hit>, goal: Option<Vec2>) -> Option<Vec2> {
        let jump = zombie.breed.stats().jump;
        let Some(Hit::Building(b)) = hit else {
            return None;
        };
        if jump <= 0.0 || !self.buildings[b].kind.is_wall() {
            return None;
        }
        let pos = zombie.pos;
        let goal = goal.unwrap_or(self.base);
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

    /// Per zombie: is there a busy one (not `calm`) or one of your units on its tile or a tile around it? Only then can
    /// somebody walk into a calm one and push it – touching, even two giants are less than a tile apart. Marked tile by
    /// tile around them, so a calm crowd far from them costs nothing more.
    fn next_to_busy(&self, calm: &[bool]) -> Vec<bool> {
        // Nobody calm (the final wave called them all) or nobody to push them: nothing to look for.
        if !calm.contains(&true) || (!calm.contains(&false) && self.units.is_empty()) {
            return vec![false; calm.len()];
        }
        let n = self.map.size as i32;
        let tile = |x: i32, y: i32| (x >= 0 && y >= 0 && x < n && y < n).then(|| (y * n + x) as usize);
        let mut near = vec![false; (n * n) as usize];
        // Your units on the ground push too.
        let busy = self.zombies.iter().zip(calm).filter(|&(_, &c)| !c).map(|(z, _)| z.pos);
        let units = self.units.iter().filter(|u| !u.is_inside()).map(|u| u.pos);
        for pos in busy.chain(units) {
            let (x, y) = (pos.x.floor() as i32, pos.y.floor() as i32);
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
    pub(super) fn per_zombie<T: Send>(&self, busy: usize, f: impl Fn((usize, &Zombie)) -> T + Sync + Send) -> Vec<T> {
        if busy >= SIDE_BY_SIDE_FROM {
            self.zombies.par_iter().enumerate().with_min_len(PER_CORE).map(f).collect()
        } else {
            self.zombies.iter().enumerate().map(f).collect()
        }
    }
}

/// The zombies' values from `data\zombies.json`.
pub(super) fn values() -> &'static data::Zombies {
    &data::get().zombies
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

/// The tile a point lies on.
fn tile_of(p: Vec2) -> (i32, i32) {
    (p.x.floor() as i32, p.y.floor() as i32)
}

/// Does the straight line from `a` to `b` cross the rectangle `min`–`max` (TAB's `IntersectWithSegment`)? Both axes
/// cut the line down to the part inside their band of the rectangle ("slabs"); something left of it – it crosses.
fn segment_hits_rect(a: Vec2, b: Vec2, min: Vec2, max: Vec2) -> bool {
    let d = b - a;
    let (mut enter, mut leave) = (0.0f32, 1.0f32);
    for (from, step, low, high) in [(a.x, d.x, min.x, max.x), (a.y, d.y, min.y, max.y)] {
        if step == 0.0 {
            if from < low || from > high {
                return false;
            }
            continue;
        }
        let (t0, t1) = ((low - from) / step, (high - from) / step);
        enter = enter.max(t0.min(t1));
        leave = leave.min(t0.max(t1));
        if enter > leave {
            return false;
        }
    }
    true
}
