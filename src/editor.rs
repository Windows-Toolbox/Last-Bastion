//! The map editor (only in `LastBastionEditor.exe` – the game has none, `available`), laid out like StarCraft 2's
//! editor (`docs\editor.md`):
//! - At the top the **menu bar** (File, Edit, View, Map, Test, Help – and the file's name, with • while there are
//!   changes not saved yet), below it the **module bar**: Terrain · Objects · Zombies | Triggers · Events · Map, and on
//!   the right Undo, Redo, Save and **Test (F10)**.
//! - **Left** the module's tools, **right** the properties of what's selected, in the **middle** the map – with small
//!   bars floating over it (whose eyes, grid, zoom; at the bottom left what the mouse does right now).
//! - At the bottom a **status line**: the tile under the mouse, its ground, the selection, what's in hand, undo, the
//!   day and hour.
//!
//! The map only gets the room between the panels (`view.rs`): the camera looks at its middle, TAB's bar with the
//! minimap sits at its bottom, clicks beside it go to the panels.
//!
//! In **Edit** no time passes, you change the map; in **Test** the round runs from the map as it is, with all tools
//! still at hand – back in Edit the map is as it was before the test. Either way you see the whole map, including the
//! ring outside the border where the zombies come from.
//!
//! The modules (what TAB's editor has, DESIGN.md 12.6, sorted SC2's way):
//! - **Terrain** (`editor\terrain.rs`) – paint mountains, forest, water, erase, ore, big rocks (TAB's painter) – as a
//!   brush, a line, a rectangle or a fill, solid or scattered, with the pictures of your choice; smooth; select an
//!   area, copy its ground, paste it, generate a forest, a lake, mountains, ore or a village of doom in it; which of
//!   TAB's landscapes the map is drawn as. On the right a minimap, what of the map is shown (SC2's layers) and what
//!   lies under the mouse.
//! - **Objects** (`editor\objects.rs`) – a catalog of every kind with its picture (buildings without the rules of
//!   the game, units, houses of doom, decoration, the railway, loot), whose the next one is, the eraser – below the
//!   map the table of everything placed (TAB's entity list), on the right the properties of what's selected.
//! - **Zombies** – brush, drop, giant, spawners, the houses of a village of doom (TAB's three sizes).
//! - **Regions** (`editor\regions.rs`) – named regions (rectangles, circles, painted tiles, put together) and points,
//!   like SC2's: what triggers and events talk about. With the playing field, the base and the spawners in its lists.
//! - **Triggers** (`editor\triggers.rs`, `story_panel\sentences.rs`) – a full page: the tree of triggers, goals, the
//!   command center's own button and variables; the chosen trigger as sentences; the palette of actions.
//! - **Events** – the map's **events** like TAB's: each comes by itself at its day and hour (plus a random offset),
//!   maybe again and again growing each time, announced or silent, with groups of several breeds from sides or chosen
//!   spawners – the stragglers, the swarms, the final wave and the victory are all events. On the right: send waves now.
//! - **Map** – name, TAB's "Info", the round (days, gold and wood at the start) and where the playing field lies in the
//!   whole map (SC2's map bounds and map size).
//!
//! A right click on a spawner opens its properties in a small window of their own (`properties_window`).
//!
//! Whatever is selected – buildings, units, zombies, houses, loot, spawners, decoration; a box in Edit takes all of it –
//! can be copied, cut and pasted (Ctrl+C, Ctrl+X, Ctrl+V – `picking.rs`, `sim/clipboard.rs`), with everything it has.
//! Zoomed out, the zombies show as coloured tiles, one colour per breed (`ZombieLook`).
//!
//! It's made with **egui** (DESIGN.md 12.9): fields to type numbers and names into (or drag them), drop-down lists,
//! menus, sections that fold, lists that scroll, panels at the edges of the window. egui works like macroquad's
//! drawing – *immediate mode*: every frame `show` describes the whole editor anew, and what was clicked or typed comes
//! back right there. The editor only reads the world; whatever it changes goes out as an order (`Order`), so it stays
//! deterministic. What the left mouse button does on the map with a tool in hand is decided by `ui.rs`, because that's
//! where the mouse is read anyway. Saving, closing and switching between Edit and Test only `game.rs` can do – the
//! editor asks for it (`request`).

use egui_macroquad::egui::{
    self, text::LayoutJob, Align, Align2, CollapsingHeader, Color32, ComboBox, DragValue, FontId, Frame, RichText,
    ScrollArea, Slider, TextEdit, TextFormat, TextStyle, Ui,
};
use macroquad::math::Vec2;

mod events;
mod map_page;
mod objects;
mod regions;
mod terrain;
mod thumbs;
mod triggers;

pub(crate) use events::{from_label, wave_group};
pub use regions::Picked;
pub use terrain::{fill_tiles, rect_tiles, PaintShape};

use crate::draw::{breed_tile_color, Layers, ZOMBIE_TILES_BELOW};
use crate::hud::Layout;
use crate::iso::{Camera, START_ZOOM};
use crate::kinds::{BuildingKind, UnitKind};
use crate::map::{BigRock, Ore, Terrain};
use crate::map::BORDER;
use crate::sim::{
    rail_pieces, snap_rail, Area, Axes, Breed, Clip, Nest, Order, OrnamentKind, Owner, Patch, Piece, SpawnMode, Spawner, TerrainClip,
    WaveFrom, World, Zone, MAP_SIZES, MOST_PER_TILE,
};

/// How wide the panel on the left starts – for the modules with tools, and the wider one for those with pages of
/// fields (Triggers, Events, Map). Both can be dragged wider or narrower at their edge; egui remembers it.
/// egui counts in points – here the same as macroquad's pixels.
const TOOLS_W: f32 = 290.0;
const PAGES_W: f32 = 380.0;
/// How wide the properties on the right start.
const PROPERTIES_W: f32 = 300.0;
/// The floating bars keep this far from the edges of the map.
const FLOAT_PAD: f32 = 8.0;
/// A spawner name and a map name have at most this many letters.
const MAX_NAME: usize = 24;
/// The round lasts at most this many days.
pub(crate) const MAX_DAYS: u32 = 200;
/// The most a number field takes (gold, wood, zombies) – far more than a map needs, only so nothing overflows.
pub(crate) const MAX_AMOUNT: u32 = 10_000_000;
/// The final wave stands out among the events – and so does a group with no spawner ticked.
const FINAL: Color32 = Color32::from_rgb(242, 115, 89);
/// The Test button: green while you can start a test, orange while one runs.
const TEST_FILL: Color32 = Color32::from_rgb(53, 90, 39);
const TESTING_FILL: Color32 = Color32::from_rgb(122, 72, 24);
/// What the zombie brush and drop can place: every breed but the giant – it has a tool of its own (`Tool::Giant`),
/// one at a time. A new breed in `Breed::ALL` gets its slider by itself.
fn small_breeds() -> Vec<Breed> {
    Breed::ALL.into_iter().filter(|&b| b != Breed::Giant).collect()
}

/// Editing the map (no time passes) or testing it (the round runs).
#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Edit,
    Test,
}

/// The modules of the module bar, like SC2's (Terrain, Units, Regions … Triggers, Data). Data comes with the step
/// that builds it (`docs\editor.md`) – a button that does nothing yet isn't shown.
#[derive(Clone, Copy, PartialEq)]
pub enum Module {
    Terrain,
    Objects,
    Zombies,
    Regions,
    Triggers,
    Events,
    Map,
}

impl Module {
    /// The ones that work on the map, then – after a line – the ones with pages of fields.
    const ON_MAP: [Module; 4] = [Module::Terrain, Module::Objects, Module::Zombies, Module::Regions];
    const PAGES: [Module; 3] = [Module::Triggers, Module::Events, Module::Map];
    /// Pages that fill the whole room between the bars (no panels, no map): the map is covered and takes no clicks.
    const FULL: [Module; 3] = [Module::Triggers, Module::Events, Module::Map];

    fn name(self) -> &'static str {
        match self {
            Module::Terrain => "Terrain",
            Module::Objects => "Objects",
            Module::Zombies => "Zombies",
            Module::Regions => "Regions",
            Module::Triggers => "Triggers",
            Module::Events => "Events",
            Module::Map => "Map",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Module::Terrain => "Ground, forest, water, ore, big rocks",
            Module::Objects => "Units, buildings, loot - and the list of all placed",
            Module::Zombies => "Zombies by brush or by the crowd, spawners, villages of doom",
            Module::Regions => "Named regions and points - what triggers and events talk about",
            Module::Triggers => "TAB's game narrative the SC2 way: triggers as sentences, goals, the button, variables",
            Module::Events => "Waves, the final wave, victory - at their day and hour",
            Module::Map => "Name, info, landscape, the round, difficulty, goals",
        }
    }
}

/// Where a part of the editor comes from – a small badge on its heading, like on the drawings in `docs\editor.md`.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Origin {
    /// From TAB's editor (yellow).
    Tab,
    /// The way SC2's editor does it (blue).
    Sc2,
    /// Neither – our own (green).
    New,
}

impl Origin {
    /// The badge's text, its letters' colour and its background.
    fn look(self) -> (&'static str, Color32, Color32) {
        match self {
            Origin::Tab => ("TAB", Color32::from_rgb(240, 207, 122), Color32::from_rgb(74, 58, 20)),
            Origin::Sc2 => ("SC2", Color32::from_rgb(156, 198, 245), Color32::from_rgb(29, 58, 92)),
            Origin::New => ("new", Color32::from_rgb(159, 224, 166), Color32::from_rgb(36, 70, 42)),
        }
    }
}

/// Whose eyes the editor looks through. In the game it's always the player's.
#[derive(Clone, Copy, PartialEq)]
pub enum Sight {
    /// No fog – everything, everywhere.
    All,
    /// Fog of war like in the game: what your units and buildings see (in co-op both players share it,
    /// DESIGN.md 12.8 – so there is only one).
    Player,
    /// What the zombies see: lit where at least one zombie looks, dark elsewhere. Your units in the dark are
    /// safe from being spotted.
    Zombies,
}

