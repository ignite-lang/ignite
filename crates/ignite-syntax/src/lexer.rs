//! Tokens of the Ignite slice (`docs/slice.md` section 2).

use crate::ast::{FileId, Span};
use crate::diagnostic::Diagnostic;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Ident(String),
    Int(i64),
    Str(String),
    /// A run of one or more line breaks.
    Newline,
    Eof,
    KwPackage,
    KwImport,
    KwType,
    KwEnum,
    KwFn,
    KwSystem,
    KwProtocol,
    KwConst,
    KwResource,
    KwComponent,
    KwContract,
    KwPolicy,
    KwAssembly,
    KwLet,
    KwVar,
    KwReturn,
    KwMatch,
    KwIf,
    KwElse,
    KwWhile,
    KwLoop,
    KwFor,
    KwIn,
    KwBreak,
    KwContinue,
    KwAwait,
    KwSpawn,
    KwRequires,
    KwTest,
    KwAssert,
    KwTrue,
    KwFalse,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Lt,
    Gt,
    Comma,
    Dot,
    Colon,
    Assign,
    FatArrow,
    Arrow,
    Question,
    Bang,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    NotEq,
    LtEq,
    GtEq,
    AndAnd,
    OrOr,
    /// `;`, the statement separator of `docs/slice.md` section 3.
    Semicolon,
    /// `|`, between the alternatives of an or-pattern.
    Pipe,
    /// `b"..."`: the bytes of a byte-string literal.
    ByteStr(Vec<u8>),
    /// A string literal with at least one `{expr}` interpolation, in source
    /// order. Literals without interpolation are [`TokenKind::Str`].
    Template(Vec<TemplatePart>),
    /// `..`, before the base of a record update `R { f = e, ..base }`
    /// (`docs/slice.md` section 13).
    DotDot,
    /// `..=`, between the bounds of a range pattern (`docs/slice.md`
    /// section 13).
    DotDotEq,
    /// `99.9`: a decimal literal, only lexed directly before `%` or a unit
    /// name (`docs/slice.md` section 14.1); the digits as written without
    /// `_`.
    Decimal(String),
}

/// A piece of an interpolated string literal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplatePart {
    /// Literal text, escapes and `{{`/`}}` already decoded; never empty.
    Text(String),
    /// The tokens between `{` and `}`, spans in the whole file, ending with
    /// an `Eof` token at the closing `}`.
    Expr(Vec<Token>),
}

impl TokenKind {
    /// How the token is named in a diagnostic: `` `fn` ``, `identifier `x``,
    /// `newline`, `end of file`.
    pub fn describe(&self) -> String {
        match self {
            TokenKind::Ident(name) => format!("identifier `{name}`"),
            TokenKind::Int(value) => format!("integer `{value}`"),
            TokenKind::Decimal(value) => format!("decimal `{value}`"),
            TokenKind::Str(_) | TokenKind::Template(_) => "string literal".to_string(),
            TokenKind::ByteStr(_) => "byte string literal".to_string(),
            TokenKind::Newline => "newline".to_string(),
            TokenKind::Eof => "end of file".to_string(),
            other => format!("`{}`", other.symbol().unwrap_or("?")),
        }
    }

