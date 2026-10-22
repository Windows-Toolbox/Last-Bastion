//! Formations like in *Diplomacy is Not an Option*: hold the right mouse button and drag – the units line up along
//! the dragged line in the chosen shape. A plain click puts them into the same shape at the point, facing the way
//! they walk. Soldiers go in front (or on the outside of the circle), snipers and medics behind them.
//!
//! Only the spots are computed here – one per unit. Walking there is the simulation's job (`sim.rs`).
//! Deterministic like the rest of the simulation (DESIGN.md 12.8): same input → same spots.

use std::f32::consts::TAU;

use macroquad::math::{vec2, Vec2};

use crate::kinds::UnitKind;

/// Distance between two neighbours in a formation (tiles, centre to centre). A unit is 0.35 tiles wide.
pub const SPACING: f32 = 0.5;
/// A long line with few units: they spread out, but never further apart than this.
const MAX_SPACING: f32 = 1.25;
/// Dragged less than this: counts as a click.
pub const MIN_DRAG: f32 = 0.75;

/// The shapes to choose from (buttons on the command card).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    /// Rows as wide as the dragged line – the more units, the more rows behind it.
    Line,
    /// A square block: the dragged line is its front.
    Box,
    /// A ring around the point where the drag started, the drag is the radius. Several rings if needed.
    Circle,
}

/// A formation as the player draws it: shape plus the dragged line (`from` = where the mouse button went down,
/// `to` = where it was released; the same point for a click).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Formation {
    pub shape: Shape,
    pub from: Vec2,
    pub to: Vec2,
}

/// One spot per unit, in the same order as `units` (where each one stands, and its type).
pub fn spots(formation: Formation, units: &[(Vec2, UnitKind)]) -> Vec<Vec2> {
    let Formation { shape, from, to } = formation;
    let n = units.len();
    let drag = to - from;
    let dragged = drag.length() >= MIN_DRAG;
    match n {
        0 => return Vec::new(),
        1 if shape == Shape::Circle || !dragged => return vec![from],
        1 => return vec![(from + to) / 2.0],
        _ => {}
    }
    match shape {
        Shape::Line | Shape::Box => rows(shape, from, to, dragged, units),
        Shape::Circle => rings(from, if dragged { drag.length() } else { 0.0 }, units),
    }
}

/// Line and box: rows along the front, one behind the other.
fn rows(shape: Shape, from: Vec2, to: Vec2, dragged: bool, units: &[(Vec2, UnitKind)]) -> Vec<Vec2> {
    let n = units.len();
    let middle = units.iter().map(|u| u.0).sum::<Vec2>() / n as f32;

    // `along`: the direction of the front line. `front_mid`: its middle.
    let (along, front_mid, length) = if dragged {
        let drag = to - from;
        (drag / drag.length(), (from + to) / 2.0, drag.length())
    } else {
        // A click: the front faces the way they walk – so the line runs across that way.
        let facing = (from - middle).try_normalize().unwrap_or(vec2(0.0, -1.0));
        (vec2(-facing.y, facing.x), from, 0.0)
    };
    // `back`: from the front towards the rows behind it – to the side the units come from.
    let mut back = vec2(-along.y, along.x);
    if (middle - front_mid).dot(back) < 0.0 {
        back = -back;
    }

    // How many stand side by side in one row, and how far apart.
    let per_row = match shape {
        Shape::Box => (n as f32).sqrt().ceil() as usize,
        _ if dragged => ((length / SPACING) as usize + 1).min(n),
        // Clicked: about three times as wide as deep.
        _ => ((n as f32 * 3.0).sqrt().ceil() as usize).min(n),
    }
    .max(1);
    let gap = if dragged && per_row > 1 { (length / (per_row - 1) as f32).clamp(SPACING, MAX_SPACING) } else { SPACING };

    // Who goes where: soldiers first (front rows), and within a type the ones that are already further in
    // front. Row after row is filled; inside a row they're sorted from left to right, so nobody crosses paths.
    let order = sorted(units, |p| -(p - front_mid).dot(back));
    let mut spots = vec![Vec2::ZERO; n];
    for (row, members) in order.chunks(per_row).enumerate() {
        let mut members = members.to_vec();
        members.sort_by(|&a, &b| {
            let (pa, pb) = ((units[a].0 - front_mid).dot(along), (units[b].0 - front_mid).dot(along));
            pa.total_cmp(&pb).then(a.cmp(&b))
        });
        let half = (members.len() - 1) as f32 / 2.0;
        for (k, &u) in members.iter().enumerate() {
            spots[u] = front_mid + back * (row as f32 * SPACING) + along * ((k as f32 - half) * gap);
        }
    }
    spots
}

/// Circle: rings around `center`, the outer one with radius `radius` (0 = just big enough). Soldiers outside.
fn rings(center: Vec2, radius: f32, units: &[(Vec2, UnitKind)]) -> Vec<Vec2> {
    let n = units.len();
    // How many fit on one ring, and in all rings from radius `r` inwards (plus one in the middle).
    let fit = |r: f32| ((TAU * r / SPACING) as usize).max(1);
    let capacity = |mut r: f32| {
        let mut total = 1;
        while r >= SPACING / 2.0 {
            total += fit(r);
            r -= SPACING;
        }
        total
    };
    // Clicked: a ring just big enough for everyone. Dragged too small: the outer ring grows until all fit.
    let mut r = if radius > 0.0 { radius } else { n as f32 * SPACING / TAU }.max(SPACING);
    while capacity(r) < n {
        r += SPACING;
    }
    let order = sorted(units, |p| p.distance(center));
    let mut spots = vec![Vec2::ZERO; n];
    let mut next = 0;
    while next < n {
        if r < SPACING / 2.0 {
            spots[order[next]] = center; // the last one stands in the middle
            break;
        }
        let take = fit(r).min(n - next);
        let mut ring: Vec<usize> = order[next..next + take].to_vec();
        next += take;
        // Around the ring in the order they stand around the centre, so nobody crosses paths.
        let angle = |u: usize| {
            let d = units[u].0 - center;
            libm::atan2f(d.y, d.x)
        };
        ring.sort_by(|&a, &b| angle(a).total_cmp(&angle(b)).then(a.cmp(&b)));
        let start = angle(ring[0]);
        for (k, &u) in ring.iter().enumerate() {
            let a = start + k as f32 * TAU / take as f32;
            spots[u] = center + vec2(libm::cosf(a), libm::sinf(a)) * r;
        }
        r -= SPACING;
    }
    spots
}

