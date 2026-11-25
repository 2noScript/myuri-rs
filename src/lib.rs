//! Rust port of `Myuri` `5.1.0` (`JWriter20/Myuri`) and embedded `cursory-js` `2.1.0`.

pub mod actions;
pub mod cursor;
pub mod cursory;
pub mod drivers;
pub mod exceptions;
pub mod geometry;
pub mod math;
pub mod motion;
pub mod options;
pub mod page;
pub mod page_functions;
pub mod plan;
pub mod profile;
pub mod rng;
pub mod scroll_plan;
pub mod settings;
pub mod target_os;
pub mod typing;

/// Crate version aligned with upstream `Myuri` 5.1.0.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use actions::{
    validate_steps, wheel_scale_from, Actions, ActionsOptions, Host, Step, Target, LANDING_MS,
    PARTS, RETRY_MS, SCROLL_PASSES, WHEEL_PROBE_MIN_PX, WHEEL_PROBE_SHARE, WHEEL_SCALE_RANGE,
};
#[cfg(feature = "playwright")]
pub use cursor::create_playwright_cursor;
pub use cursor::{create_cursor, Cursor, CursorOptions, Destination};
#[cfg(feature = "playwright")]
pub use drivers::{playwright_driver, PlaywrightDriver};
pub use drivers::{define_driver, DefinedDriver, Driver, PressOptions};
pub use exceptions::{MyuriError, Result};
pub use geometry::{get_better_starting_position, get_random_box_point, path, PathTarget, TimestampedPoint};
pub use math::{BoundingBox, TimedVector, Vector, ViewportSize};
pub use motion::{
    click_hold_ms, drag_carry_speed, drag_end_ms, drag_start_ms, generate_wheel_path,
    multi_click_gap_ms, pre_click_dwell_ms,
};
pub use options::{
    BoxOptions, ClickOptions, CursorDefaults, DragOptions, MoveOptions, MoveToOptions,
    PathOptions, RandomMoveOptions, ScrollOptions, TypeOptions,
};
pub use page_functions::{on_selector_script, BOX_JS, FOCUSED_FIELD_JS, HIT_TEST_JS, RESOLVE_SELECTOR_JS};
pub use plan::{
    plan_click, plan_move, plan_point, plan_press, plan_select_all, plan_type, plan_wheel,
    wheel_steps, PlanMoveOptions, PlanTypeOptions, PlanWheelOptions, PlannedClick, PlannedKeyStep,
    PlannedWheelStep,
};
pub use profile::generate_profile;
pub use rng::{gaussian, generate_random_seed, make_rng, Rng, SeededRng};
pub use scroll_plan::{ScrollPlan, SCROLL_EDGE, SCROLL_PAD, SCROLL_PLAN_JS};
pub use settings::{default_settings, resolve_settings, resolve_settings_value, MyuriSettings, ScrollDevice, SettingsOverride, WheelStyle};
pub use target_os::{primary_modifier, MouseButton, Os};
pub use typing::{
    draw_session_wpm, needs_shift, strokes_for_text, HumanTyper, HumanTyperOptions, KeystrokeTiming,
};

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_matches_myuri_upstream() {
        assert_eq!(VERSION, "5.1.0");
    }
}
