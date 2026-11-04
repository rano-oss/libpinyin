//! Preedit composition with segments for display.
//!
//! The composition represents the visual display of the input being composed,
//! including the converted text and segment boundaries. For example, typing
//! "nihao" might show "你好" as the preedit, with segments marking each word.

use std::ops::Range;

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
