//! Adversary of story `rust-target-text-reads-borrow` (`docs/slice.md`
//! section 6, "Text and bytes read in place"): programs whose text bytes,
//! option payloads and `Bytes` parameters the Rust target now reads in
//! place, and whose `to_text` and `to_bytes` may take a buffer, run by the
//! evaluator and both targets. The evaluator is the reference for values;
//! each target runs the program's inline tests with `ignite test` and must
//! pass them all, which means it compiles and gives the evaluator's values.

use crate::borrowed_reads_adversary::agree;

/// Groups 1 and 2: the bytes of a text and the payload of a lent option
/// alive across a change, a move, a capture, a store, a shadow, a loop or
/// a match of their text or option; `to_text` and `to_bytes` at what may
/// or may not be a last use (a later loop round, an arm, a closure, a
/// parameter, an operand an enclosing call holds, an interpolation, an
/// operator, a tuple, a record in either field order, a pinned `for` or
/// `match`).
const LIVENESS: &str = r#"package text_reads_adversary_liveness

type Holder {
  t: Text
  n: Int
}

type Pair2 {
  raw: Bytes
  text: Text
}

type Back {
  text: Text
  raw: Bytes
}

type OptHolder {
  raw: Option<Bytes>
  n: Int
}

enum Wire {
  WRaw(Bytes)
  WTxt(Text)
  WNil
}

fn grown(t: Text) -> Text {
  var u = t
  u = u + "zz"
  u
}

fn take_bytes(b: Bytes) -> Int {
  var c = b
  c.push(1)
  c.len()
}

fn total(b: Bytes) -> Int {
  var n = 0
  for c in b {
    n = n + c.to_int()
  }
  n
}

fn lent_and_text(b: Bytes, t: Text) -> Int = b.len() * 100 + t.len()

fn consume_both(b: Bytes, t: Text) -> Int {
  var c = b
  c.push(7)
  var u = t
  u = u + "!"
  c.len() * 100 + u.len()
}

fn from_b(b: Bytes) -> Text {
  var c = b
  c.push(33)
  c.to_text()
}

fn keep_text(t: Text) -> Text {
  var u = t
  u = u + ""
  u
}

fn make_list() -> List<Bytes> = [b"ab", b"cd"]

fn make_opt(k: Bool) -> Option<Bytes> = if k { Some(b"xyz") } else { None }

// ---- form 1, the binding alive across ----

fn g1_reassigned() -> Int {
  var s = "abc"
  let b = s.to_bytes()
  s = "zzzzz"
  b.len() * 10 + s.len()
}

fn g1_loop_append() -> Int {
  var s = "ab"
  let b = s.to_bytes()
  for c in b {
    s = s + "x"
  }
  b.len() * 10 + s.len()
}

fn g1_ufcs_moved() -> Int {
  let s = "abc"
  let b = s.to_bytes()
  let g = s.grown()
  b.len() * 10 + g.len()
}

fn g1_record() -> Int {
  let s = "abc"
  let b = s.to_bytes()
  let h = Holder { t = s, n = 1 }
  b.len() * 10 + h.t.len()
}

fn g1_pushed() -> Int {
  let s = "abc"
  let b = s.to_bytes()
  var l: List<Text> = []
  l.push(s)
  b.len() * 10 + l.len()
}

fn g1_tuple() -> Int {
  let s = "abc"
  let b = s.to_bytes()
  let p = (s, 2)
  b.len() * 10 + p.0.len()
}

fn g1_variant() -> Int {
  let s = "abcd"
  let b = s.to_bytes()
  let w = WTxt(s)
  let n = match w {
    WTxt(t) => t.len()
    _ => 0
  }
  b.len() * 10 + n
}

fn g1_b_captured() -> Int {
  let s = "abc"
  let b = s.to_bytes()
  let f = |n: Int| b.len() + n
  f(10)
}

fn g1_returned(s: Text) -> Bytes {
  let b = s.to_bytes()
  if b.is_empty() {
    return b"-"
  }
  b
}

fn g1_in_record() -> Int {
  let s = "abc"
  let b = s.to_bytes()
  let p = Pair2 { raw = b, text = s }
  p.raw.len() * 10 + p.text.len()
}

fn g1_b_pushed() -> Int {
  let s = "abc"
  let b = s.to_bytes()
  var l: List<Bytes> = []
  l.push(b)
  l[0].len() * 10 + s.len()
}

fn g1_shadow() -> Int {
  let s = "abc"
  let b = s.to_bytes()
  let n = b.len()
  let b = b"zz"
  n * 10 + b.len()
}

fn g1_shadow_inner(k: Bool) -> Int {
  let s = "abc"
  let b = s.to_bytes()
  var n = 0
  if k {
    let b = "zzzz".to_bytes()
    n = b.len()
  }
  n * 10 + b.len()
}

fn g1_match(k: Int) -> Int {
  let s = "hello"
  let b = s.to_bytes()
  match k {
    1 => b.len()
    _ => total(b)
  }
}

fn g1_nested() -> Int {
  let s = "abc"
  let b = s.to_bytes()
  var n = 0
  for c in b {
    for d in b {
      if d == c {
        continue
      }
      if n > 100000 {
        break
      }
      n = n + d.to_int()
    }
  }
  n
}

fn g1_var() -> Int {
  let s = "abc"
  var b = s.to_bytes()
  let n = b.len()
  b = b"z"
  n * 10 + b.len()
}

