//! Sound like in They Are Billions (DESIGN.md 8.3) – TAB's own rules. Everything that happens makes noise where it
//! happens: your units' shots (`noise` in `data\units.json` – ranger 1, soldier 3, sniper 10 per shot), zombies hitting
//! something, a building being placed, a unit dying, a building destroyed – and, loudest of all, a building being
//! infected. The other numbers are in `data\sound.json`.
//!
//! - The noise **adds up** where it's made, per cell of `CELL` × `CELL` tiles. Every `halve_seconds` **every** cell
//!   halves, quiet or not (TAB: every second). A fight that goes on doesn't get louder and louder: it settles at about
//!   twice what it makes in a second.
//! - **Listening:** every `listen_seconds` each zombie listens (TAB: 4 s, each one on another tick). Of the loud cells
//!   within `hearing` times its sight it takes the one it hears best – `noise × per_awareness × awareness / distance²`
//!   (distance in tiles, `chance`) – and comes with that many percent: 100 and more for sure. Twice as far, a quarter
//!   as sure. It goes to the middle of that cell (`Lure`, `Why::Sound`) and idles around it while it's loud there;
//!   quiet there at its next listening and nothing new heard, it forgets it and idles on from where it is.
//! - Who doesn't listen: whoever is after something or heads for where it lost sight of it, the waves, the final wave.
//! - Sleepers listen the same way (`sleep.rs`); the houses of doom and the hidden guards of loot every
//!   `places_listen_seconds`, with their own `awareness` (`doom.rs`).
//! - **Machines** (Factorio's pollution as TAB's noise): every `work_every_seconds` every working building makes its
//!   `noise` (`data\buildings.json`) – all at the same moment, so buildings close together add up.
//! - **Unrest:** everything your colony makes – shots, machines, blasts, building – also adds up in `World::unrest`,
//!   which never goes down. For every `unrest_per_zombie` of it, every side of a silent event (TAB's stragglers)
//!   brings one zombie more, for good (`events.rs`): growing loud has a price that tearing down the factory later
//!   doesn't undo.
//!
//! Only whole numbers – the same on every PC (DESIGN.md 12.8).

use macroquad::math::{vec2, Vec2};

use super::zombies::{Breed, Idle, Lure, Why, Zombie};
use super::World;
use crate::data;

/// Edge of a cell of the sound grid (tiles) – noise in it adds up. Also the cell of the lure fields (`lure.rs`).
pub const CELL: i32 = 4;

/// How loud it is where: the sound grid.
#[derive(Clone)]
pub struct Sound {
    /// Cells across (and down) – the map is square.
    cells: i32,
    /// Per cell: how loud it is.
    level: Vec<u32>,
    /// The cells that aren't quiet (level above 0), in order – only those are looked at.
    loud: Vec<usize>,
}

impl Sound {
    pub fn new(map_size: usize) -> Sound {
        let cells = (map_size as i32 + CELL - 1) / CELL;
        Sound { cells, level: vec![0; (cells * cells) as usize], loud: Vec::new() }
    }

    /// The cell a point lies in – `None` off the grid.
    pub fn cell_of(&self, p: Vec2) -> Option<usize> {
        let (cx, cy) = cell_xy(p);
        (cx >= 0 && cy >= 0 && cx < self.cells && cy < self.cells).then(|| (cy * self.cells + cx) as usize)
    }

    /// `amount` more noise at `p`.
    fn add(&mut self, p: Vec2, amount: u32) {
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
    }

    /// Every cell halves – quiet ones leave the list.
    fn halve(&mut self) {
        for &c in &self.loud {
            self.level[c] /= 2;
        }
        let level = &self.level;
        self.loud.retain(|&c| level[c] > 0);
    }

    /// How loud cell `c` is.
    pub(super) fn level(&self, c: usize) -> u32 {
        self.level[c]
    }

    /// The tile in the middle of cell `c` – where a lured zombie goes, and what it measures its distance to.
    fn middle_tile(&self, c: usize) -> (i32, i32) {
        let (cx, cy) = (c as i32 % self.cells, c as i32 / self.cells);
        (cx * CELL + CELL / 2, cy * CELL + CELL / 2)
    }

    /// The middle of cell `c` as a point.
    pub(super) fn middle(&self, c: usize) -> Vec2 {
        let (x, y) = self.middle_tile(c);
        vec2(x as f32, y as f32)
    }

    /// The loud cells: their middle, and how loud they are.
    pub fn loud(&self) -> impl Iterator<Item = (Vec2, u32)> + '_ {
        self.loud.iter().map(|&c| (self.middle(c), self.level[c]))
    }

    /// Is it quiet everywhere?
    pub(super) fn is_quiet(&self) -> bool {
        self.loud.is_empty()
    }

    /// What a listener at `pos` hears best: of the loud cells at most `range` tiles away on each axis (TAB looks in a
    /// square), the one with the highest `chance` – with that chance. Equal ones: the first in the list (row after row).
    pub(super) fn best(&self, pos: Vec2, range: f32, factor: u32) -> Option<(usize, u32)> {
        let (x, y) = (pos.x.floor() as i32, pos.y.floor() as i32);
        let r = range as i32;
        let mut best: Option<(usize, u32)> = None;
        for &c in &self.loud {
            let (mx, my) = self.middle_tile(c);
            let (dx, dy) = (mx - x, my - y);
            if dx.abs() > r || dy.abs() > r {
                continue;
            }
            let heard = chance(self.level[c], factor, dx * dx + dy * dy);
            if heard > best.map_or(0, |(_, b)| b) {
                best = Some((c, heard));
            }
        }
        best
    }
}

