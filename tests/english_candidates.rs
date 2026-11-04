//! English mixed-input: dict lookup + engine appends extras after phonetic results.

mod common;

use common::{data_dir, isolate_userdict};
use libpinyin::EnglishDict;
use std::path::PathBuf;

#[test]
fn loads_english_wordlist_from_aux_dirs() {
    isolate_userdict("english-engine");
    let mut engine = libpinyin::Engine::from_data_dir(data_dir()).expect("load engine");
    engine.set_english_enabled(true);

    let cands = engine.input("hello");
    let texts: Vec<_> = cands.iter().map(|c| c.text.as_str()).collect();
    let page_size = 9;
    let idx = texts.iter().position(|t| t.eq_ignore_ascii_case("hello"));
    assert!(
        idx.is_some(),
        "expected english 'hello' among candidates, got: {:?}",
        &texts[..texts.len().min(12)]
    );
    let idx = idx.unwrap();
    assert!(
        idx < page_size,
        "english 'hello' buried at index {} (page {}); expected first page: {:?}",
        idx,
        idx / page_size,
        &texts[..texts.len().min(page_size)]
    );
}

#[test]
fn english_dict_prefix_from_vendored_wordlist() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/english.wordlist");
    let dict = EnglishDict::from_file(&path).expect("wordlist");
    assert!(!dict.is_empty());
    let hits = dict.lookup_prefix("hel");
    assert!(hits.iter().any(|(w, _)| *w == "hello"));
}
