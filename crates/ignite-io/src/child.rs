//! Child processes (`docs/slice.md` 21.5): the capability `Launcher`, the
//! `ChildProcess` it starts, and the records `Invocation` and `ChildExit`
//! that cross both targets' boundaries as `std.process`'s records do.
//!
//! Every child is started by the thread that watches it ([`watch`]): a
//! thread named `child` for a launcher's child, the pool thread for a test
//! runner's. `PR_SET_PDEATHSIG` fires when the starting thread ends, so the
//! watcher lives as long as the child. The watcher reads both pipes with
//! `poll`, enforces the deadline, sees the end with `waitid(WEXITED |
//! WNOWAIT)` (the child stays a zombie, so its pid and group id are not
//! reused), kills the group, reaps the child and reads what is left in the
//! pipes for at most [`GRACE`]. A launcher's children run in their own
//! process group; the test runner's stay in the runner's, so Ctrl-C reaches
//! them. Live groups get SIGKILL from an [`crate::at_exit`] hook when the
//! program ends in order.

use std::io::{ErrorKind, Read};
use std::os::fd::AsFd;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Once, PoisonError, Weak};
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::fcntl::{FcntlArg, OFlag, fcntl};
use nix::poll::{PollFd, PollFlags, poll};
use nix::sys::signal::{SigHandler, Signal, kill, killpg};
use nix::sys::wait::{Id, WaitPidFlag, WaitStatus, waitid};
use nix::unistd::Pid;

use crate::IoError;

/// The default and test-only bound of children started and not yet
/// waited (`Launcher.new()`, the resource's default `maximum_children`).
pub const DEFAULT_MAXIMUM_CHILDREN: i64 = 16;

/// The largest `output_limit`, per stream.
pub const MAXIMUM_OUTPUT_LIMIT: i64 = 16 << 20;

/// How long the watcher reads the pipes after it reaped the child: a
/// grandchild that left the group and holds a pipe delays the result by
/// at most this.
pub const GRACE: Duration = Duration::from_millis(100);

/// The longest wait between two looks at a running child.
const IDLE: Duration = Duration::from_millis(50);

/// Bytes read from one pipe per turn, so a busy stream cannot starve the
/// other or postpone the deadline.
const TURN: usize = 64 << 10;

/// `ChildExit.ending` values.
const EXITED: i64 = 0;
const SIGNALLED: i64 = 1;
const TIMED_OUT: i64 = 2;
const KILLED: i64 = 3;

/// What `Launcher::execute` and `Launcher::start` run: the fields of
/// `std.process.Invocation`; `timeout` in nanoseconds, `output_limit` in
/// bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invocation {
    pub program: String,
    pub arguments: Vec<String>,
    pub directory: String,
    pub environment: Vec<String>,
    pub inherit_environment: bool,
    pub timeout: i64,
    pub output_limit: i64,
}

/// How a child ended: the fields of `std.process.ChildExit`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildExit {
    pub ending: i64,
    pub code: i64,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub dropped: i64,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The authority to start child processes, at most `maximum_children`
/// unwaited at once. A cheap clone of an `Arc`.
#[derive(Clone, Debug)]
pub struct Launcher {
    slots: Arc<Slots>,
}

/// The children of one launcher that were started and not yet waited.
#[derive(Debug)]
struct Slots {
    maximum: i64,
    taken: Mutex<i64>,
}

impl Slots {
    fn take(&self) -> Result<(), IoError> {
        let mut taken = lock(&self.taken);
        if *taken >= self.maximum {
            return Err(IoError::new(format!(
                "too many children ({})",
                self.maximum
            )));
        }
        *taken += 1;
        Ok(())
    }

    fn give_back(&self) {
        let mut taken = lock(&self.taken);
        *taken = (*taken - 1).max(0);
    }
}

impl Launcher {
    /// `Launcher.new()`: a launcher with the default bound, test-only.
    pub fn new() -> Launcher {
        Launcher::with_maximum(DEFAULT_MAXIMUM_CHILDREN)
    }

    /// The resource `Launcher { maximum_children }` at launch.
    pub fn with_maximum(maximum_children: i64) -> Launcher {
        Launcher {
            slots: Arc::new(Slots {
                maximum: maximum_children,
                taken: Mutex::new(0),
            }),
        }
    }

