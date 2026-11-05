//! Files under a root directory (`docs/slice.md` 21.4): the capability
//! `Directory`, its resource launch and its test-only constructor.
//!
//! Confinement is lexical first (an absolute path, a `..` component or a
//! NUL is refused before any IO) and then structural: every path is walked
//! from the root's descriptor one component at a time with `openat` and
//! `O_NOFOLLOW`, so no step follows a symbolic link and a component
//! swapped for one between two steps is refused at the step that opens it
//! (D4, row 4). Every symbolic link is refused, inside links too (21.15).

use std::ffi::{CStr, CString};
use std::io::Read;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use nix::dir::{Dir, Type};
use nix::errno::Errno;
use nix::fcntl::{AtFlags, OFlag, openat, renameat};
use nix::sys::stat::{FileStat, Mode, SFlag, fstat, fstatat, mkdirat};
use nix::unistd::{UnlinkatFlags, unlinkat};

use crate::IoError;

/// The largest file `read_file` returns.
const READ_LIMIT: u64 = 16 * 1024 * 1024;

/// The most entries `read_directory` lists.
const ENTRY_LIMIT: usize = 65_536;

/// The deepest tree `remove_tree` removes, the removed directory being
/// level 1.
const DEPTH_LIMIT: usize = 256;

/// A root directory: every path is resolved under it, never outside. A
/// cheap clone of an `Arc`; several tasks may use one at once.
#[derive(Clone, Debug)]
pub struct Directory {
    root: Arc<Root>,
}

#[derive(Debug)]
struct Root {
    descriptor: OwnedFd,
    path: String,
}

/// A path checked lexically: its components without empty ones and `.`.
struct Checked<'a> {
    given: &'a str,
    components: Vec<&'a str>,
}

impl Checked<'_> {
    fn error(&self, reason: impl std::fmt::Display) -> IoError {
        IoError::new(format!("path {}: {reason}", self.given))
    }

    fn os(&self, errno: Errno) -> IoError {
        self.error(errno.desc())
    }

    /// The path up to and including component `index`.
    fn prefix(&self, index: usize) -> String {
        self.components[..=index].join("/")
    }

    fn link(&self, index: usize) -> IoError {
        self.error(format_args!("symbolic link {}", self.prefix(index)))
    }
}

fn check(given: &str) -> Result<Checked<'_>, IoError> {
    let refuse = |reason: &str| IoError::new(format!("path {given}: {reason}"));
    if given.contains('\0') {
        return Err(refuse("contains NUL"));
    }
    if given.starts_with('/') {
        return Err(refuse("absolute"));
    }
    let components: Vec<&str> = given
        .split('/')
        .filter(|c| !c.is_empty() && *c != ".")
        .collect();
    if components.contains(&"..") {
        return Err(refuse("leaves its root"));
    }
    Ok(Checked { given, components })
}

fn is_link(stat: &FileStat) -> bool {
    SFlag::from_bits_truncate(stat.st_mode) & SFlag::S_IFMT == SFlag::S_IFLNK
}

fn is_directory(stat: &FileStat) -> bool {
    SFlag::from_bits_truncate(stat.st_mode) & SFlag::S_IFMT == SFlag::S_IFDIR
}

fn is_regular(stat: &FileStat) -> bool {
    SFlag::from_bits_truncate(stat.st_mode) & SFlag::S_IFMT == SFlag::S_IFREG
}

const DIRECTORY_FLAGS: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);

/// Open the directory `name` under `parent` without following a link.
/// `ELOOP` means `name` was a link when it was opened (`O_NOFOLLOW`);
/// `ENOTDIR` is checked with `fstatat`, because it is also what a plain
/// file gives.
fn open_directory(
    parent: BorrowedFd<'_>,
    name: &str,
    path: &Checked<'_>,
    index: usize,
) -> Result<OwnedFd, IoError> {
    match openat(parent, name, DIRECTORY_FLAGS, Mode::empty()) {
        Ok(fd) => Ok(fd),
        Err(Errno::ELOOP) => Err(path.link(index)),
        Err(Errno::ENOTDIR) => match fstatat(parent, name, AtFlags::AT_SYMLINK_NOFOLLOW) {
            Ok(stat) if is_link(&stat) => Err(path.link(index)),
            _ => Err(path.os(Errno::ENOTDIR)),
        },
        Err(errno) => Err(path.os(errno)),
    }
}

