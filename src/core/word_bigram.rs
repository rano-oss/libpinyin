//! Memory-mapped word bigram model for zero-copy n-gram scoring.
//!
//! Provides P(word2|word1) lookups and unigram probabilities
//! with zero heap allocation for model data.

use memmap2::Mmap;
use std::path::Path;

const WBGR_MAGIC: &[u8; 4] = b"WBGR";
const WBGR_VERSION: u32 = 1;
const WBGR_HEADER_SIZE: usize = 32;

/// Memory-mapped word bigram model.
pub struct WordBigram {
    data_mmap: Option<Mmap>,
    fst_map: Option<fst::Map<Mmap>>,
    num_words: u32,
    num_bigrams: u32,
    string_pool_size: u32,
    total_unigram_count: u64,
}

impl WordBigram {
    /// Create an empty word bigram (no data).
    pub fn empty() -> Self {
        Self {
            data_mmap: None,
            fst_map: None,
            num_words: 0,
            num_bigrams: 0,
            string_pool_size: 0,
            total_unigram_count: 0,
        }
    }

    /// Load from word_bigram.dat + word_bigram_words.fst
    pub fn load(dat_path: &Path, fst_path: &Path) -> Result<Self, String> {
        let dat_file =
            std::fs::File::open(dat_path).map_err(|e| format!("open {:?}: {}", dat_path, e))?;
        let fst_file =
            std::fs::File::open(fst_path).map_err(|e| format!("open {:?}: {}", fst_path, e))?;

        let data_mmap = unsafe { Mmap::map(&dat_file) }.map_err(|e| format!("mmap dat: {}", e))?;
        let fst_mmap = unsafe { Mmap::map(&fst_file) }.map_err(|e| format!("mmap fst: {}", e))?;

        if data_mmap.len() < WBGR_HEADER_SIZE {
            return Err("word_bigram.dat too small".into());
        }
        if &data_mmap[0..4] != WBGR_MAGIC {
            return Err("word_bigram.dat: bad magic".into());
        }
        let version = u32::from_le_bytes(data_mmap[4..8].try_into().unwrap());
        if version != WBGR_VERSION {
            return Err(format!("word_bigram.dat: unsupported version {}", version));
        }

        let num_words = u32::from_le_bytes(data_mmap[8..12].try_into().unwrap());
        let num_bigrams = u32::from_le_bytes(data_mmap[12..16].try_into().unwrap());
        let string_pool_size = u32::from_le_bytes(data_mmap[16..20].try_into().unwrap());
        let total_unigram_count = u64::from_le_bytes(data_mmap[20..28].try_into().unwrap());

        let expected_size = WBGR_HEADER_SIZE
            + (num_words as usize) * 4
            + string_pool_size as usize
            + (num_words as usize + 1) * 4
            + (num_words as usize) * 4
            + (num_bigrams as usize) * 6;
        if data_mmap.len() < expected_size {
            return Err(format!(
                "word_bigram.dat: expected {} bytes, got {}",
                expected_size,
                data_mmap.len()
            ));
        }

        let fst_map = fst::Map::new(fst_mmap).map_err(|e| format!("fst parse: {}", e))?;

        Ok(Self {
            data_mmap: Some(data_mmap),
            fst_map: Some(fst_map),
            num_words,
            num_bigrams,
            string_pool_size,
            total_unigram_count,
        })
    }

    #[inline]
    fn data(&self) -> &[u8] {
        self.data_mmap.as_ref().unwrap()
    }

    #[inline]
    fn string_offsets(&self) -> &[u8] {
        let start = WBGR_HEADER_SIZE;
        let end = start + (self.num_words as usize) * 4;
        &self.data()[start..end]
    }

    #[inline]
    fn string_pool(&self) -> &[u8] {
        let start = WBGR_HEADER_SIZE + (self.num_words as usize) * 4;
        let end = start + self.string_pool_size as usize;
        &self.data()[start..end]
    }

    #[inline]
    fn bigram_offsets(&self) -> &[u8] {
        let start =
            WBGR_HEADER_SIZE + (self.num_words as usize) * 4 + self.string_pool_size as usize;
        let end = start + (self.num_words as usize + 1) * 4;
        &self.data()[start..end]
    }

    #[inline]
    fn totals(&self) -> &[u8] {
        let start = WBGR_HEADER_SIZE
            + (self.num_words as usize) * 4
            + self.string_pool_size as usize
            + (self.num_words as usize + 1) * 4;
        let end = start + (self.num_words as usize) * 4;
        &self.data()[start..end]
    }

    #[inline]
    fn bigram_data(&self) -> &[u8] {
        let start = WBGR_HEADER_SIZE
            + (self.num_words as usize) * 4
            + self.string_pool_size as usize
            + (self.num_words as usize + 1) * 4
            + (self.num_words as usize) * 4;
        let end = start + (self.num_bigrams as usize) * 6;
        &self.data()[start..end]
    }

    #[inline]
    fn get_string_offset(&self, id: u16) -> u32 {
        let off = (id as usize) * 4;
        u32::from_le_bytes(self.string_offsets()[off..off + 4].try_into().unwrap())
    }

