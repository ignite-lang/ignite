//! A component's executable name (`docs/slice.md` 14, rule 12): one
//! portable file name, unique across every system of the program. A name
//! that is not one (empty, `.`, `..`, a path separator, an absolute path)
//! and a name two components of different systems produce are E020 from
//! `ignite check`, and `ignite build` on either target writes nothing for
//! them. The maintainer review of 2026-11-23, IG-01 and IG-08.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::common;

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("executable-names-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A kind with a `main` that does nothing, then `systems`.
fn program(systems: &str) -> String {
    format!(
        "package names\n\n\
         type Tool {{\n  executable: Text = \"unused\"\n}}\n\n\
         fn Tool.main(self: Tool) -> Result<Unit, IoError> {{\n  return Ok(Unit)\n}}\n\n\
         {systems}"
    )
}

/// `ignite check <source>`: exit code and standard error.
fn check(source: &Path) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("check")
        .arg(source)
        .output()
        .unwrap();
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn options(target: ignite::Target, out: &Path, cargo: &Path) -> ignite::BuildOptions {
    ignite::BuildOptions {
        target,
        out: Some(out.to_path_buf()),
        target_dir: (target == ignite::Target::Rust).then(|| cargo.to_path_buf()),
        runtime: (target == ignite::Target::Llvm).then(common::runtime_archive),
        ..Default::default()
    }
}

/// Every file under `dir`, recursively.
fn files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files(&path));
        } else {
            found.push(path);
        }
    }
    found
}

/// `ignite check` refuses `source` with E020 on its first line, at
/// `line:column` and with `message` in it; `ignite build` on both targets
/// fails with diagnostics and leaves `out` empty and nothing in `dir` but
/// the source, `out` and the Rust target's cargo directory. The Rust
/// target starts its dependency build ahead (`prebuild.rs`) before the
/// check, into the cargo directory and independent of the program: that
/// directory holds the build ahead's own project and cargo's files, and
/// nothing named after an executable.
fn refused(dir: &Path, source: &Path, at: &str, message: &str) {
    let (code, stderr) = check(source);
    let first = stderr.lines().next().unwrap_or("");
    assert_eq!(code, Some(1), "{}: {stderr}", source.display());
    assert!(
        first.contains(&format!("program.ignite:{at}: error[E020]")) && first.contains(message),
        "{}: {stderr}",
        source.display()
    );
    let out = dir.join("out");
    let cargo = dir.join("cargo");
    for target in [ignite::Target::Llvm, ignite::Target::Rust] {
        match ignite::build(source, &options(target, &out, &cargo)) {
            Err(ignite::Error::Diagnostics { .. }) => {}
            other => panic!("{target:?} {}: {other:?}", source.display()),
        }
        assert_eq!(files(&out), Vec::<PathBuf>::new(), "{target:?}");
    }
    let mut left: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path != &out && path != &cargo)
        .collect();
    left.sort();
    assert_eq!(left, [source.to_path_buf()], "{}", source.display());
    let ahead = [
        "ignite-prebuild",
        "debug",
        "CACHEDIR.TAG",
        ".rustc_info.json",
    ];
    for entry in std::fs::read_dir(&cargo).into_iter().flatten() {
        let name = entry.unwrap().file_name();
        assert!(
            ahead.iter().any(|kept| name == *kept),
            "{}: {name:?} in the cargo directory",
            source.display()
        );
    }
}

/// IG-01: an `executable` that is not one portable file name is E020 at
/// the property's value, and neither target writes a file for it. The
/// absolute name points into the test's own directory, so a build that
/// followed it would leave a file the last assert sees.
#[test]
fn an_executable_name_that_is_not_one_file_name_is_e020_and_nothing_is_written() {
    let dir = tmp("refused");
    let outside = dir.join("absolute");
    let absolute = outside.to_str().unwrap();
    let names = [
        "../escape",
        "../../escape",
        "../../../escape",
        "a/b",
        absolute,
        "",
        ".",
        "..",
        "a\\b",
    ];
    for (index, name) in names.iter().enumerate() {
        let case = dir.join(format!("case-{index}"));
        std::fs::create_dir_all(&case).unwrap();
        let source = case.join("program.ignite");
        let literal = name.replace('\\', "\\\\");
        std::fs::write(
            &source,
            program(&format!(
                "system S {{\n  component tool: Tool {{ executable = \"{literal}\" }}\n}}\n"
            )),
        )
        .unwrap();
        refused(&case, &source, "12:39", "is not one file name");
    }
    assert!(!outside.exists());
}

