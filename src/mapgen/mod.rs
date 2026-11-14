//! Map generation (DESIGN.md 9.1): TAB's generator, with its values on the setup screen (and in the editor's "Map"
//! tab). The same seed and the same values always give the same map – also
//! on the other PC in co-op (DESIGN.md 12.8). That's why there is no `sin`, `cos` or `powf` in here: they can
//! differ in the last bit between CPUs. Only `+ - * /` and `sqrt` (both exactly defined by IEEE 754).
//!
//! - **TAB** (`tab.rs`): They Are Billions' own generator, rebuilt from its code – it makes everything itself
//!   (terrain, stone, iron, gold, oil, villages, ruins with loot, the zombies). Its landscapes are the maps of
//!   `data\game_maps.json` – the only maps there are.
//! - **Blank**: only grass – the tests' plain map, never offered. It gets our old shared steps here in `generate`:
//!   the ring outside the world border cleared to open ground, the spawners in it, the free space around the base, a
//!   way from every open place that is still cut off (`tools::connect_pockets`), the ore patches (`ores.rs`), the
//!   villages of doom and the loot (`villages.rs`). Our own generators (Wild, Regions, Rings, Canyons) are gone – maps
//!   made with them load as TAB maps (their terrain is in the file).

mod layer;
mod ores;
mod start;
mod tab;
mod tools;
mod villages;

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use crate::data::Modifiers;
use crate::kinds::BuildingKind;
use crate::map::{Map, BORDER};
use crate::noise::Noise;
use crate::rng::Rng;
use crate::sim::{Breed, LootKind};
use tools::Tiles;
pub use layer::spiral;
pub use villages::HouseSpot;

/// Which generator makes the map.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Generator {
    /// They Are Billions' own – the only one you can pick. Setups and map files of our old generators read as this.
    #[serde(alias = "wild", alias = "regions", alias = "rings", alias = "canyons")]
    Tab,
    /// Only grass – the tests' plain map.
    Blank,
}

impl Generator {
    pub fn name(self) -> &'static str {
        match self {
            Generator::Tab => "TAB",
            Generator::Blank => "Blank",
        }
    }
}

/// How a value is shown next to its slider.
#[derive(Clone, Copy)]
pub enum Show {
    /// A plain number (whole if the step is whole).
    Number,
    /// 0.25 → "25 %".
    Percent,
    /// "12 tiles".
    Tiles,
}

/// One value on the setup screen: a slider from `min` to `max` in steps of `step`.
///
/// `'a` is a *lifetime*: the field borrows the number it changes (`&'a mut f32`) from the settings of a generator.
/// The `Field` must not live longer than those settings – the compiler checks that. That way the screen can change
/// the value directly, without copying it back afterwards.
pub struct Field<'a> {
    pub label: &'static str,
    pub value: &'a mut f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub show: Show,
    /// What the value does – shown when the mouse is on the slider.
    pub help: &'static str,
}

impl Field<'_> {
    /// The value as text: "25 %", "12 tiles", "Forest" …
    pub fn text(&self) -> String {
        let v = *self.value;
        match self.show {
            Show::Percent => format!("{:.0} %", v * 100.0),
            Show::Tiles => format!("{v:.0} tiles"),
            Show::Number if self.step >= 1.0 => format!("{v:.0}"),
            Show::Number => format!("{v:.2}"),
        }
    }

    /// Sets the value from a share of the slider (0 = left end, 1 = right end), snapped to the step.
    pub fn set_fraction(&mut self, t: f32) {
        *self.value = self.snap(self.min + t.clamp(0.0, 1.0) * (self.max - self.min));
    }

    /// Where the value sits on the slider (0 … 1).
    pub fn fraction(&self) -> f32 {
        ((*self.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }

    fn snap(&self, v: f32) -> f32 {
        let snapped = self.min + ((v - self.min) / self.step).round() * self.step;
        snapped.clamp(self.min, self.max)
    }
}

/// Values every generator has.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Common {
    /// How big the playing field inside the world border is: as many tiles as a square with this edge (TAB: 256).
    /// On screen it's a rectangle √2 times as wide as high (`map.rs`).
    pub size: f32,
    /// Radius around the base where nothing grows (tiles).
    pub base_clear: f32,
    pub spawners_per_side: f32,
    /// Villages of doom (a few houses full of sleeping zombies) and piles of loot (guarded by hidden zombies), per
    /// 10,000 tiles (`villages.rs`). Setups saved before 2026-10-20 have none – they get the defaults.
    pub villages: f32,
    /// Houses in a village near the base and at the edge – many small villages or two huge ones is up to the map. The
    /// difficulty makes villages and houses more or fewer (`World::generate`), how full the houses are too.
    pub houses_near: f32,
    pub houses_edge: f32,
    pub loot_piles: f32,
    /// Giants on a map of TAB's size (256) – more on a bigger one, fewer on a smaller one, far out (`place_giants`).
    /// The difficulty makes them more or fewer (`giants_percent`). The generator doesn't place them – the world does,
    /// asleep like the roamers.
    pub giants: f32,
    /// Ore patches per 10,000 tiles (besides the three in the start area), their size and richness (`ores.rs`).
    pub ore_patches: f32,
    pub ore_size: f32,
    pub ore_richness: f32,
    /// Groups of oil nodes per 10,000 tiles (besides the one node in the start area). Setups saved before
    /// 2026-10-18 have none – they get the default.
    pub oil_groups: f32,
}

