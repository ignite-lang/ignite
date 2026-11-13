//! The measuring half of `ignite-perf path-cost` ([`crate::path_cost`]): builds the programs
//! of the scenarios, runs them under callgrind and `strace`, and drives their operations.
//!
//! The instructions of a server's cell are a part of one callgrind run: the run is dumped
//! (`vgdb dump`, which also zeroes the counters) once the start has come to rest, once after
//! the set-up and warm-up, and once after each operation's batch; each dump holds what ran
//! since the one before. The system calls are the lines a `strace -f` trace of a second run
//! gained between the same points. A run is at rest when the valgrind process used no CPU,
//! or the trace gained no line, for [`QUIET`]: longer than the second after which a worker
//! without a task ends (`ignite-io`'s `LINGER`), so a batch's cell holds that end.
//!
//! Each execution of an operation, set-up and warm-up included, ends when the server has
//! settled ([`settled`]): every thread of it blocked. So a request never finds the server still
//! busy with the one before, and a connection's end never arrives before the server has
//! accepted it, whatever else runs on the CPUs: the path a request takes does not depend on
//! when the client is scheduled.
//!
//! The mode pins itself to two CPUs ([`pin_two_cpus`]), so what it starts (valgrind, strace,
//! the servers, `ignite` and its cargo) uses at most two cores, and a server's workers are as
//! many on every machine. The programs are staged, built and run in [`server_home`], a
//! directory outside the tree whose path has one length on a machine; a server's executable is
//! copied to its `bin/` and starts as `../bin/<executable>` from `run/`, with `TMPDIR=../tmp`
//! its only variable. The executable's path is in `/proc/self/maps`, which a starting thread
//! reads, the working directory in `getcwd`, and the build's paths in the program's data: in
//! the work directory their length moved the start of `rust/todo` by 479 instructions, and
//! `patch` and `delete` by 280, between two work directories.
//!
//! Every process the mode starts is in a process group of its own, and [`stop_on_signal`]
//! turns SIGINT, SIGTERM and SIGHUP into killing those groups before the mode exits, so a
//! stopped run leaves no server, valgrind or build behind.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

use crate::path_cost::{
    Action, CliOp, Kind, Profile, Request, Scenario, Scope, Server, ServerOp, callgrind_profile,
    strace_profile, strace_segment,
};
use crate::perf::{R, ensure, io_err};

/// How long a run shows no activity before it is at rest.
pub const QUIET: Duration = Duration::from_millis(1500);
/// The longest any wait of a run takes: the listening line, a reply, a rest, a dump.
pub const LIMIT: Duration = Duration::from_secs(600);
/// How often in a row, [`SETTLE_POLL`] apart, every thread of a server is seen blocked before
/// it has settled.
pub const SETTLE_SEEN: usize = 3;
pub const SETTLE_POLL: Duration = Duration::from_millis(1);
/// The environment of a server, relative to its run directory.
pub const SERVER_TMPDIR: &str = "../tmp";

/// The directory the programs of the work directory `work` are staged, built and run in:
/// `<XDG_CACHE_HOME or ~/.cache>/ignite/path-cost/<16 hex digits of the SHA-256 of work's path>`,
/// with `src/`, `build/`, `cargo/`, `cache/`, `out/`, `bin/`, `run/` and `tmp/` in it; the work
/// directory keeps the instruments' outputs. Its path has one length in every tree of a
/// machine, and each work directory has its own: the paths a build writes into a program, and
/// those a server sees, have one length too.
pub fn server_home(work: &Path) -> R<PathBuf> {
    use std::os::unix::ffi::OsStrExt as _;
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|h| h.join(".cache"))
        })
        .ok_or("neither XDG_CACHE_HOME nor HOME names an absolute directory")?;
    let id = crate::path_cost::digest(work.as_os_str().as_bytes());
    Ok(cache.join("ignite/path-cost").join(&id[..16]))
}

/// The process groups of the children running now, each led by the child.
static CHILDREN: Mutex<Vec<u32>> = Mutex::new(Vec::new());

fn children() -> std::sync::MutexGuard<'static, Vec<u32>> {
    CHILDREN.lock().unwrap_or_else(|e| e.into_inner())
}

/// `command` started in a process group of its own, which [`stop_on_signal`] kills.
pub fn spawn(command: &mut Command) -> std::io::Result<Child> {
    command.process_group(0);
    let mut running = children();
    let child = command.spawn()?;
    running.push(child.id());
    Ok(child)
}