fn g1_two_then_taken() -> Int {
  let s = "abc"
  let b = s.to_bytes()
  let c = s.to_bytes()
  var u = s.to_bytes()
  u.push(1)
  b.len() * 100 + c.len() * 10 + u.len()
}

fn g1_field_updated() -> Int {
  var h = Holder { t = "abc", n = 1 }
  let b = h.t.to_bytes()
  h.t = "zzzzz"
  b.len() * 10 + h.t.len()
}

fn g1_record_replaced() -> Int {
  var h = Holder { t = "abc", n = 1 }
  let b = h.t.to_bytes()
  h = Holder { t = "zzzzzz", n = 2 }
  b.len() * 10 + h.t.len()
}

fn g1_field_moved() -> Int {
  let h = Holder { t = "abc", n = 1 }
  let b = h.t.to_bytes()
  let t = h.t
  let g = grown(t)
  b.len() * 10 + g.len()
}

fn g1_other_field_moved() -> Int {
  let h = Holder { t = "abc", n = 1 }
  let b = h.t.to_bytes()
  let r = h
  b.len() * 10 + r.n
}

fn g1_element() -> Int {
  var l = ["abc", "d"]
  let b = l[0].to_bytes()
  l[0] = "zzzzzz"
  b.len() * 10 + l[0].len()
}

fn g1_param(s: Text) -> Int {
  let b = s.to_bytes()
  total(b) + b.len()
}

fn g1_taken_in_later_round() -> Int {
  let s = "abc"
  let b = s.to_bytes()
  var n = 0
  var i = 0
  while i < 2 {
    n = n + b.len()
    i = i + 1
  }
  var u = s.to_bytes()
  u.push(1)
  n * 10 + u.len()
}

fn g1_moved_and_break() -> Int {
  let s = "abc"
  let b = s.to_bytes()
  var n = 0
  for c in b {
    if c == 98 {
      n = n + grown(s).len()
      break
    }
    n = n + 1
  }
  n
}

fn g1_moved_then_read_in_loop() -> Int {
  let s = "abc"
  let b = s.to_bytes()
  let g = grown(s)
  var n = 0
  for c in b {
    n = n + c.to_int()
  }
  n + g.len()
}

fn g1_shadowed_root() -> Int {
  let s = "abc"
  let b = s.to_bytes()
  let s = "zz"
  b.len() * 10 + s.len()
}

fn g1_returned_root(k: Bool) -> Text {
  let s = "abc"
  let b = s.to_bytes()
  if k {
    return s
  }
  "{b.len()}"
}

// ---- form 2, the payload ----

fn g2_replaced_in_loop() -> Int {
  var o = Some(b"abc")
  let b = match o {
    Some(x) => x
    None => return 0
  }
  var n = 0
  for c in b {
    o = None
    n = n + 1
  }
  n * 10 + (match o {
    Some(_) => 1
    None => 2
  })
}

fn g2_option_moved() -> Int {
  let o = Some(b"abc")
  let b = match o {
    Some(x) => x
    None => return 0
  }
  var l: List<Option<Bytes>> = []
  l.push(o)
  b.len() * 10 + l.len()
}

fn g2_field(h: OptHolder) -> Int {
  let b = match h.raw {
    Some(x) => x
    None => return -1
  }
  total(b) + h.n
}

fn g2_field_local_moved() -> Int {
  let h = OptHolder { raw = Some(b"ab"), n = 3 }
  let b = match h.raw {
    Some(x) => x
    None => return -1
  }
  let r = h
  b.len() * 10 + r.n
}

fn g2_guard(o: Option<Bytes>) -> Int {
  let b = match o {
    Some(x) => if x.len() > 1 { x } else { return 0 }
    _ => return 0
  }
  total(b)
}

fn g2_enum(w: Wire) -> Int {
  let b = match w {
    WRaw(x) => x
    WTxt(t) => return t.len()
    WNil => return -1
  }
  b.len() * 100
}

fn g2_loop(l: List<Option<Bytes>>) -> Int {
  var n = 0
  for o in l {
    let b = match o {
      Some(x) => x
      None => continue
    }
    n = n + b.len()
  }
  n
}

fn g2_break(l: List<Option<Bytes>>) -> Int {
  var n = 0
  for o in l {
    let b = match o {
      Some(x) => x
      None => break
    }
    n = n + total(b)
  }
  n
}

fn g2_two_payloads(w: Wire, v: Wire) -> Int {
  let b = match w {
    WRaw(x) => x
    _ => return 0
  }
  let c = match v {
    WRaw(y) => y
    _ => return 1
  }
  b.len() * 10 + c.len()
}

fn g2_closure() -> Int {
  let f = |l: List<Option<Bytes>>| {
    var n = 0
    for o in l {
      let b = match o {
        Some(x) => x
        None => continue
      }
      n = n + b.len()
    }
    n
  }
  f([Some(b"abcd"), None])
}

fn g2_owned_scrutinee(k: Bool) -> Int {
  let o = make_opt(k)
  let b = match o {
    Some(x) => x
    None => return 7
  }
  b.to_text().len()
}

fn g2_text_then_taken() -> Text {
  var o = Some(b"ab")
  let b = match o {
    Some(x) => x
    None => return ""
  }
  let t = b.to_text()
  o = None
  t
}

