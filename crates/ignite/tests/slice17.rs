//! Slice 17 (`docs/slice.md` section 21) end to end: the fixtures under
//! `tests/fixtures/slice17/` built with `ignite::build` once per target and
//! test binary, started as real processes and driven over a real socket.
//! Every case runs once per target (`slice17::rust::<case>`,
//! `slice17::llvm::<case>`).
//!
//! Children of a program (rows 13, 14 and 17 of 21.0): `signal_mask.ignite`
//! shows what a child of a server inherits, `orphans.ignite` that children
//! end with the program, however it ends.
//!
//! Arguments (row 18): `arguments.ignite` reads two settings, a directory
//! flag, `--port` and `--ref` from one argument list (21.6).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use ignite::Target;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

use crate::common;

/// The longest a test waits for a line, a reply or an exit.
const TIMEOUT: Duration = Duration::from_secs(15);

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/slice17")
        .join(name)
}

fn tmp(name: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slice17")
        .join(name)
}

fn target_name(target: Target) -> &'static str {
    match target {
        Target::Rust => "rust",
        Target::Llvm => "llvm",
    }
}

/// `source` built once per target into this test binary's own directory;
/// the executables by file name.
fn build(source: &str, target: Target) -> Vec<PathBuf> {
    let out = tmp(format!("build-{source}-{}", target_name(target)));
    let options = match target {
        Target::Rust => ignite::BuildOptions {
            target: Target::Rust,
            out: Some(out),
            ..Default::default()
        },
        Target::Llvm => ignite::BuildOptions {
            target: Target::Llvm,
            out: Some(out),
            runtime: Some(common::runtime_archive()),
            ..Default::default()
        },
    };
    let output = ignite::build(&fixture(source), &options).unwrap_or_else(|e| panic!("{e}"));
    output.executables
}

/// The executable `name` of the fixture `source`, built once per target.
fn executable(source: &'static str, name: &str, target: Target) -> PathBuf {
    static BUILT: Mutex<Vec<(&'static str, &'static str, Vec<PathBuf>)>> = Mutex::new(Vec::new());
    let key = target_name(target);
    let mut built = BUILT.lock().unwrap_or_else(|e| e.into_inner());
    if !built.iter().any(|(s, t, _)| *s == source && *t == key) {
        let executables = build(source, target);
        built.push((source, key, executables));
    }
    let (_, _, executables) = built
        .iter()
        .find(|(s, t, _)| *s == source && *t == key)
        .expect("built just now");
    executables
        .iter()
        .find(|path| path.file_stem().and_then(|s| s.to_str()) == Some(name))
        .unwrap_or_else(|| panic!("no {name} in {executables:?}"))
        .clone()
}

