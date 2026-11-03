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
    let boundary = |i: usize| i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
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
fn lex_raw_string(text: &str, start: usize, hashes: usize) -> Result<(TokenKind, usize), Diagnostic> {
    let bytes = text.as_bytes();
    let byte_string = bytes[start] == b'b';
    let open = start + usize::from(byte_string) + 1 + hashes + 1;
    let mut i = open;
    while i < bytes.len() {
        if bytes[i] == b'"' && bytes[i + 1..].iter().take(hashes).filter(|&&b| b == b'#').count() == hashes
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