    /// The bound this launcher was made with.
    pub fn maximum_children(&self) -> i64 {
        self.slots.maximum
    }

    /// Start the child and wait for it on the calling task. The child
    /// counts against the bound while it runs.
    pub fn execute(&self, invocation: Invocation) -> Result<ChildExit, IoError> {
        self.start(invocation)?.wait()
    }

    /// Start the child and return at once. It counts against the bound
    /// until its `wait` returns.
    pub fn start(&self, invocation: Invocation) -> Result<ChildProcess, IoError> {
        let prepared = prepare(&invocation)?;
        self.slots.take()?;
        match launch(prepared) {
            Ok((id, watched)) => Ok(ChildProcess {
                inner: Arc::new(Started {
                    id,
                    watched,
                    slots: Arc::clone(&self.slots),
                }),
            }),
            Err(error) => {
                self.slots.give_back();
                Err(error)
            }
        }
    }
}

impl Default for Launcher {
    fn default() -> Launcher {
        Launcher::new()
    }
}

/// A started child. A cheap clone of an `Arc`.
#[derive(Clone, Debug)]
pub struct ChildProcess {
    inner: Arc<Started>,
}

#[derive(Debug)]
struct Started {
    id: i64,
    watched: Arc<Watched>,
    slots: Arc<Slots>,
}

impl ChildProcess {
    /// The child's process id, which is also its process group id.
    pub fn id(&self) -> i64 {
        self.inner.id
    }

    /// Wait for the end; a second wait is the error `child already waited`.
    /// The first frees the child's slot of its launcher.
    pub fn wait(&self) -> Result<ChildExit, IoError> {
        let watched = &self.inner.watched;
        let mut state = lock(&watched.state);
        if state.waited {
            return Err(IoError::new("child already waited"));
        }
        state.waited = true;
        while state.exit.is_none() {
            state = watched
                .done
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        let exit = state.exit.take();
        drop(state);
        self.inner.slots.give_back();
        exit.ok_or_else(|| IoError::new("child already waited"))
    }

    /// SIGKILL to the child's group while it is unreaped; `Ok` and nothing
    /// sent once it was reaped.
    pub fn kill(&self) -> Result<(), IoError> {
        self.inner.watched.kill();
        Ok(())
    }
}

/// What a watcher and the handles of its child share. `kill` signals only
/// while `reaped` is false, under the lock the watcher reaps under, so it
/// never reaches a reused pid or group id.
#[derive(Debug)]
pub(crate) struct Watched {
    state: Mutex<State>,
    done: Condvar,
}

#[derive(Debug, Default)]
struct State {
    pid: i32,
    own_group: bool,
    started: bool,
    reaped: bool,
    ended: bool,
    killed: bool,
    timed_out: bool,
    waited: bool,
    exit: Option<ChildExit>,
}

impl Watched {
    pub(crate) fn new(own_group: bool) -> Watched {
        Watched {
            state: Mutex::new(State {
                own_group,
                ..State::default()
            }),
            done: Condvar::new(),
        }
    }

    /// SIGKILL to the child (its group when it has its own) while it is
    /// unreaped. A child that had not ended by then reports `Killed`.
    fn kill(&self) {
        let mut state = lock(&self.state);
        if !state.started || state.reaped {
            return;
        }
        if !state.ended && ended(&mut state) {
            state.ended = true;
        }
        signal(&state);
        if !state.ended && !state.timed_out {
            state.killed = true;
        }
    }

