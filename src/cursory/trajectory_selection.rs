use std::sync::OnceLock;

use super::data::{loaded_trajectories, TrajectoryRecord};
use super::numeric::{arg_sort_top_n, hypot, search_sorted_right, sum, EPSILON};
use super::random::Generator;

struct PrecomputedIndex {
    norm_dx: Vec<f64>,
    norm_dy: Vec<f64>,
    displacements: Vec<f64>,
    lengths: Vec<f64>,
    sorted_efficiencies: Vec<f64>,
}

static INDEX: OnceLock<PrecomputedIndex> = OnceLock::new();

fn endpoint_delta(t: &TrajectoryRecord, axis: usize) -> f64 {
    t.points[t.points.len() - 1][axis] - t.points[0][axis]
}

fn precomputed() -> &'static PrecomputedIndex {
    INDEX.get_or_init(|| {
        let records = loaded_trajectories();
        let n = records.len();
        let mut norm_dx = Vec::with_capacity(n);
        let mut norm_dy = Vec::with_capacity(n);
        let mut displacements = Vec::with_capacity(n);
        let mut lengths = Vec::with_capacity(n);
        let mut efficiencies = Vec::with_capacity(n);

        for t in records {
            let dx = endpoint_delta(t, 0);
            let dy = endpoint_delta(t, 1);
            let disp = hypot(dx, dy);
            displacements.push(disp);
            lengths.push(t.length);
            if disp == 0.0 {
                norm_dx.push(0.0);
                norm_dy.push(0.0);
            } else {
                norm_dx.push(dx / disp);
                norm_dy.push(dy / disp);
            }
            efficiencies.push(if t.length > 0.0 { disp / t.length } else { 1.0 });
        }

        efficiencies.sort_by(|a, b| a.total_cmp(b));
        PrecomputedIndex {
            norm_dx,
            norm_dy,
            displacements,
            lengths,
            sorted_efficiencies: efficiencies,
        }
    })
}

pub fn find_nearest_trajectory(
    target_start: [f64; 2],
    target_end: [f64; 2],
    direction_weight: f64,
    length_weight: f64,
    top_n: usize,
) -> Vec<&'static TrajectoryRecord> {
    let records = loaded_trajectories();
    let idx = precomputed();
    let dx_target = target_end[0] - target_start[0];
    let dy_target = target_end[1] - target_start[1];
    let length_target = hypot(dx_target, dy_target);
    if length_target == 0.0 {
        return arg_sort_top_n(&idx.lengths, top_n)
            .into_iter()
            .map(|i| &records[i])
            .collect();
    }
    let norm_dx_target = dx_target / length_target;
    let norm_dy_target = dy_target / length_target;
    let denom = length_target.max(1.0);
    let mut scores = Vec::with_capacity(records.len());
    for i in 0..records.len() {
        let direction_distance =
            1.0 - (idx.norm_dx[i] * norm_dx_target + idx.norm_dy[i] * norm_dy_target);
        let length_difference_ratio = (idx.displacements[i] - length_target).abs() / denom;
        scores.push(direction_weight * direction_distance + length_weight * length_difference_ratio);
    }
    arg_sort_top_n(&scores, top_n)
        .into_iter()
        .map(|i| &records[i])
        .collect()
}

pub struct ClosestSelection {
    pub trajectory: &'static TrajectoryRecord,
    pub dx_target: f64,
    pub dy_target: f64,
    pub length_target: f64,
}

pub fn find_closest_trajectory(
    target_start: [f64; 2],
    target_end: [f64; 2],
    rng: &mut Generator,
    directness: f64,
) -> ClosestSelection {
    assert!(
        (0.0..=1.0).contains(&directness),
        "directness must be between zero and one"
    );
    let num_nearest = 5usize;
    let iterations = 20usize;
    let dx_target = target_end[0] - target_start[0];
    let dy_target = target_end[1] - target_start[1];
    let length_target = hypot(dx_target, dy_target);

    let mut candidates = find_nearest_trajectory(target_start, target_end, 0.8, 0.2, num_nearest);
    let jitter = length_target * 0.1;
    for _ in 0..iterations {
        let perturbed_end = [
            target_end[0] + rng.uniform(-jitter, jitter),
            target_end[1] + rng.uniform(-jitter, jitter),
        ];
        candidates.extend(find_nearest_trajectory(
            target_start,
            perturbed_end,
            0.8,
            0.2,
            num_nearest,
        ));
    }

    let idx = precomputed();
    let count = loaded_trajectories().len() as f64;
    let preferred_rank = 0.2 + 0.75 * directness;
    let mut weights = Vec::with_capacity(candidates.len());
    for candidate in &candidates {
        let displacement = hypot(endpoint_delta(candidate, 0), endpoint_delta(candidate, 1));
        let efficiency = if candidate.length > 0.0 {
            displacement / candidate.length
        } else {
            1.0
        };
        let rank =
            (search_sorted_right(&idx.sorted_efficiencies, efficiency) as f64) / count;
        let offset = (rank - preferred_rank) / 0.2;
        let core = (-0.5 * (offset * offset)).exp();
        let centered_rank = 2.0 * rank - 1.0;
        let tail_floor = 0.05 * (1.0 + 0.1 * centered_rank.abs().powi(3));
        weights.push(core + tail_floor);
    }
    let total = sum(&weights);
    for w in &mut weights {
        *w /= total;
    }
    let chosen = candidates[rng.choice(&weights)];
    ClosestSelection {
        trajectory: chosen,
        dx_target,
        dy_target,
        length_target,
    }
}

