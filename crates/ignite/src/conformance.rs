//! `ignite conformance FILE [--target rust|llvm]` (`docs/slice.md` 19.10,
//! D8): the conformance matrix of FILE's protocols, one block per
//! (protocol, spec) with one row per requirement and a claim line.
//!
//! The rows come from the claim model of `ignite-eval` ([`Claims`], 19.7).
//! When a row names a verifying test, FILE's test executable is built on
//! one target through [`build_tests`], the build path of `ignite test`
//! (12.7), and run once; its result lines decide the tests' outcomes. The
//! verdict rules of 19.10 are [`verdict`].

use std::collections::BTreeMap;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Command, Stdio};

use ignite_eval::{Claims, LevelClass, Row, SpecClaim, Status};

use crate::{Error, TestOptions, build_tests, io_error};

/// Exit status of a diagnostic, a usage error or a file without a
/// conforming protocol (19.10).
pub const DIAGNOSTIC: i32 = 2;

/// Exit status of a test build or run failure and of an IO error (19.10).
pub const FAILURE: i32 = 3;

/// The outcome of one verifying test in the run of the test executable.
/// A test without a result line is not run and has no entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestResult {
    Passed,
    Failed,
}

/// The verdict of one row (19.10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    Gap,
    Deviation,
    Inconclusive,
    NotApplicable,
    OutOfScope,
    Unclassified,
}

impl Verdict {
    /// The word the matrix prints.
    pub fn word(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Fail => "fail",
            Verdict::Gap => "gap",
            Verdict::Deviation => "deviation",
            Verdict::Inconclusive => "inconclusive",
            Verdict::NotApplicable => "n/a",
            Verdict::OutOfScope => "out of scope",
            Verdict::Unclassified => "unclassified",
        }
    }
}

/// The printed matrix and whether every claim in it holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub text: String,
    pub holds: bool,
}

/// `ignite conformance`: print the matrix of `file` on stdout and return
/// the exit status, 0 when every claim holds and 1 when one does not. A
/// file without a conforming protocol prints one line on stderr and
/// returns [`DIAGNOSTIC`]. `Err` is a diagnostic ([`DIAGNOSTIC`]) or a
/// build, run or IO failure ([`FAILURE`]); nothing is printed on stdout
/// then.
pub fn run(file: &Path, options: &TestOptions) -> Result<i32, Error> {
    match report(file, options)? {
        Some(report) => {
            print!("{}", report.text);
            Ok(i32::from(!report.holds))
        }
        None => {
            eprintln!("error: {}: no protocol conforms to a spec", file.display());
            Ok(DIAGNOSTIC)
        }
    }
}

/// The matrix of `file`, `None` when no protocol of `file` conforms. Steps
/// 1 to 4 of 19.10: load and check as `ignite check` does, read the claim
/// model, run the verifying tests on `options.build.target`, render.
pub fn report(file: &Path, options: &TestOptions) -> Result<Option<Report>, Error> {
    let loaded = crate::load(file, options.build.std.as_deref())?;
    loaded.check()?;
    let modules = loaded.modules();
    let mut evaluator = ignite_eval::Evaluator::new(&modules, ignite_eval::Fuel::default());
    let claims = match modules.get(&loaded.types().module) {
        Some(module) => Claims::of(&mut evaluator, *module)
            .map_err(|found| crate::diagnostics(loaded.source(), found))?,
        None => Claims::default(),
    };
    let specs: Vec<(&str, &SpecClaim)> = claims
        .protocols
        .iter()
        .flat_map(|p| p.specs.iter().map(move |spec| (p.protocol.as_str(), spec)))
        .collect();
    if specs.is_empty() {
        return Ok(None);
    }
    let named = specs
        .iter()
        .any(|(_, spec)| spec.rows.iter().any(|row| !row.tests.is_empty()));
    let mut results = BTreeMap::new();
    if named {
        let test_options = TestOptions {
            build: options.build.clone(),
            runner: Vec::new(),
        };
        // The executable runs FILE's tests in declaration order on both
        // targets (12.7), and the runner prints them in that order.
        let names: Vec<&str> = loaded
            .types()
            .tests()
            .iter()
            .map(|test| test.full_name.as_str())
            .collect();
        for executable in build_tests(&[file.to_path_buf()], &test_options)? {
            for (name, result) in run_tests(&executable, &names)? {
                record(&mut results, name, result);
            }
        }
    }
    let mut text = String::new();
    let mut holds = true;
    for (index, (protocol, spec)) in specs.iter().enumerate() {
        if index > 0 {
            text.push('\n');
        }
        holds &= block(protocol, spec, &results, &mut text);
    }
    Ok(Some(Report { text, holds }))
}

