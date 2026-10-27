//! TAB's noise layers (`DXVision\DXNoyseLayer.cs`) – what its map generator (`tab.rs`) is built of. A layer is one
//! number from 0 to 1 per tile, like a grey picture of the map: filled with noise, added to and taken from other
//! layers, cut at a threshold into 0 and 1 ("land or water"). Every value is kept between 0 and 1 – as in TAB, where
//! every write is clamped. The same in Python would be a numpy array with `np.clip` after every step.
//!
//! The rectangles here are TAB's: `(left, top, width, height)` in tiles.

use crate::noise::simplex_octaves;

/// A rectangle of tiles: left, top, width, height.
pub type Rect = (i32, i32, i32, i32);

#[derive(Clone)]
pub struct Layer {
    pub n: i32,
    cells: Vec<f64>,
}

impl Layer {
    /// `n` × `n` tiles, all 0.
    pub fn new(n: i32) -> Layer {
        Layer { n, cells: vec![0.0; (n * n) as usize] }
    }

    pub fn get(&self, x: i32, y: i32) -> f64 {
        self.cells[(y * self.n + x) as usize]
    }

    /// Set – kept between 0 and 1, like every value of a layer.
    pub fn set(&mut self, x: i32, y: i32, value: f64) {
        self.cells[(y * self.n + x) as usize] = value.clamp(0.0, 1.0);
    }

    pub fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.n && y < self.n
    }

    /// Every tile, column by column like TAB's loops (x outside, y inside) – where the order matters
    /// (`delete_alone_cells`), it's TAB's.
    fn tiles(&self) -> impl Iterator<Item = (i32, i32)> + use<> {
        let n = self.n;
        (0..n).flat_map(move |x| (0..n).map(move |y| (x, y)))
    }

    /// Every tile gets `f` of its old value (and its place).
    fn each(&mut self, f: impl Fn(i32, i32, f64) -> f64) {
        for (x, y) in self.tiles() {
            let v = self.get(x, y);
            self.set(x, y, f(x, y, v));
        }
    }

    /// TAB's `FillWithNoyse`: the value at tile (x, y) is the noise at (`dx` + `fx`·x, `dy` + `fy`·y) in slice `z`. Small
    /// `fx`, `fy`: big, smooth blobs; `octaves`: finer detail on top.
    pub fn fill_noise(&mut self, z: f64, fx: f64, fy: f64, dx: f64, dy: f64, octaves: u32) {
        self.each(|x, y, _| simplex_octaves(dx + fx * x as f64, dy + fy * y as f64, z, octaves) as f64);
    }

    pub fn add(&mut self, other: &Layer) {
        self.each(|x, y, v| v + other.get(x, y));
    }

    pub fn subtract(&mut self, other: &Layer) {
        self.each(|x, y, v| v - other.get(x, y));
    }

    pub fn multiply(&mut self, other: &Layer) {
        self.each(|x, y, v| v * other.get(x, y));
    }

    /// Spreads the values away from 0.5 (`value` > 1) – more of them end up at 0 or 1.
    pub fn contrast(&mut self, value: f64) {
        self.each(|_, _, v| value * (v - 0.5) + 0.5);
    }

    /// TAB's `TruncateTo01`: 1 where the value is at least 1 − `share`, else 0 – the bigger `share`, the more 1s.
    /// `block` > 1: then in blocks of `block` × `block` tiles – a whole block is 1 if its share of 1s is at least
    /// `share` (TAB's mountains, in blocks of 2).
    pub fn truncate(&mut self, share: f64, block: i32) {
        self.each(|_, _, v| if v >= 1.0 - share { 1.0 } else { 0.0 });
        if block <= 1 {
            return;
        }
        for bx in 0..self.n / block {
            for by in 0..self.n / block {
                let r = (bx * block, by * block, block, block);
                let filled = if self.total(r) / (block * block) as f64 >= share { 1.0 } else { 0.0 };
                self.set_rect(r, filled);
            }
        }
    }

    /// TAB's `Enlarge`: every tile with a tile of at least `value` within `radius` (a square) gets `value` – the
    /// layer grows by `radius` tiles. Once along x, then along y: a square, in a fraction of the time.
    pub fn enlarge(&mut self, radius: i32, value: f64) {
        let n = self.n;
        let mut reach = vec![false; (n * n) as usize];
        for y in 0..n {
            for x in 0..n {
                reach[(y * n + x) as usize] = (x - radius..=x + radius).any(|sx| self.inside(sx, y) && self.get(sx, y) >= value);
            }
        }
        for (x, y) in self.tiles() {
            if self.get(x, y) < value && (y - radius..=y + radius).any(|sy| sy >= 0 && sy < n && reach[(sy * n + x) as usize]) {
                self.set(x, y, value);
            }
        }
    }

    /// TAB's `SubstractCircle`: less by up to 1 in the middle of the circle, down to nothing at its edge – what lies
    /// close to (`cx`, `cy`) drops below any threshold.
    pub fn subtract_circle(&mut self, cx: i32, cy: i32, radius: i32) {
        let r2 = (radius * radius) as f64;
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                let d = (dx * dx + dy * dy) as f64 / r2;
                if self.inside(cx + dx, cy + dy) && d < 1.0 {
                    let v = self.get(cx + dx, cy + dy);
                    self.set(cx + dx, cy + dy, v - (1.0 - d));
                }
            }
        }
    }

    pub fn set_rect(&mut self, (l, t, w, h): Rect, value: f64) {
        for x in l..l + w {
            for y in t..t + h {
                if self.inside(x, y) {
                    self.set(x, y, value);
                }
            }
        }
    }

    /// All values of the rectangle added up (tiles off the layer count 0).
    pub fn total(&self, (l, t, w, h): Rect) -> f64 {
        let mut sum = 0.0;
        for x in l..l + w {
            for y in t..t + h {
                if self.inside(x, y) {
                    sum += self.get(x, y);
                }
            }
        }
        sum
    }

    /// The smallest value in the rectangle – 0 if any of it is off the layer.
    pub fn min(&self, (l, t, w, h): Rect) -> f64 {
        let mut least = 1.0f64;
        for x in l..l + w {
            for y in t..t + h {
                least = least.min(if self.inside(x, y) { self.get(x, y) } else { 0.0 });
            }
        }
        least
    }

    /// TAB's `IsValidArea`: the rectangle lies on the layer, a tile away from its right and bottom edge.
    fn valid(&self, (l, t, w, h): Rect) -> bool {
        l >= 0 && t >= 0 && l + w < self.n && t + h < self.n
    }

    /// TAB's `GetAreaNearPoint`: the closest rectangle of `w` × `h` to (`cx`, `cy`) – its middle going round in a
    /// spiral (`spiral`) – where every value is at least `least`. `None` once the spiral leaves the layer.
    pub fn area_near(&self, cx: i32, cy: i32, w: i32, h: i32, least: f64) -> Option<Rect> {
        for (x, y) in spiral(cx, cy, true) {
            let r = (x - w / 2, y - h / 2, w, h);
            if !self.valid(r) {
                return None;
            }
            if self.min(r) >= least {
                return Some(r);
            }
        }
        None
    }

    /// TAB's `DeleteAloneCells`: a tile of at least `least` with no such tile among its 8 neighbours becomes 0 – no
    /// single specks. Everything below `least` becomes 0 too. Tile by tile in TAB's order: one already cleared
    /// counts as cleared for the ones after it.
    pub fn delete_alone_cells(&mut self, least: f64) {
        for (x, y) in self.tiles() {
            if self.get(x, y) < least {
                self.set(x, y, 0.0);
                continue;
            }
            let neighbours = [(-1, -1), (-1, 0), (-1, 1), (1, -1), (1, 0), (1, 1), (0, -1), (0, 1)];
            let alone = !neighbours.iter().any(|&(dx, dy)| self.inside(x + dx, y + dy) && self.get(x + dx, y + dy) >= least);
            if alone {
                self.set(x, y, 0.0);
            }
        }
    }
}

