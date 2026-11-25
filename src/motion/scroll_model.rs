use crate::rng::Rng;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollEvent {
    pub delta_time: f64,
    pub delta_y: f64,
}

const BASE_DT: f64 = 8.2;
const BASE_DT_LONG: f64 = 10.8;
const LONG_DT_START: f64 = 2500.0;
const LONG_DT_FULL: f64 = 4500.0;
const DT_JITTER: f64 = 1.6;
const MIN_DT: f64 = 5.0;
const MAX_DT: f64 = 22.0;

const FLICK_PROBABILITY: f64 = 0.4;
const FLICK_COUNT_BASE: f64 = 14.0;
const FLICK_COUNT_PER_PX: f64 = 0.012;
const DRAG_COUNT_A: f64 = 0.912;
const DRAG_COUNT_K: f64 = 0.570;
const MIN_EVENTS: usize = 6;
const MAX_REALISTIC_EVENTS: usize = 200;

const PEAK_PHASE: f64 = 0.28;
const PLATEAU: f64 = 0.22;
const START_VEL: f64 = 0.10;
const DECAY_EXP: f64 = 2.6;

const MULTIFLICK_THRESHOLD: f64 = 1800.0;
const FLICK_PX_EACH: f64 = 1300.0;
const MAX_FLICKS: usize = 5;
const FLICK_LULL_DT: f64 = 34.0;
const FLICK_LULL_JITTER: f64 = 14.0;
const FLICK_END_VEL: f64 = 0.12;

const CORRECTION_PROBABILITY: f64 = 0.5;
const CORRECTION_FRACTION: f64 = 0.06;
const CORRECTION_REVERSE_CHANCE: f64 = 0.4;

pub const DEFAULT_SPEED_IRREGULARITY: f64 = 0.08;

fn estimate_target_events(distance_abs: f64, is_flick: bool) -> usize {
    let raw = if is_flick {
        FLICK_COUNT_BASE + FLICK_COUNT_PER_PX * distance_abs
    } else {
        DRAG_COUNT_A * distance_abs.max(1.0).powf(DRAG_COUNT_K)
    };
    (raw.round() as usize).clamp(MIN_EVENTS, MAX_REALISTIC_EVENTS)
}

fn base_dt_for(distance_abs: f64, rng: &Rng) -> f64 {
    let t = ((distance_abs - LONG_DT_START) / (LONG_DT_FULL - LONG_DT_START)).clamp(0.0, 1.0);
    let base = BASE_DT + (BASE_DT_LONG - BASE_DT) * t;
    let jittered = base + (rng.next() - 0.5) * 2.0 * DT_JITTER;
    jittered.clamp(MIN_DT, MAX_DT)
}

fn velocity_at(p: f64, tail_level: f64) -> f64 {
    if p <= PEAK_PHASE {
        let ramp_progress = p / PEAK_PHASE;
        START_VEL + (1.0 - START_VEL) * ramp_progress.powf(1.4)
    } else {
        let decay_progress = (p - PEAK_PHASE) / (1.0 - PEAK_PHASE);
        tail_level + (1.0 - tail_level) * (1.0 - decay_progress).powf(DECAY_EXP)
    }
}

fn build_burst(
    dist: f64,
    events: usize,
    rng: &Rng,
    single: bool,
    speed_irregularity: f64,
) -> (Vec<f64>, Vec<f64>) {
    let sign_d = dist.signum();
    let abs_d = dist.abs();
    let tail = if single { PLATEAU } else { FLICK_END_VEL };
    let mut curve = Vec::with_capacity(events);
    for i in 0..events {
        let p = if events > 1 {
            (i as f64) / ((events - 1) as f64)
        } else {
            0.0
        };
        let v = velocity_at(p, tail);
        let noise = 1.0 + (rng.next() - 0.5) * 2.0 * speed_irregularity;
        curve.push((v * noise).max(0.02));
    }
    let sum: f64 = curve.iter().sum();
    let dy = curve
        .into_iter()
        .map(|v| (v / sum) * abs_d * sign_d)
        .collect();
    let dt = (0..events).map(|_| base_dt_for(abs_d, rng)).collect();
    (dy, dt)
}

