//! Adversarial cases for `ignite_io::Directory` (`docs/slice.md` 21.4,
//! rows 1 to 7): what the unit tests in `src/fs.rs` do not say.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use ignite_io::Directory;

/// A directory of its own under `std::env::temp_dir()`, holding `root`
/// and `outside`, removed with its contents when dropped.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Scratch {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ignite-io-fs-adversary-{}-{n}-{name}",
            std::process::id()
        ));
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

fn open_descriptors() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

/// 21.4: `remove_tree` "removes a directory and everything under it". A
/// file whose name is not UTF-8 (left by a tool the program launched, or
/// by anyone else writing into the root) is part of the tree.
#[test]
fn remove_tree_removes_a_name_that_is_not_utf8() {
    let scratch = Scratch::new("non-utf8-tree");
    let tree = scratch.root().join("tree");
    std::fs::create_dir(&tree).unwrap();
    std::fs::write(tree.join(OsStr::from_bytes(b"\xff")), b"x").unwrap();
    std::fs::write(tree.join("plain"), b"y").unwrap();
    let d = scratch.directory();
    assert_eq!(d.remove_tree("tree").map_err(|e| e.message), Ok(()));
    assert!(!tree.exists());
}

/// 21.4: a listed entry is a path of the directory, so a name that is not
/// UTF-8, which no `Text` names back, is never listed under another name:
/// the listing is refused with the name's bytes escaped, and the entry is
/// left as it was.
#[test]
fn a_listed_name_that_is_not_utf8_can_be_removed() {
    let scratch = Scratch::new("non-utf8-list");
    std::fs::write(scratch.root().join(OsStr::from_bytes(b"a\xff")), b"x").unwrap();
    let d = scratch.directory();
    assert_eq!(
        d.read_directory("").map_err(|e| e.message),
        Err("path : entry a\\xff is not UTF-8".to_string())
    );
    assert_eq!(
        std::fs::read(scratch.root().join(OsStr::from_bytes(b"a\xff"))).unwrap(),
        b"x"
    );
}

/// 21.4: `write_file` "creates or replaces the file". A name of 250 bytes
/// is a legal file name (NAME_MAX is 255), and the OS creates it.
#[test]
fn write_file_takes_a_long_legal_name() {
    let scratch = Scratch::new("long-name");
    let name = "n".repeat(250);
    std::fs::write(scratch.root().join(&name), b"os").unwrap();
    std::fs::remove_file(scratch.root().join(&name)).unwrap();
    let d = scratch.directory();
    assert_eq!(d.write_file(&name, b"x").map_err(|e| e.message), Ok(()));
    assert_eq!(d.read_file(&name).unwrap(), b"x");
}

/// 21.4 and row 7: the only bound of `remove_tree` is its depth; the
/// 65,536-entry bound is `read_directory`'s. A directory of 65,537 files
/// is removed.
#[test]
fn remove_tree_removes_a_directory_of_many_entries() {
    let scratch = Scratch::new("many");
    let many = scratch.root().join("many");
    std::fs::create_dir(&many).unwrap();
    for i in 0..65_537 {
        std::fs::File::create(many.join(format!("{i:05}"))).unwrap();
    }
    let d = scratch.directory();
    assert_eq!(d.remove_tree("many").map_err(|e| e.message), Ok(()));
    assert!(!many.exists());
}

/// 21.4: links to a parent and a link chain inside the root never reach
/// outside, for every operation, and `remove_tree` of a tree holding them
/// leaves the outside intact.
#[test]
fn links_to_parents_never_escape() {
    let scratch = Scratch::new("parents");
    std::fs::write(scratch.outside().join("kept"), b"k").unwrap();
    std::fs::create_dir(scratch.root().join("a")).unwrap();
    std::os::unix::fs::symlink("..", scratch.root().join("a/up")).unwrap();
    std::os::unix::fs::symlink("../..", scratch.root().join("a/top")).unwrap();
    std::os::unix::fs::symlink("top", scratch.root().join("a/chain")).unwrap();
    let d = scratch.directory();
    for path in ["a/up/a", "a/top/outside/kept", "a/chain/outside/kept"] {
        let link = path.split('/').take(2).collect::<Vec<_>>().join("/");
        let want = format!("path {path}: symbolic link {link}");
        assert_eq!(d.read_file(path).map_err(|e| e.message), Err(want.clone()));
        assert_eq!(
            d.write_file(path, b"x").map_err(|e| e.message),
            Err(want.clone())
        );
        assert_eq!(d.remove(path).map_err(|e| e.message), Err(want.clone()));
        assert_eq!(d.remove_tree(path).map_err(|e| e.message), Err(want));
    }
    d.remove_tree("a").unwrap();
    assert_eq!(std::fs::read(scratch.outside().join("kept")).unwrap(), b"k");
    assert!(scratch.root().exists());
}

/// 21.3: a `Directory` lives for the process; its operations, failing or
/// not, keep no descriptor open.
#[test]
fn operations_leak_no_descriptor() {
    let scratch = Scratch::new("leak");
    let d = scratch.directory();
    std::fs::create_dir(scratch.outside().join("o")).unwrap();
    std::os::unix::fs::symlink(scratch.outside().join("o"), scratch.root().join("out")).unwrap();
    let round = |i: usize| {
        let _ = d.create_directory(format!("t/{i}/x"));
        let _ = d.write_file(format!("t/{i}/x/f"), b"data");
        let _ = d.read_file(format!("t/{i}/x/f"));
        let _ = d.read_file("out/f");
        let _ = d.read_file("t");
        let _ = d.read_directory("t");
        let _ = d.read_directory("missing/x");
        let _ = d.write_file("missing/x", b"x");
        let _ = d.remove("t");
        let _ = d.remove_tree(format!("t/{i}"));
        let _ = d.remove_tree("missing");
    };
    round(0);
    let before = open_descriptors();
    for i in 1..1_000 {
        round(i);
    }
    let after = open_descriptors();
    assert!(
        after < before + 64,
        "{before} descriptors before, {after} after 999 rounds"
    );
}

/// 21.3: several tasks may use one `Directory` at once; each sees its own
/// files and none fails for another's work.
#[test]
fn concurrent_tasks_share_one_directory() {
    let scratch = Scratch::new("concurrent");
    let d = scratch.directory();
    let threads: Vec<_> = (0..8)
        .map(|t| {
            let d = d.clone();
            std::thread::spawn(move || {
                for i in 0..200 {
                    let dir = format!("shared/t{t}/{i}");
                    d.create_directory(&dir).unwrap();
                    let file = format!("{dir}/f");
                    let data = format!("{t}-{i}").into_bytes();
                    d.write_file(&file, &data).unwrap();
                    assert_eq!(d.read_file(&file).unwrap(), data);
                    assert_eq!(d.read_directory(&dir).unwrap(), ["f"]);
                    d.remove_tree(&dir).unwrap();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    d.remove_tree("shared").unwrap();
    assert_eq!(d.read_directory("").unwrap(), Vec::<String>::new());
}
