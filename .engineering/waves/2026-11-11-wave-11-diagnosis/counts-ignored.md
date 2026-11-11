# Row 7.2 "ignored tests": 24 at v0.15.0, 27 now

Tree: `<tree>`, HEAD `aa1defb2` (branch wave/10-tls).
`S` = `<scratch>/diag/counts`.
Nothing in the tree was edited, staged, built or tested. `git status --short` at the end: ` M AGENTS.md`, ` M Cargo.toml` (both were there before).

Sections marked **Verified** hold only what a command or a file:line showed. Sections marked **Inference** are my reading.

## Result in short

| Step | Count | Kind |
|---|---|---|
| now | 27 | |
| typer fix (patch `S/c-typer.patch`, tried in a copy) | 26 | real: the ignored case passes |
| un-ignore `conformance.rs:1484` (its reason is stale, the fix is in HEAD) | 25 | real: its assertions hold today |
| resp with a real server in the gate (option a, `valkey` package) | 24 | real, needs one package on the gate machine |
| nats with a real server in the gate (option a) | 23 | real, `nats-server` is not in the pacman repositories |
| option b for resp and nats instead of a | 23 on paper | mostly a relabel, see section 3 |

Recommendation: do the first two steps (25, no install, no new fixture). Leave resp and nats ignored and give them what `image::` has, a named task and a release step. Take 24 only through option a for resp (install `valkey`), not through option b.

## 1. The 27 lines

### Verified

Counter rule (`crates/ignite/tests/quality.rs:1236-1245`): every `.rs` under `crates/` and `tools/`, lines whose `trim_start()` starts with `#[ignore`.

Commands: `sh S/c-ignored-at.sh HEAD > S/c-ignored-head.txt` (27 lines), `sh S/c-ignored-at.sh v0.15.0 > S/c-ignored-v015.txt` (24 lines). The script lists files with `git ls-tree -r --name-only <rev> -- crates tools` and applies the same line rule to `git show <rev>:<file>`. `diff S/c-a.txt S/c-b.txt` (file and test name, sorted) shows exactly three additions and no removal. None of the 27 is under `tools/`.

`quality/baseline.json:1125-1131`: bar value 27 with a decision text for each addition. At v0.15.0 (`git show v0.15.0:quality/baseline.json`, line 965): value 24.

New since v0.15.0 (3):

| File:line | Test | Reason string |
|---|---|---|
| `crates/ignite-types/tests/adversary_http_app.rs:35` | `a_record_literal_before_the_argument_that_binds_its_parameter` | `story:typer-record-literal-deferred` |
| `crates/ignite/tests/nats.rs:248` | `a_real_server_on_both_targets` | needs nats-server on PATH; run with --ignored |
| `crates/ignite/tests/resp.rs:206` | `a_real_server_and_sentinel_on_both_targets` | needs redis-server and redis-sentinel on PATH; run with --ignored |

Already there at v0.15.0 (24):

