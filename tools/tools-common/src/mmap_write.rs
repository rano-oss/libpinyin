//! Write helpers for generating mmap-ready .dat files.

use std::io::Write;
use std::path::Path;

const WBGR_MAGIC: &[u8; 4] = b"WBGR";
const WBGR_VERSION: u32 = 1;

const LXPD_MAGIC: &[u8; 4] = b"LXPD";
const LXPD_VERSION: u32 = 1;

/// Write a word_bigram.dat file from the given components.
pub fn write_word_bigram_dat(
    path: &Path,
    num_words: u32,
    total_unigram_count: u64,
    string_offsets: &[u32],
    string_pool: &[u8],
    bigram_offsets: &[u32],
    totals: &[u32],
    bigram_data: &[(u16, u32)],
) -> std::io::Result<()> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);

    // Header (32 bytes)
    f.write_all(WBGR_MAGIC)?;
    f.write_all(&WBGR_VERSION.to_le_bytes())?;
    f.write_all(&num_words.to_le_bytes())?;
    f.write_all(&(bigram_data.len() as u32).to_le_bytes())?;
    f.write_all(&(string_pool.len() as u32).to_le_bytes())?;
    f.write_all(&total_unigram_count.to_le_bytes())?;
    f.write_all(&0u32.to_le_bytes())?; // reserved

    for &off in string_offsets {
        f.write_all(&off.to_le_bytes())?;
    }
    f.write_all(string_pool)?;
    for &off in bigram_offsets {
        f.write_all(&off.to_le_bytes())?;
    }
    for &t in totals {
        f.write_all(&t.to_le_bytes())?;
    }
    for &(word2_id, count) in bigram_data {
        f.write_all(&word2_id.to_le_bytes())?;
        f.write_all(&count.to_le_bytes())?;
    }

    f.flush()
}

/// Write a lexicon.dat file from the given components.
pub fn write_lexicon_dat(
    path: &Path,
    num_keys: u32,
    key_offsets: &[u32],
    entries_data: &[u8],
) -> std::io::Result<()> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);

    // Header (16 bytes)
    f.write_all(LXPD_MAGIC)?;
    f.write_all(&LXPD_VERSION.to_le_bytes())?;
    f.write_all(&num_keys.to_le_bytes())?;
    f.write_all(&0u32.to_le_bytes())?; // reserved

    for &off in key_offsets {
        f.write_all(&off.to_le_bytes())?;
    }
    f.write_all(entries_data)?;

    f.flush()
}
