//! Commands as the player gives them (DESIGN.md 7.1) and what becomes of them: a queue of tasks per unit, each with
//! its target point and the flow field to it – shared by the whole group (magic box, formations).

use std::sync::Arc;

use macroquad::math::Vec2;

use super::terrain::free_point_near;
use super::World;
use crate::flow::FlowField;
use crate::formation::{self, Formation};
use crate::kinds::UnitKind;
use crate::map::Map;

/// A command as the player gives it (DESIGN.md 7.1).
#[derive(Clone, Copy)]
pub enum Command {
    /// Walk there, don't shoot on the way (right click, M).
    Move(Vec2),
    /// Walk there, but fight every zombie in range (T + click). Medics: aim the beam at hurt units on the way.
    AttackMove(Vec2),
    /// Back and forth between start and target, on the way like attack-move (P + click).
    Patrol(Vec2),
    /// Chase exactly this zombie until it's dead (right click on a zombie). The number is its `id`.
    /// Medics have no attack and ignore this.
    Attack(u32),
    /// T + click on one of your own units (its `id`): medics aim their beam at it until it dies,
    /// all others attack-move to where it stands.
    Heal(u32),
    /// Walk after one of your own units until it dies (right click on a unit). Its `id`.
    Follow(u32),
    /// Go into this tower (its `id`; right click on it). Medics stay outside – they have nothing to shoot.
    Enter(u32),
    /// Stand still, only shoot what comes into range (H).
    Hold,
}

impl Command {
    /// The target point – only for commands that walk to a fixed point.
    fn point(&self) -> Option<Vec2> {
        match *self {
            Command::Move(p) | Command::AttackMove(p) | Command::Patrol(p) => Some(p),
            Command::Attack(_) | Command::Heal(_) | Command::Follow(_) | Command::Enter(_) | Command::Hold => None,
        }
    }
}

/// An entry in a unit's command queue: the command plus what the unit needs for it.
pub(super) enum Task {
    Move(Waypoint),
    AttackMove(Waypoint),
    /// `to`: where it's walking right now. At the target `to` and `back` swap – back and forth forever.
    Patrol { to: Waypoint, back: Waypoint },
    /// `chase`: way to the spot where the target was last. If it moves on, it's recomputed.
    Attack { zombie: u32, chase: Option<Waypoint> },
    /// Medic: keep the beam on this unit, walk after it when it's out of range.
    Heal { unit: u32, chase: Option<Waypoint> },
    Follow { unit: u32, chase: Option<Waypoint> },
    /// Walk to the tower and climb in. `chase`: the way to the free tile next to it that is closest.
    Enter { building: u32, chase: Option<Waypoint> },
    Hold,
}

/// A target point and the flow field to it.
#[derive(Clone)]
pub(super) struct Waypoint {
    /// Where exactly this unit should go.
    pub(super) point: Vec2,
    /// `Arc`: all units of one command share the same flow field – it leads to the middle of the
    /// group at the target. Every unit has its own spot `point` there (magic box).
    pub(super) field: Arc<FlowField>,
    /// Closer to `point` than this: straight to its own spot instead of following the field to the middle.
    pub(super) straight: f32,
}

impl Waypoint {
    pub(super) fn new(map: &Map, point: Vec2) -> Waypoint {
        Waypoint { point, field: Arc::new(FlowField::new(map, point)), straight: 0.0 }
    }
}

/// What a point of the command queue means – only for drawing (the colour of the line).
#[derive(Clone, Copy)]
pub enum Mark {
    Move,
    Attack,
    Patrol,
    Heal,
}

