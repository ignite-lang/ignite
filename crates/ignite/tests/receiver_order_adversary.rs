//! Adversary (stories `place-receiver-read-before-index-and-prefix-argument`
//! and `temporary-receiver-evaluated-before-arguments`, `docs/slice.md`
//! 10.1, "Receiver first"): programs whose builtin receiver and arguments
//! both have effects, run on the evaluator, the Rust target and the LLVM
//! target ([`agree`]). Each order program records the receiver as digit 1
//! and the arguments as digits 2 and 3 of `order`.

use crate::borrowed_reads_adversary::agree;

/// `set` on a temporary receiver. The evaluator and the LLVM target run
/// the receiver first (12, 123), as for `push`, `append`, `put` and
/// `remove`; the Rust target binds the arguments to `let`s before it
/// evaluates the receiver (`indexed_mutation`, `*checked::at_mut(&mut
/// (receiver), __p3, ..) = __p4`) and gives 21 and 231.
const TEMPORARY_SET: &str = r#"package temporary_set

fn set_order() -> Int {
  var order = 0
  let _ = (if true {
    order = order * 10 + 1
    [0]
  } else {
    [0]
  }).set(0, if true {
    order = order * 10 + 2
    5
  } else {
    5
  })
  order
}

fn set_order_both() -> Int {
  var order = 0
  let _ = (if true {
    order = order * 10 + 1
    [0]
  } else {
    [0]
  }).set(if true {
    order = order * 10 + 2
    0
  } else {
    0
  }, if true {
    order = order * 10 + 3
    5
  } else {
    5
  })
  order
}

test "set runs a temporary receiver before its value" {
  assert set_order() == 12
}

test "set runs a temporary receiver before its index and value" {
  assert set_order_both() == 123
}
"#;

#[test]
fn adversary_set_on_a_temporary_runs_the_receiver_first() {
    agree("temporary_set", TEMPORARY_SET, 2);
}

/// A place receiver reached through an index (`l[i].push(x)`) whose index
/// has an effect. The evaluator (`place_path`) and the Rust target
/// (`mutable_place`, the index bound first) run the index before the
/// argument and give 12; the LLVM target lowers the whole place after the
/// arguments (`mutable_receiver` in `changed_receiver`) and gives 21.
const INDEXED_PLACE: &str = r#"package indexed_place

type Bag {
  items: List<Int>
}

fn push_order() -> Int {
  var l = [[0], [0]]
  var order = 0
  l[if true {
    order = order * 10 + 1
    0
  } else {
    0
  }].push(if true {
    order = order * 10 + 2
    5
  } else {
    5
  })
  order
}

fn set_order() -> Int {
  var l = [[0], [0]]
  var order = 0
  l[if true {
    order = order * 10 + 1
    0
  } else {
    0
  }].set(0, if true {
    order = order * 10 + 2
    5
  } else {
    5
  })
  order
}

fn put_order() -> Int {
  var ms: List<Map<Text, Int>> = [Map.empty()]
  var order = 0
  ms[if true {
    order = order * 10 + 1
    0
  } else {
    0
  }].put(if true {
    order = order * 10 + 2
    "a"
  } else {
    "a"
  }, 1)
  order
}

fn field_push_order() -> Int {
  var l = [Bag { items = [0] }]
  var order = 0
  l[if true {
    order = order * 10 + 1
    0
  } else {
    0
  }].items.push(if true {
    order = order * 10 + 2
    5
  } else {
    5
  })
  order
}

test "push runs the index of its place before its argument" {
  assert push_order() == 12
}

test "set runs the index of its place before its arguments" {
  assert set_order() == 12
}

test "put runs the index of its place before its arguments" {
  assert put_order() == 12
}

test "push on a field of an element runs the index before its argument" {
  assert field_push_order() == 12
}
"#;

#[test]
fn adversary_the_index_of_a_changed_place_runs_before_the_arguments() {
    agree("indexed_place", INDEXED_PLACE, 4);
}

/// The open form of `docs/slice.md` 10.1: a place receiver of `push`
/// whose argument assigns it. The evaluator and the LLVM target push onto
/// the value the argument left (`[4, 5, 6]`); the Rust target generates
/// `l.push(if true { l = vec![4, 5]; 6 } ..)` and rustc refuses it
/// (E0506).
const PUSH_ASSIGNS: &str = r#"package push_assigns

fn push_assigns() -> Int {
  var l = [1, 2, 3]
  l.push(if true {
    l = [4, 5]
    6
  } else {
    6
  })
  l.len() * 100 + l[2]
}

