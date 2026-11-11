---
format: aep.planning-md/3
id: review-result:adversary-build-pass-2
kind: review-result
status: active
title: Adversary pass 2, unit build (wave 11)
relations:
- reviews: story:build-time-back
revision: 1
---
red

Adversary pass 2 on unit build, attacked at `835919b6` (fix round 4, `cddabcb6..835919b6`); the pass's cases are commit `c3d1c586` (`crates/ignite/tests/dev_build_adversary.rs`, `crates/ignite-rustgen/tests/streams.rs`).

verdict: red; cases executed 1351→1356, red 5 (four of the pass, and the static metrics on improved lines only); origin: introduced 6, pre-existing 2.

Findings:
1. `crates/ignite/src/prebuild.rs:229`, NEEDS-CHANGE, introduced by `4f917b3a`, warning. `Watcher::start` blocks SIGTERM, SIGINT and SIGHUP before `command.spawn()` (`:131`), so the build ahead's cargo, rustc, cc, linker and build scripts inherit the mask (SigBlk 4003; 0 at `cddabcb6`). A build ahead left by a SIGKILLed driver no longer ends on the three signals; the doc comment at `:198` says the children get an empty mask. Test `dev_build_adversary::the_build_ahead_blocks_no_signal_and_ends_on_sigterm`.
2. `crates/ignite/src/main.rs:486` and `crates/ignite/src/prebuild.rs:284`, CONFIRMED, introduced by `4f917b3a`, warning. The driver unblocks the three signals unconditionally instead of restoring the mask it started with; a driver started with SIGTERM blocked is ended by a signal that stayed pending at `cddabcb6`, on every command. Test `dev_build_adversary::a_driver_started_with_an_ending_signal_blocked_keeps_it_blocked`.
3. `crates/ignite/src/lib.rs:2148`, CONFIRMED, introduced by `4f917b3a`, warning. `CFLAGS=-g0` set in cargo's environment hides a user's `[env] CFLAGS` of a cargo configuration file, which reached the C compiler at `cddabcb6`. Test `dev_build_adversary::c_flags_of_the_users_cargo_configuration_reach_the_c_compiler`.
4. `crates/ignite/src/prebuild.rs:290`, CONFIRMED, introduced by `4f917b3a`, note. Every driver command reads `/proc/self/status` twice and makes four or five `rt_sigprocmask` calls for the three signals without a build ahead: `ignite check` 2,678 → 2,698 system calls, `ignite plan` 2,146 → 2,169. Driver instructions are lower at head in the four commands measured. Test `dev_build_adversary::a_command_without_a_build_ahead_makes_no_system_call_for_its_signals`.
5. `crates/ignite-rustgen/src/lib.rs:7472`, CONFIRMED, introduced, note. No test failed with the `Launcher` entry removed from `PROCESS_ITEMS`; `streams::a_program_that_names_only_a_launcher_turns_on_the_substrate_feature` of this pass holds it now (green at head, red on the mutant).
6. `crates/ignite-rustgen/src/lib.rs:7474`, CONFIRMED, pre-existing, note. The generator never emits `::ignite_io::ChildExit` and emits `::ignite_io::Invocation` only beside a named `Launcher`; neither entry can change a feature decision.
7. `docs/quality.md:296`, CONFIRMED, pre-existing, warning. The modelled Rust dev build on one core is 24,831.7 M instructions against 23,815.6 M at v0.15.0 (+4.3 %); it is below v0.15.0 from two cores up (17,089.0 against 17,516.8 on two, 17,065.3 against 17,416.2 on four and eight). The round lowered the one-core sum (24,933.5 at `cddabcb6`).
8. `crates/ignite/src/prebuild.rs:276`, INFEASIBLE, introduced, note. A real ending signal the watcher dequeues around the moment `stopping` is set is swallowed and the driver runs on; read from the code, not demonstrated.

