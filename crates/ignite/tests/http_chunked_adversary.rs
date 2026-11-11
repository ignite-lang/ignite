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

/// Adversary pass 1 on unit counts (wave 11): story
/// `ignored-tests-and-long-files-back` moves 34 helpers of
/// `std.protocol.http` into `std.protocol.http.syntax` and promises "every
/// path an importer names is unchanged". Imports are not re-exported, so a
/// program that names a moved helper through `std.protocol.http` stops
/// checking. Every in-repo program that imports `std.protocol.http` checks.
#[test]
fn every_program_importing_std_protocol_http_still_checks() {
    fn walk(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "ignite") {
                out.push(path);
            }
        }
    }
    let root = repository();
    let mut files = Vec::new();
    walk(&root.join("crates/ignite/tests/fixtures"), &mut files);
    walk(&root.join("examples"), &mut files);
    files.sort();
    let importers: Vec<PathBuf> = files
        .into_iter()
        .filter(|f| {
            std::fs::read_to_string(f)
                .unwrap()
                .lines()
                .any(|l| l.trim_start().starts_with("import std.protocol.http."))
        })
        .collect();
    assert!(importers.len() >= 13, "{importers:?}");
    let failed: Vec<String> = importers
        .iter()
        .filter_map(|f| {
            ignite::check_with_std(f, Some(&standard_library()))
                .err()
                .map(|e| format!("{}: {e}", f.display()))
        })
        .collect();
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// Every name `std.protocol.http` exported at the tag `v0.15.0`, generated
/// once from `git show v0.15.0:std/protocol/http.ignite`: its records,
/// aliases, enums, fns, constants and its protocol, test-only items and
/// associated fns (`Response.json`) left out. A program could import each
/// of them by that path then, so each stays importable by it.
const V0_15_0_HTTP_NAMES: [&str; 91] = [
    "IoResult",
    "Request",
    "Response",
    "Content",
    "HttpApp",
    "no_piece",
    "HttpLimits",
    "READ_SIZE",
    "MAXIMUM_LENGTH_DIGITS",
    "Http",
    "HttpServer",
    "limit_reasons",
    "reason",
    "status_response",
    "method_not_allowed",
    "has_line_break",
    "is_list_field",
    "is_auth_field",
    "is_date_field",
    "list_elements",
    "has_empty_element",
    "has_bws",
    "is_digits",
    "is_imf_fixdate",
    "is_allowed_field",
    "checked",
    "Framing",
    "render_head",
    "render",
    "render_message",
    "inline_length",
    "render_framed",
    "inline_framing",
    "is_connect_success",
    "has_no_content",
    "streamed_framing",
    "HEX_DIGITS",
    "hex",
    "chunk",
    "send_streamed",
    "header",
    "segments",
    "lists_connection_option",
    "has_connection_option",
    "response_closes",
    "keep_alive",
    "is_token_byte",
    "is_token",
    "is_token_in",
    "is_ows",
    "trim_ows",
    "trimmed_in",
    "lowered_in",
    "is_field_value",
    "is_field_value_in",
    "is_target",
    "is_target_in",
    "absolute_form",
    "scheme_length",
    "starts_lowered",
    "is_reg_name_byte",
    "is_host",
    "resolve_host",
    "target_too_long",
    "parse_head",
    "RequestLine",
    "request_line",
    "target_parts",
    "origin_parts",
    "head_authority",
    "HeadRead",
    "read_head",
    "read_body",
    "Next",
    "rejected",
    "next_request",
    "hex_digit_value",
    "ChunkSize",
    "chunk_size",
    "LineRead",
    "read_line",
    "ChunkData",
    "read_chunk_data",
    "Chunked",
    "read_chunked",
    "transfer_codings",
    "coding_status",
    "parse_length",
    "reject",
    "asked",
    "session",
];

/// Unit counts, fix round 1 (wave 11): a program importing every name of
/// [`V0_15_0_HTTP_NAMES`] by explicit name from `std.protocol.http` checks.
#[test]
fn every_name_std_protocol_http_exported_at_v0_15_0_is_still_importable() {
    let text = format!(
        "package tests.http_v0_15_0_names\n\nimport std.protocol.http.{{{}}}\n\nfn main() -> Int = 0\n",
        V0_15_0_HTTP_NAMES.join(", ")
    );
    let file = repository().join("crates/ignite/tests/fixtures/http_v0_15_0_names.ignite");
    if let Err(e) = ignite::check_text(&file, text, Some(&standard_library())) {
        panic!("a name of std.protocol.http at v0.15.0 no longer imports:\n{e}");
    }
}
