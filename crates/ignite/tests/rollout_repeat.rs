//! Rollouts in a row on one `ignite host` (story rollout-drain-stuck): the
//! terminal design applied as revision 1, then three edits of its `status`
//! reply, each applied while no session is open. With no session the old
//! server exits on SIGTERM at once, often before the driver awaits its
//! `released` line, so the rollout sees the exit instead of `Released`.
//! Every apply exits 0 with a completed receipt, and the server answers the
//! edited reply after each.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ignite_eval::Json;

use crate::common::apply::{BUILD, LIVE, build_lock, shared_build, target_args};

const TARGET: &str = "rust";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn wait(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("read child exit") {
            return Some(status);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A copy of the terminal design on its own port and a fresh host state.
struct Case {
    root: PathBuf,
    state: PathBuf,
    source: PathBuf,
    counter: u64,
}

impl Case {
    fn new(port: u16) -> Case {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("rollout-repeat-{}-{stamp}", std::process::id()));
        let fixture = root.join("terminal");
        fs::create_dir_all(&fixture).unwrap();
        for entry in fs::read_dir(repo().join("examples/terminal")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "ignite") {
                fs::copy(&path, fixture.join(path.file_name().unwrap())).unwrap();
            }
        }
        let source = fixture.join("terminal.design.ignite");
        let original = read(&source);
        assert!(
            original.contains("port = 2323"),
            "terminal fixture port changed"
        );
        fs::write(
            &source,
            original.replace("port = 2323", &format!("port = {port}")),
        )
        .unwrap();
        let state = root.join("state");
        fs::create_dir_all(&state).unwrap();
        shared_build(&state);
        Case {
            root,
            state,
            source,
            counter: 0,
        }
    }

    fn cli(&mut self, label: &str, args: &[&str], limit: Duration) -> (Option<i32>, String) {
        self.counter += 1;
        let stdout = self.root.join(format!("{}-{label}.stdout", self.counter));
        let stderr = self.root.join(format!("{}-{label}.stderr", self.counter));
        let mut child = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .args(args)
            .current_dir(repo())
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
            .stdin(Stdio::null())
            .stdout(File::create(&stdout).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .expect("start CLI");
        let code = match wait(&mut child, limit) {
            Some(status) => status.code(),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                None
            }
        };
        (
            code,
            format!("stdout: {}\nstderr: {}", read(&stdout), read(&stderr)),
        )
    }

    fn prepare(&mut self, label: &str) -> PathBuf {
        let out = self.root.join(format!("{label}.json"));
        let source = self.source.to_str().unwrap().to_string();
        let state = self.state.to_str().unwrap().to_string();
        let mut args = vec![
            "plan".to_string(),
            source,
            "--assembly".into(),
            "local".into(),
            "--for-apply".into(),
            "--state".into(),
            state,
            "--out".into(),
            out.to_str().unwrap().into(),
        ];
        args.extend(target_args(TARGET));
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let (code, output) = {
            let _build = build_lock(TARGET);
            self.cli(label, &args, BUILD)
        };
        assert_eq!(code, Some(0), "{label}\n{output}");
        out
    }

    /// `ignite apply`, bounded by [`LIVE`]: the exit code (`None` past the
    /// bound) and the receipt's `state`.
    fn apply(&mut self, label: &str, plan: &Path) -> (Option<i32>, Option<String>, String) {
        let state = self.state.to_str().unwrap().to_string();
        let args = ["apply", plan.to_str().unwrap(), "--state", &state];
        let (code, output) = self.cli(label, &args, LIVE);
        let receipt = output
            .strip_prefix("stdout: ")
            .and_then(|rest| rest.split_once("\nstderr: "))
            .and_then(|(stdout, _)| Json::parse(stdout).ok());
        let state = receipt
            .as_ref()
            .and_then(|r| r.get("state"))
            .and_then(Json::as_str)
            .map(str::to_string);
        (code, state, output)
    }

    fn edit_status(&self, from: &str, to: &str) {
        let commands = self.source.parent().unwrap().join("commands.ignite");
        let text = read(&commands);
        let old = format!("Status => Reply {{ lines = [\"{from}\"] }}");
        assert_eq!(text.matches(&old).count(), 1, "status reply {from:?}");
        fs::write(
            &commands,
            text.replace(&old, &format!("Status => Reply {{ lines = [\"{to}\"] }}")),
        )
        .unwrap();
    }
}

