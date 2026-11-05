//! Emoji mixed-input via public [`Engine`] API.

mod common;

use common::{data_dir, isolate_userdict, lock_engine_tests};

#[test]
fn engine_appends_emoji_for_exact_input_keyword() {
    let _lock = lock_engine_tests();
    isolate_userdict("emoji-engine");
    let mut engine = libpinyin::Engine::from_data_dir(data_dir()).expect("load");

    engine.set_emoji_enabled(false);
    let without: Vec<_> = engine.input("smile").into_iter().map(|c| c.text).collect();

    engine.set_emoji_enabled(true);
    let with = engine.input("smile");
    let texts: Vec<_> = with.iter().map(|c| c.text.as_str()).collect();
    let page_size = 9;
    let emoji = with.iter().find(|c| !without.iter().any(|w| w == &c.text));
    assert!(
        emoji.is_some(),
        "expected an emoji extra for 'smile', got {:?}",
        &texts[..texts.len().min(12)]
    );
    let idx = texts
        .iter()
        .position(|t| *t == emoji.unwrap().text.as_str())
        .unwrap();
    assert!(
        idx < page_size,
        "emoji buried at index {} (page {}); expected first page: {:?}",
        idx,
        idx / page_size,
        &texts[..texts.len().min(page_size)]
    );
}

#[test]
fn engine_bridges_pinyin_to_chinese_emoji_keyword() {
    let _lock = lock_engine_tests();
    isolate_userdict("emoji-xin");
    let mut engine = libpinyin::Engine::from_data_dir(data_dir()).expect("load");

    engine.set_emoji_enabled(false);
    let without: Vec<_> = engine.input("xin").into_iter().map(|c| c.text).collect();

    engine.set_emoji_enabled(true);
    let with = engine.input("xin");
    let texts: Vec<_> = with.iter().map(|c| c.text.as_str()).collect();
    let emoji = with.iter().find(|c| !without.iter().any(|w| w == &c.text));
    assert!(
        emoji.is_some(),
        "expected emoji extra for xin (via 心), got {:?}",
        &texts[..texts.len().min(12)]
    );
    let idx = texts
        .iter()
        .position(|t| *t == emoji.unwrap().text.as_str())
        .unwrap();
    assert!(
        idx < 9,
        "emoji buried at {idx}; first page: {:?}",
        &texts[..texts.len().min(9)]
    );
}