/// The child `pid` was waited for: its group is no longer killed.
fn reaped(pid: u32) {
    children().retain(|&p| p != pid);
}

/// `command` run to its end through [`spawn`], stdin from `/dev/null`, its output collected.
pub fn output(command: &mut Command) -> std::io::Result<Output> {
    let child = spawn(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )?;
    let pid = child.id();
    let result = child.wait_with_output();
    reaped(pid);
    result
}

/// Block SIGINT, SIGTERM and SIGHUP in this thread (and so in the threads it starts; a child
/// starts with no signal blocked) and wait for them in a thread of their own: one of them
/// kills the process group of every child running, waits for each child, and exits with 128
/// and the signal's number. Called by the binary before the mode starts anything.
pub fn stop_on_signal() -> R<()> {
    use nix::sys::signal::{SigSet, Signal, killpg};
    use nix::sys::wait::waitpid;
    use nix::unistd::Pid;
    let mut set = SigSet::empty();
    for signal in [Signal::SIGINT, Signal::SIGTERM, Signal::SIGHUP] {
        set.add(signal);
    }
    set.thread_block()
        .map_err(|e| format!("blocking the signals: {e}"))?;
    std::thread::spawn(move || {
        let Ok(signal) = set.wait() else {
            return;
        };
        let running = children();
        for &pid in running.iter() {
            let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGKILL);
        }
        for &pid in running.iter() {
            let _ = waitpid(Pid::from_raw(pid as i32), None);
        }
        eprintln!(
            "{}: stopped by {signal}; killed {} process group(s)",
            crate::path_cost::METRIC,
            running.len()
        );
        std::process::exit(128 + signal as i32);
    });
    Ok(())
}

/// `name` on `PATH`.
fn on_path(name: &str) -> R<PathBuf> {
    std::env::var_os("PATH")
        .iter()
        .flat_map(std::env::split_paths)
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
        .ok_or_else(|| format!("{name} is not on PATH"))
}

/// Restrict this process (and so what it starts) to two of the CPUs it may use, chosen by its
/// process id so that two runs at once tend to differ; the CPUs chosen.
pub fn pin_two_cpus() -> R<Vec<usize>> {
    use nix::sched::{CpuSet, sched_getaffinity, sched_setaffinity};
    use nix::unistd::Pid;
    let set = sched_getaffinity(Pid::from_raw(0)).map_err(|e| format!("sched_getaffinity: {e}"))?;
    let allowed: Vec<usize> = (0..CpuSet::count())
        .filter(|&cpu| set.is_set(cpu).unwrap_or(false))
        .collect();
    ensure(!allowed.is_empty(), "no CPU in this process' affinity")?;
    if allowed.len() <= 2 {
        return Ok(allowed);
    }
    let first = std::process::id() as usize % allowed.len();
    let chosen = vec![allowed[first], allowed[(first + 1) % allowed.len()]];
    let mut pinned = CpuSet::new();
    for &cpu in &chosen {
        pinned.set(cpu).map_err(|e| format!("CPU {cpu}: {e}"))?;
    }
    sched_setaffinity(Pid::from_raw(0), &pinned).map_err(|e| format!("sched_setaffinity: {e}"))?;
    Ok(chosen)
}

/// What a measurement started, for the cost of the row inside the gate.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tally {
    pub valgrind: usize,
    pub strace: usize,
    pub vgdb: usize,
    pub builds: usize,
    pub native: usize,
    pub instructions: u64,
}

impl Tally {
    /// The processes the mode started itself, `git` included; the builds and the counted
    /// `ignite` commands start more of their own (cargo, rustc, clang, the linker).
    pub fn text(&self) -> String {
        let git = crate::path_cost::GIT_RUNS.load(std::sync::atomic::Ordering::Relaxed);
        format!(
            "{}: {} processes started by the mode ({} valgrind, {} strace, {} vgdb, {} git, {} program builds, {} native warm-up runs; not counted: what the builds and the counted commands start), {} instructions under valgrind",
            crate::path_cost::METRIC,
            self.valgrind + self.strace + self.vgdb + git + self.builds + self.native,
            self.valgrind,
            self.strace,
            self.vgdb,
            git,
            self.builds,
            self.native,
            self.instructions
        )
    }
}

