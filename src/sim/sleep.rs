//! Sleeping zombies (DESIGN.md 8.5). Most of the horde lies far from everything – those aren't zombies in the list
//! (`World::zombies`), only an entry in a block of `BLOCK` × `BLOCK` tiles: where, which breed, its hit points and its
//! pace. They cost nothing per tick, so a map can hold a hundred thousand and more.
//!
//! They wake – become zombies in the list again, right where they lay – when
//! 1. one of your eyes comes close (a unit or a building: its sight plus `WAKE_MARGIN` tiles), before it can see them.
//!    In the picture they fade in (`draw.rs`) instead of popping up at the edge of sight;
//! 2. a sound is loud enough for one of them – the same rule and the same dice as for the zombies that are awake
//!    (`sound.rs`). It goes where the sound was;
//! 3. the scan shows their place, or a barrel goes off next to them;
//! 4. the final wave calls everyone: then all of them, `WAKE_PER_TICK` a tick, the nearest to the base first – they're
//!    the first to arrive anyway.
//!
//! The other way round: a stroller that has been calm for `SLEEP_AFTER` seconds and is far from all your eyes – another
//! `SLEEP_MARGIN` tiles further than they would wake it, so nobody wakes and falls asleep again and again at that edge –
//! falls asleep where it stands (`fall_asleep`). It keeps its hit points. Not any more once the final wave has called.
//!
//! **Sleepers stroll too** (`stroll_sleepers`) – trip after trip by the same rule and dice as the strollers awake
//! (`zombies.rs`): a random free spot up to `stroll` tiles from where it lies, never into the ring around the command
//! center. Nobody sees it, so it arrives at once – and then waits as long as the walk would have taken, and stands its
//! 1–5 s, before the next trip. Is something in the straight way (water, a mountain), it stays where it is, like a
//! stroller stuck there. A trip that would end near your eyes wakes it where it starts, and it walks the rest awake – it
//! comes into sight from outside, fading in. So a place you've emptied fills up again: the sleepers around drift in.

use macroquad::math::{vec2, Vec2};

use super::zombies::{values, Breed, Lure, Wander};
use super::{clear_line, keeps_out, World, TICKS_PER_SECOND};
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
    /// Strolls where it lay.
    Calm,
    /// On a trip, to this spot – giving up after so many ticks.
    Strolling(Vec2, u32),
    /// Goes where it heard something.
    Lured(Lure),
    /// Heads for the command center with the final wave.
    Final,
}

impl World {
    /// A new roamer, asleep from the start (`scatter_roamers`) – with its own pace, rolled now like a zombie's. It
    /// stands its 1–5 s before its first trip, like a new stroller.
    pub(super) fn lay_to_sleep(&mut self, pos: Vec2, breed: Breed) {
        let stats = breed.stats();
        let speed = stats.speed * (0.8 + 0.4 * self.rng.unit_f32());
        let next = self.tick + self.standing();
        self.sleep.push(Sleeper { pos, breed, hp: stats.hp, speed, next });
    }

    /// How long a stroller stands around before its next trip – 1 to 5 s, like an awake one (`update_minds`).
    fn standing(&mut self) -> u64 {
        20 + self.rng.below(80) as u64
    }

