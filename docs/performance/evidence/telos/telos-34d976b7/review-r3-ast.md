# R3 review: AST, name and effect checker, type checker

Scope: `crates/telos-syntax/src/ast.rs`, `crates/telos-check/src/lib.rs`, `crates/telos-types/src/*.rs`.
Read-only. Line numbers are as read; `typer.rs` (2786 lines) and `lib.rs` (2667 lines) were edited by other agents during the review (last mtime 11:53:42), so re-check quotes before acting.

Counts: high 3, medium 0, low 2.

## 1. Effect roots drop the value of `if`, `match`, blocks, record literals and `List.at`; substrate methods have no second check

- Severity: high. Confidence: confirmed (traced in code; not run).
- Where: `crates/telos-check/src/lib.rs:1247` (`fn roots`), its catch-all arm at line 1281; the substrate branch at lib.rs:1603-1617 needs `roots(receiver)` for `read`, `write_all`, `close`, `accept`, `set_idle_timeout`. `at` is in `BUILTIN_METHODS` (lib.rs:147), so the UFCS arm is skipped for it.
  Typer side: `crates/telos-types/src/typer.rs:2142` calls `effect_site` only for `Ty::Store`. TcpStream and TcpListener methods have no E042 backstop.
- Quote: `_ => BTreeSet::new(),` (lib.rs:1281) and `let needed = match method_effect(&name.name) {` (lib.rs:1608).
- Scenario:
  ```
  record Conn { stream: TcpStream }
  fn peek(s: TcpStream) -> Result<Input, IoError> requires [stdin.read] {
      let c = Conn { stream: s }
      c.stream.read(4096)
  }
  ```
  `roots` of a record literal is empty, so `c` owns nothing and `read` needs no `s.read`. Expected: E016 "missing effect `s.read`". Actual: accepted, and the generated code reads the parameter's stream in a function that does not declare the read. The same holds for `let t = if flag { s } else { s }; t.read(4096)` and for `ls.at(0).read(4096)` on a `List<TcpStream>` parameter.
  Root cause is the contract: docs/slice.md section 5.1, "Roots", says "every other expression has no roots. Its value is owned by the function". The list omits record literals, `if`, `match`, blocks and `List.at`, which can return a parameter's value. The code follows the contract, so the contract needs the same fix.
  Fix direction: roots of an `if`/`match`/block/record literal and of list builtins are the union of their sub-expressions' roots.

## 2. A `requires []` callee passes its argument through, but its result is treated as owned

- Severity: high. Confidence: confirmed.
- Where: `crates/telos-syntax/src/ast.rs:207` (`FnDecl::is_effectful` is `self.requires.is_some()`), `crates/telos-check/src/lib.rs:814` (`effectful: decl.is_effectful()`), `crates/telos-check/src/lib.rs:1297` (`is_pure_call`, `.is_some_and(|f| !f.effectful)`).
- Quote: `self.requires.is_some()` (ast.rs:207).
- Scenario:
  ```
  fn first(s: TcpStream) -> TcpStream requires [] { s }
  fn peek(s: TcpStream) -> Result<Input, IoError> requires [stdin.read] {
      let t = first(s)
      t.read(4096)
  }
  ```
  `requires []` is `Some(vec![])`, so `first` is `effectful` in `Symbols`; the checker's own test helper builds effectful fns that way (`requires: effectful.then(Vec::new)`). `is_pure_call` is therefore false, `roots(first(s))` hits the catch-all (lib.rs:1281) and is empty, and `t.read` needs nothing. docs/slice.md section 5.1 gives a pure call the roots of its arguments, and a `requires []` fn declares no effect, so `s` should flow through. Expected: E016 for `s.read`. Actual: none.
  Fix direction: decide "pure call" from the callee's effect set (non-empty or inferred), not from `requires.is_some()`.

## 3. A bare payload variant that two enums share is accepted as a value

