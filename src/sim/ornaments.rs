//! Ornaments: things on the map that are only there to be looked at – TAB's treasure ruins round its loot
//! (`ZXLevelGenerator`: `TemplateRuinTreasure`) and its power line towers in the forest (`ZXMapTheme.Decorate`). The
//! generator puts them down (`mapgen/tab.rs`), the map file keeps them, `draw.rs` draws them. Nothing in the
//! simulation reads them – they block nobody, like TAB's (`CFlatMode`).

use serde::{Deserialize, Serialize};

/// What an ornament is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrnamentKind {
    /// The ruin a pile of loot lies in – 4 × 4 tiles, in the look of the landscape.
    Ruin,
    /// A high power line tower in the middle of a 5 × 5 forest, and the same mirrored.
    PowerTower,
    PowerTowerFlip,
}

/// One ornament: what, and its tile – the top left of a ruin's 4 × 4 tiles, the middle tile of a tower.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ornament {
    pub kind: OrnamentKind,
    pub tile: (i32, i32),
}
