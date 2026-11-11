//! Adversary pass on story `build-time-back` (row 6.2 rust of
//! `docs/quality.md`): the dependency build the driver starts ahead
//! (`crates/ignite/src/prebuild.rs`) is stopped when the driver fails, "so
//! none outlives the driver". A driver that is ended by a signal fails
//! nothing and drops nothing: this pins what is left running then. And the
//! debug information of the TLS engine's C code in a dev executable of the
//! Rust target, for a program built outside the repository.

use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The processes whose command line names `path`, each with its id.
fn processes_naming(path: &Path) -> Vec<(u32, String)> {
    let needle = path.to_string_lossy().into_owned();
    std::fs::read_dir("/proc")
        .unwrap()
        .flatten()
        .filter_map(|entry| {
            let id = entry.file_name().to_string_lossy().parse::<u32>().ok()?;
            let cmdline = std::fs::read(entry.path().join("cmdline")).ok()?;
            Some((id, String::from_utf8_lossy(&cmdline).replace('\0', " ")))
        })
        .filter(|(_, cmdline)| cmdline.contains(&needle))
        .collect()
}

/// `kill -<signal> <id>`.
fn signal(id: u32, signal: &str) {
    let _ = Command::new("kill")
        .arg(format!("-{signal}"))
        .arg(id.to_string())
        .stderr(Stdio::null())
        .status();
}

/// `kill <driver>` (SIGTERM, what a supervisor or a script sends to the one
/// process it started) while the driver still parses and generates: the
/// cargo it started ahead, and the compilers under it, are gone with it. A
/// second is allowed for them to end; the build ahead compiles `libc` and
/// `nix` without a compiler cache here, which takes several times that.
#[test]
fn a_driver_ended_by_sigterm_leaves_no_build_ahead() {
    a_driver_ended_by_leaves_no_build_ahead("TERM", 15);
}

/// The same with SIGINT, what a terminal sends on Ctrl-C (here to the
/// driver alone, not to its group).
#[test]
fn a_driver_ended_by_sigint_leaves_no_build_ahead() {
    a_driver_ended_by_leaves_no_build_ahead("INT", 2);
}

/// The same with SIGHUP, what a closed terminal sends.
#[test]
fn a_driver_ended_by_sighup_leaves_no_build_ahead() {
    a_driver_ended_by_leaves_no_build_ahead("HUP", 1);
}

