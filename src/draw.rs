//! Drawing. Only reads the world, never changes it.
//! Zombies, units and buildings are pictures (`sprites.rs`, `building_sprites.rs`) where their kind has one, otherwise
//! boxes. Placeholder graphics for the rest: diamonds for the ground, blocks and joined-up walls for buildings without
//! pictures (DESIGN.md 12.3). In the game view the fog of war lies over everything nobody sees.

use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

use macroquad::prelude::*;

use crate::building_sprites::{Aim, Appearance, BuildingSprite, State};
use crate::critters::Critters;
use crate::data;
use crate::effects::Effects;
use crate::ground::{Decor, Floor, Landscape};
use crate::hud::{draw_life, hp_color};
use crate::iso::{Camera, FLAT, TILE_H, TILE_W};
use crate::kinds::{BuildingKind, UnitKind};
use crate::map::{to_frame, Deposit, Map, Ore, Region, Terrain};
use crate::picture;
use crate::settings::HealthBars;
use crate::sim::{self, Breed, Building, DoomHouse, LootKind, Mark, Unit, TICK, UNIT_RADIUS, World};
use crate::sprites::{Pose, Sprite, Sprites};
use crate::vision::Vision;

// `pub`, because the minimap (hud.rs) uses the same colours.
pub const GROUND: Color = Color::new(0.20, 0.25, 0.18, 1.0);
/// Zombies on the minimap and in the editor's markings – on the map each breed has its own grey (`breed_color`).
pub const ZOMBIE: Color = Color::new(0.55, 0.75, 0.35, 1.0);
/// A zombie is a box (placeholder skin like the units): its edge on the ground is this many times its `radius` …
const ZOMBIE_SIZE: f32 = 1.12;
/// … and a normal one (radius 0.125) is this tall, in tile widths – a bigger one a bit taller.
const ZOMBIE_HEIGHT: f32 = 0.3;
/// Walkers dark grey, runners light grey, fat ones a sickly yellowish brown, the armored ones steel blue, spitters
/// acid green, the infested a fleshy purple.
const WALKER: Color = Color::new(0.30, 0.30, 0.32, 1.0);
const RUNNER: Color = Color::new(0.74, 0.74, 0.76, 1.0);
const FAT: Color = Color::new(0.58, 0.52, 0.34, 1.0);
const ARMORED: Color = Color::new(0.40, 0.47, 0.60, 1.0);
const SPITTER: Color = Color::new(0.50, 0.74, 0.20, 1.0);
const INFESTED: Color = Color::new(0.64, 0.34, 0.50, 1.0);
/// A harpy: sickly yellow, easy to pick out of the crowd – it's the one to shoot first.
const HARPY: Color = Color::new(0.90, 0.82, 0.35, 1.0);
const GIANT: Color = Color::new(0.45, 0.22, 0.18, 1.0);
/// The spitters' acid in flight, and the infested's shots.
const ACID: Color = Color::new(0.62, 0.95, 0.20, 1.0);
const ZOMBIE_SHOT: Color = Color::new(0.85, 0.45, 0.75, 0.8);
/// A house of a village of doom: dark rotten wood, its roof dark red – brighter once it's awake.
const DOOM_WALL: Color = Color::new(0.27, 0.20, 0.17, 1.0);
const DOOM_ROOF: Color = Color::new(0.38, 0.08, 0.07, 1.0);
const DOOM_AWAKE: Color = Color::new(0.75, 0.12, 0.08, 1.0);
/// The fields a farm plants on the grass around it.
const FIELD: Color = Color::new(0.66, 0.60, 0.28, 0.85);
/// What a unit holds (rifle, bow): a thin bar sticking out where it faces – so you see it turn.
const WEAPON: Color = Color::new(0.15, 0.15, 0.17, 1.0);
/// What an infected building turns towards.
const INFECTED: Color = Color::new(0.30, 0.42, 0.18, 1.0);
/// What a switched off building fades towards: dark grey, like a machine standing still.
const SWITCHED_OFF: Color = Color::new(0.12, 0.12, 0.12, 1.0);
/// The top of a tesla tower – and the reach of the power grid while placing a building that needs it (`ui.rs`).
pub const TESLA_GLOW: Color = Color::new(0.55, 0.85, 1.0, 1.0);
/// The mercenaries' gold – in their colour and their mark.
const MERC_GOLD: Color = Color::new(0.95, 0.78, 0.28, 1.0);
/// The mark of a veteran (`sim/veterans.rs`).
const VETERAN_SILVER: Color = Color::new(0.85, 0.88, 0.92, 1.0);
/// The dark band round a barrel.
const BARREL_BAND: Color = Color::new(0.25, 0.10, 0.08, 1.0);
/// A fallen unit lies flat on the ground until it rises.
const CORPSE_HEIGHT: f32 = 0.06;
const HOLD_RING: Color = ORANGE;
/// A ring under a figure is at least this wide (pixels, half its width) – zoomed out it would vanish.
const MIN_RING: f32 = 5.0;
const SHOT: Color = Color::new(1.0, 0.9, 0.4, 0.8);
const BEAM: Color = Color::new(0.35, 1.0, 0.45, 0.9);
/// The world border line in editor mode.
const BORDER_LINE: Color = Color::new(1.0, 0.55, 0.15, 0.9);
/// Behind the map – what the window shows where there is no map (`game.rs` clears the screen with it).
pub const BACKGROUND: Color = Color::new(20.0 / 255.0, 22.0 / 255.0, 28.0 / 255.0, 1.0);
/// Fog of war: never seen – black like in StarCraft 2 (also on the minimap) …
pub const UNEXPLORED: Color = Color::new(0.0, 0.0, 0.0, 1.0);
/// … seen before, but nobody is looking right now – darkened.
/// How bright the map is without a light on it: TAB's light for the fog (`ZXMapTheme.GlobalLightColorForFog`, 180 of
/// 255). Its Dark Moorland has 200 – see `FogPicture::paint`.
const EXPLORED_LIGHT: f32 = 180.0 / 255.0;
const EXPLORED_LIGHT_BR: f32 = 200.0 / 255.0;
/// Everyone who sees carries a soft light (TAB's `CMapWatcher`): its radius is this many times the sight range …
const LIGHT_REACH: f32 = 1.1 * std::f32::consts::SQRT_2;
/// … and this is how bright it is from its middle out, in steps of 5 % of that radius (TAB's picture `Light_A`). Further
/// out it's darker than the map anyway, so the curve can stop at half the radius.
const LIGHT_CURVE: [f32; 11] = [1.0, 1.0, 1.0, 1.0, 1.0, 0.988, 0.969, 0.925, 0.847, 0.776, 0.647];
const LIGHT_STEP: f32 = 0.05;
/// TAB moves its fog this far from the tile it belongs to (tiles, `_OMapFog.Location`): a bit up the screen.
const FOG_SHIFT: f32 = 0.7;
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
/// A house of doom with TAB's picture is drawn in this colour while it stirs.
const DOOM_STIRRING: Color = Color::new(1.0, 0.72, 0.66, 1.0);
/// Where the units in a tower with TAB's picture stand on it – around the middle, the ones behind first.
const TOWER_SPOTS: [Vec2; 4] = [vec2(-0.12, -0.12), vec2(0.12, -0.12), vec2(-0.12, 0.12), vec2(0.12, 0.12)];
/// A building being built is mixed with this – fresh timber, scaffolding.
const SCAFFOLD: Color = Color::new(0.85, 0.75, 0.55, 1.0);

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
    /// Which units and buildings get a health bar (setting).
    pub bars: HealthBars,
    /// The `id`s of the selected buildings – for the bars.
    pub selected_buildings: &'a [u32],
    /// TAB's grid view (setting): lines between the tiles, mountains and forests coloured instead of drawn.
    pub grid: bool,
    /// Whose eyes: fog of war like in the game, the zombies' view, or none (editor).
    pub fog: Fog<'a>,
    /// That fog, painted for this frame (`FogPicture::paint`).
    pub fog_picture: &'a FogPicture,
    /// What the picture remembers about each zombie: how visible it still is, which way it faces.
    pub looks: &'a Looks,
    /// The pictures of zombies and units.
    pub sprites: &'a Sprites,
    /// TAB's landscape pictures (`ground.rs`) – `Some` if there are some for this map's look, else the coloured
    /// diamonds.
    pub landscape: Option<&'a Landscape>,
    /// The spawners to show (only in editor mode) – with the size they have while being dragged.
    pub spawners: Vec<SpawnerShape>,
    /// The zombies as coloured tiles instead of figures (editor, zoomed out – `zombie_tiles`): far quicker with
    /// thousands of them, and you see at a glance where which breed stands.
    pub zombie_tiles: bool,
}

/// How long a zombie takes to fade in when it comes into your sight, and to fade out after it leaves it (seconds) – like
/// in TAB, nobody pops up at the edge of sight.
const FADE_IN: f32 = 0.6;
const FADE_TIME: f32 = 0.8;

/// A zombie turns this fast towards where it walks (radians per second) – so it doesn't flicker between directions
/// when the crowd jostles it.
const ZOMBIE_TURN: f32 = TAU;
/// It counts as walking when it wants to walk at least this share of its speed (`Zombie::own_step`) – low, so a
/// roaming walker wading through stakes still walks.
const WALKING: f32 = 0.05;

/// What the picture remembers about every zombie from frame to frame. Only for the picture, not part of the
/// simulation, so a `HashMap` is fine here (DESIGN.md 12.8 only forbids it in the simulation).
#[derive(Default)]
pub struct Looks {
    zombies: HashMap<u32, Look>,
    /// TAB's effects playing, and where the turning guns point (`effects.rs`).
    pub effects: Effects,
    /// Ravens and rats – only in the picture (`critters.rs`).
    critters: Critters,
}

#[derive(Clone, Copy, Default)]
pub struct Look {
    /// How visible it still is in the player's view: 1 = seen right now, falling to 0 once nobody sees it any more –
    /// so it fades out instead of vanishing at once.
    pub alpha: f32,
    /// Which way it faces (radians, like `Unit::facing`): turning towards where it walks, kept while it stands.
    pub facing: f32,
    pub walking: bool,
    /// How fast it wants to walk (tiles per second, `Zombie::own_step`) – a slow one walks, a fast one runs (`sprites.rs`).
    pub speed: f32,
    /// Since when it growls (seconds of game time) – its growl is played once from there. `None`: it doesn't.
    pub growl_from: Option<f32>,
}

impl Looks {
    /// Once per frame (`dt` seconds, `time`: seconds of game time): seen zombies fade in, the others fade out; walking
    /// ones turn. Gone: forgotten. A zombie the picture hasn't met yet starts invisible – it fades in too.
    pub fn update(&mut self, world: &World, dt: f32, time: f32) {
        let mut next = HashMap::with_capacity(world.zombies.len());
        for zombie in &world.zombies {
            // A new one: not seen yet, and facing a way of its own – so a horde standing still doesn't all look the
            // same way.
            let new = Look { facing: zombie.id as f32 * 2.4, ..Look::default() };
            let old = self.zombies.get(&zombie.id).copied().unwrap_or(new);
            let alpha = if world.vision.sees(zombie.pos) {
                (old.alpha + dt / FADE_IN).min(1.0)
            } else {
                (old.alpha - dt / FADE_TIME).max(0.0)
            };
            // What it walks by itself – being pushed by the crowd neither makes it walk nor its feet faster, and doesn't
            // turn it (like TAB: the animation follows what the zombie does, not where it's shoved).
            let step = zombie.own_step();
            let walking = step.length() > zombie.speed() * TICK * WALKING;
            let facing = if walking { turn(old.facing, step.y.atan2(step.x), ZOMBIE_TURN * dt) } else { old.facing };
            let speed = step.length() / TICK;
            // Just started growling: from now. Still growling: since then. `or`: the first `Some` of the two.
            let growl_from = zombie.is_growling().then(|| old.growl_from.unwrap_or(time));
            next.insert(zombie.id, Look { alpha, facing, walking, speed, growl_from });
        }
        // The dying keep the way they faced, and fade like the living when nobody sees them.
        for body in &world.dying {
            let old = self.zombies.get(&body.id).copied().unwrap_or_default();
            let alpha = if world.vision.sees(body.pos) { old.alpha } else { (old.alpha - dt / FADE_TIME).max(0.0) };
            next.insert(body.id, Look { alpha, walking: false, growl_from: None, ..old });
        }
        self.zombies = next;
        self.effects.update(world, time, dt);
        self.critters.update(world, time);
    }

