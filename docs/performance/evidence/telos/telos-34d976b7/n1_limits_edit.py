EDITS = [
("""//! Cost bounds of the type checker: match exhaustiveness over many literal
//! and `_` columns stays polynomial.""",
"""//! Cost bounds of the type checker: match exhaustiveness over many literal
//! and `_` columns stays polynomial; a long `+` chain neither overflows the
//! stack nor takes quadratic time (`docs/slice.md` section 13, row 14)."""),
("""#[test]
fn exhaustiveness_is_polynomial_in_bool_columns() {""",
"""/// A 100,000-term left-deep `+` chain parses, checks, types and drops on
/// a test thread's default 2 MiB stack.
#[test]
fn long_plus_chain_does_not_overflow() {
    let terms = vec!["1"; 100_000].join(" + ");
    let text = format!("package demo\\n\\nfn f() -> Int = {terms}\\n");
    let source = SourceFile::new("t.telos", text);
    let start = Instant::now();
    let program = telos_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\\n{}", source.render_all(&d)));
    let symbols = check_module(&program, &Modules::new())
        .unwrap_or_else(|d| panic!("check failed:\\n{}", source.render_all(&d)));
    let typed = type_module(&program, &symbols, &TypedModules::new())
        .unwrap_or_else(|d| panic!("type failed:\\n{}", source.render_all(&d)));
    drop(typed);
    drop(symbols);
    drop(program);
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "100,000 terms took {:?}",
        start.elapsed()
    );
}

#[test]
fn exhaustiveness_is_polynomial_in_bool_columns() {"""),
]
