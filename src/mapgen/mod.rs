//! Map generators (DESIGN.md 9.1): several ways to make a map, each with its own values you can change on the
//! setup screen (and in the editor's "Map" tab). The same seed and the same values always give the same map – also
//! on the other PC in co-op (DESIGN.md 12.8). That's why there is no `sin`, `cos` or `powf` in here: they can
//! differ in the last bit between CPUs. Only `+ - * /` and `sqrt` (both exactly defined by IEEE 754).
//!
//! Every generator only paints terrain. What all of them share comes before and after, here in `generate`:
//! before, the **ways in** (`lanes.rs`) – open lanes from every side to the base that the generator grows its
//! terrain around; after, the ring outside the world border cleared to open ground (the zombies' way around the map),
//! the spawners in it along every side, the free space around the base, a way from every open place that is still
//! cut off (`tools::connect_pockets` – the safety net, so no zombie and no unit is ever stuck in a closed-off hole)
//! and the ore patches (`ores.rs`).
//!
//! The generators:
//! - **Wild** (`wild.rs`): noise like in Factorio – lakes, mountain ridges, forests, winding rivers.
//! - **Regions** (`regions.rs`): the map split into regions like in TAB; walls of rock or forest between them with
//!   a few passes – the chokepoints you wall off.
//! - **Rings** (`rings.rs`): rings around the base with a few openings each – a fortress layout.
//! - **Canyons** (`canyons.rs`): a cellular automaton (like cave generators) – winding canyons in solid rock.
//! - **Blank**: only grass (and trees if you like) – to build a map by hand in the editor.

mod canyons;
mod lanes;
mod ores;
mod regions;
mod rings;
mod tools;
mod wild;

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use crate::map::{Map, Terrain, BORDER};
use crate::noise::Noise;
use crate::rng::Rng;
use lanes::Lanes;
use tools::Tiles;

/// Which generator makes the map.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Generator {
    Wild,
    Regions,
    Rings,
    Canyons,
    Blank,
}

impl Generator {
    pub const ALL: [Generator; 5] = [Generator::Wild, Generator::Regions, Generator::Rings, Generator::Canyons, Generator::Blank];

    pub fn name(self) -> &'static str {
        match self {
            Generator::Wild => "Wild",
            Generator::Regions => "Regions",
            Generator::Rings => "Rings",
            Generator::Canyons => "Canyons",
            Generator::Blank => "Blank",
        }
    }

    /// One line for the setup screen.
    pub fn about(self) -> &'static str {
        match self {
            Generator::Wild => "Noise like Factorio: lakes, ridges, forests, rivers",
            Generator::Regions => "Regions like TAB, walled off - passes are chokepoints",
            Generator::Rings => "Rings around the base with openings - a fortress",
            Generator::Canyons => "Cellular automaton: canyons through solid rock",
            Generator::Blank => "Only grass - build the map by hand in the editor",
        }
    }

    /// Does it get the ways in (`lanes.rs`)? Not Rings – its openings are the ways in, and lanes straight through
    /// every ring would undo the zigzag – and not Blank, where nothing is in the way.
    fn has_ways_in(self) -> bool {
        matches!(self, Generator::Wild | Generator::Regions | Generator::Canyons)
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
    /// The value picks one of these names (0 → the first …).
    Choice(&'static [&'static str]),
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
            Show::Choice(names) => names.get(v.round() as usize).copied().unwrap_or("?").to_owned(),
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

/// Wall material for Regions, Rings and Canyons.
pub const MATERIALS: &[&str] = &["Mountains", "Forest", "Mixed", "Water"];

