//! Drawing. Only reads the world, never changes it.
//! Placeholder graphics for now: diamonds for the ground, small boxes for zombies and units, blocks and joined-up
//! walls for buildings (DESIGN.md 12.3). In the game view the fog of war lies over everything nobody sees.

use std::collections::HashMap;

use macroquad::prelude::*;

use crate::hud::{draw_life, hp_color};
use crate::iso::{Camera, TILE_H, TILE_W};
use crate::kinds::{BuildingKind, UnitKind};
use crate::map::{Ore, Region, Terrain};
use crate::sim::{Breed, Building, Mark, World};
use crate::vision::Vision;

// `pub`, because the minimap (hud.rs) uses the same colours.
pub const GROUND: Color = Color::new(0.20, 0.25, 0.18, 1.0);
/// Zombies on the minimap and in the editor's markings – on the map each breed has its own grey (`breed_color`).
pub const ZOMBIE: Color = Color::new(0.55, 0.75, 0.35, 1.0);
/// A zombie is a box (placeholder skin like the units): its edge on the ground is this many times its `radius` …
const ZOMBIE_SIZE: f32 = 1.12;
/// … and a normal one (radius 0.25) is this tall, in tile widths – a bigger one a bit taller.
const ZOMBIE_HEIGHT: f32 = 0.4;
/// Walkers dark grey, runners light grey, fat ones a sickly yellowish brown.
const WALKER: Color = Color::new(0.30, 0.30, 0.32, 1.0);
const RUNNER: Color = Color::new(0.74, 0.74, 0.76, 1.0);
const FAT: Color = Color::new(0.58, 0.52, 0.34, 1.0);
/// What a unit holds (rifle, bow): a thin bar sticking out where it faces – so you see it turn.
const WEAPON: Color = Color::new(0.15, 0.15, 0.17, 1.0);
/// What an infected building turns towards.
const INFECTED: Color = Color::new(0.30, 0.42, 0.18, 1.0);
/// A fallen unit lies flat on the ground until it rises.
const CORPSE_HEIGHT: f32 = 0.06;
const HOLD_RING: Color = ORANGE;
const SHOT: Color = Color::new(1.0, 0.9, 0.4, 0.8);
const BEAM: Color = Color::new(0.35, 1.0, 0.45, 0.9);
/// The world border line in editor mode.
const BORDER_LINE: Color = Color::new(1.0, 0.55, 0.15, 0.9);
/// Fog of war: never seen – black like in StarCraft 2 (also on the minimap) …
pub const UNEXPLORED: Color = Color::new(0.0, 0.0, 0.0, 1.0);
/// … seen before, but nobody is looking right now – darkened.
const EXPLORED: Color = Color::new(0.0, 0.0, 0.0, 0.5);
/// Editor "View: Zombies": where no zombie looks – dark red, so it doesn't get mixed up with the player's fog.
const ZOMBIE_DARK: Color = Color::new(0.12, 0.0, 0.0, 0.6);
/// The parts of the wall skin (only the look): width of a wall piece, width and height of the post in the middle.
const WALL_WIDTH: f32 = 0.4;
const POST: f32 = 0.5;
/// A wall standing alone is a bigger block.
const LONE_POST: f32 = 0.7;
/// The wooden door of a gate.
const DOOR_COLOR: Color = Color::new(0.45, 0.31, 0.17, 1.0);
/// Spawners in the editor, and the glowing edge of the selected one.
const SPAWNER: Color = Color::new(0.85, 0.25, 0.55, 1.0);
const SPAWNER_GLOW: Color = Color::new(1.0, 0.85, 0.30, 1.0);
const TREE_CROWN: Color = Color::new(0.13, 0.42, 0.16, 1.0);
const TREE_TRUNK: Color = Color::new(0.35, 0.24, 0.14, 1.0);

