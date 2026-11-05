//! IME session management.
//!
//! The `ImeSession` struct combines all the IME components (input buffer,
//! composition, candidates) into a cohesive session that tracks state across
//! multiple key events.

use std::ops::Range;

use crate::context::Context;

/// Current input mode of the IME session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    /// Initial state, no input yet
    #[default]
    Init,
    /// Phonetic input (pinyin/zhuyin)
    Phonetic,
    /// Punctuation input
    Punctuation,
    /// Suggestion/prediction mode
    Suggestion,
    /// Passthrough mode (keys not processed by IME)
    Passthrough,
}

/// IME session state combining all session components.
///
/// This struct manages the state across multiple key events. It contains:
/// - Input buffer for raw user input
/// - Composition for visual preedit display
/// - Candidate list for conversion options
/// - Current input mode
///
/// The session is separate from the backend engine - the engine provides
/// the linguistic intelligence, while the session manages UI state.
#[derive(Debug, Clone, Default)]
pub struct ImeSession {
    /// Raw input buffer (e.g., "nihao")
    pub input_buffer: InputBuffer,

    /// Visual composition (e.g., "你好" with segments)
    pub composition: Composition,

    /// Available candidates
    pub candidates: CandidateList,

    /// Current input mode
    pub mode: InputMode,

    /// Whether the session is active (has state)
    pub active: bool,
}

impl ImeSession {
    /// Create a session with specified candidate page size.
    pub fn with_page_size(page_size: usize) -> Self {
        Self {
            candidates: CandidateList::with_page_size(page_size),
            ..Self::default()
        }
    }

    /// Clear all session state and return to Init mode.
    pub fn clear(&mut self) {
        self.input_buffer.clear();
        self.composition.clear();
        self.candidates.clear();
        self.mode = InputMode::Init;
        self.active = false;
    }

    /// Update the composition from the current input buffer.
    /// This is typically called after the input buffer changes.
    pub fn update_composition_from_input(&mut self) {
        let input_text = self.input_buffer.text().to_string();
        self.composition = Composition::from_text(input_text);
    }

    /// Sync session state into a host [`Context`].
    pub fn sync_to_context(&self, context: &mut Context) {
        // Clear previous state
        context.preedit_text.clear();
        context.candidates.clear();
        context.page_text.clear();
        context.preedit_segments.clear();
        // Leave auxiliary_text for Editor hints (tone filter, etc.)

        // Set preedit from composition
        if !self.composition.preedit.is_empty() {
            context.preedit_text = self.composition.preedit.clone();
            context.preedit_cursor = self.composition.cursor;
            context.preedit_segments = self.composition.segments.clone();
        }

        // Set candidates
        let page_candidates = self.candidates.current_page_candidates();
        context.candidates = page_candidates.iter().map(|c| c.text.clone()).collect();
        context.candidate_cursor = self.candidates.cursor();

        // Page indicator when multi-page (1-indexed for display)
        if self.candidates.num_pages() > 1 {
            let current_page = self.candidates.current_page() + 1;
            let total_pages = self.candidates.num_pages();
            context.page_text = format!("{current_page}/{total_pages}");
        }
    }

    /// Update session state from a host [`Context`] (rarely needed).
    pub fn sync_from_context(&mut self, context: &Context) {
        // Update composition
        if !context.preedit_text.is_empty() {
            self.composition =
                Composition::with_cursor(context.preedit_text.clone(), context.preedit_cursor);
        }
    }
}

// ========== Input buffer ==========

/// Input buffer tracking raw input and cursor position.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InputBuffer {
    text: String,
    cursor: usize, // Byte offset, not char offset
}

impl InputBuffer {
    /// Get the raw input text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get the cursor position (byte offset).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Get the length of the buffer in bytes.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Clear the buffer and reset cursor.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    /// Insert a string at the cursor position.
    pub fn insert_str(&mut self, s: &str) {
        self.text.insert_str(self.cursor, s);
        self.cursor += s.len();
    }

    /// Delete the character before the cursor (backspace).
    /// Returns true if a character was deleted.
    pub fn delete_before(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }

        // Find the previous character boundary
        let mut prev = self.cursor;
        while prev > 0 {
            prev -= 1;
            if self.text.is_char_boundary(prev) {
                break;
            }
        }

