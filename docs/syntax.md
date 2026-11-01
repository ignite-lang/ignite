# Telos syntax: less ceremony, same explicitness

Status: design proposal, 2026-11-01. Nothing here is implemented. It builds on slice 9 as
decided (closures, raw strings, globs, range patterns, tuple `.0`, structural `==`, associated
fns, `std/option.telos`, `std/result.telos`) and treats those decisions as landed. It proposes
the items of a syntax slice, called slice 10 here; section 7.4 settles the clash with
`wasm.md`'s use of that number.

Timo's request: "make a design to improve syntax in general and make it more sugary and less
verbose".

## 1 Scope and rules

### 1.1 Constraints this design must keep

| constraint | source |
|---|---|
| 30 to 50 reserved words; about 120 standard definitions around version 1 | `Telos_Design.md` §1.5 (line 81), §3.6 (line 206) |
| defer macros, implicit conversions, unrestricted reflection and user-defined syntax | `Telos_Design.md` §3.5 (line 159) |
| runtime effects require declared capabilities; an effect is never hidden | `Telos_Design.md` §3.1; `slice.md` §5 |
| 29 reserved words today, `true` and `false` included | `crates/telos-syntax/src/lexer.rs` keyword table; `vision.md` §2 |
| a fn value that crosses a component cut is a named module-level fn, never a closure | `wasm.md` §3.1, E082 |
| roles become generic over a stream bound with a `Limits` record | `protocols.md` §2.2, §4; `vision.md` §4 wave B |
| actors are pure `handle(state, msg) -> Step<S>` reducers that rebuild their state record | `actors.md` §3, §9.2 to §9.3 |
| postfix `.`, `(`, `[`, `?` start on the operand's line; `else` follows `}` on its line; arms and statements are separated by `,` or a newline | `slice.md` §3 (line 82), grammar at lines 88 to 158 |

### 1.2 Acceptance rules for a proposal

A proposal is taken when all five hold:

1. It is a pure desugar, or a small typed node whose lowering exists in both backends already.
2. It removes lines or tokens that the measurement in section 2 counts, or a future design
   (protocols, actors, wasm) asks for it.
3. It hides no effect, no authority root and no conversion. Every call that performs IO stays a
   visible call with its receiver.
4. It costs at most one reserved word, and the language stays within 30 to 50 reserved words.
5. A reader who knows Rust, Kotlin, Swift or Gleam reads it without the manual.

## 2 Measurement

### 2.1 Corpus

The script `syntax-scratch/measure.py` (under this session's scratch directory, run as
`python3 -I measure.py <repo>`) strips comments, tokenizes with one regular expression
(string, byte and number literals count as one token) and runs 29 pattern detectors. Corpus:
the seven files Telos has plus the design example.

| file | lines | blank | comment | code | tokens | test lines |
|---|---|---|---|---|---|---|
| `std/text.telos` | 334 | 36 | 28 | 270 | 2258 | 157 |
| `std/json.telos` | 608 | 51 | 37 | 520 | 4469 | 179 |
| `std/console.telos` | 109 | 14 | 12 | 83 | 557 | 40 |
| `std/protocol/telnet.telos` | 602 | 47 | 60 | 495 | 3992 | 189 |
| `std/protocol/http.telos` | 678 | 65 | 61 | 552 | 4126 | 251 |
| `examples/terminal/terminal.telos` | 96 | 13 | 2 | 81 | 608 | 29 |
| `examples/todo/todo.telos` | 151 | 20 | 9 | 122 | 1332 | 48 |
| `examples/terminal/terminal.design.telos` | 118 | 14 | 0 | 104 | 434 | 0 |
| total | 2696 | 260 | 209 | 2227 | 17776 | 893 |

### 2.2 The ten most frequent verbose patterns

"Saved" is what the pattern costs over the shortest form a proposal in section 4 allows. Line
counts are measured per site; token counts are per site times the tokens dropped.

| # | pattern | sites | saved | example | proposal |
|---|---|---|---|---|---|
| 1 | three-line early exit `if c {` / `return x` / `}` | 39 | 78 lines | `std/text.telos:68` | S0 |
| 2 | two-arm `match` with a default arm that yields `None`, `false` or `{}` | 21 | 63 lines | `std/json.telos:534` (`as_text`) | P1, P3 |
| 3 | `None => None` or `_ => None` arm that only forwards absence | 16 | 16 lines, 64 tokens | `std/text.telos:177` | P3 |
| 4 | accumulator loop `var out = …` / `for` / `out.append` / `out` | 5 | 29 lines | `std/text.telos:255`, `telnet.telos:270` | P6 |
| 5 | counter `while`: `var i = 0` / `while i < n` / `i = i + 1` | 10 | 20 lines, 80 tokens | `std/text.telos:76` | P5 |
| 6 | `x = x + e` | 25 | 50 tokens | `std/text.telos:76` | P6 |
| 7 | run of `push`/`append` after a `var` | 9 | 15 lines | `telnet.telos:251` | P6 |
| 8 | pair literal `("key", value)` in a list | 44 | 88 tokens | `std/json.telos:97` | P11 |
| 9 | `for pair in xs` / `let (a, b) = pair` | 7 | 7 lines, 28 tokens | `std/json.telos:79`, `http.telos:110` | P5 |
| 10 | empty arm `_ => {}` or `Nothing => {}` | 9 | 9 lines | `std/json.telos:202`, `telnet.telos:155` | P1 |

Counted, but not a target of this design:

| pattern | sites | why not |
|---|---|---|
| `\"` and `{{` escapes in test strings | 75 lines, 336 escapes | raw strings, slice 9 row 2 |
| `.to_bytes()` / `.to_text()` | 64 | conversions stay explicit (`Telos_Design.md` §3.5) |
| runs of 3 or more `assert`s on one callee | 52 runs, 345 asserts | one line per case already; slice §12.5 prints the failing source text |
| `match stream.read(4096)? {` read loops | 9 | belongs to `protocols.md` P1 (generic roles over `ByteStream`) |
| `requires [stream.read, stream.write]` | 4 | section 5, row "grouped `requires`" |

## 3 Comparable sugar

Every row was checked against the page on 2026-11-01 unless the last column says otherwise.

