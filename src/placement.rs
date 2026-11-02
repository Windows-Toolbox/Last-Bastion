//! All placement rules in one file – where a building may go, what it may be built on, how far apart it has to stay
//! from others, what it costs there (gold, workers, power – wood, stone and what has to stand first are in
//! `economy.rs`),
//! and how a dragged line is laid out. To change a rule, change
//! it here. The values (size, HP, cost of the buildings, how far zombies have to keep off a site, the refund for
//! replaced buildings) are in `data\buildings.json`.

use macroquad::math::{vec2, Vec2};

use crate::data;
use crate::kinds::BuildingKind;
use crate::sim::World;

/// No zombie may be this many tiles around a building site (plus the site itself) – you can't build
/// right next to the horde. `zombie_free_radius` in `data\buildings.json`.
pub fn build_clearance() -> i32 {
    data::get().buildings.zombie_free_radius
}

/// How many percent of its cost a building that gets built over gives back to the new one. Has to stay under 100,
/// otherwise "first walls, then the gate on top" would be cheaper than the gate alone (a test checks that).
fn replace_refund() -> u32 {
    data::get().buildings.replace_refund_percent
}

/// How many free tiles have to lie between a new `kind` and an `other` that already stands (0 = may touch).
/// Counted in all 8 directions: 1 means not even corner to corner. Always counts both ways – (Tower, Tent) is enough
/// to keep tents and towers apart, no matter which comes first.
fn gap(kind: BuildingKind, other: BuildingKind) -> i32 {
    match (kind, other) {
        _ if kind.is_tower() && other.is_tower() => 1,
        // Everything else may stand side by side. More rules go here, e.g. `(Tent, Tent) => 1,`.
        _ => 0,
    }
}

/// May a new `kind` be built over an `existing` one? It takes its place (the old one disappears).
/// The old one has to lie completely on the new site. A gate on walls, and the stone buildings on their wooden ones –
/// an upgrade on the spot.
pub fn replaces(kind: BuildingKind, existing: BuildingKind) -> bool {
    use BuildingKind::*;
    matches!(
        (kind, existing),
        (Gate, Wall) | (Gate, StoneWall) | (StoneWall, Wall) | (StoneTower, Tower) | (StoneHouse, Tent)
    )
}

/// The free tiles needed between two buildings – whichever of the two rules asks for more.
fn needed_gap(a: BuildingKind, b: BuildingKind) -> i32 {
    gap(a, b).max(gap(b, a))
}

/// A site in tiles: top left tile, width and depth.
#[derive(Clone, Copy)]
struct Site {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Site {
    fn new(kind: BuildingKind, tile: (i32, i32), turned: bool) -> Site {
        let (w, h) = kind.footprint(turned);
        Site { x: tile.0, y: tile.1, w, h }
    }

    fn tiles(self) -> impl Iterator<Item = (i32, i32)> {
        (self.y..self.y + self.h).flat_map(move |y| (self.x..self.x + self.w).map(move |x| (x, y)))
    }

    fn center(self) -> Vec2 {
        vec2(self.x as f32 + self.w as f32 / 2.0, self.y as f32 + self.h as f32 / 2.0)
    }

    fn covers(self, (x, y): (i32, i32)) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }

    fn contains(self, other: Site) -> bool {
        other.x >= self.x && other.y >= self.y && other.x + other.w <= self.x + self.w && other.y + other.h <= self.y + self.h
    }

    /// How many tiles lie between the two (0 if they touch – also at a corner – or overlap).
    fn gap_to(self, other: Site) -> i32 {
        let dx = (other.x - (self.x + self.w)).max(self.x - (other.x + other.w)).max(0);
        let dy = (other.y - (self.y + self.h)).max(self.y - (other.y + other.h)).max(0);
        dx.max(dy)
    }
}

