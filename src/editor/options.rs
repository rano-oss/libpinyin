//! Batch runtime options for [`Editor`] (chewing-style single blob).

use super::Editor;
use crate::context::InputPurpose;
use crate::model::CandidateRankMode;

/// All runtime editor / conversion toggles in one place.
///
/// Snapshot with [`Editor::editor_options`], apply with [`Editor::set_editor_options`].
#[derive(Debug, Clone, PartialEq)]
pub struct EditorOptions {
    pub candidates_per_page: usize,
    pub select_keys: String,
    pub full_width: bool,
    /// English / passthrough (英) mode.
    pub passthrough: bool,
    pub chinese_punctuation: bool,
    pub auto_suggestion: bool,
    pub min_suggestion_trigger_length: usize,
    pub learning_enabled: bool,
    pub v_mode_enabled: bool,
    pub u_mode_enabled: bool,
    pub inline_prediction: bool,
    pub show_raw_double_pinyin: bool,
    pub choose_char_from_phrase: bool,
    pub emoji_enabled: bool,
    pub english_enabled: bool,
    pub allow_fuzzy: bool,
    pub sort_by_pinyin_length: bool,
    pub double_pinyin_scheme: Option<String>,
    pub emit_composed_sentences: bool,
    pub candidate_rank_mode: CandidateRankMode,
    pub input_purpose: InputPurpose,
    /// Fuzzy rules written to `engine.model.config.fuzzy`.
    pub fuzzy: Vec<String>,
    pub incomplete_penalty: i32,
}

impl Default for EditorOptions {
    fn default() -> Self {
        Self {
            candidates_per_page: 5,
            select_keys: "123456789".to_string(),
            full_width: false,
            passthrough: false,
            chinese_punctuation: true,
            auto_suggestion: false,
            min_suggestion_trigger_length: 2,
            learning_enabled: true,
            v_mode_enabled: true,
            u_mode_enabled: true,
            inline_prediction: true,
            show_raw_double_pinyin: false,
            choose_char_from_phrase: true,
            emoji_enabled: false,
            english_enabled: false,
            allow_fuzzy: true,
            sort_by_pinyin_length: false,
            double_pinyin_scheme: None,
            emit_composed_sentences: true,
            candidate_rank_mode: CandidateRankMode::MatchedSpanThenScore,
            input_purpose: InputPurpose::FreeForm,
            fuzzy: Vec::new(),
            incomplete_penalty: 500,
        }
    }
}

impl Editor {
    /// Snapshot current runtime options.
    pub fn editor_options(&self) -> EditorOptions {
        let e = &self.engine;
        let c = &e.model.config;
        EditorOptions {
            candidates_per_page: self.session.candidates.page_size(),
            select_keys: c.select_keys.clone(),
            full_width: c.full_width_enabled,
            passthrough: self.passthrough,
            chinese_punctuation: c.chinese_punctuation,
            auto_suggestion: c.auto_suggestion,
            min_suggestion_trigger_length: c.min_suggestion_trigger_length,
            learning_enabled: c.learning_enabled,
            v_mode_enabled: c.v_mode_enabled,
            u_mode_enabled: c.u_mode_enabled,
            inline_prediction: c.inline_prediction,
            show_raw_double_pinyin: c.show_raw_double_pinyin,
            choose_char_from_phrase: c.choose_char_from_phrase,
            emoji_enabled: e.emoji_enabled,
            english_enabled: e.english_enabled,
            allow_fuzzy: e.allow_fuzzy,
            sort_by_pinyin_length: e.sort_by_pinyin_length,
            double_pinyin_scheme: e.double_pinyin_scheme.clone(),
            emit_composed_sentences: c.emit_composed_sentences,
            candidate_rank_mode: c.candidate_rank_mode,
            input_purpose: self.context.input_purpose,
            fuzzy: c.fuzzy.clone(),
            incomplete_penalty: c.incomplete_penalty,
        }
    }

    /// Apply all options at once (single engine cache clear).
    pub fn set_editor_options(&mut self, o: EditorOptions) {
        let e = &mut self.engine;
        e.emoji_enabled = o.emoji_enabled;
        e.english_enabled = o.english_enabled;
        e.allow_fuzzy = o.allow_fuzzy;
        e.sort_by_pinyin_length = o.sort_by_pinyin_length;
        e.double_pinyin_scheme = o.double_pinyin_scheme;
        let c = &mut e.model.config;
        c.select_keys = o.select_keys;
        c.full_width_enabled = o.full_width;
        c.chinese_punctuation = o.chinese_punctuation;
        c.auto_suggestion = o.auto_suggestion;
        c.min_suggestion_trigger_length = o.min_suggestion_trigger_length;
        c.learning_enabled = o.learning_enabled;
        c.v_mode_enabled = o.v_mode_enabled;
        c.u_mode_enabled = o.u_mode_enabled;
        c.inline_prediction = o.inline_prediction;
        c.show_raw_double_pinyin = o.show_raw_double_pinyin;
        c.choose_char_from_phrase = o.choose_char_from_phrase;
        c.emit_composed_sentences = o.emit_composed_sentences;
        c.candidate_rank_mode = o.candidate_rank_mode;
        c.fuzzy = o.fuzzy;
        c.incomplete_penalty = o.incomplete_penalty;
        self.session.candidates.set_page_size(o.candidates_per_page);

        if o.passthrough != self.passthrough {
            self.set_passthrough(o.passthrough);
        }
        // Reuses private-purpose clearing; only when it actually changes.
        if o.input_purpose != self.context.input_purpose
            || o.input_purpose != self.engine.input_purpose
        {
            self.set_input_purpose(o.input_purpose);
        }
        self.engine.clear_cache();
    }
}