    /// Source text of a keyword or punctuation token; `None` for the others.
    pub fn symbol(&self) -> Option<&'static str> {
        Some(match self {
            TokenKind::KwPackage => "package",
            TokenKind::KwImport => "import",
            TokenKind::KwType => "type",
            TokenKind::KwEnum => "enum",
            TokenKind::KwFn => "fn",
            TokenKind::KwSystem => "system",
            TokenKind::KwProtocol => "protocol",
            TokenKind::KwConst => "const",
            TokenKind::KwResource => "resource",
            TokenKind::KwComponent => "component",
            TokenKind::KwContract => "contract",
            TokenKind::KwPolicy => "policy",
            TokenKind::KwAssembly => "assembly",
            TokenKind::KwLet => "let",
            TokenKind::KwVar => "var",
            TokenKind::KwReturn => "return",
            TokenKind::KwMatch => "match",
            TokenKind::KwIf => "if",
            TokenKind::KwElse => "else",
            TokenKind::KwWhile => "while",
            TokenKind::KwLoop => "loop",
            TokenKind::KwFor => "for",
            TokenKind::KwIn => "in",
            TokenKind::KwBreak => "break",
            TokenKind::KwContinue => "continue",
            TokenKind::KwAwait => "await",
            TokenKind::KwSpawn => "spawn",
            TokenKind::KwRequires => "requires",
            TokenKind::KwTest => "test",
            TokenKind::KwAssert => "assert",
            TokenKind::KwTrue => "true",
            TokenKind::KwFalse => "false",
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::LBrace => "{",
            TokenKind::RBrace => "}",
            TokenKind::LBracket => "[",
            TokenKind::RBracket => "]",
            TokenKind::Lt => "<",
            TokenKind::Gt => ">",
            TokenKind::Comma => ",",
            TokenKind::Dot => ".",
            TokenKind::Colon => ":",
            TokenKind::Assign => "=",
            TokenKind::FatArrow => "=>",
            TokenKind::Arrow => "->",
            TokenKind::Question => "?",
            TokenKind::Bang => "!",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::EqEq => "==",
            TokenKind::NotEq => "!=",
            TokenKind::LtEq => "<=",
            TokenKind::GtEq => ">=",
            TokenKind::AndAnd => "&&",
            TokenKind::OrOr => "||",
            TokenKind::Semicolon => ";",
            TokenKind::Pipe => "|",
            TokenKind::DotDot => "..",
            TokenKind::DotDotEq => "..=",
            TokenKind::Ident(_)
            | TokenKind::Int(_)
            | TokenKind::Decimal(_)
            | TokenKind::Str(_)
            | TokenKind::ByteStr(_)
            | TokenKind::Template(_)
            | TokenKind::Newline
            | TokenKind::Eof => return None,
        })
    }

    fn keyword(word: &str) -> Option<TokenKind> {
        Some(match word {
            "package" => TokenKind::KwPackage,
            "import" => TokenKind::KwImport,
            "type" => TokenKind::KwType,
            "enum" => TokenKind::KwEnum,
            "fn" => TokenKind::KwFn,
            "system" => TokenKind::KwSystem,
            "protocol" => TokenKind::KwProtocol,
            "const" => TokenKind::KwConst,
            "resource" => TokenKind::KwResource,
            "component" => TokenKind::KwComponent,
            "contract" => TokenKind::KwContract,
            "policy" => TokenKind::KwPolicy,
            "assembly" => TokenKind::KwAssembly,
            "let" => TokenKind::KwLet,
            "var" => TokenKind::KwVar,
            "return" => TokenKind::KwReturn,
            "match" => TokenKind::KwMatch,
            "if" => TokenKind::KwIf,
            "else" => TokenKind::KwElse,
            "while" => TokenKind::KwWhile,
            "loop" => TokenKind::KwLoop,
            "for" => TokenKind::KwFor,
            "in" => TokenKind::KwIn,
            "break" => TokenKind::KwBreak,
            "continue" => TokenKind::KwContinue,
            "await" => TokenKind::KwAwait,
            "spawn" => TokenKind::KwSpawn,
            "requires" => TokenKind::KwRequires,
            "test" => TokenKind::KwTest,
            "assert" => TokenKind::KwAssert,
            "true" => TokenKind::KwTrue,
            "false" => TokenKind::KwFalse,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// Tokenize the whole text. The result always ends with an `Eof` token. An
/// unterminated string, an invalid escape, an integer out of range or an
/// unexpected character is `E001` at that position.
///
/// Comments and whitespace produce no token. Line breaks separated only by
/// whitespace and comments merge into one `Newline` token spanning them.
/// `\xNN` escapes are limited to ASCII (`00` to `7F`) because strings are
/// UTF-8 text.
///
/// A string with `{e}` parts is one [`TokenKind::Template`]: its literal
/// pieces (`{{` and `}}` already unescaped) and, per part, the tokens of
/// `e` with spans into `text`, ending in `Eof`. A lone `}`, an empty `{}`
/// and a line break inside `{ }` are `E001`; `e` may contain string
/// literals, themselves interpolated, up to [`MAX_INTERPOLATION`] levels
/// deep (E054 past it). `b'c'` is an [`TokenKind::Int`]; `b".."` is a
/// [`TokenKind::ByteStr`] (ASCII and escapes, `\xNN` up to `FF`). Raw
/// strings `r"..."`, `r#"..."#` (any number of `#`) and `br"..."` have no
/// escapes and no interpolation and may span lines; they are
/// [`TokenKind::Str`] and [`TokenKind::ByteStr`].
///
/// The literal `9223372036854775808` (2^63) is [`TokenKind::Int`] with the
/// value `i64::MIN`: no other literal has that value, and the parser accepts
/// it only directly after a prefix `-` (`-9223372036854775808`).
pub fn lex(text: &str) -> Result<Vec<Token>, Diagnostic> {
    lex_at(text, 0)
}

/// The most string literals an interpolation may nest; one more is E054.
pub const MAX_INTERPOLATION: usize = 32;

/// Put every span of `tokens`, those inside template parts included, in
/// `file`. [`crate::parse`] does it with the file's [`FileId::of_path`];
/// the parser takes the file of its spans and node ids from the first
/// token.
pub fn in_file(tokens: &mut [Token], file: FileId) {
    for token in tokens {
        token.span.file = file;
        if let TokenKind::Template(parts) = &mut token.kind {
            for part in parts {
                if let TemplatePart::Expr(inner) = part {
                    in_file(inner, file);
                }
            }
        }
    }
}

fn lex_at(text: &str, depth: usize) -> Result<Vec<Token>, Diagnostic> {
    let mut tokens = Vec::new();
    lex_into(text, &mut tokens, depth)?;
    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span::new(text.len(), text.len()),
    });
    Ok(tokens)
}

/// `tokens` with every span, those inside template parts included, moved
/// by `by` bytes.
fn shift(tokens: Vec<Token>, by: usize) -> Vec<Token> {
    tokens
        .into_iter()
        .map(|t| Token {
            kind: match t.kind {
                TokenKind::Template(parts) => TokenKind::Template(
                    parts
                        .into_iter()
                        .map(|p| match p {
                            TemplatePart::Expr(inner) => TemplatePart::Expr(shift(inner, by)),
                            text => text,
                        })
                        .collect(),
                ),
                kind => kind,
            },
            span: Span::new(t.span.start + by, t.span.end + by),
        })
        .collect()
}

/// Like [`lex`], but on an error return the tokens before it, ending with
/// an `Eof` token at the start of the error, together with the error, so
/// the parser can still report the syntax errors before it.
pub fn lex_partial(text: &str) -> (Vec<Token>, Option<Diagnostic>) {
    let mut tokens = Vec::new();
    let error = lex_into(text, &mut tokens, 0).err();
    let end = error.as_ref().map_or(text.len(), |d| d.span.start);
    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span::new(end, end),
    });
    (tokens, error)
}

