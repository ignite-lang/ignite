//! Golden files: every `tests/golden/<name>.ignite` is parsed, checked
//! against a standard library and lowered. After collapsing whitespace runs
//! to one space, `src/lib.rs` must equal `<name>.rs`, the first executable
//! `<name>.main.rs`, every further executable `<name>.<executable>.rs`, and,
//! for programs lowered with the golden standard library
//! (`tests/golden/std`), the `ignite-std` crate `<name>.std.rs`.
//!
//! `IGNITE_BLESS=1 cargo test -p ignite-rustgen --test main golden::` rewrites the
//! expected files from the current output.

use crate::common;

use std::path::{Path, PathBuf};

use ignite_rustgen::GeneratedProject;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Programs that must type: their goldens are the typed generator's
/// output. The others fall back to the untyped generator when they do not.
const TYPED: &[&str] = &[
    "terminal",
    "syntax",
    "exits",
    "variants",
    "generics",
    "components",
    "services",
    "modules",
    "targets",
    "bounds",
    "slice16",
    "rest",
    "capabilities",
];

fn lower(stem: &str, std_root: &Path) -> (GeneratedProject, GeneratedProject) {
    let file = golden_dir().join(format!("{stem}.ignite"));
    let substrate = Path::new("../ignite-io");
    if TYPED.contains(&stem) {
        common::lower_typed(&file, std_root, substrate)
    } else {
        common::lower(&file, std_root, substrate)
    }
}

