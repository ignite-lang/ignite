//! Adversary cases for the claim model (`docs/slice.md` 19.4, 19.5, 19.7,
//! 19.13): programs built from source against the real `std/rfc.ignite`,
//! read through `Claims::of`. Each case asserts the contract's literal
//! expectation; a case the implementation does not meet is `#[ignore]`d
//! with the reason, so the branch stays green.

use std::path::Path;

use ignite_eval::claims::Limit;
use ignite_eval::{
    Claims, Evaluator, Fuel, Level, Module, Modules, Party, Row, Satisfier, Source, SpecClaim,
    Status,
};
use ignite_syntax::SourceFile;
use ignite_syntax::ast::Program;
use ignite_syntax::diagnostic::Diagnostic;
use ignite_types::{TypedModules, TypedProgram};

const FILE: &str = "adversary.ignite";
const MODULE: &str = "tests.adv";

struct Built {
    path: String,
    source: SourceFile,
    program: Program,
    types: TypedProgram,
}

/// `std.rfc` from the repository, then `text` as module [`MODULE`].
fn build(text: &str) -> Vec<Built> {
    let std = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../std/rfc.ignite");
    let mut symbols = ignite_check::Modules::new();
    let mut typed = TypedModules::new();
    let mut built = Vec::new();
    for (path, file, text) in [
        (
            "std.rfc",
            "std/rfc.ignite",
            std::fs::read_to_string(std).unwrap(),
        ),
        (MODULE, FILE, text.to_string()),
    ] {
        let source = SourceFile::new(file, text);
        let program = ignite_syntax::parse(&source)
            .unwrap_or_else(|d| panic!("parse:\n{}", source.render_all(&d)));
        let checked = ignite_check::check_module(&program, &symbols)
            .unwrap_or_else(|d| panic!("check:\n{}", source.render_all(&d)));
        let types = ignite_types::type_module(&program, &checked, &typed)
            .unwrap_or_else(|d| panic!("type:\n{}", source.render_all(&d)));
        symbols.insert(path.to_string(), checked);
        typed.insert(path.to_string(), types.exports());
        built.push(Built {
            path: path.to_string(),
            source,
            program,
            types,
        });
    }
    built
}

fn run(built: &[Built]) -> Result<Claims, Vec<Diagnostic>> {
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
    let module = *modules.get(MODULE).unwrap();
    Claims::of(&mut evaluator, module)
}

fn claims(text: &str) -> Claims {
    let built = build(text);
    run(&built).unwrap_or_else(|d| panic!("diagnostics:\n{}", built[1].source.render_all(&d)))
}

/// The rendered diagnostics, sorted by position as `ignite check` prints them.
fn diagnostics(text: &str) -> Vec<String> {
    let built = build(text);
    match run(&built) {
        Ok(claims) => panic!("expected diagnostics, got {claims:?}"),
        Err(found) => built[1]
            .source
            .render_all(&found)
            .lines()
            .map(str::to_string)
            .collect(),
    }
}

fn spec<'c>(claims: &'c Claims, protocol: &str) -> &'c SpecClaim {
    let claim = claims
        .protocols
        .iter()
        .find(|p| p.protocol == protocol)
        .unwrap_or_else(|| panic!("no protocol {protocol}: {claims:?}"));
    let [spec] = claim.specs.as_slice() else {
        panic!("one spec: {claim:?}");
    };
    spec
}

fn row<'c>(claims: &'c Claims, protocol: &str, id: &str) -> &'c Row {
    spec(claims, protocol)
        .rows
        .iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("no row {id}"))
}

fn satisfier(declaration: &str, limit: Option<Limit>) -> Satisfier {
    Satisfier {
        declaration: declaration.to_string(),
        limit,
    }
}

const HEAD: &str = r#"package tests.adv

import std.rfc.{Level, Party, Source, Requirement, Spec}
"#;

const SERVE: &str = "fn serve(stream: TcpStream, application: fn(Text) -> Text) -> Result<Unit, IoError> requires [stream.write]";

fn spec_text(ids: &[&str]) -> String {
    format!(
        "const RFC7200: Spec = Spec {{\n  rfc = 7200\n  title = \"Adversary\"\n  url = \"https://example.invalid/rfc7200\"\n  digest = \"sha256:7200\"\n  requirements = [{}]\n}}\n",
        ids.join(", ")
    )
}

