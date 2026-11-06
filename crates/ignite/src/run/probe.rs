//! The probe adapter (`docs/slice.md` 16.8): one probe process per
//! attempt (O28), started through the spawner thread (16.2) with
//! `--ref <path>=<address>:<port>` for the probe's `target`, its single
//! stdout line handed to the driver as `AttemptDone` (the driver decodes
//! it by the element type of the scheduler's signal, the model's
//! `observations`, `docs/slice.md` 22.3); a missing executable, an exit status
//! other than 0 or no line is `AttemptFailed` with the reason. At the
//! deadline the engine's `Kill` becomes [`kill_attempt`]: SIGKILL to the
//! probe, after which the attempt sends nothing (the reducer already
//! emitted the timeout, 16.8).

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use nix::sys::signal::Signal;
use nix::unistd::Pid;

use super::control::{Adapters, InFlight, Observation, relay};

/// One running attempt.
#[derive(Debug)]
struct Attempt {
    pid: i32,
    /// Killed at its deadline: no observation follows.
    cancelled: bool,
    _in_flight: InFlight,
}

/// The running attempts by `(member, id)`.
#[derive(Debug, Default)]
pub(super) struct Table {
    attempts: HashMap<(String, i64), Attempt>,
}

fn lock(table: &Mutex<Table>) -> MutexGuard<'_, Table> {
    table.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Start one attempt ([`Adapters::attempt`]).
