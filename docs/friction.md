# Friction log

Problems agents hit while writing Telos, kept as input for the next syntax or language slice.
Each row names who hit it and when. A row is removed when a slice fixes it, and the slice
names the row in its contract.

| # | friction | seen by | effect today |
|---|---|---|---|
| 1 | An interpolation cannot contain a string literal | slice 7 std/json, 2026-11-01 | `+` chains with `.to_text()` |
| 2 | No raw strings: JSON in source needs `{{` and `\"` everywhere | slice 7 std/json, slice 8 tests | hard-to-read fixtures |
| 3 | `List.empty()` needs a type on its binding | slice 7 std/text | extra annotations |
| 4 | `break` is not an expression; a match arm needs `{ break }` | slice 7 std/json | braces in arms |
| 5 | Builtin method names (`slice`, `get`, `at`, `join`) cannot be reused by UFCS helpers | slice 7 std/text, std/json | `substring`, `member` instead of the natural names |
| 6 | Every UFCS helper must be imported by name | slice 7, slice 8 tests | long import lists |
| 7 | Option has no `unwrap_or`, and `!=` on Option is missing | slice 7 todo | a local `or<T>` in `examples/todo/todo.telos` |
| 8 | Patterns have no ranges | slice 7 std/text | `digit_value` is a 10-arm match |
| 9 | No closures: a fn value cannot capture locals | slice 7 todo | PATCH does `get` then `put`, not the atomic `Store.update` |
| 10 | No tuple field access (`.0`) | slice 8 tests | a helper fn per case |
| 11 | A failed `assert a == b` on a list, option or record prints only `<Type>`, not the value | slice 8 tests | failures need manual narrowing |
| 12 | Text has no `\u{..}` escape | slice 8 tests | UTF-8 written as bytes |
| 13 | No lambdas for one-off `map` arguments | slice 8 tests | named fns or hand-written loops |
| 14 | A chain of 50,000 `+` terms still overflows the 256 MiB compile thread | review fix F2, 2026-11-01 | limit is about 10,000 terms in 2 s |
| 15 | A type may contain itself only through `List` (E055); recursion through an enum payload or `Option` needs boxing in both backends | review fix F2, 2026-11-01 | `Tree<T>` is written with `List<Tree<T>>` |
| 16 | Generic instances are emitted `internal` in every LLVM module that uses them | slice 7 T2 | duplicated code across modules |
| 17 | Records have no static functions, so `Response.json` cannot be written | slice 7 std/http | free fn `json(status, value)` |
