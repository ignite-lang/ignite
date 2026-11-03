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
    let reader = norm(&project.executables[1].source);
    assert!(
        reader.contains("let back_endpoint = Endpoint { address: String::from(\"0.0.0.0\"), port: port.unwrap_or(7001) };"),
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
    assert!(lib.contains("Eof => ignite_io::exit((5) as i32)"), "{lib}");
    assert!(!lib.contains("ignite_io::Process::exit"), "{lib}");
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
        norm(&project.lib_rs).contains("level(&ignite_std::modes::Mode::Loud)"),
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
        "let port = ignite_io::args::port()?;",
        "let ingress_endpoint = Endpoint { address: String::from(\"127.0.0.1\"), port: port.unwrap_or(2500) };",
        "let client = Pinger { target: ingress_endpoint.clone(), executable: String::from(\"pinger\") };",
        "(&client) }))",
    ] {
        assert!(
            pinger.contains(expected),
            "missing `{expected}` in\n{pinger}"
        );
    }
    assert!(!pinger.contains("TcpListener::bind"), "{pinger}");
    let main = norm(&project.executables[0].source);
    for expected in [
        "let seen: ignite_io::Store<String> = ignite_io::Store::new();",
        "let port = ignite_io::args::port()?;",
        "let ingress = ignite_io::TcpListener::bind(String::from(\"127.0.0.1\"), port.unwrap_or(2500))?;",
        "ignite_io::log(&format!(\"listening on 127.0.0.1:{}\", ingress.local_port()));",
        "let banner = Banner {",
        "let server = Greeter {",
        "executable: String::from(\"greeterd\")",
        "(&server) }))",
    ] {
        assert!(main.contains(expected), "missing `{expected}` in\n{main}");
    }
    let store = main.find("Store::new()").expect(&main);
    let bind = main.find("TcpListener::bind").expect(&main);
    let banner = main.find("let banner").expect(&main);
    let server = main.find("let server").expect(&main);
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
        "pub handle: std::sync::Arc<dyn Fn(S, String) -> String + Send + Sync>,",
        "pub fn swap<A: Clone + Send + Sync + 'static, B: Clone + Send + Sync + 'static>(p: &Pair<A, B>) -> Pair<B, A>",
        "pub fn serve<S: Clone + Send + Sync + 'static>(mut stream: ignite_io::TcpStream, mut app: App<S>)",
        "let id: std::sync::Arc<dyn Fn(i64) -> i64 + Send + Sync> = (std::sync::Arc::new(|__a0: i64| identity::<i64>(&__a0)) as std::sync::Arc<dyn Fn(i64) -> i64 + Send + Sync>);",
        "counts.values().iter().cloned().map(&*(std::sync::Arc::new(|__a0: i64| show(&__a0)) as std::sync::Arc<dyn Fn(i64) -> String + Send + Sync>)).collect::<Vec<_>>()",
        "counts.update(id.clone(), &*(std::sync::Arc::new(|__a0: i64| bump(&__a0)) as std::sync::Arc<dyn Fn(i64) -> i64 + Send + Sync>))",
        "Some(rest.clone()).filter(|t: &String| !t.starts_with('+')).and_then(|t| t.parse::<i64>().ok())",
        "u8::try_from(n).ok()",
        "(b as i64)",
        "stream.set_idle_timeout(5000)",
        "(app.handle)(app.state.clone(),",
    ] {
        assert!(lib.contains(expected), "missing `{expected}` in\n{lib}");
    }
    let store = main
        .find("let counts: ignite_io::Store<i64> = ignite_io::Store::new();")
        .expect(&main);
    let bind = main.find("TcpListener::bind").expect(&main);
    let server = main
        .find("let server = Server::<ignite_io::Store<i64>> { listener: ingress.clone(), application: App { state: counts.clone(), handle: (std::sync::Arc::new(|__a0: ignite_io::Store<i64>, __a1: String| respond(&__a0, &__a1)) as std::sync::Arc<dyn Fn(ignite_io::Store<i64>, String) -> String + Send + Sync>) }, executable: String::from(\"countersd\") };")
        .expect(&main);
    let call = main.find("Server__main(&server)").expect(&main);
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
        lib.contains("ignite_io::checked::at(lines, 0, \"test.ignite\", 1, 39).clone()"),
        "{lib}"
    );
    assert!(
        lib.contains(
            "let line = ignite_io::checked::at(lines, 1, \"test.ignite\", 2, 51).clone();"
        ),
        "{lib}"
    );
    assert!(
        lib.contains("(*ignite_io::checked::at(bytes, 0, \"test.ignite\", 4, 33)) }"),
        "{lib}"
    );
}

