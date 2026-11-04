//! Emoji dict lookup + optional engine merge (exact keyword on raw input).

mod common;

use common::{data_dir, isolate_userdict};
use libpinyin::EmojiDict;
use std::path::PathBuf;

fn emoji_table() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/emoji.table");
    assert!(p.exists(), "missing {}", p.display());
    p
}

#[test]
fn emoji_exact_english_keyword() {
    let dict = EmojiDict::from_file(emoji_table()).expect("emoji.table");
    assert!(dict.lookup_best("smile").is_some());
    assert!(dict.lookup_best("smi").is_none());
}

#[test]
fn emoji_exact_chinese_keyword_available_to_ui() {
    // Chinese glosses stay in the dict for UIs that want them; core merge
    // only uses the raw input keyword.
    let dict = EmojiDict::from_file(emoji_table()).expect("emoji.table");
    assert!(dict.lookup_best("猫").is_some());
}

#[test]
fn engine_appends_emoji_for_exact_input_keyword() {
    isolate_userdict("emoji-engine");
    let mut engine = libpinyin::Engine::from_data_dir(data_dir()).expect("load");
    engine.set_emoji_enabled(true);
    let want = EmojiDict::from_file(emoji_table())
        .unwrap()
        .lookup_best("smile")
        .unwrap()
        .emoji
        .clone();
    let cands = engine.input("smile");
    let texts: Vec<_> = cands.iter().map(|c| c.text.as_str()).collect();
    let page_size = 9;
    let idx = texts.iter().position(|t| *t == want.as_str());
    assert!(
        idx.is_some(),
        "expected {} in {:?}",
        want,
        &texts[..texts.len().min(12)]
    );
    let idx = idx.unwrap();
    assert!(
        idx < page_size,
        "emoji buried at index {} (page {}); expected first page: {:?}",
        idx,
        idx / page_size,
        &texts[..texts.len().min(page_size)]
    );
}
