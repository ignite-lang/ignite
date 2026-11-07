//! Adversarial cases for story `stable-node-ids`, pass 2: the `FileId` the
//! CLI gives a file that does not exist, and node ids of two modules of one
//! compilation that are one file on disk.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ignite_syntax::ast::FileId;

/// `ignite check --stdin-for FILE` of a buffer not yet saved (an editor's
/// new file): every spelling of the one path names one file, so it has one
/// id, as `file_ids::stdin_for_an_existing_file_has_its_id_and_prints_its_path`
/// asserts for a file that exists.
#[test]
fn stdin_for_a_missing_file_has_one_id_for_every_spelling() {
    let relative = "tests/fixtures/not-yet-saved/a.ignite";
    let absolute = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    assert!(!absolute.exists());
    let spellings = [
        relative.to_string(),
        format!("./{relative}"),
        absolute.display().to_string(),
    ];
    let mut ids: Vec<(String, FileId)> = Vec::new();
    for spelling in spellings {
        let error = ignite::check_text(Path::new(&spelling), "fn (\n".to_string(), None)
            .expect_err("a syntax error");
        let ignite::Error::Diagnostics { source, .. } = &error else {
            panic!("{error}")
        };
        ids.push((spelling, source.id()));
    }
    assert!(ids.windows(2).all(|w| w[0].1 == w[1].1), "{ids:?}");
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Every `NodeId { .. }` in the debug form of a program.
fn node_ids(debug: &str) -> Vec<String> {
    debug
        .match_indices("id: NodeId {")
        .map(|(at, _)| {
            let rest = &debug[at..];
            rest[..rest.find('}').expect("closing brace") + 1].to_string()
        })
        .collect()
}

/// Two modules of one compilation (`app.util` and `app.copy`, typed apart,
/// their types named after different modules) whose files are one file
/// reached through a symlink: the canonical path gives both one `FileId`,
/// so they share every node id, and a table or cache keyed by `NodeId`
/// across the compilation (the story's outcome) cannot tell them apart.
#[test]
fn two_modules_of_one_compilation_share_no_node_id() {
    let dir = scratch("file_ids_adversary_symlink");
    std::fs::write(
        dir.join("main.ignite"),
        "package app\n\nimport app.util.{one}\nimport app.copy.{two}\n\nfn three() -> Int = one() + two()\n",
    )
    .expect("main");
    std::fs::write(
        dir.join("util.ignite"),
        "package app\n\nfn one() -> Int = 1\n\nfn two() -> Int = 2\n",
    )
    .expect("util");
    std::os::unix::fs::symlink("util.ignite", dir.join("copy.ignite")).expect("symlink");

    let loaded = ignite::load(&dir.join("main.ignite"), None).unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let mut owner: BTreeMap<String, &str> = BTreeMap::new();
    let mut shared: Vec<String> = Vec::new();
    let mut seen = Vec::new();
    for module in modules.iter() {
        if !module.path.starts_with("app") {
            continue;
        }
        seen.push(module.path.to_string());
        for id in node_ids(&format!("{:?}", module.program)) {
            if let Some(other) = owner.insert(id.clone(), module.path)
                && other != module.path
            {
                shared.push(format!("{id} in {other} and {}", module.path));
            }
        }
    }
    assert!(
        seen.contains(&"app.util".to_string()) && seen.contains(&"app.copy".to_string()),
        "both modules load: {seen:?}"
    );
    assert!(
        shared.is_empty(),
        "{} node ids shared, first: {}",
        shared.len(),
        shared[0]
    );
}
