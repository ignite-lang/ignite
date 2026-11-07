//! Adversary, second pass (story:shared-ownership-analysis at b0aa06b7):
//! the correction moved E006 for a change of a capture to the type checker
//! where the checker cannot tell the receiver's type. The other E006 rule
//! of the same table, a change through an immutable binding (a `let`, a
//! pattern binding, a `for` element), is still decided by the checker
//! alone from `static_type`, so it has the two defects pass 1 found for
//! captures: a `Map` change on a binding of a type the checker does not
//! know passes both phases (LLVM changes the binding, the Rust target fails
//! with E0596), and a user fn named `push` is refused.

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_types::{TypedModules, type_module};

/// The diagnostics of checking and then typing `text`, rendered; empty
/// when both pass. As `ownership::diagnostics`.
fn diagnostics(text: &str) -> String {
    let source = SourceFile::new("probe.ignite", text.to_string());
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    let checked = match check_module(&program, &Modules::new()) {
        Ok(checked) => checked,
        Err(d) => return format!("check: {}\n", source.render_all(&d)),
    };
    match type_module(&program, &checked, &TypedModules::new()) {
        Ok(_) => String::new(),
        Err(d) => format!("type: {}\n", source.render_all(&d)),
    }
}

/// `put` and `remove` through a `let` whose type the checker cannot read
/// off a declaration. With `let m: Map<Text, Int> = make()` the same call
/// is E006 ("cannot `put` through the immutable binding `m`"). Without the
/// annotation both phases pass; `ignite test --target llvm` runs the
/// program and `--target rust` fails with E0596 (`let m` is not `mut`).
/// `make_box` and `make` are non-generic, so the checker even holds their
/// return type (`Local::result`), read by the closure rule only.
#[test]
#[ignore = "story:immutable-binding-e006"]
fn a_map_change_through_a_let_of_undeclared_type_is_e006() {
    let out = diagnostics(
        r#"type Box { m: Map<Text, Int> }

fn make() -> Map<Text, Int> = Map.empty()

fn make_box() -> Box = Box { m = Map.empty() }

fn same<T>(x: T) -> T = x

fn from_fn() -> Int {
  let m = make()
  m.put("a", 1)
  return m.len()
}

fn from_generic() -> Int {
  let m = same(make())
  m.put("a", 1)
  return m.len()
}

fn field_of_record() -> Int {
  let b = make_box()
  b.m.remove("a")
  return b.m.len()
}
"#,
    );
    for at in [
        "probe.ignite:11:3",
        "probe.ignite:17:3",
        "probe.ignite:23:3",
    ] {
        assert!(
            out.contains(&format!("{at}: error[E006]")),
            "a Map change through an immutable binding at {at} passed; \
             the Rust target fails E0596 where LLVM runs it:\n{out}"
        );
    }
}

/// The same through a pattern binding and a `for` element, which the
/// E006 row of `docs/slice.md` section 8 names next to `let`. Both pass
/// both phases; the Rust target fails E0596 on each (`Some(m)`,
/// `for m in ..` are not `mut`).
#[test]
#[ignore = "story:immutable-binding-e006"]
fn a_map_change_through_a_pattern_or_for_binding_of_undeclared_type_is_e006() {
    let out = diagnostics(
        r#"fn make() -> Map<Text, Int> = Map.empty()

fn same<T>(x: T) -> T = x

fn pattern() -> Int {
  match Some(same(make())) {
    Some(m) => {
      m.put("a", 1)
      return m.len()
    }
    None => return 0
  }
}

fn element() -> Int {
  let ms = [same(make())]
  for m in ms {
    m.put("a", 1)
  }
  return 0
}
"#,
    );
    for at in ["probe.ignite:8:7", "probe.ignite:18:5"] {
        assert!(
            out.contains(&format!("{at}: error[E006]")),
            "a Map change through a binding at {at} passed; \
             the Rust target fails E0596 where LLVM runs it:\n{out}"
        );
    }
}

/// A user fn named `push` called by UFCS on a `let` whose type the
/// checker cannot tell: no in-place change, the callee only reads its
/// copy. The correction stopped refusing it inside a closure
/// (`ownership::a_ufcs_call_named_push_on_a_capture_is_no_change`), the
/// immutable-binding rule still refuses it.
#[test]
#[ignore = "story:immutable-binding-e006"]
fn a_ufcs_call_named_push_on_a_let_is_no_change() {
    let out = diagnostics(
        r#"type Holder { items: List<Int> }

fn push(h: Holder, x: Int) -> Int = h.items.len() + x

fn same<T>(x: T) -> T = x

fn f() -> Int {
  let h = same(Holder { items = [1] })
  return h.push(5)
}
"#,
    );
    assert_eq!(
        out, "",
        "a pure UFCS call was refused as an in-place change"
    );
}
