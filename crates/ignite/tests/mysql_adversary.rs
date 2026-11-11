//! Adversary, story `std-mysql-client`, pass 1: what the inline tests of
//! `std/protocol/mysql.ignite` and `mysql::` do not say.
//!
//! - [`a_real_server_numbers_its_reply_after_a_command_of_two_frames_and_prints_doubles_its_own_way`]:
//!   one container of the image `mysql:8.0` (as `mysql::`: it runs where
//!   `docker version` succeeds and the image is present, pulls nothing and
//!   says why it did nothing elsewhere). A command of 16 MiB - 1 bytes or
//!   more travels as two frames and the server numbers its reply 2; and
//!   the server prints a DOUBLE over the text protocol in its own notation.
//! - [`connections_leave_no_descriptor_on_both_targets`]: no container. A
//!   server of this test speaks the handshake of a server without TLS,
//!   refuses the account `denied`, answers every command with OK and ends a
//!   connection on a statement that starts with `die`. The Ignite tests
//!   open, use and end 40 connections of each kind and count the open
//!   descriptors of their own process before and after.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use crate::common;

const IMAGE: &str = "mysql:8.0";

const INIT: &str = r#"CREATE DATABASE shop CHARACTER SET utf8mb4;
CREATE USER 'legacy'@'%' IDENTIFIED WITH mysql_native_password BY 'legacy-pw';
GRANT SELECT ON shop.* TO 'legacy'@'%';
"#;

/// The Ignite tests against the real server; `@PORT@` is its port.
const REAL: &str = r##"package adversary_real

import std.net.{Endpoint}
import std.protocol.mysql.{MysqlOptions, MysqlError, MysqlValue, connect, query, execute_once, quit, describe}

test fn server() -> Endpoint = Endpoint { address = "127.0.0.1", port = @PORT@ }

test fn legacy() -> MysqlOptions = MysqlOptions { user = "legacy", password = "legacy-pw", database = "shop" }

test fn failure<R>(outcome: Result<R, MysqlError>) -> Text = match outcome {
  Ok(_) => "no error"
  Err(error) => describe(error)
}

// `count` bytes that are not all alike, built by doubling.
test fn filled(count: Int) -> Bytes {
  var out = b"0123456789abcdef"
  while out.len() < count {
    let more = out
    out.append(more)
  }
  out.slice(0, count)
}

test "a parameter of 16 MiB - 1 bytes makes a command of two frames, and its reply is read" {
  let client = connect(server(), legacy())?
  let small = execute_once(client, "SELECT LENGTH(?)", [Some(Bytes(filled(1000000)))])?
  assert small.rows == [[Some(Int(1000000))]]
  let large = execute_once(client, "SELECT LENGTH(?)", [Some(Bytes(filled(16777215)))])
  let said = failure(large)
  assert said == "no error", said
  assert large?.rows == [[Some(Int(16777215))]]
  quit(client)?
}

test "a statement of 16 MiB makes a command of two frames, and its reply is read" {
  let client = connect(server(), legacy())?
  var sql = b"SELECT LENGTH('"
  sql.append(filled(16777216))
  sql.append(b"')")
  let large = query(client, sql.to_text())
  let said = failure(large)
  assert said == "no error", said
  assert large?.rows == [[Some(Int(16777216))]]
  quit(client)?
}

test fn digits(value: Option<MysqlValue>) -> Text = match value {
  Some(Decimal(text)) => text
  _ => "no decimal"
}

test "a DOUBLE prints alike from query and from execute" {
  let client = connect(server(), legacy())?
  let sql = "SELECT 1e14, 1e15, 1e16, 123456789e7, 123456789012345680000e0, 1e21, 0.000001e0, 1e-7, 9.5e-7, 1.5e-10, 1e-15, 1000000000000000.25e0, 1125899906842624.25e0, 1e0/3, 0.1e0 + 0.2e0, 5e-324, 1.7976931348623157e308"
  let texts = query(client, sql)?
  let binaries = execute_once(client, sql, [])?
  quit(client)?
  assert texts.rows.len() == 1 && texts.rows[0].len() == 17 && binaries.rows[0].len() == 17
  var differ = ""
  var i = 0
  while i < 17 {
    if texts.rows[0][i] != binaries.rows[0][i] {
      let asked = digits(texts.rows[0][i])
      let run = digits(binaries.rows[0][i])
      differ = "{differ} [query {asked}, execute {run}]"
    }
    i = i + 1
  }
  assert differ == "", differ
}
"##;

/// The Ignite tests against [`serve`]; `@PORT@` is its port.
const DESCRIPTORS: &str = r##"package adversary_descriptors

import std.net.{Endpoint}
import std.io.IoError
import std.process.{Invocation}
import std.protocol.mysql.{MysqlOptions, MysqlError, connect, connect_tls, query, ping, quit, describe, pool_of, pool_query, checkin, quit_idle}

test fn server() -> Endpoint = Endpoint { address = "127.0.0.1", port = @PORT@ }