/// A new descriptor of the directory `parent` itself, with an offset of its
/// own, so that listing it does not disturb another task's listing.
fn reopen(parent: BorrowedFd<'_>, path: &Checked<'_>) -> Result<OwnedFd, IoError> {
    openat(parent, ".", DIRECTORY_FLAGS, Mode::empty()).map_err(|e| path.os(e))
}

/// The names in `directory` without `.` and `..`, as the raw bytes the OS
/// gave, each with whether it is a directory (never following a link), in
/// byte order; at most `limit` when one is given.
fn entries(
    directory: OwnedFd,
    path: &Checked<'_>,
    limit: Option<usize>,
) -> Result<Vec<(CString, bool)>, IoError> {
    let mut dir = Dir::from_fd(directory).map_err(|e| path.os(e))?;
    let mut read = Vec::new();
    for entry in dir.iter() {
        let entry = entry.map_err(|e| path.os(e))?;
        let name = entry.file_name();
        if name.to_bytes() == b"." || name.to_bytes() == b".." {
            continue;
        }
        if limit == Some(read.len()) {
            return Err(path.error(format_args!("more than {ENTRY_LIMIT} entries")));
        }
        read.push((name.to_owned(), entry.file_type()));
    }
    let mut found: Vec<(CString, bool)> = read
        .into_iter()
        .map(|(name, kind)| {
            let directory = match kind {
                Some(Type::Directory) => true,
                Some(_) => false,
                None => fstatat(dir.as_fd(), name.as_c_str(), AtFlags::AT_SYMLINK_NOFOLLOW)
                    .is_ok_and(|stat| is_directory(&stat)),
            };
            (name, directory)
        })
        .collect();
    found.sort();
    Ok(found)
}

/// The OS reason of a `std::io::Error`, in the words of [`Errno::desc`].
fn reason(error: &std::io::Error) -> String {
    match error.raw_os_error() {
        Some(code) => Errno::from_raw(code).desc().to_string(),
        None => error.to_string(),
    }
}