    /// SIGKILL to the group of a child that runs in its own, at exit.
    fn kill_group(&self) {
        let state = lock(&self.state);
        if state.started && !state.reaped && state.own_group {
            signal(&state);
        }
    }
}

/// SIGKILL to the child of `state`: its group, or the child alone when it
/// shares the starter's group. Nothing once it was reaped, by the watcher
/// or by anyone else ([`ended`]): its pid and group id may be reused.
fn signal(state: &State) {
    if state.reaped {
        return;
    }
    let pid = Pid::from_raw(state.pid);
    if state.own_group {
        let _ = killpg(pid, Signal::SIGKILL);
    } else {
        let _ = kill(pid, Signal::SIGKILL);
    }
}

/// Whether the unreaped child of `state` has ended; it stays a zombie.
/// An error of `waitid` (ECHILD: the child was reaped by someone else, as
/// the kernel does while SIGCHLD is ignored) also ends it and marks it
/// reaped, so nothing signals its pid or group again.
fn ended(state: &mut State) -> bool {
    loop {
        match waitid(
            Id::Pid(Pid::from_raw(state.pid)),
            WaitPidFlag::WEXITED | WaitPidFlag::WNOHANG | WaitPidFlag::WNOWAIT,
        ) {
            Ok(WaitStatus::StillAlive) => return false,
            Err(Errno::EINTR) => continue,
            Ok(_) => return true,
            Err(_) => {
                state.reaped = true;
                return true;
            }
        }
    }
}

static SIGCHLD_DEFAULT: Once = Once::new();

/// Reset SIGCHLD to its default action, once per process. A disposition
/// inherited as ignored (a parent's `trap '' CHLD` survives exec) makes the
/// kernel reap every child itself, so no status could be read
/// (`docs/slice.md` 21.5). Called at program start by [`crate::drain`] and
/// before every spawn, which covers the test runner.
pub(crate) fn default_sigchld() {
    SIGCHLD_DEFAULT.call_once(|| {
        // SAFETY: the default action installs no handler code.
        let _ = unsafe { nix::sys::signal::signal(Signal::SIGCHLD, SigHandler::SigDfl) };
    });
}

/// The live children with their own group, for the exit hook.
static LIVE: Mutex<Vec<Weak<Watched>>> = Mutex::new(Vec::new());
static HOOK: Once = Once::new();

fn register(watched: &Arc<Watched>) {
    HOOK.call_once(|| crate::at_exit(kill_live_groups));
    let mut live = lock(&LIVE);
    live.retain(|weak| weak.upgrade().is_some_and(|w| !lock(&w.state).reaped));
    live.push(Arc::downgrade(watched));
}

/// The exit hook: SIGKILL to every live child's group.
fn kill_live_groups() {
    let live: Vec<Arc<Watched>> = lock(&LIVE).drain(..).filter_map(|w| w.upgrade()).collect();
    for watched in live {
        watched.kill_group();
    }
}

/// A checked invocation, ready to start.
struct Prepared {
    command: Command,
    timeout: Duration,
    limit: usize,
}

fn prepare(invocation: &Invocation) -> Result<Prepared, IoError> {
    if invocation.timeout <= 0 {
        return Err(IoError::new("timeout must be positive"));
    }
    if invocation.output_limit > MAXIMUM_OUTPUT_LIMIT {
        return Err(IoError::new("output_limit must be at most 16 MiB"));
    }
    if invocation.output_limit < 0 {
        return Err(IoError::new("output_limit must not be negative"));
    }
    let directory = &invocation.directory;
    if !directory.is_empty() {
        if !directory.starts_with('/') {
            return Err(IoError::new(format!("directory {directory}: not absolute")));
        }
        if let Err(error) = enterable(directory) {
            return Err(IoError::new(format!("directory {directory}: {error}")));
        }
    }
    let mut environment = Vec::with_capacity(invocation.environment.len());
    for entry in &invocation.environment {
        match entry.split_once('=') {
            Some((name, value)) if !name.is_empty() => environment.push((name, value)),
            _ => {
                return Err(IoError::new(format!(
                    "environment entry {entry}: not NAME=value"
                )));
            }
        }
    }
    let path = environment
        .iter()
        .rev()
        .find(|(name, _)| *name == "PATH")
        .map(|(_, value)| std::ffi::OsString::from(value))
        .or_else(|| {
            invocation
                .inherit_environment
                .then(|| std::env::var_os("PATH"))
                .flatten()
        });
    let program = resolve(&invocation.program, path.as_deref())?;
    let mut command = Command::new(program);
    command.args(&invocation.arguments);
    if !invocation.inherit_environment {
        command.env_clear();
    }
    for (name, value) in environment {
        command.env(name, value);
    }
    if !directory.is_empty() {
        command.current_dir(directory);
    }
    Ok(Prepared {
        command,
        timeout: Duration::from_nanos(invocation.timeout as u64),
        limit: invocation.output_limit as usize,
    })
}

/// The program to execute: an absolute path as it is, a bare name looked
/// up in `path`.
fn resolve(program: &str, path: Option<&std::ffi::OsStr>) -> Result<PathBuf, IoError> {
    if program.starts_with('/') {
        return Ok(PathBuf::from(program));
    }
    if program.contains('/') {
        return Err(IoError::new(format!(
            "program {program}: a relative path with /"
        )));
    }
    let found = (!program.is_empty())
        .then(|| path.map(std::env::split_paths))
        .flatten()
        .into_iter()
        .flatten()
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(program))
        .find(|candidate| executable(candidate));
    found.ok_or_else(|| IoError::new(format!("program not found: {program}")))
}

