//! Unit and building types (DESIGN.md 6, 7.2). Their values are in `data\units\` and `data\buildings\`
//! (loaded by `data.rs`) – here only what a value means.

use serde::{Deserialize, Serialize};

use crate::attrs::{self, Attributes, Extra};
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
    /// TAB's flamethrower (`trained_in` the engineering center): a cone of fire in front of it – it sets zombies on fire
    /// and burns your own units in it too (`friendly_fire`).
    Lucifer,
    /// TAB's rocket launcher: a missile at a crowd further than `min_range` away (`splash`) – and close up a blow with
    /// the launcher (`melee`).
    Thanatos,
    /// TAB's walking cannon: far, hard shots that hit a little area (`splash`).
    Titan,
}

/// A second attack from close by (Thanatos's blow, TAB's `ThanatosAttack`) – used whenever the target is within its
/// `range`, otherwise the unit's own attack. Its times like the unit's (`Stats`), in seconds in the file.
#[derive(Deserialize)]
pub struct Melee {
    pub damage: u32,
    pub range: f32,
    /// It hits everyone in a cone this wide (degrees) and `range` long in front of it …
    pub cone_degrees: f32,
    /// … at the edge only this share of `damage` (TAB's `MinFactorDamageInArea`).
    pub edge_factor: f32,
    #[serde(rename = "windup_seconds", deserialize_with = "data::seconds")]
    pub windup: u32,
    #[serde(rename = "cooldown_seconds", deserialize_with = "data::seconds")]
    pub cooldown: u32,
}

