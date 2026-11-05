//! Pinyin Chinese input method engine.
//!
//! High-level entry points:
//! - [`Engine`] — load lexicon data and query candidates (`input`)
//! - [`Editor`] — key-event session (preedit, candidates, commit)
//! - [`Parser`] / [`DoublePinyinScheme`] — segmentation and 双拼

mod context;
mod emoji;
mod english;
mod fuzzy;
mod lexicon;
mod model;
mod session;
mod trie;
mod userdict;
mod word_bigram;

pub mod config;
pub mod double_pinyin;
pub mod editor;
pub mod engine;
pub mod parser;

pub use config::PinyinConfig;
pub use context::{ImeContext, InputPurpose};
pub use double_pinyin::DoublePinyinScheme;
pub use editor::{preferred_chinese_punct, Editor, EditorOptions, KeyEvent, KeyResult};
pub use emoji::{EmojiDict, EmojiEntry};
pub use engine::Engine;
pub use english::EnglishDict;
pub use lexicon::Lexicon;
pub use model::{utils, CandidateRankMode, Config, Model};
pub use parser::{Parser, Syllable};
pub use session::{Candidate, InputMode};
pub use userdict::UserDict;
pub use word_bigram::WordBigram;
