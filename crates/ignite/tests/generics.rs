//! Slice 7 generics and `Store` on the LLVM target: the golden program
//! `crates/ignite-rustgen/tests/golden/generics.ignite` built with `--target
//! llvm` answers the same lines as the Rust target does in
//! `crates/ignite-rustgen/tests/compiles.rs`.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

use ignite::Target;

use crate::common;

fn golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../ignite-rustgen/tests/golden")
}

fn countersd() -> &'static Path {
    static EXE: OnceLock<PathBuf> = OnceLock::new();
    EXE.get_or_init(|| {
        let options = ignite::BuildOptions {
            target: Target::Llvm,
            out: Some(Path::new(env!("CARGO_TARGET_TMPDIR")).join("generics-llvm")),
            runtime: Some(common::runtime_archive()),
            std: Some(golden().join("std")),
            ..Default::default()
        };
        let output = ignite::build(&golden().join("generics.ignite"), &options)
            .unwrap_or_else(|e| panic!("{e}"));
        let [exe] = output.executables.as_slice() else {
            panic!("{:?}", output.executables);
        };
        exe.clone()
    })
}

/// `countersd` killed on drop.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start() -> (Server, u16) {
    let mut child = Command::new(countersd())
        .args(["--port", "0"])
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let server = Server(child);
    let mut line = String::new();
    BufReader::new(stderr).read_line(&mut line).unwrap();
    let port = line
        .trim_end()
        .strip_prefix("listening on 127.0.0.1:")
        .unwrap_or_else(|| panic!("{line:?}"))
        .parse()
        .unwrap();
    (server, port)
}

struct Lines {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
}

impl Lines {
    fn connect(port: u16) -> Lines {
        let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let reader = BufReader::new(stream.try_clone().unwrap());
        Lines { stream, reader }
    }

    fn ask(&mut self, line: &str) -> String {
        self.stream
            .write_all(format!("{line}\n").as_bytes())
            .unwrap();
        let mut reply = String::new();
        self.reader.read_line(&mut reply).unwrap();
        reply.trim_end().to_string()
    }
}

#[test]
fn llvm_generics_and_store_serve() {
    let (_server, port) = start();
    let mut lines = Lines::connect(port);
    for (request, reply) in [
        ("generic", "one 1 none 2 7"),
        ("new", "1"),
        ("new", "2"),
        ("inc 1", "1"),
        ("inc 1", "2"),
        ("inc 9", "-1"),
        ("inc +1", "unknown"),
        ("get 1", "2"),
        ("all", "2,0"),
        ("del 2", "removed 0"),
        ("del 2", "missing"),
        ("all", "2"),
        ("byte 255", "byte 255"),
        ("byte 256", "not a byte"),
        ("byte -1", "not a byte"),
        ("byte 9223372036854775808", "unknown"),
    ] {
        assert_eq!(lines.ask(request), reply, "{request}");
    }
}

/// 16 connections each create 100 ids and increment one shared counter
/// 100 times: 1600 distinct ids and a count of 1600 (`update` is atomic).
#[test]
fn llvm_store_ids_and_updates_are_atomic_across_connections() {
    let (_server, port) = start();
    let mut setup = Lines::connect(port);
    assert_eq!(setup.ask("new"), "1");
    let threads: Vec<_> = (0..16)
        .map(|_| {
            std::thread::spawn(move || {
                let mut lines = Lines::connect(port);
                let mut ids = Vec::new();
                for _ in 0..100 {
                    ids.push(lines.ask("new"));
                    lines.ask("inc 1");
                }
                ids
            })
        })
        .collect();
    let mut ids: Vec<String> = threads
        .into_iter()
        .flat_map(|t| t.join().unwrap())
        .collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 1600);
    assert_eq!(setup.ask("get 1"), "1600");
}
