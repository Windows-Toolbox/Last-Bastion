//! Your units: work through the command queue, walk, arrive, push each other aside, climb into towers.
//! Whom they shoot at is in `combat.rs`, how a command becomes a queue in `commands.rs`.

use std::collections::VecDeque;
use std::f32::consts::{FRAC_PI_4, PI, TAU};

use macroquad::math::Vec2;

use super::combat::{Aim, Target};
use super::commands::{Task, Waypoint};
use super::{clear_line, push_apart, slide, World, TICK, UNIT_RADIUS};
use crate::data;
use crate::kinds::{Stats, UnitKind};

/// Whoever follows a unit stops at this distance.
const FOLLOW_DISTANCE: f32 = 1.5;
/// Space one unit takes up in a crowd (tiles²): a bit more than its diameter squared, they don't stand perfectly
/// packed.
const UNIT_SPACE: f32 = 0.6;
/// Further away than this, a unit never counts as arrived at the edge of a crowd (enough for about 100 units).
const CROWD_REACH: f32 = 8.0;
/// This close to the edge of a tower a unit climbs in.
const ENTER_REACH: f32 = 0.8;

pub struct Unit {
    /// Fixed number like with the zombies: selection, control groups and orders remember it.
    pub id: u32,
    pub kind: UnitKind,
    pub pos: Vec2,
    pub prev: Vec2,
    pub hp: u32,
    pub kills: u32,
    /// The command queue: in front the command that is running, behind it the ones added with Shift.
    pub(super) queue: VecDeque<Task>,
    /// Did the unit walk towards a target this tick? (Being pushed doesn't count.)
    pub(super) moving: bool,
    /// Where it looks: an angle in radians, 0 = along +x (on screen to the lower right), counted towards +y.
    /// It turns at its `turn_around` speed – when walking, towards where it goes; when shooting, towards the target.
    pub facing: f32,
    /// Ticks left until the weapon is reloaded (`combat.rs`).
    pub(super) cooldown: u32,
    /// Ticks it has aimed at `aim_at` so far – at `aim` (`data\units.json`) it shoots.
    pub(super) aim: u32,
    /// Whom it is aiming at – kept from tick to tick (`combat.rs`).
    pub(super) aim_at: Option<Aim>,
    /// In a tower (its `id`): it stands in the middle of it, can't be hit, shoots and sees further, isn't drawn.
    pub inside: Option<u32>,
}

impl Unit {
    pub fn stats(&self) -> &'static Stats {
        self.kind.stats()
    }

    pub fn is_inside(&self) -> bool {
        self.inside.is_some()
    }

    /// Range and sight – in a tower both are longer.
    pub fn range(&self) -> f32 {
        self.stats().range + if self.is_inside() { data::get().buildings.tower_bonus } else { 0.0 }
    }

    pub fn sight(&self) -> f32 {
        self.stats().sight + if self.is_inside() { data::get().buildings.tower_bonus } else { 0.0 }
    }

    pub fn is_holding(&self) -> bool {
        matches!(self.queue.front(), Some(Task::Hold))
    }

    /// Does it pick its targets by itself right now – standing, holding, attack-move, patrol, in a tower? (Moving,
    /// following, going into a tower: it doesn't shoot. Attack or beam at one in particular: that one is its target.)
    pub(super) fn picks_targets(&self) -> bool {
        self.is_inside() || matches!(self.queue.front(), None | Some(Task::Hold | Task::AttackMove(_) | Task::Patrol { .. }))
    }

    /// What the unit is doing right now – for the panel at the bottom.
    pub fn order_name(&self) -> &'static str {
        match self.queue.front() {
            None => "Idle",
            Some(Task::Move(_)) => "Move",
            Some(Task::AttackMove(_)) => "Attack-move",
            Some(Task::Patrol { .. }) => "Patrol",
            Some(Task::Attack { .. }) => "Attack",
            Some(Task::Heal { .. }) => "Heal",
            Some(Task::Follow { .. }) => "Follow",
            Some(Task::Enter { .. }) => "Enter tower",
            Some(Task::Hold) => "Hold",
        }
    }

    /// Turns towards `dir` – at most as far as it can in one tick (180° take `turn_around` ticks). Returns whether it
    /// faces that way now. `atan2` from `libm`, so it's the same on every PC (DESIGN.md 12.8).
    pub(super) fn turn_toward(&mut self, dir: Vec2) -> bool {
        if dir == Vec2::ZERO {
            return true; // standing right on it: any way is the right one
        }
        let want = libm::atan2f(dir.y, dir.x);
        // The shorter way round: between -180° and +180°.
        let mut off = want - self.facing;
        if off > PI {
            off -= TAU;
        } else if off < -PI {
            off += TAU;
        }
        let turn_around = self.stats().turn_around;
        let step = if turn_around == 0 { PI } else { PI / turn_around as f32 };
        if off.abs() <= step {
            self.facing = want;
            return true;
        }
        self.facing += step * off.signum();
        if self.facing > PI {
            self.facing -= TAU;
        } else if self.facing < -PI {
            self.facing += TAU;
        }
        false
    }

    /// How many commands are in the queue in total.
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// The last point of the command queue – that's where the unit stands when everything is done.
    pub(super) fn last_point(&self) -> Option<Vec2> {
        self.queue.iter().rev().find_map(|task| match task {
            Task::Move(to) | Task::AttackMove(to) | Task::Patrol { to, .. } => Some(to.point),
            Task::Attack { .. } | Task::Heal { .. } | Task::Follow { .. } | Task::Enter { .. } | Task::Hold => None,
        })
    }
}

