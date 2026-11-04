//! 以词定字: `[` / `]` pick a character from the highlighted phrase.

mod common;

use common::{data_dir, isolate_userdict, type_ascii};
use libpinyin::{Editor, KeyEvent, KeyResult};

#[test]
fn choose_char_from_phrase_behaviour() {
    isolate_userdict("choose-char");
    let mut engine = libpinyin::Engine::from_data_dir(data_dir()).expect("load");
    engine.set_english_enabled(false);
    engine.set_emoji_enabled(false);
    {
        let cfg = &mut engine.model.config;
        cfg.choose_char_from_phrase = true;
        cfg.auto_suggestion = false;
    }

    let mut ime = Editor::with_page_size(engine, 9);
    type_ascii(&mut ime, "zhongguo");
    assert_eq!(
        ime.context.candidates.first().map(|s| s.as_str()),
        Some("中国"),
        "expected 中国 top-1, got {:?}",
        ime.context.candidates
    );

    assert_eq!(
        ime.process_key(KeyEvent::ChooseCharFromPhrase(0)),
        KeyResult::Handled
    );
    assert_eq!(ime.context.commit_text, "中");
    assert!(ime.context.preedit_text.is_empty());
    assert!(ime.context.candidates.is_empty());

    ime.reset();
    type_ascii(&mut ime, "zhongguo");
    assert_eq!(
        ime.process_key(KeyEvent::ChooseCharFromPhrase(-1)),
        KeyResult::Handled
    );
    assert_eq!(ime.context.commit_text, "国");

    ime.engine.model.config.choose_char_from_phrase = false;
    ime.reset();
    type_ascii(&mut ime, "zhongguo");
    assert_eq!(
        ime.process_key(KeyEvent::ChooseCharFromPhrase(0)),
        KeyResult::NotHandled
    );
    assert!(ime.context.commit_text.is_empty());
    assert!(!ime.context.candidates.is_empty());
}
