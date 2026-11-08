//! Adversarial cases for story `infer-channel-ends` (`docs/slice.md` 11.7,
//! case-study gap G32): `ignite-check` now takes the static type of a
//! local from the declared return type of a called fn and of a `match`
//! arm binding from the scrutinee. An inferred type must never let an
//! effect check clean that the program performs.

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_types::{TypedModules, type_module};

/// Check and type `body` as a program that may import `std.lib`, a
/// module whose source is `lib`.
fn frontend(lib: &str, body: &str) -> Result<(), String> {
    let lib_source = SourceFile::new("lib.ignite", format!("package std.lib\n\n{lib}"));
    let lib_program = ignite_syntax::parse(&lib_source)
        .unwrap_or_else(|d| panic!("lib parse failed:\n{}", lib_source.render_all(&d)));
    let lib_symbols = check_module(&lib_program, &Modules::new())
        .unwrap_or_else(|d| panic!("lib check failed:\n{}", lib_source.render_all(&d)));
    let lib_typed = type_module(&lib_program, &lib_symbols, &TypedModules::new())
        .unwrap_or_else(|d| panic!("lib type failed:\n{}", lib_source.render_all(&d)));
    let mut modules = Modules::new();
    modules.insert("std.lib".to_string(), lib_symbols);
    let mut typed = TypedModules::new();
    typed.insert("std.lib".to_string(), lib_typed.exports());

    let source = SourceFile::new("probe.ignite", body.to_string());
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    let symbols = check_module(&program, &modules).map_err(|d| source.render_all(&d))?;
    type_module(&program, &symbols, &typed)
        .map(|_| ())
        .map_err(|d| source.render_all(&d))
}

fn assert_effect_reported(lib: &str, body: &str) {
    match frontend(lib, body) {
        Ok(()) => panic!(
            "checked clean, so a fn performs an effect its `requires` does not hold:\n{body}"
        ),
        Err(text) => assert!(
            ["E016", "E017", "E042"]
                .iter()
                .any(|code| text.contains(&format!("error[{code}]"))),
            "no effect diagnostic:\n{text}"
        ),
    }
}

const ENDS_LIB: &str = r#"type Ends {
  tx: Sender<Int>
}

fn wrap(s: Sender<Int>) -> Ends = Ends { tx = s }

type Tx = Sender<Int>

fn first(senders: Store<Tx>) -> Option<Tx> requires [senders.read] = senders.get(1)
"#;

#[test]
fn an_imported_fn_s_return_type_is_not_read_in_the_importer_s_namespace() {
    // `wrap` returns `std.lib`'s `Ends`, whose `tx` is a `Sender<Int>`:
    // `d.tx.close()` needs `s.send`. The importer's own `Ends` holds a
    // `Receiver<Int>`; reading `wrap`'s declared `Ends` as that one charges
    // `s.receive` instead, and the missing `s.send` checks clean.
    assert_effect_reported(
        ENDS_LIB,
        "import std.lib.{wrap}\n\ntype Ends {\n  tx: Receiver<Int>\n}\n\nfn shut(s: Sender<Int>) -> Unit requires [s.receive] {\n  let d = wrap(s)\n  d.tx.close()\n}\n",
    );
}

#[test]
fn an_imported_fn_s_alias_is_not_expanded_in_the_importer_s_namespace() {
    // `first` returns `Option<Tx>` with `std.lib`'s `Tx = Sender<Int>`; the
    // importer's `Tx` is a `Receiver<Int>`. `tx.close()` on the binding
    // needs `senders.send`.
    assert_effect_reported(
        ENDS_LIB,
        "import std.lib.{first}\n\ntype Tx = Receiver<Int>\n\nfn shut(senders: Store<Sender<Int>>) -> Unit requires [senders.read, senders.receive] {\n  match first(senders) {\n    Some(tx) => tx.close()\n    None => Unit\n  }\n}\n",
    );
}

#[test]
fn the_same_shapes_without_a_local_type_check_their_effect() {
    // Control: without the shadowing local type the imported return type
    // is unknown to the importer and the call stays E042 or the effect is
    // charged; never clean.
    assert_effect_reported(
        ENDS_LIB,
        "import std.lib.{wrap}\n\nfn shut(s: Sender<Int>) -> Unit requires [s.receive] {\n  let d = wrap(s)\n  d.tx.close()\n}\n",
    );
}

#[test]
fn the_correct_requires_of_an_imported_return_type_is_not_asked_for_a_receive() {
    // The other half of the namespace case: `requires [s.send]` is what
    // `d.tx.close()` on `std.lib`'s `Ends` needs. Whatever else is said
    // about it (the base tree asks for `s.write`, the untyped `close`), a
    // `Sender<Int>` parameter is never asked for `s.receive`, the effect of
    // the importer's `Ends`.
    if let Err(text) = frontend(
        ENDS_LIB,
        "import std.lib.{wrap}\n\ntype Ends {\n  tx: Receiver<Int>\n}\n\nfn shut(s: Sender<Int>) -> Unit requires [s.send] {\n  let d = wrap(s)\n  d.tx.close()\n}\n",
    ) {
        assert!(!text.contains("`s.receive`"), "{text}");
    }
}

const ONE_MODULE: &str = r#"fn stamp(id: Int) -> Int requires [clock.read] {
  let now = Clock.monotonic()
  return id
}

fn pick() -> Option<fn(id: Int) -> Int requires [clock.read]> = Some(stamp)

type Holder {
  f: fn(id: Int) -> Int requires [clock.read]
}

fn hold() -> Holder = Holder { f = stamp }

fn first_pair(senders: Store<Sender<Int>>) -> Option<(Sender<Int>, Int)> requires [senders.read] {
  match senders.get(1) {
    Some(tx) => Some((tx, 1))
    None => None
  }
}

fn refused(senders: Store<Sender<Int>>) -> Result<Int, Sender<Int>> requires [senders.read] {
  match senders.get(1) {
    Some(tx) => Err(tx)
    None => Ok(0)
  }
}
"#;

#[test]
fn a_fn_value_in_an_option_a_call_returns_charges_its_effect() {
    assert_effect_reported(
        "",
        &format!(
            "{ONE_MODULE}\nfn direct(n: Int) -> Int {{\n  match pick() {{\n    Some(g) => g(n)\n    None => n\n  }}\n}}\n\nfn caller(n: Int) -> Int requires [stdout.write] = direct(n)\n"
        ),
    );
}

#[test]
fn a_fn_field_of_a_record_a_call_returns_charges_its_effect() {
    assert_effect_reported(
        "",
        &format!(
            "{ONE_MODULE}\nfn direct(n: Int) -> Int {{\n  let h = hold()\n  h.f(n)\n}}\n\nfn caller(n: Int) -> Int requires [stdout.write] = direct(n)\n"
        ),
    );
}

#[test]
fn a_tuple_inside_some_charges_the_end_s_effect() {
    assert_effect_reported(
        "",
        &format!(
            "{ONE_MODULE}\nfn shut(senders: Store<Sender<Int>>) -> Unit requires [senders.read] {{\n  match first_pair(senders) {{\n    Some((tx, _)) => tx.close()\n    None => Unit\n  }}\n}}\n"
        ),
    );
}

#[test]
fn an_err_payload_charges_the_end_s_effect() {
    assert_effect_reported(
        "",
        &format!(
            "{ONE_MODULE}\nfn shut(senders: Store<Sender<Int>>) -> Unit requires [senders.read] {{\n  match refused(senders) {{\n    Ok(_) => Unit\n    Err(tx) => tx.close()\n  }}\n}}\n"
        ),
    );
}
