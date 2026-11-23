//! Adversary cases for `write_project` (`docs/slice.md` 14, rule 12: "no
//! build writes outside its output directory"). The default output
//! directory is `target/ignite/<package>` under the directory the build
//! runs in, so a checked-out repository can bring its contents along,
//! symbolic links included. The writer reaches files through the
//! executable's name; a link already in the output directory must not
//! carry that write, or the removal of stale executables, outside it.

use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use ignite_rustgen::{Executable, GeneratedProject, write_project};

fn project(name: &str) -> GeneratedProject {
    GeneratedProject {
        package_name: "names".to_string(),
        cargo_toml: "[package]\nname = \"names\"\n".to_string(),
        lib_rs: String::new(),
        executables: vec![Executable {
            name: name.to_string(),
            source: "fn main() {}\n".to_string(),
        }],
    }
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("rustgen-executable-names-adversary-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `out/src/bin` is a link to a directory outside `out` holding a file of
/// its own: writing the project removes that file as a stale executable
/// source and writes `tool.rs` beside it.
#[test]
fn a_linked_bin_directory_loses_no_file_outside_the_output_directory() {
    let dir = tmp("bin-link");
    let outside = dir.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("notes.txt"), "kept\n").unwrap();
    let out = dir.join("out");
    std::fs::create_dir_all(out.join("src")).unwrap();
    symlink(&outside, out.join("src").join("bin")).unwrap();
    let _ = write_project(&project("tool"), &out);
    let mut left: Vec<String> = std::fs::read_dir(&outside)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["notes.txt"], "files outside the output directory");
    assert_eq!(
        std::fs::read_to_string(outside.join("notes.txt"))
            .ok()
            .as_deref(),
        Some("kept\n")
    );
}

/// `out/src/bin/tool.rs` is a link to a file outside `out`: writing the
/// executable `tool` replaces that file's contents.
#[test]
fn a_linked_executable_source_is_not_written_through() {
    let dir = tmp("source-link");
    let outside = dir.join("outside.txt");
    std::fs::write(&outside, "kept\n").unwrap();
    let out = dir.join("out");
    std::fs::create_dir_all(out.join("src").join("bin")).unwrap();
    symlink(&outside, out.join("src").join("bin").join("tool.rs")).unwrap();
    let _ = write_project(&project("tool"), &out);
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "kept\n");
}
