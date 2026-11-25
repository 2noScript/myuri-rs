//! Pure planners: deterministic action timelines produced without a browser.

use crate::exceptions::Result;
use crate::geometry::{get_random_box_point, should_overshoot, trajectory};
use crate::math::{overshoot as overshoot_point, point_in_box, BoundingBox, TimedVector, Vector};
use crate::motion::click_model::{click_hold_ms, multi_click_gap_ms, pre_click_dwell_ms};
use crate::motion::scroll_model::generate_scroll_path;
use crate::motion::wheel_model::{generate_wheel_path, WheelModelOptions};
use crate::options::PathOptions;
use crate::rng::{gaussian, Rng};
use crate::settings::{
    resolve_settings, OvershootSettings, PauseSettings, ScrollDevice, SettingsOverride,
    WheelSettings, WheelStyle,
};
use crate::target_os::{primary_modifier, Os};
use crate::typing::config::DEFAULT_WPM;
use crate::typing::timing::{needs_shift, KeyActionType, KeystrokeTiming};
use crate::typing::typer::{draw_session_wpm, plan_typing};

pub const CLICK_RUN_MS: f64 = 500.0;
const MIN_TRACKPAD_DELTA: f64 = 8.0;
const SHIFT_WHEEL_LEAD: (f64, f64) = (7.023, 0.411);
const SHIFT_WHEEL_TRAIL: (f64, f64) = (7.501, 0.559);

#[derive(Debug, Clone)]
pub struct MoveSpec {
    pub move_speed: Option<f64>,
    pub overshoot: bool,
    pub settings: OvershootSettings,
}

/// Builds a human move trajectory (optionally overshooting and correcting).
pub fn move_with<F>(
    start: Vector,
    end: Vector,
    rng: &Rng,
    spec: &MoveSpec,
    mut settle: F,
) -> Vec<TimedVector>
where
    F: FnMut(TimedVector) -> Vector,
{
    let path_opts = PathOptions {
        move_speed: spec.move_speed,
    };
    if !spec.overshoot {
        return trajectory(start, end, Some(&path_opts), rng);
    }
    let overshoot_end = overshoot_point(end, spec.settings.radius, rng);
    let mut out = trajectory(start, overshoot_end, Some(&path_opts), rng);
    let last = *out.last().expect("trajectory always ends with target point");
    let dwell = &spec.settings.dwell;
    let resume = last.t + dwell.min_ms + rng.next() * (dwell.max_ms - dwell.min_ms).max(0.0);
    let correction_speed = spec.move_speed.unwrap_or(1.0) * spec.settings.correction_speed_factor;
    let corr_opts = PathOptions {
        move_speed: Some(correction_speed),
    };
    let back = trajectory(settle(last), end, Some(&corr_opts), rng);
    for p in back {
        out.push(TimedVector {
            x: p.x,
            y: p.y,
            t: resume + p.t,
        });
    }
    out
}

#[derive(Debug, Clone, Default)]
pub struct PlanMoveOptions {
    pub os: Option<Os>,
    pub move_speed: Option<f64>,
    pub overshoot: Option<bool>,
    pub settings: Option<SettingsOverride>,
}

/// Plans a human-like move from `start` to `end`.
pub fn plan_move(start: Vector, end: Vector, rng: &Rng, options: &PlanMoveOptions) -> Vec<TimedVector> {
    let resolved = resolve_settings(options.settings.as_ref());
    let overshoot = options
        .overshoot
        .unwrap_or_else(|| should_overshoot(start, end, resolved.overshoot.threshold));
    let spec = MoveSpec {
        move_speed: Some(options.move_speed.unwrap_or(resolved.mouse.speed_factor)),
        overshoot,
        settings: resolved.overshoot,
    };
    move_with(start, end, rng, &spec, |p| Vector::new(p.x, p.y))
}

/// Chooses a natural integer click point inside `box_rect` (or keeps `start` if already inside).
pub fn plan_point(box_rect: BoundingBox, rng: &Rng, start: Option<Vector>) -> Result<Vector> {
    if let Some(s) = start {
        if point_in_box(s, box_rect) {
            return Ok(s);
        }
    }
    get_random_box_point(box_rect, None, start, &[], rng)
}

#[derive(Debug, Clone)]
pub struct ClickTimingPlan {
    pub lead_ms: f64,
    pub presses: Vec<(f64, f64)>,
}