/// What the left mouse button does on the map while the editor has a tool in hand.
#[derive(Clone, Copy, PartialEq)]
pub enum Tool {
    /// Paint this terrain (hold and drag). `Grass` = rubber.
    Paint(Terrain),
    /// Paint ore on grass (hold and drag). `None` = take the ore away.
    Ore(Option<Ore>),
    /// Smooth the ground under the brush: every tile takes what most of its neighbours are (hold and drag).
    Smooth,
    /// Drag a rectangle of tiles on the map: the area the Terrain module copies and generates in (`Editor::area`).
    Area,
    /// The copied ground of an area on the mouse (`Editor::terrain_clip`) – a click puts it down, again and again.
    PasteArea,
    /// The eraser: everything on the brush area goes – buildings, units, zombies (hold and drag).
    Remove,
    /// Zombie brush, like TAB: on every tile of the brush area so many of each breed (`Editor::per_tile`) – hold and
    /// drag, painting over a tile again only fills it up.
    ZombieBrush,
    /// A whole crowd of zombies at once at the click.
    ZombieDrop,
    /// One giant on the tile clicked.
    Giant,
    /// The playing field's edges (the world border) can be grabbed and dragged – inside it, it moves as a whole.
    PlayArea,
    /// One unit at the click.
    Unit(UnitKind),
    /// A new spawner at the click.
    Spawner,
    /// The Regions module (`regions.rs`): drag a rectangle or a circle of tiles, or paint tiles with the brush – a new
    /// region, or with Ctrl / Alt a shape added to / cut out of the selected one. `Spot`: a named point at the click.
    ZoneRect,
    ZoneCircle,
    ZonePaint,
    Spot,
    /// A house of a village of doom at the click – small, medium or big like TAB's (2 × 2, 3 × 3, 5 × 5 tiles,
    /// `Nest::side`), with as many zombies inside as the generator would put there.
    Doom(Nest),
    /// A pile of loot at the click – what lies in it and its hidden guards rolled like the generator does.
    Loot,
    /// One of TAB's big rocks of this many tiles each way (`map::BIG_ROCK_SIZES`) at the click – its middle under the
    /// mouse. Its tiles become mountain; painting other ground over it takes it away.
    BigRock(i32),
    /// A piece of TAB's railway at the click (`Editor::ornament_spot`): track snaps onto an open end close by.
    Ornament(OrnamentKind),
    /// Factorio's rail planner (`ui.rs`): click an open end of the track (or empty ground for a new line) – arrows show
    /// where it can go on, the piece the mouse points to stands there as a ghost; a click builds it, holding the button
    /// builds on towards the mouse.
    RailPlanner,
    /// A building, without the rules of the game. Not really held by the editor: `ui.rs` hangs it on the mouse
    /// like a building of the command center (`placing`), so dragging lines and turning (R) work the same.
    Building(BuildingKind),
}

impl Tool {
    pub fn name(self) -> &'static str {
        match self {
            Tool::Paint(Terrain::Grass) => "Erase",
            Tool::Paint(terrain) => terrain.name(),
            Tool::Ore(Some(ore)) => ore.name(),
            Tool::Ore(None) => "No ore",
            Tool::Smooth => "Smooth",
            Tool::Area => "Select area",
            Tool::PasteArea => "Paste",
            Tool::Remove => "Remove all on it",
            Tool::ZombieBrush => "Brush",
            Tool::ZombieDrop => "Drop",
            Tool::Giant => "Giant",
            Tool::PlayArea => "Drag the border",
            Tool::Unit(kind) => &kind.stats().name,
            Tool::Spawner => "+ Spawner",
            Tool::ZoneRect => "+ Rectangle",
            Tool::ZoneCircle => "+ Circle",
            Tool::ZonePaint => "+ Paint",
            Tool::Spot => "+ Point",
            Tool::Doom(Nest::Small) => "Small house 2 x 2",
            Tool::Doom(Nest::Medium) => "Medium house 3 x 3",
            Tool::Doom(Nest::Big) => "Big house 5 x 5",
            Tool::Loot => "Loot",
            Tool::BigRock(3) => "3 x 3",
            Tool::BigRock(4) => "4 x 4",
            Tool::BigRock(_) => "5 x 5",
            Tool::Ornament(kind) => kind.name(),
            Tool::RailPlanner => "Rail planner",
            Tool::Building(BuildingKind::CommandCenter) => "CC",
            Tool::Building(kind) => &kind.stats().name,
        }
    }

    /// One of the catalog's things put down with a click (`objects.rs`): after it the tool goes out of the hand –
    /// unless Shift is held, like in SC2's editor.
    pub fn once(self) -> bool {
        matches!(self, Tool::Unit(_) | Tool::Doom(_) | Tool::Loot | Tool::Ornament(_))
    }

    /// Does it paint along the way the mouse is dragged (terrain, ore, smoothing, the eraser, the zombie brush)?
    pub fn paints(self) -> bool {
        matches!(self, Tool::Paint(_) | Tool::Ore(_) | Tool::Smooth | Tool::Remove | Tool::ZombieBrush)
    }
}

/// What the editor wants from outside, because it can't do it itself (`game.rs` takes it).
#[derive(Clone, Copy, PartialEq)]
pub enum EditorRequest {
    /// "New map…": to the setup screen.
    NewMap,
    /// "Save" (or Ctrl+S).
    Save,
    /// "Close": back to the editor's home screen.
    Close,
    /// Edit ↔ Test (the Test button, the Test menu, F10).
    Mode(Mode),
    /// "Undo" (Ctrl+Z) and "Redo" (Ctrl+Y).
    Undo,
    Redo,
    /// "Locate": the camera to this point (TAB's "Locate on map").
    Locate(Vec2),
}

/// The clipboard's commands from the Edit menu – `picking.rs` carries them out, like the keys.
#[derive(Clone, Copy, PartialEq)]
pub enum ClipAction {
    Cut,
    Copy,
    Paste,
    Delete,
}

/// How the zombies are drawn: as figures, or as tiles in the colour of their breed (TAB's editor paints its zombies
/// as cells) – by itself when zoomed out, always or never.
#[derive(Clone, Copy, PartialEq)]
pub enum ZombieLook {
    Auto,
    Tiles,
    Figures,
}

/// Which placed things the list shows (TAB: "Filter by selected Template", "Show All").
#[derive(Clone, Copy, PartialEq)]
pub enum Filter {
    All,
    Buildings,
    Units,
    Neutral,
    Tagged,
    /// Neutral things with a script that runs when they join.
    Scripted,
}

/// The kinds of waves the editor can send right now – all of them from the spawners.
#[derive(Clone, Copy, PartialEq)]
pub enum Wave {
    /// 150 zombies from one random spawner.
    Small,
    /// 500 zombies from the spawners of one random side.
    Side,
    /// 500 zombies from the spawners of each of two random sides.
    TwoSides,
}

impl Wave {
    fn order(self) -> Order {
        match self {
            Wave::Small => Order::SpawnWave { count: 150, from: WaveFrom::OneSpawner },
            Wave::Side => Order::SpawnWave { count: 500, from: WaveFrom::Sides(1) },
            Wave::TwoSides => Order::SpawnWave { count: 500, from: WaveFrom::Sides(2) },
        }
    }
}

/// What the editor may change besides sending orders – lent to `show` by `game.rs` for one frame: the camera (zoom,
/// the floating bar) and whether the grid is shown (a setting of the game, `Settings::show_grid`).
pub struct Around<'a> {
    pub world: &'a World,
    pub orders: &'a mut Vec<Order>,
    pub camera: &'a mut Camera,
    pub grid: &'a mut bool,
    /// `false` while the menu or a question is in front – then the editor is only shown, greyed out.
    pub enabled: bool,
}

