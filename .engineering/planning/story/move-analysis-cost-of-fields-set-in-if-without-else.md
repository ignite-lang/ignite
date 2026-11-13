---
format: aep.planning-md/3
id: story:move-analysis-cost-of-fields-set-in-if-without-else
kind: story
status: active
title: The move analysis stays linear when a fn sets many fields in if statements without else
relations:
- informed_by: story:slice-16-moves-analysis
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-12T23:49:08Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-13"}
- {from: "proposed", to: "active", at: "2026-11-12T23:49:08Z", actor: "human:timo", revision: 4, executor: "agent:claude-coordinator", correlation: "accepted-2026-11-13"}
---
# Story: the move analysis stays linear when a fn sets many fields in `if` statements without `else`

## Outcome

`ignite check`, `ignite test` and a build of either target type a fn that sets N fields of one `var` record, each in its own `if` without `else`, with work and memory that grow with N as they do for the same fn written with `else` branches.

## Why

Reported on 2026-11-13 by the author of a command-line tool written in Ignite: `ignite check` on a fn of this form used the machine's memory, and a test run over the same source was ended by hand. Reproduced with a synthetic program at `0.16.2+g0119dcc3`:

```
type R { f0: Int, f1: Int, … }

fn fill(k: Int) -> R {
  var r = R { f0 = 0, f1 = 0, … }
  if k > 0 { r.f0 = 0 }
  if k > 1 { r.f1 = 1 }
  …
  return r
}
```

Instructions of `ignite check` (callgrind, debug build) for N = 6, 8, 10, 12, 14: 10,973,833; 29,778,966; 124,392,959; 591,377,114; 2,857,274,395. Two more fields cost a factor of 4.75 from N = 10 to 12 and 4.83 from N = 12 to 14. Peak resident memory: 19.9 MB at N = 12, 124 MB at N = 16, 1.19 GB at N = 20 when a 20 second limit ended the run. At N = 12, 98.78% of the instructions are under `ignite_types::moves::analyse`.

The same N fields set with an `else` branch each, in an `else if` chain, in `match` arms or in nested `if`s: 15 to 17 MB of peak memory up to N = 32.

Read from the code, not measured: the backward walk of `crates/ignite-types/src/moves.rs` keeps the live places as a set of entries `{ place, except }`. The assignment `r.fi = …` adds `fi` to the `except` list of the entry for `r`; the skipped branch of an `if` without `else` keeps the entry without it; `State::join` is the union of the two sets. Every such statement doubles the entries for `r`. An entry whose `except` list excludes less conflicts with every place the other one conflicts with, so the second is redundant for `Live::dead`.

The analysis is in every release since v0.10.0; no release made it worse.

## Acceptance

- A count test, red first: instructions of `ignite check` on the program above at N = 16 against N = 8 meet a ratio a polynomial of low degree meets and the doubling does not (the bound is stated in the test with both counts); the same for peak allocation in bytes.
- N = 64 type-checks under a memory limit of 256 MB.
- The marks the analysis records (moves, late moves, consumed parameters) are identical before and after for every program of the corpus, the fixtures and `examples/`: a test compares them, or every golden of both targets is unchanged.
- A property test on generated bodies (assignments to fields and sub-fields, `if` with and without `else`, `match`, loops, reads) holds the old and the new state equal under `Live::dead` for every place.
- The forms that are flat today (`else` branches, `else if`, `match` arms, nested `if`) keep their counts within the test's bound.
- `task gate` green; no timing row of the release is worse by the rule of `docs/quality.md`.

## Scope

- `crates/ignite-types/src/moves.rs` (`Live`, `Entry`, `State::join`, `Live::assign`, `Live::read`), a test under `crates/ignite/tests/`. Inferred.

## Not in this story

- Other analyses over the same body (ownership, effects): their cost on this form is measured by the count test's total and gets its own story if it shows.
