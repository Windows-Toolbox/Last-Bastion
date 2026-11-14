//! Named regions and points, like StarCraft 2's (`docs\editor.md` step 4, DESIGN.md 12.9 step 5): what triggers and
//! events will talk about – "north gate", "village 3", "camera intro". The editor's module "Regions" makes them
//! (`editor\regions.rs`); players never see them.
//!
//! - A **zone** (the editor says "region" – `map::Region` is already the plain rectangle of the camera and the
//!   border) is a set of tiles put together from **shapes**: rectangles, circles and tiles painted by hand, each one
//!   added to or cut out of what the shapes before it gave. So "the gate without the tower in it" is two shapes.
//! - A **spot** (the editor: "point") is one tile with a name.
//!
//! Both keep their number (`id`) for good – in the map file too. Whoever uses a region names it by its number, so
//! renaming never breaks anything.
//!
//! Only whole numbers: whether a tile lies in a region is the same on every PC (DESIGN.md 12.8).

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use super::{Area, World};

/// A region has at most this many shapes, and a name at most this many letters.
pub const MOST_SHAPES: usize = 32;
const MAX_NAME: usize = 24;
/// The editor has this many colours for regions and points (`color` counts through them).
pub const ZONE_COLORS: u8 = 8;
/// A circle is at most this big, and no shape lies further off the map than the map is wide – only so nothing a
/// broken file says makes the game count for minutes.
const MOST_RADIUS: i32 = 200;

/// What a shape looks like. All in tiles.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Form {
    /// From the tile `min`, `size` tiles wide and deep.
    Rect { min: (i32, i32), size: (i32, i32) },
    /// The tiles around `center` not further away than `radius` – rounded like the editor's brush, so radius 1 is
    /// 3 × 3 tiles and radius 0 the one tile.
    Circle { center: (i32, i32), radius: i32 },
    /// These tiles, painted by hand – sorted (row by row), each once.
    Tiles(Vec<(i32, i32)>),
}

impl Form {
    pub fn contains(&self, (x, y): (i32, i32)) -> bool {
        match self {
            Form::Rect { min, size } => x >= min.0 && y >= min.1 && x < min.0 + size.0 && y < min.1 + size.1,
            Form::Circle { center, radius } => {
                let (dx, dy) = (x - center.0, y - center.1);
                dx * dx + dy * dy <= radius * radius + radius
            }
            // Sorted, so the tile is found by halving the list (`binary_search`) instead of looking at every entry.
            Form::Tiles(tiles) => tiles.binary_search_by_key(&(y, x), |&(tx, ty)| (ty, tx)).is_ok(),
        }
    }

    /// The smallest rectangle of tiles all of it lies in – `None`: it has no tiles.
    pub fn bounds(&self) -> Option<Area> {
        match self {
            Form::Rect { min, size } => Some(Area { min: *min, max: (min.0 + size.0 - 1, min.1 + size.1 - 1) }),
            Form::Circle { center, radius } => Some(Area { min: (center.0 - radius, center.1 - radius), max: (center.0 + radius, center.1 + radius) }),
            Form::Tiles(tiles) => {
                let first = *tiles.first()?;
                Some(tiles.iter().fold(Area { min: first, max: first }, |a, &(x, y)| Area { min: (a.min.0.min(x), a.min.1.min(y)), max: (a.max.0.max(x), a.max.1.max(y)) }))
            }
        }
    }

    /// The same shape `d` tiles further.
    pub fn moved(&self, d: (i32, i32)) -> Form {
        let go = |t: (i32, i32)| (t.0 + d.0, t.1 + d.1);
        match self {
            Form::Rect { min, size } => Form::Rect { min: go(*min), size: *size },
            Form::Circle { center, radius } => Form::Circle { center: go(*center), radius: *radius },
            Form::Tiles(tiles) => Form::Tiles(tiles.iter().map(|&t| go(t)).collect()),
        }
    }
}

/// One shape of a region: added to it, or – `cut` – taken out of what the shapes before it gave.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Shape {
    pub form: Form,
    /// `skip_serializing_if`: not written when it's `false` – most shapes add.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cut: bool,
}

/// A named region. `name` first: the map file starts a new line at it (`mapfile::pretty`).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Zone {
    pub name: String,
    pub id: u32,
    /// Which of the editor's colours it's drawn in – only there.
    #[serde(default)]
    pub color: u8,
    pub shapes: Vec<Shape>,
}

impl Zone {
    /// A new one of a single shape – its number, and its name if it's empty, come when it's added (`Order::AddZone`).
    pub fn new(name: &str, color: u8, form: Form) -> Zone {
        Zone { name: name.to_owned(), id: 0, color, shapes: vec![Shape { form, cut: false }] }
    }

