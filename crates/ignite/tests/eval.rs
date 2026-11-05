//! Acceptance 8 of slice 11 (`docs/slice.md` section 15.3): every fn of
//! `fixtures/eval_equivalence.ignite` gives the same debug text in the
//! evaluator, in a Rust-target build and in an LLVM-target build. The
//! fixtures of E115, E117 and E118 are also evaluated directly, for the
//! error values behind the diagnostics (`plan::fixtures_match_expected`
//! covers the rendered text).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use ignite::Target;
use ignite_eval::{EvalError, Evaluator, Fuel, Limit, Value};
use ignite_syntax::ast::{Decl, Expr, Program};
use ignite_types::QualName;

use crate::common;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// The fns of the fixture without parameters, in declaration order: the
/// ones the printer's `main` shows.
fn printed_fns(program: &Program) -> Vec<String> {
    program
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Fn(f) if f.params.is_empty() => Some(f.qualified_name()),
            _ => None,
        })
        .collect()
}

/// The debug text of every printed fn, by the evaluator.
fn evaluated() -> Vec<String> {
    let loaded =
        ignite::load(&fixture("eval_equivalence.ignite"), None).unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    let module = loaded.types().module.clone();
    printed_fns(loaded.program())
        .iter()
        .map(|name| {
            let value = evaluator
                .call(&QualName::new(module.clone(), name.clone()), Vec::new())
                .unwrap_or_else(|e| panic!("{name}: {e:?}"));
            value.debug_text()
        })
        .collect()
}

/// The fixture built for `target`, once per test binary.
fn eqprint(target: Target) -> &'static Path {
    static BUILT: [OnceLock<PathBuf>; 2] = [OnceLock::new(), OnceLock::new()];
    let cell = match target {
        Target::Rust => &BUILT[0],
        Target::Llvm => &BUILT[1],
    };
    cell.get_or_init(|| {
        let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
        let options = match target {
            Target::Rust => ignite::BuildOptions {
                target: Target::Rust,
                out: Some(tmp.join("eval-equivalence")),
                ..Default::default()
            },
            Target::Llvm => ignite::BuildOptions {
                target: Target::Llvm,
                out: Some(tmp.join("eval-equivalence-llvm")),
                runtime: Some(common::runtime_archive()),
                ..Default::default()
            },
        };
        let output = ignite::build(&fixture("eval_equivalence.ignite"), &options)
            .unwrap_or_else(|e| panic!("{e}"));
        output
            .executables
            .iter()
            .find(|path| path.file_stem().and_then(|s| s.to_str()) == Some("eqprint"))
            .unwrap_or_else(|| panic!("no eqprint in {:?}", output.executables))
            .clone()
    })
}

/// The lines the compiled fixture prints on `target`.
fn printed(target: Target) -> Vec<String> {
    let output = Command::new(eqprint(target))
        .output()
        .unwrap_or_else(|e| panic!("run eqprint: {e}"));
    assert!(
        output.status.success(),
        "{target:?}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("debug text is UTF-8");
    stdout.lines().map(str::to_string).collect()
}

fn assert_agrees(target: Target) {
    let loaded =
        ignite::load(&fixture("eval_equivalence.ignite"), None).unwrap_or_else(|e| panic!("{e}"));
    let names = printed_fns(loaded.program());
    let ours = evaluated();
    let theirs = printed(target);
    assert_eq!(
        theirs.len(),
        names.len(),
        "{target:?} printed {} lines",
        theirs.len()
    );
    for ((name, ours), theirs) in names.iter().zip(&ours).zip(&theirs) {
        assert_eq!(ours, theirs, "{name} on {target:?}");
    }
}

#[test]
fn evaluator_agrees_with_the_rust_target() {
    assert_agrees(Target::Rust);
}

#[test]
fn evaluator_agrees_with_the_llvm_target() {
    assert_agrees(Target::Llvm);
}

#[test]
fn fixture_covers_every_family() {
    let loaded =
        ignite::load(&fixture("eval_equivalence.ignite"), None).unwrap_or_else(|e| panic!("{e}"));
    let names = printed_fns(loaded.program());
    assert!(names.len() >= 40, "{} fns", names.len());
    for family in [
        "int_",
        "byte",
        "text_",
        "list_",
        "map_",
        "option_",
        "result_",
        "quantity_",
        "record_",
        "enum_",
        "closure_",
        "interpolation",
        "debug_",
        "named_args",
        "match_",
        "rest_",
        "call_",
        "move_",
    ] {
        assert!(
            names.iter().any(|name| name.starts_with(family)),
            "no fn of family {family}"
        );
    }
}

/// The one property expression of a fixture's single-member system.
fn property<'a>(loaded: &'a ignite::Loaded, field: &str) -> &'a Expr {
    let system = &loaded.types().systems()[0];
    let decl = system.decl(loaded.program()).expect("system declaration");
    system.members[0]
        .property(decl, field)
        .unwrap_or_else(|| panic!("no property {field}"))
}

fn call(
    evaluator: &mut Evaluator<'_>,
    module: &str,
    name: &str,
    args: Vec<Value>,
) -> Result<Value, EvalError> {
    evaluator.call(&QualName::new(module, name), args)
}

