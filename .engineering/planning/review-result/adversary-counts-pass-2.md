---
format: aep.planning-md/3
id: review-result:adversary-counts-pass-2
kind: review-result
status: active
title: Adversary pass 2, unit counts (wave 11)
relations:
- reviews: story:ignored-tests-and-long-files-back
revision: 1
---
red

Adversary pass 2 (last) on unit counts, `wave/11-counts` at `ecdaa9fe` plus the test commit `fa4f0c84`.

verdict: red; cases executed 1220→1221, red 1; origin: introduced 4 (two warnings, two notes), pre-existing 0.

Findings:
1. `crates/ignite-check/src/lib.rs:2944`, CONFIRMED, introduced, warning. `builtin_method` answers yes when the checker does not know the receiver's type, so a user `fn put(c: Conn, other: TcpStream)` called as `c.put(b)` on a local bound from an `if` takes the `Map.put` row and gives `c` the roots of `b`: `probe.ignite:8:10: error[E016]: missing effect b.read for read`. Passes at `1f68ceb4`. Red case `ownership_adversary::a_user_fn_named_put_on_a_receiver_of_unknown_type_gives_it_no_roots`. A user `push` has the same false E016 at base. No program in the repository is reached.
2. `crates/ignite-types/src/typer.rs:685`, CONFIRMED, introduced, warning. Each round of the fixpoint loop calls `literal_waits` again on every field, nested literals included, so the cost doubles per nesting level: calls for `Pair { a = Pair { … }, b = 2 }` nested d deep: d=10 8,166; d=14 131,038; d=18 2,097,110 (about 2^(d+3)); 0 calls at `1f68ceb4`. Every generic record literal goes through `infer`. No test case (a clock or a hang would be needed).
3. `std/protocol/http/connection_tests.ignite:17`, CONFIRMED, introduced, note. The nine copied helpers (`connected`, `answer`, `read_to_end`, `served_with`, `reply`, `pause`, `chunked`, `every_byte`, `read_at_least`) match the originals byte for byte; no check fails when a copy drifts.
4. `quality/baseline.json:311`, CONFIRMED, introduced, note. Ten unit keys moved file; the two `eval_cannot_run` keys still listed were renamed correctly; the decision text at line 311 still says "the 10 new tests of std/protocol/http.ignite".

Held: pass-1 findings 1 to 4 closed, each fix reverted in a copy and a test went red (no fixpoint 1 red, nested literal sees nothing bound 4 red, `stores` without `builtin_method` 4 red, `put` out of `stores_argument` 2 red, v0.15.0 names against the `5f37aa6d` std 34 errors). The v0.15.0 export list generated from the tag: 91 names, identical to the test's constant, all importable. `http.ignite` at head is the first 2,870 lines of the `1f68ceb4` file; the 286 removed lines are verbatim in the new module; only `test` and `test fn` items left. The 10 moved tests run on both targets; in the evaluator 8 pass and 2 are listed, as at `1f68ceb4`; 452 inline tests at both. Row 7.2: v0.15.0 24, `afd32ca4` 27, `1f68ceb4` 26, head 23. Row 7.3 at head: 4. Differential: 890 units, same outcomes on every lane as `1f68ceb4`. The five `http_reply_adversary` tests pass in the evaluator at `1f68ceb4` and head. Generated LLVM IR and Rust output of all 643 tracked files identical between `1f68ceb4` and head. Release bytes equal for both trees: Rust `todod` 1,085,664, LLVM `todod` 1,535,256, Rust `terminald` 767,032, LLVM `terminald` 1,153,200. Suite: `cargo test -p ignite-types` 118 passed, 4 ignored; `cargo test --no-fail-fast -p ignite-check` 10 + 30 passed, 2 failed (the new case, layering); `task test-ignite` 452 per target; the long `main` run 149 passed, 1 failed (static metrics, improved lines only), 3 ignored; differential red on improved lines only; clippy and fmt exit 0.

```findings
[
{"file":"crates/ignite-check/src/lib.rs","line":2944,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"a user fn named put on a receiver whose type the checker does not know takes the Map.put row and gives its receiver the argument's roots, a false E016 on a program 1f68ceb4 accepts"},
{"file":"crates/ignite-types/src/typer.rs","line":685,"category":"property","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"the fixpoint loop re-enters literal_waits on every field each round, so typing a generic record literal nested d deep costs about 2^(d+3) calls (8,166 at d=10, 2,097,110 at d=18)"},
{"file":"std/protocol/http/connection_tests.ignite","line":17,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the nine helpers copied from http.ignite are byte-identical today and no check fails when a copy drifts"},
{"file":"quality/baseline.json","line":311,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the acceptance promised unchanged unit names, yet ten keys moved file, and the decision text still places the ten tests in std/protocol/http.ignite"}
]
```