/// A child process, killed with its process group on drop.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let pid = nix::unistd::Pid::from_raw(self.0.id() as i32);
        let _ = nix::sys::signal::killpg(pid, nix::sys::signal::Signal::SIGKILL);
        let _ = self.0.wait();
        reaped(self.0.id());
    }
}

/// The measuring state: the compiler, the work directory, the programs built so far.
pub struct Tools {
    cli: PathBuf,
    repo: PathBuf,
    work: PathBuf,
    valgrind: PathBuf,
    strace: PathBuf,
    vgdb: PathBuf,
    home: PathBuf,
    next: usize,
    built: BTreeMap<(String, String), PathBuf>,
    pub tally: Tally,
}

fn absolute(path: &Path) -> R<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()
        .map_err(io_err("the current directory"))?
        .join(path))
}

impl Tools {
    pub fn new(cli: &Path, repo: &Path, work: &Path) -> R<Tools> {
        let cli = absolute(cli)?;
        ensure(
            cli.is_file(),
            format!("{}: no compiler (cargo build -p ignite)", cli.display()),
        )?;
        let work = absolute(work)?;
        let home = server_home(&work)?;
        for dir in ["tmp", "cache", "cargo"] {
            fs::create_dir_all(home.join(dir)).map_err(io_err(home.join(dir).display()))?;
        }
        Ok(Tools {
            cli,
            repo: absolute(repo)?,
            work,
            valgrind: on_path("valgrind")?,
            strace: on_path("strace")?,
            vgdb: on_path("vgdb")?,
            home,
            next: 0,
            built: BTreeMap::new(),
            tally: Tally::default(),
        })
    }

