# Row A experiment: unsafe-site removals, compiled and tested in a copy

Copy of `wave/10-tls` at HEAD aa1defb2 (`git archive`), the w10-tls tree untouched.
All paths below are under `S=<scratch>/diag/counts/`.

## Result

| state | io | rt | rust (3.6) | llvm (3.6) | tests (io unit / io main / rt unit / rt main) | runner build | clippy |
|---|---|---|---|---|---|---|---|
| baseline | 25 | 70 | 25 | 95 | 112 (+1 ignored) / 192 / 44 / 29, 0 failed | 0 | 0 |
| tier 1 | 15 | 70 | 15 | 85 | 112 (+1 ignored) / 192 / 44 / 29, 0 failed | 0 | 0 |
| tier 2 | 15 | 55 | 15 | 70 | 112 (+1 ignored) / 192 / 44 / 29, 0 failed | 0 | 0 |
| tier 3 | 15 | 29 | 15 | 44 | 112 (+1 ignored) / 192 / 44 / 29, 0 failed | 0 | 0 |

Nothing was reverted. The test counts are the same in every state, so no test was deleted.
One tier 1 run had one red test that did not repeat (see "Tier 1, test runs").

## Commands

Every run is `CARGO_TARGET_DIR=$S/exp-target`, cwd `$S/exp`, in this order (`exp_tier.sh <label> [stage]`):

    cargo test --offline -p ignite-io -p ignite-rt
    cargo build --offline -p ignite-rt --features runner
    cargo clippy --offline -p ignite-io -p ignite-rt --all-targets --features ignite-rt/runner -- -D warnings
    sh $S/count_unsafe.sh $S/exp exp-<label>

The `runner` feature is in the build and clippy steps because `ignite-rt/src/test.rs` (one removal) only compiles with it, and because clippy without it fails on the baseline (below).
Logs: `exp-test-base.log`, `exp-test-base2.log`, `exp-test-tier1.log`, `exp-test-tier2.log`, `exp-test-tier3.log`.
Counter listings: `exp-base-*.txt`, `exp-tier1-*.txt`, `exp-tier2-*.txt`, `exp-tier3-*.txt`.

Counter output:

    exp-base  ignite-io 25   exp-base  ignite-rt 70
    exp-tier1 ignite-io 15   exp-tier1 ignite-rt 70
    exp-tier2 ignite-io 15   exp-tier2 ignite-rt 55
    exp-tier3 ignite-io 15   exp-tier3 ignite-rt 29

## Baseline (unmodified copy)

- `exp_test.sh base` (no `runner`): tests exit 0 with the numbers in the table; doc tests 0 of 0.
  Clippy exit 101: `error: function args is never used` at `crates/ignite-rt/src/process.rs:68`.
  This comes from selecting two packages without the `runner` feature, not from the tree.
- `exp_tier.sh base2` (with `runner`): tests as above, runner build 0, clippy 0. This is the reference for the tiers.
- No pre-existing test failure.

## Tier 1: ignite-io, 25 -> 15

| # | file: function | sites | state | note |
|---|---|---|---|---|
| 1a | `reactor.rs`: `EpollReactor::register` | 1 -> 0 | kept | trait takes `BorrowedFd<'_>`; `Entry` keeps the `RawFd` for the identity check only |
| 1b | `reactor.rs`: `EpollReactor::deregister` | 1 -> 0 | kept | `deregister(token, descriptor)`; another descriptor than the registered one is refused with `InvalidInput` after the entry is removed |
| 1c | `reactor.rs`: `PollReactor::wait` | 1 -> 0 | kept | both variants: see "Different from the list" |
| 2a | `net.rs`: `Slot::release` (`libc::shutdown`) | 1 -> 0 | kept | `nix::sys::socket::shutdown(fd, Shutdown::Read)` |
| 2b | `net.rs`: test helper `fill` | 1 -> 0 | kept | `nix::sys::socket::send`, same flags, breaks on any error as before |
| 2c | `net.rs`: `accept_socket` | 1 -> 0 | kept | `listener.accept()` of std |
| 3 | `fs.rs`: `entries` (`from_raw_parts`) | 2 -> 1 | kept | `Vec<u8>` of 16384 bytes (same size as 2048 `u64`), `buffer.get(..filled)`; the syscall site stays |
| 4 | `task.rs`: `trim_heap` | 2 -> 1 | kept | `safe fn malloc_trim` in the `unsafe extern` block, call without a block |
| 5 | `sched.rs`: `install_overflow_handler`, `on_fault`; `child.rs`: `default_sigchld` | 3 -> 1 | kept | new `signal.rs`: `dispose(signal, Disposition::{Default, Ignore, Overflow})` holds the one `sigaction` |

Sum: reactor 3 -> 0, net 3 -> 0, fs 2 -> 1, task 2 -> 1, sched 5 -> 3, child 2 -> 1, signal 0 -> 1, census 7 and text 1 unchanged: 15.

