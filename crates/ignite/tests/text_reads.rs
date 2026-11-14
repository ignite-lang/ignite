//! Story `rust-target-text-reads-borrow`: a `Text` or `Bytes` that a
//! function only reads is read in place on the Rust target
//! (`docs/slice.md` section 6, "Text and bytes read in place").
//!
//! [`PROGRAM`] is the reported program reduced to seven functions, one
//! per form of the story: `escape_into` and `all_digits` read the bytes of
//! a text (form 1, the second with a constant), `decode_cell` reads the
//! payload of a lent `Option<Bytes>` and keeps its text (form 2), `dedupe`
//! and `cells_to_row` take lists their callers are done with (forms 3 and
//! 4), `bytes_of` turns a text payload into bytes (form 5), and `slots_of`
//! builds the cells a row is decoded from. Its inline tests run on the
//! evaluator and both targets.
//!
//! [`COUNTS`] appended to it is one executable, `text-counts FORM M N`,
//! which calls the function of `FORM` `N` times over a text of `M` bytes
//! (a row of `M` cells for `row`). Its allocations at `N` = 1001 less
//! those at `N` = 1, over 1,000, are the allocations of one call, counted
//! with valgrind memcheck's `total heap usage` line as `shared_lists::`
//! counts them. The LLVM target allocates from its task's arena, which
//! `malloc` does not see: its executable prints the values the Rust one
//! prints, and its `malloc` calls per call are printed, not held.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::borrowed_reads_adversary::agree;
use crate::common::Target;
use crate::todo;

/// Calls counted: the run of `FIRST + CALLS` calls less the run of `FIRST`.
const FIRST: u64 = 1;
const CALLS: u64 = 1000;

/// The reduced program.
pub(crate) const PROGRAM: &str = r##"package text_reads

enum Val {
  Nil
  Num(Int)
  Txt(Text)
}

enum Wire {
  WNil
  WTxt(Text)
  WRaw(Bytes)
}

type Cell {
  name: Text
  raw: Option<Bytes>
  kind: Int
}

const DIGITS: Text = "0123456789"

// Form 1: the bytes of a text read in a loop.
fn escape_into(out: Bytes, s: Text) -> Bytes {
  var buffer = out
  let b = s.to_bytes()
  for c in b {
    if c == b'"' || c == b'\\' {
      buffer.push(b'\\')
    }
    buffer.push(c)
  }
  buffer
}

// Form 1 with `is_empty` and a constant.
fn all_digits(s: Text) -> Bool {
  let b = s.to_bytes()
  if b.is_empty() {
    return false
  }
  let digits = DIGITS.to_bytes()
  for c in b {
    var found = false
    for d in digits {
      if c == d {
        found = true
      }
    }
    if !found {
      return false
    }
  }
  true
}

// Form 2: the payload of a lent option, only read.
fn decode_cell(raw: Option<Bytes>, kind: Int) -> Val {
  let b = match raw {
    Some(x) => x
    None => return Nil
  }
  if kind == 1 {
    return match Int.parse(b.to_text()) {
      Some(n) => Num(n)
      None => Nil
    }
  }
  Txt(b.to_text())
}

// Form 3: a by-value list whose only caller is done with it.
fn dedupe(cols: List<(Text, Val)>) -> List<(Text, Val)> {
  var out: List<(Text, Val)> = []
  for c in cols {
    if out.is_empty() || out[out.len() - 1].0 != c.0 {
      out.push(c)
    }
  }
  out
}

// Form 4: a loop over a lent list of records that keeps a field.
fn cells_to_row(cells: List<Cell>) -> List<(Text, Val)> {
  var columns: List<(Text, Val)> = []
  for c in cells {
    columns.push((c.name, decode_cell(c.raw, c.kind)))
  }
  dedupe(columns)
}

// Form 5: a text payload turned into bytes.
fn bytes_of(w: Wire) -> Bytes = match w {
  WNil => Bytes.empty()
  WTxt(t) => t.to_bytes()
  WRaw(b) => b
}

// The cells of one row of wire values.
fn slots_of(wires: List<Wire>, names: List<Text>) -> List<Cell> {
  var cells: List<Cell> = []
  var i = 0
  while i < wires.len() {
    let raw = match wires[i] {
      WNil => None
      _ => Some(bytes_of(wires[i]))
    }
    cells.push(Cell { name = names[i], raw = raw, kind = 0 })
    i = i + 1
  }
  cells
}

