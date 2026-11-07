//! The standard library under `std/` types clean in import order, and the
//! acceptance program types clean against it; every expression gets a type
//! and every call a callee; spot checks of types the generator relies on.

use crate::common;

use std::path::{Path, PathBuf};

use ignite_check::check_module;
use ignite_syntax::SourceFile;
use ignite_syntax::ast::{Expr, ExprKind, Program};
use ignite_types::{Builtin, Callee, QualName, Ty, TypedProgram, type_module};

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn standard_library() -> common::Std {
    common::load_std(&repository().join("std")).unwrap_or_else(|e| panic!("standard library:\n{e}"))
}

fn terminal_program() -> (SourceFile, Program) {
    let path = repository().join("examples/terminal/terminal.ignite");
    let text = std::fs::read_to_string(&path).unwrap();
    let source = SourceFile::new("terminal.ignite", text);
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    (source, program)
}

fn bytes() -> Ty {
    Ty::List(Box::new(Ty::Byte))
}

fn io(ok: Ty) -> Ty {
    Ty::Result(Box::new(ok), Box::new(Ty::IoError))
}

fn qual(module: &str, name: &str) -> QualName {
    QualName::new(module, name)
}

/// Expressions of `program` whose source text is exactly `text`.
fn find<'p>(source: &SourceFile, program: &'p Program, text: &str) -> Vec<&'p Expr> {
    let found: Vec<&Expr> = common::expressions(program)
        .into_iter()
        .filter(|e| &source.text[e.span.start..e.span.end] == text)
        .collect();
    assert!(!found.is_empty(), "no expression `{text}`");
    found
}

/// The type every expression with source text `text` has (all must agree).
fn type_of_text(typed: &TypedProgram, source: &SourceFile, program: &Program, text: &str) -> Ty {
    let found = find(source, program, text);
    let ty = typed.type_of(found[0]).clone();
    for e in &found {
        assert_eq!(typed.type_of(e), &ty, "`{text}` typed differently");
    }
    ty
}

/// The type of the binding `name` at its first occurrence inside the first
/// occurrence of `context`.
fn binding(typed: &TypedProgram, source: &SourceFile, context: &str, name: &str) -> Ty {
    let at = source
        .text
        .find(context)
        .unwrap_or_else(|| panic!("`{context}` not in source"));
    let start = at + context.find(name).expect("name inside context");
    let span = ignite_syntax::ast::Span::in_file(start, start + name.len(), source.id());
    typed
        .binding_type(span)
        .unwrap_or_else(|| panic!("no binding type for `{name}` at {start}"))
        .clone()
}

/// Every expression has a type and every call or method call a callee.
fn assert_complete(typed: &TypedProgram, source: &SourceFile, program: &Program) {
    for e in common::expressions(program) {
        let text = &source.text[e.span.start..e.span.end];
        assert!(typed.try_type_of(e).is_some(), "untyped `{text}`");
        if matches!(e.kind, ExprKind::Call { .. } | ExprKind::Method { .. }) {
            assert!(typed.callee(e).is_some(), "no callee for `{text}`");
        }
    }
}

#[test]
fn standard_library_types_in_import_order() {
    let std = standard_library();
    let order: Vec<&str> = std.files.iter().map(|f| f.module.as_str()).collect();
    let at = |module: &str| {
        order
            .iter()
            .position(|m| *m == module)
            .expect("module loaded")
    };
    assert!(at("std.text") < at("std.console"));
    assert!(at("std.console") < at("std.protocol.telnet"));
    assert_eq!(order.len(), std.typed.len());

    let console = &std.typed["std.console"];
    assert_eq!(
        console.records["Console"],
        vec![
            ("prompt".to_string(), Ty::Text),
            (
                "respond".to_string(),
                Ty::Fn(
                    vec![Ty::Text],
                    Box::new(Ty::Record(qual("std.console", "Reply")))
                )
            ),
            ("maximum_line".to_string(), Ty::Int),
        ]
    );
    assert!(
        !console.fns.contains_key("is_whitespace"),
        "imported, not declared"
    );
    let text = &std.typed["std.text"];
    assert_eq!(text.fns["is_whitespace"].params, vec![Ty::Byte]);
    assert_eq!(text.fns["is_whitespace"].ret, Ty::Bool);
    assert!(!console.fns["first_word"].effectful);

    let telnet = &std.typed["std.protocol.telnet"];
    assert_eq!(telnet.consts["IAC"], Ty::Byte);
    assert_eq!(telnet.consts["MAXIMUM_SUBNEGOTIATION"], Ty::Int);
    assert_eq!(telnet.consts["COMMAND_PROMPT"], Ty::Text);
    let serve = &telnet.protocols["Telnet"]["serve"];
    assert_eq!(
        serve.params,
        vec![
            Ty::Param("S".to_string()),
            Ty::Record(qual("std.protocol.telnet", "SessionLimits")),
            Ty::Record(qual("std.console", "Console"))
        ]
    );
    assert_eq!(serve.ret, io(Ty::Unit));
    assert!(serve.effectful);
    assert_eq!(
        telnet.enums["Decoder"][3],
        ("Negotiation".to_string(), vec![Ty::Byte])
    );
    assert_eq!(
        telnet.enums["Event"][3],
        ("Answer".to_string(), vec![bytes()])
    );
    assert!(
        !telnet.records.contains_key("Console"),
        "imports are not exports"
    );
}

