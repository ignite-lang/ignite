//! The lent parameter sets of `docs/slice.md` 20.3.7:
//! `TypedProgram::called_only` (a fn-typed parameter the body only calls)
//! and `TypedProgram::lent_bytes` (a `Bytes` parameter the body only reads
//! as a slice), each rule and each exclusion on its own fn.

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_types::{Callee, QualName, TypedModules, TypedProgram, type_module};

fn typed(text: &str) -> TypedProgram {
    let source = SourceFile::new("lent.ignite", text.to_string());
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    let checked = check_module(&program, &Modules::new())
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    type_module(&program, &checked, &TypedModules::new())
        .unwrap_or_else(|d| panic!("type failed:\n{}", source.render_all(&d)))
}

fn callee(name: &str) -> Callee {
    Callee::Fn(QualName::new("main", name))
}

#[test]
fn a_fn_parameter_that_is_only_called_is_called_only() {
    let t = typed(
        r#"
fn apply(f: fn(Int) -> Int, x: Int) -> Int = f(f(x))
fn keep(f: fn(Int) -> Int) -> fn(Int) -> Int = f
fn later(f: fn(Int) -> Int) -> fn(Int) -> Int = |x| f(x)
fn pass(f: fn(Int) -> Int, x: Int) -> Int = apply(f, x)
fn shadow(f: fn(Int) -> Int) -> Int {
  let f = 1
  f
}
"#,
    );
    assert!(t.called_only(&callee("apply"), 0), "apply only calls f");
    assert!(!t.called_only(&callee("apply"), 1), "x is no fn");
    assert!(!t.called_only(&callee("keep"), 0), "keep returns f");
    assert!(!t.called_only(&callee("later"), 0), "later captures f");
    assert!(!t.called_only(&callee("pass"), 0), "pass passes f on");
    assert!(
        !t.called_only(&callee("shadow"), 0),
        "a shadow's use counts"
    );
}

#[test]
fn a_bytes_parameter_read_only_as_a_slice_is_lent() {
    let t = typed(
        r#"
fn first(b: Bytes) -> Byte = b.at(0)
fn reads(b: Bytes) -> Int = b.len() + b.slice(0, 1).len()
fn text(b: Bytes) -> Text = b.to_text()
fn chain(b: Bytes) -> Byte = first(b)
fn recurse(b: Bytes, n: Int) -> Int = if n == 0 { b.len() } else { recurse(b, n - 1) }
fn give(b: Bytes) -> Bytes = b
fn grow(b: Bytes) -> Int {
  b.push(1)
  b.len()
}
fn compare(b: Bytes) -> Bool = b == b"x"
fn later(b: Bytes) -> fn(Int) -> Int = |x| x + b.len()
fn named(b: Bytes) -> Byte = first(b = b)
"#,
    );
    for lent in ["first", "reads", "text", "chain", "recurse"] {
        assert!(t.lent_bytes(&callee(lent), 0), "{lent} reads b as a slice");
    }
    assert!(!t.lent_bytes(&callee("recurse"), 1), "n is no Bytes");
    assert!(!t.lent_bytes(&callee("give"), 0), "give returns b");
    assert!(!t.lent_bytes(&callee("grow"), 0), "grow changes b in place");
    assert!(
        !t.lent_bytes(&callee("compare"), 0),
        "== is not a slice read"
    );
    assert!(!t.lent_bytes(&callee("later"), 0), "later captures b");
    assert!(
        !t.lent_bytes(&callee("named"), 0),
        "a named argument is not read"
    );
}
