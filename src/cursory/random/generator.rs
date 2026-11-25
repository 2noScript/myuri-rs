//! `numpy.random.Generator` subset used by Cursory.

use crate::cursory::numeric::search_sorted_right;

use super::pcg64::{create_pcg64, Pcg64};
use super::ziggurat::{FI_DOUBLE, KI_DOUBLE, WI_DOUBLE, ZIGGURAT_NOR_INV_R, ZIGGURAT_NOR_R};

pub struct Generator {
    bit_generator: Pcg64,
}

impl Generator {
    pub fn new(bit_generator: Pcg64) -> Self {
        Self { bit_generator }
    }

    pub fn random(&mut self) -> f64 {
        self.bit_generator.next_f64()
    }

    pub fn uniform(&mut self, low: f64, high: f64) -> f64 {
        let range = high - low;
        assert!(range.is_finite(), "Range exceeds valid bounds");
        low + range * self.bit_generator.next_f64()
    }

    pub fn uniform_array(&mut self, low: f64, high: f64, size: usize) -> Vec<f64> {
        let range = high - low;
        assert!(range.is_finite(), "Range exceeds valid bounds");
        (0..size)
            .map(|_| low + range * self.bit_generator.next_f64())
            .collect()
    }

    pub fn normal(&mut self, loc: f64, scale: f64) -> f64 {
        assert!(scale >= 0.0, "scale < 0");
        loc + scale * self.standard_normal()
    }

    pub fn standard_normal_array(&mut self, size: usize) -> Vec<f64> {
        (0..size).map(|_| self.standard_normal()).collect()
    }

    pub fn standard_normal(&mut self) -> f64 {
        loop {
            let mut draw = self.bit_generator.next_u64();
            let index = (draw & 0xff) as usize;
            draw >>= 8;
            let sign = draw & 0x1;
            let magnitude = (draw >> 1) & 0x000f_ffff_ffff_ffff;
            let mut x = (magnitude as f64) * WI_DOUBLE[index];
            if sign == 1 {
                x = -x;
            }
            if magnitude < KI_DOUBLE[index] {
                return x;
            }
            if index == 0 {
                loop {
                    let xx = -ZIGGURAT_NOR_INV_R * (-self.bit_generator.next_f64()).ln_1p();
                    let yy = -(-self.bit_generator.next_f64()).ln_1p();
                    if yy + yy > xx * xx {
                        return if (magnitude >> 8) & 0x1 != 0 {
                            -(ZIGGURAT_NOR_R + xx)
                        } else {
                            ZIGGURAT_NOR_R + xx
                        };
                    }
                }
            }
            let wedge = (FI_DOUBLE[index - 1] - FI_DOUBLE[index]) * self.bit_generator.next_f64()
                + FI_DOUBLE[index];
            if wedge < (-0.5 * x * x).exp() {
                return x;
            }
        }
    }

    pub fn integers(&mut self, high: usize) -> usize {
        assert!(high > 0, "high must be a positive integer");
        let range = high - 1;
        if range == 0 {
            return 0;
        }
        assert!(
            range <= u32::MAX as usize,
            "ranges wider than 2**32 are not supported"
        );
        if range == u32::MAX as usize {
            return self.bit_generator.next_u32() as usize;
        }
        self.bounded_lemire32(range as u32) as usize
    }

    fn bounded_lemire32(&mut self, range: u32) -> u32 {
        let range_exclusive = u64::from(range) + 1;
        let mut scaled = u64::from(self.bit_generator.next_u32()) * range_exclusive;
        let mut leftover = scaled & 0xffff_ffff;
        if leftover < range_exclusive {
            let threshold = u64::from(u32::MAX - range) % range_exclusive;
            while leftover < threshold {
                scaled = u64::from(self.bit_generator.next_u32()) * range_exclusive;
                leftover = scaled & 0xffff_ffff;
            }
        }
        (scaled >> 32) as u32
    }

    pub fn choice(&mut self, weights: &[f64]) -> usize {
        let mut cumulative = Vec::with_capacity(weights.len());
        let mut running = 0.0;
        for &w in weights {
            running += w;
            cumulative.push(running);
        }
        let total = *cumulative.last().expect("weights must be non-empty");
        for slot in &mut cumulative {
            *slot /= total;
        }
        search_sorted_right(&cumulative, self.bit_generator.next_f64())
    }
}

/// Equivalent of `np.random.default_rng(seed)`.
pub fn default_generator(seed: Option<u128>) -> Generator {
    Generator::new(create_pcg64(seed))
}