/// The values of a unit type – one block in `data\units\`.
#[derive(Deserialize)]
pub struct Stats {
    pub name: String,
    /// One or two letters for the icon in the selection panel.
    pub letter: String,
    /// Gold, paid when it goes into the production queue – back in full if it's cancelled (like in StarCraft).
    pub cost: u32,
    /// The rest of the price, like TAB's units (ranger 2 wood, soldier 2 iron): wood and iron from the foundry
    /// (`Resource::Iron`) out of the stock, paid and given back with the gold. Left out: 0.
    #[serde(default)]
    pub cost_wood: u32,
    #[serde(default)]
    pub cost_iron: u32,
    /// Oil out of the stock – TAB's engineering-center units.
    #[serde(default)]
    pub cost_oil: u32,
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
    /// Oil every payout for as long as it lives (TAB's `OilGen` −1, the Titan −2) – like its food. Left out: 0.
    #[serde(default)]
    pub oil: u32,
    pub hp: u32,
    /// Armor like StarCraft 2's: this many points come off every zombie hit (`after_armor`; at least 1 always gets
    /// through). Venom and fire ignore it.
    #[serde(default)]
    pub armor: u32,
    /// What it is (`attrs.rs`): `["light", "biological"]` – what a weapon's `bonus` is checked against.
    #[serde(default)]
    pub attributes: Attributes,
    pub damage: u32,
    /// Extra damage per attribute of the target, on top of `damage` (SC2's "+10 vs Armored"): `{ "armored": 10 }`.
    #[serde(default)]
    pub bonus: Extra,
    /// Range in tiles.
    pub range: f32,
    /// How far it sees, in tiles – at least as far as it shoots (forest blocks the view).
    pub sight: f32,
    /// Ticks for turning around once (180°) – like StarCraft's turn rate. A unit only aims once it faces its target.
    /// In the file in seconds (`turn_around_seconds`).
    #[serde(rename = "turn_around_seconds", deserialize_with = "data::seconds")]
    pub turn_around: u32,
    /// Ticks to bring the weapon up (TAB's `TimeLoad`, the animation `LoadAttack`) – once, when it stands and has a
    /// target; it stays up as long as it keeps shooting. In the file in seconds (`raise_seconds`).
    #[serde(rename = "raise_seconds", deserialize_with = "data::seconds")]
    pub raise: u32,
    /// Ticks to take it down again (TAB's `TimeUnload`) – while it walks, it doesn't wait for that. Walked only a short
    /// bit: it's only partly down, and raising it again is quicker (`combat.rs`). In the file `lower_seconds`.
    #[serde(rename = "lower_seconds", deserialize_with = "data::seconds")]
    pub lower: u32,
    /// Ticks of holding the aim before every shot (TAB's `TimePreAction` – only the sniper has one). Another target:
    /// aiming starts over. In the file in seconds (`aim_seconds`).
    #[serde(rename = "aim_seconds", deserialize_with = "data::seconds")]
    pub aim: u32,
    /// Ticks from the start of the shot's animation to the shot itself (TAB's `TimeAction × TimeFactorAction`) –
    /// StarCraft's "damage point". Walking off now cancels the shot: no damage, and no reloading either. In the file
    /// `windup_seconds`.
    #[serde(rename = "windup_seconds", deserialize_with = "data::seconds")]
    pub windup: u32,
    /// Ticks of reloading after a shot, before it can aim again (TAB's rest of `TimeAction` + `TimePostAction`). It goes
    /// on while the unit walks – walking off right after the shot costs no time (animation cancelling). One shot takes
    /// `aim + windup + cooldown`. In the file in seconds (`cooldown_seconds`); 0 for the medic – its beam is on every
    /// tick, so it looks continuous.
    #[serde(rename = "cooldown_seconds", deserialize_with = "data::seconds")]
    pub cooldown: u32,
    /// Its arrow flies this many tiles per second (TAB's `ProyectileSpeed`, the ranger 12) and hits when it lands
    /// (`sim/arrows.rs`). Left out: 0 – a bullet, it hits at once.
    #[serde(default)]
    pub projectile_speed: f32,
    /// Tiles per second.
    pub speed: f32,
    /// Only while one of these stands can it be trained – firearms need the munitions factory. Written as the type's
    /// name in the file (`"MunitionsFactory"`), left out: none.
    #[serde(default)]
    pub requires: Option<BuildingKind>,
    /// Where it's trained: `"Barracks"` (left out), `"Inn"` – the mercenaries – or `"EngineeringCenter"`.
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
    /// How much of venom (the spitter's acid) hurts it – its own resistance, TAB's `VenomDamageFactor`: 1 = all of it,
    /// 0.5 = half. Armor doesn't count against venom (`Unit::take_venom`). Every type has its own, so it's in every
    /// block of `data\units\` – no default.
    pub venom_factor: f32,
    /// It heals itself like TAB's units (`LifeRegenFactor`): every `regen_every_seconds` this share of its full hit
    /// points (rounded up), once it hasn't been hit for `regen_after_seconds` (`data\units\`, `sim/units.rs`). In
    /// every block – no default.
    pub regen: f32,
    /// Its shot hits everyone within this many tiles of where it lands (TAB's `EffectRadius`) – 0: only its target …
    #[serde(default)]
    pub splash: f32,
    /// … at the edge of it only this share of `damage` (TAB's `MinFactorDamageInArea`; 1: everyone all of it).
    #[serde(default = "one")]
    pub edge_factor: f32,
    /// It shoots nothing closer than this (tiles) – Thanatos's missile (TAB's `MinTargetDistance`).
    #[serde(default)]
    pub min_range: f32,
    /// Lucifer: it hits everyone in a cone this wide (degrees) and `range` long in front of it – 0: no cone.
    #[serde(default)]
    pub cone_degrees: f32,
    /// Its shot hurts your own units too (TAB's `FriendlyDamage`) – the ones in its cone or its splash.
    #[serde(default)]
    pub friendly_fire: bool,
    /// The share of hits that set a zombie on fire (TAB's `BurnFactor`, Lucifer 0.7) – it burns for its breed's
    /// `burn_seconds` (`sim/fire.rs`).
    #[serde(default)]
    pub burn_chance: f32,
    /// How much fire hurts it (TAB's `FireDamageFactor`: 1 all, 0.5 half, 0 none – Lucifer's suit). In every block – no
    /// default, like `venom_factor`.
    pub fire_factor: f32,
    /// Thanatos: the blow it deals close up instead of its missile.
    #[serde(default)]
    pub melee: Option<Melee>,
    /// Its looks: sprites in `assets\` (`sprites.rs`), e.g. `"units/ranger"` – every unit wears one of them, for life.
    /// Those that aren't there are left out (TAB's pictures only exist where they were converted). Left out: none.
    #[serde(default)]
    pub skins: Vec<String>,
    /// Its picture if none of its `skins` is there, e.g. `"test/figure"`. Missing too: a box.
    pub sprite: Option<String>,
    /// What one level of a track of upgrades gives it (`sim/upgrades.rs`): `{ "infantry_weapons": { "damage": 1 } }`.
    #[serde(default)]
    pub upgrades: std::collections::BTreeMap<String, UpgradeBonus>,
}

