---
format: aep.planning-md/3
id: review-result:adversary-std-resp-client-pass-1
kind: review-result
status: active
title: 'Adversary, pass 1: story:std-resp-client'
relations:
- reviews: story:std-resp-client
revision: 1
---
verdict: findings
unit: story:std-resp-client at 6b6f8bf2 (worktree w10-resp, one new test file added)
cases: executed 46→52 (23→26 per target, llvm+rust), red 6 (3 per target)
origin: introduced 3 / pre-existing 0 / undecided 0 (`std/protocol/resp.ignite` does not exist at 7f1b8795)
wrote-outside-worktree: 6 files, all in the assigned scratch directory (part 6)
needs-coordinator: none

1. Diff: `git diff --stat` is empty. `git status --short` shows one line, `?? crates/ignite/tests/fixtures/tests/resp_adversary.ignite`, which is a test file. No implementation file was touched. Test builds went to `target/ignite-test/adversary/` inside the worktree.

2. Cases in `crates/ignite/tests/fixtures/tests/resp_adversary.ignite`, each run alone on each target before the suite. All are red now.
   - `a chain of attributes is held to the depth limit`: expects `decode` of 3 × `|0\r\n` then `:1\r\n` with `depth = 2` to be `Invalid(TooDeep)`. It gets `Split.Complete(RespValue.Attributed([], RespValue.Attributed([], RespValue.Attributed([], RespValue.Integer(1)))), [])` on both targets.
   - `a long chain of empty attributes within the default limits is refused, not recursed into`: 200,000 × `|0\r\n` (800 KB, well under the 64 MiB reply limit) with default limits. LLVM: `FAILED (killed by signal 11)`. Rust: `thread 'main' has overflowed its stack / fatal runtime error: stack overflow, aborting` (signal 6).
   - `TYPE refused by the server is the server's error, as for every other helper`: the server answers `-NOAUTH Authentication required.`. On both targets the error is `left: resp: unexpected reply to TYPE` where `right: NOAUTH Authentication required.` was expected.

3. Commands run:
   - each case alone: `target/debug/ignite test <file> "resp_adversary/<name>" --exact --target llvm|rust ...`. All 6 runs reported `FAILED`. The logged "exit 0" belongs to the `tail` the output was piped through, not to the test.
   - `ignite test std/protocol/resp.ignite resp_adversary.ignite --target=all`: `23 passed; 3 failed` on each target, exit 1.
   - the same with my file left out (`std/protocol/resp.ignite` only): `23 passed; 0 failed` on each target, exit 0.
   - `ignite test <scratch>/probe.ignite --target=all`: 3 passed on each target, exit 0.
   - The first attempt (`case1-llvm.log`) used a filter without the `resp_adversary/` prefix and matched nothing (0 tests run, exit 0).

4. Findings:
   - **F1** `std/protocol/resp.ignite:370`, blocker, NEEDS-CHANGE, introduced. The `|` arm of `aggregate` decodes the value an attribute annotates at the same `depth`. A chain of attributes therefore never reaches the depth limit and recurses until the stack runs out. That aborts the whole process, not just the reader task. It is reachable from any reply read through `command`, `pipeline`, `subscribe`'s reader or `read_value`; Redis never sends such a chain, but a hostile or broken server or proxy can. Fix: decode the annotated value at `depth + 1`, or count attributes against `depth`.
   - **F2** `std/protocol/resp.ignite:820`, warning, CONFIRMED, introduced. `key_type` passes the reply through `text_of` without `failure`, so the server's error line is lost. This contradicts `docs/slice.md:10621-10623`, which says the server's error becomes the `IoError` message and any other wrong reply includes its debug text. It is reached by TYPE against a server that requires AUTH, or under an ACL that refuses the command. Fix: match `plain(reply)` and send the other cases to `failure("TYPE", other)`.
   - **F3** `std/protocol/resp.ignite:581`, note, INFEASIBLE, introduced. After HELLO 3 the server confirms SUBSCRIBE as a push. `pipeline` skips every push, so `command(client, ["SUBSCRIBE", ..])` waits until the idle timeout. No typed helper reaches this (`subscribe` reads its confirmation itself), so this is a doc line only.

5. Attacked and not broken:
   - Prefixes: every prefix of streamed strings, nested streamed aggregates, attributes inside arrays, RESP2 nulls, pushes and verbatim strings decodes as `Partial`.
   - Lengths: `$i64::MAX` is `BulkTooLarge`, `%2^62` is `TooManyElements`; `*i64::MIN`, `;-1` and `>-1` are refused as negative lengths.
   - Small values: `=4 txt:` decodes; `+\xff` decodes lossily.
   - Pipeline: pushes before, between and after split replies are skipped and the replies come back in order.
   - Sentinel: port 0 or 65536, an empty address, an integer port, a 3-item array and a map reply are all errors; a malformed `+switch-master` payload is ignored.
   - Subscriptions: the existing tests already cover unsubscribe with a message in flight, connection end and resubscribe, so I added nothing there.
   - Layering (by reading only, I did not run it): a name escapes only when its whole run of ASCII letters is `respond`, `response` or `respects`. `resp_x`, `RespRespond`, `resp2`, `responses` and `HttpResponse` are all still caught, and matching for the other protocols is unchanged. I found no leak.

6. Paths written outside the worktree, all under `/home/timo/.cache/scratch/2db1a4b7-2d2d-4a62-bab2-d71ac128826e/w10/resp/adversary/`: `case1-llvm.log`, `cases-alone.log`, `suite.log`, `suite-deselected.log`, `probe.ignite`, `probe.log`.

```findings
- file: std/protocol/resp.ignite
  line: 370
  category: boundary
  severity: blocker
  verdict: NEEDS-CHANGE
  origin: introduced
  message: the value an attribute annotates is decoded at the same depth, so a chain of attributes bypasses RespLimits.depth and an 800 KB reply under default limits overflows the stack on both targets
- file: std/protocol/resp.ignite
  line: 820
  category: contract-drift
  severity: warning
  verdict: CONFIRMED
  origin: introduced
  message: 'key_type turns the server''s error reply into "resp: unexpected reply to TYPE", against the slice.md contract that the server''s error line is the IoError message'
- file: std/protocol/resp.ignite
  line: 581
  category: judgement
  severity: note
  verdict: INFEASIBLE
  origin: introduced
  message: under RESP3 a SUBSCRIBE sent through command or pipeline gets its push confirmation skipped and waits for the idle timeout; no typed helper reaches this
```
