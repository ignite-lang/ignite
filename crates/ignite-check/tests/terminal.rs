//! The standard library under `std/` checks clean in import order with
//! `check_module`, and the acceptance program checks clean against it with
//! the symbols the generator needs.

use crate::common;

use std::path::{Path, PathBuf};

use ignite_check::{Modules, Symbols, check_module};
use ignite_syntax::SourceFile;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn standard_library() -> (Vec<String>, Modules) {
    let (files, modules) = common::load_std(&repository().join("std"))
        .unwrap_or_else(|e| panic!("standard library:\n{e}"));
    (files.into_iter().map(|f| f.module).collect(), modules)
}

fn terminal_symbols(modules: &Modules) -> Symbols {
    let path = repository().join("examples/terminal/terminal.ignite");
    let text = std::fs::read_to_string(&path).unwrap();
    let source = SourceFile::new("terminal.ignite", text);
    let program = ignite_syntax::parse(&source)
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
    assert!(position("std.text") < position("std.console"));
    assert!(position("std.console") < position("std.protocol.telnet"));

    let console = &modules["std.console"];
    assert!(console.exports("Console"));
    assert!(console.exports("Reply"));
    assert!(console.exports("Action"));
    assert_eq!(
        console.imported.iter().collect::<Vec<_>>(),
        [(&"is_whitespace".to_string(), &"std.text".to_string())]
    );
    assert!(
        !console.exports("is_whitespace"),
        "imports are not re-exported"
    );

    let telnet = &modules["std.protocol.telnet"];
    let roles = &telnet.protocols["Telnet"];
    assert_eq!(roles["serve"].arity, 3);
    assert_eq!(roles["connect"].arity, 1);
    assert!(telnet.exports("Telnet"));
    assert_eq!(
        telnet.imported.get("Console").map(String::as_str),
        Some("std.console")
    );
    assert!(!telnet.exports("Console"), "imports are not re-exported");

    let rfc = &modules["std.rfc"];
    for name in ["Level", "Party", "Source", "Requirement", "Spec"] {
        assert!(rfc.exports(name), "std.rfc exports {name}");
    }
    assert!(rfc.imported.is_empty(), "std.rfc is a leaf");
}

#[test]
fn terminal_checks_clean_against_the_standard_library() {
    let (_, modules) = standard_library();
    let symbols = terminal_symbols(&modules);

    for (name, module) in [
        ("TelnetServer", "std.protocol.telnet"),
        ("TelnetClient", "std.protocol.telnet"),
        ("CommandConsole", "std.console"),
    ] {
        assert_eq!(
            symbols.imported.get(name).map(String::as_str),
            Some(module),
            "{name}"
        );
        assert!(symbols.records.contains(name), "{name}");
    }
    assert!(
        symbols.record_fields["CommandConsole"].contains(&"implementation".to_string()),
        "{:?}",
        symbols.record_fields
    );
    assert!(
        symbols.record_fields["TelnetServer"].contains(&"application".to_string()),
        "{:?}",
        symbols.record_fields
    );
    assert!(
        !symbols.imported.contains_key("Telnet"),
        "the program names the kinds, not the protocol (docs/slice.md section 14.5)"
    );
    // `TelnetServer.machine` and `TelnetServer.validate` are the hooks of
    // `docs/slice.md` 16.16 that `ignite run` and E120 call (slice 12); the
    // associated fns of an imported kind come with its record (E003).
    assert_eq!(
        symbols.functions.keys().collect::<Vec<_>>(),
        [
            "TelnetClient.main",
            "TelnetServer.machine",
            "TelnetServer.main",
            "TelnetServer.validate"
        ],
        "the kinds' mains and hooks come with the imported records; the commands live in the sibling module `commands.ignite`"
    );
    assert_eq!(symbols.enum_of_variant("Some"), Some("Option"));
}