/// Values every generator has.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Common {
    /// Edge length of the playing field inside the world border (TAB: 256).
    pub size: f32,
    /// Radius around the base where nothing grows (tiles).
    pub base_clear: f32,
    pub spawners_per_side: f32,
    /// Open lanes from every side to the base (`lanes.rs`): how many per side and how wide on average (tiles).
    /// Setups saved before 2026-10-17 have none – `#[serde(default)]` gives them the defaults.
    pub ways_in: f32,
    pub way_width: f32,
    /// Zombies roaming the map at the start, per 1,000 tiles (`World::scatter_roamers`). Not used by the generator
    /// itself (they're no terrain), but by `World::generate`. Before 2026-10-17 it was "walkers" with 20 – the new name
    /// means old saved setups start with the new default instead of the old, much lower number.
    pub roamers: f32,
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
            ways_in: 2.0,
            way_width: 7.0,
            roamers: 60.0,
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
                help: "Edge of the playing field. TAB's survival maps are 256. Plus 16 tiles of spawn ring on each side.",
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
                label: "Ways in per side",
                value: &mut self.ways_in,
                min: 0.0,
                max: 4.0,
                step: 1.0,
                show: Show::Number,
                help: "Open lanes from every side to the base. The terrain grows around them; they join on the way in - the places to wall off. Rings: its openings are the ways in.",
            },
            Field {
                label: "Way width",
                value: &mut self.way_width,
                min: 3.0,
                max: 16.0,
                step: 1.0,
                show: Show::Tiles,
                help: "How wide the ways in are on average - they get wider and narrower along the way. Narrow = easy to wall off.",
            },
            Field {
                label: "Roaming zombies",
                value: &mut self.roamers,
                min: 0.0,
                max: 150.0,
                step: 5.0,
                show: Show::Number,
                help: "Zombies wandering the map from the start, per 1,000 tiles. Fewer near the base, more further out: walkers everywhere, runners further out, fat ones at the edge. 60 on 256 tiles: about 3,900.",
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

/// The round on this map (DESIGN.md 4.1, 10.1): how many days, how strong the waves. Like `roamers` not used by the
/// generator itself, but by `World::generate` – it plans the waves from it (`sim::plan_waves`).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct RoundSetup {
    /// The final wave comes on the last day. 1 day = 3 minutes.
    pub days: f32,
    /// All waves times this (1 = 100 %).
    pub waves: f32,
}

impl Default for RoundSetup {
    fn default() -> RoundSetup {
        // The mini round of DESIGN.md 13, step 4: 10 days ≈ 30 minutes, two waves and the final wave.
        RoundSetup { days: 10.0, waves: 1.0 }
    }
}

impl RoundSetup {
    fn fields(&mut self) -> Vec<Field<'_>> {
        vec![
            Field {
                label: "Days",
                value: &mut self.days,
                min: 3.0,
                max: 80.0,
                step: 1.0,
                show: Show::Number,
                help: "How long the round lasts - the final wave comes on the last day. 1 day = 3 minutes: 10 days = 30 min, 40 = 2 h.",
            },
            Field {
                label: "Waves",
                value: &mut self.waves,
                min: 0.25,
                max: 3.0,
                step: 0.25,
                show: Show::Percent,
                help: "How many zombies every wave brings, the final wave too. From day 4 a wave every 3 days (data\\spawning.json).",
            },
        ]
    }
}

/// Everything that decides how a map turns out – except the seed, which decides *where* things are.
/// Saved in the settings file, so the setup screen remembers the last values.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct MapSetup {
    pub generator: Generator,
    pub common: Common,
    pub wild: wild::Params,
    pub regions: regions::Params,
    pub rings: rings::Params,
    pub canyons: canyons::Params,
    pub blank: BlankParams,
    /// `default` (from the `#[serde(default)]` above): setups saved before 2026-10-17 have no round yet.
    pub round: RoundSetup,
}

impl Default for MapSetup {
    fn default() -> MapSetup {
        MapSetup {
            generator: Generator::Wild,
            common: Common::default(),
            wild: wild::Params::default(),
            regions: regions::Params::default(),
            rings: rings::Params::default(),
            canyons: canyons::Params::default(),
            blank: BlankParams::default(),
            round: RoundSetup::default(),
        }
    }
}

impl MapSetup {
    /// Days and waves of the round.
    pub fn round_fields(&mut self) -> Vec<Field<'_>> {
        self.round.fields()
    }

    /// The values of the chosen generator (without the common ones).
    pub fn generator_fields(&mut self) -> Vec<Field<'_>> {
        match self.generator {
            Generator::Wild => self.wild.fields(),
            Generator::Regions => self.regions.fields(),
            Generator::Rings => self.rings.fields(),
            Generator::Canyons => self.canyons.fields(),
            Generator::Blank => self.blank.fields(),
        }
    }

    pub fn common_fields(&mut self) -> Vec<Field<'_>> {
        self.common.fields()
    }

    /// The chosen generator's values back to the defaults.
    pub fn reset_generator(&mut self) {
        match self.generator {
            Generator::Wild => self.wild = wild::Params::default(),
            Generator::Regions => self.regions = regions::Params::default(),
            Generator::Rings => self.rings = rings::Params::default(),
            Generator::Canyons => self.canyons = canyons::Params::default(),
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

    /// Edge length of the whole map (playing field + ring on both sides).
    pub fn map_size(&self) -> usize {
        self.common.size.round() as usize + 2 * BORDER
    }
}

/// A spawner as the generator plans it – `sim.rs` makes the real one out of it.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnerSpot {
    pub name: String,
    pub min: (i32, i32),
    pub size: (i32, i32),
}

