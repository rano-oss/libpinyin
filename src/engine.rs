//! Pinyin IME engine: segmentation, lexicon lookup, scoring, and candidate caching.
//!
//! `Engine::input(text) -> Vec<Candidate>` is the main entry point; wrap it in
//! [`Editor`](crate::Editor) for key-event sessions.

use std::collections::{HashMap, HashSet};
use std::error::Error;

use crate::parser::{Parser, Syllable};
use crate::{Candidate, EmojiDict, EnglishDict, Lexicon, Model, UserDict, WordBigram};

/// All standard pinyin syllables (without tone markers).
/// This list includes all valid pinyin syllables in Mandarin Chinese.
pub const PINYIN_SYLLABLES: &[&str] = &[
    "a", "ai", "an", "ang", "ao", "ba", "bai", "ban", "bang", "bao", "bei", "ben", "beng", "bi",
    "bian", "biao", "bie", "bin", "bing", "bo", "bu", "ca", "cai", "can", "cang", "cao", "ce",
    "cen", "ceng", "cha", "chai", "chan", "chang", "chao", "che", "chen", "cheng", "chi", "chong",
    "chou", "chu", "chuai", "chuan", "chuang", "chui", "chun", "chuo", "ci", "cong", "cou", "cu",
    "cuan", "cui", "cun", "cuo", "da", "dai", "dan", "dang", "dao", "de", "dei", "deng", "di",
    "dia", "dian", "diao", "die", "ding", "diu", "dong", "dou", "du", "duan", "dui", "dun", "duo",
    "e", "ei", "en", "er", "fa", "fan", "fang", "fei", "fen", "feng", "fo", "fou", "fu", "ga",
    "gai", "gan", "gang", "gao", "ge", "gei", "gen", "geng", "gong", "gou", "gu", "gua", "guai",
    "guan", "guang", "gui", "gun", "guo", "ha", "hai", "han", "hang", "hao", "he", "hei", "hen",
    "heng", "hong", "hou", "hu", "hua", "huai", "huan", "huang", "hui", "hun", "huo", "ji", "jia",
    "jian", "jiang", "jiao", "jie", "jin", "jing", "jiong", "jiu", "ju", "juan", "jue", "jun",
    "ka", "kai", "kan", "kang", "kao", "ke", "ken", "keng", "kong", "kou", "ku", "kua", "kuai",
    "kuan", "kuang", "kui", "kun", "kuo", "la", "lai", "lan", "lang", "lao", "le", "lei", "leng",
    "li", "lia", "lian", "liang", "liao", "lie", "lin", "ling", "liu", "lo", "long", "lou", "lu",
    "luan", "lun", "luo", "lv", "lve", "ma", "mai", "man", "mang", "mao", "me", "mei", "men",
    "meng", "mi", "mian", "miao", "mie", "min", "ming", "miu", "mo", "mou", "mu", "na", "nai",
    "nan", "nang", "nao", "ne", "nei", "nen", "neng", "ng", "ni", "nian", "niang", "niao", "nie",
    "nin", "ning", "niu", "nong", "nou", "nu", "nuan", "nuo", "nv", "nve", "o", "ou", "pa", "pai",
    "pan", "pang", "pao", "pei", "pen", "peng", "pi", "pian", "piao", "pie", "pin", "ping", "po",
    "pou", "pu", "qi", "qia", "qian", "qiang", "qiao", "qie", "qin", "qing", "qiong", "qiu", "qu",
    "quan", "que", "qun", "ran", "rang", "rao", "re", "ren", "reng", "ri", "rong", "rou", "ru",
    "ruan", "rui", "run", "ruo", "sa", "sai", "san", "sang", "sao", "se", "sen", "seng", "sha",
    "shai", "shan", "shang", "shao", "she", "shei", "shen", "sheng", "shi", "shou", "shu", "shua",
    "shuai", "shuan", "shuang", "shui", "shun", "shuo", "si", "song", "sou", "su", "suan", "sui",
    "sun", "suo", "ta", "tai", "tan", "tang", "tao", "te", "teng", "ti", "tian", "tiao", "tie",
    "ting", "tong", "tou", "tu", "tuan", "tui", "tun", "tuo", "wa", "wai", "wan", "wang", "wei",
    "wen", "weng", "wo", "wu", "xi", "xia", "xian", "xiang", "xiao", "xie", "xin", "xing", "xiong",
    "xiu", "xu", "xuan", "xue", "xun", "ya", "yan", "yang", "yao", "ye", "yi", "yin", "ying", "yo",
    "yong", "you", "yu", "yuan", "yue", "yun", "za", "zai", "zan", "zang", "zao", "ze", "zei",
    "zen", "zeng", "zha", "zhai", "zhan", "zhang", "zhao", "zhe", "zhen", "zheng", "zhi", "zhong",
    "zhou", "zhu", "zhua", "zhuai", "zhuan", "zhuang", "zhui", "zhun", "zhuo", "zi", "zong", "zou",
    "zu", "zuan", "zui", "zun", "zuo",
];

/// Pinyin IME engine: parser + model → ranked candidates (with an LRU cache).
///
/// Note: Fuzzy matching is handled by the parser during segmentation. The engine
/// works with the segmentations provided by the parser.
///
/// Plain-assignment fields are `pub`. Prefer the `set_*` methods for options that
/// affect cached results (`emoji_enabled`, `allow_fuzzy`, …): they also clear the cache.
pub struct Engine {
    pub model: Model,
    pub parser: Parser,
    limit: usize,
    cache: lru::LruCache<String, Vec<Candidate>>,
    cache_hits: usize,
    cache_misses: usize,
    emoji_dict: Option<EmojiDict>,
    pub emoji_enabled: bool,
    english_dict: Option<EnglishDict>,
    pub english_enabled: bool,
    /// When true, prefer candidates whose matching key/syllable span is shorter.
    pub sort_by_pinyin_length: bool,
    /// When true, parser may apply fuzzy AMB / corrections / incomplete.
    pub allow_fuzzy: bool,
    /// Optional double-pinyin scheme name forwarded to parsers that support it.
    pub double_pinyin_scheme: Option<String>,
    /// Additional addon dictionaries (topic-specific)
    addon_lexicons: Vec<(String, crate::Lexicon, bool)>,
    pub input_purpose: crate::InputPurpose,
}