impl World {
    /// The points of a unit's command queue in order – for drawing the lines.
    pub fn plan(&self, unit: usize) -> Vec<(Vec2, Mark)> {
        let mut points = Vec::new();
        for task in &self.units[unit].queue {
            match task {
                Task::Move(to) => points.push((to.point, Mark::Move)),
                Task::AttackMove(to) => points.push((to.point, Mark::Attack)),
                Task::Patrol { to, back } => {
                    points.push((to.point, Mark::Patrol));
                    points.push((back.point, Mark::Patrol));
                }
                Task::Attack { zombie, .. } => {
                    if let Some(z) = self.find_zombie(*zombie) {
                        points.push((self.zombies[z].pos, Mark::Attack));
                    }
                }
                Task::Heal { unit, .. } => {
                    if let Some(u) = self.find_unit(*unit) {
                        points.push((self.units[u].pos, Mark::Heal));
                    }
                }
                Task::Follow { unit, .. } => {
                    if let Some(u) = self.find_unit(*unit) {
                        points.push((self.units[u].pos, Mark::Move));
                    }
                }
                Task::Enter { building, .. } => {
                    if let Some(b) = self.find_building(*building) {
                        points.push((self.buildings[b].center(), Mark::Move));
                    }
                }
                Task::Hold => {}
            }
        }
        points
    }

    /// After the map changed, the units' paths still lead through the old map (straight into a new mountain).
    /// So every flow field in the command queues is computed anew – once per field, so a group that
    /// shared one field (magic box) still shares one afterwards.
    pub(super) fn reroute_units(&mut self) {
        let map = &self.map;
        // (old, new): which fields have already been renewed.
        let mut renewed: Vec<(Arc<FlowField>, Arc<FlowField>)> = Vec::new();
        let mut renew = |way: &mut Waypoint| {
            let field = match renewed.iter().find(|(old, _)| Arc::ptr_eq(old, &way.field)) {
                Some((_, new)) => Arc::clone(new),
                None => {
                    // The target itself may have been painted over: then go next to it.
                    let target = free_point_near(map, way.field.target()).unwrap_or(way.field.target());
                    let new = Arc::new(FlowField::new(map, target));
                    renewed.push((Arc::clone(&way.field), Arc::clone(&new)));
                    new
                }
            };
            if map.blocks_unit(way.point.x.floor() as i32, way.point.y.floor() as i32) {
                way.point = field.target(); // its own spot in the formation is gone: to the middle
                way.straight = 0.0;
            }
            way.field = field;
        };
        for unit in &mut self.units {
            for task in &mut unit.queue {
                match task {
                    Task::Move(to) | Task::AttackMove(to) => renew(to),
                    Task::Patrol { to, back } => {
                        renew(to);
                        renew(back);
                    }
                    // Chasing ways are recomputed in the next tick anyway (`refresh_chase`).
                    Task::Attack { chase, .. }
                    | Task::Heal { chase, .. }
                    | Task::Follow { chase, .. }
                    | Task::Enter { chase, .. } => *chase = None,
                    Task::Hold => {}
                }
            }
        }
    }

