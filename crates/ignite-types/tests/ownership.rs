//! The one table of in-place builtins (`docs/slice.md` 20.3.3) and the
//! builtins it resolves to, which the checker, the moves analysis and
//! both backends read (`ignite_types::ownership`).

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_types::ownership::{self, Change};
use ignite_types::{Builtin, TypedModules, type_module};

/// The diagnostics of checking and then typing `text`, rendered; empty
/// when both pass.
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

/// A `Map` change on a capture whose type the checker cannot tell (the
/// result of a generic fn) is E006 from the type checker, which knows the
/// callee: a closure captures by copy, and the Rust target cannot compile
/// the change (E0525).
#[test]
fn a_change_of_a_capture_of_any_type_is_e006() {
    let out = diagnostics(
        r#"fn make() -> Map<Text, Int> = Map.empty()

fn same<T>(x: T) -> T = x

fn f() -> Int {
  var m = same(make())
  let g = |x: Int| {
    m.put("a", x)
    x
  }
  return g(1) + m.len()
}
"#,
    );
    assert_eq!(
        out,
        "type: probe.ignite:8:5: error[E006]: cannot change `m` inside a closure; a closure captures `m` by copy\n"
    );
}

/// A user fn named like a builtin, called by UFCS on a capture whose type
/// the checker cannot tell, is no in-place change.
#[test]
fn a_ufcs_call_named_push_on_a_capture_is_no_change() {
    let out = diagnostics(
        r#"type Holder { items: List<Int> }

fn push(h: Holder, x: Int) -> Int = h.items.len() + x

fn same<T>(x: T) -> T = x

fn f() -> Int {
  let h = same(Holder { items = [1] })
  let g = |x: Int| h.push(x)
  return g(5)
}
"#,
    );
    assert_eq!(out, "");
}

/// Every row of the table, in order: the receiver type, the method and
/// what the call changes, and the builtin the type checker resolves the
/// row to.
#[test]
fn the_one_table() {
    let rows: Vec<(&str, &str, Change, Builtin)> = ownership::table()
        .iter()
        .map(|(row, builtin)| (row.receiver, row.method, row.change, *builtin))
        .collect();
    use Builtin::*;
    use Change::{Handle, Value};
    assert_eq!(
        rows,
        [
            ("List", "push", Value, ListPush),
            ("List", "set", Value, ListSet),
            ("List", "append", Value, ListAppend),
            ("List", "clear", Value, ListClear),
            ("Bytes", "push", Value, ListPush),
            ("Bytes", "set", Value, ListSet),
            ("Bytes", "append", Value, ListAppend),
            ("Bytes", "clear", Value, ListClear),
            ("Map", "put", Value, MapPut),
            ("Map", "remove", Value, MapRemove),
            ("Store", "next_id", Handle, StoreNextId),
            ("Store", "put", Handle, StorePut),
            ("Store", "remove", Handle, StoreRemove),
            ("Store", "update", Handle, StoreUpdate),
            ("Store", "upsert", Handle, StoreUpsert),
        ]
    );
    assert_eq!(ownership::BY_NAME, ["push", "set", "append", "clear"]);
}

/// [`ownership::mutates`] holds for exactly the builtins of the table,
/// among them none that only reads its receiver.
#[test]
fn mutates_is_the_table() {
    for (row, builtin) in ownership::table() {
        assert!(ownership::mutates(*builtin), "{builtin:?}");
        assert_eq!(ownership::change(*builtin), Some(row.change), "{builtin:?}");
    }
    for builtin in [
        Builtin::ListAt,
        Builtin::ListLen,
        Builtin::MapGet,
        Builtin::StoreGet,
        Builtin::StoreValues,
        Builtin::DirectoryRemove,
        Builtin::Copy,
    ] {
        assert!(!ownership::mutates(builtin), "{builtin:?}");
    }
}