#[test]
fn standard_library_expressions_are_typed() {
    let std = standard_library();
    let file = |module: &str| {
        std.files
            .iter()
            .find(|f| f.module == module)
            .expect("module loaded")
    };
    let console = file("std.console");
    let typed = common::retype(&console.program, &console.symbols, &std.typed);
    assert_complete(&typed, &console.source, &console.program);
    let (src, prog) = (&console.source, &console.program);
    assert_eq!(type_of_text(&typed, src, prog, "bytes.at(at)"), Ty::Byte);
    assert_eq!(
        typed.callee(find(src, prog, "bytes.at(at)")[0]),
        Some(&Callee::Builtin(Builtin::ListAt))
    );
    assert_eq!(type_of_text(&typed, src, prog, "line.to_bytes()"), bytes());
    assert_eq!(type_of_text(&typed, src, prog, "0"), Ty::Int);
    assert_eq!(binding(&typed, src, "var at", "at"), Ty::Int);

    let telnet = file("std.protocol.telnet");
    let typed = common::retype(&telnet.program, &telnet.symbols, &std.typed);
    assert_complete(&typed, &telnet.source, &telnet.program);
    let (src, prog) = (&telnet.source, &telnet.program);
    assert_eq!(
        type_of_text(&typed, src, prog, "stream.read(4096)"),
        io(Ty::Input)
    );
    // Slice 14: `Telnet.connect` and `session` are generic over
    // `S: ByteStream`, so their reads are the bound's `StreamRead`;
    // `await_prompt` keeps a `TcpStream` receiver (`docs/slice.md` 18.5).
    let reads = find(src, prog, "stream.read(4096)");
    let callees: Vec<_> = reads.iter().map(|e| typed.callee(e)).collect();
    assert_eq!(
        callees,
        [
            Some(&Callee::Builtin(Builtin::StreamRead)),
            Some(&Callee::Builtin(Builtin::TcpStreamRead)),
            Some(&Callee::Builtin(Builtin::StreamRead)),
        ]
    );
    assert_eq!(type_of_text(&typed, src, prog, "4096"), Ty::Int);
    assert_eq!(type_of_text(&typed, src, prog, "bytes.at(i)"), Ty::Byte);
    assert_eq!(
        type_of_text(&typed, src, prog, "stream.read(4096)?"),
        Ty::Input
    );
    assert_eq!(
        type_of_text(&typed, src, prog, "Process.exit(0)"),
        Ty::Never
    );
    assert_eq!(
        typed.callee(find(src, prog, "Process.exit(0)")[0]),
        Some(&Callee::Builtin(Builtin::ProcessExit))
    );
    assert_eq!(type_of_text(&typed, src, prog, "Bytes.empty()"), bytes());
    assert_eq!(type_of_text(&typed, src, prog, "[IAC, WONT, b]"), bytes());
    assert_eq!(type_of_text(&typed, src, prog, "IAC"), Ty::Byte);
    assert_eq!(
        typed.callee(find(src, prog, "IAC")[0]),
        Some(&Callee::Const(qual("std.protocol.telnet", "IAC")))
    );
    assert_eq!(
        type_of_text(&typed, src, prog, "Answer([IAC, WONT, b])"),
        Ty::Enum(qual("std.protocol.telnet", "Event"))
    );
    assert_eq!(
        typed.callee(find(src, prog, "Answer([IAC, WONT, b])")[0]),
        Some(&Callee::Variant {
            enum_: qual("std.protocol.telnet", "Event"),
            index: 3
        })
    );
    for ground in find(src, prog, "Ground") {
        assert_eq!(
            typed.callee(ground),
            Some(&Callee::Variant {
                enum_: qual("std.protocol.telnet", "Decoder"),
                index: 0
            })
        );
    }
    assert_eq!(
        type_of_text(&typed, src, prog, "(Interpret, Nothing)"),
        Ty::Tuple(vec![
            Ty::Enum(qual("std.protocol.telnet", "Decoder")),
            Ty::Enum(qual("std.protocol.telnet", "Event")),
        ])
    );
    let interrupts = find(src, prog, "Interrupt");
    let first_in = |function: &str| {
        let at = src.text.find(function).expect("function in source");
        let e = interrupts
            .iter()
            .find(|e| e.span.start > at)
            .expect("`Interrupt` in the function");
        typed.callee(e).cloned()
    };
    assert_eq!(
        [first_in("fn decode("), first_in("fn apply_key(")],
        [
            Some(Callee::Variant {
                enum_: qual("std.protocol.telnet", "Event"),
                index: 5
            }),
            Some(Callee::Variant {
                enum_: qual("std.protocol.telnet", "Edit"),
                index: 3
            }),
        ],
        "bare `Interrupt` resolves against the tuple element type: Event in decode, Edit in apply_key"
    );
    assert_eq!(
        type_of_text(&typed, src, prog, "application.respond(line.to_text())"),
        Ty::Record(qual("std.console", "Reply"))
    );
    assert_eq!(
        typed.callee(find(src, prog, "application.respond(line.to_text())")[0]),
        Some(&Callee::FieldFn {
            record: qual("std.console", "Console"),
            field: "respond".to_string()
        })
    );
    assert_eq!(
        typed.callee(find(src, prog, "session(stream, limits, application)")[0]),
        Some(&Callee::Fn(qual("std.protocol.telnet", "session")))
    );
    assert_eq!(type_of_text(&typed, src, prog, "Ok(true)"), io(Ty::Bool));
    assert_eq!(
        typed.callee(find(src, prog, "Ok(true)")[0]),
        Some(&Callee::Variant {
            enum_: QualName::builtin("Result"),
            index: 0
        })
    );
    assert_eq!(
        type_of_text(&typed, src, prog, "spawn edit_input(stream)"),
        Ty::Unit
    );
    assert_eq!(binding(&typed, src, "Literal(c)", "c"), Ty::Byte);
    assert_eq!(
        binding(&typed, src, "Subnegotiation(count)", "count"),
        Ty::Int
    );
    assert_eq!(binding(&typed, src, "Answer(answer)", "answer"), bytes());
    assert_eq!(
        binding(&typed, src, "let (next, event)", "next"),
        Ty::Enum(qual("std.protocol.telnet", "Decoder"))
    );
    assert_eq!(
        binding(&typed, src, "let (next, event)", "event"),
        Ty::Enum(qual("std.protocol.telnet", "Event"))
    );
    assert_eq!(
        binding(&typed, src, "for line", "line"),
        Ty::Text,
        "for line in reply.lines"
    );
    assert_eq!(
        binding(&typed, src, "var state", "state"),
        Ty::Enum(qual("std.protocol.telnet", "Decoder")),
        "var state"
    );
    assert_eq!(typed.variant(&Ty::Input, "Data"), Some((0, vec![bytes()])));
}

