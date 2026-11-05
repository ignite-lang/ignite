//! The claim model API (`docs/slice.md` 19.7, 19.13, row 18 of 19.0): a
//! program built from source through `ignite-syntax`, `ignite-check` and
//! `ignite-types` against the real `std/rfc.ignite`, read through
//! `Claims::of`.

use std::path::Path;

use ignite_eval::claims::Limit;
use ignite_eval::{Claims, Evaluator, Fuel, Level, LevelClass, Module, Modules, Row, Status};
use ignite_syntax::SourceFile;
use ignite_syntax::ast::Program;
use ignite_types::{TypedModules, TypedProgram};

const PROGRAM: &str = r#"package tests.claims

import std.rfc.{Level, Party, Source, Requirement, Spec}

const RFC7100: Spec = Spec {
  rfc = 7100
  title = "Limits for the claim model test"
  url = "https://example.invalid/rfc7100"
  digest = "sha256:7100"
  requirements = [R2_1, R2_2]
}

const R2_1: Requirement = Requirement {
  rfc = 7100
  section = "2"
  ordinal = 1
  level = Should
  text = "A server SHOULD answer within a second."
}

const R2_2: Requirement = Requirement {
  rfc = 7100
  section = "2"
  ordinal = 2
  level = Must
  minimum = Some(8000)
  text = "A server MUST accept a head of at least 8000 bytes."
}

type Limits {
  low: Int = 7999 satisfies [RFC7100.R2_2]
  high: ByteSize = 8 KiB satisfies [RFC7100.R2_2]
}

protocol Echo conforms [RFC7100] {
  fn serve(stream: TcpStream, application: fn(Text) -> Text) -> Result<Unit, IoError> requires [stream.write] {
    return stream.write_all(b"ok")
  }
}
"#;

/// One module, parsed, checked and typed.
struct Built {
    path: String,
    source: SourceFile,
    program: Program,
    types: TypedProgram,
}

fn build(
    path: &str,
    file: &str,
    text: String,
    symbols: &ignite_check::Modules,
    typed: &TypedModules,
) -> Built {
    let source = SourceFile::new(file, text);
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse:\n{}", source.render_all(&d)));
    let checked = ignite_check::check_module(&program, symbols)
        .unwrap_or_else(|d| panic!("check:\n{}", source.render_all(&d)));
    let types = ignite_types::type_module(&program, &checked, typed)
        .unwrap_or_else(|d| panic!("type:\n{}", source.render_all(&d)));
    Built {
        path: path.to_string(),
        source,
        program,
        types,
    }
}

/// `std.rfc` from the repository's `std/rfc.ignite`, then [`PROGRAM`].
fn modules() -> Vec<Built> {
    modules_of(PROGRAM)
}

/// `std.rfc`, then `program`, a module `tests.claims`.
fn modules_of(program: &str) -> Vec<Built> {
    let std = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../std/rfc.ignite");
    let rfc = build(
        "std.rfc",
        "std/rfc.ignite",
        std::fs::read_to_string(std).unwrap(),
        &ignite_check::Modules::new(),
        &TypedModules::new(),
    );
    let mut symbols = ignite_check::Modules::new();
    symbols.insert(
        "std.rfc".to_string(),
        ignite_check::check_module(&rfc.program, &ignite_check::Modules::new()).unwrap(),
    );
    let mut typed = TypedModules::new();
    typed.insert("std.rfc".to_string(), rfc.types.exports());
    let program = build(
        "tests.claims",
        "claims.ignite",
        program.to_string(),
        &symbols,
        &typed,
    );
    vec![rfc, program]
}

fn claims(built: &[Built]) -> Claims {
    let modules: Modules<'_> = built
        .iter()
        .map(|b| Module {
            path: &b.path,
            source: &b.source,
            program: &b.program,
            types: &b.types,
        })
        .collect();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    let program = *modules.get("tests.claims").unwrap();
    Claims::of(&mut evaluator, program).unwrap_or_else(|d| {
        panic!("diagnostics:\n{}", built[1].source.render_all(&d));
    })
}

