//! Second adversarial pass on story `infer-channel-ends` (`docs/slice.md`
//! 11.7, case-study gap G32), after the correction that takes a call's
//! return type only for fns the module declares. A fn the module declares
//! may still return a type it imports, and that type's fields are declared
//! in the exporting module's namespace; an inferred type must never let an
//! effect check clean that the program performs.

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_types::{TypedModules, type_module};

/// Check and type `body` as a program that may import each of `libs`
/// (`(module path, source)`, checked in order, each seeing the ones before).
fn frontend(libs: &[(&str, &str)], body: &str) -> Result<(), String> {
    let mut modules = Modules::new();
    let mut typed = TypedModules::new();
    for (path, lib) in libs {
        let source = SourceFile::new(format!("{path}.ignite"), format!("package {path}\n\n{lib}"));
        let program = ignite_syntax::parse(&source)
            .unwrap_or_else(|d| panic!("{path} parse failed:\n{}", source.render_all(&d)));
        let symbols = check_module(&program, &modules)
            .unwrap_or_else(|d| panic!("{path} check failed:\n{}", source.render_all(&d)));
        let lib_typed = type_module(&program, &symbols, &typed)
            .unwrap_or_else(|d| panic!("{path} type failed:\n{}", source.render_all(&d)));
        modules.insert(path.to_string(), symbols);
        typed.insert(path.to_string(), lib_typed.exports());
    }
    let source = SourceFile::new("probe.ignite", body.to_string());
    let program = ignite_syntax::parse(&source)
        .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d)));
    let symbols = check_module(&program, &modules).map_err(|d| source.render_all(&d))?;
    type_module(&program, &symbols, &typed)
        .map(|_| ())
        .map_err(|d| source.render_all(&d))
}

