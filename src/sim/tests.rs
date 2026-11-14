//! Tests of the simulation. Most of them play a little scene tick by tick and look at the result.

use std::collections::HashMap;

use super::*;
use super::zombies::{values, Aim, Doing, Hit};
use super::crowd;
use crate::data;
use crate::formation::{self, Formation};
use crate::kinds::Resource;
use crate::map::{to_frame, Deposit, Map, Ore, Terrain};
use crate::mapgen::Generator;

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
const SCRIPTED_GAME_CHECKSUM: u64 = 0x4c73_3046_2f23_4df3;

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

/// How many of `breed` an event brings – per side, all its groups together (the low end of every range).
fn breed_count(event: &Event, breed: Breed) -> u32 {
    event.groups.iter().flat_map(|g| &g.breeds).filter(|z| z.breed == breed).map(|z| z.count).sum()
}

/// An event of one group that comes once at hour `hour`: these breeds (how many of each) out of `from`.
fn one_group(hour: u32, from: WaveFrom, zombies: &[(Breed, u32)]) -> Event {
    let breeds = zombies.iter().map(|&(breed, count)| BreedCount::new(count, breed)).collect();
    Event { groups: vec![WaveGroup { from: from.clone(), breeds }], ..Event::new("Test", hour, from, Breed::Runner, 0) }
}

/// The next full hour from now – an event at it comes a little later.
fn next_hour(world: &World) -> u32 {
    (world.tick / HOUR_TICKS) as u32 + 1
}

/// Steps until the event due at `hour` has come and every one of its zombies is out of the spawners (a few at a time,
/// like TAB's, `events.rs`). Returns where each appeared – its breed and `prev` in the tick it came out (in that tick
/// it already took its first step) – in the order they came.
fn step_until_it_comes(world: &mut World, hour: u32) -> Vec<(Breed, Vec2)> {
    let at = hour as u64 * HOUR_TICKS;
    let mut appeared = Vec::new();
    loop {
        let last = world.next_zombie_id;
        world.step(&[]);
        appeared.extend(world.zombies.iter().filter(|z| z.id > last).map(|z| (z.breed, z.prev)));
        if world.tick > at && world.requests.is_empty() {
            return appeared;
        }
    }
}

/// The unit at the bottom right of the start formation, in a world without zombies – none roaming, and none waiting in
/// the villages of doom or around the loot either (the eraser takes those away).
fn quiet_world() -> (World, u32) {
    quiet_world_of(7)
}

/// `quiet_world` with another seed.
fn quiet_world_of(seed: u64) -> (World, u32) {
    let mut world = World::new(seed);
    // Research is its own test (`research_unlocks_and_goes_once`) – here everything is researched.
    world.research_all();
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

/// An event comes at its hour, out of the spawners in the ring – not a tick earlier. Its zombies come out a few at a
/// time, like TAB's: the first burst `stream_speed` of them. And the clock counts days of 24 hours from day 1.
#[test]
fn an_event_comes_on_time() {
    let (mut world, _) = quiet_world();
    world.events.clear();
    let hour = next_hour(&world) + 1;
    world.step(&[Order::AddEvent(Event::new("Test", hour, WaveFrom::Sides(1), Breed::Runner, 50))]);
    while world.tick < hour as u64 * HOUR_TICKS {
        assert!(world.zombies.is_empty(), "too early, tick {}", world.tick);
        world.step(&[]);
    }
    while world.zombies.is_empty() {
        world.step(&[]);
    }
    assert_eq!(world.zombies.len() as u32, data::get().spawning.stream_speed, "not a burst");
    let appeared = step_until_it_comes(&mut world, hour);
    assert_eq!(world.zombies.len(), 50);
    assert!(appeared.iter().all(|&(_, p)| !world.map.in_play(p.x as i32, p.y as i32)));
    assert_eq!(day_and_hour(0), (1, 0));
    assert_eq!(day_and_hour(DAY_TICKS + 3 * HOUR_TICKS + 5), (2, 3));
    assert_eq!(DAY_TICKS, 60 * 20, "a day isn't TAB's 60 seconds");
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
    world.step(&[Order::Command { units: vec![id], command: Command::Attack(Prey::Zombie(zombie)), queue: false, formation: None }]);
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

/// Zombies wind up like your units (TAB's `TimeAction × TimeFactorAction`): a walker's blow lands `windup_seconds`
/// after its swing starts – on a unit that stayed, not on one that got out of its reach meanwhile (kiting).
#[test]
fn a_zombie_winds_up_and_misses_who_walked_off() {
    let windup = Breed::Walker.stats().windup;
    assert!(windup >= 2, "the walker needs a wind-up for this test");
    for walks_off in [false, true] {
        let (mut world, id) = quiet_world();
        no_more_zombies(&mut world);
        // Alone, away from the others – the walker goes for the nearest.
        let far = world.base + vec2(8.0, 8.0);
        world.step(&[Order::Command { units: vec![id], command: Command::Move(far), queue: false, formation: None }]);
        for _ in 0..200 {
            world.step(&[]);
        }
        world.step(&[Order::Command { units: vec![id], command: Command::Hold, queue: false, formation: None }]);
        let at = world.units[world.find_unit(id).unwrap()].pos;
        world.spawn_zombie_as(at + vec2(0.3, 0.0), Breed::Walker, true);
        let zombie = world.zombies.last().unwrap().id;
        let swinging = |w: &World| w.find_zombie(zombie).is_some_and(|z| w.zombies[z].swing.is_some());
        let mut ticks = 0;
        while !swinging(&world) {
            world.step(&[]);
            ticks += 1;
            assert!(ticks < 100, "the walker never swung");
        }
        let u = world.find_unit(id).unwrap();
        let hp = world.units[u].hp;
        if walks_off {
            world.units[u].pos += vec2(0.0, 1.0);
        }
        for _ in 1..windup {
            world.step(&[]);
            assert_eq!(world.units[world.find_unit(id).unwrap()].hp, hp, "hit before the end of the wind-up");
        }
        world.step(&[]);
        let after = world.units[world.find_unit(id).unwrap()].hp;
        match walks_off {
            false => assert!(after < hp, "the blow didn't land at the end of the wind-up"),
            true => assert_eq!(after, hp, "hit although it walked out of reach"),
        }
    }
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
        Order::PaintTerrain { terrain: Terrain::Forest, tiles: trees.clone() },
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
    shot |= !world.shots.is_empty();
    // It looks around (`search_seconds`), brings its rifle up and winds up – then it shoots.
    for _ in 0..20 {
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

/// TAB's way through walls: a soldier inside walls with a gap, zombies on one side see it – they go straight for it and
/// tear down the walls in their way (`wall_in_way`). Nobody goes around to the gap: the walls on the other half stay
/// whole, and the soldier stays unhurt (the walls here don't break).
#[test]
fn zombies_tear_down_the_wall_in_their_way_and_ignore_the_gap() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let fort = (world.base.x as i32 + 6, world.base.y as i32 - 6);
    clear_ground(&mut world, fort, 9);
    world.step(&wall_ring(fort, 2, &[(fort.0, fort.1 - 2)])); // gap at the top
    for b in world.buildings.iter_mut().filter(|b| b.kind == BuildingKind::Wall) {
        b.hp = 1_000_000;
    }
    let inside = vec2(fort.0 as f32 + 0.5, fort.1 as f32 + 0.5);
    let soldier = bait(&mut world, UnitKind::Soldier, inside);
    world.step(&[Order::PlaceZombies { center: inside + vec2(5.0, 0.0), radius: 1.0, count: 40, breed: Breed::Runner, wander: true }]);
    for _ in 0..300 {
        world.step(&[]);
    }
    let damaged: Vec<(i32, i32)> =
        world.buildings.iter().filter(|b| b.kind == BuildingKind::Wall && b.hp < 1_000_000).map(|b| b.tile).collect();
    assert!(!damaged.is_empty(), "no wall torn at");
    assert!(damaged.iter().all(|t| t.0 > fort.0), "a wall away from the zombies was hit: {damaged:?}");
    let unit = &world.units[world.find_unit(soldier).unwrap()];
    assert_eq!(unit.hp, unit.stats().hp, "a zombie got around to the gap");
}

/// TAB's wall rule (`wall_in_way`), looked at directly: a zombie whose straight way to a unit crosses a wall goes for
/// that wall – a harpy doesn't, it leaps over walls. With the way clear it goes for the unit. Walking diagonally between
/// two walls that touch at the corner, it takes one of them.
#[test]
fn the_wall_in_the_way_becomes_the_target() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let c = (world.base.x as i32 + 12, world.base.y as i32 - 12);
    clear_ground(&mut world, c, 8);
    let wall: Vec<Order> = (c.1 - 2..=c.1 + 2)
        .map(|y| Order::PlaceBuilding { kind: BuildingKind::Wall, tile: (c.0 + 2, y), turned: false, editor: false })
        .collect();
    world.step(&wall);
    let soldier = bait(&mut world, UnitKind::Soldier, vec2(c.0 as f32 + 4.5, c.1 as f32 + 0.5));
    world.spawn_zombie_as(vec2(c.0 as f32 + 0.5, c.1 as f32 + 0.5), Breed::Runner, true);
    world.spawn_zombie_as(vec2(c.0 as f32 + 0.5, c.1 as f32 + 1.5), Breed::Harpy, true);
    world.spawn_zombie_as(vec2(c.0 as f32 + 4.5, c.1 as f32 + 4.5), Breed::Runner, true);
    for z in &mut world.zombies {
        z.speed = 0.0;
    }
    world.step(&[]);
    let aim = |world: &World, breed: Breed, y: f32| {
        let z = world.zombies.iter().find(|z| z.breed == breed && (z.pos.y - (c.1 as f32 + y)).abs() < 0.3).unwrap();
        world.watch(z)
    };
    let in_the_way = world.buildings.iter().find(|b| b.tile == (c.0 + 2, c.1)).unwrap().id;
    assert_eq!(aim(&world, Breed::Runner, 0.5), Some(Aim::Building(in_the_way)), "it doesn't go for the wall in its way");
    assert_eq!(aim(&world, Breed::Harpy, 1.5), Some(Aim::Unit(soldier)), "the harpy goes for the wall");
    assert_eq!(aim(&world, Breed::Runner, 4.5), Some(Aim::Unit(soldier)), "a clear way, but it goes for a wall");

    // Two walls touching at the corner, the zombie going diagonally between them: one of the two.
    let d = (c.0 - 4, c.1 - 4);
    world.step(&[
        Order::PlaceBuilding { kind: BuildingKind::Wall, tile: (d.0 + 1, d.1), turned: false, editor: true },
        Order::PlaceBuilding { kind: BuildingKind::Wall, tile: (d.0, d.1 + 1), turned: false, editor: true },
    ]);
    let z = world.zombies.iter().find(|z| z.breed == Breed::Runner).unwrap().clone();
    let mut diagonal = z.clone();
    diagonal.pos = vec2(d.0 as f32 + 0.5, d.1 as f32 + 0.5);
    let wall = world.wall_in_way(&diagonal, vec2(d.0 as f32 + 1.5, d.1 as f32 + 1.5), 1).map(|b| world.buildings[b].tile);
    assert!(wall == Some((d.0 + 1, d.1)) || wall == Some((d.0, d.1 + 1)), "between two walls it found {wall:?}");
}

/// Shot like in TAB (`zombie_hit_by`): the zombie and every zombie on its tile and the four next to it that isn't after
/// anything go for the shooter – one corner to corner or further away doesn't. One that's after something already
/// doesn't turn.
#[test]
fn a_shot_zombie_and_the_ones_right_around_it_go_for_the_shooter() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    // `base` is the middle of a tile – so is `p`.
    let p = world.base + vec2(20.0, 0.0);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 6);
    for offset in [vec2(0.0, 0.0), vec2(0.2, 0.1), vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(3.0, 0.0)] {
        world.spawn_zombie_as(p + offset, Breed::Walker, true);
    }
    for z in &mut world.zombies {
        z.speed = 0.0;
    }
    let ids: Vec<u32> = world.zombies.iter().map(|z| z.id).collect();
    world.step(&[]);
    let index = |world: &World, k: usize| world.zombies.iter().position(|z| z.id == ids[k]).unwrap();
    let shooter = Aim::Unit(999);
    let z = index(&world, 0);
    world.zombie_hit_by(z, shooter);
    let aims: Vec<Option<Aim>> = (0..5).map(|k| world.zombies[index(&world, k)].aim).collect();
    assert_eq!(aims, vec![Some(shooter), Some(shooter), Some(shooter), None, None]);
    // Already after something: it stays on that.
    let far = index(&world, 4);
    world.zombies[far].aim = Some(Aim::Unit(5));
    world.zombie_hit_by(far, shooter);
    assert_eq!(world.zombies[far].aim, Some(Aim::Unit(5)));
}

/// What zombies go for (TAB's `HasPeople`): a tent with colonists in it in sight – not a wall or an empty tower right
/// next to them. Idle walkers between a lone wall, an empty tower and a tent tear at the tent.
#[test]
fn zombies_go_for_buildings_with_people_not_walls() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let c = (world.base.x as i32 + 14, world.base.y as i32 - 14);
    clear_ground(&mut world, c, 8);
    // Like in the editor: the tent is out of reach of the command center's power.
    world.step(&[
        Order::PlaceBuilding { kind: BuildingKind::Wall, tile: (c.0 - 3, c.1), turned: false, editor: true },
        Order::PlaceBuilding { kind: BuildingKind::Tower, tile: (c.0, c.1 - 3), turned: false, editor: true },
        Order::PlaceBuilding { kind: BuildingKind::Tent, tile: (c.0 + 3, c.1), turned: false, editor: true },
    ]);
    let kinds: Vec<BuildingKind> = world.buildings.iter().map(|b| b.kind).collect();
    assert_eq!(kinds, vec![BuildingKind::CommandCenter, BuildingKind::Wall, BuildingKind::Tower, BuildingKind::Tent]);
    world.step(&[Order::PlaceZombies { center: vec2(c.0 as f32 - 1.5, c.1 as f32 + 0.5), radius: 0.5, count: 5, breed: Breed::Walker, wander: true }]);
    for _ in 0..500 {
        world.step(&[]);
    }
    let whole = |b: &Building| b.hp == b.stats().hp && b.shield == b.stats().max_shield();
    assert!(whole(&world.buildings[1]), "they tore at the wall");
    assert!(whole(&world.buildings[2]), "they tore at the empty tower");
    assert!(!whole(&world.buildings[3]), "they left the tent alone");
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

/// An event of two groups (editor): 30 runners spread over two chosen spawners, 10 spitters out of a third – at the
/// same time, each group out of its own spawners, whatever they're set to. Announced: one swarm per group.
#[test]
fn an_event_has_groups_out_of_chosen_spawners() {
    let (mut world, _) = quiet_world();
    world.events.clear();
    let ids: Vec<u32> = world.spawners.iter().map(|s| s.id).collect();
    // `N 1` and `N 2` lie on the same side, `S 3` across.
    assert_eq!((world.spawners[0].side(&world.map), world.spawners[1].side(&world.map), world.spawners[5].side(&world.map)), (0, 0, 1));
    let hour = next_hour(&world);
    let mut event = one_group(hour, WaveFrom::Spawners(vec![ids[0], ids[1]]), &[(Breed::Runner, 30)]);
    event.groups.push(WaveGroup { from: WaveFrom::Spawners(vec![ids[5]]), breeds: vec![BreedCount::new(10, Breed::Spitter)] });
    event.notify_hours = Some(0);
    let off: Vec<Order> = ids.iter().map(|&spawner| Order::SetSpawnMode { spawner, mode: SpawnMode::Off }).collect();
    world.step(&off);
    world.step(&[Order::AddEvent(event)]);
    let appeared = step_until_it_comes(&mut world, hour);
    assert_eq!(world.zombies.len(), 40);
    let from = |breed: Breed| -> Vec<usize> {
        let mut found: Vec<usize> = appeared.iter().filter(|a| a.0 == breed).filter_map(|a| spawner_of(&world, a.1)).collect();
        found.sort();
        found.dedup();
        found
    };
    assert_eq!(from(Breed::Runner), vec![0, 1], "the runners don't come out of both chosen spawners");
    assert_eq!(from(Breed::Spitter), vec![5]);
    assert_eq!(world.swarms().iter().map(|&(_, n)| n).collect::<Vec<_>>(), vec![30, 10], "not one swarm per group");
}

/// A spawner that is removed is taken out of every event that named it – the others stay.
#[test]
fn a_removed_spawner_leaves_the_events() {
    let (mut world, _) = quiet_world();
    let ids: Vec<u32> = world.spawners.iter().map(|s| s.id).collect();
    world.step(&[Order::AddEvent(one_group(500, WaveFrom::Spawners(vec![ids[0], ids[1]]), &[(Breed::Fat, 5)]))]);
    world.step(&[Order::RemoveSpawner { spawner: ids[0] }]);
    assert_eq!(world.events.last().unwrap().groups[0].from, WaveFrom::Spawners(vec![ids[1]]));
}

/// A group out of regions (`WaveFrom::Zones`): its zombies come out on tiles of the region – a ring (circle minus
/// circle) leaves its hole empty. A removed region leaves the group (nobody comes then), the region says who uses it.
#[test]
fn a_group_comes_out_of_a_region() {
    use crate::sim::{Form, Shape, Zone};
    let (mut world, _) = quiet_world();
    world.units.clear();
    world.events.clear();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let center = (bx + 14, by + 14);
    clear_ground(&mut world, center, 8);
    let mut ring = Zone::new("Ring", 1, Form::Circle { center, radius: 7 });
    ring.shapes.push(Shape { form: Form::Circle { center, radius: 3 }, cut: true });
    let id = world.next_zone_id();
    world.step(&[Order::AddZone { zone: ring }]);
    let hour = next_hour(&world);
    let mut event = one_group(hour, WaveFrom::Zones(vec![id]), &[(Breed::Runner, 60)]);
    event.name = "Ring wave".into();
    event.notify_hours = Some(0);
    world.step(&[Order::AddEvent(event)]);
    assert_eq!(world.zone_uses(id), vec!["Ring wave (day 1), group 1".to_owned()]);
    let appeared = step_until_it_comes(&mut world, hour);
    assert_eq!(appeared.len(), 60, "not all of them came");
    let zone = world.zones[0].clone();
    for (_, p) in &appeared {
        assert!(zone.contains((p.x.floor() as i32, p.y.floor() as i32)), "a zombie came out at {p}, outside the region");
    }
    // Once the region is gone nobody comes: the group names none.
    world.step(&[Order::RemoveZone { zone: id }]);
    assert_eq!(world.events[0].groups[0].from, WaveFrom::Zones(Vec::new()));
    assert!(world.zone_uses(id).is_empty() || world.zones.is_empty());
}

/// The map file keeps a group's regions (format 7), and an event knows how many zombies it brings in all.
#[test]
fn regions_in_a_group_are_saved_and_events_count_their_zombies() {
    let (mut world, _) = quiet_world();
    world.events.clear();
    let mut event = one_group(24, WaveFrom::Zones(vec![3, 4]), &[(Breed::Runner, 100)]);
    event.groups.push(WaveGroup { from: WaveFrom::Sides(2), breeds: vec![BreedCount { count: 10, up_to: 30, breed: Breed::Fat }] });
    world.step(&[Order::AddEvent(event.clone())]);
    let file = crate::mapfile::MapFile::from_world(&world, "Test");
    let text = serde_json::to_string(&file).unwrap();
    let back: crate::mapfile::MapFile = serde_json::from_str(&text).unwrap();
    assert_eq!(back.events[0].groups[0].from, WaveFrom::Zones(vec![3, 4]));
    // Once: 100 out of the regions + 2 sides of about 20.
    assert_eq!(event.total(100 * 24), 140);
    // Every 24 hours, 3 times, +50 % each: 140, 210, 280.
    let growing = Event { every_hours: 24, times: 3, growth: 0.5, ..event.clone() };
    assert_eq!(growing.total(100 * 24), 140 + 210 + 280);
    // Only what comes until the end of the round counts: the second time is after day 2.
    assert_eq!(growing.total(30), 140);
}

/// An event with a spread (TAB's `MaxCellDispersionGeneration`): every side's zombies come out of one square that big,
/// not along the whole side – and all its breeds out of the same square.
#[test]
fn a_spread_event_comes_out_of_a_square() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    world.events.clear();
    let hour = next_hour(&world);
    let event = Event { dispersion: 6, ..one_group(hour, WaveFrom::Sides(1), &[(Breed::Runner, 40), (Breed::Fat, 5)]) };
    world.step(&[Order::AddEvent(event)]);
    let appeared = step_until_it_comes(&mut world, hour);
    assert_eq!(appeared.len(), 45);
    let spread = appeared.iter().flat_map(|a| appeared.iter().map(move |b| a.1.distance(b.1))).fold(0.0, f32::max);
    // A square of 6 tiles: at most its diagonal apart (and the spawner may be thinner).
    assert!(spread <= 6.0 * std::f32::consts::SQRT_2 + 0.5, "spread over {spread} tiles");
}

/// Right click, "slanted": a spawner can lie along the tiles instead of straight on screen – around the same middle,
/// about as big, turned by 45°. Zombies come out inside it. Straight again, and it's as it was.
#[test]
fn a_spawner_can_lie_slanted() {
    let (mut world, _) = quiet_world();
    let id = world.spawners[1].id;
    let (center, min, size) = (world.spawners[1].center(), world.spawners[1].min, world.spawners[1].size);
    world.step(&[Order::SetSpawnerAxes { spawner: id, axes: Axes::Tiles }]);
    let s = world.spawners[1].clone();
    assert_eq!(s.axes, Axes::Tiles);
    assert!(s.center().distance(center) < 1.5, "it moved: {} to {}", center, s.center());
    // A step of the screen axes is half a tile across: as many tiles, give or take the rounding.
    let (before, after) = (size.0 * size.1 / 2, s.size.0 * s.size.1);
    assert!((after - before).abs() * 10 < before, "{before} tiles before, {after} slanted");
    assert!(s.contains(s.center()) && !s.contains(s.center() + vec2(0.0, s.size.1 as f32)));

    world.events.clear();
    let hour = next_hour(&world);
    world.step(&[Order::AddEvent(one_group(hour, WaveFrom::Spawners(vec![id]), &[(Breed::Runner, 40)]))]);
    let appeared = step_until_it_comes(&mut world, hour);
    assert_eq!(world.zombies.len(), 40);
    assert!(appeared.iter().all(|a| s.contains(a.1)), "a zombie came out outside the slanted spawner");

    world.step(&[Order::SetSpawnerAxes { spawner: id, axes: Axes::Screen }]);
    let back = &world.spawners[1];
    assert_eq!(back.axes, Axes::Screen);
    let near = |a: (i32, i32), b: (i32, i32)| (a.0 - b.0).abs() <= 1 && (a.1 - b.1).abs() <= 1;
    assert!(near(back.min, min) && near(back.size, size), "{:?} {:?} instead of {min:?} {size:?}", back.min, back.size);
}

/// Demolish (Del, like TAB): a building torn down is gone at once, `demolish_refund_percent` of its price comes back –
/// of one still being built `demolish_unfinished_refund_percent` (TAB: 50 % both, its `FactorResourcesReturn`). Not the command center, not an infected building.
#[test]
fn demolishing_gives_part_of_the_price_back() {
    let (mut world, _) = quiet_world();
    no_more_zombies(&mut world);
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let t = (bx + 6, by - 8);
    clear_ground(&mut world, t, 4);
    let percent = (data::get().buildings.demolish_refund_percent, data::get().buildings.demolish_unfinished_refund_percent);
    assert!(percent.0 > 0 && percent.0 <= percent.1 && percent.1 < 100, "data\\buildings.json changed – change this test too");
    world.gold = 100;
    world.stock.set(Resource::Wood, 0);
    world.stock.set(Resource::Stone, 0);
    // The editor's barracks costs nothing and stands at once – torn down it gives its share of the price all the same.
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::Barracks, tile: t, turned: false, editor: true }]);
    let barracks = world.buildings.last().unwrap().id;
    world.apply_orders(&[Order::Demolish { buildings: vec![barracks] }]);
    assert!(world.find_building(barracks).is_none(), "still there");
    let s = BuildingKind::Barracks.stats();
    assert_eq!(world.gold, 100 + s.cost * percent.0 / 100);
    assert_eq!((world.stock.get(Resource::Wood), world.stock.get(Resource::Stone)), (s.cost_wood * percent.0 / 100, s.cost_stone * percent.0 / 100));

    // A tent still being built: its own share comes back.
    world.instant_build = false;
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::Tent, tile: t, turned: false, editor: false }]);
    let tent = world.buildings.last().unwrap();
    assert!(tent.kind == BuildingKind::Tent && !tent.is_built(), "the tent isn't being built");
    let (tent, gold) = (tent.id, world.gold);
    world.apply_orders(&[Order::Demolish { buildings: vec![tent] }]);
    assert_eq!(world.gold, gold + BuildingKind::Tent.stats().cost * percent.1 / 100);

    // Neither the command center nor an infected building.
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::Tent, tile: t, turned: false, editor: true }]);
    let b = world.buildings.len() - 1;
    world.buildings[b].infected = true;
    let infected = world.buildings[b].id;
    let cc = world.buildings.iter().find(|b| b.kind == BuildingKind::CommandCenter).unwrap().id;
    world.apply_orders(&[Order::Demolish { buildings: vec![cc, infected] }]);
    assert!(world.find_building(cc).is_some() && world.find_building(infected).is_some());
    // The editor removes anything, and nothing comes back.
    let gold = world.gold;
    world.apply_orders(&[Order::RemoveBuildings { buildings: vec![cc, infected] }]);
    assert!(world.find_building(cc).is_none() && world.find_building(infected).is_none());
    assert_eq!(world.gold, gold);
}

/// The editor's "Grid": the whole map grows by the same on every side – everything on it stays where it is, the
/// playing field and the spawners too (only their numbers move: the middle of the map moved). Smaller again, and it's
/// exactly as it was.
#[test]
fn the_grid_can_be_resized() {
    let (mut world, _) = quiet_world();
    let n = world.map.size;
    let from_middle = |w: &World, p: Vec2| p - Vec2::splat(w.map.size as f32 / 2.0);
    let cc = |w: &World| w.buildings.iter().find(|b| b.kind == BuildingKind::CommandCenter).map(|b| from_middle(w, b.center())).unwrap();
    let before = cc(&world);
    let (x, y) = (n as i32 / 2 + 21, n as i32 / 2 - 37);
    world.step(&[Order::PaintTerrain { terrain: Terrain::Water, tiles: vec![(x, y)] }]);
    let play = world.map.play_steps();
    let centers: Vec<Vec2> = world.spawners.iter().map(|s| from_middle(&world, s.center())).collect();
    let file = crate::mapfile::MapFile::from_world(&world, "x");

    world.step(&[Order::ResizeMap { size: n + 100 }]);
    assert_eq!(world.map.size, n + 100);
    assert_eq!(cc(&world), before, "the command center didn't stay where it was");
    assert_eq!(world.map.terrain(x + 50, y + 50), Terrain::Water, "the terrain didn't move along");
    // 50 tiles both ways: in the screen axes 0 across, 100 down.
    assert_eq!(world.map.play_steps(), ((play.0 .0, play.0 .1 + 100), play.1), "the playing field moved or changed");
    let moved: Vec<Vec2> = world.spawners.iter().map(|s| from_middle(&world, s.center())).collect();
    assert_eq!(moved, centers, "the spawners didn't stay where they were");

    world.step(&[Order::ResizeMap { size: n }]);
    assert_eq!((world.map.size, cc(&world)), (n, before));
    assert_eq!(crate::mapfile::MapFile::from_world(&world, "x"), file, "smaller again isn't the map it was");
}

