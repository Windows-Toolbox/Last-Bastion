//! TAB's train (`ZX.Entities\Locomotive.cs`, `TrainCarriage.cs`): a locomotive and its cars on a line of track
//! (`rails.rs`). A script sends it (`Action::Train`, TAB's `ZXAction_SendTrain`): it appears whole at the start of the
//! line, speeds up, brakes towards its stop next to the command center, waits there 3 s – then the arrival's script
//! runs (TAB unloads its colonists) – and goes on to the end of the line: outside the world border it's gone there, on
//! the playing field it stays. Every send is a train of its own (TAB had one, sent again it started over).
//!
//! Running, it runs over everyone in its way – your units and zombies alike, 100 damage every tick they're in it
//! (TAB's `AddDamage(100)` with the impulse of its body); standing or running, nobody stands inside it: they're pushed
//! out to its side (TAB: its physical body, and the cells it blocks while it stands). Ways aren't planned round it –
//! they just slide along it.

use macroquad::math::{vec2, Vec2};

use super::events::Notice;
use super::fx::Fx;
use super::rails::RailLine;
use super::buildings::Building;
use super::{after_armor, Place, World, UNIT_RADIUS};
use crate::data::{self, TrainValues};
use crate::kinds::BuildingKind;

/// How far from a station's site the track may run for a train to stop there (tiles) – the track right next to it.
pub const STATION_REACH: f32 = 1.0;
/// The Name-ID of the station where a train stops for the command center (`Action::Train`).
pub const COMMAND_CENTER_STATION: &str = "Command Center";

/// What a car of a train is – TAB's `TypeOfCarriage`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarKind {
    Locomotive,
    Colonists,
    Armored,
    Goods,
}

impl CarKind {
    /// How long it is (tiles, `data\trains.json`).
    pub fn length(self) -> f32 {
        let cars = &data::get().trains.cars;
        match self {
            CarKind::Locomotive => cars.locomotive,
            CarKind::Colonists => cars.colonists,
            CarKind::Armored => cars.armored,
            CarKind::Goods => cars.goods,
        }
    }
}

/// How the train is moving – TAB's `TypeOfMovement`, for its sounds (`audio.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Stopped,
    Accelerating,
    Decelerating,
    FullSpeed,
}

/// Where a train stops (TAB's `StopPoint`): this far along its line – and what runs when it's there.
#[derive(Clone, Debug, PartialEq)]
struct Stop {
    offset: f32,
    /// The script of the story that runs when it's there (`Action::Train`'s arrival).
    script: Option<Place>,
}

/// A train on a line of track.
#[derive(Clone, Debug)]
pub struct Train {
    pub line: RailLine,
    /// The locomotive first, then its cars.
    pub cars: Vec<CarKind>,
    /// How far along the line the locomotive's middle is (tiles), now and a tick ago (for drawing between ticks).
    pub offset: f32,
    pub prev_offset: f32,
    /// Tiles per tick.
    pub speed: f32,
    pub motion: Motion,
    stops: Vec<Stop>,
    next: usize,
    /// Standing at a stop since this tick – `None` while it runs.
    halted: Option<u64>,
    /// `false` once it's at the end of its line (TAB's `Started`).
    pub(super) running: bool,
}

/// One car of a train where it is: its kind, the middle, which way it points along the track (length 1).
#[derive(Clone, Copy, Debug)]
pub struct CarAt {
    pub kind: CarKind,
    pub pos: Vec2,
    pub dir: Vec2,
}

/// Where a train sent now would run (`World::train_plan`): its line from where it comes in, its cars, where the
/// locomotive's middle starts, stops for the command center and ends (tiles along the line).
#[derive(Clone, Debug)]
pub struct TrainPlan {
    pub line: RailLine,
    pub cars: Vec<CarKind>,
    pub start: f32,
    pub stop: f32,
    pub end: f32,
    /// The stop lies behind where the train comes in, from both ends – the train stops right where it comes in. The
    /// editor warns.
    pub cramped: bool,
}

impl TrainPlan {
    /// The cars standing with the locomotive's middle `offset` along the line – like a train there would.
    pub fn cars_at(&self, offset: f32) -> Vec<CarAt> {
        place_cars(&self.line, &self.cars, offset)
    }
}

