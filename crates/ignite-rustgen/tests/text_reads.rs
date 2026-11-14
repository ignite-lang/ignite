//! Story `rust-target-text-reads-borrow` (`docs/slice.md` section 6, "Text
//! and bytes read in place"): the emitted text of the forms that read a
//! `Text` or `Bytes` where it is, and of the guards that keep the copy.
//! The allocations and the values are counted end to end by
//! `text_reads::` in `crates/ignite`.

use crate::common;

const PROGRAM: &str = "const DIGITS: Text = \"0123456789\"

fn loops(s: Text) -> Int {
  let b = s.to_bytes()
  var n = 0
  for c in b {
    n = n + c.to_int()
  }
  n
}

fn digits(s: Text) -> Bool {
  let d = DIGITS.to_bytes()
  if d.is_empty() {
    return false
  }
  for c in d {
    if c.to_int() == s.len() {
      return true
    }
  }
  false
}

fn payload(raw: Option<Bytes>) -> Text {
  let b = match raw {
    Some(x) => x
    None => return \"\"
  }
  b.to_text()
}

fn payload_kept(raw: Option<Bytes>) -> Text {
  let b = match raw {
    Some(x) => x
    None => b\"\"
  }
  b.to_text()
}

fn summed(b: Bytes) -> Int {
  var n = 0
  for c in b {
    n = n + c.to_int()
  }
  n
}

fn taken(n: Int) -> Text {
  let b = \"x{n}\".to_bytes()
  let t = b.to_text()
  t + \"!\"
}

fn kept(n: Int) -> Int {
  let b = \"x{n}\".to_bytes()
  let t = b.to_text()
  t.len() + b.len()
}

fn text_taken(n: Int) -> Int {
  let s = \"y{n}\"
  var b = s.to_bytes()
  b.push(1)
  b.len()
}

fn changed(n: Int) -> Int {
  var s = \"z{n}\"
  let b = s.to_bytes()
  s = s + \"x\"
  b.len() + s.len()
}

fn taken_while_read(n: Int) -> Int {
  let s = \"w{n}\"
  let b = s.to_bytes()
  var u = s.to_bytes()
  u.push(2)
  b.len() + u.len()
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

/// Form 1: the bytes of a text, or of a constant, that a `for` and
/// `is_empty` only read are lent; a `Bytes` parameter a `for` reads is a
/// lent slice.
#[test]
fn the_bytes_of_a_text_read_by_a_for_are_lent() {
    let lib = lib();
    let text = section(&lib, "pub fn loops(");
    assert!(text.contains("let b: &[u8] = s.as_bytes();"), "{text}");
    let text = section(&lib, "pub fn digits(");
    assert!(
        text.contains("let d: &[u8] = DIGITS().as_bytes();"),
        "{text}"
    );
    let text = section(&lib, "pub fn summed(");
    assert!(text.contains("pub fn summed(b: &[u8])"), "{text}");
}

/// Form 2: the payload of a lent option whose other arm returns is lent;
/// one whose other arm is a value is copied, as before.
#[test]
fn the_payload_of_a_lent_option_only_read_is_lent() {
    let lib = lib();
    let text = section(&lib, "pub fn payload(");
    assert!(text.contains("let b: &[u8] = match raw {"), "{text}");
    assert!(text.contains("Some(x) => x,"), "{text}");
    assert!(!text.contains("clone()"), "{text}");
    let text = section(&lib, "pub fn payload_kept(");
    assert!(text.contains("Some(x) => x.clone(),"), "{text}");
}

/// Bullet 2: `to_text` and `to_bytes` at the last use of a variable the fn
/// owns take its buffer; one read after keeps the copy, and so does a
/// text whose bytes are still read, or one changed while they are.
#[test]
fn a_buffer_read_for_the_last_time_is_taken() {
    let lib = lib();
    let text = section(&lib, "pub fn taken(");
    assert!(
        text.contains("String::from_utf8(b).unwrap_or_else("),
        "{text}"
    );
    let text = section(&lib, "pub fn kept(");
    assert!(text.contains("String::from_utf8_lossy(&b)"), "{text}");
    let text = section(&lib, "pub fn text_taken(");
    assert!(text.contains("s.into_bytes()"), "{text}");
    let text = section(&lib, "pub fn changed(");
    assert!(text.contains("s.clone().into_bytes()"), "{text}");
    let text = section(&lib, "pub fn taken_while_read(");
    assert!(
        text.contains("let b = s.clone().into_bytes();") && text.contains("s.into_bytes()"),
        "{text}"
    );
}
