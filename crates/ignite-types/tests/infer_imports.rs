//! Imported declarations under the inferred types of story
//! `infer-channel-ends` (`docs/slice.md` 11.7): an imported record's field
//! types and an imported alias's target are written in the exporting
//! module's namespace. Where a name in them could mean another type in the
//! importer, `ignite-check` gives no static type (E042) rather than a wrong
//! one, and where every name means the same the effect is checked.

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_types::{TypedModules, type_module};

const LIB: &str = r#"type Ends {
  tx: Sender<Int>
}

type Pair = Ends

type Inner = Sender<Int>

type Held = Option<Inner>
"#;

fn frontend(body: &str) -> Result<(), String> {
    let lib = SourceFile::new("lib.ignite", format!("package std.lib\n\n{LIB}"));
    let lib_program = ignite_syntax::parse(&lib).expect("lib parses");
    let lib_symbols = check_module(&lib_program, &Modules::new()).expect("lib checks");
    let lib_typed =
        type_module(&lib_program, &lib_symbols, &TypedModules::new()).expect("lib types");
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

#[test]
fn an_imported_alias_is_not_expanded_into_a_local_type_of_the_same_name() {
    // `Pair` is `std.lib`'s `Ends`, whose `tx` is a `Sender<Int>`. The
    // importer's own `Ends` holds a `Receiver<Int>`; expanding `Pair` here
    // would charge `p.receive` and let the missing `p.send` check clean.
    // Unexpanded, `close` is the untyped builtin method and asks for
    // `p.write`, as it did before the story.
    let text = frontend(
        "import std.lib.{Pair}\n\ntype Ends {\n  tx: Receiver<Int>\n}\n\nfn shut(p: Pair) -> Unit requires [p.receive] {\n  p.tx.close()\n}\n",
    )
    .unwrap_err();
    assert!(
        ["E016", "E042"]
            .iter()
            .any(|code| text.contains(&format!("error[{code}]"))),
        "{text}"
    );
}

#[test]
fn an_imported_alias_whose_names_are_imported_from_its_module_is_checked() {
    let text = frontend(
        "import std.lib.{Held, Inner}\n\nfn shut(h: Held) -> Unit requires [] {\n  match h {\n    Some(tx) => tx.close()\n    None => Unit\n  }\n}\n",
    )
    .unwrap_err();
    assert!(
        text.contains("error[E016]: missing effect `h.send`"),
        "{text}"
    );
}

#[test]
fn an_imported_record_whose_field_names_only_builtins_is_checked() {
    let text = frontend(
        "import std.lib.{Ends}\n\nfn shut(d: Ends) -> Unit requires [] {\n  d.tx.close()\n}\n",
    )
    .unwrap_err();
    assert!(
        text.contains("error[E016]: missing effect `d.send`"),
        "{text}"
    );
}