impl Train {
    /// The whole train from the front of the locomotive to the end of the last car (TAB's `FullTrainLength`).
    fn full_length(&self, gap: f32) -> f32 {
        self.cars.iter().map(|c| c.length()).sum::<f32>() + gap * (self.cars.len() - 1) as f32
    }

    /// Every car with the locomotive `offset` tiles along the line (TAB's `UpdatePosition`): the first car half a gap
    /// behind it, then a whole gap between each two.
    pub fn cars_at(&self, offset: f32) -> Vec<CarAt> {
        place_cars(&self.line, &self.cars, offset)
    }

    /// The motion for the sounds (TAB's `_MovementType`): full speed, braking within `brake_tiles` of the stop,
    /// fast again over 80 % of the top speed, else speeding up.
    fn motion_towards(&self, target: f32, t: &TrainValues) -> Motion {
        if self.speed >= t.max_speed || self.speed == 0.0 {
            Motion::FullSpeed
        } else if target - self.offset <= t.brake_tiles {
            Motion::Decelerating
        } else if self.speed > t.max_speed * 0.8 {
            Motion::FullSpeed
        } else {
            Motion::Accelerating
        }
    }
}

impl World {
    /// Where a train stops for the station `station`, on `line`: the locomotive's middle at the point of the track
    /// closest to the station's middle – if the track runs at most `STATION_REACH` tiles from the station's site.
    /// `None`: the track doesn't pass it.
    pub fn station_stop(&self, line: &RailLine, station: &Building) -> Option<f32> {
        let (offset, _) = line.nearest(station.center());
        let (corner, far) = (station.corner(), station.far_corner());
        let at = line.point_at(offset);
        let outside = vec2((corner.x - at.x).max(at.x - far.x).max(0.0), (corner.y - at.y).max(at.y - far.y).max(0.0));
        (outside.length() <= STATION_REACH).then_some(offset)
    }

    /// The station for the command center: the train station with the Name-ID `Command Center`, if the editor put one
    /// down and it still stands (a destroyed one stops no train).
    pub fn command_center_station(&self) -> Option<&Building> {
        self.buildings.iter().find(|b| {
            b.kind == BuildingKind::TrainStation && b.alive() && b.tag.as_deref().is_some_and(|t| t.name_id == COMMAND_CENTER_STATION)
        })
    }

    /// Where a train sent now would run (`Action::Train`): a locomotive with a colonists', an armoured and a goods
    /// car, like TAB's – on the line the whole train fits on that passes closest to the command center, from its end
    /// nearer the edge of the map (`reverse`: from the other one). `None`: no such line on the map. The editor shows it
    /// (`TrainPlan`).
    ///
    /// It stops at the station `Command Center` if there is one: only on a line that passes it, and from the end it can
    /// be reached from – a station closer to the start than the train is long is reached from the other end (if it can
    /// from there; `TrainPlan::cramped` otherwise). Without that station it stops at the piece of track closest to the
    /// command center, like TAB's.
    pub fn train_plan(&self, reverse: bool) -> Option<TrainPlan> {
        let gap = data::get().trains.gap;
        let cars = vec![CarKind::Locomotive, CarKind::Colonists, CarKind::Armored, CarKind::Goods];
        let full = cars.iter().map(|c| c.length()).sum::<f32>() + gap * (cars.len() - 1) as f32;
        let center = self.command_center_middle();
        let station = self.command_center_station();
        let size = self.map.size as f32;
        let to_edge = |p: Vec2| p.x.min(p.y).min(size - p.x).min(size - p.y);
        // Where it stops on a line: at the station, else TAB's piece closest to the command center.
        let stop_on = |l: &RailLine| match station {
            Some(station) => self.station_stop(l, station),
            None => l.stop_near(center),
        };
        // Only a line the whole train fits on – and, with a station, one that passes it.
        let line = self
            .rail_lines()
            .into_iter()
            .filter(|l| l.length() >= full && (station.is_none() || stop_on(l).is_some()))
            .min_by(|a, b| {
                let d = |l: &RailLine| stop_on(l).map_or(f32::MAX, |o| l.point_at(o).distance_squared(center));
                d(a).total_cmp(&d(b))
            })?;
        // Whole on the line at its start; the locomotive's front at most at the end.
        let first = cars[0].length() / 2.0;
        let reach = |l: &RailLine| {
            let end = l.length() - first;
            ((full - first).min(end), end, stop_on(l))
        };
        let (a, b) = (line.point_at(0.0), line.point_at(line.length()));
        let mut line = if to_edge(b) < to_edge(a) { line.reversed() } else { line };
        // A station closer to the start than the train is long can't be reached from there – it comes in from the other
        // end then, if it can from there.
        if station.is_some() {
            let (start, _, stop) = reach(&line);
            let back = line.reversed();
            let (back_start, _, back_stop) = reach(&back);
            if stop.is_some_and(|s| s < start) && back_stop.is_some_and(|s| s >= back_start) {
                line = back;
            }
        }
        if reverse {
            line = line.reversed();
        }
        let (start, end, stop) = reach(&line);
        // One behind the start can't be reached going forward (TAB's train never got there), so the train stops right at
        // its start then.
        let cramped = stop.is_some_and(|s| s < start);
        let stop = stop.map_or(start, |o| o.clamp(start, end));
        Some(TrainPlan { line, cars, start, stop, end, cramped })
    }

