//! Adversary of story `rust-target-text-reads-borrow` (`docs/slice.md`
//! section 6, "Text and bytes read in place"): the emitted text of the
//! places where reading in place or taking a buffer would be refused by
//! rustc or change a value, so each keeps the copy, and of one where the
//! take is legal and happens. The programs run end to end in
//! `text_reads_adversary::` of `crates/ignite`.

use crate::common;

const PROGRAM: &str = "fn take_bytes(b: Bytes) -> Int {
  var c = b
  c.push(1)
  c.len()
}

fn lent_and_text(b: Bytes, t: Text) -> Int = b.len() * 100 + t.len()

type Pair2 {
  raw: Bytes
  text: Text
}

fn pinned_for() -> Text {
  let b = b\"ab\"
  var out = \"\"
  for c in b {
    out = out + b.to_text()
  }
  out
}

fn held_lent() -> Int {
  let b = b\"ab\"
  lent_and_text(b, b.to_text())
}

fn interpolated() -> Text {
  let t = \"ab\"
  \"{t}{take_bytes(t.to_bytes())}\"
}

fn record_order() -> Int {
  let b = b\"abc\"
  let p = Pair2 { raw = b, text = b.to_text() }
  p.raw.len() + p.text.len()
}

fn closure_for(b: Bytes) -> Int {
  let g = |n: Int| {
    var s = 0
    for c in b {
      s = s + c.to_int()
    }
    s + n
  }
  g(1)
}

fn payload_stored(o: Option<Bytes>) -> Int {
  let b = match o {
    Some(x) => x
    None => return 0
  }
  var l: List<Bytes> = []
  l.push(b)
  l.len()
}

fn text_bytes_in_closure(t: Text) -> Int {
  let b = t.to_bytes()
  let g = |n: Int| b.len() + n
  g(1)
}
";

/// The generated fn that starts with `head`, up to its closing brace.
fn section<'a>(lib: &'a str, head: &str) -> &'a str {
    let start = lib
        .find(head)
        .unwrap_or_else(|| panic!("no `{head}` in:\n{lib}"));
    let rest = &lib[start..];
    &rest[..rest.find("\n}\n").expect("end of the fn")]
}

fn lib() -> String {
    common::generate_source(PROGRAM, &common::repository_std())
        .unwrap_or_else(|d| panic!("{d:?}"))
        .lib_rs
}

/// A `for` over `b` holds it for the whole loop: a `to_text` in its body
/// is no last use the buffer can be taken at (`pinned`), even though
/// nothing reads `b` after it in the source.
#[test]
fn a_buffer_a_for_iterates_is_not_taken_in_its_body() {
    let lib = lib();
    let text = section(&lib, "pub fn pinned_for(");
    assert!(text.contains("String::from_utf8_lossy(&b)"), "{text}");
    assert!(!text.contains("String::from_utf8(b)"), "{text}");
}

/// An earlier operand of the call borrows `b` (`&b` to a lent bytes
/// parameter) while the later `to_text` runs: the take would be E0505.
#[test]
fn a_buffer_an_earlier_operand_holds_is_not_taken() {
    let lib = lib();
    let text = section(&lib, "pub fn held_lent(");
    assert!(!text.contains("String::from_utf8(b)"), "{text}");
}

/// `format!` borrows `t` for its first part while the second part's
/// `to_bytes` runs: the text is copied, not taken.
#[test]
fn a_text_an_interpolation_reads_before_is_not_taken() {
    let lib = lib();
    let text = section(&lib, "pub fn interpolated(");
    assert!(text.contains("t.clone().into_bytes()"), "{text}");
}

/// A record whose earlier field copies `b` and whose later field turns it
/// into text: the later field is the last use and takes the buffer, Rust
/// evaluating the fields in the order written.
#[test]
fn the_last_field_of_a_record_takes_the_buffer() {
    let lib = lib();
    let text = section(&lib, "pub fn record_order(");
    assert!(
        text.contains("raw: b.clone(), text: String::from_utf8(b)"),
        "{text}"
    );
}

/// A `Bytes` parameter whose `for` is inside a closure is captured by a
/// stored closure (`Arc<dyn Fn + 'static>`): it stays `&Vec<u8>`, copied
/// into the closure, not a lent `&[u8]`.
#[test]
fn a_bytes_parameter_a_closure_iterates_is_not_a_lent_slice() {
    let lib = lib();
    let text = section(&lib, "pub fn closure_for(");
    assert!(text.contains("pub fn closure_for(b: &Vec<u8>)"), "{text}");
}

/// A payload that is stored (pushed into a list) after the `let` is a
/// value of its own: the binding is copied, not lent.
#[test]
fn a_stored_payload_is_not_lent() {
    let lib = lib();
    let text = section(&lib, "pub fn payload_stored(");
    assert!(!text.contains("let b: &[u8]"), "{text}");
    let text = section(&lib, "pub fn text_bytes_in_closure(");
    assert!(!text.contains("let b: &[u8]"), "{text}");
}
