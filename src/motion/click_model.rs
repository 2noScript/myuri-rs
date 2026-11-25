use crate::rng::{gaussian, Rng};

const HOLD_MU: f64 = 4.533;
const HOLD_SD: f64 = 0.21;
const HOLD_MIN_MS: f64 = 35.0;
const HOLD_MAX_MS: f64 = 400.0;
const LATER_HOLD_MU: f64 = 4.419;
const LATER_HOLD_SD: f64 = 0.227;
const DWELL_MU: f64 = 3.717;
const DWELL_SD: f64 = 1.341;
const DWELL_MIN_MS: f64 = 4.0;
const DWELL_MAX_MS: f64 = 600.0;
const DRAG_START_MU: f64 = 3.58;
const DRAG_START_SD: f64 = 1.004;
const DRAG_START_MIN_MS: f64 = 4.0;
const DRAG_START_MAX_MS: f64 = 500.0;
const DRAG_END_MU: f64 = 3.182;
const DRAG_END_SD: f64 = 2.104;
const DRAG_END_MIN_MS: f64 = 0.0;
const DRAG_END_MAX_MS: f64 = 600.0;
const DRAG_CARRY_MU: f64 = -0.288;
const DRAG_CARRY_SD: f64 = 0.25;
const MULTI_GAP_MU: f64 = 4.353;
const MULTI_GAP_SD: f64 = 0.113;
const MULTI_GAP_MIN_MS: f64 = 30.0;
const MULTI_GAP_MAX_MS: f64 = 250.0;

#[inline]
fn lognormal(rng: &Rng, mu: f64, sd: f64, lo: f64, hi: f64) -> f64 {
    gaussian(rng, mu, sd).exp().clamp(lo, hi).round()
}

pub fn click_hold_ms(rng: &Rng, later: bool) -> f64 {
    if later {
        lognormal(rng, LATER_HOLD_MU, LATER_HOLD_SD, HOLD_MIN_MS, HOLD_MAX_MS)
    } else {
        lognormal(rng, HOLD_MU, HOLD_SD, HOLD_MIN_MS, HOLD_MAX_MS)
    }
}

pub fn pre_click_dwell_ms(rng: &Rng) -> f64 {
    lognormal(rng, DWELL_MU, DWELL_SD, DWELL_MIN_MS, DWELL_MAX_MS)
}

pub fn multi_click_gap_ms(rng: &Rng) -> f64 {
    lognormal(
        rng,
        MULTI_GAP_MU,
        MULTI_GAP_SD,
        MULTI_GAP_MIN_MS,
        MULTI_GAP_MAX_MS,
    )
}

pub fn drag_start_ms(rng: &Rng) -> f64 {
    lognormal(
        rng,
        DRAG_START_MU,
        DRAG_START_SD,
        DRAG_START_MIN_MS,
        DRAG_START_MAX_MS,
    )
}

pub fn drag_end_ms(rng: &Rng) -> f64 {
    lognormal(
        rng,
        DRAG_END_MU,
        DRAG_END_SD,
        DRAG_END_MIN_MS,
        DRAG_END_MAX_MS,
    )
}

pub fn drag_carry_speed(rng: &Rng) -> f64 {
    gaussian(rng, DRAG_CARRY_MU, DRAG_CARRY_SD)
        .exp()
        .clamp(0.4, 1.2)
}
