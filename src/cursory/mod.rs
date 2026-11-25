//! Embedded Rust port of `cursory-js` `2.1.0`.

pub mod data;
pub mod numeric;
pub mod random;
pub mod trajectory_selection;

use numeric::{hypot, rint, sum, EPSILON};
use random::{default_generator, Generator};
use trajectory_selection::{find_trajectory, jitter_trajectory, knot_trajectory, morph_trajectory};

/// Options configuring `generate_trajectory`.
#[derive(Debug, Clone, Copy)]
pub struct TrajectoryOptions {
    pub frequency: f64,
    pub frequency_randomizer: f64,
    pub seed: Option<u128>,
    pub directness: f64,
}

impl Default for TrajectoryOptions {
    fn default() -> Self {
        Self {
            frequency: 60.0,
            frequency_randomizer: 1.0,
            seed: None,
            directness: 0.65,
        }
    }
}

/// Generated cursor path with millimeter-accurate timing offsets (ms).
#[derive(Debug, Clone)]
pub struct GeneratedTrajectory {
    pub points: Vec<[f64; 2]>,
    pub timings: Vec<f64>,
}

fn rescale_to_total(intervals: &mut [f64], total_time: f64) {
    let factor = total_time / sum(intervals);
    for slot in intervals {
        *slot *= factor;
    }
}

fn sample_timings(
    timings: &[f64],
    frequency: f64,
    frequency_randomizer: f64,
    rng: &mut Generator,
) -> Vec<f64> {
    let total_time = 1.0_f64.max(rint(timings[timings.len() - 1] - timings[0]));
    let interval_count = 1usize.max(rint((total_time * frequency) / 1000.0) as usize);
    let mut recorded_intervals = Vec::new();
    for i in 1..timings.len() {
        let interval = timings[i] - timings[i - 1];
        if interval > 0.0 {
            recorded_intervals.push(interval);
        }
    }
    if recorded_intervals.is_empty() {
        recorded_intervals.push(1.0);
    }
    let roll_by = rng.integers(recorded_intervals.len());
    let mut sampled_intervals = Vec::with_capacity(interval_count);
    for i in 0..interval_count {
        let source_index =
            ((((i as f64) + 0.5) * (recorded_intervals.len() as f64)) / (interval_count as f64))
                .floor() as usize;
        sampled_intervals
            .push(recorded_intervals[(source_index + roll_by) % recorded_intervals.len()]);
    }
    rescale_to_total(&mut sampled_intervals, total_time);
    if frequency_randomizer != 0.0 {
        let noise = rng.uniform_array(-frequency_randomizer, frequency_randomizer, interval_count);
        for (slot, n) in sampled_intervals.iter_mut().zip(noise) {
            *slot = (*slot + n).max(EPSILON);
        }
        rescale_to_total(&mut sampled_intervals, total_time);
    }
    let mut sampled_timings = Vec::with_capacity(interval_count + 1);
    sampled_timings.push(0.0);
    let mut elapsed = 0.0;
    for dt in sampled_intervals {
        elapsed += dt;
        sampled_timings.push(rint(elapsed));
    }
    if let Some(last) = sampled_timings.last_mut() {
        *last = total_time;
    }
    sampled_timings
}

/// Generates a human-realistic cursor trajectory from `target_start` to `target_end`.
pub fn generate_trajectory(
    target_start: [f64; 2],
    target_end: [f64; 2],
    options: TrajectoryOptions,
) -> GeneratedTrajectory {
    assert!(
        options.frequency > 0.0,
        "frequency must be greater than zero"
    );
    assert!(
        options.frequency_randomizer >= 0.0,
        "frequency_randomizer must not be negative"
    );
    assert!(
        (0.0..=1.0).contains(&options.directness),
        "directness must be between zero and one"
    );
    if target_start[0] == target_end[0] && target_start[1] == target_end[1] {
        return GeneratedTrajectory {
            points: vec![target_start],
            timings: vec![0.0],
        };
    }

    let mut rng = default_generator(options.seed);
    let (trajectory_points, raw_timings) =
        find_trajectory(target_start, target_end, &mut rng, options.directness);
    let start_time = raw_timings[0];
    let timings: Vec<f64> = raw_timings.into_iter().map(|t| t - start_time).collect();
    let sampled_timings = sample_timings(
        &timings,
        options.frequency,
        options.frequency_randomizer,
        &mut rng,
    );

    let sampled_points: Vec<[f64; 2]> = sampled_timings
        .iter()
        .map(|&sample_time| {
            let mut prev_idx = 0usize;
            for (i, &t) in timings.iter().enumerate() {
                if t <= sample_time {
                    prev_idx = i;
                }
            }
            let next_idx = (prev_idx + 1).min(timings.len() - 1);
            let p0 = trajectory_points[prev_idx];
            let p1 = trajectory_points[next_idx];
            let t0 = timings[prev_idx];
            let t1 = timings[next_idx];
            let alpha = if t1 != t0 {
                (sample_time - t0) / (t1 - t0)
            } else {
                0.0
            };
            [p0[0] + alpha * (p1[0] - p0[0]), p0[1] + alpha * (p1[1] - p0[1])]
        })
        .collect();

    let dx_target = target_end[0] - target_start[0];
    let dy_target = target_end[1] - target_start[1];
    let trajectory_length = (dx_target * dx_target + dy_target * dy_target).sqrt();
    let knotted = knot_trajectory(&sampled_points, target_start, target_end, &mut rng);
    let jittered = jitter_trajectory(&knotted, trajectory_length, &mut rng, 0.01);
    let points = morph_trajectory(
        &jittered,
        target_start,
        target_end,
        dx_target,
        dy_target,
        hypot(dx_target, dy_target),
    );
    GeneratedTrajectory {
        points,
        timings: sampled_timings,
    }
}
