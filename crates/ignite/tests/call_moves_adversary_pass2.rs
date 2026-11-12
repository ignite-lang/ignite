//! Adversary, second pass (story `std-mysql-client`, unit `call-moves`):
//! only what the correction round added (34d212cc, 16da8b69). Programs run
//! by the evaluator and the targets as `call_moves_adversary` runs them.

use crate::borrowed_reads_adversary::{evaluator_failures, source, target_failure};

/// A tuple scrutinee item that holds the only sender of a channel through a
/// fn value: a record with a fn field, a list or option of fn values, a
/// variant whose payload is one. Each fn value captured the sender in a
/// callee, so it is the sender's only holder. Moved into the `match` at its
/// last use, the item is dropped with the tuple at the end of the `match`
/// and the channel is closed right after it: the evaluator says 51 for
/// each. `TypedProgram::holds_handles` does not look into a fn value's
/// captures, so the Rust target's `lent_tuple` still lends these tuples
/// (`match (&h, &v)`) and the item lives to the end of its scope: 50. At
/// 21bc8eda, before the unit, the Rust target passed all four (a tuple of
/// the places themselves, moved). The LLVM target never releases a
/// handle-holding place moved into a tuple scrutinee (`docs/llvm.md`
/// section 4, the known gap; 50 at 21bc8eda too), which this test holds as
/// it is, as `borrowed_reads::a_record_holding_a_sender_moved_into_a_tuple_match_is_dropped_with_it` does.
const FN_HOLDERS: &str = r#"package fn_holders

enum Val {
  Nil
  Num(Int)
}

type Hook {
  f: fn(Int) -> Bool
  n: Int
}

enum Slot {
  Idle
  Call(fn(Int) -> Bool)
}

fn hook_of(tx: Sender<Int>) -> fn(Int) -> Bool = |x: Int| {
  let t = tx
  x > 0
}

fn open_hook() -> (Hook, Receiver<Int>) {
  let (tx, rx) = Channel.bounded(4)
  (Hook { f = hook_of(tx), n = 2 }, rx)
}

fn open_fns() -> (List<fn(Int) -> Bool>, Receiver<Int>) {
  let (tx, rx) = Channel.bounded(4)
  ([hook_of(tx)], rx)
}

fn open_maybe() -> (Option<fn(Int) -> Bool>, Receiver<Int>) {
  let (tx, rx) = Channel.bounded(4)
  (Some(hook_of(tx)), rx)
}

fn open_slot() -> (Slot, Receiver<Int>) {
  let (tx, rx) = Channel.bounded(4)
  (Call(hook_of(tx)), rx)
}

fn score(got: Int, rx: Receiver<Int>) -> Int = if rx.closed() {
  got * 10 + 1
} else {
  got * 10
}

fn hook_item() -> Int {
  let (h, rx) = open_hook()
  let v = Num(3)
  let got = match (h, v) {
    (x, Num(k)) => x.n + k
    _ => 0
  }
  score(got, rx)
}

fn fns_item() -> Int {
  let (l, rx) = open_fns()
  let v = Num(3)
  let got = match (l, v) {
    (fs, Num(k)) => fs.len() + k + 1
    _ => 0
  }
  score(got, rx)
}

fn maybe_item() -> Int {
  let (o, rx) = open_maybe()
  let v = Num(3)
  let got = match (o, v) {
    (Some(_), Num(k)) => k + 2
    _ => 0
  }
  score(got, rx)
}

fn slot_item() -> Int {
  let (s, rx) = open_slot()
  let v = Num(3)
  let got = match (s, v) {
    (Call(_), Num(k)) => k + 2
    _ => 0
  }
  score(got, rx)
}

test "a record whose fn field holds the only sender" {
  assert hook_item() == 51
}

test "a list of fn values holding the only sender" {
  assert fns_item() == 51
}

test "an option of a fn value holding the only sender" {
  assert maybe_item() == 51
}

test "a variant whose payload is a fn value holding the only sender" {
  assert slot_item() == 51
}
"#;

#[test]
fn adversary2_a_sender_held_by_a_fn_value_is_dropped_with_a_tuple_match() {
    let file = source("fn_holders", FN_HOLDERS);
    let evaluator = evaluator_failures(&file);
    assert!(evaluator.is_empty(), "evaluator:\n{}", evaluator.join("\n"));
    if let Some(failure) = target_failure(&file, "rust", 4) {
        panic!("{failure}");
    }
    let llvm = target_failure(&file, "llvm", 4).expect("the LLVM gap is closed: update this test");
    assert_eq!(llvm.matches("left: 50").count(), 4, "{llvm}");
}

/// A lent `let`'s root moved in a statement that does not read the bytes
/// but comes before one that does (`Generator::lent_let`: a statement
/// "comes before one that does", the `read |=` latch of the reverse scan).
/// Every program of the correction round moves the root in the statement
/// that reads `b`, so the latch is decided by none of them: with `read =`
/// in place of `read |=` (a scratch copy) the Rust target lends `b` across
/// the move and rustc refuses E0505.
const MOVED_BEFORE_READ: &str = r#"package moved_before_read

