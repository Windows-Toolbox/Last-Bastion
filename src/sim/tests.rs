//! Tests of the simulation. Most of them play a little scene tick by tick and look at the result.

use super::*;
use super::zombies::values;
use crate::data;
use crate::formation::{self, Formation};
use crate::kinds::Resource;
use crate::map::{to_frame, Ore, Terrain};

/// A little game of 600 ticks with a bit of everything, returns the checksum at the end.
/// Also uses the circle formation and `PlaceZombies` – the places with `sin`, `cos` and `atan2`.
fn scripted_game() -> u64 {
    let mut world = World::new(42);
    let ids: Vec<u32> = world.units.iter().map(|u| u.id).collect();
    let base = world.base;
    let (bx, by) = (base.x as i32, base.y as i32);
    let circle = Formation { shape: formation::Shape::Circle, from: base + vec2(-6.0, -6.0), to: base + vec2(-3.0, -6.0) };
    for tick in 0..600 {
        let orders = match tick {
            // The barracks a tick before the rest, so it gets building number 2 whether the tent fits or not.
            49 => vec![Order::PlaceBuilding { kind: BuildingKind::Barracks, tile: (bx + 8, by + 8), turned: false, editor: true }],
            50 => vec![
                Order::SpawnWave { count: 300, from: WaveFrom::Sides(2) },
                Order::SpawnZombiesAt { count: 50, point: base + vec2(6.0, -4.0) },
                Order::PlaceZombies { center: base + vec2(-8.0, 8.0), radius: 3.0, count: 40, breed: Breed::Walker, wander: true },
            ],
            100 => vec![
                Order::Command { units: ids[0..3].to_vec(), command: Command::Patrol(base + vec2(6.0, -4.0)), queue: false, formation: None },
                Order::Command { units: ids[3..8].to_vec(), command: Command::Move(base + vec2(-4.0, 6.0)), queue: false, formation: None },
                Order::Command { units: ids[3..8].to_vec(), command: Command::AttackMove(base + vec2(6.0, 6.0)), queue: true, formation: None },
                Order::Command { units: ids[8..10].to_vec(), command: Command::Follow(ids[0]), queue: false, formation: None },
                Order::PlaceBuilding { kind: BuildingKind::Tent, tile: (bx + 5, by - 5), turned: false, editor: false },
                Order::Train { building: 2, kind: UnitKind::Medic },
                Order::SetRally { building: 2, rally: Rally::Point(base + vec2(-4.0, -4.0)) },
            ],
            150 => vec![Order::PaintTerrain { terrain: Terrain::Water, tiles: (bx - 4..bx + 4).map(|x| (x, by + 10)).collect() }],
            300 => vec![Order::Command { units: ids[0..8].to_vec(), command: Command::Move(circle.from), queue: false, formation: Some(circle) }],
            _ => Vec::new(),
        };
        world.step(&orders);
    }
    world.checksum()
}

/// Twice the same seed with the same orders → the same checksum (DESIGN.md 12.8).
#[test]
fn same_seed_and_orders_give_same_result() {
    assert_eq!(scripted_game(), scripted_game());
}

/// The checksum of `scripted_game`, written down. The test above only compares two runs on the same PC – this one
/// compares with the PC where the number was written down. Passes on another PC (or Linux) too → the floats come out
/// bit for bit the same there (DESIGN.md 12.8).
///
/// Changing the simulation on purpose changes the number: then copy the new one from the error message.
/// Only on an otherwise unchanged state does a different number mean a desync.
const SCRIPTED_GAME_CHECKSUM: u64 = 0x8ea8_1645_c055_49de;

#[test]
fn scripted_game_matches_written_down_checksum() {
    let checksum = scripted_game();
    assert_eq!(checksum, SCRIPTED_GAME_CHECKSUM, "new checksum: {checksum:#018x}");
}

/// A different seed must give something different – otherwise the test above checks nothing.
#[test]
fn different_seed_gives_different_result() {
    let mut a = World::new(1);
    let mut b = World::new(2);
    for _ in 0..100 {
        a.step(&[]);
        b.step(&[]);
    }
    assert_ne!(a.checksum(), b.checksum());
}

/// The unit at the bottom right of the start formation, in a world without zombies – none roaming, and none waiting in
/// the villages of doom or around the loot either (the eraser takes those away).
fn quiet_world() -> (World, u32) {
    let mut world = World::new(7);
    let houses = world.doom.iter().map(|d| d.tile);
    let loot = world.loot.iter().map(|l| (l.pos.x.floor() as i32, l.pos.y.floor() as i32));
    let tiles = houses.chain(loot).collect();
    world.step(&[Order::ClearZombies, Order::RemoveAt { tiles }]);
    assert!(world.doom.is_empty() && world.loot.is_empty());
    let id = world.units.last().unwrap().id;
    (world, id)
}

/// A barracks on cleared ground up left of the command center (tiles x −7…−5, y −8…−6 from the base), placed like
/// in the editor – without time passing, so `tick` stays where it was – and a munitions factory right of it, so it can
/// train firearms too. Returns the barracks' `id`.
fn barracks(world: &mut World) -> u32 {
    let tile = (world.base.x as i32 - 7, world.base.y as i32 - 8);
    let ground = (tile.1 - 2..tile.1 + 5).flat_map(|y| (tile.0 - 2..tile.0 + 5).map(move |x| (x, y))).collect();
    world.apply_orders(&[
        Order::PaintTerrain { terrain: Terrain::Grass, tiles: ground },
        Order::PlaceBuilding { kind: BuildingKind::Barracks, tile, turned: false, editor: true },
    ]);
    let b = world.buildings.last().unwrap();
    assert_eq!((b.kind, b.tile), (BuildingKind::Barracks, tile), "the barracks didn't fit");
    let id = b.id;
    let factory = (tile.0 + 3, tile.1);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::MunitionsFactory, tile: factory, turned: false, editor: true }]);
    assert_eq!(world.buildings.last().unwrap().kind, BuildingKind::MunitionsFactory, "the munitions factory didn't fit");
    id
}

/// A wave on the timeline comes exactly at its tick, out of the spawners in the ring – not a tick earlier.
/// And the clock counts days of 24 hours from day 1.
#[test]
fn timed_wave_comes_on_time() {
    let (mut world, _) = quiet_world();
    let at = world.tick + 30;
    world.step(&[Order::AddTimedWave(TimedWave { at, count: 50, from: WaveFrom::Sides(1), breed: Breed::Runner, final_wave: false, reveal: false })]);
    while world.tick < at {
        assert!(world.zombies.is_empty(), "too early, tick {}", world.tick);
        world.step(&[]);
    }
    world.step(&[]);
    assert_eq!(world.zombies.len(), 50);
    assert!(world.zombies.iter().all(|z| !world.map.in_play(z.pos.x as i32, z.pos.y as i32)));
    assert_eq!(day_and_hour(0), (1, 0));
    assert_eq!(day_and_hour(DAY_TICKS + 3 * HOUR_TICKS + 5), (2, 3));
}

/// Commands added with Shift are carried out in order.
#[test]
fn queued_commands_run_in_order() {
    let (mut world, id) = quiet_world();
    // Both points lie in the mountain-free area around the base.
    let a = world.base + vec2(8.0, 8.0);
    let b = world.base + vec2(8.0, -4.0);
    world.step(&[
        Order::Command { units: vec![id], command: Command::Move(a), queue: false, formation: None },
        Order::Command { units: vec![id], command: Command::AttackMove(b), queue: true, formation: None },
    ]);
    let mut reached_a = false;
    for _ in 0..300 {
        world.step(&[]);
        let pos = world.units.last().unwrap().pos;
        if pos.distance(a) < 0.1 {
            reached_a = true;
        }
        assert!(pos.distance(b) > 0.1 || reached_a, "reached B before A");
    }
    let unit = world.units.last().unwrap();
    assert!(unit.pos.distance(b) < 0.1);
    assert!(unit.queue.is_empty());
}

/// Patrol: there, back and there again – without a new command.
#[test]
fn patrol_goes_back_and_forth() {
    let (mut world, id) = quiet_world();
    let start = world.units.last().unwrap().pos;
    let p = world.base + vec2(8.0, 8.0);
    world.step(&[Order::Command { units: vec![id], command: Command::Patrol(p), queue: false, formation: None }]);
    let mut visits = Vec::new();
    for _ in 0..400 {
        world.step(&[]);
        let pos = world.units.last().unwrap().pos;
        let at = if pos.distance(p) < 0.1 {
            Some('P')
        } else if pos.distance(start) < 0.1 {
            Some('S')
        } else {
            None
        };
        if let Some(at) = at {
            if visits.last() != Some(&at) {
                visits.push(at);
            }
        }
    }
    assert!(visits.starts_with(&['P', 'S', 'P']), "visited: {visits:?}");
}

/// Right click on a zombie: walk there and finish it off, then the command is done.
#[test]
fn attack_kills_target_and_finishes() {
    let (mut world, id) = quiet_world();
    world.units.retain(|u| u.id == id); // nobody else steals the kill
    world.step(&[Order::SpawnZombiesAt { count: 1, point: world.base + vec2(9.0, -5.0) }]);
    let zombie = world.zombies[0].id;
    world.step(&[Order::Command { units: vec![id], command: Command::Attack(zombie), queue: false, formation: None }]);
    for _ in 0..300 {
        world.step(&[]);
    }
    assert!(world.zombies.is_empty());
    let unit = world.units.last().unwrap();
    assert!(unit.queue.is_empty());
    assert_eq!(unit.kills, 1);
}

/// Zombies that reach a unit hit it – until it dies and disappears from the list.
#[test]
fn zombies_kill_a_lone_unit() {
    let (mut world, id) = quiet_world();
    // Send the unit far away alone, then put 100 zombies right on it.
    let far = world.base + vec2(8.0, 8.0);
    world.step(&[Order::Command { units: vec![id], command: Command::Move(far), queue: false, formation: None }]);
    for _ in 0..200 {
        world.step(&[]);
    }
    world.step(&[
        Order::Command { units: vec![id], command: Command::Hold, queue: false, formation: None },
        Order::SpawnZombiesAt { count: 100, point: far },
    ]);
    for _ in 0..400 {
        world.step(&[]);
    }
    assert_eq!(world.find_unit(id), None);
}

/// Three units sent with a dragged line: at the end they stand side by side on that line, from one end to
/// the other.
#[test]
fn units_line_up_in_formation() {
    let (mut world, _) = quiet_world();
    let ids: Vec<u32> = world.units.iter().take(3).map(|u| u.id).collect();
    // 2.5 tiles below the last row of the start units (7 below the base for a 3 × 3 command center).
    let y = BuildingKind::CommandCenter.footprint(false).1 as f32 / 2.0 + 5.5;
    let (from, to) = (world.base + vec2(2.0, y), world.base + vec2(4.0, y));
    let formation = Some(Formation { shape: formation::Shape::Line, from, to });
    world.step(&[Order::Command { units: ids.clone(), command: Command::Move(from), queue: false, formation }]);
    for _ in 0..300 {
        world.step(&[]);
    }
    let mut xs: Vec<f32> = Vec::new();
    for id in &ids {
        let p = world.units[world.find_unit(*id).unwrap()].pos;
        assert!((p.y - from.y).abs() < 0.3, "not on the line: {p:?}");
        xs.push(p.x);
    }
    xs.sort_by(f32::total_cmp);
    assert!((xs[0] - from.x).abs() < 0.3 && (xs[2] - to.x).abs() < 0.3, "not spread along the line: {xs:?}");
}

/// A row of trees between a soldier and a zombie in range: it neither sees nor shoots it. Without the trees
/// it does.
#[test]
fn no_shooting_through_trees() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(6.0, 6.0);
    world.spawn_unit(UnitKind::Soldier, p);
    let id = world.units[0].id;
    let (x, y) = (p.x as i32, p.y as i32);
    let trees: Vec<(i32, i32)> = (y - 3..=y + 3).map(|ty| (x + 2, ty)).collect();
    world.step(&[
        Order::Command { units: vec![id], command: Command::Hold, queue: false, formation: None },
        Order::PaintTerrain { terrain: Terrain::Tree, tiles: trees.clone() },
        Order::SpawnZombiesAt { count: 1, point: p + vec2(4.0, 0.0) },
    ]);
    world.zombies[0].pos = p + vec2(4.0, 0.0);
    world.zombies[0].speed = 0.0; // stays behind the trees
    let mut shot = false;
    for _ in 0..20 {
        world.step(&[]);
        shot |= !world.shots.is_empty();
    }
    assert!(!shot, "shot through the trees");
    assert!(!world.vision.sees(world.zombies[0].pos), "sees through the trees");
    world.step(&[Order::PaintTerrain { terrain: Terrain::Grass, tiles: trees }]);
    shot |= !world.shots.is_empty(); // the first shot already falls in this tick
    for _ in 0..5 {
        world.step(&[]);
        shot |= !world.shots.is_empty();
    }
    assert!(shot, "doesn't shoot even without the trees");
}

/// The command center stands on the base and blocks the map there.
#[test]
fn command_center_blocks_the_base() {
    let world = World::new(7);
    assert_eq!(world.buildings.len(), 1);
    assert!(world.map.is_blocked(world.base.x as i32, world.base.y as i32));
    assert_eq!(world.building_at_point(world.base), Some(0));
}

/// Tents only on free ground. Once built, the spot is blocked.
#[test]
fn tents_only_on_free_ground() {
    let (mut world, _) = quiet_world();
    let tile = (world.base.x as i32 + 5, world.base.y as i32 - 5);
    assert!(world.can_place(BuildingKind::Tent, tile, false));
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tent, tile, turned: false, editor: false }]);
    assert_eq!(world.buildings.len(), 2);
    assert!(world.map.is_blocked(tile.0 + 1, tile.1 + 1));
    assert!(!world.can_place(BuildingKind::Tent, tile, false));
    // Not on the command center either.
    let cc = world.buildings[0].tile;
    assert!(!world.can_place(BuildingKind::Tent, cc, false));
}

/// Zombies arriving at a building hit it until it collapses and frees the spot. (No units: otherwise the zombies
/// see them, storm at them and go around it.) A barracks – a tent would be infected and then left alone.
#[test]
fn zombies_destroy_a_building() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let tile = (world.base.x as i32 + 5, world.base.y as i32 - 5);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Barracks, tile, turned: false, editor: false }]);
    assert_eq!(world.buildings.len(), 2, "the barracks didn't fit");
    let corner = vec2(tile.0 as f32, tile.1 as f32);
    world.step(&[Order::SpawnZombiesAt { count: 60, point: corner + vec2(1.0, -0.8) }]);
    for _ in 0..600 {
        world.step(&[]);
    }
    // (Without units shooting them, the 60 zombies then tear down the command center too.)
    assert!(world.buildings.iter().all(|b| b.kind != BuildingKind::Barracks), "the barracks is still standing");
    assert!(!world.map.is_blocked(tile.0, tile.1));
}

/// A ring of walls (distance `r` in tiles) around the tile `center` – without the tiles in `gaps`.
fn wall_ring(center: (i32, i32), r: i32, gaps: &[(i32, i32)]) -> Vec<Order> {
    let mut orders = Vec::new();
    for y in center.1 - r..=center.1 + r {
        for x in center.0 - r..=center.0 + r {
            let on_ring = (x - center.0).abs() == r || (y - center.1).abs() == r;
            if on_ring && !gaps.contains(&(x, y)) {
                orders.push(Order::PlaceBuilding { kind: BuildingKind::Wall, tile: (x, y), turned: false, editor: false });
            }
        }
    }
    orders
}

/// If the command center is completely walled in, the zombies don't walk around but tear the
/// wall down. (Before, the base was then unreachable for them.)
#[test]
fn zombies_break_through_walls() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let center = (world.base.x as i32, world.base.y as i32);
    world.step(&wall_ring(center, 4, &[]));
    assert!(world.buildings.len() > 30);
    // A tile in the ring outside the border, straight above the base.
    let out = (center.0, center.1 - world.map.size as i32 / 2 + crate::map::BORDER as i32 / 2);
    assert!(world.to_base.reachable(out.0, out.1), "the base behind the wall must stay reachable for zombies");
    world.step(&[Order::SpawnZombiesAt { count: 30, point: world.base + vec2(10.0, 0.5) }]);
    // 10 s: enough to reach the wall and hit it, too short to break through it.
    for _ in 0..200 {
        world.step(&[]);
    }
    let damaged = world.buildings.iter().filter(|b| b.kind == BuildingKind::Wall && b.hp < b.stats().hp).count();
    assert!(damaged > 0, "no wall damaged");
    // The command center is still intact: the zombies are at the wall, not in front of it.
    assert_eq!(world.buildings[0].hp, world.buildings[0].stats().hp);
}

/// A soldier inside walls with a gap, shooting: the zombie it hits and a few around it find the way through the gap,
/// the others hit the wall. Without a shot nobody looks for the gap.
#[test]
fn shot_zombies_find_the_gap_others_hit_the_wall() {
    let run = |shoots: bool| {
        let (mut world, _) = quiet_world();
        world.units.clear();
        let fort = (world.base.x as i32 + 6, world.base.y as i32 - 6);
        world.step(&wall_ring(fort, 2, &[(fort.0, fort.1 - 2)])); // gap at the top
        let inside = vec2(fort.0 as f32 + 0.5, fort.1 as f32 + 0.5);
        world.spawn_unit(UnitKind::Soldier, inside);
        if !shoots {
            world.units[0].cooldown = u32::MAX;
        }
        world.step(&[
            Order::Command { units: vec![world.units[0].id], command: Command::Hold, queue: false, formation: None },
            Order::SpawnZombiesAt { count: 40, point: inside + vec2(5.0, 0.0) },
        ]);
        for zombie in &mut world.zombies {
            zombie.hp = 100_000; // nobody dies, so the crowd stays the same
        }
        let mut hurt = false;
        let mut most_seeking = 0;
        for _ in 0..300 {
            world.step(&[]);
            hurt |= world.units.first().is_none_or(|u| u.hp < u.stats().hp);
            most_seeking = most_seeking.max(world.zombies.iter().filter(|z| z.is_seeking()).count());
        }
        let damaged = world.buildings.iter().filter(|b| b.kind == BuildingKind::Wall && b.hp < b.stats().hp).count();
        (hurt, damaged, most_seeking)
    };
    let (hurt, damaged, most_seeking) = run(true);
    assert!(hurt, "no shot zombie got through the gap");
    assert!(damaged > 0, "all zombies ignored the wall");
    assert!(most_seeking < 20, "{most_seeking} of 40 looked for the gap at once - should be only a few");
    let (_, _, most_seeking) = run(false);
    assert_eq!(most_seeking, 0, "zombies looked for the gap without a shot");
}

/// No zombie or unit stands on ground that is blocked for it.
fn nobody_on_blocked_ground(world: &World) -> bool {
    let tile = |p: Vec2| (p.x.floor() as i32, p.y.floor() as i32);
    world.zombies.iter().all(|z| !world.map.is_blocked(tile(z.pos).0, tile(z.pos).1))
        && world.units.iter().all(|u| !world.map.blocks_unit(tile(u.pos).0, tile(u.pos).1))
}

/// Painting a mountain onto a crowd of zombies and a unit pushes all of them out – nobody is lost.
#[test]
fn painted_terrain_pushes_everyone_out() {
    let (mut world, _) = quiet_world();
    world.units.clear(); // nobody shoots, so the count of zombies stays the same
    let p = world.base + vec2(7.0, 7.0);
    world.spawn_unit(UnitKind::Soldier, p);
    world.units[0].cooldown = u32::MAX;
    let id = world.units[0].id;
    world.step(&[Order::SpawnZombiesAt { count: 50, point: p }]);
    let (zombies, units) = (world.zombies.len(), world.units.len());
    let (cx, cy) = (p.x as i32, p.y as i32);
    let tiles: Vec<(i32, i32)> = (cy - 3..=cy + 3).flat_map(|y| (cx - 3..=cx + 3).map(move |x| (x, y))).collect();
    world.step(&[Order::PaintTerrain { terrain: Terrain::Mountain, tiles }]);
    assert!(world.map.is_solid(cx, cy));
    assert!(nobody_on_blocked_ground(&world));
    assert_eq!(world.zombies.len(), zombies);
    assert_eq!(world.units.len(), units);
    assert!(world.find_unit(id).is_some());
}

/// No building close to zombies: a zombie 5 tiles away allows a wall, 3 tiles away doesn't. For a tent
/// (2 × 2) the 4 tiles count from its edge, not from its top left tile.
#[test]
fn no_building_near_zombies() {
    let (mut world, _) = quiet_world();
    world.units.clear(); // nobody shoots the test zombie
    let tile = (world.base.x as i32 + 6, world.base.y as i32 + 6);
    let zombie_at = |dx: i32| vec2((tile.0 + dx) as f32 + 0.5, tile.1 as f32 + 0.5);
    world.step(&[Order::SpawnZombiesAt { count: 1, point: zombie_at(5) }]);
    world.zombies[0].pos = zombie_at(5); // exactly there, not somewhere around it
    assert!(world.can_place(BuildingKind::Wall, tile, false), "5 tiles away is far enough for a wall");
    // A tent at the same spot reaches one tile further to the right: its edge is only 4 tiles away.
    assert!(!world.can_place(BuildingKind::Tent, tile, false));
    world.zombies[0].pos = zombie_at(3);
    assert!(!world.can_place(BuildingKind::Wall, tile, false));
    let buildings = world.buildings.len();
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Wall, tile, turned: false, editor: false }]);
    assert_eq!(world.buildings.len(), buildings, "the wall must not have been built");
}

/// A gate can go on a line of walls: the walls under it disappear, the one next to it stays, and on the walls
/// it costs less. Half on walls, half on grass works too – but not on the command center.
#[test]
fn gate_replaces_walls() {
    let (mut world, _) = quiet_world();
    let (x, y) = (world.base.x as i32 + 8, world.base.y as i32 - 8);
    let walls: Vec<Order> = (0..4)
        .map(|k| Order::PlaceBuilding { kind: BuildingKind::Wall, tile: (x + k, y), turned: false, editor: false })
        .collect();
    world.step(&walls);
    assert_eq!(world.buildings.len(), 1 + 4);
    let gate = BuildingKind::Gate;
    assert!(world.can_place(gate, (x, y), false));
    assert_eq!(world.replaced_by(gate, (x, y), false).len(), 3);
    assert!(world.build_cost(gate, (x, y), false) < gate.stats().cost);
    world.step(&[Order::PlaceBuilding { kind: gate, tile: (x, y), turned: false, editor: false }]);
    let kinds: Vec<BuildingKind> = world.buildings.iter().map(|b| b.kind).collect();
    assert_eq!(kinds.iter().filter(|&&k| k == BuildingKind::Wall).count(), 1, "{kinds:?}");
    assert_eq!(kinds.iter().filter(|&&k| k == gate).count(), 1);
    assert!(world.map.is_door(x + 1, y));
    assert!(world.map.is_built(x + 3, y), "the wall next to the gate stays");
    // A gate over the last wall and two grass tiles: allowed. Over the command center: never.
    assert!(world.can_place(gate, (x + 3, y), false));
    assert!(!world.can_place(gate, world.buildings[0].tile, false));
}

