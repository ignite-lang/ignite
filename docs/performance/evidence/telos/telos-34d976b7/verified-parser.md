# Verified: parser (review-r2-parser.md)

Verifier V2, 2026-11-01. Snapshot `snap-v/`, binary `target-v/debug/telos` (debug build). Inputs in
`snap-v/verify-v2/`. Commands run from the scratch root as `timeout 20 target-v/debug/telos check snap-v/verify-v2/<file>`.

| # | finding | verdict | severity | evidence | fix direction |
|---|---|---|---|---|---|
| 1 | lowercase constant as a pattern binds | PARTIAL (behaviour confirmed; spec is self-contradictory, trigger needs a non-conventional name) | medium (was high) | see 1 below | typer: treat a `Binding` whose name resolves to a constant as a constant pattern, and fix slice.md line 156 to say so |
| 2 | nested `fn` in a block gives a second, misleading error | CONFIRMED | low (was medium; error recovery quality only, the first error is correct) | `f2_nested_fn.telos`: `2:3: error[E001]: expected expression, found \`fn\`` then `3:3: error[E001]: expected a declaration (...), found identifier \`helper\`` | on a declaration keyword inside a block, report "declarations are only allowed at the top level" and skip to the block's closing `}` |
| 3 | postfix `(` `[` across a newline inside `( )`/`[ ]` | CONFIRMED | medium | `f3_postfix_newline.telos` (`[first` NL `(second)]`, and `(if c { 1 }` NL `else { 2 })`): exit 0, no diagnostic. `f3b_postfix_newline_nonfn.telos`: `3:5: error[E040]: type mismatch: \`first\` has type \`Int\` and cannot be called`, which proves the parse is a call; its second fn, `[a` NL `[i]]` with `a: List<Int>`, passes silently as `[a[i]]` | in the postfix loop and at `else`, look at the raw next token (not `peek`) and stop at a `Newline`; or amend slice.md section 3 to exempt brackets. Spec lines 72-74 say both "Newlines are skipped inside ( ) and [ ]" and "A postfix ... must start on the same line as its operand" with no bracket exception; the parser doc at parser.rs:8-10 claims the same-line rule holds and is wrong inside brackets |
| 4 | one-element tuple type `(A,)` accepted as `A` | CONFIRMED | medium | `f4_one_tuple_type.telos`: `fn f(x: (Int,)) -> Int = x + 1` gives exit 0, no diagnostic, and `x + 1` types as `Int`. Code: parser.rs:704-707 `1 => return Ok(args.remove(0))` without checking whether a comma was eaten. Spec section 3 line 143-144: "A one-element tuple, `(a,)` as a value, type or pattern, is E001." | record whether the last element was followed by `,`; `(A,)` is E001; add a fixture |
| 5 | deep nesting overflows the stack | see V1 | - | - | - |
| 6 | `>=` after generic type args (`Option<Int>= None`, `type Box<T>= T`) | not a defect by the letter; spec-ambiguous (UNCLEAR, decision for spec owner) | low | `f6_gteq.telos`: `1:11: error[E001]: expected \`>\`, found \`>=\`` and `4:20: ... found \`>=\``. Spec: section 2 line 65 lists `>=` as one punctuation token; grammar `type := ident ('<' type (',' type)* '>')?`, `alias := 'type' ident ('<' ident (',' ident)* '>')? '=' type`, `stmt := ('let' \| 'var') ident (':' type)? '=' expr`. The spec states no longest-match rule and no token splitting. What it requires: with `>=` a single token, `>` `=` is not present, so rejection follows; accepting it needs a spec sentence (Rust-style split of `>=` where `>` is expected) | if wanted: in `expect(Gt)` split a `GtEq` into `>` and a pending `=`; otherwise document "write a space" in section 2 |
| 7 | `let _: T = e` rejected | REFUTED as a defect: the spec routes `let _` to the pattern form, which has no annotation | low (informational) | `f7_let_underscore_typed.telos`: `4:8: error[E001]: expected \`=\`, found \`:\``. Spec: grammar line 100 `('let' \| 'var') ident (':' type)? '=' expr` and line 101 `('let' \| 'var') pattern '=' expr  a binding, \`_\` or a tuple of those`; section 2 line 47 identifiers `[A-Za-z_][A-Za-z0-9_]*` (admits `_`). But line 157 ("`let _ = e` ... are `StmtKind::LetPattern`"), section 10 row 13 (line 833, "`let _ = e` = `StmtKind::LetPattern` with `PatternKind::Wildcard`") and 10.1 line 863 fix `let _` as a `LetPattern`, whose AST (ast.rs:293-297) has no type field. What it requires: `let _ = e` only; `let _: T = e` has no AST form, so E001 is consistent | if typed discard is wanted: add `ty: Option<Type>` to `LetPattern` and the grammar line; otherwise exclude `_` from `ident` in section 2 to remove the ambiguity |
| 8 | long binary chains overflow later recursive passes | CONFIRMED | medium (was low; threshold far lower than "tens of thousands") | see 8 below | iterative traversal for left-nested `Binary` in the typer (and checker), or a depth limit with a diagnostic, or run the front end on a thread with a large stack |