pub struct Editor {
    /// Is there an editor at all? Only in `LastBastionEditor.exe` – in the game nothing of it is drawn or clickable,
    /// and the whole map is never shown.
    pub available: bool,
    pub mode: Mode,
    pub module: Module,
    /// The tool in hand – `None`: the left mouse button selects as usual.
    pub tool: Option<Tool>,
    /// Brush size in tiles (edge length, odd).
    pub brush: i32,
    /// The selected spawner (its `id`) – its edge glows and can be dragged.
    pub spawner: Option<u32>,
    /// The selected spawner's properties are open in a window of their own (right click on it) – and where it goes
    /// when it's shown next (screen, once: then it stays where it's dragged to).
    properties: bool,
    properties_at: Option<Vec2>,
    /// "Help → Keys" is open.
    keys_open: bool,
    /// "Grid" while it's being changed – until "Apply" (`None`: the size the map has).
    grid_size: Option<usize>,
    /// While the selected spawner's name field has the keyboard: the text so far (`ui.rs` shows it on the map).
    pub name_edit: Option<String>,
    /// The same for the map name.
    map_name_edit: Option<String>,
    /// Name of the map – the file name when saving.
    pub map_name: String,
    /// TAB's "Info" of the map, and whether the game makes it anew every start – kept here until it's saved (`game.rs`).
    pub info: MapInfo,
    /// The tile under the mouse – shown in the status line (TAB's "X 50 Y 60").
    mouse_tile: Option<(i32, i32)>,
    /// Everything selected on the map, as pieces – set by `picking.rs` every frame. The table of placed things lights
    /// them up, the right panel shows their properties.
    pub selection: Vec<Piece>,
    /// For `picking.rs`: select these on the map (a click in the table, a thing that changed hands) – `true`: in or
    /// out of what's selected already.
    pub select: Option<(Vec<Piece>, bool)>,
    /// The brush round (big ones a rough circle) or square, like TAB's pencil shape.
    pub square_brush: bool,
    /// How the painting tools go over the map (`terrain.rs`): brush, line, rectangle, fill – and how many percent of
    /// the tiles they take (`stroke`: which ones – counted up with every press of the mouse).
    pub shape: PaintShape,
    pub density: u32,
    stroke: u32,
    /// The tile the mouse was on last while over the map – "Under the mouse".
    hover: Option<(i32, i32)>,
    /// Which pictures the painter gives what it paints – one choice per layer: forest, rocks, ore (0: mixed, 1–3: the
    /// landscape's group A, B, C – `Map::picture`).
    pictures: [u8; 3],
    /// "Keep buildings and tracks free": solid ground isn't painted, pasted or generated on or right next to
    /// buildings and the railway (`World::spare`).
    pub spare: bool,
    /// The area chosen with "Select area" – what "Copy" copies and "Generate" fills.
    pub area: Option<Area>,
    /// The ground copied last ("Copy", Ctrl+C with an area) – and whether that was the last thing copied at all:
    /// then Ctrl+V takes it in hand, otherwise it pastes the things copied (`clip`).
    pub terrain_clip: Option<TerrainClip>,
    pub ground_copied: bool,
    /// "Generate in the area": what, how much of the area, and the seed – the same seed makes the same patch.
    patch: Patch,
    patch_percent: u32,
    patch_seed: u32,
    /// What of the map is shown ("Show", SC2's layers).
    layers: Layers,
    /// The Regions module (`regions.rs`): the region or point selected, the text its lists are searched for, the
    /// region the paint brush is going into right now (its number, cutting or not), and how each region is drawn.
    pub picked: Option<Picked>,
    /// Delete was clicked for this region while events name it – asks once more (`regions.rs`).
    zone_asked: Option<u32>,
    region_search: String,
    region_stroke: Option<(u32, bool)>,
    outlines: Vec<regions::Outline>,
    /// The minimap of the Terrain module: its picture for egui, and the map (revision, size) it was painted from.
    minimap: Option<(egui::TextureHandle, u32, usize)>,
    /// What the table of placed things shows, and the text it's searched for.
    filter: Filter,
    placed_search: String,
    /// The catalog of the Objects module (`objects.rs`) – made when it's first shown –, its filter and its search.
    catalog: Vec<objects::Entry>,
    chip: objects::Chip,
    catalog_search: String,
    /// "Placing": whose the next building or unit is.
    pub place_owner: Owner,
    /// The small pictures of the catalog (`thumbs.rs`).
    thumbs: thumbs::Thumbs,
    /// What the Events page keeps (`events.rs`).
    events_page: events::EventsPage,
    triggers_page: triggers::TriggersPage,
    /// What map this is (generator, seed, days) – set by `game.rs`.
    pub map_info: String,
    /// What came of the last "Save" – set by `game.rs`.
    pub status: Option<String>,
    /// The map file's name in the menu bar, and whether there are changes that aren't in it – set by `game.rs`.
    pub file_name: String,
    pub unsaved: bool,
    /// For `game.rs`: something only it can do (`EditorRequest`).
    pub request: Option<EditorRequest>,
    /// How many steps undo and redo can go – set by `game.rs` (`undo.rs`).
    pub steps: (usize, usize),
    /// For `ui.rs`: a tool or building button was clicked – hang this building on the mouse (`Some(kind)`), or take
    /// whatever is on it off (`None`).
    pub place: Option<Option<BuildingKind>>,
    /// The building `ui.rs` has on the mouse right now (copied every frame, `sync`) – its button is lit.
    placing: Option<BuildingKind>,
    /// Whose view the editor shows (stays when switching back and forth).
    sight: Sight,
    /// Wandering (asleep, like the zombies a new map starts with) or storming – brush, drop and giant.
    pub wander: bool,
    /// The brush: how many of each breed on every tile (0 to `MOST_PER_TILE`).
    pub per_tile: Vec<(Breed, u32)>,
    /// The drop: which breed, how many.
    pub breed: Breed,
    pub drop: u32,
    /// Which picture a big rock gets (0–2: TAB's A, B, C) – `None`: mixed, picked by the tile (`big_rock`).
    pub rock_variant: Option<u8>,
    /// The zombies as figures or as tiles (View menu, the bar over the map).
    zombie_look: ZombieLook,
    /// What Ctrl+C or Ctrl+X copied last – Ctrl+V puts it down again (`picking.rs`).
    pub clip: Option<Clip>,
    /// An entry of the Edit menu clicked: cut, copy, paste, delete – `picking.rs` takes it.
    pub clip_request: Option<ClipAction>,
    /// How many pieces are selected, and what they are when it's more than units and buildings ("40 zombies") – set
    /// by `picking.rs` every frame, for the Edit menu and the status line.
    pub pieces: usize,
    pub picked_text: Option<String>,
    /// Is the mouse on a panel or a floating bar, or is one of their fields being dragged? Then the map doesn't get it
    /// (`show`).
    on_panel: bool,
    /// Does a field of the panel have the keyboard – now, or until the end of the last frame (`show`)?
    typing: bool,
    /// Did one have it at the end of the last frame?
    typed_before: bool,
    /// Are the panel's fonts set yet (`style`)?
    styled: bool,
}

impl Editor {
    /// `available`: the editor program (`true`) or the game (`false`).
    pub fn new(available: bool) -> Editor {
        Editor {
            available,
            mode: Mode::Edit,
            // Like SC2's editor, which opens in its terrain module.
            module: Module::Terrain,
            tool: None,
            brush: 3,
            spawner: None,
            properties: false,
            properties_at: None,
            keys_open: false,
            grid_size: None,
            name_edit: None,
            map_name_edit: None,
            map_name: "Untitled".to_owned(),
            info: MapInfo::default(),
            mouse_tile: None,
            selection: Vec::new(),
            select: None,
            square_brush: false,
            shape: PaintShape::Brush,
            density: 100,
            stroke: 0,
            hover: None,
            pictures: [0; 3],
            spare: false,
            area: None,
            terrain_clip: None,
            ground_copied: false,
            patch: Patch::Forest,
            patch_percent: 60,
            patch_seed: 1,
            layers: Layers::default(),
            picked: None,
            region_search: String::new(),
            region_stroke: None,
            outlines: Vec::new(),
            minimap: None,
            filter: Filter::All,
            placed_search: String::new(),
            catalog: Vec::new(),
            chip: objects::Chip::All,
            catalog_search: String::new(),
            place_owner: Owner::Colony,
            thumbs: thumbs::Thumbs::default(),
            events_page: events::EventsPage::default(),
            triggers_page: triggers::TriggersPage::default(),
            zone_asked: None,
            map_info: String::new(),
            status: None,
            file_name: String::new(),
            unsaved: false,
            request: None,
            steps: (0, 0),
            place: None,
            placing: None,
            sight: Sight::All,
            // Like the zombies a new map starts with – asleep, strolling around.
            wander: true,
            // A walker on every tile to begin with – the other sliders at 0.
            per_tile: small_breeds().into_iter().map(|b| (b, u32::from(b == Breed::Walker))).collect(),
            breed: Breed::Walker,
            drop: 100,
            rock_variant: None,
            zombie_look: ZombieLook::Auto,
            clip: None,
            clip_request: None,
            pieces: 0,
            picked_text: None,
            on_panel: false,
            typing: false,
            typed_before: false,
            styled: false,
        }
    }

    /// Every frame: the building `ui.rs` has on the mouse – its button is lit – and the tile under the mouse.
    pub fn sync(&mut self, placing: Option<BuildingKind>, mouse_tile: (i32, i32)) {
        self.placing = placing;
        self.mouse_tile = Some(mouse_tile);
        if !self.on_panel {
            self.hover = Some(mouse_tile);
        }
    }

    /// The drop: `drop` zombies around `center`, in the radius of the brush – at least as big as the crowd needs.
    pub fn drop_order(&self, center: Vec2) -> Order {
        let radius = self.drop_radius();
        Order::PlaceZombies { center, radius, count: self.drop as usize, breed: self.breed, wander: self.wander }
    }

    /// The radius (tiles) a drop lands in: the brush – at least as big as its crowd needs.
    pub fn drop_radius(&self) -> f32 {
        (self.brush as f32 / 2.0).max(0.5 + (self.drop as f32).sqrt() * 0.3)
    }

    /// The zombie brush on these tiles (`Order::PaintZombies`) – only the breeds whose slider isn't at 0.
    pub fn brush_order(&self, tiles: Vec<(i32, i32)>) -> Order {
        let per_tile = self.per_tile.iter().copied().filter(|&(_, n)| n > 0).collect();
        Order::PaintZombies { tiles, per_tile, wander: self.wander }
    }

    /// What the brush puts on a tile, as a short text on the mouse: "2 Walker, 1 Runner".
    pub fn brush_text(&self) -> String {
        let parts: Vec<String> = self.per_tile.iter().filter(|&&(_, n)| n > 0).map(|&(b, n)| format!("{n} {}", b.name())).collect();
        if parts.is_empty() { "nothing - push a slider".to_owned() } else { parts.join(", ") + " per tile" }
    }

    /// Are the zombies drawn as tiles at this zoom? Only in the editor – the game always has its figures.
    pub fn zombie_tiles(&self, zoom: f32) -> bool {
        self.available
            && match self.zombie_look {
                ZombieLook::Auto => zoom < ZOMBIE_TILES_BELOW,
                ZombieLook::Tiles => true,
                ZombieLook::Figures => false,
            }
    }

    /// What of the map is drawn ("Show") – in the game always everything.
    pub fn layers(&self) -> Layers {
        if self.editing() { self.layers } else { Layers::default() }
    }

    /// The editor's tools and the whole map – only in the editor program, in both modes.
    pub fn editing(&self) -> bool {
        self.available
    }

    /// Is a test running (the round runs from the edited map)?
    pub fn testing(&self) -> bool {
        self.available && self.mode == Mode::Test
    }

    /// Whose view is shown right now: in the game always the player's, in the editor the chosen one.
    pub fn sight(&self) -> Sight {
        if self.editing() { self.sight } else { Sight::Player }
    }

