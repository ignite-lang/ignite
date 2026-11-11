//! Handle ownership across patterns that bind two names (`own.rs`
//! `owned_scrutinee` and `referred`, which read the names a pattern binds
//! through `Pattern::bindings` and `Pattern::binds`). Adversary pass 1 on
//! `story:duplicated-rules-back`: reading only the first name of a pattern,
//! or no name at all, kept the whole crate suite green.

use ignite_syntax::SourceFile;

const PROGRAM: &str = "fn ends() -> (Sender<Int>, Receiver<Int>) = Channel.bounded(1)

fn kept_first() -> Receiver<Int> {
  match ends() {
    (tx, rx) => {
      tx.send(1)
      return rx
    }
  }
}

fn both_kept() -> Int {
  match ends() {
    (tx, rx) => {
      tx.send(1)
      rx.closed()
    }
  }
  return 0
}

fn shadowed() -> Receiver<Int> {
  let (tx, rx) = ends()
  tx.send(1)
  log(rx.closed().to_text())
  match ends() {
    (a, rx) => {
      a.send(2)
      return rx
    }
  }
}

fn plain() -> Int {
  let (tx, rx) = ends()
  tx.send(1)
  log(rx.closed().to_text())
  return 0
}
";

/// `program.ll` of [`PROGRAM`].
fn lower() -> String {
    let source = SourceFile::new("owned_patterns.ignite", PROGRAM.to_string());
    let program = ignite_syntax::parse(&source).unwrap_or_else(|d| panic!("{d:?}"));
    let symbols = ignite_check::check(&program).unwrap_or_else(|d| panic!("{d:?}"));
    let types = ignite_types::type_module(&program, &symbols, &Default::default())
        .unwrap_or_else(|d| panic!("{}", source.render_all(&d)));
    let input = ignite_llvm::ModuleInput {
        path: &types.module,
        program: &program,
        symbols: &symbols,
        types: &types,
    };
    let options = ignite_llvm::Options {
        sources: [(types.module.clone(), &source)].into(),
        target: Some(ignite_llvm::TargetSpec {
            triple: "x86_64-pc-linux-gnu".to_string(),
            datalayout:
                "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
                    .to_string(),
        }),
        ..Default::default()
    };
    ignite_llvm::generate_with(&[], &input, &options).program
}

/// The definition of the fn `name` of module `main` in `ir`.
fn body<'a>(ir: &'a str, name: &str) -> &'a str {
    let head = format!("@\"main.{name}\"(");
    let start = ir
        .lines()
        .position(|l| l.starts_with("define ") && l.contains(&head))
        .unwrap_or_else(|| panic!("no definition of main.{name}"));
    let offset: usize = ir.lines().take(start).map(|l| l.len() + 1).sum();
    let rest = &ir[offset..];
    let end = rest.find("\n}\n").expect("end of the definition");
    &rest[..end]
}

/// A `match` arm that lets its second binding escape (`return rx`) does
/// not keep what it binds, so the block does not own the scrutinee and
/// releases none of its handles; one whose arm keeps both names owns it.
#[test]
fn a_match_whose_second_binding_escapes_owns_no_handle() {
    let ir = lower();
    let kept_first = body(&ir, "kept_first");
    assert!(
        !kept_first.contains("ignite_rt_own_release"),
        "kept_first releases the handle it returns:\n{kept_first}"
    );
    let both_kept = body(&ir, "both_kept");
    assert!(
        both_kept.contains("ignite_rt_own_release"),
        "both_kept does not release its scrutinee:\n{both_kept}"
    );
}

/// A `match` arm that binds `rx` anew hides the outer `rx` in its body
/// (`referred`), so `return rx` there is not an escape of the outer one:
/// the block owns both outer ends, as it does without the `match`.
#[test]
fn a_match_arm_that_binds_a_name_again_hides_the_outer_binding() {
    let ir = lower();
    let owns = |name: &str| body(&ir, name).matches("ignite_rt_tcp_own").count();
    assert_eq!(owns("plain"), 2);
    assert_eq!(owns("shadowed"), 2, "{}", body(&ir, "shadowed"));
}
