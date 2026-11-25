use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::exceptions::{MyuriError, Result};
use crate::target_os::{primary_modifier, MouseButton, Os};

#[derive(Debug, Clone, Default)]
pub struct PressOptions {
    pub delay: Option<f64>,
}

/// Low-level browser input & evaluation interface required by `Cursor`.
#[async_trait]
pub trait Driver: Send + Sync {
    async fn move_mouse(&self, x: f64, y: f64) -> Result<()>;
    async fn click(&self, button: MouseButton, click_count: usize, hold_ms: f64) -> Result<()>;
    async fn down(&self, key: &str) -> Result<()>;
    async fn up(&self, key: &str) -> Result<()>;
    async fn wheel(&self, delta_x: f64, delta_y: f64) -> Result<()>;
    async fn press(&self, key: &str, options: Option<PressOptions>) -> Result<()>;
    async fn evaluate(&self, script: &str, arg: Option<Value>) -> Result<Value>;

    async fn mouse_down(&self, _button: MouseButton) -> Result<()> {
        Err(MyuriError::DriverError(
            "this driver has no mouse_down(), which drag needs".into(),
        ))
    }

    async fn mouse_up(&self, _button: MouseButton) -> Result<()> {
        Err(MyuriError::DriverError(
            "this driver has no mouse_up(), which drag needs".into(),
        ))
    }

    async fn insert_text(&self, text: &str) -> Result<()>;

    async fn set_clipboard(&self, text: &str) -> Result<()> {
        let script = r#"async (t) => {
            if (navigator.clipboard?.writeText) {
                await navigator.clipboard.writeText(t);
                return;
            }
            const ta = document.createElement('textarea');
            ta.value = t;
            ta.style.position = 'fixed';
            ta.style.left = '-9999px';
            document.body.appendChild(ta);
            ta.focus();
            ta.select();
            document.execCommand('copy');
            document.body.removeChild(ta);
        }"#;
        let _ = self.evaluate(script, Some(Value::String(text.to_owned()))).await?;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        false
    }

    async fn dispose(&self) -> Result<()> {
        Ok(())
    }
}

/// Wrapper implementing fallback `insert_text` via clipboard + platform paste and non-ASCII `press` delegation.
pub struct DefinedDriver {
    inner: Arc<dyn Driver>,
    os: Option<Os>,
    has_native_insert: bool,
}

impl DefinedDriver {
    pub fn new(inner: Arc<dyn Driver>, os: Option<Os>, has_native_insert: bool) -> Self {
        Self {
            inner,
            os,
            has_native_insert,
        }
    }
}

fn is_single_non_ascii_printable(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(c) = chars.next() else {
        return false;
    };
    chars.next().is_none() && !('\x20'..='\x7e').contains(&c)
}

#[async_trait]
impl Driver for DefinedDriver {
    async fn move_mouse(&self, x: f64, y: f64) -> Result<()> {
        self.inner.move_mouse(x, y).await
    }

    async fn click(&self, button: MouseButton, click_count: usize, hold_ms: f64) -> Result<()> {
        self.inner.click(button, click_count, hold_ms).await
    }

    async fn down(&self, key: &str) -> Result<()> {
        self.inner.down(key).await
    }

    async fn up(&self, key: &str) -> Result<()> {
        self.inner.up(key).await
    }

    async fn wheel(&self, delta_x: f64, delta_y: f64) -> Result<()> {
        self.inner.wheel(delta_x, delta_y).await
    }

    async fn press(&self, key: &str, options: Option<PressOptions>) -> Result<()> {
        if is_single_non_ascii_printable(key) {
            return self.insert_text(key).await;
        }
        self.inner.press(key, options).await
    }

    async fn evaluate(&self, script: &str, arg: Option<Value>) -> Result<Value> {
        self.inner.evaluate(script, arg).await
    }

    async fn mouse_down(&self, button: MouseButton) -> Result<()> {
        self.inner.mouse_down(button).await
    }

    async fn mouse_up(&self, button: MouseButton) -> Result<()> {
        self.inner.mouse_up(button).await
    }

    async fn insert_text(&self, text: &str) -> Result<()> {
        if self.has_native_insert {
            return self.inner.insert_text(text).await;
        }
        self.inner.set_clipboard(text).await?;
        let modifier = match self.os {
            Some(os) => primary_modifier(Some(os)),
            None => {
                let is_mac = self
                    .inner
                    .evaluate(
                        "() => /Mac|iPhone|iPad/i.test(navigator.platform || navigator.userAgent)",
                        None,
                    )
                    .await?
                    .as_bool()
                    .unwrap_or(false);
                if is_mac {
                    "Meta"
                } else {
                    "Control"
                }
            }
        };
        self.inner.down(modifier).await?;
        self.inner.press("v", None).await?;
        self.inner.up(modifier).await
    }

    async fn set_clipboard(&self, text: &str) -> Result<()> {
        self.inner.set_clipboard(text).await
    }

    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    async fn dispose(&self) -> Result<()> {
        self.inner.dispose().await
    }
}

pub fn define_driver(
    driver: Arc<dyn Driver>,
    os: Option<Os>,
    has_native_insert: bool,
) -> DefinedDriver {
    DefinedDriver::new(driver, os, has_native_insert)
}
