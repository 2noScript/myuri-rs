use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use base64::{engine::general_purpose::STANDARD, Engine as _};

use crate::rng::{gaussian, Rng};
use crate::settings::PauseSettings;

use super::bigrams::{BIGRAM_MEAN_LOG10, BIGRAM_TABLE_B64};

const BASE: f64 = 4.618;
const BIGRAM: f64 = -0.338;
const SAME_FINGER: f64 = 0.289;
const REPEAT: f64 = 0.178;
const ROW_JUMP: f64 = 0.04;
const SHIFTED: f64 = 0.976;
const SHIFTED_PUNCT_SHARE: f64 = 0.5;
const COMMON_PUNCT: &str = ".,'-/@:;?!";

const CONTEXT_IN_WORD: f64 = 0.0;
const CONTEXT_WORD_START: f64 = 0.788;
const CONTEXT_TO_SPACE: f64 = 0.071;
const CONTEXT_TO_ENTER: f64 = 1.2;
const CONTEXT_TO_DIGIT: f64 = 1.848;
const CONTEXT_DIGIT_RUN: f64 = 0.301;
const CONTEXT_TO_PUNCT: f64 = 1.045;
const CONTEXT_FROM_PUNCT: f64 = 1.316;
const CONTEXT_TO_COMMON_PUNCT: f64 = 0.833;
const CONTEXT_FROM_COMMON_PUNCT: f64 = 0.223;
const CONTEXT_BS_NOTICE: f64 = 1.378;
const CONTEXT_BS_REPEAT: f64 = 0.449;
const CONTEXT_AFTER_BS: f64 = 0.657;

const NOISE_SD: f64 = 0.48;
const PAUSE_PROB: f64 = 0.06;
const PAUSE_MEAN: f64 = 1.0;
const PAUSE_SD: f64 = 0.5;
const DRIFT_RHO: f64 = 0.97;
const DRIFT_STEP_SD: f64 = 0.03;
const MIN_GAP_MS: f64 = 10.0;
const MAX_GAP_MS: f64 = 5000.0;

const MIN_HOLD_MS: f64 = 25.0;
const MAX_HOLD_MS: f64 = 400.0;
const SESSION_HOLD_SD: f64 = 0.12;
const HOLD_SPEED_EXPONENT: f64 = 0.3;

const SHIFT_LEAD_MEDIAN: f64 = 115.0;
const SHIFT_LEAD_SD: f64 = 0.6;
const SHIFT_LEAD_MIN: f64 = 30.0;
const SHIFT_TRAIL_MEAN: f64 = -15.0;
const SHIFT_TRAIL_SD: f64 = 35.0;
const SHIFT_MIN_AFTER_KEY: f64 = 20.0;
const SHIFT_CLEAR: f64 = 8.0;
const SAME_KEY_CLEAR: f64 = 8.0;

const REFERENCE_TEXT: &str = "The quick check took about 20 minutes, so we moved on. Please send the report to the team before Friday; they need it for the meeting at 3pm. I think it looks good, but there are a few things we should fix first. Thanks again for your help with this! Let me know if anything changes.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stroke {
    pub key: String,
    pub insert: bool,
}