fn g2_nested(o: Option<Option<Bytes>>) -> Int {
  let b = match o {
    Some(Some(x)) => x
    _ => return 0
  }
  b.len()
}

fn g2_tuple_place() -> Int {
  let p = (Some(b"abc"), 4)
  let b = match p {
    (Some(x), _) => x
    _ => return 0
  }
  b.len() * 10 + p.1
}

// ---- the taken buffer ----

fn t_loop() -> Text {
  let b = b"ab"
  var out = ""
  var i = 0
  while i < 3 {
    out = out + b.to_text()
    i = i + 1
  }
  out
}

fn t_match(k: Int) -> Text {
  let b = "q{k}".to_bytes()
  match k {
    1 => b.to_text()
    2 => "{b.len()}"
    _ => b.to_text() + b.to_text()
  }
}

fn t_closure_twice() -> Text {
  let f = |x: Bytes| {
    var y = x
    y.push(33)
    y.to_text()
  }
  let b = b"ab"
  f(b) + f(b) + b.to_text()
}

fn t_captured_twice() -> Text {
  let b = b"cd"
  let f = |n: Int| b.to_text()
  f(1) + f(2)
}

fn conv(b: Bytes, k: Bool) -> Text {
  if k {
    var c = b
    c.push(33)
    return c.to_text()
  }
  let t = b.to_text()
  t + "?"
}

fn t_cond_move(k: Bool) -> Int {
  let b = b"abc"
  var l: List<Bytes> = []
  if k {
    l.push(b)
  }
  let t = b.to_text()
  l.len() * 10 + t.len()
}

fn t_while() -> Int {
  var b = b"a"
  var n = 0
  while b.len() < 4 {
    let t = b.to_text()
    n = n + t.len()
    b.push(98)
  }
  n
}

fn t_held_lent() -> Int {
  let b = b"ab"
  lent_and_text(b, b.to_text())
}

fn t_held_consumed() -> Int {
  let b = b"ab"
  consume_both(b, b.to_text())
}

fn t_interp() -> Text {
  let t = "ab"
  "{t}{take_bytes(t.to_bytes())}"
}

fn m_interp() -> Text {
  let t = "ab"
  "{t}{keep_text(t)}"
}

fn t_plus() -> Text {
  let t = "ab"
  t + from_b(t.to_bytes())
}

fn m_plus() -> Text {
  let t = "ab"
  t + keep_text(t)
}

fn t_eq() -> Bool {
  let t = "ab"
  t == from_b(t.to_bytes())
}

fn m_eq() -> Bool {
  let t = "ab"
  t == keep_text(t)
}

fn t_tuple() -> Int {
  let b = b"abc"
  let p = (b, b.to_text())
  p.0.len() * 10 + p.1.len()
}

fn t_record_order() -> Int {
  let b = b"abc"
  let p = Pair2 { raw = b, text = b.to_text() }
  let q = b"xy"
  let r = Back { raw = q, text = q.to_text() }
  p.raw.len() * 1000 + p.text.len() * 100 + r.raw.len() * 10 + r.text.len()
}

fn t_record_order_text_first() -> Int {
  let q = b"xy"
  let r = Pair2 { text = q.to_text(), raw = q }
  r.raw.len() * 10 + r.text.len()
}

fn t_owned_for() -> Text {
  var out = ""
  for x in make_list() {
    out = out + x.to_text()
  }
  out
}

fn t_owned_match(k: Bool) -> Text = match make_opt(k) {
  Some(x) => x.to_text()
  None => "none"
}

fn t_lent_tuple(n: Int) -> Int {
  let b = b"abc"
  match (b, n) {
    (x, 1) => x.to_text().len() + b.len()
    _ => 0
  }
}

fn t_list_literal() -> Int {
  let b = b"abc"
  let l = [b, b.to_text().to_bytes()]
  l[0].len() * 10 + l[1].len()
}

fn t_twice_in_call() -> Int {
  let b = b"abc"
  lent_and_text(b.to_text().to_bytes(), b.to_text())
}

fn t_named(b: Bytes) -> Int {
  let c = b"abcd"
  consume_both(t = c.to_text(), b = c)
}

fn t_text_param_consumed(t: Text, k: Bool) -> Int {
  if k {
    let g = grown(t)
    return g.len()
  }
  let b = t.to_bytes()
  take_bytes(b)
}

fn t_pinned_for() -> Text {
  let b = b"ab"
  var out = ""
  for c in b {
    out = out + b.to_text()
  }
  out
}

fn t_pinned_match(t: Text) -> Int {
  let u = t + "!"
  match u {
    "a!" => 1
    _ => take_bytes(u.to_bytes())
  }
}

fn t_pinned_option() -> Text {
  let o = Some("ab")
  var out = ""
  match o {
    Some(x) => {
      out = x
    }
    None => {
      out = ""
    }
  }
  out
}

test "taken buffers pinned by a for or a match" {
  assert t_pinned_for() == "abab"
  assert t_pinned_match("a") == 1
  assert t_pinned_match("abc") == 5
  assert t_pinned_option() == "ab"
}

test "form 1 reassigned" {
  assert g1_reassigned() == 35
}

test "form 1 loop append" {
  assert g1_loop_append() == 24
}

test "form 1 ufcs move" {
  assert g1_ufcs_moved() == 35
}

test "form 1 record, list, tuple, variant" {
  assert g1_record() == 33
  assert g1_pushed() == 31
  assert g1_tuple() == 33
  assert g1_variant() == 44
}

