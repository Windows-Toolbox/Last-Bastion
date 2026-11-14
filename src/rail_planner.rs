//! The editor's rail planner, like Factorio's: click an open end of the track (`rails::open_ends`) – or empty ground
//! for a new line – and arrows show every way the track can go on from there (straight, the two curves). The one the
//! mouse points to stands there as a ghost (TAB's picture, see-through); a click builds it, holding the button and
//! dragging builds piece after piece towards the mouse. Right click stops there.
//!
//! And what a train sent now would do (`World::train_plan`): where it comes in and where it stops for the command
//! center – drawn on the map while the railway or the scripts are being edited.
//!
//! Only the editor's look and its orders – the pieces go onto the map as `Order::PlaceOrnament` like any other.

use macroquad::prelude::*;

use crate::data;
use crate::draw::{draw_diamond, draw_ornament_ghost};
use crate::iso::Camera;
use crate::map::Region;
use crate::sim::{extensions, joined, line_start, open_ends, rail_pieces, CarAt, Connector, Extension, Order, Ornament, OrnamentKind, Piece, RailPiece, Side, Turn, World};
use crate::sprites::Sprites;
use crate::ui::tile_of;

/// A click this close to an open end (tiles) builds on from it – further away it starts a new line.
const PICK_END: f32 = 1.5;
/// Dragged: the next straight piece once the mouse is this far ahead of the end (tiles) – a curve once it's this far
/// ahead and this far to the side. A curve only while the mouse is at least `CURVE_AHEAD` ahead: behind the end nothing
/// is built (else a drag past it went round in circles).
const STRAIGHT_STEP: f32 = 1.2;
const CURVE_STEP: f32 = 2.5;
const CURVE_AHEAD: f32 = 1.0;
/// At most this many pieces in one frame – a fast drag catches up over a few.
const MOST_PER_FRAME: usize = 8;
/// Straight on while the mouse is within this angle of the way ahead (tan 30°) – plus this much to the side (tiles).
const STRAIGHT_CONE: f32 = 0.58;
const STRAIGHT_SIDE: f32 = 0.75;

const ARROW: Color = Color::new(1.0, 1.0, 1.0, 0.45);
const CHOSEN: Color = Color::new(1.0, 0.85, 0.2, 1.0);
const GHOST: Color = Color::new(1.0, 1.0, 1.0, 0.75);

/// Where the planner builds on from.
#[derive(Clone, Copy, PartialEq, Debug)]
enum From {
    /// A connector of the track – an open end, or one on the track the mouse points away from.
    End(Connector),
    /// Empty ground: the next piece is a buffer stop there, its track leaving towards the mouse.
    Start((i32, i32)),
    /// A tile with track on it, clicked away from any open end: on from whichever of its pieces' connectors the mouse
    /// is beyond – a branch (two pieces at one connector are a switch, `docs\trains.md` step 2), or past a buffer stop,
    /// which then goes.
    On((i32, i32)),
}

/// The piece the mouse points to: the piece, where it's built from (`From::End` or `From::Start`), and the buffer stop
/// it replaces.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Choice {
    piece: Extension,
    from: From,
    replaces: Option<RailPiece>,
}

/// The track as the planner sees it: the pieces on the map and where the map is (`Map::full_area` – the playing field
/// and the ring round it; the corners of the grid past it are off the map).
struct Track {
    pieces: Vec<RailPiece>,
    map: Region,
}

impl Track {
    fn of(world: &World) -> Track {
        Track { pieces: rail_pieces(&world.ornaments), map: world.map.full_area() }
    }

    /// May `piece` go onto the map (`replaces` gone)? Every tile of it on the map – else the order is refused, and the
    /// planner built on from a piece that isn't there –, not a piece lying there already, and a buffer stop not on
    /// other track.
    fn fits(&self, piece: Extension, replaces: Option<RailPiece>) -> bool {
        let rail = RailPiece { shape: piece.shape, tile: piece.tile };
        let (x, y, s) = (piece.tile.0, piece.tile.1, piece.shape.size());
        let inside = (y..y + s).all(|ty| (x..x + s).all(|tx| !rail.covers((tx, ty)) || self.map.contains_tile(tx, ty)));
        let mut others = self.pieces.iter().filter(|&&p| Some(p) != replaces);
        inside && !others.clone().any(|&p| p == rail) && !(piece.shape.is_end() && others.any(|p| p.covers(piece.tile)))
    }

