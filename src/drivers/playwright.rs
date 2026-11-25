use std::time::Duration;

use async_trait::async_trait;
use playwright_rs::protocol::{
    KeyboardOptions, MouseButton as PwMouseButton, MouseOptions, Page,
};
use serde_json::Value;

use crate::exceptions::{MyuriError, Result};
use crate::target_os::MouseButton;

use super::types::{Driver, PressOptions};

fn map_button(button: MouseButton) -> PwMouseButton {
    match button {
        MouseButton::Left => PwMouseButton::Left,
        MouseButton::Right => PwMouseButton::Right,
        MouseButton::Middle => PwMouseButton::Middle,
    }
}

fn pw_err(err: playwright_rs::Error) -> MyuriError {
    MyuriError::DriverError(err.to_string())
}

/// Playwright `Page` adapter implementing `Driver` via `playwright-rs`.
#[derive(Clone)]
pub struct PlaywrightDriver {
    page: Page,
}

impl PlaywrightDriver {
    pub fn new(page: Page) -> Self {
        Self { page }
    }

    pub fn page(&self) -> &Page {
        &self.page
    }
}

pub fn playwright_driver(page: Page) -> PlaywrightDriver {
    PlaywrightDriver::new(page)
}

#[async_trait]
impl Driver for PlaywrightDriver {
    async fn move_mouse(&self, x: f64, y: f64) -> Result<()> {
        self.page.mouse().move_to(x, y, None).await.map_err(pw_err)
    }

    async fn click(&self, button: MouseButton, click_count: usize, hold_ms: f64) -> Result<()> {
        let opts = MouseOptions::builder()
            .button(map_button(button))
            .click_count(click_count as u32)
            .build();
        self.page
            .mouse()
            .down(opts.clone())
            .await
            .map_err(pw_err)?;
        if hold_ms > 0.0 {
            tokio::time::sleep(Duration::from_secs_f64(hold_ms / 1000.0)).await;
        }
        self.page.mouse().up(opts).await.map_err(pw_err)
    }

    async fn mouse_down(&self, button: MouseButton) -> Result<()> {
        let opts = MouseOptions::builder().button(map_button(button)).build();
        self.page.mouse().down(opts).await.map_err(pw_err)
    }

    async fn mouse_up(&self, button: MouseButton) -> Result<()> {
        let opts = MouseOptions::builder().button(map_button(button)).build();
        self.page.mouse().up(opts).await.map_err(pw_err)
    }

    async fn down(&self, key: &str) -> Result<()> {
        self.page.keyboard().down(key).await.map_err(pw_err)
    }

    async fn up(&self, key: &str) -> Result<()> {
        self.page.keyboard().up(key).await.map_err(pw_err)
    }

    async fn wheel(&self, delta_x: f64, delta_y: f64) -> Result<()> {
        self.page
            .mouse()
            .wheel(delta_x, delta_y)
            .await
            .map_err(pw_err)
    }

    async fn press(&self, key: &str, options: Option<PressOptions>) -> Result<()> {
        let kb_opts = options.and_then(|o| {
            o.delay
                .map(|d| KeyboardOptions::builder().delay(d).build())
        });
        self.page
            .keyboard()
            .press(key, kb_opts)
            .await
            .map_err(pw_err)
    }

    async fn insert_text(&self, text: &str) -> Result<()> {
        self.page
            .keyboard()
            .insert_text(text)
            .await
            .map_err(pw_err)
    }

    async fn evaluate(&self, script: &str, arg: Option<Value>) -> Result<Value> {
        self.page
            .evaluate::<Value, Value>(script, arg.as_ref())
            .await
            .map_err(pw_err)
    }

    fn is_closed(&self) -> bool {
        self.page.is_closed()
    }
}