fn norm(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn compare(expected_path: &Path, actual: &str) -> Result<(), String> {
    if std::env::var_os("IGNITE_BLESS").is_some() {
        std::fs::write(expected_path, actual).unwrap();
        return Ok(());
    }
    let expected = std::fs::read_to_string(expected_path)
        .map_err(|e| format!("{}: {e}", expected_path.display()))?;
    if norm(&expected) == norm(actual) {
        Ok(())
    } else {
        Err(format!(
            "{} differs from the generated text:\n{actual}",
            expected_path.display()
        ))
    }
}

/// Compare the program project; with `with_std`, also the std crate.
fn check_program(stem: &str, std_root: &Path, with_std: bool) -> GeneratedProject {
    let dir = golden_dir();
    let (project, std) = lower(stem, std_root);
    let mut failures = Vec::new();
    let mut expect = |path: PathBuf, actual: &str| {
        if let Err(e) = compare(&path, actual) {
            failures.push(e);
        }
    };
    expect(dir.join(format!("{stem}.rs")), &project.lib_rs);
    let main_path = dir.join(format!("{stem}.main.rs"));
    match project.executables.split_first() {
        Some((first, rest)) => {
            expect(main_path, &first.source);
            for executable in rest {
                expect(
                    dir.join(format!("{stem}.{}.rs", executable.name)),
                    &executable.source,
                );
            }
        }
        None => assert!(
            !main_path.exists(),
            "{} exists but the program has no system",
            main_path.display()
        ),
    }
    if with_std {
        expect(dir.join(format!("{stem}.std.rs")), &std.lib_rs);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    project
}

fn executable_names(project: &GeneratedProject) -> Vec<&str> {
    project
        .executables
        .iter()
        .map(|e| e.name.as_str())
        .collect()
}

#[test]
fn terminal() {
    let project = check_program("terminal", &common::repository_std(), false);
    assert_eq!(executable_names(&project), ["terminald", "tn"]);
    assert_eq!(project.package_name, "terminal");
    assert!(
        project
            .cargo_toml
            .contains("ignite-std = { path = \"std\" }")
    );
}

#[test]
fn pure_functions() {
    check_program("pure", &common::golden_std(), false);
}

#[test]
fn record_and_enum_with_match() {
    check_program("shapes", &common::golden_std(), false);
}

/// A server kind binding its listener and a client kind naming another
/// listener as its `Endpoint` target (`docs/slice.md` section 14.5): the
/// client's main builds the endpoint with the `--port` override and binds
/// nothing. `Endpoint` comes from the golden `std.net`, so the program
/// crate depends on `ignite-std`.
#[test]
fn system_with_server_and_client() {
    let project = check_program("services", &common::golden_std(), false);
    assert_eq!(executable_names(&project), ["greeter", "readerc"]);
    assert!(project.cargo_toml.contains("ignite-std"));
    assert!(
        project
            .cargo_toml
            .contains("\n[profile.dev.package.\"ignite-std\"]\ndebug = false\n"),
        "{}",
        project.cargo_toml
    );
    let reader = norm(&project.executables[1].source);
    assert!(
        reader.contains("let __endpoint_back = Endpoint { address: String::from(\"0.0.0.0\"), port: __port.unwrap_or(7001) };"),
        "{reader}"
    );
    assert!(!reader.contains("TcpListener::bind"), "{reader}");
}

#[test]
fn protocol_import_spawn_and_function_values() {
    let project = check_program("modules", &common::golden_std(), true);
    assert_eq!(executable_names(&project), ["echod", "echo"]);
    assert!(
        project
            .cargo_toml
            .contains("ignite-std = { path = \"std\" }")
    );
}

#[test]
fn constants_qualified_patterns_and_io_error() {
    let project = check_program("consts", &common::golden_std(), true);
    assert!(project.executables.is_empty());
}

/// Slice 6 surface syntax (`docs/slice.md` section 10), the program the
/// LLVM golden uses too.
#[test]
fn surface_syntax() {
    let project = check_program("syntax", &common::golden_std(), false);
    assert_eq!(executable_names(&project), ["syntax"]);
}

/// `Process.exit(c)` has type Never and lowers to `ignite_io::exit`, which
/// returns `!`: no `?` is needed, and `?` on it lowers to the call alone.
#[test]
fn process_exit_diverges() {
    let project = check_program("exits", &common::golden_std(), false);
    assert_eq!(executable_names(&project), ["exiter"]);
    let lib = norm(&project.lib_rs);
    assert!(
        lib.contains("Eof => ::ignite_io::exit((5) as i32)"),
        "{lib}"
    );
    assert!(!lib.contains("::ignite_io::Process::exit"), "{lib}");
    assert!(!lib.contains("exit((7) as i32))?"), "{lib}");
}

/// Bare variants of an enum that is not imported, resolved by expected
/// type, name the enum by its path: `ignite_std::modes::Mode` in a program,
/// `crate::modes::Mode` inside `ignite-std`.
#[test]
fn variants_of_enums_not_imported() {
    let std_root = golden_dir().join("variants/std");
    let (project, std) = lower("variants", &std_root);
    check_program("variants", &std_root, true);
    assert!(
        norm(&project.lib_rs).contains("level(&::ignite_std::modes::Mode::Loud)"),
        "{}",
        project.lib_rs
    );
    assert!(
        norm(&std.lib_rs).contains("crate::modes::Mode::Quiet(_) =>"),
        "{}",
        std.lib_rs
    );
}

/// Slice 10 (`docs/slice.md` 14.4, 14.5): a component kind is a record
/// type with `fn K.main`. The generated main creates the store, binds the
/// listener with the `--port` override and the `listening on` log, builds
/// the embedded `banner` and then `server` with the kind's defaults for
/// the omitted fields, and calls `Greeter.main`.
#[test]
fn executable_components() {
    let project = check_program("components", &common::repository_std(), false);
    assert_eq!(executable_names(&project), ["greeterd", "pinger"]);
    let pinger = norm(&project.executables[1].source);
    for expected in [
        "let __port = ::ignite_io::args::port()?;",
        "let __endpoint_ingress = Endpoint { address: String::from(\"127.0.0.1\"), port: __port.unwrap_or(2500) };",
        "let __res_client = Pinger { target: __endpoint_ingress.clone(), executable: String::from(\"pinger\") };",
        "(&__res_client) }))",
    ] {
        assert!(
            pinger.contains(expected),
            "missing `{expected}` in\n{pinger}"
        );
    }
    assert!(!pinger.contains("TcpListener::bind"), "{pinger}");
    let main = norm(&project.executables[0].source);
    for expected in [
        "let __res_seen: ::ignite_io::Store<String> = ::ignite_io::Store::new();",
        "let __port = ::ignite_io::args::port()?;",
        "let __res_ingress = ::ignite_io::TcpListener::bind(String::from(\"127.0.0.1\"), __port.unwrap_or(2500))?;",
        "::ignite_io::log(&format!(\"listening on 127.0.0.1:{}\", __res_ingress.local_port()));",
        "let __res_banner = Banner {",
        "let __res_server = Greeter {",
        "executable: String::from(\"greeterd\")",
        "Greeter__main(__res_server) }))",
    ] {
        assert!(main.contains(expected), "missing `{expected}` in\n{main}");
    }
    let store = main.find("Store::new()").expect(&main);
    let bind = main.find("TcpListener::bind").expect(&main);
    let banner = main.find("let __res_banner").expect(&main);
    let server = main.find("let __res_server").expect(&main);
    assert!(store < bind && bind < banner && banner < server, "{main}");
}

/// Slice 7 (`docs/slice.md` section 11): generic records, enums, fns and
/// roles with the Rust bounds, an effectful fn type, a `Store` resource
/// created first in `main` and named in the `application`, the new
/// builtins, and parameters passed by reference.
#[test]
fn generics_stores_and_builtins() {
    let project = check_program("generics", &common::golden_std(), false);
    assert_eq!(executable_names(&project), ["countersd"]);
    let lib = norm(&project.lib_rs);
    let main = norm(&project.executables[0].source);
    for expected in [
        "pub struct Pair<A: Clone + Send + Sync + 'static, B: Clone + Send + Sync + 'static>",
        "pub enum Choice<T: Clone + Send + Sync + 'static>",
        "impl<T: Clone + Send + Sync + 'static> Labelled<T>",
        "label: Labelled::<i64>::default_label()",
        "pub handle: ::std::sync::Arc<dyn Fn(S, String) -> String + Send + Sync>,",
        "pub fn swap<A: Clone + Send + Sync + 'static, B: Clone + Send + Sync + 'static>(p: Pair<A, B>) -> Pair<B, A>",
        "pub fn serve<S: Clone + Send + Sync + 'static>(mut stream: ::ignite_io::TcpStream, mut app: App<S>)",
        "let id: ::std::sync::Arc<dyn Fn(i64) -> i64 + Send + Sync> = (::std::sync::Arc::new(|__a0: i64| identity::<i64>(__a0)) as ::std::sync::Arc<dyn Fn(i64) -> i64 + Send + Sync>);",
        "counts.values().iter().map(|__a0| show(__a0)).collect::<Vec<_>>()",
        "counts.update(id.clone(), &*(::std::sync::Arc::new(|__a0: i64| bump(&__a0)) as ::std::sync::Arc<dyn Fn(i64) -> i64 + Send + Sync>))",
        "Some(rest) => ::ignite_io::parse_int(&rest),",
        "u8::try_from(n).ok()",
        "(b as i64)",
        "stream.set_idle_timeout(5000)",
        "(app.handle)(app.state.clone(),",
    ] {
        assert!(lib.contains(expected), "missing `{expected}` in\n{lib}");
    }
    let store = main
        .find("let __res_counts: ::ignite_io::Store<i64> = ::ignite_io::Store::new();")
        .expect(&main);
    let bind = main.find("TcpListener::bind").expect(&main);
    let server = main
        .find("let __res_server = Server::<::ignite_io::Store<i64>> { listener: __res_ingress.clone(), application: App { state: __res_counts.clone(), handle: (::std::sync::Arc::new(|__a0: ::ignite_io::Store<i64>, __a1: String| respond(__a0, &__a1)) as ::std::sync::Arc<dyn Fn(::ignite_io::Store<i64>, String) -> String + Send + Sync>) }, executable: String::from(\"countersd\") };")
        .expect(&main);
    let call = main.find("Server__main(&__res_server)").expect(&main);
    assert!(store < bind && bind < server && server < call, "{main}");
}

/// `l.at(i)` and `l[i]` on a list whose elements are not `Copy` in Rust
/// copy the element; on `Bytes` and `List<Int>` they read it.
#[test]
fn element_reads_copy_only_non_copy_elements() {
    let project = common::generate_source(
        "fn first(lines: List<Text>) -> Text = lines.at(0)\n\
         fn second(lines: List<Text>) -> Text { let line = lines[1]\n line }\n\
         fn byte(bytes: Bytes) -> Byte = bytes.at(0)\n",
        &common::golden_std(),
    )
    .unwrap();
    let lib = norm(&project.lib_rs);
    assert!(
        lib.contains("::ignite_io::checked::at(lines, 0, \"test.ignite\", 1, 39).clone()"),
        "{lib}"
    );
    assert!(
        lib.contains(
            "let line = ::ignite_io::checked::at(lines, 1, \"test.ignite\", 2, 51).clone();"
        ),
        "{lib}"
    );
    assert!(
        lib.contains("(*::ignite_io::checked::at(bytes, 0, \"test.ignite\", 4, 33)) }"),
        "{lib}"
    );
}

/// A field of a list element is read through the `&T` that
/// `ignite_io::checked::at` returns: passed by reference and compared in
/// place, matched by reference, copied alone where a value is needed,
/// never after a copy of the whole element. An argument whose list a
/// later argument changes is copied before that argument runs.
#[test]
fn fields_of_elements_borrow_the_element() {
    let project = common::generate_source(
        "type E { key: Bytes, n: Int, name: Text }\n\
         type B { items: List<E> }\n\
         fn size(b: Bytes) -> Int = b.len()\n\
         fn both(a: Bytes, n: Int) -> Int = a.len() + n\n\
         fn by_ref(es: List<E>) -> Int = size(es[0].key)\n\
         fn key(es: List<E>) -> Bytes = es.at(0).key\n\
         fn same(es: List<E>) -> Bool { let s = es[0].key == es[1].key\n s }\n\
         fn count(es: List<E>) -> Int = es[0].n\n\
         fn nested(bs: List<B>) -> Int = size(bs[0].items.at(1).key)\n\
         fn later() -> Int { var es = [E { key = b\"a\", n = 1, name = \"a\" }]\n both(es[0].key, if true { es = []\n 1 } else { 2 }) }\n\
         fn named(es: List<E>) -> Bool = es[0].name == es.at(1).name\n\
         fn kind(es: List<E>) -> Int = match es[0].name { \"a\" => 1\n _ => 2 }\n",
        &common::golden_std(),
    )
    .unwrap();
    let lib = norm(&project.lib_rs);
    for expected in [
        "size(&::ignite_io::checked::at(es, 0, \"test.ignite\", 5, 38).key)",
        "::ignite_io::checked::at(es, 0, \"test.ignite\", 6, 32).key.clone()",
        "(&::ignite_io::checked::at(es, 0, \"test.ignite\", 7, 40).key, &::ignite_io::checked::at(es, 1, \"test.ignite\", 7, 53).key);",
        "::ignite_io::checked::at(es, 0, \"test.ignite\", 9, 32).n",
        "size(&::ignite_io::checked::at(&::ignite_io::checked::at(bs, 0, \"test.ignite\", 10, 38).items, 1, \"test.ignite\", 10, 38).key)",
        "both(&::ignite_io::checked::at(&es, 0, \"test.ignite\", 12, 7).key.clone(), &(if true {",
        "match &::ignite_io::checked::at(es, 0, \"test.ignite\", 15, 37).name {",
        "::ignite_io::checked::at(es, 0, \"test.ignite\", 14, 33).name == ::ignite_io::checked::at(es, 1, \"test.ignite\", 14, 47).name",
    ] {
        assert!(lib.contains(expected), "missing `{expected}` in\n{lib}");
    }
    for copy in [
        ".clone().key",
        ".clone().n",
        ".clone().items",
        ".clone().name",
    ] {
        assert!(!lib.contains(copy), "`{copy}` in\n{lib}");
    }
}

/// A byte string literal (story:rustgen-redundant-clones): by reference a
/// `static` built once, by value one `vec!` of bytes, instead of a
/// `Vec<i64>` collected into a `Vec<u8>` on every evaluation. A list that
/// is not all byte literals keeps the conversion.
#[test]
fn byte_literals_are_built_once() {
    let project = common::generate_source(
        "fn size(b: Bytes) -> Int = b.len()\n\
         fn by_ref() -> Int = size(b\"\\r\\n\")\n\
         fn by_value() -> Bytes = b\"ok\"\n\
         fn appended(out: Bytes) -> Bytes { out.append(b\"!\")\n out }\n\
         fn same(b: Bytes) -> Bool = b == b\"\\xff\"\n\
         fn computed(n: Byte) -> Bytes = Bytes.of([n, 1])\n\
         fn empty() -> Bytes = b\"\"\n",
        &common::golden_std(),
    )
    .unwrap();
    let lib = norm(&project.lib_rs);
    let lit = |items: &str| {
        format!(
            "{{ static BYTES: ::std::sync::LazyLock<Vec<u8>> = ::std::sync::LazyLock::new(|| vec![{items}]); &*BYTES }}"
        )
    };
    for expected in [
        format!("size({})", lit("13u8, 10u8")),
        "pub fn by_value() -> Vec<u8> { vec![111u8, 107u8] }".to_string(),
        format!("out.extend_from_slice({})", lit("33u8")),
        lit("255u8"),
        "(vec![n.clone(), 1]).iter().map(|x| *x as u8).collect::<Vec<u8>>()".to_string(),
        "pub fn empty() -> Vec<u8> { Vec::<u8>::new() }".to_string(),
    ] {
        assert!(lib.contains(&expected), "missing `{expected}` in\n{lib}");
    }
    assert!(!lib.contains("(vec![13, 10])"), "{lib}");
}

/// `Store.update` and `Store.upsert` whose result is discarded, or of which
/// only one field is read (story:rustgen-redundant-clones): `update_read`
/// and `upsert_read` lend the stored value to a read under the lock (`|_|
/// ()`, or a copy of the field) instead of returning a copy of all of it.
/// A result used whole keeps `update` and `upsert`.
#[test]
fn store_results_read_in_place() {
    let project = common::generate_source(
        "type T { n: Int, name: Text }\n\
         fn bump(t: Option<T>) -> T = match t { Some(v) => v\n None => T { n = 0, name = \"\" } }\n\
         fn discarded(s: Store<T>) requires [s.write] { _ = s.upsert(1, |t| bump(t))\n _ = s.update(1, |t| t) }\n\
         fn named(s: Store<T>) -> Int requires [s.write] = s.upsert(1, bump).n\n\
         fn whole(s: Store<T>) -> T requires [s.write] = s.upsert(1, bump)\n\
         fn kept(s: Store<T>) -> Option<T> requires [s.write] = s.update(1, |t| t)\n",
        &common::golden_std(),
    )
    .unwrap();
    let lib = norm(&project.lib_rs);
    for expected in [
        "let _ = s.upsert_read(1, |t: ::std::option::Option<T>| -> T { bump(&t) }, |_| ());",
        "let _ = s.update_read(1, |t: T| -> T { t }, |_| ());",
        "-> i64 { s.upsert_read(1, &*(::std::sync::Arc::new(|__a0: ::std::option::Option<T>| bump(&__a0))",
        "Send + Sync>), |__v| __v.n.clone()) }",
        "-> T { s.upsert(1, &*(::std::sync::Arc::new(",
        "-> ::std::option::Option<T> { s.update(1, |t: T| -> T { t }) }",
    ] {
        assert!(lib.contains(expected), "missing `{expected}` in\n{lib}");
    }
}

/// Read-only parameters that are not `Copy` are references, and one of a
/// `Copy` type is copied once on entry; a parameter the body changes or
/// returns is consumed (`docs/slice.md` 20.3.3) and declared by value,
/// with no entry copy.
#[test]
fn parameters_are_borrowed_unless_changed() {
    let project = common::generate_source(
        "fn count(text: Text, at: Int) -> Int = if text.len() > at { 1 } else { 0 }\n\
         fn same(a: Text, b: Text) -> Bool = a == b\n\
         fn grow(items: List<Int>) -> List<Int> { items.push(1)\n items }\n\
         fn keep(text: Text) -> Text = text\n\
         fn user(text: Text) -> Int = count(text, 2) + count(\"x\", 1)\n",
        &common::golden_std(),
    )
    .unwrap();
    let lib = norm(&project.lib_rs);
    for expected in [
        "pub fn count(text: &String, at: &i64) -> i64 { let mut at = at.clone();",
        "pub fn same(a: &String, b: &String) -> bool { (*a) == (*b) }",
        "pub fn grow(mut items: Vec<i64>) -> Vec<i64> { items.push(1); items }",
        "pub fn keep(text: String) -> String { text }",
        "::ignite_io::checked::add(count(text, &2), count({ static TEXT: ::std::sync::LazyLock<String> = ::std::sync::LazyLock::new(|| String::from(\"x\")); &*TEXT }, &1), \"test.ignite\", 6, 30)",
    ] {
        assert!(lib.contains(expected), "missing `{expected}` in\n{lib}");
    }
}

const ECHO: &str = "import std.net.TcpStream\nimport std.io.IoError\n\
    type Server { listener: TcpListener, port: Int }\n\
    fn Server.main(self: Server) -> Result<Unit, IoError> {\n\
      let stream = self.listener.accept()?\n _ = stream.read(1)?\n Ok(())\n }\n";

/// Every `TcpListener` the component uses is bound before its value is
/// built (`docs/slice.md` section 14.5), also when the listener is named
/// inside another property's expression. A resource named `port` keeps its
/// name: `main`'s own binding is `__port`.
#[test]
fn listeners_are_bound_before_the_component_value() {
    let project = common::generate_source(
        &format!(
            "{ECHO}type Holder {{ listener: TcpListener }}\n\
             fn bound(holder: Holder) -> Int = holder.listener.local_port()\n\
             system S {{\n\
               resource port: TcpListener {{ address = \"127.0.0.1\", port = 1 }}\n\
               component c: Server {{ listener = port, port = bound(Holder {{ listener = port }}) }}\n}}\n"
        ),
        &common::golden_std(),
    );
    let project = project.unwrap_or_else(|d| panic!("{d:?}"));
    let main = norm(&project.executables[0].source);
    let bind = main
        .find("let __res_port = ::ignite_io::TcpListener::bind(")
        .expect(&main);
    let value = main
        .find("let __res_c = Server { listener: __res_port.clone(), port: bound(&(Holder { listener: __res_port.clone() })) };")
        .expect(&main);
    assert!(bind < value, "{main}");
}

/// Slice 11b (`docs/friction.md` rows 32 to 35): a mutating call on an
/// indexed receiver writes through `ignite_io::checked::at_mut`; an enum
/// without payloads derives the `BTreeMap` key traits; `Exposure` is
/// `ignite_io::Exposure`; a record named `Box` is `Box_`, so the boxed
/// field stays `Box<::std::option::Option<Box_>>`.
#[test]
fn target_bugs_of_slice_11b() {
    let project = check_program("targets", &common::golden_std(), false);
    let lib = norm(&project.lib_rs);
    for text in [
        "(*::ignite_io::checked::at_mut(&mut lists, __p1, ",
        ")).push(__p2) }",
        ")).extend_from_slice(&__p4) }",
        ")).items.push(__p6) }",
        "#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)] pub enum Color {",
        "pub fn describe(e: &::ignite_io::Exposure) -> String {",
        "::ignite_io::Exposure::Loopback =>",
        "pub fn pick(open: &bool) -> ::ignite_io::Exposure {",
        "pub struct Box_ {",
        "pub inner: Box<::std::option::Option<Box_>>,",
        "pub fn boxed() -> Box_ {",
        "Box_ { label: String::from(\"outer\"), inner: Box::new(Some(Box_ {",
    ] {
        assert!(lib.contains(text), "no `{text}` in\n{lib}");
    }
}

/// Slice 16 (`docs/slice.md` 20.3.4, 20.0 row 13): a parameter the moves
/// analysis marks consumed is declared by value and the entry copy is gone;
/// a place that moves is passed bare to it, to a role and through a fn
/// value; a place read later is passed as `.clone()`; a read-only aggregate
/// parameter stays `&T`; a named fn wrapped as a fn value passes its owned
/// argument on by value; `adder(1)(2)` calls the call's result.
#[test]
fn last_use_moves_and_expression_calls() {
    let (project, _) = lower("slice16", &common::golden_std());
    let lib = norm(&project.lib_rs);
    for expected in [
        "pub fn step(state: State, x: &i64) -> State { let mut x = x.clone(); let mut next = state;",
        "s = step(s, &i);",
        "pub fn total(state: &State) -> i64",
        "let t = step(s.clone(), &1);",
        "pub fn ufcs() -> State { let s = fresh(); step(s, &2) }",
        "pub fn serve(mut stream: ::ignite_io::TcpStream, mut application: State) -> ::std::result::Result<(), ::ignite_io::IoError> { application.items.push(1);",
        "let s = fresh(); Counter::serve(stream.clone(), s) }",
        "pub fn publish(o: Out) -> Out { let mut p = o;",
        "pub fn advance(st: Pair) -> Pair { Pair { state: st.state, out: publish(st.out) } }",
        "a.state = (a.handle)(a.state, i.clone());",
        "step(__a0, &__a1)",
        "pub fn added() -> i64 { adder(&1)(2) }",
    ] {
        assert!(lib.contains(expected), "missing `{expected}` in\n{lib}");
    }
    for unexpected in [
        "step(s.clone(), &i)",
        "let mut state = state.clone()",
        "a.state.clone()",
    ] {
        assert!(!lib.contains(unexpected), "`{unexpected}` in\n{lib}");
    }
    check_program("slice16", &common::golden_std(), false);
}

/// Row 19 of `docs/slice.md` 20.0: rest patterns (20.5) lower to a length
/// test `len >= k`, suffix elements indexed from the end, and a rest
/// binding that is an owned copy of the elements between, on a list and on
/// `Bytes`. `rest.ignite` is a copy of the LLVM golden program.
#[test]
fn rest_patterns() {
    let (project, _) = lower("rest", &common::golden_std());
    let lib = norm(&project.lib_rs);
    for expected in [
        "__m if (__m.len() == 1) => 1, __m if (__m.len() >= 2) => 2,",
        "__m if (__m.len() >= 1) => { let rest = __m[1..].to_vec();",
        "__m if (__m.len() >= 2) => { let a = ((&__m[0])).clone(); let z = ((&__m[__m.len() - 1])).clone();",
        "(*__p11 == 255) }) => { let rest = __m[2..].to_vec();",
        ".len() >= 2) } => 400,",
    ] {
        assert!(lib.contains(expected), "missing `{expected}` in\n{lib}");
    }
    let copy = std::fs::read_to_string(golden_dir().join("rest.ignite")).unwrap();
    let original = std::fs::read_to_string(
        common::workspace().join("crates/ignite-llvm/tests/golden/rest.ignite"),
    )
    .unwrap();
    assert_eq!(
        copy, original,
        "tests/golden/rest.ignite is a copy of the LLVM golden program"
    );
    check_program("rest", &common::golden_std(), false);
}

/// Row 36 of `docs/slice.md` 21.0: the four capability resources are
/// created after the stores in the order of 21.8 (settings, directories,
/// launchers, entropy), `--tool` and `--root` are read in the generated
/// main, every method lowers to its `ignite_io` call, and the project
/// builds with cargo. `capabilities.ignite` is a copy of the LLVM golden
/// program.
#[test]
fn capabilities() {
    let project = check_program("capabilities", &common::repository_std(), false);
    assert_eq!(executable_names(&project), ["bench"]);
    let main = norm(&project.executables[0].source);
    let order = [
        "::ignite_io::Setting::launch(\"--tool\", Some(\"/bin/sh\"))?",
        "::ignite_io::Directory::launch(\"workspace\", true, \"--root\")?",
        "::ignite_io::Launcher::with_maximum(4)",
        "::ignite_io::Entropy::open()?",
        "let __res_bench = Bench {",
    ];
    let mut at = 0;
    for expected in order {
        let found = main[at..]
            .find(expected)
            .unwrap_or_else(|| panic!("missing `{expected}` after offset {at} in\n{main}"));
        at += found + expected.len();
    }
    let lib = norm(&project.lib_rs);
    for call in [
        ".read_file(",
        ".write_file(",
        ".read_directory(",
        ".create_directory(",
        ".remove(",
        ".remove_tree(",
        ".absolute_path()",
        ".execute(",
        ".start(",
        ".id()",
        ".kill()",
        ".wait()",
        ".random_bytes(",
        ".value()",
    ] {
        assert!(lib.contains(call), "no `{call}` in\n{lib}");
    }
    let copy = std::fs::read_to_string(golden_dir().join("capabilities.ignite")).unwrap();
    let original = std::fs::read_to_string(
        common::workspace().join("crates/ignite-llvm/tests/golden/capabilities.ignite"),
    )
    .unwrap();
    assert_eq!(
        copy, original,
        "tests/golden/capabilities.ignite is a copy of the LLVM golden program"
    );

    let substrate = common::workspace().join("crates/ignite-io");
    let (project, std) = common::lower_typed(
        &golden_dir().join("capabilities.ignite"),
        &common::repository_std(),
        &substrate,
    );
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| common::workspace().join("target"));
    let out = target.join("ignite-rustgen-test").join("capabilities");
    ignite_rustgen::write_project(&project, &out).unwrap();
    ignite_rustgen::write_project(&std, &out.join("std")).unwrap();
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = std::process::Command::new(cargo)
        .arg("build")
        .arg("--manifest-path")
        .arg(out.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&target)
        .env("CARGO_INCREMENTAL", "0")
        .status()
        .unwrap();
    assert!(status.success(), "cargo build of {} failed", out.display());
}

#[test]
fn syntax_copy_is_current() {
    let copy = std::fs::read_to_string(golden_dir().join("syntax.ignite")).unwrap();
    let original = std::fs::read_to_string(
        common::workspace().join("crates/ignite-llvm/tests/golden/syntax.ignite"),
    )
    .unwrap();
    assert_eq!(
        copy, original,
        "tests/golden/syntax.ignite is a copy of the LLVM golden program; refresh it and its goldens"
    );
}

#[test]
fn acceptance_copy_is_current() {
    let copy = std::fs::read_to_string(golden_dir().join("terminal.ignite")).unwrap();
    let original =
        std::fs::read_to_string(common::workspace().join("examples/terminal/terminal.ignite"))
            .unwrap();
    assert_eq!(
        copy, original,
        "tests/golden/terminal.ignite is a copy of examples/terminal/terminal.ignite; refresh it and its goldens"
    );
}

/// Write `project` and `std` under the test run's target directory as
/// `ignite-rustgen-test/<name>` and build them with cargo there.
pub(crate) fn cargo_build(project: &GeneratedProject, std: &GeneratedProject, name: &str) {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| common::workspace().join("target"));
    let out = target.join("ignite-rustgen-test").join(name);
    ignite_rustgen::write_project(project, &out).unwrap();
    ignite_rustgen::write_project(std, &out.join("std")).unwrap();
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = std::process::Command::new(cargo)
        .arg("build")
        .arg("--manifest-path")
        .arg(out.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&target)
        .env("CARGO_INCREMENTAL", "0")
        .status()
        .unwrap();
    assert!(status.success(), "cargo build of {} failed", out.display());
}