static WRITES: AtomicU64 = AtomicU64::new(0);

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

    /// Open `path` as a root, creating it first when `create`. The error is
    /// `directory <path>: <reason>` with exit code 2.
    pub fn open(path: &str, create: bool) -> Result<Directory, IoError> {
        let refuse =
            |reason: &dyn std::fmt::Display| IoError::new(format!("directory {path}: {reason}"));
        let opened = (|| {
            if path.is_empty() || path.contains('\0') {
                return Err(refuse(&"not a directory path"));
            }
            if create {
                std::fs::create_dir_all(path).map_err(|e| refuse(&reason(&e)))?;
            }
            let canonical = std::fs::canonicalize(path).map_err(|e| refuse(&reason(&e)))?;
            let descriptor = nix::fcntl::open(
                &canonical,
                OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(|e| refuse(&e.desc()))?;
            let path = canonical
                .into_os_string()
                .into_string()
                .map_err(|_| refuse(&"not UTF-8"))?;
            Ok(Directory {
                root: Arc::new(Root { descriptor, path }),
            })
        })();
        opened.map_err(|e: IoError| e.with_exit_code(2))
    }

    /// `Directory.temporary()`: a new empty directory inside the test's
    /// scratch directory (`IGNITE_TEST_SCRATCH`), test-only.
    pub fn temporary() -> Result<Directory, IoError> {
        match std::env::var_os("IGNITE_TEST_SCRATCH") {
            Some(scratch) => Directory::temporary_in(std::path::Path::new(&scratch)),
            None => Err(IoError::new(
                "Directory.temporary: no test scratch directory (IGNITE_TEST_SCRATCH)",
            )),
        }
    }

    /// A new empty directory `directory-<n>` inside `scratch`.
    fn temporary_in(scratch: &std::path::Path) -> Result<Directory, IoError> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let made = scratch.join(format!("directory-{n}"));
            match std::fs::create_dir(&made) {
                Ok(()) => {
                    let made = made.to_str().ok_or_else(|| {
                        IoError::new("Directory.temporary: the scratch path is not UTF-8")
                    })?;
                    return Directory::open(made, false).map_err(|e| e.with_exit_code(1));
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => {
                    return Err(IoError::new(format!(
                        "Directory.temporary: {}: {}",
                        scratch.display(),
                        reason(&e)
                    )));
                }
            }
        }
    }

    fn descriptor(&self) -> BorrowedFd<'_> {
        self.root.descriptor.as_fd()
    }

    /// The descriptor of the directory holding the last component, walked
    /// from the root; `None` when the path names the root.
    fn parent<'p>(&self, path: &Checked<'p>) -> Result<Option<(OwnedFd, &'p str)>, IoError> {
        let Some((last, _)) = path.components.split_last() else {
            return Ok(None);
        };
        let parent = self.walk(path, path.components.len() - 1)?;
        Ok(Some((parent, last)))
    }

    /// A descriptor of the directory made of the first `count` components.
    fn walk(&self, path: &Checked<'_>, count: usize) -> Result<OwnedFd, IoError> {
        let mut current = reopen(self.descriptor(), path)?;
        for index in 0..count {
            current = open_directory(current.as_fd(), path.components[index], path, index)?;
        }
        Ok(current)
    }

    /// The file's bytes, at most 16 MiB.
    pub fn read_file(&self, path: impl AsRef<str>) -> Result<Vec<u8>, IoError> {
        let path = check(path.as_ref())?;
        let Some((parent, name)) = self.parent(&path)? else {
            return Err(path.error("not a regular file"));
        };
        let last = path.components.len() - 1;
        let flags = OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC;
        let fd = match openat(parent.as_fd(), name, flags, Mode::empty()) {
            Ok(fd) => fd,
            Err(Errno::ELOOP) => return Err(path.link(last)),
            Err(errno) => return Err(path.os(errno)),
        };
        let stat = fstat(&fd).map_err(|e| path.os(e))?;
        if !is_regular(&stat) {
            return Err(path.error("not a regular file"));
        }
        let too_large = || path.error("larger than 16 MiB");
        if stat.st_size as u64 > READ_LIMIT {
            return Err(too_large());
        }
        let mut data = Vec::with_capacity(stat.st_size as usize);
        std::fs::File::from(fd)
            .take(READ_LIMIT + 1)
            .read_to_end(&mut data)
            .map_err(|e| path.error(reason(&e)))?;
        if data.len() as u64 > READ_LIMIT {
            return Err(too_large());
        }
        Ok(data)
    }

    /// Create or replace the file atomically: a temporary
    /// `.ignite-<pid>-<n>` in the same directory, its length independent of
    /// the target's name, renamed over the target. A replaced regular file
    /// keeps its permission bits; a new one is created 0644 under the umask.
    /// A link at the target is refused, not replaced.
    pub fn write_file(&self, path: impl AsRef<str>, data: impl AsRef<[u8]>) -> Result<(), IoError> {
        let path = check(path.as_ref())?;
        let Some((parent, name)) = self.parent(&path)? else {
            return Err(path.error("the root itself"));
        };
        let last = path.components.len() - 1;
        let kept = match fstatat(parent.as_fd(), name, AtFlags::AT_SYMLINK_NOFOLLOW) {
            Ok(stat) if is_link(&stat) => return Err(path.link(last)),
            Ok(stat) if is_directory(&stat) => return Err(path.os(Errno::EISDIR)),
            Ok(stat) if is_regular(&stat) => Some(stat.st_mode & 0o777),
            _ => None,
        };
        let n = WRITES.fetch_add(1, Ordering::Relaxed);
        let temporary = format!(".ignite-{}-{n}", std::process::id());
        let flags =
            OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC;
        let fd = openat(
            parent.as_fd(),
            temporary.as_str(),
            flags,
            Mode::from_bits_truncate(0o644),
        )
        .map_err(|e| path.os(e))?;
        let mut file = std::fs::File::from(fd);
        let written = match kept {
            Some(mode) => file.set_permissions(std::fs::Permissions::from_mode(mode)),
            None => Ok(()),
        }
        .and_then(|()| std::io::Write::write_all(&mut file, data.as_ref()))
        .map_err(|e| path.error(reason(&e)))
        .and_then(|()| {
            renameat(parent.as_fd(), temporary.as_str(), parent.as_fd(), name)
                .map_err(|e| path.os(e))
        });
        if written.is_err() {
            let _ = unlinkat(
                parent.as_fd(),
                temporary.as_str(),
                UnlinkatFlags::NoRemoveDir,
            );
        }
        written
    }

    /// The entries' names in byte order; a directory's ends with `/`. A
    /// name that is not UTF-8 has no `Text` that names it back, so the
    /// listing is refused with that name's bytes escaped
    /// (`path <p>: entry <name> is not UTF-8`).
    pub fn read_directory(&self, path: impl AsRef<str>) -> Result<Vec<String>, IoError> {
        let path = check(path.as_ref())?;
        let directory = self.walk(&path, path.components.len())?;
        entries(directory, &path, Some(ENTRY_LIMIT))?
            .into_iter()
            .map(|(name, directory)| {
                let text = name.into_string().map_err(|e| {
                    let bytes = e.into_cstring();
                    path.error(format_args!(
                        "entry {} is not UTF-8",
                        bytes.as_bytes().escape_ascii()
                    ))
                })?;
                Ok(if directory { text + "/" } else { text })
            })
            .collect()
    }

    /// Create the directory and its missing parents.
    pub fn create_directory(&self, path: impl AsRef<str>) -> Result<(), IoError> {
        let path = check(path.as_ref())?;
        let mut current = reopen(self.descriptor(), &path)?;
        for (index, name) in path.components.iter().enumerate() {
            match mkdirat(current.as_fd(), *name, Mode::from_bits_truncate(0o755)) {
                Ok(()) | Err(Errno::EEXIST) => {}
                Err(errno) => return Err(path.os(errno)),
            }
            current = open_directory(current.as_fd(), name, &path, index)?;
        }
        Ok(())
    }

    /// Remove a file, a symbolic link or an empty directory.
    pub fn remove(&self, path: impl AsRef<str>) -> Result<(), IoError> {
        let path = check(path.as_ref())?;
        let Some((parent, name)) = self.parent(&path)? else {
            return Err(path.error("the root itself"));
        };
        match unlinkat(parent.as_fd(), name, UnlinkatFlags::NoRemoveDir) {
            Ok(()) => Ok(()),
            Err(Errno::EISDIR) => {
                unlinkat(parent.as_fd(), name, UnlinkatFlags::RemoveDir).map_err(|e| path.os(e))
            }
            Err(errno) => Err(path.os(errno)),
        }
    }

    /// Remove a directory and everything under it, following no link. The
    /// depth is measured before anything is removed; both passes keep one
    /// descriptor per level, so the walk is iterative and bounded by
    /// [`DEPTH_LIMIT`].
    pub fn remove_tree(&self, path: impl AsRef<str>) -> Result<(), IoError> {
        let path = check(path.as_ref())?;
        let Some((parent, name)) = self.parent(&path)? else {
            return Err(path.error("the root itself"));
        };
        let last = path.components.len() - 1;
        let stat =
            fstatat(parent.as_fd(), name, AtFlags::AT_SYMLINK_NOFOLLOW).map_err(|e| path.os(e))?;
        if !is_directory(&stat) {
            return unlinkat(parent.as_fd(), name, UnlinkatFlags::NoRemoveDir)
                .map_err(|e| path.os(e));
        }
        let top = open_directory(parent.as_fd(), name, &path, last)?;
        measure(top, &path)?;
        let top = open_directory(parent.as_fd(), name, &path, last)?;
        empty(top, &path)?;
        unlinkat(parent.as_fd(), name, UnlinkatFlags::RemoveDir).map_err(|e| path.os(e))
    }

    /// The root's canonical absolute path as resolved at launch.
    pub fn absolute_path(&self) -> Result<String, IoError> {
        Ok(self.root.path.clone())
    }
}

