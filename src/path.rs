//! Filesystem paths for lexicon data and the user dictionary.
//!
//! Search order for system data (first hit with `lexicon.fst` wins):
//! 1. `LIBPINYIN_DATA_DIR` (exact dir, no charset suffix)
//! 2. `$HOME/.local/share/libpinyin/data/{simplified|traditional}`
//! 3. `/usr/share/libpinyin/data/{simplified|traditional}`
//! 4. `/usr/share/libpinyin/data` (flat layout)
//! 5. Optional relative checkout paths (`data/converted/…`) when present

use std::env;
use std::path::{Path, PathBuf};

/// Compiled-in system data root (override with `LIBPINYIN_DATADIR` at build time).
const SYS_PATH: Option<&str> = option_env!("LIBPINYIN_DATADIR");

#[cfg(target_family = "windows")]
const DEFAULT_SYS_PATH: &str = "C:\\ProgramData\\libpinyin\\data";
#[cfg(target_family = "unix")]
const DEFAULT_SYS_PATH: &str = "/usr/share/libpinyin/data";

/// Which installed lexicon tree to prefer under a data root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataVariant {
    #[default]
    Simplified,
    Traditional,
}

impl DataVariant {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Simplified => "simplified",
            Self::Traditional => "traditional",
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// User-writable config/data home (`$HOME/.local/share/libpinyin` on Unix).
pub fn user_data_dir() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".local/share/libpinyin"))
}

/// Default path for the persistent user dictionary (`userdict.redb`).
///
/// Override with `LIBPINYIN_USERDICT`.
pub fn userdict_path() -> PathBuf {
    if let Ok(p) = env::var("LIBPINYIN_USERDICT") {
        return PathBuf::from(p);
    }
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pinyin")
        .join("userdict.redb")
}

/// Explicit data-dir env override (`LIBPINYIN_DATA_DIR`).
pub fn data_dir_from_env() -> Option<PathBuf> {
    env::var_os("LIBPINYIN_DATA_DIR").map(PathBuf::from)
}

fn sys_data_root() -> PathBuf {
    PathBuf::from(SYS_PATH.unwrap_or(DEFAULT_SYS_PATH))
}

fn looks_like_data_dir(dir: &Path) -> bool {
    dir.join("lexicon.fst").is_file() && dir.join("lexicon.dat").is_file()
}

/// Ordered candidate directories for `variant` (does not check existence).
pub fn data_search_paths(variant: DataVariant) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(env_dir) = data_dir_from_env() {
        out.push(env_dir);
    }
    let suffix = variant.as_str();
    if let Some(user) = user_data_dir() {
        out.push(user.join("data").join(suffix));
        out.push(user.join("data"));
    }
    let sys = sys_data_root();
    out.push(sys.join(suffix));
    out.push(sys.clone());

    // Dev checkouts: repo `data/converted/{variant}` relative to cwd.
    out.push(PathBuf::from("data/converted").join(suffix));
    out.push(PathBuf::from("../libpinyin/data/converted").join(suffix));

    let mut seen = std::collections::HashSet::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

/// First search path that contains a usable lexicon, if any.
pub fn find_data_dir(variant: DataVariant) -> Option<PathBuf> {
    data_search_paths(variant)
        .into_iter()
        .find(|p| looks_like_data_dir(p))
}

/// Resolve a data dir or return a stable fallback path for error messages.
pub fn resolve_data_dir(variant: DataVariant) -> PathBuf {
    find_data_dir(variant).unwrap_or_else(|| sys_data_root().join(variant.as_str()))
}

/// Directories to probe for optional `emoji.table` / `english.wordlist`.
pub fn aux_search_dirs(data_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![data_dir.to_path_buf()];
    if let Some(parent) = data_dir.parent() {
        dirs.push(parent.to_path_buf());
        if let Some(grandparent) = parent.parent() {
            dirs.push(grandparent.to_path_buf());
        }
    }
    if let Some(user) = user_data_dir() {
        dirs.push(user.join("data").join("simplified"));
        dirs.push(user.join("data").join("traditional"));
        dirs.push(user.join("data"));
    }
    dirs.push(sys_data_root());
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|d| seen.insert(d.clone()));
    dirs
}

/// First existing file named `filename` under [`aux_search_dirs`].
pub fn find_aux_file(data_dir: &Path, filename: &str) -> Option<PathBuf> {
    aux_search_dirs(data_dir)
        .into_iter()
        .map(|d| d.join(filename))
        .find(|p| p.is_file())
}