    /// A new empty directory under the work directory.
    fn fresh(&mut self, name: &str) -> R<PathBuf> {
        self.next += 1;
        let dir = self
            .work
            .join("runs")
            .join(format!("{:03}-{name}", self.next));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).map_err(io_err(dir.display()))?;
        Ok(dir)
    }

    /// A new empty directory for the outputs of a counted `ignite` command, under
    /// [`server_home`].
    fn fresh_out(&mut self, name: &str) -> R<PathBuf> {
        self.next += 1;
        let dir = self
            .home
            .join("out")
            .join(format!("{:03}-{name}", self.next));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).map_err(io_err(dir.display()))?;
        Ok(dir)
    }

    /// The scenario's files copied into `<home>/src/<scenario>`, where its commands run.
    fn stage(&self, repo_dir: &Path, scenario: &Scenario) -> R<PathBuf> {
        let dir = self.home.join("src").join(&scenario.name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).map_err(io_err(dir.display()))?;
        for file in &scenario.files {
            let from = repo_dir.join(&file.name);
            fs::copy(&from, dir.join(&file.name)).map_err(io_err(from.display()))?;
        }
        Ok(dir)
    }

    /// The environment of an `ignite` command: this process', with the runtime cache, the
    /// temporary directory and incremental builds fixed.
    fn cli_env(&self, command: &mut Command) {
        command
            .env("XDG_CACHE_HOME", self.home.join("cache"))
            .env("TMPDIR", self.home.join("tmp"))
            .env("CARGO_INCREMENTAL", "0");
    }

    /// The environment of a server: [`SERVER_TMPDIR`] only, the same in every work directory.
    fn server_env(&self, command: &mut Command) {
        command.env_clear().env("TMPDIR", SERVER_TMPDIR);
    }

    /// A fresh directory under the work directory for the instrument's outputs, the server's
    /// fresh run directory in [`server_home`] with its executable copied beside it, and the
    /// executable as a path from the run directory.
    fn server_run(&mut self, name: &str, binary: &Path) -> R<(PathBuf, PathBuf, String)> {
        let dir = self.fresh(name)?;
        let run = self.home.join("run");
        let _ = fs::remove_dir_all(&run);
        for sub in [&run, &self.home.join("bin"), &self.home.join("tmp")] {
            fs::create_dir_all(sub).map_err(io_err(sub.display()))?;
        }
        let file = binary
            .file_name()
            .ok_or_else(|| format!("{}: no file name", binary.display()))?;
        let copy = self.home.join("bin").join(file);
        let _ = fs::remove_file(&copy);
        fs::copy(binary, &copy).map_err(io_err(copy.display()))?;
        Ok((dir, run, format!("../bin/{}", file.to_string_lossy())))
    }

    /// Every cell of `scenario` on `target`, once: `(op, instructions, system calls)`.
    pub fn measure(
        &mut self,
        scenario: &Scenario,
        target: &str,
    ) -> R<Vec<(String, Profile, Profile)>> {
        let repo_dir = self.work_repo_dir(scenario)?;
        let stage = self.stage(&repo_dir, scenario)?;
        match &scenario.kind {
            Kind::Server(server) => {
                let binary = self.program(&stage, scenario, server, target)?;
                let instructions = self.server_callgrind(&binary, server)?;
                let syscalls = self.server_strace(&binary, server)?;
                Ok(instructions
                    .into_iter()
                    .zip(syscalls)
                    .map(|((op, i), (_, s))| (op, i, s))
                    .collect())
            }
            Kind::Cli(ops) => {
                let mut out = Vec::new();
                for op in ops.iter().filter(|op| op.target == target) {
                    let (i, s) = self.cli_op(&stage, op)?;
                    out.push((op.name.clone(), i, s));
                }
                Ok(out)
            }
        }
    }

    fn work_repo_dir(&self, scenario: &Scenario) -> R<PathBuf> {
        let dir = self
            .repo
            .join(crate::path_cost::SCENARIOS)
            .join(&scenario.name);
        ensure(
            dir.is_dir(),
            format!("{}: no scenario directory", dir.display()),
        )?;
        Ok(dir)
    }

    /// The server's executable built from the staged sources for `target` (`--release`), once
    /// per scenario and target.
    fn program(
        &mut self,
        stage: &Path,
        scenario: &Scenario,
        server: &Server,
        target: &str,
    ) -> R<PathBuf> {
        let key = (scenario.name.clone(), target.to_string());
        if let Some(path) = self.built.get(&key) {
            return Ok(path.clone());
        }
        let out = self
            .home
            .join("build")
            .join(format!("{}-{target}", scenario.name));
        let _ = fs::remove_dir_all(&out);
        let mut command = Command::new(&self.cli);
        command
            .arg("build")
            .arg(&server.source)
            .args(["--target", target, "--release", "--out"])
            .arg(&out)
            .current_dir(stage);
        if target == "rust" {
            command.arg("--target-dir").arg(self.home.join("cargo"));
        }
        self.cli_env(&mut command);
        let output = output(&mut command).map_err(io_err(self.cli.display()))?;
        self.tally.builds += 1;
        ensure(
            output.status.success(),
            format!(
                "building {}/{} for {target} failed:\n{}{}",
                scenario.name,
                server.source,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        )?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let binary = stdout
            .lines()
            .filter_map(|line| line.strip_prefix("built "))
            .map(PathBuf::from)
            .find(|path| {
                path.file_name()
                    .is_some_and(|n| n == server.executable.as_str())
            })
            .ok_or_else(|| {
                format!(
                    "no {} among the built executables:\n{stdout}",
                    server.executable
                )
            })?;
        self.built.insert(key, binary.clone());
        Ok(binary)
    }

    /// The instructions of the server's start and of each operation's batch, from one run
    /// under callgrind.
    fn server_callgrind(&mut self, binary: &Path, server: &Server) -> R<Vec<(String, Profile)>> {
        let (dir, run, executable) = self.server_run("callgrind", binary)?;
        let out = dir.join("callgrind.out");
        // valgrind maps its shared memory with vgdb from a file named after the prefix into
        // the server's address space, where `/proc/self/maps` shows its path
        let prefix = self.home.join("run").join("vgdb");
        let mut command = Command::new(&self.valgrind);
        command
            .arg("--tool=callgrind")
            .arg(format!("--callgrind-out-file={}", out.display()))
            .args([
                "--dump-line=no",
                "--compress-strings=no",
                "--compress-pos=no",
                "--vgdb=yes",
            ])
            .arg(format!("--vgdb-prefix={}", prefix.display()))
            .arg(format!("--log-file={}", dir.join("valgrind.log").display()))
            .arg(&executable)
            .args(["--port", "0"])
            .current_dir(&run);
        self.server_env(&mut command);
        let (child, port) = start(command, "valgrind")?;
        self.tally.valgrind += 1;
        let pid = child.0.id();
        let vgdb = self.vgdb.clone();
        let mut dumps = 0;
        let mut dump = |tally: &mut Tally| -> R<Profile> {
            cpu_at_rest(pid)?;
            dumps += 1;
            let profile = dump_part(&vgdb, &prefix, pid, &out, dumps)?;
            tally.vgdb += 1;
            tally.instructions += profile.total;
            Ok(profile)
        };
        let mut parts = vec![("start".to_string(), dump(&mut self.tally)?)];
        let mut session = Session::open(port, pid, server)?;
        dump(&mut self.tally)?;
        for op in &server.ops {
            session.batch(op, server.repeat)?;
            parts.push((op.name.clone(), dump(&mut self.tally)?));
        }
        drop(session);
        drop(child);
        Ok(parts)
    }

    /// The system calls of the server's start and of each operation's batch, from one run
    /// under `strace -f`.
    fn server_strace(&mut self, binary: &Path, server: &Server) -> R<Vec<(String, Profile)>> {
        let (dir, run, executable) = self.server_run("strace", binary)?;
        let trace = dir.join("trace.txt");
        let mut command = Command::new(&self.strace);
        command
            .args(["-f", "--kill-on-exit", "-o"])
            .arg(&trace)
            .arg(&executable)
            .args(["--port", "0"])
            .current_dir(&run);
        self.server_env(&mut command);
        let (child, port) = start(command, "strace")?;
        self.tally.strace += 1;
        let pid = traced_child(child.0.id())?;
        let mut mark = 0;
        let mut segment = || -> R<Profile> {
            let text = trace_at_rest(&trace)?;
            let (profile, end) = strace_segment(&text, mark);
            mark = end;
            Ok(profile)
        };
        let mut parts = vec![("start".to_string(), segment()?)];
        let mut session = Session::open(port, pid, server)?;
        segment()?;
        for op in &server.ops {
            session.batch(op, server.repeat)?;
            parts.push((op.name.clone(), segment()?));
        }
        drop(session);
        drop(child);
        Ok(parts)
    }

    /// The arguments of `op` with `{out}` a fresh directory and `{cargo}` the shared target
    /// directory.
    fn arguments(&mut self, op: &CliOp) -> R<Vec<String>> {
        let out = self.fresh_out(&format!("{}-{}", op.name, op.target))?;
        let cargo = self.home.join("cargo");
        Ok(op
            .args
            .iter()
            .map(|a| {
                a.replace("{out}", &out.to_string_lossy())
                    .replace("{cargo}", &cargo.to_string_lossy())
            })
            .collect())
    }

    /// One CLI operation: once natively (caches, cargo's target directory), then its
    /// instructions under callgrind and its system calls under `strace`, each in a run of its
    /// own; with every process it starts for [`Scope::Tree`], the `ignite` process alone for
    /// [`Scope::Driver`].
    fn cli_op(&mut self, stage: &Path, op: &CliOp) -> R<(Profile, Profile)> {
        let tree = op.scope == Scope::Tree;
        let args = self.arguments(op)?;
        let mut command = Command::new(&self.cli);
        command.args(&args).current_dir(stage);
        self.cli_env(&mut command);
        finished(command, &format!("ignite {}", op.args.join(" ")))?;
        self.tally.native += 1;

        let dir = self.fresh(&format!("callgrind-{}-{}", op.name, op.target))?;
        let args = self.arguments(op)?;
        let mut command = Command::new(&self.valgrind);
        command
            .arg("--tool=callgrind")
            .arg(format!(
                "--callgrind-out-file={}/callgrind.%p",
                dir.display()
            ))
            .args([
                "--dump-line=no",
                "--compress-strings=no",
                "--compress-pos=no",
                "--vgdb=no",
            ])
            .arg(format!("--log-file={}/valgrind.%p.log", dir.display()));
        if tree {
            command.arg("--trace-children=yes");
        }
        command.arg(&self.cli).args(&args).current_dir(stage);
        self.cli_env(&mut command);
        finished(command, &format!("valgrind ignite {}", op.args.join(" ")))?;
        self.tally.valgrind += 1;
        let mut instructions = Profile::default();
        let mut files: Vec<PathBuf> = fs::read_dir(&dir)
            .map_err(io_err(dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("callgrind."))
            })
            .collect();
        files.sort();
        ensure(
            !files.is_empty(),
            format!("{}: no callgrind output", dir.display()),
        )?;
        for file in files {
            let text = fs::read_to_string(&file).map_err(io_err(file.display()))?;
            instructions
                .add(&callgrind_profile(&text).map_err(|e| format!("{}: {e}", file.display()))?);
        }
        self.tally.instructions += instructions.total;

        let dir = self.fresh(&format!("strace-{}-{}", op.name, op.target))?;
        let trace = dir.join("trace.txt");
        let args = self.arguments(op)?;
        let mut command = Command::new(&self.strace);
        if tree {
            command.arg("-f");
        }
        command
            .arg("-o")
            .arg(&trace)
            .arg(&self.cli)
            .args(&args)
            .current_dir(stage);
        self.cli_env(&mut command);
        finished(command, &format!("strace ignite {}", op.args.join(" ")))?;
        self.tally.strace += 1;
        let text = fs::read_to_string(&trace).map_err(io_err(trace.display()))?;
        Ok((instructions, strace_profile(&text)))
    }
}