/// An empty working directory of its own for one case and target.
fn workdir(case: &str, target: Target) -> PathBuf {
    let dir = tmp(format!("work-{case}-{}", target_name(target)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the working directory");
    dir
}

/// Lines of a stream as they arrive.
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

/// The next line that starts with `prefix`, without it, within [`TIMEOUT`].
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

fn wait_exit(child: &mut Child, what: &str) -> ExitStatus {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("wait") {
            return status;
        }
        assert!(
            started.elapsed() < TIMEOUT,
            "{what} did not exit within {TIMEOUT:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A server started on a free port in its own working directory; killed
/// on drop.
struct Server {
    child: Child,
    stderr: Receiver<String>,
    port: u16,
}

impl Server {
    fn start(command: &mut Command) -> Server {
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("start {command:?}: {e}"));
        let stderr = lines_of(child.stderr.take().expect("stderr"));
        let listening = wait_line(&stderr, "listening on 127.0.0.1:");
        let port = listening
            .parse()
            .unwrap_or_else(|e| panic!("{listening}: {e}"));
        Server {
            child,
            stderr,
            port,
        }
    }

    fn pid(&self) -> i32 {
        self.child.id() as i32
    }

    fn signal(&self, signal: Signal) {
        kill(Pid::from_raw(self.pid()), signal).unwrap_or_else(|e| panic!("{signal}: {e}"));
    }

    /// `GET path` on a new connection: the status and the body.
    fn get(&self, path: &str) -> (u16, String) {
        self.send("GET", path, "")
    }

    /// `method path` with `body` on a new connection: the status and the
    /// body of the reply.
    fn send(&self, method: &str, path: &str, body: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        stream.set_read_timeout(Some(TIMEOUT)).expect("timeout");
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .expect("write the request");
        let mut seen = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            if let Some(reply) = reply_of(&seen) {
                return reply;
            }
            let n = stream
                .read(&mut buffer)
                .unwrap_or_else(|e| panic!("read: {e}; seen {:?}", text(&seen)));
            assert!(n > 0, "eof before a whole reply; seen {:?}", text(&seen));
            seen.extend_from_slice(&buffer[..n]);
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Status and body of a whole HTTP reply framed by `Content-Length`.
fn reply_of(seen: &[u8]) -> Option<(u16, String)> {
    let end = seen.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = text(&seen[..end]);
    let status = head.split(' ').nth(1)?.parse().ok()?;
    let length: usize = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())
                .flatten()
        })
        .unwrap_or(0);
    let body = seen.get(end + 4..end + 4 + length)?;
    Some((status, text(body)))
}

/// The state letter of `/proc/<pid>/stat`, `None` when there is no such
/// process.
fn state_of(pid: i32) -> Option<char> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit_once(')')?.1.trim_start().chars().next()
}

fn assert_alive(pid: i32) {
    let state = state_of(pid);
    assert!(
        state.is_some_and(|s| s != 'Z'),
        "child {pid} is not running: {state:?}"
    );
}

/// Within 1 s the process `pid` is gone or a zombie.
fn assert_ends(pid: i32, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match state_of(pid) {
            None | Some('Z') => return,
            Some(state) => assert!(
                Instant::now() < deadline,
                "{what}: child {pid} still in state {state} 1 s after the program ended"
            ),
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn mask_server(case: &str, target: Target) -> Server {
    let maskd = executable("signal_mask.ignite", "maskd", target);
    Server::start(
        Command::new(maskd)
            .args(["--port", "0"])
            .current_dir(workdir(case, target)),
    )
}

/// Row 13: the drain blocks SIGTERM in every thread of the server; the
/// child starts with an empty mask, so its own SIGTERM ends it.
fn a_child_of_a_server_gets_sigterm(target: Target) {
    let server = mask_server("sigterm", target);
    let started = Instant::now();
    let (status, body) = server.get("/");
    assert_eq!((status, body.as_str()), (200, "ending 1 code 15"));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "the reply took {:?}",
        started.elapsed()
    );
}

/// Row 14: a descriptor the server inherited without close-on-exec, its
/// listener, its connection, its root directory and its entropy source do
/// not reach a child.
fn a_child_inherits_no_descriptors(target: Target) {
    let maskd = executable("signal_mask.ignite", "maskd", target);
    let server = Server::start(
        Command::new("/bin/sh")
            .args(["-c", "exec 5</dev/null; exec \"$0\" --port 0"])
            .arg(maskd)
            .current_dir(workdir("fds", target)),
    );
    let inherited = std::fs::read_link(format!("/proc/{}/fd/5", server.pid()))
        .expect("the server holds descriptor 5");
    assert_eq!(inherited, Path::new("/dev/null"));
    let (status, body) = server.get("/fds");
    assert_eq!(status, 200, "{body}");
    let listed: Vec<&str> = body.lines().collect();
    assert_eq!(listed, ["0", "1", "2", "3"], "{body:?}");
}

/// Row 17: `orphan` starts `/bin/sleep 30`, prints its id and returns from
/// `main`; the child ends with it.
fn live_children_die_with_the_program(target: Target) {
    let orphan = executable("orphans.ignite", "orphan", target);
    let mut child = Command::new(orphan)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start orphan");
    let stdout: ChildStdout = child.stdout.take().expect("stdout");
    let lines = lines_of(stdout);
    let pid: i32 = lines
        .recv_timeout(TIMEOUT)
        .expect("orphan printed its child's id")
        .parse()
        .expect("an id");
    let status = wait_exit(&mut child, "orphan");
    assert_eq!(status.code(), Some(0), "{status:?}");
    assert_ends(pid, "main returned");
}

/// An `orphand` with one unwaited child; its id.
fn orphand_with_a_child(case: &str, target: Target) -> (Server, i32) {
    let orphand = executable("orphans.ignite", "orphand", target);
    let server = Server::start(
        Command::new(orphand)
            .args(["--port", "0"])
            .current_dir(workdir(case, target)),
    );
    let (status, body) = server.get("/");
    assert_eq!(status, 200, "{body}");
    let pid: i32 = body.parse().unwrap_or_else(|e| panic!("{body}: {e}"));
    assert_alive(pid);
    (server, pid)
}

/// Row 17 after one SIGTERM: the drain ends the server with status 0, and
/// its child with it.
fn orphans_after_drain(target: Target) {
    let (mut server, pid) = orphand_with_a_child("drain", target);
    server.signal(Signal::SIGTERM);
    let status = wait_exit(&mut server.child, "orphand");
    assert_eq!(status.code(), Some(0), "{status:?}");
    assert_ends(pid, "after the drain");
}

/// Row 17 after a second SIGTERM during the drain, which an idle
/// connection holds open.
fn orphans_after_second_sigterm(target: Target) {
    let (mut server, pid) = orphand_with_a_child("second", target);
    let _idle = TcpStream::connect(("127.0.0.1", server.port)).expect("connect");
    std::thread::sleep(Duration::from_millis(50));
    server.signal(Signal::SIGTERM);
    wait_line(&server.stderr, "released ");
    assert!(
        server.child.try_wait().expect("wait").is_none(),
        "the drain ended with a session open"
    );
    assert_alive(pid);
    server.signal(Signal::SIGTERM);
    let status = wait_exit(&mut server.child, "orphand");
    assert_eq!(status.signal(), Some(Signal::SIGTERM as i32), "{status:?}");
    assert_ends(pid, "after the second SIGTERM");
}

/// Row 17 after SIGKILL of the server: `PR_SET_PDEATHSIG` ends the direct
/// child.
fn orphans_after_sigkill(target: Target) {
    let (mut server, pid) = orphand_with_a_child("sigkill", target);
    server.signal(Signal::SIGKILL);
    let status = wait_exit(&mut server.child, "orphand");
    assert_eq!(status.signal(), Some(Signal::SIGKILL as i32), "{status:?}");
    assert_ends(pid, "after SIGKILL");
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
    a_child_of_a_server_gets_sigterm,
    a_child_inherits_no_descriptors,
    live_children_die_with_the_program,
    orphans_after_drain,
    orphans_after_second_sigterm,
    orphans_after_sigkill,
    a_child_status_survives_an_ignored_sigchld,
    settings_read_their_flags,
    a_value_that_is_not_utf8_is_refused,
    adversary_flag_forms,
    adversary_port_ref_root_not_utf8,
    adversary_directory_flag_errors,
    studio_serves_through_its_state,
);

/// Adversary: a server started by a parent that left SIGCHLD ignored (a
/// wrapper's `trap '' CHLD`; the disposition survives exec). Left so, the
/// kernel reaps the server's children itself, so `waitid` and the reap
/// see ECHILD. The substrate resets SIGCHLD to its default at start
/// (`docs/slice.md` 21.5), so the child of row 13 still reports `ending 1
/// code 15`. The wrapper records the `SigIgn` of a child before the exec,
/// which shows the setup took; the server's shows the reset.
fn a_child_status_survives_an_ignored_sigchld(target: Target) {
    let maskd = executable("signal_mask.ignite", "maskd", target);
    let dir = workdir("sigchld", target);
    let server = Server::start(
        Command::new("/bin/sh")
            .args([
                "-c",
                "trap '' CHLD; grep '^SigIgn:' /proc/self/status > wrapper.status; exec \"$0\" --port 0",
            ])
            .arg(maskd)
            .current_dir(&dir),
    );
    let sigchld_ignored = |status: &str| {
        let ignored = status
            .lines()
            .find_map(|line| line.strip_prefix("SigIgn:"))
            .expect("a SigIgn line")
            .trim()
            .to_string();
        let mask = u64::from_str_radix(&ignored, 16).expect("a hex mask");
        mask & (1 << 16) != 0
    };
    let wrapper =
        std::fs::read_to_string(dir.join("wrapper.status")).expect("the wrapper's status");
    assert!(
        sigchld_ignored(&wrapper),
        "SIGCHLD is not ignored: {wrapper}"
    );
    let status = std::fs::read_to_string(format!("/proc/{}/status", server.pid()))
        .expect("the server's status");
    assert!(
        !sigchld_ignored(&status),
        "SIGCHLD stayed ignored in the server"
    );
    let (code, body) = server.get("/");
    assert_eq!((code, body.as_str()), (200, "ending 1 code 15"));
}

// ---- program arguments (U4) ----

/// `show` of `arguments.ignite` started in a working directory of its own
/// with `args`: its exit code, stdout and stderr.
fn show(case: &str, target: Target, args: &[&std::ffi::OsStr]) -> (Option<i32>, String, String) {
    let show = executable("arguments.ignite", "show", target);
    let mut child = Command::new(&show)
        .args(args)
        .current_dir(workdir(case, target))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("start {}: {e}", show.display()));
    let status = wait_exit(&mut child, "show");
    let mut stdout = String::new();
    let mut stderr = String::new();
    child
        .stdout
        .take()
        .expect("stdout")
        .read_to_string(&mut stdout)
        .expect("read stdout");
    child
        .stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut stderr)
        .expect("read stderr");
    (status.code(), stdout, stderr)
}

