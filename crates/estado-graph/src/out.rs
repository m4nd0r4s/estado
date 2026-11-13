//! A line-oriented text builder with indentation.

use alloc::string::String;

/// Builds text one line at a time; every line ends with `\n`.
pub(crate) struct Lines {
    text: String,
    indent: usize,
    unit: &'static str,
}

impl Lines {
    /// An empty builder indenting nested blocks with `unit`.
    pub(crate) fn new(unit: &'static str) -> Self {
        Self {
            text: String::new(),
            indent: 0,
            unit,
        }
    }

    /// Appends `text` as one line at the current depth.
    pub(crate) fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.text.push_str(self.unit);
        }
        self.text.push_str(text);
        self.text.push('\n');
    }

    /// Appends one line, built from pieces.
    pub(crate) fn line_of(&mut self, pieces: &[&str]) {
        for _ in 0..self.indent {
            self.text.push_str(self.unit);
        }
        for piece in pieces {
            self.text.push_str(piece);
        }
        self.text.push('\n');
    }

    /// Goes one level deeper.
    pub(crate) fn open(&mut self) {
        self.indent += 1;
    }

    /// Goes one level back.
    pub(crate) fn close(&mut self) {
        self.indent -= 1;
    }

    /// The finished text.
    pub(crate) fn finish(self) -> String {
        self.text
    }
}
