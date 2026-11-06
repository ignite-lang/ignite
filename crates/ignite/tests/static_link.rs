//! `ignite build --static` (`docs/slice.md` 23): the todo example linked
//! against musl on both targets has no program interpreter and needs no
//! shared library, and `todod` serves HTTP when it runs with an empty
//! environment from a directory that holds nothing but the binary. A
//! musl target that is not installed is exit 2 naming it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn ignite(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(args)
        .current_dir(workspace())
        .output()
        .expect("start ignite")
}

/// `ignite build examples/todo/todo.ignite --static` with `extra`; the
/// built executables, `todod` first.
fn build_static(out: &Path, extra: &[&str]) -> Vec<PathBuf> {
    let out_text = out.to_str().unwrap();
    let mut args = vec![
        "build",
        "examples/todo/todo.ignite",
        "--static",
        "--out",
        out_text,
    ];
    args.extend(extra);
    let output = ignite(&args);
    assert!(
        output.status.success(),
        "ignite {args:?}: {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut built: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix("built "))
        .map(PathBuf::from)
        .collect();
    built.sort_by_key(|path| path.file_name().map(|name| name != "todod"));
    assert_eq!(
        built.first().and_then(|path| path.file_name()),
        Some("todod".as_ref()),
        "{built:?}"
    );
    built
}

fn u16_at(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}

/// The program header types of a little-endian ELF64 file and the
/// `DT_NEEDED` entries of its dynamic section, if it has one.
fn elf_interp_and_needed(path: &Path) -> (bool, usize) {
    const PT_DYNAMIC: u32 = 2;
    const PT_INTERP: u32 = 3;
    const DT_NEEDED: u64 = 1;
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[..4], b"\x7fELF", "{} is no ELF file", path.display());
    assert_eq!((bytes[4], bytes[5]), (2, 1), "ELF64 little-endian");
    let phoff = u64_at(&bytes, 0x20) as usize;
    let phentsize = u16_at(&bytes, 0x36);
    let phnum = u16_at(&bytes, 0x38);
    let mut interp = false;
    let mut needed = 0;
    for index in 0..phnum {
        let header = phoff + index * phentsize;
        match u32_at(&bytes, header) {
            PT_INTERP => interp = true,
            PT_DYNAMIC => {
                let offset = u64_at(&bytes, header + 8) as usize;
                let size = u64_at(&bytes, header + 32) as usize;
                for entry in (offset..offset + size).step_by(16) {
                    if u64_at(&bytes, entry) == DT_NEEDED {
                        needed += 1;
                    }
                }
            }
            _ => {}
        }
    }
    (interp, needed)
}

/// No `PT_INTERP`, no `DT_NEEDED`; `file` calls it statically linked and
/// `ldd` not a dynamic executable.
fn assert_static(path: &Path) {
    assert_eq!(
        elf_interp_and_needed(path),
        (false, 0),
        "{}: PT_INTERP and DT_NEEDED count",
        path.display()
    );
    let file = Command::new("file").arg(path).output().expect("run file");
    let file = String::from_utf8_lossy(&file.stdout);
    assert!(file.contains("statically linked"), "{file}");
    let ldd = Command::new("ldd").arg(path).output().expect("run ldd");
    let ldd = format!(
        "{}{}",
        String::from_utf8_lossy(&ldd.stdout),
        String::from_utf8_lossy(&ldd.stderr)
    );
    assert!(ldd.contains("not a dynamic executable"), "{ldd}");
}

/// A child process, killed on drop.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// One HTTP/1.1 exchange on a new connection that the request closes;
/// the whole reply.
fn exchange(port: u16, request: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to todod");
    stream.set_read_timeout(Some(TIMEOUT)).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut reply = Vec::new();
    stream.read_to_end(&mut reply).expect("read the reply");
    String::from_utf8(reply).unwrap()
}

