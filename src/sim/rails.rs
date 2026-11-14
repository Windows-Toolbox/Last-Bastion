//! TAB's railway (`ZX.Components\CRailWay.cs`): pieces of track on the map (ornaments, `ornaments.rs`) and the lines
//! they make – from a buffer stop along every piece that fits on to the next buffer stop (or the last piece). A train
//! (`trains.rs`) runs along such a line.
//!
//! Every piece has one or two **connectors**: a side of one of its tiles where the track leaves it (TAB's
//! `RailConnection`). Two pieces are joined where a connector of one faces a connector of the other on the next tile.
//! A curve is a quarter circle over 6 × 6 tiles, 5.5 tiles round (TAB's `Width - 0.5`), cut into 90 steps of a degree
//! like TAB's `GetRailwayPointAt`; a straight piece is one tile; a buffer stop has no length – the line starts at its
//! edge.
//!
//! Deterministic like the rest of the simulation: the curves' sine and cosine come from `libm`.

use macroquad::math::{vec2, Vec2};
use serde::{Deserialize, Serialize};

use super::{Ornament, OrnamentKind, World};

/// The shape of a piece of track – TAB's `TypeOfShape`. A curve is named after where it bends to: `CurveSE` comes in
/// from the west at its top left tile and leaves south at its bottom right one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RailShape {
    /// Straight along x (TAB's `StraightX`) and along y.
    X,
    Y,
    CurveNE,
    CurveNW,
    CurveSE,
    CurveSW,
    /// A buffer stop at the north end of a line – the track goes on to the south (TAB's `TerminatorN`) – and so on.
    EndN,
    EndS,
    EndE,
    EndW,
}

/// A curve's radius (tiles): TAB's `Width - 0.5` of its 6 tiles.
const CURVE_RADIUS: f32 = 5.5;

/// Which tiles of its 6 × 6 a curve covers – TAB's `TerrainMask` of the four `RailWayArc90…`, row after row (y), one
/// letter per x. Only these are railway: nothing may be built on or next to them.
const CURVE_MASKS: [(RailShape, [&str; 6]); 4] = [
    (RailShape::CurveSE, ["111000", "011110", "000110", "000011", "000011", "000001"]),
    (RailShape::CurveNE, ["000001", "000011", "000011", "000110", "011110", "111000"]),
    (RailShape::CurveNW, ["100000", "110000", "110000", "011000", "001110", "000111"]),
    (RailShape::CurveSW, ["000111", "001110", "011000", "110000", "110000", "100000"]),
];

impl RailShape {
    /// How many tiles each way it covers.
    pub fn size(self) -> i32 {
        if self.is_curve() { 6 } else { 1 }
    }

    pub fn is_curve(self) -> bool {
        matches!(self, RailShape::CurveNE | RailShape::CurveNW | RailShape::CurveSE | RailShape::CurveSW)
    }

    pub fn is_end(self) -> bool {
        matches!(self, RailShape::EndN | RailShape::EndS | RailShape::EndE | RailShape::EndW)
    }

