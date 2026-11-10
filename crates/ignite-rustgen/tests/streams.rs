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

/// TLS client sessions (`docs/slice.md`, section "TLS client sessions"):
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
