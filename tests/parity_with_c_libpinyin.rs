//! Parity suite: Rust libpinyin vs system C/C++ libpinyin across features.
//!
//! This suite is **C fidelity**, not the product UX bar. For “good enough for
//! pinyin users” gates (daily vocab, progressive incompletes, Best UX sentences),
//! see `product_quality.rs`. Do not chase the last few percent of broad C
//! top-5 overlap at the expense of UX ranking or mid-type incompletes.
//!
//! # Covered features
//! - Strict full-pinyin segmentation
//! - Incomplete pinyin (`PINYIN_INCOMPLETE`)
//! - Each correction flag (`PINYIN_CORRECT_*`)
//! - Each fuzzy AMB flag (`PINYIN_AMB_*`)
//! - Combined full IME options
//! - Single-syllable recognition
//! - Candidate top-1 / top-5 overlap (smoke 100%/100%; broad ≥95%/≥90%)
//! - Long composed sentences (`pinyin_get_sentence` parity via emit option)
//! - Progressive typing buffers
//! - Double pinyin (Microsoft scheme) segmentation
//!
//! # Intentional non-goals (do not complicate Rust for these)
//! - Bit-identical LM scores / exact top-5 order
//! - Matching C on rare ambiguity ties (他/她, 买/卖, …)
//! - English / emoji mixed-input extras (Rust-only enhancements)
//!
//! # Configurable ranking (`Config` / engine setters)
//! - Product default: `emit_composed_sentences=true` + `MatchedSpanThenScore`
//! - This suite forces C-compat: emit off + `PhraseLengthThenScore`
//!
//! Run:
//! ```text
//! cargo test -p libpinyin --test parity_with_c_libpinyin -- --ignored --nocapture
//! # or: ./scripts/run_c_parity.sh
//! ```

mod common;

use std::collections::HashSet;

use common::c_options::{
    self, PINYIN_AMB_ALL, PINYIN_AMB_AN_ANG, PINYIN_AMB_C_CH, PINYIN_AMB_EN_ENG, PINYIN_AMB_F_H,
    PINYIN_AMB_G_K, PINYIN_AMB_IN_ING, PINYIN_AMB_L_N, PINYIN_AMB_L_R, PINYIN_AMB_S_SH,
    PINYIN_AMB_Z_ZH, PINYIN_CORRECT_ALL, PINYIN_CORRECT_GN_NG, PINYIN_CORRECT_IOU_IU,
    PINYIN_CORRECT_MG_NG, PINYIN_CORRECT_ON_ONG, PINYIN_CORRECT_UEI_UI, PINYIN_CORRECT_UEN_UN,
    PINYIN_CORRECT_UE_VE, PINYIN_CORRECT_V_U, PINYIN_INCOMPLETE, PINYIN_OPTIONS_FULL,
    PINYIN_OPTIONS_STRICT,
};
use common::{
    apply_best_ux, apply_c_compat, isolate_userdict, load_engine, lock_engine_tests, CLibPinyin,
    CORE_CANDIDATE_INPUTS, LONG_SENTENCE_CASES, PROGRESSIVE_INPUTS, SMOKE_CANDIDATE_INPUTS,
    TEST_INPUTS,
};
use libpinyin::{Parser, PINYIN_SYLLABLES};

const C_USER_DIR: &str = "/tmp/libpinyin_parity_user";

fn rust_parser(options: u32) -> Parser {
    let cfg = c_options::from_c_options(options);
    Parser::with_syllables_and_config(PINYIN_SYLLABLES, &cfg)
}

fn rust_seg(parser: &Parser, input: &str, soft: bool) -> Vec<String> {
    parser
        .segment_best(input, soft)
        .into_iter()
        .map(|s| s.text.to_lowercase())
        .collect()
}

fn assert_seg_ratio(
    name: &str,
    ok: usize,
    total: usize,
    mismatches: &[(String, Vec<String>, Vec<String>)],
    min_ratio: f64,
) {
    let ratio = if total == 0 {
        0.0
    } else {
        ok as f64 / total as f64
    };
    println!("[{name}] {ok}/{total} ({:.1}%)", 100.0 * ratio);
    for (input, c_seg, r_seg) in mismatches.iter().take(12) {
        println!("  {input:12} C={c_seg:?} Rust={r_seg:?}");
    }
    assert!(
        ratio >= min_ratio,
        "[{name}] seg match {:.1}% < {:.0}%",
        ratio * 100.0,
        min_ratio * 100.0
    );
}