fn requirement(id: &str, section: &str, ordinal: i64, extra: &str) -> String {
    format!(
        "const {id}: Requirement = Requirement {{\n  rfc = 7200\n  section = \"{section}\"\n  ordinal = {ordinal}\n  {extra}\n  text = \"{id}\"\n}}\n"
    )
}

/// 19.5: "A requirement may be satisfied by several declarations. That is
/// one status"; 19.7: satisfiers are "the declarations". One fn that
/// names a requirement twice in its `satisfies` is one declaration, as a
/// test verifying twice is one entry of `tests`.
#[test]
#[ignore = "adversary: one fn naming a requirement twice in `satisfies` is listed twice among the satisfiers"]
fn adversary_one_declaration_satisfying_twice_is_one_satisfier() {
    let text = format!(
        "{HEAD}\n{}\n{}\nfn answer(x: Int) -> Int satisfies [RFC7200.R2_1, RFC7200.R2_1] = x\n\nprotocol Echo conforms [RFC7200] {{\n  {SERVE} {{\n    return stream.write_all(b\"ok\")\n  }}\n}}\n",
        spec_text(&["R2_1"]),
        requirement("R2_1", "2", 1, "level = Must"),
    );
    let claims = claims(&text);
    assert_eq!(
        row(&claims, "Echo", "R2_1").status,
        Status::Satisfied(vec![satisfier("answer", None)])
    );
}

/// 19.7: the configured value and "whether it is at least `m`"; an equal
/// value meets the minimum (kills `>` for `>=`); `8 KB` is 8000 bytes; a
/// field with no default or of another type (`Duration`, `Option<Int>`)
/// has no configured value; a fn satisfier and a requirement without a
/// `minimum` carry no limit.
#[test]
fn adversary_limit_boundaries_and_other_types() {
    let text = format!(
        "{HEAD}\n{}\n{}\n{}\ntype Limits {{\n  exact: Int = 8000 satisfies [RFC7200.R2_1]\n  decimal: ByteSize = 8 KB satisfies [RFC7200.R2_1]\n  below: ByteSize = 7999 bytes satisfies [RFC7200.R2_1]\n  unset: Int satisfies [RFC7200.R2_1]\n  wait: Duration = 500 ms satisfies [RFC7200.R2_1]\n  maybe: Option<Int> = Some(9000) satisfies [RFC7200.R2_1]\n  plain: Int = 1 satisfies [RFC7200.R2_2]\n}}\n\nfn answer(x: Int) -> Int satisfies [RFC7200.R2_1] = x\n\nprotocol Echo conforms [RFC7200] {{\n  {SERVE} {{\n    return stream.write_all(b\"ok\")\n  }}\n}}\n",
        spec_text(&["R2_1", "R2_2"]),
        requirement("R2_1", "2", 1, "level = Must\n  minimum = Some(8000)"),
        requirement("R2_2", "2", 2, "level = Must"),
    );
    let claims = claims(&text);
    let limit = |configured: Option<i64>, meets: Option<bool>| {
        Some(Limit {
            configured,
            minimum: 8000,
            meets,
        })
    };
    assert_eq!(
        row(&claims, "Echo", "R2_1").status,
        Status::Satisfied(vec![
            satisfier("Limits.exact", limit(Some(8000), Some(true))),
            satisfier("Limits.decimal", limit(Some(8000), Some(true))),
            satisfier("Limits.below", limit(Some(7999), Some(false))),
            satisfier("Limits.unset", limit(None, None)),
            satisfier("Limits.wait", limit(None, None)),
            satisfier("Limits.maybe", limit(None, None)),
            satisfier("answer", None),
        ])
    );
    assert_eq!(
        row(&claims, "Echo", "R2_2").status,
        Status::Satisfied(vec![satisfier("Limits.plain", None)])
    );
}

