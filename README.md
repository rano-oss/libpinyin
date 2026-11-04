# libpinyin

Rust Pinyin input method engine: segmentation, candidates, double pinyin, mixed English/emoji input, and IME session APIs (`Engine` + `Editor`).

## Install

```toml
# Cargo.toml
libpinyin = "0.1"
# or: git = "https://github.com/rano-oss/libpinyin"
```

Runtime lexicon files are **not** shipped in the crate. Point `Engine::from_data_dir` at a directory with `lexicon.fst` / `lexicon.dat` / word-bigram artifacts (and optionally `emoji.table`, `english.wordlist`). pinyinwl installs these under `/usr/share/libpinyin/data/`.

## Quick start

```rust
use libpinyin::{Engine, KeyEvent};

let engine = Engine::from_data_dir("data/converted/simplified").unwrap();
let mut editor = engine.into_editor(9);
editor.process_key(KeyEvent::Char('n'));
```

## License

GPL-3.0-or-later
