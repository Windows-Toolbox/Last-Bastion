//! The fallen: killed zombies dying, dead units lying there – and how a blow throws them, like TAB.
//!
//! **The push (TAB's `FactorPush`):** a hit that *kills* throws the body away from whoever struck (`ZXCommand`:
//! `!cLife.IsAlive` – the living aren't pushed). TAB gives the body the force `FactorPush` for one step of its physics
//! (12 a second): `push / mass / 12` tiles per second. The giant's blow has 150 – a walker (mass 1) flies off at 12.5
//! tiles per second, a fat one (6) hardly moves. Dead, TAB's body collides with nothing any more (`CLife`: its masks
//! go to 0): it flies through walls and whoever stands there. How fast it slows down is TAB's physics library's
//! damping (in its own DLL, not readable) – ours: evenly, until it lies still after `push_seconds`.
//!
//! **Dying (TAB's `CheckDeath`):** a killed zombie plays its death for `die_seconds` (TAB's `TimeAniDie`), then it's
//! gone – it lies in `World::dying` meanwhile, only for the picture. A dead unit lies in `World::corpses` until it
//! rises (`spawning.rs`).

use macroquad::math::Vec2;

use super::fx::Fx;
use super::zombies::{values, Breed};
use super::{World, TICKS_PER_SECOND};

/// TAB's physics steps per second – its force works for one of them.
const PHYSICS_STEPS: f32 = 12.0;

/// How a body flies after the blow that killed it. Default: it doesn't.
#[derive(Clone, Copy, Default, Debug)]
pub struct Fling {
    /// How far it goes in its first tick (tiles) – every tick after a bit less.
    step: Vec2,
    /// Ticks it still flies.
    left: u32,
}

impl Fling {
    /// Struck at `to` from `from` with the push `push` (TAB's `FactorPush`), weighing `mass`: away from `from`. Right on
    /// top of each other: no way to fly – it stays.
    pub(super) fn new(from: Vec2, to: Vec2, push: f32, mass: f32) -> Fling {
        let away = (to - from).normalize_or_zero();
        let speed = push / mass.max(0.01) / PHYSICS_STEPS; // tiles per second
        Fling { step: away * speed / TICKS_PER_SECOND, left: values().push }
    }

    /// One tick on from `pos`: where the body is now. It slows evenly – the last tick a `1 / push` share of the first.
    fn fly(&mut self, pos: Vec2) -> Vec2 {
        if self.left == 0 {
            return pos;
        }
        let share = self.left as f32 / values().push.max(1) as f32;
        self.left -= 1;
        pos + self.step * share
    }
}

/// A killed zombie, dying – only for the picture: nobody sees it, hits it or walks round it.
#[derive(Clone)]
pub struct Dying {
    pub pos: Vec2,
    /// Where it was the tick before – drawn in between.
    pub prev: Vec2,
    /// Its number while it lived – the picture remembers which way it faced by it.
    pub id: u32,
    pub breed: Breed,
    /// Its skin, as it wore it (`Zombie::look`).
    pub look: u8,
    /// The tick it died.
    pub died: u64,
    pub(super) fling: Fling,
}

impl World {
    /// The dead slide on (those a blow threw), dying zombies whose death is over are taken away. Inside the playing
    /// field – a body never flies out of it.
    pub(super) fn move_fallen(&mut self) {
        let play = self.map.play_area();
        let inside = |p: Vec2| if play.contains(p) { p } else { play.clamp(p, 0.5) };
        for corpse in &mut self.corpses {
            corpse.prev = corpse.pos;
            corpse.pos = inside(corpse.fling.fly(corpse.pos));
        }
        for body in &mut self.dying {
            body.prev = body.pos;
            body.pos = inside(body.fling.fly(body.pos));
        }
        let (tick, die) = (self.tick, values().die as u64);
        self.dying.retain(|d| tick < d.died + die);
    }

    /// The zombies killed this tick start dying (`remove_dead`, before they leave the list).
    pub(super) fn start_dying(&mut self) {
        let tick = self.tick;
        let killed = self.zombies.iter().filter(|z| !z.is_alive());
        let bodies = killed.map(|z| Dying { pos: z.pos, prev: z.pos, id: z.id, breed: z.breed, look: z.look, died: tick, fling: z.fling });
        self.dying.extend(bodies);
        let deaths = self.zombies.iter().filter(|z| !z.is_alive()).map(|z| Fx::ZombieDied { at: z.pos });
        self.fx.extend(deaths);
    }
}
