//! Integration test: compare Rust parser / candidates with system libpinyin C library.
//!
//! Uses runtime dlopen (via libloading) to load the system libpinyin shared library,
//! so no devel package or unversioned .so symlink is needed.
//!
//! Requirements (any of):
//! - System libpinyin (`libpinyin.so.15` + data under `/usr/lib64/libpinyin/data`)
//! - Or env overrides for CI / extracted RPMs:
//!   `LIBPINYIN_LIB`, `LD_LIBRARY_PATH`, `LIBPINYIN_DATA_DIR`
//!
//! ```text
//! cargo test -p libpinyin --test compare_with_c_libpinyin -- --ignored --nocapture
//! ```
//!
//! Tests are `#[ignore]`d by default; they skip cleanly when the library is absent.
//!
//! # Intentional gaps (not regressions)
//! - Rust may surface English / emoji mixed-input extras; C libpinyin does not.
//!   Candidate compares disable those extras and only score phonetic overlap.
//! - Ranking / n-best tie-breaks differ; we assert top-1 agreement and top-N set
//!   overlap, not full list identity.
//! - Incomplete single-letter buffers often yield zero C candidates; those are
//!   skipped in the candidate harness (still covered by segmentation tests).
//! - Product UX bar (daily vocab / progressive / Best UX sentences) lives in
//!   `product_quality.rs` — broad C overlap past ~95% is not a user requirement.

mod common;

use common::c_options::{self, PINYIN_OPTIONS_FULL, PINYIN_OPTIONS_STRICT};
use common::{
    apply_c_compat, isolate_userdict, load_engine, lock_engine_tests, CLibPinyin,
    CORE_CANDIDATE_INPUTS, PROGRESSIVE_INPUTS, TEST_INPUTS,
};
use libpinyin::{Parser, PINYIN_SYLLABLES};
use std::collections::HashSet;

const C_USER_DIR: &str = "/tmp/libpinyin_test_user";

/// Create Rust parser mirroring upstream full IME options (incomplete + corrections).
fn rust_parser() -> Parser {
    let cfg = c_options::from_c_options(PINYIN_OPTIONS_FULL);
    Parser::with_syllables_and_config(PINYIN_SYLLABLES, &cfg)
}

/// Extract syllable texts from Rust parser result.
fn rust_parse(parser: &Parser, input: &str) -> Vec<String> {
    let seg = parser.segment_best(input, false);
    seg.into_iter().map(|s| s.text).collect()
}

#[test]
#[ignore]
fn compare_segmentation_with_c_libpinyin() {
    let c_lib = match CLibPinyin::new(C_USER_DIR) {
        Some(lib) => lib,
        None => {
            eprintln!(
                "SKIP: Could not initialize system libpinyin \
                 (not installed or data missing at /usr/lib64/libpinyin/data)"
            );
            return;
        }
    };
    c_lib.set_options(PINYIN_OPTIONS_FULL);

    let parser = rust_parser();

    let mut total = 0;
    let mut matches = 0;
    let mut mismatches: Vec<(String, Vec<String>, Vec<String>, usize)> = Vec::new();

    for &input in TEST_INPUTS {
        let (c_parsed_len, c_syllables) = c_lib.parse(input);
        let rust_syllables = rust_parse(&parser, input);

        total += 1;

        // Normalize: C libpinyin may return syllables with tone numbers or different casing.
        let c_normalized: Vec<String> = c_syllables
            .iter()
            .map(|s| {
                s.trim_end_matches(|c: char| c.is_ascii_digit())
                    .to_lowercase()
            })
            .collect();
        let rust_normalized: Vec<String> =
            rust_syllables.iter().map(|s| s.to_lowercase()).collect();

        if c_normalized == rust_normalized {
            matches += 1;
        } else {
            mismatches.push((
                input.to_string(),
                c_normalized,
                rust_normalized,
                c_parsed_len,
            ));
        }
    }

    println!("\n=== Pinyin Parser Comparison: C libpinyin vs Rust ===");
    println!("Total inputs:  {}", total);
    println!(
        "Matching:      {} ({:.1}%)",
        matches,
        100.0 * matches as f64 / total as f64
    );
    println!("Mismatches:    {}", mismatches.len());

    if !mismatches.is_empty() {
        println!("\n--- Mismatches ---");
        for (input, c_result, rust_result, c_parsed_len) in &mismatches {
            println!(
                "  Input: {:40} C(len={}): {:?}",
                input, c_parsed_len, c_result
            );
            println!("  {:42} Rust:     {:?}", "", rust_result);
        }
    }

    // Expect high agreement on the broad corpus (tie-breaks / incomplete buffers aside).
    let match_ratio = matches as f64 / total as f64;
    assert!(
        match_ratio >= 0.95,
        "Match ratio too low: {:.1}% (expected >= 95%). \
         Rust parser diverges too much from C libpinyin.",
        match_ratio * 100.0
    );
}

