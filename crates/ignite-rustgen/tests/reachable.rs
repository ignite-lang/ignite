//! Reachable-only `ignite-std` (`docs/slice.md` 16.22 item 2):
//! `generate_std_reachable` keeps the items the root texts name,
//! transitively, and drops the rest; `generate_std_for` takes the roots
//! from a generated program.

use crate::common;

use std::path::Path;

use ignite_rustgen::{
    Options, TypedStdModule, generate_std_for, generate_std_reachable, generate_std_typed,
    generate_tests, prepare_std_typed,
};

fn norm(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn options() -> Options {
    Options {
        substrate_path: Path::new("../ignite-io").to_path_buf(),
        ..Options::default()
    }
}

/// The golden standard library (`text`, `net`, `protocol.line`), typed,
/// lowered for `roots`, as normalized text.
fn golden_reachable(roots: &[&str]) -> String {
    let library = common::load_std(&common::golden_std());
    let types = common::type_std(&library).expect("the golden standard library types");
    let inputs: Vec<TypedStdModule<'_>> = library
        .iter()
        .zip(&types)
        .map(|((m, p, s), t)| (m.clone(), p, s, t))
        .collect();
    let project = generate_std_reachable(&inputs, &options(), roots).unwrap();
    norm(&project.lib_rs)
}

#[test]
fn keeps_what_the_roots_name_and_what_that_names() {
    let out = golden_reachable(&["fn main() { ignite_std::text::shout(&String::from(\"a\")); }"]);
    assert!(out.contains("pub fn shout("), "{out}");
    assert!(out.contains("pub fn SUFFIX()"), "{out}");
    assert!(!out.contains("Handler"), "{out}");
    assert!(!out.contains("pub enum Mode"), "{out}");
    assert!(!out.contains("pub struct Line"), "{out}");
    assert!(!out.contains("pub fn pump("), "{out}");
    assert!(!out.contains("pub struct Endpoint"), "{out}");
    assert!(out.contains("pub mod net { }"), "{out}");
    assert!(
        out.contains("pub mod protocol { pub mod line { } }"),
        "{out}"
    );
}

#[test]
fn follows_roles_into_their_module_and_its_imports() {
    let out = golden_reachable(&["ignite_std::protocol::line::Line::connect"]);
    assert!(out.contains("pub struct Line;"), "{out}");
    assert!(out.contains("pub fn pump("), "{out}");
    assert!(out.contains("use crate::text::Handler;"), "{out}");
    assert!(out.contains("pub struct Handler"), "{out}");
    assert!(!out.contains("pub fn shout("), "{out}");
    assert!(!out.contains("pub fn SUFFIX()"), "{out}");
    assert!(!out.contains("pub enum Mode"), "{out}");
}

#[test]
fn methods_fields_and_ranges_do_not_name_items() {
    let out = golden_reachable(&["fn main() { x.shout(); y.Line; z.pump(0..pump) }"]);
    assert!(out.contains("pub fn pump("), "{out}");
    assert!(!out.contains("pub fn shout("), "{out}");
    assert!(!out.contains("pub struct Line"), "{out}");
    let none = golden_reachable(&["fn main() { x.shout(); y.Line; z.pump() }"]);
    assert!(!none.contains("pub fn"), "{none}");
    assert!(!none.contains("pub struct"), "{none}");
    assert!(none.contains("pub mod text { }"), "{none}");
}

#[test]
fn naming_everything_gives_the_whole_library() {
    let library = common::load_std(&common::golden_std());
    let types = common::type_std(&library).expect("the golden standard library types");
    let inputs: Vec<TypedStdModule<'_>> = library
        .iter()
        .zip(&types)
        .map(|((m, p, s), t)| (m.clone(), p, s, t))
        .collect();
    let whole = generate_std_typed(&inputs, &options()).unwrap();
    let reachable =
        generate_std_reachable(&inputs, &options(), &["Handler Mode shout Line Endpoint"]).unwrap();
    assert_eq!(reachable.lib_rs, whole.lib_rs);
    assert_eq!(reachable.cargo_toml, whole.cargo_toml);
}

/// A test build of a standard-library module: the test executable names
/// the module's tests, so they and what they call stay; modules the tests
/// do not reach lose their items.
#[test]
fn a_test_build_keeps_the_selected_tests_and_what_they_reach() {
    common::with_typed_std(&common::repository_std(), |library, types| {
        let json = library
            .iter()
            .position(|(m, _, _)| m == "std.json")
            .expect("std.json");
        let options = Options {
            test_modules: vec!["std.json".to_string()],
            tests: types[json].tests().to_vec(),
            ..options()
        };
        let project = generate_tests(&[], &options).unwrap();
        let inputs: Vec<TypedStdModule<'_>> = library
            .iter()
            .zip(types)
            .map(|((m, p, s), t)| (m.clone(), p, s, t))
            .collect();
        let prepared = prepare_std_typed(&inputs, &options).unwrap();
        let generated = prepared.generate_for(&options.substrate_path, &project);
        assert_eq!(
            generated,
            generate_std_for(&inputs, &options, &project).unwrap()
        );
        let std = norm(&generated.lib_rs);
        assert!(std.contains("pub fn __ignite_test_"), "{std}");
        assert!(std.contains("pub fn encode("), "{std}");
        assert!(std.contains("pub fn parse("), "{std}");
        assert!(!std.contains("pub struct Console"), "{std}");
        assert!(!std.contains("pub struct TelnetServer"), "{std}");
    });
}

#[test]
fn a_prepared_template_renders_independent_roots_and_substrates() {
    let library = common::load_std(&common::golden_std());
    let types = common::type_std(&library).unwrap();
    let inputs: Vec<TypedStdModule<'_>> = library
        .iter()
        .zip(&types)
        .map(|((module, program, symbols), types)| (module.clone(), program, symbols, types))
        .collect();
    let options = Options {
        sources: common::std_sources(&common::golden_std()),
        ..options()
    };
    let prepared = prepare_std_typed(&inputs, &options).unwrap();
    let shout = ["ignite_std::text::shout"];
    let line = ["ignite_std::protocol::line::Line::connect"];
    let first = prepared.generate_reachable(&options.substrate_path, &shout);
    let second = prepared.generate_reachable(Path::new("another-substrate"), &line);
    let again = prepared.generate_reachable(&options.substrate_path, &shout);
    assert_eq!(
        first, again,
        "rendering another root must not consume template items"
    );
    assert!(first.lib_rs.contains("pub fn shout("));
    assert!(!first.lib_rs.contains("pub fn pump("));
    assert!(second.lib_rs.contains("pub fn pump("));
    assert!(!second.lib_rs.contains("pub fn shout("));
    assert!(second.cargo_toml.contains("another-substrate"));
    assert!(!first.cargo_toml.contains("another-substrate"));
    assert_eq!(
        first,
        generate_std_reachable(&inputs, &options, &shout).unwrap()
    );
    assert_eq!(
        prepared.generate_all(&options.substrate_path),
        generate_std_typed(&inputs, &options).unwrap()
    );
}