fn lex_into(text: &str, tokens: &mut Vec<Token>, depth: usize) -> Result<(), Diagnostic> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        let start = i;
        let raw = match c {
            b'r' => raw_string_hashes(bytes, i + 1),
            b'b' if bytes.get(i + 1) == Some(&b'r') => raw_string_hashes(bytes, i + 2),
            _ => None,
        };
        if let Some(hashes) = raw {
            let (kind, end) = lex_raw_string(text, start, hashes)?;
            i = end;
            tokens.push(Token {
                kind,
                span: Span::new(start, end),
            });
            continue;
        }
        match c {
            b' ' | b'\t' | b'\r' => i += 1,
            b'\n' => {
                i += 1;
                match tokens.last_mut() {
                    Some(last) if last.kind == TokenKind::Newline => last.span.end = i,
                    _ => tokens.push(Token {
                        kind: TokenKind::Newline,
                        span: Span::new(start, i),
                    }),
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'"' => {
                let (kind, end) = lex_string(text, start, depth)?;
                i = end;
                tokens.push(Token {
                    kind,
                    span: Span::new(start, end),
                });
            }
            b'b' if matches!(bytes.get(i + 1), Some(b'\'' | b'"')) => {
                let (kind, end) = if bytes[i + 1] == b'\'' {
                    lex_byte_char(text, start)?
                } else {
                    lex_byte_string(text, start)?
                };
                i = end;
                tokens.push(Token {
                    kind,
                    span: Span::new(start, end),
                });
            }
            b'0'..=b'9' => {
                let after_dot = tokens.last().is_some_and(|t| t.kind == TokenKind::Dot);
                if !after_dot && let Some((digits, end)) = decimal_before_unit(text, start) {
                    i = end;
                    tokens.push(Token {
                        kind: TokenKind::Decimal(digits),
                        span: Span::new(start, end),
                    });
                    continue;
                }
                let (value, end) = lex_int(text, start)?;
                i = end;
                tokens.push(Token {
                    kind: TokenKind::Int(value),
                    span: Span::new(start, end),
                });
            }
            b'A'..=b'Z' | b'a'..=b'z' | b'_' => {
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let word = &text[start..i];
                let kind =
                    TokenKind::keyword(word).unwrap_or_else(|| TokenKind::Ident(word.to_string()));
                tokens.push(Token {
                    kind,
                    span: Span::new(start, i),
                });
            }
            _ => {
                let next = bytes.get(i + 1).copied();
                let (kind, len) = match (c, next) {
                    (b'.', Some(b'.')) if bytes.get(i + 2) == Some(&b'=') => {
                        (TokenKind::DotDotEq, 3)
                    }
                    (b'.', Some(b'.')) => (TokenKind::DotDot, 2),
                    (b'=', Some(b'=')) => (TokenKind::EqEq, 2),
                    (b'=', Some(b'>')) => (TokenKind::FatArrow, 2),
                    (b'-', Some(b'>')) => (TokenKind::Arrow, 2),
                    (b'!', Some(b'=')) => (TokenKind::NotEq, 2),
                    (b'<', Some(b'=')) => (TokenKind::LtEq, 2),
                    (b'>', Some(b'=')) => (TokenKind::GtEq, 2),
                    (b'&', Some(b'&')) => (TokenKind::AndAnd, 2),
                    (b'|', Some(b'|')) => (TokenKind::OrOr, 2),
                    (b'|', _) => (TokenKind::Pipe, 1),
                    (b'(', _) => (TokenKind::LParen, 1),
                    (b')', _) => (TokenKind::RParen, 1),
                    (b'{', _) => (TokenKind::LBrace, 1),
                    (b'}', _) => (TokenKind::RBrace, 1),
                    (b'[', _) => (TokenKind::LBracket, 1),
                    (b']', _) => (TokenKind::RBracket, 1),
                    (b'<', _) => (TokenKind::Lt, 1),
                    (b'>', _) => (TokenKind::Gt, 1),
                    (b',', _) => (TokenKind::Comma, 1),
                    (b'.', _) => (TokenKind::Dot, 1),
                    (b':', _) => (TokenKind::Colon, 1),
                    (b';', _) => (TokenKind::Semicolon, 1),
                    (b'=', _) => (TokenKind::Assign, 1),
                    (b'?', _) => (TokenKind::Question, 1),
                    (b'!', _) => (TokenKind::Bang, 1),
                    (b'+', _) => (TokenKind::Plus, 1),
                    (b'-', _) => (TokenKind::Minus, 1),
                    (b'*', _) => (TokenKind::Star, 1),
                    (b'/', _) => (TokenKind::Slash, 1),
                    (b'%', _) => (TokenKind::Percent, 1),
                    _ => {
                        let ch = text[start..].chars().next().unwrap_or('?');
                        return Err(Diagnostic::new(
                            "E001",
                            Span::new(start, start + ch.len_utf8()),
                            format!("unexpected character `{}`", ch.escape_default()),
                        ));
                    }
                };
                i += len;
                tokens.push(Token {
                    kind,
                    span: Span::new(start, i),
                });
            }
        }
    }
    Ok(())
}

