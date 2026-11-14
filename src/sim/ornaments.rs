//! Ornaments: things on the map that are only there to be looked at – TAB's treasure ruins round its loot
//! (`ZXLevelGenerator`: `TemplateRuinTreasure`), the power line towers and burning trucks at its supplies of energy
//! and food (also `ZXLevelGenerator`), its power line towers in the forest (`ZXMapTheme.Decorate`) and its
//! railway (`RailWay…`, `Railway…` in `ZXGame.dxprj`). The generator puts the first two down (`mapgen/tab.rs`), the
//! editor the railway; the map file keeps them, `draw.rs` draws them. They block nobody, like TAB's (`CFlatMode`) –
//! only the pieces of track mean something: trains run on them (`rails.rs`, `trains.rs`), and nothing may be built on
//! or next to them (`placement.rs`).

use serde::{Deserialize, Serialize};

use super::rails::RailShape;

/// What an ornament is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrnamentKind {
    /// The ruin a pile of loot lies in – 3 × 3 tiles, in the look of the landscape.
    Ruin,
    /// A high power line tower in the middle of a 5 × 5 forest, and the same mirrored.
    PowerTower,
    PowerTowerFlip,
    /// A medium power line tower (2 × 2) at an energy site, and the same mirrored (TAB's `TensionTowerMedium…`).
    PowerTowerMedium,
    PowerTowerMediumFlip,
    /// The burning truck of a food site (TAB's `TruckA`, 2 × 4) – its fire and smoke: `effects/lasting.rs`.
    Truck,
    /// A piece of track (TAB's `CRailWay`): straight, a curve of 6 × 6 tiles or a buffer stop.
    Rail(RailShape),
    /// The rest of TAB's railway, only the look: the arches of a railway bridge (8 × 1, 1 × 8, 1 × 4 tiles), two
    /// barriers, a level crossing, a trolley, two mine wagons, a lamp.
    BridgeA,
    BridgeB,
    BridgeC,
    BarrierA,
    BarrierB,
    Crossing,
    Trolley,
    MineWagonA,
    MineWagonB,
    Lamp,
}

impl OrnamentKind {
    /// The railway's ornaments, in the order of the editor's buttons: the track first.
    pub const RAILWAY: [OrnamentKind; 20] = [
        OrnamentKind::Rail(RailShape::X),
        OrnamentKind::Rail(RailShape::Y),
        OrnamentKind::Rail(RailShape::CurveNE),
        OrnamentKind::Rail(RailShape::CurveNW),
        OrnamentKind::Rail(RailShape::CurveSE),
        OrnamentKind::Rail(RailShape::CurveSW),
        OrnamentKind::Rail(RailShape::EndN),
        OrnamentKind::Rail(RailShape::EndS),
        OrnamentKind::Rail(RailShape::EndE),
        OrnamentKind::Rail(RailShape::EndW),
        OrnamentKind::BridgeA,
        OrnamentKind::BridgeB,
        OrnamentKind::BridgeC,
        OrnamentKind::BarrierA,
        OrnamentKind::BarrierB,
        OrnamentKind::Crossing,
        OrnamentKind::Trolley,
        OrnamentKind::MineWagonA,
        OrnamentKind::MineWagonB,
        OrnamentKind::Lamp,
    ];

    /// What lies at TAB's loot sites, in the order of the editor's buttons.
    pub const SITES: [OrnamentKind; 4] =
        [OrnamentKind::Ruin, OrnamentKind::PowerTowerMedium, OrnamentKind::PowerTowerMediumFlip, OrnamentKind::Truck];

    /// How many tiles it covers, from its `tile` on (x, y).
    pub fn footprint(self) -> (i32, i32) {
        match self {
            OrnamentKind::Ruin => (3, 3),
            OrnamentKind::PowerTowerMedium | OrnamentKind::PowerTowerMediumFlip => (2, 2),
            OrnamentKind::Truck => (2, 4),
            OrnamentKind::Rail(shape) => (shape.size(), shape.size()),
            OrnamentKind::BridgeA => (8, 1),
            OrnamentKind::BridgeB => (1, 8),
            OrnamentKind::BridgeC => (1, 4),
            _ => (1, 1),
        }
    }

    /// Its picture in `assets\decor\` – `None` for the ruin, whose picture depends on the landscape.
    pub fn picture(self) -> Option<&'static str> {
        Some(match self {
            OrnamentKind::Ruin => return None,
            OrnamentKind::PowerTower => "power_tower_high",
            OrnamentKind::PowerTowerFlip => "power_tower_high_flip",
            OrnamentKind::PowerTowerMedium => "power_tower_medium",
            OrnamentKind::PowerTowerMediumFlip => "power_tower_medium_flip",
            OrnamentKind::Truck => "truck",
            OrnamentKind::Rail(shape) => shape.picture(),
            OrnamentKind::BridgeA => "rail_arch_a",
            OrnamentKind::BridgeB => "rail_arch_b",
            OrnamentKind::BridgeC => "rail_arch_c",
            OrnamentKind::BarrierA => "rail_barrier_a",
            OrnamentKind::BarrierB => "rail_barrier_b",
            OrnamentKind::Crossing => "rail_crossing",
            OrnamentKind::Trolley => "rail_trolley",
            OrnamentKind::MineWagonA => "mine_wagon_a",
            OrnamentKind::MineWagonB => "mine_wagon_b",
            OrnamentKind::Lamp => "train_lamp",
        })
    }

    /// Lying flat on the ground (drawn with it, under everyone): the ruin, the track, the level crossing. The rest
    /// stands up and is sorted in among the figures.
    pub fn is_flat(self) -> bool {
        matches!(self, OrnamentKind::Ruin | OrnamentKind::Rail(_) | OrnamentKind::Crossing)
    }

    /// Its name on the editor's button – with which way it lies on screen where that isn't clear from the name:
    /// `\` along x (down to the right), `/` along y (down to the left).
    pub fn name(self) -> &'static str {
        match self {
            OrnamentKind::Ruin => "Ruin",
            OrnamentKind::PowerTower | OrnamentKind::PowerTowerFlip => "Power tower",
            OrnamentKind::PowerTowerMedium => "Power tower",
            OrnamentKind::PowerTowerMediumFlip => "Power tower (mirrored)",
            OrnamentKind::Truck => "Burning truck",
            OrnamentKind::Rail(shape) => shape.name(),
            OrnamentKind::BridgeA => "Bridge 8x1",
            OrnamentKind::BridgeB => "Bridge 1x8",
            OrnamentKind::BridgeC => "Bridge 1x4",
            OrnamentKind::BarrierA => "Barrier /",
            OrnamentKind::BarrierB => "Barrier \\",
            OrnamentKind::Crossing => "Crossing",
            OrnamentKind::Trolley => "Trolley /",
            OrnamentKind::MineWagonA => "Mine wagon /",
            OrnamentKind::MineWagonB => "Mine wagon \\",
            OrnamentKind::Lamp => "Lamp",
        }
    }
}

/// One ornament: what, and its tile – the top left of what it covers (`OrnamentKind::footprint`); a power tower's is
/// the middle tile of its forest.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ornament {
    pub kind: OrnamentKind,
    pub tile: (i32, i32),
}

impl Ornament {
    /// Does it cover this tile?
    pub fn covers(&self, (x, y): (i32, i32)) -> bool {
        let (w, h) = self.kind.footprint();
        (self.tile.0..self.tile.0 + w).contains(&x) && (self.tile.1..self.tile.1 + h).contains(&y)
    }
}
