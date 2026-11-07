//! Adversary (story:shared-ownership-analysis): the checker's half of the
//! one in-place table (`ignite_check::IN_PLACE`, `docs/slice.md` 20.3.3)
//! against programs whose receiver type the checker does not know
//! statically, and the roots a value-changing row gives its receiver.

use std::path::Path;

use crate::common;
use ignite_syntax::SourceFile;

/// `text` checked against the fixture standard library as `probe.ignite`,
/// rendered as `fixtures::fixtures_match_expected` does.
fn check(text: &str) -> String {
    let std = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/std");
    let (_, modules) =
        common::load_std(&std).unwrap_or_else(|e| panic!("fixture standard library:\n{e}"));
    let source = SourceFile::new("probe.ignite", text.to_string());
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    match ignite_check::check_module(&program, &modules) {
        Ok(_) => String::new(),
        Err(diagnostics) => format!("{}\n", source.render_all(&diagnostics)),
    }
}

/// The unit's claim: "E006 for every Value row through a place rooted at a
/// capture". `Map.put` and `Map.remove` are Value rows; when the capture's
/// type is not declared (`var m = make()`), `in_place(None, "put")` is
/// `None` because the `Map` and `Store` rows disagree, so no E006. The
/// program then runs on LLVM (the closure changes its copy) and fails to
/// compile on Rust (E0525: the closure is `FnMut`), the very disagreement
/// the unit names as fixed.
#[test]
fn closure_changing_a_captured_map_of_undeclared_type_is_e006() {
    let out = check(
        r#"type Box { m: Map<Text, Int> }

fn make() -> Map<Text, Int> = Map.empty()

fn make_box() -> Box = Box { m = Map.empty() }

fn untyped_map() -> Int {
  var m = make()
  let g = |x: Int| {
    m.put("a", x)
    x
  }
  return g(1) + m.len()
}

fn untyped_box() -> Int {
  var b = make_box()
  let g = |x: Int| {
    b.m.remove("a")
    x
  }
  return g(1) + b.m.len()
}
"#,
    );
    assert!(
        out.contains("probe.ignite:10:5: error[E006]: cannot change `m` inside a closure"),
        "m.put on a captured Map of undeclared type passed the checker:\n{out}"
    );
    assert!(
        out.contains("probe.ignite:19:5: error[E006]: cannot change `b` inside a closure"),
        "b.m.remove on a captured record of undeclared type passed the checker:\n{out}"
    );
}

/// A valid program the unit's new root extension refuses: `push` here is
/// a pure user fn called by UFCS on a field of a capture. The receiver's
/// type is not known to the checker, so `builtin_method` answers yes and
/// the new `place_root` arm reports E006. Outside a closure the same call
/// checks clean and runs on both targets; before the unit only a bare
/// name receiver was looked at.
#[test]
fn ufcs_push_on_a_field_of_a_capture_is_no_e006() {
    let out = check(
        r#"type Holder { items: List<Int> }
type Wrap { h: Holder }

fn push(h: Holder, x: Int) -> Int = h.items.len() + x

fn make_wrap() -> Wrap = Wrap { h = Holder { items = [1] } }

fn field_ufcs() -> Int {
  let w = make_wrap()
  let g = |x: Int| w.h.push(x)
  return g(5)
}
"#,
    );
    assert_eq!(
        out, "",
        "a pure UFCS call was refused as an in-place change"
    );
}

/// "A Map.put of a handle losing its roots": the checker extends a
/// receiver's roots with its arguments' for `push`, `set` and `append`
/// only, not for the other Value rows of `IN_PLACE`. A `TcpStream` put
/// into a `Map` and read back through `get` is used without the effect
/// its parameter needs; the same program through `List.push` is E016.
#[test]
#[ignore = "story:map-put-handle-roots"]
fn map_put_of_a_handle_keeps_its_roots() {
    let out = check(
        r#"fn via_list(s: TcpStream) -> Result<Input, IoError> requires [stdin.read] {
  var ls: List<TcpStream> = []
  ls.push(s)
  return ls[0].read(4096)
}

fn via_map(s: TcpStream) -> Result<Input, IoError> requires [stdin.read] {
  var m: Map<Text, TcpStream> = Map.empty()
  m.put("a", s)
  return match m.get("a") {
    Some(t) => t.read(4096)
    None => Err(IoError.new("none"))
  }
}
"#,
    );
    assert!(
        out.contains("error[E016]: missing effect `s.read` for `read`; add it to the `requires` of `via_list`"),
        "the List control case:\n{out}"
    );
    assert!(
        out.contains("add it to the `requires` of `via_map`"),
        "a TcpStream put into a Map lost its roots, no E016:\n{out}"
    );
}
