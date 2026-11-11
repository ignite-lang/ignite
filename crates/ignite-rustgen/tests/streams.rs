//! Slice 14 row 10 (`docs/slice.md` 18.0, 18.10, D8 to D10): the terminal's
//! reachable `ignite-std`, generated from the repository's standard
//! library, calls the `TcpStream` instances of the generic Telnet roles by
//! turbofish, so dispatch is static: no `dyn ignite_io::ByteStream`, and
//! every `dyn` of the program crate and of `ignite-std` is a fn value's
//! `dyn Fn` (13.3).

use std::path::Path;

use crate::common;

/// The text of `pub fn name(` in `lib`, up to the next `pub fn`.
fn function<'a>(lib: &'a str, name: &str) -> &'a str {
    let start = lib
        .find(&format!("pub fn {name}("))
        .unwrap_or_else(|| panic!("no pub fn {name} in:\n{lib}"));
    let rest = &lib[start + 1..];
    let end = rest.find("pub fn ").map_or(lib.len(), |i| start + 1 + i);
    &lib[start..end]
}

/// Every `dyn` of `text` that is not a `dyn Fn`.
fn other_dyns(text: &str) -> Vec<String> {
    text.match_indices("dyn ")
        .map(|(i, _)| text[i..].chars().take(40).collect::<String>())
        .filter(|found| !found.starts_with("dyn Fn("))
        .collect()
}

#[test]
fn the_terminal_calls_the_tcp_instances_of_the_telnet_roles() {
    let terminal = common::workspace().join("examples/terminal/terminal.ignite");
    let (program, std) = common::lower_typed(
        &terminal,
        &common::repository_std(),
        Path::new("../ignite-io"),
    );
    let server = function(&std.lib_rs, "TelnetServer__main");
    assert!(
        server.contains("Telnet::serve::<::ignite_io::TcpStream>("),
        "{server}"
    );
    let client = function(&std.lib_rs, "TelnetClient__main");
    assert!(
        client.contains("Telnet::connect::<::ignite_io::TcpStream>("),
        "{client}"
    );
    let mut texts = vec![
        ("ignite-std", std.lib_rs.as_str()),
        ("program", program.lib_rs.as_str()),
    ];
    texts.extend(
        program
            .executables
            .iter()
            .map(|e| (e.name.as_str(), e.source.as_str())),
    );
    for (name, text) in texts {
        assert!(!text.contains("dyn ::ignite_io::ByteStream"), "{name}");
        assert!(!text.contains("dyn ByteStream"), "{name}");
        let others = other_dyns(text);
        assert!(others.is_empty(), "{name} has {others:?}");
    }
    assert!(
        std.lib_rs.contains("dyn Fn("),
        "the console's fn value is a dyn Fn"
    );
}

/// TLS client sessions (`docs/slice.md` 38):
/// a program that names `Tls.client` or `TlsStream` turns on the substrate's
/// feature `tls`; one that does not builds without rustls and ring.
#[test]
fn only_a_program_with_tls_turns_on_the_substrate_feature() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("streams-tls-feature");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("dial.ignite");
    std::fs::write(
        &file,
        "import std.tls.{TlsOptions}\n\n\
         fn dial(port: Int) -> Result<TlsStream, IoError> {\n  \
         let stream = TcpStream.connect(\"127.0.0.1\", port)?\n  \
         Tls.client(stream, \"localhost\", TlsOptions { alpn = [\"http/1.1\"] })\n}\n",
    )
    .unwrap();
    let (program, std) =
        common::lower_typed(&file, &common::repository_std(), Path::new("../ignite-io"));
    assert!(
        program.lib_rs.contains("::ignite_io::TlsStream::client("),
        "{}",
        program.lib_rs
    );
    assert!(
        program
            .cargo_toml
            .contains("ignite-io = { path = \"../ignite-io\", features = [\"tls\"] }"),
        "{}",
        program.cargo_toml
    );
    assert!(!std.cargo_toml.contains("tls"), "{}", std.cargo_toml);

    let terminal = common::workspace().join("examples/terminal/terminal.ignite");
    let (program, std) = common::lower_typed(
        &terminal,
        &common::repository_std(),
        Path::new("../ignite-io"),
    );
    assert!(
        !program.cargo_toml.contains("tls"),
        "{}",
        program.cargo_toml
    );
    assert!(!std.cargo_toml.contains("tls"), "{}", std.cargo_toml);
}

