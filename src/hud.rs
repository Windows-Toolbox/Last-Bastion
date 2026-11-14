//! The panel at the bottom, laid out like in They Are Billions (DESIGN.md 11.1): minimap, the clock with the pause
//! button, the control groups, the selection, the command card with the build menus, the colony's stock. Here is
//! where everything lies (`Layout`), what it looks like, and what is under the mouse (`hit`). What a click does is
//! decided by `ui.rs`.
//!
//! Where everything lies is TAB's: its clip `MapGUI` (in `ZXGame.dxprj`) places the bar and every field on a screen of
//! 1280 x 720 – our window's first size. A bigger window makes all of it bigger by the same factor (`scale`, TAB's
//! `DXScene.ScreenVirtualUnit`); a window wider than 16 : 9 gets a wider selection in the middle. Its pictures are
//! TAB's (`ui_art.rs`) – without them the old brass placeholders are drawn in the same places.

use std::cell::Cell;

use macroquad::prelude::*;

use crate::draw::{breed_color, building_color, kind_color, loot_color, ZOMBIE};
use crate::formation::Shape;
use crate::iso::{Camera, TILE_H, TILE_W};
use crate::editor::Sight;
use crate::data;
use crate::keys::Action;
use crate::kinds::{BuildingKind, Resource, Source, UnitKind};
use crate::map::{from_frame, to_frame, Region};
use crate::picture::MapPicture;
use crate::sim::{researched_in, tech, DebtStage, LootKind, Power, Product, RepairPlan, World, Zombie, MAX_PRODUCTION, TICKS_PER_SECOND};
use crate::ui::{Pick, Place, Sub, Ui};
use crate::ui_art;

/// TAB's screen the bar is laid out on (`MapGUI`): 1280 x 720 – our window's first size.
const REF_W: f32 = 1280.0;
const REF_H: f32 = 720.0;
/// The bar's picture (`GUIBackgroundControls`): its top left corner and height on TAB's screen.
const BAR_Y: f32 = 523.0;
const BAR_H: f32 = 197.7;
/// Where the bar's picture may be stretched on a window wider than 16 : 9 – a plain stretch of the green screen in
/// the middle (TAB's x, on its 1280 pixels).
const STRETCH: (f32, f32) = (600.0, 700.0);
/// Without TAB's picture: the bar is a plain rectangle from here down (the top of the minimap's frame).
const PLAIN_TOP: f32 = 553.0;

/// Multi-selection: one icon per thing, this big.
const ICON: f32 = 36.0;
const QUEUE_ICON: f32 = 36.0;
const SUMMARY_H: f32 = 16.0;
/// The values next to the text, one small box each (TAB's `AreaFeatures`).
const CELL_W: f32 = 45.5;
const CELL_H: f32 = 29.0;
/// A button of the command card: 5 x 3 of them in TAB's 200 x 120.
const BUTTON: f32 = 40.0;
const CARD_COLS: usize = 5;
const CARD_ROWS: usize = 3;

// Brass and dark wood like TAB's steampunk panel, the values glowing green like its icons.
const PANEL_BG: Color = Color::new(0.09, 0.07, 0.05, 0.97);
const SECTION_BG: Color = Color::new(0.05, 0.04, 0.03, 1.0);
const BOX_BG: Color = Color::new(0.15, 0.11, 0.07, 1.0);
/// The fill of a storage bar in the stock – and full, when what comes in is lost.
const STORAGE: Color = Color::new(0.36, 0.27, 0.12, 1.0);
const STORAGE_FULL: Color = Color::new(0.55, 0.22, 0.12, 1.0);
const BORDER: Color = Color::new(0.45, 0.32, 0.16, 1.0);
const BRASS: Color = Color::new(0.80, 0.60, 0.30, 1.0);
const DIM: Color = Color::new(0.72, 0.64, 0.50, 1.0);
const GLOW: Color = Color::new(0.62, 0.95, 0.40, 1.0);
const KEY: Color = Color::new(1.0, 0.85, 0.40, 1.0);
const TOO_DEAR: Color = Color::new(1.0, 0.35, 0.3, 1.0);
const FINAL: Color = Color::new(1.0, 0.6, 0.5, 1.0);
/// Over TAB's buttons: hovered a bit lighter, waiting (the command on the mouse, the chosen formation) green.
const HOVER_LIGHT: Color = Color::new(1.0, 0.95, 0.8, 0.16);
const WAIT_LIGHT: Color = Color::new(0.5, 1.0, 0.3, 0.30);
/// A button that can't be pressed right now (too dear, planned): its picture greyed.
const GREYED: Color = Color::new(0.45, 0.45, 0.45, 1.0);
/// The villages of doom on the minimap – red like TAB's.
const DOOM_MINIMAP: Color = Color::new(0.85, 0.12, 0.10, 1.0);
/// A train on the minimap – TAB's `GreenYellow`.
const TRAIN_MINIMAP: Color = Color::new(0.68, 1.0, 0.18, 1.0);
/// A giant on the minimap: a big dot, always shown.
const GIANT_MINIMAP: Color = Color::new(0.95, 0.45, 0.15, 1.0);
/// A swarm of a wave on the minimap: a warning triangle in its middle.
const SWARM_MINIMAP: Color = Color::new(1.0, 0.22, 0.15, 1.0);
/// The dark edge around the marks, so they stand out on any ground.
const MARK_EDGE: Color = Color::new(0.08, 0.05, 0.03, 1.0);

/// The buttons of the command card. Which ones are visible depends on the selection (`Ui::card`). Their keys:
/// `keys.rs` (`Eq` and `Hash`: so a button can be the key of a `HashMap` there).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Button {
    // Units
    Move,
    Stop,
    Hold,
    Patrol,
    Attack,
    /// Formations (like in Diplomacy is Not an Option) – only choose the shape, the right mouse button draws it.
    FormLine,
    FormBox,
    FormCircle,
    // Barracks and inn
    /// Train this unit (its building: `trained_in`).
    Train(UnitKind),
    Rally,
    /// Munitions factory: a barrel into its queue (`sim/blast.rs`).
    MakeBarrel,
    /// Market: selling on or off (`sim/market.rs`).
    Sell,
    /// Mine trap: how long it waits once a zombie steps on it – 0 to 3 steps of half a second (TAB's
    /// `DelayActivation`, `sim/blast.rs`).
    Delay(u8),
    /// The workshops: start this research (`sim/research.rs`, the index in `data\research.json`).
    Research(u16),
    /// Command center: show a circle of the map for a while (`sim/scan.rs`).
    Scan,
    /// Units carrying a barrel: put it down where they stand.
    Drop,
    /// Units: send them away – no more upkeep, their workers free.
    Dismiss,
    // Command center
    /// Open one of the build menus – the card then shows its buildings (`Ui::submenu`).
    Open(Submenu),
    /// Back from a build menu to the main buttons of its building – B (its spot in the grid) or Esc.
    Back,
    /// Put this building on the mouse, to place it.
    Place(BuildingKind),
    /// A building that's planned but not in the game yet – the index in `PLANNED`. Its button already sits where
    /// it will be, greyed out.
    Planned(usize),
    // Tower
    Unload,
    /// Every building: repair the selected buildings (F, like "fix").
    Repair,
    /// Buildings that run (`can_switch_off`): switch the selected ones off – or on again (Y, like in TAB).
    Switch,
    /// Every building but the command center: tear the selected ones down, part of the price comes back (Del, like
    /// TAB). In the editor: remove them, the command center too.
    Demolish,
    /// Command center: the map's own button (TAB's custom command, `sim/story.rs`) – only on maps that have one.
    Custom,
}

impl Button {
    /// TAB's picture on the button (`assets\ui\`): the portrait of what it trains or places, an icon for a command.
    /// `switching_on`: On/Off would switch the selected buildings on (then it shows "play", otherwise "pause").
    /// `None`: no picture for it (the formations), or TAB's pictures aren't there.
    fn picture(self, switching_on: bool) -> Option<Texture2D> {
        let icon_name = match self {
            Button::Train(kind) => return portrait_of(kind),
            Button::Place(kind) => return portrait_of(kind),
            Button::MakeBarrel => return portrait_of(BuildingKind::Barrel),
            Button::Planned(i) => return PLANNED[i].art.and_then(|art| ui_art::get(&format!("portraits/{art}"))),
            Button::FormLine | Button::FormBox | Button::FormCircle => return None,
            Button::Move => "IconTravel",
            Button::Stop => "IconStop",
            Button::Hold => "IconHold",
            Button::Patrol => "IconPatrol",
            Button::Attack => "IconAttack",
            Button::Rally => "IconSetRallyPoint",
            Button::Sell => "IconFResourcesToGold",
            Button::Delay(d) => return icon(&format!("CommandDelay{d}")),
            // Like TAB: the portrait of what it unlocks.
            Button::Research(t) => {
                let tech = tech(t);
                return match (tech.buildings.first(), tech.units.first()) {
                    (Some(&kind), _) => portrait_of(kind),
                    (None, Some(&kind)) => portrait_of(kind),
                    (None, None) => None,
                };
            }
            Button::Scan => "IconFWatchRange",
            Button::Drop => "IconLeaveObject",
            Button::Dismiss => "IconDismissUnit",
            Button::Open(Submenu::Colonists) => "IconDwellings",
            Button::Open(Submenu::Resources) => "IconResources",
            Button::Open(Submenu::Energy) => "IconEnergy",
            Button::Open(Submenu::Industry) => "IconIndustry",
            Button::Open(Submenu::Military) => "IconMilitary",
            Button::Open(Submenu::Defense) => "IconDeffense",
            Button::Open(Submenu::Wonders) => "IconWonders",
            Button::Back => "IconBack",
            Button::Unload => "IconExtractAll",
            Button::Repair => "IconRepair",
            Button::Switch if switching_on => "IconPlay",
            Button::Switch => "IconPause",
            Button::Demolish => "IconDestroyBuilding",
            Button::Custom => "CustomCommandGear",
        };
        icon(icon_name)
    }
}

/// The build menus of the command center – the keys on the left of the keyboard, like TAB's build categories.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Submenu {
    /// Q: houses for the colonists.
    Colonists,
    /// W: gathering buildings.
    Resources,
    /// E: power plants and the grid (DESIGN.md 5.5).
    Energy,
    /// A: industry and trade.
    Industry,
    /// S: military.
    Military,
    /// D: walls, gates, towers, traps.
    Defense,
    /// Y: wonders – the most expensive buildings, with very strong buffs.
    Wonders,
}

impl Submenu {
    /// Its spot on the command center's card: Q W E / A S D / Y.
    fn slot(self) -> usize {
        match self {
            Submenu::Colonists => 0,
            Submenu::Resources => 1,
            Submenu::Energy => 2,
            Submenu::Industry => 5,
            Submenu::Military => 6,
            Submenu::Defense => 7,
            Submenu::Wonders => 10,
        }
    }
}

/// A building that's planned but not in the game yet (DESIGN.md 6): which menu, which spot, its name on the button
/// (short), its full name and what it will do.
pub struct Planned {
    pub menu: Submenu,
    pub slot: usize,
    pub short: &'static str,
    pub name: &'static str,
    pub help: &'static str,
    /// Its portrait in `assets\ui\portraits\` – TAB's building of that kind, or none.
    pub art: Option<&'static str>,
}

/// Shorter to write the table below.
const fn plan(menu: Submenu, slot: usize, short: &'static str, name: &'static str, help: &'static str, art: Option<&'static str>) -> Planned {
    Planned { menu, slot, short, name, help, art }
}

/// All planned buildings, menu by menu. When one gets built, its line goes and a real button takes its spot.
pub const PLANNED: [Planned; 1] = [plan(Submenu::Energy, 4, "Oil gen.", "Oil burning generator", "Burns oil: a lot of power.", None)];

/// The buttons for units – top row like in StarCraft, the formations at the bottom.
pub const UNIT_BUTTONS: [Button; 9] = [
    Button::Move,
    Button::Stop,
    Button::Hold,
    Button::Patrol,
    Button::Attack,
    Button::FormLine,
    Button::FormBox,
    Button::FormCircle,
    Button::Dismiss,
];
/// The buttons of the command center: one per build menu, Q W E / A S D / Y – G repair. It builds, the barracks
/// trains. (The scan is the radar's.)
pub const COMMAND_CENTER_BUTTONS: [Button; 8] = [
    Button::Open(Submenu::Colonists),
    Button::Open(Submenu::Resources),
    Button::Open(Submenu::Energy),
    Button::Open(Submenu::Industry),
    Button::Open(Submenu::Military),
    Button::Open(Submenu::Defense),
    Button::Open(Submenu::Wonders),
    Button::Repair,
];
/// The buttons of the radar: its scan (C).
pub const RADAR_BUTTONS: [Button; 2] = [Button::Scan, Button::Repair];
/// The buttons of the barracks.
pub const BARRACKS_BUTTONS: [Button; 7] = [
    Button::Train(UnitKind::Soldier),
    Button::Train(UnitKind::Ranger),
    Button::Train(UnitKind::Spearman),
    Button::Train(UnitKind::Sniper),
    Button::Train(UnitKind::Medic),
    Button::Rally,
    Button::Repair,
];
/// The buttons of the inn – the mercenaries on the keys of their kind in the barracks.
pub const INN_BUTTONS: [Button; 5] = [
    Button::Train(UnitKind::MercSoldier),
    Button::Train(UnitKind::MercRanger),
    Button::Train(UnitKind::MercSniper),
    Button::Rally,
    Button::Repair,
];
/// The buttons of the engineering center – TAB's advanced units.
pub const ENGINEERING_BUTTONS: [Button; 5] = [
    Button::Train(UnitKind::Lucifer),
    Button::Train(UnitKind::Thanatos),
    Button::Train(UnitKind::Titan),
    Button::Rally,
    Button::Repair,
];
/// The buttons of the munitions factory.
pub const MUNITIONS_BUTTONS: [Button; 2] = [Button::MakeBarrel, Button::Repair];
/// The buttons of a market.
pub const MARKET_BUTTONS: [Button; 2] = [Button::Sell, Button::Repair];
/// The buttons of a mine trap: its delay, like TAB's four.
pub const MINE_BUTTONS: [Button; 5] = [Button::Delay(0), Button::Delay(1), Button::Delay(2), Button::Delay(3), Button::Repair];
/// The buttons of a tower.
pub const TOWER_BUTTONS: [Button; 3] = [Button::Unload, Button::Rally, Button::Repair];
/// Buildings that do nothing else (tent, wall, gate): only repair.
pub const REPAIR_BUTTONS: [Button; 1] = [Button::Repair];

/// What a build menu shows: the buildings that are in the game, the planned ones (greyed out) and "back".
pub fn menu_buttons(menu: Submenu) -> Vec<Button> {
    use BuildingKind::*;
    let built: &[BuildingKind] = match menu {
        Submenu::Colonists => &[Tent, WoodHouse, StoneHouse],
        Submenu::Resources => &[HunterCottage, FishingHut, Sawmill, Mine, Quarry, Farm, BigFarm, BigQuarry, OilPlatform],
        Submenu::Energy => &[TeslaTower, Windmill, CharcoalBurner, PowerPlant, BigMill],
        Submenu::Industry => &[WoodWorkshop, StoneWorkshop, Foundry, Inn, Market, Bank, Warehouse],
        Submenu::Military => &[Barracks, MunitionsFactory, EngineeringCenter, LookoutTower, Radar],
        Submenu::Defense => &[
            Wall, Gate, Tower, Stakes, Barrel, StoneWall, BladeTrap, StoneTower, Ballista, MachineGunTower, MineTrap, ShockingTower, Executor,
        ],
        Submenu::Wonders => &[Spire, Academy, Victorious, Transmutator, CrystalPalace],
    };
    let built = built.iter().map(|&kind| Button::Place(kind));
    let planned = (0..PLANNED.len()).filter(|&i| PLANNED[i].menu == menu).map(Button::Planned);
    built.chain(planned).chain([Button::Back]).collect()
}

/// The build menus, in the order of their buttons on the command center.
pub fn build_menus() -> impl Iterator<Item = Submenu> {
    COMMAND_CENTER_BUTTONS.into_iter().filter_map(|button| match button {
        Button::Open(menu) => Some(menu),
        _ => None,
    })
}

/// The card of a selected building (not the command center: its card depends on the build menu, `Ui::card`).
pub fn building_buttons(kind: BuildingKind) -> Vec<Button> {
    let mut card = match kind {
        BuildingKind::Barracks => BARRACKS_BUTTONS.to_vec(),
        BuildingKind::Inn => INN_BUTTONS.to_vec(),
        BuildingKind::EngineeringCenter => ENGINEERING_BUTTONS.to_vec(),
        BuildingKind::MunitionsFactory => MUNITIONS_BUTTONS.to_vec(),
        BuildingKind::Market => MARKET_BUTTONS.to_vec(),
        BuildingKind::MineTrap => MINE_BUTTONS.to_vec(),
        BuildingKind::Radar => RADAR_BUTTONS.to_vec(),
        BuildingKind::Tower | BuildingKind::StoneTower => TOWER_BUTTONS.to_vec(),
        _ => REPAIR_BUTTONS.to_vec(),
    };
    // The workshops: their researches (`Ui::card` leaves out the ones done or going on).
    card.extend(researched_in(kind).map(Button::Research));
    // Everything that runs can be switched off (Y).
    if kind.can_switch_off() {
        card.push(Button::Switch);
    }
    // Everything can be torn down – but not the command center: losing it loses the round.
    if kind != BuildingKind::CommandCenter {
        card.push(Button::Demolish);
    }
    card
}