    /// Its picture in `assets\decor\` (`tools\tab_buildings.py`).
    pub fn picture(self) -> &'static str {
        match self {
            RailShape::X => "rail_x",
            RailShape::Y => "rail_y",
            RailShape::CurveNE => "rail_curve_ne",
            RailShape::CurveNW => "rail_curve_nw",
            RailShape::CurveSE => "rail_curve_se",
            RailShape::CurveSW => "rail_curve_sw",
            RailShape::EndN => "rail_end_n",
            RailShape::EndS => "rail_end_s",
            RailShape::EndE => "rail_end_e",
            RailShape::EndW => "rail_end_w",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            // On screen x runs down to the right (\), y down to the left (/).
            RailShape::X => "Track \\",
            RailShape::Y => "Track /",
            RailShape::CurveNE => "Curve NE",
            RailShape::CurveNW => "Curve NW",
            RailShape::CurveSE => "Curve SE",
            RailShape::CurveSW => "Curve SW",
            RailShape::EndN => "End N",
            RailShape::EndS => "End S",
            RailShape::EndE => "End E",
            RailShape::EndW => "End W",
        }
    }

    /// Its connectors with its top left tile on `tile` – TAB's `Connector1` and `Connector2` (a buffer stop has only
    /// the first).
    fn connectors(self, (x, y): (i32, i32)) -> (Connector, Option<Connector>) {
        let c = |cell: (i32, i32), side: Side| Connector { cell, side };
        let last = self.size() - 1;
        match self {
            RailShape::X => (c((x, y), Side::W), Some(c((x, y), Side::E))),
            RailShape::Y => (c((x, y), Side::N), Some(c((x, y), Side::S))),
            RailShape::CurveSE => (c((x, y), Side::W), Some(c((x + last, y + last), Side::S))),
            RailShape::CurveSW => (c((x + last, y), Side::E), Some(c((x, y + last), Side::S))),
            RailShape::CurveNE => (c((x, y + last), Side::W), Some(c((x + last, y), Side::N))),
            RailShape::CurveNW => (c((x + last, y + last), Side::E), Some(c((x, y), Side::N))),
            RailShape::EndN => (c((x, y), Side::S), None),
            RailShape::EndS => (c((x, y), Side::N), None),
            RailShape::EndE => (c((x, y), Side::W), None),
            RailShape::EndW => (c((x, y), Side::E), None),
        }
    }

    /// The way along it from its first connector to its second (TAB's `GetRailwayPointAt` from `Connector1`): a
    /// straight piece its two ends, a curve a point every degree. A buffer stop: just its connector.
    pub fn points(self, tile: (i32, i32)) -> Vec<Vec2> {
        let (first, second) = self.connectors(tile);
        let start = first.point();
        let Some(second) = second else {
            return vec![start];
        };
        if !self.is_curve() {
            return vec![start, second.point()];
        }
        // Which way x and y go from the start: a curve coming in from the west goes on to the east (+x), …
        let (sx, sy) = match self {
            RailShape::CurveSE => (1.0, 1.0),
            RailShape::CurveSW => (-1.0, 1.0),
            RailShape::CurveNE => (1.0, -1.0),
            _ => (-1.0, -1.0),
        };
        (0..=90)
            .map(|degree| {
                let a = degree as f32 * std::f32::consts::PI / 180.0;
                let (sin, cos) = (libm::sinf(a), libm::cosf(a));
                start + vec2(sx * sin, sy * (1.0 - cos)) * CURVE_RADIUS
            })
            .collect()
    }

    /// Is this tile (counted from its top left one) railway?
    fn covers(self, dx: i32, dy: i32) -> bool {
        let size = self.size();
        if !(0..size).contains(&dx) || !(0..size).contains(&dy) {
            return false;
        }
        match CURVE_MASKS.iter().find(|(shape, _)| *shape == self) {
            Some((_, rows)) => rows[dy as usize].as_bytes()[dx as usize] == b'1',
            None => true,
        }
    }
}

/// A side of a tile, where track leaves it (TAB's `TypeOfConnector`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    N,
    S,
    E,
    W,
}

impl Side {
    fn opposite(self) -> Side {
        match self {
            Side::N => Side::S,
            Side::S => Side::N,
            Side::E => Side::W,
            Side::W => Side::E,
        }
    }

    /// The step to the tile on that side.
    fn step(self) -> (i32, i32) {
        match self {
            Side::N => (0, -1),
            Side::S => (0, 1),
            Side::E => (1, 0),
            Side::W => (-1, 0),
        }
    }

    /// The same as a direction on the map (length 1).
    pub fn dir(self) -> Vec2 {
        let (x, y) = self.step();
        vec2(x as f32, y as f32)
    }

    pub const ALL: [Side; 4] = [Side::N, Side::S, Side::E, Side::W];
}

/// Where a piece's track leaves it: a side of one of its tiles. An open one (nothing joins it) is where the track can
/// go on – the editor's rail planner builds from there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Connector {
    pub cell: (i32, i32),
    pub side: Side,
}

