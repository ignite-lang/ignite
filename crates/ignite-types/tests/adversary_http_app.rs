//! Adversary pass 1 on story `http-app-connection`: the typer rule that a
//! record literal passed to a generic fn is typed against the parameter
//! "as far as the other arguments bind it, as a closure was"
//! (`docs/slice.md` NN.1, `crates/ignite-types/src/typer.rs` `infer`).

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
#[ignore = "story:typer-record-literal-deferred"]
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
