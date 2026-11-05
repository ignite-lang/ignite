//! The last-use moves analysis through the public API (`docs/slice.md`
//! 20.3, rows 1 to 5 of 20.0): `TypedProgram::moves` marks the places in
//! move positions that are dead after them, `TypedProgram::consumed` and
//! `ModuleTypes::consumed` the parameters a callee consumes.

use std::path::Path;

use crate::common;

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_syntax::ast::{Decl, Expr, Program};
use ignite_types::{Callee, QualName, TypedModules, TypedProgram, type_module};

struct Typed {
    source: SourceFile,
    program: Box<Program>,
    typed: TypedProgram,
}

/// Parse, check and type `text` as module `main` against `modules`.
fn typed_against(text: &str, symbols: &Modules, modules: &TypedModules) -> Typed {
    let source = SourceFile::new("moves.ignite", text.to_string());
    let program = Box::new(
        ignite_syntax::parse(&source)
            .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d))),
    );
    let checked = check_module(&program, symbols)
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let typed = type_module(&program, &checked, modules)
        .unwrap_or_else(|d| panic!("type failed:\n{}", source.render_all(&d)));
    Typed {
        source,
        program,
        typed,
    }
}

/// Parse, check and type `text` as module `main` without std.
fn typed(text: &str) -> Typed {
    typed_against(text, &Modules::new(), &TypedModules::new())
}

impl Typed {
    /// Every expression of the module for which `moves` is true, as
    /// (line, column) with its text, in source order.
    fn marked(&self) -> Vec<(usize, usize, String)> {
        let mut out: Vec<(usize, usize, String)> = common::expressions(&self.program)
            .into_iter()
            .filter(|e| self.typed.moves(e))
            .map(|e| {
                let (line, col) = self.source.line_col(e.span.start);
                (line, col, self.text(e).to_string())
            })
            .collect();
        out.sort();
        out
    }

    /// The text of every marked expression inside fn `name`, in source
    /// order.
    fn marked_in(&self, name: &str) -> Vec<String> {
        let span = self
            .program
            .decls
            .iter()
            .find_map(|d| match d {
                Decl::Fn(f) if f.qualified_name() == name => Some(f.span),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no fn `{name}`"));
        let mut inside: Vec<&Expr> = common::expressions(&self.program)
            .into_iter()
            .filter(|e| e.span.start >= span.start && e.span.end <= span.end)
            .filter(|e| self.typed.moves(e))
            .collect();
        inside.sort_by_key(|e| e.span.start);
        inside.iter().map(|e| self.text(e).to_string()).collect()
    }

    fn text(&self, e: &Expr) -> &str {
        &self.source.text[e.span.start..e.span.end]
    }

    fn consumed(&self, name: &str, index: usize) -> bool {
        self.typed
            .consumed(&Callee::Fn(QualName::new("main", name)), index)
    }
}

fn at(line: usize, col: usize, text: &str) -> (usize, usize, String) {
    (line, col, text.to_string())
}

fn none() -> Vec<String> {
    Vec::new()
}

/// `run` and `step` of probe p07 (`docs/slice.md` 20.1): state threaded
/// through a call.
const P07: &str = r#"type State {
  items: List<Int>
  count: Int
}

fn step(state: State, x: Int) -> State {
  var next = state
  next.items.push(x)
  next.count = next.count + 1
  next
}

fn run(n: Int) -> Int {
  var s = State { items = [], count = 0 }
  var i = 0
  while i < n {
    s = step(s, i)
    i = i + 1
  }
  s.count
}
"#;

#[test]
fn a_var_passed_and_reassigned_moves() {
    let t = typed(P07);
    assert_eq!(
        t.marked(),
        vec![at(7, 14, "state"), at(10, 3, "next"), at(17, 14, "s")]
    );
    assert!(t.consumed("step", 0));
    assert!(!t.consumed("step", 1));
}

/// The fns the other cases call: `consume`, `take` and `after` change
/// their `State` parameter in place, `step` moves it into a `var`.
const PRELUDE: &str = r#"type State {
  items: List<Int>
  count: Int
}

fn fresh() -> State = State { items = [], count = 0 }

