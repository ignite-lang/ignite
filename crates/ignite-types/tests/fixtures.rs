//! Every `tests/fixtures/*.ignite` is parsed, checked with `check_module` and
//! typed with `type_module` against the small standard library under
//! `tests/fixtures/std/` (module `std.a.b` is `std/a/b.ignite`); the rendered
//! type diagnostics (one per line, trailing newline; empty for a clean file)
//! must equal the sibling `.expected` file. A parse or check failure fails
//! the test, and so does any diagnostic in the fixture standard library.

use crate::common;

use std::path::{Path, PathBuf};

use ignite_syntax::SourceFile;

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixtures() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(fixture_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "ignite"))
        .collect();
    paths.sort();
    paths
}

fn render(path: &Path, std: &common::Std) -> Result<String, String> {
    let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
    let text = std::fs::read_to_string(path).unwrap();
    let source = SourceFile::new(file_name, text);
    let program = ignite_syntax::parse(&source)
        .map_err(|d| format!("parse failed:\n{}", source.render_all(&d)))?;
    let symbols = ignite_check::check_module(&program, &std.modules)
        .map_err(|d| format!("check failed:\n{}", source.render_all(&d)))?;
    Ok(
        match ignite_types::type_module(&program, &symbols, &std.typed) {
            Ok(typed) => {
                for e in common::expressions(&program) {
                    if typed.try_type_of(e).is_none() {
                        return Err(format!(
                            "typed clean but `{}` has no type",
                            &source.text[e.span.start..e.span.end]
                        ));
                    }
                }
                String::new()
            }
            Err(diagnostics) => format!("{}\n", source.render_all(&diagnostics)),
        },
    )
}