/// Every card there can be – to find a key that's twice on one card (`keys.rs`). The buildings' cards each with
/// On/Off: whether a kind can be switched off depends on `data\`, and a clash that can't happen yet is still worth
/// telling.
pub fn all_cards() -> Vec<Vec<Button>> {
    let mut cards = vec![[UNIT_BUTTONS.as_slice(), &[Button::Drop]].concat(), COMMAND_CENTER_BUTTONS.to_vec()];
    cards.extend(build_menus().map(menu_buttons));
    for buttons in [&BARRACKS_BUTTONS[..], &INN_BUTTONS, &ENGINEERING_BUTTONS, &MUNITIONS_BUTTONS, &MARKET_BUTTONS, &MINE_BUTTONS, &RADAR_BUTTONS, &TOWER_BUTTONS, &REPAIR_BUTTONS] {
        cards.push([buttons, &[Button::Switch, Button::Demolish]].concat());
    }
    // The workshops with all their researches.
    for kind in [BuildingKind::WoodWorkshop, BuildingKind::StoneWorkshop, BuildingKind::Foundry] {
        cards.push(building_buttons(kind));
    }
    // The command center's card with the map's own button, and in the editor (`Ui::card`).
    cards.push([COMMAND_CENTER_BUTTONS.as_slice(), &[Button::Custom]].concat());
    cards.push([COMMAND_CENTER_BUTTONS.as_slice(), &[Button::Demolish]].concat());
    cards
}

/// StarCraft's grid layout: the keys of the 15 spots of the command card, row by row – the left part of the
/// keyboard. macroquad reads keys by their place on a US keyboard: on a German one the first key of the bottom row
/// is Y.
const GRID_KEYS: [KeyCode; 15] = [
    KeyCode::Q, KeyCode::W, KeyCode::E, KeyCode::R, KeyCode::T,
    KeyCode::A, KeyCode::S, KeyCode::D, KeyCode::F, KeyCode::G,
    KeyCode::Z, KeyCode::X, KeyCode::C, KeyCode::V, KeyCode::B,
];

impl Button {
    /// The name on the button.
    pub fn name(self) -> &'static str {
        match self {
            Button::Move => "Move",
            Button::Stop => "Stop",
            Button::Hold => "Hold",
            Button::Patrol => "Patrol",
            Button::Attack => "Attack",
            Button::FormLine => "Line",
            Button::FormBox => "Box",
            Button::FormCircle => "Circle",
            Button::Train(kind) => &kind.stats().name,
            Button::Rally => "Rally",
            Button::MakeBarrel => "Barrel",
            Button::Sell => "Selling",
            Button::Research(t) => &tech(t).name,
            Button::Delay(0) => "No delay",
            Button::Delay(1) => "Delay 1",
            Button::Delay(2) => "Delay 2",
            Button::Delay(_) => "Delay 3",
            Button::Scan => "Scan",
            Button::Drop => "Drop",
            Button::Dismiss => "Dismiss",
            Button::Open(Submenu::Colonists) => "Colonists",
            Button::Open(Submenu::Resources) => "Resources",
            Button::Open(Submenu::Energy) => "Energy",
            Button::Open(Submenu::Industry) => "Industry",
            Button::Open(Submenu::Military) => "Military",
            Button::Open(Submenu::Defense) => "Defense",
            Button::Open(Submenu::Wonders) => "Wonders",
            Button::Back => "Back",
            Button::Place(BuildingKind::HunterCottage) => "Hunter",
            Button::Place(BuildingKind::FishingHut) => "Fishing",
            Button::Place(BuildingKind::CharcoalBurner) => "Charcoal",
            Button::Place(BuildingKind::WoodWorkshop) => "Wood sh.",
            Button::Place(BuildingKind::TeslaTower) => "Tesla",
            Button::Place(BuildingKind::StoneWorkshop) => "Stone sh.",
            Button::Place(BuildingKind::MunitionsFactory) => "Munitions",
            Button::Place(BuildingKind::StoneHouse) => "Stone h.",
            Button::Place(BuildingKind::StoneTower) => "St. tower",
            Button::Place(BuildingKind::Stakes) => "Stakes",
            Button::Place(BuildingKind::MachineGunTower) => "MG tower",
            Button::Place(kind) => &kind.stats().name,
            Button::Planned(i) => PLANNED[i].short,
            Button::Unload => "Unload",
            Button::Repair => "Repair",
            Button::Switch => "On/Off",
            Button::Demolish => "Demolish",
            Button::Custom => "Command",
        }
    }

    /// The full name – in the tooltip and in messages.
    pub fn title(self) -> &'static str {
        match self {
            Button::Planned(i) => PLANNED[i].name,
            Button::Place(kind) => &kind.stats().name,
            Button::Open(Submenu::Industry) => "Industry and trade",
            _ => self.name(),
        }
    }

    /// The key until it's changed in the key mapping (`keys.rs`): for units their own letters (M = move, S = stop …),
    /// for buildings the grid (`GRID_KEYS`).
    pub fn default_key(self) -> KeyCode {
        match self {
            Button::Move => KeyCode::M,
            Button::Stop => KeyCode::S,
            Button::Hold => KeyCode::H,
            Button::Patrol => KeyCode::P,
            Button::Attack => KeyCode::T,
            Button::FormLine => KeyCode::X,
            Button::FormBox => KeyCode::C,
            Button::FormCircle => KeyCode::V,
            Button::Drop => KeyCode::D,
            // Not a letter: sending units away or tearing buildings down shouldn't happen by a slip of the finger.
            Button::Dismiss | Button::Demolish => KeyCode::Delete,
            Button::Custom => KeyCode::V,
            _ => GRID_KEYS[self.slot()],
        }
    }

    /// Spot on the command card: 0–4 top row, 5–9 middle, 10–14 bottom. For buildings this also decides the key.
    fn slot(self) -> usize {
        match self {
            Button::Move | Button::Unload | Button::MakeBarrel | Button::Sell => 0,
            // TAB's spots: one after the other in the top row.
            Button::Delay(d) => d as usize,
            Button::Research(t) => tech(t).slot,
            Button::Train(kind) => train_slot(kind),
            Button::Place(kind) => place_slot(kind),
            Button::Stop => 1,
            Button::Hold => 2,
            Button::Patrol => 3,
            Button::Attack => 4,
            Button::Drop => 7,
            Button::Scan => 12,
            Button::Open(menu) => menu.slot(),
            Button::Planned(i) => PLANNED[i].slot,
            // G, right above Delete (tearing down) – F was in the middle of the workshops' researches.
            Button::Repair => 9,
            Button::Switch => 10,
            Button::Rally => 8,
            Button::FormLine => 10,
            Button::FormBox => 11,
            Button::FormCircle => 12,
            Button::Back | Button::Dismiss | Button::Demolish => 14,
            Button::Custom => 13,
        }
    }

    /// Which formation shape this button chooses – `None` for all other buttons.
    pub fn formation(self) -> Option<Shape> {
        match self {
            Button::FormLine => Some(Shape::Line),
            Button::FormBox => Some(Shape::Box),
            Button::FormCircle => Some(Shape::Circle),
            _ => None,
        }
    }

    /// Which unit this button trains – `None` for all other buttons.
    pub fn trains(self) -> Option<UnitKind> {
        match self {
            Button::Train(kind) => Some(kind),
            _ => None,
        }
    }

    /// What the button costs in gold – the unit it trains, the building it places, a barrel (`None`: it costs nothing).
    pub fn cost(self) -> Option<u32> {
        if let Button::Research(t) = self {
            return Some(tech(t).gold);
        }
        match (self.trains(), self.places()) {
            (Some(kind), _) => Some(kind.stats().cost),
            (_, Some(kind)) => Some(kind.stats().cost),
            _ if self == Button::MakeBarrel => Some(data::get().buildings.blast.cost),
            _ => None,
        }
    }

    /// How many workers the button takes – the unit it trains or the building it places (0: none).
    pub fn workers(self) -> u32 {
        match (self.trains(), self.places()) {
            (Some(kind), _) => kind.stats().worker_cost,
            (_, Some(kind)) => kind.stats().worker_cost,
            _ => 0,
        }
    }

    /// Which building this button puts on the grid – `None` for all other buttons.
    pub fn places(self) -> Option<BuildingKind> {
        match self {
            Button::Place(kind) => Some(kind),
            _ => None,
        }
    }

    fn help(self) -> &'static str {
        match self {
            Button::Custom => "The map's own command (its story) - what it does, the map says.",
            Button::Move => "Walk there without shooting. Drag: in formation. On a unit: follow it.",
            Button::Stop => "Cancel all commands.",
            Button::Hold => "Stand still and only shoot what comes into range.",
            Button::Patrol => "Walk back and forth, fight everything on the way. Drag: in formation.",
            Button::Attack => "Walk there and fight everything on the way. Drag: in formation. On a zombie: exactly that one. Medics: on one of your units: beam at it.",
            Button::FormLine => "Formation: rows along the line you drag with the right mouse button. Soldiers in front.",
            Button::FormBox => "Formation: a square block, the dragged line is its front. Soldiers in front.",
            Button::FormCircle => "Formation: a ring around where you start dragging, the drag is the radius. Soldiers outside.",
            Button::Train(kind) => train_help(kind),
            Button::Rally => "Where new units go (tower: where units go when unloaded). On a unit: follow it. Also with right click.",
            Button::MakeBarrel => "An explosive barrel into the store (6 per factory). Set it from the defence menu, or right click the factory with units: each takes one along. Ctrl: 5 at once, Shift: on credit, hold the key: more.",
            Button::Research(_) => "Research it once, and from then on it can be built - for the rest of the round, even if the workshop falls. One research at a time per workshop; it needs power. Shift: on credit.",
            Button::Delay(0) => "Goes off at once when a zombie steps on it.",
            Button::Delay(1) => "Goes off 0.5 s after a zombie steps on it - more of the crowd behind it is in the blast.",
            Button::Delay(2) => "Goes off 1 s after a zombie steps on it - more of the crowd behind it is in the blast.",
            Button::Delay(_) => "Goes off 1.5 s after a zombie steps on it - more of the crowd behind it is in the blast.",
            Button::Sell => "Selling on/off. On (lit): at every payout the market sells what lies above 80 % of the storage - up to 1 good per 4 colonists of its houses. Off: it keeps everything, its houses still pay more.",
            Button::Scan => "Click anywhere on the map: a circle of 10 tiles is shown for 10 s, over forests too - no noise. 2 charges, a new one every 120 s - while one charges, the radar takes 30 power from its grid.",
            Button::Drop => "Put the barrels down where the units stand. Any damage and a barrel goes off - and a unit carrying one goes off when it dies.",
            Button::Dismiss => "Send the selected units away for good: no more upkeep, their workers are free again. No gold back, a barrel they carry is put down.",
            Button::Open(Submenu::Colonists) => "Houses for your colonists: the tent and the stone house.",
            Button::Open(Submenu::Resources) => "Gathering: hunter cottage, fishing hut, sawmill, mine, farm - later the oil pump.",
            Button::Open(Submenu::Energy) => "Power: tesla tower, windmill, charcoal burner - later the oil burning generator.",
            Button::Open(Submenu::Industry) => "The wood and the stone workshop, the foundry (iron), the inn (mercenaries), the market and the bank.",
            Button::Open(Submenu::Military) => "The barracks and the munitions factory - later the siege factory.",
            Button::Open(Submenu::Defense) => "Wall, gate, tower, stake trap, explosive barrel, stone wall, stone tower, ballista, machine gun tower.",
            Button::Open(Submenu::Wonders) => "The most expensive buildings, with very strong buffs - none designed yet.",
            Button::Back => "Back to the main buttons (B or Esc).",
            Button::Place(kind) => place_help(kind),
            Button::Planned(i) => PLANNED[i].help,
            Button::Unload => "Everyone out of the tower - on the side of the rally point, then they walk there.",
            Button::Repair => "Repair all selected buildings: pay gold for what's missing, it comes back over a few seconds. An infected building is cured once it's whole - it's yours again and pays gold.",
            Button::Switch => "Switch the selected buildings off: no upkeep, no workers, no power - and they do nothing. Again: on - if there are enough free workers, spare power and gold income for its upkeep.",
            Button::Demolish => "Tear them down at once: part of the price comes back, more while still being built. Not the command center, not infected ones.",
        }
    }
}

/// The spot of a building's button in its build menu (`menu_buttons`).
fn place_slot(kind: BuildingKind) -> usize {
    use BuildingKind::*;
    match kind {
        Tent | Barracks | Wall | HunterCottage | TeslaTower | WoodWorkshop | CommandCenter | Spire => 0,
        Gate | FishingHut | Windmill | WoodHouse | StoneWorkshop | MunitionsFactory | Academy => 1,
        Tower | Sawmill | CharcoalBurner | Foundry | StoneHouse | EngineeringCenter | Victorious => 2,
        Mine | Stakes | Inn | PowerPlant | LookoutTower | Transmutator => 3,
        Market | Barrel | Quarry | Radar | CrystalPalace => 4,
        // Below their wooden ones – and the farm below the hunter, the other food. The bank next to the market.
        StoneWall | Farm | Bank => 5,
        // The better ones below the first ones: the big farm next to the farm, the iron mill below the windmill, the
        // blades below the gate.
        BigFarm | BigMill | Warehouse | BladeTrap => 6,
        StoneTower | BigQuarry => 7,
        // F and G, next to the stone tower.
        Ballista | OilPlatform => 8,
        MachineGunTower => 9,
        // The bottom row: TAB's late defence.
        MineTrap => 10,
        ShockingTower => 11,
        Executor => 12,
    }
}

/// The spot of a unit's button: the mercenaries on the key of their kind in the barracks.
fn train_slot(kind: UnitKind) -> usize {
    match kind {
        UnitKind::Soldier | UnitKind::MercSoldier => 0,
        UnitKind::Ranger | UnitKind::MercRanger => 1,
        UnitKind::Spearman => 2,
        UnitKind::Sniper | UnitKind::MercSniper => 3,
        UnitKind::Medic => 4,
        // The engineering center's.
        UnitKind::Lucifer => 0,
        UnitKind::Thanatos => 1,
        UnitKind::Titan => 2,
    }
}

/// The tooltip of a unit's button.
fn train_help(kind: UnitKind) -> &'static str {
    match kind {
        UnitKind::Soldier => "Tough, short range, aims quickly, loud. Firearm: needs a munitions factory. Build time: 5 s. Ctrl: 5 at once, Shift: on credit, hold the key: more.",
        UnitKind::Ranger => "Fast and weak, draws the bow a while before every arrow, quiet. Build time: 4 s. Ctrl: 5 at once, Shift: on credit, hold the key: more.",
        UnitKind::Sniper => "Long range, one big shot - very loud. Firearm: needs a munitions factory. Build time: 8 s. Ctrl: 5 at once, Shift: on credit, hold the key: more.",
        UnitKind::Medic => "No attack. Heals hurt units nearby with its beam - but only those that haven't been hit for a few seconds: pull the wounded back. Build time: 6 s.",
        UnitKind::Spearman => "Melee with a spear: every hit pushes the zombie back, nearly silent - holds a line or a gate early on. Short reach: spitters and infested shoot it to pieces. Build time: 4 s. Ctrl: 5 at once, Shift: on credit, hold the key: more.",
        UnitKind::MercSoldier => "Mercenary soldier: half again as tough, hits harder and shoots faster than a soldier. Needs no worker - but double upkeep, and in debt mercenaries are the first to leave. Needs a munitions factory. Ctrl: 5 at once, Shift: on credit.",
        UnitKind::MercRanger => "Mercenary ranger: half again as tough, hits harder and shoots faster than a ranger, quiet. Needs no worker - but double upkeep, and in debt mercenaries are the first to leave. Ctrl: 5 at once, Shift: on credit.",
        UnitKind::MercSniper => "Mercenary sniper: half again as tough and a bigger shot than a sniper - very loud. Needs no worker - but double upkeep, and in debt mercenaries are the first to leave. Needs a munitions factory. Ctrl: 5 at once, Shift: on credit.",
        UnitKind::Lucifer => "Lucifer: a flamethrower - a cone of fire 3.5 tiles long hits everything in it and sets zombies on fire. Burns your own units in the cone too! Fireproof itself, tough. Costs oil, eats oil every payout.",
        UnitKind::Thanatos => "Thanatos: a missile launcher - a rocket up to 10 tiles far blows up a crowd (1.8 tiles round), but not closer than 3 tiles; close up it hits with the launcher. Aims long, very loud. Costs iron and oil.",
        UnitKind::Titan => "Titan: a walking cannon - hard shots 9 tiles far that hit a little area, very tough, fast. Costs a lot of gold, iron and oil, eats oil every payout.",
    }
}

