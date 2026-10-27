//! TAB's map generator (DESIGN.md 9.1, `docs\tab-nachbau.md` step 6), rebuilt from its code
//! (`--zyl_NPjjlA7DRfVtsRJCX1kN4BxSr.cs`, `z_Ezgd90E`). TAB has one generator; its six landscapes (BR, TM, AL, DS, FA,
//! VO) are only other numbers (`Params`, the maps in `data\game_maps.json`) and other pictures. Its numbers lie in a
//! file that stays closed (`ZXRules.dat`) – ours are estimated.
//!
//! It's built of noise layers (`layer.rs`) – one number per tile, cut at thresholds:
//! 1. **Land:** noise, plus the ring outside the world border ("Rand") – above `land` it's land, else water. The ring is
//!    always land: dirt, nothing on it.
//! 2. **Mountains:** the same noise without the ring, where it's highest (`mountains`), in blocks of 2 × 2.
//! 3. **Grass and forest:** a second noise on the land, sharpened – grass above `grass`, forest on the grass above
//!    `forest_on_grass`, a third noise for forest on dirt above `forest_on_dirt`.
//! 4. **Command center** on the free tile nearest the middle with room around it; less than 18 forest within 11 tiles
//!    → everything again with a new seed. What can't be reached from it doesn't count as free – less than 30 % of the
//!    playing field reachable: a new seed too (ours, `MIN_REACHED`).
//! 5. **Villages of doom:** a circle cleared, a house in the middle, houses in a spiral around it.
//! 6. **Ruins with loot** (TAB's treasures): 2–4 piles each – gold, or a good that is worth more further out.
//! 7. **Stone, iron, coal, gold:** a fine noise on the free dirt, not too close to the command center – at least 4
//!    stone and 10 iron close to it, else rolled again. Coal isn't TAB's (our charcoal burners and foundries need it):
//!    a layer of its own made like iron, also at least 10 close by.
//! 8. **Oil:** one 2 × 2 node per block of `oil_block` tiles, with room around it.
//! 9. **Zombies:** a noise over the free land, outside `weak_from` – the higher, the more per tile; further out the
//!    stronger kinds (`medium_from` …). **Giants** far out (at least 70 tiles), 40 apart.
//!
//! Not taken over (yet): TAB's old towers and ravens, the supplies of food and energy, the mutants – we don't have
//! those. The decorations of the landscapes are only pictures. TAB's start units – ours come from the difficulty.
//!
//! TAB's counts (villages, ruins, giants) are for its 256 map: `TAB_AREA` tiles inside the border. On a bigger or
//! smaller map they grow and shrink with it. Only `+ - * /`, `sqrt`, `floor` – the same map on every PC.

use std::collections::VecDeque;

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use super::layer::{spiral, Layer, Rect};
use super::villages::HouseSpot;
use super::{spawner_spots, Common, Field, Generated, LootSpot, Show, ZombieSpot};
use crate::kinds::BuildingKind;
use crate::map::{Deposit, Map, Ore, Terrain, BORDER};
use crate::rng::Rng;
use crate::sim::{Breed, LootKind};

/// Tiles inside the border of TAB's 256 map (its playing field is a square on its screen, about 0.485 · 256²). Its
/// counts are for this many.
const TAB_AREA: f32 = 31_800.0;
/// At most this many new seeds (no room for the command center, too little forest or stone around it) – the last try
/// is taken as it is.
const RESEEDS: u32 = 40;
/// At least this share of the playing field must be reachable from the command center, else a new seed (ours).
const MIN_REACHED: f32 = 0.3;
/// Stone, iron (and our coal): this many rolls for enough of it near the command center (TAB) before a new seed.
const ORE_TRIES: u32 = 100;
/// Units of ore in every tile of stone, iron, coal, gold – TAB's never run out, ours do (`ore_richness` of the map).
const ORE_AMOUNT: f32 = 1500.0;
/// Units of oil in every tile of a node.
const OIL_AMOUNT: f32 = 2000.0;

/// TAB's numbers of a landscape (`ZXMapTheme`) – their names in TAB in the comments. The defaults: estimated for BR.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    /// How big the land and the lakes are: 0 big, 1 small (`LandDispersionFactor`) – the noise's scale is 0.03 ± 0.025.
    pub land_dispersion: f32,
    /// Detail of the coast (`LandOctaves`).
    pub land_octaves: f32,
    /// Share of land, the rest is water (`LandWaterFactor`).
    pub land: f32,
    /// Mountains on the highest land (`MountainLandFactor`).
    pub mountains: f32,
    /// Detail of the grass and the forests (`VegetationOctaves`).
    pub vegetation_octaves: f32,
    /// Grass on the land, the rest is dirt (`GrassEarthFactor`).
    pub grass: f32,
    /// Forest on the grass – times `grass` (`ForestGrassFactor`) – and on the dirt (`ForestEarthFactor`).
    pub forest_on_grass: f32,
    pub forest_on_dirt: f32,
    /// Villages of doom on a TAB map: from … to (`NumDoomVillages`), and how big each, radius in tiles
    /// (`DoomVillagesSize`) – over 8 the house in the middle is a big one, over 12 every 8th.
    pub villages_min: f32,
    pub villages_max: f32,
    pub village_size_min: f32,
    pub village_size_max: f32,
    /// Ruins with loot on a TAB map: from … to (`NumTreasures`).
    pub ruins_min: f32,
    pub ruins_max: f32,
    /// Stone, iron, coal, gold: how much (`StoneEarthFactor` …) and no closer to the command center than this
    /// (`StoneFormationMinDistance` …, tiles). Coal isn't TAB's – ours, made like iron.
    pub stone: f32,
    pub stone_from: f32,
    pub iron: f32,
    pub iron_from: f32,
    pub coal: f32,
    pub coal_from: f32,
    pub gold: f32,
    pub gold_from: f32,
    /// One oil node per block of this many tiles (`NCellsPerOilSource`).
    pub oil_block: f32,
    /// Zombies: how many (`InfectedFactor`, times the difficulty's population), in how big patches (`InfectedDispersion`
    /// – the noise's scale), and from how far out each kind (`MinDistanceFor{Weak,Medium,Strong,Powerful}Infected`).
    pub infected: f32,
    pub infected_dispersion: f32,
    pub weak_from: f32,
    pub medium_from: f32,
    pub strong_from: f32,
    pub powerful_from: f32,
    /// Giants on a TAB map at population 1 (`NumInfectedGiants`).
    pub giants: f32,
    /// How big the batches of the houses of doom are (`InfectionNestsFactor`, `sim/doom.rs`) – the generator doesn't
    /// use it, the world gives it to every house.
    pub nests: f32,
    /// TAB's zombie population (`FactorZombiePopulation`): the difficulty sets it (`World::generate`), no slider.
    pub population: f32,
}

