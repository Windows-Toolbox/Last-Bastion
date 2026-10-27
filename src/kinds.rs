//! Unit and building types (DESIGN.md 6, 7.2). Their values are in `data\units.json` and `data\buildings.json`
//! (loaded by `data.rs`) – here only what a value means.

use serde::{Deserialize, Serialize};

use crate::data;
use crate::map::Ore;

/// The order here is also the order in the selection panel and for Tab.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum UnitKind {
    /// Tough, short range – in front in a formation.
    Soldier,
    /// Like TAB's ranger: fast, weak, long wind-up (drawing the bow) before every arrow.
    Ranger,
    Sniper,
    /// Like StarCraft's medic: no attack – it heals with its beam, but only units that are out of the fight.
    Medic,
    /// Early melee: a spear, a push back with every hit, nearly silent – holds a line, but dies to spitters and infested.
    Spearman,
    /// Mercenaries from the inn (`trained_in`): stronger than a veteran, dearer, double upkeep, no workers – they
    /// aren't colonists. They fight for gold: in debt they are the first to go (`sim/debt.rs`).
    MercSoldier,
    MercRanger,
    MercSniper,
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
    /// Gold every payout for as long as it lives, like in TAB (DESIGN.md 7.2) – not yet in the production queue. Not
    /// enough gold: debt (`sim/debt.rs`).
    #[serde(default)]
    pub upkeep: u32,
    /// Food every payout for as long as it lives, like in TAB (the soldier 1) – eaten before the houses
    /// (`economy_payout`). Left out: 0.
    #[serde(default)]
    pub food: u32,
    pub hp: u32,
    /// Subtracted from every hit (at least 1 damage always gets through).
    pub armor: u32,
    pub damage: u32,
    /// Range in tiles.
    pub range: f32,
    /// How far it sees, in tiles – at least as far as it shoots (forest blocks the view).
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
    /// Only while one of these stands can it be trained – firearms need the munitions factory. Written as the type's
    /// name in the file (`"MunitionsFactory"`), left out: none.
    #[serde(default)]
    pub requires: Option<BuildingKind>,
    /// Where it's trained: `"Barracks"` (left out) or `"Inn"` – the mercenaries.
    #[serde(default = "barracks")]
    pub trained_in: BuildingKind,
    /// A mercenary: works for gold, not for the colony – in debt it deserts first, already deep in debt
    /// (`sim/debt.rs`). Drawn with a gold mark.
    #[serde(default)]
    pub mercenary: bool,
    /// Build time in a building, in ticks. In the file in seconds (`build_seconds`).
    #[serde(rename = "build_seconds", deserialize_with = "data::seconds")]
    pub build_time: u32,
    /// How loud a shot is, like in TAB: this much noise where it's fired – who hears it and comes: `data\sound.json`
    /// (`sim/sound.rs`). Ranger quiet, sniper loud. Not blocked by forests or walls.
    pub noise: f32,
    /// Medic: doesn't shoot at zombies – its "shot" is a beam at your own units (DESIGN.md 6) …
    pub heals: bool,
    /// … that heals this many hit points per second …
    #[serde(default)]
    pub heal_per_second: f32,
    /// … but only a unit that hasn't been hit for this long – no healing in the thick of a fight, not like StarCraft's
    /// medic (read in seconds, kept in ticks).
    #[serde(default, rename = "heal_after_seconds", deserialize_with = "data::seconds")]
    pub heal_after: u32,
    /// Melee (the spearman): every hit pushes the zombie back this many tiles – a big one less (`combat.rs`). 0: none.
    #[serde(default)]
    pub knockback: f32,
    /// Its picture: a sprite in `assets\` (`sprites.rs`), e.g. `"test/figure"`. Missing: a box.
    pub sprite: Option<String>,
}

impl UnitKind {
    pub fn stats(self) -> &'static Stats {
        data::get().unit(self)
    }

    /// All types, in the order of the selection panel.
    pub const ALL: [UnitKind; 8] = [
        UnitKind::Soldier,
        UnitKind::Ranger,
        UnitKind::Sniper,
        UnitKind::Medic,
        UnitKind::Spearman,
        UnitKind::MercSoldier,
        UnitKind::MercRanger,
        UnitKind::MercSniper,
    ];
}