/// The tooltip of a building's button: what it does. The price is in the title above it.
fn place_help(kind: BuildingKind) -> &'static str {
    match kind {
        BuildingKind::Tent => "Tent (2 x 2). 4 colonists eat food at every payout and then pay gold, 2 workers for your buildings and units; shield down = infected. Needs power. Drag: a line.",
        BuildingKind::Barracks => "Barracks (3 x 3, 800 HP). Trains soldiers, rangers, snipers and medics - the only building that does. Needs power: without enough it trains slower.",
        BuildingKind::Wall => "Wall (1 x 1, 400 HP). Zombies have to tear it down. Drag: a line, diagonal too. Right click / Esc: stop building.",
        BuildingKind::Gate => "Gate (3 x 1, 600 HP). Your units walk through the middle, zombies don't. Can go on walls (cheaper there, but walls + gate still cost more than the gate alone). R: turn.",
        BuildingKind::Tower => "Tower (1 x 1, 800 HP). Right click it with units: up to 4 go in, shoot and see 2 tiles further and can't be hit. Needs 1 free tile to the next tower. Drag: a line.",
        BuildingKind::Sawmill => "Sawmill (2 x 2). Wood from the forest around it - the frame shows how far. Two sawmills share the forest they both reach. Needs power.",
        BuildingKind::Mine => "Mine (2 x 2). Iron ore, coal, stone and gold (like TAB: 20 times as much, straight into your gold) - whatever lies under it and up to 2 tiles around it (6 x 6, the frame). Put it right on the ore. Needs power.",
        BuildingKind::HunterCottage => "Hunter cottage (1 x 1). Food from the game in the forest around it. Needs power.",
        BuildingKind::FishingHut => "Fishing hut (1 x 1). Food from the water around it. Needs power.",
        BuildingKind::Farm => "Farm (3 x 3). Plants fields on the grass around it - food, a lot of it on open grass. Nothing grows on dirt (brown) or ore, and the farm itself has to stand on grass. Needs power and a wood workshop.",
        BuildingKind::Windmill => "Windmill (2 x 2). A lot of power, needs nothing but its upkeep. Has to stand inside the grid. Needs a wood workshop.",
        BuildingKind::CharcoalBurner => "Charcoal burner (2 x 2). Power, but it burns wood at every payout - without wood, none. Has to stand inside the grid.",
        BuildingKind::WoodWorkshop => "Wood workshop (2 x 2). Makes nothing, but while one stands you can build the farm, the windmill, the stake trap, the ballista and the stone workshop. Needs power.",
        BuildingKind::TeslaTower => "Tesla tower (1 x 1). Carries the power further: a new circle of the grid around it. Has to stand inside the grid.",
        BuildingKind::StoneWorkshop => "Stone workshop (2 x 2). Makes nothing, but while one stands you can build the munitions factory, the foundry, the stone house, stone walls and stone towers. Needs power and a wood workshop.",
        BuildingKind::Foundry => "Foundry (3 x 3). While one stands you can build the machine gun tower - and while it runs it smelts iron: up to 3 per payout, out of 2 iron ore and 1 coal each. Loud. Needs power and a stone workshop.",
        BuildingKind::MunitionsFactory => "Munitions factory (2 x 2). Makes nothing (ammunition is free), but while one stands the barracks can train soldiers and snipers - firearms. Needs power and a stone workshop.",
        BuildingKind::StoneHouse => "Stone house (2 x 2). 16 colonists eat 8 food at every payout and pay gold, 8 workers. Goes on a tent too (an upgrade). Needs power and a stone workshop.",
        BuildingKind::StoneWall => "Stone wall (1 x 1, 1000 HP). Goes on a wall too (an upgrade). Drag: a line. Needs a stone workshop.",
        BuildingKind::StoneTower => "Stone tower (1 x 1, 2000 HP). Like the tower - up to 4 units inside - but much tougher. Goes on a tower too. Needs a stone workshop.",
        BuildingKind::Stakes => "Stake trap (1 x 1). Zombies walk over it without attacking it and get hurt; every hit wears it down. Your units walk over it unharmed. Drag: a line. Needs a wood workshop.",
        BuildingKind::CommandCenter => "The heart of the colony.",
        BuildingKind::Market => "Market (3 x 3). Houses whose middle lies within 2 tiles of it pay +50 % gold (one market per house). At every payout it also sells what lies above 80 % of the storage - 1 good per 4 colonists of its houses (Q on it: selling on/off). Grows with your houses.",
        BuildingKind::Bank => "Bank (2 x 2, 1200 HP). +2000 gold storage and 500 credit: when only gold is missing, hold Shift and you buy into it - no debt penalties, but 10 % interest per payout. If it falls, its share of that debt is gone - and the gold above the smaller storage. Can't be switched off. Needs a stone workshop.",
        BuildingKind::Inn => "Inn (3 x 3). Hires mercenaries: stronger than veterans, dearer, double upkeep - but they need no workers. In debt they are the first to leave. Needs power and a stone workshop.",
        BuildingKind::Barrel => "Explosive barrel (1 x 1). Takes one from the munitions factory's store. Any damage and it goes off: 200 damage to everyone within 2.5 tiles - other barrels too - and very loud. Drag: a line.",
        BuildingKind::Ballista => "Ballista (2 x 2, 700 HP). Shoots by itself: a heavy bolt every 2.5 s, 9 tiles far, always at the toughest zombie in range - fat ones, riots, giants. No power needed. Needs a wood workshop.",
        BuildingKind::MachineGunTower => "Machine gun tower (1 x 1, 900 HP). Shoots by itself: five shots a second at the nearest zombie - against crowds. Loud. Costs iron from the foundry, needs power and a foundry.",
        BuildingKind::WoodHouse => "Wood house (2 x 2). 8 colonists eat 8 food at every payout and pay gold, 4 workers - between tent and stone house. Goes on a tent (an upgrade), a stone house goes on it. Needs power and a wood workshop.",
        BuildingKind::PowerPlant => "Power plant (3 x 3). A lot of power - but it burns 10 wood at every payout, without wood none. Loud. Has to stand inside the grid. Needs a stone workshop.",
        BuildingKind::OilPlatform => "Oil platform (2 x 2). Pumps oil - it has to stand right on an oil node (the dark pools). Needs power and a foundry.",
        BuildingKind::Quarry => "Quarry (2 x 2). Stone from the stone patches under it and up to 2 tiles around it - only stone. Put it right on the stone. Needs power.",
        BuildingKind::BigQuarry => "Advanced quarry (2 x 2). Like the quarry, twice the stone per tile. Costs oil, needs power and a foundry.",
        BuildingKind::BigFarm => "Advanced farm (2 x 2). Like the farm, twice the food per field - on grass only. Costs oil, needs power and a foundry.",
        BuildingKind::BigMill => "Iron mill (2 x 2). Twice the windmill's power, needs nothing but its upkeep. Has to stand inside the grid. Needs a foundry.",
        BuildingKind::Warehouse => "Warehouse (4 x 4). +50 room for every good (not food, not gold) - and the gathering buildings within 12 tiles of it bring in 20 % more. Needs power.",
        BuildingKind::EngineeringCenter => "Engineering center (3 x 3). Trains Lucifer, Thanatos and the Titan. Needs power and a foundry, costs oil.",
        BuildingKind::ShockingTower => "Shocking tower (2 x 2). Charges for 4.5 s, then hits every zombie within 6 tiles at once (60 each). Needs a lot of power and a stone workshop.",
        BuildingKind::Executor => "Executor (2 x 2). Shoots by itself: a heavy shot twice a second, 9 tiles far, that hits everything round its target. Needs power and a foundry, costs oil.",
        BuildingKind::BladeTrap => "Blade trap (1 x 1). Like the stake trap, harder (10 per hit) and tougher. Costs iron, needs a foundry. Drag: a line.",
        BuildingKind::MineTrap => "Mine (1 x 1). Goes off under the first zombie that steps on it: 500 damage to everyone within 1.9 tiles - your units and buildings too - and it's gone. Costs iron, needs a stone workshop. Drag: a line.",
        BuildingKind::Radar => "Radar (1 x 1). Sees 40 tiles far - over forest too - and scans (C): a circle anywhere on the map. Needs power and a foundry.",
        BuildingKind::LookoutTower => "Lookout tower (1 x 1). Sees 24 tiles far - over forest too. Needs a wood workshop.",
        BuildingKind::Spire => "The Spire (5 x 5, wonder). 800 power that reaches 30 tiles, sees 36 tiles over forest. Only one. Needs a foundry.",
        BuildingKind::Academy => "The Academy (4 x 4, wonder). While it works every one of your units is a veteran. Only one. Needs power and a stone workshop.",
        BuildingKind::Victorious => "The Victorious (4 x 4, wonder). A fortress (10,000 HP, armor) - houses within 24 tiles of it pay 20 % more gold. Eats 10 oil every payout. Only one. Needs power and a stone workshop.",
        BuildingKind::Transmutator => "The Transmutator (5 x 5, wonder). Turns 20 wood, 20 stone and 10 iron into 40 oil at every payout. Only one. Needs a lot of power and a foundry.",
        BuildingKind::CrystalPalace => "The Crystal Palace (6 x 6, wonder). 60 colonists - and 800 food at every payout, out of nothing. Only one. Needs a wood workshop.",
    }
}

/// What lies under the mouse.
#[derive(Clone, Copy, PartialEq)]
pub enum HudHit {
    /// A point on the minimap – already converted to world coordinates.
    Minimap(Vec2),
    /// The pause button of the clock.
    Pause,
    /// The grid button next to the stock.
    GridView,
    /// Camera to the command center (Backspace).
    Base,
    /// The menu (Esc).
    Menu,
    /// A control group (0 = key 1 … 9 = key 0).
    Group(usize),
    /// An icon in the selection – the index in `Ui::selection_view` (units and buildings).
    Icon(usize),
    /// An entry in the production queue of the selected building.
    Queue(usize),
    /// "Repair all" in the stock (X).
    RepairAll,
    Button(Button),
    /// Anywhere else on a panel.
    Panel,
}

thread_local! {
    /// The factor of the bar while it's being drawn (`draw`) – 1 otherwise. `draw_text` and the mouse below use it.
    static DRAWING: Cell<f32> = const { Cell::new(1.0) };
}

/// How much bigger than on TAB's 1280 x 720 the bar is drawn: as much as the map's part of the window (`view.rs` – in
/// the game all of it) is bigger – by its width, or by its height if that's less (wider than 16 : 9). Never below half.
pub fn scale() -> f32 {
    let view = crate::view::rect();
    (view.w / REF_W).min(view.h / REF_H).max(0.5)
}

// The bar is drawn in TAB's pixels, and a camera makes them bigger (`draw`). Text is the exception: drawn at the
// small size and then blown up it would be blurry. So these two replace macroquad's functions of the same name in
// this file – a function defined here wins over one that comes in with `use macroquad::prelude::*` (a `*` import
// only fills in names the file doesn't have itself). They put the letters together at the real size on screen and
// shrink them back into TAB's pixels: sharp at any size.

fn draw_text(text: impl AsRef<str>, x: f32, y: f32, size: f32, color: Color) {
    let (font_size, font_scale) = text_size(size);
    draw_text_ex(text.as_ref(), x, y, TextParams { font_size, font_scale, color, ..Default::default() });
}

fn measure_text(text: &str, font: Option<&Font>, size: u16, scale: f32) -> TextDimensions {
    let (font_size, font_scale) = text_size(size as f32 * scale);
    macroquad::text::measure_text(text, font, font_size, font_scale)
}

/// Text `size` TAB pixels high: the size of its letters on screen, and the factor back into TAB's pixels.
fn text_size(size: f32) -> (u16, f32) {
    let real = (size * DRAWING.get()).round().max(1.0);
    (real as u16, size / real)
}

/// The mouse in TAB's pixels, while the bar is drawn – counted from the corner of the map's part of the window.
fn mouse() -> Vec2 {
    (Vec2::from(mouse_position()) - crate::view::rect().point()) / DRAWING.get()
}

/// Where everything lies, in TAB's pixels (`scale` times bigger on screen). Computed anew every frame, in case the
/// window changes size.
pub struct Layout {
    /// The bar in screen pixels, from the top of its frames down (the editor's hint over the map stays above it).
    pub panel: Rect,
    /// Where TAB's corner (0, 0) lies on the screen: the corner of the map's part of the window (`view.rs`).
    origin: Vec2,
    /// TAB's pixels → screen pixels.
    s: f32,
    /// The screen in TAB's pixels: its width, and how much higher than 720 it is.
    w: f32,
    dy: f32,
    /// How much wider than 1280: the selection in the middle grows by that, the card and the stock move right.
    dx: f32,
    /// The top of the bar's frames: tooltips go above it.
    top: f32,
    /// The minimap (TAB's `@MiniMap`).
    minimap: Rect,
    /// Above the minimap: when the next wave comes (`@NextSwarm`).
    next_swarm: Rect,
    /// The selection: one thing in detail, or an icon for each of several (`@SelectionArea`).
    info: Rect,
    card: Rect,
    /// The colony's stock: colonists, gold, resources (`@ResourcesArea`).
    stock: Rect,
}

impl Layout {
    pub fn new() -> Layout {
        let s = scale();
        let view = crate::view::rect();
        let (w, h) = (view.w / s, view.h / s);
        let (dx, dy) = ((w - REF_W).max(0.0), h - REF_H);
        let at = |x: f32, y: f32, width: f32, height: f32| Rect::new(x, y + dy, width, height);
        let right = |x: f32, y: f32, width: f32, height: f32| Rect::new(x + dx, y + dy, width, height);
        let top = PLAIN_TOP + dy;
        Layout {
            panel: Rect::new(view.x, view.y + top * s, view.w, view.h - top * s),
            origin: view.point(),
            s,
            w,
            dy,
            dx,
            top,
            minimap: at(5.0, 557.0, 158.0, 158.0),
            next_swarm: at(4.0, 486.0, 64.0, 38.0),
            info: at(367.0, 591.0, 460.0 + dx, 120.0),
            card: right(824.0, 593.0, 200.0, 120.0),
            stock: right(1062.0, 612.0, 210.0, 103.0),
        }
    }

    /// TAB's 1280 x 720 position → ours: further down by what the screen is higher, right of the stretch further
    /// right by what it's wider.
    fn tab(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        let x = if x >= STRETCH.1 { x + self.dx } else { x };
        Rect::new(x, y + self.dy, w, h)
    }

    /// Is this screen point on the map (so not on the bar, and not beside the map's part of the window)? TAB's way:
    /// the bar is where its picture isn't see-through – the pipes and gears on top of it are bar, the gaps between
    /// them are map.
    pub fn in_game_view(&self, p: Vec2) -> bool {
        crate::view::rect().contains(p) && !self.covers(self.to_tab(p))
    }

    /// A screen point → TAB's pixels.
    fn to_tab(&self, p: Vec2) -> Vec2 {
        (p - self.origin) / self.s
    }

    /// Does the bar cover this point (TAB's pixels)?
    fn covers(&self, p: Vec2) -> bool {
        let bar_y = BAR_Y + self.dy;
        if p.y < bar_y {
            return false;
        }
        // Back from the stretched picture to a point on TAB's 1280 pixels.
        let x = if p.x < STRETCH.0 {
            p.x
        } else if p.x < STRETCH.1 + self.dx {
            STRETCH.0 + (p.x - STRETCH.0) * (STRETCH.1 - STRETCH.0) / (STRETCH.1 - STRETCH.0 + self.dx)
        } else {
            p.x - self.dx
        };
        match ui_art::panel_solid(x / REF_W, (p.y - bar_y) / BAR_H) {
            Some(solid) => solid,
            None => p.y >= self.top,
        }
    }

    /// What lies under the mouse? `None` = nothing of the panel, so the map.
    /// `area`: the part of the map the minimap shows (game view: inside the border, editor: everything).
    /// `selected`: how many units are selected, `queue`: how long the production queue of the
    /// selected building is, `card`: which buttons the command card shows right now.
    pub fn hit(&self, mouse: Vec2, area: Region, selected: usize, queue: usize, card: &[Button]) -> Option<HudHit> {
        let mouse = self.to_tab(mouse);
        if self.minimap.contains(mouse) {
            // Right at its edge: limit to the area.
            let p = self.minimap_to_world(mouse, area);
            return Some(HudHit::Minimap(area.clamp(p, 0.01)));
        }
        if !self.covers(mouse) {
            return None;
        }
        if self.pause_rect().contains(mouse) {
            return Some(HudHit::Pause);
        }
        for (side, hit) in SIDE_BUTTONS.iter().enumerate() {
            if self.side_rect(side).contains(mouse) {
                return Some(*hit);
            }
        }
        for g in 0..10 {
            if self.group_rect(g).contains(mouse) {
                return Some(HudHit::Group(g));
            }
        }
        if selected > 1 {
            for i in 0..selected.min(self.icon_capacity()) {
                if self.icon_rect(i).contains(mouse) {
                    return Some(HudHit::Icon(i));
                }
            }
        }
        for i in 0..queue {
            if self.queue_rect(i).contains(mouse) {
                return Some(HudHit::Queue(i));
            }
        }
        for &button in card {
            if self.button_rect(button.slot()).contains(mouse) {
                return Some(HudHit::Button(button));
            }
        }
        Some(HudHit::Panel)
    }

    // --- Minimap: the map turned the same way as on screen, only small – `area` as big as fits, not stretched ---

    /// The part of the minimap `area` takes: all of it if the area has the minimap's shape on screen, otherwise a
    /// narrower or flatter rectangle in its middle (TAB's minimap is square: a field wider than high leaves a stripe
    /// above and below). A step of the screen axes across is `TILE_W / 2` pixels, one down `TILE_H / 2`.
    fn minimap_fit(&self, area: Region) -> Rect {
        let extent = area.max - area.min;
        let on_screen = vec2(extent.x * TILE_W, extent.y * TILE_H);
        let scale = (self.minimap.w / on_screen.x).min(self.minimap.h / on_screen.y);
        let (w, h) = (on_screen.x * scale, on_screen.y * scale);
        Rect::new(self.minimap.x + (self.minimap.w - w) / 2.0, self.minimap.y + (self.minimap.h - h) / 2.0, w, h)
    }

