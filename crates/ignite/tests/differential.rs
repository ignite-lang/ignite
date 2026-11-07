//! The differential runner (`docs/quality.md`, metrics 1.1, 1.2, 1.6, 2.2
//! and 4.3; story `quality-ratchet`): one corpus of programs run by the
//! three consumers of a typed program, the Rust target, the LLVM target and
//! the evaluator, with exit, stdout and failure text compared per unit.
//!
//! The corpus is every `.ignite` file with inline tests or an executable
//! under `crates/ignite/tests/fixtures/` (a fixture with an `.expected`
//! file is a diagnostic fixture and left out), `examples/` (design files
//! left out, as `task test-ignite` does) and `std/`, that checks. A unit is
//! one inline test (`<file>::<test name>`) or one executable
//! (`<file>::main <name>`):
//!
//! - a test runs on each target as the test runner runs it
//!   (`<test executable> --run-one <module>/<name>`, `docs/slice.md` 12.7),
//!   and in the evaluator through [`ignite_eval::Evaluator::run_test`];
//! - an executable runs on each target without arguments and with an
//!   empty stdin, unless its system declares a `TcpListener` or a
//!   `Launcher` (a server or a process starter, which needs a scripted
//!   peer; listed as not run). The evaluator runs a printer's main (an
//!   executable of a component of type `Printer`: its stdout is the debug
//!   text of every fn without parameters, one per line, in declaration
//!   order); it has no IO for any other main.
//!
//! Rust is the reference. LLVM agrees with it when the exit status, stdout
//! and stderr are equal (scratch paths normalised). The evaluator agrees
//! when it passes where Rust exits 0, or fails with the report Rust's
//! stderr carries where Rust exits 101 (`docs/slice.md` 14.8). A unit the
//! evaluator has no rule for (an IO builtin, a task, its fuel exceeded)
//! is one the evaluator cannot run, with the reason.
//!
//! [`corpus_agrees_across_rust_llvm_and_the_evaluator`] writes every
//! outcome to `target/quality/differential.json` and the metrics to
//! `target/quality/differential-metrics.json` (read by `task quality`),
//! then holds the run against `quality/baseline.json`: a difference not
//! listed under `known_differences` fails, as does a listed one that is
//! gone, and the units the evaluator cannot run must be exactly the ones
//! listed under `eval_cannot_run`; the metrics go through the ratchet of
//! [`crate::quality`]. `IGNITE_BLESS=1` writes the two lists and the
//! improved metrics into the baseline.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use ignite::Target;
use ignite_eval::{EvalError, Evaluator, Fuel, Json, Limit};
use ignite_syntax::ast::{Decl, Program};
use ignite_types::QualName;

use crate::common;
use crate::quality;

/// Seconds a unit may run on a target before it is killed.
const TIMEOUT: Duration = Duration::from_secs(20);

/// Fuel of one evaluator unit: ten times a property expression's, so a
/// test that loops longer than any compile-time evaluation still runs.
const EVAL_FUEL: u64 = 10 * Fuel::DEFAULT_LIMIT;

/// Wall-clock limits of one evaluator unit: fuel counts expressions, not
/// the copies of value semantics ([`Evaluator::set_time_limit`]), so a loop
/// that copies a growing value is quadratic within its fuel. A unit over
/// [`SLOW`] is one the evaluator cannot run (`moves_scale`'s 50,000 steps
/// took 185 s alone, 2026-11-07); the baseline lists it with that reason,
/// and only `task quality` (`IGNITE_DIFFERENTIAL_SLOW=1`) runs it again, so
/// a merge never waits for it and its verdict never depends on the gate's
/// load. Every other unit runs under [`GUARD`], far above the slowest that
/// passes (28 s alone), so that a hang ends.
const SLOW: Duration = Duration::from_secs(60);
const GUARD: Duration = Duration::from_secs(300);

/// Programs the evaluator runs at once.
const EVAL_JOBS: usize = 4;

/// Units running at once on a target.
const JOBS: usize = 8;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace")
}

/// `path` relative to the workspace, with `/` separators.
fn relative(path: &Path) -> String {
    path.strip_prefix(workspace())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Every `.ignite` file under `dir`, sorted.
fn ignite_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            ignite_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "ignite") {
            out.push(path);
        }
    }
}

/// The candidate files of the corpus, before the check: fixtures without an
/// `.expected` file, examples without design files, the standard library.
fn candidates() -> Vec<PathBuf> {
    let root = workspace();
    let mut files = Vec::new();
    ignite_files(&root.join("crates/ignite/tests/fixtures"), &mut files);
    files.retain(|file| !file.with_extension("expected").exists());
    let mut examples = Vec::new();
    ignite_files(&root.join("examples"), &mut examples);
    examples.retain(|file| !file.to_string_lossy().ends_with(".design.ignite"));
    files.extend(examples);
    ignite_files(&root.join("std"), &mut files);
    files
}

fn test_names(program: &Program) -> Vec<String> {
    program
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Test(t) => Some(t.name.name.clone()),
            _ => None,
        })
        .collect()
}

/// The executables a program declares (`executable = "<name>"` on a
/// component), with whether the component's type is `Printer`.
fn executables(text: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(at) = line.find("executable = \"") else {
            continue;
        };
        let rest = &line[at + "executable = \"".len()..];
        let Some(end) = rest.find('"') else { continue };
        let printer = line.contains(": Printer");
        out.push((rest[..end].to_string(), printer));
    }
    out
}

