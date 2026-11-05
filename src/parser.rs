//
// Pinyin parser for syllable segmentation.
// - DP-based segmentation with beam search (segment_top_k)
// - TrieNode for efficient prefix matching
// - Fuzzy matching integrated via Engine
//
// References (upstream C++):
// - src/storage/pinyin_parser2.cpp
// - src/storage/pinyin_parser_table.h
// - src/pinyin.cpp
//
// Future enhancements:
// - Verify exact parity with upstream DP cost model
// - Add comprehensive test vectors from upstream test suite

use crate::fuzzy::FuzzyMap;
use crate::trie::TrieNode;

use crate::config::PinyinConfig;

/// A single matched syllable (a chunk of pinyin).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syllable {
    /// The syllable text as stored in the syllable set (e.g. "ni", "hao", "zhong").
    pub text: String,

    /// Whether this match was produced via a fuzzy rule (placeholder boolean).
    /// In a full implementation this would carry more information:
    /// which rule, penalty applied, and mapping direction.
    pub fuzzy: bool,
}

impl Syllable {
    pub fn new<T: Into<String>>(text: T, fuzzy: bool) -> Self {
        Self {
            text: text.into(),
            fuzzy,
        }
    }
}

/// Parser providing segmentation using a trie and fuzzy rules.
///
/// Public entrypoints:
/// - [`Parser::insert_syllable`] to seed the trie
/// - [`Parser::segment_best`] to obtain the best segmentation
#[derive(Debug)]
pub struct Parser {
    trie: TrieNode,
    fuzzy: FuzzyMap,
    /// Allow incomplete syllable prefixes (e.g. `zh` → `zhong`).
    pinyin_incomplete: bool,
    correct_ue_ve: bool,
    correct_v_u: bool,
    correct_uen_un: bool,
    correct_gn_ng: bool,
    correct_mg_ng: bool,
    correct_iou_iu: bool,
    correct_uei_ui: bool,
    correct_on_ong: bool,
}

impl Default for Parser {
    /// Full soft-match options from [`PinyinConfig::default`].
    fn default() -> Self {
        Self::from_pinyin_config(&PinyinConfig::default())
    }
}

impl Parser {
    /// Build a parser whose soft-match behaviour follows `PinyinConfig`.
    pub fn from_pinyin_config(cfg: &PinyinConfig) -> Self {
        Self {
            trie: TrieNode::default(),
            fuzzy: FuzzyMap::from_rules(&cfg.base.fuzzy),
            pinyin_incomplete: cfg.pinyin_incomplete,
            correct_ue_ve: cfg.correct_ue_ve,
            correct_v_u: cfg.correct_v_u,
            correct_uen_un: cfg.correct_uen_un,
            correct_gn_ng: cfg.correct_gn_ng,
            correct_mg_ng: cfg.correct_mg_ng,
            correct_iou_iu: cfg.correct_iou_iu,
            correct_uei_ui: cfg.correct_uei_ui,
            correct_on_ong: cfg.correct_on_ong,
        }
    }

    /// Create a parser and insert a list of syllables.
    pub fn with_syllables<T: AsRef<str>>(syllables: &[T]) -> Self {
        let mut p = Parser::default();
        for s in syllables {
            p.insert_syllable(s.as_ref());
        }
        p
    }

    /// Like `with_syllables`, but with an explicit option profile.
    pub fn with_syllables_and_config<T: AsRef<str>>(syllables: &[T], cfg: &PinyinConfig) -> Self {
        let mut p = Parser::from_pinyin_config(cfg);
        for s in syllables {
            p.insert_syllable(s.as_ref());
        }
        p
    }

    /// Insert a single syllable into the parser's trie.
    pub fn insert_syllable(&mut self, syllable: &str) {
        // canonicalize: lower-case and trim
        let key = syllable.trim().to_ascii_lowercase();
        if !key.is_empty() {
            self.trie.insert(&key);
        }
    }

    /// Return fuzzy alternatives for a syllable (public API for tests).
    ///
    /// This exposes the parser's fuzzy map in a controlled way so tests can
    /// validate fuzzy alternatives without accessing private fields.
    pub fn fuzzy_alternatives(&self, syllable: &str) -> Vec<String> {
        self.fuzzy
            .alternatives(syllable)
            .into_iter()
            .map(|(alt, _penalty)| alt)
            .collect()
    }

