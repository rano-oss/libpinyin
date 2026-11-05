//! Shared types and utilities for libpinyin offline tooling.
//!
//! Contains the heap-based WordBigram model (for building),
//! LexEntry type, and mmap format write helpers.

mod word_bigram;
pub use word_bigram::{WordBigram, WordBigramBuilder};

pub mod mmap_write;

/// Lexicon entry used during table conversion.
#[derive(Debug, Clone)]
pub struct LexEntry {
    pub utf8: String,
    pub token: u32,
    pub freq: u32,
}