    /// Gives a command to several units.
    pub(super) fn command(&mut self, ids: &[u32], command: Command, queue: bool, formation: Option<Formation>) {
        // Numbers → indices in the list. Whoever has died in the meantime is simply dropped – and whoever sits
        // in a tower only comes out with "unload".
        let units: Vec<usize> =
            ids.iter().filter_map(|&id| self.find_unit(id)).filter(|&i| !self.units[i].is_inside()).collect();

        // Where each unit stands when this command comes up: now, or at the end of its queue.
        let starts: Vec<Vec2> = units
            .iter()
            .map(|&i| {
                let unit = &self.units[i];
                if queue { unit.last_point().unwrap_or(unit.pos) } else { unit.pos }
            })
            .collect();

        // Where the units walk to. For "heal" the others attack-move to where the healed unit stands.
        let point = match command {
            Command::Heal(id) => self.find_unit(id).map(|u| self.units[u].pos),
            _ => command.point(),
        };
        // In a formation every unit has its own spot – otherwise everyone walks to the point. Outside the world
        // border means: at the border – a bit more than one step of the screen axes inside it, so the spot's tile
        // is inside too (`Region::clamp`).
        let area = self.map.play_area();
        let spots: Option<Vec<Vec2>> = match (point, formation) {
            (Some(_), Some(f)) if units.len() > 1 => {
                let group: Vec<(Vec2, UnitKind)> = units.iter().zip(&starts).map(|(&i, &s)| (s, self.units[i].kind)).collect();
                Some(formation::spots(f, &group).into_iter().map(|p| area.clamp(p, 1.05)).collect())
            }
            _ => None,
        };
        // The middle: of the formation, or the point itself. The flow field to it is computed only once –
        // all units share it. A click on solid terrain or a building aims at the nearest free spot next to it.
        let center = match (&spots, point) {
            (Some(spots), _) => Some(spots.iter().sum::<Vec2>() / spots.len() as f32),
            (None, Some(p)) => Some(area.clamp(p, 1.05)),
            (None, None) => None,
        };
        let target = match center {
            Some(p) => match free_point_near(&self.map, p) {
                Some(p) => Some((Arc::new(FlowField::new(&self.map, p)), p)),
                None => return, // nothing free far and wide: ignore
            },
            None => None,
        };

        // Patrol: the way back leads to where each unit stands when the patrol starts – one shared field to the
        // middle of those spots (before: one field per unit, a hitch with every patrol order).
        let back = match command {
            Command::Patrol(_) if !starts.is_empty() => {
                let middle = starts.iter().sum::<Vec2>() / starts.len() as f32;
                free_point_near(&self.map, middle).map(|p| (Arc::new(FlowField::new(&self.map, p)), p))
            }
            _ => None,
        };

        // First build all tasks (only reading), then enter them (writing).
        let mut tasks = Vec::with_capacity(units.len());
        for (k, &i) in units.iter().enumerate() {
            let (id, heals) = (self.units[i].id, self.units[i].stats().heals);
            let spot = spots.as_ref().map(|s| s[k]);
            let to = target.as_ref().map(|(field, p)| self.waypoint(field, *p, spot));
            let task = match (command, to) {
                (Command::Move(_), Some(to)) => Task::Move(to),
                (Command::AttackMove(_), Some(to)) => Task::AttackMove(to),
                (Command::Patrol(_), Some(to)) => match &back {
                    Some((field, middle)) => Task::Patrol { to, back: self.waypoint(field, *middle, Some(starts[k])) },
                    None => Task::Move(to), // nowhere free to come back to
                },
                (Command::Attack(zombie), _) if !heals => Task::Attack { zombie, chase: None },
                (Command::Heal(other), _) if heals && other != id => Task::Heal { unit: other, chase: None },
                (Command::Heal(_), Some(to)) if !heals => Task::AttackMove(to),
                (Command::Follow(other), _) if other != id => Task::Follow { unit: other, chase: None },
                (Command::Enter(building), _) if !heals => Task::Enter { building, chase: None },
                (Command::Hold, _) => Task::Hold,
                _ => continue, // e.g. following itself, or a medic told to attack a zombie
            };
            tasks.push((i, task));
        }

        for (i, task) in tasks {
            let unit = &mut self.units[i];
            if !queue {
                unit.queue.clear();
            }
            // A "hold" at the end of the queue would block everything after it – so away with it.
            if matches!(unit.queue.back(), Some(Task::Hold)) {
                unit.queue.pop_back();
            }
            unit.queue.push_back(task);
        }
    }

    /// A unit's target point: its spot in the formation – or, without one, the middle. The unit follows the
    /// shared field to the middle until it's close enough to its spot, then walks straight there.
    /// If its spot is on blocked ground (a tree in the line), it takes the nearest free spot next to it.
    fn waypoint(&self, field: &Arc<FlowField>, center: Vec2, spot: Option<Vec2>) -> Waypoint {
        match spot.and_then(|p| free_point_near(&self.map, p)) {
            Some(point) if point != center => {
                Waypoint { point, field: Arc::clone(field), straight: point.distance(center) + 2.0 }
            }
            _ => Waypoint { point: center, field: Arc::clone(field), straight: 0.0 },
        }
    }
}