/// The units' indices: by type (soldiers first), within a type by `key` (smaller first), the same → by index.
fn sorted(units: &[(Vec2, UnitKind)], key: impl Fn(Vec2) -> f32) -> Vec<usize> {
    let mut order: Vec<usize> = (0..units.len()).collect();
    order.sort_by(|&a, &b| {
        units[a].1.cmp(&units[b].1).then(key(units[a].0).total_cmp(&key(units[b].0))).then(a.cmp(&b))
    });
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(kinds: &[UnitKind]) -> Vec<(Vec2, UnitKind)> {
        kinds.iter().enumerate().map(|(i, &k)| (vec2(10.0 + i as f32, 30.0), k)).collect()
    }

    fn no_two_closer_than(spots: &[Vec2], min: f32) -> bool {
        spots.iter().enumerate().all(|(i, a)| spots[i + 1..].iter().all(|b| a.distance(*b) >= min - 0.001))
    }

    /// Dragged line of 3 tiles with 10 units: 7 in the front row exactly on the line, 3 behind it – on the side
    /// the units come from. Nobody stands on anyone else.
    #[test]
    fn line_follows_the_drag_and_rows_go_backwards() {
        let units = group(&[UnitKind::Soldier; 10]);
        let f = Formation { shape: Shape::Line, from: vec2(10.0, 10.0), to: vec2(13.0, 10.0) };
        let spots = spots(f, &units);
        assert_eq!(spots.len(), 10);
        assert_eq!(spots.iter().filter(|p| (p.y - 10.0).abs() < 0.01).count(), 7);
        // The units stand at y = 30, so the second row is at y = 10.5 (towards them), not y = 9.5.
        assert_eq!(spots.iter().filter(|p| (p.y - 10.0 - SPACING).abs() < 0.01).count(), 3);
        assert!(no_two_closer_than(&spots, SPACING));
    }

    /// Soldiers in front, snipers and medics behind – no matter how they stood before.
    #[test]
    fn soldiers_go_in_front() {
        use UnitKind::*;
        let units = group(&[Medic, Sniper, Soldier, Sniper, Soldier, Soldier]);
        let f = Formation { shape: Shape::Line, from: vec2(10.0, 10.0), to: vec2(11.0, 10.0) };
        let spots = spots(f, &units);
        for (i, (_, kind)) in units.iter().enumerate() {
            let front = (spots[i].y - 10.0).abs() < 0.01;
            assert_eq!(front, *kind == Soldier, "unit {i} ({:?})", spots[i]);
        }
    }

    /// Box: a square block (3 × 3 for 9 units), the dragged line is its front.
    #[test]
    fn box_is_square() {
        let units = group(&[UnitKind::Soldier; 9]);
        let f = Formation { shape: Shape::Box, from: vec2(20.0, 10.0), to: vec2(21.0, 10.0) };
        let spots = spots(f, &units);
        let rows: std::collections::BTreeSet<i32> = spots.iter().map(|p| (p.y * 100.0).round() as i32).collect();
        let cols: std::collections::BTreeSet<i32> = spots.iter().map(|p| (p.x * 100.0).round() as i32).collect();
        assert_eq!((rows.len(), cols.len()), (3, 3));
        assert!(no_two_closer_than(&spots, SPACING));
    }

    /// Circle dragged with radius 2: everyone on the ring (12 units fit on it easily).
    #[test]
    fn circle_puts_everyone_on_the_ring() {
        let units = group(&[UnitKind::Soldier; 12]);
        let center = vec2(50.0, 50.0);
        let f = Formation { shape: Shape::Circle, from: center, to: center + vec2(2.0, 0.0) };
        let spots = spots(f, &units);
        assert!(spots.iter().all(|p| (p.distance(center) - 2.0).abs() < 0.01));
        assert!(no_two_closer_than(&spots, SPACING));
    }

    /// Circle too small for everyone: rings inside it, and if even that isn't enough, the circle grows.
    /// Nobody stands on anyone else.
    #[test]
    fn small_circle_gets_inner_rings() {
        let units = group(&[UnitKind::Soldier; 30]);
        let center = vec2(50.0, 50.0);
        let f = Formation { shape: Shape::Circle, from: center, to: center + vec2(1.0, 0.0) };
        let spots = spots(f, &units);
        assert_eq!(spots.len(), 30);
        // The rings lie `SPACING` apart.
        let radii: std::collections::BTreeSet<i32> = spots.iter().map(|p| (p.distance(center) / SPACING).round() as i32).collect();
        assert!(radii.len() >= 2, "only one ring: {radii:?}");
        assert!(no_two_closer_than(&spots, SPACING * 0.9));
    }

    /// One unit simply goes where you clicked.
    #[test]
    fn single_unit_goes_to_the_point() {
        let units = group(&[UnitKind::Sniper]);
        let p = vec2(5.0, 5.0);
        assert_eq!(spots(Formation { shape: Shape::Line, from: p, to: p }, &units), vec![p]);
    }
}
