//! Mode-specific key handlers (phonetic, punctuation, suggestion).

use super::KeyEvent;
use crate::engine::Engine;
use crate::session::{Candidate, ImeSession};
use std::collections::HashMap;

/// Result of processing a key event in an editor.
#[derive(Debug, Clone, PartialEq)]
pub enum EditorResult {
    /// Key was handled, session state updated
    Handled,

    /// Text should be committed, stay in current mode
    Commit(String),

    /// Text should be committed and mode should reset
    CommitAndReset(String),

    /// Text should be committed, remove `bytes` from input buffer start, regenerate candidates
    CommitPartial(String, usize),

    /// Request to switch to a different mode
    ModeSwitch(crate::InputMode),

    /// Key not handled by this editor, pass to parent
    PassThrough,
}

// ============================================================================
// PhoneticEditor - Phonetic input (pinyin)
// ============================================================================

/// Phonetic input editor (pinyin).
///
/// This is the main editor for Chinese character input via phonetic typing.
/// It takes raw pinyin input, queries the engine for candidates,
/// and handles selection/commit.
///
#[derive(Default)]
pub struct PhoneticEditor {
    /// Last commit context for in-composition predictions.
    predict_context: String,
}

impl PhoneticEditor {
    /// Set context text used for inline bigram predictions.
    pub fn set_predict_context(&mut self, ctx: &str) {
        self.predict_context = ctx.to_string();
    }

    /// Handle character input
    fn handle_char(
        &mut self,
        ch: char,
        session: &mut ImeSession,
        engine: &mut Engine,
    ) -> EditorResult {
        // Add to input buffer
        session.input_buffer.insert_char(ch);

        // Update candidates
        self.update_candidates(session, engine);

        EditorResult::Handled
    }

    /// Handle backspace.
    fn handle_backspace(&mut self, session: &mut ImeSession, engine: &mut Engine) -> EditorResult {
        let deleted = session.input_buffer.delete_before();

        if deleted {
            if session.input_buffer.text().is_empty() {
                // No more input, reset
                EditorResult::CommitAndReset(String::new())
            } else {
                self.update_candidates(session, engine);
                EditorResult::Handled
            }
        } else {
            EditorResult::PassThrough
        }
    }

    /// Handle delete key.
    fn handle_delete(&mut self, session: &mut ImeSession, engine: &mut Engine) -> EditorResult {
        let deleted = session.input_buffer.delete_after();

        if deleted {
            if session.input_buffer.text().is_empty() {
                EditorResult::CommitAndReset(String::new())
            } else {
                self.update_candidates(session, engine);
                EditorResult::Handled
            }
        } else {
            EditorResult::PassThrough
        }
    }

    /// Handle space (select first candidate).
    fn handle_space(&mut self, session: &mut ImeSession, engine: &mut Engine) -> EditorResult {
        if session.candidates.is_empty() {
            // No candidates, just insert space
            return EditorResult::CommitAndReset(" ".to_string());
        }

        // Select first candidate
        if let Some(candidate) = session.candidates.selected_candidate() {
            let text = candidate.text.clone();
            let input_consumed = candidate.input_consumed;

            // Learn the phrase
            engine.commit(&text);

            if let Some(consumed) = input_consumed {
                EditorResult::CommitPartial(text, consumed)
            } else {
                EditorResult::CommitAndReset(text)
            }
        } else {
            EditorResult::PassThrough
        }
    }

    /// Handle enter (commit selection or raw input).
    fn handle_enter(&mut self, session: &mut ImeSession, engine: &mut Engine) -> EditorResult {
        if let Some(candidate) = session.candidates.selected_candidate() {
            let text = candidate.text.clone();
            let input_consumed = candidate.input_consumed;
            engine.commit(&text);
            if let Some(consumed) = input_consumed {
                EditorResult::CommitPartial(text, consumed)
            } else {
                EditorResult::CommitAndReset(text)
            }
        } else {
            // Commit raw input
            let raw = session.input_buffer.text().to_string();
            EditorResult::CommitAndReset(raw)
        }
    }

