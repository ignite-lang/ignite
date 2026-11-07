//! Diagnostics and source positions shared by every compiler stage.

use std::fmt;
use std::sync::OnceLock;

use crate::ast::{FileId, Span};

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

/// A source file with its text, for rendering positions. Two files are
/// equal when their paths and texts are.
#[derive(Clone, Debug)]
pub struct SourceFile {
    pub path: String,
    pub text: String,
    /// The byte offset at which each line of `text` starts, computed on
    /// the first position lookup and kept: a failure site per expression
    /// of a 100,000-term program asks 100,000 times.
    lines: OnceLock<Vec<usize>>,
    /// [`SourceFile::id`]: [`FileId::of_path`] of `path`, or of the
    /// identity given to [`SourceFile::with_identity`].
    id: FileId,
}

impl PartialEq for SourceFile {
    fn eq(&self, other: &SourceFile) -> bool {
        self.path == other.path && self.text == other.text
    }
}

impl Eq for SourceFile {}

impl SourceFile {
    pub fn new(path: impl Into<String>, text: impl Into<String>) -> SourceFile {
        let path = path.into();
        SourceFile {
            id: FileId::of_path(&path),
            path,
            text: text.into(),
            lines: OnceLock::new(),
        }
    }

    /// The file with its id taken from `identity` instead of `path`: a
    /// driver passes the canonical path of a file on disk, so `x/a.ignite`,
    /// `./x/a.ignite` and the absolute path give one id while diagnostics
    /// still print `path` as given.
    pub fn with_identity(mut self, identity: &str) -> SourceFile {
        self.id = FileId::of_path(identity);
        self
    }

    /// The id of the file, carried by the spans and node ids of its parse:
    /// [`FileId::of_path`] of `path`, or of the identity given to
    /// [`SourceFile::with_identity`].
    pub fn id(&self) -> FileId {
        self.id
    }

    /// The line table, computed once per file.
    fn line_starts(&self) -> &[usize] {
        self.lines.get_or_init(|| line_starts(&self.text))
    }

    /// 1-based line and column of a byte offset. Column counts characters.
    /// Linear in the line's length after the first call on a file.
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        self.line_col_in(self.line_starts(), offset)
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
        let starts = self.line_starts();
        let mut sorted: Vec<&Diagnostic> = diagnostics.iter().collect();
        sorted.sort_by_key(|d| (d.span.start, d.code));
        sorted
            .iter()
            .map(|d| {
                let (line, col) = self.line_col_in(starts, d.span.start);
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
