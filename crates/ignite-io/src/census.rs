//! The census of the runtime (`docs/leak-guard.md` 5.1, `docs/slice.md`
//! 33): the live objects of this process per kind and the heap bytes it
//! holds, read by `Runtime.census()` on both targets.
//!
//! One counter per kind, raised where an object is made and lowered where
//! it is dropped, so both targets count the same objects: a [`Counted`]
//! token lives inside the shared part of each object (the `Arc` its
//! clones share), and its drop lowers the count. The LLVM runtime adds its
//! own kinds (`handles`, `handle_slots`, `heap_copies`, `arena_bytes`)
//! through [`add`]. Heap bytes come from [`Counting`], a global allocator
//! over the system allocator that the generated Rust program, its test
//! executable and the LLVM runtime install; without it `heap_bytes` is 0.
//!
//! Seeded leaks (`docs/leak-guard.md` 7): a debug build reads
//! `IGNITE_DEBUG_LEAK=<kind>` once and then skips one release per object of
//! that kind (`task`, `channel`, `listener`, `stream`, `pipe`, `child`,
//! `directory`, `capability`; the LLVM runtime's `handle`, `heap_copy` and
//! `arena`), or keeps 370 bytes per dropped channel (`bytes`), so a soak
//! test can be shown red without a real bug. Release builds ignore it.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

/// A kind the census counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Tasks,
    Workers,
    Channels,
    Listeners,
    Streams,
    Pipes,
    Children,
    Directories,
    Capabilities,
    Handles,
    HandleSlots,
    HeapCopies,
    Stores,
    ArenaBytes,
}

const KINDS: usize = 14;

impl Kind {
    /// The value of `IGNITE_DEBUG_LEAK` that seeds a leak of this kind.
    fn seed(self) -> &'static str {
        match self {
            Kind::Tasks => "task",
            Kind::Channels => "channel",
            Kind::Listeners => "listener",
            Kind::Streams => "stream",
            Kind::Pipes => "pipe",
            Kind::Children => "child",
            Kind::Directories => "directory",
            Kind::Capabilities => "capability",
            Kind::Handles => "handle",
            Kind::HeapCopies => "heap_copy",
            Kind::ArenaBytes => "arena",
            Kind::Workers | Kind::HandleSlots | Kind::Stores => "",
        }
    }
}

static COUNTS: [AtomicI64; KINDS] = [const { AtomicI64::new(0) }; KINDS];

/// Add `n` to the count of `kind`.
pub fn add(kind: Kind, n: i64) {
    COUNTS[kind as usize].fetch_add(n, Ordering::Relaxed);
}

/// The count of `kind`.
pub fn count(kind: Kind) -> i64 {
    COUNTS[kind as usize].load(Ordering::Relaxed)
}

/// Lower the count of `kind` by one for an object released, unless a
/// leak of that kind is seeded.
pub fn release(kind: Kind) {
    if !seeded(kind.seed()) {
        add(kind, -1);
    }
}

/// `IGNITE_DEBUG_LEAK` is `leak` in a debug build.
pub fn seeded(leak: &str) -> bool {
    static SEED: OnceLock<Option<String>> = OnceLock::new();
    if !cfg!(debug_assertions) || leak.is_empty() {
        return false;
    }
    SEED.get_or_init(|| std::env::var("IGNITE_DEBUG_LEAK").ok())
        .as_deref()
        == Some(leak)
}

/// Keep `bytes` bytes of the heap for the life of the process: the seeded
/// leak `bytes`.
pub(crate) fn keep_bytes(bytes: usize) {
    std::mem::forget(vec![1u8; bytes]);
}

/// One live object of a kind: made with the object, dropped with it.
/// Clones of the object share it (inside their `Arc`); equal to every
/// other token, so a value's equality ignores it.
#[derive(Debug)]
pub struct Counted(Kind);

impl Counted {
    pub fn new(kind: Kind) -> Counted {
        add(kind, 1);
        Counted(kind)
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        release(self.0);
    }
}

impl PartialEq for Counted {
    fn eq(&self, _: &Counted) -> bool {
        true
    }
}

impl Eq for Counted {}

/// The census at one moment, the fields of `std.runtime.Census` in their
/// order (the LLVM runtime writes it as that record's layout).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Census {
    pub tasks: i64,
    pub workers: i64,
    pub channels: i64,
    pub listeners: i64,
    pub streams: i64,
    pub pipes: i64,
    pub children: i64,
    pub directories: i64,
    pub capabilities: i64,
    pub handles: i64,
    pub handle_slots: i64,
    pub heap_copies: i64,
    pub stores: i64,
    pub heap_bytes: i64,
    pub arena_bytes: i64,
    pub resident_bytes: i64,
}

/// The counters now. `arena_bytes` is every task's arena blocks; the LLVM
/// runtime subtracts the caller's own.
pub fn read() -> Census {
    Census {
        tasks: count(Kind::Tasks),
        workers: count(Kind::Workers),
        channels: count(Kind::Channels),
        listeners: count(Kind::Listeners),
        streams: count(Kind::Streams),
        pipes: count(Kind::Pipes),
        children: count(Kind::Children),
        directories: count(Kind::Directories),
        capabilities: count(Kind::Capabilities),
        handles: count(Kind::Handles),
        handle_slots: count(Kind::HandleSlots),
        heap_copies: count(Kind::HeapCopies),
        stores: count(Kind::Stores),
        heap_bytes: heap_bytes(),
        arena_bytes: count(Kind::ArenaBytes),
        resident_bytes: resident_bytes(),
    }
}