test "push onto a list its argument assigns" {
  assert push_assigns() == 306
}
"#;

#[test]
#[ignore = "story:arguments-and-views-replaced-by-a-later-argument"]
fn adversary_push_whose_argument_assigns_its_place() {
    agree("push_assigns", PUSH_ASSIGNS, 1);
}

/// A read-only receiver that is a view of a local, not a place:
/// `t.to_bytes()` and `b.slice(..)` as the haystack of `index_of`, whose
/// argument assigns `t` or `b`. `replaced_by` only copies a place, so the
/// Rust target lends `&t` / `checked::slice_ref(&b, ..)` across the
/// assignment and rustc refuses it (E0506); the LLVM target copies it
/// (`memory_root` is `None`) and gives 2.
const VIEW_RECEIVER: &str = r#"package view_receiver

fn bytes_of_text() -> Int {
  var t = "hello"
  match t.to_bytes().index_of(if true {
    t = "zzzzz"
    b"l"
  } else {
    b"l"
  }, 0) {
    Some(i) => i
    None => -1
  }
}

fn slice_of_bytes() -> Int {
  var b = b"hello"
  match b.slice(0, 5).index_of(if true {
    b = b"zz"
    b"l"
  } else {
    b"l"
  }, 0) {
    Some(i) => i
    None => -1
  }
}

test "index_of searches the bytes of a text its argument replaces" {
  assert bytes_of_text() == 2
}

test "index_of searches a slice of bytes its argument replaces" {
  assert slice_of_bytes() == 2
}
"#;

#[test]
#[ignore = "story:arguments-and-views-replaced-by-a-later-argument"]
fn adversary_a_view_receiver_whose_argument_replaces_its_local() {
    agree("view_receiver", VIEW_RECEIVER, 2);
}

/// An earlier argument of `index_of` or `put` that a later argument
/// replaces, in the arms this change reordered. The evaluator reads each
/// argument before the next one runs: the needle is `b"l"` (2) and the key
/// `"1234567"` (1). The LLVM target reads the needle and the key through a
/// pointer to the variable after the later argument assigned it, and gives
/// 0 and -1; the Rust target lends `&n` across the assignment and rustc
/// refuses `needle` (E0506). `set` with a scalar index and a temporary
/// map are controls.
const ARGUMENT_REPLACED: &str = r#"package argument_replaced

fn needle() -> Int {
  let b = b"hello"
  var n = b"l"
  match b.index_of(n, if true {
    n = b"h"
    0
  } else {
    0
  }) {
    Some(i) => i
    None => -1
  }
}

fn key() -> Int {
  var m: Map<Text, Int> = Map.empty()
  var k = Int.to_text(1234567)
  m.put(k, if true {
    k = Int.to_text(7654321)
    1
  } else {
    1
  })
  match m.get("1234567") {
    Some(v) => v
    None => -1
  }
}

fn key_of_temporary() -> Int {
  var k = Int.to_text(1234567)
  var m: Map<Text, Int> = Map.empty()
  m.put("1234567", 5)
  let _ = (if true {
    m
  } else {
    m
  }).put(k, if true {
    k = Int.to_text(7654321)
    1
  } else {
    1
  })
  k.len()
}

fn set_index() -> Int {
  var l = [0, 0]
  var i = 0
  l.set(i, if true {
    i = 1
    5
  } else {
    5
  })
  l[0]
}

test "index_of searches the needle before a later argument replaces it" {
  assert needle() == 2
}

test "put inserts the key before a later argument replaces it" {
  assert key() == 1
}

test "put on a temporary keeps its key variable" {
  assert key_of_temporary() == 7
}

test "set writes at the index before a later argument changes it" {
  assert set_index() == 5
}
"#;

#[test]
#[ignore = "story:arguments-and-views-replaced-by-a-later-argument"]
fn adversary_an_argument_replaced_by_a_later_argument() {
    agree("argument_replaced", ARGUMENT_REPLACED, 4);
}

/// Guards: forms the change reaches that no other test decides, green on
/// both targets when written. Receivers that are fields, tuple fields and
/// nested indexes; arguments that change the receiver by `clear`, `push`
/// (200 times, past any capacity), `set`, `put`, a `match` arm, a `while`
/// body, with `?`, or that move it into a fn; UTF-8 receivers; heap texts
/// replaced under `concat`, `+` and `starts_with`; a temporary receiver of
/// every read-only builtin (`at`, `slice`, `strip_prefix`, `join`,
/// `pieces`, `index_of`, `concat`, `wrapping_add`) run before its
/// arguments.
const GUARDS: &str = r#"package receiver_order_guards

