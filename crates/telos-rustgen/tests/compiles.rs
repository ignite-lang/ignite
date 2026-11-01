//! Generated projects build with cargo against the in-tree `telos-io`: the
//! terminal example with the repository's standard library, and the golden
//! `modules` program with the golden standard library (a protocol, imports,
//! `spawn` and function values across a `telos-std` crate and a program),
//! and the golden `consts` library (constants own and imported from
//! `telos-std`, qualified variant patterns, `IoError.new`), the golden
//! `variants` program (variants of enums not imported, in the program and in
//! `telos-std`) and the golden `exits` program, run (`Process.exit`).

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use telos_rustgen::write_project;

/// The cargo target directory of this test run.
fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| common::workspace().join("target"))
}

/// Lower `file`, write the program and `std/` under the target directory,
/// run `cargo build` and return the paths of the executables.
fn build(file: &Path, std_root: &Path, name: &str) -> Vec<PathBuf> {
    let substrate = common::workspace().join("crates/telos-io");
    let substrate = substrate.canonicalize().unwrap_or(substrate);
    let (project, std) = common::lower(file, std_root, &substrate);

    let target = target_dir();
    let out = target.join("telos-rustgen-test").join(name);
    write_project(&project, &out).unwrap();
    write_project(&std, &out.join("std")).unwrap();

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .arg("build")
        .arg("--manifest-path")
        .arg(out.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&target)
        .env("CARGO_INCREMENTAL", "0")
        .status()
        .unwrap();
    assert!(status.success(), "cargo build of {} failed", out.display());
    project
        .executables
        .iter()
        .map(|e| {
            target
                .join("debug")
                .join(format!("{}{}", e.name, std::env::consts::EXE_SUFFIX))
        })
        .collect()
}

#[test]
fn terminal_project_builds() {
    let executables = build(
        &common::workspace().join("examples/terminal/terminal.telos"),
        &common::repository_std(),
        "terminal",
    );
    assert_eq!(executables.len(), 2);
    for executable in executables {
        assert!(executable.exists(), "{}", executable.display());
    }
}

#[test]
fn constants_across_modules_build() {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let executables = build(
        &golden.join("consts.telos"),
        &common::golden_std(),
        "consts",
    );
    assert!(executables.is_empty());
}

#[test]
fn variants_of_enums_not_imported_build() {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let executables = build(
        &golden.join("variants.telos"),
        &golden.join("variants/std"),
        "variants",
    );
    assert!(executables.is_empty());
}

/// `Process.exit(5)` without `?` builds and ends the client with status 5
/// once the peer closes the connection.
#[test]
fn process_exit_without_try_builds_and_exits() {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let executables = build(&golden.join("exits.telos"), &common::golden_std(), "exits");
    let [exiter] = executables.as_slice() else {
        panic!("{executables:?}");
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let mut client = Command::new(exiter)
        .args(["--port", &port])
        .stdin(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let (stream, _) = listener.accept().unwrap();
    drop(stream);
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = client.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > std::time::Duration::from_secs(10) {
            client.kill().unwrap();
            let _ = client.wait();
            panic!("exiter did not exit");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(5));
}

#[test]
fn std_crate_and_program_build() {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let executables = build(
        &golden.join("modules.telos"),
        &common::golden_std(),
        "modules",
    );
    assert_eq!(executables.len(), 2);
    for executable in executables {
        assert!(executable.exists(), "{}", executable.display());
    }
}

/// `countersd` killed on drop.
struct Server(std::process::Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Start `countersd` on a free port; the port it reports.
fn start_counters() -> (Server, u16) {
    use std::io::BufRead;
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let executables = build(
        &golden.join("generics.telos"),
        &common::golden_std(),
        "generics",
    );
    let [countersd] = executables.as_slice() else {
        panic!("{executables:?}");
    };
    let mut child = Command::new(countersd)
        .args(["--port", "0"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let server = Server(child);
    let mut line = String::new();
    std::io::BufReader::new(stderr)
        .read_line(&mut line)
        .unwrap();
    let port = line
        .trim_end()
        .strip_prefix("listening on 127.0.0.1:")
        .unwrap_or_else(|| panic!("{line:?}"))
        .parse()
        .unwrap();
    (server, port)
}

/// One connection of the line protocol of `generics.telos`.
struct Lines {
    stream: std::net::TcpStream,
    reader: std::io::BufReader<std::net::TcpStream>,
}

impl Lines {
    fn connect(port: u16) -> Lines {
        let stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        let reader = std::io::BufReader::new(stream.try_clone().unwrap());
        Lines { stream, reader }
    }

    fn ask(&mut self, line: &str) -> String {
        use std::io::{BufRead, Write};
        self.stream
            .write_all(format!("{line}\n").as_bytes())
            .unwrap();
        let mut reply = String::new();
        self.reader.read_line(&mut reply).unwrap();
        reply.trim_end().to_string()
    }
}

/// The slice 7 golden runs: generic fns, the Store methods through an
/// effectful fn-typed field, `Int.parse`, `to_byte` and `to_int`.
#[test]
fn generics_and_store_serve() {
    let (_server, port) = start_counters();
    let mut lines = Lines::connect(port);
    assert_eq!(lines.ask("generic"), "one 1 none 2 7");
    assert_eq!(lines.ask("new"), "1");
    assert_eq!(lines.ask("new"), "2");
    assert_eq!(lines.ask("inc 1"), "1");
    assert_eq!(lines.ask("inc 1"), "2");
    assert_eq!(lines.ask("inc 9"), "-1");
    assert_eq!(lines.ask("inc +1"), "unknown");
    assert_eq!(lines.ask("get 1"), "2");
    assert_eq!(lines.ask("all"), "2,0");
    assert_eq!(lines.ask("del 2"), "removed 0");
    assert_eq!(lines.ask("del 2"), "missing");
    assert_eq!(lines.ask("all"), "2");
    assert_eq!(lines.ask("byte 255"), "byte 255");
    assert_eq!(lines.ask("byte 256"), "not a byte");
    assert_eq!(lines.ask("byte -1"), "not a byte");
    assert_eq!(lines.ask("byte 9223372036854775808"), "unknown");
}

/// 16 connections each create 100 ids and increment one shared counter
/// 100 times: 1600 distinct ids and a count of 1600 (`update` is atomic).
#[test]
fn store_ids_and_updates_are_atomic_across_connections() {
    let (_server, port) = start_counters();
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