/// Towers need one free tile between them – corner to corner isn't enough. Walls don't care.
#[test]
fn towers_keep_a_gap() {
    let (mut world, _) = quiet_world();
    let (x, y) = (world.base.x as i32 + 8, world.base.y as i32 - 8);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tower, tile: (x, y), turned: false, editor: false }]);
    let tower = BuildingKind::Tower;
    assert!(!world.can_place(tower, (x + 1, y), false));
    assert!(!world.can_place(tower, (x + 1, y + 1), false), "corner to corner is too close");
    assert!(world.can_place(tower, (x + 2, y + 1), false));
    assert!(world.can_place(BuildingKind::Wall, (x + 1, y), false));
}

/// Like in StarCraft you may build on your own units: they're pushed off the site. Four holding units exactly on
/// top of each other in the middle of a barracks site – afterwards none stands on it, and they push each other apart
/// even though they hold (before, holding units couldn't be pushed at all).
#[test]
fn building_pushes_units_aside() {
    let (mut world, _) = quiet_world();
    let tile = (world.base.x as i32 - 7, world.base.y as i32 - 8);
    let ground = (tile.1 - 2..tile.1 + 5).flat_map(|y| (tile.0 - 2..tile.0 + 5).map(move |x| (x, y))).collect();
    world.apply_orders(&[Order::PaintTerrain { terrain: Terrain::Grass, tiles: ground }]);
    let middle = vec2(tile.0 as f32 + 1.5, tile.1 as f32 + 1.5);
    let mut ids = Vec::new();
    for _ in 0..4 {
        world.spawn_unit(UnitKind::Soldier, middle);
        ids.push(world.units.last().unwrap().id);
    }
    assert!(world.can_place(BuildingKind::Barracks, tile, false), "units on the site must not stop the building");
    world.step(&[
        Order::Command { units: ids.clone(), command: Command::Hold, queue: false, formation: None },
        Order::PlaceBuilding { kind: BuildingKind::Barracks, tile, turned: false, editor: false },
    ]);
    assert_eq!(world.buildings.last().unwrap().kind, BuildingKind::Barracks, "not built");
    for _ in 0..60 {
        world.step(&[]);
    }
    let spots: Vec<Vec2> = ids.iter().map(|&id| world.units[world.find_unit(id).unwrap()].pos).collect();
    for (k, p) in spots.iter().enumerate() {
        assert!(!world.map.blocks_unit(p.x.floor() as i32, p.y.floor() as i32), "still on the barracks: {p:?}");
        for q in &spots[k + 1..] {
            assert!(p.distance(*q) > UNIT_RADIUS * 1.4, "still on top of each other: {p:?} {q:?}");
        }
    }
}

/// Workers like in TAB: tents give them, the barracks and every unit take some. Not enough free: nothing is built
/// or trained. A cancelled unit and a dead one give theirs back.
#[test]
fn workers_limit_building_and_training() {
    let (mut world, _) = quiet_world();
    let (barracks, tent, ranger) = (BuildingKind::Barracks.stats(), BuildingKind::Tent.stats(), UnitKind::Ranger.stats());
    assert!(barracks.worker_cost > 1 && tent.workers > 0 && ranger.worker_cost == 1);
    // One unit stays (it dies at the end). As many workers as the barracks needs, minus one.
    world.units.truncate(1);
    world.extra_workers = 0;
    world.extra_workers = (barracks.worker_cost - 1).saturating_sub(world.free_workers());
    // Use the free workers up until one is missing for the barracks.
    while world.free_workers() >= barracks.worker_cost {
        world.spawn_unit(UnitKind::Ranger, world.base + vec2(0.0, 4.0));
    }
    let tile = (world.base.x as i32 - 7, world.base.y as i32 - 8);
    let ground = (tile.1 - 2..tile.1 + 5).flat_map(|y| (tile.0 - 2..tile.0 + 5).map(move |x| (x, y))).collect();
    let place = Order::PlaceBuilding { kind: BuildingKind::Barracks, tile, turned: false, editor: false };
    world.step(&[Order::PaintTerrain { terrain: Terrain::Grass, tiles: ground }, place.clone()]);
    assert!(!world.buildings.iter().any(|b| b.kind == BuildingKind::Barracks), "built without enough workers");

    // Tents bring more (one is missing: one tent is enough): now the barracks goes up.
    let tent_tile = (world.base.x as i32 + 5, world.base.y as i32 - 5);
    let free = world.free_workers();
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tent, tile: tent_tile, turned: false, editor: false }]);
    assert_eq!(world.free_workers(), free + tent.workers);
    world.step(&[place]);
    let id = world.buildings.iter().find(|b| b.kind == BuildingKind::Barracks).expect("no barracks").id;

    // Training: as many as there are free workers, the rest is ignored.
    let free = world.free_workers() as usize;
    assert!(free < MAX_PRODUCTION, "the test wants fewer free workers than the queue has room");
    world.step(&vec![Order::Train { building: id, kind: UnitKind::Ranger }; free + 2]);
    let b = world.find_building(id).unwrap();
    assert_eq!(world.buildings[b].production.len(), free);
    assert_eq!(world.free_workers(), 0);
    world.step(&[Order::CancelTrain { building: id, index: free - 1 }]);
    assert_eq!(world.free_workers(), 1, "a cancelled unit gives its worker back");
    world.units[0].hp = 0;
    world.step(&[]);
    assert_eq!(world.free_workers(), 2, "a dead unit gives its worker back");
}

/// The editor never paints over buildings – the command center stays.
#[test]
fn paint_leaves_buildings_alone() {
    let (mut world, _) = quiet_world();
    let cc = world.buildings[0].tile;
    let ground = world.map.terrain(cc.0, cc.1);
    world.step(&[Order::PaintTerrain { terrain: Terrain::Water, tiles: vec![cc, (cc.0 + 1, cc.1 + 1)] }]);
    assert_eq!(world.map.terrain(cc.0, cc.1), ground);
    assert_eq!(world.buildings.len(), 1);
}

/// Base cut off by water: new zombies simply don't come – the game doesn't hang.
#[test]
fn sealed_off_base_does_not_hang() {
    let (mut world, _) = quiet_world();
    let (cx, cy) = (world.base.x as i32, world.base.y as i32);
    let r = 8;
    let ring: Vec<(i32, i32)> = (cy - r..=cy + r)
        .flat_map(|y| (cx - r..=cx + r).map(move |x| (x, y)))
        .filter(|&(x, y)| (x - cx).abs() == r || (y - cy).abs() == r)
        .collect();
    world.step(&[Order::PaintTerrain { terrain: Terrain::Water, tiles: ring }]);
    world.step(&[Order::SpawnZombies { count: 20 }, Order::SpawnWave { count: 20, from: WaveFrom::Sides(2) }]);
    assert!(world.zombies.is_empty());
}

/// A unit on its way, then a mountain appears across its path: it walks around it instead of
/// getting stuck at the mountain (its path is recomputed).
#[test]
fn units_walk_around_new_terrain() {
    let (mut world, id) = quiet_world();
    let start = world.units.last().unwrap().pos;
    let goal = start + vec2(0.0, 7.0);
    world.step(&[Order::Command { units: vec![id], command: Command::Move(goal), queue: false, formation: None }]);
    let (sx, sy) = (start.x.floor() as i32, start.y.floor() as i32);
    let line: Vec<(i32, i32)> = (sx - 3..=sx + 3).map(|x| (x, sy + 3)).collect();
    world.step(&[Order::PaintTerrain { terrain: Terrain::Mountain, tiles: line }]);
    for _ in 0..300 {
        world.step(&[]);
    }
    let unit = &world.units[world.find_unit(id).unwrap()];
    assert!(unit.pos.distance(goal) < 0.5, "stuck at {:?}", unit.pos);
}

/// The world border: a unit sent far outside stops at it; no unit can be put outside;
/// zombies start outside and walk in.
#[test]
fn world_border_keeps_units_in_and_lets_zombies_in() {
    let (mut world, id) = quiet_world();
    let area = world.map.play_area();
    let outside = vec2(2.0, world.base.y);
    world.step(&[
        Order::Command { units: vec![id], command: Command::Move(outside), queue: false, formation: None },
        Order::SpawnUnit { kind: UnitKind::Soldier, point: outside },
    ]);
    let units = world.units.len();
    for _ in 0..1000 {
        world.step(&[]);
        assert!(world.units.iter().all(|u| area.contains(u.pos)), "a unit left the play area");
    }
    assert_eq!(world.units.len(), units, "a unit was spawned outside the border");

    world.step(&[Order::SpawnWave { count: 50, from: WaveFrom::Sides(1) }]);
    assert!(world.zombies.iter().all(|z| !area.contains(z.pos)), "zombies have to start outside the border");
    world.step(&[Order::ClearZombies, Order::SpawnZombies { count: 100 }]);
    for _ in 0..400 {
        world.step(&[]);
    }
    assert!(world.zombies.iter().any(|z| area.contains(z.pos)), "no zombie came through the border");
}

/// The spawner a point lies in – `None` if it's in none.
fn spawner_of(world: &World, p: Vec2) -> Option<usize> {
    world.spawners.iter().position(|s| s.contains(p))
}

/// 3 spawners per side at the start. Small wave: all from one spawner. Two sides: from the spawners of exactly
/// two sides, each side the full count.
#[test]
fn waves_come_from_the_spawners() {
    let (mut world, _) = quiet_world();
    assert_eq!(world.spawners.len(), 12);
    // `prev`: where they appeared – in the same tick they already took their first step.
    world.step(&[Order::SpawnWave { count: 100, from: WaveFrom::OneSpawner }]);
    let found: Vec<Option<usize>> = world.zombies.iter().map(|z| spawner_of(&world, z.prev)).collect();
    assert_eq!(found.len(), 100);
    assert!(found.iter().all(|&s| s.is_some() && s == found[0]), "not all from one spawner");

    world.step(&[Order::ClearZombies, Order::SpawnWave { count: 100, from: WaveFrom::Sides(2) }]);
    let mut per_side = [0; 4];
    for z in &world.zombies {
        let s = spawner_of(&world, z.prev).expect("a zombie outside every spawner");
        per_side[world.spawners[s].side(&world.map) as usize] += 1;
    }
    let sides: Vec<i32> = per_side.into_iter().filter(|&count| count > 0).collect();
    assert_eq!(sides, vec![100, 100]);
}

/// A wave from one side comes along the whole side like a TAB swarm, not out of 3 points: the spawners of a new map
/// cover their side, so the zombies are spread over nearly its whole length – every fifth of it gets some.
#[test]
fn a_wave_comes_along_the_whole_side() {
    let (mut world, _) = quiet_world();
    world.step(&[Order::SpawnWave { count: 300, from: WaveFrom::Sides(1) }]);
    let first = spawner_of(&world, world.zombies[0].prev).expect("outside every spawner");
    let side = world.spawners[first].side(&world.map);
    // Along the side, in the screen axes: across for the sides at the top and bottom (0, 1), down for the left and
    // right (2, 3).
    let along: Vec<f32> = world.zombies.iter().map(|z| to_frame(z.prev)).map(|f| if side < 2 { f.x } else { f.y }).collect();
    let play = world.map.play_area();
    let (lo, hi) = if side < 2 { (play.min.x, play.max.x) } else { (play.min.y, play.max.y) };
    let fifth = (hi - lo) / 5.0;
    for k in 0..5 {
        let from = lo + fifth * k as f32;
        let count = along.iter().filter(|&&a| a >= from && a < from + fifth).count();
        assert!(count > 20, "only {count} of 300 zombies in fifth {k} of side {side}");
    }
}

/// Without spawners no zombie comes at all.
#[test]
fn no_spawners_no_zombies() {
    let (mut world, _) = quiet_world();
    let remove: Vec<Order> = world.spawners.iter().map(|s| Order::RemoveSpawner { spawner: s.id }).collect();
    world.step(&remove);
    world.step(&[
        Order::SpawnZombies { count: 50 },
        Order::SpawnWave { count: 50, from: WaveFrom::Sides(4) },
        Order::SpawnWave { count: 50, from: WaveFrom::OneSpawner },
    ]);
    assert!(world.zombies.is_empty());
}

/// Editor: place a spawner, resize and rename it. It never leaves the map and is at least 1 × 1;
/// an empty name is ignored.
#[test]
fn spawners_can_be_placed_resized_and_renamed() {
    let (mut world, _) = quiet_world();
    let id = world.next_spawner_id();
    // In the screen axes (`map::to_frame`): the base is at (0, size), the left edge of the map at −size / 2.
    let n = world.map.size as i32;
    world.step(&[Order::PlaceSpawner { center: (0, n) }]);
    let s = world.find_spawner(id).expect("not placed");
    assert_eq!(world.spawners[s].name, format!("Spawner {id}"));
    assert!(world.spawners[s].contains(world.base));
    world.step(&[
        Order::ResizeSpawner { spawner: id, min: (-n / 2 - 5, n), size: (4, 0) },
        Order::RenameSpawner { spawner: id, name: "  Graveyard ".to_owned() },
        Order::RenameSpawner { spawner: id, name: "   ".to_owned() },
    ]);
    let s = &world.spawners[world.find_spawner(id).unwrap()];
    assert_eq!((s.min, s.size), ((-n / 2, n), (4, 1)));
    assert_eq!(s.name, "Graveyard");
}

/// A medic sent onto a unit with T walks to it and keeps the green beam on it. A soldier given the
/// same command attack-moves there instead. The medic never shoots at zombies.
#[test]
fn medic_beams_at_its_target() {
    let (mut world, id) = quiet_world();
    let start = world.base + vec2(-6.0, 6.0);
    world.spawn_unit(UnitKind::Medic, start);
    let medic = world.units.last().unwrap().id;
    let soldier = world.units[0].id;
    world.step(&[
        Order::Command { units: vec![medic, soldier], command: Command::Heal(id), queue: false, formation: None },
        Order::SpawnZombiesAt { count: 1, point: start + vec2(-1.0, 0.0) },
    ]);
    let soldier_task = world.units[world.find_unit(soldier).unwrap()].order_name();
    assert!(soldier_task == "Attack-move" || soldier_task == "Idle", "soldier: {soldier_task}");
    assert_eq!(world.units[world.find_unit(medic).unwrap()].order_name(), "Heal");
    let target = world.units[world.find_unit(id).unwrap()].pos;
    let mut beamed = false;
    for _ in 0..200 {
        world.step(&[]);
        beamed |= world.beams.iter().any(|&(_, to)| to.distance(target) < 0.5);
    }
    assert!(beamed, "no beam at the target");
    assert_eq!(world.units[world.find_unit(medic).unwrap()].kills, 0);
}

/// An idle medic aims at hurt units nearby by itself – but not at healthy ones.
#[test]
fn idle_medic_beams_at_hurt_units_only() {
    let (mut world, id) = quiet_world();
    let pos = world.units[world.find_unit(id).unwrap()].pos;
    world.spawn_unit(UnitKind::Medic, pos + vec2(1.5, 0.0));
    world.step(&[]);
    assert!(world.beams.is_empty(), "beam at a healthy unit");
    let i = world.find_unit(id).unwrap();
    world.units[i].hp = 10;
    // First it turns towards the hurt unit (`turn_around_seconds`), then the beam is on.
    let mut ticks = 0;
    while world.beams.is_empty() && ticks < 20 {
        world.step(&[]);
        ticks += 1;
    }
    assert_eq!(world.beams.len(), 1, "no beam after {ticks} ticks");
}

/// The barracks builds a unit; it comes out and walks to the rally point.
#[test]
fn barracks_trains_and_sends_to_rally() {
    let (mut world, _) = quiet_world();
    let barracks = barracks(&mut world);
    let rally = world.base + vec2(6.0, -6.0);
    world.step(&[
        Order::SetRally { building: barracks, rally: Rally::Point(rally) },
        Order::Train { building: barracks, kind: UnitKind::Soldier },
    ]);
    let before = world.units.len();
    for _ in 0..UnitKind::Soldier.stats().build_time + 200 {
        world.step(&[]);
    }
    assert_eq!(world.units.len(), before + 1);
    let newest = world.units.last().unwrap();
    assert!(newest.pos.distance(rally) < 2.0, "stands at {:?}", newest.pos);
}

/// Only the barracks trains: the command center takes no unit into its queue, and no gold is paid.
#[test]
fn only_the_barracks_trains_units() {
    let (mut world, _) = quiet_world();
    let cc = world.buildings[0].id;
    let gold = world.gold;
    world.apply_orders(&[Order::Train { building: cc, kind: UnitKind::Soldier }]);
    assert!(world.buildings[0].production.is_empty(), "the command center trains");
    assert_eq!(world.gold, gold);
    let barracks = barracks(&mut world);
    world.apply_orders(&[Order::Train { building: barracks, kind: UnitKind::Soldier }]);
    let b = world.find_building(barracks).unwrap();
    assert_eq!(world.buildings[b].production.len(), 1);
    assert_eq!(world.gold, gold - UnitKind::Soldier.stats().cost);
}

/// More than 5 in the queue isn't possible, and cancelling takes one out.
#[test]
fn production_queue_is_limited() {
    let (mut world, _) = quiet_world();
    let cc = barracks(&mut world);
    let b = world.find_building(cc).unwrap();
    let train = Order::Train { building: cc, kind: UnitKind::Sniper };
    world.step(&[
        train.clone(),
        train.clone(),
        train.clone(),
        train.clone(),
        train.clone(),
        train,
    ]);
    assert_eq!(world.buildings[b].production.len(), MAX_PRODUCTION);
    world.step(&[Order::CancelTrain { building: cc, index: 0 }]);
    assert_eq!(world.buildings[b].production.len(), MAX_PRODUCTION - 1);
    // The build starts from the beginning – a build tick has already run again in the same tick.
    assert_eq!(world.buildings[b].progress, 1);
}

/// Plain grass in a square of radius `r` around `center` – no tree in the test's way.
fn clear_ground(world: &mut World, center: (i32, i32), r: i32) {
    let tiles = (center.1 - r..=center.1 + r).flat_map(|y| (center.0 - r..=center.0 + r).map(move |x| (x, y))).collect();
    world.step(&[Order::PaintTerrain { terrain: Terrain::Grass, tiles }]);
}

/// A unit that stays put and never shoots – so the zombies stay alive in the test.
fn bait(world: &mut World, kind: UnitKind, pos: Vec2) -> u32 {
    world.spawn_unit(kind, pos);
    let unit = world.units.last_mut().unwrap();
    unit.cooldown = u32::MAX;
    let id = unit.id;
    world.step(&[Order::Command { units: vec![id], command: Command::Hold, queue: false, formation: None }]);
    id
}

/// The scene from the screenshot, now with noise like TAB's (`sound.rs`): a loud fight behind a wall, water in front of
/// it, zombies 8 tiles away on the other side – too far to see anyone. They hear it, go around the water to where the
/// noise is and tear the wall down on the way (the way of the lured goes straight through walls, `lure.rs`).
#[test]
fn zombies_hear_the_noise_and_storm_the_wall() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    // Far enough from the command center that the zombies don't notice it instead (`notice_buildings`).
    let p = (world.base.x as i32 - 20, world.base.y as i32);
    clear_ground(&mut world, p, 10);
    let wall: Vec<Order> = (p.1 - 4..=p.1 + 4)
        .map(|y| Order::PlaceBuilding { kind: BuildingKind::Wall, tile: (p.0 + 2, y), turned: false, editor: false })
        .collect();
    world.step(&wall);
    let water = (p.1 - 3..=p.1 + 3).map(|y| (p.0 + 5, y)).collect();
    world.step(&[Order::PaintTerrain { terrain: Terrain::Water, tiles: water }]);
    let wanderers = Order::PlaceZombies { center: vec2(p.0 as f32 + 8.5, p.1 as f32 + 0.5), radius: 1.5, count: 20, breed: Breed::Runner, wander: true };
    world.step(&[wanderers]);
    let noise = vec2(p.0 as f32 + 0.5, p.1 as f32 + 0.5);
    for _ in 0..400 {
        // The fight goes on: loud enough for runners, every tick a bit more.
        world.make_sound(noise, 20);
        world.step(&[]);
    }
    let damaged = world.buildings.iter().filter(|b| b.kind == BuildingKind::Wall && b.hp < b.stats().hp).count();
    assert!(damaged > 0, "no zombie stormed the wall");
}

/// A hit zombie is alarmed. An alarmed zombie storms at units it can't see (12 tiles, a runner sees 6) and hits the
/// tower they sit in – without the alarm the same zombie walks past to the command center. 12, not further: in its
/// 5 s of alarm even a slow runner gets within sight, then it keeps coming because it sees the tower's units.
#[test]
fn an_alarmed_zombie_storms_unseen_units() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(8.0, -8.0);
    world.spawn_zombie(p + vec2(6.0, 0.0));
    world.zombies[0].hp = 100_000;
    world.spawn_unit(UnitKind::Sniper, p);
    // The sniper first turns and aims (`data\units.json`) – then it shoots.
    for _ in 0..40 {
        world.step(&[]);
        if world.zombies[0].alert > 0 {
            break;
        }
    }
    assert!(!world.shots.is_empty() || world.zombies[0].alert > 0, "no shot fell");
    assert!(world.zombies[0].alert > 0, "being shot doesn't alarm");

    let run = |alarmed: bool| {
        let (mut world, _) = quiet_world();
        world.units.clear();
        let t = (world.base.x as i32, world.base.y as i32 - 30);
        clear_ground(&mut world, t, 18);
        world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tower, tile: t, turned: false, editor: false }]);
        let tower = world.buildings.last().unwrap().id;
        let center = world.buildings.last().unwrap().center();
        world.spawn_zombie(center + vec2(12.0, 0.0));
        if alarmed {
            world.zombies[0].alert = values().alert;
        }
        world.spawn_unit(UnitKind::Sniper, center);
        let sniper = world.units.last_mut().unwrap();
        sniper.inside = Some(tower);
        sniper.cooldown = u32::MAX; // the alarm alone has to bring it
        for _ in 0..600 {
            world.step(&[]);
        }
        let tower = &world.buildings[world.find_building(tower).unwrap()];
        tower.hp < tower.stats().hp
    };
    assert!(run(true), "the alarmed zombie didn't come for the tower");
    assert!(!run(false), "the zombie came although it neither saw nor was alarmed");
}

/// A periodic spawner sends its zombies every few seconds, one that is off sends nothing by itself – and killed
/// zombies never come back.
#[test]
fn spawners_send_periodically_or_not_and_the_dead_stay_dead() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let ids: Vec<u32> = world.spawners.iter().map(|s| s.id).collect();
    let mut orders: Vec<Order> = ids.iter().map(|&spawner| Order::SetSpawnMode { spawner, mode: SpawnMode::Off }).collect();
    orders.push(Order::SetSpawnMode { spawner: ids[0], mode: SpawnMode::Periodic { every: 2, count: 7 } });
    world.step(&orders);
    for _ in 0..39 {
        world.step(&[]);
    }
    assert_eq!(world.zombies.len(), 7, "after 2 s: one batch");
    let inside = |z: &Zombie| world.spawners[0].contains(z.prev);
    assert!(world.zombies.iter().all(inside), "not from the periodic spawner");
    for _ in 0..40 {
        world.step(&[]);
    }
    assert_eq!(world.zombies.len(), 14, "after 4 s: two batches");

    // Killed: they stay dead – even with spawners set to "constant".
    world.step(&[Order::SetSpawnMode { spawner: ids[0], mode: SpawnMode::Off }, Order::SetSpawnMode { spawner: ids[3], mode: SpawnMode::Constant }]);
    for z in &mut world.zombies {
        z.hp = 0;
    }
    world.step(&[]);
    assert!(world.zombies.is_empty());
}