test "form 1 bytes escape" {
  assert g1_b_captured() == 13
  assert g1_returned("ab") == b"ab"
  assert g1_returned("") == b"-"
  assert g1_in_record() == 33
  assert g1_b_pushed() == 33
}

test "form 1 shadows" {
  assert g1_shadow() == 32
  assert g1_shadow_inner(true) == 43
  assert g1_shadow_inner(false) == 3
  assert g1_shadowed_root() == 32
  assert g1_var() == 31
}

test "form 1 match and loops" {
  assert g1_match(1) == 5
  assert g1_match(2) == 532
  assert g1_nested() == 588
}

test "form 1 taken and fields" {
  assert g1_two_then_taken() == 334
  assert g1_field_updated() == 35
  assert g1_record_replaced() == 36
  assert g1_field_moved() == 35
  assert g1_other_field_moved() == 31
  assert g1_element() == 36
  assert g1_param("ab") == 197
  assert g1_taken_in_later_round() == 64
}

test "form 1 moves in a loop" {
  assert g1_moved_and_break() == 6
  assert g1_moved_then_read_in_loop() == 299
  assert g1_returned_root(true) == "abc"
  assert g1_returned_root(false) == "3"
}

test "form 2 payloads" {
  assert g2_replaced_in_loop() == 32
  assert g2_option_moved() == 31
  assert g2_field(OptHolder { raw = Some(b"ab"), n = 1 }) == 196
  assert g2_field(OptHolder { raw = None, n = 1 }) == -1
  assert g2_field_local_moved() == 23
  assert g2_guard(Some(b"ab")) == 195
  assert g2_guard(Some(b"a")) == 0
  assert g2_enum(WRaw(b"abc")) == 300
  assert g2_enum(WTxt("ab")) == 2
  assert g2_enum(WNil) == -1
}

test "form 2 in loops and closures" {
  assert g2_loop([Some(b"ab"), None, Some(b"c")]) == 3
  assert g2_break([Some(b"a"), None, Some(b"c")]) == 97
  assert g2_two_payloads(WRaw(b"ab"), WRaw(b"c")) == 21
  assert g2_two_payloads(WRaw(b"ab"), WNil) == 1
  assert g2_closure() == 4
  assert g2_owned_scrutinee(true) == 3
  assert g2_owned_scrutinee(false) == 7
  assert g2_text_then_taken() == "ab"
  assert g2_nested(Some(Some(b"abc"))) == 3
  assert g2_nested(Some(None)) == 0
  assert g2_tuple_place() == 34
}

test "taken buffers in loops, arms, closures" {
  assert t_loop() == "ababab"
  assert t_match(1) == "q1"
  assert t_match(2) == "2"
  assert t_match(3) == "q3q3"
  assert t_closure_twice() == "ab!ab!ab"
  assert t_captured_twice() == "cdcd"
}

test "taken buffers of parameters and after moves" {
  let x = b"ab"
  assert conv(x, false) == "ab?"
  assert conv(x, true) == "ab!"
  assert x.to_text() == "ab"
  assert t_cond_move(true) == 13
  assert t_cond_move(false) == 3
  assert t_while() == 6
  assert t_text_param_consumed("abc", true) == 5
  assert t_text_param_consumed("abc", false) == 4
}

test "taken buffers inside calls" {
  assert t_held_lent() == 202
  assert t_held_consumed() == 303
  assert t_twice_in_call() == 303
  assert t_named(b"") == 505
}

test "taken buffers in interpolation and operators" {
  assert t_interp() == "ab3"
  assert m_interp() == "abab"
  assert t_plus() == "abab!"
  assert m_plus() == "abab"
  assert !t_eq()
  assert m_eq()
}

test "taken buffers in aggregates" {
  assert t_tuple() == 33
  assert t_record_order() == 3322
  assert t_record_order_text_first() == 22
  assert t_list_literal() == 33
  assert t_owned_for() == "abcd"
  assert t_owned_match(true) == "xyz"
  assert t_owned_match(false) == "none"
  assert t_lent_tuple(1) == 6
  assert t_lent_tuple(2) == 0
}
"#;

/// Group 3 and the non-UTF-8 cases of group 2: `Bytes` parameters a `for`
/// reads (now `&[u8]` on the Rust target) used and called every way, and
/// `to_text` of bytes that are not UTF-8 (invalid first or last byte, a
/// truncated sequence, an overlong encoding, a surrogate, a code point
/// above U+10FFFF) taken at a last use or read where they are.
const PARAMETERS: &str = r#"package text_reads_adversary_parameters

type Box2 {
  raw: Bytes
  f: fn(Bytes) -> Int
}

enum Wire {
  WRaw(Bytes)
  WNil
}

fn summed(b: Bytes) -> Int {
  var n = 0
  for c in b {
    n = n + c.to_int()
  }
  n
}

fn summed_twice(b: Bytes) -> Int {
  var n = 0
  for c in b {
    for d in b {
      n = n + 1
    }
  }
  n + b.len()
}

fn summed_in_arm(b: Bytes, k: Int) -> Int = match k {
  0 => b.len()
  _ => {
    var n = 0
    for c in b {
      if c == 0 {
        continue
      }
      n = n + c.to_int()
    }
    n
  }
}

fn summed_in_else(b: Bytes, k: Bool) -> Int {
  if k {
    b.len()
  } else if b.is_empty() {
    -1
  } else {
    var n = 0
    for c in b {
      n = n + 1
    }
    n * 1000
  }
}