    /// Convert double pinyin (shuangpin) input to full pinyin using the specified scheme.
    ///
    /// Processes input in 2-character chunks, converting each to a full pinyin syllable.
    /// Falls back to single-character and special cases when needed.
    ///
    /// # Arguments
    /// * `input` - The double pinyin input string (e.g., "uihn" for "shuang")
    /// * `scheme_name` - The scheme name ("Microsoft", "ZiRanMa", etc.)
    ///
    /// # Returns
    /// * `Some(String)` - The converted full pinyin string
    /// * `None` - If conversion fails (invalid scheme or invalid input)
    pub fn convert_double_pinyin(&self, input: &str, scheme_name: &str) -> Option<String> {
        use crate::double_pinyin::DoublePinyinScheme;

        let scheme = DoublePinyinScheme::parse(scheme_name)?;

        // Process input: letters + ';' are scheme keys; other chars pass through.
        let mut result = String::new();
        let mut current_word = String::new();

        for ch in input.chars() {
            if ch.is_ascii_alphanumeric() || ch == ';' {
                current_word.push(ch);
            } else {
                if !current_word.is_empty() {
                    let converted = Self::convert_double_pinyin_word(&current_word, &scheme)?;
                    result.push_str(&converted);
                    current_word.clear();
                }
                result.push(ch);
            }
        }

        if !current_word.is_empty() {
            let converted = Self::convert_double_pinyin_word(&current_word, &scheme)?;
            result.push_str(&converted);
        }

        Some(result)
    }

    /// Convert a single double pinyin word to full pinyin.
    ///
    /// Processes input in 2-character chunks. Handles odd-length input by
    /// processing the last character separately.
    fn convert_double_pinyin_word(
        word: &str,
        scheme: &crate::double_pinyin::DoublePinyinScheme,
    ) -> Option<String> {
        use crate::double_pinyin::{double_to_full_pinyin, get_scheme_data};

        let scheme_data = get_scheme_data(scheme);
        let chars: Vec<char> = word.chars().collect();
        let mut result = String::new();
        let mut i = 0;

        while i < chars.len() {
            if i + 1 < chars.len() {
                // Try 2-char conversion
                let first = chars[i];
                let second = chars[i + 1];
                if let Some(full) = double_to_full_pinyin(first, second, &scheme_data) {
                    result.push_str(&full);
                    i += 2;
                } else {
                    // Single char fallthrough
                    result.push(chars[i]);
                    i += 1;
                }
            } else {
                // Last char, just pass through
                result.push(chars[i]);
                i += 1;
            }
        }

        Some(result)
    }

    /// Perform segmentation on `input` and return the single-best segmentation.
    ///
    /// Tie-breaking: lower total cost, then larger covered length, then fewer
    /// segments, then lower fuzzy/correction distance.
    ///
    /// Returns syllable tokens in order (lowercase). For double pinyin use
    /// `segment_with_scheme`; for custom penalties use `segment_best_with_config`.
    pub fn segment_best(&self, input: &str, allow_fuzzy: bool) -> Vec<Syllable> {
        // Use default config penalties
        let config = crate::Config::default();
        self.segment_with_config(input, allow_fuzzy, None, &config)
    }

    /// Perform segmentation with custom config for penalty tuning.
    ///
    /// This allows callers to adjust fuzzy penalties, correction penalties, etc.
    pub fn segment_best_with_config(
        &self,
        input: &str,
        allow_fuzzy: bool,
        config: &crate::Config,
    ) -> Vec<Syllable> {
        self.segment_with_config(input, allow_fuzzy, None, config)
    }

    /// Perform segmentation with optional double pinyin scheme conversion.
    ///
    /// # Arguments
    /// * `input` - The input string (full pinyin or double pinyin)
    /// * `allow_fuzzy` - Whether to allow fuzzy matching
    /// * `scheme_name` - Optional double pinyin scheme name (e.g., "Microsoft", "ZiRanMa")
    ///
    /// If `scheme_name` is Some, the input will first be converted from double pinyin
    /// to full pinyin before segmentation. If conversion fails, it falls back to
    /// treating the input as standard pinyin.
    pub fn segment_with_scheme(
        &self,
        input: &str,
        allow_fuzzy: bool,
        scheme_name: Option<&str>,
    ) -> Vec<Syllable> {
        let config = crate::Config::default();
        self.segment_with_config(input, allow_fuzzy, scheme_name, &config)
    }

    /// Internal unified segmentation method with full configuration.
    fn segment_with_config(
        &self,
        input: &str,
        allow_fuzzy: bool,
        scheme_name: Option<&str>,
        config: &crate::Config,
    ) -> Vec<Syllable> {
        // If double pinyin scheme specified, try to convert
        let processed_input = if let Some(scheme) = scheme_name {
            // Try to convert double pinyin to full pinyin
            match self.convert_double_pinyin(input, scheme) {
                Some(converted) => converted,
                None => {
                    // Conversion failed, fall back to original input
                    input.to_string()
                }
            }
        } else {
            input.to_string()
        };

        // Now perform standard segmentation on the processed input
        self.segment_best_internal(&processed_input, allow_fuzzy, config)
    }

