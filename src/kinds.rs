//! Unit and building types (DESIGN.md 6, 7.2). Their values are in `data\units.json` and `data\buildings.json`
//! (loaded by `data.rs`) – here only what a value means.

use serde::{Deserialize, Serialize};

use crate::data;

/// The order here is also the order in the selection panel and for Tab.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum UnitKind {
    /// Tough, short range – in front in a formation.
    Soldier,
    /// Like TAB's ranger: fast, weak, long wind-up (drawing the bow) before every arrow.
    Ranger,
    Sniper,
    /// Like StarCraft's medic: no attack. For now its beam is only shown – healing comes later.
    Medic,
}

/// The values of a unit type – one block in `data\units.json`.
#[derive(Deserialize)]
pub struct Stats {
    pub name: String,
    /// One or two letters for the icon in the selection panel.
    pub letter: String,
    /// Gold, paid when it goes into the production queue – back in full if it's cancelled (like in StarCraft).
    pub cost: u32,
    /// Workers it takes from the colony (`World::workers`) – like its gold when it goes into the production queue, but
    /// they come back when it dies (or is cancelled). DESIGN.md 5.6.
    pub worker_cost: u32,
    pub hp: u32,
    /// Subtracted from every hit (at least 1 damage always gets through).
    pub armor: u32,
    pub damage: u32,
    /// Range in tiles.
    pub range: f32,
    /// How far it sees, in tiles – at least as far as it shoots (forests and trees block the view).
    pub sight: f32,
    /// Ticks for turning around once (180°) – like StarCraft's turn rate. A unit only aims once it faces its target.
    /// In the file in seconds (`turn_around_seconds`).
    #[serde(rename = "turn_around_seconds", deserialize_with = "data::seconds")]
    pub turn_around: u32,
    /// Ticks of wind-up before every shot (drawing the bow, raising the rifle) – like StarCraft's "damage point".
    /// Facing the target, the unit aims this long, then it shoots. Another target: aiming starts over.
    /// In the file in seconds (`aim_seconds`).
    #[serde(rename = "aim_seconds", deserialize_with = "data::seconds")]
    pub aim: u32,
    /// Ticks of reloading after a shot, before it can aim again. One shot takes `aim + cooldown`. In the file in
    /// seconds (`cooldown_seconds`); 0 for the medic – its beam is on every tick, so it looks continuous.
    #[serde(rename = "cooldown_seconds", deserialize_with = "data::seconds")]
    pub cooldown: u32,
    /// Tiles per second.
    pub speed: f32,
    /// Build time in a building, in ticks. In the file in seconds (`build_seconds`).
    #[serde(rename = "build_seconds", deserialize_with = "data::seconds")]
    pub build_time: u32,
    /// How loud a shot is, like in TAB: every zombie this many tiles around the shooter hears it and comes
    /// (`noise_seconds` in `data\zombies.json`). Ranger quiet, sniper loud. Not blocked by forests or walls.
    pub noise: f32,
    /// Medic: doesn't shoot at zombies – its "shot" is a beam at your own units (DESIGN.md 6).
    pub heals: bool,
    /// Its picture: a sprite in `assets\` (`sprites.rs`), e.g. `"test/figure"`. Missing: a box.
    pub sprite: Option<String>,
}

impl UnitKind {
    pub fn stats(self) -> &'static Stats {
        data::get().unit(self)
    }

    /// All types, in the order of the selection panel.
    pub const ALL: [UnitKind; 4] = [UnitKind::Soldier, UnitKind::Ranger, UnitKind::Sniper, UnitKind::Medic];
}

impl Stats {
    /// Ticks from the start of aiming to the next start of aiming – one shot.
    pub fn shot_ticks(&self) -> u32 {
        self.aim + self.cooldown
    }
}

/// Buildings. The order here is the order in the selection panel and for Tab (after the units).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum BuildingKind {
    CommandCenter,
    /// Trains your units – the only building that does (DESIGN.md 6, "Military").
    Barracks,
    Tent,
    Wall,
    /// 3 × 1, can be turned: two posts and a door in the middle that lets your units through, but not zombies.
    Gate,
    /// 1 × 1 watchtower: up to `tower_slots` units can go in and shoot from up there (DESIGN.md 6).
    Tower,
}

/// The values of a building type – one block in `data\buildings.json`.
#[derive(Deserialize)]
pub struct BuildingStats {
    pub name: String,
    pub letter: String,
    /// Width (along x) and depth (along y) in tiles, not turned.
    pub size: (i32, i32),
    pub hp: u32,
    /// Infection like in TAB (DESIGN.md 6): zombies first hit the shield. When it breaks, the building is infected –
    /// its colonists come out as zombies – and only then do the hit points go down. Given in percent of `hp`
    /// (`max_shield`). 0: no shield. Without colonists a shield is just extra life – nobody can get infected.
    pub shield_percent: u32,
    /// How many colonists live in it. Each pays `gold_per_colonist_per_hour` (`data\colony.json`) – until the
    /// building is infected.
    pub colonists: u32,
    /// Workers it gives the colony, like in TAB – the houses and the command center (`World::workers`). Infected: none.
    pub workers: u32,
    /// Workers it takes from the colony for as long as it stands. Not enough free workers: it can't be built
    /// (DESIGN.md 5.6).
    pub worker_cost: u32,
    /// Price in gold, paid when it's placed (first guess). Wood, stone … come with the economy (DESIGN.md 5). Built
    /// over other buildings it gets cheaper (`placement.rs`). The editor places buildings for free.
    pub cost: u32,
    /// Gold for a full repair – from nothing to full hit points and shield; a smaller repair pays its share of it.
    /// Paid when the repair starts. Repairing an infected building cures it (DESIGN.md 6). Once buildings cost wood
    /// and stone, repairs take their share of those too.
    pub repair_cost: u32,
    /// How far it sees, in tiles, counted from its middle.
    pub sight: f32,
    /// How tall the building is drawn (in tile widths) – only the look.
    pub height: f32,
    /// Walls, gates and towers join up into one line on screen (only the look – `draw.rs`).
    pub joins: bool,
}

impl BuildingStats {
    /// The full shield in points: `shield_percent` of the hit points.
    pub fn max_shield(&self) -> u32 {
        self.hp * self.shield_percent / 100
    }
}

impl BuildingKind {
    pub fn stats(self) -> &'static BuildingStats {
        data::get().building(self)
    }

    /// Does it train units (a production queue and a rally point)? Only the barracks.
    pub fn trains(self) -> bool {
        self == BuildingKind::Barracks
    }

    /// Width and depth in tiles – turned, the two swap (only the gate isn't square).
    pub fn footprint(self, turned: bool) -> (i32, i32) {
        let (w, h) = self.stats().size;
        if turned { (h, w) } else { (w, h) }
    }
}
