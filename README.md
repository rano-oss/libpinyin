# libpinyin

Rust Pinyin input method engine: segmentation, candidates, double pinyin, mixed English/emoji input, and IME session APIs.

Standalone split from [libchinese](https://github.com/rano-oss/libchinese). Single crate — lexicon / ranking / IME session live under `libpinyin::core`, with a pinyin facade on top.

## Layout

- `src/` — public crate (`Engine`, `Parser`, `ImeEngine`, …)
- `src/core/` — lexicon, n-gram, user dict, session primitives
- `data/converted/` — runtime lexicon artifacts (not published to crates.io)

## Quick start

```rust
use libpinyin::{Engine, KeyEvent};

let engine = Engine::from_data_dir("data/converted/simplified").unwrap();
let mut ime = engine.into_ime(9);
ime.process_key(KeyEvent::Char('n'));
```

## License

GPL-3.0-or-later
