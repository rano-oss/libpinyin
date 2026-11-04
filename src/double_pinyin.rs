//! Double Pinyin (Shuangpin 双拼) — tables aligned with upstream libpinyin
//! `src/storage/double_pinyin_table.h`.
//!
//! Schemes (same six as ibus-libpinyin):
//! 1. Microsoft / MSPY
//! 2. ZiRanMa / ZRM
//! 3. ZiGuang / ZGPY
//! 4. ABC
//! 5. PinyinJiaJia / PYJJ (拼音加加)
//! 6. XiaoHe / XHE

use std::collections::HashMap;

/// Double pinyin schemes supported by the parser (C `DoublePinyinScheme`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoublePinyinScheme {
    /// Microsoft Shuangpin (微软双拼) — `DOUBLE_PINYIN_MS`
    Microsoft,
    /// ZiRanMa (自然码) — `DOUBLE_PINYIN_ZRM`
    ZiRanMa,
    /// ZiGuang (紫光) — `DOUBLE_PINYIN_ZIGUANG`
    ZiGuang,
    /// ABC — `DOUBLE_PINYIN_ABC`
    ABC,
    /// 拼音加加 — `DOUBLE_PINYIN_PYJJ`
    PinyinJiaJia,
    /// XiaoHe (小鹤) — `DOUBLE_PINYIN_XHE`
    XiaoHe,
}

/// One or two finals for a yunmu key (C `m_yunmus[2]`).
#[derive(Debug, Clone, Copy)]
pub struct YunmuChoice {
    pub primary: &'static str,
    pub alternate: Option<&'static str>,
}

/// Mapping tables for a double pinyin scheme.
#[derive(Debug, Clone)]
pub struct DoublePinyinSchemeData {
    pub name: &'static str,
    /// Key → initial. Empty string = zero initial (`'` in C tables).
    pub shengmu: HashMap<char, &'static str>,
    pub yunmu: HashMap<char, YunmuChoice>,
    /// Exact 2-key fallbacks (C `double_pinyin_scheme_fallback_item_t`).
    pub fallback: HashMap<&'static str, &'static str>,
}

impl DoublePinyinScheme {
    /// Canonical config / UI name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Microsoft => "Microsoft",
            Self::ZiRanMa => "ZiRanMa",
            Self::ZiGuang => "ZiGuang",
            Self::ABC => "ABC",
            Self::PinyinJiaJia => "PinyinJiaJia",
            Self::XiaoHe => "XiaoHe",
        }
    }

    /// Parse scheme name (case-insensitive; accepts ibus short codes).
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "microsoft" | "mspy" | "ms" => Some(Self::Microsoft),
            "ziranma" | "zrm" => Some(Self::ZiRanMa),
            "ziguang" | "zgpy" | "zg" => Some(Self::ZiGuang),
            "abc" => Some(Self::ABC),
            "pinyinjiajia" | "pyjj" | "pinyinplusplus" | "pinyin++" | "jiajia" => {
                Some(Self::PinyinJiaJia)
            }
            "xiaohe" | "xhe" | "flypy" => Some(Self::XiaoHe),
            _ => None,
        }
    }

    /// All built-in schemes in ibus display order.
    pub fn all() -> &'static [Self] {
        &[
            Self::Microsoft,
            Self::ZiRanMa,
            Self::ABC,
            Self::ZiGuang,
            Self::PinyinJiaJia,
            Self::XiaoHe,
        ]
    }

    pub fn data(self) -> DoublePinyinSchemeData {
        match self {
            Self::Microsoft => microsoft_scheme(),
            Self::ZiRanMa => ziranma_scheme(),
            Self::ZiGuang => ziguang_scheme(),
            Self::ABC => abc_scheme(),
            Self::PinyinJiaJia => pinyinjiajia_scheme(),
            Self::XiaoHe => xiaohe_scheme(),
        }
    }
}

/// Get scheme data for a given scheme.
pub fn get_scheme_data(scheme: &DoublePinyinScheme) -> DoublePinyinSchemeData {
    scheme.data()
}