/// Why the executables of a program are not run, when they are not.
fn not_run(text: &str) -> Option<&'static str> {
    if text.contains(": TcpListener") {
        Some("serves on a TcpListener: needs a scripted client")
    } else if text.contains(": Launcher") {
        Some("starts processes through a Launcher: needs a scripted environment")
    } else {
        None
    }
}

/// One program of the corpus.
struct Entry {
    path: PathBuf,
    rel: String,
    module: String,
    tests: Vec<String>,
    mains: Vec<(String, bool)>,
    mains_not_run: Option<&'static str>,
    std: bool,
}

/// What a target did with a unit.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Run {
    /// The exit status, `timeout`, `signal` or `build failed`.
    status: String,
    stdout: String,
    stderr: String,
}

impl Run {
    fn json(&self) -> Json {
        Json::Object(BTreeMap::from([
            ("status".to_string(), Json::Str(self.status.clone())),
            ("stdout".to_string(), Json::Str(clip(&self.stdout))),
            ("stderr".to_string(), Json::Str(clip(&self.stderr))),
        ]))
    }
}

/// What the evaluator did with a unit.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Evaluated {
    /// Passed; a printer's stdout.
    Passed(String),
    /// Failed with this 14.8 report.
    Failed(String),
    /// Cannot run it, for this reason.
    Cannot(String),
}

fn clip(text: &str) -> String {
    const MAX: usize = 4000;
    if text.len() <= MAX {
        return text.to_string();
    }
    let mut end = MAX;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}… ({} bytes)", &text[..end], text.len())
}

/// The report a failing test or main printed on stderr: from the line
/// that holds `failed at ` (an `error: ` or `task ended with error: `
/// prefix removed) through its indented continuation lines.
fn report_of(stderr: &str) -> Option<String> {
    let lines: Vec<&str> = stderr.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.starts_with("error: ") && line.contains(" failed at "))
        .or_else(|| lines.iter().position(|line| line.contains(" failed at ")))?;
    let first = lines[start];
    let first = first
        .strip_prefix("task ended with error: ")
        .or_else(|| first.strip_prefix("error: "))
        .unwrap_or(first);
    let mut report = first.to_string();
    for line in &lines[start + 1..] {
        if !line.starts_with("  ") {
            break;
        }
        report.push('\n');
        report.push_str(line);
    }
    Some(report)
}

/// Whether two target runs of a unit agree: the same exit status and stdout,
/// and, when the unit failed, the same stderr (its failure text). The stderr
/// of a unit that exits 0 holds the lines of the tasks it spawned, written
/// while the unit ends, so it is no part of its result.
fn targets_agree(rust: &Run, llvm: &Run) -> bool {
    rust.status == llvm.status
        && rust.stdout == llvm.stdout
        && (rust.status == "0" || rust.stderr == llvm.stderr)
}

/// Run `command` with an empty stdin, killed after [`TIMEOUT`].
fn run_command(mut command: Command) -> Run {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // ETXTBSY: an executable just written is held open for an instant by
    // another test thread's fork (as `llvm_build::run_copied` retries).
    let mut attempts = 0;
    let spawned = loop {
        match command.spawn() {
            Err(e) if e.raw_os_error() == Some(26) && attempts < 100 => {
                attempts += 1;
                std::thread::sleep(Duration::from_millis(10));
            }
            other => break other,
        }
    };
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => {
            return Run {
                status: "cannot start".to_string(),
                stdout: String::new(),
                stderr: e.to_string(),
            };
        }
    };
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let read = |mut pipe: Box<dyn std::io::Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            String::from_utf8_lossy(&bytes).into_owned()
        })
    };
    let out = read(Box::new(stdout));
    let err = read(Box::new(stderr));
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    Run {
        status: match status {
            None => "timeout".to_string(),
            Some(status) => status
                .code()
                .map_or_else(|| "signal".to_string(), |code| code.to_string()),
        },
        stdout: out.join().unwrap(),
        stderr: err.join().unwrap(),
    }
}

/// Replace what differs between two runs of one program and is no part of
/// its meaning: the scratch directories of this runner.
fn normalise(text: &str, scratch: &Path) -> String {
    text.replace(&scratch.to_string_lossy().into_owned(), "<scratch>")
}

/// One run of a unit on a target: the executable, its arguments, and the
/// scratch directory it gets (its working directory for a main, its
/// `IGNITE_TEST_SCRATCH` for a test).
#[derive(Clone, Debug)]
struct Job {
    program: PathBuf,
    args: Vec<String>,
    scratch: PathBuf,
    test: bool,
}

impl Job {
    fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        if self.test {
            command
                .env("IGNITE_TEST_SCRATCH", &self.scratch)
                .current_dir(workspace());
        } else {
            command.current_dir(&self.scratch);
        }
        command
    }

    fn run(&self) -> Run {
        let _ = std::fs::remove_dir_all(&self.scratch);
        std::fs::create_dir_all(&self.scratch).unwrap();
        let mut run = run_command(self.command());
        run.stdout = normalise(&run.stdout, &self.scratch);
        run.stderr = normalise(&run.stderr, &self.scratch);
        let _ = std::fs::remove_dir_all(&self.scratch);
        run
    }
}