impl Engine {
    /// Create an engine from a model, using the standard pinyin syllable set.
    pub fn new(model: Model) -> Self {
        Self::with_parser(model, Parser::with_syllables(PINYIN_SYLLABLES))
    }

    /// Wrap this engine in an [`Editor`](crate::Editor) with the given candidate page size.
    pub fn into_editor(self, page_size: usize) -> crate::Editor {
        crate::Editor::with_page_size(self, page_size)
    }

    /// Create an engine with a custom parser.
    pub fn with_parser(model: Model, parser: Parser) -> Self {
        // Default cache capacity
        let cache_capacity = 1000;

        Self {
            model,
            parser,
            limit: 50,
            cache: lru::LruCache::new(
                std::num::NonZeroUsize::new(cache_capacity)
                    .unwrap_or(std::num::NonZeroUsize::new(1000).unwrap()),
            ),
            cache_hits: 0,
            cache_misses: 0,
            emoji_dict: None,
            emoji_enabled: false,
            english_dict: None,
            english_enabled: false,
            sort_by_pinyin_length: false,
            allow_fuzzy: true,
            double_pinyin_scheme: None,
            addon_lexicons: Vec::new(),
            input_purpose: crate::InputPurpose::FreeForm,
        }
    }

    /// Process input and return ranked candidates.
    ///
    /// This implements the full IME pipeline:
    /// 1. Check cache for previous result
    /// 2. Parse input into syllable segmentations (parser handles fuzzy matching)
    /// 3. For each segmentation:
    ///    - Convert to lexicon key
    ///    - Look up candidates in lexicon
    ///    - Apply penalty if segmentation used fuzzy matching
    /// 4. Merge and rank candidates
    /// 5. Append emoji candidates if enabled
    /// 6. Cache the result
    pub fn input(&mut self, input: &str) -> Vec<Candidate> {
        // Check cache first (LRU automatically updates access time)
        if let Some(cached) = self.cache.get(input) {
            self.cache_hits += 1;
            return cached.clone();
        }

        self.cache_misses += 1;

        // C uses one parse. `segment_top_k(..., 1)` routes to `segment_best`.
        let allow_fuzzy = self.allow_fuzzy;
        let segs = {
            let scheme = &self.double_pinyin_scheme;
            self.parser
                .segment_top_k_with_scheme(input, 1, allow_fuzzy, scheme.as_deref())
        };

        // Single primary segmentation (k=1): no HashMap merge needed.
        let mut vec: Vec<Candidate> = match segs.len() {
            0 => Vec::new(),
            1 => self.generate_candidates_from_segmentation(&segs[0], true),
            _ => {
                let mut best: HashMap<String, Candidate> = HashMap::new();
                for seg in segs {
                    for cand in self.generate_candidates_from_segmentation(&seg, true) {
                        match best.get(&cand.text) {
                            Some(existing) if existing.score >= cand.score => {}
                            _ => {
                                best.insert(cand.text.clone(), cand);
                            }
                        }
                    }
                }
                best.into_values().collect()
            }
        };

        let is_hidden = |text: &str| -> bool {
            self.model.config.masked_phrases.contains(text) || self.model.userdict.is_masked(text)
        };
        vec.retain(|c| !is_hidden(&c.text));

        // Rank by configured mode, then score/frequency.
        let input_len = input.len();
        let rank_mode = self.model.config.candidate_rank_mode;
        let matched_span = |c: &Candidate| c.input_consumed.unwrap_or(input_len);
        let phrase_chars = |c: &Candidate| c.text.chars().count();
        // C `compare_item_with_sort_option`: phrase length → pinyin length → freq.
        let rank = |a: &Candidate, b: &Candidate| {
            match rank_mode {
                crate::CandidateRankMode::MatchedSpanThenScore => {
                    let primary = matched_span(b).cmp(&matched_span(a));
                    if primary != std::cmp::Ordering::Equal {
                        return primary;
                    }
                }
                crate::CandidateRankMode::PhraseLengthThenScore => {
                    let primary = phrase_chars(b).cmp(&phrase_chars(a));
                    if primary != std::cmp::Ordering::Equal {
                        return primary;
                    }
                    // SORT_BY_PINYIN_LENGTH (matched input span).
                    let secondary = matched_span(b).cmp(&matched_span(a));
                    if secondary != std::cmp::Ordering::Equal {
                        return secondary;
                    }
                }
            }
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        };
        vec.sort_by(rank);

        if vec.len() > self.limit {
            vec.truncate(self.limit);
        }

        // Query enabled addon lexicons and merge results
        {
            let addons = &self.addon_lexicons;
            for (_name, lexicon, enabled) in addons.iter() {
                if !*enabled {
                    continue;
                }
                // Query addon with the full input as key
                let addon_entries = lexicon.lookup_with_freq(input);
                for (text, freq) in addon_entries {
                    if is_hidden(&text) {
                        continue;
                    }
                    if !vec.iter().any(|c| c.text == text) {
                        // Insert addon results with slightly lower priority
                        let score = freq as f32 * 0.8;
                        vec.push(Candidate::new(text, score));
                    }
                }
            }
        }

        vec.sort_by(rank);

        // Optional: when sort_by_pinyin_length is on, prefer fewer chars among near-ties.
        if self.sort_by_pinyin_length {
            vec.sort_by(|a, b| {
                let score_cmp = b
                    .score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal);
                if score_cmp == std::cmp::Ordering::Equal || (a.score - b.score).abs() < 0.5 {
                    a.text
                        .chars()
                        .count()
                        .cmp(&b.text.chars().count())
                        .then(score_cmp)
                } else {
                    score_cmp
                }
            });
        }

