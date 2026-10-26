//! Sound like in They Are Billions (DESIGN.md 8.3). Everything that happens makes noise where it happens: your units'
//! shots (`noise` in `data\units.json` – ranger 1, soldier 3, sniper 10 per shot, like TAB), zombies hitting something,
//! a building being placed, a unit dying, a building destroyed – and, loudest of all, a building being infected. The
//! other numbers are in `data\sound.json`.
//!
//! - The noise **adds up** where it's made, per cell of `CELL` × `CELL` tiles, as long as it goes on – a long fight gets
//!   louder and louder, a squad standing close together is louder than one spread out. Once a cell has been quiet for
//!   `quiet_seconds`, it **halves** every `halve_seconds` (TAB: after a second, every second).
//! - Twice a second every loud cell is listened to (`listen`): a zombie **hears** it within `hearing` times its sight
//!   (TAB: 4), quieter the further away, and if it's at least `threshold / alertness` (TAB: 1000 / 2 for the slow
//!   ones …) it may **come** – by chance, the louder the surer (TAB's "a share of them"). It goes to where the sound was
//!   (`Lure` in `zombies.rs`) and stays there unless it finds something.
//! - A house of a village of doom that hears it lets its zombies out, the hidden guards of loot come out of the ground
//!   – they all go where the sound was (`doom.rs`).
//! - **Machines** (Factorio's pollution as TAB's noise): every `work_every_seconds` every working building makes its
//!   `noise` (`data\buildings.json`) – all at the same moment, so buildings close together add up. One alone is never
//!   loud enough to lure anybody; a mine field with a charcoal burner wakes a village 24 tiles away.
//! - **Unrest:** everything your colony makes – shots, machines, blasts, building – also adds up in `World::unrest`,
//!   which never goes down. For every `unrest_per_zombie` of it, every group of the constant spawn brings one zombie
//!   more, for good (`spawning.rs`): growing loud has a price that tearing down the factory later doesn't undo.
//!
//! Only whole numbers – the same on every PC (DESIGN.md 12.8).

use macroquad::math::Vec2;

use super::zombies::{Breed, Lure};
use super::World;
use crate::data;

/// Edge of a cell of the sound grid (tiles) – noise in it adds up. Also the cell of the lure fields (`lure.rs`).
pub const CELL: i32 = 4;
/// Every so many ticks the loud cells are listened to (twice a second).
pub(super) const LISTEN_EVERY: u64 = 10;

/// How loud it is where: the sound grid.
#[derive(Clone)]
pub struct Sound {
    /// Cells across (and down) – the map is square.
    cells: i32,
    /// Per cell: how loud it is …
    level: Vec<u32>,
    /// … the tick something last made a sound there …
    last: Vec<u64>,
    /// … and where exactly that was – zombies are lured there.
    at: Vec<Vec2>,
    /// The cells that aren't quiet (level above 0), in order – only those are looked at.
    loud: Vec<usize>,
}

impl Sound {
    pub fn new(map_size: usize) -> Sound {
        let cells = (map_size as i32 + CELL - 1) / CELL;
        let n = (cells * cells) as usize;
        Sound { cells, level: vec![0; n], last: vec![0; n], at: vec![Vec2::ZERO; n], loud: Vec::new() }
    }

    /// The cell a point lies in – `None` off the grid.
    pub fn cell_of(&self, p: Vec2) -> Option<usize> {
        let (cx, cy) = cell_xy(p);
        (cx >= 0 && cy >= 0 && cx < self.cells && cy < self.cells).then(|| (cy * self.cells + cx) as usize)
    }

    /// `amount` more noise at `p`, at this tick.
    fn add(&mut self, tick: u64, p: Vec2, amount: u32) {
        let Some(c) = self.cell_of(p) else {
            return;
        };
        if amount == 0 {
            return;
        }
        if self.level[c] == 0 {
            // Into the list of loud cells, keeping it in order (the same on every PC).
            let k = self.loud.binary_search(&c).unwrap_or_else(|k| k);
            self.loud.insert(k, c);
        }
        self.level[c] = self.level[c].saturating_add(amount);
        self.last[c] = tick;
        self.at[c] = p;
    }

    /// Every tick: a cell quiet for `quiet` ticks halves, and again every `halve` ticks. Silent cells leave the list.
    fn fade(&mut self, tick: u64, quiet: u32, halve: u32) {
        let (quiet, halve) = (quiet as u64, halve.max(1) as u64);
        for &c in &self.loud {
            let since = tick - self.last[c];
            if since >= quiet && (since - quiet).is_multiple_of(halve) {
                self.level[c] /= 2;
            }
        }
        let level = &self.level;
        self.loud.retain(|&c| level[c] > 0);
    }

    /// The loud cells: where the last sound in each was, and how loud it is there.
    pub fn loud(&self) -> impl Iterator<Item = (Vec2, u32)> + '_ {
        self.loud.iter().map(|&c| (self.at[c], self.level[c]))
    }
}

/// The cell (x, y) of the sound grid a point lies in – also the key of a lure field.
pub fn cell_xy(p: Vec2) -> (i32, i32) {
    ((p.x.floor() as i32).div_euclid(CELL), (p.y.floor() as i32).div_euclid(CELL))
}

impl Breed {
    /// How loud a sound has to be for it to come: `threshold / alertness` (`data\sound.json`,
    /// `data\zombies.json`).
    pub fn threshold(self) -> u32 {
        data::get().sound.threshold / self.stats().alertness.max(1)
    }

    /// How far it hears: `hearing` times its sight.
    pub fn hearing_range(self) -> f32 {
        data::get().sound.hearing * self.stats().sight
    }
}