impl Default for Common {
    fn default() -> Common {
        Common {
            size: 256.0,
            base_clear: 14.0,
            spawners_per_side: 3.0,
            villages: 1.5,
            houses_near: 3.0,
            houses_edge: 6.0,
            loot_piles: 6.0,
            giants: 4.0,
            ore_patches: 5.0,
            ore_size: 5.0,
            ore_richness: 1.0,
            oil_groups: 1.0,
        }
    }
}

impl Common {
    fn fields(&mut self) -> Vec<Field<'_>> {
        vec![
            Field {
                label: "Map size",
                value: &mut self.size,
                min: 96.0,
                max: 256.0,
                step: 32.0,
                show: Show::Tiles,
                help: "Playing field as big as a square of this edge. TAB's survival maps are 256. Plus the spawn ring around it.",
            },
            Field {
                label: "Free around base",
                value: &mut self.base_clear,
                min: 6.0,
                max: 40.0,
                step: 1.0,
                show: Show::Tiles,
                help: "Nothing grows this close to the command center - room for the first buildings.",
            },
            Field {
                label: "Spawners per side",
                value: &mut self.spawners_per_side,
                min: 0.0,
                max: 6.0,
                step: 1.0,
                show: Show::Number,
                help: "Where the waves come from: this many on each side, side by side along the whole side, in the ring outside the border. A wave from one side comes out of all of them.",
            },
            Field {
                label: "Villages of doom",
                value: &mut self.villages,
                min: 0.0,
                max: 8.0,
                step: 0.25,
                show: Show::Number,
                help: "Villages per 10,000 tiles, away from the base: ruined houses full of sleeping zombies - how full, the difficulty says (and it makes the villages more or fewer). Come close or shoot near them and they pour out. A destroyed house drops loot. 1.5 on 256 tiles: about 10 villages, 0.3: two.",
            },
            Field {
                label: "Houses near base",
                value: &mut self.houses_near,
                min: 1.0,
                max: 30.0,
                step: 1.0,
                show: Show::Number,
                help: "Houses in a village close to the base - further out it grows to 'Houses at edge'. A big village spreads wider and keeps further from the others. The difficulty makes them more or fewer.",
            },
            Field {
                label: "Houses at edge",
                value: &mut self.houses_edge,
                min: 1.0,
                max: 30.0,
                step: 1.0,
                show: Show::Number,
                help: "Houses in a village at the edge of the map. Many small villages: 2 and 3. Two huge ones: 'Villages of doom' 0.3, 15 and 25.",
            },
            Field {
                label: "Loot",
                value: &mut self.loot_piles,
                min: 0.0,
                max: 15.0,
                step: 1.0,
                show: Show::Number,
                help: "Piles of gold, wood, stone, iron ore or coal per 10,000 tiles. Walk onto one to pick it up - but zombies lie hidden around it and come out when you get close. 6 on 256 tiles: about 40.",
            },
            Field {
                label: "Giants",
                value: &mut self.giants,
                min: 0.0,
                max: 16.0,
                step: 1.0,
                show: Show::Number,
                help: "Giants on a map of 256 tiles - more on a bigger one, fewer on a smaller one (at least one, if any). They sleep far out and stroll like the others; the final wave calls them. The difficulty makes them more or fewer. The minimap always shows where they are.",
            },
            Field {
                label: "Ore patches",
                value: &mut self.ore_patches,
                min: 0.0,
                max: 15.0,
                step: 1.0,
                show: Show::Number,
                help: "Patches of iron ore, coal and stone per 10,000 tiles - plus one of each right next to the base. 5 on 256 tiles: about 33.",
            },
            Field {
                label: "Ore patch size",
                value: &mut self.ore_size,
                min: 2.0,
                max: 12.0,
                step: 1.0,
                show: Show::Tiles,
                help: "Radius of a patch near the base. Further out they get bigger.",
            },
            Field {
                label: "Ore richness",
                value: &mut self.ore_richness,
                min: 0.25,
                max: 3.0,
                step: 0.25,
                show: Show::Percent,
                help: "How much ore a patch holds. Further out up to 3 times as much.",
            },
            Field {
                label: "Oil groups",
                value: &mut self.oil_groups,
                min: 0.0,
                max: 4.0,
                step: 0.5,
                show: Show::Number,
                help: "Groups of oil nodes (2 x 2 tiles each) per 10,000 tiles - plus one node alone next to the base. Near the base 2 nodes per group, at the edge 6. 1 on 256 tiles: about 6 groups.",
            },
        ]
    }
}

/// Blank: only single trees, if you want them.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BlankParams {
    pub trees: f32,
}

