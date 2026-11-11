---
format: aep.planning-md/3
id: review-result:adversary-unsafe-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit unsafe (wave 11)
relations:
- reviews: story:unsafe-sites-back
revision: 1
---
red

Adversary pass 1 on unit unsafe, `wave/11-unsafe` at `07c4f9aa`, test commit `bd3033a0`.

verdict: red; cases executed 318→322, red 1; origin: introduced 3, pre-existing 1.

Findings:
1. `crates/ignite-io/src/fs.rs:189` (`entries`), CONFIRMED, introduced, warning. A directory removed before its listing lists as `Ok([])` (glibc `readdir` reads `getdents64`'s ENOENT as end of directory); base returned the error. Red case `unsafe_adversary::a_listing_of_a_removed_root_is_an_error_as_before`.
2. `crates/ignite-io/src/fs.rs:576` (`Level` holds a `Dir`), INFEASIBLE, introduced, note, not measured. Each `remove_tree`/`measure` level holds a glibc `DIR` with a 32 KiB buffer, up to 256 levels, outside the census.
3. `crates/ignite-io/src/sched.rs:1181`, CONFIRMED, introduced, note. `on_fault`, a safe fn that dereferences its raw `info` pointer, widened from private to `pub(crate)`.
4. `crates/ignite-io/src/sched.rs:631` (race fix `07c4f9aa`), INFEASIBLE, pre-existing, note. The fix is correct; no test fails without it; a deterministic case needs a hook in `start_worker`.

Held: counter unchanged against base and reads `signal.rs`; row 3.6 by three methods: ignite-io 12 (v0.15.0), 25 (base), 11 (now); ignite-rt 70; nothing moved into a macro, crate or dependency (`Cargo.lock` and every `Cargo.toml` unchanged); no new `#[ignore]`; no file over 3,000 lines (largest touched `sched.rs`, 1,719); accept still one `accept4` with `SOCK_CLOEXEC`; `Slot::release` deregisters the descriptor it registered; `forget` on drop unchanged from base; byte search agrees with a window compare on 20,000 fixed-seed inputs; realloc through `Counting` gives exact census deltas; the four strace count tests exit 0 five times; `leak::` 5 passed; the new quality test is unaffected by a v0.16.0 record and goes red at two added sites (one added site is caught by `static_metrics_hold_the_bar` once the bars are 11 and 81).

Not run: release `terminald` byte count on the Rust target; clippy on the adversary's test file (cargo was stopped by the machine's disk guard at 19 GiB free).

```findings
[
{"file":"crates/ignite-io/src/fs.rs","line":189,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"a directory removed before its listing lists as Ok([]) because glibc readdir reads getdents64's ENOENT as end of directory, where base returned the error"},
{"file":"crates/ignite-io/src/fs.rs","line":576,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"each remove_tree/measure level holds a glibc DIR with its 32 KiB buffer, up to 256 levels, outside the census; not measured"},
{"file":"crates/ignite-io/src/sched.rs","line":1181,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"on_fault, a safe fn that dereferences its raw info pointer, was widened from private to pub(crate)"},
{"file":"crates/ignite-io/src/sched.rs","line":631,"category":"concurrency","severity":"note","verdict":"INFEASIBLE","origin":"pre-existing","message":"the LIVE race fix is correct but no test fails without it; a deterministic case needs a hook in start_worker"}
]
```