    /// Switches to the given mode – `game.rs` does that when it has swapped the worlds (`EditorRequest::Mode`). The
    /// test runs on a copy of the map: its spawners have other numbers, so the selected one and a name being typed go.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.spawner = None;
        self.name_edit = None;
        self.properties = false;
    }

    /// Selects a spawner (or none). A name being typed for the previous one was already saved by `show` – the click
    /// that selects took the keyboard away from the field. Its properties window closes.
    pub fn select_spawner(&mut self, spawner: Option<u32>) {
        if spawner != self.spawner {
            self.name_edit = None;
            self.properties = false;
        }
        self.spawner = spawner;
    }

    /// Right click on a spawner: select it and open its properties next to the mouse (`mouse`: on screen).
    pub fn open_properties(&mut self, spawner: u32, mouse: Vec2) {
        self.select_spawner(Some(spawner));
        self.properties = true;
        self.properties_at = Some(mouse + Vec2::new(16.0, 8.0));
    }

    /// Is something being typed into the panel (a name, a number)? Then the keyboard belongs to it and no key does
    /// anything else.
    pub fn is_typing(&self) -> bool {
        self.typing
    }

    /// Is the mouse on a panel (or dragging one of its fields)? Then clicks and the wheel don't go to the map.
    pub fn has_mouse(&self) -> bool {
        self.available && self.on_panel
    }

    /// F10: the other mode – asked for, `game.rs` switches.
    pub fn toggle(&mut self) {
        if self.available {
            self.request = Some(EditorRequest::Mode(self.other_mode()));
        }
    }

    fn other_mode(&self) -> Mode {
        if self.mode == Mode::Edit { Mode::Test } else { Mode::Edit }
    }

    /// The tiles the brush covers around `center`: a square for small brushes, a rough circle for big ones – or a
    /// square always (`square_brush`).
    pub fn brush_tiles(&self, center: (i32, i32)) -> Vec<(i32, i32)> {
        let r = self.brush / 2;
        let mut tiles = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                // r · r + r: a circle that still contains the corners of a 3 × 3 square.
                if self.square_brush || dx * dx + dy * dy <= r * r + r {
                    tiles.push((center.0 + dx, center.1 + dy));
                }
            }
        }
        tiles
    }

    /// Every frame in the editor, before `ui.rs` reads the mouse and the keys: the whole editor, built anew. `ctx` is
    /// egui's context – it remembers between frames what egui needs (which section is folded, how far a list is
    /// scrolled, how wide a panel was dragged, which field has the keyboard).
    ///
    /// egui's panels take their room from the edges of the window in the order they're added: the two bars at the top
    /// first (as wide as the window), then the status line at the bottom, then the panels left and right between them.
    /// What's left over in the middle (`available_rect`) is the map's – `view.rs` hands it to the camera and the bar.
    pub fn show(&mut self, ctx: &egui::Context, around: Around<'_>) {
        if !self.styled {
            style(ctx);
            self.styled = true;
        }
        let Around { world, orders, camera, grid, enabled } = around;
        self.refresh_outlines(world);
        egui::TopBottomPanel::top("menu bar").show(ctx, |ui| {
            // `add_enabled_ui`: everything inside greyed out and not clickable if `enabled` is false.
            ui.add_enabled_ui(enabled, |ui| self.menu_bar(ui, world, orders, camera, grid));
        });
        egui::TopBottomPanel::top("module bar").show(ctx, |ui| {
            ui.add_enabled_ui(enabled, |ui| self.module_bar(ui));
        });
        egui::TopBottomPanel::bottom("status line").show(ctx, |ui| self.status_line(ui, world));
        // A full page: one panel over the whole middle, and the map under it takes nothing (`has_mouse` is true all over).
        if Module::FULL.contains(&self.module) {
            let map = ctx.available_rect();
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_enabled_ui(enabled, |ui| match self.module {
                    Module::Triggers => self.triggers_page(ui, world, orders),
                    Module::Events => self.events_page(ui, world, orders),
                    _ => self.map_page(ui, world, orders),
                });
            });
            let k = ctx.pixels_per_point();
            crate::view::set(Some(macroquad::math::Rect::new(map.min.x * k, map.min.y * k, map.width() * k, map.height() * k)));
            self.keys_window(ctx);
            keep_focus_for_text(ctx);
            let typing = text_has_focus(ctx);
            self.typing = typing || self.typed_before;
            self.typed_before = typing;
            self.on_panel = true;
            return;
        }
        // Two left panels with names of their own, so each keeps its own width: the narrow one for tools, the wide one
        // for pages of fields.
        let pages = Module::PAGES.contains(&self.module);
        let left = if pages { egui::SidePanel::left("pages").default_width(PAGES_W) } else { egui::SidePanel::left("tools").default_width(TOOLS_W) };
        left.width_range(220.0..=700.0).show(ctx, |ui| {
            ui.add_enabled_ui(enabled, |ui| {
                ui.label(RichText::new(self.module.hint()).small().weak());
                ui.separator();
                // `id_salt`: one scroll position per module.
                let id = self.module.name();
                ScrollArea::vertical().id_salt(id).auto_shrink([false, false]).show(ui, |ui| match self.module {
                    Module::Terrain => self.terrain_module(ui, world, orders),
                    Module::Objects => self.objects_module(ui, world),
                    Module::Zombies => self.zombies_module(ui, world),
                    Module::Regions => self.regions_module(ui, world),
                    Module::Triggers => {} // a full page (`Module::FULL`), above
                    Module::Events => {} // a full page (`Module::FULL`), above
                    Module::Map => {} // a full page (`Module::FULL`), above
                });
            });
        });
        egui::SidePanel::right("properties").default_width(PROPERTIES_W).width_range(220.0..=600.0).show(ctx, |ui| {
            ui.add_enabled_ui(enabled, |ui| {
                ScrollArea::vertical().id_salt("properties").auto_shrink([false, false]).show(ui, |ui| self.properties_panel(ui, world, orders, camera, grid));
            });
        });
        // Below the map, between the side panels (they came first): the table of everything placed. Its top edge can
        // be dragged.
        if self.module == Module::Objects {
            egui::TopBottomPanel::bottom("placed").resizable(true).default_height(190.0).height_range(90.0..=500.0).show(ctx, |ui| {
                ui.add_enabled_ui(enabled, |ui| self.placed_table(ui, world));
            });
        }
        // The middle: the map. egui's points → the screen's pixels (the same unless Windows scales the window).
        let map = ctx.available_rect();
        let k = ctx.pixels_per_point();
        crate::view::set(Some(macroquad::math::Rect::new(map.min.x * k, map.min.y * k, map.width() * k, map.height() * k)));
        self.floating_bars(ctx, map, enabled, camera, grid);
        self.properties_window(ctx, world, enabled, orders);
        self.keys_window(ctx);
        keep_focus_for_text(ctx);
        // Esc and Enter take the keyboard away from a field before `show` even starts – but in that frame the key
        // was still meant for the field (not for the menu or the selection). So "typing" lasts one frame longer.
        let typing = text_has_focus(ctx);
        self.typing = typing || self.typed_before;
        self.typed_before = typing;
        // `wants_pointer_input`: also while a number is dragged and the mouse has left the panel.
        self.on_panel = ctx.wants_pointer_input() || ctx.is_pointer_over_area();
    }

    /// File, Edit, View, Map, Test, Help – and on the right the file's name, with ● while there are unsaved changes.
    /// `ui.close_menu()`: a click on an entry closes the menu, like everywhere.
    fn menu_bar(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>, camera: &mut Camera, grid: &mut bool) {
        let edit = self.mode == Mode::Edit;
        let (back, forward) = self.steps;
        // `menu::bar`: a row of menus that open with a click.
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                self.menu_entry(ui, "New map...", "", EditorRequest::NewMap, true);
                self.menu_entry(ui, "Save", "Ctrl+S", EditorRequest::Save, true);
                ui.separator();
                self.menu_entry(ui, "Close", "", EditorRequest::Close, true);
            });
            ui.menu_button("Edit", |ui| {
                self.menu_entry(ui, &format!("Undo ({back})"), "Ctrl+Z", EditorRequest::Undo, edit && back > 0);
                self.menu_entry(ui, &format!("Redo ({forward})"), "Ctrl+Y", EditorRequest::Redo, edit && forward > 0);
                ui.separator();
                let some = edit && self.pieces > 0;
                // In the Terrain module also the ground of the selected area (`picking.rs`).
                let ground = edit && self.module == Module::Terrain;
                let pastes = edit && (self.clip.is_some() || (ground && self.terrain_clip.is_some()));
                for (action, text, key, on) in [
                    (ClipAction::Cut, "Cut", "Ctrl+X", some),
                    (ClipAction::Copy, "Copy", "Ctrl+C", some || (ground && self.area.is_some())),
                    (ClipAction::Paste, "Paste", "Ctrl+V", pastes),
                    (ClipAction::Delete, "Delete", "Del", some),
                ] {
                    if ui.add_enabled(on, egui::Button::new(text).shortcut_text(key)).clicked() {
                        self.clip_request = Some(action);
                        ui.close_menu();
                    }
                }
                ui.separator();
                let clear = ui.add_enabled(edit, egui::Button::new("Clear map"));
                if clear.on_hover_text("Everything off the map, all of it grass - spawners, events and the round stay. Undo brings it back").clicked() {
                    clear_map(world, orders);
                    ui.close_menu();
                }
            });
            ui.menu_button("View", |ui| {
                for (sight, text) in [(Sight::All, "Everything (no fog)"), (Sight::Player, "What the player sees"), (Sight::Zombies, "What the zombies see")] {
                    if ui.radio_value(&mut self.sight, sight, text).clicked() {
                        ui.close_menu();
                    }
                }
                ui.separator();
                // A menu in the menu: the same switches as "Show" in the Terrain module.
                ui.menu_button("Show", |ui| self.layer_boxes(ui, grid));
                ui.label(RichText::new("Zombies").weak());
                for (look, text) in [(ZombieLook::Auto, "As tiles when zoomed out"), (ZombieLook::Tiles, "Always as tiles"), (ZombieLook::Figures, "Always as figures")] {
                    if ui.radio_value(&mut self.zombie_look, look, text).clicked() {
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.add(egui::Button::new("Zoom in").shortcut_text("Wheel")).clicked() {
                    camera.zoom_by(1.15);
                }
                if ui.button("Zoom out").clicked() {
                    camera.zoom_by(1.0 / 1.15);
                }
                if ui.button("Zoom 100 %").clicked() {
                    camera.zoom = START_ZOOM;
                    ui.close_menu();
                }
                self.menu_entry(ui, "Camera to the base", "", EditorRequest::Locate(world.base), true);
            });
            ui.menu_button("Map", |ui| {
                for (module, text) in [(Module::Map, "Name, info and the round"), (Module::Events, "Events"), (Module::Triggers, "Triggers")] {
                    if ui.button(text).clicked() {
                        self.module = module;
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button("Drag the border").on_hover_text("The playing field - grab its edges on the map").clicked() {
                    self.module = Module::Regions;
                    self.take(Tool::PlayArea);
                    ui.close_menu();
                }
            });
            ui.menu_button("Test", |ui| {
                let text = if edit { "Test the map" } else { "Back to editing" };
                self.menu_entry(ui, text, "F10", EditorRequest::Mode(self.other_mode()), true);
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Keys").clicked() {
                    self.keys_open = true;
                    ui.close_menu();
                }
            });
            // `right_to_left`: from the right edge leftwards – the name at the far right.
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                // "•": egui's own font has no "●".
                let dot = if self.unsaved { " •" } else { "" };
                ui.label(RichText::new(format!("{}{dot} - Last Bastion Editor", self.file_name)).weak())
                    .on_hover_text(if self.unsaved { "Changes not saved yet" } else { "Everything saved" });
            });
        });
    }

    /// An entry of a menu that asks `game.rs` for something – with its key on the right, greyed out if `on` is false.
    fn menu_entry(&mut self, ui: &mut Ui, text: &str, key: &str, request: EditorRequest, on: bool) {
        if ui.add_enabled(on, egui::Button::new(text).shortcut_text(key)).clicked() {
            self.request = Some(request);
            ui.close_menu();
        }
    }

    /// The modules, and on the right undo, redo, save and the Test button.
    fn module_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            // `&…[..]`: both lists as slices – they aren't the same length, so they aren't the same type as arrays.
            for (k, group) in [&Module::ON_MAP[..], &Module::PAGES[..]].into_iter().enumerate() {
                if k > 0 {
                    ui.separator();
                }
                for &module in group {
                    let button = egui::Button::new(RichText::new(module.name()).size(16.0)).selected(self.module == module).min_size(egui::vec2(0.0, 32.0));
                    if ui.add(button).on_hover_text(module.hint()).clicked() {
                        self.module = module;
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                let edit = self.mode == Mode::Edit;
                let (text, fill) = if edit { ("▶ Test (F10)", TEST_FILL) } else { ("Back to Edit (F10)", TESTING_FILL) };
                let hint = if edit { "Play the map as it is now - back in Edit it's as before" } else { "Stop the test - the map as it was before it" };
                let test = egui::Button::new(RichText::new(text).strong().size(16.0).color(Color32::WHITE)).fill(fill).min_size(egui::vec2(0.0, 32.0));
                if ui.add(test).on_hover_text(hint).clicked() {
                    self.request = Some(EditorRequest::Mode(self.other_mode()));
                }
                ui.separator();
                // Right to left: Save comes first, so it stands rightmost.
                if ui.add(egui::Button::new("Save").min_size(egui::vec2(0.0, 28.0))).on_hover_text("Ctrl+S").clicked() {
                    self.request = Some(EditorRequest::Save);
                }
                let (back, forward) = self.steps;
                if ui.add_enabled(edit && forward > 0, egui::Button::new(format!("Redo ({forward})")).min_size(egui::vec2(0.0, 28.0))).on_hover_text("Ctrl+Y").clicked() {
                    self.request = Some(EditorRequest::Redo);
                }
                if ui.add_enabled(edit && back > 0, egui::Button::new(format!("Undo ({back})")).min_size(egui::vec2(0.0, 28.0))).on_hover_text("Ctrl+Z").clicked() {
                    self.request = Some(EditorRequest::Undo);
                }
            });
        });
    }

    /// The line at the very bottom: the tile under the mouse and its ground, the selection, what's in hand – on the
    /// right undo and the clock.
    fn status_line(&self, ui: &mut Ui, world: &World) {
        ui.horizontal(|ui| {
            if self.testing() {
                ui.label(RichText::new("TEST").strong().color(Color32::from_rgb(242, 170, 90)));
                ui.separator();
            }
            match self.mouse_tile.filter(|&(x, y)| !self.on_panel && x >= 0 && y >= 0 && x < world.map.size as i32 && y < world.map.size as i32) {
                Some((x, y)) => {
                    ui.label(format!("Tile {x}, {y}"));
                    ui.separator();
                    ui.label(world.map.terrain(x, y).name());
                }
                None => {
                    ui.label(RichText::new("Tile -").weak());
                }
            }
            ui.separator();
            let (region, count) = self.region_status(world);
            if let Some(name) = region.filter(|_| !self.on_panel && self.shows_regions()) {
                ui.separator();
                ui.label(format!("Region: {name}"));
            }
            if self.module == Module::Regions {
                ui.separator();
                ui.label(count);
            }
            if self.module == Module::Triggers {
                ui.separator();
                ui.label(self.trigger_status(world));
            }
            ui.separator();
            ui.label(format!("Selected: {}", self.selection_text(world)));
            ui.separator();
            ui.label(format!("In hand: {}", self.hand_text()));
            if let Some(clip) = &self.clip {
                ui.separator();
                ui.label(format!("Clipboard: {}", clip.describe())).on_hover_text("Ctrl+V puts it down at the mouse");
            }
            if let Some(clip) = &self.terrain_clip {
                ui.separator();
                ui.label(format!("Ground copied: {} x {}", clip.size.0, clip.size.1)).on_hover_text("Terrain, \"Paste\": puts it down at the mouse");
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                let (day, hour) = world.clock();
                ui.label(format!("Day {day}, {hour:02}:00"));
                ui.separator();
                let (back, forward) = self.steps;
                ui.label(format!("Undo {back} - redo {forward}"));
                if let Some(status) = &self.status {
                    ui.separator();
                    ui.label(RichText::new(status).weak());
                }
            });
        });
    }

    /// "Tent", "3 units, 2 buildings", "spawner North" or "nothing".
    fn selection_text(&self, world: &World) -> String {
        if let Some(s) = self.spawner.and_then(|id| world.find_spawner(id)) {
            return format!("spawner {}", world.spawners[s].name);
        }
        if let Some(text) = &self.picked_text {
            return text.clone();
        }
        let count = |wanted: fn(&Piece) -> bool| self.selection.iter().filter(|p| wanted(p)).count();
        match (count(|p| matches!(p, Piece::Unit(_))), count(|p| matches!(p, Piece::Building(_)))) {
            (0, 0) => "nothing".to_owned(),
            (1, 0) | (0, 1) => self.things(world).first().map_or("1".to_owned(), |&thing| objects::describe(world, thing).name.to_owned()),
            (units, 0) => format!("{units} units"),
            (0, buildings) => format!("{buildings} buildings"),
            (units, buildings) => format!("{units} units, {buildings} buildings"),
        }
    }

    /// The tool or the building on the mouse.
    fn hand_text(&self) -> String {
        match (self.tool, self.placing) {
            (Some(tool), _) => tool.name().to_owned(),
            (None, Some(kind)) => kind.stats().name.clone(),
            (None, None) => "-".to_owned(),
        }
    }

    /// The bars that float over the map: whose eyes (top left), grid and zoom (top right), and what the mouse does
    /// (bottom left, above TAB's bar). `map`: the map's room in egui's points. An `Area` is a piece of egui that lies
    /// anywhere, not at an edge like a panel; `constrain_to` + `anchor`: in that corner of the map, this far in.
    fn floating_bars(&mut self, ctx: &egui::Context, map: egui::Rect, enabled: bool, camera: &mut Camera, grid: &mut bool) {
        let float = |ui: &mut Ui, add: &mut dyn FnMut(&mut Ui)| {
            Frame::popup(ui.style()).inner_margin(5.0).show(ui, |ui| {
                ui.add_enabled_ui(enabled, |ui| ui.horizontal(|ui| add(ui)));
            });
        };
        egui::Area::new(egui::Id::new("sight bar")).constrain_to(map).anchor(Align2::LEFT_TOP, [FLOAT_PAD, FLOAT_PAD]).show(ctx, |ui| {
            float(ui, &mut |ui| {
                ui.label(RichText::new("Sight").weak());
                for (sight, text, hint) in [
                    (Sight::All, "All", "Everything, no fog"),
                    (Sight::Player, "Player", "The fog of war like in the game"),
                    (Sight::Zombies, "Zombies", "Lit where a zombie looks"),
                ] {
                    ui.selectable_value(&mut self.sight, sight, text).on_hover_text(hint);
                }
            });
        });
        egui::Area::new(egui::Id::new("view bar")).constrain_to(map).anchor(Align2::RIGHT_TOP, [-FLOAT_PAD, FLOAT_PAD]).show(ctx, |ui| {
            float(ui, &mut |ui| {
                ui.toggle_value(grid, "Grid").on_hover_text("TAB's grid view: the tiles, and where nothing can be built");
                // Lit while the zombies are tiles – also when zooming out did it. A click: always tiles, or back to
                // "by itself".
                let tiles = self.zombie_tiles(camera.zoom);
                let hint = "The zombies as tiles, one colour per breed (colours: Zombies module) - by itself when zoomed out";
                if ui.selectable_label(tiles, "Zombie tiles").on_hover_text(hint).clicked() {
                    self.zombie_look = if self.zombie_look == ZombieLook::Tiles { ZombieLook::Auto } else { ZombieLook::Tiles };
                }
                ui.separator();
                if ui.small_button("-").on_hover_text("Zoom out (wheel)").clicked() {
                    camera.zoom_by(1.0 / 1.15);
                }
                // No number: the wheel's steps (× 1.15) gave crooked ones like 176 %.
                if ui.small_button("Reset").on_hover_text("Back to the start zoom").clicked() {
                    camera.zoom = START_ZOOM;
                }
                if ui.small_button("+").on_hover_text("Zoom in (wheel)").clicked() {
                    camera.zoom_by(1.15);
                }
            });
        });
        // Above the top of TAB's bar – `Layout` knows it now that `view.rs` has the map's room.
        let bar_top = (Layout::new().panel.y / ctx.pixels_per_point()).clamp(map.min.y, map.max.y);
        let above_bar = egui::Rect::from_min_max(map.min, egui::pos2(map.max.x, bar_top));
        // `interactable(false)`: only a text – a click on it still goes to the map.
        egui::Area::new(egui::Id::new("hint bar")).constrain_to(above_bar).anchor(Align2::LEFT_BOTTOM, [FLOAT_PAD, -FLOAT_PAD]).interactable(false).show(ctx, |ui| {
            Frame::popup(ui.style()).inner_margin(5.0).show(ui, |ui| {
                // `extend`: on one line, never wrapped.
                ui.add(egui::Label::new(self.hint()).extend());
            });
        });
    }

    /// What the mouse does on the map right now.
    fn hint(&self) -> String {
        let away = "right click: put away";
        match (self.tool, self.placing) {
            (Some(Tool::PlayArea), _) => format!("Border - drag an edge, inside: move it - {away}"),
            (Some(Tool::RailPlanner), _) => "Rail planner - click an end, then click or drag: build - right click: stop".to_owned(),
            (Some(Tool::Area), _) => format!("Select area - drag a rectangle - {away}"),
            (Some(Tool::PasteArea), _) => format!("Paste - click: the copied ground - {away}"),
            (Some(Tool::ZoneRect | Tool::ZoneCircle | Tool::ZonePaint), _) => format!("Drag: new region - Ctrl: add, Alt: cut - {away}"),
            (Some(Tool::Spot), _) => format!("Click: a new point - {away}"),
            (Some(tool), _) if tool.paints() => {
                let how = match self.shape_of(tool) {
                    PaintShape::Brush => "hold and drag",
                    PaintShape::Line => "press, drag, let go: a line",
                    PaintShape::Rect => "press, drag, let go: a rectangle",
                    PaintShape::Fill => "click: fill the patch",
                };
                format!("{} - {how} - {away}", tool.name())
            }
            (Some(tool), _) if tool.once() => format!("{} - click, Shift: keep it - {away}", tool.name()),
            (Some(tool), _) => format!("{} - click - {away}", tool.name()),
            (None, Some(kind)) => format!("{} - click, drag: a line, R turns - {away}", kind.stats().name),
            (None, None) if self.testing() => "Test - the round runs. F10: back to Edit".to_owned(),
            (None, None) if self.module == Module::Regions => "Click: select - drag: move - corners: resize - Del: remove".to_owned(),
            (None, None) => "Click or box: select - Ctrl+C / X / V - Del: remove".to_owned(),
        }
    }

    /// "Help → Keys": the editor's keys in a window of their own.
    fn keys_window(&mut self, ctx: &egui::Context) {
        let mut open = self.keys_open;
        egui::Window::new("Keys").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            egui::Grid::new("keys").num_columns(2).striped(true).show(ui, |ui| {
                for (key, what) in [
                    ("F10", "Edit / Test"),
                    ("Ctrl+S", "Save"),
                    ("Ctrl+Z", "Undo"),
                    ("Ctrl+Y, Ctrl+Shift+Z", "Redo"),
                    ("Esc", "Put the tool away - then the selection - then the menu"),
                    ("Right click", "Put the tool away - on a spawner: its window"),
                    ("Shift+click with a thing in hand", "Put it down and keep it in hand"),
                    ("Drag a box", "Select everything in it (in Edit)"),
                    ("Regions: Ctrl / Alt + drag", "Add the shape to the selected region / cut it out"),
                    ("Shift+click, Shift+box", "Add to the selection"),
                    ("Ctrl+C", "Copy the selected, with all it has"),
                    ("Ctrl+X", "Cut: copy it and take it off the map"),
                    ("Ctrl+V", "Paste at the mouse (again and again)"),
                    ("Ctrl+C / Ctrl+V in Terrain", "With an area selected: copy its ground - take the copy in hand"),
                    ("Del", "Remove the selected"),
                    ("R", "Turn the building on the mouse"),
                    ("Rail planner", "Click an open end, then click: build the ghost - hold and drag: build on to the mouse - right click: stop"),
                    ("Drag with a building", "A line of them"),
                    ("Wheel", "Zoom"),
                    ("Middle mouse, arrows", "Move the map"),
                    ("Space", "Pause (in a test)"),
                ] {
                    ui.label(RichText::new(key).strong());
                    ui.label(what);
                    ui.end_row();
                }
            });
        });
        self.keys_open = open;
    }

    /// The panel on the right: what's selected – a spawner, a placed thing –, in "Events" first the waves to send now,
    /// in "Terrain" what lies under the mouse.
    fn properties_panel(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>, camera: &Camera, grid: &mut bool) {
        if self.module == Module::Terrain {
            self.minimap_section(ui, world, camera);
            self.layers_section(ui, grid);
            self.under_mouse_section(ui, world);
        }
        if self.module == Module::Regions {
            // Its own selection – the placed things have their panel in the other modules.
            self.region_section(ui, world, orders);
            self.spawner_section(ui, world, orders);
            return;
        }
        self.spawner_section(ui, world, orders);
        let things = self.things_section(ui, world, orders);
        if self.spawner.and_then(|id| world.find_spawner(id)).is_none() && !things {
            section(ui, "Selection", "", |ui| {
                ui.label(RichText::new("Nothing selected. Click a building, unit or spawner on the map, drag a box - or click a line of Objects, \"Placed\".").weak());
            });
        }
    }

    /// TAB's "Info": author, description, a picture for the list of custom levels (a PNG next to the map file) – and
    /// TAB's "random generation": the game makes the map anew every start. Not part of the world – no undo, saved with
    /// the file. ("Clear map" moved to the Edit menu.)
    fn info_section(&mut self, ui: &mut Ui) {
        section_from(ui, "Info", Some(Origin::Tab), "shown with the custom levels", |ui| {
            egui::Grid::new("info").num_columns(2).show(ui, |ui| {
                ui.label("Author");
                ui.add(TextEdit::singleline(&mut self.info.author).char_limit(MAX_NAME).desired_width(f32::INFINITY));
                ui.end_row();
                ui.label("Picture");
                ui.add(TextEdit::singleline(&mut self.info.picture).char_limit(64).hint_text("file.png next to the map").desired_width(f32::INFINITY));
                ui.end_row();
            });
            ui.label("Description");
            ui.add(TextEdit::multiline(&mut self.info.description).char_limit(600).desired_rows(3).desired_width(f32::INFINITY));
            ui.checkbox(&mut self.info.regenerate, "New map every start")
                .on_hover_text("Like TAB's random generation: the game makes the map anew from its setup at every start - the round and the events stay. Only for maps made by a generator.");
        });
    }

    /// "Playing field": the world border – where units may go, the camera and the minimap. Drag its edges on the map
    /// (`Tool::PlayArea`, `ui.rs`) or type its size: across and down in steps of the screen axes (a step across is
    /// half a tile wide), around the same middle. Nothing on the map moves.
    fn play_section(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        section_from(ui, "Playing field", Some(Origin::Sc2), "the world border - units stay inside, zombies come from outside", |ui| {
            let (min, size) = world.map.play_steps();
            ui.horizontal(|ui| {
                self.drag_border_button(ui);
                // As many tiles as a square of this edge: one step across and down is half a tile.
                let tiles = ((size.0 * size.1) as f32 / 2.0).sqrt().round();
                ui.label(RichText::new(format!("like {tiles} x {tiles} tiles")).weak());
            });
            ui.horizontal(|ui| {
                ui.label("across");
                // `settled`: one order when the number is let go – every new border recomputes the zombies' ways.
                let w = settled(ui, size.0, |v| DragValue::new(v).range(crate::map::MIN_PLAY..=world.map.size as i32));
                ui.label("down");
                let h = settled(ui, size.1, |v| DragValue::new(v).range(crate::map::MIN_PLAY..=world.map.size as i32));
                if w.is_some() || h.is_some() {
                    let new = (w.unwrap_or(size.0), h.unwrap_or(size.1));
                    // Around the same middle.
                    let min = (min.0 + (size.0 - new.0) / 2, min.1 + (size.1 - new.1) / 2);
                    orders.push(Order::SetPlayArea { min, size: new });
                }
                ui.label("steps");
            });
            if ui.button("Like a new map").on_hover_text("In the middle, the ring of a new map around it").clicked() {
                let region = crate::map::Map::play_area_of(world.map.size, world.map.border.max(BORDER));
                let (min, extent) = (region.min.round(), (region.max - region.min).round());
                orders.push(Order::SetPlayArea { min: (min.x as i32, min.y as i32), size: (extent.x as i32, extent.y as i32) });
            }
        });
    }

    /// "Grid": the whole map the playing field lies in (SC2's map size) – more room around it, or less. It only changes
    /// with "Apply": the map is loaded anew then (`World::resize`), and smaller, what lies outside is gone (undo brings
    /// it back). Not during a test.
    fn grid_section(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        section_from(ui, "Grid", Some(Origin::Sc2), "the whole map - grows or shrinks on every side", |ui| {
            let now = world.map.size;
            let mut size = self.grid_size.unwrap_or(now);
            ui.horizontal(|ui| {
                // `speed(2)` and the rounding below: always even, the map has a middle tile pair.
                ui.add(DragValue::new(&mut size).range(MAP_SIZES).speed(2).suffix(" tiles"));
                size = size / 2 * 2;
                let apply = egui::Button::new("Apply");
                if ui.add_enabled(size != now && self.mode == Mode::Edit, apply).clicked() {
                    orders.push(Order::ResizeMap { size });
                    size = now;
                }
            });
            // `then_some`: `Some(size)` if the condition holds, otherwise `None`.
            self.grid_size = (size != now).then_some(size);
            ui.label(RichText::new("Everything stays where it is, new ground is grass. Then drag the border.").small().weak());
        });
    }

    /// Zombies by the tile (brush), by the crowd (drop), one giant; spawners; the houses of a village of doom.
    fn zombies_module(&mut self, ui: &mut Ui, world: &World) {
        section_from(ui, "Brush", Some(Origin::Tab), "on every tile of the brush - hold and drag", |ui| {
            self.tool_button(ui, Tool::ZombieBrush);
            ui.label(format!("On every tile, at most {MOST_PER_TILE} of each (like TAB):"));
            // `&mut (breed, n)`: each pair of the list lent to change `n` – the breed stays. In front of each its
            // colour as a tile – the legend of the zombie tiles.
            for (breed, n) in &mut self.per_tile {
                ui.horizontal(|ui| {
                    swatch(ui, *breed);
                    ui.add(Slider::new(n, 0..=MOST_PER_TILE).text(breed.name()));
                });
            }
            ui.horizontal(|ui| {
                swatch(ui, Breed::Giant);
                ui.label("Giant");
            });
            // Below, not beside the square: a label in a row never wraps and would make the panel wider.
            ui.label(RichText::new("Zoomed out, the zombies are tiles in these colours: the breed most on a tile, the more the more solid, a light spot where one storms.").small().weak());
        });
        section_from(ui, "Drop and giant", Some(Origin::New), "a crowd at the click - or one giant", |ui| {
            ui.horizontal(|ui| {
                self.tool_button(ui, Tool::ZombieDrop);
                self.tool_button(ui, Tool::Giant);
            });
            ui.horizontal(|ui| {
                ui.add(DragValue::new(&mut self.drop).range(1..=10_000).suffix(" zombies"));
                breed_list_box(ui, "drop breed", &mut self.breed, &small_breeds());
            });
        });
        section(ui, "They", "brush, drop and giant", |ui| {
            ui.radio_value(&mut self.wander, true, "sleep and wander");
            ui.radio_value(&mut self.wander, false, "storm the CC");
        });
        self.brush_size(ui, false);
        section_from(ui, "Spawners", Some(Origin::New), "click to place - click one to select it, drag its edge", |ui| self.tool_button(ui, Tool::Spawner));
        section_from(ui, "Villages of doom", Some(Origin::Tab), "TAB's three houses - zombies inside as the generator puts them", |ui| {
            let population = crate::data::get().difficulty(&world.difficulty).population;
            for nest in Nest::ALL {
                ui.horizontal(|ui| {
                    self.tool_button(ui, Tool::Doom(nest));
                    let side = nest.side();
                    let inside = World::doom_zombies((side, side), population);
                    ui.label(RichText::new(format!("{inside} zombies inside")).weak());
                });
            }
        });
    }

    /// The selected spawner in the panel – unless its properties window is open (right click): the same fields twice
    /// would fight over the name being typed (`name_edit`).
    fn spawner_section(&mut self, ui: &mut Ui, world: &World, orders: &mut Vec<Order>) {
        let Some(s) = self.spawner.and_then(|id| world.find_spawner(id)) else {
            return;
        };
        section_from(ui, "Selected spawner", Some(Origin::New), "drag its edge - right click: its window", |ui| {
            if self.properties {
                ui.label(RichText::new("Its properties are open in their own window.").weak());
            } else {
                self.spawner_fields(ui, &world.spawners[s], orders);
            }
        });
    }

    /// The selected spawner's properties in a window of their own, next to where it was right-clicked – like TAB's
    /// entity properties. Its X, Esc or selecting something else closes it.
    fn properties_window(&mut self, ctx: &egui::Context, world: &World, enabled: bool, orders: &mut Vec<Order>) {
        if !self.properties {
            return;
        }
        let Some(s) = self.spawner.and_then(|id| world.find_spawner(id)) else {
            self.properties = false;
            return;
        };
        let mut open = true;
        // `id`: egui knows the window by it (and where it was dragged to) – not by its title, which shows the name.
        let mut window = egui::Window::new(format!("Spawner - {}", world.spawners[s].name))
            .id(egui::Id::new("spawner properties"))
            .collapsible(false)
            .resizable(false)
            .min_width(250.0)
            .open(&mut open);
        // `take`: only the first time after the right click – after that it stays where it was dragged.
        if let Some(at) = self.properties_at.take() {
            window = window.current_pos([at.x, at.y]);
        }
        window.show(ctx, |ui| {
            if !enabled {
                ui.disable();
            }
            self.spawner_fields(ui, &world.spawners[s], orders);
        });
        if !open {
            self.properties = false;
        }
    }

    /// What can be set on a spawner: its name, straight or slanted, how it sends zombies by itself – and remove it.
    fn spawner_fields(&mut self, ui: &mut Ui, spawner: &Spawner, orders: &mut Vec<Order>) {
        let (id, name, mode) = (spawner.id, &spawner.name, spawner.mode);
        ui.horizontal(|ui| {
            ui.label("Name");
            if let Some(name) = name_field(ui, &mut self.name_edit, name) {
                orders.push(Order::RenameSpawner { spawner: id, name });
            }
        });
        ui.horizontal(|ui| {
            ui.label("Lies");
            for (axes, text, hint) in [
                (Axes::Screen, "straight", "A rectangle straight on screen, like the world border"),
                (Axes::Tiles, "slanted", "Slanted like the tiles - turned by 45 degrees, about as big"),
            ] {
                if ui.selectable_label(spawner.axes == axes, text).on_hover_text(hint).clicked() && spawner.axes != axes {
                    orders.push(Order::SetSpawnerAxes { spawner: id, axes });
                }
            }
            let (w, h) = spawner.size;
            let unit = if spawner.axes == Axes::Tiles { "tiles" } else { "steps" };
            ui.label(RichText::new(format!("{w} x {h} {unit}")).weak());
        });
        let spawner = id;
        {
            ui.label("Sends zombies by itself:");
            let mut new = mode;
            ui.horizontal(|ui| {
                let periodic = matches!(mode, SpawnMode::Periodic { .. });
                if ui.selectable_label(periodic, "Periodic").clicked() && !periodic {
                    new = SpawnMode::PERIODIC;
                }
                if ui.selectable_label(mode == SpawnMode::Off, "Off").clicked() {
                    new = SpawnMode::Off;
                }
            });
            // `if let` with a pattern that also takes the values out – `mut`: as copies that may change.
            if let SpawnMode::Periodic { mut every, mut count } = mode {
                ui.horizontal(|ui| {
                    ui.label("every");
                    ui.add(DragValue::new(&mut every).range(1..=3600).suffix(" s"));
                    ui.add(DragValue::new(&mut count).range(1..=10_000).suffix(" zombies"));
                });
                new = SpawnMode::Periodic { every, count };
            }
            if new != mode {
                orders.push(Order::SetSpawnMode { spawner, mode: new });
            }
            if ui.button("Remove spawner (Del)").clicked() {
                self.select_spawner(None);
                orders.push(Order::RemoveSpawner { spawner });
            }
        }
    }

    /// The big rock the tool would put down with its middle on `tile`. Mixed pictures: picked by the tile, so the
    /// preview and the rock put down are the same.
    pub fn big_rock(&self, size: i32, tile: (i32, i32)) -> BigRock {
        let corner = (tile.0 - size / 2, tile.1 - size / 2);
        let mixed = (corner.0 * 7 + corner.1 * 13).rem_euclid(3) as u8;
        BigRock { tile: corner, size, variant: self.rock_variant.unwrap_or(mixed) }
    }

    /// A tool's button: lit while it's in hand – clicked: take it, clicked again: put it away.
    fn tool_button(&mut self, ui: &mut Ui, tool: Tool) {
        if ui.selectable_label(self.holds(tool), tool.name()).clicked() {
            self.toggle_tool(tool);
        }
    }

    /// Is this tool in hand? A building isn't held by the editor: `ui.rs` has it on the mouse (`placing`).
    fn holds(&self, tool: Tool) -> bool {
        match tool {
            Tool::Building(kind) => self.placing == Some(kind),
            _ => self.tool == Some(tool),
        }
    }

    /// A click on a tool: take it – or put it away if it's in hand. A building goes to `ui.rs`, which hangs it on the
    /// mouse (`place`).
    fn toggle_tool(&mut self, tool: Tool) {
        let held = self.holds(tool);
        match tool {
            Tool::Building(kind) => {
                self.tool = None;
                self.place = Some(if held { None } else { Some(kind) });
            }
            _ if held => self.tool = None,
            _ => self.take(tool),
        }
    }

    /// Takes a tool in hand – whatever else is on the mouse comes off it: a building, a command waiting for its target.
    fn take(&mut self, tool: Tool) {
        self.tool = Some(tool);
        self.place = Some(None);
    }
}