    fn world_to_minimap(&self, p: Vec2, area: Region) -> Vec2 {
        let fit = self.minimap_fit(area);
        fit.point() + (to_frame(p) - area.min) / (area.max - area.min) * fit.size()
    }

    fn minimap_to_world(&self, m: Vec2, area: Region) -> Vec2 {
        let fit = self.minimap_fit(area);
        from_frame(area.min + (m - fit.point()) / fit.size() * (area.max - area.min))
    }

    // --- Where the individual fields lie (TAB's `MapGUI` and its clips) ---

    /// The round button between day and hour (`@ButtonPlay`).
    fn pause_rect(&self) -> Rect {
        self.tab(214.0, 638.8, 55.6, 55.6)
    }

    /// The gears behind the clock (`@ClipGears`, on a black square).
    fn gears_rect(&self) -> Rect {
        self.tab(171.0, 677.0, 136.9, 136.9)
    }

    /// The four small buttons between the card and the stock (`@ButtonCommandCenter`, `@ButtonRepairAll`,
    /// `@ButtonMayors`, `@ButtonOptions`).
    fn side_rect(&self, k: usize) -> Rect {
        let y = [599.0, 627.0, 654.0, 681.0][k];
        self.tab(1032.0, y, 28.0, 28.0)
    }

    /// The control groups one above the other (`ControlSelectionGroups`: TAB's 8 in 128 pixels – here 10): key 1 at
    /// the bottom, key 0 at the top.
    fn group_rect(&self, g: usize) -> Rect {
        let h = 128.0 / 10.0;
        let r = self.tab(326.0, 587.0, 36.0, 128.0);
        Rect::new(r.x, r.y + (9 - g.min(9)) as f32 * h, r.w, h - 1.0)
    }

    /// The portrait of a unit or zombie (`@ImagePortrait`, 3 : 4) – a building's picture is square (`@Image`).
    fn portrait(&self) -> Rect {
        Rect::new(self.info.x + 6.0, self.info.y + 12.0, 76.0, 101.3)
    }

    fn building_picture(&self) -> Rect {
        Rect::new(self.info.x - 8.0, self.info.y + 14.0, 97.6, 97.6)
    }

    /// Right of the portrait: name, life bar, a few lines of text (or the production queue) – TAB's `BuildingSheet`.
    fn text_column(&self) -> Rect {
        Rect::new(self.info.x + 88.0, self.info.y + 3.0, 180.0, self.info.h - 6.0)
    }

    /// Right of the text: the values, one small box each (TAB's `@AreaFeatures`).
    fn cells(&self) -> Rect {
        let x = self.text_column().right() + 6.0;
        Rect::new(x, self.info.y + 3.0, (self.info.right() - x - 4.0).max(0.0), self.info.h - 4.0)
    }

    fn icon_columns(&self) -> usize {
        ((self.info.w / ICON) as usize).max(1)
    }

    /// How many icons fit into the field.
    fn icon_capacity(&self) -> usize {
        self.icon_columns() * ((self.info.h - SUMMARY_H) / ICON) as usize
    }

    fn icon_rect(&self, i: usize) -> Rect {
        let cols = self.icon_columns();
        let (col, row) = ((i % cols) as f32, (i / cols) as f32);
        Rect::new(self.info.x + col * ICON, self.info.y + 2.0 + row * ICON, ICON - 3.0, ICON - 3.0)
    }

    /// The production queue (and a tower's garrison) lies right of the portrait, below the life bar.
    fn queue_rect(&self, i: usize) -> Rect {
        let text = self.text_column();
        Rect::new(text.x + i as f32 * QUEUE_ICON, text.y + 44.0, QUEUE_ICON - 3.0, QUEUE_ICON - 3.0)
    }

    fn button_rect(&self, b: usize) -> Rect {
        let (col, row) = ((b % CARD_COLS) as f32, (b / CARD_COLS) as f32);
        Rect::new(self.card.x + col * BUTTON, self.card.y + row * BUTTON, BUTTON - 2.0, BUTTON - 2.0)
    }
}

/// The four small buttons next to the stock, top to bottom: camera to the command center, repair all, the grid, the
/// menu – TAB has its mayors where we have the grid.
const SIDE_BUTTONS: [HudHit; 4] = [HudHit::Base, HudHit::RepairAll, HudHit::GridView, HudHit::Menu];

/// Draws the whole panel. `minimap`: the picture of the map (`picture.rs`), kept up to date by `game.rs`.
///
/// Everything here is drawn in TAB's pixels: the camera (`Camera2D`, like a `pygame.transform.scale` of the whole
/// surface, only done by the graphics card while drawing) shows `layout.w` of them across the map's part of the window.
pub fn draw(layout: &Layout, world: &World, camera: &Camera, ui: &Ui, minimap: &MapPicture) {
    // `from_display_rect` would show it upside down (it's made for drawing into a texture) – so by hand. A camera
    // shows the point `target` in the middle of the window, and `zoom` says how much of the window one pixel drawn
    // takes: 2 for the whole width (from -1 at one edge to 1 at the other). TAB's pixel `p` should land at `origin +
    // p · s` – so one pixel is `s` screen pixels (`zoom` = 2 s / window), and the window's middle shows the TAB pixel
    // that lies there: (middle - origin) / s. In the game `origin` is (0, 0), and it's TAB's middle.
    let window = vec2(screen_width(), screen_height());
    let target = (window / 2.0 - layout.origin) / layout.s;
    set_camera(&Camera2D { target, zoom: 2.0 * layout.s / window, ..Default::default() });
    DRAWING.set(layout.s);

    // Back to front, like TAB's layers: the gears behind the clock, the minimap, then the bar's picture with its holes
    // over both, then everything on top of it.
    let bar = ui_art::get("panel");
    if bar.is_some() {
        draw_gears(layout, world);
    }
    draw_minimap(layout, world, camera, ui, minimap);
    match &bar {
        Some(texture) => draw_bar(layout, texture),
        None => {
            let r = Rect::new(0.0, layout.top, layout.w, REF_H + layout.dy - layout.top);
            draw_rectangle(r.x, r.y, r.w, r.h, PANEL_BG);
            draw_line(r.x, r.y + 1.5, r.right(), r.y + 1.5, 3.0, BRASS);
            let m = layout.minimap;
            for f in [Rect::new(m.x - 3.0, m.y - 3.0, m.w + 6.0, m.h + 6.0), layout.tab(167.0, 590.0, 150.0, 128.0), layout.info, layout.stock] {
                frame(f);
            }
        }
    }
    draw_next_swarm(layout, world);
    draw_clock(layout, world, ui);
    draw_groups(layout, world, ui);
    draw_selection(layout, world, ui);
    draw_card(layout, world, ui, &ui.card(world));
    draw_stock(layout, world);
    draw_side_buttons(layout, world, ui);
    draw_events(layout, world, ui);

    DRAWING.set(1.0);
    set_default_camera();
}

/// TAB's bar: its picture across the bottom – on a window wider than 16 : 9 stretched in the middle (`STRETCH`).
fn draw_bar(layout: &Layout, texture: &Texture2D) {
    let y = BAR_Y + layout.dy;
    // Texture pixels per TAB pixel (the picture is 3840 wide: three times TAB's 1280).
    let k = texture.width() / REF_W;
    let piece = |from: f32, to: f32, x: f32, w: f32| {
        let source = Rect::new(from * k, 0.0, (to - from) * k, texture.height());
        draw_texture_ex(texture, x, y, WHITE, DrawTextureParams { dest_size: Some(vec2(w, BAR_H)), source: Some(source), ..Default::default() });
    };
    piece(0.0, STRETCH.0, 0.0, STRETCH.0);
    piece(STRETCH.0, STRETCH.1, STRETCH.0, STRETCH.1 - STRETCH.0 + layout.dx);
    piece(STRETCH.1, REF_W, STRETCH.1 + layout.dx, REF_W - STRETCH.1);
}

/// The gears behind the clock (TAB's clip `GearsA`: ten gears on a 500 x 500 canvas, on a black square). They turn
/// with the game's time – faster at a higher speed, still while paused. Neighbours turn the other way, a small one
/// faster than a big one, like real gears.
fn draw_gears(layout: &Layout, world: &World) {
    let Some(gear) = ui_art::get("gear") else {
        return;
    };
    let black = layout.tab(166.8, 679.3, 156.0, 57.0);
    draw_rectangle(black.x, black.y, black.w, black.h, BLACK);
    // (x, y, size on the canvas, turning: +1 clockwise (`GearA_Right`), -1 the other way (`GearA_Left`)).
    const GEARS: [(f32, f32, f32, f32); 10] = [
        (-61.5, 78.7, 123.6, 1.0),
        (287.1, 35.2, 167.1, -1.0),
        (176.2, 2.2, 123.6, -1.0),
        (447.6, 133.4, 173.9, -1.0),
        (187.4, 146.2, 141.6, -1.0),
        (49.5, 38.2, 134.1, -1.0),
        (330.6, 10.5, 179.9, -1.0),
        (18.0, 152.9, 174.6, -1.0),
        (289.4, 192.7, 182.9, -1.0),
        (164.2, 64.5, 182.9, 1.0),
    ];
    let area = layout.gears_rect();
    let k = area.w / 500.0;
    let time = world.tick as f32 / TICKS_PER_SECOND;
    for (x, y, size, turn) in GEARS {
        let side = size * k;
        let rotation = turn * time * 60.0 / size;
        let params = DrawTextureParams { dest_size: Some(vec2(side, side)), rotation, ..Default::default() };
        draw_texture_ex(&gear, area.x + x * k, area.y + y * k, Color::new(0.75, 0.6, 0.4, 1.0), params);
    }
}

/// A part of the panel without TAB's picture: dark inside, a brass edge with a rivet in every corner.
fn frame(r: Rect) {
    draw_rectangle(r.x, r.y, r.w, r.h, SECTION_BG);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, BORDER);
    draw_rectangle_lines(r.x + 3.0, r.y + 3.0, r.w - 6.0, r.h - 6.0, 1.0, Color { a: 0.35, ..BORDER });
    for (x, y) in [(r.x, r.y), (r.right(), r.y), (r.x, r.bottom()), (r.right(), r.bottom())] {
        draw_circle(x, y, 3.0, BRASS);
    }
}

/// A picture as big as fits into `r`, not stretched, in its middle. `color`: white = as painted.
fn draw_picture(texture: &Texture2D, r: Rect, color: Color) {
    let k = (r.w / texture.width()).min(r.h / texture.height());
    let size = vec2(texture.width(), texture.height()) * k;
    let at = r.center() - size / 2.0;
    draw_texture_ex(texture, at.x, at.y, color, DrawTextureParams { dest_size: Some(size), ..Default::default() });
}

/// A portrait on a square: only its top (the face) when it's higher than wide – TAB's 3 : 4 portraits.
fn draw_face(texture: &Texture2D, r: Rect, color: Color) {
    let side = texture.width().min(texture.height());
    let source = Rect::new((texture.width() - side) / 2.0, 0.0, side, side);
    draw_texture_ex(texture, r.x, r.y, color, DrawTextureParams { dest_size: Some(r.size()), source: Some(source), ..Default::default() });
}

/// The portrait of a kind (`assets\ui\portraits\`), by its name in Rust: `Soldier`, `MachineGunTower`, `Walker` …
fn portrait_of(name: impl std::fmt::Debug) -> Option<Texture2D> {
    ui_art::get(&format!("portraits/{name:?}"))
}

fn icon(name: &str) -> Option<Texture2D> {
    ui_art::get(&format!("icons/{name}"))
}

/// A button TAB's way: its brass plate (`plate`: `button_square` or `button_round`), the picture on it a bit smaller
/// (`inset`: the share left free on each side), lighter under the mouse, green while it waits, grey when it can't be
/// pressed. Without TAB's plate: our brass box. Returns whether the plate was there.
fn draw_button(r: Rect, plate: &str, picture: Option<&Texture2D>, inset: f32, hover: bool, waiting: bool, greyed: bool) -> bool {
    let round = plate == "button_round";
    let art = ui_art::get(plate);
    match &art {
        Some(texture) => draw_texture_ex(texture, r.x, r.y, if greyed { GREYED } else { WHITE }, DrawTextureParams { dest_size: Some(r.size()), ..Default::default() }),
        None => {
            draw_rectangle(r.x, r.y, r.w, r.h, if greyed { SECTION_BG } else { BOX_BG });
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, if greyed { BORDER } else { BRASS });
        }
    }
    if let Some(picture) = picture {
        let inner = Rect::new(r.x + r.w * inset, r.y + r.h * inset, r.w * (1.0 - 2.0 * inset), r.h * (1.0 - 2.0 * inset));
        draw_picture(picture, inner, if greyed { GREYED } else { WHITE });
    }
    let light = if waiting { Some(WAIT_LIGHT) } else if hover { Some(HOVER_LIGHT) } else { None };
    if let Some(light) = light {
        if round {
            draw_circle(r.center().x, r.center().y, r.w * 0.42, light);
        } else {
            draw_rectangle(r.x + r.w * 0.08, r.y + r.h * 0.08, r.w * 0.84, r.h * 0.84, light);
        }
    }
    art.is_some()
}

fn draw_minimap(layout: &Layout, world: &World, camera: &Camera, ui: &Ui, minimap: &MapPicture) {
    let area = ui.view_area(world);
    let to_mini = |p: Vec2| layout.world_to_minimap(p, area);
    let diamond = |corner: Vec2, size: Vec2, color: Color| {
        let a = to_mini(corner);
        let b = to_mini(corner + vec2(size.x, 0.0));
        let c = to_mini(corner + size);
        let d = to_mini(corner + vec2(0.0, size.y));
        draw_triangle(a, b, c, color);
        draw_triangle(a, c, d, color);
    };
    let m = layout.minimap;
    draw_rectangle(m.x, m.y, m.w, m.h, BLACK);

    // Terrain and fog: one picture instead of a diamond per tile. Fog only in the player's view – in the editor's
    // "View: Zombies" the map stays readable (`game.rs` paints the picture that way).
    minimap.draw(area, to_mini);
    let fog = ui.sight() == Sight::Player;
    for building in &world.buildings {
        let color = if ui.selected_buildings.contains(&building.id) { WHITE } else { building_color(building.kind) };
        diamond(building.corner(), building.far_corner() - building.corner(), color);
    }
    // The villages of doom in red and the loot in its colour, like in TAB – once explored (in the editor all of them).
    let explored = |p: Vec2| !fog || world.vision.is_explored(p.x.floor() as i32, p.y.floor() as i32);
    for house in world.doom.iter().filter(|d| explored(d.center())) {
        diamond(house.corner() - Vec2::splat(0.5), house.far_corner() - house.corner() + Vec2::ONE, DOOM_MINIMAP);
    }
    for loot in world.loot.iter().filter(|l| explored(l.pos)) {
        let m = to_mini(loot.pos);
        draw_rectangle(m.x - 1.0, m.y - 1.0, 2.0, 2.0, loot_color(loot.kind));
    }

    // Zombies only where someone sees them right now (in the editor all of them – the sleeping ones too, darker).
    for zombie in world.zombies.iter().filter(|z| area.contains(z.pos) && (!fog || world.vision.sees(z.pos))) {
        let m = to_mini(zombie.pos);
        draw_rectangle(m.x - 0.75, m.y - 0.75, 1.5, 1.5, ZOMBIE);
    }
    if !fog {
        for sleeper in world.sleep.iter().filter(|s| area.contains(s.pos)) {
            let m = to_mini(sleeper.pos);
            draw_rectangle(m.x - 0.75, m.y - 0.75, 1.5, 1.5, Color { a: 0.5, ..ZOMBIE });
        }
    }
    for unit in world.units.iter().filter(|u| !u.is_inside()) {
        let m = to_mini(unit.pos);
        let color = if ui.selected.contains(&unit.id) { WHITE } else { kind_color(unit.kind) };
        draw_rectangle(m.x - 1.0, m.y - 1.0, 2.0, 2.0, color);
    }
    // The trains, yellow-green like TAB's (`TrainCarriage.OnUpdate`: `GreenYellow`) – where someone sees them.
    for train in &world.trains {
        for car in train.cars_at(train.offset).into_iter().filter(|c| !fog || world.vision.sees(c.pos)) {
            let (a, b) = (to_mini(car.pos - car.dir * car.kind.length() / 2.0), to_mini(car.pos + car.dir * car.kind.length() / 2.0));
            draw_line(a.x, a.y, b.x, b.y, 2.5, TRAIN_MINIMAP);
        }
    }

    // On top, through the fog: every giant, awake or asleep, and the middle of every swarm of a wave – the bigger the
    // swarm, the bigger its mark (TAB's `IconInfectedGiant` and `IconMiniMap_Swarn`). A mark outside what the minimap
    // shows (a wave still in the spawn ring) sits at its edge, on the side it comes from.
    let fit = layout.minimap_fit(area);
    let (lo, hi) = (fit.point(), fit.point() + fit.size());
    let pin = |p: Vec2, r: f32| to_mini(p).clamp(lo + Vec2::splat(r), hi - Vec2::splat(r));
    let giant_icon = icon("IconInfectedGiant");
    for giant in world.giants() {
        let m = pin(giant, 4.0);
        match &giant_icon {
            Some(t) => draw_picture(t, Rect::new(m.x - 5.0, m.y - 5.0, 10.0, 10.0), WHITE),
            None => {
                draw_circle(m.x, m.y, 3.5, MARK_EDGE);
                draw_circle(m.x, m.y, 2.5, GIANT_MINIMAP);
            }
        }
    }
    // What the editor marked for the minimap (TAB's "Display Icon on Minimap", `sim/neutral.rs`): a white diamond.
    for at in world.minimap_marks() {
        let m = pin(at, 4.0);
        let (top, right, bottom, left) = (m + vec2(0.0, -4.0), m + vec2(4.0, 0.0), m + vec2(0.0, 4.0), m + vec2(-4.0, 0.0));
        draw_triangle(top, right, bottom, MARK_EDGE);
        draw_triangle(top, left, bottom, MARK_EDGE);
        let (top, right, bottom, left) = (m + vec2(0.0, -2.8), m + vec2(2.8, 0.0), m + vec2(0.0, 2.8), m + vec2(-2.8, 0.0));
        draw_triangle(top, right, bottom, WHITE);
        draw_triangle(top, left, bottom, WHITE);
    }
    let swarm_icon = icon("IconMiniMap_Swarn");
    for (middle, count) in world.swarms() {
        let r = 4.0 + (count as f32).sqrt().min(10.0) * 0.35;
        let m = pin(middle, r);
        match &swarm_icon {
            Some(t) => draw_picture(t, Rect::new(m.x - r, m.y - r, 2.0 * r, 2.0 * r), WHITE),
            None => {
                let corners = |r: f32| (m + vec2(0.0, -r), m + vec2(r * 0.9, r * 0.6), m + vec2(-r * 0.9, r * 0.6));
                let (a, b, c) = corners(r + 1.5);
                draw_triangle(a, b, c, MARK_EDGE);
                let (a, b, c) = corners(r);
                draw_triangle(a, b, c, SWARM_MINIMAP);
            }
        }
    }

    // What the camera shows right now: a rectangle, because the map and the minimap are rotated the same way. From the
    // corner of the map's part of the window down to the top of the bar.
    let view = crate::view::rect();
    let top_left = to_mini(camera.screen_to_world(view.point())).max(lo);
    let bottom_right = to_mini(camera.screen_to_world(vec2(view.right(), view.y + layout.top * layout.s))).min(hi);
    let size = bottom_right - top_left;
    draw_rectangle_lines(top_left.x, top_left.y, size.x, size.y, 1.0, WHITE);
}

