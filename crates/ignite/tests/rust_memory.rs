//! Story `rust-peak-memory-at-one-connection` (row 3.8 of
//! `docs/quality.md`): what of a Rust release executable is resident
//! memory while it serves one connection, read from the executable and from
//! `/proc`. Nothing here is timed.
//!
//! A release executable carries the standard library's backtrace
//! symbolizer, about 170 kB of code that runs only to print a backtrace.
//! The kernel maps a file's pages around every page a process touches, so
//! where the linker leaves those functions between the ones a server runs,
//! their pages are resident memory of every process. The release link puts
//! them in one run at the start of the code, where only the end of the run
//! is next to code that runs.

use std::io::{BufRead as _, BufReader, Read as _, Seek as _, SeekFrom, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// The ceiling of one read from a server: a server that does not answer
/// fails the test instead of hanging it. No duration is measured.
const TIMEOUT: Duration = Duration::from_secs(30);

/// How the functions of the backtrace symbolizer are named in a symbol:
/// the module of the standard library that walks and prints a backtrace
/// and the crates it reads debug information with, each with the length
/// that precedes an identifier in a mangled name.
const SYMBOLIZER: [&str; 5] = [
    "12backtrace_rs",
    "5gimli",
    "9addr2line",
    "14rustc_demangle",
    "11miniz_oxide",
];

/// What of the symbolizer may be resident beside code that runs: the pages
/// the kernel maps around one page fault (`fault_around_bytes`, 64 KiB) and
/// the 64 KiB of the page cache next to them.
const BESIDE: u64 = 2 * 64 * 1024;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The release executable `executable` of the example at `source` on the
/// Rust target, built into this test's own directory.
fn release_executable(source: &str, executable: &str) -> PathBuf {
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("rust-memory");
    let options = ignite::BuildOptions {
        target: ignite::Target::Rust,
        release: true,
        out: Some(scratch.join(executable)),
        target_dir: Some(scratch.join("cargo")),
        ..Default::default()
    };
    let built = ignite::build(&workspace().join(source), &options)
        .unwrap_or_else(|error| panic!("{executable}: {error}"));
    built
        .executables
        .into_iter()
        .find(|path| path.file_stem().and_then(|stem| stem.to_str()) == Some(executable))
        .unwrap_or_else(|| panic!("no {executable} among the executables"))
}

/// A function of an executable: where it starts, its size and its symbol.
struct Function {
    start: u64,
    size: u64,
    name: String,
}

impl Function {
    fn symbolizer(&self) -> bool {
        SYMBOLIZER.iter().any(|part| self.name.contains(part))
    }
}

/// The functions of `executable` that have a size, by address (`nm -S`).
fn functions(executable: &Path) -> Vec<Function> {
    let output = Command::new("nm")
        .args(["-S", "--defined-only"])
        .arg(executable)
        .output()
        .expect("run nm");
    assert!(output.status.success(), "nm {}", executable.display());
    let mut functions: Vec<Function> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let [start, size, kind, name] = fields[..] else {
                return None;
            };
            let size = u64::from_str_radix(size, 16).ok()?;
            (matches!(kind, "t" | "T") && size > 0).then(|| Function {
                start: u64::from_str_radix(start, 16).expect("an address"),
                size,
                name: name.to_string(),
            })
        })
        .collect();
    functions.sort_by_key(|function| function.start);
    functions
}

/// A child process, killed on drop.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `executable --port 0` in a directory of its own, and the port it
/// listens on.
fn serve(executable: &Path) -> (Running, u16) {
    let name = executable.file_name().expect("a file name");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("rust-memory")
        .join("run")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut child = Command::new(executable)
        .args(["--port", "0"])
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the server");
    let stderr = BufReader::new(child.stderr.take().expect("piped stderr"));
    let running = Running(child);
    for line in stderr.lines() {
        let line = line.expect("read the server's stderr");
        if let Some(port) = line.strip_prefix("listening on 127.0.0.1:") {
            return (running, port.trim().parse().expect("a port"));
        }
    }
    panic!("{} ended before it listened", executable.display());
}

/// Read from `stream` until what was read ends with `end`.
fn read_through(stream: &mut TcpStream, end: &[u8]) {
    let mut read = Vec::new();
    let mut chunk = [0u8; 4096];
    while !read.ends_with(end) {
        let count = stream.read(&mut chunk).expect("read a reply");
        assert!(
            count > 0,
            "closed after {:?}",
            String::from_utf8_lossy(&read)
        );
        read.extend_from_slice(&chunk[..count]);
    }
}