/// Track snaps onto an open end of the railway this close to the mouse (tiles).
const RAIL_SNAP: f32 = 2.5;

/// Where an ornament of `kind` goes for the mouse at `point`: a piece of track with a connector facing the closest
/// open end of the track within `RAIL_SNAP` (TAB's editor joins pieces like that, `CRailWay.ConnectTo`) – else, like
/// everything else, its middle under the mouse. Its top left tile.
pub fn ornament_spot(world: &World, kind: OrnamentKind, point: Vec2) -> (i32, i32) {
    if let OrnamentKind::Rail(shape) = kind
        && let Some(tile) = snap_rail(&rail_pieces(&world.ornaments), shape, point, RAIL_SNAP)
    {
        return tile;
    }
    let (w, h) = kind.footprint();
    ((point.x - w as f32 / 2.0 + 0.5).floor() as i32, (point.y - h as f32 / 2.0 + 0.5).floor() as i32)
}

/// Everything off the map, all of it grass – spawners, events and the round stay (undo brings it back). The eraser,
/// the grass and "no ore" over every tile – three orders, so undo takes them back as one step.
fn clear_map(world: &World, orders: &mut Vec<Order>) {
    let n = world.map.size as i32;
    let all: Vec<(i32, i32)> = (0..n).flat_map(|y| (0..n).map(move |x| (x, y))).collect();
    orders.push(Order::RemoveAt { tiles: all.clone() });
    orders.push(Order::PaintOre { ore: None, tiles: all.clone() });
    orders.push(Order::PaintTerrain { terrain: Terrain::Grass, tiles: all });
}

