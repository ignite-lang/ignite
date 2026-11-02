//! The process adapter (`docs/slice.md` 16.2, 16.7): one spawner thread
//! starts every child of the run, members and probes alike, because
//! `PR_SET_PDEATHSIG` ties the death signal to the thread that forked
//! (A3); it lives for the whole run and other threads send it requests.
//! Each child gets `PR_SET_PDEATHSIG(SIGKILL)` in the child after the fork,
//! so a crash of `ignite run` leaves no orphan, and stays in the run's
//! process group. Per member child, a waiter thread reports `Exited` or
//! `Signalled`, and a reader thread turns `listening on <address>:<port>`
//! on the child's stderr into `Ready` (O37, O44) and `released
//! <address>:<port>` into `Released` (16.12), relaying every other line as
//! `<member>: <line>`.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use nix::errno::Errno;
use nix::sys::signal::Signal;
use nix::unistd::Pid;

use super::control::{Adapters, InFlight, Observation, relay};

/// A spawn request to the spawner thread and where its result goes.
struct Request {
    command: Command,
    reply: Sender<io::Result<Child>>,
}

/// The spawner thread (A3): the parent of every child the run starts.
#[derive(Debug)]
pub(super) struct Spawner {
    requests: Sender<Request>,
}

impl Spawner {
    /// Start the thread.
    pub(super) fn start() -> io::Result<Spawner> {
        let (requests, incoming) = channel::<Request>();
        std::thread::Builder::new()
            .name("spawner".to_string())
            .spawn(move || {
                for Request { mut command, reply } in incoming {
                    let mut result = command.spawn();
                    // A concurrent fork can briefly retain a writable file
                    // descriptor after its owner closed it. Retry only that
                    // exec conflict, on this same parent thread (A3), with
                    // 31 ms total backoff before the final spawn error.
                    for delay in [1, 2, 4, 8, 16] {
                        if !matches!(
                            &result,
                            Err(error) if error.raw_os_error() == Some(Errno::ETXTBSY as i32)
                        ) {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(delay));
                        result = command.spawn();
                    }
                    let _ = reply.send(result);
                }
            })?;
        Ok(Spawner { requests })
    }

    /// Spawn `command` from the spawner thread with
    /// `PR_SET_PDEATHSIG(SIGKILL)` set in the child.
    pub(super) fn spawn(&self, mut command: Command) -> io::Result<Child> {
        // SAFETY: `prctl` is async-signal-safe and the closure touches
        // nothing else.
        unsafe {
            command.pre_exec(|| {
                nix::sys::prctl::set_pdeathsig(Signal::SIGKILL).map_err(io::Error::from)
            })
        };
        let (reply, result) = channel();
        self.requests
            .send(Request { command, reply })
            .map_err(|_| io::Error::other("the spawner thread is gone"))?;
        result
            .recv()
            .map_err(|_| io::Error::other("the spawner thread is gone"))?
    }
}

/// Whether the `released` line of a live child was awaited or seen.
#[derive(Debug, Default)]
enum Release {
    #[default]
    Unseen,
    Seen,
    /// Awaited by the engine: the guard keeps the run in flight until the
    /// line arrives or the child ends.
    Awaited {
        _in_flight: InFlight,
    },
}

/// One live child of a member.
#[derive(Debug)]
struct Live {
    member: String,
    generation: i64,
    /// In flight until `Ready`, when a listener is expected.
    ready: Option<InFlight>,
    /// In flight from a `kill` until the child ends, or until it prints
    /// `released`: a drained child (SIGTERM, 16.13) ends when its sessions
    /// end, which the deadline timer bounds, not the settle rule.
    killing: Option<InFlight>,
    release: Release,
}

/// The live member children by pid.
#[derive(Debug, Default)]
pub(super) struct Table {
    live: BTreeMap<i32, Live>,
}

fn lock(table: &Mutex<Table>) -> MutexGuard<'_, Table> {
    table.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Table {
    /// The member's live children, oldest generation first.
    fn of_member(&mut self, member: &str) -> Vec<(i64, i32)> {
        let mut found: Vec<(i64, i32)> = self
            .live
            .iter()
            .filter(|(_, live)| live.member == member)
            .map(|(pid, live)| (live.generation, *pid))
            .collect();
        found.sort();
        found
    }
}

/// `address` and `port` of a `listening on <address>:<port>` line.
fn listening(line: &str) -> Option<(String, u16)> {
    let rest = line.strip_prefix("listening on ")?;
    let (address, port) = rest.trim().rsplit_once(':')?;
    Some((address.to_string(), port.parse().ok()?))
}

/// Whether `line` is `released <address>:<port>`.
fn released(line: &str) -> bool {
    line.strip_prefix("released ")
        .and_then(|rest| rest.trim().rsplit_once(':'))
        .is_some_and(|(address, port)| !address.is_empty() && port.parse::<u16>().is_ok())
}

