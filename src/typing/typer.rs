use std::collections::HashSet;
use std::time::Duration;

use async_trait::async_trait;
use tokio::time::Instant;

use crate::exceptions::Result;
use crate::rng::{gaussian, Rng};
use crate::settings::PauseSettings;

use super::config::{DEFAULT_WPM, WPM_STD};
use super::markov::{HistoryKind, MarkovEvent, MarkovTyper};
use super::timing::{char_to_stroke, KeyAction, KeyActionType, KeystrokeTiming, Stroke};

#[async_trait]
pub trait TyperInput: Send + Sync {
    async fn down(&self, key: &str) -> Result<()>;
    async fn up(&self, key: &str) -> Result<()>;
    async fn insert_text(&self, text: &str) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct FlattenedStroke {
    pub stroke: Stroke,
    pub ch: String,
    pub event: MarkovEvent,
    pub mistake: bool,
}

fn flatten(history: Vec<MarkovEvent>) -> Vec<FlattenedStroke> {
    let mut out = Vec::new();
    for event in history {
        if event.kind == HistoryKind::Backspace {
            out.push(FlattenedStroke {
                stroke: Stroke::key("Backspace"),
                ch: String::new(),
                event,
                mistake: false,
            });
            continue;
        }
        let chars: Vec<char> = event.chars.chars().collect();
        let len = chars.len();
        for (i, ch) in chars.into_iter().enumerate() {
            out.push(FlattenedStroke {
                stroke: char_to_stroke(ch),
                ch: ch.to_string(),
                event: event.clone(),
                mistake: (event.kind == HistoryKind::Error || event.kind == HistoryKind::Swap)
                    && i + 1 == len,
            });
        }
    }
    out
}

pub fn draw_session_wpm(rng: &Rng, wpm: f64) -> f64 {
    gaussian(rng, wpm, WPM_STD).max(10.0)
}

pub struct PlannedTyping {
    pub strokes: Vec<FlattenedStroke>,
    pub actions: Vec<KeyAction>,
    pub down_at: Vec<f64>,
}

pub fn plan_typing(
    text: &str,
    mistake_multiplier: f64,
    rng: Rng,
    timing: &mut KeystrokeTiming,
) -> Result<PlannedTyping> {
    let history = MarkovTyper::new(text, mistake_multiplier, rng)?.run();
    let flattened = flatten(history);
    let raw_strokes: Vec<Stroke> = flattened.iter().map(|f| f.stroke.clone()).collect();
    let (actions, down_at) = timing.plan(&raw_strokes, None);
    Ok(PlannedTyping {
        strokes: flattened,
        actions,
        down_at,
    })
}

pub struct HumanTyperOptions {
    pub wpm: f64,
    pub mistake_multiplier: f64,
    pub pauses: Option<PauseSettings>,
    pub rng: Rng,
}

impl Default for HumanTyperOptions {
    fn default() -> Self {
        Self {
            wpm: DEFAULT_WPM,
            mistake_multiplier: 1.0,
            pauses: None,
            rng: Rng::default_entropy(),
        }
    }
}

pub struct HumanTyper<I: TyperInput> {
    input: I,
    wpm: f64,
    mistake_multiplier: f64,
    rng: Rng,
    session_wpm: f64,
    timing: KeystrokeTiming,
}

impl<I: TyperInput> HumanTyper<I> {
    pub fn new(input: I, options: HumanTyperOptions) -> Self {
        let session_wpm = draw_session_wpm(&options.rng, options.wpm);
        let timing = KeystrokeTiming::new(session_wpm, options.rng.clone(), options.pauses);
        Self {
            input,
            wpm: options.wpm,
            mistake_multiplier: options.mistake_multiplier,
            rng: options.rng,
            session_wpm,
            timing,
        }
    }

    pub async fn type_text(&mut self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }
        let planned = plan_typing(
            text,
            self.mistake_multiplier,
            self.rng.clone(),
            &mut self.timing,
        )?;
        self.play(&planned.actions).await
    }

    pub async fn play(&self, actions: &[KeyAction]) -> Result<()> {
        let start = Instant::now();
        let mut held: HashSet<String> = HashSet::new();
        let mut result = Ok(());

        for a in actions {
            let target_at = Duration::from_secs_f64((a.at / 1000.0).max(0.0));
            let elapsed = start.elapsed();
            if target_at > elapsed {
                tokio::time::sleep(target_at - elapsed).await;
            }
            let step_res = match a.action_type {
                KeyActionType::Down => {
                    let r = self.input.down(&a.key).await;
                    if r.is_ok() {
                        held.insert(a.key.clone());
                    }
                    r
                }
                KeyActionType::Up => {
                    let r = self.input.up(&a.key).await;
                    held.remove(&a.key);
                    r
                }
                KeyActionType::Insert => self.input.insert_text(&a.key).await,
            };
            if let Err(e) = step_res {
                result = Err(e);
                break;
            }
        }

        for key in held {
            let _ = self.input.up(&key).await;
        }
        result
    }

    pub async fn delete_all(
        &mut self,
        count: usize,
        select_all_modifier: Option<&str>,
    ) -> Result<()> {
        if let Some(modifier) = select_all_modifier {
            let actions = self.timing.plan_select_all_delete(modifier);
            return self.play(&actions).await;
        }
        if count == 0 {
            return Ok(());
        }
        let strokes = vec![Stroke::key("Backspace"); count];
        let (actions, _) = self.timing.plan(&strokes, None);
        self.play(&actions).await
    }

    pub fn set_wpm(&mut self, wpm: f64) {
        self.wpm = wpm;
        self.session_wpm = draw_session_wpm(&self.rng, wpm);
        self.timing.set_wpm(self.session_wpm);
    }

    pub fn wpm(&self) -> f64 {
        self.wpm
    }

    pub fn set_mistake_multiplier(&mut self, mult: f64) {
        self.mistake_multiplier = mult;
    }

    pub fn mistake_multiplier(&self) -> f64 {
        self.mistake_multiplier
    }

    pub fn session_wpm(&self) -> f64 {
        self.session_wpm
    }
}