/// The constant spawn as in `data\spawning.json`: nothing on day 1, from day 2 small groups of walkers that come more
/// and more often, from day 10 runners instead, from day 15 the first specials along, and after the last stage the
/// groups keep growing.
#[test]
fn constant_spawn_follows_the_stages() {
    let at = |day: u64, hour: u64| (day - 1) * DAY_TICKS + hour * HOUR_TICKS;
    let spawning = &data::get().spawning.constant;
    let group_at = |tick| spawning::group_at(spawning, tick);
    assert_eq!(group_at(at(1, 23)), None, "something on day 1");
    let early = group_at(at(2, 0)).unwrap();
    assert_eq!((early.breed, early.min, early.max), (Breed::Walker, 2, 3));
    let later = group_at(at(6, 0)).unwrap();
    assert_eq!(later.breed, Breed::Walker);
    assert!(later.every < early.every, "walkers don't come more often: {} vs {} ticks", later.every, early.every);
    let runners = group_at(at(10, 0)).unwrap();
    assert_eq!((runners.breed, runners.min, runners.max), (Breed::Runner, 4, 6));
    assert!(runners.specials.is_empty(), "specials before their day");
    assert_eq!(group_at(at(15, 0)).unwrap().specials, vec![(Breed::Armored, 0.15)]);
    let last = spawning.stages.last().unwrap();
    let grown = group_at(at(last.day + 5, last.hour)).unwrap();
    assert!(grown.min > last.group[0], "groups don't grow after the last stage");

    // In the world: the first walkers come out of a "constant" spawner right at the start of day 2 – together.
    let (mut world, _) = quiet_world();
    world.units.clear();
    world.tick = at(2, 0) - 1;
    world.step(&[]);
    assert!(world.zombies.is_empty(), "too early");
    world.step(&[]);
    assert!((2..=3).contains(&world.zombies.len()), "{} walkers", world.zombies.len());
    assert!(world.zombies.iter().all(|z| z.breed == Breed::Walker));
    assert!(world.spawners.iter().any(|s| s.mode == SpawnMode::Constant && s.contains(world.zombies[0].prev)));
}

/// A group of the constant spawn comes crowded on one spot – not spread over its spawner, which is nearly a whole side
/// of the map. Its specials take their share of it (here all of it), and the colony's unrest makes every group bigger.
#[test]
fn a_constant_group_comes_crowded_with_specials_and_unrest() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let stage = data::Stage { day: 1, hour: 0, breed: Breed::Runner, every_seconds: 60.0, group: [6, 6] };
    let fat = data::ConstantSpecial { day: 1, breed: Breed::Fat, share: 1.0 };
    world.constant = data::ConstantSpawn { stages: vec![stage], extra_per_day: 0.0, specials: vec![fat] };
    world.unrest = 2 * data::get().sound.unrest_per_zombie;
    world.next_group = world.tick;
    world.step(&[]);
    assert_eq!(world.zombies.len(), 8, "6 and 2 for the unrest");
    assert!(world.zombies.iter().all(|z| z.breed == Breed::Fat), "the special didn't take its share");
    let spread = world.zombies.iter().flat_map(|a| world.zombies.iter().map(move |b| a.prev.distance(b.prev))).fold(0.0, f32::max);
    assert!(spread < 5.0, "the group is spread over {spread} tiles");
}

/// Machines make noise every work beat – a mine 150, a sawmill 100, as much as their power lets them – and it adds to
/// the unrest, which never goes down. Switched off: quiet.
#[test]
fn machines_make_noise_and_the_unrest_grows() {
    let (mut world, _) = quiet_world();
    no_more_zombies(&mut world);
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    clear_ground(&mut world, (bx - 9, by - 9), 4);
    clear_ground(&mut world, (bx + 6, by - 9), 4);
    world.apply_orders(&[
        Order::PlaceBuilding { kind: BuildingKind::Mine, tile: (bx - 9, by - 9), turned: false, editor: true },
        Order::PlaceBuilding { kind: BuildingKind::Sawmill, tile: (bx + 6, by - 9), turned: false, editor: true },
    ]);
    let beat = data::get().sound.work_every as u64;
    let work_beat = |world: &mut World| {
        while !(world.tick + 1).is_multiple_of(beat) {
            world.step(&[]);
        }
        let before = world.unrest;
        world.step(&[]);
        world.unrest - before
    };
    let (mine, sawmill) = (BuildingKind::Mine.stats().noise as u64, BuildingKind::Sawmill.stats().noise as u64);
    assert_eq!(work_beat(&mut world), mine + sawmill);
    assert!(world.loudest_sound().is_some_and(|(_, level)| level >= mine as u32), "no noise at the mine");
    let id = world.buildings.iter().find(|b| b.kind == BuildingKind::Mine).unwrap().id;
    world.step(&[Order::SwitchOff { buildings: vec![id], off: true }]);
    assert_eq!(work_beat(&mut world), sawmill, "the mine is loud while switched off");
    // The unrest makes every group of the constant spawn bigger – for good.
    let base = world.spawn_group().map_or(0, |g| g.min);
    world.unrest = 3 * data::get().sound.unrest_per_zombie;
    assert_eq!(world.spawn_group().map_or(0, |g| g.min), base + 3 * world.spawn_group().is_some() as u32);
}

/// Whatever water and mountains the map generation makes: every spawner has a way to the base.
#[test]
fn every_spawner_reaches_the_base() {
    for seed in 1..=15 {
        let world = World::new(seed);
        for s in &world.spawners {
            let c = s.region().center();
            assert!(world.to_base.reachable(c.x as i32, c.y as i32), "seed {seed}: {} is cut off", s.name);
        }
    }
}

/// A tower with a rally point: unloaded units come out on that side and walk there.
#[test]
fn tower_unloads_towards_its_rally_point() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let t = (world.base.x as i32 + 8, world.base.y as i32 - 8);
    clear_ground(&mut world, t, 8);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tower, tile: t, turned: false, editor: false }]);
    let tower = world.buildings.last().unwrap().id;
    let center = world.buildings.last().unwrap().center();
    for k in 0..2 {
        world.spawn_unit(UnitKind::Soldier, center + vec2(k as f32, 0.0));
        world.units.last_mut().unwrap().inside = Some(tower);
    }
    let rally = center + vec2(0.0, -6.0);
    world.step(&[Order::SetRally { building: tower, rally: Rally::Point(rally) }, Order::Unload { building: tower }]);
    assert!(world.units.iter().all(|u| !u.is_inside() && u.pos.y < center.y), "not out on the rally side");
    for _ in 0..200 {
        world.step(&[]);
    }
    assert!(world.units.iter().all(|u| u.pos.distance(rally) < 1.5), "didn't walk to the rally point");
}

/// Units walk in a straight line at any angle – not first diagonally until one axis fits and then straight
/// (the old 8-direction staircase). Around a wall they go taut past its end and then straight again.
#[test]
fn units_walk_straight_at_any_angle() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let start = world.base + vec2(-6.5, 5.5);
    world.spawn_unit(UnitKind::Soldier, start);
    let id = world.units[0].id;
    let goal = start + vec2(9.0, 3.0); // a flat angle: the staircase would be far off the line
    world.step(&[Order::Command { units: vec![id], command: Command::Move(goal), queue: false, formation: None }]);
    let mut worst: f32 = 0.0;
    for _ in 0..120 {
        world.step(&[]);
        let p = world.units[0].pos;
        // Distance of p from the line start → goal.
        let along = (goal - start).normalize();
        let off = (p - start) - along * (p - start).dot(along);
        worst = worst.max(off.length());
    }
    assert!(world.units[0].pos.distance(goal) < 0.1, "didn't arrive: {:?}", world.units[0].pos);
    assert!(worst < 0.3, "left the straight line by {worst} tiles");

    // A wall across the way: around its end, the detour stays short.
    let wall_x = (goal.x + 3.0) as i32;
    let wall: Vec<Order> = (goal.y as i32 - 4..=goal.y as i32 + 1)
        .map(|y| Order::PlaceBuilding { kind: BuildingKind::Wall, tile: (wall_x, y), turned: false, editor: false })
        .collect();
    world.step(&wall);
    let target = goal + vec2(6.0, 0.0);
    world.step(&[Order::Command { units: vec![id], command: Command::Move(target), queue: false, formation: None }]);
    let mut walked = 0.0;
    for _ in 0..200 {
        let before = world.units[0].pos;
        world.step(&[]);
        walked += world.units[0].pos.distance(before);
    }
    assert!(world.units[0].pos.distance(target) < 0.1, "didn't get around the wall: {:?}", world.units[0].pos);
    // Straight would be 6; around the end of the wall (1–2 tiles below) a taut path is well under 9.
    assert!(walked < 9.0, "walked {walked} tiles – not taut around the corner");
}

/// Orders given in the pause and taken over at once (`apply_orders`), then a tick – exactly the same as giving
/// them with that tick. That's why showing them in the pause doesn't break determinism.
#[test]
fn orders_in_the_pause_are_the_same_as_with_the_next_tick() {
    let run = |paused: bool| {
        let mut world = World::new(9);
        let barracks = barracks(&mut world);
        let ids: Vec<u32> = world.units.iter().map(|u| u.id).collect();
        let orders = vec![
            Order::Command { units: ids[0..4].to_vec(), command: Command::AttackMove(world.base + vec2(8.0, 2.0)), queue: false, formation: None },
            Order::Train { building: barracks, kind: UnitKind::Sniper },
            Order::PlaceBuilding { kind: BuildingKind::Wall, tile: (world.base.x as i32 + 5, world.base.y as i32 - 6), turned: false, editor: false },
        ];
        if paused {
            world.apply_orders(&orders);
            // Taken over, but nobody has moved yet:
            assert_eq!(world.units[0].order_name(), "Attack-move");
            assert_eq!(world.buildings[world.find_building(barracks).unwrap()].production.len(), 1);
            assert_eq!(world.tick, 0);
            world.step(&[]);
        } else {
            world.step(&orders);
        }
        for _ in 0..100 {
            world.step(&[]);
        }
        world.checksum()
    };
    assert_eq!(run(true), run(false));
}

/// In the editor a building goes anywhere – on water (which stays water under it), outside the border, right next
/// to zombies and onto a unit (who is pushed aside). Only not onto another building.
#[test]
fn editor_builds_anywhere_but_on_buildings() {
    let (mut world, id) = quiet_world();
    let unit_tile = {
        let p = world.units[world.find_unit(id).unwrap()].pos;
        (p.x.floor() as i32, p.y.floor() as i32)
    };
    let water = (world.base.x as i32 + 9, world.base.y as i32 + 9);
    world.step(&[Order::PaintTerrain { terrain: Terrain::Water, tiles: vec![water] }, Order::SpawnZombiesAt { count: 5, point: vec2(5.5, 5.5) }]);
    let place = |tile| Order::PlaceBuilding { kind: BuildingKind::Wall, tile, turned: false, editor: true };
    let before = world.buildings.len();
    world.step(&[place(water), place((4, 4)), place(unit_tile), place(world.buildings[0].tile)]);
    assert_eq!(world.buildings.len(), before + 3, "on the command center it must not go");
    assert_eq!(world.map.terrain(water.0, water.1), Terrain::Water, "the water under it has to stay water");
    assert!(nobody_on_blocked_ground(&world), "the unit wasn't pushed aside");
    // The game's rules still apply without the editor.
    assert!(!world.can_place(BuildingKind::Wall, (3, 3), false));
}

/// The editor's zombies: the breed asked for, and wanderers stroll around – no home they keep to, but never into the
/// ring around the command center, and they don't head for it – until they see a unit, then they storm like everyone
/// else. Near the command center they stay awake (`sleep.rs`).
#[test]
fn placed_zombies_have_a_breed_and_wanderers_stroll() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let home = world.base + vec2(12.0, -12.0);
    clear_ground(&mut world, (home.x as i32, home.y as i32), 8);
    world.step(&[Order::PlaceZombies { center: home, radius: 2.0, count: 20, breed: Breed::Runner, wander: true }]);
    assert_eq!(world.zombies.len(), 20);
    assert!(world.zombies.iter().all(|z| z.breed == Breed::Runner && z.wander.is_some()));
    let start: Vec<Vec2> = world.zombies.iter().map(|z| z.pos).collect();
    for _ in 0..400 {
        world.step(&[]);
    }
    assert_eq!(world.zombies.len(), 20, "a wanderer walked off or fell asleep next to the command center");
    let keep_out = world.keep_out.of(Breed::Runner);
    assert!(world.zombies.iter().all(|z| z.wander.is_some() && !z.siege), "a wanderer heads for the base");
    assert!(world.zombies.iter().all(|z| z.pos.distance(world.base) > keep_out - 1.0), "a wanderer came into the ring");
    let moved = world.zombies.iter().zip(&start).filter(|(z, s)| z.pos.distance(**s) > 1.0).count();
    assert!(moved > 0, "nobody strolls");

    // A soldier (that doesn't shoot – no noise) appears right next to one of them: whoever sees it storms, whoever
    // stands further away than a runner sees (6 tiles, TAB) keeps wandering.
    let at = world.zombies[0].pos;
    world.spawn_unit(UnitKind::Soldier, at);
    world.units[0].cooldown = u32::MAX;
    world.step(&[]);
    world.step(&[]);
    let sight = Breed::Runner.stats().sight;
    let near: Vec<&Zombie> = world.zombies.iter().filter(|z| z.pos.distance(at) < sight - 1.0).collect();
    assert!(!near.is_empty());
    assert!(near.iter().all(|z| z.wander.is_none()), "still wandering with a soldier in sight");
    let far = world.zombies.iter().filter(|z| z.pos.distance(at) > sight + 1.0);
    assert!(far.into_iter().all(|z| z.wander.is_some()), "noticed a soldier it can't see");
}

/// Every breed strolls up to its `stroll` per trip – the rarer ones further: in the same time runners get further from
/// where they were put than walkers.
#[test]
fn rarer_breeds_stroll_further() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let home = world.base + vec2(20.0, 0.0);
    clear_ground(&mut world, (home.x as i32, home.y as i32), 24);
    let walker = Breed::Walker.stats().stroll;
    assert!(Breed::Runner.stats().stroll > walker * 1.5, "runners should stroll much further than walkers");
    for breed in [Breed::Walker, Breed::Runner] {
        world.step(&[Order::PlaceZombies { center: home, radius: 0.5, count: 10, breed, wander: true }]);
    }
    // Furthest from home so far, per zombie (by its number) – also for the ones that fall asleep far out.
    let mut furthest: std::collections::BTreeMap<u32, (Breed, f32)> = std::collections::BTreeMap::new();
    for _ in 0..1200 {
        world.step(&[]);
        for z in &world.zombies {
            let entry = furthest.entry(z.id).or_insert((z.breed, 0.0));
            entry.1 = entry.1.max(z.pos.distance(home));
        }
    }
    let average = |breed: Breed| {
        let ds: Vec<f32> = furthest.values().filter(|(b, _)| *b == breed).map(|(_, d)| *d).collect();
        ds.iter().sum::<f32>() / ds.len().max(1) as f32
    };
    let (walkers, runners) = (average(Breed::Walker), average(Breed::Runner));
    assert!(runners > walkers, "the runners stayed closer ({runners} tiles) than the walkers ({walkers})");
}

/// Every breed sees as far as `data\zombies.json` says: a runner 5.2 tiles from a soldier sees it (6 tiles), a
/// walker 5.8 tiles away doesn't (5). Seen is counted from the middle of the zombie's tile, so not exactly 5.5.
#[test]
fn zombies_see_as_far_as_their_breed() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(12.0, -12.0);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 8);
    bait(&mut world, UnitKind::Soldier, p);
    world.spawn_zombie_as(p + vec2(5.2, 0.0), Breed::Runner, true);
    world.spawn_zombie_as(p + vec2(-5.8, 0.0), Breed::Walker, true);
    for z in &mut world.zombies {
        z.speed = 0.0; // stay where they are
    }
    world.step(&[]);
    world.step(&[]);
    assert!(world.zombies[0].wander.is_none(), "the runner didn't see the soldier");
    assert!(world.zombies[1].wander.is_some(), "the walker saw further than 5 tiles");
}

/// Noise like in TAB adds up: a long fight of eight soldiers (3 per shot) is heard by a walker 12 tiles away – too far
/// to see them – and it comes to where the noise is. Eight rangers (1 per arrow) are quiet enough: it stays.
#[test]
fn a_long_loud_fight_is_heard_far_away() {
    let lured = |kind: UnitKind| {
        let (mut world, _) = quiet_world();
        world.units.clear();
        // The middle of a cell of the sound grid: the squad's noise adds up in one cell.
        let c = world.base + vec2(12.0, -12.0);
        let p = vec2((c.x / 4.0).floor() * 4.0 + 2.0, (c.y / 4.0).floor() * 4.0 + 2.0);
        clear_ground(&mut world, (p.x as i32, p.y as i32), 14);
        for k in 0..8 {
            world.spawn_unit(kind, p + vec2((k % 4) as f32 * 0.5 - 0.75, (k / 4) as f32 * 0.5 - 0.25));
        }
        // The targets: in front, standing still and too tough to die. Behind: the wanderer.
        for k in 0..3 {
            world.spawn_zombie_as(p + vec2(4.0, k as f32 - 1.0), Breed::Walker, false);
        }
        world.spawn_zombie_as(p + vec2(-12.0, 0.0), Breed::Walker, true);
        for z in &mut world.zombies[..3] {
            z.speed = 0.0;
            z.hp = 1_000_000;
        }
        // By its number: the list is sorted by place now and then (`sort_by_place`).
        let wanderer = world.zombies[3].id;
        for _ in 0..600 {
            world.step(&[]);
            if world.zombies.iter().any(|z| z.id == wanderer && z.lure.is_some()) {
                return true;
            }
        }
        false
    };
    assert!(lured(UnitKind::Soldier), "the soldiers' fight wasn't heard 12 tiles away");
    assert!(!lured(UnitKind::Ranger), "the rangers' arrows were heard 12 tiles away");
}

/// The sound grid: noise in the same cell adds up; once it's quiet for `quiet_seconds` it halves, then every
/// `halve_seconds` again, until it's gone.
#[test]
fn noise_adds_up_and_fades() {
    let (mut world, _) = quiet_world();
    let sound = &data::get().sound;
    let at = vec2((world.base.x / 4.0).floor() * 4.0 + 1.0, (world.base.y / 4.0).floor() * 4.0 + 1.0) + vec2(20.0, 20.0);
    world.make_sound(at, 100);
    world.make_sound(at + vec2(1.5, 1.5), 60);
    assert_eq!(world.loudest_sound().map(|(_, level)| level), Some(160), "it doesn't add up");
    for _ in 0..sound.quiet {
        world.step(&[]);
    }
    assert_eq!(world.loudest_sound().map(|(_, level)| level), Some(160), "it got quieter too early");
    world.step(&[]);
    assert_eq!(world.loudest_sound().map(|(_, level)| level), Some(80));
    for _ in 0..sound.halve {
        world.step(&[]);
    }
    assert_eq!(world.loudest_sound().map(|(_, level)| level), Some(40));
    for _ in 0..sound.halve * 8 {
        world.step(&[]);
    }
    assert_eq!(world.loudest_sound(), None, "it never went quiet");
}

/// A roamer that saw one of your units – and lost it – goes to where it last saw it and stays there, strolling around.
/// A wave zombie in the same spot heads on for the command center.
#[test]
fn a_zombie_goes_where_it_last_saw_a_unit_and_stays() {
    let run = |wander: bool| {
        let (mut world, _) = quiet_world();
        world.units.clear();
        let p = world.base + vec2(-14.0, 12.0);
        clear_ground(&mut world, (p.x as i32, p.y as i32), 9);
        world.spawn_zombie_as(p, Breed::Walker, wander);
        let id = bait(&mut world, UnitKind::Ranger, p + vec2(4.0, 0.0));
        for _ in 0..5 {
            world.step(&[]);
        }
        assert!(world.zombies[0].lure.is_some(), "it didn't see the unit");
        // The unit is gone – far away in an instant.
        let seen = world.zombies[0].lured_to().unwrap();
        let u = world.find_unit(id).unwrap();
        world.units[u].pos = world.base + vec2(0.0, 6.0);
        world.units[u].prev = world.units[u].pos;
        for _ in 0..500 {
            world.step(&[]);
        }
        (world.zombies[0].pos, seen, world.zombies[0].lure.is_none(), world.zombies[0].wander.is_some(), world.base)
    };
    let (pos, seen, done, strolling, _) = run(true);
    assert!(done && strolling, "the roamer doesn't stroll where it lost the unit");
    assert!(pos.distance(seen) < Breed::Walker.stats().stroll * 1.5 + 2.0, "it isn't where it last saw the unit");
    let (pos, seen, done, strolling, base) = run(false);
    assert!(done && !strolling, "the wave zombie stays");
    assert!(pos.distance(base) < seen.distance(base) - 1.0, "the wave zombie doesn't head on for the command center");
}

/// The final wave calls everyone: every zombie on the map belongs to it now and heads for the command center, every
/// house of doom empties, every hidden guard comes out of the ground. A wave that reveals the map shows all of it.
#[test]
fn the_final_wave_calls_everyone_and_reveals_the_map() {
    let mut world = World::new(5);
    assert!(!world.doom.is_empty() && !world.loot.is_empty());
    // A few per house – emptying them all takes long enough as it is.
    for house in &mut world.doom {
        house.inside = house.inside.min(20);
    }
    let roamers = world.zombies.len();
    let guards: u32 = world.loot.iter().map(|l| l.guards).sum();
    let inside: u32 = world.doom.iter().map(|d| d.inside).sum();
    world.waves.clear();
    let at = world.tick + 2;
    world.step(&[Order::AddTimedWave(TimedWave { at, count: 1, from: WaveFrom::Sides(1), breed: Breed::Runner, final_wave: true, reveal: true })]);
    while world.tick <= at {
        world.step(&[]);
    }
    assert!(world.revealed, "the map isn't shown");
    let n = world.map.size as i32;
    assert!((0..n).all(|k| world.vision.is_visible(k, n / 2)), "not all of the map is visible");
    assert!(world.zombies.iter().all(|z| z.final_wave && z.siege), "a zombie wasn't called");
    assert!(world.loot.iter().all(|l| l.guards == 0), "guards stayed in the ground");
    assert!(world.zombies.len() as u32 >= roamers as u32 + guards / 2, "the guards didn't come out");
    // A little later the first ones are out of the houses – with the final wave too. (Much later your fallen units rise
    // as walkers – those don't belong to it.)
    for _ in 0..40 {
        world.step(&[]);
    }
    assert!(world.zombies.len() as u32 > roamers as u32 + guards / 2, "nobody came out of the houses");
    assert!(world.zombies.iter().all(|z| z.final_wave), "a zombie of a house isn't part of the final wave");
    for _ in 0..(inside as f32 / data::get().doom.release_per_second * 20.0) as u32 + 40 {
        world.step(&[]);
    }
    assert!(world.doom.iter().all(|d| d.inside == 0), "a house didn't empty");
}