type Bag {
  items: List<Int>
  name: Text
}

fn count(l: List<Int>) -> Int {
  l.len()
}

fn zero() -> Result<Int, Text> {
  Ok(0)
}

fn head(t: Text) -> Text {
  t
}

fn field_index() -> Int {
  var r = Bag { items = [10], name = "cat" }
  r.items[if true {
    r = Bag { items = [20], name = "dog" }
    0
  } else {
    0
  }]
}

fn field_assign_index() -> Int {
  var r = Bag { items = [10], name = "cat" }
  r.items[if true {
    r.items = [20]
    0
  } else {
    0
  }]
}

fn field_prefix() -> Bool {
  var r = Bag { items = [10], name = "cat" }
  r.name.starts_with(if true {
    r.name = "dog"
    "c"
  } else {
    "c"
  })
}

fn nested_index() -> Int {
  var l = [[10]]
  l[0][if true {
    l = [[20]]
    0
  } else {
    0
  }]
}

fn clear_in_index() -> Int {
  var l = [10]
  l[if true {
    l.clear()
    0
  } else {
    0
  }]
}

fn push_in_index() -> Int {
  var l = [10]
  l[if true {
    var k = 0
    while k < 200 {
      l.push(k)
      k = k + 1
    }
    0
  } else {
    0
  }] + l.len()
}

fn set_in_index() -> Int {
  var l = [10]
  l[if true {
    l[0] = 20
    0
  } else {
    0
  }]
}

fn put_in_get() -> Int {
  var m: Map<Text, Int> = Map.empty()
  match m.get(if true {
    m.put("a", 9)
    "a"
  } else {
    "a"
  }) {
    Some(v) => v
    None => -1
  }
}

fn put_in_contains() -> Bool {
  var m: Map<Text, Int> = Map.empty()
  !m.contains(if true {
    m.put("a", 9)
    "a"
  } else {
    "a"
  })
}

fn match_in_index(n: Int) -> Int {
  var l = [10]
  l[match n {
    0 => {
      l = [20]
      0
    }
    _ => 0
  }]
}

fn loop_in_index() -> Int {
  var l = [10]
  l[if true {
    var j = 0
    while j < 1 {
      l = [20]
      j = j + 1
    }
    0
  } else {
    0
  }]
}

fn try_in_index() -> Result<Int, Text> {
  var l = [10]
  let v = l[if true {
    l = [20]
    zero()?
  } else {
    0
  }]
  Ok(v)
}

fn moved_in_index() -> Int {
  var l = [10]
  l[if true {
    count(l) - 1
  } else {
    0
  }]
}

fn moved_in_prefix() -> Bool {
  let t = "cat"
  t.starts_with(head(t))
}

fn moved_in_push() -> Int {
  var l = [1]
  l.push(count(l))
  0
}

fn bytes_taken_in_prefix() -> Bool {
  var t = "cat"
  t.starts_with(if true {
    var b = t.to_bytes()
    b.push(100)
    "c"
  } else {
    "c"
  })
}

fn wrapping_replaced() -> Int {
  var n = 1
  n.wrapping_add(if true {
    n = 5
    1
  } else {
    1
  })
}

fn concat_replaced() -> Text {
  var t = "cat"
  t.concat(if true {
    t = "dog"
    "x"
  } else {
    "x"
  })
}

fn join_replaced() -> Text {
  var l = ["a", "b"]
  l.join(if true {
    l = ["z"]
    ","
  } else {
    ","
  })
}

fn pieces_replaced() -> Int {
  var t = "a,b,c"
  t.pieces(if true {
    t = "x"
    ","
  } else {
    ","
  }).len()
}

fn bytes_index_of_replaced() -> Int {
  var b = b"hello"
  match b.index_of(if true {
    b = b"zz"
    b"l"
  } else {
    b"l"
  }, 0) {
    Some(i) => i
    None => -1
  }
}

fn utf8_strip() -> Text {
  var t = "éa"
  match t.strip_prefix(if true {
    t = "üb"
    "é"
  } else {
    "é"
  }) {
    Some(rest) => rest
    None => "none"
  }
}

fn temp_prefix_order() -> Int {
  var order = 0
  let ok = (if true {
    order = order * 10 + 1
    "cat"
  } else {
    "cat"
  }).starts_with(if true {
    order = order * 10 + 2
    "c"
  } else {
    "c"
  })
  if ok { order } else { -1 }
}

