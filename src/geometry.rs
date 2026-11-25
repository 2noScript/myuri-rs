use crate::cursory::{generate_trajectory, TrajectoryOptions};
use crate::exceptions::{MyuriError, Result};
use crate::math::{direction, magnitude, BoundingBox, TimedVector, Vector, ViewportSize};
use crate::options::{BoxOptions, PathOptions};
use crate::rng::Rng;

/// Keep the cursor at least this many px from the viewport edges.
pub const VIEWPORT_PADDING: f64 = 10.0;

pub fn clamp_to_browser_bounds(x: f64, y: f64, viewport: ViewportSize) -> Vector {
    Vector {
        x: x.clamp(VIEWPORT_PADDING, viewport.width - VIEWPORT_PADDING),
        y: y.clamp(VIEWPORT_PADDING, viewport.height - VIEWPORT_PADDING),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SidePaddingPx {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

fn clamp_pct(v: Option<f64>) -> Option<f64> {
    v.map(|x| x.clamp(0.0, 100.0))
}

/// Resolves per-side padding (px) inside a target box.
pub fn resolve_side_padding_px(
    width: f64,
    height: f64,
    options: Option<&BoxOptions>,
    default_width_pct: f64,
    default_height_pct: f64,
) -> SidePaddingPx {
    let width_pct = clamp_pct(options.and_then(|o| o.width_padding_percentage)).unwrap_or(default_width_pct);
    let height_pct = clamp_pct(options.and_then(|o| o.height_padding_percentage)).unwrap_or(default_height_pct);
    let left_pct = clamp_pct(options.and_then(|o| o.padding_left)).unwrap_or(width_pct / 2.0);
    let right_pct = clamp_pct(options.and_then(|o| o.padding_right)).unwrap_or(width_pct / 2.0);
    let top_pct = clamp_pct(options.and_then(|o| o.padding_top)).unwrap_or(height_pct / 2.0);
    let bottom_pct = clamp_pct(options.and_then(|o| o.padding_bottom)).unwrap_or(height_pct / 2.0);

    let mut left = (width * left_pct) / 100.0;
    let mut right = (width * right_pct) / 100.0;
    let mut top = (height * top_pct) / 100.0;
    let mut bottom = (height * bottom_pct) / 100.0;

    if left + right > width {
        let scale = width / (left + right);
        left *= scale;
        right *= scale;
    }
    if top + bottom > height {
        let scale = height / (top + bottom);
        top *= scale;
        bottom *= scale;
    }

    SidePaddingPx {
        left,
        right,
        top,
        bottom,
    }
}

pub fn is_point_excluded(point: Vector, boxes: &[BoundingBox]) -> bool {
    boxes.iter().any(|b| {
        point.x >= b.x && point.x <= b.x + b.width && point.y >= b.y && point.y <= b.y + b.height
    })
}

/// Chooses a center-biased or approach-biased integer point inside `target`.
pub fn get_random_box_point(
    target: BoundingBox,
    options: Option<&BoxOptions>,
    current_position: Option<Vector>,
    exclude_boxes: &[BoundingBox],
    rng: &Rng,
) -> Result<Vector> {
    let is_small = target.width < 40.0 || target.height < 40.0;
    let sides = resolve_side_padding_px(
        target.width,
        target.height,
        options,
        if is_small { 50.0 } else { 25.0 },
        70.0,
    );
    let eff_x = target.x + sides.left;
    let eff_y = target.y + sides.top;
    let eff_w = (target.width - sides.left - sides.right).max(0.0);
    let eff_h = (target.height - sides.top - sides.bottom).max(0.0);
    let has_excludes = !exclude_boxes.is_empty();

    let sample_normal = |spread: f64| -> Vector {
        let nx = (crate::rng::gaussian(rng, 0.0, 1.0) * spread + 0.5).clamp(0.0, 1.0);
        let ny = (crate::rng::gaussian(rng, 0.0, 1.0) * spread + 0.5).clamp(0.0, 1.0);
        Vector {
            x: eff_x + nx * eff_w,
            y: eff_y + ny * eff_h,
        }
    };
    let sample_uniform = || -> Vector {
        Vector {
            x: eff_x + rng.next() * eff_w,
            y: eff_y + rng.next() * eff_h,
        }
    };

    if current_position.is_none() {
        let max_attempts = if has_excludes { 64 } else { 1 };
        for _ in 0..max_attempts {
            let p = sample_normal(0.2);
            if !is_point_excluded(p, exclude_boxes) {
                return Ok(Vector {
                    x: p.x.round(),
                    y: p.y.round(),
                });
            }
        }
        for _ in 0..128 {
            let p = sample_uniform();
            if !is_point_excluded(p, exclude_boxes) {
                return Ok(Vector {
                    x: p.x.round(),
                    y: p.y.round(),
                });
            }
        }
        return Err(MyuriError::InvalidArgument(
            "could not find a non-excluded point inside target bounds".into(),
        ));
    }

    let current = current_position.unwrap();
    let mut candidates = Vec::with_capacity(10);
    for _ in 0..10 {
        candidates.push(sample_normal(0.25));
    }
    if has_excludes {
        candidates.retain(|c| !is_point_excluded(*c, exclude_boxes));
        for _ in 0..128 {
            if candidates.len() >= 10 {
                break;
            }
            let p = sample_uniform();
            if !is_point_excluded(p, exclude_boxes) {
                candidates.push(p);
            }
        }
        if candidates.is_empty() {
            return Err(MyuriError::InvalidArgument(
                "could not find a non-excluded point inside target bounds".into(),
            ));
        }
    }

    let center_x = eff_x + eff_w / 2.0;
    let center_y = eff_y + eff_h / 2.0;
    let mut best = candidates[0];
    let mut best_score = f64::INFINITY;
    for c in candidates {
        let dist = (c.x - current.x).hypot(c.y - current.y);
        let center_dist = (c.x - center_x).hypot(c.y - center_y);
        let score = dist * 0.7 + center_dist * 0.3;
        if score < best_score {
            best_score = score;
            best = c;
        }
    }
    Ok(Vector {
        x: best.x.round(),
        y: best.y.round(),
    })
}

/// Natural off-origin starting position (center-left of viewport).
pub fn get_better_starting_position(dimensions: Option<ViewportSize>, rng: &Rng) -> Vector {
    let vp = dimensions.unwrap_or(ViewportSize {
        width: 1920.0,
        height: 1080.0,
    });
    Vector {
        x: vp.width * 0.2 + rng.next() * 0.15,
        y: vp.height * 0.35 + rng.next() * 0.2,
    }
}

#[inline]
pub fn should_overshoot(a: Vector, b: Vector, threshold: f64) -> bool {
    magnitude(direction(a, b)) > threshold
}

const REPORT_MU: f64 = 2.303;
const REPORT_SD: f64 = 0.091;
const REPORT_MS: f64 = 10.0;

/// One human-like stroke from `start` to `end`, sampled at realistic ~10ms USB mouse report intervals.
pub fn trajectory(
    start: Vector,
    end: Vector,
    options: Option<&PathOptions>,
    rng: &Rng,
) -> Vec<TimedVector> {
    let distance = (end.x - start.x).hypot(end.y - start.y);
    let mut speed_factor = match options.and_then(|o| o.move_speed) {
        Some(ms) if ms > 0.0 => 1.0 / ms,
        _ => 1.0,
    };
    if distance < 100.0 {
        let small_movement_boost = 1.5 + 1.5 * (1.0 - distance / 100.0);
        speed_factor /= small_movement_boost;
    }

    let seed = (rng.next() * 4_294_967_296.0).floor() as u32;
    let generated = generate_trajectory(
        [start.x, start.y],
        [end.x, end.y],
        TrajectoryOptions {
            frequency: (1000.0 / REPORT_MS) * speed_factor,
            frequency_randomizer: 1.0,
            seed: Some(u128::from(seed)),
            directness: 0.65,
        },
    );

    let times: Vec<f64> = generated.timings.iter().map(|t| t * speed_factor).collect();
    let total = times[times.len() - 1];
    let points = &generated.points;

    let mut out = Vec::new();
    let report = || crate::rng::gaussian(rng, REPORT_MU, REPORT_SD).exp();
    let mut j = 0usize;
    let mut t = report();
    while t < total {
        while j + 1 < times.len() && times[j + 1] <= t {
            j += 1;
        }
        if j + 1 >= times.len() {
            break;
        }
        let a = (t - times[j]) / (times[j + 1] - times[j]);
        let x = points[j][0] + a * (points[j + 1][0] - points[j][0]);
        let y = points[j][1] + a * (points[j + 1][1] - points[j][1]);
        out.push(TimedVector {
            x: x.max(0.0),
            y: y.max(0.0),
            t,
        });
        t += report();
    }
    out.push(TimedVector {
        x: end.x,
        y: end.y,
        t,
    });
    out
}

/// Target for `path`: either a point or a bounding box (center used).
#[derive(Debug, Clone, Copy)]
pub enum PathTarget {
    Point(Vector),
    Box(BoundingBox),
}

/// Timestamped path entry (epoch ms).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimestampedPoint {
    pub x: f64,
    pub y: f64,
    pub timestamp: u64,
}

/// Generates a timed human-like path from `start` to `end`.
pub fn path(
    start: Vector,
    end: PathTarget,
    options: Option<&PathOptions>,
    rng: &Rng,
) -> Vec<TimestampedPoint> {
    let end_point = match end {
        PathTarget::Point(p) => p,
        PathTarget::Box(b) => Vector {
            x: b.x + b.width / 2.0,
            y: b.y + b.height / 2.0,
        },
    };
    let start_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    trajectory(start, end_point, options, rng)
        .into_iter()
        .map(|p| TimestampedPoint {
            x: p.x,
            y: p.y,
            timestamp: start_ms + (p.t.round() as u64),
        })
        .collect()
}