fn summed_while(b: Bytes) -> Int {
  var n = 0
  var i = 0
  while i < 2 {
    for c in b {
      n = n + c.to_int()
    }
    i = i + 1
  }
  loop {
    for c in b {
      n = n + 1
    }
    break
  }
  n
}

fn passed_on(b: Bytes) -> Int {
  var n = 0
  for c in b {
    n = n + 1
  }
  n + summed(b)
}

fn passed_to_value(b: Bytes) -> Int {
  var n = 0
  for c in b {
    n = n + 1
  }
  n + taker(b)
}

fn taker(b: Bytes) -> Int {
  var c = b
  c.push(0)
  c.len()
}

fn compared(b: Bytes) -> Int {
  var n = 0
  for c in b {
    n = n + 1
  }
  if b == b"ab" { n + 100 } else { n }
}

fn rec(b: Bytes, i: Int) -> Int {
  if i >= 2 {
    return 0
  }
  var n = 0
  for c in b {
    n = n + c.to_int()
  }
  n + rec(b, i + 1)
}

fn at_and_slice(b: Bytes) -> Int {
  var n = 0
  for c in b.slice(0, 1) {
    n = n + c.to_int()
  }
  for c in b {
    n = n + 1
  }
  n + b.at(0).to_int() + b.slice(1, 2).len()
}

fn with_text(b: Bytes) -> Text {
  var n = 0
  for c in b {
    n = n + 1
  }
  "{n}" + b.to_text()
}

fn indexed(b: Bytes) -> Int {
  var n = 0
  for c in b {
    n = n + 1
  }
  n + b[0].to_int()
}

fn weight(w: Wire, extra: Bytes) -> Int {
  var n = 0
  for c in extra {
    n = n + 1
  }
  match w {
    WRaw(r) => n + r.len() * 10
    WNil => n
  }
}

fn bytes_first(b: Bytes, k: Int) -> Int {
  var n = k
  for c in b {
    n = n + 1
  }
  n
}

fn apply(f: fn(Bytes) -> Int, b: Bytes) -> Int = f(b)

fn worker(b: Bytes, done: Sender<Int>) -> Result<Unit, IoError> {
  var n = 0
  for c in b {
    n = n + c.to_int()
  }
  done.send(n)
  Ok(())
}

fn callers(t: Text, o: Option<Bytes>, l: List<Bytes>, w: Box2) -> Int {
  var n = summed(b"ab")
  n = n + summed(t.to_bytes())
  n = n + summed(l[0])
  n = n + summed(w.raw)
  n = n + summed(taker_bytes())
  let lent = t.to_bytes()
  n = n + summed(lent)
  let p = match o {
    Some(x) => x
    None => return -1
  }
  n = n + summed(p)
  match o {
    Some(y) => { n = n + summed(y) }
    None => { n = n + 0 }
  }
  for e in l {
    n = n + summed(e)
  }
  n
}

fn taker_bytes() -> Bytes = b"a"

fn values() -> Int {
  let f = summed
  let l = [b"a", b"b"]
  let m = l.map(summed)
  let w = Box2 { raw = b"c", f = summed }
  let g = w.f
  f(b"ab") + m[0] + m[1] + apply(summed, b"d") + g(b"e") + apply(|x: Bytes| summed(x), b"f")
}

fn spawned() -> Int {
  let b = b"abc"
  let (done, results) = Channel.bounded(1)
  spawn worker(b, done)
  match results.receive() {
    Some(n) => n + b.len()
    None => -1
  }
}

fn named() -> Int = bytes_first(k = 5, b = b"ab")

fn ufcs() -> Int {
  let w = WRaw(b"xy")
  w.weight(b"abc") + b"zz".summed()
}

fn utf8(b: Bytes) -> Bytes {
  let t = b.to_text()
  t.to_bytes()
}

fn utf8_lent(b: Bytes) -> Bytes = b.to_text().to_bytes()

fn utf8_local(k: Int) -> Bytes {
  var b = Bytes.empty()
  if k == 0 {
    b = Bytes.of([255, 97])
  } else if k == 1 {
    b = Bytes.of([97, 255])
  } else if k == 2 {
    b = Bytes.of([97, 226, 130])
  } else if k == 3 {
    b = Bytes.of([192, 175, 98])
  } else if k == 4 {
    b = Bytes.of([237, 160, 128])
  } else if k == 5 {
    b = Bytes.of([240, 159, 152, 128])
  } else {
    b = Bytes.of([244, 144, 128, 128, 99])
  }
  let t = b.to_text()
  t.to_bytes()
}

fn closure_for(b: Bytes) -> Int {
  let g = |n: Int| {
    var s = 0
    for c in b {
      s = s + c.to_int()
    }
    s + n
  }
  g(1) + g(2)
}

fn lent_closure_for(t: Text) -> Int {
  let b = t.to_bytes()
  let g = |n: Int| {
    var s = 0
    for c in b {
      s = s + 1
    }
    s + n
  }
  g(10)
}

test "bytes iterated inside a closure" {
  assert closure_for(b"a") == 97 + 1 + 97 + 2
  assert lent_closure_for("abc") == 13
}

