//! The events of a map, like TAB's (`ZXLevelEvent`, DESIGN.md 8.2): everything that comes by the clock – the
//! stragglers, the swarms, the final wave and the victory are all events. An event comes at its hour (plus a random
//! offset rolled when the round starts), maybe again and again, growing each time; an announced one is shown hours
//! before, with its sides – they're picked then. When it comes, its zombies don't appear all at once: every side lets
//! them out a few at a time, like TAB's spawners (`CUnitGenerator`) – a swarm comes out as a long column.
//!
//! A new round gets TAB's survival plan (`data\spawning.json`, `plan_events`): its hours stretch with the round's days
//! (TAB's are for 100), its counts with the difficulty's population. A map file keeps the events, the editor changes
//! them ("Events" tab).

use macroquad::math::Vec2;
use serde::{Deserialize, Serialize};

use super::spawning::{BreedCount, WaveFrom, WaveGroup};
use super::story::TriggerEvent;
use super::zombies::Breed;
use super::{World, HOUR_TICKS};
use crate::data::{self, EventPlan};
use crate::rng::Rng;

/// An event of the map – its settings as TAB's editor has them (their TAB names in the comments). Hours count from the
/// start of the round (day 1, 0:00 = hour 0).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Event {
    pub name: String,
    /// It comes at this hour (`GameTime`) …
    pub hour: u32,
    /// … plus up to this many hours, rolled when the round starts (`GameTimeRandomOffset`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub random_hours: u32,
    /// Again every this many hours – 0: once (`RepeatTime`) …
    #[serde(default, skip_serializing_if = "is_zero")]
    pub every_hours: u32,
    /// … at most this many times – 0: as long as the round lasts (`MaxRepetitions`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub times: u32,
    /// The n-th time brings `1 + growth × (n − 1)` times as many, the extra at most `growth_max`
    /// (`FactorUnitsNumberPerRepetition`, `MaxFactorUnitsNumberPerRepetition` – 0: no limit).
    #[serde(default)]
    pub growth: f32,
    #[serde(default)]
    pub growth_max: f32,
    /// Announced this many hours before it comes – with a countdown on the clock and its swarms on the minimap
    /// (`AutoNotifyPlayer`, `TimeToNotifyInAdvance`, `ShowCountdown`, `ShowMiniMapIndicator`). `None`: it comes
    /// silently.
    #[serde(default)]
    pub notify_hours: Option<u32>,
    /// Every side's zombies come out of a square this many tiles big, at a random spot of one of its spawners
    /// (`MaxCellDispersionGeneration`) – 0: spread over all its spawners.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub dispersion: u32,
    /// The final wave (`FinalSwarm`, `AllInfectedToCommandCenter`): it calls every zombie on the map – they all
    /// belong to it then (`World::call_everyone`).
    #[serde(default)]
    pub final_wave: bool,
    /// From then on the whole map is shown (`ShowAllMap`).
    #[serde(default)]
    pub reveal: bool,
    /// The round is won when it comes (`GameWon`) …
    #[serde(default)]
    pub won: bool,
    /// … or once it has come every time and none of the final wave is left (`GameWonWhenSwarmDestroyed`).
    #[serde(default)]
    pub won_when_dead: bool,
    /// Its zombies: every group out of its own place, with its breeds (TAB: one group – `Generators`,
    /// `EntityType1` … `EntityType10`).
    #[serde(default)]
    pub groups: Vec<WaveGroup>,
    /// While the round runs – not in the file (`World::roll_events` sets it when the round starts). `pub` only so an
    /// event can be written as `Event { …, ..Event::new(…) }` outside the simulation – only the simulation changes it.
    #[serde(skip)]
    pub state: EventState,
}