/// Run every job on [`JOBS`] threads.
fn run_all(jobs: &BTreeMap<String, Job>) -> BTreeMap<String, Run> {
    let queue = Mutex::new(jobs.iter().collect::<Vec<_>>());
    let results = Mutex::new(BTreeMap::new());
    std::thread::scope(|scope| {
        for _ in 0..JOBS {
            scope.spawn(|| {
                loop {
                    let Some((key, job)) = queue.lock().unwrap().pop() else {
                        break;
                    };
                    let run = job.run();
                    results.lock().unwrap().insert(key.clone(), run);
                }
            });
        }
    });
    results.into_inner().unwrap()
}

fn scratch_root() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("differential")
}

fn target_name(target: Target) -> &'static str {
    match target {
        Target::Rust => "rust",
        Target::Llvm => "llvm",
    }
}

fn build_options(target: Target, out: PathBuf) -> ignite::BuildOptions {
    ignite::BuildOptions {
        target,
        out: Some(out),
        target_dir: (target == Target::Rust).then(|| scratch_root().join("cargo")),
        runtime: (target == Target::Llvm).then(common::runner_archive),
        ..Default::default()
    }
}

/// Give the generated Rust project at `project` the workspace's
/// `Cargo.lock` before its first build. The project's dependencies are
/// `ignite-io` and its crates, which the workspace lock pins, so cargo
/// resolves from the lock and the local registry cache and never updates the
/// crates.io index: the gate stays off the network. A lock the project
/// already has (from an earlier run) is kept.
fn seed_lock(target: Target, project: &Path) {
    if target != Target::Rust || project.join("Cargo.lock").exists() {
        return;
    }
    std::fs::create_dir_all(project).unwrap();
    std::fs::copy(workspace().join("Cargo.lock"), project.join("Cargo.lock")).unwrap();
}

/// The directory `ignite::build_tests` writes the Rust project of its
/// `index`-th group to under `out` (`test_build::group_dir`).
fn group_project(out: &Path, index: usize) -> PathBuf {
    let dir = if index == 0 {
        "tests".to_string()
    } else {
        format!("tests-{}", index + 1)
    };
    out.join(dir).join("rust")
}

/// The groups `ignite::build_tests` builds `entries` into, in its order
/// (`test_build::group_files`): the standard-library modules in the first,
/// every program in the first group that does not hold its module path yet.
/// Two programs with one module path (every file without a `module` line is
/// `main`) are in two groups, so two executables.
fn test_groups<'a>(entries: &[&'a Entry]) -> Vec<Vec<&'a Entry>> {
    let mut groups: Vec<Vec<&Entry>> = Vec::new();
    for entry in entries {
        let index = if entry.std {
            0
        } else {
            groups
                .iter()
                .position(|g| g.iter().all(|e| e.module != entry.module))
                .unwrap_or(groups.len())
        };
        if index == groups.len() {
            groups.push(Vec::new());
        }
        groups[index].push(entry);
    }
    groups
}

/// A test executable with the entries whose tests it holds.
type Built<'a> = (PathBuf, Vec<&'a Entry>);

/// The test executables of `entries` on `target`, each with the entries it
/// holds; a file that does not build with the others is built alone, and
/// one that does not build alone is returned with its error.
fn test_executables<'a>(
    entries: &[&'a Entry],
    target: Target,
) -> (Vec<Built<'a>>, BTreeMap<PathBuf, String>) {
    let out = scratch_root().join(format!("tests-{}", target_name(target)));
    let options = |out: PathBuf| ignite::TestOptions {
        build: build_options(target, out),
        runner: Vec::new(),
    };
    let files: Vec<PathBuf> = entries.iter().map(|e| e.path.clone()).collect();
    let groups = test_groups(entries);
    for index in 0..groups.len() {
        seed_lock(target, &group_project(&out, index));
    }
    if let Ok(executables) = ignite::build_tests(&files, &options(out.clone())) {
        assert_eq!(
            executables.len(),
            groups.len(),
            "ignite::build_tests built {} executables for {} groups",
            executables.len(),
            groups.len()
        );
        return (
            executables.into_iter().zip(groups).collect(),
            BTreeMap::new(),
        );
    }
    let mut executables = Vec::new();
    let mut failed = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        seed_lock(
            target,
            &group_project(&out.join(format!("alone-{index}")), 0),
        );
        match ignite::build_tests(
            std::slice::from_ref(&entry.path),
            &options(out.join(format!("alone-{index}"))),
        ) {
            Ok(built) => executables.extend(built.into_iter().map(|exe| (exe, vec![*entry]))),
            Err(e) => {
                failed.insert(entry.path.clone(), e.to_string());
            }
        }
    }
    (executables, failed)
}