impl Connector {
    /// The middle of that side (TAB's `RailConnection.Position`).
    pub fn point(self) -> Vec2 {
        let (x, y) = (self.cell.0 as f32, self.cell.1 as f32);
        match self.side {
            Side::E => vec2(x + 1.0, y + 0.5),
            Side::W => vec2(x, y + 0.5),
            Side::N => vec2(x + 0.5, y),
            Side::S => vec2(x + 0.5, y + 1.0),
        }
    }

    /// Does `other` face this one from the next tile – are the two pieces joined here (TAB's `ConnectedTo`)?
    fn meets(self, other: Connector) -> bool {
        let (dx, dy) = self.side.step();
        other.cell == (self.cell.0 + dx, self.cell.1 + dy) && other.side == self.side.opposite()
    }
}

/// A piece of track of the map: its shape and its top left tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RailPiece {
    pub shape: RailShape,
    pub tile: (i32, i32),
}

impl RailPiece {
    /// Where its track leaves it: two connectors – a buffer stop only the first.
    pub fn connectors(self) -> (Connector, Option<Connector>) {
        self.shape.connectors(self.tile)
    }

    /// Does it lie on this tile?
    pub fn covers(self, (x, y): (i32, i32)) -> bool {
        self.shape.covers(x - self.tile.0, y - self.tile.1)
    }
}

/// The pieces of track among the ornaments.
pub fn rail_pieces(ornaments: &[Ornament]) -> Vec<RailPiece> {
    ornaments
        .iter()
        .filter_map(|o| match o.kind {
            OrnamentKind::Rail(shape) => Some(RailPiece { shape, tile: o.tile }),
            _ => None,
        })
        .collect()
}

/// Is nothing joined to connector `c` yet?
fn is_open(pieces: &[RailPiece], c: Connector) -> bool {
    !pieces.iter().any(|p| {
        let (a, b) = p.connectors();
        c.meets(a) || b.is_some_and(|b| c.meets(b))
    })
}

/// Is something joined to connector `c` – does track go on there?
pub fn joined(pieces: &[RailPiece], c: Connector) -> bool {
    !is_open(pieces, c)
}

/// Every open end of the track: a connector nothing joins.
pub fn open_ends(pieces: &[RailPiece]) -> Vec<Connector> {
    pieces
        .iter()
        .flat_map(|p| {
            let (a, b) = p.connectors();
            [Some(a), b]
        })
        .flatten()
        .filter(|&c| is_open(pieces, c))
        .collect()
}

/// Which way a piece bends, seen from where it's built on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    Straight,
    /// To the side `Connector::side` … turned a quarter: towards `+perpendicular` (x → y) or the other way.
    Plus,
    Minus,
    /// A buffer stop: the line ends.
    End,
}

/// A piece that can go on from an open end: its shape, its top left tile, which way it bends and where the track
/// leaves it (`None` for a buffer stop).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extension {
    pub shape: RailShape,
    pub tile: (i32, i32),
    pub turn: Turn,
    pub exit: Option<Connector>,
}

/// Every piece that joins open end `from` – straight on, the two curves, a buffer stop (TAB's `ConnectTo`: the new
/// piece's connector on the next tile, facing back).
pub fn extensions(from: Connector) -> Vec<Extension> {
    let (dx, dy) = from.side.step();
    let cell = (from.cell.0 + dx, from.cell.1 + dy);
    let ahead = from.side.dir();
    let across = vec2(-ahead.y, ahead.x);
    let shapes = [
        RailShape::X,
        RailShape::Y,
        RailShape::CurveNE,
        RailShape::CurveNW,
        RailShape::CurveSE,
        RailShape::CurveSW,
        RailShape::EndN,
        RailShape::EndS,
        RailShape::EndE,
        RailShape::EndW,
    ];
    let mut out = Vec::new();
    for shape in shapes {
        let (a, b) = shape.connectors((0, 0));
        let Some((mine, other)) = [(Some(a), b), (b, Some(a))].into_iter().find_map(|(m, o)| m.filter(|m| m.side == from.side.opposite()).map(|m| (m, o)))
        else {
            continue;
        };
        let tile = (cell.0 - mine.cell.0, cell.1 - mine.cell.1);
        let exit = other.map(|o| Connector { cell: (o.cell.0 + tile.0, o.cell.1 + tile.1), side: o.side });
        let turn = match exit {
            None => Turn::End,
            Some(e) if e.side == from.side => Turn::Straight,
            Some(e) if e.side.dir().dot(across) > 0.0 => Turn::Plus,
            Some(_) => Turn::Minus,
        };
        out.push(Extension { shape, tile, turn, exit });
    }
    out
}