/// Slice 14 (`docs/slice.md` 18.10): a parameter bounded by `ByteStream`
/// is `S: ignite_io::ByteStream + ..`; the four stream methods on it are
/// calls of the trait qualified with the type; every use of a bounded fn
/// or role names its type arguments, every call of an unbounded generic
/// fn does not; `PipeStream` and `Pipe.pair()` are the substrate's. The
/// program and its test build both build with cargo.
#[test]
fn bounded_streams() {
    let project = check_program("bounds", &common::golden_std(), false);
    assert_eq!(executable_names(&project), ["relayd"]);
    let lib = norm(&project.lib_rs);
    for expected in [
        "pub fn relay<S: ::ignite_io::ByteStream + Clone + Send + Sync + 'static>(stream: &S) -> IoResult<()>",
        "pub fn serve<S: ::ignite_io::ByteStream + Clone + Send + Sync + 'static>(mut stream: S, mut limit: i64)",
        "pub fn identity<T: Clone + Send + Sync + 'static>(value: T) -> T",
        "<S as ::ignite_io::ByteStream>::read(&stream, 16)",
        "<S as ::ignite_io::ByteStream>::set_idle_timeout(&stream, limit.clone())",
        "<S as ::ignite_io::ByteStream>::close(&stream)",
        "<S as ::ignite_io::ByteStream>::write_all(&copy, ",
        "identity(",
        "once::<T>(inner)",
        "relay::<T>(inner)",
        "relay::<::ignite_io::TcpStream>(stream)",
        "forward::<::ignite_io::TcpStream>(stream)",
        "Relay::serve::<::ignite_io::TcpStream>(",
        "relay::<::ignite_io::PipeStream>(end)",
        "once::<::ignite_io::PipeStream>(end)",
        "forward::<::ignite_io::PipeStream>(end)",
        "Relay::serve::<::ignite_io::PipeStream>(",
        "pub fn over_pipe(end: &::ignite_io::PipeStream) -> IoResult<i64>",
        "end.received_batches()",
        "stream.set_idle_timeout(100)",
        "end.set_idle_timeout(100)",
    ] {
        assert!(lib.contains(expected), "missing `{expected}` in\n{lib}");
    }
    assert!(!lib.contains("identity::<"), "{lib}");
    assert!(!lib.contains("dyn ::ignite_io::ByteStream"), "{lib}");
    let main = norm(&project.executables[0].source);
    assert!(!main.contains("dyn ::ignite_io::ByteStream"), "{main}");

    let file = golden_dir().join("bounds.ignite");
    let substrate = common::workspace().join("crates/ignite-io");
    let substrate = substrate.canonicalize().unwrap_or(substrate);
    let (tests, tests_std) = common::lower_tests(&file, &common::golden_std(), &substrate);
    let test_lib = norm(&tests.lib_rs);
    assert!(
        test_lib.contains("::ignite_io::Pipe::pair()"),
        "no `::ignite_io::Pipe::pair()` in\n{test_lib}"
    );
    let (program, std) = common::lower_typed(&file, &common::golden_std(), &substrate);
    cargo_build(&program, &std, "bounds");
    cargo_build(&tests, &tests_std, "bounds-tests");
}