/// `kill -<name> <driver>` while the driver still parses and generates,
/// the driver started with the signal's default action (`env
/// --default-signal`, whatever this test process ignores): the driver
/// ends by that signal and nothing of the build ahead runs a second later.
fn a_driver_ended_by_leaves_no_build_ahead(name: &str, number: i32) {
    let dir = tmp(&format!("ahead-killed-{name}"));
    let target = dir.join("cargo");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/todo/todo.ignite");
    let mut driver = Command::new("env")
        .arg("--default-signal")
        .arg(env!("CARGO_BIN_EXE_ignite"))
        .arg("build")
        .arg(&source)
        .args(["--target", "rust"])
        .arg("--out")
        .arg(dir.join("out"))
        .arg("--target-dir")
        .arg(&target)
        .env("RUSTC_WRAPPER", "")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let ahead = |driver: u32| -> Vec<(u32, String)> {
        processes_naming(&target)
            .into_iter()
            .filter(|(id, _)| *id != driver)
            .collect()
    };
    // The project is written when generation is over; a cargo seen before
    // that is the one started ahead, not the program's build.
    let project = dir.join("out/Cargo.toml");
    let mut started = false;
    for _ in 0..4000 {
        if project.exists() || driver.try_wait().unwrap().is_some() {
            break;
        }
        if !ahead(driver.id()).is_empty() {
            started = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    if !started {
        let _ = driver.kill();
        let _ = driver.wait();
        for (id, _) in ahead(0) {
            signal(id, "KILL");
        }
        panic!("the driver started no build ahead while it generated");
    }
    signal(driver.id(), name);
    let status = driver.wait().unwrap();
    let mut left = ahead(0);
    for _ in 0..20 {
        if left.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        left = ahead(0);
    }
    // Nothing this test started outlives it, whatever it found.
    for _ in 0..200 {
        let running = ahead(0);
        if running.is_empty() {
            break;
        }
        for (id, _) in &running {
            signal(*id, "KILL");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        status.signal(),
        Some(number),
        "the driver was to end by SIG{name}: {status:?}"
    );
    assert!(
        left.is_empty(),
        "{} processes of the build ahead still ran a second after the driver was gone: {left:#?}",
        left.len()
    );
}

/// The names of the compile units of `file` that lie in a cargo registry
/// or a git checkout (`readelf --debug-dump=info`, units only).
fn third_party_units(file: &Path) -> Vec<String> {
    let dump = Command::new("readelf")
        .args(["--wide", "--debug-dump=info", "--dwarf-depth=1"])
        .arg(file)
        .output()
        .unwrap();
    assert!(dump.status.success(), "readelf {}", file.display());
    String::from_utf8_lossy(&dump.stdout)
        .lines()
        .filter(|line| line.contains("DW_AT_name"))
        .filter(|line| line.contains("/registry/src/") || line.contains("/git/checkouts/"))
        .map(|line| line.rsplit(": ").next().unwrap_or(line).trim().to_string())
        .collect()
}

/// The story's acceptance for the Rust target: the generated project
/// builds its dependencies without debug information in the dev profile.
/// The C and assembly of the TLS engine get theirs switched off by the
/// repository's `.cargo/config.toml` (`CFLAGS = "-g0"`), which cargo reads
/// only when it runs inside the repository. A program is built where its
/// author keeps it: the driver runs in `/` here, the output lies where
/// every test's does.
#[test]
fn a_tls_program_built_outside_the_repository_holds_no_third_party_debug_information() {
    let dir = tmp("tls-outside");
    let source = dir.join("dial.ignite");
    std::fs::write(
        &source,
        "package dial\n\nimport std.tls.{TlsOptions}\n\n\
         fn open(port: Int) -> Result<TlsStream, IoError> {\n  \
         let stream = TcpStream.connect(\"127.0.0.1\", port)?\n  \
         Tls.client(stream, \"localhost\", TlsOptions {})\n}\n\n\
         type Dial {\n  executable: Text\n}\n\n\
         fn Dial.main(self: Dial) -> Result<Unit, IoError> {\n  \
         let session = open(9)?\n  session.close()\n}\n\n\
         system Dialer {\n  component dial: Dial { executable = \"dial\" }\n}\n",
    )
    .unwrap();
    let target = dir.join("cargo");
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("build")
        .arg(&source)
        .args(["--target", "rust"])
        .arg("--out")
        .arg(dir.join("out"))
        .arg("--target-dir")
        .arg(&target)
        .current_dir("/")
        // `cargo test` gives this process the repository's `[env]`, the
        // `CFLAGS` in question; a build outside the repository has none.
        .env_remove("CFLAGS")
        .env("RUSTC_WRAPPER", "")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let dial = target.join("debug/dial");
    let third = third_party_units(&dial);
    assert!(
        third.is_empty(),
        "{} compile units of third-party crates in {}, e.g. {:?}",
        third.len(),
        dial.display(),
        &third[..third.len().min(4)]
    );
}

/// The signals process `id` blocks, bit `n - 1` for signal `n` (`SigBlk`
/// of `/proc/<id>/status`).
fn blocked_signals(id: u32) -> Option<u64> {
    std::fs::read_to_string(format!("/proc/{id}/status"))
        .ok()?
        .lines()
        .find_map(|line| line.strip_prefix("SigBlk:"))
        .and_then(|mask| u64::from_str_radix(mask.trim(), 16).ok())
}

/// Adversary pass 2. A process inherits the signal mask of the thread
/// that starts it; `std::process::Command` does not reset it. The driver
/// blocks SIGTERM, SIGINT and SIGHUP in the thread that starts the build
/// ahead, so that cargo and every compiler and build script under it ran
/// with the three blocked, where up to `cddabcb6` they blocked nothing.
/// A build ahead that is left behind, by a driver ended with SIGKILL
/// ("cannot be taken: the build ahead then runs to its end"), then no
/// longer ends when it is sent SIGTERM, as it did. Both are pinned: the
/// mask of each process of the build ahead, and what is left two seconds
/// after the driver was killed and each left process was sent SIGTERM.
#[test]
fn the_build_ahead_blocks_no_signal_and_ends_on_sigterm() {
    const ENDING: u64 = (1 << 14) | (1 << 1) | 1;
    let dir = tmp("ahead-mask");
    let target = dir.join("cargo");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/todo/todo.ignite");
    let mut driver = Command::new("env")
        .arg("--default-signal")
        .arg(env!("CARGO_BIN_EXE_ignite"))
        .arg("build")
        .arg(&source)
        .args(["--target", "rust"])
        .arg("--out")
        .arg(dir.join("out"))
        .arg("--target-dir")
        .arg(&target)
        .env("RUSTC_WRAPPER", "")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let ahead = |driver: u32| -> Vec<(u32, String)> {
        processes_naming(&target)
            .into_iter()
            .filter(|(id, _)| *id != driver)
            .collect()
    };
    // The compile of `libc`, the longest unit of the build ahead.
    let project = dir.join("out/Cargo.toml");
    let mut compiling = false;
    for _ in 0..4000 {
        if project.exists() || driver.try_wait().unwrap().is_some() {
            break;
        }
        if ahead(driver.id())
            .iter()
            .any(|(_, command)| command.contains("--crate-name libc "))
        {
            compiling = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // A process between `vfork` and `exec` blocks every signal for that
    // moment: only a mask of few signals is one it runs with.
    let masks: Vec<(String, u64)> = ahead(driver.id())
        .into_iter()
        .filter_map(|(id, command)| {
            let program = command.split_whitespace().next().unwrap_or_default();
            let program = program.rsplit('/').next().unwrap_or_default().to_string();
            Some((program, blocked_signals(id)?))
        })
        .filter(|(_, mask)| mask.count_ones() <= 8)
        .collect();
    let _ = driver.kill();
    let _ = driver.wait();
    let mut left = ahead(0);
    for _ in 0..40 {
        if left.is_empty() {
            break;
        }
        for (id, _) in &left {
            signal(*id, "TERM");
        }
        std::thread::sleep(Duration::from_millis(50));
        left = ahead(0);
    }
    // Nothing this test started outlives it, whatever it found.
    for _ in 0..200 {
        let running = ahead(0);
        if running.is_empty() {
            break;
        }
        for (id, _) in &running {
            signal(*id, "KILL");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        compiling && !masks.is_empty(),
        "the driver started no build ahead that compiled libc"
    );
    let blocking: Vec<String> = masks
        .iter()
        .filter(|(_, mask)| mask & ENDING != 0)
        .map(|(program, mask)| format!("{program} SigBlk {mask:016x}"))
        .collect();
    assert!(
        blocking.is_empty() && left.is_empty(),
        "of {} processes of the build ahead {} block SIGTERM, SIGINT or SIGHUP: {blocking:?}; \
         {} still ran two seconds after the driver was killed and each was sent SIGTERM",
        masks.len(),
        blocking.len(),
        left.len()
    );
}

/// Adversary pass 2. "A user's own flags win" (`docs/slice.md`, the
/// driver's `CFLAGS=-g0`): a user who sets the C flags of their builds in
/// cargo's configuration, `[env] CFLAGS` in a `.cargo/config.toml` above
/// the directory they build in (the way this repository sets its own),
/// got them to the C compiler of the TLS engine's build script up to
/// `cddabcb6`. Cargo does not let `[env]` replace a variable of its own
/// environment, so a `CFLAGS` the driver puts there takes the user's
/// place. The C compiler here is a script that logs its arguments; the
/// build is stopped at the first C file of `ring` it compiles.
#[test]
fn c_flags_of_the_users_cargo_configuration_reach_the_c_compiler() {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::CommandExt;
    const USER: &str = "-DIGNITE_ADVERSARY_USER_FLAG=1";
    let dir = tmp("cflags-config");
    let source = dir.join("dial.ignite");
    std::fs::write(
        &source,
        "package dial\n\nimport std.tls.{TlsOptions}\n\n\
         fn open(port: Int) -> Result<TlsStream, IoError> {\n  \
         let stream = TcpStream.connect(\"127.0.0.1\", port)?\n  \
         Tls.client(stream, \"localhost\", TlsOptions {})\n}\n\n\
         type Dial {\n  executable: Text\n}\n\n\
         fn Dial.main(self: Dial) -> Result<Unit, IoError> {\n  \
         let session = open(9)?\n  session.close()\n}\n\n\
         system Dialer {\n  component dial: Dial { executable = \"dial\" }\n}\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join(".cargo")).unwrap();
    std::fs::write(
        dir.join(".cargo/config.toml"),
        format!("[env]\nCFLAGS = \"{USER}\"\n"),
    )
    .unwrap();
    let log = dir.join("cc.log");
    let compiler = dir.join("cc.sh");
    std::fs::write(
        &compiler,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\nexec cc \"$@\"\n",
            log.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&compiler, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut driver = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("build")
        .arg(&source)
        .args(["--target", "rust"])
        .arg("--out")
        .arg(dir.join("out"))
        .arg("--target-dir")
        .arg(dir.join("cargo"))
        .current_dir(&dir)
        // `cargo test` gives this process the repository's `[env]`; the
        // user here has no `CFLAGS` in their environment.
        .env_remove("CFLAGS")
        .env("CC", &compiler)
        .env("RUSTC_WRAPPER", "")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    let compile_of_ring = || -> Option<String> {
        std::fs::read_to_string(&log)
            .ok()?
            .lines()
            // A compile (`-c`), not the probe of the compiler's family.
            .find(|line| {
                line.contains("/ring-")
                    && line.trim_end().ends_with(".c")
                    && line.split_whitespace().any(|word| word == "-c")
            })
            .map(str::to_string)
    };
    let mut compile = None;
    for _ in 0..24000 {
        compile = compile_of_ring();
        if compile.is_some() || driver.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // The driver, its cargo and what that started are one process group.
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{}", driver.id())])
        .stderr(Stdio::null())
        .status();
    let _ = driver.wait();
    let compile = compile.or_else(compile_of_ring);
    let compile = compile.expect("the build compiled no C file of ring");
    let flags: Vec<&str> = compile.split_whitespace().collect();
    assert!(
        flags.contains(&USER),
        "the user's `[env] CFLAGS = \"{USER}\"` did not reach the C compiler; debug flags it got: {:?}",
        flags
            .iter()
            .filter(|flag| flag.starts_with("-g"))
            .collect::<Vec<_>>()
    );
}

/// Adversary pass 2. A driver that is started with SIGTERM, SIGINT or
/// SIGHUP blocked (`env --block-signal`, a supervisor that hands its
/// signal mask to what it starts) kept the signal blocked in every one of
/// its threads up to `cddabcb6`: one sent to it stayed pending and the
/// driver did its work. The signal handling of the build ahead may block
/// more for a while; it is not to unblock what the driver was started
/// with. The driver here waits for its standard input (`ignite check
/// --stdin-for`), is sent the signal, then gets its input, and is to end
/// as a driver that was sent nothing ends: exit 0, the file being clean.
#[test]
fn a_driver_started_with_an_ending_signal_blocked_keeps_it_blocked() {
    use std::io::Write;
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/todo/todo.ignite");
    let text = std::fs::read(&source).unwrap();
    for name in ["TERM", "INT", "HUP"] {
        let mut driver = Command::new("env")
            .arg(format!("--default-signal={name}"))
            .arg(format!("--block-signal={name}"))
            .arg(env!("CARGO_BIN_EXE_ignite"))
            .args(["check", "--stdin-for"])
            .arg(&source)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // `env` blocks the signal before it becomes the driver.
        let comm = format!("/proc/{}/comm", driver.id());
        let mut is_driver = false;
        for _ in 0..4000 {
            if std::fs::read_to_string(&comm).is_ok_and(|name| name.trim() == "ignite") {
                is_driver = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(is_driver, "`env` did not become the driver");
        let masks: Vec<String> = std::fs::read_dir(format!("/proc/{}/task", driver.id()))
            .unwrap()
            .flatten()
            .filter_map(|task| std::fs::read_to_string(task.path().join("status")).ok())
            .filter_map(|status| {
                status
                    .lines()
                    .find_map(|line| line.strip_prefix("SigBlk:"))
                    .map(|mask| mask.trim().to_string())
            })
            .collect();
        signal(driver.id(), name);
        // A driver the signal ended has closed its end: the write then fails.
        let _ = driver.stdin.take().unwrap().write_all(&text);
        let output = driver.wait_with_output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "started with SIG{name} blocked and sent SIG{name}: {:?} (SigBlk of its threads \
             before the signal: {masks:?}); stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Adversary pass 2. The signal handling of the build ahead is to cost a
/// command that starts no build ahead nothing ("the watcher must not add
/// calls to a build without a build ahead"). `ignite check` starts no
/// cargo; up to `cddabcb6` it read no `/proc/self/status` and made no
/// `rt_sigprocmask` call that names SIGHUP, SIGINT and SIGTERM. Counted
/// with `strace -f` over the driver.
#[test]
fn a_command_without_a_build_ahead_makes_no_system_call_for_its_signals() {
    let works = Command::new("strace")
        .args(["-o", "/dev/null", "true"])
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("strace is needed to count system calls and did not run: {e}"));
    assert!(
        works.success(),
        "strace cannot trace a child here (ptrace is blocked): {works:?}"
    );
    let dir = tmp("check-calls");
    let log = dir.join("strace.log");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/todo/todo.ignite");
    let output = Command::new("strace")
        .args(["-f", "-e", "trace=openat,rt_sigprocmask", "-o"])
        .arg(&log)
        .arg(env!("CARGO_BIN_EXE_ignite"))
        .arg("check")
        .arg(&source)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let trace = std::fs::read_to_string(&log).unwrap();
    let status_reads = trace
        .lines()
        .filter(|line| line.contains("openat(") && line.contains("\"/proc/self/status\""))
        .count();
    let mask_calls = trace
        .lines()
        .filter(|line| line.contains("rt_sigprocmask(") && line.contains("[HUP INT TERM]"))
        .count();
    assert_eq!(
        (status_reads, mask_calls),
        (0, 0),
        "`ignite check` opened /proc/self/status {status_reads} times and made {mask_calls} \
         rt_sigprocmask calls naming [HUP INT TERM]; 0 and 0 at cddabcb6"
    );
}