| File:line | Test | Reason (string, or the doc comment where the attribute is bare) |
|---|---|---|
| `crates/ignite-check/tests/ownership_adversary.rs:106` | `map_put_of_a_handle_keeps_its_roots` | `story:map-put-handle-roots` |
| `crates/ignite-io/src/store.rs:796` | `contended_upserts_8_threads_1000_ids` | bare; load test of `docs/performance/store-lock/README.md` |
| `crates/ignite-llvm/tests/golden.rs:1447` | `adversary_an_unreferenced_constant_leaves_no_closure_behind` | known lower-then-prune deviation from 19.13 |
| `crates/ignite-rustgen/tests/slice9.rs:202` | `long_chains_build_and_run_100k` | bare; "minutes, not in the gate" |
| `crates/ignite-types/tests/builtin_tables.rs:190` | `every_type_checker_builtin_name_is_known_to_the_checker` | `story:checker-builtin-table-gaps` |
| `crates/ignite-types/tests/ownership_adversary2.rs:39` | `a_map_change_through_a_let_of_undeclared_type_is_e006` | `story:immutable-binding-e006` |
| `crates/ignite-types/tests/ownership_adversary2.rs:87` | `a_map_change_through_a_pattern_or_for_binding_of_undeclared_type_is_e006` | `story:immutable-binding-e006` |
| `crates/ignite-types/tests/ownership_adversary2.rs:128` | `a_ufcs_call_named_push_on_a_let_is_no_change` | `story:immutable-binding-e006` |
| `crates/ignite/tests/bitwise.rs:704` | `codecs_within_3x_of_hand_written_rust` | bare; builds the release runtime and times programs |
| `crates/ignite/tests/conformance.rs:1484` | `adversary_row5_a_program_conforming_to_a_std_catalogue_carries_none` | "fixed on branch fix/rustgen-builtin-shadowing (9bbd4b2 ...), not merged into s15/claims" |
| `crates/ignite/tests/conformance.rs:1599` | `adversary_http_an_obs_text_octet_reaches_the_application_as_one_octet` | gap RFC9112.R2_2_1; story http-rfc9112-9110-gaps |
| `crates/ignite/tests/control_trace.rs:223` | `a7_evaluator_steps_10000_emissions_into_a_30_day_window` | bare; a measurement, the gate runs the 1,000-step variant |
| `crates/ignite/tests/differential.rs:1179` | `the_evaluator_reports_a_failed_try_as_the_targets_do` | `story:eval-try-report` |
| `crates/ignite/tests/differential.rs:1207` | `the_evaluator_reads_a_receiver_before_its_arguments` | `story:eval-receiver-order` |
| `crates/ignite/tests/ignite_test.rs:491` | `store_values_under_updates_of_one_shard_on_both_targets` | `story:llvm-store-update-off-lock`: red on LLVM under load |
| `crates/ignite/tests/image.rs:80` | `the_image_builds_and_serves_the_todo_example_on_both_targets` | needs docker and IGNITE_IMAGE; run by `task image-check` |
| `crates/ignite/tests/llvm_build.rs:724` | `cargo_install_builds_without_a_runtime_flag` | cargo install of the workspace; run with --ignored |
| `crates/ignite/tests/llvm_build.rs:799` | `bench` | benchmark; run with task bench |
| `crates/ignite/tests/quality.rs:2204` | `record` | task quality runs it after the measurements |
| `crates/ignite/tests/rfc.rs:1430` | `adversary_prose_texts_are_verbatim_on_their_cited_pages` | needs IGNITE_RFC_TEXT_DIR with the unvendored RFC text files |
| `crates/ignite/tests/site/round.rs:1448` | `write` | run by task ux-round after the Playwright trial |
| `crates/ignite/tests/site/ux.rs:307` | `ratchet` | bare; reads `target/ux/metrics.json`, run by name by `task ux` |
| `crates/ignite/tests/slice9.rs:298` | `long_chains_100k_lower_quickly` | bare; run by hand |
| `crates/ignite/tests/slice9.rs:305` | `long_chains_100k_run_on_llvm` | bare; run by hand |

### Grouped by reason (grouping is mine, the "gate" column is inference except where a line is cited)

| Group | Tests | Could it run in the gate |
|---|---|---|
| A. Needs a program or data that is not in the repository (4) | `image::` (docker and a built image), `resp::` (redis-server, redis-sentinel), `nats::` (nats-server), `rfc::adversary_prose_texts...` (RFC text files, not vendored by decision D5) | Only where the program or files exist. `image::` no: it tests a release artifact. resp and nats: section 3. rfc: only if the three text files are provided to the gate. |
| B. A step of a task, wrong outside its sequence (4) | `quality::record` (`Taskfile.yml:76`), `site::ux::ratchet` (`:240`), `site::round::write` (`:253`), `llvm_build::bench` (`:172`) | `ratchet` already runs in every gate: `task gate` includes `task ux`, which runs it by name. The other three write results or measure; no. |
| C. Long, heavy or a timing bound (7) | `store.rs:796`, rustgen `slice9.rs:202`, `slice9.rs:298`, `slice9.rs:305`, `control_trace.rs:223`, `bitwise.rs:704`, `llvm_build.rs:724` | No by their own comments (minutes, or a timing bound on a loaded machine). Not re-measured: timing runs were excluded. |
| D. A known gap, asserted red until fixed (12) | `map-put-handle-roots` (1), `typer-record-literal-deferred` (1), `checker-builtin-table-gaps` (1), `immutable-binding-e006` (3), `eval-try-report` (1), `eval-receiver-order` (1), `llvm-store-update-off-lock` (1), `conformance.rs:1599` obs-text (1), `conformance.rs:1484` row 5 (1), `golden.rs:1447` (1) | Each one runs in the gate once its gap is closed. Two are closable now: the typer gap (section 4) and row 5, whose gap is already closed (section 5). |

## 2. The rule for `image::` and how the gate treats ignored tests

### Verified