/// Adversary (slice 14, U3): the turbofish of a bounded call (18.10, D8)
/// spells every type argument with `rust_ty`. Inside `outer<S: ByteStream>`
/// the second argument of `tagged` is the record `S`, which Ignite resolves
/// at `make`'s declaration; the Rust turbofish `tagged::<S, S>` names the
/// type parameter twice. The program checks and types, so it must build.
#[test]
fn bounded_turbofish_keeps_a_record_named_like_a_bound() {
    let source = "package examples.shadow\n\
\n\
import std.net.ByteStream\n\
import std.io.IoError\n\
\n\
type IoResult<T> = Result<T, IoError>\n\
\n\
type S { value: Int }\n\
\n\
fn make() -> S = S { value = 4 }\n\
\n\
fn tagged<T: ByteStream, U>(stream: T, tag: U) -> IoResult<U> requires [stream.write] {\n\
  stream.write_all(\"t\".to_bytes())?\n\
  Ok(tag)\n\
}\n\
\n\
fn outer<S: ByteStream>(stream: S) -> IoResult<Int> requires [stream.write] {\n\
  let record = tagged(stream, make())?\n\
  Ok(record.value)\n\
}\n";
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| common::workspace().join("target"));
    let dir = target.join("ignite-rustgen-test").join("adv-shadow-src");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("shadow.ignite");
    std::fs::write(&file, source).unwrap();
    let substrate = common::workspace().join("crates/ignite-io");
    let substrate = substrate.canonicalize().unwrap_or(substrate);
    let (program, std) = common::lower_typed(&file, &common::golden_std(), &substrate);
    assert!(
        !norm(&program.lib_rs).contains("tagged::<S, S>("),
        "the turbofish names the type parameter `S` for the record `S`:\n{}",
        program.lib_rs
    );
    cargo_build(&program, &std, "adv-shadow");
}