#[test]
fn fixtures_match_expected() {
    let std = common::load_std(&fixture_dir().join("std"))
        .unwrap_or_else(|e| panic!("fixture standard library:\n{e}"));
    assert_eq!(
        std.typed.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "std.net",
            "std.rfc",
            "std.rfc.rfc1",
            "std.shapes",
            "std.shapes.protocol"
        ]
    );
    let paths = fixtures();
    assert!(!paths.is_empty(), "no fixtures found");
    let mut failures = Vec::new();
    for path in &paths {
        let expected_path = path.with_extension("expected");
        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("{}: {e}", expected_path.display()));
        match render(path, &std) {
            Ok(actual) if actual == expected => {}
            Ok(actual) => failures.push(format!(
                "{}\n--- expected\n{expected}--- actual\n{actual}",
                path.display()
            )),
            Err(message) => failures.push(format!("{}: {message}", path.display())),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// `valid_clauses.ignite` through `clause_constant` (`docs/slice.md` 19.6,
/// 19.13): every `conforms` entry and every requirement reference resolves
/// to the constant it names, imported or local.
#[test]
fn valid_clauses_record_their_constants() {
    use ignite_syntax::ast::Decl;
    use ignite_types::QualName;

    let std = common::load_std(&fixture_dir().join("std"))
        .unwrap_or_else(|e| panic!("fixture standard library:\n{e}"));
    let path = fixture_dir().join("valid_clauses.ignite");
    let source = SourceFile::new(
        "valid_clauses.ignite",
        std::fs::read_to_string(&path).unwrap(),
    );
    let program = ignite_syntax::parse(&source).unwrap();
    let symbols = ignite_check::check_module(&program, &std.modules).unwrap();
    let typed = ignite_types::type_module(&program, &symbols, &std.typed)
        .unwrap_or_else(|d| panic!("type failed:\n{}", source.render_all(&d)));
    let resolved = |span| {
        typed
            .clause_constant(span)
            .map(|q: &QualName| format!("{}.{}", q.module, q.name))
    };
    let mut names = Vec::new();
    for decl in &program.decls {
        match decl {
            Decl::Protocol(p) => {
                names.extend(p.conforms.iter().map(|c| resolved(c.span)));
                names.extend(
                    p.fns
                        .iter()
                        .flat_map(|f| &f.satisfies)
                        .map(|r| resolved(r.span)),
                );
                names.extend(p.statuses.iter().map(|s| resolved(s.requirement.span)));
                names.extend(
                    p.statuses
                        .iter()
                        .map(|s| resolved(s.requirement.segments[0].span)),
                );
            }
            Decl::Record(r) => {
                names.extend(r.satisfies.iter().map(|s| resolved(s.span)));
                names.extend(
                    r.fields
                        .iter()
                        .flat_map(|f| &f.satisfies)
                        .map(|s| resolved(s.span)),
                );
            }
            Decl::Test(t) => names.extend(t.verifies.iter().map(|v| resolved(v.span))),
            _ => {}
        }
    }
    let names: Vec<String> = names
        .into_iter()
        .map(|n| n.unwrap_or_else(|| "unresolved".to_string()))
        .collect();
    assert_eq!(
        names,
        [
            "std.rfc.rfc1.RFC1",
            "fixtures.clauses.RFC7",
            "std.rfc.rfc1.R2_1",
            "std.rfc.rfc1.R2_2",
            "fixtures.clauses.R1_1",
            "std.rfc.rfc1.RFC1",
            "fixtures.clauses.RFC7",
            "fixtures.clauses.R1_2",
            "std.rfc.rfc1.R2_2",
            "std.rfc.rfc1.R2_1",
            "fixtures.clauses.R1_2",
        ]
    );
}

/// `valid_system.ignite` through the slice-11 API (`docs/slice.md` 15.9):
/// the system's members with kind, property sources and outputs, the
/// assembly, and the effect sets of property expressions.
#[test]
fn valid_system_describes_its_system_and_assembly() {
    use ignite_types::{MemberDeclaration, PropertySource, QualName, Ty, field_default};

    let std = common::load_std(&fixture_dir().join("std"))
        .unwrap_or_else(|e| panic!("fixture standard library:\n{e}"));
    let path = fixture_dir().join("valid_system.ignite");
    let source = SourceFile::new(
        "valid_system.ignite",
        std::fs::read_to_string(&path).unwrap(),
    );
    let program = ignite_syntax::parse(&source).unwrap();
    let symbols = ignite_check::check_module(&program, &std.modules).unwrap();
    let typed = ignite_types::type_module(&program, &symbols, &std.typed)
        .unwrap_or_else(|d| panic!("type failed:\n{}", source.render_all(&d)));
    let local = |name: &str| QualName::new("fixtures.system", name);

    let [system] = typed.systems() else {
        panic!("one system, got {:?}", typed.systems());
    };
    assert_eq!(system.name, "Terminal");
    let decl = system.decl(&program).expect("system decl");
    assert_eq!(decl.name.name, "Terminal");
    let members: Vec<(&str, MemberDeclaration, usize)> = system
        .members
        .iter()
        .map(|m| (m.name.as_str(), m.declaration, m.index))
        .collect();
    assert_eq!(
        members,
        [
            ("ingress", MemberDeclaration::Resource, 0),
            ("sessions", MemberDeclaration::Resource, 1),
            ("server", MemberDeclaration::Component, 0),
            ("probe", MemberDeclaration::Component, 1),
            ("uptime", MemberDeclaration::Contract, 0),
            ("recovery", MemberDeclaration::Policy, 0),
        ]
    );
    for m in &system.members {
        assert_eq!(m.id, format!("Terminal.{}", m.name));
    }

    let ingress = system.member("ingress").unwrap();
    assert_eq!(ingress.kind, Ty::TcpListener);
    assert_eq!(
        ingress.properties,
        [
            ("address".to_string(), PropertySource::Set(0)),
            ("port".to_string(), PropertySource::Set(1)),
            ("exposure".to_string(), PropertySource::Set(2)),
            (
                "backlog".to_string(),
                PropertySource::Default(QualName::builtin("TcpListener.backlog"))
            ),
        ]
    );
    assert!(ingress.outputs.is_empty());
    let port = ingress.property(decl, "port").expect("port expression");
    assert_eq!(typed.type_of(port), &Ty::Int);
    assert_eq!(ingress.property(decl, "backlog"), None);

    let sessions = system.member("sessions").unwrap();
    assert_eq!(
        sessions.kind,
        Ty::Store(Box::new(Ty::Record(local("Window"))))
    );
    assert!(sessions.properties.is_empty() && sessions.outputs.is_empty());

    let server = system.member("server").unwrap();
    assert_eq!(server.kind, Ty::Record(local("TelnetServer")));
    assert_eq!(
        server.properties,
        [
            ("listener".to_string(), PropertySource::Set(0)),
            ("commands".to_string(), PropertySource::Set(1)),
            ("sessions".to_string(), PropertySource::Set(2)),
            (
                "executable".to_string(),
                PropertySource::Default(local("TelnetServer.executable"))
            ),
        ]
    );
    assert!(field_default(&program, "TelnetServer", "executable").is_some());

    let probe = system.member("probe").unwrap();
    assert_eq!(probe.outputs, [("attempts".to_string(), Ty::Int)]);
    assert_eq!(probe.properties.len(), 3);

    let uptime = system.member("uptime").unwrap();
    assert!(uptime.outputs.is_empty(), "a set signal is a property");
    let signal = uptime.property(decl, "signal").unwrap();
    assert_eq!(typed.type_of(signal), &Ty::Signal(Box::new(Ty::Int)));
    let window = uptime.property(decl, "window").unwrap();
    assert!(typed.effects(window).is_some_and(|e| e.is_empty()));
    assert!(typed.effects(signal).is_some_and(|e| e.is_empty()));
    assert!(typed.effects(port).is_some_and(|e| e.is_empty()));

    let recovery = system.member("recovery").unwrap();
    assert_eq!(
        recovery.properties[2],
        (
            "budget".to_string(),
            PropertySource::Default(local("Restart.budget"))
        )
    );

    let [assembly] = typed.assemblies() else {
        panic!("one assembly, got {:?}", typed.assemblies());
    };
    assert_eq!(assembly.name, "local");
    assert_eq!(assembly.kind, Ty::Record(local("Deployment")));
    assert_eq!(assembly.system.as_deref(), Some("Terminal"));
    assert_eq!(
        assembly.properties,
        [
            ("system".to_string(), PropertySource::Set(0)),
            ("bindings".to_string(), PropertySource::Set(1)),
        ]
    );
    let bindings = assembly.property(&program, "bindings").unwrap();
    assert!(typed.effects(bindings).is_some_and(|e| e.is_empty()));
    assert_eq!(
        assembly.decl(&program).map(|a| a.name.name.as_str()),
        Some("local")
    );
}

#[test]
fn every_type_code_has_a_fixture() {
    let names: Vec<String> = fixtures()
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    for code in [
        "e005", "e040", "e041", "e042", "e043", "e044", "e045", "e046", "e048", "e017", "e051",
        "e052", "e100", "e101", "e102", "e103", "e104", "e105", "e106", "e107", "e108", "e109",
        "e116", "e096", "e140", "e141", "valid",
    ] {
        assert!(
            names.iter().any(|n| n.starts_with(code)),
            "no fixture for {code}"
        );
    }
}

/// Adversary (slice 15 U1, `docs/slice.md` 19.5, 19.6, 19.13): the second
/// segment of `Spec.Id` is looked up in the spec's module, so a local
/// requirement of the same id does not capture a reference through an
/// imported spec; a spec declared through an alias of `Spec` is a `Spec`
/// constant; an empty `parties` list is no E040.
#[test]
fn adversary_reqrefs_resolve_through_their_spec_module() {
    let std = common::load_std(&fixture_dir().join("std"))
        .unwrap_or_else(|e| panic!("fixture standard library:\n{e}"));
    let text = "package fixtures.probe\n\
        import std.rfc.{Level, Requirement, Spec}\n\
        import std.rfc.rfc1.{RFC1}\n\
        type S = Spec\n\
        const RFC7: S = Spec { rfc = 7, title = \"t\", url = \"u\", digest = \"d\", requirements = [R2_1] }\n\
        const R2_1: Requirement = Requirement { rfc = 7, section = \"2\", ordinal = 1, level = Must, text = \"t\" }\n\
        protocol Echo conforms [RFC1, RFC7] scope { parties = [] } {\n\
          fn serve(stream: TcpStream, application: fn(Text) -> Text) -> Result<Unit, IoError> requires [stream.write] satisfies [RFC1.R2_1, RFC7.R2_1] {\n\
            return stream.write_all(b\"ok\")\n\
          }\n\
        }\n";
    let source = SourceFile::new("probe.ignite", text.to_string());
    let program = ignite_syntax::parse(&source).unwrap();
    let symbols = ignite_check::check_module(&program, &std.modules)
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let typed = ignite_types::type_module(&program, &symbols, &std.typed)
        .unwrap_or_else(|d| panic!("type failed:\n{}", source.render_all(&d)));
    let ignite_syntax::ast::Decl::Protocol(p) = program
        .decls
        .iter()
        .find(|d| matches!(d, ignite_syntax::ast::Decl::Protocol(_)))
        .unwrap()
    else {
        unreachable!()
    };
    let resolved: Vec<String> = p
        .conforms
        .iter()
        .map(|c| c.span)
        .chain(p.fns[0].satisfies.iter().map(|r| r.span))
        .map(|span| {
            typed
                .clause_constant(span)
                .map(|q| format!("{}.{}", q.module, q.name))
                .unwrap_or_else(|| "unresolved".to_string())
        })
        .collect();
    assert_eq!(
        resolved,
        [
            "std.rfc.rfc1.RFC1",
            "fixtures.probe.RFC7",
            "std.rfc.rfc1.R2_1",
            "fixtures.probe.R2_1",
        ]
    );
}
