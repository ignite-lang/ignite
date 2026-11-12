---
format: aep.planning-md/3
id: approval-record:accepted-2026-11-13-move-analysis-cost
kind: approval-record
status: draft
title: 'One defect draft accepted: the move analysis doubling goes into wave 15'
relations:
- decides: story:move-analysis-cost-of-fields-set-in-if-without-else
revision: 1
---
# One defect draft accepted on 2026-11-13: the move analysis doubling goes into wave 15

Decided on 2026-11-13 through the operator's decision route, which Timo authorised on 2026-11-12 for low-risk and easily reverted questions. The question listed its options with their cost.

The question: when to fix `story:move-analysis-cost-of-fields-set-in-if-without-else`, a defect that is in every release since v0.10.0.

- A: accept it into wave 15, before v0.16.3. Cost: one implementor, one adversary pass, one `task gate`; v0.16.3 comes later by that unit.
- B: accept it and schedule it after the planned waves, the default for a finding that existed before the wave. Cost: users of v0.16.3 still hit it.
- C: leave it a draft.

Answer: A. Reason given with the answer: `ignite check` used a machine's memory on valid source, and the change is one file plus a count test.

What it accepts:

- `story:move-analysis-cost-of-fields-set-in-if-without-else`, as a unit of wave 15. It is the one exception in this wave to the rule that a finding older than the wave waits for the planned waves.

Held with the acceptance: the regression test is red on `main` first and holds a count or a bound, never a wall-clock time; the analysis accepts and refuses the same programs as before, and the unit says how that was checked; a fix that needs more than `crates/ignite-types/src/moves.rs` and its tests is put to the operator before it is built.

Not decided here: every other draft.
