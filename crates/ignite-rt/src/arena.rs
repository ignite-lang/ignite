//! Per-task region allocator (`docs/llvm.md` section 4). Each thread owns
//! one arena, a stack of blocks with a bump pointer in the last one, and a
//! second, scratch arena. A mark is a position in the stack;
//! [`ignite_rt_arena_release`] frees everything allocated after it, and
//! [`ignite_rt_task_end`] frees all of it. Nothing is freed individually: a
//! grown list leaves its old buffer in the arena until a release.
//!
//! Positions: every block has a start position, and the position of the
//! arena is the start of its last block plus the bytes used in it (for a
//! large block, its size). Allocation only ever bumps inside the last block
//! or pushes a new one, so a later allocation always has a higher position
//! and a release to a mark is a truncation of the stack. A chunk that is no
//! longer last keeps its unused tail unused.
//!
//! The scratch arena holds the copies a loop keeps across a release: while
//! [`ignite_rt_arena_scratch`] is on, allocations go to it, so releasing the
//! task arena does not free them.
//!
//! Bump state. The bump pointer of the arena allocations currently go to
//! lives in a per-thread [`State`] that generated code reads and writes
//! directly ([`ignite_rt_arena_state`], `docs/llvm.md` section 5): it
//! allocates, takes marks, tests whether a release is due and releases
//! inside the last chunk without calling the runtime. The Rust side copies
//! the cursor back into the last chunk before it touches an arena and
//! republishes the state afterwards ([`with_task`]).
//!
//! Every block is allocated zeroed, so arena memory is always initialized
//! and a read can go straight into it.
//!
//! Chunks come from the kernel (`mmap`), not from `malloc`, and a chunk
//! that is not kept for reuse goes back with `munmap`. In a `malloc` heap a
//! freed chunk stays resident until `malloc_trim` (`ignite_io::released_memory`,
//! at most once per second), and a live chunk pins the freed memory below
//! it: ember's `FLUSHDB` copies every stored value into its task's arena,
//! and the 10 MB of chunks it frees at `task_end` stayed in the process's
//! RSS until the next trim (2026-11-04). Large blocks still come from
//! `malloc`: a block per allocation from `mmap` would be a system call each.

use std::alloc::Layout;
use std::cell::{RefCell, UnsafeCell};
use std::ffi::c_void;
use std::num::NonZeroUsize;
use std::ptr::NonNull;

use nix::sys::mman::{MapFlags, ProtFlags, mmap_anonymous, munmap};

/// Size of an ordinary chunk.
const CHUNK: usize = 64 * 1024;
/// Alignment of an ordinary chunk; larger alignments get their own block.
const CHUNK_ALIGN: usize = 16;
/// Requests above this size get their own block instead of a chunk.
const LARGE: usize = CHUNK / 4;
/// Released chunks an arena keeps for reuse; the rest go back to the
/// system allocator. Large blocks always go back.
const SPARE: usize = 16;
/// Chunks the task arena keeps for the next task on the same thread.
const SPARE_AFTER_TASK: usize = 1;
/// Bytes of large blocks (`malloc`) a task's arenas must hold at
/// `task_end` for the worker to return the freed heap pages to the kernel
/// (`ignite_io::released_memory`); chunks go back by `munmap` themselves.
const RELEASED_AT_TASK_END: usize = 1024 * 1024;
/// Bytes a loop allocates above its mark before a release is due, at
/// least (`ignite_rt_arena_due`).
const DUE: u64 = 64 * 1024;

/// The bump state of the arena allocations go to (`docs/llvm.md`
/// section 5). With a chunk last: `base` is the chunk, `cursor` the next
/// free byte, `limit` the chunk's end. Otherwise all three are null. In
/// both cases the arena's position is `cursor as i64 + offset`.
#[repr(C)]
#[derive(Debug)]
pub struct State {
    pub cursor: *mut u8,
    pub limit: *mut u8,
    pub offset: i64,
    pub base: *mut u8,
}

impl State {
    const EMPTY: State = State {
        cursor: std::ptr::null_mut(),
        limit: std::ptr::null_mut(),
        offset: 0,
        base: std::ptr::null_mut(),
    };
}

struct Block {
    pointer: NonNull<u8>,
    layout: Layout,
    start: u64,
    used: usize,
    chunk: bool,
}

