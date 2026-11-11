//! Ravens and rats – TAB's animals (`Raven`, `Rat` in its `Entities`, their behaviour `BHRaven` / `BHRat` and the
//! raven's flight `CBird`). Only to be looked at: they live in the picture, not in the simulation – they hurt nobody,
//! nobody can select them, and the game computes the same with or without them (co-op, DESIGN.md 12.8). Their little
//! randomness is the picture's own.
//!
//! - **Ravens:** TAB's generator puts `NumRavens` of them (per landscape) on free tiles near the command center. On the
//!   ground they stand, peck (TAB's "special"), hop a bit. When a zombie or one of your units comes within 2 tiles –
//!   or now and then just so – they fly up, off along a curve of 5 legs of 8 to 12 tiles, and land again.
//! - **Rats:** TAB only has them in its campaign. Ours scurry round the treasure ruins and the houses of doom: they
//!   stand, sniff, run a little – and dash 2 tiles away when anyone comes within a tile.

use std::f32::consts::{PI, TAU};

use macroquad::prelude::*;

use crate::sim::{OrnamentKind, World};

/// TAB's `NumRavens` per landscape (`MapThemes`) – for its 256 map, `TAB_AREA` tiles inside the border.
fn ravens_of(look: &str) -> f32 {
    match look {
        "FA" => 8.0,
        "TM" => 24.0,
        "AL" => 0.0,
        "DS" => 40.0,
        "VO" => 12.0,
        _ => 12.0,
    }
}
const TAB_AREA: f32 = 31_800.0;

/// TAB's values (`Entities`): how far they notice someone (`WatchRange`), how often they look (`EntityWatchInterval`:
/// 30 frames, every 2nd counts – half a second; the rat's 20: a third), walking and running speed (tiles per second).
const RAVEN_WATCH: f32 = 2.0;
const RAT_WATCH: f32 = 1.0;
const RAVEN_LOOKS: f32 = 0.5;
const RAT_LOOKS: f32 = 1.0 / 3.0;
const WALK: f32 = 0.75;
const RAVEN_FLIES: f32 = 6.0;
const RAT_RUNS: f32 = 2.0;
/// How long their animations last (TAB's `TimeAni…`, seconds): standing, the special one, taking off.
const RAVEN_STAND: f32 = 3.0;
const RAVEN_PECK: f32 = 2.0;
const RAT_STAND: f32 = 2.0;
const RAT_SNIFF: f32 = 3.0;
const TAKE_OFF: f32 = 0.5;
/// A flying raven climbs and sinks this fast (TAB: 0.06 per update, 30 a second) up to this height – in TAB's
/// heights; a tile width is 2 of them (`LIFT`).
const CLIMB: f32 = 1.8;
const FLIGHT_HEIGHT: f32 = 5.0;
const LIFT: f32 = 0.5;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Raven,
    Rat,
}

/// What one is doing – and since or until when (seconds of game time).
#[derive(Clone)]
enum Doing {
    Stand { until: f32 },
    /// The raven pecks, the rat sniffs (TAB's "special").
    Special { since: f32, until: f32 },
    Walk { to: Vec2 },
    /// The rat dashing off.
    Flee { to: Vec2 },
    /// The raven in the air, along `path` – `since`: when it took off.
    Fly { since: f32, path: Vec<Vec2>, next: usize },
    Land { since: f32 },
}

struct Critter {
    kind: Kind,
    pos: Vec2,
    facing: f32,
    /// How high a raven flies (TAB's heights).
    height: f32,
    doing: Doing,
    /// When it next looks round for someone coming.
    look_at: f32,
}

/// One animal as the picture draws it.
pub struct CritterLook {
    pub raven: bool,
    /// Where it is on the ground, and where it's drawn – a flying raven higher up.
    pub pos: Vec2,
    pub at: Vec2,
    pub facing: f32,
    pub pose: crate::sprites::Pose,
}

/// All animals of the map.
#[derive(Default)]
pub struct Critters {
    list: Vec<Critter>,
    /// The map they were made for (its seed and size) – a new map, new animals.
    made_for: Option<(u64, usize)>,
    last: f32,
    seed: u32,
}

