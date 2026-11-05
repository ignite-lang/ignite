//! Recursive-descent parser for the grammar in `docs/slice.md` section 3.
//!
//! Newlines: the lexer always emits `Newline` tokens. The parser keeps a
//! stack of newline modes. Inside `( )` and `[ ]` newlines are insignificant
//! and [`Parser::peek`] skips them; inside `{ }` and at the top level they
//! are significant and only explicit [`Parser::skip_newlines`] calls (after a
//! binary operator, `=`, `=>`, `->`, `,`, `{`, and before `}`) pass over them.
//! A postfix `(`, `[`, `.` or `?` applies only when no `Newline` token lies
//! between it and its operand, and `else` only when it is on the line of
//! the closing `}`; both look at the raw next token, so the rule holds
//! inside `( )` and `[ ]` too.
//!
//! Nesting: brackets, blocks, type arguments and prefix operators count
//! one level each; past [`MAX_NESTING`] the parser reports E054 and stops
//! the declaration, so no input overflows its stack. `>=` is split into
//! `>` `=` before parsing ([`split_greater_equal`]) so a type may end in
//! `>` directly before `=`.
//!
//! Record literals: as in Rust, `Name { .. }` is not a record literal in the
//! condition of `if` and `while`, the scrutinee of `match` and the iterable
//! of `for` (`docs/slice.md` section 3). [`Parser::restricted_expr`] sets
//! `no_record` for such an expression; every `(`, `[` and `{` opened inside
//! it lifts the restriction until its closing token.
//!
//! Desugaring (`docs/slice.md` section 10.1): expression bodies, field
//! punning, interpolation (a [`TokenKind::Template`] part is parsed by a
//! sub-parser over its tokens), byte strings and `await` produce existing
//! AST forms. `let`/`var` followed by `(`, `[`, a literal, `_` or a
//! variant path parses a pattern ([`Parser::let_pattern_follows`]), which
//! must be irrefutable. `return` in expression position has no value when a
//! newline, `;`, `,` or closing bracket follows.
//!
//! Errors: every parse function returns [`Fail`] after recording one
//! diagnostic. The failure propagates to the declaration loop, which skips
//! to the next top-level keyword at the start of a line.

use crate::ast::{
    AliasDecl, Arm, AssemblyDecl, BinaryOp, Block, Bound, BraceEntry, Closure, ClosureParam,
    ComponentDecl, ConstDecl, Decl, EnumDecl, Expr, ExprKind, Field, FieldInit, FnDecl, Ident,
    Import, Param, Path, Pattern, PatternKind, Place, Program, ProtocolDecl, RecordDecl,
    ResourceDecl, Scope, Span, StatusDecl, StatusKind, Stmt, StmtKind, SystemDecl, TestDecl, Type,
    UnaryOp, Variant,
};
use crate::diagnostic::Diagnostic;
use crate::lexer::{TemplatePart, Token, TokenKind};

/// Whether `await e` is a syntax error (`docs/slice.md` section 10.2). It
/// is, since the standard library and the examples stopped using it;
/// `ParseOptions { await_is_error: false }` still parses `await e` as `e`.
pub const AWAIT_IS_ERROR: bool = true;

/// Choices that change what the parser accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseOptions {
    /// `await e` is E001 when `true`, else it parses as `e`.
    pub await_is_error: bool,
}

impl Default for ParseOptions {
    fn default() -> ParseOptions {
        ParseOptions {
            await_is_error: AWAIT_IS_ERROR,
        }
    }
}

/// Parse a token stream produced by [`crate::lexer::lex`] with the default
/// [`ParseOptions`].
pub fn parse(tokens: &[Token]) -> Result<Program, Vec<Diagnostic>> {
    parse_with(tokens, ParseOptions::default())
}

/// Parse a token stream with explicit options.
pub fn parse_with(tokens: &[Token], options: ParseOptions) -> Result<Program, Vec<Diagnostic>> {
    parse_owned(tokens.to_vec(), options)
}

/// Parse tokens whose storage belongs to the caller without copying their text.
pub(crate) fn parse_owned(
    tokens: Vec<Token>,
    options: ParseOptions,
) -> Result<Program, Vec<Diagnostic>> {
    let mut tokens = split_greater_equal_owned(tokens);
    if tokens.last().is_none_or(|t| t.kind != TokenKind::Eof) {
        let end = tokens.last().map_or(0, |t| t.span.end);
        tokens.push(Token {
            kind: TokenKind::Eof,
            span: Span::new(end, end),
        });
    }
    let mut parser = Parser::new(&tokens, options, 0);
    let program = parser.program();
    if parser.diagnostics.is_empty() {
        Ok(program)
    } else {
        let mut diagnostics = parser.diagnostics;
        diagnostics.sort_by_key(|d| d.span.start);
        Err(diagnostics)
    }
}

/// The most brackets, blocks and prefix operators an expression or type may
/// nest (`docs/slice.md` section 3); one more is E054.
pub const MAX_NESTING: usize = 256;

/// `tokens` with every `>=` split into `>` and `=` with adjacent spans, so
/// that a type's closing `>` can be followed by `=` (`x: Option<Int>= None`,
/// `type B<T>= T`). [`Parser::compare`] joins an adjacent `>` `=` back into
/// `>=`.
fn split_greater_equal(tokens: &[Token]) -> Vec<Token> {
    split_greater_equal_owned(tokens.to_vec())
}

fn split_greater_equal_owned(tokens: Vec<Token>) -> Vec<Token> {
    if !tokens.iter().any(|token| token.kind == TokenKind::GtEq) {
        return tokens;
    }
    let mut out = Vec::with_capacity(tokens.len());
    for token in tokens {
        if token.kind == TokenKind::GtEq {
            let mid = token.span.start + 1;
            out.push(Token {
                kind: TokenKind::Gt,
                span: Span::new(token.span.start, mid),
            });
            out.push(Token {
                kind: TokenKind::Assign,
                span: Span::new(mid, token.span.end),
            });
        } else {
            out.push(token);
        }
    }
    out
}

/// A syntax error has been recorded; unwind to the declaration loop.
#[derive(Debug)]
struct Fail;

type PResult<T> = Result<T, Fail>;

struct Parser<'t> {
    tokens: &'t [Token],
    pos: usize,
    prev_end: usize,
    /// Current newline mode, read for every token peek.
    insignificant: bool,
    /// Enclosing newline modes, restored when a bracket closes.
    modes: Vec<bool>,
    /// `true` while parsing an `if`/`while` condition, `match` scrutinee or
    /// `for` iterable outside any bracket: `Name {` is then not a record.
    no_record: bool,
    /// `no_record` of each enclosing open bracket, restored when it closes.
    saved_no_record: Vec<bool>,
    diagnostics: Vec<Diagnostic>,
    /// Start offsets of the recorded diagnostics: one per position.
    reported: std::collections::HashSet<usize>,
    options: ParseOptions,
    /// Open brackets, blocks and prefix operators around the next token.
    depth: usize,
}

fn is_top_level_keyword(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::KwPackage
            | TokenKind::KwImport
            | TokenKind::KwType
            | TokenKind::KwEnum
            | TokenKind::KwFn
            | TokenKind::KwSystem
            | TokenKind::KwProtocol
            | TokenKind::KwConst
            | TokenKind::KwTest
            | TokenKind::KwAssembly
    )
}

impl<'t> Parser<'t> {
    fn new(tokens: &'t [Token], options: ParseOptions, depth: usize) -> Parser<'t> {
        Parser {
            tokens,
            pos: 0,
            prev_end: 0,
            insignificant: false,
            modes: Vec::new(),
            no_record: false,
            saved_no_record: Vec::new(),
            diagnostics: Vec::new(),
            reported: std::collections::HashSet::new(),
            options,
            depth,
        }
    }

    /// One more level of nesting, opened at `span`: E054 past
    /// [`MAX_NESTING`].
    fn enter(&mut self, span: Span) -> PResult<()> {
        self.depth += 1;
        if self.depth > MAX_NESTING {
            self.report(
                "E054",
                span,
                format!("nesting deeper than {MAX_NESTING}; split the expression"),
            );
            return Err(Fail);
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// Whether the raw next token is a line break: a postfix operator or
    /// `else` does not attach across it, inside `( )` and `[ ]` too.
    fn at_line_break(&self) -> bool {
        matches!(self.tokens[self.pos].kind, TokenKind::Newline)
    }

    fn newlines_insignificant(&self) -> bool {
        self.insignificant
    }

    fn peek_index(&self) -> usize {
        let mut i = self.pos;
        if self.newlines_insignificant() {
            while matches!(self.tokens[i].kind, TokenKind::Newline) {
                i += 1;
            }
        }
        i
    }

    fn peek_token(&self) -> &'t Token {
        &self.tokens[self.peek_index()]
    }

    fn peek(&self) -> &'t TokenKind {
        &self.peek_token().kind
    }

    /// The token after the next one, by the same newline rule.
    fn peek_second(&self) -> &'t TokenKind {
        self.peek_nth(1)
    }

    /// After `let`/`var`: whether a pattern follows rather than a name, so
    /// that `let (a, b)`, `let _` and the refutable `let Some(x)` (an error)
    /// take the pattern path.
    fn let_pattern_follows(&self) -> bool {
        match self.peek_second() {
            TokenKind::LParen | TokenKind::LBracket | TokenKind::Int(_) | TokenKind::Str(_) => true,
            TokenKind::KwTrue | TokenKind::KwFalse => true,
            TokenKind::Ident(name) if name == "_" => true,
            TokenKind::Ident(_) => matches!(
                self.peek_nth(2),
                TokenKind::LParen | TokenKind::Dot | TokenKind::Pipe
            ),
            _ => false,
        }
    }

