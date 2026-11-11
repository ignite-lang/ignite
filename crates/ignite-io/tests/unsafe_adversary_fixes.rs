//! Adversary pass 2 of `story:unsafe-sites-back`: the three answers to
//! pass 1 (a listing of a removed directory, a level of `remove_tree`
//! listed through a duplicate of its descriptor, the overflow handler
//! private to `signal.rs`).
//!
//! The cases that lower `RLIMIT_NOFILE` or count the process's descriptors
//! run in a child process: the limit and the descriptor table are
//! process-wide, and the other tests of this executable open descriptors
//! concurrently.

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use ignite_io::Directory;
use nix::sys::resource::{Resource, getrlimit, setrlimit};

const CHILD: &str = "IGNITE_UNSAFE_FIXES_CHILD";

fn child(name: &str) -> Output {
    Command::new(std::env::current_exe().unwrap())
        .args([name, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .output()
        .unwrap()
}

fn in_child() -> bool {
    std::env::var_os(CHILD).is_some()
}

fn passed(output: &Output) {
    assert!(
        output.status.success(),
        "child failed: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "the child ran no test:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// A directory of its own under `std::env::temp_dir()`, removed with its
/// contents when dropped.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Scratch {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ignite-io-unsafe-fixes-{}-{n}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("root")).unwrap();
        Scratch { path }
    }

    fn root(&self) -> PathBuf {
        self.path.join("root")
    }

    fn directory(&self) -> Directory {
        Directory::open(self.root().to_str().unwrap(), false).unwrap()
    }

    /// `c/c/.../c` under the root, `levels` deep, with a file at the bottom.
    fn tree(&self, levels: usize) {
        let mut deep = self.root();
        for _ in 0..levels {
            deep.push("c");
        }
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("leaf"), b"x").unwrap();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// The descriptors open in this process (the listing's own one excluded).
fn open_descriptors() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count() - 1
}

/// `remove_tree` on the tree at `path` with the soft `RLIMIT_NOFILE` set so
/// that exactly `spare` more descriptors can be opened; the limit is put
/// back before returning.
fn remove_tree_with_spare(d: &Directory, path: &str, spare: usize) -> Result<(), String> {
    let (soft, hard) = getrlimit(Resource::RLIMIT_NOFILE).unwrap();
    let limit = (open_descriptors() + spare) as u64;
    setrlimit(Resource::RLIMIT_NOFILE, limit, hard).unwrap();
    let removed = d.remove_tree(path).map_err(|e| e.to_string());
    setrlimit(Resource::RLIMIT_NOFILE, soft, hard).unwrap();
    removed
}

/// Fix 2 (`bb7cd7ea`): a tree exactly at the depth bound (256 levels) took
/// 256 descriptors beyond those already open at the base, one per level,
/// each level listed through its own descriptor. A level now lists
/// through a duplicate, opened while the 256 level descriptors are held, so
/// the same tree needs 257: a process with exactly the base's headroom gets
/// `Too many open files` where the base removed the tree.
#[test]
fn a_tree_at_the_depth_bound_is_removed_with_the_descriptors_the_base_needed() {
    passed(&child(
        "unsafe_adversary_fixes::child_depth_bound_within_base_descriptors",
    ));
}

#[test]
fn child_depth_bound_within_base_descriptors() {
    if !in_child() {
        return;
    }
    let scratch = Scratch::new("headroom");
    let d = scratch.directory();
    scratch.tree(256);
    let before = open_descriptors();
    let at_base = remove_tree_with_spare(&d, "c", 256);
    let after = open_descriptors();
    let one_more = if at_base.is_err() {
        Some(remove_tree_with_spare(&d, "c", 257))
    } else {
        None
    };
    println!("spare 256: {at_base:?}; spare 257: {one_more:?}; descriptors {before} -> {after}");
    assert_eq!(after, before, "the failing walk left descriptors open");
    assert_eq!(
        at_base,
        Ok(()),
        "256 spare descriptors removed this tree at the base (one per level)"
    );
}

/// Fix 2: a `remove_tree` that fails (deeper than the bound, or out of
/// descriptors at the deepest level) closes every descriptor it opened,
/// the duplicates included, and removes nothing.
#[test]
fn a_failing_remove_tree_leaves_no_descriptor_open() {
    passed(&child(
        "unsafe_adversary_fixes::child_failing_remove_tree_closes_all",
    ));
}

#[test]
fn child_failing_remove_tree_closes_all() {
    if !in_child() {
        return;
    }
    let scratch = Scratch::new("leak");
    let d = scratch.directory();
    scratch.tree(257);
    let before = open_descriptors();
    let deep = d.remove_tree("c").map_err(|e| e.to_string());
    assert_eq!(deep, Err("path c: deeper than 256 levels".to_string()));
    assert_eq!(open_descriptors(), before, "after the depth refusal");
    for spare in [1, 2, 100, 255] {
        let short = remove_tree_with_spare(&d, "c", spare);
        assert_eq!(
            short,
            Err("path c: Too many open files".to_string()),
            "spare {spare}"
        );
        assert_eq!(
            open_descriptors(),
            before,
            "after EMFILE with {spare} spare"
        );
    }
    let mut leaf = scratch.root();
    for _ in 0..257 {
        leaf.push("c");
    }
    assert!(
        leaf.join("leaf").exists(),
        "a refused walk removed something"
    );
}

/// Fix 1 (`a6059e1a`): the error a caller sees for a removed root is the
/// base's word for word (`getdents64`'s `ENOENT` through `path.os`); an
/// unreadable root stays `EACCES` and a file stays `ENOTDIR`; an empty
/// live root is still an empty listing.
#[test]
fn a_removed_unreadable_or_non_directory_listing_reads_as_the_base() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("errors");
    let d = scratch.directory();
    assert_eq!(d.read_directory("").unwrap(), Vec::<String>::new());
    std::fs::write(scratch.root().join("file"), b"x").unwrap();
    assert_eq!(
        d.read_directory("file").unwrap_err().to_string(),
        "path file: Not a directory"
    );
    std::fs::create_dir(scratch.root().join("shut")).unwrap();
    std::fs::set_permissions(
        scratch.root().join("shut"),
        std::fs::Permissions::from_mode(0o300),
    )
    .unwrap();
    let shut = d.read_directory("shut").map_err(|e| e.to_string());
    std::fs::set_permissions(
        scratch.root().join("shut"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    assert_eq!(shut, Err("path shut: Permission denied".to_string()));
    std::fs::remove_file(scratch.root().join("file")).unwrap();
    std::fs::remove_dir(scratch.root().join("shut")).unwrap();
    std::fs::remove_dir(scratch.root()).unwrap();
    for path in ["", "."] {
        assert_eq!(
            d.read_directory(path).unwrap_err().to_string(),
            format!("path {path}: No such file or directory")
        );
    }
}

/// The signal number each test handler saw, 0 when it did not run.
static SEGV_SEEN: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static BUS_SEEN: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

extern "C" fn segv_seen(
    signal: i32,
    info: *mut nix::libc::siginfo_t,
    _context: *mut std::ffi::c_void,
) {
    // SAFETY: the kernel (or `on_fault` passing it on) gives a valid
    // `siginfo_t` to a handler installed with `SA_SIGINFO`.
    let number = if info.is_null() {
        -1
    } else {
        unsafe { (*info).si_signo }
    };
    SEGV_SEEN.store(signal * 100 + number, std::sync::atomic::Ordering::SeqCst);
}

extern "C" fn bus_seen(
    signal: i32,
    info: *mut nix::libc::siginfo_t,
    _context: *mut std::ffi::c_void,
) {
    // SAFETY: as in `segv_seen`.
    let number = if info.is_null() {
        -1
    } else {
        unsafe { (*info).si_signo }
    };
    BUS_SEEN.store(signal * 100 + number, std::sync::atomic::Ordering::SeqCst);
}

/// Fix 3 (`07cbf054`): `on_fault`, now private to `signal.rs`, still hands
/// a `SIGSEGV` or `SIGBUS` outside a task's guard page to the handler that
/// was installed before the first spawn, each signal to its own handler,
/// with the kernel's `siginfo_t`. A raised signal stands in for a fault
/// (`si_addr` of a `tgkill` is not in a guard page; returning does not
/// repeat it).
#[test]
fn a_signal_outside_a_guard_page_reaches_the_handler_installed_before() {
    passed(&child(
        "unsafe_adversary_fixes::child_signal_outside_a_guard_page",
    ));
}

#[test]
fn child_signal_outside_a_guard_page() {
    use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, raise, sigaction};
    if !in_child() {
        return;
    }
    for (signal, handler) in [
        (Signal::SIGSEGV, segv_seen as extern "C" fn(_, _, _)),
        (Signal::SIGBUS, bus_seen),
    ] {
        let action = SigAction::new(
            SigHandler::SigAction(handler),
            SaFlags::SA_SIGINFO | SaFlags::SA_ONSTACK,
            SigSet::empty(),
        );
        // SAFETY: the handlers only store into an atomic.
        unsafe { sigaction(signal, &action) }.unwrap();
    }
    ignite_io::spawn(|| {
        raise(Signal::SIGSEGV).unwrap();
        raise(Signal::SIGBUS).unwrap();
    });
    ignite_io::wait_for_tasks();
    for (signal, handler) in [
        (nix::libc::SIGSEGV, segv_seen as extern "C" fn(_, _, _)),
        (nix::libc::SIGBUS, bus_seen),
    ] {
        // SAFETY: a null new action only reads the current one into `now`.
        let mut now: nix::libc::sigaction = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { nix::libc::sigaction(signal, std::ptr::null(), &mut now) },
            0
        );
        assert_ne!(
            now.sa_sigaction, handler as usize,
            "the overflow handler was not installed in front of the test's for signal {signal}"
        );
    }
    let segv = SEGV_SEEN.load(std::sync::atomic::Ordering::SeqCst);
    let bus = BUS_SEEN.load(std::sync::atomic::Ordering::SeqCst);
    println!("segv handler saw {segv}, bus handler saw {bus}");
    assert_eq!(segv, 11 * 100 + 11);
    assert_eq!(bus, 7 * 100 + 7);
}
