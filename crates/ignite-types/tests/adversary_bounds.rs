//! Adversarial cases for slice 14 U1 (`docs/slice.md` 18.3, 18.13):
//! `TypedProgram::bound` through the public API, own module and a module
//! typed before it, and E096 for uses across a module boundary.

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_types::{QualName, TypedModules, TypedProgram, type_module};

const STREAMS: &str = r#"package std.streams

type Config { port: Int }

protocol Relay {
  fn serve<S: ByteStream>(stream: S, application: Int) -> Result<Unit, IoError> requires [stream.read, stream.write] = stream.close()
}

fn same<S: ByteStream>(stream: S) -> S = stream

fn Config.open<S: ByteStream, T>(stream: S, other: T) -> S = stream
"#;

fn typed(
    name: &str,
    text: &str,
    modules: &Modules,
    typed: &TypedModules,
) -> Result<(ignite_check::Symbols, TypedProgram), String> {
    let source = SourceFile::new(name, text.to_string());
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    let symbols = check_module(&program, modules)
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    type_module(&program, &symbols, typed)
        .map(|t| (symbols, t))
        .map_err(|d| source.render_all(&d))
}

fn streams() -> (Modules, TypedModules, TypedProgram) {
    let (symbols, program) = typed(
        "streams.ignite",
        STREAMS,
        &Modules::new(),
        &TypedModules::new(),
    )
    .unwrap_or_else(|e| panic!("std.streams:\n{e}"));
    let mut modules = Modules::new();
    modules.insert("std.streams".to_string(), symbols);
    let mut types = TypedModules::new();
    types.insert("std.streams".to_string(), program.exports());
    (modules, types, program)
}

#[test]
fn bound_answers_for_fns_associated_fns_and_roles_of_the_own_module() {
    let (_, _, program) = streams();
    let q = |name: &str| QualName::new("std.streams", name);
    assert_eq!(program.bound(&q("same"), "S"), Some("ByteStream"));
    assert_eq!(program.bound(&q("Config.open"), "S"), Some("ByteStream"));
    assert_eq!(program.bound(&q("Config.open"), "T"), None);
    assert_eq!(program.bound(&q("Relay.serve"), "S"), Some("ByteStream"));
    assert_eq!(program.bound(&q("same"), "T"), None);
}

#[test]
fn bound_answers_for_a_module_typed_before_the_program() {
    let (modules, types, _) = streams();
    let source =
        "import std.streams.{same}\n\nfn over(stream: TcpStream) -> TcpStream = same(stream)\n";
    let (_, program) =
        typed("main.ignite", source, &modules, &types).unwrap_or_else(|e| panic!("main:\n{e}"));
    let q = |name: &str| QualName::new("std.streams", name);
    assert_eq!(program.bound(&q("same"), "S"), Some("ByteStream"));
    assert_eq!(program.bound(&q("Relay.serve"), "S"), Some("ByteStream"));
}

#[test]
fn e096_holds_for_uses_of_another_modules_bounded_fn_and_role() {
    let (modules, types, _) = streams();
    let source = "import std.streams.{same, Relay, Config}\n\nfn direct() -> Int = same(1)\n\nfn role() -> Result<Unit, IoError> requires [] = Relay.serve(1, 2)\n\nfn associated() -> Int = Config.open(1, 2)\n";
    let rendered = match typed("main.ignite", source, &modules, &types) {
        Ok(_) => String::new(),
        Err(rendered) => rendered,
    };
    let e096: Vec<&str> = rendered.lines().filter(|l| l.contains("[E096]")).collect();
    assert_eq!(
        e096.len(),
        3,
        "expected E096 at `same(1)`, `Relay.serve(1, 2)` and `Config.open(1, 2)`, got:\n{rendered}"
    );
}