/// Above the minimap, like TAB's `@NextSwarm`: when the next announced wave comes (the final wave in its colour) – the
/// silent ones (stragglers) give none.
fn draw_next_swarm(layout: &Layout, world: &World) {
    let Some((at, final_wave)) = world.next_announced() else {
        return;
    };
    let r = layout.next_swarm;
    draw_rectangle(r.x, r.y, r.w + 12.0, r.h, Color::new(0.0, 0.0, 0.0, 0.55));
    match icon("IconMiniMap_Swarn") {
        Some(t) => draw_picture(&t, Rect::new(r.x + 2.6, r.y + 2.9, 30.7, 31.9), WHITE),
        None => draw_triangle(vec2(r.x + 18.0, r.y + 6.0), vec2(r.x + 32.0, r.y + 32.0), vec2(r.x + 4.0, r.y + 32.0), SWARM_MINIMAP),
    }
    let seconds = at.saturating_sub(world.tick) / TICKS_PER_SECOND as u64;
    let (what, color) = if final_wave { ("FINAL", FINAL) } else { ("Next", DIM) };
    draw_text(what, r.x + 36.0, r.y + 14.0, 12.0, color);
    draw_text(format!("{}:{:02}", seconds / 60, seconds % 60), r.x + 35.0, r.y + 32.0, 16.0, if final_wave { FINAL } else { WHITE });
}

/// The clock like in TAB: day and hour on the brass plate, the round pause button between them – below them how many
/// days the map has and the speed.
fn draw_clock(layout: &Layout, world: &World, ui: &Ui) {
    let (day, hour) = world.clock();
    let (day_label, hour_label) = (layout.tab(176.0, 618.0, 47.0, 14.0), layout.tab(258.0, 619.0, 51.0, 13.0));
    let (day_value, hour_value) = (layout.tab(167.0, 632.0, 49.0, 27.0), layout.tab(274.0, 633.0, 33.0, 24.0));
    draw_centered("Day", day_label.center().x - 4.0, day_label.bottom() - 2.0, 12.0, DIM);
    draw_centered("Hour", hour_label.center().x + 8.0, hour_label.bottom() - 1.0, 12.0, DIM);
    draw_fitted(&day.to_string(), day_value.center().x + 4.0, day_value.bottom() - 6.0, day_value.w, 20.0, GLOW);
    draw_fitted(&format!("{hour:02}"), hour_value.center().x, hour_value.bottom() - 5.0, hour_value.w, 20.0, GLOW);
    if world.days > 0 {
        draw_centered(&format!("of {}", world.days), day_value.center().x + 4.0, day_value.bottom() + 10.0, 10.0, DIM);
    }
    let speed = if ui.paused { "PAUSED".to_owned() } else { format!("x{}", ui.speed) };
    draw_centered(&speed, hour_value.center().x, hour_value.bottom() + 11.0, 10.0, if ui.paused { GLOW } else { DIM });

    let pause = layout.pause_rect();
    let hover = pause.contains(mouse());
    let picture = icon(if ui.paused { "IconPlay" } else { "IconPause" });
    if !draw_button(pause, "button_round", picture.as_ref(), 0.27, hover, ui.paused, false) || picture.is_none() {
        let c = pause.center();
        if ui.paused {
            // A "play" triangle: click to go on.
            draw_triangle(vec2(c.x - 7.0, c.y - 10.0), vec2(c.x - 7.0, c.y + 10.0), vec2(c.x + 10.0, c.y), GLOW);
        } else {
            draw_rectangle(c.x - 8.0, c.y - 10.0, 5.0, 20.0, BRASS);
            draw_rectangle(c.x + 3.0, c.y - 10.0, 5.0, 20.0, BRASS);
        }
    }
    if hover {
        draw_tooltip(&["Pause (Space)", "Orders still work while paused. + / - : faster, slower."], pause.x, layout.top - 4.0);
    }
}

fn draw_groups(layout: &Layout, world: &World, ui: &Ui) {
    for (g, group) in ui.groups.iter().enumerate() {
        let r = layout.group_rect(g);
        let selected = !group.is_empty() && group.units == ui.selected && group.buildings == ui.selected_buildings;
        draw_rectangle(r.x, r.y, r.w, r.h, if group.is_empty() { Color { a: 0.6, ..SECTION_BG } } else { BOX_BG });
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, if selected { WHITE } else { BORDER });

        let key = if g == 9 { "0".to_owned() } else { (g + 1).to_string() };
        draw_text(&key, r.x + 2.0, r.bottom() - 2.5, 10.0, if group.is_empty() { BORDER } else { KEY });
        if group.is_empty() {
            continue;
        }
        // Its picture: the most common unit type in the group – only buildings: the first building – plus the count.
        // Without TAB's portraits a bar in its colour.
        let count = |kind: UnitKind| group.units.iter().filter(|&&id| unit_kind(world, id) == Some(kind)).count();
        let main = UnitKind::ALL.into_iter().filter(|&kind| count(kind) > 0).max_by_key(|&kind| count(kind));
        let building = group.buildings.iter().find_map(|&id| world.find_building(id)).map(|b| world.buildings[b].kind);
        let (color, picture) = match (main, building) {
            (Some(kind), _) => (kind_color(kind), portrait_of(kind)),
            (None, Some(kind)) => (building_color(kind), portrait_of(kind)),
            (None, None) => (BORDER, None),
        };
        match picture {
            Some(picture) => {
                // As high as the row, as wide as its shape (a unit's portrait is 3 : 4, a building's square).
                let side = r.h - 1.0;
                let size = vec2(side * picture.width() / picture.height().max(1.0), side);
                let params = DrawTextureParams { dest_size: Some(size), ..Default::default() };
                draw_texture_ex(&picture, r.x + 9.0, r.y + 0.5, WHITE, params);
            }
            None => draw_rectangle(r.x + 10.0, r.y + 2.0, 4.0, r.h - 4.0, color),
        }
        draw_right(&group.len().to_string(), r.right() - 2.0, r.bottom() - 2.5, 10.0, WHITE);
    }
}

/// The four small buttons between the card and the stock: camera to the command center, repair all, the grid, the
/// menu. The plan for "repair all" is only worked out while the mouse is on it – with thousands of zombies it isn't
/// free.
fn draw_side_buttons(layout: &Layout, world: &World, ui: &Ui) {
    let pictures = [icon("IconMapColony"), icon("IconRepair"), None, icon("IconOptions")];
    for (k, picture) in pictures.iter().enumerate() {
        let r = layout.side_rect(k);
        let hover = r.contains(mouse());
        let lit = SIDE_BUTTONS[k] == HudHit::GridView && ui.settings.show_grid;
        draw_button(r, "button_square", picture.as_ref(), 0.2, hover, lit, false);
        match SIDE_BUTTONS[k] {
            HudHit::GridView => draw_grid_glyph(r, if lit { GLOW } else { BRASS }),
            _ if picture.is_none() => draw_centered(["C", "X", "", "Esc"][k], r.center().x, r.center().y + 4.0, 10.0, KEY),
            _ => {}
        }
        if !hover {
            continue;
        }
        let keys = &ui.settings.keys;
        let (title, help) = match SIDE_BUTTONS[k] {
            HudHit::Base => (format!("Camera to the command center ({})", keys.label(Action::Base)), String::new()),
            HudHit::RepairAll => {
                let plan = world.repair_plan(0..world.buildings.len());
                let title = match repair_price(&plan) {
                    Some(price) => format!("Repair all ({}) - {price} gold", keys.label(Action::RepairAll)),
                    None => format!("Repair all ({}) - {}", keys.label(Action::RepairAll), nothing_to_repair(&plan)),
                };
                (title, "Every damaged building without a zombie near it - cheapest first, as many as the gold pays for. \
                    X works while no units are selected (then X is the Line formation).".to_owned())
            }
            HudHit::GridView => (format!("Grid view ({})", keys.label(Action::GridView)), String::new()),
            _ => ("Menu (Esc)".to_owned(), String::new()),
        };
        let lines: Vec<&str> = [title.as_str(), help.as_str()].into_iter().filter(|l| !l.is_empty()).collect();
        draw_tooltip(&lines, r.x, layout.top - 4.0);
    }
}

/// The grid button's picture (TAB has none): a tile with a cross in it.
fn draw_grid_glyph(r: Rect, color: Color) {
    let c = r.center();
    let (w, h) = (r.w * 0.3, r.w * 0.3 * TILE_H / TILE_W);
    let corners = [vec2(0.0, -h), vec2(w, 0.0), vec2(0.0, h), vec2(-w, 0.0)].map(|p| c + p);
    for k in 0..4 {
        let (p, q) = (corners[k], corners[(k + 1) % 4]);
        draw_line(p.x, p.y, q.x, q.y, 1.0, color);
        // From the middle of this edge to the middle of the opposite one.
        let (m, n) = ((p + q) / 2.0, (corners[(k + 2) % 4] + corners[(k + 3) % 4]) / 2.0);
        draw_line(m.x, m.y, n.x, n.y, 1.0, color);
    }
}

/// TAB's event list in the top left (`@AreaEvents`, one `ControlEventMessage` each): its picture on the dark-green
/// circle and the text – below the map's goals, if it has some. They fade out in their last second.
fn draw_events(layout: &Layout, world: &World, ui: &Ui) {
    let mut y = goals_bottom(world).map_or(53.0, |bottom| bottom / layout.s + 6.0);
    let background = ui_art::get("event_background");
    for event in ui.events.iter().rev() {
        let fade = event.time_left.min(1.0);
        let r = Rect::new(12.0, y, 320.0, 36.0);
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.0, 0.0, 0.0, 0.5 * fade));
        let picture = Rect::new(r.x + 2.0, r.y + 2.0, 32.0, 32.0);
        let tint = Color::new(1.0, 1.0, 1.0, fade);
        if let Some(t) = &background {
            draw_picture(t, picture, tint);
        }
        match ui_art::get(&format!("events/{}", event.icon)) {
            Some(t) => draw_picture(&t, picture, tint),
            None => draw_rectangle(picture.x, picture.y, picture.w, picture.h, Color { a: fade, ..BORDER }),
        }
        draw_wrapped(&event.text, r.x + 40.0, r.y + 15.0, 276.0, 12.0, 2, Color::new(1.0, 1.0, 1.0, fade));
        y += 40.0;
    }
}

fn draw_selection(layout: &Layout, world: &World, ui: &Ui) {
    let view = ui.selection_view(world);
    match view.as_slice() {
        [] => match ui.selected_zombie.and_then(|id| world.find_zombie(id)) {
            Some(z) => draw_zombie(layout, world, &world.zombies[z]),
            None if ui.selected_place.is_some_and(|place| draw_place(layout, world, place)) => {}
            None => {
                let inner = layout.info;
                draw_text("Nothing selected", inner.x + 10.0, inner.y + 20.0, 16.0, DIM);
                // The keys of the build menus as they are set (`keys.rs`).
                let menus: Vec<String> = build_menus()
                    .map(|menu| format!("{} {}", ui.settings.keys.short(Action::of(Button::Open(menu))), Button::Open(menu).name().to_lowercase()))
                    .collect();
                let hint = format!("Click the command center to build: {}.", menus.join(", "));
                draw_wrapped(&hint, inner.x + 10.0, inner.y + 40.0, inner.w - 20.0, 13.0, 4, DIM);
            }
        },
        [Pick::Unit(i)] => draw_single(layout, world, *i),
        [Pick::Building(b)] => draw_building(layout, world, *b),
        _ => draw_icons(layout, world, ui, &view),
    }
}

/// Ticks → seconds, for showing the times of `data\units.json` the way they are written there.
fn seconds(ticks: u32) -> f32 {
    ticks as f32 / TICKS_PER_SECOND
}

/// The portrait: TAB's picture (`picture`) – a figure's 3 : 4, a building's square (`building`). Without one: a
/// coloured field with the letter of the type, `edge` around it.
fn draw_portrait(layout: &Layout, picture: Option<Texture2D>, building: bool, color: Color, letter: &str, edge: Color) {
    let r = layout.portrait();
    match picture {
        Some(t) if building => draw_picture(&t, layout.building_picture(), WHITE),
        Some(t) => {
            draw_texture_ex(&t, r.x, r.y, WHITE, DrawTextureParams { dest_size: Some(r.size()), ..Default::default() });
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5, edge);
        }
        None => {
            draw_rectangle(r.x, r.y, r.w, r.h, color);
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, edge);
            draw_centered(letter, r.x + r.w / 2.0, r.y + r.h / 2.0 + 12.0, 36.0, BLACK);
        }
    }
}

/// Name and life bar at the top of the text column. Returns where the next line goes.
fn draw_title(layout: &Layout, name: &str, hp: (u32, u32), shield: (u32, u32)) -> f32 {
    let text = layout.text_column();
    draw_fitted_left(name, text.x, text.y + 15.0, text.w, 17.0, BRASS);
    let bar = Rect::new(text.x, text.y + 21.0, text.w, 8.0);
    draw_life(bar, hp, shield, BOX_BG, Some(BORDER));
    text.y + 43.0
}

/// The values right of the text, one box each: a small label above, the number below. As many as fit.
fn draw_cells(layout: &Layout, cells: &[(&str, String)]) {
    let area = layout.cells();
    let cols = ((area.w / CELL_W) as usize).max(1);
    let rows = (area.h / CELL_H) as usize;
    for (k, (label, value)) in cells.iter().take(cols * rows).enumerate() {
        let (x, y) = (area.x + (k % cols) as f32 * CELL_W, area.y + (k / cols) as f32 * CELL_H);
        let r = Rect::new(x, y, CELL_W - 3.0, CELL_H - 3.0);
        draw_rectangle(r.x, r.y, r.w, r.h, Color { a: 0.75, ..BOX_BG });
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, BORDER);
        draw_fitted(label, r.x + r.w / 2.0, r.y + 10.0, r.w - 3.0, 10.0, DIM);
        draw_fitted(value, r.x + r.w / 2.0, r.bottom() - 4.0, r.w - 3.0, 13.0, GLOW);
    }
}