    /// Does this tile belong to it? Shape after shape: the last one that has the tile decides.
    pub fn contains(&self, tile: (i32, i32)) -> bool {
        // `rev`: from the back – the first shape found there with the tile is the last one in the list.
        self.shapes.iter().rev().find(|s| s.form.contains(tile)).is_some_and(|s| !s.cut)
    }

    /// The smallest rectangle of tiles everything added lies in (what's cut out may make it smaller than that).
    pub fn bounds(&self) -> Option<Area> {
        let mut all: Option<Area> = None;
        for bounds in self.shapes.iter().filter(|s| !s.cut).filter_map(|s| s.form.bounds()) {
            all = Some(match all {
                Some(a) => Area { min: (a.min.0.min(bounds.min.0), a.min.1.min(bounds.min.1)), max: (a.max.0.max(bounds.max.0), a.max.1.max(bounds.max.1)) },
                None => bounds,
            });
        }
        all
    }

    /// The middle of its bounds (world point) – where "Locate" goes and its name stands.
    pub fn center(&self) -> Option<Vec2> {
        self.bounds().map(|b| vec2((b.min.0 + b.max.0 + 1) as f32 / 2.0, (b.min.1 + b.max.1 + 1) as f32 / 2.0))
    }

    /// The same region `d` tiles further.
    pub fn moved(&self, d: (i32, i32)) -> Zone {
        Zone { shapes: self.shapes.iter().map(|s| Shape { form: s.form.moved(d), cut: s.cut }).collect(), ..self.clone() }
    }

    /// These tiles painted into it (or, `cut`, out of it): into its last shape if that's painted tiles of the same
    /// kind, otherwise as a new shape – so a stroke of the brush is one shape, however many frames it took.
    fn paint(&mut self, tiles: &[(i32, i32)], cut: bool) {
        let room = self.shapes.len() < MOST_SHAPES;
        match self.shapes.last_mut() {
            Some(Shape { form: Form::Tiles(old), cut: was }) if *was == cut => old.extend_from_slice(tiles),
            _ if room => self.shapes.push(Shape { form: Form::Tiles(tiles.to_vec()), cut }),
            _ => {}
        }
    }

    /// Makes it one the game can count on, whatever the editor or a file said: a short name, not too many shapes,
    /// each at least one tile and near the map, painted tiles on the map, sorted, each once.
    fn tidy(&mut self, map_size: i32) {
        self.name = self.name.trim().chars().take(MAX_NAME).collect();
        self.color %= ZONE_COLORS;
        self.shapes.truncate(MOST_SHAPES);
        let near = |t: (i32, i32)| (t.0.clamp(-map_size, 2 * map_size), t.1.clamp(-map_size, 2 * map_size));
        for shape in &mut self.shapes {
            match &mut shape.form {
                Form::Rect { min, size } => {
                    *min = near(*min);
                    *size = (size.0.clamp(1, 2 * map_size), size.1.clamp(1, 2 * map_size));
                }
                Form::Circle { center, radius } => {
                    *center = near(*center);
                    *radius = (*radius).clamp(0, MOST_RADIUS);
                }
                Form::Tiles(tiles) => {
                    tiles.retain(|&(x, y)| x >= 0 && y >= 0 && x < map_size && y < map_size);
                    tiles.sort_by_key(|&(x, y)| (y, x));
                    tiles.dedup();
                }
            }
        }
        // Painted shapes with nothing left in them go.
        self.shapes.retain(|s| !matches!(&s.form, Form::Tiles(tiles) if tiles.is_empty()));
    }
}

/// A named point: one tile.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Spot {
    pub name: String,
    pub id: u32,
    #[serde(default)]
    pub color: u8,
    pub tile: (i32, i32),
}

impl Spot {
    /// The middle of its tile (world point).
    pub fn center(&self) -> Vec2 {
        vec2(self.tile.0 as f32 + 0.5, self.tile.1 as f32 + 0.5)
    }

    fn tidy(&mut self, map_size: i32) {
        self.name = self.name.trim().chars().take(MAX_NAME).collect();
        self.color %= ZONE_COLORS;
        self.tile = (self.tile.0.clamp(0, map_size - 1), self.tile.1.clamp(0, map_size - 1));
    }
}

impl World {
    /// Where is the region with this number in the list? `None`: it has been removed.
    pub fn find_zone(&self, id: u32) -> Option<usize> {
        self.zones.iter().position(|z| z.id == id)
    }

    pub fn find_spot(&self, id: u32) -> Option<usize> {
        self.spots.iter().position(|s| s.id == id)
    }

    /// The number the next region or point will get (they count together) – the editor selects a new one right away.
    pub fn next_zone_id(&self) -> u32 {
        self.next_zone_id + 1
    }

    /// The region this tile lies in – of several the one made last (it's drawn on top).
    pub fn zone_at(&self, tile: (i32, i32)) -> Option<&Zone> {
        self.zones.iter().rev().find(|z| z.contains(tile))
    }