impl Default for BlankParams {
    fn default() -> BlankParams {
        BlankParams { trees: 0.0 }
    }
}

impl BlankParams {
    fn fields(&mut self) -> Vec<Field<'_>> {
        vec![tools::trees_field(&mut self.trees)]
    }
}

/// The round on this map (DESIGN.md 4.1, 10.1), picked like in TAB: how many days (a duration of
/// `data\game_durations.json`) and how hard (a difficulty of `data\game_difficulties.json`, by its name). Not used by
/// the generator itself, but by `World::generate`: the zombies, the waves, what the colony starts with.
/// Setups saved before the difficulties had a slider "waves" here – serde skips it.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct RoundSetup {
    /// How many days the round lasts – won at the end of the last one, the final wave at 92 % (TAB's plan, stretched).
    /// 1 day = 60 seconds. (A float: setups and map files have always kept it so.)
    pub days: f32,
    /// `""`: the default one.
    pub difficulty: String,
}

impl Default for RoundSetup {
    fn default() -> RoundSetup {
        // TAB's default: 100 days (≈ 1 h 40 min at speed x1).
        RoundSetup { days: 100.0, difficulty: String::new() }
    }
}

/// Everything that decides how a map turns out – except the seed, which decides *where* things are.
/// Saved in the settings file, so the setup screen remembers the last values.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct MapSetup {
    /// The map of `data\game_maps.json` it was picked as (home screen, like TAB) – `""` once a value was changed by
    /// hand on the setup page.
    pub map: String,
    pub generator: Generator,
    pub common: Common,
    /// `default`: setups saved before TAB's generator have none.
    pub tab: tab::Params,
    pub blank: BlankParams,
    /// `default` (from the `#[serde(default)]` above): setups saved before 2026-10-17 have no round yet.
    pub round: RoundSetup,
    /// What the landscape does to the round (`data::Modifiers`) – from the map of `game_maps.json` it was picked as.
    /// It doesn't change the map, only the round on it (`World::modifiers`).
    pub modifiers: Modifiers,
    /// Which of TAB's landscapes it's drawn as (`"BR"`, `"FA"` …, its pictures in `assets\terrain\`) – only the
    /// picture, the simulation never looks at it. `""`: BR's.
    pub look: String,
}

impl Default for MapSetup {
    fn default() -> MapSetup {
        MapSetup {
            map: String::new(),
            generator: Generator::Tab,
            common: Common::default(),
            tab: tab::Params::default(),
            blank: BlankParams::default(),
            round: RoundSetup::default(),
            modifiers: Modifiers::default(),
            look: String::new(),
        }
    }
}

impl MapSetup {
    /// The values of the chosen generator (without the common ones).
    pub fn generator_fields(&mut self) -> Vec<Field<'_>> {
        match self.generator {
            Generator::Tab => self.tab.fields(),
            Generator::Blank => self.blank.fields(),
        }
    }

    /// The common values that matter for the chosen generator – TAB's does everything else itself.
    pub fn common_fields(&mut self) -> Vec<Field<'_>> {
        let tab = self.generator == Generator::Tab;
        self.common.fields().into_iter().filter(|f| !tab || matches!(f.label, "Map size" | "Spawners per side")).collect()
    }

    /// The chosen generator's values back to the defaults.
    pub fn reset_generator(&mut self) {
        match self.generator {
            Generator::Tab => self.tab = tab::Params::default(),
            Generator::Blank => self.blank = BlankParams::default(),
        }
    }

    /// Rolls every value of the chosen generator – in the middle 70 % of its range, the very ends rarely make good
    /// maps. The map size and the other common values stay.
    pub fn randomize_generator(&mut self, rng: &mut Rng) {
        for mut field in self.generator_fields() {
            field.set_fraction(0.15 + 0.7 * rng.unit_f32());
        }
    }

    /// Edge length of the whole grid. The playing field is a diamond in it (`map.rs`) – as many tiles as a square of
    /// `size` makes it `size / √2` steps of the screen axes to every side of the middle, then comes the ring.
    pub fn map_size(&self) -> usize {
        let half = (self.common.size / std::f32::consts::SQRT_2).round() as usize;
        2 * (half + BORDER)
    }
}

/// A spawner as the generator plans it – `sim.rs` makes the real one out of it. A rectangle in the screen axes
/// (`map::to_frame`), like the real one.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnerSpot {
    pub name: String,
    pub min: (i32, i32),
    pub size: (i32, i32),
}

impl SpawnerSpot {
    /// Where it lies, in the screen axes – only the tests ask (the pictures take its tiles, `sim::spawner_tiles`).
    #[cfg(test)]
    pub fn region(&self) -> crate::map::Region {
        let min = vec2(self.min.0 as f32, self.min.1 as f32);
        crate::map::Region { min, max: min + vec2(self.size.0 as f32, self.size.1 as f32) }
    }
}

