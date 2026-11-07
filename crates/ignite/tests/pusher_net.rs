//! Story `pusher-net-builtins` (`docs/slice.md` 26) end to end: the
//! reproducers G1, G4 and G5 of `docs/research/pusher-gaps.md` with the
//! chosen names, `tests/fixtures/pusher_net/`, built with `ignite::build`
//! on both targets and run as real processes over real sockets
//! (`pusher_net::rust::<case>`, `pusher_net::llvm::<case>`).

use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use ignite::Target;

use crate::common;

/// The longest a test waits for a line or an exit.
const TIMEOUT: Duration = Duration::from_secs(15);

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pusher_net")
        .join(format!("{name}.ignite"))
}

/// The executable `name` of the fixture `name.ignite` built for `target`.
fn build(name: &str, target: Target) -> PathBuf {
    let suffix = match target {
        Target::Rust => "rust",
        Target::Llvm => "llvm",
    };
    let out = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("pusher_net")
        .join(format!("{name}-{suffix}"));
    let options = ignite::BuildOptions {
        target,
        out: Some(out),
        runtime: (target == Target::Llvm).then(common::runtime_archive),
        ..Default::default()
    };
    let built = ignite::build(&fixture(name), &options).unwrap_or_else(|e| panic!("{e}"));
    built
        .executables
        .into_iter()
        .find(|path| path.file_stem().and_then(|s| s.to_str()) == Some(name))
        .unwrap_or_else(|| panic!("no executable {name}"))
}

/// `executable` started with its standard error read line by line.
fn start(executable: &Path) -> (Child, Receiver<String>) {
    let mut child = Command::new(executable)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("start {}: {e}", executable.display()));
    let stderr = child.stderr.take().expect("stderr");
    (child, lines_of(stderr))
}

fn lines_of(stream: impl Read + Send + 'static) -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

/// The next line that starts with `prefix`, without it.
fn wait_line(lines: &Receiver<String>, prefix: &str) -> String {
    let deadline = Instant::now() + TIMEOUT;
    let mut seen = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match lines.recv_timeout(left) {
            Ok(line) => match line.strip_prefix(prefix) {
                Some(rest) => return rest.to_string(),
                None => seen.push(line),
            },
            Err(error) => panic!("no `{prefix}` line ({error}); seen:\n{}", seen.join("\n")),
        }
    }
}

fn wait_success(child: &mut Child) {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("wait") {
            assert!(status.success(), "{status}");
            return;
        }
        assert!(started.elapsed() < TIMEOUT, "no exit within {TIMEOUT:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// G1: the server logs the address and port the client connected from.
fn peer_is_the_connecting_end(target: Target) {
    let (mut child, lines) = start(&build("peer", target));
    let port: u16 = wait_line(&lines, "listening on 127.0.0.1:")
        .parse()
        .expect("a port");
    let client = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let own = client.local_addr().expect("local address");
    assert_eq!(
        wait_line(&lines, "peer "),
        format!("{} {}", own.ip(), own.port())
    );
    wait_success(&mut child);
}

/// G4: the read the idle timeout ended and the read after the close both
/// return `Eof`, and `timed_out` tells them apart.
fn a_timeout_is_told_from_a_close(target: Target) {
    let (mut child, lines) = start(&build("idle", target));
    let first = wait_line(&lines, "after ");
    let (ms, rest) = first.split_once(" ms: ").expect("`<n> ms: ` in the line");
    let ms: i64 = ms.parse().expect("milliseconds");
    assert!((190..2000).contains(&ms), "{first}");
    assert_eq!(rest, "Eof timed_out=true");
    assert_eq!(wait_line(&lines, "peer closed: "), "Eof timed_out=false");
    wait_success(&mut child);
}

/// G5: a connect to an address that never answers returns an error within
/// its timeout of 500 ms plus 100 ms.
fn connect_within_gives_up_in_time(target: Target) {
    let executable = build("connect", target);
    let (mut child, lines) = start(&executable);
    let line = wait_line(&lines, "connect failed after ");
    let ms: i64 = line
        .strip_suffix(" ms")
        .and_then(|ms| ms.parse().ok())
        .unwrap_or_else(|| panic!("{line}"));
    assert!(ms < 600, "{line}");
    wait_success(&mut child);
}

macro_rules! on_both_targets {
    ($($case:ident),* $(,)?) => {
        mod rust {
            $(#[test] fn $case() { super::$case(ignite::Target::Rust) })*
        }
        mod llvm {
            $(#[test] fn $case() { super::$case(ignite::Target::Llvm) })*
        }
    };
}

on_both_targets!(
    peer_is_the_connecting_end,
    a_timeout_is_told_from_a_close,
    connect_within_gives_up_in_time,
);