/// The tiles around (`cx`, `cy`) ring after ring, each ring the same way round – TAB's `NearPoints` (`self_first`:
/// the tile itself first). Endless: whoever uses it stops it.
pub fn spiral(cx: i32, cy: i32, self_first: bool) -> impl Iterator<Item = (i32, i32)> {
    let first = self_first.then_some((cx, cy));
    first.into_iter().chain((1..).flat_map(move |r| {
        let bottom_right = (0..=r).map(move |x| (cx + x, cy + r));
        let right = (-r + 1..=r - 1).rev().map(move |y| (cx + r, cy + y));
        let top = (-r..=r).rev().map(move |x| (cx + x, cy - r));
        let left = (-r + 1..=r - 1).map(move |y| (cx - r, cy + y));
        let bottom_left = (-r..0).map(move |x| (cx + x, cy + r));
        bottom_right.chain(right).chain(top).chain(left).chain(bottom_left)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cutting at a threshold, growing, a circle taken away, single specks deleted – like TAB's layers.
    #[test]
    fn layers_like_tab() {
        let mut a = Layer::new(20);
        a.set(10, 10, 1.0);
        a.enlarge(2, 1.0);
        assert_eq!(a.total((0, 0, 20, 20)), 25.0, "a 5 x 5 square");
        a.set(0, 0, 0.7);
        a.truncate(0.4, 1);
        assert_eq!(a.get(0, 0), 1.0);
        a.delete_alone_cells(0.5);
        assert_eq!(a.get(0, 0), 0.0, "a lone speck stays");
        a.subtract_circle(10, 10, 3);
        assert_eq!(a.get(10, 10), 0.0);
        assert!((a.get(12, 10) - 4.0 / 9.0).abs() < 1e-9, "less towards the middle");
        assert!((a.get(12, 12) - 8.0 / 9.0).abs() < 1e-9, "hardly less at the edge");
        a.set(3, 3, 7.0);
        assert_eq!(a.get(3, 3), 1.0, "kept between 0 and 1");
        // The spiral: itself, then the first ring of 8, every tile once.
        let ring: Vec<(i32, i32)> = spiral(0, 0, true).take(9).collect();
        assert_eq!(ring[0], (0, 0));
        let mut sorted = ring.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 9);
        assert!(ring.iter().all(|&(x, y)| x.abs() <= 1 && y.abs() <= 1));
        assert_eq!(spiral(0, 0, false).take(24).filter(|&(x, y)| x.abs().max(y.abs()) == 2).count(), 16);
    }
}