/// `trained_in` left out: the barracks.
fn barracks() -> BuildingKind {
    BuildingKind::Barracks
}

impl Stats {
    /// Ticks from the start of aiming to the next start of aiming – one shot.
    pub fn shot_ticks(&self) -> u32 {
        self.aim + self.cooldown
    }
}

/// Buildings. The order here is the order in the selection panel and for Tab (after the units).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
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
    /// Wood from the forest around it (`Source::Forest`).
    Sawmill,
    /// Iron ore, coal and stone from the ore patches under and around it (`Source::Ore`) – one building for all three.
    Mine,
    /// Food from the game in the forest around it.
    HunterCottage,
    /// Food from the water around it.
    FishingHut,
    /// Food from the fields it plants on the grass around it – like TAB's farm. Only on grass, not on dirt.
    Farm,
    /// Power without fuel – a little.
    Windmill,
    /// Power from burning wood: a lot, but it eats wood every hour (DESIGN.md's wood generator).
    CharcoalBurner,
    /// Makes nothing: as long as one stands, the buildings that `require` it may be built (like in TAB).
    WoodWorkshop,
    /// Carries the power further: its own `power_radius`, joined to the grid it stands in (DESIGN.md 5.5).
    TeslaTower,
    /// Like the wood workshop, for stone: makes nothing, unlocks the stone buildings and the munitions factory.
    StoneWorkshop,
    /// Makes nothing either (ammunition is free, DESIGN.md question 26): while one stands, firearms can be trained.
    MunitionsFactory,
    /// Like TAB's foundry a tech building – while one stands, the iron tier can be built (the machine gun tower) – and
    /// while it runs it smelts iron ore and coal into iron (`smelts`, DESIGN.md decision 32).
    Foundry,
    /// The second house (DESIGN.md 5.3): more colonists and workers, eats more. Goes on a tent – an upgrade.
    StoneHouse,
    /// A stronger wall – goes on a wall too.
    StoneWall,
    /// A stronger tower, the same garrison – goes on a tower too.
    StoneTower,
    /// Shoots by itself (`weapon`), early like TAB's: a heavy bolt, slow, at the toughest zombie in range.
    Ballista,
    /// Shoots by itself: fast fire at the nearest – against crowds. Costs iron, needs power.
    MachineGunTower,
    /// TAB's stake trap: zombies walk over it (they don't attack it) and get hurt; every hit wears it down.
    Stakes,
    /// Houses around it pay more gold, and it sells what the stock has too much of (`sim/market.rs`).
    Market,
    /// More room for gold – and credit (Dispo): with Shift you can buy below 0 (`sim/debt.rs`).
    Bank,
    /// Trains the mercenaries (`UnitKind::MercSoldier` …).
    Inn,
    /// An explosive barrel standing on the ground (`sim/blast.rs`): any damage and it goes off. Made in the
    /// munitions factory, set from the defence menu or put down by a unit carrying one.
    Barrel,
}

/// The goods in the colony's stock (DESIGN.md 5.1) – gold is kept apart (`World::gold`). Oil comes with the oil pump.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resource {
    Wood,
    Stone,
    /// Iron ore – out of the mines. The foundry makes iron of it.
    IronOre,
    Coal,
    Food,
    /// Iron – smelted in the foundry from iron ore and coal (`smelts` in `data\buildings.json`). The second tier of
    /// defence costs it: the machine gun tower.
    Iron,
}

impl Resource {
    /// How many there are – the size of the lists with one entry per good (`Stock`).
    pub const COUNT: usize = 6;
    pub const ALL: [Resource; Resource::COUNT] =
        [Resource::Wood, Resource::Stone, Resource::IronOre, Resource::Coal, Resource::Food, Resource::Iron];

    /// Its place in `Stock`.
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Resource::Wood => "Wood",
            Resource::Stone => "Stone",
            Resource::IronOre => "Iron ore",
            Resource::Coal => "Coal",
            Resource::Food => "Food",
            Resource::Iron => "Iron",
        }
    }

    /// What a mine gets out of a tile with this ore – oil isn't mined, gold not yet (TAB's maps have it, DESIGN.md 5).
    pub fn from_ore(ore: Ore) -> Option<Resource> {
        match ore {
            Ore::Iron => Some(Resource::IronOre),
            Ore::Coal => Some(Resource::Coal),
            Ore::Stone => Some(Resource::Stone),
            Ore::Oil | Ore::Gold => None,
        }
    }
}

