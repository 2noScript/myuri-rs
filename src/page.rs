use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::drivers::Driver;
use crate::exceptions::{MyuriError, Result};
use crate::math::{BoundingBox, ViewportSize};
use crate::page_functions::{on_selector_script, BOX_JS};

/// Reads viewport size in CSS pixels, robust to quirks mode and null-viewport fingerprints.
pub async fn viewport_of(driver: &dyn Driver) -> Result<ViewportSize> {
    let val = driver
        .evaluate(
            "() => ({ width: window.innerWidth || document.documentElement.clientWidth, height: window.innerHeight || document.documentElement.clientHeight })",
            None,
        )
        .await?;
    let vp: ViewportSize = serde_json::from_value(val).map_err(|_| {
        MyuriError::DriverError("could not read viewport size".into())
    })?;
    if vp.width <= 0.0 || vp.height <= 0.0 {
        return Err(MyuriError::DriverError(
            "could not read viewport size".into(),
        ));
    }
    Ok(vp)
}

/// Reads bounding box of `selector` in top-page viewport coordinates, or `None` if hidden/missing.
pub async fn bounding_box_of(driver: &dyn Driver, selector: &str) -> Result<Option<BoundingBox>> {
    let script = on_selector_script(BOX_JS);
    let val = driver
        .evaluate(&script, Some(json!({ "selector": selector, "arg": Value::Null })))
        .await?;
    if val.is_null() {
        return Ok(None);
    }
    serde_json::from_value(val)
        .map(Some)
        .map_err(|e| MyuriError::DriverError(e.to_string()))
}

/// Reads current page vertical scroll offset (`window.scrollY`).
pub async fn scroll_offset_of(driver: &dyn Driver) -> Result<f64> {
    let val = driver.evaluate("() => window.scrollY", None).await?;
    Ok(val.as_f64().unwrap_or(0.0))
}

/// Polls until `selector` resolves to a sized element, or `timeout_ms` elapses.
pub async fn wait_for_element(
    driver: &dyn Driver,
    selector: &str,
    timeout_ms: f64,
) -> Result<bool> {
    let deadline = Instant::now() + Duration::from_secs_f64((timeout_ms / 1000.0).max(0.0));
    loop {
        if driver.is_closed() {
            return Ok(false);
        }
        if bounding_box_of(driver, selector).await?.is_some() {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