/// One unit selected: picture, name, health, order, what it is – and its values below.
fn draw_single(layout: &Layout, world: &World, i: usize) {
    let unit = &world.units[i];
    let stats = unit.stats();
    draw_portrait(layout, portrait_of(unit.kind), false, kind_color(unit.kind), &stats.letter, BORDER);
    let text = layout.text_column();
    // A tag of the editor's (`sim/neutral.rs`): its own name, and its text instead of the kind's.
    let tag = unit.tag.as_deref();
    let shown = tag.filter(|t| !t.name.is_empty()).map_or(stats.name.as_str(), |t| t.name.as_str());
    let name = if unit.veteran { format!("{shown} (veteran)") } else { shown.to_owned() };
    let y = draw_title(layout, &name, (unit.hp, unit.max_hp()), (0, 0));
    let order = match unit.queue_len() {
        0 | 1 => format!("Order: {}", unit.order_name()),
        n => format!("Order: {} (+{} queued)", unit.order_name(), n - 1),
    };
    draw_fitted_left(&order, text.x, y, text.w, 13.0, WHITE);
    let about = tag.filter(|t| !t.text.is_empty()).map_or(unit_text(unit.kind), |t| t.text.as_str());
    draw_wrapped(about, text.x, y + 15.0, text.w, 12.0, 4, DIM);

    let mut cells = vec![
        ("HP", format!("{}/{}", unit.hp, unit.max_hp())),
        ("Damage", world.army_damage(unit.damage()).to_string()),
        ("Armor", percent(stats.armor)),
        ("Upkeep", format!("-{}", stats.upkeep)),
    ];
    if stats.food > 0 {
        cells.push(("Food", format!("-{}", stats.food)));
    }
    // Carrying a barrel: it doesn't shoot – D puts it down.
    if unit.barrel {
        cells.insert(0, ("Barrel", "D: drop".to_owned()));
    }
    if stats.heals {
        cells.push(("Beam", format!("{}", stats.range)));
    } else {
        cells.push(("Range", format!("{}", stats.range)));
        cells.push(("Shots/s", format!("{:.1}", TICKS_PER_SECOND / stats.shot_ticks() as f32)));
    }
    cells.extend([
        ("Speed", format!("{}", stats.speed)),
        ("Sight", format!("{}", stats.sight)),
        ("Kills", unit.kills.to_string()),
        // Experience towards a veteran (`sim/veterans.rs`) – mercenaries don't gather any.
        ("XP", if stats.mercenary { "-".to_owned() } else { format!("{}/{}", unit.xp / 1000, data::get().units.veterans.xp) }),
        // Bringing the weapon up once, then before every shot: aiming and the wind-up (`sim/combat.rs`).
        ("Raise", format!("{:.2} s", seconds(stats.raise))),
        ("Aim", format!("{:.2} s", seconds(stats.aim + stats.windup))),
        ("Turn", format!("{:.2} s", seconds(stats.turn_around))),
        ("Noise", format!("{}", stats.noise)),
    ]);
    draw_cells(layout, &cells);
}

/// What a unit type is, in one line.
fn unit_text(kind: UnitKind) -> &'static str {
    match kind {
        UnitKind::Soldier => "Tough, short range, aims quickly - and loud.",
        UnitKind::Ranger => "Fast and weak, draws the bow before every arrow, quiet.",
        UnitKind::Sniper => "Long range, one big shot - very loud.",
        UnitKind::Medic => "No attack. Heals hurt units with its beam - once they're out of the fight for a few seconds.",
        UnitKind::Spearman => "Melee: every hit pushes the zombie back, nearly silent. Dies to spitters and infested.",
        UnitKind::MercSoldier => "Mercenary: tougher and stronger than a soldier, no worker - double upkeep, first to leave in debt.",
        UnitKind::MercRanger => "Mercenary: tougher and stronger than a ranger, no worker - double upkeep, first to leave in debt.",
        UnitKind::MercSniper => "Mercenary: tougher and stronger than a sniper, no worker - double upkeep, first to leave in debt.",
        UnitKind::Lucifer => "Flamethrower: a cone of fire that sets zombies alight - and burns your own units in it too.",
        UnitKind::Thanatos => "Missile launcher: blows up crowds 3 to 10 tiles away, hits with the launcher close up.",
        UnitKind::Titan => "Walking cannon: far, hard shots that hit a little area. Very tough.",
    }
}

/// A zombie clicked on: picture, health, its values and what it's up to.
fn draw_zombie(layout: &Layout, world: &World, zombie: &Zombie) {
    let stats = zombie.breed.stats();
    draw_portrait(layout, portrait_of(zombie.breed), false, breed_color(zombie.breed), "Z", RED);
    let text = layout.text_column();
    let title = if zombie.final_wave { format!("{} (final wave)", zombie.breed.name()) } else { zombie.breed.name().to_owned() };
    let y = draw_title(layout, &title, (zombie.hp(), stats.hp), (0, 0));
    draw_wrapped(world.zombie_doing(zombie), text.x, y, text.w, 13.0, 5, WHITE);

    let mut cells = vec![
        ("HP", format!("{}/{}", zombie.hp(), stats.hp)),
        ("Damage", stats.damage.to_string()),
        ("Armor", percent(stats.armor)),
        ("Hits", format!("{:.1} s", seconds(zombie.breed.attack_ticks()))),
    ];
    if zombie.breed.is_ranged() {
        cells.push(("Range", format!("{}", stats.range)));
    }
    if stats.splash > 0.0 {
        cells.push(("Splash", format!("{}", stats.splash)));
    }
    cells.extend([
        ("Speed", format!("{:.2}", zombie.speed())),
        ("Sight", format!("{}", stats.sight)),
        ("Hears", format!("{}", zombie.breed.hearing_range())),
        ("Rank", stats.level.to_string()),
    ]);
    draw_cells(layout, &cells);
}

/// A house of doom or a pile of loot clicked on: what it is, what it holds and what it does. Returns `false` if it's
/// gone (shot down, picked up) – then nothing is drawn.
fn draw_place(layout: &Layout, world: &World, place: Place) -> bool {
    let text = layout.text_column();
    match place {
        Place::Doom(id) => {
            let Some(d) = world.find_doom(id) else {
                return false;
            };
            let house = &world.doom[d];
            let doom = &data::get().doom;
            // TAB's three houses of doom, by their size.
            let picture = ui_art::get(match house.size.0 {
                0..=2 => "portraits/DoomSmall",
                3 => "portraits/DoomMedium",
                _ => "portraits/DoomBig",
            });
            draw_portrait(layout, picture, true, DOOM_MINIMAP, "!", RED);
            let y = draw_title(layout, "Village of doom", (house.hp, house.full_hp()), (0, 0));
            let about = if house.is_stirring(world.tick) {
                "STIRRING - its zombies are coming out.".to_owned()
            } else if house.inside > 0 {
                format!(
                    "Its zombies come out when it hears noise, when it's hit (and its neighbours with it) and from day {} on every {} days to raid your base. Shoot it down: it drops loot, the rest never come.",
                    doom.raids_from_day, doom.raid_every_days
                )
            } else {
                "Empty - shoot it down for its loot.".to_owned()
            };
            draw_wrapped(&about, text.x, y, text.w, 12.0, 5, WHITE);
            let (w, h) = house.size;
            draw_cells(
                layout,
                &[
                    ("HP", format!("{}/{}", house.hp, house.full_hp())),
                    ("Left", house.inside.to_string()),
                    ("Armor", percent(doom.armor)),
                    ("House", house.nest().name().to_owned()),
                    ("Size", format!("{w} x {h}")),
                ],
            );
        }
        Place::Loot(id) => {
            let Some(l) = world.find_loot(id) else {
                return false;
            };
            let loot = &world.loot[l];
            let letter = if loot.kind == LootKind::Gold { "$" } else { &loot.kind.name()[..1] };
            draw_portrait(layout, ui_art::get(&format!("portraits/Loot{:?}", loot.kind)), true, loot_color(loot.kind), letter, BORDER);
            draw_fitted_left(&format!("Loot: {} {}", loot.amount, loot.kind.name().to_lowercase()), text.x, text.y + 15.0, text.w, 17.0, BRASS);
            let about = if loot.guards > 0 {
                format!("Walk onto it to pick it up - but zombies lie hidden around it and come out within {} tiles.", data::get().loot.wake_radius)
            } else {
                "Walk onto it to pick it up (right click it with your units).".to_owned()
            };
            draw_wrapped(&about, text.x, text.y + 36.0, text.w, 12.0, 6, WHITE);
            let guarded = if loot.guards > 0 { "yes" } else { "no" };
            draw_cells(layout, &[(loot.kind.name(), loot.amount.to_string()), ("Guarded", guarded.to_owned())]);
        }
    }
    true
}

/// One building selected: picture, health, what it does – for the barracks the production queue, for a tower who
/// sits in it – and its values below.
fn draw_building(layout: &Layout, world: &World, b: usize) {
    let building = &world.buildings[b];
    let stats = building.stats();
    let power = world.power();
    draw_portrait(layout, portrait_of(building.kind), true, building_color(building.kind), &stats.letter, BORDER);
    let text = layout.text_column();
    let tag = building.tag.as_deref();
    let name = tag.filter(|t| !t.name.is_empty()).map_or(stats.name.as_str(), |t| t.name.as_str());
    let y = draw_title(layout, name, (building.hp, stats.hp), (building.shield, stats.max_shield()));

    let mut cells = vec![("HP", format!("{}/{}", building.hp, stats.hp))];
    if stats.max_shield() > 0 {
        cells.push(("Shield", format!("{} %", building.shield * 100 / stats.max_shield())));
    }
    // Every + and - in these boxes is per payout (`economy_every_seconds`).
    if stats.colonists > 0 {
        cells.push(("Colonists", stats.colonists.to_string()));
    }
    if stats.gold > 0 {
        cells.push(("Gold", format!("+{}", building.paid)));
    }
    if stats.food > 0 {
        cells.push(("Food", format!("-{}", stats.food)));
    }
    if stats.food_made > 0 {
        cells.push(("Food", format!("+{}", world.food_made(building))));
    }
    if stats.armor > 0.0 {
        cells.push(("Armor", percent(stats.armor)));
    }
    // What it gathers per payout, right now.
    let rate = world.gather_rate(building.kind, building.tile, b, power.share(world, b));
    for r in Resource::ALL.into_iter().filter(|r| rate[r.index()] > 0) {
        cells.push((r.name(), format!("+{}", per_hour(rate[r.index()] as i64))));
    }
    let gold = world.gold_rate(building.kind, building.tile, b, power.share(world, b));
    if gold > 0 {
        cells.push(("Gold", format!("+{}", per_hour(gold as i64))));
    }
    // A market: its houses, the colonists in them, what it may sell and what it sold at the last payout. A house: what
    // its market adds.
    if building.kind == BuildingKind::Market {
        let houses = world.market_houses(BuildingKind::Market.trade_area(building.tile), b);
        cells.push(("Houses", houses.len().to_string()));
        cells.push(("Buyers", world.customers(&houses).to_string()));
        cells.push(("Limit", world.sell_limit(&houses).to_string()));
        cells.push(("Sold", format!("+{}", building.sold_gold)));
    }
    if stats.colonists > 0 && world.market_bonus(b) > 0 {
        cells.push(("Market", format!("+{} %", world.market_bonus(b))));
    }
    if stats.gold_storage > 0 {
        cells.push(("Storage", format!("+{}", stats.gold_storage)));
    }
    if stats.credit > 0 {
        cells.push(("Credit", stats.credit.to_string()));
        cells.push(("Interest", format!("{} %", data::get().colony.credit_interest_percent)));
    }
    if building.kind == BuildingKind::CommandCenter {
        cells.push(("Scans", format!("{}/{}", world.scan_charges, data::get().buildings.scan.charges)));
        if let Some(seconds) = world.next_scan_in() {
            cells.push(("Next", format!("{}:{:02}", seconds / 60, seconds % 60)));
        }
    }
    // A tower that shoots by itself: its gun, and how far it is through reloading.
    if let Some(weapon) = &stats.weapon {
        cells.push(("Damage", weapon.damage.to_string()));
        cells.push(("Range", format!("{}", weapon.range)));
        cells.push(("Reload", format!("{:.1} s", seconds(weapon.reload))));
        cells.push(("Ready", format!("{} %", ((1.0 - world.reload_share(b)) * 100.0).round())));
    }
    if stats.smelts > 0 {
        cells.push(("Iron", format!("+{}", building.smelted)));
    }
    // Machines: the noise it makes every few seconds while it works (`sim/sound.rs`).
    if stats.noise > 0 {
        cells.push(("Noise", stats.noise.to_string()));
    }
    // How far it gathers: tiles around its site, and whether the tiles under it count (`gather_range`, `gather_below`).
    if building.kind.gathers().is_some() {
        cells.push(("Range", stats.gather_range.to_string()));
        cells.push(("Below", if stats.gather_below { "yes" } else { "no" }.to_owned()));
    }
    // Power: what it makes, how much of what it takes it gets, how far it carries it.
    if stats.energy > 0 {
        let made = if stats.burns_wood > 0 && !building.fuelled { 0 } else { world.power_made(building) };
        cells.push(("Power", format!("+{made}")));
    }
    if stats.burns_wood > 0 {
        cells.push(("Wood", format!("-{}", stats.burns_wood)));
    }
    if stats.energy_use > 0 {
        cells.push(("Power", format!("{} %", power.share(world, b) / 10)));
    }
    if stats.power_radius > 0.0 {
        cells.push(("Reach", format!("{}", stats.power_radius)));
    }
    if stats.upkeep > 0 {
        let upkeep = if building.works() { stats.upkeep } else { 0 };
        cells.push(("Upkeep", format!("-{upkeep}")));
    }
    // What it gives (infected or not finished: nothing) or takes.
    if stats.workers > 0 {
        cells.push(("Workers", format!("+{}", if building.infected || !building.is_built() { 0 } else { stats.workers })));
    }
    if stats.worker_cost > 0 {
        cells.push(("Workers", format!("-{}", if building.off { 0 } else { stats.worker_cost })));
    }
    cells.push(("Sight", format!("{}", stats.sight)));
    let (w, h) = stats.size;
    cells.push(("Size", format!("{w} x {h}")));
    if building.repair_left > 0 {
        cells.insert(1, ("Repair", format!("+{}", building.repair_left)));
    }

    // Still being built: how far, and the text says when it works – no garrison, no queue yet.
    if !building.is_built() {
        let done = 100 - building.construction * 100 / stats.build_time.max(1);
        cells.insert(1, ("Built", format!("{done} %")));
        draw_wrapped(&building_text(world, &power, b), text.x, y, text.w, 12.0, 5, DIM);
        draw_cells(layout, &cells);
        return;
    }
    if building.kind.is_tower() {
        // One box per spot: the unit in it, or empty.
        let inside = world.garrison(building.id);
        for k in 0..data::get().buildings.tower_slots {
            let r = layout.queue_rect(k);
            match inside.get(k) {
                Some(&i) => {
                    let unit = &world.units[i];
                    match portrait_of(unit.kind) {
                        Some(t) => draw_face(&t, r, WHITE),
                        None => {
                            draw_rectangle(r.x, r.y, r.w, r.h, kind_color(unit.kind));
                            draw_centered(&unit.stats().letter, r.x + r.w / 2.0, r.y + r.h / 2.0 + 6.0, 20.0, BLACK);
                        }
                    }
                    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, BORDER);
                }
                None => draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, BORDER),
            }
        }
        cells.insert(1, ("Garrison", format!("{}/{}", inside.len(), data::get().buildings.tower_slots)));
        cells.insert(2, ("Rally", if building.rally.is_some() { "set" } else { "none" }.to_owned()));
        draw_cells(layout, &cells);
        return;
    }
    if !building.kind.trains() && !building.kind.makes_barrels() && researched_in(building.kind).next().is_none() {
        draw_wrapped(&building_text(world, &power, b), text.x, y, text.w, 12.0, 5, DIM);
        draw_cells(layout, &cells);
        return;
    }
    // Production queue: 5 spots, the front one with a progress bar. A click cancels.
    for i in 0..MAX_PRODUCTION {
        let r = layout.queue_rect(i);
        match building.production.get(i) {
            Some(&product) => {
                let (color, letter, name, picture) = match product {
                    Product::Unit(kind) => (kind_color(kind), kind.stats().letter.as_str(), kind.stats().name.as_str(), portrait_of(kind)),
                    Product::Barrel => {
                        let barrel = BuildingKind::Barrel.stats();
                        (building_color(BuildingKind::Barrel), barrel.letter.as_str(), barrel.name.as_str(), portrait_of(BuildingKind::Barrel))
                    }
                    Product::Research(t) => {
                        let button = Button::Research(t);
                        (building_color(building.kind), "R", tech(t).name.as_str(), button.picture(false))
                    }
                };
                match picture {
                    Some(t) => {
                        draw_rectangle(r.x, r.y, r.w, r.h, SECTION_BG);
                        draw_face(&t, r, WHITE);
                        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, BORDER);
                    }
                    None => {
                        draw_rectangle(r.x, r.y, r.w, r.h, color);
                        draw_centered(letter, r.x + r.w / 2.0, r.y + r.h / 2.0 + 6.0, 20.0, BLACK);
                    }
                }
                if i == 0 {
                    let t = building.progress as f32 / product.build_time().max(1) as f32;
                    draw_rectangle(r.x, r.bottom() - 5.0, r.w * t, 5.0, GREEN);
                }
                if r.contains(mouse()) {
                    draw_tooltip(&[&format!("{name} - click to cancel")], r.x, layout.top - 4.0);
                }
            }
            None => draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, BORDER),
        }
    }
    cells.insert(1, ("Queue", format!("{}/{MAX_PRODUCTION}", building.production.len())));
    if building.kind.trains() {
        cells.insert(2, ("Rally", if building.rally.is_some() { "set" } else { "none" }.to_owned()));
    } else {
        cells.insert(2, ("Barrels", format!("{}/{}", world.barrels, world.barrel_room())));
    }
    draw_cells(layout, &cells);
}

