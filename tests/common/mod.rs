//! Shared helpers for libpinyin integration tests.

#![allow(dead_code, unused_imports)]

pub mod c_libpinyin;
pub mod c_options;
pub mod corpus;

pub use c_libpinyin::CLibPinyin;
pub use corpus::{
    CORE_CANDIDATE_INPUTS, DAILY_VOCAB_CASES, LONG_SENTENCE_CASES, PROGRESSIVE_INPUTS,
    PROGRESSIVE_PREFIX_CASES, PROGRESSIVE_TARGET_CASES, SMOKE_CANDIDATE_INPUTS, TEST_INPUTS,
};

use libpinyin::{CandidateRankMode, Editor, KeyEvent};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

/// Simplified lexicon data directory (repo checkout or system install).
pub fn data_dir() -> PathBuf {
    for p in [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/converted/simplified"),
        PathBuf::from("/usr/share/libpinyin/data/simplified"),
    ] {
        if p.join("lexicon.fst").exists() {
            return p;
        }
    }
    panic!("no rust pinyin data dir");
}

static ENGINE_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Serialize `Engine::from_data_dir` tests: userdict path is process-global
/// and redb refuses a second open of the same path.
pub fn lock_engine_tests() -> MutexGuard<'static, ()> {
    ENGINE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Point user dictionary at an isolated temp file (avoids cross-test pollution).
/// Call [`lock_engine_tests`] first when multiple tests in the same binary open engines.
pub fn isolate_userdict(prefix: &str) {
    let ud = std::env::temp_dir().join(format!(
        "libpinyin-{prefix}-userdict-{}-{}.redb",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_file(&ud);
    std::env::set_var("LIBPINYIN_USERDICT", &ud);
}

/// Load engine with english/emoji off (product + C suites).
pub fn load_engine() -> libpinyin::Engine {
    let mut engine = libpinyin::Engine::from_data_dir(data_dir()).expect("engine");
    engine.set_english_enabled(false);
    engine.set_emoji_enabled(false);
    engine
}

/// Force C `guess_candidates` shape (parity / compare / perf).
pub fn apply_c_compat(engine: &mut libpinyin::Engine) {
    engine.configure(|c| {
        c.emit_composed_sentences = false;
        c.candidate_rank_mode = CandidateRankMode::PhraseLengthThenScore;
    });
}

/// Force Best UX candidate bar (sentence fold-in + span rank).
pub fn apply_best_ux(engine: &mut libpinyin::Engine) {
    engine.configure(|c| {
        c.emit_composed_sentences = true;
        c.candidate_rank_mode = CandidateRankMode::MatchedSpanThenScore;
    });
}

pub fn c_compat_engine() -> libpinyin::Engine {
    let mut engine = load_engine();
    engine.configure(|c| c.allow_fuzzy = true);
    apply_c_compat(&mut engine);
    engine
}

pub fn ux_engine() -> libpinyin::Engine {
    let mut engine = load_engine();
    engine.configure(|c| c.allow_fuzzy = true);
    apply_best_ux(&mut engine);
    engine
}

pub fn type_ascii(ime: &mut Editor, s: &str) {
    for ch in s.chars() {
        let _ = ime.process_key(KeyEvent::Char(ch));
    }
}
