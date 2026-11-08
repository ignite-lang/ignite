//! `Store<T>` (`docs/slice.md` section 11.3, `docs/llvm.md` section 5): the
//! C ABI over `ignite_io::Store<HeapValue>`. A [`HeapValue`] is a pointer to
//! a deep copy of a section-3 value whose buffers come from `malloc`, made
//! and freed by generator glue of the element type, which the generator
//! passes to [`ignite_rt_store_new`]:
//!
//! * `copy_to_heap.<T>(ptr value) -> ptr`: a heap copy of the value at
//!   `value` (in an arena or on the heap);
//! * `copy_from_heap.<T>(ptr dst, ptr heap)`: a copy of the heap value into
//!   `dst`, its buffers in the calling task's arena;
//! * `free_heap.<T>(ptr heap)`: free a heap copy and its buffers.
//!
//! Cloning a `HeapValue` is `copy_to_heap`, dropping it `free_heap`, so the
//! store's value semantics (a `get` returns a copy, a `put` stores one) and
//! its locking are `ignite-io`'s. Reads (`get`, `values`, the result of
//! `update`) use the store's borrowing forms and copy the stored heap value
//! straight into the calling task's arena under the lock: one copy per
//! value, no heap copy in between.

use ignite_io::Store;

use crate::{IgniteList, arena, put};

/// `copy_to_heap.<T>`.
pub type ToHeap = extern "C" fn(*const u8) -> *mut u8;
/// `copy_from_heap.<T>`.
pub type FromHeap = extern "C" fn(*mut u8, *const u8);
/// `free_heap.<T>`.
pub type FreeHeap = extern "C" fn(*mut u8);
/// `store_update.<T>(ptr f, ptr old) -> ptr`: the generated thunk of
/// `update` calling the Ignite fn `f` on a copy of the heap value `old` and
/// returning a heap copy of its result.
pub type UpdateThunk = extern "C" fn(*const u8, *const u8) -> *mut u8;
/// `store_upsert.<T>(ptr f, ptr old) -> ptr`: the generated thunk of
/// `upsert` calling the Ignite fn `f` on `None` when `old` is null, else on
/// `Some` of a copy of the heap value `old`, and returning a heap copy of
/// its result.
pub type UpsertThunk = extern "C" fn(*const u8, *const u8) -> *mut u8;

/// The glue and layout of one element type.
#[derive(Clone, Copy)]
pub(crate) struct Glue {
    to_heap: ToHeap,
    from_heap: FromHeap,
    free: FreeHeap,
    size: usize,
    align: usize,
}

/// One stored value: a heap copy owned by this value.
pub(crate) struct HeapValue {
    ptr: *mut u8,
    glue: Glue,
}

// SAFETY: a heap copy shares no memory with any other value or task; the
// glue only reads it (clone) or frees it (drop). A drop happens only once
// no task can reach the value: the store's lock orders it against every
// read through `read`, `read_values`, `replace_with` and `upsert_with`, and
// `ignite_io::Store`'s `update` and `upsert` (unused here) drop a taken
// value only after its last shared reference is gone.
unsafe impl Send for HeapValue {}
// SAFETY: as for `Send`; `&HeapValue` only allows a clone or a copy into an
// arena, which both only read, so two tasks may hold one at once: a value
// `ignite_io::Store`'s `update` or `upsert` has taken is read by its taker
// with no lock held while `values` reads it under the shard's lock.
unsafe impl Sync for HeapValue {}

impl Clone for HeapValue {
    fn clone(&self) -> HeapValue {
        HeapValue {
            ptr: (self.glue.to_heap)(self.ptr),
            glue: self.glue,
        }
    }
}

impl Drop for HeapValue {
    fn drop(&mut self) {
        (self.glue.free)(self.ptr);
    }
}

