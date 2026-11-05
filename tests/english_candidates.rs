//! English mixed-input via public [`Engine`] API.

mod common;

use common::{data_dir, isolate_userdict, lock_engine_tests};

#[test]
fn loads_english_wordlist_from_aux_dirs() {
    let _lock = lock_engine_tests();
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