    /// Is connector `c` still a connector of a piece on the map – or of `pending`, built but not on the map yet?
    fn has(&self, c: Connector, pending: Option<RailPiece>) -> bool {
        self.pieces.iter().chain(pending.iter()).any(|p| {
            let (a, b) = p.connectors();
            a == c || b == Some(c)
        })
    }

    /// The connectors to build on from on `tile`, of every piece lying there: both ends of a piece of track – and for
    /// a buffer stop the end of the piece it closes, facing it: the next piece takes the buffer stop's tile.
    fn starts_on(&self, tile: (i32, i32)) -> Vec<(Connector, Option<RailPiece>)> {
        let mut out = Vec::new();
        for &piece in self.pieces.iter().filter(|p| p.covers(tile)) {
            match piece.connectors() {
                (a, Some(b)) => out.extend([(a, None), (b, None)]),
                (a, None) => {
                    let (dx, dy) = (a.side.dir().x as i32, a.side.dir().y as i32);
                    let facing = Connector { cell: (a.cell.0 + dx, a.cell.1 + dy), side: opposite(a.side) };
                    out.push((facing, Some(piece)));
                }
            }
        }
        out
    }
}

fn opposite(side: Side) -> Side {
    match side {
        Side::N => Side::S,
        Side::S => Side::N,
        Side::E => Side::W,
        Side::W => Side::E,
    }
}

/// The planner while it's in hand (`Tool::RailPlanner`).
#[derive(Default)]
pub struct RailPlanner {
    from: Option<From>,
    /// The piece built last – its order goes onto the map after this frame, the planner already builds on from it.
    pending: Option<RailPiece>,
}

impl RailPlanner {
    /// Is it building on from somewhere – then right click and Esc stop that first.
    pub fn is_building(&self) -> bool {
        self.from.is_some()
    }

    pub fn stop(&mut self) {
        self.from = None;
        self.pending = None;
    }

    /// Left click on the map: pick where to build from – or, already building, build the piece the mouse points to.
    pub fn press(&mut self, world: &World, point: Vec2, orders: &mut Vec<Order>) {
        self.press_on(&Track::of(world), point, orders);
    }

    fn press_on(&mut self, track: &Track, point: Vec2, orders: &mut Vec<Order>) {
        self.check(track);
        match self.from {
            None => {
                let ends = open_ends(&track.pieces);
                let near = ends.into_iter().map(|c| (c, c.point().distance(point))).filter(|(_, d)| *d < PICK_END).min_by(|a, b| a.1.total_cmp(&b.1));
                let tile = tile_of(point);
                self.from = Some(match near {
                    Some((end, _)) => From::End(end),
                    // Track there: on from it – never a new line on top of it.
                    None if track.pieces.iter().any(|p| p.covers(tile)) => From::On(tile),
                    None => From::Start(tile),
                });
            }
            Some(from) => {
                if let Some(choice) = choice(from, point, track) {
                    self.build(choice, track, orders);
                }
            }
        }
    }

    /// The button held: piece after piece towards the mouse, as long as it's far enough ahead.
    pub fn drag(&mut self, world: &World, point: Vec2, orders: &mut Vec<Order>) {
        self.drag_on(&Track::of(world), point, orders);
    }

    fn drag_on(&mut self, track: &Track, point: Vec2, orders: &mut Vec<Order>) {
        self.check(track);
        for _ in 0..MOST_PER_FRAME {
            let Some(from) = self.from else {
                return;
            };
            match choice(from, point, track) {
                Some(c) if reached(c, point) => self.build(c, track, orders),
                _ => return,
            }
        }
    }