/// Whether the child can change into `directory`: it exists, is a
/// directory and is searchable; otherwise the OS reason. Checked before the
/// spawn, so an error of `chdir` in the child is not taken for one of the
/// program.
fn enterable(directory: &str) -> Result<(), String> {
    let found = std::fs::metadata(directory).map_err(|error| reason(&error))?;
    if !found.is_dir() {
        return Err(Errno::ENOTDIR.desc().to_string());
    }
    nix::unistd::access(directory, nix::unistd::AccessFlags::X_OK)
        .map_err(|errno| errno.desc().to_string())
}

/// The OS reason of a `std::io::Error`, in the words of [`Errno::desc`].
fn reason(error: &std::io::Error) -> String {
    match error.raw_os_error() {
        Some(code) => Errno::from_raw(code).desc().to_string(),
        None => error.to_string(),
    }
}

fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Start `prepared` on a new watcher thread named `child`; the child's pid
/// once it runs.
fn launch(prepared: Prepared) -> Result<(i64, Arc<Watched>), IoError> {
    let watched = Arc::new(Watched::new(true));
    register(&watched);
    let program = prepared
        .command
        .get_program()
        .to_string_lossy()
        .into_owned();
    let directory = prepared
        .command
        .get_current_dir()
        .map(|d| d.to_string_lossy().into_owned());
    let (sender, receiver) = mpsc::channel::<Result<u32, std::io::Error>>();
    let shared = Arc::clone(&watched);
    std::thread::Builder::new()
        .name("child".to_string())
        .spawn(move || {
            let Prepared {
                command,
                timeout,
                limit,
            } = prepared;
            let finished = watch(command, &shared, timeout, limit, |started| {
                let _ = sender.send(started);
            });
            if let Ok(finished) = finished {
                let exit = finished.exit();
                let mut state = lock(&shared.state);
                state.exit = Some(exit);
                shared.done.notify_all();
            }
        })
        .map_err(|error| IoError::new(format!("program {program}: {error}")))?;
    match receiver.recv() {
        Ok(Ok(pid)) => Ok((i64::from(pid), watched)),
        Ok(Err(error)) => Err(IoError::new(refusal(
            &program,
            directory.as_deref(),
            &error,
        ))),
        Err(_) => Err(IoError::new(format!("program {program}: not started"))),
    }
}

/// The message of a failed spawn. `prepare` checked the
/// directory, but it can be gone by the time the child changes into it;
/// a program that exists can still fail with ENOENT (a script whose
/// interpreter is missing). Only a program path that does not exist is
/// `program not found` (`docs/slice.md` 21.5).
fn refusal(program: &str, directory: Option<&str>, error: &std::io::Error) -> String {
    if let Some(directory) = directory
        && let Err(missing) = enterable(directory)
    {
        return format!("directory {directory}: {missing}");
    }
    if error.kind() == ErrorKind::NotFound && std::fs::metadata(program).is_err() {
        return format!("program not found: {program}");
    }
    format!("program {program}: {}", reason(error))
}

/// How a watched child ended.
#[derive(Debug)]
pub(crate) struct Finished {
    /// `None` when reaping failed.
    pub(crate) status: Option<ExitStatus>,
    pub(crate) timed_out: bool,
    pub(crate) killed: bool,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) dropped: i64,
}