/// The runs of every test of `entries` on `target`, by unit key.
fn run_tests(
    entries: &[Entry],
    target: Target,
    jobs: &mut BTreeMap<String, Job>,
) -> BTreeMap<String, Run> {
    let with_tests: Vec<&Entry> = entries.iter().filter(|e| !e.tests.is_empty()).collect();
    let (executables, failed) = test_executables(&with_tests, target);
    let mut results = BTreeMap::new();
    for entry in entries {
        if let Some(error) = failed.get(&entry.path) {
            for test in &entry.tests {
                results.insert(
                    format!("{}::{test}", entry.rel),
                    Run {
                        status: "build failed".to_string(),
                        stdout: String::new(),
                        stderr: error.clone(),
                    },
                );
            }
        }
    }
    let mut mine = BTreeMap::new();
    // Each test of each file runs by its own name in the executable built
    // for that file (`--run-one <module>/<name>`), not by the lines of
    // `--list`: a test name may hold a line break
    // (`fixtures/conformance/adversary_forged_result.ignite`).
    for (exe, held) in executables {
        for entry in held {
            for name in &entry.tests {
                let key = format!("{}::{name}", entry.rel);
                let scratch = scratch_root()
                    .join(format!("scratch-{}", target_name(target)))
                    .join(mine.len().to_string());
                let job = Job {
                    program: exe.clone(),
                    args: vec!["--run-one".to_string(), format!("{}/{name}", entry.module)],
                    scratch,
                    test: true,
                };
                mine.insert(key, job);
            }
        }
    }
    results.extend(run_all(&mine));
    jobs.extend(mine);
    results
}

/// The runs of every executable of `entries` on `target`, by unit key.
fn run_mains(
    entries: &[Entry],
    target: Target,
    jobs: &mut BTreeMap<String, Job>,
) -> BTreeMap<String, Run> {
    let mut results = BTreeMap::new();
    let mut mine = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        if entry.mains.is_empty() || entry.mains_not_run.is_some() {
            continue;
        }
        let out = scratch_root().join(format!("mains-{}/{index}", target_name(target)));
        seed_lock(target, &out);
        match ignite::build(&entry.path, &build_options(target, out)) {
            Ok(built) => {
                for (name, _) in &entry.mains {
                    let key = format!("{}::main {name}", entry.rel);
                    let exe = built
                        .executables
                        .iter()
                        .find(|p| p.file_stem().and_then(|s| s.to_str()) == Some(name.as_str()))
                        .unwrap_or_else(|| panic!("{key}: not in {:?}", built.executables));
                    let scratch = scratch_root()
                        .join(format!("cwd-{}", target_name(target)))
                        .join(format!("{index}-{name}"));
                    let job = Job {
                        program: exe.clone(),
                        args: Vec::new(),
                        scratch,
                        test: false,
                    };
                    mine.insert(key, job);
                }
            }
            Err(e) => {
                for (name, _) in &entry.mains {
                    results.insert(
                        format!("{}::main {name}", entry.rel),
                        Run {
                            status: "build failed".to_string(),
                            stdout: String::new(),
                            stderr: e.to_string(),
                        },
                    );
                }
            }
        }
    }
    results.extend(run_all(&mine));
    jobs.extend(mine);
    results
}

fn cannot(error: EvalError) -> Evaluated {
    match error {
        EvalError::Failed { report } if !report.starts_with("evaluation failed: ") => {
            Evaluated::Failed(report)
        }
        EvalError::Failed { report } => Evaluated::Cannot(
            report
                .strip_prefix("evaluation failed: ")
                .unwrap_or(&report)
                .to_string(),
        ),
        EvalError::Limit {
            limit: Limit::Fuel(units),
            ..
        } => Evaluated::Cannot(format!("exceeds the evaluator's fuel of {units} units")),
        EvalError::Limit { limit, .. } => {
            Evaluated::Cannot(format!("exceeds the evaluator's limit {limit:?}"))
        }
        EvalError::Unencodable { what } => Evaluated::Cannot(format!(
            "ends with a value the evaluator cannot hold: {what}"
        )),
    }
}

/// The evaluator's outcome of every unit of `entry`.
fn evaluate(
    entry: &Entry,
    modules: &ignite_eval::Modules<'_>,
    program: &Program,
    slow: &BTreeMap<String, String>,
) -> BTreeMap<String, Evaluated> {
    let mut evaluator = Evaluator::new(modules, Fuel::with_units(EVAL_FUEL));
    let again = std::env::var_os("IGNITE_DIFFERENTIAL_SLOW").is_some();
    let mut out = BTreeMap::new();
    let trace = std::env::var_os("IGNITE_DIFFERENTIAL_TRACE").is_some();
    for test in &entry.tests {
        let started = Instant::now();
        let key = format!("{}::{test}", entry.rel);
        let listed = slow.get(&key);
        if let Some(reason) = listed
            && !again
        {
            out.insert(key, Evaluated::Cannot(reason.clone()));
            continue;
        }
        evaluator.set_time_limit(Some(if listed.is_some() { SLOW } else { GUARD }));
        let outcome = match evaluator.run_test(&entry.module, test) {
            Ok(()) => Evaluated::Passed(String::new()),
            Err(error) => cannot(error),
        };
        if trace {
            eprintln!(
                "eval {:.2} s {}::{test}",
                started.elapsed().as_secs_f64(),
                entry.rel
            );
        }
        out.insert(format!("{}::{test}", entry.rel), outcome);
    }
    if entry.mains_not_run.is_none() {
        evaluator.set_time_limit(Some(GUARD));
        for (name, printer) in &entry.mains {
            let key = format!("{}::main {name}", entry.rel);
            let outcome = if *printer {
                printed(&mut evaluator, &entry.module, program)
            } else {
                Evaluated::Cannot("a main does IO; the evaluator has none".to_string())
            };
            out.insert(key, outcome);
        }
    }
    out
}