/// Wait until at most `tasks` tasks are live, at most `within`
/// nanoseconds; whether that happened. Reads the live tasks of
/// `crate::task`, not the census count, so a seeded `task` leak does not
/// hold it up.
pub fn settle(tasks: i64, within: i64) -> bool {
    crate::flush();
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_nanos(within.max(0) as u64);
    loop {
        if crate::task::live_tasks() as i64 <= tasks {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        crate::wait::sleep(std::time::Duration::from_micros(200));
    }
}

/// `VmRSS` from `/proc/self/status`; 0 where it cannot be read.
fn resident_bytes() -> i64 {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return 0;
    };
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| {
            rest.trim()
                .trim_end_matches("kB")
                .trim()
                .parse::<i64>()
                .ok()
        })
        .map_or(0, |kib| kib * 1024)
}

/// Counter blocks of the heap bytes; a thread adds to its own block, the
/// census sums them. More threads than blocks share blocks.
const BLOCKS: usize = 64;

#[repr(align(64))]
struct Block(AtomicI64);

static HEAP: [Block; BLOCKS] = [const { Block(AtomicI64::new(0)) }; BLOCKS];
static NEXT_BLOCK: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static BLOCK: Cell<usize> = const { Cell::new(usize::MAX) };
}

fn heap_add(bytes: i64) {
    let at = BLOCK
        .try_with(|block| {
            if block.get() == usize::MAX {
                block.set(NEXT_BLOCK.fetch_add(1, Ordering::Relaxed) % BLOCKS);
            }
            block.get()
        })
        .unwrap_or(0);
    HEAP[at].0.fetch_add(bytes, Ordering::Relaxed);
}

/// Bytes allocated through [`Counting`] and not freed.
pub fn heap_bytes() -> i64 {
    HEAP.iter()
        .map(|block| block.0.load(Ordering::Relaxed))
        .sum()
}

/// Count `bytes` allocated outside [`Counting`] (the LLVM runtime's
/// `heap_alloc`), or freed with a negative count.
pub fn heap_counted(bytes: i64) {
    heap_add(bytes);
}

#[cfg(test)]
thread_local! {
    /// The calls of [`Counting`] on this thread that reached `alloc` and
    /// `realloc`, kept by the unit tests' build only (which installs it),
    /// so that a test tells a growth done in place from a new allocation.
    static CALLS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}

/// Count a call of [`Counting`] that reached `alloc` (`(1, 0)`) or
/// `realloc` (`(0, 1)`); nothing outside the unit tests.
fn called(alloc: usize, realloc: usize) {
    #[cfg(test)]
    let _ = CALLS.try_with(|calls| {
        let (allocs, reallocs) = calls.get();
        calls.set((allocs + alloc, reallocs + realloc));
    });
    #[cfg(not(test))]
    let _ = (alloc, realloc);
}

/// The counting global allocator: the system allocator, with every
/// allocation's size added to the heap bytes and every free's subtracted.
/// A reallocation is the system allocator's, which grows a buffer in place
/// where it can and remaps a large one instead of copying it: it is no new
/// allocation, and the heap bytes follow its change of size.
pub struct Counting;

// SAFETY: every call goes to `System` unchanged; the counting touches no
// allocation.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        called(1, 0);
        // SAFETY: as the caller's contract.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            heap_add(layout.size() as i64);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: as the caller's contract.
        unsafe { System.dealloc(pointer, layout) };
        heap_add(-(layout.size() as i64));
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        called(0, 1);
        // SAFETY: as the caller's contract.
        let moved = unsafe { System.realloc(pointer, layout, new_size) };
        if !moved.is_null() {
            heap_add(new_size as i64 - layout.size() as i64);
        }
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_counted_token_counts_while_it_lives() {
        let before = count(Kind::HandleSlots);
        let token = Counted::new(Kind::HandleSlots);
        assert_eq!(count(Kind::HandleSlots), before + 1);
        drop(token);
        assert_eq!(count(Kind::HandleSlots), before);
    }

    #[test]
    fn a_store_counts() {
        let store: crate::Store<i64> = crate::Store::new();
        assert!(read().stores >= 1);
        drop(store);
    }

    #[test]
    fn resident_bytes_are_read() {
        assert!(read().resident_bytes > 0);
    }

    #[global_allocator]
    static ALLOCATOR: Counting = Counting;

    /// A buffer that grows is reallocated by the system allocator, which
    /// grows it in place or moves it, not allocated again and copied: the
    /// one allocation of the buffer reaches `alloc`, its growth from 16
    /// bytes to 1 MiB reaches `realloc`. Counted on this thread by the
    /// allocator itself (the heap bytes across a growth:
    /// `tests/unsafe_adversary.rs`).
    #[test]
    fn a_growing_buffer_is_reallocated_not_allocated_again() {
        let (allocs, reallocs) = CALLS.with(Cell::get);
        let mut bytes: Vec<u8> = Vec::with_capacity(16);
        bytes.extend_from_slice(&[7; 16]);
        bytes.reserve_exact(1 << 20);
        let (allocs_after, reallocs_after) = CALLS.with(Cell::get);
        let grown = bytes.capacity();
        drop(std::hint::black_box(bytes));
        assert_eq!(
            (allocs_after - allocs, reallocs_after - reallocs),
            (1, 1),
            "calls of alloc and realloc for one buffer grown once"
        );
        assert_eq!(grown, 16 + (1 << 20));
    }
}