    /// Internal segmentation method that does the actual DP work.
    ///
    /// This is separated out so that both segment_best and segment_with_scheme
    /// can use the same logic.
    fn segment_best_internal(
        &self,
        input: &str,
        allow_fuzzy: bool,
        config: &crate::Config,
    ) -> Vec<Syllable> {
        // Normalize input: lowercase and remove whitespace
        let normalized: Vec<char> = input
            .to_ascii_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();

        let n = normalized.len();
        if n == 0 {
            return Vec::new();
        }

        // Enhanced DP state per position with improved cost modeling:
        // - best_cost[pos]: comprehensive cost including length, frequency, and penalty factors
        // - best_parsed[pos]: total parsed characters (higher coverage is better)
        // - best_num_keys[pos]: number of segments used (fewer segments preferred)
        // - best_distance[pos]: accumulated fuzzy/edit distance penalty (lower is better)
        // - best_choice[pos]: the chosen transition (end_pos, matched_string, fuzzy_flag)
        let mut best_cost: Vec<f32> = vec![f32::INFINITY; n + 1];
        let mut best_parsed: Vec<usize> = vec![0; n + 1];
        let mut best_num_keys: Vec<usize> = vec![usize::MAX; n + 1];
        let mut best_distance: Vec<i32> = vec![i32::MAX; n + 1];
        let mut best_choice: Vec<Option<(usize, String, bool)>> = vec![None; n + 1];

        // base: at end of input zero cost, zero parsed, zero keys, zero distance
        best_cost[n] = 0.0;
        best_parsed[n] = 0;
        best_num_keys[n] = 0;
        best_distance[n] = 0;

        // helper to decide whether candidate should replace current best at pos
        // Use a plain function that takes references to the best_* arrays to avoid
        // closure-capture borrow conflicts when we need to mutate those arrays.
        #[allow(clippy::too_many_arguments)]
        fn should_replace(
            pos: usize,
            cand_cost: f32,
            cand_parsed: usize,
            cand_keys: usize,
            cand_dist: i32,
            best_cost: &[f32],
            best_parsed: &[usize],
            best_num_keys: &[usize],
            best_distance: &[i32],
        ) -> bool {
            // primary: strictly lower cost
            if cand_cost < best_cost[pos] {
                return true;
            }
            // nearly-equal cost: apply tie-breakers
            if (cand_cost - best_cost[pos]).abs() < 1e-6 {
                if cand_parsed > best_parsed[pos] {
                    return true;
                }
                if cand_parsed == best_parsed[pos] {
                    if cand_keys < best_num_keys[pos] {
                        return true;
                    }
                    if cand_keys == best_num_keys[pos] && cand_dist < best_distance[pos] {
                        return true;
                    }
                }
            }
            false
        }

        // iterate positions backward
        for pos in (0..n).rev() {
            // First try all exact trie prefixes from pos
            let prefixes = self.trie.walk_prefixes(&normalized, pos);

            for (end, matched) in prefixes.iter() {
                // Only consider suffixes that are reachable (best_cost[end] finite)
                if best_cost[*end].is_infinite() {
                    continue;
                }

                // Enhanced cost model based on segment length and frequency
                let seg_len = end - pos;
                let seg_cost = self.calculate_segment_cost(matched, seg_len, false);
                let cand_cost = seg_cost + best_cost[*end];
                let cand_parsed = seg_len + best_parsed[*end];
                // num_keys: 1 for this segment + keys used from end
                let cand_keys = 1 + best_num_keys[*end];
                // distance: pass-through (exact match doesn't add distance)
                let cand_dist = best_distance[*end];

                if should_replace(
                    pos,
                    cand_cost,
                    cand_parsed,
                    cand_keys,
                    cand_dist,
                    &best_cost,
                    &best_parsed,
                    &best_num_keys,
                    &best_distance,
                ) {
                    best_cost[pos] = cand_cost;
                    best_parsed[pos] = cand_parsed;
                    best_num_keys[pos] = cand_keys;
                    best_distance[pos] = cand_dist;
                    best_choice[pos] = Some((*end, matched.clone(), false));
                }
            }

            // Soft matching (corrections + fuzzy AMB) when allow_fuzzy is set.
            // Individual features are gated by PinyinConfig / upstream option bits.
            if allow_fuzzy {
                for len in 1..=4 {
                    if pos + len > n {
                        break;
                    }
                    let substr: String = normalized[pos..pos + len].iter().collect();

                    // Try pinyin corrections first (ue/ve, v/u) - these have lower penalty than fuzzy
                    let corrections = self.apply_corrections(&substr);
                    for corrected in corrections {
                        if self.trie.contains_word(&corrected) && corrected != substr {
                            let end = pos + len;
                            if end <= n && !best_cost[end].is_infinite() {
                                let seg_cost =
                                    self.calculate_segment_cost(&corrected, len, false) + 0.5;
                                let cand_cost = seg_cost + best_cost[end];
                                let cand_parsed = len + best_parsed[end];
                                let cand_keys = 1 + best_num_keys[end];
                                // Correction penalty from config (default: 200)
                                let correction_penalty = config.correction_penalty;
                                let cand_dist = correction_penalty + best_distance[end];

                                if should_replace(
                                    pos,
                                    cand_cost,
                                    cand_parsed,
                                    cand_keys,
                                    cand_dist,
                                    &best_cost,
                                    &best_parsed,
                                    &best_num_keys,
                                    &best_distance,
                                ) {
                                    best_cost[pos] = cand_cost;
                                    best_parsed[pos] = cand_parsed;
                                    best_num_keys[pos] = cand_keys;
                                    best_distance[pos] = cand_dist;
                                    best_choice[pos] = Some((end, corrected.clone(), true));
                                }
                            }
                        }
                    }

                    // Then try fuzzy alternatives
                    let alts = self.fuzzy.alternatives(&substr);
                    for (alt, penalty) in alts {
                        if self.trie.contains_word(&alt) && alt != substr {
                            // Calculate the actual end position based on the alternative's length
                            // For same-length alternatives, use original end position
                            // For different-length alternatives, adjust accordingly
                            let alt_len = alt.chars().count();
                            let original_len = substr.chars().count();

                            // For now, handle same-length and different-length cases
                            let end = if alt_len == original_len {
                                pos + len
                            } else {
                                // For different lengths, we need to consider if we can consume
                                // the alternative at this position. Since we're looking at
                                // fuzzy alternatives, we treat this as a substitution at the
                                // original position length.
                                pos + len
                            };

                            if end <= n && !best_cost[end].is_infinite() {
                                let seg_cost = self.calculate_segment_cost(&alt, alt_len, true);
                                let cand_cost = seg_cost + best_cost[end];
                                let cand_parsed = len + best_parsed[end]; // Use original length for parsing position
                                let cand_keys = 1 + best_num_keys[end];
                                // Use the per-rule penalty from fuzzy map, scaled by config multiplier
                                let fuzzy_penalty =
                                    (penalty * (config.fuzzy_penalty_multiplier as f32)) as i32;
                                let cand_dist = fuzzy_penalty + best_distance[end];

                                if should_replace(
                                    pos,
                                    cand_cost,
                                    cand_parsed,
                                    cand_keys,
                                    cand_dist,
                                    &best_cost,
                                    &best_parsed,
                                    &best_num_keys,
                                    &best_distance,
                                ) {
                                    best_cost[pos] = cand_cost;
                                    best_parsed[pos] = cand_parsed;
                                    best_num_keys[pos] = cand_keys;
                                    best_distance[pos] = cand_dist;
                                    best_choice[pos] = Some((end, alt.clone(), true));
                                }
                            }
                        }
                    }
                }
            }

            // Incomplete syllable matching mirrors PINYIN_INCOMPLETE (independent of AMB).
            // Only accept upstream-style incomplete keys (shengmu incl. zh/ch/sh, and
            // single-letter finals), not arbitrary trie prefixes like "zho".
            if allow_fuzzy && self.pinyin_incomplete && best_choice[pos].is_none() {
                let max_len = (n - pos).min(2);
                for len in (1..=max_len).rev() {
                    let incomplete: String = normalized[pos..pos + len].iter().collect();
                    if !Self::is_incomplete_key(&incomplete) {
                        continue;
                    }

                    // Find any syllable in the trie that starts with this prefix
                    if let Some(completion) = self.find_syllable_completion(&incomplete) {
                        let end = pos + len;
                        if !best_cost[end].is_infinite() {
                            // Incomplete match gets higher penalty than fuzzy but lower than unknown
                            let seg_cost =
                                self.calculate_segment_cost(&completion, len, false) + 2.0;
                            let cand_cost = seg_cost + best_cost[end];
                            let cand_parsed = len + best_parsed[end];
                            let cand_keys = 1 + best_num_keys[end];
                            // Longer incomplete keys get a slightly lower distance so zh beats z+h.
                            let incomplete_penalty = config
                                .incomplete_penalty
                                .saturating_sub((len as i32 - 1) * 10);
                            let cand_dist = incomplete_penalty + best_distance[end];

                            if should_replace(
                                pos,
                                cand_cost,
                                cand_parsed,
                                cand_keys,
                                cand_dist,
                                &best_cost,
                                &best_parsed,
                                &best_num_keys,
                                &best_distance,
                            ) {
                                best_cost[pos] = cand_cost;
                                best_parsed[pos] = cand_parsed;
                                best_num_keys[pos] = cand_keys;
                                best_distance[pos] = cand_dist;
                                // Mark as fuzzy since it's an incomplete match
                                best_choice[pos] = Some((end, incomplete.clone(), true));
                                break;
                            }
                        }
                    }
                }
            }

            // If still no choice found, fallback: consume one character as unknown token
            // We still allow this option but with a large penalty; it contributes 1 parsed char.
            if best_choice[pos].is_none() {
                let end = pos + 1;
                if !best_cost[end].is_infinite() {
                    let substr: String = normalized[pos..end].iter().collect();
                    let seg_cost = config.unknown_cost; // penalty from config (default: 10.0)
                    let cand_cost = seg_cost + best_cost[end];
                    let cand_parsed = 1 + best_parsed[end];
                    let cand_keys = 1 + best_num_keys[end];
                    let cand_dist = config.unknown_penalty + best_distance[end]; // penalty from config (default: 1000)

                    if should_replace(
                        pos,
                        cand_cost,
                        cand_parsed,
                        cand_keys,
                        cand_dist,
                        &best_cost,
                        &best_parsed,
                        &best_num_keys,
                        &best_distance,
                    ) {
                        best_cost[pos] = cand_cost;
                        best_parsed[pos] = cand_parsed;
                        best_num_keys[pos] = cand_keys;
                        best_distance[pos] = cand_dist;
                        best_choice[pos] = Some((end, substr, false));
                    }
                }
            }
        }

        // Reconstruct segmentation
        let mut out: Vec<Syllable> = Vec::new();
        let mut cur = 0usize;
        while cur < n {
            if let Some((next, word, fuzzy)) = &best_choice[cur] {
                // Treat apostrophe (') as an enforced separator and skip it in the final output.
                // Upstream behavior propagates state across apostrophes but does not emit them
                // as tokens; mimic that here by advancing the cursor without pushing a token.
                if word == "'" {
                    cur = *next;
                    continue;
                }
                out.push(Syllable::new(word.clone(), *fuzzy));
                cur = *next;
            } else {
                // defensive fallback (shouldn't happen)
                let ch: String = normalized[cur].to_string();
                out.push(Syllable::new(ch, false));
                cur += 1;
            }
        }

        out
    }

