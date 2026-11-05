//! Runtime options blob for [`Editor`] (single place for hosts to snapshot/apply).

use super::{Editor, LanguageMode};
use crate::context::InputPurpose;
use crate::model::{CandidateRankMode, CharacterForm, Config};

/// Fields copied between [`EditorOptions`] and [`Config`].
macro_rules! each_shared_field {
    ($m:ident) => {
        $m! {
            character_form,
            select_keys,
            chinese_punctuation,
            auto_suggestion,
            min_suggestion_trigger_length,
            learning_enabled,
            v_mode_enabled,
            u_mode_enabled,
            inline_prediction,
            show_raw_double_pinyin,
            choose_char_from_phrase,
            allow_fuzzy,
            sort_by_pinyin_length,
            double_pinyin_scheme,
            emit_composed_sentences,
            candidate_rank_mode,
            fuzzy,
            incomplete_penalty
        }
    };
}

/// Session + conversion toggles for [`Editor::set_editor_options`].
#[derive(Debug, Clone, PartialEq)]
pub struct EditorOptions {
    pub candidates_per_page: usize,
    pub language_mode: LanguageMode,
    pub emoji_enabled: bool,
    pub english_enabled: bool,
    pub input_purpose: InputPurpose,

    pub character_form: CharacterForm,
    pub select_keys: String,
    pub chinese_punctuation: bool,
    pub auto_suggestion: bool,
    pub min_suggestion_trigger_length: usize,
    pub learning_enabled: bool,
    pub v_mode_enabled: bool,
    pub u_mode_enabled: bool,
    pub inline_prediction: bool,
    pub show_raw_double_pinyin: bool,
    pub choose_char_from_phrase: bool,
    pub allow_fuzzy: bool,
    pub sort_by_pinyin_length: bool,
    pub double_pinyin_scheme: Option<String>,
    pub emit_composed_sentences: bool,
    pub candidate_rank_mode: CandidateRankMode,
    pub fuzzy: Vec<String>,
    pub incomplete_penalty: i32,
}

impl Default for EditorOptions {
    fn default() -> Self {
        let c = Config::default();
        Self {
            candidates_per_page: 5,
            language_mode: LanguageMode::Chinese,
            emoji_enabled: false,
            english_enabled: false,
            input_purpose: InputPurpose::FreeForm,
            character_form: c.character_form,
            select_keys: c.select_keys.clone(),
            chinese_punctuation: c.chinese_punctuation,
            auto_suggestion: c.auto_suggestion,
            min_suggestion_trigger_length: c.min_suggestion_trigger_length,
            learning_enabled: c.learning_enabled,
            v_mode_enabled: c.v_mode_enabled,
            u_mode_enabled: c.u_mode_enabled,
            inline_prediction: c.inline_prediction,
            show_raw_double_pinyin: c.show_raw_double_pinyin,
            choose_char_from_phrase: c.choose_char_from_phrase,
            allow_fuzzy: c.allow_fuzzy,
            sort_by_pinyin_length: c.sort_by_pinyin_length,
            double_pinyin_scheme: c.double_pinyin_scheme.clone(),
            emit_composed_sentences: c.emit_composed_sentences,
            candidate_rank_mode: c.candidate_rank_mode,
            fuzzy: c.fuzzy.clone(),
            incomplete_penalty: c.incomplete_penalty,
        }
    }
}

impl EditorOptions {
    fn from_config_fields(c: &Config) -> Self {
        macro_rules! from {
            ($($f:ident),*) => {
                Self {
                    candidates_per_page: 5,
                    language_mode: LanguageMode::Chinese,
                    emoji_enabled: false,
                    english_enabled: false,
                    input_purpose: InputPurpose::FreeForm,
                    $($f: c.$f.clone(),)*
                }
            };
        }
        each_shared_field!(from)
    }

    fn apply_to_config(&self, c: &mut Config) {
        macro_rules! apply {
            ($($f:ident),*) => {{
                $(c.$f = self.$f.clone();)*
            }};
        }
        each_shared_field!(apply)
    }
}

impl Editor {
    pub fn editor_options(&self) -> EditorOptions {
        let mut o = EditorOptions::from_config_fields(&self.engine.model.config);
        o.candidates_per_page = self.session.candidates.page_size();
        o.language_mode = self.language_mode;
        o.emoji_enabled = self.engine.emoji_enabled;
        o.english_enabled = self.engine.english_enabled;
        o.input_purpose = self.context.input_purpose;
        o
    }

    pub fn set_editor_options(&mut self, o: EditorOptions) {
        self.engine.emoji_enabled = o.emoji_enabled;
        self.engine.english_enabled = o.english_enabled;
        o.apply_to_config(&mut self.engine.model.config);
        self.session.candidates.set_page_size(o.candidates_per_page);
        if o.language_mode != self.language_mode {
            self.set_language_mode(o.language_mode);
        }
        if o.input_purpose != self.context.input_purpose
            || o.input_purpose != self.engine.input_purpose
        {
            self.set_input_purpose(o.input_purpose);
        }
        self.engine.clear_cache();
    }
}
