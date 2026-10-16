//! Smooth noise for the map generation (DESIGN.md 9.1): a value for every point that changes slowly and without
//! seams – like hills on a height map. Neighbouring points get similar values, far apart ones unrelated ones.
//!
//! "Value noise": on a coarse grid every corner gets a random number (always the same one for the same seed and
//! corner), and in between it's blended smoothly. Several layers on top of each other ("octaves": each one twice
//! as fine and half as strong) make big lakes with frayed shores instead of round blobs.
//! Only plain arithmetic – the same result on every PC (DESIGN.md 12.8).

pub struct Noise {
    seed: u32,
}

impl Noise {
    pub fn new(seed: u32) -> Noise {
        Noise { seed }
    }

    /// The random number of grid corner (x, y): 0.0 to 1.0. A hash function mixes the bits of x, y and the seed so
    /// thoroughly that neighbouring corners have nothing to do with each other.
    fn corner(&self, x: i32, y: i32) -> f32 {
        let mut h = self.seed ^ (x as u32).wrapping_mul(0x27d4_eb2d) ^ (y as u32).wrapping_mul(0x1656_67b1);
        h = (h ^ (h >> 15)).wrapping_mul(0x85eb_ca6b);
        h = (h ^ (h >> 13)).wrapping_mul(0xc2b2_ae35);
        h ^= h >> 16;
        (h >> 8) as f32 / 16_777_216.0 // the top 24 bits, like `Rng::unit_f32`
    }

    /// One layer: the four corners around the point, blended. `smooth` makes the blend curved (an "S"), so there
    /// are no visible kinks at the grid lines.
    fn layer(&self, x: f32, y: f32) -> f32 {
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (ix, iy) = (x0 as i32, y0 as i32);
        let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
        let (sx, sy) = (smooth(fx), smooth(fy));
        let top = lerp(self.corner(ix, iy), self.corner(ix + 1, iy), sx);
        let bottom = lerp(self.corner(ix, iy + 1), self.corner(ix + 1, iy + 1), sx);
        lerp(top, bottom, sy)
    }

    /// Several layers on top of each other ("fractal" noise), 0.0 to 1.0. `scale`: how many tiles the biggest
    /// features are across.
    pub fn fractal(&self, x: f32, y: f32, scale: f32, octaves: u32) -> f32 {
        let (mut sum, mut weight, mut amplitude, mut frequency) = (0.0, 0.0, 1.0, 1.0 / scale);
        for octave in 0..octaves {
            // Every layer shifted a bit, so the grid corners of the layers don't lie on top of each other.
            let shift = octave as f32 * 17.31;
            sum += amplitude * self.layer(x * frequency + shift, y * frequency - shift);
            weight += amplitude;
            amplitude *= 0.5;
            frequency *= 2.0;
        }
        sum / weight
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same seed, same value; neighbouring points similar; the values spread over a good part of 0..1.
    #[test]
    fn noise_is_repeatable_and_smooth() {
        let a = Noise::new(7);
        assert_eq!(a.fractal(12.3, 45.6, 20.0, 4), Noise::new(7).fractal(12.3, 45.6, 20.0, 4));
        assert_ne!(a.fractal(12.3, 45.6, 20.0, 4), Noise::new(8).fractal(12.3, 45.6, 20.0, 4));
        let (here, next) = (a.fractal(30.0, 30.0, 20.0, 4), a.fractal(30.5, 30.0, 20.0, 4));
        assert!((here - next).abs() < 0.1, "not smooth: {here} {next}");
        let values: Vec<f32> = (0..100).map(|i| a.fractal(i as f32 * 7.0, i as f32 * 3.0, 20.0, 4)).collect();
        let (lo, hi) = values.iter().fold((1.0f32, 0.0f32), |(lo, hi), &v| (lo.min(v), hi.max(v)));
        assert!(lo < 0.35 && hi > 0.65, "too flat: {lo} … {hi}");
    }
}