/// A building with workers that falls is infected, like in TAB: its workers come out as zombies where it stood – and
/// that is loud (`building_infected` + `per_infected` each, on top of `building_destroyed`). Placing a building makes
/// some noise too.
#[test]
fn a_fallen_hunter_cottage_brings_zombies_and_noise() {
    let (mut world, _) = quiet_world();
    let sound = &data::get().sound;
    let tile = (world.base.x as i32 + 9, world.base.y as i32 + 9);
    clear_ground(&mut world, tile, 3);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::HunterCottage, tile, turned: false, editor: false }]);
    assert_eq!(world.buildings.last().unwrap().kind, BuildingKind::HunterCottage, "not built");
    assert_eq!(world.loudest_sound().map(|(_, level)| level), Some(sound.building_placed), "building made no noise");
    // Quiet again – also the cottage's own work noise (`machines_work`): half a second before its next beat, the last
    // one has faded.
    let beat = sound.work_every as u64;
    for _ in 0..200 {
        world.step(&[]);
    }
    while (world.tick + 1) % beat != beat - 10 {
        world.step(&[]);
    }
    let b = world.buildings.len() - 1;
    let workers = world.buildings[b].stats().worker_cost;
    world.buildings[b].hp = 0;
    world.step(&[]);
    assert!(!world.buildings.iter().any(|b| b.kind == BuildingKind::HunterCottage));
    assert_eq!(world.zombies.len() as u32, workers, "its workers didn't come out");
    let loud = sound.building_destroyed + sound.building_infected + sound.per_infected * workers;
    assert_eq!(world.loudest_sound().map(|(_, level)| level), Some(loud));
}

/// Waves at the same time from the same kind of place come together – out of the same spawners: the runners of a wave
/// and the riots that come along.
#[test]
fn a_wave_and_its_specials_come_from_the_same_side() {
    let (mut world, _) = quiet_world();
    world.waves.clear();
    let at = world.tick + 2;
    let wave = |breed: Breed, count: u32| Order::AddTimedWave(TimedWave { at, count, from: WaveFrom::Sides(1), breed, final_wave: false, reveal: false });
    world.step(&[wave(Breed::Runner, 30), wave(Breed::Armored, 10)]);
    while world.tick <= at {
        world.step(&[]);
    }
    assert_eq!(world.zombies.len(), 40);
    // The side of the map they're closest to – a tick after coming out, some already stepped out of their spawner.
    let side = |p: Vec2| world.map.full_area().nearest_side(p);
    let first = side(world.zombies[0].pos);
    assert!(world.zombies.iter().all(|z| side(z.pos) == first), "they came from different sides");
}

/// A diamond of walls touching only at the corners (dragged diagonally) is closed: no zombie gets through a
/// corner, not even one looking for a gap – they can only tear it down.
#[test]
fn diagonal_walls_are_closed() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let c = (world.base.x as i32 + 10, world.base.y as i32 - 10);
    clear_ground(&mut world, c, 9);
    let r = 3;
    let ring: Vec<Order> = (c.1 - r..=c.1 + r)
        .flat_map(|y| (c.0 - r..=c.0 + r).map(move |x| (x, y)))
        .filter(|&(x, y)| (x - c.0).abs() + (y - c.1).abs() == r)
        .map(|tile| Order::PlaceBuilding { kind: BuildingKind::Wall, tile, turned: false, editor: false })
        .collect();
    world.step(&ring);
    for b in &mut world.buildings {
        b.hp = 1_000_000; // they mustn't break through – only slip through
    }
    let inside = vec2(c.0 as f32 + 0.5, c.1 as f32 + 0.5);
    let id = bait(&mut world, UnitKind::Soldier, inside);
    world.step(&[Order::SpawnZombiesAt { count: 40, point: inside + vec2(6.0, 0.0) }]);
    for zombie in &mut world.zombies {
        zombie.seek = u32::MAX; // all of them look for a gap the whole time
    }
    let diamond =|p: Vec2| (p.x.floor() as i32 - c.0).abs() + (p.y.floor() as i32 - c.1).abs() < r;
    for _ in 0..400 {
        world.step(&[]);
        assert!(world.zombies.iter().all(|z| !diamond(z.pos)), "a zombie slipped through a corner");
    }
    let unit = &world.units[world.find_unit(id).unwrap()];
    assert_eq!(unit.hp, unit.stats().hp);
}

/// A ring of walls with a gate: your unit walks in through the door, the zombies outside don't get in.
#[test]
fn gate_lets_units_in_but_not_zombies() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let c = (world.base.x as i32 + 10, world.base.y as i32 - 10);
    clear_ground(&mut world, c, 9);
    // The ring has a gap of 3 at the top – the gate goes there (not turned: along x, door in the middle).
    world.step(&wall_ring(c, 3, &[(c.0 - 1, c.1 - 3), (c.0, c.1 - 3), (c.0 + 1, c.1 - 3)]));
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Gate, tile: (c.0 - 1, c.1 - 3), turned: false, editor: false }]);
    assert!(!world.map.blocks_unit(c.0, c.1 - 3), "the door must be open for units");
    assert!(world.map.is_blocked(c.0, c.1 - 3), "the door must be closed for zombies");
    assert!(world.map.blocks_unit(c.0 - 1, c.1 - 3), "the posts are closed");
    for b in &mut world.buildings {
        b.hp = 1_000_000;
    }
    let inside = vec2(c.0 as f32 + 0.5, c.1 as f32 + 0.5);
    world.spawn_unit(UnitKind::Soldier, inside + vec2(0.0, -6.0));
    let id = world.units[0].id;
    world.step(&[Order::Command { units: vec![id], command: Command::Move(inside), queue: false, formation: None }]);
    for _ in 0..200 {
        world.step(&[]);
    }
    let i = world.find_unit(id).unwrap();
    assert!(world.units[i].pos.distance(inside) < 0.3, "didn't get in, stands at {:?}", world.units[i].pos);

    world.units[i].cooldown = u32::MAX;
    world.step(&[Order::SpawnZombiesAt { count: 40, point: inside + vec2(0.0, -7.0) }]);
    let ring = |p: Vec2| (p.x.floor() as i32 - c.0).abs() < 3 && (p.y.floor() as i32 - c.1).abs() < 3;
    for _ in 0..400 {
        world.step(&[]);
        assert!(world.zombies.iter().all(|z| !ring(z.pos)), "a zombie got through the gate");
    }
    assert!(world.buildings.iter().any(|b| b.kind == BuildingKind::Gate && b.hp < 1_000_000), "nobody hit the gate");
}

/// Five soldiers into one tower: four fit in, the fifth stays outside. Inside they can't be hit – the zombies
/// hit the tower. "Unload" brings them out next to it; a fallen tower lets them down too.
#[test]
fn tower_holds_four_and_protects_them() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let t = (world.base.x as i32 + 8, world.base.y as i32 - 8);
    clear_ground(&mut world, t, 8);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tower, tile: t, turned: false, editor: false }]);
    let tower = world.buildings.last().unwrap().id;
    for k in 0..5 {
        world.spawn_unit(UnitKind::Soldier, vec2(t.0 as f32 - 3.5 + k as f32, t.1 as f32 + 4.5));
    }
    let ids: Vec<u32> = world.units.iter().map(|u| u.id).collect();
    world.step(&[Order::Command { units: ids.clone(), command: Command::Enter(tower), queue: false, formation: None }]);
    for _ in 0..200 {
        world.step(&[]);
    }
    assert_eq!(world.garrison(tower).len(), data::get().buildings.tower_slots);
    let outside: Vec<&Unit> = world.units.iter().filter(|u| !u.is_inside()).collect();
    assert_eq!(outside.len(), 1);
    assert_eq!(outside[0].order_name(), "Idle", "the fifth should give up, the tower is full");

    // Zombies right at the tower: the tower takes the hits, nobody inside.
    world.units.retain(|u| u.is_inside());
    for u in &mut world.units {
        u.cooldown = u32::MAX;
    }
    let b = world.find_building(tower).unwrap();
    let center = world.buildings[b].center();
    // 10, not 20: runners hit for 10 now (TAB) – 20 of them would bring the tower (600 HP) down within the 5 s.
    world.step(&[Order::SpawnZombiesAt { count: 10, point: center + vec2(1.2, 0.0) }]);
    for _ in 0..100 {
        world.step(&[]);
    }
    assert!(world.units.iter().all(|u| u.hp == u.stats().hp), "a unit in the tower was hit: {:?}", world.units.iter().map(|u| (u.hp, u.inside, u.pos)).collect::<Vec<_>>());
    let b = world.find_building(tower).unwrap();
    assert!(world.buildings[b].hp < world.buildings[b].stats().hp, "the tower wasn't hit");

    world.step(&[Order::ClearZombies, Order::Unload { building: tower }]);
    assert!(world.garrison(tower).is_empty());
    assert!(nobody_on_blocked_ground(&world), "unloaded onto blocked ground");

    // In again, then the tower falls: they're standing where it was.
    let ids: Vec<u32> = world.units.iter().map(|u| u.id).collect();
    world.step(&[Order::Command { units: ids, command: Command::Enter(tower), queue: false, formation: None }]);
    for _ in 0..100 {
        world.step(&[]);
    }
    assert_eq!(world.garrison(tower).len(), data::get().buildings.tower_slots);
    let b = world.find_building(tower).unwrap();
    world.buildings[b].hp = 0;
    world.step(&[]);
    assert!(world.find_building(tower).is_none());
    assert!(world.units.iter().all(|u| !u.is_inside()));
    assert!(nobody_on_blocked_ground(&world));
}

/// A whole group sent to the same point with a queue (no formation): everyone works through every point.
/// Before, only the first one reached each point – the others pressed around it and never counted as arrived,
/// because the first one walked on at once instead of standing still.
#[test]
fn a_group_works_through_queued_points() {
    let (mut world, _) = quiet_world();
    let ids: Vec<u32> = world.units.iter().map(|u| u.id).collect();
    let points = [world.base + vec2(8.0, 8.0), world.base + vec2(-6.0, 9.0), world.base + vec2(-8.0, -6.0)];
    let orders: Vec<Order> = points
        .iter()
        .enumerate()
        .map(|(k, &p)| Order::Command { units: ids.clone(), command: Command::Move(p), queue: k > 0, formation: None })
        .collect();
    world.step(&orders);
    for _ in 0..500 {
        world.step(&[]);
    }
    let waiting: Vec<usize> = world.units.iter().map(|u| u.queue.len()).collect();
    assert!(waiting.iter().all(|&n| n == 0), "commands left per unit: {waiting:?}");
    // And they stand as a crowd around the last point, not spread over the way.
    for unit in &world.units {
        assert!(unit.pos.distance(points[2]) < 3.0, "too far from the last point: {:?}", unit.pos);
    }
}

/// 200 zombies in front of a gap one tile wide: they pile up in front of it instead of sliding into each other.
#[test]
fn zombies_pile_up_instead_of_merging() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let field: Vec<(i32, i32)> = (by - 14..=by + 14).flat_map(|y| (bx + 3..bx + 30).map(move |x| (x, y))).collect();
    let wall: Vec<(i32, i32)> = (by - 10..=by + 10).filter(|&y| y != by).map(|y| (bx + 12, y)).collect();
    world.step(&[Order::PaintTerrain { terrain: Terrain::Grass, tiles: field }]);
    world.step(&[
        Order::PaintTerrain { terrain: Terrain::Mountain, tiles: wall },
        // 300, not 200: with 200 the pile came out at 1.9997 tiles, right at the limit, with 250 at 0.617 of 0.625 once
        // more roamers had drawn other random numbers (2026-10-22) – it depends on where exactly they were scattered.
        Order::SpawnZombiesAt { count: 300, point: world.base + vec2(22.0, 0.5) },
    ]);
    for _ in 0..150 {
        world.step(&[]);
    }
    let zombies: Vec<Vec2> = world.zombies.iter().map(|z| z.pos).collect();
    // Pairs that stand more than half inside each other (closer than one radius – all of them are runners).
    // Before: over 100.
    let radius = Breed::Runner.stats().radius;
    let mut merged = 0;
    for (i, p) in zombies.iter().enumerate() {
        merged += zombies[i + 1..].iter().filter(|q| p.distance(**q) < radius).count();
    }
    assert!(merged <= 5, "{merged} pairs of zombies inside each other");
    // The pile in front of the gap is wide – before, they squeezed together on the line to the gap. Measured in
    // zombie sizes: half as big (2026-10-20), they need a quarter of the room and more of them get through the gap at
    // once – the pile came out at 0.69 tiles instead of over 2.
    let in_front = |p: &&Vec2| p.x > world.base.x + 12.0 && p.x < world.base.x + 16.0;
    let widest = zombies.iter().filter(in_front).map(|p| (p.y - world.base.y).abs()).fold(0.0, f32::max);
    assert!(widest > 5.0 * radius, "pile only {widest} tiles to each side");
}

/// Walkers are slower and weaker than runners, and the spawners send runners.
#[test]
fn walkers_are_slow_and_spawners_send_runners() {
    assert!(Breed::Walker.stats().speed < Breed::Runner.stats().speed);
    assert!(Breed::Walker.stats().hp < Breed::Runner.stats().hp);
    assert!(Breed::Walker.stats().damage < Breed::Runner.stats().damage);
    let (mut world, _) = quiet_world();
    world.step(&[Order::SpawnZombies { count: 30 }]);
    assert_eq!(world.zombies.len(), 30);
    assert!(world.zombies.iter().all(|z| z.breed == Breed::Runner));
}

/// A new round: zombies roam the map, none right at the base, more further out – in packs, never more than `pack`
/// allows on one tile. Near the base only walkers, further out the other breeds too – each from where `mix` in
/// `data\roaming.json` says, and none closer than the difficulty's `keep_out` for its breed. Asleep, except near your
/// eyes (`sleep.rs`); the awake ones all wander.
#[test]
fn roamers_are_scattered_by_distance() {
    let world = World::new(3);
    let roaming = &data::get().roaming;
    let all: Vec<(Vec2, Breed)> =
        world.zombies.iter().map(|z| (z.pos, z.breed)).chain(world.sleep.iter().map(|s| (s.pos, s.breed))).collect();
    assert!(all.len() > 500, "only {} zombies", all.len());
    assert!(world.zombies.iter().all(|z| z.wander.is_some()));
    assert!(!world.zombies.is_empty() && world.sleep.len() > world.zombies.len(), "near the base awake, far out asleep");
    let mut per_tile = std::collections::HashMap::new();
    for (pos, _) in &all {
        *per_tile.entry((pos.x.floor() as i32, pos.y.floor() as i32)).or_insert(0) += 1;
    }
    let most = per_tile.values().copied().max().unwrap_or(0);
    assert!(most <= roaming.pack[1], "{most} zombies on one tile");
    let play = world.map.play_area();
    let half = (play.max.x - play.min.x) / 2.0 * std::f32::consts::FRAC_1_SQRT_2;
    let out = |p: Vec2| (p.distance(world.base) / half).min(1.0);
    for (p, breed) in &all {
        assert!(p.distance(world.base) >= world.keep_out.of(*breed), "a {breed:?} too close to the base: {}", p.distance(world.base));
    }
    let near = all.iter().filter(|(p, _)| out(*p) < 0.25).count();
    let far = all.len() - near;
    assert!(near > 0 && far > near * 4, "near {near}, far {far}");
    for (pos, breed) in &all {
        if let Some(mix) = roaming.mix.iter().find(|m| m.breed == *breed) {
            assert!(out(*pos) >= mix.from, "a {breed:?} too close: {}", out(*pos));
        }
    }
    for breed in Breed::ALL {
        assert!(all.iter().any(|(_, b)| *b == breed), "no {breed:?} roams");
    }
}

/// A unit that dies lies there for 2–5 s, then gets up as a walker where it fell.
#[test]
fn dead_units_rise_as_walkers() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(10.5, -10.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 3);
    world.spawn_unit(UnitKind::Soldier, p);
    world.units[0].hp = 0;
    world.step(&[]);
    assert!(world.units.is_empty());
    assert_eq!(world.corpses.len(), 1);
    let mut rose_after = None;
    for ticks in 1..=120 {
        world.step(&[]);
        if !world.zombies.is_empty() {
            rose_after = Some(ticks);
            break;
        }
    }
    let ticks = rose_after.expect("never got up");
    assert!((40..=100).contains(&ticks), "got up after {ticks} ticks");
    assert!(world.corpses.is_empty());
    assert_eq!(world.zombies[0].breed, Breed::Walker);
    assert!(world.zombies[0].pos.distance(p) < 1.0);
}

/// Zombies hit a tent's shield first. When it breaks, the tent is infected: exactly its colonists come out as
/// zombies – 2 to 4 runners, the rest walkers – and only after that does it lose hit points.
#[test]
fn a_tent_whose_shield_breaks_lets_out_its_colonists() {
    let mut runners_seen = Vec::new();
    for seed in 0..12 {
        let mut world = World::new(seed);
        world.step(&[Order::ClearZombies]);
        world.units.clear();
        let t = (world.base.x as i32 + 8, world.base.y as i32 - 8);
        clear_ground(&mut world, t, 6);
        world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tent, tile: t, turned: false, editor: false }]);
        let b = world.buildings.len() - 1;
        world.buildings[b].shield = 7; // two hits of a runner
        world.step(&[Order::SpawnZombiesAt { count: 1, point: vec2(t.0 as f32 - 0.6, t.1 as f32 + 1.0) }]);
        while world.zombies.len() == 1 && world.tick < 400 {
            world.step(&[]);
            let tent = &world.buildings[b];
            if !tent.infected {
                assert_eq!(tent.hp, tent.stats().hp, "hit points down before the shield broke");
            }
        }
        let tent = &world.buildings[b];
        assert!(tent.infected, "seed {seed}: the shield never broke");
        assert_eq!(world.zombies.len(), 1 + tent.stats().colonists as usize, "seed {seed}");
        let runners = world.zombies[1..].iter().filter(|z| z.breed == Breed::Runner).count();
        assert!((2..=4).contains(&runners), "seed {seed}: {runners} runners");
        assert!(world.zombies[1..].iter().all(|z| z.pos.distance(tent.center()) < 3.0), "came out far from the tent");
        runners_seen.push(runners);
    }
    runners_seen.dedup();
    assert!(runners_seen.len() > 1, "always the same number of runners");
}

/// An infected tent belongs to the zombies: the ones that came out of it, standing right next to it, don't hit it –
/// they go for the command center. And it's no longer something to tear down on their way: it's open.
#[test]
fn zombies_leave_an_infected_tent_alone() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let t = (world.base.x as i32 + 8, world.base.y as i32 - 8);
    clear_ground(&mut world, t, 6);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tent, tile: t, turned: false, editor: false }]);
    let b = world.buildings.len() - 1;
    let shield = world.buildings[b].shield;
    assert!(world.buildings[b].take_damage(shield));
    world.infect(b);
    assert_eq!(world.zombies.len(), world.buildings[b].stats().colonists as usize);
    assert!(!world.map.is_blocked(t.0, t.1) && !world.map.can_tear_down(t.0, t.1));
    for _ in 0..200 {
        world.step(&[]);
    }
    let tent = &world.buildings[b];
    assert_eq!(tent.hp, tent.stats().hp, "the zombies hit their own tent");
}

/// Every payout a fed tent pays its `gold` (`data\buildings.json`) – on top of the command center's. An infected tent
/// pays nothing.
#[test]
fn tents_pay_gold_every_payout() {
    let (mut world, _) = quiet_world();
    world.units.clear(); // their upkeep would come off the gold too
    let t = (world.base.x as i32 + 8, world.base.y as i32 - 8);
    clear_ground(&mut world, t, 6);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tent, tile: t, turned: false, editor: false }]);
    let (tent, center) = (BuildingKind::Tent.stats().gold, BuildingKind::CommandCenter.stats().gold);
    assert!(tent > 0);
    let start = world.gold;
    // In any 2 payouts' time the clock passes exactly two payouts.
    for _ in 0..2 * payout_ticks() {
        world.step(&[]);
    }
    assert_eq!(world.gold - start, 2 * (tent + center));
    assert_eq!(world.income(), tent + center, "the HUD shows what was paid at the last payout");

    let b = world.buildings.len() - 1;
    let shield = world.buildings[b].shield;
    assert!(world.buildings[b].take_damage(shield), "breaking the shield doesn't infect");
    assert_eq!(world.income(), center);
    let start = world.gold;
    for _ in 0..2 * payout_ticks() {
        world.step(&[]);
    }
    assert_eq!(world.gold - start, 2 * center, "an infected tent still pays");
}

/// A tent next to the base whose shield just broke: infected, its zombies out and cleared away again. Returns the
/// tent's index and its top left tile.
fn infected_tent(world: &mut World) -> (usize, (i32, i32)) {
    let t = (world.base.x as i32 + 8, world.base.y as i32 - 8);
    clear_ground(world, t, 6);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tent, tile: t, turned: false, editor: false }]);
    let b = world.buildings.len() - 1;
    let shield = world.buildings[b].shield;
    assert!(world.buildings[b].take_damage(shield));
    world.infect(b);
    world.step(&[Order::ClearZombies]);
    (b, t)
}

/// The walls of an infected building are broken: zombies and units walk right through it – a unit sent into its middle
/// gets there.
#[test]
fn units_and_zombies_walk_through_an_infected_tent() {
    let (mut world, id) = quiet_world();
    let (b, _) = infected_tent(&mut world);
    for (x, y) in world.buildings[b].tiles() {
        assert!(!world.map.is_blocked(x, y) && !world.map.blocks_unit(x, y), "tile {x}, {y} still closed");
    }
    let middle = world.buildings[b].center();
    world.step(&[Order::Command { units: vec![id], command: Command::Move(middle), queue: false, formation: None }]);
    for _ in 0..600 {
        world.step(&[]);
    }
    let u = world.find_unit(id).unwrap();
    assert!(world.units[u].pos.distance(middle) < 0.5, "stopped at {:?}, not in the tent", world.units[u].pos);
}

/// A repair costs the share of `repair_cost` that's missing (half the hit points: half the price), paid at once – and
/// that share of `repair_seconds`. Without enough gold nothing happens.
#[test]
fn a_repair_costs_its_share_and_takes_its_time() {
    let (mut world, _) = quiet_world();
    let id = barracks(&mut world);
    let b = world.find_building(id).unwrap();
    let (full, cost) = (world.buildings[b].stats().hp, world.buildings[b].stats().repair_cost);
    world.buildings[b].hp = full / 2;
    let price = world.buildings[b].repair_price();
    assert_eq!(price, cost.div_ceil(2));

    world.gold = price - 1;
    world.step(&[Order::Repair { buildings: vec![id] }]);
    assert_eq!((world.gold, world.buildings[b].repair_left), (price - 1, 0), "repaired without enough gold");

    world.gold = price;
    world.step(&[Order::Repair { buildings: vec![id] }]);
    assert_eq!(world.gold, 0);
    assert_eq!(world.buildings[b].repair_price(), 0, "the same damage can be paid twice");
    let mut ticks = 1;
    while world.buildings[b].hp < full && ticks < 1000 {
        world.step(&[]);
        ticks += 1;
    }
    let half = (data::get().buildings.repair_seconds * TICKS_PER_SECOND / 2.0) as i32;
    assert!((ticks - half).abs() <= 1, "took {ticks} ticks, half a full repair is {half}");
    assert_eq!(world.buildings[b].repair_left, 0);
}