impl Default for Params {
    /// BR's numbers, estimated – about 8 % water, 6 % mountains, 30 % forest and a third each of grass and dirt
    /// (measured on TAB's noise, `noise_quantiles`).
    fn default() -> Params {
        Params {
            land_dispersion: 0.5,
            land_octaves: 3.0,
            land: 0.7,
            mountains: 0.27,
            vegetation_octaves: 3.0,
            grass: 0.53,
            forest_on_grass: 0.43,
            forest_on_dirt: 0.34,
            villages_min: 3.0,
            villages_max: 5.0,
            village_size_min: 6.0,
            village_size_max: 12.0,
            ruins_min: 8.0,
            ruins_max: 12.0,
            stone: 0.13,
            stone_from: 6.0,
            iron: 0.11,
            iron_from: 15.0,
            coal: 0.11,
            coal_from: 15.0,
            gold: 0.075,
            gold_from: 30.0,
            oil_block: 48.0,
            infected: 0.65,
            infected_dispersion: 0.08,
            weak_from: 10.0,
            medium_from: 25.0,
            strong_from: 45.0,
            powerful_from: 65.0,
            giants: 2.0,
            nests: 1.0,
            population: 1.0,
        }
    }
}

impl Params {
    /// The sliders of the setup page – the rest only in `data\game_maps.json` (more don't fit).
    pub fn fields(&mut self) -> Vec<Field<'_>> {
        let share = |label, value, help| Field { label, value, min: 0.0, max: 1.0, step: 0.01, show: Show::Percent, help };
        let count = |label, value, max, help| Field { label, value, min: 0.0, max, step: 1.0, show: Show::Number, help };
        let tiles = |label, value, max, help| Field { label, value, min: 0.0, max, step: 1.0, show: Show::Tiles, help };
        vec![
            share("Land", &mut self.land, "TAB's LandWaterFactor: the higher, the more land - the rest is lakes."),
            share("Mountains", &mut self.mountains, "TAB's MountainLandFactor: mountains on the highest land."),
            share("Grass", &mut self.grass, "TAB's GrassEarthFactor: grass on the land, the rest is dirt (no farm grows there)."),
            share("Forest on grass", &mut self.forest_on_grass, "TAB's ForestGrassFactor: forest on the grass."),
            share("Forest on dirt", &mut self.forest_on_dirt, "TAB's ForestEarthFactor: forest on the dirt."),
            count("Villages from", &mut self.villages_min, 20.0, "TAB's NumDoomVillages: at least this many villages of doom on a TAB map - more on a bigger one."),
            count("Villages to", &mut self.villages_max, 20.0, "At most this many villages of doom on a TAB map."),
            tiles("Village size from", &mut self.village_size_min, 30.0, "TAB's DoomVillagesSize: a village's radius - over 8 it has a big house in the middle."),
            tiles("Village size to", &mut self.village_size_max, 30.0, "The biggest village's radius - over 12 every 8th house is a big one."),
            count("Ruins from", &mut self.ruins_min, 40.0, "TAB's NumTreasures: ruins with 2 to 4 piles of loot each, near the base - on a TAB map."),
            count("Ruins to", &mut self.ruins_max, 40.0, "At most this many ruins on a TAB map."),
            share("Stone", &mut self.stone, "TAB's StoneEarthFactor: stone on the free dirt. At least 4 tiles close to the base."),
            share("Iron", &mut self.iron, "TAB's IronEarthFactor: iron on the free dirt. At least 10 tiles within 25 of the base."),
            share("Coal", &mut self.coal, "Ours, not TAB's: coal on the free dirt, made like iron. At least 10 tiles within 25 of the base."),
            share("Gold", &mut self.gold, "TAB's GoldEarthFactor: gold on the free dirt - a mine on it digs gold, 20 times as much as stone."),
            share("Zombies", &mut self.infected, "TAB's InfectedFactor: how much of the land has zombies - times the difficulty's population."),
            Field { label: "Zombie patches", value: &mut self.infected_dispersion, min: 0.01, max: 0.3, step: 0.01, show: Show::Number, help: "TAB's InfectedDispersion: small = big patches of zombies, big = many small ones." },
            tiles("Medium from", &mut self.medium_from, 120.0, "Runners and riots only this far from the base and further (TAB's MinDistanceForMediumInfected)."),
            tiles("Strong from", &mut self.strong_from, 120.0, "Riots alone in a tile from here (MinDistanceForStrongInfected)."),
            tiles("Powerful from", &mut self.powerful_from, 120.0, "Fat ones, harpies and spitters from here (MinDistanceForPowerfulInfected)."),
            count("Giants", &mut self.giants, 10.0, "TAB's NumInfectedGiants on a TAB map at normal population - 0 below half, twice from 1.2, three times from 1.4."),
            Field { label: "House batches", value: &mut self.nests, min: 0.0, max: 3.0, step: 0.1, show: Show::Number, help: "TAB's InfectionNestsFactor: how many zombies come out of a house of doom at once - times its size and the population." },
        ]
    }
}

