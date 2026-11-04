//! Adversarial cases for the last-use moves analysis (`docs/slice.md`
//! 20.3): shapes the unit's own tests in `moves.rs` do not cover, each
//! asserting the contract's rule directly.

use crate::common;

use ignite_check::{Modules, check_module};
use ignite_syntax::SourceFile;
use ignite_syntax::ast::{Decl, Expr, Program};
use ignite_types::{Callee, QualName, TypedModules, TypedProgram, type_module};

struct Typed {
    source: SourceFile,
    program: Box<Program>,
    typed: TypedProgram,
}

fn typed(text: &str) -> Typed {
    let source = SourceFile::new("moves_adversary.ignite", text.to_string());
    let program = Box::new(
        ignite_syntax::parse(&source)
            .unwrap_or_else(|d| panic!("parse failed:\n{}", source.render_all(&d))),
    );
    let checked = check_module(&program, &Modules::new())
        .unwrap_or_else(|d| panic!("check failed:\n{}", source.render_all(&d)));
    let typed = type_module(&program, &checked, &TypedModules::new())
        .unwrap_or_else(|d| panic!("type failed:\n{}", source.render_all(&d)));
    Typed {
        source,
        program,
        typed,
    }
}

impl Typed {
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
        let mut inside: Vec<&Expr> = common::expressions(&self.program)
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

    fn consumed(&self, name: &str, index: usize) -> bool {
        self.typed
            .consumed(&Callee::Fn(QualName::new("main", name)), index)
    }
}

const PRELUDE: &str = r#"type State {
  items: List<Int>
  count: Int
}

fn fresh() -> State = State { items = [1], count = 0 }

fn wipe(x: State) -> Int {
  x.items.clear()
  0
}

fn drain(l: List<Int>) -> Int {
  l.clear()
  0
}

fn consume(x: State) -> Int {
  x.items.push(1)
  x.count
}

fn look(s: State, n: Int) -> Int = s.items.len() + n
"#;

/// 20.3.2: "A borrowed parameter would see the moved value change". An
/// enclosing call that has already taken a borrowed operand rooted at `s`
/// is still running when a nested argument moves `s` (or `s.items`) into a
/// callee that changes it in place: on LLVM (20.3.5) the borrow is the
/// place's pointer and sees the cleared list; on Rust (20.3.4)
/// `look(&s, wipe(s))` is E0505. The evaluator gives `look` the old value
/// (1 item), so no argument here may move.
#[test]
fn an_enclosing_call_that_borrows_the_place_keeps_the_copy() {
    let t = typed(&format!(
        "{PRELUDE}\n{}",
        r#"
fn enclosing_argument() -> Int {
  let s = fresh()
  look(s, wipe(s))
}

fn enclosing_field() -> Int {
  let s = fresh()
  look(s, drain(s.items))
}

fn enclosing_ufcs_receiver() -> Int {
  let s = fresh()
  s.look(wipe(s))
}

fn enclosing_builtin_receiver() -> Int {
  let s = fresh()
  s.items.at(wipe(s))
}
"#
    ));
    assert!(!t.consumed("look", 0), "look borrows its State");
    assert!(t.consumed("wipe", 0), "wipe changes its State in place");
    let mut wrong = Vec::new();
    for name in [
        "enclosing_argument",
        "enclosing_field",
        "enclosing_ufcs_receiver",
        "enclosing_builtin_receiver",
    ] {
        let marked = t.marked_in(name);
        if !marked.is_empty() {
            wrong.push(format!("{name}: {marked:?}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "moved while an enclosing call borrows the place:\n{}",
        wrong.join("\n")
    );
}

/// The module's own reading of 20.3.3 (moves.rs, module doc): "a fn whose
/// every path loops forever moves nothing, so its parameters are not
/// consumed". `spin` never exits and its parameter is never dead at the
/// call (the next iteration reads it), so no move happens anywhere; it
/// was borrowed before, and consuming it makes a caller copy (20.3.3: "no
/// call makes more copies than before").
#[test]
fn a_body_that_never_exits_consumes_nothing() {
    let t = typed(&format!(
        "{PRELUDE}\n{}",
        r#"
fn spin(p: State) {
  loop {
    let n = consume(p)
  }
}
"#
    ));
    assert_eq!(t.marked_in("spin"), Vec::<String>::new());
    assert!(!t.consumed("spin", 0), "spin moves `p` on no path");
}

/// 20.3.3: "The set is the least fixpoint over each module's fns". A chain
/// declared caller first, `f0(x) = f1(x)`, ..., `f69` changing `x` in
/// place: every link passes its parameter at its last use to a consumed
/// parameter, so every link consumes it.
#[test]
fn consumed_reaches_the_fixpoint_on_a_long_chain() {
    let links = 70;
    let mut source = String::from(PRELUDE);
    for i in 0..links - 1 {
        source.push_str(&format!("\nfn f{i}(x: State) -> Int = f{}(x)\n", i + 1));
    }
    source.push_str(&format!(
        "\nfn f{}(x: State) -> Int {{\n  x.items.push(1)\n  x.count\n}}\n",
        links - 1
    ));
    let t = typed(&source);
    let missing: Vec<String> = (0..links)
        .filter(|i| !t.consumed(&format!("f{i}"), 0))
        .map(|i| format!("f{i}"))
        .collect();
    assert!(missing.is_empty(), "not consumed: {missing:?}");
}

/// 20.3.3: the in-place half of `consumed` "is the union of what
/// `region::mutates` (LLVM) and `mutates_receiver` with `mutated_names`
/// (Rust) count today ... `consumed` must be a superset of both". LLVM's
/// `region::modified` roots a mutating receiver through `?`, `.at(i)` and a
/// block's result (`region::root` in `crates/ignite-llvm/src/region.rs`),
/// so these parameters are owned on LLVM today.
#[test]
fn in_place_changes_the_llvm_backend_counts_consume() {
    let t = typed(
        r#"
fn through_at(p: List<List<Int>>) -> Int {
  p.at(0).push(1)
  p.len()
}

fn through_try(o: Result<List<Int>, Text>) -> Result<Int, Text> {
  o?.push(1)
  Ok(0)
}
"#,
    );
    assert!(t.consumed("through_at", 0), "p.at(0).push(1)");
    assert!(t.consumed("through_try", 0), "o?.push(1)");
}
