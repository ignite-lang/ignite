//! The evaluator's model of the substrate under a test (`docs/slice.md`
//! NN): stores, capabilities, pipes, tasks, channels and a virtual clock,
//! in memory and deterministic, active only in
//! [`ignite_eval::Evaluator::run_test`]. Every program here is written to
//! the test's scratch directory, outside the differential corpus: some pin
//! what only the model decides (the order of tasks, a deadlock), which a
//! target leaves to its scheduler.

use std::path::{Path, PathBuf};

use ignite_eval::{EvalError, Evaluator, Fuel, Limit};

/// The fuel of the differential runner's evaluator units.
const FUEL: u64 = 10 * Fuel::DEFAULT_LIMIT;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// `source` written as `<name>.ignite` in the scratch directory, loaded
/// and checked.
fn program(name: &str, source: &str) -> ignite::Loaded {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("eval_model");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.ignite"));
    std::fs::write(&path, source).unwrap();
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