/// Repairing an infected tent cures it once it's whole: closed again (whoever stood in it is pushed out), something to
/// tear down again, and it pays gold again.
#[test]
fn a_repair_cures_an_infected_tent() {
    let (mut world, _) = quiet_world();
    let (b, t) = infected_tent(&mut world);
    let middle = world.buildings[b].center();
    let inside = bait(&mut world, UnitKind::Soldier, middle);
    assert_eq!(world.buildings[b].full_income(), 0);
    let price = world.buildings[b].repair_price();
    assert!(price > 0 && price <= world.buildings[b].stats().repair_cost);
    world.gold = price;
    let id = world.buildings[b].id;
    world.step(&[Order::Repair { buildings: vec![id] }]);
    for _ in 0..(data::get().buildings.repair_seconds * TICKS_PER_SECOND) as u32 + 5 {
        world.step(&[]);
    }
    let tent = &world.buildings[b];
    assert!(!tent.infected, "still infected");
    assert_eq!((tent.hp, tent.shield), (tent.stats().hp, tent.stats().max_shield()));
    assert!(world.map.is_blocked(t.0, t.1) && world.map.can_tear_down(t.0, t.1));
    one_payout(&mut world);
    assert!(world.buildings[b].paid > 0, "the cured tent pays no gold");
    let u = world.find_unit(inside).unwrap();
    let pos = world.units[u].pos;
    assert!(!world.map.blocks_unit(pos.x.floor() as i32, pos.y.floor() as i32), "a unit stuck in the cured tent");
}

/// No repair with a zombie in the building's build zone (the same zone as for building it) – once it's gone, it works.
#[test]
fn no_repair_with_a_zombie_nearby() {
    let (mut world, _) = quiet_world();
    let id = barracks(&mut world);
    let b = world.find_building(id).unwrap();
    world.buildings[b].hp /= 2;
    world.gold = 1000;
    let near = world.buildings[b].center() + vec2(crate::placement::build_clearance() as f32, 0.0);
    world.spawn_zombie_as(near, Breed::Walker, true);
    assert!(!world.can_repair(b));
    let plan = world.repair_plan([b]);
    assert_eq!((plan.chosen.len(), plan.damaged, plan.blocked), (0, 1, 1));
    world.step(&[Order::Repair { buildings: vec![id] }]);
    assert_eq!((world.gold, world.buildings[b].repair_left), (1000, 0), "repaired with a zombie next to it");

    world.step(&[Order::ClearZombies]);
    assert!(world.can_repair(b));
    world.step(&[Order::Repair { buildings: vec![id] }]);
    assert!(world.gold < 1000 && world.buildings[b].repair_left > 0);
}

/// "Repair all" with too little gold: the cheapest first, so as many buildings as possible get repaired.
#[test]
fn repair_all_takes_the_cheapest_first() {
    let (mut world, _) = quiet_world();
    let id = barracks(&mut world);
    let barracks = world.find_building(id).unwrap();
    let cc = world.buildings.iter().position(|b| b.kind == BuildingKind::CommandCenter).unwrap();
    world.buildings[barracks].hp /= 2;
    world.buildings[cc].hp /= 2;
    let (small, big) = (world.buildings[barracks].repair_price(), world.buildings[cc].repair_price());
    assert!(small < big);
    world.gold = big - 1;
    let plan = world.repair_plan(0..world.buildings.len());
    assert_eq!(plan.chosen, vec![barracks]);
    assert_eq!((plan.price, plan.damaged, plan.blocked), (small + big, 2, 0));
}

/// How many shots a unit needs for a zombie – as decided on 2026-10-17 (soldier with TAB's 15 damage, so 3 for a
/// walker and 4 for a runner). If a value in `data\` changes, this test says whether they still hold.
#[test]
fn shots_to_kill_walkers_and_runners() {
    let shots = |kind: UnitKind, breed: Breed| {
        let damage = kind.stats().damage.saturating_sub(breed.stats().armor).max(1);
        breed.stats().hp.div_ceil(damage)
    };
    use UnitKind::*;
    assert_eq!([shots(Ranger, Breed::Walker), shots(Soldier, Breed::Walker), shots(Sniper, Breed::Walker)], [4, 3, 1]);
    assert_eq!([shots(Ranger, Breed::Runner), shots(Soldier, Breed::Runner), shots(Sniper, Breed::Runner)], [6, 4, 1]);
}

/// A ranger with its back to a zombie: first it turns around, then it draws the bow – only then the first arrow.
/// After that one arrow every `aim + cooldown` ticks.
#[test]
fn units_turn_and_aim_before_they_shoot() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(8.5, -8.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 6);
    world.spawn_unit(UnitKind::Ranger, p);
    // Right behind it (it faces +x +y), standing still and too tough to die.
    world.spawn_zombie_as(p - vec2(3.0, 3.0), Breed::Walker, false);
    world.zombies[0].speed = 0.0;
    world.zombies[0].hp = 100_000;
    let stats = UnitKind::Ranger.stats();
    let mut shots = Vec::new();
    for tick in 1..=80u32 {
        world.step(&[]);
        if !world.shots.is_empty() {
            shots.push(tick);
        }
    }
    // The tick it finishes turning already counts as the first tick of aiming.
    let first = stats.turn_around + stats.aim;
    assert_eq!(shots.first(), Some(&first), "turn {} + aim {} ticks: {shots:?}", stats.turn_around, stats.aim);
    assert!(shots.windows(2).all(|w| w[1] - w[0] == stats.shot_ticks()), "not every {} ticks: {shots:?}", stats.shot_ticks());
}

/// Fat zombies are big: they keep further apart from each other than runners – and they're the toughest.
#[test]
fn fat_zombies_are_big_and_tough() {
    let (fat, runner) = (Breed::Fat.stats(), Breed::Runner.stats());
    assert!(fat.radius > runner.radius && fat.hp > runner.hp && fat.armor > runner.armor);
    let (mut world, _) = quiet_world();
    world.units.clear();
    let home = world.base + vec2(12.0, -12.0);
    clear_ground(&mut world, (home.x as i32, home.y as i32), 8);
    world.step(&[Order::PlaceZombies { center: home, radius: 1.0, count: 12, breed: Breed::Fat, wander: true }]);
    for _ in 0..60 {
        world.step(&[]);
    }
    let zombies: Vec<Vec2> = world.zombies.iter().map(|z| z.pos).collect();
    // No two fat ones more than half inside each other – with runners' bodies they would stand much closer.
    let mut merged = 0;
    for (i, p) in zombies.iter().enumerate() {
        merged += zombies[i + 1..].iter().filter(|q| p.distance(**q) < fat.radius * 1.5).count();
    }
    assert!(merged <= 1, "{merged} pairs of fat zombies inside each other");
}

/// A unit is paid when it goes into the queue – without enough gold it doesn't go in – and cancelling pays it back in
/// full (like StarCraft).
#[test]
fn training_costs_gold_and_cancelling_pays_it_back() {
    let (mut world, _) = quiet_world();
    let cc = barracks(&mut world);
    let cost = UnitKind::Soldier.stats().cost;
    world.gold = cost * 2 + cost / 2;
    let train = Order::Train { building: cc, kind: UnitKind::Soldier };
    world.apply_orders(&[train.clone(), train.clone(), train]);
    assert_eq!(world.buildings[world.find_building(cc).unwrap()].production.len(), 2, "the third one wasn't paid for");
    assert_eq!(world.gold, cost / 2);
    world.apply_orders(&[Order::CancelTrain { building: cc, index: 1 }]);
    assert_eq!(world.gold, cost + cost / 2);
    assert_eq!(world.score.gold_spent, cost);
}

/// A building costs its price: of a dragged line only as many pieces as the gold pays for. The editor builds for free.
#[test]
fn buildings_cost_gold_and_a_line_stops_when_it_runs_out() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    clear_ground(&mut world, (bx + 10, by - 8), 6);
    world.gold = BuildingKind::Wall.stats().cost * 3;
    let line: Vec<Order> =
        (0..5).map(|k| Order::PlaceBuilding { kind: BuildingKind::Wall, tile: (bx + 8 + k, by - 6), turned: false, editor: false }).collect();
    world.step(&line);
    assert_eq!(world.buildings.iter().filter(|b| b.kind == BuildingKind::Wall).count(), 3);
    assert_eq!(world.gold, 0);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tent, tile: (bx + 8, by - 11), turned: false, editor: true }]);
    assert_eq!(world.buildings.iter().filter(|b| b.kind == BuildingKind::Tent).count(), 1, "the editor's tent isn't there");
    assert_eq!(world.gold, 0, "the editor's tent cost something");
}

/// Lost: when the last command center falls – not before. The editor's eraser taking it away loses nothing (a map
/// without a command center can't be lost that way).
#[test]
fn losing_the_last_command_center_loses_the_round() {
    let (mut world, _) = quiet_world();
    world.step(&[]);
    assert_eq!(world.outcome, None);
    world.buildings[0].hp = 0;
    world.step(&[]);
    assert!(matches!(world.outcome, Some(Outcome::Lost { .. })), "{:?}", world.outcome);

    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    world.step(&[Order::RemoveAt { tiles: vec![(bx, by)] }]);
    assert!(world.buildings.iter().all(|b| b.kind != BuildingKind::CommandCenter));
    world.step(&[]);
    assert_eq!(world.outcome, None);
}

/// Won: once the final wave has come and its last zombie is dead. The final wave calls every zombie on the map – they
/// belong to it then, and have to die too. Zombies coming later (the constant spawn) don't matter.
#[test]
fn killing_the_final_wave_wins_the_round() {
    let (mut world, _) = quiet_world();
    world.waves.clear();
    let at = world.tick + 5;
    world.step(&[
        Order::AddTimedWave(TimedWave { at, count: 3, from: WaveFrom::Sides(1), breed: Breed::Runner, final_wave: true, reveal: false }),
        Order::SpawnWave { count: 10, from: WaveFrom::Sides(1) },
    ]);
    while world.tick <= at {
        assert_eq!(world.outcome, None, "won before the final wave came");
        world.step(&[]);
    }
    assert_eq!(world.zombies.iter().filter(|z| z.final_wave).count(), 13, "the zombies on the map weren't called");
    world.step(&[Order::SpawnWave { count: 5, from: WaveFrom::Sides(1) }]);
    assert_eq!(world.outcome, None, "won with the final wave alive");
    for zombie in world.zombies.iter_mut().filter(|z| z.final_wave) {
        zombie.hp = 0;
    }
    world.step(&[]);
    assert!(matches!(world.outcome, Some(Outcome::Won { .. })), "{:?}", world.outcome);
    assert_eq!(world.zombies.len(), 5, "the later zombies are gone too");
    assert_eq!(world.score.zombies_killed, 13);
}

/// A new round plans its waves from its days and how often its duration says they come (`data\game_durations.json`):
/// the mini round (10 days, from day 4 every 3) gets two waves (day 4 and 7) and the final wave on day 10 from all
/// sides, with every breed – it's final and shows the map. 40 days (from day 5 every 4) get nine, growing: the first
/// only runners, then more and more riots, infested, spitters, fat ones. A difficulty with 200 % waves doubles every
/// count.
#[test]
fn waves_are_planned_from_the_days() {
    let (ten, forty) = (data::get().duration(10), data::get().duration(40));
    assert_eq!((ten.first_wave_day, ten.wave_every_days, forty.first_wave_day, forty.wave_every_days), (4, 3, 5, 4), "game_durations.json changed – change this test too");
    let mini = plan_waves(10, ten, 1.0);
    let runners: Vec<&TimedWave> = mini.iter().filter(|w| !w.final_wave && w.breed == Breed::Runner).collect();
    let days: Vec<u64> = runners.iter().map(|w| day_and_hour(w.at).0).collect();
    assert_eq!(days, vec![4, 7]);
    assert!(mini.iter().filter(|w| w.final_wave).all(|w| day_and_hour(w.at).0 == 10 && w.from == WaveFrom::Sides(4) && w.reveal));
    assert!(mini.iter().all(|w| w.final_wave == w.reveal), "a wave that isn't final shows the map");
    for breed in Breed::ALL.into_iter().filter(|&b| b != Breed::Walker) {
        assert!(mini.iter().any(|w| w.final_wave && w.breed == breed), "no {breed:?} in the final wave");
    }

    let long = plan_waves(40, forty, 1.0);
    let at = |w: &&TimedWave| w.at;
    let runners: Vec<&TimedWave> = long.iter().filter(|w| !w.final_wave && w.breed == Breed::Runner).collect();
    assert_eq!(runners.iter().map(|w| day_and_hour(w.at).0).collect::<Vec<_>>(), vec![5, 9, 13, 17, 21, 25, 29, 33, 37]);
    assert!(runners.windows(2).all(|pair| pair[0].at < pair[1].at && pair[0].count <= pair[1].count), "not growing");
    // The first wave only runners, the last before the final wave every special breed.
    let first = runners.first().map(at).unwrap();
    let last = runners.last().map(at).unwrap();
    assert!(long.iter().filter(|w| w.at == first).all(|w| w.breed == Breed::Runner), "specials in the first wave");
    for special in &data::get().spawning.waves.specials {
        assert!(long.iter().any(|w| w.at == last && w.breed == special.breed), "no {:?} in the last wave", special.breed);
    }
    let riots: Vec<u32> = long.iter().filter(|w| !w.final_wave && w.breed == Breed::Armored).map(|w| w.count).collect();
    assert!(riots.len() > 3 && riots.windows(2).all(|pair| pair[0] <= pair[1]) && riots[0] < riots[riots.len() - 1], "riots don't grow: {riots:?}");
    assert!(plan_waves(0, ten, 1.0).is_empty());
    assert_eq!(plan_waves(10, ten, 2.0)[0].count, mini[0].count * 2);

    let world = World::new(3);
    assert_eq!(world.days, 10);
    assert_eq!(world.waves, mini, "a new round doesn't get the planned waves");
}

/// Changing the days (editor) moves the final wave along to the new last day, at the same hour – the other waves stay.
#[test]
fn changing_the_days_moves_the_final_wave() {
    let (mut world, _) = quiet_world();
    let before = world.waves.clone();
    world.step(&[Order::SetDays { days: 20 }]);
    assert_eq!(world.days, 20);
    for (old, new) in before.iter().zip(&world.waves) {
        if old.final_wave {
            assert_eq!(day_and_hour(new.at), (20, day_and_hour(old.at).1));
        } else {
            assert_eq!(old, new);
        }
    }
    assert!(world.waves.iter().any(|w| w.final_wave), "the final wave is gone");
}

/// The editor's eraser takes everything off its tiles: units, zombies – and a building even if it only stands on one
/// of them (the command center by one corner).
#[test]
fn the_eraser_takes_everything_off_its_tiles() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    // The start units stand in 3 rows below the command center, on the tiles bx - 2 … bx + 3 and from `first` on
    // (by + 3 for a 3 × 3 one).
    let (w, h) = BuildingKind::CommandCenter.footprint(false);
    let first = by + (h + 3) / 2;
    let center = vec2(bx as f32 + 2.5, (first + 1) as f32 + 0.5);
    world.apply_orders(&[Order::PlaceZombies { center, radius: 0.4, count: 3, breed: Breed::Walker, wander: true }]);
    let mut tiles = vec![(bx + w / 2, by + h / 2)];
    for y in first..=first + 2 {
        for x in bx - 1..=bx + 3 {
            tiles.push((x, y));
        }
    }
    world.apply_orders(&[Order::RemoveAt { tiles }]);
    assert!(world.buildings.is_empty(), "the command center is still there");
    assert!(!world.map.is_built(bx, by), "its tiles are still taken");
    assert!(world.zombies.is_empty());
    assert!(world.units.iter().all(|u| (u.pos.x.floor() as i32) == bx - 2), "a unit under the eraser is still there");
    assert_eq!(world.units.len(), 3);
}

/// The constant spawn belongs to the map: the editor's orders change it, and its stages stay in order of time.
#[test]
fn the_constant_spawn_can_be_edited() {
    let (mut world, _) = quiet_world();
    let stage = data::Stage { day: 1, hour: 2, breed: Breed::Fat, every_seconds: 5.0, group: [2, 2] };
    world.apply_orders(&[Order::AddStage(stage)]);
    assert_eq!(world.constant.stages[0], stage, "not sorted to the front");
    assert_eq!(world.spawn_group(), None, "before 2:00 of day 1 nothing should come");
    world.tick = round::tick_of(1, 2);
    assert_eq!(world.spawn_group().map(|g| g.breed), Some(Breed::Fat));
    world.apply_orders(&[Order::RemoveStage { index: 0 }, Order::SetExtraPerDay { extra: 3.0 }]);
    assert_eq!(world.constant.extra_per_day, 3.0);
    assert_eq!(world.constant.stages, data::get().spawning.constant.stages);
}

/// Overkill prevention: two snipers side by side, two walkers in front of them. One shot kills a walker – so the second
/// sniper takes the other walker instead of the same one (before 2026-10-17 both aimed at the nearer one).
#[test]
fn a_crowd_spreads_its_fire() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(6.5, 6.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 7);
    let walker = |at: Vec2| Order::PlaceZombies { center: at, radius: 0.0, count: 1, breed: Breed::Walker, wander: true };
    world.step(&[
        Order::SpawnUnit { kind: UnitKind::Sniper, point: p },
        Order::SpawnUnit { kind: UnitKind::Sniper, point: p + vec2(0.0, 0.8) },
        walker(p + vec2(5.0, 0.3)),
        walker(p + vec2(5.5, 1.5)),
    ]);
    world.step(&[]);
    let (a, b) = (world.units[0].aim_at, world.units[1].aim_at);
    assert!(a.is_some() && b.is_some(), "not both aiming: {}, {}", a.is_some(), b.is_some());
    assert!(a != b, "both snipers aim at the same walker");
}

/// Not a real test – a tool for balancing the mini round (DESIGN.md 13, step 4): plays it at full size, once with a
/// player who does nothing, once with a very simple one (every hour: a tent while there are fewer than 12, then a
/// barracks, then soldiers – all of them simply stand at the base). Prints every half day how it's going and how the round ends – up
/// to two days after the last one: the final wave can take a while to arrive, through narrow passes one by one.
/// In the fast build, it takes a minute or two:
/// `cargo test --release balance_mini_round -- --ignored --nocapture`
/// A new round starts with what its difficulty says (`data\game_difficulties.json`) – units, gold and wood, nothing
/// else in store. Every difficulty, every house of a village of doom exactly as full as it says.
#[test]
fn a_new_round_starts_with_the_difficulty_values() {
    for difficulty in &data::get().difficulties.difficulties {
        let mut setup = MapSetup::default();
        setup.common.size = 128.0;
        setup.round.difficulty = difficulty.name.clone();
        let world = World::generate(3, &setup);
        let count = |kind: UnitKind| world.units.iter().filter(|u| u.kind == kind).count();
        let name = &difficulty.name;
        assert_eq!(count(UnitKind::Ranger), difficulty.start_rangers, "{name}");
        assert_eq!(count(UnitKind::Soldier), difficulty.start_soldiers, "{name}");
        assert_eq!(count(UnitKind::Sniper), difficulty.start_snipers, "{name}");
        assert_eq!((world.gold, world.stock.get(Resource::Wood)), (difficulty.start_gold, difficulty.start_wood), "{name}");
        assert_eq!(world.stock.get(Resource::Food), 0);
        assert_eq!((&world.difficulty, world.keep_out), (name, difficulty.keep_out));
        assert!(!world.doom.is_empty() && world.doom.iter().all(|d| d.inside == difficulty.doom_zombies), "{name}: houses not as full as it says");
    }
}

/// A harder difficulty: more zombies roaming, fuller houses, stronger waves, a bigger constant spawn – and the
/// special breeds closer to the base. The same map either way.
#[test]
fn a_harder_difficulty_brings_more() {
    let make = |name: &str| {
        let mut setup = MapSetup::default();
        setup.common.size = 128.0;
        setup.round.difficulty = name.to_owned();
        World::generate(3, &setup)
    };
    let (easy, brutal) = (make("Easy"), make("Brutal"));
    assert!(easy.map.same_terrain(&brutal.map), "the difficulty changed the map");
    assert!(brutal.doom.len() > easy.doom.len(), "{} vs. {} houses of doom", brutal.doom.len(), easy.doom.len());
    let zombies = |w: &World| w.zombies.len() + w.sleep.len();
    assert!(zombies(&brutal) > zombies(&easy) * 3 / 2, "{} vs. {} roaming", zombies(&brutal), zombies(&easy));
    let inside = |w: &World| w.doom.iter().map(|d| d.inside).sum::<u32>();
    assert!(inside(&brutal) > inside(&easy) * 2, "the houses aren't fuller");
    let wave = |w: &World| w.waves.iter().map(|t| t.count).sum::<u32>();
    assert!(wave(&brutal) > wave(&easy) * 2, "the waves aren't stronger");
    let group = |w: &World| w.constant.stages.iter().map(|s| s.group[1]).sum::<u32>();
    assert!(group(&brutal) > group(&easy), "the constant spawn isn't bigger");
    // The closest fat one: much closer on Brutal (10 tiles may) than on Easy (60).
    let closest = |w: &World, breed: Breed| w.sleep.iter().filter(|s| s.breed == breed).map(|s| s.pos.distance(w.base)).fold(f32::INFINITY, f32::min);
    assert!(closest(&easy, Breed::Fat) >= easy.keep_out.fat, "a fat one too close on Easy");
    assert!(closest(&brutal, Breed::Fat) < closest(&easy, Breed::Fat), "Brutal's fat ones aren't closer");
}

/// A zombie lured into the ring its breed keeps out of (your bad luck) strolls on from there at random: now and then
/// a little closer to the base – at most `inside_step` per trip – but on the whole back out. Nothing is kept per
/// zombie for it: only where it stands counts.
#[test]
fn a_lured_zombie_strolls_on_inside_the_ring_but_not_far_in() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    world.keep_out = KeepOut { walker: 30.0, runner: 30.0, fat: 30.0, armored: 30.0, spitter: 30.0, infested: 30.0 };
    let step = data::get().roaming.inside_step;
    let middle = (world.base.x as i32 + 18, world.base.y as i32);
    clear_ground(&mut world, middle, 16);
    world.step(&[Order::PlaceZombies { center: world.base + vec2(18.0, 0.0), radius: 1.0, count: 20, breed: Breed::Walker, wander: true }]);
    let distances = |w: &World| -> Vec<f32> {
        w.zombies.iter().map(|z| z.pos).chain(w.sleep.iter().map(|s| s.pos)).map(|p| p.distance(w.base)).collect()
    };
    let start = distances(&world);
    let mut closest = f32::INFINITY;
    for _ in 0..(180.0 * TICKS_PER_SECOND) as u32 {
        world.step(&[]);
        closest = distances(&world).into_iter().fold(closest, f32::min);
    }
    let end = distances(&world);
    let mean = |d: &[f32]| d.iter().sum::<f32>() / d.len() as f32;
    let started = start.iter().copied().fold(f32::INFINITY, f32::min);
    assert!(closest < started - 1.0, "none strolled any closer: {closest} (started at {started})");
    assert!(closest > started - 3.0 * step, "one strolled far in: {closest} (started at {started})");
    assert!(mean(&end) > mean(&start) + 3.0, "they don't drift back out: {} -> {}", mean(&start), mean(&end));
}

