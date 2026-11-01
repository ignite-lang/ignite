//! Diagnostics and source positions shared by every compiler stage.

use std::fmt;

use crate::ast::Span;

/// One error. `code` is a stable identifier from `docs/slice.md` section 8.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn new(code: &'static str, span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            code,
            message: message.into(),
            span,
        }
    }
}

/// A source file with its text, for rendering positions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    pub path: String,
    pub text: String,
}

impl SourceFile {
    pub fn new(path: impl Into<String>, text: impl Into<String>) -> SourceFile {
        SourceFile {
            path: path.into(),
            text: text.into(),
        }
    }

    /// 1-based line and column of a byte offset. Column counts characters.
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let starts = line_starts(&self.text);
        self.line_col_in(&starts, offset)
    }

    /// [`SourceFile::line_col`] with the line starts computed once.
    fn line_col_in(&self, starts: &[usize], offset: usize) -> (usize, usize) {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = starts.partition_point(|&s| s <= offset);
        let line_start = starts[line - 1];
        let col = self.text[line_start..offset].chars().count() + 1;
        (line, col)
    }

    /// `<path>:<line>:<col>: error[<code>]: <message>`
    pub fn render(&self, diagnostic: &Diagnostic) -> String {
        let (line, col) = self.line_col(diagnostic.span.start);
        self.format(diagnostic, line, col)
    }

    fn format(&self, diagnostic: &Diagnostic, line: usize, col: usize) -> String {
        format!(
            "{}:{}:{}: error[{}]: {}",
            self.path, line, col, diagnostic.code, diagnostic.message
        )
    }

    /// Render all, sorted by position, one per line. Linear in the text
    /// and the number of diagnostics (up to the sort).
    pub fn render_all(&self, diagnostics: &[Diagnostic]) -> String {
        let starts = line_starts(&self.text);
        let mut sorted: Vec<&Diagnostic> = diagnostics.iter().collect();
        sorted.sort_by_key(|d| (d.span.start, d.code));
        sorted
            .iter()
            .map(|d| {
                let (line, col) = self.line_col_in(&starts, d.span.start);
                self.format(d, line, col)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// The byte offset at which each line starts; the first is 0.
fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            text.bytes()
                .enumerate()
                .filter(|&(_, b)| b == b'\n')
                .map(|(i, _)| i + 1),
        )
        .collect()
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error[{}]: {}", self.code, self.message)
    }
}

impl std::error::Error for Diagnostic {}