/// The bytes a session reads and writes are borrowed where they are only
/// read: `append` borrows a place it does not alias, an interpolation
/// formats its parts in place, and an element passed by reference is
/// borrowed from its list. An `append` of the receiver itself is still
/// copied, and `write_all` still copies a local even at its last use (a
/// move there is slice 16 U3's). The program checks and types, so it must
/// build.
#[test]
fn session_bytes_are_borrowed_where_they_are_only_read() {
    let source = "package examples.borrows\n\
\n\
import std.net.ByteStream\n\
import std.io.IoError\n\
\n\
type IoResult<T> = Result<T, IoError>\n\
\n\
fn line(prefix: Text, n: Int) -> Bytes = \"{prefix}{n} {{x}}\\r\\n\".to_bytes()\n\
\n\
fn session<S: ByteStream>(stream: S) -> IoResult<Unit> requires [stream.read, stream.write] {\n\
  var buffer = Bytes.empty()\n\
  loop {\n\
    match stream.read(16)? {\n\
      Eof => return Ok(())\n\
      Data(more) => buffer.append(more)\n\
    }\n\
    let fresh = line(\"+\", buffer.len())\n\
    if fresh.len() > 0 {\n\
      stream.write_all(fresh)?\n\
    }\n\
    let kept = buffer.slice(0, 1)\n\
    stream.write_all(kept)?\n\
    buffer.append(kept)\n\
  }\n\
}\n\
\n\
fn twice<S: ByteStream>(stream: S) -> IoResult<Unit> requires [stream.write] {\n\
  var outer = Bytes.empty()\n\
  outer.append(outer)\n\
  var count = 0\n\
  while count < 2 {\n\
    stream.write_all(outer)?\n\
    count = count + 1\n\
  }\n\
  Ok(())\n\
}\n\
\n\
fn size(data: Bytes) -> Int = data.len()\n\
\n\
fn first(args: List<Bytes>) -> Int = size(args.at(0))\n";
    let dir = adversary_sources("borrows-src", &[("borrows.ignite", source)]);
    let file = dir.join("borrows.ignite");
    let (program, std) = common::lower_typed(&file, &common::golden_std(), &adversary_substrate());
    let lib = norm(&program.lib_rs);
    for expected in [
        "(format!(\"{}{} {{x}}\\r\\n\", prefix, n))",
        "size(::ignite_io::checked::at(args, 0, ",
        "buffer.extend_from_slice(&more)",
        "<S as ::ignite_io::ByteStream>::write_all(&stream, fresh.clone())",
        "<S as ::ignite_io::ByteStream>::write_all(&stream, kept.clone())",
        "buffer.extend_from_slice(&kept)",
        "outer.extend_from_slice(&outer.clone())",
        "<S as ::ignite_io::ByteStream>::write_all(&stream, outer.clone())",
    ] {
        assert!(lib.contains(expected), "missing `{expected}` in\n{lib}");
    }
    let borrowed = program
        .lib_rs
        .lines()
        .find(|line| line.contains("size(::ignite_io::checked::at(args"))
        .unwrap_or_default();
    assert!(!borrowed.contains(".clone()"), "{borrowed}");
    cargo_build(&program, &std, "borrows");
}

/// story:rustgen-type-parameter-shadowing: inside `fn outer<S>` the
/// literal `S { .. }` is the record `S`, and inside `fn pick<E>` the
/// variants `E.A` and `E.B(n)` are the enum `E`'s; the generated Rust
/// must not name the type parameter for either. The program checks and
/// types, so it must build.
#[test]
fn record_and_enum_named_like_a_type_parameter() {
    let source = "package examples.shadowlit\n\
\n\
type S { value: Int }\n\
\n\
enum E { A, B(Int) }\n\
\n\
fn outer<S>(item: S) -> Int {\n\
  let record = S { value = 4 }\n\
  record.value\n\
}\n\
\n\
fn pick<E>(item: E) -> Int {\n\
  let chosen = E.B(2)\n\
  match chosen {\n\
    E.A => 0\n\
    E.B(n) => n\n\
  }\n\
}\n";
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| common::workspace().join("target"));
    let dir = target
        .join("ignite-rustgen-test")
        .join("shadow-literal-src");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("shadowlit.ignite");
    std::fs::write(&file, source).unwrap();
    let substrate = common::workspace().join("crates/ignite-io");
    let substrate = substrate.canonicalize().unwrap_or(substrate);
    let (program, std) = common::lower_typed(&file, &common::golden_std(), &substrate);
    cargo_build(&program, &std, "shadow-literal");
}

/// A fresh directory `<target>/ignite-rustgen-test/<name>` holding `files`
/// (path relative to it, text), for the adversary cases below.
pub(crate) fn adversary_sources(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| common::workspace().join("target"));
    let dir = target.join("ignite-rustgen-test").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    for (path, text) in files {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
    }
    dir
}

pub(crate) fn adversary_substrate() -> PathBuf {
    let substrate = common::workspace().join("crates/ignite-io");
    substrate.canonicalize().unwrap_or(substrate)
}

/// Adversary (story:rustgen-type-parameter-shadowing): the clash inside
/// the standard library, which `Generator::item_module_path` spells from
/// the `ignite-std` root. `std.clash` declares a record `S` and an enum
/// `E` and fns whose type parameters are named `S` and `E`; the program
/// calls them, so they are emitted. The record literal, the closure's
/// return type and the enum variants must name the std items.
#[test]
fn std_record_and_enum_named_like_a_type_parameter() {
    let clash = "package std.clash\n\
\n\
type S { value: Int, extra: Int = 1 }\n\
\n\
enum E { A, B(Int) }\n\
\n\
fn make() -> S = S { value = 4 }\n\
\n\
fn literal<S>(item: S) -> Int = S { value = 2 }.value + make().extra\n\
\n\
fn closure<S>(item: S) -> Int {\n\
  let f = |n: Int| S { value = n }\n\
  f(5).value\n\
}\n\
\n\
fn variant<E>(item: E) -> Int = match E.B(3) {\n\
  E.A => 0\n\
  E.B(n) => n\n\
}\n";
    let program = "package examples.stdclash\n\
\n\
import std.clash.{literal, closure, variant}\n\
\n\
fn total() -> Int = literal(1) + closure(1) + variant(1)\n";
    let dir = adversary_sources(
        "adv-std-clash-src",
        &[("std/clash.ignite", clash), ("stdclash.ignite", program)],
    );
    let (program, std) = common::lower_typed(
        &dir.join("stdclash.ignite"),
        &dir.join("std"),
        &adversary_substrate(),
    );
    cargo_build(&program, &std, "adv-std-clash");
}