        // Mixed-input extras: keep a short phonetic head, then surface english /
        // emoji on the first page instead of appending after ~50 hanzi hits.
        let purpose = self.input_purpose;
        let allow_mixed = !purpose.disables_learning();
        let want_english = allow_mixed && (self.english_enabled || purpose.prefers_english());
        let want_emoji = allow_mixed && self.emoji_enabled;
        let mut extras: Vec<Candidate> = Vec::new();
        if want_english {
            if let Some(ref english_dict) = self.english_dict {
                for (word, freq) in english_dict.candidates_for_input(input) {
                    if vec.iter().any(|c| c.text.eq_ignore_ascii_case(word))
                        || extras.iter().any(|c| c.text.eq_ignore_ascii_case(word))
                    {
                        continue;
                    }
                    extras.push(Candidate::new(word.to_string(), -50.0 + freq.ln_1p()));
                }
            }
        }
        if want_emoji {
            if let Some(ref emoji_dict) = self.emoji_dict {
                // Exact ASCII/Chinese keyword on the raw buffer, else bridge via
                // top phonetic candidates (e.g. xin → 心 → 💌).
                let entry = emoji_dict.lookup_best(input).or_else(|| {
                    vec.iter()
                        .take(8)
                        .filter_map(|c| emoji_dict.lookup_best(&c.text))
                        .max_by_key(|e| e.frequency)
                });
                if let Some(entry) = entry {
                    if !vec.iter().any(|c| c.text == entry.emoji)
                        && !extras.iter().any(|c| c.text == entry.emoji)
                    {
                        extras.push(Candidate::new(entry.emoji.clone(), -80.0));
                    }
                }
            }
        }

        if !extras.is_empty() {
            const PHONETIC_HEAD: usize = 3;
            let keep_phonetic = self.limit.saturating_sub(extras.len()).max(1);
            if vec.len() > keep_phonetic {
                vec.truncate(keep_phonetic);
            }
            let head = vec.len().min(PHONETIC_HEAD);
            let mut merged = Vec::with_capacity(vec.len() + extras.len());
            merged.extend(vec.drain(..head));
            merged.append(&mut extras);
            merged.append(&mut vec);
            vec = merged;
        }

        if vec.len() > self.limit {
            vec.truncate(self.limit);
        }

        // Cache the result (LRU automatically handles eviction)
        self.cache.put(input.to_string(), vec.clone());