/// Child processes and pipes (`docs/slice.md` 21, story `build-time-back`):
/// a program that names a `Launcher`, a `ChildProcess` or a pipe turns on
/// the substrate's feature `process`; one that does not builds
/// `ignite-io` without its `child` and `pipe` modules.
#[test]
fn only_a_program_with_processes_or_pipes_turns_on_the_substrate_feature() {
    let process = "ignite-io = { path = \"../ignite-io\", features = [\"process\"] }";
    let orphans = common::workspace().join("crates/ignite/tests/fixtures/slice17/orphans.ignite");
    let (program, std) = common::lower_typed(
        &orphans,
        &common::repository_std(),
        Path::new("../ignite-io"),
    );
    assert!(
        program.cargo_toml.contains(process),
        "{}",
        program.cargo_toml
    );
    assert!(!std.cargo_toml.contains("process"), "{}", std.cargo_toml);

    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("streams-process-feature");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("pair.ignite");
    std::fs::write(
        &file,
        "import std.net.{PipeStream}\n\n\
         fn end(stream: PipeStream) -> Result<Unit, IoError> = stream.close()\n",
    )
    .unwrap();
    let (program, _) =
        common::lower_typed(&file, &common::repository_std(), Path::new("../ignite-io"));
    assert!(
        program.cargo_toml.contains(process),
        "{}",
        program.cargo_toml
    );

    // A program that names only a child process it was given.
    let file = dir.join("child.ignite");
    std::fs::write(&file, "fn pid(child: ChildProcess) -> Int = child.id()\n").unwrap();
    let (program, _) =
        common::lower_typed(&file, &common::repository_std(), Path::new("../ignite-io"));
    assert!(
        program.lib_rs.contains("::ignite_io::ChildProcess"),
        "{}",
        program.lib_rs
    );
    for other in ["Launcher", "ChildExit", "Invocation", "Pipe"] {
        assert!(
            !program.lib_rs.contains(&format!("::ignite_io::{other}")),
            "{other}: {}",
            program.lib_rs
        );
    }
    assert!(
        program.cargo_toml.contains(process),
        "{}",
        program.cargo_toml
    );

    for example in ["todo/todo", "terminal/terminal"] {
        let source = common::workspace().join(format!("examples/{example}.ignite"));
        let (program, std) = common::lower_typed(
            &source,
            &common::repository_std(),
            Path::new("../ignite-io"),
        );
        assert!(
            !program.cargo_toml.contains("process"),
            "{example}: {}",
            program.cargo_toml
        );
        assert!(
            !std.cargo_toml.contains("process"),
            "{example}: {}",
            std.cargo_toml
        );
    }
}

/// Adversary pass 2. The entry `::ignite_io::Launcher` of the generator's
/// list of process items is held by no other test: with it removed every
/// test of this crate passes. A program can hold a launcher that it only
/// passes on or keeps, naming no other process item; `ignite_io::Launcher`
/// exists only under the feature, so without it the generated crate does
/// not compile.
#[test]
fn a_program_that_names_only_a_launcher_turns_on_the_substrate_feature() {
    let process = "ignite-io = { path = \"../ignite-io\", features = [\"process\"] }";
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("streams-launcher-feature");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("keeper.ignite");
    std::fs::write(
        &file,
        "type Keeper {\n  tools: Launcher\n  name: Text\n}\n\n\
         fn Keeper.label(self: Keeper) -> Text = self.name\n",
    )
    .unwrap();
    let (program, _) =
        common::lower_typed(&file, &common::repository_std(), Path::new("../ignite-io"));
    assert!(
        program.lib_rs.contains("::ignite_io::Launcher"),
        "{}",
        program.lib_rs
    );
    for other in ["ChildProcess", "ChildExit", "Invocation", "Pipe"] {
        assert!(
            !program.lib_rs.contains(&format!("::ignite_io::{other}")),
            "{other}: {}",
            program.lib_rs
        );
    }
    assert!(
        program.cargo_toml.contains(process),
        "a program that names only `::ignite_io::Launcher` gets no feature `process`: {}",
        program.cargo_toml
    );
}
