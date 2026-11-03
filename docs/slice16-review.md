# Slice 16 contract review

Status: review of the drafted contract, 2026-11-04; findings resolved (see "Resolution").

Independent review of `docs/slice.md` section 20 (slice 16, language gaps wave A: last-use moves,
calls of fn-typed expressions, rest patterns, E076) and its rows in section 8 (the E076 row of the
syntax table, the paragraph "Codes of slice 16"), as committed in `ec3160a` on
`feat/slice16-language-gaps`. 4 blockers, 5 should-fix, 14 notes. Code is cited by symbol at the
slice base `7ab3f7c`; section 20 is cited by subsection and acceptance row.

Reviewed against `docs/vision.md` §6 (X1 to X24, L1 to L4), invariants 1 to 9 (§3), §7 (T1 to
T4), §4 rows A and C, §4.1 and §5; `AGENTS.md`; `docs/actors.md` R1, §3.3, §5.1 and §9.2;
`docs/syntax.md` P9, R7 and §8.3; `docs/protocols.md` §5.2; `docs/friction.md`; and the code of the
base (`crates/ignite-{syntax,check,types,rustgen,llvm,eval}`, `crates/ignite`, `std/`). Claims
about today's behaviour were checked at run time with the `target/debug/ignite` binary of
2026-11-03 23:41 built at the base, in a scratch directory (probes r01 to r05 below: shadowing,
a handle stepped in a loop, an effectful fn field, the checker on `nope(1)(2)` and `assert` as an
arm, an assignment inside a later argument). Every other finding is read from source.

## Vision §6

| item | result | where |
|---|---|---|
| X1 backend primacy | respects: Rust stays the default; every runtime row runs both targets | 20.0 rows 8 to 12, 18, 21 |
| X2 interpreter | respects: the evaluator copies by construction and stays the reference | 20.3, 20.3.6, rows 20 and 23 |
| X3 IR layer | respects: no new IR; the analysis is a side table on `TypedProgram` | 20.6 |
| X4 priority order | not applicable | |
| X5 panics end a thread | not applicable | |
| X6 diagnostic codes | respects: E076 lies in T4's syntax range E070 to E079 with `syntax.md` §8.3's meaning | 20.7, section 8 |
| X7 `assert`, not `expect`/`fail` | respects: every fixture is `test` blocks with `assert` | 20.0 |
| X8 todo example | respects: `examples/todo/` and `todo.rs` unchanged | row 26, 20.8 item 1 |
| X9 substrate libraries | respects: no dependency added | 20.2 |
| X10 `await` | not applicable | |
| X11 declaration inventory | respects: `..` is an existing token, no keyword is added (invariant 6) | 20.5 |
| X12 `Text` | not applicable | |
| X13 design §11.2 | not applicable | |
| X14 archive path | not applicable | |
| X15 title | not addressed: the title still says slices 1 to 15 (N14) | `docs/slice.md` title |
| X16 telnet bound | not applicable: no std change is planned | row 25 |
| X17 keyword count | respects: unchanged | 20.5 |
| X18 cite symbols | respects: section 20 cites files and symbols, no line numbers | 20 intro |
| X19 layout table | respects: row 27 names the `crates/ignite-types/` row | row 27 |
| X20 to X22 | not applicable | |
| X23 checker vs backend | claimed, not met: 20.3.4 requires rustc to accept every accepted program, but B1 and B2 describe accepted programs that rustc rejects after the change | 20.3.4 |
| X24 capability roots | not applicable | |
| L1, L2, L4 | not applicable | |
| L3 region soundness | partly: 20.3.5 extends the soundness argument to moved places, but B2 (b) and N6 are cases it does not cover | 20.3.5 |

## Findings

### Blockers

