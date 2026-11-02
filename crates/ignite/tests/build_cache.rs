//! Reusing a Cargo target directory across generated projects must rebuild
//! each project's own executable, even when its pruned std is identical.
use std::path::Path;
use std::process::Command;

fn build(source: &Path, out: &Path, target: &Path) -> std::path::PathBuf {
    ignite::build(
        source,
        &ignite::BuildOptions {
            out: Some(out.to_path_buf()),
            target_dir: Some(target.to_path_buf()),
            ..Default::default()
        },
    )
    .unwrap_or_else(|error| panic!("{error}"))
    .executables
    .remove(0)
}

#[test]
fn alternating_projects_with_the_same_std_keep_their_own_executable() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("build-cache");
    std::fs::create_dir_all(&root).unwrap();
    let first = root.join("first.ignite");
    let second = root.join("second.ignite");
    let source = r#"
import std.text.to_lower
type Printer { message: Text }
fn Printer.main(self: Printer) -> Result<Unit, IoError> {
  Stdout.write_all(to_lower(self.message).to_bytes())
}
system Example { component printer: Printer { message = "FIRST" } }
"#;
    std::fs::write(&first, source).unwrap();
    std::fs::write(&second, source.replace("FIRST", "SECOND")).unwrap();
    let target = root.join("cargo");
    // Begin with an unknown owner, retaining lockfiles and compiled dependencies.
    let _ = std::fs::remove_file(target.join("ignite-test-project"));
    for (file, out, expected) in [
        (&first, root.join("first"), "first"),
        (&second, root.join("second"), "second"),
        (&first, root.join("first"), "first"),
    ] {
        let executable = build(file, &out, &target);
        let output = Command::new(executable).output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}