/// Start `executable args` for `member` ([`Adapters::spawn`]).
pub(super) fn spawn(
    adapters: &Adapters,
    member: &str,
    generation: i64,
    executable: &Path,
    args: &[String],
    expect_ready: bool,
) {
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match adapters.spawner.spawn(command) {
        Ok(child) => child,
        Err(error) => {
            let _ = adapters.observations.send(Observation::SpawnFailed {
                member: member.to_string(),
                reason: format!("cannot start {}: {error}", executable.display()),
            });
            return;
        }
    };
    let pid = child.id() as i32;
    lock(&adapters.processes).live.insert(
        pid,
        Live {
            member: member.to_string(),
            generation,
            ready: expect_ready.then(|| adapters.counter.enter()),
            killing: None,
            release: Release::Unseen,
        },
    );
    let _ = adapters.observations.send(Observation::Started {
        member: member.to_string(),
        generation,
        pid,
    });
    if let Some(stdout) = child.stdout.take() {
        let name = member.to_string();
        let _ = std::thread::Builder::new()
            .name(format!("{member} stdout"))
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    relay(&name, &line);
                }
            });
    }
    if let Some(stderr) = child.stderr.take() {
        let name = member.to_string();
        let observations = adapters.observations.clone();
        let processes = Arc::clone(&adapters.processes);
        let _ = std::thread::Builder::new()
            .name(format!("{member} stderr"))
            .spawn(move || {
                for line in BufReader::new(stderr).lines() {
                    let Ok(line) = line else { break };
                    if let Some((address, port)) = listening(&line) {
                        if let Some(live) = lock(&processes).live.get_mut(&pid) {
                            live.ready = None;
                        }
                        let _ = observations.send(Observation::Ready {
                            member: name.clone(),
                            pid,
                            address,
                            port,
                        });
                    } else if released(&line) {
                        let mut table = lock(&processes);
                        let Some(live) = table.live.get_mut(&pid) else {
                            continue;
                        };
                        let awaited = matches!(live.release, Release::Awaited { .. });
                        live.release = Release::Seen;
                        live.killing = None;
                        drop(table);
                        if awaited {
                            let _ = observations.send(Observation::Released {
                                member: name.clone(),
                                pid,
                            });
                        }
                    } else {
                        relay(&name, &line);
                    }
                }
            });
    }
    let name = member.to_string();
    let observations = adapters.observations.clone();
    let processes = Arc::clone(&adapters.processes);
    let _ = std::thread::Builder::new()
        .name(format!("{member} waiter"))
        .spawn(move || {
            let status = child.wait();
            let observation = match status {
                Ok(status) => match (status.code(), status.signal()) {
                    (Some(code), _) => Observation::Exited {
                        member: name,
                        pid,
                        code,
                    },
                    (None, Some(signal)) => Observation::Signalled {
                        member: name,
                        pid,
                        signal,
                    },
                    (None, None) => Observation::Exited {
                        member: name,
                        pid,
                        code: -1,
                    },
                },
                Err(error) => {
                    relay(&name, &format!("wait failed: {error}"));
                    Observation::Exited {
                        member: name,
                        pid,
                        code: -1,
                    }
                }
            };
            // Publish completion before making quiescence visible. The stop
            // loop checks this table, then drains the observations already
            // queued; neither it nor settle may see the child retired first.
            let mut table = lock(&processes);
            let _ = observations.send(observation);
            drop(table.live.remove(&pid));
        });
}

/// `signal` to the live child `pid` ([`Adapters::kill`]).
pub(super) fn kill(adapters: &Adapters, pid: i32, signal: i32) -> io::Result<()> {
    let signal = Signal::try_from(signal)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, format!("signal {signal}")))?;
    let mut table = lock(&adapters.processes);
    let live = table.live.get_mut(&pid).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("no live child with pid {pid}"),
        )
    })?;
    nix::sys::signal::kill(Pid::from_raw(pid), signal)?;
    let drained = signal == Signal::SIGTERM && matches!(live.release, Release::Seen);
    if live.killing.is_none() && !drained {
        live.killing = Some(adapters.counter.enter());
    }
    Ok(())
}

/// `Released` once the member's oldest live child printed `released`
/// ([`Adapters::await_release`]).
pub(super) fn await_release(adapters: &Adapters, member: &str) -> bool {
    let mut table = lock(&adapters.processes);
    let Some((_, pid)) = table.of_member(member).into_iter().next() else {
        return false;
    };
    let Some(live) = table.live.get_mut(&pid) else {
        return false;
    };
    match live.release {
        Release::Seen => {
            drop(table);
            let _ = adapters.observations.send(Observation::Released {
                member: member.to_string(),
                pid,
            });
        }
        Release::Unseen => {
            live.release = Release::Awaited {
                _in_flight: adapters.counter.enter(),
            }
        }
        Release::Awaited { .. } => {}
    }
    true
}

