//! Helpers shared by the driver's integration tests.

pub mod apply;

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use ignite_eval::Json;

/// Setup includes compilation and build-lock waits. The first stdout segment
/// follows build/copy completion; the on-disk journal header can precede it.
/// Return the original line for the caller's normal record consumption, after
/// which its live protocol deadlines apply unchanged.
pub fn wait_run_initial_segment(
    lines: &Receiver<String>,
    stderr: impl FnOnce() -> String,
) -> String {
    const SETUP_TIMEOUT: Duration = Duration::from_secs(60);
    let line = lines.recv_timeout(SETUP_TIMEOUT).unwrap_or_else(|error| {
        panic!("ignite run setup did not produce a segment within {SETUP_TIMEOUT:?}: {error}\nstderr:\n{}", stderr())
    });
    let record = Json::parse(&line).unwrap_or_else(|error| panic!("{line}: {error}"));
    assert_eq!(
        record.get("kind").and_then(Json::as_str),
        Some("segment"),
        "ignite run setup expected the first stdout record to be a segment: {line}"
    );
    line
}

#[test]
fn run_setup_preserves_the_raw_segment_and_later_records() {
    let (send, receive) = std::sync::mpsc::channel();
    let segment = " { \"kind\" : \"segment\", \"generation\" : 1 } ";
    send.send(segment.to_string()).unwrap();
    send.send("later record".to_string()).unwrap();
    assert_eq!(
        wait_run_initial_segment(&receive, || panic!("unexpected setup failure")),
        segment
    );
    assert_eq!(receive.try_recv().unwrap(), "later record");
}

/// The runtime archive `libignite_rt.a`. `ignite-rt` is a dev-dependency, so
/// cargo builds its `staticlib` next to this test binary as
/// `deps/libignite_rt-<hash>.a`; the newest one is the current build. The
/// uplifted `target/<profile>/libignite_rt.a` exists only after a
/// `cargo build -p ignite-rt` and can be stale, so it is the fallback.
pub fn runtime_archive() -> PathBuf {
    let exe = std::env::current_exe().expect("test executable path");
    let deps = exe.parent().expect("test executable directory");
    let newest = std::fs::read_dir(deps)
        .expect("read the deps directory")
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("libignite_rt-") && name.ends_with(".a")
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .max()
        .map(|(_, path)| path);
    newest
        .or_else(|| {
            let uplifted = deps.parent()?.join("libignite_rt.a");
            uplifted.is_file().then_some(uplifted)
        })
        .unwrap_or_else(|| panic!("no libignite_rt-*.a in {}", deps.display()))
}

/// The newest `libignite_rt-*.a` beside this test binary that carries the
/// test runner (`ignite_rt_test_main`), which `ignite test --target llvm`
/// links; a `cargo build -p ignite-rt` into the same target directory can
/// leave a newer archive without it.
pub fn runner_archive() -> PathBuf {
    let exe = std::env::current_exe().expect("test executable path");
    let deps = exe.parent().expect("test executable directory");
    let mut archives: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(deps)
        .expect("read the deps directory")
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("libignite_rt-") && name.ends_with(".a")
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    archives.sort();
    let symbol = b"ignite_rt_test_main";
    archives
        .into_iter()
        .rev()
        .map(|(_, path)| path)
        .find(|path| {
            std::fs::read(path)
                .is_ok_and(|bytes| bytes.windows(symbol.len()).any(|window| window == symbol))
        })
        .unwrap_or_else(runtime_archive)
}