    /// Is where it builds from still there? Undone or erased: it stops.
    fn check(&mut self, track: &Track) {
        if self.pending.is_some_and(|p| track.pieces.contains(&p)) {
            self.pending = None;
        }
        let there = match self.from {
            Some(From::End(c)) => track.has(c, self.pending),
            Some(From::On(tile)) => track.pieces.iter().any(|p| p.covers(tile)),
            _ => true,
        };
        if !there {
            self.stop();
        }
    }

    /// Onto the map (a buffer stop it replaces off it) – and on from its far end. Not on: a buffer stop ends the line,
    /// and track that joins what's there (a gap closed, another line reached) is done.
    fn build(&mut self, choice: Choice, track: &Track, orders: &mut Vec<Order>) {
        let piece = choice.piece;
        if let Some(stop) = choice.replaces {
            orders.push(Order::RemovePieces { pieces: vec![Piece::Ornament(Ornament { kind: OrnamentKind::Rail(stop.shape), tile: stop.tile })] });
        }
        orders.push(Order::PlaceOrnament(Ornament { kind: OrnamentKind::Rail(piece.shape), tile: piece.tile }));
        self.pending = Some(RailPiece { shape: piece.shape, tile: piece.tile });
        self.from = piece.exit.filter(|&e| !joined(&track.pieces, e)).map(From::End);
    }

    /// Its look: the open ends to pick from – or the arrows from where it builds on, the ghost of the piece the mouse
    /// points to and its name.
    pub fn draw(&self, world: &World, camera: &Camera, sprites: &Sprites, mouse: Vec2) {
        let point = camera.screen_to_world(mouse);
        let track = Track::of(world);
        let Some(from) = self.from else {
            for end in open_ends(&track.pieces) {
                let at = camera.world_to_screen(end.point());
                draw_circle_lines(at.x, at.y, 7.0, 2.0, CHOSEN);
            }
            draw_text("Rail planner: click an open end, track to branch off - or empty ground", mouse.x + 14.0, mouse.y - 8.0, 20.0, CHOSEN);
            return;
        };
        let chosen = choice(from, point, &track);
        match from {
            From::Start(tile) => {
                let middle = vec2(tile.0 as f32, tile.1 as f32) + 0.5;
                for side in Side::ALL {
                    let color = if chosen.is_some_and(|c| c.piece.exit.is_some_and(|e| e.side == side)) { CHOSEN } else { ARROW };
                    arrow(camera, &[middle, middle + side.dir() * 1.5], color);
                }
            }
            From::End(end) => end_arrows(camera, end, None, &track, chosen),
            From::On(tile) => {
                for (end, replaces) in track.starts_on(tile) {
                    end_arrows(camera, end, replaces, &track, chosen);
                }
            }
        }
        match chosen {
            Some(c) => {
                let piece = c.piece;
                let rail = RailPiece { shape: piece.shape, tile: piece.tile };
                let size = piece.shape.size();
                for y in piece.tile.1..piece.tile.1 + size {
                    for x in (piece.tile.0..piece.tile.0 + size).filter(|&x| rail.covers((x, y))) {
                        draw_diamond(camera, vec2(x as f32, y as f32), Vec2::ONE, Color::new(1.0, 0.85, 0.2, 0.18));
                    }
                }
                draw_ornament_ghost(camera, sprites, Ornament { kind: OrnamentKind::Rail(piece.shape), tile: piece.tile }, GHOST);
                let name = if c.replaces.is_some() { format!("{} (instead of the buffer stop)", piece.shape.name()) } else { piece.shape.name().to_owned() };
                draw_text(&name, mouse.x + 14.0, mouse.y - 8.0, 20.0, CHOSEN);
            }
            // The mouse points where nothing may go: straight on along track that's there, off the map, …
            None => {
                draw_text("Nothing fits there", mouse.x + 14.0, mouse.y - 8.0, 20.0, ARROW);
            }
        }
    }
}

