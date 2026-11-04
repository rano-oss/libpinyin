//! Redb-backed UserDict with an in-memory hot cache for scoring / mask checks.
//!
//! Hot-path reads (`frequency`, `is_masked`) never open redb transactions.
//! Mutations update both the DB and the cache.

use std::collections::{HashMap, HashSet};

use redb::{Database, ReadableTable, TableDefinition};

#[derive(Default)]
struct HotCache {
    freqs: HashMap<String, u64>,
    masks: HashSet<String>,
}

/// UserDict backed by `redb`.
pub struct UserDict {
    db: Database,
    hot: HotCache,
}

impl std::fmt::Debug for UserDict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserDict")
            .field("phrases", &self.hot.freqs.len())
            .field("masks", &self.hot.masks.len())
            .finish()
    }
}

#[allow(clippy::result_large_err)] // redb::Error is large; keep typed API
impl UserDict {
    /// Create/open a redb-backed user dict at the given path.
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Result<Self, redb::Error> {
        if let Some(dir) = path.as_ref().parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let db = Database::create(path.as_ref())?;
        let mut ud = UserDict {
            db,
            hot: HotCache::default(),
        };
        ud.reload_hot_cache()?;
        Ok(ud)
    }

    fn table_def() -> TableDefinition<'static, &'static str, u64> {
        TableDefinition::new("user_dict")
    }

    fn bigram_table_def() -> TableDefinition<'static, &'static str, u64> {
        TableDefinition::new("user_bigram")
    }

    fn masked_table_def() -> TableDefinition<'static, &'static str, u64> {
        // value unused (always 1); presence means "hidden from candidates"
        TableDefinition::new("masked_phrases")
    }

    /// Encode bigram key as "w1\0w2" for redb storage.
    fn encode_bigram_key(w1: &str, w2: &str) -> String {
        format!("{}\0{}", w1, w2)
    }

    /// Decode bigram key from "w1\0w2" format.
    fn decode_bigram_key(key: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = key.split('\0').collect();
        if parts.len() == 2 {
            Some((parts[0].to_string(), parts[1].to_string()))
        } else {
            None
        }
    }

    fn reload_hot_cache(&mut self) -> Result<(), redb::Error> {
        let mut freqs = HashMap::new();
        let mut masks = HashSet::new();
        let r = self.db.begin_read()?;
        match r.open_table(Self::table_def()) {
            Ok(table) => {
                for item in table.iter()? {
                    let (k, v) = item?;
                    freqs.insert(k.value().to_string(), v.value());
                }
            }
            Err(redb::TableError::TableDoesNotExist(_)) => {}
            Err(e) => return Err(e.into()),
        }
        match r.open_table(Self::masked_table_def()) {
            Ok(table) => {
                for item in table.iter()? {
                    let (k, _) = item?;
                    masks.insert(k.value().to_string());
                }
            }
            Err(redb::TableError::TableDoesNotExist(_)) => {}
            Err(e) => return Err(e.into()),
        }
        self.hot = HotCache { freqs, masks };
        Ok(())
    }

    /// Learn a phrase (increment by 1).
    pub fn learn(&mut self, phrase: &str) {
        let _ = self.learn_with_count(phrase, 1);
    }

    /// Learn with a custom delta.
    ///
    /// Also clears any persisted mask so re-learning a forgotten phrase restores it.
    pub fn learn_with_count(&mut self, phrase: &str, delta: u64) -> Result<(), redb::Error> {
        let cur = self.frequency(phrase);
        let new = cur.saturating_add(delta);
        let w = self.db.begin_write()?;
        {
            let mut table = w.open_table(Self::table_def())?;
            table.insert(&phrase, &new)?;
            let mut masks = w.open_table(Self::masked_table_def())?;
            let _ = masks.remove(&phrase)?;
        }
        w.commit()?;
        let hot = &mut self.hot;
        hot.freqs.insert(phrase.to_string(), new);
        hot.masks.remove(phrase);
        Ok(())
    }

    /// Get frequency for phrase (hot cache; no redb I/O).
    pub fn frequency(&self, phrase: &str) -> u64 {
        self.hot.freqs.get(phrase).copied().unwrap_or(0)
    }

    /// Snapshot full contents as a HashMap.
    pub fn snapshot(&self) -> HashMap<String, u64> {
        self.hot.freqs.clone()
    }

