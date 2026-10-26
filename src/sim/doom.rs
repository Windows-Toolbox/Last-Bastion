//! The zombies' own places on the map, like in TAB (DESIGN.md 9.1, "points of interest"):
//! - **Villages of doom**: ruined houses full of sleeping zombies. One of your units coming close (`wake_radius`)
//!   lets only a few out (`near_zombies`). What really rouses a house is **noise** (`sound.rs`): while it hears a
//!   sound loud enough, its zombies pour out (`release_per_second`) and go where the sound was. Every hit lets one more
//!   out (`hit_zombies` – on top of the ones inside, as long as it isn't empty). Shot down, it lets out at once all that
//!   are still inside and drops loot. The final wave empties every house (`call_everyone`).
//! - **Loot**: piles of gold or goods lying around. A unit walking onto one picks it up. Zombies lie hidden in the
//!   ground around it – they all come out at once when one of your units comes close or a sound is loud enough there.
//!
//! The numbers are in `data\doom.json` and `data\loot.json`, the breeds of the zombies come from `mix` in
//! `data\roaming.json`.
//! Where the villages and the loot lie, the map generator decides (`mapgen/villages.rs`); a map file keeps them.

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use super::zombies::{Breed, Lure};
use super::{World, TICKS_PER_SECOND};
use crate::data;
use crate::kinds::Resource;

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
    /// Zombies still inside.
    pub inside: u32,
    /// Ticks left in which it lets its zombies out – a loud enough sound rouses it (`rouse_houses`), and it stays roused
    /// as long as the sound goes on.
    pub roused: u32,
    /// Where they go: the sound that roused it, or the last unit that shot it.
    call: Vec2,
    /// The final wave called everyone: it empties, and its zombies belong to the final wave.
    pub(super) final_call: bool,
    /// Ticks until one of your units close by lets a few out again.
    near_wait: u32,
    /// Thousandths of a zombie towards the next one out – `release_per_second` isn't a whole number per tick.
    release: u32,
}

/// Where the zombies coming out of a house or the ground go.
#[derive(Clone, Copy)]
pub(super) enum Bound {
    /// Lured there – they stay unless they find something.
    To(Lure),
    /// With the final wave, to the command center.
    Final,
}

impl DoomHouse {
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

