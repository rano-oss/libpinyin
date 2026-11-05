//! Runtime options blob for [`Editor`] (single place for hosts to snapshot/apply).

use super::{Editor, LanguageMode};
use crate::context::InputPurpose;
use crate::model::{CandidateRankMode, CharacterForm, Config};

/// Names of fields copied between [`HostKnobs`] and [`Config`].
macro_rules! each_host_knob {
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

/// Conversion knobs shared with [`Config`].
#[derive(Debug, Clone, PartialEq)]
pub struct HostKnobs {
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

impl Default for HostKnobs {
    fn default() -> Self {
        Self::from_config(&Config::default())
    }
}

impl HostKnobs {
    pub fn from_config(c: &Config) -> Self {
        macro_rules! from {
            ($($f:ident),*) => {
                Self { $($f: c.$f.clone(),)* }
            };
        }
        each_host_knob!(from)
    }

    pub fn apply_to_config(&self, c: &mut Config) {
        macro_rules! apply {
            ($($f:ident),*) => {{
                $(c.$f = self.$f.clone();)*
            }};
        }
        each_host_knob!(apply)
    }
}

/// Session + conversion toggles. Conversion fields are in [`HostKnobs`]
/// (also via [`Deref`](std::ops::Deref) / [`DerefMut`](std::ops::DerefMut)).
#[derive(Debug, Clone, PartialEq)]
pub struct EditorOptions {
    pub candidates_per_page: usize,
    pub language_mode: LanguageMode,
    pub emoji_enabled: bool,
    pub english_enabled: bool,
    pub input_purpose: InputPurpose,
    pub knobs: HostKnobs,
}

impl Default for EditorOptions {
    fn default() -> Self {
        Self {
            candidates_per_page: 5,
            language_mode: LanguageMode::Chinese,
            emoji_enabled: false,
            english_enabled: false,
            input_purpose: InputPurpose::FreeForm,
            knobs: HostKnobs::default(),
        }
    }
}

impl std::ops::Deref for EditorOptions {
    type Target = HostKnobs;
    fn deref(&self) -> &HostKnobs {
        &self.knobs
    }
}

impl std::ops::DerefMut for EditorOptions {
    fn deref_mut(&mut self) -> &mut HostKnobs {
        &mut self.knobs
    }
}

impl Editor {
    pub fn editor_options(&self) -> EditorOptions {
        EditorOptions {
            candidates_per_page: self.session.candidates.page_size(),
            language_mode: self.language_mode,
            emoji_enabled: self.engine.emoji_enabled,
            english_enabled: self.engine.english_enabled,
            input_purpose: self.context.input_purpose,
            knobs: HostKnobs::from_config(&self.engine.model.config),
        }
    }

    pub fn set_editor_options(&mut self, o: EditorOptions) {
        self.engine.emoji_enabled = o.emoji_enabled;
        self.engine.english_enabled = o.english_enabled;
        o.knobs.apply_to_config(&mut self.engine.model.config);
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