fn assert_effect_reported(libs: &[(&str, &str)], body: &str) {
    match frontend(libs, body) {
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

fn assert_clean(libs: &[(&str, &str)], body: &str) {
    if let Err(text) = frontend(libs, body) {
        panic!("G32 form needs no annotation, yet:\n{text}\n--- program:\n{body}");
    }
}

/// `Ends.tx` is declared as `Inner`, `std.lib`'s alias of `Sender<Int>`;
/// `Inner` itself is not imported by the probes.
const NESTED_LIB: &str = r#"type Inner = Sender<Int>

type Ends {
  tx: Inner
}
"#;

#[test]
fn a_module_fn_returning_an_imported_record_reads_its_fields_in_the_exporter_s_namespace() {
    // `mk` is declared here, so the correction infers `d: Ends`. `Ends` is
    // `std.lib`'s, its `tx` a `Sender<Int>`, and `d.tx.close()` needs
    // `s.send`. The probe's own `Inner` is a `Receiver<Int>`; resolving the
    // field's declared `Inner` here charges `s.receive` instead.
    assert_effect_reported(
        &[("std.lib", NESTED_LIB)],
        "import std.lib.{Ends}\n\ntype Inner = Receiver<Int>\n\nfn mk(s: Sender<Int>) -> Ends = Ends { tx = s }\n\nfn shut(s: Sender<Int>) -> Unit requires [s.receive] {\n  let d = mk(s)\n  d.tx.close()\n}\n",
    );
}

#[test]
fn a_module_fn_returning_an_option_of_an_imported_record_binds_the_exporter_s_field_type() {
    // The match form of the same: `Some(e)` binds `std.lib`'s `Ends`.
    assert_effect_reported(
        &[("std.lib", NESTED_LIB)],
        "import std.lib.{Ends}\n\ntype Inner = Receiver<Int>\n\nfn find(s: Sender<Int>) -> Option<Ends> = Some(Ends { tx = s })\n\nfn shut(s: Sender<Int>) -> Unit requires [s.receive] {\n  match find(s) {\n    Some(e) => e.tx.close()\n    None => Unit\n  }\n}\n",
    );
}

#[test]
fn an_intermediate_module_s_fn_returning_a_record_it_imports_is_checked_there() {
    // Re-export shape: `std.mid` imports `std.lib`'s `Ends` and declares
    // `mk` and `shut` over it, with its own `Inner`. Imports are not
    // re-exported, so this is the only place the chain is checked.
    assert_effect_reported(
        &[("std.lib", NESTED_LIB)],
        "package std.mid\n\nimport std.lib.{Ends}\n\ntype Inner = Receiver<Int>\n\nfn mk(s: Sender<Int>) -> Ends = Ends { tx = s }\n\nfn shut(s: Sender<Int>) -> Unit requires [s.receive] {\n  let d = mk(s)\n  d.tx.close()\n}\n",
    );
}

#[test]
fn a_parameter_of_an_imported_record_reads_its_fields_in_the_exporter_s_namespace() {
    // Control for the origin: no call and no match, only a parameter of
    // the imported record type. Reproduces on the base if this is red.
    assert_effect_reported(
        &[("std.lib", NESTED_LIB)],
        "import std.lib.{Ends}\n\ntype Inner = Receiver<Int>\n\nfn shut(s: Sender<Int>, d: Ends) -> Unit requires [s.receive, d.receive] {\n  d.tx.close()\n}\n",
    );
}

#[test]
fn a_reordering_generic_alias_returned_by_a_module_fn_binds_the_right_payload() {
    // `Flip<A, B>` is `Result<B, A>`: `Ok(tx)` is the `Sender<Int>`.
    assert_effect_reported(
        &[],
        "type Flip<A, B> = Result<B, A>\n\nfn flip(s: Sender<Int>) -> Flip<Receiver<Int>, Sender<Int>> = Ok(s)\n\nfn shut(s: Sender<Int>) -> Unit requires [s.receive] {\n  match flip(s) {\n    Ok(tx) => tx.close()\n    Err(rx) => rx.close()\n  }\n}\n",
    );
}

#[test]
fn a_nested_option_from_a_store_charges_the_inner_end_s_effect() {
    assert_effect_reported(
        &[],
        "fn shut(slots: Store<Option<Sender<Int>>>) -> Unit requires [slots.read] {\n  match slots.get(1) {\n    Some(Some(tx)) => tx.close()\n    _ => Unit\n  }\n}\n",
    );
}

#[test]
fn a_module_fn_shadowing_a_glob_import_uses_its_own_return_type() {
    // `import std.lib.*` offers a `wrap` returning `std.lib`'s `Ends` (a
    // `Sender<Int>`); the module declares its own `wrap` and `Ends` (a
    // `Receiver<Int>`), which win. `d.tx.close()` needs `r.receive`.
    assert_effect_reported(
        &[(
            "std.lib",
            "type Ends {\n  tx: Sender<Int>\n}\n\nfn wrap(s: Sender<Int>) -> Ends = Ends { tx = s }\n",
        )],
        "import std.lib.*\n\ntype Ends {\n  tx: Receiver<Int>\n}\n\nfn wrap(r: Receiver<Int>) -> Ends = Ends { tx = r }\n\nfn shut(r: Receiver<Int>) -> Unit requires [r.send] {\n  let d = wrap(r)\n  d.tx.close()\n}\n",
    );
}

#[test]
fn the_g32_forms_through_an_alias_and_a_tuple_need_no_annotation() {
    // The two G32 forms where the module's types reach the end through an
    // alias inside a tuple and an alias of the record: both are declared in
    // this module, so the static type follows from declarations alone.
    assert_clean(
        &[],
        "type Tx = Sender<Int>\n\ntype Ends {\n  tx: Tx\n  rx: Receiver<Int>\n}\n\ntype Pair = Ends\n\nfn ends() -> Pair {\n  let pair: (Sender<Int>, Receiver<Int>) = Channel.bounded(4)\n  Ends { tx = pair.0, rx = pair.1 }\n}\n\nfn first(senders: Store<Tx>) -> Option<(Tx, Int)> requires [senders.read] {\n  match senders.get(1) {\n    Some(tx) => Some((tx, 1))\n    None => None\n  }\n}\n\nfn ring(senders: Store<Tx>) -> Bool requires [senders.read, senders.send] {\n  match first(senders) {\n    Some((tx, _)) => tx.try_send(1)\n    None => false\n  }\n}\n\nfn round_trip() -> Option<Int> {\n  let d = ends()\n  d.tx.try_send(1)\n  d.rx.receive()\n}\n",
    );
}