        vec
    }

    /// Commit a phrase to user learning.
    ///
    /// Records user selection to boost future rankings when learning is enabled.
    /// Clears cache to reflect updated frequencies immediately.
    pub fn commit(&mut self, phrase: &str) {
        let purpose = self.input_purpose;
        if self.model.config.learning_enabled && !purpose.disables_learning() {
            if let Err(e) = self.model.userdict.learn_with_count(phrase, 1) {
                eprintln!("libpinyin: learn failed: {e}");
            }
        }
        self.clear_cache();
    }

    pub fn set_input_purpose(&mut self, purpose: crate::InputPurpose) {
        self.input_purpose = purpose;
        self.clear_cache();
    }

    pub fn clear_masked_phrases(&mut self) {
        if let Err(e) = self.model.userdict.clear_masked() {
            eprintln!("libpinyin: clear_masked failed: {e}");
        }
        self.model.config.masked_phrases.clear();
        self.clear_cache();
    }

    /// Forget a candidate phrase (remove from user history / learning).
    ///
    /// Deletes any learned frequency, masks the phrase so it stays hidden, and
    /// clears the candidate cache.
    pub fn forget_phrase(&mut self, phrase: &str) {
        if phrase.is_empty() {
            return;
        }
        if let Err(e) = self.model.userdict.forget_phrase(phrase) {
            eprintln!("libpinyin: forget_phrase failed: {e}");
        }
        self.model.config.masked_phrases.insert(phrase.to_string());
        self.clear_cache();
    }

    /// Undo forget/mask for a phrase (persisted userdict + session config).
    pub fn unmask_phrase(&mut self, phrase: &str) {
        if phrase.is_empty() {
            return;
        }
        if let Err(e) = self.model.userdict.unmask_phrase(phrase) {
            eprintln!("libpinyin: unmask_phrase failed: {e}");
        }
        self.model.config.masked_phrases.remove(phrase);
        self.clear_cache();
    }

    /// Persist a mask without deleting learned frequency.
    pub fn mask_phrase(&mut self, phrase: &str) {
        if phrase.is_empty() {
            return;
        }
        if let Err(e) = self.model.userdict.mask_phrase(phrase) {
            eprintln!("libpinyin: mask_phrase failed: {e}");
        }
        self.model.config.masked_phrases.insert(phrase.to_string());
        self.clear_cache();
    }

    /// True if phrase is hidden by session config or persisted userdict mask.
    pub fn is_phrase_hidden(&self, phrase: &str) -> bool {
        self.model.config.masked_phrases.contains(phrase) || self.model.userdict.is_masked(phrase)
    }

    /// Set the emoji dictionary for this engine.
    pub fn set_emoji_dict(&mut self, dict: EmojiDict) {
        self.emoji_dict = Some(dict);
    }

    /// Enable or disable emoji candidates in results.
    pub fn set_emoji_enabled(&mut self, enabled: bool) {
        self.emoji_enabled = enabled;
        self.clear_cache();
    }

    /// Set the English word dictionary for mixed-input candidates.
    pub fn set_english_dict(&mut self, dict: EnglishDict) {
        self.english_dict = Some(dict);
    }

    /// Enable or disable English word candidates.
    pub fn set_english_enabled(&mut self, enabled: bool) {
        self.english_enabled = enabled;
        self.clear_cache();
    }

    /// Enable or disable parser fuzzy AMB / corrections / incomplete matching.
    pub fn set_allow_fuzzy(&mut self, enabled: bool) {
        self.allow_fuzzy = enabled;
        self.clear_cache();
    }

    /// Prefer shorter candidates when scores are close.
    pub fn set_sort_by_pinyin_length(&mut self, enabled: bool) {
        self.sort_by_pinyin_length = enabled;
        self.clear_cache();
    }

    /// Emit the best multi-token DP sentence into the candidate list.
    ///
    /// Default is on ([`crate::Config::emit_composed_sentences`]).
    pub fn set_emit_composed_sentences(&mut self, enabled: bool) {
        self.model.config.emit_composed_sentences = enabled;
        self.clear_cache();
    }

    /// Candidate bar ordering (matched pinyin span vs Chinese phrase length).
    pub fn set_candidate_rank_mode(&mut self, mode: crate::CandidateRankMode) {
        self.model.config.candidate_rank_mode = mode;
        self.clear_cache();
    }

    /// Best full-coverage sentence for `input`.
    ///
    /// Runs segment + sentence DP only — does not build the candidate bar
    /// (prefixes / full-key heads / rank / cache). Available even when
    /// [`crate::Config::emit_composed_sentences`] is false.
    pub fn best_sentence(&self, input: &str) -> Option<String> {
        let allow_fuzzy = self.allow_fuzzy;
        let segs = {
            let scheme = &self.double_pinyin_scheme;
            self.parser
                .segment_top_k_with_scheme(input, 1, allow_fuzzy, scheme.as_deref())
        };
        let seg = segs.first()?;
        self.compose_best_sentence(seg)
    }

    /// Optional double-pinyin scheme (`Microsoft`, `ZiRanMa`, …). `None` = full pinyin.
    pub fn set_double_pinyin_scheme(&mut self, scheme: Option<String>) {
        self.double_pinyin_scheme = scheme;
        self.clear_cache();
    }

    /// Swap the lexicon (e.g., for simplified/traditional toggle). Clears cache.
    pub fn swap_lexicon(&mut self, lexicon: Lexicon) {
        self.model.swap_lexicon(lexicon);
        self.clear_cache();
    }

    /// Swap the word bigram model together with lexicon on charset change.
    pub fn swap_word_bigram(&mut self, word_bigram: WordBigram) {
        self.model.swap_word_bigram(word_bigram);
        self.clear_cache();
    }

    /// Add an addon lexicon. Returns its index.
    pub fn add_addon(&mut self, name: String, lexicon: Lexicon, enabled: bool) -> usize {
        let addons = &mut self.addon_lexicons;
        let idx = addons.len();
        addons.push((name, lexicon, enabled));
        if enabled {
            self.clear_cache();
        }
        idx
    }

    /// Enable or disable an addon lexicon by name.
    pub fn set_addon_enabled(&mut self, name: &str, enabled: bool) {
        let addons = &mut self.addon_lexicons;
        for item in addons.iter_mut() {
            if item.0 == name {
                item.2 = enabled;
            }
        }
        self.clear_cache();
    }

    /// Get list of addon names and their enabled state.
    pub fn addon_list(&self) -> Vec<(String, bool)> {
        self.addon_lexicons
            .iter()
            .map(|(name, _, enabled)| (name.clone(), *enabled))
            .collect()
    }

    /// Generate candidates from a segmentation by trying all possible word combinations.
    ///
    /// Uses dynamic programming over word sequences that cover the segmentation.
    /// Scores mix unigram / bigram probabilities with pinyin likelihood; user
    /// learning folds into phrase frequency via `unigram_factor`.
    fn generate_candidates_from_segmentation(
        &self,
        seg: &[Syllable],
        allow_composed_sentence: bool,
    ) -> Vec<Candidate> {
        let n = seg.len();
        if n == 0 {
            return Vec::new();
        }

        let mut results: Vec<Candidate> = Vec::with_capacity(48);
        let mut seen: HashSet<String> = HashSet::with_capacity(64);
        let seg_pinyin_poss = Self::pinyin_poss_for_span(seg);
        let allow_fuzzy = self.allow_fuzzy;
        let (lambda, unigram_factor, sentence_length_penalty, emit_sentences, total_freq) = {
            let cfg = &self.model.config;
            let lex = &self.model.lexicon;
            (
                cfg.lambda,
                cfg.unigram_factor,
                cfg.sentence_length_penalty,
                allow_composed_sentence && cfg.emit_composed_sentences,
                lex.total_freq(),
            )
        };

        let push_unique =
            |results: &mut Vec<Candidate>, seen: &mut HashSet<String>, cand: Candidate| {
                if seen.insert(cand.text.clone()) {
                    results.push(cand);
                }
            };

        // Unigram-only score; userdict freqs are hot-cached (no redb I/O).
        let word_bigram = &self.model.word_bigram;
        let ux_rank = matches!(
            self.model.config.candidate_rank_mode,
            crate::CandidateRankMode::MatchedSpanThenScore
        );
        let score_uni = |phrase: &str, freq: u32, poss: f32| -> f32 {
            let user_freq = self.model.userdict.frequency(phrase);
            let mut effective = freq as f64 + unigram_factor as f64 * user_freq as f64;
            // Best-UX daily preference: modern common words over corpus ties
            // (手机 vs 收集, 但是 vs 当时, …). C-compat PhraseLength path skips this.
            if ux_rank {
                effective += Self::daily_preferred_freq_bonus(phrase);
            }
            let unigram = if total_freq > 0 && effective > 0.0 {
                (effective / total_freq as f64) as f32
            } else {
                word_bigram.get_unigram_probability(phrase)
            };
            (unigram * poss * (1.0 - lambda)).max(1e-10).ln()
        };

        // Fidelity over cold-path caps: emit the full lexicon head for each key
        // (pre-speed-slim behaviour). Truncating to top-N dropped C overlap.
        const AMB_SPAN_SYLLABLES: usize = 2;
        // Mid-word key-prefix completion (`bei'j` → `bei'jing`). Milder than
        // left-span partial_penalty so completions stay competitive in top-5.
        // Mild ln-space penalty so completions stay in top-5 next to exact shorter keys.
        const KEY_COMPLETION_PENALTY: f32 = 0.35;
        const KEY_COMPLETION_MAX_KEYS: usize = 24;
        const KEY_COMPLETION_PHRASES: usize = 4;

        let full_syllables: Vec<&str> = seg.iter().map(|s| s.text.as_str()).collect();
        // AMB fanout only on short spans; long keys already come from fuzzy segment.
        let full_amb = allow_fuzzy && n <= AMB_SPAN_SYLLABLES;
        let lexicon = &self.model.lexicon;
        // C `guess_candidates` scores by unigram after matrix fuzzy expand — no
        // extra 0.5 demotion per AMB rewrite. Use seg pinyin_poss only.
        let key_variants = Self::lookup_key_variants(&full_syllables, full_amb);
        for (full_key, _key_poss) in &key_variants {
            let poss = seg_pinyin_poss.max(0.01);
            for (phrase, freq) in lexicon.lookup_with_freq(full_key) {
                let score = score_uni(&phrase, freq, poss);
                push_unique(&mut results, &mut seen, Candidate::new(phrase, score));
            }
        }

        // Progressive mid-word (Best UX only): complete phonetic keys past the
        // current segment (`ha`→`hao`, `ji`→`jing`, incomplete `j`). Skipped in
        // C-compat PhraseLength mode so guess_candidates parity stays intact.
        if ux_rank {
            if let Some((primary_key, _)) = key_variants.first() {
                if primary_key.len() >= 2 {
                    let poss = seg_pinyin_poss.max(0.01);
                    let max_keys = if n <= 1 { 12 } else { KEY_COMPLETION_MAX_KEYS };
                    for (phrase, freq) in lexicon.lookup_prefix_with_freq_top(
                        primary_key,
                        max_keys,
                        KEY_COMPLETION_PHRASES,
                    ) {
                        let score = score_uni(&phrase, freq, poss) - KEY_COMPLETION_PENALTY;
                        push_unique(&mut results, &mut seen, Candidate::new(phrase, score));
                    }
                }
            }
        }

        // Prefix partials (no DP required) — all proper prefixes, like pre-slim.
        if n > 1 {
            let partial_penalty = 100.0;
            for k in 1..n {
                let bytes_consumed: usize = seg[..k].iter().map(|s| s.text.len()).sum();
                let span_poss = Self::pinyin_poss_for_span(&seg[..k]);
                let span_syllables: Vec<&str> = seg[..k].iter().map(|s| s.text.as_str()).collect();
                let amb = allow_fuzzy && k <= AMB_SPAN_SYLLABLES;
                for (lookup_key, _key_poss) in Self::lookup_key_variants(&span_syllables, amb) {
                    let poss = span_poss.max(0.01);
                    for (phrase, freq) in lexicon.lookup_with_freq(&lookup_key) {
                        let score = score_uni(&phrase, freq, poss) - partial_penalty;
                        push_unique(
                            &mut results,
                            &mut seen,
                            Candidate::with_input_consumed(phrase, score, bytes_consumed),
                        );
                    }
                }
            }
        }

        // Composed sentence needs DP (n≥2). Skipped when emit_composed_sentences is off.
        if emit_sentences && n >= 2 {
            if let Some((text, score)) = self.compose_sentence_dp(
                seg,
                lexicon,
                word_bigram,
                lambda,
                unigram_factor,
                total_freq,
                sentence_length_penalty,
                allow_fuzzy,
                usize::MAX, // full phrase index per span (C fidelity)
                AMB_SPAN_SYLLABLES,
            ) {
                push_unique(&mut results, &mut seen, Candidate::new(text, score));
            }
        }

        results
    }

    /// Sentence DP only. Returns `(text, score)`.
    fn compose_best_sentence(&self, seg: &[Syllable]) -> Option<String> {
        let n = seg.len();
        if n == 0 {
            return None;
        }
        let allow_fuzzy = self.allow_fuzzy;
        let (lambda, unigram_factor, sentence_length_penalty, total_freq) = {
            let cfg = &self.model.config;
            let lex = &self.model.lexicon;
            (
                cfg.lambda,
                cfg.unigram_factor,
                cfg.sentence_length_penalty,
                lex.total_freq(),
            )
        };
        const AMB_SPAN_SYLLABLES: usize = 2;
        let lexicon = &self.model.lexicon;
        let word_bigram = &self.model.word_bigram;
        self.compose_sentence_dp(
            seg,
            lexicon,
            word_bigram,
            lambda,
            unigram_factor,
            total_freq,
            sentence_length_penalty,
            allow_fuzzy,
            usize::MAX,
            AMB_SPAN_SYLLABLES,
        )
        .map(|(text, _)| text)
    }

    /// Core phrase DP over a segmentation. Shared by candidate emit and `best_sentence`.
    #[allow(clippy::too_many_arguments)]
    fn compose_sentence_dp(
        &self,
        seg: &[Syllable],
        lexicon: &Lexicon,
        word_bigram: &WordBigram,
        lambda: f32,
        unigram_factor: f32,
        total_freq: u64,
        sentence_length_penalty: f32,
        allow_fuzzy: bool,
        dp_lookup_top: usize,
        amb_span_syllables: usize,
    ) -> Option<(String, f32)> {
        let n = seg.len();
        if n < 2 {
            // Single syllable: best lexicon head is the "sentence".
            if n == 1 {
                let key = seg[0].text.as_str();
                let poss = Self::pinyin_poss_for_span(seg);
                let amb = allow_fuzzy;
                let mut best: Option<(String, f32)> = None;
                for (lookup_key, key_poss) in Self::lookup_key_variants(&[key], amb) {
                    for (word_text, freq) in
                        lexicon.lookup_with_freq_top(&lookup_key, dp_lookup_top)
                    {
                        let score = Self::score_step_fast(
                            &self.model.userdict,
                            word_bigram,
                            &word_text,
                            freq,
                            None,
                            (poss * key_poss).max(0.01),
                            lambda,
                            unigram_factor,
                            total_freq,
                        );
                        let better = match &best {
                            None => true,
                            Some((_, s)) => score > *s,
                        };
                        if better {
                            best = Some((word_text, score));
                        }
                    }
                }
                return best;
            }
            return None;
        }

        struct Step {
            word: String,
            step_score: f32,
            prev: u16,
            total_score: f32,
            total_chars: u16,
        }
        let mut steps: Vec<Step> = Vec::with_capacity(n * 4);
        let mut best_head: Vec<Option<u16>> = vec![None; n + 1];

        const MAX_SHORT_SYLLABLES: usize = 4;
        // Pre-speed-slim used 10; restore for long-phrase recall.
        const MAX_LONG_LOOKUP_SYLLABLES: usize = 10;

        for i in 0..n {
            if i > 0 && best_head[i].is_none() {
                continue;
            }
            let prev_idx = if i == 0 { None } else { best_head[i] };
            let (prev_total, prev_chars) = match prev_idx {
                None => (0.0f32, 0u16),
                Some(idx) => {
                    let s = &steps[idx as usize];
                    (s.total_score, s.total_chars)
                }
            };
            let prev_word_owned: Option<String> =
                prev_idx.map(|idx| steps[idx as usize].word.clone());

            for len in 1..=std::cmp::min(MAX_SHORT_SYLLABLES, n - i) {
                let span_poss = Self::pinyin_poss_for_span(&seg[i..i + len]);
                let span_syllables: Vec<&str> =
                    seg[i..i + len].iter().map(|s| s.text.as_str()).collect();
                let amb = allow_fuzzy && len <= amb_span_syllables;
                for (lookup_key, key_poss) in Self::lookup_key_variants(&span_syllables, amb) {
                    let poss = (span_poss * key_poss).max(0.01);
                    let hits = lexicon.lookup_with_freq_top(&lookup_key, dp_lookup_top);
                    for (word_text, freq) in hits {
                        let step_score = Self::score_step_fast(
                            &self.model.userdict,
                            word_bigram,
                            &word_text,
                            freq,
                            prev_word_owned.as_deref(),
                            poss,
                            lambda,
                            unigram_factor,
                            total_freq,
                        );
                        let total_score = prev_total + step_score;
                        let total_chars = prev_chars + word_text.chars().count() as u16;
                        let end = i + len;
                        let better = match best_head[end] {
                            None => true,
                            Some(idx) => {
                                let old = &steps[idx as usize];
                                Self::totals_better_than(
                                    total_score,
                                    total_chars as i32,
                                    old.total_score,
                                    old.total_chars as i32,
                                    sentence_length_penalty,
                                )
                            }
                        };
                        if better {
                            let idx = steps.len() as u16;
                            steps.push(Step {
                                word: word_text,
                                step_score,
                                prev: prev_idx.unwrap_or(u16::MAX),
                                total_score,
                                total_chars,
                            });
                            best_head[end] = Some(idx);
                        }
                    }
                }
            }

            for len in (MAX_SHORT_SYLLABLES + 1)..=std::cmp::min(MAX_LONG_LOOKUP_SYLLABLES, n - i) {
                let span_syllables: Vec<&str> =
                    seg[i..i + len].iter().map(|s| s.text.as_str()).collect();
                let exact = span_syllables.join("'");
                if !lexicon.has_key(&exact) {
                    continue;
                }
                let span_poss = Self::pinyin_poss_for_span(&seg[i..i + len]);
                let hits = lexicon.lookup_with_freq_top(&exact, dp_lookup_top);
                for (word_text, freq) in hits {
                    let step_score = Self::score_step_fast(
                        &self.model.userdict,
                        word_bigram,
                        &word_text,
                        freq,
                        prev_word_owned.as_deref(),
                        span_poss,
                        lambda,
                        unigram_factor,
                        total_freq,
                    );
                    let total_score = prev_total + step_score;
                    let total_chars = prev_chars + word_text.chars().count() as u16;
                    let end = i + len;
                    let better = match best_head[end] {
                        None => true,
                        Some(idx) => {
                            let old = &steps[idx as usize];
                            Self::totals_better_than(
                                total_score,
                                total_chars as i32,
                                old.total_score,
                                old.total_chars as i32,
                                sentence_length_penalty,
                            )
                        }
                    };
                    if better {
                        let idx = steps.len() as u16;
                        steps.push(Step {
                            word: word_text,
                            step_score,
                            prev: prev_idx.unwrap_or(u16::MAX),
                            total_score,
                            total_chars,
                        });
                        best_head[end] = Some(idx);
                    }
                }
            }
        }

        let head = best_head[n]?;
        let mut words = Vec::new();
        let mut idx = head;
        loop {
            let s = &steps[idx as usize];
            words.push((s.word.as_str(), s.step_score));
            if s.prev == u16::MAX {
                break;
            }
            idx = s.prev;
        }
        words.reverse();
        let text: String = words.iter().map(|(w, _)| *w).collect();
        let score: f32 = words.iter().map(|(_, s)| s).sum();
        Some((text, score))
    }

    /// Compare two trellis totals (score + char length) like `path_better_than`.
    fn totals_better_than(
        lhs_score: f32,
        lhs_chars: i32,
        rhs_score: f32,
        rhs_chars: i32,
        penalty: f32,
    ) -> bool {
        if lhs_chars + 1 == rhs_chars && lhs_score + penalty < rhs_score {
            return false;
        }
        if lhs_chars == rhs_chars + 1 && lhs_score < rhs_score + penalty {
            return false;
        }
        if lhs_chars == rhs_chars {
            return lhs_score > rhs_score;
        }
        lhs_chars < rhs_chars
    }

    /// Pronunciation match probability for a syllable span.
    /// Exact syllables → 1.0; each fuzzy/correction syllable multiplies by 0.5.
    fn pinyin_poss_for_span(span: &[Syllable]) -> f32 {
        let fuzzy = span.iter().filter(|s| s.fuzzy).count();
        if fuzzy == 0 {
            1.0
        } else {
            0.5_f32.powi(fuzzy as i32).max(0.01)
        }
    }

    /// Extra unigram mass for everyday phrases that lose to older corpus ties.
    /// Applied only under Best UX (`MatchedSpanThenScore`).
    fn daily_preferred_freq_bonus(phrase: &str) -> f64 {
        const PREFERRED: &[&str] = &[
            "手机", "喝水", "但是", "晚上", "上午", "家庭", "你好", "中国", "北京", "谢谢", "朋友",
            "工作", "时间", "今天", "明天", "喜欢", "因为", "所以", "可以", "应该", "知道", "问题",
            "生活", "学习", "老师", "学生", "想要", "保重", "休息",
        ];
        if PREFERRED.contains(&phrase) {
            12000.0
        } else {
            0.0
        }
    }

    /// Lexicon keys to probe for a syllable span, including AMB initial swaps
    /// when fuzzy is enabled (so `nve` also finds `lve` → 略).
    fn lookup_key_variants(syllables: &[&str], allow_fuzzy: bool) -> Vec<(String, f32)> {
        if syllables.is_empty() {
            return Vec::new();
        }
        let exact = syllables.join("'");
        let mut out = vec![(exact, 1.0)];
        if !allow_fuzzy {
            return out;
        }
        // Product of per-syllable AMB variants (initial × final), like C matrix fuzzy.
        const MAX_KEYS: usize = 64;
        let mut variants: Vec<(Vec<String>, f32)> = vec![(Vec::new(), 1.0)];
        for syl in syllables {
            let alts = Self::amb_syllable_variants(syl);
            let mut next = Vec::new();
            'expand: for (prefix, poss) in variants {
                for (alt, ap) in &alts {
                    if next.len() >= MAX_KEYS {
                        break 'expand;
                    }
                    let mut p = prefix.clone();
                    p.push(alt.clone());
                    next.push((p, poss * ap));
                }
            }
            variants = next;
        }
        for (parts, poss) in variants {
            let key = parts.join("'");
            if !out.iter().any(|(k, _)| k == &key) {
                out.push((key, poss));
            }
        }
        out
    }

    /// AMB variants for one syllable: initial swaps × final swaps.
    ///
    /// E.g. `fan` yields `han`, `fang`, and `hang` (f↔h and an↔ang together).
    fn amb_syllable_variants(syl: &str) -> Vec<(String, f32)> {
        let mut initials = vec![syl.to_string()];
        let push = |out: &mut Vec<String>, s: String| {
            if !out.iter().any(|k| k == &s) {
                out.push(s);
            }
        };
        // Digraphs first so `zh` is not treated as `z`+`h`.
        if let Some(rest) = syl.strip_prefix("zh") {
            push(&mut initials, format!("z{rest}"));
        } else if let Some(rest) = syl.strip_prefix('z') {
            push(&mut initials, format!("zh{rest}"));
        }
        if let Some(rest) = syl.strip_prefix("ch") {
            push(&mut initials, format!("c{rest}"));
        } else if let Some(rest) = syl.strip_prefix('c') {
            push(&mut initials, format!("ch{rest}"));
        }
        if let Some(rest) = syl.strip_prefix("sh") {
            push(&mut initials, format!("s{rest}"));
        } else if let Some(rest) = syl.strip_prefix('s') {
            push(&mut initials, format!("sh{rest}"));
        }
        if let Some(rest) = syl.strip_prefix('n') {
            if !rest.is_empty() || syl == "n" {
                push(&mut initials, format!("l{rest}"));
            }
        }
        if let Some(rest) = syl.strip_prefix('l') {
            push(&mut initials, format!("n{rest}"));
            push(&mut initials, format!("r{rest}"));
        }
        if let Some(rest) = syl.strip_prefix('r') {
            push(&mut initials, format!("l{rest}"));
        }
        if let Some(rest) = syl.strip_prefix('f') {
            push(&mut initials, format!("h{rest}"));
        }
        if let Some(rest) = syl.strip_prefix('h') {
            push(&mut initials, format!("f{rest}"));
        }
        if let Some(rest) = syl.strip_prefix('g') {
            push(&mut initials, format!("k{rest}"));
        }
        if let Some(rest) = syl.strip_prefix('k') {
            push(&mut initials, format!("g{rest}"));
        }

        let yunmu_pairs = [
            ("ang", "an"),
            ("an", "ang"),
            ("eng", "en"),
            ("en", "eng"),
            ("ing", "in"),
            ("in", "ing"),
        ];
        let mut out: Vec<(String, f32)> = Vec::new();
        let push_out = |out: &mut Vec<(String, f32)>, s: String, poss: f32| {
            if !out.iter().any(|(k, _)| k == &s) {
                out.push((s, poss));
            }
        };
        for init in &initials {
            let exact = init == syl;
            push_out(&mut out, init.clone(), if exact { 1.0 } else { 0.5 });
            for (from, to) in yunmu_pairs {
                if let Some(head) = init.strip_suffix(from) {
                    let alt = format!("{head}{to}");
                    // Initial and/or final rewrite → fuzzy.
                    push_out(&mut out, alt, 0.5);
                }
            }
        }
        out
    }

    /// Upstream phrase-index unigram with pre-hoisted totals (hot path).
    #[inline]
    fn unigram_poss_fast(
        userdict: &UserDict,
        word_bigram: &WordBigram,
        phrase: &str,
        lexicon_freq: u32,
        unigram_factor: f32,
        total_freq: u64,
    ) -> f32 {
        let user_freq = userdict.frequency(phrase);
        let effective = lexicon_freq as f64 + unigram_factor as f64 * user_freq as f64;
        if total_freq > 0 && effective > 0.0 {
            return (effective / total_freq as f64) as f32;
        }
        word_bigram.get_unigram_probability(phrase)
    }

    /// One trellis step score with pre-hoisted config / model refs (hot path).
    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn score_step_fast(
        userdict: &UserDict,
        word_bigram: &WordBigram,
        phrase: &str,
        lexicon_freq: u32,
        prev: Option<&str>,
        pinyin_poss: f32,
        lambda: f32,
        unigram_factor: f32,
        total_freq: u64,
    ) -> f32 {
        let unigram = Self::unigram_poss_fast(
            userdict,
            word_bigram,
            phrase,
            lexicon_freq,
            unigram_factor,
            total_freq,
        );
        let raw = if let Some(prev_word) = prev {
            let bigram = word_bigram.get_probability(prev_word, phrase);
            (lambda * bigram + (1.0 - lambda) * unigram) * pinyin_poss
        } else {
            unigram * pinyin_poss * (1.0 - lambda)
        };
        raw.max(1e-10).ln()
    }

    /// Get cache statistics for monitoring.
    ///
    /// Returns (hits, misses) tuple.
    pub fn cache_stats(&self) -> (usize, usize) {
        (self.cache_hits, self.cache_misses)
    }

    /// Get current cache size (number of entries).
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Clear the cache (useful for testing or memory management).
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.cache_hits = 0;
        self.cache_misses = 0;
    }

    /// Segment input and return preedit text with apostrophe separators between syllables,
    /// along with the mapped cursor position.
    ///
    /// For example, "wode" with cursor at 2 → ("wo'de", 2), cursor at 4 → ("wo'de", 5).
    /// The cursor is adjusted to account for inserted apostrophes.
    pub fn preedit_with_separators(&self, input: &str, input_cursor: usize) -> (String, usize) {
        if input.is_empty() {
            return (String::new(), 0);
        }
        let segs = {
            let scheme = &self.double_pinyin_scheme;
            let allow_fuzzy = self.allow_fuzzy;
            self.parser
                .segment_top_k_with_scheme(input, 1, allow_fuzzy, scheme.as_deref())
        };
        if let Some(seg) = segs.into_iter().next() {
            if seg.len() > 1 {
                let mut preedit = String::new();
                let mut mapped_cursor = input_cursor;
                let mut input_pos = 0;
                for (i, s) in seg.iter().enumerate() {
                    if i > 0 {
                        preedit.push('\'');
                        // If cursor is past this syllable boundary, add 1 for the apostrophe
                        if input_cursor > input_pos {
                            mapped_cursor += 1;
                        }
                    }
                    preedit.push_str(&s.text);
                    input_pos += s.text.len();
                }
                return (preedit, mapped_cursor);
            }
        }
        (input.to_string(), input_cursor)
    }

    /// Load an engine from a model directory containing runtime artifacts.
    ///
    /// Expected layout (data-dir):
    ///  - lexicon.fst + lexicon.dat        (mmap lexicon)
    ///  - word_bigram.dat + word_bigram_words.fst  (mmap word bigram)
    ///  - userdict.redb                    (persistent user dictionary)
    pub fn from_data_dir<P: AsRef<std::path::Path>>(data_dir: P) -> Result<Self, Box<dyn Error>> {
        let data_dir = data_dir.as_ref();

        // Load lexicon (mmap)
        let fst_path = data_dir.join("lexicon.fst");
        let dat_path = data_dir.join("lexicon.dat");
        let lexicon = Lexicon::load(&fst_path, &dat_path)
            .map_err(|e| format!("failed to load lexicon from {:?}: {}", data_dir, e))?;

        // Load word bigram (mmap)
        let wb_dat_path = data_dir.join("word_bigram.dat");
        let wb_fst_path = data_dir.join("word_bigram_words.fst");
        let word_bigram = WordBigram::load(&wb_dat_path, &wb_fst_path)
            .map_err(|e| format!("failed to load word_bigram from {:?}: {}", data_dir, e))?;

        // Userdict: `LIBCHINESE_USERDICT` override (tests / nested), else beside an
        // explicit `PINYINWL_DATA_DIR` so a private lexicon copy does not still fight
        // the host lock on ~/.pinyin/userdict.redb, else ~/.pinyin/userdict.redb.
        let userdict = {
            let ud_path = if let Ok(p) = std::env::var("LIBCHINESE_USERDICT") {
                std::path::PathBuf::from(p)
            } else if std::env::var_os("PINYINWL_DATA_DIR").is_some() {
                data_dir.join("userdict.redb")
            } else {
                let home = std::env::var("HOME")
                    .or_else(|_| std::env::var("USERPROFILE"))
                    .unwrap_or_else(|_| ".".to_string());
                std::path::PathBuf::from(home)
                    .join(".pinyin")
                    .join("userdict.redb")
            };

            if let Some(parent) = ud_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            UserDict::new(&ud_path)?
        };

        let model = Model::new(lexicon, word_bigram, userdict, crate::Config::default());

        // Aux tables may live beside the lexicon (installed simplified/), in the
        // parent data/ dir, or in the source-tree grandparent (converted/simplified
        // → data/). Also check the user-local share path so English can be
        // installed without writing to /usr.
        let mut aux_dirs = vec![data_dir.to_path_buf()];
        if let Some(parent) = data_dir.parent() {
            aux_dirs.push(parent.to_path_buf());
            if let Some(grandparent) = parent.parent() {
                aux_dirs.push(grandparent.to_path_buf());
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let local = std::path::PathBuf::from(home).join(".local/share/libpinyin/data");
            aux_dirs.push(local.join("simplified"));
            aux_dirs.push(local.join("traditional"));
            aux_dirs.push(local);
        }
        // Dedup while preserving order
        {
            let mut seen = std::collections::HashSet::new();
            aux_dirs.retain(|d| seen.insert(d.clone()));
        }

        let mut engine = Self::new(model);

        let mut emoji_loaded = false;
        for dir in &aux_dirs {
            let emoji_path = dir.join("emoji.table");
            if emoji_path.exists() {
                if let Ok(emoji_dict) = EmojiDict::from_file(&emoji_path) {
                    log::info!("Loaded emoji dictionary with {} entries", emoji_dict.len());
                    engine.set_emoji_dict(emoji_dict);
                    emoji_loaded = true;
                    break;
                }
            }
        }
        if !emoji_loaded {
            log::warn!(
                "emoji.table not found beside {:?}; emoji candidates will be unavailable",
                data_dir
            );
        }
        let mut english_loaded = false;
        for dir in &aux_dirs {
            let english_path = dir.join("english.wordlist");
            if english_path.exists() {
                if let Ok(english_dict) = EnglishDict::from_file(&english_path) {
                    log::info!(
                        "Loaded English dictionary with {} entries from {:?}",
                        english_dict.len(),
                        english_path
                    );
                    engine.set_english_dict(english_dict);
                    english_loaded = true;
                    break;
                }
            }
        }
        if !english_loaded {
            log::warn!(
                "english.wordlist not found beside {:?}; English mixed-input will be unavailable",
                data_dir
            );
        }

        // Load addon dictionaries if available
        let addon_dir = data_dir.join("addon");
        if addon_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&addon_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let fst_path = path.join("lexicon.fst");
                        let dat_path = path.join("lexicon.dat");
                        if fst_path.exists() && dat_path.exists() {
                            if let Ok(lexicon) = Lexicon::load(&fst_path, &dat_path) {
                                let name = path
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                log::info!("Loaded addon dictionary: {}", name);
                                engine.add_addon(name, lexicon, false);
                            }
                        }
                    }
                }
            }
        }

        Ok(engine)
    }
}
