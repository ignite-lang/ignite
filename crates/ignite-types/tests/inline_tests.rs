//! Slice-8 tests through the public API (`docs/slice.md` section 12): the
//! test list, the operands and display of `assert a == b`, and `?` in test
//! bodies, as the backends read them.

use crate::common;

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_syntax::ast::{Decl, Expr, ExprKind, Program, StmtKind};
use ignite_types::{
    AssertCompare, Builtin, QualName, Show, TestInfo, TestTry, Ty, TypedModules, TypedProgram,
    type_module,
};

struct Typed {
    source: SourceFile,
    program: Box<Program>,
}

impl Typed {
    fn new(text: &str) -> Typed {
        let source = SourceFile::new("inline.ignite", text.to_string());
        let program = ignite_syntax::parse(&source)
            .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
        Typed {
            source,
            program: Box::new(program),
        }
    }

    fn typed(&self) -> TypedProgram {
        let symbols = check_module(&self.program, &Modules::new())
            .unwrap_or_else(|d| panic!("check failed:\n{}", self.source.render_all(&d)));
        type_module(&self.program, &symbols, &TypedModules::new())
            .unwrap_or_else(|d| panic!("type failed:\n{}", self.source.render_all(&d)))
    }

    fn find(&self, text: &str) -> &Expr {
        common::expressions(&self.program)
            .into_iter()
            .find(|e| &self.source.text[e.span.start..e.span.end] == text)
            .unwrap_or_else(|| panic!("no expression `{text}`"))
    }
}

const SOURCE: &str = r#"package demo

enum Event { Nothing, Word(Text) }

type Pair { a: Int, b: Text }

fn to_text(pair: Pair) -> Text = "{pair.a}{pair.b}"

fn word() -> Event = Word("w")

fn parse(text: Text) -> Result<Int, IoError> = match Int.parse(text) {
  Some(n) => Ok(n)
  None => Err(IoError.new("bad"))
}

fn plain() -> Result<Int, IoError> {
  let n = parse("1")?
  assert n == 1
  return Ok(n)
}

test "first" {
  assert word() != Nothing
  assert 1 + 1 == 2
  assert Pair { a = 1, b = "x" } == Pair { a = 1, b = "x" }
  assert "a" == "a"
  assert b"ab" == b"ab"
  assert Int.parse("1") == Some(1)
  assert true
}

test "second" {
  let a = parse("2")?
  let b = Int.parse("3")?
  assert a < b
}
"#;

#[test]
fn tests_are_listed_in_declaration_order() {
    let t = Typed::new(SOURCE);
    let typed = t.typed();
    let decls: Vec<usize> = t
        .program
        .decls
        .iter()
        .enumerate()
        .filter(|(_, d)| matches!(d, Decl::Test(_)))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        typed.tests(),
        [
            TestInfo {
                module: "demo".to_string(),
                name: "first".to_string(),
                full_name: "demo/first".to_string(),
                decl: decls[0],
            },
            TestInfo {
                module: "demo".to_string(),
                name: "second".to_string(),
                full_name: "demo/second".to_string(),
                decl: decls[1],
            },
        ]
    );
}

#[test]
fn a_program_without_package_is_main() {
    let t = Typed::new("test \"t\" {\n  assert true\n}\n");
    assert_eq!(t.typed().tests()[0].full_name, "main/t");
}

#[test]
fn assert_compare_records_operand_and_show() {
    let t = Typed::new(SOURCE);
    let typed = t.typed();
    let compare = |text: &str| typed.assert_compare(t.find(text)).cloned();
    let event = Ty::Enum(QualName::new("demo", "Event"));
    assert_eq!(
        compare("word() != Nothing"),
        Some(AssertCompare {
            operand: event,
            show: Some(Show::Debug),
        })
    );
    assert_eq!(
        compare("1 + 1 == 2"),
        Some(AssertCompare {
            operand: Ty::Int,
            show: Some(Show::Builtin(Builtin::IntToTextMethod)),
        })
    );
    assert_eq!(
        compare(r#"Pair { a = 1, b = "x" } == Pair { a = 1, b = "x" }"#),
        Some(AssertCompare {
            operand: Ty::Record(QualName::new("demo", "Pair")),
            show: Some(Show::Fn(QualName::new("demo", "to_text"))),
        })
    );
    assert_eq!(
        compare(r#""a" == "a""#).and_then(|c| c.show),
        Some(Show::Builtin(Builtin::TextToText))
    );
    assert_eq!(
        compare(r#"b"ab" == b"ab""#).and_then(|c| c.show),
        Some(Show::Builtin(Builtin::BytesToText))
    );
    assert_eq!(
        compare(r#"Int.parse("1") == Some(1)"#),
        Some(AssertCompare {
            operand: Ty::Option(Box::new(Ty::Int)),
            show: Some(Show::Debug),
        })
    );
    assert_eq!(compare("true"), None);
    assert_eq!(compare("n == 1").map(|c| c.operand), Some(Ty::Int));
    assert_eq!(typed.type_of(t.find("word() != Nothing")), &Ty::Bool);
}

#[test]
fn test_try_covers_only_test_bodies() {
    let t = Typed::new(SOURCE);
    let typed = t.typed();
    let tries: Vec<&Expr> = common::expressions(&t.program)
        .into_iter()
        .filter(|e| matches!(e.kind, ExprKind::Try(_)))
        .collect();
    let text = |e: &Expr| &t.source.text[e.span.start..e.span.end];
    let found: Vec<(&str, Option<TestTry>)> = tries
        .iter()
        .map(|e| (text(e), typed.test_try(e).cloned()))
        .collect();
    assert_eq!(
        found,
        [
            (r#"parse("1")?"#, None),
            (
                r#"parse("2")?"#,
                Some(TestTry {
                    error: Some(Ty::IoError),
                    show: Some(Show::IoErrorMessage),
                })
            ),
            (
                r#"Int.parse("3")?"#,
                Some(TestTry {
                    error: None,
                    show: None,
                })
            ),
        ]
    );
    assert_eq!(typed.type_of(tries[2]), &Ty::Int);
}

#[test]
fn assert_message_is_typed() {
    let t = Typed::new("fn f(x: Int) {\n  assert x > 0, \"x is {x}\"\n}\n");
    let typed = t.typed();
    let Decl::Fn(f) = &t.program.decls[0] else {
        panic!("expected a fn")
    };
    let StmtKind::Assert {
        message: Some(message),
        ..
    } = &f.body.stmts[0].kind
    else {
        panic!("expected an assert with a message")
    };
    assert_eq!(typed.type_of(message), &Ty::Text);
}
