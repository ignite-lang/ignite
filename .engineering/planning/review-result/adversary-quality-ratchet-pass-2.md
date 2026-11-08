---
format: aep.planning-md/3
id: review-result:adversary-quality-ratchet-pass-2
kind: review-result
status: active
title: 'Adversary pass 2: quality-ratchet'
relations:
- reviews: story:quality-ratchet
revision: 1
---
unit: story:quality-ratchet, `feat/quality-ratchet` @ 4029b559 plus 5 uncommitted adversary2 cases (worktree `/home/timo/.local/state/worktree/trees/codewandler/ignite-lang/quality`)
verdict: NEEDS-CHANGE
cases: executed 17→22, red 5 (`quality::` and `differential::`, skipping `corpus_agrees`; the "before" run deselected `adversary2_`)
origin: introduced 5 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 (scratch `unit-quality/adv2/`: logs and scripts; `target/` removed)
needs-coordinator: none

## Cases (uncommitted, all red)

- `quality::adversary2_a_new_regression_swapped_in_for_an_intermittent_entry_is_refused`
- `quality::adversary2_main_decision_with_a_trailing_space_is_not_a_new_decision`
- `quality::adversary2_an_intermittent_entry_naming_an_implemented_story_is_refused`
- `differential::adversary2_a_seeded_lock_follows_the_workspace_lock`
- `differential::adversary2_a_test_the_executable_does_not_hold_is_not_a_run`

## Findings

1. F1 (blocker) `crates/ignite/tests/quality.rs:788-811`, `:840-848`: gate 1.7 counts intermittent entries without checking which units; swap main's entry for a new steady regression under any story and 1.7 stays 1, 1.1 skips it. Fix: refuse an intermittent unit main does not list unless it has its own decision.
2. F2 (warning) `quality.rs` `new_decision` (~:539): main's decision plus a trailing space, or a decision copied from another bar, passes. Fix: trimmed comparison, unique across bars.
3. F3 (warning) `quality.rs` `story_exists` (~:766): an intermittent entry may name an implemented story. Fix: refuse terminal statuses.
4. F4 (warning) `crates/ignite/tests/differential.rs:404-410` `seed_lock`: keeps a stale lock from an earlier run; after a dependency bump cargo goes to the index; no `CARGO_NET_OFFLINE`. Fix: always overwrite, set offline.
5. F5 (note, infeasible today) `differential.rs:906-952`: an exit-2 `no test named …` run counts as a run. Fix: treat it as `no_run`.

## Not broken

Group-k mapping (3+ files, order, siblings, std in group 0); odd test names; duplicate names refused by the checker; prefix names; A3, A4, A5 as designed; ETXTBSY retries; 5.3 manual-clock exact 100 ms.

## Coordinator disposition

Round 3 to the implementor with F1 to F5; verified by the coordinator; no third adversary pass.

```json
[
{"id":"F1","severity":"blocker","title":"intermittent entry swap hides a new steady regression","evidence":"quality.rs:788-811,840-848; swapped entry passes differential_lists+check vs main","test":"quality::adversary2_a_new_regression_swapped_in_for_an_intermittent_entry_is_refused","pre_existing":false},
{"id":"F2","severity":"warning","title":"main's decision plus whitespace counts as a new decision","evidence":"quality.rs new_decision ~:539; errors []","test":"quality::adversary2_main_decision_with_a_trailing_space_is_not_a_new_decision","pre_existing":false},
{"id":"F3","severity":"warning","title":"intermittent entry may name an implemented story","evidence":"quality.rs story_exists ~:766","test":"quality::adversary2_an_intermittent_entry_naming_an_implemented_story_is_refused","pre_existing":false},
{"id":"F4","severity":"warning","title":"seed_lock keeps a stale lock; offline claim breaks after a dependency bump","evidence":"differential.rs:404-410; 56 vs 8851 bytes","test":"differential::adversary2_a_seeded_lock_follows_the_workspace_lock","pre_existing":false},
{"id":"F5","severity":"note","title":"a 'no test named' exit-2 run counts as a run (INFEASIBLE today)","evidence":"differential.rs:906-952; 24 mispaired units have runs","test":"differential::adversary2_a_test_the_executable_does_not_hold_is_not_a_run","pre_existing":false}
]
```
