//! Section 6 row by row, on hand-built ASTs (no parser). Generated text is
//! compared after collapsing whitespace runs to one space.

use std::path::PathBuf;

use ignite_check::{BUILTIN_ENUMS, FnInfo, Symbols};
use ignite_rustgen::{GeneratedProject, Options, generate};
use ignite_syntax::ast::*;
use ignite_syntax::diagnostic::Diagnostic;

fn sp() -> Span {
    Span::default()
}

fn id(name: &str) -> Ident {
    Ident {
        name: name.to_string(),
        span: sp(),
    }
}

fn ex(kind: ExprKind) -> Expr {
    Expr { kind, span: sp() }
}

fn name(n: &str) -> Expr {
    ex(ExprKind::Name(id(n)))
}

fn int(value: i64) -> Expr {
    ex(ExprKind::Int(value))
}

fn text(value: &str) -> Expr {
    ex(ExprKind::Str(value.to_string()))
}

fn boolean(value: bool) -> Expr {
    ex(ExprKind::Bool(value))
}

fn call(callee: Expr, args: Vec<Expr>) -> Expr {
    ex(ExprKind::Call {
        callee: Box::new(callee),
        args,
    })
}

fn callf(function: &str, args: Vec<Expr>) -> Expr {
    call(name(function), args)
}

fn method(receiver: Expr, n: &str, args: Vec<Expr>) -> Expr {
    ex(ExprKind::Method {
        receiver: Box::new(receiver),
        name: id(n),
        args,
    })
}

fn field(receiver: Expr, n: &str) -> Expr {
    ex(ExprKind::Field {
        receiver: Box::new(receiver),
        name: id(n),
    })
}

fn list(items: Vec<Expr>) -> Expr {
    ex(ExprKind::List(items))
}

fn record(n: &str, fields: Vec<(&str, Expr)>) -> Expr {
    ex(ExprKind::Record {
        name: id(n),
        fields: fields
            .into_iter()
            .map(|(f, value)| FieldInit {
                name: id(f),
                value,
                span: sp(),
            })
            .collect(),
    })
}

fn try_(inner: Expr) -> Expr {
    ex(ExprKind::Try(Box::new(inner)))
}

fn unary(op: UnaryOp, operand: Expr) -> Expr {
    ex(ExprKind::Unary {
        op,
        operand: Box::new(operand),
    })
}

fn binary(op: BinaryOp, left: Expr, right: Expr) -> Expr {
    ex(ExprKind::Binary {
        op,
        left: Box::new(left),
        right: Box::new(right),
    })
}

fn block(stmts: Vec<Stmt>) -> Block {
    Block { stmts, span: sp() }
}

fn block_expr(stmts: Vec<Stmt>) -> Expr {
    ex(ExprKind::Block(block(stmts)))
}

fn if_(cond: Expr, then: Vec<Stmt>, otherwise: Option<Vec<Stmt>>) -> Expr {
    ex(ExprKind::If {
        cond: Box::new(cond),
        then: block(then),
        otherwise: otherwise.map(|stmts| Box::new(block_expr(stmts))),
    })
}

fn match_(scrutinee: Expr, arms: Vec<(Pattern, Expr)>) -> Expr {
    ex(ExprKind::Match {
        scrutinee: Box::new(scrutinee),
        arms: arms
            .into_iter()
            .map(|(pattern, body)| Arm {
                pattern,
                body,
                span: sp(),
            })
            .collect(),
    })
}

fn pat(kind: PatternKind) -> Pattern {
    Pattern { kind, span: sp() }
}

fn pvariant(path: &[&str], args: Vec<Pattern>) -> Pattern {
    pat(PatternKind::Variant {
        path: path.iter().map(|s| id(s)).collect(),
        args,
    })
}

fn pbind(n: &str) -> Pattern {
    pat(PatternKind::Binding(id(n)))
}

fn st(kind: StmtKind) -> Stmt {
    Stmt { kind, span: sp() }
}

fn expr_stmt(e: Expr) -> Stmt {
    st(StmtKind::Expr(e))
}

fn let_(n: &str, value: Expr) -> Stmt {
    st(StmtKind::Let {
        name: id(n),
        ty: None,
        value,
        mutable: false,
    })
}

fn var_(n: &str, value: Expr) -> Stmt {
    st(StmtKind::Let {
        name: id(n),
        ty: None,
        value,
        mutable: true,
    })
}

fn ret(value: Expr) -> Stmt {
    st(StmtKind::Return(Some(value)))
}

fn assign(base: &str, fields: &[&str], value: Expr) -> Stmt {
    st(StmtKind::Assign {
        target: Place {
            base: id(base),
            fields: fields.iter().map(|f| id(f)).collect(),
            span: sp(),
        },
        value,
    })
}

fn ty(n: &str) -> Type {
    Type {
        name: id(n),
        args: Vec::new(),
        span: sp(),
        fn_names: Vec::new(),
        requires: None,
    }
}

fn ty_of(n: &str, args: Vec<Type>) -> Type {
    Type {
        name: id(n),
        args,
        span: sp(),
        fn_names: Vec::new(),
        requires: None,
    }
}

fn func(
    n: &str,
    params: &[(&str, Type)],
    ret: Option<Type>,
    effectful: bool,
    stmts: Vec<Stmt>,
) -> Decl {
    Decl::Fn(FnDecl {
        test_only: false,
        owner: None,
        name: id(n),
        params: params
            .iter()
            .map(|(p, t)| Param {
                name: id(p),
                ty: t.clone(),
                span: sp(),
            })
            .collect(),
        ret,
        requires: if effectful { Some(Vec::new()) } else { None },
        body: block(stmts),
        span: sp(),
        type_params: Vec::new(),
        bounds: Vec::new(),
    })
}

fn user_decls() -> Vec<Decl> {
    vec![
        Decl::Enum(EnumDecl {
            test_only: false,
            name: id("Command"),
            variants: vec![
                Variant {
                    name: id("Help"),
                    payload: vec![],
                    span: sp(),
                },
                Variant {
                    name: id("Echo"),
                    payload: vec![ty("Text")],
                    span: sp(),
                },
            ],
            span: sp(),
            type_params: Vec::new(),
        }),
        Decl::Enum(EnumDecl {
            test_only: false,
            name: id("Action"),
            variants: vec![
                Variant {
                    name: id("Continue"),
                    payload: vec![],
                    span: sp(),
                },
                Variant {
                    name: id("Close"),
                    payload: vec![],
                    span: sp(),
                },
            ],
            span: sp(),
            type_params: Vec::new(),
        }),
        Decl::Record(RecordDecl {
            test_only: false,
            name: id("Reply"),
            fields: vec![
                Field {
                    name: id("lines"),
                    ty: ty_of("List", vec![ty("Text")]),
                    default: None,
                    span: sp(),
                },
                Field {
                    name: id("action"),
                    ty: ty("Action"),
                    default: None,
                    span: sp(),
                },
            ],
            span: sp(),
            type_params: Vec::new(),
        }),
    ]
}

