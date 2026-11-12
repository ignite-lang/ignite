//! Story `std-mysql-client`, unit `mysql-cost`: a read of a list's
//! elements that keeps nothing borrows them on the Rust target instead of
//! copying the list or the element (`docs/slice.md` 20.3.7). A service that
//! walks the rows of a result set read them in these forms, and the copies
//! and their drops were most of its work.
//!
//! Each fn of [`PROGRAM`] is one form; its emitted text holds no copy of
//! the list (`l.clone()`) and none of the element read
//! (`checked::at(..).clone()`).

use crate::common;

const PROGRAM: &str = "enum V {
  N
  S(Text)
  L(List<V>)
}

type Run {
  steps: List<(Text, List<V>)>
}

fn size(v: V) -> Int = match v {
  L(items) => items.len()
  _ => 0
}

fn find_pair(l: List<(Text, V)>, k: Text) -> Int {
  for (name, v) in l {
    if name == k {
      return size(v)
    }
  }
  0
}

fn sum_pairs(l: List<(Int, Text)>) -> Int {
  var n = 0
  for (k, t) in l {
    n = n + k + t.len()
  }
  n
}

fn steps_len(r: Run) -> Int {
  var n = 0
  for (name, vs) in r.steps {
    n = n + name.len() + vs.len()
  }
  n
}

fn nth_param(l: List<V>, i: Int) -> Int = size(l[i])

fn nth_bound(v: V, i: Int) -> Int = match v {
  L(l) => size(l[i])
  _ => 0
}

fn nested(l: List<List<V>>, i: Int, j: Int) -> Int = size(l[i][j])

fn kind_at(l: List<V>, i: Int) -> Int = match l[i] {
  L(items) => items.len()
  S(_) => 1
  N => 0
}

fn count_in(l: List<List<V>>, i: Int) -> Int {
  var n = 0
  for v in l[i] {
    n = n + size(v)
  }
  n
}

fn len_at(l: List<Text>, i: Int) -> Int = l[i].len()

fn same_at(l: List<Text>, i: Int, k: Text) -> Bool = l[i] == k

type Counts {
  counts: List<Int>
}

fn total(l: List<Int>) -> Int {
  var n = 0
  for x in l {
    n = n + x
  }
  n
}

fn ones(b: Bytes) -> Int {
  var n = 0
  for x in b {
    if x == 1 {
      n = n + 1
    }
  }
  n
}

fn counted(c: Counts) -> Int {
  var n = 0
  for x in c.counts {
    n = n + x
  }
  n
}
";

/// The generated fn that starts with `head`, up to its closing brace.
fn section<'a>(lib: &'a str, head: &str) -> &'a str {
    let start = lib
        .find(head)
        .unwrap_or_else(|| panic!("no `{head}` in:\n{lib}"));
    let rest = &lib[start..];
    &rest[..rest.find("\n}\n").expect("end of the fn")]
}

fn lib() -> String {
    common::generate_source(PROGRAM, &common::repository_std())
        .unwrap_or_else(|d| panic!("{d:?}"))
        .lib_rs
}

/// The copies `text` makes of a list or of an element read from one; the
/// copy of an `Int` a pattern binds (`let k = (k).clone();`) is none.
fn copies(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| {
            (line.contains("checked::at(") && line.contains(").clone()"))
                || line.contains(".clone())")
                || line.contains("l.clone()")
                || line.contains("steps.clone()")
        })
        .collect()
}

#[test]
fn a_tuple_pattern_in_a_for_over_a_list_borrows_its_elements() {
    let lib = lib();
    for name in ["find_pair", "sum_pairs", "steps_len"] {
        let text = section(&lib, &format!("pub fn {name}("));
        assert!(copies(text).is_empty(), "{name} copies: {text}");
        assert!(
            text.contains(").iter()"),
            "{name} does not iterate by reference: {text}"
        );
    }
}

#[test]
fn an_indexed_element_passed_to_a_borrowed_parameter_is_lent() {
    let lib = lib();
    for name in ["nth_param", "nth_bound", "nested"] {
        let text = section(&lib, &format!("pub fn {name}("));
        assert!(copies(text).is_empty(), "{name} copies: {text}");
    }
}

#[test]
fn a_for_over_a_list_of_copy_elements_reads_them_out_of_the_list() {
    let lib = lib();
    for name in ["total", "ones", "counted"] {
        let text = section(&lib, &format!("pub fn {name}("));
        assert!(
            text.contains(").iter().copied()") && !text.contains(".clone())"),
            "{name} iterates a copy: {text}"
        );
    }
}

#[test]
fn an_indexed_element_matched_iterated_measured_or_compared_is_read_in_place() {
    let lib = lib();
    for name in ["kind_at", "count_in", "len_at", "same_at"] {
        let text = section(&lib, &format!("pub fn {name}("));
        assert!(copies(text).is_empty(), "{name} copies: {text}");
    }
}

/// Unit `call-moves`: a variable passed by value whose only later reads
/// are in later arguments of the same call (form 9 of the reporter's
/// program), and a `match` on a tuple of two places (form 7). The guards
/// keep their copy: a later argument that assigns the variable or changes
/// it in place, one whose value borrows from it, an arm that assigns a
/// place of the tuple. `crates/ignite/tests/fixtures/call_moves.ignite`
/// runs each of them on the evaluator and both targets.
const MOVES: &str = "enum Val {
  Nil
  Num(Int)
  Txt(Text)
  Rec(List<(Text, Val)>)
}

type Ctx {
  label: Text
  inputs: List<(Text, Val)>
  rows: List<Text>
  count: Int
}