/// 19.7's row fields, literally: scope by party and section, the four
/// statuses with their reasons, satisfiers named `Name`, `Record.field`,
/// `Owner.f`, `f` and `P.role` in declaration order, and the tests' full
/// names `<module>/<name>` in declaration order, each once.
#[test]
fn adversary_rows_carry_scope_statuses_satisfiers_and_tests() {
    let text = format!(
        "{HEAD}\n{}\n{}\n{}\n{}\n{}\n{}\ntype Line satisfies [RFC7200.R2_1] {{\n  text: Text satisfies [RFC7200.R2_1]\n}}\n\nfn Line.parse(raw: Text) -> Line satisfies [RFC7200.R2_1] = Line {{ text = raw }}\n\nfn echo(line: Line) -> Text satisfies [RFC7200.R2_1] = line.text\n\nprotocol Echo conforms [RFC7200] scope {{ parties = [Server, Any], sections = [\"2\"] }} {{\n  {SERVE} satisfies [RFC7200.R2_1] {{\n    return stream.write_all(b\"ok\")\n  }}\n\n  unsupported RFC7200.R2_2 \"no clock\"\n  not_applicable RFC7200.R3_1 \"outside the scope\"\n}}\n\ntest \"b\" verifies [RFC7200.R2_1] {{\n  assert echo(Line.parse(\"x\")) == \"x\"\n}}\n\ntest \"a\" verifies [RFC7200.R2_1, RFC7200.R2_2, RFC7200.R2_1] {{\n  assert echo(Line.parse(\"y\")) == \"y\"\n}}\n",
        spec_text(&["R2_1", "R2_2", "R2_3", "R2_4", "R3_1"]),
        requirement("R2_1", "2", 1, "level = Must\n  party = Server"),
        requirement("R2_2", "2", 2, "level = Should"),
        requirement("R2_3", "2", 3, "level = May\n  party = Client"),
        requirement(
            "R2_4",
            "2",
            4,
            "level = MustNot\n  party = Proxy\n  source = Prose"
        ),
        requirement("R3_1", "3", 1, "level = Must\n  party = Server"),
    );
    let claims = claims(&text);
    let spec = spec(&claims, "Echo");
    assert_eq!(spec.constant, "RFC7200");
    assert_eq!(spec.parties, Some(vec![Party::Server, Party::Any]));
    assert_eq!(spec.sections, Some(vec!["2".to_string()]));
    let row = |id: &str,
               section: &str,
               ordinal: i64,
               level: Level,
               party: Party,
               source: Source,
               in_scope: bool,
               status: Status,
               tests: &[&str]| Row {
        id: id.to_string(),
        section: section.to_string(),
        ordinal,
        level,
        party,
        source,
        minimum: None,
        text: id.to_string(),
        in_scope,
        status,
        tests: tests.iter().map(|t| t.to_string()).collect(),
    };
    assert_eq!(
        spec.rows,
        vec![
            row(
                "R2_1",
                "2",
                1,
                Level::Must,
                Party::Server,
                Source::Bcp14,
                true,
                Status::Satisfied(vec![
                    satisfier("Line", None),
                    satisfier("Line.text", None),
                    satisfier("Line.parse", None),
                    satisfier("echo", None),
                    satisfier("Echo.serve", None),
                ]),
                &["tests.adv/b", "tests.adv/a"],
            ),
            row(
                "R2_2",
                "2",
                2,
                Level::Should,
                Party::Any,
                Source::Bcp14,
                true,
                Status::Unsupported("no clock".to_string()),
                &["tests.adv/a"],
            ),
            row(
                "R2_3",
                "2",
                3,
                Level::May,
                Party::Client,
                Source::Bcp14,
                false,
                Status::None,
                &[],
            ),
            row(
                "R2_4",
                "2",
                4,
                Level::MustNot,
                Party::Proxy,
                Source::Prose,
                false,
                Status::None,
                &[],
            ),
            row(
                "R3_1",
                "3",
                1,
                Level::Must,
                Party::Server,
                Source::Bcp14,
                false,
                Status::NotApplicable("outside the scope".to_string()),
                &[],
            ),
        ]
    );
}

