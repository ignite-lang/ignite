//! The evaluator's model of the substrate under a test (`docs/slice.md`
//! 33.5): stores, capabilities, pipes, tasks, channels and a virtual clock,
//! in memory and deterministic, active only in
//! [`ignite_eval::Evaluator::run_test`]. Every program here is written to
//! the test's scratch directory, outside the differential corpus: some pin
//! what only the model decides (the order of tasks, a deadlock), which a
//! target leaves to its scheduler.

use std::path::{Path, PathBuf};

use ignite_eval::{EvalError, Evaluator, Fuel, Limit};
use ignite_types::QualName;

/// The fuel of the differential runner's evaluator units.
const FUEL: u64 = 10 * Fuel::DEFAULT_LIMIT;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// `source` written as `<name>.ignite` in the scratch directory, loaded
/// and checked. Tests running at once write the same text; each writes a
/// file of its own and renames it into place.
fn program(name: &str, source: &str) -> ignite::Loaded {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("eval_model");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.ignite"));
    let own = dir.join(format!("{name}.{:?}.tmp", std::thread::current().id()));
    std::fs::write(&own, source).unwrap();
    std::fs::rename(&own, &path).unwrap();
    load(&path)
}

fn load(path: &Path) -> ignite::Loaded {
    let loaded = ignite::load(path, None).unwrap_or_else(|e| panic!("{e}"));
    loaded.check().unwrap_or_else(|e| panic!("{e}"));
    loaded
}

/// The outcome of the inline test `test` of `loaded` in the evaluator,
/// under `fuel`.
fn run_with(loaded: &ignite::Loaded, test: &str, fuel: Fuel) -> Result<(), EvalError> {
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, fuel);
    evaluator.run_test(&loaded.types().module, test)
}

fn run(loaded: &ignite::Loaded, test: &str) -> Result<(), EvalError> {
    run_with(loaded, test, Fuel::with_units(FUEL))
}

/// The report of a failed outcome.
fn report(outcome: Result<(), EvalError>) -> String {
    match outcome {
        Err(EvalError::Failed { report }) => report,
        other => panic!("expected a failure, found {other:?}"),
    }
}

/// The field of a `module(..)` value is the fn that fills it.
#[test]
fn a_module_field_is_the_fn_that_fills_it() {
    let loaded = load(&fixture("siblings/demo.ignite"));
    assert_eq!(run(&loaded, "module greeting"), Ok(()));
}

const STORES: &str = r#"package model.stores

test "a store gives ids from 1 and its values in id order" {
  let store: Store<Text> = Store.new()
  let a = store.next_id()
  let b = store.next_id()
  assert a == 1
  assert b == 2
  store.put(b, "two")
  store.put(a, "one")
  assert store.values() == ["one", "two"]
  assert store.get(a) == Some("one")
  assert store.get(7) == None
  assert store.update(a, |t| t + "!") == Some("one!")
  assert store.update(9, |t| t + "?") == None
  assert store.get(9) == None
  assert store.upsert(9, nine) == "nine"
  assert store.upsert(9, nine) == "nine."
  assert store.remove(b) == Some("two")
  assert store.get(b) == None
  assert store.values() == ["one!", "nine."]
}

fn nine(old: Option<Text>) -> Text = match old {
  Some(t) => t + "."
  None => "nine"
}

test "a copy of a store shares it" {
  let store: Store<Int> = Store.new()
  let other = store
  other.put(1, 5)
  assert store.get(1) == Some(5)
}

test fn fill(store: Store<List<Int>>, count: Int) -> Unit requires [store.write] {
  var i = 0
  while i < count {
    store.put(i, [i, i, i, i, i, i, i, i, i, i])
    i = i + 1
  }
}

test "values held by a store count against the node limit" {
  let store: Store<List<Int>> = Store.new()
  fill(store, 1000)
}

test fn make() -> Store<Int> = Store.new()
"#;

/// `Store<T>` under a test: ids from 1, values in id order, `update` and
/// `upsert` as on the targets, and a copy is a share of the same store.
#[test]
fn a_store_holds_values_by_id() {
    let loaded = program("stores", STORES);
    assert_eq!(
        run(
            &loaded,
            "a store gives ids from 1 and its values in id order"
        ),
        Ok(())
    );
    assert_eq!(run(&loaded, "a copy of a store shares it"), Ok(()));
}

/// The values a store holds count against the node limit (15.3, O14).
#[test]
fn stored_values_count_against_the_node_limit() {
    let loaded = program("stores", STORES);
    let test = "values held by a store count against the node limit";
    assert_eq!(run(&loaded, test), Ok(()));
    let fuel = Fuel {
        units: FUEL,
        nodes: 5_000,
    };
    match run_with(&loaded, test, fuel) {
        Err(EvalError::Limit {
            limit: Limit::Nodes(5_000),
            ..
        }) => {}
        other => panic!("expected the node limit, found {other:?}"),
    }
}

/// Outside a test the model is not there: a property or a fn evaluated at
/// compile time that reaches `Store.new()` has no value (`ignite check`,
/// `ignite plan`).
#[test]
fn outside_a_test_a_store_has_no_value() {
    let loaded = program("stores", STORES);
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    let module = loaded.types().module.clone();
    let outcome = evaluator.call(&QualName::new(module, "make"), Vec::new());
    assert_eq!(
        outcome,
        Err(EvalError::Failed {
            report: "evaluation failed: builtin `StoreNew` has no value at compile time"
                .to_string()
        })
    );
}

const CAPABILITIES: &str = r#"package model.capabilities

test "a test's capabilities give what they were made of" {
  let setting: Setting = Setting.of("--port", "8080")
  assert setting.value() == "8080"
  let command_line: CommandLine = CommandLine.of(["add", "x", "--all"])
  assert command_line.arguments() == ["add", "x", "--all"]
  let environment = Environment.of(["A=1", "B=2=3", "A=4", "NOEQ", "=5", "E="])
  assert environment.get("A") == Some("4")
  assert environment.get("B") == Some("2=3")
  assert environment.get("E") == Some("")
  assert environment.get("NOEQ") == None
  assert environment.get("") == None
}
"#;

/// `Setting.of`, `CommandLine.of` and `Environment.of` under a test: the
/// capability gives what it was made of, the last entry of a variable
/// winning.
#[test]
fn a_test_capability_gives_what_it_was_made_of() {
    let loaded = program("capabilities", CAPABILITIES);
    assert_eq!(
        run(&loaded, "a test's capabilities give what they were made of"),
        Ok(())
    );
}