/// What one level of an upgrade track gives one kind of unit or building – flat, like in SC2 (a marine +1, a marauder
/// +2). Left out: 0.
#[derive(Deserialize, Clone, Copy, Default, Debug, PartialEq)]
#[serde(default)]
pub struct UpgradeBonus {
    pub hp: u32,
    pub shield: u32,
    pub damage: u32,
    /// Armor points (SC2: +1 per level).
    pub armor: u32,
    /// A building's shield armor points.
    pub shield_armor: u32,
    /// Added to *every* extra damage of the weapon (`bonus`, `attrs.rs`) – SC2's marauder: +1 (+1 vs Armored).
    pub bonus_damage: u32,
    pub range: f32,
    pub sight: f32,
    /// Tiles per second.
    pub speed: f32,
    /// Percent points of attack speed.
    pub attack_speed: u32,
}

impl UnitKind {
    pub fn stats(self) -> &'static Stats {
        data::get().unit(self)
    }

    /// All types, in the order of the selection panel.
    pub const ALL: [UnitKind; 11] = [
        UnitKind::Soldier,
        UnitKind::Ranger,
        UnitKind::Sniper,
        UnitKind::Medic,
        UnitKind::Spearman,
        UnitKind::MercSoldier,
        UnitKind::MercRanger,
        UnitKind::MercSniper,
        UnitKind::Lucifer,
        UnitKind::Thanatos,
        UnitKind::Titan,
    ];
}

/// `trained_in` left out: the barracks.
fn barracks() -> BuildingKind {
    BuildingKind::Barracks
}

/// 1 – `edge_factor` left out: everyone in the splash takes all of it.
fn one() -> f32 {
    1.0
}