/// A new line starting on `tile`: a buffer stop there whose track leaves towards `side`.
pub fn line_start(tile: (i32, i32), side: Side) -> Extension {
    let shape = match side {
        Side::S => RailShape::EndN,
        Side::N => RailShape::EndS,
        Side::W => RailShape::EndE,
        Side::E => RailShape::EndW,
    };
    Extension { shape, tile, turn: Turn::Straight, exit: Some(Connector { cell: tile, side }) }
}

/// Where a piece of `shape` goes so that it joins the track: its top left tile such that one of its connectors faces
/// an open connector of a piece already there (TAB's `ConnectTo` when its editor lays a line) – the open connector
/// closest to `near` (tiles), if one is closer than `reach`. `None`: none that close, or it fits none of them.
pub fn snap(pieces: &[RailPiece], shape: RailShape, near: Vec2, reach: f32) -> Option<(i32, i32)> {
    let open = |c: Connector| is_open(pieces, c);
    let mut best: Option<((i32, i32), f32)> = None;
    for piece in pieces {
        let (a, b) = piece.connectors();
        for there in [Some(a), b].into_iter().flatten().filter(|&c| open(c)) {
            let d = there.point().distance(near);
            if d >= reach || best.is_some_and(|(_, bd)| bd <= d) {
                continue;
            }
            // The new piece's connector must sit on the next tile, facing back.
            let (dx, dy) = there.side.step();
            let cell = (there.cell.0 + dx, there.cell.1 + dy);
            let (mine_a, mine_b) = shape.connectors((0, 0));
            if let Some(mine) = [Some(mine_a), mine_b].into_iter().flatten().find(|m| m.side == there.side.opposite()) {
                best = Some(((cell.0 - mine.cell.0, cell.1 - mine.cell.1), d));
            }
        }
    }
    best.map(|(tile, _)| tile)
}

/// A line of track a train runs along (TAB's `TrainLine`): the way from one end to the other as points, and how far
/// along the way each point lies. `pieces`: where each piece's stretch of the way starts and how long it is – only the
/// ones with a length (not the buffer stops).
#[derive(Clone, Debug, PartialEq)]
pub struct RailLine {
    points: Vec<Vec2>,
    along: Vec<f32>,
    pieces: Vec<(f32, f32)>,
    /// The buffer stop it starts at, and the one it ends at (or the last piece, if the track just stops).
    pub start: (i32, i32),
    pub end: (i32, i32),
}

impl RailLine {
    /// The line that starts at buffer stop `origin` of `pieces` – along every piece that joins on, until a buffer
    /// stop or a piece nothing joins (TAB's `TrainLine.UpdateFrom`).
    fn from(pieces: &[RailPiece], origin: usize) -> RailLine {
        let mut used = vec![false; pieces.len()];
        used[origin] = true;
        let (mut exit, _) = pieces[origin].connectors();
        let mut line = RailLine { points: vec![exit.point()], along: vec![0.0], pieces: Vec::new(), start: pieces[origin].tile, end: pieces[origin].tile };
        loop {
            // The next piece: one whose connector faces where the track left the last one.
            let next = (0..pieces.len()).filter(|&p| !used[p]).find_map(|p| {
                let (a, b) = pieces[p].connectors();
                if exit.meets(a) {
                    Some((p, false))
                } else if b.is_some_and(|b| exit.meets(b)) {
                    Some((p, true))
                } else {
                    None
                }
            });
            let Some((p, backwards)) = next else {
                break;
            };
            used[p] = true;
            let piece = pieces[p];
            line.end = piece.tile;
            if piece.shape.is_end() {
                break;
            }
            let mut way = piece.shape.points(piece.tile);
            let (a, b) = piece.connectors();
            if backwards {
                way.reverse();
                exit = a;
            } else {
                exit = b.expect("only buffer stops have one connector");
            }
            let from = line.length();
            for point in way.into_iter().skip(1) {
                let last = *line.points.last().expect("a line has a first point");
                let d = point.distance(last);
                line.points.push(point);
                line.along.push(line.length() + d);
            }
            line.pieces.push((from, line.length() - from));
        }
        line
    }

