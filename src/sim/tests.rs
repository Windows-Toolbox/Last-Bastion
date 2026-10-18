//! Tests of the simulation. Most of them play a little scene tick by tick and look at the result.

use super::*;
use super::zombies::values;
use crate::data;
use crate::formation::{self, Formation};
use crate::map::Terrain;

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
const SCRIPTED_GAME_CHECKSUM: u64 = 0x340f_39ae_9e47_387c;

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

/// The unit at the bottom right of the start formation, in a world without zombies.
fn quiet_world() -> (World, u32) {
    let mut world = World::new(7);
    world.step(&[Order::ClearZombies]);
    let id = world.units.last().unwrap().id;
    (world, id)
}

/// A barracks on cleared ground up left of the command center (tiles x −7…−5, y −8…−6 from the base), placed like
/// in the editor – without time passing, so `tick` stays where it was. Returns its `id`.
fn barracks(world: &mut World) -> u32 {
    let tile = (world.base.x as i32 - 7, world.base.y as i32 - 8);
    let ground = (tile.1 - 2..tile.1 + 5).flat_map(|y| (tile.0 - 2..tile.0 + 5).map(move |x| (x, y))).collect();
    world.apply_orders(&[
        Order::PaintTerrain { terrain: Terrain::Grass, tiles: ground },
        Order::PlaceBuilding { kind: BuildingKind::Barracks, tile, turned: false, editor: true },
    ]);
    let b = world.buildings.last().unwrap();
    assert_eq!((b.kind, b.tile), (BuildingKind::Barracks, tile), "the barracks didn't fit");
    b.id
}

