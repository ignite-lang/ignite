//! `Store<T>` (`docs/slice.md` section 11.3): an in-memory map from `Int`
//! ids to values, shared by every task of a process.
//!
//! A clone shares the map, like the other substrate handles. The map is one
//! `BTreeMap` behind a `Mutex`, so `values` is in ascending id order and
//! every method sees a consistent map; ids come from a separate atomic
//! counter, so `next_id` never waits for the lock. Values have copy
//! semantics: `get` and `values` return clones, `put` stores the value it is
//! given. `update` holds the lock while it applies `f`, so two updates of
//! one id never lose a write. A task that panics while holding the lock does
//! not make the store unusable: the next caller takes the map as it is.
//! A `put` that replaces a value, a `remove` that finds one and an `update`
//! call [`crate::released_memory`], so the task's end returns the freed
//! heap pages to the kernel.
//!
//! `read`, `read_values` and `replace_with` are the borrowing forms of
//! `get`, `values` and `update`: their callbacks see the stored values under
//! the lock and copy only what they need, so a caller that makes its own
//! copy (the LLVM target's runtime, into a task's arena) makes one, not two.
//! A callback must not call the same store: the lock is not reentrant.

use std::collections::BTreeMap;
use std::collections::btree_map::Values;
use std::convert::Infallible;
use std::fmt;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

pub struct Store<T> {
    inner: Arc<Shared<T>>,
}

struct Shared<T> {
    values: Mutex<BTreeMap<i64, T>>,
    last_id: AtomicI64,
}

impl<T> Clone for Store<T> {
    fn clone(&self) -> Store<T> {
        Store {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> fmt::Debug for Store<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Store")
    }
}

impl<T> Default for Store<T> {
    fn default() -> Store<T> {
        Store::new()
    }
}

impl<T> Store<T> {
    /// An empty store whose first `next_id` is 1.
    pub fn new() -> Store<T> {
        Store {
            inner: Arc::new(Shared {
                values: Mutex::new(BTreeMap::new()),
                last_id: AtomicI64::new(0),
            }),
        }
    }