fn consume(x: State) -> Int {
  x.items.push(1)
  x.count
}

fn take(x: State, n: Int) -> Int {
  x.items.push(n)
  x.count
}

fn after(n: Int, x: State) -> Int {
  x.items.push(n)
  x.count
}

fn merge(a: List<Int>, x: State) -> Int {
  x.items.push(1)
  a.len()
}

fn step(state: State, x: Int) -> State {
  var next = state
  next.items.push(x)
  next
}
"#;

fn with_prelude(cases: &str) -> Typed {
    typed(&format!("{PRELUDE}\n{cases}"))
}

#[test]
fn a_later_read_keeps_the_copy() {
    let t = with_prelude(
        r#"
fn later_read() -> Int {
  let s = fresh()
  let n = consume(s)
  n + s.count
}

fn next_iteration(n: Int) -> Int {
  let s = fresh()
  var total = 0
  var i = 0
  while i < n {
    total = total + consume(s)
    i = i + 1
  }
  total
}

fn other_argument() -> Int {
  let s = fresh()
  after(s.count, s)
}

fn receiver_mentions() -> Int {
  let s = fresh()
  s.items.merge(s)
}

fn assigned_in_argument(c: Bool) -> Int {
  var s = fresh()
  take(s, if c {
    s = fresh()
    0
  } else {
    0
  })
}

fn in_match_arm() -> Int {
  let s = fresh()
  match s.count {
    0 => consume(s)
    _ => 1
  }
}

fn in_for_body() -> Int {
  let s = fresh()
  var total = 0
  for x in s.items {
    if x == 3 {
      return consume(s)
    }
    total = total + x
  }
  total
}

fn changed_after() -> State {
  var s = fresh()
  let t = step(s, 1)
  s.items.push(2)
  t
}

fn branch_moves(c: Bool) -> Int {
  let s = fresh()
  if c { consume(s) } else { s.count }
}

fn shadowed(c: Bool) -> Int {
  let a = fresh()
  let b = fresh()
  var t = fresh()
  let n = consume(t)
  if c {
    var t = a
  } else {
    var t = b
  }
  n + t.count
}
"#,
    );
    for name in [
        "later_read",
        "next_iteration",
        "other_argument",
        "receiver_mentions",
        "assigned_in_argument",
        "in_match_arm",
        "in_for_body",
    ] {
        assert_eq!(t.marked_in(name), none(), "{name}");
    }
    assert_eq!(t.marked_in("changed_after"), vec!["t"]);
    assert_eq!(t.marked_in("branch_moves"), vec!["s"]);
    assert_eq!(t.marked_in("shadowed"), vec!["a", "b"]);
}