Callers changed: `sched.rs` (`wait_descriptor`, `Readiness::wait`, `Readiness::deregister(fd)`), `wait.rs` (`wait_descriptor(BorrowedFd<'_>, ..)`), `stdio.rs`, `tests/reactor.rs`, `tests/waiter.rs`.
`Readiness` has two owners, both in `net.rs`: `Stream` and the listener `Slot`. ignite-rt does not call the reactor.

### Tier 1, test runs

1. First run was void and is discarded (`exp-test-tier1-void.log`): `cp -a` kept the staged files' times, cargo rebuilt nothing and re-ran the baseline binaries. The script now copies with fresh times and logs `rebuilt_io=`/`rebuilt_rt=`.
2. Second run: the library compiled; the unit-test build failed on `MsgFlags` not imported in `fill`, and clippy on four unused imports (`wait.rs` `Event`, `sched.rs` `AsRawFd` and `RawFd`, `stdio.rs` `AsRawFd`). Fixed, no removal affected.
3. Third run (`exp-test-tier1-run1-flake.log`): io unit 111 passed, 1 failed: `net::tests::the_exit_flush_sends_everything_to_a_peer_that_reads_late` ("the peer did not receive `bye`"). Cargo stopped there.
4. `repeat_io_unit.sh tier1 12` (`repeat-tier1.txt`): the io unit tests 12 times, 12 green.
5. Final run (`exp-test-tier1.log`): all four binaries green, same sources and binaries as run 3 (binary newer than the sources, new error string present in it).

The red test: 1 failure in 14 runs of the tier 1 unit tests; the baseline was run twice, both green, which is too few to say whether the baseline flakes the same way. The test gives the exit flush 100 ms (`EXIT_FLUSH`) while a reader thread starts 20 ms later; load average was between 50 and 150 during the experiment. Its path differs from the baseline only in `fill`, which issues the same `send`. I kept the removal and did not revert; treat this as "not reproduced", not as "proven unrelated".

### Tier 1, accept check

`strace` is on PATH. `strace_accept.sh` traces `accept`, `accept4`, `getpeername` for the one test. The test is named `net::tests::an_accepted_socket_blocks_is_close_on_exec_and_has_its_peer` in the tree (not `..._has_its_mode_...`).

- baseline (`strace-accept-base.txt`): 1 `accept4(3, {AF_INET ...}, [128 => 16], SOCK_CLOEXEC) = 5`, 0 `accept`, 0 `getpeername`.
- tier 1 (`strace-accept-tier1.txt`): the same line, 0 `accept`, 0 `getpeername`. Test green.

## Tier 2: ignite-rt production code, 70 -> 55

| file: function | sites | state | note |
|---|---|---|---|
| `arena.rs`: `publish` (245, 246), `bump` (318) | 3 -> 0 | kept | `wrapping_add` |
| `arena.rs`: `with_task` | 1 -> 0 | kept | goes through `with_state` |
| `arena.rs`: new private `fresh<T>` | 0 -> 1 | kept | `pub(crate)` wrappers `fresh_bytes`, `fresh_lists` |
| `lib.rs`: `IgniteList::from_bytes`, `from_texts` | 2 -> 0 | kept | use the wrappers |
| `net.rs`: `read_into_arena` (644) | 1 -> 0 | kept | `fresh_bytes` |
| `text.rs`: `fresh` (72), `ignite_rt_text_pieces` (186) | 2 -> 0 | kept | `fresh_bytes`, `fresh_lists` |
| `net.rs`: `slot_at` (255, 267) | 2 -> 0 | kept | `[OnceLock<Box<[Slot]>>; SEGMENTS]`, `get_or_init`, `.get(offset)` |
| `process.rs`: `error` path (417) | 1 -> 0 | kept | `bytes(&error.message)` |
| `test.rs`: test names (39) | 1 -> 0 | kept | `crate::bytes(&entry.name)`; compiled only with `runner` (built, clippy 0) |
| `process.rs`: `atexit` call | 1 -> 0 | kept | `safe fn atexit` in the `unsafe extern` block |
| `process.rs`: SIGPIPE ignore (100) | 1 -> 0 | kept | `ignite_io::signal::ignore(Signal::SIGPIPE)` |
| `fail.rs`: two `longjmp` calls | 2 -> 1 | kept | private `Handler::jump` |

Sum: arena 24 -> 21, lib 7 -> 5, net 3 -> 0, text 3 -> 1, process 13 -> 10, test 2 -> 1, fail 3 -> 2: 55.

## Tier 3: ignite-rt test modules in `src/`, 55 -> 29

| file | sites | state | note |
|---|---|---|---|
| `process.rs` tests | 7 -> 0 | kept | `bytes(&x)` at all seven |
| `store.rs` tests | 8 -> 1 | kept | glue through `crate::glue`; the `from_raw_parts(list.ptr.cast::<i64>(), 2)` read stays |
| `channel.rs` tests | 3 -> 0 | kept | same glue |
| new `glue.rs` (`#[cfg(test)]`) | 0 -> 3 | kept | `read`, `write`, `unbox` (and a safe `boxed`) |
| `arena.rs` tests | 14 -> 2 | kept | helper `allocated(size, align) -> &mut [u8]` (1 site); `with_state` for the `base` read; `generated_code_and_runtime_share_the_bump_state` keeps its `&mut *state` |