/// What the picture should show – everything that isn't in the world itself.
pub struct View<'a> {
    /// The part of the map that is drawn at all. In the game view the playing field inside the world border –
    /// beyond it there is nothing to see, not even the zombies coming. In editor mode the whole map.
    pub area: Region,
    /// Editor mode: also draw the world border as a line.
    pub editing: bool,
    /// Show the whole command queue (setting; otherwise like in StarCraft only while Shift is held), otherwise only
    /// the current target.
    pub full_plan: bool,
    /// Health bars above all units (setting).
    pub always_bars: bool,
    /// Whose eyes: fog of war like in the game, the zombies' view, or none (editor).
    pub fog: Fog<'a>,
    /// In the player's view: how visible each zombie still is (fading out after leaving sight).
    pub fades: &'a Fades,
    /// The spawners to show (only in editor mode) – with the size they have while being dragged.
    pub spawners: Vec<SpawnerShape>,
}

/// How long a zombie takes to fade out after it leaves your sight (seconds).
const FADE_TIME: f32 = 0.8;

/// How visible every zombie still is in the player's view (1 = seen right now, falling to 0 once nobody sees it any
/// more) – so they fade out instead of vanishing at once. Only for the picture, not part of the simulation, so a
/// `HashMap` is fine here (DESIGN.md 12.8 only forbids it in the simulation).
#[derive(Default)]
pub struct Fades {
    alpha: HashMap<u32, f32>,
}

impl Fades {
    /// Once per frame: seen zombies are fully there, the others fade by `dt` (seconds). Gone or faded: forgotten.
    pub fn update(&mut self, world: &World, dt: f32) {
        let mut next = HashMap::with_capacity(self.alpha.len());
        for zombie in &world.zombies {
            let alpha = if world.vision.sees(zombie.pos) {
                1.0
            } else {
                self.alpha.get(&zombie.id).copied().unwrap_or(0.0) - dt / FADE_TIME
            };
            if alpha > 0.0 {
                next.insert(zombie.id, alpha);
            }
        }
        self.alpha = next;
    }

    /// How visible this zombie is (0 = not at all).
    pub fn alpha(&self, id: u32) -> f32 {
        self.alpha.get(&id).copied().unwrap_or(0.0)
    }
}

/// What lies over the map, depending on whose eyes you look through (`editor::Sight`).
/// `'a`: the zombies' view is borrowed from `main` – the `View` mustn't live longer than it (a "lifetime").
pub enum Fog<'a> {
    /// Editor "All": no fog.
    Off,
    /// The player: black where nobody has ever looked, darkened where nobody looks right now, and zombies only
    /// where someone sees them.
    Player,
    /// The zombies: dark where no zombie looks. All zombies are drawn – they are the eyes.
    Zombies(&'a Vision),
}

/// A spawner as the editor shows it.
pub struct SpawnerShape {
    /// Top left corner and size in tiles.
    pub min: Vec2,
    pub size: Vec2,
    pub name: String,
    /// Selected: the edge glows.
    pub selected: bool,
}

