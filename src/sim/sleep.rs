//! Sleeping zombies (DESIGN.md 8.5). Most of the horde lies far from everything – those aren't zombies in the list
//! (`World::zombies`), only an entry in a block of `BLOCK` × `BLOCK` tiles: where, which breed, its hit points and its
//! pace. They cost nothing per tick, so a map can hold a hundred thousand and more.
//!
//! They wake – become zombies in the list again, right where they lay – when
//! 1. one of your eyes comes close (a unit or a building: its sight plus `WAKE_MARGIN` tiles), before it can see them.
//!    In the picture they fade in (`draw.rs`) instead of popping up at the edge of sight;
//! 2. it hears a sound – it listens like the zombies awake, by the same rule and dice (`sound.rs`), and goes to the
//!    middle of the cell it heard;
//! 3. the scan shows their place, or a barrel goes off next to them;
//! 4. the final wave calls everyone: then all of them, `WAKE_PER_TICK` a tick, the nearest to the base first – they're
//!    the first to arrive anyway;
//! 5. a wave marches over it: enough wave zombies of its rank or higher on its tile, and it wakes and joins them
//!    (`wake_joining`, TAB's contagion – in TAB nobody sleeps, so a wave takes along every crowd it walks through).
//!
//! The other way round: an idle one that has been calm for `SLEEP_AFTER` seconds and is far from all your eyes – another
//! `SLEEP_MARGIN` tiles further than they would wake it, so nobody wakes and falls asleep again and again at that edge –
//! falls asleep where it stands (`fall_asleep`). It keeps its hit points. Not any more once the final wave has called.
//!
//! **Sleepers idle too** (`stroll_sleepers`) – by the same rule and dice as the zombies awake (`zombies.rs`): TAB's
//! stand / growl / roam, rolled ahead until it's roam (`idle_wait`), then a trip to a spot found like theirs
//! (`roam_spot`). Nobody sees it, so it arrives at once – and then waits as long as the walk would have taken, and its
//! standing and growling, before the next trip. Is something in the straight way (water, a mountain), it stays where it
//! is, like one stuck there. A trip that would end near your eyes wakes it where it starts, and it walks the rest awake –
//! it comes into sight from outside, fading in. So a place you've emptied fills up again: the sleepers around drift in.

use macroquad::math::{vec2, Vec2};

use super::zombies::{values, Breed, Idle, Lure};
use super::{clear_line, World, DAY_TICKS, TICKS_PER_SECOND};
use crate::data;

/// Edge of a block (tiles).
const BLOCK: i32 = 8;
/// Every this many ticks the eyes wake the blocks around them (0.5 s) …
pub(super) const WAKE_EVERY: u64 = 10;
/// … everything within their sight and this many tiles more.
const WAKE_MARGIN: f32 = 8.0;
/// A calm zombie falls asleep only this much further from every eye than they would wake it …
const SLEEP_MARGIN: f32 = 8.0;
/// … and only after this long calm (seconds).
const SLEEP_AFTER: f32 = 20.0;
/// The final wave wakes at least this many sleepers a tick (whole blocks at a time).
const WAKE_PER_TICK: usize = 2000;
/// Every this many ticks the sleepers whose trip is due make it (1 s, `stroll_sleepers`).
const STROLL_EVERY: u64 = 20;

/// A sleeping zombie: all it needs to wake as the one it was – and when it makes its next trip (`stroll_sleepers`).
#[derive(Clone, Copy)]
pub struct Sleeper {
    pub pos: Vec2,
    pub breed: Breed,
    hp: u32,
    speed: f32,
    next: u64,
}

/// The sleepers, block by block.
#[derive(Clone)]
pub struct Sleep {
    /// Blocks across (and down) – the map is square.
    side: i32,
    /// Per block its sleepers, in the order they lay down.
    blocks: Vec<Vec<Sleeper>>,
    count: usize,
    /// The final wave has called (`call_sleepers`): from now on nobody falls asleep …
    called: bool,
    /// … and these blocks are still to wake – the nearest to the base at the end, taken from there.
    waiting: Vec<usize>,
}

