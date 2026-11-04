//! Long-sentence composition and ranking config knobs.
//!
//! Note: userdict.redb is exclusive — keep these in one test body.

mod common;

use common::{apply_best_ux, apply_c_compat, isolate_userdict, load_engine};
use libpinyin::CandidateRankMode;

#[test]
fn composed_sentences_and_rank_options() {
    isolate_userdict("composed");
    let mut engine = load_engine();
    engine.set_allow_fuzzy(true);
    apply_best_ux(&mut engine);

    // woshinuoweiren → 我是挪威人 (C pinyin_get_sentence parity)
    let tops: Vec<_> = engine
        .input("woshinuoweiren")
        .into_iter()
        .take(5)
        .map(|c| c.text)
        .collect();
    assert_eq!(
        tops.first().map(String::as_str),
        Some("我是挪威人"),
        "expected composed sentence at top-1, got {tops:?}"
    );

    for (input, expect) in [
        ("woshizhongguoren", "我是中国人"),
        ("woaini", "我爱你"),
        ("womenyiqiqubeijing", "我们一起去北京"),
    ] {
        let texts: Vec<_> = engine
            .input(input)
            .into_iter()
            .take(8)
            .map(|c| c.text)
            .collect();
        assert!(
            texts.iter().any(|t| t == expect),
            "{input}: missing {expect}, got {texts:?}"
        );
    }

    // Option off: suppress composed sentence
    engine.set_emit_composed_sentences(false);
    let tops: Vec<_> = engine
        .input("woshinuoweiren")
        .into_iter()
        .take(8)
        .map(|c| c.text)
        .collect();
    assert!(
        !tops.iter().any(|t| t == "我是挪威人"),
        "composed sentence should be suppressed when option is off, got {tops:?}"
    );
    engine.set_emit_composed_sentences(true);

    // Phrase-length rank prefers Chinese char length (C SORT_BY_PHRASE_LENGTH).
    engine.set_allow_fuzzy(false);
    engine.set_emit_composed_sentences(false);

    engine.set_candidate_rank_mode(CandidateRankMode::MatchedSpanThenScore);
    let span_tops: Vec<_> = engine
        .input("beijing")
        .into_iter()
        .take(8)
        .map(|c| c.text)
        .collect();

    engine.set_candidate_rank_mode(CandidateRankMode::PhraseLengthThenScore);
    let phrase_tops: Vec<_> = engine
        .input("beijing")
        .into_iter()
        .take(8)
        .map(|c| c.text)
        .collect();

    assert_eq!(span_tops.first().map(String::as_str), Some("北京"));
    assert_eq!(phrase_tops.first().map(String::as_str), Some("北京"));
    // Phrase-length mode should surface 1-char unigrams (C-style padding).
    assert!(
        phrase_tops.iter().any(|t| t.chars().count() == 1),
        "PhraseLengthThenScore missing unigrams in top-8: {phrase_tops:?}"
    );
    assert_eq!(
        engine.candidate_rank_mode(),
        CandidateRankMode::PhraseLengthThenScore
    );

    // C-compat knobs: no sentence in bar; still available via best_sentence().
    apply_c_compat(&mut engine);
    let compat_tops: Vec<_> = engine
        .input("woshinuoweiren")
        .into_iter()
        .take(5)
        .map(|c| c.text)
        .collect();
    assert!(
        !compat_tops.iter().any(|t| t == "我是挪威人"),
        "compat candidate bar should not fold sentence; got {compat_tops:?}"
    );
    assert_eq!(
        engine.best_sentence("woshinuoweiren").as_deref(),
        Some("我是挪威人"),
        "compat knobs still expose sentence via best_sentence()"
    );

    // UX knobs: sentences in the bar + span ranking.
    engine.set_emit_composed_sentences(true);
    engine.set_candidate_rank_mode(CandidateRankMode::MatchedSpanThenScore);
    assert!(engine.emit_composed_sentences());
    assert_eq!(
        engine.candidate_rank_mode(),
        CandidateRankMode::MatchedSpanThenScore
    );
    let ux_top = engine
        .input("woshinuoweiren")
        .into_iter()
        .next()
        .map(|c| c.text);
    assert_eq!(ux_top.as_deref(), Some("我是挪威人"));
}
