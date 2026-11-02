//! The slice-9 frontend contract through the public API (`docs/slice.md`
//! section 13): closures, range bounds, boxing, equality, debug text,
//! associated fns and glob imports.

use crate::common;

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_syntax::ast::{Expr, ExprKind, Pattern, PatternKind, Program};
use ignite_types::{
    Builtin, Callee, Equality, QualName, Show, Ty, TypedModules, TypedProgram, type_module,
};

const SOURCE: &str = r#"
type Todo {
  id: Int
  title: Text
}

enum Tree<T> {
  Leaf,
  Node(T, Tree<T>),
}

type Chain {
  value: Int
  next: Option<Chain>
}

type Outer {
  inner: Inner
}

enum Inner {
  Wrap(Outer),
  Empty,
}

type Holder {
  run: fn(Int) -> Int
}

const LOW: Byte = 48
const HIGH: Byte = LOW

fn Todo.new(id: Int) -> Todo = Todo { id = id, title = "" }

fn digit(b: Byte) -> Int {
  return match b {
    LOW..=57 => 1
    b'a'..=b'f' => 2
    -0..=HIGH => 3
    _ => 0
  }
}

fn sum(base: Int, step: Int) -> Int {
  let add = |x: Int| x + base + step + base
  return add(1)
}

fn writer(conn: TcpStream) -> Result<Unit, IoError> requires [conn.write] {
  let send = |b: Bytes| conn.write_all(b)
  return send([1])
}

fn shout() -> Result<Unit, IoError> requires [stdout.write] {
  let say = |t: Text| Stdout.write_all(t.to_bytes())
  return say("x")
}

fn show(todo: Todo) -> Text = todo.debug()

fn make() -> Todo = Todo.new(1)
"#;

struct Typed {
    source: SourceFile,
    program: Box<Program>,
    typed: TypedProgram,
}

fn typed() -> Typed {
    let source = SourceFile::new("slice9.ignite", SOURCE.to_string());
    let program = Box::new(
        ignite_syntax::parse(&source)
            .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d))),
    );
    let symbols = check_module(&program, &Modules::new())
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let typed = type_module(&program, &symbols, &TypedModules::new())
        .unwrap_or_else(|d| panic!("type failed:\n{}", source.render_all(&d)));
    Typed {
        source,
        program,
        typed,
    }
}

impl Typed {
    fn find(&self, text: &str) -> &Expr {
        common::expressions(&self.program)
            .into_iter()
            .find(|e| &self.source.text[e.span.start..e.span.end] == text)
            .unwrap_or_else(|| panic!("no expression `{text}`"))
    }

    fn ranges(&self) -> Vec<&Pattern> {
        let mut out = Vec::new();
        for e in common::expressions(&self.program) {
            if let ExprKind::Match { arms, .. } = &e.kind {
                for arm in arms {
                    if let PatternKind::Range { .. } = arm.pattern.kind {
                        out.push(&arm.pattern);
                    }
                }
            }
        }
        out
    }
}

fn q(name: &str) -> QualName {
    QualName::new("main", name)
}

#[test]
fn every_expression_is_typed() {
    let t = typed();
    for e in common::expressions(&t.program) {
        assert!(
            t.typed.try_type_of(e).is_some(),
            "untyped `{}`",
            &t.source.text[e.span.start..e.span.end]
        );
    }
}

#[test]
fn range_bounds_evaluate_constants() {
    let t = typed();
    let bounds: Vec<_> = t
        .ranges()
        .into_iter()
        .map(|p| t.typed.range_bounds(p))
        .collect();
    assert_eq!(bounds, [Some((48, 57)), Some((97, 102)), Some((0, 48))]);
}

#[test]
fn closure_captures_in_order_of_first_use() {
    let t = typed();
    let closure = t.find("|x: Int| x + base + step + base");
    let info = t.typed.closure(closure).expect("closure info");
    assert_eq!(info.params, [("x".to_string(), Ty::Int)]);
    assert_eq!(info.ret, Ty::Int);
    let names: Vec<_> = info.captures.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["base", "step"]);
    assert!(info.effects.is_none());
    assert_eq!(
        t.typed.type_of(closure),
        &Ty::Fn(vec![Ty::Int], Box::new(Ty::Int))
    );
}

#[test]
fn closure_effects_on_captures_and_ambient() {
    let t = typed();
    let send = t
        .typed
        .closure(t.find("|b: Bytes| conn.write_all(b)"))
        .expect("closure info");
    assert_eq!(send.captures.len(), 1);
    assert_eq!(send.captures[0].effects, ["conn.write"]);
    let effects = send.effects.as_ref().expect("effectful");
    assert_eq!(effects.requires, ["conn.write"]);
    let say = t
        .typed
        .closure(t.find("|t: Text| Stdout.write_all(t.to_bytes())"))
        .expect("closure info");
    assert!(say.captures.is_empty());
    assert_eq!(
        say.effects.as_ref().expect("effectful").requires,
        ["stdout.write"]
    );
}

#[test]
fn boxing() {
    let t = typed();
    assert!(t.typed.boxed_payload(&q("Tree"), 1, 1));
    assert!(!t.typed.boxed_payload(&q("Tree"), 1, 0));
    assert!(t.typed.boxed_field(&q("Chain"), "next"));
    assert!(!t.typed.boxed_field(&q("Chain"), "value"));
    assert!(t.typed.boxed_field(&q("Outer"), "inner"));
    assert!(t.typed.boxed_payload(&q("Inner"), 0, 0));
    assert!(!t.typed.boxed_field(&q("Holder"), "run"));
    assert!(!t.typed.boxed_field(&q("Todo"), "title"));
}

#[test]
fn equality_kinds() {
    let t = typed();
    let eq = |ty: Ty| t.typed.equality(&ty);
    assert_eq!(eq(Ty::Int), Some(Equality::Scalar));
    assert_eq!(eq(Ty::Text), Some(Equality::Text));
    assert_eq!(eq(Ty::Record(q("Todo"))), Some(Equality::Structural));
    assert_eq!(
        eq(Ty::GenericEnum(q("Tree"), vec![Ty::Int])),
        Some(Equality::Structural)
    );
    assert_eq!(eq(Ty::Record(q("Holder"))), None);
    assert_eq!(eq(Ty::Option(Box::new(Ty::IoError))), None);
    assert_eq!(eq(Ty::List(Box::new(Ty::Param("T".to_string())))), None);
}

#[test]
fn debug_and_associated_fns() {
    let t = typed();
    assert_eq!(
        t.typed.callee(t.find("todo.debug()")),
        Some(&Callee::Builtin(Builtin::Debug))
    );
    assert_eq!(
        t.typed.callee(t.find("Todo.new(1)")),
        Some(&Callee::Fn(q("Todo.new")))
    );
    assert!(t.typed.exports().fns.contains_key("Todo.new"));
    let _ = Show::Debug;
}