/// A finished map and where its spawners, the houses of its villages of doom and its loot go.
pub struct Generated {
    pub map: Map,
    pub spawners: Vec<SpawnerSpot>,
    pub houses: Vec<HouseSpot>,
    pub loot: Vec<LootSpot>,
    /// Treasure ruins and power line towers – only to be looked at (`sim/ornaments.rs`). TAB's generator only.
    pub ornaments: Vec<crate::sim::Ornament>,
    /// The command center's middle tile – the middle of the map, TAB's generator puts it on the free spot nearest it.
    pub base: (i32, i32),
    /// The zombies lying on the map at the start (TAB's generator) – `None`: the world scatters them by its difficulty
    /// (`scatter_roamers`, `place_giants`).
    pub zombies: Option<Vec<ZombieSpot>>,
}

/// A pile of loot: its tile, and what lies in it – `None`: the world rolls it (`roll_loot`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LootSpot {
    pub tile: (i32, i32),
    pub haul: Option<(LootKind, u32)>,
}

/// A zombie the generator puts on the map – it lies asleep there at the start (`sleep.rs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZombieSpot {
    pub pos: Vec2,
    pub breed: Breed,
}

/// A start spawner: this deep and this far from the edge of the map (steps of the screen axes) – inside the ring of
/// `BORDER` steps outside the world border. How long it is depends on how many share a side.
const SPAWNER_DEPTH: i32 = 10;
const SPAWNER_INSET: i32 = 3;
/// Open places smaller than this (tiles) are filled instead of connected – a hole of 5 tiles in a forest isn't
/// worth a path.
const MIN_POCKET: usize = 24;

/// Makes the map: TAB's generator on its own – Blank paints its grass, then the shared steps.
pub fn generate(seed: u64, setup: &MapSetup) -> Generated {
    generate_counting(seed, setup).0
}

/// `generate`, and how many blocked tiles the safety net (`tools::connect_pockets`) still had to open – and the middle
/// of every village of doom (the tests count them).
fn generate_counting(seed: u64, setup: &MapSetup) -> (Generated, usize, Vec<Vec2>) {
    let size = setup.map_size();
    if setup.generator == Generator::Tab {
        let (generated, villages) = tab::generate(seed, size, &setup.common, &setup.tab);
        return (generated, 0, villages);
    }
    let mut rng = Rng::new(seed);
    let mut tiles = Tiles::new(size);
    let center = tiles.center();
    let center_point = vec2(center.0 as f32 + 0.5, center.1 as f32 + 0.5);
    let c = &setup.common;
    // Inside the world border: what `Map::in_play` will say. Beyond `full` the corners of the grid are off the map.
    let (play, full) = (Map::play_area_of(size, BORDER), Map::full_area_of(size));
    let ctx = Context {
        center: center_point,
        keep_out: c.base_clear,
        grid: BuildingKind::CommandCenter.stats().power_radius,
        relay: BuildingKind::TeslaTower.stats().power_radius,
    };
    tools::scatter_trees(&mut tiles, &mut rng, setup.blank.trees);

    // The ring outside the border is open ground – the terrain ends at the border you see. Zombies coming out of a
    // spawner walk straight in where the field is open, or along the ring to the next opening (like a TAB swarm from
    // the north that comes in from the west because the north is blocked) – never through the terrain out there.
    // The spawners lie in it, so they are free as well.
    tiles.clear_outside(&play, &full);
    tiles.clear_circle(ctx.center, ctx.keep_out);
    // The start area: forest within reach of the command center's grid (the ore's start patches come with the ore).
    start::ensure_forest(&mut tiles, &mut rng, &ctx);
    let rough = Noise::new(rng.next_u64() as u32);
    let cut = tools::connect_pockets(&mut tiles, center, MIN_POCKET, &rough, |x, y| play.contains_tile(x, y), |x, y| full.contains_tile(x, y));

    let ore = ores::OreSettings { per_10k: c.ore_patches, size: c.ore_size, richness: c.ore_richness, oil_per_10k: c.oil_groups };
    let deposits = ores::place(&tiles, &mut rng, &ctx, &play, &ore);
    let village = villages::Settings { villages: c.villages, houses_near: c.houses_near, houses_edge: c.houses_edge, loot: c.loot_piles };
    let places = villages::place(&tiles, &deposits, &mut rng, &ctx, &play, &village);
    let mut map = Map::from_terrain(size, BORDER, tiles.into_terrain());
    for (x, y, deposit) in deposits {
        map.set_deposit(x, y, Some(deposit));
    }
    let spawners = spawner_spots(size, c.spawners_per_side.round() as i32);
    let loot = places.loot.into_iter().map(|tile| LootSpot { tile, haul: None }).collect();
    let base = (size as i32 / 2, size as i32 / 2);
    (Generated { map, spawners, houses: places.houses, loot, ornaments: Vec::new(), base, zombies: None }, cut, places.villages)
}

/// What every generator needs to know besides its own values: where the base is, how much room it keeps, and
/// where the ways in run.
pub struct Context {
    pub center: Vec2,
    /// Radius around the base that stays free (tiles) – generators let their features fade out before it, so the
    /// clearing doesn't look cut out with scissors.
    pub keep_out: f32,
    /// How far the command center's power reaches and a tesla tower's (`data\buildings.json`, tiles) – the start area
    /// is planned with them (`start.rs`, `ores.rs`): what a building needs at the start lies within the grid, or one
    /// tesla tower further.
    pub grid: f32,
    pub relay: f32,
}

