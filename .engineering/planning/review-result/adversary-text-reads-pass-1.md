---
format: aep.planning-md/3
id: review-result:adversary-text-reads-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit textreads (wave 15)
relations:
- reviews: story:rust-target-text-reads-borrow
revision: 1
---
green

Adversary pass on unit `textreads` (`story:rust-target-text-reads-borrow`, first unit: forms 1 and 2, the buffer taken at its last use, std's MySQL text cell) at `fae2a9ce` (2026-11-13): CONFIRMED; 6 findings, all introduced, one warning and five notes; no case red against the head. Cases in `crates/ignite/tests/text_reads_adversary.rs` (7 programs with 33 inline tests on the evaluator and both targets) and `crates/ignite-rustgen/tests/text_reads_adversary.rs` (6 cases on the emitted text), commits `455a2279`, `7f5039ac`, `45305fda`, `f0818e53`.

Corrected in the unit:

- `docs/slice.md` section 6 gave "Ignite has no private fn" as the reason forms 3 and 4 need the callers of other modules. No module can import the program's own module (E031), and that module is typed in one pass, so every caller of one of its fns is known where the fn is decided. The text now says so; forms 3 and 4 stay with the story for the fns of the program's module (the story's section "After the first unit").

Confirmed and kept as tests:

- The unit's guard for bytes that are not UTF-8 went through a lent parameter and never reached the taken `String::from_utf8` path: a mutant with `from_utf8_unchecked` passed every program test of the unit. The adversary's `PARAMETERS` and `MYSQL_CELLS` programs fail on it.
- Four checks no program test of the unit observed (`lent_after` for the lent option payload, `held_overlaps` and `pinned` in `record_taken`, the nested `for` inside a closure): each mutant is killed only by the adversary's cases, which stay.

Confirmed and left:

- `taken_in` in `touched_in` (`crates/ignite-rustgen/src/lib.rs`) cannot matter: a shared binding is never a `Text` or `Bytes`. Its removal changes no test. The call stays; it costs one walk of an arm's body.

Attacked and not broken: about 40 forms of the lent bytes alive across a change or a move of the text, about 45 forms of the taken buffer with seven shapes of bytes that are not UTF-8, the `&[u8]` parameter through every kind of caller, crafted text and binary MySQL rows against the strings the decoder gave before the change, and the real MySQL 8 servers of `mysql::real_servers_with_both_plugins_on_both_targets` on the head. `ignite check --json` over the 668 `.ignite` files of the repository gives the same output at `de0312d1` and at the head; `ignite emit` differs in 48 files, and each of the 134 distinct changed lines is one of the story's forms or a version hash. Eleven mutants: ten killed, the eleventh equivalent (above).

Cost of the analysis, instructions under callgrind, debug builds, `de0312d1` against the head: `check` of `std/protocol/mysql.ignite` 1,258,184,681 to 1,260,557,067 (+0.189 %), `check` of the terminal design file 851,975,462 to 853,258,201 (+0.151 %), `emit` of the two 1,612,673,340 to 1,615,185,762 (+0.156 %) and 1,163,233,300 to 1,165,191,565 (+0.168 %).

Not covered: the real-server run on the base, the crafted packets on the base's LLVM target, a diff of the LLVM IR, release builds.

```findings
[
{"file":"docs/slice.md","line":844,"category":"judgement","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"The program module cannot be imported by any other module (E031) and is typed in one pass with all its callers, so the stated reason for leaving forms 3 and 4 undone does not hold for its fns. The text is corrected in the unit; forms 3 and 4 stay with the story for the fns of the program's module."},
{"file":"crates/ignite/tests/text_reads.rs","line":596,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The non-UTF-8 guard goes through a lent parameter, never the taken from_utf8 path, so removing the UTF-8 check (and the taken_in check of lent_after) passes every program test of the unit. The adversary's PARAMETERS and MYSQL_CELLS programs kill both."},
{"file":"crates/ignite-rustgen/src/lib.rs","line":3862,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"Dropping lent_after for the lent option payload survives all tests of the unit; the adversary cases a_stored_payload_is_not_lent and LIVENESS kill it."},
{"file":"crates/ignite-types/src/moves.rs","line":1551,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"Removing held_overlaps or pinned from record_taken survives all tests of the unit; the adversary cases LIVENESS, OPERANDS and PINNED kill them."},
{"file":"crates/ignite-types/src/moves.rs","line":475,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"Treating a for inside a closure as a slice read survives all tests of the unit; the adversary's ESCAPING program and its closure case on the emitted text kill it."},
{"file":"crates/ignite-rustgen/src/lib.rs","line":6207,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"taken_in in touched_in is dead code because shared bindings are never Bytes or Text; its removal is an equivalent mutant. Left in place."}
]
```