/// What a building does, in a sentence or two – like TAB's text next to the portrait. What's wrong with it comes
/// first, in capitals: infected, no power, hungry, out of wood.
fn building_text(world: &World, power: &Power, b: usize) -> String {
    let building = &world.buildings[b];
    // The editor's text for it (its tag, `sim/neutral.rs`) instead of the usual one.
    if let Some(text) = building.tag.as_deref().map(|t| t.text.clone()).filter(|t| !t.is_empty()) {
        return text;
    }
    let stats = building.stats();
    let share = power.share(world, b);
    if building.infected {
        return "INFECTED - its colonists came out as zombies, everyone walks through it. Repair cures it.".to_owned();
    }
    if !building.is_built() {
        let seconds = building.construction as f32 / TICKS_PER_SECOND;
        return format!("BEING BUILT - ready in {seconds:.0} s. Until then it does nothing, but zombies can already hit it.");
    }
    if building.off {
        return "SWITCHED OFF - no upkeep, no workers, no power, and it does nothing. On/Off switches it on again.".to_owned();
    }
    if building.kind.on_grid() && power.grid_of[b].is_none() {
        return "NO POWER - it stands outside the grid (its command center or tesla tower is gone).".to_owned();
    }
    if share < 1000 {
        let g = power.grid_of[b].unwrap_or(0);
        return format!("LOW POWER - its grid makes {} but needs {}: it works at {} %. Build a windmill or a charcoal \
            burner.", power.made[g], power.used[g], share / 10);
    }
    if stats.colonists > 0 && !building.fed {
        return "HUNGRY - no food last hour, so no gold. Build a hunter cottage, a fishing hut or a farm.".to_owned();
    }
    if stats.burns_wood > 0 && !building.fuelled {
        return "OUT OF WOOD - no power until it gets wood again (it takes some at every payout).".to_owned();
    }
    if stats.smelts > 0 && building.smelted == 0 {
        return format!("IDLE - it smelted nothing at the last payout: every iron takes {} iron ore and {} coal. Build mines on iron and coal patches.", stats.smelt_ore, stats.smelt_coal);
    }
    if let Some(weapon) = &stats.weapon {
        let whom = if weapon.toughest { "the toughest zombie (most hit points)" } else { "the nearest zombie" };
        return format!("Shoots by itself at {whom} within {} tiles - {} damage every {:.1} s, {} noise per shot.", weapon.range, weapon.damage, seconds(weapon.reload), weapon.noise);
    }
    if let Some((source, _)) = building.kind.gathers() {
        let tiles = world.worked_tiles(building.kind, building.tile, b).len();
        let what = match source {
            Source::Forest => "forest",
            Source::Hunting => "forest, grass and dirt",
            Source::Water => "water",
            Source::Ore => "ore",
            Source::Fertile => "grass",
            Source::Stone => "stone",
            Source::Oil => "oil",
        };
        let place = if stats.gather_below { "under and around" } else { "around" };
        return format!("Works {tiles} {what} tiles {place} it. Tiles another one of its kind already works don't count.");
    }
    match building.kind {
        BuildingKind::CommandCenter => "The heart of the colony - it builds everything and its power reaches far. Lose it and the round is lost.".to_owned(),
        BuildingKind::Wall => "Holds zombies back - they have to tear it down.".to_owned(),
        BuildingKind::Gate => "Your units walk through the middle - zombies have to tear it down.".to_owned(),
        BuildingKind::WoodWorkshop => "While it stands, the farm, the windmill, the stake trap, the ballista and the stone workshop can be built.".to_owned(),
        BuildingKind::StoneWorkshop => "While it stands, the munitions factory, the foundry and the stone buildings can be built.".to_owned(),
        BuildingKind::Foundry => format!("Smelted {} iron at the last payout (up to {} - 2 iron ore and 1 coal each). While it stands, the machine gun tower can be built.", building.smelted, stats.smelts),
        BuildingKind::MunitionsFactory => "While it stands, the barracks can train soldiers and snipers. Makes barrels (Q) - right click it with units: each takes one along.".to_owned(),
        BuildingKind::Market if !building.selling => "SELLING OFF - it keeps every good (Q switches selling on). Its houses still pay more.".to_owned(),
        BuildingKind::Market => format!("Its houses pay +{} % gold. It sells what lies above {} % of the storage - at the last payout {} goods for {} gold.", stats.house_bonus_percent, data::get().buildings.sell_from_percent, building.sold_goods, building.sold_gold),
        BuildingKind::Bank => format!("More room for gold, and {} credit: with Shift you may buy into it when only gold is missing - no penalties, {} % interest per payout. Falls: its share of that debt is gone.", stats.credit, data::get().colony.credit_interest_percent),
        BuildingKind::Barrel => "Any damage and it goes off - hurting everyone around it, loudly.".to_owned(),
        BuildingKind::Stakes => "Hurts every zombie walking over it - every hit wears it down.".to_owned(),
        BuildingKind::TeslaTower => "Carries the power further - a circle of the grid around it.".to_owned(),
        BuildingKind::Windmill | BuildingKind::CharcoalBurner => "Makes power for its grid.".to_owned(),
        _ if stats.colonists > 0 => format!("{} colonists live here, eat and pay gold. Shield down = infected.", stats.colonists),
        _ => "Does nothing yet - but blocks the way.".to_owned(),
    }
}

/// Thousandths per payout as a short number: "2.5", "12".
fn per_hour(milli: i64) -> String {
    let tenths = (milli.abs() + 50) / 100;
    let sign = if milli < 0 { "-" } else { "" };
    if tenths % 10 == 0 { format!("{sign}{}", tenths / 10) } else { format!("{sign}{}.{}", tenths / 10, tenths % 10) }
}

/// What an icon in the selection shows: type (for Tab), colour, letter, name, hit points, shield.
struct IconInfo {
    sub: Sub,
    color: Color,
    letter: &'static str,
    name: &'static str,
    hp: (u32, u32),
    shield: (u32, u32),
    /// TAB's portrait – a figure's (3 : 4, `figure`) or a building's.
    picture: Option<Texture2D>,
    figure: bool,
}

fn icon_info(world: &World, pick: Pick) -> IconInfo {
    match pick {
        Pick::Unit(i) => {
            let unit = &world.units[i];
            let stats = unit.stats();
            let (letter, name) = (stats.letter.as_str(), stats.name.as_str());
            let picture = portrait_of(unit.kind);
            IconInfo { sub: Sub::Unit(unit.kind), color: kind_color(unit.kind), letter, name, hp: (unit.hp, unit.max_hp()), shield: (0, 0), picture, figure: true }
        }
        Pick::Building(b) => {
            let building = &world.buildings[b];
            let stats = building.stats();
            let (letter, name) = (stats.letter.as_str(), stats.name.as_str());
            let (hp, shield) = ((building.hp, stats.hp), (building.shield, stats.max_shield()));
            let picture = portrait_of(building.kind);
            IconInfo { sub: Sub::Building(building.kind), color: building_color(building.kind), letter, name, hp, shield, picture, figure: false }
        }
    }
}

/// Several things selected: one icon each, units first, sorted by type. The active type (Tab) is bright.
fn draw_icons(layout: &Layout, world: &World, ui: &Ui, view: &[Pick]) {
    let capacity = layout.icon_capacity();
    for (k, &pick) in view.iter().take(capacity).enumerate() {
        let info = icon_info(world, pick);
        let r = layout.icon_rect(k);
        let base = info.color;
        let fill = if ui.active == Some(info.sub) { base } else { Color::new(base.r * 0.4, base.g * 0.4, base.b * 0.4, 1.0) };
        let frac = info.hp.0 as f32 / info.hp.1.max(1) as f32;
        // TAB's portrait – the types not active (Tab) greyed.
        let tint = if ui.active == Some(info.sub) { WHITE } else { GREYED };
        match &info.picture {
            Some(t) => {
                draw_rectangle(r.x, r.y, r.w, r.h, SECTION_BG);
                if info.figure { draw_face(t, r, tint) } else { draw_picture(t, r, tint) }
            }
            None => {
                draw_rectangle(r.x, r.y, r.w, r.h, fill);
                draw_centered(info.letter, r.x + r.w / 2.0, r.y + r.h / 2.0 + 6.0, 20.0, BLACK);
            }
        }
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.5, hp_color(frac));
        // small life bar at the bottom of the icon
        draw_life(Rect::new(r.x + 3.0, r.bottom() - 6.0, r.w - 6.0, 3.0), info.hp, info.shield, BLACK, None);
    }

    // Tooltip above the icon under the mouse: name and health.
    let mouse = mouse();
    for (k, &pick) in view.iter().take(capacity).enumerate() {
        let r = layout.icon_rect(k);
        if r.contains(mouse) {
            let info = icon_info(world, pick);
            let text = format!("{}   HP {} / {}", info.name, info.hp.0, info.hp.1);
            draw_tooltip(&[&text], r.x, layout.top - 4.0);
        }
    }

    // Summary: how many, which type is active.
    let mut text = format!("{} selected", view.len());
    if view.len() > capacity {
        text += &format!(" (+{} not shown)", view.len() - capacity);
    }
    if let Some(sub) = ui.active {
        text += &format!("   active: {} ({})", sub.name(), ui.settings.keys.label(Action::NextType));
    }
    let inner = layout.info;
    draw_fitted_left(&text, inner.x + 2.0, inner.bottom() - 3.0, inner.w - 4.0, 12.0, DIM);
}

fn draw_card(layout: &Layout, world: &World, ui: &Ui, card: &[Button]) {
    let mouse = mouse();

    // Empty spots: without TAB's pictures a dark box each (TAB leaves them empty).
    if ui_art::get("button_square").is_none() {
        for b in 0..CARD_COLS * CARD_ROWS {
            let r = layout.button_rect(b);
            draw_rectangle(r.x, r.y, r.w, r.h, SECTION_BG);
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, Color { a: 0.5, ..BORDER });
        }
    }

    let mut hovered = None;
    let free_workers = world.free_workers();
    for &button in card {
        let r = layout.button_rect(button.slot());
        let hover = r.contains(mouse);
        if hover {
            hovered = Some(button);
        }
        let planned = matches!(button, Button::Planned(_));
        // Highlighted: the command waiting for its target, the building on the mouse, the chosen formation.
        let waiting = ui.targeting == Some(button)
            || (button.places().is_some() && ui.placing == button.places())
            || button.formation().is_some_and(|shape| shape == ui.formation)
            || (button == Button::Switch && ui.switching_on(world))
            || (button == Button::Sell && ui.selling(world))
            || matches!(button, Button::Delay(d) if ui.mine_delay(world) == Some(d));
        // Too expensive right now – gold or workers: the picture greys and the price turns red (like StarCraft's
        // missing minerals or supply). Planned: everything grey – the button only shows where it will be.
        let cost = shown_cost(button, world, ui);
        let goods = button.places().is_none_or(|kind| world.can_afford_goods(kind) && world.unlocked(kind))
            && button.trains().is_none_or(|kind| world.unit_unlocked(kind) && world.can_afford_unit_goods(kind))
            && !matches!(button, Button::Research(t) if !world.can_afford_tech_goods(t));
        let affordable = cost.is_none_or(|cost| cost <= world.gold) && button.workers() <= free_workers && goods;
        // Repair with nothing to repair: grey like a planned button. The map's own button once it can't be pressed.
        let idle = (button == Button::Repair && cost.is_none()) || (button == Button::Custom && !world.command_ready());
        let greyed = planned || idle || !affordable;
        // TAB's picture: a portrait for what it trains or builds, an icon for a command.
        let picture = button.picture(ui.switching_on(world));
        let figure = button.trains().is_some();
        let plate = draw_button(r, "button_square", None, 0.0, false, false, planned);
        if let Some(t) = &picture {
            let inner = Rect::new(r.x + 4.0, r.y + 4.0, r.w - 8.0, r.h - 8.0);
            let tint = if greyed { GREYED } else { WHITE };
            if figure { draw_face(t, inner, tint) } else { draw_picture(t, inner, tint) }
        }
        let light = if waiting { Some(WAIT_LIGHT) } else if hover { Some(HOVER_LIGHT) } else { None };
        if let Some(light) = light {
            draw_rectangle(r.x + 3.0, r.y + 3.0, r.w - 6.0, r.h - 6.0, light);
        }
        if waiting {
            draw_rectangle_lines(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0, 1.5, GREEN);
        }
        // Only the first key fits on the button – the tooltip has both.
        let key = ui.settings.keys.short(Action::of(button));
        draw_shadowed(key, r.x + 4.0, r.y + 11.0, 11.0, if planned { DIM } else { KEY });
        if let Some(cost) = cost {
            let price = cost.to_string();
            let x = r.right() - 3.0 - text_width(&price, 10.0);
            draw_shadowed(&price, x, r.bottom() - 4.0, 10.0, if affordable { KEY } else { TOO_DEAR });
        } else if let (Button::Custom, Some(n)) = (button, world.command_state.counter) {
            // The map's own number on its button (the counter expression).
            draw_shadowed(&n.to_string(), r.right() - 3.0 - text_width(&n.to_string(), 10.0), r.bottom() - 4.0, 10.0, KEY);
        } else if planned && !plate {
            draw_right("soon", r.right() - 3.0, r.y + 11.0, 9.0, BORDER);
        } else if matches!(button, Button::Open(_)) && picture.is_none() {
            draw_right(">", r.right() - 4.0, r.y + 12.0, 12.0, BRASS);
        }
        // No picture (the formations, or no TAB pictures): its name. The map's own button carries the map's name for
        // it.
        if picture.is_none() {
            let name = match (button, &world.story.command) {
                (Button::Custom, Some(command)) => command.label.as_str(),
                _ => button.name(),
            };
            draw_fitted(name, r.x + r.w / 2.0, r.y + r.h / 2.0 + 8.0, r.w - 4.0, 11.0, if greyed { DIM } else { GLOW });
        }
    }

    // Tooltip above the command card, like in StarCraft – with the price.
    if let Some(button) = hovered {
        let key = ui.settings.keys.label(Action::of(button));
        let title = match (shown_cost(button, world, ui), button) {
            (Some(cost), _) => format!("{} ({}) - {}", button.title(), key, price_text(button, cost, world)),
            (None, Button::Repair) => {
                let plan = world.repair_plan(ui.selected_building_indices(world));
                format!("{} ({}) - {}", button.title(), key, nothing_to_repair(&plan))
            }
            (None, Button::Planned(_)) => format!("{} ({}) - planned, not in the game yet", button.title(), key),
            (None, Button::Demolish) if ui.editor.editing() && !ui.editor.testing() => format!("Remove ({key}) - nothing comes back"),
            (None, Button::Demolish) => format!("{} ({}) - {}", button.title(), key, refund_text(world, &ui.selected_building_indices(world))),
            (None, Button::Custom) => {
                let label = world.story.command.as_ref().map_or("", |c| c.label.as_str());
                format!("{label} ({key})")
            }
            (None, _) => format!("{} ({})", button.title(), key),
        };
        let help = match (button, &world.story.command) {
            (Button::Custom, Some(command)) if !world.command_state.enabled && !command.disabled_text.is_empty() => command.disabled_text.as_str(),
            (Button::Custom, Some(command)) if !command.text.is_empty() => command.text.as_str(),
            _ => button.help(),
        };
        draw_tooltip(&[&title, help], layout.card.x, layout.top - 4.0);
    }
}

/// The whole price of a button for its tooltip: gold, wood, stone, workers – and what has to stand first.
fn price_text(button: Button, gold: u32, world: &World) -> String {
    let mut parts = vec![format!("{gold} gold")];
    if let Some(kind) = button.places() {
        let s = kind.stats();
        if s.cost_wood > 0 {
            parts.push(format!("{} wood", s.cost_wood));
        }
        if s.cost_stone > 0 {
            parts.push(format!("{} stone", s.cost_stone));
        }
        if s.cost_coal > 0 {
            parts.push(format!("{} coal", s.cost_coal));
        }
        if s.cost_iron > 0 {
            parts.push(format!("{} iron", s.cost_iron));
        }
        if s.cost_oil > 0 {
            parts.push(format!("{} oil", s.cost_oil));
        }
    }
    if let Button::Research(t) = button {
        let tech = tech(t);
        for (r, n) in tech.goods().into_iter().filter(|&(_, n)| n > 0) {
            parts.push(format!("{n} {}", resource_word(r)));
        }
        let seconds = tech.ticks(&data::get().research) as f32 / TICKS_PER_SECOND;
        parts.push(format!("{seconds:.0} s"));
    }
    if let Some(kind) = button.trains() {
        let s = kind.stats();
        if s.cost_wood > 0 {
            parts.push(format!("{} wood", s.cost_wood));
        }
        if s.cost_iron > 0 {
            parts.push(format!("{} iron", s.cost_iron));
        }
        if s.cost_oil > 0 {
            parts.push(format!("{} oil", s.cost_oil));
        }
    }
    match button.workers() {
        0 => {}
        1 => parts.push("1 worker".to_owned()),
        n => parts.push(format!("{n} workers")),
    }
    let upkeep = button.places().map(|kind| kind.stats().upkeep).or(button.trains().map(|kind| kind.stats().upkeep));
    if let Some(upkeep) = upkeep.filter(|&u| u > 0) {
        parts.push(format!("{upkeep} gold/h upkeep"));
    }
    if let Some(food) = button.trains().map(|kind| kind.stats().food).filter(|&f| f > 0) {
        parts.push(format!("{food} food/h"));
    }
    let mut text = parts.join(", ");
    let building = button.places().filter(|&kind| !world.unlocked(kind)).and_then(|kind| kind.stats().requires);
    let unit = button.trains().filter(|&kind| !world.unit_unlocked(kind)).and_then(|kind| kind.stats().requires);
    let needed = building.or(unit);
    if let Some(needed) = needed.filter(|&n| !world.stands(Some(n))) {
        text += &format!(" - needs a {} first", needed.stats().name.to_lowercase());
    } else if let Some(t) = button.places().and_then(|k| world.missing_research(k)).or(button.trains().and_then(|k| world.missing_unit_research(k))) {
        text += &format!(" - {}", research_first(t));
    }
    text
}