test "lent bytes parameters iterated" {
  assert summed(b"ab") == 195
  assert summed_twice(b"abc") == 12
  assert summed_in_arm(b"ab", 0) == 2
  assert summed_in_arm(Bytes.of([0, 97]), 1) == 97
  assert summed_in_else(b"ab", true) == 2
  assert summed_in_else(b"", false) == -1
  assert summed_in_else(b"abc", false) == 3000
  assert summed_while(b"a") == 195
}

test "lent bytes parameters passed on" {
  assert passed_on(b"ab") == 197
  assert passed_to_value(b"ab") == 5
  assert compared(b"ab") == 102
  assert compared(b"a") == 1
  assert rec(b"a", 0) == 194
  assert at_and_slice(b"ab") == 97 + 2 + 97 + 1
  assert with_text(b"xy") == "2xy"
  assert indexed(b"ab") == 99
}

test "lent bytes parameters through every caller" {
  let w = Box2 { raw = b"c", f = summed }
  assert callers("a", Some(b"b"), [b"d", b"e"], w) == 195 + 97 + 100 + 99 + 97 + 97 + 98 + 98 + 100 + 101
  assert callers("a", None, [b"d"], w) == -1
  assert values() == 195 + 97 + 98 + 100 + 101 + 102
  assert spawned() == 297
  assert named() == 7
  assert ufcs() == 23 + 244
}

test "text of bytes that are not UTF-8" {
  assert utf8(Bytes.of([255, 97])) == Bytes.of([239, 191, 189, 97])
  assert utf8_lent(Bytes.of([255, 97])) == Bytes.of([239, 191, 189, 97])
  assert utf8_local(0) == Bytes.of([239, 191, 189, 97])
  assert utf8_local(1) == Bytes.of([97, 239, 191, 189])
  assert utf8_local(2) == Bytes.of([97, 239, 191, 189])
  assert utf8_local(3) == Bytes.of([239, 191, 189, 239, 191, 189, 98])
  assert utf8_local(4) == Bytes.of([239, 191, 189, 239, 191, 189, 239, 191, 189])
  assert utf8_local(5) == Bytes.of([240, 159, 152, 128])
  assert utf8_local(6) == Bytes.of([239, 191, 189, 239, 191, 189, 239, 191, 189, 239, 191, 189, 99])
}
"#;

/// Group 5: `std.protocol.mysql` row readers on crafted packets, every
/// kind of string cell, NULL, an empty text, a cell after a NULL, binary
/// and BIT columns, bytes that are not UTF-8, text and binary protocol.
/// The expected rows are the base decoder's (`de0312d1`), checked with its
/// binary on the Rust target.
const MYSQL_CELLS: &str = r#"package text_reads_adversary_mysql

import std.protocol.mysql.{Column, MysqlValue, read_binary_row, read_text_row}

fn col(kind: Int, charset: Int, length: Int) -> Column = Column { schema = "s", table = "t", name = "c", charset = charset, length = length, kind = kind, flags = 0, decimals = 0 }

fn listed(b: Bytes) -> Text {
  var out = ""
  var first = true
  for c in b {
    if !first {
      out = out + ","
    }
    out = out + "{c.to_int()}"
    first = false
  }
  out
}

fn show(row: List<Option<MysqlValue>>) -> Text {
  var out = ""
  for cell in row {
    out = out + match cell {
      None => "N;"
      Some(Int(n)) => "I{n};"
      Some(Decimal(t)) => "D{t};"
      Some(Bool(b)) => if b { "B1;" } else { "B0;" }
      Some(Text(t)) => "T{listed(t.to_bytes())};"
      Some(Bytes(b)) => "X{listed(b)};"
    }
  }
  out
}

fn cell(out: Bytes, b: Bytes) -> Bytes {
  var o = out
  o.push(match b.len().to_byte() {
    Some(n) => n
    None => 0
  })
  o.append(b)
  o
}

fn text_columns() -> List<Column> = [
  col(253, 45, 255),
  col(253, 45, 255),
  col(253, 45, 255),
  col(253, 45, 255),
  col(252, 63, 65535),
  col(16, 63, 1),
  col(16, 63, 16),
  col(245, 63, 0),
  col(246, 63, 10),
  col(3, 63, 11),
  col(13, 63, 4),
  col(10, 63, 10),
  col(253, 45, 255),
  col(16, 45, 1),
  col(253, 63, 255),
  col(0, 63, 10),
  col(4, 63, 12),
  col(3, 63, 11),
  col(253, 45, 255)
]

fn text_payload() -> Bytes {
  var p = cell(b"", b"ab")
  p.push(251)
  p = cell(p, b"cd")
  p = cell(p, b"")
  p = cell(p, Bytes.of([1, 2]))
  p = cell(p, Bytes.of([1]))
  p = cell(p, Bytes.of([0, 1]))
  p = cell(p, b"[]")
  p = cell(p, b"1.5")
  p = cell(p, b"42")
  p = cell(p, b"2024")
  p = cell(p, b"2024-01-02")
  p = cell(p, Bytes.of([255, 97]))
  p = cell(p, Bytes.of([0]))
  p = cell(p, b"zz")
  p = cell(p, b"7")
  p = cell(p, b"2.5")
  p = cell(p, b"x4")
  p = cell(p, Bytes.of([226, 130]))
  p
}

fn binary_columns() -> List<Column> = [
  col(253, 45, 255),
  col(253, 45, 255),
  col(253, 45, 255),
  col(253, 45, 255),
  col(252, 63, 65535),
  col(16, 63, 1),
  col(16, 63, 16),
  col(245, 63, 0),
  col(246, 63, 10),
  col(253, 45, 255),
  col(0, 63, 10),
  col(15, 45, 20),
  col(254, 63, 4),
  col(16, 45, 1)
]