/// The playing field (the world border) can lie anywhere on the map and be any size – the ground stays, only what's
/// inside changes: units may only go there, and a unit outside it goes to the nearest spot inside. It's saved with
/// the map, and loaded the base is the command center wherever it stands.
#[test]
fn the_playing_field_can_be_moved() {
    let (mut world, unit) = quiet_world();
    let ((x0, y0), (w, h)) = world.map.play_steps();
    let terrain = (0..world.map.size as i32).map(|x| world.map.terrain(x, 40)).collect::<Vec<_>>();
    // A narrow strip on the left, as high as before.
    world.step(&[Order::SetPlayArea { min: (x0, y0), size: (w / 3, h) }]);
    assert_eq!(world.map.play_steps(), ((x0, y0), (w / 3, h)));
    assert_eq!((0..world.map.size as i32).map(|x| world.map.terrain(x, 40)).collect::<Vec<_>>(), terrain, "the ground changed");
    let play = world.map.play_area();
    let u = world.find_unit(unit).unwrap();
    assert!(play.contains(world.units[u].pos), "a unit stayed outside the border");
    assert!(!world.map.in_play(world.base.x as i32 + 30, world.base.y as i32 - 30), "inside the old field is still inside");
    // Never off the map, never smaller than `MIN_PLAY`.
    world.step(&[Order::SetPlayArea { min: (-100_000, 0), size: (1, 1) }]);
    let ((x, _), (w2, h2)) = world.map.play_steps();
    assert_eq!((x, w2, h2), (-(world.map.size as i32) / 2, crate::map::MIN_PLAY, crate::map::MIN_PLAY));

    // Saved and loaded: the same field. The command center moved somewhere else – the base goes with it.
    world.step(&[Order::SetPlayArea { min: (x0, y0), size: (w / 3, h) }]);
    let cc = world.buildings.iter().find(|b| b.kind == BuildingKind::CommandCenter).unwrap().id;
    let spot = (world.base.x as i32 - 20, world.base.y as i32 + 30);
    clear_ground(&mut world, spot, 5);
    world.step(&[
        Order::RemoveBuildings { buildings: vec![cc] },
        Order::PlaceBuilding { kind: BuildingKind::CommandCenter, tile: spot, turned: false, editor: true },
    ]);
    let loaded = crate::mapfile::MapFile::from_world(&world, "x").to_world().unwrap();
    assert_eq!(loaded.map.play_steps(), world.map.play_steps());
    let center = loaded.buildings.iter().find(|b| b.kind == BuildingKind::CommandCenter).unwrap().center();
    assert_eq!(loaded.base, center, "the base isn't the command center");
}

/// The zombie brush, like TAB: on every tile so many of each breed – painting over it again only fills it up, never
/// more than 3 of a breed. Asleep like a new map's zombies, or awake and storming. Not on water. A giant: one tile.
#[test]
fn the_zombie_brush_fills_tiles() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32 + 20, world.base.y as i32 + 20);
    clear_ground(&mut world, (bx, by), 4);
    let tiles = vec![(bx, by), (bx + 1, by), (bx + 2, by)];
    let on = |w: &World, breed: Breed| {
        let here = |p: Vec2| tiles.contains(&(p.x.floor() as i32, p.y.floor() as i32));
        w.zombies.iter().filter(|z| z.breed == breed && here(z.pos)).count() + w.sleep.iter().filter(|s| s.breed == breed && here(s.pos)).count()
    };
    let brush = |walkers: u32, runners: u32, wander: bool| Order::PaintZombies { tiles: tiles.clone(), per_tile: vec![(Breed::Walker, walkers), (Breed::Runner, runners)], wander };
    world.apply_orders(&[brush(2, 1, true)]);
    assert_eq!((on(&world, Breed::Walker), on(&world, Breed::Runner)), (6, 3));
    assert_eq!(world.sleep.len(), 9, "the wanderers don't sleep");
    world.apply_orders(&[brush(2, 1, true)]);
    assert_eq!((on(&world, Breed::Walker), on(&world, Breed::Runner)), (6, 3), "painting again added more");
    world.apply_orders(&[brush(9, 1, false)]);
    assert_eq!(on(&world, Breed::Walker), 9, "more than 3 walkers on a tile");
    assert_eq!(world.zombies.len(), 3, "the storming ones aren't awake");

    let water = (bx + 3, by);
    world.apply_orders(&[Order::PaintTerrain { terrain: Terrain::Water, tiles: vec![water] }]);
    let before = world.zombies.len() + world.sleep.len();
    world.apply_orders(&[Order::PaintZombies { tiles: vec![water], per_tile: vec![(Breed::Fat, 3)], wander: true }]);
    assert_eq!(world.zombies.len() + world.sleep.len(), before, "zombies on water");
    world.apply_orders(&[Order::PaintZombies { tiles: vec![(bx, by + 2)], per_tile: vec![(Breed::Giant, 1)], wander: true }]);
    assert_eq!(world.sleep.iter().filter(|s| s.breed == Breed::Giant).count(), world.sleep.iter().filter(|s| s.breed == Breed::Giant && s.pos.y.floor() as i32 == by + 2).count());
}

/// A medic sent onto a unit with T walks to it and keeps the green beam on it. A soldier given the
/// same command shoots it instead (an attack on your own, on purpose). The medic never shoots at zombies.
#[test]
fn medic_beams_at_its_target() {
    let (mut world, id) = quiet_world();
    let start = world.base + vec2(-6.0, 6.0);
    world.spawn_unit(UnitKind::Medic, start);
    let medic = world.units.last().unwrap().id;
    let soldier = world.units[0].id;
    world.step(&[
        Order::Command { units: vec![medic, soldier], command: Command::Attack(Prey::Unit(id)), queue: false, formation: None },
        Order::SpawnZombiesAt { count: 1, point: start + vec2(-1.0, 0.0) },
    ]);
    let soldier_task = world.units[world.find_unit(soldier).unwrap()].order_name();
    assert_eq!(soldier_task, "Attack", "soldier: {soldier_task}");
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

/// Like TAB, units cost goods too: the ranger wood, the soldier iron from the foundry. One short, and it isn't
/// trained; trained, the goods leave the stock with the gold – and come back with it when it's cancelled.
#[test]
fn units_cost_wood_and_iron() {
    let (mut world, _) = quiet_world();
    let barracks = barracks(&mut world);
    let b = world.find_building(barracks).unwrap();
    let (ranger, soldier) = (UnitKind::Ranger.stats(), UnitKind::Soldier.stats());
    assert!(ranger.cost_wood > 0 && soldier.cost_iron > 0, "data\\units.json changed – change this test too");
    world.gold = 10_000;
    world.stock.set(Resource::Wood, ranger.cost_wood - 1);
    world.stock.set(Resource::Iron, soldier.cost_iron - 1);
    world.apply_orders(&[
        Order::Train { building: barracks, kind: UnitKind::Ranger },
        Order::Train { building: barracks, kind: UnitKind::Soldier },
    ]);
    assert!(world.buildings[b].production.is_empty(), "trained without the goods");
    assert_eq!(world.gold, 10_000);

    world.stock.set(Resource::Wood, ranger.cost_wood);
    world.stock.set(Resource::Iron, soldier.cost_iron);
    world.apply_orders(&[
        Order::Train { building: barracks, kind: UnitKind::Ranger },
        Order::Train { building: barracks, kind: UnitKind::Soldier },
    ]);
    assert_eq!(world.buildings[b].production.len(), 2);
    assert_eq!((world.stock.get(Resource::Wood), world.stock.get(Resource::Iron)), (0, 0));
    world.apply_orders(&[Order::CancelTrain { building: barracks, index: 1 }, Order::CancelTrain { building: barracks, index: 0 }]);
    assert_eq!((world.stock.get(Resource::Wood), world.stock.get(Resource::Iron)), (ranger.cost_wood, soldier.cost_iron));
    assert_eq!(world.gold, 10_000);
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
    let tiles: Vec<(i32, i32)> = (center.1 - r..=center.1 + r).flat_map(|y| (center.0 - r..=center.0 + r).map(move |x| (x, y))).collect();
    // The ore too: the plain test map has its start patches right by the base (`mapgen/ores.rs`).
    world.step(&[Order::PaintTerrain { terrain: Terrain::Grass, tiles: tiles.clone() }, Order::PaintOre { ore: None, tiles }]);
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
    // Far enough from the command center that the zombies don't see it and go for it instead.
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

/// Shot, a zombie goes for the shooter even from beyond its sight (a sniper 7.5 tiles away, a runner sees 6). It keeps
/// on it once more when it next looks around, then goes to where it was and sees it from there (`watch`): a runner shot
/// by the sniper in a tower 12 tiles away comes and tears at the tower. Not shot, the same zombie walks past to the
/// command center.
#[test]
fn a_shot_zombie_comes_for_the_shooter() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(8.0, -8.0);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 9);
    world.spawn_zombie_as(p + vec2(7.5, 0.0), Breed::Runner, true);
    world.zombies[0].hp = 100_000;
    world.zombies[0].speed = 0.0;
    world.spawn_unit(UnitKind::Sniper, p);
    let sniper = world.units[0].id;
    // The sniper first looks around, turns, raises its rifle and aims (`data\units.json`) – then it shoots. (Healing
    // itself, the zombie would fall back to its own hit points – it has to live through the shot.)
    for _ in 0..80 {
        world.zombies[0].hp = 100_000;
        world.step(&[]);
        if world.zombies[0].aim.is_some() {
            break;
        }
    }
    assert_eq!(world.zombies[0].aim, Some(Aim::Unit(sniper)), "being shot doesn't bring it");

    let run = |shot: bool| {
        let (mut world, _) = quiet_world();
        world.units.clear();
        let t = (world.base.x as i32, world.base.y as i32 - 30);
        clear_ground(&mut world, t, 18);
        world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tower, tile: t, turned: false, editor: false }]);
        let tower = world.buildings.last().unwrap().id;
        let center = world.buildings.last().unwrap().center();
        world.spawn_zombie(center + vec2(12.0, 0.0));
        world.zombies[0].hp = 100_000;
        world.spawn_unit(UnitKind::Sniper, center);
        let sniper = world.units.last_mut().unwrap();
        sniper.inside = Some(tower);
        sniper.cooldown = u32::MAX; // the one shot below has to bring it
        world.step(&[]);
        if shot {
            world.zombie_hit_by(0, Aim::Building(tower));
        }
        for _ in 0..600 {
            world.step(&[]);
        }
        let tower = &world.buildings[world.find_building(tower).unwrap()];
        tower.hp < tower.stats().hp
    };
    assert!(run(true), "the shot zombie didn't come for the tower");
    assert!(!run(false), "the zombie came although it neither saw it nor was shot");
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

    // Killed: they stay dead.
    world.step(&[Order::SetSpawnMode { spawner: ids[0], mode: SpawnMode::Off }]);
    for z in &mut world.zombies {
        z.hp = 0;
    }
    world.step(&[]);
    assert!(world.zombies.is_empty());
}

/// TAB's stragglers (`data\spawning.json`): nothing until hour 48 (at 100 days), then a few walkers from every side,
/// silently – no swarm on the minimap, no notice. The colony's unrest brings one more per side for every
/// `unrest_per_zombie`. An announced swarm is told hours before it comes.
#[test]
fn the_stragglers_come_from_every_side() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    world.events = plan_events(100, 1.0);
    world.roll_events();
    world.unrest = 2 * data::get().sound.unrest_per_zombie;
    let weak = world.events.iter().find(|e| e.name == "Stragglers weak").unwrap().hour;
    assert_eq!(weak, 48);
    while world.tick < weak as u64 * HOUR_TICKS {
        world.step(&[]);
    }
    assert!(world.zombies.is_empty(), "too early");
    let appeared = step_until_it_comes(&mut world, weak);
    // 3 to 6 walkers and 2 for the unrest, from each of the 4 sides.
    assert!((4 * 5..=4 * 8).contains(&appeared.len()), "{} stragglers", appeared.len());
    assert!(appeared.iter().all(|a| a.0 == Breed::Walker));
    let mut sides: Vec<u32> = appeared.iter().filter_map(|a| spawner_of(&world, a.1)).map(|s| world.spawners[s].side(&world.map)).collect();
    sides.sort();
    sides.dedup();
    assert_eq!(sides, vec![0, 1, 2, 3], "not from every side");
    assert!(world.swarms().is_empty(), "stragglers are marked on the minimap");
    assert!(world.notices.is_empty(), "stragglers are announced");

    // The first swarm: announced 8 hours before it comes.
    let swarm = world.events.iter().position(|e| e.name == "Swarm easy").unwrap();
    let comes = world.events[swarm].next_hour().unwrap();
    world.tick = (comes - 8) as u64 * HOUR_TICKS - 1;
    world.events.retain(|e| e.name == "Swarm easy");
    world.step(&[]);
    world.step(&[]);
    assert!(matches!(world.notices.last(), Some(Notice::Coming { at, .. }) if *at == comes as u64 * HOUR_TICKS), "{:?}", world.notices);
    assert_eq!(world.next_announced(), Some((comes as u64 * HOUR_TICKS, false)));
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
    // The unrest makes every side of the stragglers bigger – for good.
    world.unrest = 3 * data::get().sound.unrest_per_zombie;
    assert_eq!(world.unrest_zombies(), 3);
}

/// Whatever water and mountains the map generation makes: every spawner has a way to the base.
#[test]
fn every_spawner_reaches_the_base() {
    for seed in 1..=15 {
        let world = World::new(seed);
        for s in &world.spawners {
            let c = s.center();
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
        world.research_all();
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
    // In the ring outside the world border it may go – in a corner of the grid past the map's edge not.
    let ring = (world.base.x as i32, world.base.y as i32 - world.map.size as i32 / 2 + crate::map::BORDER as i32 / 2);
    assert!(world.map.on_map(ring.0, ring.1) && !world.map.in_play(ring.0, ring.1) && !world.map.on_map(4, 4));
    world.step(&[place(water), place(ring), place((4, 4)), place(unit_tile), place(world.buildings[0].tile)]);
    assert_eq!(world.buildings.len(), before + 3, "not off the map, not on the command center");
    assert_eq!(world.map.terrain(water.0, water.1), Terrain::Water, "the water under it has to stay water");
    assert!(nobody_on_blocked_ground(&world), "the unit wasn't pushed aside");
    // The game's rules still apply without the editor.
    assert!(!world.can_place(BuildingKind::Wall, (3, 3), false));
}

/// The editor's zombies: the breed asked for, and wanderers idle around – no home they keep to, and they don't head for
/// the command center – until they see a unit, then they storm like everyone else. Near the command center they stay
/// awake (`sleep.rs`).
#[test]
fn placed_zombies_have_a_breed_and_wanderers_stroll() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let home = world.base + vec2(12.0, -12.0);
    clear_ground(&mut world, (home.x as i32, home.y as i32), 8);
    world.step(&[Order::PlaceZombies { center: home, radius: 2.0, count: 20, breed: Breed::Walker, wander: true }]);
    assert_eq!(world.zombies.len(), 20);
    assert!(world.zombies.iter().all(|z| z.breed == Breed::Walker && !z.siege));
    let start: Vec<Vec2> = world.zombies.iter().map(|z| z.pos).collect();
    for _ in 0..400 {
        world.step(&[]);
    }
    assert_eq!(world.zombies.len(), 20, "a wanderer walked off or fell asleep next to the command center");
    assert!(world.zombies.iter().all(|z| z.aim.is_none() && !z.siege), "a wanderer heads for the base");
    let moved = world.zombies.iter().zip(&start).filter(|(z, s)| z.pos.distance(**s) > 0.5).count();
    assert!(moved > 0, "nobody roams");

    // A soldier (that doesn't shoot – no noise) appears right next to one of them: whoever sees it goes for it the
    // next time it looks around (`watch_seconds`), whoever stands further away than a walker sees (5 tiles) idles on.
    let at = world.zombies[0].pos;
    world.spawn_unit(UnitKind::Soldier, at);
    world.units[0].cooldown = u32::MAX;
    for _ in 0..Breed::Walker.stats().watch + 2 {
        world.step(&[]);
    }
    let sight = Breed::Walker.stats().sight;
    let near: Vec<&Zombie> = world.zombies.iter().filter(|z| z.pos.distance(at) < sight - 1.0).collect();
    assert!(!near.is_empty());
    assert!(near.iter().all(|z| z.aim.is_some()), "still idle with a soldier in sight");
    let far = world.zombies.iter().filter(|z| z.pos.distance(at) > sight + 1.0);
    assert!(far.into_iter().all(|z| z.aim.is_none()), "noticed a soldier it can't see");
}

/// Like TAB (`GetRandomRoamingPosition`): a roam goes on each axis on its own between `stroll[0]` and `stroll[1] + 1`
/// tiles away, either way. TAB lets nearly every breed stroll 0–1 (so under 2 tiles), the harpy 1–2 and the giant 2–3:
/// the walker never gets 2 tiles away on an axis, the harpy always at least 1, and the giant goes furthest.
#[test]
fn every_roam_stays_within_its_stroll() {
    let (mut world, _) = quiet_world();
    let home = world.base + vec2(20.5, 0.5);
    clear_ground(&mut world, (home.x as i32, home.y as i32), 8);
    for breed in [Breed::Walker, Breed::Runner, Breed::Harpy, Breed::Giant] {
        let [least, most] = breed.stats().stroll;
        let mut furthest = 0.0f32;
        for _ in 0..200 {
            let spot = world.roam_spot(home, breed).expect("no free spot on cleared ground");
            let d = (spot - home).abs();
            for axis in [d.x, d.y] {
                assert!(axis >= least - 1e-4 && axis < most + 1.0, "{breed:?} roamed {axis} tiles on an axis, stroll is {least}–{most}");
            }
            furthest = furthest.max(d.x.max(d.y));
        }
        assert!(furthest > most, "{breed:?} never got further than {furthest} tiles – the +1 of TAB is missing");
    }
    let [walker, harpy, giant] = [Breed::Walker, Breed::Harpy, Breed::Giant].map(|b| b.stats().stroll);
    assert!(walker[1] < harpy[1] && harpy[1] < giant[1], "TAB: the harpy strolls further than a walker, the giant furthest");
}

/// Every breed sees as far as `data\zombies.json` says: a runner 5.2 tiles from a soldier sees it (6 tiles), a
/// walker 5.8 tiles away doesn't (5) – counted to the soldier's edge.
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
    // Long enough for both to look around (`watch_seconds`).
    for _ in 0..Breed::Walker.stats().watch.max(Breed::Runner.stats().watch) + 2 {
        world.step(&[]);
    }
    let aim = |breed: Breed| world.zombies.iter().find(|z| z.breed == breed).unwrap().aim;
    assert!(aim(Breed::Runner).is_some(), "the runner didn't see the soldier");
    assert!(aim(Breed::Walker).is_none(), "the walker saw further than 5 tiles");
}

/// Noise like in TAB: a long fight is heard by walkers 12 tiles away – too far to see the squad – and they come by
/// chance, the louder the surer. Eight soldiers (3 per shot) bring many more of them than eight rangers (1 per arrow).
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
        // The targets: in front, standing still and too tough to die. Behind: the wanderers, in a column 12 tiles away.
        for k in 0..3 {
            world.spawn_zombie_as(p + vec2(4.0, k as f32 - 1.0), Breed::Walker, false);
        }
        for k in 0..WANDERERS {
            world.spawn_zombie_as(p + vec2(-12.0, (k % 5) as f32 - 2.0), Breed::Walker, true);
        }
        for z in &mut world.zombies[..3] {
            z.speed = 0.0;
            z.hp = 1_000_000;
        }
        // By their numbers: the list is sorted by place now and then (`sort_by_place`).
        let wanderers: Vec<u32> = world.zombies[3..].iter().map(|z| z.id).collect();
        let mut came = std::collections::BTreeSet::new();
        for _ in 0..600 {
            world.step(&[]);
            for z in world.zombies.iter().filter(|z| wanderers.contains(&z.id) && z.lure.is_some()) {
                came.insert(z.id);
            }
        }
        came.len()
    };
    const WANDERERS: usize = 20;
    let (soldiers, rangers) = (lured(UnitKind::Soldier), lured(UnitKind::Ranger));
    assert!(soldiers >= WANDERERS / 3, "only {soldiers} of {WANDERERS} heard the soldiers' fight 12 tiles away");
    assert!(rangers * 2 <= soldiers, "the rangers' arrows brought {rangers}, the soldiers' shots {soldiers}");
}

/// The sound grid like TAB's: noise in the same cell adds up, and every `halve_seconds` every cell halves – quiet or
/// not. A noise that goes on settles at about twice what it makes in that time instead of growing for ever.
#[test]
fn noise_adds_up_and_halves_every_second() {
    let (mut world, _) = quiet_world();
    let halve = data::get().sound.halve as usize;
    let level = |world: &World| world.loudest_sound().map(|(_, level)| level);
    let at = vec2((world.base.x / 4.0).floor() * 4.0 + 1.0, (world.base.y / 4.0).floor() * 4.0 + 1.0) + vec2(20.0, 20.0);
    world.make_sound(at, 100);
    world.make_sound(at + vec2(1.5, 1.5), 60);
    assert_eq!(level(&world), Some(160), "it doesn't add up");
    let levels: Vec<Option<u32>> = (0..halve * 2).map(|_| {
        world.step(&[]);
        level(&world)
    }).collect();
    // Halved within one beat, then again exactly one beat later.
    let first = levels.iter().position(|&l| l == Some(80)).expect("it never halved");
    assert!(first < halve, "it waited {first} ticks to halve");
    assert_eq!(levels[first + halve], Some(40), "it didn't halve again a beat later");
    for _ in 0..halve * 8 {
        world.step(&[]);
    }
    assert_eq!(level(&world), None, "it never went quiet");

    // Going on: the same noise every tick settles.
    let (mut world, _) = quiet_world();
    let mut loudest = 0;
    for t in 0..halve * 20 {
        world.make_sound(at, 10);
        world.step(&[]);
        if t >= halve * 10 {
            loudest = loudest.max(level(&world).unwrap_or(0));
        }
    }
    let per_beat = 10 * halve as u32;
    assert!(loudest > per_beat && loudest <= 2 * per_beat, "a steady noise got to {loudest}, a beat makes {per_beat}");
}

