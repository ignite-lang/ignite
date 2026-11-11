---
format: aep.planning-md/3
id: review-result:adversary-size-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit size (wave 11)
relations:
- reviews: story:binary-size-back
revision: 1
---
red

Adversary pass 1 on unit size, attacked at `9e4cd746`; the pass's cases are commits `a8bca070` and `36e56c24` (`crates/ignite-io/tests/timer_adversary.rs`, `crates/ignite/tests/binary_size_adversary.rs`).

verdict: red; cases executed 463→467, red 3; origin: introduced 7, pre-existing 2.

Findings:
1. `crates/ignite-io/src/sched.rs:536` (returned as the next wake at `:1046`), NEEDS-CHANGE, introduced by `5433f7c6`, blocker. An ended task's timer entry stays in the heap until such entries are half of it, and the worker wakes for it. With `todod`, 10 keep-alive connections, 4 closed, then idle: 9 `epoll_wait` against 6 without the heap on both targets; idle instructions 47,322 against 45,636 (LLVM) and 49,611 against 47,043 (Rust). Over a whole open, request, close run the heap is lower in total at 10, 1,000 and 10,000 connections. Test `timer_adversary::a_worker_does_not_wake_for_the_timer_of_a_session_that_ended`.
2. `crates/ignite-rustgen/src/lib.rs:7375`, NEEDS-CHANGE, introduced by `e1ec9dee`, warning. A Rust dev backtrace names no file and line on `ignite_std` frames and on the `ignite_io::checked::*` instances inside them; the LLVM target keeps `std/time.ignite:29`; the failure report is unchanged. The story's acceptance says "still names file and line". Test `binary_size_adversary::a_rust_dev_backtrace_names_file_and_line_of_a_standard_library_frame`.
3. `crates/ignite/src/lib.rs:2129`, NEEDS-CHANGE, introduced by c2 (`78e53200`), warning. `target.'cfg(all())'.rustflags` makes cargo ignore the user's `build.rustflags` on every dynamic Rust build; the base driver passes them through in dev and release. Test `binary_size_adversary::a_rust_build_keeps_the_build_rustflags_of_the_cargo_configuration`.
4. `crates/ignite/src/lib.rs:2119`, NEEDS-CHANGE, introduced by c2, warning. On the Rust target packed relocations lower stripped bytes by 22,024 / 18,968 and raise the start: `todod` 438,956 and 438,986 instructions with c2, 438,342 and 438,372 without (+614, run to run ±30); `terminald` +151. On LLVM c2 saves about 2,900 at start.
5. `crates/ignite-rustgen/src/lib.rs:7337`, CONFIRMED, introduced by c4c (`76a183a2`), note. `opt-level = "s"` for nix and corosensei adds 576 / 560 stripped bytes at head instead of saving 512 / 672; it lowers the start by about 234 / 182 instructions, so the time rule keeps it; the "for size" wording and the test name are wrong.
6. `crates/ignite-io/src/reactor.rs:317`, CONFIRMED, introduced by `4f0c73a8`, note. The slab costs 82 instructions more per connection on LLVM readiness (35,506 against 35,424, identical in two runs) and 22 in threads mode; the start is 1,517 lower and totals are lower at every connection count; Rust within noise.
7. `crates/ignite/tests/binary_size.rs:46`, CONFIRMED, introduced, note. `a_release_failure_report_is_the_dev_one_on_both_targets` links the dev runner archive, not the release runtime archive of c6; by hand a release executable on that archive gives the dev report on both fixtures.
8. `docs/quality.md` row 3.9 Rust, INFEASIBLE, pre-existing, note. Stripped Rust `todod` 857,848 and `terminald` 584,408 against 755,976 and 518,992 at v0.15.0 (base of the unit: 896,136 and 618,832). The growth in `.text` is code of v0.16.0: `ignite_io` +77 KB (`sched` +41.8, `run` +26.5), `ignite_std` +37 KB, the program +33.7 KB. No saving found that costs no time; unmeasured candidates: the `OnceLock` init of `worker_limit` (8,188 B) and http `reason` (6,354 B).
9. `docs/quality.md`, CONFIRMED, pre-existing, note. Readiness-mode cost per connection and per session is above v0.15.0, and the unit's base already has it: LLVM instructions per connection 22,022 at v0.15.0, 36,055 at base, 35,506 at head; LLVM `terminald` session instructions 40,240 / 74,749 / 73,403; session system calls 25 / 63 / 63.