impl World {
    /// Can a building go here?
    /// - every tile inside the world border, on open ground, and free – or under a building it may replace
    ///   (`replaces`),
    /// - a farm: every tile on grass – nothing grows on dirt (`BuildingKind::needs_fertile`),
    /// - far enough from other buildings (`gap`),
    /// - no zombie on the site or within `build_clearance` tiles around it (`build_zone`),
    /// - a building that makes, takes or carries power (`BuildingKind::on_grid`): its middle inside a grid
    ///   (`in_grid`) – the command center's reach or a tesla tower's.
    ///
    /// Your units may stand on it, like in StarCraft: the new building pushes them off to the nearest free spot
    /// (`push_out_of_terrain`).
    pub fn can_place(&self, kind: BuildingKind, tile: (i32, i32), turned: bool) -> bool {
        let site = Site::new(kind, tile, turned);
        let ground = site.tiles().all(|(x, y)| {
            self.map.is_buildable(x, y)
                || (self.map.in_play(x, y) && !self.map.is_solid(x, y) && self.replaceable_at(kind, site, (x, y)))
        });
        let fertile = !kind.needs_fertile() || site.tiles().all(|(x, y)| self.map.terrain(x, y).is_fertile());
        let powered = !kind.on_grid() || self.in_grid(site.center());
        ground && fertile && powered && self.far_enough(kind, site) && !self.zombies_in_build_zone(kind, tile, turned)
    }

    /// Editor: can a building go here without the rules of the game? Only on the map and not onto another building
    /// (except one it may replace) – terrain, zombies, units, distances and the world border don't matter.
    pub fn can_place_anywhere(&self, kind: BuildingKind, tile: (i32, i32), turned: bool) -> bool {
        let site = Site::new(kind, tile, turned);
        let size = self.map.size as i32;
        site.tiles().all(|(x, y)| {
            let on_map = x >= 0 && y >= 0 && x < size && y < size;
            // The map knows at once – the building index only after all orders of the tick. Never on a house of doom.
            on_map && !self.map.is_doom(x, y) && (!self.map.is_built(x, y) || self.replaceable_at(kind, site, (x, y)))
        })
    }

    /// The buildings (their `id`s) that a new one here would replace – they disappear when it's built.
    pub fn replaced_by(&self, kind: BuildingKind, tile: (i32, i32), turned: bool) -> Vec<u32> {
        let site = Site::new(kind, tile, turned);
        self.buildings.iter().filter(|b| self.is_replaceable(kind, site, b.kind, b.tile, b.turned)).map(|b| b.id).collect()
    }

    /// What the building costs here: its price, minus `replace_refund` percent of every building it replaces.
    /// Nothing is paid yet – the economy comes later (DESIGN.md 5).
    pub fn build_cost(&self, kind: BuildingKind, tile: (i32, i32), turned: bool) -> u32 {
        let site = Site::new(kind, tile, turned);
        let refund: u32 = self
            .buildings
            .iter()
            .filter(|b| self.is_replaceable(kind, site, b.kind, b.tile, b.turned))
            .map(|b| b.stats().cost * replace_refund() / 100)
            .sum();
        kind.stats().cost.saturating_sub(refund)
    }

    /// How many free workers the building needs here (`World::workers`): its `worker_cost`, minus those of every
    /// building it replaces – they go away with it, so their workers are free again. All of them, not a share like
    /// the gold.
    pub fn build_workers(&self, kind: BuildingKind, tile: (i32, i32), turned: bool) -> u32 {
        let site = Site::new(kind, tile, turned);
        let freed: u32 = self
            .buildings
            .iter()
            .filter(|b| self.is_replaceable(kind, site, b.kind, b.tile, b.turned))
            .map(|b| b.stats().worker_cost)
            .sum();
        kind.stats().worker_cost.saturating_sub(freed)
    }

    /// How much spare power the building needs here: its `energy_use`, minus that of every building it replaces (a
    /// stone house on a tent).
    pub fn build_energy(&self, kind: BuildingKind, tile: (i32, i32), turned: bool) -> u32 {
        let site = Site::new(kind, tile, turned);
        let freed: u32 = self
            .buildings
            .iter()
            .filter(|b| self.is_replaceable(kind, site, b.kind, b.tile, b.turned))
            .map(|b| self.energy_use(b.kind))
            .sum();
        self.energy_use(kind).saturating_sub(freed)
    }

