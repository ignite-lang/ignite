//! Adversary pass 1 on story `http-app-connection`: the typer rule that a
//! record literal passed to a generic fn is typed against the parameter
//! "as far as the other arguments bind it, as a closure was"
//! (`docs/slice.md` 37.1, `crates/ignite-types/src/typer.rs` `infer`).

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_types::{TypedModules, type_module};

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

const BOX: &str = r#"
type Box<S, T> {
  state: S
  touch: Option<fn(state: S, stream: T) -> Int> = None
}
"#;

/// A closure is typed after every other argument (fn-typed parameters are
/// deferred to the second pass), a record literal only after the arguments
/// before it: the stream given after the literal does not bind its `T`.
#[test]
fn a_record_literal_before_the_argument_that_binds_its_parameter() {
    let program = format!(
        "{BOX}
fn run<T, S>(boxed: Box<S, T>, stream: T) -> Int = match boxed.touch {{
  Some(touch) => touch(boxed.state, stream)
  None => 0
}}

fn main() -> Int = run(Box {{ state = \"s\" }}, 7)
"
    );
    assert_eq!(diagnostics_of(&program), None);
}

/// The callee's type parameters and the record's have the same names in
/// swapped places: the call's `'T` must not be read as the record's `'T`.
#[test]
fn swapped_parameter_names_between_the_call_and_the_record() {
    let program = format!(
        "{BOX}
fn swapped<S, T>(stream: S, boxed: Box<T, S>) -> Int = match boxed.touch {{
  Some(touch) => touch(boxed.state, stream)
  None => 0
}}

fn main() -> Int = swapped(7, Box {{ state = \"s\" }})
"
    );
    assert_eq!(diagnostics_of(&program), None);
}

/// A generic record literal inside a generic record literal, the inner
/// one's `T` bound only by the call.
#[test]
fn a_nested_generic_record_literal_takes_its_parameter_from_the_call() {
    let program = format!(
        "{BOX}
type Outer<S, T> {{
  inner: Box<S, T>
  tag: Int = 0
}}

fn serve<T, S>(stream: T, outer: Outer<S, T>) -> Int = match outer.inner.touch {{
  Some(touch) => touch(outer.inner.state, stream)
  None => outer.tag
}}

fn main() -> Int = serve(7, Outer {{ inner = Box {{ state = \"s\" }} }})
"
    );
    assert_eq!(diagnostics_of(&program), None);
}

/// A plain record literal where the parameter is itself a type parameter:
/// typed as before the change.
#[test]
fn a_plain_record_literal_where_the_parameter_is_a_type_parameter() {
    let program = r#"
type Todo {
  id: Int
  title: Text
}

fn keep<X>(x: X) -> Int = 0

fn main() -> Int = keep(Todo { id = 1, title = "t" })
"#;
    assert_eq!(diagnostics_of(program), None);
}

/// A field that conflicts with what the call binds is still an error.
#[test]
fn a_field_that_conflicts_with_the_call_is_still_e040() {
    let program = format!(
        "{BOX}
fn serve<T, S>(stream: T, boxed: Box<S, T>) -> Int = 0

fn text_touch(state: Text, stream: Text) -> Int = 1

fn main() -> Int = serve(7, Box {{ state = \"s\", touch = Some(text_touch) }})
"
    );
    let text = diagnostics_of(&program).expect("`touch` takes Text where the call gives Int");
    assert!(text.contains("error[E040]"), "{text}");
}

/// The literal waits only for the arguments of its call: a parameter none
/// of them binds is still E042, reported at the literal.
#[test]
fn a_record_literal_whose_parameter_no_argument_binds_is_still_e042() {
    let program = format!(
        "{BOX}
fn hold<T, S>(boxed: Box<S, T>, count: Int) -> Int = count

fn main() -> Int = hold(Box {{ state = \"s\" }}, 7)
"
    );
    let text = diagnostics_of(&program).expect("nothing binds `T`");
    assert!(text.contains("error[E042]"), "{text}");
    assert!(text.contains("`T` of `Box`"), "{text}");
}

/// A nested generic record literal before the argument that binds the
/// inner one's `T`.
#[test]
fn a_nested_record_literal_before_the_argument_that_binds_its_parameter() {
    let program = format!(
        "{BOX}
type Outer<S, T> {{
  inner: Box<S, T>
  tag: Int = 0
}}

fn serve<T, S>(outer: Outer<S, T>, stream: T) -> Int = match outer.inner.touch {{
  Some(touch) => touch(outer.inner.state, stream)
  None => outer.tag
}}

fn main() -> Int = serve(Outer {{ inner = Box {{ state = \"s\" }} }}, 7)
"
    );
    assert_eq!(diagnostics_of(&program), None);
}

/// A closure after a waiting literal still sees what the literal binds:
/// the literal is typed before the closures, after the plain arguments.
#[test]
fn a_closure_is_typed_after_a_record_literal_that_waited() {
    let program = format!(
        "{BOX}
fn fold<T, S>(boxed: Box<S, T>, read: fn(state: S) -> Int, stream: T) -> Int = read(boxed.state)

fn main() -> Int = fold(Box {{ state = 4 }}, |state| state + 1, \"t\")
"
    );
    assert_eq!(diagnostics_of(&program), None);
}

/// A later argument that conflicts with a field of the waiting literal is
/// still an error.
#[test]
fn a_later_argument_that_conflicts_with_the_waiting_literal_is_an_error() {
    let program = format!(
        "{BOX}
fn run<T, S>(boxed: Box<S, T>, stream: T) -> Int = 0

fn text_touch(state: Text, stream: Text) -> Int = 1

fn main() -> Int = run(Box {{ state = \"s\", touch = Some(text_touch) }}, 7)
"
    );
    let text = diagnostics_of(&program).expect("`touch` takes Text where the call gives Int");
    assert!(text.contains("error[E040]"), "{text}");
}

/// A generic record literal given where the parameter is a type parameter
/// of the call, which a later argument binds to an instance of the record.
#[test]
fn a_record_literal_for_a_type_parameter_a_later_argument_binds() {
    let program = format!(
        "{BOX}
fn same<X>(first: X, second: X) -> Int = 0

fn boxed() -> Box<Text, Int> = Box {{ state = \"b\" }}

fn main() -> Int = same(Box {{ state = \"a\" }}, boxed())
"
    );
    assert_eq!(diagnostics_of(&program), None);
}