/// Adversary (story:rustgen-type-parameter-shadowing): a record and an
/// enum imported from a sibling module of the package, shadowed by type
/// parameters in the program crate (`ignite build`'s layout, not the test
/// executable's).
#[test]
fn sibling_record_and_enum_named_like_a_type_parameter() {
    let other = "package examples.sib.other\n\
\n\
type R { v: Int }\n\
\n\
enum F { H, G(Int) }\n\
\n\
fn make_r() -> R = R { v = 1 }\n";
    let main = "package examples.sib\n\
\n\
import examples.sib.other.{R, F, make_r}\n\
\n\
fn lit<R>(item: R) -> Int = R { v = 2 }.v\n\
\n\
fn inferred<R>(item: R) -> Int {\n\
  let xs = [make_r()]\n\
  let f = |n: Int| R { v = n }\n\
  xs.len() + f(3).v\n\
}\n\
\n\
fn variants<F>(item: F) -> Int = match F.G(5) {\n\
  F.H => 0\n\
  F.G(n) => n\n\
}\n\
\n\
fn make_f() -> F = F.G(7)\n\
\n\
fn bare<F>(item: F) -> Int = match make_f() {\n\
  H => 0\n\
  G(n) => n\n\
}\n";
    let dir = adversary_sources(
        "adv-sibling-shadow-src",
        &[("sib.ignite", main), ("other.ignite", other)],
    );
    let (program, std) = common::lower_typed(
        &dir.join("sib.ignite"),
        &common::golden_std(),
        &adversary_substrate(),
    );
    cargo_build(&program, &std, "adv-sibling-shadow");
}

/// Adversary, outside the story's claim (it covers records and enums the
/// program declares or imports; `Generator::unshadowed` leaves
/// `BUILTIN_ENUMS` out): a type parameter named `Option`. The closure's
/// return type is `Option<i64>`, which rustc reads as the parameter
/// (E0109). The LLVM target builds and passes the same program.
#[test]
fn type_parameter_named_like_a_builtin_enum() {
    let source = "package examples.builtinenum\n\
\n\
fn opt<Option>(item: Option) -> Int {\n\
  let parse = |s: Text| Int.parse(s)\n\
  match parse(\"3\") {\n\
    Some(n) => n\n\
    None => 0\n\
  }\n\
}\n\
\n\
fn total() -> Int = opt(1)\n";
    let dir = adversary_sources("adv-builtin-enum-src", &[("builtinenum.ignite", source)]);
    let (program, std) = common::lower_typed(
        &dir.join("builtinenum.ignite"),
        &common::golden_std(),
        &adversary_substrate(),
    );
    cargo_build(&program, &std, "adv-builtin-enum");
}

/// Adversary, outside the story's claim (a builtin type, not a record or
/// enum): inside `fn keep<Text>` the parameter's annotations are spelled
/// by `rust_type`, which maps the name `Text` to `String` before looking
/// at the fn's type parameters, so `keep(5)` passes an `i64` where the
/// Rust fn takes `&String` (E0308). The LLVM target builds and passes the
/// same program.
#[test]
fn type_parameter_named_like_a_builtin_type() {
    let source = "package examples.builtintype\n\
\n\
fn keep<Text>(value: Text) -> Text = value\n\
\n\
fn total() -> Int = keep(5)\n";
    let dir = adversary_sources("adv-builtin-type-src", &[("builtintype.ignite", source)]);
    let (program, std) = common::lower_typed(
        &dir.join("builtintype.ignite"),
        &common::golden_std(),
        &adversary_substrate(),
    );
    cargo_build(&program, &std, "adv-builtin-type");
}

/// Adversary, outside the story's claim (a protocol, not a record or
/// enum): inside `fn call<P>` the role call `P.serve(end, 3)` is the
/// protocol `P`'s, and the Rust `P::serve::<..>(..)` names the type
/// parameter (E0599). The LLVM target builds and passes the same program.
#[test]
fn type_parameter_named_like_a_protocol() {
    let source = "package examples.protoshadow\n\
\n\
import std.net.{ByteStream, PipeStream}\n\
import std.io.IoError\n\
\n\
type IoResult<T> = Result<T, IoError>\n\
\n\
protocol P {\n\
  fn serve<S: ByteStream>(stream: S, application: Int) -> IoResult<Int> requires [stream.write] {\n\
    stream.write_all(\"x\".to_bytes())?\n\
    Ok(application)\n\
  }\n\
}\n\
\n\
fn call<P>(item: P, end: PipeStream) -> IoResult<Int> requires [end.write] = P.serve(end, 3)\n";
    let dir = adversary_sources("adv-protocol-shadow-src", &[("protoshadow.ignite", source)]);
    let (program, std) = common::lower_typed(
        &dir.join("protoshadow.ignite"),
        &common::golden_std(),
        &adversary_substrate(),
    );
    cargo_build(&program, &std, "adv-protocol-shadow");
}

/// `docs/slice.md` D2 and 19.13: a clause roots nothing, so a program whose
/// only use of a catalogue is `conforms [RFC855]` emits none of the
/// catalogue's constants: not the `use` line in the program crate and not
/// the constants or their text in `ignite-std`. The LLVM target drops them
/// in `prune`. A program that reads `RFC855.title` keeps them, and both
/// programs build.
#[test]
fn a_clause_only_catalogue_emits_none_of_its_constants() {
    let serve = "  fn serve(stream: TcpStream, application: fn(Text) -> Text) -> Result<Unit, IoError> requires [stream.write] {\n    return stream.write_all(b\"ok\")\n  }\n";
    let clause = format!(
        "package examples.clauseonly\n\nimport std.rfc.rfc855.{{RFC855}}\n\nprotocol Echo conforms [RFC855] {{\n{serve}}}\n\nfn total() -> Int = 1\n"
    );
    let read = format!(
        "package examples.clauseread\n\nimport std.rfc.rfc855.{{RFC855}}\n\nprotocol Echo conforms [RFC855] {{\n{serve}}}\n\nfn title() -> Text = RFC855.title\n"
    );
    let dir = adversary_sources(
        "clause-only-src",
        &[("clauseonly.ignite", &clause), ("clauseread.ignite", &read)],
    );
    let substrate = adversary_substrate();
    let std_root = common::repository_std();
    let catalogue = [
        "pub fn RFC855(",
        "pub fn RP1_1(",
        "Telnet Option Specifications",
        "carefully documented",
    ];
    let (program, std) = common::lower_typed(&dir.join("clauseonly.ignite"), &std_root, &substrate);
    assert!(!program.lib_rs.contains("RFC855"), "{}", program.lib_rs);
    for text in catalogue {
        assert!(!std.lib_rs.contains(text), "{text} in:\n{}", std.lib_rs);
    }
    cargo_build(&program, &std, "clause-only");
    let (program, std) = common::lower_typed(&dir.join("clauseread.ignite"), &std_root, &substrate);
    assert!(program.lib_rs.contains("RFC855"), "{}", program.lib_rs);
    for text in catalogue {
        assert!(
            std.lib_rs.contains(text),
            "{text} missing from:\n{}",
            std.lib_rs
        );
    }
    cargo_build(&program, &std, "clause-read");
}

/// Names that are Rust keywords but not Ignite keywords pass `ignite check`
/// and build on LLVM: record fields `type`, `match`, `loop` and `fn` (field
/// names may be keywords), fns `dyn` and `super`, params `impl`, `ref` and
/// `self`, a local `move` and a variant `Self`. Rust spells a keyword as a
/// raw identifier (`r#type`); `self`, `Self`, `crate` and `super` cannot be
/// raw and are mangled.
#[test]
fn names_that_are_rust_keywords_build() {
    let source = "package examples.rustkeywords\n\
\n\
type Token { type: Int, match: Text = \"m\", loop: Int, fn: Int = 4 }\n\
\n\
enum Kind { Self, Other(Int) }\n\
\n\
fn dyn(impl: Int, ref: Int) -> Int = impl + ref\n\
\n\
fn super(self: Token) -> Int {\n\
  let move = self.type\n\
  move + self.loop + self.fn\n\
}\n\
\n\
fn kind(k: Kind) -> Int = match k {\n\
  Self => 1\n\
  Other(n) => n\n\
}\n\
\n\
fn total() -> Int {\n\
  let t = Token { type = 1, loop = 2 }\n\
  dyn(t.type, super(t)) + t.match.len() + kind(Kind.Self)\n\
}\n";
    let dir = adversary_sources("rust-keywords-src", &[("rustkeywords.ignite", source)]);
    let (program, std) = common::lower_typed(
        &dir.join("rustkeywords.ignite"),
        &common::golden_std(),
        &adversary_substrate(),
    );
    cargo_build(&program, &std, "rust-keywords");
}

