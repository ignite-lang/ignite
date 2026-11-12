//! Adversary (story `std-mysql-client`, unit `call-moves`): programs in
//! which a place moves into a fn call after the later arguments that read
//! it, or a `match` reads a tuple of places by reference (`docs/slice.md`
//! 20.3.7), run by the evaluator and both targets
//! (`borrowed_reads_adversary::agree`: every inline test passes on each).

use crate::borrowed_reads_adversary::agree;

/// A lent `let` (`let b = r.label.to_bytes()` read only as a slice is
/// `let b: &[u8] = r.label.as_bytes();`, `docs/slice.md` 20.3.7) is alive
/// across a call that moves `r` late because a later argument reads
/// `r.count`. The move analysis does not know that `b` borrows `r`, so the
/// Rust target moves `r` into the call while `b` is still read after it:
/// rustc refuses it with E0505 ("cannot move out of `r` because it is
/// borrowed"). At 1dfc2937 the same program copied `r` (`r.clone()`) and
/// compiled; the evaluator and the LLVM target give the values asserted.
const LENT_LET_LATE: &str = r#"package lent_let_late

type Ctx {
  label: Text
  rows: List<Text>
  count: Int
}

fn ctx(n: Int) -> Ctx = Ctx { label = "ctx", rows = ["a", "b"], count = n }

fn consume(r: Ctx, a: Int, c: Int) -> Int {
  var o = r
  o.label = "consumed"
  o.rows = []
  o.count + a * 10 + c
}

fn call() -> Int {
  let r = ctx(2)
  let b = r.label.to_bytes()
  let n = consume(r, b.len(), r.count + 1)
  n * 100 + b.len()
}

fn receiver() -> Int {
  let r = ctx(3)
  let b = r.label.to_bytes()
  let n = r.consume(b.len(), r.count + 2)
  n * 100 + b.len()
}

fn in_loop() -> Int {
  var acc = 0
  var i = 0
  while i < 3 {
    let r = ctx(i)
    let b = r.label.to_bytes()
    acc = acc + consume(r, b.len(), r.rows.len()) * 10 + b.len()
    i = i + 1
  }
  acc
}

fn plain() -> Int {
  let r = ctx(4)
  let b = r.label.to_bytes()
  let n = consume(r, b.len(), 1)
  n * 100 + b.len()
}

test "a lent let read after a call that moves its record late" {
  assert call() == 3503
}

test "a lent let read after a call that moves its record" {
  assert plain() == 3503
}

test "a lent let read after a receiver that moves late" {
  assert receiver() == 3803
}

test "a lent let read after a late move in a loop" {
  assert in_loop() == 999
}
"#;

#[test]
fn adversary_a_lent_let_alive_across_a_late_move_compiles_on_the_rust_target() {
    agree("lent_let_late", LENT_LET_LATE, 4);
}

/// A `match` on a tuple of places whose arm binds the whole tuple to one
/// name (`pair => pair_size(pair)`). `Generator::lent_tuple` matches
/// `(&a, &b)`, so `pair` is a `(&Val, &Val)`, while the arm reads it as a
/// `(Val, Val)` (passed by reference as `&(Val, Val)`, or compared with
/// one): rustc refuses it with E0308. At 1dfc2937 the tuple was a tuple of
/// copies and compiled; the evaluator and the LLVM target give the values
/// asserted.
const WHOLE_TUPLE: &str = r#"package whole_tuple

enum Val {
  Nil
  Num(Int)
  Txt(Text)
}

fn size(v: Val) -> Int = match v {
  Nil => 0
  Num(n) => n
  Txt(t) => t.len()
}

fn pair_size(p: (Val, Val)) -> Int = size(p.0) * 10 + size(p.1)

fn local() -> Int {
  let a = Txt("ab")
  let b = Num(3)
  match (a, b) {
    (Num(x), Num(y)) => x + y
    other => pair_size(other)
  }
}

fn params(a: Val, b: Val) -> Int = match (a, b) {
  (Txt(x), Txt(y)) => x.len() + y.len()
  other => pair_size(other)
}