impl Sleep {
    pub fn new(map_size: usize) -> Sleep {
        let side = (map_size as i32 + BLOCK - 1) / BLOCK;
        Sleep { side, blocks: vec![Vec::new(); (side * side) as usize], count: 0, called: false, waiting: Vec::new() }
    }

    /// How many sleep right now.
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Every sleeper, block by block.
    pub fn iter(&self) -> impl Iterator<Item = &Sleeper> {
        self.blocks.iter().flatten()
    }

    /// Has the final wave called, and are some still waiting to wake? Then the round isn't won yet.
    pub fn still_waking(&self) -> bool {
        !self.waiting.is_empty()
    }

    /// Everybody gone (the editor clears the map).
    pub(super) fn clear(&mut self) {
        for block in &mut self.blocks {
            block.clear();
        }
        self.count = 0;
        self.waiting.clear();
    }

    /// Takes out every sleeper `gone` says (the editor's eraser).
    pub(super) fn remove(&mut self, gone: impl Fn(Vec2) -> bool) {
        for block in &mut self.blocks {
            let before = block.len();
            block.retain(|s| !gone(s.pos));
            self.count -= before - block.len();
        }
    }

    fn block_of(&self, p: Vec2) -> usize {
        let bx = (p.x.floor() as i32).div_euclid(BLOCK).clamp(0, self.side - 1);
        let by = (p.y.floor() as i32).div_euclid(BLOCK).clamp(0, self.side - 1);
        (by * self.side + bx) as usize
    }

    fn push(&mut self, sleeper: Sleeper) {
        let b = self.block_of(sleeper.pos);
        self.blocks[b].push(sleeper);
        self.count += 1;
    }

    fn take(&mut self, b: usize) -> Vec<Sleeper> {
        let sleepers = std::mem::take(&mut self.blocks[b]);
        self.count -= sleepers.len();
        sleepers
    }

    /// The blocks touching the circle of radius `r` around `at` – in order.
    fn blocks_near(&self, at: Vec2, r: f32) -> impl Iterator<Item = usize> + '_ {
        let reach = |c: f32| ((c / BLOCK as f32).floor() as i32).clamp(0, self.side - 1);
        let (x0, x1, y0, y1) = (reach(at.x - r), reach(at.x + r), reach(at.y - r), reach(at.y + r));
        (y0..=y1).flat_map(move |by| (x0..=x1).map(move |bx| (bx, by))).filter_map(move |(bx, by)| {
            let min = vec2((bx * BLOCK) as f32, (by * BLOCK) as f32);
            let closest = at.clamp(min, min + Vec2::splat(BLOCK as f32));
            (closest.distance(at) <= r).then_some((by * self.side + bx) as usize)
        })
    }

    /// The middle of block `b`.
    fn middle(&self, b: usize) -> Vec2 {
        let (bx, by) = (b as i32 % self.side, b as i32 / self.side);
        vec2((bx * BLOCK) as f32, (by * BLOCK) as f32) + Vec2::splat(BLOCK as f32 / 2.0)
    }
}

/// What a sleeper does once it's awake.
#[derive(Clone, Copy)]
enum Waking {
    /// Idles where it lay.
    Calm,
    /// On a trip, to this spot – giving up after so many ticks.
    Strolling(Vec2, u32),
    /// Goes where it heard something.
    Lured(Lure),
    /// Joins the wave walking over it – this swarm.
    Joins(u32),
    /// Heads for the command center with the final wave.
    Final,
}

impl World {
    /// A new roamer, asleep from the start (`scatter_roamers`) – with its own pace, rolled now like a zombie's. It
    /// stands and growls before its first trip, like one awake (`idle_wait`).
    pub(super) fn lay_to_sleep(&mut self, pos: Vec2, breed: Breed) {
        let stats = breed.stats();
        let speed = stats.speed * (0.8 + 0.4 * self.rng.unit_f32());
        let next = self.tick + self.idle_wait();
        self.sleep.push(Sleeper { pos, breed, hp: stats.hp, speed, next });
    }

