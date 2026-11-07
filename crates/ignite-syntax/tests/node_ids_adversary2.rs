//! Adversarial cases for story `stable-node-ids`, pass 2: the 64-bit
//! `FileId`. `node_ids_adversary::distinct_paths_never_share_node_ids`
//! searches 2,000,000 sibling paths and returns without asserting when it
//! finds no pair, which a 64-bit hash never gives it; a parallel rho search
//! over the same path shape finds a pair in 30 s on one workstation
//! (12 threads), so a collision is a fixture, not a search.

use ignite_syntax::SourceFile;
use ignite_syntax::ast::FileId;

/// Two sibling module paths whose 64-bit FNV-1a is `0x8c90799f98ec11cb`.
const A: &str = "/work/project/examples/app/m28f332a082b5e328.ignite";
const B: &str = "/work/project/examples/app/m3b4caa5b359d6895.ignite";

#[test]
fn two_sibling_paths_have_distinct_file_ids() {
    assert_ne!(A, B);
    assert_ne!(FileId::of_path(A), FileId::of_path(B), "`{A}` and `{B}`");
}

/// The consequence the story cares about: two files of one compilation,
/// each parsed with the id its canonical path gives, share every node id
/// of the same index.
#[test]
fn two_sibling_files_share_no_node_id() {
    let first = ignite_syntax::parse(&SourceFile::new(A, "fn f() -> Int = 1 + 2\n")).expect("a");
    let second = ignite_syntax::parse(&SourceFile::new(B, "fn g() -> Text = \"x\"\n")).expect("b");
    let ids = |debug: String| -> Vec<String> {
        debug
            .match_indices("id: NodeId {")
            .map(|(at, _)| {
                let rest = &debug[at..];
                rest[..rest.find('}').expect("brace") + 1].to_string()
            })
            .collect()
    };
    let first = ids(format!("{first:?}"));
    let shared: Vec<_> = ids(format!("{second:?}"))
        .into_iter()
        .filter(|id| first.contains(id))
        .collect();
    assert!(shared.is_empty(), "shared node ids {shared:?}");
}