    /// Does the grid where the building would stand have the power it needs to spare (`build_energy`)? Like in TAB:
    /// short of power, nothing new that takes it.
    pub fn has_power_for(&self, kind: BuildingKind, tile: (i32, i32), turned: bool) -> bool {
        let needed = self.build_energy(kind, tile, turned);
        let (w, h) = kind.footprint(turned);
        let center = vec2(tile.0 as f32 + w as f32 / 2.0, tile.1 as f32 + h as f32 / 2.0);
        needed == 0 || self.spare_power_at(&self.power(), center).is_some_and(|(_, spare)| spare >= needed)
    }

    /// The area that has to be free of zombies for a building: the site plus `build_clearance` tiles on every
    /// side, as (top left corner, bottom right corner). A wall (1 × 1) needs 9 × 9 tiles, a tent (2 × 2) 10 × 10.
    pub fn build_zone(kind: BuildingKind, tile: (i32, i32), turned: bool) -> (Vec2, Vec2) {
        let (w, h) = kind.footprint(turned);
        let min = vec2((tile.0 - build_clearance()) as f32, (tile.1 - build_clearance()) as f32);
        let max = min + vec2((w + 2 * build_clearance()) as f32, (h + 2 * build_clearance()) as f32);
        (min, max)
    }

    /// Is a (living) zombie inside the build zone? Checked by tile, like the rest of the grid.
    pub fn zombies_in_build_zone(&self, kind: BuildingKind, tile: (i32, i32), turned: bool) -> bool {
        let (min, max) = World::build_zone(kind, tile, turned);
        self.zombies.iter().any(|z| {
            z.is_alive() && z.pos.x >= min.x && z.pos.y >= min.y && z.pos.x < max.x && z.pos.y < max.y
        })
    }

    /// May building `b` be repaired now? Not with a living zombie in its build zone – the same zone that keeps you
    /// from building there (`zombies_in_build_zone`): nobody patches a wall the horde is hitting.
    /// Later also not on creep (DESIGN.md 8.4) – that check goes here once creep exists (roadmap step 9).
    pub fn can_repair(&self, b: usize) -> bool {
        let building = &self.buildings[b];
        !self.zombies_in_build_zone(building.kind, building.tile, building.turned)
    }

    /// Does a building stand on this tile that the new one may replace?
    fn replaceable_at(&self, kind: BuildingKind, site: Site, tile: (i32, i32)) -> bool {
        self.buildings.iter().any(|b| {
            Site::new(b.kind, b.tile, b.turned).covers(tile) && self.is_replaceable(kind, site, b.kind, b.tile, b.turned)
        })
    }

    /// May the new `kind` on `site` replace this building – right kind and completely on the site?
    fn is_replaceable(&self, kind: BuildingKind, site: Site, other: BuildingKind, tile: (i32, i32), turned: bool) -> bool {
        replaces(kind, other) && site.contains(Site::new(other, tile, turned))
    }

    /// Is the site far enough from every building that stays (`gap`)? Buildings it replaces don't count.
    fn far_enough(&self, kind: BuildingKind, site: Site) -> bool {
        self.buildings.iter().all(|b| {
            let other = Site::new(b.kind, b.tile, b.turned);
            let needed = needed_gap(kind, b.kind);
            needed == 0 || self.is_replaceable(kind, site, b.kind, b.tile, b.turned) || site.gap_to(other) >= needed
        })
    }
}

/// The top left tile if the building stands centred under the mouse – snapped to the grid.
pub fn placement_tile(kind: BuildingKind, turned: bool, point: Vec2) -> (i32, i32) {
    let (w, h) = kind.footprint(turned);
    let corner = (point - vec2(w as f32, h as f32) / 2.0).round();
    (corner.x as i32, corner.y as i32)
}

