//! What the triggers wait for besides the other parts of the simulation: time (every so many seconds, a day and hour),
//! the zombies killed, your units coming into or going out of a region, a unit reaching a neutral thing (TAB's "On
//! Enter", `CScriptOnEnter`). Checked in a fixed place of the tick (`World::watch`), in this order, so the triggers
//! run in the same order on every PC. Buildings finished or fallen fire their events where they happen
//! (`buildings.rs`).
//!
//! All of it is cheap when no trigger asks: every part first looks at what the triggers want.

use macroquad::math::Vec2;

use super::spawning::WaveFrom;
use super::story::{Action, Part, Site, Step, TriggerEvent};
use super::{day_and_hour, World, HOUR_TICKS, TICKS_PER_SECOND};

/// Regions are looked at every this many ticks (a fifth of a second) – a unit crosses a border in more than that.
const REGION_EVERY: u64 = 4;
/// A neutral thing is looked at for a unit that reached it every this many ticks (TAB: 20 frames = 10 ticks).
const REACHED_EVERY: u64 = 10;

/// What `World::watch` remembers between ticks. Starts new with the round.
#[derive(Clone, Default)]
pub struct Watched {
    /// The zombies killed as of the last look.
    kills: u32,
    /// Which of your units (number) stood in which region (number) at the last look – sorted. `None`: not looked yet –
    /// the first look only notes who stands where, so units that start in a region don't "enter" it.
    inside: Option<Vec<(u32, u32)>>,
    /// The name IDs of the neutral things already reached – each fires once.
    reached: Vec<String>,
}

impl World {
    /// The triggers that are on and whose event `pick` finds in them, one value each – in the order of the list, each
    /// value once.
    fn wanted<T: PartialEq>(&self, pick: impl Fn(&TriggerEvent) -> Option<T>) -> Vec<T> {
        let mut found = Vec::new();
        for t in self.story.triggers.iter().filter(|t| t.on) {
            if let Some(v) = pick(&t.event)
                && !found.contains(&v)
            {
                found.push(v);
            }
        }
        found
    }

    /// Time, kills, regions, things reached: the events of these fire now. Once a tick, after everyone moved.
    pub(super) fn watch_triggers(&mut self) {
        if self.story.triggers.is_empty() {
            return;
        }
        self.watch_time();
        self.watch_kills();
        self.watch_regions();
        self.watch_reached();
    }

    fn watch_time(&mut self) {
        let tick = self.tick;
        for seconds in self.wanted(|e| if let TriggerEvent::Every { seconds } = e { Some(*seconds) } else { None }) {
            let every = seconds.max(1) as u64 * TICKS_PER_SECOND as u64;
            if tick > 0 && tick.is_multiple_of(every) {
                self.fire(&TriggerEvent::Every { seconds });
            }
        }
        if tick.is_multiple_of(HOUR_TICKS) {
            let (day, hour) = day_and_hour(tick);
            self.fire(&TriggerEvent::Clock { day: day as u32, hour: hour as u32 });
        }
    }

    fn watch_kills(&mut self) {
        let (before, now) = (self.watched.kills, self.score.zombies_killed);
        if now <= before {
            return;
        }
        self.watched.kills = now;
        let mut counts = self.wanted(|e| if let TriggerEvent::Kills { count } = e { Some(*count) } else { None });
        counts.sort_unstable();
        for count in counts.into_iter().filter(|&c| c > before && c <= now) {
            self.fire(&TriggerEvent::Kills { count });
        }
    }

    /// Your units on the ground, by tile against the regions the triggers ask about.
    fn watch_regions(&mut self) {
        let mut zones = self.wanted(|e| match e {
            TriggerEvent::Enters { zone } | TriggerEvent::Leaves { zone } => Some(*zone),
            _ => None,
        });
        zones.sort_unstable();
        if zones.is_empty() {
            self.watched.inside = None;
            return;
        }
        if !self.tick.is_multiple_of(REGION_EVERY) {
            return;
        }
        let mut now: Vec<(u32, u32)> = Vec::new();
        for &zone in &zones {
            let Some(z) = self.find_zone(zone) else {
                continue;
            };
            for unit in self.units.iter().filter(|u| !u.is_inside()) {
                if self.zones[z].contains((unit.pos.x.floor() as i32, unit.pos.y.floor() as i32)) {
                    now.push((zone, unit.id));
                }
            }
        }
        now.sort_unstable();
        let Some(before) = self.watched.inside.replace(now.clone()) else {
            return;
        };
        let changed = |from: &[(u32, u32)], to: &[(u32, u32)]| {
            let mut zones: Vec<u32> = from.iter().filter(|p| to.binary_search(p).is_err()).map(|p| p.0).collect();
            zones.dedup();
            zones
        };
        let entered = changed(&now, &before);
        let left = changed(&before, &now);
        for zone in zones {
            if entered.contains(&zone) {
                self.fire(&TriggerEvent::Enters { zone });
            }
            if left.contains(&zone) {
                self.fire(&TriggerEvent::Leaves { zone });
            }
        }
    }