struct Host {
    child: Option<Child>,
    stderr: PathBuf,
}

impl Host {
    fn start(case: &Case) -> Host {
        let stderr = case.root.join("host.stderr");
        let child = Command::new(env!("CARGO_BIN_EXE_ignite"))
            .args(["host", "--state"])
            .arg(&case.state)
            .current_dir(repo())
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "line-tables-only")
            .stdin(Stdio::piped())
            .stdout(File::create(case.root.join("host.stdout")).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .expect("start host");
        let mut host = Host {
            child: Some(child),
            stderr,
        };
        let deadline = Instant::now() + LIVE;
        loop {
            assert!(
                host.child.as_mut().unwrap().try_wait().unwrap().is_none(),
                "host exited: {}",
                read(&host.stderr)
            );
            if ignite::apply::inspect(&case.state, None)
                .is_ok_and(|status| status.get("online").and_then(Json::as_bool) == Some(true))
            {
                return host;
            }
            assert!(
                Instant::now() < deadline,
                "host socket deadline: {}",
                read(&host.stderr)
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn stop(&mut self) {
        let mut child = self.child.take().expect("host is running");
        let stdin = child.stdin.as_mut().expect("host control stdin");
        writeln!(stdin, "stop").unwrap();
        stdin.flush().unwrap();
        let status = wait(&mut child, LIVE);
        assert!(
            status.is_some_and(|status| status.success()),
            "host stop failed: {status:?}\n{}",
            read(&self.stderr)
        );
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Whether the server on `port` answers `status` with `text`.
fn answers(port: u16, text: &str) -> bool {
    let Ok(mut stream) =
        TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_secs(2))
    else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let prompt = |stream: &mut TcpStream| {
        let mut received = Vec::new();
        let mut bytes = [0; 1024];
        while !received.ends_with(b"terminal> \xff\xf9") {
            match stream.read(&mut bytes) {
                Ok(0) | Err(_) => return None,
                Ok(count) => received.extend_from_slice(&bytes[..count]),
            }
        }
        Some(received)
    };
    if prompt(&mut stream).is_none() || stream.write_all(b"status\r\n").is_err() {
        return false;
    }
    let reply = prompt(&mut stream);
    let _ = stream.write_all(b"quit\r\n");
    reply.is_some_and(|reply| {
        String::from_utf8_lossy(&reply)
            .lines()
            .any(|line| line.trim() == text)
    })
}

fn await_answer(port: u16, text: &str, host: &Host) {
    let deadline = Instant::now() + LIVE;
    while !answers(port, text) {
        assert!(
            Instant::now() < deadline,
            "server never answered {text:?}\n{}",
            read(&host.stderr)
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Revision 1 starts the server; revisions 2 to 4 are three rollouts in a
/// row. Before the fix the second rollout waited for the first one's
/// deadline and the third was refused for good.
#[test]
fn three_rollouts_in_a_row_each_apply_completes() {
    let reserved = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = reserved.local_addr().unwrap().port();
    let mut case = Case::new(port);
    let mut host = Host::start(&case);
    let mut plan = case.prepare("revision-1");
    drop(reserved);
    let mut text = "ready".to_string();
    for revision in 1..=4 {
        if revision > 1 {
            let next = format!("ready {revision}");
            case.edit_status(&text, &next);
            text = next;
            plan = case.prepare(&format!("revision-{revision}"));
        }
        let started = Instant::now();
        let (code, state, output) = case.apply(&format!("apply-{revision}"), &plan);
        assert_eq!(
            (code, state.as_deref()),
            (Some(0), Some("completed")),
            "apply of revision {revision} after {:?}\n{output}\nhost: {}",
            started.elapsed(),
            read(&host.stderr)
        );
        await_answer(port, &text, &host);
        assert!(
            !read(&host.stderr).contains("rollout already in progress"),
            "revision {revision}: a rollout was refused\n{}",
            read(&host.stderr)
        );
    }
    host.stop();
}
