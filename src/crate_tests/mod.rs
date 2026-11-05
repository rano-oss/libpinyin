//! Unit tests that need `pub(crate)` modules (parser, config, …).

#![allow(dead_code, unused_imports, clippy::needless_collect)]

mod c_options;
mod common;

mod compare_with_c_libpinyin;
mod double_pinyin_integration;
mod enhancement_features;
mod parity_with_c_libpinyin;
mod perf_vs_c_libpinyin;
mod ported_parser_vectors;