1. **B1. The consumed rule of 20.3.3 omits `Map.put` and `Map.remove`, and does not say that
   moving a field of a parameter consumes it.**
   - Evidence:
     - 20.3.3 defines "changes it in place" as "(the rule that makes it owned today: assignment
       to it or a field of it, `push`, `append` or `clear` through a place rooted at it)". The
       rule today is wider. LLVM: `region::mutates` (`crates/ignite-llvm/src/region.rs`) counts
       `ListPush`, `ListAppend`, `ListClear`, `MapPut`, `MapRemove` and the `Store*` builtins.
       Rust: `mutated_names` with `mutates_receiver` (`crates/ignite-rustgen/src/lib.rs`) counts
       `MUTATING_METHODS` (`push`, `append`, `clear`) plus `MapPut` and `MapRemove`.
     - An implementer who follows 20.3.3 word for word leaves a parameter that is only changed
       with `m.put(k, v)` unconsumed. 20.3.5 makes it borrowed on LLVM ("owned exactly when it is
       consumed"), so the callee writes into the caller's map; 20.3.4 makes it `&T` on Rust, so
       the generated `m.put(..)` on a `&Map` does not compile. Row 11 or 12 fails.
     - 20.3.1 lets a field chain `v.f.g` move. Whether `consume(p.items)` with `p` a parameter
       "moves" `p` in the sense of 20.3.3 is not said. If it does not, Rust keeps `p: &T`, and
       moving `p.items` out of a `&T` is rustc E0507; LLVM keeps `p` borrowed and hands a pointer
       into the caller's memory to an owned parameter, which the callee changes in place.
   - Required change:
     - Define "changes it in place" as the union the backends use today, by builtin: assignment
       to the parameter or a field of it, and `push`, `append`, `clear`, `put`, `remove` (List,
       Map) and the `Store` mutators through a place rooted at it. Name `region::mutates` and
       `mutates_receiver` as the two sets the analysis replaces, and say that `consumed` must be
       a superset of both.
     - Say that a move of any place rooted at the parameter (`p`, `p.f`, `p.f.g`) consumes the
       parameter.
     - Add to row 4: a fn whose only change is `m.put(k, v)` on its parameter consumes it; a fn
       that passes `p.items` to a consumed parameter at its last use consumes `p`.

2. **B2. Liveness by name and "reads" alone let a value move that is still used.**
   - Evidence:
     - (a) Shadowing. An inner `var t = …` may shadow an outer `t` (probe r01: `var t = s` then
       `if … { var t = State {…}; t = … }` then `t.count` passes on both targets and reads the
       outer `t`). 20.3.2 says "reassigned as a whole (`v = e`)" by name, and the backends'
       precedent is by name too (`region::mutates` matches names; its comment accepts "an extra
       copy" for a shadowing body, which is safe there and unsafe here). In
       `consume(t); if c { var t = a } else { var t = b }; t.count` a by-name analysis sees `t`
       reassigned on both paths before the read of `t.count`, moves `t`, and Rust gives E0382 on
       `t.count`; LLVM reads a slot the callee owns and may have changed.
     - (b) An assignment inside a later argument. Probe r05: `take(s, if true { s = State {…};
       0 } else { 0 })` with `take` changing its first parameter is accepted and returns the old
       `s.count` on both targets today. Under 20.3.2 `s` is dead after argument 0 (the next event
       on every path is a whole reassignment), and the "another argument reads `v`" exclusion
       does not apply, since an assignment is not a read. On LLVM 20.3.5 passes the slot of `s`
       to the owned parameter, the second argument then overwrites that slot before the call, and
       `take` sees the new value. Rust is unaffected (the argument is moved into a temporary
       first), so the two targets disagree.
   - Required change:
     - State that liveness is computed per resolved binding: a shadowing `let` or `var` starts a
       new variable and neither reads nor reassigns the outer one.
     - Widen the exclusion: no move at an argument when another argument of the same call, or its
       receiver, mentions `v` at all (a read, an in-place change, or an assignment to `v` or a
       place rooted at it).
     - Add both cases to row 2's list "no move".

3. **B3. The exhaustiveness rule of 20.5 is unsound: checking one representative length m+1 does
   not cover every longer length.**
   - Evidence:
     - 20.5 checks the lengths up to m (the longest fixed pattern) one by one and represents every
       longer length by m+1. Counterexample on `List<Bool>`: arms `[]`, `[_]`, `[_, _]`,
       `[_, true, ..]`, `[.., false, _]`. Here m = 2. At length 3, position 1 is both the second
       element (prefix of `[_, true, ..]`) and the second from last (suffix of
       `[.., false, _]`), so `true` or `false` covers every value. At length 4 the two positions
       differ, and `[_, false, true, _]` matches no arm. The rule accepts the match and the
       program fails at run time on both targets.
     - The correct split is the one rustc uses for slice patterns: with P the largest prefix and S
       the largest suffix among rest patterns, every length at or above P+S behaves the same;
       lengths below that must each be checked.
   - Required change:
     - Check every length L from 0 to max(m, P+S−1) one by one, and represent every longer
       length by L = max(m+1, P+S), with each rest pattern expanded to that length.
     - Add the counterexample to `e046_rest`, with its witness `[_, false, true, _]`, and a
       matching valid case (the same arms plus `[_, _, _, _, ..]`) to `valid_rest_patterns`.

4. **B4. Rows 1, 13 and 14 require observations that the rules of 20.3 do not produce.**
   - Evidence:
     - Row 1 says the marked list is "exactly the `s` in `s = step(s, i)` and the `state` in
       `var next = state`". 20.3.1 makes "the result expression of a fn body" a move position,
       and in p07's `step` the result `next` is a tracked `var` of type `State` that is dead
       there. The rules mark it too, so the test as written fails.
     - Row 13 requires `fn step(state: State, x: i64) -> State` and the call `step(s, i)` with no
       `&`. Today every parameter that is not a role's is `&T` (`function_line`), including
       `Int` ones, and a Copy parameter is cloned on entry; 20.3.4 says a parameter that is not
       consumed "stays `&T`", and `x` is not tracked (20.3.1) and cannot be consumed. The rules
       produce `x: &i64` and `step(s, &i)`.
     - Row 14 requires that "the body of `run` calls no `copy.` function". `s` is reassigned in
       the loop, so it is a carried variable, and the loop's release block copies carried
       variables out of the region with `copy.<T>`. 20.3.5 keeps that rule ("Loop regions keep
       their rule"), and the existing golden test `loops_release_their_region` asserts
       `call void @"copy.Text"(` for the same shape.
   - Required change:
     - Row 1: add the `next` that is the result of `step`.
     - Row 13: either require `x: &i64` and `step(s, &i)`, or add to 20.3.4 a rule that passes
       scalar and fn-typed parameters by value, and list it among the golden changes row 24
       allows.
     - Row 14: require that the call of `step` in `run` is preceded by no `copy.` call and
       receives the slot of `s`, and allow the release block's `copy.State`; or say what replaces
       the carried copy (not proposed by 20.3.5).

### Should-fix

1. **S1. "No program makes more copies than before" (20.3.3, D3) is false.**
   - Evidence:
     - A parameter that is moved on some paths only becomes consumed, and then every call that
       does not move its argument pays a copy, also on the paths where the callee did not need
       one. `fn pick(a: T, b: T, c: Bool) -> T = if c { a } else { b }`: today one copy at the
       result (Rust `a.clone()` or `b.clone()`, LLVM a copy into the result); after the change
       both `a` and `b` are consumed, and a caller whose arguments are live copies both: two
       copies instead of one.
     - The same holds for std: `unwrap_or(value: Option<T>, fallback: T)` (`std/option.ignite`)
       returns `fallback` on the `None` path only, so `fallback` becomes consumed and every
       caller with a live fallback copies it, also when the option is `Some`.
     - Row 2's case "`if c { consume(s) } else { s.count }` as a fn body's result" uses `s` as a
       local; the reader cannot tell whether a parameter `s` would make that fn consume it.
   - Required change: make a parameter consumed by a move only when every path through the body
     moves it or changes it in place; otherwise keep the copy at the move point. Or drop the
     claim from 20.3.3 and D3, and measure copy counts in 20.12. Say in row 2 that `s` is a local
     there.

2. **S2. The shapes actors need still copy under 20.3.2.**
   - Evidence:
     - `a.state = a.handle(a.state, i)` (the handle loop, probe r02, passing today on both
       targets): the call's receiver path reads `a`, and `a` is never reassigned as a whole, only
       `a.state`; so `a.state` never moves.
     - `answer(state, reply, state.table.values())` (`docs/actors.md` §9.2): another argument
       reads `state`.
     - `Step { state = step.state, out = publish(step.out, …) }` (§5.1, §9.2): `step.out` is read
       after `step.state`, so `step.state` does not move.
     - R1 is the reason for the slice (20 intro: "last-use moves (actors R1)"), and 20.8 item 5
       rewrites R1 as if it were solved.
   - Required change: either make liveness field-disjoint (a move of `v.f` is killed by a
     reassignment of `v.f`, and a read of `v.g` with g ≠ f does not keep `v.f` alive), or record
     the limitation in 20.13 and in the U6 edit of actors R1, with the form that does move
     (`var st = a.state; st = a.handle(st, i); a.state = st`). Add the §5.1 shape to row 9's
     fixture either way, as a move or as a documented copy.

3. **S3. The harnesses of rows 9, 10, 12 and 18 are not defined.**
   - Evidence:
     - `ignite-llvm`'s golden and run tests type a module standalone: `lower_with` calls
       `type_module` with `&Default::default()` modules, so no std; `run_test_build` links a
       custom main and runs one test symbol. The `ignite-rustgen` tests build through
       `common::lower_tests(file, std_root, substrate)` plus cargo. The fixtures of the rows live
       under `crates/ignite/tests/fixtures/slice16/`, outside both crates.
     - Row 9 says "runs its tests … separately" and times them, but not with which runner, whether
       the time is the test's or the executable's, or how a single test is selected.
   - Required change: state for each row whether the fixture imports std (and if so how the LLVM
     test loads it), which helper builds it, and how one test is run and timed (for example the
     test executable with an exact filter, wall time of the process). If the fixtures must not
     import std, say so in 20.0.

4. **S4. D4's premise is wrong: the checker already types `hs[0]` and `h.run`, so 20.4 makes
   `(h.send)(s)` E042 while `h.send(s)` passes.**
   - Evidence:
     - `static_type` in `crates/ignite-check/src/lib.rs` types `Field`, `Index` of `List` and
       `Bytes`, `TupleField` and `Closure`; `fn_value_effects` reads a fn type's effects.
     - Probe r03: a record field `send: fn(s: PipeStream) -> IoResult<Unit> requires
       [s.write]` called as `h.send(s)`, and `let f = hs[0]; f(s)`, check and run on both targets
       today.
     - 20.4 (D4) reports E042 for any non-name callee with effects. The same fn reached as
       `(h.send)(s)` or `hs[0](s)` would then be rejected though its effects are known.
   - Required change: in the non-name arms of the checker's `call`, use `static_type(callee)` and
     `fn_value_effects` as the `Name` branch does, and record the effect site; report E042 only
     when `static_type` gives nothing. Or correct D4's premise and add the inconsistency to
     20.13.

5. **S5. Rust roles already take parameters by value; 20.3.4 does not say what changes for them.**
   - Evidence: `function_line` declares a role's parameters `mut name: T` by value (section 6,
     "not of a protocol role"), and argument emission (`args`, through `value`) passes a place
     as `.clone()`. 20.3.4 says "A parameter
     that is not consumed stays `&T`", which is wrong for roles, and does not say whether a
     moving argument to a role drops the `.clone()`.
   - Required change: state that role parameters stay by value whether consumed or not, and that
     a moving argument to a role is emitted bare; add a role call to row 13's golden.

### Notes

1. **N1.** 20.1 states that `PatternKind::List` is matched in `exhaustive.rs`. It is not:
   `exhaustive.rs` has no `List` arm, and list patterns fall to the catch-all rule ("add a `_` arm
   or a binding arm", `e046_patterns.expected`). Correct the fact; U5 adds the arm.
2. **N2.** The `ignite-syntax` fixture test is `fixtures_match_expected_diagnostics`, not
   `fixtures_match_expected` (U5's cell in 20.10). Row 16's command `fixtures::` matches either.
3. **N3.** `ignite-syntax` has no coverage test like `every_checker_code_has_a_fixture` and
   `every_type_code_has_a_fixture`, so nothing enforces `AGENTS.md`'s rule "Each diagnostic code
   … has a fixture" for E076. Add `every_syntax_code_has_a_fixture` to U5's scope, or name the
   fixture that covers it in row 16 as the check.
4. **N4.** Vision §5 lists `\u{..}` escapes and an interpolation that holds a string literal
   (friction 1 and 12) beside raw strings; 20.1 gives them "holds", but row 22's `holds.ignite`
   does not list a test for either. Add both.
5. **N5.** 20.5 does not say which witness E046 prints when a length is only partly covered
   (for example `[false]` on `List<Bool>` with `[true]` and `[_, _, ..]`). Say it prints the
   first uncovered value in the representative's order, with `_` for unconstrained positions,
   and put one such case in `e046_rest`.
6. **N6.** A moving `let (a, b) = pair` on LLVM: `lower.rs` makes a `LetPattern` initializer
   owned and binds into it. If the move hands over `pair`'s slot itself, a later
   `pair = other` overwrites what `a` and `b` point into. Pattern bindings are not tracked
   (20.3.1), so nothing stops the reassignment. 20.3.5 should say that a moving `LetPattern`
   initializer is `memcpy`'d into a fresh slot before binding.
7. **N7.** 20.6: say that `consumed` is false for `Callee::FieldFn` and `Callee::Const` (and for
   any callee that is not a declared fn, role or variant), as it is for `Callee::Local`.
8. **N8.** 20.3.1 excludes `Int`, `Byte`, `Bool`, `Unit`, quantities and handles but does not say
   whether a fn-typed variable or one whose type is a type parameter is tracked. Rust's `is_copy`
   in rustgen treats fn types as Copy today (the entry clone), but 13.3 lowers them to `Arc`.
   State both cases.
9. **N9.** 20.3.5 requires `moves.ignite` to grow a moved state past 64 KiB inside a loop, but the
   fixture is owned by U1 (20.10). List the requirement in U1's scope, or move the fixture's
   ownership.
10. **N10.** 20.8 item 4 edits section 6's `fn(A, B) -> R` row, but section 6's parameter
    paragraph also names "fn types" among the Copy parameters cloned on entry and shows the
    `as fn(i64) -> i64` wrapper. Edit those in the same item.
11. **N11.** Row 15 expects one revision step after W3 and one more after W4. That holds only if
    U3 and U4 both merge in W3 and share one step; say whether U4 takes its own step.
12. **N12.** 20.3.5 lists no backend-written call sites on LLVM (the `spawn` thunk, the fn-value
    thunk, the `store_update` thunk, the mains), as 20.3.4 does for Rust. They all pass fresh
    copies today, so an owned callee is safe; say so, so U4 does not have to rediscover it.
13. **N13.** Rows 9 and 10 time runs inside `cargo test --workspace` under gate load (20 cores
    shared, load 3 to 40 in earlier slices). The ratio form ("under 8 times") is robust; the
    absolute "under 1 s" is not. Say the ratio decides when the two disagree.
14. **N14.** `docs/slice.md`'s title still says "slices 1 to 15". 20.8 and row 27 do not retitle
    it, as earlier slices' stale points did (the X15 precedent). Add a 20.8 item for U6.

## Verified, no finding

- The facts of 20.1 on calls: the checker's `call` has the arms `Name`, path-headed `Field` and
  `_`, and the last two report E003 `not callable`; `Callee::Local` is recorded only in the
  typer's `Name` branch; the `Callee::Local` arms in `typer/effects.rs`, rustgen, LLVM and eval
  exist as cited; `effect_site`'s E042 message is as quoted.
- The probes 20.1 cites reproduce: p03 three E003 `not callable`; p04 E017; p05 two E042 with the
  quoted message; p06 E001 `expected pattern, found \`..\``; p09 the E046 list message; p07 passes
  on both targets and grows quadratically (Rust 2.16 s, LLVM 17.61 s total). Probe r04:
  `nope(1)(2)` gives E003 `unknown name \`nope\`` and `not callable` today, so row 6's "no `not
  callable`" is a real change; `Some(h) => assert h > 0` is E001, matching 20.8 item 9.
- The handle loop of row 8 (`Actor<S, M>` with `handle: fn(S, M) -> S`, stepped in a `while`)
  passes on both targets today (probe r02).
- `CODEGEN_REVISION` is `ignite-codegen/0.1.0-4` at the base; `DEFAULT_TIMEOUT` is 60 s.
- E076 is unused in the code and proposed in `docs/syntax.md` §8.3 with the meaning 20.7 gives;
  it lies in T4's syntax range.
- The cited tests exist: the slice 9 and slice 10 features, the std option, json and text tests,
  `failover::a_failing_session_does_not_end_the_service` (LLVM), `slice10::crashd_survives_failing_handlers`
  (Rust), `e042_closure`, `e017_closure`, `read_only_parameters_are_borrowed`,
  `loops_release_their_region`, `fixture_covers_every_family` with the `closure_` and `match_`
  families.
- `ModuleTypes` derives `Default`, and both struct literals of it (`query_snapshot` and
  `layout::module_types` in `ignite-llvm`) end in `..Default::default()`, so adding `consumed`
  is add-only (20.6).
  `query_snapshot` copies records, enums, defaults, generics and bounds and says snapshots are
  never typing inputs; `consumed` need not be in it.
- No existing fixture expects `not callable` except the constant case, so 20.7's E003 change
  breaks no `.expected` file beyond those 20.0 names.
- The debug `libignite_rt` that the `ignite-llvm` tests link poisons released memory with 0xDB
  (`poison` under `cfg!(debug_assertions)` in `arena.rs`), as row 12 relies on.
- Rust `Store::put(&self, id, value: T)` takes the value by value, so a bare move at `put`
  (20.3.1) fits the substrate.
- `crud_flow` (20.1, closures) runs on both targets as `todo::rust::crud_flow` and
  `todo::llvm::crud_flow` through `both_targets!`, so row 26's `todo::` filter covers it.
- Every new test file named in 20.0 is a module of its crate's `tests/main.rs` (invariant 9,
  `AGENTS.md`); no `[[test]]` is added.
- The owns cells of 20.10 do not overlap within a wave.
- Vision §4 row A: every item (closures, `unwrap_or` and `!=`, rest patterns, ranges, raw
  strings, tuple fields, `break`, boxing, `Map`, last-use moves, T1) is either delivered with a
  row on both targets or cited as holding with a test in 20.1; row C's dependency on "A (last-use
  moves), T1" is named. Static fns (§5) hold with the test "associated fns". T1 holds with one
  test per target (failover on LLVM, crashd on Rust).
- The vision §4.1 "15+" row and the epic's closure acceptance are stale exactly as 20.8 items 1
  and 2 say; `docs/friction.md` lists rows 1, 2, 4, 7, 8, 9, 10, 12, 13, 15 and 17 under "Solved
  in slice 9" and has no row for `assert` as an arm body (20.8 item 9).
- The `docs/actors.md` and `docs/syntax.md` quotes of 20.8 items 5 to 7 are verbatim.

## Resolution

Folded into `docs/slice.md` section 20 on `feat/slice16-language-gaps`, 2026-11-04. Section
numbers below are of that text. Every blocker and should-fix is folded; no finding was rejected.

| finding | resolution |
|---|---|
| B1, consumed rule | 20.3.3: in-place changes listed by builtin (`push`, `append`, `clear`, `put`, `remove`, assignments), named as the union of `region::mutates` and `mutates_receiver` with `mutated_names`, `consumed` a superset of both; a move of any place rooted at `p` consumes `p`. 20.1 gains the fact row. Row 4 adds `d(m)` with `m.put` and `e(p)` passing `p.items` |
| B2, shadowing and assignments in a later argument | 20.3.1: liveness on resolved bindings, a shadowing binding is a new variable. 20.3.2: the same-call exclusion covers any mention of an overlapping place, assignments included, with the LLVM slot-overwrite reason. Row 2 adds both cases |
| B3, exhaustiveness | 20.5: rustc's split, every length up to `max(m, P + S - 1)` checked, representative `max(m + 1, P + S)`. Row 17 adds the counterexample with witness `[_, false, true, _]` and its valid completion |
| B4, rows 1, 13, 14 | row 1 lists `next`; row 13 requires `x: &i64` and `step(s, &i)`, as 20.3.4 now says scalars keep `&T`; row 14 asserts no copy between the loop head and the call of `step` and allows the release block's `copy.State` |
| S1, copy count | 20.3.3 and D3: a move consumes a parameter only when it happens on every path to an exit; a parameter that is not consumed is copied at the move point as today. `pick` and `unwrap_or`'s `fallback` stay unconsumed (row 4); the copy-count argument is restated for both kinds. Row 2 says `s` is a local |
| S2, actor shapes | liveness per field path (20.3.2, D2): `a.state = a.handle(a.state, i)` and `Step { state = st.state, … }` move (row 3). `answer(state, reply, state.table.values())` still copies; the form that moves is in 20.3.2, 20.8 item 5 (U6's edit of actors R1) and 20.13 item 4 |
| S3, harnesses | row 9 names the rustgen helpers of `slice9.rs` with the repository's `std/`, the exact filters and the runner's `finished in` figure; rows 10 and 12 run through `ignite test --target llvm` in `crates/ignite/tests/slice16.rs`, owned by U4 in W3 (20.10, shared files); row 14 says the LLVM golden is self-contained; row 18 follows |
| S4, checker already types callees | 20.4.2 and D4: the non-name arms of `call` use `static_type` and `fn_value_effects`; E042 only where `static_type` gives no type (a call's result). Row 7: `hs[0](conn)` and `(h.send)(conn)` are valid, `pick_sender(true)(conn)` is the E042 case; 20.7 and section 8 reworded; 20.1 fact row added |
| S5, Rust roles by value | 20.3.4: role parameters stay `mut name: T`; a moving argument to a role is emitted bare. 20.1 fact row; row 13's golden has a role call |

Notes: N1 (20.1 fact on `exhaustive.rs`; U5 owns the new list arm), N2 (row 16 and U5's first
failing test name `fixtures_match_expected_diagnostics`), N3 (not folded as a new test: the E076
fixture of row 16 is the check, as for the other syntax codes), N4 (row 22 adds `\u{..}` and an
interpolation holding a string literal), N5 (20.5 witness order; row 17 adds `[false]`), N6 (a
pattern `let` initializer is no move position, 20.3.1, row 3), N7 (20.6), N8 (20.3.1: type
parameters tracked, fn types not; 20.13 item 4), N9 (U1's cell names the 64 KiB requirement,
20.3.5), N10 (20.3.4, 20.8 item 11), N11 (row 15: one step for U3 and U4 together), N12 (20.3.5
last item), N13 (rows 9 and 10 use the ratio alone), N14 (20.8 item 10, row 27, U6's cell).