fn os<'a>(args: &[&'a str]) -> Vec<&'a std::ffi::OsStr> {
    args.iter().map(|arg| std::ffi::OsStr::new(*arg)).collect()
}

/// Row 18: two settings, a directory flag, `--port` and `--ref` read from
/// one argument list; a missing required flag and a flag without a value
/// end the program with exit 2 before it binds a port.
fn settings_read_their_flags(target: Target) {
    let root = tmp(format!("arguments-root-{}", target_name(target)));
    std::fs::create_dir_all(&root).expect("create the root");
    let canonical = std::fs::canonicalize(&root).expect("canonical root");
    let root = root.to_str().expect("a UTF-8 root");
    let peer = "Arguments.peer=127.0.0.1:9";
    let given = [
        "--root",
        root,
        "--tool",
        "/bin/true",
        "--port",
        "0",
        "--ref",
        peer,
    ];
    let (code, stdout, stderr) = show("flags", target, &os(&given));
    assert_eq!(code, Some(0), "{stderr}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 5, "{stdout}");
    assert_eq!(lines[0], "tool /bin/true");
    assert_eq!(lines[1], "mode fast");
    assert_eq!(lines[2], format!("root {}", canonical.display()));
    let port: u16 = lines[3]
        .strip_prefix("listening 127.0.0.1:")
        .unwrap_or_else(|| panic!("{stdout}"))
        .parse()
        .unwrap_or_else(|e| panic!("{stdout}: {e}"));
    assert!(port != 0 && port != 8096, "{stdout}");
    assert!(
        stderr.contains(&format!("listening on 127.0.0.1:{port}")),
        "{stderr}"
    );
    assert_eq!(lines[4], "peer 127.0.0.1:9");

    let mut slow = given.to_vec();
    slow.push("--mode=slow");
    let (code, stdout, stderr) = show("slow", target, &os(&slow));
    assert_eq!(code, Some(0), "{stderr}");
    assert!(stdout.lines().any(|l| l == "mode slow"), "{stdout}");

    let without_tool = ["--root", root, "--port", "0", "--ref", peer];
    let (code, stdout, stderr) = show("no-tool", target, &os(&without_tool));
    assert_eq!(code, Some(2), "{stdout}{stderr}");
    assert_eq!(stderr, "error: --tool: required argument missing\n");
    assert!(stdout.is_empty(), "{stdout}");

    let no_value = [
        "--root",
        root,
        "--mode",
        "--tool",
        "/bin/true",
        "--port",
        "0",
        "--ref",
        peer,
    ];
    let (code, stdout, stderr) = show("no-value", target, &os(&no_value));
    assert_eq!(code, Some(2), "{stdout}{stderr}");
    assert_eq!(stderr, "error: --mode needs a value\n");
    assert!(stdout.is_empty(), "{stdout}");
}

/// 21.6: a flag's value that is not UTF-8 has no `Text`, so the read
/// refuses it with its bytes as `escape_ascii` writes them, exit 2, on both
/// targets; an argument that is not UTF-8 and that no read asks for is
/// ignored like any other unknown argument.
fn a_value_that_is_not_utf8_is_refused(target: Target) {
    use std::os::unix::ffi::OsStrExt;
    let root = tmp(format!("arguments-utf8-{}", target_name(target)));
    std::fs::create_dir_all(&root).expect("create the root");
    let root = root.to_str().expect("a UTF-8 root");
    let bad = std::ffi::OsStr::from_bytes(b"/bin/\xfftrue");
    let bad_joined = std::ffi::OsStr::from_bytes(b"--tool=/bin/\xfftrue");
    let stray = std::ffi::OsStr::from_bytes(b"--stray=\xfe");
    let peer = "Arguments.peer=127.0.0.1:9";
    let rest = os(&["--root", root, "--port", "0", "--ref", peer]);

    for (case, tool) in [
        ("utf8-split", vec![std::ffi::OsStr::new("--tool"), bad]),
        ("utf8-joined", vec![bad_joined]),
    ] {
        let args: Vec<&std::ffi::OsStr> = tool.into_iter().chain(rest.iter().copied()).collect();
        let (code, stdout, stderr) = show(case, target, &args);
        assert_eq!(code, Some(2), "{case}: {stdout}{stderr}");
        assert_eq!(
            stderr, "error: --tool: value /bin/\\xfftrue is not UTF-8\n",
            "{case}"
        );
        assert!(stdout.is_empty(), "{case}: {stdout}");
    }

    let mut args = vec![stray];
    args.extend(os(&["--tool", "/bin/true"]));
    args.extend(rest.iter().copied());
    let (code, stdout, stderr) = show("utf8-stray", target, &args);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(stdout.starts_with("tool /bin/true\n"), "{stdout}");
    assert!(stdout.ends_with("peer 127.0.0.1:9\n"), "{stdout}");
}

// ---- adversary (U4): argument forms, errors on both targets ----

/// `show` of `arguments.ignite` started in `dir` (left as it is) with
/// `args`: its exit code, stdout and stderr.
fn show_in(dir: &Path, target: Target, args: &[&std::ffi::OsStr]) -> (Option<i32>, String, String) {
    let show = executable("arguments.ignite", "show", target);
    let output = Command::new(&show)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("run {}: {e}", show.display()));
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The first line of `show`'s stdout that starts with `prefix`.
fn line_of<'a>(stdout: &'a str, prefix: &str) -> &'a str {
    stdout
        .lines()
        .find(|line| line.starts_with(prefix))
        .unwrap_or_else(|| panic!("no `{prefix}` line in:\n{stdout}"))
}