fn add_enum(symbols: &mut Symbols, e: &str, vs: Vec<String>) {
    for v in &vs {
        symbols
            .variants
            .entry(v.clone())
            .or_default()
            .push(e.to_string());
    }
    symbols.enums.insert(e.to_string(), vs);
}

/// What the checker returns for `program`, built by hand.
fn symbols_for(program: &Program) -> Symbols {
    let mut symbols = Symbols::default();
    for (e, vs) in BUILTIN_ENUMS {
        add_enum(&mut symbols, e, vs.iter().map(|v| v.to_string()).collect());
    }
    for decl in &program.decls {
        match decl {
            Decl::Assembly(_) => {}
            Decl::Alias(a) => {
                symbols.aliases.insert(a.name.name.clone(), a.params.len());
            }
            Decl::Const(c) => {
                symbols.constants.insert(c.name.name.clone());
            }
            Decl::Enum(e) => add_enum(
                &mut symbols,
                &e.name.name,
                e.variants.iter().map(|v| v.name.name.clone()).collect(),
            ),
            Decl::Record(r) => {
                symbols.records.insert(r.name.name.clone());
                symbols.record_fields.insert(
                    r.name.name.clone(),
                    r.fields.iter().map(|f| f.name.name.clone()).collect(),
                );
                symbols.record_defaults.insert(
                    r.name.name.clone(),
                    r.fields
                        .iter()
                        .filter(|f| f.default.is_some())
                        .map(|f| f.name.name.clone())
                        .collect(),
                );
            }
            Decl::Fn(f) => {
                symbols.functions.insert(
                    f.name.name.clone(),
                    FnInfo {
                        effectful: f.is_effectful(),
                        arity: f.params.len(),
                    },
                );
            }
            Decl::System(_) | Decl::Test(_) => {}
            Decl::Protocol(p) => {
                symbols.protocols.insert(
                    p.name.name.clone(),
                    p.fns
                        .iter()
                        .map(|f| {
                            (
                                f.name.name.clone(),
                                FnInfo {
                                    effectful: f.is_effectful(),
                                    arity: f.params.len(),
                                },
                            )
                        })
                        .collect(),
                );
            }
        }
    }
    symbols
}

fn program(decls: Vec<Decl>) -> Program {
    Program {
        package: Some(Path {
            segments: vec![id("examples"), id("demo")],
            span: sp(),
        }),
        imports: vec![],
        decls,
    }
}

fn options() -> Options {
    Options {
        substrate_path: PathBuf::from("/sub/ignite-io"),
        ..Options::default()
    }
}

fn try_generate(decls: Vec<Decl>) -> Result<GeneratedProject, Vec<Diagnostic>> {
    let mut all = user_decls();
    all.extend(decls);
    let program = program(all);
    let symbols = symbols_for(&program);
    generate(&program, &symbols, &options())
}