pub(super) fn attempt(
    adapters: &Adapters,
    member: &str,
    id: i64,
    executable: &Path,
    refs: &[(String, String)],
) {
    let in_flight = adapters.counter.enter();
    let mut command = Command::new(executable);
    for (path, endpoint) in refs {
        command.arg("--ref").arg(format!("{path}={endpoint}"));
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = match adapters.spawner.spawn(command) {
        Ok(child) => child,
        Err(error) => {
            drop(in_flight);
            let _ = adapters.observations.send(Observation::AttemptFailed {
                member: member.to_string(),
                id,
                reason: format!("cannot start {}: {error}", executable.display()),
            });
            return;
        }
    };
    let key = (member.to_string(), id);
    lock(&adapters.attempts).attempts.insert(
        key.clone(),
        Attempt {
            pid: child.id() as i32,
            cancelled: false,
            _in_flight: in_flight,
        },
    );
    let observations = adapters.observations.clone();
    let attempts = Arc::clone(&adapters.attempts);
    let _ = std::thread::Builder::new()
        .name(format!("{member} attempt {id}"))
        .spawn(move || {
            let output = child.wait_with_output();
            let finished = lock(&attempts).attempts.remove(&key);
            let (member, id) = key;
            let output = match output {
                Ok(output) => output,
                Err(error) => {
                    let _ = observations.send(Observation::AttemptFailed {
                        member,
                        id,
                        reason: format!("wait failed: {error}"),
                    });
                    return;
                }
            };
            let stderr = String::from_utf8_lossy(&output.stderr);
            for line in stderr.lines() {
                relay(&member, line);
            }
            if finished.is_none_or(|attempt| attempt.cancelled) {
                return;
            }
            let observation = match output.status.code() {
                Some(0) => match String::from_utf8_lossy(&output.stdout).lines().next() {
                    Some(line) if !line.trim().is_empty() => Observation::AttemptDone {
                        member,
                        id,
                        line: line.trim_end_matches('\r').to_string(),
                    },
                    _ => Observation::AttemptFailed {
                        member,
                        id,
                        reason: "no output".to_string(),
                    },
                },
                Some(code) => Observation::AttemptFailed {
                    member,
                    id,
                    reason: match stderr.lines().next() {
                        Some(line) => format!("exit status {code}: {line}"),
                        None => format!("exit status {code}"),
                    },
                },
                None => Observation::AttemptFailed {
                    member,
                    id,
                    reason: match std::os::unix::process::ExitStatusExt::signal(&output.status) {
                        Some(signal) => format!("signal {signal}"),
                        None => "ended without a status".to_string(),
                    },
                },
            };
            let _ = observations.send(observation);
        });
}

/// SIGKILL to the probe of attempt `id` ([`Adapters::kill_attempt`]).
pub(super) fn kill_attempt(adapters: &Adapters, member: &str, id: i64) {
    let mut table = lock(&adapters.attempts);
    if let Some(attempt) = table.attempts.get_mut(&(member.to_string(), id)) {
        attempt.cancelled = true;
        let _ = nix::sys::signal::kill(Pid::from_raw(attempt.pid), Signal::SIGKILL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{Receiver, channel};
    use std::time::Duration;

    const LIMIT: Duration = Duration::from_secs(5);

    fn adapters() -> (Adapters, Receiver<Observation>) {
        let (sender, receiver) = channel();
        (Adapters::start(sender).unwrap(), receiver)
    }

    /// A probe executable: a shell script in the temp dir that runs `body`
    /// with `--ref T.server=127.0.0.1:4242` as its arguments.
    fn script(name: &str, body: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join("ignite-probe-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}-{}.sh", std::process::id()));
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn script_attempt(adapters: &Adapters, id: i64, body: &str) {
        let path = script(&format!("attempt-{id}"), body);
        adapters.attempt(
            "T.probe",
            id,
            &path,
            &[("T.server".to_string(), "127.0.0.1:4242".to_string())],
        );
    }

    #[test]
    fn refs_become_ref_arguments() {
        let (adapters, observations) = adapters();
        adapters.attempt(
            "T.probe",
            1,
            Path::new("/bin/echo"),
            &[("T.server".to_string(), "127.0.0.1:4242".to_string())],
        );
        assert_eq!(
            observations.recv_timeout(LIMIT),
            Ok(Observation::AttemptDone {
                member: "T.probe".to_string(),
                id: 1,
                line: "--ref T.server=127.0.0.1:4242".to_string(),
            })
        );
        assert!(adapters.settle(LIMIT));
    }

    #[test]
    fn the_first_line_is_the_result() {
        let (adapters, observations) = adapters();
        script_attempt(
            &adapters,
            2,
            "[ \"$1\" = --ref ] || exit 9; echo '{\"received\":true}'; echo second; echo warn >&2",
        );
        assert_eq!(
            observations.recv_timeout(LIMIT),
            Ok(Observation::AttemptDone {
                member: "T.probe".to_string(),
                id: 2,
                line: "{\"received\":true}".to_string(),
            })
        );
    }

    #[test]
    fn failures_are_attempt_failed() {
        let (adapters, observations) = adapters();
        script_attempt(&adapters, 3, "echo boom >&2; exit 3");
        match observations.recv_timeout(LIMIT) {
            Ok(Observation::AttemptFailed { id: 3, reason, .. }) => {
                assert_eq!(reason, "exit status 3: boom");
            }
            other => panic!("{other:?}"),
        }
        script_attempt(&adapters, 4, "exit 0");
        match observations.recv_timeout(LIMIT) {
            Ok(Observation::AttemptFailed { id: 4, reason, .. }) => {
                assert_eq!(reason, "no output");
            }
            other => panic!("{other:?}"),
        }
        adapters.attempt("T.probe", 5, Path::new("/nonexistent/promptProbe"), &[]);
        match observations.recv_timeout(LIMIT) {
            Ok(Observation::AttemptFailed { id: 5, reason, .. }) => {
                assert!(reason.starts_with("cannot start "), "{reason}");
            }
            other => panic!("{other:?}"),
        }
        assert!(adapters.settle(LIMIT));
    }

    #[test]
    fn a_killed_attempt_sends_nothing() {
        let (adapters, observations) = adapters();
        script_attempt(&adapters, 6, "exec sleep 30");
        assert!(
            !adapters.settle(Duration::from_millis(50)),
            "a running probe is in flight"
        );
        adapters.kill_attempt("T.probe", 6);
        assert!(adapters.settle(LIMIT), "the kill ends the flight");
        assert_eq!(
            observations.recv_timeout(Duration::from_millis(200)).ok(),
            None
        );
        assert!(lock(&adapters.attempts).attempts.is_empty());
    }
}
