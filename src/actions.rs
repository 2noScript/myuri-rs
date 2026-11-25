use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::exceptions::{MyuriError, Result};
use crate::geometry::{get_random_box_point, should_overshoot};
use crate::math::{point_in_box, BoundingBox, Vector};
use crate::options::ResolvedBoxOptions;
use crate::page_functions::HIT_TEST_JS;
use crate::plan::{
    move_with, plan_click, plan_point, plan_press, plan_type, wheel_steps, wheel_style_for,
    MoveSpec, PlanTypeOptions, PlannedKeyStep, PlannedWheelStep, WheelSpec, CLICK_RUN_MS,
};
use crate::rng::Rng;
use crate::scroll_plan::{RawScrollPlan, ScrollPlan, ScrollPlanArgs, SCROLL_EDGE, SCROLL_PAD, SCROLL_PLAN_JS};
use crate::settings::{resolve_settings, MyuriSettings, ScrollDevice, SettingsOverride};
use crate::target_os::{MouseButton, Os};
use crate::typing::timing::KeyActionType;

pub const PARTS: [&str; 8] = [
    "move", "press", "point", "reaches", "intoView", "wheel", "type", "key",
];
pub const RETRY_MS: [u64; 5] = [0, 20, 100, 100, 500];
pub const SCROLL_PASSES: usize = 3;
pub const LANDING_MS: (u64, u64) = (50, 1000);
pub const WHEEL_PROBE_MIN_PX: f64 = 400.0;
pub const WHEEL_PROBE_SHARE: f64 = 0.5;
pub const WHEEL_SCALE_RANGE: (f64, f64) = (0.3, 3.0);

/// Infers wheel-px-per-page-px scale factor from observed page movement.
pub fn wheel_scale_from(sent: f64, moved: f64) -> Option<f64> {
    if !(sent.abs() > 10.0 && moved.abs() > 10.0) || sent.signum() != moved.signum() {
        return None;
    }
    let ratio = ((sent / moved) * 100.0).round() / 100.0;
    Some(ratio.clamp(WHEEL_SCALE_RANGE.0, WHEEL_SCALE_RANGE.1))
}

/// Timeline step played by `Actions::play`.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Move { x: f64, y: f64, t: f64 },
    Down { button: MouseButton, t: f64 },
    Up { button: MouseButton, t: f64 },
    Wheel { dx: f64, dy: f64, t: f64 },
    Key { key: String, action: KeyActionType, t: f64 },
    Text { text: String, t: f64 },
}

impl Step {
    pub fn t(&self) -> f64 {
        match self {
            Self::Move { t, .. }
            | Self::Down { t, .. }
            | Self::Up { t, .. }
            | Self::Wheel { t, .. }
            | Self::Key { t, .. }
            | Self::Text { t, .. } => *t,
        }
    }
}

/// Validates that a timeline's step timestamps are finite, >= 0, and non-decreasing.
pub fn validate_steps(steps: &[Step]) -> Result<()> {
    let mut last = 0.0_f64;
    for (i, step) in steps.iter().enumerate() {
        let t = step.t();
        if !t.is_finite() || t < 0.0 {
            return Err(MyuriError::InvalidArgument(format!(
                "play() step {i}: t must be a finite number of milliseconds >= 0"
            )));
        }
        if t < last {
            return Err(MyuriError::InvalidArgument(format!(
                "play() step {i}: t goes back in time ({t} after {last})"
            )));
        }
        if let Step::Move { x, y, .. } = step {
            if !x.is_finite() || !y.is_finite() {
                return Err(MyuriError::InvalidArgument(format!(
                    "play() step {i}: move takes finite numbers"
                )));
            }
        }
        if let Step::Wheel { dx, dy, .. } = step {
            if !dx.is_finite() || !dy.is_finite() {
                return Err(MyuriError::InvalidArgument(format!(
                    "play() step {i}: wheel takes finite numbers"
                )));
            }
        }
        if let Step::Key { key, action, .. } = step {
            if key.is_empty() || !matches!(action, KeyActionType::Down | KeyActionType::Up) {
                return Err(MyuriError::InvalidArgument(format!(
                    "play() step {i}: key takes a non-empty key name and Down/Up"
                )));
            }
        }
        last = t;
    }
    Ok(())
}

fn key_steps(steps: Vec<PlannedKeyStep>) -> Vec<Step> {
    steps
        .into_iter()
        .map(|s| match s.action {
            KeyActionType::Insert => Step::Text {
                text: s.key,
                t: s.at,
            },
            action => Step::Key {
                key: s.key,
                action,
                t: s.at,
            },
        })
        .collect()
}

