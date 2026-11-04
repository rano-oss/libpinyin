//! Memory-mapped lexicon for zero-copy dictionary lookup.
//!
//! FST (`lexicon.fst`) maps phonetic keys to indices; flat `lexicon.dat` payload
//! stores entries. Only pages in data on demand, keeping RSS minimal.
//!
//! `lexicon.dat` layout:
//! ```text
//! Header (16 bytes): magic LXPD, version, num_keys, reserved
//! Key offsets: [u32; num_keys + 1]
//! Entries: per key, packed (u16 str_len + bytes + u32 freq)
//! ```

use fst::{IntoStreamer, Streamer};
use memmap2::Mmap;
use std::path::Path;

const LXPD_MAGIC: &[u8; 4] = b"LXPD";
const LXPD_VERSION: u32 = 1;
const LXPD_HEADER_SIZE: usize = 16;

/// Memory-mapped lexicon. FST for key lookup, flat payload for entries.
pub struct Lexicon {
    fst_map: fst::Map<Mmap>,
    payload_mmap: Mmap,
    num_keys: u32,
    /// Sum of all entry frequencies — upstream `get_phrase_index_total_freq()`.
    total_freq: u64,
}

impl Lexicon {
    /// Load from lexicon.fst + lexicon.dat
    pub fn load(fst_path: &Path, dat_path: &Path) -> Result<Self, String> {
        let fst_file =
            std::fs::File::open(fst_path).map_err(|e| format!("open {:?}: {}", fst_path, e))?;
        let dat_file =
            std::fs::File::open(dat_path).map_err(|e| format!("open {:?}: {}", dat_path, e))?;

        let fst_mmap = unsafe { Mmap::map(&fst_file) }.map_err(|e| format!("mmap fst: {}", e))?;
        let payload_mmap =
            unsafe { Mmap::map(&dat_file) }.map_err(|e| format!("mmap dat: {}", e))?;

        if payload_mmap.len() < LXPD_HEADER_SIZE {
            return Err("lexicon.dat too small".into());
        }
        if &payload_mmap[0..4] != LXPD_MAGIC {
            return Err("lexicon.dat: bad magic".into());
        }
        let version = u32::from_le_bytes(payload_mmap[4..8].try_into().unwrap());
        if version != LXPD_VERSION {
            return Err(format!("lexicon.dat: unsupported version {}", version));
        }
        let num_keys = u32::from_le_bytes(payload_mmap[8..12].try_into().unwrap());

        let fst_map = fst::Map::new(fst_mmap).map_err(|e| format!("fst parse: {}", e))?;

        let mut lex = Self {
            fst_map,
            payload_mmap,
            num_keys,
            total_freq: 0,
        };
        // Match FacadePhraseIndex::get_phrase_index_total_freq()
        let mut total: u64 = 0;
        for idx in 0..num_keys {
            for (_, freq) in lex.read_entries(idx) {
                total += freq as u64;
            }
        }
        lex.total_freq = total;

        Ok(lex)
    }

    /// Total of all phrase frequencies (phrase-index normalizer).
    #[inline]
    pub fn total_freq(&self) -> u64 {
        self.total_freq
    }

    #[inline]
    fn key_offsets_start(&self) -> usize {
        LXPD_HEADER_SIZE
    }

    #[inline]
    fn entries_start(&self) -> usize {
        LXPD_HEADER_SIZE + (self.num_keys as usize + 1) * 4
    }

    #[inline]
    fn get_key_offset(&self, idx: u32) -> u32 {
        let off = self.key_offsets_start() + (idx as usize) * 4;
        u32::from_le_bytes(self.payload_mmap[off..off + 4].try_into().unwrap())
    }

    fn read_entries(&self, idx: u32) -> Vec<(String, u32)> {
        self.read_entries_limited(idx, usize::MAX)
    }

    /// Read payload entries for a key index, stopping after `limit` phrases.
    ///
    /// Entries are stored highest-frequency first (`convert_table` sorts), so a
    /// small `limit` avoids materializing thousands of `shi`-class hits.
    fn read_entries_limited(&self, idx: u32, limit: usize) -> Vec<(String, u32)> {
        if limit == 0 {
            return Vec::new();
        }
        let start = self.entries_start() + self.get_key_offset(idx) as usize;
        let end = self.entries_start() + self.get_key_offset(idx + 1) as usize;
        let data = &self.payload_mmap[start..end];

        let mut results = Vec::with_capacity(limit.min(32));
        let mut pos = 0;
        while results.len() < limit && pos + 2 <= data.len() {
            let str_len = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap()) as usize;
            pos += 2;
            if pos + str_len + 4 > data.len() {
                break;
            }
            let s = std::str::from_utf8(&data[pos..pos + str_len]).unwrap_or("");
            pos += str_len;
            let freq = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
            pos += 4;
            results.push((s.to_string(), freq));
        }
        results
    }

    /// Lookup candidates for a given phonetic key.
    pub fn lookup(&self, key: &str) -> Vec<String> {
        if let Some(idx) = self.fst_map.get(key) {
            self.read_entries(idx as u32)
                .into_iter()
                .map(|(s, _)| s)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Lookup with frequencies.
    pub fn lookup_with_freq(&self, key: &str) -> Vec<(String, u32)> {
        if let Some(idx) = self.fst_map.get(key) {
            self.read_entries(idx as u32)
        } else {
            Vec::new()
        }
    }

    /// Lookup with frequencies, keeping only the `limit` highest-frequency hits.
    ///
    /// Single-syllable keys like `shi` can have thousands of entries; DP only
    /// needs the head. Payload order is descending frequency (convert_table).
    pub fn lookup_with_freq_top(&self, key: &str, limit: usize) -> Vec<(String, u32)> {
        if limit == 0 {
            return Vec::new();
        }
        if let Some(idx) = self.fst_map.get(key) {
            self.read_entries_limited(idx as u32, limit)
        } else {
            Vec::new()
        }
    }

    /// Check if a key exists (FST only, no payload access).
    pub fn has_key(&self, key: &str) -> bool {
        self.fst_map.get(key).is_some()
    }

    /// Phonetic key-prefix completion: FST keys starting with `prefix`.
    ///
    /// Used for mid-word progressive typing (`bei'j` → `bei'jing` → 北京).
    /// Only same syllable-arity keys (same `'` count) so `bei'jing` does not
    /// pull `bei'jing'hang'kong'…` (北京航空航天大学).
    pub fn lookup_prefix_with_freq_top(
        &self,
        prefix: &str,
        max_keys: usize,
        max_phrases_per_key: usize,
    ) -> Vec<(String, u32)> {
        if prefix.is_empty() || max_keys == 0 || max_phrases_per_key == 0 {
            return Vec::new();
        }
        let arity = prefix.bytes().filter(|&b| b == b'\'').count();
        let mut stream = self.fst_map.range().ge(prefix).into_stream();
        let mut out = Vec::new();
        let mut keys_seen = 0usize;
        while let Some((key_bytes, idx)) = stream.next() {
            let Ok(key) = std::str::from_utf8(key_bytes) else {
                continue;
            };
            if !key.starts_with(prefix) {
                break;
            }
            // Skip the exact key — callers already did exact lookup.
            if key == prefix {
                continue;
            }
            if key.bytes().filter(|&b| b == b'\'').count() != arity {
                continue;
            }
            keys_seen += 1;
            if keys_seen > max_keys {
                break;
            }
            out.extend(self.read_entries_limited(idx as u32, max_phrases_per_key));
        }
        out
    }
}