impl Stats {
    /// Ticks from the start of aiming to the next start of aiming – one shot.
    pub fn shot_ticks(&self) -> u32 {
        self.aim + self.windup + self.cooldown
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
    /// TAB's wooden house: the house between tent and stone house – goes on a tent, a stone house goes on it.
    WoodHouse,
    /// TAB's power plant: a lot of power, burns wood like TAB's (DESIGN.md's coal plant comes later).
    PowerPlant,
    /// TAB's oil platform: right on an oil node (all its tiles oil), pumps oil.
    OilPlatform,
    /// TAB's quarry: stone from the stone patches under and around it – only stone (DESIGN.md 6).
    Quarry,
    /// TAB's advanced quarry: the same, twice as much per tile.
    BigQuarry,
    /// TAB's advanced farm: twice the food per field.
    BigFarm,
    /// TAB's iron mill: twice the windmill's power.
    BigMill,
    /// TAB's warehouse: more room in the stock, and the gathering buildings around it bring in more.
    Warehouse,
    /// TAB's advanced unit center: trains Lucifer, Thanatos and Titan.
    EngineeringCenter,
    /// TAB's shocking tower: charges, then hits every zombie in range at once.
    ShockingTower,
    /// TAB's executor: a turning gun like the ballista's, fast, hard, a little splash.
    Executor,
    /// TAB's blade trap: the stake trap, harder.
    BladeTrap,
    /// TAB's mine: goes off under the first zombie that steps on it.
    MineTrap,
    /// TAB's radar: sees very far, through forest too.
    Radar,
    /// TAB's lookout tower: sees far, through forest too.
    LookoutTower,
    /// TAB's wonders (DESIGN.md question 29) – one each, with TAB's effects: the Spire (much power, far reach), the
    /// Academy (every unit a veteran), the Victorious (a fortress, houses around it pay more), the Transmutator (goods
    /// into oil), the Crystal Palace (colonists and food).
    Spire,
    Academy,
    Victorious,
    Transmutator,
    CrystalPalace,
    /// A stop for trains (`docs/trains.md` step 3): a building with life that zombies can hit. A train stops on the
    /// track next to it (`World::station_stop`). Only the editor sets it – never in a build menu.
    TrainStation,
}

impl BuildingKind {
    /// Every type, in the order above – `kind as usize` is its place in this list.
    pub const ALL: [BuildingKind; 49] = [
        BuildingKind::CommandCenter,
        BuildingKind::Barracks,
        BuildingKind::Tent,
        BuildingKind::Wall,
        BuildingKind::Gate,
        BuildingKind::Tower,
        BuildingKind::Sawmill,
        BuildingKind::Mine,
        BuildingKind::HunterCottage,
        BuildingKind::FishingHut,
        BuildingKind::Farm,
        BuildingKind::Windmill,
        BuildingKind::CharcoalBurner,
        BuildingKind::WoodWorkshop,
        BuildingKind::TeslaTower,
        BuildingKind::StoneWorkshop,
        BuildingKind::MunitionsFactory,
        BuildingKind::Foundry,
        BuildingKind::StoneHouse,
        BuildingKind::StoneWall,
        BuildingKind::StoneTower,
        BuildingKind::Ballista,
        BuildingKind::MachineGunTower,
        BuildingKind::Stakes,
        BuildingKind::Market,
        BuildingKind::Bank,
        BuildingKind::Inn,
        BuildingKind::Barrel,
        BuildingKind::WoodHouse,
        BuildingKind::PowerPlant,
        BuildingKind::OilPlatform,
        BuildingKind::Quarry,
        BuildingKind::BigQuarry,
        BuildingKind::BigFarm,
        BuildingKind::BigMill,
        BuildingKind::Warehouse,
        BuildingKind::EngineeringCenter,
        BuildingKind::ShockingTower,
        BuildingKind::Executor,
        BuildingKind::BladeTrap,
        BuildingKind::MineTrap,
        BuildingKind::Radar,
        BuildingKind::LookoutTower,
        BuildingKind::Spire,
        BuildingKind::Academy,
        BuildingKind::Victorious,
        BuildingKind::Transmutator,
        BuildingKind::CrystalPalace,
        BuildingKind::TrainStation,
    ];
}

/// The goods in the colony's stock (DESIGN.md 5.1) – gold is kept apart (`World::gold`). Oil comes with the oil pump.
/// `Serialize, Deserialize`: a script names them in the map file (`sim/story.rs`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Resource {
    Wood,
    Stone,
    /// Iron ore – out of the mines. The foundry makes iron of it.
    IronOre,
    Coal,
    Food,
    /// Iron – smelted in the foundry from iron ore and coal (`smelts` in `data\buildings\`). The second tier of
    /// defence costs it: the machine gun tower.
    Iron,
    /// Oil – pumped by the oil platform from an oil node (DESIGN.md 5.1). TAB's late units and buildings cost it.
    Oil,
}

impl Resource {
    /// How many there are – the size of the lists with one entry per good (`Stock`).
    pub const COUNT: usize = 7;
    pub const ALL: [Resource; Resource::COUNT] =
        [Resource::Wood, Resource::Stone, Resource::IronOre, Resource::Coal, Resource::Food, Resource::Iron, Resource::Oil];

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
            Resource::Oil => "Oil",
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
    /// Game, like TAB's hunter: forest, grass and dirt, each giving its own share (the map's `hunting_*`).
    Hunting,
    Water,
    /// Grass without ore on it – the fields of a farm (`Terrain::is_fertile`).
    Fertile,
    /// Ore patches (not oil) – each tile gives its own ore.
    Ore,
    /// Only the stone patches – the quarries.
    Stone,
    /// The oil nodes – the oil platform.
    Oil,
}

