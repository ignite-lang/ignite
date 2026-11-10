//! `std.protocol.nats` against a real `nats-server` (`docs/slice.md`, the
//! NATS client section): one server on a free loopback port with a user
//! and a password, then an Ignite test that connects with them, subscribes
//! with wildcards and a queue group, publishes with and without headers,
//! sends a request that a task of its own answers and one that nobody
//! answers, is refused with a wrong password, and reconnects through a
//! proxy of its own that cuts the connection: a message held longer than
//! a second is dropped, a fresh one is sent. Once per target. Ignored by
//! default, since the server is not installed where the gate runs:
//! `cargo test -p ignite --test main nats:: -- --ignored` runs it where
//! `nats-server` is on `PATH`. The inline tests of
//! `std/protocol/nats.ignite` cover the same client over a scripted server
//! in every gate.

use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::common;

/// The server process, killed when the test ends however it ends.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind a free port")
        .local_addr()
        .expect("local address")
        .port()
}

fn tmp(target: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("nats")
        .join(target);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the test directory");
    dir
}

/// Starts `nats-server` on `port` and waits until it listens.
fn start(port: u16, dir: &Path) -> Server {
    let child = Command::new("nats-server")
        .args([
            "-a",
            "127.0.0.1",
            "-p",
            &port.to_string(),
            "--user",
            "app",
            "--pass",
            "secret",
        ])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("start nats-server (is it on PATH?): {e}"));
    let server = Server(child);
    let deadline = Instant::now() + Duration::from_secs(10);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(
            Instant::now() < deadline,
            "nats-server does not listen on {port}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    server
}

