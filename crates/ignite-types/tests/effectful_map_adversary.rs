//! Adversarial cases for stories `effectful-list-map` and
//! `effectful-fn-in-local` (`docs/slice.md` 20.4.3): a local of effectful
//! fn type whose type `ignite-check` cannot tell (bound from a call, a
//! field of a call's result, a pattern) passed to `List.map` where the
//! type checker leaves the charge to `ignite-check`. A direct call of the
//! same local is E042; the `map` of it must not check clean either.

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_types::{TypedModules, type_module};

const STAMP: &str = r#"fn stamp(id: Int) -> Int requires [clock.read] {
  let now = Clock.monotonic()
  return id
}

fn make() -> fn(id: Int) -> Int requires [clock.read] = stamp

type Holder {
  f: fn(id: Int) -> Int requires [clock.read]
}

fn hold() -> Holder = Holder { f = stamp }
"#;

fn frontend(body: &str) -> Result<(), String> {
    let source = SourceFile::new("probe.ignite", format!("{STAMP}\n{body}"));
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    let symbols = check_module(&program, &Modules::new()).map_err(|d| source.render_all(&d))?;
    type_module(&program, &symbols, &TypedModules::new())
        .map(|_| ())
        .map_err(|d| source.render_all(&d))
}

fn assert_effect_reported(body: &str) {
    match frontend(body) {
        Ok(()) => panic!(
            "checked clean, so a fn performs `clock.read` that no `requires` or inferred set holds:\n{body}"
        ),
        Err(text) => assert!(
            ["E016", "E017", "E042"]
                .iter()
                .any(|code| text.contains(&format!("error[{code}]"))),
            "no effect diagnostic:\n{text}"
        ),
    }
}

#[test]
fn a_direct_call_of_a_local_bound_from_a_call_is_e042() {
    let text = frontend(
        "fn direct(n: Int) -> Int {\n  let g = make()\n  return g(n)\n}\n\nfn caller(n: Int) -> Int requires [stdout.write] = direct(n)\n",
    )
    .unwrap_err();
    assert!(text.contains("error[E042]"), "{text}");
}

#[test]
fn mapping_a_local_bound_from_a_call_charges_the_caller_of_an_inferred_fn() {
    assert_effect_reported(
        "fn hidden(ids: List<Int>) -> List<Int> {\n  let g = make()\n  return ids.map(g)\n}\n\nfn caller(ids: List<Int>) -> List<Int> requires [stdout.write] = hidden(ids)\n",
    );
}

#[test]
fn mapping_a_field_of_a_call_result_through_a_local_charges_the_caller_of_an_inferred_fn() {
    assert_effect_reported(
        "fn by_field(ids: List<Int>) -> List<Int> {\n  let h = hold()\n  let g = h.f\n  return ids.map(g)\n}\n\nfn caller(ids: List<Int>) -> List<Int> requires [stdout.write] = by_field(ids)\n",
    );
}

#[test]
fn mapping_a_match_binding_of_effectful_fn_type_charges_the_caller_of_an_inferred_fn() {
    assert_effect_reported(
        "fn by_match(ids: List<Int>, o: Option<fn(id: Int) -> Int requires [clock.read]>) -> List<Int> {\n  match o {\n    Some(g) => return ids.map(g),\n    None => return ids,\n  }\n}\n\nfn caller(ids: List<Int>) -> List<Int> requires [stdout.write] = by_match(ids, None)\n",
    );
}

#[test]
fn a_closure_passed_to_map_under_requires_cannot_map_an_untyped_effectful_local() {
    assert_effect_reported(
        "fn declared(rows: List<List<Int>>) -> List<List<Int>> requires [stdout.write] {\n  return rows.map(|row: List<Int>| {\n    let g = make()\n    row.map(g)\n  })\n}\n",
    );
}

#[test]
fn an_uncovered_effectful_fn_mapped_into_an_unannotated_let_is_e017() {
    let text = frontend(
        "fn unannotated(ids: List<Int>) -> List<Int> requires [stdout.write] {\n  let out = ids.map(stamp)\n  return out\n}\n",
    )
    .unwrap_err();
    assert!(
        text.contains("error[E017]: effectful function `stamp` passed to `map` needs `clock.read`, which the `requires` of `unannotated` does not cover"),
        "{text}"
    );
}