/// The values of a building type – one block in `data\buildings\`.
#[derive(Deserialize)]
pub struct BuildingStats {
    pub name: String,
    pub letter: String,
    /// Width (along x) and depth (along y) in tiles, not turned.
    pub size: (i32, i32),
    pub hp: u32,
    /// Armor points of its hit points: they come off every hit that reaches them (`after_armor`, at least 1 gets
    /// through). TAB's buildings have none – left out: 0.
    #[serde(default)]
    pub armor: u32,
    /// Armor points of its shield – SC2's shield armor (Protoss): they come off every hit that lands on the shield, and
    /// the hit points keep their own `armor`. Left out: 0.
    #[serde(default)]
    pub shield_armor: u32,
    /// What it is (`attrs.rs`). Left out: armored and a structure.
    #[serde(default = "attrs::building_default")]
    pub attributes: Attributes,
    /// Infection like in TAB (DESIGN.md 6): zombies first hit the shield. When it breaks, the building is infected –
    /// its people come out as zombies (`World::infect`) – and only then do the hit points go down. In points, on top of
    /// `hp`: TAB's `DeffensesLife` – TAB's `Life` is `hp` + `shield`. 0: no shield. Without colonists a shield is just
    /// extra life: nobody can get infected. The command center's broken shield loses the round (`round.rs`).
    pub shield: u32,
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
    /// Oil from the oil platform – TAB's late buildings cost it.
    #[serde(default)]
    pub cost_oil: u32,
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
    /// The same with coal (the power plant – DESIGN.md's coal plant).
    #[serde(default)]
    pub burns_coal: u32,
    /// Goods it turns into others every payout while it works (TAB's negative and positive `WoodGen`, `OilGen` …): the
    /// Transmutator wood, stone and iron into oil, the Victorious eats oil. Only when all it `takes` is in the stock.
    #[serde(default)]
    pub converts: Option<Convert>,
    /// This much more of every good fits into the stock while it stands – not food, not gold (TAB's
    /// `ResourcesStorage`, the warehouse).
    #[serde(default)]
    pub goods_storage: u32,
    /// TAB's buildings that help their neighbours (`CBuildingEnhanced`): the buildings whose whole site lies within its
    /// site plus this many tiles …
    #[serde(default)]
    pub boost_range: i32,
    /// … bring in this many percent more when they gather (the warehouse: TAB's `FactorProductionNearBuildings`) …
    #[serde(default)]
    pub gather_boost_percent: u32,
    /// … or pay this many percent more gold (the Victorious: TAB's `FactorGoldNearBuildings`).
    #[serde(default)]
    pub gold_boost_percent: u32,
    /// Sees over forest – nothing blocks its view (TAB's `WatchThroughOpaque`: radar, lookout tower, wonders).
    #[serde(default)]
    pub sees_through_forest: bool,
    /// At most one of it may stand (TAB's `MaxInstances` 1 – the wonders).
    #[serde(default)]
    pub only_one: bool,
    /// While it works every one of your units counts as a veteran (TAB's `MakeSoldiersVeteran`, the Academy).
    #[serde(default)]
    pub veteran_units: bool,
    /// The mine trap: what it does when a zombie steps on it (`sim/traps.rs`).
    #[serde(default)]
    pub mine: Option<MineBlast>,
    /// Food its colonists eat every payout – without it, they pay no gold (houses).
    #[serde(default)]
    pub food: u32,
    /// Gold it pays every payout (`economy_every_seconds`) at full power – houses only when their colonists are fed
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
    /// … and how much each fitting tile gives per payout (at full power). The hunter cottage: times the map's
    /// `hunting_forest` / `_grass` / `_dirt` (`data::Modifiers`).
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
    /// Everyone on its tiles – zombies and your units – walks this many percent slower (TAB's stakes:
    /// `TerrainSpeedPercentage` −50). Left out: 0.
    #[serde(default)]
    pub slow_percent: u32,
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
    /// Its looks, like the units' `skins`: TAB's pictures in `assets\` (`building_sprites.rs`), e.g.
    /// `"buildings/tent_1"` – every building wears one of them, for life (the fishing hut: the one facing its
    /// water). Those that aren't there are left out; none there or left out: drawn as a block.
    #[serde(default)]
    pub skins: Vec<String>,
    /// What it looks like when it falls: effects in `assets\effects\` (`effects.rs`), like TAB's `ExplosionOnDestroy`
    /// – with two, every other one. Only the look.
    #[serde(default)]
    pub explosion: Vec<String>,
    /// What one level of a track of upgrades gives it (`sim/upgrades.rs`) – a tower's damage, range, attack speed, life.
    #[serde(default)]
    pub upgrades: std::collections::BTreeMap<String, UpgradeBonus>,
}

/// The gun of a tower that shoots by itself – one block `weapon` in `data\buildings\`.
#[derive(Deserialize)]
pub struct Weapon {
    /// Per shot, minus the zombie's armor (at least 1).
    pub damage: u32,
    /// Extra damage per attribute of the zombie, on top of `damage` (SC2's "+50 vs Massive"): `{ "massive": 50 }`.
    #[serde(default)]
    pub bonus: Extra,
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
    /// The shocking tower: once charged it hits every zombie in range and in sight at once (TAB's `ShockingTower`).
    #[serde(default)]
    pub all_in_range: bool,
    /// Its shot hits everyone within this many tiles of its target, all of `damage` (the executor: TAB's
    /// `EffectRadius`). 0: only the target.
    #[serde(default)]
    pub splash: f32,
}