| language | feature | what it does | URL | for Telos |
|---|---|---|---|---|
| Rust | let-else | `let P = e else { diverge }`; the else block has the never type; the initializer may not end in `}` or be `&&`/`\|\|` | https://doc.rust-lang.org/reference/statements.html#let-statements | P2, taken as is |
| Rust | let chains | `if let P = e && c` in `if` and `while`, stable in 1.88, edition 2024 only | https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/ | the Rust lowering of P1 |
| Rust | `?` | early return of `Err` or `None` | https://doc.rust-lang.org/reference/expressions/operator-expr.html#the-question-mark-operator (not fetched) | P3 |
| Rust | functional record update | `S { f: e, ..base }` | https://doc.rust-lang.org/reference/expressions/struct-expr.html#functional-update-syntax (not fetched) | P7 |
| Rust | if-let guards | unstable, tracking issue | https://github.com/rust-lang/rust/issues/51114 (not fetched) | why P4's guards bind nothing |
| C# | `is` pattern | `e is P` is a Bool; `if (x is int a && y is int b)` binds `a`, `b` in the true branch; list patterns with `..` | https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/operators/is | P1, P9 |
| Kotlin | scope functions | "execute a block of code within the context of an object": `let`, `run`, `with`, `apply`, `also` | https://kotlinlang.org/docs/scope-functions.html | rejected (section 5) |
| Kotlin | trailing lambdas, `it` | the last fn argument outside the parentheses; `it` names a single parameter | https://kotlinlang.org/docs/lambdas.html (not fetched) | rejected |
| Kotlin | data-class `copy` | `p.copy(age = 3)` | https://kotlinlang.org/docs/data-classes.html (not fetched) | P7 in record form |
| Swift | `guard let … else` | the else clause must leave the scope | https://docs.swift.org/swift-book/documentation/the-swift-programming-language/controlflow/ (renders client-side; not fetched) | P2 |
| Swift | dictionary literal | `["a": 1]` | https://docs.swift.org/swift-book/documentation/the-swift-programming-language/collectiontypes/ (not fetched) | P11 |
| Gleam | `use` | "All the code below the `use` becomes an anonymous function" | https://tour.gleam.run/advanced-features/use/ | rejected |
| Gleam | record update | "create a new record from an existing one of the same type" | https://tour.gleam.run/data-types/record-updates/ | P7 |
| Gleam | labelled arguments | call by label, any order | https://tour.gleam.run/functions/labelled-arguments/ (not fetched) | P10 |
| Elixir | `\|>`, `with` | pipe into the first argument; chain of pattern matches with one `else` | https://hexdocs.pm/elixir/Kernel.html, https://hexdocs.pm/elixir/Kernel.SpecialForms.html (not fetched) | rejected; P2 covers `with` |
| F# | computation expressions | builder-defined `let!`, `return` | https://learn.microsoft.com/en-us/dotnet/fsharp/language-reference/computation-expressions (not fetched) | rejected |
| OCaml | binding operators | `let*` bound to a user monad | https://ocaml.org/manual/5.2/bindingops.html (not fetched) | rejected |
| Roc | `?`, `!` suffix | `?` returns the `Err`; effectful fns end in `!` by convention; backpassing is gone from the tutorial | https://www.roc-lang.org/tutorial | P3; backpassing rejected |
| Roc | record builders | `<-` kept after backpassing was removed | https://roc-lang.github.io/zulip-export/stream/383402-API-design/topic/record.20builder.20syntax.html | rejected |
| Koka | effect handlers | `with handler { … }` | https://koka-lang.github.io/koka/doc/book.html#sec-handlers | rejected |
| Pkl | amending | `(base) { field = … }` | https://pkl-lang.org/main/current/language-reference/index.html#amending-objects | covered by field defaults and P7 |
| CUE | defaults | `*` marks a preferred value | https://cuelang.org/docs/tour/types/defaults/ | covered by field defaults (slice §4.7) |

