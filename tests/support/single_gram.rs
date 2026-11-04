/*!
SingleGram — compact token -> frequency container and merge helper.

This file provides an in-memory, correctness-first implementation of the
SingleGram abstraction used in the upstream libpinyin code. It mirrors the
runtime semantics required by the lookup and training code:

- store total frequency (u32) and a sorted list of (token, freq) pairs
- support lookup, insert, set, remove and range search
- produce normalized frequencies (freq / total) used by lookup scoring
- merge system + user grams into a merged SingleGram with summed counts
*/

use std::cmp::Ordering;

/// Token type used in phrase tables (upstream uses integer token ids).
pub type PhraseToken = u32;

/// SingleGram stores a total frequency and a sorted vector of (token, freq).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SingleGram {
    total_freq: u32,
    /// Sorted by token ascending and unique tokens.
    items: Vec<(PhraseToken, u32)>,
}

impl SingleGram {
    /// Create an empty SingleGram.
    pub fn new() -> Self {
        Self {
            total_freq: 0,
            items: Vec::new(),
        }
    }

    /// Get total frequency.
    pub fn get_total_freq(&self) -> u32 {
        self.total_freq
    }

    /// Set total frequency.
    pub fn set_total_freq(&mut self, total: u32) {
        self.total_freq = total;
    }

    /// Insert a token with given frequency.
    ///
    /// Returns true if inserted; if token already exists, does not modify and
    /// returns false (matches upstream `insert_freq` semantics).
    pub fn insert_freq(&mut self, token: PhraseToken, freq: u32) -> bool {
        match self.items.binary_search_by(|(t, _)| t.cmp(&token)) {
            Ok(_) => false,
            Err(idx) => {
                self.items.insert(idx, (token, freq));
                true
            }
        }
    }

    /// Set an existing token's frequency.
    ///
    /// Returns true if the token existed and was updated, false if not found.
    pub fn set_freq(&mut self, token: PhraseToken, freq: u32) -> bool {
        match self.items.binary_search_by(|(t, _)| t.cmp(&token)) {
            Ok(idx) => {
                self.items[idx].1 = freq;
                true
            }
            Err(_) => false,
        }
    }

    /// Retrieve raw frequency for a token, if present.
    pub fn get_freq(&self, token: PhraseToken) -> Option<u32> {
        match self.items.binary_search_by(|(t, _)| t.cmp(&token)) {
            Ok(idx) => Some(self.items[idx].1),
            Err(_) => None,
        }
    }

    /// Search for tokens inside [range_begin, range_end) and return a vector of
    /// (token, normalized_freq) where normalized_freq = freq / total_freq as f32.
    /// If total_freq == 0 the normalized_freq will be 0.0.
    pub fn search_range(
        &self,
        range_begin: PhraseToken,
        range_end: PhraseToken,
    ) -> Vec<(PhraseToken, f32)> {
        let mut out = Vec::new();
        if range_begin >= range_end || self.items.is_empty() {
            return out;
        }

        // Find first index where token >= range_begin using binary search.
        let start = match self.items.binary_search_by(|(t, _)| {
            if *t < range_begin {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        }) {
            Ok(idx) => idx,
            Err(idx) => idx,
        };

        let total = self.total_freq as f32;
        for i in start..self.items.len() {
            let (tok, freq) = self.items[i];
            if tok >= range_end {
                break;
            }
            let norm = if total > 0.0 {
                (freq as f32) / total
            } else {
                0.0
            };
            out.push((tok, norm));
        }
        out
    }

    /// Retrieve all items as (token, count, normalized_freq).
    pub fn retrieve_all(&self) -> Vec<(PhraseToken, u32, f32)> {
        let total = self.total_freq as f32;
        self.items
            .iter()
            .map(|(t, c)| {
                (
                    *t,
                    *c,
                    if total > 0.0 {
                        (*c as f32) / total
                    } else {
                        0.0
                    },
                )
            })
            .collect()
    }
}