/// One connection to the server on `port` and 20 requests on it, as the
/// native benchmark's cells `get-1` and `rr-1` send them; the connection,
/// still open.
fn one_connection(executable: &str, port: u16) -> TcpStream {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream.set_read_timeout(Some(TIMEOUT)).unwrap();
    if executable == "todod" {
        let body = r#"{"title":"bench"}"#;
        let created = r#"{"id":1,"title":"bench","done":false}"#;
        write!(
            stream,
            "POST /todos HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        read_through(&mut stream, created.as_bytes());
        for _ in 0..20 {
            stream
                .write_all(b"GET /todos/1 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
                .unwrap();
            read_through(&mut stream, created.as_bytes());
        }
    } else {
        let prompt = b"terminal> \xff\xf9";
        read_through(&mut stream, prompt);
        for _ in 0..20 {
            stream.write_all(b"echo hello\r\n").unwrap();
            read_through(&mut stream, prompt);
        }
    }
    stream
}

/// Where the process `pid` has `executable` loaded (the start of its first
/// mapping) and the size of a page of that mapping in bytes.
fn load_address(pid: u32, executable: &Path) -> (u64, u64) {
    let executable = executable.canonicalize().expect("the executable's path");
    let smaps = std::fs::read_to_string(format!("/proc/{pid}/smaps")).expect("read smaps");
    let mut lines = smaps.lines();
    let start = lines
        .by_ref()
        .find(|line| line.ends_with(&*executable.to_string_lossy()))
        .and_then(|line| line.split('-').next())
        .and_then(|start| u64::from_str_radix(start, 16).ok())
        .expect("the executable's mapping in smaps");
    let page = lines
        .find_map(|line| line.strip_prefix("KernelPageSize:"))
        .and_then(|rest| {
            rest.trim()
                .trim_end_matches("kB")
                .trim()
                .parse::<u64>()
                .ok()
        })
        .expect("KernelPageSize in smaps");
    (start, page * 1024)
}

/// Whether each of `pages` (page numbers in the address space of `pid`) is
/// resident: bit 63 of its entry in `/proc/<pid>/pagemap`.
fn resident(pid: u32, pages: &[u64]) -> Vec<bool> {
    let mut pagemap = std::fs::File::open(format!("/proc/{pid}/pagemap")).expect("open pagemap");
    pages
        .iter()
        .map(|page| {
            let mut entry = [0u8; 8];
            pagemap.seek(SeekFrom::Start(page * 8)).expect("seek");
            pagemap.read_exact(&mut entry).expect("read pagemap");
            u64::from_le_bytes(entry) >> 63 == 1
        })
        .collect()
}

/// `ignite build --release` of the fixture `backtrace.ignite` into `dir`
/// with `RUSTFLAGS` choosing the linker gold: whether it succeeded, and
/// what it wrote to stderr.
fn build_with_gold(dir: &Path) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("build")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/backtrace.ignite"))
        .args(["--target", "rust", "--release", "--out"])
        .arg(dir.join("out"))
        .arg("--target-dir")
        .arg(dir.join("cargo"))
        .env("RUSTFLAGS", "-C link-arg=-fuse-ld=gold")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .output()
        .expect("run ignite build");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A linker that does not read the release layout's script fails the link
/// with it (gold, which has no `INSERT`). The
/// release build then builds again without the script and says so, and
/// the next build into the same target directory does not try the script.
/// Skipped, with a line on stderr, where gold is not installed.
#[test]
fn a_release_build_links_without_the_layout_where_the_linker_reads_no_script() {
    if Command::new("ld.gold").arg("--version").output().is_err() {
        eprintln!("skipped: no ld.gold on this host");
        return;
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("rust-memory")
        .join("gold");
    let _ = std::fs::remove_dir_all(&dir);
    let note = "building without it";
    let (built, stderr) = build_with_gold(&dir);
    assert!(built, "{stderr}");
    assert!(stderr.contains(note), "no `{note}` in:\n{stderr}");
    assert!(dir.join("cargo/release/overflow").is_file(), "{stderr}");
    let (built, stderr) = build_with_gold(&dir);
    assert!(built, "{stderr}");
    assert!(!stderr.contains(note), "tried the script again:\n{stderr}");
}

/// Row 3.8, `rust/todo/get-1` and `rust/terminal/rr-1`: in a Rust release
/// executable the functions of the backtrace symbolizer are one run at the
/// start of the code, with no other function before or among them, and in
/// a server that has answered 20 requests on one connection at most
/// [`BESIDE`] of the pages that hold only such functions are resident.
#[test]
fn a_rust_release_server_keeps_the_backtrace_symbolizer_out_of_its_resident_code() {
    for (source, executable) in [
        ("examples/todo/todo.ignite", "todod"),
        ("examples/terminal/terminal.ignite", "terminald"),
    ] {
        let path = release_executable(source, executable);
        let functions = functions(&path);
        let symbolizer: Vec<&Function> = functions.iter().filter(|f| f.symbolizer()).collect();
        let bytes: u64 = symbolizer.iter().map(|f| f.size).sum();
        assert!(
            bytes > BESIDE,
            "{executable}: {bytes} bytes of symbolizer functions by name, of {} functions",
            functions.len()
        );
        let end = symbolizer.iter().map(|f| f.start + f.size).max().unwrap();
        let others: Vec<&Function> = functions
            .iter()
            .filter(|f| !f.symbolizer() && f.start < end)
            .collect();
        assert!(
            others.is_empty(),
            "{executable}: {} functions that are not the symbolizer's lie before or among its {} \
             ({bytes} bytes, which end at {end:#x}), {} bytes of them; the first:\n  {}",
            others.len(),
            symbolizer.len(),
            others.iter().map(|f| f.size).sum::<u64>(),
            others
                .iter()
                .take(8)
                .map(|f| format!("{:#x} {} {}", f.start, f.size, f.name))
                .collect::<Vec<_>>()
                .join("\n  ")
        );

        let (server, port) = serve(&path);
        let connection = one_connection(executable, port);
        let pid = server.0.id();
        let (base, page) = load_address(pid, &path);
        let begin = symbolizer.iter().map(|f| f.start).min().unwrap();
        // The pages wholly inside the run: no byte of another function.
        let pages: Vec<u64> = (begin.div_ceil(page)..end / page)
            .map(|index| base / page + index)
            .collect();
        let held = resident(pid, &pages).iter().filter(|held| **held).count() as u64;
        drop(connection);
        assert!(
            held * page <= BESIDE,
            "{executable}: {held} of the symbolizer's {} pages of {page} bytes are resident at \
             one connection, more than {BESIDE} bytes",
            pages.len()
        );
    }
}