fn binary_payload() -> Bytes {
  var p = Bytes.of([0, 8, 0])
  p = cell(p, b"ab")
  p = cell(p, b"cd")
  p = cell(p, b"")
  p = cell(p, Bytes.of([1, 2]))
  p = cell(p, Bytes.of([1]))
  p = cell(p, Bytes.of([0, 1]))
  p = cell(p, b"[]")
  p = cell(p, b"1.5")
  p = cell(p, Bytes.of([97, 192, 175]))
  p = cell(p, b"7")
  p = cell(p, b"vc")
  p = cell(p, Bytes.of([0, 255, 0, 1]))
  p = cell(p, Bytes.of([1]))
  p
}

fn text_row() -> Text = match read_text_row(text_columns(), text_payload()) {
  Ok(row) => show(row)
  Err(_) => "error"
}

fn binary_row() -> Text = match read_binary_row(binary_columns(), binary_payload()) {
  Ok(row) => show(row)
  Err(_) => "error"
}

fn short_row() -> Text {
  var p = cell(b"", b"ab")
  p.push(5)
  p.append(b"xy")
  match read_text_row([col(253, 45, 255), col(253, 45, 255)], p) {
    Ok(row) => show(row)
    Err(_) => "error"
  }
}

test "a text row decodes every kind of cell" {
  assert text_row() == "T97,98;N;T99,100;T;X1,2;B1;X0,1;T91,93;D1.5;I42;I2024;T50,48,50,52,45,48,49,45,48,50;T239,191,189,97;B0;X122,122;D7;D2.5;Dx4;T239,191,189;"
}

test "a binary row decodes every kind of string cell" {
  assert binary_row() == "T97,98;N;T99,100;T;X1,2;B1;X0,1;T91,93;D1.5;T97,239,191,189,239,191,189;D7;T118,99;X0,255,0,1;B1;"
}

test "a short text row is malformed" {
  assert short_row() == "error"
}
"#;

/// Group 2: a take where an operand of a map, an operator, an indexed
/// assignment, a builtin receiver or an assert holds the text, and takes in
/// a `loop` left by `break`, a `while` condition, an assert message.
const OPERANDS: &str = r#"package text_reads_adversary_operands

fn from_b(b: Bytes) -> Text {
  var c = b
  c.push(33)
  c.to_text()
}

fn map_put() -> Int {
  var m: Map<Text, Text> = Map.empty()
  let t = "ab"
  m.put(t, from_b(t.to_bytes()))
  match m.get("ab") {
    Some(v) => v.len()
    None => -1
  }
}

fn map_key_after() -> Int {
  var m: Map<Text, Int> = Map.empty()
  let t = "ab"
  m.put(from_b(t.to_bytes()), t.len())
  m.len()
}

fn less() -> Bool {
  let t = "ab"
  t < from_b(t.to_bytes())
}

fn concat() -> Text {
  let t = "ab"
  t.concat(from_b(t.to_bytes()))
}

fn list_set() -> Int {
  var l = ["x", "y"]
  let t = "ab"
  l[0] = from_b(t.to_bytes())
  l[0].len()
}

fn starts() -> Bool {
  let t = "ab"
  t.starts_with(from_b(t.to_bytes()))
}

fn loop_break() -> Text {
  let b = b"ab"
  var out = ""
  loop {
    out = b.to_text()
    break
  }
  out
}

fn while_reassigned() -> Text {
  var b = b"a"
  var out = ""
  var i = 0
  while i < 3 {
    out = out + b.to_text()
    b = "b{i}".to_bytes()
    i = i + 1
  }
  out
}

fn while_cond_take() -> Int {
  var b = b"xy"
  var n = 0
  while b.to_text() != "" {
    b = b""
    n = n + 1
  }
  n
}

fn assert_message(k: Bool) -> Int {
  let b = b"ab"
  assert k, b.to_text()
  1
}

fn nested_call() -> Int {
  let t = "ab"
  let u = from_b(from_b(t.to_bytes()).to_bytes())
  u.len()
}

fn option_default(o: Option<Text>) -> Text {
  let b = b"dflt"
  match o {
    Some(v) => v
    None => b.to_text()
  }
}

fn try_take(b: Bytes) -> Option<Int> {
  let c = b
  match Int.parse(c.to_text()) {
    Some(n) => Some(n + 1)
    None => None
  }
}

test "operands that hold a text while it is taken" {
  assert map_put() == 3
  assert map_key_after() == 1
  assert less()
  assert concat() == "abab!"
  assert list_set() == 3
  assert !starts()
}

test "takes in loops, conditions and messages" {
  assert loop_break() == "ab"
  assert while_reassigned() == "ab0b1"
  assert while_cond_take() == 1
  assert assert_message(true) == 1
  assert nested_call() == 4
  assert option_default(None) == "dflt"
  assert option_default(Some("v")) == "v"
  assert try_take(b"41") == Some(42)
  assert try_take(b"x") == None
}

test "an assert whose left operand holds the text" {
  let t = "ab"
  assert t != from_b(t.to_bytes())
  let u = "cd"
  assert "{u}" + "!" == from_b(u.to_bytes())
}
"#;

