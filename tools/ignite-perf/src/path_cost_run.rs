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
//! The mode pins itself to two CPUs ([`pin_two_cpus`]), so what it starts (valgrind, strace,
//! the servers, `ignite` and its cargo) uses at most two cores, and a server's workers are as
//! many on every machine. A server runs with an empty environment (`PATH` and `TMPDIR` only)
//! in a fresh directory.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::path_cost::{
    Action, CliOp, Kind, Profile, Request, Scenario, Scope, Server, ServerOp, callgrind_profile,
    strace_profile,
};
use crate::perf::{R, ensure, io_err};

/// How long a run shows no activity before it is at rest.
pub const QUIET: Duration = Duration::from_millis(1500);
/// The longest any wait of a run takes: the listening line, a reply, a rest, a dump.
pub const LIMIT: Duration = Duration::from_secs(600);

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
    pub builds: usize,
    pub native: usize,
    pub instructions: u64,
}

impl Tally {
    pub fn text(&self) -> String {
        format!(
            "{}: {} processes started ({} under valgrind, {} under strace, {} program builds, {} native warm-up runs), {} instructions under valgrind",
            crate::path_cost::METRIC,
            self.valgrind + self.strace + self.builds + self.native,
            self.valgrind,
            self.strace,
            self.builds,
            self.native,
            self.instructions
        )
    }
}

/// A child process, killed on drop.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The measuring state: the compiler, the work directory, the programs built so far.
pub struct Tools {
    cli: PathBuf,
    repo: PathBuf,
    work: PathBuf,
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
        for dir in ["tmp", "cache", "cargo"] {
            fs::create_dir_all(work.join(dir)).map_err(io_err(work.join(dir).display()))?;
        }
        Ok(Tools {
            cli,
            repo: absolute(repo)?,
            work,
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

    /// The scenario's files copied into `<work>/src/<scenario>`, where its commands run.
    fn stage(&self, repo_dir: &Path, scenario: &Scenario) -> R<PathBuf> {
        let dir = self.work.join("src").join(&scenario.name);
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
            .env("XDG_CACHE_HOME", self.work.join("cache"))
            .env("TMPDIR", self.work.join("tmp"))
            .env("CARGO_INCREMENTAL", "0");
    }

    /// The environment of a server: `PATH` and `TMPDIR` only.
    fn server_env(&self, command: &mut Command) {
        command.env_clear();
        if let Some(path) = std::env::var_os("PATH") {
            command.env("PATH", path);
        }
        command.env("TMPDIR", self.work.join("tmp"));
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
            .work
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
            command.arg("--target-dir").arg(self.work.join("cargo"));
        }
        self.cli_env(&mut command);
        let output = command.output().map_err(io_err(self.cli.display()))?;
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
        let dir = self.fresh("callgrind")?;
        let run = dir.join("run");
        fs::create_dir_all(&run).map_err(io_err(run.display()))?;
        let out = dir.join("callgrind.out");
        let prefix = dir.join("vgdb");
        let mut command = Command::new("valgrind");
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
            .arg(binary)
            .args(["--port", "0"])
            .current_dir(&run);
        self.server_env(&mut command);
        let (child, port) = start(command, "valgrind")?;
        self.tally.valgrind += 1;
        let pid = child.0.id();
        let mut dumps = 0;
        let mut dump = |tally: &mut Tally| -> R<Profile> {
            cpu_at_rest(pid)?;
            dumps += 1;
            let profile = dump_part(&prefix, pid, &out, dumps)?;
            tally.instructions += profile.total;
            Ok(profile)
        };
        let mut parts = vec![("start".to_string(), dump(&mut self.tally)?)];
        let mut session = Session::open(port, server)?;
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
        let dir = self.fresh("strace")?;
        let run = dir.join("run");
        fs::create_dir_all(&run).map_err(io_err(run.display()))?;
        let trace = dir.join("trace.txt");
        let mut command = Command::new("strace");
        command
            .args(["-f", "--kill-on-exit", "-o"])
            .arg(&trace)
            .arg(binary)
            .args(["--port", "0"])
            .current_dir(&run);
        self.server_env(&mut command);
        let (child, port) = start(command, "strace")?;
        self.tally.strace += 1;
        let mut mark = 0;
        let mut segment = || -> R<Profile> {
            let text = trace_at_rest(&trace)?;
            let end = text.rfind('\n').map_or(0, |i| i + 1);
            let profile = strace_profile(&text[mark.min(end)..end]);
            mark = end;
            Ok(profile)
        };
        let mut parts = vec![("start".to_string(), segment()?)];
        let mut session = Session::open(port, server)?;
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
        let out = self.fresh(&format!("out-{}-{}", op.name, op.target))?;
        let cargo = self.work.join("cargo");
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
        let mut command = Command::new("valgrind");
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
        let mut command = Command::new("strace");
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
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(io_err(what))?;
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
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
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
/// the `n`th file after `out`.
fn dump_part(prefix: &Path, pid: u32, out: &Path, n: usize) -> R<Profile> {
    let output = Command::new("vgdb")
        .arg(format!("--vgdb-prefix={}", prefix.display()))
        .arg(format!("--pid={pid}"))
        .arg("dump")
        .stdin(Stdio::null())
        .output()
        .map_err(io_err("vgdb"))?;
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
    http: Option<Client>,
    executed: BTreeMap<String, u64>,
}

impl Session {
    /// The set-up requests, then `warm` executions of each operation.
    fn open(port: u16, server: &Server) -> R<Session> {
        let mut session = Session {
            port,
            http: None,
            executed: BTreeMap::new(),
        };
        for (times, request) in &server.setup {
            for _ in 0..*times {
                session.send(request, 0)?;
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
            Action::Connect => {
                let stream =
                    TcpStream::connect(("127.0.0.1", self.port)).map_err(io_err("connect"))?;
                drop(stream);
            }
            Action::Telnet(line) => telnet(self.port, line)?,
        }
        self.executed.insert(op.name.clone(), count + 1);
        Ok(())
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

/// A Telnet session: connect, read until the first prompt ends (`IAC GA`), send `line`, read
/// until the server closes.
fn telnet(port: u16, line: &str) -> R<()> {
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
