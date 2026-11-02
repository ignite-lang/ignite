# R2 review: parser (crates/telos-syntax/src/parser.rs)

Scope: parser.rs against docs/slice.md sections 2 to 5 and 10, with the lexer, and with telos-check and telos-types where a finding depends on them. Read-only; no build or test was run.

Line numbers are as of 11:56 on 2026-11-01. crates/telos-types/src/typer.rs and crates/telos-check/src/lib.rs were modified at 11:53:42 while I was reading; I re-read the cited lines after the change. parser.rs (mtime 11:33) and lexer.rs (mtime 11:15) were not changed during the review.

## Findings, most severe first

1. Lowercase constant in a pattern silently binds. Severity: high. Confidence: confirmed (traced in parser and typer; not run).
   - `crates/telos-syntax/src/parser.rs:1657-1658`:
     `if path.len() == 1 && !has_args && !path[0].is_uppercase() {` / `PatternKind::Binding(path.remove(0))`
   - `crates/telos-types/src/typer.rs:2634-2640` sends a pattern to `const_pattern` only for `PatternKind::Variant`; `typer.rs:2661` binds every `PatternKind::Binding`.
   - Failure scenario:
     ```
     const limit: Int = 3
     fn label(x: Int) -> Text = match x {
       limit => "at limit"
       _ => "other"
     }
     ```
     `label(5)` returns "at limit". The parser makes `limit` a Binding, which matches every value, so `_` is unreachable. No diagnostic is raised: neither telos-check nor telos-types gives constants a case rule, and no fixture uses a lowercase constant.
   - The spec requires the opposite. docs/slice.md section 4.3: "A payload-less name in a pattern that names a constant is a constant pattern". Section 10, row 5 and section 10.1 ("Constant pattern: a `Variant { path: [NAME], args: [] }`") say the same.
   - Direction: the case split in the parser is the root. Either the typer's const branch also takes a Binding whose name is in `consts`, or the parser emits Variant for every payload-less single name and the typer binds when no constant matches.

2. A nested `fn`, `const` or `type` inside a block produces a second, misleading error. Severity: medium. Confidence: confirmed (traced).
   - `crates/telos-syntax/src/parser.rs:1454`: `_ => return Err(self.expected("expression")),` fails at the nested keyword. `recover()` at `parser.rs:352-365` stops at that line-start keyword, and `program()` at `parser.rs:398-410` then parses it as a top-level declaration.
   - Input:
     ```
     fn main() -> Unit {
       fn helper() -> Int = 1
       helper()
     }
     ```
   - Output: `2:3` E001 "expected expression, found `fn`", then `3:3` E001 "expected a declaration (...), found identifier `helper`". The second error names a declaration, not the real problem. The rest of the block is skipped to the next top-level keyword.

3. Postfix `(`, `[`, `.`, `?` and `else` attach across a newline inside `( )` and `[ ]`. Severity: medium. Confidence: confirmed for the parse result; the checker outcome was not traced. Intent unclear, please decide.
   - `crates/telos-syntax/src/parser.rs:140-156` (`peek_index` skips `Newline` while `newlines_insignificant()`), `parser.rs:1284` (the postfix loop reads `peek()`), `parser.rs:1521` (`else` is taken with `eat`).
   - The module doc at `parser.rs:8-10` says a postfix "applies only when no `Newline` token lies between it and its operand". docs/slice.md section 3 says: "A postfix `(`, `[`, `.`, `?` must start on the same line as its operand. `else` must follow the closing `}` of its `if` on the same line." The code does not keep either rule inside brackets.
   - Failure scenario:
     ```
     let xs = [
       first
       (second)
     ]
     ```
     parses as `[first(second)]` with no error. A missing comma between two list items becomes a call, an index or a field access instead of E001. `(if c { 1 }` followed by a newline and `else { 2 })` is also accepted.

4. One-element tuple type `(A,)` is accepted as `A`. Severity: medium. Confidence: confirmed.
   - `crates/telos-syntax/src/parser.rs:705-707`: `1 => return Ok(args.remove(0)),`
   - Input: `fn f(x: (Int,)) -> Int = 0`.
   - docs/slice.md section 3: "A one-element tuple, `(a,)` as a value, type or pattern, is E001." Section 8 lists "a one-element tuple" under E001. The value form is checked at `parser.rs:1440-1449` and the pattern form at `parser.rs:1613-1621`. The type form is not checked, and no fixture covers it.