impl Critters {
    /// Once per frame, `now` in seconds of game time – paused, they stand still too.
    pub fn update(&mut self, world: &World, now: f32) {
        if self.made_for != Some((world.seed, world.map.size)) {
            self.spawn(world);
            self.last = now;
        }
        let dt = (now - self.last).clamp(0.0, 0.25);
        self.last = now;
        if dt <= 0.0 {
            return;
        }
        for i in 0..self.list.len() {
            self.step(world, i, now, dt);
        }
    }

    /// The animals of a new map: ravens near the command center, rats round the ruins and every third house of doom.
    fn spawn(&mut self, world: &World) {
        self.made_for = Some((world.seed, world.map.size));
        self.seed = (world.seed as u32) ^ 0x5EED_CAFE;
        self.list.clear();
        let n = world.map.size as i32;
        let area = world.map.play_area().area();
        let ravens = (ravens_of(&world.look) * area / TAB_AREA).round() as usize;
        let (bx, by) = (world.base.x as i32, world.base.y as i32);
        for _ in 0..ravens {
            for _ in 0..200 {
                let (x, y) = (self.below(n), self.below(n));
                if (x - bx).abs().max((y - by).abs()) <= n / 4 && world.map.in_play(x, y) && !world.map.is_blocked(x, y) {
                    let pos = vec2(x as f32 + self.random(), y as f32 + self.random());
                    self.add(Kind::Raven, pos);
                    break;
                }
            }
        }
        let mut spots: Vec<Vec2> = Vec::new();
        for ruin in world.ornaments.iter().filter(|o| o.kind == OrnamentKind::Ruin) {
            let corner = vec2(ruin.tile.0 as f32, ruin.tile.1 as f32);
            spots.push(corner + vec2(1.0, 2.5));
            spots.push(corner + vec2(3.0, 1.5));
        }
        spots.extend(world.doom.iter().step_by(3).map(|d| d.center() + vec2(d.size.0 as f32 / 2.0 + 0.6, 0.3)));
        for spot in spots {
            let (x, y) = (spot.x.floor() as i32, spot.y.floor() as i32);
            if world.map.in_play(x, y) && !world.map.is_blocked(x, y) {
                self.add(Kind::Rat, spot);
            }
        }
    }

    fn add(&mut self, kind: Kind, pos: Vec2) {
        let facing = self.random() * TAU;
        let until = self.random() * 3.0;
        let look_at = self.random();
        self.list.push(Critter { kind, pos, facing, height: 0.0, doing: Doing::Stand { until }, look_at });
    }

    /// One animal, one frame.
    fn step(&mut self, world: &World, i: usize, now: f32, dt: f32) {
        let critter = &self.list[i];
        let (kind, pos) = (critter.kind, critter.pos);
        let on_ground = !matches!(critter.doing, Doing::Fly { .. } | Doing::Land { .. } | Doing::Flee { .. });
        // Looking round – TAB's `WatchForEnemies`: someone close, and the raven flies up, the rat dashes off.
        if on_ground && now >= critter.look_at {
            let (watch, every) = if kind == Kind::Raven { (RAVEN_WATCH, RAVEN_LOOKS) } else { (RAT_WATCH, RAT_LOOKS) };
            self.list[i].look_at = now + every;
            if world.anyone_near(pos, watch) {
                self.list[i].doing = match kind {
                    Kind::Raven => self.take_off(world, pos, now),
                    Kind::Rat => Doing::Flee { to: self.free_spot(world, pos, 2.0, 2.0) },
                };
            }
        }
        let doing = self.list[i].doing.clone();
        let next = match doing {
            Doing::Stand { until } | Doing::Special { until, .. } if now >= until => Some(self.ground_activity(world, kind, pos, now)),
            Doing::Walk { to } => self.walk(i, to, WALK, dt).then(|| match kind {
                // TAB: a raven pecks after hopping.
                Kind::Raven => Doing::Special { since: now, until: now + RAVEN_PECK },
                Kind::Rat => Doing::Stand { until: now + RAT_STAND },
            }),
            Doing::Flee { to } => self.walk(i, to, RAT_RUNS, dt).then_some(Doing::Stand { until: now + RAT_STAND }),
            Doing::Fly { since, path, next } => self.fly(i, since, path, next, now, dt),
            Doing::Land { since } if now - since >= TAKE_OFF => Some(Doing::Stand { until: now + RAVEN_STAND }),
            _ => None,
        };
        let critter = &mut self.list[i];
        // Out of the air it sinks back to the ground.
        if !matches!(critter.doing, Doing::Fly { .. }) {
            critter.height = (critter.height - CLIMB * dt).max(0.0);
        }
        if let Some(next) = next {
            critter.doing = next;
        }
    }