pub fn clicks_with(rng: &Rng, first: usize, last: usize) -> ClickTimingPlan {
    let mut lead_ms = if first == 1 {
        pre_click_dwell_ms(rng)
    } else {
        0.0
    };
    let mut presses = Vec::new();
    let mut at = 0.0_f64;
    for count in first..=last {
        if count > 1 {
            let gap = multi_click_gap_ms(rng);
            if count == first {
                lead_ms = gap;
            } else {
                at += gap;
            }
        }
        let up = at + click_hold_ms(rng, count > 1);
        presses.push((at, up));
        at = up;
    }
    ClickTimingPlan { lead_ms, presses }
}

#[derive(Debug, Clone)]
pub struct PlannedClick {
    pub dwell_ms: f64,
    pub presses: Vec<(f64, f64)>,
}

/// Plans a single, double, or triple click timing sequence.
pub fn plan_click(
    rng: &Rng,
    click_count: usize,
    since_release_ms: Option<f64>,
) -> PlannedClick {
    match since_release_ms {
        Some(since) if since < CLICK_RUN_MS => {
            let plan = clicks_with(rng, 2, click_count + 1);
            PlannedClick {
                dwell_ms: (plan.lead_ms - since).max(0.0),
                presses: plan.presses,
            }
        }
        Some(_) => {
            let plan = clicks_with(rng, 1, click_count);
            PlannedClick {
                dwell_ms: 0.0,
                presses: plan.presses,
            }
        }
        None => {
            let plan = clicks_with(rng, 1, click_count);
            PlannedClick {
                dwell_ms: plan.lead_ms,
                presses: plan.presses,
            }
        }
    }
}

/// Step representation returned by `plan_type`, `plan_press`, and `plan_select_all`.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedKeyStep {
    pub at: f64,
    pub action: KeyActionType,
    pub key: String,
}

#[derive(Debug, Clone, Default)]
pub struct PlanTypeOptions {
    pub os: Option<Os>,
    pub wpm: Option<f64>,
    pub mistake_multiplier: Option<f64>,
    pub pauses: Option<PauseSettings>,
}

fn key_session(rng: &Rng, options: &PlanTypeOptions) -> KeystrokeTiming {
    KeystrokeTiming::new(
        draw_session_wpm(rng, options.wpm.unwrap_or(DEFAULT_WPM)),
        rng.clone(),
        options.pauses.clone(),
    )
}

/// Plans typing `text` with human typos, corrections, and realistic timing.
pub fn plan_type(
    text: &str,
    rng: &Rng,
    options: &PlanTypeOptions,
) -> Result<Vec<PlannedKeyStep>> {
    let mut timing = key_session(rng, options);
    let planned = plan_typing(
        text,
        options.mistake_multiplier.unwrap_or(1.0),
        rng.clone(),
        &mut timing,
    )?;
    Ok(planned
        .actions
        .into_iter()
        .map(|a| PlannedKeyStep {
            at: a.at,
            action: a.action_type,
            key: a.key,
        })
        .collect())
}

/// Plans pressing a key or chord in Playwright notation (`"Enter"`, `"Control+a"`, `"A"`).
pub fn plan_press(key: &str, rng: &Rng, options: &PlanTypeOptions) -> Vec<PlannedKeyStep> {
    let mut parts: Vec<String> = key.split('+').map(ToOwned::to_owned).collect();
    let (main, mut modifiers) = if key.ends_with('+') && key.len() > 1 {
        let len = parts.len();
        ("+".to_owned(), parts[..len.saturating_sub(2)].to_vec())
    } else {
        let main = parts.pop().unwrap_or_default();
        (main, parts)
    };
    if needs_shift(&main) && !modifiers.iter().any(|m| m == "Shift") {
        modifiers.push("Shift".to_owned());
    }
    let mod_refs: Vec<&str> = modifiers.iter().map(String::as_str).collect();
    let session = key_session(rng, options);
    session
        .plan_chord(&mod_refs, &main)
        .into_iter()
        .map(|a| PlannedKeyStep {
            at: a.at,
            action: a.action_type,
            key: a.key,
        })
        .collect()
}

/// Plans select-all for `os` (`Meta+a` on macOS, `Control+a` elsewhere).
pub fn plan_select_all(rng: &Rng, os: Option<Os>, options: &PlanTypeOptions) -> Vec<PlannedKeyStep> {
    let chord = format!("{}+a", primary_modifier(os));
    plan_press(&chord, rng, options)
}