/// A roamer that went for one of your units – and lost sight of it as it ran off – goes to where the unit was then
/// (TAB's `LastEnemyPosition`) and stays there, idling. A wave zombie in the same spot heads on for the command center
/// from there.
#[test]
fn a_zombie_goes_where_it_last_saw_a_unit_and_stays() {
    let run = |wander: bool| {
        let (mut world, _) = quiet_world();
        world.units.clear();
        let p = world.base + vec2(-14.0, 12.0);
        clear_ground(&mut world, (p.x as i32, p.y as i32), 32);
        world.spawn_zombie_as(p, Breed::Walker, wander);
        let id = bait(&mut world, UnitKind::Ranger, p + vec2(3.0, 0.0));
        for _ in 0..Breed::Walker.stats().watch + 2 {
            world.step(&[]);
        }
        assert_eq!(world.zombies[0].aim, Some(Aim::Unit(id)), "it didn't see the unit");
        // The unit runs off, out of its sight.
        world.step(&[Order::Command { units: vec![id], command: Command::Move(p + vec2(0.0, 30.0)), queue: false, formation: None }]);
        let mut seen = None;
        for _ in 0..400 {
            world.step(&[]);
            if world.zombies[0].aim.is_none() {
                seen = world.zombies[0].lured_to();
                break;
            }
        }
        let seen = seen.expect("it never lost sight of the unit");
        // Gone for good – else, if it lost sight only once the unit had stopped (when it looks depends on its number),
        // it would walk right up to the unit and find it again.
        world.units.retain(|u| u.id != id);
        // A walker walks slowly – on open ground it loses sight of the unit late, and far from where it stands.
        for _ in 0..1500 {
            world.step(&[]);
        }
        let zombie = &world.zombies[0];
        // Done with the unit: nothing it heads for – a wave zombie may be at the command center already, hitting it.
        let done = zombie.lure.is_none() && zombie.aim.is_none_or(|a| matches!(a, Aim::Building(_)));
        (zombie.pos, seen, done, !zombie.siege, world.base)
    };
    let (pos, seen, done, idle, _) = run(true);
    assert!(done && idle, "the roamer doesn't idle where it lost the unit");
    assert!(pos.distance(seen) < Breed::Walker.stats().stroll[1] * 1.5 + 2.0, "it isn't where it last saw the unit");
    let (pos, seen, done, idle, base) = run(false);
    assert!(done && !idle, "the wave zombie stays");
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
    world.events.clear();
    // At hour 0: it comes in this very tick – before any of your units falls and rises (those don't belong to it).
    world.step(&[Order::AddEvent(Event { final_wave: true, reveal: true, ..Event::new("Final", 0, WaveFrom::Sides(1), Breed::Runner, 1) })]);
    world.step(&[]);
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
    // At least one a batch, one batch a second.
    for _ in 0..inside.min(20) * data::get().doom.batch_every + 40 {
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


/// A group of an event comes out of the same side: the runners and the riots that come along.
#[test]
fn an_event_and_its_specials_come_from_the_same_side() {
    let (mut world, _) = quiet_world();
    world.events.clear();
    let hour = next_hour(&world);
    world.step(&[Order::AddEvent(one_group(hour, WaveFrom::Sides(1), &[(Breed::Runner, 30), (Breed::Armored, 10)]))]);
    let appeared = step_until_it_comes(&mut world, hour);
    assert_eq!(appeared.len(), 40);
    let side = |p: Vec2| world.map.full_area().nearest_side(p);
    let first = side(appeared[0].1);
    assert!(appeared.iter().all(|a| side(a.1) == first), "not all from the same side");
    assert!(appeared.iter().any(|a| a.0 == Breed::Armored) && appeared.iter().any(|a| a.0 == Breed::Runner));
}

/// A diamond of walls touching only at the corners (dragged diagonally) is closed: no zombie gets through a
/// corner – they can only tear it down.
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

/// 120 zombies in front of a gap one tile wide flow through it like a liquid (`crowd.rs`): they pile up wide in front
/// of it instead of sliding into each other, nobody zigzags, and within 15 s all of them are through (about 13). 120 of
/// TAB's size (radius 0.2) are as much body as 300 of the smaller ones before – only 2 fit through side by side, not 4:
/// 300 now take about 20 s, still flowing.
#[test]
fn a_horde_flows_through_a_gap() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let field: Vec<(i32, i32)> = (by - 14..=by + 14).flat_map(|y| (bx + 3..bx + 30).map(move |x| (x, y))).collect();
    let wall: Vec<(i32, i32)> = (by - 10..=by + 10).filter(|&y| y != by).map(|y| (bx + 12, y)).collect();
    world.step(&[Order::PaintTerrain { terrain: Terrain::Grass, tiles: field }]);
    world.step(&[
        Order::PaintTerrain { terrain: Terrain::Mountain, tiles: wall },
        Order::SpawnZombiesAt { count: 120, point: world.base + vec2(22.0, 0.5) },
    ]);
    // From 4.5 s to 7.5 s: how often each one steps visibly back against its step before (by `id`).
    let mut last: HashMap<u32, Vec2> = HashMap::new();
    let mut back: HashMap<u32, u32> = HashMap::new();
    for t in 0..150 {
        world.step(&[]);
        for z in world.zombies.iter().filter(|_| t >= 90) {
            let moved = z.pos - z.prev;
            if last.get(&z.id).is_some_and(|before| moved.length() > 0.02 && moved.dot(*before) < 0.0) {
                *back.entry(z.id).or_default() += 1;
            }
            last.insert(z.id, moved);
        }
    }
    let zombies: Vec<Vec2> = world.zombies.iter().map(|z| z.pos).collect();
    // Pairs that stand more than half inside each other (closer than one radius – all of them are runners).
    let radius = Breed::Runner.stats().radius;
    let mut merged = 0;
    for (i, p) in zombies.iter().enumerate() {
        merged += zombies[i + 1..].iter().filter(|q| p.distance(**q) < radius).count();
    }
    assert!(merged <= 5, "{merged} pairs of zombies inside each other");
    // The pile in front of the gap is wide (now about 1 tile to each side), not squeezed on the line to the gap.
    let in_front = |p: &&Vec2| p.x > world.base.x + 12.0 && p.x < world.base.x + 16.0;
    let widest = zombies.iter().filter(in_front).map(|p| (p.y - world.base.y).abs()).fold(0.0, f32::max);
    assert!(widest > 5.0 * radius, "pile only {widest} tiles to each side");
    // Squeezing into the gap, nobody zigzags (now at most 7 times). Turning on the spot (without `TURN`), a few at the
    // corner of the gap did, up to 16 times.
    let zigzag = back.values().filter(|&&n| n >= 8).count();
    assert_eq!(zigzag, 0, "{zigzag} zombies stepped back and forth 8 times or more in 3 s");
    for _ in 0..150 {
        world.step(&[]);
    }
    let behind = world.zombies.iter().filter(|z| z.pos.x > world.base.x + 12.0).count();
    assert_eq!(behind, 0, "{behind} of 120 still in front of the gap after 15 s");
}

/// A crowd pressing from all sides against a ring of walls stands a little inside itself and still, instead of
/// jittering (`crowd.rs`) – and nobody is pushed into the walls. With 6 rounds of pushing apart and all of the way each
/// round, 157 of these 600 stood more than a third inside another and they stepped back and forth 600 times in 3 s;
/// now about 5 and 20.
#[test]
fn a_crowd_at_a_wall_stands_a_little_inside_itself_and_calm() {
    /// A step this long (tiles) shows on the screen – about a pixel at the closest zoom.
    const VISIBLE: f32 = 0.02;
    let (mut world, _) = quiet_world();
    world.units.clear();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let field: Vec<(i32, i32)> = (by - 20..=by + 20).flat_map(|y| (bx - 20..=bx + 20).map(move |x| (x, y))).collect();
    world.step(&[Order::PaintTerrain { terrain: Terrain::Grass, tiles: field }]);
    world.step(&wall_ring((bx, by), 4, &[]));
    for wall in &mut world.buildings {
        wall.hp = u32::MAX / 2; // nobody breaks through
    }
    let sides = [vec2(12.0, 0.5), vec2(-12.0, 0.5), vec2(0.5, 12.0), vec2(0.5, -12.0)];
    let spawns = sides.map(|side| Order::SpawnZombiesAt { count: 150, point: world.base + side });
    world.step(&spawns);
    for _ in 0..250 {
        world.step(&[]);
    }
    // Per zombie: how close its nearest neighbour is, measured in "touching" (1 = just touching).
    let zombies: Vec<(Vec2, f32)> = world.zombies.iter().map(|z| (z.pos, z.radius)).collect();
    let nearest = |&(p, r): &(Vec2, f32)| {
        zombies.iter().filter(|(q, _)| *q != p).map(|&(q, s)| p.distance(q) / (r + s)).fold(f32::MAX, f32::min)
    };
    let deep = zombies.iter().filter(|z| nearest(z) < 0.65).count();
    assert!(deep * 20 <= zombies.len(), "{deep} of {} zombies stand more than a third inside another", zombies.len());
    // Their bodies stay out of the walls – the walls don't give way.
    let into_wall = |z: &Zombie| z.radius - world.buildings.iter().map(|b| b.distance_to(z.pos)).fold(f32::MAX, f32::min);
    let deepest = world.zombies.iter().map(into_wall).fold(0.0, f32::max);
    assert!(deepest < 0.01, "a zombie stands {deepest} tiles inside a wall");
    // Jitter: visible steps straight back against the one before, over 3 s (by `id` – the list gets sorted).
    let mut last: HashMap<u32, Vec2> = HashMap::new();
    let mut back = 0;
    for _ in 0..60 {
        world.step(&[]);
        for z in &world.zombies {
            let moved = z.pos - z.prev;
            if last.get(&z.id).is_some_and(|before| moved.length() > VISIBLE && moved.dot(*before) < 0.0) {
                back += 1;
            }
            last.insert(z.id, moved);
        }
    }
    // Fewer than one in 600 per tick.
    assert!(back <= 60 * zombies.len() / 600, "{back} visible steps back and forth in 60 ticks - the crowd jitters");
}

/// A stream of zombies coming in one behind the other (like out of a spawner) doesn't queue up in front of a wall: the
/// ones behind press, the front gives way to the sides, and the mass spreads along the whole side of the wall
/// (`crowd.rs`) – after 45 s all 80 stand within about a tile of it (1.0), over most of its length. 80 of TAB's size
/// (radius 0.2) take as much room as the 200 smaller ones before; 200 of them pile 1.8 tiles deep – packed tight, that's
/// as flat as they fit.
#[test]
fn a_stream_at_a_wall_spreads_along_it() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    no_more_zombies(&mut world);
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    clear_ground(&mut world, (bx + 5, by), 30);
    world.step(&wall_ring((bx, by), 6, &[]));
    for wall in &mut world.buildings {
        wall.hp = u32::MAX / 2; // nobody breaks through
    }
    for t in 0..900 {
        let spawn = t < 160 && t % 8 == 0;
        world.step(&if spawn { vec![Order::SpawnZombiesAt { count: 4, point: world.base + vec2(32.0, 0.5) }] } else { vec![] });
    }
    assert_eq!(world.zombies.len(), 80);
    // The east side of the ring: its outer edge 6.5 tiles from the base, from 6.5 tiles up to 6.5 down.
    let face = world.base.x + 6.5;
    let deepest = world.zombies.iter().map(|z| z.pos.x - z.radius - face).fold(0.0, f32::max);
    assert!(deepest < 1.5, "the pile goes {deepest} tiles out from the wall - they queue up behind each other");
    let at_face: Vec<f32> =
        world.zombies.iter().filter(|z| z.pos.x - z.radius - face < 0.12).map(|z| z.pos.y - world.base.y).collect();
    let (top, bottom) = at_face.iter().fold((0.0f32, 0.0f32), |(t, b), &y| (t.min(y), b.max(y)));
    assert!(top < -4.5 && bottom > 4.5, "along the wall only from {top} to {bottom}");
}

/// Who walks into another pushes it along – by their weight (`mass`, `crowd.rs`): a heavier one less far, a calm
/// stroller just as far. A runner heading for the command center walks into one that has stood for long in its way
/// (with speed 0: it doesn't walk by itself), and slides off around it. Measured on both sides – the runner's way
/// isn't quite straight.
#[test]
fn zombies_push_each_other() {
    let pushed = |breed: Breed, calm: bool| {
        let mut sum = 0.0;
        for side in [0.01, -0.01] {
            let (mut world, _) = quiet_world();
            world.units.clear();
            no_more_zombies(&mut world);
            let p = world.base + vec2(12.5, 0.5);
            clear_ground(&mut world, (p.x as i32, p.y as i32), 6);
            world.spawn_zombie_as(p, breed, calm);
            world.zombies[0].speed = 0.0;
            let touching = breed.stats().radius + Breed::Runner.stats().radius;
            world.spawn_zombie_as(p + vec2(touching, side), Breed::Runner, false);
            world.zombies[1].speed = Breed::Runner.stats().speed;
            for _ in 0..20 {
                world.step(&[]);
            }
            sum += world.zombies.iter().find(|z| z.breed == breed && z.speed == 0.0).unwrap().pos.distance(p);
        }
        sum / 2.0
    };
    let (walker, fat, calm) = (pushed(Breed::Walker, false), pushed(Breed::Fat, false), pushed(Breed::Walker, true));
    assert!(walker > 0.15, "a walker in the way was pushed only {walker} tiles");
    assert!(fat > 0.02 && fat < walker * 0.6, "a fat one was pushed {fat} tiles, a walker {walker}");
    assert!(calm > walker * 0.8, "a calm walker was pushed {calm} tiles, a busy one {walker}");
}

/// TAB's rocks (`crowd.rs`): one that has just stopped stands firm – the others slide off it instead of shoving it
/// away; after `ROCK_SECONDS` it gives way again. Your units too, while they hold or aim: the crowd doesn't shove them.
#[test]
fn who_just_stopped_stands_firm() {
    let pushed = |still: u32| {
        let (mut world, _) = quiet_world();
        world.units.clear();
        no_more_zombies(&mut world);
        let p = world.base + vec2(12.5, 0.5);
        clear_ground(&mut world, (p.x as i32, p.y as i32), 6);
        world.spawn_zombie_as(p, Breed::Walker, false);
        world.zombies[0].speed = 0.0;
        world.zombies[0].still = still;
        let touching = 2.0 * Breed::Walker.stats().radius;
        world.spawn_zombie_as(p + vec2(touching, 0.01), Breed::Runner, false);
        world.zombies[1].speed = Breed::Runner.stats().speed;
        for _ in 0..20 {
            world.step(&[]);
        }
        world.zombies.iter().find(|z| z.speed == 0.0).unwrap().pos.distance(p)
    };
    let (rock, long_ago) = (pushed(1), pushed(crowd::rock_ticks() + 1));
    assert!(rock < 0.01, "one that just stopped was pushed {rock} tiles");
    assert!(long_ago > 0.15, "one that stood for long was pushed only {long_ago} tiles");

    // A zombie pressed deep into one of your units: one that holds stays where it is and the zombie is pushed out of
    // it; one that has stood around for long gives way. A medic – it doesn't shoot, the zombie stays alive.
    let shoved = |hold: bool| {
        let (mut world, _) = quiet_world();
        no_more_zombies(&mut world);
        world.units.clear();
        let p = world.base + vec2(12.5, 0.5);
        clear_ground(&mut world, (p.x as i32, p.y as i32), 8);
        world.spawn_unit(UnitKind::Medic, p);
        if hold {
            let id = world.units[0].id;
            world.step(&[Order::Command { units: vec![id], command: Command::Hold, queue: false, formation: None }]);
        }
        let touching = UNIT_RADIUS + Breed::Walker.stats().radius;
        world.spawn_zombie_as(p + vec2(touching * 0.5, 0.0), Breed::Walker, false);
        world.zombies[0].speed = 0.0;
        for _ in 0..5 {
            world.step(&[]);
        }
        (world.units[0].pos.distance(p), world.zombies[0].pos.distance(world.units[0].pos) / touching)
    };
    let ((held, apart), (idle, _)) = (shoved(true), shoved(false));
    assert!(held < 0.005, "the holding medic was shoved {held} tiles");
    assert!(apart > 0.95, "the zombie still stands {apart} of touching from the holding medic");
    assert!(idle > 0.01, "the idle medic gave way only {idle} tiles");
}

/// A giant walks through a crowd of walkers: they give way, it hardly slows down (`mass`).
#[test]
fn a_giant_walks_through_walkers() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    no_more_zombies(&mut world);
    let p = world.base + vec2(20.5, 0.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 12);
    for y in -6..=6 {
        for x in -3..=3 {
            world.spawn_zombie_as(p + vec2(x as f32, y as f32) * 0.3, Breed::Walker, true);
        }
    }
    world.spawn_zombie_as(p + vec2(4.0, 0.0), Breed::Giant, false);
    let giant = world.zombies.len() - 1;
    world.zombies[giant].speed = Breed::Giant.stats().speed;
    let start = world.zombies[giant].pos;
    // Time for 10 tiles at its speed: through the walkers (6 tiles), not yet at the command center (20 tiles away) –
    // TAB's giant runs 4 tiles a second. Not counting its roar (every 12 s, when depends on its number): it stands then.
    let ticks = (10.0 / (Breed::Giant.stats().speed * TICK)).ceil() as u32;
    let mut walking = 0;
    while walking < ticks {
        world.step(&[]);
        if world.zombies.iter().any(|z| z.breed == Breed::Giant && z.roar == 0) {
            walking += 1;
        }
    }
    let giant = world.zombies.iter().find(|z| z.breed == Breed::Giant).unwrap();
    let walked = giant.pos.distance(start);
    let free = Breed::Giant.stats().speed * TICK * ticks as f32;
    assert!(walked > free * 0.8, "the giant got only {walked} of {free} tiles through the walkers");
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
    assert!(world.zombies.iter().all(|z| !z.siege));
    assert!(!world.zombies.is_empty() && world.sleep.len() > world.zombies.len(), "near the base awake, far out asleep");
    // The packs – the giants come on top, anywhere out there (`place_giants`).
    let mut per_tile = std::collections::HashMap::new();
    for (pos, _) in all.iter().filter(|(_, b)| *b != Breed::Giant) {
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
    // Right at the ring around the command center only one per tile, and fewer of them than a little further out
    // (`ring_ramp`) – not three on a tile right away.
    let ring = world.keep_out.least();
    let beyond = |tile: &(i32, i32)| vec2(tile.0 as f32 + 0.5, tile.1 as f32 + 0.5).distance(world.base) - ring;
    let most_at_ring = per_tile.iter().filter(|(t, _)| beyond(t) < 1.5).map(|(_, &n)| n).max().unwrap_or(0);
    let band = |from: f32| per_tile.iter().filter(|(t, _)| (from..from + 3.0).contains(&beyond(t))).map(|(_, &n)| n).sum::<u32>();
    let (at_ring, further) = (band(0.0), band(roaming.ring_ramp));
    assert!(most_at_ring <= 1, "{most_at_ring} zombies on one tile right at the ring");
    assert!(at_ring * 2 < further, "{at_ring} zombies right at the ring, {further} a little further out");
}

/// A unit that dies lies there for `rise_min_seconds` to `rise_max_seconds` (`data\zombies.json`), then gets up where it
/// fell as one of TAB's infected (`infected`), heading for the command center.
#[test]
fn dead_units_rise_as_tabs_infected() {
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
    let (min, max) = (values().rise_min, values().rise_max);
    for ticks in 1..=max + 20 {
        world.step(&[]);
        if !world.zombies.is_empty() {
            rose_after = Some(ticks);
            break;
        }
    }
    let ticks = rose_after.expect("never got up");
    assert!((min..=max).contains(&ticks), "got up after {ticks} ticks, not after {min} to {max}");
    assert!(world.corpses.is_empty());
    assert!(values().infected.contains(&world.zombies[0].breed) && world.zombies[0].siege);
    assert!(world.zombies[0].pos.distance(p) < 1.0);
}

/// How many zombies come out of an infected building like TAB's (`World::infect`): who lives and works in it beyond
/// the workers it gives, plus its `extra_infected`.
fn infected_people(kind: BuildingKind) -> usize {
    let s = kind.stats();
    ((s.worker_cost + s.colonists).saturating_sub(s.workers) + s.extra_infected) as usize
}

/// Zombies hit a tent's shield first. When it breaks, the tent is infected (TAB): its people come out as zombies
/// (`infected_people`), each one of TAB's infected at random, all heading for the command center – and only after that
/// does it lose hit points.
#[test]
fn a_tent_whose_shield_breaks_lets_out_its_people() {
    let mut walkers_seen = Vec::new();
    for seed in 0..12 {
        let (mut world, _) = quiet_world_of(seed);
        world.units.clear();
        let t = (world.base.x as i32 + 8, world.base.y as i32 - 8);
        clear_ground(&mut world, t, 6);
        world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tent, tile: t, turned: false, editor: false }]);
        let b = world.buildings.len() - 1;
        world.buildings[b].shield = 7; // two hits of a runner
        world.step(&[Order::SpawnZombiesAt { count: 1, point: vec2(t.0 as f32 - 0.6, t.1 as f32 + 1.0) }]);
        let first = world.zombies[0].id;
        while world.zombies.len() == 1 && world.tick < 400 {
            world.step(&[]);
            let tent = &world.buildings[b];
            if !tent.infected {
                assert_eq!(tent.hp, tent.stats().hp, "hit points down before the shield broke");
            }
        }
        let tent = &world.buildings[b];
        assert!(tent.infected, "seed {seed}: the shield never broke");
        assert_eq!(world.zombies.len(), 1 + infected_people(BuildingKind::Tent), "seed {seed}");
        let out: Vec<&Zombie> = world.zombies.iter().filter(|z| z.id != first).collect();
        assert!(out.iter().all(|z| values().infected.contains(&z.breed) && z.siege), "seed {seed}: not TAB's infected");
        assert!(out.iter().all(|z| z.pos.distance(tent.center()) < 3.0), "came out far from the tent");
        walkers_seen.push(out.iter().filter(|z| z.breed == Breed::Walker).count());
    }
    walkers_seen.sort_unstable();
    walkers_seen.dedup();
    assert!(walkers_seen.len() > 1, "always the same breeds");
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
    assert_eq!(world.zombies.len(), infected_people(BuildingKind::Tent));
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

/// A repair costs the share of `repair_cost` that's missing (half of hit points + shield: half the price), paid at once – and
/// that share of `repair_seconds`. Without enough gold nothing happens.
#[test]
fn a_repair_costs_its_share_and_takes_its_time() {
    let (mut world, _) = quiet_world();
    let id = barracks(&mut world);
    let b = world.find_building(id).unwrap();
    let (hp, shield, cost) = {
        let s = world.buildings[b].stats();
        (s.hp, s.max_shield(), s.repair_cost)
    };
    // TAB's barracks: as much shield as hit points – the shield gone is half of all it can lose.
    assert_eq!(hp, shield, "data\\buildings.json changed – change this test too");
    world.buildings[b].shield = 0;
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
    while world.buildings[b].shield < shield && ticks < 1000 {
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

/// How many shots a unit needs for a zombie – with TAB's values (ZXRules) and TAB's armor (damage × (1 − armor),
/// rounded): 3 soldier shots for a walker and for a runner, 4 / 5 arrows. If a value in `data\` changes, this test says whether they still hold.
#[test]
fn shots_to_kill_walkers_and_runners() {
    let shots = |kind: UnitKind, breed: Breed| {
        let damage = after_armor(kind.stats().damage, breed.stats().armor);
        breed.stats().hp.div_ceil(damage)
    };
    use UnitKind::*;
    assert_eq!([shots(Ranger, Breed::Walker), shots(Soldier, Breed::Walker), shots(Sniper, Breed::Walker)], [4, 3, 1]);
    assert_eq!([shots(Ranger, Breed::Runner), shots(Soldier, Breed::Runner), shots(Sniper, Breed::Runner)], [5, 3, 1]);
}

/// A ranger with its back to a zombie: once it has looked around and found it, it turns around and brings its bow up
/// (both at once), then draws it – only then the first arrow. After that one arrow every `aim + windup + cooldown`
/// ticks, the bow stays up.
#[test]
fn units_turn_raise_and_aim_before_they_shoot() {
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
    let (mut found, mut shots) = (None, Vec::new());
    for tick in 1..=120u32 {
        world.step(&[]);
        if found.is_none() && world.units[0].aim_at.is_some() {
            found = Some(tick);
        }
        if !world.shots.is_empty() {
            shots.push(tick);
        }
    }
    let found = found.expect("never found the zombie");
    assert!(found <= data::get().units.search, "found it only in tick {found}");
    // The tick it finds it already turns and raises; the tick both are done already counts for the wind-up.
    let first = found - 1 + stats.turn_around.max(stats.raise) + stats.aim + stats.windup;
    assert_eq!(shots.first(), Some(&first), "found in {found}, turn {} / raise {} + windup {}: {shots:?}", stats.turn_around, stats.raise, stats.windup);
    assert!(shots.windows(2).all(|w| w[1] - w[0] == stats.shot_ticks()), "not every {} ticks: {shots:?}", stats.shot_ticks());
}

/// A soldier facing a zombie, its rifle up, in the middle of its wind-up – then one more tick with `orders_then`
/// (given the soldier's `id` and where it stands). Returns the world and the soldier's index.
fn soldier_winding_up(orders_then: impl Fn(u32, Vec2) -> Vec<Order>) -> (World, usize) {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(8.5, -8.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 6);
    world.spawn_unit(UnitKind::Soldier, p);
    world.spawn_zombie_as(p + vec2(2.0, 2.0), Breed::Walker, false); // right in front of it
    world.zombies[0].speed = 0.0;
    world.zombies[0].hp = 100_000;
    let stats = UnitKind::Soldier.stats();
    assert!(stats.windup >= 2, "the soldier needs a wind-up of 2 ticks or more for this test");
    let mut ticks = 0;
    while world.units[0].aim <= stats.aim {
        world.step(&[]);
        assert!(world.shots.is_empty(), "shot before its wind-up");
        ticks += 1;
        assert!(ticks < 60, "never started its wind-up");
    }
    assert!(world.units[0].swing_from.is_some(), "the shot's animation didn't start");
    let id = world.units[0].id;
    world.step(&orders_then(id, p));
    (world, 0)
}

/// Animation cancelling, part 1: walking off in the middle of the wind-up cancels the shot – no bullet, no reloading
/// either, and the picture doesn't play the shot on.
#[test]
fn walking_off_during_the_windup_cancels_the_shot() {
    let away = |id, p: Vec2| vec![Order::Command { units: vec![id], command: Command::Move(p - vec2(3.0, 3.0)), queue: false, formation: None }];
    let (mut world, u) = soldier_winding_up(away);
    assert!(world.shots.is_empty(), "shot although it walked off");
    assert!(world.units[u].is_walking());
    assert_eq!(world.units[u].cooldown, 0, "reloading although it didn't shoot");
    assert_eq!(world.units[u].swing_from, None, "the cancelled shot's animation plays on");
    for _ in 0..10 {
        world.step(&[]);
        assert!(world.shots.is_empty(), "shot while walking");
    }
}

/// Animation cancelling, part 3: the same attack again in the middle of the wind-up starts the shot over (like
/// StarCraft) – it doesn't just go on with the old one.
#[test]
fn the_same_attack_again_starts_the_shot_over() {
    let (mut world, u) = soldier_winding_up(|_, _| Vec::new());
    assert!(world.shots.is_empty(), "the wind-up was too short for this test");
    let started = world.units[u].swing_from.expect("not winding up");
    let (id, zombie) = (world.units[u].id, world.zombies[0].id);
    world.step(&[Order::Command { units: vec![id], command: Command::Attack(Prey::Zombie(zombie)), queue: false, formation: None }]);
    assert!(world.shots.is_empty(), "shot although the attack started over");
    let restarted = world.units[u].swing_from.expect("no new wind-up");
    assert!(restarted > started && restarted == world.tick - 1, "the wind-up went on: started {started}, now {restarted}");
    let stats = UnitKind::Soldier.stats();
    for _ in 0..stats.aim + stats.windup {
        world.step(&[]);
    }
    assert!(!world.shots.is_empty(), "never shot after the new wind-up");
}

/// Attack-move with zombies in range: the unit stops at once – also on the tick the command comes, and after every
/// kill. It doesn't take a step towards its goal while it waits to look around (`search_seconds`).
#[test]
fn attack_move_doesnt_step_forward_between_targets() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(8.5, -8.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 6);
    world.spawn_unit(UnitKind::Soldier, p);
    world.spawn_zombie_as(p + vec2(1.5, 1.5), Breed::Walker, false);
    world.spawn_zombie_as(p + vec2(2.5, 2.5), Breed::Walker, false);
    world.zombies.iter_mut().for_each(|z| z.speed = 0.0);
    world.zombies[0].hp = 1;
    world.zombies[1].hp = 100_000;
    let (id, tough) = (world.units[0].id, world.zombies[1].id);
    world.step(&[Order::Command { units: vec![id], command: Command::AttackMove(p + vec2(4.0, 4.0)), queue: false, formation: None }]);
    for tick in 0..100 {
        assert!(world.units[0].pos.distance(p) < 1e-4, "stepped forward in tick {tick}: {:?}", world.units[0].pos - p);
        world.step(&[]);
    }
    assert_eq!(world.zombies.len(), 1, "the weak zombie isn't dead");
    let z = world.find_zombie(tough).unwrap();
    assert!(world.zombies[z].hp < 100_000, "never shot at the second zombie");
}

/// T on one of your own units: it's shot on purpose, until it's dead – then the command is done, and nobody shoots
/// your own by themselves.
#[test]
fn attack_on_your_own_unit() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(8.5, -8.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 6);
    world.spawn_unit(UnitKind::Soldier, p);
    world.spawn_unit(UnitKind::Soldier, p + vec2(2.0, 2.0));
    world.spawn_unit(UnitKind::Soldier, p - vec2(2.0, 2.0));
    let (shooter, victim) = (world.units[0].id, world.units[1].id);
    world.step(&[Order::Command { units: vec![shooter], command: Command::Attack(Prey::Unit(victim)), queue: false, formation: None }]);
    assert_eq!(world.units[0].order_name(), "Attack");
    let mut hurt = false;
    for _ in 0..400 {
        world.step(&[]);
        hurt |= world.find_unit(victim).is_some_and(|v| world.units[v].hp < world.units[v].max_hp());
        if world.find_unit(victim).is_none() {
            break;
        }
    }
    assert!(hurt && world.find_unit(victim).is_none(), "the unit wasn't shot dead");
    world.step(&[]); // the tick after: its target is gone, the command is done
    let s = world.find_unit(shooter).unwrap();
    assert!(world.units[s].queue.is_empty(), "the attack isn't done");
    assert_eq!(world.units[s].kills, 0, "a kill for shooting your own");
    // The third one is never shot – neither by the shooter nor by itself.
    for _ in 0..100 {
        world.step(&[]);
    }
    assert!(world.units.iter().all(|u| u.hp == u.max_hp()), "someone shot their own without being told to");
}

