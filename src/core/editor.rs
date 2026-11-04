//! Editor trait and implementations for different input modes.
//!
//! The editor architecture provides pluggable input handlers for different
//! modes (phonetic, punctuation, suggestions). Each editor implements the
//! `Editor` trait and processes key events in its specific context.

use crate::core::candidate::Candidate;
use crate::core::engine::{Engine, SyllableParser};
use crate::core::ime_engine::KeyEvent;
use crate::core::session::ImeSession;
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
    ModeSwitch(crate::core::session::InputMode),

    /// Key not handled by this editor, pass to parent
    PassThrough,
}

// ============================================================================
// PhoneticEditor - Phonetic input (pinyin/zhuyin)
// ============================================================================

/// Phonetic input editor (pinyin/zhuyin).
///
/// This is the main editor for Chinese character input via phonetic typing.
/// It takes raw pinyin/zhuyin input, queries the backend engine for candidates,
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
    fn handle_char<P: SyllableParser>(
        &mut self,
        ch: char,
        session: &mut ImeSession,
        backend: &mut Engine<P>,
    ) -> EditorResult {
        // Add to input buffer
        session.input_buffer_mut().insert_char(ch);

        // Update candidates
        self.update_candidates(session, backend);

        EditorResult::Handled
    }

    /// Handle backspace.
    fn handle_backspace<P: SyllableParser>(
        &mut self,
        session: &mut ImeSession,
        backend: &mut Engine<P>,
    ) -> EditorResult {
        let deleted = session.input_buffer_mut().delete_before();

        if deleted {
            if session.input_buffer().text().is_empty() {
                // No more input, reset
                EditorResult::CommitAndReset(String::new())
            } else {
                self.update_candidates(session, backend);
                EditorResult::Handled
            }
        } else {
            EditorResult::PassThrough
        }
    }

    /// Handle delete key.
    fn handle_delete<P: SyllableParser>(
        &mut self,
        session: &mut ImeSession,
        backend: &mut Engine<P>,
    ) -> EditorResult {
        let deleted = session.input_buffer_mut().delete_after();

        if deleted {
            if session.input_buffer().text().is_empty() {
                EditorResult::CommitAndReset(String::new())
            } else {
                self.update_candidates(session, backend);
                EditorResult::Handled
            }
        } else {
            EditorResult::PassThrough
        }
    }

    /// Handle space (select first candidate).
    fn handle_space<P: SyllableParser>(
        &mut self,
        session: &mut ImeSession,
        backend: &mut Engine<P>,
    ) -> EditorResult {
        if session.candidates().is_empty() {
            // No candidates, just insert space
            return EditorResult::CommitAndReset(" ".to_string());
        }

        // Select first candidate
        if let Some(candidate) = session.candidates().selected_candidate() {
            let text = candidate.text.clone();
            let input_consumed = candidate.input_consumed;

            // Learn the phrase
            backend.commit(&text);

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
    fn handle_enter<P: SyllableParser>(
        &mut self,
        session: &mut ImeSession,
        backend: &mut Engine<P>,
    ) -> EditorResult {
        if let Some(candidate) = session.candidates().selected_candidate() {
            let text = candidate.text.clone();
            let input_consumed = candidate.input_consumed;
            backend.commit(&text);
            if let Some(consumed) = input_consumed {
                EditorResult::CommitPartial(text, consumed)
            } else {
                EditorResult::CommitAndReset(text)
            }
        } else {
            // Commit raw input
            let raw = session.input_buffer().text().to_string();
            EditorResult::CommitAndReset(raw)
        }
    }

    /// Handle number key for candidate selection (1-9).
    fn handle_number<P: SyllableParser>(
        &mut self,
        n: u8,
        session: &mut ImeSession,
        backend: &mut Engine<P>,
    ) -> EditorResult {
        if session.candidates().is_empty() {
            return EditorResult::PassThrough;
        }

        if !(1..=9).contains(&n) {
            return EditorResult::PassThrough;
        }

        let index = (n - 1) as usize;
        if let Some(candidate) = session.candidates_mut().select_by_index(index) {
            let text = candidate.text.clone();
            let input_consumed = candidate.input_consumed;
            backend.commit(&text);
            if let Some(consumed) = input_consumed {
                EditorResult::CommitPartial(text, consumed)
            } else {
                EditorResult::CommitAndReset(text)
            }
        } else {
            EditorResult::PassThrough
        }
    }

    fn inline_predictions<P: SyllableParser>(
        &self,
        lambda: f32,
        top_n: usize,
        backend: &Engine<P>,
    ) -> Vec<Candidate> {
        let ctx = self.predict_context.trim();
        if ctx.is_empty() {
            return Vec::new();
        }
        let chars: Vec<char> = ctx.chars().collect();
        let max_suffix = chars.len().min(4);
        for len in (1..=max_suffix).rev() {
            let key: String = chars[chars.len() - len..].iter().collect();
            let mut preds = backend
                .model()
                .word_bigram
                .get_predictions(&key, lambda, top_n);
            if preds.is_empty() {
                continue;
            }
            // Slightly below typical top phrase scores so they sit at slots 2–3.
            return preds
                .drain(..)
                .take(top_n)
                .filter(|(w, _)| !backend.is_phrase_hidden(w))
                .map(|(w, s)| Candidate::new(w, s - 0.5))
                .collect();
        }
        Vec::new()
    }

    /// Forget the highlighted candidate (remove from user history + mask).
    fn handle_forget_word<P: SyllableParser>(
        &mut self,
        session: &mut ImeSession,
        backend: &mut Engine<P>,
    ) -> EditorResult {
        let Some(candidate) = session.candidates().selected_candidate() else {
            return EditorResult::PassThrough;
        };
        let phrase = candidate.text.clone();
        if phrase.is_empty() {
            return EditorResult::PassThrough;
        }
        backend.forget_phrase(&phrase);
        self.update_candidates(session, backend);
        // Keep composing with the same input; just drop the forgotten item.
        EditorResult::Handled
    }

    /// 以词定字: commit one character from the highlighted multi-char phrase.
    ///
    /// `char_index` is 0-based; `-1` means the last character (fcitx `]`).
    /// Consumes the same pinyin span as selecting the whole phrase (fcitx
    /// `selectCustom` behaviour) — useful when a rare character is easier to
    /// find inside a common word.
    fn handle_choose_char_from_phrase<P: SyllableParser>(
        &mut self,
        char_index: i32,
        session: &mut ImeSession,
        backend: &mut Engine<P>,
    ) -> EditorResult {
        if !backend.config().choose_char_from_phrase {
            return EditorResult::PassThrough;
        }
        let Some(candidate) = session.candidates().selected_candidate() else {
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
        backend.commit(&ch);
        if let Some(consumed) = input_consumed {
            EditorResult::CommitPartial(ch, consumed)
        } else {
            EditorResult::CommitAndReset(ch)
        }
    }

    pub fn process_key<P: SyllableParser>(
        &mut self,
        key: KeyEvent,
        session: &mut ImeSession,
        backend: &mut Engine<P>,
    ) -> EditorResult {
        match key {
            KeyEvent::Char(ch) => self.handle_char(ch, session, backend),
            KeyEvent::Backspace => self.handle_backspace(session, backend),
            KeyEvent::Delete => self.handle_delete(session, backend),
            KeyEvent::Space => self.handle_space(session, backend),
            KeyEvent::Enter => self.handle_enter(session, backend),
            KeyEvent::Number(n) => self.handle_number(n, session, backend),
            KeyEvent::ChooseCharFromPhrase(idx) => {
                self.handle_choose_char_from_phrase(idx, session, backend)
            }
            KeyEvent::ForgetWord => self.handle_forget_word(session, backend),
            KeyEvent::PinPhrase | KeyEvent::Tab => EditorResult::PassThrough,

            // Cursor navigation - update session but stay in mode
            KeyEvent::Left => {
                session.input_buffer_mut().move_left();
                self.update_candidates(session, backend);
                EditorResult::Handled
            }
            KeyEvent::Right => {
                session.input_buffer_mut().move_right();
                self.update_candidates(session, backend);
                EditorResult::Handled
            }
            KeyEvent::Home => {
                session.input_buffer_mut().move_to_start();
                self.update_candidates(session, backend);
                EditorResult::Handled
            }
            KeyEvent::End => {
                session.input_buffer_mut().move_to_end();
                self.update_candidates(session, backend);
                EditorResult::Handled
            }
            KeyEvent::Up => {
                if !session.candidates().is_empty() {
                    if !session.candidates_mut().cursor_up() {
                        // At top of page, go to previous page and select last item
                        let candidates = session.candidates_mut();
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
                if !session.candidates().is_empty() {
                    if !session.candidates_mut().cursor_down() {
                        // At bottom of page, go to next page and select first item
                        let candidates = session.candidates_mut();
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
                if !session.candidates().is_empty() {
                    session.candidates_mut().page_up();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::PageDown => {
                if !session.candidates().is_empty() {
                    session.candidates_mut().page_down();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::Escape => EditorResult::CommitAndReset(String::new()),
            // Global shortcuts handled by ImeEngine before routing
            KeyEvent::Ctrl(_) | KeyEvent::ShiftLock => EditorResult::PassThrough,
        }
    }

    pub fn update_candidates<P: SyllableParser>(
        &mut self,
        session: &mut ImeSession,
        backend: &mut Engine<P>,
    ) {
        let input = session.input_buffer().text().to_string();

        if input.is_empty() {
            session.candidates_mut().clear();
            return;
        }

        let v_on = backend.config().v_mode_enabled;
        let u_on = backend.config().u_mode_enabled;
        let inline = backend.config().inline_prediction;
        let lambda = backend.config().lambda;

        let mut candidates: Vec<Candidate> = Vec::new();

        // Windows-style v/u helpers take precedence when the buffer matches.
        if v_on && input.starts_with('v') && input.len() >= 2 {
            let rest = &input[1..];
            if rest.chars().all(|c| c.is_ascii_digit()) {
                candidates.extend(crate::core::expand_v_input(rest));
            }
        }
        if u_on && input.starts_with('u') && input.len() >= 2 {
            let rest = &input[1..];
            if rest.chars().all(|c| c.is_ascii_alphabetic()) {
                candidates.extend(crate::core::expand_u_input(rest));
            }
        }

        let helper_only = !candidates.is_empty()
            && ((v_on && input.starts_with('v')) || (u_on && input.starts_with('u')));

        if !helper_only {
            let mut backend_candidates = backend.input(&input);
            // Inline predictions (Mac-style): insert after top-1.
            if inline && !self.predict_context.is_empty() && !backend_candidates.is_empty() {
                let preds = self.inline_predictions(lambda, 2, backend);
                if !preds.is_empty() {
                    let top = backend_candidates.remove(0);
                    let mut merged = vec![top];
                    let mut seen: std::collections::HashSet<String> =
                        merged.iter().map(|c| c.text.clone()).collect();
                    for p in preds {
                        if seen.insert(p.text.clone()) {
                            merged.push(p);
                        }
                    }
                    merged.extend(backend_candidates);
                    backend_candidates = merged;
                }
            }
            // Prefer helpers first when both exist (v123 still shows 一二三 above pinyin).
            if candidates.is_empty() {
                candidates = backend_candidates;
            } else {
                let mut seen: std::collections::HashSet<String> =
                    candidates.iter().map(|c| c.text.clone()).collect();
                for c in backend_candidates {
                    if seen.insert(c.text.clone()) {
                        candidates.push(c);
                    }
                }
            }
        }

        session.candidates_mut().set_candidates(candidates);

        // Update composition with syllable-separated preedit (e.g., "wo'de"),
        // or raw typed keys when double-pinyin + show_raw is enabled.
        let input_cursor = session.input_buffer().cursor();
        let show_raw =
            backend.double_pinyin_scheme().is_some() && backend.config().show_raw_double_pinyin;
        let composition = session.composition_mut();
        if show_raw {
            composition.set_text(input.clone());
            composition.set_cursor(input_cursor);
            if !input.is_empty() {
                composition.add_segment(0..input.len(), false);
            }
        } else {
            let (preedit, cursor) = backend.preedit_with_separators(&input, input_cursor);
            composition.set_text(preedit);
            composition.set_cursor(cursor);
            composition.set_syllable_segments_from_preedit();
        }
    }

    pub fn reset(&mut self) {
        // PhoneticEditor is stateless - backend Engine handles state
    }

    pub fn name(&self) -> &'static str {
        "PhoneticEditor"
    }

    pub fn can_handle(&self, key: &KeyEvent) -> bool {
        // Lowercase always; digits only for v-mode numeral entry.
        match key {
            KeyEvent::Char(ch) => ch.is_ascii_lowercase() || ch.is_ascii_digit(),
            _ => true,
        }
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
    pub fn activate<P: SyllableParser>(
        &mut self,
        previous_text: &str,
        session: &mut ImeSession,
        backend: &mut Engine<P>,
    ) {
        self.context = previous_text.to_string();
        self.active = true;

        // Generate prediction candidates
        // Note: This is a simplified implementation. A full implementation
        // would use the n-gram model to predict next words based on context.
        self.update_candidates(session, backend);
    }

    /// Check if suggestions are currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get the current context.
    pub fn context(&self) -> &str {
        &self.context
    }

    /// Learn user bigram when a prediction is selected.
    fn learn_selection<P: SyllableParser>(&self, selected_text: &str, backend: &mut Engine<P>) {
        if !backend.config().learning_enabled || backend.input_purpose().disables_learning() {
            return;
        }
        let chars: Vec<char> = self.context.chars().collect();
        if let Some(&last_char) = chars.last() {
            let w1 = last_char.to_string();
            let selected_chars: Vec<char> = selected_text.chars().collect();
            if let Some(&first_selected) = selected_chars.first() {
                let w2 = first_selected.to_string();
                backend.userdict_mut().learn_bigram(&w1, &w2);
            }
        }
    }

    pub fn process_key<P: SyllableParser>(
        &mut self,
        key: KeyEvent,
        session: &mut ImeSession,
        backend: &mut Engine<P>,
    ) -> EditorResult {
        match key {
            // Character input - exit suggestion mode and switch to phonetic
            KeyEvent::Char(ch) if ch.is_ascii_lowercase() => {
                EditorResult::ModeSwitch(super::session::InputMode::Phonetic)
            }

            // Number selection
            KeyEvent::Number(n) => {
                if !(1..=9).contains(&n) {
                    return EditorResult::PassThrough;
                }

                let index = (n - 1) as usize;
                if let Some(candidate) = session.candidates_mut().select_by_index(index) {
                    let text = candidate.text.clone();

                    // Learn user bigram for personalization
                    self.learn_selection(&text, backend);

                    // Learn the selection in userdict
                    backend.commit(&text);

                    // Stay in suggestion mode with new context
                    self.context.push_str(&text);
                    self.update_candidates(session, backend);

                    EditorResult::Commit(text)
                } else {
                    EditorResult::PassThrough
                }
            }

            // Space - select first candidate
            KeyEvent::Space => {
                if let Some(candidate) = session.candidates().selected_candidate() {
                    let text = candidate.text.clone();

                    // Learn user bigram
                    self.learn_selection(&text, backend);

                    backend.commit(&text);

                    // Update context and stay in suggestion mode
                    self.context.push_str(&text);
                    self.update_candidates(session, backend);

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
                if !session.candidates().is_empty() {
                    session.candidates_mut().cursor_up();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::Down => {
                if !session.candidates().is_empty() {
                    session.candidates_mut().cursor_down();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::PageUp => {
                if !session.candidates().is_empty() {
                    session.candidates_mut().page_up();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }
            KeyEvent::PageDown => {
                if !session.candidates().is_empty() {
                    session.candidates_mut().page_down();
                    EditorResult::Handled
                } else {
                    EditorResult::PassThrough
                }
            }

            // Escape - exit suggestion mode
            KeyEvent::Escape => EditorResult::CommitAndReset(String::new()),

            KeyEvent::ForgetWord => {
                let Some(candidate) = session.candidates().selected_candidate() else {
                    return EditorResult::PassThrough;
                };
                let phrase = candidate.text.clone();
                backend.forget_phrase(&phrase);
                self.update_candidates(session, backend);
                EditorResult::Handled
            }

            // Any other key - exit suggestion mode and pass through
            _ => {
                self.active = false;
                EditorResult::PassThrough
            }
        }
    }

    pub fn update_candidates<P: SyllableParser>(
        &mut self,
        session: &mut ImeSession,
        backend: &mut Engine<P>,
    ) {
        if self.context.is_empty() {
            session.candidates_mut().clear();
            return;
        }

        let context = self.context.trim();
        let lambda = backend.config().lambda;

        // Mac/Windows-style 联想: try longest known suffix first (full commit →
        // multi-char tail → last char). Word-bigram keys are phrases/chars, not
        // the entire typed sentence.
        let mut combined: Vec<(String, f32)> = Vec::new();
        let chars: Vec<char> = context.chars().collect();
        let max_suffix = chars.len().min(4);
        for len in (1..=max_suffix).rev() {
            let start = chars.len() - len;
            let key: String = chars[start..].iter().collect();
            let word_predictions = backend
                .model()
                .word_bigram
                .get_predictions(&key, lambda, 12);
            for (word, score) in word_predictions {
                if backend.is_phrase_hidden(&word) {
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
            for (word, user_count) in backend.userdict().get_bigrams_after(&key) {
                if backend.is_phrase_hidden(&word) {
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
            session.candidates_mut().set_candidates(candidates);
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
            session.candidates_mut().set_candidates(candidates);
        }

        session.composition_mut().clear();
    }

    pub fn reset(&mut self) {
        self.context.clear();
        self.active = false;
    }

    pub fn name(&self) -> &'static str {
        "SuggestionEditor"
    }

    pub fn can_handle(&self, _key: &KeyEvent) -> bool {
        self.active
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

            session.candidates_mut().set_candidates(candidates);

            // Set preedit to show the original key
            session.composition_mut().preedit = key.to_string();
            // Byte offset at end of the key (ASCII here, but keep UTF-8 safe).
            session.composition_mut().cursor = key.len_utf8();

            true
        } else {
            false
        }
    }

    /// Handle selection of a punctuation candidate.
    fn select_candidate(&mut self, session: &mut ImeSession) -> Option<String> {
        session
            .candidates()
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
                if let Some(candidate) = session.candidates_mut().select_by_index(index) {
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
                session.candidates_mut().cursor_up();
                EditorResult::Handled
            }
            KeyEvent::Down => {
                session.candidates_mut().cursor_down();
                EditorResult::Handled
            }

            // Page navigation
            KeyEvent::PageUp => {
                session.candidates_mut().page_up();
                EditorResult::Handled
            }
            KeyEvent::PageDown => {
                session.candidates_mut().page_down();
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

    pub fn update_candidates(&mut self, _session: &mut ImeSession) {
        // Punctuation candidates are set on activation, no dynamic updates
    }

    pub fn reset(&mut self) {
        self.active_key = None;
    }

    pub fn name(&self) -> &'static str {
        "PunctuationEditor"
    }

    pub fn can_handle(&self, key: &KeyEvent) -> bool {
        // Can handle most keys when active
        !matches!(
            key,
            KeyEvent::Char(_) | KeyEvent::Backspace | KeyEvent::Delete
        )
    }
}