    /// How long an idle zombie stands and growls before it roams next – TAB's dice of the zombies awake (`next_idle`),
    /// rolled ahead for a sleeper: stand, growl, again and again until it comes up roam. No weight on roaming: a day.
    fn idle_wait(&mut self) -> u64 {
        let (z, w) = (values(), &values().idle);
        if w.roam == 0 {
            return DAY_TICKS;
        }
        let mut wait = 0;
        loop {
            let roll = self.rng.below(w.stand + w.growl + w.roam);
            if roll < w.stand {
                wait += z.stand as u64;
            } else if roll < w.stand + w.growl {
                wait += z.growl as u64;
            } else {
                return wait;
            }
        }
    }

    /// Every tick: the eyes wake what's around them (every `WAKE_EVERY` ticks), the sleepers whose trip is due make it
    /// (every `STROLL_EVERY`), after the final wave's call the next ones wake to join it, and the ones a wave walks over
    /// join it (`wake_joining`).
    pub(super) fn wake_sleepers(&mut self) {
        if self.tick.is_multiple_of(WAKE_EVERY) {
            self.wake_near_eyes();
        }
        if self.tick.is_multiple_of(STROLL_EVERY) {
            self.stroll_sleepers();
        }
        self.wake_called();
        self.wake_joining();
    }

    /// TAB's contagion for the sleepers (`joins_wave` in `zombies.rs`): every `contagion_seconds`, block by block – each
    /// block on the tick its number gives it – a sleeper with at least `contagion_count` wave zombies of its `level` or a
    /// higher one on its tile wakes and joins them. Not the giant.
    fn wake_joining(&mut self) {
        if self.sleep.is_empty() {
            return;
        }
        let (every, count) = (values().contagion.max(1) as u64, values().contagion_count);
        let tick = self.tick;
        let due: Vec<usize> = (0..self.sleep.blocks.len())
            .filter(|&b| (tick + b as u64).is_multiple_of(every) && !self.sleep.blocks[b].is_empty())
            .collect();
        if due.is_empty() {
            return;
        }
        // The wave zombies by tile – (tile row and column, rank, swarm), in order.
        let tile = |p: Vec2| (p.y.floor() as i32, p.x.floor() as i32);
        let mut waves: Vec<((i32, i32), u32, u32)> =
            self.zombies.iter().filter(|z| z.siege && z.is_alive()).map(|z| (tile(z.pos), z.breed.stats().level, z.swarm)).collect();
        if waves.is_empty() {
            return;
        }
        waves.sort_unstable();
        for b in due {
            let mut stay = Vec::new();
            let mut joining = Vec::new();
            for sleeper in self.sleep.take(b) {
                let (at, level) = (tile(sleeper.pos), sleeper.breed.stats().level);
                let from = waves.partition_point(|w| w.0 < at);
                let mut here = waves[from..].iter().take_while(|w| w.0 == at).filter(|w| w.1 >= level);
                let swarm = here.next().map(|w| w.2);
                match swarm.filter(|_| !sleeper.breed.roars() && 1 + here.count() >= count) {
                    Some(swarm) => joining.push((sleeper, swarm)),
                    None => stay.push(sleeper),
                }
            }
            self.sleep.count += stay.len();
            self.sleep.blocks[b] = stay;
            for (sleeper, swarm) in joining {
                self.wake(sleeper, Waking::Joins(swarm));
            }
        }
    }

    /// Per block: is it within `extra` tiles more than one of your eyes sees?
    fn near_eyes(&self, extra: f32) -> Vec<bool> {
        let mut near = vec![false; self.sleep.blocks.len()];
        for (at, sight) in self.eyes() {
            for b in self.sleep.blocks_near(at, sight + extra) {
                near[b] = true;
            }
        }
        near
    }