/// The arrows from connector `end`: every piece that may go on from it, the chosen one highlighted.
fn end_arrows(camera: &Camera, end: Connector, replaces: Option<RailPiece>, track: &Track, chosen: Option<Choice>) {
    for piece in extensions(end).into_iter().filter(|p| p.turn != Turn::End && track.fits(*p, replaces)) {
        let color = if chosen.is_some_and(|c| c.piece == piece) { CHOSEN } else { ARROW };
        arrow(camera, &way(end, piece), color);
    }
    let at = camera.world_to_screen(end.point());
    draw_circle(at.x, at.y, 4.0, CHOSEN);
}

/// The piece the mouse at `point` points to: from an end straight on while it's ahead (within 30°) or behind, the
/// curve to its side while it's ahead and further to the side, a buffer stop right on the end; on empty ground a buffer
/// stop with its track towards the mouse; on track from the connector the mouse is furthest beyond. `None`: what it
/// points to may not go there (`Track::fits`).
fn choice(from: From, point: Vec2, track: &Track) -> Option<Choice> {
    match from {
        From::Start(tile) => {
            let v = point - (vec2(tile.0 as f32, tile.1 as f32) + 0.5);
            let side = match (v.x.abs() >= v.y.abs(), v.x >= 0.0, v.y >= 0.0) {
                (true, true, _) => Side::E,
                (true, false, _) => Side::W,
                (false, _, true) => Side::S,
                (false, _, false) => Side::N,
            };
            let piece = line_start(tile, side);
            track.fits(piece, None).then_some(Choice { piece, from, replaces: None })
        }
        From::End(end) => end_choice(end, point, None, track),
        From::On(tile) => {
            let starts = track.starts_on(tile);
            let (end, replaces) = starts.into_iter().max_by(|a, b| local(a.0, point).0.total_cmp(&local(b.0, point).0))?;
            end_choice(end, point, replaces, track)
        }
    }
}

/// The piece from connector `end` the mouse points to (see `choice`).
fn end_choice(end: Connector, point: Vec2, replaces: Option<RailPiece>, track: &Track) -> Option<Choice> {
    let (ahead, aside) = local(end, point);
    let turn = if (point - end.point()).length() < 0.6 {
        Turn::End
    } else if ahead < CURVE_AHEAD || aside.abs() <= ahead * STRAIGHT_CONE + STRAIGHT_SIDE {
        Turn::Straight
    } else if aside > 0.0 {
        Turn::Plus
    } else {
        Turn::Minus
    };
    let piece = extensions(end).into_iter().find(|p| p.turn == turn).filter(|p| track.fits(*p, replaces))?;
    Some(Choice { piece, from: From::End(end), replaces })
}

/// Dragging: is the mouse far enough to build the chosen piece already?
fn reached(choice: Choice, point: Vec2) -> bool {
    match (choice.from, choice.piece.turn) {
        (_, Turn::End) | (From::On(_), _) => false,
        (From::Start(tile), _) => (point - (vec2(tile.0 as f32, tile.1 as f32) + 0.5)).length() >= 1.0,
        (From::End(end), Turn::Straight) => local(end, point).0 >= STRAIGHT_STEP,
        (From::End(end), _) => {
            let (ahead, aside) = local(end, point);
            ahead >= CURVE_STEP && aside.abs() >= CURVE_STEP
        }
    }
}

/// `point` seen from an open end: how far ahead along the track, how far to its side (+: `Turn::Plus`).
fn local(end: Connector, point: Vec2) -> (f32, f32) {
    let ahead = end.side.dir();
    let v = point - end.point();
    (v.dot(ahead), v.dot(vec2(-ahead.y, ahead.x)))
}

/// The way along `piece` from `end` – a straight piece a bit longer, so its arrow shows.
fn way(end: Connector, piece: Extension) -> Vec<Vec2> {
    if piece.turn == Turn::Straight {
        return vec![end.point(), end.point() + end.side.dir() * 1.5];
    }
    let mut points = piece.shape.points(piece.tile);
    if points.first().is_some_and(|p| p.distance(end.point()) > 0.1) {
        points.reverse();
    }
    points
}

