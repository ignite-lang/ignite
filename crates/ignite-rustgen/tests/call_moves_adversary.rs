//! Adversary (story `std-mysql-client`, unit `call-moves`): the reporter's
//! form 9, `r = set_in(r, k, read_in(r, "b")?)`, in a loop over the
//! record's own rows (`for row in r.rows`). The loop's body assigns `r`,
//! so both targets iterate a copy of the list (`r.rows.clone()` here,
//! `docs/llvm.md` section 4 on LLVM) and nothing reads `r.rows` in place
//! while the body runs; still `Walk::pinned` keeps every move of `r` in the
//! body from happening, so each update copies the whole record
//! (`set_in(r.clone(), ..)`), as before the unit. Over the parameter
//! `rows` of the same loop (`updated_over_rows`) the record moves.

use crate::common;

const PROGRAM: &str = "enum Val {
  Nil
  Num(Int)
  Txt(Text)
}

type Ctx {
  inputs: List<(Text, Val)>
  rows: List<Text>
}

fn set_in(r: Ctx, name: Text, v: Val) -> Ctx {
  var o = r
  o.inputs.push((name, v))
  o
}

fn read_in(r: Ctx, name: Text) -> Result<Val, Text> {
  for e in r.inputs {
    if e.0 == name {
      return Ok(e.1)
    }
  }
  Err(\"no input {name}\")
}

fn updated_over_own_rows(r0: Ctx) -> Result<Ctx, Text> {
  var r = r0
  for row in r.rows {
    r = set_in(r, row, read_in(r, \"b\")?)
  }
  Ok(r)
}

fn updated_over_rows(r0: Ctx, rows: List<Text>) -> Result<Ctx, Text> {
  var r = r0
  for row in rows {
    r = set_in(r, row, read_in(r, \"b\")?)
  }
  Ok(r)
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

#[test]
fn adversary_an_update_in_a_loop_over_the_records_own_rows_moves_the_record() {
    let lib = common::generate_source(PROGRAM, &common::repository_std())
        .unwrap_or_else(|d| panic!("{d:?}"))
        .lib_rs;
    let over_rows = section(&lib, "pub fn updated_over_rows(");
    assert!(
        !over_rows.contains("r.clone()"),
        "updated_over_rows copies r: {over_rows}"
    );
    let own = section(&lib, "pub fn updated_over_own_rows(");
    assert!(
        own.contains("r.rows.clone()"),
        "updated_over_own_rows does not iterate a copy of the rows: {own}"
    );
    assert!(
        !own.contains("set_in(r.clone()"),
        "updated_over_own_rows copies r for every update: {own}"
    );
}