fn norm(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn lib(decls: Vec<Decl>) -> String {
    norm(&try_generate(decls).expect("generates").lib_rs)
}

/// The library for `fn f(x: Text, n: Int, b: Bytes, l: List<Text>, r: Reply,
/// flag: Bool, c: Command)` with `stmts` as its body and no return type.
fn body(stmts: Vec<Stmt>) -> String {
    lib(vec![func(
        "f",
        &[
            ("x", ty("Text")),
            ("n", ty("Int")),
            ("b", ty("Bytes")),
            ("l", ty_of("List", vec![ty("Text")])),
            ("r", ty("Reply")),
            ("flag", ty("Bool")),
            ("c", ty("Command")),
        ],
        None,
        false,
        stmts,
    )])
}

#[track_caller]
fn assert_has(haystack: &str, needle: &str) {
    let needle = norm(needle);
    assert!(
        haystack.contains(&needle),
        "expected\n  {needle}\nin\n  {haystack}"
    );
}

#[track_caller]
fn assert_lacks(haystack: &str, needle: &str) {
    let needle = norm(needle);
    assert!(
        !haystack.contains(&needle),
        "did not expect\n  {needle}\nin\n  {haystack}"
    );
}

#[test]
fn header_and_declarations() {
    let out = lib(vec![]);
    assert!(out.starts_with(&norm(ignite_rustgen::FILE_HEADER)));
    assert!(ignite_rustgen::FILE_HEADER.contains("unreachable_code"));
    assert_has(
        &out,
        "#[derive(Clone)] pub enum Command { Help, Echo(String), }",
    );
    assert_has(
        &out,
        "#[derive(Clone)] pub struct Reply { pub lines: Vec<String>, pub action: Action, }",
    );
    assert!(!out.contains("use ignite_io::*"));
}

#[test]
fn type_table() {
    let out = lib(vec![func(
        "g",
        &[
            ("a", ty("Text")),
            ("b", ty("Bytes")),
            ("c", ty("Byte")),
            ("d", ty("Int")),
            ("e", ty("Bool")),
            ("f", ty("Unit")),
            ("g", ty_of("List", vec![ty("Int")])),
            ("h", ty_of("Option", vec![ty("Text")])),
            ("i", ty_of("Result", vec![ty("Unit"), ty("IoError")])),
            ("j", ty("TcpListener")),
            ("k", ty("TcpStream")),
            ("m", ty("Input")),
            ("o", ty("Reply")),
            ("p", ty_of("Mystery", vec![ty("Int")])),
        ],
        Some(ty("Unit")),
        false,
        vec![],
    )]);
    assert_has(
        &out,
        "pub fn g(a: &String, b: &Vec<u8>, c: &u8, d: &i64, e: &bool, f: &(), \
         g: &Vec<i64>, h: &Option<String>, i: &Result<(), ignite_io::IoError>, \
         j: &ignite_io::TcpListener, k: &ignite_io::TcpStream, m: &ignite_io::Input, \
         o: &Reply, p: &Mystery<i64>) -> () { let mut c = c.clone(); let mut d = d.clone(); \
         let mut e = e.clone(); let mut f = f.clone(); }",
    );
}

#[test]
fn effectful_function_is_a_plain_fn() {
    let out = lib(vec![func(
        "h",
        &[("s", ty("TcpStream"))],
        Some(ty_of("Result", vec![ty("Unit"), ty("IoError")])),
        true,
        vec![ret(callf("Ok", vec![name("Unit")]))],
    )]);
    assert_has(
        &out,
        "pub fn h(s: &ignite_io::TcpStream) -> Result<(), ignite_io::IoError> { return Ok(()); }",
    );
}

#[test]
fn builtin_methods() {
    let cases: Vec<(Expr, &str)> = vec![
        (method(name("x"), "len", vec![]), "(x.len() as i64)"),
        (method(name("x"), "is_empty", vec![]), "x.is_empty()"),
        (method(name("x"), "trim", vec![]), "x.trim().to_string()"),
        (
            method(name("x"), "starts_with", vec![text("p")]),
            "x.starts_with(&String::from(\"p\"))",
        ),
        (
            method(name("x"), "strip_prefix", vec![text("p")]),
            "x.strip_prefix(&String::from(\"p\")).map(|s| s.to_string())",
        ),
        (method(name("x"), "first_word", vec![]), "x.first_word()"),
        (
            method(name("x"), "rest_after_word", vec![]),
            "x.rest_after_word()",
        ),
        (
            method(name("x"), "concat", vec![text("u")]),
            "format!(\"{}{}\", x, String::from(\"u\"))",
        ),
        (
            method(name("x"), "to_bytes", vec![]),
            "x.clone().into_bytes()",
        ),
        (
            method(name("b"), "to_text", vec![]),
            "String::from_utf8_lossy(&b).into_owned()",
        ),
        (method(name("b"), "at", vec![int(1)]), "b[(1) as usize]"),
        (
            method(name("b"), "slice", vec![int(1), int(2)]),
            "b[(1) as usize..(2) as usize].to_vec()",
        ),
        (method(name("b"), "push", vec![int(7)]), "b.push(7)"),
        (
            method(name("b"), "append", vec![callf("g", vec![])]),
            "b.extend_from_slice(&g())",
        ),
        (method(name("b"), "clear", vec![]), "b.clear()"),
        (
            method(name("l"), "join", vec![text(",")]),
            "l.join(&String::from(\",\"))",
        ),
        (method(name("x"), "copy", vec![]), "x.clone()"),
        (method(name("Bytes"), "empty", vec![]), "Vec::<u8>::new()"),
        (
            call(field(name("Bytes"), "empty"), vec![]),
            "Vec::<u8>::new()",
        ),
        (
            method(name("Bytes"), "of", vec![list(vec![int(1), int(2)])]),
            "(vec![1, 2]).iter().map(|x| *x as u8).collect::<Vec<u8>>()",
        ),
        (
            method(name("Bytes"), "of", vec![list(vec![])]),
            "Vec::<u8>::new()",
        ),
        (method(name("List"), "empty", vec![]), "Vec::new()"),
        (
            method(name("Int"), "to_text", vec![int(5)]),
            "5.to_string()",
        ),
        (
            method(name("Int"), "to_text", vec![name("n")]),
            "n.clone().to_string()",
        ),
        (
            callf("log", vec![name("x")]),
            "ignite_io::log(&(x.clone()))",
        ),
        (
            call(
                field(name("TcpListener"), "bind"),
                vec![text("127.0.0.1"), int(0)],
            ),
            "ignite_io::TcpListener::bind(String::from(\"127.0.0.1\"), 0)",
        ),
        (
            method(
                name("TcpStream"),
                "connect",
                vec![text("127.0.0.1"), name("n")],
            ),
            "ignite_io::TcpStream::connect(String::from(\"127.0.0.1\"), n.clone())",
        ),
        (
            method(name("Stdin"), "read", vec![int(4096)]),
            "ignite_io::Stdin::read(4096)",
        ),
        (
            call(field(name("Stdout"), "write_all"), vec![name("b")]),
            "ignite_io::Stdout::write_all(b.clone())",
        ),
        (
            method(name("x"), "write_all", vec![name("b")]),
            "x.write_all(b.clone())",
        ),
        (
            method(name("Stdin"), "is_terminal", vec![]),
            "ignite_io::Stdin::is_terminal()",
        ),
        (
            method(name("Stdin"), "set_raw", vec![boolean(true)]),
            "ignite_io::Stdin::set_raw(true)",
        ),
        (
            method(name("Process"), "exit", vec![int(0)]),
            "ignite_io::exit((0) as i32)",
        ),
        (
            method(name("IoError"), "new", vec![text("bad")]),
            "ignite_io::IoError::new(String::from(\"bad\"))",
        ),
        (
            call(field(name("IoError"), "new"), vec![name("x")]),
            "ignite_io::IoError::new(x.clone())",
        ),
    ];
    for (expr, expected) in cases {
        let out = body(vec![let_("y", expr)]);
        assert_has(&out, &format!("let y = {expected};"));
    }
}

#[test]
fn receiver_that_is_not_atomic_is_parenthesized() {
    let out = body(vec![let_(
        "y",
        method(
            binary(BinaryOp::Add, name("n"), int(1)),
            "to_string_like",
            vec![],
        ),
    )]);
    assert_has(&out, "let y = (n + 1).to_string_like();");
}

#[test]
fn ownership_value_positions_clone_places() {
    let cases: Vec<(Stmt, &str)> = vec![
        (expr_stmt(callf("g", vec![name("x")])), "g(x.clone());"),
        (
            expr_stmt(method(name("b"), "push", vec![name("n")])),
            "b.push(n.clone());",
        ),
        (let_("y", name("x")), "let y = x.clone();"),
        (var_("y", name("x")), "let mut y = x.clone();"),
        (
            let_("y", field(name("r"), "action")),
            "let y = r.action.clone();",
        ),
        (assign("r", &["lines"], name("l")), "r.lines = l.clone();"),
        (
            expr_stmt(match_(
                field(name("r"), "action"),
                vec![(pat(PatternKind::Wildcard), int(1))],
            )),
            "match r.action.clone() { _ => 1, };",
        ),
        (ret(name("x")), "return x.clone();"),
        (
            let_("y", list(vec![name("x"), text("a")])),
            "let y = vec![x.clone(), String::from(\"a\")];",
        ),
        (
            let_(
                "y",
                record(
                    "Reply",
                    vec![("lines", name("l")), ("action", field(name("r"), "action"))],
                ),
            ),
            "let y = Reply { lines: l.clone(), action: r.action.clone() };",
        ),
        (
            st(StmtKind::For {
                name: id("i"),
                iter: name("l"),
                body: block(vec![]),
            }),
            "for i in (l.clone()) { }",
        ),
        (let_("y", try_(name("x"))), "let y = (x.clone())?;"),
        (
            let_("y", unary(UnaryOp::Not, name("flag"))),
            "let y = !flag.clone();",
        ),
        (
            let_("y", unary(UnaryOp::Neg, name("n"))),
            "let y = -n.clone();",
        ),
    ];
    for (stmt, expected) in cases {
        let out = body(vec![stmt]);
        assert_has(&out, expected);
    }
}

#[test]
fn ownership_non_value_positions_do_not_clone() {
    let out = body(vec![
        let_("a", method(name("x"), "trim", vec![])),
        let_("a2", method(field(name("r"), "lines"), "len", vec![])),
        assign("r", &["action"], field(name("Action"), "Close")),
        let_("s", binary(BinaryOp::Add, name("n"), name("n"))),
        let_(
            "e",
            binary(
                BinaryOp::Or,
                binary(BinaryOp::Eq, name("x"), text("a")),
                name("flag"),
            ),
        ),
    ]);
    assert_has(&out, "let a = x.trim().to_string();");
    assert_has(&out, "let a2 = (r.lines.len() as i64);");
    assert_has(&out, "r.action = Action::Close;");
    assert_has(&out, "let s = n + n;");
    assert_has(&out, "let e = ((*x) == String::from(\"a\")) || flag;");
    assert_lacks(&out, "r.lines.clone()");
}

#[test]
fn functions_and_variants_are_not_places() {
    let out = body(vec![
        let_("y", callf("g", vec![])),
        let_("h", name("Help")),
        let_("k", name("g")),
    ]);
    assert_has(&out, "let y = g();");
    assert_has(&out, "let h = Command::Help;");
    assert_has(&out, "let k = g;");
}

#[test]
fn bindings_are_scoped() {
    let out = body(vec![
        expr_stmt(block_expr(vec![let_("inner", int(1))])),
        expr_stmt(callf("g", vec![name("inner")])),
    ]);
    assert_has(&out, "g(inner);");
}

#[test]
fn block_values() {
    let out = body(vec![
        let_(
            "v",
            if_(
                name("flag"),
                vec![expr_stmt(int(1))],
                Some(vec![expr_stmt(int(2))]),
            ),
        ),
        let_(
            "w",
            if_(name("flag"), vec![expr_stmt(callf("g", vec![]))], None),
        ),
        expr_stmt(if_(
            name("flag"),
            vec![expr_stmt(callf("g", vec![]))],
            Some(vec![expr_stmt(callf("g", vec![]))]),
        )),
        expr_stmt(match_(
            name("c"),
            vec![
                (
                    pvariant(&["Help"], vec![]),
                    block_expr(vec![expr_stmt(callf("g", vec![])), expr_stmt(int(1))]),
                ),
                (pat(PatternKind::Wildcard), int(2)),
            ],
        )),
        st(StmtKind::While {
            cond: name("flag"),
            body: block(vec![expr_stmt(int(3))]),
        }),
        st(StmtKind::Loop {
            body: block(vec![expr_stmt(int(4)), st(StmtKind::Break)]),
        }),
        st(StmtKind::For {
            name: id("i"),
            iter: name("l"),
            body: block(vec![expr_stmt(int(5)), st(StmtKind::Continue)]),
        }),
        expr_stmt(int(6)),
    ]);
    assert_has(&out, "let v = if flag { 1 } else { 2 };");
    assert_has(&out, "let w = if flag { g(); };");
    assert_has(&out, "if flag { g(); } else { g(); };");
    assert_has(&out, "Command::Help => { g(); 1 }, _ => 2, };");
    assert_has(&out, "while flag { 3; }");
    assert_has(&out, "loop { 4; break; }");
    assert_has(&out, "for i in (l.clone()) { 5; continue; }");
    assert_has(&out, "6; }");
}

#[test]
fn function_body_value_depends_on_return_type() {
    let out = lib(vec![
        func("a", &[], Some(ty("Int")), false, vec![expr_stmt(int(1))]),
        func("u", &[], Some(ty("Unit")), false, vec![expr_stmt(int(1))]),
        func("n", &[], None, false, vec![expr_stmt(int(1))]),
        func(
            "t",
            &[("flag", ty("Bool"))],
            Some(ty("Int")),
            false,
            vec![expr_stmt(if_(
                name("flag"),
                vec![expr_stmt(int(1))],
                Some(vec![expr_stmt(int(2))]),
            ))],
        ),
        func(
            "r",
            &[("flag", ty("Bool"))],
            Some(ty("Int")),
            false,
            vec![ret(if_(
                name("flag"),
                vec![expr_stmt(int(1))],
                Some(vec![expr_stmt(int(2))]),
            ))],
        ),
        func(
            "lp",
            &[],
            Some(ty("Int")),
            false,
            vec![st(StmtKind::Loop {
                body: block(vec![ret(int(1))]),
            })],
        ),
    ]);
    assert_has(&out, "pub fn a() -> i64 { 1 }");
    assert_has(&out, "pub fn u() -> () { 1; }");
    assert_has(&out, "pub fn n() { 1; }");
    assert_has(
        &out,
        "pub fn t(flag: &bool) -> i64 { let mut flag = flag.clone(); if flag { 1 } else { 2 } }",
    );
    assert_has(
        &out,
        "pub fn r(flag: &bool) -> i64 { let mut flag = flag.clone(); return if flag { 1 } else { 2 }; }",
    );
    assert_has(&out, "pub fn lp() -> i64 { loop { return 1; } }");
}

#[test]
fn else_if_chain_keeps_statement_position() {
    let out = body(vec![expr_stmt(ex(ExprKind::If {
        cond: Box::new(name("flag")),
        then: block(vec![expr_stmt(int(1))]),
        otherwise: Some(Box::new(if_(
            name("flag"),
            vec![expr_stmt(int(2))],
            Some(vec![expr_stmt(int(3))]),
        ))),
    }))]);
    assert_has(&out, "if flag { 1; } else if flag { 2; } else { 3; };");
}

#[test]
fn unit_value_and_type() {
    let out = body(vec![
        let_("u", name("Unit")),
        let_("o", callf("Ok", vec![name("Unit")])),
    ]);
    assert_has(&out, "let u = ();");
    assert_has(&out, "let o = Ok(());");
}

#[test]
fn typed_let() {
    let out = body(vec![st(StmtKind::Let {
        name: id("z"),
        ty: Some(ty_of("List", vec![ty("Byte")])),
        value: method(name("List"), "empty", vec![]),
        mutable: true,
    })]);
    assert_has(&out, "let mut z: Vec<u8> = Vec::new();");
}

#[test]
fn variant_expressions() {
    let cases: Vec<(Expr, &str)> = vec![
        (name("Help"), "Command::Help"),
        (field(name("Action"), "Continue"), "Action::Continue"),
        (callf("Echo", vec![name("x")]), "Command::Echo(x.clone())"),
        (
            call(field(name("Command"), "Echo"), vec![name("x")]),
            "Command::Echo(x.clone())",
        ),
        (
            method(name("Command"), "Echo", vec![text("a")]),
            "Command::Echo(String::from(\"a\"))",
        ),
        (callf("Some", vec![name("x")]), "Some(x.clone())"),
        (name("None"), "None"),
        (field(name("Option"), "None"), "None"),
        (callf("Ok", vec![boolean(true)]), "Ok(true)"),
        (callf("Err", vec![name("x")]), "Err(x.clone())"),
        (
            callf("Data", vec![name("b")]),
            "ignite_io::Input::Data(b.clone())",
        ),
        (name("Eof"), "ignite_io::Input::Eof"),
        (field(name("Input"), "Eof"), "ignite_io::Input::Eof"),
        (
            call(field(name("Input"), "Data"), vec![name("b")]),
            "ignite_io::Input::Data(b.clone())",
        ),
    ];
    for (expr, expected) in cases {
        let out = body(vec![let_("y", expr)]);
        assert_has(&out, &format!("let y = {expected};"));
    }
}

#[test]
fn variant_patterns() {
    let out = body(vec![
        expr_stmt(match_(
            name("c"),
            vec![
                (pvariant(&["Help"], vec![]), int(1)),
                (
                    pvariant(&["Command", "Echo"], vec![pbind("t")]),
                    callf("g", vec![name("t")]),
                ),
                (pbind("other"), int(3)),
            ],
        )),
        expr_stmt(match_(
            name("x"),
            vec![
                (pvariant(&["Some"], vec![pbind("v")]), name("v")),
                (pvariant(&["None"], vec![]), int(0)),
                (pvariant(&["Ok"], vec![pat(PatternKind::Wildcard)]), int(0)),
                (pvariant(&["Err"], vec![pbind("e")]), int(0)),
                (pvariant(&["Data"], vec![pbind("bytes")]), int(0)),
                (pvariant(&["Eof"], vec![]), int(0)),
                (pvariant(&["Input", "Eof"], vec![]), int(0)),
                (pat(PatternKind::Binding(id("Help"))), int(0)),
                (pat(PatternKind::Int(7)), int(0)),
                (pat(PatternKind::Str("s".into())), int(0)),
            ],
        )),
    ]);
    assert_has(&out, "Command::Help => 1,");
    assert_has(&out, "Command::Echo(t) => g(t.clone()),");
    assert_has(&out, "other => 3,");
    assert_has(&out, "Some(v) => v,");
    assert_has(&out, "None => 0,");
    assert_has(&out, "Ok(_) => 0,");
    assert_has(&out, "Err(e) => 0,");
    assert_has(&out, "ignite_io::Input::Data(bytes) => 0,");
    assert_has(
        &out,
        "ignite_io::Input::Eof => 0, ignite_io::Input::Eof => 0,",
    );
    assert_has(&out, "Command::Help => 0,");
    assert_has(&out, "7 => 0,");
    assert_has(&out, "__m if (*(&__m) == \"s\") => 0,");
}

#[test]
fn qualified_variant_patterns() {
    let out = body(vec![
        expr_stmt(match_(
            name("c"),
            vec![
                (pvariant(&["Command", "Help"], vec![]), int(1)),
                (
                    pvariant(
                        &["Command", "Echo"],
                        vec![pvariant(&["Option", "None"], vec![])],
                    ),
                    int(2),
                ),
                (pat(PatternKind::Wildcard), int(3)),
            ],
        )),
        expr_stmt(match_(
            name("x"),
            vec![
                (pvariant(&["Option", "Some"], vec![pbind("v")]), int(0)),
                (pvariant(&["Result", "Err"], vec![pbind("e")]), int(0)),
                (pvariant(&["Input", "Data"], vec![pbind("bytes")]), int(0)),
            ],
        )),
    ]);
    assert_has(&out, "Command::Help => 1,");
    assert_has(&out, "Command::Echo(None) => 2,");
    assert_has(&out, "Some(v) => 0,");
    assert_has(&out, "Err(e) => 0,");
    assert_has(&out, "ignite_io::Input::Data(bytes) => 0,");
}

fn constant(n: &str, t: Type, value: Expr) -> Decl {
    Decl::Const(ConstDecl {
        test_only: false,
        name: id(n),
        ty: t,
        value,
        span: sp(),
    })
}

#[test]
fn constants_are_zero_argument_functions() {
    let out = lib(vec![
        constant("LIMIT", ty("Int"), int(4096)),
        constant("PROMPT", ty("Text"), text("> ")),
        constant(
            "DOUBLE",
            ty("Int"),
            binary(BinaryOp::Mul, name("LIMIT"), int(2)),
        ),
        constant(
            "GREETING",
            ty("Text"),
            method(name("PROMPT"), "concat", vec![text("hi")]),
        ),
        func(
            "f",
            &[("n", ty("Int"))],
            Some(ty("Bool")),
            false,
            vec![
                let_("a", name("LIMIT")),
                let_("b", callf("g", vec![name("PROMPT")])),
                let_("c", method(name("PROMPT"), "len", vec![])),
                ret(binary(BinaryOp::Lt, name("n"), name("LIMIT"))),
            ],
        ),
    ]);
    assert_has(
        &out,
        "#[allow(non_snake_case)] pub fn LIMIT() -> i64 { 4096 }",
    );
    assert_has(
        &out,
        "#[allow(non_snake_case)] pub fn PROMPT() -> String { String::from(\"> \") }",
    );
    assert_has(&out, "pub fn DOUBLE() -> i64 { LIMIT() * 2 }");
    assert_has(
        &out,
        "pub fn GREETING() -> String { format!(\"{}{}\", PROMPT(), String::from(\"hi\")) }",
    );
    assert_has(&out, "let a = LIMIT();");
    assert_has(&out, "let b = g(PROMPT());");
    assert_has(&out, "let c = (PROMPT().len() as i64);");
    assert_has(&out, "return n < LIMIT();");
    assert_lacks(&out, "LIMIT().clone()");
}

#[test]
fn imported_constant_and_local_shadowing() {
    let decls = {
        let mut all = user_decls();
        all.push(func(
            "f",
            &[],
            Some(ty("Int")),
            false,
            vec![
                let_("a", name("MAXIMUM")),
                let_("MAXIMUM", int(1)),
                ret(name("MAXIMUM")),
            ],
        ));
        all
    };
    let program = program(decls);
    let mut symbols = symbols_for(&program);
    symbols.constants.insert("MAXIMUM".to_string());
    symbols
        .imported
        .insert("MAXIMUM".to_string(), "std.text".to_string());
    let out = norm(&generate(&program, &symbols, &options()).unwrap().lib_rs);
    assert_has(&out, "let a = MAXIMUM();");
    assert_has(&out, "let MAXIMUM = 1; return MAXIMUM.clone();");
    assert_lacks(&out, "pub fn MAXIMUM()");
}

#[test]
fn string_escaping() {
    let out = body(vec![let_("y", text("a\\b\"c\nd\re\tf\0g\u{1}h\u{7f}i é"))]);
    assert_has(
        &out,
        "let y = String::from(\"a\\\\b\\\"c\\nd\\re\\tf\\0g\\x01h\\x7fi é\");",
    );
}

#[test]
fn int_literals_are_plain_digits() {
    let out = body(vec![let_("y", int(4096)), let_("z", int(-3))]);
    assert_has(&out, "let y = 4096;");
    assert_has(&out, "let z = (-3);");
}

#[test]
fn rust_keywords_are_escaped() {
    let out = body(vec![let_("move", int(1)), let_("y", name("move"))]);
    assert_has(&out, "let r#move = 1;");
    assert_has(&out, "let y = r#move.clone();");
}

fn result_unit() -> Type {
    ty_of("Result", vec![ty("Unit"), ty("IoError")])
}

/// `fn name(params) -> Result<Unit, IoError>` returning `Ok(Unit)`.
fn role(n: &str, params: &[(&str, Type)], effectful: bool) -> FnDecl {
    let Decl::Fn(decl) = func(
        n,
        params,
        Some(result_unit()),
        effectful,
        vec![ret(callf("Ok", vec![name("Unit")]))],
    ) else {
        unreachable!()
    };
    decl
}

/// `protocol <n>` with the named roles, each with the section 4.2 shape.
fn protocol(n: &str, roles: &[&str]) -> Decl {
    Decl::Protocol(ProtocolDecl {
        name: id(n),
        fns: roles
            .iter()
            .map(|r| match *r {
                "serve" => role(
                    "serve",
                    &[("stream", ty("TcpStream")), ("application", ty("Reply"))],
                    true,
                ),
                other => role(other, &[("stream", ty("TcpStream"))], true),
            })
            .collect(),
        span: sp(),
    })
}

fn prop(n: &str, value: Expr, start: usize) -> FieldInit {
    FieldInit {
        name: Ident {
            name: n.to_string(),
            span: Span::new(start, start + n.len()),
        },
        value: Expr {
            span: Span::new(start + 100, start + 101),
            kind: value.kind.clone(),
        },
        span: Span::new(start, start + 101),
    }
}

fn resource(n: &str, kind: &str, properties: Vec<FieldInit>) -> ResourceDecl {
    ResourceDecl {
        name: id(n),
        kind: Ident {
            name: kind.to_string(),
            span: Span::new(900, 900 + kind.len()),
        },
        properties,
        span: sp(),
        args: Vec::new(),
    }
}

fn component(n: &str, kind: &str, properties: Vec<FieldInit>) -> ComponentDecl {
    ComponentDecl {
        name: id(n),
        kind: Ident {
            name: kind.to_string(),
            span: Span::new(950, 950 + kind.len()),
        },
        properties,
        span: sp(),
    }
}

fn ingress() -> ResourceDecl {
    resource(
        "ingress",
        "TcpListener",
        vec![
            prop("address", text("127.0.0.1"), 10),
            prop("port", int(2323), 20),
        ],
    )
}

fn system(resources: Vec<ResourceDecl>, components: Vec<ComponentDecl>) -> Decl {
    Decl::System(SystemDecl {
        name: id("Sys"),
        resources,
        components,
        span: sp(),
        contracts: Vec::new(),
        policies: Vec::new(),
    })
}

fn codes(result: Result<GeneratedProject, Vec<Diagnostic>>) -> Vec<(&'static str, usize)> {
    let mut found: Vec<(&'static str, usize)> = result
        .expect_err("diagnostics")
        .iter()
        .map(|d| (d.code, d.span.start))
        .collect();
    found.sort();
    found
}

#[test]
fn no_system_is_a_library_without_executables() {
    let project = try_generate(vec![]).expect("generates");
    assert!(project.executables.is_empty());
    assert!(!project.cargo_toml.contains("[[bin]]"));
    assert!(project.cargo_toml.contains("[lib]"));
}

#[test]
fn package_name_defaults_to_program() {
    let mut p = program(user_decls());
    p.package = None;
    let symbols = symbols_for(&p);
    let project = generate(&p, &symbols, &options()).unwrap();
    assert_eq!(project.package_name, "program");
}

#[test]
fn e020_resource_kind() {
    let result = try_generate(vec![system(vec![resource("q", "Queue", vec![])], vec![])]);
    assert_eq!(codes(result), [("E020", 900)]);
}

#[test]
fn e020_bad_properties() {
    let result = try_generate(vec![system(
        vec![resource(
            "ingress",
            "TcpListener",
            vec![
                prop("address", int(1), 10),
                prop("port", text("x"), 20),
                prop("colour", int(1), 30),
            ],
        )],
        vec![],
    )]);
    assert_eq!(codes(result), [("E020", 30), ("E020", 110), ("E020", 120)]);
}

/// Components are lowered from the type checker's executables
/// (`docs/slice.md` section 14.5): without types every component is E020
/// at its kind.
#[test]
fn e020_components_without_types() {
    let result = try_generate(vec![system(
        vec![ingress()],
        vec![component(
            "w",
            "Worker",
            vec![prop("executable", text("w"), 40)],
        )],
    )]);
    assert_eq!(codes(result), [("E020", 950)]);
}

fn import(path: &[&str], names: &[&str]) -> Import {
    Import {
        path: Path {
            segments: path.iter().map(|s| id(s)).collect(),
            span: sp(),
        },
        names: names.iter().map(|n| id(n)).collect(),
        span: sp(),
    }
}

#[test]
fn imports_become_use_lines() {
    let mut program = program(user_decls());
    program.imports = vec![
        import(&["std", "net"], &["TcpListener", "TcpStream"]),
        import(&["std", "io", "IoError"], &[]),
        import(&["std", "protocol", "telnet"], &["Telnet", "Console"]),
        import(&["std", "console", "execute"], &[]),
    ];
    let symbols = symbols_for(&program);
    let project = generate(&program, &symbols, &options()).unwrap();
    let out = norm(&project.lib_rs);
    assert_has(
        &out,
        "clippy::all)] use ignite_std::protocol::telnet::{Telnet, Console}; use ignite_std::console::execute;",
    );
    assert!(!out.contains("::net"), "{out}");
    assert!(!out.contains("::io"), "{out}");
    assert!(
        project
            .cargo_toml
            .contains("ignite-std = { path = \"std\" }")
    );
}

#[test]
fn substrate_imports_need_no_std_crate() {
    let mut program = program(user_decls());
    program.imports = vec![
        import(&["std", "net"], &["TcpStream"]),
        import(&["std", "io", "IoError"], &[]),
    ];
    let symbols = symbols_for(&program);
    let project = generate(&program, &symbols, &options()).unwrap();
    assert!(!project.cargo_toml.contains("ignite-std"));
    assert!(!project.lib_rs.contains("use "));
}

#[test]
fn protocol_lowering() {
    let out = lib(vec![
        protocol("Line", &["serve", "connect"]),
        func(
            "run",
            &[("s", ty("TcpStream")), ("r", ty("Reply"))],
            Some(result_unit()),
            true,
            vec![
                expr_stmt(method(name("Line"), "serve", vec![name("s"), name("r")])),
                ret(call(field(name("Line"), "connect"), vec![name("s")])),
            ],
        ),
    ]);
    assert_has(
        &out,
        "pub struct Line; impl Line { \
         pub fn serve(mut stream: ignite_io::TcpStream, mut application: Reply) -> Result<(), ignite_io::IoError> { return Ok(()); } \
         pub fn connect(mut stream: ignite_io::TcpStream) -> Result<(), ignite_io::IoError> { return Ok(()); } }",
    );
    assert_has(&out, "{ Line::serve(s.clone(), r.clone());");
    assert_has(&out, "return Line::connect(s.clone());");
}

#[test]
fn own_protocol_is_known_without_symbols() {
    let decls = {
        let mut all = user_decls();
        all.push(protocol("Line", &["serve"]));
        all.push(func(
            "run",
            &[("s", ty("TcpStream")), ("r", ty("Reply"))],
            Some(result_unit()),
            true,
            vec![ret(method(
                name("Line"),
                "serve",
                vec![name("s"), name("r")],
            ))],
        ));
        all
    };
    let program = program(decls);
    let mut symbols = symbols_for(&program);
    symbols.protocols.clear();
    let project = generate(&program, &symbols, &options()).unwrap();
    assert_has(
        &norm(&project.lib_rs),
        "return Line::serve(s.clone(), r.clone());",
    );
}

fn fn_type(params: Vec<Type>, ret: Type) -> Type {
    let mut args = params;
    args.push(ret);
    ty_of("fn", args)
}

#[test]
fn function_types_and_values() {
    let out = lib(vec![
        Decl::Record(RecordDecl {
            test_only: false,
            name: id("Console"),
            fields: vec![Field {
                name: id("respond"),
                ty: fn_type(vec![ty("Text")], ty("Reply")),
                default: None,
                span: sp(),
            }],
            span: sp(),
            type_params: Vec::new(),
        }),
        func(
            "answer",
            &[("line", ty("Text"))],
            Some(ty("Reply")),
            false,
            vec![ret(record(
                "Reply",
                vec![("lines", list(vec![])), ("action", name("Close"))],
            ))],
        ),
        func(
            "apply",
            &[
                ("f", fn_type(vec![ty("Text")], ty("Reply"))),
                ("g", fn_type(vec![], ty("Unit"))),
                ("console", ty("Console")),
            ],
            Some(ty("Reply")),
            false,
            vec![
                let_("made", record("Console", vec![("respond", name("answer"))])),
                let_("h", name("answer")),
                let_("respond", field(name("console"), "respond")),
                expr_stmt(callf("respond", vec![text("x")])),
                expr_stmt(callf("h", vec![text("y")])),
                ret(callf("f", vec![text("z")])),
            ],
        ),
    ]);
    assert_has(
        &out,
        "pub struct Console { pub respond: std::sync::Arc<dyn Fn(String) -> Reply + Send + Sync>, }",
    );
    assert_has(
        &out,
        "pub fn apply(f: &std::sync::Arc<dyn Fn(String) -> Reply + Send + Sync>, g: &std::sync::Arc<dyn Fn() -> () + Send + Sync>, console: &Console) -> Reply { let mut f = f.clone(); let mut g = g.clone();",
    );
    assert_has(
        &out,
        "let made = Console { respond: std::sync::Arc::new(|__a0| answer(&__a0)) };",
    );
    assert_has(&out, "let h = std::sync::Arc::new(|__a0| answer(&__a0));");
    assert_has(&out, "let respond = console.respond.clone();");
    assert_has(&out, "respond(String::from(\"x\"));");
    assert_has(&out, "h(String::from(\"y\"));");
    assert_has(&out, "return f(String::from(\"z\"));");
}

fn spawn_(inner: Expr) -> Expr {
    ex(ExprKind::Spawn(Box::new(inner)))
}

#[test]
fn spawn_lowering() {
    let out = lib(vec![
        protocol("Line", &["serve"]),
        func(
            "worker",
            &[("s", ty("TcpStream")), ("n", ty("Int"))],
            Some(result_unit()),
            true,
            vec![ret(callf("Ok", vec![name("Unit")]))],
        ),
        func(
            "main_loop",
            &[("s", ty("TcpStream")), ("r", ty("Reply"))],
            Some(result_unit()),
            true,
            vec![
                expr_stmt(spawn_(callf("worker", vec![name("s"), int(1)]))),
                expr_stmt(spawn_(method(
                    name("Line"),
                    "serve",
                    vec![name("s"), name("r")],
                ))),
                expr_stmt(spawn_(call(
                    field(name("Line"), "serve"),
                    vec![name("s"), name("r")],
                ))),
                ret(callf("Ok", vec![name("Unit")])),
            ],
        ),
    ]);
    let log = "ignite_io::log(&format!(\"task ended with error: {}\", error));";
    assert_has(
        &out,
        &format!(
            "{{ let __spawn_0 = s.clone(); let __spawn_1 = 1; ignite_io::spawn(move || {{ \
             if let Err(error) = worker(&__spawn_0, &__spawn_1) {{ {log} }} }}); }};"
        ),
    );
    let role = format!(
        "{{ let __spawn_0 = s.clone(); let __spawn_1 = r.clone(); ignite_io::spawn(move || {{ \
         if let Err(error) = Line::serve(__spawn_0, __spawn_1) {{ {log} }} }}); }};"
    );
    assert_eq!(out.matches(&norm(&role)).count(), 2, "{out}");
}

#[test]
fn generate_std_nests_modules() {
    let console = Program {
        package: Some(Path {
            segments: vec![id("std"), id("console")],
            span: sp(),
        }),
        imports: vec![],
        decls: user_decls(),
    };
    let telnet = Program {
        package: Some(Path {
            segments: vec![id("std"), id("protocol"), id("telnet")],
            span: sp(),
        }),
        imports: vec![
            import(&["std", "console"], &["Reply", "Action"]),
            import(&["std", "net", "TcpStream"], &[]),
        ],
        decls: vec![protocol("Telnet", &["connect"])],
    };
    let http = Program {
        package: None,
        imports: vec![],
        decls: vec![func(
            "version",
            &[],
            Some(ty("Int")),
            false,
            vec![expr_stmt(int(1))],
        )],
    };
    let console_symbols = symbols_for(&console);
    let telnet_symbols = symbols_for(&telnet);
    let http_symbols = symbols_for(&http);
    let modules = vec![
        ("std.console".to_string(), &console, &console_symbols),
        ("std.protocol.telnet".to_string(), &telnet, &telnet_symbols),
        ("std.protocol.http".to_string(), &http, &http_symbols),
    ];
    let project = ignite_rustgen::generate_std(&modules, &options()).unwrap();
    assert_eq!(project.package_name, "ignite-std");
    assert!(project.executables.is_empty());
    let out = norm(&project.lib_rs);
    assert!(out.starts_with(&norm(ignite_rustgen::FILE_HEADER)), "{out}");
    assert_has(
        &out,
        "pub mod console { #[derive(Clone)] pub enum Command {",
    );
    assert_has(
        &out,
        "pub mod protocol { pub mod http { pub fn version() -> i64 { 1 } } \
         pub mod telnet { use crate::console::{Reply, Action}; pub struct Telnet; impl Telnet {",
    );
    let (head, tail) = project
        .cargo_toml
        .split_once("version = \"0.0.0-")
        .expect("version with content hash");
    assert_eq!(head, "[package]\nname = \"ignite-std\"\n");
    let (hash, rest) = tail.split_once("\"\n").unwrap();
    assert_eq!(hash.len(), 16, "{hash}");
    assert!(hash.bytes().all(|b| b.is_ascii_hexdigit()), "{hash}");
    assert_eq!(
        rest,
        "edition = \"2024\"\n\n\
         [lib]\npath = \"src/lib.rs\"\n\n[dependencies]\nignite-io = { path = \"/sub/ignite-io\" }\n"
    );
}

#[test]
fn std_version_follows_the_library_text() {
    let one = program(user_decls());
    let mut two = program(user_decls());
    two.decls.pop();
    let one_symbols = symbols_for(&one);
    let two_symbols = symbols_for(&two);
    let version = |p: &Program, s: &Symbols| {
        let modules = vec![("std.x".to_string(), p, s)];
        let project = ignite_rustgen::generate_std(&modules, &options()).unwrap();
        project.cargo_toml.lines().nth(2).unwrap().to_string()
    };
    assert_eq!(version(&one, &one_symbols), version(&one, &one_symbols));
    assert_ne!(version(&one, &one_symbols), version(&two, &two_symbols));
}

#[test]
fn generate_std_rejects_a_system() {
    let bad = program(vec![system(vec![ingress()], vec![])]);
    let good = program(user_decls());
    let bad_symbols = symbols_for(&bad);
    let good_symbols = symbols_for(&good);
    let modules = vec![
        ("std.good".to_string(), &good, &good_symbols),
        ("std.bad".to_string(), &bad, &bad_symbols),
    ];
    let errors = ignite_rustgen::generate_std(&modules, &options()).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].0, "std.bad");
    assert_eq!(errors[0].1[0].code, "E020");
}

