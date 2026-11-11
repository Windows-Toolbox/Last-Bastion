//! What happened in a tick that only the picture shows – shots, deaths, explosions, buildings falling (`effects.rs`
//! draws TAB's effects for them). The simulation only writes this list, it never reads it: it's emptied at the start of
//! every tick, the game takes it after each tick (`game.rs`). Without a picture (tests, the headless round) nobody
//! takes it – it's just emptied again.

use macroquad::math::Vec2;

use crate::kinds::{BuildingKind, UnitKind};

use super::doom::Nest;

/// One thing that happened.
#[derive(Clone, Copy, Debug)]
pub enum Fx {
    /// A unit shot from `from` at `to` – `bleeds`: at a zombie or one of your units (blood where it hits). `flight`: ticks its arrow flies until it
    /// lands there (`arrows.rs`); 0: a bullet, it hits at once.
    UnitShot { from: Vec2, to: Vec2, kind: UnitKind, bleeds: bool, flight: u32 },
    /// A tower that shoots by itself (`towers.rs`) shot at a zombie at `to`.
    TowerShot { building: u32, kind: BuildingKind, from: Vec2, to: Vec2 },
    /// A zombie died at `at`.
    ZombieDied { at: Vec2 },
    /// A unit of yours died at `at`.
    UnitDied { at: Vec2 },
    /// Acid landed at `at` (`ranged.rs`).
    Acid { at: Vec2 },
    /// Something blew up at `at` with this `radius` (tiles): a barrel or a mine (`blast.rs`), a missile, a shell
    /// (`areas.rs`).
    Blast { at: Vec2, radius: f32 },
    /// Lucifer's flame from `from` to the end of its cone (`areas.rs`).
    Flame { from: Vec2, to: Vec2 },
    /// The shocking tower `building` let go at the zombie at `to` (one ray per zombie).
    Shock { building: u32, from: Vec2, to: Vec2 },
    /// A building or a house of doom fell (or was torn down): what, its top left tile and its size.
    Fell { what: Fallen, tile: (i32, i32), size: (i32, i32) },
    /// A building was infected.
    Infected { tile: (i32, i32), size: (i32, i32) },
}

/// What fell – each looks different when it does (TAB's `ExplosionOnDestroy`).
#[derive(Clone, Copy, Debug)]
pub enum Fallen {
    Building(BuildingKind),
    Doom(Nest),
}
