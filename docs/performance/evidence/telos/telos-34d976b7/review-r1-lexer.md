# R1 review: lexer and token consumption (`crates/telos-syntax`)

Scope: `crates/telos-syntax/src/lexer.rs`, the parser's token handling in `parser.rs`, `diagnostic.rs` (spans, `line_col`), `lib.rs`. Read-only; nothing was built or run (cargo is not allowed for this review), so every threshold below is a code-path finding, not a measured one.

Count: high 2, medium 1, low 5.

## Findings, most severe first

### 1. High: parser recursion has no depth bound (stack overflow from source text)

- `crates/telos-syntax/src/parser.rs:1271`: `let kind = wrap(Box::new(self.unary()?));`
- `crates/telos-syntax/src/parser.rs:1432-1433`: `while !self.at(&TokenKind::RParen) {` / `items.push(self.expr()?);`

Prefix `!` and `-` recurse through `unary` once per operator. `(`, `[`, `{`, `match` and `if` re-enter `expr()`, which goes through the precedence ladder (`parser.rs:1144-1275`, about 12 frames per level). No depth counter exists in `telos-syntax` (grep finds none). The CLI parses on the main thread with no `stack_size` or thread spawn (`crates/telos/src/lib.rs:254-255`, called from lines 275 and 422).

Failure scenario: a function body holding about 40,000 `!` before a literal, or about 3,000 nested `(`, makes the process overflow the 8 MB main stack and abort with no diagnostic. The exact depth is suspected, not measured.

Severity: high under the rubric (crash), but the input is pathological. The lead may downgrade it to medium.
Confidence: confirmed path end to end; threshold suspected.

### 2. High: left-deep trees from a flat token run are walked recursively downstream

- `crates/telos-syntax/src/parser.rs:1154-1159`: `let mut left = next(self)?;` / `left = binary(op, left, right, self.span_from(start));`
- `crates/telos-syntax/src/parser.rs:1504-1506`: `Some(left) => binary(BinaryOp::Add, left, piece, span),`
- `crates/telos-types/src/typer.rs:2422`: `let left_ty = self.expr(left, Mode::Value(hint));`

The parser loop is iterative, so it does not overflow. It builds a left-deep tree of depth N for N operands, and an interpolated string with N parts gets the same shape. The typer then recurses on the left operand once per level. No depth guard exists in `telos-types`.

Failure scenario: `let n = 1 + 1 + ... + 1` with about 20,000 terms, or `const S: Text = "{a}{a}...{a}"` with about 20,000 interpolations (about 60 KB). The parse succeeds and the type checker overflows the stack. Threshold suspected, not measured.

I did not trace the walkers in `telos-check`, `telos-rustgen` or `telos-llvm`; they probably recurse the same way.

Severity: high under the rubric (crash), pathological input.
Confidence: tree shape and typer recursion confirmed; overflow threshold suspected.

### 3. Medium: misleading diagnostic for an unclosed `{` before the closing quote

- `crates/telos-syntax/src/lexer.rs:491-496`: `Some((o, '"')) => { return Err(Diagnostic::new("E001", Span::new(o, o + 1), "a string literal cannot appear inside `{ }` of a string; bind it to a name first"` 

The `{` arm treats any `"` as the start of a string literal, even when it is the outer string's closing quote. The newline and end-of-file branch (`lexer.rs:498-502`) already gives the right message, "unterminated `{` in string; write `{{` for a literal brace".

Failure scenario: `const G: Text = "Hello {name"` reports a string literal inside braces at the closing quote. No string literal is there. `"{"` and `"a {"` give the same wrong text.

Related: a backslash before a line break inside a string (`"abc\` then newline) reports "unknown escape `\n` in string" (`lexer.rs:425`), not "unterminated string".

Severity: medium (wrong diagnostic).
Confidence: confirmed by trace.

### 4. Low: interpolation scanner counts raw characters, not tokens

- `crates/telos-syntax/src/lexer.rs:484-505`: the `{` arm reads characters until a `}` at depth 0 (`Some(_) => {}`), with no knowledge of byte literals.
- `docs/slice.md:55`: "The text up to the matching `}` is lexed as tokens".

Failure scenario: `"{b'}'}"` takes the inner text `b'` and reports "malformed byte literal" at that `b'`. The spec says it is valid. `"{b'\"'}"` reports "a string literal cannot appear inside `{ }`" for a byte literal that holds a double quote.

Severity: low (edge case).
Confidence: confirmed by trace.

### 5. Low: byte strings reject `\'`, though the spec says they take the same escapes as byte characters