/// T on one of your own buildings: shot like a zombie hits it – first the shield, then the hit points – but it's
/// never infected by that. With the shield gone, though, the next zombie hit infects it at once. And a repair makes it
/// whole again, even from 1 hit point.
#[test]
fn attack_on_your_own_building() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let t = (world.base.x as i32 + 8, world.base.y as i32 - 8);
    clear_ground(&mut world, t, 6);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tent, tile: t, turned: false, editor: true }]);
    let b = world.buildings.len() - 1;
    let (tent, full_hp, full_shield) = {
        let tent = &world.buildings[b];
        assert!(tent.is_built() && tent.stats().colonists > 0 && tent.stats().max_shield() > 0, "the tent changed");
        (tent.id, tent.stats().hp, tent.stats().max_shield())
    };
    world.buildings[b].shield = full_shield.min(30); // a few shots
    world.spawn_unit(UnitKind::Soldier, vec2(t.0 as f32 - 2.5, t.1 as f32 + 0.5));
    let id = world.units[0].id;
    world.step(&[Order::Command { units: vec![id], command: Command::Attack(Prey::Building(tent)), queue: false, formation: None }]);
    let mut ticks = 0;
    while world.buildings[b].hp == full_hp {
        world.step(&[]);
        ticks += 1;
        assert!(ticks < 600, "the hit points never went down");
        assert!(!world.buildings[b].infected, "infected by your own shots");
    }
    world.step(&[Order::Stop { units: vec![id] }]);
    assert_eq!(world.buildings[b].shield, 0, "hit points down before the shield");
    assert!(!world.buildings[b].infected, "infected by your own shots");
    // Without its shield the next zombie hit infects it – and hurts it too.
    let mut hit = world.buildings[b].clone();
    assert!(hit.take_damage(1) && hit.infected, "a zombie hit didn't infect it");
    assert!(hit.hp < world.buildings[b].hp, "the infecting hit did no damage");
    // A repair brings everything back, even from 1 hit point.
    world.buildings[b].hp = 1;
    world.gold = 1_000_000;
    world.step(&[Order::Repair { buildings: vec![tent] }]);
    for _ in 0..2000 {
        if world.buildings[b].repair_left == 0 {
            break;
        }
        world.step(&[]);
    }
    let tent = &world.buildings[b];
    assert_eq!((tent.hp, tent.shield), (full_hp, full_shield), "not whole after the repair");
}

/// A ranger, a zombie `at` this far from it (in front of it) with `hp` – standing still. Steps until the ranger shoots;
/// returns the world (ranger 0, zombie 0) and where the ranger stands.
fn ranger_shoots_at(at: Vec2, hp: u32) -> (World, Vec2) {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(8.5, -8.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 8);
    world.spawn_unit(UnitKind::Ranger, p);
    world.spawn_zombie_as(p + at, Breed::Walker, false);
    world.zombies[0].speed = 0.0;
    world.zombies[0].hp = hp;
    for _ in 0..80 {
        world.step(&[]);
        if !world.shots.is_empty() {
            return (world, p);
        }
    }
    panic!("the ranger never shot");
}

/// An arrow flies (TAB: 12 tiles a second) and hits only when it gets there – a bullet would have hit already.
#[test]
fn an_arrow_hits_when_it_lands() {
    let (mut world, p) = ranger_shoots_at(vec2(3.8, 3.8), 100_000);
    let full = world.zombies[0].hp;
    let flight = arrows::flight_ticks(UnitKind::Ranger.stats().projectile_speed, p, world.zombies[0].pos);
    assert!(flight > 1, "{flight} ticks is no flight");
    for tick in 1..flight {
        world.step(&[]);
        assert_eq!(world.zombies[0].hp, full, "hit after {tick} of {flight} ticks");
    }
    world.step(&[]);
    let damage = after_armor(UnitKind::Ranger.stats().damage, Breed::Walker.stats().armor);
    assert_eq!(world.zombies[0].hp, full - damage, "not hit when the arrow landed");
}

/// An arrow on its way counts against overkill (TAB's `AddFutureDamage`): its zombie dies of it, so a second ranger
/// that looks around meanwhile takes the other zombie – even though the first one is nearer. The first ranger walks off
/// right after its shot, so only the arrow keeps the zombie "taken".
#[test]
fn an_arrow_on_its_way_counts_against_overkill() {
    let (mut world, p) = ranger_shoots_at(vec2(3.8, 3.8), 1);
    let doomed = world.zombies[0].id;
    let first = world.units[0].id;
    world.step(&[
        Order::Command { units: vec![first], command: Command::Move(p - vec2(3.0, 3.0)), queue: false, formation: None },
        Order::SpawnUnit { kind: UnitKind::Ranger, point: p + vec2(0.0, 0.6) },
        Order::PlaceZombies { center: p + vec2(5.5, 1.0), radius: 0.0, count: 1, breed: Breed::Walker, wander: true },
    ]);
    let other = world.zombies.iter().find(|z| z.id != doomed).unwrap().id;
    world.zombies.iter_mut().for_each(|z| z.speed = 0.0);
    let second = world.units.len() - 1;
    let mut took = None;
    while world.find_zombie(doomed).is_some() {
        if let Some(aim) = world.units[second].aim_at {
            took = Some(aim);
            break;
        }
        world.step(&[]);
    }
    assert_eq!(took, Some(combat::Aim::Zombie(other)), "the second ranger didn't take the other zombie while the arrow flew");
}

/// Animation cancelling, part 2: walking off right after the shot costs nothing – the reloading goes on while it walks.
/// The weapon goes down bit by bit: after one step it's only partly down, and it comes up again in that share of
/// `raise` (once the soldier has looked around and found the zombie again).
#[test]
fn walking_off_after_the_shot_keeps_reloading() {
    let stats = UnitKind::Soldier.stats();
    // Let it finish the wind-up and shoot.
    let (mut world, u) = soldier_winding_up(|_, _| Vec::new());
    for _ in 0..stats.windup {
        if !world.shots.is_empty() {
            break;
        }
        world.step(&[]);
    }
    assert!(!world.shots.is_empty(), "didn't shoot at the end of its wind-up");
    let reload = world.units[u].cooldown;
    let (id, p) = (world.units[u].id, world.units[u].pos);
    world.step(&[Order::Command { units: vec![id], command: Command::Move(p - vec2(3.0, 3.0)), queue: false, formation: None }]);
    let unit = &world.units[u];
    assert!(unit.is_walking());
    assert_eq!(unit.cooldown, reload - 1, "the reloading stopped while it walked");
    assert!(unit.weapon > 0 && unit.weapon < units::WEAPON_UP, "after one step the weapon should be partly down: {}", unit.weapon);
    let down = units::WEAPON_UP - unit.weapon;
    // Stop: once it has found the zombie again, the rifle comes up – in that share of `raise`.
    world.step(&[Order::Command { units: vec![id], command: Command::Hold, queue: false, formation: None }]);
    let mut waited = 0;
    while world.units[u].aim_at.is_none() {
        assert_eq!(world.units[u].weapon, units::WEAPON_UP - down, "the weapon moved while it looked around");
        world.step(&[]);
        waited += 1;
        assert!(waited <= data::get().units.search, "didn't find the zombie again");
    }
    let mut raising = 1; // the tick it found it, it already raised
    while world.units[u].weapon < units::WEAPON_UP {
        world.step(&[]);
        raising += 1;
    }
    let expected = down.div_ceil(units::WEAPON_UP.div_ceil(stats.raise.max(1)));
    assert_eq!(raising, expected, "raising it from partly down");
    assert!(raising < stats.raise, "as long as from all the way down");
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

/// Won when the final wave is dead (an event that `won_when_dead`, like the final waves of old maps): once it has come
/// and its last zombie is dead. The final wave calls every zombie on the map – they belong to it then, and have to die
/// too. Zombies coming later don't matter.
#[test]
fn killing_the_final_wave_wins_the_round() {
    let (mut world, _) = quiet_world();
    world.events.clear();
    let hour = next_hour(&world);
    world.step(&[
        Order::AddEvent(Event { final_wave: true, won_when_dead: true, ..Event::new("Final", hour, WaveFrom::Sides(1), Breed::Runner, 3) }),
        Order::SpawnWave { count: 10, from: WaveFrom::Sides(1) },
    ]);
    while world.tick <= hour as u64 * HOUR_TICKS || !world.requests.is_empty() {
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

/// Won by the clock, like TAB: the victory event comes at the end of the last day – whoever is still alive.
#[test]
fn the_round_is_won_by_the_clock() {
    let (mut world, _) = quiet_world();
    world.events.clear();
    let hour = next_hour(&world);
    world.step(&[Order::AddEvent(Event { won: true, ..Event::new("Victory", hour, WaveFrom::Sides(1), Breed::Runner, 0) }), Order::SpawnWave { count: 10, from: WaveFrom::Sides(1) }]);
    while world.tick < hour as u64 * HOUR_TICKS {
        world.step(&[]);
        assert_eq!(world.outcome, None, "won too early");
    }
    world.step(&[]);
    assert!(matches!(world.outcome, Some(Outcome::Won { .. })), "{:?}", world.outcome);
    assert_eq!(world.zombies.len(), 10);
}

/// A new round gets TAB's survival plan (`plan_events`), stretched to its days and as big as its population: the
/// victory at the end of the last day, the final wave from all four sides with every breed but the giants, showing
/// the map, announced a day before.
#[test]
fn a_new_round_gets_tabs_plan() {
    let world = World::new(3);
    assert_eq!(world.days, 100);
    assert_eq!(world.events, plan_events(100, data::get().difficulty("").population), "a new round doesn't get the planned events");
    let won = world.events.iter().find(|e| e.won).expect("no victory");
    assert_eq!(won.hour, 100 * 24);
    let last = world.events.iter().find(|e| e.final_wave).expect("no final wave");
    assert_eq!((last.hour, last.notify_hours), (92 * 24, Some(24)));
    assert!(last.reveal && last.groups[0].from == WaveFrom::Sides(4));
    for breed in [Breed::Runner, Breed::Armored, Breed::Fat, Breed::Harpy, Breed::Spitter] {
        assert!(breed_count(last, breed) > 0, "no {breed:?} in the final wave");
    }
    // The swarms grow every time they come.
    let swarm = world.events.iter().find(|e| e.name == "Swarm hard").unwrap();
    assert!(swarm.growth > 0.0 && swarm.every_hours > 0 && swarm.times == 5);
}

/// Changing the days (editor) moves the victory to the end of the new last day – the other events stay.
#[test]
fn changing_the_days_moves_the_victory() {
    let (mut world, _) = quiet_world();
    let before = world.events.clone();
    world.step(&[Order::SetDays { days: 20 }]);
    assert_eq!(world.days, 20);
    for (old, new) in before.iter().zip(&world.events) {
        if old.won {
            assert_eq!(new.hour, 20 * 24);
        } else {
            assert_eq!(old, new);
        }
    }
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

/// The events belong to the map: the editor's orders add, change and remove them. A changed one starts anew – one
/// whose hour has passed comes at once.
#[test]
fn the_events_can_be_edited() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    world.events.clear();
    world.apply_orders(&[Order::AddEvent(one_group(500, WaveFrom::Sides(1), &[(Breed::Fat, 2)]))]);
    assert_eq!(world.events[0].next_hour(), Some(500));
    let mut changed = world.events[0].clone();
    changed.hour = 0;
    world.apply_orders(&[Order::SetEvent { index: 0, event: changed }]);
    let appeared = step_until_it_comes(&mut world, 0);
    assert_eq!(appeared.len(), 2, "the changed event didn't come at once");
    assert_eq!(world.events[0].next_hour(), None, "it comes again");
    world.apply_orders(&[Order::RemoveEvent { index: 0 }]);
    assert!(world.events.is_empty());
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
    // Each looks around on its own tick (`search_seconds`) – after one search both have found theirs.
    for _ in 0..data::get().units.search {
        world.step(&[]);
    }
    let (a, b) = (world.units[0].aim_at, world.units[1].aim_at);
    assert!(a.is_some() && b.is_some(), "not both aiming: {}, {}", a.is_some(), b.is_some());
    assert!(a != b, "both snipers aim at the same walker");
}

/// A new round starts with what its difficulty says (`data\game_difficulties.json`) – units, gold and wood, nothing
/// else in store. Every difficulty, every house of a village of doom as full as its population and size say.
#[test]
fn a_new_round_starts_with_the_difficulty_values() {
    for difficulty in &data::get().difficulties.difficulties {
        let mut setup = MapSetup { generator: Generator::Blank, ..MapSetup::default() };
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
        let full = |d: &DoomHouse| d.inside == World::doom_zombies(d.size, difficulty.population) && d.factor == difficulty.population;
        assert!(!world.doom.is_empty() && world.doom.iter().all(full), "{name}: houses not as full as it says");
    }
}

/// A harder difficulty: more zombies roaming, fuller houses, stronger events (TAB's population) – and the
/// special breeds closer to the base. The same map either way. On a map of ours – on TAB's the difficulty is its
/// zombie population (`a_tab_map_round`).
#[test]
fn a_harder_difficulty_brings_more() {
    let make = |name: &str| {
        let mut setup = MapSetup { generator: Generator::Blank, ..MapSetup::default() };
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
    let events = |w: &World| w.events.iter().flat_map(|e| &e.groups).flat_map(|g| &g.breeds).map(|b| b.count).sum::<u32>();
    assert!(events(&brutal) > events(&easy) * 2, "the events aren't stronger");
    // The closest fat one: much closer on Brutal (10 tiles may) than on Easy (60).
    let closest = |w: &World, breed: Breed| w.sleep.iter().filter(|s| s.breed == breed).map(|s| s.pos.distance(w.base)).fold(f32::INFINITY, f32::min);
    assert!(closest(&easy, Breed::Fat) >= easy.keep_out.fat, "a fat one too close on Easy");
    assert!(closest(&brutal, Breed::Fat) < closest(&easy, Breed::Fat), "Brutal's fat ones aren't closer");
}

/// The editor's difficulty (`Order::SetDifficulty`): how close the breeds may come changes, the gold and wood and what
/// is placed stay – and the choice is in the map file.
#[test]
fn setting_the_difficulty_changes_keep_out_only() {
    let (mut world, _) = quiet_world();
    world.step(&[Order::SetGold { gold: 321 }, Order::SetWood { wood: 45 }]);
    let before = (world.zombies.len() + world.sleep.len(), world.buildings.len(), world.units.len(), world.events.clone());
    world.step(&[Order::SetDifficulty { name: "Brutal".into() }]);
    let brutal = data::get().difficulty("Brutal");
    assert_eq!((world.difficulty.as_str(), world.keep_out), ("Brutal", brutal.keep_out));
    assert_ne!(brutal.keep_out, data::get().difficulty("Easy").keep_out);
    assert_eq!((world.gold, world.stock.get(Resource::Wood)), (321, 45));
    assert_eq!(before, (world.zombies.len() + world.sleep.len(), world.buildings.len(), world.units.len(), world.events.clone()));
    // Saved and loaded.
    let file = crate::mapfile::MapFile::from_world(&world, "Test");
    assert_eq!(file.round.difficulty, "Brutal");
    let loaded = file.to_world().unwrap();
    assert_eq!((loaded.difficulty.as_str(), loaded.keep_out), ("Brutal", brutal.keep_out));
    // A name nobody knows is the default difficulty.
    world.step(&[Order::SetDifficulty { name: "nonsense".into() }]);
    assert_eq!(world.difficulty, data::get().difficulty("").name);
}

/// Idle like TAB (`next_idle`): rolled with the weights 1300 stand / 150 growl / 150 roam – and a roam goes `stroll`
/// tiles away on each axis on its own (a square ring, not a circle). Roaming has no ring around the command center –
/// only the giant keeps out of its `roam_ring`, unless it stands in it already.
#[test]
fn idle_zombies_mostly_stand_and_roam_in_a_square() {
    let mut world = World::empty(1, Map::blank(256, crate::map::BORDER));
    let p = world.base + vec2(30.5, 0.5);
    let [least, most] = Breed::Walker.stats().stroll;
    let mut counts = [0; 3];
    for _ in 0..3200 {
        match world.next_idle(p, Breed::Walker, 0.4).doing {
            Doing::Stand => counts[0] += 1,
            Doing::Growl => counts[1] += 1,
            Doing::Roam(goal) => {
                counts[2] += 1;
                let d = (goal - p).abs();
                assert!(d.x >= least && d.x <= most + 1.0 && d.y >= least && d.y <= most + 1.0, "roamed {d:?}");
            }
        }
    }
    // Expected: 2600 / 300 / 300.
    assert!((2450..2750).contains(&counts[0]) && (220..380).contains(&counts[1]) && (220..380).contains(&counts[2]), "{counts:?}");

    // A walker goes closer to the base as it likes; a giant outside its ring never into it, one inside it anywhere.
    let ring = Breed::Giant.stats().roam_ring;
    let base = world.base;
    let spots = |world: &mut World, from: Vec2, breed: Breed| -> Vec<f32> {
        (0..300).filter_map(|_| world.roam_spot(from, breed)).map(|s| s.distance(base)).collect()
    };
    let walker = spots(&mut world, p, Breed::Walker);
    assert!(walker.iter().any(|&d| d < 29.5), "a walker never roams closer to the base");
    let outside = base + vec2(ring + 3.0, 0.5);
    assert!(world.map.in_play(outside.x as i32, outside.y as i32));
    let giant = spots(&mut world, outside, Breed::Giant);
    assert!(!giant.is_empty() && giant.iter().all(|&d| d > ring), "a giant roamed into its ring");
    let inside = spots(&mut world, base + vec2(30.5, 0.5), Breed::Giant);
    assert!(inside.iter().any(|&d| d < 30.0), "a giant inside its ring can't move about in it");
}

/// TAB's contagion: a zombie that isn't with a wave joins one when at least `contagion_count` of its zombies with its
/// rank or a higher one stand on its tile – checked every `contagion_seconds`. Two aren't enough, a walker joins
/// runners, a fat one doesn't, a giant never. A sleeper joins the same way (`wake_joining`).
#[test]
fn wave_zombies_take_along_the_ones_they_walk_over() {
    let joins = |wave: usize, breed: Breed, asleep: bool| {
        let (mut world, _) = quiet_world();
        world.units.clear();
        // Far from the command center's eyes, so a sleeper stays asleep until the wave takes it. The middle of a tile,
        // like `base`.
        let p = world.base + vec2(40.0, 0.0);
        clear_ground(&mut world, (p.x as i32, p.y as i32), 3);
        for _ in 0..wave {
            world.spawn_zombie_as(p, Breed::Runner, false);
        }
        for z in &mut world.zombies {
            z.speed = 0.0; // the wave stays on the tile
        }
        if asleep {
            world.lay_to_sleep(p, breed);
        } else {
            world.spawn_zombie_as(p, breed, true);
            world.zombies.last_mut().unwrap().speed = 0.0;
        }
        for _ in 0..values().contagion + 2 {
            world.step(&[]);
        }
        let joined = world.zombies.iter().filter(|z| z.breed == breed).all(|z| z.siege);
        world.sleep.is_empty() && joined && world.zombies.iter().any(|z| z.breed == breed)
    };
    assert!(joins(3, Breed::Walker, false), "a walker didn't join three runners");
    assert!(!joins(2, Breed::Walker, false), "a walker joined only two runners");
    assert!(!joins(3, Breed::Fat, false), "a fat one joined runners");
    assert!(!joins(3, Breed::Giant, false), "a giant joined a wave");
    assert!(joins(3, Breed::Walker, true), "a sleeping walker didn't join three runners");
    assert!(!joins(2, Breed::Walker, true), "a sleeping walker joined only two runners");
}

/// The giant stops every `roar_every_seconds` and roars for `growl_seconds`, whatever it's doing – here walking to the
/// command center with a wave. Meanwhile it stays put.
#[test]
fn a_giant_stops_to_roar() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(30.5, 0.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 6);
    world.spawn_zombie_as(p, Breed::Giant, false);
    let every = Breed::Giant.stats().roar.unwrap();
    // Where it stood on every tick of its roar.
    let mut roaring = Vec::new();
    for _ in 0..every + values().growl + 2 {
        world.step(&[]);
        let giant = &world.zombies[0];
        if giant.roar > 0 {
            roaring.push(giant.pos);
        }
    }
    assert!(roaring.len() as u32 >= values().growl - 1, "it roared for {} ticks", roaring.len());
    // The first few ticks it may still be pushed a little, then it stands.
    assert!(roaring[4].distance(*roaring.last().unwrap()) < 0.01, "it walked on roaring");
    assert!(world.zombies[0].pos.distance(p) > 2.0, "it doesn't walk at all");
}

/// TAB's safety: with fewer than `cull_below` zombies left, one that has no way to the command center – on an island –
/// dies within `cull_every_seconds`, so the round doesn't hang on it. One with a way lives.
#[test]
fn a_zombie_on_an_island_dies_when_few_are_left() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(20.0, 0.0);
    let (cx, cy) = (p.x as i32, p.y as i32);
    clear_ground(&mut world, (cx, cy), 4);
    let water = (cy - 2..=cy + 2).flat_map(|y| (cx - 2..=cx + 2).map(move |x| (x, y))).filter(|&(x, y)| (x - cx).abs() == 2 || (y - cy).abs() == 2);
    world.step(&[Order::PaintTerrain { terrain: Terrain::Water, tiles: water.collect() }]);
    world.spawn_zombie_as(p, Breed::Walker, true);
    world.spawn_zombie_as(world.base + vec2(-20.0, 0.0), Breed::Walker, true);
    let (island, free) = (world.zombies[0].id, world.zombies[1].id);
    for z in &mut world.zombies {
        z.speed = 0.0;
    }
    for _ in 0..values().cull_every + 2 {
        world.step(&[]);
    }
    assert!(world.zombies.iter().all(|z| z.id != island), "the zombie on the island lives on");
    assert!(world.zombies.iter().any(|z| z.id == free), "a zombie with a way died");
}

/// No zombies coming anymore – no events: tests that wait for many payouts wait for days (`payout_ticks`), long enough
/// for the first stragglers.
fn no_more_zombies(world: &mut World) {
    world.events.clear();
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

/// A sawmill brings in its share of the forest every payout: `gather_per_tile` wood per tile it works. A second one right next to it
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
    // 10 payouts, `gather_per_tile` each per tile.
    let per_tile = BuildingKind::Sawmill.stats().gather_per_tile;
    assert_eq!(world.stock.get(Resource::Wood), (tiles as f32 * per_tile * 10.0).round() as u32);

    let second = (spot.0 + 2, spot.1);
    let alone = world.worked_tiles(BuildingKind::Sawmill, second, 0).len();
    let shared = world.worked_tiles(BuildingKind::Sawmill, second, world.buildings.len()).len();
    assert!(shared < alone, "the second sawmill gets the first one's trees too ({shared} of {alone})");
}

/// Like TAB: the mine and the tesla tower need no workshop – the barracks needs stone, so the mine is there from the
/// start. It gets whatever ore lies around it – iron ore, coal and stone.
#[test]
fn a_mine_needs_no_workshop_and_gets_every_ore() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let spot = (bx - 9, by - 9);
    clear_ground(&mut world, spot, 6);
    assert!(!world.buildings.iter().any(|b| b.kind == BuildingKind::WoodWorkshop));
    assert!(world.unlocked(BuildingKind::Mine) && world.unlocked(BuildingKind::TeslaTower));
    world.apply_orders(&[
        Order::PaintOre { ore: Some(Ore::Iron), tiles: vec![(spot.0 - 1, spot.1)] },
        Order::PaintOre { ore: Some(Ore::Coal), tiles: vec![(spot.0 + 2, spot.1)] },
        Order::PaintOre { ore: Some(Ore::Stone), tiles: vec![(spot.0, spot.1 - 1), (spot.0 + 1, spot.1 - 1)] },
    ]);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Mine, tile: spot, turned: false, editor: false }]);
    assert!(world.buildings.iter().any(|b| b.kind == BuildingKind::Mine), "not built");
    for r in [Resource::IronOre, Resource::Coal, Resource::Stone] {
        world.stock.set(r, 0);
    }
    no_more_zombies(&mut world);
    for _ in 0..10 {
        one_payout(&mut world);
    }
    let got = |r: Resource| world.stock.get(r);
    let n = (BuildingKind::Mine.stats().gather_per_tile * 10.0).round() as u32;
    assert_eq!((got(Resource::IronOre), got(Resource::Coal), got(Resource::Stone)), (n, n, 2 * n));
}

/// A mine on gold (TAB's) digs gold – `gold_per_tile` per tile and payout, straight into the colony's gold, and the HUD
/// counts it as income. Stone next to it goes into the stock as before.
#[test]
fn a_mine_digs_gold() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let spot = (bx - 9, by - 9);
    clear_ground(&mut world, spot, 6);
    let gold: Vec<(i32, i32)> = (0..2).flat_map(|dy| (0..3).map(move |dx| (spot.0 + dx, spot.1 + dy))).collect();
    world.apply_orders(&[
        Order::PaintOre { ore: Some(Ore::Gold), tiles: gold },
        // 10 tiles of stone left of it: one stone a payout.
        Order::PaintOre { ore: Some(Ore::Stone), tiles: (0..10).map(|k| (spot.0 - 2 + k % 2, spot.1 - 2 + k / 2)).collect() },
    ]);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Mine, tile: spot, turned: false, editor: false }]);
    let b = world.buildings.iter().position(|b| b.kind == BuildingKind::Mine).expect("not built");
    assert_eq!(world.gold_rate(BuildingKind::Mine, spot, b, 1000), (6.0 * BuildingKind::Mine.stats().gold_per_tile * 1000.0) as u64);
    no_more_zombies(&mut world);
    world.stock.set(Resource::Stone, 0);
    one_payout(&mut world);
    let (before, income) = (world.score.gold_earned, world.buildings[b].paid);
    one_payout(&mut world);
    let dug = (6.0 * BuildingKind::Mine.stats().gold_per_tile) as u32;
    assert_eq!(world.buildings[b].paid, dug, "the mine's gold isn't its income");
    assert!(income == dug && world.score.gold_earned - before >= dug, "no gold dug");
    assert!(world.stock.get(Resource::Stone) > 0, "the stone next to it isn't mined any more");
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
    world.stock.set(Resource::IronOre, 0);
    no_more_zombies(&mut world);
    for _ in 0..10 {
        one_payout(&mut world);
    }
    // 10 payouts, `gather_per_tile` each per tile under it.
    let per_tile = BuildingKind::Mine.stats().gather_per_tile;
    assert_eq!(world.stock.get(Resource::IronOre), (under.len() as f32 * per_tile * 10.0).round() as u32);
}

