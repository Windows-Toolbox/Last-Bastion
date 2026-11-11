//! The zombies' own places on the map, like in TAB (DESIGN.md 9.1, "points of interest"):
//! - **Villages of doom**: ruined houses, like TAB's (`CInfectionNest`). Their zombies aren't inside – they come into
//!   being in **batches** when something happens, up to `max_units` per house (times the difficulty's population). A
//!   batch is 0.5 … 1.5 × the house's size (small 1, medium 2, big 5) × its `factor`, times what happened, rounded
//!   up – at most one batch a second per house:
//!   - it **hears** a sound (every second, `sound.rs`): × 0.5, they go there;
//!   - every **12 hours** (each house at its own hour): fewer than 2 × its size of zombies around it – × 1 stays as
//!     its guards;
//!   - from **day 20 every 5 days** (each house on its own day): × 1 **marches on the command center**;
//!   - **hit**: × 3 go for the closest of your units within 10 tiles, and every house nearby sends × 1 along;
//!   - the **final wave** called: × 0.8 every second, with the final wave, until it's empty.
//!
//!   Shot down, it drops loot; whatever it hadn't let out yet is gone.
//! - **Loot**: piles of gold or goods lying around. A unit walking onto one picks it up. Zombies lie hidden in the
//!   ground around it – they all come out at once when one of your units comes close or a sound is loud enough there.
//!
//! The numbers are in `data\doom.json` and `data\loot.json`; the breeds of the loot's guards come from `mix` in
//! `data\roaming.json`.
//! Where the villages and the loot lie, the map generator decides (`mapgen/villages.rs`, `mapgen/tab.rs`); a map file
//! keeps them.

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use super::fx::{Fallen, Fx};
use super::zombies::{Breed, Lure};
use super::{after_armor, World, DAY_TICKS, HOUR_TICKS};
use crate::data;
use crate::kinds::{BuildingKind, Resource};

/// A unit picks up loot when its middle is this close to the pile's middle (tiles).
const LOOT_REACH: f32 = 0.6;
/// Hidden guards come out of the ground at most this far from their loot (tiles) …
const GUARD_SPREAD: f32 = 3.0;
/// … but not closer than this – not right on top of the pile.
const GUARD_GAP: f32 = 1.0;

/// A house of a village of doom. It blocks its tiles like a mountain (`Map::set_doom`): nobody walks through, the
/// zombies don't tear it down – only your units can, by shooting it.
#[derive(Clone)]
pub struct DoomHouse {
    pub id: u32,
    /// The top left tile, and width and depth.
    pub tile: (i32, i32),
    pub size: (i32, i32),
    pub hp: u32,
    /// How many it may still let out – they aren't inside, they come into being with each batch (TAB's
    /// `InfectionNestMaxUnits` × population, minus the ones out already).
    pub inside: u32,
    /// How big its batches are besides its size: TAB's `InfectedFactor` (the population) × the landscape's
    /// `InfectionNestsFactor`.
    pub factor: f32,
    /// The tick of its last batch – the next one comes `batch_every_seconds` later at the earliest.
    last_batch: Option<u64>,
    /// The hour it last looked for its guards, the day it last raided (`update_doom`).
    guard_hour: u64,
    raid_day: u64,
    /// The final wave called everyone: it lets everything out, with the final wave.
    pub(super) final_call: bool,
}

/// How big a house of doom is, like TAB's `ZXInfectionNestSize`: it decides how big its batches are, how many it
/// lets out in all and which breeds. By its tiles: 2 × 2 small, 3 × 3 medium, bigger big.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Nest {
    Small,
    Medium,
    Big,
}

impl Nest {
    pub const ALL: [Nest; 3] = [Nest::Small, Nest::Medium, Nest::Big];

    pub fn of(size: (i32, i32)) -> Nest {
        match size.0.max(size.1) {
            ..=2 => Nest::Small,
            3 => Nest::Medium,
            _ => Nest::Big,
        }
    }

    /// TAB's number for it: small 1, medium 2, big 5 – a batch is this big at factor 1.
    pub fn weight(self) -> u32 {
        match self {
            Nest::Small => 1,
            Nest::Medium => 2,
            Nest::Big => 5,
        }
    }