/// Adversary, 21.6: the forms of a flag. `--flag=V` takes `V` as it is,
/// also empty or starting with `--`; the first of repeated flags wins in
/// either form, and a later flag without a value does not matter; flags
/// are matched exactly (case, prefixes); `--` is an argument like any other
/// (21.6 names no terminator); a Unicode value is kept.
fn adversary_flag_forms(target: Target) {
    let root = tmp(format!("adv-forms-root-{}", target_name(target)));
    std::fs::create_dir_all(&root).expect("create the root");
    let root = root.to_str().expect("a UTF-8 root").to_string();
    let base = [
        "--root",
        root.as_str(),
        "--port",
        "0",
        "--ref",
        "Arguments.peer=127.0.0.1:9",
    ];
    let run = |case: &str, flags: &[&str]| {
        let args: Vec<&str> = flags.iter().copied().chain(base.iter().copied()).collect();
        show(case, target, &os(&args))
    };
    let ok = |case: &str, flags: &[&str], tool: &str, mode: &str| {
        let (code, stdout, stderr) = run(case, flags);
        assert_eq!(code, Some(0), "{case}: {stdout}{stderr}");
        assert_eq!(line_of(&stdout, "tool "), format!("tool {tool}"), "{case}");
        assert_eq!(line_of(&stdout, "mode "), format!("mode {mode}"), "{case}");
    };
    ok("empty-joined", &["--tool="], "", "fast");
    ok("dashes-joined", &["--tool=--mode"], "--mode", "fast");
    ok(
        "first-split",
        &["--tool", "/a", "--tool", "/b"],
        "/a",
        "fast",
    );
    ok("first-joined", &["--tool=/a", "--tool", "/b"], "/a", "fast");
    ok("first-then-bare", &["--tool", "/a", "--tool"], "/a", "fast");
    ok(
        "mode-first",
        &["--mode=x", "--mode", "y", "--tool", "/a"],
        "/a",
        "x",
    );
    ok("terminator", &["--", "--tool", "/a"], "/a", "fast");
    ok("single-dash", &["--tool", "-"], "-", "fast");
    ok("unicode", &["--tool", "/bin/é✓"], "/bin/é✓", "fast");
    ok("empty-split", &["--tool", ""], "", "fast");
    ok("value-equals", &["--tool=a=b"], "a=b", "fast");

    let missing = |case: &str, flags: &[&str]| {
        let (code, stdout, stderr) = run(case, flags);
        assert_eq!(code, Some(2), "{case}: {stdout}{stderr}");
        assert_eq!(
            stderr, "error: --tool: required argument missing\n",
            "{case}"
        );
        assert!(stdout.is_empty(), "{case}: {stdout}");
    };
    missing("upper", &["--TOOL", "/a"]);
    missing("longer", &["--toolbox", "/a", "--tool-x=/b"]);
    missing("single-dash-flag", &["-tool", "/a"]);

    let (code, stdout, stderr) = run("bare-at-end", &["--mode=slow", "--tool"]);
    assert_eq!(code, Some(2), "{stdout}{stderr}");
    assert_eq!(stderr, "error: --tool needs a value\n");
    assert!(stdout.is_empty(), "{stdout}");
}