/// Read-only parameters that are not `Copy` are references; a parameter
/// the body changes, and one of a `Copy` type, is copied once on entry.
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
        "pub fn grow(items: &Vec<i64>) -> Vec<i64> { let mut items = items.clone(); items.push(1); items }",
        "pub fn keep(text: &String) -> String { text.clone() }",
        "ignite_io::checked::add(count(text, &2), count(&String::from(\"x\"), &1), \"test.ignite\", 6, 30)",
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
/// inside another property's expression. A resource named like one of
/// `main`'s own bindings gets a trailing `_`.
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
        .find("let port_ = ignite_io::TcpListener::bind(")
        .expect(&main);
    let value = main
        .find("let c = Server { listener: port_.clone(), port: bound(&(Holder { listener: port_.clone() })) };")
        .expect(&main);
    assert!(bind < value, "{main}");
}

/// Slice 11b (`docs/friction.md` rows 32 to 35): a mutating call on an
/// indexed receiver writes through `ignite_io::checked::at_mut`; an enum
/// without payloads derives the `BTreeMap` key traits; `Exposure` is
/// `ignite_io::Exposure`; a record named `Box` is `Box_`, so the boxed
/// field stays `Box<Option<Box_>>`.
#[test]
fn target_bugs_of_slice_11b() {
    let project = check_program("targets", &common::golden_std(), false);
    let lib = norm(&project.lib_rs);
    for text in [
        "(*ignite_io::checked::at_mut(&mut lists, __p1, ",
        ")).push(__p2) }",
        ")).extend_from_slice(&__p4) }",
        ")).items.push(__p6) }",
        "#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)] pub enum Color {",
        "pub fn describe(e: &ignite_io::Exposure) -> String {",
        "ignite_io::Exposure::Loopback =>",
        "pub fn pick(open: &bool) -> ignite_io::Exposure {",
        "pub struct Box_ {",
        "pub inner: Box<Option<Box_>>,",
        "pub fn boxed() -> Box_ {",
        "Box_ { label: String::from(\"outer\"), inner: Box::new(Some(Box_ {",
    ] {
        assert!(lib.contains(text), "no `{text}` in\n{lib}");
    }
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
fn cargo_build(project: &GeneratedProject, std: &GeneratedProject, name: &str) {
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
        "pub fn relay<S: ignite_io::ByteStream + Clone + Send + Sync + 'static>(stream: &S) -> IoResult<()>",
        "pub fn serve<S: ignite_io::ByteStream + Clone + Send + Sync + 'static>(mut stream: S, mut limit: i64)",
        "pub fn identity<T: Clone + Send + Sync + 'static>(value: &T) -> T",
        "<S as ignite_io::ByteStream>::read(&stream, 16)",
        "<S as ignite_io::ByteStream>::set_idle_timeout(&stream, limit.clone())",
        "<S as ignite_io::ByteStream>::close(&stream)",
        "<S as ignite_io::ByteStream>::write_all(&copy, ",
        "identity(",
        "once::<T>(inner)",
        "relay::<T>(inner)",
        "relay::<ignite_io::TcpStream>(stream)",
        "forward::<ignite_io::TcpStream>(stream)",
        "Relay::serve::<ignite_io::TcpStream>(",
        "relay::<ignite_io::PipeStream>(end)",
        "once::<ignite_io::PipeStream>(end)",
        "forward::<ignite_io::PipeStream>(end)",
        "Relay::serve::<ignite_io::PipeStream>(",
        "pub fn over_pipe(end: &ignite_io::PipeStream) -> IoResult<i64>",
        "end.received_batches()",
        "stream.set_idle_timeout(100)",
        "end.set_idle_timeout(100)",
    ] {
        assert!(lib.contains(expected), "missing `{expected}` in\n{lib}");
    }
    assert!(!lib.contains("identity::<"), "{lib}");
    assert!(!lib.contains("dyn ignite_io::ByteStream"), "{lib}");
    let main = norm(&project.executables[0].source);
    assert!(!main.contains("dyn ignite_io::ByteStream"), "{main}");

    let file = golden_dir().join("bounds.ignite");
    let substrate = common::workspace().join("crates/ignite-io");
    let substrate = substrate.canonicalize().unwrap_or(substrate);
    let (tests, tests_std) = common::lower_tests(&file, &common::golden_std(), &substrate);
    let test_lib = norm(&tests.lib_rs);
    assert!(
        test_lib.contains("ignite_io::Pipe::pair()"),
        "no `ignite_io::Pipe::pair()` in\n{test_lib}"
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
fn adversary_sources(name: &str, files: &[(&str, &str)]) -> PathBuf {
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

fn adversary_substrate() -> PathBuf {
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