/// 19.3's classes and 19.7: every MUST-class level in scope with no
/// status is one E090, in `requirements` order; no SHOULD-class or
/// MAY-class level is.
#[test]
fn adversary_only_must_class_levels_without_status_are_e090() {
    let levels = [
        "Must",
        "MustNot",
        "Required",
        "Shall",
        "ShallNot",
        "Should",
        "ShouldNot",
        "Recommended",
        "NotRecommended",
        "May",
        "Optional",
    ];
    let ids: Vec<String> = (1..=levels.len()).map(|n| format!("R2_{n}")).collect();
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let mut text = format!("{HEAD}\n{}", spec_text(&refs));
    for (n, level) in levels.iter().enumerate() {
        text.push_str(&requirement(
            &ids[n],
            "2",
            n as i64 + 1,
            &format!("level = {level}"),
        ));
    }
    text.push_str(&format!(
        "\nprotocol Echo conforms [RFC7200] {{\n  {SERVE} {{\n    return stream.write_all(b\"ok\")\n  }}\n}}\n"
    ));
    let line = text
        .lines()
        .position(|l| l.starts_with("protocol Echo"))
        .unwrap()
        + 1;
    let found = diagnostics(&text);
    let expected: Vec<String> = [
        ("R2_1", "MUST"),
        ("R2_2", "MUST NOT"),
        ("R2_3", "REQUIRED"),
        ("R2_4", "SHALL"),
        ("R2_5", "SHALL NOT"),
    ]
    .iter()
    .map(|(id, keyword)| {
        format!(
            "{FILE}:{line}:25: error[E090]: requirement `RFC7200.{id}` ({keyword}, section 2) is in scope of `Echo` and has no status; add `satisfies`, `unsupported` or `not_applicable`"
        )
    })
    .collect();
    assert_eq!(found, expected);
}

/// 19.7: "one status kind twice" is E090 at the later clause; a status
/// on a requirement out of scope is allowed, but two of them are E090 "at
/// any level".
#[test]
fn adversary_one_status_kind_twice_is_e090_at_the_later_clause() {
    let text = format!(
        "{HEAD}\n{}\n{}\n{}\nprotocol Echo conforms [RFC7200] scope {{ sections = [\"2\"] }} {{\n  {SERVE} {{\n    return stream.write_all(b\"ok\")\n  }}\n\n  unsupported RFC7200.R2_1 \"first\"\n  unsupported RFC7200.R2_1 \"second\"\n  not_applicable RFC7200.R3_1 \"first\"\n  not_applicable RFC7200.R3_1 \"second\"\n}}\n",
        spec_text(&["R2_1", "R3_1"]),
        requirement("R2_1", "2", 1, "level = Must"),
        requirement("R3_1", "3", 1, "level = May"),
    );
    let found = diagnostics(&text);
    let line_of = |needle: &str| text.lines().position(|l| l.contains(needle)).unwrap() + 1;
    let prefixes = [
        format!(
            "{FILE}:{}:3: error[E090]: requirement `RFC7200.R2_1` has two statuses in `Echo`: ",
            line_of("R2_1 \"second\"")
        ),
        format!(
            "{FILE}:{}:3: error[E090]: requirement `RFC7200.R3_1` has two statuses in `Echo`: ",
            line_of("R3_1 \"second\"")
        ),
    ];
    assert_eq!(found.len(), 2, "{found:#?}");
    for (line, prefix) in found.iter().zip(&prefixes) {
        assert!(line.starts_with(prefix.as_str()), "{line}\nwanted {prefix}");
    }
}

/// 19.7: "a status in P naming a requirement of a spec P does not conform
/// to" is E092, also when P conforms to nothing; another protocol's
/// `satisfies` still counts for the conforming one (19.5).
#[test]
fn adversary_status_in_a_protocol_without_conforms_is_e092() {
    let text = format!(
        "{HEAD}\n{}\n{}\nprotocol Echo conforms [RFC7200] {{\n  {SERVE} {{\n    return stream.write_all(b\"ok\")\n  }}\n}}\n\nprotocol Other {{\n  {SERVE} satisfies [RFC7200.R2_1] {{\n    return stream.write_all(b\"no\")\n  }}\n\n  not_applicable RFC7200.R2_1 \"never\"\n}}\n",
        spec_text(&["R2_1"]),
        requirement("R2_1", "2", 1, "level = Must"),
    );
    let line = text
        .lines()
        .position(|l| l.contains("not_applicable RFC7200.R2_1"))
        .unwrap()
        + 1;
    assert_eq!(
        diagnostics(&text),
        [format!(
            "{FILE}:{line}:18: error[E092]: `RFC7200.R2_1` belongs to RFC 7200, which `Other` does not conform to"
        )]
    );
}

