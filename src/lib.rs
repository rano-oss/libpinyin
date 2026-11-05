//! Pinyin Chinese input method engine.
//!
//! # Entry points
//!
//! - [`Engine`] — load lexicon data and query candidates (`input`)
//! - [`Editor`] — key-event session (preedit, candidates, commit)
//! - [`Context`] — host-facing snapshot after each [`Editor::process_key`]
//! - [`path`] — data-dir / userdict discovery ([`Engine::from_default_paths`])
//!
//! Configure the editor with [`EditorOptions`].

mod context;
mod fuzzy;
mod session;
mod trie;

pub(crate) mod config;
pub mod double_pinyin;
pub mod editor;
pub(crate) mod emoji;
pub mod engine;
pub(crate) mod english;
pub(crate) mod lexicon;
pub(crate) mod model;
pub(crate) mod parser;
pub mod path;
pub mod userdict;
pub(crate) mod word_bigram;

pub use context::{Context, InputPurpose};
pub use editor::{Editor, EditorOptions, KeyEvent, KeyResult, LanguageMode};
pub use engine::Engine;
pub use model::{CandidateRankMode, CharacterForm, Config};
pub use path::DataVariant;
pub use session::{Candidate, InputMode};

#[cfg(test)]
mod crate_tests;