Sum: process 10 -> 3, store 11 -> 4, channel 3 -> 0, glue 3, arena 21 -> 9: 29.

No site reads memory after a release on purpose: `release_frees_what_came_after_the_mark` reads a block allocated before the mark, `a_released_large_block_is_reused` writes into the block handed out again. Where a test compares a block's address across a release it keeps the raw pointer of `ignite_rt_alloc`; the slice helper is used only while the block is live.

The glue helpers are the weakest removal: they are safe functions that trust a raw pointer, as the `extern "C"` glue around them already did. It is a dedupe of ten identical sites into three, not a stronger argument.

## Different from the list

- `PollReactor`: both variants, not one. The thread path of `wait::wait_descriptor` uses a new `PollReactor::poll_one` (one borrow-checked `poll` of the eventfd and the descriptor, no registration, no dup, the same system calls as before). The `Reactor` impl of `PollReactor` owns a dup per registration (`Arc<OwnedFd>`), used only by the contract tests.
- `impl Drop for Readiness` no longer deregisters, since it cannot lend the descriptor: it is a `debug_assert!` that nothing is registered. `Stream::drop` and `Slot::release` deregister, and `Slot` got an `impl Drop` that does the same for a listener still present. The assertion fired in no test (debug builds). In a release build an owner that forgets leaves a reactor entry until the worker's reactor is dropped.
- `tests/reactor.rs`: the scripted double used descriptor number 1,000,000, which is no open descriptor. `BorrowedFd` needs a real one, so the harness now owns a `UnixStream` stand-in. No assertion changed, but the property "the double never hands the descriptor to the kernel" was implied by the impossible number before and is only true by the double's code now. This is the one place where a check got weaker.
- `accept_socket` through std: std retries `EINTR` itself (the caller's `EINTR` arm is now unreachable in practice; the listener is non-blocking), and a peer of another family than IPv4 or IPv6 is an error instead of `None`.
- `signal.rs` is a new `pub mod` of ignite-io: `dispose` and `Disposition` are crate-private, `restore_default` and `ignore` are public for ignite-rt. `sched::on_fault` became `pub(crate)`.
- Tier 2, `fresh` callers take the raw pointer after the last use of the slice (`text.rs`, `net.rs`), so the stored pointer is not derived before the writes.
- Tier 3: `glue.rs` is declared in `lib.rs` as `#[cfg(test)] mod glue;` after `mod contain;`.
- Not asked for: the `runner` build and clippy in every run, and a rerun of the baseline with them.

## Not verified

- Only the tests of ignite-io and ignite-rt ran, as directed. The runtime entries changed in tier 2 (arena, the handle table in `net.rs`, `fail.rs`, `process.rs`) are mostly exercised by generated code in the tests of `ignite-llvm` and `ignite`, which did not run. Tier 2 needs the workspace gate before it is trusted.
- No timing runs. Unmeasured hot-path changes: `slot_at` (a `OnceLock` read, a `Box` indirection and a bounds check instead of an atomic load and an add), bounds checks in `fresh` callers, and `wrapping_add` in the bump allocator.
- Whether the baseline also produces the three core files the io integration tests leave in `crates/ignite-io` (signal 6, from the three "stack overflow is reported and aborts" tests, which pass in every state). The baseline directory was replaced before I looked. I deleted those three files from the copy before making the patch.
- The copy is HEAD (aa1defb2) as archived. The w10-tls working tree has uncommitted changes in `AGENTS.md` (13 lines) and `Cargo.toml` (5 lines), dated 23:08 and 23:16 on 2026-11-10, not made by this experiment and not part of the copy.

## std probe

No. This toolchain's std (rustc 1.99.1) has no public, stable, safe byte-substring search to replace the `memchr`/`memmem` call in `ignite-io/src/text.rs`:
`<[u8]>::find` does not exist (E0599); `std::bstr::ByteStr` is unstable (feature `bstr`); `core::slice::memchr::memchr` is unstable (`slice_internals`). Only `iter().position` and `windows(n).position` compile, which are the naive searches. Probe files: `probe/`.

## Files

- `a-experiment.patch`: all three tiers, `diff -ruN` of the pristine crates against the tested ones (22 files, `patch -p1` at the repository root). Checked: applied to a pristine copy it gives the tested tree.
- `a-experiment-tier1.patch`, `a-experiment-tier2.patch`, `a-experiment-tier3.patch`: the tiers one after another, same `-p1`.
- Removed: `exp`, `exp-orig`, `exp-target`. Left in place: `stage`, `stage2`, `stage3` (my staging copies of the two crates, the sources of the tier patches); they can go with `rm -rf $S/stage $S/stage2 $S/stage3`.
