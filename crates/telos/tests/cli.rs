//! The `telos` binary: exit statuses and output of `check`, `emit` and
//! `build`, with the repository's standard library and with small
//! standard-library fixtures written under the cargo target directory.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn telos(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_telos"))
        .args(args)
        .current_dir(workspace())
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// `^<path>:\d+:\d+: error\[<code>\]` where `<path>` ends with `file`.
fn is_diagnostic_line(line: &str, file: &str, code: &str) -> bool {
    let Some(at) = line.find(&format!("{file}:")) else {
        return false;
    };
    if !line[..at].is_empty() && !line[..at].ends_with('/') {
        return false;
    }
    let rest = &line[at + file.len() + 1..];
    let mut parts = rest.splitn(3, ':');
    let line_no = parts.next().unwrap_or("");
    let col = parts.next().unwrap_or("");
    let tail = parts.next().unwrap_or("");
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    digits(line_no) && digits(col) && tail.starts_with(&format!(" error[{code}]"))
}

/// A fresh directory under the cargo target directory with `files`
/// (relative path, contents) written into it.
fn fixture(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cli-{name}"));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    for (path, contents) in files {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
    dir
}

fn arg(path: &Path) -> &str {
    path.to_str().unwrap()
}

const PROGRAM: &str = "package demo\n\nfn f(x: Int) -> Int {\n  return x\n}\n";

const GREET: &str = r#"package std.greet

protocol Greeting {
  fn serve(stream: TcpStream, text: Text) -> Result<Unit, IoError> requires [stream.write] {
    stream.write_all(text.to_bytes())?
    stream.close()?
    return Ok(Unit)
  }

  fn connect(stream: TcpStream) -> Result<Unit, IoError> requires [stream.read, stdout.write] {
    loop {
      let input = stream.read(1024)?
      match input {
        Eof => { return Ok(Unit) }
        Data(bytes) => { Stdout.write_all(bytes)? }
      }
    }
  }
}
"#;

const HELLO: &str = r#"package hello

import std.greet.Greeting

system Hello {
  resource ingress: TcpListener { address = "127.0.0.1", port = 0 }
  component server: Service {
    listener = ingress
    protocol = Greeting
    application = "hello from telos\n"
    executable = "hellod"
  }
  component client: Client { target = ingress, protocol = Greeting, executable = "hello" }
}
"#;