/// What an event keeps while the round runs.
#[derive(Clone, Default, Debug)]
pub struct EventState {
    /// The hour it comes the first time: `hour` plus the rolled offset (`StartTimeH`).
    pub start: u32,
    /// How often it came, how often it was announced (`NTimesTriggered`, `NTimesNotified`).
    pub fired: u32,
    pub announced: u32,
    /// Per group, per side: the spawners (`id`) its zombies come out of next – picked when it's announced (or, silent,
    /// when it comes).
    from: Option<Vec<Vec<Vec<u32>>>>,
    /// Its "done" triggers have fired.
    done_ran: bool,
}

/// Two events are the same when their settings are – where they are in the round doesn't count (the editor compares
/// them to see whether something was changed).
impl PartialEq for EventState {
    fn eq(&self, _: &EventState) -> bool {
        true
    }
}

/// For `skip_serializing_if`: a 0 isn't written – the file stays short.
fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// Something about the events (and the neutral things) the screen should tell the player – the game takes them (`World::notices`).
#[derive(Clone, Debug, PartialEq)]
pub enum Notice {
    /// An announced event will come at tick `at`, with about this many zombies, from these sides (`Spawner::side`:
    /// 0 top, 1 bottom, 2 left, 3 right – empty: chosen spawners).
    Coming { name: String, at: u64, zombies: u32, sides: Vec<u32>, final_wave: bool },
    /// An announced event came.
    Came { name: String, zombies: u32, final_wave: bool },
    /// A neutral building or unit joined the colony (`neutral.rs`).
    Joined { name: String },
    /// A research is done (`research.rs`).
    Researched { name: String },
    /// A script's line at the top of the screen, and its window (`story.rs`).
    Message { text: String },
    Window { title: String, text: String },
    /// A train stopped at the command center (`trains.rs`), here.
    TrainArrived { at: Vec2 },
}

/// Zombies an event still has to bring out of one side (TAB's `GenerationRequest`): every `stream_every_seconds` the
/// side lets out some of each of its breeds (`stream`).
#[derive(Clone)]
pub(super) struct Request {
    /// The spawners (`id`) of the side – any of them, or …
    spawners: Vec<u32>,
    /// … (no spawners then) tiles of these regions (`id`, `WaveFrom::Zones`) – or …
    zones: Vec<u32>,
    /// … a square (in the axes of the first one: middle, half its size) – `None`: anywhere in them.
    square: Option<(Vec2, Vec2)>,
    breed: Breed,
    left: u32,
    /// Its share of each burst, rolled once (TAB rolls it from the same seed every time – the same number).
    share: f32,
    swarm: u32,
    final_wave: bool,
    /// They storm the command center – a script's zombies may idle where they come out instead (`Request::idle`).
    storm: bool,
}

impl Request {
    /// Its zombies idle where they come out instead of storming (`Action::Zombies` with `storm` off).
    pub(super) fn idle(request: &mut Request) {
        request.storm = false;
    }
}

impl Event {
    /// An event that comes once at `hour`, with one group of one breed – the editor's "+ Add event" and the tests.
    pub fn new(name: &str, hour: u32, from: WaveFrom, breed: Breed, count: u32) -> Event {
        Event {
            name: name.to_owned(),
            hour,
            random_hours: 0,
            every_hours: 0,
            times: 0,
            growth: 0.0,
            growth_max: 0.0,
            notify_hours: None,
            dispersion: 0,
            final_wave: false,
            reveal: false,
            won: false,
            won_when_dead: false,
            groups: vec![WaveGroup { from, breeds: vec![BreedCount::new(count, breed)] }],
            state: EventState::default(),
        }
    }

    /// How often it comes in all – `None`: as long as the round lasts.
    pub fn repeats(&self) -> Option<u32> {
        match (self.every_hours, self.times) {
            (0, _) => Some(1),
            (_, 0) => None,
            (_, n) => Some(n),
        }
    }

    /// How often it came so far.
    pub fn fired(&self) -> u32 {
        self.state.fired
    }

