//! Undo and redo in the map editor (Ctrl+Z / Ctrl+Y, DESIGN.md 12.9 step 1). What's remembered aren't copies of the
//! map but the **orders** of the last `STEPS` steps – plus one copy: the map before the oldest of them (`base`).
//!
//! Undo: the base, and every step but the last one carried out on it again. The simulation is deterministic (12.5),
//! so that gives exactly the map as it was – also where the zombie brush or the loot rolled dice: the dice (`Rng`) are
//! part of the copy. Redo: the step taken back, carried out once more.
//!
//! A step is everything from pressing the left mouse button to letting it go – a whole stroke of the brush, a number
//! dragged from 5 to 50 – or a single order (Del, a typed number). Only the editor's Edit mode keeps a history: a test
//! runs on a copy of the map anyway.

use std::collections::VecDeque;

use crate::sim::{Order, World};

/// How many steps undo goes back.
const STEPS: usize = 20;

/// One step: the order lists of the frames it took, in order – a stroke of the brush has many, a click one.
type Step = Vec<Vec<Order>>;

pub struct History {
    /// The map before the oldest step still remembered – `None` until the first change.
    base: Option<World>,
    /// The steps, the oldest first. `VecDeque`: a list that can also give up its first entry quickly (like Python's
    /// `collections.deque`).
    steps: VecDeque<Step>,
    /// What undo took back, the last one at the end – for redo.
    undone: Vec<Step>,
    /// Is the last step still going on (the mouse button still held)? Then the next orders belong to it.
    open: bool,
}

impl History {
    pub fn new() -> History {
        History { base: None, steps: VecDeque::new(), undone: Vec::new(), open: false }
    }

    /// Edit mode: carries out `orders` on `world` and remembers them. `going_on`: the mouse button is still held –
    /// the next orders belong to the same step.
    pub fn apply(&mut self, world: &mut World, orders: &[Order], going_on: bool) {
        // The very first change: the map as it is goes aside – a copy (`clone`).
        if self.base.is_none() {
            self.base = Some(world.clone());
        }
        edit(world, orders);
        // A new change: what undo took back can't come back any more (like in every editor).
        self.undone.clear();
        match self.steps.back_mut() {
            Some(step) if self.open => step.push(orders.to_vec()),
            _ => self.steps.push_back(vec![orders.to_vec()]),
        }
        self.open = going_on;
        // More than `STEPS`: the oldest step goes into the base and is forgotten.
        let oldest = if self.steps.len() > STEPS { self.steps.pop_front() } else { None };
        if let (Some(oldest), Some(base)) = (oldest, &mut self.base) {
            for orders in &oldest {
                edit(base, orders);
            }
        }
    }

    /// The mouse button was let go: the next orders start a new step.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Ctrl+Z: the last step undone. `false`: there is none.
    pub fn undo(&mut self, world: &mut World) -> bool {
        // `let … else`: if the pattern doesn't fit, the `else` block runs – and has to leave the function.
        let Some(base) = &self.base else {
            return false;
        };
        let Some(step) = self.steps.pop_back() else {
            return false;
        };
        let mut rebuilt = base.clone();
        // `flatten`: the order lists of all steps one after the other, as if it were one long list.
        for orders in self.steps.iter().flatten() {
            edit(&mut rebuilt, orders);
        }
        *world = rebuilt;
        self.undone.push(step);
        self.open = false;
        true
    }

    /// Ctrl+Y: the step undo took back last, once more. `false`: there is none.
    pub fn redo(&mut self, world: &mut World) -> bool {
        let Some(step) = self.undone.pop() else {
            return false;
        };
        for orders in &step {
            edit(world, orders);
        }
        self.steps.push_back(step);
        self.open = false;
        true
    }

    /// How many steps undo and redo can go right now.
    pub fn counts(&self) -> (usize, usize) {
        (self.steps.len(), self.undone.len())
    }
}

/// What Edit mode does with orders – the same for a change and when it's carried out again, so both give the same map.
fn edit(world: &mut World, orders: &[Order]) {
    world.apply_orders(orders);
    world.update_vision();
}

