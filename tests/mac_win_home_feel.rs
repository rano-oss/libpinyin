//! Mac/Windows home-feel: Chinese punct, v/u helpers, tone Tab, pin phrase.

mod common;

use common::{data_dir, isolate_userdict, type_ascii};
use libpinyin::{ImeEngine, KeyEvent, KeyResult};

#[test]
fn chinese_punct_and_helpers_and_tone_and_pin() {
    isolate_userdict("home-feel");
    let mut engine = libpinyin::Engine::from_data_dir(data_dir()).expect("load");
    engine.set_english_enabled(false);
    engine.set_emoji_enabled(false);
    {
        let cfg = engine.config_mut();
        cfg.auto_suggestion = false;
        cfg.chinese_punctuation = true;
        cfg.v_mode_enabled = true;
        cfg.u_mode_enabled = true;
        cfg.inline_prediction = false;
        cfg.learning_enabled = true;
    }

    let mut ime = ImeEngine::with_page_size(engine.into_inner(), 9);

    // Idle Chinese punctuation
    assert_eq!(ime.process_key(KeyEvent::Char('.')), KeyResult::Handled);
    assert_eq!(ime.context().commit_text, "。");

    // v-mode numerals
    ime.reset();
    type_ascii(&mut ime, "v123");
    assert!(
        ime.context().candidates.iter().any(|t| t == "一二三"),
        "v123 helpers: {:?}",
        ime.context().candidates
    );

    // u-mode symbol
    ime.reset();
    type_ascii(&mut ime, "uheart");
    assert!(
        ime.context().candidates.iter().any(|t| t == "♥"),
        "uheart: {:?}",
        ime.context().candidates
    );

    // Tone filter: ma → 妈/麻/马/骂; Tab should narrow
    ime.reset();
    type_ascii(&mut ime, "ma");
    let before = ime.context().candidates.clone();
    assert!(!before.is_empty());
    assert_eq!(ime.process_key(KeyEvent::Tab), KeyResult::Handled); // 1声
    let t1 = ime.context().candidates.clone();
    assert!(!t1.is_empty());
    // After a few Tabs we return to all
    let _ = ime.process_key(KeyEvent::Tab);
    let _ = ime.process_key(KeyEvent::Tab);
    let _ = ime.process_key(KeyEvent::Tab);
    let _ = ime.process_key(KeyEvent::Tab); // back to all
    assert_eq!(
        ime.context().candidates.len(),
        before.len(),
        "tone filter should restore full list"
    );

    // Pin phrase
    ime.reset();
    type_ascii(&mut ime, "beijing");
    let top = ime.context().candidates.first().cloned().expect("cands");
    assert_eq!(ime.process_key(KeyEvent::PinPhrase), KeyResult::Handled);
    assert_eq!(ime.backend().userdict().frequency(&top), 500);

    // Commit default + Chinese punct
    ime.reset();
    type_ascii(&mut ime, "nihao");
    assert_eq!(ime.process_key(KeyEvent::Char('.')), KeyResult::Handled);
    assert!(
        ime.context().commit_text.ends_with('。'),
        "got {:?}",
        ime.context().commit_text
    );
    assert!(!ime.context().commit_text.is_empty());

    // Inline prediction stays enabled without crashing across commit → compose.
    {
        let cfg = ime.backend_mut().config_mut();
        cfg.inline_prediction = true;
        cfg.auto_suggestion = false;
    }
    ime.reset();
    type_ascii(&mut ime, "nihao");
    assert_eq!(ime.process_key(KeyEvent::Space), KeyResult::Handled);
    assert!(!ime.context().commit_text.is_empty());
    type_ascii(&mut ime, "ma");
    assert!(
        !ime.context().candidates.is_empty(),
        "inline_prediction path should still yield candidates"
    );
}
