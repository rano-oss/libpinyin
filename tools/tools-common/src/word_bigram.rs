// Word-level bigram model for offline building and conversion.
//
// Uses string interning (u16 IDs) and flat sorted arrays to minimize memory.

use std::collections::HashMap;
use std::path::Path;

/// Word-level bigram model with compact storage.
#[derive(Debug, Clone)]
pub struct WordBigram {
    words: Vec<String>,
    bigram_data: Vec<(u16, u32)>,
    bigram_offsets: Vec<u32>,
    totals: Vec<u32>,
    unigram_counts: Vec<u32>,
    total_unigram_count: u64,
}

/// Builder for constructing a WordBigram model incrementally.
pub struct WordBigramBuilder {
    words: Vec<String>,
    word_to_id: HashMap<String, u16>,
    data: HashMap<u16, Vec<(u16, u32)>>,
    totals: HashMap<u16, u32>,
    unigram_counts: HashMap<u16, u32>,
    total_unigram_count: u64,
}

impl WordBigramBuilder {
    pub fn new() -> Self {
        Self {
            words: Vec::new(),
            word_to_id: HashMap::new(),
            data: HashMap::new(),
            totals: HashMap::new(),
            unigram_counts: HashMap::new(),
            total_unigram_count: 0,
        }
    }

    fn intern(&mut self, word: &str) -> u16 {
        if let Some(&id) = self.word_to_id.get(word) {
            return id;
        }
        let id = self.words.len() as u16;
        self.words.push(word.to_string());
        self.word_to_id.insert(word.to_string(), id);
        id
    }

    pub fn add_bigram(&mut self, word1: String, word2: String, count: u32) {
        let id1 = self.intern(&word1);
        let id2 = self.intern(&word2);
        self.data.entry(id1).or_default().push((id2, count));
        *self.totals.entry(id1).or_insert(0) += count;
    }

    pub fn add_unigram(&mut self, word: String, count: u32) {
        let id = self.intern(&word);
        *self.unigram_counts.entry(id).or_insert(0) += count;
        self.total_unigram_count += count as u64;
    }

    pub fn build(self) -> WordBigram {
        let num_words = self.words.len();

        let mut totals = vec![0u32; num_words];
        for (&id, &total) in &self.totals {
            totals[id as usize] = total;
        }

        let mut unigram_counts = vec![0u32; num_words];
        for (&id, &count) in &self.unigram_counts {
            unigram_counts[id as usize] = count;
        }

        let mut bigram_offsets = vec![0u32; num_words + 1];
        let mut bigram_data = Vec::new();

        for word_id in 0..num_words as u16 {
            bigram_offsets[word_id as usize] = bigram_data.len() as u32;
            if let Some(entries) = self.data.get(&word_id) {
                bigram_data.extend_from_slice(entries);
            }
        }
        bigram_offsets[num_words] = bigram_data.len() as u32;

        WordBigram {
            words: self.words,
            bigram_data,
            bigram_offsets,
            totals,
            unigram_counts,
            total_unigram_count: self.total_unigram_count,
        }
    }
}

impl Default for WordBigramBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl WordBigram {
    pub fn new() -> Self {
        Self {
            words: Vec::new(),
            bigram_data: Vec::new(),
            bigram_offsets: vec![0],
            totals: Vec::new(),
            unigram_counts: Vec::new(),
            total_unigram_count: 0,
        }
    }

    pub fn num_words(&self) -> usize {
        self.words.len()
    }

    pub fn total_bigrams(&self) -> usize {
        self.bigram_data.len()
    }

    /// Get number of unique word1 entries that have bigrams.
    pub fn len(&self) -> usize {
        (0..self.words.len())
            .filter(|&i| {
                let start = self.bigram_offsets[i] as usize;
                let end = self.bigram_offsets[i + 1] as usize;
                end > start
            })
            .count()
    }

    pub fn is_empty(&self) -> bool {
        self.bigram_data.is_empty()
    }

    pub fn word_by_id(&self, id: u16) -> &str {
        &self.words[id as usize]
    }

    pub fn unigram_count_by_id(&self, id: u16) -> u32 {
        self.unigram_counts.get(id as usize).copied().unwrap_or(0)
    }

    /// Get raw bigram entries and total for a word1_id.
    pub fn bigrams_raw(&self, word1_id: u16) -> (&[(u16, u32)], u32) {
        let start = self.bigram_offsets[word1_id as usize] as usize;
        let end = self.bigram_offsets[word1_id as usize + 1] as usize;
        let entries = &self.bigram_data[start..end];
        let total = self.totals.get(word1_id as usize).copied().unwrap_or(0);
        (entries, total)
    }

    pub fn total_unigram_count(&self) -> u64 {
        self.total_unigram_count
    }
}

impl Default for WordBigram {
    fn default() -> Self {
        Self::new()
    }
}

impl WordBigram {
    /// Write directly to mmap-ready .dat + .fst files (bypasses bincode intermediate).
    pub fn write_mmap<P: AsRef<Path>>(
        &self,
        dat_path: P,
        fst_path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let num_words = self.words.len() as u32;

        // Build string pool (length-prefixed: u16 len + utf8 bytes)
        let mut string_pool = Vec::new();
        let mut string_offsets = Vec::with_capacity(self.words.len());
        for word in &self.words {
            string_offsets.push(string_pool.len() as u32);
            let bytes = word.as_bytes();
            string_pool.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            string_pool.extend_from_slice(bytes);
        }

        // Build FST: word -> packed(id << 32 | unigram_count), sorted by key
        let mut sorted_for_fst: Vec<(&str, u64)> = self
            .words
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let uc = self.unigram_counts.get(i).copied().unwrap_or(0);
                (w.as_str(), ((i as u64) << 32) | (uc as u64))
            })
            .collect();
        sorted_for_fst.sort_by(|a, b| a.0.cmp(b.0));

        let mut fst_builder = fst::MapBuilder::memory();
        for (word, packed) in &sorted_for_fst {
            fst_builder.insert(word, *packed)?;
        }
        let fst_bytes = fst_builder.into_inner()?;
        std::fs::write(fst_path.as_ref(), &fst_bytes)?;

        // Write .dat
        crate::mmap_write::write_word_bigram_dat(
            dat_path.as_ref(),
            num_words,
            self.total_unigram_count,
            &string_offsets,
            &string_pool,
            &self.bigram_offsets,
            &self.totals,
            &self.bigram_data,
        )?;

        Ok(())
    }
}