/// `selected`: the `id`s of the selected units.
/// `alpha` (0..1): how far we are between the previous and the current tick.
pub fn draw_world(world: &World, camera: &Camera, view: &View, selected: &[u32], alpha: f32) {
    let area = view.area;

    // --- Ground: one big diamond for the visible area, terrain as small diamonds on top ---
    draw_diamond(camera, area.min, Vec2::splat(area.size), GROUND);
    let (lo, hi) = (area.min.x as i32, area.max().x as i32);
    for y in lo..hi {
        for x in lo..hi {
            let terrain = world.map.terrain(x, y);
            let deposit = world.map.deposit(x, y);
            if terrain == Terrain::Grass && deposit.is_none() {
                continue;
            }
            let corner = vec2(x as f32, y as f32);
            if !on_screen(camera.world_to_screen(corner + 0.5), 40.0) {
                continue;
            }
            // Ore on grass: the tile in the ore's colour, a smaller lighter diamond in the middle as a "lump".
            if let Some(deposit) = deposit {
                let color = ore_color(deposit.ore);
                draw_diamond(camera, corner, Vec2::ONE, color);
                draw_diamond(camera, corner + 0.3, Vec2::splat(0.4), lighten(color));
                continue;
            }
            if terrain == Terrain::Tree {
                draw_tree(camera, corner + 0.5);
                continue;
            }
            draw_diamond(camera, corner, Vec2::ONE, terrain_color(terrain));
            // Forest: a darker smaller diamond in the middle as a "treetop", so it doesn't look like grass.
            if terrain == Terrain::Forest {
                draw_diamond(camera, corner + 0.25, Vec2::splat(0.5), Color::new(0.06, 0.20, 0.08, 1.0));
            }
        }
    }
    // Fog of war on top of the ground – before the figures, which are hidden in the fog anyway (zombies) or
    // always visible (your own units and buildings see themselves).
    match view.fog {
        Fog::Off => {}
        Fog::Player => draw_fog(camera, area, |x, y| {
            if world.vision.is_visible(x, y) {
                None
            } else if world.vision.is_explored(x, y) {
                Some(EXPLORED)
            } else {
                Some(UNEXPLORED)
            }
        }),
        Fog::Zombies(sight) => draw_fog(camera, area, |x, y| (!sight.is_visible(x, y)).then_some(ZOMBIE_DARK)),
    }
    for spawner in &view.spawners {
        draw_spawner(camera, spawner);
    }
    // Editor: where the world border runs.
    if view.editing {
        let play = world.map.play_area();
        let (min, max) = (play.min, play.max());
        let corners = [min, vec2(max.x, min.y), max, vec2(min.x, max.y)];
        for k in 0..4 {
            let (p, q) = (camera.world_to_screen(corners[k]), camera.world_to_screen(corners[(k + 1) % 4]));
            draw_line(p.x, p.y, q.x, q.y, 2.5, BORDER_LINE);
        }
    }

    // --- Selection rings and command queues lie on the ground, so draw them before the figures ---
    for (i, unit) in world.units.iter().enumerate() {
        if !selected.contains(&unit.id) {
            continue;
        }
        let s = camera.world_to_screen(lerp(unit.prev, unit.pos, alpha));
        let w = 0.7 * TILE_W * camera.zoom;
        let ring = if unit.is_holding() { HOLD_RING } else { GREEN };
        draw_ellipse_lines(s.x, s.y, w, w / 2.0, 0.0, 1.5, ring);

        // Line from point to point of the command queue.
        let plan = world.plan(i);
        let shown = if view.full_plan { plan.len() } else { plan.len().min(1) };
        let mut from = s;
        for &(point, mark) in &plan[..shown] {
            let to = camera.world_to_screen(point);
            let color = mark_color(mark);
            draw_line(from.x, from.y, to.x, to.y, 1.0, Color { a: 0.5, ..color });
            draw_circle(to.x, to.y, 3.0, color);
            from = to;
        }
    }

    // --- Sort figures and buildings by depth: whoever stands further back is drawn first ---
    let mut things: Vec<(f32, Prism)> = Vec::with_capacity(world.zombies.len() + world.units.len());
    for zombie in world.zombies.iter().filter(|z| area.contains(z.pos)) {
        // In the player's view only the ones you see – and those still fading out.
        let visible = if matches!(view.fog, Fog::Player) { view.fades.alpha(zombie.id) } else { 1.0 };
        if visible <= 0.0 {
            continue;
        }
        let p = lerp(zombie.prev, zombie.pos, alpha);
        let color = breed_color(zombie.breed);
        let size = zombie.radius * ZOMBIE_SIZE;
        let height = ZOMBIE_HEIGHT * (0.5 + 2.0 * zombie.radius);
        let ground = rect(p - Vec2::splat(size / 2.0), Vec2::splat(size));
        things.push((p.x + p.y, Prism(ground, 0.0, height, Color { a: visible, ..color })));
    }
    for building in &world.buildings {
        building_parts(world, building, &mut things);
    }
    // Fallen units: flat and darker, a bit bigger than standing – they lie there.
    for corpse in world.corpses.iter().filter(|c| area.contains(c.pos)) {
        let (size, _) = unit_shape(corpse.kind);
        let ground = rect(corpse.pos - Vec2::splat(size * 0.7), Vec2::splat(size * 1.4));
        things.push((corpse.pos.x + corpse.pos.y, Prism(ground, 0.0, CORPSE_HEIGHT, mix(kind_color(corpse.kind), WALKER, 0.6))));
    }
    // Units in a tower aren't drawn – they show up on the tower's roof (`building_parts`).
    for unit in world.units.iter().filter(|u| !u.is_inside()) {
        let p = lerp(unit.prev, unit.pos, alpha);
        let color = kind_color(unit.kind);
        // Selected ones a bit lighter.
        let color = if selected.contains(&unit.id) { lighten(color) } else { color };
        let (size, height) = unit_shape(unit.kind);
        // The block turned the way the unit faces, and its weapon sticking out in front, at chest height.
        // (Drawing may use the normal `cos`/`sin` – only the simulation has to compute the same on every PC.)
        let facing = vec2(unit.facing.cos(), unit.facing.sin());
        let ground = bar(p - facing * size / 2.0, p + facing * size / 2.0, size);
        things.push((p.x + p.y, Prism(ground, 0.0, height, color)));
        if !unit.stats().heals {
            let tip = p + facing * (size / 2.0 + 0.2);
            let middle = (p + tip) / 2.0;
            let weapon = bar(p, tip, 0.07);
            things.push((middle.x + middle.y, Prism(weapon, height * 0.5, height * 0.62, WEAPON)));
        }
    }
    things.sort_by(|a, b| a.0.total_cmp(&b.0));

    for &(_, Prism(ground, bottom, top, color)) in &things {
        draw_prism(camera, ground, bottom, top, color);
    }

    // --- Shots and medic beams of this tick ---
    let lift = vec2(0.0, -0.3 * TILE_W * camera.zoom);
    for (from, to) in &world.shots {
        let a = camera.world_to_screen(*from) + lift;
        let b = camera.world_to_screen(*to) + lift * 0.6;
        draw_line(a.x, a.y, b.x, b.y, 1.5, SHOT);
    }
    for (from, to) in &world.beams {
        let a = camera.world_to_screen(*from) + lift;
        let b = camera.world_to_screen(*to) + lift;
        draw_line(a.x, a.y, b.x, b.y, 2.5, BEAM);
    }

    // --- Health bars above hurt and selected units, just above the block ---
    for unit in world.units.iter().filter(|u| !u.is_inside()) {
        let max = unit.stats().hp;
        if unit.hp == max && !view.always_bars && !selected.contains(&unit.id) {
            continue;
        }
        let frac = unit.hp as f32 / max as f32;
        let feet = camera.world_to_screen(lerp(unit.prev, unit.pos, alpha));
        let (size, height) = unit_shape(unit.kind);
        let w = 0.6 * TILE_W * camera.zoom;
        let y = feet.y - (height * TILE_W + size * TILE_H / 2.0 + 6.0) * camera.zoom;
        draw_rectangle(feet.x - w / 2.0, y, w, 3.0, BLACK);
        draw_rectangle(feet.x - w / 2.0, y, w * frac, 3.0, hp_color(frac));
    }

    // --- Life bars above damaged buildings, above the roof: hit points and shield in one bar (`hud::draw_life`) ---
    for building in &world.buildings {
        let stats = building.stats();
        let hurt = building.hp < stats.hp || building.shield < stats.max_shield();
        if !hurt && !view.always_bars {
            continue;
        }
        // Above the middle: from there up to the top corner of the diamond, then the height of the building.
        let (fw, fh) = building.footprint();
        let middle = camera.world_to_screen(building.center());
        let lift = ((fw + fh) as f32 * TILE_H / 4.0 + (stats.height + 0.15) * TILE_W) * camera.zoom;
        let w = fw.max(fh) as f32 * 0.5 * TILE_W * camera.zoom;
        let bar = Rect::new(middle.x - w / 2.0, middle.y - lift, w, 4.0);
        draw_life(bar, (building.hp, stats.hp), (building.shield, stats.max_shield()), BLACK, None);
    }

    // --- Spawner names on top of everything, so they can always be read ---
    for spawner in &view.spawners {
        let s = camera.world_to_screen(spawner.min + spawner.size / 2.0);
        let w = measure_text(&spawner.name, None, 18, 1.0).width;
        draw_text(&spawner.name, s.x - w / 2.0 + 1.0, s.y + 6.0 + 1.0, 18.0, BLACK);
        draw_text(&spawner.name, s.x - w / 2.0, s.y + 6.0, 18.0, if spawner.selected { SPAWNER_GLOW } else { WHITE });
    }
}