    /// Get word string for a given id (zero-copy into mmap).
    pub fn id_to_word(&self, id: u16) -> &str {
        let pool = self.string_pool();
        let offset = self.get_string_offset(id) as usize;
        let len = u16::from_le_bytes(pool[offset..offset + 2].try_into().unwrap()) as usize;
        std::str::from_utf8(&pool[offset + 2..offset + 2 + len]).unwrap_or("")
    }

    #[inline]
    fn lookup_word(&self, word: &str) -> Option<(u16, u32)> {
        self.fst_map.as_ref()?.get(word).map(|packed| {
            // Writer stores `(id as u64) << 32 | unigram_count`; id fits in u16
            // (num_words ≤ 65535) but must not truncate a corrupted/wide value silently.
            let id32 = (packed >> 32) as u32;
            let id = id32 as u16;
            let unigram_count = packed as u32;
            debug_assert!(
                (id32 as usize) < self.num_words as usize,
                "word bigram id {id32} out of range for {word:?} (num_words={})",
                self.num_words
            );
            (id, unigram_count)
        })
    }

    #[inline]
    fn lookup_id(&self, word: &str) -> Option<u16> {
        self.lookup_word(word).map(|(id, _)| id)
    }

    #[inline]
    fn get_bigram_offset(&self, id: u16) -> u32 {
        let off = (id as usize) * 4;
        u32::from_le_bytes(self.bigram_offsets()[off..off + 4].try_into().unwrap())
    }

    #[inline]
    fn get_total(&self, id: u16) -> u32 {
        let off = (id as usize) * 4;
        u32::from_le_bytes(self.totals()[off..off + 4].try_into().unwrap())
    }

    #[inline]
    fn get_unigram_count(&self, word: &str) -> u32 {
        self.lookup_word(word).map(|(_, c)| c).unwrap_or(0)
    }

    fn bigrams_for(&self, word1_id: u16) -> BigramIter<'_> {
        let start = self.get_bigram_offset(word1_id) as usize;
        let end = self.get_bigram_offset(word1_id + 1) as usize;
        let data = self.bigram_data();
        BigramIter {
            data: &data[start * 6..end * 6],
            pos: 0,
        }
    }

    /// Get probability P(word2 | word1).
    pub fn get_probability(&self, word1: &str, word2: &str) -> f32 {
        let id1 = match self.lookup_id(word1) {
            Some(id) => id,
            None => return 0.0,
        };
        let id2 = match self.lookup_id(word2) {
            Some(id) => id,
            None => return 0.0,
        };
        let total = self.get_total(id1);
        if total == 0 {
            return 0.0;
        }
        for (wid, count) in self.bigrams_for(id1) {
            if wid == id2 {
                return count as f32 / total as f32;
            }
        }
        0.0
    }

    /// Get unigram probability P(word).
    pub fn get_unigram_probability(&self, word: &str) -> f32 {
        if self.total_unigram_count == 0 {
            return 0.0;
        }
        let count = self.get_unigram_count(word);
        if count == 0 {
            return 0.0;
        }
        (count as f64 / self.total_unigram_count as f64) as f32
    }

    /// Get top N predictions after word1.
    pub fn get_predictions(&self, word1: &str, lambda: f32, top_n: usize) -> Vec<(String, f32)> {
        let id1 = match self.lookup_id(word1) {
            Some(id) => id,
            None => return Vec::new(),
        };
        let total = self.get_total(id1);
        if total == 0 {
            return Vec::new();
        }

        let mut predictions: Vec<(String, f32)> = self
            .bigrams_for(id1)
            .map(|(id2, count)| {
                let bigram_prob = count as f32 / total as f32;
                let unigram_count = self
                    .lookup_word(self.id_to_word(id2))
                    .map(|(_, c)| c)
                    .unwrap_or(0);
                let unigram_prob = if self.total_unigram_count > 0 {
                    (unigram_count as f64 / self.total_unigram_count as f64) as f32
                } else {
                    0.0
                };
                let interpolated = lambda * bigram_prob + (1.0 - lambda) * unigram_prob;
                let score = interpolated.ln();
                (self.id_to_word(id2).to_string(), score)
            })
            .collect();

        predictions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        predictions.truncate(top_n);
        predictions
    }

    pub fn num_words(&self) -> usize {
        self.num_words as usize
    }

    pub fn total_bigrams(&self) -> usize {
        self.num_bigrams as usize
    }

    pub fn is_empty(&self) -> bool {
        self.num_bigrams == 0
    }
}

struct BigramIter<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Iterator for BigramIter<'a> {
    type Item = (u16, u32);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos + 6 > self.data.len() {
            return None;
        }
        let word2_id = u16::from_le_bytes(self.data[self.pos..self.pos + 2].try_into().unwrap());
        let count = u32::from_le_bytes(self.data[self.pos + 2..self.pos + 6].try_into().unwrap());
        self.pos += 6;
        Some((word2_id, count))
    }
}