/// A stroller never strolls closer to the base than its breed may (`keep_out` of the difficulty) – not even on the way
/// to its goal: fat ones placed outside their ring and left alone for two minutes, awake and asleep, stay out of it,
/// the walkers next to them may go closer.
#[test]
fn every_breed_keeps_its_distance_while_strolling() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    world.keep_out = KeepOut { walker: 10.0, runner: 10.0, fat: 30.0, armored: 30.0, spitter: 30.0, infested: 30.0 };
    let p = world.base + vec2(36.0, 0.0);
    let middle = (world.base.x as i32 + 28, world.base.y as i32);
    clear_ground(&mut world, middle, 20);
    world.step(&[
        Order::PlaceZombies { center: p, radius: 2.0, count: 20, breed: Breed::Fat, wander: true },
        Order::PlaceZombies { center: p, radius: 2.0, count: 20, breed: Breed::Walker, wander: true },
    ]);
    let mut closest = [f32::INFINITY; 2];
    for _ in 0..(120.0 * TICKS_PER_SECOND) as u32 {
        world.step(&[]);
        // Far from every eye they fall asleep after 20 s – and stroll on asleep.
        let awake = world.zombies.iter().map(|z| (z.pos, z.breed));
        for (pos, breed) in awake.chain(world.sleep.iter().map(|s| (s.pos, s.breed))) {
            let k = if breed == Breed::Fat { 0 } else { 1 };
            closest[k] = closest[k].min(pos.distance(world.base));
        }
    }
    assert!(closest[0] > 29.0, "a fat one strolled to {} tiles", closest[0]);
    assert!(closest[1] < 29.0, "the walkers didn't stroll closer: {}", closest[1]);
}

/// No zombies coming anymore – no waves, no constant spawn: tests that wait for many payouts wait for days
/// (`payout_ticks`), long enough for the first waves.
fn no_more_zombies(world: &mut World) {
    world.waves.clear();
    for spawner in &mut world.spawners {
        spawner.mode = SpawnMode::Off;
    }
}

/// Steps until the next payout of the economy has passed (`payout_ticks`).
fn one_payout(world: &mut World) {
    world.step(&[]);
    while !world.tick.is_multiple_of(payout_ticks()) {
        world.step(&[]);
    }
}

/// A sawmill brings in its share of the forest every hour: 0.1 wood per tile it works. A second one right next to it
/// only gets the trees the first one doesn't reach – they share, nobody gets them twice.
#[test]
fn a_sawmill_brings_wood_and_two_share_the_forest() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let spot = (bx + 6, by - 9);
    clear_ground(&mut world, spot, 7);
    // A strip of forest above the spot.
    let forest = (spot.0 - 4..spot.0 + 6).flat_map(|x| (spot.1 - 4..spot.1 - 1).map(move |y| (x, y))).collect();
    world.step(&[Order::PaintTerrain { terrain: Terrain::Forest, tiles: forest }]);
    world.stock.set(Resource::Wood, 100);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Sawmill, tile: spot, turned: false, editor: false }]);
    let b = world.buildings.len() - 1;
    assert_eq!(world.buildings[b].kind, BuildingKind::Sawmill, "not built");
    let tiles = world.worked_tiles(BuildingKind::Sawmill, world.buildings[b].tile, b).len();
    assert!(tiles >= 10, "only {tiles} forest tiles");
    world.stock.set(Resource::Wood, 0);
    no_more_zombies(&mut world);
    for _ in 0..10 {
        one_payout(&mut world);
    }
    // 10 hours, 0.1 per tile: one wood per tile.
    assert_eq!(world.stock.get(Resource::Wood), tiles as u32);

    let second = (spot.0 + 2, spot.1);
    let alone = world.worked_tiles(BuildingKind::Sawmill, second, 0).len();
    let shared = world.worked_tiles(BuildingKind::Sawmill, second, world.buildings.len()).len();
    assert!(shared < alone, "the second sawmill gets the first one's trees too ({shared} of {alone})");
}

/// The mine needs a wood workshop first. Then it gets whatever ore lies around it – iron ore, coal and stone.
#[test]
fn a_mine_needs_the_workshop_and_gets_every_ore() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let spot = (bx - 9, by - 9);
    clear_ground(&mut world, spot, 6);
    let place = Order::PlaceBuilding { kind: BuildingKind::Mine, tile: spot, turned: false, editor: false };
    assert!(!world.unlocked(BuildingKind::Mine));
    world.step(std::slice::from_ref(&place));
    assert!(!world.buildings.iter().any(|b| b.kind == BuildingKind::Mine), "built without a wood workshop");

    world.apply_orders(&[
        Order::PlaceBuilding { kind: BuildingKind::WoodWorkshop, tile: (bx + 6, by + 6), turned: false, editor: true },
        Order::PaintOre { ore: Some(Ore::Iron), tiles: vec![(spot.0 - 1, spot.1)] },
        Order::PaintOre { ore: Some(Ore::Coal), tiles: vec![(spot.0 + 2, spot.1)] },
        Order::PaintOre { ore: Some(Ore::Stone), tiles: vec![(spot.0, spot.1 - 1), (spot.0 + 1, spot.1 - 1)] },
    ]);
    assert!(world.unlocked(BuildingKind::Mine));
    world.step(&[place]);
    assert!(world.buildings.iter().any(|b| b.kind == BuildingKind::Mine), "not built with the wood workshop");
    for r in [Resource::Iron, Resource::Coal, Resource::Stone] {
        world.stock.set(r, 0);
    }
    no_more_zombies(&mut world);
    for _ in 0..10 {
        one_payout(&mut world);
    }
    let got = |r: Resource| world.stock.get(r);
    assert_eq!((got(Resource::Iron), got(Resource::Coal), got(Resource::Stone)), (1, 1, 2));
}

/// Like TAB: a mine (2 × 2, `gather_range` 2) in the middle of a big ore patch works 6 × 6 tiles – a square, the
/// corners too, and not one tile further.
#[test]
fn a_mine_works_six_by_six() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let spot = (bx - 9, by - 9);
    clear_ground(&mut world, spot, 6);
    let patch: Vec<(i32, i32)> = (-4..6).flat_map(|dy| (-4..6).map(move |dx| (spot.0 + dx, spot.1 + dy))).collect();
    world.step(&[Order::PaintOre { ore: Some(Ore::Coal), tiles: patch }]);
    let worked = world.worked_tiles(BuildingKind::Mine, spot, world.buildings.len());
    assert_eq!(worked.len(), 36);
    assert!(worked.contains(&(spot.0 - 2, spot.1 - 2)) && worked.contains(&(spot.0 + 3, spot.1 + 3)), "corners missing");
    assert!(!worked.contains(&(spot.0 - 3, spot.1)) && !worked.contains(&(spot.0 + 4, spot.1)), "one tile too far");
}

/// A mine right on the ore digs the tiles under itself too (like Factorio's drill), and the preview counts the same as
/// the mine once it stands. Another building on the ore covers it: no mine gets that tile.
#[test]
fn a_mine_digs_the_ore_under_itself() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let spot = (bx - 9, by - 9);
    clear_ground(&mut world, spot, 6);
    // Iron ore only under the mine's 2 × 2 site – and on one tile beside it, where a wall goes.
    let under: Vec<(i32, i32)> = (0..2).flat_map(|dy| (0..2).map(move |dx| (spot.0 + dx, spot.1 + dy))).collect();
    let beside = (spot.0 + 2, spot.1);
    world.apply_orders(&[
        Order::PlaceBuilding { kind: BuildingKind::WoodWorkshop, tile: (bx + 6, by + 6), turned: false, editor: true },
        Order::PaintOre { ore: Some(Ore::Iron), tiles: under.iter().copied().chain([beside]).collect() },
        Order::PlaceBuilding { kind: BuildingKind::Wall, tile: beside, turned: false, editor: true },
    ]);
    let planned = world.worked_tiles(BuildingKind::Mine, spot, world.buildings.len());
    assert_eq!(planned, under, "the preview should show the ore under the mine, not the one under the wall");
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Mine, tile: spot, turned: false, editor: false }]);
    let b = world.buildings.iter().position(|b| b.kind == BuildingKind::Mine).expect("not built on the ore");
    assert_eq!(world.worked_tiles(BuildingKind::Mine, spot, b), planned, "built, it works other tiles than planned");
    world.stock.set(Resource::Iron, 0);
    no_more_zombies(&mut world);
    for _ in 0..10 {
        one_payout(&mut world);
    }
    // 10 payouts, 0.1 per tile: one iron ore per tile under it.
    assert_eq!(world.stock.get(Resource::Iron), under.len() as u32);
}

/// The barracks costs wood and stone besides its gold: one short, and it isn't built; enough, and both are paid.
#[test]
fn buildings_cost_wood_and_stone() {
    let (mut world, _) = quiet_world();
    let tile = (world.base.x as i32 - 7, world.base.y as i32 - 8);
    clear_ground(&mut world, (tile.0 + 1, tile.1 + 1), 4);
    let s = BuildingKind::Barracks.stats();
    assert!(s.cost_wood > 0 && s.cost_stone > 0);
    world.stock.set(Resource::Wood, s.cost_wood - 1);
    world.stock.set(Resource::Stone, s.cost_stone);
    let place = Order::PlaceBuilding { kind: BuildingKind::Barracks, tile, turned: false, editor: false };
    world.step(std::slice::from_ref(&place));
    assert!(!world.buildings.iter().any(|b| b.kind == BuildingKind::Barracks), "built without enough wood");
    world.stock.set(Resource::Wood, s.cost_wood);
    let gold = world.gold;
    world.step(&[place]);
    assert!(world.buildings.iter().any(|b| b.kind == BuildingKind::Barracks), "not built");
    assert_eq!((world.stock.get(Resource::Wood), world.stock.get(Resource::Stone)), (0, 0));
    assert_eq!(world.gold, gold - s.cost);
}

/// A building that needs power only goes inside a grid. A tesla tower inside it carries the power further; take it
/// away, and what stood in its circle only is out of the grid – no power for it.
#[test]
fn tesla_towers_carry_the_power_further() {
    let (mut world, _) = quiet_world();
    world.extra_reach = 0.0;
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let reach = BuildingKind::CommandCenter.stats().power_radius as i32;
    let far = (bx + reach + 3, by);
    clear_ground(&mut world, far, 3);
    assert!(!world.can_place(BuildingKind::Tent, far, false), "a tent outside every grid");
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::WoodWorkshop, tile: (bx - 7, by + 6), turned: false, editor: true }]);
    let pole = (bx + reach - 2, by);
    clear_ground(&mut world, pole, 1);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::TeslaTower, tile: pole, turned: false, editor: false }]);
    assert_eq!(world.buildings.last().unwrap().kind, BuildingKind::TeslaTower, "the tesla tower isn't built");
    assert!(world.can_place(BuildingKind::Tent, far, false), "the tesla tower doesn't carry the power");
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tent, tile: far, turned: false, editor: false }]);
    let tent = world.buildings.len() - 1;
    assert_eq!(world.buildings[tent].kind, BuildingKind::Tent);
    let power = world.power();
    assert_eq!(power.grid_of[tent], power.grid_of[0], "the tent isn't in the command center's grid");
    assert_eq!(power.share(&world, tent), 1000);

    world.apply_orders(&[Order::RemoveAt { tiles: vec![pole] }]);
    let tent = world.buildings.iter().position(|b| b.kind == BuildingKind::Tent).unwrap();
    assert_eq!(world.power().share(&world, tent), 0, "power without the tesla tower");
}

/// Like in TAB: a building that takes power is only built where its grid has that much to spare – one sawmill more
/// than the command center's power is enough for stays unbuilt.
#[test]
fn nothing_is_built_without_spare_power() {
    let (mut world, _) = quiet_world();
    world.extra_power = 0;
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let fits = BuildingKind::CommandCenter.stats().energy / BuildingKind::Sawmill.stats().energy_use;
    clear_ground(&mut world, (bx, by - 9), 12);
    let tiles: Vec<(i32, i32)> = (0..fits as i32 + 1).map(|k| (bx - 10 + 3 * k, by - 8)).collect();
    for &tile in &tiles {
        world.step(&[Order::PlaceBuilding { kind: BuildingKind::Sawmill, tile, turned: false, editor: false }]);
    }
    let built = world.buildings.iter().filter(|b| b.kind == BuildingKind::Sawmill).count() as u32;
    assert_eq!(built, fits, "built more than the power is enough for");
    assert!(!world.has_power_for(BuildingKind::Sawmill, *tiles.last().unwrap(), false));
}

/// More taken than made in a grid: every building in it gets only that share – tents pay that much less gold. A
/// charcoal burner with wood makes up for it and burns its wood every hour; without wood it makes nothing.
#[test]
fn too_little_power_slows_the_grid_and_burners_help() {
    let (mut world, _) = quiet_world();
    world.extra_power = 0;
    // Far enough for every row of tents.
    world.extra_reach = 20.0;
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let made = BuildingKind::CommandCenter.stats().energy;
    let tent_use = BuildingKind::Tent.stats().energy_use;
    // Twice as many tents as the command center has power for – placed like in the editor, in rows of 10 above it.
    let tents = (2 * made / tent_use) as i32;
    let tiles: Vec<(i32, i32)> = (0..tents).map(|k| (bx - 10 + 2 * (k % 10), by - 6 - 3 * (k / 10))).collect();
    clear_ground(&mut world, (bx, by - 9), 12);
    let orders: Vec<Order> =
        tiles.iter().map(|&tile| Order::PlaceBuilding { kind: BuildingKind::Tent, tile, turned: false, editor: true }).collect();
    world.apply_orders(&orders);
    let tent = world.buildings.iter().position(|b| b.kind == BuildingKind::Tent).unwrap();
    assert_eq!(world.power().share(&world, tent), 500);
    one_payout(&mut world);
    let full = world.buildings[tent].full_income();
    assert_eq!(world.buildings[tent].paid, full / 2, "half the power, half the gold");

    let burner = (bx + 5, by + 5);
    clear_ground(&mut world, burner, 2);
    world.stock.set(Resource::Wood, 100);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::CharcoalBurner, tile: burner, turned: false, editor: false }]);
    let b = world.buildings.len() - 1;
    assert!(world.buildings[b].fuelled, "it doesn't burn right away");
    let after_building = world.stock.get(Resource::Wood);
    one_payout(&mut world);
    assert_eq!(world.stock.get(Resource::Wood), after_building - BuildingKind::CharcoalBurner.stats().burns_wood);
    let power = world.power();
    let g = power.grid_of[b].unwrap();
    assert_eq!(power.made[g], made + BuildingKind::CharcoalBurner.stats().energy);

    world.stock.set(Resource::Wood, 0);
    one_payout(&mut world);
    assert!(!world.buildings[b].fuelled);
    assert_eq!(world.power().made[g], made, "a burner without wood still makes power");
}

/// The storage holds only so much: gold and food that come in at a payout stop at the limit, the rest is lost. More
/// than the limit already in it (the editor) stays.
#[test]
fn the_storage_is_full_at_its_limit() {
    let (mut world, _) = quiet_world();
    world.extra_storage = 0;
    let (gold, food) = (world.gold_storage(), world.storage(Resource::Food));
    world.gold = gold - 1;
    world.stock.set(Resource::Food, food - 1);
    one_payout(&mut world);
    // Full – then the upkeep and the soldiers' food go out.
    let units_food: u32 = world.units.iter().map(|u| u.stats().food).sum();
    let left = (gold - world.upkeep(), food - units_food);
    assert_eq!((world.gold, world.stock.get(Resource::Food)), left, "more than the storage holds");
    world.gold = gold + 500;
    one_payout(&mut world);
    assert_eq!(world.gold, gold + 500 - world.upkeep(), "the storage took gold away");
}

/// Tents eat at every payout – without food they pay no gold. The command center brings in a little food by itself,
/// the soldiers eat first: one tent more than the rest feeds, and the last one goes hungry.
#[test]
fn hungry_tents_pay_no_gold() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let units_food: u32 = world.units.iter().map(|u| u.stats().food).sum();
    let left = BuildingKind::CommandCenter.stats().food_made - units_food;
    let fed = (left / BuildingKind::Tent.stats().food) as i32;
    assert!(fed > 0);
    clear_ground(&mut world, (bx, by - 9), 12);
    let tents: Vec<Order> = (0..fed + 1)
        .map(|k| Order::PlaceBuilding { kind: BuildingKind::Tent, tile: (bx - 10 + 2 * (k % 10), by - 6 - 3 * (k / 10)), turned: false, editor: true })
        .collect();
    world.apply_orders(&tents);
    world.stock.set(Resource::Food, 0);
    one_payout(&mut world);
    let tents: Vec<&Building> = world.buildings.iter().filter(|b| b.kind == BuildingKind::Tent).collect();
    assert_eq!(tents.len() as i32, fed + 1);
    let (hungry, last) = (tents.iter().filter(|t| !t.fed).count(), tents.last().unwrap());
    assert_eq!(hungry, 1, "the command center's food feeds the others");
    assert!(!last.fed && last.paid == 0, "a hungry tent paid gold");
    assert!(tents[0].fed && tents[0].paid > 0);
}

/// Every hour the units and the running buildings cost their upkeep. What the gold doesn't cover is debt – and gold
/// that comes in pays it off first.
#[test]
fn upkeep_is_paid_every_hour_and_the_rest_is_debt() {
    let (mut world, _) = quiet_world();
    let units: u32 = world.units.iter().map(|u| u.stats().upkeep).sum();
    assert!(units > 0 && world.upkeep() == units, "the start units cost upkeep, the command center none");
    // The command center pays its gold every payout – snipers until they cost more than that.
    let center = BuildingKind::CommandCenter.stats().gold;
    while world.upkeep() <= center + 5 {
        world.spawn_unit(UnitKind::Sniper, world.base);
    }
    let upkeep = world.upkeep();
    world.gold = upkeep - center + 5;
    one_payout(&mut world);
    assert_eq!((world.gold, world.debt), (5, 0));
    one_payout(&mut world);
    assert_eq!((world.gold, world.debt), (0, upkeep - center - 5));
    assert_eq!(world.debt_stage(), DebtStage::Debt);
    world.gold = upkeep; // loot, say
    world.step(&[]);
    assert_eq!((world.gold, world.debt), (center + 5, 0), "gold that comes in pays the debt first");
    assert_eq!(world.debt_stage(), DebtStage::None);
}

/// In debt the units do less damage; deep in debt they desert – one every `desert_every_seconds`, and it's gone for
/// good (no corpse to rise).
#[test]
fn deep_in_debt_units_get_weaker_and_desert() {
    let (mut world, _) = quiet_world();
    let colony = &data::get().colony;
    let damage = UnitKind::Sniper.stats().damage;
    world.gold = 0;
    world.debt = 1;
    world.step(&[]);
    assert_eq!(world.army_damage(damage), damage * (100 - colony.debt_penalty_percent) / 100);
    let units = world.units.len();
    world.debt = world.upkeep() * colony.desert_debt_payouts + 1;
    for _ in 0..colony.desert_every {
        world.step(&[]);
    }
    assert_eq!(world.debt_stage(), DebtStage::Desert);
    assert_eq!(world.units.len(), units - 1, "one deserted");
    assert_eq!(world.score.units_deserted, 1);
    assert!(world.corpses.is_empty(), "a deserter isn't a corpse");
    assert_eq!(world.army_damage(damage), damage * (100 - colony.deep_debt_penalty_percent) / 100);
}

/// Y switches a building off: no upkeep, its workers are free, it gathers nothing, and as a workshop it unlocks
/// nothing. On again only with enough free workers and gold income for its upkeep.
#[test]
fn a_switched_off_building_costs_and_does_nothing() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let t = (world.base.x as i32 + 8, world.base.y as i32 - 8);
    clear_ground(&mut world, t, 6);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Sawmill, tile: t, turned: false, editor: false }]);
    let b = world.buildings.len() - 1;
    let id = world.buildings[b].id;
    let stats = BuildingKind::Sawmill.stats();
    assert_eq!(world.upkeep(), stats.upkeep);
    let free = world.free_workers();
    world.step(&[Order::SwitchOff { buildings: vec![id], off: true }]);
    assert!(world.buildings[b].off);
    assert_eq!(world.upkeep(), 0);
    assert_eq!(world.free_workers(), free + stats.worker_cost);
    // Plant forest around it: switched off it brings in nothing.
    let tiles = (t.1 - 4..t.1 + 6).flat_map(|y| (t.0 - 4..t.0 + 6).map(move |x| (x, y))).filter(|&(x, y)| !world.map.is_built(x, y)).collect();
    world.step(&[Order::PaintTerrain { terrain: Terrain::Forest, tiles }]);
    let wood = world.stock.get(Resource::Wood);
    let gold = world.gold;
    one_payout(&mut world);
    assert_eq!(world.stock.get(Resource::Wood), wood, "a switched off sawmill gathered wood");
    assert_eq!(world.gold, gold + BuildingKind::CommandCenter.stats().gold, "a switched off sawmill cost upkeep");
    // No free workers (rangers took them all): it stays off.
    world.extra_workers = 0;
    let point = world.base + vec2(0.0, 4.0);
    let rangers: Vec<Order> = (0..world.free_workers()).map(|_| Order::SpawnUnit { kind: UnitKind::Ranger, point }).collect();
    world.step(&rangers);
    assert_eq!(world.free_workers(), 0);
    world.step(&[Order::SwitchOff { buildings: vec![id], off: false }]);
    assert_eq!(world.switch_on_blocked(b), Some(SwitchBlock::Workers));
    assert!(world.buildings[b].off, "on again without workers");
    world.units.clear();
    world.extra_workers = 100_000;
    // Snipers cost more upkeep than the command center pays: it stays off too.
    let snipers = world.income() / UnitKind::Sniper.stats().upkeep + 1;
    let orders: Vec<Order> = (0..snipers).map(|_| Order::SpawnUnit { kind: UnitKind::Sniper, point }).collect();
    world.step(&orders);
    world.step(&[Order::SwitchOff { buildings: vec![id], off: false }]);
    assert_eq!(world.switch_on_blocked(b), Some(SwitchBlock::Income));
    assert!(world.buildings[b].off, "on again without gold income");
    // Without them the income is enough.
    world.units.clear();
    assert!(world.income() >= stats.upkeep);
    world.step(&[Order::SwitchOff { buildings: vec![id], off: false }]);
    assert!(!world.buildings[b].off);

    // A workshop switched off unlocks nothing.
    let w = (t.0 - 6, t.1);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::WoodWorkshop, tile: w, turned: false, editor: false }]);
    assert!(world.unlocked(BuildingKind::Windmill));
    let workshop = world.buildings.last().unwrap().id;
    world.step(&[Order::SwitchOff { buildings: vec![workshop], off: true }]);
    assert!(!world.unlocked(BuildingKind::Windmill));
}

