# libpinyin

[![Crates.io](https://img.shields.io/crates/v/libpinyin.svg)](https://crates.io/crates/libpinyin)
[![Docs.rs](https://docs.rs/libpinyin/badge.svg)](https://docs.rs/libpinyin)
[![CI](https://github.com/rano-oss/libpinyin/actions/workflows/ci.yml/badge.svg)](https://github.com/rano-oss/libpinyin/actions/workflows/ci.yml)

Rust Pinyin input method engine: segmentation, candidates, double pinyin, mixed English/emoji input, and IME session APIs (`Engine` + `Editor`).

## Install

```toml
# Cargo.toml
libpinyin = "0.4"
# or: git = "https://github.com/rano-oss/libpinyin"
```

## Quick start

```rust
use libpinyin::{DataVariant, Engine, KeyEvent};

let engine = Engine::from_default_paths(DataVariant::Simplified).unwrap();
let mut editor = engine.into_editor(9);
editor.process_key(KeyEvent::Char('n'));
// Read editor.context for preedit / candidates / commit_text
```

Runtime lexicon files are **not** shipped on crates.io. Resolve them with:

- `Engine::from_default_paths(DataVariant::…)` — search order in [`path`](https://docs.rs/libpinyin/latest/libpinyin/path/index.html)
- or `Engine::from_data_dir("/path/to/dir")`

Expected artifacts: `lexicon.fst` / `lexicon.dat`, `word_bigram.dat` / `word_bigram_words.fst`, optional `emoji.table` / `english.wordlist` / `addon/*/`.

| Env | Purpose |
|---|---|
| `LIBPINYIN_DATA_DIR` | Exact data directory (skips search) |
| `LIBPINYIN_USERDICT` | Path to `userdict.redb` |
| `LIBPINYIN_DATADIR` | Build-time default system root (compile env) |

## API sketch

| Item | Role |
|---|---|
| `Engine` | Lexicon load + `input()` candidates |
| `Editor` | Key session; options via `EditorOptions` |
| `Context` | Preedit / candidates / commit snapshot after `process_key` |
| `LanguageMode` | 中 / 英 |
| `CharacterForm` | 半 / 全 |
| `path` | Data / userdict discovery |
| `editor::{preferred_chinese_punct, to_fullwidth}` | Host helpers for bypass paths |

Also public: `double_pinyin`, `userdict`, and `engine::{Lexicon, WordBigram}` — see [docs.rs](https://docs.rs/libpinyin).

## Building data files

Place upstream libpinyin tables under `data/` (and `data/zhuyin/`), then:

```bash
./scripts/build_data.sh
# or:
cargo run -p convert_table --release
cargo run -p gen_word_bigrams --release -- data/interpolation2.text data/converted/simplified
```

Output goes to `data/converted/`. Convert tools are workspace members (`publish = false`).

## License

GPL-3.0-or-later