fn text_of(v: Val) -> Text = match v {
  Nil => "nil"
  Num(n) => "{n}"
  Txt(t) => t
}

test "the bytes of a text are escaped" {
  let out = escape_into(b"<", "a\"b\\c")
  assert out.to_text() == "<a\\\"b\\\\c"
  assert escape_into(Bytes.empty(), "").is_empty()
}

test "digits are recognised" {
  assert all_digits("0123")
  assert !all_digits("01a3")
  assert !all_digits("")
}

test "a cell is decoded" {
  assert text_of(decode_cell(Some(b"42"), 1)) == "42"
  assert text_of(decode_cell(Some(b"x4"), 1)) == "nil"
  assert text_of(decode_cell(Some(b"abc"), 0)) == "abc"
  assert text_of(decode_cell(None, 0)) == "nil"
}

test "a row keeps one column per name" {
  let cells = slots_of([WTxt("a"), WRaw(b"1"), WNil, WTxt("d")], ["k", "k", "m", "n"])
  let row = cells_to_row(cells)
  assert row.len() == 3
  assert row[0].0 == "k"
  assert text_of(row[0].1) == "a"
  assert text_of(row[1].1) == "nil"
  assert text_of(row[2].1) == "d"
}

test "the bytes of a wire value" {
  assert bytes_of(WTxt("ab")) == b"ab"
  assert bytes_of(WRaw(b"c")) == b"c"
  assert bytes_of(WNil).is_empty()
}
"##;

/// The counting executable appended to [`PROGRAM`].
const COUNTS: &str = r#"
type Counts {
  command_line: CommandLine
  executable: Text
}

fn number(t: Text) -> Int = match Int.parse(t) {
  Some(n) => n
  None => 0
}

fn repeated(m: Int, unit: Text) -> Text {
  var out = ""
  var i = 0
  while i < m {
    out = out + unit
    i = i + 1
  }
  out
}

fn size(v: Val) -> Int = match v {
  Nil => 0
  Num(n) => n
  Txt(t) => t.len()
}

fn measure(form: Text, m: Int, n: Int) -> Int {
  let s = repeated(m, "a")
  let d = repeated(m, "7")
  let raw = Some(s.to_bytes())
  let w = WTxt(s)
  var wires: List<Wire> = []
  var names: List<Text> = []
  var k = 0
  while k < m {
    wires.push(WTxt("v{k}"))
    names.push("c{k}")
    k = k + 1
  }
  var buf = Bytes.with_capacity(2 * m + 8)
  var total = 0
  var i = 0
  if form == "escape" {
    while i < n {
      buf.clear()
      buf = escape_into(buf, s)
      total = total + buf.len()
      i = i + 1
    }
  } else if form == "digits" {
    while i < n {
      if all_digits(d) {
        total = total + 1
      }
      i = i + 1
    }
  } else if form == "decode" {
    while i < n {
      total = total + size(decode_cell(raw, 0))
      i = i + 1
    }
  } else if form == "bytes" {
    while i < n {
      total = total + bytes_of(w).len()
      i = i + 1
    }
  } else if form == "row" {
    while i < n {
      let cells = slots_of(wires, names)
      let row = cells_to_row(cells)
      total = total + row.len()
      i = i + 1
    }
  }
  total
}

fn Counts.main(self: Counts) -> Result<Unit, IoError> {
  let arguments = self.command_line.arguments()
  let m = number(arguments[1])
  let n = number(arguments[2])
  log("{arguments[0]} {m}: {measure(arguments[0], m, n)}")
  return Ok(Unit)
}

system TextCounts {
  resource command_line: CommandLine
  component counts: Counts { command_line = command_line, executable = "text-counts" }
}
"#;

/// The counting executable, built for `target`.
fn counting(target: Target) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("text-reads-source-{target:?}"));
    std::fs::create_dir_all(&dir).expect("the source directory");
    let file = dir.join("text_reads.ignite");
    std::fs::write(&file, format!("{PROGRAM}{COUNTS}")).expect("write the source");
    todo::build(
        &file,
        &format!("text-reads-{target:?}"),
        target,
        "text-counts",
    )
}