#[allow(clippy::type_complexity)]
fn compare_seg_list(
    c: &CLibPinyin,
    options: u32,
    soft: bool,
    inputs: &[(&str, /*exact*/ bool)],
) -> (usize, usize, Vec<(String, Vec<String>, Vec<String>)>) {
    c.set_options(options);
    let parser = rust_parser(options);
    let mut ok = 0usize;
    let mut total = 0usize;
    let mut mismatches = Vec::new();
    for &(input, _exact) in inputs {
        let c_seg = c.parse_normalized(input);
        let r_seg = rust_seg(&parser, input, soft);
        total += 1;
        if c_seg == r_seg {
            ok += 1;
        } else {
            mismatches.push((input.to_string(), c_seg, r_seg));
        }
    }
    (ok, total, mismatches)
}

// ---------------------------------------------------------------------------
// Feature corpora
// ---------------------------------------------------------------------------

const STRICT_INPUTS: &[&str] = &[
    "nihao",
    "zhongguo",
    "beijing",
    "shanghai",
    "xiexie",
    "putonghua",
    "xian",
    "women",
    "yinggai",
    "chenggong",
    "shijian",
    "gongzuo",
    "mingtian",
    "jintian",
    "pengyou",
];

const INCOMPLETE_INPUTS: &[&str] = &[
    "n", "ni", "nih", "niha", "zh", "zho", "zhon", "zhong", "zhongg", "b", "be", "bei", "beij",
    "sh", "x", "xi", "xin", "ch", "c", "z",
];

/// One representative input per correction flag.
const CORRECT_CASES: &[(&str, u32)] = &[
    ("nue", PINYIN_CORRECT_UE_VE),
    ("lve", PINYIN_CORRECT_UE_VE),
    ("nv", PINYIN_CORRECT_V_U),
    ("liou", PINYIN_CORRECT_IOU_IU),
    ("guei", PINYIN_CORRECT_UEI_UI),
    ("duen", PINYIN_CORRECT_UEN_UN),
    ("bagn", PINYIN_CORRECT_GN_NG),
    ("bamg", PINYIN_CORRECT_MG_NG),
    ("zon", PINYIN_CORRECT_ON_ONG),
];

/// One representative input per AMB flag.
const AMB_CASES: &[(&str, u32)] = &[
    ("zongguo", PINYIN_AMB_Z_ZH),
    ("ceng", PINYIN_AMB_C_CH),
    ("sui", PINYIN_AMB_S_SH),
    ("fan", PINYIN_AMB_AN_ANG),
    ("fen", PINYIN_AMB_EN_ENG),
    ("bin", PINYIN_AMB_IN_ING),
    ("nihao", PINYIN_AMB_L_N), // may stay identical; still exercises flag path
    ("fang", PINYIN_AMB_F_H),
    ("gai", PINYIN_AMB_G_K),
    ("ren", PINYIN_AMB_L_R),
];

const FULL_INPUTS: &[&str] = &[
    "nihao", "zhongguo", "zongguo", "nue", "lve", "n", "zh", "zhon", "beijin", "fan", "yinggai",
    "liou", "guei", "xiexie", "shanghai",
];