/// A line along `points` (tiles) with a head at its end.
fn arrow(camera: &Camera, points: &[Vec2], color: Color) {
    let screen: Vec<Vec2> = points.iter().map(|&p| camera.world_to_screen(p)).collect();
    for pair in screen.windows(2) {
        draw_line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, 2.5, color);
    }
    if let [.., a, b] = screen[..] {
        let back = (a - b).normalize_or_zero() * 10.0;
        let side = vec2(-back.y, back.x) * 0.6;
        draw_line(b.x, b.y, b.x + back.x + side.x, b.y + back.y + side.y, 2.5, color);
        draw_line(b.x, b.y, b.x + back.x - side.x, b.y + back.y - side.y, 2.5, color);
    }
}

/// What a train sent now would do (`World::train_plan`): the whole train where it comes in (white) and where it stops
/// for the command center (yellow). No line long enough: why, at the first piece of track.
pub fn draw_train_plan(world: &World, camera: &Camera) {
    let pieces = rail_pieces(&world.ornaments);
    let Some(first) = pieces.first() else {
        return;
    };
    let Some(plan) = world.train_plan(false) else {
        let t = &data::get().trains;
        let cars = &t.cars;
        let length = cars.locomotive + cars.colonists + cars.armored + cars.goods + 3.0 * t.gap;
        let at = camera.world_to_screen(vec2(first.tile.0 as f32, first.tile.1 as f32) + 0.5);
        let longest = world.rail_lines().iter().map(|l| l.length()).fold(None, |m: Option<f32>, l| Some(m.map_or(l, |m| m.max(l))));
        let text = match longest {
            None => "No line for a train: a line starts at a buffer stop - the track has none".to_owned(),
            Some(l) => format!("No line fits a train: the longest from a buffer stop is {l:.0} tiles, a train needs {}", length.ceil()),
        };
        draw_text(&text, at.x + 10.0, at.y - 10.0, 20.0, Color::new(1.0, 0.45, 0.4, 1.0));
        return;
    };
    // The text above the locomotive (`up` lines).
    let label = |cars: &[CarAt], text: &str, up: f32, color: Color| {
        for car in cars {
            outline(camera, car, color);
        }
        if let Some(front) = cars.first() {
            let at = camera.world_to_screen(front.pos);
            draw_text(text, at.x + 12.0, at.y - 18.0 - 20.0 * up, 20.0, color);
        }
    };
    let seconds = data::get().trains.stop as f32 / crate::sim::TICKS_PER_SECOND;
    let stops = format!("Stops here for the command center ({seconds:.0} s)");
    if plan.stop - plan.start < 1.0 {
        label(&plan.cars_at(plan.stop), &format!("Train comes in here - {stops}"), 0.0, CHOSEN);
    } else {
        label(&plan.cars_at(plan.start), "Train comes in here", 0.0, Color::new(1.0, 1.0, 1.0, 0.7));
        label(&plan.cars_at(plan.stop), &stops, 1.0, CHOSEN);
    }
}