struct Arena {
    blocks: Vec<Block>,
    spare: Vec<NonNull<u8>>,
    /// Bytes held from the kernel and from `malloc`, spare chunks included.
    reserved: usize,
    /// The part of `reserved` held in large blocks from `malloc`.
    heap: usize,
}

/// A zeroed chunk of `CHUNK` bytes from the kernel.
fn chunk_new() -> Result<NonNull<u8>, String> {
    let length = NonZeroUsize::new(CHUNK).expect("CHUNK is not zero");
    // SAFETY: a new private anonymous mapping aliases no memory; it is
    // page-aligned and zero-filled.
    unsafe {
        mmap_anonymous(
            None,
            length,
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_PRIVATE,
        )
    }
    .map(NonNull::cast)
    .map_err(|error| format!("allocation of {CHUNK} bytes failed: {error}"))
}

/// Give a chunk from [`chunk_new`] back to the kernel.
fn chunk_drop(pointer: NonNull<u8>) {
    // SAFETY: a mapping of `CHUNK` bytes made by `chunk_new` that nothing
    // uses any more. A failure would leave it mapped, which only leaks.
    let _ = unsafe { munmap(pointer.cast::<c_void>(), CHUNK) };
}

impl Arena {
    const fn new() -> Arena {
        Arena {
            blocks: Vec::new(),
            spare: Vec::new(),
            reserved: 0,
            heap: 0,
        }
    }

    fn chunk_layout() -> Layout {
        Layout::from_size_align(CHUNK, CHUNK_ALIGN).expect("chunk layout is valid")
    }

    /// The position after the last allocation.
    fn top(&self) -> u64 {
        self.blocks
            .last()
            .map_or(0, |block| block.start + block.used as u64)
    }

    /// Take the bump position of `state` into the last chunk.
    fn pull(&mut self, state: &State) {
        if state.base.is_null() {
            return;
        }
        if let Some(last) = self.blocks.last_mut()
            && last.chunk
            && last.pointer.as_ptr() == state.base
        {
            last.used = state.cursor as usize - state.base as usize;
        }
    }

    /// Publish the bump position of the last block in `state`.
    fn publish(&self, state: &mut State) {
        match self.blocks.last() {
            Some(block) if block.chunk => {
                let base = block.pointer.as_ptr();
                state.base = base;
                // SAFETY: `used` and `CHUNK` are within the chunk.
                state.cursor = unsafe { base.add(block.used) };
                state.limit = unsafe { base.add(CHUNK) };
                state.offset = block.start as i64 - base as i64;
            }
            _ => {
                *state = State::EMPTY;
                state.offset = self.top() as i64;
            }
        }
    }

    fn push(&mut self, layout: Layout, chunk: bool, used: usize) -> Result<*mut u8, String> {
        let reused = if chunk { self.spare.pop() } else { None };
        let pointer = match reused {
            Some(pointer) => pointer,
            None if chunk => {
                let pointer = chunk_new()?;
                self.reserved += CHUNK;
                pointer
            }
            None => {
                // SAFETY: `layout` has a non-zero size.
                let pointer = NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) })
                    .ok_or_else(|| format!("allocation of {} bytes failed", layout.size()))?;
                self.reserved += layout.size();
                self.heap += layout.size();
                pointer
            }
        };
        let start = self.top();
        self.blocks.push(Block {
            pointer,
            layout,
            start,
            used,
            chunk,
        });
        Ok(pointer.as_ptr())
    }

    fn bump(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        let block = self.blocks.last_mut().filter(|block| block.chunk)?;
        let address = block.pointer.as_ptr() as usize + block.used;
        let start = block.used + (address.next_multiple_of(align) - address);
        let end = start.checked_add(size)?;
        if end > CHUNK {
            return None;
        }
        block.used = end;
        // SAFETY: `start < CHUNK`, inside the chunk.
        Some(unsafe { block.pointer.as_ptr().add(start) })
    }

    fn alloc(&mut self, size: usize, align: usize) -> Result<*mut u8, String> {
        if !align.is_power_of_two() {
            return Err(format!(
                "allocation alignment {align} is not a power of two"
            ));
        }
        if size == 0 {
            return Ok(std::ptr::without_provenance_mut(align));
        }
        if let Some(pointer) = self.bump(size, align) {
            return Ok(pointer);
        }
        if size > LARGE || align > CHUNK_ALIGN {
            let layout = Layout::from_size_align(size, align)
                .map_err(|_| format!("allocation of {size} bytes is too large"))?;
            return self.push(layout, false, size);
        }
        self.push(Self::chunk_layout(), true, 0)?;
        Ok(self
            .bump(size, align)
            .expect("a fresh chunk holds a small request"))
    }

    fn free(&mut self, block: Block) {
        if block.chunk && self.spare.len() < SPARE {
            self.spare.push(block.pointer);
        } else if block.chunk {
            self.reserved -= CHUNK;
            chunk_drop(block.pointer);
        } else {
            self.reserved -= block.layout.size();
            self.heap -= block.layout.size();
            // SAFETY: allocated in `Arena::push` with this layout.
            unsafe { std::alloc::dealloc(block.pointer.as_ptr(), block.layout) };
        }
    }

    /// Free every allocation made after the position `mark`. A mark at or
    /// above the current position frees nothing.
    fn release(&mut self, mark: u64) {
        if mark >= self.top() {
            return;
        }
        while let Some(last) = self.blocks.last() {
            if last.start < mark {
                break;
            }
            let block = self.blocks.pop().expect("a last block");
            poison(block.pointer.as_ptr(), 0, block.used);
            self.free(block);
        }
        if let Some(last) = self.blocks.last_mut()
            && last.chunk
        {
            let used = last.used.min((mark - last.start) as usize);
            poison(last.pointer.as_ptr(), used, last.used);
            last.used = used;
        }
    }

    /// Free everything, keeping at most `keep` chunks for reuse.
    fn clear(&mut self, keep: usize) {
        self.release(0);
        while self.spare.len() > keep {
            let pointer = self.spare.pop().expect("a spare chunk");
            self.reserved -= CHUNK;
            chunk_drop(pointer);
        }
    }
}