/// Run `command` to its end; it must succeed.
fn finished(mut command: Command, what: &str) -> R<()> {
    let output = output(&mut command).map_err(io_err(what))?;
    ensure(
        output.status.success(),
        format!(
            "{what} failed ({}):\n{}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

/// `command` started, and the port of its `listening on 127.0.0.1:<port>` line on stderr.
fn start(mut command: Command, what: &str) -> R<(Running, u16)> {
    let mut child = spawn(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped()),
    )
    .map_err(io_err(what))?;
    let stderr = child.stderr.take().ok_or("no stderr")?;
    let running = Running(child);
    let (ports, port) = mpsc::channel();
    std::thread::spawn(move || {
        let mut stderr = BufReader::new(stderr);
        let mut line = String::new();
        let mut seen = String::new();
        while stderr.read_line(&mut line).unwrap_or(0) > 0 {
            if let Some(rest) = line.trim_end().strip_prefix("listening on 127.0.0.1:") {
                let _ = ports.send(rest.parse::<u16>().map_err(|e| e.to_string()));
            } else if seen.len() < 4096 {
                seen.push_str(&line);
            }
            line.clear();
        }
        let _ = ports.send(Err(format!("the server ended before it listened: {seen}")));
    });
    let port = port
        .recv_timeout(LIMIT)
        .map_err(|_| format!("{what}: no `listening on 127.0.0.1:<port>` line"))??;
    Ok((running, port))
}

/// The CPU ticks the process `pid` used (user and system).
fn cpu_ticks(pid: u32) -> R<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))
        .map_err(io_err(format!("process {pid}")))?;
    let (_, after) = stat.rsplit_once(") ").ok_or("a /proc stat line")?;
    let fields: Vec<&str> = after.split(' ').collect();
    let tick = |i: usize| -> R<u64> {
        fields
            .get(i)
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| format!("field {i} of {stat:?}"))
    };
    Ok(tick(11)? + tick(12)?)
}