test fn account(user: Text) -> MysqlOptions = MysqlOptions { user, password = "secret", read_timeout = 5 seconds }

test fn failure<R>(outcome: Result<R, MysqlError>) -> Text = match outcome {
  Ok(_) => "no error"
  Err(error) => describe(error)
}

// The open descriptors of this process, counted by a child of it.
test fn descriptors() -> Result<Int, IoError> {
  let exit = Launcher.new().execute(Invocation { program = "/bin/sh", arguments = ["-c", "printf %s $(ls /proc/$PPID/fd | wc -l)"] })?
  match Int.parse(exit.stdout.to_text()) {
    Some(count) => Ok(count)
    None => Err(IoError.new("no descriptor count"))
  }
}

// One connection opened, used and quit.
test fn session() -> Result<Unit, MysqlError> {
  let client = connect(server(), account("app"))?
  ping(client)?
  quit(client)
}

// The descriptors while three connections are open: the count sees a socket.
test fn three_open() -> Result<Int, IoError> {
  let a = connect(server(), account("app"))
  let b = connect(server(), account("app"))
  let c = connect(server(), account("app"))
  let open = descriptors()?
  assert failure(a) == "no error" && failure(b) == "no error" && failure(c) == "no error"
  Ok(open)
}

test "the count of descriptors sees an open connection" {
  session()?
  let before = descriptors()?
  let open = three_open()?
  assert open == before + 3, "before {before}, with three connections open {open}"
}

test "40 connections opened, used and quit leave no descriptor" {
  session()?
  let before = descriptors()?
  var i = 0
  while i < 40 {
    session()?
    i = i + 1
  }
  let after = descriptors()?
  assert after == before, "{before} descriptors before, {after} after 40 connections"
}

test "40 connections the server refuses leave no descriptor" {
  assert failure(connect(server(), account("denied"))) == "server error 1045 (28000)"
  let before = descriptors()?
  var i = 0
  while i < 40 {
    assert failure(connect(server(), account("denied"))) == "server error 1045 (28000)"
    i = i + 1
  }
  let after = descriptors()?
  assert after == before, "{before} descriptors before, {after} after 40 refused connections"
}

test "40 TLS requests to a server that offers no TLS leave no descriptor" {
  assert failure(connect_tls(server(), account("app"))) == "the server offers no TLS"
  let before = descriptors()?
  var i = 0
  while i < 40 {
    assert failure(connect_tls(server(), account("app"))) == "the server offers no TLS"
    i = i + 1
  }
  let after = descriptors()?
  assert after == before, "{before} descriptors before, {after} after 40 refused TLS requests"
}

// A pool whose connection fails, is taken out and replaced, and whose
// replacement is quit with the pool.
test fn pooled() -> Result<Unit, MysqlError> {
  let pool = pool_of([connect(server(), account("app"))?], 20 ms)
  assert pool_query(pool, "die") == Err(Closed)
  assert pool_query(pool, "SELECT 1") == Err(NoConnection)
  checkin(pool, connect(server(), account("app"))?)
  _ = pool_query(pool, "SELECT 1")?
  checkin(pool, connect(server(), account("app"))?)
  quit_idle(pool)
  Ok(())
}

test "40 pools with a failed connection, its replacement and one too many leave no descriptor" {
  pooled()?
  let before = descriptors()?
  var i = 0
  while i < 40 {
    pooled()?
    i = i + 1
  }
  let after = descriptors()?
  assert after == before, "{before} descriptors before, {after} after 40 pools of 3 connections"
}
"##;

fn docker(args: &[&str]) -> Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("start docker")
}

/// Removes the container when the test ends, passed or not.
struct Container(String);

impl Drop for Container {
    fn drop(&mut self) {
        let _ = docker(&["rm", "-f", "-v", &self.0]);
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind a free port")
        .local_addr()
        .expect("local address")
        .port()
}

/// An empty directory of this module's test `name`.
fn directory(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("mysql_adversary")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the test directory");
    dir
}

/// Runs the tests of `source` on `target`: the exit status of `ignite
/// test` and what it printed, standard output before standard error.
fn run(target: &str, dir: &Path, source: &str) -> (Option<i32>, String) {
    let dir = dir.join(target);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("adversary.ignite");
    std::fs::write(&file, source).expect("write the program");
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .arg(&file)
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
    let printed = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.code(), printed)
}