/// What a unit wants to do this tick. First it's computed for everyone (only reading),
/// then carried out (writing) – like with the zombies.
pub(super) struct Intent {
    pub(super) velocity: Vec2,
    pub(super) walking: bool,
    pub(super) target: Option<Target>,
}

impl World {
    /// Where is the unit with this `id` in the list? `None` if it's dead.
    pub fn find_unit(&self, id: u32) -> Option<usize> {
        self.units.iter().position(|u| u.id == id)
    }

    pub(super) fn spawn_unit(&mut self, kind: UnitKind, pos: Vec2) {
        self.next_unit_id += 1;
        self.units.push(Unit {
            id: self.next_unit_id,
            kind,
            pos,
            prev: pos,
            hp: kind.stats().hp,
            kills: 0,
            queue: VecDeque::new(),
            moving: false,
            facing: FRAC_PI_4, // along +x and +y: straight down on screen, towards the player
            cooldown: 0,
            aim: 0,
            aim_at: None,
            inside: None,
        });
    }

    /// Units: work through the command queue, walk, shoot (or beam).
    pub(super) fn update_units(&mut self) {
        // 1. Refresh attack, heal and follow: target dead → done; target moved on → new way.
        for i in 0..self.units.len() {
            self.refresh_chase(i);
        }

        // 2. Read: what does every unit want this tick? Whom they aim at is chosen for all of them together, so a crowd
        // spreads its fire (`combat.rs`) – then each one's walking.
        let targets = self.choose_targets();
        let intents: Vec<Intent> = (0..self.units.len()).map(|i| self.intent(i, targets[i])).collect();

        // 3. Write: move. Units stay inside the world border.
        let map = &self.map;
        for (unit, intent) in self.units.iter_mut().zip(&intents) {
            unit.prev = unit.pos;
            unit.pos = slide(unit.pos, intent.velocity, |x, y| map.blocks_unit(x, y));
            unit.moving = intent.walking;
            // Walking turns it towards where it goes – it doesn't wait for that, like most units in StarCraft.
            // (Shooting turns it towards the target – `combat.rs`.)
            if intent.walking {
                unit.turn_toward(unit.pos - unit.prev);
            }
        }

        // 4. Arrived? Then the command is done and the next one in the queue is up. At a tower: climb in.
        for i in 0..self.units.len() {
            self.check_arrival(i);
            self.try_enter(i);
        }

        // 5. Shoot – or, for medics, beam (`combat.rs`).
        self.shoot(&intents);
    }