/// Goods a building turns into others every payout (`BuildingStats::converts`): `[["Wood", 20], …]`.
#[derive(Deserialize)]
pub struct Convert {
    #[serde(default)]
    pub takes: Vec<(Resource, u32)>,
    #[serde(default)]
    pub makes: Vec<(Resource, u32)>,
}

/// What a mine trap does (TAB's `TrapMine`): a zombie steps on it, its delay (0–3, set on its card like TAB's
/// `DelayActivation`) × `delay_step_seconds` later it goes off – `damage` to everyone within `radius` (zombies, your
/// units, buildings, like a barrel's blast), and it's gone.
#[derive(Deserialize)]
pub struct MineBlast {
    pub damage: u32,
    pub radius: f32,
    #[serde(rename = "delay_step_seconds", deserialize_with = "data::seconds")]
    pub delay_step: u32,
}

impl BuildingStats {
    /// The full shield in points (`shield`).
    pub fn max_shield(&self) -> u32 {
        self.shield
    }
}

impl BuildingKind {
    pub fn stats(self) -> &'static BuildingStats {
        data::get().building(self)
    }

    /// Does it train units (a production queue and a rally point)? The barracks – and the inn its mercenaries. Which
    /// unit where: `trained_in` in `data\units\`.
    pub fn trains(self) -> bool {
        matches!(self, BuildingKind::Barracks | BuildingKind::Inn | BuildingKind::EngineeringCenter)
    }

    /// Does it make barrels (a production queue of them, `sim/blast.rs`)? The munitions factory.
    pub fn makes_barrels(self) -> bool {
        self.stats().barrel_storage > 0
    }

    /// Only the editor puts it down – it isn't in any build menu (the train station).
    pub fn editor_only(self) -> bool {
        self == BuildingKind::TrainStation
    }

    /// TAB's wonders – the build menu Y.
    pub fn is_wonder(self) -> bool {
        matches!(self, BuildingKind::Spire | BuildingKind::Academy | BuildingKind::Victorious | BuildingKind::Transmutator | BuildingKind::CrystalPalace)
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

    /// May it be built on ore (`placement.rs`)? Only defence – walls, gates, towers, the ones that shoot by themselves
    /// and the traps – and what digs it: the mine and the quarries, which work the tiles under them too
    /// (`gather_below`). Ours: TAB builds nothing on ore. Oil is another matter: only the oil platform (`needs_oil`).
    pub fn on_ore(self) -> bool {
        self.is_defence() || self.is_trap() || matches!(self.gathers(), Some((Source::Ore | Source::Stone, _)))
    }

    /// A trap: everyone walks over it, zombies don't attack it – it hurts them (`trap_damage`), or it goes off under
    /// them (the mine).
    pub fn is_trap(self) -> bool {
        self.stats().trap_damage > 0 || self.stats().mine.is_some()
    }

    /// What a gathering building works, and what it gets from it – a mine: whatever ore lies there (`None`).
    pub fn gathers(self) -> Option<(Source, Option<Resource>)> {
        match self {
            BuildingKind::Sawmill => Some((Source::Forest, Some(Resource::Wood))),
            BuildingKind::HunterCottage => Some((Source::Hunting, Some(Resource::Food))),
            BuildingKind::FishingHut => Some((Source::Water, Some(Resource::Food))),
            BuildingKind::Farm | BuildingKind::BigFarm => Some((Source::Fertile, Some(Resource::Food))),
            BuildingKind::Mine => Some((Source::Ore, None)),
            BuildingKind::Quarry | BuildingKind::BigQuarry => Some((Source::Stone, Some(Resource::Stone))),
            BuildingKind::OilPlatform => Some((Source::Oil, Some(Resource::Oil))),
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
        matches!(self, BuildingKind::Farm | BuildingKind::BigFarm)
    }

    /// Does it have to stand right on an oil node – every tile of it oil? The oil platform (TAB's `OilSource`).
    pub fn needs_oil(self) -> bool {
        self == BuildingKind::OilPlatform
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
