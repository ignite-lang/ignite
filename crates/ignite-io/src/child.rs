//! Child processes (`docs/slice.md` 21.5): the capability `Launcher`, the
//! `ChildProcess` it starts, and the records `Invocation` and `ChildExit`
//! that cross both targets' boundaries as `std.process`'s records do.
//!
//! Slice 17 U1 fixes the surface; every operation is a placeholder (21.13)
//! until U3 implements the watcher.

use std::sync::Arc;

use crate::IoError;

/// The default and test-only bound of children started and not yet
/// waited (`Launcher.new()`, the resource's default `maximum_children`).
pub const DEFAULT_MAXIMUM_CHILDREN: i64 = 16;

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

/// The authority to start child processes, at most `maximum_children`
/// unwaited at once. A cheap clone of an `Arc`.
#[derive(Clone, Debug)]
pub struct Launcher {
    maximum_children: Arc<i64>,
}

impl Launcher {
    /// `Launcher.new()`: a launcher with the default bound, test-only.
    pub fn new() -> Launcher {
        Launcher::with_maximum(DEFAULT_MAXIMUM_CHILDREN)
    }

    /// The resource `Launcher { maximum_children }` at launch.
    pub fn with_maximum(maximum_children: i64) -> Launcher {
        Launcher {
            maximum_children: Arc::new(maximum_children),
        }
    }

    /// The bound this launcher was made with.
    pub fn maximum_children(&self) -> i64 {
        *self.maximum_children
    }

    /// Start the child and wait for it on the calling task.
    pub fn execute(&self, invocation: Invocation) -> Result<ChildExit, IoError> {
        let _ = (self.maximum_children(), invocation);
        unimplemented!("slice 17: Launcher.execute")
    }

    /// Start the child and return at once.
    pub fn start(&self, invocation: Invocation) -> Result<ChildProcess, IoError> {
        let _ = (self.maximum_children(), invocation);
        unimplemented!("slice 17: Launcher.start")
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
    id: Arc<i64>,
}

impl ChildProcess {
    /// The child's process id, which is also its process group id.
    pub fn id(&self) -> i64 {
        let _ = &self.id;
        unimplemented!("slice 17: ChildProcess.id")
    }

    /// Wait for the end; a second wait is the error `child already waited`.
    pub fn wait(&self) -> Result<ChildExit, IoError> {
        let _ = &self.id;
        unimplemented!("slice 17: ChildProcess.wait")
    }

    /// SIGKILL to the child's group while it is unreaped.
    pub fn kill(&self) -> Result<(), IoError> {
        let _ = &self.id;
        unimplemented!("slice 17: ChildProcess.kill")
    }
}
