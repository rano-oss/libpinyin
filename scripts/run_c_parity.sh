#!/usr/bin/env bash
# Run ignored C↔Rust fidelity suites when system (or extracted) libpinyin is present.
# Skips cleanly if the library cannot be loaded.
# Product UX gates (no C required): cargo test -p libpinyin --test product_quality
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Optional: Fedora RPM extract under /tmp/libpinyin-sys (see prior local setup).
if [[ -z "${LIBPINYIN_LIB:-}" && -f /tmp/libpinyin-sys/usr/lib64/libpinyin.so.15 ]]; then
  export LIBPINYIN_LIB=/tmp/libpinyin-sys/usr/lib64/libpinyin.so.15
  export LD_LIBRARY_PATH="/tmp/libpinyin-sys/usr/lib64${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  export LIBPINYIN_DATA_DIR=/tmp/libpinyin-sys/usr/lib64/libpinyin/data
fi

if [[ -z "${LIBPINYIN_LIB:-}" ]] && ! ldconfig -p 2>/dev/null | grep -q 'libpinyin\.so\.15'; then
  if [[ ! -e /usr/lib64/libpinyin.so.15 && ! -e /usr/lib/libpinyin.so.15 ]]; then
    echo "SKIP: libpinyin.so.15 not found (install package or set LIBPINYIN_LIB)"
    exit 0
  fi
fi

# C suites live under src/crate_tests (unit tests) so they can use pub(crate) parser/config.
exec cargo test -p libpinyin --lib \
  compare_with_c_libpinyin parity_with_c_libpinyin \
  -- --ignored --nocapture "$@"
