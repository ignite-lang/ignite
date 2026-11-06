//! The test runner of `docs/slice.md` section 12.7 and the failure reports
//! of `assert` and `?` (section 12.5), written once for both targets: the
//! Rust target's `ignite-test` executable calls [`test_main`], the LLVM
//! target's main calls `ignite_rt_test_main`, which forwards to
//! [`test_main_with`].
//!
//! The runner starts every selected test as a child process of the same
//! executable (`--run-one <full name>`), so a failed assert, a panic, a
//! `Process.exit`, a leaked socket or raw terminal mode ends only that
//! child. Children run at most `--jobs` at a time on a pool of threads;
//! each has a deadline (`--timeout`) after which it is killed. Results
//! are printed in the order of `tests`, each once it and every test before
//! it have finished, so the output does not depend on scheduling.

use std::io::Write;
use std::process::{Command, ExitStatus};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::FAILURE_EXIT_CODE;

/// Exit status of a malformed runner command line and of an unknown
/// `--run-one` name.
pub const USAGE_EXIT_CODE: i32 = 2;

/// Default per-test timeout in seconds.
pub const DEFAULT_TIMEOUT: u64 = 60;

/// One test of a test executable: its full name (`<module>/<name>`) and
/// its body.
pub struct Test<'a> {
    pub name: &'a str,
    pub run: &'a dyn Fn(),
}

/// The runner's command line, without the executable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arguments {
    pub run_one: Option<String>,
    pub filter: Option<String>,
    pub exact: bool,
    pub list: bool,
    pub jobs: usize,
    pub timeout: u64,
}

impl Arguments {
    /// Parse `args` (without the executable). An unknown flag, a missing or
    /// malformed value or a second filter is an error message.
    pub fn parse(args: &[String]) -> Result<Arguments, String> {
        let mut parsed = Arguments {
            run_one: None,
            filter: None,
            exact: false,
            list: false,
            jobs: std::thread::available_parallelism().map_or(1, usize::from),
            timeout: DEFAULT_TIMEOUT,
        };
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let mut value = |flag: &str| {
                args.next()
                    .cloned()
                    .ok_or_else(|| format!("{flag} needs a value"))
            };
            match arg.as_str() {
                "--run-one" => parsed.run_one = Some(value("--run-one")?),
                "--exact" => parsed.exact = true,
                "--list" => parsed.list = true,
                "--jobs" => {
                    let text = value("--jobs")?;
                    parsed.jobs = text
                        .parse::<usize>()
                        .ok()
                        .filter(|jobs| *jobs > 0)
                        .ok_or_else(|| format!("--jobs {text}: expected a positive integer"))?;
                }
                "--timeout" => {
                    let text = value("--timeout")?;
                    parsed.timeout = text
                        .parse::<u64>()
                        .ok()
                        .filter(|secs| *secs > 0)
                        .ok_or_else(|| format!("--timeout {text}: expected a positive integer"))?;
                }
                flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
                filter => {
                    if parsed.filter.is_some() {
                        return Err(format!("unexpected argument {filter}: one filter only"));
                    }
                    parsed.filter = Some(filter.to_string());
                }
            }
        }
        Ok(parsed)
    }

    /// Whether the runner selects the test named `name`.
    pub fn selects(&self, name: &str) -> bool {
        match &self.filter {
            None => true,
            Some(filter) if self.exact => name == filter,
            Some(filter) => name.contains(filter.as_str()),
        }
    }
}