    /// The same line the other way round – it starts where this one ends.
    pub fn reversed(&self) -> RailLine {
        let total = self.length();
        let points: Vec<Vec2> = self.points.iter().rev().copied().collect();
        let along: Vec<f32> = self.along.iter().rev().map(|a| total - a).collect();
        let pieces = self.pieces.iter().rev().map(|&(from, len)| (total - from - len, len)).collect();
        RailLine { points, along, pieces, start: self.end, end: self.start }
    }

    /// How long it is (tiles) – TAB's `RailwayPathDistance`.
    pub fn length(&self) -> f32 {
        *self.along.last().expect("a line has a first point")
    }

    /// The point `offset` tiles along it – before its start its first point, past its end its last (TAB's
    /// `GetNextPoint`).
    pub fn point_at(&self, offset: f32) -> Vec2 {
        let offset = offset.clamp(0.0, self.length());
        // The first point at least that far along; the one before it starts the stretch the offset lies on.
        let i = self.along.partition_point(|&a| a < offset).clamp(1, self.points.len().max(2) - 1);
        if self.points.len() < 2 {
            return self.points[0];
        }
        let (a, b) = (self.along[i - 1], self.along[i]);
        let t = if b > a { (offset - a) / (b - a) } else { 0.0 };
        self.points[i - 1].lerp(self.points[i], t)
    }

    /// Which way the line runs at `offset` – from 0.1 tiles before to 0.1 after (TAB's `GetAngleAtPoint`), as a
    /// direction of length 1.
    pub fn direction_at(&self, offset: f32) -> Vec2 {
        let way = self.point_at(offset + 0.1) - self.point_at(offset - 0.1);
        if way.length_squared() > 0.0 { way.normalize() } else { vec2(1.0, 0.0) }
    }

    /// The offset of the middle of the piece of track closest to `to` (TAB's `GetNearestOffsetTo`: where a train stops
    /// for a place). `None`: the line has no piece with a length.
    pub fn stop_near(&self, to: Vec2) -> Option<f32> {
        self.pieces
            .iter()
            .map(|&(from, len)| from + len / 2.0)
            .min_by(|&a, &b| self.point_at(a).distance_squared(to).total_cmp(&self.point_at(b).distance_squared(to)))
    }
}

