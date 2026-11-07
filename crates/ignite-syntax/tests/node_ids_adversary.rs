//! Adversarial cases for story `stable-node-ids`: node ids of distinct
//! files, and the stack a nesting level costs the debug parser.

use std::collections::{HashMap, HashSet};

use ignite_syntax::SourceFile;
use ignite_syntax::ast::FileId;

/// Every `NodeId { .. }` in the debug form of a program.
fn ids(debug: &str) -> HashSet<String> {
    debug
        .match_indices("id: NodeId {")
        .map(|(at, _)| {
            let rest = &debug[at..];
            rest[..rest.find('}').expect("closing brace") + 1].to_string()
        })
        .collect()
}

/// The paths searched for a collision: a 32-bit id collides within them
/// (the first pair is at 749,192), a 64-bit one with a chance of about
/// 2e-7.
const SEARCHED: u32 = 2_000_000;

fn sibling(i: u32) -> String {
    format!("/work/project/examples/app/m{i}.ignite")
}

/// Two distinct paths of sibling modules beside one program whose
/// `FileId::of_path` is the same, among the first [`SEARCHED`]; `None`
/// when there is none.
fn colliding_paths() -> Option<(String, String)> {
    let mut seen: HashMap<FileId, u32> = HashMap::with_capacity(SEARCHED as usize);
    for i in 0..SEARCHED {
        if let Some(other) = seen.insert(FileId::of_path(&sibling(i)), i) {
            return Some((sibling(other), sibling(i)));
        }
    }
    None
}

/// Node ids are what the typed tables are keyed by, and the story's outcome
/// is that typed modules can be cached and serialised by them: two distinct
/// files of one compilation must not share a node id.
#[test]
fn distinct_paths_never_share_node_ids() {
    let Some((a, b)) = colliding_paths() else {
        return;
    };
    let first = ignite_syntax::parse(&SourceFile::new(a.clone(), "fn f() -> Int = 1 + 2\n"))
        .expect("parses");
    let second = ignite_syntax::parse(&SourceFile::new(b.clone(), "fn g() -> Text = \"x\"\n"))
        .expect("parses");
    let shared: Vec<_> = ids(&format!("{first:?}"))
        .intersection(&ids(&format!("{second:?}")))
        .cloned()
        .collect();
    assert!(
        shared.is_empty(),
        "`{a}` and `{b}` share node ids {shared:?}"
    );
}

/// Run `f` on a thread with the 2 MiB stack of a test thread.
fn on_test_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(f)
        .expect("thread")
        .join()
        .expect("parse thread");
}

fn parses(text: String) {
    on_test_stack(move || {
        ignite_syntax::parse(&SourceFile::new("t.ignite", text)).expect("parses");
    });
}

const LEVELS: usize = 100;

#[test]
fn stack_100_parens() {
    parses(format!(
        "fn f() -> Int = {}1{}\n",
        "(".repeat(LEVELS),
        ")".repeat(LEVELS)
    ));
}

#[test]
fn stack_100_lists() {
    parses(format!(
        "fn f() -> Int = {}1{}\n",
        "[".repeat(LEVELS),
        "]".repeat(LEVELS)
    ));
}

#[test]
fn stack_100_calls() {
    parses(format!(
        "fn f() -> Int = {}1{}\n",
        "g(".repeat(LEVELS),
        ")".repeat(LEVELS)
    ));
}

#[test]
fn stack_100_matches() {
    parses(format!(
        "fn f() -> Int = {}1{}\n",
        "match x { _ => ".repeat(LEVELS),
        " }".repeat(LEVELS)
    ));
}

#[test]
fn stack_100_records() {
    parses(format!(
        "fn f() -> R = {}1{}\n",
        "R { x = ".repeat(LEVELS),
        " }".repeat(LEVELS)
    ));
}

#[test]
fn stack_100_closures() {
    parses(format!("fn f() -> Int = {}1\n", "|x| ".repeat(LEVELS)));
}

#[test]
fn stack_100_negations() {
    parses(format!("fn f() -> Int = {}1\n", "-".repeat(LEVELS)));
}

/// Every `.ignite` file of the repository that parses gives each node its
/// own id: no desugaring in the parser (byte strings, `l[i] = v`,
/// interpolation, field shorthand) hands one id to two nodes.
#[test]
fn every_repository_file_has_distinct_node_ids() {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(repository_ids)
        .expect("thread")
        .join()
        .expect("walk");
}

fn repository_ids() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut pending = vec![root.join("std"), root.join("examples"), root.join("crates")];
    let mut files = 0;
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).expect("dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                if !path.ends_with("target") {
                    pending.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "ignite") {
                let text = std::fs::read_to_string(&path).expect("read");
                let Ok(program) =
                    ignite_syntax::parse(&SourceFile::new(path.display().to_string(), text))
                else {
                    continue;
                };
                let debug = format!("{program:?}");
                let all = debug.match_indices("id: NodeId {").count();
                assert_eq!(ids(&debug).len(), all, "{}", path.display());
                files += 1;
            }
        }
    }
    assert!(files > 100, "only {files} files");
}
