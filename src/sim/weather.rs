//! The weather – snow, rain or clear, like TAB's map conditions (`ZXWeatherEffectType`: Snowy, Rainy). TAB lets it snow
//! on its frozen landscape and rain on its volcanic one; here that's only where a round starts: a script changes it
//! whenever it likes (`Action::Weather` – at the start, with an event, at random, by the player's choice). Only the
//! picture and the sound follow it (`effects/lasting.rs`, `audio.rs`) – the simulation never asks.

use serde::{Deserialize, Serialize};

use super::World;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Weather {
    #[default]
    Clear,
    /// TAB's `Snow_B` over the map.
    Snow,
    /// TAB's `Rain_A` over the map, and its rain sound.
    Rain,
}

impl Weather {
    pub const ALL: [Weather; 3] = [Weather::Clear, Weather::Snow, Weather::Rain];

    /// The weather a round on this landscape starts with – TAB's: snow on the frozen highlands (AL), rain on the
    /// caustic lands (VO), clear elsewhere.
    pub fn of_look(look: &str) -> Weather {
        match look {
            "AL" => Weather::Snow,
            "VO" => Weather::Rain,
            _ => Weather::Clear,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Weather::Clear => "Clear",
            Weather::Snow => "Snow",
            Weather::Rain => "Rain",
        }
    }
}

/// TAB's six landscapes a map can be drawn as (`World::look`), in TAB's order: the two letters of its pictures
/// (`assets\terrain\<look>\`) and TAB's name.
pub const LOOKS: [(&str, &str); 6] = [
    ("FA", "The Deep Forest"),
    ("BR", "The Dark Moorland"),
    ("TM", "The Peaceful Lowlands"),
    ("AL", "The Frozen Highlands"),
    ("DS", "The Desolated Wasteland"),
    ("VO", "Caustic Lands"),
];

impl World {
    /// The landscape the map is drawn as (`look`) – and with it the weather it starts with (`Weather::of_look`).
    pub fn set_look(&mut self, look: &str) {
        self.look = look.to_string();
        self.weather = Weather::of_look(look);
    }
}