fn put(fields: List<(Text, Val)>, k: Text, v: Val) -> List<(Text, Val)> {
  var out = fields
  out.push((k, v))
  out
}

fn set_in(r: Ctx, name: Text, v: Val) -> Ctx {
  var o = r
  o.inputs = put(o.inputs, name, v)
  o
}

fn read_in(r: Ctx, name: Text) -> Result<Val, Text> {
  for e in r.inputs {
    if e.0 == name {
      return Ok(e.1)
    }
  }
  Err(\"no input {name}\")
}

fn attach_inline(r0: Ctx) -> Result<Ctx, Text> {
  var r = r0
  r = set_in(r, \"a\", read_in(r, \"b\")?)
  r = set_in(r, \"c\", read_in(r, \"a\")?)
  Ok(r)
}

fn attach_receiver(r0: Ctx) -> Result<Ctx, Text> {
  var r = r0
  r = r.set_in(\"a\", r.read_in(\"b\")?)
  Ok(r)
}

fn attach_named(r0: Ctx) -> Result<Ctx, Text> {
  var r = r0
  r = set_in(r = r, v = read_in(r, \"b\")?, name = \"a\")
  Ok(r)
}

fn later_assigns(r0: Ctx) -> Ctx {
  var r = r0
  r = set_in(r, \"a\", if true {
    r = r0
    Num(1)
  } else {
    Nil
  })
  r
}

fn later_pushes(r0: Ctx) -> Ctx {
  var r = r0
  r = set_in(r, \"a\", if true {
    r.rows.push(\"x\")
    Num(1)
  } else {
    Nil
  })
  r
}

fn grown(r: Ctx, rows: List<Text>) -> Int {
  var o = r
  o.rows.push(\"x\")
  o.rows.len() + rows.len()
}

fn later_borrows(r0: Ctx) -> Int {
  let r = r0
  grown(r, r.rows)
}

fn kind_of(v: Val) -> Text = match v {
  Nil => \"nil\"
  Num(_) => \"num\"
  Txt(_) => \"txt\"
  Rec(_) => \"rec\"
}

fn same_kind(a: Val, b: Val) -> Result<Bool, Text> {
  match (a, b) {
    (Txt(x), Txt(y)) => Ok(x == y)
    (Num(x), Num(y)) => Ok(x == y)
    _ => Err(\"cannot compare {kind_of(a)} and {kind_of(b)}\")
  }
}

fn same_local(m: Int) -> Int {
  let a = Txt(\"{m}\")
  let b = Num(m)
  let n = match (a, b) {
    (Txt(x), Num(y)) => x.len() + y
    _ => 0
  }
  n + kind_of(a).len() + kind_of(b).len()
}

fn tuple_assigned(m: Int) -> Int {
  var a = Txt(\"ab\")
  let b = Num(m)
  let n = match (a, b) {
    (Txt(x), Num(y)) => {
      a = Nil
      x.len() + y
    }
    _ => 0
  }
  n + kind_of(a).len() + kind_of(b).len()
}

fn renamed(r: Ctx) -> Ctx = set_in(r, \"k\", Num(r.count))

fn renamed_twice(r0: Ctx) -> Ctx {
  var r = r0
  r = renamed(r)
  r = renamed(r)
  r
}
";

fn moves_lib() -> String {
    common::generate_source(MOVES, &common::repository_std())
        .unwrap_or_else(|d| panic!("{d:?}"))
        .lib_rs
}

#[test]
fn a_variable_read_by_a_later_argument_is_moved_after_it() {
    let lib = moves_lib();
    for name in ["attach_inline", "attach_receiver", "attach_named"] {
        let text = section(&lib, &format!("pub fn {name}("));
        assert!(!text.contains("r.clone()"), "{name} copies r: {text}");
        assert!(
            text.contains("read_in(&r,") || text.contains("read_in(r,"),
            "{name} does not read r in place: {text}"
        );
    }
}

#[test]
fn a_later_argument_that_changes_or_borrows_the_variable_keeps_the_copy() {
    let lib = moves_lib();
    for name in ["later_assigns", "later_pushes", "later_borrows"] {
        let text = section(&lib, &format!("pub fn {name}("));
        assert!(text.contains("r.clone()"), "{name} lost its copy: {text}");
    }
}

#[test]
fn a_tuple_of_places_is_matched_by_reference() {
    let lib = moves_lib();
    let text = section(&lib, "pub fn same_kind(");
    assert!(text.contains("match (a, b) {"), "same_kind: {text}");
    let text = section(&lib, "pub fn same_local(");
    assert!(text.contains("match (&a, &b) {"), "same_local: {text}");
    let text = section(&lib, "pub fn tuple_assigned(");
    assert!(
        text.contains("a.clone()"),
        "tuple_assigned lost its copy: {text}"
    );
}

/// A parameter whose only move is a late one (`set_in(r, "k",
/// Num(r.count))`) is consumed: the fn takes it by value and moves it on,
/// and a caller that updates its record with the fn moves the record in.
/// Were the late move not counted as a move of the parameter, the fn would
/// borrow it and copy it into `set_in` on every call.
#[test]
fn a_parameter_moved_late_is_taken_by_value() {
    let lib = moves_lib();
    let text = section(&lib, "pub fn renamed(");
    assert!(
        text.starts_with("pub fn renamed(r: Ctx)"),
        "renamed: {text}"
    );
    assert_eq!(
        text.matches("r.clone()").count(),
        0,
        "renamed copies: {text}"
    );
    let text = section(&lib, "pub fn renamed_twice(");
    assert_eq!(
        text.matches("r.clone()").count() + text.matches("r0.clone()").count(),
        0,
        "renamed_twice copies: {text}"
    );
}