/// Adversary, 21.6 "This holds for `--port`, `--ref` (each binding is
/// read) and every setting and directory flag": end to end on both
/// targets, each refused with exit 2 and the exact line, before any port
/// is bound (no `listening on`) and before `main` prints.
fn adversary_port_ref_root_not_utf8(target: Target) {
    use std::os::unix::ffi::OsStrExt;
    let root = tmp(format!("adv-utf8-root-{}", target_name(target)));
    std::fs::create_dir_all(&root).expect("create the root");
    let root = root.to_str().expect("a UTF-8 root").to_string();
    let b = |bytes: &'static [u8]| std::ffi::OsStr::from_bytes(bytes);
    let s = std::ffi::OsStr::new;
    let cases: Vec<(&str, Vec<&std::ffi::OsStr>, &str)> = vec![
        (
            "port-split",
            vec![
                s("--tool"),
                s("/a"),
                s("--root"),
                s(root.as_str()),
                s("--port"),
                b(b"\xff"),
                s("--ref"),
                s("Arguments.peer=127.0.0.1:9"),
            ],
            "error: --port: value \\xff is not UTF-8\n",
        ),
        (
            "port-joined",
            vec![
                s("--tool"),
                s("/a"),
                s("--root"),
                s(root.as_str()),
                b(b"--port=1\xff"),
                s("--ref"),
                s("Arguments.peer=127.0.0.1:9"),
            ],
            "error: --port: value 1\\xff is not UTF-8\n",
        ),
        (
            "root-split",
            vec![
                s("--tool"),
                s("/a"),
                s("--root"),
                b(b"/x\xfe"),
                s("--port"),
                s("0"),
                s("--ref"),
                s("Arguments.peer=127.0.0.1:9"),
            ],
            "error: --root: value /x\\xfe is not UTF-8\n",
        ),
        (
            "mode-joined",
            vec![
                s("--tool"),
                s("/a"),
                b(b"--mode=\xc3"),
                s("--root"),
                s(root.as_str()),
                s("--port"),
                s("0"),
                s("--ref"),
                s("Arguments.peer=127.0.0.1:9"),
            ],
            "error: --mode: value \\xc3 is not UTF-8\n",
        ),
    ];
    for (case, args, expected) in &cases {
        let (code, stdout, stderr) = show(case, target, args);
        assert_eq!(code, Some(2), "{case}: {stdout}{stderr}");
        assert_eq!(stderr, *expected, "{case}");
        assert!(stdout.is_empty(), "{case}: {stdout}");
    }
    let args = vec![
        s("--tool"),
        s("/a"),
        s("--root"),
        s(root.as_str()),
        s("--port"),
        s("0"),
        s("--ref"),
        b(b"Arguments.peer=\xff:9"),
    ];
    let (code, stdout, stderr) = show("ref", target, &args);
    assert_eq!(code, Some(2), "ref: {stdout}{stderr}");
    assert!(
        stderr.ends_with("error: --ref: value Arguments.peer=\\xff:9 is not UTF-8\n"),
        "ref: {stderr}"
    );
    assert!(stdout.is_empty(), "ref: {stdout}");
}

