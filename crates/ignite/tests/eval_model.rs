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
    load(&write(name, source))
}

/// The path `source` is written to as `<name>.ignite`.
fn write(name: &str, source: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("eval_model");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.ignite"));
    let own = dir.join(format!("{name}.{:?}.tmp", std::thread::current().id()));
    std::fs::write(&own, source).unwrap();
    std::fs::rename(&own, &path).unwrap();
    path
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

const COMPILE_TIME: &str = r#"package model.compile_time

type Holder {
  size: Int
}

fn sized() -> Int {
  let ends: (Sender<Int>, Receiver<Int>) = Channel.bounded(1)
  1
}

system S {
  component h: Holder { size = sized() }
}
"#;

/// `ignite check` and the model `ignite plan` builds evaluate a property
/// at compile time, where the model of a test is not there: a property
/// that reaches `Channel.bounded` still has no value (E117).
#[test]
fn check_and_plan_still_find_no_value_at_compile_time() {
    let path = write("compile_time", COMPILE_TIME);
    let refusal = "builtin `ChannelBounded` has no value at compile time";
    let checked = ignite::check(&path).expect_err("check evaluates the property");
    assert!(checked.to_string().contains(refusal), "{checked}");
    let loaded = ignite::load(&path, None).unwrap_or_else(|e| panic!("{e}"));
    let planned = loaded
        .model(&ignite::PlanOptions::default())
        .expect_err("plan evaluates the property");
    assert!(planned.to_string().contains(refusal), "{planned}");
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

const PIPES: &str = r#"package model.pipes

import std.net.{Pipe, PipeStream}
import std.io.IoError

type IoResult<T> = Result<T, IoError>

test "a pipe delivers a task's writes at its flush points, one batch each" {
  let (a, b) = Pipe.pair()
  a.write_all(b"he")?
  a.write_all(b"llo")?
  assert b.received_batches() == 0
  assert b.read(3)? == Data(b"hel")
  assert b.received_batches() == 1
  assert b.read(100)? == Data(b"lo")
  a.write_all(b"x")?
  Process.flush()
  a.write_all(b"y")?
  a.close()?
  assert b.received_batches() == 3
  assert b.read(10)? == Data(b"xy")
  assert b.read(10)? == Eof
  assert failed(a.write_all(b"z")) == "write to a closed stream"
}

test fn failed(result: IoResult<Unit>) -> Text = match result {
  Ok(_) => "no error"
  Err(e) => e.message
}

test fn read_failed(result: IoResult<Input>) -> Text = match result {
  Ok(_) => "no error"
  Err(e) => e.message
}

test "an idle timeout reads as the end of the stream in virtual time" {
  let (a, b) = Pipe.pair()
  a.set_idle_timeout(50)?
  assert a.read(1)? == Eof
  b.write_urgent(b"ab")?
  assert a.urgent_pending()?
  assert a.read(10)? == Data(b"a")
  assert a.read(10)? == Data(b"b")
  assert !a.urgent_pending()?
}

test "a dropped end ends its peer's stream" {
  let (a, _) = Pipe.pair()
  a.write_all(b"z")?
  Process.flush()
  assert read_failed(a.read(1)) == "pipe peer is gone"
  assert a.read(1)? == Eof
}
"#;

/// `Pipe.pair()` under a test: writes wait in the send buffer until a flush
/// point (a read, `Process.flush()`, `close`), each delivery one batch
/// (`received_batches`), an idle timeout ends a read in virtual time, the
/// urgent byte stops a read, and a dropped end ends its peer's stream.
#[test]
fn a_pipe_delivers_at_flush_points() {
    let loaded = program("pipes", PIPES);
    for test in [
        "a pipe delivers a task's writes at its flush points, one batch each",
        "an idle timeout reads as the end of the stream in virtual time",
        "a dropped end ends its peer's stream",
    ] {
        assert_eq!(run(&loaded, test), Ok(()), "{test}");
    }
}

const TASKS: &str = r#"package model.tasks

import std.net.{Pipe, PipeStream}
import std.io.IoError

type IoResult<T> = Result<T, IoError>

test fn say(out: PipeStream, text: Bytes) -> IoResult<Unit> {
  out.write_all(text)?
  Ok(())
}

test fn exactly(client: PipeStream, count: Int) -> IoResult<Bytes> {
  var out = Bytes.empty()
  while out.len() < count {
    match client.read(count - out.len())? {
      Eof => return Ok(out)
      Data(bytes) => out.append(bytes)
    }
  }
  Ok(out)
}

test fn rest(client: PipeStream) -> IoResult<Bytes> {
  var out = Bytes.empty()
  loop {
    match client.read(4096)? {
      Eof => return Ok(out)
      Data(bytes) => out.append(bytes)
    }
  }
}

test fn echo_twice(input: PipeStream, out: PipeStream) -> IoResult<Unit> {
  let first = exactly(input, 1)?
  out.write_all(first)?
  out.write_all(first)?
  Ok(())
}

test fn fails(stream: PipeStream) -> IoResult<Unit> {
  let values = [1, 2]
  assert values.len() == 3
  stream.write_all(b"never")?
  Ok(())
}

test fn spins(out: PipeStream) -> IoResult<Unit> {
  var i = 0
  while i >= 0 {
    i = i + 1
  }
  out.write_all(b"x")?
  Ok(())
}

test "tasks run in spawn order, each until it waits" {
  let (a, b) = Pipe.pair()
  let (c, d) = Pipe.pair()
  spawn echo_twice(c, a)
  spawn say(a, b"1")
  spawn say(a, b"2")
  spawn say(d, b"x")
  assert exactly(b, 4)? == b"12xx"
}

test "a task that fails ends only itself, and the ends it took go with it" {
  let (client, server) = Pipe.pair()
  spawn fails(server)
  assert rest(client)? == b""
}

test "every task waiting with no deadline is a deadlock" {
  let (a, b) = Pipe.pair()
  let (c, d) = Pipe.pair()
  spawn echo_twice(c, a)
  assert exactly(b, 1)? == b"x"
  d.close()?
}

test "a task that never waits ends the test on fuel" {
  let (a, b) = Pipe.pair()
  spawn spins(a)
  assert exactly(b, 1)? == b"x"
}
"#;

/// Tasks under a test (`docs/slice.md` 33.5): cooperative, the first task
/// in spawn order that can run holds the turn until it waits or ends, so
/// the order is the model's own and the same on every run.
#[test]
fn tasks_run_in_spawn_order_until_they_wait() {
    let loaded = program("tasks", TASKS);
    for _ in 0..3 {
        assert_eq!(run(&loaded, "tasks run in spawn order, each until it waits"), Ok(()));
    }
}

/// A failing task ends only itself (14.8); the pipe end moved into it at
/// its last use (20.3.1) is dropped with it, so its peer reads the end of
/// the stream.
#[test]
fn a_failing_task_ends_only_itself() {
    let loaded = program("tasks", TASKS);
    assert_eq!(
        run(
            &loaded,
            "a task that fails ends only itself, and the ends it took go with it"
        ),
        Ok(())
    );
}

/// Every task waits and none has a deadline: the model reports a deadlock
/// and the unit stays one the evaluator cannot run.
#[test]
fn a_deadlock_is_reported() {
    let loaded = program("tasks", TASKS);
    assert_eq!(
        report(run(&loaded, "every task waiting with no deadline is a deadlock")),
        "evaluation failed: every task of the model waits and none has a deadline (a deadlock)"
    );
}

/// A task that never waits keeps the turn and ends on the fuel the test
/// was given, which the tasks share.
#[test]
fn a_spinning_task_ends_on_fuel() {
    let loaded = program("tasks", TASKS);
    match run_with(
        &loaded,
        "a task that never waits ends the test on fuel",
        Fuel::with_units(100_000),
    ) {
        Err(EvalError::Limit {
            limit: Limit::Fuel(100_000),
            ..
        }) => {}
        other => panic!("expected the fuel limit, found {other:?}"),
    }
}