/// A finished map and where its spawners go.
pub struct Generated {
    pub map: Map,
    pub spawners: Vec<SpawnerSpot>,
}

/// A start spawner: this deep and this far from the map edge (tiles) – inside the ring of `BORDER` tiles outside
/// the world border. How long it is depends on how many share a side.
const SPAWNER_DEPTH: i32 = 8;
const SPAWNER_INSET: i32 = 4;
/// Beside a way in, features fade out within at most this many tiles (`Context::fade`).
const LANE_FADE: f32 = 4.0;
/// Open places smaller than this (tiles) are filled instead of connected – a hole of 5 tiles in a forest isn't
/// worth a path.
const MIN_POCKET: usize = 24;

/// Makes the map: the ways in are planned, the chosen generator paints the terrain around them, then the steps all
/// of them share.
pub fn generate(seed: u64, setup: &MapSetup) -> Generated {
    generate_counting(seed, setup).0
}

/// `generate`, and how many blocked tiles the safety net (`tools::connect_pockets`) still had to open – few is good:
/// the ways in and the generators themselves should connect nearly everything (a test checks it).
fn generate_counting(seed: u64, setup: &MapSetup) -> (Generated, usize) {
    let size = setup.map_size();
    let mut rng = Rng::new(seed);
    let mut tiles = Tiles::new(size);
    let center = tiles.center();
    let center_point = vec2(center.0 as f32 + 0.5, center.1 as f32 + 0.5);
    let c = &setup.common;
    // Inside the world border: what `Map::in_play` will say.
    let (lo, hi) = (BORDER as i32, size as i32 - BORDER as i32);
    let ctx = Context { center: center_point, keep_out: c.base_clear, lanes: ways_in(seed, setup) };
    match setup.generator {
        Generator::Wild => wild::generate(&mut tiles, &setup.wild, &mut rng, &ctx),
        Generator::Regions => regions::generate(&mut tiles, &setup.regions, &mut rng, &ctx),
        Generator::Rings => rings::generate(&mut tiles, &setup.rings, &mut rng, &ctx),
        Generator::Canyons => canyons::generate(&mut tiles, &setup.canyons, &mut rng, &ctx),
        Generator::Blank => tools::scatter_trees(&mut tiles, &mut rng, setup.blank.trees),
    }

    // The ring outside the border is open ground – the terrain ends at the border you see. Zombies coming out of a
    // spawner walk straight in where the field is open, or along the ring to the next opening (like a TAB swarm from
    // the north that comes in from the west because the north is blocked) – never through the terrain out there.
    // The spawners lie in it, so they are free as well.
    tiles.clear_outside(lo, hi);
    // What still lies in a way in – a river, the wall between two regions – is cleared: a ford, a pass.
    for (x, y) in tiles.all() {
        if ctx.lanes.is_open(x, y) {
            tiles.set(x, y, Terrain::Grass);
        }
    }
    tiles.clear_circle(ctx.center, ctx.keep_out);
    let rough = Noise::new(rng.next_u64() as u32);
    let cut = tools::connect_pockets(&mut tiles, center, MIN_POCKET, &rough, |x, y| x >= lo && y >= lo && x < hi && y < hi);

    let ore = ores::OreSettings { per_10k: c.ore_patches, size: c.ore_size, richness: c.ore_richness, oil_per_10k: c.oil_groups };
    let deposits = ores::place(&tiles, &mut rng, &ctx, (lo, hi), &ore);
    let mut map = Map::from_terrain(size, BORDER, tiles.into_terrain());
    for (x, y, deposit) in deposits {
        map.set_deposit(x, y, Some(deposit));
    }
    let spawners = spawner_spots(size as i32, c.spawners_per_side.round() as i32);
    (Generated { map, spawners }, cut)
}

/// The ways in of this map (`lanes.rs`) – none for the generators without them. They get their own random numbers:
/// changing how many or how wide only moves the lanes, the noise of the terrain stays the same – the map stays
/// recognisable.
fn ways_in(seed: u64, setup: &MapSetup) -> Lanes {
    let size = setup.map_size() as i32;
    if !setup.generator.has_ways_in() {
        return Lanes::none(size);
    }
    let c = &setup.common;
    let center = vec2((size / 2) as f32 + 0.5, (size / 2) as f32 + 0.5);
    let mut rng = Rng::new(seed ^ 0x4C41_4E45); // "LANE"
    Lanes::plan(&mut rng, size, BORDER as i32, center, c.base_clear, c.ways_in.round() as u32, c.way_width)
}

