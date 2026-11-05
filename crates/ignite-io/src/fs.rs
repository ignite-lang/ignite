//! Files under a root directory (`docs/slice.md` 21.4): the capability
//! `Directory`, its resource launch and its test-only constructor.
//!
//! Slice 17 U1 fixes the surface both targets call; every operation is a
//! placeholder (21.13) until U2 implements the `openat` walk.

use std::sync::Arc;

use crate::IoError;

/// A root directory: every path is resolved under it, never outside. A
/// cheap clone of an `Arc`; several tasks may use one at once.
#[derive(Clone, Debug)]
pub struct Directory {
    root: Arc<str>,
}

impl Directory {
    /// The resource `Directory { path, create, flag }` at launch: the value
    /// of `flag` when it is not empty and given, else `path`; created when
    /// `create`; opened once. A failure ends the program with exit status 2
    /// (`error: directory <path>: <reason>`).
    pub fn launch(path: &str, create: bool, flag: &str) -> Result<Directory, IoError> {
        let given = if flag.is_empty() {
            None
        } else {
            crate::args::flag(flag)?
        };
        Directory::open(given.as_deref().unwrap_or(path), create)
    }

    /// Open `path` as a root, creating it first when `create`.
    pub fn open(path: &str, create: bool) -> Result<Directory, IoError> {
        let _ = (path, create);
        unimplemented!("slice 17: Directory.open")
    }

    /// `Directory.temporary()`: a new empty directory inside the test's
    /// scratch directory (`IGNITE_TEST_SCRATCH`), test-only.
    pub fn temporary() -> Result<Directory, IoError> {
        unimplemented!("slice 17: Directory.temporary")
    }

    /// The file's bytes, at most 16 MiB.
    pub fn read_file(&self, path: impl AsRef<str>) -> Result<Vec<u8>, IoError> {
        let _ = (&self.root, path.as_ref());
        unimplemented!("slice 17: Directory.read_file")
    }

    /// Create or replace the file atomically.
    pub fn write_file(&self, path: impl AsRef<str>, data: impl AsRef<[u8]>) -> Result<(), IoError> {
        let _ = (&self.root, path.as_ref(), data.as_ref());
        unimplemented!("slice 17: Directory.write_file")
    }

    /// The entries' names in byte order; a directory's ends with `/`.
    pub fn read_directory(&self, path: impl AsRef<str>) -> Result<Vec<String>, IoError> {
        let _ = (&self.root, path.as_ref());
        unimplemented!("slice 17: Directory.read_directory")
    }

    /// Create the directory and its missing parents.
    pub fn create_directory(&self, path: impl AsRef<str>) -> Result<(), IoError> {
        let _ = (&self.root, path.as_ref());
        unimplemented!("slice 17: Directory.create_directory")
    }

    /// Remove a file, a symbolic link or an empty directory.
    pub fn remove(&self, path: impl AsRef<str>) -> Result<(), IoError> {
        let _ = (&self.root, path.as_ref());
        unimplemented!("slice 17: Directory.remove")
    }

    /// Remove a directory and everything under it, following no link.
    pub fn remove_tree(&self, path: impl AsRef<str>) -> Result<(), IoError> {
        let _ = (&self.root, path.as_ref());
        unimplemented!("slice 17: Directory.remove_tree")
    }

    /// The root's canonical absolute path as resolved at launch.
    pub fn absolute_path(&self) -> Result<String, IoError> {
        let _ = &self.root;
        unimplemented!("slice 17: Directory.absolute_path")
    }
}