    pub fn get(&self, id: u32) -> Look {
        self.zombies.get(&id).copied().unwrap_or_default()
    }
}

/// From the angle `from` towards `to` by at most `max` (radians) – the short way round.
fn turn(from: f32, to: f32, max: f32) -> f32 {
    let diff = (to - from + PI).rem_euclid(TAU) - PI;
    (from + diff.clamp(-max, max)).rem_euclid(TAU)
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
    /// Where it lies – its four corners on the ground, in order around it (straight on screen or slanted).
    pub corners: [Vec2; 4],
    pub name: String,
    /// Selected: the edge glows.
    pub selected: bool,
}

/// `selected`: the `id`s of the selected units.
/// `alpha` (0..1): how far we are between the previous and the current tick.
pub fn draw_world(world: &World, camera: &Camera, view: &View, selected: &[u32], alpha: f32) {
    let area = view.area;

    // --- Ground: TAB's pictures (`ground.rs`) – or one big rectangle for the visible area, terrain as small diamonds
    // on top ---
    match view.landscape {
        Some(landscape) => landscape.draw_ground(camera, area),
        None => draw_quad(camera, area.corners(), GROUND),
    }
    // Every tile that shows at least a bit of itself inside the area – what sticks out over its straight edge is
    // covered at the end (`cover_outside`).
    let rows: Vec<(i32, std::ops::Range<i32>)> = area.rows(1.0).collect();
    for (y, xs) in rows.iter().cloned() {
        for x in xs {
            let terrain = world.map.terrain(x, y);
            let deposit = world.map.deposit(x, y);
            if terrain == Terrain::Grass && deposit.is_none() {
                continue;
            }
            // With TAB's pictures only what the look has no picture for is still drawn like this: coal (older
            // landscapes without our coal lumps) and oil (no oil source).
            if let Some(landscape) = view.landscape {
                let ours = |d: Deposit| (d.ore == Ore::Coal && !landscape.draws_coal()) || (d.ore == Ore::Oil && !landscape.draws_oil());
                if !deposit.is_some_and(ours) {
                    continue;
                }
            }
            let corner = vec2(x as f32, y as f32);
            if !on_screen(camera.world_to_screen(corner + 0.5), 40.0) {
                continue;
            }
            // Ore on grass: the tile in the ore's colour, a smaller lighter diamond in the middle as a "lump". Oil: one
            // big dark pool over the whole node, drawn with its last tile (so the other three don't paint over it –
            // nodes never touch, so that's the one with no oil below or to the right of it).
            if let Some(deposit) = deposit {
                let color = ore_color(deposit.ore);
                draw_diamond(camera, corner, Vec2::ONE, color);
                if deposit.ore != Ore::Oil {
                    draw_diamond(camera, corner + 0.3, Vec2::splat(0.4), lighten(color));
                } else if !is_oil(world, x + 1, y) && !is_oil(world, x, y + 1) {
                    draw_diamond(camera, corner - 0.55, Vec2::splat(1.1), Color::new(0.06, 0.04, 0.08, 1.0));
                    draw_diamond(camera, corner - 0.25, Vec2::splat(0.3), lighten(color));
                }
                continue;
            }
            draw_diamond(camera, corner, Vec2::ONE, terrain_color(terrain));
            // Forest: a darker smaller diamond in the middle as a "treetop", so it doesn't look like grass.
            if terrain == Terrain::Forest {
                draw_diamond(camera, corner + 0.25, Vec2::splat(0.5), Color::new(0.06, 0.20, 0.08, 1.0));
            }
        }
    }
    // The paved floor under the buildings (`ground::Floor`), then the flat things on the ground (herbs – not under a
    // floor –, ore lumps) – before the fog, which darkens them like the ground.
    if let Some(landscape) = view.landscape {
        let floors = floors(world, view);
        landscape.draw_floors(camera, &floors);
        landscape.decor_in(area, false, |d| {
            if !d.is_paved(&floors) {
                landscape.draw_decor(camera, d, WHITE)
            }
        });
    }
    // Game time (seconds) for the effects and animations: stands still while paused, runs faster at higher speed.
    let now = (world.tick as f32 + alpha) * TICK;
    // The shimmer on the water (`effects.rs`) – with TAB's pictures, not in the grid view.
    if view.landscape.is_some() && !view.grid {
        view.looks.effects.draw_water(world, area, camera, now);
    }
    // The fields of the farms: every grass tile a farm works, a field (`World::worked_tiles`). Before the fog, so it
    // darkens them like the ground.
    // With TAB's wheat (`CFarmer`) the fields are pictures standing among the rest – further down.
    let wheat = view.sprites.decor("wheat").filter(|_| !view.grid);
    // TAB's treasure ruins: lying on the ground round their loot, in the look of the landscape.
    let ruin_look = match world.look.as_str() {
        look @ ("TM" | "AL" | "DS" | "VO") => look.to_lowercase(),
        _ => "br".to_owned(),
    };
    if let Some(ruin) = view.sprites.decor(&format!("treasure_ruin_{ruin_look}")) {
        for ornament in world.ornaments.iter().filter(|o| o.kind == sim::OrnamentKind::Ruin) {
            // Its picture in the middle of its 4 × 4 tiles.
            let width = ruin.size();
            let at = vec2(ornament.tile.0 as f32, ornament.tile.1 as f32) + Vec2::splat(2.0 + width / 2.0);
            if area.contains(at) && seen(world, &view.fog, at - width / 2.0)
                && let Some(body) = ruin.body()
            {
                ruin.draw(camera, body, State::Working, 0.0, Aim::default(), at, width, WHITE);
            }
        }
    }
    for (b, farm) in world.buildings.iter().enumerate().filter(|(_, b)| b.kind == BuildingKind::Farm && !b.infected && wheat.is_none()) {
        for (x, y) in world.worked_tiles(farm.kind, farm.tile, b) {
            if area.contains_tile(x, y) {
                draw_diamond(camera, vec2(x as f32 + 0.08, y as f32 + 0.08), Vec2::splat(0.84), FIELD);
            }
        }
    }
    // The zombies as tiles (editor, zoomed out): in the grid view they lie in it like TAB's painted cells – on its
    // flat colours, under its lines. Over TAB's pictures they're drawn over everything that stands (below), so trees
    // and houses don't hide them.
    let zombie_tiles = if view.zombie_tiles { zombie_tiles(world, view) } else { Vec::new() };
    // The grid lies on the ground like the fields: the fog darkens it, and whatever stands there covers it.
    if view.grid {
        draw_grid(camera, &world.map, area, &zombie_tiles);
    }
    // Blood and acid lying on the ground (`effects.rs`) – where you see it.
    let shows = |p: Vec2| !matches!(view.fog, Fog::Player) || world.vision.sees(p);
    view.looks.effects.draw_stains(camera, now, shows);
    for spawner in &view.spawners {
        draw_spawner(camera, spawner);
    }
    // Editor: where the world border runs.
    if view.editing {
        let corners = world.map.play_area().corners();
        for k in 0..4 {
            let (p, q) = (camera.world_to_screen(corners[k]), camera.world_to_screen(corners[(k + 1) % 4]));
            draw_line(p.x, p.y, q.x, q.y, 2.5, BORDER_LINE);
        }
    }

    // --- Selection rings and command queues lie on the ground, so draw them before the figures ---
    // A selected tower that shoots by itself: its range, like TAB's.
    for building in world.buildings.iter().filter(|b| view.selected_buildings.contains(&b.id)) {
        if let Some(weapon) = &building.stats().weapon {
            view.looks.effects.draw_range(camera, building.center(), weapon.range, now);
        }
    }
    for (i, unit) in world.units.iter().enumerate() {
        if !selected.contains(&unit.id) {
            continue;
        }
        let p = lerp(unit.prev, unit.pos, alpha);
        let s = camera.world_to_screen(p);
        draw_ring(camera, p, UNIT_RADIUS, 1.5, if unit.is_holding() { HOLD_RING } else { GREEN });

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
    let mut things: Vec<(f32, Thing)> = Vec::with_capacity(world.zombies.len() + world.units.len());
    // Game time (seconds) for the animations: stands still while paused, runs faster at higher speed.
    let time = (world.tick as f32 + alpha) * TICK;
    for zombie in world.zombies.iter().filter(|z| !view.zombie_tiles && area.contains(z.pos)) {
        let look = view.looks.get(zombie.id);
        // In the player's view only the ones you see – and those still fading out.
        let visible = if matches!(view.fog, Fog::Player) { look.alpha } else { 1.0 };
        if visible <= 0.0 {
            continue;
        }
        let p = lerp(zombie.prev, zombie.pos, alpha);
        // A harpy leaping over a wall: up in an arc. On screen "up" is towards the top left of the map – the figure is
        // drawn that much further back, but sorted by where it stands.
        let lift = zombie.leap_height(alpha);
        let color = breed_color(zombie.breed);
        if let Some(sprite) = view.sprites.breed(zombie.breed, zombie.look as u32) {
            let pose = match (zombie.since_hit(), look.growl_from) {
                (Some(share), _) => Pose::Attack(Some(share)),
                _ if look.walking => Pose::Walk(look.speed),
                (None, Some(from)) => Pose::Special(time - from),
                (None, None) => Pose::Idle,
            };
            let color = Color { a: visible, ..if sprite.tinted() { color } else { WHITE } };
            let at = p - Vec2::splat(2.0 * lift);
            let figure = Figure { sprite, at, facing: look.facing, pose, clock: clock(time, zombie.id), color };
            things.push((p.x + p.y, Thing::Figure(figure)));
            continue;
        }
        let size = zombie.radius * ZOMBIE_SIZE;
        let height = ZOMBIE_HEIGHT * (0.5 + 4.0 * zombie.radius);
        let ground = rect(p - Vec2::splat(size / 2.0), Vec2::splat(size));
        things.push((p.x + p.y, Thing::Body(Prism(ground, lift, lift + height, Color { a: visible, ..color }))));
    }
    // The sleeping ones (`sim/sleep.rs`) only where there's no fog (editor): standing still, a little darker. In the
    // player's view they're never seen – they wake before your eyes get there.
    if !matches!(view.fog, Fog::Player) && !view.zombie_tiles {
        for sleeper in world.sleep.iter().filter(|s| area.contains(s.pos)) {
            let color = darken(breed_color(sleeper.breed), 0.7);
            // A way to face of its own, from where it lies – the same every frame.
            let facing = (sleeper.pos.x * 7.0 + sleeper.pos.y * 3.0).rem_euclid(TAU);
            if let Some(sprite) = view.sprites.breed(sleeper.breed, sleeper.look as u32) {
                let color = if sprite.tinted() { color } else { WHITE };
                let figure = Figure { sprite, at: sleeper.pos, facing, pose: Pose::Idle, clock: 0.0, color };
                things.push((sleeper.pos.x + sleeper.pos.y, Thing::Figure(figure)));
                continue;
            }
            let radius = sleeper.breed.stats().radius;
            let ground = rect(sleeper.pos - Vec2::splat(radius * ZOMBIE_SIZE / 2.0), Vec2::splat(radius * ZOMBIE_SIZE));
            let body = Prism(ground, 0.0, ZOMBIE_HEIGHT * (0.5 + 4.0 * radius), color);
            things.push((sleeper.pos.x + sleeper.pos.y, Thing::Body(body)));
        }
    }
    let mut parts = Vec::new();
    for building in &world.buildings {
        if !building_pictures(world, building, view, time, &mut things) {
            building_parts(world, building, &mut parts);
        }
    }
    // Neutral buildings and units (`sim/neutral.rs`): grey like a building switched off – once seen, like the houses of
    // doom.
    for neutral in world.neutral.iter().filter(|n| area.contains(n.center())) {
        if !seen(world, &view.fog, neutral.center()) {
            continue;
        }
        match neutral.thing {
            sim::Thing::Building { kind, tile, turned } => {
                let mut building = Building::standing(0, kind, tile, turned);
                building.off = true;
                if !building_pictures(world, &building, view, time, &mut things) {
                    building_parts(world, &building, &mut parts);
                }
            }
            sim::Thing::Unit { kind, pos } => {
                let p = vec2(pos.0, pos.1);
                let (size, height) = unit_shape(kind);
                let color = mix(kind_color(kind), SWITCHED_OFF, 0.6);
                parts.push((p.x + p.y, Prism(rect(p - Vec2::splat(size / 2.0), Vec2::splat(size)), 0.0, height, color)));
            }
        }
    }
    // The houses of the villages of doom and the loot: once seen, they stay on the map like the terrain – darkened where
    // nobody looks right now, not at all where nobody has ever looked.
    for house in world.doom.iter().filter(|d| area.contains(d.center())) {
        if !seen(world, &view.fog, house.center()) {
            continue;
        }
        // TAB's house, its flag waving – reddish while it stirs (ours: so you see a batch come out).
        if let Some(sprite) = view.sprites.doom(house.nest(), tile_look(house.tile))
            && let Some(body) = sprite.body()
        {
            let color = if house.is_stirring(world.tick) { DOOM_STIRRING } else { WHITE };
            let width = house.size.0 as f32;
            let at = house.far_corner();
            let ticking = clock(time, house.id);
            let picture = Picture { sprite, looks: body, state: State::Working, clock: ticking, aim: Aim::default(), at, width, color };
            let middle = house.center();
            things.push((middle.x + middle.y, Thing::Picture(picture)));
            continue;
        }
        doom_parts(house, world.tick, &mut parts);
    }
    // Wheat on the farm's fields: every grass tile it works, not under it – 70 % opaque, like TAB's.
    if let Some(wheat) = wheat
        && let Some(body) = wheat.body()
    {
        for (b, farm) in world.buildings.iter().enumerate().filter(|(_, b)| b.kind == BuildingKind::Farm && b.works()) {
            for (x, y) in world.worked_tiles(farm.kind, farm.tile, b) {
                let at = vec2(x as f32 + 1.0, y as f32 + 1.0);
                if area.contains_tile(x, y) {
                    let picture = Picture { sprite: wheat, looks: body, state: State::Working, clock: 0.0, aim: Aim::default(), at, width: 1.0, color: Color::new(1.0, 1.0, 1.0, 0.7) };
                    things.push((at.x + at.y - 1.0, Thing::Picture(picture)));
                }
            }
        }
    }
    // TAB's power line towers in the forest – gone in the grid view, like the trees.
    for ornament in world.ornaments.iter().filter(|_| !view.grid) {
        let name = match ornament.kind {
            sim::OrnamentKind::PowerTower => "power_tower_high",
            sim::OrnamentKind::PowerTowerFlip => "power_tower_high_flip",
            sim::OrnamentKind::Ruin => continue,
        };
        let middle = vec2(ornament.tile.0 as f32 + 0.5, ornament.tile.1 as f32 + 0.5);
        if let Some(tower) = view.sprites.decor(name)
            && let Some(body) = tower.body()
            && area.contains(middle)
        {
            let width = tower.size();
            let at = middle + Vec2::splat(width / 2.0);
            let picture = Picture { sprite: tower, looks: body, state: State::Working, clock: 0.0, aim: Aim::default(), at, width, color: WHITE };
            things.push((middle.x + middle.y, Thing::Picture(picture)));
        }
    }
    for loot in world.loot.iter().filter(|l| area.contains(l.pos)) {
        // TAB's picture of it (coal is TAB's oil barrel) – else a little crate.
        let name = match loot.kind {
            LootKind::Gold => "loot_gold",
            LootKind::Wood => "loot_wood",
            LootKind::Stone => "loot_stone",
            LootKind::Iron => "loot_iron",
            LootKind::Coal => "loot_oil_1",
        };
        if let Some(sprite) = view.sprites.decor(name)
            && let Some(body) = sprite.body()
            && seen(world, &view.fog, loot.pos)
        {
            let at = loot.pos + 0.5;
            let picture = Picture { sprite, looks: body, state: State::Working, clock: 0.0, aim: Aim::default(), at, width: 1.0, color: WHITE };
            things.push((loot.pos.x + loot.pos.y, Thing::Picture(picture)));
            continue;
        }
        if seen(world, &view.fog, loot.pos) {
            let color = loot_color(loot.kind);
            let p = loot.pos;
            parts.push((p.x + p.y, Prism(rect(p - Vec2::splat(0.18), Vec2::splat(0.36)), 0.0, 0.22, color)));
            parts.push((p.x + p.y + 0.01, Prism(rect(p - Vec2::splat(0.1), Vec2::splat(0.2)), 0.22, 0.27, lighten(color))));
        }
    }
    things.extend(parts.into_iter().map(|(depth, part)| (depth, Thing::Body(part))));
    // Trees and rocks (`ground.rs`): sorted in like everything else. Always there, like TAB's – the fog lies over them,
    // so they never pop up tile by tile. In the grid view gone, like TAB's: their tiles are coloured instead
    // (`draw_grid`), and you see what stands among them.
    if let Some(landscape) = view.landscape.filter(|_| !view.grid) {
        landscape.decor_in(area, true, |d| things.push((d.at.x + d.at.y, Thing::Decor(landscape, *d, WHITE))));
    }
    // Killed zombies dying (`sim/fallen.rs`) – only as a picture with a death, as before they're just gone otherwise.
    for body in world.dying.iter().filter(|d| !view.zombie_tiles && area.contains(d.pos)) {
        let look = view.looks.get(body.id);
        let visible = if matches!(view.fog, Fog::Player) { look.alpha } else { 1.0 };
        let Some(sprite) = view.sprites.breed(body.breed, body.look as u32).filter(|s| s.dies()) else {
            continue;
        };
        if visible <= 0.0 {
            continue;
        }
        let p = lerp(body.prev, body.pos, alpha);
        let color = Color { a: visible, ..if sprite.tinted() { breed_color(body.breed) } else { WHITE } };
        let pose = Pose::Die(time - body.died as f32 * TICK);
        let figure = Figure { sprite, at: p, facing: look.facing, pose, clock: 0.0, color };
        things.push((p.x + p.y, Thing::Figure(figure)));
    }
    // Fallen units: their death, then lying in its last picture. Without one: flat and darker, a bit bigger than
    // standing.
    for corpse in world.corpses.iter().filter(|c| area.contains(c.pos)) {
        if let Some(sprite) = view.sprites.unit(corpse.kind, corpse.id).filter(|s| s.dies()) {
            let p = lerp(corpse.prev, corpse.pos, alpha);
            let color = if sprite.tinted() { kind_color(corpse.kind) } else { WHITE };
            let pose = Pose::Die(time - corpse.died as f32 * TICK);
            let figure = Figure { sprite, at: p, facing: corpse.facing, pose, clock: 0.0, color };
            things.push((p.x + p.y, Thing::Figure(figure)));
            continue;
        }
        let (size, _) = unit_shape(corpse.kind);
        let ground = rect(corpse.pos - Vec2::splat(size * 0.7), Vec2::splat(size * 1.4));
        let body = Prism(ground, 0.0, CORPSE_HEIGHT, mix(kind_color(corpse.kind), WALKER, 0.6));
        things.push((corpse.pos.x + corpse.pos.y, Thing::Body(body)));
    }
    // Units in a tower aren't drawn – they show up on the tower's roof (`building_parts`).
    for unit in world.units.iter().filter(|u| !u.is_inside()) {
        let p = lerp(unit.prev, unit.pos, alpha);
        let color = kind_color(unit.kind);
        // Selected ones a bit lighter.
        let color = if selected.contains(&unit.id) { lighten(color) } else { color };
        // A mercenary wears a gold mark above its head, one carrying a barrel has it on its back.
        let (_, height) = unit_shape(unit.kind);
        if unit.stats().mercenary {
            let mark = Prism(rect(p - Vec2::splat(0.05), Vec2::splat(0.1)), height + 0.12, height + 0.2, MERC_GOLD);
            things.push((p.x + p.y + 0.01, Thing::Body(mark)));
        }
        // A veteran: a silver bar above its head – wider and flatter than the mercenaries' gold mark.
        if unit.veteran {
            let mark = Prism(rect(p - vec2(0.09, 0.03), vec2(0.18, 0.06)), height + 0.1, height + 0.14, VETERAN_SILVER);
            things.push((p.x + p.y + 0.01, Thing::Body(mark)));
        }
        if unit.barrel {
            carried_barrel(view.sprites, unit, p, height, &mut things);
        }
        // A unit's skin by its number – it never changes.
        if let Some(sprite) = view.sprites.unit(unit.kind, unit.id) {
            let pose = unit_pose(unit, time);
            let color = if sprite.tinted() { color } else { WHITE };
            let figure = Figure { sprite, at: p, facing: unit.facing, pose, clock: clock(time, unit.id), color };
            things.push((p.x + p.y, Thing::Figure(figure)));
            continue;
        }
        let (size, height) = unit_shape(unit.kind);
        // The block turned the way the unit faces, and its weapon sticking out in front, at chest height.
        // (Drawing may use the normal `cos`/`sin` – only the simulation has to compute the same on every PC.)
        let facing = vec2(unit.facing.cos(), unit.facing.sin());
        let ground = bar(p - facing * size / 2.0, p + facing * size / 2.0, size);
        things.push((p.x + p.y, Thing::Body(Prism(ground, 0.0, height, color))));
        if !unit.stats().heals {
            let tip = p + facing * (size / 2.0 + 0.1);
            let middle = (p + tip) / 2.0;
            let weapon = bar(p, tip, 0.04);
            things.push((middle.x + middle.y, Thing::Body(Prism(weapon, height * 0.5, height * 0.62, WEAPON))));
        }
    }
    // Ravens and rats (`critters.rs`) – only where you see them. A flying raven is drawn over what it flies above.
    for critter in view.looks.critters.looks(time).filter(|c| area.contains(c.pos) && shows(c.pos)) {
        let name = if critter.raven { "animals/raven" } else { "animals/rat" };
        if let Some(sprite) = view.sprites.animal(name) {
            let depth = critter.pos.x + critter.pos.y + 2.0 * (critter.pos - critter.at).x;
            let figure = Figure { sprite, at: critter.at, facing: critter.facing, pose: critter.pose, clock: time + critter.pos.x, color: WHITE };
            things.push((depth, Thing::Figure(figure)));
        }
    }
    things.sort_by(|a, b| a.0.total_cmp(&b.0));

    for (_, thing) in &things {
        match thing {
            Thing::Body(Prism(ground, bottom, top, color)) => draw_prism(camera, *ground, *bottom, *top, *color),
            Thing::Figure(f) => f.sprite.draw(camera, f.at, f.facing, f.pose, f.clock, f.color),
            Thing::Decor(landscape, decor, color) => landscape.draw_decor(camera, decor, *color),
            Thing::Picture(p) => p.sprite.draw(camera, p.looks, p.state, p.clock, p.aim, p.at, p.width, p.color),
            Thing::Shadow(sprite, picture, feet, width) => {
                let feet = camera.world_to_screen(*feet);
                sprite.draw_squashed(*picture, feet, sprite.scale(camera, *width), 0.45, Color::new(0.0, 0.0, 0.0, 0.3));
            }
        }
    }
    if !view.grid {
        draw_zombie_tiles(camera, &zombie_tiles, ZOMBIE_TILE_GAP);
    }

    // --- TAB's effects (`effects.rs`): flashes, arrows, blood, blasts – over everything, like TAB's. Without them:
    // shots as lines, a barrel's blast as a growing fireball ---
    let effects = &view.looks.effects;
    effects.draw(camera, now, shows);
    // Fire and smoke on damaged buildings, green smoke on infected ones, leaves falling – where you see it.
    effects.draw_buildings(world, camera, now, shows);
    effects.draw_burning(world, camera, now, shows);
    effects.draw_leaves(world, area, camera, now, shows);
    let pictured = effects.shows_shots();
    let lift = vec2(0.0, -0.3 * TILE_W * camera.zoom);
    for (from, to) in world.shots.iter().filter(|_| !pictured) {
        let a = camera.world_to_screen(*from) + lift;
        let b = camera.world_to_screen(*to) + lift * 0.6;
        draw_line(a.x, a.y, b.x, b.y, 1.5, SHOT);
    }
    for (from, to) in &world.beams {
        let a = camera.world_to_screen(*from) + lift;
        let b = camera.world_to_screen(*to) + lift;
        draw_line(a.x, a.y, b.x, b.y, 2.5, BEAM);
    }
    // The zombies' shots – only where you see them, like the zombies themselves.
    for (from, to) in world.zombie_shots.iter().filter(|(from, _)| shows(*from)) {
        let a = camera.world_to_screen(*from) + lift * 0.8;
        let b = camera.world_to_screen(*to) + lift * 0.6;
        draw_line(a.x, a.y, b.x, b.y, 1.5, ZOMBIE_SHOT);
    }
    // Barrels going off: a fireball that grows and fades (`sim/blast.rs`) – seen wherever it is, it's that loud.
    let blast_radius = data::get().buildings.blast.radius;
    for blast in world.blasts.iter().filter(|_| !pictured) {
        let s = camera.world_to_screen(blast.at);
        let t = blast.left as f32 / 12.0; // 1 when it goes off, 0 when it's gone
        let r = blast_radius * TILE_W * std::f32::consts::FRAC_1_SQRT_2 * camera.zoom * (1.2 - 0.5 * t);
        draw_ellipse(s.x, s.y, r, r * FLAT, 0.0, Color::new(1.0, 0.55, 0.15, 0.55 * t));
        draw_ellipse(s.x, s.y, r * 0.5, r * 0.5 * FLAT, 0.0, Color::new(1.0, 0.9, 0.5, 0.8 * t));
    }
    // The command center's scans: the circle they show (`sim/scan.rs`).
    let scan_radius = data::get().buildings.scan.radius;
    for scan in &world.scans {
        let s = camera.world_to_screen(scan.at);
        let r = scan_radius * TILE_W * std::f32::consts::FRAC_1_SQRT_2 * camera.zoom;
        draw_ellipse_lines(s.x, s.y, r, r * FLAT, 0.0, 2.0, Color::new(0.55, 0.85, 1.0, 0.6));
    }
    // Acid in flight: a blob on an arc, its shadow on the ground below – so you see where it will land.
    for spit in world.spits.iter().filter(|s| shows(s.from) || shows(s.to)) {
        let t = spit.progress(alpha);
        let ground = camera.world_to_screen(lerp(spit.from, spit.to, t));
        let height = (0.3 + 4.0 * t * (1.0 - t) * spit.from.distance(spit.to) * 0.25) * TILE_W * camera.zoom;
        // At least a few pixels – zoomed out it would vanish.
        let size = (0.1 * TILE_W * camera.zoom).max(2.5);
        draw_ellipse(ground.x, ground.y, size, size * FLAT, 0.0, Color::new(0.0, 0.0, 0.0, 0.4));
        draw_circle(ground.x, ground.y - height, size, ACID);
    }

    // The landscape's weather (snow, rain) over the whole map – under the fog, which darkens it like the rest.
    if view.landscape.is_some() {
        effects.draw_weather(&world.look, camera, now);
    }

    // Fog of war over everything that stands on the map, like TAB's: a tall thing is covered where its top reaches into
    // the dark. The bars come after it – your units and buildings always see themselves.
    if !matches!(view.fog, Fog::Off) {
        view.fog_picture.draw(camera, area);
    }

    // --- Health bars above units, just above the block – which ones: the setting (hurt and selected ones at first) ---
    for unit in world.units.iter().filter(|u| !u.is_inside()) {
        let max = unit.max_hp();
        if !view.bars.shows(unit.hp < max, selected.contains(&unit.id)) {
            continue;
        }
        let frac = unit.hp as f32 / max as f32;
        let feet = camera.world_to_screen(lerp(unit.prev, unit.pos, alpha));
        let (size, height) = unit_shape(unit.kind);
        let w = 0.3 * TILE_W * camera.zoom;
        let y = feet.y - (height * TILE_W + size * TILE_H / 2.0 + 6.0) * camera.zoom;
        draw_rectangle(feet.x - w / 2.0, y, w, 3.0, BLACK);
        draw_rectangle(feet.x - w / 2.0, y, w * frac, 3.0, hp_color(frac));
    }

    // --- Life bars above buildings, above the roof: hit points and shield in one bar (`hud::draw_life`) ---
    for building in &world.buildings {
        let stats = building.stats();
        let hurt = building.hp < stats.hp || building.shield < stats.max_shield();
        if !view.bars.shows(hurt, view.selected_buildings.contains(&building.id)) {
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
    // The same over houses of doom you can see – they can't be selected.
    for house in world.doom.iter().filter(|d| view.bars.shows(d.hp < d.full_hp(), false) && watched(world, &view.fog, d.center())) {
        let middle = camera.world_to_screen(house.center());
        let lift = ((house.size.0 + house.size.1) as f32 * TILE_H / 4.0 + (doom_height(house) + 0.3) * TILE_W) * camera.zoom;
        let w = house.size.0 as f32 * 0.5 * TILE_W * camera.zoom;
        draw_life(Rect::new(middle.x - w / 2.0, middle.y - lift, w, 4.0), (house.hp, house.full_hp()), (0, 0), BLACK, None);
    }

    // Beyond the straight edge of the area there is nothing to see – the tiles sticking out over it, the zombies
    // coming. Then the spawner names: always on top, so they can always be read.
    cover_outside(camera, area);
    for spawner in &view.spawners {
        let s = camera.world_to_screen(spawner.corners.iter().sum::<Vec2>() / 4.0);
        let w = measure_text(&spawner.name, None, 18, 1.0).width;
        draw_text(&spawner.name, s.x - w / 2.0 + 1.0, s.y + 6.0 + 1.0, 18.0, BLACK);
        draw_text(&spawner.name, s.x - w / 2.0, s.y + 6.0, 18.0, if spawner.selected { SPAWNER_GLOW } else { WHITE });
    }
}

/// What lies over a tile: nothing (seen right now, or no fog), or a darkening colour.
fn fog_cover(world: &World, fog: &Fog, x: i32, y: i32, light: f32) -> Option<Color> {
    match fog {
        Fog::Off => None,
        Fog::Player if world.vision.is_visible(x, y) || world.vision.is_explored(x, y) => {
            (light < 1.0).then_some(Color::new(0.0, 0.0, 0.0, 1.0 - light))
        }
        Fog::Player => Some(UNEXPLORED),
        Fog::Zombies(sight) => (!sight.is_visible(x, y)).then_some(ZOMBIE_DARK),
    }
}

/// The fog as a picture of the whole grid – one pixel per tile in its cover colour, see-through where someone looks –
/// laid over the map as two triangles, like the minimap (`picture::draw_tiles`). Before, it was one long strip per row
/// of tiles: neighbouring rows ended at different corners, and along the edge where they met rounding left a gap
/// thinner than a pixel – at some zoom levels it caught a whole row of pixels, a line through the black.
pub struct FogPicture {
    image: Image,
    /// How bright the lights make every tile (0..1), worked out anew every frame.
    light: Vec<f32>,
    /// `None` until painted for the first time.
    texture: Option<Texture2D>,
}

impl FogPicture {
    pub fn new() -> FogPicture {
        FogPicture { image: Image { bytes: Vec::new(), width: 0, height: 0 }, light: Vec::new(), texture: None }
    }

    /// Paints the tiles of `area` anew – every frame, the fog changes with every step of every unit. Tiles outside
    /// it keep what they had: they aren't drawn (`draw_world` covers everything outside `area`).
    pub fn paint(&mut self, world: &World, fog: &Fog, area: Region) {
        let ambient = match world.look.as_str() {
            _ if world.revealed => 1.0, // TAB switches its light off when the whole map is shown
            "BR" => EXPLORED_LIGHT_BR,
            _ => EXPLORED_LIGHT,
        };
        let area = with_fog_margin(area);
        let n = world.map.size;
        if self.image.width as usize != n {
            self.image = Image { bytes: vec![0; n * n * 4], width: n as u16, height: n as u16 };
        }
        self.light.clear();
        self.light.resize(n * n, 0.0);
        if matches!(fog, Fog::Player) {
            for (eye, range) in world.eyes() {
                shine(&mut self.light, n, eye, range);
            }
        }
        let rgba = |c: Color| [(c.r * 255.0) as u8, (c.g * 255.0) as u8, (c.b * 255.0) as u8, (c.a * 255.0) as u8];
        for (y, xs) in area.rows(1.0) {
            for x in xs.filter(|&x| x >= 0 && y >= 0 && (x as usize) < n && (y as usize) < n) {
                let tile = y as usize * n + x as usize;
                let i = tile * 4;
                let c = fog_cover(world, fog, x, y, self.light[tile].max(ambient)).unwrap_or(Color::new(0.0, 0.0, 0.0, 0.0));
                self.image.bytes[i..i + 4].copy_from_slice(&rgba(c));
            }
        }
        match &self.texture {
            Some(texture) if texture.width() as usize == n => texture.update(&self.image),
            _ => {
                let texture = Texture2D::from_image(&self.image);
                // Smoothed between the tiles (TAB: `TImageFilter.Linear`) – a soft edge, not a saw of diamonds.
                texture.set_filter(FilterMode::Linear);
                self.texture = Some(texture);
            }
        }
    }

    fn draw(&self, camera: &Camera, area: Region) {
        if let Some(texture) = &self.texture {
            let shift = Vec2::splat(FOG_SHIFT);
            picture::draw_tiles(texture, with_fog_margin(area), |p| camera.world_to_screen(p - shift));
        }
    }
}

/// How bright a light is at this share of its radius (0: its middle, 1: its edge) – `LIGHT_CURVE`, smooth between its
/// steps. 0 where the curve ends.
fn light_at(share: f32) -> f32 {
    let at = share / LIGHT_STEP;
    let step = at as usize;
    if step + 1 >= LIGHT_CURVE.len() {
        return 0.0;
    }
    LIGHT_CURVE[step] + (LIGHT_CURVE[step + 1] - LIGHT_CURVE[step]) * (at - step as f32)
}

/// Adds the light of someone at `eye` who sees `range` tiles. Where lights overlap the brightest counts, like TAB's
/// (`GlobalLightUseMaxValue`). The light ignores forests, like TAB's: it only shows how bright it is, not who sees what.
fn shine(light: &mut [f32], n: usize, eye: Vec2, range: f32) {
    let radius = range * LIGHT_REACH;
    let reach = radius * LIGHT_STEP * (LIGHT_CURVE.len() - 1) as f32;
    // The fog is drawn `FOG_SHIFT` away from its tile – the light has to fall where the tile is shown.
    let spot = eye + Vec2::splat(FOG_SHIFT);
    let (x0, x1) = (((spot.x - reach).floor() as i32).max(0), ((spot.x + reach).ceil() as i32).min(n as i32 - 1));
    let (y0, y1) = (((spot.y - reach).floor() as i32).max(0), ((spot.y + reach).ceil() as i32).min(n as i32 - 1));
    for y in y0..=y1 {
        for x in x0..=x1 {
            let brightness = light_at(vec2(x as f32 + 0.5, y as f32 + 0.5).distance(spot) / radius);
            let tile = &mut light[y as usize * n + x as usize];
            *tile = tile.max(brightness);
        }
    }
}

/// `area` a little bigger: moved up the screen, the fog would leave a strip of its lower edge uncovered. What sticks out
/// is covered at the end (`cover_outside`).
fn with_fog_margin(area: Region) -> Region {
    Region { min: area.min - Vec2::splat(2.0), max: area.max + Vec2::splat(2.0) }
}

/// Everything on screen outside the straight rectangle of `area` gets the background colour again: four bars
/// around it (top, bottom, left, right).
fn cover_outside(camera: &Camera, area: Region) {
    let [top_left, _, bottom_right, _] = area.corners().map(|c| camera.world_to_screen(c));
    let (w, h) = (screen_width(), screen_height());
    let (left, right) = (top_left.x.clamp(0.0, w), bottom_right.x.clamp(0.0, w));
    let (top, bottom) = (top_left.y.clamp(0.0, h), bottom_right.y.clamp(0.0, h));
    draw_rectangle(0.0, 0.0, w, top, BACKGROUND);
    draw_rectangle(0.0, bottom, w, h - bottom, BACKGROUND);
    draw_rectangle(0.0, top, left, bottom - top, BACKGROUND);
    draw_rectangle(right, top, w - right, bottom - top, BACKGROUND);
}

/// Four world points, drawn as one filled shape on the ground.
fn draw_quad(camera: &Camera, corners: [Vec2; 4], color: Color) {
    let [a, b, c, d] = corners.map(|p| camera.world_to_screen(p));
    draw_triangle(a, b, c, color);
    draw_triangle(a, c, d, color);
}

/// A spawner on the ground: see-through area with an edge. The selected one's edge glows (pulsing).
fn draw_spawner(camera: &Camera, spawner: &SpawnerShape) {
    let corners = spawner.corners;
    draw_quad(camera, corners, Color { a: 0.18, ..SPAWNER });
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

/// One thing in the depth order: a body (placeholder) or a picture.
/// `<'a>`: a `Figure` borrows its sprite from `Sprites` – the list mustn't outlive them.
enum Thing<'a> {
    Body(Prism),
    Figure(Figure<'a>),
    /// A tree or rock of TAB's landscape, in this colour.
    Decor(&'a Landscape, Decor, Color),
    /// A building, a piece of one or a house of doom as TAB's picture.
    Picture(Picture<'a>),
    /// A picture's shadow lying on the ground (a carried barrel's): picture number, its feet, how wide.
    Shadow(&'a BuildingSprite, usize, Vec2, f32),
}

/// What `BuildingSprite::draw` needs: which picture in which look, its feet on `at`, for a building `width` tiles wide.
struct Picture<'a> {
    sprite: &'a BuildingSprite,
    looks: &'a Appearance,
    state: State,
    clock: f32,
    aim: Aim,
    at: Vec2,
    width: f32,
    color: Color,
}

/// A zombie or unit as a picture – what `Sprite::draw` needs.
struct Figure<'a> {
    sprite: &'a Sprite,
    at: Vec2,
    facing: f32,
    pose: Pose,
    clock: f32,
    color: Color,
}

/// What a unit's picture shows (`time`: seconds of game time, between ticks too): walking; the medic's beam; or
/// standing with its weapon – how far it's up and how long ago its shot's animation started, like TAB plays them
/// (`sim/combat.rs` decides both, the picture only follows).
fn unit_pose(unit: &Unit, time: f32) -> Pose {
    if unit.is_walking() {
        return Pose::Walk(unit.stats().speed);
    }
    if unit.stats().heals {
        return if unit.is_aiming() { Pose::Attack(None) } else { Pose::Idle };
    }
    let shot = unit.swing_from().map(|tick| time - tick as f32 * TICK);
    // Thanatos's blow: its `fight`, played once over the blow's wind-up and reloading.
    if let (Some(seconds), true, Some(melee)) = (shot, unit.fighting(), unit.stats().melee.as_ref()) {
        let length = (melee.windup + melee.cooldown).max(1) as f32 * TICK;
        if seconds < length {
            return Pose::Fight(seconds / length);
        }
    }
    Pose::Weapon { up: unit.weapon_up(), shot }
}

/// `time` shifted a bit for every figure (by its `id`, 0–10 s), so a crowd doesn't move in step.
fn clock(time: f32, id: u32) -> f32 {
    time + (id as f32 * 0.618_034).fract() * 10.0
}

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
    let color = if building.off { mix(color, SWITCHED_OFF, 0.55) } else { color };
    // Being built: pale like fresh timber, and it grows out of the ground until it's finished.
    let built = 1.0 - building.construction as f32 / building.stats().build_time.max(1) as f32;
    let color = if building.is_built() { color } else { mix(color, SCAFFOLD, 0.5) };
    let height = building.stats().height * (0.2 + 0.8 * built);
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
        BuildingKind::Wall | BuildingKind::StoneWall => {
            let middle = building.center();
            if arms(&mut part, building.tile, (0, 0)) {
                part(rect(middle - Vec2::splat(POST / 2.0), Vec2::splat(POST)), 0.0, height + 0.1, color);
            } else {
                part(rect(middle - Vec2::splat(LONE_POST / 2.0), Vec2::splat(LONE_POST)), 0.0, height, color);
            }
        }
        BuildingKind::Tower | BuildingKind::StoneTower => {
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
            if !gate_open(world, door) {
                part(bar(door - step * 0.5, door + step * 0.5, 0.18), 0.0, height, DOOR_COLOR);
            }
            part(bar(first + step * 0.3, last - step * 0.3, 0.25), height, height + 0.15, color);
        }
        // Four sharp stakes sticking out of the ground.
        BuildingKind::Stakes => {
            for (dx, dy) in [(0.3, 0.3), (0.7, 0.3), (0.3, 0.7), (0.7, 0.7)] {
                let p = building.corner() + vec2(dx, dy);
                part(rect(p - Vec2::splat(0.06), Vec2::splat(0.12)), 0.0, height, color);
            }
        }
        // A red drum standing on the ground, a dark band round its middle.
        BuildingKind::Barrel => {
            let middle = building.center();
            part(rect(middle - Vec2::splat(0.22), Vec2::splat(0.44)), 0.0, height, color);
            part(rect(middle - Vec2::splat(0.24), Vec2::splat(0.48)), height * 0.45, height * 0.6, BARREL_BAND);
        }
        // A wooden platform with the big crossbow on it – its bow a beam across the top.
        BuildingKind::Ballista => {
            let (min, middle) = (building.corner(), building.center());
            let size = building.far_corner() - min;
            part(rect(min + Vec2::splat(0.15), size - Vec2::splat(0.3)), 0.0, height * 0.6, color);
            part(rect(middle - Vec2::splat(0.3), Vec2::splat(0.6)), height * 0.6, height, darken(color, 0.75));
            part(bar(middle + vec2(-0.7, 0.7), middle + vec2(0.7, -0.7), 0.14), height, height + 0.1, DOOR_COLOR);
        }
        // Like a tower, joined to the walls – a dark gun on top.
        BuildingKind::MachineGunTower => {
            let middle = building.center();
            arms(&mut part, building.tile, (0, 0));
            part(rect(middle - Vec2::splat(0.38), Vec2::splat(0.76)), 0.0, height, color);
            part(rect(middle - Vec2::splat(0.18), Vec2::splat(0.36)), height, height + 0.22, darken(color, 0.5));
        }
        // A sooty block with a tall chimney in one corner.
        BuildingKind::Foundry => {
            let (min, max) = (building.corner(), building.far_corner());
            part(rect(min, max - min), 0.0, height, color);
            part(rect(min + Vec2::splat(0.3), Vec2::splat(0.7)), height, height + 1.2, darken(color, 0.6));
        }
        // A tall thin pole with a glowing top – it carries the power.
        BuildingKind::TeslaTower => {
            let middle = building.center();
            part(rect(middle - Vec2::splat(0.15), Vec2::splat(0.3)), 0.0, height, color);
            part(rect(middle - Vec2::splat(0.25), Vec2::splat(0.5)), height, height + 0.15, TESLA_GLOW);
        }
        BuildingKind::CommandCenter
        | BuildingKind::Barracks
        | BuildingKind::Tent
        | BuildingKind::Sawmill
        | BuildingKind::Mine
        | BuildingKind::HunterCottage
        | BuildingKind::FishingHut
        | BuildingKind::Farm
        | BuildingKind::Windmill
        | BuildingKind::CharcoalBurner
        | BuildingKind::WoodWorkshop
        | BuildingKind::StoneWorkshop
        | BuildingKind::MunitionsFactory
        | BuildingKind::StoneHouse
        | BuildingKind::Market
        | BuildingKind::Bank
        | BuildingKind::Inn
        | BuildingKind::WoodHouse
        | BuildingKind::PowerPlant
        | BuildingKind::OilPlatform
        | BuildingKind::Quarry
        | BuildingKind::BigQuarry
        | BuildingKind::BigFarm
        | BuildingKind::BigMill
        | BuildingKind::Warehouse
        | BuildingKind::EngineeringCenter
        | BuildingKind::ShockingTower
        | BuildingKind::Executor
        | BuildingKind::Radar
        | BuildingKind::LookoutTower
        | BuildingKind::Spire
        | BuildingKind::Academy
        | BuildingKind::Victorious
        | BuildingKind::Transmutator
        | BuildingKind::CrystalPalace => {
            part(rect(building.corner(), building.far_corner() - building.corner()), 0.0, height, color);
        }
        // Flat plates on the ground: the blades a ring of steel, the mine a dark disc.
        BuildingKind::BladeTrap | BuildingKind::MineTrap => {
            let middle = building.center();
            part(rect(middle - Vec2::splat(0.3), Vec2::splat(0.6)), 0.0, height, color);
        }
    }
}

/// A unit carrying a barrel (`sim/blast.rs`) – like TAB's `CCarriedObject`: the barrel's picture a quarter tile behind
/// it, lifted off the ground and a bit smaller (0.8), sorted just behind the unit, with a faint shadow below. Without a
/// picture: a red drum on its back.
fn carried_barrel<'a>(sprites: &'a Sprites, unit: &sim::Unit, p: Vec2, height: f32, things: &mut Vec<(f32, Thing<'a>)>) {
    let back = p - vec2(unit.facing.cos(), unit.facing.sin()) * 0.25;
    let depth = p.x + p.y - 0.01;
    let picture = sprites.building(BuildingKind::Barrel, unit.id).and_then(|s| Some((s, s.body()?)));
    let Some((sprite, looks)) = picture else {
        let drum = Prism(rect(back - Vec2::splat(0.07), Vec2::splat(0.14)), height * 0.3, height * 0.85, building_color(BuildingKind::Barrel));
        things.push((depth, Thing::Body(drum)));
        return;
    };
    // Its feet are the lowest corner of its (0.8 tiles big) square – that square's middle on `back`.
    const SIZE: f32 = 0.8;
    const LIFT: f32 = 0.25; // tile widths above the ground – on screen "up" is towards smaller x and y
    let feet = back + Vec2::splat(SIZE / 2.0);
    if let Some(i) = sprite.first_picture(looks, State::Working) {
        things.push((depth - 0.001, Thing::Shadow(sprite, i, feet + vec2(0.1, 0.1), SIZE)));
    }
    let at = feet - Vec2::splat(2.0 * LIFT);
    let picture = Picture { sprite, looks, state: State::Working, clock: 0.0, aim: Aim::default(), at, width: SIZE, color: WHITE };
    things.push((depth, Thing::Picture(picture)));
}

/// Is the door of a gate (its middle at `door`) open – one of your units close to it?
fn gate_open(world: &World, door: Vec2) -> bool {
    world.units.iter().any(|u| !u.is_inside() && u.pos.distance(door) < 1.3)
}

/// Which of its skins a building or house of doom on this tile wears – the same every time, mixed by the tile like TAB's
/// random clips (`RandomMode.GridTwoCells`).
fn tile_look((x, y): (i32, i32)) -> u32 {
    ((x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663)) >> 4
}

/// TAB's fishing hut looks towards its water (`FishermanCottage.cs`): skin 0 water above (smaller y), 1 left (smaller
/// x), 2 below, 3 right. First a side with water all along it, then one with water and a corner, then any.
fn water_side(world: &World, (x, y): (i32, i32)) -> u32 {
    let water = |dx: i32, dy: i32| world.map.terrain(x + dx, y + dy) == Terrain::Water;
    let (up, down, left, right) = (water(0, -1), water(0, 1), water(-1, 0), water(1, 0));
    let (up_left, up_right, down_left, down_right) = (water(-1, -1), water(1, -1), water(-1, 1), water(1, 1));
    let full = [up && up_left && up_right, left && up_left && down_left, down && down_left && down_right, right && up_right && down_right];
    let corner = [up && (up_left || up_right), left && (up_left || down_left), down && (down_left || down_right), right && (up_right || down_right)];
    let any = [up, left, down, right];
    [full, corner, any].iter().find_map(|sides| sides.iter().position(|&w| w)).unwrap_or(0) as u32
}

/// Does a tile join TAB's wall pieces: a wall, or a pillar of a gate (not its door)? Towers don't, like in TAB (`CWall`).
fn wall_at(world: &World, x: i32, y: i32) -> bool {
    let at = world.building_at_point(vec2(x as f32 + 0.5, y as f32 + 0.5));
    at.is_some_and(|b| world.buildings[b].kind.is_wall()) && !world.map.is_door(x, y)
}

/// A building as TAB's pictures (`building_sprites.rs`), sorted in with the rest – `false`: its type has none, it's
/// drawn as blocks (`building_parts`).
/// - Most are one picture, its feet on the far corner of the site: working, idle (switched off, infected, neutral) or
///   being built – infected a bit green.
/// - Walls like TAB's `CWall`: a pillar on the tile, and a piece towards a wall to the lower right (+x) and the lower left
///   (+y), diagonally only where no straight one closes the corner. The stone wall's pillar is plainer in a straight
///   line. A gate: its two pillars (joined like walls) and the door, gone while it's open.
/// - A tower: its body, the units inside on it, its front wall in front of them.
fn building_pictures<'a>(world: &World, building: &Building, view: &View<'a>, time: f32, things: &mut Vec<(f32, Thing<'a>)>) -> bool {
    let sprites = view.sprites;
    let look = match building.kind {
        BuildingKind::FishingHut => water_side(world, building.tile),
        _ => tile_look(building.tile),
    };
    let Some(sprite) = sprites.building(building.kind, look) else {
        return false;
    };
    let state = match building.is_built() {
        false => State::Building,
        true if building.works() => State::Working,
        true => State::Idle,
    };
    let color = if building.infected { mix(WHITE, INFECTED, 0.5) } else { WHITE };
    let ticking = clock(time, building.id);
    // A turning gun (ballista, machine gun): where it points, how far through its shot – from `effects.rs`. Before its
    // first shot it looks down the screen.
    let aim = |looks: &Appearance| match view.looks.effects.gun(building.id) {
        Some((degrees, shot)) => {
            let share = (time - shot) / looks.shot_seconds();
            Aim { degrees, shot: (0.0..1.0).contains(&share).then_some(share) }
        }
        None => Aim { degrees: 45.0, shot: None },
    };
    let picture = |looks: &'a Appearance, at: Vec2, width: f32| {
        Thing::Picture(Picture { sprite, looks, state, clock: ticking, aim: aim(looks), at, width, color })
    };

    if sprite.has_pieces() {
        let (x, y) = building.tile;
        let along = if building.turned { (0, 1) } else { (1, 0) };
        let pillars = match building.kind {
            BuildingKind::Gate => vec![(x, y), (x + 2 * along.0, y + 2 * along.1)],
            _ => vec![(x, y)],
        };
        for (px, py) in pillars {
            let at = vec2(px as f32 + 1.0, py as f32 + 1.0);
            let depth = (px + py) as f32;
            let w = |dx: i32, dy: i32| wall_at(world, px + dx, py + dy);
            // TAB's `SetPillarOrnament`: a straight line along an axis or a diagonal.
            let (n, s, e, west) = (w(0, -1), w(0, 1), w(1, 0), w(-1, 0));
            let (ne, nw, se, sw) = (w(1, -1), w(-1, -1), w(1, 1), w(-1, 1));
            let alone = !n && !s && !e && !west;
            let straight = (n && s && !e && !west)
                || (!n && !s && e && west)
                || (alone && ne && sw && !nw && !se)
                || (alone && nw && se && !ne && !sw);
            let pillar = sprite.piece("pillar_straight").filter(|_| straight).or(sprite.piece("pillar"));
            // The pieces, whether they're there, and how far in front of the tile they're sorted (TAB's `GetZ`).
            let pieces = [
                (pillar, true, 1.0),
                (sprite.piece("left"), s, 1.75),
                (sprite.piece("right"), e, 1.75),
                (sprite.piece("horizontal"), ne && !n && !e, 1.2),
                (sprite.piece("vertical"), se && !e && !s, 2.0),
            ];
            for (looks, shown, ahead) in pieces {
                if let (true, Some(looks)) = (shown, looks) {
                    things.push((depth + ahead, picture(looks, at, 1.0)));
                }
            }
        }
        if building.kind == BuildingKind::Gate {
            let door = vec2(x as f32 + 0.5 + along.0 as f32, y as f32 + 0.5 + along.1 as f32);
            let name = if building.turned { "door_y" } else { "door_x" };
            if let (false, Some(looks)) = (gate_open(world, door), sprite.piece(name)) {
                things.push((door.x + door.y, picture(looks, vec2(x as f32 + 1.0, y as f32 + 1.0), 1.0)));
            }
        }
        return true;
    }

    let Some(body) = sprite.body() else {
        return false;
    };
    let middle = building.center();
    let depth = middle.x + middle.y;
    let width = building.footprint().0 as f32;
    things.push((depth, picture(body, building.far_corner(), width)));
    if let Some(front) = sprite.front() {
        let lift = Vec2::splat(2.0 * sprite.roof());
        for (&i, spot) in world.garrison(building.id).iter().zip(TOWER_SPOTS) {
            let unit = &world.units[i];
            if let Some(figure) = sprites.unit(unit.kind, unit.id) {
                let pose = unit_pose(unit, time);
                let color = if figure.tinted() { kind_color(unit.kind) } else { WHITE };
                let at = middle + spot - lift;
                let figure = Figure { sprite: figure, at, facing: unit.facing, pose, clock: clock(time, unit.id), color };
                things.push((depth + 0.01 + (spot.x + spot.y) * 0.001, Thing::Figure(figure)));
            }
        }
        things.push((depth + 0.02, picture(front, building.far_corner(), width)));
    }
    true
}

/// Has something lying on the map (a house of doom, loot) ever been seen in this view? Only the player's view has fog
/// that hides it. How dark it is now is the fog's business: it lies over it.
/// The floors under the buildings in view (TAB's `CFloorUndeBuilding`): every building but the farm (TAB's
/// `FarmBase.HasFloor`), ours and the neutral ones once seen.
fn floors(world: &World, view: &View) -> Vec<Floor> {
    let near = |tile: (i32, i32), (w, h): (i32, i32)| {
        view.area.contains(vec2(tile.0 as f32 + w as f32 / 2.0, tile.1 as f32 + h as f32 / 2.0)) || {
            // A big one whose middle is just off screen: its corners.
            let (a, b) = (vec2(tile.0 as f32, tile.1 as f32), vec2((tile.0 + w) as f32, (tile.1 + h) as f32));
            [a, b, vec2(a.x, b.y), vec2(b.x, a.y)].iter().any(|&c| view.area.contains(c))
        }
    };
    let ours = world.buildings.iter().map(|b| (b.kind, b.tile, b.footprint()));
    let neutral = world.neutral.iter().filter_map(|n| match n.thing {
        sim::Thing::Building { kind, tile, turned } if seen(world, &view.fog, n.center()) => Some((kind, tile, kind.footprint(turned))),
        _ => None,
    });
    ours.chain(neutral)
        .filter(|&(kind, tile, size)| kind != BuildingKind::Farm && near(tile, size))
        .map(|(_, tile, size)| Floor::under(&world.map, tile, size))
        .collect()
}

fn seen(world: &World, fog: &Fog, p: Vec2) -> bool {
    let (x, y) = (p.x.floor() as i32, p.y.floor() as i32);
    match fog {
        Fog::Player => world.vision.is_visible(x, y) || world.vision.is_explored(x, y),
        _ => true,
    }
}

/// Does someone look at this point right now? Only the player's view asks.
fn watched(world: &World, fog: &Fog, p: Vec2) -> bool {
    match fog {
        Fog::Player => world.vision.is_visible(p.x.floor() as i32, p.y.floor() as i32),
        _ => true,
    }
}

/// How tall a house of doom is drawn – the bigger, the taller.
fn doom_height(house: &DoomHouse) -> f32 {
    0.35 + 0.12 * house.size.0.max(house.size.1) as f32
}

/// A house of a village of doom: a block of dark rotten wood and a dark red roof on top – bright red while it stirs
/// – a batch just came out, or the final wave called it.
fn doom_parts(house: &DoomHouse, tick: u64, things: &mut Vec<(f32, Prism)>) {
    let (min, size) = (house.corner(), house.far_corner() - house.corner());
    let height = doom_height(house);
    let middle = house.center();
    let roof = if house.is_stirring(tick) { DOOM_AWAKE } else { DOOM_ROOF };
    things.push((middle.x + middle.y, Prism(rect(min, size), 0.0, height, DOOM_WALL)));
    let inset = Vec2::splat(0.25);
    things.push((middle.x + middle.y + 0.01, Prism(rect(min + inset, size - inset * 2.0), height, height + 0.15, roof)));
}

/// The colour of a pile of loot – like what lies in it.
pub fn loot_color(kind: LootKind) -> Color {
    match kind {
        LootKind::Gold => Color::new(0.95, 0.78, 0.22, 1.0),
        LootKind::Wood => Color::new(0.55, 0.38, 0.20, 1.0),
        LootKind::Stone => ore_color(Ore::Stone),
        LootKind::Iron => ore_color(Ore::Iron),
        LootKind::Coal => Color::new(0.16, 0.16, 0.18, 1.0),
    }
}

/// A colour seen at `shade` of its brightness (1: as it is).
fn darken(c: Color, shade: f32) -> Color {
    Color::new(c.r * shade, c.g * shade, c.b * shade, c.a)
}

/// How a unit looks as a block: (edge length on the ground in tiles, height in tile widths).
pub fn unit_shape(kind: UnitKind) -> (f32, f32) {
    match kind {
        UnitKind::Soldier => (0.22, 0.28),
        UnitKind::Ranger => (0.17, 0.28),
        UnitKind::Sniper => (0.18, 0.33),
        UnitKind::Medic => (0.21, 0.25),
        UnitKind::Spearman => (0.20, 0.30),
        UnitKind::MercSoldier => unit_shape(UnitKind::Soldier),
        UnitKind::MercRanger => unit_shape(UnitKind::Ranger),
        UnitKind::MercSniper => unit_shape(UnitKind::Sniper),
        // Big men in heavy suits – the Titan a walking cannon.
        UnitKind::Lucifer => (0.26, 0.32),
        UnitKind::Thanatos => (0.26, 0.34),
        UnitKind::Titan => (0.4, 0.5),
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
        Terrain::Dirt => Color::new(0.33, 0.27, 0.19, 1.0),
        Terrain::Mountain => Color::new(0.33, 0.31, 0.29, 1.0),
        Terrain::Forest => Color::new(0.10, 0.30, 0.12, 1.0),
        Terrain::Water => Color::new(0.16, 0.34, 0.58, 1.0),
    }
}

fn is_oil(world: &World, x: i32, y: i32) -> bool {
    world.map.deposit(x, y).is_some_and(|d| d.ore == Ore::Oil)
}

/// The colour of an ore patch – on the map, on the minimap and in the previews.
pub fn ore_color(ore: Ore) -> Color {
    match ore {
        Ore::Iron => Color::new(0.55, 0.33, 0.24, 1.0),
        Ore::Coal => Color::new(0.10, 0.10, 0.11, 1.0),
        Ore::Gold => Color::new(0.85, 0.70, 0.20, 1.0),
        Ore::Stone => Color::new(0.62, 0.60, 0.54, 1.0),
        Ore::Oil => Color::new(0.24, 0.10, 0.30, 1.0),
    }
}

/// The colour of a building type – on the map, on the minimap and in the selection.
pub fn building_color(kind: BuildingKind) -> Color {
    match kind {
        BuildingKind::CommandCenter => Color::new(0.32, 0.45, 0.70, 1.0),
        // Brick red – a military building, not to be mixed up with the brown tents.
        BuildingKind::Barracks => Color::new(0.62, 0.32, 0.28, 1.0),
        BuildingKind::Tent => Color::new(0.62, 0.48, 0.30, 1.0),
        BuildingKind::Wall => Color::new(0.58, 0.57, 0.53, 1.0),
        BuildingKind::Gate => Color::new(0.52, 0.50, 0.46, 1.0),
        BuildingKind::Tower => Color::new(0.66, 0.64, 0.58, 1.0),
        BuildingKind::Sawmill => Color::new(0.50, 0.36, 0.20, 1.0),
        BuildingKind::Mine => Color::new(0.40, 0.36, 0.33, 1.0),
        BuildingKind::HunterCottage => Color::new(0.42, 0.50, 0.26, 1.0),
        BuildingKind::FishingHut => Color::new(0.26, 0.50, 0.56, 1.0),
        // Straw yellow, like its fields.
        BuildingKind::Farm => Color::new(0.76, 0.66, 0.34, 1.0),
        BuildingKind::Windmill => Color::new(0.80, 0.76, 0.62, 1.0),
        BuildingKind::CharcoalBurner => Color::new(0.30, 0.25, 0.23, 1.0),
        BuildingKind::WoodWorkshop => Color::new(0.72, 0.52, 0.26, 1.0),
        BuildingKind::TeslaTower => Color::new(0.45, 0.62, 0.78, 1.0),
        BuildingKind::StoneWorkshop => Color::new(0.60, 0.58, 0.52, 1.0),
        BuildingKind::MunitionsFactory => Color::new(0.45, 0.42, 0.30, 1.0),
        BuildingKind::StoneHouse => Color::new(0.68, 0.64, 0.56, 1.0),
        BuildingKind::StoneWall => Color::new(0.72, 0.71, 0.66, 1.0),
        BuildingKind::StoneTower => Color::new(0.78, 0.76, 0.70, 1.0),
        BuildingKind::Stakes => Color::new(0.55, 0.40, 0.22, 1.0),
        // Awnings in warm orange, like a market stall.
        BuildingKind::Market => Color::new(0.82, 0.50, 0.24, 1.0),
        // Sandstone and gold.
        BuildingKind::Bank => Color::new(0.84, 0.74, 0.42, 1.0),
        // Dark wood, a red sign.
        BuildingKind::Inn => Color::new(0.52, 0.30, 0.22, 1.0),
        // Soot and glowing iron.
        BuildingKind::Foundry => Color::new(0.38, 0.22, 0.18, 1.0),
        // Fresh timber, like the wood workshop that unlocks it.
        BuildingKind::Ballista => Color::new(0.64, 0.46, 0.26, 1.0),
        // Gun metal.
        BuildingKind::MachineGunTower => Color::new(0.36, 0.40, 0.44, 1.0),
        BuildingKind::Barrel => Color::new(0.78, 0.18, 0.12, 1.0),
        BuildingKind::WoodHouse => Color::new(0.66, 0.52, 0.34, 1.0),
        BuildingKind::PowerPlant => Color::new(0.34, 0.30, 0.28, 1.0),
        // Black like oil.
        BuildingKind::OilPlatform => Color::new(0.22, 0.18, 0.24, 1.0),
        BuildingKind::Quarry => Color::new(0.56, 0.54, 0.50, 1.0),
        BuildingKind::BigQuarry => Color::new(0.48, 0.47, 0.45, 1.0),
        BuildingKind::BigFarm => Color::new(0.82, 0.72, 0.36, 1.0),
        BuildingKind::BigMill => Color::new(0.56, 0.58, 0.60, 1.0),
        BuildingKind::Warehouse => Color::new(0.55, 0.46, 0.34, 1.0),
        BuildingKind::EngineeringCenter => Color::new(0.42, 0.44, 0.52, 1.0),
        // Electric blue, like the tesla tower.
        BuildingKind::ShockingTower => Color::new(0.40, 0.68, 0.90, 1.0),
        BuildingKind::Executor => Color::new(0.30, 0.34, 0.38, 1.0),
        BuildingKind::BladeTrap => Color::new(0.62, 0.64, 0.68, 1.0),
        BuildingKind::MineTrap => Color::new(0.25, 0.25, 0.22, 1.0),
        BuildingKind::Radar => Color::new(0.50, 0.62, 0.66, 1.0),
        BuildingKind::LookoutTower => Color::new(0.60, 0.48, 0.30, 1.0),
        // The wonders in rich colours: copper, marble, bronze, violet, crystal.
        BuildingKind::Spire => Color::new(0.72, 0.48, 0.30, 1.0),
        BuildingKind::Academy => Color::new(0.86, 0.84, 0.78, 1.0),
        BuildingKind::Victorious => Color::new(0.62, 0.50, 0.30, 1.0),
        BuildingKind::Transmutator => Color::new(0.56, 0.38, 0.72, 1.0),
        BuildingKind::CrystalPalace => Color::new(0.70, 0.86, 0.92, 1.0),
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
        // Bronze like a spearhead.
        UnitKind::Spearman => Color::new(0.80, 0.55, 0.30, 1.0),
        // The mercenaries: the colour of their kind, with gold in it – they work for gold.
        UnitKind::MercSoldier => mix(kind_color(UnitKind::Soldier), MERC_GOLD, 0.45),
        UnitKind::MercRanger => mix(kind_color(UnitKind::Ranger), MERC_GOLD, 0.45),
        UnitKind::MercSniper => mix(kind_color(UnitKind::Sniper), MERC_GOLD, 0.45),
        // Fire orange, rocket olive, cannon steel.
        UnitKind::Lucifer => Color::new(0.95, 0.45, 0.15, 1.0),
        UnitKind::Thanatos => Color::new(0.55, 0.60, 0.30, 1.0),
        UnitKind::Titan => Color::new(0.55, 0.58, 0.64, 1.0),
    }
}

/// The colour of a zombie breed – on the map and in the panel when one is clicked.
pub fn breed_color(breed: Breed) -> Color {
    match breed {
        Breed::Walker => WALKER,
        Breed::Runner => RUNNER,
        Breed::Fat => FAT,
        Breed::Armored => ARMORED,
        Breed::Spitter => SPITTER,
        Breed::Infested => INFESTED,
        Breed::Harpy => HARPY,
        Breed::Giant => GIANT,
    }
}

fn lighten(c: Color) -> Color {
    mix(c, WHITE, 0.5)
}

/// `share` of the way from colour `a` to colour `b` (0: `a`, 1: `b`). Keeps the transparency of `a`.
fn mix(a: Color, b: Color, share: f32) -> Color {
    Color::new(a.r + (b.r - a.r) * share, a.g + (b.g - a.g) * share, a.b + (b.b - a.b) * share, a.a)
}

/// The ring on the ground under a figure – selected, under the mouse, where it will stand: as big as its body
/// (`radius`, tiles), so about as big as the shadow under it. On screen a flat ellipse, like the tiles.
pub fn draw_ring(camera: &Camera, at: Vec2, radius: f32, thickness: f32, color: Color) {
    let s = camera.world_to_screen(at);
    let w = (radius * TILE_W * std::f32::consts::FRAC_1_SQRT_2 * camera.zoom).max(MIN_RING);
    draw_ellipse_lines(s.x, s.y, w, w * FLAT, 0.0, thickness, color);
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

/// TAB's flat mode (`ZXSystem_GameLevel.UpdateFlatObject`), which comes with its grid: one colour per tile – mountain
/// black, forest dark green (`Color.DarkGreen`), outside the world border almost black (16, 16, 16) – the whole layer
/// half see-through, with a white edge (`DXShaderEffect_Border(Color.White, 2)`) where it meets open ground.
const FLAT_MOUNTAIN: Color = Color::new(0.0, 0.0, 0.0, 0.5);
const FLAT_FOREST: Color = Color::new(0.0, 100.0 / 255.0, 0.0, 0.5);
const FLAT_OUTSIDE: Color = Color::new(16.0 / 255.0, 16.0 / 255.0, 16.0 / 255.0, 0.5);
const FLAT_EDGE: Color = Color::new(1.0, 1.0, 1.0, 0.5);
const FLAT_EDGE_WIDTH: f32 = 2.0;

/// A tile's colour in the flat mode – `None` on open ground (also water: it shows anyway, TAB leaves it out too).
/// Mountain and forest win over the world border, like in TAB, where they are painted over it.
fn flat_color(map: &Map, x: i32, y: i32) -> Option<Color> {
    match map.terrain(x, y) {
        Terrain::Mountain => Some(FLAT_MOUNTAIN),
        Terrain::Forest => Some(FLAT_FOREST),
        _ if !map.in_play(x, y) => Some(FLAT_OUTSIDE),
        _ => None,
    }
}

/// TAB's grid view (`DXIsometricGridObject`: thin white lines around every tile, on the ground under everything that
/// stands), and under the lines its flat mode (`flat_color`): which tiles you can't build on, though the trees and
/// rocks are hidden meanwhile (`draw_world`). Only what lies inside `area` and on screen.
fn draw_grid(camera: &Camera, map: &Map, area: Region, zombies: &[ZombieTile]) {
    let screen = Region {
        min: to_frame(camera.screen_to_world(Vec2::ZERO)),
        max: to_frame(camera.screen_to_world(vec2(screen_width(), screen_height()))),
    };
    let shown = Region { min: screen.min.max(area.min), max: screen.max.min(area.max) };
    let size = map.size as i32;
    let on_map = |x: i32, y: i32| x >= 0 && y >= 0 && x < size && y < size;
    // The colours: tiles of one colour next to each other in a row as one strip – far fewer triangles than one
    // diamond per tile.
    for (y, xs) in shown.rows(1.0) {
        let mut run: Option<(i32, Color)> = None; // where the strip began, and its colour
        for x in xs.start..=xs.end {
            // `xs.end` once more, as `None`: so the last strip is drawn too.
            let color = if x < xs.end && on_map(x, y) { flat_color(map, x, y) } else { None };
            if run.map(|(_, c)| c) == color {
                continue;
            }
            if let Some((from, c)) = run {
                draw_diamond(camera, vec2(from as f32, y as f32), vec2((x - from) as f32, 1.0), c);
            }
            run = color.map(|c| (x, c));
        }
        // The white edge: along every side of a coloured tile whose neighbour there is open ground.
        for x in xs.filter(|&x| on_map(x, y) && flat_color(map, x, y).is_some()) {
            let (left, right, top, bottom) = (x as f32, x as f32 + 1.0, y as f32, y as f32 + 1.0);
            let sides = [
                ((x - 1, y), vec2(left, top), vec2(left, bottom)),
                ((x + 1, y), vec2(right, top), vec2(right, bottom)),
                ((x, y - 1), vec2(left, top), vec2(right, top)),
                ((x, y + 1), vec2(left, bottom), vec2(right, bottom)),
            ];
            for ((nx, ny), from, to) in sides {
                if on_map(nx, ny) && flat_color(map, nx, ny).is_none() {
                    let (p, q) = (camera.world_to_screen(from), camera.world_to_screen(to));
                    draw_line(p.x, p.y, q.x, q.y, FLAT_EDGE_WIDTH, FLAT_EDGE);
                }
            }
        }
    }
    // The zombies' tiles on the flat colours, the lines over them – each a cell of the grid.
    draw_zombie_tiles(camera, zombies, 0.0);
    // Zoomed far out the lines lie close together – fainter, so the map still shows through.
    let tile_pixels = TILE_W * camera.zoom;
    let color = Color::new(1.0, 1.0, 1.0, 0.35 * (tile_pixels / 40.0).clamp(0.3, 1.0));
    for (from, to) in grid_lines(shown) {
        let (p, q) = (camera.world_to_screen(from), camera.world_to_screen(to));
        draw_line(p.x, p.y, q.x, q.y, 1.0, color);
    }
}

/// Every line between two tiles inside `shown`, cut off at its edges: from one end to the other, in tiles.
///
/// In the screen axes (`to_frame`) the point `(x, y)` is `(x - y, x + y)`. So on a line with a fixed `x = k`, staying
/// between `min.x` and `max.x` means `k - max.x <= y <= k - min.x`, and staying between `min.y` and `max.y` means
/// `min.y - k <= y <= max.y - k` – the line runs where both hold. On a line with a fixed `y = k` likewise.
fn grid_lines(shown: Region) -> Vec<(Vec2, Vec2)> {
    let (min, max) = (shown.min, shown.max);
    let mut lines = Vec::new();
    for k in ((min.x + min.y) / 2.0).ceil() as i32..=((max.x + max.y) / 2.0).floor() as i32 {
        let k = k as f32;
        let (from, to) = ((k - max.x).max(min.y - k), (k - min.x).min(max.y - k));
        if from < to {
            lines.push((vec2(k, from), vec2(k, to)));
        }
    }
    for k in ((min.y - max.x) / 2.0).ceil() as i32..=((max.y - min.x) / 2.0).floor() as i32 {
        let k = k as f32;
        let (from, to) = ((min.x + k).max(min.y - k), (max.x + k).min(max.y - k));
        if from < to {
            lines.push((vec2(from, k), vec2(to, k)));
        }
    }
    lines
}

/// Below this zoom the editor shows the zombies as tiles by itself (`Editor::zombie_tiles`): a tile is less than 16
/// pixels wide then, a figure hardly more than a dot – and drawing tens of thousands of them makes the frame slow.
pub const ZOMBIE_TILES_BELOW: f32 = 0.25;
/// Over TAB's pictures each zombie tile is this much smaller than its tile on every side – a hairline of the ground
/// between two tiles, so a crowd reads as cells and not as one blot. In the grid view its lines do that.
const ZOMBIE_TILE_GAP: f32 = 0.07;

/// A tile with zombies on it, as the editor shows it zoomed out: in the colour of the breed most of them are, the more
/// there are the more solid – and a light spot in the middle if one of them storms (not asleep or wandering).
pub struct ZombieTile {
    pub tile: (i32, i32),
    pub color: Color,
    pub storming: bool,
}

/// Each breed's colour as a tile – bright, and far from what the ground is (grass and forest green, mountain black,
/// water blue, the grid's white lines), unlike the greys the figures are tinted with (`breed_color`).
pub fn breed_tile_color(breed: Breed) -> Color {
    let rgb = |r: u8, g: u8, b: u8| Color::from_rgba(r, g, b, 255);
    match breed {
        Breed::Walker => rgb(225, 60, 50),
        Breed::Runner => rgb(250, 145, 35),
        Breed::Fat => rgb(245, 190, 140),
        Breed::Armored => rgb(130, 150, 255),
        Breed::Spitter => rgb(40, 215, 200),
        Breed::Infested => rgb(170, 80, 220),
        Breed::Harpy => rgb(250, 225, 60),
        Breed::Giant => rgb(255, 50, 200),
    }
}

/// How solid a tile with this many zombies is: 1, 2, 3 or more – TAB's brush puts up to 3 of a kind on a tile.
fn tile_alpha(count: u32) -> f32 {
    match count {
        0 | 1 => 0.55,
        2 => 0.75,
        _ => 0.92,
    }
}

/// The zombies inside the area, counted per tile – the awake ones you see (in the player's view only those you see),
/// and the sleepers where there's no fog. A `HashMap` from the tile to how many of each breed (in the order of
/// `Breed::ALL`) and whether one storms – like a Python dict of lists. Only drawing, so its order doesn't matter.
fn zombie_tiles(world: &World, view: &View) -> Vec<ZombieTile> {
    let mut counts: HashMap<(i32, i32), ([u32; Breed::ALL.len()], bool)> = HashMap::new();
    let mut add = |pos: Vec2, breed: Breed, storming: bool| {
        if view.area.contains(pos) {
            let entry = counts.entry((pos.x.floor() as i32, pos.y.floor() as i32)).or_default();
            entry.0[Breed::ALL.iter().position(|&b| b == breed).unwrap_or(0)] += 1;
            entry.1 |= storming;
        }
    };
    let player = matches!(view.fog, Fog::Player);
    for zombie in world.zombies.iter().filter(|z| z.is_alive()) {
        if !player || view.looks.get(zombie.id).alpha > 0.0 {
            add(zombie.pos, zombie.breed, !world.is_wanderer(zombie));
        }
    }
    if !player {
        for sleeper in world.sleep.iter() {
            add(sleeper.pos, sleeper.breed, false);
        }
    }
    counts
        .into_iter()
        .map(|(tile, (per_breed, storming))| {
            // The breed most of them are – of a tie the first in `Breed::ALL` (`max_by_key` takes the last of the
            // biggest, so the place counts backwards).
            let (most, _) = per_breed.iter().enumerate().max_by_key(|&(b, &n)| (n, usize::MAX - b)).unwrap_or((0, &0));
            let color = Color { a: tile_alpha(per_breed.iter().sum()), ..breed_tile_color(Breed::ALL[most]) };
            ZombieTile { tile, color, storming }
        })
        .collect()
}

/// The zombie tiles on screen, each `gap` smaller than its tile on every side.
fn draw_zombie_tiles(camera: &Camera, tiles: &[ZombieTile], gap: f32) {
    for t in tiles {
        let corner = vec2(t.tile.0 as f32, t.tile.1 as f32);
        if !on_screen(camera.world_to_screen(corner + 0.5), 20.0) {
            continue;
        }
        draw_diamond(camera, corner + gap, Vec2::splat(1.0 - 2.0 * gap), t.color);
        if t.storming {
            draw_diamond(camera, corner + 0.32, Vec2::splat(0.36), Color::new(1.0, 1.0, 1.0, 0.85));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A light is brightest in its middle, fades steadily, and has gone out where its curve ends.
    #[test]
    fn a_light_fades_outwards() {
        assert_eq!(light_at(0.0), 1.0);
        let steps: Vec<f32> = (0..=40).map(|k| light_at(k as f32 * 0.0125)).collect();
        assert!(steps.windows(2).all(|w| w[1] <= w[0]), "{steps:?}");
        assert_eq!(light_at(0.6), 0.0);
        assert_eq!(light_at(5.0), 0.0);
    }

    /// Where two lights overlap the brighter one counts; they don't add up, and nothing leaves the map.
    #[test]
    fn lights_overlap_by_the_brightest() {
        let n = 16;
        let mut light = vec![0.0; n * n];
        shine(&mut light, n, vec2(0.0, 0.0), 40.0);
        shine(&mut light, n, vec2(8.0, 8.0), 4.0);
        assert!(light.iter().all(|&l| (0.0..=1.0).contains(&l)));
        assert!(light[8 * n + 8] > 0.9);
    }

    /// The grid's lines stay inside what's shown, end on its edges, and none is missing.
    #[test]
    fn grid_lines_are_cut_at_the_edges() {
        let shown = Region { min: vec2(-3.5, 10.25), max: vec2(4.5, 18.0) };
        let lines = grid_lines(shown);
        for (from, to) in &lines {
            for end in [*from, *to] {
                let f = to_frame(end);
                assert!(f.x >= shown.min.x - 1e-4 && f.x <= shown.max.x + 1e-4, "{end} is outside");
                assert!(f.y >= shown.min.y - 1e-4 && f.y <= shown.max.y + 1e-4, "{end} is outside");
            }
        }
        // Every tile corner inside lies on a line in each direction.
        for x in 0..20 {
            for y in 0..20 {
                let p = vec2(x as f32, y as f32);
                if shown.contains(p) {
                    let along_y = lines.iter().any(|(a, b)| a.x == p.x && b.x == p.x && a.y <= p.y && p.y <= b.y);
                    let along_x = lines.iter().any(|(a, b)| a.y == p.y && b.y == p.y && a.x <= p.x && p.x <= b.x);
                    assert!(along_y && along_x, "no lines through {p}");
                }
            }
        }
    }
}
