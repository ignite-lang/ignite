//! `docs/slice.md` 30 end to end: the case study's G3 rewritten without
//! bells (`fixtures/channels/g3.ignite`), built once per target and started
//! as real processes, and 1,000 waiters on one channel woken by one sender
//! without a single extra file descriptor.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use ignite::Target;

use crate::common;

const SOURCE: &str = "g3.ignite";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/channels")
        .join(name)
}

fn tmp(name: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("channels")
        .join(name)
}

fn target_name(target: Target) -> &'static str {
    match target {
        Target::Rust => "rust",
        Target::Llvm => "llvm",
    }
}

/// The executable `name` of the fixture, built once per target.
fn executable(name: &str, target: Target) -> PathBuf {
    static BUILT: Mutex<Vec<(&'static str, Vec<PathBuf>)>> = Mutex::new(Vec::new());
    let key = target_name(target);
    let mut built = BUILT.lock().unwrap_or_else(|e| e.into_inner());
    if !built.iter().any(|(t, _)| *t == key) {
        let out = tmp(format!("build-{key}"));
        let options = ignite::BuildOptions {
            target,
            out: Some(out),
            runtime: (target == Target::Llvm).then(common::runtime_archive),
            ..Default::default()
        };
        let output = ignite::build(&fixture(SOURCE), &options).unwrap_or_else(|e| panic!("{e}"));
        built.push((key, output.executables));
    }
    let (_, executables) = built
        .iter()
        .find(|(t, _)| *t == key)
        .expect("built just now");
    executables
        .iter()
        .find(|path| path.file_stem().and_then(|s| s.to_str()) == Some(name))
        .unwrap_or_else(|| panic!("no {name} in {executables:?}"))
        .clone()
}

macro_rules! both_targets {
    ($($case:ident),* $(,)?) => {
        mod rust {
            $(#[test]
            fn $case() {
                super::$case(ignite::Target::Rust);
            })*
        }
        mod llvm {
            $(#[test]
            fn $case() {
                super::$case(ignite::Target::Llvm);
            })*
        }
    };
}

both_targets!(
    g3_long_poll_heartbeat_without_bells,
    a_thousand_waiters_hold_no_extra_descriptor,
);

/// The number in `line` after `prefix`.
fn number_after(line: &str, prefix: &str) -> u64 {
    line.strip_prefix(prefix)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("`{prefix}` and a number expected: {line}"))
}

/// G3: a long-poll held to its 200 ms deadline returns `None` then, one
/// woken by a message 50 ms later returns it then, and a heartbeat ticks
/// every 30 ms until its stop channel is closed.
fn g3_long_poll_heartbeat_without_bells(target: Target) {
    let path = executable("longpoll", target);
    let output = Command::new(&path)
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("run {}: {e}", path.display()));
    let text = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(0), "{text}");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    let deadline = number_after(lines[0], "deadline ");
    assert!((200..2000).contains(&deadline), "{text}");
    assert!(lines[0].ends_with(" None"), "{text}");
    let woken = number_after(lines[1], "woken ");
    assert!((50..2000).contains(&woken), "{text}");
    assert!(lines[1].ends_with(" message"), "{text}");
    assert_eq!(lines[2], "heartbeat stopped", "{text}");
}

/// The open descriptors of process `pid`.
fn descriptors(pid: u32) -> usize {
    std::fs::read_dir(format!("/proc/{pid}/fd"))
        .unwrap_or_else(|e| panic!("/proc/{pid}/fd: {e}"))
        .count()
}

/// 1,000 tasks wait on one channel and one sender wakes them all; the
/// process holds as many descriptors while they wait as before they were
/// spawned (a bell costs two per waiter), but for the one eventfd of the
/// process that ends a worker's blocking `poll` once the live tasks are as
/// many as workers may start (`ESCALATION` in `ignite-io`'s `sched.rs`).
fn a_thousand_waiters_hold_no_extra_descriptor(target: Target) {
    let path = executable("waiters", target);
    let mut child = Command::new(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("run {}: {e}", path.display()));
    let pid = child.id();
    let mut stdin = child.stdin.take().expect("stdin");
    let mut lines = BufReader::new(child.stderr.take().expect("stderr")).lines();
    let mut next = || {
        lines
            .next()
            .expect("a line before the end")
            .expect("stderr readable")
    };
    assert_eq!(next(), "start");
    let before = descriptors(pid);
    stdin.write_all(b"\n").expect("write stdin");
    stdin.flush().expect("flush stdin");
    assert_eq!(next(), "waiting 1000");
    let waiting = descriptors(pid);
    stdin.write_all(b"\n").expect("write stdin");
    stdin.flush().expect("flush stdin");
    assert_eq!(next(), "woken 1000 sum 500500");
    let status = child.wait().expect("wait");
    assert_eq!(status.code(), Some(0));
    assert!(
        (before..=before + 1).contains(&waiting),
        "descriptors before {before}, while waiting {waiting}"
    );
}