/// Test that parsed length (bytes consumed) agrees between implementations.
#[test]
#[ignore]
fn compare_parsed_length() {
    let c_lib = match CLibPinyin::new(C_USER_DIR) {
        Some(lib) => lib,
        None => {
            eprintln!("SKIP: Could not initialize system libpinyin");
            return;
        }
    };
    c_lib.set_options(PINYIN_OPTIONS_FULL);

    let parser = rust_parser();
    let mut length_mismatches = Vec::new();

    for &input in TEST_INPUTS {
        let (c_parsed_len, _) = c_lib.parse(input);
        let rust_seg = parser.segment_best(input, false);
        // Rust parsed length = sum of syllable text lengths
        let rust_parsed_len: usize = rust_seg.iter().map(|s| s.text.len()).sum();

        if c_parsed_len != rust_parsed_len {
            length_mismatches.push((input, c_parsed_len, rust_parsed_len));
        }
    }

    println!("\n=== Parsed Length Comparison ===");
    println!("Total inputs: {}", TEST_INPUTS.len());
    println!("Length mismatches: {}", length_mismatches.len());

    if !length_mismatches.is_empty() {
        println!("\n--- Parsed Length Mismatches ---");
        for (input, c_len, rust_len) in &length_mismatches {
            println!("  {:40} C: {:2}  Rust: {:2}", input, c_len, rust_len);
        }
    }

    let match_ratio = 1.0 - length_mismatches.len() as f64 / TEST_INPUTS.len() as f64;
    assert!(
        match_ratio >= 0.95,
        "Parsed length match ratio too low: {:.1}% (expected >= 95%)",
        match_ratio * 100.0
    );
}

/// Every standard pinyin syllable should be recognized by both implementations.
#[test]
#[ignore]
fn compare_single_syllable_recognition() {
    let c_lib = match CLibPinyin::new(C_USER_DIR) {
        Some(lib) => lib,
        None => {
            eprintln!("SKIP: Could not initialize system libpinyin");
            return;
        }
    };
    c_lib.set_options(PINYIN_OPTIONS_FULL);

    let parser = rust_parser();

    let mut c_unrecognized = Vec::new();
    let mut rust_unrecognized = Vec::new();
    let mut both_recognized = 0;

    for &syllable in PINYIN_SYLLABLES {
        let (c_parsed_len, c_syls) = c_lib.parse(syllable);
        let rust_syls = rust_parse(&parser, syllable);

        let c_ok = c_parsed_len == syllable.len() && c_syls.len() == 1;
        let rust_ok = rust_syls.len() == 1 && rust_syls[0] == syllable;

        if c_ok && rust_ok {
            both_recognized += 1;
        } else if !c_ok {
            c_unrecognized.push(syllable);
        } else if !rust_ok {
            rust_unrecognized.push(syllable);
        }
    }

    println!("\n=== Single Syllable Recognition ===");
    println!("Total syllables: {}", PINYIN_SYLLABLES.len());
    println!("Both recognize:  {}", both_recognized);
    if !c_unrecognized.is_empty() {
        println!(
            "C unrecognized ({}):\n  {:?}",
            c_unrecognized.len(),
            &c_unrecognized[..c_unrecognized.len().min(20)]
        );
    }
    if !rust_unrecognized.is_empty() {
        println!(
            "Rust unrecognized ({}):\n  {:?}",
            rust_unrecognized.len(),
            &rust_unrecognized[..rust_unrecognized.len().min(20)]
        );
    }

    assert!(
        rust_unrecognized.len() <= 5,
        "Rust fails to recognize too many standard syllables: {:?}",
        rust_unrecognized
    );
}

/// Broad candidate compare: core smoke + full TEST_INPUTS + progressive buffers.
#[test]
#[ignore]
fn compare_top_candidates_with_c_libpinyin() {
    let _lock = lock_engine_tests();
    isolate_userdict("compare");
    let Some(c_lib) = CLibPinyin::new(C_USER_DIR) else {
        eprintln!("SKIP: Could not initialize system libpinyin");
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
    for (name, options, fuzzy, min_top1, min_overlap) in [
        (
            "cand-strict-broad",
            PINYIN_OPTIONS_STRICT,
            false,
            0.95_f64,
            0.90_f64,
        ),
        ("cand-full-broad", PINYIN_OPTIONS_FULL, true, 0.95, 0.90),
    ] {
        c_lib.set_options(options);
        engine.set_allow_fuzzy(fuzzy);
        let mut compared = 0usize;
        let mut top1_matches = 0usize;
        let mut overlap_sum = 0.0f64;
        let mut skipped_empty_c = 0usize;
        let mut top1_mismatches: Vec<(String, String, String)> = Vec::new();

        for &input in &inputs {
            let c_top = c_lib.candidates(input, TOP_N);
            if c_top.is_empty() {
                skipped_empty_c += 1;
                continue;
            }
            let rust_top: Vec<String> = engine
                .input(input)
                .into_iter()
                .take(TOP_N)
                .map(|c| c.text)
                .collect();
            if rust_top.is_empty() {
                top1_mismatches.push((input.to_string(), c_top[0].clone(), String::new()));
                compared += 1;
                continue;
            }

            compared += 1;
            if rust_top[0] == c_top[0] {
                top1_matches += 1;
            } else {
                top1_mismatches.push((input.to_string(), c_top[0].clone(), rust_top[0].clone()));
            }

            let c_set: HashSet<&str> = c_top.iter().map(|s| s.as_str()).collect();
            let overlap = rust_top
                .iter()
                .filter(|t| c_set.contains(t.as_str()))
                .count();
            overlap_sum += overlap as f64 / TOP_N as f64;
        }

        let top1_ratio = top1_matches as f64 / compared.max(1) as f64;
        let avg_overlap = overlap_sum / compared.max(1) as f64;

        println!("\n=== [{name}] Top-{TOP_N} (phonetic, C-compat engine) ===");
        println!("Inputs compared:     {compared}");
        println!("Skipped (empty C):   {skipped_empty_c}");
        println!(
            "Top-1 matches:       {top1_matches} ({:.1}%)",
            100.0 * top1_ratio
        );
        println!("Avg top-N overlap:   {:.1}%", 100.0 * avg_overlap);
        for (input, c, rust) in top1_mismatches.iter().take(25) {
            println!("  {input:16} C: {c:8} Rust: {rust}");
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
        "broad candidate compare gaps:\n  {}",
        failures.join("\n  ")
    );
}