impl Glue {
    /// The glue a generated program passes for one element type; a null
    /// pointer is a panic the calling entry contains (`crate::contain`).
    pub(crate) fn new(
        to_heap: Option<ToHeap>,
        from_heap: Option<FromHeap>,
        free: Option<FreeHeap>,
        size: i64,
        align: i64,
    ) -> Glue {
        Glue {
            to_heap: to_heap.expect("a null copy_to_heap"),
            from_heap: from_heap.expect("a null copy_from_heap"),
            free: free.expect("a null free_heap"),
            size: usize::try_from(size).unwrap_or(0),
            align: usize::try_from(align).unwrap_or(1).max(1),
        }
    }
}

impl HeapValue {
    /// A heap copy of the section-3 value at `value`.
    pub(crate) fn copy_in(value: *const u8, glue: Glue) -> HeapValue {
        HeapValue {
            ptr: (glue.to_heap)(value),
            glue,
        }
    }

    /// Copy the value into the section-3 memory at `out`.
    pub(crate) fn copy_out(&self, out: *mut u8) {
        if !out.is_null() {
            (self.glue.from_heap)(out, self.ptr);
        }
    }
}

/// A store resource: the `ptr` a generated program holds.
pub struct RtStore {
    store: Store<HeapValue>,
    glue: Glue,
}

/// The store behind a `ptr` from [`ignite_rt_store_new`].
fn store<'a>(handle: *const RtStore) -> &'a RtStore {
    // SAFETY: the generator passes only pointers `store_new` returned, and
    // stores are never freed.
    unsafe { &*handle }
}

/// `ptr store_new(ptr copy_to_heap, ptr copy_from_heap, ptr free_heap,
/// i64 size, i64 align)`: an empty store of elements of `size` bytes
/// aligned to `align`. A store is a system resource and lives until the
/// process exits. A null glue pointer is a panic the entry contains: it
/// ends the calling task (`crate::contain`).
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_store_new(
    to_heap: Option<ToHeap>,
    from_heap: Option<FromHeap>,
    free: Option<FreeHeap>,
    size: i64,
    align: i64,
) -> *const RtStore {
    crate::entry(move || {
        let glue = Glue {
            to_heap: to_heap.expect("store_new of a null copy_to_heap"),
            from_heap: from_heap.expect("store_new of a null copy_from_heap"),
            free: free.expect("store_new of a null free_heap"),
            size: usize::try_from(size).unwrap_or(0),
            align: usize::try_from(align).unwrap_or(1).max(1),
        };
        Box::into_raw(Box::new(RtStore {
            store: Store::new(),
            glue,
        }))
    })
}

/// `i64 store_next_id(ptr store)`: 1, 2, 3, … per store.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_store_next_id(handle: *const RtStore) -> i64 {
    crate::entry(move || store(handle).store.next_id())
}

/// `void store_put(ptr store, i64 id, ptr value)`: store a copy of the
/// value at `value`, replacing any value under `id`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_store_put(handle: *const RtStore, id: i64, value: *const u8) {
    crate::entry(move || {
        let s = store(handle);
        let value = HeapValue {
            ptr: (s.glue.to_heap)(value),
            glue: s.glue,
        };
        s.store.put(id, value);
    })
}

/// `i8 store_get(ptr store, i64 id, ptr out)`: 1 and a copy of the value in
/// `out`, or 0 when there is none.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_store_get(handle: *const RtStore, id: i64, out: *mut u8) -> i8 {
    crate::entry(move || {
        let found = store(handle).store.read(id, |value| value.copy_out(out));
        i8::from(found.is_some())
    })
}

/// `i8 store_remove(ptr store, i64 id, ptr out)`: 1 and the removed value
/// in `out`, or 0 when there was none.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_store_remove(handle: *const RtStore, id: i64, out: *mut u8) -> i8 {
    crate::entry(move || match store(handle).store.remove(id) {
        Some(value) => {
            value.copy_out(out);
            1
        }
        None => 0,
    })
}