    /// Its value out of `data\doom.json`.
    pub fn pick<T>(self, values: &data::PerNest<T>) -> &T {
        match self {
            Nest::Small => &values.small,
            Nest::Medium => &values.medium,
            Nest::Big => &values.big,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Nest::Small => "Small",
            Nest::Medium => "Medium",
            Nest::Big => "Big",
        }
    }

    /// How many tiles each way the generator makes it – TAB's 2 × 2, 3 × 3 and 5 × 5 (`mapgen/tab.rs`). The editor
    /// places them that big.
    pub fn side(self) -> i32 {
        match self {
            Nest::Small => 2,
            Nest::Medium => 3,
            Nest::Big => 5,
        }
    }
}

/// Where the zombies coming out of a house or the ground go.
#[derive(Clone, Copy)]
pub(super) enum Bound {
    /// Lured there – they stay unless they find something.
    To(Lure),
    /// Nowhere: they idle where they came out (a house's guards).
    Stay,
    /// A raid: to the command center, like a wave.
    Raid,
    /// With the final wave, to the command center.
    Final,
}

impl DoomHouse {
    /// How big it is (`Nest::of` its tiles).
    pub fn nest(&self) -> Nest {
        Nest::of(self.size)
    }

    /// Its hit points when whole – by its size, like TAB's three houses (`hp` in `data\doom.json`).
    pub fn full_hp(&self) -> u32 {
        *self.nest().pick(&data::get().doom.hp)
    }

    /// The top left corner in world coordinates.
    pub fn corner(&self) -> Vec2 {
        vec2(self.tile.0 as f32, self.tile.1 as f32)
    }

    /// The bottom right corner.
    pub fn far_corner(&self) -> Vec2 {
        self.corner() + vec2(self.size.0 as f32, self.size.1 as f32)
    }

    pub fn center(&self) -> Vec2 {
        (self.corner() + self.far_corner()) / 2.0
    }

    /// How far a point is from its walls (0 on it) – like `Building::distance_to`.
    pub fn distance_to(&self, p: Vec2) -> f32 {
        (self.corner() - p).max(p - self.far_corner()).max(Vec2::ZERO).length()
    }

    /// Is it letting its zombies out right now – a batch in the last two seconds, or called by the final wave?
    pub fn is_stirring(&self, tick: u64) -> bool {
        self.inside > 0 && (self.final_call || self.last_batch.is_some_and(|t| tick < t + 2 * super::TICKS_PER_SECOND as u64))
    }

    /// The spot of its walls closest to `p` – where a shot at it lands.
    pub fn closest_point(&self, p: Vec2) -> Vec2 {
        p.clamp(self.corner(), self.far_corner())
    }

    /// The tiles right around it – where its zombies come out. Off the map they are left out by the caller.
    fn ring(&self) -> Vec<(i32, i32)> {
        let ((x0, y0), (w, h)) = (self.tile, self.size);
        let mut ring = Vec::new();
        for y in y0 - 1..=y0 + h {
            for x in x0 - 1..=x0 + w {
                if x == x0 - 1 || x == x0 + w || y == y0 - 1 || y == y0 + h {
                    ring.push((x, y));
                }
            }
        }
        ring
    }

    /// Its tiles as (left, top, right, bottom), the right and bottom ones not included.
    fn rect(&self) -> (i32, i32, i32, i32) {
        (self.tile.0, self.tile.1, self.tile.0 + self.size.0, self.tile.1 + self.size.1)
    }
}

/// Do two rectangles of tiles (`DoomHouse::rect`) overlap?
fn overlap(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

/// A rectangle of tiles grown by `by` tiles on every side.
fn grown(r: (i32, i32, i32, i32), by: i32) -> (i32, i32, i32, i32) {
    (r.0 - by, r.1 - by, r.2 + by, r.3 + by)
}

/// What lies in a pile of loot: gold, or one of the goods of the stock.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum LootKind {
    Gold,
    Wood,
    Stone,
    Iron,
    Coal,
}

impl LootKind {
    /// The goods – gold is apart (`gold_share` in `data\loot.json`).
    pub const GOODS: [LootKind; 4] = [LootKind::Wood, LootKind::Stone, LootKind::Iron, LootKind::Coal];

    /// Where it goes in the stock – `None` for gold (`World::gold`).
    pub fn resource(self) -> Option<Resource> {
        match self {
            LootKind::Gold => None,
            LootKind::Wood => Some(Resource::Wood),
            LootKind::Stone => Some(Resource::Stone),
            LootKind::Iron => Some(Resource::IronOre),
            LootKind::Coal => Some(Resource::Coal),
        }
    }