#[test]
fn write_project_creates_layout() {
    let project = try_generate(user_decls()).unwrap();
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("units-write-project");
    ignite_rustgen::write_project(&project, &out).unwrap();
    ignite_rustgen::write_project(&project, &out).unwrap();
    assert_eq!(
        std::fs::read_to_string(out.join("Cargo.toml")).unwrap(),
        project.cargo_toml
    );
    assert_eq!(
        std::fs::read_to_string(out.join("src/lib.rs")).unwrap(),
        project.lib_rs
    );
    assert!(project.executables.is_empty());
    assert!(!out.join("src/bin").exists());
}

fn ptuple(items: Vec<Pattern>) -> Pattern {
    pat(PatternKind::Tuple(items))
}

fn tuple(items: Vec<Expr>) -> Expr {
    ex(ExprKind::Tuple(items))
}

fn let_pattern(pattern: Pattern, value: Expr, mutable: bool) -> Stmt {
    st(StmtKind::LetPattern {
        pattern,
        value,
        mutable,
    })
}

#[test]
fn tuples_and_pattern_lets() {
    let out = body(vec![
        let_("p", tuple(vec![name("n"), name("x")])),
        let_pattern(
            ptuple(vec![pbind("a"), pat(PatternKind::Wildcard)]),
            name("p"),
            true,
        ),
        let_pattern(pat(PatternKind::Wildcard), callf("g", vec![]), false),
        let_("u", tuple(vec![])),
    ]);
    assert_has(&out, "let p = (n.clone(), x.clone());");
    assert_has(&out, "let (mut a, _) = p.clone();");
    assert_has(&out, "let _ = g();");
    assert_has(&out, "let u = ();");
}