/// A car's outline on the ground.
fn outline(camera: &Camera, car: &CarAt, color: Color) {
    let half = car.dir * car.kind.length() / 2.0;
    let side = vec2(-car.dir.y, car.dir.x) * 0.6;
    let corners = [car.pos - half - side, car.pos + half - side, car.pos + half + side, car.pos - half + side].map(|p| camera.world_to_screen(p));
    for i in 0..4 {
        let (a, b) = (corners[i], corners[(i + 1) % 4]);
        draw_line(a.x, a.y, b.x, b.y, 2.0, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Map;
    use crate::sim::RailShape;

    fn piece(shape: RailShape, tile: (i32, i32)) -> RailPiece {
        RailPiece { shape, tile }
    }

    /// A line along x on row 32 of a 64 map: buffer stops at x = 10 and 16, straight track in between.
    fn line() -> Track {
        let mut pieces = vec![piece(RailShape::EndW, (10, 32)), piece(RailShape::EndE, (16, 32))];
        pieces.extend((11..16).map(|x| piece(RailShape::X, (x, 32))));
        Track { pieces, map: Map::full_area_of(64) }
    }

    /// The pieces the orders put onto the map.
    fn placed(orders: &[Order]) -> Vec<(RailShape, (i32, i32))> {
        orders
            .iter()
            .filter_map(|o| match o {
                Order::PlaceOrnament(Ornament { kind: OrnamentKind::Rail(shape), tile }) => Some((*shape, *tile)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_click_on_track_branches_off_it_instead_of_starting_a_line_on_top() {
        let track = line();
        let mut planner = RailPlanner::default();
        let mut orders = Vec::new();
        planner.press_on(&track, vec2(13.5, 32.5), &mut orders);
        assert_eq!(planner.from, Some(From::On((13, 32))));
        assert!(orders.is_empty(), "the first click only picks where to build from");
        // Straight on east is the track that's there – nothing to build.
        assert_eq!(choice(From::On((13, 32)), vec2(20.5, 32.5), &track), None);
        // Ahead and to the side: the curve branching off at the tile's east end.
        planner.press_on(&track, vec2(19.0, 38.0), &mut orders);
        assert_eq!(placed(&orders), vec![(RailShape::CurveSE, (14, 32))]);
        assert!(planner.from.is_some(), "it builds on from the curve's far end");
    }

    #[test]
    fn a_click_on_a_buffer_stop_builds_the_line_on_past_it() {
        let track = line();
        let mut planner = RailPlanner::default();
        let mut orders = Vec::new();
        planner.press_on(&track, vec2(10.5, 32.5), &mut orders);
        planner.press_on(&track, vec2(6.5, 32.5), &mut orders);
        // The buffer stop goes, a straight piece takes its tile.
        assert!(matches!(&orders[0], Order::RemovePieces { pieces } if pieces.len() == 1));
        assert_eq!(placed(&orders), vec![(RailShape::X, (10, 32))]);
        assert_eq!(planner.from, Some(From::End(Connector { cell: (10, 32), side: Side::W })));
    }

    #[test]
    fn track_that_reaches_other_track_stops_the_planner() {
        let track = Track { pieces: vec![piece(RailShape::X, (10, 32)), piece(RailShape::X, (12, 32))], map: Map::full_area_of(64) };
        let mut planner = RailPlanner { from: Some(From::End(Connector { cell: (10, 32), side: Side::E })), pending: None };
        let mut orders = Vec::new();
        planner.press_on(&track, vec2(12.5, 32.5), &mut orders);
        assert_eq!(placed(&orders), vec![(RailShape::X, (11, 32))]);
        assert_eq!(planner.from, None, "the gap is closed - nothing more to build");
    }

    #[test]
    fn nothing_off_the_map_or_a_buffer_stop_on_other_track() {
        // At the edge of the map (a 64 map: row 5 is on it from x = 27 to 36): no piece past it.
        let edge = Track { pieces: vec![piece(RailShape::X, (36, 5))], map: Map::full_area_of(64) };
        assert_eq!(choice(From::End(Connector { cell: (36, 5), side: Side::E }), vec2(39.0, 5.5), &edge), None);
        // Track along y crossing the next tile: a buffer stop can't go there, straight across (a crossing) can.
        let mut pieces = vec![piece(RailShape::X, (11, 32))];
        pieces.extend((30..35).map(|y| piece(RailShape::Y, (12, y))));
        let crossing = Track { pieces, map: Map::full_area_of(64) };
        let end = From::End(Connector { cell: (11, 32), side: Side::E });
        assert_eq!(choice(end, vec2(12.1, 32.5), &crossing), None);
        assert_eq!(choice(end, vec2(14.0, 32.5), &crossing).map(|c| c.piece.shape), Some(RailShape::X));
    }

    #[test]
    fn the_planner_stops_when_its_track_is_gone() {
        let track = line();
        let mut planner = RailPlanner::default();
        let mut orders = Vec::new();
        planner.press_on(&track, vec2(6.0, 32.5), &mut orders);
        // Built a piece: not on the map yet (its order is), the planner goes on from it.
        planner.press_on(&track, vec2(3.0, 32.5), &mut orders);
        planner.check(&track);
        assert!(planner.from.is_some());
        // Undone: the track it builds from isn't there – it stops instead of building in the air.
        planner.pending = None;
        planner.check(&track);
        assert_eq!(planner.from, None);
    }
}
