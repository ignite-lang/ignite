//! The slice-7 frontend contract through the public API (`docs/slice.md`
//! section 11.7): generic instances, type parameters, `Store<T>`, effectful
//! fn types and the new builtins.

use crate::common;

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_syntax::ast::{Decl, Expr, Program};
use ignite_types::{
    Builtin, Callee, FnEffects, QualName, Ty, TypedModules, TypedProgram, type_module,
};

const SOURCE: &str = r#"
type Pair<A, B> {
  first: A
  second: B
}

enum Tree<T> {
  Leaf,
  Node(T, List<Tree<T>>),
}

type Todo {
  id: Int
  title: Text
}

type App<S> {
  state: S
  handle: fn(state: S, stream: TcpStream) -> Result<Unit, IoError> requires [state.*, stream.*]
}

protocol Serve {
  fn serve<S>(stream: TcpStream, application: App<S>) -> Result<Unit, IoError> requires [stream.*, application.*] {
    return application.handle(application.state, stream)
  }
}

fn swap<A, B>(p: Pair<A, B>) -> Pair<B, A> = Pair { first = p.second, second = p.first }

fn identity<T>(value: T) -> T = value

fn title_of(todo: Todo) -> Text = todo.title

fn respond(todos: Store<Todo>, stream: TcpStream) -> Result<Unit, IoError> requires [todos.*, stream.write] {
  let id = todos.next_id()
  todos.put(id, Todo { id = id, title = "milk" })
  let titles = todos.values().map(title_of)
  return stream.write_all(Int.to_text(id).to_bytes())
}

fn uses() -> Int {
  let swapped = swap(Pair { first = 1, second = "one" })
  let tree = Node(true, [Leaf])
  let f: fn(Text) -> Text = identity
  let parsed = Int.parse("12")
  return swapped.second
}

type Server<S> {
  listener: TcpListener
  application: App<S>
  executable: Text
}

fn Server.main<S>(self: Server<S>) -> Result<Unit, IoError> {
  loop {
    let stream = self.listener.accept()?
    spawn Serve.serve(stream, self.application)
  }
}

system Todos {
  resource ingress: TcpListener {
    address = "127.0.0.1"
    port = 7072
  }

  resource todos: Store<Todo>

  component server: Server {
    listener = ingress
    application = App { state = todos, handle = respond }
    executable = "todod"
  }
}
"#;

struct Typed {
    source: SourceFile,
    program: Box<Program>,
    typed: TypedProgram,
}

fn typed() -> Typed {
    let source = SourceFile::new("generics.ignite", SOURCE.to_string());
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
}

fn q(name: &str) -> QualName {
    QualName::new("main", name)
}

