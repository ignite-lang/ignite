//! Helpers shared by the driver's integration tests.

use std::path::PathBuf;

/// The runtime archive `libtelos_rt.a`. `telos-rt` is a dev-dependency, so
/// cargo builds its `staticlib` next to this test binary as
/// `deps/libtelos_rt-<hash>.a`; the newest one is the current build. The
/// uplifted `target/<profile>/libtelos_rt.a` exists only after a
/// `cargo build -p telos-rt` and can be stale, so it is the fallback.
pub fn runtime_archive() -> PathBuf {
    let exe = std::env::current_exe().expect("test executable path");
    let deps = exe.parent().expect("test executable directory");
    let newest = std::fs::read_dir(deps)
        .expect("read the deps directory")
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("libtelos_rt-") && name.ends_with(".a")
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .max()
        .map(|(_, path)| path);
    newest
        .or_else(|| {
            let uplifted = deps.parent()?.join("libtelos_rt.a");
            uplifted.is_file().then_some(uplifted)
        })
        .unwrap_or_else(|| panic!("no libtelos_rt-*.a in {}", deps.display()))
}