const DOUBLE_MS_INPUTS: &[&str] = &[
    "ui", // shi
    "gh", // gang
    "ji", "uihf", // shi hen (partial progressive)
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn parity_strict_segmentation() {
    let Some(c) = CLibPinyin::new(C_USER_DIR) else {
        eprintln!("SKIP: system libpinyin unavailable");
        return;
    };
    let inputs: Vec<_> = STRICT_INPUTS.iter().map(|s| (*s, true)).collect();
    let (ok, total, mism) = compare_seg_list(&c, PINYIN_OPTIONS_STRICT, false, &inputs);
    assert_seg_ratio("strict", ok, total, &mism, 0.90);
}

#[test]
#[ignore]
fn parity_incomplete_pinyin() {
    let Some(c) = CLibPinyin::new(C_USER_DIR) else {
        eprintln!("SKIP: system libpinyin unavailable");
        return;
    };
    let inputs: Vec<_> = INCOMPLETE_INPUTS.iter().map(|s| (*s, true)).collect();
    let (ok, total, mism) =
        compare_seg_list(&c, PINYIN_OPTIONS_STRICT | PINYIN_INCOMPLETE, true, &inputs);
    assert_seg_ratio("incomplete", ok, total, &mism, 0.85);
}

#[test]
#[ignore]
fn parity_each_correction_flag() {
    let Some(c) = CLibPinyin::new(C_USER_DIR) else {
        eprintln!("SKIP: system libpinyin unavailable");
        return;
    };
    let mut ok = 0usize;
    let mut total = 0usize;
    let mut mism = Vec::new();
    for &(input, flag) in CORRECT_CASES {
        let options = PINYIN_OPTIONS_STRICT | flag;
        c.set_options(options);
        let parser = rust_parser(options);
        let c_seg = c.parse_normalized(input);
        let r_seg = rust_seg(&parser, input, true);
        total += 1;
        if c_seg == r_seg {
            ok += 1;
        } else {
            mism.push((format!("{input}/0x{flag:x}"), c_seg, r_seg));
        }
    }
    // Also all-corrections together.
    let all_inputs: Vec<_> = CORRECT_CASES.iter().map(|(s, _)| (*s, true)).collect();
    let (ok2, total2, mism2) = compare_seg_list(
        &c,
        PINYIN_OPTIONS_STRICT | PINYIN_CORRECT_ALL,
        true,
        &all_inputs,
    );
    ok += ok2;
    total += total2;
    mism.extend(mism2);
    assert_seg_ratio("corrections", ok, total, &mism, 0.85);
}

#[test]
#[ignore]
fn parity_each_fuzzy_amb_flag() {
    let Some(c) = CLibPinyin::new(C_USER_DIR) else {
        eprintln!("SKIP: system libpinyin unavailable");
        return;
    };
    let mut ok = 0usize;
    let mut total = 0usize;
    let mut mism = Vec::new();
    for &(input, flag) in AMB_CASES {
        let options = PINYIN_OPTIONS_STRICT | flag;
        c.set_options(options);
        let parser = rust_parser(options);
        let c_seg = c.parse_normalized(input);
        let r_seg = rust_seg(&parser, input, true);
        total += 1;
        if c_seg == r_seg {
            ok += 1;
        } else {
            mism.push((format!("{input}/0x{flag:x}"), c_seg, r_seg));
        }
    }
    let all_inputs: Vec<_> = AMB_CASES.iter().map(|(s, _)| (*s, true)).collect();
    let (ok2, total2, mism2) = compare_seg_list(
        &c,
        PINYIN_OPTIONS_STRICT | PINYIN_AMB_ALL,
        true,
        &all_inputs,
    );
    ok += ok2;
    total += total2;
    mism.extend(mism2);
    assert_seg_ratio("fuzzy-amb", ok, total, &mism, 0.75);
}

#[test]
#[ignore]
fn parity_full_ime_options() {
    let Some(c) = CLibPinyin::new(C_USER_DIR) else {
        eprintln!("SKIP: system libpinyin unavailable");
        return;
    };
    let inputs: Vec<_> = FULL_INPUTS.iter().map(|s| (*s, true)).collect();
    let (ok, total, mism) = compare_seg_list(&c, PINYIN_OPTIONS_FULL, true, &inputs);
    assert_seg_ratio("full", ok, total, &mism, 0.90);
}

#[test]
#[ignore]
fn parity_single_syllable_recognition() {
    let Some(c) = CLibPinyin::new(C_USER_DIR) else {
        eprintln!("SKIP: system libpinyin unavailable");
        return;
    };
    c.set_options(PINYIN_OPTIONS_STRICT);
    let parser = rust_parser(PINYIN_OPTIONS_STRICT);
    let mut both = 0usize;
    let mut rust_miss = Vec::new();
    for &syl in PINYIN_SYLLABLES.iter().take(200) {
        // Sample head of the table for runtime; full table is huge.
        let c_seg = c.parse_normalized(syl);
        let r_seg = rust_seg(&parser, syl, false);
        let c_ok = c_seg.len() == 1 && c_seg[0] == syl;
        let r_ok = r_seg.len() == 1 && r_seg[0] == syl;
        if c_ok && r_ok {
            both += 1;
        } else if c_ok && !r_ok {
            rust_miss.push((syl, c_seg, r_seg));
        }
    }
    println!(
        "[single-syllable] both_ok={both}/200 rust_miss={}",
        rust_miss.len()
    );
    for (s, c_seg, r_seg) in rust_miss.iter().take(10) {
        println!("  {s:8} C={c_seg:?} Rust={r_seg:?}");
    }
    assert!(
        rust_miss.len() <= 10,
        "too many Rust single-syllable misses: {}",
        rust_miss.len()
    );
}

#[test]
#[ignore]
fn parity_candidates_and_beijing_top1() {
    let _lock = lock_engine_tests();
    isolate_userdict("parity");
    let Some(c) = CLibPinyin::new(C_USER_DIR) else {
        eprintln!("SKIP: system libpinyin unavailable");
        return;
    };
    let mut engine = load_engine();
    apply_c_compat(&mut engine);

    c.set_options(PINYIN_OPTIONS_STRICT);
    engine.set_allow_fuzzy(false);
    let c_bj = c.candidates("beijing", 5);
    assert!(
        !c_bj.is_empty() && c_bj[0] == "北京",
        "C top-1 for beijing should be 北京, got {c_bj:?}"
    );
    let r_bj: Vec<String> = engine
        .input("beijing")
        .into_iter()
        .take(5)
        .map(|x| x.text)
        .collect();
    assert!(
        !r_bj.is_empty() && r_bj[0] == "北京",
        "Rust top-1 for beijing should be 北京, got {r_bj:?}"
    );

    const TOP_N: usize = 5;
    let mut failures = Vec::new();
    for (name, options, min_top1, min_overlap, rust_fuzzy) in [
        // Faithful C port targets (algorithm + overlaid lexicon freqs).
        (
            "cand-strict",
            PINYIN_OPTIONS_STRICT,
            1.0_f64,
            1.0_f64,
            false,
        ),
        ("cand-full", PINYIN_OPTIONS_FULL, 1.0, 1.0, true),
    ] {
        c.set_options(options);
        engine.set_allow_fuzzy(rust_fuzzy);
        let mut compared = 0usize;
        let mut top1 = 0usize;
        let mut overlap_sum = 0.0f64;
        let mut mismatches = Vec::new();
        for &input in SMOKE_CANDIDATE_INPUTS {
            let c_top = c.candidates(input, TOP_N);
            if c_top.is_empty() {
                continue;
            }
            let r_top: Vec<String> = engine
                .input(input)
                .into_iter()
                .take(TOP_N)
                .map(|x| x.text)
                .collect();
            if r_top.is_empty() {
                continue;
            }
            compared += 1;
            if r_top[0] == c_top[0] {
                top1 += 1;
            } else {
                mismatches.push((input, format!("{c_top:?}"), format!("{r_top:?}")));
            }
            let set: HashSet<&str> = c_top.iter().map(|s| s.as_str()).collect();
            let overlap = r_top.iter().filter(|t| set.contains(t.as_str())).count();
            overlap_sum += overlap as f64 / TOP_N as f64;
        }
        let top1_ratio = top1 as f64 / compared.max(1) as f64;
        let avg_overlap = overlap_sum / compared.max(1) as f64;
        println!(
            "\n[{name}] compared={compared} top1={:.1}% overlap={:.1}%",
            100.0 * top1_ratio,
            100.0 * avg_overlap
        );
        for (i, c0, r0) in mismatches.iter().take(10) {
            println!("  {i:12}\n    C={c0}\n    R={r0}");
        }
        if compared < 8 {
            failures.push(format!("[{name}] too few comparisons ({compared})"));
        }
        if top1_ratio < min_top1 {
            failures.push(format!(
                "[{name}] top-1 {:.1}% < {:.0}%",
                top1_ratio * 100.0,
                min_top1 * 100.0
            ));
        }
        if avg_overlap < min_overlap {
            failures.push(format!(
                "[{name}] overlap {:.1}% < {:.0}%",
                avg_overlap * 100.0,
                min_overlap * 100.0
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "candidate parity gaps:\n  {}",
        failures.join("\n  ")
    );
}

/// Broad corpus (≥500 inputs) + progressive buffers — high but not bit-identical gates.
#[test]
#[ignore]
fn parity_candidates_broad_corpus() {
    let _lock = lock_engine_tests();
    isolate_userdict("parity-broad");
    let Some(c) = CLibPinyin::new(C_USER_DIR) else {
        eprintln!("SKIP: system libpinyin unavailable");
        return;
    };
    let mut engine = load_engine();
    apply_c_compat(&mut engine);

    let mut inputs: Vec<&str> = Vec::new();
    let mut seen = HashSet::new();
    for &s in CORE_CANDIDATE_INPUTS
        .iter()
        .chain(TEST_INPUTS.iter())
        .chain(PROGRESSIVE_INPUTS.iter())
    {
        if seen.insert(s) {
            inputs.push(s);
        }
    }

    const TOP_N: usize = 5;
    let mut failures = Vec::new();
    for (name, options, min_top1, min_overlap, rust_fuzzy) in [
        (
            "broad-strict",
            PINYIN_OPTIONS_STRICT,
            0.95_f64,
            0.90_f64,
            false,
        ),
        ("broad-full", PINYIN_OPTIONS_FULL, 0.95, 0.90, true),
    ] {
        c.set_options(options);
        engine.set_allow_fuzzy(rust_fuzzy);
        let mut compared = 0usize;
        let mut top1 = 0usize;
        let mut overlap_sum = 0.0f64;
        let mut mismatches = Vec::new();
        for &input in &inputs {
            let c_top = c.candidates(input, TOP_N);
            if c_top.is_empty() {
                continue;
            }
            let r_top: Vec<String> = engine
                .input(input)
                .into_iter()
                .take(TOP_N)
                .map(|x| x.text)
                .collect();
            if r_top.is_empty() {
                continue;
            }
            compared += 1;
            if r_top[0] == c_top[0] {
                top1 += 1;
            } else if mismatches.len() < 30 {
                mismatches.push((input, c_top[0].clone(), r_top[0].clone()));
            }
            let set: HashSet<&str> = c_top.iter().map(|s| s.as_str()).collect();
            let overlap = r_top.iter().filter(|t| set.contains(t.as_str())).count();
            overlap_sum += overlap as f64 / TOP_N as f64;
        }
        let top1_ratio = top1 as f64 / compared.max(1) as f64;
        let avg_overlap = overlap_sum / compared.max(1) as f64;
        println!(
            "\n[{name}] compared={compared} top1={:.1}% overlap={:.1}%",
            100.0 * top1_ratio,
            100.0 * avg_overlap
        );
        for (i, c0, r0) in &mismatches {
            println!("  {i:20} C={c0} Rust={r0}");
        }
        if compared < 50 {
            failures.push(format!("[{name}] too few comparisons ({compared})"));
        }
        if top1_ratio < min_top1 {
            failures.push(format!(
                "[{name}] top-1 {:.1}% < {:.0}%",
                top1_ratio * 100.0,
                min_top1 * 100.0
            ));
        }
        if avg_overlap < min_overlap {
            failures.push(format!(
                "[{name}] overlap {:.1}% < {:.0}%",
                avg_overlap * 100.0,
                min_overlap * 100.0
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "broad candidate parity gaps:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
#[ignore]
fn parity_double_pinyin_microsoft() {
    let Some(c) = CLibPinyin::new(C_USER_DIR) else {
        eprintln!("SKIP: system libpinyin unavailable");
        return;
    };
    c.set_options(PINYIN_OPTIONS_STRICT);
    c.set_double_pinyin_microsoft();
    let parser = rust_parser(PINYIN_OPTIONS_STRICT);

    let mut ok = 0usize;
    let mut total = 0usize;
    let mut mism = Vec::new();
    for &input in DOUBLE_MS_INPUTS {
        let c_seg = c.parse_double(input);
        // Skip empty C parses (scheme edge cases).
        if c_seg.is_empty() {
            continue;
        }
        let r_seg: Vec<String> = parser
            .segment_with_scheme(input, false, Some("Microsoft"))
            .into_iter()
            .map(|s| s.text.to_lowercase())
            .collect();
        total += 1;
        if c_seg == r_seg {
            ok += 1;
        } else {
            mism.push((input.to_string(), c_seg, r_seg));
        }
    }
    assert_seg_ratio("double-ms", ok, total, &mism, 0.50);
}

#[test]
fn pinyin_config_roundtrips_c_option_bits() {
    let full = c_options::from_c_options(PINYIN_OPTIONS_FULL);
    assert!(full.pinyin_incomplete);
    assert!(full.correct_ue_ve);
    assert!(!full.base.fuzzy.is_empty());
    let bits = c_options::to_c_options(&full);
    assert_eq!(bits & PINYIN_INCOMPLETE, PINYIN_INCOMPLETE);
    assert_eq!(bits & PINYIN_CORRECT_ALL, PINYIN_CORRECT_ALL);
    assert_eq!(bits & PINYIN_AMB_ALL, PINYIN_AMB_ALL);

    let strict = c_options::from_c_options(PINYIN_OPTIONS_STRICT);
    assert!(!strict.pinyin_incomplete);
    assert!(!strict.corrections_enabled());
    assert!(strict.base.fuzzy.is_empty());
}

#[test]
fn feature_matrix_is_documented() {
    // Guardrail: keep this suite's feature list explicit so new flags get tests.
    let features = [
        "strict_segmentation",
        "incomplete_pinyin",
        "each_correction_flag",
        "each_fuzzy_amb_flag",
        "full_ime_options",
        "single_syllable_recognition",
        "candidates_and_beijing_top1",
        "candidates_broad_corpus",
        "double_pinyin_microsoft",
        "long_composed_sentences",
        // Product UX bar lives in product_quality.rs (not C bit-identity).
        "see_also_product_quality_bar",
    ];
    assert_eq!(features.len(), 11);
    assert!(CORRECT_CASES.len() >= 8);
    assert!(AMB_CASES.len() >= 8);
    assert!(TEST_INPUTS.len() >= 400);
    assert!(PROGRESSIVE_INPUTS.len() >= 20);
    assert!(LONG_SENTENCE_CASES.len() >= 6);
    assert!(SMOKE_CANDIDATE_INPUTS.len() >= 10);
    assert!(CORE_CANDIDATE_INPUTS.len() >= SMOKE_CANDIDATE_INPUTS.len());
}

#[test]
#[ignore]
fn parity_long_composed_sentences() {
    let _lock = lock_engine_tests();
    isolate_userdict("parity-long");
    let Some(c) = CLibPinyin::new(C_USER_DIR) else {
        eprintln!("SKIP: system libpinyin unavailable");
        return;
    };
    let mut engine = load_engine();
    engine.set_allow_fuzzy(true);
    apply_best_ux(&mut engine);
    c.set_options(PINYIN_OPTIONS_FULL);

    let mut ok = 0usize;
    let mut c_ok = 0usize;
    let mut present = 0usize;
    for &(input, expect) in LONG_SENTENCE_CASES {
        let c_sent = c.sentence(input);
        let r_top: Vec<String> = engine
            .input(input)
            .into_iter()
            .take(8)
            .map(|x| x.text)
            .collect();
        println!("\n{input}\n  C_sentence={c_sent:?}\n  R_top={r_top:?}\n  expect={expect}");
        if c_sent.as_deref() == Some(expect) {
            c_ok += 1;
        }
        if r_top.iter().any(|t| t == expect) {
            present += 1;
        }
        if r_top.first().map(|s| s.as_str()) == Some(expect) {
            ok += 1;
        }
    }
    let n = LONG_SENTENCE_CASES.len();
    println!("\n[long-sentences] C_match={c_ok}/{n} rust_present={present}/{n} top1={ok}/{n}");
    // C should agree with the gold labels for most; Rust must surface them.
    assert!(
        c_ok >= n.saturating_sub(2),
        "C sentence labels drifted: {c_ok}/{n}"
    );
    assert!(
        present >= n.saturating_sub(1),
        "Rust missing too many composed sentences: {present}/{n}"
    );
    assert!(
        ok >= (n * 3 / 4),
        "composed sentence top-1 too low: {ok}/{n}"
    );
}