/// Overwrite bytes `from..to` at `base` with 0xDB in a debug build, so a
/// read through a pointer into released memory fails loudly in tests
/// instead of seeing the old value. Release builds do nothing.
fn poison(base: *mut u8, from: usize, to: usize) {
    if cfg!(debug_assertions) && from < to {
        // SAFETY: `from..to` lies inside the block's used bytes.
        unsafe { std::ptr::write_bytes(base.add(from), 0xDB, to - from) };
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        for block in self.blocks.drain(..) {
            if block.chunk {
                chunk_drop(block.pointer);
            } else {
                // SAFETY: allocated in `Arena::push` with this layout.
                unsafe { std::alloc::dealloc(block.pointer.as_ptr(), block.layout) };
            }
        }
        for pointer in self.spare.drain(..) {
            chunk_drop(pointer);
        }
    }
}

/// A task's arena and its scratch arena.
struct Task {
    arena: Arena,
    scratch: Arena,
    in_scratch: bool,
}

impl Task {
    const fn new() -> Task {
        Task {
            arena: Arena::new(),
            scratch: Arena::new(),
            in_scratch: false,
        }
    }

    /// The arena allocations go to.
    fn current(&mut self) -> &mut Arena {
        if self.in_scratch {
            &mut self.scratch
        } else {
            &mut self.arena
        }
    }
}

thread_local! {
    static TASK: RefCell<Task> = const { RefCell::new(Task::new()) };
    static STATE: UnsafeCell<State> = const { UnsafeCell::new(State::EMPTY) };
}

fn state() -> *mut State {
    STATE.with(UnsafeCell::get)
}

/// Run `f` on the current task with its arenas in sync with the bump
/// state: the cursor is taken into the current arena before and the state
/// republished after. `None` once the thread's task storage is gone.
fn with_task<R>(f: impl FnOnce(&mut Task) -> R) -> Option<R> {
    TASK.try_with(|task| {
        let mut task = task.borrow_mut();
        // SAFETY: the state is this thread's, and generated code does not
        // run while the runtime holds this reference.
        let state = unsafe { &mut *state() };
        task.current().pull(state);
        let result = f(&mut task);
        task.current().publish(state);
        result
    })
    .ok()
}

