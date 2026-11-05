//! Pinyin Chinese input method engine.
//!
//! # Entry points
//!
//! - [`Engine`] — load lexicon data and query candidates (`input`)
//! - [`Editor`] — key-event session (preedit, candidates, commit)
//! - [`Context`] — host-facing snapshot after each [`Editor::process_key`]
//! - [`path`] — data-dir / userdict discovery ([`Engine::from_default_paths`])
//!
//! Configure the editor with [`EditorOptions`] (conversion knobs in [`HostKnobs`]).
//! Use [`LanguageMode`] (中/英) and [`CharacterForm`] (半/全) instead of ad-hoc bools.
//!
//! Advanced APIs live in submodules: [`parser`], [`config`], [`double_pinyin`],
//! [`lexicon`], [`emoji`], [`english`], …

mod context;
mod fuzzy;
mod session;
mod trie;

pub mod config;
pub mod double_pinyin;
pub mod editor;
pub mod emoji;
pub mod engine;
pub mod english;
pub mod lexicon;
pub mod model;
pub mod parser;
pub mod path;
pub mod userdict;
pub mod word_bigram;

pub use context::{Context, InputPurpose};
pub use editor::{Editor, EditorOptions, HostKnobs, KeyEvent, KeyResult, LanguageMode};
pub use engine::Engine;
pub use model::{CandidateRankMode, CharacterForm, Config};
pub use path::DataVariant;
pub use session::{Candidate, InputMode};
