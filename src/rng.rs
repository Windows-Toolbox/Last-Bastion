//! Our own random number generator for the simulation (DESIGN.md 12.5, 12.8).
//! Same seed → the same numbers in the same order, on every PC.

/// SplitMix64: a tiny, fast generator, good enough for a game.
#[derive(Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng { state: seed }
    }

    /// The next random number, anywhere between 0 and 2^64 - 1.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Whole number from 0 to max - 1.
    pub fn below(&mut self, max: u32) -> u32 {
        (self.next_u64() % max as u64) as u32
    }

    /// Decimal number from 0.0 to just below 1.0.
    pub fn unit_f32(&mut self) -> f32 {
        // The top 24 bits fit exactly into the mantissa of an f32.
        (self.next_u64() >> 40) as f32 / 16_777_216.0
    }
}