/// A directory being visited: its descriptor and the subdirectories still
/// to visit (for [`measure`]) or the entries still to remove (for
/// [`empty`]).
struct Level {
    descriptor: OwnedFd,
    pending: Vec<(CString, bool)>,
}

impl Level {
    fn open(descriptor: OwnedFd, path: &Checked<'_>) -> Result<Level, IoError> {
        let listing = descriptor.try_clone().map_err(|e| path.error(reason(&e)))?;
        let pending = entries(listing, path, None)?;
        Ok(Level {
            descriptor,
            pending,
        })
    }

    fn child(&self, name: &CStr, path: &Checked<'_>) -> Result<OwnedFd, IoError> {
        match openat(
            self.descriptor.as_fd(),
            name,
            DIRECTORY_FLAGS,
            Mode::empty(),
        ) {
            Ok(fd) => Ok(fd),
            Err(errno) => Err(path.os(errno)),
        }
    }
}

/// Refuse a tree deeper than [`DEPTH_LIMIT`] levels, `top` being level 1.
fn measure(top: OwnedFd, path: &Checked<'_>) -> Result<(), IoError> {
    let mut stack = vec![Level::open(top, path)?];
    loop {
        let depth = stack.len();
        let Some(level) = stack.last_mut() else {
            return Ok(());
        };
        match level.pending.pop() {
            None => {
                stack.pop();
            }
            Some((_, false)) => {}
            Some((name, true)) => {
                if depth == DEPTH_LIMIT {
                    return Err(too_deep(path));
                }
                let child = level.child(&name, path)?;
                stack.push(Level::open(child, path)?);
            }
        }
    }
}