Lesson across the table: the languages that removed nesting without hiding control flow did it
with three devices, and Telos lacks all three: a Bool pattern test (C# `is`, Rust `matches!`),
a binding that exits on mismatch (Rust let-else, Swift `guard`), and guards on match arms. The
devices that hide control flow inside a closure (Gleam `use`, Roc backpassing, F# builders,
Kotlin trailing lambdas) are the ones Roc removed and the ones `wasm.md` E082 cannot lower at a
component edge.

## 4 Proposals

Each proposal gives the syntax, one before/after from the corpus, the typing and desugaring
rule, the cost per compiler layer and the interaction with effects. Costs are estimates of
changed Rust lines: S under 100, M 100 to 300, per crate.

### S0 One-line early exits (style, no compiler change)

`if c { return x }` is already legal: a block holds one statement without a separator (slice
§3 grammar, `block`). The corpus writes 39 of them in three lines.

Before (`std/text.telos:68`), 3 lines:

```telos
  if at < 0 || at + needle.len() > bytes.len() {
    return false
  }
```

After, 1 line: `  if at < 0 || at + needle.len() > bytes.len() { return false }`

Rule for std and examples: a block of one `return`, `break`, `continue` or one call statement
under 100 columns stays on its line. 78 lines, no cost, no effect change. A later `telos fmt`
enforces it.

### P1 `e is P`: a Bool pattern test

Syntax: `compare := additive (cmp additive | 'is' pattern)?`. `is` binds tighter than `==`,
`&&` and `||` and looser than `+`. New reserved word `is` (29 to 30); the corpus uses `is` only
inside test names and comments (`grep -nw is`).

Before (`std/json.telos:534`) and after:

```telos
fn as_text(value: Json) -> Option<Text> = match value {
  Str(text) => Some(text)
  _ => None
}

fn as_text(value: Json) -> Option<Text> = if value is Str(text) { Some(text) } else { None }
```

Other sites: `is_whitespace` (`std/text.telos:11`) becomes
`b is b' ' | b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r'`; the `_ => {}` arm at
`std/json.telos:200` to 203 becomes `if bytes.at(i) is b'.' | b'e' | b'E' { return Err(…) }`;
the test at `http.telos:605` to 612 becomes
`assert next_of(padded(…, 8192))? is Got(_, rest) && rest.is_empty()`.

Typing:

- `e is P` has type `Bool`. `P` is checked against `e`'s type exactly as a match arm pattern
  (E040, E003, constant patterns per slice §4.3, range patterns per slice 9 row 8).
- Bindings of `P` are in scope where the test is known true: the right operand of an `&&`
  whose left operand contains the test as a conjunct, the then-block of `if`, and the body of
  `while`. They are not in scope under `!`, after `||`, in `else`, or after the statement.
  A use outside is E003 (unknown name).
- `assert e is P` works without a rule of its own, and a failure prints `e`'s debug text
  (slice 9 row 11).

Desugar: `if e is P && c { A } else { B }` is `match e { P => if c { A } else { B }, _ => B }`
with `B` emitted once behind a label. As a value, `e is P` is `match e { P => true, _ => false }`.

| layer | change | size |
|---|---|---|
| `telos-syntax` | `Is(expr, pattern)` node; `is` keyword | S |
| `telos-check` | condition scopes for the bindings | S |
| `telos-types` | Bool type, pattern check reused | S |
| `telos-rustgen` | `if let P = e && c` (let chains), `while let`, `matches!(e, P)` for a bare value | S |
| `telos-llvm` | the arm pattern test, branching to then or else | S to M |

Effects: `e` is evaluated once, with its own effects. Nothing else runs. No authority change.

### P2 `let … else`

Syntax: `stmt := 'let' pattern (':' type)? '=' expr 'else' block`. Only here may a `let`
pattern be refutable; elsewhere a refutable `let` pattern stays E001 (slice §8).

Before (`examples/todo/todo.telos:31` to 41), 11 lines:

```telos
fn create(todos: Store<Todo>, body: Text) -> Response = match parse(body) {
  Ok(value) => match value.text("title") {
    Some(title) => {
      let todo = Todo { id = todos.next_id(), title, done = or(value.bool("done"), false) }
      todos.put(todo.id, todo)
      json(201, to_json(todo))
    }
    None => status_response(400)
  }
  Err(_) => status_response(400)
}
```

After, 7 lines:

```telos
fn create(todos: Store<Todo>, body: Text) -> Response {
  let Ok(value) = parse(body) else { return status_response(400) }
  let Some(title) = value.text("title") else { return status_response(400) }
  let todo = Todo { id = todos.next_id(), title, done = value.bool("done").unwrap_or(false) }
  todos.put(todo.id, todo)
  Response.json(201, to_json(todo))
}
```

Also `http.telos:444` to 447 (`let request = match parse_head(…) { Some(parsed) => parsed
None => return … }`, 4 lines to 1) and `json.telos:520` (`member`, together with P5, 12 to 6).

Typing: the else block must have type Never (slice 9 row 4: `return`, `break`, `continue`
and blocks ending in them); otherwise E070. An initializer that ends in `}` (an `if`, `match`,
block or record literal) or whose top operator is `&&` or `||` is E001 with the hint
"parenthesize the initializer", as in Rust, so `else` never attaches to the wrong `if`. The
bindings are in scope for the rest of the block, immutable (E006 on assignment).

Desugar: `let P = e else { D }` followed by `rest` is `match e { P => { rest }, _ => D }`.

| layer | change | size |
|---|---|---|
| `telos-syntax` | `else` after a `let` initializer; the `}` and `&&` restriction | S |
| `telos-check` | refutable pattern allowed here; scope of the rest of the block | S |
| `telos-types` | Never check on the block, E070 | S |
| `telos-rustgen` | Rust `let P = e else { D };`, one to one | S |
| `telos-llvm` | pattern test, diverging block on failure | S |

Effects: the else block's effects are the function's, as for any statement. The construct
replaces nested matches with the same control flow, written top to bottom.

### P3 `?` on `Option`

Today `?` applies to `Result` only and reports E040 "`?` needs a function returning `Result`"
outside one (`crates/telos-types/src/typer.rs`, `try_`). Tests already accept it on `Option`
(slice §12.3).

Rule: in a fn or closure whose return type is `Option<U>`, `e?` with `e: Option<T>` has type
`T` and returns `None` on `None`. `?` on `Option` in a `Result` fn, or the reverse, stays E040
with the message naming both types; there is no conversion. In a closure, `?` returns from the
closure, as in Rust.

Before (`std/json.telos:555`), 4 lines, and the same for `int` and `bool`:

```telos
fn text(value: Json, name: Text) -> Option<Text> = match member(value, name) {
  Some(Str(t)) => Some(t)
  _ => None
}
```

After, 1 line: `fn text(value: Json, name: Text) -> Option<Text> = member(value, name)?.as_text()`

Other sites: `split_once` (`std/text.telos:175`), `hex4` (`std/json.telos:268`, 10 lines to
4: `Some((hex_value(w.at(at))?, hex_value(w.at(at + 1))?, …))`), `unicode_escape`
(`json.telos:281`), `parse_head` (`http.telos:305`). About 21 lines.

Cost: `telos-types` S (accept `Option` in `try_`); `telos-rustgen` none (Rust's `?` already
does it); `telos-llvm` S (the `None` early-return path beside the `Err` one). Effects: none;
`?` is control flow on a value.

### P4 Match guards

Syntax: `arm := pattern ('if' expr)? '=>' (block | expr)`.

Before (`telnet.telos:136` to 142), 7 lines:

```telos
  (Subnegotiation(count), _) => if count + 1 > MAXIMUM_SUBNEGOTIATION {
    (Ground, SubnegotiationTooLong)
  } else if b == IAC {
    (SubnegotiationInterpret(count + 1), Nothing)
  } else {
    (Subnegotiation(count + 1), Nothing)
  }
```

After, 3 lines:

```telos
  (Subnegotiation(count), _) if count + 1 > MAXIMUM_SUBNEGOTIATION => (Ground, SubnegotiationTooLong)
  (Subnegotiation(count), IAC) => (SubnegotiationInterpret(count + 1), Nothing)
  (Subnegotiation(count), _) => (Subnegotiation(count + 1), Nothing)
```

Typing: the guard is `Bool` (E040) and sees the arm's bindings. A guard is pure: an
effectful call or `?` in it is E072. A guarded arm does not count toward exhaustiveness
(E046). An `is` inside a guard binds nothing that flows into the arm, because Rust, the first
target, has no stable if-let guards (rust-lang/rust#51114); a binding so used is E003. An
or-pattern with a guard tests the guard once for the whole arm, as in Rust.

Desugar: no pure desugar without duplicating the remaining arms; it is a node.
`telos-syntax` S, `telos-types` S (exhaustiveness skips guarded arms), `telos-rustgen` S
(native `P if g =>`), `telos-llvm` S to M (a failed guard jumps to the next arm's test). About
12 lines across the corpus (`decode`, `apply_key`, `encode_input`).

Effects: purity of guards keeps a property the language has today: testing patterns never
performs IO, so the order in which a backend tests arms is not observable.

### P5 `for` patterns and integer ranges

Syntax: `'for' pattern 'in' (expr | expr ('..' | '..=') expr) block`. The pattern is a `let`
pattern (a name always binds, slice §4.3) and must be irrefutable (E071).

Before (`http.telos:110`) and after:

```telos
  for pair in response.headers {
    let (name, value) = pair
  for (name, value) in response.headers {
```

Before (`telnet.telos:283` to 286) and after:

```telos
  var i = 0
  while i < bytes.len() {
    let b = bytes.at(i)
    i = i + 1
  for i in 0..bytes.len() {
```

Typing: both bounds `Int` (E040), evaluated once, before the loop. `a..b` is half-open,
`a..=b` closed. A range is not a value: `..` outside a `for` head and outside a pattern is
E073, so `std` gains no `Range` type and the definition budget does not move.

Desugar of `for i in a..b { body }`:

```telos
{
  var next = a
  let end = b
  while next < end {
    let i = next
    next = next + 1
    body
  }
}
```

The increment comes before `body`, so `continue` in `body` cannot skip it. `i` is a fresh
immutable binding per iteration (E006 on assignment), which also keeps the loop clear of the
`for`-binding aliasing defects of `vision.md` §10.1 M2.

Cost: `telos-syntax` S; `telos-check` S (pattern bindings); `telos-types` S; both backends
none if the desugar runs before them, or S for a native Rust `for i in a..b`. About 27 lines
(10 counter loops, 7 destructuring loops). Effects: none.

### P6 Compound assignment and concatenation

Syntax: `stmt := place ('+=' | '-=') expr`. `+` gains `List<T> + List<T>` and
`Bytes + Bytes` (concatenation; today E045).

Before (`telnet.telos:270` to 276), 8 lines with the comment, and after, 5:

```telos
fn render(reply: Reply) -> Bytes {
  var out = Bytes.empty()
  for line in reply.lines {
    out.append(line.to_bytes())
    out.append(b"\r\n")
  }
  out
}

fn render(reply: Reply) -> Bytes {
  var out = Bytes.empty()
  for line in reply.lines { out += line.to_bytes() + b"\r\n" }
  out
}
```

And the default arm of `apply_key` (`telnet.telos:381` to 386, 6 lines) becomes
`_ => (line + [b], Show([b]))`.

Typing: `p += e` is `p = p + e` with `p` evaluated once; the operand types are those `+`
allows (`Int`, `Byte`, `Text`, `List<T>`, `Bytes`); `-=` is `Int` and `Byte`. Assignment to a
`let` binding or constant is E006, as for `=`. Overflow rules for `Int` and `Byte` are those of
`+`.

Lowering: on a `var` place of `List`, `Bytes` or `Text`, both backends append in place
(`extend_from_slice`, `push_str`; the LLVM arena's regrow path), which is the same value as
the copy because the place is owned by the function. `xs + ys` elsewhere builds a new value.
Cost: `telos-syntax` S, `telos-types` S, `telos-rustgen` S, `telos-llvm` S to M (concat on
arena lists). About 15 lines and 50 tokens. Effects: none; `+=` is not a capability operation.

### P7 Record update `R { f = e, ..base }`

Slice 9 row 9 already requires `Store.update(id, |t| Todo { done = flag, ..t })`, but no row
states the rule. This fixes it, whether it lands in slice 9 or here.

Syntax: `primary := ident '{' (init sep)* ('..' expr sep?)? '}'`; the `..base` entry comes
last.

Before (`actors.md` §9.2, lines 408 to 422), and after:

```telos
    let next = Todos { table = state.table.put(id, changed), changes = state.changes }
    let next = Todos { table = state.table.put(id, changed), ..state }
```

Typing: `base` has type `R` (E040), with the same type arguments for a generic record. Listed
fields follow E047 (unknown, duplicate). Every field not listed is taken from `base`; field
defaults (slice §4.7) do not apply when a base is given. Punning works: `Todo { title, ..t }`.
The record type cannot change: a `TodoV1` to `Todo` migration (`wasm.md` §5.3) stays a fn.

Desugar: `R { f = e, ..b }` is `{ let f1 = e; let b1 = b; R { f = f1, g = b1.g, … } }`,
evaluating in source order. Cost: `telos-syntax` S, `telos-types` S, `telos-rustgen` S (Rust
`..b1`, cloning `b1` when it is used later), `telos-llvm` S (field copy glue exists). Effects:
those of `e` and `base`; the copy itself is pure.

### P8 Visibility (proposal for an open question)

Slice §4.1: every module item is exported. The 5 std modules export 136 items; about 46 are
used outside their module. Six names collide between std modules (`IoResult`, `render`,
`session`, `IDLE_TIMEOUT_MS` in telnet and http; `is_space`, `skip_space` in console and
json), and slice 9 row 6 makes two globs that both provide a used name E041. Private by default
with a contextual `pub` (Rust, Gleam) costs 46 `pub` tokens and puts std at about 46
definitions against the budget of 120. It is not sugar and it adds tokens, so it is question
Q2 rather than a contract item.

### P9 Rest patterns in list patterns

Syntax: in a list pattern, one `..` anywhere, or one `..name` as the last element:
`[_, _, ..]`, `[first, ..rest]`, `[.., last]`.

Wanted by `protocols.md` line 584 (`([_, _, ..], _, _) => Reject(400)`; line 595: "a rest
pattern that slice §3 lacks") and `vision.md` §4 wave A.

Typing: `..name` binds a `List<T>` (or `Bytes` where list patterns match `Bytes`) holding the
remaining elements. A second `..`, or `..name` not last, is E076. Exhaustiveness: a list
pattern with `k` fixed elements and a rest covers every length `>= k`; exact-length patterns
cover their length, so `[]` with `[_, ..]` is exhaustive.

Lowering: length test `len >= k`, element tests by index, `rest` as `slice(k, len)`. Cost:
`telos-syntax` S, `telos-types` M (exhaustiveness over length sets), backends S each. Effects:
none.

### P10 Parameter defaults and named arguments

Syntax: `param := ident ':' type ('=' expr)?`; `args := (expr ',')* (ident '=' expr ',')*`.
The design already writes named arguments: `stream.read(maximum = 4096 bytes)`
(`Telos_Design.md:448`), `nativeProcess(includes = […])` (`:665`).

Before (`examples/todo/todo.telos:97` and 119):

```telos
test fn request(method: Text, path: Text, body: Text) -> Request =
  assert answer(todos, "GET", "/todos", "") == (200, "[]")
test fn request(method: Text, path: Text, body: Text = "") -> Request =
  assert answer(todos, "GET", "/todos") == (200, "[]")
```

Other site: `find` and `find_from` (`std/text.telos:94`, 98) merge into
`fn find(text: Text, needle: Text, start: Int = 0)`, one definition less.

Typing: defaulted parameters come after the others; a default is pure and follows the field
default rules (slice §4.7, E010). At a call, positional arguments come first, then named ones
in any order, each naming a parameter once. An unknown name, a repeat, a positional argument
after a named one, or a required parameter after a defaulted one is E074. The receiver of a
UFCS call is the first positional argument. Fn types carry no defaults and no names for calls:
calling a fn value with fewer arguments than its type, or with a named argument, is E075.
Protocol roles take no defaults (E030), because components call them by position.

Desugar: at each call, fill the omitted defaults and order the arguments; the backends see
plain calls. Cost: `telos-syntax` S, `telos-check` S, `telos-types` S, backends none. Effects:
none hidden, because defaults are pure. `wasm.md` E082 is unaffected: a fn value has no
defaults.

### P11 Pair-list literals `["key": value]`

Syntax: `primary := '[' expr ':' expr (',' expr ':' expr)* ']'`. Mixing pair and plain
elements is E001.

Before (`examples/todo/todo.telos:27`) and after:

```telos
  Object([("id", Number(todo.id)), ("title", Str(todo.title)), ("done", Bool(todo.done))])
  Object(["id": Number(todo.id), "title": Str(todo.title), "done": Bool(todo.done)])
```

Typing and desugar: `[k: v, …]` is `[(k, v), …]` of type `List<(K, V)>`. When `Map<K, V>`
arrives (`vision.md` §5, wave A; `Telos_Design.md` §3.3) the same literal builds a `Map` where
the expected type is one, so the syntax is reserved for both now. Cost: `telos-syntax` S only.
44 sites, 88 tokens (JSON objects, HTTP headers, test fixtures). Effects: none.

## 5 Rejected

| candidate | reason |
|---|---|
| pipe `\|>` (Elixir, F#, Gleam) | UFCS already is the pipe: `x.f(a).g(b)` (slice §4.8); a second spelling of one call adds a token and no power |
| implicit `it` or `_` lambdas (Kotlin, Scala) | saves `\|x\|` and hides which value a body refers to; slice 9 closures are short enough |
| trailing closures, Gleam `use`, Roc backpassing | hide control flow in a closure; Roc removed backpassing; a closure cannot cross a component cut (`wasm.md` E082); `?` already gives direct style |
| list comprehensions | closures with std `map` and `filter` cover them without a new form |
| format specs in interpolation (`{x:>8}`) | a library fn (`pad_left`) does it; no corpus site needs one |
| multi-scrutinee `match a, b` | clashes with `,` as the arm separator (slice §3); the tuple form `match (a, b)` works today |
| safe navigation `?.` | P3 plus `option.map` covers it without a second absence operator |
| `defer` | a reserved word for 2 sites (`telnet.telos:24`, `http.telos` session close); the role closing its stream is what tests rely on |
| Kotlin scope functions, `with` blocks | an implicit receiver hides which handle an effect uses; authority must be readable at the call |
| computation expressions, `do`, OCaml `let*` | user-defined control flow is deferred (`Telos_Design.md` §3.5); `docs/research/lean.md` §4 rejects monadic `do` |
| Koka effect handlers | Telos effects are capabilities on roots (slice §5), not algebraic effects to intercept |
| Pkl amending, CUE unification for `system` | field defaults (slice §4.7) and P7 give the same shortening without a second evaluation model |
| inline anonymous resources in components | a resource has an identity the graph names (`protocols.md` §2.3, `Telos_Design.md` §3.4) |
| table-test sugar | each case is one `assert` line already, and a failure prints the source text (slice §12.5) |
| leading-dot continuation lines | conflicts with the rule that a postfix `.` starts on the operand's line (slice §3, line 82); revisit with a formatter |
| `const` type inference from the literal | 15 sites, 30 tokens; a constant's type is part of an exported API |
| quantity literals (`4096 bytes`) | in the design (`Telos_Design.md` §3.3), owned by wave F (`vision.md` §4), not syntax sugar |
| grouped `requires [stream.{read, write}]` | 4 sites; `requires` is inferred since slice 6 and written only on roles |
| implicit conversions (`Text` to `Bytes`) | 64 explicit sites are the price of no implicit conversions (`Telos_Design.md` §3.5) |
| range slicing `xs[a..b]` | waits for `Buffer` and indexed places (`wasm.md` G2, G3); `.slice(a, b)` exists |

## 6 Priority

Lines saved are across the corpus of section 2.1, measured for S0 and estimated from the
detector lists for the others. Cost is the sum over layers from section 4.

| rank | item | lines saved | tokens saved | cost | reason for the rank |
|---|---|---|---|---|---|
| 1 | S0 one-line early exits | 78 | 0 | none | largest saving, no compiler work |
| 2 | P1 `is` | 39 | about 120 | 5 × S | second largest; also gives `assert e is P` |
| 3 | P5 `for` patterns, ranges | 27 | 108 | 3 × S | removes the counter-loop bug class (`continue` skipping `i = i + 1`) |
| 4 | P3 `?` on `Option` | 21 | 64 | 2 × S | one rule in the typer |
| 5 | P6 `+=`, `+` on lists | 15 | 50 | 4 × S | also the in-place append that the LLVM audit wants explicit (`vision.md` §10.1) |
| 6 | P2 let-else | 12 | about 40 | 5 × S | flattens the nested matches of todo and http |
| 7 | P4 match guards | 12 | about 30 | 4 × S | needed by state machines (`decode`) and actors' `handle` |
| 8 | P7 record update | 2 now | — | 4 × S | already required by slice 9 row 9; every actor reducer (`actors.md` §9) |
| 9 | P10 defaults, named args | 3 | 26 | 3 × S | the design's own call syntax (`Telos_Design.md:448`) |
| 10 | P11 pair-list literals | 0 | 88 | 1 × S | JSON and headers; reserves the `Map` literal |
| 11 | P9 rest patterns | 0 now | — | S + M | `protocols.md` P1 needs it |
| — | P8 visibility | 0 | −46 | M | question Q2 |

Total of ranks 1 to 11: about 209 lines of 2696 (7.8 %), 78 of them without compiler work.
Reserved words: 29 to 30 (`is`); `pub` would be contextual.

## 7 Slice 10 contract draft

### 7.1 Grammar delta against slice §3

```
stmt       += 'let' pattern (':' type)? '=' expr 'else' block       P2; expr not ending in '}', not && or ||
            | place ('+=' | '-=') expr                              P6
            | 'for' pattern 'in' (expr | expr ('..' | '..=') expr) block   P5 (replaces 'for' ident)
compare    := additive (('<' | '>' | '<=' | '>=') additive | 'is' pattern)?   P1
param      := ident ':' type ('=' expr)?                            P10
args       := (expr (',' expr)*)? (',' ident '=' expr)*             P10; or only named
primary    += ident '{' (init sep)* ('..' expr sep?)? '}'           P7, record update
            | '[' expr ':' expr (',' expr ':' expr)* ']'            P11
arm        := pattern ('if' expr)? '=>' (block | expr)              P4
alt        += '[' (pattern ',')* ('..' ident? | pattern) (',' pattern)* ']'   P9, one '..'
```

Keyword table: add `is`. Contextual words: none new.

### 7.2 Rules

| id | rule | section |
|---|---|---|
| R1 | `e is P` is `Bool`; bindings flow into `&&` right operands, `if` then-blocks and `while` bodies, nowhere else | 4 P1 |
| R2 | `let P = e else B`: `B` has type Never; bindings scope over the rest of the block | 4 P2 |
| R3 | `?` on `Option<T>` in a fn or closure returning `Option<U>`; no conversion between `Option` and `Result` | 4 P3 |
| R4 | match guards are pure `Bool`; guarded arms do not count for E046; `is` in a guard binds nothing for the arm | 4 P4 |
| R5 | `for` patterns are irrefutable `let` patterns; `a..b` and `a..=b` only in `for` heads, `Int` bounds evaluated once; the counter is a fresh immutable binding | 4 P5 |
| R6 | `p += e` is `p = p + e` with `p` evaluated once; `+` concatenates `List<T>` and `Bytes`; in-place append on `var` places | 4 P6 |
| R7 | `R { …, ..b }`: `b: R`; unlisted fields from `b`; source-order evaluation; defaults unused | 4 P7 |
| R8 | list rest patterns: one `..`, `..name` last, binds the tail; length-set exhaustiveness | 4 P9 |
| R9 | trailing pure parameter defaults; named arguments after positional ones; none through fn values or on roles | 4 P10 |
| R10 | `[k: v, …]` is `List<(K, V)>` until `Map` exists | 4 P11 |
| R11 | std and examples follow S0 | 4 S0 |

### 7.3 Diagnostics

`vision.md` §6 (X6) moves the actors' codes to E060 to E064, `wasm.md` takes E080 to E083 and
`protocols.md` E090 to E098. This slice takes E070 to E076.

| code | meaning | layer |
|---|---|---|
| E070 | the `else` block of a `let … else` can finish (its type is not Never) | types |
| E071 | a refutable pattern in a `for` head | check |
| E072 | an effectful call or `?` in a match guard | types |
| E073 | a range `..` or `..=` outside a `for` head or a pattern | syntax |
| E074 | a named argument that names no parameter or repeats one; a positional argument after a named one; a required parameter after a defaulted one | check |
| E075 | a named or omitted argument in a call through a fn value; a default on a protocol role (also E030) | types |
| E076 | a second `..` in a list pattern, or `..name` not last | syntax |

Reused: E001 (initializer ending in `}` before `else`; mixed pair and plain list elements),
E003 (an `is` binding used out of scope), E006 (`+=` on a `let` binding), E040, E045 (`+` on
other types), E046, E047.

### 7.4 Acceptance

1. `examples/todo/todo.telos` is the section 8.1 text and passes its tests on both targets.
2. `std/protocol/telnet.telos` lines 115 to 175, 268 to 296 and 356 to 387 are the section
   8.2 text, and `task gate` passes.
3. Each code E070 to E076 has a fixture (AGENTS.md, conventions).
4. A golden file per backend for `is` in `if`, `while` and value position, let-else, a guarded
   match, a `for` range with `continue`, and `+=` on a `List` var.
5. The corpus of section 2.1, counted with the same script, shrinks by at least 150 lines
   against the post-slice-9 count.

Numbering: `wasm.md` §4.3 (lines 350, 351) and its plan (line 670) assign G1 to G4 to "slices
9 and 10". Decision taken here: this syntax slice is slice 10, and wasm's G1 to G4 move to the
slice after it; the place grammar change of P6 is where G3 `xs[i] = e` attaches.

## 8 Rewrites

Line counts are `wc -l` of the full text. The slice-10 versions also use slice 9 features
(raw strings, `unwrap_or`, `Response.json`, globs, closures, `.1`, `!=`, range patterns); the
credit column separates them.

### 8.1 `examples/todo/todo.telos`: 151 lines to 133

| part | current | proposed | credit |
|---|---|---|---|
| imports | 4 | 3 | slice 9 glob and `std.option` |
| `or` and its blank line | 5 | 0 | slice 9 `unwrap_or` |
| `create` | 11 | 7 | P2 |
| `patch` | 13 | 11 | P2, P7, slice 9 closure and atomic `update` |
| `delete` | 4 | 2 | P1 |
| `handle` | 15 | 13 | possible today: one `match (method, Int.parse(id))` |
| test `to_json and or` | 6 | 4 | slice 9 (`or` gone) |
| test bodies | | | raw strings (slice 9), defaulted `body` (P10), pair literals (P11): tokens, not lines |
| total | 151 lines, 1332 tokens | 133 lines, 1263 tokens | 8 lines slice 10, 8 slice 9, 2 today |

Current, the parts that change (`todo.telos:21` to 24, 48 to 65; `create` is in section 4 P2):

```telos
fn or<T>(value: Option<T>, fallback: T) -> T = match value {
  Some(present) => present
  None => fallback
}

fn patch(todos: Store<Todo>, id: Int, body: Text) -> Response = match (todos.get(id), parse(body)) {
  (None, _) => status_response(404)
  (Some(todo), Ok(value)) => {
    let changed = Todo {
      id
      title = or(value.text("title"), todo.title)
      done = or(value.bool("done"), todo.done)
    }
    todos.put(id, changed)
    json(200, to_json(changed))
  }
  _ => status_response(400)
}

fn delete(todos: Store<Todo>, id: Int) -> Response = match todos.remove(id) {
  Some(_) => status_response(204)
  None => status_response(404)
}
```

Proposed, the whole file. `patch` keeps "404 before 400" (current arm order, line 49) and is
atomic through `update`; a todo deleted between `get` and `update` answers 404.

```telos
package examples.todo

import std.json.{Json, parse, text, bool}
import std.option.unwrap_or
import std.protocol.http.*

// A todo list served over HTTP/1.1 with JSON bodies, kept in memory:
//   GET /todos            every todo, in id order
//   POST /todos           {"title": "milk"} creates one; 201 with the todo
//   GET /todos/{id}       one todo, or 404
//   PATCH /todos/{id}     {"title": .., "done": ..}, either optional
//   DELETE /todos/{id}    204, or 404

type Todo {
  id: Int
  title: Text
  done: Bool = false
}

fn to_json(todo: Todo) -> Json =
  Object(["id": Number(todo.id), "title": Str(todo.title), "done": Bool(todo.done)])

fn list(todos: Store<Todo>) -> Response = Response.json(200, Array(todos.values().map(to_json)))

fn create(todos: Store<Todo>, body: Text) -> Response {
  let Ok(value) = parse(body) else { return status_response(400) }
  let Some(title) = value.text("title") else { return status_response(400) }
  let todo = Todo { id = todos.next_id(), title, done = value.bool("done").unwrap_or(false) }
  todos.put(todo.id, todo)
  Response.json(201, to_json(todo))
}

fn show(todos: Store<Todo>, id: Int) -> Response = match todos.get(id) {
  Some(todo) => Response.json(200, to_json(todo))
  None => status_response(404)
}

fn patch(todos: Store<Todo>, id: Int, body: Text) -> Response {
  if todos.get(id) is None { return status_response(404) }
  let Ok(value) = parse(body) else { return status_response(400) }
  let changed = todos.update(id, |t| Todo {
    title = value.text("title").unwrap_or(t.title)
    done = value.bool("done").unwrap_or(t.done)
    ..t
  })
  let Some(todo) = changed else { return status_response(404) }
  Response.json(200, to_json(todo))
}

fn delete(todos: Store<Todo>, id: Int) -> Response =
  status_response(if todos.remove(id) is Some(_) { 204 } else { 404 })

fn handle(todos: Store<Todo>, request: Request) -> Response = match (request.method, request.segments()) {
  ("GET", ["todos"]) => list(todos)
  ("POST", ["todos"]) => create(todos, request.body)
  (_, ["todos"]) => method_not_allowed("GET, POST")
  (method, ["todos", id]) => match (method, Int.parse(id)) {
    (_, None) => status_response(404)
    ("GET", Some(n)) => show(todos, n)
    ("PATCH", Some(n)) => patch(todos, n, request.body)
    ("DELETE", Some(n)) => delete(todos, n)
    _ => method_not_allowed("GET, PATCH, DELETE")
  }
  _ => status_response(404)
}

system TodoService {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8080 }
  resource todos: Store<Todo>

  component server: Service {
    listener = ingress
    protocol = Http
    application = HttpApp { state = todos, handle }
    executable = "todod"
  }
}

// ---- tests ------------------------------------------------------------------

test fn request(method: Text, path: Text, body: Text = "") -> Request =
  Request { method, path, query = "", version = "HTTP/1.1", headers = [], body }

// The status and body of the response to `method path` with `body`.
test fn answer(todos: Store<Todo>, method: Text, path: Text, body: Text = "") -> (Int, Text) {
  let response = handle(todos, request(method, path, body))
  (response.status, response.body)
}

// The status of the response to `method path` with `body`.
test fn status(todos: Store<Todo>, method: Text, path: Text, body: Text = "") -> Int =
  handle(todos, request(method, path, body)).status

test "to_json" {
  let milk = Object(["id": Number(1), "title": Str("milk"), "done": Bool(false)])
  assert to_json(Todo { id = 1, title = "milk" }) == milk
}

test "handle creates, lists, reads, updates and deletes" {
  let todos: Store<Todo> = Store.new()
  assert answer(todos, "GET", "/todos") == (200, "[]")
  assert answer(todos, "POST", "/todos", r#"{"title":"say \"hi\""}"#) == (201, r#"{"id":1,"title":"say \"hi\"","done":false}"#)
  assert answer(todos, "POST", "/todos/", r#"{"title":"eggs","done":true}"#) == (201, r#"{"id":2,"title":"eggs","done":true}"#)
  assert answer(todos, "GET", "/todos") == (200, r#"[{"id":1,"title":"say \"hi\"","done":false},{"id":2,"title":"eggs","done":true}]"#)
  assert answer(todos, "GET", "/todos/2") == (200, r#"{"id":2,"title":"eggs","done":true}"#)
  assert answer(todos, "PATCH", "/todos/1", r#"{"done":true}"#) == (200, r#"{"id":1,"title":"say \"hi\"","done":true}"#)
  assert answer(todos, "PATCH", "/todos/1", r#"{"title":"tea","id":9}"#) == (200, r#"{"id":1,"title":"tea","done":true}"#)
  assert answer(todos, "DELETE", "/todos/1") == (204, "")
  assert answer(todos, "GET", "/todos/1") == (404, "Not Found\n")
  assert answer(todos, "DELETE", "/todos/1") == (404, "Not Found\n")
  assert answer(todos, "GET", "/todos") == (200, r#"[{"id":2,"title":"eggs","done":true}]"#)
}

test "handle answers 400, 404 and 405" {
  let todos: Store<Todo> = Store.new()
  assert status(todos, "POST", "/todos", r"{not json") == 400
  assert status(todos, "POST", "/todos", r#"{"done":true}"#) == 400
  assert status(todos, "POST", "/todos", r#"{"title":7}"#) == 400
  assert status(todos, "PATCH", "/todos/1", r"{}") == 404
  assert status(todos, "GET", "/") == 404
  assert status(todos, "GET", "/nothing") == 404
  assert status(todos, "GET", "/todos/abc") == 404
  assert status(todos, "GET", "/todos/1/2") == 404
  assert status(todos, "POST", "/todos", r#"{"title":"x"}"#) == 201
  assert status(todos, "PATCH", "/todos/1", "[1") == 400
  assert answer(todos, "GET", "/todos") == (200, r#"[{"id":1,"title":"x","done":false}]"#)
  let put = handle(todos, request("PUT", "/todos"))
  assert put.status == 405
  assert put.headers.at(1) == ("Allow", "GET, POST")
  let post = handle(todos, request("POST", "/todos/1"))
  assert post.status == 405
  assert post.headers.at(1) == ("Allow", "GET, PATCH, DELETE")
}
```

### 8.2 `std/protocol/telnet.telos`, the codec: 124 lines to 96

The part is `telnet.telos:115` to 175 (`decode`, `feed`, `events`, `filled`), 268 to 296
(`render`, `encode_input`) and 356 to 387 (`erase_last`, `apply_key`), comments included,
joined by two blank lines. Tokens 830 to 784.

| fn | current | proposed | credit |
|---|---|---|---|
| `decode` | 30 | 26 | P4 |
| `feed` | 15 | 12 | S0 and slice 9 `!=` (the `Nothing => {}` match) |
| `events` | 5 | 2 | slice 9 `.1` |
| `filled` | 8 | 6 | P5 range, S0 |
| `render` | 9 | 6 | P6 |
| `encode_input` | 20 | 16 | P5 (−3), P4 (−1) |
| `erase_last` | 8 | 6 | S0, P1 with a slice 9 range pattern, P6 `-=` |
| `apply_key` | 23 | 16 | P4 (−2 net), P6 with a slice 9 range pattern (−5) |
| total | 124 | 96 | 15 slice 10, 7 S0, 6 slice 9 |

Current, the functions that change beyond `decode` (whose change is in section 4 P4):

```telos
test fn feed(state: Decoder, bytes: Bytes) -> (Decoder, List<Event>) {
  var at = state
  var events: List<Event> = List.empty()
  for b in bytes {
    let (next, event) = decode(at, b)
    at = next
    match event {
      Nothing => {}
      _ => events.push(event)
    }
  }
  (at, events)
}

test fn events(bytes: Bytes) -> List<Event> {
  let (_, out) = feed(Ground, bytes)
  out
}

test fn filled(count: Int, b: Byte) -> Bytes {
  var out = Bytes.empty()
  while out.len() < count {
    out.push(b)
  }
  out
}

fn encode_input(bytes: Bytes) -> Bytes {
  var out = Bytes.empty()
  var i = 0
  while i < bytes.len() {
    let b = bytes.at(i)
    i = i + 1
    match b {
      b'\n' => out.append(b"\r\n")
      b'\r' => if i >= bytes.len() || bytes.at(i) != b'\n' {
        out.append(b"\r\0")
      }
      IAC => out.append([IAC, IAC])
      _ => out.push(b)
    }
  }
  out
}

fn erase_last(line: Bytes) -> Bytes {
  var end = line.len() - 1
  while end > 0 && line.at(end) >= 128 && line.at(end) < 192 {
    end = end - 1
  }
  line.slice(0, end)
}

fn apply_key(line: Bytes, b: Byte) -> (Bytes, Edit) = match b {
  b'\r' | b'\n' => (Bytes.empty(), Enter(line))
  BACKSPACE | DELETE => if line.is_empty() {
    (line, Unchanged)
  } else {
    (erase_last(line), Show(b"\x08 \x08"))
  }
  CONTROL_C => (Bytes.empty(), Interrupt)
  CONTROL_D => if line.is_empty() { (line, EndOfInput) } else { (line, Unchanged) }
  ESCAPE => (line, Escape)
  _ => if b < b' ' {
    (line, Unchanged)
  } else {
    var longer = line
    longer.push(b)
    (longer, Show([b]))
  }
}
```

Proposed (`render` is in section 4 P6; doc comments unchanged and left out here):

```telos
test fn feed(state: Decoder, bytes: Bytes) -> (Decoder, List<Event>) {
  var at = state
  var events: List<Event> = []
  for b in bytes {
    let (next, event) = decode(at, b)
    at = next
    if event != Nothing { events.push(event) }
  }
  (at, events)
}

test fn events(bytes: Bytes) -> List<Event> = feed(Ground, bytes).1

test fn filled(count: Int, b: Byte) -> Bytes {
  var out = Bytes.empty()
  for _ in 0..count { out.push(b) }
  out
}

fn encode_input(bytes: Bytes) -> Bytes {
  var out = Bytes.empty()
  for i in 0..bytes.len() {
    match bytes.at(i) {
      b'\n' => out.append(b"\r\n")
      b'\r' if i + 1 == bytes.len() || bytes.at(i + 1) != b'\n' => out.append(b"\r\0")
      b'\r' => {}
      IAC => out.append([IAC, IAC])
      other => out.push(other)
    }
  }
  out
}

fn erase_last(line: Bytes) -> Bytes {
  var end = line.len() - 1
  while end > 0 && line.at(end) is 0x80..=0xBF { end -= 1 }
  line.slice(0, end)
}

fn apply_key(line: Bytes, b: Byte) -> (Bytes, Edit) = match b {
  b'\r' | b'\n' => (Bytes.empty(), Enter(line))
  BACKSPACE | DELETE if line.is_empty() => (line, Unchanged)
  BACKSPACE | DELETE => (erase_last(line), Show(b"\x08 \x08"))
  CONTROL_C => (Bytes.empty(), Interrupt)
  CONTROL_D if line.is_empty() => (line, EndOfInput)
  CONTROL_D => (line, Unchanged)
  ESCAPE => (line, Escape)
  0x00..=0x1F => (line, Unchanged)
  _ => (line + [b], Show([b]))
}
```

Behaviour is unchanged: `encode_input`'s CR test reads the next byte at `i + 1` because the
range counter is not advanced inside the body; `apply_key`'s `0x00..=0x1F` is `b < b' '`
after the named control keys, and those keys stay earlier arms.

## 9 Open questions

| # | question | default if unanswered |
|---|---|---|
| Q1 | Should `is` be a reserved word, or should P1 use `matches(e, P)`, which needs a pattern argument and is a form of its own anyway? | reserved `is` (29 to 30 words) |
| Q2 | Visibility (P8): private by default with a contextual `pub`, everything public as now, or Dart-style `_name` privacy? It decides the std definition count (136 now, budget 120) and the glob collisions of six names | keep everything public; rename the six colliding names |
| Q3 | Should match guards (P4) allow effectful calls, as Rust does, instead of E072? | pure guards |
| Q4 | Record update (P7): slice 9 needs it for todo's PATCH. Does the slice 9 agent own it, with this section as its rule? | yes; this document's R7 is the rule |
| Q5 | Numbering: this syntax slice as slice 10, and wasm's G1 to G4 as the next slice, or the reverse? | syntax first (section 7.4) |
| Q6 | Does S0 become a formatter (`telos fmt`) in slice 10, or stay a written convention? | written convention; a formatter later |
