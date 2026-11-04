//! Emoji dictionary for optional mixed-input suggestions.
//!
//! Format: `<keyword>\t<emoji>\t<id>\t<frequency>`
//!
//! The dict only answers lookups. Whether emoji appear in the candidate bar,
//! and where, is decided by the engine merge helpers or the UI.

use std::collections::HashMap;
use std::path::Path;

/// A single emoji entry with its display character and frequency.
#[derive(Debug, Clone)]
pub struct EmojiEntry {
    pub emoji: String,
    pub frequency: u32,
}

/// In-memory emoji dictionary.
#[derive(Debug, Clone)]
pub struct EmojiDict {
    /// ASCII keywords are lowercased; non-ASCII (e.g. Chinese glosses) kept as-is.
    entries: HashMap<String, Vec<EmojiEntry>>,
}

impl EmojiDict {
    /// Load from a `.table` file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::parse(&content))
    }

    /// Parse table content.
    pub fn parse(content: &str) -> Self {
        let mut entries: HashMap<String, Vec<EmojiEntry>> = HashMap::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 4 {
                continue;
            }
            let raw_kw = parts[0];
            let keyword = if raw_kw.is_ascii() {
                raw_kw.to_ascii_lowercase()
            } else {
                raw_kw.to_string()
            };
            let emoji = parts[1].to_string();
            let frequency = parts[3].parse::<u32>().unwrap_or(0);

            entries
                .entry(keyword)
                .or_default()
                .push(EmojiEntry { emoji, frequency });
        }

        for list in entries.values_mut() {
            list.sort_by_key(|e| std::cmp::Reverse(e.frequency));
        }

        Self { entries }
    }

    pub fn empty() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Exact keyword lookup.
    pub fn lookup(&self, keyword: &str) -> Vec<&EmojiEntry> {
        let key = if keyword.is_ascii() {
            keyword.to_ascii_lowercase()
        } else {
            keyword.to_string()
        };
        self.entries
            .get(&key)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Highest-frequency emoji for an exact keyword, if any.
    pub fn lookup_best(&self, keyword: &str) -> Option<&EmojiEntry> {
        self.lookup(keyword).into_iter().next()
    }

    /// Prefix match (for tooling / alternate UIs).
    pub fn lookup_prefix(&self, prefix: &str) -> Vec<(&str, &EmojiEntry)> {
        let prefix_key = if prefix.is_ascii() {
            prefix.to_ascii_lowercase()
        } else {
            prefix.to_string()
        };
        let mut results: Vec<(&str, &EmojiEntry)> = Vec::new();
        for (keyword, entries) in &self.entries {
            if keyword.starts_with(&prefix_key) {
                for entry in entries {
                    results.push((keyword.as_str(), entry));
                }
            }
        }
        results.sort_by_key(|(_, e)| std::cmp::Reverse(e.frequency));
        results
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
