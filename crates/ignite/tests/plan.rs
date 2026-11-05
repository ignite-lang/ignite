//! `ignite plan` and `ignite explain` (`docs/slice.md` sections 15.4 to
//! 15.8, acceptance 15.0 items 4 to 7 and 11): the model of the design
//! file is byte-identical across processes and working directories and
//! equals its golden; every value in it is encoded as 15.4 says and no
//! property is source text; the assembly `local` resolves each of the five
//! members once; the explain goldens of 15.8; the fixtures of E110 to E114
//! and E119.
//!
//! `IGNITE_BLESS=1 cargo test -p ignite --test main plan::` rewrites the
//! goldens and the fixtures' `.expected` files from the current output.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

const DESIGN: &str = "examples/terminal/terminal.design.ignite";

/// `ignite args` run from `dir`.
fn ignite_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn ignite(args: &[&str]) -> Output {
    ignite_in(&workspace(), args)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("UTF-8 output")
}

/// The stdout of a successful run.
fn success(output: &Output) -> String {
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(output.stderr.is_empty(), "stderr: {}", text(&output.stderr));
    text(&output.stdout)
}

/// `actual` must equal the golden at `path`; `IGNITE_BLESS=1` rewrites it.
fn compare(path: &Path, actual: &str) {
    if std::env::var_os("IGNITE_BLESS").is_some() {
        std::fs::write(path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e} (IGNITE_BLESS=1 to create)", path.display()));
    assert!(
        expected == actual,
        "{} differs from the output (IGNITE_BLESS=1 to accept):\n{actual}",
        path.display()
    );
}

fn design_model() -> String {
    success(&ignite(&["plan", DESIGN]))
}

#[test]
fn design_model_is_byte_identical_across_processes_and_directories() {
    let first = design_model();
    let second = success(&ignite_in(
        &workspace().join("examples"),
        &["plan", "terminal/terminal.design.ignite"],
    ));
    assert_eq!(first, second);
    compare(&golden_dir().join("terminal.design.model.json"), &first);
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("plan-out.json");
    let written = ignite(&["plan", DESIGN, "--out", out.to_str().unwrap()]);
    assert_eq!(success(&written), "");
    assert_eq!(std::fs::read_to_string(&out).unwrap(), first);
}

#[test]
fn design_model_has_no_expression_text() {
    let model = design_model();
    for forbidden in [
        "exponential(",
        "rolling(",
        "calendar_month(",
        "event(",
        "module(",
        "localSocket(",
        "nativeProcess(",
        "workstationExecutable(",
        "scheduledProbeHost(",
        "endpoint(",
        " bytes",
        " KiB",
        " minutes",
        " seconds",
        " ms",
        " days",
        " hour",
        "%",
        "/home/",
    ] {
        assert!(!model.contains(forbidden), "{forbidden:?} in\n{model}");
    }
}

#[test]
fn design_model_is_canonical_and_every_value_is_encoded_as_15_4_says() {
    let model = design_model();
    assert!(model.ends_with('\n') && !model.ends_with("\n\n"));
    let json = Json::parse(&model);
    let top = json.object();
    assert_eq!(
        keys(top),
        [
            "assembly",
            "components",
            "contracts",
            "edges",
            "formatVersion",
            "machines",
            "observations",
            "policies",
            "resources",
            "sourceMap",
            "systemId"
        ]
    );
    assert_eq!(top["formatVersion"], Json::Str("ignite-model/0.1".into()));
    assert_eq!(top["systemId"], Json::Str("Terminal".into()));
    let mut ids = Vec::new();
    for list in ["components", "contracts", "policies", "resources"] {
        let declaration = &list[..list.len() - 1];
        let mut previous = String::new();
        for member in top[list].array() {
            let member = member.object();
            assert_eq!(
                keys(member),
                [
                    "declaration",
                    "defaults",
                    "executable",
                    "id",
                    "kind",
                    "outputs",
                    "properties"
                ]
            );
            assert_eq!(member["declaration"], Json::Str(declaration.into()));
            let id = member["id"].string();
            assert!(id > previous.as_str(), "{list} not sorted by id at {id}");
            previous = id.to_string();
            ids.push(id.to_string());
            for value in member["properties"].object().values() {
                check_value(value);
            }
            for default in member["defaults"].array() {
                assert!(member["properties"].object().contains_key(default.string()));
            }
        }
    }
    assert_eq!(
        ids,
        [
            "Terminal.client",
            "Terminal.console",
            "Terminal.promptProbe",
            "Terminal.server",
            "Terminal.customerAccess",
            "Terminal.promptFast",
            "Terminal.reachable",
            "Terminal.ingress",
        ]
    );
    for (key, position) in top["sourceMap"].object() {
        let position = position.string();
        let mut parts = position.rsplitn(3, ':');
        let column = parts.next().unwrap();
        let line = parts.next().unwrap();
        let file = parts.next().unwrap();
        assert!(
            line.parse::<usize>().is_ok() && column.parse::<usize>().is_ok(),
            "{key}: {position}"
        );
        assert!(
            file == "terminal.design.ignite" || file.starts_with("std/"),
            "{key}: {position}"
        );
    }
    let assembly = top["assembly"].object();
    assert_eq!(
        keys(assembly),
        [
            "defaults",
            "id",
            "kind",
            "properties",
            "resolution",
            "system"
        ]
    );
    for value in assembly["properties"].object().values() {
        check_value(value);
    }
    for entry in assembly["resolution"].array() {
        let entry = entry.object();
        assert_eq!(keys(entry), ["binding", "member", "via"]);
        check_value(&entry["binding"]);
    }
    let mut previous = ("", "", "");
    for edge in top["edges"].array() {
        let edge = edge.object();
        assert_eq!(keys(edge), ["from", "kind", "to"]);
        let kind = edge["kind"].string();
        assert!(
            [
                "uses",
                "references",
                "observes",
                "hosts",
                "provisioning",
                "readiness",
                "recovery"
            ]
            .contains(&kind)
        );
        let current = (edge["from"].string(), kind, edge["to"].string());
        assert!(current > previous, "edges not sorted at {current:?}");
        previous = current;
    }
}

#[test]
fn design_model_resolves_each_of_the_five_members_once() {
    let json = Json::parse(&design_model());
    let assembly = json.object()["assembly"].object();
    assert_eq!(assembly["id"], Json::Str("local".into()));
    assert_eq!(assembly["kind"], Json::Str("std.deploy.Deployment".into()));
    let members: Vec<&str> = assembly["resolution"]
        .array()
        .iter()
        .map(|entry| entry.object()["member"].string())
        .collect();
    assert_eq!(
        members,
        [
            "Terminal.client",
            "Terminal.console",
            "Terminal.ingress",
            "Terminal.promptProbe",
            "Terminal.server"
        ]
    );
    let console = assembly["resolution"].array()[1].object();
    assert_eq!(console["via"], Json::Str("Terminal.server".into()));
    let edges = json.object()["edges"].array();
    assert!(edges.iter().any(|edge| {
        let edge = edge.object();
        edge["from"].string() == "Terminal.server"
            && edge["kind"].string() == "hosts"
            && edge["to"].string() == "Terminal.console"
    }));
}

#[test]
fn terminal_example_model_matches_golden() {
    let model = success(&ignite(&["plan", "examples/terminal/terminal.ignite"]));
    assert!(model.contains("\"assembly\": null"), "{model}");
    compare(&golden_dir().join("terminal.model.json"), &model);
}

#[test]
fn todo_example_model_matches_golden() {
    let model = success(&ignite(&["plan", "examples/todo/todo.ignite"]));
    assert!(model.contains("\"assembly\": null"), "{model}");
    compare(&golden_dir().join("todo.model.json"), &model);
}

/// `docs/slice.md` 21.0 row 25: the four capability resources in the model,
/// `Launcher`'s `maximum_children` as a default.
#[test]
fn studio_model_matches_golden() {
    let fixture = fixtures_dir().join("slice17/studio.ignite");
    let model = success(&ignite(&["plan", fixture.to_str().unwrap()]));
    for resource in [
        r#""id": "Studio.files""#,
        r#""kind": "Directory""#,
        r#""id": "Studio.tools""#,
        r#""maximum_children": 16"#,
        r#""id": "Studio.entropy""#,
        r#""id": "Studio.tool""#,
        r#""flag": "--tool""#,
    ] {
        assert!(model.contains(resource), "{resource} missing:\n{model}");
    }
    compare(&golden_dir().join("studio.model.json"), &model);
}

/// Slice 17: only a `TcpListener` provisions the executable that binds it
/// (16.14). The studio's capability resources, among them `Studio.entropy`,
/// which sorts before its listener `Studio.ingress`, are created by the
/// process but provision nothing, so a reader of the first provisioning
/// edge finds the listener.
#[test]
fn only_a_listener_provisions_its_executable() {
    let fixture = fixtures_dir().join("slice17/studio_defaults.ignite");
    let json = Json::parse(&success(&ignite(&["plan", fixture.to_str().unwrap()])));
    let provisioning: Vec<(&str, &str)> = json.object()["edges"]
        .array()
        .iter()
        .map(Json::object)
        .filter(|edge| edge["kind"].string() == "provisioning")
        .map(|edge| (edge["from"].string(), edge["to"].string()))
        .collect();
    assert_eq!(provisioning, [("Studio.ingress", "Studio.server")]);
}

/// `docs/slice.md` 15.6: a capability resource needs no binding. Under the
/// standard `Deployment`, which binds only the studio's listener and
/// server, `ignite check` is silent and the resolution hosts each of the
/// four capabilities via `Studio.server`, with the server's binding.
#[test]
fn capability_resources_are_hosted_by_their_members_process() {
    let fixture = fixtures_dir().join("slice17/studio_deployment.ignite");
    let fixture = fixture.to_str().unwrap();
    assert_eq!(success(&ignite(&["check", fixture])), "");
    let json = Json::parse(&success(&ignite(&["plan", fixture])));
    let resolution: Vec<(&str, &str, &str)> = json.object()["assembly"].object()["resolution"]
        .array()
        .iter()
        .map(Json::object)
        .map(|entry| {
            (
                entry["member"].string(),
                entry["via"].string(),
                entry["binding"].object()["variant"].string(),
            )
        })
        .collect();
    assert_eq!(
        resolution,
        [
            ("Studio.entropy", "Studio.server", "NativeProcess"),
            ("Studio.files", "Studio.server", "NativeProcess"),
            ("Studio.ingress", "Studio.ingress", "LocalSocket"),
            ("Studio.server", "Studio.server", "NativeProcess"),
            ("Studio.tool", "Studio.server", "NativeProcess"),
            ("Studio.tools", "Studio.server", "NativeProcess"),
        ]
    );
    let hosted: Vec<&str> = json.object()["edges"]
        .array()
        .iter()
        .map(Json::object)
        .filter(|edge| edge["kind"].string() == "hosts")
        .map(|edge| {
            assert_eq!(edge["from"].string(), "Studio.server");
            edge["to"].string()
        })
        .collect();
    assert_eq!(
        hosted,
        [
            "Studio.entropy",
            "Studio.files",
            "Studio.tool",
            "Studio.tools"
        ]
    );
}

#[test]
fn explain_matches_the_goldens_of_15_8() {
    for (path, golden) in [
        (
            "Terminal.reachable",
            "terminal.design.explain.Terminal.reachable.txt",
        ),
        (
            "Terminal.server.recovery",
            "terminal.design.explain.Terminal.server.recovery.txt",
        ),
        ("local", "terminal.design.explain.local.txt"),
        ("Terminal", "terminal.design.explain.Terminal.txt"),
    ] {
        let explanation = success(&ignite(&["explain", DESIGN, path]));
        compare(&golden_dir().join(golden), &explanation);
    }
    let nested = success(&ignite(&[
        "explain",
        DESIGN,
        "Terminal.server.sessions.idle_timeout",
    ]));
    assert!(nested.contains("  value = 15 minutes\n"), "{nested}");
}

#[test]
fn explain_unknown_path_is_e119() {
    for path in [
        "Terminal.nothing",
        "Nothing",
        "Terminal.server.nothing",
        "remote",
    ] {
        let output = ignite(&["explain", DESIGN, path]);
        assert_eq!(output.status.code(), Some(1), "{path}");
        assert!(output.stdout.is_empty());
        let stderr = text(&output.stderr);
        assert!(
            stderr.contains("terminal.design.ignite:1:1: error[E119]: `") && stderr.contains(path),
            "{path}: {stderr}"
        );
    }
}

/// Every `tests/fixtures/e11*.ignite`, `e12*` and `e090` to `e092` (the
/// claim model, `docs/slice.md` 19.12) run through `ignite check` (or the
/// subcommand its first line names, `// ignite plan` or `// ignite explain
/// <path>`, the file name inserted after the subcommand) from the fixtures
/// directory; stderr must equal the `.expected` file beside it.
#[test]
fn fixtures_match_expected() {
    let mut fixtures: Vec<PathBuf> = std::fs::read_dir(fixtures_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            (name.starts_with("e11")
                || name.starts_with("e12")
                || ["e090", "e091", "e092"].contains(&name.get(..4).unwrap_or("")))
                && name.ends_with(".ignite")
                && !["e125", "e126", "e127", "e128"].contains(&&name[..4])
        })
        .collect();
    fixtures.sort();
    assert!(fixtures.len() >= 12, "{fixtures:?}");
    for fixture in fixtures {
        let name = fixture.file_name().unwrap().to_str().unwrap();
        let source = std::fs::read_to_string(&fixture).unwrap();
        let words: Vec<&str> = source
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("// ignite "))
            .unwrap_or("check")
            .split_whitespace()
            .collect();
        let mut args = vec![words[0], name];
        args.extend_from_slice(&words[1..]);
        let output = ignite_in(&fixtures_dir(), &args);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{name}: {}",
            text(&output.stderr)
        );
        assert!(output.stdout.is_empty(), "{name}: {}", text(&output.stdout));
        let code = &name[..4].to_uppercase();
        let stderr = text(&output.stderr);
        assert!(
            stderr.contains(&format!("error[{code}]")),
            "{name}: {stderr}"
        );
        compare(&fixture.with_extension("expected"), &stderr);
    }
}