- `Taskfile.yml:54-59`: `gate` = `check`, `test`, `ux`. `Taskfile.yml:25-29`: `test` = `cargo test --workspace`, then `test-ignite`. No `--ignored` and no `--include-ignored` anywhere in the gate, so an ignored test runs in the gate only when a task names it. The only such case inside the gate is `site::ux::ratchet` (`Taskfile.yml:240`, task `ux`).
- Tasks that run an ignored test by name: `quality::record` (`:76`), `image::the_image_builds...` (`:159`, task `image-check`), `llvm_build::bench` (`:172`), `site::ux::ratchet` (`:240`), `site::round::write` (`:253`).
- The `image::` rule is written in three places:
  - `crates/ignite/tests/image.rs` module doc: "Run by `task image-check IMAGE=... VERSION=...`; ignored otherwise, since it needs docker and a built image". The test reads `IGNITE_IMAGE` and `IGNITE_VERSION` with `.expect`.
  - `AGENTS.md:100`, Commands table: `task image-check VERSION=X.Y.Z [IMAGE=REF]`, "ignored elsewhere: needs docker".
  - `AGENTS.md` section Releases (about lines 503-511): after the archive upload, `task image VERSION`, then `task image-check VERSION`, then the docker push.
  So for `image::`: who = whoever cuts a release, when = at each release before the push, how = a named task with its env, what it needs = docker and the built image.
- resp and nats have none of this. `grep` for `resp`, `nats`, `redis` in `Taskfile.yml`, `.github/workflows/*.yml` and `dist/toolchain/Dockerfile` finds nothing. Their only instruction is the reason string "run with --ignored". The only workflow file is `.github/workflows/pages.yml`; no CI job runs tests.

### Inference

The baseline's phrase "ignored like image::" (`quality/baseline.json:1129`) is stronger than what exists: `image::` has a task and a fixed moment, resp and nats have neither, so today nothing ensures they are ever run.

## 3. `resp::a_real_server_and_sentinel_on_both_targets` and `nats::a_real_server_on_both_targets`

### Verified: what each does

Both tests already find and start their servers themselves; the `#[ignore]` only keeps a missing binary from failing the gate.

`crates/ignite/tests/resp.rs` (210 lines):
1. Takes four free ports (master, replica, sentinel, one left dead).
2. Spawns `redis-server` twice by name from PATH (`--port P --bind 127.0.0.1 --save "" --appendonly no`, the second with `--replicaof 127.0.0.1 <master>`), writes a `sentinel.conf` (monitor `resp-master`, quorum 1, down-after 1000 ms, failover-timeout 10000) and spawns `redis-sentinel`. A failed spawn panics with "start {program} (is it on PATH?)" (`resp.rs:55`); each port is awaited for up to 10 s; `Drop` kills the processes.
3. Writes one Ignite test program and runs `ignite test` on it for llvm, then rust, asserting exit 0 and "1 passed".
4. The program: connect over TCP; SET and GET; a pipeline of INCR, INCR, GET; HSET and HGETALL; expire and TTL; HELLO 3 and HGETALL again in RESP3 form; a second connection that subscribes, a PUBLISH that reaches it, unsubscribe; DEL; `master_at([dead, sentinel])` returns the master (the dead endpoint is skipped); a watch on the sentinel's `+switch-master`; `SENTINEL failover resp-master` retried until OK; `next_master` returns the replica; `master_at` returns the replica.

`crates/ignite/tests/nats.rs` (252 lines):
1. Takes a free port, spawns `nats-server -a 127.0.0.1 -p P --user app --pass secret` from PATH; panics "start nats-server (is it on PATH?)" (`nats.rs:67`); waits up to 10 s.
2. Writes one Ignite test program and runs it on llvm, then rust, asserting "1 passed".
3. The program: `connect_to` with user and password; reads the server's INFO (headers); subscriptions with `>` and `*`, two members of one queue group; publish with and without headers; exactly one queue member gets the job; request and reply through a spawned responder; the no-responders error text; a wrong password gives "nats: Authorization Violation"; then a TCP proxy written in Ignite cuts the connection, a stale and a fresh publish are held, the client reconnects, only the fresh one arrives, the subscription is restored, and the counters are stale 1, losses 1, connections 2, errors 0.

Both carry the comment that one test covers both targets "so the gate counts one ignored test".

### Verified: what the gate already covers without them

