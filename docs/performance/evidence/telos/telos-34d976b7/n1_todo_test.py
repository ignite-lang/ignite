EDITS = [
("""#[test]
fn terminal_program_types_against_the_standard_library() {""",
"""#[test]
fn todo_program_types_against_the_standard_library() {
    let std = standard_library();
    let path = repository().join("examples/todo/todo.telos");
    let source = SourceFile::new("todo.telos", std::fs::read_to_string(&path).unwrap());
    let program = telos_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\\n{}", source.render_all(&d)));
    let symbols = check_module(&program, &std.modules)
        .unwrap_or_else(|d| panic!("check failed:\\n{}", source.render_all(&d)));
    let typed = type_module(&program, &symbols, &std.typed)
        .unwrap_or_else(|d| panic!("type failed:\\n{}", source.render_all(&d)));
    assert_eq!(typed.module, "examples.todo");
    assert_complete(&typed, &source, &program);
}

#[test]
fn terminal_program_types_against_the_standard_library() {"""),
]