#[derive(Debug, Clone)]
pub struct WheelSpec {
    pub device: ScrollDevice,
    pub wheel: WheelSettings,
    pub speed_irregularity: f64,
    pub speed_factor: f64,
    pub stroke_scale: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlannedWheelStep {
    Wheel {
        dx: f64,
        dy: f64,
        at: f64,
    },
    Key {
        key: String,
        action: KeyActionType,
        at: f64,
    },
}

impl PlannedWheelStep {
    pub fn at(&self) -> f64 {
        match self {
            Self::Wheel { at, .. } | Self::Key { at, .. } => *at,
        }
    }
}

pub fn wheel_style_for(os: Option<Os>, settings: Option<&SettingsOverride>) -> WheelStyle {
    if let Some(style) = settings
        .and_then(|s| s.scroll.as_ref())
        .and_then(|sc| sc.wheel.as_ref())
        .and_then(|w| w.style)
    {
        return style;
    }
    match os {
        Some(Os::Macos) => WheelStyle::Macos,
        _ => WheelStyle::Notched,
    }
}

pub fn wheel_with(distance: f64, rng: &Rng, spec: &WheelSpec) -> (Vec<(f64, f64)>, f64, f64) {
    let is_mouse = spec.device == ScrollDevice::Mouse;
    let model = if is_mouse {
        generate_wheel_path(
            distance,
            rng,
            WheelModelOptions {
                style: spec.wheel.style,
                px_per_notch: spec.wheel.px_per_notch,
                px_per_line: spec.wheel.px_per_line,
                stroke_scale: spec.stroke_scale,
            },
        )
    } else {
        generate_scroll_path(distance, rng, None, spec.speed_irregularity)
    };

    let mut raw = Vec::with_capacity(model.len());
    let mut end = 0.0_f64;
    let mut total = 0.0_f64;
    for ev in model {
        total += ev.delta_y;
        raw.push((end, ev.delta_y));
        end += ev.delta_time / spec.speed_factor;
    }
    if is_mouse {
        return (raw, end, total);
    }

    let mut events = Vec::new();
    let mut accumulator = 0.0_f64;
    for (at, delta) in raw {
        accumulator += delta;
        if accumulator.abs() >= MIN_TRACKPAD_DELTA {
            let send = accumulator.trunc();
            accumulator -= send;
            events.push((at, send));
        }
    }
    if accumulator.round().abs() >= 1.0 {
        events.push((end, accumulator.round()));
    }
    (events, end, total)
}

pub fn wheel_steps(
    delta_x: f64,
    delta_y: f64,
    rng: &Rng,
    os: Option<Os>,
    spec: &WheelSpec,
) -> Vec<PlannedWheelStep> {
    let mut out = Vec::new();
    let mut t = 0.0_f64;
    if delta_y != 0.0 {
        let (events, end, _) = wheel_with(delta_y, rng, spec);
        for (at, d) in events {
            out.push(PlannedWheelStep::Wheel { dx: 0.0, dy: d, at });
        }
        t = end;
    }
    if delta_x != 0.0 {
        out.push(PlannedWheelStep::Key {
            key: "Shift".into(),
            action: KeyActionType::Down,
            at: t,
        });
        t += gaussian(rng, SHIFT_WHEEL_LEAD.0, SHIFT_WHEEL_LEAD.1).exp() / spec.speed_factor;
        let (events, _, _) = wheel_with(delta_x, rng, spec);
        for (at, d) in events {
            if os == Some(Os::Macos) {
                out.push(PlannedWheelStep::Wheel {
                    dx: d,
                    dy: 0.0,
                    at: t + at,
                });
            } else {
                out.push(PlannedWheelStep::Wheel {
                    dx: 0.0,
                    dy: d,
                    at: t + at,
                });
            }
        }
        let last_at = out.last().map(PlannedWheelStep::at).unwrap_or(t);
        t = last_at + gaussian(rng, SHIFT_WHEEL_TRAIL.0, SHIFT_WHEEL_TRAIL.1).exp() / spec.speed_factor;
        out.push(PlannedWheelStep::Key {
            key: "Shift".into(),
            action: KeyActionType::Up,
            at: t,
        });
    }
    out
}

#[derive(Debug, Clone, Default)]
pub struct PlanWheelOptions {
    pub os: Option<Os>,
    pub device: Option<ScrollDevice>,
    pub settings: Option<SettingsOverride>,
}

/// Plans vertical + horizontal wheel gestures.
pub fn plan_wheel(
    delta_x: f64,
    delta_y: f64,
    rng: &Rng,
    options: &PlanWheelOptions,
) -> Vec<PlannedWheelStep> {
    let mut resolved = resolve_settings(options.settings.as_ref());
    resolved.scroll.wheel.style = wheel_style_for(options.os, options.settings.as_ref());
    let spec = WheelSpec {
        device: options.device.unwrap_or(resolved.scroll.device),
        wheel: resolved.scroll.wheel,
        speed_irregularity: resolved.scroll.realism.speed_irregularity,
        speed_factor: resolved.scroll.speed_factor,
        stroke_scale: 1.0,
    };
    wheel_steps(delta_x, delta_y, rng, options.os, &spec)
}
