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
//! its locking are `ignite-io`'s. Results are copied into the calling task's
//! arena before the heap copy the store handed out is dropped.

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

/// The glue and layout of one element type.
#[derive(Clone, Copy)]
struct Glue {
    to_heap: ToHeap,
    from_heap: FromHeap,
    free: FreeHeap,
    size: usize,
    align: usize,
}

/// One stored value: a heap copy owned by this value.
struct HeapValue {
    ptr: *mut u8,
    glue: Glue,
}

// SAFETY: a heap copy shares no memory with any other value or task; the
// glue only reads it (clone) or frees it (drop), and the store's lock
// orders every access.
unsafe impl Send for HeapValue {}
// SAFETY: as for `Send`; `&HeapValue` only allows a clone, which reads.
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

impl HeapValue {
    /// Copy the value into the section-3 memory at `out`.
    fn copy_out(&self, out: *mut u8) {
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
/// process exits.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_store_new(
    to_heap: ToHeap,
    from_heap: FromHeap,
    free: FreeHeap,
    size: i64,
    align: i64,
) -> *const RtStore {
    let glue = Glue {
        to_heap,
        from_heap,
        free,
        size: usize::try_from(size).unwrap_or(0),
        align: usize::try_from(align).unwrap_or(1).max(1),
    };
    Box::into_raw(Box::new(RtStore {
        store: Store::new(),
        glue,
    }))
}

/// `i64 store_next_id(ptr store)`: 1, 2, 3, … per store.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_store_next_id(handle: *const RtStore) -> i64 {
    store(handle).store.next_id()
}

/// `void store_put(ptr store, i64 id, ptr value)`: store a copy of the
/// value at `value`, replacing any value under `id`.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_store_put(handle: *const RtStore, id: i64, value: *const u8) {
    let s = store(handle);
    let value = HeapValue {
        ptr: (s.glue.to_heap)(value),
        glue: s.glue,
    };
    s.store.put(id, value);
}

/// `i8 store_get(ptr store, i64 id, ptr out)`: 1 and a copy of the value in
/// `out`, or 0 when there is none.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_store_get(handle: *const RtStore, id: i64, out: *mut u8) -> i8 {
    match store(handle).store.get(id) {
        Some(value) => {
            value.copy_out(out);
            1
        }
        None => 0,
    }
}

/// `i8 store_remove(ptr store, i64 id, ptr out)`: 1 and the removed value
/// in `out`, or 0 when there was none.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_store_remove(handle: *const RtStore, id: i64, out: *mut u8) -> i8 {
    match store(handle).store.remove(id) {
        Some(value) => {
            value.copy_out(out);
            1
        }
        None => 0,
    }
}

/// `void store_values(ptr store, List* out)`: copies of every value in
/// ascending id order.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_store_values(handle: *const RtStore, out: *mut IgniteList) {
    let s = store(handle);
    let values = s.store.values();
    let size = s.glue.size;
    let buffer = arena::alloc(size * values.len(), s.glue.align);
    for (i, value) in values.iter().enumerate() {
        value.copy_out(buffer.wrapping_add(i * size));
    }
    put(
        out,
        IgniteList {
            ptr: buffer,
            len: values.len() as i64,
            cap: values.len() as i64,
        },
    );
}

/// `i8 store_update(ptr store, i64 id, ptr f, ptr thunk, ptr out)`: under
/// the store's lock, replace the value under `id` by `thunk(f, value)`; 1
/// and a copy of the new value in `out`, or 0 (and `f` not called) when
/// there is none.
#[unsafe(no_mangle)]
pub extern "C" fn ignite_rt_store_update(
    handle: *const RtStore,
    id: i64,
    f: *const u8,
    thunk: UpdateThunk,
    out: *mut u8,
) -> i8 {
    let s = store(handle);
    let updated = s.store.update(id, |old| HeapValue {
        ptr: thunk(f, old.ptr),
        glue: s.glue,
    });
    match updated {
        Some(value) => {
            value.copy_out(out);
            1
        }
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI64, Ordering};

    use super::*;

    static LIVE: AtomicI64 = AtomicI64::new(0);

    extern "C" fn to_heap(value: *const u8) -> *mut u8 {
        LIVE.fetch_add(1, Ordering::SeqCst);
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
        let s = ignite_rt_store_new(to_heap, from_heap, free, 8, 8);
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
            ignite_rt_store_update(s, 2, std::ptr::null(), double, (&raw mut out).cast()),
            1
        );
        assert_eq!(out, 14);
        assert_eq!(
            ignite_rt_store_update(s, 9, std::ptr::null(), double, (&raw mut out).cast()),
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
    }
}