Held: exit statuses are equal at `cddabcb6` and head for group INT, TERM, HUP, driver-only TERM, TERM then INT, QUIT and PIPE during the build ahead, for starts with TERM ignored and with INT and HUP ignored, for group INT during the program build, and for driver TERM, HUP and group TERM, INT during `ignite run`. The program under `ignite run` and the `ignite test` LLVM test program have identical SigBlk, SigIgn and SigCgt at both commits. No watcher thread exists outside the build ahead. Descendants are found through `/proc/<pid>/task/<tid>/children` after SIGSTOP and killed by pid; nothing was left in any signalled-driver run. `CFLAGS`, an empty `CFLAGS=`, `HOST_CFLAGS` and both `CFLAGS_<target>` spellings pass through with no `-g0`. Dev TLS executable 10,964,808 bytes inside the repository and 10,964,816 outside, 0 third-party debug units in both (11,484,168 and 27 units outside at `cddabcb6`). `835919b6`: rustc unit counts equal (todo 20, terminal 19, tls 32), only host units gain `-C debug-assertions=off`, a probe binary prints `debug_assertions true overflow_panics true` at both commits, build-script outputs differ in 0 lines. Rust all-process sum 24,847.2 M, modelled path 14,771.7 M against 15,538.8 M at v0.15.0, LLVM path 4,318.6 M. Removing `ChildProcess` or `Pipe` from `PROCESS_ITEMS` is caught by existing tests. The round adds 0 `unsafe` and 0 `#[ignore`. Not probed: `ignite test --target rust`, `ignite apply`, PID 1, a compiler-cache server first started under the build ahead, a driver SIGKILLed between `stop_tree`'s SIGSTOP and SIGKILL, CFLAGS on the LLVM target.

```findings
[
{"file":"crates/ignite/src/prebuild.rs","line":229,"category":"judgement","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the build ahead's cargo, rustc, cc, linker and build scripts inherit SIGTERM, SIGINT and SIGHUP blocked (SigBlk 4003, 0 at cddabcb6), so a build ahead left by a SIGKILLed driver no longer ends on those signals (test dev_build_adversary::the_build_ahead_blocks_no_signal_and_ends_on_sigterm)"},
{"file":"crates/ignite/src/main.rs","line":486,"category":"judgement","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"a driver started with SIGTERM, SIGINT or SIGHUP blocked gets it unblocked (also prebuild.rs:284) and is ended by a signal that stayed pending at cddabcb6, on every command (test dev_build_adversary::a_driver_started_with_an_ending_signal_blocked_keeps_it_blocked)"},
{"file":"crates/ignite/src/lib.rs","line":2148,"category":"judgement","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"CFLAGS=-g0 set in cargo's environment hides a user's [env] CFLAGS of a cargo configuration file, which reached the C compiler at cddabcb6 (test dev_build_adversary::c_flags_of_the_users_cargo_configuration_reach_the_c_compiler)"},
{"file":"crates/ignite/src/prebuild.rs","line":290,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"every driver command reads /proc/self/status twice and makes four or five rt_sigprocmask calls without a build ahead: ignite check 2678 to 2698 system calls, ignite plan 2146 to 2169 (test dev_build_adversary::a_command_without_a_build_ahead_makes_no_system_call_for_its_signals)"},
{"file":"crates/ignite-rustgen/src/lib.rs","line":7472,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"no test failed with the Launcher entry removed from PROCESS_ITEMS until streams::a_program_that_names_only_a_launcher_turns_on_the_substrate_feature of this pass"},
{"file":"crates/ignite-rustgen/src/lib.rs","line":7474,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"the generator never emits ::ignite_io::ChildExit and emits ::ignite_io::Invocation only beside a named Launcher, so neither PROCESS_ITEMS entry can change a feature decision"},
{"file":"docs/quality.md","line":296,"category":"judgement","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"the modelled Rust dev build on one core is 24,831.7 M instructions against 23,815.6 M at v0.15.0 (+4.3 %), lower than v0.15.0 only from two cores up"},
{"file":"crates/ignite/src/prebuild.rs","line":276,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"a real ending signal the watcher dequeues around the moment stopping is set is swallowed and the driver runs on; read from the code, not demonstrated"}
]
```