/// A rectangle of tiles: top left, bottom right – the bottom right one just outside (`BuildingKind::around`).
pub type Area = ((i32, i32), (i32, i32));

/// Does the tile lie in the area?
pub fn in_area(((x0, y0), (x1, y1)): Area, (x, y): (i32, i32)) -> bool {
    x >= x0 && y >= y0 && x < x1 && y < y1
}

/// What a gathering building works – the tiles of its `gather_area`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    /// Forest – a single tile of it too.
    Forest,
    Water,
    /// Grass without ore on it – the fields of a farm (`Terrain::is_fertile`).
    Fertile,
    /// Ore patches (not oil) – each tile gives its own ore.
    Ore,
}

/// The values of a building type – one block in `data\buildings.json`.
#[derive(Deserialize)]
pub struct BuildingStats {
    pub name: String,
    pub letter: String,
    /// Width (along x) and depth (along y) in tiles, not turned.
    pub size: (i32, i32),
    pub hp: u32,
    /// Subtracted from every zombie hit (at least 1 gets through) – the command center's 10, like in TAB.
    #[serde(default)]
    pub armor: u32,
    /// Infection like in TAB (DESIGN.md 6): zombies first hit the shield. When it breaks, the building is infected –
    /// its people come out as zombies (`World::infect`) – and only then do the hit points go down. Given in percent of `hp`
    /// (`max_shield`). 0: no shield. Without colonists – and on the command center – a shield is just extra life:
    /// nobody can get infected.
    pub shield_percent: u32,
    /// How many colonists live in it. They pay `gold` – until the building is infected, then they come out as zombies.
    pub colonists: u32,
    /// Infected, this many zombies come out on top of its people (TAB's `ExtraUnitsWhenInfected`, `World::infect`).
    #[serde(default)]
    pub extra_infected: u32,
    /// Workers it gives the colony, like in TAB – the houses and the command center (`World::workers`). Infected: none.
    pub workers: u32,
    /// Workers it takes from the colony for as long as it stands. Not enough free workers: it can't be built
    /// (DESIGN.md 5.6).
    pub worker_cost: u32,
    /// Price in gold, paid when it's placed (first guess). Built over other buildings it gets cheaper
    /// (`placement.rs`). The editor places buildings for free.
    pub cost: u32,
    /// The rest of the price: wood and stone from the stock, paid with the gold. `default`: 0 if it's not in the file.
    #[serde(default)]
    pub cost_wood: u32,
    #[serde(default)]
    pub cost_stone: u32,
    #[serde(default)]
    pub cost_coal: u32,
    /// Iron from the foundry (`Resource::Iron`) – the second tier of defence costs it.
    #[serde(default)]
    pub cost_iron: u32,
    /// Only while one of these stands can it be built – like TAB's wood workshop. Written as the type's name in the
    /// file (`"WoodWorkshop"`).
    #[serde(default)]
    pub requires: Option<BuildingKind>,
    /// Power it makes for its grid (DESIGN.md 5.5) …
    #[serde(default)]
    pub energy: u32,
    /// … and power it takes from it. Not enough in the grid: it works that much slower (`World::power`).
    #[serde(default)]
    pub energy_use: u32,
    /// The command center and tesla towers: how far their power reaches (tiles, from the middle). Buildings that make,
    /// take or carry power have to stand inside it.
    #[serde(default)]
    pub power_radius: f32,
    /// Wood it burns every payout – without it, it makes no power (the charcoal burner).
    #[serde(default)]
    pub burns_wood: u32,
    /// Food its colonists eat every payout – without it, they pay no gold (houses).
    #[serde(default)]
    pub food: u32,
    /// Gold it pays every payout (`economy_every_hours`) at full power – houses only when their colonists are fed
    /// (TAB: tent 8, stone house 40, command center 200 for nothing).
    #[serde(default)]
    pub gold: u32,
    /// Food it brings in every payout all by itself, without tiles – the command center (TAB's +20 food).
    #[serde(default)]
    pub food_made: u32,
    /// Gold it costs every payout while it runs (TAB: sawmill 4, farm 20 …) – switched off or infected: nothing. Not
    /// enough gold: debt (`sim/debt.rs`).
    #[serde(default)]
    pub upkeep: u32,
    /// Gathering buildings: how many tiles around their site they work, on every side (like TAB: a 2 × 2 mine with
    /// range 2 works 6 × 6) …
    #[serde(default)]
    pub gather_range: i32,
    /// … whether they work the tiles they stand on too (the mine: the ore under it) …
    #[serde(default)]
    pub gather_below: bool,
    /// … and how much each fitting tile gives per payout (at full power).
    #[serde(default)]
    pub gather_per_tile: f32,
    /// The mine: gold per payout for every tile of gold it works (at full power) – TAB's mine digs 20 times as much
    /// gold from a tile as stone (`World::gold_rate`).
    #[serde(default)]
    pub gold_per_tile: f32,
    /// The market: its houses are the ones whose middle lies on its site or this many tiles around it (`trade_area`) …
    #[serde(default)]
    pub trade_range: i32,
    /// … they pay this many percent more gold …
    #[serde(default)]
    pub house_bonus_percent: u32,
    /// … and for every this many colonists living in them it sells one good per payout (`sim/market.rs`).
    #[serde(default)]
    pub colonists_per_good: u32,
    /// The bank: this much more gold fits in the storage …
    #[serde(default)]
    pub gold_storage: u32,
    /// … and this much credit it gives – gold you may buy below 0 with Shift (`sim/debt.rs`).
    #[serde(default)]
    pub credit: u32,
    /// The munitions factory: this many barrels fit in its store (`sim/blast.rs`).
    #[serde(default)]
    pub barrel_storage: u32,
    /// The foundry: iron it smelts at every payout at full power (less with less power, none without ore or coal) …
    #[serde(default)]
    pub smelts: u32,
    /// … out of this much iron ore and coal for every one (`World::smelt`).
    #[serde(default)]
    pub smelt_ore: u32,
    #[serde(default)]
    pub smelt_coal: u32,
    /// A tower that shoots by itself (ballista, machine gun tower, `sim/towers.rs`). Left out: it doesn't shoot.
    #[serde(default)]
    pub weapon: Option<Weapon>,
    /// Noise it makes every `work_every_seconds` while it works – machines are loud (`sim/sound.rs`). Left out: 0, quiet.
    #[serde(default)]
    pub noise: u32,
    /// A trap: damage to every zombie on it, every `trap_every_seconds` …
    #[serde(default)]
    pub trap_damage: u32,
    /// … and how many of its hit points every such hit costs it.
    #[serde(default)]
    pub trap_wear: u32,
    /// Ticks from placing it until it's finished – it builds itself, like in TAB (`World::start_construction`). In the
    /// file in seconds (`build_seconds`): short, longer the bigger it is and the higher in the tech tree. Left out: 0,
    /// it stands at once.
    #[serde(default, rename = "build_seconds", deserialize_with = "data::seconds")]
    pub build_time: u32,
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

/// The gun of a tower that shoots by itself – one block `weapon` in `data\buildings.json`.
#[derive(Deserialize)]
pub struct Weapon {
    /// Per shot, minus the zombie's armor (at least 1).
    pub damage: u32,
    /// Tiles from its middle.
    pub range: f32,
    /// Ticks from one shot to the next – in the file in seconds (`reload_seconds`). Short of power it reloads slower.
    #[serde(rename = "reload_seconds", deserialize_with = "data::seconds")]
    pub reload: u32,
    /// Noise per shot, like the units' (`sim/sound.rs`).
    pub noise: u32,
    /// Whom it aims at: `true` the toughest zombie in range (most hit points – the ballista's bolt is wasted on
    /// walkers), `false` the nearest.
    #[serde(default)]
    pub toughest: bool,
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

