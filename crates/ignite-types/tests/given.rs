//! The by-value parameters every caller gives up (`docs/slice.md` section
//! 6, "By-value parameters given by every caller"): `type_program` adds
//! them to `TypedProgram::consumed` for the program's own module, and
//! `type_module`, which types every module of `std` and every sibling
//! module, does not. Each case below is one of the callers the rule must
//! refuse, beside one it gives.

use crate::common;

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_syntax::ast::{Decl, Program};
use ignite_types::{Callee, QualName, TypedModules, TypedProgram, type_module, type_program};

struct Typed {
    source: SourceFile,
    program: Box<Program>,
    typed: TypedProgram,
}

/// Parse, check and type `text` as module `main` without std, as the
/// program's own module (`root`) or as any other module.
fn typed(text: &str, root: bool) -> Typed {
    let source = SourceFile::new("given.ignite", text.to_string());
    let program = Box::new(
        ignite_syntax::parse(&source)
            .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d))),
    );
    let checked = check_module(&program, &Modules::new())
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let modules = TypedModules::new();
    let typed = if root {
        type_program(&program, &checked, &modules)
    } else {
        type_module(&program, &checked, &modules)
    }
    .unwrap_or_else(|d| panic!("type failed:\n{}", source.render_all(&d)));
    Typed {
        source,
        program,
        typed,
    }
}

impl Typed {
    fn consumed(&self, name: &str, index: usize) -> bool {
        self.typed
            .consumed(&Callee::Fn(QualName::new("main", name)), index)
    }

    /// The text of every place of fn `name` that moves, in source order.
    fn marked_in(&self, name: &str) -> Vec<String> {
        let span = self
            .program
            .decls
            .iter()
            .find_map(|d| match d {
                Decl::Fn(f) if f.qualified_name() == name => Some(f.span),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no fn `{name}`"));
        let mut inside: Vec<_> = common::expressions(&self.program)
            .into_iter()
            .filter(|e| e.span.start >= span.start && e.span.end <= span.end)
            .filter(|e| self.typed.moves(e))
            .collect();
        inside.sort_by_key(|e| e.span.start);
        inside
            .iter()
            .map(|e| self.source.text[e.span.start..e.span.end].to_string())
            .collect()
    }
}

/// A fn that keeps the elements of its list parameter: an owner of it.
const KEEP: &str = r#"fn keep(l: List<Text>) -> List<Text> {
  var out: List<Text> = []
  for x in l {
    out.push(x)
  }
  out
}
"#;

fn with_keep(callers: &str, root: bool) -> Typed {
    typed(&format!("{KEEP}\n{callers}"), root)
}

/// The rule's own case: every caller gives the list up. The program's
/// module moves into `keep`; the same text typed as a module of `std` or
/// a sibling module keeps it lent, its callers being in other modules.
#[test]
fn only_the_program_module_moves_into_a_parameter_every_caller_gives_up() {
    let callers = r#"fn caller() -> Int {
  let l = ["a"]
  keep(l).len() + keep(["b"]).len()
}
"#;
    let program = with_keep(callers, true);
    assert!(program.consumed("keep", 0));
    assert_eq!(program.marked_in("caller"), ["l"]);
    let module = with_keep(callers, false);
    assert!(!module.consumed("keep", 0));
    assert!(module.marked_in("caller").is_empty());
}

/// A fn used as a value has callers the analysis does not see: passed to
/// another fn, bound, or kept in a record, it keeps its lent parameter
/// although every call of it gives its argument up.
#[test]
fn a_fn_used_as_a_value_keeps_its_parameter_lent() {
    for value in [
        "fn apply(f: fn(List<Text>) -> List<Text>, l: List<Text>) -> Int = f(l).len()\n\
         fn other() -> Int = apply(keep, [\"c\"])",
        "fn other() -> Int {\n  let f = keep\n  f([\"c\"]).len()\n}",
        "type Holder {\n  f: fn(List<Text>) -> List<Text>\n}\n\
         fn other() -> Holder = Holder { f = keep }",
    ] {
        let t = with_keep(
            &format!("fn caller() -> Int = keep([\"b\"]).len()\n\n{value}\n"),
            true,
        );
        assert!(!t.consumed("keep", 0), "consumed with:\n{value}");
    }
}