    /// Return top-K segmentation alternatives (beam search).
    ///
    /// This implements a left-to-right beam search that expands exact trie
    /// Calculate the cost of a segment based on length, content, and fuzzy status.
    /// Cost model for a candidate segment (length, rarity, fuzzy penalty).
    fn calculate_segment_cost(&self, segment: &str, length: usize, is_fuzzy: bool) -> f32 {
        let base_cost = 1.0_f32;

        // Length bonus: longer segments are generally preferred in pinyin
        let length_bonus = match length {
            1 => 0.3,  // Single char segments get penalty
            2 => 0.0,  // Standard two-character segments are neutral
            3 => -0.2, // Three-character segments get small bonus
            4 => -0.3, // Four+ character segments get larger bonus
            _ => -0.4,
        };

        // Content-based adjustments for common vs rare syllables
        let content_adjustment = match segment.len() {
            0..=2 => 0.1, // Very short segments slightly penalized
            3..=5 => 0.0, // Normal length segments
            _ => 0.05,    // Long segments slightly penalized for complexity
        };

        // Fuzzy penalty based on type of fuzzy match
        let fuzzy_penalty = if is_fuzzy { 0.8 } else { 0.0 };

        base_cost + length_bonus + content_adjustment + fuzzy_penalty
    }

    /// Find a syllable completion for an incomplete prefix.
    ///
    /// For example, "n" might complete to "ni", "nh" might complete to "nihao".
    /// Returns the first completion found, or None if no completions exist.
    pub fn find_syllable_completion(&self, prefix: &str) -> Option<String> {
        self.trie.first_completion(prefix)
    }