/// The runner of `docs/slice.md` section 12.7 over the process arguments.
pub fn test_main(tests: &[Test<'_>]) -> ! {
    test_main_with(std::env::args().collect(), tests)
}

/// [`test_main`] with the process arguments given, `args[0]` being the
/// executable.
pub fn test_main_with(args: Vec<String>, tests: &[Test<'_>]) -> ! {
    let parsed = match Arguments::parse(args.get(1..).unwrap_or_default()) {
        Ok(parsed) => parsed,
        Err(message) => {
            crate::log(&format!("error: {message}"));
            crate::log(
                "usage: ignite-test [FILTER] [--exact] [--list] [--jobs N] [--timeout SECS] | --run-one NAME",
            );
            crate::exit(USAGE_EXIT_CODE)
        }
    };
    if let Some(name) = &parsed.run_one {
        run_one(tests, name)
    }
    let code = run_tests(tests, &parsed);
    crate::exit(code)
}

/// Run the test `name` in this process under [`crate::run`] and exit:
/// 0 when its body returns, 101 when it fails (a failed `assert` or `?`, an
/// overflow, a panic: `run` logs the report), the code of a `Process.exit`
/// it calls. A failure in a task the test spawned ends that task only, as
/// in a program.
fn run_one(tests: &[Test<'_>], name: &str) -> ! {
    let Some(test) = tests.iter().find(|test| test.name == name) else {
        crate::log(&format!("no test named {name}"));
        crate::exit(USAGE_EXIT_CODE)
    };
    let code = crate::run(|| {
        (test.run)();
        Ok(())
    });
    crate::exit(code)
}

/// Why a test failed, or that it passed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Passed,
    /// Exit 101: a failed assert or `?`, or a panic.
    Failed,
    Exited(i32),
    Signal(i32),
    TimedOut(u64),
}

impl Verdict {
    fn of(status: ExitStatus) -> Verdict {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if let Some(signal) = status.signal() {
                return Verdict::Signal(signal);
            }
        }
        match status.code() {
            Some(0) => Verdict::Passed,
            Some(FAILURE_EXIT_CODE) => Verdict::Failed,
            Some(code) => Verdict::Exited(code),
            None => Verdict::Signal(0),
        }
    }

    /// The result word of a test line: `ok`, `FAILED` or `FAILED (<reason>)`.
    pub fn word(&self) -> String {
        match self {
            Verdict::Passed => "ok".to_string(),
            Verdict::Failed => "FAILED".to_string(),
            Verdict::Exited(code) => format!("FAILED (exited with code {code})"),
            Verdict::Signal(signal) => format!("FAILED (killed by signal {signal})"),
            Verdict::TimedOut(secs) => format!("FAILED (timed out after {secs} s)"),
        }
    }
}

/// One finished child.
struct Outcome {
    verdict: Verdict,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Run each selected test as a child, print the report, return the exit
/// status: 0 when every selected test passed, else 1.
fn run_tests(tests: &[Test<'_>], arguments: &Arguments) -> i32 {
    let selected: Vec<&str> = tests
        .iter()
        .map(|test| test.name)
        .filter(|name| arguments.selects(name))
        .collect();
    let filtered_out = tests.len() - selected.len();
    let mut out = std::io::stdout().lock();
    if arguments.list {
        for name in &selected {
            let _ = writeln!(out, "{name}: test");
        }
        let _ = writeln!(out, "{}", count(selected.len()));
        return 0;
    }
    let _ = writeln!(out, "running {}", count(selected.len()));
    let _ = out.flush();
    let started = Instant::now();
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => {
            crate::log(&format!("error: cannot find the test executable: {error}"));
            return 1;
        }
    };
    let mut outcomes: Vec<Option<Outcome>> = selected.iter().map(|_| None).collect();
    let next = AtomicUsize::new(0);
    let (sender, receiver) = mpsc::channel::<(usize, Outcome)>();
    std::thread::scope(|scope| {
        for _ in 0..arguments.jobs.min(selected.len()) {
            let sender = sender.clone();
            let (next, selected, exe) = (&next, &selected, &exe);
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    let Some(name) = selected.get(index) else {
                        break;
                    };
                    let scratch = Scratch::create(index);
                    let outcome = run_child(exe, name, arguments.timeout, &scratch.path);
                    drop(scratch);
                    if sender.send((index, outcome)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);
        let mut printed = 0;
        for (index, outcome) in receiver {
            outcomes[index] = Some(outcome);
            while let Some(Some(outcome)) = outcomes.get(printed) {
                let _ = writeln!(
                    out,
                    "test {} ... {}",
                    selected[printed],
                    outcome.verdict.word()
                );
                let _ = out.flush();
                printed += 1;
            }
        }
    });
    let failed: Vec<(&str, &Outcome)> = selected
        .iter()
        .zip(&outcomes)
        .filter_map(|(name, outcome)| outcome.as_ref().map(|o| (*name, o)))
        .filter(|(_, outcome)| outcome.verdict != Verdict::Passed)
        .collect();
    let passed = selected.len() - failed.len();
    let _ = writeln!(out);
    if !failed.is_empty() {
        let _ = writeln!(out, "failures:");
        let _ = writeln!(out);
        for (name, outcome) in &failed {
            let mut shown = false;
            for (stream, bytes) in [("stdout", &outcome.stdout), ("stderr", &outcome.stderr)] {
                if bytes.is_empty() {
                    continue;
                }
                shown = true;
                let _ = writeln!(out, "---- {name} {stream} ----");
                let _ = out.write_all(bytes);
                if !bytes.ends_with(b"\n") {
                    let _ = writeln!(out);
                }
            }
            if shown {
                let _ = writeln!(out);
            }
        }
        let _ = writeln!(out, "failures:");
        for (name, _) in &failed {
            let _ = writeln!(out, "    {name}");
        }
        let _ = writeln!(out);
    }
    let _ = writeln!(
        out,
        "test result: {}. {passed} passed; {} failed; {filtered_out} filtered out; finished in {:.2}s",
        if failed.is_empty() { "ok" } else { "FAILED" },
        failed.len(),
        started.elapsed().as_secs_f64()
    );
    let _ = out.flush();
    i32::from(!failed.is_empty())
}

/// `N tests`, `1 test`.
fn count(n: usize) -> String {
    if n == 1 {
        "1 test".to_string()
    } else {
        format!("{n} tests")
    }
}

/// The environment variable that names a test child's scratch directory
/// (`docs/slice.md` 21.8), where `Directory.temporary()` makes its roots.
pub const SCRATCH_VARIABLE: &str = "IGNITE_TEST_SCRATCH";

/// The scratch directory of one test child: created under
/// `std::env::temp_dir()` before the child starts, removed with its
/// contents when dropped, after the runner reaped the child, so a test
/// killed at its timeout leaves nothing.
struct Scratch {
    path: std::path::PathBuf,
}

impl Scratch {
    /// `<temp>/ignite-test-<runner pid>-<index>`, empty. A directory left
    /// by an earlier runner with the same pid is emptied first.
    fn create(index: usize) -> Scratch {
        let path = std::env::temp_dir().join(format!("ignite-test-{}-{index}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let _ = std::fs::create_dir_all(&path);
        Scratch { path }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Run `<exe> --run-one <name>` with its output captured and `scratch` as
/// its [`SCRATCH_VARIABLE`]; kill it after `timeout` seconds. The child is
/// watched by [`crate::child::watch`] on the calling pool thread and stays
/// in the runner's process group, so Ctrl-C at the terminal reaches it.
fn run_child(
    exe: &std::path::Path,
    name: &str,
    timeout: u64,
    scratch: &std::path::Path,
) -> Outcome {
    let mut command = Command::new(exe);
    command
        .arg("--run-one")
        .arg(name)
        .env(SCRATCH_VARIABLE, scratch);
    let watched = crate::child::Watched::new(false);
    let finished = crate::child::watch(
        command,
        &watched,
        Duration::from_secs(timeout),
        usize::MAX,
        Vec::new(),
        |_| {},
    );
    match finished {
        Ok(finished) => Outcome {
            verdict: match finished.status {
                _ if finished.timed_out => Verdict::TimedOut(timeout),
                Some(status) => Verdict::of(status),
                None => Verdict::Exited(-1),
            },
            stdout: finished.stdout,
            stderr: finished.stderr,
        },
        Err(error) => Outcome {
            verdict: Verdict::Exited(-1),
            stdout: Vec::new(),
            stderr: format!("cannot start {}: {error}\n", exe.display()).into_bytes(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Arguments, String> {
        Arguments::parse(&args.iter().map(|a| a.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn arguments() {
        let all = parse(&[]).unwrap();
        assert_eq!(all.filter, None);
        assert_eq!(all.timeout, DEFAULT_TIMEOUT);
        assert!(all.jobs >= 1);
        let some = parse(&["split", "--exact", "--jobs", "3", "--timeout", "1"]).unwrap();
        assert_eq!(some.filter.as_deref(), Some("split"));
        assert!(some.exact);
        assert_eq!((some.jobs, some.timeout), (3, 1));
        assert_eq!(
            parse(&["--run-one", "m/a b"]).unwrap().run_one.as_deref(),
            Some("m/a b")
        );
        assert!(parse(&["--jobs", "0"]).is_err());
        assert!(parse(&["--timeout"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
        assert!(parse(&["a", "b"]).is_err());
    }

    #[test]
    fn selection() {
        let mut arguments = parse(&["split"]).unwrap();
        assert!(arguments.selects("app.text/split_once splits"));
        assert!(!arguments.selects("app.text/trim"));
        arguments.exact = true;
        assert!(!arguments.selects("app.text/split_once splits"));
        assert!(arguments.selects("split"));
    }

    #[test]
    fn verdict_words() {
        assert_eq!(Verdict::Passed.word(), "ok");
        assert_eq!(Verdict::Failed.word(), "FAILED");
        assert_eq!(Verdict::Exited(3).word(), "FAILED (exited with code 3)");
        assert_eq!(Verdict::TimedOut(1).word(), "FAILED (timed out after 1 s)");
        assert_eq!(Verdict::Signal(9).word(), "FAILED (killed by signal 9)");
    }

    /// `docs/slice.md` 21.8: a test child's scratch directory exists and
    /// is empty before the child starts and is gone, with its contents,
    /// once the runner drops it after reaping the child.
    #[test]
    fn scratch_is_created_empty_and_removed_with_its_contents() {
        let scratch = super::Scratch::create(usize::MAX);
        let path = scratch.path.clone();
        assert!(path.starts_with(std::env::temp_dir()));
        assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
        std::fs::create_dir_all(path.join("a/b")).unwrap();
        std::fs::write(path.join("a/b/c"), b"x").unwrap();
        drop(scratch);
        assert!(!path.exists());
    }
}
