# Verified: R3 (AST, telos-check effect roots and E016, telos-types typer)

Verifier V3. Snapshot `snap-v3/` taken 12:03 (rsync of the live tree). It built without changes
(`cargo build -p telos`, then `-p telos-rt`), so `crates/telos-rustgen` was NOT replaced from `snap-v/`.
`diff -rq snap-v/crates snap-v3/crates` shows no difference in telos-syntax, telos-check or telos-types
(the line numbers of the review still hold); the differences are telos-io, telos-rt, telos-llvm goldens
and telos-rustgen (319 changed lines, none touching variant lowering). Binary
`target-v3/debug/telos`. Inputs in `snap-v3/verify-v3/`.

| # | finding | verdict | severity | evidence | fix direction |
|---|---|---|---|---|---|
| 1 | effect roots drop `if`, `match`, record literals, `List.at` | CONFIRMED (spec-conform: the contract is the defect) | high | `telos check verify-v3/f1_nocontrol.telos` exit 0, no output; `f1_roots.telos` reports only the control: `f1_roots.telos:6:10: error[E016]: missing effect `s.read` for `s.read`` and accepts `via_record`, `via_if`, `via_match`, `via_at`, `via_at_write`. Code: `_ => BTreeSet::new()` lib.rs:1281; typer calls `effect_site` only at typer.rs:1637, 2143, 2193 (Store and fn values), no backstop for TcpStream/TcpListener. "blocks" in the finding is moot: a block is not an expression (`let t = {` gives E001). | roots of `if`/`match` = union of arm results; record literal = union of field values; `List.at`/`l[i]`-style builtins = roots of receiver; change section 5.1 and the fixture together |
| 2 | `requires []` callee treated as effectful, result owned | CONFIRMED | high | `f2_requires_empty.telos`: `peek` (calls `fn first(s) -> TcpStream requires []`) accepted; control `first_inferred` (no `requires`) gives `18:10: error[E016]: missing effect `s.read` for `t.read``. UFCS form `s.first().read(4096)` also accepted (`f2_more2.telos`, exit 0). Code: ast.rs:207 `self.requires.is_some()`, lib.rs:1297 `!f.effectful`. | pure = empty declared or inferred set; keep `requires.is_some()` only for fn types (11.7) |
| 3 | bare payload variant shared by two enums accepted | CONFIRMED | high | `telos check verify-v3/f3_shared_variant.telos` exit 0; control with one enum (`f3_control.telos`) gives `6:10: error[E003]: variant `X` takes 1 payload value; call it`. Build rust: rustc `error[E0308]: mismatched types ... return A::X; expected `A`, found enum constructor`, `cargo build failed with status 101`, telos exit 2. Build llvm: `built verify-v3/out-llvm/bin/c`, exit 0; `program.ll` `@"demo.f"` stores `i32 0` into a fresh `[16 x i8]` alloca and memcpys all 16 bytes to `%sret`: payload bytes uninitialised. Guards: typer.rs:1434, `enum_of_variant` lib.rs:336-341 returns None for 2 owners. | typer: run the payload-arity E003 for any variant resolved by expected type, not only names absent from `symbols.variants` |
| 4 | spurious E040 when an arithmetic/comparison operand is `Never` | CONFIRMED | low | `f4_never.telos`: `4:11: error[E040]: type mismatch: expected `Never`, found `Int`` for `1 + Process.exit(0)` and `9:28` for `Process.exit(0) < 1`. typer.rs:2443-2453. | when one side is `Never`, synth the other side instead of checking against `Never` |
| 5 | exhaustiveness exponential in mixed literal/wildcard columns | CONFIRMED (measured) | low | generator `v3-genmatch.py` (n Bool columns, 2n arms, exhaustive); debug build, load avg ~120: n=10 0.06 s, 12 0.14 s, 14 0.48 s, 16 1.9 s, 18 7.4 s, n=20 38.9 s (exit 0). About x2 per column. Cause: no "first row all catch-all, covered" shortcut; exhaustive.rs:147 only skips columns of catch-alls only. | return covered early when the first row is all catch-alls; or cap the work and report the match as unchecked |
| NEW-1 | an effectful fn that returns its parameter also launders it | CONFIRMED (spec-conform, same contract gap as 1) | high (part of 1) | `f2_more2.telos`: `fn first_eff(s: TcpStream) -> TcpStream requires [stdin.read] { return s }`, then `let t = first_eff(s); t.read(4096)` in a `requires [stdin.read]` fn: exit 0. Section 5.1 gives roots only to calls of pure functions. | give every call result the roots of the arguments whose type can carry a capability (or of all arguments), not only pure calls |

## Spec reading (docs/slice.md)

- Section 5.1 opens: "Every operation an effectful function performs needs its effect. A function
  with `requires` must list it; a missing one is E016". Its Roots rule then says: "every other
  expression has no roots. Its value is owned by the function: a stream returned by `accept` or
  `TcpStream.connect` needs no effect for its own operations". The list of rooted expressions is
  parameters, locals, `e.f`, `e[i]`, `e.copy()`, `e?`, tuples, calls of pure functions and fn-typed
  locals/fields, pure UFCS. `if`, `match`, record literals, `l.at(i)` and effectful calls are absent.
  So findings 1 and NEW-1 are allowed by the letter of the spec and contradict its first sentence:
  the spec is internally inconsistent, and the code follows the narrower Roots list.
- Section 11.7 adds only Store, `set_idle_timeout`, fn-value and `x.*` rules ("`r` each root of the
  receiver"); it does not extend the Roots list, so it does not close the gap.
- Finding 2 is a deviation from the spec: section 5 says "A function that performs no effectful
  operation is **pure**", and 5.1 says "a call to a pure function ... has the roots of all its
  arguments". `first` performs none. 11.7's "`Some` makes a fn type effectful (`requires []`
  included)" is about fn types, not fn declarations.
- Finding 3 is a deviation: section 4 says "also E003 for such a variant with a payload used
  without arguments" and "The type checker reports these E005 and E003".
