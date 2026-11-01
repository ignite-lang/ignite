//! The LLVM target (`docs/llvm.md`): small programs built with clang and
//! run, and the bypass proof of section 8 items 1 and 2: the terminal
//! example builds with no cargo or rustc reachable, and its binaries hold
//! `std.protocol.telnet.*` symbols and no Rust-generated Telos code.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use telos::Target;

mod common;

const TIMEOUT: Duration = Duration::from_secs(10);

fn tmp(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

struct Probes {
    probe: PathBuf,
    crash: PathBuf,
}

fn probes() -> &'static Probes {
    static BUILT: OnceLock<Probes> = OnceLock::new();
    BUILT.get_or_init(|| {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe.telos");
        let options = telos::BuildOptions {
            target: Target::Llvm,
            out: Some(tmp("llvm-probe")),
            runtime: Some(common::runtime_archive()),
            ..Default::default()
        };
        let output = telos::build(&source, &options).unwrap_or_else(|e| panic!("{e}"));
        let out = tmp("llvm-probe");
        for name in ["std.ll", "program.ll", "main_probe.ll", "main_crash.ll"] {
            assert!(out.join("llvm").join(name).is_file(), "{name} missing");
        }
        assert_eq!(
            output.executables,
            [out.join("bin/probe"), out.join("bin/crash")]
        );
        Probes {
            probe: output.executables[0].clone(),
            crash: output.executables[1].clone(),
        }
    })
}

/// Waits for `child` to exit; its exit code, standard output and error.
fn finish(mut child: std::process::Child) -> (Option<i32>, String, String) {
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("the program did not exit");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    (status.code(), stdout, stderr)
}

fn spawn(executable: &Path, port: u16) -> std::process::Child {
    Command::new(executable)
        .args(["--port", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn")
}

/// Records, enums, lists with growth, text builtins, match on tags,
/// payloads, integers and text, `?`, constants, fn values, loops, value
/// semantics on copy, `spawn` and the client exit status 4 on `Err`.
#[test]
fn probe_runs_every_check() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let child = spawn(&probes().probe, port);
    let (mut stream, _) = listener.accept().unwrap();
    stream.set_read_timeout(Some(TIMEOUT)).unwrap();
    let mut greeting = [0u8; 13];
    stream.read_exact(&mut greeting).unwrap();
    assert_eq!(&greeting, b"spawned hello");
    stream.write_all(b"go").unwrap();
    let (code, stdout, stderr) = finish(child);
    let expected = [
        "got go", "12", "12", "0", "one", "many", "2", "a1b2", "x,y", "300", "AAA", "9", "3",
        "odd: 3", "hi", "text ok", "rest", "flags ok", "5",
    ];
    assert_eq!(stdout.lines().collect::<Vec<_>>(), expected);
    assert_eq!(code, Some(4), "stderr: {stderr}");
    assert_eq!(stderr, "error: odd: 1\n");
}

#[test]
fn index_out_of_range_panics_with_101() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (code, stdout, stderr) = finish(spawn(&probes().crash, port));
    drop(listener);
    assert_eq!(code, Some(101), "stderr: {stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("index out of range"), "stderr: {stderr}");
}

#[test]
fn client_exits_3_when_nothing_listens() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let (code, _, stderr) = finish(spawn(&probes().probe, port));
    assert_eq!(code, Some(3), "stderr: {stderr}");
    assert!(stderr.starts_with("error: "), "stderr: {stderr}");
}

/// The symbol names in `binary`, from `nm`.
fn symbols(binary: &Path) -> String {
    let output = Command::new("nm").arg(binary).output().expect("run nm");
    assert!(output.status.success(), "nm {}", binary.display());
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// `docs/llvm.md` section 8 items 1 and 2.
#[test]
fn llvm_build_runs_neither_cargo_nor_rustc() {
    let tools = tmp("llvm-bypass-tools");
    let _ = std::fs::remove_dir_all(&tools);
    std::fs::create_dir_all(&tools).unwrap();
    for tool in ["clang", "ld"] {
        let found = which(tool).unwrap_or_else(|| panic!("{tool} not on PATH"));
        std::os::unix::fs::symlink(found, tools.join(tool)).unwrap();
    }
    for forbidden in ["cargo", "rustc"] {
        assert!(!tools.join(forbidden).exists());
    }
    let out = tmp("llvm-bypass");
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/terminal/terminal.telos");
    let output = Command::new(env!("CARGO_BIN_EXE_telos"))
        .arg("build")
        .arg("--target")
        .arg("llvm")
        .arg(&source)
        .arg("--out")
        .arg(&out)
        .arg("--clang")
        .arg(tools.join("clang"))
        .arg("--runtime")
        .arg(common::runtime_archive())
        .env_clear()
        .env("PATH", &tools)
        .env("CARGO", "/bin/false")
        .env("RUSTC", "/bin/false")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines,
        [
            format!("built {}", out.join("bin/terminald").display()),
            format!("built {}", out.join("bin/tn").display()),
        ]
    );
    for executable in ["terminald", "tn"] {
        let names = symbols(&out.join("bin").join(executable));
        assert!(
            names.contains(" T std.protocol.telnet.decode"),
            "{executable} has no std.protocol.telnet.decode"
        );
        assert!(names.contains(" T std.protocol.telnet.Telnet."));
        assert_eq!(
            names.contains(" T examples.terminal.execute"),
            executable == "terminald",
            "only terminald reaches execute; --gc-sections drops it from tn"
        );
        assert!(
            !names.contains("telos_std"),
            "{executable} holds telos_std code"
        );
        assert!(
            !names.contains("_ZN8terminal"),
            "{executable} holds program crate code"
        );
    }
}

fn which(tool: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(tool))
            .find(|path| path.is_file())
    })
}

/// `telos build --target llvm` with a clang that fails is exit 2.
#[test]
fn clang_failure_is_exit_2() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe.telos");
    let output = Command::new(env!("CARGO_BIN_EXE_telos"))
        .arg("build")
        .arg("--target")
        .arg("llvm")
        .arg(&source)
        .arg("--out")
        .arg(tmp("llvm-clang-fails"))
        .arg("--clang")
        .arg("/bin/false")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}