    /// Does it train units (a production queue and a rally point)? The barracks – and the inn its mercenaries. Which
    /// unit where: `trained_in` in `data\units.json`.
    pub fn trains(self) -> bool {
        matches!(self, BuildingKind::Barracks | BuildingKind::Inn)
    }

    /// Does it make barrels (a production queue of them, `sim/blast.rs`)? The munitions factory.
    pub fn makes_barrels(self) -> bool {
        self.stats().barrel_storage > 0
    }

    /// The walls, and the gates in them – a harpy leaps over them (`leap_over`).
    pub fn is_wall(self) -> bool {
        matches!(self, BuildingKind::Wall | BuildingKind::StoneWall | BuildingKind::Gate)
    }

    /// The towers – units go in (`tower_slots`).
    pub fn is_tower(self) -> bool {
        matches!(self, BuildingKind::Tower | BuildingKind::StoneTower)
    }

    /// Defence – walls, gates, towers and the ones that shoot by themselves (TAB's `DeffensiveStructure`): what a
    /// zombie tears down when it stands in its way (`wall_in_way` in `sim/zombies.rs`). Not the traps: they walk over
    /// them.
    pub fn is_defence(self) -> bool {
        self.is_wall() || self.is_tower() || self.stats().weapon.is_some()
    }

