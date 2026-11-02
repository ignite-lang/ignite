proposal, not scheduled

# Ignite syntax: less ceremony, same explicitness

Status: proposal drafted 2026-11-01. Nothing here is implemented or scheduled. Under
`vision.md` §7 T2 a future design stays a proposal until a wave adopts it, and `vision.md` §4.1
plans no sugar slice before the system-declaration slice (slice 10, wave F1): a sugar item
enters a slice only when that slice's declarations need it. Section 7 lists what slice 10
would need from here; section 8 is a candidate contract for a later sugar slice.

The proposal builds on slice 9 as decided and treats it as landed: closures `|x| e`, record
update `R { f = e, ..base }`, raw strings, glob imports, range patterns, tuple `.0`, structural
`==`, `break` as an expression, associated fns, `std/option.ignite`, `std/result.ignite`.

Timo's request: "make a design to improve syntax in general and make it more sugary and less
verbose".

## 1 Scope and rules

### 1.1 Constraints

| constraint | source |
|---|---|
| 30 to 50 reserved words; about 120 standard definitions around version 1 | `design.md` §1.5 (line 82), §3.6 (line 210) |
| defer macros, implicit conversions, unrestricted reflection, user-defined syntax | `design.md` §3.5 (line 163) |
| runtime effects need declared capabilities; no effect is hidden | `design.md` §3.1; `slice.md` §5 |
| 29 reserved words today, `true` and `false` included | `crates/ignite-syntax/src/lexer.rs` keyword table |
| a fn value crossing a component cut is a named module-level fn, never a closure | `wasm.md` §3.1, E082 |
| roles become generic over a stream bound with a `Limits` record | `protocols.md` §2.2, §4 |
| actors are pure `handle(state, msg) -> Step<S>` reducers | `actors.md` §3, §9 |
| postfix `.`, `(`, `[`, `?` start on the operand's line; `else` follows `}`; `,` or newline separates arms and statements | `slice.md` §3 (line 82), grammar lines 88 to 158 |

### 1.2 When a proposal is taken

1. It is a pure desugar, or a small typed node whose lowering both backends already have.
2. It removes lines or tokens counted in section 2, or a future design asks for it.
3. It hides no effect, no authority root and no conversion.
4. It costs at most one reserved word.
5. A reader of Rust, Kotlin, Swift or Gleam reads it without the manual.

## 2 Measurement

### 2.1 Corpus

`measure.py` (session scratch directory, run as `python3 -I measure.py <repo>`) strips
comments, tokenizes with one regular expression (a literal is one token) and runs 29 pattern
detectors.

| file | lines | blank | comment | code | tokens | test lines |
|---|---|---|---|---|---|---|
| `std/text.ignite` | 334 | 36 | 28 | 270 | 2258 | 157 |
| `std/json.ignite` | 608 | 51 | 37 | 520 | 4469 | 179 |
| `std/console.ignite` | 109 | 14 | 12 | 83 | 557 | 40 |
| `std/protocol/telnet.ignite` | 602 | 47 | 60 | 495 | 3992 | 189 |
| `std/protocol/http.ignite` | 678 | 65 | 61 | 552 | 4126 | 251 |
| `examples/terminal/terminal.ignite` | 96 | 13 | 2 | 81 | 608 | 29 |
| `examples/todo/todo.ignite` | 151 | 20 | 9 | 122 | 1332 | 48 |
| `examples/terminal/terminal.design.ignite` | 118 | 14 | 0 | 104 | 434 | 0 |
| total | 2696 | 260 | 209 | 2227 | 17776 | 893 |

### 2.2 The ten most frequent verbose patterns

"Saved" is the cost over the shortest form a proposal of section 4 allows, summed per site.

| # | pattern | sites | saved | example | proposal |
|---|---|---|---|---|---|
| 1 | three-line early exit `if c {` / `return x` / `}` | 39 | 78 lines | `std/text.ignite:68` | S0 |
| 2 | two-arm `match` whose default arm yields `None`, `false` or `{}` | 21 | 63 lines | `std/json.ignite:534` | P1, P3 |
| 3 | `None => None` or `_ => None` forwarding absence | 16 | 16 lines, 64 tokens | `std/text.ignite:177` | P3 |
| 4 | accumulator loop: `var out`, `for`, `append`, `out` | 5 | 29 lines | `std/text.ignite:255` | P6 |
| 5 | counter `while`: `var i = 0`, `while i < n`, `i = i + 1` | 10 | 20 lines, 80 tokens | `std/text.ignite:76` | P5 |
| 6 | `x = x + e` | 25 | 50 tokens | `std/text.ignite:76` | P6 |
| 7 | run of `push`/`append` after a `var` | 9 | 15 lines | `telnet.ignite:251` | P6 |
| 8 | pair literal `("key", value)` in a list | 44 | 88 tokens | `std/json.ignite:97` | P11 |
| 9 | `for pair in xs` then `let (a, b) = pair` | 7 | 7 lines, 28 tokens | `http.ignite:110` | P5 |
| 10 | empty arm `_ => {}` or `Nothing => {}` | 9 | 9 lines | `std/json.ignite:202` | P1 |