type Ctx {
  label: Text
  rows: List<Text>
  count: Int
}

fn ctx(n: Int) -> Ctx = Ctx { label = "ctx", rows = ["a", "b"], count = n }

fn consume(r: Ctx, a: Int) -> Int {
  var o = r
  o.label = "consumed"
  o.rows = []
  o.count + a * 10
}

fn moved_then_read() -> Int {
  let r = ctx(2)
  let b = r.label.to_bytes()
  let n = consume(r, 1)
  n * 100 + b.len()
}

fn returned_early(c: Bool) -> Int {
  let r = ctx(2)
  let b = r.label.to_bytes()
  if c {
    return consume(r, 1)
  }
  b.len()
}

fn moved_in_a_branch(c: Bool) -> Int {
  let r = ctx(2)
  let b = r.label.to_bytes()
  var n = 0
  if c {
    n = consume(r, 1)
  }
  n * 100 + b.len()
}

test "a root moved before the statement that reads the bytes" {
  assert moved_then_read() == 1203
  assert returned_early(true) == 12
  assert returned_early(false) == 3
  assert moved_in_a_branch(true) == 1203
  assert moved_in_a_branch(false) == 3
}
"#;

#[test]
fn adversary2_a_lent_let_whose_root_moves_before_its_read_compiles_on_the_rust_target() {
    crate::borrowed_reads_adversary::agree("moved_before_read", MOVED_BEFORE_READ, 1);
}

/// `TypedProgram::holds_handles` looks through a list, an option, an enum
/// payload, a generic record's type argument, a record inside a record and
/// a type parameter; the correction round's programs reach only a record
/// holding a sender and a channel end itself, so each of those branches is
/// decided by no test (switched off one at a time in a scratch copy, every
/// suite stays green). Each item here holds the only sender of a channel
/// and is moved into the `match` at its last use: the evaluator and the
/// Rust target close the channel at the end of the `match` (51). The LLVM
/// target's gap (`docs/llvm.md` section 4) is held as it is (50, also at
/// 21bc8eda).
const HANDLE_BRANCHES: &str = r#"package handle_branches

enum Val {
  Nil
  Num(Int)
}

type Holder<T> {
  item: T
  n: Int
}

type Link {
  tx: Sender<Int>
  n: Int
}

type Deep {
  link: Link
  m: Int
}

type Deeper {
  deep: Deep
}

enum Carry {
  Empty
  Has(Sender<Int>)
}

fn open_holder() -> (Holder<Sender<Int>>, Receiver<Int>) {
  let (tx, rx) = Channel.bounded(4)
  (Holder { item = tx, n = 2 }, rx)
}

fn open_option() -> (Option<Sender<Int>>, Receiver<Int>) {
  let (tx, rx) = Channel.bounded(4)
  (Some(tx), rx)
}

fn open_list() -> (List<Sender<Int>>, Receiver<Int>) {
  let (tx, rx) = Channel.bounded(4)
  ([tx], rx)
}

fn open_carry() -> (Carry, Receiver<Int>) {
  let (tx, rx) = Channel.bounded(4)
  (Has(tx), rx)
}

fn open_deeper() -> (Deeper, Receiver<Int>) {
  let (tx, rx) = Channel.bounded(4)
  (Deeper { deep = Deep { link = Link { tx = tx, n = 2 }, m = 0 } }, rx)
}

fn open_link() -> (Link, Receiver<Int>) {
  let (tx, rx) = Channel.bounded(4)
  (Link { tx = tx, n = 2 }, rx)
}

fn score(got: Int, rx: Receiver<Int>) -> Int = if rx.closed() {
  got * 10 + 1
} else {
  got * 10
}

fn generic_holder() -> Int {
  let (h, rx) = open_holder()
  let v = Num(3)
  let got = match (h, v) {
    (x, Num(k)) => x.n + k
    _ => 0
  }
  score(got, rx)
}

fn in_option() -> Int {
  let (o, rx) = open_option()
  let v = Num(3)
  let got = match (o, v) {
    (Some(_), Num(k)) => k + 2
    _ => 0
  }
  score(got, rx)
}

fn in_list() -> Int {
  let (l, rx) = open_list()
  let v = Num(3)
  let got = match (l, v) {
    (xs, Num(k)) => xs.len() + k + 1
    _ => 0
  }
  score(got, rx)
}

fn in_enum() -> Int {
  let (c, rx) = open_carry()
  let v = Num(3)
  let got = match (c, v) {
    (Has(_), Num(k)) => k + 2
    _ => 0
  }
  score(got, rx)
}

fn two_down() -> Int {
  let (d, rx) = open_deeper()
  let v = Num(3)
  let got = match (d, v) {
    (x, Num(k)) => x.deep.link.n + k
    _ => 0
  }
  score(got, rx)
}