#[test]
fn every_move_position_moves() {
    let t = with_prelude(
        r#"
type Holder {
  state: State
}

enum Wrapped {
  Wrap(State),
  Empty,
}

fn through_value(f: fn(State) -> Int) -> Int {
  let s = fresh()
  f(s)
}

fn into_push() -> List<State> {
  let s = fresh()
  var xs: List<State> = []
  xs.push(s)
  xs
}

fn into_put() -> Map<Text, State> {
  let s = fresh()
  var m: Map<Text, State> = Map.empty()
  m.put("k", s)
  m
}

fn into_let() -> State {
  let s = fresh()
  var t = s
  let u = fresh()
  t = u
  t
}

fn into_return(c: Bool) -> State {
  let s = fresh()
  if c {
    return s
  }
  let t = fresh()
  let u = if c {
    let v = t
    v
  } else {
    fresh()
  }
  u
}

fn into_literals() -> Int {
  let a = fresh()
  let b = fresh()
  let c = fresh()
  let d = fresh()
  let e = fresh()
  let f = fresh()
  let g = fresh()
  let h = fresh()
  let list = [a]
  let holder = Holder { state = b }
  let updated = State { count = 1, ..c }
  let wrapped = Wrap(d)
  let some = Some(e)
  let ok: Result<State, State> = Ok(f)
  let err: Result<State, State> = Err(g)
  let pair = (h, 1)
  list.len() + holder.state.count + updated.count + pair.1
}

fn receiver_of_builtin() -> State {
  let a = fresh()
  a.copy()
}

fn scrutinee() -> Int {
  let w = Wrap(fresh())
  match w {
    Wrap(s) => s.count
    Empty => 0
  }
}

fn iterable() -> Int {
  let xs = [1, 2]
  var total = 0
  for x in xs {
    total = total + x
  }
  total
}

fn operand() -> Bool {
  let a = fresh()
  let b = fresh()
  a == b
}

fn text_operand() -> Text {
  let a = "x".concat("y")
  let b = "z".concat("w")
  a + b
}

fn worker(s: State) -> Result<Unit, IoError> requires [stdout.write] {
  s.items.push(1)
  Stdout.write_all("x".to_bytes())
}

fn launch() -> Result<Unit, IoError> requires [stdout.write] {
  let s = fresh()
  spawn worker(s)
  Ok(())
}

fn capture() -> Int {
  let s = fresh()
  let f = |x: Int| s.count + x
  f(1)
}

fn pattern_let() -> Int {
  let p = (fresh(), 1)
  let (a, n) = p
  a.count + n
}

fn asserted() -> Int {
  let a = fresh()
  assert a == fresh()
  0
}
"#,
    );
    assert_eq!(t.marked_in("through_value"), vec!["s"]);
    assert_eq!(t.marked_in("into_push"), vec!["s", "xs"]);
    assert_eq!(t.marked_in("into_put"), vec!["s", "m"]);
    assert_eq!(t.marked_in("into_let"), vec!["s", "u", "t"]);
    assert_eq!(t.marked_in("into_return"), vec!["s", "t", "v", "u"]);
    assert_eq!(
        t.marked_in("into_literals"),
        vec!["a", "b", "c", "d", "e", "f", "g", "h"]
    );
    for name in [
        "receiver_of_builtin",
        "scrutinee",
        "iterable",
        "operand",
        "text_operand",
        "launch",
        "capture",
        "pattern_let",
        "asserted",
    ] {
        assert_eq!(t.marked_in(name), none(), "{name}");
    }
}

#[test]
fn places_move_through_fields_only() {
    let t = with_prelude(
        r#"
type Pack {
  inner: State
  n: Int
}

type Chain {
  value: Int
  next: Option<Chain>
}

type Actor {
  state: State
  handle: fn(State, Int) -> State
}

type Out {
  log: List<Int>
}

type Step {
  state: State
  out: Out
}

type Table {
  items: List<Int>
}

type Server {
  table: Table
  n: Int
}

fn field_moves() -> State {
  let b = Pack { inner = fresh(), n = 1 }
  b.inner
}

fn index_never() -> State {
  let xs = [fresh()]
  xs[0]
}

fn boxed_never() -> Option<Chain> {
  let c = Chain { value = 1, next = None }
  c.next
}

fn binding_never(o: Option<State>) -> List<Int> {
  match o {
    Some(v) => v.items
    None => []
  }
}

fn handle_loop(n: Int) -> Actor {
  var a = Actor { state = fresh(), handle = step }
  var i = 0
  while i < n {
    a.state = a.handle(a.state, i)
    i = i + 1
  }
  a
}

fn publish(o: Out) -> Out {
  var p = o
  p.log.push(1)
  p
}

fn step_literal() -> Step {
  let st = Step { state = fresh(), out = Out { log = [] } }
  Step { state = st.state, out = publish(st.out) }
}

fn answer(state: Server, reply: Int, values: List<Int>) -> Server {
  var s = state
  s.n = reply + values.len()
  s
}

fn values(t: Table) -> List<Int> = t.items

fn answering() -> Server {
  let state = Server { table = Table { items = [] }, n = 0 }
  answer(state, 1, state.table.values())
}

fn answering_after() -> Server {
  let state = Server { table = Table { items = [] }, n = 0 }
  let vs = state.table.values()
  answer(state, 1, vs)
}

fn scalars(conn: TcpStream, f: fn(Int) -> Int) -> Int {
  let s = fresh()
  let n = s.count
  let flag = true
  let flag2 = flag
  let u = ()
  let u2 = u
  let d = 5 ms
  let d2 = d
  let by: Byte = b'a'
  let by2 = by
  let c = conn
  let g = f
  g(n)
}
"#,
    );
    assert_eq!(t.marked_in("field_moves"), vec!["b.inner"]);
    for name in ["index_never", "boxed_never", "binding_never", "scalars"] {
        assert_eq!(t.marked_in(name), none(), "{name}");
    }
    assert_eq!(t.marked_in("handle_loop"), vec!["a.state", "a"]);
    assert_eq!(t.marked_in("step_literal"), vec!["st.state", "st.out"]);
    assert_eq!(t.marked_in("answering"), vec!["state.table"]);
    assert_eq!(t.marked_in("answering_after"), vec!["state"]);
}