/// Makes the map – TAB's steps, see the top of the file. `size`: the edge of the whole grid (`MapSetup::map_size`).
/// Also the middle of every village of doom.
pub fn generate(seed: u64, size: usize, common: &Common, p: &Params) -> (Generated, Vec<Vec2>) {
    let mut rng = Rng::new(seed);
    for attempt in 0..RESEEDS {
        if let Some(generated) = attempt_map(&mut rng, size, common, p, attempt + 1 < RESEEDS) {
            return generated;
        }
    }
    unreachable!("the last try never gives up")
}

/// One try. `strict`: give up (`None`) where TAB takes a new seed – the last try keeps going instead.
fn attempt_map(rng: &mut Rng, size: usize, common: &Common, p: &Params, strict: bool) -> Option<(Generated, Vec<Vec2>)> {
    let n = size as i32;
    let play = Map::play_area_of(size, BORDER);
    let full = Map::full_area_of(size);
    // TAB's counts are for its 256 map – here as many per tile.
    let scale = play.area() / TAB_AREA;
    let scaled = |count: i32| (count as f32 * scale).round() as i32;
    // Where in the endless noise this map lies: TAB's three random offsets, the two along x and y moved on after every
    // layer so the layers don't repeat each other.
    let (mut dx, mut dy, z) = (unit_f64(rng) * 1000.0, unit_f64(rng) * 1000.0, unit_f64(rng) * 1000.0);
    let mut next_offsets = |rng: &mut Rng| {
        let (x, y) = (dx, dy);
        dx += unit_f64(rng) * 1000.0;
        dy += unit_f64(rng) * 1000.0;
        (x, y)
    };

    // 1. The ring outside the border ("Rand"), and the same grown by 10 and by 4 tiles.
    let mut rand = Layer::new(n);
    for x in 0..n {
        for y in 0..n {
            if !play.contains_tile(x, y) {
                rand.set(x, y, 1.0);
            }
        }
    }
    let mut rand10 = rand.clone();
    rand10.enlarge(10, 1.0);
    let mut rand4 = rand.clone();
    rand4.enlarge(4, 1.0);

    // 2. Land: noise plus the ring, cut at `land`. Mountains: the same noise without the ring, in blocks of 2.
    let dispersion = 0.03 + 0.025 * (p.land_dispersion as f64 * 2.0 - 1.0);
    let mut noise = Layer::new(n);
    let (ox, oy) = next_offsets(rng);
    noise.fill_noise(z, dispersion, dispersion, ox, oy, p.land_octaves as u32);
    noise.add(&rand);
    let mut land = noise.clone();
    land.truncate(p.land as f64, 1);
    let mut mountains = noise.clone();
    mountains.subtract(&rand);
    mountains.truncate(p.mountains as f64, 2);

    // 3. Grass and forest.
    let mut vegetation = Layer::new(n);
    let (ox, oy) = next_offsets(rng);
    vegetation.fill_noise(z, 0.03, 0.03, ox, oy, p.vegetation_octaves as u32);
    vegetation.subtract(&rand);
    vegetation.multiply(&land);
    vegetation.contrast(2.0);
    let mut grass = vegetation.clone();
    grass.truncate(p.grass as f64, 1);
    let mut trees = vegetation.clone();
    trees.subtract(&rand);
    trees.subtract(&mountains);
    trees.truncate((p.forest_on_grass * p.grass) as f64, 1);
    let mut dirt_forest = Layer::new(n);
    let (ox, oy) = next_offsets(rng);
    dirt_forest.fill_noise(z, 0.03, 0.03, ox, oy, 2);
    dirt_forest.multiply(&land);
    dirt_forest.subtract(&grass);
    dirt_forest.subtract(&rand);
    dirt_forest.subtract(&mountains);
    dirt_forest.truncate(p.forest_on_dirt as f64, 1);
    trees.add(&dirt_forest);
    // The ground under it all: water, dirt, grass (TAB's terrain layer – the forests and mountains are objects on it).
    let mut ground: Vec<Terrain> = (0..n * n)
        .map(|i| {
            let (x, y) = (i % n, i / n);
            if land.get(x, y) < 0.5 {
                Terrain::Water
            } else if grass.get(x, y) >= 0.5 {
                Terrain::Grass
            } else {
                Terrain::Dirt
            }
        })
        .collect();
    // Free: land without forest, mountains and the ring.
    let mut free = land.clone();
    free.subtract(&trees);
    free.subtract(&mountains);
    free.subtract(&rand);
    free.truncate(0.5, 1);

    // 4. The command center: the free spot nearest the middle with 2 tiles of room around it, forest close by.
    let (w, h) = BuildingKind::CommandCenter.footprint(false);
    let middle = n / 2;
    let base = match free.area_near(middle, middle, w + 4, h + 4, 1.0) {
        Some((l, t, aw, ah)) => (l + aw / 2, t + ah / 2),
        None if strict => return None,
        None => (middle, middle),
    };
    let cc_area: Rect = (base.0 - w / 2, base.1 - h / 2, w, h);
    let near_cc = (cc_area.0 - 11, cc_area.1 - 11, w + 22, h + 22);
    if strict && trees.total(near_cc) < 18.0 {
        return None;
    }
    // What can be reached from it on foot is free – the rest not.
    let reached = flood(&free, cc_area);
    // Ours, not TAB's: a command center shut in by water and forest leaves nearly nothing for the rest – a new seed.
    // (Our estimated numbers of the wet landscapes, FA and AL, do that about once in 10 maps.)
    if strict && (reached.iter().filter(|&&r| r).count() as f32) < MIN_REACHED * play.area() {
        return None;
    }
    for x in 0..n {
        for y in 0..n {
            if !reached[(y * n + x) as usize] {
                free.set(x, y, 0.0);
            }
        }
    }
    free.set_rect(cc_area, 0.0);
    let mut reached = reached;
    let base_point = vec2(base.0 as f32, base.1 as f32);
    let dist = |x: i32, y: i32| vec2(x as f32, y as f32).distance(base_point);

    // 5. Villages of doom.
    let mut houses: Vec<HouseSpot> = Vec::new();
    // Where a house stands already – a village cleared next to another one doesn't free those tiles again (TAB does,
    // and then two houses can stand on top of each other).
    let mut built = vec![false; (n * n) as usize];
    let mut villages: Vec<(i32, i32)> = Vec::new();
    let mut count = scaled(range(rng, p.villages_min as i32, p.villages_max as i32 + 1));
    if p.population < 0.5 {
        count /= 2;
    }
    for _ in 0..count {
        let r = range(rng, p.village_size_min as i32, p.village_size_max as i32 + 1).max(1);
        let near = (16.0 + r as f32 * 1.4) as i32;
        let far = n / 2 - 60 - r;
        let spot = (0..10_000).map(|_| (range(rng, r, n - r), range(rng, r, n - r))).find(|&(x, y)| {
            let axis = (x - base.0).abs().max((y - base.1).abs());
            // Not right at the border either (ours, not TAB's): the house in the middle would stand over it.
            reached[(y * n + x) as usize]
                && rand4.get(x, y) == 0.0
                && axis >= near
                && axis <= far
                && villages.iter().all(|&(vx, vy)| vec2((vx - x) as f32, (vy - y) as f32).length() >= 20.0)
        });
        let Some(c) = spot else {
            continue;
        };
        // The circle cleared: water becomes dirt, forest and mountains go.
        for x in (c.0 - r).max(0)..=(c.0 + r).min(n - 1) {
            for y in (c.1 - r).max(0)..=(c.1 + r).min(n - 1) {
                if (x - c.0) * (x - c.0) + (y - c.1) * (y - c.1) < r * r {
                    let i = (y * n + x) as usize;
                    if ground[i] == Terrain::Water {
                        ground[i] = Terrain::Dirt;
                    }
                    trees.set(x, y, 0.0);
                    mountains.set(x, y, 0.0);
                    if !built[i] {
                        free.set(x, y, 1.0);
                    }
                    reached[i] = true;
                }
            }
        }
        villages.push(c);
        // The house in the middle – unless a house of the village next to it stands there.
        let middle_house = house_at(c, if r > 8 { 5 } else { 3 });
        let (l, t, w, h) = rect_of(&middle_house);
        if (l..l + w).all(|x| (t..t + h).all(|y| !built[(y * n + x) as usize])) {
            build(middle_house, &mut built, &mut free, &mut houses);
        }
        // Around it in a spiral: every fifth spot left out; over 12 every 8th house big, else 2 of 3 small.
        let mut k = 0;
        for (x, y) in spiral(c.0, c.1, false) {
            if (x - c.0).abs() > r {
                break;
            }
            if !rand4.inside(x, y) || rand4.get(x, y) == 1.0 || (x - c.0) * (x - c.0) + (y - c.1) * (y - c.1) > r * r {
                continue;
            }
            if rng.below(5) > 3 {
                continue;
            }
            let side = if r > 12 && k % 8 == 0 {
                5
            } else if k % 3 > 0 {
                2
            } else {
                3
            };
            let house = house_at((x, y), side);
            let (l, t, hw, hh) = rect_of(&house);
            if free.min((l - 1, t - 1, hw + 2, hh + 2)) == 0.0 {
                continue;
            }
            k += 1;
            build(house, &mut built, &mut free, &mut houses);
        }
    }

    // 6. Ruins with loot: on 4 × 4 free dirt, 8 to 20 + n / 5 tiles from the command center, 20 apart.
    let reach = 20 + n / 5;
    let mut spots = Vec::new();
    for x in 20..n - 20 {
        for y in 20..n - 20 {
            let fits = (x..x + 4).all(|bx| {
                (y..y + 4).all(|by| {
                    let d2 = (bx - base.0) * (bx - base.0) + (by - base.1) * (by - base.1);
                    free.get(bx, by) != 0.0
                        && rand10.get(bx, by) != 1.0
                        && ground[(by * n + bx) as usize] == Terrain::Dirt
                        && d2 > 64
                        && d2 <= reach * reach
                })
            });
            if fits {
                spots.push((x, y));
            }
        }
    }
    let mut ruins = Layer::new(n);
    let mut taken: Vec<(i32, i32)> = Vec::new();
    let mut loot = Vec::new();
    let ruin_count = scaled(range(rng, p.ruins_min as i32, p.ruins_max as i32 + 1));
    for _ in 0..ruin_count {
        if spots.is_empty() {
            break;
        }
        for _ in 0..1000 {
            let at = spots[rng.below(spots.len() as u32) as usize];
            if taken.iter().any(|&(tx, ty)| (tx - at.0) * (tx - at.0) + (ty - at.1) * (ty - at.1) <= 400) {
                continue;
            }
            taken.push(at);
            ruins.set_rect((at.0, at.1, 4, 4), 1.0);
            // 2 to 4 piles: gold in 35 of 100, else a good – the further out and the fewer piles, the better.
            let piles = range(rng, 2, 5);
            let mut used: Vec<(i32, i32)> = Vec::new();
            for _ in 0..piles {
                let haul = if rng.below(100) < 35 {
                    (LootKind::Gold, 500)
                } else {
                    let roll = rng.below(100) as i32 + dist(at.0, at.1) as i32 - piles * 15;
                    let kind = match roll {
                        r if r > 90 => LootKind::Gold,
                        r if r > 69 => LootKind::Coal,
                        r if r > 55 => LootKind::Iron,
                        r if r > 27 => LootKind::Stone,
                        _ => LootKind::Wood,
                    };
                    (kind, if kind == LootKind::Gold { 500 } else { 10 })
                };
                let tile = loop {
                    let tile = (at.0 + rng.below(4) as i32, at.1 + rng.below(4) as i32);
                    if !used.contains(&tile) {
                        break tile;
                    }
                };
                used.push(tile);
                if free.get(tile.0, tile.1) >= 1.0 {
                    loot.push(LootSpot { tile, haul: Some(haul) });
                }
            }
            break;
        }
    }

    // 7. Stone, iron, coal, gold – on free dirt, away from the ring, the grass, the ruins and the command center.
    let mut mineral = |rng: &mut Rng, share: f32, from: f32, gold: bool| {
        let mut layer = Layer::new(n);
        let (ox, oy) = next_offsets(rng);
        layer.fill_noise(z, 0.07, 0.07, ox, oy, 0);
        layer.subtract(&rand10);
        layer.multiply(&free);
        layer.subtract(&grass);
        if !gold {
            layer.subtract(&ruins);
        }
        layer.subtract_circle(base.0, base.1, from as i32);
        if gold {
            layer.delete_alone_cells(share as f64);
            layer.subtract(&ruins);
        }
        layer.truncate(share as f64, 1);
        layer.multiply(&free);
        layer
    };
    let mut enough = |rng: &mut Rng, share: f32, from: f32, square: i32, least: f64| {
        for _ in 0..ORE_TRIES {
            let layer = mineral(rng, share, from, false);
            if layer.total((base.0 - square / 2, base.1 - square / 2, square, square)) >= least {
                return Some(layer);
            }
        }
        if strict { None } else { Some(mineral(rng, share, from, false)) }
    };
    let stone = enough(rng, p.stone, p.stone_from, 20, 4.0)?;
    let iron = enough(rng, p.iron, p.iron_from, 50, 10.0)?;
    // Ours: coal like iron, so the charcoal burners' and foundries' mine finds some close by too – but it never costs a
    // new seed (TAB's own checks stay as they are): the best of the rolls if none has 10 close by. Not on stone or iron:
    // it would take their tiles.
    let near = (base.0 - 25, base.1 - 25, 50, 50);
    let mut roll_coal = |rng: &mut Rng| {
        let mut layer = mineral(rng, p.coal, p.coal_from, false);
        layer.subtract(&stone);
        layer.subtract(&iron);
        layer
    };
    let mut coal = roll_coal(rng);
    for _ in 1..ORE_TRIES {
        if coal.total(near) >= 10.0 {
            break;
        }
        let other = roll_coal(rng);
        if other.total(near) > coal.total(near) {
            coal = other;
        }
    }
    let gold = mineral(rng, p.gold, p.gold_from, true);

    // 8. Oil: one node per block, 6 × 6 around it free of everything.
    let mut oil = Vec::new();
    let block = (p.oil_block as i32).max(9);
    for bx in 0..n / block {
        for by in 0..n / block {
            for _ in 0..500 {
                let (x, y) = (bx * block + range(rng, 4, block - 4), by * block + range(rng, 4, block - 4));
                let clear = (x - 2..x + 4).all(|cx| {
                    (y - 2..y + 4).all(|cy| {
                        free.inside(cx, cy)
                            && free.get(cx, cy) == 1.0
                            && rand10.get(cx, cy) == 0.0
                            && grass.get(cx, cy) == 0.0
                            && trees.get(cx, cy) == 0.0
                            && mountains.get(cx, cy) == 0.0
                            && stone.get(cx, cy) == 0.0
                            && iron.get(cx, cy) == 0.0
                            && coal.get(cx, cy) == 0.0
                            && gold.get(cx, cy) == 0.0
                            && land.get(cx, cy) == 1.0
                            && ruins.get(cx, cy) != 1.0
                    })
                });
                if clear {
                    oil.push((x, y));
                    free.set_rect((x, y, 2, 2), 0.0);
                    break;
                }
            }
        }
    }

    // 9. Zombies: the higher the noise above the threshold, the more on a tile; the further out, the stronger.
    let mut infected = Layer::new(n);
    let (ox, oy) = next_offsets(rng);
    let d = p.infected_dispersion as f64;
    infected.fill_noise(z, d, d, ox, oy, 0);
    infected.subtract(&rand);
    infected.subtract(&rand4);
    infected.multiply(&free);
    infected.subtract_circle(base.0, base.1, p.weak_from as i32);
    let threshold = 1.0 - (p.infected * p.population) as f64;
    let mut zombies = Vec::new();
    for x in 0..n {
        for y in 0..n {
            let v = infected.get(x, y);
            if v < threshold || v <= 0.0 {
                continue;
            }
            let level = (4.0 * (v - threshold) / (1.0 - threshold)).round() as i32;
            let far = dist(x, y);
            let roll = rng.below(100);
            let (kinds, many): (&[Breed], i32) = if roll >= 50 && far >= p.medium_from {
                if roll >= 60 && far >= p.powerful_from {
                    (&[Breed::Fat, Breed::Harpy, Breed::Spitter], if level <= 2 { 1 } else { 2 })
                } else if far >= p.strong_from {
                    (&[Breed::Armored], level_count(level))
                } else {
                    (&[Breed::Runner, Breed::Runner, Breed::Runner, Breed::Runner, Breed::Armored], level_count(level))
                }
            } else if roll >= 30 {
                (&[Breed::Walker], level_count(level))
            } else {
                continue;
            };
            for _ in 0..many {
                let breed = kinds[rng.below(kinds.len() as u32) as usize];
                let offset = vec2(rng.unit_f32() * 2.0 - 1.0, rng.unit_f32() * 2.0 - 1.0) * 0.4;
                zombies.push(ZombieSpot { pos: vec2(x as f32 + 0.5, y as f32 + 0.5) + offset, breed });
            }
        }
    }
    // Giants: on zombie land, at least 70 tiles out, 40 apart – none below half the population, more above 1.
    let per_giants = match p.population {
        q if q < 0.5 => 0.0,
        q if q < 1.0 => 0.5,
        q if q < 1.2 => 1.0,
        q if q < 1.4 => 2.0,
        _ => 3.0,
    };
    let mut giants: Vec<Vec2> = Vec::new();
    for _ in 0..scaled((p.giants * per_giants).floor() as i32) {
        for _ in 0..10_000 {
            let (x, y) = (rng.below(n as u32) as i32, rng.below(n as u32) as i32);
            let at = vec2(x as f32 + 0.5, y as f32 + 0.5);
            if infected.get(x, y) > 0.0 && dist(x, y) >= 70.0 && giants.iter().all(|g| g.distance(at) > 40.0) {
                giants.push(at);
                break;
            }
        }
    }
    zombies.extend(giants.into_iter().map(|pos| ZombieSpot { pos, breed: Breed::Giant }));

    // The map: mountains and forest on the ground, the grid's corners beyond the map mountain like on every map.
    let terrain: Vec<Terrain> = (0..n * n)
        .map(|i| {
            let (x, y) = (i % n, i / n);
            if !full.contains_tile(x, y) || mountains.get(x, y) >= 0.5 {
                Terrain::Mountain
            } else if trees.get(x, y) >= 0.5 {
                Terrain::Forest
            } else {
                ground[i as usize]
            }
        })
        .collect();
    let mut map = Map::from_terrain(size, BORDER, terrain);
    let amount = (ORE_AMOUNT * common.ore_richness).round().max(1.0) as u32;
    for (layer, ore) in [(&stone, Ore::Stone), (&iron, Ore::Iron), (&coal, Ore::Coal), (&gold, Ore::Gold)] {
        for x in 0..n {
            for y in 0..n {
                if layer.get(x, y) >= 0.5 && !map.is_solid(x, y) {
                    map.set_deposit(x, y, Some(Deposit { ore, amount }));
                }
            }
        }
    }
    let oil_amount = (OIL_AMOUNT * common.ore_richness).round().max(1.0) as u32;
    for (x, y) in oil {
        for (ox, oy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            map.set_deposit(x + ox, y + oy, Some(Deposit { ore: Ore::Oil, amount: oil_amount }));
        }
    }
    let spawners = spawner_spots(size, common.spawners_per_side.round() as i32);
    let middles = villages.iter().map(|&(x, y)| vec2(x as f32 + 0.5, y as f32 + 0.5)).collect();
    Some((Generated { map, spawners, houses, loot, base, zombies: Some(zombies) }, middles))
}