    pub fn name(self) -> &'static str {
        match self.resource() {
            Some(r) => r.name(),
            None => "Gold",
        }
    }
}

/// A pile of loot on the ground – everyone walks over it.
#[derive(Clone)]
pub struct Loot {
    pub id: u32,
    pub pos: Vec2,
    pub kind: LootKind,
    pub amount: u32,
    /// Zombies hidden around it. They come out all at once when a unit comes close – then 0.
    pub guards: u32,
}

impl World {
    /// A house of a village of doom that may let out `zombies` in all, in batches as big as `factor` says (`DoomHouse`).
    /// Its tiles are blocked from now on – recomputed after the orders of the tick (`map_changed`), or by `refresh_map`
    /// / `settle` when a map is made.
    pub fn add_doom(&mut self, tile: (i32, i32), size: (i32, i32), zombies: u32, factor: f32) {
        self.next_doom_id += 1;
        let hp = *Nest::of(size).pick(&data::get().doom.hp);
        self.doom.push(DoomHouse {
            id: self.next_doom_id,
            tile,
            size,
            hp,
            inside: zombies,
            factor,
            last_batch: None,
            guard_hour: 0,
            raid_day: 0,
            final_call: false,
        });
        self.map.set_doom(tile.0, tile.1, size, true);
        self.map_changed = true;
    }

    /// How many a new house of `size` lets out in all at this zombie population (`max_units` × population, TAB's
    /// `InfectionNestMaxUnits × InfectedFactor`).
    pub fn doom_zombies(size: (i32, i32), population: f32) -> u32 {
        let max = *Nest::of(size).pick(&data::get().doom.max_units);
        (max as f32 * population + 0.5).floor() as u32
    }

    /// A pile of loot, with `guards` zombies hidden around it.
    pub fn add_loot(&mut self, pos: Vec2, kind: LootKind, amount: u32, guards: u32) {
        self.next_loot_id += 1;
        self.loot.push(Loot { id: self.next_loot_id, pos, kind, amount, guards });
    }

    /// What lies in a pile at `pos`, as the generator and the editor put it there: gold or goods (`gold_share`), and how
    /// many guards are hidden around it – from `guards[0]` near the base to `guards[1]` at the edge (`loot_guards` of
    /// the difficulty).
    pub(super) fn roll_loot(&mut self, pos: Vec2, guards: [u32; 2]) -> (LootKind, u32, u32) {
        let values = &data::get().loot;
        let (kind, amount) = self.roll_haul(values.gold, values.goods);
        let guards = self.grows_outwards(pos, guards[0], guards[1]);
        (kind, amount, guards)
    }

    /// Gold (in `gold_share` of the cases) or one of the goods, each as likely.
    fn roll_haul(&mut self, gold: u32, goods: u32) -> (LootKind, u32) {
        if self.rng.unit_f32() < data::get().loot.gold_share {
            (LootKind::Gold, gold)
        } else {
            (LootKind::GOODS[self.rng.below(LootKind::GOODS.len() as u32) as usize], goods)
        }
    }

    /// A number from `near` (at the base) to `far` (at the edge) by how far out `pos` lies, ±20 % at random.
    fn grows_outwards(&mut self, pos: Vec2, near: u32, far: u32) -> u32 {
        let out = self.way_out(pos);
        let mean = near as f32 + (far as f32 - near as f32) * out;
        let random = 0.8 + 0.4 * self.rng.unit_f32();
        (mean * random + 0.5).floor().max(0.0) as u32
    }

    /// May a house of doom stand here (top left tile, width and depth)? Inside the world border, on open ground that
    /// nothing else takes – the editor's houses (`Order::PlaceDoom`, paste) and the one on the mouse.
    pub fn doom_fits(&self, tile: (i32, i32), size: (i32, i32)) -> bool {
        (tile.1..tile.1 + size.1).all(|y| (tile.0..tile.0 + size.0).all(|x| self.map.in_play(x, y) && !self.map.is_solid(x, y) && !self.map.is_built(x, y)))
    }

    /// Where is the house with this `id` in the list? `None` if it's destroyed.
    pub fn find_doom(&self, id: u32) -> Option<usize> {
        self.doom.iter().position(|d| d.id == id)
    }

    /// Where is the pile with this `id` in the list? `None` once it's picked up.
    pub fn find_loot(&self, id: u32) -> Option<usize> {
        self.loot.iter().position(|l| l.id == id)
    }

