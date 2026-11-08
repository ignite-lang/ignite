//! Story `rustgen-fewer-allocations`: the Rust target's text of the todo
//! example and its reachable `ignite-std` (the HTTP server's per-request
//! path) allocates no `String` for a literal it only reads, copies no
//! `Text` to read its bytes, and passes a closure literal to a fn that only
//! calls it without an `Arc`.

use std::path::Path;

use crate::common;

/// The text of `pub fn name(` (or `pub fn name<`) in `lib`, up to the next
/// `pub fn`.
fn function<'a>(lib: &'a str, name: &str) -> &'a str {
    let start = lib
        .find(&format!("pub fn {name}("))
        .or_else(|| lib.find(&format!("pub fn {name}<")))
        .unwrap_or_else(|| panic!("no pub fn {name} in:\n{lib}"));
    let rest = &lib[start + 1..];
    let end = rest.find("pub fn ").map_or(lib.len(), |i| start + 1 + i);
    &lib[start..end]
}

fn todo() -> (String, String) {
    let file = common::workspace().join("examples/todo/todo.ignite");
    let (program, std) =
        common::lower_typed(&file, &common::repository_std(), Path::new("../ignite-io"));
    (program.lib_rs, std.lib_rs)
}

#[test]
fn a_literal_read_by_reference_is_not_allocated_per_use() {
    let (program, std) = todo();
    for (name, text) in [("program", &program), ("ignite-std", &std)] {
        let found: Vec<String> = text
            .match_indices("&String::from(\"")
            .map(|(i, _)| text[i..].chars().take(60).collect())
            .collect();
        assert!(found.is_empty(), "{name} borrows fresh literals: {found:?}");
    }
}

#[test]
fn a_comparison_with_a_literal_neither_copies_nor_allocates() {
    let (program, std) = todo();
    for (name, text) in [("program", &program), ("ignite-std", &std)] {
        for pattern in [
            "== String::from(\"",
            "!= String::from(\"",
            ".clone() == ",
            ".clone() != ",
        ] {
            let found: Vec<String> = text
                .match_indices(pattern)
                .map(|(i, _)| {
                    let from = text[..i]
                        .char_indices()
                        .rev()
                        .nth(60)
                        .map_or(0, |(at, _)| at);
                    text[from..i + pattern.len() + 20].to_string()
                })
                .collect();
            assert!(found.is_empty(), "{name} has `{pattern}`: {found:?}");
        }
    }
}

#[test]
fn reading_the_bytes_of_a_text_borrows_them() {
    let (_, std) = todo();
    let split = function(&std, "split");
    assert!(!split.contains(".clone().into_bytes()"), "{split}");
    let find_from = function(&std, "find_from");
    assert!(!find_from.contains(".clone().into_bytes()"), "{find_from}");
    let quote = function(&std, "quote");
    assert!(!quote.contains(".clone().into_bytes()"), "{quote}");
}

#[test]
fn text_is_appended_in_place_and_a_final_conversion_takes_the_buffer() {
    let (_, std) = todo();
    let head = function(&std, "render_head");
    assert!(!head.contains("format!(\"{}{}\", head"), "{head}");
    assert!(head.contains("write!(head, \"{}: {}\\r\\n\""), "{head}");
    assert!(!head.contains("head.clone().into_bytes()"), "{head}");
    let lower = function(&std, "to_lower");
    assert!(lower.contains("String::from_utf8(out)"), "{lower}");
}

#[test]
fn list_map_reads_the_elements_in_place() {
    let (_, std) = todo();
    let encode = function(&std, "encode");
    assert!(!encode.contains(".iter().cloned()"), "{encode}");
    assert!(!encode.contains("Arc::new"), "{encode}");
}

#[test]
fn a_for_over_a_slice_of_a_list_borrows_its_elements() {
    let (_, std) = todo();
    let head = function(&std, "parse_head");
    assert!(!head.contains("checked::slice(&lines"), "{head}");
    assert!(head.contains("checked::slice_ref(&lines"), "{head}");
}

#[test]
fn a_closure_literal_passed_to_a_fn_that_only_calls_it_has_no_arc() {
    let (_, std) = todo();
    let split_once = function(&std, "split_once");
    assert!(!split_once.contains("Arc::new"), "{split_once}");
    assert!(!split_once.contains(".clone();"), "{split_once}");
}