/// `void store_values(ptr store, List* out)`: copies of every value in
/// ascending id order.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_store_values(handle: *const RtStore, out: *mut IgniteList) {
    crate::entry(move || {
        let s = store(handle);
        let size = s.glue.size;
        let list = s.store.read_values(|values| {
            let len = values.len();
            let buffer = arena::alloc(size * len, s.glue.align);
            for (i, value) in values.enumerate() {
                value.copy_out(buffer.wrapping_add(i * size));
            }
            IgniteList {
                ptr: buffer,
                len: len as i64,
                cap: len as i64,
            }
        });
        put(out, list);
    })
}

/// `i8 store_update(ptr store, i64 id, ptr f, ptr thunk, ptr out)`: under
/// the store's lock, replace the value under `id` by `thunk(f, value)`; 1
/// and a copy of the new value in `out`, or 0 (and `f` not called) when
/// there is none. A null from the thunk is a failure of `f` its callback
/// guard stopped (`crate::fail`): the old value stays, the lock is
/// released, then the task ends with the failure's report.
/// A null `thunk` ends the task as a contained panic, as in `store_new`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_store_update(
    handle: *const RtStore,
    id: i64,
    f: *const u8,
    thunk: Option<UpdateThunk>,
    out: *mut u8,
) -> i8 {
    let updated = crate::entry(move || {
        let thunk = thunk.expect("store_update of a null thunk");
        let s = store(handle);
        s.store.try_replace_with(
            id,
            |old| match thunk(f, old.ptr) {
                ptr if ptr.is_null() => Err(()),
                ptr => Ok(HeapValue { ptr, glue: s.glue }),
            },
            |new| new.copy_out(out),
        )
    });
    match updated {
        Some(Ok(())) => 1,
        Some(Err(())) => crate::fail::resume(),
        None => 0,
    }
}