#[test]
fn check_acceptance_program_is_silent() {
    let output = telos(&["check", "examples/terminal/terminal.telos"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(output.stdout.is_empty(), "stdout: {}", text(&output.stdout));
    assert!(output.stderr.is_empty(), "stderr: {}", text(&output.stderr));
}

#[test]
fn check_unknown_name_is_e003() {
    let dir = fixture(
        "unknown-name",
        &[(
            "program.telos",
            "package demo\n\nfn f(x: Int) -> Int {\n  return y\n}\n",
        )],
    );
    let output = telos(&["check", arg(&dir.join("program.telos"))]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = text(&output.stderr);
    let first = stderr.lines().next().unwrap_or("");
    assert!(
        is_diagnostic_line(first, "program.telos", "E003"),
        "stderr: {stderr}"
    );
}

#[test]
fn check_reports_unsupported_component_kind() {
    let dir = fixture(
        "e020",
        &[(
            "program.telos",
            "package demo\n\nsystem S {\n  component w: Worker { executable = \"w\" }\n}\n",
        )],
    );
    let output = telos(&["check", arg(&dir.join("program.telos"))]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        is_diagnostic_line(stderr.lines().next().unwrap_or(""), "program.telos", "E020"),
        "stderr: {stderr}"
    );
}

#[test]
fn emit_prints_the_program_and_the_std_crate() {
    let output = telos(&["emit", "examples/terminal/terminal.telos"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    assert!(stdout.contains("pub fn serve("), "{stdout}");
    assert!(stdout.contains("[[bin]]"));
    assert!(stdout.contains("name = \"terminald\""));
    assert!(stdout.contains("telos-std = { path = \"std\" }"));
    assert!(stdout.contains("pub mod protocol {"));
    let headers: Vec<&str> = stdout
        .lines()
        .filter(|l| l.starts_with("// ==== "))
        .collect();
    assert_eq!(
        headers,
        [
            "// ==== Cargo.toml ====",
            "// ==== src/lib.rs ====",
            "// ==== src/bin/terminald.rs ====",
            "// ==== src/bin/tn.rs ====",
            "// ==== std/Cargo.toml ====",
            "// ==== std/src/lib.rs ====",
        ]
    );
}

#[test]
fn emit_uses_given_substrate() {
    let output = telos(&[
        "emit",
        "examples/terminal/terminal.telos",
        "--substrate",
        "/opt/telos-io",
    ]);
    assert_eq!(output.status.code(), Some(0));
    assert!(text(&output.stdout).contains("telos-io = { path = \"/opt/telos-io\" }"));
}

#[test]
fn emit_with_std_fixture() {
    let dir = fixture(
        "emit-std",
        &[
            ("std/greet.telos", GREET),
            (
                "std/nested/deeper/names.telos",
                "package std.nested.deeper.names\n\nfn name() -> Text {\n  return \"n\"\n}\n",
            ),
            ("hello.telos", HELLO),
        ],
    );
    let output = telos(&[
        "emit",
        arg(&dir.join("hello.telos")),
        "--std",
        arg(&dir.join("std")),
    ]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    assert!(
        stdout.contains("use telos_std::greet::Greeting;"),
        "{stdout}"
    );
    assert!(stdout.contains("pub mod greet {"), "{stdout}");
    assert!(stdout.contains("pub mod nested {"), "{stdout}");
    assert!(stdout.contains("pub mod deeper {"), "{stdout}");
    assert!(
        stdout.contains("Greeting::connect(stream).map_err("),
        "{stdout}"
    );
}

#[test]
fn std_package_mismatch_is_e031_in_the_std_file() {
    let dir = fixture(
        "e031",
        &[
            (
                "std/text.telos",
                "package std.txt\n\nfn f() -> Int {\n  return 1\n}\n",
            ),
            ("program.telos", PROGRAM),
        ],
    );
    let output = telos(&[
        "check",
        arg(&dir.join("program.telos")),
        "--std",
        arg(&dir.join("std")),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    let first = stderr.lines().next().unwrap_or("");
    assert!(
        is_diagnostic_line(first, "std/text.telos", "E031"),
        "{stderr}"
    );
    assert!(first.contains("text.telos:1:9:"), "{stderr}");
    assert!(first.contains("`std.text`"), "{stderr}");
}

#[test]
fn std_file_without_package_is_e031() {
    let dir = fixture(
        "e031-missing",
        &[
            ("std/text.telos", "fn f() -> Int {\n  return 1\n}\n"),
            ("program.telos", PROGRAM),
        ],
    );
    let output = telos(&[
        "check",
        arg(&dir.join("program.telos")),
        "--std",
        arg(&dir.join("std")),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        is_diagnostic_line(
            stderr.lines().next().unwrap_or(""),
            "std/text.telos",
            "E031"
        ),
        "{stderr}"
    );
}

#[test]
fn std_import_cycle_is_e032() {
    let dir = fixture(
        "e032",
        &[
            (
                "std/a.telos",
                "package std.a\n\nimport std.b.{g}\n\nfn f() -> Int {\n  return g()\n}\n",
            ),
            (
                "std/b.telos",
                "package std.b\n\nimport std.c.h\n\nfn g() -> Int {\n  return h()\n}\n",
            ),
            (
                "std/c.telos",
                "package std.c\n\nimport std.a.{f}\n\nfn h() -> Int {\n  return f()\n}\n",
            ),
            (
                "std/z.telos",
                "package std.z\n\nfn z() -> Int {\n  return 0\n}\n",
            ),
            ("program.telos", PROGRAM),
        ],
    );
    let output = telos(&[
        "check",
        arg(&dir.join("program.telos")),
        "--std",
        arg(&dir.join("std")),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    let first = stderr.lines().next().unwrap_or("");
    assert!(is_diagnostic_line(first, "std/a.telos", "E032"), "{stderr}");
    assert!(first.contains("a.telos:3:1:"), "{stderr}");
    assert!(
        first.ends_with("import cycle: std.a -> std.b -> std.c -> std.a"),
        "{stderr}"
    );
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
}

#[test]
fn std_modules_are_checked_in_import_order() {
    let dir = fixture(
        "order",
        &[
            (
                "std/a.telos",
                "package std.a\n\nimport std.b.{g}\n\nfn f() -> Int {\n  return g()\n}\n",
            ),
            (
                "std/b.telos",
                "package std.b\n\nfn g() -> Int {\n  return 1\n}\n",
            ),
            (
                "program.telos",
                "package demo\n\nimport std.a.f\n\nfn main_value() -> Int {\n  return f()\n}\n",
            ),
        ],
    );
    let output = telos(&[
        "check",
        arg(&dir.join("program.telos")),
        "--std",
        arg(&dir.join("std")),
    ]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
}

#[test]
fn std_diagnostic_is_rendered_with_its_file() {
    let dir = fixture(
        "std-e003",
        &[
            (
                "std/bad.telos",
                "package std.bad\n\nfn f() -> Int {\n  return nothing\n}\n",
            ),
            ("program.telos", PROGRAM),
        ],
    );
    let output = telos(&[
        "check",
        arg(&dir.join("program.telos")),
        "--std",
        arg(&dir.join("std")),
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    let first = stderr.lines().next().unwrap_or("");
    assert!(
        is_diagnostic_line(first, "std/bad.telos", "E003"),
        "{stderr}"
    );
    assert!(first.contains("bad.telos:4:10:"), "{stderr}");
}

#[test]
fn missing_std_root_is_exit_3() {
    let dir = fixture("no-std", &[("program.telos", PROGRAM)]);
    let output = telos(&[
        "check",
        arg(&dir.join("program.telos")),
        "--std",
        arg(&dir.join("std")),
    ]);
    assert_eq!(output.status.code(), Some(3));
    assert!(text(&output.stderr).contains("std"));
}

#[test]
fn missing_file_is_exit_3() {
    let output = telos(&["check", "examples/terminal/does-not-exist.telos"]);
    assert_eq!(output.status.code(), Some(3));
    assert!(text(&output.stderr).contains("does-not-exist.telos"));
}

#[test]
fn no_arguments_is_exit_2() {
    let output = telos(&[]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn default_substrate_is_the_sibling_crate() {
    let path = telos::default_substrate_path();
    assert!(path.join("Cargo.toml").exists(), "{}", path.display());
    assert!(path.ends_with("telos-io"));
}

#[test]
fn default_std_is_the_repository_std() {
    let path = telos::default_std_path();
    assert!(path.is_absolute(), "{}", path.display());
    assert!(path.ends_with("std"), "{}", path.display());
    assert!(path.join("console.telos").exists(), "{}", path.display());
}

/// The cargo target directory of this test run, shared so the substrate's
/// dependencies are built once.
fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace().join("target"))
}

#[test]
fn build_std_and_program_then_run_both_executables() {
    let dir = fixture(
        "build",
        &[("std/greet.telos", GREET), ("hello.telos", HELLO)],
    );
    let out = dir.join("out");
    let target = target_dir();
    let output = telos(&[
        "build",
        arg(&dir.join("hello.telos")),
        "--std",
        arg(&dir.join("std")),
        "--out",
        arg(&out),
        "--target-dir",
        arg(&target),
    ]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    let built: Vec<&str> = stdout.lines().collect();
    let hellod = target.join("debug").join("hellod");
    let hello = target.join("debug").join("hello");
    assert_eq!(
        built,
        [
            format!("built {}", hellod.display()),
            format!("built {}", hello.display())
        ]
    );
    assert!(out.join("std/Cargo.toml").exists());
    assert!(out.join("std/src/lib.rs").exists());

    let mut server = Command::new(&hellod)
        .args(["--port", "0"])
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(server.stderr.take().unwrap()).lines();
    let listening = lines.next().unwrap().unwrap();
    let port = listening
        .strip_prefix("listening on 127.0.0.1:")
        .unwrap_or_else(|| panic!("{listening}"))
        .to_string();
    let mut client = Command::new(&hello)
        .args(["--port", &port])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let status = loop {
        if let Some(status) = client.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > Duration::from_secs(10) {
            client.kill().unwrap();
            server.kill().unwrap();
            panic!("client did not exit");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut received = String::new();
    std::io::Read::read_to_string(&mut client.stdout.take().unwrap(), &mut received).unwrap();
    server.kill().unwrap();
    let _ = server.wait();
    assert!(status.success());
    assert_eq!(received, "hello from telos\n");
}

const FAILING: &str = r#"package std.failing

protocol Failing {
  fn connect(stream: TcpStream) -> Result<Unit, IoError> requires [stream.write] {
    stream.close()?
    return Err(IoError.new("role failed"))
  }
}
"#;

const EXITS: &str = r#"package exits

import std.greet.Greeting
import std.failing.Failing

system Exits {
  resource ingress: TcpListener { address = "127.0.0.1", port = 1 }
  component plain: Client { target = ingress, protocol = Greeting, executable = "exits-plain" }
  component failing: Client { target = ingress, protocol = Failing, executable = "exits-failing" }
}
"#;

/// Run `executable` with `args` and stdin closed; its exit code and stderr.
fn run_client(executable: &Path, args: &[&str]) -> (Option<i32>, String) {
    let output = Command::new(executable)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (output.status.code(), text(&output.stderr))
}

#[test]
fn client_exit_statuses() {
    let dir = fixture(
        "exits",
        &[
            ("std/greet.telos", GREET),
            ("std/failing.telos", FAILING),
            ("exits.telos", EXITS),
        ],
    );
    let target = target_dir();
    let output = telos(&[
        "build",
        arg(&dir.join("exits.telos")),
        "--std",
        arg(&dir.join("std")),
        "--out",
        arg(&dir.join("out")),
        "--target-dir",
        arg(&target),
    ]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let plain = target.join("debug").join("exits-plain");
    let failing = target.join("debug").join("exits-failing");

    let (code, stderr) = run_client(&plain, &["--port", "abc"]);
    assert_eq!(code, Some(2), "{stderr}");
    assert!(stderr.starts_with("error: "), "{stderr}");
    let (code, stderr) = run_client(&plain, &["--port", "70000"]);
    assert_eq!(code, Some(2), "{stderr}");

    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = closed.local_addr().unwrap().port().to_string();
    drop(closed);
    let (code, stderr) = run_client(&plain, &["--port", &port]);
    assert_eq!(code, Some(3), "{stderr}");
    assert!(stderr.starts_with("error: connect 127.0.0.1:"), "{stderr}");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let (code, stderr) = run_client(&failing, &["--port", &port]);
    drop(listener);
    assert_eq!(code, Some(4), "{stderr}");
    assert_eq!(stderr, "error: role failed\n");
}
