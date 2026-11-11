//! Adversary (story eval-substrate-model, pass 1): programs on which the
//! evaluator's model of the substrate (`docs/slice.md` 33.5) and both
//! targets must agree, or the evaluator must refuse the test with
//! `evaluation failed: ...`. A wrong outcome is worse than a refusal: the
//! differential runner compares every unit the evaluator runs with Rust.

use std::path::{Path, PathBuf};

use ignite_eval::{EvalError, Evaluator, Fuel};

/// The fuel of the differential runner's evaluator units.
const FUEL: u64 = 10 * Fuel::DEFAULT_LIMIT;

/// `source` written as `<name>.ignite` in the scratch directory, loaded and
/// checked.
fn program(name: &str, source: &str) -> ignite::Loaded {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("eval_model_adversary");
    std::fs::create_dir_all(&dir).unwrap();
    let path: PathBuf = dir.join(format!("{name}.ignite"));
    let own = dir.join(format!("{name}.{:?}.tmp", std::thread::current().id()));
    std::fs::write(&own, source).unwrap();
    std::fs::rename(&own, &path).unwrap();
    let loaded = ignite::load(&path, None).unwrap_or_else(|e| panic!("{e}"));
    loaded.check().unwrap_or_else(|e| panic!("{e}"));
    loaded
}

fn run(loaded: &ignite::Loaded, test: &str) -> Result<(), EvalError> {
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::with_units(FUEL));
    evaluator.run_test(&loaded.types().module, test)
}

/// What both targets do, or a refusal: anything else is a difference.
fn agrees_or_refuses(outcome: &Result<(), EvalError>) -> bool {
    match outcome {
        Ok(()) => true,
        Err(EvalError::Failed { report }) => report.starts_with("evaluation failed: "),
        Err(_) => true,
    }
}

const ORDER: &str = r#"package model.adversary_order

import std.net.{Pipe, PipeStream}
import std.io.IoError

type IoResult<T> = Result<T, IoError>

fn pick(log: Store<Int>, s: Store<Int>) -> Store<Int> requires [log.write] {
  log.put(log.next_id(), 1)
  s
}

fn arg(log: Store<Int>) -> Int requires [log.write] {
  log.put(log.next_id(), 2)
  5
}

fn end_of(log: Store<Int>, s: PipeStream) -> PipeStream requires [log.write] {
  log.put(log.next_id(), 1)
  s
}

fn bytes_of(log: Store<Int>) -> Bytes requires [log.write] {
  log.put(log.next_id(), 2)
  b"x"
}

test "a store method reads its receiver before its arguments" {
  let log: Store<Int> = Store.new()
  let s: Store<Int> = Store.new()
  pick(log, s).put(arg(log), 9)
  assert log.values() == [1, 2]
}

test "a pipe method reads its receiver before its arguments" {
  let log: Store<Int> = Store.new()
  let (a, b) = Pipe.pair()
  end_of(log, a).write_all(bytes_of(log))?
  assert log.values() == [1, 2]
}

test "one store gives the receiver's index and the argument" {
  let s: Store<Int> = Store.new()
  let stores: List<Store<Int>> = [s]
  stores[s.next_id() - 1].put(s.next_id(), 7)
  assert s.values() == [7]
  assert s.get(2) == Some(7)
}
"#;

/// Both targets evaluate a builtin method's receiver before its arguments:
/// `ignite test --target rust` and `--target llvm` pass all three tests of
/// [`ORDER`]. The model's method path (`Evaluator::builtin_method` in
/// `crates/ignite-eval/src/eval.rs`, the `is_model_method` branch)
/// evaluates the arguments first, so where the receiver and the arguments
/// both act on the model the evaluator fails a test the targets pass. At
/// the base (`f58a7733`) the same tests were refused (`Store.new()` had no
/// value), so the model turns a refusal into a difference.
#[test]
fn adversary_a_model_method_reads_its_receiver_before_its_arguments() {
    let loaded = program("adversary_order", ORDER);
    let found: Vec<String> = [
        "a store method reads its receiver before its arguments",
        "a pipe method reads its receiver before its arguments",
        "one store gives the receiver's index and the argument",
    ]
    .into_iter()
    .filter_map(|test| {
        let outcome = run(&loaded, test);
        (!agrees_or_refuses(&outcome)).then(|| format!("{test}: {outcome:?}"))
    })
    .collect();
    assert!(
        found.is_empty(),
        "the targets pass these tests; the evaluator neither passes nor refuses them:\n{}",
        found.join("\n")
    );
}

const PLAIN_ORDER: &str = r#"package model.adversary_plain_order

fn note(log: Store<Int>, n: Int) -> Unit requires [log.write] {
  log.put(log.next_id(), n)
}

fn text_of(log: Store<Int>) -> Text requires [log.write] {
  note(log, 1)
  "abc"
}

fn prefix_of(log: Store<Int>) -> Text requires [log.write] {
  note(log, 2)
  "a"
}

test "a plain builtin whose receiver and argument act on a store" {
  let log: Store<Int> = Store.new()
  let t = text_of(log).starts_with(prefix_of(log))
  assert t
  assert log.values() == [1, 2]
}
"#;

/// Adversary (story eval-substrate-model, pass 2). The fix of pass 1 moved
/// the receiver first only in the model's branch of `builtin_method`; the
/// other builtins still evaluate the arguments first (the `_` arm of
/// `Evaluator::builtin_method`). Under the model an argument and a receiver
/// can both write a store, so `text_of(log).starts_with(prefix_of(log))`
/// logs `[2, 1]` in the evaluator where `ignite test --target all` passes
/// on Rust and LLVM (`[1, 2]`). At the base (`f58a7733`) the test was
/// refused (`Store.new()` had no value), so the model turns a refusal into
/// a difference, the same shape as pass 1's finding.
#[test]
fn adversary_a_plain_builtin_reads_its_receiver_before_its_arguments_under_the_model() {
    let loaded = program("adversary_plain_order", PLAIN_ORDER);
    let test = "a plain builtin whose receiver and argument act on a store";
    let outcome = run(&loaded, test);
    assert!(
        agrees_or_refuses(&outcome),
        "the targets pass this test; the evaluator neither passes nor refuses it: {outcome:?}"
    );
}