/// A printer's stdout by the evaluator: the debug text of every fn without
/// parameters, one per line.
fn printed(evaluator: &mut Evaluator<'_>, module: &str, program: &Program) -> Evaluated {
    let mut stdout = String::new();
    for decl in &program.decls {
        let Decl::Fn(f) = decl else { continue };
        if !f.params.is_empty() {
            continue;
        }
        match evaluator.call(&QualName::new(module, f.qualified_name()), Vec::new()) {
            Ok(value) => {
                stdout.push_str(&value.debug_text());
                stdout.push('\n');
            }
            Err(error) => return cannot(error),
        }
    }
    Evaluated::Passed(stdout)
}

/// Whether the evaluator's outcome agrees with Rust's run.
fn eval_agrees(evaluated: &Evaluated, rust: &Run) -> bool {
    match evaluated {
        Evaluated::Cannot(_) => true,
        Evaluated::Passed(stdout) => {
            rust.status == "0" && (stdout.is_empty() || *stdout == rust.stdout)
        }
        Evaluated::Failed(report) => {
            rust.status == "101" && report_of(&rust.stderr).as_deref() == Some(report.as_str())
        }
    }
}

/// The result of one run of the corpus.
#[derive(Clone)]
pub struct Outcome {
    /// Unit key to its JSON record.
    pub units: BTreeMap<String, Json>,
    /// Units whose LLVM run differs from Rust's, with the difference.
    pub llvm_differs: BTreeMap<String, String>,
    /// Units whose evaluator outcome differs from Rust's run.
    pub eval_differs: BTreeMap<String, String>,
    /// Units the evaluator cannot run, with the reason.
    pub eval_cannot: BTreeMap<String, String>,
    /// Programs that check and do not build, by target.
    pub build_failed: BTreeMap<&'static str, BTreeSet<String>>,
    /// Programs excluded because they do not check, with the first line of
    /// the error.
    pub unchecked: BTreeMap<String, String>,
    /// Units with no run on a target and no listed reason (a main a
    /// `not run:` line names): each one fails the runner.
    pub no_run: BTreeMap<String, String>,
    /// Units run per consumer and inline tests of std/ and examples/ per
    /// target: the metrics.
    pub metrics: Vec<(String, String, i64)>,
}