    /// Neutral things with a name ID a trigger asks about: one of your units within a tile fires it, once.
    fn watch_reached(&mut self) {
        if !self.tick.is_multiple_of(REACHED_EVERY) {
            return;
        }
        let names = self.wanted(|e| if let TriggerEvent::Reached { name_id } = e { Some(name_id.clone()) } else { None });
        let names: Vec<String> = names.into_iter().filter(|n| !self.watched.reached.contains(n)).collect();
        if names.is_empty() {
            return;
        }
        let units: Vec<Vec2> = self.units.iter().filter(|u| !u.is_inside()).map(|u| u.pos).collect();
        let hit: Vec<String> = self
            .neutral
            .iter()
            .filter_map(|n| n.tag.as_ref().filter(|t| names.contains(&t.name_id)).map(|t| (n, t.name_id.clone())))
            .filter(|(n, _)| n.touched(units.iter().copied()))
            .map(|(_, name)| name)
            .collect();
        for name_id in names.into_iter().filter(|n| hit.contains(n)) {
            self.watched.reached.push(name_id.clone());
            self.fire(&TriggerEvent::Reached { name_id });
        }
    }

    /// The middle of a region or point (world point) – `None`: it is gone.
    pub(super) fn site_center(&self, site: Site) -> Option<Vec2> {
        match site {
            Site::Region(id) => self.zones.get(self.find_zone(id)?)?.center(),
            Site::Point(id) => self.spots.get(self.find_spot(id)?).map(|s| s.center()),
        }
    }

    /// What in the story names this region (`zone` true) or point: its triggers' events and actions, for "Used by"
    /// in the Regions module. Deleting a region takes it out of the zombies' groups only (`forget_zone`) – a trigger
    /// for a region that's gone never fires, and an action for it does nothing.
    pub fn story_uses(&self, zone: bool, id: u32) -> Vec<String> {
        let mut uses = Vec::new();
        for t in &self.story.triggers {
            let event = zone
                && matches!(t.event, TriggerEvent::Enters { zone: z } | TriggerEvent::Leaves { zone: z } if z == id);
            if event {
                uses.push(format!("Trigger \"{}\"", t.name));
            }
            if names_site(&t.actions, zone, id) {
                uses.push(format!("Trigger \"{}\", an action", t.name));
            }
        }
        uses
    }

    /// Does an action of this script name a region or point that doesn't exist (or none chosen yet)? For the editor's
    /// list of problems.
    pub fn names_missing_site(&self, script: &[Step]) -> bool {
        script.iter().any(|step| {
            let missing = match &step.action {
                Action::Camera { to } => self.site_center(*to).is_none(),
                Action::Noise { at, .. } => self.site_center(*at).is_none(),
                _ => false,
            };
            let mut k = 0;
            let mut inner = false;
            while let Some(child) = step.action.child(k) {
                inner |= self.names_missing_site(child);
                k += 1;
            }
            missing || inner
        })
    }

    /// Region `id` is gone: the zombies a trigger's action sends don't come from it any more.
    pub(super) fn forget_zone_in_story(&mut self, id: u32) {
        self.story.walk(&mut |part| {
            if let Part::Action(Action::Zombies { groups, .. }) = part {
                for group in groups {
                    if let WaveFrom::Zones(ids) = &mut group.from {
                        ids.retain(|&z| z != id);
                    }
                }
            }
        });
    }
}

/// Does a step of this script, or one inside it, name the region / point?
fn names_site(script: &[Step], zone: bool, id: u32) -> bool {
    script.iter().any(|step| {
        let (zones, spots) = step.action.sites();
        if (if zone { zones } else { spots }).contains(&id) {
            return true;
        }
        let mut k = 0;
        while let Some(child) = step.action.child(k) {
            if names_site(child, zone, id) {
                return true;
            }
            k += 1;
        }
        false
    })
}
