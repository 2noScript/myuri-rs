pub mod bigrams;
pub mod config;
pub mod keyboard;
pub mod language;
pub mod markov;
pub mod timing;
pub mod typer;

pub use config::DEFAULT_WPM;
pub use timing::{char_to_stroke, needs_shift, strokes_for_text, KeyAction, KeyActionType, KeystrokeTiming, Stroke};
pub use typer::{draw_session_wpm, plan_typing, HumanTyper, HumanTyperOptions, PlannedTyping, TyperInput};
