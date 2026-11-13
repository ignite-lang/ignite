---
format: aep.planning-md/3
id: review-result:adversary-moves-cost-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit movescost (wave 15)
relations:
- reviews: story:move-analysis-cost-of-fields-set-in-if-without-else
revision: 1
---
red

Adversary pass on unit `movescost` (`story:move-analysis-cost-of-fields-set-in-if-without-else`) at `ab630ace` (2026-11-13): NEEDS-CHANGE; 3 findings, all introduced. Cases in `crates/ignite-types/src/moves.rs` (`mod adversary_tests`, 6 cases) and `crates/ignite/tests/moves_cost_adversary.rs` (1 case), commit `93faadaf`; 1 of the 7 red at `ab630ace`.

Fixed in `37ecc0b5`:

- The join of `ab630ace` dropped an entry that an entry of the other side covers, and compared every entry of one side with every entry of the other at a place. An `if`/`else` that sets a different field of one record in each branch keeps both entries, so the set still doubled per statement and each join cost the square of the set: `ignite check` of 10 such statements executed 2,591,447,568 instructions against 262,887,994 before the unit. The join now makes the entries of one place one entry; the same program executes 13,012,259 instructions, and 64 statements execute 221,572,356 where the build before the unit ran out of memory.
- N text variables each set in an `if` without `else` cost 39 % more instructions than before the unit (4,257,627,752 against 3,046,415,021 at N = 400). With the corrected join: 3,194,831,253, 4.9 % above the build before the unit.

Confirmed and kept:

- The covering test weakened to equality, or to a covering entry without exceptions, passed the unit's own property test and count tests. The adversary's four size guards fail on both; they stay in `mod adversary_tests`.

Not broken: 691 comparisons of `ignite check --json` and `ignite emit` over 229 files in 3 modes gave no difference between the build before the unit and `ab630ace`; 21 functions gave the same text on the Rust target, the LLVM target and the test build; six mutants of the join each fail a test.

After the correction the coordinator compared `ignite check --json` and `ignite emit` of all 659 `.ignite` files of the repository between the build before the unit and `37ecc0b5`: no difference. The unit's property test (4,000 bodies, 5,864,910 answers) and the adversary's two state tests hold the corrected join to the plain union of both sides.

```findings
[
{"file":"crates/ignite-types/src/moves.rs","line":1095,"category":"boundary","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"The first join compared every entry of one side with every entry of the other at a place, so an if/else that sets a different field per branch (exponential before the unit too) cost 2,591,447,568 instructions of ignite check at 10 statements against 262,887,994 at the base. Fixed in the correction pass: the entries of one place become one entry, and the form is no longer exponential."},
{"file":"crates/ignite-types/src/moves.rs","line":1018,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"The covering test weakened to equality, or to a covering entry without exceptions, passed the unit's property test and its count tests. The adversary's four size guards hold both."},
{"file":"crates/ignite-types/src/moves.rs","line":1087,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"N text variables each reassigned in an if without else cost 39 percent more instructions than at the base with the first join (4,257,627,752 against 3,046,415,021 at N = 400). The correction pass states the count of the same program with the join that keeps one entry per place."}
]
```