/// `digits '.' digits` directly followed by `%` or, after spaces on the
/// same line, by an identifier: a decimal literal before a unit
/// (`docs/slice.md` section 14.1). Returns the digits without `_` and the
/// end of the number. `None` for anything else, which lexes as an integer.
fn decimal_before_unit(text: &str, start: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    let digits = |mut at: usize| {
        while at < bytes.len() && (bytes[at].is_ascii_digit() || bytes[at] == b'_') {
            at += 1;
        }
        at
    };
    let dot = digits(start);
    if bytes.get(dot) != Some(&b'.') || !bytes.get(dot + 1).is_some_and(u8::is_ascii_digit) {
        return None;
    }
    let end = digits(dot + 1);
    if bytes.get(end).is_some_and(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    let mut next = end;
    while matches!(bytes.get(next), Some(b' ' | b'\t')) {
        next += 1;
    }
    let unit = match bytes.get(next) {
        Some(b'%') => next == end,
        Some(b) => b.is_ascii_alphabetic(),
        None => false,
    };
    if !unit {
        return None;
    }
    let literal: String = text[start..end].chars().filter(|&c| c != '_').collect();
    Some((literal, end))
}

fn lex_int(text: &str, start: usize) -> Result<(i64, usize), Diagnostic> {
    let bytes = text.as_bytes();
    let hex = bytes[start] == b'0' && matches!(bytes.get(start + 1), Some(b'x' | b'X'));
    let digits_start = if hex { start + 2 } else { start };
    let mut end = digits_start;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    let span = Span::new(start, end);
    let raw = &text[digits_start..end];
    let radix = if hex { 16 } else { 10 };
    let digits: String = raw.chars().filter(|&c| c != '_').collect();
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return Err(Diagnostic::new(
            "E001",
            span,
            format!("invalid integer literal `{}`", &text[start..end]),
        ));
    }
    match u64::from_str_radix(&digits, radix) {
        Ok(value) if value <= i64::MAX as u64 => Ok((value as i64, end)),
        Ok(value) if value == 1u64 << 63 => Ok((i64::MIN, end)),
        _ => Err(Diagnostic::new(
            "E001",
            span,
            format!(
                "integer literal `{}` does not fit in 64 bits",
                &text[start..end]
            ),
        )),
    }
}

