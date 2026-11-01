//! What `check_module` records about imports, checked against the fixture
//! standard library under `tests/fixtures/std/`.

mod common;

use std::path::Path;

use telos_check::{FnInfo, Modules, check, check_module};
use telos_syntax::SourceFile;
use telos_syntax::ast::Program;

fn modules() -> Modules {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/std");
    common::load_std(&root).unwrap_or_else(|e| panic!("{e}")).1
}

fn program(text: &str) -> Program {
    let source = SourceFile::new("t.telos", text);
    telos_syntax::parse(&source).unwrap_or_else(|d| panic!("{}", source.render_all(&d)))
}

#[test]
fn check_is_check_module_without_modules() {
    let text = "import std.net.{TcpStream}\nenum E { A, B(Int) }\ntype R { e: E }\nfn f(r: R) -> E { return r.e }\nprotocol P {\n  fn connect(s: TcpStream) -> Result<Unit, IoError> requires [net] { return Ok(Unit) }\n}";
    let program = program(text);
    assert_eq!(check(&program), check_module(&program, &Modules::new()));
    assert!(check(&program).is_ok());

    let broken = self::program("import std.greeting.{Greeting}");
    assert_eq!(check(&broken), check_module(&broken, &Modules::new()));
    assert_eq!(
        check(&broken)
            .unwrap_err()
            .iter()
            .map(|d| d.code)
            .collect::<Vec<_>>(),
        ["E031"]
    );
}

#[test]
fn imported_definitions_enter_symbols() {
    let modules = modules();
    let program = program(
        "import std.greeting.{Greeting, Mood, greet, send}\nimport std.protocol.hello.Hello\nfn own() -> Int { return 1 }",
    );
    let symbols = check_module(&program, &modules).unwrap_or_else(|d| panic!("{d:?}"));

    let imported: Vec<(&str, &str)> = symbols
        .imported
        .iter()
        .map(|(n, m)| (n.as_str(), m.as_str()))
        .collect();
    assert_eq!(
        imported,
        [
            ("Greeting", "std.greeting"),
            ("Hello", "std.protocol.hello"),
            ("Mood", "std.greeting"),
            ("greet", "std.greeting"),
            ("send", "std.greeting"),
        ]
    );
    assert!(symbols.records.contains("Greeting"));
    assert_eq!(symbols.record_fields["Greeting"], ["text", "mood"]);
    assert_eq!(symbols.enums["Mood"], ["Happy", "Grumpy"]);
    assert_eq!(symbols.payloads["Mood"]["Grumpy"], 1);
    assert_eq!(symbols.enum_of_variant("Happy"), Some("Mood"));
    assert_eq!(
        symbols.functions["send"],
        FnInfo {
            effectful: true,
            arity: 2
        }
    );
    assert_eq!(
        symbols.functions["own"],
        FnInfo {
            effectful: false,
            arity: 0
        }
    );
    let roles: Vec<(&str, usize)> = symbols.protocols["Hello"]
        .iter()
        .map(|(r, i)| (r.as_str(), i.arity))
        .collect();
    assert_eq!(roles, [("connect", 1), ("serve", 2)]);

    assert!(!symbols.exports("Greeting"), "imports are not exports");
    assert!(symbols.exports("own"));
    assert!(!symbols.exports("Option"), "builtins are not exports");
}

#[test]
fn only_named_imports_enter_scope() {
    let modules = modules();
    let program =
        program("import std.greeting.{Greeting}\nfn f() -> Greeting {\n  return greet(\"x\")\n}");
    let diagnostics = check_module(&program, &modules).unwrap_err();
    let codes: Vec<&str> = diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["E003"]);
}

#[test]
fn constants_are_exported_imported_and_listed() {
    let modules = modules();
    let greeting = &modules["std.greeting"];
    let own: Vec<&str> = greeting.constants.iter().map(String::as_str).collect();
    assert_eq!(own, ["BASE_LOUDNESS", "DEFAULT_NAME", "LOUDNESS"]);
    assert!(greeting.exports("LOUDNESS"));
    assert!(!modules["std.protocol.hello"].exports("DEFAULT_NAME"));

    let program = program(
        "import std.greeting.{LOUDNESS}\nconst LOCAL: Int = LOUDNESS + 1\nfn f() -> Int { return LOCAL }",
    );
    let symbols = check_module(&program, &modules).unwrap_or_else(|d| panic!("{d:?}"));
    let constants: Vec<&str> = symbols.constants.iter().map(String::as_str).collect();
    assert_eq!(constants, ["LOCAL", "LOUDNESS"]);
    assert_eq!(symbols.imported["LOUDNESS"], "std.greeting");
    assert!(symbols.exports("LOCAL"));
    assert!(!symbols.exports("LOUDNESS"));
    assert!(!symbols.functions.contains_key("LOCAL"));
}