/// The state letter of each thread of the process `pid` (`R` running, `S` blocked, `t` stopped
/// by a tracer, ...).
fn thread_states(pid: u32) -> R<Vec<char>> {
    let dir = format!("/proc/{pid}/task");
    let mut states = Vec::new();
    for entry in fs::read_dir(&dir).map_err(io_err(format!("process {pid}")))? {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        if let Some((_, after)) = stat.rsplit_once(") ")
            && let Some(state) = after.chars().next()
        {
            states.push(state);
        }
    }
    ensure(!states.is_empty(), format!("process {pid} has no thread"))?;
    Ok(states)
}

/// Wait until every thread of the server `pid` is blocked (`S`) at [`SETTLE_SEEN`] looks in a
/// row, [`SETTLE_POLL`] apart: the server has done all a request or connection gave it to do
/// and waits for the next.
fn settled(pid: u32) -> R<()> {
    let deadline = Instant::now() + LIMIT;
    let mut seen = 0;
    while seen < SETTLE_SEEN {
        if thread_states(pid)?.iter().all(|&s| s == 'S') {
            seen += 1;
        } else {
            seen = 0;
        }
        ensure(
            Instant::now() < deadline,
            format!("process {pid} never settled"),
        )?;
        std::thread::sleep(SETTLE_POLL);
    }
    Ok(())
}

/// The process `strace` (`pid`) traces: its child.
fn traced_child(pid: u32) -> R<u32> {
    for entry in fs::read_dir("/proc").map_err(io_err("/proc"))? {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(child) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let parent = stat
            .rsplit_once(") ")
            .and_then(|(_, after)| after.split(' ').nth(1))
            .and_then(|p| p.parse::<u32>().ok());
        if parent == Some(pid) {
            return Ok(child);
        }
    }
    Err(format!("strace {pid} traces no process"))
}

