---
format: aep.planning-md/3
id: review-result:adversary-size-pass-2
kind: review-result
status: active
title: Adversary pass 2, unit size (wave 11)
relations:
- reviews: story:binary-size-back
revision: 1
---
red

Adversary pass 2 on unit size, attacked at `ad4d980d` (fix round 2, `9e4cd746..ad4d980d`); the pass's cases are commit `1e31b619` (`crates/ignite-io/tests/timer_adversary.rs`, `crates/ignite/tests/binary_size_adversary.rs`).

verdict: red; cases executed 232→240 (ignite-io, both IO modes) and 26→27 (ignite selection), red 0; origin: introduced 5, pre-existing 2. No case is red and no correctness defect was found in the indexed heap; the verdict is for counts above the unit's base.

Scenario of findings 1 to 3: todod, one worker, N keep-alive connections (open, one request each, close 40 %, request, close the rest), instructions. At 1000 connections, base / pre-round head / head / head with the slab back: LLVM per request 14,321 / 13,247 / 13,421 / 13,324; Rust per request 13,888 / 12,360 / 12,474 / 12,377; LLVM close 600: 3,468,810 / 3,373,584 / 3,575,184 / 3,431,597; Rust close 600: 2,895,244 / 2,830,133 / 2,978,868 / 2,866,743.

Findings:
1. `crates/ignite-io/src/sched.rs:596`, NEEDS-CHANGE, introduced, warning. Closing many connections with armed timers costs more than at the unit's base: close 600 of 1000 +3.1 % on LLVM and +2.9 % on Rust; close 60 of 100 +7.9 % on Rust and −1.5 % on LLVM; at 10 equal or lower. `Timers::sift_down` takes 459 instructions per removal at 600 entries and 254 at 60; the BTreeMap took about 164. Head with the slab back is below base here.
2. `crates/ignite-io/src/reactor.rs:346`, CONFIRMED, introduced by `829c28e9`, warning. Against the pre-round head the HashMap costs +97 instructions per request on both targets (`EpollReactor::wait` 198 → 297 per event) and about +317 per connection opened, at 10 and 1000 connections, and 5 system calls once per process; equal to base. The single-connection metric behind the revert does not show it.
3. `crates/ignite-io/src/sched.rs:518`, CONFIRMED, introduced by `58b5ffe0`, warning. Against head with the pre-round heap the indexed heap costs +64 instructions per request on LLVM and +14 on Rust, and +275 / +131 per connection closed among 1000; on Rust `Timers::slot` is not inlined (+40 per request). It saves 2,116 (LLVM) and 3,102 (Rust) instructions per idle period at 10 connections; 6 `epoll_wait` against 9 confirmed. Below base.
4. `crates/ignite-io/src/sched.rs:603`, CONFIRMED, introduced, note. Three of 16 heap mutants survived the round's tests: a removal that only sifts down, `fire_timers` without the second arming, an `arm` that never moves earlier. Cases of `1e31b619` hold all three.
5. `crates/ignite-rustgen/src/lib.rs:7424`, CONFIRMED, introduced by `664ae7f8`, note. `rustc ignite_std` in dev takes 2,138.0 M instructions against 1,892.2 M at the pre-round head (+13 %), the price of split line tables for the standard library's frames; the whole Rust dev path is lower (24,634.7 M against 26,414.2 M) and so is the link.
6. `std/protocol/http.ignite:221`, CONFIRMED, pre-existing, warning. On the LLVM target todod never closes an accepted socket: 40 served and closed connections leave 40 open descriptors at the unit's base (7 → 47) and at head (7 → 48); the Rust target leaves none (6 → 6) and LLVM terminald closes. v0.15.0 not measured in the pass.
7. `crates/ignite-io/tests/scheduler.rs:35`, CONFIRMED, pre-existing, note. `child()` waits for its child without a limit, so a worker that panics hangs the test run instead of failing it.

Held: heap correctness by review, a model test and 13 red mutants (equal deadlines, root, last and only entry, an entry already popped, re-arming, slot reuse, 4 workers, threads mode). The fixes of F1, F2, F3 and F7 each turn their named test red when reverted. `reactor.rs` equals base; LLVM start 356,226 against 416,135 at base. The +226 on Rust requests in threads mode does not reproduce (131,039 at head in four rounds, 131,035 three times and 131,014 once before). Failure report and backtrace on both fixtures and targets keep file and line in place, moved and without the `.dwo` files; release has no `.dwo` file and no debug section; dev link Rust 157.9 M against 163.4 M, LLVM 256.1 M against 258.1 M; LLVM dev path 4,290.8 M against 4,292.9 M; 6 of 6 build scripts and host crates without debug assertions. Packed relocations on LLVM dev and release, none on Rust, none with `--static`; `RUSTFLAGS` and `CARGO_BUILD_RUSTFLAGS` reach the link. System calls on a single connection equal within spread; LLVM terminald threads session 26 at both heads. Suite at `1e31b619`: fmt and clippy 0 for ignite-io and ignite; ignite-io 118 + 240 in both modes; ignite selection 27 passed. Not run: `task gate`, quality.

```findings
[
{"file":"crates/ignite-io/src/sched.rs","line":596,"category":"judgement","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"closing many connections with armed timers costs more than at the unit's base (close 600 of 1000 +3.1 % LLVM, +2.9 % Rust; close 60 of 100 +7.9 % Rust), Timers::sift_down taking 254 to 459 instructions per removal against about 164 for the BTreeMap"},
{"file":"crates/ignite-io/src/reactor.rs","line":346,"category":"judgement","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"the HashMap brought back by 829c28e9 costs +97 instructions per request and about +317 per connection against the pre-round head on both targets once a worker holds more than one connection; equal to base"},
{"file":"crates/ignite-io/src/sched.rs","line":518,"category":"judgement","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"the indexed heap costs +64 (LLVM) and +14 (Rust) instructions per request and +275 and +131 per closed connection against the pre-round heap, for 2,116 to 3,102 fewer per idle period; below base"},
{"file":"crates/ignite-io/src/sched.rs","line":603,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"a removal that only sifts down, fire_timers without the second arming and an arm that never moves earlier each left the round's tests green; cases that hold them are in 1e31b619"},
{"file":"crates/ignite-rustgen/src/lib.rs","line":7424,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"rustc ignite_std in dev takes 2,138.0 M instructions against 1,892.2 M at the pre-round head, the price of split line tables for the standard library's frames, while the whole dev path and the link are lower"},
{"file":"std/protocol/http.ignite","line":221,"category":"judgement","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"on the LLVM target todod never closes an accepted socket: 40 served and closed connections leave 40 open descriptors at the unit's base and at the head, none on the Rust target"},
{"file":"crates/ignite-io/tests/scheduler.rs","line":35,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"child() waits for its child without a limit, so a worker that panics hangs the test run instead of failing it"}
]
```
