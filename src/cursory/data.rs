//! Embedded recorded human cursor trajectories from `cursory-js`.

use std::sync::OnceLock;

use flate2::read::GzDecoder;
use serde::Deserialize;

static TRAJECTORIES_GZ: &[u8] = include_bytes!("trajectories.json.gz");
static LOADED: OnceLock<Vec<TrajectoryRecord>> = OnceLock::new();

#[derive(Debug, Clone, Deserialize)]
pub struct TrajectoryRecord {
    pub points: Vec<[f64; 2]>,
    pub timing: Vec<f64>,
    pub length: f64,
}

/// Returns the shared slice of all recorded human trajectories, lazily decompressed once.
pub fn loaded_trajectories() -> &'static [TrajectoryRecord] {
    LOADED
        .get_or_init(|| {
            let decoder = GzDecoder::new(TRAJECTORIES_GZ);
            serde_json::from_reader(decoder).expect("embedded trajectories.json.gz is valid")
        })
        .as_slice()
}
