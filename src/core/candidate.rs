//! Candidate types for IME text conversion.
//!
//! This module provides:
//! - `Candidate`: A single text candidate with score
//! - `CandidateList`: Paginated list with cursor navigation

use std::ops::Range;

/// A single text candidate with an associated score.
///
/// Scores are on a relative scale; higher is better. Use `f32` for compactness
/// and performance.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub text: String,
    pub score: f32,
    /// Number of input bytes this candidate consumes (for partial candidates).
    /// When set, selecting this candidate should remove this many bytes from the input buffer
    /// rather than clearing the entire buffer.
    pub input_consumed: Option<usize>,
}

impl Candidate {
    pub fn new<T: Into<String>>(text: T, score: f32) -> Self {
        Candidate {
            text: text.into(),
            score,
            input_consumed: None,
        }
    }

    pub fn with_input_consumed<T: Into<String>>(text: T, score: f32, consumed: usize) -> Self {
        Candidate {
            text: text.into(),
            score,
            input_consumed: Some(consumed),
        }
    }
}

/// A paginated list of candidates with cursor navigation.
#[derive(Debug, Clone)]
pub struct CandidateList {
    /// All available candidates
    candidates: Vec<Candidate>,

    /// Number of candidates per page
    page_size: usize,

    /// Current page index (0-based)
    current_page: usize,

    /// Cursor position within the current page (0-based)
    cursor: usize,
}

impl Default for CandidateList {
    fn default() -> Self {
        Self {
            candidates: Vec::new(),
            page_size: 5,
            current_page: 0,
            cursor: 0,
        }
    }
}

impl CandidateList {
    /// Create a candidate list with specified page size.
    pub fn with_page_size(page_size: usize) -> Self {
        Self {
            candidates: Vec::new(),
            page_size: page_size.max(1), // Ensure at least 1
            current_page: 0,
            cursor: 0,
        }
    }

    /// Set the page size.
    pub fn set_page_size(&mut self, page_size: usize) {
        self.page_size = page_size.max(1);
        // Reset to first page if current page is now out of bounds
        if self.current_page >= self.num_pages() && self.num_pages() > 0 {
            self.current_page = 0;
        }
        // Reset cursor if it's out of bounds
        if self.cursor >= self.current_page_len() && self.current_page_len() > 0 {
            self.cursor = 0;
        }
    }

    /// Get the page size.
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// Set the candidates, resetting pagination state.
    pub fn set_candidates(&mut self, candidates: Vec<Candidate>) {
        self.candidates = candidates;
        self.current_page = 0;
        self.cursor = 0;
    }

    /// Get all candidates.
    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    /// Get the total number of candidates.
    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// Check if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// Get the total number of pages.
    pub fn num_pages(&self) -> usize {
        if self.candidates.is_empty() {
            0
        } else {
            self.candidates.len().div_ceil(self.page_size)
        }
    }

    /// Get the current page index (0-based).
    pub fn current_page(&self) -> usize {
        self.current_page
    }

    /// Get the cursor position within the current page (0-based).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Get the number of candidates on the current page.
    fn current_page_len(&self) -> usize {
        let start = self.current_page * self.page_size;
        let end = (start + self.page_size).min(self.candidates.len());
        end.saturating_sub(start)
    }

    /// Get the range of candidates for the current page.
    fn current_page_range(&self) -> Range<usize> {
        let start = self.current_page * self.page_size;
        let end = (start + self.page_size).min(self.candidates.len());
        start..end
    }

    /// Get the candidates for the current page.
    pub fn current_page_candidates(&self) -> &[Candidate] {
        if self.candidates.is_empty() {
            return &[];
        }
        let range = self.current_page_range();
        &self.candidates[range]
    }

    /// Get the currently selected candidate (under cursor).
    pub fn selected_candidate(&self) -> Option<&Candidate> {
        let page_candidates = self.current_page_candidates();
        page_candidates.get(self.cursor)
    }

    /// Move cursor up (to previous candidate on current page).
    /// Returns true if the cursor moved.
    pub fn cursor_up(&mut self) -> bool {
        if self.cursor > 0 {
            self.cursor -= 1;
            true
        } else {
            false
        }
    }

    /// Move cursor down (to next candidate on current page).
    /// Returns true if the cursor moved.
    pub fn cursor_down(&mut self) -> bool {
        let page_len = self.current_page_len();
        if page_len > 0 && self.cursor < page_len - 1 {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    /// Move to the previous page.
    /// Returns true if the page changed.
    pub fn page_up(&mut self) -> bool {
        if self.current_page > 0 {
            self.current_page -= 1;
            // Keep cursor valid
            let page_len = self.current_page_len();
            if page_len > 0 && self.cursor >= page_len {
                self.cursor = page_len - 1;
            }
            true
        } else {
            false
        }
    }

    /// Move to the next page.
    /// Returns true if the page changed.
    pub fn page_down(&mut self) -> bool {
        let num_pages = self.num_pages();
        if num_pages > 0 && self.current_page < num_pages - 1 {
            self.current_page += 1;
            // Keep cursor valid
            let page_len = self.current_page_len();
            if page_len > 0 && self.cursor >= page_len {
                self.cursor = page_len - 1;
            }
            true
        } else {
            false
        }
    }

    /// Select a candidate by index within the current page.
    /// Returns the selected candidate if the index is valid.
    pub fn select_by_index(&mut self, page_index: usize) -> Option<&Candidate> {
        if page_index < self.current_page_len() {
            self.cursor = page_index;
            self.selected_candidate()
        } else {
            None
        }
    }

    /// Clear the candidate list.
    pub fn clear(&mut self) {
        self.candidates.clear();
        self.current_page = 0;
        self.cursor = 0;
    }

    /// Reset pagination state (go to first page, first candidate).
    pub fn reset(&mut self) {
        self.current_page = 0;
        self.cursor = 0;
    }
}