/// The fog over `area`. `cover` says what lies over a tile: nothing (visible), or a darkening colour.
/// Row by row: neighbouring tiles with the same cover become one long strip, so it's a few hundred strips
/// instead of 16,000 single diamonds.
fn draw_fog(camera: &Camera, area: Region, cover: impl Fn(i32, i32) -> Option<Color>) {
    let (lo, hi) = (area.min.x as i32, area.max().x as i32);
    for y in lo..hi {
        let mut x = lo;
        while x < hi {
            let here = cover(x, y);
            let start = x;
            while x < hi && cover(x, y) == here {
                x += 1;
            }
            if let Some(color) = here {
                draw_diamond(camera, vec2(start as f32, y as f32), vec2((x - start) as f32, 1.0), color);
            }
        }
    }
}

/// A single tree standing on the middle of its tile: a short trunk and a round crown above it.
fn draw_tree(camera: &Camera, center: Vec2) {
    let feet = camera.world_to_screen(center);
    if !on_screen(feet, 40.0) {
        return;
    }
    let unit = TILE_W * camera.zoom;
    let crown = feet - vec2(0.0, 0.42 * unit);
    draw_ellipse(feet.x, feet.y, 0.22 * unit, 0.11 * unit, 0.0, Color::new(0.0, 0.0, 0.0, 0.3)); // shadow
    draw_line(feet.x, feet.y, crown.x, crown.y, (0.06 * unit).max(1.0), TREE_TRUNK);
    draw_circle(crown.x, crown.y, 0.24 * unit, TREE_CROWN);
    draw_circle(crown.x - 0.06 * unit, crown.y - 0.07 * unit, 0.10 * unit, Color::new(0.22, 0.55, 0.24, 1.0));
}

