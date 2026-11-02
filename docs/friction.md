# Friction log

Problems agents hit while writing Ignite, kept as input for the next syntax or language slice.
Each row names who hit it and when. A row is removed when a slice fixes it, and the slice
names the row in its contract.

## Open

| # | friction | seen by | effect today |
|---|---|---|---|
| 14 | The Rust target cannot build a left-deep chain of 100,000 `+` terms in useful time | review fix F2, 2026-11-01; measured by slice 9 N2 and N3 | the frontend no longer overflows (solved in slice 9, `docs/slice.md` §13): 100,000 terms check in 1.9 s and emit in 1.1 s (debug CLI); the 10,000-term crate builds in 8.3 s; the 100,000-term rustc build ran over 600 s and was killed, so it is `#[ignore]` and not in the gate; the LLVM target builds and runs it in 16.1 s; LLVM emit of 100,000 terms takes 3.1 s alone and 6.7 s under two parallel gates, and the gate test `long_chains_lower_quickly` allows 20 s of wall time |
| 18 | `x.debug()` and the debug text of a failed assert exist only on types without type parameters | slice 9 N1, N2, 2026-11-01 | `.debug()` on a `T`, `List<T>` or `Option<T>` inside a generic fn is E041; a failed `assert a == b` there prints the type name (`<T>`) instead of the values |
| 19 | `for` binds a name, not a pattern | slice 9 N4 (std/http, std/json), 2026-11-01 | `for header in headers` then `header.0`, `header.1`, or a `let (name, value) = header` line |
| 20 | No match guards and no `name @ lo..=hi` binding | slice 9 N4 (std/json `unicode_escape`), 2026-11-01 | a range arm that also needs the value is a `b =>` arm with a nested `if`, or the range is matched twice |
| 21 | std.option and std.result export the same names (`unwrap_or`, `map`), and UFCS picks a user fn by name, not by receiver type | slice 9 N4, 2026-11-01 | `import std.option.*` and `import std.result.*` together make every shared name E041 at its use; a module imports the one it needs by name |
| 22 | `List` has no `filter` | slice 9 N4 (std/http `segments`), 2026-11-01 | a `var out = []` and a `for` loop around `if … { out.push(..) }` |
| 23 | An integer list literal is `List<Int>` and does not take `Byte` from a later use | slice 9 N4 (audit `range_exhaustive`), 2026-11-01 | E040 at the use; the binding needs `let edges: Bytes = […]` |
| 24 | Literal braces beside an interpolation in one string still need `{{` and `}}` | slice 9 N4 (std/json `encode`), 2026-11-01 | `"{{" + members… + "}}"` or a raw string for the braces and `+` for the values |
| 25 | `x.f(a)` where `f` is both a fn-typed field of `x` and a module fn in scope is E003 from `ignite-check` (the arity check counts the call as UFCS) before the typer's E041 | slice 10 P4 (probe: `Handler { respond: fn(Text) -> Text }` beside `fn respond`), 2026-11-02 | the fn and the field get different names (`std/console` reads `declared.implementation.respond` as a field and `examples/terminal/commands` names its fn `respond` only because no caller holds both) |
| 26 | `protocol` is a keyword, so a parameter cannot be named `protocol` (a field can) | slice 10 P4 (std/cli `endpoint`), 2026-11-02 | the parameter is `speaking: Protocol` |
| 27 | A generic kind's `main` restates the type parameter: `fn HttpServer.main<S>(self: HttpServer<S>)`, and `main_args` carries it to the generated main | slice 10 P4 (std/protocol/http `HttpServer`), 2026-11-02 | written as above; `docs/slice.md` 14.5 names the form |
| 28 | A record with a `Signal<T>` field cannot be built in a test, because no expression has type `Signal<T>` | slice 10 P4 (std/probe `TelnetPrompt.attempts`, std/contract `Availability.signal`), 2026-11-02 | the tests cover the helper fns (`satisfies`, `prompt_seen`, `meets`) and not the records |
| 29 | Protocol roles have a fixed arity (`serve(stream, application)`), so a server kind that hands its session limits to a session needs a module fn beside the role | slice 10 P4 (std/protocol/telnet `serve_with`), 2026-11-02 | `Telnet.serve` delegates to `serve_with(stream, application, SessionLimits {})` and `TelnetServer.main` spawns `serve_with` |
| 30 | On the LLVM target a `TcpStream` has one owner: a stream handed to `spawn` moves to the task only when the `spawn` is the last mention of a `let`-bound fresh value in its block; a parent that keeps using a stream it also spawned with leaves it open until the process exits | slice 10 P4 (`Telnet.connect` and `forward_input`), 2026-11-02 | `docs/llvm.md` section 4; the Rust target's `Arc` clones close on the last drop |

## Solved in slice 9 (`docs/slice.md` §13)

| # | friction | seen by | where the fix is used |
|---|---|---|---|
| 1 | An interpolation cannot contain a string literal | slice 7 std/json, 2026-11-01 | `"{items.map(encode).join(",")}"` in std/json `encode` |
| 2 | No raw strings: JSON in source needs `{{` and `\"` everywhere | slice 7 std/json, slice 8 tests | the tests of std/json, std/http and examples/todo; `br#"\""#` in std/json `quote` |
| 3 | `List.empty()` needs a type on its binding | slice 7 std/text | `var pieces = []`, `var items = []`, `var headers = []`, `let todos = Store.new()` |
| 4 | `break` is not an expression; a match arm needs `{ break }` | slice 7 std/json | `None => break` in std/json `parse_number` |
| 5 | Builtin method names (`slice`, `get`, `at`, `join`) cannot be reused by UFCS helpers | slice 7 std/text, std/json | std/text `slice` (was `substring`), std/json `get` (was `member`) |
| 6 | Every UFCS helper must be imported by name | slice 7, slice 8 tests | `import std.text.*` in std/http, `import std.json.*` and `import std.protocol.http.*` in examples/todo |
| 7 | Option has no `unwrap_or`, and `!=` on Option is missing | slice 7 todo | `std/option.ignite` and `std/result.ignite`; todo's `or<T>` is gone; `find(..) != None` in std/text `contains` |
| 8 | Patterns have no ranges | slice 7 std/text | `b'0'..=b'9'` in std/text, std/json, std/http; `0..=31` in std/protocol/telnet |
| 9 | No closures: a fn value cannot capture locals | slice 7 todo | `todos.update(id, \|todo\| Todo { …, ..todo })` in examples/todo `patch` |
| 10 | No tuple field access (`.0`) | slice 8 tests | std/json `get`, std/http headers, the todo tests |
| 11 | A failed `assert a == b` on a list, option or record prints only `<Type>`, not the value | slice 8 tests | every non-generic assert; the generic remainder is row 18 |
| 12 | Text has no `\u{..}` escape | slice 8 tests | `"\u{FFFD}"` in std/text, the std/json escape tests |
| 13 | No lambdas for one-off `map` arguments | slice 8 tests | std/text `to_lower`, `to_upper`, `lines`; std/console tests |
| 15 | A type may contain itself only through `List` (E055); recursion through an enum payload or `Option` needs boxing in both backends | review fix F2, 2026-11-01 | audit probe `io/p1_recursive` builds on both targets |
| 16 | Generic instances are emitted `internal` in every LLVM module that uses them | slice 7 T2 | `linkonce_odr`: todod grows from 5,915,280 to 5,919,584 bytes; 0 instances are shared between modules today |
| 17 | Records have no static functions, so `Response.json` cannot be written | slice 7 std/http | `Response.json` and `Response.plain` in std/http, used by examples/todo |
