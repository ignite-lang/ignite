# Diagnosis: count rows 3.6, 1.1 llvm, 7.2, 7.3

Written by the coordinator from the agent's final message (the agent could not write this file).
Detail files here: `a-experiment.md` (3.6), `c-ignored.md` (7.2), `d-split.md` (7.3). Patches:
`a-experiment.patch` and `a-experiment-tier1/2/3.patch`, `b-experiment.patch`, `c-typer.patch`,
`d-http-split.diff`. Logs: `b-exp/`, `d-http-verify.txt`, `exp-tier1-ignite-io.txt`. V = verified
by the command or file named, I = inference.

## Reach

- 3.6 rust 12 -> 25 now; firm removals reach 15. llvm 82 -> 95 now; reach 70 (production code
  only) or 44 (with the test modules under `src`).
- 1.1 llvm 0 -> 4 now; reach 0 (patch tried: attack2 16 passed on LLVM).
- 7.2 24 -> 27 now; reach 25 without a machine change.
- 7.3 4 -> 5 now; reach 4 with one std split, 0 with four pure moves.

## A. 3.6

Counter (V): `quality.rs:1145-1160` counts a line whose code before the first `//` contains
`unsafe {`, `unsafe fn`, `unsafe impl` or `unsafe extern`, or ends with `unsafe`, in every `.rs`
under `crates/ignite-io/src` and `crates/ignite-rt/src`, test modules included.
`ignite-rt`: 70 at v0.15.0 and at HEAD, textually identical. `ignite-io`: 12 older, minus 3
removed since, plus 16 new = 25 (the decision text names 15; the 16th is `accept_socket`,
`net.rs:498`, commit d0c50164).

ignite-io 25 -> 15 (`a-experiment-tier1.patch`): reactor x3 (`Reactor::register`/`deregister`
take `BorrowedFd<'_>`); `net.rs` `Slot::release` through nix `shutdown`; test helper `fill`
through nix `send`; `accept_socket` through std `accept` (strace: one `accept4(.., SOCK_CLOEXEC)`,
no `getpeername`, before and after); `fs.rs` `entries` over a `Vec<u8>` with `get(..filled)`;
`task.rs` `safe fn malloc_trim`; three `sigaction`/`signal` sites merged into one `signal.rs`
`dispose(signal, Disposition)` (-2).
Stay: counting allocator 7 (`census.rs:259-277`), `sched.rs:522` yielder, `sched.rs:1143`
sigaltstack and mmap, `sched.rs:1175` `si_addr`, `fs.rs:203` getdents64, `task.rs:268` extern
block, `child.rs:735` `pre_exec`, `text.rs:21` memchr/memmem, one sigaction.
To reach 12 on rust, three more out of: drop the `realloc` override (-2, count stays exact,
in-place growth lost), naive search instead of `memchr`/`memmem` (-1; rustc 1.99.1 has no stable
safe byte-substring search), `getdents64` through nix `Dir` (-1; I: two to three more system
calls per listing). Each costs speed or system calls; not measured.
`mallinfo2` instead of `GlobalAlloc` (-7): glibc only and inexact; weakens the census.

ignite-rt production 70 -> 55 (`a-experiment-tier2.patch`): `arena.rs` `publish`/`bump` with
`wrapping_add` (-3), `with_task` through `with_state` (-1), one private `fresh<T>` for five
fresh-block sites (-4 net), `net.rs` `slot_at` over `OnceLock<Box<[Slot]>>` segments (-2),
`process.rs:417` and `test.rs:39` through `crate::bytes` (-2), `safe fn atexit` (-1), SIGPIPE
through the signal helper (-1), one `Handler::jump` for both `longjmp` calls (-1).
ignite-rt test modules 55 -> 29 (`a-experiment-tier3.patch`): `process.rs` tests 7 -> 0,
`store.rs` and `channel.rs` tests 11 -> 1 through a `#[cfg(test)]` `glue.rs` with 3 sites,
`arena.rs` tests 14 -> 2.

