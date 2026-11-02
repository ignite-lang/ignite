# Friction log

Problems agents hit while writing Telos, kept as input for the next syntax or language slice.
Each row names who hit it and when. A row is removed when a slice fixes it, and the slice
names the row in its contract.

| # | friction | seen by | fixed by |
|---|---|---|---|
| 1 | An interpolation cannot contain a string literal | slice 7 std/json, 2026-11-01 | slice 9: `docs/slice.md` section 13.1 row 1 |
| 2 | No raw strings: JSON in source needs `{{` and `\"` everywhere | slice 7 std/json, slice 8 tests | slice 9: `docs/slice.md` section 13.1 row 2 |
| 3 | `List.empty()` needs a type on its binding | slice 7 std/text | slice 9: `docs/slice.md` section 13.1 row 3 (local unification) |
| 4 | `break` is not an expression; a match arm needs `{ break }` | slice 7 std/json | slice 9: `docs/slice.md` section 13.1 row 4 |
| 5 | Builtin method names (`slice`, `get`, `at`, `join`) cannot be reused by UFCS helpers | slice 7 std/text, std/json | slice 9: `docs/slice.md` section 13.1 row 5 |
| 6 | Every UFCS helper must be imported by name | slice 7, slice 8 tests | slice 9: `docs/slice.md` section 13.1 row 6 |
| 7 | Option has no `unwrap_or`, and `!=` on Option is missing | slice 7 todo | slice 9: `docs/slice.md` section 13.5 (`==`, `!=` on every comparable type); `unwrap_or` stays open |
| 8 | Patterns have no ranges | slice 7 std/text | slice 9: `docs/slice.md` section 13.1 row 8 |
| 9 | No closures: a fn value cannot capture locals | slice 7 todo | slice 9: `docs/slice.md` section 13.3 (closures) and 13.1 row 9 (`R { f = e, ..base }`) |
| 10 | No tuple field access (`.0`) | slice 8 tests | slice 9: `docs/slice.md` section 13.1 row 10 |
| 11 | A failed `assert a == b` on a list, option or record prints only `<Type>`, not the value | slice 8 tests | slice 9: `docs/slice.md` section 13.1 row 11 |
| 12 | Text has no `\u{..}` escape | slice 8 tests | slice 9: `docs/slice.md` section 13.1 row 12 |
| 13 | No lambdas for one-off `map` arguments | slice 8 tests | slice 9: `docs/slice.md` section 13.3 |
| 14 | A chain of 50,000 `+` terms still overflows the 256 MiB compile thread | review fix F2, 2026-11-01 | slice 9: `docs/slice.md` section 13.1 row 14 (frontend; backends iterative too) |
| 15 | A type may contain itself only through `List` (E055); recursion through an enum payload or `Option` needs boxing in both backends | review fix F2, 2026-11-01 | slice 9: `docs/slice.md` section 13.4 (boxing) |
| 16 | Generic instances are emitted `internal` in every LLVM module that uses them | slice 7 T2 | slice 9: `docs/slice.md` section 13.1 row 16 (LLVM target) |
| 17 | Records have no static functions, so `Response.json` cannot be written | slice 7 std/http | slice 9: `docs/slice.md` section 13.8 (associated fns) |