    /// Attack, heal and follow: where is the target now? Dead → command done. Moved on → new way.
    /// Enter: tower gone or full → done. Otherwise the way to the free tile next to it that is closest.
    fn refresh_chase(&mut self, i: usize) {
        let unit = &self.units[i];
        let pos = unit.pos;
        if let Some(Task::Enter { building, chase }) = unit.queue.front() {
            let tower = self.find_building(*building).filter(|&b| self.has_room(b));
            match tower {
                None => {
                    self.units[i].queue.pop_front();
                }
                Some(b) if chase.is_none() => {
                    if let Some(spot) = self.exit_point(b, pos) {
                        let way = Waypoint::new(&self.map, spot);
                        if let Some(Task::Enter { chase, .. }) = self.units[i].queue.front_mut() {
                            *chase = Some(way);
                        }
                    }
                }
                Some(_) => {}
            }
            return;
        }
        // `shoots`: attack and beam need a clear view of the target, following doesn't.
        let (target, close_enough, shoots) = match unit.queue.front() {
            Some(Task::Attack { zombie, .. }) => {
                (self.find_zombie(*zombie).map(|z| self.zombies[z].pos), unit.stats().range, true)
            }
            Some(Task::Heal { unit: other, .. }) => {
                (self.find_unit(*other).map(|o| self.units[o].pos), unit.stats().range, true)
            }
            Some(Task::Follow { unit: other, .. }) => {
                (self.find_unit(*other).map(|o| self.units[o].pos), FOLLOW_DISTANCE, false)
            }
            _ => return,
        };
        let Some(target) = target else {
            self.units[i].queue.pop_front(); // target dead: on with the next command
            return;
        };
        if target.distance(pos) <= close_enough && (!shoots || self.map.clear_sight(pos, target)) {
            return; // close enough: no way needed. Behind a tree it has to walk around it first.
        }
        let map = &self.map;
        if let Some(Task::Attack { chase, .. } | Task::Heal { chase, .. } | Task::Follow { chase, .. }) =
            self.units[i].queue.front_mut()
        {
            if chase.as_ref().map_or(true, |way| way.point.distance(target) > 2.0) {
                *chase = Some(Waypoint::new(map, target));
            }
        }
    }

    /// `nearby`: what it would aim at all by itself (`choose_targets` in `combat.rs`) – what it's already aiming at,
    /// otherwise the nearest zombie not yet taken care of, for medics the nearest hurt unit.
    fn intent(&self, i: usize, nearby: Option<Target>) -> Intent {
        let unit = &self.units[i];
        let stats = unit.stats();
        // In a tower: stand still and shoot from up there.
        if unit.is_inside() {
            return Intent { velocity: Vec2::ZERO, walking: false, target: nearby };
        }

        // Where to walk, what to aim at?
        let (walk_to, target): (Option<&Waypoint>, Option<Target>) = match unit.queue.front() {
            // No command or holding: stand still, aim at whatever is in range.
            None | Some(Task::Hold) => (None, nearby),
            // Move means only move.
            Some(Task::Move(to)) => (Some(to), None),
            // Attack-move and patrol: if something is in range, stop and aim at it.
            Some(Task::AttackMove(to)) | Some(Task::Patrol { to, .. }) => match nearby {
                Some(t) => (None, Some(t)),
                None => (Some(to), None),
            },
            // A particular zombie: shoot when in range and in sight, otherwise go after it.
            Some(Task::Attack { zombie, chase }) => match self.find_zombie(*zombie) {
                Some(z) if self.in_shot(unit.pos, self.zombies[z].pos, stats.range) => (None, Some(Target::Zombie(z))),
                _ => (chase.as_ref(), None),
            },
            // Medic on a particular unit: beam when in range and in sight (hurt or not), otherwise go after it.
            Some(Task::Heal { unit: other, chase }) => match self.find_unit(*other) {
                Some(o) if self.in_shot(unit.pos, self.units[o].pos, stats.range) => (None, Some(Target::Ally(o))),
                _ => (chase.as_ref(), None),
            },
            // Follow: close enough → stand still (and don't shoot, like in StarCraft).
            Some(Task::Follow { unit: other, chase }) => match self.find_unit(*other) {
                Some(o) if self.units[o].pos.distance(unit.pos) <= FOLLOW_DISTANCE => (None, None),
                _ => (chase.as_ref(), None),
            },
            // On the way to a tower: only walk (like "move").
            Some(Task::Enter { chase, .. }) => (chase.as_ref(), None),
        };

        let step = stats.speed * TICK;
        let mut velocity = match walk_to {
            Some(to) => {
                let to_goal = to.point - unit.pos;
                let d = to_goal.length();
                let field = to.field.direction(unit.pos);
                if d <= step {
                    to_goal // last step: exactly onto the target, not beyond it
                } else if d <= to.straight && (field == Vec2::ZERO || clear_line(&self.map, unit.pos, to.point)) {
                    // Close to its own spot in the formation and nothing in the way (or already at the middle,
                    // where the field has no arrow anymore): straight there.
                    to_goal / d * step
                } else {
                    field * step
                }
            }
            None => Vec2::ZERO,
        };
        // Whoever holds can't be pushed away.
        if !unit.is_holding() {
            velocity += self.unit_separation(i) * step;
        }
        Intent { velocity, walking: walk_to.is_some(), target }
    }