    /// Is it letting its zombies out right now – roused by a sound, or called by the final wave?
    pub fn is_stirring(&self) -> bool {
        self.inside > 0 && (self.roused > 0 || self.final_call)
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
    /// A house of a village of doom with `zombies` inside, asleep. Its tiles are blocked from now on – recomputed after
    /// the orders of the tick (`map_changed`), or by `refresh_map` / `settle` when a map is made.
    pub fn add_doom(&mut self, tile: (i32, i32), size: (i32, i32), zombies: u32) {
        self.next_doom_id += 1;
        let hp = data::get().doom.hp;
        let call = vec2(tile.0 as f32, tile.1 as f32) + vec2(size.0 as f32, size.1 as f32) / 2.0;
        self.doom.push(DoomHouse {
            id: self.next_doom_id,
            tile,
            size,
            hp,
            inside: zombies,
            roused: 0,
            call,
            final_call: false,
            near_wait: 0,
            release: 0,
        });
        self.map.set_doom(tile.0, tile.1, size, true);
        self.map_changed = true;
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

    /// Every tick: one of your units close to a house lets a few out (again every `near_every_seconds` while it stays),
    /// a roused house – or one the final wave called – lets out `release_per_second`; hidden guards come out when a
    /// unit comes close to their loot; loot a unit stands on is picked up. All in the order of the lists – the same on
    /// every PC.
    pub(super) fn update_doom(&mut self) {
        let doom = &data::get().doom;
        let tick = self.tick;
        // Per tick, in thousandths of a zombie.
        let per_tick = (doom.release_per_second * 1000.0 / TICKS_PER_SECOND + 0.5).floor() as u32;
        for d in 0..self.doom.len() {
            let house = &self.doom[d];
            let near = self.units.iter().find(|u| house.distance_to(u.pos) <= doom.wake_radius).map(|u| u.pos);
            let house = &mut self.doom[d];
            house.near_wait = house.near_wait.saturating_sub(1);
            if let Some(unit) = near.filter(|_| house.near_wait == 0 && house.inside > 0) {
                house.near_wait = doom.near_every;
                let count = doom.near_zombies.min(house.inside);
                self.let_out(d, count, Bound::To(Lure::seen(unit, tick)), false);
            }
            let house = &mut self.doom[d];
            if (house.roused == 0 && !house.final_call) || house.inside == 0 {
                continue;
            }
            house.roused = house.roused.saturating_sub(1);
            house.release += per_tick;
            let count = (house.release / 1000).min(house.inside);
            house.release %= 1000;
            let bound = if house.final_call { Bound::Final } else { Bound::To(Lure::new(house.call, doom.hear_threshold, tick)) };
            self.let_out(d, count, bound, false);
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

    /// A sound `level` loud at `at` (`sound.rs`): every house that hears it – within `hearing` tiles, at least
    /// `hear_threshold` – is roused until the next listening, and its zombies go there.
    pub(super) fn rouse_houses(&mut self, at: Vec2, level: u32) {
        let doom = &data::get().doom;
        if level < doom.hear_threshold {
            return;
        }
        for house in self.doom.iter_mut().filter(|d| d.distance_to(at) <= doom.hearing) {
            house.roused = house.roused.max(super::sound::LISTEN_EVERY as u32 * 2);
            house.call = at;
        }
    }

    /// The same for the hidden guards of loot: within `hearing`, at least `hear_threshold` – they all come out, and go
    /// where the sound was.
    pub(super) fn rouse_guards(&mut self, at: Vec2, level: u32) {
        let loot = &data::get().loot;
        if level < loot.hear_threshold {
            return;
        }
        let tick = self.tick;
        for l in 0..self.loot.len() {
            if self.loot[l].guards > 0 && self.loot[l].pos.distance(at) <= loot.hearing {
                self.raise_guards(l, Bound::To(Lure::new(at, level, tick)));
            }
        }
    }

    /// One of your units at `from` hit house `d` for `damage`: it loses hit points, and `hit_zombies` more come out –
    /// on top of the ones inside, as long as it isn't empty – and go for the shooter. Whoever is still inside follows
    /// the shooter too once it falls (`call`).
    pub(super) fn hit_house(&mut self, d: usize, damage: u32, from: Vec2) {
        let doom = &data::get().doom;
        let house = &mut self.doom[d];
        house.hp = house.hp.saturating_sub(damage.saturating_sub(doom.armor).max(1));
        house.call = from;
        if house.inside > 0 {
            let bound = Bound::To(Lure::seen(from, self.tick));
            self.let_out(d, doom.hit_zombies, bound, true);
        }
    }

    /// `count` zombies out of house `d`, onto free tiles right around it, going where `bound` says. `extra`: on top of
    /// the ones inside (a hit) – otherwise they're taken from them. Walled in all around: they wait inside.
    fn let_out(&mut self, d: usize, count: u32, bound: Bound, extra: bool) {
        let map = &self.map;
        let ring: Vec<(i32, i32)> = self.doom[d].ring().into_iter().filter(|&(x, y)| !map.is_blocked(x, y)).collect();
        if ring.is_empty() || count == 0 {
            return;
        }
        for _ in 0..count {
            let (x, y) = ring[self.rng.below(ring.len() as u32) as usize];
            let pos = vec2(x as f32 + self.rng.unit_f32(), y as f32 + self.rng.unit_f32());
            let breed = self.roll_breed(pos);
            self.spawn_bound(pos, breed, bound);
        }
        if !extra {
            self.doom[d].inside -= count;
        }
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

    /// A zombie that comes out of a house or the ground: lured somewhere (it stays there), or with the final wave.
    fn spawn_bound(&mut self, pos: Vec2, breed: Breed, bound: Bound) {
        match bound {
            Bound::To(lure) => self.spawn_lured(pos, breed, lure),
            Bound::Final => {
                self.spawn_zombie_as(pos, breed, false);
                if let Some(zombie) = self.zombies.last_mut() {
                    zombie.final_wave = true;
                }
            }
        }
    }

    /// Destroyed houses (no hit points left), at the end of the tick: whoever is still inside comes out at once and goes
    /// for the last shooter, the house drops loot where it stood (`loot_gold` or `loot_goods`, no guards), and its tiles
    /// are free again. Returns whether one fell – then the map has to be recomputed.
    pub(super) fn remove_razed_houses(&mut self) -> bool {
        let razed: Vec<usize> = (0..self.doom.len()).filter(|&d| self.doom[d].hp == 0).collect();
        for &d in &razed {
            let bound = if self.doom[d].final_call { Bound::Final } else { Bound::To(Lure::seen(self.doom[d].call, self.tick)) };
            self.let_out(d, self.doom[d].inside, bound, false);
            let doom = &data::get().doom;
            let (kind, amount) = self.roll_haul(doom.loot_gold, doom.loot_goods);
            self.add_loot(self.doom[d].center(), kind, amount, 0);
            let house = &self.doom[d];
            self.map.set_doom(house.tile.0, house.tile.1, house.size, false);
            self.score.doom_razed += 1;
        }
        self.doom.retain(|d| d.hp > 0);
        !razed.is_empty()
    }
}
