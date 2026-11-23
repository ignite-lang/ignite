//! Adversary cases for executable names (`docs/slice.md` 14, rule 12):
//! what `ignite check` accepts as an executable name has to be a name both
//! targets build, and an `executable` property whose value is not one
//! portable file name is refused however the value is written.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tmp(name: &str) -> PathBuf {
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("executable-names-adversary-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
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

/// A kind `Tool` whose `executable` field is declared as `field`, a `main`
/// that does nothing, then `rest`.
fn program(field: &str, rest: &str) -> String {
    format!(
        "package names\n\n\
         type Tool {{\n  {field}\n}}\n\n\
         fn Tool.main(self: Tool) -> Result<Unit, IoError> {{\n  return Ok(Unit)\n}}\n\n\
         {rest}"
    )
}

/// The names cargo forbids as a binary target (they are directories of its
/// own output directory) are portable file names, so `ignite check` takes
/// them. A name the checker takes has to build on the Rust target too:
/// either the checker refuses it or the Rust build produces it.
#[test]
fn a_name_check_accepts_builds_on_the_rust_target() {
    let dir = tmp("cargo-reserved");
    let cargo = dir.join("cargo");
    let mut broken = Vec::new();
    for name in ["build", "deps", "examples", "incremental"] {
        let source = dir.join(format!("{name}.ignite"));
        std::fs::write(
            &source,
            program(
                "executable: Text = \"unused\"",
                &format!("system S {{\n  component tool: Tool {{ executable = \"{name}\" }}\n}}\n"),
            ),
        )
        .unwrap();
        let (code, stderr) = check(&source);
        if code != Some(0) {
            assert!(stderr.contains("error[E020]"), "{name}: {stderr}");
            continue;
        }
        let options = ignite::BuildOptions {
            target: ignite::Target::Rust,
            out: Some(dir.join(format!("out-{name}"))),
            target_dir: Some(cargo.clone()),
            ..Default::default()
        };
        match ignite::build(&source, &options) {
            Ok(built) if built.executables.iter().any(|path| path.is_file()) => {}
            other => broken.push(format!("{name}: {other:?}")),
        }
    }
    assert_eq!(
        broken,
        Vec::<String>::new(),
        "`ignite check` accepts these executable names and the Rust target cannot build them"
    );
}

/// The `executable` property holds "../escape" through a constant and
/// through the kind's field default: rule 12 names the executable after the
/// component then, while the property the program reads is the path. The
/// story's outcome is that an `executable` property that is not one
/// portable file name is refused by `ignite check`.
#[test]
fn an_executable_property_that_is_not_one_file_name_is_refused_however_it_is_written() {
    let dir = tmp("not-literal");
    let cases = [
        (
            "constant",
            program(
                "executable: Text = \"unused\"",
                "const NAME: Text = \"../escape\"\n\n\
                 system S {\n  component tool: Tool { executable = NAME }\n}\n",
            ),
        ),
        (
            "field default",
            program(
                "executable: Text = \"../escape\"",
                "system S {\n  component tool: Tool {}\n}\n",
            ),
        ),
    ];
    let mut accepted = Vec::new();
    for (case, text) in cases {
        let source = dir.join(format!("{}.ignite", case.replace(' ', "-")));
        std::fs::write(&source, text).unwrap();
        let (code, stderr) = check(&source);
        if code == Some(0) {
            accepted.push(case);
        } else {
            assert!(stderr.contains("error[E020]"), "{case}: {stderr}");
        }
    }
    assert_eq!(
        accepted,
        Vec::<&str>::new(),
        "`ignite check` accepts an `executable` property of \"../escape\""
    );
}
