//! Pinyin Chinese input method engine.
//!
//! High-level entry points:
//! - [`Engine`] — load lexicon data and query candidates (`input`)
//! - [`ImeEngine`] — key-event session (preedit, candidates, commit)
//! - [`Parser`] / [`DoublePinyinScheme`] — segmentation and 双拼
//!
//! Shared session / lexicon types are re-exported from the internal `core` module.
//! Advanced building blocks (`PhoneticEditor`, buffers, …) are available as `libpinyin::core::…`.

pub mod core;

pub mod config;
pub mod double_pinyin;
pub mod engine;
pub mod parser;

pub use double_pinyin::DoublePinyinScheme;
pub use engine::{Engine, PINYIN_SYLLABLES};
pub use parser::{Parser, Syllable};

pub use crate::core::{
    preferred_chinese_punct, utils, Candidate, CandidateRankMode, Config, EmojiDict, EnglishDict,
    ImeContext, ImeEngine, InputMode, InputPurpose, KeyEvent, KeyResult, Lexicon, Model, UserDict,
    WordBigram,
};

pub use config::PinyinConfig;
