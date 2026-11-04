//! IME engine with session management and key event processing.
//!
//! The `ImeEngine` wraps the backend `Engine` with session state management,
//! providing a `process_key()` method that handles key events and maintains
//! IME state across multiple interactions. It uses a pluggable editor
//! architecture to support different input modes (phonetic, punctuation, suggestion).

use super::context::{ImeContext, InputPurpose};
use super::editor::{EditorResult, PhoneticEditor, PunctuationEditor, SuggestionEditor};
use super::session::{ImeSession, InputMode};
use crate::core::engine::{Engine, SyllableParser};

/// Key event types that the IME can process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyEvent {
    /// Character input (a-z, punctuation, etc.)
    Char(char),
    /// Backspace key
    Backspace,
    /// Delete key
    Delete,
    /// Left arrow key
    Left,
    /// Right arrow key
    Right,
    /// Home — move preedit caret to start (fcitx/ibus).
    Home,
    /// End — move preedit caret to end.
    End,
    /// Up arrow key (candidate cursor up)
    Up,
    /// Down arrow key (candidate cursor down)
    Down,
    /// Page up (candidate page up)
    PageUp,
    /// Page down (candidate page down)
    PageDown,
    /// Space key (select first candidate or commit)
    Space,
    /// Enter/Return key (commit current selection)
    Enter,
    /// Escape key (clear/cancel)
    Escape,
    /// Number key for candidate selection (1-9)
    Number(u8),
    /// 以词定字: take the N-th character (0-based) of the highlighted phrase.
    /// Use `-1` for the last character (fcitx `]`).
    ChooseCharFromPhrase(i32),
    /// Forget the highlighted candidate (fcitx Ctrl+7).
    ForgetWord,
    /// Pin highlighted candidate as a custom user phrase (fcitx Ctrl+8).
    PinPhrase,
    /// Tab: cycle Mandarin tone filter (Mac-style).
    Tab,
    /// Ctrl + character (e.g., Ctrl+period for punctuation toggle)
    Ctrl(char),
    /// Shift lock toggle (for passthrough mode)
    ShiftLock,
}

/// Result of processing a key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyResult {
    /// Key was handled by the IME
    Handled,
    /// Key was not handled (pass through to application)
    NotHandled,
}

/// IME engine with session management.
///
/// This struct combines the backend Engine with a session that tracks
/// input state across multiple key events. It uses pluggable editors
/// for different input modes (phonetic, punctuation, suggestion).
pub struct ImeEngine<P: SyllableParser> {
    /// Backend linguistic engine
    backend: Engine<P>,

    /// Phonetic input editor
    phonetic_editor: PhoneticEditor,

    /// Punctuation selection editor
    punct_editor: PunctuationEditor,

    /// Suggestion/prediction editor
    suggestion_editor: SuggestionEditor,

    /// Session state
    session: ImeSession,

    /// Context for platform communication
    context: ImeContext,

    /// Durable English / Latin passthrough mode (ibus-libpinyin `modeChinese == false`).
    /// Survives composition `reset()` so Shift 中/英 stays sticky across focus clears.
    passthrough: bool,

    /// Last committed text (for in-composition predictions).
    last_commit: String,

    /// Tone filter: None = all, Some(1..=4) = keep candidates with that first-char tone.
    tone_filter: Option<u8>,

    /// Character → tone map for Tab filter.
    tone_map: crate::core::ToneMap,
}

impl<P: SyllableParser> ImeEngine<P> {
    /// Create a new IME engine with the given backend.
    pub fn new(backend: Engine<P>) -> Self {
        Self {
            backend,
            phonetic_editor: PhoneticEditor::default(),
            punct_editor: PunctuationEditor::default(),
            suggestion_editor: SuggestionEditor::default(),
            session: ImeSession::with_page_size(5),
            context: ImeContext::default(),
            passthrough: false,
            last_commit: String::new(),
            tone_filter: None,
            tone_map: crate::core::ToneMap::default(),
        }
    }

    /// Create an IME engine with specified candidate page size.
    pub fn with_page_size(backend: Engine<P>, page_size: usize) -> Self {
        let mut engine = Self::new(backend);
        engine.session = ImeSession::with_page_size(page_size);
        engine
    }

    /// Access the linguistic backend.
    pub fn backend(&self) -> &Engine<P> {
        &self.backend
    }

    /// Mutable access to the linguistic backend.
    pub fn backend_mut(&mut self) -> &mut Engine<P> {
        &mut self.backend
    }

    /// Get a reference to the context for reading IME state.
    pub fn context(&self) -> &ImeContext {
        &self.context
    }