#[cfg(test)]
mod tests {
    use macroquad::math::vec2;

    use super::*;
    use crate::map::Terrain;
    use crate::mapfile::MapFile;
    use crate::sim::{Breed, Piece};

    /// When two maps count as the same here: what their files hold, and where everything is (`checksum`).
    fn state(world: &World) -> (MapFile, u64) {
        (MapFile::from_world(world, "Test"), world.checksum())
    }

    /// Undo goes back exactly, step by step – where the dice rolled too – and redo forward again. A step is everything
    /// while the mouse is held. Only `STEPS` steps back: the older ones went into the base.
    #[test]
    fn undo_and_redo_give_the_map_back_exactly() {
        let mut world = World::new(4);
        let mut history = History::new();
        let c = world.map.size as i32 / 2;
        let mut states = vec![state(&world)];
        for i in 0..STEPS as i32 + 5 {
            // Zombies at random spots (dice), then water under the mouse – both while it's held: one step.
            let zombies = Order::PlaceZombies { center: vec2((c + i) as f32, (c + 10) as f32), radius: 3.0, count: 5, breed: Breed::Walker, wander: true };
            history.apply(&mut world, &[zombies], true);
            history.apply(&mut world, &[Order::PaintTerrain { terrain: Terrain::Water, tiles: vec![(c + i, c - 12)] }], false);
            states.push(state(&world));
        }
        assert_eq!(history.counts(), (STEPS, 0));
        for back in 1..=STEPS {
            assert!(history.undo(&mut world));
            assert!(state(&world) == states[states.len() - 1 - back], "undo {back} isn't the map as it was");
        }
        assert!(!history.undo(&mut world), "undo went back further than {STEPS} steps");
        for forward in (0..STEPS).rev() {
            assert!(history.redo(&mut world));
            assert!(state(&world) == states[states.len() - 1 - forward], "redo isn't the map as it was");
        }
        assert!(!history.redo(&mut world));

        // A new change after undo: there's nothing to redo any more.
        history.undo(&mut world);
        history.apply(&mut world, &[Order::SetGold { gold: 5 }], false);
        assert_eq!(history.counts(), (STEPS, 0));
    }

    /// Paste and cut are orders like the others: undo takes the pasted things away again and gives the cut ones back.
    #[test]
    fn undo_takes_paste_and_cut_back() {
        let mut world = World::new(4);
        let mut history = History::new();
        let before = state(&world);
        let units: Vec<Piece> = world.units.iter().map(|u| Piece::Unit(u.id)).collect();
        let clip = world.copy(&units);
        let count = world.units.len();
        history.apply(&mut world, &[Order::Paste { clip: Box::new(clip.clone()), to: (clip.middle.0 + 3, clip.middle.1 + 3) }], false);
        assert!(world.units.len() > count, "nothing was pasted");
        assert!(history.undo(&mut world));
        assert!(state(&world) == before, "undo didn't take the paste back");
        history.apply(&mut world, &[Order::RemovePieces { pieces: units }], false);
        assert!(world.units.is_empty());
        assert!(history.undo(&mut world));
        assert!(state(&world) == before, "undo didn't give the cut ones back");
    }

    /// "Map size" is an order like any other: undo gives the map back as it was before, redo makes it as big again –
    /// exactly, the map loaded anew from its file comes out the same every time.
    #[test]
    fn undo_takes_a_new_map_size_back() {
        let mut world = World::new(4);
        let mut history = History::new();
        let before = state(&world);
        let n = world.map.size;
        history.apply(&mut world, &[Order::ResizeMap { size: n - 60 }], false);
        let smaller = state(&world);
        assert_eq!(world.map.size, n - 60);
        assert!(history.undo(&mut world));
        assert!(state(&world) == before, "undo isn't the map as it was");
        assert!(history.redo(&mut world));
        assert!(state(&world) == smaller, "redo isn't the smaller map");
    }
}
