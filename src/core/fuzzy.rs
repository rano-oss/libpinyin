//! Fuzzy matching for phonetic similarity (pinyin/zhuyin).
use std::collections::HashMap;

/// A single fuzzy rule with penalty.
#[derive(Debug, Clone)]
pub struct FuzzyRule {
    /// The canonical (correct) form
    pub from: String,
    /// The alternative (fuzzy) form
    pub to: String,
    /// Penalty for using this fuzzy match (higher = more penalty)
    pub penalty: f32,
}

impl FuzzyRule {
    pub fn new(from: &str, to: &str, penalty: f32) -> Self {
        Self {
            from: from.to_string(),
            to: to.to_string(),
            penalty,
        }
    }
}

/// Represents fuzzy alternatives for phonetic units (syllables).
///
/// For each canonical syllable (lowercased), stores a vector of alternative
/// syllables with their associated penalties.
#[derive(Debug, Clone)]
pub struct FuzzyMap {
    /// Mapping from syllable to (alternative, penalty) pairs
    map: HashMap<String, Vec<(String, f32)>>,
    /// Default penalty for rules without explicit penalty
    default_penalty: f32,
}

impl Default for FuzzyMap {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
            default_penalty: 1.0,
        }
    }
}

impl FuzzyMap {
    /// Build a `FuzzyMap` from a list of fuzzy rule strings.
    ///
    /// The rules are expected to be textual fuzzy pairs like `"zh=z"` or
    /// `"zh=z:1.5"` (with optional penalty).
    /// Pairs are inserted bidirectionally by default.
    pub fn from_rules(rules: &[String]) -> Self {
        let mut fm = FuzzyMap {
            map: HashMap::new(),
            default_penalty: 1.0,
        };

        for pair in rules.iter() {
            // Parse formats: "a=b" or "a=b:penalty"
            if let Some((rule, penalty_str)) = pair.split_once(':') {
                // Has explicit penalty
                let penalty = penalty_str
                    .trim()
                    .parse::<f32>()
                    .unwrap_or(fm.default_penalty);
                if let Some((a, b)) = rule.split_once('=') {
                    let a = a.trim().to_string();
                    let b = b.trim().to_string();
                    if !a.is_empty() && !b.is_empty() {
                        fm.add_rule(&a, &b, penalty);
                    }
                }
            } else if let Some((a, b)) = pair.split_once('=') {
                // No explicit penalty, use default
                let a = a.trim().to_string();
                let b = b.trim().to_string();
                if !a.is_empty() && !b.is_empty() {
                    fm.add_rule(&a, &b, fm.default_penalty);
                }
            } else {
                // Single token - just ensure it has an entry
                let token = pair.trim().to_string();
                if !token.is_empty() {
                    fm.map.entry(token).or_default();
                }
            }
        }

        fm
    }

    /// Add a fuzzy rule (bidirectional by default).
    pub fn add_rule(&mut self, from: &str, to: &str, penalty: f32) {
        let from = from.to_string();
        let to = to.to_string();

        // Add both directions
        self.map
            .entry(from.clone())
            .or_default()
            .push((to.clone(), penalty));
        self.map.entry(to).or_default().push((from, penalty));
    }

    /// Return alternatives for a syllable including the syllable itself.
    ///
    /// Returns a vector of (alternative, penalty) pairs.
    /// The original syllable is always included with penalty 0.0.
    pub fn alternatives(&self, syllable: &str) -> Vec<(String, f32)> {
        let key = syllable.trim().to_string();
        let mut out = Vec::new();

        // Always include the original with zero penalty
        out.push((key.clone(), 0.0));

        if let Some(alts) = self.map.get(&key) {
            for (alt, penalty) in alts.iter() {
                // Avoid duplicates
                if !out.iter().any(|(s, _)| s == alt) {
                    out.push((alt.clone(), *penalty));
                }
            }
        }
        out
    }
}
