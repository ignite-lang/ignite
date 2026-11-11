---
format: aep.planning-md/3
id: story:binary-size-back
kind: story
status: active
title: Every binary is at or below its v0.15.0 size, and peak memory at one connection with it
relations:
- depends_on: story:build-time-back
- depends_on: story:unsafe-sites-back
- informed_by: story:tls-engine-own-crates
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: cited
  path: Cargo.toml
- confidence: inferred
  path: Taskfile.yml
- confidence: cited
  path: crates/ignite-io/src/reactor.rs
- confidence: cited
  path: crates/ignite-io/src/sched.rs
- confidence: cited
  path: crates/ignite-io/src/task.rs
- confidence: cited
  path: crates/ignite-io/src/wait.rs
- confidence: cited
  path: crates/ignite-rustgen/src/lib.rs
- confidence: cited
  path: crates/ignite/src/llvm_build.rs
- confidence: inferred
  path: dist/toolchain/Dockerfile
- confidence: inferred
  path: quality/baseline.json
- confidence: inferred
  path: tools/ignite-perf/src/bench.rs
- confidence: inferred
  path: tools/ignite-perf/src/remote-user-data.sh
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-11T00:24:54Z", actor: "human:timo", revision: 4, executor: "agent:claude-coordinator", correlation: "wave-11"}
- {from: "proposed", to: "active", at: "2026-11-11T00:24:54Z", actor: "human:timo", revision: 5, executor: "agent:claude-coordinator", correlation: "wave-11"}
---
# Story: every binary is at or below its v0.15.0 size, and peak memory at one connection with it

## Outcome

Rows 3.9 (four cells), 3.2 (eight cells) and 3.8 at one connection (`rr-1`, `get-1`, both
targets) are at or better than v0.15.0.

## Why

Dedicated-host run `iq-20261110t211023z-863a` (HEAD `4520b8cf`, BASE v0.15.0): 3.9 llvm/terminal
1.406x -> 1.830x, llvm/todo 0.720x -> 0.901x, rust/terminal 1.304x -> 1.547x, rust/todo 0.723x ->
0.850x; 3.8 llvm/terminal/rr-1 +7.2 %, llvm/todo/get-1 +8.5 %, rust/terminal/rr-1 +4.8 %,
rust/todo/get-1 +7.4 %. Row 3.2: all eight bars were moved worse by decisions for v0.16.0.