    /// The sleepers whose trip is due make it – to a spot found like the zombies awake find theirs (`roam_spot`). It
    /// arrives at once (nobody sees it) and its next trip comes after the walk would have taken and its standing and
    /// growling (`idle_wait`). Something in the straight way (or no free spot found): it stays where it is. A trip
    /// ending where your eyes wake everyone wakes it where it starts – it walks the rest awake, into sight from outside.
    /// After the final wave's call nobody roams: they all wake anyway.
    fn stroll_sleepers(&mut self) {
        if self.sleep.is_empty() || self.sleep.called {
            return;
        }
        let awake = self.near_eyes(WAKE_MARGIN);
        let share = values().wander_pace;
        let tick = self.tick;
        let mut moved = Vec::new();
        let mut waking = Vec::new();
        for b in 0..self.sleep.blocks.len() {
            if self.sleep.blocks[b].iter().all(|s| s.next > tick) {
                continue;
            }
            let mut stay = Vec::new();
            for mut sleeper in self.sleep.take(b) {
                if sleeper.next > tick {
                    stay.push(sleeper);
                    continue;
                }
                let spot = self.roam_spot(sleeper.pos, sleeper.breed);
                let standing = self.idle_wait();
                match spot.filter(|&to| clear_line(&self.map, sleeper.pos, to)) {
                    Some(to) => {
                        let pace = (sleeper.speed * share).max(0.05);
                        let walk = sleeper.pos.distance(to) / pace * TICKS_PER_SECOND;
                        if awake[self.sleep.block_of(to)] {
                            // Like `next_idle`: the time it needs at its pace, half as much again for detours.
                            waking.push((sleeper, to, (walk * 1.5) as u32 + 40));
                        } else {
                            sleeper.pos = to;
                            sleeper.next = tick + walk as u64 + standing;
                            moved.push(sleeper);
                        }
                    }
                    None => {
                        sleeper.next = tick + standing;
                        stay.push(sleeper);
                    }
                }
            }
            self.sleep.count += stay.len();
            self.sleep.blocks[b] = stay;
        }
        // Into the block where they lie now – after the loop, so nobody makes two trips at once.
        for sleeper in moved {
            self.sleep.push(sleeper);
        }
        for (sleeper, to, timer) in waking {
            self.wake(sleeper, Waking::Strolling(to, timer));
        }
    }

    /// Your eyes – units (in a tower they see further) and buildings that aren't infected: where, and how far they see.
    fn eyes(&self) -> Vec<(Vec2, f32)> {
        let units = self.units.iter().map(|u| (u.pos, u.sight()));
        let buildings = self.buildings.iter().filter(|b| !b.infected).map(|b| (b.center(), b.stats().sight));
        units.chain(buildings).collect()
    }

    /// Wakes every block within sight (plus `WAKE_MARGIN`) of one of your eyes – also right at the start of a round and
    /// after loading a map, so nobody near the base sleeps.
    pub(super) fn wake_near_eyes(&mut self) {
        if self.sleep.is_empty() {
            return;
        }
        for (at, sight) in self.eyes() {
            self.wake_area(at, sight + WAKE_MARGIN);
        }
    }

    /// Wakes every block touching the circle of radius `r` around `at` (the eyes, the scan, a barrel going off).
    pub(super) fn wake_area(&mut self, at: Vec2, r: f32) {
        let blocks: Vec<usize> = self.sleep.blocks_near(at, r).filter(|&b| !self.sleep.blocks[b].is_empty()).collect();
        for b in blocks {
            for sleeper in self.sleep.take(b) {
                self.wake(sleeper, Waking::Calm);
            }
        }
    }

