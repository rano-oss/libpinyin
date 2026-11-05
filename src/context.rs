//! Platform-facing session snapshot after each key.
//!
//! [`Context`] is a plain data bag: after [`crate::Editor::process_key`], the
//! host reads these fields to update preedit, candidates, and commit text.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputPurpose {
    #[default]
    FreeForm,
    Email,
    Url,
    Password,
    Number,
    Phone,
    Terminal,
}

impl InputPurpose {
    pub fn is_private(self) -> bool {
        matches!(self, Self::Password)
    }

    pub fn disables_learning(self) -> bool {
        matches!(self, Self::Password | Self::Terminal)
    }

    pub fn prefers_english(self) -> bool {
        matches!(self, Self::Email | Self::Url)
    }
}

/// Snapshot of preedit / candidates / commit for the host UI.
#[derive(Debug, Clone, Default)]
pub struct Context {
    /// Text being composed (preedit).
    pub preedit_text: String,
    /// Cursor within preedit (byte offset).
    pub preedit_cursor: usize,
    /// Text to commit (host should consume and clear).
    pub commit_text: String,
    /// Candidate strings for the current page / list.
    pub candidates: Vec<String>,
    /// Highlighted candidate index (0-based).
    pub candidate_cursor: usize,
    /// Mode hints (tone filter, punctuation, …).
    pub auxiliary_text: String,
    /// Page chrome (e.g. `"2/5"`), separate from [`Self::auxiliary_text`].
    pub page_text: String,
    pub preedit_segments: Vec<crate::session::Segment>,
    pub input_purpose: InputPurpose,
}

impl Context {
    /// Clear preedit/candidates/auxiliary. Does not clear `commit_text`.
    pub fn clear(&mut self) {
        self.preedit_text.clear();
        self.preedit_cursor = 0;
        self.candidates.clear();
        self.candidate_cursor = 0;
        self.auxiliary_text.clear();
        self.page_text.clear();
        self.preedit_segments.clear();
    }

    pub fn take_commit(&mut self) -> String {
        std::mem::take(&mut self.commit_text)
    }

    pub fn has_visible_state(&self) -> bool {
        !self.preedit_text.is_empty() || !self.candidates.is_empty()
    }

    pub fn has_commit(&self) -> bool {
        !self.commit_text.is_empty()
    }
}