    /// A trap: everyone walks over it, zombies don't attack it – it hurts them (`trap_damage`).
    pub fn is_trap(self) -> bool {
        self.stats().trap_damage > 0
    }

    /// What a gathering building works, and what it gets from it – a mine: whatever ore lies there (`None`).
    pub fn gathers(self) -> Option<(Source, Option<Resource>)> {
        match self {
            BuildingKind::Sawmill => Some((Source::Forest, Some(Resource::Wood))),
            BuildingKind::HunterCottage => Some((Source::Forest, Some(Resource::Food))),
            BuildingKind::FishingHut => Some((Source::Water, Some(Resource::Food))),
            BuildingKind::Farm => Some((Source::Fertile, Some(Resource::Food))),
            BuildingKind::Mine => Some((Source::Ore, None)),
            _ => None,
        }
    }

    /// Its site standing at `tile` (top left), plus `range` tiles on every side – a square (`Area`). Only for buildings
    /// that are never turned.
    pub fn around(self, tile: (i32, i32), range: i32) -> Area {
        let (w, h) = self.footprint(false);
        ((tile.0 - range, tile.1 - range), (tile.0 + w + range, tile.1 + h + range))
    }

    /// The tiles a gathering building standing at `tile` can work: its site plus `gather_range` tiles around it.
    pub fn gather_area(self, tile: (i32, i32)) -> Area {
        self.around(tile, self.stats().gather_range)
    }

    /// Where a market standing at `tile` finds its houses: its site plus `trade_range` tiles around it.
    pub fn trade_area(self, tile: (i32, i32)) -> Area {
        self.around(tile, self.stats().trade_range)
    }

    /// Does it have to stand on grass – every tile of it fertile (`Terrain::is_fertile`)? The farm: nothing grows on
    /// dirt.
    pub fn needs_fertile(self) -> bool {
        self == BuildingKind::Farm
    }

    /// Does it have to stand inside the power grid – does it make, take or carry power? Not the command center: it
    /// is where the grid starts.
    pub fn on_grid(self) -> bool {
        let s = self.stats();
        self != BuildingKind::CommandCenter && (s.energy > 0 || s.energy_use > 0 || s.power_radius > 0.0)
    }

    /// Can it be switched off (Y) – does it cost or make anything while it runs? Not the command center, not the
    /// houses (their colonists live there), not walls, towers and traps (they take nothing). Not the bank either:
    /// switched off its storage and credit would be gone – and with them the gold above and the debt in them.
    pub fn can_switch_off(self) -> bool {
        let s = self.stats();
        let runs = s.upkeep > 0 || s.worker_cost > 0 || s.energy_use > 0 || s.energy > 0 || s.power_radius > 0.0;
        let bank = s.gold_storage > 0 || s.credit > 0;
        self != BuildingKind::CommandCenter && s.colonists == 0 && !bank && runs
    }

    /// Width and depth in tiles – turned, the two swap (only the gate isn't square).
    pub fn footprint(self, turned: bool) -> (i32, i32) {
        let (w, h) = self.stats().size;
        if turned { (h, w) } else { (w, h) }
    }
}