/// Group 2: buffers taken from owned scrutinees and iterables (an `if`, a
/// list literal, a field of a call), a `spawn` argument and the parameter
/// of a closure `map` lends or one it changes.
const OWNED: &str = r#"package text_reads_adversary_owned

type Rec {
  raw: Option<Bytes>
  n: Int
}

fn make(k: Bool) -> Rec = Rec { raw = if k { Some(b"mk") } else { None }, n = 1 }

fn worker(t: Text, done: Sender<Int>) -> Result<Unit, IoError> {
  done.send(t.len())
  Ok(())
}

fn if_scrutinee(k: Bool) -> Text {
  let a = Some(b"aa")
  let b = Some(b"bbb")
  let out = match if k { a } else { b } {
    Some(x) => x.to_text()
    None => ""
  }
  out + "{a == None}"
}

fn literal_for() -> Text {
  let a = b"x"
  let b = b"yz"
  var out = ""
  for e in [a, b] {
    out = out + e.to_text()
  }
  out + "{a.len()}"
}

fn field_of_call(k: Bool) -> Text = match make(k).raw {
  Some(x) => x.to_text()
  None => "none"
}

fn spawned_take() -> Int {
  let b = b"abcd"
  let (done, results) = Channel.bounded(1)
  spawn worker(b.to_text(), done)
  match results.receive() {
    Some(n) => n
    None => -1
  }
}

fn mapped(l: List<Bytes>) -> List<Text> = l.map(|x: Bytes| x.to_text())

fn mapped_changed(l: List<Bytes>) -> List<Text> = l.map(|x: Bytes| {
  var y = x
  y.push(33)
  y.to_text()
})

test "owned scrutinees and iterables give up their buffers" {
  assert if_scrutinee(true) == "aafalse"
  assert if_scrutinee(false) == "bbbfalse"
  assert literal_for() == "xyz1"
  assert field_of_call(true) == "mk"
  assert field_of_call(false) == "none"
  assert spawned_take() == 4
  assert mapped([b"a", b"b"]) == ["a", "b"]
  assert mapped_changed([b"a"]) == ["a!"]
}
"#;

/// Group 2: a `to_text` or `to_bytes` inside a `for` over the buffer that
/// leaves the loop after it, and inside a `match` on the buffer whose arm
/// binding is read after it: the loop and the binding still borrow it.
const PINNED: &str = r#"package text_reads_adversary_pinned

fn for_break() -> Text {
  let b = b"ab"
  var out = ""
  for c in b {
    out = b.to_text()
    break
  }
  out
}

fn for_return() -> Text {
  let b = b"cd"
  for c in b {
    if c == 99 {
      return b.to_text()
    }
  }
  "none"
}

fn match_binding() -> Int {
  let b = b"xyz"
  match b {
    x => {
      let t = b.to_text()
      x.len() * 10 + t.len()
    }
  }
}

fn match_text_binding(k: Int) -> Int {
  let s = "q{k}"
  match s {
    "q0" => 0
    other => {
      let u = s.to_bytes()
      other.len() * 10 + u.len()
    }
  }
}

test "a buffer a for or a match holds is not taken inside it" {
  assert for_break() == "ab"
  assert for_return() == "cd"
  assert match_binding() == 33
  assert match_text_binding(0) == 0
  assert match_text_binding(12) == 33
}
"#;

/// Group 3: a closure that iterates a `Bytes` parameter and outlives the
/// call (returned, stored in a record): the parameter must stay a value the
/// closure copies, not a lent `&[u8]`.
const ESCAPING: &str = r#"package text_reads_adversary_escaping

type Counter {
  count: fn(Int) -> Int
}

fn counter_of(b: Bytes) -> fn(Int) -> Int = |n: Int| {
  var s = n
  for c in b {
    s = s + c.to_int()
  }
  s
}

fn record_of(b: Bytes) -> Counter = Counter { count = |n: Int| {
  var s = n
  for c in b {
    s = s + 1
  }
  s
} }

test "a closure that iterates a bytes parameter outlives the call" {
  let f = counter_of(b"ab")
  assert f(1) == 196
  let r = record_of(b"xyz")
  let g = r.count
  assert g(10) == 13
}
"#;

#[test]
fn lent_bytes_and_taken_buffers_keep_the_evaluator_value_on_both_targets() {
    agree("text_reads_adversary_liveness", LIVENESS, 17);
}

#[test]
fn bytes_parameters_read_by_a_for_build_and_agree_on_both_targets() {
    agree("text_reads_adversary_parameters", PARAMETERS, 5);
}

#[test]
fn mysql_cells_decode_as_before_on_the_evaluator_and_both_targets() {
    agree("text_reads_adversary_mysql", MYSQL_CELLS, 3);
}

#[test]
fn taken_buffers_beside_operands_that_hold_them_agree_on_both_targets() {
    agree("text_reads_adversary_operands", OPERANDS, 3);
}

#[test]
fn buffers_of_owned_scrutinees_and_iterables_agree_on_both_targets() {
    agree("text_reads_adversary_owned", OWNED, 1);
}

#[test]
fn buffers_a_for_or_a_match_holds_agree_on_both_targets() {
    agree("text_reads_adversary_pinned", PINNED, 1);
}

#[test]
fn closures_that_iterate_a_bytes_parameter_and_escape_agree_on_both_targets() {
    agree("text_reads_adversary_escaping", ESCAPING, 1);
}
