// Ported ngram tests (from upstream tests/storage/test_ngram.cpp)
//
// These tests focus on SingleGram behaviors exercised by the upstream C++ test:
// - setting total frequency
// - inserting and updating token frequencies (insert_freq / set_freq semantics)
// - retrieving token frequencies
// - range search that returns normalized frequencies (freq / total_freq)
//
// Note: upstream test also touches persistence via Bigram attach/store. That
// persistence layer is out of scope for this in-memory parity test; we port
// the core SingleGram behaviors that are required by lookup/scoring logic.

// SingleGram is test-only, so we include it as a module here
#[path = "support/single_gram.rs"]
mod single_gram;
use single_gram::SingleGram;

#[test]
fn ported_singlegram_basic_inserts_and_get() {
    let mut sg = SingleGram::new();
    sg.set_total_freq(16);

    let tokens: [u32; 6] = [2, 6, 4, 3, 1, 3];
    let freqs: [u32; 6] = [1, 2, 4, 8, 16, 32];

    for i in 0..tokens.len() {
        let t = tokens[i];
        let f = freqs[i];
        if sg.get_freq(t).is_some() {
            // token exists -> update frequency (upstream uses set_freq in this case)
            let ok = sg.set_freq(t, f);
            assert!(ok, "expected token {} to exist for set_freq", t);
        } else {
            // token absent -> insert
            let inserted = sg.insert_freq(t, f);
            assert!(inserted, "expected insert to succeed for token {}", t);
        }
    }

    // After the sequence, token 3 should have been updated to 32 (last occurrence).
    assert_eq!(
        sg.get_freq(3),
        Some(32),
        "token 3 should have frequency 32 after updates"
    );

    // Check total freq remains as set (upstream test set total before inserts).
    assert_eq!(
        sg.get_total_freq(),
        16,
        "total_freq should equal the value set earlier (16)"
    );

    // Range search: [0, 8) should include tokens 1,2,3,4,6 (all our tokens are < 8).
    // Validate that token 3 appears with normalized frequency = 32 / 16 = 2.0
    let results = sg.search_range(0, 8);
    let mut found_three = false;
    for (tok, norm) in results.iter() {
        if *tok == 3 {
            found_three = true;
            assert!(
                (norm - 2.0f32).abs() < 1e-6,
                "normalized freq for token 3 expected 2.0, got {}",
                norm
            );
        }
    }
    assert!(
        found_three,
        "expected token 3 to appear in range search results"
    );
}

#[test]
fn ported_singlegram_retrieve_and_normalization() {
    // Another small sanity test matching upstream arithmetic expectations.
    let mut sg = SingleGram::new();
    sg.insert_freq(10, 10);
    sg.insert_freq(20, 30);
    sg.set_total_freq(40);

    let all = sg.retrieve_all();
    // Expect two items in ascending token order.
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].0, 10);
    assert_eq!(all[1].0, 20);

    // Normalized frequencies: 10/40 = 0.25, 30/40 = 0.75
    let norm0 = all[0].2;
    let norm1 = all[1].2;
    assert!((norm0 - 0.25f32).abs() < 1e-6);
    assert!((norm1 - 0.75f32).abs() < 1e-6);
}
