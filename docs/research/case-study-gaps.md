# Ignite gaps found while building a WebSocket push service (the case study)

- Version: Ignite v0.13.0, commit 33b591237303d213842d57dd6b1f1a7fe2f100aa, installed from source
  with `cargo install --locked --path crates/ignite`.
- Target: llvm (the default) unless an entry says otherwise.
- Reproducers: `docs/research/case-study-gaps/` (R below).
- Paths without a prefix are in the ignite repository.
- No GitHub issues were filed.

## G1 No peer address on a TcpStream (blocking)

- Kind: missing-feature/ergonomic.
- Reproducer: docs/research/case-study-gaps/G1.ignite.
- The service needs: the `/push` allow list (127.0.0.0/32, 172.16.0.0/12, 10.0.0.0/8) is decided by the
  connection's peer address, and the 403 body names it (`ip`). Conformance `Publish/grant/denied`
  sends from an outside network and through a loopback relay.
- Expected: `stream.peer_address()` (or an `(address, port)` pair) gives the remote address.
- Actual (`ignite check`): `G1.ignite:10:14: error[E041]: `TcpStream` has no method `peer_address``.
- Evidence: `crates/ignite-io/src/net.rs`, `TcpListener::accept`, drops `_peer` from
  `listener.accept()`.
- Workaround: read `/proc/self/net/tcp` through a `Directory` resource after each accept on the
  ingress port. The socket inode that is new since the previous snapshot gives the remote
  address. This costs O(open sockets) per accept, is Linux only, and fails closed (403) when the
  diff is ambiguous.
- Blocks: correct and cheap behaviour, yes.

## G2 No environment variables

- Kind: missing-feature/ergonomic.
- Reproducer: docs/research/case-study-gaps/G2.ignite.
- The service needs: SESSION_STORE, NATS_URL, REPLICA_ID, REDIS_*,
  AUTH_BACKEND_DEFAULT_URL (12-factor configuration under Kubernetes).
- Expected: a way to read an environment variable, for example an `Environment` resource.
- Actual: `G2.ignite:8:7: error[E003]: unknown name `Env``.
- Evidence: `docs/slice.md` §21.3 and §21.6 offer `Setting` (flags only), nothing for the
  environment.
- Workaround: `resource procs: Directory { path = "/proc/self" }`, then
  `procs.read_file("environ")`, parsed as NUL-separated `K=V` pairs.
- Blocks: no.

## G3 No sleep, timer, condition variable or channel

- Kind: missing-feature/ergonomic.
- Reproducer: docs/research/case-study-gaps/G3.ignite.
- The service needs:
  - hold a long-poll connect up to its deadline and wake it when a message arrives;
  - heartbeat every 5 s;
  - WebSocket ping every 10 s;
  - a drain window;
  - client expiry.
- Expected: `Clock.sleep(d)`, plus a waitable signal or channel with a timeout.
- Actual: `G3.ignite:8:9: error[E003]: `Clock` has no static function `sleep``.
- Evidence: `docs/actors.md` says actors and channels are not implemented.
- Workaround: "bells". A loopback TCP pair goes through an internal listener on 127.0.0.1:0. The
  waiter reads its end with `set_idle_timeout(ms)`, and the ringer writes one byte to the
  accepted end, which is kept in a `Store<TcpStream>`. A sleep is a read on a bell nobody rings.
  This costs two fds and a loopback connect per waiter.
- Blocks: no (costly).
- Closed in wave 7 by story `channels-and-timers` (`docs/slice.md` 30): bounded channels with `receive_within` wake a waiter on a message or a deadline; the sleep part was closed by the process builtins story (21.20).
- Confirmed closed by the case study on v0.15.0 (2026-11-08); a leak it then found in channels is G30.

## G4 An idle timeout reads as Eof, so it cannot be told from the peer closing