    /// A train sets off (`Action::Train`, `train_plan`). `stop`: it stops next to the command center and `on_arrival`
    /// runs there. Trains already on that line run on – TAB's `SetTrainToOrigin` sent its one train back to the start.
    pub(super) fn send_train(&mut self, stop: bool, reverse: bool, on_arrival: Place) {
        let Some(plan) = self.train_plan(reverse) else {
            return;
        };
        let mut stops = Vec::new();
        if stop {
            stops.push(Stop { offset: plan.stop, script: Some(on_arrival) });
        }
        stops.push(Stop { offset: plan.end, script: None });
        stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
        let train = Train {
            line: plan.line,
            cars: plan.cars,
            offset: plan.start,
            prev_offset: plan.start,
            speed: 0.0,
            motion: Motion::Accelerating,
            stops,
            next: 0,
            halted: None,
            running: true,
        };
        let at = train.line.point_at(train.offset);
        self.trains.push(train);
        self.fx.push(Fx::TrainStart { at });
    }

    /// The command center's middle – or, without one, the base.
    fn command_center_middle(&self) -> Vec2 {
        self.buildings.iter().find(|b| b.kind == crate::kinds::BuildingKind::CommandCenter).map_or(self.base, |b| b.center())
    }

    /// Every tick: the trains run, stop, go on (TAB's `Locomotive.OnUpdate`) – and run over or push away who's in
    /// their way (`TrainCarriage.OnUpdate`). At the end of a line outside the world border a train is gone.
    pub(super) fn run_trains(&mut self) {
        let t = &data::get().trains;
        let mut scripts = Vec::new();
        let mut gone = Vec::new();
        for (i, train) in self.trains.iter_mut().enumerate() {
            train.prev_offset = train.offset;
            if !train.running {
                continue;
            }
            if let Some(since) = train.halted {
                // TAB's `Continue`: off to the next stop – after the last one it stays where it is.
                if self.tick > since + t.stop as u64 {
                    if train.next + 1 < train.stops.len() {
                        train.next += 1;
                        train.halted = None;
                        self.fx.push(Fx::TrainStart { at: train.line.point_at(train.offset) });
                    } else {
                        train.running = false;
                    }
                }
                continue;
            }
            let Some(target) = train.stops.get(train.next).map(|s| s.offset) else {
                continue;
            };
            let left = (target - train.offset).abs();
            train.speed = (train.speed + t.acceleration).min(t.max_speed);
            train.speed = (t.max_speed * left / t.brake_tiles).clamp(t.least_speed, train.speed.max(t.least_speed));
            train.motion = train.motion_towards(target, t);
            let first = train.cars[0].length() / 2.0;
            let (lo, hi) = (train.full_length(t.gap) - first, train.line.length() - first);
            train.offset = (train.offset + train.speed).clamp(lo.min(hi), hi);
            // There (TAB checks the way left from before this step): it stands, the stop's script runs.
            if left < 0.1 {
                let end = train.line.point_at(train.line.length());
                if train.next + 1 == train.stops.len() && !self.map.in_play(end.x.floor() as i32, end.y.floor() as i32) {
                    gone.push(i);
                    continue;
                }
                train.halted = Some(self.tick);
                train.speed = 0.0;
                train.motion = Motion::Stopped;
                if let Some(script) = train.stops[train.next].script.clone() {
                    scripts.push((train.line.point_at(train.offset), script));
                }
            }
        }
        for i in gone.into_iter().rev() {
            self.trains.remove(i);
        }
        self.trains_push();
        for (at, script) in scripts {
            // TAB's `ZXAction_TrainUnloadColonists`: the event "the train has arrived" and the officer – then the
            // arrival's script.
            self.notices.push(Notice::TrainArrived { at });
            self.begin(script);
        }
    }

