use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::actions::{ActionClickOptions, Actions, ActionsOptions, Host, Target};
use crate::drivers::{define_driver, DefinedDriver, Driver};
use crate::exceptions::{MyuriError, Result};
use crate::geometry::{
    get_better_starting_position, get_random_box_point, is_point_excluded,
};
use crate::math::{point_in_box, BoundingBox, Vector, ViewportSize};
use crate::motion::click_model::{
    drag_carry_speed, drag_end_ms, drag_start_ms, pre_click_dwell_ms,
};
use crate::options::{
    ClickOptions, CursorDefaults, DragOptions, MoveOptions, MoveToOptions, ResolvedBoxOptions,
    ScrollOptions, TypeOptions,
};
use crate::page::{bounding_box_of, scroll_offset_of, viewport_of, wait_for_element};
use crate::page_functions::{on_selector_script, FOCUSED_FIELD_JS};
use crate::profile::generate_profile;
use crate::rng::{generate_random_seed, make_rng, Rng};
use crate::settings::{MyuriSettings, SettingsOverride};
use crate::target_os::{primary_modifier, MouseButton, Os};
use crate::typing::typer::{HumanTyper, HumanTyperOptions, TyperInput};

async fn delay_ms(ms: f64) {
    if ms < 1.0 {
        return;
    }
    tokio::time::sleep(Duration::from_secs_f64(ms / 1000.0)).await;
}

struct DriverHost {
    driver: Arc<DefinedDriver>,
    started_at: Instant,
    wheel_queue: tokio::sync::Mutex<Vec<(f64, f64)>>,
}