#[test]
fn e115_limits() {
    let loaded = ignite::load(&fixture("e115_fuel_exhausted.ignite"), None)
        .unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let steps = property(&loaded, "steps");

    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    assert_eq!(
        evaluator.evaluate(loaded.types(), steps),
        Err(EvalError::Limit {
            limit: Limit::Fuel(1_000_000),
            running: "fixture_e115.forever".to_string(),
        })
    );

    let mut evaluator = Evaluator::new(&modules, Fuel::with_units(100));
    assert_eq!(
        evaluator.evaluate(loaded.types(), steps),
        Err(EvalError::Limit {
            limit: Limit::Fuel(100),
            running: "fixture_e115.forever".to_string(),
        })
    );

    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    assert_eq!(
        call(&mut evaluator, "fixture_e115", "deep", vec![Value::Int(0)]),
        Err(EvalError::Limit {
            limit: Limit::Depth(Evaluator::MAX_DEPTH),
            running: "fixture_e115.deep".to_string(),
        })
    );
    assert_eq!(
        call(&mut evaluator, "fixture_e115", "grow", vec![Value::Int(1)]),
        Err(EvalError::Limit {
            limit: Limit::Nodes(1_000_000),
            running: "fixture_e115.grow".to_string(),
        })
    );
}

/// Sharing a model's stack must preserve independent budgets for every
/// property, including the properties after a failed evaluation.
#[test]
fn model_properties_keep_fresh_fuel_budgets() {
    let loaded =
        ignite::load(&fixture("eval_scoped_fuel.ignite"), None).unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let mut reference = Evaluator::new(&modules, Fuel::default());
    assert_eq!(
        reference.evaluate(loaded.types(), property(&loaded, "first")),
        Ok(Value::Int(20))
    );
    let units = reference.units_used();
    assert!(units > 1);
    let mut exact = Evaluator::new(&modules, Fuel::with_units(units));
    for _ in 0..2 {
        ignite_eval::Model::check(&mut exact, loaded.types()).unwrap();
    }
    let mut short = Evaluator::new(&modules, Fuel::with_units(units - 1));
    let errors = ignite_eval::Model::check(&mut short, loaded.types()).unwrap_err();
    assert_eq!(errors.len(), 2, "the shorter third property still succeeds");
    assert!(errors.iter().all(|error| error.code == "E115"));
    assert_eq!(
        ignite_eval::Model::check(&mut short, loaded.types()),
        Err(errors)
    );
}

fn failed(result: Result<Value, EvalError>) -> String {
    match result {
        Err(EvalError::Failed { report }) => report,
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn e117_failures_carry_the_target_report() {
    let loaded = ignite::load(&fixture("e117_evaluation_failed.ignite"), None)
        .unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    let file = fixture("e117_evaluation_failed.ignite")
        .display()
        .to_string();

    let report = failed(evaluator.evaluate(loaded.types(), property(&loaded, "steps")));
    assert_eq!(report, format!("/ failed at {file}:9:35: 1 / 0"));

    let report = failed(call(&mut evaluator, "fixture_e117", "overflow", Vec::new()));
    assert_eq!(
        report,
        format!("+ failed at {file}:13:24: 9223372036854775807 + 1")
    );

    let report = failed(call(&mut evaluator, "fixture_e117", "checked", Vec::new()));
    assert_eq!(
        report,
        format!("assert failed at {file}:16:3: 1 + 1 == 3\n  message: math\n  left: 2\n  right: 3")
    );

    let list = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
    let report = failed(call(&mut evaluator, "fixture_e117", "third", vec![list]));
    assert_eq!(
        report,
        format!("index failed at {file}:20:34: index 3 out of range for length 3")
    );

    let value = call(
        &mut evaluator,
        "fixture_e117",
        "parsed",
        vec![Value::Text("x".to_string())],
    )
    .unwrap();
    assert_eq!(
        value,
        Value::err(Value::Text("not a number: x".to_string()))
    );
    let value = call(
        &mut evaluator,
        "fixture_e117",
        "parsed",
        vec![Value::Text("12".to_string())],
    )
    .unwrap();
    assert_eq!(value, Value::ok(Value::Int(12)));
}

#[test]
fn e118_closures_are_unencodable() {
    let loaded = ignite::load(&fixture("e118_unencodable_value.ignite"), None)
        .unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    assert_eq!(
        evaluator.evaluate(loaded.types(), property(&loaded, "scale")),
        Err(EvalError::Unencodable {
            what: "a closure".to_string()
        })
    );
    assert_eq!(
        call(
            &mut evaluator,
            "fixture_e118",
            "double",
            vec![Value::Int(21)]
        ),
        Ok(Value::Int(42))
    );
}

/// Invariant 9 (`docs/slice.md` section 15.3): every set property of the
/// design file evaluates within the default fuel; the most expensive one is
/// printed (`--nocapture`) for the `AGENTS.md` row.
#[test]
fn design_file_properties_evaluate_within_default_fuel() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/terminal/terminal.design.ignite");
    let loaded = ignite::load(&file, None).unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    let mut most = (0, String::new());
    let mut count = 0;
    for system in loaded.types().systems() {
        let decl = system.decl(loaded.program()).expect("system declaration");
        for member in &system.members {
            for (field, source) in &member.properties {
                if !matches!(source, ignite_types::PropertySource::Set(_)) {
                    continue;
                }
                let expr = member.property(decl, field).expect("set property");
                let value = evaluator.evaluate(loaded.types(), expr);
                assert!(
                    value.is_ok(),
                    "{}.{}.{field}: {value:?}",
                    system.name,
                    member.name
                );
                count += 1;
                if evaluator.units_used() > most.0 {
                    most = (
                        evaluator.units_used(),
                        format!("{}.{}.{field}", system.name, member.name),
                    );
                }
            }
        }
    }
    assert!(count > 0);
    println!(
        "most expensive property: {} ({} units of {} properties)",
        most.1, most.0, count
    );
    assert!(most.0 < Fuel::DEFAULT_LIMIT / 10, "{most:?}");
}
