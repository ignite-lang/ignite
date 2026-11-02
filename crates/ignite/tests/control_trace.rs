//! The reducers of `std/control.ignite` across the evaluator and both
//! targets (`docs/slice.md` 16.0 item 10, extending 15.0 item 8): every fn
//! of `fixtures/control_trace.ignite` without parameters gives the same
//! debug text in the evaluator, in a Rust-target build and in an
//! LLVM-target build; since slice 13 that includes the transition reducer
//! of 17.4 (decision D1). The A7 measurement of 16.10 steps ten thousand
//! emissions into a thirty-day rolling window in the evaluator under the
//! default fuel per step and prints its wall time.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Instant;

use ignite::Target;
use ignite_eval::{Evaluator, Fuel, Value};
use ignite_syntax::ast::{Decl, Program};
use ignite_types::QualName;

use crate::common;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/control_trace.ignite")
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
fn evaluated(loaded: &ignite::Loaded) -> Vec<String> {
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    let module = loaded.types().module.clone();
    printed_fns(loaded.program())
        .iter()
        .map(|name| {
            evaluator
                .call(&QualName::new(module.clone(), name.clone()), Vec::new())
                .unwrap_or_else(|e| panic!("{name}: {e:?}"))
                .debug_text()
        })
        .collect()
}

/// The fixture built for `target`, once per test binary.
fn traceprint(target: Target) -> &'static Path {
    static BUILT: [OnceLock<PathBuf>; 2] = [OnceLock::new(), OnceLock::new()];
    let cell = match target {
        Target::Rust => &BUILT[0],
        Target::Llvm => &BUILT[1],
    };
    cell.get_or_init(|| {
        let tmp = Path::new(env!("CARGO_TARGET_TMPDIR"));
        let options = match target {
            Target::Rust => ignite::BuildOptions {
                out: Some(tmp.join("control-trace")),
                ..Default::default()
            },
            Target::Llvm => ignite::BuildOptions {
                target: Target::Llvm,
                out: Some(tmp.join("control-trace-llvm")),
                runtime: Some(common::runtime_archive()),
                ..Default::default()
            },
        };
        let output = ignite::build(&fixture(), &options).unwrap_or_else(|e| panic!("{e}"));
        output
            .executables
            .iter()
            .find(|path| path.file_stem().and_then(|s| s.to_str()) == Some("traceprint"))
            .unwrap_or_else(|| panic!("no traceprint in {:?}", output.executables))
            .clone()
    })
}

/// The lines the compiled fixture prints on `target`.
fn printed(target: Target) -> Vec<String> {
    let output = Command::new(traceprint(target))
        .output()
        .unwrap_or_else(|e| panic!("run traceprint: {e}"));
    assert!(
        output.status.success(),
        "{target:?}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("debug text is UTF-8");
    stdout.lines().map(str::to_string).collect()
}

/// 16.0 item 10: the three debug texts agree, fn by fn.
#[test]
fn reducers_agree_across_the_evaluator_and_both_targets() {
    let loaded = ignite::load(&fixture(), None).unwrap_or_else(|e| panic!("{e}"));
    let names = printed_fns(loaded.program());
    assert!(names.len() >= 19, "{names:?}");
    let ours = evaluated(&loaded);
    let shown = |name: &str| {
        let index = names
            .iter()
            .position(|n| n == name)
            .unwrap_or_else(|| panic!("{name} not in {names:?}"));
        ours[index].clone()
    };
    // 17.4, D1: the transition stops first, switches, starts providers
    // first, cuts the rollout member over and commits at convergence.
    assert_eq!(
        shown("transition_effects"),
        r#"[TransitionEffect.StopMember("T.probe"), TransitionEffect.StopMember("T.old"), TransitionEffect.SwitchRevision, TransitionEffect.StartMember("T.ingress"), TransitionEffect.CutOver("T.server"), TransitionEffect.StartMember("T.probe"), TransitionEffect.Commit]"#
    );
    assert_eq!(
        shown("transition_gave_up"),
        r#"[TransitionEffect.StopMember("T.probe"), TransitionEffect.StopMember("T.old"), TransitionEffect.SwitchRevision, TransitionEffect.StartMember("T.ingress"), TransitionEffect.Abandon("gave_up", "T.ingress")]"#
    );
    std::thread::scope(|scope| {
        let handles: Vec<_> = [Target::Rust, Target::Llvm]
            .into_iter()
            .map(|target| scope.spawn(move || (target, printed(target))))
            .collect();
        for handle in handles {
            let (target, theirs) = handle.join().expect("printer thread");
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
    });
}

fn record_field<'v>(record: &'v Value, name: &str) -> Option<&'v Value> {
    match record {
        Value::Record { fields, .. } => fields.iter().find(|(f, _)| f == name).map(|(_, v)| v),
        _ => None,
    }
}

/// A7 (16.10, O33): `steps` emissions at five-second slots into a
/// thirty-day rolling window, each step under the default fuel; the final
/// state holds every attempt in `buckets` buckets of `max(interval,
/// window / 10,000)`, 259.2 s here, plus the open one. The wall time is
/// printed for `AGENTS.md`.
fn a7(steps: i64, buckets: usize) {
    let loaded = ignite::load(&fixture(), None).unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    let module = loaded.types().module.clone();
    let a7_machine = QualName::new(module.clone(), "a7_machine");
    let a7_event = QualName::new(module, "a7_event");
    let step = QualName::new("std.control", "step");
    let mut machine = evaluator
        .call(&a7_machine, Vec::new())
        .unwrap_or_else(|e| panic!("a7_machine: {e:?}"));
    let started = Instant::now();
    let mut effects = 0usize;
    for i in 0..steps {
        let event = evaluator
            .call(&a7_event, vec![Value::Int(i)])
            .unwrap_or_else(|e| panic!("a7_event({i}): {e:?}"));
        let result = evaluator
            .call(&step, vec![machine, event])
            .unwrap_or_else(|e| panic!("step {i}: {e:?}"));
        machine = record_field(&result, "machine")
            .cloned()
            .unwrap_or_else(|| panic!("step {i} returned no machine"));
        if let Some(Value::List(items)) = record_field(&result, "effects") {
            effects += items.len();
        }
    }
    let elapsed = started.elapsed();
    eprintln!("A7: {steps} steps in {elapsed:?} ({effects} effects)");
    let Value::Enum {
        variant, fields, ..
    } = &machine
    else {
        panic!("{}", machine.debug_text());
    };
    assert_eq!(variant, "Evaluate");
    let state = &fields[0];
    assert_eq!(
        record_field(state, "sum_executed"),
        Some(&Value::Int(steps)),
        "{}",
        state.debug_text()
    );
    assert_eq!(
        record_field(state, "sum_good"),
        Some(&Value::Int(steps)),
        "{}",
        state.debug_text()
    );
    let Some(Value::List(closed)) = record_field(state, "buckets") else {
        panic!("{}", state.debug_text());
    };
    assert_eq!(closed.len(), buckets, "{}", state.debug_text());
    assert_eq!(effects, steps as usize, "one Publish per emission");
}

/// One tenth of A7 in the gate: 1,000 steps, about 1.2 s.
#[test]
fn a7_evaluator_steps_1000_emissions_into_a_30_day_window() {
    a7(1_000, 19);
}

/// The A7 measurement itself (12 s): `cargo test -p ignite --test main
/// control_trace::a7_evaluator_steps_10000 -- --ignored --nocapture`.
#[test]
#[ignore]
fn a7_evaluator_steps_10000_emissions_into_a_30_day_window() {
    a7(10_000, 192);
}