- Kind: missing-feature/ergonomic.
- Reproducer: docs/research/case-study-gaps/G4.ignite (built and run).
- Expected: a timed-out read is distinguishable, for example `Input.Timeout`.
- Actual output:

  ```
  after 205 ms: Eof
  peer closed: Eof
  ```

- Evidence: `crates/ignite-io/src/net.rs:388` maps WouldBlock and TimedOut to `Ok(0)`, which is
  `Eof`.
- Workaround: time the read; an `Eof` that arrives clearly before the timeout is a close.
- Blocks: no.

## G5 No connect timeout

- Kind: missing-feature/ergonomic.
- Reproducer: docs/research/case-study-gaps/G5.ignite (connects to the non-routable 10.255.255.1:80; not run here, it waits
  for the kernel's SYN retries, about 2 min).
- The service needs: `/push/auth` gives up after 10 s, and the Redis store after 1 s, including connect.
- Expected: `TcpStream.connect` with a timeout.
- Actual: `TcpStream::connect` calls `std::net::TcpStream::connect` with no timeout
  (`crates/ignite-io/src/net.rs`, `TcpStream::connect`, around line 359).
- Workaround: none for connect; read timeouts cover a peer that accepts and never answers.
- Blocks: partially (timing against an unreachable host).

## G6 The SIGTERM drain releases every listener, internal ones included

- Kind: missing-feature/ergonomic.
- Reproducer: docs/research/case-study-gaps/G6.ignite (built and run, SIGTERM sent).
- The service needs: after SIGTERM, keep answering `/readyz` with 503 on the admin port, keep the
  internal loopback listener (G3) working, and answer held connects over a drain window.
- Expected: a way to keep a listener through the drain.
- Actual output:

  ```
  ingress accept: draining
  internal accept: draining
  released 127.0.0.1:45815
  released 127.0.0.1:44539
  ```

- Evidence: `crates/ignite-io/src/drain.rs`, `begin()` and `release_all()`.
- Workaround: on "draining", `TcpListener.bind` the admin, ingress and bell ports again. A listener
  bound after the drain began is not released.
- Blocks: no.
- Closed in wave 7 by story `drain-keeps-listeners` (`docs/slice.md` 16.13): a `TcpListener` with `keep_through_drain = true` keeps accepting through the drain.
- Partly closed, per the case study on v0.15.0 (2026-11-08): it works for a listener resource with a fixed port; a listener bound at run time is still released (G31).

## G7 No bitwise operators

- Kind: missing-feature/ergonomic.
- Reproducer: docs/research/case-study-gaps/G7.ignite.
- The service needs: SHA-1 and base64 for the WebSocket accept key, XOR masking of frames, and frame
  header bits.
- Actual: `G7.ignite:4:16: error[E001]: unexpected character `^``.
- Evidence: `site/book/src/reference/grammar.md`, the operator table (about line 260), lists no
  `& | ^ << >>`.
- Workaround: bit-by-bit arithmetic over Int, in a hand-written module.
- Blocks: no (slow per frame).
- Closed in wave 7 by story `bitwise-operators` (`docs/slice.md` 29).
- Confirmed closed by the case study on v0.15.0 (2026-11-08).

## G8 No wall clock

- Kind: missing-feature/ergonomic.
- Reproducer: docs/research/case-study-gaps/G8.ignite.
- The service needs: `/heartbeat` `{time, date}` in epoch ms and as a JS Date string.
- Actual: `G8.ignite:8:13: error[E003]: `Clock` has no static function `now``.
- Workaround: `btime` from /proc/stat plus /proc/uptime at start, then `Clock.monotonic()`.
  Accurate to about 1 s.
- Blocks: no.

## G9 No YAML (or regex) in std

- Kind: missing-feature/ergonomic.
- Reproducer: docs/research/case-study-gaps/G9.ignite.
- The service needs: the rule set in its YAML configuration, and url-pattern matching.
- Actual: `G9.ignite:3:8: error[E031]: unknown module `std.yaml``.
- Workaround: the rules are embedded as Ignite data, with a hand-written url-pattern matcher.
- Blocks: no. Related: there is no WebSocket, NATS or Redis/Sentinel client in std. These are
  written in Ignite over TcpStream (RFC 6455 framing, RESP, the NATS text protocol).

## G10 No setrlimit (open-files limit)

- Kind: missing-feature/ergonomic.
- Reproducer: docs/research/case-study-gaps/G10.ignite.
- The service needs: raise RLIMIT_NOFILE to the hard limit at start, as the reference implementation does.
- Actual: `G10.ignite:8:11: error[E003]: `Process` has no static function `set_open_files_limit``.
- Workaround: none; the container default applies.
- Blocks: no (it caps connections under the benchmark).
- Closed in wave 7 by story `open-files-limit` (`docs/slice.md` 28): every program raises its open-files soft limit to the hard limit at start.
- Confirmed by the case study on v0.15.0 (2026-11-08): works as needed.

## G11 No FFI

- Kind: missing-feature/ergonomic.
- No foreign-function mechanism exists, so none of these gaps can be closed with a thin binding.
- Reproducer: none (absence).

## G12 No ESS generator target for Ignite

- Kind: missing-feature/ergonomic.
- This is a gap in the ESS tooling, not in Ignite: `ess generate` has no Ignite target, so all code
  is hand-written.
- Reproducer: none.

## G13 No explicit flush from Ignite

- Kind: missing-feature/ergonomic.
- Reproducer: docs/research/case-study-gaps/G13.ignite.
- Expected: `flush()`. `ignite_io::flush()` exists (`docs/slice.md` §7) but is not a builtin.
- Actual: `G13.ignite:4:3: error[E003]: unknown name `flush``.
- Workaround: none needed so far. A task that rings a bell or writes to another task's socket
  relies on its own next wait or its end to send the bytes.
- Blocks: no.

## G14 A user fn named like a builtin List method is refused even for plain calls

- Kind: refusal-of-valid-program (minor).
- Reproducer: docs/research/case-study-gaps/G14.ignite.
- Expected: `set(o, k, v)`, called as a plain function, compiles.
- Actual: `G14.ignite:3:4: error[E041]: fn `set` takes a `List<(Text, Text)>` first, and
  `List<(Text, Text)>` has the builtin method `set`, so `x.set(..)` could never call it; rename it`.
- Workaround: renamed the fn.
- Blocks: no.

## G15 A record-literal binding needs a type annotation before its Store fields are used

- Version: ignite 0.13.0 (33b591237303d213842d57dd6b1f1a7fe2f100aa). Target: llvm.
- Kind: refusal-of-valid-program (minor).
- What the service needs: `let w = World { ... }` in `main`, then `w.numbers.put(..)` on the Store fields.
- Reproducer: docs/research/case-study-gaps/G15.ignite.
- Expected: the type of `w` is inferred from the record literal `World { ... }` and the effects of `put` and `get` are checked against it; the test passes.
- Actual (`ignite test G15.ignite`):
  `G15.ignite:13:3: error[E042]: cannot check the effects of `put`: the type of its receiver is not declared; annotate the binding`
  `G15.ignite:14:10: error[E042]: cannot check the effects of `get`: the type of its receiver is not declared; annotate the binding`
- Workaround: `let w: World = World { ... }`.
- Blocks: no.

## G16 Process.exit during the SIGTERM drain waits for every live task and ignores its code

- Version: ignite 0.13.0 (33b591237303d213842d57dd6b1f1a7fe2f100aa). Target: llvm.
- Kind: wrong-result.
- What the service needs: to end the process at the end of its drain window, while tasks still wait in `accept` on listeners bound after the drain began, in sleeps and in idle client reads.
- What Ignite does: `Process.exit(code)` lowers to `ignite_rt_exit` (`crates/ignite-llvm/src/lower.rs:4607`). While draining, that function calls `drained()`, which runs `wait_for_tasks()` with no bound and then `exit(0)` (`crates/ignite-rt/src/process.rs:307-348`). `ignite_io::exit` says the opposite: "Other tasks and pending reads are not waited for" (`crates/ignite-io/src/lib.rs:171-182`).
- Reproducer: docs/research/case-study-gaps/G16.ignite. Build it with `ignite build`, start `g16`, connect nothing, then send SIGTERM.
- Expected: the process exits with status 3 right after `calling Process.exit(3)`.
- Actual: the log shows `drain began`, `released 127.0.0.1:<port>` twice and `calling Process.exit(3)`. The process is still running 3 s later and stays up until SIGKILL. In the case study, the service hung after `push server stopped`.
- Workaround (the service's `stop` and `main`) sets a STOPPING flag, then makes every task end:
  - accept loops: one connection to each listener;
  - sleepers: a ring on every bell;
  - the NATS reader: a PING, so it gets a PONG;
  - client connections: their write half is closed;
  - keep-alive reads: an idle timeout of 5 s, as the reference implementation uses.
  Then `main` returns `Ok`. A SIGTERM with idle clients now exits 0 within about 4 s with a 1 s window.
- Blocks: it blocked shutdown. Without the workaround the process never exits after SIGTERM.


## G17 No formatter and no format check

- Version: ignite 0.13.0. Target: both.
- Kind: missing-tool.
- What the service needs: a format gate, as every other implementation of the case study has (`gofmt`, `prettier`).
- What Ignite does: there is no `ignite fmt` and no `--check` mode; the service's `task fmt` is a no-op that prints this.
- Blocks: no.

## G18 `IoError` has no `==`, not even inside `assert`

- Version: ignite 0.13.0. Target: both.
- Kind: missing-feature.
- What the service needs: to unit-test that parsing a bad port gives the expected error.
- What Ignite does: `assert port("x") == Err(IoError.new("bad port"))` is E045 ("operator `==` is not defined for `Result<Int, IoError>`, not even in an `assert`"), as `docs/slice.md` 12.3 states for any type holding an `IoError`.
- Not a gap: `Result.is_err` exists as a UFCS helper in `std.result` (`std/result.ignite:30`); it needs `import std.result.*` (friction row 6). `assert port("x").is_err()` checks.
- Workaround in the service: the parser returns its errors as `Text` instead of `IoError`. Inside Ignite: compare `error.message` (`docs/slice.md` 4.4).
- Blocks: no.

- Closed in wave 7 by story `ioerror-assert-equality` (`docs/slice.md` 12.3): two `IoError`s compare equal inside `assert` when their messages are.
- Confirmed closed by the case study on v0.15.0 (2026-11-08).

## G19 No HTTP client; the server role does not fit the service

- Version: ignite 0.13.0. Target: both. Hand-written in the service: 738 lines (request parsing, chunked bodies, keep-alive, URL parsing, a client for the auth backend call).
- Kind: missing-feature.
- What std has: the server role `Http.serve` in `std/protocol/http.ignite` reads requests with `Content-Length` or chunked bodies (RFC 9112 6.1, 6.3), keeps connections alive and streams chunked responses (`docs/slice.md` 19.23).
- What is missing: a client role (requests, response bodies, keep-alive, timeouts), a URL type, and a server hook to take over the connection (the WebSocket upgrade, a long-poll held past the handler).
- Stories: `story:http-client-role` (client and URL), `story:std-websocket` (the upgrade hook).
- Blocks: no.

## G20 JSON was hand-written although std.json exists

- Version: ignite 0.13.0. Hand-written in the service: 389 lines.
- Kind: not a gap of std. `std.json` (`std/json.ignite`) has `Json`, `parse` and `encode` (RFC 8259, `docs/slice.md` 19.20) since before 0.13.0.
- Since wave 7 (`story:data-value-model`): `read`, `write`, `write_pretty`, `to_data`, `of_data` over the `std.data` value model; typed codecs follow in `story:data-codec-combinators` (wave 8) and the derived `codec<T>()` later (`docs/serde.md`).
- Blocks: no.

## G21 No WebSocket server

- Version: ignite 0.13.0. Hand-written in the service: about 121 lines plus parts of the server module (handshake, framing, masking, ping/pong, close echo).
- Kind: missing-feature.
- Story: `story:std-websocket` (wave 8) covers the server and the client role, the upgrade through `std.protocol.http`, control frames and the close handshake.
- Blocks: no.

## G22 No SHA-1 and no base64

- Version: ignite 0.13.0. Hand-written in the service with bit arithmetic: part of a 288-line module that also works around G7.
- Kind: missing-feature.
- Since wave 7: bitwise operators (`story:bitwise-operators`); `std.data` has a `base64` helper (`story:data-value-model`).
- Story: `story:std-digest` (wave 8): SHA-1 and SHA-256, base64 and base64url as std modules with the RFC test vectors.
- Blocks: no.

## G23 No NATS client

- Version: ignite 0.13.0. Hand-written in the service: 163 lines (CONNECT, PUB, SUB, PING/PONG, reconnect with a bounded buffer and drop-on-stale).
- Kind: missing-feature.
- Story: `story:std-nats-client` (wave 8), with the bounded reconnect buffer and drop-on-stale.
- Blocks: no.

## G24 No Redis client

- Version: ignite 0.13.0. Hand-written in the service: 442 lines (RESP2/3, pipelining, Sentinel master discovery and failover).
- Kind: missing-feature.
- Story: `story:std-resp-client` (wave 8).
- Blocks: no.

## G25 No TLS client

- Version: ignite 0.13.0. Target: both.
- Kind: missing-feature.
- What the service needs: reach an https auth backend or a broker behind TLS; today it refuses https backend URLs at start.
- Rule: the TLS engine (rustls) may live in `crates/ignite-io`, the `Tls<T>` layer is Ignite (AGENTS.md, Timo 2026-11-01).
- Story: `story:tls-client` (wave 8).
- Blocks: yes, for an https backend.


## G26 A compiler built from a commit reports the release version

- Version: ignite main `d02a39b2`. Target: n/a.
- Kind: missing-feature/tooling.
- What happens: `ignite --version` prints `0.13.0` with no commit, so a build from source cannot be told apart from the release.
- Story: `story:version-names-commit`.
- Blocks: no.

## G27 A static build from source needs the musl target and the network

- Version: ignite main `d02a39b2`. Target: both, `--static`.
- Kind: missing-feature/tooling.
- What happens: a compiler built from source needs `rustup target add x86_64-unknown-linux-musl` and cargo network access at the first `--static` build; the release binary does not (`docs/slice.md` 23).
- Story: `story:static-build-from-source`.
- Blocks: no.

## G28 `Http.serve` cannot host a WebSocket or long-poll server

- Version: ignite main `d02a39b2`. Target: both.
- Kind: missing-feature.
- What the service needs: the peer address for an IP allow list, a takeover of the connection for an upgrade, a request held until a message or a deadline, and its own answer to a too-large body (a 403 naming the caller, or its own 413 with keep-alive and the body discarded).
- What Ignite does: the `HttpApp` handler (`fn(state, request) -> Response requires [state.*]`) never sees the connection, cannot wait (a hold is E017), and std answers a too-large body with its own 413 before the application runs.
- Workaround: the service keeps its hand-written HTTP server; the server role of G19 does not cover this.
- Story: `story:http-app-connection`; `story:std-websocket` builds its upgrade on it.
- Blocks: no (with the workaround).

## G29 `std.json` `get` returns the first of duplicated object keys

- Version: ignite main `d02a39b2`. Target: both.
- Kind: interoperability.
- What happens: for `{"channel":"/a","channel":"/meta/handshake"}`, `get` returns `/a`; ECMAScript's `JSON.parse` keeps the last member, so a server interoperating with JavaScript clients must use `/meta/handshake`. RFC 8259 section 4 leaves the choice open.
- Story: `story:json-duplicate-keys` (last member wins, documented, plus an accessor for every member).
- Blocks: no.

## G30 A closed channel's memory is never freed

- Version: ignite 0.15.0. Target: as reported, not stated.
- Kind: leak.
- What happens: a spawned task running `let (r, b) = Channel.bounded(1); r.close(); b.receive_within(0 ms)` grows the resident size by about 370 B per task; 200,000 such tasks take it from 14 MB to 88 MB.
- Minimal example, spawned 200,000 times by a `main` that waits for each `done`:

  ```
  fn poll(done: Sender<Int>) -> Result<Unit, IoError> {
    let (r, b) = Channel.bounded(1)
    r.close()
    let none: Option<Int> = b.receive_within(0 ms)
    done.send(1)
    Ok(())
  }
  ```

- Not reproduced on `main` (05574a7c8, no runtime change since v0.15.0), 2026-11-08: that example spawned one at a time and 1,000 at once, and the three lines looped 200,000 times inside one task, peak at 2.0 to 6.5 MB on both targets, dev, `--release` and `--static`, the same as the control without a channel. The leak needs something the reduced form does not have; the service's own shape is the next reproducer.
- Cause (the case study's reproducer, 2026-11-08, LLVM target): a channel end's handle-table entry was released only while the end stayed in a plain `let (tx, rx) = Channel.bounded(n)` binding used through methods. Four forms left it unowned, about 490 B per end and per task: `alias` (`let r: Receiver<Int> = rx`), `typed` (`let ends: (Sender<Int>, Receiver<Int>) = Channel.bounded(1)`), `field` (the receiver in a record field), `server` (the sender in a `Store<Sender<Int>>`, the receiver returned in a record, the sender removed and closed). The Rust target drops by Rust ownership and never leaked. G32's annotation workaround is the `alias` form.
- Fixed (`story:channel-release-on-close`, `story:llvm-release-all-handles`): on the LLVM target a block owns every handle of a binding whose value no one else owns, through aliases, fields, tuples and stores; heap copies in stores and channels and tasks spawned with a record are owners of their own (`docs/slice.md` 30.3). Tests: the soaks "a channel end bound a second time is released (G30)", "a channel end kept in a record field is released (G30)", "an annotated pair of channel ends is released (G30)" and "a channel end put in a store and taken out is released (G30)" in `std/sync.ignite` (2,000 runs each, both targets); `leak::g30_reproducer_stays_flat_in_every_mode` (`crates/ignite/tests/fixtures/leak/repro.ignite`, every mode at `N=20000` within 2 MB of `none` on both targets); `leak::every_soak_passes_without_a_seeded_leak` (the listener, stream, child, directory and capability soaks, both targets).
- Workaround in the service: closed channels go on an idle list and are reused.
- Code: `crates/ignite-io/src/channel.rs` (`Shared` behind the two ends' `Arc<End>`, freed when both ends' handles are dropped); `crates/ignite-rt/src/channel.rs` (`ignite_rt_channel_bounded` registers each end as `Handle::Sender` or `Handle::Receiver`); the handle table in `crates/ignite-rt/src/net.rs` (`register`, `release_owner`, the task's `OWNED` list with its tombstones, `ignite_rt_own_release`; segments are never freed, slots are reused through `FREE`); the LLVM target's ownership of ends in `crates/ignite-llvm/src/lower.rs` (`own_streams`, `own_value`, `release_owned`, `spawn_with`), `own.rs` and the handle glue in `crates/ignite-llvm/src/glue.rs` (`own_handles`, `retain_handles`, `release_handles`); `docs/slice.md` 30.3.
- Story: `story:channel-release-on-close`.
- Blocks: no (with the workaround).

## G31 A listener bound at run time cannot be kept through the drain

- Version: ignite 0.15.0. Target: both.
- Kind: missing-feature.
- What the service needs: its admin and internal ports come from the environment or configuration, and those listeners must keep accepting through the SIGTERM drain (G6).
- What Ignite does: `keep_through_drain = true` is a property of a `TcpListener` resource whose port is fixed in the system (the Rust target needs a literal, E020; `docs/slice.md` 16.13). `TcpListener.bind(address, port)` has no such form, so its listener is released at the drain.
- Minimal example: `let admin = TcpListener.bind("0.0.0.0", port_from_env)?` in `main`; after SIGTERM, `admin.accept()` fails with `draining`.
- Workaround: on `draining`, bind the port again (G6's workaround); a listener bound after the drain began is not released.
- Code: `crates/ignite-io/src/net.rs` (`TcpListener::bind`, and `bind_kept`/`bind_with`, which exist but only the resource path reaches), `crates/ignite-io/src/drain.rs` (a kept listener is never registered); `crates/ignite-rt/src/net.rs` (`ignite_rt_tcp_bind`, `ignite_rt_tcp_bind_kept`); the builtin `TcpListenerBind` (`crates/ignite-check/src/lib.rs` tables, `crates/ignite-llvm/src/lower.rs`, `crates/ignite-rustgen/src/lib.rs`); the resource property in `crates/ignite-types/src/typer/system.rs`, `crates/ignite-llvm/src/lib.rs` and `crates/ignite-rustgen/src/lib.rs`.
- Story: `story:drain-keeps-bound-listeners`.
- Blocks: no (with the workaround).

## G32 A channel end from a Store lookup or a returned record needs a type annotation

- Version: ignite 0.15.0, reproduced on `main` 05574a7c8. Target: both (a frontend error).
- Kind: refusal-of-valid-program.
- What happens: both are `error[E042]: cannot check the effects of `try_send`: the type of its receiver is not declared; annotate the binding`:

  ```
  fn from_store(senders: Store<Sender<Int>>, id: Int) -> Bool requires [senders.read] {
    match senders.get(id) {
      Some(tx) => tx.try_send(1)
      None => false
    }
  }

  type Ends { tx: Sender<Int>, rx: Receiver<Int> }
  fn ends() -> Ends { let (tx, rx) = Channel.bounded(1); Ends { tx = tx, rx = rx } }
  fn from_record() -> Bool {
    let d = ends()
    d.tx.try_send(1)
  }
  ```

- Workaround: `let tx: Sender<Int> = …` inside the arm, `let d: Ends = ends()`.
- Code: the effect check takes a receiver's type from declarations alone (`docs/slice.md` 11.7): `static_type` in `crates/ignite-check/src/lib.rs` resolves no call of a declared fn (`ExprKind::Call` covers only `Owner.constructor(..)`), and `pattern` binds a `PatternKind::Binding` without a type from the scrutinee (`Option<T>` of `Store.get`). The type checker reports the missing effect site: `effect_site` and `unchecked_effects` in `crates/ignite-types/src/typer.rs`. Same class as G15.
- Story: `story:infer-channel-ends`.
- Blocks: no (with the workaround).

## G20 (correction)

- The service used `std.json` already; only JavaScript-compatible truthiness and number formatting sit on top of it.

## G13 (update) Missing flush hit in the stop path

- `close()` sends only its own stream (`crates/ignite-io/src/net.rs:438`). A write made just before `main` returns is never sent while draining, because `ignite_rt_exit` waits for tasks without flushing first.
- The NATS PING of the stop is written before the connects of `stop` (`TcpStream.connect` flushes, `net.rs:358`). Order matters, and there is nothing to call instead.
- Blocks: no, with the workaround.
