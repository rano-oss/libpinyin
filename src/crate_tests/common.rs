//! Shared helpers for crate-internal tests (mirrors `tests/common` with `crate::`).

#[path = "../../tests/common/c_libpinyin.rs"]
mod c_libpinyin;
#[path = "../../tests/common/corpus.rs"]
mod corpus;

pub use c_libpinyin::CLibPinyin;
pub use corpus::{
    CORE_CANDIDATE_INPUTS, DAILY_VOCAB_CASES, LONG_SENTENCE_CASES, PROGRESSIVE_INPUTS,
    PROGRESSIVE_PREFIX_CASES, PROGRESSIVE_TARGET_CASES, SMOKE_CANDIDATE_INPUTS, TEST_INPUTS,
};

use crate::{CandidateRankMode, Editor, Engine, KeyEvent};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

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

pub fn lock_engine_tests() -> MutexGuard<'static, ()> {
    ENGINE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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

pub fn load_engine() -> Engine {
    let mut engine = Engine::from_data_dir(data_dir()).expect("engine");
    engine.set_english_enabled(false);
    engine.set_emoji_enabled(false);
    engine
}

pub fn apply_c_compat(engine: &mut Engine) {
    engine.configure(|c| {
        c.emit_composed_sentences = false;
        c.candidate_rank_mode = CandidateRankMode::PhraseLengthThenScore;
    });
}

pub fn apply_best_ux(engine: &mut Engine) {
    engine.configure(|c| {
        c.emit_composed_sentences = true;
        c.candidate_rank_mode = CandidateRankMode::MatchedSpanThenScore;
    });
}

pub fn c_compat_engine() -> Engine {
    let mut engine = load_engine();
    engine.configure(|c| c.allow_fuzzy = true);
    apply_c_compat(&mut engine);
    engine
}

pub fn ux_engine() -> Engine {
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
