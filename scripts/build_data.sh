#!/usr/bin/env bash
# Build converted lexicon / word-bigram artifacts from upstream table inputs.
# Expects raw tables under data/ and data/zhuyin/ (see tools/convert_table).
# Writes data/converted/{simplified,traditional,...}.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ ! -f data/gb_char.table ]]; then
  echo "Missing data/gb_char.table (and related upstream tables). Place libpinyin table sources under data/ first." >&2
  exit 1
fi

cargo run -p convert_table --release
cargo run -p gen_word_bigrams --release -- data/interpolation2.text data/converted/simplified
if [[ -f data/zhuyin/interpolation2.text ]]; then
  cargo run -p gen_word_bigrams --release -- data/zhuyin/interpolation2.text data/converted/traditional
fi

echo "Done. Artifacts under data/converted/"
