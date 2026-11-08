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

## G13 (update) Missing flush hit in the stop path

- `close()` sends only its own stream (`crates/ignite-io/src/net.rs:438`). A write made just before `main` returns is never sent while draining, because `ignite_rt_exit` waits for tasks without flushing first.
- The NATS PING of the stop is written before the connects of `stop` (`TcpStream.connect` flushes, `net.rs:358`). Order matters, and there is nothing to call instead.
- Blocks: no, with the workaround.