fn row<'c>(claims: &'c Claims, id: &str) -> &'c Row {
    let [protocol] = claims.protocols.as_slice() else {
        panic!("one protocol: {claims:?}");
    };
    assert_eq!(protocol.protocol, "Echo");
    let [spec] = protocol.specs.as_slice() else {
        panic!("one spec: {protocol:?}");
    };
    assert_eq!(spec.constant, "RFC7100");
    assert_eq!(spec.rfc, 7100);
    spec.rows
        .iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("no row {id}: {spec:?}"))
}

#[test]
fn should_without_status_is_unclassified() {
    let built = modules();
    let claims = claims(&built);
    let row = row(&claims, "R2_1");
    assert_eq!(row.level, Level::Should);
    assert_eq!(row.level.class(), LevelClass::Should);
    assert_eq!(row.level.keyword(), "SHOULD");
    assert!(row.in_scope);
    assert_eq!(row.status, Status::None);
    assert!(row.tests.is_empty());
}

#[test]
fn limit_below_minimum_does_not_meet_it() {
    let built = modules();
    let claims = claims(&built);
    let row = row(&claims, "R2_2");
    assert_eq!(row.minimum, Some(8000));
    let Status::Satisfied(satisfiers) = &row.status else {
        panic!("satisfied: {row:?}");
    };
    let limits: Vec<(&str, Option<Limit>)> = satisfiers
        .iter()
        .map(|s| (s.declaration.as_str(), s.limit))
        .collect();
    assert_eq!(
        limits,
        [
            (
                "Limits.low",
                Some(Limit {
                    configured: Some(7999),
                    minimum: 8000,
                    meets: Some(false),
                })
            ),
            (
                "Limits.high",
                Some(Limit {
                    configured: Some(8192),
                    minimum: 8000,
                    meets: Some(true),
                })
            ),
        ]
    );
}

/// A specification that is no RFC (19.19): its `Spec` and requirements
/// carry the text id and no `rfc`; the claim is labelled by the id.
const TEXT_ID_PROGRAM: &str = r#"package tests.claims

import std.rfc.{Level, Party, Source, Requirement, Spec}

const LINE_ECHO_1: Spec = Spec {
  id = "line-echo-1"
  title = "A line echo that is no RFC"
  url = "https://example.invalid/line-echo-1"
  digest = "sha256:0001"
  requirements = [R2_1]
}

const R2_1: Requirement = Requirement {
  section = "2"
  ordinal = 1
  level = Must
  source = Prose
  text = "A server MUST echo every line."
}

protocol Echo conforms [LINE_ECHO_1] {
  fn serve(stream: TcpStream, application: fn(Text) -> Text) -> Result<Unit, IoError> requires [stream.write] satisfies [LINE_ECHO_1.R2_1] {
    return stream.write_all(b"ok")
  }
}
"#;

#[test]
fn a_spec_with_only_a_text_id_is_claimed_under_its_id() {
    let built = modules_of(TEXT_ID_PROGRAM);
    let claims = claims(&built);
    let [protocol] = claims.protocols.as_slice() else {
        panic!("one protocol: {claims:?}");
    };
    let [spec] = protocol.specs.as_slice() else {
        panic!("one spec: {protocol:?}");
    };
    assert_eq!(spec.constant, "LINE_ECHO_1");
    assert_eq!(spec.id, "line-echo-1");
    assert_eq!(spec.rfc, 0);
    assert_eq!(spec.label(), "line-echo-1");
    let [row] = spec.rows.as_slice() else {
        panic!("one row: {spec:?}");
    };
    assert_eq!(row.id, "R2_1");
    assert!(matches!(&row.status, Status::Satisfied(s) if s[0].declaration == "Echo.serve"));
}

#[test]
fn an_rfc_spec_is_labelled_by_its_number() {
    let built = modules();
    let claims = claims(&built);
    let spec = &claims.protocols[0].specs[0];
    assert_eq!(spec.id, "");
    assert_eq!(spec.label(), "rfc7100");
}
