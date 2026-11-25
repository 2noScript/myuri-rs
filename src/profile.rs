//! Seed -> behavioral profile mapping, identical across Rust, TypeScript, and Python.

use crate::plan::wheel_style_for;
use crate::rng::{gaussian, make_rng};
use crate::settings::{resolve_settings, MyuriSettings, SettingsOverride};
use crate::target_os::Os;

/// Draws a reproducible per-person behavioral profile from `seed` around `settings`.
pub fn generate_profile(
    seed: u32,
    settings: Option<&SettingsOverride>,
    os: Option<Os>,
) -> MyuriSettings {
    let mut p = resolve_settings(settings);
    p.scroll.wheel.style = wheel_style_for(os, settings);
    let rng = make_rng(seed);
    let v = p.variation;
    let vary = |x: f64| -> f64 {
        let scale = gaussian(&rng, 1.0, v / 2.0).clamp(1.0 - v, 1.0 + v);
        x * scale
    };
    p.mouse.speed_factor = vary(p.mouse.speed_factor);
    p.overshoot.radius = vary(p.overshoot.radius);
    p.overshoot.threshold = vary(p.overshoot.threshold);
    p.overshoot.correction_speed_factor = vary(p.overshoot.correction_speed_factor);
    p.overshoot.dwell.min_ms = vary(p.overshoot.dwell.min_ms);
    p.overshoot.dwell.max_ms = vary(p.overshoot.dwell.max_ms);
    p.scroll.speed_factor = vary(p.scroll.speed_factor);
    p.scroll.finish_position = vary(p.scroll.finish_position).min(1.0);
    p.scroll.realism.speed_irregularity = vary(p.scroll.realism.speed_irregularity);
    p.typing.wpm = vary(p.typing.wpm);
    p.typing.mistake_multiplier = vary(p.typing.mistake_multiplier);
    p.typing.pauses.rate = vary(p.typing.pauses.rate);
    p.typing.pauses.length = vary(p.typing.pauses.length);
    p
}