impl Stroke {
    pub fn key(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            insert: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyActionType {
    Down,
    Up,
    Insert,
}

impl KeyActionType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Down => "down",
            Self::Up => "up",
            Self::Insert => "insert",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyAction {
    pub at: f64,
    pub action_type: KeyActionType,
    pub key: String,
    pub stroke: Option<usize>,
}

struct KeyboardGeo {
    unshift: HashMap<char, char>,
    finger: HashMap<char, u8>,
    row: HashMap<char, usize>,
    bigram_log10: Vec<f64>,
    common_punct: HashSet<char>,
    ref_mean_gap_ms: f64,
}

fn build_geo() -> KeyboardGeo {
    let shifted = "~!@#$%^&*()_+{}|:\"<>?";
    let plain = "`1234567890-=[]\\;',./";
    let mut unshift = HashMap::new();
    for (s, p) in shifted.chars().zip(plain.chars()) {
        unshift.insert(s, p);
    }

    let mut finger = HashMap::new();
    let left = ["`12345", "qwert", "asdfg", "zxcvb"];
    for row in left {
        let fingers: &[u8] = if row.starts_with('`') {
            &[0, 0, 1, 2, 3, 3]
        } else {
            &[0, 1, 2, 3, 3]
        };
        for (c, &f) in row.chars().zip(fingers) {
            finger.insert(c, f);
        }
    }
    let right = ["67890-=", "yuiop[]\\", "hjkl;'", "nm,./"];
    let right_fingers: [u8; 8] = [4, 4, 5, 6, 7, 7, 7, 7];
    for row_s in right {
        for (c, &f) in row_s.chars().zip(&right_fingers) {
            finger.insert(c, f);
        }
    }

    let mut row = HashMap::new();
    let rows = ["`1234567890-=", "qwertyuiop[]\\", "asdfghjkl;'", "zxcvbnm,./"];
    for (r, r_str) in rows.iter().enumerate() {
        for c in r_str.chars() {
            row.insert(c, r);
        }
    }

    let raw = STANDARD
        .decode(BIGRAM_TABLE_B64)
        .expect("embedded bigram table is valid base64");
    let bigram_log10: Vec<f64> = raw.into_iter().map(|b| -f64::from(b) / 20.0).collect();

    let common_punct: HashSet<char> = COMMON_PUNCT.chars().collect();
    let noise_mean_factor = (0.5 * NOISE_SD * NOISE_SD).exp()
        * (1.0 - PAUSE_PROB + PAUSE_PROB * (PAUSE_MEAN + 0.5 * PAUSE_SD * PAUSE_SD).exp())
        * (0.5 * (DRIFT_STEP_SD * DRIFT_STEP_SD) / (1.0 - DRIFT_RHO * DRIFT_RHO)).exp();

    let mut geo = KeyboardGeo {
        unshift,
        finger,
        row,
        bigram_log10,
        common_punct,
        ref_mean_gap_ms: 1.0,
    };

    let ref_strokes = strokes_for_text(REFERENCE_TEXT);
    let mut sum = 0.0;
    for i in 1..ref_strokes.len() {
        sum += log_gap_mean_with(&geo, &ref_strokes[i - 1], &ref_strokes[i]).exp();
    }
    geo.ref_mean_gap_ms = (sum / ((ref_strokes.len() - 1) as f64)) * noise_mean_factor;
    geo
}

fn geo() -> &'static KeyboardGeo {
    static GEO: OnceLock<KeyboardGeo> = OnceLock::new();
    GEO.get_or_init(build_geo)
}

fn single_char(s: &str) -> Option<char> {
    let mut chars = s.chars();
    let c = chars.next()?;
    if chars.next().is_none() {
        Some(c)
    } else {
        None
    }
}

fn physical(key: &str) -> String {
    let Some(ch) = single_char(key) else {
        return key.to_owned();
    };
    let lower = ch.to_lowercase().next().unwrap_or(ch);
    geo().unshift.get(&lower).copied().unwrap_or(lower).to_string()
}

fn needs_shift_with(unshift: &HashMap<char, char>, key: &str) -> bool {
    let Some(ch) = single_char(key) else {
        return false;
    };
    ch.is_ascii_uppercase() || unshift.contains_key(&ch)
}

pub fn needs_shift(key: &str) -> bool {
    needs_shift_with(&geo().unshift, key)
}

#[inline]
fn is_lower_letter(s: &str) -> Option<char> {
    let c = single_char(s)?;
    c.is_ascii_lowercase().then_some(c)
}

#[inline]
fn is_digit_char(s: &str) -> bool {
    single_char(s).is_some_and(|c| c.is_ascii_digit())
}

#[inline]
fn is_alnum_ascii(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

fn context_of(g: &KeyboardGeo, prev: &Stroke, cur: &Stroke) -> (&'static str, f64) {
    let p = prev.key.as_str();
    let c = cur.key.as_str();
    if c == "Backspace" {
        return if p == "Backspace" {
            ("bsRepeat", CONTEXT_BS_REPEAT)
        } else {
            ("bsNotice", CONTEXT_BS_NOTICE)
        };
    }
    if p == "Backspace" {
        return ("afterBs", CONTEXT_AFTER_BS);
    }
    if c == " " {
        return ("toSpace", CONTEXT_TO_SPACE);
    }
    if c == "Enter" || c == "Tab" {
        return ("toEnter", CONTEXT_TO_ENTER);
    }
    if p == " " || p == "Enter" || p == "Tab" {
        return ("wordStart", CONTEXT_WORD_START);
    }
    if cur.insert {
        return ("toPunct", CONTEXT_TO_PUNCT);
    }
    if is_digit_char(c) {
        return if is_digit_char(p) {
            ("digitRun", CONTEXT_DIGIT_RUN)
        } else {
            ("toDigit", CONTEXT_TO_DIGIT)
        };
    }
    if let Some(cc) = single_char(c) {
        if !is_alnum_ascii(cc) {
            return if g.common_punct.contains(&cc) {
                ("toCommonPunct", CONTEXT_TO_COMMON_PUNCT)
            } else {
                ("toPunct", CONTEXT_TO_PUNCT)
            };
        }
    }
    if let Some(pc) = single_char(p) {
        if g.common_punct.contains(&pc) {
            return ("fromCommonPunct", CONTEXT_FROM_COMMON_PUNCT);
        }
    }
    if prev.insert || is_digit_char(p) || single_char(p).is_some_and(|pc| !is_alnum_ascii(pc)) {
        return ("fromPunct", CONTEXT_FROM_PUNCT);
    }
    ("inWord", CONTEXT_IN_WORD)
}

fn log_gap_mean_with(g: &KeyboardGeo, prev: &Stroke, cur: &Stroke) -> f64 {
    let (ctx_name, ctx_val) = context_of(g, prev, cur);
    let shifted = !cur.insert
        && needs_shift_with(&g.unshift, &cur.key)
        && !needs_shift_with(&g.unshift, &prev.key);
    let mut mu = BASE
        + ctx_val
            * if shifted && ctx_name == "toCommonPunct" {
                SHIFTED_PUNCT_SHARE
            } else {
                1.0
            };
    if cur.insert || prev.insert {
        return mu;
    }

    let phys_key = |k: &str| -> String {
        let Some(ch) = single_char(k) else {
            return k.to_owned();
        };
        let lower = ch.to_lowercase().next().unwrap_or(ch);
        g.unshift.get(&lower).copied().unwrap_or(lower).to_string()
    };
    let a = phys_key(&prev.key);
    let b = phys_key(&cur.key);
    if let (Some(ac), Some(bc)) = (is_lower_letter(&a), is_lower_letter(&b)) {
        let idx = ((ac as usize) - 97) * 26 + ((bc as usize) - 97);
        let share = g.bigram_log10[idx];
        mu += BIGRAM * (share - BIGRAM_MEAN_LOG10);
    }
    if let (Some(ac), Some(bc)) = (single_char(&a), single_char(&b)) {
        if let (Some(&fa), Some(&fb)) = (g.finger.get(&ac), g.finger.get(&bc)) {
            if fa == fb && ac != bc {
                mu += SAME_FINGER;
            }
        }
        if ac == bc {
            mu += REPEAT;
        }
        if let (Some(&ra), Some(&rb)) = (g.row.get(&ac), g.row.get(&bc)) {
            mu += ROW_JUMP * ((ra as isize - rb as isize).abs() as f64);
        }
    }
    if shifted {
        mu += SHIFTED;
    }
    mu
}

fn hold_params(key: &str) -> (f64, f64) {
    match key {
        " " => (80.0, 0.22),
        "Backspace" => (85.0, 0.45),
        "Enter" | "Tab" => (95.0, 0.25),
        _ => (100.0, 0.28),
    }
}

pub fn char_to_stroke(ch: char) -> Stroke {
    match ch {
        '\n' => Stroke {
            key: "Enter".into(),
            insert: false,
        },
        '\t' => Stroke {
            key: "Tab".into(),
            insert: false,
        },
        c if ('\x20'..='\x7e').contains(&c) => Stroke {
            key: c.to_string(),
            insert: false,
        },
        c => Stroke {
            key: c.to_string(),
            insert: true,
        },
    }
}

pub fn strokes_for_text(text: &str) -> Vec<Stroke> {
    text.chars().map(char_to_stroke).collect()
}

pub fn mean_gap_for_wpm(wpm: f64) -> f64 {
    12_000.0 / wpm
}

pub struct KeystrokeTiming {
    rng: Rng,
    scale: f64,
    hold_factor: f64,
    drift: f64,
    pauses: PauseSettings,
}

impl KeystrokeTiming {
    pub fn new(wpm: f64, rng: Rng, pauses: Option<PauseSettings>) -> Self {
        let g = geo();
        let scale = mean_gap_for_wpm(wpm) / g.ref_mean_gap_ms;
        let hold_factor = gaussian(&rng, 0.0, SESSION_HOLD_SD).exp();
        Self {
            rng,
            scale,
            hold_factor,
            drift: 0.0,
            pauses: pauses.unwrap_or(PauseSettings {
                rate: 1.0,
                length: 1.0,
            }),
        }
    }

    pub fn set_wpm(&mut self, wpm: f64) {
        self.scale = mean_gap_for_wpm(wpm) / geo().ref_mean_gap_ms;
    }

    pub fn gap(&mut self, prev: &Stroke, cur: &Stroke) -> f64 {
        self.drift = DRIFT_RHO * self.drift + gaussian(&self.rng, 0.0, DRIFT_STEP_SD);
        let noise = gaussian(&self.rng, 0.0, NOISE_SD);
        let mut ms =
            self.scale * (log_gap_mean_with(geo(), prev, cur) + noise + self.drift).exp();
        if self.rng.next() < PAUSE_PROB * self.pauses.rate {
            let pause_scale =
                (gaussian(&self.rng, PAUSE_MEAN, PAUSE_SD).max(0.0).exp()) - 1.0;
            ms *= 1.0 + self.pauses.length * pause_scale;
        }
        ms.clamp(MIN_GAP_MS, MAX_GAP_MS)
    }

    pub fn hold(&self, key: &str) -> f64 {
        let (median, sd) = hold_params(key);
        let ms = median
            * self.hold_factor
            * self.scale.powf(HOLD_SPEED_EXPONENT)
            * gaussian(&self.rng, 0.0, sd).exp();
        ms.clamp(MIN_HOLD_MS, MAX_HOLD_MS)
    }

    pub fn shift_lead(&self) -> f64 {
        (SHIFT_LEAD_MEDIAN * gaussian(&self.rng, 0.0, SHIFT_LEAD_SD).exp()).max(SHIFT_LEAD_MIN)
    }

    pub fn plan(
        &mut self,
        strokes: &[Stroke],
        after: Option<&Stroke>,
    ) -> (Vec<KeyAction>, Vec<f64>) {
        struct SeqAction {
            at: f64,
            action_type: KeyActionType,
            key: String,
            stroke: Option<usize>,
            seq: usize,
        }
        struct PressIdx {
            down: f64,
            up: usize,
        }

        let mut actions: Vec<SeqAction> = Vec::with_capacity(strokes.len() * 2);
        let mut seq = 0usize;
        let mut down_at: Vec<f64> = Vec::with_capacity(strokes.len());
        let mut last_press: HashMap<String, PressIdx> = HashMap::new();
        let mut shift_held = false;
        let mut last_shifted_down = 0.0_f64;
        let mut last_shifted_up = 0.0_f64;

        let push = |actions: &mut Vec<SeqAction>,
                    seq: &mut usize,
                    at: f64,
                    action_type: KeyActionType,
                    key: String,
                    stroke: Option<usize>| {
            let cur_seq = *seq;
            *seq += 1;
            actions.push(SeqAction {
                at,
                action_type,
                key,
                stroke,
                seq: cur_seq,
            });
        };

        for (i, s) in strokes.iter().enumerate() {
            let prev = if i > 0 { Some(&strokes[i - 1]) } else { after };
            let cold = i == 0 && prev.is_none();
            let mut t = match prev {
                Some(p) => {
                    let base = if i > 0 { down_at[i - 1] } else { 0.0 };
                    base + self.gap(p, s)
                }
                None => 0.0,
            };

            let shifted = !s.insert && needs_shift(&s.key);
            if !shifted && shift_held {
                t = t.max(last_shifted_down + SHIFT_MIN_AFTER_KEY + SHIFT_CLEAR);
                let at = (last_shifted_up
                    + gaussian(&self.rng, SHIFT_TRAIL_MEAN, SHIFT_TRAIL_SD))
                .max(last_shifted_down + SHIFT_MIN_AFTER_KEY)
                .min(t - SHIFT_CLEAR);
                push(
                    &mut actions,
                    &mut seq,
                    at,
                    KeyActionType::Up,
                    "Shift".into(),
                    None,
                );
                shift_held = false;
            }
            if shifted && !shift_held {
                let lead = self.shift_lead();
                let floor = if i > 0 { down_at[i - 1] + 10.0 } else { 0.0 };
                if cold {
                    t = lead;
                }
                let shift_at = floor.max(t - lead);
                t = t.max(shift_at + SHIFT_LEAD_MIN);
                push(
                    &mut actions,
                    &mut seq,
                    shift_at,
                    KeyActionType::Down,
                    "Shift".into(),
                    None,
                );
                shift_held = true;
            }

            if !s.insert {
                let phys = physical(&s.key);
                if let Some(last) = last_press.get(&phys) {
                    let latest_up = t - SAME_KEY_CLEAR;
                    if actions[last.up].at > latest_up {
                        let earliest_up = last.down + MIN_HOLD_MS;
                        if latest_up >= earliest_up {
                            actions[last.up].at = latest_up;
                        } else {
                            actions[last.up].at = earliest_up;
                            t = earliest_up + SAME_KEY_CLEAR;
                        }
                    }
                }
            }

            down_at.push(t);
            if s.insert {
                push(
                    &mut actions,
                    &mut seq,
                    t,
                    KeyActionType::Insert,
                    s.key.clone(),
                    Some(i),
                );
                continue;
            }

            push(
                &mut actions,
                &mut seq,
                t,
                KeyActionType::Down,
                s.key.clone(),
                Some(i),
            );
            let up = t + self.hold(&s.key);
            let up_idx = actions.len();
            last_press.insert(physical(&s.key), PressIdx { down: t, up: up_idx });
            push(
                &mut actions,
                &mut seq,
                up,
                KeyActionType::Up,
                s.key.clone(),
                Some(i),
            );
            if shifted {
                last_shifted_down = t;
                last_shifted_up = up;
            }
        }

        if shift_held {
            let at = (last_shifted_up + gaussian(&self.rng, SHIFT_TRAIL_MEAN, SHIFT_TRAIL_SD))
                .max(last_shifted_down + SHIFT_MIN_AFTER_KEY);
            push(
                &mut actions,
                &mut seq,
                at,
                KeyActionType::Up,
                "Shift".into(),
                None,
            );
        }

        actions.sort_by(|a, b| a.at.total_cmp(&b.at).then_with(|| a.seq.cmp(&b.seq)));
        let planned = actions
            .into_iter()
            .map(|a| KeyAction {
                at: a.at,
                action_type: a.action_type,
                key: a.key,
                stroke: a.stroke,
            })
            .collect();
        (planned, down_at)
    }

    pub fn plan_chord(&self, modifiers: &[&str], key: &str) -> Vec<KeyAction> {
        let mut out = Vec::with_capacity(modifiers.len() * 2 + 2);
        let mut at = 0.0_f64;
        for (i, &m) in modifiers.iter().enumerate() {
            if i > 0 {
                at += self.shift_lead();
            }
            out.push(KeyAction {
                at,
                action_type: KeyActionType::Down,
                key: m.to_owned(),
                stroke: None,
            });
        }
        let key_down = if modifiers.is_empty() {
            0.0
        } else {
            at + self.shift_lead()
        };
        let key_up = key_down + self.hold(key);
        out.push(KeyAction {
            at: key_down,
            action_type: KeyActionType::Down,
            key: key.to_owned(),
            stroke: None,
        });
        out.push(KeyAction {
            at: key_up,
            action_type: KeyActionType::Up,
            key: key.to_owned(),
            stroke: None,
        });
        for &m in modifiers.iter().rev() {
            let up = (key_up + gaussian(&self.rng, SHIFT_TRAIL_MEAN, SHIFT_TRAIL_SD))
                .max(key_down + SHIFT_MIN_AFTER_KEY);
            out.push(KeyAction {
                at: up,
                action_type: KeyActionType::Up,
                key: m.to_owned(),
                stroke: None,
            });
        }
        out.sort_by(|a, b| a.at.total_cmp(&b.at));
        out
    }

    pub fn plan_select_all_delete(&mut self, modifier: &str) -> Vec<KeyAction> {
        let mut out = self.plan_chord(&[modifier], "a");
        let a_down = out
            .iter()
            .find(|a| a.action_type == KeyActionType::Down && a.key == "a")
            .map(|a| a.at)
            .unwrap_or(0.0);
        let mod_up = out
            .iter()
            .find(|a| a.action_type == KeyActionType::Up && a.key == modifier)
            .map(|a| a.at)
            .unwrap_or(0.0);
        let gap = self.gap(&Stroke::key("a"), &Stroke::key("Backspace"));
        let bs_down = (a_down + gap).max(mod_up + SHIFT_CLEAR);
        let bs_up = bs_down + self.hold("Backspace");
        out.push(KeyAction {
            at: bs_down,
            action_type: KeyActionType::Down,
            key: "Backspace".into(),
            stroke: None,
        });
        out.push(KeyAction {
            at: bs_up,
            action_type: KeyActionType::Up,
            key: "Backspace".into(),
            stroke: None,
        });
        out.sort_by(|a, b| a.at.total_cmp(&b.at));
        out
    }
}