    /// Iterate all entries as Vec<(String,u64)>.
    pub fn iter_all(&self) -> Vec<(String, u64)> {
        self.hot
            .freqs
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    // ========== User Bigram Learning API ==========

    /// Learn a bigram (w1 → w2) by incrementing its count.
    pub fn learn_bigram(&mut self, w1: &str, w2: &str) {
        let _ = self.learn_bigram_with_count(w1, w2, 1);
    }

    /// Learn a bigram with a custom count delta.
    pub fn learn_bigram_with_count(
        &mut self,
        w1: &str,
        w2: &str,
        delta: u64,
    ) -> Result<(), redb::Error> {
        let key = Self::encode_bigram_key(w1, w2);
        let cur = self.bigram_frequency(w1, w2);
        let w = self.db.begin_write()?;
        {
            let mut table = w.open_table(Self::bigram_table_def())?;
            let new = cur.saturating_add(delta);
            table.insert(key.as_str(), &new)?;
        }
        w.commit()?;
        Ok(())
    }

    /// Get the frequency count for a specific bigram (w1 → w2).
    pub fn bigram_frequency(&self, w1: &str, w2: &str) -> u64 {
        self.bigram_frequency_result(w1, w2).unwrap_or(0)
    }

    fn bigram_frequency_result(&self, w1: &str, w2: &str) -> Result<u64, redb::Error> {
        let key = Self::encode_bigram_key(w1, w2);
        let r = self.db.begin_read()?;
        match r.open_table(Self::bigram_table_def()) {
            Ok(table) => {
                if let Some(v) = table.get(key.as_str())? {
                    Ok(v.value())
                } else {
                    Ok(0)
                }
            }
            Err(e) => {
                if matches!(e, redb::TableError::TableDoesNotExist(_)) {
                    Ok(0)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    /// Get all bigrams that start with w1, returning (w2 → count) mapping.
    pub fn get_bigrams_after(&self, w1: &str) -> HashMap<String, u64> {
        self.get_bigrams_after_result(w1).unwrap_or_default()
    }

    fn get_bigrams_after_result(&self, w1: &str) -> Result<HashMap<String, u64>, redb::Error> {
        let mut out = HashMap::new();
        let r = self.db.begin_read()?;
        let prefix = format!("{}\0", w1);

        match r.open_table(Self::bigram_table_def()) {
            Ok(table) => {
                for item in table.iter()? {
                    let (key, count) = item?;
                    let key_str = key.value();
                    if key_str.starts_with(&prefix) {
                        if let Some((_, w2)) = Self::decode_bigram_key(key_str) {
                            out.insert(w2, count.value());
                        }
                    }
                }
            }
            Err(e) => {
                if matches!(e, redb::TableError::TableDoesNotExist(_)) {
                    // Empty table
                } else {
                    return Err(e.into());
                }
            }
        }
        Ok(out)
    }

    // ========== User Phrase Management API for GUI ==========

    /// List all phrases in user dictionary.
    pub fn list_all(&self) -> Vec<(String, u64)> {
        self.iter_all()
    }

    /// Add a phrase manually with specified frequency.
    pub fn add_phrase(&mut self, phrase: &str, frequency: u64) -> Result<(), redb::Error> {
        let w = self.db.begin_write()?;
        {
            let mut table = w.open_table(Self::table_def())?;
            table.insert(&phrase, &frequency)?;
            let mut masks = w.open_table(Self::masked_table_def())?;
            let _ = masks.remove(&phrase)?;
        }
        w.commit()?;
        let hot = &mut self.hot;
        hot.freqs.insert(phrase.to_string(), frequency);
        hot.masks.remove(phrase);
        Ok(())
    }

    /// Delete a phrase from the user dictionary.
    pub fn delete_phrase(&mut self, phrase: &str) -> Result<(), redb::Error> {
        let w = self.db.begin_write()?;
        {
            let mut table = w.open_table(Self::table_def())?;
            table.remove(&phrase)?;
        }
        w.commit()?;
        self.hot.freqs.remove(phrase);
        Ok(())
    }

    /// Forget a phrase: remove learned frequency and hide it from future candidates.
    ///
    /// Delete + mask run in one write transaction so a crash cannot leave
    /// frequency gone without a mask (phrase would otherwise reappear).
    pub fn forget_phrase(&mut self, phrase: &str) -> Result<(), redb::Error> {
        if phrase.is_empty() {
            return Ok(());
        }
        let w = self.db.begin_write()?;
        {
            let mut freqs = w.open_table(Self::table_def())?;
            let _ = freqs.remove(&phrase)?;
            let mut masks = w.open_table(Self::masked_table_def())?;
            masks.insert(&phrase, &1u64)?;
        }
        w.commit()?;
        let hot = &mut self.hot;
        hot.freqs.remove(phrase);
        hot.masks.insert(phrase.to_string());
        Ok(())
    }

    /// Persist a mask so `phrase` is hidden from candidates.
    pub fn mask_phrase(&mut self, phrase: &str) -> Result<(), redb::Error> {
        if phrase.is_empty() {
            return Ok(());
        }
        let w = self.db.begin_write()?;
        {
            let mut table = w.open_table(Self::masked_table_def())?;
            table.insert(&phrase, &1u64)?;
        }
        w.commit()?;
        self.hot.masks.insert(phrase.to_string());
        Ok(())
    }

    /// Remove a mask (allow the phrase in candidates again).
    pub fn unmask_phrase(&mut self, phrase: &str) -> Result<bool, redb::Error> {
        let w = self.db.begin_write()?;
        let removed = {
            let mut table = w.open_table(Self::masked_table_def())?;
            let guard = table.remove(&phrase)?;
            let had = guard.is_some();
            drop(guard);
            had
        };
        w.commit()?;
        if removed {
            self.hot.masks.remove(phrase);
        }
        Ok(removed)
    }

    /// Whether `phrase` is masked (hot cache; no redb I/O).
    pub fn is_masked(&self, phrase: &str) -> bool {
        self.hot.masks.contains(phrase)
    }

    /// True if any phrase is masked (avoids per-candidate checks when empty).
    pub fn has_masks(&self) -> bool {
        !self.hot.masks.is_empty()
    }

    /// List all masked phrases.
    pub fn list_masked(&self) -> Vec<String> {
        let mut out: Vec<String> = self.hot.masks.iter().cloned().collect();
        out.sort();
        out
    }

    pub fn clear_masked(&mut self) -> Result<(), redb::Error> {
        let phrases = self.list_masked();
        if phrases.is_empty() {
            return Ok(());
        }
        let w = self.db.begin_write()?;
        {
            let mut table = w.open_table(Self::masked_table_def())?;
            for p in &phrases {
                let _ = table.remove(p.as_str())?;
            }
        }
        w.commit()?;
        self.hot.masks.clear();
        Ok(())
    }

    /// Search phrases by prefix (for GUI filtering).
    pub fn search_by_prefix(&self, prefix: &str) -> Result<Vec<(String, u64)>, redb::Error> {
        let mut results: Vec<(String, u64)> = self
            .hot
            .freqs
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        results.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(results)
    }
}
