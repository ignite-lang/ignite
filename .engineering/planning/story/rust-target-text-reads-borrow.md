---
format: aep.planning-md/3
id: story:rust-target-text-reads-borrow
kind: story
status: active
title: A text or byte string that is only read is not copied on the Rust target
relations:
- informed_by: story:rust-target-shared-list-storage
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-12T12:37:04Z", actor: "human:timo", revision: 2, executor: "agent:claude-coordinator", correlation: "after-v0.16.2"}
- {from: "proposed", to: "active", at: "2026-11-12T12:37:04Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "after-v0.16.2"}
---
# Story: a text or byte string that is only read is not copied on the Rust target

## Outcome

On the Rust target a `Text` or `Bytes` that a function only reads is read in place: `t.to_bytes()` whose result is only read, a payload of a lent `Option<Bytes>` that is only read, and a by-value list parameter whose every caller passes a list it never reads again.

## Why

Reported on 2026-11-12 by the first user of `std.protocol.mysql` (a read-only HTTP service backed by MySQL 8 over TLS, Rust target, release). After the shared lists (branch `wave/14-shared-lists`) copies of `Text` are the second largest group of allocations of its largest answer, and after the user's own source changes its row building is still the largest allocating site of a request. The user reduced its five functions to one synthetic program of 140 lines; `ignite emit` of that program at `3673bee8` (branch `wave/14-shared-lists`) gives the lines quoted here.

1. Bytes of a text that are only read. `let b = s.to_bytes()` followed by `for c in b` or `b.is_empty()` is emitted as `let b = s.clone().into_bytes();`: one allocation and a copy of the text for a read. With a constant: `let digits = TABLE().clone().into_bytes();`, two allocations (`story:rust-target-constants-built-once` removes one).
2. A payload of a lent option that is moved out and only read. `let b = match raw { Some(x) => x  None => return Nil }` on a read-only `Option<Bytes>` parameter is emitted as `Some(x) => x.clone(),`, and `b.to_text()` as `String::from_utf8_lossy(&b).into_owned()`: two allocations where the text is the only thing kept.
3. A by-value list whose caller is done with it. `fn dedupe(cols: List<(Text, Val)>)` pushes the elements of `cols` into its result; its only caller builds the list for this call and never reads it again. The parameter is emitted as `cols: &Vec<(String, Val)>`, the call as `dedupe(&columns)`, and the push as `out.push(c.clone());`: one copy of a text and a value per element.
4. A loop over a lent list of records that keeps a field. `for c in cells { columns.push((c.name, decode_cell(c.raw, c.kind))) }` is emitted as `columns.push((c.name.clone(), decode_cell(&c.raw, &c.kind)));`: the name is copied because `cells` is lent, and `cells` is lent although its caller passes an element it copied out for the call.
5. A text payload turned into bytes. `WTxt(t) => t.to_bytes()` in a function over a lent enum is emitted as `Wire::WTxt(t) => t.clone().into_bytes(),`: the same form as 1 inside an arm; here the copy is needed unless the caller gives the value up (as 3).

std has form 2: `std/protocol/mysql.ignite:883` `Text(bytes.to_text())` in `string_value` copies a cell that `take_bytes` (line 344) has just sliced out, one copy per text cell that a move of the buffer would save; in the first user's profile at `3673bee8` it is among the ten largest allocating sites of the largest answer.

Forms 1, 2 and 5 are a read of a text's bytes; forms 3 and 4 are the choice between lending a parameter and moving into it, which the generator makes from the function's body alone and not from what its callers do with the argument.

## Acceptance

- A count test with the reduced program's functions (`escape_into`, `all_digits`, `decode_cell`, `dedupe`, `cells_to_row`, `bytes_of`, `slots_of`), in the manner of `borrowed_reads::`: the allocations per call are counted at two sizes on the Rust target, red first. Form 1: 0 allocations for the read of the bytes. Form 2: at most one allocation (the text that is kept). Forms 3 and 4: the count per element is stated before and after; the acceptance is that an element is copied at most once between `slots_of` and the result of `cells_to_row`.
- `b.to_text()` on a `Bytes` the function owns, at its last use, moves the buffer into the text when it is valid UTF-8 (0 allocations; today one); a count test over `std.protocol.mysql`'s text cells shows one allocation per cell less.
- The evaluator and the LLVM target give the same values; the reduced program's own test passes on all three.
- A text changed while its bytes are read (`let b = s.to_bytes(); s = s + "x"; b.len()`) gives the evaluator's value: the read in place applies only where the text is not changed and not moved while the bytes binding lives.
- For forms 3 and 4 the rule is written down in `docs/slice.md` section 6: when a by-value parameter is moved into (every caller in the package passes a last use, or the function is private), and what a public function does. If the rule needs the callers of a public function, the story says so and stops at private functions.
- The LLVM target's count for the same functions is written down; a fix there is a separate story if it copies.
- `CODEGEN_REVISION` is bumped; `task gate` green; a dedicated-host run shows no row worse.

## Scope

- `crates/ignite-rustgen/src/lib.rs` (`to_bytes` and `to_text` receivers, the arm bindings of a lent option, the parameter modes), `crates/ignite-types/src/moves.rs` (last uses at call sites, if the parameter rule needs them), `crates/ignite-rustgen/tests/golden/`, `crates/ignite/tests/` (count test), `docs/slice.md` section 6. Inferred.

## Not in this story

- Sharing the storage of `Text` (a copy that increments a count): a design of its own, with the same trade as the shared lists (`story:rust-target-shared-list-storage`).
- `let v = l[i]` that is only read, `m.get(k)`, `m.keys()` (`story:rust-target-read-only-bindings-borrow`).
- Module constants (`story:rust-target-constants-built-once`).

## Order of work

Prepared on 2026-11-12 for wave 15 and not started; no code exists for it. The branch `wave/15-text-reads` holds no commit of the unit.

Order of work for the implementor:

1. The count test (Acceptance bullet 1), red first: the seven functions the story names (`escape_into`, `all_digits`, `decode_cell`, `dedupe`, `cells_to_row`, `bytes_of`, `slots_of`) as one synthetic program in the manner of `crates/ignite/tests/borrowed_reads.rs` (read it, `value_copies.rs` and `shared_lists.rs` first). Allocations per call at two sizes on the Rust target.
2. Forms 1, 2 and 5 (a read of a text's bytes): `t.to_bytes()` whose result is only read reads the text in place; a payload of a lent `Option<Bytes>` that is only read is read in place; `b.to_text()` on a `Bytes` the function owns, at its last use, moves the buffer into the text when it is valid UTF-8. The guard of bullet 4 (a text changed or moved while its bytes binding lives gives the evaluator's value) gets its own test before the change.
3. std's form 2 (bullet 2): the count test over the text cells of `std.protocol.mysql` (`string_value`, `take_bytes`) shows one allocation per cell less; extend the existing MySQL count test.
4. Forms 3 and 4 (a by-value parameter moved into): decide the rule from the code, write it into `docs/slice.md` section 6, implement it for private functions. If a public function needs its callers, say so in the docs and stop there (the story allows it).
5. The LLVM target (bullet 6): count the same functions there and write the numbers into the docs. No change to `crates/ignite-llvm`.
6. Goldens under `crates/ignite-rustgen/tests/golden/` regenerated.

Forms 1, 2 and 5 with std's form are worth a merge of their own; forms 3 and 4 can be a second unit.

The coordinator's: `CODEGEN_REVISION`, `task gate`, the dedicated-host run (bullet 7).

Constraints:

- Rust must accept every program the checker accepts. Every new borrowed form gets a test with the binding alive across a call that takes the text by value, across a `spawn`, across a closure capture and across a loop that changes the text. When in doubt, keep the copy.
- Row 3.1 counts `.clone()` in the Rust goldens with tolerance 0: count before and after (`quality::static_metrics_hold_the_bar`); where it falls, write the lower value into the bar with `IGNITE_BLESS=1`.
- Rows 3.2 and 3.9 (stripped size of `todod` and `terminald` on the Rust target): measure before and after by the method of `ignite-perf quality`; a rise is reported.
- Row 3.10 (`task path-cost`) now judges one GET of `examples/todo` on the Rust target at every merge.
- After the generator change every module of `std/` must still build on both targets.
- The meaning of `to_bytes`, `to_text` and a by-value parameter in the checker and the evaluator does not change.

## Second unit: where it stands

The second unit (forms 3, 4 and 5) is unfinished work on branch `wave/15-text-reads-2`, one commit `26d63103` on top of the first unit (`6f42174a`), pushed. It is not reviewed, not gated and not for a merge as it is.

What the commit holds:

- The rule "By-value parameters given by every caller" (`docs/slice.md` section 6 on the branch; `crates/ignite-types/src/moves.rs`, `Given`): in the program's own module a by-value parameter is declared by value when its fn is only ever called, every call passes a last use or a value built for the call, and the body uses the parameter as an owner. On the Rust target a `for` or `match` over such a parameter at its last use takes it whole (`moves_owned`, `takes_whole`; the driver types the program with `type_program`).
- Tests on the branch: `crates/ignite/tests/text_reads.rs` (the bars of the forms, a `wire` form, a guard program of 9 inline tests), `crates/ignite-rustgen/tests/text_reads.rs` (2 cases on the emitted text).

What was run: `cargo build -p ignite` exit 0. One run of `cargo test -p ignite --test main text_reads`: 11 passed, 1 failed; the failure was the expected number of inline tests of the guard program in the test itself (10 where it has 9), corrected in the commit and not run again. In that run a row of the reduced program read 2.063 allocations per element (6.063 after the first unit, the unit's bar 2), `decode_cell` 1.000 and 1.001, the `wire` form 1.000 and 1.000, `bytes_of` 1.000 and 1.001.

Not compiled: the tests of `ignite-rustgen` and `ignite-types`. Not run: fmt, clippy, the other lanes of `cargo test -p ignite --test main`, the goldens, row 3.1, the sizes and instruction counts of the head, `ignite check --json` and `ignite emit` over the files of the repository at base and head. `CODEGEN_REVISION` is not bumped for this change. `docs/slice.md` section 44.3 on the branch lacks its paragraph on sizes, instructions and goldens.

To resume, in a managed worktree of the branch rebased on `main`, in this order:

1. `cargo test -p ignite-rustgen --test main text_reads`, then `cargo test -p ignite --test main text_reads` (12 expected).
2. `cargo test -p ignite-types` (at the base: 10 and 119 passed, 4 ignored) and `cargo test -p ignite-rustgen` (at the base: 180 passed, 1 ignored); bless the goldens and read every changed hunk.
3. The lanes of `cargo test -p ignite --test main` that hold the moves: `borrowed_reads`, `borrowed_reads_adversary`, `call_moves_adversary`, `call_moves_adversary_pass2`, `moves_cost`, `moves_cost_adversary`, `slice16`, `typed_json`, `check_json`, `shared_lists`, `rust_memory`, `text_reads_adversary` (its programs unchanged).
4. Row 3.1 (`quality::static_metrics_hold_the_bar`, 155 at the base), sizes of the stripped release binaries, one GET of the todo server, instructions of `ignite check` and `ignite emit` on `std/protocol/mysql.ignite` and the terminal design file at base and head. At the base: check 1,260,411,904 and 853,190,126, emit 1,612,540,075 and 1,164,687,672 (callgrind, debug driver); stripped `todod` 878,848 B, `terminald` 600,192 B.
5. The marks: this unit changes `TypedProgram::moves`, so `ignite check --json` over every `.ignite` file of the repository at base and head, and the list of every file whose marks change, goes into the PR.
6. fmt and clippy per crate, the paragraph of section 44.3, a bump of `CODEGEN_REVISION`, one adversary pass, a green `task gate`.

The row of `crates/ignite-types/` in the layout table of `AGENTS.md` does not name the rule, `moves_owned` or `takes_whole`; the implementor's one-row change to that table is not in the commit, because a change to `AGENTS.md` is the operator's to accept. It goes up as its own PR when the unit is merged.

For the adversary pass of this unit, the points the first pass and the implementation leave open: a fn that is called and also used as a value; a caller that passes the same binding twice; a parameter given by every caller and then lent to a `spawn` or captured by a fn value in the body; recursion (the fixpoint over given and consumed); a call inside a loop whose argument is a last use only on the last round; a parameter that the body reads after a `for` took it whole; a program of two modules where the sibling module's fn has the same name.

## After the first unit

The first unit (released in v0.16.4, 2026-11-13) holds forms 1 and 2, the buffer taken at its last use and std's MySQL text cell. Its section in `docs/slice.md` is 44; the lowering is in section 6, "Text and bytes read in place".

Met, with the test that holds each:

- Form 1: `escape_into` 1.000 and 1.006 allocations per call to 0.000 and 0.001 (texts of 10 and 200 bytes), `all_digits` 2.005 and 2.000 to 0 (`text_reads::` in `crates/ignite/tests/text_reads.rs`).
- Form 2: `decode_cell` 2.000 and 2.006 to 1.000 and 1.001, the text that is kept.
- `b.to_text()` and `t.to_bytes()` on a buffer the fn owns, at its last use, take the buffer; bytes that are not UTF-8 give the evaluator's text.
- std: a text cell of a MySQL row costs 1.021 allocations per cell where it cost 2.021 (`text_reads::a_text_cell_of_a_mysql_row_takes_the_buffer_of_its_bytes`).
- The evaluator and the LLVM target give the same values; a text changed, moved, given to `spawn` or captured while its bytes are read gives the evaluator's value (nine guards of the unit, 7 programs of the adversary pass).
- The LLVM target's counts are written down in `docs/slice.md` section 44: under 0.002 `malloc` calls per call for every form.
- `CODEGEN_REVISION` is `ignite-codegen/0.1.0-55`.

Not met, the second unit of this story:

- Forms 3 and 4: a row of the reduced program costs 6.063 allocations per element (7.063 before) against the bar of at most one copy of an element between `slots_of` and the result of `cells_to_row`. Form 5 (`bytes_of`): 1.000 per call, unchanged.
- The first unit left them out with the reason that Ignite has no private fn. The adversary pass showed the reason does not hold for the program's own module: no module can import it (E031), it is typed in one pass, so every caller of one of its fns is known. The rule of the second unit: a by-value parameter of a fn of the program's module is moved into when every call of the fn in that module passes a last use; a fn that is used as a value, and a fn of `std` or of a sibling module, keeps the lent parameter. The rule goes into `docs/slice.md` section 6 with the unit.
- Whether the first user's five functions are in its program module or in a sibling module is not known; the reduced program has them in one module.
- The dedicated-host run of the last acceptance bullet: the run of v0.16.4 (`iq-20261113t134302z-0527`, v0.16.3 against `3f221dfb`, 5 passes) shows no row worse by the rule of `docs/quality.md`. It runs no program that reads MySQL rows.

What the adversary pass hands to the second unit: its programs `LIVENESS`, `PARAMETERS`, `OPERANDS`, `OWNED`, `PINNED`, `ESCAPING` in `crates/ignite/tests/text_reads_adversary.rs` must stay green and unchanged; a parameter rule changes the marks of `TypedProgram::moves`, which the first unit did not (`ignite check --json` over the 668 `.ignite` files of the repository gave the same output before and after it), so the second unit lists every file whose marks change.