    /// Upstream incomplete pinyin keys: shengmu (incl. zh/ch/sh) and simple finals.
    fn is_incomplete_key(s: &str) -> bool {
        matches!(
            s,
            // Shengmu
            "b" | "p" | "m" | "f"
                | "d" | "t" | "n" | "l"
                | "g" | "k" | "h"
                | "j" | "q" | "x"
                | "zh" | "ch" | "sh" | "r"
                | "z" | "c" | "s"
                | "y" | "w"
                // Simple finals / medials seen as incomplete keys in C
                | "a" | "o" | "e" | "i" | "u" | "v"
                | "ai" | "ei" | "ao" | "ou"
                | "an" | "en" | "ang" | "eng" | "er"
        )
    }

    /// Apply pinyin corrections (ue/ve, v/u) to a string.
    ///
    /// Returns corrected alternatives if applicable.
    pub fn apply_corrections(&self, s: &str) -> Vec<String> {
        let mut results = Vec::new();

        // Correction 1: ue ↔ ve (e.g., "nue" ↔ "nve", "lue" ↔ "lve")
        if self.correct_ue_ve {
            if s.contains("ue") {
                results.push(s.replace("ue", "ve"));
            }
            if s.contains("ve") {
                results.push(s.replace("ve", "ue"));
            }
        }

        // Correction 2: v ↔ u in certain contexts (e.g., "nv" ↔ "nu", "lv" ↔ "lu")
        // This is context-sensitive: only after n, l
        if self.correct_v_u {
            for &initial in &["n", "l"] {
                let vu_pattern = format!("{}u", initial);
                let vv_pattern = format!("{}v", initial);

                if s.contains(&vu_pattern) {
                    results.push(s.replace(&vu_pattern, &vv_pattern));
                }
                if s.contains(&vv_pattern) {
                    results.push(s.replace(&vv_pattern, &vu_pattern));
                }
            }
        }

        // Correction 3: uen ↔ un (e.g., "juen" ↔ "jun", "chuen" ↔ "chun")
        // PINYIN_CORRECT_UEN_UN
        if self.correct_uen_un {
            if s.contains("uen") {
                results.push(s.replace("uen", "un"));
            }
            if s.contains("un") {
                results.push(s.replace("un", "uen"));
            }
        }

        // Correction 4: gn ↔ ng (e.g., "bagn" ↔ "bang", "hegn" ↔ "heng")
        // PINYIN_CORRECT_GN_NG
        if self.correct_gn_ng {
            if s.contains("gn") {
                results.push(s.replace("gn", "ng"));
            }
            if s.contains("ng") {
                results.push(s.replace("ng", "gn"));
            }
        }

        // Correction 5: mg ↔ ng (e.g., "bamg" ↔ "bang", "hemg" ↔ "heng")
        // PINYIN_CORRECT_MG_NG
        if self.correct_mg_ng && s.contains("mg") {
            results.push(s.replace("mg", "ng"));
        }

        // Correction 6: iou ↔ iu (e.g., "liou" ↔ "liu", "jiou" ↔ "jiu")
        // PINYIN_CORRECT_IOU_IU
        if self.correct_iou_iu {
            if s.contains("iou") {
                results.push(s.replace("iou", "iu"));
            }
            if s.contains("iu") {
                results.push(s.replace("iu", "iou"));
            }
        }

        // Correction 7: uei ↔ ui (e.g., "guei" ↔ "gui")
        // PINYIN_CORRECT_UEI_UI
        if self.correct_uei_ui {
            if s.contains("uei") {
                results.push(s.replace("uei", "ui"));
            }
            if s.contains("ui") {
                results.push(s.replace("ui", "uei"));
            }
        }

        // Correction 8: on ↔ ong (e.g., "zon" ↔ "zong")
        // PINYIN_CORRECT_ON_ONG
        if self.correct_on_ong {
            if let Some(stem) = s.strip_suffix("ong") {
                if !stem.is_empty() {
                    results.push(format!("{stem}on"));
                }
            } else if let Some(stem) = s.strip_suffix("on") {
                if !stem.is_empty() {
                    results.push(format!("{stem}ong"));
                }
            }
        }

        results
            .into_iter()
            .filter(|r| !r.is_empty() && r != s)
            .collect()
    }

