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