    /// What it does next on the ground – TAB's weights: stand 100, the special one 100, a few steps 100, and the raven
    /// flies off now and then (3).
    fn ground_activity(&mut self, world: &World, kind: Kind, pos: Vec2, now: f32) -> Doing {
        let roll = self.random() * if kind == Kind::Raven { 303.0 } else { 300.0 };
        let (stand, special) = if kind == Kind::Raven { (RAVEN_STAND, RAVEN_PECK) } else { (RAT_STAND, RAT_SNIFF) };
        match roll {
            r if r < 100.0 => Doing::Stand { until: now + stand },
            r if r < 200.0 => Doing::Special { since: now, until: now + special },
            r if r < 300.0 => {
                // TAB: 0.3 to 1 tile (the raven), 0.3 to 1.3 (the rat's roaming).
                let far = if kind == Kind::Raven { 1.0 } else { 1.3 };
                Doing::Walk { to: self.free_spot(world, pos, 0.3, far) }
            }
            _ => self.take_off(world, pos, now),
        }
    }

    /// Goes towards `to` at `speed` tiles per second. Is it there?
    fn walk(&mut self, i: usize, to: Vec2, speed: f32, dt: f32) -> bool {
        let critter = &mut self.list[i];
        let way = to - critter.pos;
        let step = speed * dt;
        if way.length() <= step {
            critter.pos = to;
            return true;
        }
        critter.facing = way.y.atan2(way.x);
        critter.pos += way.normalize() * step;
        false
    }

    /// The raven up in the air: along its path, climbing to its height – and down again towards the end (TAB: the
    /// last 10 points of its path). At the end it lands.
    fn fly(&mut self, i: usize, since: f32, path: Vec<Vec2>, mut next: usize, now: f32, dt: f32) -> Option<Doing> {
        let critter = &mut self.list[i];
        let mut step = RAVEN_FLIES * dt;
        while next < path.len() && step > 0.0 {
            let way = path[next] - critter.pos;
            if way.length() <= step {
                step -= way.length();
                critter.pos = path[next];
                next += 1;
            } else {
                critter.facing = way.y.atan2(way.x);
                critter.pos += way.normalize() * step;
                step = 0.0;
            }
        }
        let goal = if next + 10 >= path.len() { 0.0 } else { FLIGHT_HEIGHT };
        critter.height = if critter.height < goal { (critter.height + CLIMB * dt).min(goal) } else { (critter.height - CLIMB * dt).max(goal) };
        if next >= path.len() {
            return Some(Doing::Land { since: now });
        }
        Some(Doing::Fly { since, path, next })
    }

    /// Off into the air (TAB's `CBird.CreateNewFlightPath`): 5 legs of 8 to 12 tiles, each turned a little from the
    /// one before, inside the playing field – the last two onto free ground. The curve through them, 26 points.
    fn take_off(&mut self, world: &World, from: Vec2, now: f32) -> Doing {
        let mut points = vec![from];
        let mut heading = (self.random() * 2.0 - 1.0) * 2.0 * PI;
        let mut at = from;
        for leg in 0..5 {
            let mut next = world.map.play_area().center();
            for _ in 0..50 {
                let length = 8.0 + 4.0 * self.random();
                let turn = heading + 0.3 * (self.random() * 2.0 - 1.0) * 2.0 * PI;
                let p = at + Vec2::from_angle(turn) * length;
                let (x, y) = (p.x.floor() as i32, p.y.floor() as i32);
                if world.map.in_play(x, y) && (leg < 3 || !world.map.is_blocked(x, y)) {
                    heading = turn;
                    next = p;
                    break;
                }
            }
            points.push(next);
            at = next;
        }
        let path = (1..=25).map(|k| curve(&points, k as f32 / 25.0)).collect();
        Doing::Fly { since: now, path, next: 0 }
    }