/// On ore only defence and what digs it (ours – TAB builds nothing on ore): a house can't stand on it, a wall, a tower and
/// the mine can. On oil nothing but the oil platform – not even a wall.
#[test]
fn only_defence_and_mines_go_on_ore() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let spot = (bx - 9, by - 9);
    clear_ground(&mut world, spot, 6);
    let seam: Vec<(i32, i32)> = (0..2).flat_map(|dy| (0..2).map(move |dx| (spot.0 + dx, spot.1 + dy))).collect();
    let oil: Vec<(i32, i32)> = seam.iter().map(|&(x, y)| (x + 3, y)).collect();
    world.apply_orders(&[Order::PaintOre { ore: Some(Ore::Iron), tiles: seam }, Order::PaintOre { ore: Some(Ore::Oil), tiles: oil }]);
    assert!(!world.can_place(BuildingKind::Tent, spot, false), "a house on the ore");
    for kind in [BuildingKind::Wall, BuildingKind::Tower, BuildingKind::Mine] {
        assert!(world.can_place(kind, spot, false), "no {kind:?} on the ore");
    }
    let node = (spot.0 + 3, spot.1);
    for kind in [BuildingKind::Wall, BuildingKind::Tower, BuildingKind::Mine, BuildingKind::Tent] {
        assert!(!world.can_place(kind, node, false), "a {kind:?} on the oil");
    }
    assert!(world.can_place(BuildingKind::OilPlatform, node, false), "the oil platform doesn't go on its oil");
}

/// Nobody walks into an oil pool: a unit sent right through the middle of a node goes round it – its middle never
/// comes closer than the pool and its own radius – and arrives.
#[test]
fn units_walk_round_oil() {
    let (mut world, id) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let spot = (bx - 12, by + 6);
    clear_ground(&mut world, spot, 8);
    let node: Vec<(i32, i32)> = (0..2).flat_map(|dy| (0..2).map(move |dx| (spot.0 + dx, spot.1 + dy))).collect();
    world.apply_orders(&[Order::PaintOre { ore: Some(Ore::Oil), tiles: node }]);
    let middle = vec2(spot.0 as f32 + 1.0, spot.1 as f32 + 1.0);
    let u = world.find_unit(id).unwrap();
    world.units[u].pos = middle + vec2(-4.0, 0.0);
    world.units[u].prev = world.units[u].pos;
    let goal = middle + vec2(4.0, 0.0);
    world.step(&[Order::Command { units: vec![id], command: Command::Move(goal), queue: false, formation: None }]);
    let closest = (crate::map::OIL_POOL + UNIT_RADIUS).min(1.0) - 0.01;
    for _ in 0..400 {
        world.step(&[]);
        let pos = world.units[world.find_unit(id).unwrap()].pos;
        assert!(pos.distance(middle) >= closest, "walked into the oil: {pos} – the middle at {middle}");
    }
    let pos = world.units[world.find_unit(id).unwrap()].pos;
    assert!(pos.distance(goal) < 1.0, "never got round the oil: at {pos}, sent to {goal}");
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

    assert!(!world.unlocked(BuildingKind::MunitionsFactory));
    let at = |dx: i32| (tile.0 + dx, tile.1 + 4);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::WoodWorkshop, tile: at(0), turned: false, editor: true }]);
    assert!(!world.unlocked(BuildingKind::MunitionsFactory), "the munitions factory needs the stone workshop");
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::StoneWorkshop, tile: at(3), turned: false, editor: false }]);
    let coal = world.stock.get(Resource::Coal);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::MunitionsFactory, tile: at(6), turned: false, editor: false }]);
    assert_eq!(world.buildings.last().unwrap().kind, BuildingKind::MunitionsFactory, "not built");
    assert_eq!(world.stock.get(Resource::Coal), coal - BuildingKind::MunitionsFactory.stats().cost_coal);
    world.apply_orders(&[Order::Train { building: id, kind: UnitKind::Soldier }]);
    let b = world.find_building(id).unwrap();
    assert_eq!(world.buildings[b].production.len(), 2, "no soldier with the munitions factory");
}

/// Research like TAB (`research.rs`): the stone house is locked until the stone workshop has researched it. The
/// research costs its gold at once, takes its time in the workshop (one at a time – a second one is refused), and
/// is done for good: its button goes, the stone house can be built – and a second time it can't be researched.
/// Cancelled, all of the gold comes back.
#[test]
fn research_unlocks_and_goes_once() {
    let (mut world, _) = quiet_world();
    world.researched.clear();
    world.extra_power = 1000;
    let t = data::get().research.index("TechStoneHouse").unwrap();
    let other = data::get().research.index("TechBank").unwrap();
    let workshop_tile = cleared_spot(&mut world, -10, 6, 6);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::StoneWorkshop, tile: workshop_tile, turned: false, editor: true }]);
    let w = world.buildings.len() - 1;
    let id = world.buildings[w].id;
    assert!(world.stands(Some(BuildingKind::StoneWorkshop)));
    assert!(!world.unlocked(BuildingKind::StoneHouse), "the stone house before its research");
    // Cancelled: the gold back.
    world.gold = 10_000;
    world.apply_orders(&[Order::Research { building: id, tech: t }]);
    assert_eq!(world.gold, 10_000 - tech(t).gold);
    assert!(world.researching(t) && !world.tech_offered(t));
    world.apply_orders(&[Order::CancelTrain { building: id, index: 0 }]);
    assert_eq!(world.gold, 10_000, "cancelled – the gold back");
    // Started again, and a second one in the same workshop is refused.
    world.apply_orders(&[Order::Research { building: id, tech: t }, Order::Research { building: id, tech: other }]);
    assert_eq!(world.buildings[w].production.len(), 1, "two at once");
    let ticks = tech(t).ticks(&data::get().research);
    for _ in 0..ticks + 2 {
        world.step(&[]);
    }
    assert!(world.researched(t), "not done after {ticks} ticks");
    assert!(world.unlocked(BuildingKind::StoneHouse) && !world.tech_offered(t));
    assert!(world.notices.iter().any(|n| matches!(n, Notice::Researched { .. })), "no notice");
    // Once is enough.
    let gold = world.gold;
    world.apply_orders(&[Order::Research { building: id, tech: t }]);
    assert_eq!(world.gold, gold, "researched twice");
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

/// Like TAB: a building placed by a player builds itself first – `build_seconds` long, from `build_start_percent` of
/// its hit points up, without a shield – that grows once the hit points are full. Until then it does nothing: a tent
/// gives no workers, a barracks trains nothing, and a zombie can't infect it (nobody lives in it yet). A hit taken
/// meanwhile stays missing. Finished, it works. The editor's buildings stand at once.
#[test]
fn buildings_build_themselves_first() {
    let (mut world, _) = quiet_world();
    world.instant_build = false;
    no_more_zombies(&mut world);
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let t = (bx + 6, by - 8);
    clear_ground(&mut world, t, 4);
    let workers = world.workers().1;
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tent, tile: t, turned: false, editor: false }]);
    let b = world.buildings.iter().position(|b| b.kind == BuildingKind::Tent).expect("not placed");
    let stats = BuildingKind::Tent.stats();
    assert!(stats.build_time > 20, "the tent has no build time to test with");
    assert!(!world.buildings[b].is_built() && world.buildings[b].shield == 0);
    assert!(world.buildings[b].hp < stats.hp / 2, "starts with {} of {} hit points", world.buildings[b].hp, stats.hp);
    assert_eq!(world.workers().1, workers, "a tent being built gives workers");
    world.buildings[b].hp -= 5;
    let mut shielded = false;
    while !world.buildings[b].is_built() {
        world.step(&[]);
        let tent = &world.buildings[b];
        assert!(tent.shield == 0 || tent.hp == stats.hp, "shield before full hit points");
        if tent.shield > 0 && !tent.is_built() && !shielded {
            shielded = true;
            let mut hit = tent.clone();
            assert!(!hit.take_damage(tent.shield + 1) && !hit.infected, "infected while it was built");
        }
    }
    assert!(shielded, "the shield never grew while it was built");
    assert_eq!(world.buildings[b].hp, stats.hp);
    assert_eq!(world.buildings[b].shield, stats.max_shield() - 5, "the hit while it was built is gone");
    assert_eq!(world.workers().1, workers + stats.workers);

    // A barracks being built trains nothing; finished, it does.
    let tile = (bx - 7, by - 8);
    clear_ground(&mut world, (tile.0 + 1, tile.1 + 1), 4);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Barracks, tile, turned: false, editor: false }]);
    let id = world.buildings.last().unwrap().id;
    world.step(&[Order::Train { building: id, kind: UnitKind::Ranger }]);
    assert!(world.buildings.last().unwrap().production.is_empty(), "trains while being built");
    while !world.buildings.last().unwrap().is_built() {
        world.step(&[]);
    }
    world.step(&[Order::Train { building: id, kind: UnitKind::Ranger }]);
    assert_eq!(world.buildings.last().unwrap().production.len(), 1, "doesn't train once it's finished");

    // The editor's buildings stand at once.
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Tower, tile: (bx + 8, by + 8), turned: false, editor: true }]);
    assert!(world.buildings.last().unwrap().is_built());
}

/// Dismissing units (Del): they're gone – no corpse, nobody rises – and so are their upkeep and the workers they took.
#[test]
fn dismissed_units_leave_the_colony() {
    let (mut world, _) = quiet_world();
    no_more_zombies(&mut world);
    let (upkeep, used) = (world.upkeep(), world.workers().0);
    let leaving: Vec<usize> = (0..3).collect();
    let ids: Vec<u32> = leaving.iter().map(|&i| world.units[i].id).collect();
    let their_upkeep: u32 = leaving.iter().map(|&i| world.units[i].stats().upkeep).sum();
    let their_workers: u32 = leaving.iter().map(|&i| world.units[i].stats().worker_cost).sum();
    world.step(&[Order::Dismiss { units: ids.clone() }]);
    assert!(ids.iter().all(|&id| world.find_unit(id).is_none()), "still there");
    assert!(world.corpses.is_empty(), "they left a corpse");
    assert_eq!(world.upkeep(), upkeep - their_upkeep);
    assert_eq!(world.workers().0, used - their_workers);
    assert_eq!(world.score.units_dismissed, 3);
}

/// The medic heals with its beam – but only a unit that hasn't been hit for `heal_after_seconds`: no healing in the
/// thick of a fight, unlike StarCraft. Then `heal_per_second`, up to full and not further.
#[test]
fn medics_heal_only_out_of_the_fight() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    no_more_zombies(&mut world);
    let p = world.base + vec2(8.5, 8.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 4);
    world.spawn_unit(UnitKind::Medic, p);
    world.spawn_unit(UnitKind::Soldier, p + vec2(1.5, 0.0));
    // A second soldier, hurt the same, far from the medic: it only heals itself (`regen.rs`) – the difference is the
    // medic's.
    let far = p + vec2(0.0, 14.0);
    clear_ground(&mut world, (far.x as i32, far.y as i32), 2);
    world.spawn_unit(UnitKind::Soldier, far);
    let tick = world.tick;
    world.units[1].take_hit(60, tick);
    world.units[2].take_hit(60, tick);
    let medic = UnitKind::Medic.stats();
    for _ in 0..medic.heal_after - 2 {
        world.step(&[]);
    }
    assert_eq!(world.units[1].hp, world.units[2].hp, "healed right after the hit");
    assert!(!world.beams.is_empty(), "the beam isn't on the hurt soldier");
    let before = world.units[1].hp - world.units[2].hp;
    let two_seconds = 2 * TICKS_PER_SECOND as u32;
    for _ in 0..2 + two_seconds {
        world.step(&[]);
    }
    let healed = world.units[1].hp - world.units[2].hp - before;
    let expected = (medic.heal_per_second * 2.0) as u32;
    assert!(healed + 1 >= expected && healed <= expected + 1, "healed {healed} in 2 s, not {expected}");
    for _ in 0..60 * TICKS_PER_SECOND as u32 {
        world.step(&[]);
    }
    assert_eq!(world.units[1].hp, UnitKind::Soldier.stats().hp, "not healed to full – or beyond it");
}

/// Veterans (DESIGN.md 7.3): a zombie that dies gives its experience to the units around it, split – a veteran takes
/// half a share. Enough of it and a unit is a veteran: more hit points (at once) and more damage. A unit far away gets
/// nothing, a mercenary neither.
#[test]
fn units_near_the_fallen_become_veterans() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    no_more_zombies(&mut world);
    let p = world.base + vec2(8.5, 8.5);
    world.spawn_unit(UnitKind::Ranger, p);
    world.spawn_unit(UnitKind::Ranger, p + vec2(1.0, 0.0));
    world.spawn_unit(UnitKind::MercRanger, p + vec2(0.0, 1.0));
    world.spawn_unit(UnitKind::Ranger, p + vec2(30.0, 0.0));
    let veterans = &data::get().units.veterans;
    // Walkers falling right next to them – two rangers share: each needs twice its `xp` of walkers.
    let fall = |world: &mut World, breed: Breed, count: u32| {
        for _ in 0..count {
            world.spawn_zombie_as(p + vec2(0.5, 0.5), breed, false);
            world.zombies.last_mut().unwrap().hp = 0;
        }
        world.share_experience();
        world.zombies.clear();
    };
    let walkers = 2 * veterans.xp / Breed::Walker.stats().xp;
    fall(&mut world, Breed::Walker, walkers - 1);
    assert!(!world.units[0].veteran, "a veteran too soon");
    fall(&mut world, Breed::Walker, 1);
    for i in 0..2 {
        let unit = &world.units[i];
        assert!(unit.veteran, "ranger {i} isn't a veteran after {walkers} walkers");
        let stats = unit.stats();
        assert_eq!(unit.hp, stats.hp * (100 + veterans.hp_percent) / 100, "the extra hit points didn't come");
        assert_eq!(unit.damage(), stats.damage * (100 + veterans.damage_percent) / 100);
    }
    assert_eq!(world.units[2].xp, 0, "the mercenary learned");
    assert_eq!(world.units[3].xp, 0, "the one far away learned");
    // A veteran takes half a share: next to a fresh one, the fresh one gets two thirds.
    world.spawn_unit(UnitKind::Ranger, p + vec2(0.0, -1.0));
    world.units[1].pos += vec2(30.0, 30.0);
    let before = (world.units[0].xp, world.units[4].xp);
    fall(&mut world, Breed::Fat, 1);
    let gained = (world.units[0].xp - before.0, world.units[4].xp - before.1);
    assert_eq!(gained.1, 2 * gained.0, "the veteran took a whole share");
    assert!(gained.1 > Breed::Walker.stats().xp * 1000, "a fat one gives no more than a walker");
}

/// The spearman fights in melee: every hit pushes the zombie back – a big one less than the whole `knockback`. Nearly
/// silent: a little louder than a zombie's hit, quieter than a rifle.
#[test]
fn spearmen_push_zombies_back() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    no_more_zombies(&mut world);
    let p = world.base + vec2(8.5, 8.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 5);
    world.spawn_unit(UnitKind::Spearman, p);
    world.spawn_zombie_as(p + vec2(1.0, 0.0), Breed::Fat, false);
    world.zombies[0].speed = 0.0; // it doesn't walk back in by itself
    let hp = world.zombies[0].hp;
    for _ in 0..40 {
        world.step(&[]);
        if world.zombies[0].hp < hp {
            break;
        }
    }
    assert!(world.zombies[0].hp < hp, "the spearman never hit");
    let stats = UnitKind::Spearman.stats();
    let pushed = world.zombies[0].pos.distance(world.units[0].pos) - 1.0;
    let fat = stats.knockback * Breed::Walker.stats().radius / Breed::Fat.stats().radius;
    assert!(fat < stats.knockback * 0.8, "a fat one isn't pushed less than a walker");
    assert!(pushed > fat * 0.8 && pushed < fat * 1.2, "pushed {pushed} tiles, not about {fat}");
    assert!(stats.range < 2.0, "the spearman should be melee");
    let zombie_hit = data::get().sound.zombie_hit as f32;
    assert!(stats.noise > zombie_hit && stats.noise < UnitKind::Soldier.stats().noise, "a spear makes {} noise", stats.noise);
}

/// The harpy (TAB's): it hears better than every other breed – and every hit it lands is loud (`hit_noise`), quicker
/// than the noise halves, so it adds up while it hits. It's the zombies' noise: the colony's unrest stays.
#[test]
fn a_harpy_gets_louder_the_longer_it_hits() {
    let harpy = Breed::Harpy;
    for breed in Breed::ALL.into_iter().filter(|&b| b != harpy) {
        assert!(harpy.hear_factor() > breed.hear_factor(), "the {breed:?} hears as well as the harpy");
    }
    let (mut world, _) = quiet_world();
    world.units.clear();
    no_more_zombies(&mut world);
    let p = world.base + vec2(12.5, 0.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 10);
    bait(&mut world, UnitKind::Ranger, p);
    world.spawn_zombie_as(p + vec2(1.0, 0.0), harpy, true);
    let unrest = world.unrest;
    let level = |world: &World| world.loudest_sound().map_or(0, |(_, level)| level);
    // Five seconds of hits (it first looks around, `watch_seconds`, and steps up): had the noise faded between two of
    // them, it would never get past one hit's worth.
    let mut loudest = 0;
    for _ in 0..120 {
        world.step(&[]);
        loudest = loudest.max(level(&world));
    }
    let one = harpy.hit_noise();
    assert!(one > Breed::Walker.hit_noise(), "a harpy's hit is no louder than a walker's");
    assert!(loudest >= 2 * one, "it doesn't add up – the loudest was {loudest}, one hit {one}");
    assert_eq!(world.unrest, unrest, "the harpy's noise counted as the colony's");
}

/// A harpy leaps over a wall (`jump` tiles thick at most) instead of tearing it down – the wall stays whole, the harpy
/// is inside. A thicker wall it can't leap: it hits it like everybody else.
#[test]
fn a_harpy_leaps_over_a_thin_wall_but_not_a_thick_one() {
    let jump = Breed::Harpy.stats().jump;
    assert!(jump >= 1.0, "the harpy can't leap over a single wall");
    let run = |thickness: i32| {
        let (mut world, _) = quiet_world();
        world.units.clear();
        no_more_zombies(&mut world);
        let center = (world.base.x as i32, world.base.y as i32);
        clear_ground(&mut world, center, 12);
        let orders: Vec<Order> = (0..thickness).flat_map(|k| wall_ring(center, 4 + k, &[])).collect();
        world.step(&orders);
        // Building the walls was loud – the harpy would come to that instead of heading for the command center.
        for _ in 0..300 {
            world.step(&[]);
        }
        world.spawn_zombie_as(world.base + vec2(12.5, 0.5), Breed::Harpy, false);
        for _ in 0..200 {
            world.step(&[]);
        }
        let harpy = world.zombies[0].pos - world.base;
        let inside = harpy.x.abs().max(harpy.y.abs()) < 4.0;
        let damaged = world.buildings.iter().filter(|b| b.kind == BuildingKind::Wall && b.hp < b.stats().hp).count();
        (inside, damaged)
    };
    let (inside, damaged) = run(1);
    assert!(inside, "the harpy didn't get over a single wall");
    assert_eq!(damaged, 0, "it hit the wall instead of leaping");
    let (inside, damaged) = run(jump.floor() as i32 + 1);
    assert!(!inside, "it got over a wall thicker than its jump");
    assert!(damaged > 0, "at a thick wall it should hit");
}

/// Every side an announced event comes out of is one swarm – the minimap marks its middle. Gone once all of it is dead.
#[test]
fn every_side_of_an_event_is_one_swarm() {
    let (mut world, _) = quiet_world();
    world.events.clear();
    let hour = next_hour(&world);
    let event = Event { notify_hours: Some(0), ..one_group(hour, WaveFrom::Sides(2), &[(Breed::Runner, 20), (Breed::Armored, 5)]) };
    world.step(&[Order::AddEvent(event)]);
    step_until_it_comes(&mut world, hour);
    let swarms = world.swarms();
    assert_eq!(swarms.iter().map(|&(_, n)| n).collect::<Vec<_>>(), vec![25, 25], "the runners and riots of a side are one swarm");
    let side = |p: Vec2| world.map.full_area().nearest_side(p);
    assert_ne!(side(swarms[0].0), side(swarms[1].0), "both swarms marked on the same side");
    world.zombies.clear();
    assert!(world.swarms().is_empty(), "a dead swarm is still marked");
}

/// The giants: only a few per map, far out and asleep – `giants` of the map on a map of TAB's size, fewer on a
/// smaller one, at least one.
#[test]
fn a_few_giants_lie_far_out() {
    let mut difficulty = data::get().difficulty("").clone();
    difficulty.roaming_zombies = 0.0;
    difficulty.giants_percent = 100;
    let mut big_setup = MapSetup { generator: Generator::Blank, ..MapSetup::default() };
    big_setup.common.giants = 4.0;
    let count = |world: &World| world.sleep.iter().filter(|s| s.breed == Breed::Giant).count()
        + world.zombies.iter().filter(|z| z.breed == Breed::Giant).count();
    let big = World::generate_with(3, &big_setup, &difficulty);
    assert_eq!(count(&big), 4);
    assert_eq!(big.giants().count(), 4, "the minimap doesn't find every giant");
    let from = data::get().roaming.giants_from;
    for giant in big.sleep.iter().filter(|s| s.breed == Breed::Giant) {
        assert!(big.way_out(giant.pos) >= from, "a giant {} of the way out", big.way_out(giant.pos));
    }
    let mut small = big_setup.clone();
    small.common.size = 128.0;
    assert_eq!(count(&World::generate_with(3, &small, &difficulty)), 1);
    // The difficulty: half as many on Easy …
    difficulty.giants_percent = 50;
    assert_eq!(count(&World::generate_with(3, &big_setup, &difficulty)), 2);
    // … and a map without giants has none on any difficulty.
    difficulty.giants_percent = 225;
    small.common.giants = 0.0;
    assert_eq!(count(&World::generate_with(3, &small, &difficulty)), 0);
}

/// The foundry (DESIGN.md decision 32): a tech building like TAB's – while it stands the machine gun tower can be
/// built – that smelts iron at every payout: `smelts` at most, `smelt_ore` iron ore and `smelt_coal` coal each, only as
/// much as there is. The machine gun tower costs that iron.
#[test]
fn the_foundry_smelts_iron_and_unlocks_the_machine_gun_tower() {
    let (mut world, _) = quiet_world();
    no_more_zombies(&mut world);
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    clear_ground(&mut world, (bx + 8, by - 8), 5);
    assert!(!world.unlocked(BuildingKind::MachineGunTower), "the machine gun tower without a foundry");
    world.step(&[
        Order::PlaceBuilding { kind: BuildingKind::StoneWorkshop, tile: (bx - 8, by + 6), turned: false, editor: true },
        Order::PlaceBuilding { kind: BuildingKind::Foundry, tile: (bx + 6, by - 10), turned: false, editor: false },
    ]);
    assert!(world.buildings.iter().any(|b| b.kind == BuildingKind::Foundry), "the foundry wasn't built");
    assert!(world.unlocked(BuildingKind::MachineGunTower));
    let stats = BuildingKind::Foundry.stats();
    let ore = stats.smelt_ore * stats.smelts + 1; // enough for all of it, and one left over
    world.stock.set(Resource::IronOre, ore);
    world.stock.set(Resource::Coal, 100);
    world.stock.set(Resource::Iron, 0);
    one_payout(&mut world);
    assert_eq!(world.stock.get(Resource::Iron), stats.smelts);
    assert_eq!(world.stock.get(Resource::IronOre), 1);
    assert_eq!(world.stock.get(Resource::Coal), 100 - stats.smelt_coal * stats.smelts);
    // One iron ore left: not enough for one iron – nothing more.
    one_payout(&mut world);
    assert_eq!(world.stock.get(Resource::Iron), stats.smelts);
    assert_eq!(world.buildings.iter().find(|b| b.kind == BuildingKind::Foundry).unwrap().smelted, 0);

    // The machine gun tower: not without its iron.
    let mg = BuildingKind::MachineGunTower;
    let place = Order::PlaceBuilding { kind: mg, tile: (bx + 10, by - 4), turned: false, editor: false };
    world.stock.set(Resource::Iron, mg.stats().cost_iron - 1);
    world.step(std::slice::from_ref(&place));
    assert!(!world.buildings.iter().any(|b| b.kind == mg), "built without its iron");
    world.stock.set(Resource::Iron, mg.stats().cost_iron);
    world.step(&[place]);
    assert!(world.buildings.iter().any(|b| b.kind == mg), "not built with its iron");
    assert_eq!(world.stock.get(Resource::Iron), 0);
}

/// The ballista (DESIGN.md decision 33): early like TAB's – the wood workshop unlocks it – and it shoots by itself,
/// always at the toughest zombie in range: a fat one before a walker standing closer. Then it reloads; every shot
/// makes noise. The machine gun tower shoots the nearest, many times a second.
#[test]
fn towers_shoot_by_themselves() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    no_more_zombies(&mut world);
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let spot = (bx + 12, by);
    clear_ground(&mut world, (spot.0 + 4, spot.1), 9);
    let ballista = Order::PlaceBuilding { kind: BuildingKind::Ballista, tile: spot, turned: false, editor: false };
    world.step(std::slice::from_ref(&ballista));
    assert!(!world.buildings.iter().any(|b| b.kind == BuildingKind::Ballista), "built without a wood workshop");
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::WoodWorkshop, tile: (bx - 8, by + 6), turned: false, editor: true }]);
    world.step(&[ballista]);
    let b = world.buildings.iter().position(|b| b.kind == BuildingKind::Ballista).expect("not built with a wood workshop");
    let center = world.buildings[b].center();
    world.spawn_zombie_as(center + vec2(3.0, 0.0), Breed::Walker, true);
    world.spawn_zombie_as(center + vec2(7.0, 1.0), Breed::Fat, true);
    for zombie in &mut world.zombies {
        zombie.speed = 0.0;
    }
    let (walker, fat) = (world.zombies[0].hp, world.zombies[1].hp);
    let unrest = world.unrest;
    let weapon = BuildingKind::Ballista.stats().weapon.as_ref().unwrap();
    world.step(&[]);
    assert_eq!(world.zombies[0].hp, walker, "the bolt went to the walker");
    assert_eq!(world.zombies[1].hp, fat - after_armor(weapon.damage, Breed::Fat.stats().armor), "the fat one wasn't hit");
    assert_eq!(world.unrest, unrest + weapon.noise as u64, "the shot made no noise");
    // Reloading: no second bolt before `reload_seconds`.
    for _ in 0..weapon.reload - 1 {
        world.step(&[]);
    }
    assert_eq!(world.zombies[1].hp, fat - after_armor(weapon.damage, Breed::Fat.stats().armor), "a second bolt too soon");

    // The machine gun tower: the nearest one, again and again.
    world.zombies.clear();
    world.buildings[b].off = true;
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::MachineGunTower, tile: (spot.0, spot.1 + 4), turned: false, editor: true }]);
    let tower = world.buildings.last().unwrap().center();
    world.spawn_zombie_as(tower + vec2(2.0, 0.0), Breed::Fat, true);
    world.spawn_zombie_as(tower + vec2(5.0, 0.0), Breed::Fat, true);
    for zombie in &mut world.zombies {
        zombie.speed = 0.0;
    }
    let fat = world.zombies[0].hp;
    for _ in 0..TICKS_PER_SECOND as u32 {
        world.step(&[]);
    }
    let mg = BuildingKind::MachineGunTower.stats().weapon.as_ref().unwrap();
    let hits = (fat - world.zombies[0].hp) / after_armor(mg.damage, Breed::Fat.stats().armor);
    assert!(hits >= 4, "only {hits} shots in a second");
    assert_eq!(world.zombies[1].hp, fat, "it shot the one further away");
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
    let damage = after_armor(stats.trap_damage, Breed::Fat.stats().armor);
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