fn pair_eq(a: Val, b: Val) -> Bool = match (a, b) {
  (Nil, Nil) => true
  pair => pair == (a, b)
}

test "a catch-all binding of a tuple of owned places" {
  assert local() == 23
}

test "a catch-all binding of a tuple of borrowed parameters" {
  assert params(Txt("abc"), Num(4)) == 34
  assert params(Txt("a"), Txt("bc")) == 3
}

test "a catch-all binding of the tuple compared" {
  assert pair_eq(Num(1), Txt("x"))
}
"#;

#[test]
fn adversary_a_binding_of_the_whole_tuple_scrutinee_compiles_on_the_rust_target() {
    agree("whole_tuple", WHOLE_TUPLE, 3);
}

/// One program per guard of the change that no value or compile test of
/// the unit decides (`borrowed_reads::` in `ignite-rustgen` and `ignite`,
/// `copies::` in `ignite-llvm`, `moves::` in `ignite-types`, the adversary
/// pass before): each callee changes its record in place, so a place moved
/// while something else still reads it shows the change on the LLVM
/// target, and the Rust target refuses the move. Green at bf21b392; each
/// went red with one guard switched off in a scratch copy of the
/// implementation, all four suites above staying green: in `late_moves`
/// the `dead` check (read after the call: E0382, 40227), `held_overlaps`
/// (held by an enclosing call: E0505, 2704), `pinned` (a `match` on an
/// element of it: E0505, 1499) and the check of the operands before it (an
/// earlier argument lends a field: E0505, 551); the hold of a late operand
/// while the later ones run (`Operand::Move if late[j]`; only the name pin
/// `moves::places_move_through_fields_only` saw it: E0382, 62);
/// `take_late`'s `own` (a borrowed parameter moved late behind `?`: 707 on
/// LLVM); `Evaluator::late_args`' `is_ok` (`continue` in a later argument:
/// "`()` has no field `count`"); on LLVM `lent_tuple`'s `changed_in` (443)
/// and `moved_in` (2043).
const GUARDS: &str = r#"package call_move_guards

enum Val {
  Nil
  Num(Int)
  Txt(Text)
}

type Row {
  k: Text
  v: Val
}

type Ctx {
  label: Text
  inputs: List<(Text, Val)>
  rows: List<Row>
  count: Int
}

fn ctx(m: Int) -> Ctx {
  var rows: List<Row> = []
  var i = 0
  while i < m {
    rows.push(Row { k = "r{i}", v = Txt("t{i}") })
    i = i + 1
  }
  Ctx { label = "ctx", inputs = [("b", Num(7)), ("t", Txt("x"))], rows = rows, count = 0 }
}

fn size(v: Val) -> Int = match v {
  Nil => 0
  Num(n) => n
  Txt(t) => t.len()
}

fn set_in(r: Ctx, name: Text, v: Val) -> Ctx {
  var o = r
  o.inputs.push((name, v))
  o.count = o.count + 1
  o
}

fn read_in(r: Ctx, name: Text) -> Result<Val, Text> {
  for e in r.inputs {
    if e.0 == name {
      return Ok(e.1)
    }
  }
  Err("no input {name}")
}

fn tally(r: Ctx) -> Int = r.count * 1000 + r.rows.len() * 10 + r.inputs.len()

fn weigh(rows: List<Row>, n: Int) -> Int = rows.len() * 1000 + rows[0].k.len() * 100 + n

fn overwrite_rows(r: Ctx, n: Int) -> Int {
  var o = r
  o.rows[0] = Row { k = "zzzzzzz", v = Nil }
  o.rows.push(Row { k = "y", v = Nil })
  o.count + n + o.rows.len()
}

fn held_by_outer() -> Int {
  let r = ctx(2)
  weigh(r.rows, overwrite_rows(r, r.count + 1))
}

fn read_after() -> Int {
  let r = ctx(2)
  let n = overwrite_rows(r, r.count + 1)
  n * 10000 + tally(r) * 10 + r.rows[0].k.len()
}

fn overwrite_inputs(r: Ctx, n: Int) -> Int {
  var o = r
  o.inputs[0] = ("zzzz", Num(99))
  o.count + n
}