/// The Ignite test, with the port of this run.
fn program(port: u16) -> String {
    format!(
        r#"package real

import std.net.{{Endpoint, TcpListener, TcpStream}}
import std.io.IoError
import std.protocol.nats.{{Nats, NatsLimits, NatsMessage, NatsOptions, Subscription, connect_to, flush, publish, publish_message, queue_subscribe, request, respond, shutdown, stats, subscribe}}

type IoResult<T> = Result<T, IoError>

const PORT: Int = {port}

fn answer(client: Nats<TcpStream>, service: Subscription) -> IoResult<Unit> = match service.messages.receive() {{
  Some(message) => respond(client, message, b"pong")
  None => Ok(())
}}

fn pump(from: TcpStream, to: TcpStream) -> IoResult<Unit> {{
  loop {{
    match from.read(65536) {{
      Ok(Data(bytes)) => match to.write_all(bytes) {{
        Ok(_) => {{}}
        Err(_) => return Ok(())
      }}
      _ => {{
        _ = to.close()
        return Ok(())
      }}
    }}
  }}
}}

// Forwards each accepted connection to the server; a value on `cuts`
// closes the current one, and the next is accepted after a value on
// `resumes`.
fn proxy(listener: TcpListener, cuts: Receiver<Int>, resumes: Receiver<Int>) -> IoResult<Unit> {{
  loop {{
    let near = listener.accept()?
    let far = TcpStream.connect_within("127.0.0.1", PORT, 5 seconds)?
    spawn pump(near, far)
    spawn pump(far, near)
    match cuts.receive() {{
      Some(_) => {{}}
      None => return Ok(())
    }}
    _ = near.close()
    _ = far.close()
    match resumes.receive() {{
      Some(_) => {{}}
      None => return Ok(())
    }}
  }}
}}

fn message_of<T>(result: IoResult<T>) -> Text = match result {{
  Ok(_) => "ok"
  Err(error) => error.message
}}

test "a real server: auth, wildcards, queue groups, headers, request and reply, reconnect" {{
  let server = Endpoint {{ address = "127.0.0.1", port = PORT }}
  let options = NatsOptions {{ auth = UserPassword("app", "secret"), name = "real" }}
  let client = connect_to(server, 5 seconds, options, NatsLimits {{}})?
  assert client.info.headers
  let all: Subscription = subscribe(client, "real.>")?
  let one: Subscription = subscribe(client, "real.*")?
  let workers: Subscription = queue_subscribe(client, "real.jobs", "workers")?
  let more: Subscription = queue_subscribe(client, "real.jobs", "workers")?
  flush(client, 5 seconds)?
  publish(client, "real.a", b"1")?
  publish(client, "real.a.b", b"2")?
  publish_message(client, NatsMessage {{ subject = "real.jobs", headers = [("Trace", "t1")], payload = b"job" }})?
  flush(client, 5 seconds)?
  let job = NatsMessage {{ subject = "real.jobs", headers = [("Trace", "t1")], payload = b"job" }}
  assert all.messages.receive_within(5 seconds) == Some(NatsMessage {{ subject = "real.a", payload = b"1" }})
  assert all.messages.receive_within(5 seconds) == Some(NatsMessage {{ subject = "real.a.b", payload = b"2" }})
  assert all.messages.receive_within(5 seconds) == Some(job)
  assert one.messages.receive_within(5 seconds) == Some(NatsMessage {{ subject = "real.a", payload = b"1" }})
  assert one.messages.receive_within(5 seconds) == Some(job)
  let first = workers.messages.receive_within(1 seconds)
  let second = more.messages.receive_within(1 seconds)
  assert (first == None) != (second == None), "one member of the queue group gets the job"

  let service: Subscription = subscribe(client, "real.time")?
  flush(client, 5 seconds)?
  spawn answer(client, service)
  assert request(client, "real.time", b"ping", 5 seconds)?.payload == b"pong"
  assert message_of(request(client, "real.nobody", b"x", 5 seconds)) == "nats: no responders for real.nobody"
  assert message_of(connect_to(server, 5 seconds, NatsOptions {{ auth = UserPassword("app", "wrong") }}, NatsLimits {{}})) == "nats: Authorization Violation"

  let listener = TcpListener.bind("127.0.0.1", 0)?
  let (cut, cuts) = Channel.bounded(1)
  let (resume, resumes) = Channel.bounded(1)
  spawn proxy(listener, cuts, resumes)
  let proxied = connect_to(Endpoint {{ address = "127.0.0.1", port = listener.local_port() }}, 5 seconds, options, NatsLimits {{}})?
  let back: Subscription = subscribe(proxied, "real.back")?
  flush(proxied, 5 seconds)?
  let watcher: Subscription = subscribe(client, "real.held")?
  flush(client, 5 seconds)?
  _ = cut.send(1)
  var waited = 0
  while stats(proxied)?.connected && waited < 500 {{
    Clock.sleep(10 ms)
    waited = waited + 1
  }}
  assert !stats(proxied)?.connected, "the cut connection is lost"
  publish(proxied, "real.held", b"stale")?
  Clock.sleep(1500 ms)
  publish(proxied, "real.held", b"fresh")?
  _ = resume.send(1)
  waited = 0
  while !stats(proxied)?.connected && waited < 1000 {{
    Clock.sleep(10 ms)
    waited = waited + 1
  }}
  flush(proxied, 5 seconds)?
  assert watcher.messages.receive_within(5 seconds) == Some(NatsMessage {{ subject = "real.held", payload = b"fresh" }})
  assert watcher.messages.receive_within(500 ms) == None
  publish(client, "real.back", b"again")?
  assert back.messages.receive_within(5 seconds) == Some(NatsMessage {{ subject = "real.back", payload = b"again" }})
  let counts = stats(proxied)?
  assert counts.stale == 1
  assert counts.losses == 1
  assert counts.connections == 2
  shutdown(proxied)
  assert stats(client)?.errors == 0
  shutdown(client)
}}
"#
    )
}

fn against_a_real_server(target: &str) {
    let dir = tmp(target);
    let port = free_port();
    let _server = start(port, &dir);
    let source = dir.join("real.ignite");
    std::fs::write(&source, program(port)).expect("write the program");
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .arg(&source)
        .arg(format!("--target={target}"))
        .arg("--out")
        .arg(dir.join("out"))
        .arg("--target-dir")
        .arg(dir.join("cargo"))
        .arg("--runtime")
        .arg(common::runner_archive())
        .args(["--timeout", "120"])
        .output()
        .expect("run ignite test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("1 passed"),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

/// One test for both targets, one after the other, so the gate counts one
/// ignored test (`quality/baseline.json` 7.2).
#[test]
#[ignore = "needs nats-server on PATH; run with --ignored"]
fn a_real_server_on_both_targets() {
    against_a_real_server("llvm");
    against_a_real_server("rust");
}
