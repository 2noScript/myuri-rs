use crate::rng::{gaussian, Rng};
use crate::settings::WheelStyle;

use super::scroll_model::ScrollEvent;

const STROKE_MU: f64 = 5.215;
const STROKE_SD: f64 = 0.552;
const STROKE_MIN_MS: f64 = 60.0;
const STROKE_MAX_MS: f64 = 450.0;
const REGRIP_MU: f64 = 4.952;
const REGRIP_SD: f64 = 0.563;
const REGRIP_MIN_MS: f64 = 50.0;
const REGRIP_MAX_MS: f64 = 600.0;
const LONE_NOTCH_PROBABILITY: f64 = 0.33;
const NOTCHES_MU: f64 = 2.079;
const NOTCHES_SD: f64 = 0.342;
const FLICK_MU: f64 = 4.828;
const FLICK_SD: f64 = 0.352;
const FLICK_REGRIP_MU: f64 = 4.828;
const FLICK_REGRIP_SD: f64 = 0.727;
const FRAME_MS: f64 = 1000.0 / 60.0;
const NOTCH_MAX_MS: f64 = 2.0 * FRAME_MS;
const NUDGE_MAX_NOTCHES: usize = 7;
const NUDGE_MU: f64 = std::f64::consts::LN_2;
const NUDGE_SD: f64 = 0.54;
const NUDGE_REGRIP_MU: f64 = 4.258;
const NUDGE_REGRIP_SD: f64 = 0.446;
const MAC_FRAME_MS: f64 = 20.0;
const MAC_DROPPED_FRAME_PROBABILITY: f64 = 0.1;
const MAC_PEAK_MEAN: f64 = 10.0;
const MAC_PEAK_SD: f64 = 1.6;
const MAC_PEAK_MIN: f64 = 6.0;
const MAC_PEAK_MAX: f64 = 13.0;
const MAC_CARRY_MS: f64 = 150.0;
const MAC_CARRY_DROP_PROBABILITY: f64 = 0.45;

#[derive(Debug, Clone, Copy)]
pub struct WheelModelOptions {
    pub style: WheelStyle,
    pub px_per_notch: f64,
    pub px_per_line: f64,
    pub stroke_scale: f64,
}

impl Default for WheelModelOptions {
    fn default() -> Self {
        Self {
            style: WheelStyle::Notched,
            px_per_notch: 100.0,
            px_per_line: 16.0,
            stroke_scale: 1.0,
        }
    }
}

#[inline]
fn lognormal(rng: &Rng, mu: f64, sd: f64) -> f64 {
    gaussian(rng, mu, sd).exp()
}

fn notched_path(
    target: f64,
    rng: &Rng,
    px_per_notch: f64,
    stroke_scale: f64,
) -> Vec<ScrollEvent> {
    let sign = target.signum();
    let mut left = ((target.abs() / px_per_notch).round() as usize).max(1);
    struct FrameGroup {
        frame: i64,
        notches: usize,
    }
    let mut frames: Vec<FrameGroup> = Vec::new();
    let mut start = 0.0_f64;
    let nudged = left <= NUDGE_MAX_NOTCHES;

    while left > 0 {
        let size = if nudged {
            if frames.is_empty() {
                1.0
            } else {
                lognormal(rng, NUDGE_MU, NUDGE_SD)
            }
        } else {
            lognormal(rng, NOTCHES_MU, NOTCHES_SD) * stroke_scale
        };
        let n = left.min((size.round() as usize).max(1));
        let duration = ((n as f64) * NOTCH_MAX_MS).min(
            lognormal(rng, FLICK_MU, FLICK_SD).clamp(STROKE_MIN_MS, STROKE_MAX_MS),
        );
        let regrip = if nudged {
            lognormal(rng, NUDGE_REGRIP_MU, NUDGE_REGRIP_SD).clamp(REGRIP_MIN_MS, REGRIP_MAX_MS)
        } else {
            lognormal(rng, FLICK_REGRIP_MU, FLICK_REGRIP_SD).clamp(REGRIP_MIN_MS, REGRIP_MAX_MS)
        };
        let mut last = start;
        for i in 0..n {
            last = start + duration * (1.0 - (1.0 - ((i as f64) + 0.5) / (n as f64)).sqrt());
            let frame = (last / FRAME_MS).round() as i64;
            if let Some(prev) = frames.last_mut().filter(|f| f.frame == frame) {
                prev.notches += 1;
            } else {
                frames.push(FrameGroup { frame, notches: 1 });
            }
        }
        left -= n;
        start = last + regrip;
    }

    let at = |i: usize| -> f64 {
        if i < frames.len() {
            ((frames[i].frame as f64) * FRAME_MS).round()
        } else {
            start.round()
        }
    };
    frames
        .iter()
        .enumerate()
        .map(|(i, group)| ScrollEvent {
            delta_time: at(i + 1) - at(i),
            delta_y: sign * (group.notches as f64) * px_per_notch,
        })
        .collect()
}

fn mac_path(target: f64, rng: &Rng, px_per_line: f64) -> Vec<ScrollEvent> {
    let sign = target.signum();
    let mut left = ((target.abs() / px_per_line).round() as usize).max(1);
    let mut events = Vec::new();
    let mut lines: usize = 0;

    while left > 0 {
        let frames = if rng.next() < LONE_NOTCH_PROBABILITY {
            1
        } else {
            let duration = lognormal(rng, STROKE_MU, STROKE_SD).clamp(STROKE_MIN_MS, STROKE_MAX_MS);
            ((duration / MAC_FRAME_MS).round() as usize).max(2)
        };
        lines = if lines > 0 {
            lines
                .saturating_sub(usize::from(rng.next() < MAC_CARRY_DROP_PROBABILITY))
                .max(1)
        } else {
            1
        };
        let peak = lines.max(
            gaussian(rng, MAC_PEAK_MEAN, MAC_PEAK_SD)
                .clamp(MAC_PEAK_MIN, MAC_PEAK_MAX)
                .round() as usize,
        );
        for f in 0..frames {
            if left == 0 {
                break;
            }
            if f > 0 {
                let step = if rng.next() < 0.2 {
                    5
                } else {
                    1 + (rng.next() * 3.0).floor() as usize
                };
                lines = peak.min(lines + step);
            }
            let sent = lines.min(left);
            left -= sent;
            let last = f + 1 == frames || left == 0;
            let gap = if last {
                lognormal(rng, REGRIP_MU, REGRIP_SD)
                    .clamp(REGRIP_MIN_MS, REGRIP_MAX_MS)
                    .round()
            } else if rng.next() < MAC_DROPPED_FRAME_PROBABILITY {
                (2.0 * MAC_FRAME_MS).round()
            } else {
                MAC_FRAME_MS.round()
            };
            events.push(ScrollEvent {
                delta_time: gap,
                delta_y: sign * (sent as f64) * px_per_line,
            });
            if last && gap > MAC_CARRY_MS {
                lines = 0;
            }
        }
    }
    events
}

/// Plans a physical mouse-wheel gesture covering approximately `desired_d` px.
pub fn generate_wheel_path(
    desired_d: f64,
    rng: &Rng,
    options: WheelModelOptions,
) -> Vec<ScrollEvent> {
    if desired_d.round() == 0.0 {
        return Vec::new();
    }
    match options.style {
        WheelStyle::Macos => mac_path(desired_d, rng, options.px_per_line),
        WheelStyle::Notched => {
            notched_path(desired_d, rng, options.px_per_notch, options.stroke_scale)
        }
    }
}