fn generic_param<T>(x: T, v: Val, closed: fn() -> Bool requires [channel.*]) -> Int {
  let got = match (x, v) {
    (_, Num(k)) => k + 2
    _ => 0
  }
  if closed() {
    got * 10 + 1
  } else {
    got * 10
  }
}

fn through_generic() -> Int {
  let (l, rx) = open_link()
  generic_param(l, Num(3), || rx.closed())
}

test "a generic record holding a sender" {
  assert generic_holder() == 51
}

test "an option holding a sender" {
  assert in_option() == 51
}

test "a list holding a sender" {
  assert in_list() == 51
}

test "an enum payload holding a sender" {
  assert in_enum() == 51
}

test "a record two levels down holding a sender" {
  assert two_down() == 51
}

test "a type parameter holding a sender" {
  assert through_generic() == 51
}
"#;

#[test]
fn adversary2_each_branch_of_holds_handles_closes_the_channel_on_the_rust_target() {
    let file = source("handle_branches", HANDLE_BRANCHES);
    let evaluator = evaluator_failures(&file);
    assert!(evaluator.is_empty(), "evaluator:\n{}", evaluator.join("\n"));
    if let Some(failure) = target_failure(&file, "rust", 6) {
        panic!("{failure}");
    }
    let llvm = target_failure(&file, "llvm", 6).expect("the LLVM gap is closed: update this test");
    assert_eq!(llvm.matches("left: 50").count(), 6, "{llvm}");
}

/// Conditions of the correction round that only save a copy, which no
/// value test can see, each switched off or inverted in a scratch copy
/// with every suite green: `lent_let` lends `b` when its root moves only
/// after the last statement that reads it (the `read &&` of the reverse
/// scan; `docs/slice.md` 20.3.7: "in no statement that reads `b` or comes
/// before one that does"); `match_expr` copies a whole-tuple binding only
/// when the arm reads it (`ownership::mentions`; "when its body reads the
/// name"); `holds_handles` looks at a generic record's type arguments, not
/// at the type parameters of its fields (`holds_handles_in`'s `param`); and
/// `Walk::late_moves` moves a place late when its only later operand
/// names it (`Walk::names`: with the test inverted, every program of the
/// unit still moves late, since each has a later operand that does not
/// name the place, `consume(r, b.len(), r.count + 1)`, `set_in(r, "k",
/// Num(r.count))`).
const COPIES_SAVED: &str = r#"package copies_saved

type Ctx {
  label: Text
  rows: List<Text>
  count: Int
}

fn ctx(n: Int) -> Ctx = Ctx { label = "ctx", rows = ["a", "b"], count = n }

fn consume(r: Ctx, a: Int) -> Int {
  var o = r
  o.label = "consumed"
  o.count + a * 10
}

fn moved_after_read() -> Int {
  let r = ctx(2)
  let b = r.label.to_bytes()
  let s = b.len()
  s * 100 + consume(r, 1)
}

fn late_only() -> Int {
  let r = ctx(2)
  consume(r, r.count + 1)
}

enum V {
  N
  S(Text)
}

fn unread_whole(a: V, b: V) -> Int = match (a, b) {
  (S(x), S(y)) => x.len() + y.len()
  other => 0
}

type Holder<T> {
  item: T
  n: Int
}

fn holder_pair(h: Holder<Int>, v: V) -> Int = match (h, v) {
  (x, S(t)) => x.item + x.n + t.len()
  _ => 0
}

test "the values" {
  assert moved_after_read() == 312
  assert late_only() == 32
  assert unread_whole(S("ab"), S("c")) == 3
  assert unread_whole(N, S("c")) == 0
  assert holder_pair(Holder { item = 1, n = 2 }, S("abc")) == 6
}
"#;

#[test]
fn adversary2_the_copies_the_correction_round_saves_stay_saved() {
    let file = source("copies_saved", COPIES_SAVED);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("emit")
        .arg(&file)
        .output()
        .expect("run ignite emit");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "emit: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let section = |head: &str| -> String {
        let start = text
            .find(head)
            .unwrap_or_else(|| panic!("no `{head}` in:\n{text}"));
        let rest = &text[start..];
        rest[..rest.find("\n}\n").expect("end of the fn")].to_string()
    };
    let moved_after = section("pub fn moved_after_read(");
    assert!(
        moved_after.contains("let b: &[u8] = r.label.as_bytes();"),
        "moved_after_read copies the bytes: {moved_after}"
    );
    let unread = section("pub fn unread_whole(");
    assert!(
        !unread.contains("let other = ("),
        "unread_whole copies a tuple it never reads: {unread}"
    );
    let holder = section("pub fn holder_pair(");
    assert!(
        holder.contains("match (&h, &v)"),
        "holder_pair copies a generic record whose type argument holds no handle: {holder}"
    );
    let late = section("pub fn late_only(");
    assert!(
        late.contains("consume(r, __ignite_l1)") && !late.contains("r.clone()"),
        "late_only copies the place its only later argument reads: {late}"
    );
}