    /// `Order::AddZone`: a new region – or one from a map file, which keeps its number if no other has it.
    pub(crate) fn add_zone(&mut self, mut zone: Zone) {
        zone.tidy(self.map.size as i32);
        if zone.shapes.is_empty() {
            return;
        }
        zone.id = self.new_zone_id(zone.id);
        if zone.name.is_empty() {
            zone.name = format!("Region {}", zone.id);
        }
        self.zones.push(zone);
    }

    /// `Order::AddSpot`: the same for a point.
    pub(crate) fn add_spot(&mut self, mut spot: Spot) {
        spot.tidy(self.map.size as i32);
        spot.id = self.new_zone_id(spot.id);
        if spot.name.is_empty() {
            spot.name = format!("Point {}", spot.id);
        }
        self.spots.push(spot);
    }

    /// `wanted` if it's a number nobody has yet (a map file's), otherwise the next free one.
    fn new_zone_id(&mut self, wanted: u32) -> u32 {
        let taken = wanted == 0 || self.find_zone(wanted).is_some() || self.find_spot(wanted).is_some();
        let id = if taken { self.next_zone_id + 1 } else { wanted };
        self.next_zone_id = self.next_zone_id.max(id);
        id
    }

    /// `Order::SetZone`: the region with `zone.id` is this one now – its name, colour and shapes. An empty name keeps
    /// the old one; without a shape left nothing changes (removing is `RemoveZone`).
    pub(super) fn set_zone(&mut self, zone: &Zone) {
        let Some(z) = self.find_zone(zone.id) else {
            return;
        };
        let mut new = zone.clone();
        new.tidy(self.map.size as i32);
        if new.shapes.is_empty() {
            return;
        }
        if new.name.is_empty() {
            new.name = self.zones[z].name.clone();
        }
        self.zones[z] = new;
    }

    /// `Order::PaintZone`: tiles painted into a region, or out of it.
    pub(super) fn paint_zone(&mut self, id: u32, tiles: &[(i32, i32)], cut: bool) {
        let Some(z) = self.find_zone(id) else {
            return;
        };
        let mut new = self.zones[z].clone();
        new.paint(tiles, cut);
        new.tidy(self.map.size as i32);
        // Everything painted away: the region stays as it was – an empty one couldn't be found on the map again.
        if !new.shapes.is_empty() {
            self.zones[z] = new;
        }
    }

