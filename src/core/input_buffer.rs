//! Input buffer with cursor tracking for IME input.
//!
//! The input buffer stores the raw input characters (e.g., "nihao") and tracks
//! the cursor position within this buffer. This is separate from the visual
//! preedit composition which shows converted text.

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