/// `todod` copied alone into a new directory and started there with an
/// empty environment: it creates a todo and lists it.
fn serves_with_an_empty_environment(todod: &Path, name: &str) {
    let dir = tmp(name);
    let alone = dir.join("todod");
    std::fs::copy(todod, &alone).unwrap();
    let mut child = Command::new(&alone)
        .args(["--port", "0"])
        .env_clear()
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn todod");
    let stderr = child.stderr.take().unwrap();
    let _running = Running(child);
    let (ports, port) = mpsc::channel();
    std::thread::spawn(move || {
        let mut stderr = BufReader::new(stderr);
        let mut line = String::new();
        while stderr.read_line(&mut line).unwrap_or(0) > 0 {
            if let Some(rest) = line.trim_end().strip_prefix("listening on 127.0.0.1:") {
                let _ = ports.send(rest.parse::<u16>());
            }
            line.clear();
        }
    });
    let port = port
        .recv_timeout(TIMEOUT)
        .expect("todod printed no listening line")
        .expect("port in the listening line");
    let body = r#"{"title":"milk"}"#;
    let created = exchange(
        port,
        &format!(
            "POST /todos HTTP/1.1\r\nHost: x\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    );
    assert!(created.starts_with("HTTP/1.1 201 "), "{created}");
    assert!(
        created.ends_with(r#"{"id":1,"title":"milk","done":false}"#),
        "{created}"
    );
    let listed = exchange(
        port,
        "GET /todos HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
    );
    assert!(listed.starts_with("HTTP/1.1 200 "), "{listed}");
    assert!(
        listed.ends_with(r#"[{"id":1,"title":"milk","done":false}]"#),
        "{listed}"
    );
}

#[test]
fn llvm_todo_is_static_and_serves_with_an_empty_environment() {
    let out = tmp("static-todo-llvm");
    let built = build_static(&out, &[]);
    for path in &built {
        assert_static(path);
    }
    serves_with_an_empty_environment(&built[0], "static-todo-llvm-run");
}

#[test]
fn rust_todo_is_static_and_serves_with_an_empty_environment() {
    let out = tmp("static-todo-rust");
    let target_dir = out.join("target");
    let built = build_static(
        &out,
        &[
            "--target",
            "rust",
            "--target-dir",
            target_dir.to_str().unwrap(),
        ],
    );
    for path in &built {
        assert_static(path);
    }
    serves_with_an_empty_environment(&built[0], "static-todo-rust-run");
}

/// Every regular file named `name` under `dir`.
fn find(dir: &Path, name: &str, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            find(&path, name, found);
        } else if entry.file_name() == name {
            found.push(path);
        }
    }
}

/// `ignite test --static` links each target's test executable against
/// musl, and the inline tests of `std.json` and `std.process` (which
/// starts and kills children) pass in them.
#[test]
fn ignite_test_static_runs_the_inline_tests_in_static_executables() {
    let out = tmp("static-ignite-test");
    let output = ignite(&[
        "test",
        "std/json.ignite",
        "std/process.ignite",
        "--target",
        "all",
        "--static",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "{:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut executables = Vec::new();
    find(&out, "ignite-test", &mut executables);
    executables.retain(|path| {
        let inside = path.strip_prefix(&out).unwrap();
        inside
            .components()
            .any(|part| part.as_os_str() == "llvm-static" || part.as_os_str() == "rust-static")
            && !inside.components().any(|part| part.as_os_str() == "target")
    });
    assert_eq!(executables.len(), 2, "{executables:?}");
    for path in &executables {
        assert_static(path);
    }
}

/// D4: a musl target whose Rust target is not installed is a stated
/// error, exit 2, naming the target and how to install it; never a
/// dynamic build. `aarch64-unknown-linux-musl` is not installed on the
/// hosts this suite runs on; the test says so when it is.
#[test]
fn static_without_the_musl_target_exits_2_naming_it() {
    const TRIPLE: &str = "aarch64-unknown-linux-musl";
    let installed = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).contains(TRIPLE));
    assert!(
        !installed,
        "{TRIPLE} is installed: this test needs a host without it"
    );
    for target in ["llvm", "rust"] {
        let out = tmp(&format!("static-missing-{target}"));
        let output = ignite(&[
            "build",
            "examples/todo/todo.ignite",
            "--static",
            "--triple",
            TRIPLE,
            "--target",
            target,
            "--out",
            out.to_str().unwrap(),
        ]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{target}: {stderr}");
        assert!(
            stderr.contains(&format!("rustup target add {TRIPLE}")),
            "{target}: {stderr}"
        );
        assert!(
            !out.join("bin/todod").exists(),
            "{target}: a binary was written"
        );
    }
}

/// `--static` links against musl only: a glibc triple is exit 2.
#[test]
fn static_with_a_glibc_triple_exits_2() {
    let out = tmp("static-gnu");
    let output = ignite(&[
        "build",
        "examples/todo/todo.ignite",
        "--static",
        "--triple",
        "x86_64-unknown-linux-gnu",
        "--out",
        out.to_str().unwrap(),
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("musl"), "{stderr}");
}
