//! Standard-library dependencies of module records need no explicit import.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn fixture(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("reachability-{name}"));
    std::fs::create_dir_all(&root).unwrap();
    for (name, text) in files {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    root
}

const A: &str = r#"package std.a

type Ops { value: fn() -> Int }

fn value() -> Int {
  let ops: Module<Ops> = module("std.z")
  let read = ops.value
  return read()
}
"#;

const Z: &str = "package std.z\nfn value() -> Int = 7\n";

#[test]
fn module_literals_load_transitive_dependencies_before_their_callers() {
    let root = fixture(
        "literal",
        &[
            ("std/a.ignite", A),
            ("std/z.ignite", Z),
            ("std/unreached.ignite", "this file must not be parsed"),
            (
                "main.ignite",
                "type Ops { value: fn() -> Int }\nfn ops() -> Module<Ops> = module(\"std.a\")\n",
            ),
        ],
    );
    let library =
        ignite::load_std_reachable(&root.join("std"), &BTreeSet::from(["std.a".to_string()]))
            .unwrap();
    assert_eq!(
        library
            .modules
            .iter()
            .map(|m| m.path.as_str())
            .collect::<Vec<_>>(),
        ["std.z", "std.a"]
    );
    ignite::type_std(&library).unwrap();
    let emitted =
        ignite::emit_with_std(&root.join("main.ignite"), None, Some(&root.join("std"))).unwrap();
    assert!(emitted.std.lib_rs.contains("pub mod a"));
    assert!(emitted.std.lib_rs.contains("pub mod z"));
    assert!(emitted.program.lib_rs.contains("::ignite_std::a::value"));
}

#[test]
fn sibling_module_literals_are_standard_library_roots() {
    let root = fixture(
        "sibling",
        &[
            ("std/a.ignite", A),
            ("std/z.ignite", Z),
            (
                "main.ignite",
                "package demo\nimport demo.helper.ops\nfn answer() -> Int { let actions = ops(); let read = actions.value; return read() }\n",
            ),
            (
                "helper.ignite",
                "package demo.helper\ntype Ops { value: fn() -> Int }\nfn ops() -> Module<Ops> = module(\"std.a\")\n",
            ),
        ],
    );
    let emitted =
        ignite::emit_with_std(&root.join("main.ignite"), None, Some(&root.join("std"))).unwrap();
    assert!(emitted.std.lib_rs.contains("pub mod z"));
    assert!(emitted.program.lib_rs.contains("::ignite_std::a::value"));
}

#[test]
fn a_module_record_can_name_its_own_module_without_an_import_cycle() {
    let root = fixture(
        "self",
        &[(
            "self.ignite",
            "package std.self\ntype Ops { value: fn() -> Int }\nfn value() -> Int = 7\nfn ops() -> Module<Ops> = module(\"std.self\")\n",
        )],
    );
    let library =
        ignite::load_std_reachable(&root, &BTreeSet::from(["std.self".to_string()])).unwrap();
    ignite::type_std(&library).unwrap();
}

#[test]
fn module_literal_with_a_path_separator_stays_an_unknown_module() {
    let root = fixture(
        "invalid-path",
        &[
            ("std/bad/path.ignite", "this file must not be parsed"),
            (
                "main.ignite",
                "type Ops { value: fn() -> Int }\nfn ops() -> Module<Ops> = module(\"std.bad/path\")\n",
            ),
        ],
    );
    let error = ignite::emit_with_std(&root.join("main.ignite"), None, Some(&root.join("std")))
        .unwrap_err()
        .to_string();
    assert!(error.contains("E102"), "{error}");
    assert!(error.contains("module `std.bad/path` not found"), "{error}");
}

#[test]
fn a_program_without_an_assembly_does_not_reach_deployment_resolution() {
    let root = fixture(
        "no-assembly",
        &[
            ("std/deploy.ignite", "this unused file must not be parsed"),
            ("main.ignite", "fn value() -> Int = 7\n"),
        ],
    );
    ignite::check_with_std(&root.join("main.ignite"), Some(&root.join("std"))).unwrap();
}
