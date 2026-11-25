pub mod click_model;
pub mod scroll_model;
pub mod wheel_model;

pub use click_model::{
    click_hold_ms, drag_carry_speed, drag_end_ms, drag_start_ms, multi_click_gap_ms,
    pre_click_dwell_ms,
};
pub use scroll_model::{generate_scroll_path, ScrollEvent, DEFAULT_SPEED_IRREGULARITY};
pub use wheel_model::{generate_wheel_path, WheelModelOptions};