/// Wait until the process `pid` used no CPU for [`QUIET`].
fn cpu_at_rest(pid: u32) -> R<()> {
    let deadline = Instant::now() + LIMIT;
    let mut last = cpu_ticks(pid)?;
    let mut since = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(100));
        let now = cpu_ticks(pid)?;
        if now != last {
            last = now;
            since = Instant::now();
        } else if since.elapsed() >= QUIET {
            return Ok(());
        }
        ensure(
            Instant::now() < deadline,
            format!("process {pid} never came to rest"),
        )?;
    }
}

/// The text of the trace at `path` once it gained nothing for [`QUIET`].
fn trace_at_rest(path: &Path) -> R<String> {
    let deadline = Instant::now() + LIMIT;
    let mut text = String::new();
    let mut since = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(100));
        let now = fs::read_to_string(path).unwrap_or_default();
        if now.len() != text.len() {
            text = now;
            since = Instant::now();
        } else if since.elapsed() >= QUIET {
            return Ok(text);
        }
        ensure(
            Instant::now() < deadline,
            format!("{} never came to rest", path.display()),
        )?;
    }
}

/// Dump the callgrind run `pid` (`vgdb dump`, which zeroes its counters) and read the part,
/// the `n`th file after `out`. `vgdb` runs with the server's environment: valgrind names its
/// shared memory file after the user and host of its environment, and both sides must name the
/// same file.
fn dump_part(vgdb: &Path, prefix: &Path, pid: u32, out: &Path, n: usize) -> R<Profile> {
    let mut command = Command::new(vgdb);
    command
        .env_clear()
        .env("TMPDIR", SERVER_TMPDIR)
        .arg(format!("--vgdb-prefix={}", prefix.display()))
        .arg(format!("--pid={pid}"))
        .arg("dump");
    let output = output(&mut command).map_err(io_err("vgdb"))?;
    ensure(
        output.status.success(),
        format!(
            "vgdb dump of {pid} failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    let file = PathBuf::from(format!("{}.{n}", out.display()));
    let deadline = Instant::now() + LIMIT;
    loop {
        if let Ok(text) = fs::read_to_string(&file)
            && text
                .trim_end()
                .lines()
                .last()
                .is_some_and(|l| l.starts_with("totals:"))
        {
            return callgrind_profile(&text).map_err(|e| format!("{}: {e}", file.display()));
        }
        ensure(
            Instant::now() < deadline,
            format!("{}: the dump was not written", file.display()),
        )?;
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The operations of a server's run: the keep-alive HTTP connection the requests share, and
/// how often each operation ran.
struct Session {
    port: u16,
    /// The server's process, which [`settled`] watches.
    pid: u32,
    http: Option<Client>,
    executed: BTreeMap<String, u64>,
}

impl Session {
    /// The set-up requests, then `warm` executions of each operation.
    fn open(port: u16, pid: u32, server: &Server) -> R<Session> {
        let mut session = Session {
            port,
            pid,
            http: None,
            executed: BTreeMap::new(),
        };
        for (times, request) in &server.setup {
            for _ in 0..*times {
                session.send(request, 0)?;
                settled(pid)?;
            }
        }
        for op in &server.ops {
            for _ in 0..server.warm {
                session.execute(op)?;
            }
        }
        Ok(session)
    }

    fn batch(&mut self, op: &ServerOp, repeat: usize) -> R<()> {
        for _ in 0..repeat {
            self.execute(op)?;
        }
        Ok(())
    }

    fn send(&mut self, request: &Request, count: u64) -> R<()> {
        let path = with_id(&request.path, count)?;
        if self.http.is_none() {
            self.http = Some(Client::connect(self.port)?);
        }
        let client = self.http.as_mut().ok_or("no connection")?;
        let (status, body) = client.send(&http_request(&request.method, &path, &request.body))?;
        ensure(
            status == request.status,
            format!(
                "{} {path}: {status}, {} expected: {body}",
                request.method, request.status
            ),
        )
    }

    fn execute(&mut self, op: &ServerOp) -> R<()> {
        let count = self.executed.get(&op.name).copied().unwrap_or(0);
        match &op.action {
            Action::Http(request) => self.send(request, count)?,
            Action::Connect => closed_connection(self.port, self.pid)?,
            Action::Telnet(line) => telnet(self.port, self.pid, line)?,
        }
        settled(self.pid)?;
        self.executed.insert(op.name.clone(), count + 1);
        Ok(())
    }
}

/// A connection opened and closed: once the server has accepted it and settled, this end's
/// write half shut, then read until the server has closed its end too. Shut at once instead,
/// the end would arrive sometimes before and sometimes after the server accepted, and the
/// count of a batch would vary with the scheduling of the two processes.
fn closed_connection(port: u16, pid: u32) -> R<()> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).map_err(io_err("connect"))?;
    stream
        .set_read_timeout(Some(LIMIT))
        .map_err(io_err("timeout"))?;
    settled(pid)?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(io_err("shutdown"))?;
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk).map_err(io_err("read to the end"))?;
        if n == 0 {
            return Ok(());
        }
    }
}