/// Adversary, 21.4 with 21.6: a `Directory` flag's value replaces `path`
/// and resolves against the working directory; a missing root, a root that
/// is a file and an empty `--root=` end the program with exit 2 and
/// `error: directory <path>: <reason>` before a port is bound; a relative
/// value and a symlink resolve to the canonical directory.
fn adversary_directory_flag_errors(target: Target) {
    let dir = workdir("adv-dirs", target);
    std::fs::create_dir_all(dir.join("real/sub")).expect("create real/sub");
    std::fs::write(dir.join("file"), b"x").expect("write file");
    std::os::unix::fs::symlink(dir.join("real"), dir.join("link")).expect("symlink");
    let canonical = std::fs::canonicalize(dir.join("real")).expect("canonical real");
    let rest = [
        "--tool",
        "/a",
        "--port",
        "0",
        "--ref",
        "Arguments.peer=127.0.0.1:9",
    ];
    let run = |root: &[&str]| {
        let args: Vec<&str> = root.iter().copied().chain(rest.iter().copied()).collect();
        show_in(&dir, target, &os(&args))
    };
    let refused = |root: &[&str], expected: &str| {
        let (code, stdout, stderr) = run(root);
        assert_eq!(code, Some(2), "{root:?}: {stdout}{stderr}");
        assert_eq!(stderr, expected, "{root:?}");
        assert!(stdout.is_empty(), "{root:?}: {stdout}");
    };
    refused(
        &["--root", "nosuch"],
        "error: directory nosuch: No such file or directory\n",
    );
    refused(
        &["--root", "file"],
        "error: directory file: Not a directory\n",
    );
    refused(&["--root="], "error: directory : not a directory path\n");
    refused(&[], "error: directory missing: No such file or directory\n");
    refused(&["--root"], "error: --root needs a value\n");

    for root in [
        &["--root", "real"][..],
        &["--root=link"],
        &["--root", "./link/sub/.."],
    ] {
        let (code, stdout, stderr) = run(root);
        assert_eq!(code, Some(0), "{root:?}: {stdout}{stderr}");
        assert_eq!(
            line_of(&stdout, "root "),
            format!("root {}", canonical.display()),
            "{root:?}"
        );
    }
}