#[test]
fn tuple_type() {
    let out = lib(vec![func(
        "swap",
        &[("p", ty_of("Tuple", vec![ty("Int"), ty("Text")]))],
        Some(ty_of("Tuple", vec![ty("Text"), ty("Int")])),
        false,
        vec![expr_stmt(tuple(vec![int(1), text("a")]))],
    )]);
    assert_has(&out, "pub fn swap(p: &(i64, String)) -> (String, i64)");
}

#[test]
fn return_expressions() {
    let out = lib(vec![func(
        "sign",
        &[("n", ty("Int"))],
        Some(ty("Int")),
        false,
        vec![expr_stmt(match_(
            name("n"),
            vec![
                (
                    pat(PatternKind::Int(0)),
                    ex(ExprKind::Return(Some(Box::new(int(7))))),
                ),
                (pat(PatternKind::Wildcard), int(1)),
            ],
        ))],
    )]);
    assert_has(&out, "0 => return 7,");
}

#[test]
fn bool_or_and_tuple_patterns_are_native() {
    let out = body(vec![
        expr_stmt(match_(
            name("flag"),
            vec![
                (pat(PatternKind::Bool(true)), int(1)),
                (pat(PatternKind::Bool(false)), int(0)),
            ],
        )),
        expr_stmt(match_(
            name("n"),
            vec![
                (
                    pat(PatternKind::Or(vec![
                        pat(PatternKind::Int(0)),
                        pat(PatternKind::Int(1)),
                    ])),
                    int(0),
                ),
                (pat(PatternKind::Wildcard), int(1)),
            ],
        )),
        expr_stmt(match_(
            tuple(vec![name("flag"), name("n")]),
            vec![
                (
                    ptuple(vec![pat(PatternKind::Bool(true)), pbind("k")]),
                    name("k"),
                ),
                (pat(PatternKind::Wildcard), int(1)),
            ],
        )),
    ]);
    assert_has(&out, "true => 1, false => 0,");
    assert_has(&out, "0 | 1 => 0,");
    assert_has(&out, "(true, k) => k,");
}