impl Finished {
    fn exit(self) -> ChildExit {
        let (ending, code) = if self.killed {
            (KILLED, -1)
        } else if self.timed_out {
            (TIMED_OUT, -1)
        } else {
            match self.status {
                Some(status) => match (status.code(), status.signal()) {
                    (Some(code), _) => (EXITED, i64::from(code)),
                    (None, Some(signal)) => (SIGNALLED, i64::from(signal)),
                    (None, None) => (SIGNALLED, -1),
                },
                None => (SIGNALLED, -1),
            }
        };
        ChildExit {
            ending,
            code,
            stdout: self.stdout,
            stderr: self.stderr,
            dropped: self.dropped,
        }
    }
}

/// Start `command` on the calling thread and watch it to its end: stdin
/// the null device, stdout and stderr captured up to `limit` bytes each
/// (the rest counted), its own process group when `watched` says so, an
/// empty signal mask, `PR_SET_PDEATHSIG` SIGKILL and no descriptor from 3
/// up. After `timeout` the child (its group) gets SIGKILL. `started`
/// learns the pid, or the error when the child did not start, which is
/// then also the result.
pub(crate) fn watch(
    mut command: Command,
    watched: &Watched,
    timeout: Duration,
    limit: usize,
    started: impl FnOnce(Result<u32, std::io::Error>),
) -> Result<Finished, std::io::Error> {
    let own_group = lock(&watched.state).own_group;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if own_group {
        command.process_group(0);
    }
    confine(&mut command);
    default_sigchld();
    let mut state = lock(&watched.state);
    let spawned = command.spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            drop(state);
            let copy = match error.raw_os_error() {
                Some(code) => std::io::Error::from_raw_os_error(code),
                None => std::io::Error::new(error.kind(), error.to_string()),
            };
            started(Err(copy));
            return Err(error);
        }
    };
    state.pid = child.id() as i32;
    state.started = true;
    drop(state);
    started(Ok(child.id()));
    Ok(supervise(&mut child, watched, timeout, limit))
}

/// Install the `pre_exec` hook: an empty signal mask (the drain blocks
/// SIGTERM in every thread of the program, and `Command` keeps the mask of
/// the thread that forks: row 13 of `docs/slice.md` 21.0), `PR_SET_PDEATHSIG`
/// SIGKILL (and exit at once when the parent already died), then every
/// descriptor from 3 up marked close-on-exec with `close_range`, so a
/// descriptor the program inherited without close-on-exec does not reach
/// the child. Marking instead of closing keeps the pipe `Command` reports
/// an exec failure on.
fn confine(command: &mut Command) {
    let parent = nix::unistd::getpid();
    // SAFETY: the hook runs in the forked child before exec and calls only
    // async-signal-safe functions (`sigemptyset`, `sigprocmask`, `prctl`,
    // `getppid`, `close_range`, `fcntl`, `_exit`); it allocates nothing and
    // takes no lock.
    unsafe {
        command.pre_exec(move || {
            let mut empty: nix::libc::sigset_t = std::mem::zeroed();
            if nix::libc::sigemptyset(&mut empty) != 0
                || nix::libc::sigprocmask(nix::libc::SIG_SETMASK, &empty, std::ptr::null_mut()) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            if nix::libc::prctl(
                nix::libc::PR_SET_PDEATHSIG,
                nix::libc::SIGKILL as nix::libc::c_ulong,
                0,
                0,
                0,
            ) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            if nix::libc::getppid() != parent.as_raw() {
                nix::libc::_exit(1);
            }
            let marked = nix::libc::syscall(
                nix::libc::SYS_close_range,
                3 as nix::libc::c_uint,
                nix::libc::c_uint::MAX,
                nix::libc::CLOSE_RANGE_CLOEXEC,
            );
            if marked != 0 {
                let open = nix::libc::sysconf(nix::libc::_SC_OPEN_MAX);
                let last = if open > 3 { open } else { 1024 };
                for fd in 3..last as nix::libc::c_int {
                    nix::libc::fcntl(fd, nix::libc::F_SETFD, nix::libc::FD_CLOEXEC);
                }
            }
            Ok(())
        });
    }
}

/// One captured stream: what is kept, up to the limit, and whether the
/// pipe is still open.
struct Capture<R> {
    pipe: Option<R>,
    kept: Vec<u8>,
}