    /// The house whose tiles this point lies on.
    pub fn doom_at_point(&self, p: Vec2) -> Option<usize> {
        self.doom.iter().position(|d| d.distance_to(p) == 0.0)
    }

    /// Every tick, like TAB's houses (`CInfectionNest`, its own clock): each house looks for its guards every
    /// `guard_every_hours`, raids from day `raids_from_day` on, and lets out its batches once the final wave called.
    /// Hidden guards come out when a unit comes close to their loot; loot a unit stands on is picked up. All in the
    /// order of the lists – the same on every PC.
    pub(super) fn update_doom(&mut self) {
        let doom = &data::get().doom;
        let tick = self.tick;
        // TAB counts whole hours and days since the start (`ColonyHours`, `ColonyDays`) – day 20 is after 20 days.
        let (hours, day) = (tick / HOUR_TICKS, tick / DAY_TICKS);
        let has_command_center = self.buildings.iter().any(|b| b.kind == BuildingKind::CommandCenter);
        for d in 0..self.doom.len() {
            if self.doom[d].inside == 0 {
                continue;
            }
            // Called by the final wave: from now on only that – everything goes to the command center.
            if self.doom[d].final_call {
                let count = self.batch(d, doom.final_batch);
                self.let_out(d, count, Bound::Final);
                continue;
            }
            // Each house at its own hour, on its own day: its number shifts them.
            let id = self.doom[d].id as u64;
            if hours > self.doom[d].guard_hour && (hours + id).is_multiple_of(doom.guard_every_hours.max(1)) {
                self.doom[d].guard_hour = hours;
                if self.around_house(d) < (doom.guards_per_size * self.doom[d].nest().weight()) as usize {
                    let count = self.batch(d, doom.guard_batch);
                    self.let_out(d, count, Bound::Stay);
                }
            }
            if has_command_center && day >= doom.raids_from_day && day > self.doom[d].raid_day && (day + id).is_multiple_of(doom.raid_every_days.max(1)) {
                self.doom[d].raid_day = day;
                let count = self.batch(d, doom.raid_batch);
                self.let_out(d, count, Bound::Raid);
            }
        }

        let wake = data::get().loot.wake_radius;
        for l in 0..self.loot.len() {
            let pos = self.loot[l].pos;
            if self.loot[l].guards == 0 {
                continue;
            }
            if let Some(unit) = self.units.iter().find(|u| u.pos.distance(pos) <= wake).map(|u| u.pos) {
                self.raise_guards(l, Bound::To(Lure::seen(unit, tick)));
            }
        }

        // Picked up by whoever stands on it – a unit on the ground, not one up in a tower.
        let units = &self.units;
        let picked: Vec<usize> = (0..self.loot.len())
            .filter(|&l| units.iter().any(|u| !u.is_inside() && u.pos.distance(self.loot[l].pos) <= LOOT_REACH))
            .collect();
        // From the back, so the indices in front stay right.
        for &l in picked.iter().rev() {
            let at = self.loot[l].pos;
            self.raise_guards(l, Bound::To(Lure::seen(at, tick)));
            let loot = self.loot.remove(l);
            match loot.kind.resource() {
                Some(r) => self.stock.add(r, loot.amount, self.storage(r)),
                None => self.earn(loot.amount),
            }
            self.score.loot_found += 1;
        }
    }

    /// Every `places_listen_seconds` each house that may still let some out listens – each on the tick its number
    /// gives it – like a zombie with its `awareness` (TAB's nests, `sound.rs`): heard, a batch × `heard_batch` goes to
    /// the middle of the cell it heard, however quiet it is there by then.
    pub(super) fn listen_houses(&mut self) {
        if self.sound.is_quiet() {
            return;
        }
        let doom = &data::get().doom;
        let every = data::get().sound.places_listen.max(1) as u64;
        let factor = super::sound::hear_factor(doom.awareness);
        let tick = self.tick;
        for d in 0..self.doom.len() {
            let house = &self.doom[d];
            if !(tick + house.id as u64).is_multiple_of(every) || house.inside == 0 || house.final_call {
                continue;
            }
            let best = self.sound.best(house.center(), *house.nest().pick(&doom.hearing), factor);
            if let Some((c, _)) = best.filter(|&(_, chance)| self.rng.below(100) < chance) {
                let to = self.sound.middle(c);
                let count = self.batch(d, doom.heard_batch);
                self.let_out(d, count, Bound::To(Lure::sent(to, tick)));
            }
        }
    }