    /// Who's inside a train: run over if it's running fast enough (TAB: everyone within 1.5 tiles before and behind
    /// a car and a tile to each side), and pushed out to the side it's on (its body: a tile before and behind, a tile
    /// to each side). Units in a tower aren't on the ground.
    fn trains_push(&mut self) {
        let t = &data::get().trains;
        let cars: Vec<(CarAt, bool)> = self
            .trains
            .iter()
            .flat_map(|train| {
                let hits = train.running && train.halted.is_none() && train.speed > t.hits_from_speed;
                train.cars_at(train.offset).into_iter().map(move |c| (c, hits))
            })
            .collect();
        if cars.is_empty() {
            return;
        }
        let tick = self.tick;
        let map = &self.map;
        // In the way of `car` (a body of `radius` at `pos`): where it's pushed to – out to the side, unless that's
        // blocked (then it stays), and in front of or behind the car's body not at all. `None`: not in the way.
        let in_way = |car: &CarAt, pos: Vec2, radius: f32| -> Option<Vec2> {
            let d = pos - car.pos;
            let side = vec2(-car.dir.y, car.dir.x);
            let (along, across) = (d.dot(car.dir), d.dot(side));
            let half = car.kind.length() / 2.0;
            if across.abs() > 1.0 + radius || along.abs() > half + 1.5 {
                return None;
            }
            if along.abs() > half + 1.0 {
                return Some(pos);
            }
            let sign = if across < 0.0 { -1.0 } else { 1.0 };
            let to = car.pos + car.dir * along + side * sign * (1.0 + radius);
            let free = !map.is_blocked(to.x.floor() as i32, to.y.floor() as i32);
            Some(if free { to } else { pos })
        };
        for (car, hits) in &cars {
            for zombie in self.zombies.iter_mut().filter(|z| z.is_alive()) {
                if let Some(to) = in_way(car, zombie.pos, zombie.radius) {
                    if *hits {
                        zombie.hurt(after_armor(t.damage, zombie.armor()), tick);
                    }
                    zombie.pos = to;
                }
            }
            for unit in self.units.iter_mut().filter(|u| u.hp > 0 && !u.is_inside()) {
                if let Some(to) = in_way(car, unit.pos, UNIT_RADIUS) {
                    if *hits {
                        unit.take_hit(t.damage, tick);
                    }
                    unit.pos = to;
                }
            }
        }
    }
}

/// Every car of `cars` on `line` with the locomotive's middle `offset` along it (TAB's `UpdatePosition`): the first car
/// half a gap behind it, then a whole gap between each two.
fn place_cars(line: &RailLine, cars: &[CarKind], offset: f32) -> Vec<CarAt> {
    let gap = data::get().trains.gap;
    let mut at = offset;
    let mut placed = Vec::with_capacity(cars.len());
    for (i, &kind) in cars.iter().enumerate() {
        let half = kind.length() / 2.0;
        if i > 0 {
            at -= half;
        }
        placed.push(CarAt { kind, pos: line.point_at(at), dir: line.direction_at(at) });
        at -= half + if i == 0 { gap / 2.0 } else { gap };
    }
    placed
}