/// Firearms need the munitions factory: without it the barracks takes no soldier (and no gold for one) – a ranger
/// (a bow) it does. The factory itself needs the stone workshop, which needs the wood workshop.
#[test]
fn firearms_need_the_munitions_factory() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let tile = (bx - 7, by - 8);
    clear_ground(&mut world, (tile.0 + 1, tile.1 + 1), 6);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::Barracks, tile, turned: false, editor: true }]);
    let id = world.buildings.last().unwrap().id;
    let gold = world.gold;
    world.apply_orders(&[Order::Train { building: id, kind: UnitKind::Soldier }, Order::Train { building: id, kind: UnitKind::Sniper }]);
    assert_eq!(world.gold, gold, "paid for a firearm without a munitions factory");
    world.apply_orders(&[Order::Train { building: id, kind: UnitKind::Ranger }]);
    assert_eq!(world.buildings.last().unwrap().production.len(), 1, "no ranger");

    assert!(!world.unlocked(BuildingKind::StoneWorkshop) && !world.unlocked(BuildingKind::MunitionsFactory));
    let at = |dx: i32| (tile.0 + dx, tile.1 + 4);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::WoodWorkshop, tile: at(0), turned: false, editor: true }]);
    assert!(world.unlocked(BuildingKind::StoneWorkshop) && !world.unlocked(BuildingKind::MunitionsFactory));
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::StoneWorkshop, tile: at(3), turned: false, editor: false }]);
    let coal = world.stock.get(Resource::Coal);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::MunitionsFactory, tile: at(6), turned: false, editor: false }]);
    assert_eq!(world.buildings.last().unwrap().kind, BuildingKind::MunitionsFactory, "not built");
    assert_eq!(world.stock.get(Resource::Coal), coal - BuildingKind::MunitionsFactory.stats().cost_coal);
    world.apply_orders(&[Order::Train { building: id, kind: UnitKind::Soldier }]);
    let b = world.find_building(id).unwrap();
    assert_eq!(world.buildings[b].production.len(), 2, "no soldier with the munitions factory");
}

/// The stone buildings go on their wooden ones – an upgrade on the spot: the tent becomes a stone house, the wall a
/// stone wall. Part of the old one's gold comes back, like a gate on walls.
#[test]
fn stone_buildings_upgrade_their_wooden_ones() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let t = (bx + 6, by - 8);
    clear_ground(&mut world, t, 4);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::StoneWorkshop, tile: (bx - 7, by + 6), turned: false, editor: true }]);
    let wall = (t.0 + 3, t.1);
    world.step(&[
        Order::PlaceBuilding { kind: BuildingKind::Tent, tile: t, turned: false, editor: false },
        Order::PlaceBuilding { kind: BuildingKind::Wall, tile: wall, turned: false, editor: false },
    ]);
    let refund = BuildingKind::Tent.stats().cost * data::get().buildings.replace_refund_percent / 100;
    assert_eq!(world.build_cost(BuildingKind::StoneHouse, t, false), BuildingKind::StoneHouse.stats().cost - refund);
    world.step(&[
        Order::PlaceBuilding { kind: BuildingKind::StoneHouse, tile: t, turned: false, editor: false },
        Order::PlaceBuilding { kind: BuildingKind::StoneWall, tile: wall, turned: false, editor: false },
    ]);
    let kinds: Vec<BuildingKind> = world.buildings.iter().map(|b| b.kind).collect();
    assert!(!kinds.contains(&BuildingKind::Tent) && !kinds.contains(&BuildingKind::Wall), "the old ones are still there");
    assert!(kinds.contains(&BuildingKind::StoneHouse) && kinds.contains(&BuildingKind::StoneWall), "no upgrade");
}

/// The stake trap: everyone walks over it, zombies don't go for it – the ones on it get hurt every
/// `trap_every_seconds`, and every hit wears it down until it's gone.
#[test]
fn stakes_hurt_zombies_and_wear_out() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let t = (bx + 8, by + 8);
    clear_ground(&mut world, t, 5);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::WoodWorkshop, tile: (bx - 7, by - 8), turned: false, editor: true }]);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Stakes, tile: t, turned: false, editor: false }]);
    let trap = world.buildings.len() - 1;
    assert_eq!(world.buildings[trap].kind, BuildingKind::Stakes, "not built");
    assert!(!world.map.is_blocked(t.0, t.1) && !world.map.blocks_unit(t.0, t.1), "the trap blocks the way");
    assert!(world.buildings[trap].zombies_ignore());

    world.units.clear(); // nobody shoots it – only the trap hurts it
    let on_trap = vec2(t.0 as f32 + 0.5, t.1 as f32 + 0.5);
    world.add_zombie(on_trap, Breed::Fat, true);
    let hp = world.zombies[0].hp;
    let trap_hp = world.buildings[trap].hp;
    for _ in 0..data::get().buildings.trap_every {
        world.step(&[]);
    }
    let stats = BuildingKind::Stakes.stats();
    let damage = stats.trap_damage.saturating_sub(Breed::Fat.stats().armor).max(1);
    assert_eq!(world.zombies[0].hp, hp - damage, "the zombie on the trap isn't hurt");
    assert_eq!(world.buildings[trap].hp, trap_hp - stats.trap_wear);

    world.buildings[trap].hp = stats.trap_wear;
    world.zombies[0].pos = on_trap; // it may have wandered off meanwhile
    for _ in 0..data::get().buildings.trap_every {
        world.step(&[]);
    }
    assert!(!world.buildings.iter().any(|b| b.kind == BuildingKind::Stakes), "the worn-out trap is still there");
}

/// A house of doom on cleared ground 14 tiles up right of the base, far from the start units. Returns its index.
fn doom_house(world: &mut World) -> usize {
    let tile = (world.base.x as i32 + 14, world.base.y as i32 - 14);
    clear_ground(world, (tile.0 + 1, tile.1 + 1), 12);
    world.step(&[Order::PlaceDoom { tile, size: (3, 3) }]);
    assert_eq!(world.doom.len(), 1, "the house didn't fit");
    assert!(world.map.is_doom(tile.0 + 1, tile.1 + 1) && world.map.is_blocked(tile.0, tile.1) && !world.map.is_buildable(tile.0, tile.1));
    0
}

/// A house of doom sleeps while your units are far away. One of them comes within `wake_radius`: only a few come out
/// (`near_zombies`), going for it – and again after `near_every_seconds` while it stays.
#[test]
fn a_unit_close_to_a_village_lets_only_a_few_out() {
    let (mut world, _) = quiet_world();
    let d = doom_house(&mut world);
    let inside = world.doom[d].inside;
    assert!(inside > 10, "only {inside} zombies inside");
    for _ in 0..40 {
        world.step(&[]);
    }
    assert!(world.zombies.is_empty(), "zombies came out with nobody near");

    let doom = &data::get().doom;
    let center = world.doom[d].center();
    let spot = center + vec2(-1.5 - doom.wake_radius + 1.0, 0.0);
    bait(&mut world, UnitKind::Soldier, spot);
    world.units.last_mut().unwrap().hp = 1_000_000; // it has to live through the crowd
    for _ in 0..20 {
        world.step(&[]);
    }
    assert_eq!(world.zombies.len() as u32, doom.near_zombies, "not just a few came out");
    assert_eq!(world.doom[d].inside, inside - doom.near_zombies);
    assert!(world.zombies.iter().all(|z| z.lure.is_some_and(|l| l.to.distance(spot) < 1.0)), "they don't go for the unit");
    for _ in 0..doom.near_every {
        world.step(&[]);
    }
    assert_eq!(world.doom[d].inside, inside - 2 * doom.near_zombies, "the unit stayed, but no more came");
}

/// What really rouses a village is noise (`sound.rs`): a sound loud enough within its `hearing` and its zombies pour
/// out – `release_per_second` – and go where the sound was. A quieter one does nothing. The guards of loot the same.
#[test]
fn noise_rouses_a_village_and_its_zombies_go_there() {
    let (mut world, _) = quiet_world();
    let d = doom_house(&mut world);
    let center = world.doom[d].center();
    world.add_loot(center + vec2(-6.0, 6.0), LootKind::Wood, 10, 5);
    let at = center + vec2(-10.0, 0.0);
    let doom = &data::get().doom;
    world.make_sound(at, doom.hear_threshold / 2);
    for _ in 0..20 {
        world.step(&[]);
    }
    assert!(world.zombies.is_empty() && world.loot[0].guards == 5, "a quiet sound roused them");

    let loud = data::get().loot.hear_threshold.max(doom.hear_threshold) * 2;
    world.make_sound(at, loud);
    for _ in 0..20 {
        world.step(&[]);
    }
    assert_eq!(world.loot[0].guards, 0, "the loud sound didn't raise the guards");
    let out = world.zombies.len() as f32 - 5.0;
    assert!(out >= doom.release_per_second * 0.5, "only {out} came out of the house");
    assert!(world.zombies.iter().all(|z| z.lure.is_some_and(|l| l.to == at)), "they don't go to the sound");
}

/// Your units shoot a house of doom when no zombie is in range – a sniper even from outside its wake radius. Every hit
/// lets `hit_zombies` more out, on top of the ones inside. Shot down, it lets out everyone still inside at once – they
/// go for the shooter – drops loot where it stood, and its tiles are open again.
#[test]
fn a_shot_down_house_drops_loot() {
    let (mut world, _) = quiet_world();
    let d = doom_house(&mut world);
    let (inside, center, tile) = (world.doom[d].inside, world.doom[d].center(), world.doom[d].tile);
    let doom = &data::get().doom;
    // Its wall 7.5 tiles away: out of the wake radius, in the sniper's range.
    let sniper = center + vec2(-1.5 - (doom.wake_radius + 0.5), 0.0);
    world.spawn_unit(UnitKind::Sniper, sniper);
    world.doom[d].hp = 1;
    let mut fell = false;
    for _ in 0..60 {
        world.step(&[]);
        if world.doom.is_empty() {
            fell = true;
            break;
        }
    }
    assert!(fell, "the sniper didn't shoot the house down");
    assert_eq!(world.zombies.len() as u32, inside + doom.hit_zombies, "not all came out at once, or no extra for the hit");
    assert!(world.zombies.iter().all(|z| z.lure.is_some_and(|l| l.to.distance(sniper) < 0.5)), "they don't go for the shooter");
    assert_eq!(world.loot.len(), 1);
    assert_eq!(world.loot[0].pos, center);
    assert_eq!(world.loot[0].guards, 0);
    assert!(!world.map.is_doom(tile.0, tile.1) && !world.map.is_blocked(tile.0 + 1, tile.1 + 1), "its tiles are still taken");
    assert_eq!(world.score.doom_razed, 1);
}

/// Loot: its hidden guards come out when a unit comes within `wake_radius`, and whoever walks onto it picks it up –
/// gold into the gold, goods into the stock.
#[test]
fn loot_is_picked_up_and_its_guards_come_out() {
    let (mut world, id) = quiet_world();
    let spot = world.base + vec2(-12.5, 0.5);
    clear_ground(&mut world, (spot.x as i32, spot.y as i32), 8);
    world.add_loot(spot, LootKind::Gold, 500, 12);
    world.add_loot(spot + vec2(0.0, 4.0), LootKind::Stone, 10, 0);
    let (gold, stone) = (world.gold, world.stock.get(Resource::Stone));
    let u = world.find_unit(id).unwrap();
    world.units[u].hp = 1_000_000;
    world.step(&[Order::Command { units: vec![id], command: Command::Move(spot), queue: false, formation: None }]);
    world.step(&[Order::Command { units: vec![id], command: Command::Move(spot + vec2(0.0, 4.0)), queue: true, formation: None }]);
    let mut guards_out = false;
    for _ in 0..400 {
        world.step(&[]);
        guards_out |= world.zombies.len() == 12;
        if world.loot.is_empty() {
            break;
        }
    }
    assert!(guards_out, "the guards didn't come out");
    assert!(world.loot.is_empty(), "the loot wasn't picked up");
    assert_eq!(world.gold, gold + 500);
    assert_eq!(world.stock.get(Resource::Stone), stone + 10);
    assert_eq!(world.score.loot_found, 2);
}

/// A spitter spits acid at a unit in its range: it flies a moment and burns everyone around where the unit stood – a
/// unit next to it too, one further away not. A unit that steps away in time isn't hit.
#[test]
fn spitter_acid_flies_and_splashes() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(-10.5, 8.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 8);
    let stats = Breed::Spitter.stats();
    let a = bait(&mut world, UnitKind::Ranger, p);
    let b = bait(&mut world, UnitKind::Ranger, p + vec2(0.0, stats.splash * 0.5));
    let c = bait(&mut world, UnitKind::Ranger, p + vec2(-(stats.splash + 1.0), 0.0));
    world.spawn_zombie_as(p + vec2(stats.range - 0.5, 0.0), Breed::Spitter, false);
    let hp = |world: &World, id: u32| world.units[world.find_unit(id).unwrap()].hp;
    let full = UnitKind::Ranger.stats().hp;
    let mut flew = false;
    for _ in 0..60 {
        world.step(&[]);
        flew |= !world.spits.is_empty() && hp(&world, a) == full;
        if hp(&world, a) < full {
            break;
        }
    }
    assert!(flew, "the acid didn't fly – it hit at once");
    assert_eq!(hp(&world, a), full - stats.damage, "the target");
    assert_eq!(hp(&world, b), full - stats.damage, "next to the target");
    assert_eq!(hp(&world, c), full, "further away");

    // Once more – but this time the target steps aside while the acid flies.
    world.zombies.clear();
    world.spits.clear();
    world.spawn_zombie_as(p + vec2(stats.range - 0.5, 0.0), Breed::Spitter, false);
    while world.spits.is_empty() {
        world.step(&[]);
    }
    let a_at = world.find_unit(a).unwrap();
    world.units[a_at].pos += vec2(0.0, -3.0);
    let before = hp(&world, a);
    while !world.spits.is_empty() {
        world.step(&[]);
    }
    assert_eq!(hp(&world, a), before, "the acid followed the unit");
}

/// The infested shoots back like StarCraft's infested marine: from a little less far than your soldiers, at once
/// (no flying acid), and it stands still to shoot.
#[test]
fn infested_shoot_from_almost_a_soldiers_range() {
    let range = Breed::Infested.stats().range;
    let soldier = UnitKind::Soldier.stats().range;
    assert!(range < soldier && range >= soldier - 1.0, "infested {range}, soldier {soldier}");
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(-10.5, 8.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 8);
    let id = bait(&mut world, UnitKind::Ranger, p);
    let at = p + vec2(range - 0.3, 0.0);
    world.spawn_zombie_as(at, Breed::Infested, false);
    let mut shot = false;
    for _ in 0..40 {
        world.step(&[]);
        shot |= !world.zombie_shots.is_empty();
    }
    assert!(shot, "no shot");
    assert!(world.spits.is_empty(), "the infested's shot flies");
    assert!(world.units[world.find_unit(id).unwrap()].hp < UnitKind::Ranger.stats().hp, "the shot did nothing");
    assert!(world.zombies[0].pos.distance(at) < 0.3, "it walked up instead of shooting from where it was");
}

/// The armored runner (TAB's): as fast as a runner, but its armor eats most of an arrow – a sniper still kills it with
/// one shot.
#[test]
fn armored_runners_are_fast_and_shrug_off_arrows() {
    let (armored, runner) = (Breed::Armored.stats(), Breed::Runner.stats());
    assert_eq!(armored.speed, runner.speed);
    assert!(armored.armor > runner.armor);
    let shots = |kind: UnitKind, breed: Breed| breed.stats().hp.div_ceil(kind.stats().damage.saturating_sub(breed.stats().armor).max(1));
    assert!(shots(UnitKind::Ranger, Breed::Armored) > shots(UnitKind::Ranger, Breed::Runner));
    assert_eq!(shots(UnitKind::Sniper, Breed::Armored), 1);
    assert_ne!(Breed::Armored.name(), "Suits");
}

/// A farm needs the wood workshop, stands only on grass – not on dirt – and only plants the grass around it: dirt and
/// ore give nothing. Every hour its fields bring food.
#[test]
fn farms_grow_only_on_grass() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let spot = (bx + 6, by - 10);
    clear_ground(&mut world, (spot.0 + 1, spot.1 + 1), 7);
    let farm = Order::PlaceBuilding { kind: BuildingKind::Farm, tile: spot, turned: false, editor: false };
    world.step(&[farm.clone()]);
    assert!(!world.buildings.iter().any(|b| b.kind == BuildingKind::Farm), "built without a wood workshop");
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::WoodWorkshop, tile: (bx - 8, by + 6), turned: false, editor: true }]);
    world.step(&[Order::PaintTerrain { terrain: Terrain::Dirt, tiles: vec![(spot.0 + 1, spot.1 + 1)] }]);
    world.step(&[farm.clone()]);
    assert!(!world.buildings.iter().any(|b| b.kind == BuildingKind::Farm), "built on dirt");
    world.step(&[Order::PaintTerrain { terrain: Terrain::Grass, tiles: vec![(spot.0 + 1, spot.1 + 1)] }]);
    world.step(&[farm]);
    let b = world.buildings.iter().position(|b| b.kind == BuildingKind::Farm).expect("not built on grass");
    let tile = world.buildings[b].tile;
    let all_grass = world.worked_tiles(BuildingKind::Farm, tile, b).len();
    assert!(all_grass > 30, "only {all_grass} fields");
    // Dirt on one side, ore on the other: no fields there.
    let dirt: Vec<(i32, i32)> = (spot.1 - 1..spot.1 + 4).map(|y| (spot.0 - 1, y)).collect();
    world.step(&[Order::PaintTerrain { terrain: Terrain::Dirt, tiles: dirt.clone() }]);
    world.step(&[Order::PaintOre { ore: Some(Ore::Coal), tiles: vec![(spot.0 + 4, spot.1 + 1)] }]);
    let fields = world.worked_tiles(BuildingKind::Farm, tile, b).len();
    assert_eq!(fields, all_grass - dirt.len() - 1);
    world.stock.set(Resource::Food, 0);
    no_more_zombies(&mut world);
    for _ in 0..10 {
        one_payout(&mut world);
    }
    let per_tile = BuildingKind::Farm.stats().gather_per_tile;
    let units_food: u32 = world.units.iter().map(|u| u.stats().food).sum();
    let center = BuildingKind::CommandCenter.stats().food_made - units_food;
    assert_eq!(world.stock.get(Resource::Food), (fields as f32 * per_tile * 10.0 + 0.001) as u32 + 10 * center);
}

/// A new round has villages of doom and loot far from the base, the houses never on top of each other or cutting off a
/// way – every open tile still reaches the base – and dirt on part of the open ground.
#[test]
fn a_new_round_has_villages_loot_and_dirt() {
    let world = World::new(5);
    assert!(world.doom.len() >= 3, "only {} houses", world.doom.len());
    assert!(world.loot.len() >= 3, "only {} piles of loot", world.loot.len());
    let keep_out = MapSetup::default().common.base_clear;
    for house in &world.doom {
        assert!(house.center().distance(world.base) > keep_out + 10.0, "a house right at the base");
        assert!(house.inside > 0);
    }
    for loot in &world.loot {
        assert!(loot.pos.distance(world.base) > keep_out + 10.0, "loot right at the base");
    }
    // Every tile a unit can stand on reaches the base, around the houses.
    let n = world.map.size as i32;
    let mut seen = vec![false; world.map.size * world.map.size];
    let c = n / 2;
    let mut stack = vec![(c, c + 3)];
    while let Some((x, y)) = stack.pop() {
        for (nx, ny) in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
            let i = (ny * n + nx) as usize;
            if nx >= 0 && ny >= 0 && nx < n && ny < n && !world.map.blocks_unit(nx, ny) && !world.map.is_built(nx, ny) && !seen[i] {
                seen[i] = true;
                stack.push((nx, ny));
            }
        }
    }
    let cut = (0..n).flat_map(|y| (0..n).map(move |x| (x, y)))
        .filter(|&(x, y)| !world.map.blocks_unit(x, y) && !world.map.is_built(x, y) && !seen[(y * n + x) as usize])
        .count();
    assert_eq!(cut, 0, "{cut} tiles cut off by the houses");
    let open = (0..n).flat_map(|y| (0..n).map(move |x| (x, y))).filter(|&(x, y)| world.map.in_play(x, y) && !world.map.terrain(x, y).is_solid());
    let (all, dirt) = open.fold((0, 0), |(all, dirt), (x, y)| (all + 1, dirt + (world.map.terrain(x, y) == Terrain::Dirt) as u32));
    let share = dirt as f32 / all as f32;
    assert!((share - MapSetup::default().common.dirt).abs() < 0.05, "{:.0} % dirt", share * 100.0);
}

/// The default difficulty. With `$env:WAVES = "3"` first, the waves are that strong (3 = 300 %). With
/// `$env:ROAMERS = "0"` no zombies roam the map at the start – to look at the economy alone.
#[test]
#[ignore]
fn balance_mini_round() {
    let setup = MapSetup::default();
    let mut difficulty = data::get().difficulty("").clone();
    if let Some(waves) = std::env::var("WAVES").ok().and_then(|w| w.parse::<f32>().ok()) {
        difficulty.waves_percent = (waves * 100.0) as u32;
    }
    if let Some(roamers) = std::env::var("ROAMERS").ok().and_then(|r| r.parse().ok()) {
        difficulty.roaming_zombies = roamers;
    }
    for simple_player in [false, true] {
        let mut world = World::generate_with(1, &setup, &difficulty);
        println!("--- {} ---", if simple_player { "simple player" } else { "player does nothing" });
        let end = round::tick_of(world.days + 3, 0);
        while world.tick < end && world.outcome.is_none() {
            let orders = if simple_player && world.tick % HOUR_TICKS == 0 { simple_orders(&world) } else { Vec::new() };
            world.step(&orders);
            if world.tick % (12 * HOUR_TICKS) == 0 {
                let (day, hour) = world.clock();
                let cc = world.buildings.iter().find(|b| b.kind == BuildingKind::CommandCenter).map_or(0, |b| b.hp);
                let tents = world.buildings.iter().filter(|b| b.kind == BuildingKind::Tent && !b.infected).count();
                let barracks = world.buildings.iter().any(|b| b.kind == BuildingKind::Barracks);
                let power = world.power();
                let (made, used): (u32, u32) = (power.made.iter().sum(), power.used.iter().sum());
                let stock = |r: Resource| world.stock.get(r);
                println!(
                    "day {day:2} {hour:02}:00  zombies {:5}  units {:3}  tents {tents:2}  gold {:5} (+{}/h)  wood {:3} stone {:3} \
                     food {:3}  power {used}/{made}  barracks {}  CC {cc}",
                    world.zombies.len(),
                    world.units.len(),
                    world.gold,
                    world.income(),
                    stock(Resource::Wood),
                    stock(Resource::Stone),
                    stock(Resource::Food),
                    if barracks { "yes" } else { "no" },
                );
            }
        }
        println!("{:?}  {:?}", world.outcome, world.score);
    }
}