/// The cell (x, y) of the sound grid a point lies in – also the key of a lure field.
pub fn cell_xy(p: Vec2) -> (i32, i32) {
    ((p.x.floor() as i32).div_euclid(CELL), (p.y.floor() as i32).div_euclid(CELL))
}

/// TAB's rule: in percent, how sure a listener with this `factor` (`per_awareness × awareness`) is to come to a sound
/// `level` loud that lies `d2` away (distance in tiles, squared) – 100 and more: for sure.
pub(super) fn chance(level: u32, factor: u32, d2: i32) -> u32 {
    let d2 = d2.max(1) as u64;
    (level as u64 * factor as u64 / d2).min(u32::MAX as u64) as u32
}

/// `per_awareness × awareness` as a whole number – what a sound's level is multiplied with (`chance`).
pub(super) fn hear_factor(awareness: f32) -> u32 {
    (data::get().sound.per_awareness as f32 * awareness).floor().max(0.0) as u32
}

impl Breed {
    /// What a sound's level is multiplied with for this breed (`chance`): TAB's 20 × its `awareness`.
    pub fn hear_factor(self) -> u32 {
        hear_factor(self.stats().awareness)
    }

    /// How far it hears: `hearing` times its sight.
    pub fn hearing_range(self) -> f32 {
        data::get().sound.hearing * self.stats().sight
    }

    /// In percent, how sure it is to come to a sound `level` loud `tiles` away – for the developer overlay.
    pub fn hears(self, level: u32, tiles: i32) -> u32 {
        chance(level, self.hear_factor(), tiles * tiles)
    }
}

impl World {
    /// Noise at `at`, `amount` loud – it adds up with what's already there (`Sound`).
    pub(super) fn make_sound(&mut self, at: Vec2, amount: u32) {
        self.sound.add(at, amount);
    }

    /// Noise your colony makes – a shot, a machine, a blast, a new building: like `make_sound`, and it adds to the
    /// unrest for good. (What the zombies cause – hits, the infected, the dying – doesn't.)
    pub(super) fn make_colony_sound(&mut self, at: Vec2, amount: u32) {
        self.make_sound(at, amount);
        self.unrest += amount as u64;
    }

    /// How many zombies more every side of a silent event brings because of the unrest.
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

    /// Every `halve_seconds`: every cell halves (`Sound::halve`) – at the end of the first tick of a beat, so it never
    /// falls on the tick the machines work (`machines_work`, the last tick of their beat).
    pub(super) fn fade_sound(&mut self) {
        let halve = data::get().sound.halve.max(1) as u64;
        if self.tick.is_multiple_of(halve) {
            self.sound.halve();
        }
    }

    /// Every tick: whose turn it is listens – the zombies awake (`listen_zombies`), the sleepers (`listen_sleepers`),
    /// the houses of doom and the guards of loot (`listen_houses`, `listen_guards`).
    pub(super) fn listen(&mut self) {
        self.listen_zombies();
        self.listen_sleepers();
        self.listen_houses();
        self.listen_guards();
    }

    /// Does this zombie listen at all? Not while it's after something or heads for where it lost sight of it, not
    /// with a wave, not in the air (TAB: those come first in its mind, `ZXBehaviour`).
    fn listens(&self, zombie: &Zombie) -> bool {
        zombie.is_alive()
            && !zombie.siege
            && !zombie.final_wave
            && zombie.leap.is_none()
            && zombie.aim.is_none()
            && !zombie.lure.is_some_and(|lure| lure.why == Why::Seen)
    }

    /// The zombies whose turn it is (every `listen_seconds`, each on the tick its number gives it) listen: the cell they
    /// hear best, and the dice whether they come (`Sound::best`). Not coming, a zombie on its way to a sound that has
    /// gone quiet forgets it and idles on from where it is. First only reading, then the dice – in the order of the
    /// list, the same on every PC.
    fn listen_zombies(&mut self) {
        let every = data::get().sound.listen.max(1) as u64;
        let tick = self.tick;
        let heard: Vec<(usize, Option<(usize, u32)>)> = (0..self.zombies.len())
            .filter(|&i| (tick + self.zombies[i].id as u64).is_multiple_of(every) && self.listens(&self.zombies[i]))
            .map(|i| {
                let zombie = &self.zombies[i];
                (i, self.sound.best(zombie.pos, self.hearing_range(zombie.breed), self.hear_factor(zombie.breed)))
            })
            .collect();
        for (i, best) in heard {
            let comes = best.filter(|&(_, chance)| self.rng.below(100) < chance);
            let zombie = &mut self.zombies[i];
            match comes {
                Some((c, _)) => zombie.lure = Some(Lure::sound(c, self.sound.middle(c), tick)),
                None => {
                    if let Some(Why::Sound(c)) = zombie.lure.map(|lure| lure.why)
                        && self.sound.level(c) == 0
                    {
                        zombie.lure = None;
                        zombie.idle = Idle::new();
                    }
                }
            }
        }
    }

    /// The loudest place right now and how loud – for the developer overlay (F3).
    /// Every loud cell of the sound grid: its middle and how loud it is – the editor's heat map (`draw::Layers`).
    pub fn noise(&self) -> impl Iterator<Item = (Vec2, u32)> + '_ {
        self.sound.loud()
    }

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