/// `size` bytes aligned to `align` from the current task's arena (its
/// scratch arena while scratch is on). Failure (memory exhausted, a bad
/// alignment) ends the process like `ignite_rt_panic`.
pub(crate) fn alloc(size: usize, align: usize) -> *mut u8 {
    if size > 0 && size <= LARGE && align.is_power_of_two() && align <= CHUNK_ALIGN {
        // SAFETY: this thread's state; see `with_task`.
        let state = unsafe { &mut *state() };
        let cursor = state.cursor as usize;
        let start = cursor.next_multiple_of(align);
        if !state.cursor.is_null() && start + size <= state.limit as usize {
            state.cursor = state.cursor.wrapping_add(start + size - cursor);
            return state.cursor.wrapping_sub(size);
        }
    }
    let result = with_task(|task| task.current().alloc(size, align))
        .unwrap_or_else(|| Err(String::from("allocation after the task ended")));
    match result {
        Ok(pointer) => pointer,
        Err(message) => crate::process::die(&message),
    }
}

/// Give back the tail of the block of `size` bytes at `pointer` past its
/// first `used` bytes, when it is the last allocation of the current
/// arena's last chunk; otherwise do nothing.
pub(crate) fn trim(pointer: *mut u8, size: usize, used: usize) {
    // SAFETY: this thread's state; see `with_task`.
    let state = unsafe { &mut *state() };
    if used < size && !state.cursor.is_null() && pointer.wrapping_add(size) == state.cursor {
        state.cursor = pointer.wrapping_add(used);
    }
}

/// Bytes the current task's arenas hold from the system allocator,
/// including chunks kept for reuse.
pub fn arena_reserved() -> usize {
    with_task(|task| task.arena.reserved + task.scratch.reserved).unwrap_or(0)
}

/// `ptr arena_state()`: this thread's [`State`]. The address is fixed for
/// the life of the thread; generated code may read it once per function.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_arena_state() -> *mut State {
    state()
}

/// `ptr alloc(i64 size, i64 align)`: arena allocation of the current task.
/// A negative size or a non-power-of-two alignment is fatal (exit 101).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_alloc(size: i64, align: i64) -> *mut u8 {
    let (Ok(size), Ok(align)) = (usize::try_from(size), usize::try_from(align)) else {
        crate::process::die(&format!(
            "invalid allocation of {size} bytes aligned {align}"
        ));
    };
    alloc(size, align.max(1))
}

fn position(mark: i64) -> u64 {
    u64::try_from(mark).unwrap_or(0)
}

/// `i64 arena_mark()`: the current position of the task arena.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_arena_mark() -> i64 {
    with_task(|task| task.arena.top() as i64).unwrap_or(0)
}

/// `void arena_release(i64 mark)`: free everything the task arena handed
/// out after `mark`. Pointers into that memory are invalid afterwards. A
/// mark above the current position frees nothing. The scratch arena is not
/// touched.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_arena_release(mark: i64) {
    let _ = with_task(|task| task.arena.release(position(mark)));
}

/// `i8 arena_due(i64 mark, i64 kept)`: 1 when the task arena holds more
/// than `max(64 KiB, 4 * kept)` bytes above `mark`, where `kept` is what
/// the loop kept above the mark after its last release. A compaction copies
/// a carried list with its capacity equal to its length, so the next push
/// allocates twice `kept` at once; with a limit of 4 * kept that does not
/// trigger another compaction, the next doubling does. The copying is then
/// bounded by two thirds of what the loop allocates, and a loop that grows
/// a list stays linear.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_arena_due(mark: i64, kept: i64) -> i8 {
    let above = position(ignite_rt_arena_mark()).saturating_sub(position(mark));
    let limit = DUE.max(position(kept).saturating_mul(4));
    i8::from(above > limit)
}

/// `void arena_scratch(i8 on)`: with 1, later allocations of this task go
/// to its scratch arena; with 0, to its task arena again.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_arena_scratch(on: i8) {
    let _ = with_task(|task| task.in_scratch = on != 0);
}

/// `void arena_scratch_clear()`: free everything in the scratch arena.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_arena_scratch_clear() {
    let _ = with_task(|task| task.scratch.release(0));
}