fn temp_index_order() -> Int {
  var order = 0
  let v = (if true {
    order = order * 10 + 1
    [5]
  } else {
    [5]
  })[if true {
    order = order * 10 + 2
    0
  } else {
    0
  }]
  order * 10 + v
}

fn temp_append_order() -> Int {
  var order = 0
  let _ = (if true {
    order = order * 10 + 1
    [0]
  } else {
    [0]
  }).append(if true {
    order = order * 10 + 2
    [5]
  } else {
    [5]
  })
  order
}

fn temp_remove_order() -> Int {
  var order = 0
  var m: Map<Text, Int> = Map.empty()
  m.put("a", 1)
  let _ = (if true {
    order = order * 10 + 1
    m
  } else {
    m
  }).remove(if true {
    order = order * 10 + 2
    "a"
  } else {
    "a"
  })
  order
}

test "field_index" {
  assert field_index() == 10
}

test "field_assign_index" {
  assert field_assign_index() == 10
}

test "field_prefix" {
  assert field_prefix()
}

test "nested_index" {
  assert nested_index() == 10
}

test "clear_in_index" {
  assert clear_in_index() == 10
}

test "push_in_index" {
  assert push_in_index() == 211
}

test "set_in_index" {
  assert set_in_index() == 10
}

test "put_in_get" {
  assert put_in_get() == -1
}

test "put_in_contains" {
  assert put_in_contains()
}

test "match_in_index" {
  assert match_in_index(0) == 10
}

test "loop_in_index" {
  assert loop_in_index() == 10
}

test "try_in_index" {
  assert try_in_index() == Ok(10)
}

test "moved_in_index" {
  assert moved_in_index() == 10
}

test "moved_in_prefix" {
  assert moved_in_prefix()
}

test "moved_in_push" {
  assert moved_in_push() == 0
}

test "bytes_taken_in_prefix" {
  assert bytes_taken_in_prefix()
}

test "wrapping_replaced" {
  assert wrapping_replaced() == 2
}

test "concat_replaced" {
  assert concat_replaced() == "catx"
}

test "join_replaced" {
  assert join_replaced() == "a,b"
}

test "pieces_replaced" {
  assert pieces_replaced() == 3
}

test "bytes_index_of_replaced" {
  assert bytes_index_of_replaced() == 2
}

test "utf8_strip" {
  assert utf8_strip() == "a"
}

test "temp_prefix_order" {
  assert temp_prefix_order() == 12
}

test "temp_index_order" {
  assert temp_index_order() == 125
}

test "temp_append_order" {
  assert temp_append_order() == 12
}

test "temp_remove_order" {
  assert temp_remove_order() == 12
}

fn tuple_index() -> Int {
  var p = ([10], 1)
  p.0[if true {
    p = ([20], 2)
    0
  } else {
    0
  }]
}

fn field_at() -> Int {
  var r = Bag { items = [10, 11], name = "cat" }
  r.items.at(if true {
    r.items = [20, 21]
    1
  } else {
    1
  })
}

fn field_slice() -> Int {
  var r = Bag { items = [10, 11], name = "cat" }
  r.items.slice(0, if true {
    r = Bag { items = [20, 21], name = "dog" }
    1
  } else {
    1
  })[0]
}

fn heap_concat() -> Text {
  var t = Int.to_text(1234567)
  t.concat(if true {
    t = Int.to_text(7654321)
    let z = Int.to_text(9999999)
    "x"
  } else {
    "x"
  })
}

fn heap_plus() -> Text {
  var t = Int.to_text(1234567)
  t + if true {
    t = Int.to_text(7654321)
    let z = Int.to_text(9999999)
    "x"
  } else {
    "x"
  }
}

fn heap_prefix() -> Bool {
  var t = Int.to_text(1234567)
  t.starts_with(if true {
    t = Int.to_text(7654321)
    "12"
  } else {
    "12"
  })
}

fn map_field_get() -> Int {
  var m: Map<Text, Int> = Map.empty()
  m.put("a", 7)
  var ms = [m]
  match ms[0].get(if true {
    ms = [Map.empty()]
    "a"
  } else {
    "a"
  }) {
    Some(v) => v
    None => -1
  }
}

fn index_index_name() -> Int {
  var l = [10, 11]
  var i = 0
  l[l[i]-10 + if true {
    l = [20, 21]
    0
  } else {
    0
  }]
}

test "tuple_index" {
  assert tuple_index() == 10
}

test "field_at" {
  assert field_at() == 11
}

test "field_slice" {
  assert field_slice() == 10
}

test "heap_concat" {
  assert heap_concat() == "1234567x"
}

test "heap_plus" {
  assert heap_plus() == "1234567x"
}