/// A spawner on the ground: see-through area with an edge. The selected one's edge glows (pulsing).
fn draw_spawner(camera: &Camera, spawner: &SpawnerShape) {
    draw_diamond(camera, spawner.min, spawner.size, Color { a: 0.18, ..SPAWNER });
    let (min, max) = (spawner.min, spawner.min + spawner.size);
    let corners = [min, vec2(max.x, min.y), max, vec2(min.x, max.y)];
    let edge = |width: f32, color: Color| {
        for k in 0..4 {
            let (p, q) = (camera.world_to_screen(corners[k]), camera.world_to_screen(corners[(k + 1) % 4]));
            draw_line(p.x, p.y, q.x, q.y, width, color);
        }
    };
    if spawner.selected {
        // A wide faint line under a narrow bright one looks like a glow. It pulses about once per second.
        let pulse = 0.65 + 0.35 * (get_time() as f32 * 6.0).sin();
        edge(9.0, Color { a: 0.18 * pulse, ..SPAWNER_GLOW });
        edge(5.0, Color { a: 0.35 * pulse, ..SPAWNER_GLOW });
        edge(2.5, SPAWNER_GLOW);
    } else {
        edge(1.5, SPAWNER);
    }
}

/// Everything that stands on the map – a zombie, a unit, a building or a piece of one – is drawn as a body: its
/// outline on the ground (4 corners in order around it, any direction – so a wall piece can also run diagonally),
/// from height `bottom` to `top` (in tile widths), colour. Drawn sorted by depth.
#[derive(Clone, Copy)]
struct Prism([Vec2; 4], f32, f32, Color);

/// The 4 corners of a rectangle on the ground, in order around it.
fn rect(min: Vec2, size: Vec2) -> [Vec2; 4] {
    [min, min + vec2(size.x, 0.0), min + size, min + vec2(0.0, size.y)]
}

/// A bar on the ground from `a` to `b`, `width` wide (in tiles) – a piece of wall in any direction.
fn bar(a: Vec2, b: Vec2, width: f32) -> [Vec2; 4] {
    let dir = (b - a).normalize_or_zero();
    let side = vec2(-dir.y, dir.x) * width / 2.0;
    [a + side, b + side, b - side, a - side]
}

/// Does a wall on this tile join up with its neighbours: a wall, a tower or the post of a gate (not its door)?
fn joins(world: &World, x: i32, y: i32) -> bool {
    let at = world.building_at_point(vec2(x as f32 + 0.5, y as f32 + 0.5));
    at.is_some_and(|b| world.buildings[b].stats().joins) && !world.map.is_door(x, y)
}