/// Where the editor's lists of places come from: the spawners and the regions of the map.
#[derive(Clone, Copy)]
pub(crate) struct Places<'a> {
    pub spawners: &'a [Spawner],
    pub zones: &'a [Zone],
    pub spots: &'a [crate::sim::Spot],
    /// The variables a condition or code may use (`story_panel::code_field`).
    pub variables: &'a [crate::sim::Variable],
}

impl<'a> Places<'a> {
    pub(crate) fn of(world: &'a World) -> Places<'a> {
        Places { spawners: &world.spawners, zones: &world.zones, spots: &world.spots, variables: &world.story.variables }
    }
}

/// A section with a heading that folds shut with a click (egui remembers it by the heading) and a grey hint under it.
/// `add`: what's in it – a *closure*, like a `lambda` in Python, only that it may have many lines. `impl FnOnce(&mut
/// Ui)`: any closure that takes the section's `Ui` and is called once.
pub(crate) fn section(ui: &mut Ui, title: &str, hint: &str, add: impl FnOnce(&mut Ui)) {
    section_from(ui, title, None, hint, add);
}

/// A section like `section`, with a badge after its heading that says where it comes from (`Origin`). The heading is
/// a `LayoutJob`: a text put together from pieces in different looks – here the title, then the badge in small
/// letters on its own background.
pub(crate) fn section_from(ui: &mut Ui, title: &str, origin: Option<Origin>, hint: &str, add: impl FnOnce(&mut Ui)) {
    let mut heading = LayoutJob::default();
    let strong = TextFormat { font_id: TextStyle::Body.resolve(ui.style()), color: ui.visuals().strong_text_color(), ..Default::default() };
    heading.append(title, 0.0, strong);
    if let Some(origin) = origin {
        let (text, color, background) = origin.look();
        // The spaces around it widen the background a little; 8.0: the room before it.
        heading.append(&format!(" {text} "), 8.0, TextFormat { font_id: FontId::proportional(11.0), color, background, ..Default::default() });
    }
    // `id_salt`: egui remembers whether it's folded by the title alone, not by the badge.
    CollapsingHeader::new(heading).id_salt(title).default_open(true).show(ui, |ui| {
        if !hint.is_empty() {
            ui.label(RichText::new(hint).small().weak());
        }
        add(ui);
    });
}

/// A name field: shows `name`; while it has the keyboard the text so far is in `edit` (the map shows a spawner's new
/// name while it's typed). Hands out the new name when typing ends – with Enter or a click elsewhere, not with Esc,
/// and not if it's empty or unchanged.
fn name_field(ui: &mut Ui, edit: &mut Option<String>, name: &str) -> Option<String> {
    let mut text = edit.clone().unwrap_or_else(|| name.to_owned());
    let field = ui.add(TextEdit::singleline(&mut text).char_limit(MAX_NAME).desired_width(f32::INFINITY));
    if field.has_focus() {
        *edit = Some(text);
        return None;
    }
    *edit = None;
    let escaped = ui.input(|input| input.key_pressed(egui::Key::Escape));
    let text = text.trim();
    (field.lost_focus() && !escaped && !text.is_empty() && text != name).then(|| text.to_owned())
}

/// A number field whose new value only counts once it's settled: let go after dragging, Enter or a click elsewhere
/// after typing. Until then it shows its own copy, kept in egui's memory. For the size of the playing field: every
/// change of it moves the map, so dragging shouldn't send an order for every step.
///
/// `field` makes the field for a value: `impl FnOnce(&mut T) -> DragValue<'_>` – a closure that gets the value
/// lent (`&mut`) and gives back a field that borrows it (`'_`: as long as the value is lent).
///
/// `<T>`: a *generic* function – it works for any type `T` that fulfils what stands after `where`: a number egui can
/// drag (`Numeric`), and one egui's memory may keep (`Send + Sync`: safe across threads) – the compiler makes one
/// copy of the function for every type it's used with.
fn settled<T>(ui: &mut Ui, value: T, field: impl FnOnce(&mut T) -> DragValue<'_>) -> Option<T>
where
    T: egui::emath::Numeric + Send + Sync,
{
    // The id the field is going to get – `with`: one of its own, derived from it.
    let id = ui.next_auto_id().with("settled");
    let mut shown = ui.data(|data| data.get_temp::<T>(id)).unwrap_or(value);
    let response = ui.add(field(&mut shown).update_while_editing(false));
    if response.dragged() || response.has_focus() {
        ui.data_mut(|data| data.insert_temp(id, shown));
        return None;
    }
    ui.data_mut(|data| data.remove::<T>(id));
    (shown != value).then_some(shown)
}

/// A text field like `settled_name` – but an empty text counts too (it takes the text away), and it may have several
/// lines.
pub(crate) fn settled_text(ui: &mut Ui, value: &str, multiline: bool) -> Option<String> {
    let id = ui.next_auto_id().with("settled text");
    let mut text = ui.data(|data| data.get_temp::<String>(id)).unwrap_or_else(|| value.to_owned());
    let field = if multiline {
        ui.add(TextEdit::multiline(&mut text).char_limit(400).desired_rows(2).desired_width(f32::INFINITY))
    } else {
        ui.add(TextEdit::singleline(&mut text).char_limit(MAX_NAME * 2).desired_width(f32::INFINITY))
    };
    if field.has_focus() {
        ui.data_mut(|data| data.insert_temp(id, text));
        return None;
    }
    ui.data_mut(|data| data.remove::<String>(id));
    let text = text.trim();
    (text != value).then(|| text.to_owned())
}

/// A name field like `settled`: while it has the keyboard the text so far stays in egui's memory, the new name counts
/// when typing ends (Enter or a click elsewhere) – one order, not one per letter. Empty or unchanged: none.
fn settled_name(ui: &mut Ui, name: &str) -> Option<String> {
    let id = ui.next_auto_id().with("settled name");
    let mut text = ui.data(|data| data.get_temp::<String>(id)).unwrap_or_else(|| name.to_owned());
    let field = ui.add(TextEdit::singleline(&mut text).char_limit(MAX_NAME).desired_width(170.0));
    if field.has_focus() {
        ui.data_mut(|data| data.insert_temp(id, text));
        return None;
    }
    ui.data_mut(|data| data.remove::<String>(id));
    let text = text.trim();
    (!text.is_empty() && text != name).then(|| text.to_owned())
}

/// A drop-down list of the zombie breeds. `id`: egui tells the lists apart by it.
fn breed_box(ui: &mut Ui, id: impl std::hash::Hash, breed: &mut Breed) {
    breed_list_box(ui, id, breed, &Breed::ALL);
}

/// A small square in a breed's colour as a tile on the map (`draw::breed_tile_color`).
fn swatch(ui: &mut Ui, breed: Breed) {
    let c = breed_tile_color(breed);
    let color = Color32::from_rgb((c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8);
    // `allocate_exact_size`: room for it in the row; the painter then draws into that room.
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, color);
}

/// A drop-down list of these breeds.
fn breed_list_box(ui: &mut Ui, id: impl std::hash::Hash, breed: &mut Breed, breeds: &[Breed]) {
    ComboBox::from_id_salt(id).width(90.0).selected_text(breed.name()).show_ui(ui, |ui| {
        for &b in breeds {
            ui.selectable_value(breed, b, b.name());
        }
    });
}

/// The panel's fonts a bit bigger than egui's own (12.5 points) – near the 17 pixels of the rest of the game.
fn style(ctx: &egui::Context) {
    ctx.style_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Body, FontId::proportional(15.0)),
            (TextStyle::Button, FontId::proportional(15.0)),
            (TextStyle::Heading, FontId::proportional(19.0)),
            (TextStyle::Monospace, FontId::monospace(14.0)),
        ]
        .into();
    });
}