- `task test-ignite` (`Taskfile.yml:31-40`) runs every inline test of `std/` with `--target=all`, in every gate. `std/protocol/resp.ignite` has 23 inline tests, `std/protocol/nats.ignite` 37 (`grep -c '^test "'`).
- Those tests use scripted servers written in Ignite, over an in-memory pipe (`Pipe.pair()`, `resp.ignite:1172`, `nats.ignite:1816`) and also over real TCP sockets on 127.0.0.1: `resp.ignite:1679` "master_at asks sentinels over TCP in order"; `nats.ignite:2257` "connect_to reconnects over TCP by itself and subscribes again", `:2329` and `:2337` keepalive over TCP, and listeners at `:2589`, `:2613`, `:2704`, `:2838`.
- `resp.ignite:560` `connect` (TCP connect plus idle timeout) is called by no inline test (`grep -n '\bconnect(' std/protocol/resp.ignite` shows only its definition). The real-server test is its only caller under test.

### Inference: what only the real-server tests cover

1. That the scripted replies are what the real servers send. The scripts and the client have one author; a script can only confirm the client against that author's reading of the protocol. Real HELLO 3 and RESP3 map replies, the real `+switch-master` payload and a real failover, the real INFO, real queue-group distribution, the real no-responders reply and the real authorization error text are checked nowhere else.
2. `resp.connect` over TCP (verified above as untested elsewhere).
3. Wire encoding and transport on both targets is not unique to them; the inline tests cover it.

An earlier assumption of mine, that the gate never runs these clients over TCP, is wrong; the inline tests do.

### Options

**(a) Un-ignored, starts the server, fails when the binary is missing.**
- Verified: `command -v redis-server valkey-server nats-server` finds none on this machine; docker, clang and ld.lld are present.
- Verified: `pacman -Si redis` fails (no such package). `pacman -Si valkey`: repository extra, version 9.1.2-1, provides `redis`, 4.4 MiB installed. `pacman -Fl valkey` lists `usr/bin/redis-server` and `usr/bin/redis-sentinel`. So `pacman -S valkey` supplies both names `resp.rs` spawns. Nothing was installed.
- Verified: `pacman -Si nats-server` fails and `pacman -Ss '^nats'` returns nothing: it is not in the configured repositories.
- Verified: no CI runs tests and `dist/toolchain/Dockerfile` runs none, so the only place affected is each machine that runs `task gate`.
- Precedent in the repository for "required tool, fail when missing": `crates/ignite/tests/llvm_build.rs:31-39` `which()` panics "{tool} not on PATH" (clang).
- Inference: valkey answers the test's commands as redis does (not tried). The failover part (down-after 1000 ms, up to 120 tries of 500 ms) is timing sensitive; `quality/baseline.json:419` records another test that fails only at gate load 35 to 39, so a real failover in the loaded gate may flake. nats-server would come from the AUR or the upstream release binary; I verified neither.
- The code change is deleting the attribute. The gate is red on every machine without the binaries until they are installed, and `AGENTS.md` needs the prerequisite.

**(b) An in-process fake server in the Rust test, un-ignored on both targets; the real server only with an env switch such as `IGNITE_REAL_SERVERS=1`, failing when the binary is missing.**
- What the fake must speak (inference from the test programs and the command words in the clients; the exact frames were not traced line by line):
  - RESP: SET, GET, INCR, HSET, HGETALL, PEXPIRE, PTTL, DEL, HELLO 3 with the RESP2 to RESP3 switch per connection, SUBSCRIBE, UNSUBSCRIBE, PUBLISH across connections, and a sentinel role: `SENTINEL get-master-addr-by-name`, `SENTINEL failover`, SUBSCRIBE to `+switch-master` and the push after a failover. Estimate 300 to 450 lines of Rust.
  - NATS: INFO greeting, CONNECT with user and password and `-ERR 'Authorization Violation'`, PING and PONG, SUB with `*` and `>` and queue groups, UNSUB, PUB, HPUB, MSG and HMSG delivery, the no-responders reply. Estimate 250 to 400 lines. The reconnect part needs nothing new (the proxy is in the Ignite program).
- What the un-gated part would add over the inline tests: the same client against a server written from the same reading of the protocol, which the scripted servers already are; plus `resp.connect` over TCP, which one inline test in `std/protocol/resp.ignite` would cover in about ten lines. The property that makes these two tests worth having, a server nobody here wrote, stays behind the switch and off by default.
- So option b removes two counted lines while the real-server run stays exactly as optional as today. That is mostly a relabel, paid for with 550 to 850 lines of fixture that must track two protocols.

**(c) Leave as is.** Count stays; the reason is honest; but see section 2: no task and no fixed moment runs them.