/// The pieces of a building, each with its depth for sorting. Walls, gates and towers are a test skin
/// (DESIGN.md 12.3) that joins up with its neighbours like in TAB:
/// - a wall standing alone is one block,
/// - otherwise a post in the middle, and a piece of wall from it to every neighbour that joins – straight
///   to the edge of the tile, or diagonally to the corner if the neighbour only touches the corner. Two neighbours
///   meeting there make one line,
/// - a tower: a tall post plus pieces of wall, on its roof one small block per unit inside,
/// - a gate: two posts (joining outwards), a beam on top, and the door between them – open while one of your
///   units is close.
fn building_parts(world: &World, building: &Building, things: &mut Vec<(f32, Prism)>) {
    let mut part = |ground: [Vec2; 4], bottom: f32, top: f32, color: Color| {
        let middle = (ground[0] + ground[1] + ground[2] + ground[3]) / 4.0;
        things.push((middle.x + middle.y, Prism(ground, bottom, top, color)));
    };
    let color = building_color(building.kind);
    // Infected: the colour turns sickly – like the dark-green rot on TAB's infected buildings.
    let color = if building.infected { mix(color, INFECTED, 0.6) } else { color };
    let height = building.stats().height;
    // Pieces of wall look like a wall – also those of a tower or a gate.
    let (wall_height, wall_color) = (BuildingKind::Wall.stats().height, building_color(BuildingKind::Wall));
    // The pieces of wall from the middle of a tile to its neighbours. `skip`: not in this direction.
    // Returns whether there was any.
    let arms = |part: &mut dyn FnMut([Vec2; 4], f32, f32, Color), tile: (i32, i32), skip: (i32, i32)| {
        let (x, y) = tile;
        let middle = vec2(x as f32 + 0.5, y as f32 + 0.5);
        let mut any = false;
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            if (dx, dy) != skip && joins(world, x + dx, y + dy) {
                part(bar(middle, middle + vec2(dx as f32, dy as f32) * 0.5, WALL_WIDTH), 0.0, wall_height, wall_color);
                any = true;
            }
        }
        for (dx, dy) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
            // Only if the corner isn't already closed by a neighbour on the side (then it's an "L").
            if joins(world, x + dx, y + dy) && !joins(world, x + dx, y) && !joins(world, x, y + dy) {
                let corner = middle + vec2(dx as f32, dy as f32) * 0.5;
                // A little past the corner, so the two pieces overlap there instead of leaving a notch.
                let past = corner + vec2(dx as f32, dy as f32).normalize() * 0.12;
                part(bar(middle, past, WALL_WIDTH), 0.0, wall_height, wall_color);
                any = true;
            }
        }
        any
    };

    match building.kind {
        BuildingKind::Wall => {
            let middle = building.center();
            if arms(&mut part, building.tile, (0, 0)) {
                part(rect(middle - Vec2::splat(POST / 2.0), Vec2::splat(POST)), 0.0, height + 0.1, color);
            } else {
                part(rect(middle - Vec2::splat(LONE_POST / 2.0), Vec2::splat(LONE_POST)), 0.0, height, color);
            }
        }
        BuildingKind::Tower => {
            let middle = building.center();
            arms(&mut part, building.tile, (0, 0));
            part(rect(middle - Vec2::splat(0.4), Vec2::splat(0.8)), 0.0, height, color);
            // Who sits inside: a small block per unit on the roof, in the four corners.
            let spots = [vec2(-0.18, -0.18), vec2(0.18, -0.18), vec2(-0.18, 0.18), vec2(0.18, 0.18)];
            for (&i, spot) in world.garrison(building.id).iter().zip(spots) {
                let p = middle + spot;
                part(rect(p - Vec2::splat(0.11), Vec2::splat(0.22)), height, height + 0.25, kind_color(world.units[i].kind));
            }
        }
        BuildingKind::Gate => {
            let axis = if building.turned { (0, 1) } else { (1, 0) };
            let step = vec2(axis.0 as f32, axis.1 as f32);
            let (x, y) = building.tile;
            let first = vec2(x as f32 + 0.5, y as f32 + 0.5);
            let door = first + step;
            let last = first + step * 2.0;
            // The two posts, each joining outwards – not towards the door.
            arms(&mut part, (x, y), axis);
            arms(&mut part, (x + 2 * axis.0, y + 2 * axis.1), (-axis.0, -axis.1));
            for post in [first, last] {
                part(rect(post - Vec2::splat(0.3), Vec2::splat(0.6)), 0.0, height + 0.25, color);
            }
            // The door: closed it's a wooden board between the posts, open it's gone (only the beam above stays).
            let open = world.units.iter().any(|u| !u.is_inside() && u.pos.distance(door) < 1.3);
            if !open {
                part(bar(door - step * 0.5, door + step * 0.5, 0.18), 0.0, height, DOOR_COLOR);
            }
            part(bar(first + step * 0.3, last - step * 0.3, 0.25), height, height + 0.15, color);
        }
        BuildingKind::CommandCenter | BuildingKind::Tent => {
            part(rect(building.corner(), building.far_corner() - building.corner()), 0.0, height, color);
        }
    }
}

