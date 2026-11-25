//! Seeded PRNG (`Mulberry32`) matching `myuri`'s TypeScript & Python streams.

use std::sync::{Arc, Mutex};

/// Mulberry32 32-bit PRNG. Same seed produces the exact same sequence across Rust, TS, and Python.
#[derive(Debug, Clone)]
pub struct SeededRng {
    state: u32,
}

impl SeededRng {
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6d2b_79f5);
        let mut t = self.state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

/// Thread-safe shared handle to an RNG stream yielding floats in `[0, 1)`.
#[derive(Clone)]
pub struct Rng(Arc<Mutex<Box<dyn FnMut() -> f64 + Send>>>);

impl std::fmt::Debug for Rng {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Rng").finish()
    }
}

impl Rng {
    pub fn from_seed(seed: u32) -> Self {
        let mut inner = SeededRng::new(seed);
        Self(Arc::new(Mutex::new(Box::new(move || inner.next()))))
    }

    pub fn from_fn<F>(f: F) -> Self
    where
        F: FnMut() -> f64 + Send + 'static,
    {
        Self(Arc::new(Mutex::new(Box::new(f))))
    }

    pub fn default_entropy() -> Self {
        Self::from_seed(generate_random_seed())
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&self) -> f64 {
        let mut guard = self.0.lock().expect("rng lock poisoned");
        (guard)()
    }
}

/// Generates a fresh random 32-bit seed from OS entropy.
pub fn generate_random_seed() -> u32 {
    let mut buf = [0u8; 4];
    getrandom::fill(&mut buf).expect("OS entropy unavailable");
    u32::from_le_bytes(buf)
}

/// Creates a shared Mulberry32 `Rng` from `seed`.
pub fn make_rng(seed: u32) -> Rng {
    Rng::from_seed(seed)
}

/// Box-Muller Gaussian sample matching `rng.ts` (`u1` guarded with `Number.MIN_VALUE`).
pub fn gaussian(rng: &Rng, mean: f64, std: f64) -> f64 {
    let raw_u1 = rng.next();
    let u1 = if raw_u1 == 0.0 {
        f64::from_bits(1)
    } else {
        raw_u1
    };
    let u2 = rng.next();
    let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
    mean + z * std
}
