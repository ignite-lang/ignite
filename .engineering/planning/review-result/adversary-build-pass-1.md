---
format: aep.planning-md/3
id: review-result:adversary-build-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit build (wave 11)
relations:
- reviews: story:build-time-back
revision: 1
---
red

Adversary pass 1 on unit build, attacked at `cddabcb6`; the pass's cases are commits `0db0ca8e` and `b5dbb007` (`crates/ignite/tests/dev_build_adversary.rs`).

verdict: red; cases executed 1172→1174, red 4 (two of the pass, `allocations::text_is_appended_in_place` of unit reply's `b4406420`, and the static metrics on improved lines only); origin: introduced 1, pre-existing 2.

Findings:
1. `crates/ignite/src/prebuild.rs:108`, red, introduced by `31c28342`, medium. A driver ended by SIGTERM while it generates leaves the cargo it started ahead and that cargo's rustc running; only `Drop` stops them. Test `dev_build_adversary::a_driver_ended_by_sigterm_leaves_no_build_ahead`: "2 processes of the build ahead still ran a second after the driver was gone". A shell probe saw the orphan run to its end and write 10 rlibs. Ctrl-C and `timeout` signal the whole group, so the reach is a driver signalled alone.
2. `.cargo/config.toml:2`, red, pre-existing behaviour, acceptance not met, low. `CFLAGS = "-g0"` is read only when cargo runs inside the repository; a TLS program built on the Rust target with the driver outside the repository keeps ring's C debug information: 27 third-party compile units and 512,400 more bytes (11,484,672 against 10,972,272). `cargo test` hands the variable to every test process, so the unit's tests could not see it. Test `dev_build_adversary::a_tls_program_built_outside_the_repository_holds_no_third_party_debug_information`.
3. `docs/quality.md:264`, open, pre-existing, medium. The four dev cells of row 3.2 at head are under the current bars and above v0.15.0: llvm todod 10,401,624 against 8,230,560 (+26 %), llvm terminald 9,673,184 against 7,563,720 (+28 %), rust todod 8,563,424 against 7,405,864 (+16 %), rust terminald 7,066,248 against 6,223,520 (+14 %). Left to unit size.

Held: the modelled row 6.2 path is under v0.15.0 on both targets (Rust, pipelined with the LLVM work spread, 14,823.3 M instructions against 15,535.2; pipelined 17,128.1 against 17,410.5; plain chain 20,393.7 against 22,477.6; LLVM 4,332.3 against 6,322.7). The sum over all processes on Rust is above v0.15.0: 24,948.8 against 23,818.9 (+4.7 %). Feature `process`: no program found that needs it and does not get it (rust, llvm, `--static`, todo, a raw-mode probe, `orphans`, `signal_mask` build and run); `::ignite_io::child::` in `PROCESS_ITEMS` matches nothing the generator emits, and no test names only `ChildProcess`. The skipped call is `rt_sigaction(SIGCHLD, SIG_DFL)` on the Rust target (9→8 in readiness mode, 7→6 in threads mode); a program started with SIGCHLD ignored keeps it ignored; only `child.rs` and `test.rs` start processes. Raw mode is byte-identical old against new for exit, panic and SIGTERM on both targets; `--static` uses TCGETS instead of TCGETS2 with the same flags. The repository has no licence or dependency gate; termios is in `Cargo.lock` and `docs/slice.md`. Build ahead: a type error leaves 0 processes in 24 rounds; two builds at once in one target directory both exit 0; a read-only target directory is a clean error, exit 3; a stale or invalid `ignite-prebuild/` builds; no network with an empty cargo home is one cargo error with 0 processes left. The backtrace and lld-or-fallback tests pass under both configurations; the dev `todod` is linked by lld and has a build id. `plan.json` of the terminal design is byte-identical. `CODEGEN_REVISION` is unchanged although the generated `Cargo.toml` changed; no test forces a bump. 7.2 is 23, 7.3 is 4, 2.6 is 6; the unit adds 0 `#[ignore`, 0 `unsafe`, 0 `#[allow`. LLVM todo dev link input 37,484,026 section bytes and 292,763 debug relocations (v0.15.0: 47.5 MB and 556,988). Not run: the mutation that removes the prebuild.

```findings
[
{"file":"crates/ignite/src/prebuild.rs","line":108,"category":"judgement","severity":"blocker","verdict":"CONFIRMED","origin":"introduced","message":"a driver ended by SIGTERM while it generates leaves the cargo started ahead and its rustc running; only Drop stops them (test dev_build_adversary::a_driver_ended_by_sigterm_leaves_no_build_ahead)"},
{"file":".cargo/config.toml","line":2,"category":"acceptance","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"CFLAGS=-g0 reaches only cargo runs inside the repository; a TLS program built on the Rust target elsewhere holds 27 compile units of ring's C code, 512,400 more bytes (test dev_build_adversary::a_tls_program_built_outside_the_repository_holds_no_third_party_debug_information)"},
{"file":"docs/quality.md","line":264,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"the four dev cells of row 3.2 at head are under the current bars and 14 to 28 % above v0.15.0 (llvm todod 10,401,624 against 8,230,560); left to unit size"}
]
```
