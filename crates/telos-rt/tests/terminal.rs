//! Terminal mode on a pseudo-terminal, and the ways out of the process that
//! restore it. Each case re-runs this test binary as a child whose standard
//! input is the pty's slave side; the `child` test does the work when
//! `TELOS_RT_CHILD` names a mode, and the parent reads the slave's
//! attributes after the child ended.

use std::os::fd::{AsFd, OwnedFd};
use std::process::{Command, Output, Stdio};

use nix::pty::openpty;
use nix::sys::termios::{
    InputFlags, LocalFlags, OutputFlags, SpecialCharacterIndices, Termios, tcgetattr,
};
use telos_rt::*;

const MODE: &str = "TELOS_RT_CHILD";

fn flags(termios: &Termios) -> String {
    let local = termios.local_flags;
    let input = termios.input_flags;
    format!(
        "icanon={} echo={} isig={} ixon={} icrnl={} opost={} vmin={} vtime={}",
        u8::from(local.contains(LocalFlags::ICANON)),
        u8::from(local.contains(LocalFlags::ECHO)),
        u8::from(local.contains(LocalFlags::ISIG)),
        u8::from(input.contains(InputFlags::IXON)),
        u8::from(input.contains(InputFlags::ICRNL)),
        u8::from(termios.output_flags.contains(OutputFlags::OPOST)),
        termios.control_chars[SpecialCharacterIndices::VMIN as usize],
        termios.control_chars[SpecialCharacterIndices::VTIME as usize],
    )
}

fn report(line: &str) {
    let data = TelosList::from_bytes(format!("\nchild: {line}\n").as_bytes());
    let mut err = TelosIoError {
        message: data,
        exit_code: 0,
    };
    assert_eq!(telos_rt_stdout_write_all(&data, &mut err), 0);
}

fn set_raw(on: bool) {
    let mut err = TelosIoError {
        message: TelosList::from_bytes(b""),
        exit_code: 0,
    };
    assert_eq!(telos_rt_stdin_set_raw(i8::from(on), &mut err), 0);
}

fn current() -> String {
    flags(&tcgetattr(std::io::stdin()).unwrap())
}

#[test]
fn child() {
    let Ok(mode) = std::env::var(MODE) else {
        return;
    };
    telos_rt_init(0, std::ptr::null());
    if mode == "cat" {
        let mut err = TelosIoError {
            message: TelosList::from_bytes(b""),
            exit_code: 0,
        };
        loop {
            let mut input = TelosInput {
                tag: -1,
                data: TelosList::from_bytes(b""),
            };
            assert_eq!(telos_rt_stdin_read(3, &mut input, &mut err), 0);
            if input.tag == 1 {
                return;
            }
            assert!(input.data.len <= 3);
            let mut framed = TelosList::from_bytes(b"[");
            telos_rt_text_concat(&raw mut framed, &raw const framed, &input.data);
            telos_rt_text_concat(
                &raw mut framed,
                &raw const framed,
                &TelosList::from_bytes(b"]"),
            );
            assert_eq!(telos_rt_stdout_write_all(&framed, &mut err), 0);
        }
    }
    let (mut terminal, mut err) = (
        -1i8,
        TelosIoError {
            message: TelosList::from_bytes(b""),
            exit_code: 0,
        },
    );
    assert_eq!(telos_rt_stdin_is_terminal(&mut terminal, &mut err), 0);
    report(&format!("terminal={terminal}"));
    set_raw(true);
    if terminal == 1 {
        report(&format!("raw {}", current()));
        set_raw(false);
        report(&format!("cooked {}", current()));
        set_raw(true);
        set_raw(true);
        report(&format!("raw {}", current()));
    }
    match mode.as_str() {
        "exit" => telos_rt_exit(5),
        "fail" => {
            let mut error = TelosIoError {
                message: TelosList::from_bytes(b""),
                exit_code: 0,
            };
            telos_rt_io_error_new(&mut error, &TelosList::from_bytes(b"terminal gone"));
            error.exit_code = 4;
            telos_rt_fail(&error)
        }
        "panic" => telos_rt_panic(&TelosList::from_bytes(b"index 3 out of range")),
        "rust-panic" => panic!("runtime bug"),
        _ => {}
    }
}

struct Run {
    output: Output,
    after: String,
    before: String,
}

impl Run {
    fn stdout(&self) -> Vec<String> {
        String::from_utf8_lossy(&self.output.stdout)
            .lines()
            .filter_map(|line| line.strip_prefix("child: ").map(String::from))
            .collect()
    }

    fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.output.stderr).into_owned()
    }
}

fn child_command(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "child", "--nocapture", "--test-threads=1"])
        .env(MODE, mode)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn run_on_pty(mode: &str) -> Run {
    let pty = openpty(None, None).unwrap();
    let slave: OwnedFd = pty.slave;
    let before = flags(&tcgetattr(slave.as_fd()).unwrap());
    let output = child_command(mode)
        .stdin(Stdio::from(slave.try_clone().unwrap()))
        .output()
        .unwrap();
    let after = flags(&tcgetattr(slave.as_fd()).unwrap());
    drop(pty.master);
    Run {
        output,
        after,
        before,
    }
}

fn assert_raw_then_restored(run: &Run) {
    let lines = run.stdout();
    assert_eq!(lines[0], "terminal=1", "{lines:?} {}", run.stderr());
    let raw = "raw icanon=0 echo=0 isig=0 ixon=0 icrnl=0 opost=1 vmin=1 vtime=0";
    assert_eq!(lines[1], raw);
    assert_eq!(lines[2], format!("cooked {}", run.before));
    assert_eq!(lines[3], raw);
    assert!(run.before.starts_with("icanon=1 echo=1"), "{}", run.before);
    assert_eq!(run.after, run.before, "terminal restored");
}

#[test]
fn exit_restores_the_terminal() {
    let run = run_on_pty("exit");
    assert_eq!(run.output.status.code(), Some(5));
    assert_raw_then_restored(&run);
}

#[test]
fn fail_restores_the_terminal() {
    let run = run_on_pty("fail");
    assert_eq!(run.output.status.code(), Some(4));
    assert!(
        run.stderr().contains("error: terminal gone\n"),
        "{}",
        run.stderr()
    );
    assert_raw_then_restored(&run);
}

#[test]
fn panic_restores_the_terminal() {
    let run = run_on_pty("panic");
    assert_eq!(run.output.status.code(), Some(101));
    assert!(
        run.stderr().contains("panic: index 3 out of range\n"),
        "{}",
        run.stderr()
    );
    assert_raw_then_restored(&run);
}

#[test]
fn rust_panic_restores_the_terminal() {
    let run = run_on_pty("rust-panic");
    assert_eq!(run.output.status.code(), Some(101));
    assert!(
        run.stderr().contains("panic: runtime bug at "),
        "{}",
        run.stderr()
    );
    assert_raw_then_restored(&run);
}

#[test]
fn normal_exit_restores_the_terminal_at_exit() {
    let run = run_on_pty("return");
    assert_eq!(run.output.status.code(), Some(0), "{}", run.stderr());
    assert_raw_then_restored(&run);
}

#[test]
fn non_terminal_stdin_is_a_no_op() {
    let output = child_command("return")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout.contains("child: terminal=0\n"), "{stdout}");
    assert!(!stdout.contains("child: raw"), "{stdout}");
}

#[test]
fn stdin_reads_until_eof() {
    let mut child = child_command("cat").stdin(Stdio::piped()).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    std::io::Write::write_all(&mut stdin, b"abcdefg").unwrap();
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout.contains("[abc][def][g]"), "{stdout}");
}
