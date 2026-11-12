---
format: aep.planning-md/3
id: story:rust-target-text-reads-borrow
kind: story
status: draft
title: A text or byte string that is only read is not copied on the Rust target
relations:
- informed_by: story:rust-target-shared-list-storage
revision: 1
---
# Story: a text or byte string that is only read is not copied on the Rust target

## Outcome

On the Rust target a `Text` or `Bytes` that a function only reads is read in place: `t.to_bytes()` whose result is only read, a payload of a lent `Option<Bytes>` that is only read, and a by-value list parameter whose every caller passes a list it never reads again.

## Why

Reported on 2026-11-12 by the first user of `std.protocol.mysql` (a read-only HTTP service backed by MySQL 8 over TLS, Rust target, release). After the shared lists (branch `wave/14-shared-lists`) 23 % of the allocations of its largest answer are copies of `Text`, and after the user's own source changes its row building still allocates 12,602 times per request. The user reduced its five functions to one synthetic program of 140 lines; `ignite emit` of that program at `3673bee8` (branch `wave/14-shared-lists`) gives the lines quoted here.

1. Bytes of a text that are only read. `let b = s.to_bytes()` followed by `for c in b` or `b.is_empty()` is emitted as `let b = s.clone().into_bytes();`: one allocation and a copy of the text for a read. With a constant: `let digits = TABLE().clone().into_bytes();`, two allocations (`story:rust-target-constants-built-once` removes one).
2. A payload of a lent option that is moved out and only read. `let b = match raw { Some(x) => x  None => return Nil }` on a read-only `Option<Bytes>` parameter is emitted as `Some(x) => x.clone(),`, and `b.to_text()` as `String::from_utf8_lossy(&b).into_owned()`: two allocations where the text is the only thing kept.
3. A by-value list whose caller is done with it. `fn dedupe(cols: List<(Text, Val)>)` pushes the elements of `cols` into its result; its only caller builds the list for this call and never reads it again. The parameter is emitted as `cols: &Vec<(String, Val)>`, the call as `dedupe(&columns)`, and the push as `out.push(c.clone());`: one copy of a text and a value per element.
4. A loop over a lent list of records that keeps a field. `for c in cells { columns.push((c.name, decode_cell(c.raw, c.kind))) }` is emitted as `columns.push((c.name.clone(), decode_cell(&c.raw, &c.kind)));`: the name is copied because `cells` is lent, and `cells` is lent although its caller passes an element it copied out for the call.
5. A text payload turned into bytes. `WTxt(t) => t.to_bytes()` in a function over a lent enum is emitted as `Wire::WTxt(t) => t.clone().into_bytes(),`: the same form as 1 inside an arm; here the copy is needed unless the caller gives the value up (as 3).

std has form 2: `std/protocol/mysql.ignite:883` `Text(bytes.to_text())` in `string_value` copies a cell that `take_bytes` (line 344) has just sliced out, one copy per text cell that a move of the buffer would save; in the first user's profile at `3673bee8` it is 2.6 % of the allocations of the largest answer.

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
