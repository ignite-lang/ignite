use std::path::{Path, PathBuf};

use ignite_perf::json::Json;

/// Recorded evidence of the Python harness under `docs/performance/`.
pub fn evidence(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/performance")
        .join(relative)
}

pub fn read(relative: &str) -> String {
    let path = evidence(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

pub fn json(relative: &str) -> Json {
    Json::parse(&read(relative)).unwrap_or_else(|e| panic!("{relative}: {e}"))
}

pub fn records(relative: &str) -> Vec<Json> {
    read(relative)
        .lines()
        .map(|line| Json::parse(line).expect("record line"))
        .collect()
}

/// The release evidence of slice 12c, written by the Python harness `ff20d7cf…`.
pub const RELEASE: &str = "12c/release-815cf74";

/// The LLVM readiness evidence of candidate c6b22cf, written by the Python verifier.
pub const READINESS: &str = "12c/readiness-c6b22cf";

/// A fresh directory under Cargo's per-test scratch root.
pub fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ignite-perf-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("scratch directory");
    path
}
