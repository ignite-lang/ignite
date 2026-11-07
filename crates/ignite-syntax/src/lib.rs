//! Ignite front end: lexer, parser, AST and diagnostics for the slice defined
//! in `docs/slice.md` sections 2 and 3. That file is the grammar; this crate
//! does not restate it.
//!
//! Contract:
//!
//! * [`parse`] turns one source file into an [`ast::Program`] or a list of
//!   diagnostics with code `E001` (syntax) or `E002` (unexpected end of
//!   file). After a syntax error the parser skips to the next top-level
//!   keyword (`package`, `import`, `type`, `enum`, `fn`, `const`, `system`,
//!   `protocol`, `test`) at the start of a line and continues, so one run reports
//!   several errors. It never reports the same position twice.
//! * Spans are byte offsets into the source text and carry the file's
//!   [`ast::FileId`] ([`SourceFile::id`]). A node's span covers its first
//!   to last token.
//! * Every [`ast::Expr`], [`ast::Pattern`], [`ast::Type`], [`ast::Path`]
//!   and [`ast::Place`] has an [`ast::NodeId`]: the file and the node's
//!   number in the parse, distinct within a file and kept by a clone.
//!   Later stages key their side tables by it, never by an address.
//! * The lexer emits one `Newline` token for a run of line breaks, and no
//!   token for whitespace or comments.
//! * `Ident::is_uppercase` decides record literal versus block after an
//!   identifier, and variant versus binding in patterns.

pub mod ast;
pub mod diagnostic;
pub mod lexer;
pub mod parser;

pub use ast::Program;
pub use diagnostic::{Diagnostic, SourceFile};

/// Lex and parse one file. After a lexer error the parser still runs on
/// the tokens before it, so syntax errors earlier in the file are reported
/// too; parser diagnostics at or after the lexer error are dropped (they
/// only see the cut-off end).
pub fn parse(source: &SourceFile) -> Result<Program, Vec<Diagnostic>> {
    let file = source.id();
    let (mut tokens, lex_error) = lexer::lex_partial(&source.text);
    lexer::in_file(&mut tokens, file);
    let parsed = parser::parse_owned(tokens, parser::ParseOptions::default());
    let Some(mut lex_error) = lex_error else {
        return parsed;
    };
    lex_error.span.file = file;
    let mut diagnostics: Vec<Diagnostic> = parsed
        .err()
        .unwrap_or_default()
        .into_iter()
        .filter(|d| d.span.start < lex_error.span.start)
        .collect();
    diagnostics.push(lex_error);
    diagnostics.sort_by_key(|d| d.span.start);
    Err(diagnostics)
}