    /// Get a mutable reference to the context.
    pub fn context_mut(&mut self) -> &mut ImeContext {
        &mut self.context
    }

    /// Get a reference to the session.
    pub fn session(&self) -> &ImeSession {
        &self.session
    }

    /// Reset composition state. Passthrough (English) mode is preserved.
    pub fn reset(&mut self) {
        self.session.clear();
        self.context.clear();
        self.phonetic_editor.reset();
        self.punct_editor.reset();
        self.tone_filter = None;
        // Note: Don't reset suggestion_editor as it may be about to activate
        // Note: passthrough survives — like ibus-libpinyin modeChinese across reset
        if self.passthrough {
            self.session.set_mode(InputMode::Passthrough);
            self.context.auxiliary_text = "英".to_string();
        }
    }

    /// Whether Latin/English passthrough is active (keys should not form pinyin).
    pub fn is_passthrough(&self) -> bool {
        self.passthrough
    }

    /// Enter or leave English passthrough. Clears any active composition.
    pub fn set_passthrough(&mut self, enabled: bool) {
        self.passthrough = enabled;
        // Drop in-progress phonetic/punct/suggestion state.
        self.session.clear();
        self.context.clear();
        self.phonetic_editor.reset();
        self.punct_editor.reset();
        if enabled {
            self.session.set_mode(InputMode::Passthrough);
            self.context.auxiliary_text = "英".to_string();
        }
    }

    pub fn set_input_purpose(&mut self, purpose: InputPurpose) {
        let prev = self.context.input_purpose;
        self.context.input_purpose = purpose;
        self.backend.set_input_purpose(purpose);
        if purpose.is_private() && prev != purpose {
            self.session.clear();
            self.context.clear();
            self.context.input_purpose = purpose;
            self.phonetic_editor.reset();
            self.punct_editor.reset();
            self.tone_filter = None;
        }
    }

    pub fn input_purpose(&self) -> InputPurpose {
        self.context.input_purpose
    }

    /// Maybe enter suggestion mode automatically after a commit.
    ///
    /// This checks configuration settings to determine if auto-suggestion
    /// should be triggered based on the committed text.
    fn maybe_auto_suggest(&mut self, committed_text: &str) {
        // Skip if text is empty
        if committed_text.is_empty() {
            return;
        }

        if self.context.input_purpose.disables_learning() {
            return;
        }

        let auto_suggestion = self.backend.config().auto_suggestion;
        let min_len = self.backend.config().min_suggestion_trigger_length;
        if !auto_suggestion {
            return;
        }
        let should_activate = committed_text.chars().count() >= min_len;
        if !should_activate {
            return;
        }

        // Activate suggestion mode
        self.session.activate();
        self.session.set_mode(InputMode::Suggestion);
        self.suggestion_editor
            .activate(committed_text, &mut self.session, &mut self.backend);
        self.session.sync_to_context(&mut self.context);
        self.update_auxiliary_text();
    }

