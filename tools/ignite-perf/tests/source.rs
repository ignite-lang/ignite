use std::path::Path;

use ignite_perf::HARNESS_SOURCE;

#[test]
fn the_harness_bundle_holds_every_source_file_of_the_crate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec!["Cargo.toml".to_string()];
    for entry in std::fs::read_dir(root.join("src")).unwrap() {
        files.push(format!(
            "src/{}",
            entry.unwrap().file_name().to_string_lossy()
        ));
    }
    files.sort();
    // A module of the crate by `#[path]`: the release runtime's recipe,
    // shared with the driver's build script.
    files.push("../../crates/ignite-rt/release_std.rs".to_string());
    let listed: Vec<&str> = HARNESS_SOURCE.iter().map(|(name, _)| *name).collect();
    assert_eq!(files, listed);
    for (name, text) in HARNESS_SOURCE {
        assert_eq!(
            std::fs::read_to_string(root.join(name)).unwrap(),
            *text,
            "{name}"
        );
    }
}

#[test]
fn no_python_remains_in_the_repository_tooling() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let taskfile = std::fs::read_to_string(root.join("Taskfile.yml")).unwrap();
    assert!(!taskfile.contains("python3"));
    assert!(taskfile.contains("cargo fmt --all -- --check"));
    let scripts = root.join("scripts");
    if scripts.exists() {
        for entry in std::fs::read_dir(scripts).unwrap() {
            let name = entry.unwrap().file_name();
            assert!(!name.to_string_lossy().ends_with(".py"), "{name:?}");
        }
    }
}