Diagnosis (`.engineering/waves/2026-11-11-wave-11-diagnosis/size.md`, `rss-rr.md`; sizes are deterministic and were
measured locally, stripped sizes reproduce the dedicated host's to within one page):

- Release growth is the readiness scheduler (about 90 % for terminal, 80 % for todo); the rest is
  the HTTP library and channel builtins. Wave 9's LTO and opt-level 1 hide growth; they do not
  remove it.
- Dev growth on the LLVM target after the five `debug = false` overrides: about 53 % debug info
  of `ignite-io` itself, about 37 % the C objects of the TLS engine's crypto crate, which its
  build script compiles with full debug info whatever the profile says.
- Peak memory at one connection grew with the executable (164.5 / 184.5 / 94.3 / 129.8 KiB of
  file against +212 / +252 / +100 / +240 KiB of memory); anonymous memory is nearly unchanged.
- Every binary, at v0.15.0 and now, carries the standard library's backtrace symbolizer: about
  171 KB of code plus about 33 KB of helpers on the LLVM target.

Measured changes, stripped bytes todo / terminal:

1. compressed debug sections on dev links: LLVM dev 11.7 / 11.0 MB -> 4.2 / 3.5 MB, Rust dev 9.3 /
   7.8 -> 4.6 / 3.5 MB; backtraces keep file and line under zlib.
2. packed relative relocations: LLVM -36,792 / -36,792, Rust -22,104 / -19,480.
3. five code cuts in `ignite-io` (prototypes, compiled and linked, no test run): the CPU count
   from the affinity mask, timers in a heap, no thread mode, a slab for the reactor table, a
   direct `poll` off a worker: together LLVM -49,200 / -49,200, Rust -50,024 / -50,472.
4. `ignite-io` and `ignite-rt` at `opt-level = "s"`: LLVM -57,360 / -53,264, Rust -43,248 /
   -37,672.
5. `--discard-all` on the LLVM release link: 3.2 release -143,272 / -130,912.
6. the runtime archive on a standard library built without its backtrace feature (an unstable
   cargo flag on the stable toolchain): LLVM -129,456 / -137,648.
7. the generated std crate at `opt-level = "s"`: Rust -38,824 / -20,672.
8. `--as-needed` on the LLVM link (an unused library is no longer loaded): 330 to 440 KiB of
   peak memory locally.

Reach: with 2, 3 and 4 the Rust terminal cell is under v0.15.0, the Rust todo cell needs 7. The
two LLVM cells of 3.9 stop at 796,592 / 591,792 against 751,016 / 562,600 on a stock standard
library; with 6 they are at 667,136 / 454,144.

## Decisions the coordinator took for this story

- The LLVM target's release runtime archive is built on a standard library without the
  backtrace symbolizer (change 6). Reason: it is the only measured way to the two LLVM cells of
  3.9, and an Ignite program reports its failures itself. Cost: an unstable cargo flag and the
  standard library's source wherever that archive is built; a panic inside the runtime prints
  its message and location with an unsymbolized backtrace.
- The CPU quota of a container stays honoured: the affinity count replaces the standard
  library's call only together with an own, small read of the quota, tested against fixture
  files. `IGNITE_IO=threads` stays unless a cell cannot be reached without it; in that case the
  unit stops and reports the cell and the bytes.

## Acceptance

- Row 3.9: the four stripped sizes are at or below v0.15.0's bytes of the dedicated-host run
  (llvm 750,968 / 558,456, rust 753,936 / 517,992, todo / terminal), measured locally by the same
  build `tools/ignite-perf/src/bench.rs` makes; a test or the unit's report states each.
- Row 3.2: the eight cells are at or below their v0.15.0 bars (llvm terminald dev 7,563,720,
  release 1,468,648; llvm todod dev 8,230,560, release 2,042,752; rust terminald dev 6,223,520,
  release 681,368; rust todod dev 7,405,864, release 1,011,216), the bars are blessed to the
  measured values and the v0.16.0 decisions on 3.2, 3.9 and 3.8 are withdrawn.
- A program that names no `Tls` links no object of the TLS engine, C objects included, on the
  LLVM target in dev and release; a test builds one and fails on a compile unit or a symbol of
  it. A program that opens a TLS session still builds and passes the inline tests of
  `std/tls.ignite` on both targets.
- A dev backtrace of a failing Ignite program still names file and line; a release build's
  failure report is unchanged.
- No `unsafe` site is added (row 3.6). The dev link does no more work than
  `story:build-time-back` left it with: the link input bytes and debug relocations of the todo
  dev build do not rise, and a change that compresses or post-processes debug sections states
  the bytes it reads and writes.
- `task dist`, the toolchain image and the dedicated host's setup build the archive the same
  way; `AGENTS.md` § Releases names what they need installed.
- The speed of what changes 3, 4 and 7 touch is "not measured locally"; the dedicated-host run
  of the release candidate decides. A row of 3.3 or 3.7 that run traces to one of them comes
  back to this story.

## Scope

- `Cargo.toml` (profiles), `crates/ignite/src/llvm_build.rs` (link flags, archive choice),
  `crates/ignite-rustgen/src/lib.rs` (generated profile). Cited.
- `crates/ignite-io/src/sched.rs`, `reactor.rs`, `wait.rs`, `task.rs`, timers. Cited.
- `Taskfile.yml` (how the runtime archive is built), `dist/toolchain/Dockerfile`,
  `tools/ignite-perf/src/bench.rs` and the host scripts. Inferred.
- Runs after `story:build-time-back` in the same slot: both change `Cargo.toml`,
  `llvm_build.rs` and the generated profile. It changes `crates/ignite-io/src/sched.rs` and
  `reactor.rs` after `story:readiness-syscalls-per-connection` and `story:unsafe-sites-back`
  have merged.