    /// Process a key event and update IME state.
    ///
    /// This is the main entry point for IME interaction. After calling this,
    /// the platform should read `context()` to update the UI (preedit,
    /// candidates, commit text).
    ///
    /// Returns `KeyResult::Handled` if the IME consumed the key,
    /// or `KeyResult::NotHandled` if it should pass through to the application.
    pub fn process_key(&mut self, key: KeyEvent) -> KeyResult {
        // Clear commit text from previous key
        self.context.commit_text.clear();

        if self.context.input_purpose.is_private() {
            if matches!(key, KeyEvent::ShiftLock) {
                self.set_passthrough(!self.passthrough);
                return KeyResult::Handled;
            }
            return KeyResult::NotHandled;
        }

        // Translate selection key characters to Number events
        // This allows configurable selection keys (e.g., asdfghjkl vs 123456789).
        // Exception: in Windows-style v-mode, digit keys extend the numeral buffer
        // (v123 → 一二三); select with Space/Enter instead.
        let key = if let KeyEvent::Char(ch) = key {
            let v_mode = self.backend.config().v_mode_enabled;
            let v_digit_entry = v_mode
                && ch.is_ascii_digit()
                && self.session.mode() == InputMode::Phonetic
                && self.session.input_buffer().text().starts_with('v');
            if !v_digit_entry {
                if let Some(index) = self.backend.config().selection_key_index(ch) {
                    // Convert to 1-based number (index 0 → number 1, etc.)
                    KeyEvent::Number((index + 1) as u8)
                } else {
                    key
                }
            } else {
                key
            }
        } else {
            key
        };

        // Handle global shortcuts first (before mode routing)
        match key {
            KeyEvent::ShiftLock => {
                self.set_passthrough(!self.passthrough);
                return KeyResult::Handled;
            }
            KeyEvent::Ctrl('.') => {
                // Ctrl+period: toggle punctuation mode
                // But not in passthrough mode
                if self.passthrough {
                    return KeyResult::NotHandled;
                }

                let was_phonetic = self.session.mode() == InputMode::Phonetic;

                if was_phonetic {
                    // Commit current preedit if any
                    if !self.session.input_buffer().is_empty() {
                        let text = self.session.input_buffer().text().to_string();
                        self.context.commit_text = text;
                    }
                    self.reset();
                    // After reset from phonetic, we're done (stay in Init)
                    return KeyResult::Handled;
                }

                // Toggle: if in punctuation, go to init; else go to punctuation
                if self.session.mode() == InputMode::Punctuation {
                    self.reset();
                } else {
                    self.session.set_mode(InputMode::Punctuation);
                    self.session.activate();
                }

                self.session.sync_to_context(&mut self.context);
                self.update_auxiliary_text();
                return KeyResult::Handled;
            }
            _ => {}
        }

        // Passthrough mode: ignore all other keys (client types Latin directly)
        if self.passthrough {
            return KeyResult::NotHandled;
        }

        // Route to appropriate editor based on current mode
        let result = match self.session.mode() {
            InputMode::Init => {
                let is_phonetic_input = matches!(key, KeyEvent::Char(ch) if
                    ch.is_ascii_lowercase()
                    || ('\u{3105}'..='\u{3129}').contains(&ch)
                    || matches!(ch, 'ˊ' | 'ˇ' | 'ˋ' | '˙')
                );

                if is_phonetic_input {
                    self.session.activate();
                    self.session.set_mode(InputMode::Phonetic);
                    self.phonetic_editor.set_predict_context(&self.last_commit);
                    self.phonetic_editor
                        .process_key(key, &mut self.session, &mut self.backend)
                } else if let KeyEvent::Char(ch) = key {
                    let want_cn = self.backend.config().chinese_punctuation;
                    if want_cn {
                        if let Some(p) = crate::core::preferred_chinese_punct(ch) {
                            EditorResult::CommitAndReset(p.to_string())
                        } else if self.punct_editor.has_alternatives(ch) {
                            self.session.activate();
                            self.session.set_mode(InputMode::Punctuation);
                            self.punct_editor.activate(ch, &mut self.session);
                            EditorResult::Handled
                        } else {
                            EditorResult::PassThrough
                        }
                    } else if self.punct_editor.has_alternatives(ch) {
                        self.session.activate();
                        self.session.set_mode(InputMode::Punctuation);
                        self.punct_editor.activate(ch, &mut self.session);
                        EditorResult::Handled
                    } else {
                        EditorResult::PassThrough
                    }
                } else {
                    EditorResult::PassThrough
                }
            }
            InputMode::Phonetic => {
                match &key {
                    KeyEvent::Tab => {
                        self.cycle_tone_filter();
                        self.apply_tone_filter_to_session();
                        self.phonetic_editor.set_predict_context(&self.last_commit);
                        // Re-query so filter applies on fresh list
                        self.phonetic_editor
                            .update_candidates(&mut self.session, &mut self.backend);
                        self.apply_tone_filter_to_session();
                        EditorResult::Handled
                    }
                    KeyEvent::PinPhrase => self.handle_pin_phrase(),
                    KeyEvent::Char(ch)
                        if self.backend.config().chinese_punctuation
                            && crate::core::preferred_chinese_punct(*ch).is_some() =>
                    {
                        self.commit_default_then_punct(*ch)
                    }
                    _ => {
                        self.phonetic_editor.set_predict_context(&self.last_commit);
                        let result = self.phonetic_editor.process_key(
                            key,
                            &mut self.session,
                            &mut self.backend,
                        );
                        self.apply_tone_filter_to_session();
                        result
                    }
                }
            }
            InputMode::Punctuation => self.punct_editor.process_key(key, &mut self.session),
            InputMode::Suggestion => {
                let result = self.suggestion_editor.process_key(
                    key.clone(),
                    &mut self.session,
                    &mut self.backend,
                );
                // If suggestion editor switches to phonetic mode, re-process the key
                match &result {
                    EditorResult::ModeSwitch(mode) => {
                        let mode = *mode;
                        self.suggestion_editor.reset();
                        self.session.set_mode(mode);
                        self.session.candidates_mut().clear();
                        if mode == InputMode::Phonetic {
                            self.phonetic_editor.process_key(
                                key,
                                &mut self.session,
                                &mut self.backend,
                            )
                        } else {
                            result
                        }
                    }
                    EditorResult::CommitAndReset(s) if s.is_empty() => {
                        let is_phonetic = matches!(key, KeyEvent::Char(ch) if
                            ch.is_ascii_lowercase()
                            || ('\u{3105}'..='\u{3129}').contains(&ch)
                            || matches!(ch, 'ˊ' | 'ˇ' | 'ˋ' | '˙')
                        );
                        if is_phonetic {
                            // Reset and re-process as Init → Phonetic
                            self.reset();
                            self.session.activate();
                            self.session.set_mode(InputMode::Phonetic);
                            self.phonetic_editor.process_key(
                                key,
                                &mut self.session,
                                &mut self.backend,
                            )
                        } else {
                            result
                        }
                    }
                    EditorResult::PassThrough => {
                        // Suggestion editor deactivated itself, clean up
                        self.reset();
                        EditorResult::PassThrough
                    }
                    _ => result,
                }
            }
            InputMode::Passthrough => {
                // Unreachable: passthrough handled before match
                unreachable!("Passthrough mode should be handled before routing")
            }
        };

        // Handle editor result
        match result {
            EditorResult::Handled => {
                // Sync session to context
                self.session.sync_to_context(&mut self.context);
                self.update_auxiliary_text();
                KeyResult::Handled
            }
            EditorResult::Commit(text) => {
                // Apply full-width conversion if enabled
                let text = if self.backend.config().is_fullwidth() {
                    crate::core::utils::to_fullwidth(&text)
                } else {
                    text
                };

                // Commit but stay active
                if !text.is_empty() {
                    self.last_commit.push_str(&text);
                }
                self.context.commit_text = text.clone();
                self.session.sync_to_context(&mut self.context);
                self.update_auxiliary_text();

                // Auto-enter suggestion mode if enabled and text meets criteria
                self.maybe_auto_suggest(&text);

                KeyResult::Handled
            }
            EditorResult::CommitAndReset(text) => {
                // Apply full-width conversion if enabled
                let text = if self.backend.config().is_fullwidth() {
                    crate::core::utils::to_fullwidth(&text)
                } else {
                    text
                };

                // Commit and prepare for auto-suggestion
                let committed_text = text.clone();
                if !text.is_empty() {
                    self.last_commit = text.clone();
                    self.context.commit_text = text;
                }
                self.reset();

                // Auto-enter suggestion mode after reset if enabled
                self.maybe_auto_suggest(&committed_text);

                // No auxiliary text after reset (inactive)
                KeyResult::Handled
            }
            EditorResult::CommitPartial(text, bytes_consumed) => {
                // Apply full-width conversion if enabled
                let text = if self.backend.config().is_fullwidth() {
                    crate::core::utils::to_fullwidth(&text)
                } else {
                    text
                };

                // Commit the partial text
                if !text.is_empty() {
                    self.last_commit.push_str(&text);
                    self.context.commit_text = text;
                }

                // Remove consumed bytes from input buffer and regenerate candidates
                self.session.input_buffer_mut().remove_front(bytes_consumed);
                self.phonetic_editor.set_predict_context(&self.last_commit);
                self.phonetic_editor
                    .update_candidates(&mut self.session, &mut self.backend);
                self.apply_tone_filter_to_session();
                self.session.sync_to_context(&mut self.context);
                self.update_auxiliary_text();

                KeyResult::Handled
            }
            EditorResult::ModeSwitch(mode) => {
                // Switch to new mode
                self.session.set_mode(mode);
                self.session.sync_to_context(&mut self.context);
                self.update_auxiliary_text();
                KeyResult::Handled
            }
            EditorResult::PassThrough => KeyResult::NotHandled,
        }
    }

