//! Forget-word (Ctrl+7): remove learned freq + mask from candidates.

mod common;

use common::{data_dir, isolate_userdict, type_ascii};
use libpinyin::{Editor, KeyResult};

#[test]
fn forget_word_hides_candidate() {
    isolate_userdict("forget");
    let mut engine = libpinyin::Engine::from_data_dir(data_dir()).expect("load");
    engine.set_english_enabled(false);
    engine.set_emoji_enabled(false);
    {
        let cfg = &mut engine.model.config;
        cfg.auto_suggestion = false;
        cfg.learning_enabled = true;
    }

    // Boost a secondary candidate so we can forget something visible.
    engine.commit("背景");

    let mut ime = Editor::with_page_size(engine, 9);
    type_ascii(&mut ime, "beijing");
    let before = ime.context.candidates.clone();
    assert!(
        before.iter().any(|t| t == "背景"),
        "expected 背景 among candidates after learn, got {before:?}"
    );

    while ime.context.candidates.get(ime.context.candidate_cursor) != Some(&"背景".to_string()) {
        assert_eq!(
            ime.process_key(libpinyin::KeyEvent::Down),
            KeyResult::Handled
        );
    }

    assert_eq!(
        ime.process_key(libpinyin::KeyEvent::ForgetWord),
        KeyResult::Handled
    );
    let after = ime.context.candidates.clone();
    assert!(
        !after.iter().any(|t| t == "背景"),
        "背景 should be forgotten/masked, got {after:?}"
    );
    assert!(ime.engine.model.userdict.is_masked("背景"));
    assert_eq!(ime.engine.model.userdict.frequency("背景"), 0);

    ime.reset();
    type_ascii(&mut ime, "beijing");
    assert!(
        !ime.context.candidates.iter().any(|t| t == "背景"),
        "mask should persist across sessions, got {:?}",
        ime.context.candidates
    );

    ime.engine.unmask_phrase("背景");
    assert!(!ime.engine.is_phrase_hidden("背景"));
    ime.engine.mask_phrase("背景");
    assert!(ime.engine.is_phrase_hidden("背景"));
    ime.engine.commit("背景");
    assert!(!ime.engine.model.userdict.is_masked("背景"));
    assert!(ime.engine.model.userdict.frequency("背景") > 0);

    ime.engine.forget_phrase("背景");
    assert!(ime.engine.model.userdict.is_masked("背景"));
    assert_eq!(ime.engine.model.userdict.frequency("背景"), 0);
}

#[test]
fn forget_mask_persists_across_userdict_reopen() {
    let ud = std::env::temp_dir().join(format!(
        "libpinyin-forget-reopen-{}-{}.redb",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_file(&ud);
    {
        let mut udict = libpinyin::UserDict::new(&ud).expect("open");
        udict.learn("背景");
        assert!(udict.frequency("背景") > 0);
        udict.forget_phrase("背景").expect("forget");
        assert!(udict.is_masked("背景"));
        assert_eq!(udict.frequency("背景"), 0);
    }
    let reopened = libpinyin::UserDict::new(&ud).expect("reopen");
    assert!(reopened.is_masked("背景"));
    assert_eq!(reopened.frequency("背景"), 0);
}