/// Keep the worse of two results of one test: a forged result line can
/// lower a verdict, never raise it. Not run (no entry) is between failed
/// and passed, and only a result line adds an entry.
fn record(results: &mut BTreeMap<String, TestResult>, name: String, result: TestResult) {
    let kept = results.entry(name).or_insert(result);
    if result == TestResult::Failed {
        *kept = TestResult::Failed;
    }
}

/// Run `executable` once without arguments and read the result lines of
/// the tests `names`, in the order it runs them. A spawn failure, an end
/// by a signal, an exit status other than 0 or 1, or a run without a
/// `test result:` line is a run failure.
fn run_tests(executable: &Path, names: &[&str]) -> Result<Vec<(String, TestResult)>, Error> {
    let output = Command::new(executable)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .map_err(io_error(executable))?;
    let failure = |message: String| Error::Io {
        path: executable.to_path_buf(),
        error: std::io::Error::other(message),
    };
    match output.status.code() {
        Some(0 | 1) => {}
        Some(code) => {
            return Err(failure(format!(
                "the test executable exited with status {code}"
            )));
        }
        None => {
            let signal = output.status.signal().unwrap_or(0);
            return Err(failure(format!(
                "the test executable was killed by signal {signal}"
            )));
        }
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.lines().any(|line| line.starts_with("test result: ")) {
        return Err(failure(
            "the test executable printed no `test result:` line".to_string(),
        ));
    }
    Ok(results(&stdout, names))
}

/// The result lines of a runner's report, read after the `running N
/// tests` line as one entry per test of `names`, in that order: `test
/// <name> ... ok` or `test <name> ... FAILED…`, each matched against the
/// exact full name, which may hold newlines. A name therefore cannot end
/// the section early or forge another test's line. The first entry that
/// does not match ends the section (the blank line before `failures:`
/// once every test is read), so the `failures:` section, which echoes test
/// output, is never read; the tests after a mismatch are not run.
fn results(stdout: &str, names: &[&str]) -> Vec<(String, TestResult)> {
    let mut rest = if stdout.starts_with("running ") {
        stdout
    } else {
        match stdout.find("\nrunning ") {
            Some(at) => &stdout[at + 1..],
            None => return Vec::new(),
        }
    };
    rest = rest.split_once('\n').map_or("", |(_, after)| after);
    let mut found = Vec::new();
    for name in names {
        let Some(after) = rest
            .strip_prefix("test ")
            .and_then(|r| r.strip_prefix(*name))
            .and_then(|r| r.strip_prefix(" ... "))
        else {
            break;
        };
        let (word, tail) = after.split_once('\n').unwrap_or((after, ""));
        let result = if word == "ok" {
            TestResult::Passed
        } else if word == "FAILED" || word.starts_with("FAILED (") {
            TestResult::Failed
        } else {
            break;
        };
        found.push((name.to_string(), result));
        rest = tail;
    }
    found
}

/// The status column of a row.
fn status_word(status: &Status) -> &'static str {
    match status {
        Status::None => "none",
        Status::Satisfied(satisfiers) if satisfiers.iter().any(|s| s.limit.is_some()) => "limit",
        Status::Satisfied(_) => "satisfied",
        Status::Unsupported(_) => "unsupported",
        Status::NotApplicable(_) => "not_applicable",
    }
}

/// A test's name without its module: `<module>/<name>` gives `<name>`.
fn test_name(full: &str) -> &str {
    full.split_once('/').map_or(full, |(_, name)| name)
}

