---
format: aep.planning-md/3
id: story:shared-list-costs-back
kind: story
status: draft
title: A shared list costs nothing in a program that never shares one
relations:
- informed_by: story:rust-target-shared-list-storage
revision: 1
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
