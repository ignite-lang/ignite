//! Child processes (`docs/slice.md` 21.5): the C ABI over
//! `ignite_io::{Launcher, ChildProcess}`. A launcher and a started child
//! are handles of the one table in `crate::net`; `Invocation` and
//! `ChildExit` cross as the layouts of `std.process`'s records.

use ignite_io::{ChildExit, ChildProcess, Invocation, IoError, Launcher};

use crate::net::{Handle, capability, register, release_waited_child, waited_child_id};
use crate::text::texts;
use crate::{IgniteIoError, IgniteList, bytes, get, status};

/// `std.process.Invocation`: its fields in declaration order
/// (`docs/llvm.md` section 3); `timeout` in nanoseconds, `output_limit` in
/// bytes, `input` the bytes written to the child's stdin.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IgniteInvocation {
    pub program: IgniteList,
    pub arguments: IgniteList,
    pub directory: IgniteList,
    pub environment: IgniteList,
    pub inherit_environment: i8,
    pub timeout: i64,
    pub output_limit: i64,
    pub input: IgniteList,
}

/// `std.process.ChildExit`: its fields in declaration order.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IgniteChildExit {
    pub ending: i64,
    pub code: i64,
    pub stdout: IgniteList,
    pub stderr: IgniteList,
    pub dropped: i64,
}

fn text(list: &IgniteList) -> String {
    String::from_utf8_lossy(bytes(list)).into_owned()
}

impl IgniteInvocation {
    /// The `ignite-io` invocation of the record at `value`.
    fn read(value: *const IgniteInvocation) -> Result<Invocation, IoError> {
        let record = get(value).ok_or_else(|| IoError::new("no invocation"))?;
        let list = |l: &IgniteList| texts(l).iter().map(text).collect::<Vec<String>>();
        Ok(Invocation {
            program: text(&record.program),
            arguments: list(&record.arguments),
            directory: text(&record.directory),
            environment: list(&record.environment),
            inherit_environment: record.inherit_environment != 0,
            timeout: record.timeout,
            output_limit: record.output_limit,
            input: bytes(&record.input).to_vec(),
        })
    }
}

impl IgniteChildExit {
    /// `exit` with its output copied into the calling task's arena.
    fn of(exit: &ChildExit) -> IgniteChildExit {
        IgniteChildExit {
            ending: exit.ending,
            code: exit.code,
            stdout: IgniteList::from_bytes(&exit.stdout),
            stderr: IgniteList::from_bytes(&exit.stderr),
            dropped: exit.dropped,
        }
    }
}

fn launcher(handle: i64) -> Result<Launcher, IoError> {
    capability(handle, "launcher", |found| match found {
        Handle::Launcher(launcher) => Some(launcher),
        _ => None,
    })
}

fn child(handle: i64) -> Result<ChildProcess, IoError> {
    capability(handle, "child process", |found| match found {
        Handle::Child(child) => Some(child),
        _ => None,
    })
}

/// Whether `error`, from looking `handle` up, says the handle was released:
/// a child handle is released only by its `wait` (`docs/slice.md` 21.5),
/// so it names a waited child, also one whose slot a later child reused.
fn released(handle: i64, error: &IoError) -> bool {
    waited_child_id(handle).is_some() || error.message.ends_with("was released")
}

/// `i64 launcher_new(i64 maximum_children)`: `Launcher.new()` (16) and the
/// resource at launch.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_launcher_new(maximum_children: i64) -> i64 {
    crate::entry(move || register(Handle::Launcher(Launcher::with_maximum(maximum_children))))
}

/// `i32 launcher_execute(i64 l, Invocation* invocation, ChildExit* out,
/// IoError* err)`: start the child and wait for it on the calling task.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_launcher_execute(
    handle: i64,
    invocation: *const IgniteInvocation,
    out: *mut IgniteChildExit,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let ran = launcher(handle)
            .and_then(|l| l.execute(IgniteInvocation::read(invocation)?))
            .map(|exit| IgniteChildExit::of(&exit));
        status(ran, out, err)
    })
}

/// `i32 launcher_start(i64 l, Invocation* invocation, i64* out, IoError*
/// err)`: start the child; its handle.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_launcher_start(
    handle: i64,
    invocation: *const IgniteInvocation,
    out: *mut i64,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let started = launcher(handle)
            .and_then(|l| l.start(IgniteInvocation::read(invocation)?))
            .map(|child| register(Handle::Child(child)));
        status(started, out, err)
    })
}

/// `i64 child_id(i64 c)`: the child's process id, also once it was waited
/// and its handle released, also after a later handle reused its slot; 0
/// for a handle that names no child.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_child_id(handle: i64) -> i64 {
    crate::entry(move || {
        child(handle).map_or_else(|_| waited_child_id(handle).unwrap_or(0), |c| c.id())
    })
}

/// `i32 child_wait(i64 c, ChildExit* out, IoError* err)`: the first wait
/// frees the handle's slot (`docs/slice.md` 21.5); a later one is `child
/// already waited`, as on the Rust target.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_child_wait(
    handle: i64,
    out: *mut IgniteChildExit,
    err: *mut IgniteIoError,
) -> i32 {
    crate::entry(move || {
        let waited = match child(handle) {
            Ok(c) => c.wait().inspect(|_| {
                release_waited_child(handle, c.id());
            }),
            Err(error) if released(handle, &error) => Err(IoError::new("child already waited")),
            Err(error) => Err(error),
        };
        status(waited.map(|exit| IgniteChildExit::of(&exit)), out, err)
    })
}

/// `i32 child_kill(i64 c, IoError* err)`: `Ok` and nothing sent once the
/// child was waited.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_child_kill(handle: i64, err: *mut IgniteIoError) -> i32 {
    crate::entry(move || {
        let killed = match child(handle) {
            Ok(c) => c.kill(),
            Err(error) if released(handle, &error) => Ok(()),
            Err(error) => Err(error),
        };
        status(killed, std::ptr::null_mut(), err)
    })
}