impl World {
    /// Noise at `at`, `amount` loud – it adds up with what's already there (`Sound`).
    pub(super) fn make_sound(&mut self, at: Vec2, amount: u32) {
        self.sound.add(self.tick, at, amount);
    }

    /// Noise your colony makes – a shot, a machine, a blast, a new building: like `make_sound`, and it adds to the
    /// unrest for good. (What the zombies cause – hits, the infected, the dying – doesn't.)
    pub(super) fn make_colony_sound(&mut self, at: Vec2, amount: u32) {
        self.make_sound(at, amount);
        self.unrest += amount as u64;
    }

    /// How many zombies more every group of the constant spawn brings because of the unrest.
    pub fn unrest_zombies(&self) -> u32 {
        (self.unrest / data::get().sound.unrest_per_zombie.max(1)).min(u32::MAX as u64) as u32
    }

    /// Every `work_every_seconds`: every building that works makes its `noise` – as much of it as it has power for. A
    /// burner without wood, a building switched off or infected: nothing.
    pub(super) fn machines_work(&mut self) {
        if !self.buildings.iter().any(|b| b.stats().noise > 0) {
            return;
        }
        let power = self.power();
        for b in 0..self.buildings.len() {
            let building = &self.buildings[b];
            let stats = building.stats();
            // A burner without wood and a foundry without ore stand still – quiet.
            let unfed = (stats.burns_wood > 0 && !building.fuelled) || (stats.smelts > 0 && building.smelted == 0);
            let idle = !building.works() || unfed;
            if stats.noise == 0 || idle {
                continue;
            }
            let amount = stats.noise * power.share(self, b) / 1000;
            let center = building.center();
            self.make_colony_sound(center, amount);
        }
    }

    /// Every tick: quiet places get quieter (`Sound::fade`).
    pub(super) fn fade_sound(&mut self) {
        let sound = &data::get().sound;
        self.sound.fade(self.tick, sound.quiet, sound.halve);
    }

    /// Twice a second: who hears the loud places? Zombies come (`lure_to_sound`), houses of doom let their zombies out,
    /// hidden guards come out of the ground (`doom.rs`). Cell by cell in order – the same on every PC.
    pub(super) fn listen(&mut self) {
        if !self.tick.is_multiple_of(LISTEN_EVERY) {
            return;
        }
        let quietest = Breed::ALL.iter().map(|b| b.threshold()).min().unwrap_or(u32::MAX);
        let quietest = quietest.min(data::get().doom.hear_threshold).min(data::get().loot.hear_threshold);
        let loud: Vec<(Vec2, u32)> = self.sound.loud().filter(|&(_, level)| level >= quietest).collect();
        if loud.is_empty() {
            return;
        }
        // The grid of the last tick still has the zombies killed since then – fill it anew.
        self.fill_zombie_grid();
        for (at, level) in loud {
            self.lure_to_sound(at, level);
            self.rouse_sleepers(at, level);
            self.rouse_houses(at, level);
            self.rouse_guards(at, level);
        }
    }

    /// Every zombie that hears the sound at `at` loud enough may come – by chance: just loud enough, every other time;
    /// twice as loud, always. Further away it's quieter: at the edge of its hearing half as loud. Who is storming one of
    /// your units, or is on its way to something at least as loud, stays at that; the final wave isn't lured away.
    fn lure_to_sound(&mut self, at: Vec2, level: u32) {
        let reach = Breed::ALL.iter().map(|b| b.hearing_range()).fold(0.0, f32::max);
        let r = reach.ceil() as i32;
        let (cx, cy) = (at.x.floor() as i32, at.y.floor() as i32);
        // First only reading: who hears it, how loud. Then the dice – writing.
        let mut hearers: Vec<(usize, u32)> = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                for z in self.grid.cell(cx + dx, cy + dy) {
                    let zombie = &self.zombies[z];
                    let (range, d) = (zombie.breed.hearing_range(), zombie.pos.distance(at));
                    if d > range || zombie.final_wave || !zombie.is_alive() {
                        continue;
                    }
                    let heard = level - (level as f32 * d / (2.0 * range)) as u32;
                    let busy = zombie.lure.is_some_and(|lure| lure.loud >= heard);
                    if heard >= zombie.breed.threshold() && !busy && !self.notices(zombie) {
                        hearers.push((z, heard));
                    }
                }
            }
        }
        let tick = self.tick;
        for (z, heard) in hearers {
            let chance = heard as f32 / (2 * self.zombies[z].breed.threshold().max(1)) as f32;
            if self.rng.unit_f32() < chance {
                let zombie = &mut self.zombies[z];
                zombie.lure = Some(Lure::new(at, heard, tick));
                zombie.wander = None;
            }
        }
    }

    /// The loudest place right now and how loud – for the developer overlay (F3).
    pub fn loudest_sound(&self) -> Option<(Vec2, u32)> {
        self.sound.loud().max_by_key(|&(_, level)| level)
    }

    /// Everybody comes – the final wave (DESIGN.md 8.2, like TAB): every zombie on the map heads for the command center
    /// and belongs to the final wave now, every house of doom empties, every hidden guard comes out of the ground.
    pub(super) fn call_everyone(&mut self) {
        for zombie in &mut self.zombies {
            zombie.siege = true;
            zombie.final_wave = true;
            zombie.lure = None;
            zombie.wander = None;
        }
        for house in &mut self.doom {
            house.final_call = true;
        }
        self.call_sleepers();
        for l in 0..self.loot.len() {
            self.raise_guards(l, super::doom::Bound::Final);
        }
    }
}