    /// The same for the hidden guards of loot, with their `awareness`: heard, they all come out of the ground and go to
    /// the middle of the cell they heard.
    pub(super) fn listen_guards(&mut self) {
        if self.sound.is_quiet() {
            return;
        }
        let loot = &data::get().loot;
        let every = data::get().sound.places_listen.max(1) as u64;
        let factor = super::sound::hear_factor(loot.awareness);
        let tick = self.tick;
        for l in 0..self.loot.len() {
            let pile = &self.loot[l];
            if !(tick + pile.id as u64).is_multiple_of(every) || pile.guards == 0 {
                continue;
            }
            let best = self.sound.best(pile.pos, loot.hearing, factor);
            if let Some((c, _)) = best.filter(|&(_, chance)| self.rng.below(100) < chance) {
                let to = self.sound.middle(c);
                self.raise_guards(l, Bound::To(Lure::sent(to, tick)));
            }
        }
    }

    /// House `d` was hit for `damage` – by one of your units or a barrel going off. Like TAB's (`OnMessage`): it loses
    /// hit points, and a batch × `hit_batch` goes for the closest of your units within `hit_reach` tiles of its middle
    /// (none there: they stay at the house). Every other house whose tiles touch the area around it (3 times its size,
    /// 4 tiles more on every side) sends a batch × `neighbour_batch` to the same spot. Only one batch a second: a
    /// house shot again and again gives a batch every second.
    pub(super) fn hit_house(&mut self, d: usize, damage: u32) {
        let doom = &data::get().doom;
        let house = &mut self.doom[d];
        house.hp = house.hp.saturating_sub(after_armor(damage, doom.armor));
        let count = self.batch(d, doom.hit_batch);
        if count == 0 {
            return;
        }
        let center = self.doom[d].center();
        let closest = self
            .units
            .iter()
            .map(|u| u.pos)
            .filter(|p| p.distance(center) <= doom.hit_reach)
            .min_by(|a, b| a.distance(center).total_cmp(&b.distance(center)));
        let bound = closest.map_or(Bound::Stay, |p| Bound::To(Lure::sent(p, self.tick)));
        self.let_out(d, count, bound);
        // The area around it: its tiles 3 times as big around its middle (TAB's `Scale(…, 3, true)`), then 4 more.
        let (l, t, r, b) = self.doom[d].rect();
        let (w, h) = (r - l, b - t);
        let area = grown((l - w, t - h, r + w, b + h), 4);
        for o in 0..self.doom.len() {
            if o != d && overlap(area, self.doom[o].rect()) {
                let count = self.batch(o, doom.neighbour_batch);
                self.let_out(o, count, bound);
            }
        }
    }

    /// TAB's batch (`CInfectionNest`): rounded up 0.5 … 1.5 × the house's size (`Nest::weight`) × its `factor` × `k`
    /// – at most as many as it may still let out. None if it let out a batch less than `batch_every_seconds` ago;
    /// otherwise the clock starts anew, even for an empty batch.
    fn batch(&mut self, d: usize, k: f32) -> u32 {
        let every = data::get().doom.batch_every as u64;
        let house = &self.doom[d];
        if house.inside == 0 || house.last_batch.is_some_and(|t| self.tick < t + every) {
            return 0;
        }
        let roll = 0.5 + self.rng.unit_f32();
        let house = &mut self.doom[d];
        house.last_batch = Some(self.tick);
        let count = (roll * house.nest().weight() as f32 * house.factor * k).ceil() as u32;
        count.min(house.inside)
    }

    /// How many zombies stand around house `d` – on its tiles grown by its width on every side, asleep or awake. The
    /// houses there count too, itself as well (TAB counts every body of the zombies' side).
    fn around_house(&self, d: usize) -> usize {
        let house = &self.doom[d];
        let (l, t, r, b) = grown(house.rect(), house.size.0);
        let awake: usize = (t..b).flat_map(|y| (l..r).map(move |x| (x, y))).map(|(x, y)| self.grid.cell(x, y).count()).sum();
        let (min, max) = (vec2(l as f32, t as f32), vec2(r as f32, b as f32));
        let asleep = self.sleep.count_in(min, max);
        let houses = self.doom.iter().filter(|o| overlap((l, t, r, b), o.rect())).count();
        awake + asleep + houses
    }