/// `void store_upsert(ptr store, i64 id, ptr f, ptr thunk, ptr out)`: under
/// the store's lock, store `thunk(f, value)` under `id`, the thunk getting
/// the stored heap value or null when `id` holds nothing, and copy the
/// stored value to `out`. A null from the thunk is a failure of `f` its
/// callback guard stopped, as for [`ignite_rt_store_update`]: the store is
/// unchanged, the lock is released, then the task ends with the report.
/// A null `thunk` ends the task as a contained panic.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_store_upsert(
    handle: *const RtStore,
    id: i64,
    f: *const u8,
    thunk: Option<UpsertThunk>,
    out: *mut u8,
) {
    let stored = crate::entry(move || {
        let thunk = thunk.expect("store_upsert of a null thunk");
        let s = store(handle);
        s.store.upsert_with(
            id,
            |old| match thunk(f, old.map_or(std::ptr::null(), |old| old.ptr)) {
                ptr if ptr.is_null() => Err(()),
                ptr => Ok(HeapValue { ptr, glue: s.glue }),
            },
            |new| new.copy_out(out),
        )
    });
    if stored.is_err() {
        crate::fail::resume()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI64, Ordering};

    use super::*;

    static LIVE: AtomicI64 = AtomicI64::new(0);
    static HEAP_COPIES: AtomicI64 = AtomicI64::new(0);

    extern "C" fn to_heap(value: *const u8) -> *mut u8 {
        LIVE.fetch_add(1, Ordering::SeqCst);
        HEAP_COPIES.fetch_add(1, Ordering::SeqCst);
        // SAFETY: the tests store `i64`s.
        Box::into_raw(Box::new(unsafe { value.cast::<i64>().read() })).cast()
    }

    extern "C" fn from_heap(dst: *mut u8, heap: *const u8) {
        // SAFETY: as above.
        unsafe { dst.cast::<i64>().write(heap.cast::<i64>().read()) };
    }

    extern "C" fn free(heap: *mut u8) {
        LIVE.fetch_sub(1, Ordering::SeqCst);
        // SAFETY: made by `to_heap`.
        drop(unsafe { Box::from_raw(heap.cast::<i64>()) });
    }

    extern "C" fn double(_f: *const u8, old: *const u8) -> *mut u8 {
        // SAFETY: as above.
        let doubled = unsafe { old.cast::<i64>().read() } * 2;
        to_heap((&raw const doubled).cast())
    }

    #[test]
    fn values_are_copied_in_and_out_and_freed() {
        let s = ignite_rt_store_new(Some(to_heap), Some(from_heap), Some(free), 8, 8);
        assert_eq!(ignite_rt_store_next_id(s), 1);
        assert_eq!(ignite_rt_store_next_id(s), 2);
        let seven: i64 = 7;
        ignite_rt_store_put(s, 2, (&raw const seven).cast());
        ignite_rt_store_put(s, 1, (&raw const seven).cast());
        let mut out: i64 = 0;
        assert_eq!(ignite_rt_store_get(s, 2, (&raw mut out).cast()), 1);
        assert_eq!(out, 7);
        assert_eq!(ignite_rt_store_get(s, 3, (&raw mut out).cast()), 0);
        assert_eq!(
            ignite_rt_store_update(s, 2, std::ptr::null(), Some(double), (&raw mut out).cast()),
            1
        );
        assert_eq!(out, 14);
        assert_eq!(
            ignite_rt_store_update(s, 9, std::ptr::null(), Some(double), (&raw mut out).cast()),
            0
        );
        let mut list = IgniteList {
            ptr: std::ptr::null_mut(),
            len: 0,
            cap: 0,
        };
        ignite_rt_store_values(s, &raw mut list);
        assert_eq!(list.len, 2);
        // SAFETY: two `i64`s in the arena.
        let values = unsafe { std::slice::from_raw_parts(list.ptr.cast::<i64>(), 2) };
        assert_eq!(values, [7, 14]);
        assert_eq!(ignite_rt_store_remove(s, 1, (&raw mut out).cast()), 1);
        assert_eq!(out, 7);
        assert_eq!(ignite_rt_store_remove(s, 1, (&raw mut out).cast()), 0);
        assert_eq!(LIVE.load(Ordering::SeqCst), 1);
        assert_eq!(
            HEAP_COPIES.load(Ordering::SeqCst),
            3,
            "two puts and the result of one update; reads make no heap copy"
        );
    }

    /// Heap copies alive in [`upsert_inserts_then_replaces`], counted apart
    /// from [`LIVE`] so that the two tests may run at once.
    static UPSERT_LIVE: AtomicI64 = AtomicI64::new(0);

    extern "C" fn upsert_to_heap(value: *const u8) -> *mut u8 {
        UPSERT_LIVE.fetch_add(1, Ordering::SeqCst);
        // SAFETY: the test stores `i64`s.
        Box::into_raw(Box::new(unsafe { value.cast::<i64>().read() })).cast()
    }

    extern "C" fn upsert_free(heap: *mut u8) {
        UPSERT_LIVE.fetch_sub(1, Ordering::SeqCst);
        // SAFETY: made by `upsert_to_heap`.
        drop(unsafe { Box::from_raw(heap.cast::<i64>()) });
    }

    /// `old + 1`, or 1 for a null `old`.
    extern "C" fn count(_f: *const u8, old: *const u8) -> *mut u8 {
        let next = if old.is_null() {
            1
        } else {
            // SAFETY: a non-null `old` is a stored `i64`.
            let old = unsafe { old.cast::<i64>().read() };
            old + 1
        };
        upsert_to_heap((&raw const next).cast())
    }

    #[test]
    fn upsert_inserts_then_replaces() {
        let s = ignite_rt_store_new(
            Some(upsert_to_heap),
            Some(from_heap),
            Some(upsert_free),
            8,
            8,
        );
        let mut out: i64 = 0;
        ignite_rt_store_upsert(s, 4, std::ptr::null(), Some(count), (&raw mut out).cast());
        assert_eq!(out, 1, "a null old for an empty id");
        ignite_rt_store_upsert(s, 4, std::ptr::null(), Some(count), (&raw mut out).cast());
        assert_eq!(out, 2);
        assert_eq!(ignite_rt_store_get(s, 4, (&raw mut out).cast()), 1);
        assert_eq!(out, 2);
        assert_eq!(
            UPSERT_LIVE.load(Ordering::SeqCst),
            1,
            "the replaced copy was freed"
        );
    }
}
