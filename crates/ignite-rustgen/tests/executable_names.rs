//! `write_project` and `write_release_project` refuse an executable name
//! that is not one file name before they touch the disk, whatever name
//! reaches them (the maintainer review of 2026-11-23, IG-01): the type
//! checker refuses such a name at the property, and the writer does not
//! rely on that.

use std::path::{Path, PathBuf};

use ignite_rustgen::{Executable, GeneratedProject, write_project, write_release_project};

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

#[test]
fn a_name_that_is_not_one_file_name_is_refused_and_nothing_is_written() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("rustgen-executable-names");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let absolute = dir.join("absolute");
    let names = [
        "../escape",
        "../../escape",
        "../../../escape",
        "a/b",
        absolute.to_str().unwrap(),
        "",
        ".",
        "..",
        "a\\b",
    ];
    for name in names {
        let out = dir.join("out");
        let refused = write_project(&project(name), &out).unwrap_err();
        assert_eq!(refused.kind(), std::io::ErrorKind::InvalidInput, "{name:?}");
        assert!(
            refused.to_string().contains("not one file name"),
            "{refused}"
        );
        let refused = write_release_project(&project(name), &out).unwrap_err();
        assert_eq!(refused.kind(), std::io::ErrorKind::InvalidInput, "{name:?}");
        let left: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(left, Vec::<PathBuf>::new(), "{name:?}");
    }
}

/// Control: a name with `.`, `-` and `_` is written as `src/bin/<name>.rs`.
#[test]
fn a_portable_name_is_written() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("rustgen-executable-names-control");
    let _ = std::fs::remove_dir_all(&dir);
    write_project(&project("daemon.v2-a_b"), &dir).unwrap();
    assert!(dir.join("src/bin/daemon.v2-a_b.rs").is_file());
}