fn todo() -> Ty {
    Ty::Record(q("Todo"))
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
fn type_parameters_are_exported() {
    let t = typed();
    let names = |ps: Option<&[String]>| ps.map(<[String]>::to_vec);
    assert_eq!(
        names(t.typed.type_params(&q("Pair"))),
        Some(vec!["A".to_string(), "B".to_string()])
    );
    assert_eq!(
        names(t.typed.type_params(&q("Tree"))),
        Some(vec!["T".to_string()])
    );
    assert_eq!(t.typed.type_params(&q("Todo")), None);
    assert_eq!(
        names(t.typed.role_type_params(&q("Serve"), "serve")),
        Some(vec!["S".to_string()])
    );
    let fields = t.typed.record_fields(&q("Pair")).unwrap();
    assert_eq!(fields[0].1, Ty::Param("A".to_string()));
    let sig = &t.typed.exports().fns["swap"];
    assert_eq!(
        sig.ret,
        Ty::GenericRecord(
            q("Pair"),
            vec![Ty::Param("B".to_string()), Ty::Param("A".to_string())]
        )
    );
}

#[test]
fn instances_of_generic_uses() {
    let t = typed();
    let call = t.find(r#"swap(Pair { first = 1, second = "one" })"#);
    assert_eq!(t.typed.instance(call), Some(&[Ty::Int, Ty::Text][..]));
    assert_eq!(
        t.typed.type_of(call),
        &Ty::GenericRecord(q("Pair"), vec![Ty::Text, Ty::Int])
    );
    let literal = t.find(r#"Pair { first = 1, second = "one" }"#);
    assert_eq!(t.typed.instance(literal), Some(&[Ty::Int, Ty::Text][..]));
    let node = t.find("Node(true, [Leaf])");
    assert_eq!(t.typed.instance(node), Some(&[Ty::Bool][..]));
    assert_eq!(
        t.typed.variant(t.typed.type_of(node), "Node"),
        Some((
            1,
            vec![
                Ty::Bool,
                Ty::List(Box::new(Ty::GenericEnum(q("Tree"), vec![Ty::Bool])))
            ]
        ))
    );
    let leaf = t.find("Leaf");
    assert_eq!(t.typed.instance(leaf), Some(&[Ty::Bool][..]));
    let value = t.find("identity");
    assert_eq!(t.typed.instance(value), Some(&[Ty::Text][..]));
    let inner = t.find("Pair { first = p.second, second = p.first }");
    assert_eq!(
        t.typed.instance(inner),
        Some(&[Ty::Param("B".to_string()), Ty::Param("A".to_string())][..])
    );
    assert_eq!(t.typed.instance(t.find("Int.parse(\"12\")")), None);
}

/// A component of a generic kind (`docs/slice.md` section 14.5): the
/// executable's kind and the type arguments of its `main` are inferred
/// from the properties.
#[test]
fn component_kind_carries_the_type_arguments() {
    let t = typed();
    let Some(Decl::System(system)) = t.program.decls.last() else {
        panic!("no system");
    };
    let store = Ty::Store(Box::new(todo()));
    let executables = t.typed.executables();
    assert_eq!(executables.len(), 1);
    assert_eq!(executables[0].executable, "todod");
    assert_eq!(
        executables[0].kind,
        Ty::GenericRecord(q("Server"), vec![store.clone()])
    );
    assert_eq!(executables[0].main_args, vec![store.clone()]);
    let application = &system.components[0]
        .properties
        .iter()
        .find(|p| p.name.name == "application")
        .unwrap()
        .value;
    assert_eq!(
        t.typed.type_of(application),
        &Ty::GenericRecord(q("App"), vec![store])
    );
}

#[test]
fn store_and_new_builtins() {
    let t = typed();
    let callee = |text: &str| t.typed.callee(t.find(text)).cloned();
    assert_eq!(
        callee("todos.next_id()"),
        Some(Callee::Builtin(Builtin::StoreNextId))
    );
    assert_eq!(
        callee(r#"todos.put(id, Todo { id = id, title = "milk" })"#),
        Some(Callee::Builtin(Builtin::StorePut))
    );
    assert_eq!(
        t.typed.type_of(t.find("todos.values()")),
        &Ty::List(Box::new(todo()))
    );
    let map = t.find("todos.values().map(title_of)");
    assert_eq!(
        t.typed.callee(map),
        Some(&Callee::Builtin(Builtin::ListMap))
    );
    assert_eq!(t.typed.type_of(map), &Ty::List(Box::new(Ty::Text)));
    let parse = t.find("Int.parse(\"12\")");
    assert_eq!(
        t.typed.callee(parse),
        Some(&Callee::Builtin(Builtin::IntParse))
    );
    assert_eq!(t.typed.type_of(parse), &Ty::Option(Box::new(Ty::Int)));
}

#[test]
fn effectful_fn_types() {
    let t = typed();
    let call = t.find("application.handle(application.state, stream)");
    assert_eq!(
        t.typed.callee(call),
        Some(&Callee::FieldFn {
            record: q("App"),
            field: "handle".to_string()
        })
    );
    let field = &t.typed.record_fields(&q("App")).unwrap()[1].1;
    let renamed = Ty::EffectFn(
        vec![Ty::Param("S".to_string()), Ty::TcpStream],
        Box::new(Ty::Result(Box::new(Ty::Unit), Box::new(Ty::IoError))),
        FnEffects {
            names: vec!["s".to_string(), "t".to_string()],
            requires: vec!["t.*".to_string(), "s.*".to_string()],
        },
    );
    assert_eq!(field, &renamed, "effects compare by parameter position");
    assert_eq!(
        field.to_string(),
        "fn(state: S, stream: TcpStream) -> Result<Unit, IoError> requires [state.*, stream.*]"
    );
}

/// The diagnostics of checking and typing `body` alone, rendered.
fn diagnostics_of(body: &str) -> Option<String> {
    let source = SourceFile::new("probe.ignite", body.to_string());
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    let symbols = match check_module(&program, &Modules::new()) {
        Ok(symbols) => symbols,
        Err(d) => return Some(source.render_all(&d)),
    };
    type_module(&program, &symbols, &TypedModules::new())
        .err()
        .map(|d| source.render_all(&d))
}

/// A record literal passed to a generic fn is typed against the parameter
/// as far as the other arguments bind it: a type parameter of the record
/// that none of its fields names (`T`, the stream type of a field's fn
/// type that defaults) comes from the call, as a closure's parameter
/// types do. Without such an argument it stays E042.
#[test]
fn a_record_literal_argument_takes_its_type_parameters_from_the_call() {
    let program = r#"
type Box<S, T> {
  state: S
  touch: Option<fn(state: S, stream: T) -> Int> = None
}

fn serve<T, S>(stream: T, boxed: Box<S, T>) -> Int = match boxed.touch {
  Some(touch) => touch(boxed.state, stream)
  None => 0
}

fn hold(stream: Int, boxed: Box<Text, Int>) -> Int = serve(stream, boxed)

fn main() -> Int = serve(7, Box { state = "s" })
"#;
    assert_eq!(diagnostics_of(program), None);
    let unbound = r#"
type Box<S, T> {
  state: S
  touch: Option<fn(state: S, stream: T) -> Int> = None
}

fn keep<S, T>(boxed: Box<S, T>) -> Int = 0

fn main() -> Int = keep(Box { state = "s" })
"#;
    let text = diagnostics_of(unbound).expect("`T` is bound by nothing");
    assert!(
        text.contains("error[E042]") && text.contains("`T`"),
        "{text}"
    );
}
