//! `std.protocol.resp` against a real `redis-server` and `redis-sentinel`
//! (`docs/slice.md`, the RESP client section): a master, its replica and
//! one sentinel on free loopback ports, then an Ignite test that sends
//! commands, a pipeline and a subscription to the master, asks the
//! sentinel for the master and follows its `+switch-master` to the replica
//! after a forced failover. Once per target, each with servers of its own.
//! Ignored by default, since neither server is installed where the gate
//! runs: `cargo test -p ignite --test main resp:: -- --ignored` runs it
//! where both are on `PATH`. The inline tests of `std/protocol/resp.ignite`
//! cover the same client over a scripted server in every gate.

use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::common;

/// A server process, killed when the test ends however it ends.
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
        .join("resp")
        .join(target);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the test directory");
    dir
}

/// Starts `program` with `args` and waits until it listens on `port`.
fn start(program: &str, args: &[String], port: u16, dir: &Path) -> Server {
    let child = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("start {program} (is it on PATH?): {e}"));
    let server = Server(child);
    let deadline = Instant::now() + Duration::from_secs(10);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(
            Instant::now() < deadline,
            "{program} does not listen on {port}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    server
}

fn redis(port: u16, dir: &Path, extra: &[String]) -> Server {
    let mut args: Vec<String> = [
        "--port",
        &port.to_string(),
        "--bind",
        "127.0.0.1",
        "--save",
        "",
        "--appendonly",
        "no",
        "--dbfilename",
        &format!("dump-{port}.rdb"),
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    args.extend(extra.iter().cloned());
    start("redis-server", &args, port, dir)
}

/// The Ignite test, with the ports of this run.
fn program(master: u16, replica: u16, sentinel: u16, dead: u16) -> String {
    format!(
        r#"package real

import std.net.{{Endpoint, TcpStream}}
import std.protocol.resp.{{RespValue, RespLimits, Expiry, Topics, Message, connect, command, words, pipeline, set, get, del, hset, hgetall, expire, ttl, hello, publish, subscribe, unsubscribe, master_at, watch_master, next_master}}

const MASTER: Int = {master}
const REPLICA: Int = {replica}
const SENTINEL: Int = {sentinel}
const DEAD: Int = {dead}

test "a real server, its sentinel and a failover" {{
  let master = Endpoint {{ address = "127.0.0.1", port = MASTER }}
  let client = connect(master, 5 seconds)?
  set(client, "resp:k", b"v")?
  assert get(client, "resp:k")? == Some(b"v")
  assert pipeline(client, [words(["INCR", "resp:n"]), words(["INCR", "resp:n"]), words(["GET", "resp:none"])])? == [Integer(1), Integer(2), Null]
  assert hset(client, "resp:h", [("a", b"1")])? == 1
  assert hgetall(client, "resp:h")? == [("a", b"1")]
  assert expire(client, "resp:k", 60 seconds)?
  assert match ttl(client, "resp:k")? {{
    Remaining(left) => left > 50 seconds
    _ => false
  }}
  assert hello(client, 3)?.len() > 0
  assert hgetall(client, "resp:h")? == [("a", b"1")]
  let listening = TcpStream.connect_within("127.0.0.1", MASTER, 5 seconds)?
  listening.set_idle_timeout(10000)?
  let subscription = subscribe(listening, Topics {{ channels = ["resp:news"] }}, RespLimits {{}})?
  assert publish(client, "resp:news", b"hi")? == 1
  assert subscription.messages.receive() == Some(Message {{ channel = "resp:news", payload = b"hi" }})
  unsubscribe(subscription)?
  assert subscription.messages.receive() == None
  _ = listening.close()
  assert del(client, ["resp:k", "resp:n", "resp:h"])? == 3

  let sentinels = [Endpoint {{ address = "127.0.0.1", port = DEAD }}, Endpoint {{ address = "127.0.0.1", port = SENTINEL }}]
  assert master_at(sentinels, "resp-master", 2 seconds, RespLimits {{}})? == master
  let watching = TcpStream.connect_within("127.0.0.1", SENTINEL, 5 seconds)?
  watching.set_idle_timeout(60000)?
  let watch = watch_master(watching, RespLimits {{}})?
  let admin = connect(Endpoint {{ address = "127.0.0.1", port = SENTINEL }}, 5 seconds)?
  var attempts = 0
  while command(admin, words(["SENTINEL", "failover", "resp-master"]))? != Simple("OK") {{
    attempts = attempts + 1
    assert attempts < 120, "the sentinel never accepts the failover"
    Clock.sleep(500 ms)
  }}
  let replica = Endpoint {{ address = "127.0.0.1", port = REPLICA }}
  assert next_master(watch, "resp-master") == Some(replica)
  _ = watching.close()
  assert master_at(sentinels, "resp-master", 2 seconds, RespLimits {{}})? == replica
}}
"#
    )
}

fn against_real_servers(target: &str) {
    let dir = tmp(target);
    let (master, replica, sentinel, dead) = (free_port(), free_port(), free_port(), free_port());
    let _master = redis(master, &dir, &[]);
    let _replica = redis(
        replica,
        &dir,
        &[
            "--replicaof".to_string(),
            "127.0.0.1".to_string(),
            master.to_string(),
        ],
    );
    let config = dir.join("sentinel.conf");
    std::fs::write(
        &config,
        format!(
            "port {sentinel}\nbind 127.0.0.1\ndir {}\nsentinel monitor resp-master 127.0.0.1 {master} 1\nsentinel down-after-milliseconds resp-master 1000\nsentinel failover-timeout resp-master 10000\n",
            dir.display()
        ),
    )
    .expect("write the sentinel configuration");
    let _sentinel = start(
        "redis-sentinel",
        &[config.display().to_string()],
        sentinel,
        &dir,
    );
    let source = dir.join("real.ignite");
    std::fs::write(&source, program(master, replica, sentinel, dead)).expect("write the program");
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
#[ignore = "needs redis-server and redis-sentinel on PATH; run with --ignored"]
fn a_real_server_and_sentinel_on_both_targets() {
    against_real_servers("llvm");
    against_real_servers("rust");
}