Counted but not targeted: 75 lines with 336 `\"` and `{{` escapes (raw strings, slice 9);
64 `.to_bytes()`/`.to_text()` conversions (explicit by `design.md` §3.5); 52 runs of three
or more `assert`s on one callee, 345 asserts (one line per case already; slice §12.5 prints the
failing source); 9 `match stream.read(4096)? {` loops (owned by `protocols.md` P1).

## 3 Comparable sugar

"y" in the last column: the page was read on 2026-11-01; "n": cited from the documented
feature, page not fetched (the Swift book renders client-side).

| language | feature | behaviour | URL | used for | read |
|---|---|---|---|---|---|
| Rust | let-else | else block has the never type; initializer may not end in `}` or be `&&`/`\|\|` | https://doc.rust-lang.org/reference/statements.html#let-statements | P2 | y |
| Rust | let chains | `if let P = e && c` in `if`, `while`; Rust 1.88, edition 2024 | https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/ | P1 lowering | y |
| Rust | `?` | early return of `Err` or `None` | https://doc.rust-lang.org/reference/expressions/operator-expr.html#the-question-mark-operator | P3 | n |
| Rust | if-let guards | unstable | https://github.com/rust-lang/rust/issues/51114 | P4 binds nothing | n |
| C# | `is` pattern | Bool; `x is int a && y is int b` binds in the true branch; list patterns with `..` | https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/operators/is | P1, P9 | y |
| Kotlin | scope functions | "execute a block of code within the context of an object" | https://kotlinlang.org/docs/scope-functions.html | rejected | y |
| Kotlin | trailing lambdas, `it` | last fn argument outside the parentheses | https://kotlinlang.org/docs/lambdas.html | rejected | n |
| Swift | `guard let … else` | else must leave the scope | https://docs.swift.org/swift-book/documentation/the-swift-programming-language/controlflow/ | P2 | n |
| Swift | dictionary literal | `["a": 1]` | https://docs.swift.org/swift-book/documentation/the-swift-programming-language/collectiontypes/ | P11 | n |
| Gleam | `use` | "All the code below the `use` becomes an anonymous function" | https://tour.gleam.run/advanced-features/use/ | rejected | y |
| Gleam | labelled arguments | call by label | https://tour.gleam.run/functions/labelled-arguments/ | P10 | n |
| Elixir | `\|>`, `with` | pipe into the first argument; chained matches with one `else` | https://hexdocs.pm/elixir/Kernel.SpecialForms.html | rejected; P2 covers `with` | n |
| F#, OCaml | computation expressions, `let*` | user-defined binding | https://learn.microsoft.com/en-us/dotnet/fsharp/language-reference/computation-expressions, https://ocaml.org/manual/5.2/bindingops.html | rejected | n |
| Roc | `?`, `!` suffix | `?` returns the `Err`; backpassing absent from the tutorial | https://www.roc-lang.org/tutorial | P3; backpassing rejected | y |
| Roc | record builders | `<-` kept after backpassing went | https://roc-lang.github.io/zulip-export/stream/383402-API-design/topic/record.20builder.20syntax.html | rejected | y |
| Koka | effect handlers | `with handler { … }` | https://koka-lang.github.io/koka/doc/book.html#sec-handlers | rejected | y |
| Pkl | amending | `(base) { field = … }` | https://pkl-lang.org/main/current/language-reference/index.html#amending-objects | field defaults, record update | y |
| CUE | defaults | `*` marks the preferred value | https://cuelang.org/docs/tour/types/defaults/ | field defaults | y |