    /// Return top-K segmentation alternatives (beam search).
    ///
    /// This implements a left-to-right beam search that expands exact trie
    /// prefixes and simple fuzzy alternatives (up to a small substring length).
    /// States are ranked by a tuple similar to the DP tie-breakers used in
    /// `segment_best`: (cost ascending, parsed descending, keys ascending, distance ascending).
    ///
    /// The implementation is intentionally conservative and correctness-first:
    /// it favors clarity and parity with `segment_best`'s cost model while
    /// producing up to `k` distinct segmentation hypotheses.
    ///
    /// For double pinyin support, use `segment_top_k_with_scheme` instead.
    /// For custom penalty configuration, use `segment_top_k_with_config`.
    pub fn segment_top_k(&self, input: &str, k: usize, allow_fuzzy: bool) -> Vec<Vec<Syllable>> {
        let config = crate::Config::default();
        self.segment_top_k_with_config(input, k, allow_fuzzy, None, &config)
    }

    /// Perform top-K segmentation with custom config for penalty tuning.
    pub fn segment_top_k_with_config(
        &self,
        input: &str,
        k: usize,
        allow_fuzzy: bool,
        scheme_name: Option<&str>,
        config: &crate::Config,
    ) -> Vec<Vec<Syllable>> {
        // If double pinyin scheme specified, try to convert
        let processed_input = if let Some(scheme) = scheme_name {
            self.convert_double_pinyin(input, scheme)
                .unwrap_or_else(|| input.to_string())
        } else {
            input.to_string()
        };

        if k <= 1 {
            let best = self.segment_best_internal(&processed_input, allow_fuzzy, config);
            return if best.is_empty() {
                Vec::new()
            } else {
                vec![best]
            };
        }
        self.segment_top_k_internal(&processed_input, k, allow_fuzzy, config)
    }