/// `(generation, pid)` of the member's live children, oldest first.
pub(super) fn pids(adapters: &Adapters, member: &str) -> Vec<(i64, i32)> {
    lock(&adapters.processes).of_member(member)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::fd::OwnedFd;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc::Receiver;
    use std::time::Duration;

    const LIMIT: Duration = Duration::from_secs(5);

    fn adapters() -> (Adapters, Receiver<Observation>) {
        let (sender, receiver) = channel();
        (Adapters::start(sender).unwrap(), receiver)
    }

    fn sh(script: &str) -> Vec<String> {
        vec!["-c".to_string(), script.to_string()]
    }

    #[test]
    fn a_busy_executable_is_retried_after_its_writer_closes() {
        let path =
            std::env::temp_dir().join(format!("ignite-spawn-busy-{}.sh", std::process::id()));
        let mut writer = std::fs::File::create(&path).unwrap();
        writer.write_all(b"#!/bin/sh\nexit 7\n").unwrap();
        writer
            .set_permissions(std::fs::Permissions::from_mode(0o700))
            .unwrap();
        // The parent drops its writable handle after spawn. The holder's
        // stdout keeps the executable busy until its stdin reaches EOF.
        let (holder_reader, holder_input) = UnixStream::pair().unwrap();
        let mut holder = Command::new("/bin/cat")
            .stdin(Stdio::from(OwnedFd::from(holder_reader)))
            .stdout(Stdio::from(writer))
            .spawn()
            .unwrap();
        assert_eq!(
            Command::new(&path).spawn().unwrap_err().raw_os_error(),
            Some(Errno::ETXTBSY as i32)
        );
        let (notify_child, mut notify_parent) = UnixStream::pair().unwrap();
        let (mut acknowledge_parent, acknowledge_child) = UnixStream::pair().unwrap();
        notify_parent.set_read_timeout(Some(LIMIT)).unwrap();
        acknowledge_child.set_read_timeout(Some(LIMIT)).unwrap();
        let controller = std::thread::spawn(move || -> io::Result<()> {
            let mut byte = [0];
            let attempts = (|| {
                notify_parent.read_exact(&mut byte)?;
                // First exec happens while the holder still owns the writer.
                acknowledge_parent.write_all(&[1])?;
                notify_parent.read_exact(&mut byte)
            })();
            // Shutdown reaches every inherited duplicate of this socket;
            // no child needs to close a borrowed numeric file descriptor.
            let stopped = holder_input.shutdown(std::net::Shutdown::Write);
            let released = holder.wait();
            attempts?;
            stopped?;
            assert!(released?.success());
            // Second exec starts only after the holder released its writer.
            acknowledge_parent.write_all(&[1])
        });
        let mut command = Command::new(&path);
        // SAFETY: the child performs only async-signal-safe socket read/write
        // syscalls; the streams are owned and initialized before the fork.
        unsafe {
            command.pre_exec(move || {
                nix::unistd::write(&notify_child, &[1]).map_err(io::Error::from)?;
                let mut byte = [0];
                match nix::unistd::read(&acknowledge_child, &mut byte).map_err(io::Error::from)? {
                    1 => Ok(()),
                    _ => Err(io::ErrorKind::UnexpectedEof.into()),
                }
            });
        }
        let spawner = Spawner::start().unwrap();
        let result = spawner.spawn(command);
        let controlled = controller.join().unwrap();
        let status = result.and_then(|mut child| child.wait());
        std::fs::remove_file(path).unwrap();
        let status = status.expect("retry the transient executable write conflict");
        controlled.unwrap();
        assert_eq!(status.code(), Some(7));
    }

    #[test]
    fn lines_are_recognised() {
        assert_eq!(
            listening("listening on 127.0.0.1:2323"),
            Some(("127.0.0.1".to_string(), 2323))
        );
        assert_eq!(
            listening("listening on ::1:80"),
            Some(("::1".to_string(), 80))
        );
        assert_eq!(listening("listening on nowhere"), None);
        assert_eq!(listening("task ended with error: x"), None);
        assert!(released("released 127.0.0.1:2323"));
        assert!(!released("released"));
        assert!(!released("release 127.0.0.1:1"));
    }

    #[test]
    fn a_child_is_started_ready_and_signalled() {
        let (adapters, observations) = adapters();
        adapters.spawn(
            "T.server",
            1,
            Path::new("/bin/sh"),
            &sh("echo 'listening on 127.0.0.1:4242' >&2; echo noise >&2; exec sleep 30"),
            true,
        );
        let Ok(Observation::Started {
            member,
            generation,
            pid,
        }) = observations.recv_timeout(LIMIT)
        else {
            panic!("no Started");
        };
        assert_eq!((member.as_str(), generation), ("T.server", 1));
        assert_eq!(adapters.pids("T.server"), vec![(1, pid)]);
        assert_eq!(
            observations.recv_timeout(LIMIT),
            Ok(Observation::Ready {
                member: "T.server".to_string(),
                pid,
                address: "127.0.0.1".to_string(),
                port: 4242,
            })
        );
        assert!(adapters.settle(LIMIT), "ready ends the spawn's flight");
        adapters.kill(pid, 15).unwrap();
        assert_eq!(
            observations.recv_timeout(LIMIT),
            Ok(Observation::Signalled {
                member: "T.server".to_string(),
                pid,
                signal: 15,
            })
        );
        assert!(adapters.settle(LIMIT), "the exit ends the kill's flight");
        assert!(adapters.pids("T.server").is_empty());
        assert!(adapters.kill(pid, 15).is_err(), "a reaped pid is not live");
    }

    #[test]
    fn an_exit_before_ready_ends_the_flight() {
        let (adapters, observations) = adapters();
        adapters.spawn("T.quick", 2, Path::new("/bin/sh"), &sh("exit 3"), true);
        let Ok(Observation::Started { pid, .. }) = observations.recv_timeout(LIMIT) else {
            panic!("no Started");
        };
        assert!(adapters.settle(LIMIT), "the child has exited before ready");
        assert!(adapters.pids("T.quick").is_empty());
        assert_eq!(
            observations.try_recv(),
            Ok(Observation::Exited {
                member: "T.quick".to_string(),
                pid,
                code: 3,
            }),
            "quiescence must not precede publication of the exit"
        );
    }

    #[test]
    fn a_missing_executable_fails_to_spawn() {
        let (adapters, observations) = adapters();
        adapters.spawn("T.none", 1, Path::new("/nonexistent/terminald"), &[], true);
        match observations.recv_timeout(LIMIT) {
            Ok(Observation::SpawnFailed { member, reason }) => {
                assert_eq!(member, "T.none");
                assert!(reason.contains("/nonexistent/terminald"), "{reason}");
            }
            other => panic!("{other:?}"),
        }
        assert!(adapters.settle(LIMIT));
    }

    #[test]
    fn released_is_reported_once_awaited() {
        let (adapters, observations) = adapters();
        adapters.spawn(
            "T.old",
            1,
            Path::new("/bin/sh"),
            &sh("echo 'released 127.0.0.1:4242' >&2; exec sleep 30"),
            false,
        );
        let Ok(Observation::Started { pid, .. }) = observations.recv_timeout(LIMIT) else {
            panic!("no Started");
        };
        std::thread::sleep(Duration::from_millis(100));
        assert!(observations.try_recv().is_err(), "released before awaited");
        assert!(adapters.await_release("T.old"));
        assert_eq!(
            observations.recv_timeout(LIMIT),
            Ok(Observation::Released {
                member: "T.old".to_string(),
                pid,
            })
        );
        adapters.kill(pid, 9).unwrap();
        assert!(matches!(
            observations.recv_timeout(LIMIT),
            Ok(Observation::Signalled { signal: 9, .. })
        ));
        assert!(!adapters.await_release("T.old"));
    }

    #[test]
    fn two_generations_are_told_apart_by_pid() {
        let (adapters, observations) = adapters();
        adapters.spawn("T.s", 1, Path::new("/bin/sh"), &sh("exec sleep 30"), false);
        adapters.spawn("T.s", 2, Path::new("/bin/sh"), &sh("exec sleep 30"), false);
        let mut pids = Vec::new();
        for _ in 0..2 {
            let Ok(Observation::Started {
                generation, pid, ..
            }) = observations.recv_timeout(LIMIT)
            else {
                panic!("no Started");
            };
            pids.push((generation, pid));
        }
        assert_eq!(adapters.pids("T.s"), pids);
        adapters.kill(pids[1].1, 9).unwrap();
        assert_eq!(
            observations.recv_timeout(LIMIT),
            Ok(Observation::Signalled {
                member: "T.s".to_string(),
                pid: pids[1].1,
                signal: 9,
            })
        );
        assert_eq!(adapters.pids("T.s"), vec![pids[0]]);
        adapters.kill(pids[0].1, 9).unwrap();
        assert_eq!(
            observations.recv_timeout(LIMIT),
            Ok(Observation::Signalled {
                member: "T.s".to_string(),
                pid: pids[0].1,
                signal: 9,
            })
        );
        assert!(adapters.kill(pids[0].1, 9).is_err());
    }
}