/// Run the corpus on the three consumers.
pub fn run_corpus() -> Outcome {
    // The units the baseline lists as over the evaluator's time limit
    // ([`SLOW`]).
    let slow: BTreeMap<String, String> = quality::load_baseline()
        .eval_cannot_run
        .into_iter()
        .filter(|(_, reason)| quality::is_slow(reason))
        .collect();
    let std_root = ignite::default_std_path();
    let library = ignite::load_std(&std_root).unwrap_or_else(|e| panic!("std: {e}"));
    let (std_types, _) = ignite::type_std(&library).unwrap_or_else(|e| panic!("std: {e}"));
    let std_modules: ignite_eval::Modules<'_> = library
        .modules
        .iter()
        .zip(&std_types)
        .map(|(module, types)| ignite_eval::Module {
            path: &module.path,
            source: &module.source,
            program: &module.program,
            types,
        })
        .collect();

    let mut entries = Vec::new();
    let mut loaded = Vec::new();
    let mut unchecked = BTreeMap::new();
    for path in candidates() {
        let rel = relative(&path);
        let text = std::fs::read_to_string(&path).unwrap();
        if let Ok(inside) = path.strip_prefix(&std_root) {
            let name = format!(
                "std.{}",
                inside
                    .with_extension("")
                    .to_string_lossy()
                    .replace('/', ".")
            );
            let Some(module) = library.modules.iter().find(|m| m.path == name) else {
                continue;
            };
            let tests = test_names(&module.program);
            if tests.is_empty() {
                continue;
            }
            entries.push(Entry {
                path: path.clone(),
                rel,
                module: module.path.clone(),
                tests,
                mains: Vec::new(),
                mains_not_run: None,
                std: true,
            });
            loaded.push(None);
            continue;
        }
        let mains = executables(&text);
        let has_tests = text.lines().any(|l| l.trim_start().starts_with("test \""));
        if mains.is_empty() && !has_tests {
            continue;
        }
        let program = match ignite::load(&path, None).and_then(|l| l.check().map(|()| l)) {
            Ok(program) => program,
            Err(e) => {
                let message = e.to_string();
                unchecked.insert(rel, message.lines().next().unwrap_or("").to_string());
                continue;
            }
        };
        entries.push(Entry {
            path: path.clone(),
            rel,
            module: program.types().module.clone(),
            tests: test_names(program.program()),
            mains_not_run: not_run(&text),
            mains,
            std: false,
        });
        loaded.push(Some(program));
    }

    let ((mut rust_runs, rust_jobs), (mut llvm_runs, llvm_jobs), evaluated) =
        std::thread::scope(|scope| {
            let rust = scope.spawn(|| {
                let mut jobs = BTreeMap::new();
                let mut runs = run_tests(&entries, Target::Rust, &mut jobs);
                runs.extend(run_mains(&entries, Target::Rust, &mut jobs));
                (runs, jobs)
            });
            let llvm = scope.spawn(|| {
                let mut jobs = BTreeMap::new();
                let mut runs = run_tests(&entries, Target::Llvm, &mut jobs);
                runs.extend(run_mains(&entries, Target::Llvm, &mut jobs));
                (runs, jobs)
            });
            let next = std::sync::atomic::AtomicUsize::new(0);
            let evaluated = Mutex::new(BTreeMap::new());
            std::thread::scope(|workers| {
                for _ in 0..EVAL_JOBS {
                    workers.spawn(|| {
                        loop {
                            let index = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            let Some(entry) = entries.get(index) else {
                                break;
                            };
                            let outcomes = match &loaded[index] {
                                None => {
                                    let module = library
                                        .modules
                                        .iter()
                                        .find(|m| m.path == entry.module)
                                        .unwrap();
                                    evaluate(entry, &std_modules, &module.program, &slow)
                                }
                                Some(program) => {
                                    let modules = program.modules();
                                    evaluate(entry, &modules, program.program(), &slow)
                                }
                            };
                            evaluated.lock().unwrap().extend(outcomes);
                        }
                    });
                }
            });
            let evaluated = evaluated.into_inner().unwrap();
            (rust.join().unwrap(), llvm.join().unwrap(), evaluated)
        });

    // A unit whose targets disagree runs again on both, one after the other
    // and alone: a test that bounds a wait (`store_load`, the idle timeouts)
    // can miss its bound while the corpus loads the machine. A difference that
    // holds alone is a difference.
    let disagreeing: Vec<String> = rust_runs
        .iter()
        .filter(|(key, rust)| {
            llvm_runs
                .get(*key)
                .is_some_and(|llvm| !targets_agree(rust, llvm))
        })
        .map(|(key, _)| key.clone())
        .collect();
    for key in disagreeing {
        if let (Some(rust), Some(llvm)) = (rust_jobs.get(&key), llvm_jobs.get(&key)) {
            rust_runs.insert(key.clone(), rust.run());
            llvm_runs.insert(key.clone(), llvm.run());
        }
    }

    let mut keys: BTreeSet<String> = BTreeSet::new();
    for entry in &entries {
        for test in &entry.tests {
            keys.insert(format!("{}::{test}", entry.rel));
        }
        for (name, _) in &entry.mains {
            keys.insert(format!("{}::main {name}", entry.rel));
        }
    }
    let mut units = BTreeMap::new();
    let mut llvm_differs = BTreeMap::new();
    let mut eval_differs = BTreeMap::new();
    let mut eval_cannot = BTreeMap::new();
    let mut no_run = BTreeMap::new();
    let mut build_failed: BTreeMap<&'static str, BTreeSet<String>> = BTreeMap::new();
    let mut ran: BTreeMap<&str, i64> = BTreeMap::new();
    let mut inline: BTreeMap<&str, i64> = BTreeMap::new();
    for key in &keys {
        let (file, _) = key.split_once("::").unwrap();
        let entry = entries.iter().find(|e| e.rel == file).unwrap();
        let inline_test = !key.contains("::main ") && (entry.std || file.starts_with("examples/"));
        let mut record = BTreeMap::new();
        let rust = rust_runs.get(key);
        let llvm = llvm_runs.get(key);
        let eval = evaluated.get(key);
        let listed = entry.mains_not_run.filter(|_| key.contains("::main "));
        if let (None, None, None, Some(reason)) = (rust, llvm, eval, listed) {
            record.insert("not_run".to_string(), Json::Str(reason.to_string()));
            units.insert(key.clone(), Json::Object(record));
            continue;
        }
        for (name, run) in [("rust", rust), ("llvm", llvm)] {
            match run {
                Some(run) => {
                    if run.status == "build failed" {
                        build_failed
                            .entry(name)
                            .or_default()
                            .insert(file.to_string());
                    } else {
                        *ran.entry(name).or_default() += 1;
                        if inline_test {
                            *inline.entry(name).or_default() += 1;
                        }
                    }
                    record.insert(name.to_string(), run.json());
                }
                None => {
                    record.insert(name.to_string(), Json::Str("missing".to_string()));
                }
            }
        }
        match (rust, llvm) {
            (Some(rust), Some(llvm)) if !targets_agree(rust, llvm) => {
                let what = if rust.status != llvm.status {
                    format!("exit {} on Rust, {} on LLVM", rust.status, llvm.status)
                } else if rust.stdout != llvm.stdout {
                    "stdout differs".to_string()
                } else {
                    "stderr differs".to_string()
                };
                llvm_differs.insert(key.clone(), what);
            }
            (Some(_), None) | (None, Some(_)) | (None, None) => {
                let on: Vec<&str> = [("Rust", rust), ("LLVM", llvm)]
                    .iter()
                    .filter(|(_, run)| run.is_none())
                    .map(|(name, _)| *name)
                    .collect();
                no_run.insert(key.clone(), format!("no run on {}", on.join(" and ")));
            }
            _ => {}
        }
        match eval {
            Some(Evaluated::Cannot(reason)) => {
                eval_cannot.insert(key.clone(), reason.clone());
                record.insert("eval".to_string(), Json::Str(format!("cannot: {reason}")));
            }
            Some(evaluated) => {
                *ran.entry("eval").or_default() += 1;
                if inline_test {
                    *inline.entry("eval").or_default() += 1;
                }
                record.insert(
                    "eval".to_string(),
                    Json::Str(match evaluated {
                        Evaluated::Passed(_) => "passed".to_string(),
                        Evaluated::Failed(report) => format!("failed: {report}"),
                        Evaluated::Cannot(_) => unreachable!(),
                    }),
                );
                if let Some(rust) = rust
                    && rust.status != "build failed"
                    && !eval_agrees(evaluated, rust)
                {
                    eval_differs.insert(
                        key.clone(),
                        format!(
                            "Rust exits {}, the evaluator {}",
                            rust.status,
                            match evaluated {
                                Evaluated::Passed(_) => "passes".to_string(),
                                Evaluated::Failed(report) => format!("fails: {report}"),
                                Evaluated::Cannot(_) => unreachable!(),
                            }
                        ),
                    );
                }
            }
            None => {}
        }
        units.insert(key.clone(), Json::Object(record));
    }
    let mut metrics = Vec::new();
    for target in ["rust", "llvm", "eval"] {
        metrics.push((
            "1.2".to_string(),
            target.to_string(),
            ran.get(target).copied().unwrap_or(0),
        ));
        metrics.push((
            "4.3".to_string(),
            target.to_string(),
            inline.get(target).copied().unwrap_or(0),
        ));
    }
    metrics.push((
        "1.1".to_string(),
        "llvm".to_string(),
        llvm_differs.len() as i64,
    ));
    metrics.push((
        "1.1".to_string(),
        "eval".to_string(),
        eval_differs.len() as i64,
    ));
    metrics.push((
        "1.6".to_string(),
        "eval".to_string(),
        eval_cannot.len() as i64,
    ));
    for target in ["rust", "llvm"] {
        metrics.push((
            "2.2".to_string(),
            target.to_string(),
            build_failed.get(target).map_or(0, |s| s.len() as i64),
        ));
    }
    Outcome {
        units,
        llvm_differs,
        eval_differs,
        eval_cannot,
        build_failed,
        unchecked,
        no_run,
        metrics,
    }
}