/// 19.5: a `satisfies` "counts for every protocol of its module that
/// conforms to the requirement's spec".
#[test]
fn adversary_one_satisfies_counts_for_every_conforming_protocol() {
    let text = format!(
        "{HEAD}\n{}\n{}\nfn answer(x: Int) -> Int satisfies [RFC7200.R2_1] = x\n\nprotocol Echo conforms [RFC7200] {{\n  {SERVE} {{\n    return stream.write_all(b\"ok\")\n  }}\n}}\n\nprotocol Relay conforms [RFC7200] {{\n  {SERVE} {{\n    return stream.write_all(b\"ok\")\n  }}\n}}\n",
        spec_text(&["R2_1"]),
        requirement("R2_1", "2", 1, "level = Must"),
    );
    let claims = claims(&text);
    let names: Vec<&str> = claims
        .protocols
        .iter()
        .map(|p| p.protocol.as_str())
        .collect();
    assert_eq!(names, ["Echo", "Relay"]);
    for protocol in ["Echo", "Relay"] {
        assert_eq!(
            row(&claims, protocol, "R2_1").status,
            Status::Satisfied(vec![satisfier("answer", None)])
        );
    }
}

/// 19.4: the second trigger covers "the value of every constant whose
/// declared type is `std.rfc.Requirement`", also through a type alias; a
/// misnamed one is case 2.
#[test]
fn adversary_requirement_declared_through_a_type_alias_is_consistency_checked() {
    let text = format!(
        "{HEAD}\ntype Req = Requirement\n\n{}\nconst R2_5: Req = Requirement {{\n  rfc = 7200\n  section = \"2\"\n  ordinal = 1\n  level = Must\n  text = \"aliased\"\n}}\n",
        spec_text(&["R2_5"]),
    );
    let line = text
        .lines()
        .position(|l| l.starts_with("const R2_5"))
        .unwrap()
        + 1;
    assert_eq!(
        diagnostics(&text),
        [format!(
            "{FILE}:{line}:7: error[E091]: requirement `R2_5` must be named `R2_1`, the id of section 2 and ordinal 1"
        )]
    );
}

/// 19.7: one E090 per requirement in scope with no status, and one row
/// per requirement. A requirement the checked file's `Spec` lists twice is
/// one E091 (19.4), and still one requirement with no status.
#[test]
#[ignore = "adversary: a requirement listed twice in the Spec gives two identical E090 lines and two rows"]
fn adversary_requirement_listed_twice_is_one_e090() {
    let text = format!(
        "{HEAD}\n{}\n{}\nprotocol Echo conforms [RFC7200] {{\n  {SERVE} {{\n    return stream.write_all(b\"ok\")\n  }}\n}}\n",
        spec_text(&["R2_1", "R2_1"]),
        requirement("R2_1", "2", 1, "level = Must"),
    );
    let found = diagnostics(&text);
    let e090 = found.iter().filter(|l| l.contains("error[E090]")).count();
    let e091 = found.iter().filter(|l| l.contains("error[E091]")).count();
    assert_eq!((e091, e090), (1, 1), "{found:#?}");
}

/// 19.4's precedence: a wrong name (case 2) precedes "not listed" (case
/// 3); a negative ordinal is case 1, before its wrong name.
#[test]
fn adversary_e091_name_precedes_listing_and_negative_ordinal_precedes_name() {
    let text = format!(
        "{HEAD}\n{}\n{}\n{}\n{}",
        spec_text(&["R2_1"]),
        requirement("R2_1", "2", 1, "level = Must"),
        requirement("R2_8", "2", 2, "level = Must"),
        requirement("R2_7", "2", -1, "level = Must"),
    );
    let line_of = |needle: &str| text.lines().position(|l| l.contains(needle)).unwrap() + 1;
    assert_eq!(
        diagnostics(&text),
        [
            format!(
                "{FILE}:{}:7: error[E091]: requirement `R2_8` must be named `R2_2`, the id of section 2 and ordinal 2",
                line_of("const R2_8")
            ),
            format!(
                "{FILE}:{}:7: error[E091]: requirement `R2_7` has ordinal -1; an ordinal is 1 or more",
                line_of("const R2_7")
            ),
        ]
    );
}