#[test]
fn list_and_text_patterns_are_guarded_arms() {
    let out = body(vec![expr_stmt(match_(
        name("l"),
        vec![
            (pat(PatternKind::List(vec![])), int(0)),
            (pat(PatternKind::List(vec![pbind("w")])), int(1)),
            (
                pat(PatternKind::List(vec![
                    pat(PatternKind::Str("q".into())),
                    pat(PatternKind::Wildcard),
                ])),
                int(2),
            ),
            (pat(PatternKind::Wildcard), int(3)),
        ],
    ))]);
    assert_has(&out, "match l.clone() {");
    assert_has(&out, "__m if ((&__m).len() == 0) => 0,");
    assert_has(
        &out,
        "__m if ((&__m).len() == 1) => { let w = ((&(&__m)[0])).clone(); 1 },",
    );
    assert_lacks(&out, "[w]");
    assert_has(&out, "_ => 3,");
}

#[test]
fn aliases_and_field_defaults() {
    let out = lib(vec![
        Decl::Alias(AliasDecl {
            name: id("Lines"),
            params: vec![id("T")],
            ty: ty_of("List", vec![ty("T")]),
            span: sp(),
        }),
        Decl::Record(RecordDecl {
            test_only: false,
            name: id("Config"),
            fields: vec![
                Field {
                    name: id("name"),
                    ty: ty("Text"),
                    default: None,
                    span: sp(),
                },
                Field {
                    name: id("size"),
                    ty: ty("Int"),
                    default: Some(int(80)),
                    span: sp(),
                },
            ],
            span: sp(),
            type_params: Vec::new(),
        }),
        func(
            "make",
            &[],
            Some(ty("Config")),
            false,
            vec![expr_stmt(record("Config", vec![("name", text("n"))]))],
        ),
    ]);
    assert_has(&out, "pub type Lines<T> = Vec<T>;");
    assert_has(&out, "pub fn default_size() -> i64 { 80 }");
    assert_has(&out, "size: Config::default_size()");
}
