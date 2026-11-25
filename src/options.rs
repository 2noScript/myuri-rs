use crate::math::BoundingBox;
use crate::target_os::MouseButton;

/// Padding controls for choosing a point inside a target box.
#[derive(Debug, Clone, Default)]
pub struct BoxOptions {
    pub width_padding_percentage: Option<f64>,
    pub height_padding_percentage: Option<f64>,
    pub padding_left: Option<f64>,
    pub padding_right: Option<f64>,
    pub padding_top: Option<f64>,
    pub padding_bottom: Option<f64>,
}

/// Trajectory speed control.
#[derive(Debug, Clone, Default)]
pub struct PathOptions {
    pub move_speed: Option<f64>,
}

/// Options for moving onto an element.
#[derive(Debug, Clone, Default)]
pub struct MoveOptions {
    pub box_options: BoxOptions,
    pub move_speed: Option<f64>,
    pub max_tries: Option<usize>,
    pub overshoot_threshold: Option<f64>,
    pub move_delay: Option<f64>,
    pub randomize_move_delay: Option<bool>,
    pub timeout: Option<f64>,
    pub exclude_selectors: Vec<String>,
}

/// Options for clicking an element or current cursor point.
#[derive(Debug, Clone, Default)]
pub struct ClickOptions {
    pub move_options: MoveOptions,
    pub button: Option<MouseButton>,
    pub click_count: Option<usize>,
    pub hold_ms: Option<f64>,
    pub wait_for_click: Option<f64>,
}

/// Options for dragging between two selectors or viewport points.
#[derive(Debug, Clone, Default)]
pub struct DragOptions {
    pub move_options: MoveOptions,
    pub button: Option<MouseButton>,
    pub wait_for_release: Option<f64>,
}

/// Options for moving to an absolute viewport coordinate.
#[derive(Debug, Clone, Default)]
pub struct MoveToOptions {
    pub move_speed: Option<f64>,
    pub move_delay: Option<f64>,
    pub randomize_move_delay: Option<bool>,
}

/// Options for idle background cursor movements.
#[derive(Debug, Clone, Default)]
pub struct RandomMoveOptions {
    pub move_delay: Option<f64>,
    pub randomize_move_delay: Option<bool>,
    pub move_speed: Option<f64>,
}

/// Options for scrolling an element into view.
#[derive(Debug, Clone, Default)]
pub struct ScrollOptions {
    pub speed_factor: Option<f64>,
    pub finish_position: Option<f64>,
    pub speed_irregularity: Option<f64>,
    pub timeout: Option<f64>,
    pub max_tries: Option<usize>,
}

/// Options for typing into a focused or selected element.
#[derive(Debug, Clone, Default)]
pub struct TypeOptions {
    pub timeout: Option<f64>,
    pub max_tries: Option<usize>,
    pub clear_existing: Option<bool>,
}

/// Padding + exclusion boxes attached to an active `Cursor` action.
#[derive(Debug, Clone, Default)]
pub struct ResolvedBoxOptions {
    pub padding: BoxOptions,
    pub exclude: Vec<BoundingBox>,
}

/// Default per-action options attached to a `Cursor`.
#[derive(Debug, Clone, Default)]
pub struct CursorDefaults {
    pub move_opts: MoveOptions,
    pub move_to: MoveToOptions,
    pub click: ClickOptions,
    pub scroll: ScrollOptions,
    pub typing: TypeOptions,
    pub random_move: RandomMoveOptions,
}