What the table says: languages that removed nesting without hiding control flow used three
devices, all missing in Ignite: a Bool pattern test (C# `is`), a binding that exits on mismatch
(Rust let-else, Swift `guard`) and match guards. The devices that move control flow into a
closure (Gleam `use`, Roc backpassing, F# builders, Kotlin trailing lambdas) are the ones Roc
dropped and the ones `wasm.md` E082 cannot lower at a component edge.

## 4 Proposals

Cost is estimated changed Rust lines per crate: S under 100, M 100 to 300. Section 9 shows
each proposal in the rewritten todo and telnet code.

### S0 One-line early exits (convention, no compiler change)

`if c { return x }` is legal today: a block holds one statement without a separator (slice §3,
`block`). The corpus writes 39 of them on three lines (`std/text.ignite:68` to 70). Rule for std
and examples: a block of one `return`, `break`, `continue` or call that fits in 100 columns
stays on its line. 78 lines. No effect change.

### P1 `e is P`, a Bool pattern test

Syntax: `compare := additive (cmp additive | 'is' pattern)?`; `is` binds tighter than `==`,
`&&`, `||` and looser than `+`. New reserved word `is` (29 to 30); the corpus has `is` only in
strings and comments (`grep -rnw is std examples`).

```ignite
fn as_text(value: Json) -> Option<Text> = match value {      // std/json.ignite:534, 4 lines
  Str(text) => Some(text)
  _ => None
}
fn as_text(value: Json) -> Option<Text> = if value is Str(text) { Some(text) } else { None }
```

Other sites: `is_whitespace` (`std/text.ignite:11`) becomes
`b is b' ' | b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r'`; the `_ => {}` arm at
`std/json.ignite:200` becomes `if bytes.at(i) is b'.' | b'e' | b'E' { return Err(…) }`; the
test at `http.ignite:605` to 612 (8 lines) becomes `assert next_of(…)? is Got(_, rest) &&
rest.is_empty()`; todo's `delete` drops from 4 lines to 2. About 39 lines.

Typing: `Bool`. `P` is checked as a match-arm pattern (E040, E003, constant and range
patterns). Its bindings are in scope where the test is known true: the right operand of an
`&&` that has the test as a conjunct, the then-block of `if`, the body of `while`. Not under
`!`, after `||`, in `else`, or after the statement; a use there is E003. `assert e is P`
needs no rule; a failure prints `e`'s debug text (slice 9 row 11).

Desugar: `if e is P && c { A } else { B }` is `match e { P => if c { A } else { B }, _ => B }`
with `B` emitted once; as a value, `match e { P => true, _ => false }`.

Cost: syntax S (node, keyword), check S (condition scopes), types S, rustgen S (let chains,
`while let`, `matches!`), llvm S to M (arm pattern test feeding a branch). Effects: `e` once,
with its own effects; nothing else runs.

### P2 `let … else`

Syntax: `stmt := 'let' pattern (':' type)? '=' expr 'else' block`. Only here may a `let`
pattern be refutable; elsewhere that stays E001 (slice §8).

Todo's `create` drops from 11 lines to 7 (section 9.1):
`let Ok(value) = parse(body) else { return status_response(400) }`. Also `http.ignite:444`
(`let request = match parse_head(…) { Some(parsed) => parsed, None => return … }`, 4 lines to
1) and `std/json.ignite:520` (`member`, with P5: 12 lines to 6). About 12 lines.

Typing: the else block has type Never (slice 9 row 4), otherwise E070. An initializer that
ends in `}` or whose top operator is `&&` or `||` is E001 with the hint "parenthesize", as in
Rust, so `else` never attaches to an inner `if`. Bindings are immutable and scope over the
rest of the block.

Desugar: `let P = e else { D }` then `rest` is `match e { P => { rest }, _ => D }`. Cost:
syntax S, check S, types S (E070), rustgen S (Rust `let … else` one to one), llvm S. Effects:
those of the else block, as for any statement; control flow reads top to bottom instead of
nesting.

### P3 `?` on `Option`

Today `?` needs a `Result` fn (E040 in `crates/ignite-types/src/typer.rs`, `try_`); tests
accept it on `Option` (slice §12.3). Rule: in a fn or closure returning `Option<U>`, `e?` with
`e: Option<T>` has type `T` and returns `None`. Mixing `Option` and `Result` stays E040; there
is no conversion. In a closure `?` returns from the closure.

```ignite
fn text(value: Json, name: Text) -> Option<Text> = match member(value, name) {   // json:555
  Some(Str(t)) => Some(t)
  _ => None
}
fn text(value: Json, name: Text) -> Option<Text> = member(value, name)?.as_text()
```

Also `int`, `bool` (`json:561`, 567), `split_once` (`text:175`), `hex4` (`json:268`, 10 lines
to 4), `unicode_escape`, `parse_head` (`http:305`). About 21 lines. Cost: types S, rustgen
none, llvm S. Effects: none.

### P4 Match guards

Syntax: `arm := pattern ('if' expr)? '=>' (block | expr)`. Example: the `Subnegotiation`
arm of `decode` (`telnet.ignite:136` to 142), 7 lines to 3 (section 9.2).

Typing: the guard is `Bool` and sees the arm's bindings. It is pure: an effectful call or `?`
in it is E072, so testing patterns never performs IO and the order in which a backend tests
arms stays unobservable. A guarded arm does not count for E046. An `is` in a guard binds
nothing for the arm, because Rust, the first target, has no stable if-let guards; such a use
is E003. A guard on an or-pattern is tested once for the arm.

Not a pure desugar (it would duplicate the remaining arms). Cost: syntax S, types S
(exhaustiveness skips guarded arms), rustgen S (native), llvm S to M (failed guard jumps to the
next arm's test). About 12 lines (`decode`, `apply_key`, `encode_input`).

### P5 `for` patterns and integer ranges

Syntax: `'for' pattern 'in' (expr | expr ('..' | '..=') expr) block`. The pattern is a `let`
pattern (a name always binds, slice §4.3) and irrefutable (E071).

```ignite
  for pair in response.headers {          // http.ignite:110
    let (name, value) = pair
  for (name, value) in response.headers {
```

Typing: `Int` bounds (E040) evaluated once before the loop; `a..b` half-open, `a..=b` closed.
A range is not a value: `..` outside a `for` head or a pattern is E073, so std gains no
`Range` type and the definition budget does not move.

Desugar of `for i in a..b { body }`: `{ var n = a; let end = b; while n < end { let i = n;
n = n + 1; body } }`. The increment precedes `body`, so `continue` cannot skip it, and `i` is a
fresh immutable binding per iteration (E006 on assignment), clear of the `for`-binding
aliasing defect M2 of `vision.md` §10.1. Cost: syntax S, check S, types S, backends none. About
27 lines (10 counter loops, 7 destructuring loops).

### P6 Compound assignment and concatenation

Syntax: `stmt := place ('+=' | '-=') expr`; `+` also concatenates `List<T>` and `Bytes`
(today E045). `render` (`telnet.ignite:270`) drops from 8 lines to 5:
`for line in reply.lines { out += line.to_bytes() + b"\r\n" }`; `apply_key`'s default arm
(6 lines) becomes `_ => (line + [b], Show([b]))`.

Typing: `p += e` is `p = p + e` with `p` evaluated once, for the types `+` allows (`Int`,
`Byte`, `Text`, `List<T>`, `Bytes`); `-=` for `Int` and `Byte`. A `let` binding or constant on
the left is E006. On a `var` place of `List`, `Bytes` or `Text` both backends append in place
(`extend_from_slice`, `push_str`, the arena regrow path); the value equals the copy because
the function owns the place. Cost: syntax S, types S, rustgen S, llvm S to M. About 15 lines
and 50 tokens. Effects: none.

### P8 Visibility (for question Q2)

Every module item is exported (slice §4.1). The five std modules export 136 items; about 46
are used outside their module. Six names collide between modules (`IoResult`, `render`,
`session`, `IDLE_TIMEOUT_MS` in telnet and http; `is_space`, `skip_space` in console and
json), and two globs providing a used name are E041 (slice 9 row 6). Private by default with a
contextual `pub` costs 46 tokens and brings std to about 46 definitions against the budget of
120. It adds tokens rather than removing them, so it is a question, not a contract item.

### P9 Rest patterns in list patterns

Syntax: one `..` anywhere in a list pattern, or one `..name` as the last element:
`[_, _, ..]`, `[first, ..rest]`, `[.., last]`. Wanted by `protocols.md` line 585
(`([_, _, ..], _, _) => Reject(400)`) and line 596, and by `vision.md` §5.

Typing: `..name` binds the remaining elements as `List<T>` (or `Bytes` where list patterns
match `Bytes`). A second `..`, or `..name` not last, is E076. A pattern with `k` fixed
elements and a rest covers every length `>= k`, so `[]` with `[_, ..]` is exhaustive.
Lowering: `len >= k`, element tests by index, `rest` as `slice(k, len)`. Cost: syntax S,
types M (length sets for E046), backends S. No corpus lines today.

### P10 Parameter defaults and named arguments

Syntax: `param := ident ':' type ('=' expr)?`; arguments are positional, then `name = expr`.
The design writes a named argument already: `nativeProcess(includes = [Terminal.console])`
(`design.md:668` to 670, `terminal.design.ignite:112` to 114).

Examples: todo's test helpers take `body: Text = ""`, dropping `, ""` from 13 calls
(section 9.1); `find` and `find_from` (`std/text.ignite:94`, 98) merge into
`fn find(text: Text, needle: Text, start: Int = 0)`.

Typing: defaulted parameters come last; a default is pure under the field-default rules
(slice §4.7, E010). Named arguments follow positional ones, each naming a parameter once. An
unknown or repeated name, a positional argument after a named one, or a required parameter
after a defaulted one is E074. A UFCS receiver is the first positional argument. Fn types
carry no defaults: a call through a fn value with a named or omitted argument is E075, so
`wasm.md` E082 is unaffected. Protocol roles take no defaults (E075).

Desugar at the call site: fill defaults, order arguments; backends see plain calls. Cost:
syntax S, check S, types S. Effects: none hidden, because defaults are pure.

Scheduled (steering session, 2026-11-01): slice 10 takes named arguments without defaults,
`f(a, b, name = e)`, positional arguments first, only on fns declared in Ignite, not through fn
values or on protocol roles. Defaults stay unscheduled.

### P11 Pair-list literals `["key": value]`

`[k: v, …]` is `[(k, v), …]`, type `List<(K, V)>`; mixing pair and plain elements is E001.
When `Map<K, V>` arrives (`vision.md` §5; `design.md` §3.3) the same literal builds a
`Map` where one is expected, so the form is reserved for both. Todo's `to_json` becomes
`Object(["id": Number(todo.id), "title": Str(todo.title), "done": Bool(todo.done)])`. Cost:
syntax S. 44 sites, 88 tokens. Effects: none.

Not proposed: record update `R { f = e, ..base }` is in slice 9 (decision row 9 uses
`Todo { done = flag, ..t }`); it shortens every actor reducer of `actors.md` §9.2 to §9.3
(`Todos { table = …, changes = state.changes }` becomes `Todos { table = …, ..state }`).

## 5 Rejected

| candidate | reason |
|---|---|
| pipe `\|>` | UFCS already is the pipe, `x.f(a).g(b)` (slice §4.8); a second spelling of one call |
| implicit `it` or `_` lambdas | hide which value a body refers to; slice 9 closures are short already |
| trailing closures, Gleam `use`, Roc backpassing | move control flow into a closure; Roc removed backpassing; closures cannot cross a component cut (E082); `?` gives direct style |
| comprehensions | closures with std `map` and `filter` cover them |
| format specs in interpolation | a library fn (`pad_left`) does it; no corpus site needs one |
| multi-scrutinee `match a, b` | clashes with `,` as arm separator (slice §3); `match (a, b)` works |
| `?.` | P3 plus `option.map` covers it |
| `defer` | a reserved word for 2 sites (`telnet.ignite:24`, http's session close) |
| Kotlin scope functions, `with` | an implicit receiver hides which handle an effect uses |
| computation expressions, `do`, `let*` | user-defined control flow is deferred (`design.md` §3.5); `docs/research/lean.md` §4 |
| Koka handlers | Ignite effects are capabilities on roots (slice §5), not algebraic effects |
| Pkl amending, CUE unification | field defaults (slice §4.7) and record update give the same shortening |
| inline anonymous resources | a resource has an identity the graph names (`protocols.md` §2.3, `design.md` §3.4) |
| table-test sugar | one `assert` line per case already; failures print the source (slice §12.5) |
| leading-dot continuation lines | clashes with postfix `.` on the operand's line (slice §3, line 82) |
| `const` type inference | 15 sites, 30 tokens; a constant's type is exported API |
| grouped `requires [stream.{read, write}]` | 4 sites; `requires` is inferred and written only on roles |
| implicit conversions | the 64 explicit sites are the price of none (`design.md` §3.5) |
| range slicing `xs[a..b]` | waits for `Buffer` and indexed places (`wasm.md` G2, G3); `.slice(a, b)` exists |

## 6 Priority

Lines saved across the corpus of section 2.1: measured for S0, estimated from the detector
lists for the rest.

| rank | item | lines | tokens | cost | why this rank |
|---|---|---|---|---|---|
| 1 | S0 one-line early exits | 78 | 0 | none | largest saving, no compiler work |
| 2 | P1 `is` | 39 | about 120 | 5 × S | also gives `assert e is P` |
| 3 | P5 `for` patterns, ranges | 27 | 108 | 3 × S | removes the `continue`-skips-increment bug class |
| 4 | P3 `?` on `Option` | 21 | 64 | 2 × S | one typer rule |
| 5 | P6 `+=`, `+` on lists | 15 | 50 | 4 × S | in-place append made explicit (`vision.md` §10.1) |
| 6 | P2 let-else | 12 | about 40 | 5 × S | flattens todo's and http's nested matches |
| 7 | P4 match guards | 12 | about 30 | 4 × S | state machines (`decode`), actors' `handle` |
| 8 | P10 defaults, named args | 3 | 26 | 3 × S | the design's call syntax; needed by slice 10 (section 7) |
| 9 | P11 pair-list literals | 0 | 88 | S | JSON and headers; reserves the `Map` literal |
| 10 | P9 rest patterns | 0 | — | S + M | `protocols.md` P1 |
| — | P8 visibility | 0 | −46 | M | question Q2 |

Total of ranks 1 to 10: about 207 of 2696 lines (7.7 %), 78 of them without compiler work.
Reserved words: 29 to 30 (`is`).

## 7 What the system-declaration slice would need

Slice 10 (`vision.md` §4.1, wave F1) adds quantity literals with units, `resource`,
`component name: Type { … }`, `contract`, `assembly` and `module("…")` references; its
acceptance is `ignite check examples/terminal/terminal.design.ignite` exit 0. Read against that
file, it needs three items of the kind this document is about. None of P1 to P6, P9 or P11 is
needed there; S0 does not apply to declarations.

| need | where | item | note |
|---|---|---|---|
| named arguments | `nativeProcess(includes = [Terminal.console])`, `terminal.design.ignite:112`; `exponential(100 ms, 10 seconds)` (line 47) is positional | P10, named arguments only | defaults are not needed; the alternative is a record argument `NativeProcess { includes = … }` (question Q5) |
| record literal typed by the expected type, `{ field = … }` | `sessions = { maximum = 1000 … }` (line 38), `commands = { connect = Command { … } }` (line 60) | not in section 4 | like variant inference (slice §10): the property's declared type names the record; `{` in property position cannot be a block, because blocks follow only `fn`, `if`, `match`, loops (slice §3) |
| keyed literal with path keys | `bindings = { Terminal.ingress = localSocket() … }` (line 110) | related to P11 | a map from component references to bindings, written with `=` in the design; P11's `:` form and this form should be one decision (question Q4) |

Quantity literals (`4096 bytes`, `99.9%`, `15 minutes`) are slice 10's own content, not sugar,
and are not proposed here.

## 8 Candidate contract for a later sugar slice

Not scheduled. A slice that adopts any part copies that part into `docs/slice.md` (T2).

### 8.1 Grammar delta against slice §3

```
stmt       += 'let' pattern (':' type)? '=' expr 'else' block       P2; expr not ending in '}', not && or ||
            | place ('+=' | '-=') expr                              P6
            | 'for' pattern 'in' (expr | expr ('..' | '..=') expr) block   P5, replaces 'for' ident
compare    := additive (('<' | '>' | '<=' | '>=') additive | 'is' pattern)?   P1
param      := ident ':' type ('=' expr)?                            P10
args       := (expr (',' expr)*)? (',' ident '=' expr)*             P10; or only named
primary    += '[' expr ':' expr (',' expr ':' expr)* ']'            P11
arm        := pattern ('if' expr)? '=>' (block | expr)              P4
alt        += '[' (pattern ',')* ('..' ident? | pattern) (',' pattern)* ']'   P9, one '..'
```

Keyword table: add `is`.

### 8.2 Rules

| id | rule | from |
|---|---|---|
| R1 | `e is P` is `Bool`; bindings flow into `&&` right operands, `if` then-blocks, `while` bodies only | P1 |
| R2 | `let P = e else B`: `B` is Never; bindings scope over the rest of the block | P2 |
| R3 | `?` on `Option<T>` in a fn or closure returning `Option<U>`; no `Option`/`Result` conversion | P3 |
| R4 | guards are pure `Bool`; guarded arms do not count for E046; `is` in a guard binds nothing for the arm | P4 |
| R5 | `for` patterns are irrefutable `let` patterns; ranges only in `for` heads, `Int` bounds evaluated once; fresh immutable counter | P5 |
| R6 | `p += e` is `p = p + e`, `p` evaluated once; `+` concatenates `List<T>` and `Bytes`; in place on `var` places | P6 |
| R7 | one `..` per list pattern, `..name` last, binding the tail; length-set exhaustiveness | P9 |
| R8 | trailing pure defaults; named arguments after positional ones; none through fn values or on roles | P10 |
| R9 | `[k: v, …]` is `List<(K, V)>` until `Map` exists | P11 |
| R10 | std and examples follow S0 | S0 |

### 8.3 Diagnostics

`vision.md` §6 (X6) gives the actors E060 to E064; `wasm.md` takes E080 to E083 and
`protocols.md` E090 to E098. This proposal takes E070 to E076.

| code | meaning | layer |
|---|---|---|
| E070 | the `else` block of a `let … else` can finish (its type is not Never) | types |
| E071 | a refutable pattern in a `for` head | check |
| E072 | an effectful call or `?` in a match guard | types |
| E073 | a range `..` or `..=` outside a `for` head or a pattern | syntax |
| E074 | a named argument that names no parameter or repeats one; a positional argument after a named one; a required parameter after a defaulted one | check |
| E075 | a named or omitted argument in a call through a fn value; a default on a protocol role | types |
| E076 | a second `..` in a list pattern, or `..name` not last | syntax |

Reused: E001 (initializer ending in `}` before `else`; mixed list elements), E003 (`is`
binding out of scope), E006 (`+=` on a `let` binding), E040, E045, E046, E047.

### 8.4 Acceptance

1. `examples/todo/todo.ignite` is the text of section 9.1 and passes its tests on both targets.
2. The telnet codec is the text of section 9.2, and `task gate` passes.
3. Each of E070 to E076 has a fixture (`AGENTS.md`, conventions).
4. A golden file per backend for `is` in `if`, `while` and value position, let-else, a guarded
   match, a `for` range with `continue`, and `+=` on a `List` var.
5. The corpus of section 2.1, counted by the same script, shrinks by at least 150 lines
   against its count after slice 9.

`wasm.md` §4.3 (lines 350, 351) and its plan (line 669) put G1 to G4 in "slices 9 and 10";
`vision.md` §4.1 now gives slice 10 to F1 and places wasm in slices 14 and later. This proposal
claims no slice number.

## 9 Rewrites

Line counts are `wc -l`. The rewrites also use slice 9 features (raw strings, `unwrap_or`,
`Response.json`, globs, closures, record update, `.1`, `!=`, range patterns); the credit
columns separate them.

### 9.1 `examples/todo/todo.ignite`: 151 lines to 133

| part | current | proposed | credit |
|---|---|---|---|
| imports | 4 | 3 | slice 9 glob and `std.option` |
| `or` and its blank line | 5 | 0 | slice 9 `unwrap_or` |
| `create` (`todo.ignite:31`) | 11 | 7 | P2 |
| `patch` (`:48`) | 13 | 11 | P2, P1; atomic `update` with record update from slice 9 |
| `delete` (`:62`) | 4 | 2 | P1 |
| `handle` (`:67`) | 15 | 13 | possible today: one `match (method, Int.parse(id))` |
| test `to_json and or` | 6 | 4 | slice 9 (`or` gone) |
| test bodies | | | raw strings (slice 9), defaulted `body` (P10), pair literals (P11): tokens only |
| total | 151 lines, 1332 tokens | 133 lines, 1263 tokens | 8 lines this proposal, 8 slice 9, 2 today |

`patch` keeps "404 before 400" (the current `(None, _)` arm comes first, line 49) and becomes
atomic through `update`; a todo deleted between `get` and `update` answers 404.

```ignite
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

### 9.2 `std/protocol/telnet.ignite`, the codec: 124 lines to 96

The part is `telnet.ignite:115` to 175 (`decode`, `feed`, `events`, `filled`), 268 to 296
(`render`, `encode_input`) and 356 to 387 (`erase_last`, `apply_key`), comments included,
joined by one blank line each. Tokens 830 to 787. Credit: P4 −7 (`decode` 4, `apply_key` 2,
`encode_input` 1), P6 −8 (`render` 3; `apply_key`'s default arm 5, with a slice 9 range
pattern), P5 −3 (`encode_input`), S0 −4 (`filled`, `erase_last`), slice 9 −6 (`feed` through
`!=`, `events` through `.1`). Current and proposed side by side as a unified diff, one line of
context:

```diff
--- current
+++ proposed
@@ -20,9 +20,5 @@
   (Negotiation(_), _) => (Ground, Nothing)
-  (Subnegotiation(count), _) => if count + 1 > MAXIMUM_SUBNEGOTIATION {
-    (Ground, SubnegotiationTooLong)
-  } else if b == IAC {
-    (SubnegotiationInterpret(count + 1), Nothing)
-  } else {
-    (Subnegotiation(count + 1), Nothing)
-  }
+  (Subnegotiation(count), _) if count + 1 > MAXIMUM_SUBNEGOTIATION => (Ground, SubnegotiationTooLong)
+  (Subnegotiation(count), IAC) => (SubnegotiationInterpret(count + 1), Nothing)
+  (Subnegotiation(count), _) => (Subnegotiation(count + 1), Nothing)
   (SubnegotiationInterpret(_), SE) => (Ground, Nothing)
@@ -39,6 +35,3 @@
     at = next
-    match event {
-      Nothing => {}
-      _ => events.push(event)
-    }
+    if event != Nothing { events.push(event) }
   }
@@ -48,6 +41,3 @@
 // The events of `bytes` decoded from `Ground`.
-test fn events(bytes: Bytes) -> List<Event> {
-  let (_, out) = feed(Ground, bytes)
-  out
-}
+test fn events(bytes: Bytes) -> List<Event> = feed(Ground, bytes).1
 
@@ -56,5 +46,3 @@
   var out = Bytes.empty()
-  while out.len() < count {
-    out.push(b)
-  }
+  for _ in 0..count { out.push(b) }
   out
@@ -65,6 +53,3 @@
   var out = Bytes.empty()
-  for line in reply.lines {
-    out.append(line.to_bytes())
-    out.append(b"\r\n")
-  }
+  for line in reply.lines { out += line.to_bytes() + b"\r\n" }
   out
@@ -76,13 +61,9 @@
   var out = Bytes.empty()
-  var i = 0
-  while i < bytes.len() {
-    let b = bytes.at(i)
-    i = i + 1
-    match b {
+  for i in 0..bytes.len() {
+    match bytes.at(i) {
       b'\n' => out.append(b"\r\n")
-      b'\r' => if i >= bytes.len() || bytes.at(i) != b'\n' {
-        out.append(b"\r\0")
-      }
+      b'\r' if i + 1 == bytes.len() || bytes.at(i + 1) != b'\n' => out.append(b"\r\0")
+      b'\r' => {}
       IAC => out.append([IAC, IAC])
-      _ => out.push(b)
+      other => out.push(other)
     }
@@ -95,5 +76,3 @@
   var end = line.len() - 1
-  while end > 0 && line.at(end) >= 128 && line.at(end) < 192 {
-    end = end - 1
-  }
+  while end > 0 && line.at(end) is 0x80..=0xBF { end -= 1 }
   line.slice(0, end)
@@ -108,17 +87,10 @@
   b'\r' | b'\n' => (Bytes.empty(), Enter(line))
-  BACKSPACE | DELETE => if line.is_empty() {
-    (line, Unchanged)
-  } else {
-    (erase_last(line), Show(b"\x08 \x08"))
-  }
+  BACKSPACE | DELETE if line.is_empty() => (line, Unchanged)
+  BACKSPACE | DELETE => (erase_last(line), Show(b"\x08 \x08"))
   CONTROL_C => (Bytes.empty(), Interrupt)
-  CONTROL_D => if line.is_empty() { (line, EndOfInput) } else { (line, Unchanged) }
+  CONTROL_D if line.is_empty() => (line, EndOfInput)
+  CONTROL_D => (line, Unchanged)
   ESCAPE => (line, Escape)
-  _ => if b < b' ' {
-    (line, Unchanged)
-  } else {
-    var longer = line
-    longer.push(b)
-    (longer, Show([b]))
-  }
+  0x00..=0x1F => (line, Unchanged)
+  _ => (line + [b], Show([b]))
 }
```

Behaviour is unchanged: `encode_input`'s CR test reads the next byte at `i + 1` because the
range counter is not advanced inside the body, and `0x00..=0x1F` is the old `b < b' '` test
after the named control keys, which stay earlier arms.

## 10 Open questions

| # | question | default if unanswered |
|---|---|---|
| Q1 | `is` as a reserved word (P1), or a `matches(e, P)` form, which takes a pattern argument and so is special syntax anyway? | reserved `is` (29 to 30 words) |
| Q2 | Visibility (P8): private by default with a contextual `pub`, all public as now, or `_name` privacy? It decides the std definition count (136, budget 120) and six glob collisions | all public; rename the six colliding names |
| Q3 | Match guards with effectful calls, as Rust allows, instead of E072? | pure guards |
| Q4 | For slice 10: does `bindings = { Terminal.ingress = … }` (design line 110) become a map literal, and is it the same form as P11's `["k": v]` or replaces it? | slice 10 decides `{ path = expr }` for maps keyed by references; P11 stays `:` for value keys |
| Q5 | For slice 10: named arguments (P10) for `nativeProcess(includes = …)`, or a record argument? | named arguments without defaults |
| Q6 | S0 as a `ignite fmt` rule, or a written convention only? | written convention until a formatter exists |

Q4, answered by the steering session on 2026-11-01:

- `Map<K, V>` (`design.md` §3.3) arrives in slice 10 with `get`, `put`, `remove`, `keys`,
  `len` and iteration in key order, on both targets.
- In slice 10 the brace literal `{ k = e, … }` takes its type from the expected type: a record
  type, or a `Map` with `Text` keys for identifier keys and with reference keys for component
  references.
- P11's `["k": v]` stays unscheduled.