/// How a unit looks as a block: (edge length on the ground in tiles, height in tile widths).
pub fn unit_shape(kind: UnitKind) -> (f32, f32) {
    match kind {
        UnitKind::Soldier => (0.45, 0.55),
        UnitKind::Ranger => (0.34, 0.55),
        UnitKind::Sniper => (0.36, 0.65),
        UnitKind::Medic => (0.42, 0.50),
    }
}

/// A body standing on the ground (placeholder graphics) – a unit, a building or a piece of wall: the visible side
/// walls and the roof. `ground`: its outline, 4 corners in order around it; it reaches from height `bottom` to
/// `top` (tile widths – a beam over a gate floats). Isometric means: the ground becomes a diamond, and "up" is
/// simply up on screen.
fn draw_prism(camera: &Camera, ground: [Vec2; 4], bottom: f32, top: f32, color: Color) {
    let lift = |h: f32| vec2(0.0, -h * TILE_W * camera.zoom);
    let screen = ground.map(|p| camera.world_to_screen(p));
    if !screen.iter().any(|&p| on_screen(p, 200.0)) {
        return;
    }
    let middle = (ground[0] + ground[1] + ground[2] + ground[3]) / 4.0;
    let shade = |k: f32| Color::new(color.r * k, color.g * k, color.b * k, color.a); // see-through stays see-through (fading zombies)
    // The side walls. We look at the map from the +x +y side (the bottom of the screen), so only walls facing
    // that way can be seen. Facing +x (bottom right) they're lighter, facing +y (bottom left) darker.
    for k in 0..4 {
        let (a, b) = (ground[k], ground[(k + 1) % 4]);
        let edge = b - a;
        let mut out = vec2(edge.y, -edge.x); // at a right angle to the wall …
        if out.dot((a + b) / 2.0 - middle) < 0.0 {
            out = -out; // … and pointing outwards
        }
        if out.x + out.y <= 0.0 {
            continue; // faces away from us
        }
        let t = out.x.max(0.0) / (out.x.abs() + out.y.abs()); // 1 = facing +x, 0 = facing +y
        let (sa, sb) = (screen[k], screen[(k + 1) % 4]);
        let wall = shade(0.55 + 0.2 * t);
        draw_triangle(sa + lift(bottom), sb + lift(bottom), sb + lift(top), wall);
        draw_triangle(sa + lift(bottom), sb + lift(top), sa + lift(top), wall);
    }
    let roof = screen.map(|p| p + lift(top));
    draw_triangle(roof[0], roof[1], roof[2], color);
    draw_triangle(roof[0], roof[2], roof[3], color);
}

/// The colour of a terrain – on the map, on the minimap and for the editor brush.
pub fn terrain_color(terrain: Terrain) -> Color {
    match terrain {
        Terrain::Grass => GROUND,
        Terrain::Mountain => Color::new(0.33, 0.31, 0.29, 1.0),
        Terrain::Forest => Color::new(0.10, 0.30, 0.12, 1.0),
        Terrain::Tree => TREE_CROWN,
        Terrain::Water => Color::new(0.16, 0.34, 0.58, 1.0),
    }
}