    pub(super) fn set_spot(&mut self, spot: &Spot) {
        let Some(s) = self.find_spot(spot.id) else {
            return;
        };
        let mut new = spot.clone();
        new.tidy(self.map.size as i32);
        if new.name.is_empty() {
            new.name = self.spots[s].name.clone();
        }
        self.spots[s] = new;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapfile::MapFile;
    use crate::sim::Order;

    fn rect(x: i32, y: i32, w: i32, h: i32) -> Form {
        Form::Rect { min: (x, y), size: (w, h) }
    }

    /// Shapes are put together in order: a circle cut out of a rectangle is a hole, a rectangle added after it fills
    /// part of the hole again.
    #[test]
    fn shapes_add_and_cut_in_order() {
        let mut zone = Zone::new("Gate", 0, rect(10, 10, 12, 6));
        assert!(zone.contains((10, 10)) && zone.contains((21, 15)));
        assert!(!zone.contains((22, 15)) && !zone.contains((9, 10)));
        zone.shapes.push(Shape { form: Form::Circle { center: (15, 12), radius: 1 }, cut: true });
        assert!(!zone.contains((15, 12)) && !zone.contains((16, 13)), "the circle of radius 1 is 3 x 3 tiles");
        assert!(zone.contains((17, 12)));
        zone.shapes.push(Shape { form: rect(15, 12, 1, 1), cut: false });
        assert!(zone.contains((15, 12)) && !zone.contains((14, 12)));
        assert_eq!(zone.bounds(), Some(Area { min: (10, 10), max: (21, 15) }));
        let moved = zone.moved((3, -2));
        assert!(moved.contains((18, 10)) && !moved.contains((17, 10)));
    }

    /// The orders: a new region gets a number and a name, painting goes into one shape per stroke, a region can't be
    /// emptied, numbers are never given twice – regions and points count together.
    #[test]
    fn orders_make_change_and_remove() {
        let mut world = World::new(3);
        let first = world.next_zone_id();
        world.apply_orders(&[
            Order::AddZone { zone: Zone::new("", 9, rect(5, 5, 4, 4)) },
            Order::AddSpot { spot: Spot { name: "  Camera intro  ".into(), id: 0, color: 1, tile: (-3, 7) } },
        ]);
        assert_eq!((world.zones[0].id, world.spots[0].id), (first, first + 1));
        assert_eq!(world.zones[0].name, format!("Region {first}"));
        assert_eq!(world.zones[0].color, 9 % ZONE_COLORS);
        assert_eq!((world.spots[0].name.as_str(), world.spots[0].tile), ("Camera intro", (0, 7)), "trimmed, and onto the map");

        // Two frames of one stroke: one shape, sorted, each tile once, nothing off the map.
        world.apply_orders(&[Order::PaintZone { zone: first, tiles: vec![(20, 20), (19, 20), (-1, 0)], cut: false }]);
        world.apply_orders(&[Order::PaintZone { zone: first, tiles: vec![(20, 20), (20, 19)], cut: false }]);
        assert_eq!(world.zones[0].shapes.len(), 2);
        assert_eq!(world.zones[0].shapes[1].form, Form::Tiles(vec![(20, 19), (19, 20), (20, 20)]));
        assert!(world.zones[0].contains((19, 20)) && !world.zones[0].contains((21, 20)));
        world.apply_orders(&[Order::PaintZone { zone: first, tiles: vec![(19, 20), (6, 6)], cut: true }]);
        assert!(!world.zones[0].contains((19, 20)) && !world.zones[0].contains((6, 6)) && world.zones[0].contains((5, 5)));
        assert_eq!(world.zone_at((5, 5)).map(|z| z.id), Some(first));
        assert_eq!(world.zone_at((6, 6)).map(|z| z.id), None);

        // Renamed and recoloured; an empty name keeps the old one, no shapes changes nothing.
        let renamed = Zone { name: "North gate".into(), color: 2, ..world.zones[0].clone() };
        world.apply_orders(&[Order::SetZone { zone: renamed.clone() }]);
        assert_eq!(world.zones[0], renamed);
        world.apply_orders(&[Order::SetZone { zone: Zone { name: " ".into(), shapes: Vec::new(), ..renamed.clone() } }]);
        assert_eq!(world.zones[0], renamed);

        world.apply_orders(&[Order::RemoveZone { zone: first }, Order::RemoveSpot { spot: first + 1 }]);
        assert!(world.zones.is_empty() && world.spots.is_empty());
        world.apply_orders(&[Order::AddZone { zone: Zone::new("Again", 0, rect(1, 1, 1, 1)) }]);
        assert_eq!(world.zones[0].id, first + 2, "a number is never given twice");
    }

    /// Saved and loaded, regions and points are as they were, numbers included – the next new one counts on from
    /// there. A map file from before them (format 5) loads without any.
    #[test]
    fn regions_are_in_the_map_file() {
        let mut world = World::new(4);
        let mut gate = Zone::new("North gate", 3, rect(30, 40, 12, 6));
        gate.shapes.push(Shape { form: Form::Circle { center: (33, 42), radius: 2 }, cut: true });
        gate.shapes.push(Shape { form: Form::Tiles(vec![(50, 50), (51, 50)]), cut: false });
        world.apply_orders(&[
            Order::AddZone { zone: Zone::new("Gone", 0, rect(1, 1, 2, 2)) },
            Order::AddZone { zone: gate },
            Order::AddSpot { spot: Spot { name: "Rally".into(), id: 0, color: 5, tile: (60, 61) } },
        ]);
        let gone = world.zones[0].id;
        world.apply_orders(&[Order::RemoveZone { zone: gone }]);
        let file = MapFile::from_world(&world, "Regions");
        let text = serde_json::to_string(&file).unwrap();
        let back: MapFile = serde_json::from_str(&text).unwrap();
        let mut loaded = back.to_world().unwrap();
        assert_eq!(loaded.zones, world.zones);
        assert_eq!(loaded.spots, world.spots);
        loaded.apply_orders(&[Order::AddZone { zone: Zone::new("", 0, rect(1, 1, 1, 1)) }]);
        assert_eq!(loaded.zones[1].id, world.spots[0].id + 1);

        // On a bigger grid everything lies as many tiles further as the grid grew on each side.
        let grown = file.resized(file.size + 8).to_world().unwrap();
        assert!(grown.zones[0].contains((34, 44)) && !grown.zones[0].contains((30, 40)));
        assert!(grown.zones[0].contains((54, 54)));
        assert_eq!(grown.spots[0].tile, (64, 65));

        // An old file: no regions, and it isn't refused.
        let old = text.replacen(&format!("\"version\":{}", file.version), "\"version\":5", 1);
        let old = old[..old.find(",\"regions\"").unwrap()].to_owned() + &old[old.find(",\"buildings\"").unwrap()..];
        let mut old: MapFile = serde_json::from_str(&old).unwrap();
        assert!(old.regions.is_empty() && old.points.is_empty());
        assert!(old.to_world().unwrap().zones.is_empty());
        old.regions = file.regions.clone();
        assert_eq!(old.to_world().unwrap().zones, world.zones);
    }
}