fn strings(map: &BTreeMap<String, String>) -> Json {
    Json::Object(
        map.iter()
            .map(|(k, v)| (k.clone(), Json::Str(v.clone())))
            .collect(),
    )
}

/// Write the run to `target/quality/`.
fn write_outcome(outcome: &Outcome) {
    let dir = workspace().join("target/quality");
    std::fs::create_dir_all(&dir).unwrap();
    let mut root = BTreeMap::new();
    root.insert("units".to_string(), Json::Object(outcome.units.clone()));
    root.insert("llvm_differs".to_string(), strings(&outcome.llvm_differs));
    root.insert("eval_differs".to_string(), strings(&outcome.eval_differs));
    root.insert("eval_cannot_run".to_string(), strings(&outcome.eval_cannot));
    root.insert("unchecked".to_string(), strings(&outcome.unchecked));
    root.insert(
        "build_failed".to_string(),
        Json::Object(
            outcome
                .build_failed
                .iter()
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        Json::Array(v.iter().map(|s| Json::Str(s.clone())).collect()),
                    )
                })
                .collect(),
        ),
    );
    let mut text = String::new();
    Json::Object(root).write(&mut text, 0);
    text.push('\n');
    std::fs::write(dir.join("differential.json"), text).unwrap();
    // The values `quality::check_differential` holds: 1.1 without the
    // intermittent units, and 1.7.
    quality::write_measured(
        &dir.join("differential-metrics.json"),
        &quality::steady(&quality::load_baseline(), outcome),
    );
}

/// Metric 1.1: one result across Rust, LLVM and the evaluator.
#[test]
fn corpus_agrees_across_rust_llvm_and_the_evaluator() {
    let started = Instant::now();
    let outcome = run_corpus();
    write_outcome(&outcome);
    eprintln!(
        "differential: {} units, {} LLVM differences, {} evaluator differences, {} the evaluator cannot run, {:.1} s",
        outcome.units.len(),
        outcome.llvm_differs.len(),
        outcome.eval_differs.len(),
        outcome.eval_cannot.len(),
        started.elapsed().as_secs_f64()
    );
    let errors = quality::check_differential(&outcome);
    assert!(
        errors.is_empty(),
        "the differential runner refuses this run (target/quality/differential.json has every outcome):\n  {}",
        errors.join("\n  ")
    );
}

#[test]
fn a_report_is_read_from_stderr_with_its_continuation_lines() {
    let stderr =
        "log line\nerror: assert failed at a.ignite:3:3: x == y\n  left: 1\n  right: 2\nafter\n";
    assert_eq!(
        report_of(stderr).as_deref(),
        Some("assert failed at a.ignite:3:3: x == y\n  left: 1\n  right: 2")
    );
    assert_eq!(report_of("nothing\n"), None);
    let rust = Run {
        status: "101".to_string(),
        stdout: String::new(),
        stderr: stderr.to_string(),
    };
    assert!(eval_agrees(
        &Evaluated::Failed(
            "assert failed at a.ignite:3:3: x == y\n  left: 1\n  right: 2".to_string()
        ),
        &rust
    ));
    assert!(!eval_agrees(
        &Evaluated::Failed("assert failed at a.ignite:3:3: x == y".to_string()),
        &rust
    ));
    assert!(!eval_agrees(&Evaluated::Passed(String::new()), &rust));
    let tasks = "task ended with error: index failed at a.ignite:9:1: 7\nerror: assert failed at a.ignite:3:3: x\n";
    assert_eq!(
        report_of(tasks).as_deref(),
        Some("assert failed at a.ignite:3:3: x"),
        "the main failure, not a task's"
    );
}

