---
format: aep.planning-md/3
id: review-result:adversary-http-app-pass-1
kind: review-result
status: active
title: 'Adversary, pass 1: http-app-connection'
relations:
- reviews: story:http-app-connection
revision: 1
---
```json
[
  {"id":"F1","severity":"minor","title":"With on_oversized, a client that sent Expect: 100-continue and never sent the body loses its next request: the server answers 413 keep-alive, reads that request as the body, then answers the rest with 400","file":"std/protocol/http.ignite:2009","evidence":"http_app_adversary.ignite 'adv: a body over the limit announced with Expect: 100-continue and never sent does not swallow the next request', red on LLVM and Rust under readiness and threads: 'error: assert failed at crates/ignite/tests/fixtures/http_app_adversary.ignite:72:3: !out.contains(\"400 Bad Request\")'. Output: 'HTTP/1.1 413 Content Too Large ... Connection: keep-alive ... over 4HTTP/1.1 400 Bad Request'. Std never sends 100. Reach: curl closes the connection in this case; a client that reuses it does not. Go's net/http closes in this state. Fix: when the request has Expect: 100-continue, keep = false.","introduced_by_unit":true,"acceptance":"on_oversized: 'discards the body and keeps the connection alive when the answer allows it'"},
  {"id":"F2","severity":"minor","title":"A record literal gets its type parameters only from the arguments before it, so the claim 'as a closure was' is inexact","file":"crates/ignite-types/src/typer.rs:617","evidence":"adversary_http_app::a_record_literal_before_the_argument_that_binds_its_parameter: 'left: Some(\"probe.ignite:12:20: error[E042]: cannot infer type parameter `S` of `run`; ... error[E042]: cannot infer type parameter `T` of `Box`\")'. The call is run(Box { state = \"s\" }, 7) with run<T, S>(boxed: Box<S, T>, stream: T). Closures are deferred to the second pass; record literals are not. Base gives E042 too. docs/slice.md:10633 says 'as far as the other arguments bind it ... as a closure was'.","introduced_by_unit":false,"acceptance":"none"},
  {"id":"F3","severity":"note","title":"AGENTS.md and the book still show HttpApp<S> after the breaking change","file":"AGENTS.md:45","evidence":"AGENTS.md:45 gives 'serve(stream: T, limits: HttpLimits, application: HttpApp<S>)'. site/book/src/reference/std/rfc.md:99 (a fragment block, which nothing checks) shows HttpApp<S>. The std/net.ignite row of AGENTS.md:34 does not mention local() or Connection.","introduced_by_unit":true,"acceptance":"none"},
  {"id":"F4","severity":"note","title":"NN.6 does not say that Upgrade, Expect and 413 behaviour is outside the imported catalogue","file":"docs/slice.md:10739","evidence":"std/rfc/rfc9110.ignite imports only sections 5, 6, 7.2 and 9.x. Upgrade (7.8), Expect (10.1.1) and 413 (15.5.14) have no claim. NN.6 says only that no clause moved; it does not name these as outside the catalogue.","introduced_by_unit":true,"acceptance":"none"},
  {"id":"F5","severity":"note","title":"A held request whose client has gone keeps its task, receiver and register entry until the deadline","file":"std/protocol/http.ignite:1941","evidence":"Found by reading the code; no test. wait_held does not read the stream while it waits, so a disconnect is not seen. The application's send returns true and the answer is written to a dead socket. This is bounded by the deadline, and the docs do not state it.","introduced_by_unit":true,"acceptance":"none"}
]
```
- I stopped early: free disk fell to 28 GB (limit 30 GB). I killed my own suite run partway. So the full suite after the cases (std http, net and sync on both targets and both IO modes, then the `ignite-types` crate) never ran, and I have no before/after case count.
- `git --no-pager diff --stat`: the only tracked change is `crates/ignite-types/tests/main.rs | 1 +`; the 16 evidence deletions were already there. Untracked and uncommitted: `crates/ignite-types/tests/adversary_http_app.rs` and `crates/ignite/tests/fixtures/http_app_adversary.ignite`. No implementation file was touched.
- Typer cases alone: `cargo test -p ignite-types --test main adversary_http_app::` exit 101, 4 passed and 1 red (F2).
- Fixture alone (14 cases), exit 1 on all four runs (LLVM and Rust, each under readiness and threads). Only F1 is red. One case of mine raced under rust/threads: the second send can arrive after the receiver is closed. I removed that assertion, and the rerun on rust/threads was 13 passed, 1 red (F1).
- Mutants on scratch copies of std, unit tests on LLVM. Removing the deadline fallback, skipping the discard and handing takeover empty bytes are each caught by the unit's own tests. Not ending the register at drain passes the unit's inline tests (exit 0). It is caught by a replay of `drain::the_drain_answers_a_held_http_request_at_once` on LLVM, where the server "did not exit within 60 s", and by my case "the register of held requests ends at the drain", where `Runtime.settle` fails. Rust was not replayed: both builds wrote to the same output path, and I could not rebuild because of disk.
- What held:
  - **Holds:**
    - A dropped sender gets `on_timeout` at once.
    - Two messages are answered once, and pipelined requests stay in order, HEAD included.
    - A request held after the drain is answered at once.
  - **Upgrades:**
    - Takeover gets exactly the early bytes after a body, after a chunked body, with nothing following, and with pipelined requests following.
    - A 200 upgrade is framed by its length.
    - A failing takeover ends the connection.
    - An oversized upgrade request goes to `on_oversized`.
  - **Oversized bodies:**
    - Exactly at the limit is handled; one byte over goes to the hook, and the next request starts right after the discarded body.
    - A stalled discard ends at the idle timeout.
  - **Connection:**
    - Forwarding headers cannot spoof the allow list.
    - A dual-stack `::` listener shows an IPv4 peer and local end as IPv4.
    - `local()` stays known after the peer closes and after this end closes.
  - **Typer:** swapped type-parameter names, nested generic records and a plain record passed to a generic parameter all type without error; a conflicting field is still E040.
- Breaking change: a plain app does not name `T` in a declaration or a call, where it is inferred (`TcpStream` under `HttpServer`, the call's stream type otherwise). An unannotated `let app = HttpApp {..}` is E042, as NN.1 says; nothing in std/, examples/, site/ or the test fixtures does that.
- No "readiness only" lines: none of my cases use many connections. I did not run the known thousand-hold test.
- Paths written outside the worktree: everything under `/home/timo/.cache/scratch/2db1a4b7-2d2d-4a62-bab2-d71ac128826e/w10/http-app/adv1/` (scripts, logs, `std-m1` to `std-m4`, `out-*`, `build-*`, 144 MB).
- Builds that went into the worktree's own target: `target/ignite-test/rt` (the runner runtime, rebuilt), and in `target/ignite-test/cargo` the fixture and `holddrain` builds, where `debug/holddrain` is now the head build.