/// The E092 fixtures run against `fixtures/claims_std/`, whose vocabulary
/// must stay the real one (`docs/slice.md` 19.12, row 40 of 19.0).
#[test]
fn claims_std_vocabulary_is_std_rfc() {
    let copy = std::fs::read(fixtures_dir().join("claims_std/rfc.ignite")).unwrap();
    let real = std::fs::read(workspace().join("std/rfc.ignite")).unwrap();
    assert!(
        copy == real,
        "crates/ignite/tests/fixtures/claims_std/rfc.ignite differs from std/rfc.ignite"
    );
}

/// The protocol of `e090_missing_status` with a status on every
/// requirement checks silently (row 41 of 19.0).
#[test]
fn complete_claim_fixture_is_silent() {
    let output = ignite_in(&fixtures_dir(), &["check", "claim_ok_complete.ignite"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
}

#[test]
fn e114_covers_an_unknown_assembly_name_and_check_passes() {
    let fixture = "e114_assembly_selection.ignite";
    let output = ignite_in(&fixtures_dir(), &["plan", fixture, "--assembly", "nowhere"]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains(
            "error[E114]: no assembly named `nowhere`: the program declares hosted, workstation"
        ),
        "{stderr}"
    );
    let output = ignite_in(&fixtures_dir(), &["check", fixture]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let selected = success(&ignite_in(
        &fixtures_dir(),
        &["plan", fixture, "--assembly", "workstation"],
    ));
    assert!(selected.contains("\"id\": \"workstation\""), "{selected}");
    assert!(
        selected.contains("\"variant\": \"WorkstationExecutable\""),
        "{selected}"
    );
    let explained = success(&ignite_in(
        &fixtures_dir(),
        &["explain", fixture, "hosted", "--assembly", "hosted"],
    ));
    assert!(
        explained.starts_with(
            "hosted: assembly std.deploy.Deployment of S (e114_assembly_selection.ignite:36:1)\n"
        ),
        "{explained}"
    );
    assert!(
        explained.contains("  S.watch = ScheduledProbeHost, executable watch\n"),
        "{explained}"
    );
}

#[test]
fn exit_statuses() {
    let output = ignite(&["plan", "examples/terminal/does-not-exist.ignite"]);
    assert_eq!(output.status.code(), Some(3));
    let output = ignite(&["plan"]);
    assert_eq!(output.status.code(), Some(2));
    let output = ignite(&["explain", DESIGN]);
    assert_eq!(output.status.code(), Some(2));
    let output = ignite(&["plan", DESIGN, "--fuel", "1"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).contains("error[E115]"),
        "{}",
        text(&output.stderr)
    );
}

fn keys(object: &BTreeMap<String, Json>) -> Vec<&str> {
    object.keys().map(String::as_str).collect()
}

/// One value of the model against the table of 15.4.
fn check_value(value: &Json) {
    let object = match value {
        Json::Null | Json::Bool(_) | Json::Int(_) | Json::Str(_) => return,
        Json::Array(items) => return items.iter().for_each(check_value),
        Json::Object(object) => object,
    };
    let ty = object
        .get("type")
        .unwrap_or_else(|| panic!("untagged object {value:?}"))
        .string();
    let keys = keys(object);
    match ty {
        "Byte" => {
            assert_eq!(keys, ["type", "value"]);
            assert!(matches!(object["value"], Json::Int(0..=255)));
        }
        "Tuple" => {
            assert_eq!(keys, ["items", "type"]);
            object["items"].array().iter().for_each(check_value);
        }
        "Map" => {
            assert_eq!(keys, ["entries", "type"]);
            for entry in object["entries"].array() {
                let pair = entry.array();
                assert_eq!(pair.len(), 2);
                pair.iter().for_each(check_value);
            }
        }
        "Duration" | "ByteSize" | "Ratio" => {
            let unit = match ty {
                "Duration" => "ns",
                "ByteSize" => "bytes",
                _ => "ppm",
            };
            assert_eq!(keys, [unit, "type"]);
            assert!(matches!(object[unit], Json::Int(_)));
        }
        "Ref" | "Signal" | "SystemRef" | "Protocol" => {
            assert_eq!(keys, ["ref", "type"]);
            assert!(!object["ref"].string().is_empty());
        }
        "Module" => {
            assert_eq!(keys, ["fns", "module", "type"]);
            for fn_name in object["fns"].object().values() {
                assert!(fn_name.string().contains('.'));
            }
        }
        "Fn" => {
            assert_eq!(keys, ["fn", "type"]);
        }
        _ if object.contains_key("variant") => {
            assert!(
                keys == ["type", "variant"] || keys == ["fields", "type", "variant"],
                "{keys:?}"
            );
            if let Some(fields) = object.get("fields") {
                let fields = fields.array();
                assert!(!fields.is_empty());
                fields.iter().for_each(check_value);
            }
        }
        _ => {
            assert_eq!(keys, ["fields", "type"], "{ty}");
            object["fields"].object().values().for_each(check_value);
        }
    }
}

/// A JSON value as the test reads the model; numbers are integers only.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

impl Json {
    fn parse(text: &str) -> Json {
        let mut parser = Parser {
            bytes: text.as_bytes(),
            at: 0,
        };
        let value = parser.value();
        parser.space();
        assert_eq!(parser.at, text.len(), "trailing text");
        value
    }

    fn object(&self) -> &BTreeMap<String, Json> {
        match self {
            Json::Object(object) => object,
            other => panic!("not an object: {other:?}"),
        }
    }

    fn array(&self) -> &[Json] {
        match self {
            Json::Array(items) => items,
            other => panic!("not an array: {other:?}"),
        }
    }

    fn string(&self) -> &str {
        match self {
            Json::Str(text) => text,
            other => panic!("not a string: {other:?}"),
        }
    }
}

struct Parser<'t> {
    bytes: &'t [u8],
    at: usize,
}

impl Parser<'_> {
    fn space(&mut self) {
        while self.at < self.bytes.len() && self.bytes[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
    }

    fn expect(&mut self, literal: &str) {
        assert!(
            self.bytes[self.at..].starts_with(literal.as_bytes()),
            "expected {literal} at {}",
            self.at
        );
        self.at += literal.len();
    }

    fn value(&mut self) -> Json {
        self.space();
        match self.bytes.get(self.at) {
            Some(b'n') => {
                self.expect("null");
                Json::Null
            }
            Some(b't') => {
                self.expect("true");
                Json::Bool(true)
            }
            Some(b'f') => {
                self.expect("false");
                Json::Bool(false)
            }
            Some(b'"') => Json::Str(self.string()),
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                self.space();
                if self.bytes[self.at] == b']' {
                    self.at += 1;
                    return Json::Array(items);
                }
                loop {
                    items.push(self.value());
                    self.space();
                    match self.bytes[self.at] {
                        b',' => self.at += 1,
                        b']' => {
                            self.at += 1;
                            return Json::Array(items);
                        }
                        other => panic!("unexpected {:?} in array", other as char),
                    }
                }
            }
            Some(b'{') => {
                self.at += 1;
                let mut entries = BTreeMap::new();
                self.space();
                if self.bytes[self.at] == b'}' {
                    self.at += 1;
                    return Json::Object(entries);
                }
                loop {
                    self.space();
                    let key = self.string();
                    self.space();
                    self.expect(":");
                    let value = self.value();
                    assert!(entries.insert(key, value).is_none(), "duplicate key");
                    self.space();
                    match self.bytes[self.at] {
                        b',' => self.at += 1,
                        b'}' => {
                            self.at += 1;
                            return Json::Object(entries);
                        }
                        other => panic!("unexpected {:?} in object", other as char),
                    }
                }
            }
            Some(b'-' | b'0'..=b'9') => {
                let start = self.at;
                self.at += 1;
                while self.at < self.bytes.len() && self.bytes[self.at].is_ascii_digit() {
                    self.at += 1;
                }
                let digits = std::str::from_utf8(&self.bytes[start..self.at]).unwrap();
                assert!(
                    !matches!(self.bytes.get(self.at), Some(b'.' | b'e' | b'E')),
                    "non-integer number at {start}"
                );
                Json::Int(digits.parse().unwrap())
            }
            other => panic!("unexpected {other:?} at {}", self.at),
        }
    }

    fn string(&mut self) -> String {
        self.expect("\"");
        let mut out = String::new();
        loop {
            let rest = std::str::from_utf8(&self.bytes[self.at..]).unwrap();
            let c = rest.chars().next().expect("unterminated string");
            self.at += c.len_utf8();
            match c {
                '"' => return out,
                '\\' => {
                    let escape = self.bytes[self.at];
                    self.at += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'r' => out.push('\r'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'u' => {
                            let hex =
                                std::str::from_utf8(&self.bytes[self.at..self.at + 4]).unwrap();
                            self.at += 4;
                            out.push(
                                char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap(),
                            );
                        }
                        other => panic!("unknown escape {:?}", other as char),
                    }
                }
                c => out.push(c),
            }
        }
    }
}

/// A fresh directory under the test target for an adversary program and
/// its sibling modules (`docs/slice.md` 14.4).
fn adversary_dir(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir
}

const ADVERSARY_SERVE: &str = "  fn serve(stream: TcpStream, application: fn(Text) -> Text) -> Result<Unit, IoError> requires [stream.write] {\n    return stream.write_all(b\"ok\")\n  }\n";

/// 19.7: the claim model reads "the values of the catalogue constants its
/// protocols name", here a catalogue in a sibling module: its MUST with no
/// status is one E090. The E091 second trigger runs "for the constants
/// declared in the checked file only", so the sibling's misnamed
/// requirement is reported only when the sibling is the checked file.
#[test]
fn adversary_catalogue_in_a_sibling_module_is_claimed() {
    let catalogue = "package advsib.cat\n\nimport std.rfc.{Level, Party, Source, Requirement, Spec}\n\nconst RFC7300: Spec = Spec {\n  rfc = 7300\n  title = \"A sibling catalogue\"\n  url = \"https://example.invalid/rfc7300\"\n  digest = \"sha256:7300\"\n  requirements = [R1_1, R1_9]\n}\n\nconst R1_1: Requirement = Requirement {\n  rfc = 7300\n  section = \"1\"\n  ordinal = 1\n  level = Must\n  text = \"A server MUST answer.\"\n}\n\nconst R1_9: Requirement = Requirement {\n  rfc = 7300\n  section = \"1\"\n  ordinal = 2\n  level = Should\n  text = \"Misnamed: its id is R1_2.\"\n}\n";
    let main = format!(
        "package advsib\n\nimport advsib.cat.{{RFC7300}}\n\nprotocol Echo conforms [RFC7300] {{\n{ADVERSARY_SERVE}}}\n"
    );
    let dir = adversary_dir(
        "adversary-claims-sibling",
        &[("main.ignite", &main), ("cat.ignite", catalogue)],
    );
    let output = ignite_in(&dir, &["check", "main.ignite"]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    assert_eq!(
        text(&output.stderr),
        "main.ignite:5:25: error[E090]: requirement `RFC7300.R1_1` (MUST, section 1) is in scope of `Echo` and has no status; add `satisfies`, `unsupported` or `not_applicable`\n"
    );
    let output = ignite_in(&dir, &["check", "cat.ignite"]);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    assert!(
        text(&output.stderr).contains("error[E091]: requirement `R1_9` must be named `R1_2`"),
        "{}",
        text(&output.stderr)
    );
}

/// Imports are not re-exported (`docs/slice.md` section 4): a spec that a
/// sibling only imports is no name of that sibling. Whatever the frontend
/// does with it, a protocol conforming to it must not check silently
/// while its MUST requirement `RFC1.R1_1` has no status (19.7).
#[test]
fn adversary_spec_imported_through_a_sibling_is_no_silent_claim() {
    let std = fixtures_dir().join("claims_std");
    let main = format!(
        "package advre\n\nimport advre.cat.{{RFC1}}\n\nprotocol Echo conforms [RFC1] {{\n{ADVERSARY_SERVE}}}\n"
    );
    let dir = adversary_dir(
        "adversary-claims-reexport",
        &[
            ("main.ignite", &main),
            (
                "cat.ignite",
                "package advre.cat\n\nimport std.rfc.rfc1.{RFC1}\n\nfn unused() -> Int = 1\n",
            ),
        ],
    );
    let output = ignite_in(
        &dir,
        &["check", "main.ignite", "--std", std.to_str().unwrap()],
    );
    assert_ne!(
        output.status.code(),
        Some(0),
        "a conforming protocol with an unclassified MUST checked silently"
    );
    assert!(!output.stderr.is_empty());
}

/// `docs/slice.md` 22.0 row 10 (22.5, D6): a `Store` resource needs no
/// binding. The assembly of `store_hosted.ignite` binds only its listener
/// and its server; `ignite plan` exits 0, hosts `S.counts` via `S.server`
/// with the server's binding, and the model has the `hosts` edge from the
/// server to the store.
#[test]
fn store_is_hosted_by_its_server() {
    let fixture = fixtures_dir().join("store_hosted.ignite");
    let fixture = fixture.to_str().unwrap();
    let json = Json::parse(&success(&ignite(&["plan", fixture])));
    let resolution: Vec<(&str, &str, &str)> = json.object()["assembly"].object()["resolution"]
        .array()
        .iter()
        .map(Json::object)
        .map(|entry| {
            (
                entry["member"].string(),
                entry["via"].string(),
                entry["binding"].object()["variant"].string(),
            )
        })
        .collect();
    assert_eq!(
        resolution,
        [
            ("S.counts", "S.server", "NativeProcess"),
            ("S.ingress", "S.ingress", "LocalSocket"),
            ("S.server", "S.server", "NativeProcess"),
        ]
    );
    let hosted: Vec<(&str, &str)> = json.object()["edges"]
        .array()
        .iter()
        .map(Json::object)
        .filter(|edge| edge["kind"].string() == "hosts")
        .map(|edge| (edge["from"].string(), edge["to"].string()))
        .collect();
    assert_eq!(hosted, [("S.server", "S.counts")]);
}