pub fn morph_trajectory(
    points: &[[f64; 2]],
    target_start: [f64; 2],
    target_end: [f64; 2],
    dx_target: f64,
    dy_target: f64,
    length_target: f64,
) -> Vec<[f64; 2]> {
    let start = points[0];
    let end = points[points.len() - 1];
    let dx_orig = end[0] - start[0];
    let dy_orig = end[1] - start[1];
    let length_orig = (dx_orig * dx_orig + dy_orig * dy_orig).sqrt();
    let scale = if length_orig != 0.0 {
        length_target / length_orig
    } else {
        1.0
    };
    let rotation = dy_target.atan2(dx_target) - dy_orig.atan2(dx_orig);
    let cos = rotation.cos();
    let sin = rotation.sin();
    let mut morphed: Vec<[f64; 2]> = points
        .iter()
        .map(|p| {
            let x = (p[0] - start[0]) * scale;
            let y = (p[1] - start[1]) * scale;
            [
                x * cos - y * sin + target_start[0],
                x * sin + y * cos + target_start[1],
            ]
        })
        .collect();
    morphed[0] = target_start;
    let last = morphed.len() - 1;
    morphed[last] = target_end;
    morphed
}

pub fn endpoint_weights(points: &[[f64; 2]]) -> Vec<f64> {
    let count = points.len();
    let mut progress = vec![0.0_f64; count];
    for i in 1..count {
        let dx = points[i][0] - points[i - 1][0];
        let dy = points[i][1] - points[i - 1][1];
        progress[i] = progress[i - 1] + (dx * dx + dy * dy).sqrt();
    }
    let mut weights = vec![0.0_f64; count];
    let total = if count > 0 { progress[count - 1] } else { 0.0 };
    if total == 0.0 {
        return weights;
    }
    for i in 0..count {
        let fraction = progress[i] / total;
        let weight = 4.0 * fraction * (1.0 - fraction);
        weights[i] = weight * weight;
    }
    weights
}

pub fn jitter_trajectory(
    points: &[[f64; 2]],
    trajectory_length: f64,
    rng: &mut Generator,
    scale: f64,
) -> Vec<[f64; 2]> {
    let count = points.len();
    let length_scale = (trajectory_length / 400.0).min(1.0);
    let mut gaps = vec![0.0_f64; count.saturating_sub(1)];
    for i in 0..count.saturating_sub(1) {
        let dx = points[i + 1][0] - points[i][0];
        let dy = points[i + 1][1] - points[i][1];
        gaps[i] = (dx * dx + dy * dy).sqrt();
    }
    let weights = endpoint_weights(points);
    let mut adaptive_scales = Vec::with_capacity(count);
    for i in 0..count {
        let left = if i > 0 { gaps[i - 1] } else { 0.0 };
        let right = if i < count - 1 { gaps[i] } else { 0.0 };
        let average_gap = (left + right) / 2.0;
        adaptive_scales
            .push(scale * (average_gap / average_gap.max(1.0)) * length_scale * weights[i]);
    }

    let mut normal_x = vec![0.0_f64; count];
    let mut normal_y = vec![0.0_f64; count];
    for i in 0..count {
        let ahead = points[(i + 1).min(count - 1)];
        let behind = if i == 0 { points[0] } else { points[i - 1] };
        let tangent_x = ahead[0] - behind[0];
        let tangent_y = ahead[1] - behind[1];
        let tangent_len = (tangent_x * tangent_x + tangent_y * tangent_y).sqrt();
        if tangent_len > EPSILON {
            normal_x[i] = -tangent_y / tangent_len;
            normal_y[i] = tangent_x / tangent_len;
        }
    }

    let innovations = rng.standard_normal_array(count);
    let correlation = 0.75_f64;
    let innovation_scale = (1.0 - correlation * correlation).sqrt();
    let mut jittered = Vec::with_capacity(count);
    let mut lateral_offset = 0.0;
    for i in 0..count {
        lateral_offset = if i == 0 {
            innovations[0]
        } else {
            correlation * lateral_offset + innovation_scale * innovations[i]
        };
        let displacement = lateral_offset * adaptive_scales[i];
        jittered.push([
            points[i][0] + normal_x[i] * displacement,
            points[i][1] + normal_y[i] * displacement,
        ]);
    }
    jittered
}

