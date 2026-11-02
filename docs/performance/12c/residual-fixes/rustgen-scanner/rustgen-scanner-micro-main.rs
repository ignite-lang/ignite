use std::collections::{BTreeSet, HashSet};
use std::hint::black_box;
use std::time::Instant;

// Same branches as production. DIRECT changes only alpha/alphanumeric byte
// classification; raw identifiers, numeric tokens, dots and whitespace stay.
fn scan<'a, const DIRECT: bool>(text: &'a str, mut visit: impl FnMut(&'a str)) {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut after_dot = false;
    while i < bytes.len() {
        let b = bytes[i];
        let letter = if DIRECT { matches!(b, b'a'..=b'z' | b'A'..=b'Z') } else { b.is_ascii_alphabetic() };
        if letter || b == b'_' {
            let start = i;
            while i < bytes.len() && {
                let b = bytes[i];
                (if DIRECT { matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9') } else { b.is_ascii_alphanumeric() }) || b == b'_'
            } { i += 1; }
            if i - start == 1 && b == b'r' && bytes.get(i) == Some(&b'#') {
                i += 1;
                continue;
            }
            if !after_dot { visit(&text[start..i]); }
            after_dot = false;
        } else if b.is_ascii_digit() {
            while i < bytes.len() && {
                let b = bytes[i];
                (if DIRECT { matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9') } else { b.is_ascii_alphanumeric() }) || b == b'_'
            } { i += 1; }
            after_dot = false;
        } else if b == b'.' {
            if bytes.get(i + 1) == Some(&b'.') { i += 2; after_dot = false; }
            else { i += 1; after_dot = true; }
        } else if b.is_ascii_whitespace() { i += 1; }
        else { i += 1; after_dot = false; }
    }
}

fn names<const DIRECT: bool, const TREE: bool>(text: &str) -> BTreeSet<&str> {
    if TREE {
        let mut set = BTreeSet::new();
        scan::<DIRECT>(text, |name| { set.insert(name); });
        set
    } else {
        let mut set = HashSet::new();
        scan::<DIRECT>(text, |name| { set.insert(name); });
        set.into_iter().collect()
    }
}

fn run(variant: usize, text: &str) -> BTreeSet<&str> {
    match variant {
        0 => names::<false, false>(text),
        1 => names::<false, true>(text),
        2 => names::<true, false>(text),
        3 => names::<true, true>(text),
        _ => unreachable!(),
    }
}

fn main() {
    let source = std::env::args().nth(1).expect("existing 10k generated root path");
    let root = std::fs::read_to_string(source).unwrap();
    let text = root.repeat(10);
    let edges = [
        "r#type r#match foo.r#method . field ..Range 1..pump 0xff_suffix 1e9_tail",
        "// comment shout\n\"literal Handler\" /*Mode*/ name .\n method; name..pump",
        "r# r## a...b a.0.foo 12abc_def _0 _a A9 9foo . r#field, r#type",
        "éshout λHandler 中文Mode café 🦀Line .éfoo ..λbar",
        "a.\tfoo a.\nfoo a.\rfoo a.\u{000c}foo a.\u{000b}foo a.#foo",
    ];
    for input in std::iter::once(root.as_str()).chain(edges) {
        let expected = run(0, input);
        for variant in 1..4 { assert_eq!(expected, run(variant, input), "variant {variant}, edge case {input:?}"); }
        let mut first = Vec::new();
        scan::<false>(input, |name| first.push(name));
        let mut direct = Vec::new();
        scan::<true>(input, |name| direct.push(name));
        assert_eq!(first, direct, "every identifier occurrence must match");
    }
    // Exhaustive byte classification, including invalid UTF-8 bytes as numeric
    // values: matches! predicates must agree with the original ASCII methods.
    for b in 0u8..=255 {
        assert_eq!(b.is_ascii_alphabetic(), matches!(b, b'a'..=b'z' | b'A'..=b'Z'));
        assert_eq!(b.is_ascii_alphanumeric(), matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9'));
    }
    let expected = run(0, &text);
    println!("bytes={} unique={} edge cases and all occurrence sequences equal", text.len(), expected.len());
    let labels = ["ascii-hash", "ascii-tree", "match-hash", "match-tree"];
    // Alternate order, retaining every sample. Checks/conversion are included.
    for pass in 0..5 {
        for offset in 0..4 {
            let variant = if pass % 2 == 0 { offset } else { 3-offset };
            let start = Instant::now();
            let result = run(variant, black_box(&text));
            let elapsed = start.elapsed().as_secs_f64();
            assert_eq!(expected, result);
            black_box(&result);
            println!("pass={} variant={} seconds={elapsed:.9}", pass+1, labels[variant]);
        }
    }
}