    /// A free spot `near` to `far` tiles away – or where it is, if there is none.
    fn free_spot(&mut self, world: &World, from: Vec2, near: f32, far: f32) -> Vec2 {
        for _ in 0..8 {
            let p = from + Vec2::from_angle(self.random() * TAU) * (near + (far - near) * self.random());
            if world.map.in_play(p.x.floor() as i32, p.y.floor() as i32) && !world.map.is_blocked(p.x.floor() as i32, p.y.floor() as i32) {
                return p;
            }
        }
        from
    }

    /// Every animal as the picture draws it.
    pub fn looks(&self, now: f32) -> impl Iterator<Item = CritterLook> + '_ {
        use crate::sprites::Pose;
        self.list.iter().map(move |c| {
            let pose = match &c.doing {
                Doing::Stand { .. } => Pose::Idle,
                Doing::Special { since, .. } => Pose::Special(now - since),
                Doing::Walk { .. } => Pose::Walk(WALK),
                Doing::Flee { .. } => Pose::Walk(RAT_RUNS),
                Doing::Fly { since, .. } if now - since < TAKE_OFF => Pose::TakeOff(now - since),
                Doing::Fly { .. } => Pose::Fly,
                Doing::Land { since } => Pose::Land(now - since),
            };
            // Up on screen is towards the top left of the map – like a harpy's leap (`draw.rs`).
            let at = c.pos - Vec2::splat(2.0 * c.height * LIFT);
            CritterLook { raven: c.kind == Kind::Raven, pos: c.pos, at, facing: c.facing, pose }
        })
    }

    /// A number 0..1 (xorshift) – the picture's own.
    fn random(&mut self) -> f32 {
        let mut x = self.seed.wrapping_add(0x9E37_79B9);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / (1 << 24) as f32
    }

    fn below(&mut self, n: i32) -> i32 {
        ((self.random() * n as f32) as i32).min(n - 1)
    }
}

/// A point `t` (0..1) along a smooth curve through `points` (Catmull-Rom: it goes through every point).
fn curve(points: &[Vec2], t: f32) -> Vec2 {
    let legs = points.len() - 1;
    let x = (t * legs as f32).min(legs as f32 - 0.0001);
    let k = x as usize;
    let u = x - k as f32;
    let p = |i: isize| points[i.clamp(0, legs as isize) as usize];
    let (p0, p1, p2, p3) = (p(k as isize - 1), p(k as isize), p(k as isize + 1), p(k as isize + 2));
    0.5 * ((2.0 * p1) + (p2 - p0) * u + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u * u + (3.0 * p1 - p0 - 3.0 * p2 + p3) * u * u * u)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data;
    use crate::mapgen::Generator;

    /// A TAB map gets its ravens near the command center and rats at its ruins – and they stay off blocked tiles.
    #[test]
    fn a_tab_map_gets_ravens_and_rats() {
        let map = data::get().maps.maps.iter().find(|m| m.setup.generator == Generator::Tab && m.setup.look == "VO").unwrap();
        let world = World::generate(42, &map.setup);
        let mut critters = Critters::default();
        critters.update(&world, 0.0);
        let ravens = critters.list.iter().filter(|c| c.kind == Kind::Raven).count();
        let rats = critters.list.iter().filter(|c| c.kind == Kind::Rat).count();
        assert!(ravens > 10 && rats > 5, "{ravens} ravens, {rats} rats");
        for c in &critters.list {
            assert!(!world.map.is_blocked(c.pos.x.floor() as i32, c.pos.y.floor() as i32));
        }
        // A raven that's made to fly lands again on free ground.
        let raven = critters.list.iter().position(|c| c.kind == Kind::Raven).unwrap();
        let from = critters.list[raven].pos;
        critters.list[raven].doing = critters.take_off(&world, from, 0.0);
        let mut now = 0.0;
        while now < 30.0 && !matches!(critters.list[raven].doing, Doing::Stand { .. } | Doing::Special { .. }) {
            now += 0.1;
            critters.step(&world, raven, now, 0.1);
        }
        let end = critters.list[raven].pos;
        assert!(end.distance(from) > 5.0, "it flew from {from} to {end}");
        assert_eq!(critters.list[raven].height, 0.0);
    }
}
