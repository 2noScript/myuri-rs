use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::exceptions::{MyuriError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrollDevice {
    Trackpad,
    Mouse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WheelStyle {
    Notched,
    Macos,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MouseSettings {
    pub speed_factor: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DwellSettings {
    pub min_ms: f64,
    pub max_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OvershootSettings {
    pub radius: f64,
    pub threshold: f64,
    pub correction_speed_factor: f64,
    pub dwell: DwellSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollRealismSettings {
    pub speed_irregularity: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelSettings {
    pub style: WheelStyle,
    pub px_per_notch: f64,
    pub px_per_line: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollSettings {
    pub speed_factor: f64,
    pub finish_position: f64,
    pub realism: ScrollRealismSettings,
    pub device: ScrollDevice,
    pub wheel: WheelSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseSettings {
    pub rate: f64,
    pub length: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypingSettings {
    pub wpm: f64,
    pub mistake_multiplier: f64,
    pub pauses: PauseSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MyuriSettings {
    pub variation: f64,
    pub mouse: MouseSettings,
    pub overshoot: OvershootSettings,
    pub scroll: ScrollSettings,
    pub typing: TypingSettings,
}

impl Default for MyuriSettings {
    fn default() -> Self {
        default_settings()
    }
}

/// Partial override structs matching `Partial<MyuriSettings>` in JS.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MouseSettingsOverride {
    pub speed_factor: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DwellSettingsOverride {
    pub min_ms: Option<f64>,
    pub max_ms: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OvershootSettingsOverride {
    pub radius: Option<f64>,
    pub threshold: Option<f64>,
    pub correction_speed_factor: Option<f64>,
    pub dwell: Option<DwellSettingsOverride>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollRealismOverride {
    pub speed_irregularity: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelSettingsOverride {
    pub style: Option<WheelStyle>,
    pub px_per_notch: Option<f64>,
    pub px_per_line: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollSettingsOverride {
    pub speed_factor: Option<f64>,
    pub finish_position: Option<f64>,
    pub realism: Option<ScrollRealismOverride>,
    pub device: Option<ScrollDevice>,
    pub wheel: Option<WheelSettingsOverride>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseSettingsOverride {
    pub rate: Option<f64>,
    pub length: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypingSettingsOverride {
    pub wpm: Option<f64>,
    pub mistake_multiplier: Option<f64>,
    pub pauses: Option<PauseSettingsOverride>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsOverride {
    pub variation: Option<f64>,
    pub mouse: Option<MouseSettingsOverride>,
    pub overshoot: Option<OvershootSettingsOverride>,
    pub scroll: Option<ScrollSettingsOverride>,
    pub typing: Option<TypingSettingsOverride>,
}

pub fn default_settings() -> MyuriSettings {
    MyuriSettings {
        variation: 0.2,
        mouse: MouseSettings { speed_factor: 1.0 },
        overshoot: OvershootSettings {
            radius: 120.0,
            threshold: 500.0,
            correction_speed_factor: 1.0,
            dwell: DwellSettings {
                min_ms: 20.0,
                max_ms: 60.0,
            },
        },
        scroll: ScrollSettings {
            speed_factor: 1.0,
            finish_position: 0.5,
            realism: ScrollRealismSettings {
                speed_irregularity: 0.08,
            },
            device: ScrollDevice::Trackpad,
            wheel: WheelSettings {
                style: WheelStyle::Notched,
                px_per_notch: 100.0,
                px_per_line: 16.0,
            },
        },
        typing: TypingSettings {
            wpm: 80.0,
            mistake_multiplier: 1.0,
            pauses: PauseSettings {
                rate: 1.0,
                length: 1.0,
            },
        },
    }
}

/// Resolves a full `MyuriSettings` from an optional partial `SettingsOverride`.
pub fn resolve_settings(override_opts: Option<&SettingsOverride>) -> MyuriSettings {
    let mut out = default_settings();
    let Some(ov) = override_opts else {
        return out;
    };
    if let Some(v) = ov.variation {
        out.variation = v;
    }
    if let Some(m) = &ov.mouse {
        if let Some(sf) = m.speed_factor {
            out.mouse.speed_factor = sf;
        }
    }
    if let Some(o) = &ov.overshoot {
        if let Some(r) = o.radius {
            out.overshoot.radius = r;
        }
        if let Some(t) = o.threshold {
            out.overshoot.threshold = t;
        }
        if let Some(c) = o.correction_speed_factor {
            out.overshoot.correction_speed_factor = c;
        }
        if let Some(d) = &o.dwell {
            if let Some(min_ms) = d.min_ms {
                out.overshoot.dwell.min_ms = min_ms;
            }
            if let Some(max_ms) = d.max_ms {
                out.overshoot.dwell.max_ms = max_ms;
            }
        }
    }
    if let Some(s) = &ov.scroll {
        if let Some(sf) = s.speed_factor {
            out.scroll.speed_factor = sf;
        }
        if let Some(fp) = s.finish_position {
            out.scroll.finish_position = fp;
        }
        if let Some(r) = &s.realism {
            if let Some(si) = r.speed_irregularity {
                out.scroll.realism.speed_irregularity = si;
            }
        }
        if let Some(d) = s.device {
            out.scroll.device = d;
        }
        if let Some(w) = &s.wheel {
            if let Some(style) = w.style {
                out.scroll.wheel.style = style;
            }
            if let Some(p) = w.px_per_notch {
                out.scroll.wheel.px_per_notch = p;
            }
            if let Some(p) = w.px_per_line {
                out.scroll.wheel.px_per_line = p;
            }
        }
    }
    if let Some(t) = &ov.typing {
        if let Some(wpm) = t.wpm {
            out.typing.wpm = wpm;
        }
        if let Some(mm) = t.mistake_multiplier {
            out.typing.mistake_multiplier = mm;
        }
        if let Some(p) = &t.pauses {
            if let Some(rate) = p.rate {
                out.typing.pauses.rate = rate;
            }
            if let Some(len) = p.length {
                out.typing.pauses.length = len;
            }
        }
    }
    out
}

/// Validates and merges a dynamic JSON settings override onto `default_settings()`.
/// Rejects unknown keys with `MyuriError::UnknownSetting`.
pub fn resolve_settings_value(override_val: Option<&Value>) -> Result<MyuriSettings> {
    let mut base = serde_json::to_value(default_settings())
        .map_err(|e| MyuriError::InvalidArgument(e.to_string()))?;
    if let Some(v) = override_val {
        if !v.is_null() {
            deep_merge_json(&mut base, v, "")?;
        }
    }
    serde_json::from_value(base).map_err(|e| MyuriError::InvalidArgument(e.to_string()))
}

fn deep_merge_json(base: &mut Value, override_val: &Value, path: &str) -> Result<()> {
    let base_map = base
        .as_object_mut()
        .ok_or_else(|| MyuriError::InvalidArgument("settings base is not an object".into()))?;
    let ov_map = override_val
        .as_object()
        .ok_or_else(|| MyuriError::InvalidArgument("settings override must be an object".into()))?;

    for (key, ov) in ov_map {
        let Some(bv) = base_map.get_mut(key) else {
            return Err(MyuriError::UnknownSetting(format!("{path}{key}")));
        };
        if ov.is_null() {
            continue;
        }
        if ov.is_object() && bv.is_object() {
            deep_merge_json(bv, ov, &format!("{path}{key}."))?;
        } else {
            *bv = ov.clone();
        }
    }
    Ok(())
}