Tests in the copy (V): io unit 112 passed 1 ignored, io main 192, rt unit 44, rt main 29, 0
failed, in every state; runner-feature build and clippy `-D warnings` exit 0. Not run:
`ignite-llvm` and `ignite` tests. One red run of
`net::tests::the_exit_flush_sends_everything_to_a_peer_that_reads_late` in 14 at load 50 to 150,
not reproduced. Weaker in the tried variant: `Readiness::drop` is only a `debug_assert!` (land it
with `Readiness` owning its descriptor); std `accept` retries EINTR itself and a peer of another
address family is an error instead of `None`.

## B. 1.1 llvm

Red today (V, `b-exp/attack2.log`): 12 passed, 4 failed at `attack2.ignite:244`, `:323`, `:336`,
`:348`. The fixture was added after v0.15.0 (97b8c3c9); the leaks are older than the row.
Path: `Lower::builtin` arm `StoreGet | StoreRemove` (`crates/ignite-llvm/src/lower.rs:5327`)
calls glue `copy_from_heap.<T>` (`glue.rs:920-929`), which retains every handle in the copy; only
a block that registered the value (`Lower::own_value`, `lower.rs:570-611`) releases it at exit;
registration is decided syntactically in `own.rs` (`owned` :63, `owned_match` :105, `owned_iter`
:118). The Rust target drops temporaries at the end of the statement.
Fix (`b-experiment.patch`, +119 -2 in `own.rs` and `lower.rs`): an owner scope around a `match`
no statement claimed; `copied_keeps = aliases(arg) || keeps(arg)` for record-literal `put`
arguments; new `own::kept_temporaries` for produced arguments of a call whose result cannot alias
them; a scope around a bare statement whose expression produces a value holding handles.
Results (V): attack2 16 passed; variants 8 passed (before 4 and 4); `leak/attack.ignite` 6,
`seeded.ignite` 11; nine std modules and `examples/todo` same pass counts; `cargo test -p
ignite-llvm` 41 passed 1 ignored. Not run: rust target, `task gate`, clippy.
Same commit: `crates/ignite/tests/leak.rs:322-365` flips to 16 passed on both targets; four
`known_differences` removed; bar 1.1 llvm 0; `docs/slice.md` 30.3 and `docs/llvm.md:520-538`;
`CODEGEN_REVISION` 49 -> 50. I: add negative cases (a temporary returned, passed to `spawn`, in a
loop condition, under `?`, in a failing task).

## C. 7.2

27 at HEAD, 24 at v0.15.0; the difference is the three named tests.
C1 typer: `Typer::infer` (`crates/ignite-types/src/typer.rs:573-646`), a record literal with an
own type parameter nothing binds yet waits for the plain arguments after it; `c-typer.patch`
+45 -4; `cargo test -p ignite-types` 106 passed 5 ignored -> 111 passed 4 ignored. -> 26.
C2: stale `#[ignore]` at `conformance.rs:1484` (its cited fix 9bbd4b2 is an ancestor of HEAD;
assertions hold when replayed, the test itself not run). -> 25.
C3/C4: `resp::` and `nats::` already start their servers from PATH and panic when missing;
`command -v redis-server redis-sentinel nats-server` prints nothing; `valkey` is in the distro's
`extra` repository, `nats-server` in none. Needs a machine change.
A fake server behind an environment switch is a relabel; not recommended.

## D. 7.3

Now / v0.15.0: `ignite-rustgen/src/lib.rs` 7,484 / 7,157; `ignite-llvm/src/lower.rs` 6,073 /
5,752; `ignite-check/src/lib.rs` 4,854 / 4,726; `ignite-types/src/typer.rs` 4,644 / 4,559;
`std/protocol/http.ignite` 3,133 / 2,518. Next below: `parser.rs` 2,981, `nats.ignite` 2,856.
D1 (V, `d-http-verify.txt`): 35 clause-free helpers that need only `std.text` move to
`std/protocol/http/syntax.ignite` (510 lines); `http.ignite` 2,636; `ignite check` exits 0 on both
files and all 11 in-repo importers; conformance matrix byte-identical; LLVM tests 88 passed, same
names. Not run: rust target, `layering::`, gate. Also an AGENTS.md Layout row, the book page.
D2 to D5 (I, outlines): `typer.rs` -> 2,075 plus three modules; `ignite-check/src/lib.rs` ->
2,026 plus three; `lower.rs` -> 1,686 plus three (collides with B); `ignite-rustgen/src/lib.rs`
-> 2,261 plus seven.