    /// Perform beam search segmentation with optional double pinyin scheme conversion.
    ///
    /// # Arguments
    /// * `input` - The input string (full pinyin or double pinyin)
    /// * `k` - Number of top segmentations to return
    /// * `allow_fuzzy` - Whether to allow fuzzy matching
    /// * `scheme_name` - Optional double pinyin scheme name
    ///
    /// If `scheme_name` is Some, converts double pinyin to full pinyin first.
    pub fn segment_top_k_with_scheme(
        &self,
        input: &str,
        k: usize,
        allow_fuzzy: bool,
        scheme_name: Option<&str>,
    ) -> Vec<Vec<Syllable>> {
        let config = crate::Config::default();
        self.segment_top_k_with_config(input, k, allow_fuzzy, scheme_name, &config)
    }

    /// Internal beam search method that does the actual work.
    fn segment_top_k_internal(
        &self,
        input: &str,
        k: usize,
        allow_fuzzy: bool,
        config: &crate::Config,
    ) -> Vec<Vec<Syllable>> {
        // Normalize input: lowercase and remove whitespace (same as segment_best)
        let normalized: Vec<char> = input
            .to_ascii_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let n = normalized.len();
        if n == 0 {
            return Vec::new();
        }

        // Beam state
        #[derive(Clone)]
        struct State {
            pos: usize,
            tokens: Vec<Syllable>,
            cost: f32,
            parsed: usize,
            keys: usize,
            dist: i32,
        }

        // Comparator used for ranking states (lower is better)
        fn state_cmp(a: &State, b: &State) -> std::cmp::Ordering {
            // primary: cost (smaller better)
            if (a.cost - b.cost).abs() > 1e-6 {
                return a
                    .cost
                    .partial_cmp(&b.cost)
                    .unwrap_or(std::cmp::Ordering::Equal);
            }
            // tie: prefer larger parsed
            if a.parsed != b.parsed {
                return b.parsed.cmp(&a.parsed);
            }
            // tie: prefer fewer keys
            if a.keys != b.keys {
                return a.keys.cmp(&b.keys);
            }
            // tie: prefer smaller distance
            a.dist.cmp(&b.dist)
        }

        // initial state
        let start = State {
            pos: 0,
            tokens: Vec::new(),
            cost: 0.0,
            parsed: 0,
            keys: 0,
            dist: 0,
        };

        let mut beam: Vec<State> = vec![start];
        let mut completed: Vec<State> = Vec::new();

        // beam width: allow more slack for longer inputs to avoid premature pruning
        // Scale with input length to handle phrases like "woshinuoweiren" (7 syllables)
        let base_width = std::cmp::max(16, k.saturating_mul(8));
        let beam_width = base_width + (n / 4); // Add extra width for longer inputs

        while !beam.is_empty() {
            let mut next_beam: Vec<State> = Vec::new();

            for st in beam.into_iter() {
                // If this state already finished, keep it in completed set.
                if st.pos == n {
                    completed.push(st);
                    continue;
                }

                // Expand exact trie prefixes starting at st.pos
                let prefixes = self.trie.walk_prefixes(&normalized, st.pos);
                for (end, matched) in prefixes.into_iter() {
                    // only expand if suffix from `end` is reachable (we don't require that here)
                    let mut new_tokens = st.tokens.clone();
                    new_tokens.push(Syllable::new(matched.clone(), false));
                    let new_state = State {
                        pos: end,
                        tokens: new_tokens,
                        cost: st.cost + 1.0_f32, // exact match cost
                        parsed: st.parsed + (end - st.pos),
                        keys: st.keys + 1,
                        dist: st.dist,
                    };
                    next_beam.push(new_state);
                }

                // Corrections + fuzzy AMB + incomplete (gated like segment_best).
                if allow_fuzzy {
                    for len in 1..=4 {
                        if st.pos + len > n {
                            break;
                        }
                        let substr: String = normalized[st.pos..st.pos + len].iter().collect();
                        for corrected in self.apply_corrections(&substr) {
                            if self.trie.contains_word(&corrected) && corrected != substr {
                                let end = st.pos + len;
                                let mut new_tokens = st.tokens.clone();
                                new_tokens.push(Syllable::new(corrected.clone(), true));
                                next_beam.push(State {
                                    pos: end,
                                    tokens: new_tokens,
                                    cost: st.cost + 0.5,
                                    parsed: st.parsed + (end - st.pos),
                                    keys: st.keys + 1,
                                    dist: st.dist + config.correction_penalty,
                                });
                            }
                        }
                        let alts = self.fuzzy.alternatives(&substr);
                        for (alt, penalty) in alts.into_iter() {
                            // If the alt is an exact syllable in the trie, use it as a fuzzy match
                            if self.trie.contains_word(&alt) && alt != substr {
                                // Accept fuzzy alternatives even if they differ in length
                                // (e.g., lan -> nan via l=n rule if both are valid syllables)
                                let end = st.pos + len;
                                let mut new_tokens = st.tokens.clone();
                                new_tokens.push(Syllable::new(alt.clone(), true));
                                let new_state = State {
                                    pos: end,
                                    tokens: new_tokens,
                                    cost: st.cost + penalty, // Use per-rule penalty
                                    parsed: st.parsed + (end - st.pos),
                                    keys: st.keys + 1,
                                    dist: st.dist + (penalty * 100.0) as i32, // Scale for distance
                                };
                                next_beam.push(new_state);
                            }
                        }
                    }

                    if self.pinyin_incomplete {
                        let max_len = (n - st.pos).min(2);
                        for len in (1..=max_len).rev() {
                            let incomplete: String =
                                normalized[st.pos..st.pos + len].iter().collect();
                            if !Self::is_incomplete_key(&incomplete) {
                                continue;
                            }
                            if let Some(completion) = self.find_syllable_completion(&incomplete) {
                                let end = st.pos + len;
                                let mut new_tokens = st.tokens.clone();
                                new_tokens.push(Syllable::new(incomplete.clone(), true));
                                let _ = completion; // completion proves a valid prefix exists
                                next_beam.push(State {
                                    pos: end,
                                    tokens: new_tokens,
                                    cost: st.cost + 2.0 - 0.1 * (len as f32 - 1.0),
                                    parsed: st.parsed + len,
                                    keys: st.keys + 1,
                                    dist: st.dist
                                        + config
                                            .incomplete_penalty
                                            .saturating_sub((len as i32 - 1) * 10),
                                });
                                break; // longest incomplete prefix only
                            }
                        }
                    }
                }

                // Unknown fallback: consume one character with heavy penalty
                let end = st.pos + 1;
                if end <= n {
                    let substr: String = normalized[st.pos..end].iter().collect();
                    let mut new_tokens = st.tokens.clone();
                    new_tokens.push(Syllable::new(substr.clone(), false));
                    let new_state = State {
                        pos: end,
                        tokens: new_tokens,
                        cost: st.cost + 10.0_f32,
                        parsed: st.parsed + 1,
                        keys: st.keys + 1,
                        dist: st.dist + 1000,
                    };
                    next_beam.push(new_state);
                }
            }

            if next_beam.is_empty() {
                break;
            }

            // prune next_beam to beam_width using our comparator
            next_beam.sort_by(state_cmp);
            if next_beam.len() > beam_width {
                next_beam.truncate(beam_width);
            }

            beam = next_beam;
        }

        // If no completed segmentation was found, fall back to best single segmentation
        if completed.is_empty() {
            return vec![self.segment_best_internal(input, allow_fuzzy, config)];
        }

        // Sort completed states and return top-k token sequences
        completed.sort_by(state_cmp);
        let mut out: Vec<Vec<Syllable>> = Vec::new();
        for st in completed.into_iter().take(k) {
            out.push(st.tokens);
        }
        out
    }
}
