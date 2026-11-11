//! `let b = t.to_bytes()` is read as a borrowed slice of `t` only when no
//! later statement of its block binds `b` again (`docs/slice.md` 20.3.7,
//! `Generator::lent_let`), a `let` pattern included (`Pattern::binds`).
//! Adversary pass 1 on `story:duplicated-rules-back`: dropping the `let`
//! pattern from that rule kept the whole crate suite green.

use crate::common;

const PROGRAM: &str = "fn rebound(t: Text) -> Int {
  let b = t.to_bytes()
  let n = b.len()
  let (b, m) = (Bytes.empty(), 1)
  n + b.len() + m
}

fn kept(t: Text) -> Int {
  let b = t.to_bytes()
  let n = b.len()
  n + b.len()
}
";

/// The generated fn that starts with `head`, up to its closing brace.
fn section<'a>(lib: &'a str, head: &str) -> &'a str {
    let start = lib
        .find(head)
        .unwrap_or_else(|| panic!("no `{head}` in:\n{lib}"));
    let rest = &lib[start..];
    &rest[..rest.find("\n}\n").expect("end of the fn")]
}

#[test]
fn a_let_pattern_that_binds_the_bytes_again_keeps_them_owned() {
    let project = common::generate_source(PROGRAM, &common::repository_std())
        .unwrap_or_else(|d| panic!("{d:?}"));
    let rebound = section(&project.lib_rs, "pub fn rebound(");
    assert!(
        rebound.contains("let b = t.clone().into_bytes();"),
        "{rebound}"
    );
    let kept = section(&project.lib_rs, "pub fn kept(");
    assert!(kept.contains("let b: &[u8] = t.as_bytes();"), "{kept}");
}