    /// The token `n` places after the next one, by the same newline rule.
    fn peek_nth(&self, n: usize) -> &'t TokenKind {
        let mut i = self.peek_index();
        for _ in 0..n {
            if self.tokens[i].kind == TokenKind::Eof {
                break;
            }
            i += 1;
            if self.newlines_insignificant() {
                while matches!(self.tokens[i].kind, TokenKind::Newline) {
                    i += 1;
                }
            }
        }
        &self.tokens[i].kind
    }

    fn at(&self, kind: &TokenKind) -> bool {
        self.peek() == kind
    }

    fn start(&self) -> usize {
        self.peek_token().span.start
    }

    fn span_from(&self, start: usize) -> Span {
        Span::new(start, self.prev_end.max(start))
    }

    fn bump(&mut self) -> &'t Token {
        let i = self.peek_index();
        let token = &self.tokens[i];
        if token.kind != TokenKind::Eof {
            self.pos = i + 1;
            self.prev_end = token.span.end;
        }
        token
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.tokens[self.pos].kind, TokenKind::Newline) {
            self.pos += 1;
        }
    }

    fn report(&mut self, code: &'static str, span: Span, message: String) {
        if self.reported.insert(span.start) {
            self.diagnostics.push(Diagnostic::new(code, span, message));
        }
    }

    /// Record "expected X" at the next token: `E002` at end of file, `E001`
    /// otherwise.
    fn expected(&mut self, what: &str) -> Fail {
        let token = self.peek_token();
        if token.kind == TokenKind::Eof {
            self.report(
                "E002",
                token.span,
                format!("unexpected end of file, expected {what}"),
            );
        } else {
            self.report(
                "E001",
                token.span,
                format!("expected {what}, found {}", token.kind.describe()),
            );
        }
        Fail
    }

    fn expect(&mut self, kind: &TokenKind) -> PResult<&'t Token> {
        if self.at(kind) {
            Ok(self.bump())
        } else {
            let what = format!("`{}`", kind.symbol().unwrap_or("?"));
            Err(self.expected(&what))
        }
    }

    /// `true` at `}`; at end of file records E002 "expected `}`" and fails.
    fn at_close_brace(&mut self) -> PResult<bool> {
        match self.peek() {
            TokenKind::RBrace => Ok(true),
            TokenKind::Eof => Err(self.expected("`}`")),
            _ => Ok(false),
        }
    }

    fn ident(&mut self) -> PResult<Ident> {
        let token = self.peek_token();
        match &token.kind {
            TokenKind::Ident(name) => {
                self.bump();
                Ok(Ident {
                    name: name.clone(),
                    span: token.span,
                })
            }
            _ => Err(self.expected("identifier")),
        }
    }

    /// A segment of a `package`, `import` or effect path, or the name in a
    /// property or record-literal field `name = value`: an identifier or a
    /// keyword, so `std.protocol.echo` is a path and `protocol = Echo` a
    /// property although `protocol` is a keyword.
    fn name_or_keyword(&mut self) -> PResult<Ident> {
        let token = self.peek_token();
        let keyword = token
            .kind
            .symbol()
            .filter(|s| s.bytes().all(|b| b.is_ascii_alphabetic()));
        match keyword {
            Some(word) => {
                self.bump();
                Ok(Ident {
                    name: word.to_string(),
                    span: token.span,
                })
            }
            None => self.ident(),
        }
    }

    fn open(&mut self, kind: &TokenKind, insignificant: bool) -> PResult<&'t Token> {
        let token = self.expect(kind)?;
        self.enter(token.span)?;
        self.modes.push(self.insignificant);
        self.insignificant = insignificant;
        self.saved_no_record.push(self.no_record);
        self.no_record = false;
        Ok(token)
    }

    fn close(&mut self, kind: &TokenKind) -> PResult<&'t Token> {
        let token = self.expect(kind)?;
        self.leave();
        self.insignificant = self.modes.pop().unwrap_or(false);
        self.no_record = self.saved_no_record.pop().unwrap_or(false);
        Ok(token)
    }

    /// Forget every open bracket after an error.
    fn reset_nesting(&mut self) {
        self.modes.clear();
        self.insignificant = false;
        self.saved_no_record.clear();
        self.no_record = false;
        self.depth = 0;
    }

    /// After a declaration keyword inside a block: skip to the end of the
    /// outermost open bracket, which closes the enclosing top-level
    /// declaration, so the nested declaration produces no second error.
    fn skip_enclosing_declaration(&mut self) {
        let mut depth = self.modes.len();
        while depth > 0 {
            let token = &self.tokens[self.pos];
            match token.kind {
                TokenKind::Eof => break,
                TokenKind::LBrace | TokenKind::LParen | TokenKind::LBracket => depth += 1,
                TokenKind::RBrace | TokenKind::RParen | TokenKind::RBracket => depth -= 1,
                _ => {}
            }
            self.pos += 1;
            self.prev_end = token.span.end;
        }
        self.reset_nesting();
    }

    /// An expression in which `Name {` is not a record literal: the
    /// condition of `if`/`while`, the scrutinee of `match`, the iterable of
    /// `for`.
    fn restricted_expr(&mut self) -> PResult<Expr> {
        let saved = self.no_record;
        self.no_record = true;
        let expr = self.expr();
        self.no_record = saved;
        expr
    }

    /// Skip to the next top-level keyword that is the first token or directly
    /// follows a `Newline` token.
    fn recover(&mut self) {
        self.reset_nesting();
        loop {
            let token = &self.tokens[self.pos];
            if token.kind == TokenKind::Eof {
                return;
            }
            let line_start = self.pos == 0 || self.tokens[self.pos - 1].kind == TokenKind::Newline;
            if line_start && is_top_level_keyword(&token.kind) {
                return;
            }
            self.pos += 1;
        }
    }

    /// Move past the `}` matching the `{` at token index `open`, or to end of
    /// file, so recovery after an error inside a protocol does not resume at
    /// the protocol's own `fn` lines.
    fn skip_past_matching_brace(&mut self, open: usize) {
        self.reset_nesting();
        let mut depth = 0usize;
        for i in open..self.tokens.len() {
            match self.tokens[i].kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        self.pos = i + 1;
                        self.prev_end = self.tokens[i].span.end;
                        return;
                    }
                }
                TokenKind::Eof => {
                    self.pos = i;
                    return;
                }
                _ => {}
            }
        }
    }

    // program := NL* decl (NL+ decl)* NL*

    fn program(&mut self) -> Program {
        let mut program = Program::default();
        self.skip_newlines();
        while !self.at(&TokenKind::Eof) {
            match self.decl(&mut program) {
                Ok(()) => match self.peek() {
                    TokenKind::Newline => self.skip_newlines(),
                    TokenKind::Eof => {}
                    _ => {
                        self.expected("newline after declaration");
                        self.recover();
                    }
                },
                Err(Fail) => self.recover(),
            }
        }
        program
    }

    fn decl(&mut self, program: &mut Program) -> PResult<()> {
        match self.peek() {
            TokenKind::KwPackage => {
                let keyword = self.bump().span;
                let path = self.path()?;
                if program.package.is_some() {
                    self.report(
                        "E001",
                        keyword,
                        "a file has at most one `package` declaration".to_string(),
                    );
                    return Err(Fail);
                }
                program.package = Some(path);
            }
            TokenKind::KwImport => {
                let import = self.import()?;
                program.imports.push(import);
            }
            TokenKind::KwType => {
                let decl = self.type_decl()?;
                program.decls.push(decl);
            }
            TokenKind::KwEnum => {
                let decl = self.enum_decl()?;
                program.decls.push(Decl::Enum(decl));
            }
            TokenKind::KwFn => {
                let decl = self.fn_decl()?;
                program.decls.push(Decl::Fn(decl));
            }
            TokenKind::KwSystem => {
                let decl = self.system()?;
                program.decls.push(Decl::System(decl));
            }
            TokenKind::KwProtocol => {
                let decl = self.protocol()?;
                program.decls.push(Decl::Protocol(decl));
            }
            TokenKind::KwConst => {
                let decl = self.const_decl()?;
                program.decls.push(Decl::Const(decl));
            }
            TokenKind::KwTest => {
                let decl = self.test_decl()?;
                program.decls.push(decl);
            }
            TokenKind::KwAssembly => {
                let (name, kind, _, properties, span) = self.system_member(false)?;
                program.decls.push(Decl::Assembly(AssemblyDecl {
                    name,
                    kind,
                    properties,
                    span,
                }));
            }
            _ => {
                return Err(self.expected(
                    "a declaration (`package`, `import`, `type`, `enum`, `fn`, `const`, `protocol`, `system`, `assembly` or `test`)",
                ));
            }
        }
        Ok(())
    }

    // test := 'test' STRING block | 'test' (fn | record | enum | const)

    /// A test, or a test-only fn, record, enum or const; its span starts at
    /// `test`. An empty or interpolated name is E053, an alias E001.
    fn test_decl(&mut self) -> PResult<Decl> {
        let keyword = self.expect(&TokenKind::KwTest)?.span;
        let widen = |span: Span| Span::new(keyword.start, span.end);
        match self.peek() {
            TokenKind::KwFn => {
                let mut decl = self.fn_decl()?;
                decl.test_only = true;
                decl.span = widen(decl.span);
                Ok(Decl::Fn(decl))
            }
            TokenKind::KwEnum => {
                let mut decl = self.enum_decl()?;
                decl.test_only = true;
                decl.span = widen(decl.span);
                Ok(Decl::Enum(decl))
            }
            TokenKind::KwConst => {
                let mut decl = self.const_decl()?;
                decl.test_only = true;
                decl.span = widen(decl.span);
                Ok(Decl::Const(decl))
            }
            TokenKind::KwType => match self.type_decl()? {
                Decl::Record(mut decl) => {
                    decl.test_only = true;
                    decl.span = widen(decl.span);
                    Ok(Decl::Record(decl))
                }
                other => {
                    self.report(
                        "E001",
                        keyword,
                        format!(
                            "type alias `{}` cannot be test-only; remove `test`",
                            other.name().name
                        ),
                    );
                    Err(Fail)
                }
            },
            TokenKind::Str(text) => {
                let span = self.bump().span;
                if text.is_empty() {
                    self.report(
                        "E053",
                        span,
                        "a test name cannot be empty; describe what the test checks".to_string(),
                    );
                }
                self.test_rest(keyword, text.clone(), span)
            }
            TokenKind::Template(_) => {
                let span = self.bump().span;
                self.report(
                    "E053",
                    span,
                    "a test name cannot be interpolated; write a plain string literal".to_string(),
                );
                self.test_rest(keyword, String::new(), span)
            }
            _ => Err(self.expected(
                "a test name (a string literal), or `fn`, `type`, `enum` or `const` after `test`",
            )),
        }
    }

    /// A test after its name: `verifies? block` (`docs/slice.md` 19.5).
    fn test_rest(&mut self, keyword: Span, name: String, span: Span) -> PResult<Decl> {
        let verifies = if self.at_word("verifies") {
            self.clause_refs("verifies")?
        } else {
            Vec::new()
        };
        let body = self.block()?;
        if self.at_word("verifies") {
            let span = self.peek_token().span;
            self.report(
                "E001",
                span,
                "`verifies` goes between the test's name and its block".to_string(),
            );
            return Err(Fail);
        }
        Ok(Decl::Test(TestDecl {
            name: Ident { name, span },
            body,
            span: self.span_from(keyword.start),
            verifies,
        }))
    }

    /// Whether the next token is the identifier `word`: the clause words
    /// of `docs/slice.md` 19.5 are identifiers, not keywords.
    fn at_word(&self, word: &str) -> bool {
        matches!(self.peek(), TokenKind::Ident(name) if name == word)
    }

    // satisfies := 'satisfies' '[' reqref (',' reqref)* ','? ']'
    // verifies  := 'verifies' '[' reqref (',' reqref)* ','? ']'

    /// The clause `word [Spec.Id, ..]` at the clause word.
    fn clause_refs(&mut self, word: &str) -> PResult<Vec<Path>> {
        self.bump();
        if !self.at(&TokenKind::LBracket) {
            return Err(self.expected(&format!("`[` after `{word}`")));
        }
        self.open(&TokenKind::LBracket, true)?;
        let mut refs = vec![self.reqref()?];
        while self.eat(&TokenKind::Comma) {
            if self.at(&TokenKind::RBracket) {
                break;
            }
            refs.push(self.reqref()?);
        }
        self.close(&TokenKind::RBracket)?;
        Ok(refs)
    }

    // reqref := ident '.' ident

    /// A requirement reference `Spec.Id`: a path of exactly two segments.
    fn reqref(&mut self) -> PResult<Path> {
        let spec = self.ident()?;
        let start = spec.span.start;
        if !self.at(&TokenKind::Dot) {
            self.report(
                "E001",
                spec.span,
                format!(
                    "a requirement reference is `Spec.Id`; `{}` alone names no requirement",
                    spec.name
                ),
            );
            return Err(Fail);
        }
        self.bump();
        let id = self.ident()?;
        if self.at(&TokenKind::Dot) {
            let span = self.span_from(start);
            self.report(
                "E001",
                span,
                "a requirement reference has two segments, `Spec.Id`".to_string(),
            );
            return Err(Fail);
        }
        Ok(Path {
            segments: vec![spec, id],
            span: self.span_from(start),
        })
    }

    fn path(&mut self) -> PResult<Path> {
        let first = self.name_or_keyword()?;
        let start = first.span.start;
        let mut segments = vec![first];
        while self.eat(&TokenKind::Dot) {
            segments.push(self.name_or_keyword()?);
        }
        Ok(Path {
            segments,
            span: self.span_from(start),
        })
    }

    // import := 'import' path ('.' '{' ident (',' ident)* '}' | '.' '*')?

    fn import(&mut self) -> PResult<Import> {
        let start = self.expect(&TokenKind::KwImport)?.span.start;
        let first = self.name_or_keyword()?;
        let path_start = first.span.start;
        let mut segments = vec![first];
        let mut path_end = self.prev_end;
        let mut names = Vec::new();
        while self.eat(&TokenKind::Dot) {
            if self.at(&TokenKind::Star) {
                let star = self.bump();
                segments.push(Ident {
                    name: "*".to_string(),
                    span: star.span,
                });
                path_end = self.prev_end;
                break;
            }
            if self.at(&TokenKind::LBrace) {
                self.open(&TokenKind::LBrace, false)?;
                self.skip_newlines();
                names.push(self.ident()?);
                while self.eat(&TokenKind::Comma) {
                    self.skip_newlines();
                    names.push(self.ident()?);
                }
                self.skip_newlines();
                self.close(&TokenKind::RBrace)?;
                break;
            }
            segments.push(self.name_or_keyword()?);
            path_end = self.prev_end;
        }
        Ok(Import {
            path: Path {
                segments,
                span: Span::new(path_start, path_end),
            },
            names,
            span: self.span_from(start),
        })
    }

    /// `'{' (item sep)* item? '}'` where `sep` is a run of `,` and newlines.
    fn braced_list<T>(&mut self, mut item: impl FnMut(&mut Self) -> PResult<T>) -> PResult<Vec<T>> {
        self.open(&TokenKind::LBrace, false)?;
        self.skip_newlines();
        let mut items = Vec::new();
        while !self.at_close_brace()? {
            items.push(item(self)?);
            match self.peek() {
                TokenKind::Comma | TokenKind::Newline => {
                    while matches!(self.peek(), TokenKind::Comma | TokenKind::Newline) {
                        if self.at(&TokenKind::Comma) {
                            self.bump();
                        } else {
                            self.skip_newlines();
                        }
                    }
                }
                TokenKind::RBrace => {}
                _ => return Err(self.expected("`,`, newline or `}`")),
            }
        }
        self.close(&TokenKind::RBrace)?;
        Ok(items)
    }

    // record := 'type' ident tparams? satisfies? '{' (field sep)* field? '}'
    // field  := ident ':' type ('=' expr)? satisfies?
    // alias  := 'type' ident ('<' ident (',' ident)* '>')? '=' type

    fn type_decl(&mut self) -> PResult<Decl> {
        let start = self.expect(&TokenKind::KwType)?.span.start;
        let name = self.ident()?;
        let type_params = self.type_params()?;
        if self.at(&TokenKind::Assign) {
            let params = type_params;
            self.expect(&TokenKind::Assign)?;
            self.skip_newlines();
            let ty = self.ty()?;
            return Ok(Decl::Alias(AliasDecl {
                name,
                params,
                ty,
                span: self.span_from(start),
            }));
        }
        let satisfies = if self.at_word("satisfies") {
            self.clause_refs("satisfies")?
        } else {
            Vec::new()
        };
        let fields = self.braced_list(|p| {
            let name = p.name_or_keyword()?;
            p.expect(&TokenKind::Colon)?;
            let ty = p.ty()?;
            let default = if p.eat(&TokenKind::Assign) {
                p.skip_newlines();
                Some(p.expr()?)
            } else {
                None
            };
            let span = p.span_from(name.span.start);
            let satisfies = if p.at_word("satisfies") {
                p.clause_refs("satisfies")?
            } else {
                Vec::new()
            };
            Ok(Field {
                span,
                name,
                ty,
                default,
                satisfies,
            })
        })?;
        Ok(Decl::Record(RecordDecl {
            name,
            fields,
            span: self.span_from(start),
            satisfies,
            type_params,
            test_only: false,
        }))
    }

    /// `('<' ident (',' ident)* '>')?` after the name of a record, enum or
    /// alias: its type parameters. A bound `T: B` is E001 here
    /// (`docs/slice.md` 18.3).
    fn type_params(&mut self) -> PResult<Vec<Ident>> {
        let (params, bounds) = self.fn_type_params()?;
        if let Some(bound) = bounds.first() {
            self.report(
                "E001",
                bound.span,
                "a bound is allowed on the type parameters of a fn or role only".to_string(),
            );
            return Err(Fail);
        }
        Ok(params)
    }

    // fn_type_params := '<' fn_type_param (',' fn_type_param)* '>'
    // fn_type_param  := ident (':' ident ('+' ident)*)?

    /// The type parameters of a fn, associated fn or role, with their
    /// bounds (`docs/slice.md` 18.3).
    fn fn_type_params(&mut self) -> PResult<(Vec<Ident>, Vec<Bound>)> {
        let mut params = Vec::new();
        let mut bounds = Vec::new();
        if self.eat(&TokenKind::Lt) {
            loop {
                let param = self.ident()?;
                if self.eat(&TokenKind::Colon) {
                    let mut interfaces = vec![self.ident()?];
                    while self.eat(&TokenKind::Plus) {
                        interfaces.push(self.ident()?);
                    }
                    bounds.push(Bound {
                        span: self.span_from(param.span.start),
                        param: param.clone(),
                        interfaces,
                    });
                }
                params.push(param);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::Gt)?;
        }
        Ok((params, bounds))
    }

    /// `'requires' '[' (effect (',' effect)*)? ']'` after the keyword has
    /// been seen; an effect is a path that may end in `.*`.
    fn requires_list(&mut self) -> PResult<Vec<Path>> {
        self.expect(&TokenKind::KwRequires)?;
        self.open(&TokenKind::LBracket, true)?;
        let mut effects = Vec::new();
        if !self.at(&TokenKind::RBracket) {
            effects.push(self.effect()?);
            while self.eat(&TokenKind::Comma) {
                effects.push(self.effect()?);
            }
        }
        self.close(&TokenKind::RBracket)?;
        Ok(effects)
    }

    /// `path ('.' '*')?`: `stream.write`, `stdin.read`, `app.*`.
    fn effect(&mut self) -> PResult<Path> {
        let first = self.name_or_keyword()?;
        let start = first.span.start;
        let mut segments = vec![first];
        while self.eat(&TokenKind::Dot) {
            if self.at(&TokenKind::Star) {
                let span = self.bump().span;
                segments.push(Ident {
                    name: "*".to_string(),
                    span,
                });
                break;
            }
            segments.push(self.name_or_keyword()?);
        }
        Ok(Path {
            segments,
            span: self.span_from(start),
        })
    }

    // enum := 'enum' ident '{' (variant sep)* variant? '}'

    fn enum_decl(&mut self) -> PResult<EnumDecl> {
        let start = self.expect(&TokenKind::KwEnum)?.span.start;
        let name = self.ident()?;
        let type_params = self.type_params()?;
        let variants = self.braced_list(|p| {
            let name = p.ident()?;
            let mut payload = Vec::new();
            if p.at(&TokenKind::LParen) {
                p.open(&TokenKind::LParen, true)?;
                payload.push(p.ty()?);
                while p.eat(&TokenKind::Comma) {
                    payload.push(p.ty()?);
                }
                p.close(&TokenKind::RParen)?;
            }
            Ok(Variant {
                span: p.span_from(name.span.start),
                name,
                payload,
            })
        })?;
        Ok(EnumDecl {
            name,
            variants,
            span: self.span_from(start),
            type_params,
            test_only: false,
        })
    }

    // type := ident ('<' type (',' type)* '>')?
    //        | 'fn' '(' (fparam (',' fparam)*)? ')' ('->' type)? requires?
    // fparam := (ident ':')? type

    fn ty(&mut self) -> PResult<Type> {
        if self.at(&TokenKind::KwFn) {
            return self.fn_type();
        }
        if self.at(&TokenKind::LParen) {
            return self.tuple_type();
        }
        let name = self.ident()?;
        let mut args = Vec::new();
        if self.at(&TokenKind::Lt) {
            let lt = self.bump().span;
            self.enter(lt)?;
            args.push(self.ty()?);
            while self.eat(&TokenKind::Comma) {
                self.skip_newlines();
                args.push(self.ty()?);
            }
            self.expect(&TokenKind::Gt)?;
            self.leave();
        }
        let span = self.span_from(name.span.start);
        Ok(Type::named(name, args, span))
    }

    /// `(A, B)` as `Type { name: "Tuple", args: [A, B] }`; `()` is `Unit` and
    /// `(A)` is `A`; `(A,)` is E001.
    fn tuple_type(&mut self) -> PResult<Type> {
        let open = self.open(&TokenKind::LParen, true)?.span;
        let mut args = Vec::new();
        let mut comma = false;
        while !self.at(&TokenKind::RParen) {
            args.push(self.ty()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            comma = true;
        }
        self.close(&TokenKind::RParen)?;
        let span = self.span_from(open.start);
        let name = match args.len() {
            0 => "Unit",
            1 if comma => {
                self.report(
                    "E001",
                    span,
                    "a tuple type has at least two elements; drop the `,` to group a type"
                        .to_string(),
                );
                return Err(Fail);
            }
            1 => return Ok(args.remove(0)),
            _ => "Tuple",
        };
        let name = Ident {
            name: name.to_string(),
            span,
        };
        Ok(Type::named(name, args, span))
    }

    /// `fn(A, B) -> R` as `Type { name: "fn", args: [A, B, R] }`. Without
    /// `-> R` the return type is `Unit`, spanning the closing parenthesis.
    /// Named parameters `fn(state: S) -> R` fill `fn_names` (all or none,
    /// E001 for a mix); `requires [..]` after the return type makes the fn
    /// type effectful and belongs to the innermost fn type.
    fn fn_type(&mut self) -> PResult<Type> {
        let keyword = self.expect(&TokenKind::KwFn)?.span;
        self.open(&TokenKind::LParen, true)?;
        let mut args = Vec::new();
        let mut names = Vec::new();
        let mut unnamed = 0;
        if !self.at(&TokenKind::RParen) {
            loop {
                if matches!(self.peek(), TokenKind::Ident(_))
                    && self.peek_second() == &TokenKind::Colon
                {
                    let name = self.ident()?;
                    self.expect(&TokenKind::Colon)?;
                    names.push(name);
                } else {
                    unnamed += 1;
                }
                let ty = self.ty()?;
                if !names.is_empty() && unnamed > 0 {
                    self.report(
                        "E001",
                        ty.span,
                        "name every parameter of a fn type or none".to_string(),
                    );
                    return Err(Fail);
                }
                args.push(ty);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }
        let close = self.close(&TokenKind::RParen)?.span;
        let ret = if self.eat(&TokenKind::Arrow) {
            self.skip_newlines();
            self.ty()?
        } else {
            let name = Ident {
                name: "Unit".to_string(),
                span: close,
            };
            Type::named(name, Vec::new(), close)
        };
        args.push(ret);
        let requires = if self.at(&TokenKind::KwRequires) {
            Some(self.requires_list()?)
        } else {
            None
        };
        let name = Ident {
            name: "fn".to_string(),
            span: keyword,
        };
        let span = self.span_from(keyword.start);
        Ok(Type {
            name,
            args,
            span,
            fn_names: names,
            requires,
        })
    }

    // protocol := 'protocol' ident conforms? '{' NL* ((fn | status) NL*)* '}'

    fn protocol(&mut self) -> PResult<ProtocolDecl> {
        let start = self.expect(&TokenKind::KwProtocol)?.span.start;
        let name = self.ident()?;
        let (conforms, scope) = match self.protocol_header() {
            Ok(header) => header,
            Err(fail) => {
                self.skip_protocol_body();
                return Err(fail);
            }
        };
        let open = self.peek_index();
        self.open(&TokenKind::LBrace, false)?;
        self.skip_newlines();
        let mut fns = Vec::new();
        let mut statuses = Vec::new();
        loop {
            let member = match self.at_close_brace() {
                Ok(true) => break,
                Ok(false) if self.at(&TokenKind::KwFn) => self.fn_decl().map(|f| fns.push(f)),
                Ok(false) if self.at_word("unsupported") || self.at_word("not_applicable") => {
                    self.status().map(|s| statuses.push(s))
                }
                Ok(false) => {
                    Err(self.expected("`fn`, `unsupported`, `not_applicable` or `}` in a protocol"))
                }
                Err(fail) => Err(fail),
            };
            if let Err(fail) = member {
                self.skip_past_matching_brace(open);
                return Err(fail);
            }
            self.skip_newlines();
        }
        self.close(&TokenKind::RBrace)?;
        Ok(ProtocolDecl {
            name,
            fns,
            span: self.span_from(start),
            conforms,
            scope,
            statuses,
        })
    }

    // conforms   := 'conforms' '[' ident (',' ident)* ','? ']' scope?

    /// The clauses between a protocol's name and its body.
    fn protocol_header(&mut self) -> PResult<(Vec<Ident>, Option<Scope>)> {
        if self.at_word("scope") {
            let span = self.peek_token().span;
            self.report(
                "E001",
                span,
                "`scope` follows `conforms [..]`; a protocol without `conforms` has no scope"
                    .to_string(),
            );
            return Err(Fail);
        }
        if !self.at_word("conforms") {
            return Ok((Vec::new(), None));
        }
        self.bump();
        if !self.at(&TokenKind::LBracket) {
            return Err(self.expected("`[` after `conforms`"));
        }
        self.open(&TokenKind::LBracket, true)?;
        let mut conforms = Vec::new();
        loop {
            let spec = self.ident()?;
            if self.at(&TokenKind::Dot) {
                self.report(
                    "E001",
                    spec.span,
                    format!(
                        "a `conforms` entry is the name of a `Spec` constant; import the constant instead of qualifying it with `{}.`",
                        spec.name
                    ),
                );
                return Err(Fail);
            }
            conforms.push(spec);
            if !self.eat(&TokenKind::Comma) || self.at(&TokenKind::RBracket) {
                break;
            }
        }
        self.close(&TokenKind::RBracket)?;
        let scope = if self.at_word("scope") {
            Some(self.scope()?)
        } else {
            None
        };
        Ok((conforms, scope))
    }

    // scope      := 'scope' '{' (scope_item sep)* scope_item? '}'
    // scope_item := 'parties' '=' '[' (ident (',' ident)* ','?)? ']'
    //             | 'sections' '=' '[' (STRING (',' STRING)* ','?)? ']'

    fn scope(&mut self) -> PResult<Scope> {
        let start = self.bump().span.start;
        if !self.at(&TokenKind::LBrace) {
            return Err(self.expected("`{` after `scope`"));
        }
        let mut parties: Option<Vec<Ident>> = None;
        let mut sections: Option<Vec<(String, Span)>> = None;
        self.braced_list(|p| {
            let item = p.ident()?;
            let twice = match item.name.as_str() {
                "parties" => parties.is_some(),
                "sections" => sections.is_some(),
                _ => {
                    p.report(
                        "E001",
                        item.span,
                        format!(
                            "expected `parties` or `sections` in `scope`, found `{}`",
                            item.name
                        ),
                    );
                    return Err(Fail);
                }
            };
            if twice {
                p.report(
                    "E001",
                    item.span,
                    format!("`{}` is given twice in `scope`", item.name),
                );
                return Err(Fail);
            }
            p.expect(&TokenKind::Assign)?;
            p.open(&TokenKind::LBracket, true)?;
            if item.name == "parties" {
                let mut list = Vec::new();
                while !p.at(&TokenKind::RBracket) {
                    list.push(p.ident()?);
                    if !p.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                parties = Some(list);
            } else {
                let mut list = Vec::new();
                while !p.at(&TokenKind::RBracket) {
                    let token = p.peek_token();
                    match &token.kind {
                        TokenKind::Str(text) => {
                            p.bump();
                            list.push((text.clone(), token.span));
                        }
                        TokenKind::Template(_) => {
                            p.report(
                                "E001",
                                token.span,
                                "a section is a string literal without interpolation".to_string(),
                            );
                            return Err(Fail);
                        }
                        _ => return Err(p.expected("a section (a string literal)")),
                    }
                    if !p.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                sections = Some(list);
            }
            p.close(&TokenKind::RBracket)?;
            Ok(())
        })?;
        Ok(Scope {
            parties,
            sections,
            span: self.span_from(start),
        })
    }

    // status := ('unsupported' | 'not_applicable') reqref STRING

    fn status(&mut self) -> PResult<StatusDecl> {
        let token = self.bump();
        let start = token.span.start;
        let kind = match &token.kind {
            TokenKind::Ident(word) if word == "unsupported" => StatusKind::Unsupported,
            _ => StatusKind::NotApplicable,
        };
        let requirement = self.reqref()?;
        let token = self.peek_token();
        let reason = match &token.kind {
            TokenKind::Str(text) if !text.is_empty() => {
                self.bump();
                text.clone()
            }
            TokenKind::Str(_) => {
                self.report(
                    "E001",
                    token.span,
                    format!("the reason of `{}` cannot be empty", kind.word()),
                );
                return Err(Fail);
            }
            TokenKind::Template(_) => {
                self.report(
                    "E001",
                    token.span,
                    format!(
                        "the reason of `{}` is a string literal without interpolation",
                        kind.word()
                    ),
                );
                return Err(Fail);
            }
            _ => {
                return Err(self.expected(&format!(
                    "a reason (a string literal) after `{} {}`",
                    kind.word(),
                    requirement.dotted()
                )));
            }
        };
        Ok(StatusDecl {
            kind,
            requirement,
            reason,
            span: self.span_from(start),
        })
    }

    /// After an error in a protocol's header: move past its body, so that
    /// recovery does not resume at the body's `fn` lines. The body is the
    /// first `{` outside brackets that does not follow `scope`; a line
    /// break outside brackets ends the search.
    fn skip_protocol_body(&mut self) {
        let mut depth = self.modes.len();
        let mut i = self.pos;
        while i < self.tokens.len() {
            match &self.tokens[i].kind {
                TokenKind::Eof => break,
                TokenKind::LBrace if depth == 0 => {
                    let after_scope = self.tokens[..i]
                        .iter()
                        .rev()
                        .find(|t| t.kind != TokenKind::Newline)
                        .is_some_and(|t| matches!(&t.kind, TokenKind::Ident(w) if w == "scope"));
                    if !after_scope {
                        self.skip_past_matching_brace(i);
                        return;
                    }
                    depth += 1;
                }
                TokenKind::LBrace | TokenKind::LBracket | TokenKind::LParen => depth += 1,
                TokenKind::RBrace | TokenKind::RBracket | TokenKind::RParen => {
                    depth = depth.saturating_sub(1);
                }
                TokenKind::Newline if depth == 0 => break,
                _ => {}
            }
            i += 1;
        }
        self.pos = i;
        self.reset_nesting();
    }

    // fn := 'fn' ident ('.' ident)? tparams? '(' (param (',' param)*)? ')' ('->' type)? requires? satisfies? (block | '=' expr)

    fn fn_decl(&mut self) -> PResult<FnDecl> {
        let start = self.expect(&TokenKind::KwFn)?.span.start;
        let mut name = self.ident()?;
        let mut owner = None;
        if self.peek() == &TokenKind::Dot {
            self.bump();
            owner = Some(std::mem::replace(&mut name, self.ident()?));
        }
        let (type_params, bounds) = self.fn_type_params()?;
        self.open(&TokenKind::LParen, true)?;
        let mut params = Vec::new();
        if !self.at(&TokenKind::RParen) {
            loop {
                let name = self.ident()?;
                self.expect(&TokenKind::Colon)?;
                let ty = self.ty()?;
                params.push(Param {
                    span: self.span_from(name.span.start),
                    name,
                    ty,
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.close(&TokenKind::RParen)?;
        let ret = if self.eat(&TokenKind::Arrow) {
            self.skip_newlines();
            Some(self.ty()?)
        } else {
            None
        };
        let requires = if self.at(&TokenKind::KwRequires) {
            Some(self.requires_list()?)
        } else {
            None
        };
        let satisfies = if self.at_word("satisfies") {
            self.clause_refs("satisfies")?
        } else {
            Vec::new()
        };
        let body = if self.eat(&TokenKind::Assign) {
            self.skip_newlines();
            let value = self.expr()?;
            let span = value.span;
            Block {
                stmts: vec![Stmt {
                    kind: StmtKind::Expr(value),
                    span,
                }],
                span,
            }
        } else {
            self.block()?
        };
        Ok(FnDecl {
            name,
            params,
            ret,
            requires,
            body,
            span: self.span_from(start),
            type_params,
            test_only: false,
            owner,
            bounds,
            satisfies,
        })
    }

    // const := 'const' ident ':' type '=' expr

    fn const_decl(&mut self) -> PResult<ConstDecl> {
        let start = self.expect(&TokenKind::KwConst)?.span.start;
        let name = self.ident()?;
        self.expect(&TokenKind::Colon)?;
        let ty = self.ty()?;
        self.expect(&TokenKind::Assign)?;
        self.skip_newlines();
        let value = self.expr()?;
        Ok(ConstDecl {
            name,
            ty,
            value,
            span: self.span_from(start),
            test_only: false,
        })
    }

    // system := 'system' ident '{' (resource | component)* '}'

    fn system(&mut self) -> PResult<SystemDecl> {
        let start = self.expect(&TokenKind::KwSystem)?.span.start;
        let name = self.ident()?;
        self.open(&TokenKind::LBrace, false)?;
        self.skip_newlines();
        let mut resources = Vec::new();
        let mut components = Vec::new();
        let mut contracts = Vec::new();
        let mut policies = Vec::new();
        while !self.at_close_brace()? {
            match self.peek() {
                TokenKind::KwResource => {
                    let (name, kind, args, properties, span) = self.system_member(true)?;
                    resources.push(ResourceDecl {
                        name,
                        kind,
                        properties,
                        span,
                        args,
                    });
                }
                TokenKind::KwComponent | TokenKind::KwContract | TokenKind::KwPolicy => {
                    let list = match self.peek() {
                        TokenKind::KwContract => &mut contracts,
                        TokenKind::KwPolicy => &mut policies,
                        _ => &mut components,
                    };
                    let (name, kind, _, properties, span) = self.system_member(false)?;
                    list.push(ComponentDecl {
                        name,
                        kind,
                        properties,
                        span,
                    });
                }
                _ => {
                    return Err(
                        self.expected("`resource`, `component`, `contract`, `policy` or `}`")
                    );
                }
            }
            match self.peek() {
                TokenKind::Newline => self.skip_newlines(),
                TokenKind::RBrace => {}
                _ => return Err(self.expected("newline or `}`")),
            }
        }
        self.close(&TokenKind::RBrace)?;
        Ok(SystemDecl {
            name,
            resources,
            components,
            span: self.span_from(start),
            contracts,
            policies,
        })
    }

    /// `('resource' | 'component') ident ':' ident '{' properties '}'`. A
    /// resource kind may take type arguments (`Store<Todo>`) and its
    /// properties may be left out (`resource todos: Store<Todo>`).
    #[allow(clippy::type_complexity)]
    fn system_member(
        &mut self,
        resource: bool,
    ) -> PResult<(Ident, Ident, Vec<Type>, Vec<FieldInit>, Span)> {
        let start = self.bump().span.start;
        let name = self.ident()?;
        self.expect(&TokenKind::Colon)?;
        let kind = self.ident()?;
        let mut args = Vec::new();
        if resource && self.eat(&TokenKind::Lt) {
            args.push(self.ty()?);
            while self.eat(&TokenKind::Comma) {
                args.push(self.ty()?);
            }
            self.expect(&TokenKind::Gt)?;
        }
        let properties = if resource && !self.at(&TokenKind::LBrace) {
            Vec::new()
        } else {
            self.braced_list(Self::field_init)?
        };
        Ok((name, kind, args, properties, self.span_from(start)))
    }

    /// `(ident | keyword) ('=' expr)?`; without `= expr` the value is the
    /// name itself (`Reply { lines }` is `Reply { lines = lines }`).
    fn field_init(&mut self) -> PResult<FieldInit> {
        let name = self.name_or_keyword()?;
        if !self.at(&TokenKind::Assign) {
            return Ok(FieldInit {
                span: name.span,
                value: Expr {
                    kind: ExprKind::Name(name.clone()),
                    span: name.span,
                },
                name,
            });
        }
        self.expect(&TokenKind::Assign)?;
        self.skip_newlines();
        let value = self.expr()?;
        Ok(FieldInit {
            span: self.span_from(name.span.start),
            name,
            value,
        })
    }

    // block := '{' (stmt sep)* stmt? '}'

    fn block(&mut self) -> PResult<Block> {
        let start = self.open(&TokenKind::LBrace, false)?.span.start;
        self.skip_newlines();
        let mut stmts = Vec::new();
        while !self.at_close_brace()? {
            stmts.push(self.stmt()?);
            match self.peek() {
                TokenKind::Semicolon | TokenKind::Newline => {
                    while matches!(self.peek(), TokenKind::Semicolon | TokenKind::Newline) {
                        if self.at(&TokenKind::Semicolon) {
                            self.bump();
                        } else {
                            self.skip_newlines();
                        }
                    }
                }
                TokenKind::RBrace => {}
                _ => return Err(self.expected("`;`, newline or `}`")),
            }
        }
        self.close(&TokenKind::RBrace)?;
        Ok(Block {
            stmts,
            span: self.span_from(start),
        })
    }

    fn stmt(&mut self) -> PResult<Stmt> {
        let start = self.start();
        if is_top_level_keyword(self.peek()) {
            let token = self.peek_token();
            self.report(
                "E001",
                token.span,
                format!(
                    "{} declarations are only allowed at the top level; move it out of the block",
                    token.kind.describe()
                ),
            );
            self.skip_enclosing_declaration();
            return Err(Fail);
        }
        let kind = match self.peek() {
            TokenKind::Ident(name) if name == "_" && *self.peek_second() == TokenKind::Assign => {
                let pattern = self.pattern()?;
                self.expect(&TokenKind::Assign)?;
                self.skip_newlines();
                let value = self.expr()?;
                StmtKind::LetPattern {
                    pattern,
                    value,
                    mutable: false,
                }
            }
            TokenKind::KwLet | TokenKind::KwVar if self.let_pattern_follows() => {
                let mutable = self.bump().kind == TokenKind::KwVar;
                let pattern = self.pattern()?;
                if !irrefutable(&pattern) {
                    self.report(
                        "E001",
                        pattern.span,
                        "the pattern of `let` or `var` is a name, `_` or a tuple of those"
                            .to_string(),
                    );
                    return Err(Fail);
                }
                self.expect(&TokenKind::Assign)?;
                self.skip_newlines();
                let value = self.expr()?;
                StmtKind::LetPattern {
                    pattern,
                    value,
                    mutable,
                }
            }
            TokenKind::KwLet | TokenKind::KwVar => {
                let mutable = self.bump().kind == TokenKind::KwVar;
                let name = self.ident()?;
                let ty = if self.eat(&TokenKind::Colon) {
                    Some(self.ty()?)
                } else {
                    None
                };
                self.expect(&TokenKind::Assign)?;
                self.skip_newlines();
                let value = self.expr()?;
                StmtKind::Let {
                    name,
                    ty,
                    value,
                    mutable,
                }
            }
            TokenKind::KwReturn => {
                self.bump();
                match self.peek() {
                    TokenKind::Newline
                    | TokenKind::Semicolon
                    | TokenKind::RBrace
                    | TokenKind::Eof => StmtKind::Return(None),
                    _ => StmtKind::Return(Some(self.expr()?)),
                }
            }
            TokenKind::KwWhile => {
                self.bump();
                let cond = self.restricted_expr()?;
                let body = self.block()?;
                StmtKind::While { cond, body }
            }
            TokenKind::KwLoop => {
                self.bump();
                StmtKind::Loop {
                    body: self.block()?,
                }
            }
            TokenKind::KwFor if *self.peek_second() == TokenKind::LParen => {
                self.bump();
                let pattern = self.pattern()?;
                if !irrefutable(&pattern) {
                    self.report(
                        "E001",
                        pattern.span,
                        "the pattern of `for` is a name, `_` or a tuple of those".to_string(),
                    );
                    return Err(Fail);
                }
                self.expect(&TokenKind::KwIn)?;
                let iter = self.restricted_expr()?;
                let body = self.block()?;
                StmtKind::ForPattern {
                    pattern,
                    iter,
                    body,
                }
            }
            TokenKind::KwFor => {
                self.bump();
                let name = self.ident()?;
                self.expect(&TokenKind::KwIn)?;
                let iter = self.restricted_expr()?;
                let body = self.block()?;
                StmtKind::For { name, iter, body }
            }
            TokenKind::KwBreak => {
                self.bump();
                StmtKind::Break
            }
            TokenKind::KwContinue => {
                self.bump();
                StmtKind::Continue
            }
            TokenKind::KwAssert => {
                self.bump();
                let cond = self.expr()?;
                let message = if self.eat(&TokenKind::Comma) {
                    self.skip_newlines();
                    Some(self.expr()?)
                } else {
                    None
                };
                StmtKind::Assert { cond, message }
            }
            _ => {
                let expr = self.expr()?;
                if self.at(&TokenKind::Assign) {
                    let Some(target) = place(&expr) else {
                        self.report(
                            "E001",
                            expr.span,
                            "the left side of `=` must be a name or a field path such as `a.b`"
                                .to_string(),
                        );
                        return Err(Fail);
                    };
                    self.bump();
                    self.skip_newlines();
                    let value = self.expr()?;
                    StmtKind::Assign { target, value }
                } else {
                    StmtKind::Expr(expr)
                }
            }
        };
        Ok(Stmt {
            kind,
            span: self.span_from(start),
        })
    }

    // expressions, loosest first

    fn expr(&mut self) -> PResult<Expr> {
        self.or()
    }

    fn binary_left(
        &mut self,
        next: fn(&mut Self) -> PResult<Expr>,
        op_of: fn(&TokenKind) -> Option<BinaryOp>,
    ) -> PResult<Expr> {
        let start = self.start();
        let mut left = next(self)?;
        while let Some(op) = op_of(self.peek()) {
            self.bump();
            self.skip_newlines();
            let right = next(self)?;
            left = binary(op, left, right, self.span_from(start));
        }
        Ok(left)
    }

    fn binary_non_assoc(
        &mut self,
        next: fn(&mut Self) -> PResult<Expr>,
        op_of: fn(&TokenKind) -> Option<BinaryOp>,
    ) -> PResult<Expr> {
        let start = self.start();
        let left = next(self)?;
        let Some(op) = op_of(self.peek()) else {
            return Ok(left);
        };
        self.bump();
        self.skip_newlines();
        let right = next(self)?;
        let expr = binary(op, left, right, self.span_from(start));
        if let Some(second) = op_of(self.peek()) {
            let span = self.peek_token().span;
            self.report(
                "E001",
                span,
                format!(
                    "`{}` cannot follow `{}` without parentheses; comparisons do not chain",
                    second.symbol(),
                    op.symbol()
                ),
            );
            return Err(Fail);
        }
        Ok(expr)
    }

    fn or(&mut self) -> PResult<Expr> {
        self.binary_left(Self::and, |k| {
            (*k == TokenKind::OrOr).then_some(BinaryOp::Or)
        })
    }

    fn and(&mut self) -> PResult<Expr> {
        self.binary_left(Self::equality, |k| {
            (*k == TokenKind::AndAnd).then_some(BinaryOp::And)
        })
    }

    fn equality(&mut self) -> PResult<Expr> {
        self.binary_non_assoc(Self::compare, |k| match k {
            TokenKind::EqEq => Some(BinaryOp::Eq),
            TokenKind::NotEq => Some(BinaryOp::Ne),
            _ => None,
        })
    }

    /// `>=` arrives as `>` and `=` with adjacent spans
    /// ([`split_greater_equal`]); this joins them.
    fn compare_op(&self) -> Option<(BinaryOp, usize)> {
        let i = self.peek_index();
        Some(match &self.tokens[i].kind {
            TokenKind::Lt => (BinaryOp::Lt, 1),
            TokenKind::LtEq => (BinaryOp::Le, 1),
            TokenKind::GtEq => (BinaryOp::Ge, 1),
            TokenKind::Gt => {
                let next = &self.tokens[i + 1];
                if next.kind == TokenKind::Assign && next.span.start == self.tokens[i].span.end {
                    (BinaryOp::Ge, 2)
                } else {
                    (BinaryOp::Gt, 1)
                }
            }
            _ => return None,
        })
    }

    fn compare(&mut self) -> PResult<Expr> {
        let start = self.start();
        let left = self.additive()?;
        let Some((op, len)) = self.compare_op() else {
            return Ok(left);
        };
        for _ in 0..len {
            self.bump();
        }
        self.skip_newlines();
        let right = self.additive()?;
        let expr = binary(op, left, right, self.span_from(start));
        if let Some((second, _)) = self.compare_op() {
            let span = self.peek_token().span;
            self.report(
                "E001",
                span,
                format!(
                    "`{}` cannot follow `{}` without parentheses; comparisons do not chain",
                    second.symbol(),
                    op.symbol()
                ),
            );
            return Err(Fail);
        }
        Ok(expr)
    }

    fn additive(&mut self) -> PResult<Expr> {
        self.binary_left(Self::multiplicative, |k| match k {
            TokenKind::Plus => Some(BinaryOp::Add),
            TokenKind::Minus => Some(BinaryOp::Sub),
            _ => None,
        })
    }

    fn multiplicative(&mut self) -> PResult<Expr> {
        self.binary_left(Self::unary, |k| match k {
            TokenKind::Star => Some(BinaryOp::Mul),
            TokenKind::Slash => Some(BinaryOp::Div),
            TokenKind::Percent => Some(BinaryOp::Rem),
            _ => None,
        })
    }

    // unary := ('!' | '-') unary | 'await' unary | 'spawn' unary | postfix

    fn unary(&mut self) -> PResult<Expr> {
        let start = self.start();
        let wrap: fn(Box<Expr>) -> ExprKind = match self.peek() {
            TokenKind::Bang => |operand| ExprKind::Unary {
                op: UnaryOp::Not,
                operand,
            },
            TokenKind::Minus => |operand| ExprKind::Unary {
                op: UnaryOp::Neg,
                operand,
            },
            TokenKind::KwAwait => {
                if self.options.await_is_error {
                    let span = self.peek_token().span;
                    self.report(
                        "E001",
                        span,
                        "remove `await`: effectful calls need no marker".to_string(),
                    );
                    return Err(Fail);
                }
                self.bump();
                return self.unary();
            }
            TokenKind::KwSpawn => ExprKind::Spawn,
            _ => return self.postfix(),
        };
        let op = self.bump();
        if op.kind == TokenKind::Minus
            && self.peek() == &TokenKind::Int(i64::MIN)
            && !matches!(
                self.peek_second(),
                TokenKind::Dot | TokenKind::LParen | TokenKind::LBracket | TokenKind::Question
            )
        {
            self.bump();
            return Ok(Expr {
                kind: ExprKind::Int(i64::MIN),
                span: self.span_from(start),
            });
        }
        self.enter(op.span)?;
        let operand = self.unary()?;
        self.leave();
        let kind = wrap(Box::new(operand));
        Ok(Expr {
            kind,
            span: self.span_from(start),
        })
    }

    // postfix := primary ( '(' args ')' | '.' ident ('(' args ')')? | '.' INT | '[' expr ']' | '?' )*

    fn postfix(&mut self) -> PResult<Expr> {
        let start = self.start();
        let mut expr = self.primary()?;
        loop {
            if self.at_line_break() {
                return Ok(expr);
            }
            let kind = match self.peek() {
                TokenKind::LParen => {
                    let args = self.args()?;
                    ExprKind::Call {
                        callee: Box::new(expr),
                        args,
                    }
                }
                TokenKind::Dot if matches!(self.peek_second(), TokenKind::Int(_)) => {
                    self.bump();
                    let TokenKind::Int(index) = self.bump().kind else {
                        unreachable!("peeked an integer")
                    };
                    ExprKind::TupleField {
                        receiver: Box::new(expr),
                        index: usize::try_from(index).unwrap_or(usize::MAX),
                    }
                }
                TokenKind::Dot => {
                    self.bump();
                    let name = self.name_or_keyword()?;
                    if self.at(&TokenKind::LParen) {
                        let args = self.args()?;
                        ExprKind::Method {
                            receiver: Box::new(expr),
                            name,
                            args,
                        }
                    } else {
                        ExprKind::Field {
                            receiver: Box::new(expr),
                            name,
                        }
                    }
                }
                TokenKind::LBracket => {
                    self.open(&TokenKind::LBracket, true)?;
                    let index = self.expr()?;
                    self.close(&TokenKind::RBracket)?;
                    ExprKind::Index {
                        receiver: Box::new(expr),
                        index: Box::new(index),
                    }
                }
                TokenKind::Question => {
                    self.bump();
                    ExprKind::Try(Box::new(expr))
                }
                _ => return Ok(expr),
            };
            expr = Expr {
                kind,
                span: self.span_from(start),
            };
        }
    }

    /// `'(' (arg (',' arg)*)? ')'`, `arg := (ident '=')? expr`: a named
    /// argument is [`ExprKind::NamedArg`] (`docs/slice.md` section 14.2).
    fn args(&mut self) -> PResult<Vec<Expr>> {
        self.open(&TokenKind::LParen, true)?;
        let mut args = Vec::new();
        if !self.at(&TokenKind::RParen) {
            args.push(self.arg()?);
            while self.eat(&TokenKind::Comma) {
                args.push(self.arg()?);
            }
        }
        self.close(&TokenKind::RParen)?;
        Ok(args)
    }

    fn arg(&mut self) -> PResult<Expr> {
        if matches!(self.peek(), TokenKind::Ident(_)) && *self.peek_second() == TokenKind::Assign {
            let start = self.start();
            let name = self.ident()?;
            self.bump();
            let value = self.expr()?;
            return Ok(Expr {
                kind: ExprKind::NamedArg {
                    name,
                    value: Box::new(value),
                },
                span: self.span_from(start),
            });
        }
        self.expr()
    }

    fn primary(&mut self) -> PResult<Expr> {
        let token = self.peek_token();
        let start = token.span.start;
        let kind = match &token.kind {
            TokenKind::Int(i64::MIN) => return Err(self.int_too_large(token)),
            TokenKind::Int(value) => {
                self.bump();
                match self.unit_after(token) {
                    Some(unit) => ExprKind::Quantity {
                        literal: value.to_string(),
                        unit,
                    },
                    None => ExprKind::Int(*value),
                }
            }
            TokenKind::Decimal(literal) => {
                self.bump();
                match self.unit_after(token) {
                    Some(unit) => ExprKind::Quantity {
                        literal: literal.clone(),
                        unit,
                    },
                    None => {
                        self.report(
                            "E001",
                            token.span,
                            format!(
                                "a decimal literal needs a unit or `%` after it, as in `{literal}%`"
                            ),
                        );
                        return Err(Fail);
                    }
                }
            }
            TokenKind::LBrace => {
                let entries = self.brace_entries()?;
                ExprKind::Brace { entries }
            }
            TokenKind::Str(value) => {
                self.bump();
                ExprKind::Str(value.clone())
            }
            TokenKind::Template(parts) => {
                self.bump();
                return self.template(parts, token.span);
            }
            TokenKind::ByteStr(bytes) => {
                self.bump();
                let span = token.span;
                let at = |kind| Expr { kind, span };
                let items = bytes.iter().map(|b| at(ExprKind::Int(i64::from(*b))));
                ExprKind::Method {
                    receiver: Box::new(at(ExprKind::Name(Ident {
                        name: "Bytes".to_string(),
                        span,
                    }))),
                    name: Ident {
                        name: "of".to_string(),
                        span,
                    },
                    args: vec![at(ExprKind::List(items.collect()))],
                }
            }
            TokenKind::KwReturn => {
                self.bump();
                let value = match self.peek() {
                    TokenKind::Newline
                    | TokenKind::Semicolon
                    | TokenKind::Comma
                    | TokenKind::RBrace
                    | TokenKind::RParen
                    | TokenKind::RBracket
                    | TokenKind::Eof => None,
                    _ => {
                        self.enter(token.span)?;
                        let value = self.expr()?;
                        self.leave();
                        Some(Box::new(value))
                    }
                };
                ExprKind::Return(value)
            }
            TokenKind::KwBreak => {
                self.bump();
                ExprKind::Break
            }
            TokenKind::KwContinue => {
                self.bump();
                ExprKind::Continue
            }
            TokenKind::Pipe | TokenKind::OrOr => {
                self.enter(token.span)?;
                let closure = self.closure()?;
                self.leave();
                ExprKind::Closure(closure)
            }
            TokenKind::KwTrue => {
                self.bump();
                ExprKind::Bool(true)
            }
            TokenKind::KwFalse => {
                self.bump();
                ExprKind::Bool(false)
            }
            TokenKind::Ident(_) => {
                let name = self.ident()?;
                if name.is_uppercase() && !self.no_record && self.at(&TokenKind::LBrace) {
                    let (fields, base) = self.record_body()?;
                    match base {
                        Some(base) => ExprKind::RecordUpdate {
                            name,
                            fields,
                            base: Box::new(base),
                        },
                        None => ExprKind::Record { name, fields },
                    }
                } else {
                    ExprKind::Name(name)
                }
            }
            TokenKind::LBracket => {
                self.open(&TokenKind::LBracket, true)?;
                let mut items = Vec::new();
                if !self.at(&TokenKind::RBracket) {
                    items.push(self.expr()?);
                    while self.eat(&TokenKind::Comma) {
                        items.push(self.expr()?);
                    }
                }
                self.close(&TokenKind::RBracket)?;
                ExprKind::List(items)
            }
            TokenKind::KwMatch => {
                self.bump();
                let scrutinee = Box::new(self.restricted_expr()?);
                let arms = self.braced_list(Self::arm)?;
                ExprKind::Match { scrutinee, arms }
            }
            TokenKind::KwIf => return self.if_expr(),
            TokenKind::LParen => {
                self.open(&TokenKind::LParen, true)?;
                let mut items = Vec::new();
                let mut comma = false;
                while !self.at(&TokenKind::RParen) {
                    items.push(self.expr()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                    comma = true;
                }
                self.close(&TokenKind::RParen)?;
                if items.len() == 1 {
                    if comma {
                        self.report(
                            "E001",
                            self.span_from(start),
                            "a tuple has at least two elements; drop the `,` to group an expression"
                                .to_string(),
                        );
                        return Err(Fail);
                    }
                    return Ok(items.remove(0));
                }
                ExprKind::Tuple(items)
            }
            _ => return Err(self.expected("expression")),
        };
        Ok(Expr {
            kind,
            span: self.span_from(start),
        })
    }

    /// After the number `literal` (just consumed): the unit of a quantity
    /// literal (`docs/slice.md` section 14.1), consumed, or `None`. A unit
    /// is a unit name on the same line, or a `%` that touches the number
    /// and is not followed by an operand (`10 % 3`, `n%2` stay remainders).
    fn unit_after(&mut self, literal: &Token) -> Option<Ident> {
        let next = &self.tokens[self.pos];
        let unit = match &next.kind {
            TokenKind::Ident(name) if crate::ast::quantity_unit(name).is_some() => name.clone(),
            TokenKind::Percent if next.span.start == literal.span.end => {
                let after = &self.tokens[self.pos + 1].kind;
                let decimal = matches!(literal.kind, TokenKind::Decimal(_));
                if !decimal && starts_operand(after) {
                    return None;
                }
                "%".to_string()
            }
            _ => return None,
        };
        self.pos += 1;
        self.prev_end = next.span.end;
        Some(Ident {
            name: unit,
            span: next.span,
        })
    }

    /// `'{' (key '=' expr sep)* '}'`, `key := name ('.' name)*`: the
    /// entries of a brace literal without a type name.
    fn brace_entries(&mut self) -> PResult<Vec<BraceEntry>> {
        self.braced_list(|p| {
            let start = p.start();
            let mut segments = vec![p.name_or_keyword()?];
            while p.eat(&TokenKind::Dot) {
                segments.push(p.name_or_keyword()?);
            }
            let key = Path {
                span: p.span_from(start),
                segments,
            };
            p.expect(&TokenKind::Assign)?;
            p.skip_newlines();
            let value = p.expr()?;
            Ok(BraceEntry {
                key,
                value,
                span: p.span_from(start),
            })
        })
    }

    /// `'{' (field_init sep)* ('..' expr sep?)? '}'`: the fields of a record
    /// literal and the base of a record update, which comes last.
    fn record_body(&mut self) -> PResult<(Vec<FieldInit>, Option<Expr>)> {
        self.open(&TokenKind::LBrace, false)?;
        self.skip_newlines();
        let mut fields = Vec::new();
        let mut base = None;
        while !self.at_close_brace()? {
            if base.is_some() {
                return Err(self.expected("`}` after the base of a record update"));
            }
            if self.eat(&TokenKind::DotDot) {
                self.skip_newlines();
                base = Some(self.expr()?);
            } else {
                fields.push(self.field_init()?);
            }
            match self.peek() {
                TokenKind::Comma | TokenKind::Newline => {
                    while matches!(self.peek(), TokenKind::Comma | TokenKind::Newline) {
                        if self.at(&TokenKind::Comma) {
                            self.bump();
                        } else {
                            self.skip_newlines();
                        }
                    }
                }
                TokenKind::RBrace => {}
                _ => return Err(self.expected("`,`, newline or `}`")),
            }
        }
        self.close(&TokenKind::RBrace)?;
        Ok((fields, base))
    }

    // closure := ('|' (param (',' param)*)? '|' | '||') (block | expr)
    // param   := ident (':' type)?

    fn closure(&mut self) -> PResult<Closure> {
        let mut params = Vec::new();
        if !self.eat(&TokenKind::OrOr) {
            self.expect(&TokenKind::Pipe)?;
            while !self.at(&TokenKind::Pipe) {
                let name = self.ident()?;
                let ty = if self.eat(&TokenKind::Colon) {
                    Some(self.ty()?)
                } else {
                    None
                };
                params.push(ClosureParam {
                    span: self.span_from(name.span.start),
                    name,
                    ty,
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::Pipe)?;
        }
        let body = if self.at(&TokenKind::LBrace) {
            let block = self.block()?;
            Expr {
                span: block.span,
                kind: ExprKind::Block(block),
            }
        } else {
            self.expr()?
        };
        Ok(Closure {
            params,
            body: Box::new(body),
        })
    }

    /// E001 for the literal `9223372036854775808` anywhere but after `-`.
    fn int_too_large(&mut self, token: &Token) -> Fail {
        self.report(
            "E001",
            token.span,
            "integer literal `9223372036854775808` does not fit in 64 bits; only `-9223372036854775808` does"
                .to_string(),
        );
        Fail
    }

    /// An interpolated string literal: its pieces joined with `+`, each
    /// `{e}` as `e.to_text()` (`docs/slice.md` section 10.1).
    fn template(&mut self, parts: &'t [TemplatePart], span: Span) -> PResult<Expr> {
        let mut joined: Option<Expr> = None;
        for part in parts {
            let piece = match part {
                TemplatePart::Text(text) => Expr {
                    kind: ExprKind::Str(text.clone()),
                    span,
                },
                TemplatePart::Expr(tokens) => {
                    let tokens = split_greater_equal(tokens);
                    let mut inner = Parser::new(&tokens, self.options, self.depth);
                    let parsed = inner.expr();
                    if parsed.is_ok() && !inner.at(&TokenKind::Eof) {
                        inner.expected("`}` after the interpolated expression");
                    }
                    let failed = !inner.diagnostics.is_empty();
                    for d in inner.diagnostics {
                        let message = d.message.replace(
                            "unexpected end of file, expected",
                            "unexpected `}` in string, expected",
                        );
                        let code = if d.code == "E002" { "E001" } else { d.code };
                        self.report(code, d.span, message);
                    }
                    let value = match parsed {
                        Ok(value) if !failed => value,
                        _ => return Err(Fail),
                    };
                    let at = value.span;
                    Expr {
                        kind: ExprKind::Method {
                            receiver: Box::new(value),
                            name: Ident {
                                name: "to_text".to_string(),
                                span: at,
                            },
                            args: Vec::new(),
                        },
                        span: at,
                    }
                }
            };
            joined = Some(match joined {
                None => piece,
                Some(left) => binary(BinaryOp::Add, left, piece, span),
            });
        }
        joined.ok_or_else(|| {
            self.report("E001", span, "empty string template".to_string());
            Fail
        })
    }

    // 'if' expr block ('else' ('if' ... | block))?

    fn if_expr(&mut self) -> PResult<Expr> {
        let start = self.expect(&TokenKind::KwIf)?.span.start;
        let cond = Box::new(self.restricted_expr()?);
        let then = self.block()?;
        let otherwise = if !self.at_line_break() && self.eat(&TokenKind::KwElse) {
            if self.at(&TokenKind::KwIf) {
                Some(Box::new(self.if_expr()?))
            } else {
                let block = self.block()?;
                Some(Box::new(Expr {
                    span: block.span,
                    kind: ExprKind::Block(block),
                }))
            }
        } else {
            None
        };
        Ok(Expr {
            kind: ExprKind::If {
                cond,
                then,
                otherwise,
            },
            span: self.span_from(start),
        })
    }

    // arm := pattern '=>' (block | expr)

    fn arm(&mut self) -> PResult<Arm> {
        let pattern = self.pattern()?;
        self.expect(&TokenKind::FatArrow)?;
        self.skip_newlines();
        let body = if self.at(&TokenKind::LBrace) {
            let block = self.block()?;
            Expr {
                span: block.span,
                kind: ExprKind::Block(block),
            }
        } else {
            self.expr()?
        };
        Ok(Arm {
            span: self.span_from(pattern.span.start),
            pattern,
            body,
        })
    }

    // pattern := '_' | INT | STRING | path ('(' pattern (',' pattern)* ')')?
    // A single lowercase name without arguments is a binding.

    fn pattern(&mut self) -> PResult<Pattern> {
        let first = self.pattern_alternative()?;
        if !self.at(&TokenKind::Pipe) {
            return Ok(first);
        }
        let start = first.span.start;
        let mut alternatives = vec![first];
        while self.eat(&TokenKind::Pipe) {
            self.skip_newlines();
            alternatives.push(self.pattern_alternative()?);
        }
        Ok(Pattern {
            kind: PatternKind::Or(alternatives),
            span: self.span_from(start),
        })
    }

    /// `'(' (pattern (',' pattern)*)? ')'` or the same in `[ ]`: the
    /// patterns and whether a `,` was written.
    fn pattern_list(&mut self, open: TokenKind, close: TokenKind) -> PResult<(Vec<Pattern>, bool)> {
        self.open(&open, true)?;
        let mut items = Vec::new();
        let mut comma = false;
        while !self.at(&close) {
            items.push(self.pattern()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            comma = true;
        }
        self.close(&close)?;
        Ok((items, comma))
    }

    /// `'[' (elem (',' elem)*)? ']'` with `elem := pattern | '..' ident?`
    /// (`docs/slice.md` 20.5). A second `..`, or a `..name` that is not
    /// last, is E076; the list is still returned.
    fn list_pattern_items(&mut self) -> PResult<Vec<Pattern>> {
        self.open(&TokenKind::LBracket, true)?;
        let mut items = Vec::new();
        while !self.at(&TokenKind::RBracket) {
            if self.at(&TokenKind::DotDot) {
                let start = self.bump().span.start;
                let name = if matches!(self.peek_token().kind, TokenKind::Ident(_)) {
                    Some(self.ident()?)
                } else {
                    None
                };
                items.push(Pattern {
                    kind: PatternKind::Rest(name),
                    span: self.span_from(start),
                });
            } else {
                items.push(self.pattern()?);
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.close(&TokenKind::RBracket)?;
        let rests: Vec<&Pattern> = items
            .iter()
            .filter(|item| matches!(item.kind, PatternKind::Rest(_)))
            .collect();
        if let Some(second) = rests.get(1) {
            let span = second.span;
            self.report(
                "E076",
                span,
                "a list pattern has at most one `..`".to_string(),
            );
        } else if let Some(named) = items[..items.len().saturating_sub(1)]
            .iter()
            .find(|item| matches!(item.kind, PatternKind::Rest(Some(_))))
        {
            let span = named.span;
            self.report(
                "E076",
                span,
                "`..name` must be the last element of a list pattern".to_string(),
            );
        }
        Ok(items)
    }

    /// One alternative, or `start..=end` between two bounds. The checker
    /// and typer decide what a bound may be (`docs/slice.md` section 13.7).
    fn pattern_alternative(&mut self) -> PResult<Pattern> {
        let start = self.pattern_atom()?;
        if self.at(&TokenKind::DotDot) {
            let span = self.peek_token().span;
            self.report(
                "E001",
                span,
                "a range pattern includes its end: write `..=`".to_string(),
            );
            return Err(Fail);
        }
        if !self.eat(&TokenKind::DotDotEq) {
            return Ok(start);
        }
        let end = self.pattern_atom()?;
        Ok(Pattern {
            span: Span::new(start.span.start, end.span.end),
            kind: PatternKind::Range {
                start: Box::new(start),
                end: Box::new(end),
            },
        })
    }

    fn pattern_atom(&mut self) -> PResult<Pattern> {
        let token = self.peek_token();
        let start = token.span.start;
        let kind = match &token.kind {
            TokenKind::KwTrue | TokenKind::KwFalse => {
                self.bump();
                PatternKind::Bool(token.kind == TokenKind::KwTrue)
            }
            TokenKind::LParen => {
                let (mut items, comma) = self.pattern_list(TokenKind::LParen, TokenKind::RParen)?;
                if items.len() == 1 {
                    if comma {
                        self.report(
                            "E001",
                            self.span_from(start),
                            "a tuple pattern has at least two elements".to_string(),
                        );
                        return Err(Fail);
                    }
                    return Ok(items.remove(0));
                }
                PatternKind::Tuple(items)
            }
            TokenKind::LBracket => PatternKind::List(self.list_pattern_items()?),
            TokenKind::Int(i64::MIN) => return Err(self.int_too_large(token)),
            TokenKind::Int(value) => {
                self.bump();
                PatternKind::Int(*value)
            }
            TokenKind::Minus if matches!(self.peek_second(), TokenKind::Int(_)) => {
                self.bump();
                let TokenKind::Int(value) = self.bump().kind else {
                    unreachable!("peeked an integer")
                };
                PatternKind::Int(value.wrapping_neg())
            }
            TokenKind::Str(value) => {
                self.bump();
                PatternKind::Str(value.clone())
            }
            TokenKind::Ident(name) if name == "_" => {
                self.bump();
                PatternKind::Wildcard
            }
            TokenKind::Ident(_) => {
                let mut path = vec![self.ident()?];
                while self.eat(&TokenKind::Dot) {
                    path.push(self.ident()?);
                }
                let mut args = Vec::new();
                let has_args = self.at(&TokenKind::LParen);
                if has_args {
                    self.open(&TokenKind::LParen, true)?;
                    args.push(self.pattern()?);
                    while self.eat(&TokenKind::Comma) {
                        args.push(self.pattern()?);
                    }
                    self.close(&TokenKind::RParen)?;
                }
                if path.len() == 1 && !has_args && !path[0].is_uppercase() {
                    PatternKind::Binding(path.remove(0))
                } else {
                    PatternKind::Variant { path, args }
                }
            }
            _ => return Err(self.expected("pattern")),
        };
        Ok(Pattern {
            kind,
            span: self.span_from(start),
        })
    }
}

/// A pattern `let` and `var` accept: a binding, `_` or a tuple of those.
/// Whether a token can begin an operand, so a `%` before it is the
/// remainder operator and not a percent unit.
fn starts_operand(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Ident(_)
            | TokenKind::Int(_)
            | TokenKind::Decimal(_)
            | TokenKind::Str(_)
            | TokenKind::Template(_)
            | TokenKind::ByteStr(_)
            | TokenKind::LParen
            | TokenKind::LBracket
            | TokenKind::LBrace
            | TokenKind::Minus
            | TokenKind::Bang
            | TokenKind::Pipe
            | TokenKind::OrOr
            | TokenKind::KwTrue
            | TokenKind::KwFalse
            | TokenKind::KwMatch
            | TokenKind::KwIf
            | TokenKind::KwReturn
            | TokenKind::KwSpawn
    )
}

fn irrefutable(pattern: &Pattern) -> bool {
    match &pattern.kind {
        PatternKind::Wildcard | PatternKind::Binding(_) => true,
        PatternKind::Tuple(items) => items.iter().all(irrefutable),
        _ => false,
    }
}

fn binary(op: BinaryOp, left: Expr, right: Expr, span: Span) -> Expr {
    Expr {
        kind: ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        },
        span,
    }
}

/// `x` or `x.a.b` as an assignment target; `None` for anything else.
fn place(expr: &Expr) -> Option<Place> {
    let mut fields = Vec::new();
    let mut current = expr;
    loop {
        match &current.kind {
            ExprKind::Field { receiver, name } => {
                fields.push(name.clone());
                current = receiver;
            }
            ExprKind::Name(base) => {
                fields.reverse();
                return Some(Place {
                    base: base.clone(),
                    fields,
                    span: expr.span,
                });
            }
            _ => return None,
        }
    }
}