/// Adversary (story:rustgen-builtin-and-role-shadowing): `source` written
/// as `<name>.ignite`, lowered as a test build against `std_root`, built
/// with cargo in its own target directory and its `ignite-test` run. Returns
/// the run's output after asserting the build succeeded.
pub(crate) fn adversary_run_tests(
    name: &str,
    source: &str,
    std_root: &Path,
) -> std::process::Output {
    let file = format!("{name}.ignite");
    let dir = adversary_sources(&format!("{name}-src"), &[(file.as_str(), source)]);
    let (project, std) = common::lower_tests(&dir.join(&file), std_root, &adversary_substrate());
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| common::workspace().join("target"));
    let out = target.join("ignite-rustgen-test").join(name);
    ignite_rustgen::write_project(&project, &out).unwrap();
    ignite_rustgen::write_project(&std, &out.join("std")).unwrap();
    let builds = target
        .join("ignite-rustgen-test")
        .join(format!("{name}-target"));
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = std::process::Command::new(cargo)
        .arg("build")
        .arg("--manifest-path")
        .arg(out.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&builds)
        .env("CARGO_INCREMENTAL", "0")
        .status()
        .unwrap();
    assert!(status.success(), "cargo build of {} failed", out.display());
    let executable = builds.join("debug").join(format!(
        "{}{}",
        ignite_rustgen::TEST_EXECUTABLE,
        std::env::consts::EXE_SUFFIX
    ));
    std::process::Command::new(executable).output().unwrap()
}

/// [`adversary_run_tests`], asserting that every test passed and that
/// `passed` of them ran.
pub(crate) fn adversary_tests_pass(name: &str, source: &str, std_root: &Path, passed: usize) {
    let output = adversary_run_tests(name, source, std_root);
    let shown = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{shown}");
    assert!(shown.contains(&format!("{passed} passed")), "{shown}");
}

/// Adversary: the 2018+ and reserved Rust keywords that are not Ignite
/// keywords, as record fields (with defaults), an enum's variants, a fn,
/// a generic fn and its type parameter, params, locals, a closure param, a
/// `for` binding, payload bindings and a UFCS call. The test build runs and
/// asserts the values, so a keyword spelled the wrong way cannot pass.
#[test]
fn adversary_reserved_and_edition_keywords_in_every_position_run() {
    let source = "package examples.kwa\n\
\n\
type Token { type: Int, async: Int = 1, dyn: Int = 2, try: Int = 3, gen: Int = 4, abstract: Int = 5, become: Int = 6, box: Int = 7, final: Int = 8, macro: Int = 9, override: Int = 10, priv: Int = 11, typeof: Int = 12, unsized: Int = 13, virtual: Int = 14, yield: Int = 15 }\n\
\n\
enum Self { become(Int), final, yield(Int) }\n\
\n\
fn macro(override: Int, priv: Int) -> Int {\n\
  let typeof = override + priv\n\
  let unsized = |virtual: Int| virtual * 2\n\
  unsized(typeof)\n\
}\n\
\n\
fn gen<box>(try: box) -> box = try\n\
\n\
fn sum(xs: List<Int>) -> Int {\n\
  var async = 0\n\
  for dyn in xs {\n\
    async = async + dyn\n\
  }\n\
  async\n\
}\n\
\n\
fn pick(a: Self) -> Int = match a {\n\
  become(yield) => yield\n\
  Self.final => 0\n\
  Self.yield(box) => box + 100\n\
}\n\
\n\
fn all(t: Token) -> Int = t.type + t.async + t.dyn + t.try + t.gen + t.abstract + t.become + t.box + t.final + t.macro + t.override + t.priv + t.typeof + t.unsized + t.virtual + t.yield\n\
\n\
test \"keywords in every position\" {\n\
  let t = Token { type = 0 }\n\
  assert all(t) == 120\n\
  assert macro(2, 3) == 10\n\
  assert gen(7) == 7\n\
  assert sum([1, 2, 3]) == 6\n\
  assert pick(Self.become(4)) == 4\n\
  assert pick(Self.final) == 0\n\
  assert pick(Self.yield(1)) == 101\n\
  assert 3.macro(4) == 14\n\
}\n";
    adversary_tests_pass("adv-kw-positions", source, &common::golden_std(), 1);
}

/// Adversary, inside 995907a's claim (`self` "stays mangled"): `ident`
/// spells the field `self` as `self_`, the name of the record's other
/// field, so the struct declares `self_` twice (E0124) and the literal
/// sets it twice (E0062). `ignite check` accepts the program.
#[test]
fn adversary_a_field_self_beside_a_field_self_underscore_builds() {
    let source = "package examples.kwb\n\
\n\
type Pair { self: Int, self_: Int }\n\
\n\
test \"a field self beside a field self_\" {\n\
  let p = Pair { self = 1, self_ = 2 }\n\
  assert p.self + p.self_ == 3\n\
}\n";
    adversary_tests_pass("adv-kw-self-field", source, &common::golden_std(), 1);
}

/// Adversary, inside 995907a's claim: the param `self` is `self_` in Rust,
/// so the local `let self_ = 10` shadows the param and `self + self_` is
/// `10 + 10`. The program builds and the test fails at run time
/// (`shadow(1) == 11`): a silent miscompile, not a rustc error.
#[test]
fn adversary_a_param_self_beside_a_local_self_underscore_keeps_both() {
    let source = "package examples.kwc\n\
\n\
fn shadow(self: Int) -> Int {\n\
  let self_ = 10\n\
  self + self_\n\
}\n\
\n\
test \"a param self beside a local self_\" {\n\
  assert shadow(1) == 11\n\
}\n";
    adversary_tests_pass("adv-kw-self-local", source, &common::golden_std(), 1);
}

/// Adversary, inside 995907a's claim: the fns `super` and `super_` are both
/// `super_` in Rust (E0428).
#[test]
fn adversary_fns_super_and_super_underscore_build() {
    let source = "package examples.kwe\n\
\n\
fn super() -> Int = 1\n\
\n\
fn super_() -> Int = 2\n\
\n\
test \"fns super and super_\" {\n\
  assert super() + super_() == 3\n\
}\n";
    adversary_tests_pass("adv-kw-super-fns", source, &common::golden_std(), 1);
}

/// Adversary, inside 995907a's claim: the params `self` and `self_` are
/// both `self_` in Rust (E0415).
#[test]
fn adversary_params_self_and_self_underscore_build() {
    let source = "package examples.kwf\n\
\n\
fn twice(self: Int, self_: Int) -> Int = self * 10 + self_\n\
\n\
test \"params self and self_\" {\n\
  assert twice(1, 2) == 12\n\
}\n";
    adversary_tests_pass("adv-kw-self-params", source, &common::golden_std(), 1);
}

/// `tests/golden/exits.ignite` (a system with one executable) under the
/// package `examples.<package>`, whose `stop_tried` calls `code()` from the
/// sibling module `examples.<package>.<sibling>`.
pub(crate) fn adversary_package_named(package: &str, sibling: &str) -> PathBuf {
    let main = include_str!("golden/exits.ignite")
        .replace(
            "package golden.exits",
            &format!("package examples.{package}\n\nimport examples.{package}.{sibling}.{{code}}"),
        )
        .replace("Process.exit(7)?", "Process.exit(code())?");
    let other = format!("package examples.{package}.{sibling}\n\nfn code() -> Int = 7\n");
    let dir = adversary_sources(
        &format!("adv-package-{package}-src"),
        &[
            (format!("{package}.ignite").as_str(), main.as_str()),
            (format!("{sibling}.ignite").as_str(), other.as_str()),
        ],
    );
    dir.join(format!("{package}.ignite"))
}

/// Adversary, inside 995907a's claim (every Rust keyword a raw identifier):
/// a package whose last segment is `async`, with a sibling module `dyn`.
/// The module path is `r#dyn`, but the executable's `use async::*;` and
/// `use async::r#dyn::code;` spell the crate name bare (`executable_main`,
/// `Layout::in_bin`), a parse error in rustc. `ignite build --target llvm`
/// builds the same program.
#[test]
fn adversary_a_package_named_async_builds() {
    let file = adversary_package_named("async", "dyn");
    let (program, std) = common::lower_typed(&file, &common::golden_std(), &adversary_substrate());
    cargo_build(&program, &std, "adv-package-async");
}

