use serde::{Deserialize, Serialize};

use crate::exceptions::{MyuriError, Result};

/// Browser target OS (`macos`, `windows`, `linux`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Macos,
    Windows,
    Linux,
}

impl Os {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "macos" => Ok(Self::Macos),
            "windows" => Ok(Self::Windows),
            "linux" => Ok(Self::Linux),
            other => Err(MyuriError::UnknownOs(other.to_owned())),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Macos => "macos",
            Self::Windows => "windows",
            Self::Linux => "linux",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    #[default]
    Left,
    Right,
    Middle,
}

impl MouseButton {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Middle => "middle",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            "middle" => Ok(Self::Middle),
            other => Err(MyuriError::InvalidArgument(format!(
                "the button must be one of left, right, middle (got {other:?})"
            ))),
        }
    }
}

/// Validates an optional `Os` (no-op for valid enum variants).
pub fn check_os(_os: Option<Os>) {}

/// Modifier key used for select-all/copy/paste on `os`: `Meta` on macOS, `Control` elsewhere.
pub fn primary_modifier(os: Option<Os>) -> &'static str {
    match os {
        Some(Os::Macos) => "Meta",
        _ => "Control",
    }
}
