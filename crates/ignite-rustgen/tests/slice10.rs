//! Slice 10 on the Rust target (`docs/slice.md` section 14): the inline
//! tests of `tests/slice10/features.ignite` (quantities, named arguments,
//! brace literals, `Map`, `with_exit_code`) are lowered as a test build,
//! built with cargo and run, and must pass; `failures.ignite` must fail with
//! the 14.8 reports. T1 end to end: `crash.ignite`, the terminal with a
//! `crash` command, is built and run as `crashd` on a free port, and
//! survives sessions whose handler fails an `assert`, overflows or indexes
//! out of range.

use crate::common;

use std::io::{BufRead, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use ignite_rustgen::{TEST_EXECUTABLE, write_project};

const TIMEOUT: Duration = Duration::from_secs(10);
/// The prompt as `std.protocol.telnet` sends it: the text, then IAC GA.
const PROMPT: &[u8] = b"terminal> \xff\xf9";

fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| common::workspace().join("target"))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/slice10")
        .join(name)
}

fn substrate() -> PathBuf {
    let substrate = common::workspace().join("crates/ignite-io");
    substrate.canonicalize().unwrap_or(substrate)
}

fn cargo_build(out: &Path, builds: &Path) {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .arg("build")
        .arg("--manifest-path")
        .arg(out.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(builds)
        .env("CARGO_INCREMENTAL", "0")
        .status()
        .unwrap();
    assert!(status.success(), "cargo build of {} failed", out.display());
}

/// Lower `file` as a test build against the repository's standard
/// library, build it and return its `ignite-test` executable.
fn build_tests(file: &Path, name: &str) -> PathBuf {
    let (project, std) = common::lower_tests(file, &common::repository_std(), &substrate());
    let out = target_dir().join("ignite-rustgen-slice10").join(name);
    write_project(&project, &out).unwrap();
    write_project(&std, &out.join("std")).unwrap();
    let builds = target_dir()
        .join("ignite-rustgen-slice10")
        .join(format!("{name}-target"));
    cargo_build(&out, &builds);
    let file = format!("{TEST_EXECUTABLE}{}", std::env::consts::EXE_SUFFIX);
    builds.join("debug").join(file)
}

/// Lower program `file` against the repository's standard library, build
/// it and return its executables.
fn build_program(file: &Path, name: &str) -> Vec<PathBuf> {
    let (project, std) = common::lower_typed(file, &common::repository_std(), &substrate());
    let out = target_dir().join("ignite-rustgen-slice10").join(name);
    write_project(&project, &out).unwrap();
    write_project(&std, &out.join("std")).unwrap();
    let builds = target_dir()
        .join("ignite-rustgen-slice10")
        .join(format!("{name}-target"));
    cargo_build(&out, &builds);
    project
        .executables
        .iter()
        .map(|e| {
            builds
                .join("debug")
                .join(format!("{}{}", e.name, std::env::consts::EXE_SUFFIX))
        })
        .collect()
}

fn run(executable: &Path, args: &[&str]) -> Output {
    Command::new(executable).args(args).output().unwrap()
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The driver's test build (`generate_tests`): a packaged program sits at
/// its module path inside the `ignite-tests` crate, its tests are lowered
/// there, and the test executable names them by that path.
#[test]
fn generate_tests_lays_a_program_out_by_its_module_path() {
    let (project, _) = common::lower_tests_grouped(
        &fixture("features.ignite"),
        &common::repository_std(),
        &substrate(),
    );
    let lib = project
        .lib_rs
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        lib.contains("pub mod fixtures { pub mod features {"),
        "{lib}"
    );
    assert!(lib.contains("pub fn __ignite_test_"), "{lib}");
    let [main] = project.executables.as_slice() else {
        panic!("{:?}", project.executables.len());
    };
    assert!(
        main.source
            .contains("&ignite_tests::fixtures::features::__ignite_test_"),
        "{}",
        main.source
    );
}

#[test]
fn slice10_features_pass() {
    let executable = build_tests(&fixture("features.ignite"), "features");
    let output = run(&executable, &[]);
    assert!(output.status.success(), "{}", text(&output));
}

/// Each failing test ends with exit 101 and one report naming the
/// operation, the Ignite position and the operands (14.8).
#[test]
fn failures_report_the_operation_and_position() {
    let file = fixture("failures.ignite");
    let executable = build_tests(&file, "failures");
    let path = common::source_name(&file);
    let cases = [
        (
            "slice",
            format!("error: slice failed at {path}:13:10: slice 1..5 out of range for length 3\n"),
        ),
        (
            "at",
            format!("error: index failed at {path}:18:10: index 3 out of range for length 1\n"),
        ),
        ("div", format!("error: / failed at {path}:22:10: 1 / 0\n")),
        (
            "quantity overflow",
            format!("error: + failed at {path}:27:10: 9223372036854775807 + 1\n"),
        ),
        (
            "neg",
            format!("error: - failed at {path}:31:10: -(-9223372036854775808)\n"),
        ),
        (
            "parse a slice",
            format!("error: slice failed at {path}:36:20: slice 1..5 out of range for length 2\n"),
        ),
        (
            "parse a slice of a slice",
            format!("error: slice failed at {path}:41:20: slice 1..3 out of range for length 2\n"),
        ),
        (
            "text of a slice",
            format!("error: slice failed at {path}:46:10: slice -1..1 out of range for length 2\n"),
        ),
    ];
    for (name, report) in &cases {
        let output = run(
            &executable,
            &["--run-one", &format!("fixtures.failures/{name}")],
        );
        let shown = text(&output);
        assert_eq!(output.status.code(), Some(101), "{name}: {shown}");
        assert!(
            shown.contains(report),
            "{name}: missing `{report}` in\n{shown}"
        );
    }
    let output = run(&executable, &[]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    assert!(
        text(&output).contains("0 passed; 8 failed"),
        "{}",
        text(&output)
    );
}

/// `crashd` killed on drop.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Server {
    process: Running,
    port: u16,
    log: Arc<Mutex<String>>,
    /// The thread that collects stderr; finished once stderr is closed.
    reader: std::thread::JoinHandle<()>,
}

impl Server {
    /// `crashd --port 0`; the port from its `listening on` line, stderr
    /// collected in `log`.
    fn start(crashd: &Path) -> Server {
        let mut child = Command::new(crashd)
            .args(["--port", "0"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn crashd");
        let stderr = child.stderr.take().expect("piped stderr");
        let process = Running(child);
        let (ports, port) = mpsc::channel();
        let log = Arc::new(Mutex::new(String::new()));
        let lines = Arc::clone(&log);
        let reader = std::thread::spawn(move || {
            let mut stderr = std::io::BufReader::new(stderr);
            let mut line = String::new();
            while stderr.read_line(&mut line).unwrap_or(0) > 0 {
                if let Some(rest) = line.trim_end().strip_prefix("listening on 127.0.0.1:") {
                    let _ = ports.send(rest.parse::<u16>());
                }
                lines.lock().unwrap().push_str(&line);
                line.clear();
            }
        });
        let port = port
            .recv_timeout(TIMEOUT)
            .expect("crashd printed no `listening on 127.0.0.1:<port>` line")
            .expect("port in the listening line");
        Server {
            process,
            port,
            log,
            reader,
        }
    }

    /// Waits until stderr holds `text`; the whole log.
    #[track_caller]
    fn logged(&self, text: &str) -> String {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let log = self.log.lock().unwrap().clone();
            if log.contains(text) {
                return log;
            }
            assert!(Instant::now() < deadline, "no `{text}` in stderr: {log}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// A raw connection with the first prompt consumed.
    fn connect(&self) -> TcpStream {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        let mut prompt = vec![0; PROMPT.len()];
        stream.read_exact(&mut prompt).expect("the prompt in time");
        assert_eq!(prompt, PROMPT);
        stream
    }

    #[track_caller]
    fn assert_alive(&mut self) {
        assert!(
            self.process.0.try_wait().expect("try_wait").is_none(),
            "crashd exited"
        );
    }

    /// The exit code, once the process ends within `limit` and `log` holds
    /// all of its stderr (the reader saw the end of the pipe).
    #[track_caller]
    fn wait(&mut self, limit: Duration) -> Option<i32> {
        let deadline = Instant::now() + limit;
        loop {
            if let Some(status) = self.process.0.try_wait().expect("try_wait")
                && self.reader.is_finished()
            {
                return status.code();
            }
            assert!(
                Instant::now() < deadline,
                "still running: {}",
                self.log.lock().unwrap()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// 14.5 end to end: `greeterd` binds its listener on `--port 0` and logs
/// the port, `Greeter.main` runs with the embedded `banner` value, the
/// quantity defaults and the store, and the process exits with the code of
/// `main`'s `Err` (`with_exit_code`, O12) or 0.
#[test]
fn executable_component_main_runs_and_maps_the_exit_code() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/components.ignite");
    let executables = build_program(&file, "components");
    let [greeterd, pinger] = executables.as_slice() else {
        panic!("{executables:?}");
    };
    for (line, code) in [("hello", 0), ("fail", 3)] {
        let mut server = Server::start(greeterd);
        let mut stream = TcpStream::connect(("127.0.0.1", server.port)).expect("connect");
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
        let mut banner = String::new();
        reader.read_line(&mut banner).unwrap();
        assert_eq!(banner, "hello 80 bytes 30 seconds\n");
        send_line(&mut stream, line);
        let status = server.wait(TIMEOUT);
        let log = server.log.lock().unwrap().clone();
        assert_eq!(status, Some(code), "{log}");
        assert_eq!(log.contains("error: asked to fail"), code == 3, "{log}");
    }
    // The client component: its `Endpoint` target takes the `--port`
    // override, it reads the banner and answers, and both exit 0.
    let mut server = Server::start(greeterd);
    let output = Command::new(pinger)
        .args(["--port", &server.port.to_string()])
        .stdin(Stdio::null())
        .output()
        .expect("run pinger");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert_eq!(
        server.wait(TIMEOUT),
        Some(0),
        "{}",
        server.log.lock().unwrap()
    );
}

fn send_line(stream: &mut TcpStream, line: &str) {
    stream.write_all(line.as_bytes()).unwrap();
    stream.write_all(b"\r\n").unwrap();
}

/// Reads until the output ends with the prompt.
fn read_through_prompt(stream: &mut TcpStream) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chunk = [0u8; 256];
    while !out.ends_with(PROMPT) {
        let n = stream.read(&mut chunk).expect("data in time");
        assert!(n > 0, "closed before the prompt: {out:?}");
        out.extend_from_slice(&chunk[..n]);
    }
    out
}

/// Reads until the peer closes; a reset counts as closed.
fn read_until_closed(stream: &mut TcpStream) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chunk = [0u8; 256];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => return out,
            Ok(n) => out.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => return out,
            Err(e) => panic!("{e}"),
        }
    }
}

/// T1 over a real socket: a session whose handler fails ends with the
/// session, is logged with the 14.8 report, and the server keeps
/// answering on the next connection.
#[test]
fn crashd_survives_failing_handlers() {
    let file = fixture("crash.ignite");
    let executables = build_program(&file, "crash");
    let [crashd] = executables.as_slice() else {
        panic!("{executables:?}");
    };
    let mut server = Server::start(crashd);
    let path = common::source_name(&file);

    let mut stream = server.connect();
    send_line(&mut stream, "status");
    assert_eq!(
        read_through_prompt(&mut stream),
        b"ready\r\nterminal> \xff\xf9"
    );

    let failures = [
        (
            "crash assert",
            format!(
                "task ended with error: assert failed at {path}:28:3: kind == \"fine\"\n  message: the handler asserted\n"
            ),
        ),
        (
            "crash overflow",
            format!("task ended with error: + failed at {path}:33:11: 9223372036854775807 + 1\n"),
        ),
        (
            "crash index",
            format!(
                "task ended with error: index failed at {path}:39:11: index 5 out of range for length 2\n"
            ),
        ),
    ];
    for (command, report) in &failures {
        let mut stream = server.connect();
        send_line(&mut stream, command);
        let rest = read_until_closed(&mut stream);
        assert!(
            !rest.ends_with(PROMPT),
            "{command}: the session went on: {rest:?}"
        );
        server.logged(report);
        server.assert_alive();
    }

    let mut stream = server.connect();
    send_line(&mut stream, "status");
    assert_eq!(
        read_through_prompt(&mut stream),
        b"ready\r\nterminal> \xff\xf9"
    );
    send_line(&mut stream, "quit");
    assert_eq!(read_until_closed(&mut stream), b"bye\r\n");
    server.assert_alive();
    let log = server.log.lock().unwrap().clone();
    assert_eq!(log.matches("task ended with error").count(), 3, "{log}");
}