impl DriverHost {
    fn new(driver: Arc<DefinedDriver>) -> Self {
        Self {
            driver,
            started_at: Instant::now(),
            wheel_queue: tokio::sync::Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl Host for DriverHost {
    async fn move_mouse(&self, x: f64, y: f64) -> Result<()> {
        self.driver.move_mouse(x, y).await
    }

    async fn click(&self, button: MouseButton, click_count: usize, hold_ms: f64) -> Result<()> {
        self.driver.click(button, click_count, hold_ms).await
    }

    async fn down(&self, button: MouseButton) -> Result<()> {
        self.driver.mouse_down(button).await
    }

    async fn up(&self, button: MouseButton) -> Result<()> {
        self.driver.mouse_up(button).await
    }

    async fn wheel(&self, dx: f64, dy: f64) -> Result<()> {
        self.wheel_queue.lock().await.push((dx, dy));
        Ok(())
    }

    async fn flush_wheels(&self) -> Result<()> {
        let pending: Vec<(f64, f64)> = {
            let mut guard = self.wheel_queue.lock().await;
            std::mem::take(&mut *guard)
        };
        for (dx, dy) in pending {
            let _ = self.driver.wheel(dx, dy).await;
        }
        Ok(())
    }

    async fn key_down(&self, key: &str) -> Result<()> {
        self.driver.down(key).await
    }

    async fn key_up(&self, key: &str) -> Result<()> {
        self.driver.up(key).await
    }

    async fn insert_text(&self, text: &str) -> Result<()> {
        self.driver.insert_text(text).await
    }

    async fn evaluate(&self, target: &Target, fn_source: &str, arg: Value) -> Result<Value> {
        let script = on_selector_script(fn_source);
        self.driver
            .evaluate(&script, Some(json!({ "selector": target.selector, "arg": arg })))
            .await
    }

    async fn offset(&self, target: &Target) -> Result<Option<[f64; 2]>> {
        let script = on_selector_script("() => true");
        let found = self
            .driver
            .evaluate(&script, Some(json!({ "selector": target.selector, "arg": Value::Null })))
            .await?;
        Ok(found.as_bool().filter(|&b| b).map(|_| [0.0, 0.0]))
    }

    async fn viewport(&self) -> Result<[f64; 2]> {
        let vp = viewport_of(self.driver.as_ref()).await?;
        Ok([vp.width, vp.height])
    }

    fn now(&self) -> f64 {
        self.started_at.elapsed().as_secs_f64() * 1000.0
    }

    async fn scroll_offset(&self) -> Result<Option<f64>> {
        scroll_offset_of(self.driver.as_ref()).await.map(Some)
    }
}

struct DriverTyperInput(Arc<DefinedDriver>);

#[async_trait]
impl TyperInput for DriverTyperInput {
    async fn down(&self, key: &str) -> Result<()> {
        self.0.down(key).await
    }

    async fn up(&self, key: &str) -> Result<()> {
        self.0.up(key).await
    }

    async fn insert_text(&self, text: &str) -> Result<()> {
        self.0.insert_text(text).await
    }
}

#[derive(Debug, Clone)]
pub enum Destination {
    Selector(String),
    Point(Vector),
}

impl From<&str> for Destination {
    fn from(s: &str) -> Self {
        Self::Selector(s.to_owned())
    }
}

impl From<String> for Destination {
    fn from(s: String) -> Self {
        Self::Selector(s)
    }
}

impl From<Vector> for Destination {
    fn from(p: Vector) -> Self {
        Self::Point(p)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FocusedFieldInfo {
    #[serde(rename = "box")]
    box_rect: BoundingBox,
    empty: bool,
    selection: String,
    lines: usize,
    single_line: bool,
    first_line_span: Option<FirstLineSpan>,
    visual_lines: usize,
    scroll_top: f64,
    first_line: Vector,
}

#[derive(Debug, Clone, Deserialize)]
struct FirstLineSpan {
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
}

#[derive(Debug, Clone, Default)]
pub struct CursorOptions {
    pub os: Option<Os>,
    pub seed: Option<u32>,
    pub settings: Option<SettingsOverride>,
    pub defaults: Option<CursorDefaults>,
    pub select_all_modifier: Option<String>,
    pub start: Option<Vector>,
    pub dimensions: Option<ViewportSize>,
    pub has_native_insert: Option<bool>,
}

/// Human-like cursor orchestrating pointer movement, clicks, drags, scrolling, and typing.
pub struct Cursor {
    pub driver: Arc<DefinedDriver>,
    pub actions: Actions,
    pub rng: Rng,
    pub seed: u32,
    pub settings: MyuriSettings,
    pub defaults: CursorDefaults,
    pub select_all_modifier: Option<String>,
    typer: HumanTyper<DriverTyperInput>,
    disposed: bool,
}

impl Cursor {
    pub fn new(driver: Arc<dyn Driver>, options: CursorOptions) -> Result<Self> {
        if let (Some(os), Some(ref sam)) = (options.os, &options.select_all_modifier) {
            if sam != primary_modifier(Some(os)) {
                return Err(MyuriError::InvalidArgument(format!(
                    "select_all_modifier {sam} is not select-all on {}",
                    os.as_str()
                )));
            }
        }
        let has_native_insert = options.has_native_insert.unwrap_or(true);
        let defined = Arc::new(define_driver(driver, options.os, has_native_insert));
        let seed = options.seed.unwrap_or_else(generate_random_seed);
        let rng = make_rng(seed);
        let profile = generate_profile(seed, options.settings.as_ref(), options.os);
        let host = Arc::new(DriverHost::new(Arc::clone(&defined)));
        let mut actions = Actions::new(
            host,
            rng.clone(),
            ActionsOptions {
                os: options.os,
                device: None,
                settings: options.settings,
            },
        );
        actions.settings = profile.clone();
        actions.move_speed = profile.mouse.speed_factor;
        actions.overshoot_threshold = profile.overshoot.threshold;
        actions.finish_position = profile.scroll.finish_position;
        actions.speed_factor = profile.scroll.speed_factor;
        actions.speed_irregularity = profile.scroll.realism.speed_irregularity;
        actions.cursor = Some(
            options
                .start
                .unwrap_or_else(|| get_better_starting_position(options.dimensions, &rng)),
        );

        let typer = HumanTyper::new(
            DriverTyperInput(Arc::clone(&defined)),
            HumanTyperOptions {
                wpm: profile.typing.wpm,
                mistake_multiplier: profile.typing.mistake_multiplier,
                pauses: Some(profile.typing.pauses.clone()),
                rng: rng.clone(),
            },
        );

        Ok(Self {
            driver: defined,
            actions,
            rng,
            seed,
            settings: profile,
            defaults: options.defaults.unwrap_or_default(),
            select_all_modifier: options.select_all_modifier,
            typer,
            disposed: false,
        })
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }

    pub fn settings(&self) -> &MyuriSettings {
        &self.settings
    }

    pub fn location(&self) -> Vector {
        self.actions.cursor.unwrap_or(Vector::new(0.0, 0.0))
    }

    pub fn mistake_multiplier(&self) -> f64 {
        self.settings.typing.mistake_multiplier
    }

    pub fn set_mistake_multiplier(&mut self, multiplier: f64) {
        self.settings.typing.mistake_multiplier = multiplier;
        self.actions.settings.typing.mistake_multiplier = multiplier;
        self.typer.set_mistake_multiplier(multiplier);
    }

    async fn end_delay(&self, move_delay: Option<f64>, randomize_move_delay: Option<bool>) {
        let base = move_delay.unwrap_or(0.0);
        let jitter = if randomize_move_delay.unwrap_or(false) {
            self.rng.next()
        } else {
            1.0
        };
        delay_ms(base * jitter).await;
    }

    async fn glide(&mut self, to: Vector, move_speed: Option<f64>) -> Result<()> {
        let saved_speed = self.actions.move_speed;
        let saved_thresh = self.actions.overshoot_threshold;
        if let Some(ms) = move_speed {
            self.actions.move_speed = ms;
        }
        self.actions.overshoot_threshold = f64::INFINITY;
        let res = self.actions.move_to(to.x, to.y).await;
        self.actions.move_speed = saved_speed;
        self.actions.overshoot_threshold = saved_thresh;
        res
    }

    pub async fn move_to(&mut self, destination: Vector, options: Option<MoveToOptions>) -> Result<()> {
        let opts = options.unwrap_or_else(|| self.defaults.move_to.clone());
        let move_delay = opts.move_delay.or(self.defaults.move_to.move_delay);
        let rand_delay = opts
            .randomize_move_delay
            .or(self.defaults.move_to.randomize_move_delay)
            .or(Some(true));
        let speed = opts.move_speed.or(self.defaults.move_to.move_speed);
        let res = self.glide(destination, speed).await;
        self.end_delay(move_delay, rand_delay).await;
        res
    }

    async fn onto(&self, opts: &MoveOptions) -> Result<(Option<f64>, Option<f64>, Option<ResolvedBoxOptions>)> {
        let mut exclude = Vec::new();
        for sel in &opts.exclude_selectors {
            if let Some(b) = bounding_box_of(self.driver.as_ref(), sel).await? {
                exclude.push(b);
            }
        }
        let b = &opts.box_options;
        let has_box = b.width_padding_percentage.is_some()
            || b.height_padding_percentage.is_some()
            || b.padding_left.is_some()
            || b.padding_right.is_some()
            || b.padding_top.is_some()
            || b.padding_bottom.is_some();
        let box_options = if !exclude.is_empty() || has_box {
            Some(ResolvedBoxOptions {
                padding: b.clone(),
                exclude,
            })
        } else {
            None
        };
        Ok((opts.move_speed, opts.overshoot_threshold, box_options))
    }

    async fn reach(
        &mut self,
        selector: &str,
        move_opts: &MoveOptions,
        button: MouseButton,
        click_count: usize,
        hold_ms: Option<f64>,
        press: bool,
    ) -> Result<()> {
        let (ms, ot, bo) = self.onto(move_opts).await?;
        let saved_ms = self.actions.move_speed;
        let saved_ot = self.actions.overshoot_threshold;
        let saved_bo = self.actions.box_options.take();
        let saved_hold = self.actions.hold_ms;

        if let Some(v) = ms {
            self.actions.move_speed = v;
        }
        if let Some(v) = ot {
            self.actions.overshoot_threshold = v;
        }
        self.actions.box_options = bo;
        self.actions.hold_ms = hold_ms;

        let res = self
            .actions
            .click(
                &Target::new(selector),
                ActionClickOptions {
                    button,
                    click_count,
                    press,
                    timeout: Some(move_opts.timeout.unwrap_or(10_000.0)),
                    attempts: move_opts.max_tries,
                    ..Default::default()
                },
            )
            .await;

        self.actions.move_speed = saved_ms;
        self.actions.overshoot_threshold = saved_ot;
        self.actions.box_options = saved_bo;
        self.actions.hold_ms = saved_hold;
        res
    }

    pub async fn move_to_selector(
        &mut self,
        selector: &str,
        options: Option<MoveOptions>,
    ) -> Result<()> {
        let opts = options.unwrap_or_else(|| self.defaults.move_opts.clone());
        let res = self
            .reach(selector, &opts, MouseButton::Left, 1, None, false)
            .await;
        self.end_delay(
            opts.move_delay.or(Some(0.0)),
            opts.randomize_move_delay.or(Some(true)),
        )
        .await;
        res
    }

    pub async fn click(
        &mut self,
        selector: Option<&str>,
        options: Option<ClickOptions>,
    ) -> Result<()> {
        let opts = options.unwrap_or_else(|| self.defaults.click.clone());
        let button = opts.button.or(self.defaults.click.button).unwrap_or(MouseButton::Left);
        let click_count = opts
            .click_count
            .or(self.defaults.click.click_count)
            .unwrap_or(1)
            .clamp(1, 3);
        let wait_for_click = opts
            .wait_for_click
            .or(self.defaults.click.wait_for_click)
            .unwrap_or(50.0);

        match selector {
            Some(sel) => {
                self.reach(
                    sel,
                    &opts.move_options,
                    button,
                    click_count,
                    opts.hold_ms,
                    true,
                )
                .await?;
            }
            None => {
                let saved_hold = self.actions.hold_ms;
                self.actions.hold_ms = opts.hold_ms;
                let r = self.actions.press(button, click_count, &[]).await;
                self.actions.hold_ms = saved_hold;
                r?;
            }
        }
        delay_ms(wait_for_click.max(15.0)).await;
        self.end_delay(
            opts.move_options.move_delay.or(Some(0.0)),
            opts.move_options.randomize_move_delay.or(Some(true)),
        )
        .await;
        Ok(())
    }

    pub async fn drag(
        &mut self,
        from: impl Into<Destination>,
        to: impl Into<Destination>,
        options: Option<DragOptions>,
    ) -> Result<()> {
        let opts = options.unwrap_or_default();
        let button = opts.button.unwrap_or(MouseButton::Left);
        let wait_for_release = opts.wait_for_release.unwrap_or(50.0);

        match from.into() {
            Destination::Selector(sel) => {
                self.reach(&sel, &opts.move_options, button, 1, None, false)
                    .await?;
            }
            Destination::Point(pt) => {
                self.glide(pt, opts.move_options.move_speed).await?;
            }
        }
        delay_ms(pre_click_dwell_ms(&self.rng) - self.actions.since_move()).await;
        self.driver.mouse_down(button).await?;

        let drag_res: Result<()> = async {
            delay_ms(drag_start_ms(&self.rng)).await;
            let target = match to.into() {
                Destination::Selector(sel) => {
                    let box_rect = bounding_box_of(self.driver.as_ref(), &sel)
                        .await?
                        .ok_or_else(|| {
                            MyuriError::DriverError(format!(
                                "drag target not found or not visible: {sel}"
                            ))
                        })?;
                    get_random_box_point(
                        box_rect,
                        Some(&opts.move_options.box_options),
                        self.actions.cursor,
                        &[],
                        &self.rng,
                    )?
                }
                Destination::Point(pt) => pt,
            };
            let carry = opts
                .move_options
                .move_speed
                .unwrap_or(self.actions.move_speed)
                * drag_carry_speed(&self.rng);
            self.glide(target, Some(carry)).await?;
            delay_ms(drag_end_ms(&self.rng) - self.actions.since_move()).await;
            Ok(())
        }
        .await;

        let up_res = self.driver.mouse_up(button).await;
        drag_res?;
        up_res?;
        delay_ms(wait_for_release).await;
        Ok(())
    }

    async fn read_focused_field(driver: &dyn Driver) -> Result<Option<FocusedFieldInfo>> {
        let val = driver.evaluate(FOCUSED_FIELD_JS, None).await?;
        if val.is_null() {
            return Ok(None);
        }
        serde_json::from_value(val)
            .map(Some)
            .map_err(|e| MyuriError::DriverError(e.to_string()))
    }

    async fn clear_focused_field(&mut self, mut focus_clicked: bool) -> Result<()> {
        let mut field = Self::read_focused_field(self.driver.as_ref())
            .await?
            .ok_or_else(|| {
                MyuriError::DriverError(
                    "clearExisting needs a focused field (pass a selector)".into(),
                )
            })?;
        if field.empty {
            return Ok(());
        }

        if let Some(ref sam) = self.select_all_modifier {
            self.typer.delete_all(0, Some(sam)).await?;
        } else {
            let here = self.actions.cursor.unwrap_or(Vector::new(0.0, 0.0));
            if !point_in_box(here, field.box_rect) {
                let pt = get_random_box_point(field.box_rect, None, Some(here), &[], &self.rng)?;
                self.glide(pt, None).await?;
                focus_clicked = false;
            }
            let at = self.actions.cursor.unwrap_or(here);
            let on_first_line = field.first_line_span.as_ref().is_none_or(|s| {
                at.x >= s.left && at.x <= s.right - 2.0 && at.y >= s.top && at.y <= s.bottom
            });
            let mut clicks = if focus_clicked { 2 } else { 3 };
            if field.lines > 1 || (!field.single_line && !on_first_line) {
                for _ in 0..30 {
                    if field.scroll_top <= 0.0 {
                        break;
                    }
                    let dy = -field.scroll_top.min(80.0 + self.rng.next() * 80.0);
                    self.driver.wheel(0.0, dy).await?;
                    delay_ms(30.0 + self.rng.next() * 40.0).await;
                    if let Some(updated) = Self::read_focused_field(self.driver.as_ref()).await? {
                        field = updated;
                    }
                }
                let target = Vector {
                    x: field.first_line.x + self.rng.next() * 12.0,
                    y: field.first_line.y + (self.rng.next() - 0.5) * 3.0,
                };
                self.glide(target, None).await?;
                clicks = 3;
            }
            self.actions.press(MouseButton::Left, clicks, &[]).await?;
            if field.lines > 1 {
                delay_ms(120.0 + self.rng.next() * 120.0).await;
                self.driver.down("Shift").await?;
                let max_presses = field.lines.max(field.visual_lines) + 2;
                for _ in 0..max_presses {
                    let cur = Self::read_focused_field(self.driver.as_ref()).await?;
                    if cur.as_ref().is_none_or(|f| f.selection != "partial") {
                        break;
                    }
                    self.driver.press("ArrowDown", None).await?;
                    delay_ms(40.0 + self.rng.next() * 60.0).await;
                }
                self.driver.up("Shift").await?;
            }
            delay_ms(60.0 + self.rng.next() * 90.0).await;
            self.typer.delete_all(1, None).await?;
        }

        let after = Self::read_focused_field(self.driver.as_ref()).await?;
        if !after.is_some_and(|f| f.empty) {
            return Err(MyuriError::DriverError(
                "clearExisting could not empty the focused field".into(),
            ));
        }
        Ok(())
    }

    pub async fn type_text(
        &mut self,
        text: &str,
        selector: Option<&str>,
        options: Option<TypeOptions>,
    ) -> Result<()> {
        let opts = options.unwrap_or_else(|| self.defaults.typing.clone());
        if let Some(sel) = selector {
            self.click(
                Some(sel),
                Some(ClickOptions {
                    move_options: MoveOptions {
                        timeout: opts.timeout,
                        max_tries: opts.max_tries,
                        ..Default::default()
                    },
                    wait_for_click: Some(30.0),
                    ..Default::default()
                }),
            )
            .await?;
        }
        if opts.clear_existing.unwrap_or(false) {
            self.clear_focused_field(selector.is_some()).await?;
        }
        self.typer.type_text(text).await
    }

    pub async fn paste(&mut self, value: &str, selector: Option<&str>) -> Result<()> {
        if let Some(sel) = selector {
            self.click(
                Some(sel),
                Some(ClickOptions {
                    wait_for_click: Some(30.0),
                    ..Default::default()
                }),
            )
            .await?;
        }
        self.driver.insert_text(value).await?;
        delay_ms(self.rng.next() * 25.0 + 25.0).await;
        Ok(())
    }

    pub async fn scroll(&mut self, selector: &str, options: Option<ScrollOptions>) -> Result<()> {
        let opts = options.unwrap_or_else(|| self.defaults.scroll.clone());
        let here = self.actions.cursor.unwrap_or(Vector::new(0.0, 0.0));
        self.glide(
            Vector {
                x: here.x + self.rng.next() * 20.0 - 10.0,
                y: here.y + self.rng.next() * 20.0 - 10.0,
            },
            None,
        )
        .await?;
        let timeout = opts.timeout.unwrap_or(10_000.0);
        if !wait_for_element(self.driver.as_ref(), selector, timeout).await? {
            return Err(MyuriError::DriverError(format!(
                "element not found: {selector}"
            )));
        }

        let saved_fp = self.actions.finish_position;
        let saved_passes = self.actions.scroll_passes;
        let saved_sf = self.actions.speed_factor;
        let saved_si = self.actions.speed_irregularity;

        if let Some(fp) = opts.finish_position {
            self.actions.finish_position = fp;
        }
        if let Some(mt) = opts.max_tries {
            self.actions.scroll_passes = mt;
        }
        if let Some(sf) = opts.speed_factor {
            self.actions.speed_factor = sf;
        }
        if let Some(si) = opts.speed_irregularity {
            self.actions.speed_irregularity = si;
        }

        let res = self.actions.into_view(&Target::new(selector)).await;

        self.actions.finish_position = saved_fp;
        self.actions.scroll_passes = saved_passes;
        self.actions.speed_factor = saved_sf;
        self.actions.speed_irregularity = saved_si;
        res
    }

    pub async fn set_cursor_location(&mut self, location: Vector) -> Result<()> {
        let vp = viewport_of(self.driver.as_ref()).await?;
        let x = location.x.round().clamp(0.0, vp.width - 1.0);
        let y = location.y.round().clamp(0.0, vp.height - 1.0);
        self.driver.move_mouse(x, y).await?;
        self.actions.moved(x, y);
        Ok(())
    }

    pub async fn dispose(&mut self) -> Result<()> {
        if self.disposed {
            return Ok(());
        }
        self.disposed = true;
        self.driver.dispose().await
    }
}

pub fn create_cursor(driver: Arc<dyn Driver>, options: CursorOptions) -> Result<Cursor> {
    Cursor::new(driver, options)
}

#[cfg(feature = "playwright")]
pub fn create_playwright_cursor(
    page: playwright_rs::protocol::Page,
    options: CursorOptions,
) -> Result<Cursor> {
    let drv = Arc::new(crate::drivers::playwright_driver(page));
    Cursor::new(drv, options)
}

#[allow(dead_code)]
fn _unused_helpers_check() {
    let _ = is_point_excluded;
}