#[test]
fn todo_program_types_against_the_standard_library() {
    let mut std = standard_library();
    sibling(&mut std, "todo", "health");
    let path = repository().join("examples/todo/todo.ignite");
    let source = SourceFile::new("todo.ignite", std::fs::read_to_string(&path).unwrap());
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    let symbols = check_module(&program, &std.modules)
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let typed = type_module(&program, &symbols, &std.typed)
        .unwrap_or_else(|d| panic!("type failed:\n{}", source.render_all(&d)));
    assert_eq!(typed.module, "examples.todo");
    assert_complete(&typed, &source, &program);
}

/// The sibling module `examples/<example>/<name>.ignite` (`docs/slice.md` section
/// 14.4), checked and typed against the standard library and the siblings
/// typed before it, and added to both for the program.
fn sibling(
    std: &mut common::Std,
    example: &str,
    name: &str,
) -> (SourceFile, Program, TypedProgram) {
    let path = repository().join(format!("examples/{example}/{name}.ignite"));
    let source = SourceFile::new(
        format!("{name}.ignite"),
        std::fs::read_to_string(&path).unwrap(),
    );
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    let symbols = check_module(&program, &std.modules)
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let typed = type_module(&program, &symbols, &std.typed)
        .unwrap_or_else(|d| panic!("type failed:\n{}", source.render_all(&d)));
    let module = format!("examples.{example}.{name}");
    assert_eq!(typed.module, module);
    std.typed.insert(module.clone(), typed.exports());
    std.modules.insert(module, symbols);
    (source, program, typed)
}

