//! Mixed-input extras must appear on the first candidate page (page size 9).
//!
//! One test body so we only open the shared userdict.redb once (redb is exclusive).

mod common;

use libpinyin::{Editor, KeyEvent};
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/converted/simplified"),
        PathBuf::from("/usr/share/libpinyin/data/simplified"),
    ];
    for p in candidates {
        if p.join("lexicon.fst").exists() {
            return p;
        }
    }
    panic!("no pinyin data dir found");
}

fn type_ascii(ime: &mut Editor, s: &str) {
    for ch in s.chars() {
        let _ = ime.process_key(KeyEvent::Char(ch));
    }
}

#[test]
fn english_and_emoji_on_first_page_via_editor() {
    common::isolate_userdict("mixed-input");
    let mut engine = libpinyin::Engine::from_data_dir(data_dir()).expect("load");
    engine.set_english_enabled(true);
    engine.set_emoji_enabled(true);
    let page_size = 9;

    let mut ime = Editor::with_page_size(engine, page_size);
    type_ascii(&mut ime, "hello");
    let page = &ime.context.candidates;
    assert!(
        page.iter().any(|t| t.eq_ignore_ascii_case("hello")),
        "expected 'hello' on first page, got {:?}",
        page
    );

    ime.reset();
    type_ascii(&mut ime, "smile");
    let page = &ime.context.candidates;
    assert!(
        page.iter().any(|t| !t.is_ascii()),
        "expected a non-ASCII emoji candidate on first page for smile, got {:?}",
        page
    );
}
