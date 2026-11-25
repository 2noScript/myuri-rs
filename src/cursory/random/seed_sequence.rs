//! numpy's `SeedSequence`, ported from `numpy/random/bit_generator.pyx` (BSD-3-Clause).

const INIT_A: u32 = 0x43b0_d7e5;
const MULT_A: u32 = 0x931e_8875;
const INIT_B: u32 = 0x8b51_f9dd;
const MULT_B: u32 = 0x58f3_8ded;
const MIX_MULT_L: u32 = 0xca01_f9dd;
const MIX_MULT_R: u32 = 0x4973_f715;
const XSHIFT: u32 = 16;

/// numpy's `DEFAULT_POOL_SIZE`.
pub const POOL_SIZE: usize = 4;

/// Splits a non-negative integer into `u32` words, least-significant word first.
pub fn int_to_u32_array(value: u128) -> Vec<u32> {
    if value == 0 {
        return vec![0];
    }
    let mut words = Vec::new();
    let mut remaining = value;
    while remaining > 0 {
        words.push((remaining & 0xffff_ffff) as u32);
        remaining >>= 32;
    }
    words
}

struct Hasher {
    constant: u32,
}

impl Hasher {
    fn new(initial: u32) -> Self {
        Self { constant: initial }
    }

    fn mix(&mut self, value: u32) -> u32 {
        let mixed = value ^ self.constant;
        self.constant = self.constant.wrapping_mul(MULT_A);
        let mixed = mixed.wrapping_mul(self.constant);
        mixed ^ (mixed >> XSHIFT)
    }
}

fn mix(x: u32, y: u32) -> u32 {
    let result = MIX_MULT_L.wrapping_mul(x).wrapping_sub(MIX_MULT_R.wrapping_mul(y));
    result ^ (result >> XSHIFT)
}

/// numpy's `SeedSequence.mix_entropy`.
pub fn mix_entropy(entropy: &[u32]) -> [u32; POOL_SIZE] {
    let mut pool = [0u32; POOL_SIZE];
    let mut hasher = Hasher::new(INIT_A);
    for (i, slot) in pool.iter_mut().enumerate() {
        *slot = hasher.mix(entropy.get(i).copied().unwrap_or(0));
    }
    for source in 0..POOL_SIZE {
        for destination in 0..POOL_SIZE {
            if source != destination {
                let mixed = hasher.mix(pool[source]);
                pool[destination] = mix(pool[destination], mixed);
            }
        }
    }
    for &word in entropy.iter().skip(POOL_SIZE) {
        for slot in &mut pool {
            let mixed = hasher.mix(word);
            *slot = mix(*slot, mixed);
        }
    }
    pool
}

/// numpy's `SeedSequence.generate_state(word_count, uint64)`.
pub fn generate_state64(pool: &[u32; POOL_SIZE], word_count: usize) -> Vec<u64> {
    let mut words = vec![0u32; word_count * 2];
    let mut constant = INIT_B;
    for (i, slot) in words.iter_mut().enumerate() {
        let mut value = pool[i % pool.len()] ^ constant;
        constant = constant.wrapping_mul(MULT_B);
        value = value.wrapping_mul(constant);
        *slot = value ^ (value >> XSHIFT);
    }
    let mut state = Vec::with_capacity(word_count);
    for i in 0..word_count {
        let lo = u64::from(words[2 * i]);
        let hi = u64::from(words[2 * i + 1]);
        state.push(lo | (hi << 32));
    }
    state
}
