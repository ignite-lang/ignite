//! Slice 6 surface syntax (`docs/slice.md` section 10) built and run on
//! both targets: the golden program `crates/ignite-llvm/tests/golden/syntax.ignite`
//! prints one line per check; each target must print the same lines and
//! exit 0.

use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use ignite::Target;

use crate::common;

const TIMEOUT: Duration = Duration::from_secs(20);

/// Each feature with the line the program prints for it, in output order.
const EXPECTED: &[(&str, &str)] = &[
    (
        "tuple value, tuple parameter and result, let (a, b), interpolation",
        "seven 7",
    ),
    ("or-pattern of Int, constant pattern", "small base other"),
    ("list pattern []", "none"),
    ("list pattern [w], + on Text", "one x"),
    (
        "constant pattern inside a list pattern",
        "quit with argument",
    ),
    ("list pattern [a, b]", "two ab"),
    ("list pattern catch-all", "many"),
    ("nested tuple, Bool and list patterns in Some", "pair 1 2"),
    ("or-pattern of payload patterns", "off or none"),
    ("or-pattern alternative None", "off or none"),
    ("fall-through after a refuted nested list pattern", "other"),
    ("or-pattern of Text, Bool to_text", "true false"),
    ("return statement inside for", "5"),
    ("return expression in a match arm", "zero+-"),
    ("for over Bytes, constant Byte pattern", "3"),
    ("UFCS on Int, expression body", "42"),
    ("UFCS on Text", "hey!"),
    ("fn-field call", "h!"),
    ("field default from a constant", "code 42"),
    ("field default variant", "continue"),
    ("to_text on Bool, Byte and Text", "true 32 t"),
    ("tuple carried across loop regions", "3000 3000"),
    ("var (a, b) with mutation", "2ab"),
    ("variant resolved by expected type", "light off"),
    ("lowercase constant pattern", "lowest other 4"),
];

fn tmp(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

/// The listener the built clients connect to. Its port is written into the
/// copy of the golden program that [`source`] builds, because `Runner`
/// holds a plain `port` field: the golden lowers without a standard library,
/// so it cannot take an `Endpoint` from `std.net` and the `--port` argument.
/// Nothing accepts: the kernel completes each connection from the backlog,
/// and the client only writes to its standard output.
fn listener() -> &'static TcpListener {
    static LISTENER: OnceLock<TcpListener> = OnceLock::new();
    LISTENER.get_or_init(|| TcpListener::bind("127.0.0.1:0").unwrap())
}

fn source() -> PathBuf {
    // Both targets build concurrently. Publish the immutable copy once so
    // neither parser can read it between the other writer's truncate/write.
    static SOURCE: OnceLock<PathBuf> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            let golden = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../ignite-llvm/tests/golden/syntax.ignite");
            let text = std::fs::read_to_string(&golden).unwrap();
            let placeholder = "port = 1 }";
            assert!(
                text.contains(placeholder),
                "syntax.ignite has no `{placeholder}`"
            );
            let port = listener().local_addr().unwrap().port();
            let dir = tmp("syntax-source");
            std::fs::create_dir_all(&dir).unwrap();
            let copy = dir.join("syntax.ignite");
            std::fs::write(
                &copy,
                text.replace(placeholder, &format!("port = {port} }}")),
            )
            .unwrap();
            copy
        })
        .clone()
}

fn build(target: Target) -> PathBuf {
    let options = match target {
        Target::Rust => ignite::BuildOptions {
            out: Some(tmp("syntax-rust")),
            target_dir: Some(tmp("syntax-rust-target")),
            ..Default::default()
        },
        Target::Llvm => ignite::BuildOptions {
            target: Target::Llvm,
            out: Some(tmp("syntax-llvm")),
            runtime: Some(common::runtime_archive()),
            ..Default::default()
        },
    };
    let output = ignite::build(&source(), &options).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(output.executables.len(), 1);
    output.executables[0].clone()
}

fn executable(target: Target) -> &'static Path {
    static RUST: OnceLock<PathBuf> = OnceLock::new();
    static LLVM: OnceLock<PathBuf> = OnceLock::new();
    match target {
        Target::Rust => RUST.get_or_init(|| build(Target::Rust)),
        Target::Llvm => LLVM.get_or_init(|| build(Target::Llvm)),
    }
}

/// Runs the client against [`listener`]; exit code, standard output and
/// standard error.
fn run(executable: &Path) -> (Option<i32>, String, String) {
    let mut child = Command::new(executable)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{} did not exit", executable.display());
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

fn check(target: Target) {
    let (code, stdout, stderr) = run(executable(target));
    let lines: Vec<&str> = stdout.lines().collect();
    let mut failures = Vec::new();
    for (index, (feature, expected)) in EXPECTED.iter().enumerate() {
        let actual = lines.get(index).copied();
        if actual != Some(*expected) {
            failures.push(format!(
                "line {}: {feature}: expected {expected:?}, got {actual:?}",
                index + 1
            ));
        }
    }
    if lines.len() != EXPECTED.len() {
        failures.push(format!(
            "{} lines, expected {}",
            lines.len(),
            EXPECTED.len()
        ));
    }
    assert!(
        failures.is_empty(),
        "{target:?}:\n{}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        failures.join("\n")
    );
    assert_eq!(code, Some(0), "{target:?} stderr: {stderr}");
}

#[test]
fn rust_target_runs_every_feature() {
    check(Target::Rust);
}

#[test]
fn llvm_target_runs_every_feature() {
    check(Target::Llvm);
}