fn generate_multi_flick(
    target_d: f64,
    rng: &Rng,
    speed_irregularity: f64,
) -> (Vec<f64>, Vec<f64>) {
    let sign_d = target_d.signum();
    let abs_d = target_d.abs();
    let has_correction = rng.next() < CORRECTION_PROBABILITY;
    let reverse = has_correction && rng.next() < CORRECTION_REVERSE_CHANCE;
    let correction_abs = if has_correction {
        (abs_d * CORRECTION_FRACTION * (0.5 + rng.next())).round()
    } else {
        0.0
    };
    let main_abs = if reverse {
        abs_d + correction_abs
    } else {
        abs_d - correction_abs
    };
    let num_flicks = ((main_abs / FLICK_PX_EACH).round() as usize).clamp(2, MAX_FLICKS);
    let weights: Vec<f64> = (0..num_flicks)
        .map(|_| 0.75 + rng.next() * 0.5)
        .collect();
    let w_sum: f64 = weights.iter().sum();
    let flick_dists: Vec<f64> = weights
        .into_iter()
        .map(|w| (w / w_sum) * main_abs * sign_d)
        .collect();

    let mut dy = Vec::new();
    let mut dt = Vec::new();
    for (f, &fd) in flick_dists.iter().enumerate() {
        let flick_ev = estimate_target_events(fd.abs(), true);
        let drag_ev = estimate_target_events(fd.abs(), false);
        let f_events = (((flick_ev + drag_ev) as f64) / 2.0).round() as usize;
        let (b_dy, b_dt) = build_burst(fd, f_events, rng, false, speed_irregularity);
        dy.extend(b_dy);
        dt.extend(b_dt);
        if f + 1 < num_flicks {
            let glide_events = 1 + (rng.next() * 2.0).floor() as usize;
            for _ in 0..glide_events {
                dy.push(sign_d * (1.0 + rng.next() * 2.0));
                dt.push(FLICK_LULL_DT + (rng.next() - 0.5) * 2.0 * FLICK_LULL_JITTER);
            }
        }
    }
    if has_correction && correction_abs >= 2.0 {
        dt.push(FLICK_LULL_DT + (rng.next() - 0.5) * 2.0 * FLICK_LULL_JITTER);
        let corr_sign = if reverse { -sign_d } else { sign_d };
        dy.push(corr_sign * (1.0 + rng.next() * 2.0));
        let corr_events = estimate_target_events(correction_abs, true).clamp(4, 14);
        let (c_dy, c_dt) = build_burst(
            corr_sign * correction_abs,
            corr_events,
            rng,
            true,
            speed_irregularity,
        );
        dy.extend(c_dy);
        dt.extend(c_dt);
    }
    (dy, dt)
}

fn absorb_into_tail(path: &mut [ScrollEvent], remainder: f64, max_events: usize) -> f64 {
    let step = remainder.signum();
    let mut left = remainder.abs();
    while left > 0.0 {
        let mut applied = false;
        let len = path.len();
        for i in 0..max_events.min(len) {
            if left <= 0.0 {
                break;
            }
            let ev = &mut path[len - 1 - i];
            let next = ev.delta_y + step;
            if next != 0.0 && next.signum() == ev.delta_y.signum() {
                ev.delta_y = next;
                left -= 1.0;
                applied = true;
            }
        }
        if !applied {
            break;
        }
    }
    left * step
}

/// Trackpad bimodal scroll path generator.
pub fn generate_scroll_path(
    desired_d: f64,
    rng: &Rng,
    force_flick: Option<bool>,
    speed_irregularity: f64,
) -> Vec<ScrollEvent> {
    let target = desired_d.round();
    if target == 0.0 {
        return Vec::new();
    }
    let is_flick = force_flick.unwrap_or_else(|| rng.next() < FLICK_PROBABILITY);
    let abs_d = desired_d.abs();
    let (tmpl_dy, tmpl_dt) = if !is_flick && abs_d >= MULTIFLICK_THRESHOLD {
        generate_multi_flick(desired_d, rng, speed_irregularity)
    } else {
        let events = estimate_target_events(abs_d, is_flick);
        build_burst(desired_d, events, rng, true, speed_irregularity)
    };

    let mut gen_path: Vec<ScrollEvent> = Vec::with_capacity(tmpl_dy.len());
    let mut pending_dt = 0.0;
    for (raw_dy, raw_dt) in tmpl_dy.into_iter().zip(tmpl_dt) {
        let rounded_dt = MIN_DT.max(raw_dt.round());
        let rounded_dy = raw_dy.round();
        if rounded_dy != 0.0 {
            gen_path.push(ScrollEvent {
                delta_time: rounded_dt + pending_dt,
                delta_y: rounded_dy,
            });
            pending_dt = 0.0;
        } else {
            pending_dt += rounded_dt;
        }
    }
    if pending_dt > 0.0 {
        if let Some(last) = gen_path.last_mut() {
            last.delta_time += pending_dt;
        }
    }
    if gen_path.is_empty() {
        return vec![ScrollEvent {
            delta_time: MIN_DT.max(pending_dt),
            delta_y: target,
        }];
    }

    let current_sum: f64 = gen_path.iter().map(|p| p.delta_y).sum();
    let mut remainder = target - current_sum;
    if remainder != 0.0 {
        let remainder_pct = remainder.abs() / target.abs();
        if remainder.abs() <= 3.0 {
            remainder = absorb_into_tail(&mut gen_path, remainder, 1);
        } else if remainder_pct <= 0.08 {
            let max_ev = gen_path.len().min(5);
            remainder = absorb_into_tail(&mut gen_path, remainder, max_ev);
        }
        if remainder != 0.0 {
            if remainder.signum() == target.signum() {
                let num_rem = ((remainder.abs() / 25.0).ceil() as usize).clamp(1, 3);
                let per_event = (remainder / (num_rem as f64)).trunc();
                let mut left = remainder;
                for i in 0..num_rem {
                    let is_last = i + 1 == num_rem;
                    let delta_y = if is_last { left } else { per_event };
                    if delta_y == 0.0 {
                        continue;
                    }
                    gen_path.push(ScrollEvent {
                        delta_time: base_dt_for(desired_d.abs(), rng).round(),
                        delta_y,
                    });
                    left -= delta_y;
                }
                remainder = left;
            } else {
                let len = gen_path.len();
                remainder = absorb_into_tail(&mut gen_path, remainder, len);
            }
        }
        if remainder != 0.0 {
            let total_dt: f64 = gen_path.iter().map(|p| p.delta_time).sum();
            gen_path.clear();
            gen_path.push(ScrollEvent {
                delta_time: total_dt,
                delta_y: target,
            });
        }
    }

    gen_path
}
