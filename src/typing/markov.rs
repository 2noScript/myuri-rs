use crate::exceptions::{MyuriError, Result};
use crate::rng::Rng;

use super::config::{
    COMMON_WORD_ERROR_MULT, COMPLEX_WORD_ERROR_MULT, COMPOSED_ACCENT_ERROR_MULT,
    DRIFT_CORRECTION_PROB, PROB_ERROR, PROB_NOTICE_ERROR, PROB_SWAP_ERROR,
};
use super::keyboard::KeyboardLayout;
use super::language::{get_word_difficulty, WordDifficulty};

const WORD_BOUNDARY_CHARS: &str = " \n\t.,;!?:()[]{}<>\"'";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryKind {
    Type,
    Error,
    Swap,
    Backspace,
    Correct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkovEvent {
    pub kind: HistoryKind,
    pub chars: String,
}

struct MarkovState {
    current_text: Vec<char>,
    history: Vec<MarkovEvent>,
    last_kind: Option<HistoryKind>,
    mental_cursor_pos: usize,
}

/// Markov chain-based typing simulation engine (neighbor-key slips, swaps, and backspaces).
pub struct MarkovTyper {
    target_text: String,
    target_chars: Vec<char>,
    keyboard: KeyboardLayout,
    state: MarkovState,
    mistake_multiplier: f64,
    rng: Rng,
}

impl MarkovTyper {
    pub fn new(target_text: &str, mistake_multiplier: f64, rng: Rng) -> Result<Self> {
        if target_text.is_empty() {
            return Err(MyuriError::InvalidArgument(
                "target_text must be a non-empty string".into(),
            ));
        }
        Ok(Self {
            target_text: target_text.to_owned(),
            target_chars: target_text.chars().collect(),
            keyboard: KeyboardLayout::new(),
            state: Self::fresh_state(),
            mistake_multiplier,
            rng,
        })
    }

    fn fresh_state() -> MarkovState {
        MarkovState {
            current_text: Vec::new(),
            history: Vec::new(),
            last_kind: None,
            mental_cursor_pos: 0,
        }
    }

    fn is_complete(&self) -> bool {
        let s: String = self.state.current_text.iter().collect();
        s == self.target_text
    }

    fn get_current_word_context(&self) -> String {
        let idx = self.state.mental_cursor_pos;
        if idx >= self.target_chars.len() {
            return String::new();
        }
        let mut start = idx;
        while start > 0 && self.target_chars[start - 1] != ' ' {
            start -= 1;
        }
        let mut end = idx;
        while end < self.target_chars.len() && self.target_chars[end] != ' ' {
            end += 1;
        }
        self.target_chars[start..end].iter().collect()
    }

    fn advance_char(&mut self, ch: char) {
        self.state.current_text.push(ch);
        self.state.mental_cursor_pos += 1;
    }

    fn emit_keystroke(&mut self, ch: char, kind: HistoryKind) -> MarkovEvent {
        self.advance_char(ch);
        let event = MarkovEvent {
            kind,
            chars: ch.to_string(),
        };
        self.state.history.push(event.clone());
        self.state.last_kind = Some(kind);
        event
    }

    pub fn step(&mut self) -> Option<MarkovEvent> {
        if self.is_complete() {
            return None;
        }
        let current_len = self.state.current_text.len();

        let mut first_error_pos = self.target_chars.len();
        let min_len = current_len.min(self.target_chars.len());
        for i in 0..min_len {
            if self.state.current_text[i] != self.target_chars[i] {
                first_error_pos = i;
                break;
            }
        }
        if current_len > self.target_chars.len() && first_error_pos == self.target_chars.len() {
            first_error_pos = self.target_chars.len();
        }

        if first_error_pos < current_len {
            let mut should_correct = false;
            if self.state.last_kind == Some(HistoryKind::Backspace)
                || self.state.mental_cursor_pos >= self.target_chars.len()
            {
                should_correct = true;
            } else if current_len > 0 {
                let last_char = self.state.current_text[current_len - 1];
                let distance = current_len - first_error_pos;
                if WORD_BOUNDARY_CHARS.contains(last_char) {
                    should_correct = true;
                } else if distance >= 2 {
                    if self.rng.next() < DRIFT_CORRECTION_PROB {
                        should_correct = true;
                    }
                } else if distance == 1 && self.rng.next() < PROB_NOTICE_ERROR {
                    should_correct = true;
                }
            }

            if should_correct {
                self.state.current_text.pop();
                let event = MarkovEvent {
                    kind: HistoryKind::Backspace,
                    chars: String::new(),
                };
                self.state.history.push(event.clone());
                self.state.last_kind = Some(HistoryKind::Backspace);
                self.state.mental_cursor_pos = self.state.current_text.len();
                return Some(event);
            }
        }

        if self.state.mental_cursor_pos > self.state.current_text.len() {
            self.state.mental_cursor_pos = self.state.current_text.len();
        }
        if self.state.mental_cursor_pos >= self.target_chars.len() {
            return None;
        }

        let char_intended = self.target_chars[self.state.mental_cursor_pos];
        if !self.keyboard.has_key(char_intended) && char_intended != ' ' {
            return Some(self.emit_keystroke(char_intended, HistoryKind::Type));
        }

        if self.target_chars.len() > self.state.mental_cursor_pos + 1 {
            let char_after = self.target_chars[self.state.mental_cursor_pos + 1];
            if char_after != ' '
                && char_after != char_intended
                && self.rng.next() < (PROB_SWAP_ERROR * self.mistake_multiplier).min(1.0)
            {
                self.advance_char(char_after);
                self.advance_char(char_intended);
                let mut chars = String::with_capacity(8);
                chars.push(char_after);
                chars.push(char_intended);
                let event = MarkovEvent {
                    kind: HistoryKind::Swap,
                    chars,
                };
                self.state.history.push(event.clone());
                self.state.last_kind = Some(HistoryKind::Swap);
                return Some(event);
            }
        }

        let mut current_prob_error = PROB_ERROR * self.mistake_multiplier;
        match get_word_difficulty(&self.get_current_word_context()) {
            WordDifficulty::Complex => current_prob_error *= COMPLEX_WORD_ERROR_MULT,
            WordDifficulty::Common => current_prob_error *= COMMON_WORD_ERROR_MULT,
            WordDifficulty::Normal => {}
        }
        if self.keyboard.is_composed_accent(char_intended) {
            current_prob_error *= COMPOSED_ACCENT_ERROR_MULT;
        }
        current_prob_error = current_prob_error.min(1.0);

        if self.rng.next() < current_prob_error {
            let wrong = self
                .keyboard
                .get_random_neighbor(char_intended, &self.rng);
            return Some(self.emit_keystroke(wrong, HistoryKind::Error));
        }

        Some(self.emit_keystroke(char_intended, HistoryKind::Type))
    }

    fn run_steps(&mut self, budget: usize) {
        let mut steps = 0usize;
        while self.step().is_some() {
            steps += 1;
            if steps > budget {
                break;
            }
        }
    }

    fn emit_correct_typing(&mut self) {
        self.state = Self::fresh_state();
        let chars = self.target_chars.clone();
        for ch in chars {
            self.emit_keystroke(ch, HistoryKind::Correct);
        }
    }

    pub fn run(&mut self) -> Vec<MarkovEvent> {
        let budget = self.target_chars.len() * 10;
        self.run_steps(budget);
        if !self.is_complete() {
            self.run_steps(budget);
        }
        if !self.is_complete() {
            self.emit_correct_typing();
        }
        self.state.history.clone()
    }
}