    /// Handle number key for candidate selection (1-9).
    fn handle_number(
        &mut self,
        n: u8,
        session: &mut ImeSession,
        engine: &mut Engine,
    ) -> EditorResult {
        if session.candidates.is_empty() {
            return EditorResult::PassThrough;
        }

        if !(1..=9).contains(&n) {
            return EditorResult::PassThrough;
        }

        let index = (n - 1) as usize;
        if let Some(candidate) = session.candidates.select_by_index(index) {
            let text = candidate.text.clone();
            let input_consumed = candidate.input_consumed;
            engine.commit(&text);
            if let Some(consumed) = input_consumed {
                EditorResult::CommitPartial(text, consumed)
            } else {
                EditorResult::CommitAndReset(text)
            }
        } else {
            EditorResult::PassThrough
        }
    }

    fn inline_predictions(&self, lambda: f32, top_n: usize, engine: &Engine) -> Vec<Candidate> {
        let ctx = self.predict_context.trim();
        if ctx.is_empty() {
            return Vec::new();
        }
        let chars: Vec<char> = ctx.chars().collect();
        let max_suffix = chars.len().min(4);
        for len in (1..=max_suffix).rev() {
            let key: String = chars[chars.len() - len..].iter().collect();
            let mut preds = engine
                .model
                .word_bigram
                .get_predictions(&key, lambda, top_n);
            if preds.is_empty() {
                continue;
            }
            // Slightly below typical top phrase scores so they sit at slots 2–3.
            return preds
                .drain(..)
                .take(top_n)
                .filter(|(w, _)| !engine.is_phrase_hidden(w))
                .map(|(w, s)| Candidate::new(w, s - 0.5))
                .collect();
        }
        Vec::new()
    }

    /// Forget the highlighted candidate (remove from user history + mask).
    fn handle_forget_word(
        &mut self,
        session: &mut ImeSession,
        engine: &mut Engine,
    ) -> EditorResult {
        let Some(candidate) = session.candidates.selected_candidate() else {
            return EditorResult::PassThrough;
        };
        let phrase = candidate.text.clone();
        if phrase.is_empty() {
            return EditorResult::PassThrough;
        }
        engine.forget_phrase(&phrase);
        self.update_candidates(session, engine);
        // Keep composing with the same input; just drop the forgotten item.
        EditorResult::Handled
    }

    /// 以词定字: commit one character from the highlighted multi-char phrase.
    ///
    /// `char_index` is 0-based; `-1` means the last character.
    /// Consumes the same pinyin span as selecting the whole phrase — useful
    /// when a rare character is easier to find inside a common word.
    fn handle_choose_char_from_phrase(
        &mut self,
        char_index: i32,
        session: &mut ImeSession,
        engine: &mut Engine,
    ) -> EditorResult {
        if !engine.model.config.choose_char_from_phrase {
            return EditorResult::PassThrough;
        }
        let Some(candidate) = session.candidates.selected_candidate() else {
            return EditorResult::PassThrough;
        };
        let chars: Vec<char> = candidate.text.chars().collect();
        if chars.len() < 2 {
            return EditorResult::PassThrough;
        }
        let idx = if char_index < 0 {
            chars.len() - 1
        } else {
            char_index as usize
        };
        if idx >= chars.len() {
            return EditorResult::PassThrough;
        }
        let ch = chars[idx].to_string();
        let input_consumed = candidate.input_consumed;
        engine.commit(&ch);
        if let Some(consumed) = input_consumed {
            EditorResult::CommitPartial(ch, consumed)
        } else {
            EditorResult::CommitAndReset(ch)
        }
    }