/// Where an escape appears: what it may contain and how a message names it.
#[derive(Clone, Copy)]
enum Quoted {
    /// `"..."`: no `\'`, `\xNN` up to `7F`.
    Text,
    /// `b'..'` and `b".."`: `\'` too, `\xNN` up to `FF`.
    Byte,
    ByteString,
}

impl Quoted {
    fn max_hex(self) -> u32 {
        match self {
            Quoted::Text => 0x7F,
            Quoted::Byte | Quoted::ByteString => 0xFF,
        }
    }

    fn what(self) -> &'static str {
        match self {
            Quoted::Text => "string",
            Quoted::Byte => "byte literal",
            Quoted::ByteString => "byte string",
        }
    }
}

/// A decoded escape: the character it stands for, `Ok(None)` when the line
/// or the text ends after the backslash, or `Err` with the end of the bad
/// escape and the message.
fn escape(
    chars: &mut std::iter::Peekable<impl Iterator<Item = (usize, char)>>,
    offset: usize,
    quoted: Quoted,
) -> Result<Option<u32>, (usize, String)> {
    let max_hex = quoted.max_hex();
    let Some((_, e)) = chars.next() else {
        return Ok(None);
    };
    let code = match e {
        '\n' | '\r' => return Ok(None),
        'n' => '\n' as u32,
        'r' => '\r' as u32,
        't' => '\t' as u32,
        'v' => 0x0B,
        'f' => 0x0C,
        '0' => 0,
        '\\' => '\\' as u32,
        '"' => '"' as u32,
        '\'' if !matches!(quoted, Quoted::Text) => '\'' as u32,
        'u' if matches!(quoted, Quoted::Text) => {
            let malformed = |end: usize| {
                Err((
                    end,
                    "`\\u` escape must be `\\u{` with 1 to 6 hexadecimal digits and `}`"
                        .to_string(),
                ))
            };
            if !chars.peek().is_some_and(|&(_, c)| c == '{') {
                return malformed(offset + 2);
            }
            chars.next();
            let mut digits = String::new();
            let mut end = offset + 3;
            loop {
                match chars.peek().copied() {
                    Some((o, '}')) => {
                        chars.next();
                        end = o + 1;
                        break;
                    }
                    Some((o, c)) if c.is_ascii_hexdigit() && digits.len() < 6 => {
                        chars.next();
                        digits.push(c);
                        end = o + 1;
                    }
                    _ => return malformed(end),
                }
            }
            if digits.is_empty() {
                return malformed(end);
            }
            let code = u32::from_str_radix(&digits, 16).unwrap_or(u32::MAX);
            if char::from_u32(code).is_none() {
                return Err((
                    end,
                    format!("`\\u{{{digits}}}` is not a Unicode scalar value"),
                ));
            }
            return Ok(Some(code));
        }
        'x' => {
            let hi = chars.next();
            let lo = chars.next();
            let code = match (hi, lo) {
                (Some((_, h)), Some((_, l))) => {
                    h.to_digit(16).zip(l.to_digit(16)).map(|(h, l)| h * 16 + l)
                }
                _ => None,
            };
            return match code {
                Some(code) if code <= max_hex => Ok(Some(code)),
                Some(_) => Err((
                    offset + 4,
                    format!("`\\x` escape must be in the range `00` to `{max_hex:02X}`"),
                )),
                None => Err((
                    offset + 2,
                    "`\\x` escape needs two hexadecimal digits".to_string(),
                )),
            };
        }
        other => {
            let shown = if other.is_control() {
                format!("`\\` followed by U+{:04X}", other as u32)
            } else {
                format!("`\\{other}`")
            };
            return Err((
                offset + 1 + other.len_utf8(),
                format!("unknown escape {shown} in {}", quoted.what()),
            ));
        }
    };
    Ok(Some(code))
}