    /// Has it come every time it will?
    pub fn is_done(&self) -> bool {
        self.repeats().is_some_and(|n| self.state.fired >= n)
    }

    /// The hour it comes next – `None` once it's done (TAB's `StartTimeH + NTimesTriggered × RepeatTimeH`).
    pub fn next_hour(&self) -> Option<u32> {
        (!self.is_done()).then(|| self.state.start + self.state.fired * self.every_hours)
    }

    /// Is it announced and on its way – the countdown shows (`IsCurrentlyBeingNotified`)?
    pub fn is_announced(&self) -> bool {
        self.notify_hours.is_some() && self.state.announced > self.state.fired
    }

    /// How many times as many it brings the next time: `1 + growth × (times so far)`, the extra at most `growth_max`.
    fn factor(&self, fired: u32) -> f32 {
        let mut extra = fired as f32 * self.growth;
        if self.growth_max > 0.0 {
            extra = extra.min(self.growth_max);
        }
        1.0 + extra
    }

    /// About how many zombies one group brings once – the middle of every range, before the growth.
    fn middle(group: &WaveGroup) -> u32 {
        group.breeds.iter().map(|b| (b.count + b.up_to.max(b.count)) / 2).sum()
    }

    /// About how many zombies it brings the next time, for the notices: per group the middle of its ranges, times the
    /// growth, times its sides picked (`from`) – a group out of regions comes once.
    fn expected(&self, from: &[Vec<Vec<u32>>]) -> u32 {
        let factor = self.factor(self.state.fired);
        self.groups
            .iter()
            .zip(from)
            .map(|(group, sides)| {
                let places = match &group.from {
                    WaveFrom::Zones(ids) => ids.len().min(1) as u32,
                    _ => sides.len() as u32,
                };
                (Event::middle(group) as f32 * factor) as u32 * places
            })
            .sum()
    }

    /// About how many zombies it brings the `time`-th time it comes (0 is the first): its groups' middles times the
    /// growth – times the sides of a `Sides` group, once for any other.
    pub fn at_time(&self, time: u32) -> u32 {
        let per_time: u32 = self
            .groups
            .iter()
            .map(|g| {
                let places = match &g.from {
                    WaveFrom::Sides(n) => (*n).min(4),
                    WaveFrom::Spawners(ids) | WaveFrom::Zones(ids) => ids.len().min(1) as u32,
                    WaveFrom::OneSpawner => 1,
                };
                Event::middle(g) * places
            })
            .sum();
        (per_time as f32 * self.factor(time)) as u32
    }

    /// About how many zombies it brings in all, counting every time it comes up to the hour `last` (the round's end).
    pub fn total(&self, last: u32) -> u32 {
        let mut all = 0;
        for time in 0..self.repeats().unwrap_or(u32::MAX) {
            if self.hour + time * self.every_hours > last {
                break;
            }
            all += self.at_time(time);
            if self.every_hours == 0 {
                break;
            }
        }
        all
    }
}

/// TAB's survival plan (`data\spawning.json`) for a new round of `days` days at this zombie `population`: its hours
/// stretched by `days / 100` (rounded up, TAB's `z_6sbTOMQ` – not the random offset, not the notice), every count
/// times the population, at least 1 (not the giants – TAB scales only the breeds of power up to 100). Below
/// population 0.5 an event takes its `low_population` timing. 0 days: no events – a map without a set length.
pub fn plan_events(days: u32, population: f32) -> Vec<Event> {
    if days == 0 {
        return Vec::new();
    }
    let stretch = days as f32 / 100.0;
    let scale = |hours: u32| (hours as f32 * stretch).ceil() as u32;
    let people = |n: u32| ((n as f32 * population + 0.5).floor() as u32).max(1);
    data::get()
        .spawning
        .events
        .iter()
        .map(|plan: &EventPlan| {
            let mut event = plan.event.clone();
            if let Some(low) = plan.low_population.filter(|_| population < 0.5) {
                (event.hour, event.every_hours, event.times) = (low.hour, low.every_hours, low.times);
            }
            event.hour = scale(event.hour);
            event.every_hours = scale(event.every_hours);
            for b in event.groups.iter_mut().flat_map(|g| g.breeds.iter_mut()).filter(|b| b.breed != Breed::Giant) {
                if b.count > 0 || b.up_to > 0 {
                    (b.count, b.up_to) = (people(b.count), if b.up_to > 0 { people(b.up_to) } else { 0 });
                }
            }
            event
        })
        .collect()
}

