//! `Stdin.is_terminal` and `Stdin.set_raw` on a pseudo-terminal.
//!
//! The parent opens a pseudo-terminal and runs this test binary again with
//! the terminal side as standard input and `TELOS_IO_CHILD` naming a
//! scenario. The child switches modes and checks the attributes it sees;
//! the parent checks the attributes the terminal is left with after the
//! child's `run` has returned, failed or panicked.

use std::io::Read;
use std::os::fd::{AsFd, OwnedFd};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use nix::pty::openpty;
use nix::sys::termios::{
    InputFlags, LocalFlags, OutputFlags, SpecialCharacterIndices, Termios, tcgetattr,
};
use telos_io::{IoError, Stdin};

const CHILD: &str = "TELOS_IO_CHILD";

fn scenario() -> Option<String> {
    std::env::var(CHILD).ok()
}

fn stdin_attributes() -> Termios {
    tcgetattr(std::io::stdin()).unwrap()
}

fn is_cooked(attributes: &Termios) -> bool {
    attributes
        .local_flags
        .contains(LocalFlags::ICANON | LocalFlags::ECHO | LocalFlags::ISIG)
}

fn assert_raw(attributes: &Termios) {
    let local = attributes.local_flags;
    let input = attributes.input_flags;
    assert!(!local.contains(LocalFlags::ICANON), "ICANON on");
    assert!(!local.contains(LocalFlags::ECHO), "ECHO on");
    assert!(!local.contains(LocalFlags::ISIG), "ISIG on");
    assert!(!input.contains(InputFlags::IXON), "IXON on");
    assert!(!input.contains(InputFlags::ICRNL), "ICRNL on");
    assert!(
        attributes.output_flags.contains(OutputFlags::OPOST),
        "OPOST off"
    );
    assert_eq!(
        attributes.control_chars[SpecialCharacterIndices::VMIN as usize],
        1
    );
}

/// Run `test` in a child with the terminal side of a new pseudo-terminal
/// as standard input. Returns whether it succeeded, its output, and the
/// terminal's attributes after it exited.
fn run_on_terminal(test: &str) -> (Option<i32>, String, Termios) {
    let pty = openpty(None, None).unwrap();
    assert!(is_cooked(&tcgetattr(pty.slave.as_fd()).unwrap()));
    let terminal: OwnedFd = pty.slave.try_clone().unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args([test, "--exact", "--test-threads=1", "--nocapture"])
        .env(CHILD, test)
        .stdin(Stdio::from(terminal))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let (status, output) = wait(child, Duration::from_secs(20));
    let after = tcgetattr(pty.slave.as_fd()).unwrap();
    drop(pty.master);
    (status, output, after)
}

/// Wait up to `limit`; kill the child and panic when it is still running.
fn wait(mut child: Child, limit: Duration) -> (Option<i32>, String) {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let mut output = String::new();
            child
                .stdout
                .take()
                .unwrap()
                .read_to_string(&mut output)
                .unwrap();
            child
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut output)
                .unwrap();
            return (status.code(), output);
        }
        if started.elapsed() > limit {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("child still running after {limit:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn child_raw_round_trip() {
    if scenario().as_deref() != Some("child_raw_round_trip") {
        return;
    }
    let status = telos_io::run(|| {
        assert!(Stdin::is_terminal()?);
        Stdin::set_raw(true)?;
        assert_raw(&stdin_attributes());
        Stdin::set_raw(true)?;
        assert_raw(&stdin_attributes());
        Stdin::set_raw(false)?;
        assert!(is_cooked(&stdin_attributes()));
        Stdin::set_raw(true)?;
        assert_raw(&stdin_attributes());
        Ok(())
    });
    assert_eq!(status, 0);
    assert!(is_cooked(&stdin_attributes()));
}

#[test]
fn set_raw_switches_and_run_restores_on_return() {
    if scenario().is_some() {
        return;
    }
    let (status, output, after) = run_on_terminal("child_raw_round_trip");
    assert_eq!(status, Some(0), "{output}");
    assert!(is_cooked(&after), "terminal left raw: {after:?}");
}

#[test]
fn child_raw_then_error() {
    if scenario().as_deref() != Some("child_raw_then_error") {
        return;
    }
    let status = telos_io::run(|| {
        Stdin::set_raw(true)?;
        Err(IoError::new("role failed").with_exit_code(4))
    });
    assert_eq!(status, 4);
    assert!(is_cooked(&stdin_attributes()));
}

#[test]
fn run_restores_the_terminal_when_main_fails() {
    if scenario().is_some() {
        return;
    }
    let (status, output, after) = run_on_terminal("child_raw_then_error");
    assert_eq!(status, Some(0), "{output}");
    assert!(output.contains("error: role failed"), "{output}");
    assert!(is_cooked(&after), "terminal left raw: {after:?}");
}

#[test]
fn child_raw_then_panic() {
    if scenario().as_deref() != Some("child_raw_then_panic") {
        return;
    }
    let outcome = std::panic::catch_unwind(|| {
        telos_io::run(|| {
            Stdin::set_raw(true)?;
            assert_raw(&stdin_attributes());
            panic!("main panicked");
        })
    });
    assert!(outcome.is_err());
    assert!(is_cooked(&stdin_attributes()));
}

#[test]
fn run_restores_the_terminal_when_main_panics() {
    if scenario().is_some() {
        return;
    }
    let (status, output, after) = run_on_terminal("child_raw_then_panic");
    assert_eq!(status, Some(0), "{output}");
    assert!(output.contains("main panicked"), "{output}");
    assert!(is_cooked(&after), "terminal left raw: {after:?}");
}

#[test]
fn child_raw_then_process_exit() {
    if scenario().as_deref() != Some("child_raw_then_process_exit") {
        return;
    }
    telos_io::run(|| {
        Stdin::set_raw(true)?;
        assert_raw(&stdin_attributes());
        telos_io::spawn(|| {
            let _ = Stdin::read(16);
        });
        telos_io::Process::exit(7)?;
        unreachable!("Process.exit returned");
    });
    unreachable!("run returned after Process.exit");
}

#[test]
fn process_exit_restores_the_terminal_and_exits_with_the_code() {
    if scenario().is_some() {
        return;
    }
    let (status, output, after) = run_on_terminal("child_raw_then_process_exit");
    assert_eq!(status, Some(7), "{output}");
    assert!(is_cooked(&after), "terminal left raw: {after:?}");
}

#[test]
fn child_not_a_terminal() {
    if scenario().as_deref() != Some("child_not_a_terminal") {
        return;
    }
    let status = telos_io::run(|| {
        assert!(!Stdin::is_terminal()?);
        Stdin::set_raw(true)?;
        Stdin::set_raw(false)
    });
    assert_eq!(status, 0);
}

#[test]
fn set_raw_is_a_no_op_on_a_pipe() {
    if scenario().is_some() {
        return;
    }
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "child_not_a_terminal",
            "--exact",
            "--test-threads=1",
            "--nocapture",
        ])
        .env(CHILD, "child_not_a_terminal")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let (status, output) = wait(child, Duration::from_secs(20));
    assert_eq!(status, Some(0), "{output}");
}