    /// Every tick: the eyes wake what's around them (every `WAKE_EVERY` ticks), the sleepers whose trip is due make it
    /// (every `STROLL_EVERY`), and after the final wave's call the next ones wake to join it.
    pub(super) fn wake_sleepers(&mut self) {
        if self.tick.is_multiple_of(WAKE_EVERY) {
            self.wake_near_eyes();
        }
        if self.tick.is_multiple_of(STROLL_EVERY) {
            self.stroll_sleepers();
        }
        self.wake_called();
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

    /// The sleepers whose trip is due make it – by the same rule and dice as the strollers awake (`update_minds`): a
    /// random free spot up to `stroll` tiles from where it lies, never into its breed's `keep_out` ring. It arrives at once
    /// (nobody sees it) and its next trip comes after the walk would have taken and its 1–5 s of standing. Something
    /// in the straight way (or no free spot found): it stays where it is. A trip ending where your eyes wake everyone
    /// wakes it where it starts – it walks the rest awake, into sight from outside. After the final wave's call nobody
    /// strolls: they all wake anyway.
    fn stroll_sleepers(&mut self) {
        if self.sleep.is_empty() || self.sleep.called {
            return;
        }
        let awake = self.near_eyes(WAKE_MARGIN);
        let (keep_out, share, inside_step) = (self.keep_out, values().wander_pace, data::get().roaming.inside_step);
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
                let radius = sleeper.breed.stats().stroll;
                let mut spot = None;
                for _ in 0..5 {
                    let offset = vec2(self.rng.unit_f32() * 2.0 - 1.0, self.rng.unit_f32() * 2.0 - 1.0) * radius;
                    let to = sleeper.pos + offset;
                    let free = !self.map.is_blocked(to.x.floor() as i32, to.y.floor() as i32);
                    if free && keeps_out(sleeper.pos, to, self.base, keep_out.of(sleeper.breed), inside_step) {
                        spot = Some(to);
                        break;
                    }
                }
                let standing = self.standing();
                match spot.filter(|&to| clear_line(&self.map, sleeper.pos, to)) {
                    Some(to) => {
                        let pace = (sleeper.speed * share).max(0.05);
                        let walk = sleeper.pos.distance(to) / pace * TICKS_PER_SECOND;
                        if awake[self.sleep.block_of(to)] {
                            // Like `update_minds`: the time it needs at its pace, half as much again for detours.
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

    /// A sound `level` loud at `at`: every sleeper that hears it loud enough may wake and come – by the same rule and
    /// dice as the zombies awake (`lure_to_sound`): just loud enough, every other time; twice as loud, always. Block by
    /// block, in the order they lay – the same on every PC.
    pub(super) fn rouse_sleepers(&mut self, at: Vec2, level: u32) {
        if self.sleep.is_empty() {
            return;
        }
        let reach = Breed::ALL.iter().map(|b| b.hearing_range()).fold(0.0, f32::max);
        let blocks: Vec<usize> = self.sleep.blocks_near(at, reach).filter(|&b| !self.sleep.blocks[b].is_empty()).collect();
        let tick = self.tick;
        for b in blocks {
            let mut stay = Vec::new();
            let mut woken = Vec::new();
            for sleeper in self.sleep.take(b) {
                let (range, d) = (sleeper.breed.hearing_range(), sleeper.pos.distance(at));
                let heard = if d <= range { level - (level as f32 * d / (2.0 * range)) as u32 } else { 0 };
                let threshold = sleeper.breed.threshold();
                let comes = heard >= threshold && self.rng.unit_f32() < heard as f32 / (2 * threshold.max(1)) as f32;
                if comes {
                    woken.push((sleeper, heard));
                } else {
                    stay.push(sleeper);
                }
            }
            self.sleep.count += stay.len();
            self.sleep.blocks[b] = stay;
            for (sleeper, heard) in woken {
                self.wake(sleeper, Waking::Lured(Lure::new(at, heard, tick)));
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
            Waking::Strolling(goal, timer) => zombie.wander = Some(Wander::towards(goal, timer)),
            Waking::Lured(lure) => {
                zombie.lure = Some(lure);
                zombie.wander = None;
            }
            Waking::Final => {
                zombie.siege = true;
                zombie.final_wave = true;
                zombie.wander = None;
            }
        }
    }

    /// Every now and then, at the end of a tick: strollers that have been calm for `SLEEP_AFTER` and lie far from all
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
        // Asleep it strolls on – first it stands its 1–5 s.
        for mut sleeper in sleepers {
            sleeper.next = self.tick + self.standing();
            self.sleep.push(sleeper);
        }
    }
}