/// The same binding passed to two parameters is a last use of neither:
/// each argument is read by the other, so both parameters stay lent and
/// neither argument moves.
#[test]
fn the_same_binding_passed_twice_keeps_both_parameters_lent() {
    let t = typed(
        r#"fn pair(a: List<Text>, b: List<Text>) -> List<Text> {
  var out: List<Text> = []
  for x in a {
    out.push(x)
  }
  for y in b {
    out.push(y)
  }
  out
}

fn caller() -> Int {
  let l = ["a"]
  pair(l, l).len()
}
"#,
        true,
    );
    assert!(!t.consumed("pair", 0));
    assert!(!t.consumed("pair", 1));
    assert!(t.marked_in("caller").is_empty());
}

/// Recursion: the least fixpoint. A fn that passes its own parameter on
/// to itself, and two fns that pass one to each other, keep it lent; a
/// recursive fn that gives itself a value built for the call is moved
/// into.
#[test]
fn a_recursive_fn_that_passes_its_parameter_on_keeps_it_lent() {
    let t = typed(
        r#"fn walk(l: List<Text>, n: Int) -> List<Text> {
  if n == 0 {
    var out: List<Text> = []
    for x in l {
      out.push(x)
    }
    return out
  }
  walk(l, n - 1)
}

fn ping(l: List<Text>, n: Int) -> List<Text> {
  if n == 0 {
    var out: List<Text> = []
    for x in l {
      out.push(x)
    }
    return out
  }
  pong(l, n - 1)
}

fn pong(l: List<Text>, n: Int) -> List<Text> {
  if n == 0 {
    var out: List<Text> = []
    for x in l {
      out.push(x)
    }
    return out
  }
  ping(l, n - 1)
}

fn fresh(l: List<Text>, n: Int) -> List<Text> {
  if n == 0 {
    var out: List<Text> = []
    for x in l {
      out.push(x)
    }
    return out
  }
  fresh(["z"], n - 1)
}

fn caller() -> Int = walk(["a"], 2).len() + ping(["b"], 3).len() + fresh(["c"], 1).len()
"#,
        true,
    );
    assert!(!t.consumed("walk", 0));
    assert!(!t.consumed("ping", 0));
    assert!(!t.consumed("pong", 0));
    assert!(t.consumed("fresh", 0));
}

/// A call in a loop: an argument bound before the loop is read again by
/// the next round, also where only the last round calls; one bound in the
/// round is a last use.
#[test]
fn a_call_in_a_loop_gives_only_what_the_round_built() {
    for (caller, given) in [
        (
            "fn caller(n: Int) -> Int {\n  let l = [\"a\"]\n  var total = 0\n  var i = 0\n  \
             while i < n {\n    total = total + keep(l).len()\n    i = i + 1\n  }\n  total\n}",
            false,
        ),
        (
            "fn caller() -> Int {\n  let l = [\"a\"]\n  var r: List<Text> = []\n  var i = 0\n  \
             while i < 3 {\n    if i == 2 {\n      r = keep(l)\n    }\n    i = i + 1\n  }\n  \
             r.len()\n}",
            false,
        ),
        (
            "fn caller(n: Int) -> Int {\n  var total = 0\n  var i = 0\n  while i < n {\n    \
             let m = [\"b\"]\n    total = total + keep(m).len()\n    i = i + 1\n  }\n  total\n}",
            true,
        ),
    ] {
        let t = with_keep(&format!("{caller}\n"), true);
        assert_eq!(t.consumed("keep", 0), given, "{caller}");
    }
}
