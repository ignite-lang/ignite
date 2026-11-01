//! The standard library under `std/` checks clean in import order with
//! `check_module`, and the acceptance program checks clean against it with
//! the symbols the generator needs.

mod common;

use std::path::{Path, PathBuf};

use telos_check::{Modules, Symbols, check_module};
use telos_syntax::SourceFile;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn standard_library() -> (Vec<String>, Modules) {
    let (files, modules) = common::load_std(&repository().join("std"))
        .unwrap_or_else(|e| panic!("standard library:\n{e}"));
    (files.into_iter().map(|f| f.module).collect(), modules)
}

fn terminal_symbols(modules: &Modules) -> Symbols {
    let path = repository().join("examples/terminal/terminal.telos");
    let text = std::fs::read_to_string(&path).unwrap();
    let source = SourceFile::new("terminal.telos", text);
    let program = telos_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    check_module(&program, modules)
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)))
}

#[test]
fn standard_library_checks_in_import_order() {
    let (order, modules) = standard_library();
    let position = |m: &str| {
        order
            .iter()
            .position(|o| o == m)
            .unwrap_or_else(|| panic!("{m} missing from {order:?}"))
    };
    assert!(position("std.console") < position("std.protocol.telnet"));

    let console = &modules["std.console"];
    assert!(console.exports("Console"));
    assert!(console.exports("Reply"));
    assert!(console.exports("Action"));
    assert!(console.imported.is_empty(), "{:?}", console.imported);

    let telnet = &modules["std.protocol.telnet"];
    let roles = &telnet.protocols["Telnet"];
    assert_eq!(roles["serve"].arity, 2);
    assert_eq!(roles["connect"].arity, 1);
    assert!(telnet.exports("Telnet"));
    assert_eq!(
        telnet.imported.get("Console").map(String::as_str),
        Some("std.console")
    );
    assert!(!telnet.exports("Console"), "imports are not re-exported");
}

#[test]
fn terminal_checks_clean_against_the_standard_library() {
    let (_, modules) = standard_library();
    let symbols = terminal_symbols(&modules);

    assert_eq!(
        symbols.imported.get("Telnet").map(String::as_str),
        Some("std.protocol.telnet")
    );
    assert_eq!(
        symbols.imported.get("Console").map(String::as_str),
        Some("std.console")
    );
    assert!(symbols.protocols["Telnet"].contains_key("serve"));
    assert!(symbols.protocols["Telnet"].contains_key("connect"));
    assert!(symbols.records.contains("Console"));
    assert!(
        symbols.record_fields["Console"].contains(&"respond".to_string()),
        "{:?}",
        symbols.record_fields
    );

    let execute = &symbols.functions["execute"];
    assert!(!execute.effectful, "execute is passed as a function value");
    assert_eq!(execute.arity, 1);
    assert!(!symbols.imported.contains_key("execute"));

    assert_eq!(symbols.enum_of_variant("Help"), Some("Command"));
    assert_eq!(
        symbols.enum_of_variant("Close"),
        None,
        "`Action` is not imported; the type checker resolves `Close` by the field type"
    );
    assert_eq!(symbols.enum_of_variant("Some"), Some("Option"));
}