/// The colour of an ore patch – on the map, on the minimap and in the previews.
pub fn ore_color(ore: Ore) -> Color {
    match ore {
        Ore::Iron => Color::new(0.55, 0.33, 0.24, 1.0),
        Ore::Coal => Color::new(0.10, 0.10, 0.11, 1.0),
        Ore::Stone => Color::new(0.62, 0.60, 0.54, 1.0),
    }
}

/// The colour of a building type – on the map, on the minimap and in the selection.
pub fn building_color(kind: BuildingKind) -> Color {
    match kind {
        BuildingKind::CommandCenter => Color::new(0.32, 0.45, 0.70, 1.0),
        BuildingKind::Tent => Color::new(0.62, 0.48, 0.30, 1.0),
        BuildingKind::Wall => Color::new(0.58, 0.57, 0.53, 1.0),
        BuildingKind::Gate => Color::new(0.52, 0.50, 0.46, 1.0),
        BuildingKind::Tower => Color::new(0.66, 0.64, 0.58, 1.0),
    }
}

/// The colour of a unit type – on the map, on the minimap and in the selection.
pub fn kind_color(kind: UnitKind) -> Color {
    match kind {
        UnitKind::Soldier => Color::new(0.35, 0.60, 0.95, 1.0),
        // Green like a hunter in the woods.
        UnitKind::Ranger => Color::new(0.40, 0.80, 0.50, 1.0),
        UnitKind::Sniper => Color::new(0.75, 0.50, 0.95, 1.0),
        // White like StarCraft's medic (DESIGN.md 6).
        UnitKind::Medic => Color::new(0.92, 0.92, 0.90, 1.0),
    }
}

/// The colour of a zombie breed on the map.
fn breed_color(breed: Breed) -> Color {
    match breed {
        Breed::Walker => WALKER,
        Breed::Runner => RUNNER,
        Breed::Fat => FAT,
    }
}

fn lighten(c: Color) -> Color {
    mix(c, WHITE, 0.5)
}

/// `share` of the way from colour `a` to colour `b` (0: `a`, 1: `b`). Keeps the transparency of `a`.
fn mix(a: Color, b: Color, share: f32) -> Color {
    Color::new(a.r + (b.r - a.r) * share, a.g + (b.g - a.g) * share, a.b + (b.b - a.b) * share, a.a)
}

/// The colour of a kind of command: move green, attack red, patrol yellow, heal light green.
pub fn mark_color(mark: Mark) -> Color {
    match mark {
        Mark::Move => GREEN,
        Mark::Attack => RED,
        Mark::Patrol => YELLOW,
        Mark::Heal => BEAM,
    }
}

/// Text in the top left, on a dark box – as wide as the longest line.
pub fn draw_debug(lines: &[String]) {
    let line_height = 20.0;
    let width = lines.iter().map(|line| measure_text(line, None, 20, 1.0).width).fold(0.0, f32::max);
    draw_rectangle(8.0, 8.0, width + 16.0, lines.len() as f32 * line_height + 12.0, Color::new(0.0, 0.0, 0.0, 0.6));
    for (i, line) in lines.iter().enumerate() {
        draw_text(line, 16.0, 28.0 + i as f32 * line_height, 20.0, WHITE);
    }
}

/// A rectangle of the world (corner + size in tiles) as a diamond on screen.
pub fn draw_diamond(camera: &Camera, corner: Vec2, size: Vec2, color: Color) {
    let a = camera.world_to_screen(corner);
    let b = camera.world_to_screen(corner + vec2(size.x, 0.0));
    let c = camera.world_to_screen(corner + size);
    let d = camera.world_to_screen(corner + vec2(0.0, size.y));
    draw_triangle(a, b, c, color);
    draw_triangle(a, c, d, color);
}

fn on_screen(p: Vec2, margin: f32) -> bool {
    p.x > -margin && p.y > -margin && p.x < screen_width() + margin && p.y < screen_height() + margin
}

fn lerp(from: Vec2, to: Vec2, t: f32) -> Vec2 {
    from + (to - from) * t
}