/// A string literal starting at the `"` at `start`: [`TokenKind::Str`], or
/// [`TokenKind::Template`] when it has a `{expr}` part. `{{` and `}}` are
/// literal braces; a lone `}` is an error. The expression inside `{ }` may
/// not contain a line break; it may contain string literals, `depth` being
/// the number of interpolations around this literal.
fn lex_string(text: &str, start: usize, depth: usize) -> Result<(TokenKind, usize), Diagnostic> {
    if depth > MAX_INTERPOLATION {
        return Err(Diagnostic::new(
            "E054",
            Span::new(start, start + 1),
            format!(
                "string literals nested deeper than {MAX_INTERPOLATION} interpolations; bind the inner one to a name"
            ),
        ));
    }
    let mut parts: Vec<TemplatePart> = Vec::new();
    let mut value = String::new();
    let mut chars = text[start + 1..]
        .char_indices()
        .map(|(o, c)| (start + 1 + o, c))
        .peekable();
    let unterminated =
        || Diagnostic::new("E001", Span::new(start, start + 1), "unterminated string");
    loop {
        let Some((offset, c)) = chars.next() else {
            return Err(unterminated());
        };
        match c {
            '"' => {
                let end = offset + 1;
                if parts.is_empty() {
                    return Ok((TokenKind::Str(value), end));
                }
                if !value.is_empty() {
                    parts.push(TemplatePart::Text(value));
                }
                return Ok((TokenKind::Template(parts), end));
            }
            '\n' => return Err(unterminated()),
            '\\' => match escape(&mut chars, offset, Quoted::Text) {
                Ok(Some(code)) => value.push(char::from_u32(code).unwrap_or('\0')),
                Ok(None) => return Err(unterminated()),
                Err((end, message)) => {
                    return Err(Diagnostic::new("E001", Span::new(offset, end), message));
                }
            },
            '{' if chars.peek().is_some_and(|&(_, n)| n == '{') => {
                chars.next();
                value.push('{');
            }
            '}' if chars.peek().is_some_and(|&(_, n)| n == '}') => {
                chars.next();
                value.push('}');
            }
            '}' => {
                return Err(Diagnostic::new(
                    "E001",
                    Span::new(offset, offset + 1),
                    "unmatched `}` in string; write `}}` for a literal brace",
                ));
            }
            '{' => {
                let inner_start = offset + 1;
                let inner_end = interpolation_end(text, offset, depth)?;
                while chars.peek().is_some_and(|&(o, _)| o <= inner_end) {
                    chars.next();
                }
                let inner = &text[inner_start..inner_end];
                if inner.trim().is_empty() {
                    return Err(Diagnostic::new(
                        "E001",
                        Span::new(offset, inner_end + 1),
                        "empty `{}` in string; write `{{}}` for literal braces",
                    ));
                }
                let tokens = lex_at(inner, depth + 1).map_err(|mut d| {
                    d.span = Span::new(d.span.start + inner_start, d.span.end + inner_start);
                    d
                })?;
                let tokens = shift(tokens, inner_start);
                if !value.is_empty() {
                    parts.push(TemplatePart::Text(std::mem::take(&mut value)));
                }
                parts.push(TemplatePart::Expr(tokens));
            }
            other => value.push(other),
        }
    }
}

