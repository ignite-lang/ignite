---
format: aep.planning-md/3
id: review-result:adversary-reply-pass-1
kind: review-result
status: active
title: Adversary pass 1, unit reply (wave 11)
relations:
- reviews: story:http-reply-cost-back
revision: 1
---
red

Adversary pass 1 on unit reply, `wave/11-reply` at `fb731c8e`, test commit `8197ddce`.

verdict: red (nothing red on head; four introduced notes); cases executed 640→650, red 0; origin: introduced 4.

Findings:
1. `std/protocol/http.ignite:1849`, CONFIRMED, introduced, note. Removing the new `Streamed` branch that gives `piece` the connection left every unit test green; the adversary's piece case in `fixtures/http_reply_adversary.ignite` now catches it.
2. `crates/ignite-io/src/net.rs:358`, CONFIRMED, introduced, warning. A task keeps its pending list at its largest size until it ends: 261,869 B on the Rust target after one task wrote to 10,000 pipe pairs (16,384 slots of 16 B). On LLVM the same probe left +11.4 MB, not separated.
3. `std/protocol/http.ignite`, CONFIRMED, introduced, note. A file already over 3,000 lines grew from 3,133 to 3,156.
4. `std/protocol/http.ignite` / `net.rs`, CONFIRMED, introduced, note. Rust release `todod` grew 448 B over the implementor's base build (1,086,680 → 1,087,128); LLVM release shrank 1,556,016 → 1,543,528.

Held: allocations per reply counted two ways (valgrind trace, an LD_PRELOAD counter), release builds, v0.15.0 / base / after: Rust GET /todos/1 37/53/37, GET 404 31/47/31, GET /todos 36/52/36, PATCH 95/116/95, POST 86/107/86, DELETE 63/75/63; LLVM 1/1/0, 1/1/0, 1/1/0, 12/12/11, 10/10/9, 12/12/11. System calls per GET reply 1.97 to 2.03 on all six binaries. 19 split and pipelining cases byte-identical across v0.15.0 and after on both targets. RSS flat from 2,000 to 20,000 requests on one connection. Mutants caught: always start `Holds` (census test red), connection on the kept request (Rust 39), old `flush` (Rust 38), old `read_head` copy (Rust 38). `flush` after an error sends no bytes twice. "Has a `reply`" is decided at run time on the `Option`. Inline test names of `http.ignite` identical to base (88). No new `unsafe`, dependency or ignored test. Suite: 102 passed, 2 ignored; ignite-io 112 + 203; clippy 0; inline tests 116/116 per target.

Startup system calls of a Rust `todod`: v0.15.0 262, base 326 to 330, after 306 to 308.

```findings
[
{"file":"std/protocol/http.ignite","line":1849,"category":"test-gap","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"removing the new Streamed branch that gives piece the connection left every unit test green; the piece case in fixtures/http_reply_adversary.ignite now catches it"},
{"file":"crates/ignite-io/src/net.rs","line":358,"category":"resource","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"a task keeps its pending list at its largest size until it ends: 261,869 B on the Rust target after one task wrote to 10,000 pipe pairs (16,384 slots of 16 B)"},
{"file":"std/protocol/http.ignite","line":1,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"a file already over 3,000 lines grew from 3,133 to 3,156; story ignored-tests-and-long-files-back splits it"},
{"file":"std/protocol/http.ignite","line":1,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"Rust release todod grew 448 B over the base build (1,086,680 to 1,087,128); LLVM release shrank 1,556,016 to 1,543,528; story binary-size-back takes the Rust bytes"}
]
```
