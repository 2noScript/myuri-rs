use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{json, Value};

use myuri_rs::actions::{validate_steps, wheel_scale_from, Step};
use myuri_rs::cursor::{create_cursor, CursorOptions};
use myuri_rs::drivers::{Driver, PressOptions};
use myuri_rs::math::Vector;
use myuri_rs::target_os::{MouseButton, Os};
use myuri_rs::typing::KeyActionType;
use myuri_rs::Result;

#[derive(Default)]
struct RecordingDriver {
    events: Mutex<Vec<String>>,
}

impl RecordingDriver {
    fn events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

#[async_trait]
impl Driver for RecordingDriver {
    async fn move_mouse(&self, x: f64, y: f64) -> Result<()> {
        self.events
            .lock()
            .unwrap()
            .push(format!("move:{:.1},{:.1}", x, y));
        Ok(())
    }

    async fn click(&self, button: MouseButton, click_count: usize, _hold_ms: f64) -> Result<()> {
        self.events
            .lock()
            .unwrap()
            .push(format!("click:{}:{}", button.as_str(), click_count));
        Ok(())
    }

    async fn down(&self, key: &str) -> Result<()> {
        self.events.lock().unwrap().push(format!("down:{key}"));
        Ok(())
    }

    async fn up(&self, key: &str) -> Result<()> {
        self.events.lock().unwrap().push(format!("up:{key}"));
        Ok(())
    }

    async fn wheel(&self, dx: f64, dy: f64) -> Result<()> {
        self.events
            .lock()
            .unwrap()
            .push(format!("wheel:{:.1},{:.1}", dx, dy));
        Ok(())
    }

    async fn press(&self, key: &str, _options: Option<PressOptions>) -> Result<()> {
        self.events.lock().unwrap().push(format!("press:{key}"));
        Ok(())
    }

    async fn insert_text(&self, text: &str) -> Result<()> {
        self.events.lock().unwrap().push(format!("insert:{text}"));
        Ok(())
    }

    async fn evaluate(&self, script: &str, _arg: Option<Value>) -> Result<Value> {
        if script.contains("chosenDelta") {
            return Ok(json!({
                "inView": true,
                "deltaY": 0.0,
                "hoverX": 500.0,
                "hoverY": 350.0,
                "isPage": true,
                "region": { "x": 100.0, "y": 100.0, "width": 120.0, "height": 40.0 },
                "box": { "x": 100.0, "y": 100.0, "width": 120.0, "height": 40.0 },
                "borders": [0.0, 0.0]
            }));
        }
        if script.contains("innerWidth") {
            return Ok(json!({ "width": 1000.0, "height": 700.0 }));
        }
        if script.contains("scrollY") {
            return Ok(json!(0.0));
        }
        if script.contains("() => true") {
            return Ok(json!(true));
        }
        Ok(Value::Null)
    }
}

#[test]
fn validate_steps_rejects_backwards_time() {
    let ok = validate_steps(&[
        Step::Move {
            x: 10.0,
            y: 20.0,
            t: 0.0,
        },
        Step::Key {
            key: "a".into(),
            action: KeyActionType::Down,
            t: 5.0,
        },
    ]);
    assert!(ok.is_ok());

    let err = validate_steps(&[
        Step::Move {
            x: 10.0,
            y: 20.0,
            t: 10.0,
        },
        Step::Move {
            x: 11.0,
            y: 21.0,
            t: 5.0,
        },
    ]);
    assert!(err.is_err());
}

#[test]
fn wheel_scale_from_clamps_range() {
    assert_eq!(wheel_scale_from(5.0, 5.0), None);
    assert_eq!(wheel_scale_from(100.0, -50.0), None);
    assert_eq!(wheel_scale_from(100.0, 100.0), Some(1.0));
    assert_eq!(wheel_scale_from(1000.0, 50.0), Some(3.0));
    assert_eq!(wheel_scale_from(20.0, 1000.0), Some(0.3));
}

#[tokio::test]
async fn cursor_move_click_and_type_play_through_driver() {
    let driver = Arc::new(RecordingDriver::default());
    let mut cursor = create_cursor(
        driver.clone(),
        CursorOptions {
            os: Some(Os::Macos),
            seed: Some(1234),
            start: Some(Vector::new(50.0, 50.0)),
            ..Default::default()
        },
    )
    .unwrap();
    cursor.actions.settings.typing.wpm = 300.0;

    cursor.click(Some("#btn"), None).await.unwrap();
    cursor.type_text("ok", None, None).await.unwrap();

    let events = driver.events();
    assert!(events.iter().any(|e| e.starts_with("move:")));
    assert!(events.iter().any(|e| e == "click:left:1"));
    assert!(events.iter().any(|e| e == "down:o"));
    assert!(events.iter().any(|e| e == "up:o"));
    assert!(events.iter().any(|e| e == "down:k"));
    assert!(events.iter().any(|e| e == "up:k"));
}