/// How many on a tile by its level (TAB: 1, 2, and 3 for everything else – a 0 too, TAB's quirk).
fn level_count(level: i32) -> i32 {
    match level {
        1 => 1,
        2 => 2,
        _ => 3,
    }
}

/// Puts `house` on the map: its tiles are built on and not free anymore.
fn build(house: HouseSpot, built: &mut [bool], free: &mut Layer, houses: &mut Vec<HouseSpot>) {
    let (l, t, w, h) = rect_of(&house);
    for (x, y) in (l..l + w).flat_map(|x| (t..t + h).map(move |y| (x, y))) {
        built[(y * free.n + x) as usize] = true;
    }
    free.set_rect((l, t, w, h), 0.0);
    houses.push(house);
}

/// A house of `side` × `side` tiles around the tile `at` (TAB's middle tile – for an even side the one right below the
/// middle).
fn house_at(at: (i32, i32), side: i32) -> HouseSpot {
    HouseSpot { tile: (at.0 - (side - 1) / 2, at.1 - (side - 1) / 2), size: (side, side) }
}

fn rect_of(house: &HouseSpot) -> Rect {
    (house.tile.0, house.tile.1, house.size.0, house.size.1)
}

/// Every tile that can be reached on foot from around `area` over `free` tiles (straight steps, TAB's way) – one
/// entry per tile, row by row.
fn flood(free: &Layer, (l, t, w, h): Rect) -> Vec<bool> {
    let n = free.n;
    let mut seen = vec![false; (n * n) as usize];
    let mut reached = vec![false; (n * n) as usize];
    let mut queue = VecDeque::new();
    // The ring of tiles around the area, its corners too.
    for x in l - 1..=l + w {
        for y in t - 1..=t + h {
            let on_ring = x == l - 1 || x == l + w || y == t - 1 || y == t + h;
            if on_ring && free.inside(x, y) {
                queue.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        seen[(y * n + x) as usize] = true;
        if free.get(x, y) < 1.0 {
            continue;
        }
        reached[(y * n + x) as usize] = true;
        for (nx, ny) in [(x - 1, y), (x, y - 1), (x + 1, y), (x, y + 1)] {
            if nx > 0 && ny > 0 && nx < n - 1 && ny < n - 1 && !seen[(ny * n + nx) as usize] {
                seen[(ny * n + nx) as usize] = true;
                queue.push_back((nx, ny));
            }
        }
    }
    reached
}

/// A whole number from `lo` up to (not including) `hi` – .NET's `Random.Next(lo, hi)`, which TAB uses. Nothing to
/// pick: `lo`.
fn range(rng: &mut Rng, lo: i32, hi: i32) -> i32 {
    if hi <= lo { lo } else { lo + rng.below((hi - lo) as u32) as i32 }
}

/// A number from 0 to just under 1 with all the bits a 64-bit float has – for the noise offsets.
fn unit_f64(rng: &mut Rng) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapgen::{Generator, MapSetup};

    /// The maps of `data\game_maps.json` made with TAB's generator.
    fn tab_maps() -> Vec<(String, MapSetup)> {
        let maps = crate::data::get().maps.maps.iter().filter(|m| m.setup.generator == Generator::Tab);
        maps.map(|m| (m.name.clone(), m.setup.clone())).collect()
    }

    fn make(seed: u64, setup: &MapSetup) -> (Generated, Vec<Vec2>) {
        generate(seed, setup.map_size(), &setup.common, &setup.tab)
    }

    /// How many tiles of the rectangle have this ore.
    fn ore_in(map: &Map, (l, t, w, h): Rect, ore: Ore) -> usize {
        (l..l + w).flat_map(|x| (t..t + h).map(move |y| (x, y))).filter(|&(x, y)| map.deposit(x, y).is_some_and(|d| d.ore == ore)).count()
    }

    /// Every landscape, three seeds: what TAB's generator promises. The command center stands on free ground with
    /// forest around it (18 tiles within 11), stone (4 in the 20 × 20 around it) and iron (10 in the 50 × 50). Houses
    /// and loot on open ground inside the border, not on top of each other. Oil in lone nodes of 2 × 2 on dirt. The
    /// zombies only as close as their kind may come, the giants far out and apart – and plenty of them: the base isn't
    /// shut in (`MIN_REACHED`, FA's seed 3 was).
    #[test]
    fn every_landscape_keeps_tabs_promises() {
        for (name, setup) in tab_maps() {
            let p = &setup.tab;
            for seed in 1..=3 {
                let (g, villages) = make(seed, &setup);
                let map = &g.map;
                let (bx, by) = g.base;
                let (w, h) = BuildingKind::CommandCenter.footprint(false);
                let cc = (bx - w / 2, by - h / 2, w, h);
                let tiles = |(l, t, w, h): Rect| (l..l + w).flat_map(move |x| (t..t + h).map(move |y| (x, y)));
                for (x, y) in tiles(cc) {
                    assert!(map.in_play(x, y) && !map.is_solid(x, y), "{name} {seed}: the command center on ({x}, {y})");
                }
                let forest = tiles((cc.0 - 11, cc.1 - 11, w + 22, h + 22)).filter(|&(x, y)| map.terrain(x, y) == Terrain::Forest).count();
                assert!(forest >= 18, "{name} {seed}: only {forest} forest around the command center");
                let stone = ore_in(map, (bx - 10, by - 10, 20, 20), Ore::Stone);
                let iron = ore_in(map, (bx - 25, by - 25, 50, 50), Ore::Iron);
                // Coal (ours) like iron – it never costs a new seed, so a few may fall short – and gold somewhere.
                let coal = ore_in(map, (bx - 25, by - 25, 50, 50), Ore::Coal);
                assert!(stone >= 4 && iron >= 10 && coal >= 4, "{name} {seed}: {stone} stone, {iron} iron, {coal} coal near the command center");
                assert!(ore_in(map, (0, 0, map.size as i32, map.size as i32), Ore::Gold) > 0, "{name} {seed}: no gold");
                assert!(!villages.is_empty() && !g.houses.is_empty(), "{name} {seed}: no village");

                let mut houses = vec![false; map.size * map.size];
                for (x, y) in tiles(cc) {
                    houses[y as usize * map.size + x as usize] = true;
                }
                for house in &g.houses {
                    for (x, y) in tiles((house.tile.0, house.tile.1, house.size.0, house.size.1)) {
                        assert!(map.in_play(x, y) && !map.is_solid(x, y) && map.deposit(x, y).is_none(), "{name} {seed}: a house {:?} on ({x}, {y}): in play {}, {:?}, {:?}", house, map.in_play(x, y), map.terrain(x, y), map.deposit(x, y));
                        let i = y as usize * map.size + x as usize;
                        assert!(!houses[i], "{name} {seed}: two houses on ({x}, {y})");
                        houses[i] = true;
                    }
                }
                for spot in &g.loot {
                    let (x, y) = spot.tile;
                    assert!(map.in_play(x, y) && !map.is_solid(x, y) && !houses[y as usize * map.size + x as usize], "{name} {seed}: loot on ({x}, {y})");
                    assert!(matches!(spot.haul, Some((LootKind::Gold, 500)) | Some((_, 10))), "{name} {seed}: loot {:?}", spot.haul);
                }

                let oil = |x: i32, y: i32| map.deposit(x, y).is_some_and(|d| d.ore == Ore::Oil);
                let n = map.size as i32;
                for (x, y) in (0..n).flat_map(|x| (0..n).map(move |y| (x, y))).filter(|&(x, y)| oil(x, y) && !oil(x - 1, y) && !oil(x, y - 1)) {
                    for (dx, dy) in (-1..3).flat_map(|dx| (-1..3).map(move |dy| (dx, dy))) {
                        let inside = (0..2).contains(&dx) && (0..2).contains(&dy);
                        assert_eq!(oil(x + dx, y + dy), inside, "{name} {seed}: the oil at ({x}, {y}) isn't a lone 2 x 2 node");
                    }
                    assert_eq!(map.terrain(x, y), Terrain::Dirt, "{name} {seed}: oil on {:?}", map.terrain(x, y));
                }

                let zombies = g.zombies.as_ref().expect("TAB's generator lays out the zombies");
                assert!(zombies.len() > 10_000, "{name} {seed}: only {} zombies - the base shut in?", zombies.len());
                let base = vec2(bx as f32, by as f32);
                let mut giants = Vec::new();
                for z in zombies {
                    let (x, y) = (z.pos.x.floor() as i32, z.pos.y.floor() as i32);
                    assert!(map.in_play(x, y) && !map.is_solid(x, y), "{name} {seed}: a zombie on ({x}, {y})");
                    let d = vec2(x as f32, y as f32).distance(base);
                    let from = match z.breed {
                        Breed::Runner | Breed::Armored => p.medium_from,
                        Breed::Fat | Breed::Harpy | Breed::Spitter => p.powerful_from,
                        Breed::Giant => 70.0,
                        _ => 0.0,
                    };
                    assert!(d >= from, "{name} {seed}: a {:?} {d} tiles from the base", z.breed);
                    if z.breed == Breed::Giant {
                        giants.push(z.pos);
                    }
                }
                for (i, a) in giants.iter().enumerate() {
                    assert!(giants[i + 1..].iter().all(|b| a.distance(*b) > 40.0), "{name} {seed}: two giants close together");
                }
            }
        }
    }

    /// The same seed and values make the same map and the same zombies; another seed another map.
    #[test]
    fn tab_maps_are_repeatable() {
        let setup = MapSetup { common: Common { size: 160.0, ..Common::default() }, ..MapSetup::default() };
        let (a, b, c) = (make(4, &setup).0, make(4, &setup).0, make(5, &setup).0);
        assert!(a.map.same_terrain(&b.map) && a.zombies == b.zombies && a.houses == b.houses && a.loot == b.loot);
        assert!(!a.map.same_terrain(&c.map));
    }

    /// The difficulty's zombie population (TAB's `FactorZombiePopulation`): more zombies the higher it is, the land
    /// the same – below 0.5 half the villages and no giants, from 1.2 twice the giants.
    #[test]
    fn the_population_brings_zombies_and_giants() {
        let with = |population: f32| {
            let mut setup = MapSetup::default();
            setup.tab.population = population;
            make(2, &setup)
        };
        let ((low, low_villages), (normal, normal_villages), (high, _)) = (with(0.4), with(1.0), with(1.2));
        let count = |g: &Generated, giant: bool| g.zombies.as_ref().unwrap().iter().filter(|z| (z.breed == Breed::Giant) == giant).count();
        assert!(count(&low, false) < count(&normal, false) && count(&normal, false) < count(&high, false), "zombies don't grow with the population");
        assert_eq!(count(&low, true), 0, "giants below half the population");
        assert_eq!(count(&high, true), 2 * count(&normal, true), "giants from 1.2");
        assert!(low_villages.len() < normal_villages.len(), "{} villages vs. {}", low_villages.len(), normal_villages.len());
        assert!(normal.map.same_terrain(&high.map), "the population changed the land");
    }

    /// Not a real test – a tool for estimating a landscape's numbers: prints for every map of TAB's generator
    /// what share of the playing field is what, and how many villages, houses, piles of loot, zombies of which kind.
    /// `cargo test --release tab_numbers -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn tab_numbers() {
        for (name, setup) in tab_maps() {
            for seed in 1..=3 {
                let (g, villages) = make(seed, &setup);
                let map = &g.map;
                let n = map.size as i32;
                let mut shares = std::collections::BTreeMap::new();
                let mut inside = 0;
                for (x, y) in (0..n).flat_map(|x| (0..n).map(move |y| (x, y))).filter(|&(x, y)| map.in_play(x, y)) {
                    inside += 1;
                    *shares.entry(map.terrain(x, y).name().to_string()).or_insert(0) += 1;
                    if let Some(d) = map.deposit(x, y) {
                        *shares.entry(d.ore.name().to_string()).or_insert(0) += 1;
                    }
                }
                let mut kinds = std::collections::BTreeMap::new();
                for z in g.zombies.as_ref().unwrap() {
                    *kinds.entry(z.breed.name()).or_insert(0) += 1;
                }
                let shares: Vec<String> = shares.iter().map(|(k, v)| format!("{k} {:.1} %", *v as f32 * 100.0 / inside as f32)).collect();
                println!("{name} {seed}: {}", shares.join(", "));
                println!("  {} villages, {} houses, {} loot, zombies {kinds:?}", villages.len(), g.houses.len(), g.loot.len());
            }
        }
    }

    /// Not a real test – a tool: how TAB's noise spreads its values (the shares of `Params` are thresholds on it).
    /// `cargo test --release noise_quantiles -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn noise_quantiles() {
        for (name, scale, octaves) in [("land, 3 octaves", 0.03, 3), ("vegetation, 2 octaves", 0.03, 2), ("ore, zombies", 0.07, 0)] {
            let mut values: Vec<f32> = (0..400 * 400).map(|i| crate::noise::simplex_octaves(100.0 + scale * (i % 400) as f64, 300.0 + scale * (i / 400) as f64, 500.0, octaves)).collect();
            values.sort_by(|a, b| a.total_cmp(b));
            let at = |share: f32| values[((values.len() - 1) as f32 * share) as usize];
            let line: Vec<String> = [0.05, 0.1, 0.25, 0.5, 0.75, 0.9, 0.95].iter().map(|&q| format!("{:.0} % {:.3}", q * 100.0, at(q))).collect();
            println!("{name}: {}", line.join(", "));
        }
    }
}
