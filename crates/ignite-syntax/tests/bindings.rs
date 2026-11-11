//! `Pattern::bindings` and `Pattern::binds`: the names a pattern binds, the
//! one walk the moves analysis, the checker and both backends read
//! (story `duplicated-rules-back`). Each case is a shape one of the former
//! copies handled: `name`, `..name`, tuples, lists, variant payloads, `|`
//! alternatives, literals and ranges.

use ignite_syntax::ast::{Decl, ExprKind, Pattern, StmtKind};
use ignite_syntax::{SourceFile, parse};

/// The pattern of the single arm of `match x { <pattern> => 0 }`.
fn pattern(text: &str) -> Pattern {
    let source = SourceFile::new(
        "t.ignite",
        format!("fn f() {{\nmatch x {{ {text} => 0 }}\n}}"),
    );
    let program = parse(&source).unwrap_or_else(|d| panic!("{}", source.render_all(&d)));
    let Some(Decl::Fn(f)) = program.decls.into_iter().next() else {
        panic!("expected a fn")
    };
    let StmtKind::Expr(e) = f.body.stmts.into_iter().next().unwrap().kind else {
        panic!("expected an expression statement")
    };
    let ExprKind::Match { arms, .. } = &e.kind else {
        panic!("expected a match")
    };
    arms[0].pattern.clone()
}

fn names(text: &str) -> Vec<String> {
    pattern(text)
        .bindings()
        .iter()
        .map(|id| id.name.clone())
        .collect()
}

#[test]
fn a_name_and_a_named_rest_bind() {
    assert_eq!(names("x"), ["x"]);
    assert_eq!(names("[first, ..rest]"), ["first", "rest"]);
    assert_eq!(names("[.., last]"), ["last"]);
}

#[test]
fn nested_names_bind_in_source_order() {
    assert_eq!(
        names("(a, [b, ..tail], Some(Pair(c, d)), Shape.Circle(r))"),
        ["a", "b", "tail", "c", "d", "r"]
    );
}

/// Every alternative of `|` contributes its names, so a name two
/// alternatives bind is listed twice (the checker refuses an or-pattern
/// that binds, E049, by the first name of an alternative).
#[test]
fn every_alternative_contributes() {
    assert_eq!(names("Ok(a) | Err(a)"), ["a", "a"]);
    assert_eq!(names("(1, x) | (y, 2)"), ["x", "y"]);
}

#[test]
fn literals_wildcards_and_constants_bind_nothing() {
    for text in [
        "_",
        "1",
        "\"s\"",
        "true",
        "[..]",
        "None",
        "Color.Red",
        "(_, 0)",
    ] {
        assert_eq!(names(text), Vec::<String>::new(), "{text}");
    }
}

/// A range binds nothing, a lowercase bound included: a bound is a literal
/// or a constant, and the type checker refuses any other (E045,
/// `docs/slice.md` 13.7), so no backend sees a range bound that is a name.
#[test]
fn a_range_binds_nothing() {
    assert_eq!(names("0..=9"), Vec::<String>::new());
    assert_eq!(names("LOW..=HIGH"), Vec::<String>::new());
    assert_eq!(names("low..=high"), Vec::<String>::new());
    assert!(!pattern("low..=high").binds("low"));
}

#[test]
fn binds_is_membership_in_bindings() {
    let p = pattern("(a, [b, ..tail], Some(c) | None)");
    for name in ["a", "b", "tail", "c"] {
        assert!(p.binds(name), "{name}");
    }
    for name in ["x", "Some", "None", "_"] {
        assert!(!p.binds(name), "{name}");
    }
}