/// Both targets run `source`, and then both must have passed its `cases`
/// tests: a target that fails does not hide the other's result.
fn both_targets_pass(dir: &Path, source: &str, cases: usize) {
    let mut failed = Vec::new();
    for target in ["llvm", "rust"] {
        let (status, printed) = run(target, dir, source);
        println!("mysql_adversary: target {target}\n{printed}");
        let passed = format!("test result: ok. {cases} passed; 0 failed;");
        if status != Some(0) || !printed.contains(&passed) {
            failed.push(format!("target {target}, exit {status:?}:\n{printed}"));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// Waits until the server behind `port` sends its handshake.
fn ready(container: &Container, port: u16) {
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut head = [0u8; 5];
            if stream.read_exact(&mut head).is_ok() && head[4] == 10 {
                return;
            }
        }
        if Instant::now() >= deadline {
            let logs = docker(&["logs", "--tail", "40", &container.0]);
            panic!(
                "{} sends no handshake on {port}:\n{}",
                container.0,
                String::from_utf8_lossy(&logs.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

#[test]
fn a_real_server_numbers_its_reply_after_a_command_of_two_frames_and_prints_doubles_its_own_way() {
    let image = std::env::var("IGNITE_MYSQL_IMAGE").unwrap_or_else(|_| IMAGE.to_string());
    let runtime = Command::new("docker").arg("version").output();
    if !runtime.is_ok_and(|output| output.status.success()) {
        eprintln!("mysql_adversary: skipped, no container runtime (`docker version` fails)");
        return;
    }
    if !docker(&["image", "inspect", &image]).status.success() {
        eprintln!("mysql_adversary: skipped, the image {image} is not present");
        return;
    }
    let dir = directory("real");
    std::fs::write(dir.join("init.sql"), INIT).unwrap();
    let port = free_port();
    let name = format!("ignite-mysql-adversary-{}", std::process::id());
    let publish = format!("127.0.0.1:{port}:3306");
    let init = format!(
        "{}:/docker-entrypoint-initdb.d/init.sql:ro",
        dir.join("init.sql").display()
    );
    let mut args = vec!["run", "-d", "--rm", "--name", &name];
    let label = std::env::var("IGNITE_CONTAINER_LABEL").ok();
    if let Some(label) = &label {
        args.extend(["--label", label]);
    }
    args.extend([
        "-p",
        &publish,
        "-e",
        "MYSQL_ROOT_PASSWORD=root-pw",
        "-v",
        &init,
        &image,
        "--default-authentication-plugin=mysql_native_password",
    ]);
    let output = docker(&args);
    assert!(
        output.status.success(),
        "docker {}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    let container = Container(name.clone());
    ready(&container, port);
    both_targets_pass(&dir, &REAL.replace("@PORT@", &port.to_string()), 3);
}

/// One packet: its length, `sequence` and `payload` (shorter than 64 KiB).
fn packet(sequence: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![payload.len() as u8, (payload.len() >> 8) as u8, 0, sequence];
    out.extend_from_slice(payload);
    out
}

fn read_packet(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut head = [0u8; 4];
    stream.read_exact(&mut head)?;
    let size = usize::from(head[0]) | usize::from(head[1]) << 8 | usize::from(head[2]) << 16;
    let mut payload = vec![0u8; size];
    stream.read_exact(&mut payload)?;
    Ok(payload)
}

/// One connection of the scripted server: the handshake of a server
/// "8.0.36" without TLS (capabilities 0xDFFFF7FF) whose plugin is
/// `mysql_native_password`; error 1045 for the account `denied`, OK for
/// any other; then OK for every command, until `COM_QUIT`, a statement
/// that starts with `die` or the end of the stream, each of which ends the
/// connection.
fn serve(stream: &mut TcpStream) -> std::io::Result<()> {
    const OK: &[u8] = b"\x00\x00\x00\x02\x00\x00\x00";
    stream.set_read_timeout(Some(Duration::from_secs(20)))?;
    let mut greeting = b"\x0a8.0.36\x00".to_vec();
    greeting.extend_from_slice(&[77, 0, 0, 0]);
    greeting.extend_from_slice(b"\x01\x02\x03\x04\x05\x06\x07\x08\x00");
    greeting.extend_from_slice(&[0xff, 0xf7, 0xff, 0x02, 0x00, 0xff, 0xdf, 21]);
    greeting.extend_from_slice(&[0; 10]);
    greeting.extend_from_slice(b"\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14\x00");
    greeting.extend_from_slice(b"mysql_native_password\x00");
    stream.write_all(&packet(0, &greeting))?;
    let response = read_packet(stream)?;
    if response
        .get(32..)
        .is_some_and(|account| account.starts_with(b"denied\x00"))
    {
        return stream.write_all(&packet(2, b"\xff\x15\x04#28000Access denied"));
    }
    stream.write_all(&packet(2, OK))?;
    loop {
        let command = read_packet(stream)?;
        if command.first() == Some(&1) || command.starts_with(b"\x03die") {
            return Ok(());
        }
        stream.write_all(&packet(1, OK))?;
    }
}

#[test]
fn connections_leave_no_descriptor_on_both_targets() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind the scripted server");
    let port = listener.local_addr().expect("local address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            std::thread::spawn(move || {
                let _ = serve(&mut stream);
            });
        }
    });
    let dir = directory("descriptors");
    both_targets_pass(&dir, &DESCRIPTORS.replace("@PORT@", &port.to_string()), 5);
}
