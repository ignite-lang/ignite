# Slice 9: friction fixes. Decisions by the coordinator, 2026-11-01

Timo asked to "solve frictions" in `docs/friction.md` (17 rows). The coordinator took these
decisions. They are reversible. Agents record them in `docs/slice.md` section 13, the slice 9
contract, and change the matching row of `docs/friction.md` to point at section 13.

| row | decision |
|---|---|
| 1 | An interpolation `{ … }` may contain any expression, string literals included. The lexer tracks nesting. |
| 2 | Raw strings, Rust-style: `r"…"` and `r#"…"#`, with any number of `#`. No escapes, no interpolation. Raw byte strings `br"…"` work too. |
| 3 | `List.empty()`, `Option` `None` and other generic values with an unbound parameter take their type from the expected type, and otherwise from later uses in the same fn body (local unification). E042 remains only when neither decides. |
| 4 | `break` and `continue` are expressions of type Never, usable as match arms without braces. |
| 5 | A user fn may share its name with a builtin method. `x.f(..)` resolves to the builtin when the builtin applies to `x`'s type; otherwise it resolves to the user fn by UFCS. A user fn whose first parameter type equals a builtin receiver type, with the same name as that builtin, is E041 at the declaration. |
| 6 | Glob import `import std.text.*` brings every non-test public item of the module into scope, UFCS included. An explicit import wins over a glob. Two globs that both provide a used name are E041 at the use. |
| 7 | `==` and `!=` are structural on every type that contains no fn, handle, `IoError` or `Store`, everywhere, not only in `assert`. The assert glue from slice 8 becomes general equality glue. A new `std/option.telos` holds `unwrap_or`, `map`, `and_then`, `is_some`, `is_none`, `ok_or`, written in Telos with generics. A new `std/result.telos` holds `unwrap_or`, `map`, `map_err`, `is_ok`, `is_err`, `ok`. |
| 8 | Range patterns `a..=b` for Int and Byte literals and constants, e.g. `b'0'..=b'9' => …`. Exhaustiveness understands them. |
| 9, 13 | Closures `\|x, y\| expr` and `\|x\| { … }`, with optional parameter types `\|x: Int\| …`. They capture by copy (value semantics) and their effects are inferred. A closure that captures a handle carries that authority, and its fn type shows the effects on the captured roots. Fn values become closures in both backends: Rust uses `Arc<dyn Fn(..) -> .. + Send + Sync>`, or generics where cheaper; LLVM uses a `{fn ptr, env ptr}` pair with env copy glue. Named fns used as values keep working. `Store.update(id, \|t\| Todo { done = flag, ..t })` must work, so PATCH in `examples/todo/todo.telos` becomes atomic. |
| 10 | Tuple field access `t.0`, `t.1`. |
| 11 | A failed `assert a == b` prints both values for every comparable type, using generated debug text on both targets (records as `Todo { id: 1, title: "milk", done: false }`, lists `[1, 2]`, `Some(3)`). The same text is available as a builtin `x.debug()` returning Text. |
| 12 | `\u{XXXX}` escape in Text literals: 1 to 6 hex digits, a valid scalar value. |
| 14 | A left-deep chain of 100,000 `+` terms checks and builds on both targets without a stack overflow. Make the long-chain walks iterative in every pass, or prove the depth bound some other way. |
| 15 | Recursion through an enum payload, including `Option`, is allowed. Backends box the recursive payload: Rust `Box`, LLVM pointer with copy glue. E055 stays only for a record that contains itself through records and tuples alone, with no enum in between, because such a value would be infinite. Restore `Tree<T> { Leaf, Node(T, Tree<T>) }` as a valid fixture. |
| 16 | Generic instances are emitted once per program with `linkonce_odr` (or in one shared module), not `internal` in each module. Measure the size before and after on todod. |
| 17 | Associated functions: `fn Response.json(status: Int, body: Json) -> Response = …`, declared in the module that declares `Response`, called as `Response.json(..)`. Use it in `std/protocol/http.telos`. |

The slice-6 to slice-8 rules stay the same unless a row changes them.