5. Deeply nested source can overflow the stack. Severity: medium. Confidence: confirmed that no depth limit exists; the overflow threshold was not measured (building was not allowed).
   - Rated medium rather than high: triggering it takes thousands or tens of thousands of nested constructs in one local file. Nothing in the CLI path reads untrusted network input.
   - No depth guard exists in the front end. A search of all crates for depth, nesting, limit and "too deep" finds only lexer string braces and parser nesting-reset helpers. The CLI parses on the main thread with the default stack: `crates/telos/src/main.rs:86` (`fn main`), `crates/telos/src/lib.rs:255` (`telos_syntax::parse`), and no `stack_size` anywhere in crates/telos.
   - Recursion paths: unary chains `parser.rs:1265` and `parser.rs:1271` (`- - - x`, `!!!x`); expression nesting through `primary` → `expr` for parentheses, list literals, call args, index, and match scrutinee (`parser.rs:1311`, `1336`, `1413`, `1423`, `1433`); else-if chains `parser.rs:1523`; nested blocks `parser.rs:1002` (stmt → block → stmt for `while`, `loop` and `for`, which do not pass through `expr`); types `parser.rs:681`, `698`, `739`; patterns `parser.rs:1593`, `1651`.

6. `>=` is one token, so type arguments written without a space before `=` are rejected. Severity: low. Confidence: confirmed for the rejection. The spec is ambiguous, so please decide.
   - `crates/telos-syntax/src/lexer.rs:300`: `(b'>', Some(b'=')) => (TokenKind::GtEq, 2),`. The closing `expect(&TokenKind::Gt)` is at `parser.rs:593`, `686` and `961`.
   - Input: `let x: Option<Int>= None` gives "expected `>`, found `>=`". The same happens for `type Box<T>= T`.
   - docs/slice.md section 2 lists `>=` as punctuation, so maximal munch yields this by the letter. Rust splits the token in this position. Whether to accept it is a decision for the spec owner.

7. `let _: T = e` is rejected. Severity: low. Confidence: confirmed for the rejection. The spec is ambiguous, so please decide.
   - `crates/telos-syntax/src/parser.rs:174` (`Ident("_")` makes `let_pattern_follows` return true), then `parser.rs:1038-1050` (the pattern branch reaches `expect(&TokenKind::Assign)` at line 1050 with `:` as the next token).
   - Input: `let _: Int = read()` gives "expected `=`, found `:`".
   - docs/slice.md section 3 writes `('let' | 'var') ident (':' type)? '=' expr`, and section 2 defines identifiers as `[A-Za-z_][A-Za-z0-9_]*`, which admits `_`. The pattern rule writes `'_'` quoted, and the pattern form has no type annotation. The spec does not say which form applies to `_`.

8. Long chains build deep left-leaning ASTs, and later recursive passes are the ones at risk. Severity: low. Confidence: suspected for the threshold; the call sites are confirmed.
   - The parser's own loops are iterative (`binary_left` at `parser.rs:1148-1162`, postfix at `parser.rs:1280-1329`, template joining at `parser.rs:1504-1507`), but each builds a left-nested tree. The checker recurses into both children of a Binary (`crates/telos-check/src/lib.rs:1537-1539`), and the typer does too (`crates/telos-types/src/typer.rs:1349`). A Box chain of that depth is also dropped recursively.
   - Input: an expression with tens of thousands of `+` terms, or `f()()()...`. Not traced beyond the call sites.

## Checked, no finding

- Infinite loops: each declaration consumes its keyword before it can fail (`parser.rs:414-464`), so the `program()` loop always advances. The `braced_list`, `block`, `system`, `protocol`, `pattern_list` and `tuple_type` loops each consume a token or return an error.
- Panics and unwraps: none reachable from source text that I could find. `items.remove(0)` and `path.remove(0)` are guarded by length checks. The `expect` in `Path::last` (`crates/telos-syntax/src/ast.rs:54`) has no producer of an empty path. `line_col` in `diagnostic.rs` receives token-start offsets, which are char boundaries.
- A failed parse always has a recorded diagnostic (`Fail` is only returned after a `report` or `expected`), so `parse_with` cannot return `Ok` for a failed parse. `report` dedups by start position only, which is correct for the cases above.
- Precedence and associativity match section 3: `||` < `&&` < `==`/`!=` < `<` `>` `<=` `>=`, each comparison level non-associative; `*` binds tighter than `+`; unary binds tighter than binary; `?` is postfix.
- Record-literal restriction: `no_record` is restored correctly across brackets, blocks and nested `if`/`match`, and `else if` handles the condition the same way.
- Protocol recovery skips the whole protocol body (`skip_past_matching_brace`), so one error does not cascade inside a protocol.
- Lexer: linear; string interpolation cannot contain strings; `b'...'` and `b"..."` spans and slicing are safe.
- Trailing commas are accepted in `(a, b,)`, `(p, q,)` and `[p,]`, which section 3 does not allow, but rejected in call args and list literals. Harmless, not reported.
- A lowercase record name (`type point { ... }`) cannot be constructed, because `point { x = 1 }` parses as a name followed by a block. Section 3 requires an uppercase initial for record literals, so this is by design.