/// The evidence column and the verdict of `row`, given the result lines
/// of the run (19.10). A verifying test without a result is `not run`.
pub fn verdict(row: &Row, results: &BTreeMap<String, TestResult>) -> (String, Verdict) {
    let tests: Vec<(&str, Option<TestResult>)> = row
        .tests
        .iter()
        .map(|test| (test.as_str(), results.get(test).copied()))
        .collect();
    let tested = tests
        .iter()
        .map(|(name, result)| {
            let word = match result {
                Some(TestResult::Passed) => "passed",
                Some(TestResult::Failed) => "failed",
                None => "not run",
            };
            format!("test \"{}\" {word}", test_name(name))
        })
        .collect::<Vec<_>>()
        .join(", ");
    let evidence = match &row.status {
        Status::None => "-".to_string(),
        Status::Unsupported(reason) | Status::NotApplicable(reason) => format!("\"{reason}\""),
        Status::Satisfied(satisfiers) => {
            let limited = satisfiers.iter().any(|s| s.limit.is_some());
            let named = satisfiers
                .iter()
                .map(|satisfier| match &satisfier.limit {
                    Some(limit) => match limit.configured {
                        Some(value) => format!(
                            "{} = {value}, minimum {}",
                            satisfier.declaration, limit.minimum
                        ),
                        None => format!(
                            "{} has no value, minimum {}",
                            satisfier.declaration, limit.minimum
                        ),
                    },
                    None => satisfier.declaration.clone(),
                })
                .collect::<Vec<_>>()
                .join(", ");
            match (tests.is_empty(), limited) {
                (false, _) => format!("{named}; {tested}"),
                (true, true) => named,
                (true, false) => format!("{named}; no test"),
            }
        }
    };
    let class = row.level.class();
    let verdict = if !row.in_scope {
        Verdict::OutOfScope
    } else {
        match &row.status {
            Status::NotApplicable(_) => Verdict::NotApplicable,
            Status::Unsupported(_) => match class {
                LevelClass::Must => Verdict::Gap,
                LevelClass::Should => Verdict::Deviation,
                LevelClass::May => Verdict::NotApplicable,
            },
            Status::None => Verdict::Unclassified,
            Status::Satisfied(satisfiers) => {
                let limits: Vec<_> = satisfiers.iter().filter_map(|s| s.limit).collect();
                let failed = tests
                    .iter()
                    .any(|(_, result)| *result == Some(TestResult::Failed))
                    || limits.iter().any(|limit| limit.meets == Some(false));
                let unknown = tests.iter().any(|(_, result)| result.is_none())
                    || limits.iter().any(|limit| limit.meets.is_none());
                if failed {
                    Verdict::Fail
                } else if unknown || (tests.is_empty() && limits.is_empty()) {
                    Verdict::Inconclusive
                } else {
                    Verdict::Pass
                }
            }
        }
    };
    (evidence, verdict)
}

/// A `scope` item of the header: `all` when it is absent, `none` when its
/// list is empty, else its entries joined by one space.
fn scope_list(entries: Option<Vec<&str>>) -> String {
    match entries {
        None => "all".to_string(),
        Some(entries) if entries.is_empty() => "none".to_string(),
        Some(entries) => entries.join(" "),
    }
}