/// What every generator needs to know besides its own values: where the base is, how much room it keeps, and
/// where the ways in run.
pub struct Context {
    pub center: Vec2,
    /// Radius around the base that stays free (tiles) – generators let their features fade out before it, so the
    /// clearing doesn't look cut out with scissors.
    pub keep_out: f32,
    /// The ways in (`lanes.rs`) – they stay free just like the base. None for Rings and Blank.
    pub lanes: Lanes,
}

impl Context {
    /// 0 at the base and up to the free radius, rising smoothly to 1 within the next `fade` tiles – and the same
    /// along every way in: 0 inside the lane, 1 from `fade` tiles beside it, but at most `LANE_FADE` (a way in that
    /// opens the land 10 tiles to both sides isn't a chokepoint anymore). Whichever is lower.
    pub fn fade(&self, x: i32, y: i32, fade: f32) -> f32 {
        let d = vec2(x as f32 + 0.5, y as f32 + 0.5).distance(self.center);
        let base = tools::smoothstep(self.keep_out, self.keep_out + fade, d);
        let lane = tools::smoothstep(0.0, fade.min(LANE_FADE), self.lanes.clearance(x, y));
        base.min(lane)
    }
}

/// `per_side` spawners on every side, in the ring outside the world border. Side by side they cover the whole side
/// (only a 2-tile gap between two, so the editor shows them apart): a wave from one side comes out of all of them –
/// along the whole side like a TAB swarm, not out of a few points. Named after the side as you see it on screen (the
/// map is a diamond: the side with y = 0 runs to the top right, "NE").
fn spawner_spots(size: i32, per_side: i32) -> Vec<SpawnerSpot> {
    let (depth, inset) = (SPAWNER_DEPTH, SPAWNER_INSET);
    let (lo, hi) = (BORDER as i32, size - BORDER as i32);
    let mut spots = Vec::new();
    for (side, name) in ["NE", "SW", "NW", "SE"].into_iter().enumerate() {
        for k in 0..per_side {
            // The k-th piece of the side, as long as the playing field is wide, 1 tile off at both ends.
            let from = lo + (hi - lo) * k / per_side + 1;
            let len = lo + (hi - lo) * (k + 1) / per_side - 1 - from;
            let (min, rect) = match side {
                0 => ((from, inset), (len, depth)),
                1 => ((from, size - inset - depth), (len, depth)),
                2 => ((inset, from), (depth, len)),
                _ => ((size - inset - depth, from), (depth, len)),
            };
            spots.push(SpawnerSpot { name: format!("{name} {}", k + 1), min, size: rect });
        }
    }
    spots
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(generator: Generator) -> MapSetup {
        MapSetup { generator, common: Common { size: 128.0, ..Common::default() }, ..MapSetup::default() }
    }

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

    /// For every generator and a few seeds: the base is free, every spawner reaches it, and there is no open place
    /// the base can't reach.
    #[test]
    fn every_generator_keeps_the_base_free_and_everything_connected() {
        for generator in Generator::ALL {
            for seed in 1..=4 {
                let s = setup(generator);
                let g = generate(seed, &s);
                let map = &g.map;
                let c = map.size as i32 / 2;
                let r = s.common.base_clear as i32 - 1;
                for y in c - r..=c + r {
                    for x in c - r..=c + r {
                        if (x - c) * (x - c) + (y - c) * (y - c) <= r * r {
                            assert_eq!(map.terrain(x, y), Terrain::Grass, "{generator:?} seed {seed}: ({x}, {y}) at the base");
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

    /// Same seed + same values = same map; another seed or another value = another map. Every generator except
    /// Blank puts something on the map.
    #[test]
    fn generators_are_repeatable_and_listen_to_their_values() {
        for generator in Generator::ALL {
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
        let mut wet = setup(Generator::Wild);
        wet.wild.water = 0.25;
        let count_water = |m: &Map| (0..m.size as i32).flat_map(|y| (0..m.size as i32).map(move |x| (x, y)))
            .filter(|&(x, y)| m.terrain(x, y) == Terrain::Water)
            .count();
        assert!(count_water(&generate(5, &wet).map) > count_water(&generate(5, &setup(Generator::Wild)).map) * 2);
    }

    /// The map size slider really changes the size; randomizing stays inside the ranges and is repeatable.
    #[test]
    fn size_and_randomize() {
        let mut s = MapSetup::default();
        s.common.size = 160.0;
        assert_eq!(generate(1, &s).map.size, 160 + 2 * BORDER);
        for generator in Generator::ALL {
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
    /// (they can't leave it) – every generator, default values and rolled ones, full size. Before 2026-10-17 thousands of
    /// tiles per map were only connected through the ring outside the border.
    #[test]
    fn units_and_zombies_reach_the_base_from_everywhere() {
        for generator in Generator::ALL {
            for seed in 1..=6u64 {
                let mut s = MapSetup { generator, ..MapSetup::default() };
                if seed > 3 {
                    s.randomize_generator(&mut Rng::new(seed * 77));
                }
                let g = generate(seed, &s);
                let map = &g.map;
                for units in [false, true] {
                    let open = |x: i32, y: i32| if units { !map.blocks_unit(x, y) } else { !map.is_solid(x, y) };
                    let seen = flood_from_base(map, &open);
                    let n = map.size as i32;
                    let cut = (0..n).flat_map(|y| (0..n).map(move |x| (x, y))).filter(|&(x, y)| open(x, y) && !seen[(y * n + x) as usize]).count();
                    assert_eq!(cut, 0, "{generator:?} seed {seed}: {cut} tiles cut off for the {}", if units { "units" } else { "zombies" });
                    if !units {
                        for sp in &g.spawners {
                            assert!(seen[(sp.min.1 * n + sp.min.0) as usize], "{generator:?} seed {seed}: spawner {} cut off", sp.name);
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

    /// Ore: one patch of each kind right next to the base on every map, all of it on grass inside the world border,
    /// patches further out richer – and the slider for patches really changes how many there are.
    #[test]
    fn ore_patches_start_near_the_base_and_get_richer_further_out() {
        use crate::map::Ore;
        for generator in Generator::ALL {
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
                    assert!(map.in_play(x, y) && map.terrain(x, y) == Terrain::Grass, "{generator:?} seed {seed}: ore at ({x}, {y})");
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
            let mut s = MapSetup::default();
            s.common.ore_patches = patches;
            let map = generate(2, &s).map;
            (0..map.size as i32)
                .flat_map(|y| (0..map.size as i32).map(move |x| (x, y)))
                .filter(|&(x, y)| map.deposit(x, y).is_some_and(|d| d.ore != Ore::Oil))
                .count()
        };
        assert!(count(10.0) > count(2.0) * 2, "the slider doesn't change the ore");
    }

    /// Oil: only whole nodes of 2 × 2 grass tiles inside the border that never touch each other (not even at a
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
                    assert!(map.in_play(x, y) && map.terrain(x, y) == Terrain::Grass, "seed {seed}: oil at ({x}, {y})");
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
        for generator in Generator::ALL {
            for seed in 1..=3u64 {
                let s = MapSetup { generator, ..MapSetup::default() };
                let (nodes, base) = oil_nodes(&s, seed);
                let near = nodes.iter().filter(|n| n.distance(base) < s.common.base_clear + 16.0).count();
                assert_eq!(near, 1, "{generator:?} seed {seed}: {near} oil nodes near the base instead of one");
                assert!(nodes.len() > 6, "{generator:?} seed {seed}: only {} oil nodes", nodes.len());
            }
        }
        let count = |groups: f32| {
            let mut s = MapSetup::default();
            s.common.oil_groups = groups;
            oil_nodes(&s, 2).0.len()
        };
        assert_eq!(count(0.0), 1, "without oil groups only the node next to the base should be left");
        assert!(count(3.0) > count(1.0), "the slider doesn't change the oil");
    }

    /// The terrain ends at the world border: the ring outside is open ground on every map, so zombies never have to
    /// find their way through terrain out there (before 2026-10-17 the corner spawners sent theirs past the other
    /// spawners). The spawners cover their sides – together nearly the whole length of each side, inside the ring.
    #[test]
    fn the_ring_is_open_and_the_spawners_cover_every_side() {
        for generator in Generator::ALL {
            let s = MapSetup { generator, ..MapSetup::default() };
            let g = generate(4, &s);
            let n = g.map.size as i32;
            for (x, y) in (0..n).flat_map(|y| (0..n).map(move |x| (x, y))) {
                if !g.map.in_play(x, y) {
                    assert_eq!(g.map.terrain(x, y), Terrain::Grass, "{generator:?}: ({x}, {y}) outside the border");
                }
            }
            let play = n - 2 * BORDER as i32;
            for side in ["NE", "SW", "NW", "SE"] {
                let spots: Vec<_> = g.spawners.iter().filter(|sp| sp.name.starts_with(side)).collect();
                assert_eq!(spots.len(), 3);
                let length: i32 = spots.iter().map(|sp| sp.size.0.max(sp.size.1)).sum();
                assert!(length * 10 >= play * 9, "{generator:?} {side}: spawners cover {length} of {play} tiles");
                for sp in spots {
                    for (x, y) in [sp.min, (sp.min.0 + sp.size.0 - 1, sp.min.1 + sp.size.1 - 1)] {
                        assert!(x >= 0 && y >= 0 && x < n && y < n && !g.map.in_play(x, y), "{side}: spawner not in the ring");
                    }
                }
            }
        }
    }

    /// The ways in stay open – every tile of every lane is grass in the finished map (a river gets a ford there, a
    /// region wall a pass) – and so the safety net hardly has to cut anything: less than 0.5 % of the playing field,
    /// for the default values (before the ways in: up to 600 tiles = 0.9 % on Canyons, in 2-tile lines).
    #[test]
    fn ways_in_stay_open_and_the_safety_net_cuts_little() {
        for generator in Generator::ALL {
            for seed in 1..=4u64 {
                let s = MapSetup { generator, ..MapSetup::default() };
                let (g, cut) = generate_counting(seed, &s);
                let lanes = ways_in(seed, &s);
                let n = g.map.size as i32;
                let mut lane_tiles = 0;
                for (x, y) in (0..n).flat_map(|y| (0..n).map(move |x| (x, y))) {
                    if lanes.is_open(x, y) {
                        lane_tiles += 1;
                        assert_eq!(g.map.terrain(x, y), Terrain::Grass, "{generator:?} seed {seed}: ({x}, {y}) in a way in");
                    }
                }
                assert_eq!(lane_tiles > 0, generator.has_ways_in(), "{generator:?}: ways in or not?");
                let play = (n - 2 * BORDER as i32) as usize;
                assert!(cut * 200 < play * play, "{generator:?} seed {seed}: the safety net opened {cut} tiles");
            }
        }
    }

    /// Not a real test – a tool: writes a picture of every generator (2 seeds) as PNG into the folder `DUMP_DIR`,
    /// one pixel per tile, twice as big. That's how the generators were tuned on 2026-10-17:
    /// `$env:DUMP_DIR = "C:\temp\maps"; cargo test dump_previews -- --ignored`
    #[test]
    #[ignore]
    fn dump_previews() {
        use macroquad::color::Color;
        let out = std::env::var("DUMP_DIR").expect("set DUMP_DIR to a folder");
        for generator in Generator::ALL {
            for seed in [1u64, 2] {
                let s = MapSetup { generator, ..MapSetup::default() };
                let g = generate(seed, &s);
                let map = &g.map;
                let (n, scale) = (map.size, 2);
                let w = n * scale;
                let mut bytes = vec![0u8; w * w * 4];
                for y in 0..n {
                    for x in 0..n {
                        let (xi, yi) = (x as i32, y as i32);
                        let mut c = match map.deposit(xi, yi) {
                            Some(d) => crate::draw::ore_color(d.ore),
                            None => crate::draw::terrain_color(map.terrain(xi, yi)),
                        };
                        if !map.in_play(xi, yi) {
                            c = Color::new(c.r * 0.6, c.g * 0.6, c.b * 0.6, 1.0);
                        }
                        let spawner = g.spawners.iter().any(|sp| xi >= sp.min.0 && xi < sp.min.0 + sp.size.0 && yi >= sp.min.1 && yi < sp.min.1 + sp.size.1);
                        if spawner {
                            c = Color::new(0.85, 0.25, 0.55, 1.0);
                        }
                        let px = [(c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8, 255];
                        for dy in 0..scale {
                            for dx in 0..scale {
                                let i = ((y * scale + dy) * w + x * scale + dx) * 4;
                                bytes[i..i + 4].copy_from_slice(&px);
                            }
                        }
                    }
                }
                let image = macroquad::texture::Image { bytes, width: w as u16, height: w as u16 };
                image.export_png(&format!("{out}/{generator:?}_{seed}.png"));
            }
        }
    }
}