/// A wave on the timeline comes exactly at its tick, out of the spawners in the ring – not a tick earlier.
/// And the clock counts days of 24 hours from day 1.
#[test]
fn timed_wave_comes_on_time() {
    let (mut world, _) = quiet_world();
    let at = world.tick + 30;
    world.step(&[Order::AddTimedWave(TimedWave { at, count: 50, from: WaveFrom::Sides(1), breed: Breed::Runner, final_wave: false })]);
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
    let (from, to) = (world.base + vec2(2.0, 7.0), world.base + vec2(6.0, 7.0));
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
    assert!(world.to_base.reachable(0, 0), "the base behind the wall must stay reachable for zombies");
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

/// Buildings don't push anyone away: on a unit you can't build.
#[test]
fn no_building_on_units() {
    let (world, id) = quiet_world();
    let unit = &world.units[world.find_unit(id).unwrap()];
    let tile = (unit.pos.x.floor() as i32, unit.pos.y.floor() as i32);
    assert!(!world.can_place(BuildingKind::Wall, tile, false));
}

/// The editor never paints over buildings – the command center stays.
#[test]
fn paint_leaves_buildings_alone() {
    let (mut world, _) = quiet_world();
    let cc = world.buildings[0].tile;
    world.step(&[Order::PaintTerrain { terrain: Terrain::Water, tiles: vec![cc, (cc.0 + 1, cc.1 + 1)] }]);
    assert_eq!(world.map.terrain(cc.0, cc.1), Terrain::Grass);
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
        per_side[world.spawners[s].side(world.map.size) as usize] += 1;
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
    let side = world.spawners[first].side(world.map.size);
    // Along the side: x for the sides at the top and bottom of the map (0, 1), y for the left and right (2, 3).
    let along: Vec<f32> = world.zombies.iter().map(|z| if side < 2 { z.prev.x } else { z.prev.y }).collect();
    let play = world.map.play_area();
    let fifth = play.size / 5.0;
    for k in 0..5 {
        let from = play.min.x + fifth * k as f32;
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
    world.step(&[Order::PlaceSpawner { center: (20, 20) }]);
    let s = world.find_spawner(id).expect("not placed");
    assert_eq!(world.spawners[s].name, format!("Spawner {id}"));
    assert!(world.spawners[s].contains(vec2(20.5, 20.5)));
    world.step(&[
        Order::ResizeSpawner { spawner: id, min: (-5, 3), size: (4, 0) },
        Order::RenameSpawner { spawner: id, name: "  Graveyard ".to_owned() },
        Order::RenameSpawner { spawner: id, name: "   ".to_owned() },
    ]);
    let s = &world.spawners[world.find_spawner(id).unwrap()];
    assert_eq!((s.min, s.size), ((0, 3), (4, 1)));
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
    assert_eq!(world.buildings.last().unwrap().production.len(), 1);
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

/// The scene from the screenshot: a sniper behind a wall, water in front of it, zombies 8 tiles away on the
/// other side – the command center lies behind the zombies. They don't see that far (6 tiles, TAB), but the sniper
/// is loud: its first shot is heard 10 tiles around, they go around the water and storm the wall.
#[test]
fn zombies_hear_the_shot_and_storm_the_wall() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = (world.base.x as i32 - 12, world.base.y as i32);
    clear_ground(&mut world, p, 10);
    let wall: Vec<Order> = (p.1 - 4..=p.1 + 4)
        .map(|y| Order::PlaceBuilding { kind: BuildingKind::Wall, tile: (p.0 + 2, y), turned: false, editor: false })
        .collect();
    world.step(&wall);
    let water = (p.1 - 3..=p.1 + 3).map(|y| (p.0 + 5, y)).collect();
    world.step(&[Order::PaintTerrain { terrain: Terrain::Water, tiles: water }]);
    world.spawn_unit(UnitKind::Sniper, vec2(p.0 as f32 + 0.5, p.1 as f32 + 0.5));
    let sniper = world.units.last().unwrap().id;
    world.step(&[Order::Command { units: vec![sniper], command: Command::Hold, queue: false, formation: None }]);
    world.step(&[Order::SpawnZombiesAt { count: 20, point: vec2(p.0 as f32 + 8.5, p.1 as f32 + 0.5) }]);
    for _ in 0..400 {
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

/// The constant spawn as in `data\spawning.json`: nothing on day 1, from day 2 single walkers that come more and
/// more often, from day 4 groups of 4–5 runners and no walkers anymore, and after the last stage the groups keep
/// growing.
#[test]
fn constant_spawn_follows_the_stages() {
    let at = |day: u64, hour: u64| (day - 1) * DAY_TICKS + hour * HOUR_TICKS;
    let spawning = &data::get().spawning.constant;
    let group_at = |tick| spawning::group_at(spawning, tick);
    assert_eq!(group_at(at(1, 23)), None, "something on day 1");
    let early = group_at(at(2, 0)).unwrap();
    assert_eq!((early.breed, early.min, early.max), (Breed::Walker, 1, 1));
    let later = group_at(at(3, 12)).unwrap();
    assert_eq!(later.breed, Breed::Walker);
    assert!(later.every < early.every, "walkers don't come more often: {} vs {} ticks", later.every, early.every);
    let runners = group_at(at(4, 0)).unwrap();
    assert_eq!((runners.breed, runners.min, runners.max), (Breed::Runner, 4, 5));
    let last = spawning.stages.last().unwrap();
    let grown = group_at(at(last.day + 5, last.hour)).unwrap();
    assert!(grown.min > last.group[0], "groups don't grow after the last stage");

    // In the world: the first walker comes out of a "constant" spawner right at the start of day 2.
    let (mut world, _) = quiet_world();
    world.units.clear();
    world.tick = at(2, 0) - 1;
    world.step(&[]);
    assert!(world.zombies.is_empty(), "too early");
    world.step(&[]);
    assert_eq!(world.zombies.len(), 1);
    assert_eq!(world.zombies[0].breed, Breed::Walker);
    assert!(world.spawners.iter().any(|s| s.mode == SpawnMode::Constant && s.contains(world.zombies[0].prev)));
}

/// Whatever water and mountains the map generation makes: every spawner has a way to the base.
#[test]
fn every_spawner_reaches_the_base() {
    for seed in 1..=15 {
        let world = World::new(seed);
        for s in &world.spawners {
            let (min, size) = s.rect();
            let c = min + size / 2.0;
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

/// The editor's zombies: the breed asked for, and wanderers stay near where they were put – until they see a unit,
/// then they storm like everyone else.
#[test]
fn placed_zombies_have_a_breed_and_wanderers_stay() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let home = world.base + vec2(12.0, -12.0);
    clear_ground(&mut world, (home.x as i32, home.y as i32), 8);
    world.step(&[Order::PlaceZombies { center: home, radius: 2.0, count: 20, breed: Breed::Runner, wander: true }]);
    assert_eq!(world.zombies.len(), 20);
    assert!(world.zombies.iter().all(|z| z.breed == Breed::Runner && z.wander.is_some()));
    for _ in 0..400 {
        world.step(&[]);
    }
    assert_eq!(world.zombies.len(), 20, "a wanderer walked off to the base");
    assert!(world.zombies.iter().all(|z| z.pos.distance(home) < 2.0 + values().wander_radius * 1.5), "a wanderer strayed too far");
    assert!(world.zombies.iter().any(|z| z.pos.distance(z.prev) > 0.0 || z.wander.unwrap().goal.is_some()), "nobody wanders");

    // A soldier (that doesn't shoot – no noise) appears in their middle: whoever sees it storms, whoever stands
    // further away than a runner sees (6 tiles, TAB) keeps wandering.
    world.spawn_unit(UnitKind::Soldier, home);
    world.units[0].cooldown = u32::MAX;
    world.step(&[]);
    world.step(&[]);
    let sight = Breed::Runner.stats().sight;
    let near: Vec<&Zombie> = world.zombies.iter().filter(|z| z.pos.distance(home) < sight - 1.0).collect();
    assert!(!near.is_empty());
    assert!(near.iter().all(|z| z.wander.is_none()), "still wandering with a soldier in sight");
    let far = world.zombies.iter().filter(|z| z.pos.distance(home) > sight + 1.0);
    assert!(far.into_iter().all(|z| z.wander.is_some()), "noticed a soldier it can't see");
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

/// Noise like in TAB: a shot wakes every zombie within the shooter's `noise`. A wanderer 9 tiles behind the shooter
/// (too far to see it) hears a sniper (10) – but not a ranger (1).
#[test]
fn loud_shots_wake_zombies_quiet_ones_dont() {
    let woken = |kind: UnitKind| {
        let (mut world, _) = quiet_world();
        world.units.clear();
        let p = world.base + vec2(12.0, -12.0);
        clear_ground(&mut world, (p.x as i32, p.y as i32), 10);
        world.spawn_unit(kind, p);
        // The target: right in front, standing still and too tough to die. Behind: the wanderer.
        world.spawn_zombie_as(p + vec2(5.0, 0.0), Breed::Walker, false);
        world.spawn_zombie_as(p + vec2(-9.0, 0.0), Breed::Walker, true);
        for z in &mut world.zombies {
            z.speed = 0.0;
            z.hp = 100_000;
        }
        let mut shot = false;
        for _ in 0..60 {
            world.step(&[]);
            shot |= !world.shots.is_empty();
        }
        assert!(shot, "{kind:?} never shot");
        world.zombies[1].wander.is_none()
    };
    assert!(woken(UnitKind::Sniper), "the sniper's shot wasn't heard 9 tiles away");
    assert!(!woken(UnitKind::Ranger), "the ranger's arrow was heard 9 tiles away");
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
        // 250, not 200: with 200 the pile came out at 1.9997 tiles, right at the limit – it depends on where
        // exactly they were scattered.
        Order::SpawnZombiesAt { count: 250, point: world.base + vec2(22.0, 0.5) },
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
    // The pile in front of the gap is wide – before, they squeezed together on the line to the gap.
    let in_front = |p: &&Vec2| p.x > world.base.x + 12.0 && p.x < world.base.x + 16.0;
    let widest = zombies.iter().filter(in_front).map(|p| (p.y - world.base.y).abs()).fold(0.0, f32::max);
    assert!(widest > 2.0, "pile only {widest} tiles to each side");
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

/// A new round: zombies roam the map, all of them wandering, none right at the base, more further out. Near the
/// base only walkers, further out runners too, at the edge also fat ones (`roaming` in `data\zombies.json`).
#[test]
fn roamers_are_scattered_by_distance() {
    let world = World::new(3);
    let roaming = &values().roaming;
    assert!(world.zombies.len() > 500, "only {} zombies", world.zombies.len());
    assert!(world.zombies.iter().all(|z| z.wander.is_some()));
    let half = world.map.play_area().size / 2.0;
    let out = |z: &Zombie| (z.pos.distance(world.base) / half).min(1.0);
    assert!(world.zombies.iter().all(|z| z.pos.distance(world.base) >= roaming.keep_out), "a zombie right at the base");
    let near = world.zombies.iter().filter(|z| out(z) < 0.25).count();
    let far = world.zombies.len() - near;
    assert!(near > 0 && far > near * 4, "near {near}, far {far}");
    for z in &world.zombies {
        match z.breed {
            Breed::Runner => assert!(out(z) >= roaming.runners_from, "a runner too close: {}", out(z)),
            Breed::Fat => assert!(out(z) >= roaming.fat_from, "a fat one too close: {}", out(z)),
            Breed::Walker => {}
        }
    }
    for breed in Breed::ALL {
        assert!(world.zombies.iter().any(|z| z.breed == breed), "no {breed:?} roams");
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

/// Zombies hit a tent's shield first. When it breaks, the tent is infected: exactly its 6 colonists come out as
/// zombies – 2 to 4 runners, the rest walkers – and only after that does it lose hit points.
#[test]
fn a_tent_whose_shield_breaks_lets_out_six_zombies() {
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
        assert_eq!(world.zombies.len(), 1 + 6, "seed {seed}");
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
    assert_eq!(world.zombies.len(), 6);
    assert!(!world.map.is_blocked(t.0, t.1) && !world.map.can_tear_down(t.0, t.1));
    for _ in 0..200 {
        world.step(&[]);
    }
    let tent = &world.buildings[b];
    assert_eq!(tent.hp, tent.stats().hp, "the zombies hit their own tent");
}

/// Every hour of the round (7.5 s) a tent pays `gold_per_colonist_per_hour` (`data\colony.json`) for each of its 6
/// colonists. An infected tent pays nothing.
#[test]
fn tents_pay_gold_every_hour() {
    let (mut world, _) = quiet_world();
    let t = (world.base.x as i32 + 8, world.base.y as i32 - 8);
    clear_ground(&mut world, t, 6);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tent, tile: t, turned: false, editor: false }]);
    let per_hour = BuildingKind::Tent.stats().colonists * data::get().colony.gold_per_colonist_per_hour;
    assert!(per_hour > 0);
    assert_eq!(world.income(), per_hour);
    let start = world.gold;
    // In any 2 hours in a row the clock passes exactly two full hours.
    for _ in 0..2 * HOUR_TICKS {
        world.step(&[]);
    }
    assert_eq!(world.gold - start, 2 * per_hour);

    let b = world.buildings.len() - 1;
    let shield = world.buildings[b].shield;
    assert!(world.buildings[b].take_damage(shield), "breaking the shield doesn't infect");
    assert_eq!(world.income(), 0);
    let start = world.gold;
    for _ in 0..2 * HOUR_TICKS {
        world.step(&[]);
    }
    assert_eq!(world.gold, start, "an infected tent still pays");
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
    assert_eq!(world.income(), 0);
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
    assert!(world.income() > 0);
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

/// Won: once the final wave has come and its last zombie is dead – other zombies still around don't matter.
#[test]
fn killing_the_final_wave_wins_the_round() {
    let (mut world, _) = quiet_world();
    world.waves.clear();
    let at = world.tick + 5;
    world.step(&[
        Order::AddTimedWave(TimedWave { at, count: 3, from: WaveFrom::Sides(1), breed: Breed::Runner, final_wave: true }),
        Order::SpawnWave { count: 10, from: WaveFrom::Sides(1) },
    ]);
    while world.tick <= at {
        assert_eq!(world.outcome, None, "won before the final wave came");
        world.step(&[]);
    }
    assert_eq!(world.zombies.iter().filter(|z| z.final_wave).count(), 3);
    world.step(&[]);
    assert_eq!(world.outcome, None, "won with the final wave alive");
    for zombie in world.zombies.iter_mut().filter(|z| z.final_wave) {
        zombie.hp = 0;
    }
    world.step(&[]);
    assert!(matches!(world.outcome, Some(Outcome::Won { .. })), "{:?}", world.outcome);
    assert_eq!(world.zombies.len(), 10, "the other zombies are gone too");
    assert_eq!(world.score.zombies_killed, 3);
}

/// A new round plans its waves from its days: the mini round (10 days) gets two waves (day 4 and 7) and the final wave
/// on day 10 from all sides, with fat ones. 40 days get twelve, growing. "Waves" 200 % doubles every count.
#[test]
fn waves_are_planned_from_the_days() {
    let mini = plan_waves(10, 1.0);
    let normal: Vec<&TimedWave> = mini.iter().filter(|w| !w.final_wave).collect();
    let days: Vec<u64> = normal.iter().map(|w| day_and_hour(w.at).0).collect();
    assert_eq!(days, vec![4, 7]);
    assert!(mini.iter().filter(|w| w.final_wave).all(|w| day_and_hour(w.at).0 == 10 && w.from == WaveFrom::Sides(4)));
    assert!(mini.iter().any(|w| w.final_wave && w.breed == Breed::Fat), "no fat ones in the final wave");

    let long = plan_waves(40, 1.0);
    let normal: Vec<&TimedWave> = long.iter().filter(|w| !w.final_wave).collect();
    assert_eq!(normal.len(), 12);
    assert!(normal.windows(2).all(|pair| pair[0].at < pair[1].at && pair[0].count <= pair[1].count), "not growing");
    assert!(plan_waves(0, 1.0).is_empty());
    assert_eq!(plan_waves(10, 2.0)[0].count, mini[0].count * 2);

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
/// of them (the command center, 3 × 3, by one corner).
#[test]
fn the_eraser_takes_everything_off_its_tiles() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    // The start units stand in rows below the base, on the tiles bx - 2 … bx + 3 and by + 3 … by + 5.
    let center = vec2(bx as f32 + 2.5, by as f32 + 4.5);
    world.apply_orders(&[Order::PlaceZombies { center, radius: 0.4, count: 3, breed: Breed::Walker, wander: true }]);
    let mut tiles = vec![(bx + 1, by + 1)];
    for y in by + 3..=by + 5 {
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
/// With `$env:WAVES = "3"` first, the waves are that strong (the setup's "Waves": 3 = 300 %).
#[test]
#[ignore]
fn balance_mini_round() {
    let mut setup = MapSetup::default();
    if let Some(waves) = std::env::var("WAVES").ok().and_then(|w| w.parse().ok()) {
        setup.round.waves = waves;
    }
    for simple_player in [false, true] {
        let mut world = World::generate(1, &setup);
        println!("--- {} ---", if simple_player { "simple player" } else { "player does nothing" });
        let end = round::tick_of(world.days + 3, 0);
        while world.tick < end && world.outcome.is_none() {
            let orders = if simple_player && world.tick % HOUR_TICKS == 0 { simple_orders(&world) } else { Vec::new() };
            world.step(&orders);
            if world.tick % (12 * HOUR_TICKS) == 0 {
                let (day, hour) = world.clock();
                let cc = world.buildings.iter().find(|b| b.kind == BuildingKind::CommandCenter).map_or(0, |b| b.hp);
                let tents = world.buildings.iter().filter(|b| b.kind == BuildingKind::Tent && !b.infected).count();
                println!(
                    "day {day:2} {hour:02}:00  zombies {:5}  units {:3}  tents {tents:2}  gold {:5} (+{}/h)  CC {cc}",
                    world.zombies.len(),
                    world.units.len(),
                    world.gold,
                    world.income()
                );
            }
        }
        println!("{:?}  {:?}", world.outcome, world.score);
    }
}

/// The simple player of `balance_mini_round`: a tent on the first free spot around the base while there are fewer
/// than 12, then a barracks, then a soldier – if the gold is there.
fn simple_orders(world: &World) -> Vec<Order> {
    if !world.buildings.iter().any(|b| b.kind == BuildingKind::CommandCenter) {
        return Vec::new();
    }
    let tents = world.buildings.iter().filter(|b| b.kind == BuildingKind::Tent).count();
    let barracks = world.buildings.iter().find(|b| b.kind == BuildingKind::Barracks);
    let build = match (tents < 12, barracks) {
        (true, _) => BuildingKind::Tent,
        (false, None) => BuildingKind::Barracks,
        (false, Some(barracks)) if world.gold >= UnitKind::Soldier.stats().cost => {
            return vec![Order::Train { building: barracks.id, kind: UnitKind::Soldier }];
        }
        (false, Some(_)) => return Vec::new(),
    };
    if world.gold < build.stats().cost {
        return Vec::new();
    }
    // The first free spot on rings around the base, in steps as wide as the building (tent 2, barracks 3).
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let step = build.stats().size.0 as usize;
    for r in (4..30).step_by(step) {
        for dy in (-r..=r).step_by(step) {
            for dx in (-r..=r).step_by(step) {
                let tile = (bx + dx, by + dy);
                if (dx.abs() == r || dy.abs() == r) && world.can_place(build, tile, false) {
                    return vec![Order::PlaceBuilding { kind: build, tile, turned: false, editor: false }];
                }
            }
        }
    }
    Vec::new()
}