        self.text.remove(prev);
        self.cursor = prev;
        true
    }

    /// Delete the character after the cursor (delete key).
    /// Returns true if a character was deleted.
    pub fn delete_after(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }

        // Remove character at cursor
        if self.text.is_char_boundary(self.cursor) {
            self.text.remove(self.cursor);
            true
        } else {
            false
        }
    }

    /// Move cursor to the left by one character.
    /// Returns true if cursor moved.
    pub fn move_left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }

        // Find the previous character boundary
        let mut prev = self.cursor;
        while prev > 0 {
            prev -= 1;
            if self.text.is_char_boundary(prev) {
                self.cursor = prev;
                return true;
            }
        }
        false
    }

    /// Move cursor to the right by one character.
    /// Returns true if cursor moved.
    pub fn move_right(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }

        // Find the next character boundary
        let mut next = self.cursor + 1;
        while next < self.text.len() && !self.text.is_char_boundary(next) {
            next += 1;
        }
        if next <= self.text.len() {
            self.cursor = next;
            true
        } else {
            false
        }
    }

    /// Move cursor to the beginning.
    pub fn move_to_start(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to the end.
    pub fn move_to_end(&mut self) {
        self.cursor = self.text.len();
    }

    /// Set the cursor position (must be at a character boundary).
    pub fn set_cursor(&mut self, pos: usize) -> bool {
        if pos <= self.text.len() && self.text.is_char_boundary(pos) {
            self.cursor = pos;
            true
        } else {
            false
        }
    }

    /// Remove the first `n` bytes from the buffer.
    /// Adjusts cursor position accordingly.
    pub fn remove_front(&mut self, n: usize) {
        let n = n.min(self.text.len());
        self.text.drain(..n);
        self.cursor = self.cursor.saturating_sub(n);
    }
}

// ========== Preedit composition ==========

/// A segment in the preedit composition.
///
/// Segments mark boundaries between different parts of the composition,
/// such as individual words or syllables that can be converted independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Byte range in the preedit text
    pub range: Range<usize>,
    /// Whether this segment has been confirmed by the user
    pub confirmed: bool,
}

/// Preedit composition for display.
///
/// This represents the visual text shown to the user during input composition,
/// along with cursor position and segment boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Composition {
    /// The preedit text to display (e.g., "你好")
    pub preedit: String,
    /// Cursor position in the preedit (byte offset)
    pub cursor: usize,
    /// Segments marking conversion boundaries
    pub segments: Vec<Segment>,
}

impl Composition {
    /// Create a composition with simple text (no segments).
    pub fn from_text(text: String) -> Self {
        let cursor = text.len();
        Self {
            preedit: text,
            cursor,
            segments: Vec::new(),
        }
    }

    /// Create a composition with text and cursor position.
    pub fn with_cursor(text: String, cursor: usize) -> Self {
        Self {
            preedit: text,
            cursor,
            segments: Vec::new(),
        }
    }

    /// Check if the composition is empty.
    pub fn is_empty(&self) -> bool {
        self.preedit.is_empty()
    }

    /// Clear the composition.
    pub fn clear(&mut self) {
        self.preedit.clear();
        self.cursor = 0;
        self.segments.clear();
    }

    /// Get the length of the preedit text in bytes.
    pub fn len(&self) -> usize {
        self.preedit.len()
    }

    pub fn set_text(&mut self, text: String) {
        self.preedit = text;
        self.cursor = self.preedit.len();
        self.segments.clear();
    }

    pub fn set_syllable_segments_from_preedit(&mut self) {
        self.segments.clear();
        if self.preedit.is_empty() {
            return;
        }
        let mut ranges = Vec::new();
        let mut start = 0;
        for (i, ch) in self.preedit.char_indices() {
            if ch == '\'' {
                if start < i {
                    ranges.push(start..i);
                }
                start = i + ch.len_utf8();
            }
        }
        if start < self.preedit.len() {
            ranges.push(start..self.preedit.len());
        }
        for range in ranges {
            self.add_segment(range, false);
        }
    }

    /// Set the cursor position.
    pub fn set_cursor(&mut self, cursor: usize) {
        if cursor <= self.preedit.len() {
            self.cursor = cursor;
        }
    }

    /// Add a segment.
    pub fn add_segment(&mut self, range: Range<usize>, confirmed: bool) {
        self.segments.push(Segment { range, confirmed });
    }

    /// Get the segment at the cursor position, if any.
    pub fn segment_at_cursor(&self) -> Option<&Segment> {
        self.segments
            .iter()
            .find(|seg| seg.range.contains(&self.cursor))
    }

    /// Get mutable reference to segment at cursor.
    pub fn segment_at_cursor_mut(&mut self) -> Option<&mut Segment> {
        let cursor = self.cursor;
        self.segments
            .iter_mut()
            .find(|seg| seg.range.contains(&cursor))
    }

    /// Confirm all segments.
    pub fn confirm_all(&mut self) {
        for seg in &mut self.segments {
            seg.confirmed = true;
        }
    }

    /// Get text of a specific segment.
    pub fn segment_text(&self, segment: &Segment) -> &str {
        &self.preedit[segment.range.clone()]
    }
}

// ========== Candidates ==========

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