## 1. Lowercase constant pattern

Input `f1_lowconst.telos`: `const limit: Int = 3`, `const LIMIT: Int = 3`, `label` matches `limit => "at limit"`, `_ => "other"`; `label_upper` the same with `LIMIT`; a `Client` protocol prints both for 5 and 3.

- `telos check`: exit 0, no diagnostic (no unreachable-arm or shadowing warning).
- `telos build ... --target llvm --out snap-v/verify-v2/out-f1-llvm`, run against `socat TCP-LISTEN:47193`:
  ```
  lower label(5): at limit
  lower label(3): at limit
  upper label(5): other
  upper label(3): at limit
  ```
  The lowercase constant binds and matches every value; the uppercase one compares.
- Code: parser.rs:1657-1658 (`!path[0].is_uppercase()` makes a `Binding`; `is_uppercase` is ast.rs:35-40, first byte ASCII uppercase); typer.rs:2639 calls `const_pattern` only from the `Variant` branch; typer.rs:2661 binds every `Binding`.
- Spec text, which conflicts with itself:
  - section 4.3 line 239: "A payload-less name in a pattern that names a constant is a constant pattern: it matches a value equal to the constant." Line 237: "Names are UPPER_SNAKE by convention."
  - section 2 line 47-48: "...start with an uppercase letter by convention; the parser does not enforce case."
  - section 3 AST, line 155-156: "a single lowercase name without arguments is a `Binding`" (supports the parser).
  - section 10.1 line 908: "Constant pattern: a `Variant { path: [NAME], args: [] }` for which `TypedProgram::pattern_const(pattern)` is `Some(q)`".
  - So 4.3 says a lowercase constant name is a constant pattern; the AST rule says the parser must make it a `Binding`, and 10.1 only recognises a `Variant`. No rule says what a lowercase constant name in a pattern does.
- Severity medium rather than high: wrong runtime behaviour with no diagnostic, but only for a constant named against the stated convention. Either fix (typer consults `consts` for a `Binding`, or E-code a lowercase constant name) needs the spec to pick one.

## 8. Long chains

`fn f() -> Int = 1 + 1 + ... + 1` generated with N terms (`f8_chain_N.telos`):

| N | exit |
|---|---|
| 1000, 1200 | 0 |
| 1500, 1700, 2000, 3000, 5000, 7000, 10000, 50000, 200000 | 134, `thread 'main' has overflowed its stack` |

`gdb` backtrace for N=2000: about 4400 frames cycling `Typer::expr` typer.rs:1302 -> typer.rs:1349 -> typer.rs:2449 / 2419, entered from `Typer::run` typer.rs:292, `telos_types::type_module` lib.rs:700, `crates/telos/src/lib.rs:471-472`, `main.rs:89`. The parser and the checker passed; the overflow is in the typer. Debug build; the release threshold was not measured.