/// A house of doom does nothing while nothing happens – one of your units right next to it doesn't wake it (TAB's
/// houses only hear and feel). Every `guard_every_hours`, each at its own hour, it tops up its guards: fewer than
/// `guards_per_size` × its size around it (itself counted, like TAB) – a batch comes out and idles there.
#[test]
fn a_house_of_doom_keeps_its_guards() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let d = doom_house(&mut world);
    let inside = world.doom[d].inside;
    assert!(inside > 10, "only {inside} zombies inside");
    let center = world.doom[d].center();
    bait(&mut world, UnitKind::Soldier, center + vec2(-2.5, 0.0));
    for _ in 0..40 {
        world.step(&[]);
    }
    assert!(world.zombies.is_empty(), "a unit close by let zombies out");
    world.units.clear();

    // Within the next 12 hours its hour comes – a medium house (2) with nobody around: a batch of 1 to 3.
    let doom = &data::get().doom;
    for _ in 0..doom.guard_every_hours * HOUR_TICKS + 1 {
        world.step(&[]);
    }
    let guards = (inside - world.doom[d].inside) as usize;
    assert!((1..=3).contains(&guards), "{guards} guards came out");
    assert_eq!(world.zombies.len() + world.sleep.len(), guards);
    assert!(world.zombies.iter().all(|z| z.lure.is_none() && !z.siege), "the guards went somewhere");
}

/// From day `raids_from_day` on, every `raid_every_days` days – each house on its own day – a batch marches on the
/// command center, like a wave. Not before.
#[test]
fn a_house_of_doom_raids_the_base_from_day_20() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    world.events.clear();
    let d = doom_house(&mut world);
    let (id, inside) = (world.doom[d].id as u64, world.doom[d].inside);
    let doom = &data::get().doom;
    // Its day before day 20: nothing.
    let early = (0..doom.raids_from_day).rev().find(|day| (day + id).is_multiple_of(doom.raid_every_days)).unwrap();
    world.tick = early * DAY_TICKS;
    world.step(&[]);
    assert_eq!(world.doom[d].inside, inside, "it raided before day {}", doom.raids_from_day);
    // Its first day from 20 on: a batch of 1 to 3 (a medium house) heads for the command center.
    let raid = (doom.raids_from_day..).find(|day| (day + id).is_multiple_of(doom.raid_every_days)).unwrap();
    world.tick = raid * DAY_TICKS;
    world.step(&[]);
    let out = (inside - world.doom[d].inside) as usize;
    assert!((1..=3).contains(&out), "{out} came out for the raid");
    assert_eq!(world.zombies.len(), out);
    assert!(world.zombies.iter().all(|z| z.siege && !z.final_wave), "the raid doesn't head for the command center");
    // The same day again: no second raid.
    for _ in 0..doom.batch_every * 3 {
        world.step(&[]);
    }
    assert_eq!(world.zombies.len(), out, "it raided twice on one day");
}

/// What rouses a village is noise (`sound.rs`): a house listens every `places_listen_seconds` like a zombie with its
/// `awareness` – heard, a batch × `heard_batch` comes out and goes to the middle of the cell it heard. A sound so quiet
/// that the chance is 0 % does nothing. The guards of loot the same.
#[test]
fn noise_rouses_a_village_and_its_zombies_go_there() {
    let (mut world, _) = quiet_world();
    let d = doom_house(&mut world);
    let center = world.doom[d].center();
    world.add_loot(center + vec2(-6.0, 6.0), LootKind::Wood, 10, 5);
    let at = center + vec2(-10.0, 0.0);
    let middle = vec2((at.x / 4.0).floor() * 4.0 + 2.0, (at.y / 4.0).floor() * 4.0 + 2.0);
    let beat = data::get().sound.places_listen as usize;
    world.make_sound(at, 1);
    for _ in 0..beat {
        world.step(&[]);
    }
    assert!(world.zombies.is_empty() && world.loot[0].guards == 5, "a quiet sound roused them");

    // Loud enough for both to hear it for sure, at every listening until it has halved a few times.
    world.make_sound(at, 4000);
    for _ in 0..beat * 2 {
        world.step(&[]);
    }
    assert_eq!(world.loot[0].guards, 0, "the loud sound didn't raise the guards");
    // A batch × 0.5 of a medium house is 1 or 2 – two listenings, at most 4.
    let out = world.zombies.len() - 5;
    assert!((1..=4).contains(&out), "{out} came out of the house");
    assert!(world.zombies.iter().all(|z| z.lure.is_some_and(|l| l.to == middle)), "they don't go to the sound");
}

/// Your units shoot a house of doom when no zombie is in range. A hit lets a batch × `hit_batch` out – they go for the
/// closest of your units within `hit_reach` tiles. Shot down, it drops loot where it stood, its tiles are open again –
/// and whatever it hadn't let out yet never comes (TAB's).
#[test]
fn a_shot_down_house_drops_loot() {
    let (mut world, _) = quiet_world();
    let d = doom_house(&mut world);
    let (inside, center, tile) = (world.doom[d].inside, world.doom[d].center(), world.doom[d].tile);
    // Its wall 7.5 tiles away, 9 from its middle: in the sniper's range and within `hit_reach`.
    let sniper = center + vec2(-9.0, 0.0);
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
    // A medium house (2) × 3: 3 to 9.
    let out = world.zombies.len() as u32;
    assert!((3..=9).contains(&out) && out < inside, "{out} came out of {inside}");
    assert!(world.zombies.iter().all(|z| z.lure.is_some_and(|l| l.to.distance(sniper) < 0.5)), "they don't go for the shooter");
    assert_eq!(world.loot.len(), 1);
    assert_eq!(world.loot[0].pos, center);
    assert_eq!(world.loot[0].guards, 0);
    assert!(!world.map.is_doom(tile.0, tile.1) && !world.map.is_blocked(tile.0 + 1, tile.1 + 1), "its tiles are still taken");
    assert_eq!(world.score.doom_razed, 1);
}

/// A hit house calls its neighbours (TAB's `OnMessage`): every house whose tiles touch the area around it – 3 times its
/// size, 4 tiles more on every side – sends a batch to the same spot. One further away doesn't. Hit again at once:
/// nothing – one batch a second.
#[test]
fn a_hit_house_calls_its_neighbours() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let d = doom_house(&mut world);
    let (tile, center) = (world.doom[d].tile, world.doom[d].center());
    clear_ground(&mut world, (tile.0 + 1, tile.1 + 1), 25);
    // A neighbour 5 tiles right of it (in the area: 3 + 3 + 4 tiles to the right), one 20 tiles left (not).
    world.step(&[
        Order::PlaceDoom { tile: (tile.0 + 8, tile.1), size: (3, 3) },
        Order::PlaceDoom { tile: (tile.0 - 20, tile.1), size: (3, 3) },
    ]);
    assert_eq!(world.doom.len(), 3);
    let soldier = center + vec2(0.0, 5.0);
    bait(&mut world, UnitKind::Soldier, soldier);
    world.units.last_mut().unwrap().hp = 1_000_000;
    let before: Vec<u32> = world.doom.iter().map(|h| h.inside).collect();
    world.hit_house(d, 10);
    let after: Vec<u32> = world.doom.iter().map(|h| h.inside).collect();
    assert!(after[0] < before[0], "the hit house let nobody out");
    assert!(after[1] < before[1], "the neighbour didn't send anyone");
    assert_eq!(after[2], before[2], "the far house sent some too");
    assert!(world.zombies.iter().all(|z| z.lure.is_some_and(|l| l.to == soldier)), "they don't all go for the soldier");
    world.hit_house(d, 10);
    assert_eq!(world.doom[0].inside, after[0], "a second batch in the same second");
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
        // Healing itself between the guards' blows, it would fall back to its own hit points.
        if let Some(u) = world.find_unit(id) {
            world.units[u].hp = 1_000_000;
        }
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

/// TAB's supplies: picked up, the command center makes that much more food per payout and power for the rest of the
/// round.
#[test]
fn supplies_make_the_command_center_make_more() {
    let (mut world, id) = quiet_world();
    world.extra_power = 0;
    let spot = world.base + vec2(-6.5, 0.5);
    clear_ground(&mut world, (spot.x as i32, spot.y as i32), 4);
    world.add_loot(spot, LootKind::Food, 20, 0);
    world.add_loot(spot + vec2(0.0, 1.0), LootKind::Energy, 20, 0);
    let center = world.buildings.iter().position(|b| b.kind == BuildingKind::CommandCenter).unwrap();
    let grid = world.power().grid_of[center].unwrap();
    let (power, food) = (world.power().made[grid], world.food_made(&world.buildings[center]));
    world.step(&[Order::Command { units: vec![id], command: Command::Move(spot), queue: false, formation: None }]);
    world.step(&[Order::Command { units: vec![id], command: Command::Move(spot + vec2(0.0, 1.0)), queue: true, formation: None }]);
    for _ in 0..400 {
        world.step(&[]);
        if world.loot.is_empty() {
            break;
        }
    }
    assert!(world.loot.is_empty(), "the supplies weren't picked up");
    assert_eq!((world.found_food, world.found_power), (20, 20));
    assert_eq!(world.power().made[grid], power + 20);
    assert_eq!(world.food_made(&world.buildings[center]), food + 20);
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
    // Not hit – it may only have healed a little meanwhile (`regen.rs`).
    assert!(hp(&world, a) >= before, "the acid followed the unit");
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
    // It stays put until it has looked around (`watch_seconds`) – a wave zombie would walk on to the command center –,
    // then it may walk: it has seen the ranger and shoots from where it is.
    world.zombies[0].speed = 0.0;
    let mut shot = false;
    for t in 0..40 {
        if t == Breed::Infested.stats().watch + 2 {
            world.zombies[0].speed = Breed::Infested.stats().speed;
        }
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
    let shots = |kind: UnitKind, breed: Breed| breed.stats().hp.div_ceil(after_armor(kind.stats().damage, breed.stats().armor));
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
}

/// A round on a map of TAB's generator (the default now): the command center stands where it put the base, its
/// zombies lie there asleep (the ones near your eyes awake) – as many as it laid out, none scattered on top – and a
/// house lets out as many as its size and the population say (`max_units`). The difficulty is its zombie population:
/// more zombies, the same land.
#[test]
fn a_tab_map_round() {
    let mut setup = MapSetup::default();
    setup.common.size = 160.0;
    let difficulty = data::get().difficulty("");
    let world = World::generate(3, &setup);
    let generated = mapgen::generate(3, &super::with_villages(&setup, difficulty));
    let (bx, by) = generated.base;
    assert_eq!(world.base, vec2(bx as f32 + 0.5, by as f32 + 0.5));
    let cc = world.buildings.iter().find(|b| b.kind == BuildingKind::CommandCenter).unwrap();
    assert_eq!(cc.center(), world.base, "the command center isn't on the base");
    let laid_out = generated.zombies.as_ref().unwrap().len();
    assert_eq!(world.sleep.len() + world.zombies.len(), laid_out, "not the zombies TAB's generator laid out");
    for house in &world.doom {
        assert_eq!(house.inside, World::doom_zombies(house.size, difficulty.population), "a house of {:?}", house.size);
        assert_eq!(house.factor, difficulty.population * setup.tab.nests);
    }
    let brutal = World::generate(3, &MapSetup { round: mapgen::RoundSetup { difficulty: "Brutal".into(), ..setup.round.clone() }, ..setup.clone() });
    let zombies = |w: &World| w.sleep.len() + w.zombies.len();
    assert!(zombies(&brutal) > zombies(&world), "{} vs. {} zombies", zombies(&brutal), zombies(&world));
    assert!(brutal.map.same_terrain(&world.map), "the difficulty changed the land");
}

/// The start area (DESIGN.md 9.1, `mapgen/start.rs`) on the plain map (TAB's generator keeps only TAB's promises –
/// `mapgen/tab.rs`), ten seeds – checked
/// with the game's own rules (`worked_tiles`), zombies aside (the start units clear them):
/// - a sawmill finds 20 forest tiles where the power reaches – in the command center's grid, or one tesla tower
///   further if the clearing is wider than the grid – and a hunter cottage 15 (`MIN_FOREST` in `start.rs`),
/// - a mine finds 15 stone tiles one tesla tower out – the barracks needs stone.
#[test]
fn the_start_area_works_on_every_map() {
    let grid = BuildingKind::CommandCenter.stats().power_radius;
    let relay = BuildingKind::TeslaTower.stats().power_radius;
    // The most tiles a building of `kind` works with its middle within `reach` of the base – of `ore`, or any it gathers.
    let best = |world: &World, kind: BuildingKind, reach: f32, ore: Option<Ore>| {
        let (w, h) = kind.footprint(false);
        let (bx, by) = (world.base.x as i32, world.base.y as i32);
        let r = reach.ceil() as i32 + 2;
        let mut most = 0;
        for y in by - r..=by + r {
            for x in bx - r..=bx + r {
                let middle = vec2(x as f32 + w as f32 / 2.0, y as f32 + h as f32 / 2.0);
                let ground = (0..h).all(|dy| (0..w).all(|dx| world.map.is_buildable(x + dx, y + dy)));
                if !ground || middle.distance(world.base) > reach {
                    continue;
                }
                let worked = world.worked_tiles(kind, (x, y), world.buildings.len());
                let n = worked.into_iter().filter(|&(tx, ty)| ore.is_none_or(|o| world.map.deposit(tx, ty).is_some_and(|d| d.ore == o))).count();
                most = most.max(n);
            }
        }
        most
    };
    // No roaming zombies: placing 65,000 of them only makes the test slow.
    let mut difficulty = data::get().difficulty("").clone();
    difficulty.roaming_zombies = 0.0;
    let plain = MapSetup { generator: Generator::Blank, ..MapSetup::default() };
    for setup in [plain] {
        let wood_reach = if setup.common.base_clear + 2.0 <= grid { grid } else { grid + relay };
        for seed in 1..=10 {
            let world = World::generate_with(seed, &setup, &difficulty);
            let name = "Blank";
            let sawmill = best(&world, BuildingKind::Sawmill, wood_reach, None);
            let hunter = best(&world, BuildingKind::HunterCottage, wood_reach, None);
            let stone = best(&world, BuildingKind::Mine, grid + relay, Some(Ore::Stone));
            assert!(sawmill >= 20, "{name} seed {seed}: a sawmill finds only {sawmill} forest tiles");
            assert!(hunter >= 15, "{name} seed {seed}: a hunter cottage finds only {hunter} forest tiles");
            assert!(stone >= 15, "{name} seed {seed}: a mine finds only {stone} stone tiles");
        }
    }
}

/// Not a real test – a tool for balancing the mini round (DESIGN.md 13, step 4): plays it at full size with a player
/// who does nothing and prints every half day how it's going and how the round ends – up to two days after the last
/// one: the final wave can take a while to arrive, through narrow passes one by one.
/// `cargo test --release balance_mini_round -- --ignored --nocapture`
/// The default difficulty. With `$env:POPULATION = "1.5"` first, TAB's zombie population is that (more zombies, bigger
/// waves). With `$env:ROAMERS = "0"` no zombies roam the map at the start – to look at the economy alone (only on our
/// generators – TAB's lays out its own).
#[test]
#[ignore]
fn balance_mini_round() {
    let setup = MapSetup::default();
    let mut difficulty = data::get().difficulty("").clone();
    if let Some(population) = std::env::var("POPULATION").ok().and_then(|w| w.parse::<f32>().ok()) {
        difficulty.population = population;
    }
    if let Some(roamers) = std::env::var("ROAMERS").ok().and_then(|r| r.parse().ok()) {
        difficulty.roaming_zombies = roamers;
    }
    let mut world = World::generate_with(1, &setup, &difficulty);
    let end = (world.days as u64 + 3) * DAY_TICKS;
    while world.tick < end && world.outcome.is_none() {
        world.step(&[]);
        if world.tick.is_multiple_of(12 * HOUR_TICKS) {
            let (day, hour) = world.clock();
            let cc = world.buildings.iter().find(|b| b.kind == BuildingKind::CommandCenter).map_or(0, |b| b.hp);
            println!(
                "day {day:2} {hour:02}:00  zombies {:5}  units {:3}  gold {:5} (+{}/h)  CC {cc}",
                world.zombies.len(),
                world.units.len(),
                world.gold,
                world.income(),
            );
        }
    }
    println!("{:?}  {:?}", world.outcome, world.score);
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

    // Deep in debt, not deserting yet: the mercenaries go, the others stay. Right after a payout, so the next one
    // (TAB's training takes long enough for one to fall in between) doesn't pay the debt off first.
    one_payout(&mut world);
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

/// Editor, before the round: the command center made neutral and taken back counts with its payout again at once –
/// not only the upkeep until a payout that never comes in the editor.
#[test]
fn a_building_taken_back_counts_its_payout_in_the_editor() {
    let mut world = World::new(3);
    world.settle();
    assert_eq!(world.tick, 0);
    let cc = world.buildings.iter().find(|b| b.kind == BuildingKind::CommandCenter).expect("no command center");
    let (id, paid) = (cc.id, cc.paid);
    assert!(paid > 0, "the command center pays nothing at the start");
    world.apply_orders(&[Order::SetOwner { thing: ThingRef::Building(id), owner: Owner::Neutral }]);
    let n = world.neutral.last().expect("not neutral").id;
    world.apply_orders(&[Order::SetOwner { thing: ThingRef::Neutral(n), owner: Owner::Colony }]);
    let back = world.buildings.iter().find(|b| b.kind == BuildingKind::CommandCenter).expect("not back");
    assert_eq!(back.paid, paid, "back in the colony it counts only its upkeep");
}

/// Editor, "Placing: owner": a building or a unit put down as neutral is neutral right away – and where nothing can
/// be put down, the thing placed before it isn't touched.
#[test]
fn the_editor_places_things_with_an_owner() {
    let (mut world, _) = quiet_world();
    let tile = (world.base.x as i32 + 12, world.base.y as i32);
    let (buildings, units) = (world.buildings.len(), world.units.len());
    let tent = Order::PlaceBuilding { kind: BuildingKind::Tent, tile, turned: false, editor: true };
    world.apply_orders(&[Order::PlaceAs { owner: Owner::Joinable, order: Box::new(tent.clone()) }]);
    assert_eq!(world.buildings.len(), buildings, "the tent is the colony's");
    assert!(matches!(world.neutral.last(), Some(n) if n.joinable && matches!(n.thing, Thing::Building { kind: BuildingKind::Tent, .. })));
    let point = world.base + vec2(12.0, 6.0);
    world.apply_orders(&[Order::PlaceAs { owner: Owner::Neutral, order: Box::new(Order::SpawnUnit { kind: UnitKind::Soldier, point }) }]);
    assert_eq!(world.units.len(), units, "the soldier is the colony's");
    assert!(matches!(world.neutral.last(), Some(n) if !n.joinable && matches!(n.thing, Thing::Unit { kind: UnitKind::Soldier, .. })));
    // The same spot again: no room – nothing new, and nobody else changes hands.
    world.apply_orders(&[Order::PlaceAs { owner: Owner::Neutral, order: Box::new(tent) }]);
    assert_eq!((world.buildings.len(), world.units.len(), world.neutral.len()), (buildings, units, 2));
}

/// The scan is the radar's: without one nothing happens. A radar charges its scans (none at the start), and while one
/// charges its grid needs more power. The scan shows a far circle of the map at once and for a while – a bit wider than
/// its ring – then it's dark again. It uses a charge, which comes back after a while.
#[test]
fn the_radar_scans_a_circle_and_costs_power() {
    let (mut world, _) = quiet_world();
    let scan = &data::get().buildings.scan;
    let far = world.base + vec2(30.0, -30.0);
    // Inside the scan's light, outside its ring: the soft edge lies beyond the ring.
    let edge = far + vec2(scan.radius + 0.5, 0.0);
    assert!(!world.vision.sees(far));
    world.apply_orders(&[Order::Scan { at: far }]);
    assert!(!world.vision.sees(far) && world.scans.is_empty(), "a scan without a radar");
    let tile = (world.base.x as i32 + 4, world.base.y as i32 + 4);
    clear_ground(&mut world, tile, 2);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::Radar, tile, turned: false, editor: true }]);
    assert!(world.scanner().is_some(), "the radar doesn't work");
    let used = |world: &World| world.power().used.iter().sum::<u32>();
    assert_eq!(world.scan_charges, 0, "a new radar is charged already");
    let charging = used(&world);
    for _ in 0..scan.every {
        world.step(&[]);
    }
    assert_eq!(world.scan_charges, 1, "the radar didn't charge a scan");
    world.apply_orders(&[Order::Scan { at: far }]);
    assert!(world.vision.sees(far) && world.vision.sees(edge), "the scan shows nothing, or not past its ring");
    assert_eq!(world.scan_charges, 0);
    assert_eq!(used(&world), charging, "charging costs no power");
    for _ in 0..scan.time + 1 {
        world.step(&[]);
    }
    assert!(!world.vision.sees(far), "still shown after the scan is over");
    // All charged: the radar takes no more for it.
    world.scan_charges = scan.charges;
    world.step(&[]);
    assert_eq!(used(&world) + scan.power, charging, "it still charges with all charges there");
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

/// Sleepers listen like the zombies awake: one that hears a sound wakes and goes to the middle of its cell.
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
    // Loud enough for a walker this close for sure, even halved a few times – heard when its block's turn comes.
    let noise = p + vec2(5.0, 0.0);
    world.make_sound(noise, 2000);
    for _ in 0..data::get().sound.listen {
        world.step(&[]);
    }
    assert!(world.sleep.is_empty(), "the sound didn't wake it");
    let zombie = world.zombies.iter().find(|z| z.id > id).expect("it isn't awake");
    let middle = vec2((noise.x / 4.0).floor() * 4.0 + 2.0, (noise.y / 4.0).floor() * 4.0 + 2.0);
    assert_eq!(zombie.lured_to(), Some(middle), "it doesn't go to the sound");
}

/// A calm stroller far from all your eyes falls asleep after a while – and wakes again with the hit points it had,
/// plus what it healed asleep (`regen.rs`): once every `regen_every_seconds` it slept.
#[test]
fn calm_zombies_fall_asleep_far_from_your_eyes() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(45.0, 0.0);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 8);
    world.spawn_zombie_as(p, Breed::Fat, true);
    world.zombies[0].hp = 10;
    // Hit just now, as far as healing goes: awake it doesn't heal – only asleep.
    world.zombies[0].hurt_at = u64::MAX / 2;
    let mut waited = 0;
    while world.sleep.is_empty() && waited < (25.0 * TICKS_PER_SECOND) as u32 {
        world.step(&[]);
        waited += 1;
    }
    assert_eq!((world.zombies.len(), world.sleep.len()), (0, 1), "it didn't fall asleep");
    // It fell asleep in the tick just gone – 5 s more asleep.
    let since = world.tick - 1;
    for _ in 0..(5.0 * TICKS_PER_SECOND) as u32 {
        world.step(&[]);
    }
    let at = world.sleep.iter().next().unwrap().pos;
    world.wake_area(at, 1.0);
    assert_eq!(world.zombies.len(), 1);
    let stats = Breed::Fat.stats();
    let times = ((world.tick - since) / data::get().units.regen_every as u64) as u32;
    let expected = 10 + times * regen::regen_amount(stats.hp, stats.regen);
    assert!(times >= 5 && expected < stats.hp, "the test needs it to heal a few times, but not to full");
    assert_eq!(world.zombies[0].hp, expected, "it woke as a new one, or didn't heal asleep");
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

/// Sleepers idle like the zombies awake (`sleep.rs`): trip after trip they drift – and far from your eyes they stay
/// asleep doing it. TAB's trips are short (`stroll` 0–1, so under 2 tiles an axis): in 2 minutes most get more than a
/// tile from where any of them lay.
#[test]
fn sleepers_stroll_too() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    // No events: two minutes are long enough for the first stragglers to come.
    no_more_zombies(&mut world);
    let p = world.base + vec2(50.0, 0.0);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 14);
    for k in 0..30 {
        world.lay_to_sleep(p + vec2((k % 6) as f32 * 0.4, (k / 6) as f32 * 0.4), Breed::Walker);
    }
    let start: Vec<Vec2> = world.sleep.iter().map(|s| s.pos).collect();
    for _ in 0..(120.0 * TICKS_PER_SECOND) as u32 {
        world.step(&[]);
    }
    assert_eq!((world.sleep.len(), world.zombies.len()), (30, 0), "they woke without a reason");
    let moved = world.sleep.iter().filter(|s| start.iter().all(|a| a.distance(s.pos) > 1.0)).count();
    assert!(moved >= 15, "only {moved} of 30 sleepers strolled");
}

/// A place you've emptied stays empty for a while, like in TAB: with trips under 2 tiles the sleepers 20–28 tiles
/// around don't stroll into your ranger's sight in 2 minutes – and at no time does one lie asleep where it sees.
#[test]
fn an_emptied_place_fills_up_only_slowly() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let q = world.base + vec2(45.0, 0.0);
    clear_ground(&mut world, (q.x as i32, q.y as i32), 32);
    let id = bait(&mut world, UnitKind::Ranger, q);
    let u = world.find_unit(id).unwrap();
    world.units[u].hp = 1_000_000;
    let sight = world.units[u].sight();
    // A ring of sleepers 20–28 tiles away – mostly out of what the ranger wakes (its sight + 8).
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
    let near = world.zombies.iter().filter(|z| z.pos.distance(q) <= sight).count();
    assert_eq!(near, 0, "{near} strolled into the ranger's sight – TAB's trips are shorter");
}


/// TAB's neutral things (`neutral.rs`): a joinable tent blocks its tiles and does nothing – it's not one of the
/// colony's – until one of your units comes within a tile of it; then it's an ordinary building of the colony, with the
/// hit points and the tag it had, and the screen says so. A neutral one (not joinable) stays where it is.
#[test]
fn a_joinable_building_joins_when_a_unit_comes_close() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    let tile = (bx + 10, by + 10);
    clear_ground(&mut world, (tile.0 + 1, tile.1 + 1), 8);
    let tag = Tag { name_id: "outpost".into(), name: "Old outpost".into(), ..Tag::default() };
    world.add_neutral(Thing::Building { kind: BuildingKind::Tent, tile, turned: false }, true, 50, false, Some(Box::new(tag.clone())));
    world.add_neutral(Thing::Building { kind: BuildingKind::Tent, tile: (tile.0 - 5, tile.1), turned: false }, false, 100, false, None);
    world.step(&[]);
    assert_eq!(world.neutral.len(), 2);
    assert!(world.map.is_blocked(tile.0, tile.1) && !world.map.is_buildable(tile.0, tile.1), "its tiles are free");
    let tents = |w: &World| w.buildings.iter().filter(|b| b.kind == BuildingKind::Tent).count();
    assert_eq!(tents(&world), 0, "a neutral tent counts as the colony's");
    // A unit 3 tiles away: nothing. Then right next to both.
    bait(&mut world, UnitKind::Soldier, vec2(tile.0 as f32 + 0.5, tile.1 as f32 - 3.0));
    for _ in 0..20 {
        world.step(&[]);
    }
    assert_eq!(world.neutral.len(), 2, "it joined from 3 tiles away");
    // On the tile left of it – within a tile of its edge.
    world.units[0].pos = vec2(tile.0 as f32 - 0.5, tile.1 as f32 + 0.5);
    for _ in 0..20 {
        world.step(&[]);
    }
    assert_eq!(world.neutral.len(), 1, "the joinable tent didn't join, or the neutral one did");
    let tent = world.buildings.iter().find(|b| b.kind == BuildingKind::Tent).expect("no tent of the colony");
    assert_eq!((tent.tile, tent.hp, tent.tag.as_deref()), (tile, BuildingKind::Tent.stats().hp / 2, Some(&tag)));
    assert!(world.notices.iter().any(|n| matches!(n, Notice::Joined { name } if name == "Old outpost")));
}