Held: `ld.lld` takes 254,984,474 instructions with c1 and the split, 269,970,653 without; dev `todod` 7,993,888 against 10,137,536 bytes with `.text` equal within 48 B. The failure report is identical with and without the split on both targets, both fixtures, moved, with build directories gone, and `--static`; a backtrace without `.dwo` files keeps file and line and loses one inlined frame. glibc floor 2.36 (`GLIBC_ABI_DT_RELR`) on both targets in dev and release; base and v0.15.0 need 2.34. Removing c2 (LLVM), c2 (Rust), c5, c8 or c4c each turns its named test red. c5: bytes and counts equal when stripped; c8: start 53,056 lower; c6: start 3,590 lower and no `gimli` symbol in the archive. No test present at base is missing at head (1,099 → 1,136); ignored 30 → 27. Head against base: every start, request, connection and session count equal or lower except Rust per connection +372 (noise ±326). Idle VmHWM below v0.15.0 and base on all four executables. LLVM release and all dev bytes below v0.15.0. Not run: `task dist`, the `build.rs` fallback without `rust-src`, row 6.2 on the Rust target, a libm-calling program.

```findings
[
{"file":"crates/ignite-io/src/sched.rs","line":536,"category":"judgement","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"an ended task's timer entry stays in the heap and the worker wakes for it: 9 epoll_wait against 6 without the heap after 4 of 10 keep-alive connections closed (test timer_adversary::a_worker_does_not_wake_for_the_timer_of_a_session_that_ended)"},
{"file":"crates/ignite-rustgen/src/lib.rs","line":7375,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"a Rust dev backtrace names no file and line on ignite_std frames and on the ignite_io::checked instances inside them (test binary_size_adversary::a_rust_dev_backtrace_names_file_and_line_of_a_standard_library_frame)"},
{"file":"crates/ignite/src/lib.rs","line":2129,"category":"judgement","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"target.'cfg(all())'.rustflags makes cargo ignore the user's build.rustflags on every dynamic Rust build (test binary_size_adversary::a_rust_build_keeps_the_build_rustflags_of_the_cargo_configuration)"},
{"file":"crates/ignite/src/lib.rs","line":2119,"category":"judgement","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"packed relocations on the Rust target lower stripped bytes by 22,024 / 18,968 and raise the start of todod by 614 instructions (438,956 against 438,342) and of terminald by 151"},
{"file":"crates/ignite-rustgen/src/lib.rs","line":7337,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"opt-level s for nix and corosensei adds 576 / 560 stripped bytes at head and lowers the start by about 234 / 182 instructions; the for-size wording and the test name are wrong"},
{"file":"crates/ignite-io/src/reactor.rs","line":317,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the epoll slab costs 82 instructions more per connection on LLVM readiness (35,506 against 35,424) and 22 in threads mode; the start is 1,517 lower"},
{"file":"crates/ignite/tests/binary_size.rs","line":46,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"a_release_failure_report_is_the_dev_one_on_both_targets links the dev runner archive, not the release runtime archive"},
{"file":"docs/quality.md","line":271,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"pre-existing","message":"stripped Rust todod 857,848 and terminald 584,408 against 755,976 and 518,992 at v0.15.0; the growth is code of v0.16.0 (ignite_io +77 KB, ignite_std +37 KB, the program +33.7 KB) and no saving was found that costs no time"},
{"file":"docs/quality.md","line":265,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"readiness-mode cost per connection and per session is above v0.15.0 and the unit's base already has it: LLVM instructions per connection 22,022 at v0.15.0, 36,055 at base, 35,506 at head; terminald session 40,240 / 74,749 / 73,403; session system calls 25 / 63 / 63"}
]
```