- `crates/telos-syntax/src/lexer.rs:594`: `'\\' => match escape(&mut chars, offset, '"', 0xFF) {`
- `crates/telos-syntax/src/lexer.rs:400`: `'\'' if quote == '\'' => '\'' as u32,`
- `docs/slice.md:59-61`: the `b'c'` escape list includes `\'`, and `b"..."` takes "the same escapes".

Failure scenario: `b"it\'s"` gives E001 "unknown escape `\'` in string". The spec says it is valid. The message also says "in string" for a byte string.

Severity: low.
Confidence: confirmed by trace.

### 6. Low: `-9223372036854775808` (i64::MIN) cannot be written, and the message blames the wrong thing

- `crates/telos-syntax/src/lexer.rs:367`: `match i64::from_str_radix(&digits, radix) {`
- `crates/telos-types/src/typer.rs:130-140`: `is_literal` treats `Unary { Neg, Int }` as a literal, so negative literals are designed in.

Failure scenario: `const MIN: Int = -9223372036854775808` gives E001 "integer literal `9223372036854775808` does not fit in 64 bits". The lexer rejects 2^63 before the parser sees the minus, so a valid `Int` value cannot be written. This may be deliberate; if it is, the message should say so.

Severity: low.
Confidence: confirmed by trace.

### 7. Low: a lexer error aborts the file, so no syntax errors are reported with it

- `crates/telos-syntax/src/lib.rs:30`: `let tokens = lexer::lex(&source.text).map_err(|d| vec![d])?;`
- `docs/slice.md:727`: "so one run reports several errors" (stated about parser recovery).

Failure scenario: a file with an unterminated string on line 3 and a missing `}` on line 400 reports only the lexer error. The parser error appears after the first is fixed.

Severity: low.
Confidence: confirmed by trace.

### 8. Low: diagnostic rendering and reporting cost grows with the square of the diagnostic count

- `crates/telos-syntax/src/diagnostic.rs:41-48`: `line_col` scans `text[..offset]` on every call.
- `crates/telos-syntax/src/diagnostic.rs:60-66`: `render_all` renders each diagnostic, so the cost is O(d x n).
- `crates/telos-syntax/src/parser.rs:237-241`: `report` scans every earlier diagnostic, so the cost is O(d^2).

Failure scenario: a 2 MB file with 50,000 top-level `fn` lines that lack a name gives one diagnostic per line, because recovery stops at each next `fn`. `render_all` visits about d x n/2, roughly 5 x 10^10 bytes, which is seconds to tens of seconds depending on the build profile. Realistic only for a generated broken file.

Severity: low.
Confidence: confirmed by trace; cost estimated, not measured.

## Checked, no finding

- Escapes: `\v` is 0x0B, `\f` is 0x0C, `\0` is NUL, `\xNN` is limited to `00` to `7F` in strings and `00` to `FF` in byte literals (`lexer.rs:382-430`). Covered by `crates/telos-syntax/tests/grammar.rs`. There are no `\u{...}` escapes in the spec, so `\u` is E001, which is consistent.
- Interpolation: `{{`, `}}`, `{}`, nested braces and a lone `}` (`lexer.rs:470-520`). Byte char and byte string values (`lexer.rs:539-600`).
- Numbers: i64::MAX accepted, 2^63 rejected, hex overflow, `0x` alone, `1_` accepted, `007` is 7 (decimal; the spec is silent on leading zeros), `1e5` and `123abc` are E001.
- UTF-8: every offset the lexer produces lands on a character boundary (ASCII delimiters or token ends). The `_` arm's `text[start..]` slices at a boundary. `line_col` cannot panic on a lexer or parser span. It would panic on an offset inside a multi-byte character, so it has no guard for spans from elsewhere (latent, not reachable as far as I traced).
- Unterminated literals at end of file (string, byte string, byte char, `{` in a string, trailing backslash) are all E001 at the opening position. No panic.
- Panics: no `unwrap`, `expect` or `panic!` in the lexer or parser. The one `expect` in `ast.rs:54` (`Path::last`) holds for every parser-built path.
- Token indexing: `lex` pushes exactly one `Eof`, and `parse_with` appends one when missing. `peek`, `peek_nth`, `skip_newlines` and `recover` stay in bounds. Every declaration consumes its keyword before it can fail, so top-level recovery terminates. The loops in `braced_list`, `block`, `pattern_list` and `fn_type` consume a token or fail.
- Codegen string escaping: `escape_str` (`crates/telos-rustgen/src/lib.rs:421`) and `c_string` (`crates/telos-llvm/src/ir.rs:58`) handle NUL, control bytes, quote, backslash and UTF-8 correctly.
- Comments: `//` only, no block comments, and no doc-comment syntax in the spec, so `///` is an ordinary comment. A string containing `//` is not cut. Newline runs merge across comments and whitespace.