/// The offset of the `}` that closes the interpolation whose `{` is at
/// `open`, inside a string literal nested `depth` interpolations deep.
/// Nested `{ }` pairs, byte literals (`b'}'`, `b'"'`) and string literals
/// (plain, byte, raw, themselves interpolated) are skipped. A line break or
/// the end of the text first is "unclosed `{`".
fn interpolation_end(text: &str, open: usize, depth: usize) -> Result<usize, Diagnostic> {
    let bytes = text.as_bytes();
    let unclosed = || {
        Diagnostic::new(
            "E001",
            Span::new(open, open + 1),
            "unclosed `{` in interpolation; write `{{` for a literal brace",
        )
    };
    let boundary =
        |i: usize| i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
    let mut braces = 0usize;
    let mut i = open + 1;
    while i < bytes.len() {
        if boundary(i) {
            let raw = match bytes[i] {
                b'r' => raw_string_hashes(bytes, i + 1),
                b'b' if bytes.get(i + 1) == Some(&b'r') => raw_string_hashes(bytes, i + 2),
                _ => None,
            };
            if let Some(hashes) = raw {
                let (_, end) = lex_raw_string(text, i, hashes)?;
                i = end;
                continue;
            }
        }
        match bytes[i] {
            b'}' if braces == 0 => return Ok(i),
            b'}' => braces -= 1,
            b'{' => braces += 1,
            b'\n' => return Err(unclosed()),
            b'"' => {
                let byte_string = i > 0 && bytes[i - 1] == b'b' && boundary(i - 1);
                i = string_end(text, i, depth + 1, byte_string).ok_or_else(unclosed)??;
                continue;
            }
            b'b' if bytes.get(i + 1) == Some(&b'\'') && boundary(i) => {
                let mut j = i + 2;
                j += match (bytes.get(j), bytes.get(j + 1)) {
                    (Some(b'\\'), Some(b'x')) => 4,
                    (Some(b'\\'), _) => 2,
                    _ => 1,
                };
                if bytes.get(j) == Some(&b'\'') {
                    i = j + 1;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    Err(unclosed())
}

/// The offset after the closing `"` of the string literal whose opening
/// `"` is at `start`, nested `depth` interpolations deep; a byte string has
/// no interpolation. `None` when a line break or the end of the text comes
/// first; `Some(Err)` for an error inside one of its interpolations.
fn string_end(
    text: &str,
    start: usize,
    depth: usize,
    byte_string: bool,
) -> Option<Result<usize, Diagnostic>> {
    if depth > MAX_INTERPOLATION {
        return Some(Err(Diagnostic::new(
            "E054",
            Span::new(start, start + 1),
            format!(
                "string literals nested deeper than {MAX_INTERPOLATION} interpolations; bind the inner one to a name"
            ),
        )));
    }
    let bytes = text.as_bytes();
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => return Some(Ok(i + 1)),
            b'\n' => return None,
            b'\\' => {
                if matches!(bytes.get(i + 1), None | Some(b'\n')) {
                    return None;
                }
                i += 2;
                continue;
            }
            b'{' | b'}' if !byte_string && bytes.get(i + 1) == Some(&bytes[i]) => {
                i += 2;
                continue;
            }
            b'{' if !byte_string => match interpolation_end(text, i, depth) {
                Ok(close) => {
                    i = close + 1;
                    continue;
                }
                Err(d) if d.code == "E054" => return Some(Err(d)),
                Err(_) => return None,
            },
            _ => {}
        }
        i += 1;
    }
    None
}

/// After an `r` (or `br`) at `at`: the number of `#` before the opening `"`
/// of a raw string, or `None` when no raw string starts there.
fn raw_string_hashes(bytes: &[u8], at: usize) -> Option<usize> {
    let mut i = at;
    while bytes.get(i) == Some(&b'#') {
        i += 1;
    }
    (bytes.get(i) == Some(&b'"')).then_some(i - at)
}

/// A raw string starting at `start` (the `r` or the `b` of `br`) with
/// `hashes` `#` around its quotes: the text up to the first `"` followed by
/// as many `#`, without escapes or interpolation, line breaks included. A
/// raw byte string holds ASCII only.
fn lex_raw_string(
    text: &str,
    start: usize,
    hashes: usize,
) -> Result<(TokenKind, usize), Diagnostic> {
    let bytes = text.as_bytes();
    let byte_string = bytes[start] == b'b';
    let open = start + usize::from(byte_string) + 1 + hashes + 1;
    let mut i = open;
    while i < bytes.len() {
        if bytes[i] == b'"'
            && bytes[i + 1..]
                .iter()
                .take(hashes)
                .filter(|&&b| b == b'#')
                .count()
                == hashes
        {
            let body = &text[open..i];
            let end = i + 1 + hashes;
            if !byte_string {
                return Ok((TokenKind::Str(body.to_string()), end));
            }
            if let Some(o) = body.bytes().position(|b| !b.is_ascii()) {
                let c = body[o..].chars().next().unwrap_or('?');
                return Err(Diagnostic::new(
                    "E001",
                    Span::new(open + o, open + o + c.len_utf8()),
                    format!("raw byte string contains the non-ASCII character `{c}`"),
                ));
            }
            return Ok((TokenKind::ByteStr(body.as_bytes().to_vec()), end));
        }
        i += 1;
    }
    Err(Diagnostic::new(
        "E001",
        Span::new(start, open),
        "unterminated raw string",
    ))
}

/// `b'c'`: one ASCII character or escape (`\xNN` up to `FF`), an
/// [`TokenKind::Int`] of its byte value.
fn lex_byte_char(text: &str, start: usize) -> Result<(TokenKind, usize), Diagnostic> {
    let mut chars = text[start + 2..]
        .char_indices()
        .map(|(o, c)| (start + 2 + o, c))
        .peekable();
    let malformed = |end: usize| {
        Diagnostic::new(
            "E001",
            Span::new(start, end),
            "malformed byte literal; write one ASCII character or escape, e.g. `b' '` or `b'\\n'`",
        )
    };
    let code = match chars.next() {
        Some((offset, '\\')) => match escape(&mut chars, offset, Quoted::Byte) {
            Ok(Some(code)) => code,
            Ok(None) => return Err(malformed(offset + 1)),
            Err((end, message)) => {
                return Err(Diagnostic::new("E001", Span::new(offset, end), message));
            }
        },
        Some((offset, c)) if c == '\'' || c == '\n' || !c.is_ascii() => {
            return Err(malformed(offset + c.len_utf8()));
        }
        Some((_, c)) => c as u32,
        None => return Err(malformed(text.len())),
    };
    match chars.next() {
        Some((offset, '\'')) => Ok((TokenKind::Int(i64::from(code)), offset + 1)),
        Some((offset, c)) => Err(malformed(offset + c.len_utf8())),
        None => Err(malformed(text.len())),
    }
}

/// `b"..."`: ASCII characters and escapes (`\xNN` up to `FF`), no
/// interpolation.
fn lex_byte_string(text: &str, start: usize) -> Result<(TokenKind, usize), Diagnostic> {
    let mut value = Vec::new();
    let mut chars = text[start + 2..]
        .char_indices()
        .map(|(o, c)| (start + 2 + o, c))
        .peekable();
    let unterminated = || {
        Diagnostic::new(
            "E001",
            Span::new(start, start + 2),
            "unterminated byte string",
        )
    };
    loop {
        let Some((offset, c)) = chars.next() else {
            return Err(unterminated());
        };
        match c {
            '"' => return Ok((TokenKind::ByteStr(value), offset + 1)),
            '\n' => return Err(unterminated()),
            '\\' => match escape(&mut chars, offset, Quoted::ByteString) {
                Ok(Some(code)) => value.push(code as u8),
                Ok(None) => return Err(unterminated()),
                Err((end, message)) => {
                    return Err(Diagnostic::new("E001", Span::new(offset, end), message));
                }
            },
            c if c.is_ascii() => value.push(c as u8),
            c => {
                return Err(Diagnostic::new(
                    "E001",
                    Span::new(offset, offset + c.len_utf8()),
                    format!(
                        "byte string contains the non-ASCII character `{c}`; write its bytes as `\\xNN`"
                    ),
                ));
            }
        }
    }
}
