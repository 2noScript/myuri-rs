//! Typing model configuration ported from `HumanTyping` (`myuri/typing/config.ts`).

pub const DEFAULT_WPM: f64 = 80.0;
pub const WPM_STD: f64 = 10.0;

pub const PROB_ERROR: f64 = 0.04;
pub const PROB_SWAP_ERROR: f64 = 0.015;
pub const PROB_NOTICE_ERROR: f64 = 0.85;
pub const DRIFT_CORRECTION_PROB: f64 = 0.8;

pub const COMPLEX_WORD_ERROR_MULT: f64 = 1.5;
pub const COMMON_WORD_ERROR_MULT: f64 = 0.5;
pub const COMPOSED_ACCENT_ERROR_MULT: f64 = 2.0;
