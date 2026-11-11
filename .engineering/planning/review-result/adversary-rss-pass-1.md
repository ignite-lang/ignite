---
format: aep.planning-md/3
id: review-result:adversary-rss-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit rss (wave 11)
relations:
- reviews: story:rust-peak-memory-at-one-connection
revision: 1
---
red

Adversary pass 1 on unit rss (`story:rust-peak-memory-at-one-connection`), attacked at `22f8696b`; the pass's cases are commit `e1d49284` (`crates/ignite/tests/rust_memory_adversary.rs`, +291 lines). The adversary wrote no findings file; this is its final message, written down by the coordinator.

verdict: red; cases executed 163→168, red 3; origin: introduced 5, pre-existing 1.

Findings:
1. `crates/ignite/src/lib.rs:2099` (marker write after the arm at `:2092`), CONFIRMED, introduced, major. Any cargo failure that passes on the second run leaves `ignite-no-release-layout`, and every later release build into that target directory links without the layout and prints nothing. Red: `rust_memory_adversary::a_release_build_after_one_that_cargo_failed_once_still_links_with_the_layout` ("290 functions that are not the symbolizer's lie before or among its 185"). Reached by a compiler killed for memory in the fat-LTO step, a full disk, a registry that does not answer. It also hit the unit's own test: `target/tmp/rust-memory/cargo` is never cleaned, so after one mutant the memory test failed under the next ones until the directory was removed. Fix direction: decide from the linker's own error, or probe the linker, before writing the marker.
2. `crates/ignite/src/lib.rs:2092`, CONFIRMED, introduced, minor. The arm matches every `Error::Cargo`, so every failing release build runs cargo twice and blames the linker script. Red: `rust_memory_adversary::a_release_build_that_fails_before_any_link_fails_once_and_blames_no_linker_script` (cargo's "failed to load manifest" 2 times, base 1, the note between). A genuine link error: base prints 4 `error: linking with cc failed`, exit 2; head prints 8 plus the note, exit 2.
3. `crates/ignite-rustgen/src/lib.rs:1461`, CONFIRMED, introduced, minor. `cargo build --release` by hand in the release project of a program without an executable fails: "invalid instruction `cargo::rustc-link-arg-bins` ... does not have a bin target"; base exits 0. Red: `rust_memory_adversary::cargo_by_hand_builds_the_release_project_of_a_program_without_an_executable`. Reached only by the by-hand use `docs/slice.md:814` documents.
4. `crates/ignite/tests/rust_memory.rs:284`, CONFIRMED, introduced, minor (test gap). Two mutants survive the unit's three tests: `noconfig` (the driver passes no `--config`; cargo compiles and runs `layout.rs` on every cold release build) and `nostatic` (no layout when a triple is given). Pins added, green at head and red against each mutant: `the_drivers_release_build_compiles_no_build_script_of_the_project`, `a_static_release_build_links_with_the_layout`. Killed: `patterns`, `links`, `missing`, `nofallback`, `nomarker`.
5. `docs/slice.md:820`, CONFIRMED, introduced, minor. "writes the project without the three additions" is not what happens: `layout.ld` and `layout.rs` stay after the fallback, and after a later dev build into the same directory.
6. Acceptance bullet 2 for `rust/terminal/rr-1`, pre-existing, major. "terminald v0.15.0 2644, head 2632" does not reproduce: 25 interleaved runs, binaries copied then stripped in place, VmHWM kB: terminald v0.15.0 median 2660 (2604 to 2676), head 2692 (2628 to 2700), 18 of 25 head runs above the v0.15.0 maximum; todod 2984 against 2944, 0 of 25 above. Base is 2788 (7 runs), so the unit lowers the cell without reaching v0.15.0. The executable's code is at or below v0.15.0 (276 to 340 against 340); the rest is the libc window around the clock read.

Held: a rebuild with no change compiles nothing (0 `Compiling`, 13 `Fresh`); dev/release alternation recompiles nothing after the first two, the dev manifest equals base; base and head alternating in one directory leave no stale state; cold build steps 4 rustc and 0 build-script runs at base and head (static 18 and 4); rust-lld, GNU ld, clang as linker, musl static and gold (falls back) each give one executable segment, no RWX, no overlap, RELRO and `.eh_frame_hdr`, no linker warning, the symbolizer first (gold apart); resident code of todod by smaps: lld 668→508, GNU ld 676→552, static 700→644; a release `failoverd` logs 3 `task ended with error` and keeps serving; the stack overflow report equals base; a toy program linked with the generated script prints a symbolized backtrace under lld, GNU ld and musl; function sizes are the same multiset as base; paths with space and quote characters; two projects in one target directory; no new `unsafe`, ignored test or file over 3,000 lines.

Not run: `ignite apply` and `ignite run` builds, mold, the `rust:1.98-trixie` image, instruction counts.

For the coordinator: the generated release project differs (`Cargo.toml` +2 lines, `layout.ld`, `layout.rs`), so `CODEGEN_REVISION` needs its bump; whether `ld.gold` exists in the toolchain image is unknown (without it the fallback test prints "skipped").

```findings
[
{"file":"crates/ignite/src/lib.rs","line":2099,"category":"wrong-result","severity":"blocker","verdict":"CONFIRMED","origin":"introduced","message":"any cargo failure that passes on the second run writes the marker ignite-no-release-layout, and every later release build into that target directory links without the layout and prints nothing; red test rust_memory_adversary::a_release_build_after_one_that_cargo_failed_once_still_links_with_the_layout (290 functions that are not the symbolizer's lie before or among its 185)"},
{"file":"crates/ignite/src/lib.rs","line":2092,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"the fallback arm matches every Error::Cargo: a failing release build runs cargo twice and prints a note that blames the linker script; red test rust_memory_adversary::a_release_build_that_fails_before_any_link_fails_once_and_blames_no_linker_script (failed to load manifest 2 times, base 1); a genuine link error prints 8 linker errors and the note where base prints 4"},
{"file":"crates/ignite-rustgen/src/lib.rs","line":1461,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"cargo build --release by hand in the release project of a program without an executable fails in layout.rs (invalid instruction cargo::rustc-link-arg-bins, no bin target); base exits 0; red test rust_memory_adversary::cargo_by_hand_builds_the_release_project_of_a_program_without_an_executable; reached by the by-hand use docs/slice.md 814 documents"},
{"file":"crates/ignite/tests/rust_memory.rs","line":284,"category":"missing-test","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"two mutants survive the unit's three tests: the driver passing no --config (cargo then compiles and runs layout.rs on every cold release build) and no layout when a triple is given; the pass added two pins, green at head and red on each mutant"},
{"file":"docs/slice.md","line":820,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the text says the fallback writes the project without the three additions; layout.ld and layout.rs stay after the fallback and after a later dev build into the same directory"},
{"file":".engineering/planning/story/rust-peak-memory-at-one-connection.md","line":1,"category":"acceptance","severity":"warning","verdict":"INFEASIBLE","origin":"pre-existing","message":"terminald VmHWM at head is above v0.15.0 in 18 of 25 interleaved runs (median 2692 kB against 2660; todod 2944 against 2984 holds, 0 of 25 above); base is 2788, so the unit lowers the cell without reaching v0.15.0; the remainder is the libc code around the clock read of the idle timeout, outside this unit's files"}
]
```