/// "Research X in the wood workshop first" – what a locked button or a refused order says.
pub fn research_first(t: u16) -> String {
    let tech = tech(t);
    format!("research {} in the {} first", tech.name.to_lowercase(), tech.researched_in.stats().name.to_lowercase())
}

/// A good's word in a price.
fn resource_word(r: Resource) -> &'static str {
    match r {
        Resource::Wood => "wood",
        Resource::Stone => "stone",
        Resource::Iron => "iron",
        Resource::Oil => "oil",
        Resource::Coal => "coal",
        _ => "goods",
    }
}

/// What tearing these buildings down gives back (`World::demolish_refund`) – the ones that may be torn down.
fn refund_text(world: &World, buildings: &[usize]) -> String {
    let mut sum = [0; 5];
    for &b in buildings.iter().filter(|&&b| world.can_demolish(b)) {
        for (total, n) in sum.iter_mut().zip(world.demolish_refund(b)) {
            *total += n;
        }
    }
    let names = ["gold", "wood", "stone", "coal", "iron"];
    let parts: Vec<String> = sum.iter().zip(names).filter(|(n, _)| **n > 0).map(|(n, name)| format!("{n} {name}")).collect();
    if parts.is_empty() { "nothing comes back".to_owned() } else { format!("back: {}", parts.join(", ")) }
}

/// The price on a button: what it trains or places – for Repair what repairing the selection costs right now
/// (`None`: nothing that may be repaired).
fn shown_cost(button: Button, world: &World, ui: &Ui) -> Option<u32> {
    match button {
        Button::Repair => repair_price(&world.repair_plan(ui.selected_building_indices(world))),
        _ => button.cost(),
    }
}

/// The price of a repair plan – `None` if nothing in it may be repaired.
fn repair_price(plan: &RepairPlan) -> Option<u32> {
    (plan.damaged > plan.blocked).then_some(plan.price)
}

/// Why a repair plan has no price: nothing damaged, or zombies too close.
fn nothing_to_repair(plan: &RepairPlan) -> &'static str {
    if plan.blocked > 0 { "zombies too close" } else { "nothing to repair" }
}

/// The colony's stock on the right, like TAB's (`@ResourcesArea`): people, food and power on the left with TAB's icons,
/// goods with a bar on the right – with what comes in (or goes out) per payout next to it.
fn draw_stock(layout: &Layout, world: &World) {
    let mouse = mouse();
    let mut tooltip: Option<[&str; 2]> = None;

    let power = world.power();
    let flow = world.flow(&power);
    // People, food, power.
    let colonists: u32 = world.buildings.iter().filter(|b| !b.infected).map(|b| b.stats().colonists).sum();
    let (used, workers) = world.workers();
    let workers_help = "Free / all. Tents (and the command center) give them, every unit and most buildings take some \
        as long as they live or stand. None free: nothing new. An infected tent gives none.";
    let (made, needed): (u32, u32) = (power.made.iter().sum(), power.used.iter().sum());
    let food = world.stock.get(Resource::Food);
    let food_flow = flow[Resource::Food.index()];
    let food_text = if food_flow == 0 { food.to_string() } else { format!("{food} ({})", signed(food_flow)) };
    let power_help = "Needed / made in all grids. The command center makes some, windmills and charcoal burners more. \
        Not enough: every building in that grid works slower.";
    // TAB's places: its icon (x, y, size) and where its number starts (y) – `@ColonistsLabel`, `@Colonists` …
    let people: [(&str, &str, (f32, f32, f32), f32, String, bool, &str); 4] = [
        ("Colonists", "IconFColonist", (1079.0, 622.2, 23.9), 631.0, colonists.to_string(), false, "The colonists living in your houses (and the command center) - fed, they pay gold at every payout."),
        ("Workers", "IconFWorker", (1077.0, 643.0, 25.3), 650.0, format!("{}/{workers}", workers.saturating_sub(used)), false, workers_help),
        ("Food", "IconFFood", (1081.0, 665.2, 21.3), 671.0, food_text, food_flow < 0 && food < 10, "Every house and every soldier eats some at every payout. The command center, hunter cottages, fishing huts and farms bring it in - what doesn't fit in the storage is lost."),
        ("Energy", "IconFEnergy", (1082.0, 687.2, 20.9), 692.0, format!("{needed}/{made}"), needed > made, power_help),
    ];
    for (label, picture, (x, y, size), value_y, text, short, help) in people {
        let at = layout.tab(x, y, size, size);
        match icon(picture) {
            Some(t) => draw_picture(&t, at, WHITE),
            None => draw_fitted(label, at.center().x, at.center().y + 3.0, 26.0, 9.0, DIM),
        }
        let value = layout.tab(1105.0, value_y, 36.0, 15.0);
        draw_fitted_left(&text, value.x, value.bottom() - 3.0, value.w, 13.0, if short { TOO_DEAR } else { GLOW });
        if Rect::new(at.x, value.y - 2.0, value.right() - at.x, value.h + 4.0).contains(mouse) {
            tooltip = Some([label, help]);
        }
    }

    // Goods: gold and the stock, with what comes in (or goes out) per payout. Gold: what was paid last payout minus
    // the upkeep now – in debt it shows the debt, red.
    let credit_help = format!("IN THE BANKS' CREDIT ({} of {}) - no penalties, but {} gold interest at the next payout. New gold pays it off first. With Shift you can buy into the rest of it.", world.debt.min(world.credit_line()), world.credit_line(), world.interest_due());
    let gold_help = match world.debt_stage() {
        DebtStage::None if world.debt > 0 => credit_help.as_str(),
        DebtStage::None => "Pays for units and buildings. The command center and fed houses pay it at every payout, units and running buildings cost upkeep. What doesn't fit in the storage (more with banks) is lost.",
        DebtStage::Debt => "IN DEBT - the upkeep was more than your gold. Units do less damage and shoot slower. Switch buildings off (On/Off)?",
        DebtStage::Deep => "DEEP IN DEBT - units do much less damage, shoot and walk slower. Soon they desert.",
        DebtStage::Desert => "UNITS DESERT - every few seconds one leaves for good, until the debt is smaller.",
    };
    let net = (world.income() as i64 - world.upkeep() as i64) * 1000;
    let good = |r: Resource| (world.stock.get(r), world.storage(r), flow[r.index()]);
    // TAB has an icon for gold, wood, stone and iron – iron ore and coal (ours) get their name instead.
    let goods: [(&str, Option<&str>, (u32, u32, i64), &str); 7] = [
        ("Gold", Some("IconFGold"), (world.gold, world.gold_storage(), net), gold_help),
        ("Wood", Some("IconFWood"), good(Resource::Wood), "From sawmills. Buildings cost it, charcoal burners burn it."),
        ("Stone", Some("IconFStone"), good(Resource::Stone), "From mines on stone patches. Buildings cost it."),
        ("Iron ore", None, good(Resource::IronOre), "From mines on iron patches - the foundry smelts iron of it."),
        ("Coal", None, good(Resource::Coal), "From mines on coal patches - for barrels and the foundry."),
        ("Iron", Some("IconFIron"), good(Resource::Iron), "From the foundry: 2 iron ore and 1 coal each. The machine gun tower costs it."),
        ("Oil", Some("IconFOil"), good(Resource::Oil), "From oil platforms on oil nodes (and the Transmutator). TAB's late units and buildings cost it, they burn some every payout."),
    ];
    // TAB's five bars 18 pixels apart (`@GoldValue` … `@OilValue`) – our seven closer.
    let every = (crate::sim::payout_ticks() as f32 / TICKS_PER_SECOND).round();
    for (k, (label, picture, (value, limit, per_hour), help)) in goods.into_iter().enumerate() {
        let y = 619.0 + k as f32 * 13.3;
        let at = layout.tab(1140.0, y - 2.5, 16.0, 16.0);
        match picture.and_then(icon) {
            Some(t) => draw_picture(&t, at, WHITE),
            None => draw_fitted(if label == "Coal" { "Coal" } else { "Ore" }, at.center().x, at.bottom() - 4.0, 19.0, 9.0, DIM),
        }
        let bar = layout.tab(1160.0, y, 72.0, 11.0);
        draw_rectangle(bar.x, bar.y, bar.w, bar.h, Color::new(0.0, 0.0, 0.0, 0.45));
        // How full the storage is – full, and what comes in is lost.
        let full = (value as f32 / limit.max(1) as f32).min(1.0);
        draw_rectangle(bar.x, bar.y, bar.w * full, bar.h, if value >= limit { STORAGE_FULL } else { STORAGE });
        draw_rectangle_lines(bar.x, bar.y, bar.w, bar.h, 1.0, BORDER);
        // In debt the gold shows the debt: yellow while it's all inside the banks' credit, red beyond it.
        let (value, color) = match label {
            "Gold" if world.debt > 0 && world.penalty_debt() == 0 => (format!("-{} of {}", world.debt, world.credit_line()), KEY),
            "Gold" if world.debt > 0 => (format!("-{}", world.debt), TOO_DEAR),
            _ => (format!("{value}/{limit}"), WHITE),
        };
        draw_fitted(&value, bar.center().x, bar.bottom() - 2.0, bar.w - 2.0, 10.5, color);
        if per_hour != 0 {
            let color = if per_hour < 0 { TOO_DEAR } else { GLOW };
            draw_fitted_left(&signed(per_hour), bar.right() + 3.0, bar.bottom() - 2.0, 34.0, 10.0, color);
        }
        if Rect::new(at.x, bar.y - 2.0, bar.right() + 36.0 - at.x, 15.0).contains(mouse) {
            tooltip = Some([label, help]);
        }
    }
    if let Some([label, help]) = tooltip {
        let title = format!("{label} - the number right of a bar comes in (or goes out) every {every} s");
        let title = if people_label(label) { label.to_owned() } else { title };
        draw_tooltip(&[&title, help], layout.stock.x, layout.top - 4.0);
    }
}

/// One of the people's rows (no bar, no payout number)?
fn people_label(label: &str) -> bool {
    matches!(label, "Colonists" | "Workers" | "Food" | "Energy")
}

/// Thousandths per payout with a sign: "+2.5", "-1".
fn signed(milli: i64) -> String {
    if milli > 0 { format!("+{}", per_hour(milli)) } else { per_hour(milli) }
}

/// A box with text above the panel. `x`: left edge (or right edge if there's not enough room),
/// `bottom`: bottom edge. The first line is bright, the rest grey.
fn draw_tooltip(lines: &[&str], x: f32, bottom: f32) {
    let line_h = 17.0;
    let w = lines.iter().map(|line| measure_text(line, None, 15, 1.0).width).fold(0.0, f32::max) + 16.0;
    let h = lines.len() as f32 * line_h + 8.0;
    // The width of the map's part of the window in the pixels drawn in right now (TAB's while the bar is drawn).
    let width = crate::view::rect().w / DRAWING.get();
    let x = if x + w > width { (width - w).max(0.0) } else { x };
    let y = bottom - h;
    draw_rectangle(x, y, w, h, PANEL_BG);
    draw_rectangle_lines(x, y, w, h, 1.0, BRASS);
    for (k, line) in lines.iter().enumerate() {
        let color = if k == 0 { WHITE } else { DIM };
        draw_text(line, x + 8.0, y + 17.0 + k as f32 * line_h, 15.0, color);
    }
}

/// The gray-blue of a shield – the part of the life bar behind the hit points.
pub const SHIELD: Color = Color::new(0.50, 0.58, 0.70, 1.0);

/// One life bar for hit points and shield (`(now, max)` each): the whole bar is both together, first the hit points
/// in their colour (green, yellow, red), then what's left of the shield in gray-blue. Without a shield it's a plain
/// health bar. `border`: a frame around it (the panel), or none (above a building).
pub fn draw_life(bar: Rect, hp: (u32, u32), shield: (u32, u32), background: Color, border: Option<Color>) {
    let total = (hp.1 + shield.1).max(1) as f32;
    let hp_w = bar.w * hp.0 as f32 / total;
    let shield_w = bar.w * shield.0 as f32 / total;
    draw_rectangle(bar.x, bar.y, bar.w, bar.h, background);
    draw_rectangle(bar.x, bar.y, hp_w, bar.h, hp_color(hp.0 as f32 / hp.1.max(1) as f32));
    draw_rectangle(bar.x + hp_w, bar.y, shield_w, bar.h, SHIELD);
    if let Some(border) = border {
        draw_rectangle_lines(bar.x, bar.y, bar.w, bar.h, 1.0, border);
    }
}

/// Green at full health, yellow from half, red below a quarter.
pub fn hp_color(frac: f32) -> Color {
    if frac > 0.5 {
        GREEN
    } else if frac > 0.25 {
        YELLOW
    } else {
        RED
    }
}

fn unit_kind(world: &World, id: u32) -> Option<UnitKind> {
    world.find_unit(id).map(|i| world.units[i].kind)
}

fn text_width(text: &str, size: f32) -> f32 {
    measure_text(text, None, size as u16, 1.0).width
}

/// Text centred around x, baseline at y.
fn draw_centered(text: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(text, x - text_width(text, size) / 2.0, y, size, color);
}

/// Text ending at x, baseline at y.
fn draw_right(text: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(text, x - text_width(text, size), y, size, color);
}

/// Text with a dark shadow – readable on a picture (the key and the price on a button).
fn draw_shadowed(text: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(text, x + 1.0, y + 1.0, size, Color::new(0.0, 0.0, 0.0, 0.85));
    draw_text(text, x, y, size, color);
}

/// The font size at which `text` fits into `max_w` – `size`, or smaller.
fn fitted_size(text: &str, max_w: f32, size: f32) -> f32 {
    let w = text_width(text, size);
    if w > max_w { size * max_w / w } else { size }
}

/// Text centred around x – gets smaller if it doesn't fit into `max_w`.
fn draw_fitted(text: &str, x: f32, y: f32, max_w: f32, size: f32, color: Color) {
    draw_centered(text, x, y, fitted_size(text, max_w, size), color);
}

/// Text starting at x – gets smaller if it doesn't fit into `max_w`.
fn draw_fitted_left(text: &str, x: f32, y: f32, max_w: f32, size: f32, color: Color) {
    draw_text(text, x, y, fitted_size(text, max_w, size), color);
}

/// Text broken into lines of at most `max_w`, the first baseline at y – at most `max_lines` lines, the last one
/// ends with "..." if the text goes on.
pub fn draw_wrapped(text: &str, x: f32, y: f32, max_w: f32, size: f32, max_lines: usize, color: Color) {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if text_width(&format!("{line} {word}"), size) <= max_w => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            last.push_str(" ...");
        }
    }
    for (k, line) in lines.iter().enumerate() {
        draw_text(line, x, y + k as f32 * (size + 2.0), size, color);
    }
}

/// The map's goals (its story, `sim/story.rs`) in the top left corner, like TAB's mission goals: each with how far along
/// it is – a tick once reached. Defeat goals in red.
/// Where the goals' box ends on screen – `None`: the map has no goals. The event list goes below it.
fn goals_bottom(world: &World) -> Option<f32> {
    let goals = world.story.goals.len();
    (goals > 0).then(|| 46.0 + 22.0 * goals as f32 + 26.0)
}

pub fn draw_goals(world: &World) {
    let goals = &world.story.goals;
    if goals.is_empty() {
        return;
    }
    // In the corner of the map's part of the window – `goals_bottom` counts from there too.
    let corner = crate::view::rect().point();
    let (x, mut y) = (corner.x + 12.0, corner.y + 64.0);
    let w = 340.0;
    draw_rectangle(x - 6.0, y - 18.0, w, 22.0 * goals.len() as f32 + 26.0, Color::new(0.0, 0.0, 0.0, 0.55));
    draw_text("Goals", x, y, 18.0, BRASS);
    for (g, goal) in goals.iter().enumerate() {
        y += 22.0;
        let progress = world.goal_progress(g);
        let mark = if progress >= 100 { "[x]" } else { "[ ]" };
        let color = if goal.defeat { TOO_DEAR } else if progress >= 100 { GREEN } else { WHITE };
        let line = if progress >= 100 || progress == 0 { format!("{mark} {}", goal.text) } else { format!("{mark} {} - {progress} %", goal.text) };
        draw_fitted_left(&line, x, y, w - 12.0, 16.0, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every card: no two buttons on the same spot (they'd share a key and one would hide the other).
    #[test]
    fn no_two_buttons_share_a_spot() {
        let mut cards: Vec<Vec<Button>> =
            vec![UNIT_BUTTONS.to_vec(), COMMAND_CENTER_BUTTONS.to_vec(), BARRACKS_BUTTONS.to_vec(), TOWER_BUTTONS.to_vec()];
        // Every building's card gets On/Off when it can be switched off (`Ui::card`).
        for card in [BARRACKS_BUTTONS.as_slice(), &TOWER_BUTTONS, &REPAIR_BUTTONS] {
            cards.push(card.iter().copied().chain([Button::Switch]).collect());
        }
        let menus = [Submenu::Colonists, Submenu::Resources, Submenu::Energy, Submenu::Industry, Submenu::Military, Submenu::Defense, Submenu::Wonders];
        cards.extend(menus.map(menu_buttons));
        for card in &cards {
            for (k, a) in card.iter().enumerate() {
                for b in &card[k + 1..] {
                    assert_ne!(a.slot(), b.slot(), "{a:?} and {b:?} on the same spot");
                }
            }
        }
    }
}

/// Armor like TAB's (a share of every hit) as the panel shows it: "40 %".
fn percent(share: f32) -> String {
    format!("{} %", (share * 100.0).round())
}
