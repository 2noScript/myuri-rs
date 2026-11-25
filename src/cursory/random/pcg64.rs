//! PCG64 (XSL-RR 128/64), ported from `numpy/random/src/pcg64/pcg64.h`.

use super::seed_sequence::{generate_state64, int_to_u32_array, mix_entropy};

const MULTIPLIER: u128 = (2_549_297_995_355_413_924_u128 << 64) | 4_865_540_595_714_422_341_u128;
const DOUBLE_SCALE: f64 = 1.0 / 9_007_199_254_740_992.0;

#[derive(Debug, Clone)]
pub struct Pcg64 {
    state: u128,
    increment: u128,
    has_spare_u32: bool,
    spare_u32: u32,
}

impl Pcg64 {
    pub fn new(initial_state: u128, initial_sequence: u128) -> Self {
        let increment = (initial_sequence << 1) | 1;
        let mut rng = Self {
            state: 0,
            increment,
            has_spare_u32: false,
            spare_u32: 0,
        };
        rng.step();
        rng.state = rng.state.wrapping_add(initial_state);
        rng.step();
        rng
    }

    #[inline]
    fn step(&mut self) {
        self.state = self.state.wrapping_mul(MULTIPLIER).wrapping_add(self.increment);
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.step();
        let state = self.state;
        let value = ((state >> 64) ^ state) as u64;
        let rotation = (state >> 122) as u32 & 63;
        value.rotate_right(rotation)
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        if self.has_spare_u32 {
            self.has_spare_u32 = false;
            return self.spare_u32;
        }
        let next = self.next_u64();
        self.has_spare_u32 = true;
        self.spare_u32 = (next >> 32) as u32;
        (next & 0xffff_ffff) as u32
    }

    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64) * DOUBLE_SCALE
    }
}

fn random_entropy() -> u128 {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("OS entropy unavailable");
    u128::from_be_bytes(bytes)
}

/// Builds the bit generator `np.random.default_rng(seed)` produces.
pub fn create_pcg64(seed: Option<u128>) -> Pcg64 {
    let seed_val = seed.unwrap_or_else(random_entropy);
    let entropy = int_to_u32_array(seed_val);
    let state = generate_state64(&mix_entropy(&entropy), 4);
    let initial_state = (u128::from(state[0]) << 64) | u128::from(state[1]);
    let initial_sequence = (u128::from(state[2]) << 64) | u128::from(state[3]);
    Pcg64::new(initial_state, initial_sequence)
}