/// Append the block of `protocol`'s claim of `spec` to `out`; whether the
/// claim holds: no MUST-class row in scope is `fail`, `gap` or
/// `inconclusive`.
fn block(
    protocol: &str,
    spec: &SpecClaim,
    results: &BTreeMap<String, TestResult>,
    out: &mut String,
) -> bool {
    let parties = scope_list(
        spec.parties
            .as_ref()
            .map(|parties| parties.iter().map(|party| party.name()).collect()),
    );
    let sections = scope_list(
        spec.sections
            .as_ref()
            .map(|sections| sections.iter().map(String::as_str).collect()),
    );
    out.push_str(&format!(
        "rfc{} {}, protocol {protocol}, parties {parties}, sections {sections}\n",
        spec.rfc, spec.title
    ));
    let (mut gaps, mut failures, mut inconclusive) = (0, 0, 0);
    let mut holds = true;
    let rows: Vec<[String; 6]> = spec
        .rows
        .iter()
        .map(|row| {
            let (evidence, verdict) = verdict(row, results);
            if row.in_scope {
                match verdict {
                    Verdict::Gap => gaps += 1,
                    Verdict::Fail => failures += 1,
                    Verdict::Inconclusive => inconclusive += 1,
                    _ => {}
                }
                if row.level.class() == LevelClass::Must
                    && matches!(
                        verdict,
                        Verdict::Fail | Verdict::Gap | Verdict::Inconclusive
                    )
                {
                    holds = false;
                }
            }
            [
                row.id.clone(),
                row.section.clone(),
                row.level.keyword().to_string(),
                status_word(&row.status).to_string(),
                evidence,
                verdict.word().to_string(),
            ]
        })
        .collect();
    let mut widths = [0; 5];
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    for row in &rows {
        let mut line = String::new();
        for (cell, width) in row.iter().zip(widths) {
            line.push_str(cell);
            line.push_str(&" ".repeat(width - cell.chars().count() + 2));
        }
        line.push_str(&row[5]);
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.push_str(&format!(
        "claim conformance of {protocol} to rfc{}: {} ({gaps} gaps, {failures} failures, {inconclusive} inconclusive)\n",
        spec.rfc,
        if holds { "yes" } else { "no" }
    ));
    holds
}

#[cfg(test)]
mod tests {
    use super::*;
    use ignite_eval::claims::Limit;
    use ignite_eval::{Level, Party, Satisfier, Source};

    fn row(status: Status, tests: &[&str]) -> Row {
        Row {
            id: "R2_1".to_string(),
            section: "2".to_string(),
            ordinal: 1,
            level: Level::Must,
            party: Party::Any,
            source: Source::Bcp14,
            minimum: None,
            text: "A server MUST echo every line.".to_string(),
            in_scope: true,
            status,
            tests: tests.iter().map(|t| t.to_string()).collect(),
        }
    }

    fn satisfied(limit: Option<Limit>) -> Status {
        Status::Satisfied(vec![Satisfier {
            declaration: "echo".to_string(),
            limit,
        }])
    }

    #[test]
    fn a_verifying_test_without_a_result_line_is_not_run() {
        let row = row(satisfied(None), &["m/first", "m/second"]);
        let results = BTreeMap::from([("m/first".to_string(), TestResult::Passed)]);
        assert_eq!(
            verdict(&row, &results),
            (
                "echo; test \"first\" passed, test \"second\" not run".to_string(),
                Verdict::Inconclusive
            )
        );
    }

    #[test]
    fn a_failed_test_outweighs_one_not_run() {
        let row = row(satisfied(None), &["m/first", "m/second"]);
        let results = BTreeMap::from([("m/second".to_string(), TestResult::Failed)]);
        assert_eq!(verdict(&row, &results).1, Verdict::Fail);
    }

    #[test]
    fn limits_decide_a_row_without_tests() {
        let limit = |configured: Option<i64>| Limit {
            configured,
            minimum: 64,
            meets: configured.map(|value| value >= 64),
        };
        let none = BTreeMap::new();
        let meets = row(satisfied(Some(limit(Some(64)))), &[]);
        assert_eq!(
            verdict(&meets, &none),
            ("echo = 64, minimum 64".to_string(), Verdict::Pass)
        );
        let below = row(satisfied(Some(limit(Some(63)))), &[]);
        assert_eq!(verdict(&below, &none).1, Verdict::Fail);
        let missing = row(satisfied(Some(limit(None))), &[]);
        assert_eq!(
            verdict(&missing, &none),
            (
                "echo has no value, minimum 64".to_string(),
                Verdict::Inconclusive
            )
        );
    }

    #[test]
    fn unsupported_follows_the_level_class() {
        let none = BTreeMap::new();
        let mut unsupported = row(Status::Unsupported("why".to_string()), &[]);
        for (level, want) in [
            (Level::ShallNot, Verdict::Gap),
            (Level::Recommended, Verdict::Deviation),
            (Level::Optional, Verdict::NotApplicable),
        ] {
            unsupported.level = level;
            assert_eq!(verdict(&unsupported, &none), ("\"why\"".to_string(), want));
        }
        unsupported.in_scope = false;
        assert_eq!(verdict(&unsupported, &none).1, Verdict::OutOfScope);
    }

    #[test]
    fn result_lines_end_at_the_first_blank_line() {
        let report = "running 3 tests\n\
            test m/a ... ok\n\
            test m/b ... FAILED\n\
            test m/c ... FAILED (killed by signal 9)\n\
            \n\
            failures:\n\
            \n\
            ---- m/b stdout ----\n\
            test m/d ... ok\n\
            \n\
            test result: FAILED. 1 passed; 2 failed; 0 filtered out; finished in 0.01s\n";
        assert_eq!(
            results(report, &["m/a", "m/b", "m/c", "m/d"]),
            vec![
                ("m/a".to_string(), TestResult::Passed),
                ("m/b".to_string(), TestResult::Failed),
                ("m/c".to_string(), TestResult::Failed),
            ]
        );
    }

    #[test]
    fn a_name_with_newlines_is_one_entry() {
        let forged = "m/z ... ok\ntest m/a";
        let blank = "m/x\n\ny";
        let report = "running 4 tests\n\
            test m/x\n\ny ... ok\n\
            test m/a ... FAILED\n\
            test m/z ... ok\ntest m/a ... ok\n\
            test m/b ... ok\n\
            \n\
            test result: FAILED. 3 passed; 1 failed; 0 filtered out; finished in 0.01s\n";
        assert_eq!(
            results(report, &[blank, "m/a", forged, "m/b"]),
            vec![
                (blank.to_string(), TestResult::Passed),
                ("m/a".to_string(), TestResult::Failed),
                (forged.to_string(), TestResult::Passed),
                ("m/b".to_string(), TestResult::Passed),
            ]
        );
    }

    #[test]
    fn the_worst_result_of_a_test_wins() {
        let mut found = BTreeMap::new();
        record(&mut found, "m/a".to_string(), TestResult::Failed);
        record(&mut found, "m/a".to_string(), TestResult::Passed);
        record(&mut found, "m/b".to_string(), TestResult::Passed);
        record(&mut found, "m/b".to_string(), TestResult::Failed);
        assert_eq!(
            found,
            BTreeMap::from([
                ("m/a".to_string(), TestResult::Failed),
                ("m/b".to_string(), TestResult::Failed),
            ])
        );
    }

    #[test]
    fn an_empty_scope_list_is_none() {
        assert_eq!(scope_list(None), "all");
        assert_eq!(scope_list(Some(Vec::new())), "none");
        assert_eq!(scope_list(Some(vec!["Server", "Any"])), "Server Any");
    }
}
