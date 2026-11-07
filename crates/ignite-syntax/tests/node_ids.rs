//! Node ids and file ids (story `stable-node-ids`): every expression,
//! pattern, type, path and place of a parse has its own id in the file's
//! id, every span carries the file, and a clone keeps the ids.

use std::collections::HashSet;

use ignite_syntax::SourceFile;
use ignite_syntax::ast::{FileId, NodeId};

const SOURCE: &str = r#"package main

import std.text.{split}

type Pair<A> { left: A, right: List<A> = [] }

fn f(p: Pair<Int>, g: fn(Int) -> Int requires [stdin.read]) -> Text {
    var q = p
    q.left = g(1) + 2 * 3
    let l = [1, 2]
    l[0] = 4
    let b = b"ab"
    match q.left {
        0..=9 => "digit {q.left + 1} and {"inner {l[0]}"}"
        n | n => "{n}"
    }
}
"#;

/// Every `NodeId` in the debug form of `value`, in order.
fn ids(debug: &str) -> Vec<String> {
    debug
        .match_indices("id: NodeId {")
        .map(|(at, _)| {
            let rest = &debug[at..];
            rest[..rest.find('}').expect("closing brace") + 1].to_string()
        })
        .collect()
}

#[test]
fn every_node_of_a_parse_has_its_own_id_in_the_file() {
    let source = SourceFile::new("main.ignite", SOURCE);
    let program = ignite_syntax::parse(&source).expect("parses");
    let debug = format!("{program:?}");
    let all = ids(&debug);
    assert!(all.len() > 60, "only {} ids", all.len());
    let distinct: HashSet<&String> = all.iter().collect();
    assert_eq!(distinct.len(), all.len(), "an id is given twice");
    let file = format!("file: FileId({})", source.id().0);
    assert!(all.iter().all(|id| id.contains(&file)), "{all:?}");
    assert_ne!(source.id(), FileId::NONE);
    assert_ne!(source.id(), FileId::SYNTHETIC);
    assert!(!debug.contains("FileId(0)"), "a span without the file");
}

#[test]
fn a_clone_keeps_the_ids_and_is_equal() {
    let source = SourceFile::new("main.ignite", SOURCE);
    let program = ignite_syntax::parse(&source).expect("parses");
    let clone = program.clone();
    assert_eq!(clone, program);
    assert_eq!(ids(&format!("{clone:?}")), ids(&format!("{program:?}")));
}

#[test]
fn the_same_path_gives_the_same_ids_and_another_path_others() {
    let first = ignite_syntax::parse(&SourceFile::new("main.ignite", SOURCE)).expect("parses");
    let again = ignite_syntax::parse(&SourceFile::new("main.ignite", SOURCE)).expect("parses");
    let other = ignite_syntax::parse(&SourceFile::new("other.ignite", SOURCE)).expect("parses");
    assert_eq!(ids(&format!("{first:?}")), ids(&format!("{again:?}")));
    assert_eq!(
        FileId::of_path("main.ignite"),
        FileId::of_path("main.ignite")
    );
    assert_ne!(
        FileId::of_path("main.ignite"),
        FileId::of_path("other.ignite")
    );
    // Equality is structural: the same text in another file differs only
    // in the spans' file.
    assert_ne!(first, other);
}

#[test]
fn fresh_ids_are_distinct_and_synthetic() {
    let a = NodeId::fresh();
    let b = NodeId::fresh();
    assert_ne!(a, b);
    assert_eq!(a.file, FileId::SYNTHETIC);
    assert_ne!(a, NodeId::DETACHED);
}