/// IG-08: two systems each with a component `tool` are E020 at the
/// second, naming the first.
#[test]
fn two_systems_with_a_component_of_one_name_are_e020() {
    let dir = tmp("two-tools");
    let source = dir.join("program.ignite");
    std::fs::write(
        &source,
        program(
            "system A {\n  component tool: Tool {}\n}\n\n\
             system B {\n  component tool: Tool {}\n}\n",
        ),
    )
    .unwrap();
    refused(
        &dir,
        &source,
        "16:13",
        "`B.tool` produces the executable `tool`, as `A.tool` does",
    );
}

/// IG-08: an `executable` of one system that is the name of a component
/// of another is E020 too.
#[test]
fn an_executable_property_and_a_component_name_of_another_system_are_e020() {
    let dir = tmp("property-and-name");
    let source = dir.join("program.ignite");
    std::fs::write(
        &source,
        program(
            "system A {\n  component tool: Tool { executable = \"x\" }\n}\n\n\
             system B {\n  component x: Tool {}\n}\n",
        ),
    )
    .unwrap();
    refused(
        &dir,
        &source,
        "16:13",
        "`B.x` produces the executable `x`, as `A.tool` does",
    );
}

/// Control: one system with a repeated name stays E020.
#[test]
fn one_system_with_a_repeated_executable_name_stays_e020() {
    let dir = tmp("one-system");
    let source = dir.join("program.ignite");
    std::fs::write(
        &source,
        program(
            "system A {\n  component tool: Tool {}\n  component other: Tool { executable = \"tool\" }\n}\n",
        ),
    )
    .unwrap();
    refused(
        &dir,
        &source,
        "13:13",
        "`A.other` produces the executable `tool`, as `A.tool` does",
    );
}

/// Control: names with `-` and `_`, a component without `executable` (its
/// own name) and distinct names across two systems check clean and build
/// every executable on both targets; a name with `.` checks clean and
/// builds on the LLVM target. The Rust target does not build a `.` in an
/// executable name today (rustc takes the cargo binary name as the crate
/// name), so it is not asked to here.
#[test]
fn portable_names_distinct_across_systems_build_on_both_targets() {
    let dir = tmp("controls");
    let cargo = dir.join("cargo");
    let common = "system A {\n  component plain: Tool {}\n}\n\n\
                  system B {\n  component other: Tool { executable = \"b-tool_2\" }\n}\n";
    let dotted = "system C {\n  component dotted: Tool { executable = \"daemon.v2\" }\n}\n";
    for (target, systems, expected) in [
        (
            ignite::Target::Llvm,
            format!("{common}\n{dotted}"),
            &["b-tool_2", "daemon.v2", "plain"][..],
        ),
        (
            ignite::Target::Rust,
            common.to_string(),
            &["b-tool_2", "plain"][..],
        ),
    ] {
        let source = dir.join(format!("{target:?}.ignite"));
        std::fs::write(&source, program(&systems)).unwrap();
        let (code, stderr) = check(&source);
        assert_eq!(code, Some(0), "{target:?}: {stderr}");
        let out = dir.join(format!("out-{target:?}"));
        let built = ignite::build(&source, &options(target, &out, &cargo))
            .unwrap_or_else(|e| panic!("{target:?}: {e}"));
        let mut names: Vec<String> = built
            .executables
            .iter()
            .map(|path| {
                assert!(path.is_file(), "{target:?}: {}", path.display());
                path.file_name().unwrap().to_string_lossy().into_owned()
            })
            .collect();
        names.sort();
        assert_eq!(names, expected, "{target:?}");
    }
}