impl<R: Read + AsFd> Capture<R> {
    fn new(pipe: Option<R>) -> Capture<R> {
        let pipe = pipe.filter(|p| nonblocking(p).is_ok());
        Capture {
            pipe,
            kept: Vec::new(),
        }
    }

    /// Read what the pipe holds, at most [`TURN`] bytes; bytes past
    /// `limit` are counted in `dropped`. Whether anything was read.
    fn drain(&mut self, limit: usize, dropped: &mut i64) -> bool {
        let Some(pipe) = self.pipe.as_mut() else {
            return false;
        };
        let mut buffer = [0; 8192];
        let mut read = 0;
        while read < TURN {
            match pipe.read(&mut buffer) {
                Ok(0) => {
                    self.pipe = None;
                    break;
                }
                Ok(n) => {
                    read += n;
                    let room = limit.saturating_sub(self.kept.len()).min(n);
                    self.kept.extend_from_slice(&buffer[..room]);
                    *dropped += (n - room) as i64;
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(_) => {
                    self.pipe = None;
                    break;
                }
            }
        }
        read > 0
    }
}

fn nonblocking(stream: &impl AsFd) -> nix::Result<()> {
    let flags = OFlag::from_bits_truncate(fcntl(stream, FcntlArg::F_GETFL)?);
    fcntl(stream, FcntlArg::F_SETFL(flags | OFlag::O_NONBLOCK)).map(|_| ())
}

/// Wait up to `wait` for either pipe to become readable or close.
fn wait_for_output(out: &Capture<ChildStdout>, err: &Capture<ChildStderr>, wait: Duration) {
    let mut fds = Vec::with_capacity(2);
    if let Some(pipe) = &out.pipe {
        fds.push(PollFd::new(pipe.as_fd(), PollFlags::POLLIN));
    }
    if let Some(pipe) = &err.pipe {
        fds.push(PollFd::new(pipe.as_fd(), PollFlags::POLLIN));
    }
    let millis = wait.as_millis().clamp(1, IDLE.as_millis()) as u16;
    if fds.is_empty() {
        std::thread::sleep(Duration::from_millis(u64::from(millis)));
        return;
    }
    let _ = poll(&mut fds, millis);
}

/// The watcher's loop over the running `child`.
fn supervise(child: &mut Child, watched: &Watched, timeout: Duration, limit: usize) -> Finished {
    let pid = child.id() as i32;
    let mut out = Capture::new(child.stdout.take());
    let mut err = Capture::new(child.stderr.take());
    let mut dropped = 0i64;
    let mut deadline = Instant::now().checked_add(timeout);
    let mut idle = Duration::from_millis(1);
    loop {
        let fresh = out.drain(limit, &mut dropped) | err.drain(limit, &mut dropped);
        if ended(&mut lock(&watched.state)) {
            break;
        }
        let now = Instant::now();
        if deadline.is_some_and(|d| now >= d) {
            deadline = None;
            let mut state = lock(&watched.state);
            if !state.killed {
                state.timed_out = true;
                signal(&state);
            }
        }
        idle = if fresh {
            Duration::from_millis(1)
        } else {
            (idle * 2).min(IDLE)
        };
        let wait = deadline.map_or(idle, |d| idle.min(d.saturating_duration_since(now)));
        wait_for_output(&out, &err, wait);
    }
    let mut state = lock(&watched.state);
    state.ended = true;
    let gone = state.reaped;
    if state.own_group && !gone {
        let _ = killpg(Pid::from_raw(pid), Signal::SIGKILL);
    }
    let status = if gone { None } else { child.wait().ok() };
    state.reaped = true;
    let (timed_out, killed) = (state.timed_out, state.killed);
    drop(state);
    let grace = Instant::now() + GRACE;
    loop {
        out.drain(limit, &mut dropped);
        err.drain(limit, &mut dropped);
        let now = Instant::now();
        if (out.pipe.is_none() && err.pipe.is_none()) || now >= grace {
            break;
        }
        wait_for_output(&out, &err, grace - now);
    }
    Finished {
        status,
        timed_out,
        killed,
        stdout: out.kept,
        stderr: err.kept,
        dropped,
    }
}
