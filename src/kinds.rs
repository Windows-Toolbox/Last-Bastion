//! Unit and building types and their values (DESIGN.md 6, 7.2). In the prototype they are constants in the code –
//! later they come from JSON files in `data\` (DESIGN.md 12.4).

use serde::{Deserialize, Serialize};

/// The order here is also the order in the selection panel and for Tab.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum UnitKind {
    Soldier,
    Sniper,
    Medic,
}

pub struct Stats {
    pub name: &'static str,
    /// One or two letters for the icon in the selection panel.
    pub letter: &'static str,
    pub hp: u32,
    /// Subtracted from every hit (at least 1 damage always gets through).
    pub armor: u32,
    pub damage: u32,
    /// Range in tiles.
    pub range: f32,
    /// How far it sees, in tiles – at least as far as it shoots (forests and trees block the view).
    pub sight: f32,
    /// Ticks between two shots (20 ticks = 1 s).
    pub cooldown: u32,
    /// Tiles per second.
    pub speed: f32,
    /// Build time in a building, in ticks (20 ticks = 1 s).
    pub build_time: u32,
    /// Medic: doesn't shoot at zombies – its "shot" is a beam at your own units (DESIGN.md 6).
    pub heals: bool,
}

const SOLDIER: Stats = Stats {
    name: "Soldier",
    letter: "S",
    hp: 100,
    armor: 1,
    damage: 10,
    range: 6.0,
    sight: 10.0,
    cooldown: 10,
    speed: 3.0,
    build_time: 100,
    heals: false,
};

const SNIPER: Stats = Stats {
    name: "Sniper",
    letter: "Sn",
    hp: 60,
    armor: 0,
    damage: 35,
    range: 10.0,
    sight: 13.0,
    cooldown: 40,
    speed: 2.6,
    build_time: 160,
    heals: false,
};

/// Like StarCraft's medic: no attack. For now its beam is only shown – healing comes later.
const MEDIC: Stats = Stats {
    name: "Medic",
    letter: "M",
    hp: 60,
    armor: 1,
    damage: 0,
    range: 4.0,
    sight: 10.0,
    cooldown: 0, // the beam is on every tick, so it looks continuous
    speed: 2.9,
    build_time: 120,
    heals: true,
};

impl UnitKind {
    pub fn stats(self) -> &'static Stats {
        match self {
            UnitKind::Soldier => &SOLDIER,
            UnitKind::Sniper => &SNIPER,
            UnitKind::Medic => &MEDIC,
        }
    }
}

/// Buildings. The order doesn't matter, buildings are always selected one at a time.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum BuildingKind {
    CommandCenter,
    Tent,
    Wall,
    /// 3 × 1, can be turned: two posts and a door in the middle that lets your units through, but not zombies.
    Gate,
    /// 1 × 1 watchtower: up to `TOWER_SLOTS` units can go in and shoot from up there (DESIGN.md 6).
    Tower,
}

/// How many units fit into a tower (DESIGN.md 6: "can put in 4 ranged units").
pub const TOWER_SLOTS: usize = 4;
/// Units in a tower shoot and see this many tiles further – they stand higher up.
pub const TOWER_BONUS: f32 = 2.0;

pub struct BuildingStats {
    pub name: &'static str,
    pub letter: &'static str,
    /// Width (along x) and depth (along y) in tiles, not turned.
    pub size: (i32, i32),
    pub hp: u32,
    /// How tall the building is drawn (in tile widths).
    pub height: f32,
    /// How far it sees, in tiles, counted from its middle.
    pub sight: f32,
    /// Walls, gates and towers join up into one line on screen (only the look – `draw.rs`).
    pub joins: bool,
    /// Price, first guess. Only shown for now – nothing is paid until the economy comes (DESIGN.md 5), and which
    /// resources (wood, stone …) is decided then. Built over other buildings it gets cheaper (`placement.rs`).
    pub cost: u32,
}

const COMMAND_CENTER: BuildingStats = BuildingStats {
    name: "Command Center",
    letter: "CC",
    size: (3, 3),
    hp: 1500,
    height: 0.9,
    sight: 11.0,
    joins: false,
    cost: 0,
};

const TENT: BuildingStats = BuildingStats {
    name: "Tent",
    letter: "T",
    size: (2, 2),
    hp: 300,
    height: 0.45,
    sight: 5.0,
    joins: false,
    cost: 50,
};

const WALL: BuildingStats = BuildingStats {
    name: "Wall",
    letter: "W",
    size: (1, 1),
    hp: 400,
    height: 0.55,
    sight: 3.0,
    joins: true,
    cost: 10,
};

const GATE: BuildingStats = BuildingStats {
    name: "Gate",
    letter: "G",
    size: (3, 1),
    hp: 400,
    height: 0.6,
    sight: 3.0,
    joins: true,
    cost: 40,
};

/// 50 % more HP than a wall, and costs more (DESIGN.md 6).
const TOWER: BuildingStats = BuildingStats {
    name: "Tower",
    letter: "Tw",
    size: (1, 1),
    hp: 600,
    height: 1.2,
    sight: 6.0,
    joins: true,
    cost: 30,
};

impl BuildingKind {
    pub fn stats(self) -> &'static BuildingStats {
        match self {
            BuildingKind::CommandCenter => &COMMAND_CENTER,
            BuildingKind::Tent => &TENT,
            BuildingKind::Wall => &WALL,
            BuildingKind::Gate => &GATE,
            BuildingKind::Tower => &TOWER,
        }
    }

    /// Width and depth in tiles – turned, the two swap (only the gate isn't square).
    pub fn footprint(self, turned: bool) -> (i32, i32) {
        let (w, h) = self.stats().size;
        if turned { (h, w) } else { (w, h) }
    }
}