test "heap_prefix" {
  assert heap_prefix()
}

test "map_field_get" {
  assert map_field_get() == 7
}

test "index_index_name" {
  assert index_index_name() == 10
}

fn list_slice_join() -> Text {
  var l = ["a", "b", "c"]
  l.slice(0, 2).join(if true {
    l = ["z"]
    ","
  } else {
    ","
  })
}

fn list_slice_index() -> Int {
  var l = [10, 11, 12]
  l.slice(1, 3)[if true {
    l = [20]
    0
  } else {
    0
  }]
}

fn to_bytes_index() -> Int {
  var t = "abc"
  t.to_bytes()[if true {
    t = "xyz"
    0
  } else {
    0
  }].to_int()
}

fn slice_pieces() -> Int {
  var l = ["a,b", "c"]
  l.slice(0, 1)[0].pieces(if true {
    l = ["x"]
    ","
  } else {
    ","
  }).len()
}

test "list_slice_join" {
  assert list_slice_join() == "a,b"
}

test "list_slice_index" {
  assert list_slice_index() == 11
}

test "to_bytes_index" {
  assert to_bytes_index() == 97
}

test "slice_pieces" {
  assert slice_pieces() == 2
}

fn at_order() -> Int {
  var order = 0
  let v = (if true {
    order = order * 10 + 1
    [5, 6]
  } else {
    [5, 6]
  }).at(if true {
    order = order * 10 + 2
    1
  } else {
    1
  })
  order * 10 + v
}

fn slice_order() -> Int {
  var order = 0
  let s = (if true {
    order = order * 10 + 1
    [5, 6]
  } else {
    [5, 6]
  }).slice(0, if true {
    order = order * 10 + 2
    1
  } else {
    1
  })
  order * 10 + s.len()
}

fn strip_order() -> Int {
  var order = 0
  let s = (if true {
    order = order * 10 + 1
    "cat"
  } else {
    "cat"
  }).strip_prefix(if true {
    order = order * 10 + 2
    "c"
  } else {
    "c"
  })
  match s {
    Some(_) => order
    None => -1
  }
}

fn join_order() -> Int {
  var order = 0
  let s = (if true {
    order = order * 10 + 1
    ["a", "b"]
  } else {
    ["a", "b"]
  }).join(if true {
    order = order * 10 + 2
    ","
  } else {
    ","
  })
  order * 10 + s.len()
}

fn pieces_order() -> Int {
  var order = 0
  let p = (if true {
    order = order * 10 + 1
    "a,b"
  } else {
    "a,b"
  }).pieces(if true {
    order = order * 10 + 2
    ","
  } else {
    ","
  })
  order * 10 + p.len()
}

fn index_of_order() -> Int {
  var order = 0
  let found = (if true {
    order = order * 10 + 1
    b"hello"
  } else {
    b"hello"
  }).index_of(if true {
    order = order * 10 + 2
    b"l"
  } else {
    b"l"
  }, if true {
    order = order * 10 + 3
    0
  } else {
    0
  })
  match found {
    Some(i) => order * 10 + i
    None => -1
  }
}

fn concat_order() -> Int {
  var order = 0
  let s = (if true {
    order = order * 10 + 1
    "a"
  } else {
    "a"
  }).concat(if true {
    order = order * 10 + 2
    "b"
  } else {
    "b"
  })
  order * 10 + s.len()
}

fn wrapping_order() -> Int {
  var order = 0
  let s = (if true {
    order = order * 10 + 1
    1
  } else {
    1
  }).wrapping_add(if true {
    order = order * 10 + 2
    1
  } else {
    1
  })
  order * 10 + s
}

fn set_place_value_order() -> Int {
  var l = [0, 0]
  var order = 0
  l.set(if true {
    order = order * 10 + 1
    0
  } else {
    0
  }, if true {
    order = order * 10 + 2
    5
  } else {
    5
  })
  order
}

test "at_order" {
  assert at_order() == 126
}

test "slice_order" {
  assert slice_order() == 121
}

test "strip_order" {
  assert strip_order() == 12
}

test "join_order" {
  assert join_order() == 123
}

test "pieces_order" {
  assert pieces_order() == 122
}

test "index_of_order" {
  assert index_of_order() == 1232
}

test "concat_order" {
  assert concat_order() == 122
}

test "wrapping_order" {
  assert wrapping_order() == 122
}

test "set_place_value_order" {
  assert set_place_value_order() == 12
}

"#;

#[test]
fn adversary_receiver_first_guards() {
    agree("receiver_order_guards", GUARDS, 47);
}
