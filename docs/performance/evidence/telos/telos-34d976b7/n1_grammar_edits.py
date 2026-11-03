EDITS = [
("""    assert_eq!(
        message("const A: Text = \\"{a + \\"b\\"}\\""),
        "a string literal cannot appear inside `{ }` of a string; bind it to a name first"
    );
""", ""),
("""#[test]
fn byte_literals() {""",
"""// slice 9 (`docs/slice.md` section 13)

#[test]
fn interpolation_holds_string_literals() {
    assert_eq!(
        expr("\\"a{f(\\"x\\")}b\\""),
        expr("\\"a\\" + f(\\"x\\").to_text() + \\"b\\"")
    );
    assert_eq!(
        expr("\\"{\\"in {r\\"raw}\\"}\\"}\\""),
        expr("(\\"in \\" + \\"raw}\\".to_text()).to_text()")
    );
    let deep = (0..40).fold("x".to_string(), |inner, _| format!("\\"{{{inner}}}\\""));
    assert!(errors(&format!("const A: Text = {deep}")).iter().any(|e| e.starts_with("E054")));
}

#[test]
fn raw_strings_and_unicode_escapes() {
    assert_eq!(expr("r\\"a\\\\n{b}\\""), expr("\\"a\\\\\\\\n{{b}}\\""));
    assert_eq!(expr("r#\\"say \\"hi\\"\\"#"), expr("\\"say \\\\\\"hi\\\\\\"\\""));
    assert_eq!(expr("br\\"ab\\""), expr("b\\"ab\\""));
    assert_eq!(expr("\\"\\\\u{48}\\\\u{1F600}\\""), expr("\\"H\\u{1F600}\\""));
    assert_eq!(errors("const A: Text = \\"\\\\u{D800}\\""), ["E001@1:18"]);
    assert_eq!(errors("const A: Text = \\"\\\\u{}\\""), ["E001@1:18"]);
}

#[test]
fn closures() {
    assert_eq!(expr("|x| x + 1"), "(closure [x] (+ x 1))");
    assert_eq!(expr("|a: Int, b| a"), "(closure [a: Int, b] a)");
    assert_eq!(expr("|| 1"), "(closure [] 1)");
    assert_eq!(
        expr("xs.map(|x| x * 2)"),
        "(.map xs (closure [x] (* x 2)))"
    );
    assert!(expr("|x| { let y = x\\n y }").starts_with("(closure [x] {"));
}

#[test]
fn record_update() {
    assert_eq!(
        expr("Todo { done = true, ..t }"),
        "(update Todo (done true) ..t)"
    );
    assert_eq!(expr("Todo {\\n ..t\\n}"), "(update Todo ..t)");
    assert_eq!(errors("fn f() { T { ..t, a = 1 } }"), ["E001@1:19"]);
}

#[test]
fn tuple_fields() {
    assert_eq!(expr("t.0"), "(. t 0)");
    assert_eq!(expr("t.1.0"), "(. (. t 1) 0)");
    assert_eq!(expr("f().0.len()"), "(.len (. (f) 0))");
}

#[test]
fn break_and_continue_are_expressions() {
    assert_eq!(
        expr("match x { None => break, Some(v) => continue }"),
        "(match x (None => (break)) (Some($v) => (continue)))"
    );
}

#[test]
fn range_patterns() {
    assert_eq!(
        expr("match b { b'0'..=b'9' => 1, -5..=-1 => 2, LO..=HI => 3, _ => 4 }"),
        "(match b (48..=57 => 1) (-5..=-1 => 2) (LO..=HI => 3) (_ => 4))"
    );
    assert_eq!(errors("fn f() { match b { 1..2 => 1 } }"), ["E001@1:21"]);
}

#[test]
fn glob_import() {
    let program = program("import std.text.*\\nimport a.{b}");
    assert!(program.imports[0].is_glob());
    assert_eq!(program.imports[0].module(), "std.text");
    assert!(program.imports[0].names.is_empty());
    assert!(!program.imports[1].is_glob());
}

#[test]
fn associated_fn() {
    match decls("fn Response.json(status: Int) -> Int = status").remove(0) {
        Decl::Fn(f) => {
            assert_eq!(f.owner.map(|o| o.name), Some("Response".to_string()));
            assert_eq!(f.name.name, "json");
        }
        other => panic!("expected a fn, got {other:?}"),
    }
}

#[test]
fn byte_literals() {"""),
]