fn too_deep(path: &Checked<'_>) -> IoError {
    path.error(format_args!("deeper than {DEPTH_LIMIT} levels"))
}

/// Remove everything under `top`, depth first; a link is removed as a
/// link. A tree that grew past [`DEPTH_LIMIT`] since [`measure`] is
/// refused where it is found.
fn empty(top: OwnedFd, path: &Checked<'_>) -> Result<(), IoError> {
    let mut stack: Vec<(Level, CString)> = vec![(Level::open(top, path)?, CString::default())];
    loop {
        let depth = stack.len();
        let Some((level, _)) = stack.last_mut() else {
            return Ok(());
        };
        match level.pending.pop() {
            None => {
                let (_, name) = stack.pop().unwrap_or_else(|| unreachable!());
                if let Some((parent, _)) = stack.last() {
                    unlinkat(
                        parent.descriptor.as_fd(),
                        name.as_c_str(),
                        UnlinkatFlags::RemoveDir,
                    )
                    .map_err(|e| path.os(e))?;
                }
            }
            Some((name, false)) => {
                unlinkat(
                    level.descriptor.as_fd(),
                    name.as_c_str(),
                    UnlinkatFlags::NoRemoveDir,
                )
                .map_err(|e| path.os(e))?;
            }
            Some((name, true)) => {
                if depth == DEPTH_LIMIT {
                    return Err(too_deep(path));
                }
                let child = level.child(&name, path)?;
                stack.push((Level::open(child, path)?, name));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Directory;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A directory of its own under `std::env::temp_dir()` for one test,
    /// removed with its contents when dropped.
    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(name: &str) -> Scratch {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("ignite-io-fs-{}-{n}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(path.join("root")).unwrap();
            std::fs::create_dir_all(path.join("outside")).unwrap();
            Scratch { path }
        }

        fn root(&self) -> PathBuf {
            self.path.join("root")
        }

        fn outside(&self) -> PathBuf {
            self.path.join("outside")
        }

        fn directory(&self) -> Directory {
            Directory::open(self.root().to_str().unwrap(), false).unwrap()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn names(path: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(path)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    fn message<T: std::fmt::Debug>(result: Result<T, crate::IoError>) -> String {
        result.unwrap_err().message
    }

    /// Row 3 of `docs/slice.md` 21.0: links out of the root are refused at
    /// every component, and `remove` removes a link, not its target.
    #[test]
    fn symlinks_out_are_refused() {
        let scratch = Scratch::new("links");
        std::fs::create_dir(scratch.outside().join("dir")).unwrap();
        std::fs::write(scratch.outside().join("dir/secret"), b"secret").unwrap();
        std::fs::write(scratch.outside().join("file"), b"outside bytes").unwrap();
        std::os::unix::fs::symlink(scratch.outside().join("dir"), scratch.root().join("out"))
            .unwrap();
        std::os::unix::fs::symlink(scratch.outside().join("file"), scratch.root().join("f"))
            .unwrap();
        let d = scratch.directory();
        assert_eq!(
            message(d.read_file("out/secret")),
            "path out/secret: symbolic link out"
        );
        assert_eq!(
            message(d.write_file("out/new", b"x")),
            "path out/new: symbolic link out"
        );
        assert_eq!(
            message(d.read_directory("out")),
            "path out: symbolic link out"
        );
        assert_eq!(
            message(d.create_directory("out/d")),
            "path out/d: symbolic link out"
        );
        assert_eq!(message(d.read_file("f")), "path f: symbolic link f");
        assert_eq!(message(d.write_file("f", b"x")), "path f: symbolic link f");
        assert_eq!(names(&scratch.outside().join("dir")), ["secret"]);
        assert_eq!(
            std::fs::read(scratch.outside().join("file")).unwrap(),
            b"outside bytes"
        );
        d.remove("f").unwrap();
        assert!(std::fs::symlink_metadata(scratch.root().join("f")).is_err());
        assert_eq!(
            std::fs::read(scratch.outside().join("file")).unwrap(),
            b"outside bytes"
        );
        assert_eq!(names(&scratch.root()), ["out"]);
    }

    /// Row 3: a FIFO is not opened for a blocking read.
    #[test]
    fn a_fifo_is_not_read() {
        let scratch = Scratch::new("fifo");
        nix::unistd::mkfifo(
            &scratch.root().join("p"),
            nix::sys::stat::Mode::from_bits_truncate(0o600),
        )
        .unwrap();
        let d = scratch.directory();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(d.read_file("p").map_err(|e| e.message));
        });
        let read = receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("read_file of a FIFO returned within 1 s");
        assert_eq!(read, Err("path p: not a regular file".to_string()));
    }

    /// Row 4: a directory component exchanged with a link to an outside
    /// directory never lets a read reach the outside bytes.
    #[test]
    fn a_swapped_component_never_escapes() {
        let scratch = Scratch::new("swap");
        std::fs::create_dir(scratch.root().join("d")).unwrap();
        std::fs::write(scratch.root().join("d/inner"), b"inside").unwrap();
        std::fs::write(scratch.outside().join("inner"), b"outside").unwrap();
        std::os::unix::fs::symlink(scratch.outside(), scratch.root().join("e")).unwrap();
        let d = scratch.directory();
        let root = std::fs::File::open(scratch.root()).unwrap();
        let swapper = std::thread::spawn(move || {
            for _ in 0..10_000 {
                nix::fcntl::renameat2(
                    &root,
                    "d",
                    &root,
                    "e",
                    nix::fcntl::RenameFlags::RENAME_EXCHANGE,
                )
                .unwrap();
            }
        });
        let (mut inside, mut refused) = (0, 0);
        for _ in 0..10_000 {
            match d.read_file("d/inner") {
                Ok(bytes) => {
                    assert_eq!(bytes, b"inside");
                    inside += 1;
                }
                Err(e) if e.message.contains("symbolic link") => {
                    assert_eq!(e.message, "path d/inner: symbolic link d");
                    refused += 1;
                }
                Err(_) => {}
            }
        }
        swapper.join().unwrap();
        assert!(inside + refused > 0);
        assert_eq!(
            std::fs::read(scratch.outside().join("inner")).unwrap(),
            b"outside"
        );
    }

    /// Row 5: readers racing replacing writers see one whole file or the
    /// other, and no temporary is left.
    #[test]
    fn write_replaces_atomically() {
        let scratch = Scratch::new("atomic");
        let d = scratch.directory();
        let a = vec![b'A'; 64 * 1024];
        let b = vec![b'B'; 64 * 1024];
        d.write_file("file", &a).unwrap();
        let writer = {
            let d = d.clone();
            let (a, b) = (a.clone(), b.clone());
            std::thread::spawn(move || {
                for i in 0..1_000 {
                    d.write_file("file", if i % 2 == 0 { &b } else { &a })
                        .unwrap();
                }
            })
        };
        for _ in 0..1_000 {
            let read = d.read_file("file").unwrap();
            assert!(
                read == a || read == b,
                "a torn read of {} bytes",
                read.len()
            );
        }
        writer.join().unwrap();
        assert_eq!(d.read_directory("").unwrap(), ["file"]);
    }

    /// `write_file` replacing a file keeps the file's permission bits and
    /// leaves no temporary; the temporary's name does not grow with the
    /// target's.
    #[test]
    fn a_replaced_file_keeps_its_permission_bits() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new("mode");
        let d = scratch.directory();
        let mode = |name: &str| {
            std::fs::metadata(scratch.root().join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777
        };
        for (name, bits) in [("private", 0o600), ("script", 0o751), ("shared", 0o664)] {
            d.write_file(name, b"first").unwrap();
            std::fs::set_permissions(
                scratch.root().join(name),
                std::fs::Permissions::from_mode(bits),
            )
            .unwrap();
            d.write_file(name, b"second").unwrap();
            assert_eq!(mode(name), bits, "{name}");
            assert_eq!(d.read_file(name).unwrap(), b"second");
        }
        let long = "n".repeat(255);
        d.write_file(&long, b"x").unwrap();
        d.write_file(&long, b"y").unwrap();
        assert_eq!(d.read_file(&long).unwrap(), b"y");
        assert_eq!(
            names(&scratch.root()),
            [long.as_str(), "private", "script", "shared"]
        );
    }

    /// Row 7: the listing and depth bounds; a refused `remove_tree` removes
    /// nothing.
    #[test]
    fn bounds_hold() {
        let scratch = Scratch::new("bounds");
        let d = scratch.directory();
        let many = scratch.root().join("many");
        std::fs::create_dir(&many).unwrap();
        for i in 0..65_537 {
            std::fs::File::create(many.join(format!("{i:05}"))).unwrap();
        }
        assert_eq!(
            message(d.read_directory("many")),
            "path many: more than 65536 entries"
        );
        std::fs::remove_file(many.join("00000")).unwrap();
        assert_eq!(d.read_directory("many").unwrap().len(), 65_536);
        d.remove_tree("many").unwrap();
        let mut deep = scratch.root();
        for _ in 0..257 {
            deep.push("c");
        }
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("leaf"), b"x").unwrap();
        assert_eq!(
            message(d.remove_tree("c")),
            "path c: deeper than 256 levels"
        );
        assert!(deep.join("leaf").exists());
        std::fs::remove_file(deep.join("leaf")).unwrap();
        std::fs::remove_dir(&deep).unwrap();
        d.remove_tree("c").unwrap();
        assert_eq!(d.read_directory("").unwrap(), Vec::<String>::new());
    }

    /// Rows 1, 2 and 6 at the substrate: a round trip, the lexical
    /// refusals of every operation that takes a path, the root itself, and
    /// a tree holding a link out.
    #[test]
    fn paths_round_trip_and_leave_nothing_outside() {
        let scratch = Scratch::new("paths");
        let d = scratch.directory();
        let every: Vec<u8> = (0..=255).collect();
        d.write_file("a.bin", &every).unwrap();
        assert_eq!(d.read_file("a.bin").unwrap(), every);
        assert_eq!(d.read_directory("").unwrap(), ["a.bin"]);
        d.create_directory("sub/deeper").unwrap();
        d.create_directory("sub/deeper").unwrap();
        assert_eq!(d.read_directory(".").unwrap(), ["a.bin", "sub/"]);
        d.remove("a.bin").unwrap();
        d.remove_tree("sub").unwrap();
        assert_eq!(d.read_directory("").unwrap(), Vec::<String>::new());
        for (path, reason) in [
            ("../x", "leaves its root"),
            ("a/../../x", "leaves its root"),
            ("/etc/passwd", "absolute"),
            ("a\0b", "contains NUL"),
        ] {
            let want = format!("path {path}: {reason}");
            assert_eq!(message(d.read_file(path)), want);
            assert_eq!(message(d.write_file(path, b"x")), want);
            assert_eq!(message(d.read_directory(path)), want);
            assert_eq!(message(d.create_directory(path)), want);
            assert_eq!(message(d.remove(path)), want);
            assert_eq!(message(d.remove_tree(path)), want);
        }
        assert!(!scratch.path.join("x").exists());
        assert!(message(d.read_file("")).starts_with("path : "));
        assert_eq!(message(d.remove_tree("")), "path : the root itself");
        assert_eq!(message(d.remove_tree(".")), "path .: the root itself");
        assert_eq!(message(d.remove("")), "path : the root itself");
        assert!(message(d.remove("missing")).starts_with("path missing: "));
        assert!(message(d.remove_tree("missing")).starts_with("path missing: "));
        std::fs::write(scratch.outside().join("kept"), b"k").unwrap();
        d.create_directory("tree/a/b").unwrap();
        d.write_file("tree/a/b/f", b"f").unwrap();
        d.write_file("tree/g", b"g").unwrap();
        std::os::unix::fs::symlink(scratch.outside(), scratch.root().join("tree/out")).unwrap();
        assert_eq!(
            message(d.read_directory("tree/out")),
            "path tree/out: symbolic link tree/out"
        );
        assert_eq!(d.read_directory("tree").unwrap(), ["a/", "g", "out"]);
        d.remove_tree("tree").unwrap();
        assert_eq!(names(&scratch.root()), Vec::<String>::new());
        assert_eq!(names(&scratch.outside()), ["kept"]);
        let canonical = std::fs::canonicalize(scratch.root()).unwrap();
        assert_eq!(d.absolute_path().unwrap(), canonical.to_str().unwrap());
    }

    /// `Directory.temporary()` makes a new empty directory inside the
    /// scratch directory it is given, a different one on each call.
    #[test]
    fn temporaries_are_new_and_inside_the_scratch_directory() {
        let scratch = Scratch::new("temporary");
        let first = Directory::temporary_in(&scratch.root()).unwrap();
        let second = Directory::temporary_in(&scratch.root()).unwrap();
        let (first, second) = (
            first.absolute_path().unwrap(),
            second.absolute_path().unwrap(),
        );
        assert_ne!(first, second);
        let canonical = std::fs::canonicalize(scratch.root()).unwrap();
        for made in [&first, &second] {
            assert_eq!(Path::new(made).parent().unwrap(), canonical);
            assert_eq!(names(Path::new(made)), Vec::<String>::new());
        }
    }

    /// The root is opened once: a missing root is an error naming it, and
    /// `create` makes it with its parents.
    #[test]
    fn open_creates_or_refuses_a_root() {
        let scratch = Scratch::new("open");
        let missing = scratch.root().join("x/y");
        let refused = message(Directory::open(missing.to_str().unwrap(), false));
        assert!(
            refused.starts_with(&format!("directory {}: ", missing.display())),
            "{refused}"
        );
        let made = Directory::open(missing.to_str().unwrap(), true).unwrap();
        made.write_file("f", b"1").unwrap();
        assert_eq!(std::fs::read(missing.join("f")).unwrap(), b"1");
        std::fs::write(scratch.root().join("plain"), b"").unwrap();
        let plain = scratch.root().join("plain");
        assert!(Directory::open(plain.to_str().unwrap(), false).is_err());
    }
}