/// `studio.ignite` started on a free port with its root and tool given as
/// flags, in a working directory of its own.
fn studio_server(case: &str, root: &Path, target: Target) -> Server {
    let studiod = executable("studio.ignite", "studiod", target);
    Server::start(
        Command::new(studiod)
            .args(["--port", "0", "--root"])
            .arg(root)
            .args(["--tool", "/bin/sh"])
            .current_dir(workdir(case, target)),
    )
}

/// Row 24 (P3, 21.3): a handler with no `requires` writes, reads and
/// confines files, runs the `--tool` setting through the launcher in the
/// root, and draws random bytes, all through its state.
fn studio_serves_through_its_state(target: Target) {
    let root = tmp(format!("studio-root-{}", target_name(target)));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create the root");
    let server = studio_server("studio", &root, target);

    assert_eq!(
        server.send("POST", "/write", "a.ignite\nx"),
        (204, String::new())
    );
    assert_eq!(
        std::fs::read_to_string(root.join("a.ignite")).expect("read a.ignite"),
        "x"
    );
    assert_eq!(
        server.send("POST", "/read", "a.ignite"),
        (200, "x".to_string())
    );
    assert_eq!(
        server.send("POST", "/read", "../x"),
        (400, "path ../x: leaves its root".to_string())
    );
    assert_eq!(server.send("POST", "/run", ""), (200, "x".to_string()));

    let token = |server: &Server| {
        let (status, body) = server.get("/token");
        assert_eq!(status, 200, "{body}");
        assert_eq!(body.len(), 64, "{body}");
        assert!(
            body.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "{body}"
        );
        body
    };
    let first = token(&server);
    drop(server);
    let again = studio_server("studio-again", &root, target);
    assert_ne!(first, token(&again), "two launches drew the same token");
}
