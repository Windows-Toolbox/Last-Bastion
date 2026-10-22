//! When the map changes (painted terrain, new or fallen buildings): recompute what depends on it and push
//! everyone off new obstacles. Plus finding a free spot near a point.

use macroquad::math::{vec2, Vec2};

use super::buildings::NO_BUILDING;
use super::zombies::{values, BUILDING_DETOUR};
use super::World;
use crate::flow::{FlowField, Walker};
use crate::kinds::BuildingKind;
use crate::map::Map;

/// Someone standing where new terrain appears is pushed out – at most this many tiles far.
const PUSH_SEARCH: i32 = 24;
/// …and ends up at least this far inside the free tile, so it doesn't stick to the edge.
const PUSH_MARGIN: f32 = 0.3;

impl World {
    /// After changes to the map or the buildings: recompute the tile index, the buildings' threat map and
    /// the zombies' paths. The zombies walk towards the command center; if there is none anymore, towards the
    /// base tile. To them buildings in the way are only a detour price, not a wall – except infected ones and traps:
    /// those are theirs or under their feet, not a target (left out of the threat map) and not something to tear down (`Map::can_tear_down`).
    pub(super) fn refresh_map(&mut self) {
        self.map_changed = false;
        // The ways to the lure places went around what stood before – made anew when needed (`zombies.rs`).
        self.lures.clear();
        self.building_at.fill(NO_BUILDING);
        self.building_aggro.clear();
        let size = self.map.size as i32;
        for (b, building) in self.buildings.iter().enumerate() {
            for (x, y) in building.tiles() {
                self.building_at[(y * size + x) as usize] = b as u32;
            }
            if !building.zombies_ignore() {
                self.building_aggro.stamp(b, building.corner(), building.far_corner(), values().notice_buildings);
            }
        }
        let goals = match self.buildings.iter().find(|b| b.kind == BuildingKind::CommandCenter) {
            Some(cc) => cc.tiles(),
            None => vec![(self.base.x.floor() as i32, self.base.y.floor() as i32)],
        };
        let zombie = Walker::Zombie { detour: BUILDING_DETOUR };
        self.to_base = FlowField::to_area(&self.map, &goals, self.base, zombie);
    }

    /// Whoever now stands on painted terrain or a new building moves to the nearest free spot – you may build on
    /// your own units, like in StarCraft they get out of the way (zombies can't be that close: `can_place`). Several
    /// pushed onto the same tile push each other apart afterwards (`unit_separation`).
    pub(super) fn push_out_of_terrain(&mut self) {
        let map = &self.map;
        for zombie in &mut self.zombies {
            if let Some(free) = push_out(zombie.pos, |x, y| map.is_blocked(x, y)) {
                zombie.pos = free;
                zombie.prev = free; // don't draw it sliding through the obstacle
            }
        }
        // Units in a tower stand on a building on purpose.
        for unit in self.units.iter_mut().filter(|u| !u.is_inside()) {
            if let Some(free) = push_out(unit.pos, |x, y| map.blocks_unit(x, y)) {
                unit.pos = free;
                unit.prev = free;
            }
        }
    }
}

/// The nearest tile to `p` that isn't `blocked` (its own tile if that is free), searched ring by ring up to
/// `max_r` tiles away. `blocked` decides who is asking: units also count the world border, zombies don't.
fn nearest_free_tile(p: Vec2, max_r: i32, blocked: impl Fn(i32, i32) -> bool) -> Option<(i32, i32)> {
    let (px, py) = (p.x.floor() as i32, p.y.floor() as i32);
    if !blocked(px, py) {
        return Some((px, py));
    }
    for r in 1..=max_r {
        // Only the ring at distance r – the inner rings had nothing free.
        let mut best: Option<(f32, (i32, i32))> = None;
        for y in py - r..=py + r {
            for x in px - r..=px + r {
                let on_ring = (x - px).abs() == r || (y - py).abs() == r;
                if !on_ring || blocked(x, y) {
                    continue;
                }
                let d = vec2(x as f32 + 0.5, y as f32 + 0.5).distance(p);
                if best.is_none_or(|(best_d, _)| d < best_d) {
                    best = Some((d, (x, y)));
                }
            }
        }
        if let Some((_, tile)) = best {
            return Some(tile);
        }
    }
    None
}

/// For units: the point itself if it's free – otherwise the middle of the nearest free tile (up to 4 tiles
/// away). That way you can also click on a building or a mountain and the units walk up to it.
pub(super) fn free_point_near(map: &Map, p: Vec2) -> Option<Vec2> {
    let blocked = |x, y| map.blocks_unit(x, y);
    if !blocked(p.x.floor() as i32, p.y.floor() as i32) {
        return Some(p);
    }
    nearest_free_tile(p, 4, blocked).map(|(x, y)| vec2(x as f32 + 0.5, y as f32 + 0.5))
}

/// Where something standing on blocked ground at `pos` goes: onto the nearest free tile, as close to
/// where it was as possible. `None` if `pos` is free anyway (or nothing is free far and wide).
fn push_out(pos: Vec2, blocked: impl Fn(i32, i32) -> bool) -> Option<Vec2> {
    if !blocked(pos.x.floor() as i32, pos.y.floor() as i32) {
        return None;
    }
    let (x, y) = nearest_free_tile(pos, PUSH_SEARCH, blocked)?;
    let min = vec2(x as f32, y as f32) + Vec2::splat(PUSH_MARGIN);
    let max = vec2(x as f32 + 1.0, y as f32 + 1.0) - Vec2::splat(PUSH_MARGIN);
    Some(pos.clamp(min, max))
}