/// Adversary, inside 995907a's claim (`self` mangled): a package whose last
/// segment is `self`, with a sibling module `super`. The crate is named
/// `self`, and the executable's `use self::*;` imports the bin into itself
/// (E0432); `self` cannot be raw, so the crate needs another name.
/// `ignite build --target llvm` builds the same program.
#[test]
fn adversary_a_package_named_self_builds() {
    let file = adversary_package_named("self", "super");
    let (program, std) = common::lower_typed(&file, &common::golden_std(), &adversary_substrate());
    cargo_build(&program, &std, "adv-package-self");
}

/// Adversary: type parameters named `Result`, `List`, `Map`, `Bytes`,
/// `Int` and `Bool`. `Result` shadows the builtin enum the closure
/// returns; the others shadow builtin types in the signature. The values
/// are asserted at run time.
#[test]
fn adversary_type_parameters_named_like_builtins_run() {
    let source = "package examples.tpa\n\
\n\
import std.option.{ok_or}\n\
\n\
fn res<Result>(item: Result) -> Int {\n\
  let parse = |s: Text| Int.parse(s).ok_or(\"bad\")\n\
  match parse(\"3\") {\n\
    Ok(n) => n\n\
    Err(e) => 0\n\
  }\n\
}\n\
\n\
fn lst<List>(item: List) -> Int {\n\
  let xs = [1, 2]\n\
  xs.len()\n\
}\n\
\n\
fn mp<Map>(item: Map) -> Map = item\n\
\n\
fn byt<Bytes>(item: Bytes) -> Bytes = item\n\
\n\
fn int<Int>(item: Int) -> Int = item\n\
\n\
fn bol<Bool>(item: Bool) -> Bool = item\n\
\n\
test \"type parameters named like builtins\" {\n\
  assert res(1) == 3\n\
  assert lst(\"x\") == 2\n\
  assert mp(4) == 4\n\
  assert byt(5) == 5\n\
  assert int(\"i\") == \"i\"\n\
  assert bol(6) == 6\n\
}\n";
    adversary_tests_pass("adv-tp-builtins", source, &common::repository_std(), 1);
}

/// Adversary: a type parameter named like a program's type alias (`Num`),
/// one named `Text` inside closures (an annotated closure param and a
/// closure returning `Text` by interpolation) and as a generic record's
/// field beside an `Option<Int>` field with a default.
#[test]
fn adversary_type_parameters_named_like_an_alias_or_text_in_closures_and_records_run() {
    let source = "package examples.tpb\n\
\n\
type Num = Int\n\
\n\
type Holder<Text> { value: Text, label: Option<Int> = None }\n\
\n\
fn alias<Num>(item: Num) -> Num = item\n\
\n\
fn real() -> Num = 3\n\
\n\
fn closure<Text>(item: Text) -> Int {\n\
  let f = |t: Text| 1\n\
  let g = |s: Int| \"n={s}\"\n\
  f(item) + g(2).len()\n\
}\n\
\n\
fn hold<Text>(item: Text) -> Holder<Text> = Holder { value = item }\n\
\n\
test \"type parameters named like an alias or a builtin inside closures and records\" {\n\
  assert alias(\"a\") == \"a\"\n\
  assert real() == 3\n\
  assert closure(5) == 4\n\
  assert hold(7).value == 7\n\
}\n";
    adversary_tests_pass("adv-tp-alias-text", source, &common::golden_std(), 1);
}

/// Adversary, outside the story's claim (not a keyword, not a type
/// parameter): in Ignite a lowercase name in a pattern binds the value
/// (`PatternKind::Binding`), also when the matched enum has a lowercase
/// variant of that name. Rust's deny-by-default `bindings_with_variant_name`
/// rejects the generated `fin => 0` (E0170). The LLVM target builds and
/// passes the same program.
#[test]
fn adversary_a_binding_named_like_a_lowercase_variant_builds() {
    let source = "package examples.kwg\n\
\n\
enum Shape { fin, Other(Int) }\n\
\n\
fn pick(s: Shape) -> Int = match s {\n\
  Other(n) => n\n\
  fin => 0\n\
}\n\
\n\
test \"a lowercase binding named like a lowercase variant\" {\n\
  assert pick(Shape.Other(4)) == 4\n\
  assert pick(Shape.fin) == 0\n\
}\n";
    adversary_tests_pass("adv-binding-variant", source, &common::golden_std(), 1);
}

/// Adversary: a role whose type parameter is named `Text` and holds an
/// annotated `Option<Int>` local, called through a fn whose type parameter
/// is named `Option`; the role writes into a pipe and the result is
/// asserted at run time.
#[test]
fn adversary_a_role_type_parameter_named_text_called_under_option_runs() {
    let source = "package examples.tpc\n\
\n\
import std.net.{ByteStream, Pipe, PipeStream}\n\
import std.io.IoError\n\
import std.option.{unwrap_or}\n\
\n\
type IoResult<T> = Result<T, IoError>\n\
\n\
protocol P {\n\
  fn serve<Text: ByteStream>(stream: Text, application: Int) -> IoResult<Int> requires [stream.write] {\n\
    let label: Option<Int> = Some(application)\n\
    stream.write_all(\"x\".to_bytes())?\n\
    Ok(label.unwrap_or(0))\n\
  }\n\
}\n\
\n\
fn call<Option>(item: Option, end: PipeStream) -> IoResult<Int> requires [end.write] = P.serve(end, 3)\n\
\n\
test \"a role whose type parameter is named Text and a caller whose is named Option\" {\n\
  let (a, b) = Pipe.pair()\n\
  let n = match call(1, a) {\n\
    Ok(v) => v\n\
    Err(e) => 0\n\
  }\n\
  assert n == 3\n\
}\n";
    adversary_tests_pass("adv-tp-role-text", source, &common::repository_std(), 1);
}

/// Adversary (9bbd4b2): one import of three catalogue constants, of which
/// `RFC855` is named only by the `conforms` clause and its status clauses,
/// `RP1_2` is read by a fn and `RP1_1` only by a test. The program crate
/// keeps the two read and drops `RFC855`; `ignite-std` keeps `RP1_1` and
/// `RP1_2` without the catalogue's title or the other requirements.
#[test]
fn adversary_a_clause_beside_read_constants_keeps_only_the_read_ones() {
    let source = "package examples.cla\n\
\n\
import std.rfc.rfc855.{RFC855, RP1_1, RP1_2}\n\
\n\
protocol Echo conforms [RFC855] {\n\
  fn serve(stream: TcpStream, application: fn(Text) -> Text) -> Result<Unit, IoError> requires [stream.write] {\n\
    return stream.write_all(b\"ok\")\n\
  }\n\
\n\
  not_applicable RFC855.RP1_2 \"no options\"\n\
  not_applicable RFC855.RP1_3 \"no options\"\n\
  not_applicable RFC855.RP2_1 \"no options\"\n\
  not_applicable RFC855.RP3_1 \"no options\"\n\
  not_applicable RFC855.RP3_2 \"no options\"\n\
}\n\
\n\
fn section() -> Text = RP1_2.section\n\
\n\
test \"a clause-only constant beside read ones\" {\n\
  assert section() == \"p1\"\n\
  assert RP1_1.ordinal == 1\n\
}\n";
    let dir = adversary_sources("adv-clause-mixed-src", &[("cla.ignite", source)]);
    let (program, std) = common::lower_tests(
        &dir.join("cla.ignite"),
        &common::repository_std(),
        &adversary_substrate(),
    );
    assert!(
        program
            .lib_rs
            .contains("use ::ignite_std::rfc::rfc855::{RP1_1, RP1_2};"),
        "{}",
        program.lib_rs
    );
    assert!(!program.lib_rs.contains("RFC855"), "{}", program.lib_rs);
    for kept in ["pub fn RP1_1(", "pub fn RP1_2("] {
        assert!(
            std.lib_rs.contains(kept),
            "{kept} missing from:\n{}",
            std.lib_rs
        );
    }
    for dropped in [
        "pub fn RFC855(",
        "pub fn RP1_3(",
        "Telnet Option Specifications",
    ] {
        assert!(
            !std.lib_rs.contains(dropped),
            "{dropped} in:\n{}",
            std.lib_rs
        );
    }
    adversary_tests_pass("adv-clause-mixed", source, &common::repository_std(), 1);
}
