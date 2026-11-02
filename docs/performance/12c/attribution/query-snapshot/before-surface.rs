//! The slice-6 surface syntax through the public API (`docs/slice.md`
//! section 10.1): the types and callees the generators read for inferred
//! variants, tuples, interpolation, UFCS and fn-field calls, aliases, field
//! defaults, `return` expressions and constant patterns.

use crate::common;

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_syntax::ast::{Decl, Expr, ExprKind, PatternKind, Program, StmtKind};
use ignite_types::{Builtin, Callee, QualName, Ty, TypedModules, TypedProgram, type_module};

struct Typed {
    source: SourceFile,
    program: Box<Program>,
}

impl Typed {
    fn new(text: &str) -> Typed {
        let source = SourceFile::new("surface.ignite", text.to_string());
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

    /// The first expression whose source text is exactly `text`.
    fn find(&self, text: &str) -> &Expr {
        common::expressions(&self.program)
            .into_iter()
            .find(|e| &self.source.text[e.span.start..e.span.end] == text)
            .unwrap_or_else(|| panic!("no expression `{text}`"))
    }
}

fn main_q(name: &str) -> QualName {
    QualName::new("main", name)
}

#[test]
fn every_expression_is_typed() {
    let t = Typed::new(SOURCE);
    let typed = t.typed();
    for e in common::expressions(&t.program) {
        assert!(
            typed.try_type_of(e).is_some(),
            "untyped `{}`",
            &t.source.text[e.span.start..e.span.end]
        );
    }
}

#[test]
fn bare_variants_take_the_expected_enum() {
    let t = Typed::new(SOURCE);
    let typed = t.typed();
    let stop = t.find("Stop");
    assert_eq!(typed.type_of(stop), &Ty::Enum(main_q("Flow")));
    assert_eq!(
        typed.callee(stop),
        Some(&Callee::Variant {
            enum_: main_q("Flow"),
            index: 1
        })
    );
    let go = t.find("Go(1)");
    assert_eq!(typed.type_of(go), &Ty::Enum(main_q("Flow")));
}

#[test]
fn tuples_and_unit() {
    let t = Typed::new(SOURCE);
    let typed = t.typed();
    assert_eq!(
        typed.type_of(t.find("(t, n)")),
        &Ty::Tuple(vec![Ty::Text, Ty::Int])
    );
    assert_eq!(typed.type_of(t.find("()")), &Ty::Unit);
    let swap = fn_decl(&t.program, "swap");
    let StmtKind::LetPattern { pattern, .. } = &swap.body.stmts[0].kind else {
        panic!("expected a let pattern");
    };
    let PatternKind::Tuple(items) = &pattern.kind else {
        panic!("expected a tuple pattern");
    };
    let PatternKind::Binding(n) = &items[0].kind else {
        panic!("expected a binding");
    };
    assert_eq!(typed.binding_type(n.span), Some(&Ty::Int));
}

#[test]
fn interpolation_is_text_concatenation_with_to_text() {
    let t = Typed::new(SOURCE);
    let typed = t.typed();
    let line = t.find("\"n={n} b={b} ok={ok} t={t} raw={raw}\"");
    assert_eq!(typed.type_of(line), &Ty::Text);
    assert_eq!(typed.callee(line), None);
    let mut builtins = Vec::new();
    collect_to_text(line, &typed, &mut builtins);
    assert_eq!(
        builtins,
        [
            Builtin::IntToTextMethod,
            Builtin::ByteToText,
            Builtin::BoolToText,
            Builtin::TextToText,
            Builtin::BytesToText
        ]
    );
}

fn collect_to_text(e: &Expr, typed: &TypedProgram, out: &mut Vec<Builtin>) {
    match &e.kind {
        ExprKind::Binary { left, right, .. } => {
            assert_eq!(typed.type_of(e), &Ty::Text);
            collect_to_text(left, typed, out);
            collect_to_text(right, typed, out);
        }
        ExprKind::Method { name, .. } if name.name == "to_text" => match typed.callee(e) {
            Some(Callee::Builtin(b)) => out.push(*b),
            other => panic!("to_text callee {other:?}"),
        },
        _ => {}
    }
}

#[test]
fn ufcs_and_field_fn_calls() {
    let t = Typed::new(SOURCE);
    let typed = t.typed();
    let ufcs = t.find("n.twice()");
    assert_eq!(typed.callee(ufcs), Some(&Callee::Fn(main_q("twice"))));
    assert_eq!(typed.type_of(ufcs), &Ty::Int);
    let field = t.find("config.on_line(config.port)");
    assert_eq!(
        typed.callee(field),
        Some(&Callee::FieldFn {
            record: main_q("Config"),
            field: "on_line".to_string()
        })
    );
    assert_eq!(typed.type_of(field), &Ty::Int);
}

#[test]
fn aliases_resolve_to_their_targets() {
    let t = Typed::new(SOURCE);
    let typed = t.typed();
    let swap = fn_decl(&t.program, "swap");
    assert_eq!(
        typed.resolved_type(&swap.params[0].ty),
        Some(&Ty::Tuple(vec![Ty::Int, Ty::Text]))
    );
    let exports = typed.exports();
    let pair = &exports.aliases["Pair"];
    assert_eq!(pair.params, ["A", "B"]);
    assert_eq!(
        exports.fns["apply"].params[0],
        Ty::Fn(vec![Ty::Int], Box::new(Ty::Int))
    );
}

#[test]
fn field_defaults() {
    let t = Typed::new(SOURCE);
    let typed = t.typed();
    let config = main_q("Config");
    assert!(typed.field_has_default(&config, "port"));
    assert!(typed.field_has_default(&config, "on_line"));
    assert!(!typed.field_has_default(&config, "name"));
    assert_eq!(
        typed.exports().defaults["Config"],
        ["port".to_string(), "on_line".to_string()]
    );
    let Some(Decl::Record(record)) = t.program.decls.iter().find(|d| d.name().name == "Config")
    else {
        panic!("no record Config");
    };
    let default = record.fields[0].default.as_ref().unwrap();
    assert_eq!(typed.type_of(default), &Ty::Int);
    let on_line = record.fields[2].default.as_ref().unwrap();
    assert_eq!(typed.callee(on_line), Some(&Callee::Fn(main_q("twice"))));
}

#[test]
fn return_is_never_and_constant_patterns_are_recorded() {
    let t = Typed::new(SOURCE);
    let typed = t.typed();
    assert_eq!(typed.type_of(t.find("return 0")), &Ty::Never);
    let first = fn_decl(&t.program, "first");
    let StmtKind::Let { value, .. } = &first.body.stmts[0].kind else {
        panic!("expected a let");
    };
    let ExprKind::Match { arms, .. } = &value.kind else {
        panic!("expected a match");
    };
    assert_eq!(typed.pattern_const(&arms[0].pattern), Some(&main_q("NONE")));
    assert_eq!(typed.pattern_const(&arms[1].pattern), None);
    let capped = fn_decl(&t.program, "capped");
    let StmtKind::Let { value, .. } = &capped.body.stmts[0].kind else {
        panic!("expected a let");
    };
    let ExprKind::Match { arms, .. } = &value.kind else {
        panic!("expected a match");
    };
    assert_eq!(
        typed.pattern_const(&arms[0].pattern),
        Some(&main_q("limit")),
        "a lowercase constant name in an arm is a constant pattern"
    );
    assert_eq!(typed.pattern_const(&arms[1].pattern), None);
}

#[test]
fn inferred_effects_reach_the_signature() {
    let t = Typed::new(SOURCE);
    let exports = t.typed().exports();
    assert!(exports.fns["greet"].effectful);
    assert!(!exports.fns["twice"].effectful);
}

fn fn_decl<'p>(program: &'p Program, name: &str) -> &'p ignite_syntax::ast::FnDecl {
    program
        .decls
        .iter()
        .find_map(|d| match d {
            Decl::Fn(f) if f.name.name == name => Some(f),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no fn {name}"))
}

const SOURCE: &str = r#"
enum Action { Go(Int), Stop }
enum Flow { Go(Int), Stop }

type Handler = fn(Int) -> Int
type Pair<A, B> = (A, B)

type Config {
  port: Int = 2324
  name: Text
  on_line: Handler = twice
}

const NONE: Int = 0
const limit: Int = 3

fn twice(n: Int) -> Int = n * 2

fn apply(h: Handler, n: Int) -> Int = h(n)

fn flow(go: Bool) -> Flow {
  if go {
    return Go(1)
  }
  return Stop
}

fn swap(p: Pair<Int, Text>) -> (Text, Int) {
  let (n, t) = p
  return (t, n)
}

fn unit() {
  let u = ()
}

fn render(n: Int, b: Byte, ok: Bool, t: Text, raw: Bytes) -> Text {
  return "n={n} b={b} ok={ok} t={t} raw={raw}"
}

fn configure(name: Text, n: Int) -> Int {
  let config = Config { name }
  return config.on_line(config.port) + n.twice()
}

fn first(values: List<Int>) -> Int {
  let n = match values.len() {
    NONE => return 0
    _ => values.at(0)
  }
  return n
}

fn capped(n: Int) -> Int {
  let c = match n {
    limit => 1
    other => other
  }
  return c
}

fn greet() -> Result<Unit, IoError> = Stdout.write_all("hi".to_bytes())
"#;