/// `per_side` spawners on every side, in the ring outside the world border. Side by side they cover the whole side
/// (only a gap of 2 between two, so the editor shows them apart): a wave from one side comes out of all of them –
/// along the whole side like a TAB swarm, not out of a few points. Named after the side as you see it on screen.
/// In the screen axes (`map::to_frame`), like the border: there every side is a straight line.
fn spawner_spots(size: usize, per_side: i32) -> Vec<SpawnerSpot> {
    let (depth, inset) = (SPAWNER_DEPTH, SPAWNER_INSET);
    let (play, full) = (Map::play_area_of(size, BORDER), Map::full_area_of(size));
    // The playing field and the whole map in whole steps: on the screen axes the middle of the map is (0, size).
    let (lo, hi) = (play.min.x.round() as i32, play.max.x.round() as i32);
    let (top, bottom) = (full.min.y.round() as i32, full.max.y.round() as i32);
    let (left, right) = (full.min.x.round() as i32, full.max.x.round() as i32);
    let shift = play.min.y.round() as i32 - lo; // the screen axis down (b) is the one across (a) moved by this much
    let mut spots = Vec::new();
    for (side, name) in ["N", "S", "W", "E"].into_iter().enumerate() {
        for k in 0..per_side {
            // The k-th piece of the side, as long as the playing field is wide, 1 off at both ends.
            let from = lo + (hi - lo) * k / per_side + 1;
            let len = lo + (hi - lo) * (k + 1) / per_side - 1 - from;
            let (min, rect) = match side {
                0 => ((from, top + inset), (len, depth)),
                1 => ((from, bottom - inset - depth), (len, depth)),
                2 => ((left + inset, from + shift), (depth, len)),
                _ => ((right - inset - depth, from + shift), (depth, len)),
            };
            spots.push(SpawnerSpot { name: format!("{name} {}", k + 1), min, size: rect });
        }
    }
    spots
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::villages::village_radius;
    use crate::map::Terrain;

    fn setup(generator: Generator) -> MapSetup {
        MapSetup { generator, common: Common { size: 128.0, ..Common::default() }, ..MapSetup::default() }
    }

    /// The map of ours – what the shared steps promise (the safety net, the start area, the ore) only it keeps. TAB's
    /// makes its map its own way (`tab.rs` has its tests).
    fn ours() -> impl Iterator<Item = Generator> {
        [Generator::Blank].into_iter()
    }

    /// Both generators.
    const BOTH: [Generator; 2] = [Generator::Tab, Generator::Blank];

    /// Tiles you can walk on, reachable from the base (4 neighbours – stricter than the units, who also go diagonally).
    fn reachable(map: &Map) -> Vec<bool> {
        let n = map.size as i32;
        let mut seen = vec![false; map.size * map.size];
        let c = n / 2;
        let mut stack = vec![(c, c)];
        seen[(c * n + c) as usize] = true;
        while let Some((x, y)) = stack.pop() {
            for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if nx >= 0 && ny >= 0 && nx < n && ny < n && !map.is_solid(nx, ny) && !seen[(ny * n + nx) as usize] {
                    seen[(ny * n + nx) as usize] = true;
                    stack.push((nx, ny));
                }
            }
        }
        seen
    }

    /// For every generator of ours and a few seeds: the base is free, every spawner reaches it, and there is no open
    /// place the base can't reach.
    #[test]
    fn every_generator_keeps_the_base_free_and_everything_connected() {
        for generator in ours() {
            for seed in 1..=4 {
                let s = setup(generator);
                let g = generate(seed, &s);
                let map = &g.map;
                let c = map.size as i32 / 2;
                let r = s.common.base_clear as i32 - 1;
                for y in c - r..=c + r {
                    for x in c - r..=c + r {
                        if (x - c) * (x - c) + (y - c) * (y - c) <= r * r {
                            assert!(!map.terrain(x, y).is_solid(), "{generator:?} seed {seed}: ({x}, {y}) at the base");
                        }
                    }
                }
                let seen = reachable(map);
                for y in 0..map.size as i32 {
                    for x in 0..map.size as i32 {
                        if !map.is_solid(x, y) {
                            assert!(seen[y as usize * map.size + x as usize], "{generator:?} seed {seed}: ({x}, {y}) cut off");
                        }
                    }
                }
                assert_eq!(g.spawners.len(), 12);
            }
        }
    }

    /// Same seed + same values = same map; another seed = another map. TAB's puts something on the map.
    #[test]
    fn generators_are_repeatable_and_listen_to_their_values() {
        for generator in BOTH {
            let s = setup(generator);
            let a = generate(5, &s).map;
            assert!(a.same_terrain(&generate(5, &s).map), "{generator:?} not repeatable");
            if generator != Generator::Blank {
                assert!(!a.same_terrain(&generate(6, &s).map), "{generator:?} ignores the seed");
                let solid = (0..a.size as i32).flat_map(|y| (0..a.size as i32).map(move |x| (x, y)))
                    .filter(|&(x, y)| a.is_solid(x, y))
                    .count();
                assert!(solid > a.size * a.size / 20, "{generator:?}: only {solid} solid tiles");
            }
        }
    }

    /// The map size slider really changes the size; randomizing stays inside the ranges and is repeatable.
    #[test]
    fn size_and_randomize() {
        let mut s = MapSetup::default();
        s.common.size = 160.0;
        // As many tiles inside the border as a square of 160 – give or take the stairs along the edge.
        let map = generate(1, &s).map;
        let inside = (0..map.size as i32).flat_map(|y| (0..map.size as i32).map(move |x| (x, y))).filter(|&(x, y)| map.in_play(x, y)).count();
        assert!((inside as f32 / (160.0 * 160.0) - 1.0).abs() < 0.03, "{inside} tiles inside the border");
        for generator in BOTH {
            s.generator = generator;
            let (mut a, mut b) = (s.clone(), s.clone());
            a.randomize_generator(&mut Rng::new(3));
            b.randomize_generator(&mut Rng::new(3));
            assert_eq!(a, b);
            for f in a.generator_fields() {
                assert!(*f.value >= f.min && *f.value <= f.max, "{generator:?} {}: {}", f.label, f.value);
            }
        }
    }

    /// Every open tile reaches the base: for the zombies anywhere on the map, for your units **inside the world border**
    /// (they can't leave it) – every generator of ours, default values and rolled ones, full size. Before 2026-10-17
    /// thousands of tiles per map were only connected through the ring outside the border. (TAB's leaves closed-off
    /// pockets as they are – nothing lies in them.)
    #[test]
    fn units_and_zombies_reach_the_base_from_everywhere() {
        for generator in ours() {
            for seed in 1..=6u64 {
                let mut s = MapSetup { generator, ..MapSetup::default() };
                if seed > 3 {
                    s.randomize_generator(&mut Rng::new(seed * 77));
                }
                let g = generate(seed, &s);
                let map = &g.map;
                for units in [false, true] {
                    // Oil counts as open: ways go round a node, but a body gets past its pool at the corners.
                    let open = |x: i32, y: i32| if units { !map.blocks_unit_body(x, y) } else { !map.is_solid(x, y) };
                    let seen = flood_from_base(map, &open);
                    let n = map.size as i32;
                    let cut = (0..n).flat_map(|y| (0..n).map(move |x| (x, y))).filter(|&(x, y)| open(x, y) && !seen[(y * n + x) as usize]).count();
                    assert_eq!(cut, 0, "{generator:?} seed {seed}: {cut} tiles cut off for the {}", if units { "units" } else { "zombies" });
                    if !units {
                        for sp in &g.spawners {
                            let c = sp.region().center();
                            assert!(seen[(c.y as i32 * n + c.x as i32) as usize], "{generator:?} seed {seed}: spawner {} cut off", sp.name);
                        }
                    }
                }
            }
        }
    }

    /// Tiles reachable from the base with straight steps over `open` tiles.
    fn flood_from_base(map: &Map, open: &dyn Fn(i32, i32) -> bool) -> Vec<bool> {
        let n = map.size as i32;
        let mut seen = vec![false; map.size * map.size];
        let c = n / 2;
        let mut stack = vec![(c, c)];
        seen[(c * n + c) as usize] = true;
        while let Some((x, y)) = stack.pop() {
            for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if nx >= 0 && ny >= 0 && nx < n && ny < n && open(nx, ny) && !seen[(ny * n + nx) as usize] {
                    seen[(ny * n + nx) as usize] = true;
                    stack.push((nx, ny));
                }
            }
        }
        seen
    }

    /// Ore: one patch of each kind right next to the base on every map, all of it on open ground inside the border,
    /// patches further out richer – and the slider for patches really changes how many there are.
    #[test]
    fn ore_patches_start_near_the_base_and_get_richer_further_out() {
        use crate::map::Ore;
        for generator in ours() {
            for seed in 1..=3u64 {
                let s = MapSetup { generator, ..MapSetup::default() };
                let map = generate(seed, &s).map;
                let base = vec2(map.size as f32 / 2.0, map.size as f32 / 2.0);
                let deposits: Vec<_> = (0..map.size as i32)
                    .flat_map(|y| (0..map.size as i32).map(move |x| (x, y)))
                    .filter_map(|(x, y)| map.deposit(x, y).map(|d| (x, y, d)))
                    .filter(|(_, _, d)| d.ore != Ore::Oil) // oil has its own test
                    .collect();
                let near = s.common.base_clear + 4.0 + 2.0 + 10.0 + 4.0 * 1.5;
                for ore in Ore::PATCHES {
                    let close = deposits.iter().any(|&(x, y, d)| d.ore == ore && vec2(x as f32 + 0.5, y as f32 + 0.5).distance(base) < near);
                    assert!(close, "{generator:?} seed {seed}: no {ore:?} near the base");
                }
                for &(x, y, _) in &deposits {
                    assert!(map.in_play(x, y) && !map.terrain(x, y).is_solid(), "{generator:?} seed {seed}: ore at ({x}, {y})");
                }
                // The richest tile near the base against the richest far out.
                let richest = |far: bool| {
                    deposits
                        .iter()
                        .filter(|&&(x, y, _)| (vec2(x as f32, y as f32).distance(base) > 90.0) == far)
                        .map(|&(_, _, d)| d.amount)
                        .max()
                        .unwrap_or(0)
                };
                assert!(richest(true) > richest(false), "{generator:?} seed {seed}: far patches not richer");
            }
        }
        let count = |patches: f32| {
            let mut s = MapSetup { generator: Generator::Blank, ..MapSetup::default() };
            s.common.ore_patches = patches;
            let map = generate(2, &s).map;
            (0..map.size as i32)
                .flat_map(|y| (0..map.size as i32).map(move |x| (x, y)))
                .filter(|&(x, y)| map.deposit(x, y).is_some_and(|d| d.ore != Ore::Oil))
                .count()
        };
        assert!(count(10.0) > count(2.0) * 2, "the slider doesn't change the ore");
    }

    /// Oil: only whole nodes of 2 × 2 open tiles inside the border that never touch each other (not even at a
    /// corner), exactly one of them near the base – and the slider for oil groups really adds nodes.
    #[test]
    fn oil_lies_in_separate_two_by_two_nodes_with_one_near_the_base() {
        use crate::map::Ore;
        let oil_nodes = |s: &MapSetup, seed: u64| {
            let map = generate(seed, s).map;
            let oil = |x: i32, y: i32| map.deposit(x, y).is_some_and(|d| d.ore == Ore::Oil);
            let mut nodes = Vec::new();
            for y in 0..map.size as i32 {
                for x in 0..map.size as i32 {
                    if !oil(x, y) {
                        continue;
                    }
                    assert!(map.in_play(x, y) && !map.terrain(x, y).is_solid(), "seed {seed}: oil at ({x}, {y})");
                    // The top corner of a node: nothing above it or to its left.
                    if oil(x - 1, y) || oil(x, y - 1) {
                        continue;
                    }
                    for dy in -1..3 {
                        for dx in -1..3 {
                            let inside = (0..2).contains(&dx) && (0..2).contains(&dy);
                            assert_eq!(oil(x + dx, y + dy), inside, "seed {seed}: the node at ({x}, {y}) isn't a lone 2 x 2 square");
                        }
                    }
                    nodes.push(vec2(x as f32 + 1.0, y as f32 + 1.0));
                }
            }
            (nodes, vec2(map.size as f32 / 2.0, map.size as f32 / 2.0))
        };
        for generator in ours() {
            for seed in 1..=3u64 {
                let s = MapSetup { generator, ..MapSetup::default() };
                let (nodes, base) = oil_nodes(&s, seed);
                let near = nodes.iter().filter(|n| n.distance(base) < s.common.base_clear + 16.0).count();
                assert_eq!(near, 1, "{generator:?} seed {seed}: {near} oil nodes near the base instead of one");
                assert!(nodes.len() > 6, "{generator:?} seed {seed}: only {} oil nodes", nodes.len());
            }
        }
        let count = |groups: f32| {
            let mut s = MapSetup { generator: Generator::Blank, ..MapSetup::default() };
            s.common.oil_groups = groups;
            oil_nodes(&s, 2).0.len()
        };
        assert_eq!(count(0.0), 1, "without oil groups only the node next to the base should be left");
        assert!(count(3.0) > count(1.0), "the slider doesn't change the oil");
    }

    /// The terrain ends at the world border: the ring outside is open ground on every map (grass – TAB's: dirt, only a
    /// few of its 2 × 2 mountains reach over the border), so zombies never have to find their way through terrain out
    /// there (before 2026-10-17 the corner spawners sent
    /// theirs past the other spawners). The spawners cover their sides – together nearly the whole length of each side,
    /// inside the ring.
    #[test]
    fn the_ring_is_open_and_the_spawners_cover_every_side() {
        for generator in BOTH {
            let s = MapSetup { generator, ..MapSetup::default() };
            let g = generate(4, &s);
            let n = g.map.size as i32;
            let full = g.map.full_area();
            let open = if generator == Generator::Tab { Terrain::Dirt } else { Terrain::Grass };
            let ring: Vec<Terrain> = (0..n)
                .flat_map(|y| (0..n).map(move |x| (x, y)))
                .filter(|&(x, y)| full.contains_tile(x, y) && !g.map.in_play(x, y))
                .map(|(x, y)| g.map.terrain(x, y))
                .collect();
            let closed = ring.iter().filter(|&&t| t != open).count();
            let allowed = if generator == Generator::Tab { ring.len() / 100 } else { 0 };
            assert!(closed <= allowed, "{generator:?}: {closed} tiles outside the border aren't {open:?}");
            let play = g.map.play_area();
            let width = (play.max.x - play.min.x) as i32;
            for side in ["N", "S", "W", "E"] {
                let spots: Vec<_> = g.spawners.iter().filter(|sp| sp.name.starts_with(side)).collect();
                assert_eq!(spots.len(), 3);
                let length: i32 = spots.iter().map(|sp| sp.size.0.max(sp.size.1)).sum();
                assert!(length * 10 >= width * 9, "{generator:?} {side}: spawners cover {length} of {width}");
                for sp in spots {
                    for (y, xs) in sp.region().rows(0.0) {
                        for x in xs {
                            assert!(full.contains_tile(x, y) && !g.map.in_play(x, y), "{side}: spawner not in the ring at ({x}, {y})");
                        }
                    }
                }
            }
        }
    }

    /// The map decides how many villages and how big – many small ones (4 per 10,000 tiles, 2–3 houses) or two huge
    /// ones (0.3, 15–25 houses), on the same generator and seed. A huge village spreads wider: no house further from
    /// its middle than its radius allows.
    #[test]
    fn many_small_villages_or_two_huge_ones() {
        let mut small = MapSetup { generator: Generator::Blank, ..MapSetup::default() };
        (small.common.villages, small.common.houses_near, small.common.houses_edge) = (4.0, 2.0, 3.0);
        let mut huge = small.clone();
        (huge.common.villages, huge.common.houses_near, huge.common.houses_edge) = (0.3, 15.0, 25.0);
        let (s, _, small_villages) = generate_counting(5, &small);
        let (h, _, huge_villages) = generate_counting(5, &huge);
        assert!(small_villages.len() >= 15, "only {} small villages", small_villages.len());
        assert_eq!(huge_villages.len(), 2);
        let per_village = |g: &Generated, middles: &[Vec2]| g.houses.len() as f32 / middles.len() as f32;
        assert!(per_village(&s, &small_villages) <= 3.0, "{} houses per small village", per_village(&s, &small_villages));
        assert!(per_village(&h, &huge_villages) >= 15.0, "{} houses per huge village", per_village(&h, &huge_villages));
        for house in &h.houses {
            let near = huge_villages.iter().map(|m| (house.center() - *m).abs().max_element()).fold(f32::INFINITY, f32::min);
            assert!(near <= village_radius(25) + 2.0, "a house {near} tiles from its village");
        }
    }

    /// Not a real test – a tool: writes a picture of every map of `data\game_maps.json` (seeds 1 to `DUMP_SEEDS`,
    /// default 2) as PNG into the folder `DUMP_DIR`. Turned like the minimap and TAB's map: the
    /// playing field is a square, the ring around it darker, the base a blue dot. That's how the generators were tuned:
    /// `$env:DUMP_DIR = "C:\temp\maps"; cargo test dump_previews -- --ignored`
    #[test]
    #[ignore]
    fn dump_previews() {
        let out = std::env::var("DUMP_DIR").expect("set DUMP_DIR to a folder");
        let seeds: u64 = std::env::var("DUMP_SEEDS").ok().and_then(|s| s.parse().ok()).unwrap_or(2);
        let setups: Vec<(String, MapSetup)> = crate::data::get().maps.maps.iter().map(|m| (m.name.replace(' ', "_"), m.setup.clone())).collect();
        for (name, setup) in setups {
            for seed in 1..=seeds {
                write_preview(&generate(seed, &setup), &format!("{out}/{name}_{seed}.png"));
            }
        }
    }

    /// The map as a PNG, turned like on screen (`to_frame`): 2 pixels per step of the screen axes.
    fn write_preview(g: &Generated, path: &str) {
        use crate::map::from_frame;
        use macroquad::color::Color;
        let map = &g.map;
        let full = map.full_area();
        let scale = 2.0;
        let (w, h) = (((full.max.x - full.min.x) * scale) as usize, ((full.max.y - full.min.y) * scale) as usize);
        let base = vec2(g.base.0 as f32 + 0.5, g.base.1 as f32 + 0.5);
        let mut bytes = vec![0u8; w * h * 4];
        for py in 0..h {
            for px in 0..w {
                let p = from_frame(full.min + vec2(px as f32 + 0.5, py as f32 + 0.5) / scale);
                let (x, y) = (p.x.floor() as i32, p.y.floor() as i32);
                let mut c = match map.deposit(x, y) {
                    Some(d) => crate::draw::ore_color(d.ore),
                    None => crate::draw::terrain_color(map.terrain(x, y)),
                };
                if g.spawners.iter().any(|sp| sp.region().contains_tile(x, y)) {
                    c = Color::new(0.85, 0.25, 0.55, 1.0);
                } else if !map.in_play(x, y) {
                    c = Color::new(c.r * 0.6, c.g * 0.6, c.b * 0.6, 1.0);
                }
                if p.distance(base) < 2.5 {
                    c = Color::new(0.32, 0.45, 0.70, 1.0);
                }
                let i = (py * w + px) * 4;
                bytes[i..i + 4].copy_from_slice(&[(c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8, 255]);
            }
        }
        macroquad::texture::Image { bytes, width: w as u16, height: h as u16 }.export_png(path);
    }
}
