//! Helpers shared by the driver's integration tests.

use std::path::PathBuf;

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