#[test]
fn terminal_program_types_against_the_standard_library() {
    let mut std = standard_library();
    let (source, program, typed) = sibling(&mut std, "terminal", "commands");
    sibling(&mut std, "terminal", "command_schema");
    assert_complete(&typed, &source, &program);

    let command = Ty::Enum(qual("examples.terminal.commands", "Command"));
    let reply = Ty::Record(qual("std.console", "Reply"));
    let (src, prog) = (&source, &program);
    assert_eq!(
        type_of_text(&typed, src, prog, "Some(Help)"),
        Ty::Option(Box::new(command.clone()))
    );
    assert_eq!(type_of_text(&typed, src, prog, "Help"), command);
    assert_eq!(
        typed.callee(find(src, prog, "Some(Help)")[0]),
        Some(&Callee::Variant {
            enum_: QualName::builtin("Option"),
            index: 0
        })
    );
    assert_eq!(
        type_of_text(&typed, src, prog, "None"),
        Ty::Option(Box::new(command.clone()))
    );
    assert_eq!(
        typed.callee(find(src, prog, "first_word(line)")[0]),
        Some(&Callee::Fn(qual("std.console", "first_word")))
    );
    assert_eq!(
        type_of_text(&typed, src, prog, "Close"),
        Ty::Enum(qual("std.console", "Action"))
    );
    assert_eq!(
        typed.callee(find(src, prog, "Close")[0]),
        Some(&Callee::Variant {
            enum_: qual("std.console", "Action"),
            index: 1
        })
    );
    assert_eq!(
        type_of_text(&typed, src, prog, "Reply { lines = [text] }"),
        reply,
        "a literal omitting the defaulted `action`"
    );
    assert!(typed.field_has_default(&qual("std.console", "Reply"), "action"));
    assert_eq!(
        type_of_text(
            &typed,
            src,
            prog,
            "\"unknown command: {first_word(trimmed)}\""
        ),
        Ty::Text
    );
    assert_eq!(
        type_of_text(&typed, src, prog, "[]"),
        Ty::List(Box::new(Ty::Text))
    );
    assert_eq!(type_of_text(&typed, src, prog, "line.trim()"), Ty::Text);
    assert_eq!(
        binding(&typed, src, "Some(command)", "command"),
        command,
        "Some(command) pattern"
    );
    assert_eq!(
        binding(&typed, src, "Echo(text)", "text"),
        Ty::Text,
        "Command.Echo(text)"
    );

    let exports = typed.exports();
    assert_eq!(exports.fns["respond"].params, vec![Ty::Text]);
    assert_eq!(exports.fns["respond"].ret, reply);

    let (source, program) = terminal_program();
    let symbols = check_module(&program, &std.modules)
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let typed = type_module(&program, &symbols, &std.typed)
        .unwrap_or_else(|d| panic!("type failed:\n{}", source.render_all(&d)));
    assert_eq!(typed.module, "examples.terminal");
    assert_complete(&typed, &source, &program);
    let system_application = system_property(&program, "application");
    assert_eq!(
        typed.type_of(system_application),
        &Ty::Record(qual("std.console", "CommandConsole")),
        "`application = console` embeds the console component"
    );
    let executables: Vec<&str> = typed
        .executables()
        .iter()
        .map(|e| e.executable.as_str())
        .collect();
    assert_eq!(executables, ["terminald", "tn"]);
    assert_eq!(
        typed
            .record_fields(&qual("std.console", "Reply"))
            .map(<[_]>::len),
        Some(2)
    );
}

fn system_property<'p>(program: &'p Program, name: &str) -> &'p Expr {
    program
        .decls
        .iter()
        .find_map(|d| match d {
            ignite_syntax::ast::Decl::System(s) => s
                .components
                .iter()
                .flat_map(|c| &c.properties)
                .find(|p| p.name.name == name)
                .map(|p| &p.value),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no system property `{name}`"))
}