/// The spots of a dragged line from `start` to `end` (each the top left tile).
/// - Walls and towers like in TAB: dragged diagonally they stand corner to corner, one tile wide. Nobody gets
///   through such a corner – walking always goes past a side, never across a corner (`slide`, flow field).
/// - Tents edge to edge as a staircase.
/// - A gate is only ever placed alone – where the mouse is let go.
/// If a kind has to keep a `gap` to its own kind (towers), the line leaves that many tiles out between two pieces.
pub fn placement_line(kind: BuildingKind, start: (i32, i32), end: (i32, i32)) -> Vec<(i32, i32)> {
    let spacing = needed_gap(kind, kind);
    match kind {
        BuildingKind::Wall
        | BuildingKind::StoneWall
        | BuildingKind::Tower
        | BuildingKind::StoneTower
        | BuildingKind::Stakes
        | BuildingKind::Barrel
        | BuildingKind::MachineGunTower => {
            // Keep only pieces that are far enough from the last kept one (counted like `gap`: in all 8 directions).
            let mut kept: Vec<(i32, i32)> = Vec::new();
            for tile in diagonal_line(start, end) {
                let far = kept.last().map_or(true, |last| (tile.0 - last.0).abs().max((tile.1 - last.1).abs()) > spacing);
                if far {
                    kept.push(tile);
                }
            }
            kept
        }
        BuildingKind::Gate
        | BuildingKind::CommandCenter
        | BuildingKind::Barracks
        | BuildingKind::Sawmill
        | BuildingKind::Mine
        | BuildingKind::HunterCottage
        | BuildingKind::FishingHut
        | BuildingKind::Farm
        | BuildingKind::Windmill
        | BuildingKind::CharcoalBurner
        | BuildingKind::WoodWorkshop
        | BuildingKind::TeslaTower
        | BuildingKind::StoneWorkshop
        | BuildingKind::MunitionsFactory
        | BuildingKind::Market
        | BuildingKind::Bank
        | BuildingKind::Inn
        | BuildingKind::Foundry
        | BuildingKind::Ballista => vec![end],
        BuildingKind::Tent | BuildingKind::StoneHouse => {
            // Computed in "building steps": a tent (2 × 2) needs 2 tiles per step, plus the gap.
            let size = kind.stats().size.0 + spacing;
            let steps = |delta: i32| (delta as f32 / size as f32).round() as i32;
            let cells = grid_line((0, 0), (steps(end.0 - start.0), steps(end.1 - start.1)));
            cells.into_iter().map(|(x, y)| (start.0 + x * size, start.1 + y * size)).collect()
        }
    }
}

/// The tiles on a line from `start` to `end`, every step horizontal or vertical – so neighbouring
/// tiles always share a side: no gaps, no diagonal corners.
pub fn grid_line(start: (i32, i32), end: (i32, i32)) -> Vec<(i32, i32)> {
    let (cx, cy) = (end.0 - start.0, end.1 - start.1);
    let (sx, sy) = (cx.signum(), cy.signum());
    let (mut x, mut y) = (0, 0);
    let mut line = vec![start];
    while (x, y) != (cx, cy) {
        // One step horizontally or vertically – whichever stays closer to the straight line.
        // `|x · cy − y · cx|` is (up to a factor) the distance of the point from the line.
        let off_x = ((x + sx) * cy - y * cx).abs();
        let off_y = (x * cy - (y + sy) * cx).abs();
        let step_x = if x == cx {
            false
        } else if y == cy {
            true
        } else {
            off_x <= off_y
        };
        if step_x {
            x += sx;
        } else {
            y += sy;
        }
        line.push((start.0 + x, start.1 + y));
    }
    line
}