/// Tab hands egui's keyboard focus on to the next button – and then Space or Enter would press it. In this game Tab
/// and Space are keys of their own (`keys.rs`), so only a field you type into keeps the focus.
fn keep_focus_for_text(ctx: &egui::Context) {
    let button = ctx.memory(|memory| memory.focused()).filter(|&id| TextEdit::load_state(ctx, id).is_none());
    if let Some(id) = button {
        ctx.memory_mut(|memory| memory.surrender_focus(id));
    }
}

/// Is something being typed into a field (a name, or a number clicked to type it)? Only those keep a text cursor in
/// egui's memory (`TextEdit::load_state`) – a button that has the focus doesn't.
fn text_has_focus(ctx: &egui::Context) -> bool {
    ctx.memory(|memory| memory.focused()).is_some_and(|id| TextEdit::load_state(ctx, id).is_some())
}

/// TAB's "Info" of a map (and its random generation) as the editor shows it – out of the map file and back into it
/// (`game.rs`).
#[derive(Clone, Default, PartialEq)]
pub struct MapInfo {
    pub author: String,
    pub description: String,
    pub picture: String,
    pub regenerate: bool,
}

impl MapInfo {
    pub fn of(file: &crate::mapfile::MapFile) -> MapInfo {
        MapInfo { author: file.author.clone(), description: file.description.clone(), picture: file.picture.clone(), regenerate: file.regenerate }
    }
}