    /// Compact status hints (tone filter, punctuation mode). Page numbers live
    /// in [`ImeContext::page_text`] and are not overwritten here.
    fn update_auxiliary_text(&mut self) {
        if self.passthrough {
            self.context.auxiliary_text.clear();
            return;
        }
        if !self.session.is_active() {
            self.context.auxiliary_text.clear();
            return;
        }

        let aux_text = match self.session.mode() {
            InputMode::Init => String::new(),
            InputMode::Phonetic => match self.tone_filter {
                Some(t) => format!("{t}声"),
                None => String::new(),
            },
            InputMode::Punctuation => "标点".to_string(),
            InputMode::Suggestion => "预测".to_string(),
            InputMode::Passthrough => String::new(),
        };

        self.context.auxiliary_text = aux_text;
    }

    /// Active Mandarin tone filter (1–4), or `None` for all tones.
    pub fn tone_filter(&self) -> Option<u8> {
        self.tone_filter
    }

    fn cycle_tone_filter(&mut self) {
        self.tone_filter = match self.tone_filter {
            None => Some(1),
            Some(1) => Some(2),
            Some(2) => Some(3),
            Some(3) => Some(4),
            Some(_) => None,
        };
    }

    fn apply_tone_filter_to_session(&mut self) {
        let Some(tone) = self.tone_filter else {
            return;
        };
        let map = &self.tone_map;
        let mut kept = Vec::new();
        for c in self.session.candidates().candidates() {
            match map.first_han_tone(&c.text) {
                Some(t) if t == tone => kept.push(c.clone()),
                None => kept.push(c.clone()), // unknown tone: keep
                _ => {}
            }
        }
        if !kept.is_empty() {
            self.session.candidates_mut().set_candidates(kept);
        }
    }