/// The editor's owner: a unit of the colony made neutral keeps its hit points, veteran and tag – and back in the colony
/// too. Neutral it isn't in the colony's list (nobody selects it, the zombies don't see it).
#[test]
fn the_owner_can_change_and_back() {
    let (mut world, id) = quiet_world();
    let tag = Tag { name: "Sarge".into(), minimap: true, ..Tag::default() };
    world.apply_orders(&[
        Order::SetTag { thing: ThingRef::Unit(id), tag: tag.clone() },
        Order::SetMark { thing: ThingRef::Unit(id), on: true },
        Order::SetHealth { thing: ThingRef::Unit(id), percent: 40 },
    ]);
    let unit = world.units.iter().find(|u| u.id == id).unwrap();
    assert!(unit.veteran && unit.hp == unit.max_hp() * 40 / 100);
    assert_eq!(world.minimap_marks().len(), 1);
    let (_, _, neutral) = world.next_ids();
    world.apply_orders(&[Order::SetOwner { thing: ThingRef::Unit(id), owner: Owner::Neutral }]);
    assert!(world.units.iter().all(|u| u.id != id), "still one of the colony's");
    assert_eq!(world.neutral.len(), 1);
    assert_eq!(world.neutral[0].id, neutral, "not the number the editor expects");
    assert_eq!((world.neutral[0].hp_percent, world.neutral[0].veteran), (40, true));
    let (_, unit_id, _) = world.next_ids();
    world.apply_orders(&[Order::SetOwner { thing: ThingRef::Neutral(neutral), owner: Owner::Colony }]);
    let back = world.units.iter().find(|u| u.id == unit_id).expect("not back in the colony");
    assert!(back.veteran && back.hp == back.max_hp() * 40 / 100 && back.tag.as_deref() == Some(&tag));
}

/// A script of these actions, none with a condition or code.
fn script(actions: Vec<Action>) -> Script {
    actions.into_iter().map(Step::new).collect()
}

/// A step that sets a variable to a number, or adds it – code like `name = value` / `name += value`.
fn var_step(name: &str, add: bool, value: i32) -> Step {
    let how = if add { expr::Assign::Add } else { expr::Assign::Set };
    Step::code(vec![expr::Stmt::Set { name: name.into(), how, value: Expr::number(value) }])
}

/// A trigger with these actions for this event.
fn trigger_of(event: TriggerEvent, actions: Script) -> Trigger {
    Trigger { actions, ..Trigger::new("test", event) }
}

/// The map's story (`story.rs`): the start trigger fires in the first tick – gold, a message, a variable; a victory goal
/// on the variable wins the round once a trigger raised it far enough – and then the victory trigger fires.
#[test]
fn a_story_starts_counts_and_wins() {
    let mut world = World::new(3);
    world.events.clear();
    world.story = Story {
        triggers: vec![
            trigger_of(
                TriggerEvent::Start,
                vec![
                    Step::new(Action::Resources { gold: 500, goods: vec![(Resource::Stone, 7)] }),
                    Step::new(Action::Notify { text: "Hold the line".into() }),
                    var_step("keys", false, 1),
                ],
            ),
            trigger_of(TriggerEvent::Won, script(vec![Action::Notify { text: "Well done".into() }])),
            trigger_of(TriggerEvent::Button, vec![var_step("keys", true, 1)]),
        ],
        goals: vec![Goal { text: "Find 3 keys".into(), condition: Condition::Variable { name: "keys".into(), at_least: 3 }, defeat: false }],
        command: Some(CustomCommand { label: "Dig".into(), text: String::new(), uses: 2 }),
        variables: vec![Variable { name: "keys".into(), kind: VarKind::Number, start: Value::Number(0) }],
    };
    let (gold, stone) = (world.gold, world.stock.get(Resource::Stone));
    world.step(&[]);
    assert_eq!((world.gold, world.stock.get(Resource::Stone)), (gold + 500, stone + 7));
    assert!(world.notices.iter().any(|n| matches!(n, Notice::Message { text } if text == "Hold the line")));
    assert_eq!((world.variable("keys"), world.goal_progress(0)), (1, 33));
    // The command center's button, twice – a third time does nothing.
    world.step(&[Order::CustomCommand]);
    assert_eq!(world.outcome, None);
    world.step(&[Order::CustomCommand]);
    assert!(matches!(world.outcome, Some(Outcome::Won { .. })), "the goal didn't win");
    assert!(world.notices.iter().any(|n| matches!(n, Notice::Message { text } if text == "Well done")), "no victory script");
    assert!(!world.command_ready());
}

/// The weather (`weather.rs`): a round starts with its landscape's – snow on AL, rain on VO, clear on the others –, and
/// a script changes it (here the start script, so it never snows on this AL map).
#[test]
fn the_weather_comes_with_the_landscape_and_scripts_change_it() {
    let mut world = World::new(3);
    for (look, weather) in [("AL", Weather::Snow), ("VO", Weather::Rain), ("BR", Weather::Clear), ("", Weather::Clear)] {
        world.set_look(look);
        assert_eq!(world.weather, weather, "on {look:?}");
    }
    world.set_look("AL");
    world.events.clear();
    world.story.triggers = vec![trigger_of(TriggerEvent::Start, script(vec![Action::Weather { weather: Weather::Clear }]))];
    world.step(&[]);
    assert_eq!(world.weather, Weather::Clear, "the start script didn't stop the snow");
    world.run(&script(vec![Action::Weather { weather: Weather::Rain }]));
    assert_eq!(world.weather, Weather::Rain);
}

/// A choice of a script waits for the player: their pick comes as an order and runs its script. A random action
/// runs exactly one of its scripts. A defeat goal loses the round.
#[test]
fn a_story_lets_the_player_choose() {
    let (mut world, _) = quiet_world();
    world.events.clear();
    let gold = |n: i32| script(vec![Action::Resources { gold: n, goods: Vec::new() }]);
    world.story.triggers = vec![trigger_of(
        TriggerEvent::Start,
        script(vec![
            Action::Choose { text: "Pay the raiders?".into(), options: vec![("Pay".into(), gold(-50)), ("Fight".into(), script(vec![Action::Lose]))] },
            Action::Random { options: vec![(1, gold(1)), (1, gold(2))] },
        ]),
    )];
    world.tick = 0;
    let before = world.gold;
    world.step(&[]);
    let rolled = world.gold - before;
    assert!(rolled == 1 || rolled == 2, "the random action ran {rolled}");
    assert_eq!(world.choices.len(), 1);
    world.step(&[Order::Choose { index: 0 }]);
    assert!(world.choices.is_empty() && world.gold == before + rolled - 50, "the choice didn't run its script");
    world.story.goals = vec![Goal { text: "Don't lose 2 units".into(), condition: Condition::Day(1), defeat: true }];
    world.step(&[]);
    assert!(matches!(world.outcome, Some(Outcome::Lost { .. })), "the defeat goal didn't lose");
}

/// An event's triggers fire every time it comes, its "done" triggers once when it has come every time. A neutral
/// thing's triggers fire when it joins.
#[test]
fn events_and_joins_fire_their_triggers() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    world.events.clear();
    let add = |n: i32| vec![var_step("n", true, n)];
    let hour = next_hour(&world);
    let mut event = Event { every_hours: 1, times: 2, ..one_group(hour, WaveFrom::Sides(1), &[(Breed::Walker, 1)]) };
    event.name = "Raid".into();
    let mut story = world.story.clone();
    story.triggers = vec![
        trigger_of(TriggerEvent::EventComes { event: "Raid".into() }, add(1)),
        trigger_of(TriggerEvent::EventDone { event: "Raid".into() }, add(100)),
    ];
    world.step(&[Order::AddEvent(event), Order::SetStory { story }]);
    for _ in 0..3 * HOUR_TICKS {
        world.step(&[]);
    }
    assert_eq!(world.variable("n"), 102);

    let p = world.base + vec2(10.5, 0.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 3);
    let tag = Tag { name_id: "hostage".into(), ..Tag::default() };
    world.add_neutral(Thing::Unit { kind: UnitKind::Soldier, pos: (p.x, p.y) }, true, 100, false, Some(Box::new(tag)));
    let mut story = world.story.clone();
    story.triggers.push(trigger_of(TriggerEvent::Join { name_id: "hostage".into() }, add(1000)));
    world.step(&[Order::SetStory { story }]);
    bait(&mut world, UnitKind::Ranger, p + vec2(0.0, 1.0));
    for _ in 0..20 {
        world.step(&[]);
    }
    assert!(world.neutral.is_empty() && world.variable("n") == 1102, "the join script didn't run");
}

/// A trigger fires only when it is on and all its conditions hold; triggers of one event run in the order of the list,
/// so a later one sees what an earlier one did. A yes/no variable stays 0 or 1.
#[test]
fn triggers_check_their_conditions_and_keep_their_order() {
    let (mut world, _) = quiet_world();
    world.events.clear();
    let set = |name: &str, value: i32| vec![var_step(name, false, value)];
    let mut off = trigger_of(TriggerEvent::Start, set("off", 1));
    off.on = false;
    let mut needs = trigger_of(TriggerEvent::Start, set("needs", 1));
    needs.conditions = vec![Condition::Variable { name: "first".into(), at_least: 1 }.to_expr()];
    let mut never = trigger_of(TriggerEvent::Start, set("never", 1));
    never.conditions = vec![Condition::Day(2).to_expr()];
    world.story = Story {
        triggers: vec![off, trigger_of(TriggerEvent::Start, set("first", 1)), needs, never, trigger_of(TriggerEvent::Start, set("flag", 5))],
        variables: vec![Variable { name: "flag".into(), kind: VarKind::Flag, start: Value::Flag(false) }],
        ..Story::default()
    };
    world.tick = 0;
    world.step(&[]);
    let got: Vec<i32> = ["off", "first", "needs", "never", "flag"].iter().map(|n| world.variable(n)).collect();
    assert_eq!(got, [0, 1, 1, 0, 1], "off / first / needs / never / flag");
}

/// Renaming an event or a name ID in the editor takes its triggers along – unless another one still has the old name.
#[test]
fn triggers_follow_the_names_of_events_and_things() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    world.events.clear();
    let mut event = Event::new("Raid", 24, WaveFrom::Sides(1), Breed::Walker, 1);
    world.step(&[Order::AddEvent(event.clone())]);
    let tag = Tag { name_id: "boss".into(), ..Tag::default() };
    world.add_neutral(Thing::Unit { kind: UnitKind::Soldier, pos: (5.5, 5.5) }, false, 100, false, Some(Box::new(tag)));
    let id = world.neutral[0].id;
    let mut story = world.story.clone();
    story.triggers = vec![
        Trigger::new("a", TriggerEvent::EventComes { event: "Raid".into() }),
        Trigger::new("b", TriggerEvent::EventDone { event: "Raid".into() }),
        Trigger::new("c", TriggerEvent::Join { name_id: "boss".into() }),
    ];
    world.step(&[Order::SetStory { story }]);
    event.name = "Assault".into();
    world.step(&[Order::SetEvent { index: 0, event }, Order::SetTag { thing: ThingRef::Neutral(id), tag: Tag { name_id: "chief".into(), ..Tag::default() } }]);
    let events: Vec<&TriggerEvent> = world.story.triggers.iter().map(|t| &t.event).collect();
    assert_eq!(
        events,
        [
            &TriggerEvent::EventComes { event: "Assault".into() },
            &TriggerEvent::EventDone { event: "Assault".into() },
            &TriggerEvent::Join { name_id: "chief".into() }
        ]
    );
}

/// Code and expressions work on the world: the variables keep their kind, a division by 0 gives 0, the built-in values
/// and functions of `funcs.rs` read and change the colony.
#[test]
fn code_and_expressions_run_in_the_world() {
    let (mut world, _) = quiet_world();
    world.events.clear();
    let v = |name: &str, kind| Variable { name: name.into(), kind, start: Value::default() };
    world.story.variables = vec![v("keys", VarKind::Number), v("met", VarKind::Flag), v("who", VarKind::Text)];
    let run = |w: &mut World, text: &str| {
        let variables = w.story.variables.clone();
        let code = parse_code(text, Scope { variables: &variables }).unwrap_or_else(|e| panic!("{text}: {e}"));
        w.exec(&code);
    };
    let holds = |w: &mut World, text: &str| {
        let variables = w.story.variables.clone();
        let e = parse_condition(text, Scope { variables: &variables }).unwrap_or_else(|e| panic!("{text}: {e}"));
        w.holds(&e)
    };
    run(&mut world, "keys = 7; keys += 3; keys -= 1; keys = keys * 2 / 3; who = \"a\"; who += \"b\"; met = true");
    assert_eq!((world.variable("keys"), world.variable_value("who"), world.variable_value("met")), (6, Value::Text("ab".into()), Value::Flag(true)));
    run(&mut world, "keys = 5 / (keys - 6)");
    assert_eq!(world.variable("keys"), 0, "a division by 0 gives 0");
    assert!(holds(&mut world, "met & who = \"ab\" & keys = 0") && !holds(&mut world, "!met | keys > 0"));

    // The clock.
    world.tick = DAY_TICKS * 2 + HOUR_TICKS * 5;
    assert!(holds(&mut world, "Day = 3 & Hour = 5 & TotalHours = 77"));
    // The colony: gold and goods change, below 0 they're taken, units and tagged things are counted.
    let (gold, wood) = (world.gold, world.stock.get(Resource::Wood));
    run(&mut world, "GiveGold(50); GiveWood(7)");
    assert_eq!((world.gold, world.stock.get(Resource::Wood)), (gold + 50, wood + 7));
    run(&mut world, "GiveGold(-50); GiveWood(-1000000)");
    assert_eq!((world.gold, world.stock.get(Resource::Wood)), (gold, 0));
    let units = world.units.len();
    assert!(holds(&mut world, &format!("Count(\"Unit\") = {units} & Gold = {gold}")));
    assert!(holds(&mut world, "IsDead(\"hero\")"));
    let unit = world.units[0].id;
    world.step(&[Order::SetTag { thing: ThingRef::Unit(unit), tag: Tag { name_id: "hero".into(), ..Tag::default() } }]);
    assert!(!holds(&mut world, "IsDead(\"hero\")") && holds(&mut world, "!EventDone(\"nothing\")"));
    // The dice are the world's: a roll is in its range.
    for _ in 0..50 {
        assert!(holds(&mut world, "D100 >= 1 & D100 <= 100 & Random(3) <= 3 & Random(-2, 2) >= -2 & Random(-2, 2) <= 2"));
    }
    run(&mut world, "EndGameWon()");
    assert_eq!(world.ending, Some(story::Ending::Won));
}

/// A step's condition skips its action and its code; the code runs after the action – after the option's script of a
/// random action, after the player's pick of a choice.
#[test]
fn steps_have_conditions_and_code() {
    let (mut world, _) = quiet_world();
    world.events.clear();
    world.story.variables = vec![Variable { name: "n".into(), kind: VarKind::Number, start: Value::Number(0) }];
    let scope = |w: &World| w.story.variables.clone();
    let variables = scope(&world);
    let sc = Scope { variables: &variables };
    let when = |t: &str| Some(parse_condition(t, sc).unwrap());
    let code = |t: &str| parse_code(t, sc).unwrap();
    let said = |w: &World, what: &str| w.notices.iter().any(|n| matches!(n, Notice::Message { text } if text == what));
    let note = |text: &str| Action::Notify { text: text.into() };
    let steps = vec![
        Step { action: note("a"), when: when("n = 1"), code: code("n += 10") },
        Step { action: note("b"), when: None, code: code("n = n + 5") },
        Step { action: note("c"), when: when("n = 5"), code: code("n += 1") },
        Step { action: Action::Random { options: vec![(1, vec![var_step("n", true, 100)])] }, when: None, code: code("n = n * 2") },
        Step { action: Action::Choose { text: "?".into(), options: vec![("go".into(), vec![var_step("n", true, 1000)])] }, when: None, code: code("n += 7") },
    ];
    world.run(&steps);
    assert!(!said(&world, "a") && said(&world, "b") && said(&world, "c"));
    // 0 -> 5 -> 6 -> (6 + 100) * 2: the code of the random action came after its script. The choice waits.
    assert_eq!(world.variable("n"), 212);
    assert_eq!(world.choices.len(), 1);
    world.step(&[Order::Choose { index: 0 }]);
    assert_eq!(world.variable("n"), 212 + 1000 + 7, "the choice's code runs after the pick");
}

/// Like TAB's: units and zombies heal themselves – `regen` of their full hit points (rounded up) every
/// `regen_every_seconds`, but only `regen_after_seconds` after the last hit.
#[test]
fn units_and_zombies_heal_themselves() {
    let (mut world, _) = quiet_world();
    no_more_zombies(&mut world);
    let values = &data::get().units;
    let p = world.base + vec2(30.0, 0.0);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 4);
    world.spawn_zombie_as(p, Breed::Walker, true);
    let z = world.zombies.len() - 1;
    let tick = world.tick;
    world.zombies[z].hurt(30, tick);
    world.units[0].take_hit(100, tick);
    let (zombie_hp, unit_hp) = (world.zombies[z].hp, world.units[0].hp);
    for _ in 0..values.regen_after - 1 {
        world.step(&[]);
    }
    assert_eq!((world.zombies[z].hp, world.units[0].hp), (zombie_hp, unit_hp), "healed right after the hit");
    for _ in 0..values.regen_every + 1 {
        world.step(&[]);
    }
    let walker = Breed::Walker.stats();
    let soldier = world.units[0].stats();
    assert_eq!(world.zombies[z].hp, zombie_hp + regen::regen_amount(walker.hp, walker.regen), "the zombie didn't heal once");
    assert_eq!(world.units[0].hp, unit_hp + regen::regen_amount(world.units[0].max_hp(), soldier.regen), "the unit didn't heal once");
    for _ in 0..(15.0 * TICKS_PER_SECOND) as u32 {
        world.step(&[]);
    }
    assert_eq!(world.zombies[z].hp, walker.hp, "not healed to full – or beyond it");
}

/// Like TAB's (`EndGameIfInfected`): the command center's shield broken, it's infected – and the round is lost, with
/// half its life left.
#[test]
fn an_infected_command_center_loses_the_round() {
    let (mut world, _) = quiet_world();
    assert_eq!(world.buildings[0].kind, BuildingKind::CommandCenter);
    let shield = world.buildings[0].shield;
    assert!(!world.buildings[0].take_damage(shield - 1), "infected too early");
    world.step(&[]);
    assert_eq!(world.outcome, None);
    assert!(world.buildings[0].take_damage(1), "the broken shield didn't infect it");
    world.step(&[]);
    assert!(matches!(world.outcome, Some(Outcome::Lost { .. })), "{:?}", world.outcome);
    assert!(world.buildings[0].hp > 0, "it should still stand");
}

/// Like TAB's hunter: forest, grass and dirt all have game – each its own share of `gather_per_tile`, by the map
/// (`Modifiers::hunting_*`).
#[test]
fn hunters_hunt_on_forest_grass_and_dirt() {
    let (mut world, _) = quiet_world();
    let tile = (world.base.x as i32, world.base.y as i32 + 16);
    let kind = BuildingKind::HunterCottage;
    let range = kind.stats().gather_range;
    clear_ground(&mut world, tile, range + 2);
    let food = |world: &World| world.gather_rate(kind, tile, world.buildings.len(), 1000)[Resource::Food.index()];
    let per_tile = (kind.stats().gather_per_tile * 1000.0).round() as u64;
    let m = world.modifiers;
    for (terrain, percent) in [(Terrain::Grass, m.hunting_grass), (Terrain::Dirt, m.hunting_dirt), (Terrain::Forest, m.hunting_forest)] {
        let tiles = (tile.1 - range..=tile.1 + range).flat_map(|y| (tile.0 - range..=tile.0 + range).map(move |x| (x, y))).collect();
        world.step(&[Order::PaintTerrain { terrain, tiles }]);
        let worked = world.worked_tiles(kind, tile, world.buildings.len()).len() as u64;
        assert!(worked > 20, "{terrain:?}: only {worked} tiles");
        assert_eq!(food(&world), worked * (per_tile * percent as u64 / 100), "{terrain:?}");
    }
    world.modifiers.hunting_forest = 0;
    assert_eq!(food(&world), 0, "a landscape without game in the forest");
}

/// TAB's giant sweeps: its blow lands on its target and everyone in front of it within `sweep_radius` – units and
/// other zombies alike – but not behind it.
#[test]
fn the_giant_sweeps_everyone_in_front_of_it() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    no_more_zombies(&mut world);
    let p = world.base + vec2(20.5, 0.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 6);
    world.spawn_zombie_as(p, Breed::Giant, true);
    let giant = world.zombies.len() - 1;
    world.spawn_zombie_as(p + vec2(1.5, -1.0), Breed::Walker, true);
    world.spawn_zombie_as(p + vec2(6.0, 0.0), Breed::Walker, true);
    let (near_walker, far_walker) = (giant + 1, giant + 2);
    for offset in [vec2(1.2, 0.0), vec2(2.0, 1.2), vec2(-2.0, 0.0)] {
        world.spawn_unit(UnitKind::Soldier, p + offset);
    }
    let mut infected = Vec::new();
    world.sweep(giant, Hit::Unit(0), &mut infected);
    let full = UnitKind::Soldier.stats().hp;
    let blow = after_armor(Breed::Giant.stats().damage, UnitKind::Soldier.stats().armor);
    assert_eq!(world.units[0].hp, full - blow, "the target");
    assert_eq!(world.units[1].hp, full - blow, "the one beside it");
    assert_eq!(world.units[2].hp, full, "the one behind the giant");
    assert!(!world.zombies[near_walker].is_alive(), "the walker in front");
    assert!(world.zombies[far_walker].is_alive(), "the walker out of reach");
    assert!(world.zombies[giant].is_alive());
}

/// TAB's push (`FactorPush`): whom the giant's blow kills flies away from it and slides to a stop – a unit's corpse
/// as well as a walker, dying. Whom it only hurts stays where it is.
#[test]
fn the_giants_blow_throws_the_killed_away() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    no_more_zombies(&mut world);
    let p = world.base + vec2(20.5, 0.5);
    clear_ground(&mut world, (p.x as i32, p.y as i32), 8);
    world.spawn_zombie_as(p, Breed::Giant, true);
    let giant = world.zombies.len() - 1;
    world.spawn_zombie_as(p + vec2(1.5, -1.0), Breed::Walker, true);
    world.spawn_unit(UnitKind::Ranger, p + vec2(1.2, 0.0));
    world.spawn_unit(UnitKind::Soldier, p + vec2(2.0, 1.2));
    world.units[0].hp = 1; // the ranger dies, the soldier only gets hurt
    let walker_at = world.zombies[giant + 1].pos;
    let (ranger_at, soldier_at) = (world.units[0].pos, world.units[1].pos);
    let mut infected = Vec::new();
    world.sweep(giant, Hit::Unit(0), &mut infected);
    world.remove_dead();
    assert_eq!(world.corpses.len(), 1, "the ranger lies there");
    assert_eq!(world.dying.len(), 1, "the walker dies");
    assert_eq!(world.units[0].pos, soldier_at, "the living aren't pushed");
    for _ in 0..crate::data::get().zombies.push + 5 {
        world.move_fallen();
    }
    // TAB: 150 / mass 1 / 12 = 12.5 tiles a second, slowing evenly.
    let ranger = world.corpses[0].pos;
    let walker = world.dying[0].pos;
    assert!(ranger.distance(ranger_at) > 2.0, "the ranger flew only {}", ranger.distance(ranger_at));
    assert!(walker.distance(walker_at) > 2.0, "the walker flew only {}", walker.distance(walker_at));
    // Away from the giant.
    assert!((ranger - p).length() > (ranger_at - p).length());
    assert!((walker - p).length() > (walker_at - p).length());
    let still = world.corpses[0].pos;
    world.move_fallen();
    assert_eq!(world.corpses[0].pos, still, "it lies still now");
}

/// TAB's stakes: everyone on them – zombies and your units – walks half as fast (`slow_percent`).
#[test]
fn stakes_slow_everyone_down() {
    let (mut world, _) = quiet_world();
    let tile = (world.base.x as i32 + 12, world.base.y as i32);
    clear_ground(&mut world, tile, 2);
    world.step(&[Order::PlaceBuilding { kind: BuildingKind::Stakes, tile, turned: false, editor: true }]);
    assert_eq!(world.buildings.last().unwrap().kind, BuildingKind::Stakes, "no stakes");
    let on = vec2(tile.0 as f32 + 0.5, tile.1 as f32 + 0.5);
    let slow = BuildingKind::Stakes.stats().slow_percent;
    assert_eq!(world.ground_pace(on), (100 - slow) as f32 / 100.0);
    assert_eq!(world.ground_pace(on + vec2(1.0, 0.0)), 1.0);
}

/// Dismissed, a unit gives half its wood and iron back – rounded up, half of 5 wood is 3 – and none of its gold.
#[test]
fn dismissing_gives_half_the_goods_back() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let p = world.base + vec2(6.5, 6.5);
    world.spawn_unit(UnitKind::Ranger, p);
    world.spawn_unit(UnitKind::Sniper, p + vec2(1.0, 0.0));
    world.spawn_unit(UnitKind::MercRanger, p + vec2(2.0, 0.0));
    let ids: Vec<u32> = world.units.iter().map(|u| u.id).collect();
    world.stock.set(Resource::Wood, 0);
    world.stock.set(Resource::Iron, 0);
    let gold = world.gold;
    world.step(&[Order::Dismiss { units: ids }]);
    let back = |n: u32| n.div_ceil(2);
    let (r, s, m) = (UnitKind::Ranger.stats(), UnitKind::Sniper.stats(), UnitKind::MercRanger.stats());
    assert_eq!(world.stock.get(Resource::Wood), back(r.cost_wood) + back(s.cost_wood) + back(m.cost_wood));
    assert_eq!(world.stock.get(Resource::Iron), back(r.cost_iron) + back(s.cost_iron) + back(m.cost_iron));
    assert!(m.cost_wood % 2 == 1, "the test wants an odd price to round");
    assert!(world.gold <= gold, "gold came back");
}

