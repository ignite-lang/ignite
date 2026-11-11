---
format: aep.planning-md/3
id: review-result:adversary-reply-pass-2
kind: review-result
status: active
title: Adversary pass 2, unit reply (wave 11)
relations:
- reviews: story:http-reply-cost-back
revision: 1
---
red

Adversary pass 2 (last) on unit reply, fix rounds `2e57c8c8..83600833`, attacked at `c0a319cf`; the pass's cases are commit `c05b5bd1`.

verdict: red; cases executed 1919→1934, red 0 (the red verdict is a measured count, finding 1); origin: introduced 3, pre-existing 0.

Findings:
1. `crates/ignite-io/src/signal.rs:98`, CONFIRMED, introduced, blocker. LLVM release bytes are above `1f68ceb4` on both programs. File bytes `1f68ceb4` → head: `todod` with the release runtime 1,214,408 → 1,216,184 (+1,776); `terminald` 964,504 → 966,128 (+1,624); `todod` with the debug runner archive (row 3.2's method) 2,229,312 → 2,244,264 (+14,952); `terminald` 1,629,112 → 1,631,776 (+2,664). Stripped the growth is +8 B each. Rust shrank: `todod` −1,736, `terminald` −304. `.text` of LLVM `todod` grew 2,107 B: the reply rendering helpers of `http.ignite` (`b4406420`) +1,288 net; `text::find` (`ecd857e7`) +432; `<[Signal; 2]>::try_map` in `install_overflow_handler` +428, `OnceLock<fn()>` +245, `alternate_signal_stack` +125 (all `83600833`); `Counting::realloc` with `realloc_fallback` (`eae98c9c`) +336. The implementor's per-commit builds show `83600833` alone adds +1,824 to `todod`; before it the build was 48 B under `1f68ceb4`. Suggested: install the two signals with two calls of a small helper instead of the array map.
2. `crates/ignite-io/src/signal.rs:66` and `docs/slice.md:11263`, CONFIRMED, introduced, note. Both say a Rust program started with SIGSEGV or SIGBUS ignored gets no alternate stack; that holds only when both are ignored. With only SIGBUS ignored the overflow is reported (status 134). With only SIGSEGV ignored the process ends by signal 11 with no report (status 139) although std gave every thread a stack; `1f68ceb4` reported in all four kinds of start. It matches v0.15.0, where tasks were std threads.
3. `std/protocol/http.ignite:578`, CONFIRMED, introduced, note. The rounds grew the file from 3,156 to 3,208 lines with rendering helpers outside the brief's boundary; unit counts takes 286 lines out of the file.

Held: allocations per reply by valgrind trace and by an LD_PRELOAD counter agree, release builds, v0.15.0 → head, Rust: GET /todos/1 37+3 → 36+0, 404 28+4 → 27+1, GET /todos 36+3 → 35+0, PATCH 95+5.8 → 94+2.8, POST 86.3+6.8 → 85.3+3.8, DELETE 63+7.8 → 63+5.8; LLVM: 1 → 0, 1 → 0, 1 → 0, 12+0.8 → 11+0.8, 10.2+1 → 9.2+1, 12+1 → 11+1. Per connection head is below the tree before unit unsafe (Rust 47 against 50.3, LLVM 19.1 against 22.1) and above v0.15.0 (44 and 9). Instructions (callgrind), head / before unit unsafe / `1f68ceb4`: Rust GET 15,708 / 15,877 / 17,117, DELETE 28,976 / 28,826 / 29,903, per connection 25,137 / 25,636 / 26,300; LLVM GET 14,146 / 14,525 / 15,161, per connection 29,601 / 30,313 / 30,735. System calls per request equal on the three trees (2 / 2 / 2 / 15 / 19 / 15); per connection Rust 11.05 against 11.13, LLVM 10.12 against 10.43. Row 3.6 by the row's counter and by `git grep`: `ignite-io` 12, `ignite-rt` 67. 19 split, pipelined and oversized scenarios are byte-identical to v0.15.0 on both targets. SIGPIPE: with the ignore removed from a copy of the runtime `std.process/a child that does not read its input still ends` fails; green on head. No other byte loop of `b20dceda` is left; the LLVM runtime calls the same `find`. No worker starts before the hook is set; the hook is a `OnceLock` set before any spawn. No Ignite program can reach `pub mod signal`. The leak soaks, `llvm_memory::`, `terminal::idle_terminald_runs_one_thread` hold; no new `#[ignore`. Suite: `cargo test -p ignite-io` 116 passed, 1 ignored and 227 passed in both modes; `cargo test -p ignite-rt` 44 and 29; the `ignite` lanes 235 passed, 1 failed (static metrics, improved rows only), 2 ignored; `cargo test --no-fail-fast -p ignite-check` 10 and 25 passed, 1 ignored, exit 0; `task test-ignite` 452 per target; clippy and fmt exit 0.

```findings
[
{"file":"crates/ignite-io/src/signal.rs","line":98,"category":"acceptance","severity":"blocker","verdict":"CONFIRMED","origin":"introduced","message":"LLVM release todod and terminald are above 1f68ceb4's (+1,776/+1,624 B with the release runtime, +14,952/+2,664 B with row 3.2's debug archive), mostly from the round-5 signal code, with the array map in install_overflow_handler carrying 428 B of text"},
{"file":"crates/ignite-io/src/signal.rs","line":66,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the docs (also docs/slice.md:11263) say a Rust start with SIGSEGV or SIGBUS ignored has no alternate stack, but with only SIGBUS ignored the overflow is reported, and with only SIGSEGV ignored it now ends by signal 11 with no report where 1f68ceb4 reported"},
{"file":"std/protocol/http.ignite","line":578,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the rounds grew the file from 3,156 to 3,208 lines with rendering helpers outside the brief's serve_listener/Request/Connection boundary, where unit counts moves helpers next"}
]
```
