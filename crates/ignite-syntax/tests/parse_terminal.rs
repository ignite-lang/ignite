//! The standard library under `std/` and the acceptance program
//! `examples/terminal/terminal.ignite` parse, with the shapes later stages
//! rely on: every std file declares its module path as `package`, Telnet is
//! a protocol with both roles, and the program's system has a `server` and
//! a `client` component.

use std::path::{Path, PathBuf};

use ignite_syntax::ast::{Decl, ExprKind, Program, SystemDecl};
use ignite_syntax::{SourceFile, parse};

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn parse_file(path: &Path) -> (String, Program) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let source = SourceFile::new(path.display().to_string(), text.clone());
    match parse(&source) {
        Ok(program) => (text, program),
        Err(diagnostics) => panic!("{}", source.render_all(&diagnostics)),
    }
}

fn ignite_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            ignite_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "ignite") {
            out.push(path);
        }
    }
}

/// Every std file with its module path (`std/a/b.ignite` is `std.a.b`).
fn std_files() -> Vec<(String, PathBuf)> {
    let root = repository().join("std");
    let mut paths = Vec::new();
    ignite_files(&root, &mut paths);
    paths
        .into_iter()
        .map(|path| {
            let relative = path.strip_prefix(&root).expect("under std/");
            let module = std::iter::once("std".to_string())
                .chain(
                    relative
                        .with_extension("")
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned()),
                )
                .collect::<Vec<_>>()
                .join(".");
            (module, path)
        })
        .collect()
}

fn example() -> (String, Program) {
    parse_file(&repository().join("examples/terminal/terminal.ignite"))
}

fn system(program: &Program) -> &SystemDecl {
    program
        .decls
        .iter()
        .find_map(|d| match d {
            Decl::System(s) => Some(s),
            _ => None,
        })
        .expect("the program declares a system")
}

#[test]
fn std_files_parse_and_name_their_module() {
    let files = std_files();
    let modules: Vec<&str> = files.iter().map(|(m, _)| m.as_str()).collect();
    for expected in ["std.console", "std.protocol.telnet"] {
        assert!(
            modules.contains(&expected),
            "{expected} missing: {modules:?}"
        );
    }
    for (module, path) in &files {
        let (_, program) = parse_file(path);
        assert_eq!(
            program.package.as_ref().map(|p| p.dotted()).as_deref(),
            Some(module.as_str()),
            "{}",
            path.display()
        );
    }
}

#[test]
fn telnet_is_a_protocol_with_both_roles() {
    let (_, program) = parse_file(&repository().join("std/protocol/telnet.ignite"));
    let telnet = program
        .decls
        .iter()
        .find_map(|d| match d {
            Decl::Protocol(p) if p.name.name == "Telnet" => Some(p),
            _ => None,
        })
        .expect("std.protocol.telnet declares protocol Telnet");
    let serve = telnet.role("serve").expect("Telnet.serve");
    let names: Vec<&str> = serve.params.iter().map(|p| p.name.name.as_str()).collect();
    assert_eq!(names, ["stream", "limits", "application"]);
    assert_eq!(serve.bounds.len(), 1);
    assert_eq!(serve.bounds[0].param.name, "S");
    assert_eq!(serve.bounds[0].interfaces[0].name, "ByteStream");
    assert!(serve.is_effectful());
    let connect = telnet.role("connect").expect("Telnet.connect");
    assert_eq!(connect.params.len(), 1);
    assert_eq!(connect.bounds.len(), 1);
    assert!(connect.is_effectful());
}

#[test]
fn console_has_a_function_typed_field() {
    let (_, program) = parse_file(&repository().join("std/console.ignite"));
    let console = program
        .decls
        .iter()
        .find_map(|d| match d {
            Decl::Record(r) if r.name.name == "Console" => Some(r),
            _ => None,
        })
        .expect("std.console declares record Console");
    let respond = console
        .fields
        .iter()
        .find(|f| f.name.name == "respond")
        .expect("Console.respond");
    assert!(respond.ty.is_fn());
    assert_eq!(respond.ty.fn_params().len(), 1);
}

#[test]
fn example_imports_the_standard_library() {
    let (_, program) = example();
    assert_eq!(
        program.package.as_ref().map(|p| p.dotted()).as_deref(),
        Some("examples.terminal")
    );
    let imports: Vec<String> = program.imports.iter().map(|i| i.path.dotted()).collect();
    assert!(
        imports.iter().any(|i| i.starts_with("std.protocol.telnet")),
        "{imports:?}"
    );
    assert!(
        imports.iter().any(|i| i.starts_with("std.console")),
        "{imports:?}"
    );
}

#[test]
fn example_system_has_server_and_client() {
    let (_, program) = example();
    let system = system(&program);
    let components: Vec<(&str, &str)> = system
        .components
        .iter()
        .map(|c| (c.name.name.as_str(), c.kind.name.as_str()))
        .collect();
    assert_eq!(
        components,
        [
            ("console", "CommandConsole"),
            ("server", "TelnetServer"),
            ("client", "TelnetClient"),
        ]
    );
    assert!(
        system
            .resources
            .iter()
            .any(|r| r.kind.name == "TcpListener")
    );
    for (component, property, value) in [
        ("server", "application", "console"),
        ("server", "listener", "ingress"),
        ("client", "target", "ingress"),
    ] {
        let declared = system
            .components
            .iter()
            .find(|c| c.name.name == component)
            .unwrap();
        let found = declared
            .properties
            .iter()
            .find(|p| p.name.name == property)
            .unwrap_or_else(|| panic!("{component} has no `{property}`"));
        assert!(
            matches!(&found.value.kind, ExprKind::Name(n) if n.name == value),
            "{component}.{property}"
        );
    }
}

#[test]
fn every_fn_span_ends_at_its_closing_brace() {
    let (text, program) = example();
    for decl in &program.decls {
        let Decl::Fn(f) = decl else { continue };
        let start = text
            .find(&format!("fn {}(", f.name.name))
            .expect("fn in source");
        let close = text[start..].find("\n}").expect("closing brace") + start + 1;
        assert_eq!(decl.span(), ignite_syntax::ast::Span::new(start, close + 1));
    }
}