#[test]
fn consumed_is_a_fixpoint() {
    let t = with_prelude(
        r#"
type Bag {
  items: List<Int>
  n: Int
}

fn a(x: State) -> Int = b(x)

fn b(y: State) -> Int {
  y.items.push(1)
  if y.count > 10 { a(y) } else { y.count }
}

fn c(z: State) -> Int = z.count + z.items.len()

fn d(m: Map<Text, Int>) -> Int {
  m.put("k", 1)
  m.len()
}

fn sink(l: List<Int>) -> Int {
  l.push(1)
  l.len()
}

fn e(p: Bag) -> Int = sink(p.items)

fn pick(a: State, b: State, c: Bool) -> State = if c { a } else { b }
"#,
    );
    assert!(t.consumed("a", 0));
    assert!(t.consumed("b", 0));
    assert!(!t.consumed("c", 0));
    assert!(t.consumed("d", 0));
    assert!(t.consumed("e", 0));
    assert!(!t.consumed("pick", 0));
    assert!(!t.consumed("pick", 1));
    assert!(!t.consumed("pick", 2));
    assert!(t.marked_in("pick").is_empty());
    let exports = t.typed.exports();
    assert_eq!(exports.consumed.get("a"), Some(&vec![true]));
    assert_eq!(exports.consumed.get("c"), Some(&vec![false]));
    assert_eq!(
        exports.consumed.get("pick"),
        Some(&vec![false, false, false])
    );

    let std = common::load_std(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../std"))
        .unwrap_or_else(|e| panic!("std: {e}"));
    let option = &std.typed["std.option"];
    assert_eq!(option.consumed.get("unwrap_or"), Some(&vec![false, false]));
    let json = &std.typed["std.json"];
    assert_eq!(json.consumed.get("error"), Some(&vec![true, false]));
    let program = typed_against(
        r#"import std.json.{error, JsonError}

fn fail(n: Int) -> JsonError {
  let message = "bad ".concat(n.to_text())
  error(message, n)
}
"#,
        &std.modules,
        &std.typed,
    );
    assert_eq!(program.marked_in("fail"), vec!["message"]);
    assert!(
        program
            .typed
            .consumed(&Callee::Fn(QualName::new("std.json", "error")), 0)
    );
    assert!(
        !program
            .typed
            .consumed(&Callee::Fn(QualName::new("std.json", "error")), 1)
    );
}

#[test]
fn closure_bodies_never_move() {
    let t = with_prelude(
        r#"
fn closure_body() -> Int {
  let f = |x: Int| {
    let s = fresh()
    consume(s)
  }
  f(1)
}

fn capture_reads() -> Int {
  let s = fresh()
  let n = consume(s)
  let f = |x: Int| s.count + x
  n + f(1)
}
"#,
    );
    assert_eq!(t.marked_in("closure_body"), none());
    assert_eq!(t.marked_in("capture_reads"), none());
}

/// Stale point S10 of `docs/slice.md` 21.10: the five capability types are
/// handles (invariant 3), so a capability passed on at its last use is
/// shared, never moved as an aggregate.
#[test]
fn capabilities_are_handles_and_never_move() {
    let t = typed(
        r#"
fn keep(files: Directory) -> Directory = files

fn pass(files: Directory, tools: Launcher, child: ChildProcess, entropy: Entropy, tool: Setting) -> Directory {
  let t = tools
  let c = child
  let e = entropy
  let s = tool
  keep(files)
}
"#,
    );
    assert_eq!(t.marked_in("pass"), none());
}