impl World {
    /// The round starts: every event forgets what it did and rolls its random offset – the same on every PC.
    pub fn roll_events(&mut self) {
        for e in 0..self.events.len() {
            self.roll_event(e);
        }
    }

    /// The same for event `e` alone. Its offset comes from a random generator of its own (like TAB's `Random` per
    /// event), seeded by the map's seed and its place in the list – the world's dice stay untouched.
    pub(super) fn roll_event(&mut self, e: usize) {
        let mut dice = Rng::new(self.seed ^ (e as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let offset = (dice.unit_f32() * self.events[e].random_hours as f32) as u32;
        let event = &mut self.events[e];
        event.state = EventState { start: event.hour + offset, ..EventState::default() };
    }

    /// Every tick, like TAB's `ZXLevelEvent` (`ShouldNotifyNow`, `ShouldTriggerNow`): an event whose hour minus its
    /// notice has come is announced (its sides picked); one whose hour has come comes – an announced one only after it
    /// was announced. Then the sides let out their zombies (`stream`). In the order of the list.
    pub(super) fn update_events(&mut self) {
        let hours = (self.tick / HOUR_TICKS) as u32;
        for e in 0..self.events.len() {
            let event = &self.events[e];
            let Some(next) = event.next_hour() else {
                continue;
            };
            if let Some(notice) = event.notify_hours {
                let announce_at = (event.state.start + event.state.announced * event.every_hours).saturating_sub(notice);
                let more = event.repeats().is_none_or(|n| event.state.announced < n);
                if more && event.state.announced == event.state.fired && hours >= announce_at {
                    self.announce(e);
                }
                if self.events[e].state.announced <= self.events[e].state.fired {
                    continue;
                }
            }
            if hours >= next {
                self.fire_event(e);
            }
        }
        self.finish_events();
        self.stream();
    }

    /// Event `e` is announced: its sides are picked now, and the screen says it's coming.
    fn announce(&mut self, e: usize) {
        let from = self.pick_from(e);
        let event = &self.events[e];
        let sides = self.sides_of(&from);
        let at = event.next_hour().unwrap_or(0) as u64 * HOUR_TICKS;
        let zombies = event.expected(&from);
        self.notices.push(Notice::Coming { name: event.name.clone(), at, zombies, sides, final_wave: event.final_wave });
        let state = &mut self.events[e].state;
        state.announced += 1;
        state.from = Some(from);
    }

    /// Event `e` comes (TAB's `z_rITPsSA`): the map is shown, the final wave calls everyone, the round may be won – and
    /// every side of every group gets its zombies to let out: per breed a random count in its range (the same on every
    /// side), times its growth. A silent event – TAB's stragglers – brings the colony's unrest along, one more of its
    /// first breed per side for every `unrest_per_zombie` (`sound.rs`).
    fn fire_event(&mut self, e: usize) {
        let from = match self.events[e].state.from.take() {
            Some(from) => from,
            None => self.pick_from(e),
        };
        let event = self.events[e].clone();
        let factor = event.factor(event.state.fired);
        self.events[e].state.fired += 1;
        if event.reveal {
            self.revealed = true;
        }
        if event.final_wave {
            self.call_everyone();
        }
        let unrest = if event.notify_hours.is_none() { self.unrest_zombies() } else { 0 };
        let mut total = 0;
        for (group, sides) in event.groups.iter().zip(&from) {
            let counts: Vec<(Breed, u32)> = group
                .breeds
                .iter()
                .enumerate()
                .map(|(k, b)| {
                    let (lo, hi) = (b.count, b.up_to.max(b.count));
                    let rolled = lo + self.rng.below(hi - lo + 1);
                    let grown = (rolled as f32 * factor + 0.5).floor() as u32;
                    (b.breed, grown + if k == 0 { unrest } else { 0 })
                })
                .collect();
            if let Some(zones) = self.wave_zones(&group.from) {
                if !zones.is_empty() {
                    total += counts.iter().map(|c| c.1).sum::<u32>();
                }
                self.request_zones(&zones, &counts, event.notify_hours.is_some(), event.final_wave);
            }
            for side in sides {
                total += counts.iter().map(|c| c.1).sum::<u32>();
                self.request(side, &counts, event.dispersion, event.notify_hours.is_some(), event.final_wave);
            }
        }
        if event.notify_hours.is_some() {
            self.notices.push(Notice::Came { name: event.name.clone(), zombies: total, final_wave: event.final_wave });
        }
        self.fire(&TriggerEvent::EventComes { event: event.name.clone() });
    }

    /// Every event that has come every time fires its "done" triggers once (TAB's `ScriptOnFinished`) – a final wave
    /// only once none of it is left or still to come.
    fn finish_events(&mut self) {
        for e in 0..self.events.len() {
            let event = &self.events[e];
            if event.state.done_ran || !event.is_done() {
                continue;
            }
            if event.final_wave && (self.final_wave_pending() || self.zombies.iter().any(|z| z.final_wave)) {
                continue;
            }
            self.events[e].state.done_ran = true;
            let name = self.events[e].name.clone();
            self.fire(&TriggerEvent::EventDone { event: name });
        }
    }

    /// Where event `e`'s groups come out this time – per group a list of sides, each a list of spawners (`id`).
    fn pick_from(&mut self, e: usize) -> Vec<Vec<Vec<u32>>> {
        let froms: Vec<WaveFrom> = self.events[e].groups.iter().map(|g| g.from.clone()).collect();
        froms
            .iter()
            .map(|from| self.wave_sides(from).into_iter().map(|side| side.into_iter().map(|s| self.spawners[s].id).collect()).collect())
            .collect()
    }

    /// The sides of the map these spawners lie on, each once, in order – for the notice. Chosen spawners: none.
    fn sides_of(&self, from: &[Vec<Vec<u32>>]) -> Vec<u32> {
        let mut sides: Vec<u32> = from
            .iter()
            .flatten()
            .filter_map(|side| side.first())
            .filter_map(|&id| self.find_spawner(id))
            .map(|s| self.spawners[s].side(&self.map))
            .collect();
        sides.sort_unstable();
        sides.dedup();
        sides
    }

    /// One side of an event gets its zombies to let out: one request per breed, all from the same square of one of its
    /// spawners if the event has a `dispersion` (TAB: one seed per side), a swarm of their own if it's announced (the
    /// minimap marks it).
    pub(super) fn request(&mut self, side: &[u32], counts: &[(Breed, u32)], dispersion: u32, shown: bool, final_wave: bool) {
        let alive: Vec<u32> = side.iter().copied().filter(|&id| self.find_spawner(id).is_some()).collect();
        if alive.is_empty() {
            return;
        }
        let (spawners, square) = if dispersion > 0 {
            let id = alive[self.rng.below(alive.len() as u32) as usize];
            (vec![id], self.square_in(id, dispersion as f32))
        } else {
            (alive, None)
        };
        let swarm = if shown { self.new_swarm() } else { 0 };
        for &(breed, left) in counts.iter().filter(|c| c.1 > 0) {
            let share = self.rng.unit_f32();
            self.requests.push(Request { spawners: spawners.clone(), zones: Vec::new(), square, breed, left, share, swarm, final_wave, storm: true });
        }
    }

    /// A group from regions gets its zombies to let out: one request per breed, anywhere in the regions, a swarm of
    /// their own if it's announced.
    pub(super) fn request_zones(&mut self, zones: &[u32], counts: &[(Breed, u32)], shown: bool, final_wave: bool) {
        if zones.is_empty() {
            return;
        }
        let swarm = if shown { self.new_swarm() } else { 0 };
        for &(breed, left) in counts.iter().filter(|c| c.1 > 0) {
            let share = self.rng.unit_f32();
            self.requests.push(Request { spawners: Vec::new(), zones: zones.to_vec(), square: None, breed, left, share, swarm, final_wave, storm: true });
        }
    }

    /// A square `size` tiles big at a random spot of spawner `id`, in its axes (TAB's `MaxCellDispersionGeneration`):
    /// its middle and half its size – no bigger than the spawner. A step of the screen axes is `1 / √2` of a tile.
    fn square_in(&mut self, id: u32, size: f32) -> Option<(Vec2, Vec2)> {
        let s = self.find_spawner(id)?;
        let spawner = &self.spawners[s];
        let steps = match spawner.axes {
            super::Axes::Screen => size * std::f32::consts::SQRT_2,
            super::Axes::Tiles => size,
        };
        let full = Vec2::new(spawner.size.0 as f32, spawner.size.1 as f32);
        let half = Vec2::splat(steps).min(full) / 2.0;
        let room = full - half * 2.0;
        let min = Vec2::new(spawner.min.0 as f32, spawner.min.1 as f32);
        let middle = min + half + Vec2::new(self.rng.unit_f32() * room.x, self.rng.unit_f32() * room.y);
        Some((middle, half))
    }

    /// Every `stream_every_seconds` each side lets out a burst, like TAB's spawners: its requests, from the last one
    /// back, each `left × speed / all left × share × 5` (at least 1, at most `stream_speed`); the first one fills up
    /// to `stream_speed`. So a side's breeds come mixed, a big swarm as a long column. A side whose spawners are gone,
    /// or where nothing leads to the base, gives up its zombies.
    fn stream(&mut self) {
        let spawning = &data::get().spawning;
        if self.requests.is_empty() || !self.tick.is_multiple_of(spawning.stream_every.max(1) as u64) {
            return;
        }
        let speed = spawning.stream_speed.max(1);
        // The sides (by their spawners) in the order they first appear.
        let place = |r: &Request| (r.spawners.clone(), r.zones.clone());
        let mut sides: Vec<(Vec<u32>, Vec<u32>)> = Vec::new();
        for r in &self.requests {
            if !sides.contains(&place(r)) {
                sides.push(place(r));
            }
        }
        for side in sides {
            let mine: Vec<usize> = (0..self.requests.len()).filter(|&r| place(&self.requests[r]) == side).collect();
            let all: u32 = mine.iter().map(|&r| self.requests[r].left).sum();
            let mut made = 0;
            for (k, &r) in mine.iter().enumerate().rev() {
                let req = &self.requests[r];
                let n = if k == 0 {
                    req.left.min(speed.saturating_sub(made))
                } else {
                    req.left.min((req.left as f32 * speed as f32 / all as f32 * req.share * 5.0 + 0.5).floor() as u32)
                };
                let n = n.clamp(1, speed).min(req.left);
                made += n;
                self.let_out_request(r, n);
            }
        }
        self.requests.retain(|r| r.left > 0);
    }

    /// `n` zombies of request `r` come out – on free spots of its square or its spawners that lead to the base. None
    /// found: the request is dropped (nobody can come from there).
    fn let_out_request(&mut self, r: usize, n: u32) {
        let req = self.requests[r].clone();
        if !req.zones.is_empty() {
            for _ in 0..n {
                let Some(pos) = self.zone_point(&req.zones) else {
                    self.requests[r].left = 0;
                    return;
                };
                self.spawn_zombie_as(pos, req.breed, !req.storm);
                if let Some(zombie) = self.zombies.last_mut() {
                    zombie.final_wave = req.final_wave;
                    zombie.swarm = req.swarm;
                }
                self.requests[r].left -= 1;
            }
            return;
        }
        let indices: Vec<usize> = req.spawners.iter().filter_map(|&id| self.find_spawner(id)).collect();
        if indices.is_empty() {
            self.requests[r].left = 0;
            return;
        }
        for _ in 0..n {
            let point = match req.square {
                Some(square) => self.point_in_square(indices[0], square),
                None => self.spawn_point(&indices).map(|(p, _)| p),
            };
            let Some(pos) = point.or_else(|| self.spawn_point(&indices).map(|(p, _)| p)) else {
                self.requests[r].left = 0;
                return;
            };
            self.spawn_zombie_as(pos, req.breed, !req.storm);
            if let Some(zombie) = self.zombies.last_mut() {
                zombie.final_wave = req.final_wave;
                zombie.swarm = req.swarm;
            }
            self.requests[r].left -= 1;
        }
    }

    /// A random free point in the square (`square_in`) of spawner `s` that leads to the base – `None` after a few tries.
    fn point_in_square(&mut self, s: usize, (middle, half): (Vec2, Vec2)) -> Option<Vec2> {
        let axes = self.spawners[s].axes;
        for _ in 0..100 {
            let a = middle + Vec2::new((self.rng.unit_f32() * 2.0 - 1.0) * half.x, (self.rng.unit_f32() * 2.0 - 1.0) * half.y);
            let p = axes.back(a);
            let (x, y) = (p.x.floor() as i32, p.y.floor() as i32);
            if !self.map.is_blocked(x, y) && self.to_base.reachable(x, y) {
                return Some(p);
            }
        }
        None
    }

    /// Are zombies of the final wave still to come out of a spawner?
    pub(super) fn final_wave_pending(&self) -> bool {
        self.requests.iter().any(|r| r.final_wave)
    }

    /// The tick the next announced event comes at, and whether it's the final wave – for the clock.
    pub fn next_announced(&self) -> Option<(u64, bool)> {
        self.events
            .iter()
            .filter(|e| e.is_announced())
            .filter_map(|e| e.next_hour().map(|h| (h as u64 * HOUR_TICKS, e.final_wave)))
            .min_by_key(|&(at, _)| at)
    }

    /// The event with this index was changed or added in the editor: it starts anew.
    pub(super) fn event_changed(&mut self, e: usize) {
        self.roll_event(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TAB's plan stretched to the days and scaled to the population: hours × days / 100 rounded up, counts ×
    /// population (at least 1), the victory at the end of the last day.
    #[test]
    fn the_plan_stretches_with_the_days() {
        let hundred = plan_events(100, 1.0);
        let eighty = plan_events(80, 1.0);
        let won = |events: &[Event]| events.iter().find(|e| e.won).map(|e| e.hour);
        assert_eq!(won(&hundred), Some(2400));
        assert_eq!(won(&eighty), Some(1920));
        for (h, e) in hundred.iter().zip(&eighty) {
            assert_eq!(e.hour, (h.hour as f32 * 0.8).ceil() as u32, "{}", h.name);
            assert_eq!(e.every_hours, (h.every_hours as f32 * 0.8).ceil() as u32, "{}", h.name);
        }
        let low = plan_events(100, 0.6);
        let count = |events: &[Event]| events.iter().flat_map(|e| &e.groups).flat_map(|g| &g.breeds).map(|b| b.count).sum::<u32>();
        assert!(count(&low) < count(&hundred));
        assert!(low.iter().flat_map(|e| &e.groups).flat_map(|g| &g.breeds).all(|b| b.count >= 1));
        assert!(plan_events(0, 1.0).is_empty());
    }
}