impl World {
    /// The lines of track on the map: one from every buffer stop that isn't the far end of a line found before – in
    /// the order the ornaments lie in.
    pub fn rail_lines(&self) -> Vec<RailLine> {
        let pieces = rail_pieces(&self.ornaments);
        let mut lines: Vec<RailLine> = Vec::new();
        for (p, piece) in pieces.iter().enumerate().filter(|(_, p)| p.shape.is_end()) {
            if lines.iter().any(|l| l.end == piece.tile) {
                continue;
            }
            lines.push(RailLine::from(&pieces, p));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(shape: RailShape, tile: (i32, i32)) -> RailPiece {
        RailPiece { shape, tile }
    }

    #[test]
    fn a_line_runs_from_buffer_stop_to_buffer_stop() {
        // A buffer stop at the west end (its track goes east), three straight pieces, a curve down to the south, two
        // pieces south, and a buffer stop at the south end (its track goes north).
        let mut pieces = vec![piece(RailShape::EndW, (10, 5))];
        for x in 11..14 {
            pieces.push(piece(RailShape::X, (x, 5)));
        }
        // The curve comes in from the west at its top left tile (14, 5) and leaves south at (19, 10).
        pieces.push(piece(RailShape::CurveSE, (14, 5)));
        pieces.push(piece(RailShape::Y, (19, 11)));
        pieces.push(piece(RailShape::Y, (19, 12)));
        pieces.push(piece(RailShape::EndS, (19, 13)));
        let line = RailLine::from(&pieces, 0);
        assert_eq!(line.end, (19, 13));
        let quarter = std::f32::consts::FRAC_PI_2 * CURVE_RADIUS;
        assert!((line.length() - (3.0 + quarter + 2.0)).abs() < 0.01, "{}", line.length());
        assert_eq!(line.point_at(0.0), vec2(11.0, 5.5));
        assert_eq!(line.point_at(line.length()), vec2(19.5, 13.0));
        assert!(line.direction_at(1.0).distance(vec2(1.0, 0.0)) < 1e-4);
        assert!(line.direction_at(line.length() - 0.5).distance(vec2(0.0, 1.0)) < 1e-4);
        // The other way round it starts at the far end.
        let back = line.reversed();
        assert_eq!(back.point_at(0.0), vec2(19.5, 13.0));
        assert!((back.length() - line.length()).abs() < 1e-4);
        // A train stops in the middle of the piece closest to the place.
        let stop = line.stop_near(vec2(12.5, 8.0)).unwrap();
        assert!((stop - 1.5).abs() < 1e-4, "{stop}");
    }

    #[test]
    fn a_new_piece_snaps_onto_the_open_end() {
        let pieces = vec![piece(RailShape::EndW, (10, 5)), piece(RailShape::X, (11, 5))];
        assert_eq!(snap(&pieces, RailShape::X, vec2(12.6, 5.4), 3.0), Some((12, 5)));
        // A curve going south from there: its top left tile right after the track.
        assert_eq!(snap(&pieces, RailShape::CurveSE, vec2(13.0, 6.0), 3.0), Some((12, 5)));
        // A straight piece along y fits no connector facing east.
        assert_eq!(snap(&pieces, RailShape::Y, vec2(12.6, 5.4), 3.0), None);
        // Too far away.
        assert_eq!(snap(&pieces, RailShape::X, vec2(30.0, 5.4), 3.0), None);
    }

    #[test]
    fn the_planner_offers_straight_two_curves_and_a_buffer_stop() {
        // Going east from tile (4, 4).
        let from = Connector { cell: (4, 4), side: Side::E };
        let ext = extensions(from);
        let find = |turn: Turn| ext.iter().find(|e| e.turn == turn).copied().unwrap();
        assert_eq!(ext.len(), 4);
        assert_eq!(find(Turn::Straight).shape, RailShape::X);
        assert_eq!(find(Turn::Straight).tile, (5, 4));
        assert_eq!(find(Turn::End).shape, RailShape::EndE);
        // +perpendicular of east is +y (south): the curve bending south, its top left tile right after the track.
        assert_eq!(find(Turn::Plus).shape, RailShape::CurveSE);
        assert_eq!(find(Turn::Plus).tile, (5, 4));
        assert_eq!(find(Turn::Plus).exit.unwrap().side, Side::S);
        assert_eq!(find(Turn::Minus).shape, RailShape::CurveNE);
        assert_eq!(find(Turn::Minus).tile, (5, -1));
        // Built on, the old end isn't open any more – the new piece's far end is.
        let pieces = vec![piece(RailShape::EndW, (4, 4)), piece(RailShape::X, (5, 4))];
        assert_eq!(open_ends(&pieces), vec![Connector { cell: (5, 4), side: Side::E }]);
        // Going west: the curve that comes in from the east at its top right tile.
        let west = extensions(Connector { cell: (4, 4), side: Side::W });
        assert!(west.iter().any(|e| e.shape == RailShape::CurveSW && e.tile == (-2, 4) && e.turn == Turn::Minus));
    }

    #[test]
    fn a_curve_covers_only_its_mask() {
        let curve = piece(RailShape::CurveSE, (0, 0));
        assert!(curve.covers((0, 0)) && curve.covers((2, 0)) && curve.covers((5, 5)));
        assert!(!curve.covers((5, 0)) && !curve.covers((0, 5)) && !curve.covers((6, 6)));
    }
}