    /// `count` zombies out of house `d`, onto free tiles right around it, going where `bound` says – the breeds of its
    /// size (`breeds` in `data\doom.json`). Walled in all around: none come out, it keeps them.
    fn let_out(&mut self, d: usize, count: u32, bound: Bound) {
        let map = &self.map;
        let ring: Vec<(i32, i32)> = self.doom[d].ring().into_iter().filter(|&(x, y)| !map.is_blocked(x, y)).collect();
        if ring.is_empty() || count == 0 {
            return;
        }
        let breeds = self.doom[d].nest().pick(&data::get().doom.breeds);
        for _ in 0..count {
            let (x, y) = ring[self.rng.below(ring.len() as u32) as usize];
            let pos = vec2(x as f32 + self.rng.unit_f32(), y as f32 + self.rng.unit_f32());
            let breed = self.roll_weighted(breeds);
            self.spawn_bound(pos, breed, bound);
        }
        self.doom[d].inside -= count;
    }

    /// One breed out of `breeds`, each as likely as its weight says. An empty list: a walker.
    fn roll_weighted(&mut self, breeds: &[data::Weighted]) -> Breed {
        let total: u32 = breeds.iter().map(|b| b.weight).sum();
        if total == 0 {
            return Breed::Walker;
        }
        let mut roll = self.rng.below(total);
        for b in breeds {
            if roll < b.weight {
                return b.breed;
            }
            roll -= b.weight;
        }
        Breed::Walker
    }

    /// All guards of loot `l` come out of the ground around it – on free spots `GUARD_GAP` to `GUARD_SPREAD` tiles away
    /// – going where `bound` says.
    pub(super) fn raise_guards(&mut self, l: usize, bound: Bound) {
        let (center, count) = (self.loot[l].pos, std::mem::take(&mut self.loot[l].guards));
        for _ in 0..count {
            // A few tries for a free spot; none: this one stays in the ground.
            for _ in 0..10 {
                let angle = self.rng.unit_f32() * std::f32::consts::TAU;
                let r = GUARD_GAP + (GUARD_SPREAD - GUARD_GAP) * self.rng.unit_f32();
                let pos = center + vec2(libm::cosf(angle), libm::sinf(angle)) * r;
                if !self.map.is_blocked(pos.x.floor() as i32, pos.y.floor() as i32) {
                    let breed = self.roll_breed(pos);
                    self.spawn_bound(pos, breed, bound);
                    break;
                }
            }
        }
    }

    /// A zombie that comes out of a house or the ground: lured somewhere (it stays there), idling where it is, on a
    /// raid, or with the final wave.
    fn spawn_bound(&mut self, pos: Vec2, breed: Breed, bound: Bound) {
        match bound {
            Bound::To(lure) => self.spawn_lured(pos, breed, lure),
            Bound::Stay => self.spawn_zombie_as(pos, breed, true),
            Bound::Raid => self.spawn_zombie_as(pos, breed, false),
            Bound::Final => {
                self.spawn_zombie_as(pos, breed, false);
                if let Some(zombie) = self.zombies.last_mut() {
                    zombie.final_wave = true;
                }
            }
        }
    }

    /// Destroyed houses (no hit points left), at the end of the tick: the house drops loot where it stood (`loot_gold`
    /// or `loot_goods`, no guards, like TAB's), and its tiles are free again. Whatever it hadn't let out yet is gone –
    /// TAB's lets nothing out when it falls. Returns whether one fell – then the map has to be recomputed.
    pub(super) fn remove_razed_houses(&mut self) -> bool {
        let razed: Vec<usize> = (0..self.doom.len()).filter(|&d| self.doom[d].hp == 0).collect();
        for &d in &razed {
            let doom = &data::get().doom;
            let (kind, amount) = self.roll_haul(doom.loot_gold, doom.loot_goods);
            self.add_loot(self.doom[d].center(), kind, amount, 0);
            let house = &self.doom[d];
            self.map.set_doom(house.tile.0, house.tile.1, house.size, false);
            let fell = Fx::Fell { what: Fallen::Doom(house.nest()), tile: house.tile, size: house.size };
            self.fx.push(fell);
            self.score.doom_razed += 1;
        }
        self.doom.retain(|d| d.hp > 0);
        !razed.is_empty()
    }
}