fn pinned_by_match() -> Int {
  let r = ctx(1)
  match r.inputs[0] {
    (k, v) => overwrite_inputs(r, r.count + 1) * 1000 + k.len() * 100 + size(v)
  }
}

fn rows_then(rows: List<Row>, r: Ctx, n: Int) -> Int {
  var o = r
  o.rows[0] = Row { k = "zzzzz", v = Nil }
  rows[0].k.len() * 100 + o.rows[0].k.len() * 10 + n
}

fn earlier_mention() -> Int {
  let r = ctx(2)
  rows_then(r.rows, r, r.count + 1)
}

fn take_rows(rows: List<Row>) -> Int {
  var o = rows
  o[0] = Row { k = "zzzzzz", v = Nil }
  o.len()
}

fn first_len(r: Ctx, n: Int) -> Int {
  var o = r
  o.count = n
  o.rows[0].k.len() * 10 + o.count
}

fn later_takes_a_field() -> Int {
  let r = ctx(2)
  first_len(r, take_rows(r.rows))
}

fn set_first(r: Ctx, v: Val) -> Ctx {
  var o = r
  o.rows[0] = Row { k = "changed", v = v }
  o
}

fn try_set(r: Ctx) -> Result<Ctx, Text> = Ok(set_first(r, read_in(r, "b")?))

fn param_behind_try() -> Int {
  let r = ctx(2)
  let n = match try_set(r) {
    Ok(x) => x.rows[0].k.len()
    Err(_) => 0
  }
  n * 100 + r.rows[0].k.len()
}

fn continue_in_later() -> Int {
  var r = ctx(1)
  var i = 0
  while i < 4 {
    i = i + 1
    r = set_in(r, "i{i}", if i == 2 {
      continue
    } else {
      Num(r.count + i)
    })
  }
  tally(r) * 10 + i
}

fn arm_overwrites() -> Int {
  var l = ["ab", "c"]
  let v = Num(3)
  match (l, v) {
    (rows, Num(n)) => {
      l[0] = "zzzz"
      rows[0].len() * 100 + l[0].len() * 10 + n
    }
    _ => 0
  }
}

fn overwrite_first(l: List<Text>) -> Int {
  var o = l
  o[0] = "zzzz"
  o.len()
}

fn arm_moves_into_change() -> Int {
  let l = ["ab", "c"]
  let v = Num(3)
  match (l, v) {
    (rows, Num(n)) => overwrite_first(l) * 1000 + rows[0].len() * 10 + n
    _ => 0
  }
}

fn patterns(a: Val, l: List<Int>) -> Int = match (a, l) {
  (Txt("x"), [k]) => k
  (Txt(t), []) => t.len() * 10
  _ => -1
}

fn patterns_local(m: Int) -> Int {
  let a = Txt("x")
  let l = [m]
  match (a, l) {
    (Txt("x"), [k]) => k
    _ => -1
  }
}

test "a place an enclosing call holds is not moved late" {
  assert held_by_outer() == 2204
}

test "a place read after the call is not moved late" {
  assert read_after() == 40222
}

test "a place a match pins is not moved late" {
  assert pinned_by_match() == 1107
}

test "a place an earlier argument lends is not moved late" {
  assert earlier_mention() == 251
}

test "a field of the late place is copied into a later argument" {
  assert later_takes_a_field() == 22
}

test "a borrowed parameter moved late behind a question mark stays the caller's" {
  assert param_behind_try() == 702
}

test "continue in a later argument keeps the variable" {
  assert continue_in_later() == 30154
}

test "an arm that overwrites an element of a tuple place" {
  assert arm_overwrites() == 243
}

test "an arm that moves a tuple place into a change" {
  assert arm_moves_into_change() == 2023
}

test "text and list patterns in a tuple" {
  assert patterns(Txt("x"), [5]) == 5
  assert patterns(Txt("abc"), []) == 30
  assert patterns(Nil, []) == -1
  assert patterns_local(4) == 4
}
"#;

#[test]
fn adversary_guard_each_guard_of_a_late_move_or_a_lent_tuple_decides_one_program() {
    agree("call_move_guards", GUARDS, 10);
}