/// The allocations of `binary form m n` run to its end under memcheck
/// (its `total heap usage` line), and its log line without the count.
fn heap(binary: &Path, form: &str, m: u64, n: u64) -> (u64, String) {
    let log = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "text-reads-{}-{form}-{m}-{n}-{}.log",
        binary
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|p| p.to_str())
            .unwrap_or("run"),
        std::process::id()
    ));
    let output = Command::new("valgrind")
        .arg("--tool=memcheck")
        .arg(format!("--log-file={}", log.display()))
        .arg(binary)
        .args([form, &m.to_string(), &n.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("run valgrind");
    let line = String::from_utf8_lossy(&output.stderr).trim().to_string();
    assert!(
        output.status.success(),
        "{form} {m} {n}: {:?}: {line}",
        output.status
    );
    let text = std::fs::read_to_string(&log).expect("the valgrind log");
    let _ = std::fs::remove_file(&log);
    let count = text
        .lines()
        .find_map(|line| line.split_once("total heap usage: ").map(|(_, rest)| rest))
        .and_then(|usage| usage.split_whitespace().next())
        .and_then(|count| count.replace(',', "").parse().ok())
        .unwrap_or_else(|| panic!("no heap usage in the valgrind log:\n{text}"));
    (count, line)
}

/// Allocations of one call of `form` over size `m` after the first call,
/// and the log line of the longer run.
fn per_call(binary: &Path, form: &str, m: u64) -> (f64, String) {
    let (first, _) = heap(binary, form, m, FIRST);
    let (all, line) = heap(binary, form, m, FIRST + CALLS);
    (all.saturating_sub(first) as f64 / CALLS as f64, line)
}

/// The most allocations one call of each form may make on the Rust target
/// at `M` = 10 and at `M` = 200 (for `row`, per call at `M` = 10 and per
/// element more at `M` = 200), and the count before the story.
///
/// - `escape`, `digits`: form 1, the bytes of a text only read: 0 (1 and
///   2 before: the copy of the text and, in `all_digits`, of the constant).
/// - `decode`: form 2, a payload of a lent option read and turned into
///   text: 1, the text (2 before: the payload copied first).
/// - `bytes`: form 5, a text payload of a lent enum returned as bytes: 1,
///   the copy the caller's value needs (as before).
/// - `row`: forms 1, 2 and 5 along `slots_of` and `cells_to_row`, per
///   element: the name copied by `slots_of`, by `cells_to_row` and by
///   `dedupe`, the value by `bytes_of`, as a text by `decode_cell` and
///   by `dedupe`: 6 (7 before: the payload copied in `decode_cell`).
const FORMS: [(&str, f64, f64); 5] = [
    ("escape", 0.0, 1.0),
    ("digits", 0.0, 2.0),
    ("decode", 1.0, 2.0),
    ("bytes", 1.0, 1.0),
    ("row", 6.0, 7.0),
];

/// Acceptance bullet 1: the allocations per call of the reduced
/// program's functions at two sizes on the Rust target, held at
/// [`FORMS`]; the LLVM target's values agree and its `malloc` calls per
/// call are printed.
#[test]
fn a_text_or_bytes_only_read_is_not_copied_on_the_rust_target() {
    let status = Command::new("valgrind")
        .args(["--tool=memcheck", "--log-file=/dev/null", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("valgrind is needed to count allocations and did not run: {e}"));
    assert!(status.success(), "valgrind did not run `true`: {status:?}");
    let rust = counting(Target::Rust);
    let llvm = counting(Target::Llvm);
    let mut failures = Vec::new();
    for (form, most, before) in FORMS {
        let (small, small_line) = per_call(&rust, form, 10);
        let (large, large_line) = per_call(&rust, form, 200);
        let (llvm_small, llvm_small_line) = per_call(&llvm, form, 10);
        let (llvm_large, llvm_large_line) = per_call(&llvm, form, 200);
        println!(
            "{form}: Rust {small:.3} allocations per call at M = 10, {large:.3} at M = 200; \
             LLVM malloc {llvm_small:.3} and {llvm_large:.3}"
        );
        let held = if form == "row" {
            let per_element = (large - small) / 190.0;
            println!("{form}: Rust {per_element:.3} allocations per element");
            [per_element]
        } else {
            [small.max(large)]
        };
        if held[0] > most + 0.2 {
            failures.push(format!(
                "{form}: {small:.3} allocations per call at M = 10, {large:.3} at M = 200, \
                 at most {most} expected ({before} before the story)"
            ));
        }
        for (llvm_line, line) in [(llvm_small_line, small_line), (llvm_large_line, large_line)] {
            if llvm_line != line {
                failures.push(format!("{form}: LLVM printed {llvm_line:?}, Rust {line:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The rows of `std.protocol.mysql`: `cells FORM M N` reads one row of
/// `M` text cells of 8 bytes `N` times with `read_text_row` (`text`) or
/// `read_binary_row` (`binary`).
const CELLS: &str = r#"package mysql_cells

import std.protocol.mysql.{Column, MysqlValue, read_binary_row, read_text_row}

type Cells {
  command_line: CommandLine
  executable: Text
}

fn number(t: Text) -> Int = match Int.parse(t) {
  Some(n) => n
  None => 0
}

fn columns(m: Int) -> List<Column> {
  var out: List<Column> = []
  var i = 0
  while i < m {
    out.push(Column { schema = "s", table = "t", name = "c{i}", charset = 45, length = 255, kind = 253, flags = 0, decimals = 0 })
    i = i + 1
  }
  out
}

fn fields(start: Bytes, m: Int) -> Bytes {
  var out = start
  var i = 0
  while i < m {
    out.push(8)
    out.append("cell{1000 + i}".to_bytes())
    i = i + 1
  }
  out
}

fn size(row: List<Option<MysqlValue>>) -> Int {
  var n = 0
  for cell in row {
    n = n + match cell {
      Some(Text(t)) => t.len()
      _ => -1000
    }
  }
  n
}

fn read(form: Text, m: Int, n: Int) -> Int {
  var bitmap = Bytes.of([0])
  var k = 0
  while k < (m + 9) / 8 {
    bitmap.push(0)
    k = k + 1
  }
  let text = fields(b"", m)
  let binary = fields(bitmap, m)
  let cs = columns(m)
  var total = 0
  var i = 0
  while i < n {
    let row = if form == "text" { read_text_row(cs, text) } else { read_binary_row(cs, binary) }
    total = total + match row {
      Ok(values) => size(values)
      Err(_) => -1
    }
    i = i + 1
  }
  total
}

fn Cells.main(self: Cells) -> Result<Unit, IoError> {
  let arguments = self.command_line.arguments()
  let m = number(arguments[1])
  log("{arguments[0]} {m}: {read(arguments[0], m, number(arguments[2]))}")
  return Ok(Unit)
}

system MysqlCells {
  resource command_line: CommandLine
  component cells: Cells { command_line = command_line, executable = "cells" }
}
"#;

/// Acceptance bullet 2: a text cell of `std.protocol.mysql`'s rows, text
/// and binary protocol, allocates once (the slice its row reader takes
/// out, whose buffer becomes the text) on the Rust target, one less than
/// before the story (the slice and its copy as text). Per cell: the
/// allocations of a row of 200 cells less those of a row of 10, over 190.
/// The LLVM target prints the same values.
#[test]
fn a_text_cell_of_a_mysql_row_takes_the_buffer_of_its_bytes() {
    let status = Command::new("valgrind")
        .args(["--tool=memcheck", "--log-file=/dev/null", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("valgrind is needed to count allocations and did not run: {e}"));
    assert!(status.success(), "valgrind did not run `true`: {status:?}");
    let build = |target: Target| {
        let dir =
            Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("mysql-cells-source-{target:?}"));
        std::fs::create_dir_all(&dir).expect("the source directory");
        let file = dir.join("mysql_cells.ignite");
        std::fs::write(&file, CELLS).expect("write the source");
        todo::build(&file, &format!("mysql-cells-{target:?}"), target, "cells")
    };
    let rust = build(Target::Rust);
    let llvm = build(Target::Llvm);
    let mut failures = Vec::new();
    for form in ["text", "binary"] {
        let (small, small_line) = per_call(&rust, form, 10);
        let (large, large_line) = per_call(&rust, form, 200);
        let per_cell = (large - small) / 190.0;
        println!(
            "{form}: Rust {small:.3} allocations per row of 10 cells, {large:.3} of 200, \
             {per_cell:.3} per cell"
        );
        if per_cell > 1.1 {
            failures.push(format!(
                "{form}: {per_cell:.3} allocations per text cell, at most 1 expected \
                 (2 before the story)"
            ));
        }
        for (m, line) in [(10, small_line), (200, large_line)] {
            let (llvm_count, llvm_line) = per_call(&llvm, form, m);
            println!("{form}: LLVM malloc {llvm_count:.3} per row of {m} cells");
            if llvm_line != line {
                failures.push(format!(
                    "{form} {m}: LLVM printed {llvm_line:?}, Rust {line:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Acceptance bullet 3: the reduced program's own tests pass on the
/// evaluator and both targets.
#[test]
fn the_reduced_program_gives_the_evaluator_values_on_both_targets() {
    agree("text_reads", PROGRAM, 5);
}

/// Acceptance bullet 4 and the guards of the story: the bytes of a text
/// and a payload read in place keep the value they had when they were
/// bound while the text or the option is changed, moved into a callee
/// that changes it, given to `spawn`, captured by a closure or changed by
/// a loop, and a buffer moved into a text is one no later read sees.
/// Each test is one guard; the evaluator is the reference and both
/// targets must compile the program and give its values.
const GUARDS: &str = r#"package text_read_guards

fn grown(t: Text) -> Text {
  var u = t
  u = u + "zz"
  u
}

fn count(b: Bytes) -> Int {
  var n = 0
  for c in b {
    n = n + c.to_int()
  }
  n
}

fn sent(t: Text, done: Sender<Int>) -> Result<Unit, IoError> {
  var u = t
  u = u + "!"
  done.send(u.len())
  Ok(())
}

fn first_of(raw: Option<Bytes>) -> Int {
  let b = match raw {
    Some(x) => x
    None => return -1
  }
  if b.is_empty() { 0 } else { b[0].to_int() }
}

fn text_after(b: Bytes) -> Text {
  let t = b.to_text()
  t
}

fn tuple_taken(n: Int) -> Int {
  let t = "abcd"
  match (t, n) {
    (a, 1) => {
      var x = t.to_bytes()
      x.push(33)
      a.len() * 10 + x.len()
    }
    _ => 0
  }
}

test "a text changed while its bytes are read" {
  var s = "abc"
  let b = s.to_bytes()
  s = s + "x"
  assert b.len() == 3
  assert s.len() == 4
}

test "a text moved into a callee that changes it while its bytes are read" {
  let s = "abc"
  let b = s.to_bytes()
  let g = grown(s)
  assert b.len() + count(b) == 3 + 294
  assert g == "abczz"
}

test "a text given to spawn while its bytes are read" {
  let s = "abcd"
  let b = s.to_bytes()
  let (done, results) = Channel.bounded(1)
  spawn sent(s, done)
  assert results.receive() == Some(5)
  assert b.len() == 4
}

test "a text captured by a closure while its bytes are read" {
  var s = "ab"
  let b = s.to_bytes()
  let f = |n: Int| s.len() + n
  s = s + "cde"
  assert f(1) == 3
  assert b.len() == 2
  assert count(b) == 195
}

test "a text changed by a loop while its bytes are read" {
  var s = "a"
  let b = s.to_bytes()
  var i = 0
  var seen = 0
  while i < 3 {
    seen = seen + b.len()
    s = s + "b"
    i = i + 1
  }
  for c in b {
    seen = seen + c.to_int()
  }
  assert seen == 3 + 97
  assert s == "abbb"
}

test "an option changed while its payload is read" {
  var raw = Some(b"xy")
  let b = match raw {
    Some(x) => x
    None => b""
  }
  raw = None
  assert b.len() == 2
  assert raw == None
  assert first_of(Some(b"A")) == 65
  assert first_of(Some(b"")) == 0
  assert first_of(None) == -1
}

test "bytes turned into text at their last use and before it" {
  var b = b"ab"
  let t = b.to_text()
  b.push(99)
  assert t == "ab"
  assert b.len() == 3
  assert text_after(b"cd") == "cd"
  assert text_after(Bytes.of([255, 97])).len() == 4
}

test "a text whose buffer is taken while its bytes are read" {
  let s = "abc"
  let b = s.to_bytes()
  let u = s.to_bytes()
  assert b.len() * 10 + u.len() == 33
  assert tuple_taken(1) == 45
  assert tuple_taken(2) == 0
}

test "bytes given to text at their last use in a loop" {
  var out: List<Text> = []
  var i = 0
  while i < 3 {
    let b = "x{i}".to_bytes()
    out.push(b.to_text())
    i = i + 1
  }
  assert out == ["x0", "x1", "x2"]
}
"#;

#[test]
fn a_text_or_payload_read_in_place_keeps_the_evaluator_value_on_both_targets() {
    agree("text_read_guards", GUARDS, 9);
}