    /// Arrived: at the target, just before it at a unit that's already standing there, or at the edge of a crowd
    /// that already fills the space around the target. Give up: if there's no way at all from here to the target.
    fn check_arrival(&mut self, i: usize) {
        let unit = &self.units[i];
        let to = match unit.queue.front() {
            Some(Task::Move(to)) | Some(Task::AttackMove(to)) | Some(Task::Patrol { to, .. }) => to,
            _ => return,
        };
        let pos = unit.pos;
        let d = pos.distance(to.point);
        let no_way = !to.field.reachable(pos.x.floor() as i32, pos.y.floor() as i32);
        if !(d < 0.05 || (d < 2.0 && self.touches_idle_unit(i)) || self.crowd_reached(i, to.point, d) || no_way) {
            return;
        }

        let unit = &mut self.units[i];
        match unit.queue.front_mut() {
            // Patrol: turn around and back – forever, until a new command comes.
            Some(Task::Patrol { to, back }) => std::mem::swap(to, back),
            _ => {
                unit.queue.pop_front();
            }
        }
    }

    /// Close enough to the tower it's heading for, and there's still room: climb in. It stands in the middle
    /// of the tower from now on, and its command queue is empty – only "unload" gets it out again.
    fn try_enter(&mut self, i: usize) {
        let Some(Task::Enter { building, .. }) = self.units[i].queue.front() else {
            return;
        };
        let Some(b) = self.find_building(*building) else {
            return;
        };
        let tower = &self.buildings[b];
        if tower.distance_to(self.units[i].pos) > ENTER_REACH || !self.has_room(b) {
            return;
        }
        let (id, center) = (tower.id, tower.center());
        let unit = &mut self.units[i];
        unit.inside = Some(id);
        unit.pos = center;
        unit.prev = center;
        unit.queue.clear();
    }

    /// Pushing: away from other units and – weaker – away from zombies. Units in a tower don't push anyone.
    fn unit_separation(&self, me: usize) -> Vec2 {
        let pos = self.units[me].pos;
        let mut push = Vec2::ZERO;
        for (i, other) in self.units.iter().enumerate() {
            if i != me && !other.is_inside() {
                push += push_apart(pos, other.pos, UNIT_RADIUS * 2.0);
            }
        }
        let (cx, cy) = (pos.x.floor() as i32, pos.y.floor() as i32);
        for dy in -1..=1 {
            for dx in -1..=1 {
                for &z in self.grid.cell(cx + dx, cy + dy) {
                    let zombie = &self.zombies[z as usize];
                    push += push_apart(pos, zombie.pos, UNIT_RADIUS + zombie.radius) * 0.5;
                }
            }
        }
        push
    }

    /// A group sent to one point can't all stand on it. Before, only the first one got there; the others pressed
    /// around it and only stopped at a unit standing *still* – but with a queue the first one walks on at once,
    /// so they never arrived. Now: if the circle around the point that is closer than me is already full
    /// (as many units in it as fit), I'm as close as I'll get – arrived. The crowd's edge grows with the group,
    /// like the arrival radius in StarCraft. Whether those units stand still or walk on doesn't matter.
    fn crowd_reached(&self, me: usize, point: Vec2, d: f32) -> bool {
        if d > CROWD_REACH {
            return false;
        }
        let closer = self
            .units
            .iter()
            .enumerate()
            .filter(|&(i, other)| i != me && !other.is_inside() && other.pos.distance(point) < d)
            .count();
        // How many units fit into the circle: its area divided by the space one unit needs. `d - UNIT_RADIUS`,
        // because I touch the ones in front of me – my own half doesn't have to fit in.
        let inner = (d - UNIT_RADIUS).max(0.0);
        let room = std::f32::consts::PI * inner * inner / UNIT_SPACE;
        closer > 0 && closer as f32 >= room
    }

    fn touches_idle_unit(&self, me: usize) -> bool {
        let pos = self.units[me].pos;
        self.units.iter().enumerate().any(|(i, other)| {
            i != me && !other.moving && !other.is_inside() && pos.distance(other.pos) < UNIT_RADIUS * 2.0 + 0.05
        })
    }
}