    /// The sleepers listen like the zombies awake (`listen_zombies` in `sound.rs`): every `listen_seconds`, block by
    /// block – each block on the tick its number gives it. One that comes wakes and goes to the middle of the cell it
    /// heard best. In the order they lay – the same on every PC.
    pub(super) fn listen_sleepers(&mut self) {
        if self.sleep.is_empty() || self.sound.is_quiet() {
            return;
        }
        let every = data::get().sound.listen.max(1) as u64;
        let tick = self.tick;
        let due: Vec<usize> = (0..self.sleep.blocks.len())
            .filter(|&b| (tick + b as u64).is_multiple_of(every) && !self.sleep.blocks[b].is_empty())
            .collect();
        for b in due {
            let mut stay = Vec::new();
            let mut woken = Vec::new();
            for sleeper in self.sleep.take(b) {
                let best = self.sound.best(sleeper.pos, sleeper.breed.hearing_range(), sleeper.breed.hear_factor());
                match best.filter(|&(_, chance)| self.rng.below(100) < chance) {
                    Some((c, _)) => woken.push((sleeper, c)),
                    None => stay.push(sleeper),
                }
            }
            self.sleep.count += stay.len();
            self.sleep.blocks[b] = stay;
            for (sleeper, c) in woken {
                let lure = Lure::sound(c, self.sound.middle(c), tick);
                self.wake(sleeper, Waking::Lured(lure));
            }
        }
    }

    /// The final wave calls everyone (`call_everyone`): the sleepers too – a few thousand per tick, the nearest first
    /// (`wake_called`). From now on nobody falls asleep any more.
    pub(super) fn call_sleepers(&mut self) {
        self.sleep.called = true;
        let base = self.base;
        let mut waiting: Vec<usize> = (0..self.sleep.blocks.len()).filter(|&b| !self.sleep.blocks[b].is_empty()).collect();
        // The furthest first in the list – taken from the end, so the nearest come first. Equally far: by number.
        waiting.sort_by(|&a, &b| {
            let (da, db) = (self.sleep.middle(a).distance(base), self.sleep.middle(b).distance(base));
            db.total_cmp(&da).then(b.cmp(&a))
        });
        self.sleep.waiting = waiting;
    }

    /// After the final wave's call: the next blocks wake and join it – at least `WAKE_PER_TICK` sleepers a tick.
    fn wake_called(&mut self) {
        let mut woken = 0;
        while woken < WAKE_PER_TICK {
            let Some(b) = self.sleep.waiting.pop() else {
                return;
            };
            for sleeper in self.sleep.take(b) {
                self.wake(sleeper, Waking::Final);
                woken += 1;
            }
        }
    }

    /// A sleeper wakes: a zombie in the list again, where it lay, with its hit points and its pace.
    fn wake(&mut self, sleeper: Sleeper, waking: Waking) {
        self.spawn_zombie_as(sleeper.pos, sleeper.breed, true);
        let Some(zombie) = self.zombies.last_mut() else {
            return;
        };
        zombie.hp = sleeper.hp;
        zombie.speed = sleeper.speed;
        match waking {
            Waking::Calm => {}
            Waking::Strolling(goal, timer) => zombie.idle = Idle::roaming(goal, timer),
            Waking::Lured(lure) => zombie.lure = Some(lure),
            Waking::Joins(swarm) => {
                zombie.siege = true;
                zombie.swarm = swarm;
            }
            Waking::Final => {
                zombie.siege = true;
                zombie.final_wave = true;
            }
        }
    }

    /// Every now and then, at the end of a tick: idle ones that have been calm for `SLEEP_AFTER` and lie far from all
    /// your eyes fall asleep where they stand – they leave the list (`remove_dead` calls this before sorting it).
    pub(super) fn fall_asleep(&mut self) {
        if self.sleep.called || self.zombies.is_empty() {
            return;
        }
        // The blocks where nobody may fall asleep: near an eye.
        let kept = self.near_eyes(WAKE_MARGIN + SLEEP_MARGIN);
        let calm_long = (SLEEP_AFTER * TICKS_PER_SECOND) as u32;
        let mut sleepers = Vec::new();
        let sleep = &self.sleep;
        self.zombies.retain(|z| {
            let sleepy = z.is_alive() && z.calm_for >= calm_long && !kept[sleep.block_of(z.pos)];
            if sleepy {
                sleepers.push(Sleeper { pos: z.pos, breed: z.breed, hp: z.hp, speed: z.speed, next: 0 });
            }
            !sleepy
        });
        // Asleep it idles on – first it stands and growls (`idle_wait`).
        for mut sleeper in sleepers {
            sleeper.next = self.tick + self.idle_wait();
            self.sleep.push(sleeper);
        }
    }
}