/// The tiles on a line from `start` to `end` – a step may also go diagonally, so the line is as short as
/// possible: as many tiles as the longer side. (Like Bresenham's line algorithm from computer graphics.)
fn diagonal_line(start: (i32, i32), end: (i32, i32)) -> Vec<(i32, i32)> {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let n = dx.abs().max(dy.abs());
    if n == 0 {
        return vec![start];
    }
    // The k-th tile lies k/n of the way along, rounded to the nearest tile.
    let along = |delta: i32, k: i32| (delta as f32 * k as f32 / n as f32).round() as i32;
    (0..=n).map(|k| (start.0 + along(dx, k), start.1 + along(dy, k))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Walls dragged straight: a line. Diagonal: one tile wide, corner to corner (like TAB), and at any other angle
    /// every piece still touches the previous one at least at a corner. Towers: one tile left free in between.
    /// Tents: a staircase, edge to edge, 2 tiles per step. A gate: only one, where the mouse is let go.
    #[test]
    fn placement_lines() {
        assert_eq!(placement_line(BuildingKind::Wall, (0, 0), (3, 0)), vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
        assert_eq!(placement_line(BuildingKind::Wall, (0, 0), (2, -2)), vec![(0, 0), (1, -1), (2, -2)]);
        let slanted = placement_line(BuildingKind::Wall, (0, 0), (5, 2));
        assert_eq!(slanted.len(), 6, "as many pieces as the longer side: {slanted:?}");
        assert_eq!(slanted.last(), Some(&(5, 2)));
        for pair in slanted.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert!((a.0 - b.0).abs() <= 1 && (a.1 - b.1).abs() <= 1, "gap between {a:?} and {b:?}");
        }
        assert_eq!(placement_line(BuildingKind::Tower, (0, 0), (4, 0)), vec![(0, 0), (2, 0), (4, 0)]);
        assert_eq!(placement_line(BuildingKind::Tower, (0, 0), (3, -3)), vec![(0, 0), (2, -2)]);
        let stairs = placement_line(BuildingKind::Tent, (0, 0), (4, -4));
        for pair in stairs.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert_eq!((a.0 - b.0).abs() + (a.1 - b.1).abs(), 2, "tents not edge to edge: {a:?} {b:?}");
        }
        assert_eq!(placement_line(BuildingKind::Tent, (10, 10), (10, 14)), vec![(10, 10), (10, 12), (10, 14)]);
        assert_eq!(placement_line(BuildingKind::Gate, (0, 0), (6, 3)), vec![(6, 3)]);
    }

    /// A painted stroke starts and ends where the mouse was and has no gap either.
    #[test]
    fn grid_line_connects_start_and_end() {
        let line = grid_line((5, 5), (9, 7));
        assert_eq!(line.first(), Some(&(5, 5)));
        assert_eq!(line.last(), Some(&(9, 7)));
        assert_eq!(line.len(), 4 + 2 + 1);
        assert_eq!(grid_line((3, 3), (3, 3)), vec![(3, 3)]);
    }

    /// The gate on walls is cheaper than on bare ground – but walls plus gate still cost more than the gate alone.
    #[test]
    fn gate_on_walls_is_not_worth_it() {
        let (wall, gate) = (BuildingKind::Wall.stats().cost, BuildingKind::Gate.stats().cost);
        for walls in 1..=3 {
            let discounted = gate.saturating_sub(walls * wall * replace_refund() / 100);
            assert!(discounted < gate, "no discount for {walls} walls");
            assert!(walls * wall + discounted > gate, "{walls} walls + gate on top is cheaper than the gate alone");
        }
    }

    /// The gap is the number of tiles in between, in all 8 directions.
    #[test]
    fn gap_counts_tiles_in_between() {
        let one = |x, y| Site { x, y, w: 1, h: 1 };
        assert_eq!(one(0, 0).gap_to(one(1, 0)), 0);
        assert_eq!(one(0, 0).gap_to(one(1, 1)), 0, "corner to corner touches");
        assert_eq!(one(0, 0).gap_to(one(2, 1)), 1);
        assert_eq!(Site { x: 0, y: 0, w: 3, h: 1 }.gap_to(one(5, 0)), 2);
    }
}
