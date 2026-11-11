//! The release runtime archive of the LLVM target (story `binary-size-back`,
//! row 3.9): `ignite-perf runtime` builds it as every release build of it is
//! built, on the standard library from source without its backtrace
//! symbolizer; on a toolchain without that source it is built on the
//! prebuilt standard library instead, and says so.

use std::path::Path;
use std::process::Command;

use ignite_perf::bench;
use ignite_perf::cli::default_repo;

/// The defined symbols of `archive` that belong to the standard library's
/// symbolizer (`gimli`, which reads the executable's debug information).
fn symbolizer(archive: &Path) -> usize {
    let output = Command::new("nm")
        .args(["--defined-only", "--demangle"])
        .arg(archive)
        .output()
        .expect("run nm");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.contains("gimli::"))
        .count()
}

#[test]
fn the_release_runtime_leaves_out_the_symbolizer_unless_the_std_source_is_missing() {
    let repo = default_repo();
    let target_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("release-runtime");
    let empty = target_dir.join("sysroot-without-source");
    std::fs::create_dir_all(&empty).unwrap();

    let without = bench::release_runtime(&repo, &target_dir, Some(&empty)).unwrap();
    assert!(!without.from_source, "{without:?}");
    assert!(without.archive.is_file(), "{without:?}");
    assert!(
        symbolizer(&without.archive) > 0,
        "{}: the prebuilt standard library carries the symbolizer",
        without.archive.display()
    );

    let built = bench::release_runtime(&repo, &target_dir, None).unwrap();
    if !built.from_source {
        eprintln!("this toolchain has no rust-src: only the build without it is checked");
        return;
    }
    assert!(built.archive.is_file(), "{built:?}");
    assert_ne!(built.archive, without.archive);
    assert_eq!(
        symbolizer(&built.archive),
        0,
        "{} carries the symbolizer",
        built.archive.display()
    );
}