**An env-gated early return alone** (drop `#[ignore]`, `if env::var_os(..).is_none() { return; }`) is a relabel: the same code runs in the same cases and the counter stops seeing it. The repository has two skip-by-return tests today (`llvm_build.rs:313-316`, `terminal.rs:963`), both decided by probing the host, not by an env switch; adding more of them for row 7.2 would lower the number without running anything.

### Existing conventions in `crates/ignite/tests` (verified by grep for `env::var`, `var_os`, `on PATH`, `skipped`)

- Required tool: panic when missing (`llvm_build.rs:31-39`, clang).
- Optional host feature: probe, print "skipped: ...", return (`llvm_build.rs:313-316`, `terminal.rs:963`).
- External input: `#[ignore]` plus an env variable read with `.expect` (`image.rs` `IGNITE_IMAGE`, `rfc.rs:1430-1434` `IGNITE_RFC_TEXT_DIR`).
- Env switches that change a test's mode, not whether it runs: `IGNITE_BLESS`, `IGNITE_DIFFERENTIAL_SLOW`, `IGNITE_DIFFERENTIAL_TRACE`.
- No `which`/`command -v` probe for redis or nats, and no existing "real servers" switch.

### Recommendation (inference)

Keep both ignored (c), and close the gap to the `image::` rule: one task (for example `task servers-check`, running the two tests by name with `--ignored`), a line in the `AGENTS.md` Commands table, and a step in the Releases section, so that "ignored like image::" becomes true. Add one inline test for `resp.connect` over TCP. If 24 is required, take option a for resp only: install `valkey`, delete the attribute, watch the failover part under gate load. Not option b.

## 4. The typer gap

### Verified

- Test: `crates/ignite-types/tests/adversary_http_app.rs:34-48`. `run<T, S>(boxed: Box<S, T>, stream: T)` called as `run(Box { state = "s" }, 7)`; `Box.touch` (the only field naming `T`) is defaulted.
- Code: `Typer::infer`, `crates/ignite-types/src/typer.rs:573-646`. Arguments are typed in two passes (`:592-601`): pass 2 holds arguments whose declared type is a fn type or which `needs_expected` (`:216`). A record literal is in pass 1 and is typed where it stands, against the parameter "as far as it is known" (`:613-618`). Its own `infer` (through `fields_literal`, `crates/ignite-types/src/typer/system.rs:685-768`) can bind `T` only from a given field or from the expected type, and `unify` (`typer.rs:150-161`) binds nothing from an expected type that still holds a variable. So with the stream after the literal the result is E042 for `T` of `Box`.
- Story `.engineering/planning/story/typer-record-literal-deferred.md`: status draft; acceptance: the ignored case passes, and a literal whose parameter no argument binds still gives E042.
- `docs/slice.md:10884-10885` states the current rule ("as far as the arguments before it bind it").

### The fix that was tried

In `Typer::infer`: the two-pass `deferred: Vec<bool>` becomes a three-pass `stage: Vec<u8>` (0 in order, 1 a record literal that waited, 2 the old second pass). In pass 0, a record literal whose parameter is still open is asked `literal_waits`; if yes it moves to pass 1. New method `Typer::literal_waits(arg, param)`: true when one of the literal's own type parameters is left open by `param` and is mentioned by no given field's declared type (a field given a nested literal that itself waits counts as not binding).

Why not simply "defer every record literal": a plain argument after the literal can depend on what the literal binds (it is `check`ed against a closed parameter instead of synthesised), and a closure before it in argument order would be typed first. With `literal_waits`, a literal waits only where the old code was certain to report E042 at that literal, so a call that typed before is typed in the same order as before (argued, not proven beyond the tests below).

Size: `typer.rs` +45 −4; `adversary_http_app.rs` +69 −1 (the attribute removed, four cases added). Patch: `S/c-typer.patch`; `git apply --check` against HEAD passes (run without applying).

### Experiment (verified)

Copy of HEAD by `git archive` into `S/exp-c`, `CARGO_TARGET_DIR=S/exp-c-target cargo test --offline -p ignite-types --no-fail-fast`:

| Run | passed | failed | ignored | Log |
|---|---|---|---|---|
| before | 106 | 0 | 5 | `S/c-typer-before.log` |
| after | 111 | 0 | 4 | `S/c-typer-after.log` |