    fn handle_pin_phrase(&mut self) -> EditorResult {
        let Some(candidate) = self.session.candidates().selected_candidate() else {
            return EditorResult::PassThrough;
        };
        let phrase = candidate.text.clone();
        if phrase.is_empty() {
            return EditorResult::PassThrough;
        }
        // High boost so the phrase ranks first next time.
        let _ = self.backend.userdict_mut().add_phrase(&phrase, 500);
        self.backend.clear_cache();
        self.phonetic_editor.set_predict_context(&self.last_commit);
        self.phonetic_editor
            .update_candidates(&mut self.session, &mut self.backend);
        self.apply_tone_filter_to_session();
        EditorResult::Handled
    }

    fn commit_default_then_punct(&mut self, punct_key: char) -> EditorResult {
        let Some(punct) = crate::core::preferred_chinese_punct(punct_key) else {
            return EditorResult::PassThrough;
        };
        let mut out = String::new();
        if let Some(candidate) = self.session.candidates().selected_candidate() {
            let text = candidate.text.clone();
            self.backend.commit(&text);
            out.push_str(&text);
        } else if !self.session.input_buffer().is_empty() {
            // No candidates: commit raw buffer then punct (rare).
            out.push_str(self.session.input_buffer().text());
        }
        out.push_str(punct);
        EditorResult::CommitAndReset(out)
    }

    // ========== Configuration Management API ==========

    /// Toggle full-width mode on/off.
    pub fn toggle_fullwidth(&mut self) {
        self.backend.config_mut().toggle_fullwidth();
    }

    /// Set full-width mode explicitly.
    pub fn set_fullwidth(&mut self, enabled: bool) {
        self.backend.config_mut().set_fullwidth(enabled);
    }

    /// Check if full-width mode is enabled.
    pub fn is_fullwidth(&self) -> bool {
        self.backend.config().is_fullwidth()
    }

    /// Set the selection keys string (e.g., "asdfghjkl" or "123456789").
    pub fn set_select_keys(&mut self, keys: &str) {
        self.backend.config_mut().set_select_keys(keys);
    }

    /// Set the number of candidates displayed per page.
    pub fn set_page_size(&mut self, page_size: usize) {
        self.session.candidates_mut().set_page_size(page_size);
    }

    /// Get the current selection keys.
    pub fn get_select_keys(&self) -> String {
        self.backend.config().get_select_keys().to_string()
    }

    /// Add a phrase to the mask list (hide from suggestions).
    /// Persists via userdict and session config.
    pub fn mask_phrase(&mut self, phrase: &str) {
        self.backend.mask_phrase(phrase);
    }

    /// Remove a phrase from the mask list (allow in suggestions).
    /// Clears persisted userdict mask and session config.
    pub fn unmask_phrase(&mut self, phrase: &str) -> bool {
        let was = self.backend.is_phrase_hidden(phrase);
        self.backend.unmask_phrase(phrase);
        was
    }

    /// Check if a phrase is masked (session or persisted).
    pub fn is_masked(&self, phrase: &str) -> bool {
        self.backend.is_phrase_hidden(phrase)
    }

    /// Get all masked phrases (session ∪ persisted).
    pub fn get_masked_phrases(&self) -> Vec<String> {
        let mut set: std::collections::HashSet<String> = self
            .backend
            .config()
            .get_masked_phrases()
            .into_iter()
            .collect();
        for p in self.backend.userdict().list_masked() {
            set.insert(p);
        }
        let mut phrases: Vec<_> = set.into_iter().collect();
        phrases.sort();
        phrases
    }

    pub fn clear_masked_phrases(&mut self) {
        self.backend.clear_masked_phrases();
    }
}