/// `path` with `{id:N}` replaced by `N + count`.
fn with_id(path: &str, count: u64) -> R<String> {
    let Some(start) = path.find("{id:") else {
        return Ok(path.to_string());
    };
    let end = path[start..]
        .find('}')
        .map(|i| start + i)
        .ok_or_else(|| format!("{path}: `{{id:N}}` not closed"))?;
    let first: u64 = path[start + 4..end]
        .parse()
        .map_err(|_| format!("{path}: `{{id:N}}` with a number"))?;
    Ok(format!(
        "{}{}{}",
        &path[..start],
        first + count,
        &path[end + 1..]
    ))
}

/// A request as the load generator sends it (`ignite-load`'s `http::request`).
fn http_request(method: &str, path: &str, body: &str) -> Vec<u8> {
    let mut out = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n");
    if !body.is_empty() || matches!(method, "POST" | "PATCH" | "PUT") {
        out.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        ));
    }
    out.push_str("\r\n");
    out.push_str(body);
    out.into_bytes()
}

/// One keep-alive HTTP/1.1 connection that sends a request and reads its whole answer.
struct Client {
    stream: TcpStream,
    buffer: Vec<u8>,
}

impl Client {
    fn connect(port: u16) -> R<Client> {
        let stream = TcpStream::connect(("127.0.0.1", port)).map_err(io_err("connect"))?;
        stream.set_nodelay(true).map_err(io_err("nodelay"))?;
        stream
            .set_read_timeout(Some(LIMIT))
            .map_err(io_err("timeout"))?;
        Ok(Client {
            stream,
            buffer: Vec::new(),
        })
    }

    fn send(&mut self, request: &[u8]) -> R<(u16, String)> {
        self.stream.write_all(request).map_err(io_err("send"))?;
        loop {
            if let Some(end) = self.buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&self.buffer[..end]).into_owned();
                let length: usize = head
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().ok())?
                    })
                    .unwrap_or(0);
                if self.buffer.len() >= end + 4 + length {
                    let body = String::from_utf8_lossy(&self.buffer[end + 4..end + 4 + length])
                        .into_owned();
                    self.buffer.drain(..end + 4 + length);
                    let status = head
                        .get(9..12)
                        .and_then(|code| code.parse().ok())
                        .ok_or_else(|| format!("status line of {head:?}"))?;
                    return Ok((status, body));
                }
            }
            let mut chunk = [0u8; 65536];
            let n = self
                .stream
                .read(&mut chunk)
                .map_err(io_err("read the answer"))?;
            ensure(n > 0, "the server closed before a whole answer")?;
            self.buffer.extend_from_slice(&chunk[..n]);
        }
    }
}

/// A Telnet session: connect, read until the first prompt ends (`IAC GA`), wait until the
/// server settled, send `line`, read until the server closes.
fn telnet(port: u16, pid: u32, line: &str) -> R<()> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).map_err(io_err("connect"))?;
    stream
        .set_read_timeout(Some(LIMIT))
        .map_err(io_err("timeout"))?;
    let mut seen = Vec::new();
    let mut chunk = [0u8; 4096];
    while !seen.windows(2).any(|w| w == [0xff, 0xf9]) {
        let n = stream.read(&mut chunk).map_err(io_err("read the prompt"))?;
        ensure(n > 0, format!("closed before the prompt: {seen:?}"))?;
        seen.extend_from_slice(&chunk[..n]);
    }
    settled(pid)?;
    stream
        .write_all(format!("{line}\r\n").as_bytes())
        .map_err(io_err("send the line"))?;
    loop {
        let n = stream.read(&mut chunk).map_err(io_err("read to the end"))?;
        if n == 0 {
            return Ok(());
        }
    }
}