/// What the map's landscape does (`data::Modifiers`): from the setup into the world and through the map file; power
/// rounded like TAB's mods; the zombies' hearing, sight and speed.
#[test]
fn landscape_modifiers_change_the_round() {
    let cold = data::Modifiers { building_energy: 130, ..data::Modifiers::default() };
    assert_eq!([1, 3, 4, 5, 10].map(|n| cold.energy(n)), [1, 4, 5, 7, 13]);

    let mut setup = MapSetup { generator: Generator::Blank, ..MapSetup::default() };
    setup.common.size = 128.0;
    setup.modifiers = data::Modifiers { zombie_hearing: 50, zombie_sight: 110, zombie_speed: 80, unit_speed: 80, ..data::Modifiers::default() };
    let mut world = World::generate(4, &setup);
    assert_eq!(world.modifiers, setup.modifiers);
    let walker = Breed::Walker.stats();
    assert_eq!(world.zombie_sight(Breed::Walker), walker.sight * 1.1);
    assert_eq!(world.hear_factor(Breed::Walker), sound::hear_factor(walker.awareness * 0.5));
    let p = world.base + vec2(20.0, 0.0);
    world.spawn_zombie_as(p, Breed::Walker, true);
    let speed = world.zombies.last().unwrap().speed();
    assert!(speed >= walker.speed * 0.8 * 0.8 - 1e-4 && speed <= walker.speed * 1.2 * 0.8 + 1e-4, "{speed}");

    let file = crate::mapfile::MapFile::from_world(&world, "x");
    assert_eq!(file.to_world().unwrap().modifiers, setup.modifiers, "lost in the map file");
    let plain = crate::mapfile::MapFile::from_world(&World::new(4), "x");
    assert_eq!(plain.modifiers, data::Modifiers::default());
}

/// TAB's generator puts its ornaments down (`ornaments.rs`): a treasure ruin under every pile of loot it makes, a
/// medium power line tower next to every energy supply and a burning truck next to every food supply, and its high
/// power line towers in the middle of a 5 × 5 forest.
#[test]
fn tab_maps_have_ruins_round_their_loot_and_towers_in_the_forest() {
    let mut difficulty = data::get().difficulty("").clone();
    difficulty.roaming_zombies = 0.0;
    let map = data::get().maps.maps.iter().find(|m| m.setup.generator == Generator::Tab && m.setup.look == "VO").expect("a TAB map");
    let world = World::generate_with(42, &map.setup, &difficulty);
    let ruins: Vec<(i32, i32)> = world.ornaments.iter().filter(|o| o.kind == OrnamentKind::Ruin).map(|o| o.tile).collect();
    assert!(!ruins.is_empty(), "no ruins");
    // Is the tile on or right next to an ornament of these kinds?
    let next_to = |kinds: &[OrnamentKind], (x, y): (i32, i32)| {
        world.ornaments.iter().filter(|o| kinds.contains(&o.kind)).any(|o| {
            let (w, h) = o.kind.footprint();
            (o.tile.0 - 1..=o.tile.0 + w).contains(&x) && (o.tile.1 - 1..=o.tile.1 + h).contains(&y)
        })
    };
    for loot in &world.loot {
        let (x, y) = (loot.pos.x as i32, loot.pos.y as i32);
        // The generator's loot lies in ruins, the supplies by their tower or truck; loot dropped by houses of doom
        // elsewhere – there are none at tick 0.
        match loot.kind {
            LootKind::Energy => assert!(next_to(&[OrnamentKind::PowerTowerMedium, OrnamentKind::PowerTowerMediumFlip], (x, y)), "energy at {x}, {y}"),
            LootKind::Food => assert!(next_to(&[OrnamentKind::Truck], (x, y)), "food at {x}, {y}"),
            _ => {
                let inside = ruins.iter().any(|&(rx, ry)| x >= rx && y >= ry && x < rx + 3 && y < ry + 3);
                assert!(inside, "loot at {x}, {y} outside every ruin {ruins:?}");
            }
        }
    }
    assert!(world.loot.iter().any(|l| l.kind == LootKind::Energy), "no energy supply");
    assert!(world.loot.iter().any(|l| l.kind == LootKind::Food), "no food supply");
    for tower in world.ornaments.iter().filter(|o| matches!(o.kind, OrnamentKind::PowerTower | OrnamentKind::PowerTowerFlip)) {
        let (x, y) = tower.tile;
        assert!((-2..=2).all(|d| world.map.terrain(x + d, y) == Terrain::Forest && world.map.terrain(x, y + d) == Terrain::Forest));
    }
}

/// A spot on cleared grass `dx`, `dy` tiles from the base, `r` tiles round it cleared – for the tests of TAB's late
/// buildings and units below.
fn cleared_spot(world: &mut World, dx: i32, dy: i32, r: i32) -> (i32, i32) {
    let t = (world.base.x as i32 + dx, world.base.y as i32 + dy);
    clear_ground(world, t, r);
    t
}

/// Zombies that stand where they are put – nothing but the attack under test moves them.
fn frozen_zombies(world: &mut World, spots: &[Vec2], breed: Breed) {
    for &p in spots {
        world.add_zombie(p, breed, true);
    }
    for zombie in &mut world.zombies {
        zombie.speed = 0.0;
    }
}

/// The shocking tower (TAB's): once charged it hits every zombie within its range at once – not the one further out.
#[test]
fn shocking_tower_hits_every_zombie_in_range() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let t = cleared_spot(&mut world, 10, 10, 9);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::ShockingTower, tile: t, turned: false, editor: true }]);
    let tower = world.buildings.last().unwrap().center();
    let spots = [tower + vec2(2.0, 0.0), tower + vec2(0.0, 3.0), tower + vec2(-3.0, -2.0), tower + vec2(8.0, 0.0)];
    frozen_zombies(&mut world, &spots, Breed::Fat);
    let hp = world.zombies[0].hp;
    world.step(&[]);
    let weapon = BuildingKind::ShockingTower.stats().weapon.as_ref().unwrap();
    let hit = hp - after_armor(weapon.damage, Breed::Fat.stats().armor);
    let after: Vec<u32> = world.zombies.iter().map(|z| z.hp).collect();
    assert_eq!(after, vec![hit, hit, hit, hp], "every one in range at once, not the far one");
}

/// TAB's mine: a zombie steps on it, it goes off – everyone round it is hurt, the one further away isn't – and it's
/// gone.
#[test]
fn a_mine_goes_off_under_a_zombie() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let t = cleared_spot(&mut world, 10, 10, 6);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::MineTrap, tile: t, turned: false, editor: true }]);
    assert_eq!(world.buildings.last().unwrap().kind, BuildingKind::MineTrap);
    assert!(world.buildings.last().unwrap().zombies_ignore(), "zombies would attack the mine");
    let on = vec2(t.0 as f32 + 0.5, t.1 as f32 + 0.5);
    frozen_zombies(&mut world, &[on, on + vec2(1.2, 0.0), on + vec2(4.0, 0.0)], Breed::Walker);
    let hp = world.zombies[2].hp;
    for _ in 0..data::get().buildings.trap_every + 2 {
        world.step(&[]);
    }
    assert!(!world.buildings.iter().any(|b| b.kind == BuildingKind::MineTrap), "the mine is still there");
    assert_eq!(world.zombies.len(), 1, "the two near it should be dead");
    assert_eq!(world.zombies[0].hp, hp, "the far one was hurt");
}

/// TAB's mine delay (`DelayActivation`): set to 2, a mine waits 2 × 0.5 s after the zombie steps on it – still there
/// a little before, gone after.
#[test]
fn a_mine_waits_its_delay() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let t = cleared_spot(&mut world, 10, 10, 6);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::MineTrap, tile: t, turned: false, editor: true }]);
    let id = world.buildings.last().unwrap().id;
    world.apply_orders(&[Order::SetMineDelay { buildings: vec![id], delay: 2 }]);
    assert_eq!(world.buildings.last().unwrap().delay, 2);
    let on = vec2(t.0 as f32 + 0.5, t.1 as f32 + 0.5);
    frozen_zombies(&mut world, &[on], Breed::Walker);
    let wait = 2 * data::get().buildings.mine_trap.mine.as_ref().unwrap().delay_step;
    let trap_every = data::get().buildings.trap_every;
    let mine_there = |world: &World| world.buildings.iter().any(|b| b.kind == BuildingKind::MineTrap);
    for _ in 0..trap_every + wait - 2 {
        world.step(&[]);
    }
    assert!(mine_there(&world), "went off before its delay");
    for _ in 0..trap_every + 4 {
        world.step(&[]);
    }
    assert!(!mine_there(&world), "didn't go off after its delay");
}

/// Lucifer: its flame hits every zombie in its cone and sets some alight – and burns your own unit standing in it.
#[test]
fn lucifer_burns_zombies_and_friends_in_its_cone() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let t = cleared_spot(&mut world, 10, 10, 8);
    let at = vec2(t.0 as f32 + 0.5, t.1 as f32 + 0.5);
    world.spawn_unit(UnitKind::Lucifer, at);
    let lucifer = world.units.last().unwrap().id;
    bait(&mut world, UnitKind::Soldier, at + vec2(1.0, 1.0));
    let soldier_hp = world.units.last().unwrap().hp;
    let (front, beside) = ([at + vec2(2.0, 2.0), at + vec2(1.7, 2.3)], at + vec2(-2.0, 2.5));
    frozen_zombies(&mut world, &[front[0], front[1], beside], Breed::Fat);
    let hp = world.zombies[0].hp;
    let mut burned = false;
    for _ in 0..(4.0 * TICKS_PER_SECOND) as u32 {
        world.step(&[]);
        burned |= world.zombies.iter().any(|z| z.burning > 0);
    }
    assert!(world.find_unit(lucifer).is_some());
    // They stand still, but the list is sorted now and then – found by where they stand.
    let hp_at = |p: Vec2| world.zombies.iter().find(|z| z.pos == p).map_or(0, |z| z.hp);
    assert!(hp_at(front[0]) < hp && hp_at(front[1]) < hp, "the two in front aren't burnt");
    assert_eq!(hp_at(beside), hp, "the one beside it was hit");
    assert!(burned, "nobody caught fire");
    let soldier = world.units.iter().find(|u| u.kind == UnitKind::Soldier).map_or(0, |u| u.hp);
    assert!(soldier < soldier_hp, "the soldier in the flame wasn't burnt");
}

/// Thanatos: its missile blows up a crowd far off, but it doesn't fire at one closer than `min_range` – there it hits
/// with its launcher.
#[test]
fn thanatos_blows_up_crowds_and_hits_close_up() {
    let (mut world, _) = quiet_world();
    world.units.clear();
    let t = cleared_spot(&mut world, 10, 10, 10);
    let at = vec2(t.0 as f32 + 0.5, t.1 as f32 + 0.5);
    world.spawn_unit(UnitKind::Thanatos, at);
    let far = at + vec2(5.0, 5.0);
    frozen_zombies(&mut world, &[far, far + vec2(0.6, 0.0), far + vec2(0.0, 0.7)], Breed::Fat);
    let hp = world.zombies[0].hp;
    for _ in 0..(8.0 * TICKS_PER_SECOND) as u32 {
        world.step(&[]);
    }
    assert!(world.zombies.iter().all(|z| z.hp < hp), "the missile didn't hit all three");

    // Close up: 2 tiles – too close for the missile, too far for the blow: nothing. 1 tile: the blow.
    world.zombies.clear();
    frozen_zombies(&mut world, &[at + vec2(1.4, 1.4)], Breed::Fat);
    let i = world.units.iter().position(|u| u.kind == UnitKind::Thanatos).unwrap();
    assert!(!world.units[i].reaches(world.zombies[0].pos), "2 tiles away is out of both");
    world.zombies[0].pos = at + vec2(0.7, 0.7);
    let hp = world.zombies[0].hp;
    for _ in 0..(4.0 * TICKS_PER_SECOND) as u32 {
        world.step(&[]);
    }
    assert!(world.zombies.first().is_none_or(|z| z.hp < hp), "no blow close up");
}

/// The new economy: the power plant burns wood like TAB's (none, no power), the oil platform only on oil and pumps it, the
/// warehouse makes room, the Transmutator turns goods into oil.
#[test]
fn power_plant_oil_warehouse_transmutator() {
    let (mut world, _) = quiet_world();
    world.extra_power = 0;
    let base = world.base;
    let t = cleared_spot(&mut world, 4, -10, 8);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::PowerPlant, tile: t, turned: false, editor: true }]);
    let plant = world.buildings.len() - 1;
    assert_eq!(world.buildings[plant].kind, BuildingKind::PowerPlant);
    world.stock.set(Resource::Wood, 0);
    world.economy_payout();
    let without: u32 = world.power().made.iter().sum();
    world.stock.set(Resource::Wood, 100);
    world.economy_payout();
    let with: u32 = world.power().made.iter().sum();
    assert_eq!(with - without, BuildingKind::PowerPlant.stats().energy, "wood makes the power");
    assert_eq!(world.stock.get(Resource::Wood), 100 - BuildingKind::PowerPlant.stats().burns_wood);

    // The oil platform: not beside the oil, right on it.
    let o = cleared_spot(&mut world, -12, 4, 4);
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        world.map.set_deposit(o.0 + dx, o.1 + dy, Some(Deposit { ore: Ore::Oil, amount: 1000 }));
    }
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::Foundry, tile: (t.0 - 6, t.1), turned: false, editor: true }]);
    assert!(!world.can_place(BuildingKind::OilPlatform, (o.0 + 1, o.1), false), "half beside the oil");
    assert!(world.can_place(BuildingKind::OilPlatform, o, false), "not on the oil node");
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::OilPlatform, tile: o, turned: false, editor: true }]);
    world.extra_power = 100_000;
    world.stock.set(Resource::Oil, 0);
    world.economy_payout();
    assert!(world.stock.get(Resource::Oil) > 0, "no oil pumped");

    // The warehouse: 50 more room for the goods, not for food.
    let (goods, food) = (world.storage(Resource::Stone), world.storage(Resource::Food));
    let w = cleared_spot(&mut world, -12, -12, 4);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::Warehouse, tile: w, turned: false, editor: true }]);
    assert_eq!(world.storage(Resource::Stone), goods + 50);
    assert_eq!(world.storage(Resource::Food), food);

    // The Transmutator: wood, stone and iron into oil – one only.
    let m = (base.x as i32 + 12, base.y as i32 + 6);
    clear_ground(&mut world, (m.0 + 2, m.1 + 2), 5);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::Transmutator, tile: m, turned: false, editor: true }]);
    assert_eq!(world.buildings.last().unwrap().kind, BuildingKind::Transmutator, "the Transmutator didn't fit");
    assert!(!world.can_place(BuildingKind::Transmutator, (m.0, m.1 + 8), false), "a second wonder");
    for r in [Resource::Wood, Resource::Stone, Resource::Iron] {
        world.stock.set(r, 100);
    }
    world.stock.set(Resource::Oil, 0);
    world.economy_payout();
    // 20 for the Transmutator – and the power plant from above burns its wood too.
    let burned = BuildingKind::PowerPlant.stats().burns_wood;
    assert_eq!(world.stock.get(Resource::Wood), 80 - burned, "it took no wood");
    assert!(world.stock.get(Resource::Oil) >= 40, "it made no oil");
}

/// The Academy: while it works, every unit is a veteran.
#[test]
fn the_academy_makes_veterans() {
    let (mut world, _) = quiet_world();
    assert!(world.units.iter().all(|u| !u.veteran));
    let t = cleared_spot(&mut world, 10, -10, 5);
    world.apply_orders(&[Order::PlaceBuilding { kind: BuildingKind::Academy, tile: t, turned: false, editor: true }]);
    world.step(&[]);
    assert!(world.units.iter().all(|u| u.veteran), "not every unit a veteran");
}

/// The editor's copy and paste (`clipboard.rs`): a tent with its tag, hit points and infection, a veteran with its tag,
/// a sleeping zombie, a house of doom and a spawner come again 12 tiles further right with everything they had – a
/// second command center doesn't. A cut takes the originals off the map.
#[test]
fn a_copy_keeps_everything_and_a_cut_takes_it_away() {
    let (mut world, _) = quiet_world();
    let (bx, by) = (world.base.x as i32, world.base.y as i32);
    clear_ground(&mut world, (bx + 16, by + 12), 14);
    let tile = (bx + 8, by + 8);
    world.apply_orders(&[
        Order::PlaceBuilding { kind: BuildingKind::Tent, tile, turned: false, editor: true },
        Order::SpawnUnit { kind: UnitKind::Soldier, point: vec2(tile.0 as f32 + 3.5, tile.1 as f32 + 0.5) },
        Order::PaintZombies { tiles: vec![(tile.0 + 5, tile.1)], per_tile: vec![(Breed::Runner, 1)], wander: true },
        Order::PlaceDoom { tile: (tile.0, tile.1 + 4), size: (2, 2) },
    ]);
    let (tent, unit) = (world.buildings.last().unwrap().id, world.units.last().unwrap().id);
    let tag = Tag { name_id: "home".into(), name: "Old home".into(), minimap: true, ..Tag::default() };
    world.apply_orders(&[
        Order::SetTag { thing: ThingRef::Building(tent), tag: tag.clone() },
        Order::SetHealth { thing: ThingRef::Building(tent), percent: 40 },
        Order::SetMark { thing: ThingRef::Building(tent), on: true },
        Order::SetTag { thing: ThingRef::Unit(unit), tag: tag.clone() },
        Order::SetMark { thing: ThingRef::Unit(unit), on: true },
    ]);
    let cc = world.buildings.iter().find(|b| b.kind == BuildingKind::CommandCenter).unwrap().id;
    let sleeper = world.sleep.iter().find(|s| s.pos.x.floor() as i32 == tile.0 + 5).expect("no zombie painted").pos;
    let house = world.doom.last().unwrap().id;
    let spawner = world.spawners[0].id;
    let pieces = vec![Piece::Building(tent), Piece::Building(cc), Piece::Unit(unit), Piece::Sleeper(sleeper), Piece::Doom(house), Piece::Spawner(spawner)];
    let clip = world.copy(&pieces);
    assert_eq!(clip.describe(), "2 buildings, 1 unit, 1 zombie, 1 house of doom, 1 spawner");
    let (spawners, inside) = (world.spawners.len(), world.doom.last().unwrap().inside);
    world.apply_orders(&[Order::Paste { clip: Box::new(clip.clone()), to: (clip.middle.0 + 12, clip.middle.1) }]);

    let ccs = world.buildings.iter().filter(|b| b.kind == BuildingKind::CommandCenter).count();
    assert_eq!(ccs, 1, "a second command center was pasted");
    let copy = world.buildings.iter().find(|b| b.tile == (tile.0 + 12, tile.1)).expect("the tent wasn't pasted");
    assert_eq!((copy.kind, copy.hp, copy.infected, copy.tag.as_deref()), (BuildingKind::Tent, BuildingKind::Tent.stats().hp * 40 / 100, true, Some(&tag)));
    let soldier = world.units.iter().find(|u| u.pos == vec2(tile.0 as f32 + 15.5, tile.1 as f32 + 0.5)).expect("the unit wasn't pasted");
    assert!(soldier.veteran && soldier.tag.as_deref() == Some(&tag) && soldier.id != unit);
    assert!(world.sleep.iter().any(|s| s.pos == sleeper + vec2(12.0, 0.0) && s.breed == Breed::Runner), "the zombie wasn't pasted");
    let doom = world.doom.iter().find(|d| d.tile == (tile.0 + 12, tile.1 + 4)).expect("the house wasn't pasted");
    assert_eq!((doom.size, doom.inside), ((2, 2), inside));
    assert_eq!(world.spawners.len(), spawners + 1);
    let (first, pasted) = (&world.spawners[0], world.spawners.last().unwrap());
    assert!(pasted.id != first.id && (pasted.name.as_str(), pasted.size, pasted.axes, pasted.mode) == (first.name.as_str(), first.size, first.axes, first.mode));

    let cut: Vec<Piece> = pieces.into_iter().filter(|&p| p != Piece::Building(cc)).collect();
    world.apply_orders(&[Order::RemovePieces { pieces: cut }]);
    assert!(world.find_building(tent).is_none() && world.find_unit(unit).is_none() && world.find_doom(house).is_none() && world.find_spawner(spawner).is_none());
    assert!(world.sleep.iter().all(|s| s.pos != sleeper), "the zombie is still there");
    assert!(!world.map.is_built(tile.0, tile.1 + 4), "the house's tiles are still taken");
}

/// A straight line of track below the command center, west to east across the map with a buffer stop at each end
/// (`rails.rs`) – both outside the world border – and its row.
fn railway(world: &mut World) -> i32 {
    let n = world.map.size as i32;
    railway_between(world, 4, n - 5)
}

/// `railway` from buffer stop `from` to buffer stop `to` (columns).
fn railway_between(world: &mut World, from: i32, to: i32) -> i32 {
    let y = world.base.y as i32 + 9;
    world.ornaments.push(Ornament { kind: OrnamentKind::Rail(rails::RailShape::EndW), tile: (from, y) });
    for x in from + 1..to {
        world.ornaments.push(Ornament { kind: OrnamentKind::Rail(rails::RailShape::X), tile: (x, y) });
    }
    world.ornaments.push(Ornament { kind: OrnamentKind::Rail(rails::RailShape::EndE), tile: (to, y) });
    y
}

/// TAB's train (`trains.rs`): sent by a script, it comes in from the end of the line nearer the edge of the map,
/// stops below the command center, waits, runs the arrival's script there (and says so), then goes on to the end of
/// the line – outside the world border, so there it's gone.
#[test]
fn a_train_stops_at_the_command_center_and_leaves_the_map() {
    let (mut world, _) = quiet_world();
    world.events.clear();
    let y = railway(&mut world);
    assert!(!world.map.in_play(4, y) && !world.map.in_play(world.map.size as i32 - 5, y));
    let arrived = vec![var_step("arrived", true, 1)];
    world.run(&script(vec![Action::Train { stop: true, reverse: false, on_arrival: arrived }]));
    assert_eq!(world.trains.len(), 1);
    let start = world.trains[0].line.point_at(0.0);
    let mut stood = None;
    let mut last = None;
    for _ in 0..3000 {
        world.step(&[]);
        let Some(train) = world.trains.first() else {
            break;
        };
        if world.variable("arrived") == 1 && stood.is_none() {
            stood = Some((world.tick, train.line.point_at(train.offset)));
        }
        last = Some((world.tick, train.line.point_at(train.offset), train.line.point_at(train.line.length())));
    }
    let (when, at) = stood.expect("it never stopped at the command center");
    assert!((at.x - world.base.x).abs() < 1.0, "it stopped at {at}, the base is at {}", world.base);
    assert!(world.trains.is_empty(), "it's still there");
    // Gone with the locomotive's front at the buffer stop: its middle half its length before the end.
    let (gone, middle, end) = last.unwrap();
    assert!(end.distance(start) > 50.0);
    let front = middle.distance(end);
    assert!((front - CarKind::Locomotive.length() / 2.0).abs() < 0.5, "{front}");
    assert!(gone > when + data::get().trains.stop as u64, "it didn't wait at the stop");
    assert_eq!(world.variable("arrived"), 1, "the arrival ran more than once");
}

/// A line that ends on the playing field: there the train stays, like TAB's, its front at the buffer stop.
#[test]
fn a_train_stays_at_the_end_of_a_line_on_the_map() {
    let (mut world, _) = quiet_world();
    let x = world.base.x as i32;
    let y = railway_between(&mut world, x - 30, x + 30);
    assert!(world.map.in_play(x - 30, y) && world.map.in_play(x + 30, y));
    world.run(&script(vec![Action::Train { stop: false, reverse: false, on_arrival: Vec::new() }]));
    for _ in 0..3000 {
        world.step(&[]);
        if !world.trains[0].running {
            break;
        }
    }
    let train = &world.trains[0];
    assert!(!train.running && train.speed == 0.0);
    let front = train.line.point_at(train.offset).distance(train.line.point_at(train.line.length()));
    assert!((front - CarKind::Locomotive.length() / 2.0).abs() < 0.2, "{front}");
}

/// Sent again while the first is still on the line, a second train comes – the first runs on.
#[test]
fn a_second_train_doesnt_take_the_first_ones_place() {
    let (mut world, _) = quiet_world();
    railway(&mut world);
    world.run(&script(vec![Action::Train { stop: true, reverse: false, on_arrival: Vec::new() }]));
    for _ in 0..60 {
        world.step(&[]);
    }
    let first = world.trains[0].offset;
    world.run(&script(vec![Action::Train { stop: true, reverse: false, on_arrival: Vec::new() }]));
    assert_eq!(world.trains.len(), 2);
    assert_eq!(world.trains[0].offset, first, "the first one was sent back");
    assert!(world.trains[1].offset < first);
}

/// Running, a train runs over whoever is in its way; standing (at its stop), it pushes them out to its side.
#[test]
fn a_train_runs_over_who_is_in_its_way() {
    let (mut world, _) = quiet_world();
    world.events.clear();
    let y = railway(&mut world);
    let arrived = vec![var_step("arrived", true, 1)];
    world.run(&script(vec![Action::Train { stop: true, reverse: false, on_arrival: arrived }]));
    // Until it runs at full speed.
    while world.trains[0].speed < data::get().trains.max_speed {
        world.step(&[]);
    }
    let train = &world.trains[0];
    let ahead = train.line.point_at(train.offset + 6.0);
    world.spawn_zombie_as(ahead, Breed::Walker, true);
    let id = world.zombies.last().unwrap().id;
    for _ in 0..20 {
        world.step(&[]);
    }
    assert!(world.find_zombie(id).is_none(), "the zombie on the track is still there");
    // At the command center it stands: a unit put inside it is pushed out to the side, off the track's row – unhurt.
    while world.variable("arrived") == 0 {
        world.step(&[]);
    }
    let train = &world.trains[0];
    let middle = train.line.point_at(train.offset);
    world.spawn_unit(UnitKind::Soldier, middle + vec2(0.0, 0.1));
    world.step(&[]);
    let unit = world.units.last().unwrap();
    assert!(unit.hp == unit.max_hp(), "a standing train hurt it");
    assert!((unit.pos.y - (y as f32 + 0.5)).abs() >= 1.0, "it's still inside the train at {}", unit.pos);
}

/// Like TAB: nothing is built on the railway or right next to it – only a gate across it, its middle on the track.
#[test]
fn nothing_is_built_on_or_next_to_the_railway() {
    let (mut world, _) = quiet_world();
    let y = railway(&mut world);
    let x = world.base.x as i32 + 4;
    assert!(!world.can_place(BuildingKind::Wall, (x, y), false), "on the track");
    assert!(!world.can_place(BuildingKind::Wall, (x, y + 1), false), "right next to it");
    assert!(world.can_place(BuildingKind::Wall, (x, y + 2), false), "two tiles away");
    // A gate turned across the track (1 × 3): its middle tile on it.
    assert!(world.can_place(BuildingKind::Gate, (x, y - 1), true), "a gate across the track");
    assert!(!world.can_place(BuildingKind::Gate, (x, y), true), "a gate with its end on the track");
}