    pub fn process_key(
        &mut self,
        key: KeyEvent,
        session: &mut ImeSession,
        engine: &mut Engine,
    ) -> EditorResult {
        match key {
            KeyEvent::Char(ch) => self.handle_char(ch, session, engine),
            KeyEvent::Backspace => self.handle_backspace(session, engine),
            KeyEvent::Delete => self.handle_delete(session, engine),
            KeyEvent::Space => self.handle_space(session, engine),
            KeyEvent::Enter => self.handle_enter(session, engine),
            KeyEvent::Number(n) => self.handle_number(n, session, engine),
            KeyEvent::ChooseCharFromPhrase(idx) => {
                self.handle_choose_char_from_phrase(idx, session, engine)
            }
            KeyEvent::ForgetWord => self.handle_forget_word(session, engine),
            KeyEvent::PinPhrase | KeyEvent::Tab => EditorResult::PassThrough,

            // Cursor navigation - update session but stay in mode
            KeyEvent::Left => {
                session.input_buffer.move_left();
                self.update_candidates(session, engine);
                EditorResult::Handled
            }
            KeyEvent::Right => {
                session.input_buffer.move_right();
                self.update_candidates(session, engine);
                EditorResult::Handled
            }
            KeyEvent::Home => {
                session.input_buffer.move_to_start();
                self.update_candidates(session, engine);
                EditorResult::Handled
            }
            KeyEvent::End => {
                session.input_buffer.move_to_end();
                self.update_candidates(session, engine);
                EditorResult::Handled
            }
            KeyEvent::Up => {
                if !session.candidates.is_empty() {
                    if !session.candidates.cursor_up() {
                        // At top of page, go to previous page and select last item
                        let candidates = &mut session.candidates;
                        if candidates.page_up() {
                            let page_len = candidates.current_page_candidates().len();
                            if page_len > 0 {
                                candidates.select_by_index(page_len - 1);
                            }
                        }
                    }
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::Down => {
                if !session.candidates.is_empty() {
                    if !session.candidates.cursor_down() {
                        // At bottom of page, go to next page and select first item
                        let candidates = &mut session.candidates;
                        if candidates.page_down() {
                            candidates.select_by_index(0);
                        }
                    }
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::PageUp => {
                if !session.candidates.is_empty() {
                    session.candidates.page_up();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::PageDown => {
                if !session.candidates.is_empty() {
                    session.candidates.page_down();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::Escape => EditorResult::CommitAndReset(String::new()),
            // Global shortcuts handled by Editor before routing
            KeyEvent::Ctrl(_) | KeyEvent::ShiftLock => EditorResult::PassThrough,
        }
    }

    pub fn update_candidates(&mut self, session: &mut ImeSession, engine: &mut Engine) {
        let input = session.input_buffer.text().to_string();

        if input.is_empty() {
            session.candidates.clear();
            return;
        }

        let v_on = engine.model.config.v_mode_enabled;
        let u_on = engine.model.config.u_mode_enabled;
        let inline = engine.model.config.inline_prediction;
        let lambda = engine.model.config.lambda;

        let mut candidates: Vec<Candidate> = Vec::new();

        // Windows-style v/u helpers take precedence when the buffer matches.
        if v_on && input.starts_with('v') && input.len() >= 2 {
            let rest = &input[1..];
            if rest.chars().all(|c| c.is_ascii_digit()) {
                candidates.extend(expand_v_input(rest));
            }
        }
        if u_on && input.starts_with('u') && input.len() >= 2 {
            let rest = &input[1..];
            if rest.chars().all(|c| c.is_ascii_alphabetic()) {
                candidates.extend(expand_u_input(rest));
            }
        }

        let helper_only = !candidates.is_empty()
            && ((v_on && input.starts_with('v')) || (u_on && input.starts_with('u')));

        if !helper_only {
            let mut engine_candidates = engine.input(&input);
            // Inline predictions (Mac-style): insert after top-1.
            if inline && !self.predict_context.is_empty() && !engine_candidates.is_empty() {
                let preds = self.inline_predictions(lambda, 2, engine);
                if !preds.is_empty() {
                    let top = engine_candidates.remove(0);
                    let mut merged = vec![top];
                    let mut seen: std::collections::HashSet<String> =
                        merged.iter().map(|c| c.text.clone()).collect();
                    for p in preds {
                        if seen.insert(p.text.clone()) {
                            merged.push(p);
                        }
                    }
                    merged.extend(engine_candidates);
                    engine_candidates = merged;
                }
            }
            // Prefer helpers first when both exist (v123 still shows 一二三 above pinyin).
            if candidates.is_empty() {
                candidates = engine_candidates;
            } else {
                let mut seen: std::collections::HashSet<String> =
                    candidates.iter().map(|c| c.text.clone()).collect();
                for c in engine_candidates {
                    if seen.insert(c.text.clone()) {
                        candidates.push(c);
                    }
                }
            }
        }

        session.candidates.set_candidates(candidates);

        // Update composition with syllable-separated preedit (e.g., "wo'de"),
        // or raw typed keys when double-pinyin + show_raw is enabled.
        let input_cursor = session.input_buffer.cursor();
        let show_raw =
            engine.double_pinyin_scheme.is_some() && engine.model.config.show_raw_double_pinyin;
        let composition = &mut session.composition;
        if show_raw {
            composition.set_text(input.clone());
            composition.set_cursor(input_cursor);
            if !input.is_empty() {
                composition.add_segment(0..input.len(), false);
            }
        } else {
            let (preedit, cursor) = engine.preedit_with_separators(&input, input_cursor);
            composition.set_text(preedit);
            composition.set_cursor(cursor);
            composition.set_syllable_segments_from_preedit();
        }
    }

    pub fn reset(&mut self) {
        // PhoneticEditor is mostly buffer-driven; `Engine` does lookup
    }
}

// ============================================================================
// SuggestionEditor - Post-commit predictions
// ============================================================================

/// Suggestion editor for predictive text.
///
/// After committing text, this editor can suggest likely next words based
/// on the previous context using n-gram predictions.
///
#[derive(Default)]
pub struct SuggestionEditor {
    /// Previous committed text (context for predictions)
    context: String,

    /// Whether suggestions are currently active
    active: bool,
}

impl SuggestionEditor {
    /// Activate suggestions based on previous context.
    ///
    /// This should be called after committing text to show predictions
    /// for the next word.
    pub fn activate(&mut self, previous_text: &str, session: &mut ImeSession, engine: &mut Engine) {
        self.context = previous_text.to_string();
        self.active = true;

        // Generate prediction candidates
        // Note: This is a simplified implementation. A full implementation
        // would use the n-gram model to predict next words based on context.
        self.update_candidates(session, engine);
    }

    /// Learn user bigram when a prediction is selected.
    fn learn_selection(&self, selected_text: &str, engine: &mut Engine) {
        if !engine.model.config.learning_enabled || engine.input_purpose.disables_learning() {
            return;
        }
        let chars: Vec<char> = self.context.chars().collect();
        if let Some(&last_char) = chars.last() {
            let w1 = last_char.to_string();
            let selected_chars: Vec<char> = selected_text.chars().collect();
            if let Some(&first_selected) = selected_chars.first() {
                let w2 = first_selected.to_string();
                engine.model.userdict.learn_bigram(&w1, &w2);
            }
        }
    }

    pub fn process_key(
        &mut self,
        key: KeyEvent,
        session: &mut ImeSession,
        engine: &mut Engine,
    ) -> EditorResult {
        match key {
            // Character input - exit suggestion mode and switch to phonetic
            KeyEvent::Char(ch) if ch.is_ascii_lowercase() => {
                EditorResult::ModeSwitch(crate::session::InputMode::Phonetic)
            }

            // Number selection
            KeyEvent::Number(n) => {
                if !(1..=9).contains(&n) {
                    return EditorResult::PassThrough;
                }

                let index = (n - 1) as usize;
                if let Some(candidate) = session.candidates.select_by_index(index) {
                    let text = candidate.text.clone();

                    // Learn user bigram for personalization
                    self.learn_selection(&text, engine);

                    // Learn the selection in userdict
                    engine.commit(&text);

                    // Stay in suggestion mode with new context
                    self.context.push_str(&text);
                    self.update_candidates(session, engine);

                    EditorResult::Commit(text)
                } else {
                    EditorResult::PassThrough
                }
            }

            // Space - select first candidate
            KeyEvent::Space => {
                if let Some(candidate) = session.candidates.selected_candidate() {
                    let text = candidate.text.clone();

                    // Learn user bigram
                    self.learn_selection(&text, engine);

                    engine.commit(&text);

                    // Update context and stay in suggestion mode
                    self.context.push_str(&text);
                    self.update_candidates(session, engine);

                    EditorResult::Commit(text)
                } else {
                    // No candidates, exit suggestion mode
                    EditorResult::CommitAndReset(" ".to_string())
                }
            }

            // Enter - exit suggestion mode
            KeyEvent::Enter => EditorResult::CommitAndReset(String::new()),

            // Navigation
            KeyEvent::Up => {
                if !session.candidates.is_empty() {
                    session.candidates.cursor_up();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::Down => {
                if !session.candidates.is_empty() {
                    session.candidates.cursor_down();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::PageUp => {
                if !session.candidates.is_empty() {
                    session.candidates.page_up();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::PageDown => {
                if !session.candidates.is_empty() {
                    session.candidates.page_down();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }

            // Escape - exit suggestion mode
            KeyEvent::Escape => EditorResult::CommitAndReset(String::new()),

            KeyEvent::ForgetWord => {
                let Some(candidate) = session.candidates.selected_candidate() else {
                    return EditorResult::PassThrough;
                };
                let phrase = candidate.text.clone();
                engine.forget_phrase(&phrase);
                self.update_candidates(session, engine);
                EditorResult::Handled
            }

            // Any other key - exit suggestion mode and pass through
            _ => {
                self.active = false;
                EditorResult::PassThrough
            }
        }
    }

    pub fn update_candidates(&mut self, session: &mut ImeSession, engine: &mut Engine) {
        if self.context.is_empty() {
            session.candidates.clear();
            return;
        }

        let context = self.context.trim();
        let lambda = engine.model.config.lambda;

        // Mac/Windows-style 联想: try longest known suffix first (full commit →
        // multi-char tail → last char). Word-bigram keys are phrases/chars, not
        // the entire typed sentence.
        let mut combined: Vec<(String, f32)> = Vec::new();
        let chars: Vec<char> = context.chars().collect();
        let max_suffix = chars.len().min(4);
        for len in (1..=max_suffix).rev() {
            let start = chars.len() - len;
            let key: String = chars[start..].iter().collect();
            let word_predictions = engine.model.word_bigram.get_predictions(&key, lambda, 12);
            for (word, score) in word_predictions {
                if engine.is_phrase_hidden(&word) {
                    continue;
                }
                if let Some(existing) = combined.iter_mut().find(|(w, _)| *w == word) {
                    // Prefer hits from longer (more specific) context keys.
                    if score > existing.1 {
                        existing.1 = score;
                    }
                } else {
                    combined.push((word, score));
                }
            }
            for (word, user_count) in engine.model.userdict.get_bigrams_after(&key) {
                if engine.is_phrase_hidden(&word) {
                    continue;
                }
                let user_boost = (1.0 + user_count as f32).ln() + 0.5;
                if let Some(existing) = combined.iter_mut().find(|(w, _)| *w == word) {
                    existing.1 += user_boost;
                } else {
                    combined.push((word, user_boost));
                }
            }
            // Longest suffix that produced anything is enough.
            if !combined.is_empty() {
                break;
            }
        }

        combined.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        combined.truncate(10);

        if !combined.is_empty() {
            let candidates: Vec<Candidate> = combined
                .into_iter()
                .map(|(word, score)| Candidate::new(word, score))
                .collect();
            session.candidates.set_candidates(candidates);
        } else {
            // Fallback particles (common after any commit).
            let candidates = vec![
                Candidate::new("的", 0.1),
                Candidate::new("了", 0.09),
                Candidate::new("吗", 0.08),
                Candidate::new("呢", 0.07),
                Candidate::new("吧", 0.06),
                Candidate::new("啊", 0.05),
            ];
            session.candidates.set_candidates(candidates);
        }

        session.composition.clear();
    }

    pub fn reset(&mut self) {
        self.context.clear();
        self.active = false;
    }
}

// ============================================================================
// PunctuationEditor - Full-width punctuation selection
// ============================================================================

/// Punctuation editor for full-width character selection.
///
/// When the user types a punctuation key (like comma or period) during
/// Chinese input, this editor shows a list of full-width alternatives
/// to choose from.
pub struct PunctuationEditor {
    /// Map from ASCII punct to full-width alternatives
    punct_map: HashMap<char, Vec<&'static str>>,

    /// Currently active punctuation key (if any)
    active_key: Option<char>,
}

impl Default for PunctuationEditor {
    fn default() -> Self {
        let mut punct_map = HashMap::new();

        // Comma variants
        punct_map.insert(',', vec!["，", ",", "、", "﹐", "﹑"]);

        // Period variants
        punct_map.insert('.', vec!["。", ".", "·", "﹒", "．"]);

        // Semicolon variants
        punct_map.insert(';', vec!["；", ";", "﹔"]);

        // Colon variants
        punct_map.insert(':', vec!["：", ":", "﹕"]);

        // Question mark variants
        punct_map.insert('?', vec!["？", "?", "﹖"]);

        // Exclamation mark variants
        punct_map.insert('!', vec!["！", "!", "﹗"]);

        // Quote variants
        punct_map.insert('"', vec!["\u{201C}", "\u{201D}", "\"", "＂"]); // ""
        punct_map.insert('\'', vec!["\u{2018}", "\u{2019}", "'", "＇"]); // ''

        // Parentheses
        punct_map.insert('(', vec!["（", "(", "﹙"]);
        punct_map.insert(')', vec!["）", ")", "﹚"]);

        // Brackets
        punct_map.insert('[', vec!["【", "[", "［"]);
        punct_map.insert(']', vec!["】", "]", "］"]);

        // Braces
        punct_map.insert('{', vec!["｛", "{", "「", "『"]);
        punct_map.insert('}', vec!["｝", "}", "」", "』"]);

        // Dash/Hyphen
        punct_map.insert('-', vec!["—", "–", "-", "－"]);

        // Ellipsis
        punct_map.insert('~', vec!["～", "…", "~"]);

        Self {
            punct_map,
            active_key: None,
        }
    }
}

impl PunctuationEditor {
    /// Check if a character has punctuation alternatives.
    pub fn has_alternatives(&self, ch: char) -> bool {
        self.punct_map.contains_key(&ch)
    }

    /// Activate punctuation selection for a given key.
    pub fn activate(&mut self, key: char, session: &mut ImeSession) -> bool {
        if let Some(alternatives) = self.punct_map.get(&key) {
            self.active_key = Some(key);

            // Set candidates
            let candidates: Vec<Candidate> = alternatives
                .iter()
                .map(|&s| Candidate::new(s, 1.0))
                .collect();

            session.candidates.set_candidates(candidates);

            // Set preedit to show the original key
            session.composition.preedit = key.to_string();
            // Byte offset at end of the key (ASCII here, but keep UTF-8 safe).
            session.composition.cursor = key.len_utf8();

            true
        } else {
            false
        }
    }

    /// Handle selection of a punctuation candidate.
    fn select_candidate(&mut self, session: &mut ImeSession) -> Option<String> {
        session
            .candidates
            .selected_candidate()
            .map(|candidate| candidate.text.clone())
    }

    pub fn process_key(&mut self, key: KeyEvent, session: &mut ImeSession) -> EditorResult {
        match key {
            // Number selection
            KeyEvent::Number(n) => {
                if !(1..=9).contains(&n) {
                    return EditorResult::PassThrough;
                }

                let index = (n - 1) as usize;
                if let Some(candidate) = session.candidates.select_by_index(index) {
                    let text = candidate.text.clone();
                    EditorResult::CommitAndReset(text)
                } else {
                    EditorResult::PassThrough
                }
            }

            // Space or Enter - select first candidate
            KeyEvent::Space | KeyEvent::Enter => {
                if let Some(text) = self.select_candidate(session) {
                    EditorResult::CommitAndReset(text)
                } else {
                    EditorResult::PassThrough
                }
            }

            // Up/Down for candidate navigation
            KeyEvent::Up => {
                session.candidates.cursor_up();
                EditorResult::Handled
            }
            KeyEvent::Down => {
                session.candidates.cursor_down();
                EditorResult::Handled
            }

            // Page navigation
            KeyEvent::PageUp => {
                session.candidates.page_up();
                EditorResult::Handled
            }
            KeyEvent::PageDown => {
                session.candidates.page_down();
                EditorResult::Handled
            }

            // Escape - cancel and use original character
            KeyEvent::Escape => {
                if let Some(key) = self.active_key {
                    EditorResult::CommitAndReset(key.to_string())
                } else {
                    EditorResult::CommitAndReset(String::new())
                }
            }

            // Any other key - commit first candidate and pass through
            _ => {
                if let Some(text) = self.select_candidate(session) {
                    // Commit the punctuation and indicate pass through for the new key
                    EditorResult::Commit(text)
                } else {
                    EditorResult::PassThrough
                }
            }
        }
    }

    pub fn reset(&mut self) {
        self.active_key = None;
    }
}

/// Preferred Chinese punctuation for an ASCII key (Mac/Windows 中-mode default).
///
/// Returns the primary form that should be committed immediately without a picker.
pub fn preferred_chinese_punct(ch: char) -> Option<&'static str> {
    match ch {
        ',' => Some("，"),
        '.' => Some("。"),
        ';' => Some("；"),
        ':' => Some("："),
        '?' => Some("？"),
        '!' => Some("！"),
        '"' => Some("\u{201C}"),
        '\'' => Some("\u{2018}"),
        '(' => Some("（"),
        ')' => Some("）"),
        '[' => Some("【"),
        ']' => Some("】"),
        '{' => Some("「"),
        '}' => Some("」"),
        '<' => Some("《"),
        '>' => Some("》"),
        '~' => Some("～"),
        '\\' => Some("、"),
        '^' => Some("……"),
        '$' => Some("￥"),
        _ => None,
    }
}

// ========== Windows-style `v` / `u` input helpers (numerals, dates, symbols) ==========

const DIGITS_CN: &[char] = &['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'];

fn digit_to_cn(d: u8) -> Option<char> {
    if d <= 9 {
        Some(DIGITS_CN[d as usize])
    } else {
        None
    }
}

fn digits_to_chinese(s: &str) -> Option<String> {
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut out = String::with_capacity(s.len() * 3);
    for c in s.chars() {
        let d = (c as u8) - b'0';
        out.push(digit_to_cn(d)?);
    }
    Some(out)
}

fn year_forms(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(cn) = digits_to_chinese(s) {
        out.push(cn.clone());
        out.push(format!("{cn}年"));
        if s.len() == 4 {
            out.push(format!("{s}年"));
        }
    }
    out
}

/// Expand `v`-mode remainder (buffer after leading `v`) into helper candidates.
fn expand_v_input(rest: &str) -> Vec<Candidate> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Vec::new();
    }
    let mut texts: Vec<String> = Vec::new();
    if rest.chars().all(|c| c.is_ascii_digit()) {
        if let Some(cn) = digits_to_chinese(rest) {
            texts.push(cn);
        }
        if rest.len() == 4 {
            texts.extend(year_forms(rest));
        } else if rest.len() == 8 {
            // YYYYMMDD → 二〇二六年十月四日-style + numeric date
            let y = &rest[0..4];
            let m = &rest[4..6];
            let d = &rest[6..8];
            if let (Ok(mi), Ok(di)) = (m.parse::<u32>(), d.parse::<u32>()) {
                if (1..=12).contains(&mi) && (1..=31).contains(&di) {
                    texts.push(format!("{y}年{mi}月{di}日"));
                    if let (Some(yc), Some(mc), Some(dc)) = (
                        digits_to_chinese(y),
                        digits_to_chinese(&mi.to_string()),
                        digits_to_chinese(&di.to_string()),
                    ) {
                        texts.push(format!("{yc}年{mc}月{dc}日"));
                    }
                }
            }
        }
    }
    // Dedup preserving order
    let mut seen = std::collections::HashSet::new();
    texts
        .into_iter()
        .filter(|t| seen.insert(t.clone()))
        .enumerate()
        .map(|(i, t)| Candidate::new(t, 10.0 - i as f32 * 0.1))
        .collect()
}

/// Small offline symbol table for `u`-mode (code → symbol).
fn u_symbol(code: &str) -> Option<&'static str> {
    match code.to_ascii_lowercase().as_str() {
        "heart" | "xin" => Some("♥"),
        "star" | "xing" => Some("★"),
        "smile" => Some("☺"),
        "arrow" | "right" => Some("→"),
        "left" => Some("←"),
        "up" => Some("↑"),
        "down" => Some("↓"),
        "check" | "ok" => Some("✓"),
        "cross" | "x" => Some("✗"),
        "dot" => Some("·"),
        "deg" | "degree" => Some("°"),
        "celsius" | "sheshi" => Some("℃"),
        "euro" => Some("€"),
        "pound" => Some("£"),
        "yen" | "rmb" | "yuan" => Some("¥"),
        "copy" | "copyright" => Some("©"),
        "tm" => Some("™"),
        "reg" => Some("®"),
        "infty" | "inf" => Some("∞"),
        "neq" => Some("≠"),
        "leq" => Some("≤"),
        "geq" => Some("≥"),
        "plusminus" | "pm" => Some("±"),
        "multiply" | "times" => Some("×"),
        "divide" => Some("÷"),
        "section" => Some("§"),
        "para" | "pilcrow" => Some("¶"),
        "bullet" => Some("•"),
        "ellipsis" | "ddd" => Some("…"),
        "dash" | "em" => Some("—"),
        "square" => Some("■"),
        "circle" => Some("●"),
        "diamond" => Some("◆"),
        "spade" => Some("♠"),
        "club" => Some("♣"),
        "diamond2" => Some("♦"),
        "male" => Some("♂"),
        "female" => Some("♀"),
        "sun" => Some("☀"),
        "moon" => Some("☾"),
        "cloud" => Some("☁"),
        "music" => Some("♪"),
        "phone" => Some("☎"),
        "email" | "mail" => Some("✉"),
        "warn" => Some("⚠"),
        "info" => Some("ℹ"),
        _ => None,
    }
}

/// Expand `u`-mode remainder into symbol candidates (exact + prefix matches).
fn expand_u_input(rest: &str) -> Vec<Candidate> {
    let rest = rest.trim().to_ascii_lowercase();
    if rest.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Some(sym) = u_symbol(&rest) {
        out.push(Candidate::new(sym, 20.0));
    }
    // Prefix suggestions from a fixed code list
    const CODES: &[&str] = &[
        "heart", "star", "smile", "arrow", "left", "up", "down", "check", "cross", "dot", "degree",
        "celsius", "euro", "pound", "yen", "yuan", "copy", "tm", "reg", "infty", "neq", "leq",
        "geq", "pm", "times", "divide", "section", "bullet", "ellipsis", "dash", "square",
        "circle", "diamond", "spade", "club", "music", "phone", "email", "warn",
    ];
    for code in CODES {
        if code.starts_with(&rest) && *code != rest.as_str() {
            if let Some(sym) = u_symbol(code) {
                out.push(Candidate::new(sym, 5.0));
            }
        }
        if out.len() >= 8 {
            break;
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|c| seen.insert(c.text.clone()));
    out.truncate(8);
    out
}
