---
format: aep.planning-md/3
id: story:shared-list-costs-back
kind: story
status: active
title: A shared list costs nothing in a program that never shares one
relations:
- informed_by: story:rust-target-shared-list-storage
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-12T12:37:04Z", actor: "human:timo", revision: 3, executor: "agent:claude-coordinator", correlation: "after-v0.16.2"}
- {from: "proposed", to: "active", at: "2026-11-12T12:37:04Z", actor: "human:timo", revision: 4, executor: "agent:claude-coordinator", correlation: "after-v0.16.2"}
---
# Story: a shared list costs nothing in a program that never shares one

## Outcome

On the Rust target the size, build-time and per-list costs that the shared list storage (`ignite_io::List`) added to programs that never copy a list are back at their values before it: rows 3.9 and 6.2 of `docs/quality.md` for the Rust target, the instructions per reply of the terminal server, and the instructions of a table build.

## Why

The shared lists (unit `shared-lists`, merged 2026-11-12) remove the full copy of a list when a value that holds it is copied. Programs that never share a list pay for the second block and its release code. Measured on the dedicated host against `main` before the merge (run `w14-shared-3`, 5 passes; the size cells repeat in two earlier runs):

- 3.9 stripped size against plain Rust: terminal 1.507 → 1.509 (598,760 → 599,512 B), todo 0.839 → 0.840 (876,232 → 877,192 B).
- 6.2 todo dev build on the Rust target: 4104 → 4120 ms, every pass above every pass of the base; 4099 → 4120 ms in the run before, one pass overlapping.

Counted locally (the unit's commit `bf76448f`): the terminal server's echo 3,480 → 3,602 instructions per round (one more allocation and release per reply for the block of `Reply.lines`); a table build 6,414,688 → 7,443,117 instructions.

Where the bytes are, from the symbol sizes of the two builds of the terminal server (the same +752 B locally): `Arc<Vec<String>>::drop_slow` 409 B, the unwind glue of the block 321 B, and `terminal::commands::respond` +472 B from `Arc::new` repeated at 5 list literals; the drop glue of `Reply` (320 B) and 339 B of the telnet server's closure are gone. The todo server has three list element types and pays the release code three times.

Tried on 2026-11-12, not merged: building the block in one function that is not inlined (`List::from_vec` calls an `#[inline(never)]` helper) gives terminal +592 B and todo +160 B over the base, which reads 1.509 and 0.839. Writing the elements into an empty block through `Arc::get_mut` made both larger (+1,728 B and +1,904 B). What is left on the terminal server is the release code `Arc` generates per element type.

## Acceptance

- Row 3.9 on the dedicated host: `rust/terminal` at most 1.507 and `rust/todo` at most 0.839, with the copies the shared lists removed still removed (the count tests of `crates/ignite/tests/shared_lists.rs` unchanged).
- Row 6.2 `rust` on the dedicated host is not worse than the base of the run by the rule of `docs/quality.md`.
- The terminal server's echo and the table build are counted before and after; a cost that stays is written down in `docs/slice.md` section 6 with its reason.
- The substrate takes no new `unsafe` site, or the story names the site and why a safe form cannot give the size.
- `task gate` green.

## Scope

- `crates/ignite-io/src/list.rs` (the block, how it is built and released), `crates/ignite-rustgen/src/lib.rs` (list literals of a shared field: an empty literal as `List::new()`, one constructor call per literal). Inferred.

## The release run of v0.16.2 (2026-11-12)

The dedicated-host run of the release (BASE v0.16.1 against HEAD `20cfcbe6`, 5 ABBA passes, run `iq-20261112t092750z-a6f0`) lists four rows as worse than v0.16.1, all on the Rust target. v0.16.1 holds neither the shared lists nor the call moves; the run of the call moves alone (`w13-moves-3`) read 0.752 in both columns of the first row below, so the row is the shared lists'.

- 3.7 p99 of todo/patch-16: 0.745× → 0.778× of plain Rust, every pass above every pass of the base (base 726, 753, 745, 731, 746; head 770, 769, 778, 798, 804). The run of the unit (`26c9c7e2` against `d6f308f1`) read 0.756× → 0.776× with overlapping passes (base 756, 767, 783, 748, 754; head 776, 778, 791, 752, 773). The LLVM row is 0.586× in both columns. This row is not in the Outcome above and belongs to it: back at or below 0.752×.
- 6.2 todo dev build on the Rust target: 4103 → 4128 ms, every pass above every pass of the base.
- 3.9 stripped size against plain Rust: terminal 1.507× → 1.509×, todo 0.839× → 0.840×, as in the run of the unit.

The local release check lists row 3.2 on the Rust target: todod release 1,062,560 → 1,063,856 B and dev 7,586,840 → 7,616,952 B; terminald release 743,384 → 744,360 B and dev 6,344,576 → 6,349,688 B.

The release was cut with these rows tracked and not refused (the rule of 2026-11-11).