pub fn generate_middle_biased_point(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    rng: &mut Generator,
    bias_factor: f64,
) -> [f64; 2] {
    let min_x = x1.min(x2);
    let max_x = x1.max(x2);
    let min_y = y1.min(y2);
    let max_y = y1.max(y2);
    let offset_x = rng.normal(0.0, (max_x - min_x) / (2.0 * bias_factor));
    let offset_y = rng.normal(0.0, (max_y - min_y) / (2.0 * bias_factor));
    [
        ((min_x + max_x) / 2.0 + offset_x).clamp(min_x, max_x),
        ((min_y + max_y) / 2.0 + offset_y).clamp(min_y, max_y),
    ]
}

#[inline]
fn signed_sqrt(value: f64) -> f64 {
    let root = value.abs().sqrt();
    if value < 0.0 {
        -root
    } else {
        root
    }
}

pub fn knot_trajectory(
    points: &[[f64; 2]],
    target_start: [f64; 2],
    target_end: [f64; 2],
    rng: &mut Generator,
) -> Vec<[f64; 2]> {
    let num_knots = 5usize;
    let knot_strength = 0.15_f64;
    let count = points.len();
    let mut offset_x = vec![0.0_f64; count];
    let mut offset_y = vec![0.0_f64; count];

    for _ in 0..num_knots {
        let knot = generate_middle_biased_point(
            target_start[0],
            target_start[1],
            target_end[0],
            target_end[1],
            rng,
            2.0,
        );
        let mut to_knot_x = Vec::with_capacity(count);
        let mut to_knot_y = Vec::with_capacity(count);
        let mut distances = Vec::with_capacity(count);
        let mut max_distance = f64::NEG_INFINITY;
        for p in points {
            let tx = knot[0] - p[0];
            let ty = knot[1] - p[1];
            let d = (tx * tx + ty * ty).sqrt();
            if d > max_distance {
                max_distance = d;
            }
            to_knot_x.push(tx);
            to_knot_y.push(ty);
            distances.push(d);
        }
        if max_distance < 1e-6 {
            continue;
        }
        for i in 0..count {
            let scaling = (1.0 - distances[i] / max_distance) * knot_strength;
            offset_x[i] += to_knot_x[i] * scaling;
            offset_y[i] += to_knot_y[i] * scaling;
        }
    }

    let weights = endpoint_weights(points);
    points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            [
                p[0] + signed_sqrt(offset_x[i]) * weights[i],
                p[1] + signed_sqrt(offset_y[i]) * weights[i],
            ]
        })
        .collect()
}

pub fn find_trajectory(
    target_start: [f64; 2],
    target_end: [f64; 2],
    rng: &mut Generator,
    directness: f64,
) -> (Vec<[f64; 2]>, Vec<f64>) {
    let ClosestSelection {
        trajectory,
        dx_target: _,
        dy_target: _,
        length_target,
    } = find_closest_trajectory(target_start, target_end, rng, directness);

    let recorded_length = trajectory.length;
    let mut timings = trajectory.timing.clone();
    if length_target > 0.0 && length_target < recorded_length {
        let time_scale = (length_target / recorded_length).sqrt();
        let start_time = timings[0];
        for timing in &mut timings {
            *timing = start_time + (*timing - start_time) * time_scale;
        }
    }
    let morphed = morph_trajectory(
        &trajectory.points,
        target_start,
        target_end,
        target_end[0] - target_start[0],
        target_end[1] - target_start[1],
        length_target,
    );
    let jittered = jitter_trajectory(&morphed, length_target, rng, 0.01);
    let points = knot_trajectory(&jittered, target_start, target_end, rng);
    (points, timings)
}
