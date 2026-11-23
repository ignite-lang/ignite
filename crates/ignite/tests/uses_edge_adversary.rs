//! Adversary (story conditional-property-keeps-its-uses-edge): the model's
//! `uses` edges (`docs/slice.md` 15.5) are the members a property reads,
//! whatever expression encloses the read, and no member it does not read.
//! Each program is written to the test's scratch directory and planned with
//! `ignite plan`; `explain` reads the same edges.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn source(name: &str, text: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("uses_edge_adversary");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.ignite"));
    std::fs::write(&path, text).unwrap();
    path
}

fn ignite(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// The model of `path`; the plan must succeed.
fn plan(path: &Path) -> String {
    let output = ignite(&["plan", path.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    text(&output.stdout)
}

/// The `(kind, to)` edges out of `from` in the model text.
fn edges_from(model: &str, from: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = model.lines().map(str::trim).collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if *line == format!("\"from\": \"{from}\",") {
            let kind = lines[i + 1]
                .trim_start_matches("\"kind\": \"")
                .trim_end_matches("\",");
            let to = lines[i + 2]
                .trim_start_matches("\"to\": \"")
                .trim_end_matches('"');
            out.push((kind.to_string(), to.to_string()));
        }
    }
    out
}

/// The `explain` lines of `member` that name an edge out of it.
fn explained(path: &Path, member: &str) -> Vec<String> {
    let output = ignite(&["explain", path.to_str().unwrap(), member]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    text(&output.stdout)
        .lines()
        .filter(|l| l.starts_with("uses ") || l.starts_with("references "))
        .map(str::to_string)
        .collect()
}

fn pair(kind: &str, to: &str) -> (String, String) {
    (kind.to_string(), to.to_string())
}

const CALLEE: &str = r#"package uses_edge_adversary_callee

type Box {
  size: Int
}

type Sized {
  size: Int
}

fn base() -> Int = 9

system U {
  component base: Box { size = 7 }
  component call: Sized { size = base() }
}
"#;

/// A member name in callee position is the member, not a function of the
/// same name: the type checker refuses `base()` with E040, so the walk
/// visiting the callee draws no edge to a member the property does not read.
#[test]
fn a_callee_named_like_a_member_is_the_member() {
    let path = source("callee", CALLEE);
    let output = ignite(&["plan", path.to_str().unwrap()]);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("error[E040]: type mismatch: `base` has type `Box` and cannot be called"),
        "{stderr}"
    );
}

const SHADOW: &str = r#"package uses_edge_adversary_shadow

type Box {
  size: Int
}

type Sized {
  size: Int
}

fn apply(f: fn(Box) -> Int, x: Box) -> Int = f(x)

system U {
  component base: Box { size = 7 }
  component other: Box { size = 3 }
  component local: Sized {
    size = if true {
      let base = Box { size = 5 }
      base.size
    } else { 0 }
  }
  component param: Sized { size = apply(|base| base.size, other) }
}
"#;

/// A member name shadowed by a `let` or a closure parameter: the edge to
/// the member is right only if the planned value is the member's (7), the
/// reading of the type checker's `name` (members first) that the evaluator
/// shares.
#[test]
fn a_shadowed_member_name_is_read_as_the_member() {
    let path = source("shadow", SHADOW);
    let model = plan(&path);
    for (from, value, to) in [
        ("U.local", 7, vec![pair("uses", "U.base")]),
        (
            "U.param",
            7,
            vec![pair("uses", "U.base"), pair("uses", "U.other")],
        ),
    ] {
        let start = model.find(&format!("\"id\": \"{from}\"")).unwrap();
        let block = &model[start..start + model[start..].find("\n    }").unwrap()];
        assert!(
            block.contains(&format!("\"size\": {value}")),
            "{from}\n{model}"
        );
        assert_eq!(edges_from(&model, from), to, "{from}\n{model}");
    }
}

const FORMS: &str = r#"package uses_edge_adversary_forms

type Box {
  size: Int
}

type Sized {
  size: Int
}

type Label {
  name: Text
}

type Pair {
  left: Int
  right: Int
}

fn apply(f: fn(Int) -> Int, x: Int) -> Int = f(x)

system U {
  component base: Box { size = 7 }
  component other: Box { size = 3 }
  component interpolated: Label { name = "n{base.size}" }
  component nested: Sized { size = apply(|x| apply(|y| y + base.size, x), 0) }
  component dead: Sized { size = if false { base.size } else { 0 } }
  component negated: Sized { size = if !(base.size > 0) { 0 } else { 1 } }
  component record: Sized { size = (Pair { left = base.size, right = 0 }).left }
  component update: Sized { size = (Pair { right = 1, ..Pair { left = other.size, right = 0 } }).left }
  component two: Sized { size = if base.size > 0 { other.size } else { other.size } }
  component looped: Sized {
    size = if true {
      var n = 0
      for x in [base.size] {
        n = n + x
      }
      n
    } else { 0 }
  }
}
"#;

/// Guards of the claim that every form of expression is walked: string
/// interpolation, nested closures, a dead branch (the edge is syntactic),
/// `!`, record literals, a `for` loop in a block, two members in one
/// property (sorted, once each); `plan` and `explain` agree.
#[test]
fn every_form_of_expression_gives_its_uses_edge() {
    let path = source("forms", FORMS);
    let model = plan(&path);
    for (from, to) in [
        ("U.interpolated", vec!["U.base"]),
        ("U.nested", vec!["U.base"]),
        ("U.dead", vec!["U.base"]),
        ("U.negated", vec!["U.base"]),
        ("U.record", vec!["U.base"]),
        ("U.update", vec!["U.other"]),
        ("U.two", vec!["U.base", "U.other"]),
        ("U.looped", vec!["U.base"]),
    ] {
        let expected: Vec<(String, String)> = to.iter().map(|t| pair("uses", t)).collect();
        assert_eq!(edges_from(&model, from), expected, "{from}\n{model}");
        let lines: Vec<String> = to.iter().map(|t| format!("uses {t}")).collect();
        assert_eq!(explained(&path, from), lines, "{from}");
    }
}

const SIGNAL: &str = r#"package uses_edge_adversary_signal

type Pinger {
  size: Int
}

type Pointer {
  target: Ref<Pinger>
}

type Pointers {
  targets: List<Ref<Pinger>>
}

system S {
  component ping: Pinger { size = 1 }
  component pointer: Pointer { target = if true { ping } else { S.ping } }
  component pointers: Pointers {
    targets = match 1 {
      1 => [ping]
      _ => []
    }
  }
}
"#;

/// References stay references inside an `if` or a `match`: a name where a
/// `Ref` is expected is a `references` edge, not a `uses` edge.
#[test]
fn references_inside_an_if_give_no_uses_edge() {
    let path = source("signal", SIGNAL);
    let model = plan(&path);
    assert_eq!(
        edges_from(&model, "S.pointers"),
        vec![pair("references", "S.ping")],
        "{model}"
    );
    assert_eq!(
        edges_from(&model, "S.pointer"),
        vec![pair("references", "S.ping")],
        "{model}"
    );
}

const SETTING: &str = r#"package uses_edge_adversary_setting

import std.control.{Machine, Supervisor}
import std.deploy.Member
import std.policy.Restart

type Tool {
  tool: Setting
  size: Int
}

type Box {
  size: Int
}

fn Tool.main(self: Tool) -> Result<Unit, IoError> = Ok(Unit)

fn Tool.machine(self: Tool, id: Text) -> Machine = Machine.Supervise(Supervisor {
  id = "{id}.supervisor"
  member = id
  restart = Restart {}
  has_listener = false
})

enum Place {
  NativeProcess,
  InProcess
}

fn admits(binding: Place, member: Member) -> Option<Text> = None

fn includes(binding: Place) -> List<Ref> = []

fn entries(binding: Place) -> List<Text> = match binding {
  NativeProcess => ["main", "machine"]
  InProcess => []
}

type Placement {
  system: SystemRef
  bindings: Map<Ref, Place>
}

system Bench {
  resource tool: Setting { flag = "--tool" }
  resource mode: Setting { flag = "--mode", default = "fast" }
  component runner: Tool {
    tool = mode
    size = match 1 {
      1 => if true { 3 } else { 0 }
      _ => if true {
        let t = tool
        0
      } else { 0 }
    }
  }
}

assembly local: Placement {
  system = Bench
  bindings = {
    Bench.tool = InProcess
    Bench.mode = InProcess
    Bench.runner = NativeProcess
  }
}
"#;

/// E142 follows `uses` edges, so a setting read only inside a `match`
/// arm's block of a started member is refused before anything starts.
#[test]
fn a_setting_read_inside_a_match_is_e142() {
    let path = source("setting", SETTING);
    let state = Path::new(env!("CARGO_TARGET_TMPDIR")).join("uses_edge_adversary_state");
    let _ = std::fs::remove_dir_all(&state);
    let output = ignite(&[
        "run",
        path.to_str().unwrap(),
        "--state",
        state.to_str().unwrap(),
    ]);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert_eq!(stderr.matches("E142").count(), 1, "{stderr}");
    assert!(
        stderr.contains(
            "setting `Bench.tool` (`--tool`) has no `default`, and `Bench.runner` uses it"
        ),
        "{stderr}"
    );
    assert!(!state.exists(), "the run created its state");
}