/// `void task_end()`: free the current task's arenas. Every pointer they
/// handed out is invalid afterwards; later allocations start new arenas.
/// The thread keeps one chunk for the next task it runs; the other chunks
/// go back to the kernel at once. Arenas that held more than 1 MiB in large
/// blocks mark the memory released, so the worker trims the heap.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_task_end() {
    let _ = with_task(|task| {
        if task.arena.heap + task.scratch.heap > RELEASED_AT_TASK_END {
            ignite_io::released_memory();
        }
        task.in_scratch = false;
        task.arena.clear(SPARE_AFTER_TASK);
        task.scratch.clear(0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocations_are_aligned_and_disjoint() {
        let mut blocks = Vec::new();
        for i in 0..2000usize {
            let size = (i * 37) % 300 + 1;
            let align = 1 << (i % 6);
            let pointer = ignite_rt_alloc(size as i64, align as i64);
            assert_eq!(pointer as usize % align, 0);
            // SAFETY: a fresh arena block of `size` bytes.
            unsafe { std::ptr::write_bytes(pointer, (i % 251) as u8, size) };
            blocks.push((pointer, size, (i % 251) as u8));
        }
        for (pointer, size, fill) in blocks {
            // SAFETY: still inside the live arena.
            let bytes = unsafe { std::slice::from_raw_parts(pointer, size) };
            assert!(bytes.iter().all(|byte| *byte == fill));
        }
        ignite_rt_task_end();
    }

    #[test]
    fn large_and_overaligned_requests_get_own_blocks() {
        let big = ignite_rt_alloc(1 << 20, 8);
        assert!(!big.is_null());
        // SAFETY: 1 MiB block.
        unsafe { std::ptr::write_bytes(big, 7, 1 << 20) };
        let page = ignite_rt_alloc(100, 4096);
        assert_eq!(page as usize % 4096, 0);
        assert!(arena_reserved() >= 1 << 20);
        ignite_rt_task_end();
    }

    #[test]
    fn zero_size_is_aligned_and_non_null() {
        let pointer = ignite_rt_alloc(0, 8);
        assert!(!pointer.is_null());
        assert_eq!(pointer as usize % 8, 0);
    }

    #[test]
    fn task_end_frees_the_arena() {
        for _ in 0..100 {
            ignite_rt_alloc(1000, 8);
        }
        assert!(arena_reserved() >= 100_000);
        ignite_rt_task_end();
        assert!(arena_reserved() <= CHUNK);
        let pointer = ignite_rt_alloc(16, 8);
        assert!(!pointer.is_null());
        assert_eq!(arena_reserved(), CHUNK);
        ignite_rt_task_end();
    }

    /// Chunks are mappings of their own (page-aligned, not counted as
    /// `malloc` memory); large blocks are `malloc`'s. `task_end` gives all
    /// but one chunk back.
    #[test]
    fn chunks_come_from_the_kernel_and_large_blocks_from_malloc() {
        let heap = || with_task(|task| task.arena.heap + task.scratch.heap).unwrap();
        for _ in 0..40 {
            let pointer = ignite_rt_alloc(LARGE as i64, 8);
            // SAFETY: a fresh block of `LARGE` bytes.
            unsafe { std::ptr::write_bytes(pointer, 5, LARGE) };
            // SAFETY: this thread's state.
            let base = unsafe { (*ignite_rt_arena_state()).base };
            assert_eq!(base as usize % 4096, 0, "a chunk is a mapping of its own");
        }
        assert_eq!(heap(), 0);
        assert!(arena_reserved() >= 10 * CHUNK);
        ignite_rt_alloc(1 << 20, 8);
        assert!(heap() >= 1 << 20);
        ignite_rt_task_end();
        assert_eq!(heap(), 0);
        assert_eq!(arena_reserved(), CHUNK);
        let reused = ignite_rt_alloc(64, 8);
        // SAFETY: 64 fresh bytes in the kept chunk.
        unsafe { std::ptr::write_bytes(reused, 1, 64) };
        ignite_rt_task_end();
    }

    #[test]
    fn arenas_are_per_thread() {
        ignite_rt_alloc(100, 8);
        let mine = arena_reserved();
        let other = std::thread::spawn(arena_reserved).join().unwrap();
        assert_eq!(other, 0);
        assert_eq!(arena_reserved(), mine);
        ignite_rt_task_end();
    }

    #[test]
    fn release_frees_what_came_after_the_mark() {
        let before = ignite_rt_alloc(100, 8);
        // SAFETY: 100 fresh bytes.
        unsafe { std::ptr::write_bytes(before, 1, 100) };
        let mark = ignite_rt_arena_mark();
        for _ in 0..10 {
            ignite_rt_alloc(30_000, 8);
        }
        ignite_rt_alloc(1 << 20, 8);
        ignite_rt_alloc(100, 4096);
        assert!(ignite_rt_arena_mark() > mark);
        ignite_rt_arena_release(mark);
        assert_eq!(ignite_rt_arena_mark(), mark);
        // SAFETY: allocated before the mark, still live.
        let bytes = unsafe { std::slice::from_raw_parts(before, 100) };
        assert!(bytes.iter().all(|byte| *byte == 1));
        let again = ignite_rt_alloc(8, 8);
        assert_eq!(again as usize, before as usize + 104);
        ignite_rt_task_end();
    }

    #[test]
    fn a_loop_of_release_does_not_grow() {
        let mark = ignite_rt_arena_mark();
        for _ in 0..10 {
            for size in [100, 20_000, 70_000, 300] {
                ignite_rt_alloc(size, 8);
            }
            ignite_rt_arena_release(mark);
        }
        let settled = arena_reserved();
        for _ in 0..1000 {
            for size in [100, 20_000, 70_000, 300] {
                ignite_rt_alloc(size, 8);
            }
            ignite_rt_arena_release(mark);
        }
        assert_eq!(arena_reserved(), settled);
        ignite_rt_task_end();
    }

    #[test]
    fn spare_chunks_are_bounded() {
        let mark = ignite_rt_arena_mark();
        for _ in 0..100 {
            ignite_rt_alloc(LARGE as i64, 8);
        }
        ignite_rt_arena_release(mark);
        assert_eq!(arena_reserved(), SPARE * CHUNK);
        ignite_rt_task_end();
    }

    #[test]
    fn scratch_survives_a_release_of_the_task_arena() {
        let mark = ignite_rt_arena_mark();
        let inside = ignite_rt_alloc(64, 8);
        // SAFETY: 64 fresh bytes.
        unsafe { std::ptr::write_bytes(inside, 3, 64) };
        ignite_rt_arena_scratch(1);
        let kept = ignite_rt_alloc(64, 8);
        // SAFETY: 64 fresh bytes in the scratch arena.
        unsafe { std::ptr::copy_nonoverlapping(inside, kept, 64) };
        ignite_rt_arena_scratch(0);
        ignite_rt_arena_release(mark);
        assert_eq!(ignite_rt_arena_mark(), mark);
        let moved = ignite_rt_alloc(64, 8);
        // SAFETY: both live: `kept` in scratch, `moved` in the task arena.
        unsafe { std::ptr::copy_nonoverlapping(kept, moved, 64) };
        ignite_rt_arena_scratch_clear();
        // SAFETY: `moved` is live.
        let bytes = unsafe { std::slice::from_raw_parts(moved, 64) };
        assert!(bytes.iter().all(|byte| *byte == 3));
        ignite_rt_task_end();
    }

    #[test]
    fn release_is_due_above_the_larger_of_64_kib_and_four_times_kept() {
        let mark = ignite_rt_arena_mark();
        ignite_rt_alloc(60_000, 8);
        assert_eq!(ignite_rt_arena_due(mark, 0), 0);
        ignite_rt_alloc(10_000, 8);
        assert_eq!(ignite_rt_arena_due(mark, 0), 1);
        assert_eq!(ignite_rt_arena_due(mark, 40_000), 0);
        assert_eq!(ignite_rt_arena_due(mark, 20_000), 0);
        assert_eq!(ignite_rt_arena_due(mark, 17_000), 1);
        ignite_rt_task_end();
    }

    /// What generated code does with the state: bump the cursor, read the
    /// position, move the cursor back to a mark inside the chunk. The
    /// runtime sees the same positions afterwards.
    #[test]
    fn generated_code_and_runtime_share_the_bump_state() {
        ignite_rt_alloc(8, 8);
        let state = ignite_rt_arena_state();
        // SAFETY: this thread's state, used as generated code uses it.
        let state = unsafe { &mut *state };
        let position = |s: &State| s.cursor as i64 + s.offset;
        let mark = position(state);
        assert_eq!(mark, ignite_rt_arena_mark());
        let block = state.cursor;
        state.cursor = state.cursor.wrapping_add(100);
        assert_eq!(ignite_rt_arena_mark(), mark + 100);
        let next = ignite_rt_alloc(1, 1);
        assert_eq!(next, block.wrapping_add(100));
        assert_eq!(position(state), mark + 101);
        state.cursor = block;
        assert_eq!(ignite_rt_arena_mark(), mark);
        let read = alloc(4096, 1);
        trim(read, 4096, 10);
        assert_eq!(ignite_rt_arena_mark(), mark + 10);
        ignite_rt_task_end();
        assert_eq!(ignite_rt_arena_mark(), 0);
        assert_eq!(position(state), 0);
    }
}