fn y(primary: &'static str) -> YunmuChoice {
    YunmuChoice {
        primary,
        alternate: None,
    }
}

fn y2(primary: &'static str, alternate: &'static str) -> YunmuChoice {
    YunmuChoice {
        primary,
        alternate: Some(alternate),
    }
}

fn build(
    name: &'static str,
    shengmu_pairs: &[(char, &'static str)],
    yunmu_pairs: &[(char, YunmuChoice)],
    fallback_pairs: &[(&'static str, &'static str)],
) -> DoublePinyinSchemeData {
    let mut shengmu = HashMap::new();
    for &(k, v) in shengmu_pairs {
        shengmu.insert(k, v);
    }
    let mut yunmu = HashMap::new();
    for &(k, v) in yunmu_pairs {
        yunmu.insert(k, v);
    }
    let mut fallback = HashMap::new();
    for &(k, v) in fallback_pairs {
        fallback.insert(k, v);
    }
    DoublePinyinSchemeData {
        name,
        shengmu,
        yunmu,
        fallback,
    }
}

/// Microsoft / MSPY — `double_pinyin_mspy_*`
fn microsoft_scheme() -> DoublePinyinSchemeData {
    build(
        "Microsoft",
        &[
            ('b', "b"),
            ('c', "c"),
            ('d', "d"),
            ('f', "f"),
            ('g', "g"),
            ('h', "h"),
            ('i', "ch"),
            ('j', "j"),
            ('k', "k"),
            ('l', "l"),
            ('m', "m"),
            ('n', "n"),
            ('o', ""), // zero initial
            ('p', "p"),
            ('q', "q"),
            ('r', "r"),
            ('s', "s"),
            ('t', "t"),
            ('u', "sh"),
            ('v', "zh"),
            ('w', "w"),
            ('x', "x"),
            ('y', "y"),
            ('z', "z"),
        ],
        &[
            ('a', y("a")),
            ('b', y("ou")),
            ('c', y("iao")),
            ('d', y2("uang", "iang")),
            ('e', y("e")),
            ('f', y("en")),
            ('g', y2("eng", "ng")),
            ('h', y("ang")),
            ('i', y("i")),
            ('j', y("an")),
            ('k', y("ao")),
            ('l', y("ai")),
            ('m', y("ian")),
            ('n', y("in")),
            ('o', y2("uo", "o")),
            ('p', y("un")),
            ('q', y("iu")),
            ('r', y2("uan", "er")),
            ('s', y2("ong", "iong")),
            ('t', y("ue")),
            ('u', y("u")),
            ('v', y2("ui", "ue")),
            ('w', y2("ia", "ua")),
            ('x', y("ie")),
            ('y', y2("uai", "v")),
            ('z', y("ei")),
            (';', y("ing")),
        ],
        &[("aa", "a"), ("ee", "e"), ("oo", "o")],
    )
}

/// ZiRanMa / ZRM
fn ziranma_scheme() -> DoublePinyinSchemeData {
    build(
        "ZiRanMa",
        &[
            ('b', "b"),
            ('c', "c"),
            ('d', "d"),
            ('f', "f"),
            ('g', "g"),
            ('h', "h"),
            ('i', "ch"),
            ('j', "j"),
            ('k', "k"),
            ('l', "l"),
            ('m', "m"),
            ('n', "n"),
            ('o', ""),
            ('p', "p"),
            ('q', "q"),
            ('r', "r"),
            ('s', "s"),
            ('t', "t"),
            ('u', "sh"),
            ('v', "zh"),
            ('w', "w"),
            ('x', "x"),
            ('y', "y"),
            ('z', "z"),
        ],
        &[
            ('a', y("a")),
            ('b', y("ou")),
            ('c', y("iao")),
            ('d', y2("uang", "iang")),
            ('e', y("e")),
            ('f', y("en")),
            ('g', y2("eng", "ng")),
            ('h', y("ang")),
            ('i', y("i")),
            ('j', y("an")),
            ('k', y("ao")),
            ('l', y("ai")),
            ('m', y("ian")),
            ('n', y("in")),
            ('o', y2("uo", "o")),
            ('p', y("un")),
            ('q', y("iu")),
            ('r', y2("uan", "er")),
            ('s', y2("ong", "iong")),
            ('t', y("ue")),
            ('u', y("u")),
            ('v', y2("ui", "v")),
            ('w', y2("ia", "ua")),
            ('x', y("ie")),
            ('y', y2("uai", "ing")),
            ('z', y("ei")),
        ],
        &[
            ("aa", "a"),
            ("ai", "ai"),
            ("an", "an"),
            ("ah", "ang"),
            ("ao", "ao"),
            ("ee", "e"),
            ("ei", "ei"),
            ("en", "en"),
            ("er", "er"),
            ("oo", "o"),
            ("ou", "ou"),
        ],
    )
}

/// ZiGuang / ZGPY
fn ziguang_scheme() -> DoublePinyinSchemeData {
    build(
        "ZiGuang",
        &[
            ('a', "ch"),
            ('b', "b"),
            ('c', "c"),
            ('d', "d"),
            ('f', "f"),
            ('g', "g"),
            ('h', "h"),
            ('i', "sh"),
            ('j', "j"),
            ('k', "k"),
            ('l', "l"),
            ('m', "m"),
            ('n', "n"),
            ('o', ""),
            ('p', "p"),
            ('q', "q"),
            ('r', "r"),
            ('s', "s"),
            ('t', "t"),
            ('u', "zh"),
            ('w', "w"),
            ('x', "x"),
            ('y', "y"),
            ('z', "z"),
        ],
        &[
            ('a', y("a")),
            ('b', y("iao")),
            ('d', y("ie")),
            ('e', y("e")),
            ('f', y("ian")),
            ('g', y2("iang", "uang")),
            ('h', y2("ong", "iong")),
            ('i', y("i")),
            ('j', y2("er", "iu")),
            ('k', y("ei")),
            ('l', y("uan")),
            ('m', y("un")),
            ('n', y2("ue", "ui")),
            ('o', y2("uo", "o")),
            ('p', y("ai")),
            ('q', y("ao")),
            ('r', y("an")),
            ('s', y("ang")),
            ('t', y2("eng", "ng")),
            ('u', y("u")),
            ('v', y("v")),
            ('w', y("en")),
            ('x', y2("ia", "ua")),
            ('y', y2("in", "uai")),
            ('z', y("ou")),
            (';', y("ing")),
        ],
        &[("aa", "a"), ("ee", "e"), ("oo", "o")],
    )
}

/// ABC
fn abc_scheme() -> DoublePinyinSchemeData {
    build(
        "ABC",
        &[
            ('a', "zh"),
            ('b', "b"),
            ('c', "c"),
            ('d', "d"),
            ('e', "ch"),
            ('f', "f"),
            ('g', "g"),
            ('h', "h"),
            ('j', "j"),
            ('k', "k"),
            ('l', "l"),
            ('m', "m"),
            ('n', "n"),
            ('o', ""),
            ('p', "p"),
            ('q', "q"),
            ('r', "r"),
            ('s', "s"),
            ('t', "t"),
            ('v', "sh"),
            ('w', "w"),
            ('x', "x"),
            ('y', "y"),
            ('z', "z"),
        ],
        &[
            ('a', y("a")),
            ('b', y("ou")),
            ('c', y2("in", "uai")),
            ('d', y2("ia", "ua")),
            ('e', y("e")),
            ('f', y("en")),
            ('g', y2("eng", "ng")),
            ('h', y("ang")),
            ('i', y("i")),
            ('j', y("an")),
            ('k', y("ao")),
            ('l', y("ai")),
            ('m', y2("ue", "ui")),
            ('n', y("un")),
            ('o', y2("uo", "o")),
            ('p', y("uan")),
            ('q', y("ei")),
            ('r', y2("er", "iu")),
            ('s', y2("ong", "iong")),
            ('t', y2("iang", "uang")),
            ('u', y("u")),
            ('v', y2("v", "ue")),
            ('w', y("ian")),
            ('x', y("ie")),
            ('y', y("ing")),
            ('z', y("iao")),
        ],
        &[("aa", "a"), ("ee", "e"), ("oo", "o")],
    )
}

/// 拼音加加 / PYJJ — was incorrectly named PinYin++ with MSPY-like maps.
fn pinyinjiajia_scheme() -> DoublePinyinSchemeData {
    build(
        "PinyinJiaJia",
        &[
            ('a', ""), // zero initial
            ('b', "b"),
            ('c', "c"),
            ('d', "d"),
            ('f', "f"),
            ('g', "g"),
            ('h', "h"),
            ('i', "sh"),
            ('j', "j"),
            ('k', "k"),
            ('l', "l"),
            ('m', "m"),
            ('n', "n"),
            ('o', ""),
            ('p', "p"),
            ('q', "q"),
            ('r', "r"),
            ('s', "s"),
            ('t', "t"),
            ('u', "ch"),
            ('v', "zh"),
            ('w', "w"),
            ('x', "x"),
            ('y', "y"),
            ('z', "z"),
        ],
        &[
            ('a', y("a")),
            ('b', y2("ia", "ua")),
            ('c', y("uan")),
            ('d', y("ao")),
            ('e', y("e")),
            ('f', y("an")),
            ('g', y("ang")),
            ('h', y2("iang", "uang")),
            ('i', y("i")),
            ('j', y("ian")),
            ('k', y("iao")),
            ('l', y("in")),
            ('m', y("ie")),
            ('n', y("iu")),
            ('o', y2("uo", "o")),
            ('p', y("ou")),
            ('q', y2("er", "ing")),
            ('r', y("en")),
            ('s', y("ai")),
            ('t', y2("eng", "ng")),
            ('u', y("u")),
            ('v', y2("v", "ui")),
            ('w', y("ei")),
            ('x', y2("uai", "ue")),
            ('y', y2("ong", "iong")),
            ('z', y("un")),
        ],
        &[
            ("aa", "a"),
            ("as", "ai"),
            ("af", "an"),
            ("ag", "ang"),
            ("ad", "ao"),
            ("ee", "e"),
            ("ew", "ei"),
            ("er", "en"),
            ("eq", "er"),
            ("oo", "o"),
            ("op", "ou"),
        ],
    )
}

/// XiaoHe / XHE
fn xiaohe_scheme() -> DoublePinyinSchemeData {
    build(
        "XiaoHe",
        &[
            ('b', "b"),
            ('c', "c"),
            ('d', "d"),
            ('f', "f"),
            ('g', "g"),
            ('h', "h"),
            ('i', "ch"),
            ('j', "j"),
            ('k', "k"),
            ('l', "l"),
            ('m', "m"),
            ('n', "n"),
            ('o', ""),
            ('p', "p"),
            ('q', "q"),
            ('r', "r"),
            ('s', "s"),
            ('t', "t"),
            ('u', "sh"),
            ('v', "zh"),
            ('w', "w"),
            ('x', "x"),
            ('y', "y"),
            ('z', "z"),
        ],
        &[
            ('a', y("a")),
            ('b', y("in")),
            ('c', y("ao")),
            ('d', y("ai")),
            ('e', y("e")),
            ('f', y("en")),
            ('g', y2("eng", "ng")),
            ('h', y("ang")),
            ('i', y("i")),
            ('j', y("an")),
            ('k', y2("uai", "ing")),
            ('l', y2("iang", "uang")),
            ('m', y("ian")),
            ('n', y("iao")),
            ('o', y2("uo", "o")),
            ('p', y("ie")),
            ('q', y("iu")),
            ('r', y2("uan", "er")),
            ('s', y2("ong", "iong")),
            ('t', y("ue")),
            ('u', y("u")),
            ('v', y2("v", "ui")),
            ('w', y("ei")),
            ('x', y2("ia", "ua")),
            ('y', y("un")),
            ('z', y("ou")),
        ],
        &[
            ("aa", "a"),
            ("ai", "ai"),
            ("an", "an"),
            ("ah", "ang"),
            ("ao", "ao"),
            ("ee", "e"),
            ("ei", "ei"),
            ("en", "en"),
            ("er", "er"),
            ("oo", "o"),
            ("ou", "ou"),
        ],
    )
}

/// Pick primary vs alternate final using initial (j/q/x/y / zero-initial rules).
fn pick_yunmu(initial: &str, choice: &YunmuChoice) -> &'static str {
    let Some(alt) = choice.alternate else {
        return choice.primary;
    };
    let a = choice.primary;
    let b = alt;
    let i_group = matches!(initial, "j" | "q" | "x" | "y");
    let zero = initial.is_empty();

    // Order pairs as stored in C tables; choose by initial class.
    match (a, b) {
        ("uang", "iang") | ("iang", "uang") => {
            if i_group {
                "iang"
            } else {
                "uang"
            }
        }
        ("ia", "ua") | ("ua", "ia") => {
            if i_group {
                "ia"
            } else {
                "ua"
            }
        }
        ("ong", "iong") | ("iong", "ong") => {
            if i_group {
                "iong"
            } else {
                "ong"
            }
        }
        ("eng", "ng") => {
            if zero {
                "ng"
            } else {
                "eng"
            }
        }
        ("uo", "o") => {
            if zero || matches!(initial, "b" | "p" | "m" | "f") {
                "o"
            } else {
                "uo"
            }
        }
        ("uan", "er") | ("er", "uan") | ("er", "iu") | ("iu", "er") => {
            if zero {
                "er"
            } else if a == "er" {
                b
            } else {
                a
            }
        }
        ("er", "ing") => {
            if zero {
                "er"
            } else {
                "ing"
            }
        }
        ("ui", "ue") | ("ue", "ui") => {
            if i_group {
                "ue"
            } else {
                "ui"
            }
        }
        ("ui", "v") | ("v", "ui") => {
            if i_group {
                "v"
            } else {
                "ui"
            }
        }
        ("uai", "v") => {
            if i_group {
                "v"
            } else {
                "uai"
            }
        }
        ("uai", "ing") | ("ing", "uai") => {
            if i_group || matches!(initial, "ch" | "sh" | "zh" | "r") {
                // XiaoHe/ZRM: k→uai/ing — ing after zh/ch/sh/r; uai otherwise
                if matches!(initial, "zh" | "ch" | "sh" | "r" | "y") {
                    "ing"
                } else {
                    "uai"
                }
            } else {
                "uai"
            }
        }
        ("uai", "ue") | ("ue", "uai") => {
            if i_group {
                "ue"
            } else {
                "uai"
            }
        }
        ("in", "uai") => {
            if i_group {
                "in"
            } else {
                "uai"
            }
        }
        ("v", "ue") => {
            if i_group {
                "ue"
            } else {
                "v"
            }
        }
        _ => {
            if i_group && (b.starts_with('i') || b == "v" || b == "ue") {
                b
            } else {
                a
            }
        }
    }
}

fn is_scheme_key(ch: char) -> bool {
    ch.is_ascii_lowercase() || ch == ';'
}

/// Convert 2-key double pinyin input to full pinyin syllable.
pub fn double_to_full_pinyin(
    first: char,
    second: char,
    scheme: &DoublePinyinSchemeData,
) -> Option<String> {
    if !is_scheme_key(first) || !is_scheme_key(second) {
        return None;
    }

    let pair: String = [first, second].iter().collect();
    if let Some(&syllable) = scheme.fallback.get(pair.as_str()) {
        return Some(syllable.to_string());
    }

    let initial = *scheme.shengmu.get(&first)?;
    let choice = scheme.yunmu.get(&second)?;
    let final_part = pick_yunmu(initial, choice);
    Some(format!("{initial}{final_part}"))
}
