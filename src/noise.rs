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

/// TAB's noise (`DXVision\Noise.cs`, the map generator's `FillWithNoyse`): 3D simplex noise as Stefan Gustavson wrote
/// it, with its fixed table – 0.0 to 1.0, mostly around 0.5. TAB's map generator cuts its layers at thresholds made for
/// exactly this spread of values (`mapgen/tab.rs`), so it's this one and not our `Noise`. `z` picks a slice: the same
/// `z` with other `x`, `y` is the same landscape moved. Only `+ - * /` and `floor` – the same on every PC.
pub fn simplex(x: f64, y: f64, z: f64) -> f32 {
    const F3: f64 = 1.0 / 3.0;
    const G3: f64 = 1.0 / 6.0;
    // Which cube of the skewed grid the point is in …
    let s = (x + y + z) * F3;
    let (i, j, k) = ((x + s).floor() as i32, (y + s).floor() as i32, (z + s).floor() as i32);
    let t = (i + j + k) as f64 * G3;
    let (x0, y0, z0) = (x - (i as f64 - t), y - (j as f64 - t), z - (k as f64 - t));
    // … and which of the six pieces (tetrahedra) of that cube: its second and third corner.
    let ((i1, j1, k1), (i2, j2, k2)) = if x0 >= y0 {
        if y0 >= z0 {
            ((1, 0, 0), (1, 1, 0))
        } else if x0 >= z0 {
            ((1, 0, 0), (1, 0, 1))
        } else {
            ((0, 0, 1), (1, 0, 1))
        }
    } else if y0 < z0 {
        ((0, 0, 1), (0, 1, 1))
    } else if x0 < z0 {
        ((0, 1, 0), (0, 1, 1))
    } else {
        ((0, 1, 0), (1, 1, 0))
    };
    let corners = [
        (0, 0, 0, x0, y0, z0),
        (i1, j1, k1, x0 - i1 as f64 + G3, y0 - j1 as f64 + G3, z0 - k1 as f64 + G3),
        (i2, j2, k2, x0 - i2 as f64 + 2.0 * G3, y0 - j2 as f64 + 2.0 * G3, z0 - k2 as f64 + 2.0 * G3),
        (1, 1, 1, x0 - 1.0 + 3.0 * G3, y0 - 1.0 + 3.0 * G3, z0 - 1.0 + 3.0 * G3),
    ];
    let (ii, jj, kk) = ((i & 255) as usize, (j & 255) as usize, (k & 255) as usize);
    let perm = |n: usize| PERM[n & 255] as usize;
    // Every corner adds its slope (one of 12 directions, by the table), fading out within its reach.
    let mut sum = 0.0;
    for (di, dj, dk, cx, cy, cz) in corners {
        let fade = 0.6 - cx * cx - cy * cy - cz * cz;
        if fade < 0.0 {
            continue;
        }
        let g = GRAD3[perm(ii + di + perm(jj + dj + perm(kk + dk))) % 12];
        let fade = fade * fade;
        sum += fade * fade * (g[0] * cx + g[1] * cy + g[2] * cz);
    }
    (32.0 * sum + 1.0) as f32 * 0.5
}

/// TAB's noise in `octaves` layers on top of each other, each twice as fine and half as strong (TAB's
/// `GetOctaveNoise`); 0 octaves: one plain layer.
pub fn simplex_octaves(x: f64, y: f64, z: f64, octaves: u32) -> f32 {
    if octaves == 0 {
        return simplex(x, y, z);
    }
    let (mut sum, mut weight, mut strength, mut fineness) = (0.0f32, 0.0f32, 1.0f32, 1.0f32);
    for _ in 0..octaves {
        sum += simplex(x * fineness as f64, y * fineness as f64, z) * strength;
        weight += strength;
        strength *= 0.5;
        fineness *= 2.0;
    }
    sum / weight
}

/// The 12 slopes of 3D simplex noise: the middles of a cube's edges.
const GRAD3: [[f64; 3]; 12] = [
    [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0], [1.0, -1.0, 0.0], [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0], [-1.0, 0.0, 1.0], [1.0, 0.0, -1.0], [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0], [0.0, -1.0, 1.0], [0.0, 1.0, -1.0], [0.0, -1.0, -1.0],
];

/// Ken Perlin's table of the numbers 0–255 shuffled – the one TAB uses, so its noise comes out the same here.
const PERM: [u8; 256] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30, 69, 142, 8, 99, 37, 240,
    21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32, 57, 177, 33, 88,
    237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175, 74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83,
    111, 229, 122, 60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25, 63, 161, 1, 216,
    80, 73, 209, 76, 132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173, 186,
    3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212, 207, 206, 59, 227, 47, 16, 58,
    17, 182, 189, 28, 42, 223, 183, 170, 213, 119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9,
    129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104, 218, 246, 97, 228, 251, 34, 242, 193,
    238, 210, 144, 12, 191, 179, 162, 241, 81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157,
    184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141, 128,
    195, 78, 66, 215, 61, 156, 180,
];

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

    /// TAB's simplex noise: in 0..1, the same for the same point, smooth, spread around 0.5.
    #[test]
    fn simplex_like_tab() {
        assert_eq!(simplex(1.3, 2.7, 0.4), simplex(1.3, 2.7, 0.4));
        let values: Vec<f32> = (0..2000).map(|i| simplex_octaves(i as f64 * 0.37, i as f64 * 0.11, 5.0, 3)).collect();
        assert!(values.iter().all(|v| (0.0..=1.0).contains(v)));
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        assert!((mean - 0.5).abs() < 0.05, "mean {mean}");
        assert!((simplex(10.0, 10.0, 1.0) - simplex(10.01, 10.0, 1.0)).abs() < 0.02, "not smooth");
    }
}
