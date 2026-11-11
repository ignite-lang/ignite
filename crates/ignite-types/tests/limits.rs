//! Cost bounds of the type checker: match exhaustiveness over many literal
//! and `_` columns stays polynomial; a long `+` chain neither overflows the
//! stack nor takes quadratic time (`docs/slice.md` section 13, row 14);
//! deciding whether a nested record literal waits stays polynomial.

use std::time::{Duration, Instant};

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_syntax::ast::{Decl, ExprKind, StmtKind};
use ignite_types::{TypedModules, type_module};

/// The wall-clock bound of every cost check. The sizes set the
/// separation, not the bound: the linear and polynomial paths take well
/// under a second in a debug build (0.2 s, 28 ms and 0.5 ms unloaded on
/// 2026-11-05), while the regressions they guard against (a quadratic
/// walk of 100,000 terms, a line table per lookup over 400 KB, 2^40 rows
/// for 40 columns) take minutes or never end. A deadline this generous
/// still fails them and holds under any load a gate sees.
const DEADLINE: Duration = Duration::from_secs(30);

/// A match on a tuple of `n` `Bool`s with `2n` arms, one `true` and one
/// `false` per column and `_` elsewhere: exhaustive.
fn bool_columns(n: usize) -> String {
    let tuple = vec!["Bool"; n].join(", ");
    let mut arms = String::new();
    for i in 0..n {
        for literal in ["true", "false"] {
            let mut columns = vec!["_"; n];
            columns[i] = literal;
            arms.push_str(&format!("    ({}) => {i}\n", columns.join(", ")));
        }
    }
    format!("package demo\n\nfn g(t: ({tuple})) -> Int {{\n  return match t {{\n{arms}  }}\n}}\n")
}

/// A 100,000-term left-deep `+` chain parses, checks, types and drops on
/// a test thread's default 2 MiB stack.
#[test]
fn long_plus_chain_does_not_overflow() {
    let terms = vec!["1"; 100_000].join(" + ");
    let text = format!("package demo\n\nfn f() -> Int = {terms}\n");
    let source = SourceFile::new("t.ignite", text);
    let start = Instant::now();
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    let symbols = check_module(&program, &Modules::new())
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let typed = type_module(&program, &symbols, &TypedModules::new())
        .unwrap_or_else(|d| panic!("type failed:\n{}", source.render_all(&d)));
    drop(typed);
    drop(symbols);
    drop(program);
    assert!(
        start.elapsed() < DEADLINE,
        "100,000 terms took {:?}",
        start.elapsed()
    );
}

#[test]
fn exhaustiveness_is_polynomial_in_bool_columns() {
    let source = SourceFile::new("t.ignite", bool_columns(40));
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    let symbols = check_module(&program, &Modules::new())
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let start = Instant::now();
    let typed = type_module(&program, &symbols, &TypedModules::new());
    let elapsed = start.elapsed();
    if let Err(d) = typed {
        panic!("type failed:\n{}", source.render_all(&d));
    }
    assert!(
        elapsed < DEADLINE,
        "40 Bool columns with 80 arms took {elapsed:?}"
    );
}

/// Every `+` of the 100,000-term chain has a failure site, and resolving
/// all of them to file positions costs one scan of the source
/// (`SourceFile` keeps its line table), not one scan per site.
#[test]
fn failure_site_positions_of_the_long_chain_are_cheap() {
    let terms = vec!["1"; 100_000].join(" + ");
    let text = format!("package demo\n\nfn f() -> Int = {terms}\n");
    let source = SourceFile::new("t.ignite", text);
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    let symbols = check_module(&program, &Modules::new())
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let typed = type_module(&program, &symbols, &TypedModules::new())
        .unwrap_or_else(|d| panic!("type failed:\n{}", source.render_all(&d)));
    // Walk the left spine of the chain iteratively: a recursive walker
    // would overflow the test thread's stack before the lookups start.
    let Some(Decl::Fn(f)) = program.decls.first() else {
        panic!("a fn");
    };
    let Some(StmtKind::Expr(root)) = f.body.stmts.first().map(|s| &s.kind) else {
        panic!("an expression body");
    };
    let mut node: &ignite_syntax::ast::Expr = root;
    let start = Instant::now();
    let mut sites = 0;
    loop {
        if let Some(site) = typed.failure_site(node) {
            let (path, line, _) = site.position(&source);
            assert_eq!((path.as_str(), line), ("t.ignite", 3));
            sites += 1;
        }
        match &node.kind {
            ExprKind::Binary { left, .. } => node = &**left,
            _ => break,
        }
    }
    let elapsed = start.elapsed();
    assert_eq!(sites, 99_999);
    assert!(
        elapsed < DEADLINE,
        "{sites} failure-site positions took {elapsed:?}"
    );
}

/// A generic record literal nested `depth` deep as the argument of a
/// generic fn: `keep(Pair { a = Pair { a = .. Pair { a = 1, b = 2 } .., b = 2 }, b = 2 })`.
fn nested_pairs(depth: usize) -> String {
    let mut literal = "Pair { a = 1, b = 2 }".to_string();
    for _ in 1..depth {
        literal = format!("Pair {{ a = {literal}, b = 2 }}");
    }
    format!(
        "package demo\n\ntype Pair<A, B> {{\n  a: A\n  b: B\n}}\n\nfn keep<X>(x: X) -> Int = 0\n\nfn main() -> Int = keep({literal})\n"
    )
}

/// Whether a record literal argument waits for the arguments after it
/// (`docs/slice.md` 37.1) is decided by one walk of the literal, linear in
/// its size: typing a chain of `depth` nested generic literals asks it once
/// for the literal of every call and every literal around a literal, so the
/// literals looked at stay within `depth * (depth + 1)`. Counted, not
/// timed (`ignite_types::literal_wait_steps`); a walk that asks the nested
/// literals again for each field that binds looks at about 2^(depth + 3).
#[test]
fn deciding_whether_a_nested_record_literal_waits_is_polynomial_in_depth() {
    let mut counts = Vec::new();
    for depth in [10, 14, 18, 24] {
        let source = SourceFile::new("t.ignite", nested_pairs(depth));
        let program = ignite_syntax::parse(&source)
            .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
        let symbols = check_module(&program, &Modules::new())
            .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
        let before = ignite_types::literal_wait_steps();
        if let Err(d) = type_module(&program, &symbols, &TypedModules::new()) {
            panic!("type failed:\n{}", source.render_all(&d));
        }
        let steps = ignite_types::literal_wait_steps() - before;
        eprintln!("depth {depth}: {steps} record literals looked at");
        counts.push((depth, steps));
    }
    for (depth, steps) in counts {
        let bound = (depth * (depth + 1)) as u64;
        assert!(
            steps <= bound,
            "depth {depth}: {steps} record literals looked at, over {bound}"
        );
    }
}