/// Target abstraction passed to `Actions` (`"selector"` in a browser driver, or custom host handle).
#[derive(Debug, Clone)]
pub struct Target {
    pub selector: String,
}

impl Target {
    pub fn new(selector: impl Into<String>) -> Self {
        Self {
            selector: selector.into(),
        }
    }
}

/// Execution host required by `Actions`.
#[async_trait]
pub trait Host: Send + Sync {
    async fn move_mouse(&self, x: f64, y: f64) -> Result<()>;
    async fn click(&self, button: MouseButton, click_count: usize, hold_ms: f64) -> Result<()>;
    async fn down(&self, button: MouseButton) -> Result<()>;
    async fn up(&self, button: MouseButton) -> Result<()>;
    async fn wheel(&self, dx: f64, dy: f64) -> Result<()>;
    async fn key_down(&self, key: &str) -> Result<()>;
    async fn key_up(&self, key: &str) -> Result<()>;
    async fn insert_text(&self, text: &str) -> Result<()>;
    async fn evaluate(&self, target: &Target, fn_source: &str, arg: Value) -> Result<Value>;
    async fn offset(&self, target: &Target) -> Result<Option<[f64; 2]>>;
    async fn viewport(&self) -> Result<[f64; 2]>;
    fn now(&self) -> f64;
    async fn sleep(&self, ms: f64) {
        if ms > 0.0 {
            tokio::time::sleep(Duration::from_secs_f64(ms / 1000.0)).await;
        }
    }
    async fn flush_wheels(&self) -> Result<()> {
        Ok(())
    }
    async fn scroll_offset(&self) -> Result<Option<f64>> {
        Ok(None)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ActionsOptions {
    pub os: Option<Os>,
    pub device: Option<ScrollDevice>,
    pub settings: Option<SettingsOverride>,
}

#[derive(Debug, Clone)]
pub struct ActionClickOptions {
    pub button: MouseButton,
    pub click_count: usize,
    pub modifiers: Vec<String>,
    pub position: Option<Vector>,
    pub force: bool,
    pub press: bool,
    pub timeout: Option<f64>,
    pub attempts: Option<usize>,
}

impl Default for ActionClickOptions {
    fn default() -> Self {
        Self {
            button: MouseButton::Left,
            click_count: 1,
            modifiers: Vec::new(),
            position: None,
            force: false,
            press: true,
            timeout: None,
            attempts: None,
        }
    }
}

pub struct Actions {
    pub host: Arc<dyn Host>,
    pub rng: Rng,
    pub os: Option<Os>,
    pub settings: MyuriSettings,
    pub device: ScrollDevice,
    pub move_speed: f64,
    pub overshoot_threshold: f64,
    pub hold_ms: Option<f64>,
    pub finish_position: f64,
    pub speed_factor: f64,
    pub speed_irregularity: f64,
    pub scroll_passes: usize,
    pub wheel_scale: Option<f64>,
    pub wheel_sent: Option<f64>,
    pub cursor: Option<Vector>,
    pub placed: bool,
    pub moved_at: f64,
    pub released_at: f64,
    pub box_options: Option<ResolvedBoxOptions>,
}

impl Actions {
    pub fn new(host: Arc<dyn Host>, rng: Rng, options: ActionsOptions) -> Self {
        let mut settings = resolve_settings(options.settings.as_ref());
        settings.scroll.wheel.style = wheel_style_for(options.os, options.settings.as_ref());
        let device = options.device.unwrap_or(settings.scroll.device);
        let move_speed = settings.mouse.speed_factor;
        let overshoot_threshold = settings.overshoot.threshold;
        let finish_position = settings.scroll.finish_position;
        let speed_factor = settings.scroll.speed_factor;
        let speed_irregularity = settings.scroll.realism.speed_irregularity;
        Self {
            host,
            rng,
            os: options.os,
            settings,
            device,
            move_speed,
            overshoot_threshold,
            hold_ms: None,
            finish_position,
            speed_factor,
            speed_irregularity,
            scroll_passes: SCROLL_PASSES,
            wheel_scale: None,
            wheel_sent: None,
            cursor: None,
            placed: false,
            moved_at: f64::NEG_INFINITY,
            released_at: f64::NEG_INFINITY,
            box_options: None,
        }
    }

    pub fn moved(&mut self, x: f64, y: f64) {
        self.cursor = Some(Vector { x, y });
        self.placed = true;
        self.moved_at = self.host.now();
    }

    pub fn since_move(&self) -> f64 {
        self.host.now() - self.moved_at
    }

    async fn wait(&self, ms: f64) {
        if ms > 0.0 {
            self.host.sleep(ms).await;
        }
    }

    pub async fn here(&mut self) -> Result<Vector> {
        if let Some(pos) = self.cursor {
            return Ok(pos);
        }
        let [width, height] = self.host.viewport().await?;
        let pos = Vector {
            x: self.rng.next() * width,
            y: self.rng.next() * height,
        };
        self.cursor = Some(pos);
        Ok(pos)
    }

    pub async fn play(&mut self, steps: &[Step]) -> Result<()> {
        validate_steps(steps)?;
        let start = self.host.now();
        for step in steps {
            loop {
                let remaining = start + step.t() - self.host.now();
                if remaining <= 0.0 {
                    break;
                }
                self.host.sleep(remaining).await;
            }
            match step {
                Step::Move { x, y, .. } => {
                    self.host.move_mouse(*x, *y).await?;
                    self.moved(*x, *y);
                }
                Step::Down { button, .. } => self.host.down(*button).await?,
                Step::Up { button, .. } => {
                    self.host.up(*button).await?;
                    self.released_at = self.host.now();
                }
                Step::Wheel { dx, dy, .. } => self.host.wheel(*dx, *dy).await?,
                Step::Key { key, action, .. } => match action {
                    KeyActionType::Down => self.host.key_down(key).await?,
                    KeyActionType::Up => self.host.key_up(key).await?,
                    KeyActionType::Insert => {}
                },
                Step::Text { text, .. } => self.host.insert_text(text).await?,
            }
        }
        Ok(())
    }

    pub async fn move_to(&mut self, x: f64, y: f64) -> Result<()> {
        let [width, height] = self.host.viewport().await?;
        let settle = |p_x: f64, p_y: f64| Vector {
            x: p_x.round().clamp(0.0, width - 1.0),
            y: p_y.round().clamp(0.0, height - 1.0),
        };
        let end = settle(x, y);
        if self.placed && self.cursor.is_some_and(|c| c.x == end.x && c.y == end.y) {
            return Ok(());
        }
        let start = self.here().await?;
        let spec = MoveSpec {
            move_speed: Some(self.move_speed),
            overshoot: should_overshoot(start, end, self.overshoot_threshold),
            settings: self.settings.overshoot.clone(),
        };
        let points = move_with(start, end, &self.rng, &spec, |p| settle(p.x, p.y));
        let mut steps = Vec::with_capacity(points.len());
        let mut last = if self.placed { self.cursor } else { None };
        for p in points {
            let pos = settle(p.x, p.y);
            if last.is_some_and(|l| pos.x == l.x && pos.y == l.y)
                && self.os != Some(Os::Macos)
            {
                continue;
            }
            steps.push(Step::Move {
                x: pos.x,
                y: pos.y,
                t: p.t,
            });
            last = Some(pos);
        }
        self.play(&steps).await
    }

    pub async fn press(
        &mut self,
        button: MouseButton,
        click_count: usize,
        modifiers: &[String],
    ) -> Result<()> {
        for key in modifiers {
            self.host.key_down(key).await?;
        }

        let now = self.host.now();
        let resting = self.moved_at < self.released_at;
        let since = resting.then_some(now - self.released_at);
        let planned = plan_click(&self.rng, click_count, since);
        let wait_ms = if resting {
            planned.dwell_ms
        } else {
            self.moved_at + planned.dwell_ms - now
        };
        self.wait(wait_ms).await;

        let first = match since {
            Some(s) if s < CLICK_RUN_MS => 2,
            _ => 1,
        };
        for (i, &(down, up)) in planned.presses.iter().enumerate() {
            if i > 0 {
                self.wait(down - planned.presses[i - 1].1).await;
            }
            self.host
                .click(button, first + i, self.hold_ms.unwrap_or(up - down))
                .await?;
        }
        self.released_at = self.host.now();

        for key in modifiers.iter().rev() {
            self.host.key_up(key).await?;
        }
        Ok(())
    }

    pub async fn click_at(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
        click_count: usize,
    ) -> Result<()> {
        self.move_to(x, y).await?;
        self.press(button, click_count, &[]).await
    }

    pub fn wheel_spec(&self) -> WheelSpec {
        WheelSpec {
            device: self.device,
            wheel: self.settings.scroll.wheel.clone(),
            speed_irregularity: self.speed_irregularity,
            speed_factor: self.speed_factor,
            stroke_scale: self.wheel_scale.unwrap_or(1.0),
        }
    }

    pub fn wheel_plan(&self, dx: f64, dy: f64) -> (Vec<Step>, f64) {
        let m = self.wheel_scale.unwrap_or(1.0);
        let raw_steps = wheel_steps(dx * m, dy * m, &self.rng, self.os, &self.wheel_spec());
        let mut sent = 0.0_f64;
        let steps: Vec<Step> = raw_steps
            .into_iter()
            .map(|s| match s {
                PlannedWheelStep::Wheel { dx, dy, at } => {
                    sent += dy;
                    Step::Wheel { dx, dy, t: at }
                }
                PlannedWheelStep::Key { key, action, at } => Step::Key {
                    key,
                    action,
                    t: at,
                },
            })
            .collect();
        (steps, sent)
    }

    pub async fn wheel(&mut self, dx: f64, dy: f64) -> Result<()> {
        let (steps, sent) = self.wheel_plan(dx, dy);
        let pre = self.host.scroll_offset().await?;
        self.play(&steps).await?;
        self.host.flush_wheels().await?;
        self.wheel_sent = Some(sent);
        if let Some(pre_y) = pre {
            if self.wheel_scale.is_none() {
                let settled_y = self.settle_scroll(pre_y).await?;
                self.wheel_scale = wheel_scale_from(sent, settled_y - pre_y);
            }
        }
        Ok(())
    }

    async fn settle_scroll(&self, previous: f64) -> Result<f64> {
        let mut last = previous;
        let mut stable = 0usize;
        let deadline = self.host.now() + 600.0;
        loop {
            self.wait(40.0).await;
            let cur = self.host.scroll_offset().await?.unwrap_or(last);
            if (cur - last).abs() < f64::EPSILON {
                stable += 1;
            } else {
                stable = 0;
                last = cur;
            }
            if stable >= 3 || self.host.now() >= deadline {
                return Ok(cur);
            }
        }
    }

    pub async fn type_text(&mut self, text: &str) -> Result<()> {
        let steps = plan_type(
            text,
            &self.rng,
            &PlanTypeOptions {
                os: self.os,
                wpm: Some(self.settings.typing.wpm),
                mistake_multiplier: Some(self.settings.typing.mistake_multiplier),
                pauses: Some(self.settings.typing.pauses.clone()),
            },
        )?;
        self.play(&key_steps(steps)).await
    }

    pub async fn key(&mut self, key: &str) -> Result<()> {
        let steps = plan_press(
            key,
            &self.rng,
            &PlanTypeOptions {
                os: self.os,
                wpm: Some(self.settings.typing.wpm),
                mistake_multiplier: None,
                pauses: Some(self.settings.typing.pauses.clone()),
            },
        );
        self.play(&key_steps(steps)).await
    }

    pub async fn measure(&mut self, target: &Target) -> Result<Option<ScrollPlan>> {
        let Some([dx, dy]) = self.host.offset(target).await? else {
            return Ok(None);
        };
        let here = self.here().await?;
        let args = serde_json::to_value(ScrollPlanArgs {
            finish_position: self.finish_position,
            pad: SCROLL_PAD,
            edge: SCROLL_EDGE,
            from_x: here.x - dx,
            from_y: here.y - dy,
        })
        .map_err(|e| MyuriError::DriverError(e.to_string()))?;

        let raw = self.host.evaluate(target, SCROLL_PLAN_JS, args).await?;
        if raw.is_null() {
            return Ok(None);
        }
        let parsed: RawScrollPlan =
            serde_json::from_value(raw).map_err(|e| MyuriError::DriverError(e.to_string()))?;
        let shift = |b: BoundingBox| BoundingBox {
            x: b.x + dx,
            y: b.y + dy,
            width: b.width,
            height: b.height,
        };
        Ok(Some(ScrollPlan {
            in_view: parsed.in_view,
            delta_y: parsed.delta_y,
            hover_x: parsed.hover_x + dx,
            hover_y: parsed.hover_y + dy,
            is_page: parsed.is_page,
            region: parsed.region.map(shift),
            box_rect: Some(shift(parsed.box_rect)),
            borders: parsed.borders,
        }))
    }

    async fn choose_point(&mut self, region: BoundingBox, attempt: usize) -> Result<Vector> {
        if let Some(opts) = self.box_options.clone() {
            let here = self.cursor;
            if let Some(h) = here {
                if self.placed
                    && point_in_box(h, region)
                    && !crate::geometry::is_point_excluded(h, &opts.exclude)
                {
                    return Ok(h);
                }
            }
            return get_random_box_point(region, Some(&opts.padding), here, &opts.exclude, &self.rng);
        }
        let here = self.here().await?;
        if attempt == 0 {
            plan_point(region, &self.rng, Some(here))
        } else {
            get_random_box_point(region, None, Some(here), &[], &self.rng)
        }
    }

    async fn reaches(
        &self,
        target: &Target,
        x: f64,
        y: f64,
        pressing: bool,
    ) -> Result<Option<String>> {
        let Some([ox, oy]) = self.host.offset(target).await? else {
            return Ok(Some("element is not visible".into()));
        };
        let res = self
            .host
            .evaluate(
                target,
                HIT_TEST_JS,
                json!({ "x": x - ox, "y": y - oy, "pressing": pressing }),
            )
            .await?;
        Ok(res.as_str().map(ToOwned::to_owned))
    }

    async fn landed(&mut self, target: &Target) -> Result<()> {
        let (every, limit) = LANDING_MS;
        let deadline = self.host.now() + (limit as f64);
        let mut last = self.measure(target).await?;
        while self.host.now() < deadline {
            self.wait(every as f64).await;
            let cur = self.measure(target).await?;
            if cur.as_ref().and_then(|p| p.box_rect) == last.as_ref().and_then(|p| p.box_rect) {
                return Ok(());
            }
            last = cur;
        }
        Ok(())
    }

    pub async fn into_view(&mut self, target: &Target) -> Result<()> {
        let mut probed = false;
        let mut i = 0usize;
        while i < self.scroll_passes {
            let Some(plan) = self.measure(target).await? else {
                return Ok(());
            };
            if plan.in_view || plan.delta_y.abs() < 1.0 {
                return Ok(());
            }
            self.move_to(plan.hover_x, plan.hover_y).await?;
            let probe =
                !probed && self.wheel_scale.is_none() && plan.delta_y.abs() > WHEEL_PROBE_MIN_PX;
            probed |= probe;
            self.wheel_sent = None;
            let step_dy = if probe {
                plan.delta_y * WHEEL_PROBE_SHARE
            } else {
                plan.delta_y
            };
            self.wheel(0.0, step_dy).await?;
            self.landed(target).await?;
            if self.wheel_scale.is_none() {
                if let (Some(sent), Some(after)) = (self.wheel_sent, self.measure(target).await?) {
                    if after.delta_y.abs() >= 1.0 {
                        if let (Some(b1), Some(b2)) = (plan.box_rect, after.box_rect) {
                            self.wheel_scale = wheel_scale_from(sent, b1.y - b2.y);
                        }
                    }
                }
            }
            if !probe {
                i += 1;
            }
        }
        Ok(())
    }

    pub async fn click(&mut self, target: &Target, options: ActionClickOptions) -> Result<()> {
        let deadline = options
            .timeout
            .map(|t| self.host.now() + t)
            .unwrap_or(f64::INFINITY);
        let mut attempt = 0usize;
        loop {
            let res: Result<()> = async {
                self.into_view(target).await?;
                let plan = self
                    .measure(target)
                    .await?
                    .ok_or_else(|| MyuriError::Missed("element is not visible".into()))?;
                let p = if let Some(pos) = options.position {
                    let b = plan
                        .box_rect
                        .ok_or_else(|| MyuriError::Missed("element is not visible".into()))?;
                    Vector {
                        x: b.x + plan.borders[0] + pos.x,
                        y: b.y + plan.borders[1] + pos.y,
                    }
                } else {
                    let region = plan.region.ok_or_else(|| {
                        MyuriError::Missed("element is outside of the viewport or covered".into())
                    })?;
                    self.choose_point(region, attempt).await?
                };
                self.move_to(p.x, p.y).await?;
                if !options.force {
                    let at = if self.placed {
                        self.cursor.unwrap_or(p)
                    } else {
                        p
                    };
                    if let Some(reason) = self.reaches(target, at.x, at.y, options.press).await? {
                        return Err(MyuriError::Missed(reason));
                    }
                }
                if options.press {
                    self.press(options.button, options.click_count, &options.modifiers)
                        .await?;
                }
                Ok(())
            }
            .await;

            match res {
                Ok(()) => return Ok(()),
                Err(e @ MyuriError::Missed(_)) => {
                    let exhausted = options
                        .attempts
                        .is_some_and(|max| attempt + 1 >= max);
                    if exhausted || self.host.now() >= deadline {
                        return Err(e);
                    }
                    let wait = RETRY_MS[attempt.min(RETRY_MS.len() - 1)];
                    self.wait(wait as f64).await;
                    attempt += 1;
                }
                Err(other) => return Err(other),
            }
        }
    }
}
