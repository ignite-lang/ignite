//! Or-patterns against constant patterns and bindings (`docs/slice.md`,
//! the slice table's row 5): an or-pattern binds no name (E049, at the
//! first name each alternative binds), and a bare name that resolves to a
//! constant is a constant pattern whatever its case, not a binding.
//! Adversary pass 1 on `story:duplicated-rules-back`, which moved the
//! checker's walk to `Pattern::bindings`.

use ignite_check::Modules;
use ignite_syntax::SourceFile;

/// The rendered diagnostics of checking `text` alone; empty when clean.
fn check(text: &str) -> String {
    let source = SourceFile::new("t.ignite", text.to_string());
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    match ignite_check::check_module(&program, &Modules::new()) {
        Ok(_) => String::new(),
        Err(diagnostics) => source.render_all(&diagnostics),
    }
}

/// E049 points at the first name of each alternative in source order:
/// `a` (not `b`) in `(a, b)`, and `c` in `(c, 0)`.
#[test]
fn e049_names_the_first_binding_of_each_alternative() {
    let text = "fn f(p: (Int, Int)) -> Int {\n  return match p {\n    (a, b) | (c, 0) => 1\n    _ => 2\n  }\n}\n";
    let rendered = check(text);
    let lines: Vec<&str> = rendered.lines().filter(|l| l.contains("E049")).collect();
    assert_eq!(
        lines,
        [
            "t.ignite:3:6: error[E049]: `a` is bound inside an or-pattern; an or-pattern cannot bind names, match the alternatives in separate arms",
            "t.ignite:3:15: error[E049]: `c` is bound inside an or-pattern; an or-pattern cannot bind names, match the alternatives in separate arms",
        ],
        "{rendered}"
    );
}

/// An uppercase constant in an or-pattern is a constant pattern: no E049.
#[test]
fn an_uppercase_constant_in_an_or_pattern_is_no_binding() {
    let text = "const LIMIT: Int = 10\n\nfn f(n: Int) -> Int {\n  return match n {\n    LIMIT | 0 => 1\n    _ => 2\n  }\n}\n";
    assert_eq!(check(text), "");
}

/// A lowercase constant is a constant pattern as well ("whatever its
/// case: `lowest` is a constant pattern when a constant `lowest` is in
/// scope"), so the same or-pattern with `limit` binds nothing either. The
/// checker reports E049 for it: `Pattern::bindings`, which E049 reads,
/// lists every lowercase name, constants included.
#[test]
fn a_lowercase_constant_in_an_or_pattern_is_no_binding() {
    let text = "const limit: Int = 10\n\nfn f(n: Int) -> Int {\n  return match n {\n    limit | 0 => 1\n    _ => 2\n  }\n}\n";
    assert_eq!(check(text), "");
}
