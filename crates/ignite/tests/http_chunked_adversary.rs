//! Adversary, story `http-chunked-adversary-findings`, pass 2. The inline
//! test "adversary: the body limit in one-byte chunks is decoded in under
//! 8 s" guards the move of the chunk decoder's input buffer on LLVM only: on
//! the Rust target the per-chunk copy it is meant to catch costs about 2.4 s,
//! under its bound. This test reads the generated code of both targets
//! instead: no decoder fn of `std.protocol.http` copies a `Bytes` parameter,
//! and `read_chunk_data` takes `data` by value. `IGNITE_ADVERSARY_STD` points
//! it at another standard library (a mutated copy).

use std::path::PathBuf;
use std::process::Command;

const DECODERS: [&str; 5] = [
    "read_chunk_data",
    "read_chunked",
    "read_line",
    "read_body",
    "next_request",
];

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn standard_library() -> PathBuf {
    std::env::var_os("IGNITE_ADVERSARY_STD")
        .map(PathBuf::from)
        .unwrap_or_else(|| repository().join("std"))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The text of the generated fn `name` from its signature to the next fn.
fn rust_fn<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source
        .find(&format!("pub fn {name}<"))
        .unwrap_or_else(|| panic!("no generated Rust fn {name}"));
    let rest = &source[start..];
    let end = rest[1..].find("pub fn ").map_or(rest.len(), |at| at + 1);
    &rest[..end]
}

#[test]
fn the_chunk_decoder_copies_no_bytes_parameter_on_either_target() {
    let std_dir = standard_library();
    let file = repository().join("examples/todo/todo.ignite");

    let emit = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("emit")
        .arg("--std")
        .arg(&std_dir)
        .arg(&file)
        .output()
        .expect("run ignite emit");
    assert_eq!(emit.status.code(), Some(0), "{}", text(&emit.stderr));
    let rust = text(&emit.stdout);
    let chunk_data = rust_fn(&rust, "read_chunk_data");
    let signature = chunk_data.lines().next().unwrap();
    assert!(
        signature.contains("data: Vec<u8>"),
        "read_chunk_data borrows its input buffer: {signature}"
    );
    for name in DECODERS {
        let body = rust_fn(&rust, name);
        for parameter in ["data", "buffer", "body"] {
            assert!(
                !body.contains(&format!("{parameter}.clone()")),
                "generated Rust {name} copies `{parameter}`"
            );
        }
    }
    let chunked = rust_fn(&rust, "read_chunked");
    assert!(
        chunked.contains("read_chunk_data::<T>(stream, limits, data,"),
        "read_chunked does not pass its buffer by value to read_chunk_data"
    );

    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("http-chunked-adversary-llvm");
    let _ = std::fs::remove_dir_all(&out);
    let build = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(["build", "--target", "llvm"])
        .arg("--std")
        .arg(&std_dir)
        .arg(&file)
        .arg("--out")
        .arg(&out)
        .arg("--runtime")
        .arg(crate::common::runtime_archive())
        .output()
        .expect("run ignite build");
    assert_eq!(build.status.code(), Some(0), "{}", text(&build.stderr));
    let ir = std::fs::read_to_string(out.join("llvm/main_todod.ll")).expect("main_todod.ll");
    let mut function = String::new();
    let mut copies = vec![];
    for line in ir.lines() {
        if line.starts_with("define ") {
            function = line.to_string();
        }
        let decoder = DECODERS
            .iter()
            .any(|name| function.contains(&format!("std.protocol.http.{name}<")));
        if decoder
            && line.contains("call void @\"copy.List<Byte>\"(ptr %t")
            && line.contains(", ptr %p")
        {
            copies.push(format!(
                "{}\n  {}",
                &function[..function.len().min(90)],
                line.trim()
            ));
        }
    }
    assert!(
        copies.is_empty(),
        "LLVM decoder fns copy a Bytes parameter:\n{}",
        copies.join("\n")
    );
}