    fn map(&self) -> MutexGuard<'_, BTreeMap<i64, T>> {
        self.inner
            .values
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// 1, 2, 3, … per store; never the same id twice, from any task.
    pub fn next_id(&self) -> i64 {
        self.inner.last_id.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Insert `value` under `id`, replacing what was there.
    pub fn put(&self, id: i64, value: T) {
        let old = self.map().insert(id, value);
        if old.is_some() {
            crate::released_memory();
        }
    }

    /// The value under `id`, taken out of the store.
    pub fn remove(&self, id: i64) -> Option<T> {
        let old = self.map().remove(&id);
        if old.is_some() {
            crate::released_memory();
        }
        old
    }

    /// The number of values held.
    pub fn len(&self) -> usize {
        self.map().len()
    }

    /// Whether the store holds no value.
    pub fn is_empty(&self) -> bool {
        self.map().is_empty()
    }

    /// `read` of the value under `id`, borrowed under the lock, or `None`
    /// (and `read` is not called) when `id` holds nothing.
    pub fn read<R>(&self, id: i64, read: impl FnOnce(&T) -> R) -> Option<R> {
        self.map().get(&id).map(read)
    }

    /// `read` of every value in ascending id order, borrowed under the lock.
    pub fn read_values<R>(&self, read: impl FnOnce(Values<'_, i64, T>) -> R) -> R {
        read(self.map().values())
    }

    /// Replace the value under `id` by `f` of a borrow of it and return
    /// `read` of the new value, atomically: no other method runs on the
    /// store between reading the old value and `read` of the new one.
    /// `None` (and neither is called) when `id` holds nothing. When `f`
    /// panics the old value stays.
    pub fn replace_with<R>(
        &self,
        id: i64,
        f: impl FnOnce(&T) -> T,
        read: impl FnOnce(&T) -> R,
    ) -> Option<R> {
        let replaced = self.try_replace_with(id, |old| Ok::<T, Infallible>(f(old)), read);
        replaced.map(|result| match result {
            Ok(read) => read,
        })
    }

    /// [`Store::replace_with`] with an `f` that may fail: on `Err` the old
    /// value stays, `read` is not called and the lock is released before
    /// the error is returned. The LLVM target's runtime uses it to end a
    /// task whose `update` callback failed only after the lock is free.
    pub fn try_replace_with<R, E>(
        &self,
        id: i64,
        f: impl FnOnce(&T) -> Result<T, E>,
        read: impl FnOnce(&T) -> R,
    ) -> Option<Result<R, E>> {
        let mut map = self.map();
        let entry = map.get_mut(&id)?;
        match f(entry) {
            Ok(new) => {
                *entry = new;
                crate::released_memory();
                Some(Ok(read(entry)))
            }
            Err(error) => Some(Err(error)),
        }
    }
}

impl<T: Clone> Store<T> {
    /// A copy of the value under `id`.
    pub fn get(&self, id: i64) -> Option<T> {
        self.read(id, T::clone)
    }

    /// Copies of every value, in ascending id order.
    pub fn values(&self) -> Vec<T> {
        self.read_values(|values| values.cloned().collect())
    }

    /// Replace the value under `id` by `f` of it, atomically: no other
    /// method runs on the store between reading the value and storing the
    /// result. The new value, or `None` (and `f` is not called) when `id`
    /// holds nothing. When `f` panics the old value stays.
    pub fn update(&self, id: i64, f: impl FnOnce(T) -> T) -> Option<T> {
        self.replace_with(id, |old| f(old.clone()), T::clone)
    }
}

#[cfg(test)]
mod tests {
    use super::Store;

    #[test]
    fn ids_start_at_one_and_count_up() {
        let store: Store<i64> = Store::new();
        assert_eq!(
            (store.next_id(), store.next_id(), store.next_id()),
            (1, 2, 3)
        );
        let other: Store<i64> = Store::new();
        assert_eq!(other.next_id(), 1);
    }

    #[test]
    fn put_get_remove_values() {
        let store = Store::new();
        assert_eq!(store.get(1), None);
        store.put(3, "c".to_string());
        store.put(1, "a".to_string());
        store.put(2, "b".to_string());
        store.put(2, "B".to_string());
        assert_eq!(store.get(2), Some("B".to_string()));
        assert_eq!(store.values(), ["a", "B", "c"]);
        assert_eq!(store.remove(1), Some("a".to_string()));
        assert_eq!(store.remove(1), None);
        assert_eq!(store.values(), ["B", "c"]);
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn dropping_a_value_marks_memory_released() {
        let releases = || crate::task::RELEASES.with(std::cell::Cell::get);
        let start = releases();
        let store = Store::new();
        store.put(1, "a".to_string());
        assert_eq!(store.get(1), Some("a".to_string()));
        assert_eq!(store.remove(2), None);
        assert_eq!(store.update(2, |v| v), None);
        assert_eq!(releases(), start, "nothing was dropped");
        store.put(1, "b".to_string());
        assert_eq!(releases(), start + 1, "put replaced a value");
        store.update(1, |v| v + "c");
        assert_eq!(releases(), start + 2, "update replaced a value");
        assert_eq!(store.remove(1), Some("bc".to_string()));
        assert_eq!(releases(), start + 3, "remove took a value");
    }

    #[test]
    fn get_returns_a_copy() {
        let store = Store::new();
        store.put(1, vec![1]);
        let mut copy = store.get(1).unwrap();
        copy.push(2);
        assert_eq!(store.get(1), Some(vec![1]));
    }

    std::thread_local! {
        static CLONES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    #[derive(Debug, PartialEq)]
    struct Counted(i64);

    impl Clone for Counted {
        fn clone(&self) -> Counted {
            CLONES.with(|n| n.set(n.get() + 1));
            Counted(self.0)
        }
    }

    #[test]
    fn borrowing_reads_copy_nothing_and_owned_reads_copy_once() {
        let clones = || CLONES.with(std::cell::Cell::get);
        let store = Store::new();
        store.put(1, Counted(1));
        store.put(2, Counted(2));
        let start = clones();
        assert_eq!(store.read(1, |v| v.0), Some(1));
        assert_eq!(store.read(3, |_| -> i64 { panic!("called") }), None);
        let all: Vec<i64> = store.read_values(|values| values.map(|v| v.0).collect());
        assert_eq!(all, [1, 2]);
        assert_eq!(
            store.replace_with(2, |v| Counted(v.0 * 10), |v| v.0),
            Some(20)
        );
        assert_eq!(
            store.replace_with(9, |_| panic!("called"), |_| -> i64 { panic!("called") }),
            None
        );
        assert_eq!(clones(), start, "the borrowing forms made a copy");
        assert_eq!(store.get(2), Some(Counted(20)));
        assert_eq!(clones(), start + 1, "get copies the value once");
        assert_eq!(store.values(), [Counted(1), Counted(20)]);
        assert_eq!(clones(), start + 3, "values copies each value once");
        assert_eq!(store.update(1, |v| Counted(v.0 + 1)), Some(Counted(2)));
        assert_eq!(
            clones(),
            start + 5,
            "update copies the old and the new value"
        );
    }

    #[test]
    fn a_panic_in_replace_with_keeps_the_old_value() {
        let store: Store<i64> = Store::new();
        store.put(1, 5);
        let clone = store.clone();
        let result =
            std::thread::spawn(move || clone.replace_with(1, |_| panic!("boom"), |v| *v)).join();
        assert!(result.is_err());
        assert_eq!(store.read(1, |v| *v), Some(5));
    }

    #[test]
    fn a_failed_try_replace_with_keeps_the_old_value_and_frees_the_lock() {
        let releases = || crate::task::RELEASES.with(std::cell::Cell::get);
        let store: Store<i64> = Store::new();
        store.put(1, 5);
        let start = releases();
        let failed: Option<Result<i64, &str>> =
            store.try_replace_with(1, |_| Err("failed"), |_| panic!("read called"));
        assert_eq!(failed, Some(Err("failed")));
        assert_eq!(releases(), start, "nothing was dropped");
        assert_eq!(store.get(1), Some(5), "the old value stays");
        let missing: Option<Result<i64, &str>> =
            store.try_replace_with(2, |_| panic!("f called"), |v| *v);
        assert_eq!(missing, None);
        let replaced: Option<Result<i64, &str>> = store.try_replace_with(1, |v| Ok(v + 1), |v| *v);
        assert_eq!(replaced, Some(Ok(6)));
        assert_eq!(releases(), start + 1, "the old value was dropped");
    }

    #[test]
    fn update_missing_id_does_not_call_f() {
        let store: Store<i64> = Store::new();
        assert_eq!(store.update(7, |_| panic!("called")), None);
        store.put(7, 1);
        assert_eq!(store.update(7, |v| v + 1), Some(2));
        assert_eq!(store.get(7), Some(2));
    }

    #[test]
    fn concurrent_ids_and_updates_lose_nothing() {
        let store: Store<i64> = Store::new();
        store.put(0, 0);
        let threads: Vec<_> = (0..16)
            .map(|_| {
                let store = store.clone();
                std::thread::spawn(move || {
                    let mut ids = Vec::new();
                    for _ in 0..100 {
                        ids.push(store.next_id());
                        store.update(0, |v| {
                            std::thread::yield_now();
                            v + 1
                        });
                    }
                    ids
                })
            })
            .collect();
        let mut ids: Vec<i64> = threads
            .into_iter()
            .flat_map(|t| t.join().unwrap())
            .collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 1600);
        assert_eq!(ids.first(), Some(&1));
        assert_eq!(ids.last(), Some(&1600));
        assert_eq!(store.get(0), Some(1600));
    }

    #[test]
    fn a_panic_in_update_keeps_the_old_value_and_the_store() {
        let store: Store<i64> = Store::new();
        store.put(1, 5);
        let clone = store.clone();
        let result = std::thread::spawn(move || clone.update(1, |_| panic!("boom"))).join();
        assert!(result.is_err());
        assert_eq!(store.get(1), Some(5));
        assert_eq!(store.update(1, |v| v * 2), Some(10));
    }
}