- Severity: high (wrong code on the LLVM target; a generated-crate build failure on the Rust target). Confidence: confirmed for acceptance and lowering (traced in code; not built or run).
- Where: `crates/telos-types/src/typer.rs:1434` (`if payload > 0 && !self.symbols.variants.contains_key(&name.name) {`), so the E003 is skipped for every name the checker sees as a variant. `crates/telos-check/src/lib.rs:1940` (`if let Some(owner) = self.symbols.enum_of_variant(&name.name).map(str::to_string) {`) returns None for an ambiguous name, so the checker does not report either.
  Backends: `crates/telos-llvm/src/lower.rs:1091-1093` (`Some(Callee::Variant { .. })` then `self.variant(&ty, &id.name, Vec::new())`), `crates/telos-rustgen/src/lib.rs:1092` (`typed_variant` for a bare name).
- Scenario:
  ```
  enum A { X(Int), Y }
  enum B { X(Int), Z }
  fn f() -> A { X }
  ```
  Expected: E003 "variant `X` takes 1 payload value; call it", as for an unambiguous `X`. Actual: no diagnostic. The typer's `name()` takes `expected_enum(A)`, the guard at 1434 is false because `symbols.variants` contains `X`, and `enum_variant` with `args = None` returns `Ty::Enum(A)`. The Rust backend emits `A::X` as a value, which the generated crate cannot compile. LLVM stores the tag and no payload (`variant()` at lower.rs:1114-1127 zips an empty argument list), so the payload is undefined.

## 4. Spurious E040 when one arithmetic or comparison operand has type `Never`

- Severity: low. Confidence: confirmed.
- Where: `crates/telos-types/src/typer.rs:2443-2444` (`let left_ty = match &right_ty {` / `Some(t) => self.check(left, t),`) and `typer.rs:2451` (`Some(t) => self.check(right, t),`). `compatible` (typer.rs:55, `if found == expected || *found == Ty::Never`) accepts a `Never` found, not a `Never` expected.
- Scenario: `let n = 1 + Process.exit(0)`. The right operand is `Never`, `check(1, Never)` gives `compatible(Int, Never) == false`, and the result is E040 "expected `never`, found `Int`". A `Never` left operand fails the same way at line 2451. The `And`/`Or` branch is not affected because it checks against `Bool`, which `Never` satisfies.

## 5. Exhaustiveness cost is exponential in the number of columns that mix literal and wildcard rows

- Severity: low. Confidence: suspected (the recursion is traced; cost not measured).
- Where: `crates/telos-types/src/exhaustive.rs:147` (`let only_catch_all = ...`; the guard only helps columns where every row is a catch-all), and the recursion over `split.cases()` in `uncovered` (exhaustive.rs ~170-200).
- Scenario: n `Bool` columns and 2n arms `(_ × (i-1), true, _ …)` and `(_ × (i-1), false, _ …)` for i = 1..n. The match is exhaustive. Each column split keeps literal rows in both branches, so T(n) = 2·T(n-1): about 2^n recursive calls, each cloning its rows; n = 20 is on the order of 10^6 calls. Not measured. Realistic matches are much narrower, so low. The witness vector is also uncapped; only `SHOWN = 8` are printed (exhaustive.rs:23, 215-222).

## Traced, no finding

- E016 fixed point: `update_inferred` (lib.rs:1131) is monotone (set union, `changed` only on growth), and `check_module` (lib.rs:365-390) keeps the diagnostics of the final walk. No non-termination found.
- Store methods and calls of EffectFn values are cross-checked by the typer's E042 against `effect_sites` (typer.rs:447-456, 1636, 2192).
- Imported effect signatures are copied (lib.rs:637-706), including protocol roles.
- E015 cycle chain reconstruction (`const_cycles`, lib.rs:1049-1097) has no panicking index.
- `Path::last` (ast.rs:51-55) cannot panic: the parser always pushes the first segment (parser.rs:468-475, 500-512, 625-636).
- Byte literal range is checked (E044, typer.rs:1403 and 2398).
- No temporary `Expr` is keyed by address in telos-types (grep for `Expr {` finds only the static `WILDCARD` pattern in exhaustive.rs).
- Tuple, list and or-pattern subpatterns are walked by the checker (lib.rs:2177-2290).
