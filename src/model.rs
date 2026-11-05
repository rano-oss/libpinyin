//! Shared model types: configuration, lexicon/bigram/userdict bundle, and utilities.

use serde::{Deserialize, Serialize};

use crate::lexicon::Lexicon;
use crate::userdict::UserDict;
use crate::word_bigram::WordBigram;

/// How to order the candidate bar after scoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CandidateRankMode {
    /// Longer matched pinyin span first, then score (product default).
    #[default]
    MatchedSpanThenScore,
    /// Longer Chinese phrase first, then score.
    PhraseLengthThenScore,
}

/// Half-width vs full-width ASCII / punctuation output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CharacterForm {
    #[default]
    Halfwidth,
    Fullwidth,
}

/// Conversion / ranking configuration stored on [`Model`].
///
/// Session-facing toggles (中/英, page size, …) live on [`crate::EditorOptions`].
/// Scoring knobs that hosts rarely touch stay here only.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Fuzzy equivalence rules (e.g. `"zh=z"`, `"an=ang"`).
    pub fuzzy: Vec<String>,
    pub auto_suggestion: bool,
    pub min_suggestion_trigger_length: usize,
    pub character_form: CharacterForm,
    /// Candidate selection keys (e.g. `"123456789"` or `"asdfghjkl"`).
    pub select_keys: String,
    pub masked_phrases: std::collections::HashSet<String>,

    pub correction_penalty: i32,
    pub fuzzy_penalty_multiplier: i32,
    pub incomplete_penalty: i32,
    pub unknown_penalty: i32,
    pub unknown_cost: f32,

    /// Unigram/bigram interpolation (`bigram_λ = lambda`). Default `0.312699`.
    pub lambda: f32,
    /// Trellis path length±1 penalty (`log(1.2)` by default).
    pub sentence_length_penalty: f32,
    /// `lexicon_freq + user_count * unigram_factor` before normalizing.
    pub unigram_factor: f32,

    pub show_raw_double_pinyin: bool,
    pub emit_composed_sentences: bool,
    pub candidate_rank_mode: CandidateRankMode,
    pub choose_char_from_phrase: bool,
    pub learning_enabled: bool,
    pub chinese_punctuation: bool,
    pub v_mode_enabled: bool,
    pub u_mode_enabled: bool,
    pub inline_prediction: bool,

    /// Parser may apply fuzzy AMB / corrections / incomplete syllables.
    pub allow_fuzzy: bool,
    /// Prefer shorter matched pinyin spans when scores are close.
    pub sort_by_pinyin_length: bool,
    /// Double-pinyin scheme name (`Microsoft`, `ZiRanMa`, …). `None` = full pinyin.
    pub double_pinyin_scheme: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            fuzzy: vec![],
            auto_suggestion: false,
            min_suggestion_trigger_length: 2,
            character_form: CharacterForm::Halfwidth,
            select_keys: "123456789".to_string(),
            masked_phrases: std::collections::HashSet::new(),
            correction_penalty: 200,
            fuzzy_penalty_multiplier: 100,
            incomplete_penalty: 500,
            unknown_penalty: 1000,
            unknown_cost: 10.0,
            lambda: 0.312699,
            sentence_length_penalty: 1.2_f32.ln(),
            unigram_factor: 3.0,
            show_raw_double_pinyin: false,
            emit_composed_sentences: true,
            candidate_rank_mode: CandidateRankMode::MatchedSpanThenScore,
            choose_char_from_phrase: true,
            learning_enabled: true,
            chinese_punctuation: true,
            v_mode_enabled: true,
            u_mode_enabled: true,
            inline_prediction: true,
            allow_fuzzy: true,
            sort_by_pinyin_length: false,
            double_pinyin_scheme: None,
        }
    }
}

impl Config {
    /// Index of `ch` in [`Self::select_keys`] (0-based), if it is a selection key.
    pub fn selection_key_index(&self, ch: char) -> Option<usize> {
        self.select_keys.chars().position(|c| c == ch)
    }
}

/// Lexicon + bigram + user dictionary bundle.
pub struct Model {
    pub lexicon: Lexicon,
    pub word_bigram: WordBigram,
    pub userdict: UserDict,
    pub config: Config,
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Model")
            .field("lexicon", &"Lexicon")
            .field("word_bigram", &"WordBigram")
            .finish()
    }
}

impl Model {
    pub fn new(
        lexicon: Lexicon,
        word_bigram: WordBigram,
        userdict: UserDict,
        config: Config,
    ) -> Self {
        Self {
            lexicon,
            word_bigram,
            userdict,
            config,
        }
    }

    pub fn swap_lexicon(&mut self, lexicon: Lexicon) {
        self.lexicon = lexicon;
    }

    pub fn swap_word_bigram(&mut self, word_bigram: WordBigram) {
        self.word_bigram = word_bigram;
    }
}