/// The simple player of `balance_mini_round`, once an hour, one thing at a time: a sawmill; food before every tent
/// (a hunter cottage when the food would run short); a charcoal burner when the power runs short; 4 tents; the wood
/// workshop; a mine on stone (a tesla tower first if no stone is in reach); the barracks; a second sawmill; the stone
/// workshop; a mine on coal if there's none, the munitions factory; then tents up to 12. Besides, once the barracks
/// stands: a unit whenever the gold is there – soldiers once the munitions factory stands, rangers before.
fn simple_orders(world: &World) -> Vec<Order> {
    use BuildingKind::*;
    if !world.buildings.iter().any(|b| b.kind == CommandCenter) {
        return Vec::new();
    }
    let count = |kind: BuildingKind| world.buildings.iter().filter(|b| b.kind == kind).count();
    let power = world.power();
    let flow = world.flow(&power);
    let (made, used): (u32, u32) = (power.made.iter().sum(), power.used.iter().sum());
    let food_after_tent = flow[Resource::Food.index()] - Tent.stats().food as i64 * 1000;
    let coal_short = world.stock.get(Resource::Coal) < MunitionsFactory.stats().cost_coal && flow[Resource::Coal.index()] == 0;
    // Which ore the next mine is for.
    let mut ore = Ore::Stone;
    let build = if count(Sawmill) == 0 {
        Sawmill
    } else if count(Tent) < 12 && food_after_tent < 0 {
        HunterCottage
    } else if used + 4 > made {
        CharcoalBurner
    } else if count(Tent) < 4 {
        Tent
    } else if count(WoodWorkshop) == 0 {
        WoodWorkshop
    } else if count(Mine) == 0 {
        Mine
    } else if count(Barracks) == 0 {
        Barracks
    } else if count(Sawmill) < 2 {
        Sawmill
    } else if count(StoneWorkshop) == 0 {
        StoneWorkshop
    } else if count(MunitionsFactory) == 0 && coal_short {
        ore = Ore::Coal;
        Mine
    } else if count(MunitionsFactory) == 0 {
        MunitionsFactory
    } else {
        Tent
    };
    let mut orders = build_order(world, build, ore);
    // A unit with what the gold leaves over.
    let unit = if world.unit_unlocked(UnitKind::Soldier) { UnitKind::Soldier } else { UnitKind::Ranger };
    let spent = orders.first().map_or(0, |_| build.stats().cost);
    let barracks = world.buildings.iter().find(|b| b.kind == Barracks);
    if let Some(b) = barracks.filter(|b| b.production.len() < 2 && world.gold >= spent + unit.stats().cost) {
        orders.push(Order::Train { building: b.id, kind: unit });
    }
    orders
}

/// The order that places `build` (or a tesla tower towards `ore`, `best_spot`) – if it's affordable.
fn build_order(world: &World, build: BuildingKind, ore: Ore) -> Vec<Order> {
    use BuildingKind::*;
    if build == Tent && world.buildings.iter().filter(|b| b.kind == Tent).count() >= 12 {
        return Vec::new();
    }
    // No spot for the mine inside the grid: a tesla tower towards the nearest patch of that ore first.
    let build = if build == Mine && best_spot(world, Mine, ore).is_none() { TeslaTower } else { build };
    let affordable = world.gold >= build.stats().cost && world.can_afford_goods(build);
    if !affordable || !world.unlocked(build) || world.free_workers() < build.stats().worker_cost {
        return Vec::new();
    }
    match best_spot(world, build, ore) {
        Some(tile) => vec![Order::PlaceBuilding { kind: build, tile, turned: false, editor: false }],
        None => Vec::new(),
    }
}

/// Where the simple player puts a building: on rings around the base, a gathering building where it works the most
/// tiles (a mine: tiles of `ore`; none: nowhere), a tesla tower as close as it gets to the nearest `ore`, anything else
/// on the first free spot.
fn best_spot(world: &World, build: BuildingKind, ore: Ore) -> Option<(i32, i32)> {
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let (w, h) = build.footprint(false);
    let nearest = world
        .map
        .all_deposits()
        .filter(|(_, _, d)| d.ore == ore)
        .map(|(x, y, _)| vec2(x as f32, y as f32))
        .min_by(|a, b| a.distance(world.base).total_cmp(&b.distance(world.base)));
    // (score, tile) – the higher the score, the better.
    let mut best: Option<(f32, (i32, i32))> = None;
    for r in 4..30 {
        for dy in -r..=r {
            for dx in -r..=r {
                let tile = (bx + dx, by + dy);
                if (dx.abs() != r && dy.abs() != r) || !world.can_place(build, tile, false) {
                    continue;
                }
                let center = vec2(tile.0 as f32 + w as f32 / 2.0, tile.1 as f32 + h as f32 / 2.0);
                let worked = || world.worked_tiles(build, tile, world.buildings.len());
                let score = if build == BuildingKind::Mine {
                    worked().into_iter().filter(|&(x, y)| world.map.deposit(x, y).is_some_and(|d| d.ore == ore)).count() as f32
                } else if build.gathers().is_some() {
                    worked().len() as f32
                } else if let (BuildingKind::TeslaTower, Some(nearest)) = (build, nearest) {
                    -center.distance(nearest)
                } else {
                    return Some(tile);
                };
                if best.is_none_or(|(most, _)| score > most) {
                    best = Some((score, tile));
                }
            }
        }
    }
    best.filter(|&(score, _)| build.gathers().is_none() || score > 0.0).map(|(_, tile)| tile)
}

/// The houses around a market pay half again as much gold – one outside its area doesn't. At the payout it sells what
/// lies above 80 % of the storage, as much as its customers buy (1 good per 4 colonists) – and switched off (Q), nothing.
#[test]
fn a_market_raises_its_houses_gold_and_sells_the_surplus() {
    let (mut world, _) = quiet_world();
    no_more_zombies(&mut world);
    world.extra_storage = 0;
    world.gold = 0;
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    clear_ground(&mut world, (bx + 13, by + 1), 9);
    let place = |kind, tile| Order::PlaceBuilding { kind, tile, turned: false, editor: true };
    // The market 3 × 3, its area 7 × 7 around it: three tents in it, one further out.
    world.apply_orders(&[
        place(BuildingKind::Market, (bx + 10, by)),
        place(BuildingKind::Tent, (bx + 8, by - 2)),
        place(BuildingKind::Tent, (bx + 13, by)),
        place(BuildingKind::Tent, (bx + 10, by + 3)),
        place(BuildingKind::Tent, (bx + 18, by)),
    ]);
    let tents: Vec<usize> = (0..world.buildings.len()).filter(|&b| world.buildings[b].kind == BuildingKind::Tent).collect();
    assert_eq!(tents.len(), 4, "a tent didn't fit");
    let m = world.buildings.iter().position(|b| b.kind == BuildingKind::Market).unwrap();
    let houses = world.market_houses(BuildingKind::Market.trade_area(world.buildings[m].tile), m);
    assert_eq!(houses, tents[..3].to_vec(), "the market's houses");
    assert_eq!(world.sell_limit(&houses), 3, "12 colonists, 1 good per 4");
    // Wood above the mark (40 of 50), everything else below it – the food too, after everyone ate.
    for r in Resource::ALL {
        world.stock.set(r, 0);
    }
    world.stock.set(Resource::Wood, 50);
    world.stock.set(Resource::Food, 60);
    one_payout(&mut world);
    let tent = BuildingKind::Tent.stats().gold;
    for &t in &tents[..3] {
        assert_eq!(world.buildings[t].paid, tent * 3 / 2, "a tent at the market");
    }
    assert_eq!(world.buildings[tents[3]].paid, tent, "the tent further out");
    let market = &world.buildings[m];
    assert_eq!((market.sold_goods, market.sold_gold), (3, 3 * data::get().buildings.sell_prices.wood));
    assert_eq!(world.stock.get(Resource::Wood), 47, "only the wood was above the mark");
    // Selling off: the wood above the mark stays.
    let id = world.buildings[m].id;
    world.step(&[Order::SetSelling { buildings: vec![id], on: false }]);
    one_payout(&mut world);
    assert_eq!(world.stock.get(Resource::Wood), 47, "sold although it was switched off");
    assert_eq!(world.buildings[m].sold_goods, 0);
}

/// A bank: more room for gold, and credit. Buying goes into it only as an `Order::Credit` (Shift) – and only up to the
/// credit. Debt inside it brings no penalties but interest. When the bank falls, its share of that debt is gone, and
/// so is the gold above the smaller storage.
#[test]
fn a_bank_gives_storage_and_credit() {
    let (mut world, _) = quiet_world();
    no_more_zombies(&mut world);
    world.extra_storage = 0;
    let barracks = barracks(&mut world);
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    clear_ground(&mut world, (bx + 9, by + 9), 3);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::Bank, tile: (bx + 8, by + 8), turned: false, editor: true }]);
    let bank = world.buildings.iter().position(|b| b.kind == BuildingKind::Bank).expect("the bank didn't fit");
    let (storage, credit) = (data::get().colony.storage_gold, BuildingKind::Bank.stats().credit);
    assert_eq!(world.gold_storage(), storage + BuildingKind::Bank.stats().gold_storage);
    assert_eq!(world.credit_line(), credit);

    // No gold: without Shift nothing, with Shift into the credit – and no penalty for it.
    world.gold = 0;
    let train = Order::Train { building: barracks, kind: UnitKind::Ranger };
    world.apply_orders(std::slice::from_ref(&train));
    assert_eq!(world.debt, 0, "bought on credit without Shift");
    world.apply_orders(&[Order::Credit(Box::new(train.clone()))]);
    let ranger = UnitKind::Ranger.stats().cost;
    assert_eq!(world.debt, ranger);
    assert_eq!(world.debt_stage(), DebtStage::None, "debt inside the credit brings a penalty");
    assert_eq!(world.interest_due(), (ranger * data::get().colony.credit_interest_percent).div_ceil(100));
    // Never beyond the credit: as many snipers as it pays for, the rest is refused (the queue has room for them).
    let sniper = UnitKind::Sniper.stats().cost;
    for _ in 0..3 {
        world.apply_orders(&[Order::Credit(Box::new(Order::Train { building: barracks, kind: UnitKind::Sniper }))]);
    }
    assert!(world.debt <= credit && world.debt + sniper > credit, "debt {} with a credit of {credit}", world.debt);

    // The bank falls: its share of the debt goes with it – here all of it.
    world.buildings[bank].hp = 0;
    world.step(&[]);
    world.step(&[]);
    assert_eq!(world.credit_line(), 0);
    assert_eq!(world.debt, 0, "the fallen bank's debt is still there");

    // A new bank, the storage full – it falls, and the gold above the old storage is lost.
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::Bank, tile: (bx + 8, by + 8), turned: false, editor: true }]);
    world.step(&[]);
    world.gold = world.gold_storage();
    let bank = world.buildings.iter().position(|b| b.kind == BuildingKind::Bank).unwrap();
    world.buildings[bank].hp = 0;
    world.step(&[]);
    world.step(&[]);
    assert_eq!(world.gold, storage);
}

/// Mercenaries come from the inn, not the barracks, and take no workers. Deep in debt – a stage before the others –
/// they desert, and only they.
#[test]
fn mercenaries_come_from_the_inn_and_leave_first() {
    let (mut world, _) = quiet_world();
    no_more_zombies(&mut world);
    let barracks = barracks(&mut world);
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    clear_ground(&mut world, (bx + 9, by + 9), 3);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::Inn, tile: (bx + 8, by + 8), turned: false, editor: true }]);
    let inn = world.buildings.last().unwrap();
    assert_eq!(inn.kind, BuildingKind::Inn, "the inn didn't fit");
    let inn = inn.id;
    world.apply_orders(&[Order::Train { building: barracks, kind: UnitKind::MercRanger }]);
    assert!(world.buildings.iter().all(|b| b.production.is_empty()), "the barracks trains mercenaries");
    let (used, _) = world.workers();
    let units = world.units.len();
    world.apply_orders(&[Order::Train { building: inn, kind: UnitKind::MercRanger }]);
    for _ in 0..UnitKind::MercRanger.stats().build_time + 5 {
        world.step(&[]);
    }
    assert_eq!(world.units.len(), units + 1, "no mercenary came out");
    assert_eq!(world.workers().0, used, "a mercenary took a worker");

    // Deep in debt, not deserting yet: the mercenaries go, the others stay.
    world.gold = 0;
    world.debt = world.upkeep() * data::get().colony.deep_debt_payouts + 1;
    assert_eq!(world.debt_stage(), DebtStage::Deep);
    let others = world.units.iter().filter(|u| !u.stats().mercenary).count();
    for _ in 0..data::get().colony.desert_every + 1 {
        world.step(&[]);
    }
    assert!(world.units.iter().all(|u| !u.stats().mercenary), "the mercenary stayed");
    assert_eq!(world.units.iter().filter(|u| !u.stats().mercenary).count(), others, "someone else deserted");
}

/// The scan shows a far circle of the map at once and for a while – then it's dark again. It uses a charge, which
/// comes back after a while – and while it charges, the command center's grid needs more power.
#[test]
fn the_scan_shows_a_circle_and_costs_power() {
    let (mut world, _) = quiet_world();
    let scan = &data::get().buildings.scan;
    let far = world.base + vec2(30.0, -30.0);
    assert!(!world.vision.sees(far));
    let used = |world: &World| world.power().used.iter().sum::<u32>();
    let before = used(&world);
    world.apply_orders(&[Order::Scan { at: far }]);
    assert!(world.vision.sees(far), "the scan shows nothing");
    assert_eq!(world.scan_charges, scan.charges - 1);
    assert_eq!(used(&world), before + scan.power, "charging costs no power");
    for _ in 0..scan.time + 1 {
        world.step(&[]);
    }
    assert!(!world.vision.sees(far), "still shown after the scan is over");
    for _ in 0..scan.every {
        world.step(&[]);
    }
    assert_eq!(world.scan_charges, scan.charges, "the charge didn't come back");
    assert_eq!(used(&world), before);
    // Without a charge nothing happens.
    world.scan_charges = 0;
    world.apply_orders(&[Order::Scan { at: far }]);
    assert!(world.scans.iter().all(|s| s.until <= world.tick) && world.scan_charges == 0);
}

/// Barrels: made in the munitions factory, set from the store. Hit, one goes off – and sets off the one next to it,
/// hurting the zombie standing there. A unit fetches one from the factory, carries it and puts it down; a carrier that
/// dies goes off too.
#[test]
fn barrels_go_off_and_set_each_other_off() {
    let (mut world, _) = quiet_world();
    no_more_zombies(&mut world);
    barracks(&mut world);
    let factory = world.buildings.iter().position(|b| b.kind == BuildingKind::MunitionsFactory).unwrap();
    let factory_id = world.buildings[factory].id;
    world.apply_orders(&[Order::MakeBarrel { building: factory_id }, Order::MakeBarrel { building: factory_id }]);
    for _ in 0..2 * data::get().buildings.blast.build_time + 5 {
        world.step(&[]);
    }
    assert_eq!(world.barrels, 2, "no barrels made");

    // Set one from the store, one more next to it.
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let spot = (bx + 8, by - 8);
    clear_ground(&mut world, spot, 4);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::Barrel, tile: spot, turned: false, editor: false }]);
    assert_eq!(world.barrels, 1, "setting it took none from the store");
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::Barrel, tile: (spot.0 + 2, spot.1), turned: false, editor: false }]);
    let barrels = |world: &World| world.buildings.iter().filter(|b| b.kind == BuildingKind::Barrel).count();
    assert_eq!(barrels(&world), 2);
    world.spawn_zombie_as(vec2(spot.0 as f32 + 3.5, spot.1 as f32 + 0.5), Breed::Walker, true);
    let walker = world.zombies.last().unwrap().id;
    let first = world.buildings.iter().position(|b| b.kind == BuildingKind::Barrel).unwrap();
    world.buildings[first].hp -= 1;
    world.step(&[]);
    assert_eq!(barrels(&world), 0, "not both went off");
    let hp = Breed::Walker.stats().hp;
    assert!(world.zombies.iter().find(|z| z.id == walker).is_none_or(|z| z.hp < hp), "the walker beside the second one wasn't hurt");

    // Fetch one from the factory's store, put it down elsewhere.
    world.barrels = 1;
    let id = world.units[0].id;
    world.step(&[Order::Command { units: vec![id], command: Command::Enter(factory_id), queue: false, formation: None }]);
    for _ in 0..600 {
        world.step(&[]);
        if world.units[0].barrel {
            break;
        }
    }
    assert!(world.units[0].barrel && world.barrels == 0, "it didn't fetch the barrel");
    world.step(&[Order::DropBarrels { units: vec![id] }]);
    assert!(!world.units[0].barrel);
    assert_eq!(barrels(&world), 1, "it didn't put it down");

    // A carrier that dies goes off.
    world.units[1].barrel = true;
    world.units[1].hp = 0;
    let at = world.units[1].pos;
    world.step(&[]);
    assert!(world.blasts.iter().any(|b| b.at == at), "the carrier didn't go off");
}

/// Not a real test – a tool for the speed of the simulation: a "Large" map (256) with ever more zombies roaming it,
/// and how long one tick takes then (it has 50 ms, at speed ×1) – roaming, and once the final wave has called them all.
/// Prints how many there are, how close to the base, and the times. In the fast build:
/// `cargo test --release horde_tick_time -- --ignored --nocapture`
#[test]
#[ignore]
fn horde_tick_time() {
    for roamers in [500.0f32, 1000.0, 1500.0, 2000.0] {
        let mut difficulty = data::get().difficulty("").clone();
        difficulty.roaming_zombies = roamers;
        let mut world = World::generate_with(7, &MapSetup::default(), &difficulty);
        let n = world.zombies.len();
        let asleep = world.sleep.len();
        let near = world.zombies.iter().filter(|z| world.way_out(z.pos) < 0.3).count();
        let ticks = 200;
        let start = std::time::Instant::now();
        for _ in 0..ticks {
            world.step(&[]);
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0 / ticks as f64;
        // The worst case: the final wave calls them all – nobody is calm anymore, everyone marches.
        world.call_everyone();
        let start = std::time::Instant::now();
        for _ in 0..ticks {
            world.step(&[]);
        }
        let all = start.elapsed().as_secs_f64() * 1000.0 / ticks as f64;
        println!("{roamers} per 1,000 open tiles: {n} zombies awake ({near} near the base), {asleep} asleep - {ms:.2} ms per tick, all called: {all:.2} ms");
    }
}

/// Far from everything the roamers sleep (`sleep.rs`). One of your units walking out there wakes the ones around it
/// before it can see them – nobody within its sight is left asleep.
#[test]
fn sleepers_wake_before_your_eyes_get_there() {
    let mut world = World::new(5);
    assert!(!world.sleep.is_empty(), "nobody sleeps");
    // A sleeper far out, and a ranger a few tiles further than it sees from it.
    let far = world.sleep.iter().map(|s| s.pos).max_by(|a, b| a.distance(world.base).total_cmp(&b.distance(world.base))).unwrap();
    let ranger = UnitKind::Ranger.stats().sight;
    let at = (far - world.base).normalize() * (far.distance(world.base) - ranger - 2.0) + world.base;
    let (awake, asleep) = (world.zombies.len(), world.sleep.len());
    world.units.clear();
    world.spawn_unit(UnitKind::Ranger, at);
    for _ in 0..sleep::WAKE_EVERY {
        world.step(&[]);
    }
    assert!(world.sleep.len() < asleep && world.zombies.len() > awake, "nobody woke");
    let seen = world.sleep.iter().filter(|s| s.pos.distance(at) <= ranger + 1.0).count();
    assert_eq!(seen, 0, "{seen} still asleep within the ranger's sight");
}

/// A sound wakes the sleepers that hear it loud enough – and they go where it was, like the zombies awake.
#[test]
fn a_loud_sound_wakes_sleepers_and_lures_them() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(40.0, 0.0);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 8);
    world.lay_to_sleep(p, Breed::Walker);
    let id = world.next_zombie_id;
    for _ in 0..sleep::WAKE_EVERY {
        world.step(&[]);
    }
    assert_eq!((world.sleep.len(), world.zombies.len()), (1, 0), "it woke without a reason");
    // Loud enough for a walker this close, every time – heard at the next listening (twice a second).
    let noise = p + vec2(5.0, 0.0);
    world.make_sound(noise, 2000);
    for _ in 0..sound::LISTEN_EVERY {
        world.step(&[]);
    }
    assert!(world.sleep.is_empty(), "the sound didn't wake it");
    let zombie = world.zombies.iter().find(|z| z.id > id).expect("it isn't awake");
    assert_eq!(zombie.lured_to(), Some(noise), "it doesn't go to the sound");
}

/// A calm stroller far from all your eyes falls asleep after a while – and wakes again with the hit points it had.
#[test]
fn calm_zombies_fall_asleep_far_from_your_eyes() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(45.0, 0.0);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 8);
    world.spawn_zombie_as(p, Breed::Walker, true);
    world.zombies[0].hp = 10;
    for _ in 0..(25.0 * TICKS_PER_SECOND) as u32 {
        world.step(&[]);
    }
    assert_eq!((world.zombies.len(), world.sleep.len()), (0, 1), "it didn't fall asleep");
    let at = world.sleep.iter().next().unwrap().pos;
    world.wake_area(at, 1.0);
    assert_eq!(world.zombies.len(), 1);
    assert_eq!(world.zombies[0].hp, 10, "it woke as a new one");
}

/// The final wave calls everyone: every sleeper wakes and joins it – a few thousand per tick, so not all at once – and
/// until the last one is awake the round can't be won.
#[test]
fn the_final_wave_wakes_every_sleeper() {
    let mut world = World::new(6);
    let asleep = world.sleep.len();
    assert!(asleep > 2000, "only {asleep} asleep");
    world.call_everyone();
    world.step(&[]);
    assert!(world.sleep.still_waking() && !world.sleep.is_empty(), "all woke at once");
    for _ in 0..asleep / 1000 + 5 {
        world.step(&[]);
    }
    assert!(world.sleep.is_empty() && !world.sleep.still_waking(), "some still sleep");
    assert!(world.zombies.iter().all(|z| z.final_wave), "a zombie didn't join the final wave");
}

/// Sleepers stroll like the strollers awake (`sleep.rs`): trip after trip they drift, never into the ring around the
/// command center – and far from your eyes they stay asleep doing it.
#[test]
fn sleepers_stroll_too() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(50.0, 0.0);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 14);
    for k in 0..30 {
        world.lay_to_sleep(p + vec2((k % 6) as f32 * 0.4, (k / 6) as f32 * 0.4), Breed::Walker);
    }
    let start: Vec<Vec2> = world.sleep.iter().map(|s| s.pos).collect();
    for _ in 0..(60.0 * TICKS_PER_SECOND) as u32 {
        world.step(&[]);
    }
    assert_eq!((world.sleep.len(), world.zombies.len()), (30, 0), "they woke without a reason");
    let moved = world.sleep.iter().filter(|s| start.iter().all(|a| a.distance(s.pos) > 1.0)).count();
    assert!(moved >= 15, "only {moved} of 30 sleepers strolled");
    let keep_out = world.keep_out.of(Breed::Walker);
    assert!(world.sleep.iter().all(|s| s.pos.distance(world.base) >= keep_out), "a sleeper strolled into the ring");
}

/// A place you've emptied fills up again: the sleepers around stroll in – and wake before they get into sight, so at no
/// time does one lie asleep where your unit sees.
#[test]
fn strolling_sleepers_fill_up_an_empty_place() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let q = world.base + vec2(45.0, 0.0);
    clear_ground(&mut world, (q.x as i32, q.y as i32), 32);
    let id = bait(&mut world, UnitKind::Ranger, q);
    let u = world.find_unit(id).unwrap();
    world.units[u].hp = 1_000_000;
    let sight = world.units[u].sight();
    // A ring of sleepers 20–28 tiles away – out of what the ranger wakes (its sight + 8).
    for k in 0..200 {
        let angle = k as f32 * 0.7;
        let r = 20.0 + (k % 9) as f32;
        world.lay_to_sleep(q + vec2(libm::cosf(angle), libm::sinf(angle)) * r, Breed::Walker);
    }
    for _ in 0..(120.0 * TICKS_PER_SECOND) as u32 {
        world.step(&[]);
        let seen = world.sleep.iter().filter(|s| s.pos.distance(q) <= sight).count();
        assert_eq!(seen, 0, "a sleeper lies in the ranger's sight at tick {}", world.tick);
    }
    let near = world.zombies.iter().filter(|z| z.pos.distance(q) < sight + 8.0).count();
    assert!(near > 0, "nobody strolled in");
}