#[test]
fn targets_agree_on_exit_stdout_and_the_failure_text() {
    let run = |status: &str, stdout: &str, stderr: &str| Run {
        status: status.to_string(),
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
    };
    assert!(targets_agree(&run("0", "a", "x"), &run("0", "a", "")));
    assert!(!targets_agree(&run("0", "a", ""), &run("0", "b", "")));
    assert!(!targets_agree(&run("0", "a", ""), &run("101", "a", "")));
    assert!(!targets_agree(&run("101", "", "x"), &run("101", "", "y")));
    assert!(targets_agree(&run("101", "", "x"), &run("101", "", "x")));
}

fn fixture(name: &str) -> PathBuf {
    workspace().join("crates/ignite/tests/fixtures").join(name)
}

/// Known difference (`quality/baseline.json`, `story:eval-try-report`):
/// a failed `?` in a test is reported by both targets at the `?` with the
/// error's text, `? failed at <file>:50:21: Err(not a number: x)`; the
/// evaluator reports it at the start of the operand with the error's debug
/// text, `<file>:50:11: Err(IoError("not a number: x"))`, and a `None` at
/// the operand's start too.
#[test]
#[ignore = "story:eval-try-report: the evaluator reports a failed `?` at another column and with another text than both targets"]
fn the_evaluator_reports_a_failed_try_as_the_targets_do() {
    let path = fixture("tests/basics.ignite");
    let loaded = ignite::load(&path, None).unwrap_or_else(|e| panic!("{e}"));
    let modules = loaded.modules();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    let module = loaded.types().module.clone();
    let file = loaded.source().path.clone();
    assert_eq!(
        evaluator.run_test(&module, "try on err fails"),
        Err(EvalError::Failed {
            report: format!("? failed at {file}:50:21: Err(not a number: x)")
        })
    );
    assert_eq!(
        evaluator.run_test(&module, "try on none fails"),
        Err(EvalError::Failed {
            report: format!("? failed at {file}:55:25: None")
        })
    );
}

/// Known difference (`quality/baseline.json`,
/// `story:eval-receiver-order`): in `data.slice(a, b)` whose bound changes
/// `data`, both targets slice the receiver as it was before the bound ran;
/// the evaluator reads it after, so the two std.text tests that pin the
/// targets' order fail in it.
#[test]
#[ignore = "story:eval-receiver-order: the evaluator reads a builtin's receiver after its arguments, the targets before"]
fn the_evaluator_reads_a_receiver_before_its_arguments() {
    let library = ignite::load_std(&ignite::default_std_path()).unwrap_or_else(|e| panic!("{e}"));
    let (types, _) = ignite::type_std(&library).unwrap_or_else(|e| panic!("{e}"));
    let modules: ignite_eval::Modules<'_> = library
        .modules
        .iter()
        .zip(&types)
        .map(|(module, types)| ignite_eval::Module {
            path: &module.path,
            source: &module.source,
            program: &module.program,
            types,
        })
        .collect();
    let mut evaluator = Evaluator::new(&modules, Fuel::default());
    for test in [
        "a later bound that changes the bytes leaves the slice before it as it was",
        "a start bound, both bounds or a bound in a loop that change the receiver",
    ] {
        assert_eq!(evaluator.run_test("std.text", test), Ok(()), "{test}");
    }
}

/// Adversary (story quality-ratchet, pass 1): two fixtures without a
/// `module` declaration both have the module path `main`. `run_tests` keys
/// a listed test by the first entry of its module, so every test of the
/// second file gets no run on the target, and `run_corpus` then compares
/// none of its units (neither LLVM with Rust nor the evaluator with Rust).
/// In the committed corpus that is 186 units (slice9/, slice16/, streams/).
#[test]
fn adversary_every_test_of_two_files_with_one_module_path_runs_on_a_target() {
    let entry = |name: &str| {
        let path = fixture(name);
        let loaded = ignite::load(&path, None).unwrap_or_else(|e| panic!("{e}"));
        Entry {
            rel: relative(&path),
            module: loaded.types().module.clone(),
            tests: test_names(loaded.program()),
            mains: Vec::new(),
            mains_not_run: None,
            std: false,
            path,
        }
    };
    let entries = vec![entry("slice16/holds.ignite"), entry("slice16/moves.ignite")];
    assert_eq!(entries[0].module, entries[1].module, "one module path");
    let mut jobs = BTreeMap::new();
    let runs = run_tests(&entries, Target::Llvm, &mut jobs);
    let missing: Vec<String> = entries
        .iter()
        .flat_map(|e| e.tests.iter().map(move |t| format!("{}::{t}", e.rel)))
        .filter(|key| !runs.contains_key(key))
        .collect();
    assert!(
        missing.is_empty(),
        "units with no run on LLVM: {missing:#?}"
    );
}
