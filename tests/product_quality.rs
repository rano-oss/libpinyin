//! Product “good enough” bar for pinyin users (not C bit-identity).
//!
//! Gates:
//! - Daily vocabulary top-1 ≥ 99% (Best UX ranking + daily preference)
//! - Progressive mid-word prefixes: intended word in top-5 ≥ 95%
//! - Progressive incomplete finals: intended in top-5 ≥ 95%
//! - Long continuous sentences: gold in top-1 ≥ 75% (Best UX)
//!
//! C↔Rust fidelity stays in `parity_with_c_libpinyin` / `compare_with_c_libpinyin`
//! (those suites force C-compat knobs explicitly).
//!
//! ```text
//! cargo test -p libpinyin --test product_quality -- --nocapture
//! ```

mod common;

use common::{
    c_compat_engine, data_dir, isolate_userdict, lock_engine_tests, ux_engine, DAILY_VOCAB_CASES,
    LONG_SENTENCE_CASES, PROGRESSIVE_PREFIX_CASES, PROGRESSIVE_TARGET_CASES,
};
use libpinyin::CandidateRankMode;

#[test]
fn defaults_are_best_ux() {
    let _lock = lock_engine_tests();
    isolate_userdict("product-defaults");
    let engine = libpinyin::Engine::from_data_dir(data_dir()).expect("engine");
    assert!(engine.emit_composed_sentences());
    assert_eq!(
        engine.candidate_rank_mode(),
        CandidateRankMode::MatchedSpanThenScore
    );
}

#[test]
fn daily_vocab_top1_at_least_99_percent() {
    let _lock = lock_engine_tests();
    isolate_userdict("product-daily");
    let mut engine = ux_engine();

    let mut ok = 0usize;
    let mut misses = Vec::new();
    for &(input, expect) in DAILY_VOCAB_CASES {
        let top = engine
            .input(input)
            .into_iter()
            .next()
            .map(|c| c.text)
            .unwrap_or_default();
        if top == expect {
            ok += 1;
        } else {
            misses.push((input, expect, top));
        }
    }
    let n = DAILY_VOCAB_CASES.len();
    let ratio = ok as f64 / n as f64;
    println!("\n[daily-vocab] top1={ok}/{n} ({:.1}%)", 100.0 * ratio);
    for (i, e, g) in misses.iter().take(20) {
        println!("  {i:16} expect={e} got={g}");
    }
    assert!(n >= 50, "daily vocab too small ({n})");
    assert!(
        ok * 100 >= n * 99,
        "daily vocab top-1 {:.1}% < 99%",
        ratio * 100.0
    );
}

#[test]
fn progressive_incomplete_intended_in_top5() {
    let _lock = lock_engine_tests();
    isolate_userdict("product-progressive");
    let mut engine = c_compat_engine();

    let mut checked = 0usize;
    let mut ok = 0usize;
    let mut misses = Vec::new();
    for &(input, expect) in PROGRESSIVE_TARGET_CASES {
        if input.len() < 4 {
            continue;
        }
        checked += 1;
        let tops: Vec<String> = engine
            .input(input)
            .into_iter()
            .take(5)
            .map(|c| c.text)
            .collect();
        if tops.iter().any(|t| t == expect) {
            ok += 1;
        } else {
            misses.push((input, expect, tops));
        }
    }
    let ratio = ok as f64 / checked.max(1) as f64;
    println!(
        "\n[progressive-incomplete] intended_in_top5={ok}/{checked} ({:.1}%)",
        100.0 * ratio
    );
    for (i, e, tops) in &misses {
        println!("  {i:16} expect={e} tops={tops:?}");
    }
    assert!(checked >= 15);
    assert!(
        ratio >= 0.95,
        "progressive incomplete {:.1}% < 95%",
        ratio * 100.0
    );
}

#[test]
fn progressive_prefix_completion_in_top5() {
    let _lock = lock_engine_tests();
    isolate_userdict("product-prefix");
    let mut engine = ux_engine();

    let mut ok = 0usize;
    let mut misses = Vec::new();
    let n = PROGRESSIVE_PREFIX_CASES.len();
    for &(input, expect) in PROGRESSIVE_PREFIX_CASES {
        let tops: Vec<String> = engine
            .input(input)
            .into_iter()
            .take(5)
            .map(|c| c.text)
            .collect();
        if tops.iter().any(|t| t == expect) {
            ok += 1;
        } else {
            misses.push((input, expect, tops));
        }
    }
    let ratio = ok as f64 / n.max(1) as f64;
    println!(
        "\n[progressive-prefix] intended_in_top5={ok}/{n} ({:.1}%)",
        100.0 * ratio
    );
    for (i, e, tops) in &misses {
        println!("  {i:16} expect={e} tops={tops:?}");
    }
    assert!(n >= 10);
    assert!(
        ratio >= 0.95,
        "progressive prefix {:.1}% < 95%",
        ratio * 100.0
    );
}

#[test]
fn long_sentences_best_ux_top1_at_least_75_percent() {
    let _lock = lock_engine_tests();
    isolate_userdict("product-sentences");
    let mut engine = ux_engine();

    let mut ok = 0usize;
    let mut present = 0usize;
    for &(input, expect) in LONG_SENTENCE_CASES {
        let tops: Vec<String> = engine
            .input(input)
            .into_iter()
            .take(8)
            .map(|c| c.text)
            .collect();
        if tops.iter().any(|t| t == expect) {
            present += 1;
        }
        if tops.first().map(|s| s.as_str()) == Some(expect) {
            ok += 1;
        } else {
            println!("  miss top1 {input}: expect={expect} tops={tops:?}");
        }
    }
    let n = LONG_SENTENCE_CASES.len();
    println!("\n[best-ux sentences] top1={ok}/{n} present={present}/{n}");
    assert!(n >= 6);
    assert!(present == n, "missing gold sentences: {present}/{n}");
    assert!(ok * 4 >= n * 3, "sentence top-1 too low: {ok}/{n}");
}

#[test]
fn product_bar_is_documented() {
    assert!(DAILY_VOCAB_CASES.len() >= 50);
    assert!(
        PROGRESSIVE_TARGET_CASES
            .iter()
            .filter(|(s, _)| s.len() >= 4)
            .count()
            >= 15
    );
    assert!(PROGRESSIVE_PREFIX_CASES.len() >= 10);
    assert!(LONG_SENTENCE_CASES.len() >= 6);
}