- 111 = 106 + the un-ignored case + four added cases: a parameter no argument binds is still E042 naming `T` of `Box`; a nested literal before the binding argument types; a closure after a waiting literal still sees what the literal binds; a later argument that conflicts with the literal's field is E040.
- No other test or fixture of `ignite-types` changed: every other file under `crates/ignite-types` in the copy is byte-identical to HEAD (hash comparison in `S/c-typer-patch.sh`).
- `cargo fmt -p ignite-types -- --check` in the copy: exit 0. Clippy was not run.
- `S/exp-c` and `S/exp-c-target` are removed.

### Risks (inference)

- Only `ignite-types` was built and tested. std, the examples, the rustgen and LLVM goldens and `task test-ignite` were not run with the change.
- Programs that were already wrong can get a different diagnostic: where a later argument now binds the parameter, E042 at the literal becomes either no error or E040 at the literal. A fixture elsewhere that expects that E042 would change.
- The rule is syntactic: a given field that mentions the parameter but cannot bind it (its value is `None`) does not make the literal wait, so that call stays E042 as today.
- To land it: the story must be accepted (it is draft), `docs/slice.md:10884-10885` and the doc comment of `infer` need the new sentence, and the baseline 7.2 decision text loses its typer clause. Work in the tree: apply the patch, `task gate`.

## 5. Counts and what goes below 24

### Verified

- `conformance.rs:1484`: its reason says the fix is on a branch that is not merged. `git merge-base --is-ancestor 9bbd4b2 HEAD` exits 0; `git log -1 9bbd4b2`: "fix(rustgen): a clause-only catalogue import emits none of its constants". The reason is stale.
- Its assertions hold today. `S/c-row5.sh` replays `assert_carries_no_catalogue` (`conformance.rs:1489-1550`) with the binary already built in the tree (`target/debug/ignite`, built after the HEAD commit) and output under `S/c-row5`: `check` exit 0 with empty stderr, `emit` exit 0 and non-empty, `build --target llvm` exit 0, a `std.ll` exists, 0 hits for the five digests and four requirement texts in the emitted Rust and both `.ll` files. The needles are right: the same digests are found in `std/rfc/rfc854.ignite`, `rfc855.ignite`, `rfc1143.ignite`. The test itself was not run (that needs a build of the `ignite` crate).
- `conformance.rs:1599`: still a gap. `std/protocol/http.ignite:152` declares `RFC9112.R2_2_1` unsupported. The story it cites, `http-rfc9112-9110-gaps`, is `implemented` and moved R2_2_1 out to `http-octet-head-and-date` (story file line 62), which is draft. The reason holds; the story name in the attribute is stale.
- Story status of the other reasons (`grep -m1 '^status:' .engineering/planning/story/<id>.md`): `map-put-handle-roots`, `checker-builtin-table-gaps`, `immutable-binding-e006`, `eval-try-report`, `eval-receiver-order`, `llvm-store-update-off-lock`, `typer-record-literal-deferred`: all draft.

### Counts per option

| Option | Count |
|---|---|
| typer fix alone | 27 -> 26 |
| typer fix + un-ignore row 5 (`conformance.rs:1484`) | 25 |
| + resp by option a | 24 |
| + nats by option a | 23 |
| typer fix + option b for both (the parent's 24) | 24, or 23 with row 5; two of the removed lines are a relabel |

### Further candidates below 24 (inference)

- `checker-builtin-table-gaps` (1 line): the test wants four names in `ignite_check::BUILTIN_METHODS`. Few lines, but that table decides name resolution in the checker (`crates/ignite-check/src/lib.rs:2444`) and the typer (`typer.rs:3457`, `:3538`), so it is a behaviour change, not a table edit. Not tried.
- `immutable-binding-e006` (3 lines), `eval-try-report`, `eval-receiver-order`, `map-put-handle-roots`, `llvm-store-update-off-lock` (1 each): each needs its story implemented; none looked at in depth.
- `golden.rs:1447`: a stated deviation with no story; not checked whether it still holds.
- Group C (7 lines) and group B (4 lines) should stay ignored; un-ignoring them means either timing bounds in a loaded gate or a relabel.

## Files

- `S/c-ignored.md` (this file)
- `S/c-ignored-at.sh`, `S/c-ignored-head.txt`, `S/c-ignored-v015.txt`, `S/c-a.txt`, `S/c-b.txt`
- `S/c-typer.patch`, `S/c-typer-before.log`, `S/c-typer-after.log`, `S/c-typer-fmt.log`, `S/c-typer-run.sh`, `S/c-typer-patch.sh`, `S/c-typer-extra-tests.rs`
- `S/c-row5.sh`, `S/c-row5/` (emit output and two `.ll` files)
