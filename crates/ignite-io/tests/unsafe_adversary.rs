//! Adversarial cases for `story:unsafe-sites-back`: the request-path
//! removals (the byte search, the directory listing through `nix::dir::Dir`)
//! keep the behaviour of the code they replaced.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use ignite_io::Directory;

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
            "ignite-io-unsafe-adversary-{}-{n}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("root")).unwrap();
        Scratch { path }
    }

    fn root(&self) -> PathBuf {
        self.path.join("root")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// The listing read `getdents64` itself and returned its error; Linux
/// answers `ENOENT` for a directory that was removed (`iterate_dir` on a
/// dead inode). glibc's `readdir`, which `nix::dir::Dir` calls, turns that
/// `ENOENT` into the end of the directory, so the listing of a removed
/// root became an empty list where it was an error. The story asks for the
/// listing's behaviour unchanged ("a directory removed while listed").
#[test]
fn a_listing_of_a_removed_root_is_an_error_as_before() {
    let scratch = Scratch::new("removed-root");
    std::fs::write(scratch.root().join("kept"), b"x").unwrap();
    let directory = Directory::open(scratch.root().to_str().unwrap(), false).unwrap();
    assert_eq!(
        directory.read_directory("").unwrap(),
        ["kept"],
        "the root lists before it is removed"
    );
    std::fs::remove_file(scratch.root().join("kept")).unwrap();
    std::fs::remove_dir(scratch.root()).unwrap();
    let listed = directory.read_directory("");
    assert!(
        listed.is_err(),
        "a removed root listed as {listed:?}; getdents64 reports ENOENT for it"
    );
}

/// The reference the byte search must agree with: every window compared.
fn naive(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// `text::find` against the reference over inputs from a fixed seed: an
/// alphabet of three bytes (one not UTF-8) so partial matches are common,
/// needles from empty to longer than the haystack, and needles cut from
/// the haystack's end so a match at the last offset occurs.
#[test]
fn the_byte_search_agrees_with_every_window_compared() {
    let mut state: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let alphabet = [b'a', b'b', 0xff];
    let mut checked = 0;
    for _ in 0..20_000 {
        let length = (next() % 24) as usize;
        let haystack: Vec<u8> = (0..length)
            .map(|_| alphabet[(next() % 3) as usize])
            .collect();
        let needle: Vec<u8> = match next() % 3 {
            0 if !haystack.is_empty() => {
                let take = 1 + (next() as usize % haystack.len());
                haystack[haystack.len() - take..].to_vec()
            }
            _ => {
                let length = (next() % 27) as usize;
                (0..length)
                    .map(|_| alphabet[(next() % 3) as usize])
                    .collect()
            }
        };
        assert_eq!(
            ignite_io::text::find(&haystack, &needle),
            naive(&haystack, &needle),
            "find({haystack:?}, {needle:?})"
        );
        checked += 1;
    }
    assert_eq!(checked, 20_000);
    assert_eq!(ignite_io::text::find(b"", b""), Some(0));
    assert_eq!(ignite_io::text::find(b"ab", b"abc"), None);
    assert_eq!(ignite_io::text::find(b"aab", b"b"), Some(2));
    assert_eq!(ignite_io::text::find(b"aab", b"ab"), Some(1));
    assert_eq!(ignite_io::text::find(&[0xff, 0xfe, 0xff], &[0xff]), Some(0));
    assert_eq!(ignite_io::text::find(&[0xfe, 0xff], &[0xfe, 0xff]), Some(0));
}

/// The counting allocator lost its `realloc` override: a reallocation is
/// the trait's default (allocate, copy, free). Called directly, not
/// installed, so no other test's allocation reaches the census while this
/// one measures. Exact through a grow, a shrink and a shrink to one byte,
/// with the bytes moved intact.
#[test]
fn a_reallocation_through_the_counting_allocator_is_counted_exactly() {
    use ignite_io::census::{Counting, heap_bytes};
    use std::alloc::{GlobalAlloc, Layout};
    let start = heap_bytes();
    let layout = Layout::from_size_align(16, 8).unwrap();
    // SAFETY: a valid non-zero layout; every pointer below is the live
    // allocation of the layout it is passed with, and each size is non-zero.
    unsafe {
        let first = Counting.alloc(layout);
        assert!(!first.is_null());
        first.write_bytes(7, 16);
        assert_eq!(heap_bytes() - start, 16, "the first allocation");
        let grown = Counting.realloc(first, layout, 4096);
        assert!(!grown.is_null());
        assert_eq!(heap_bytes() - start, 4096, "after a grow");
        assert!((0..16).all(|at| *grown.add(at) == 7), "moved intact");
        let grown_layout = Layout::from_size_align(4096, 8).unwrap();
        let shrunk = Counting.realloc(grown, grown_layout, 1);
        assert!(!shrunk.is_null());
        assert_eq!(heap_bytes() - start, 1, "after a shrink");
        assert_eq!(*shrunk, 7, "moved intact");
        Counting.dealloc(shrunk, Layout::from_size_align(1, 8).unwrap());
    }
    assert_eq!(heap_bytes() - start, 0, "after the free");
}

/// A listing of an empty root and a root with a name that is not UTF-8:
/// as before, empty, and refused with the name escaped.
#[test]
fn listings_of_an_empty_root_and_a_non_utf8_name_are_as_before() {
    use std::os::unix::ffi::OsStrExt;
    let scratch = Scratch::new("listings");
    let directory = Directory::open(scratch.root().to_str().unwrap(), false).unwrap();
    assert_eq!(directory.read_directory("").unwrap(), Vec::<String>::new());
    std::fs::create_dir(scratch.root().join("b")).unwrap();
    std::fs::write(scratch.root().join("a"), b"x").unwrap();
    std::os::unix::fs::symlink("b", scratch.root().join("c")).unwrap();
    assert_eq!(directory.read_directory("").unwrap(), ["a", "b/", "c"]);
    std::fs::write(
        scratch
            .root()
            .join(std::ffi::OsStr::from_bytes(b"\xff\xfe")),
        b"x",
    )
    .unwrap();
    let refused = directory.read_directory("").unwrap_err().to_string();
    assert!(
        refused.contains("entry \\xff\\xfe is not UTF-8"),
        "refused as {refused}"
    );
}
