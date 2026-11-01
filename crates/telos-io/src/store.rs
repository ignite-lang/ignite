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

use std::collections::BTreeMap;
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
        self.map().insert(id, value);
    }

    /// The value under `id`, taken out of the store.
    pub fn remove(&self, id: i64) -> Option<T> {
        self.map().remove(&id)
    }

    /// The number of values held.
    pub fn len(&self) -> usize {
        self.map().len()
    }

    /// Whether the store holds no value.
    pub fn is_empty(&self) -> bool {
        self.map().is_empty()
    }
}

impl<T: Clone> Store<T> {
    /// A copy of the value under `id`.
    pub fn get(&self, id: i64) -> Option<T> {
        self.map().get(&id).cloned()
    }

    /// Copies of every value, in ascending id order.
    pub fn values(&self) -> Vec<T> {
        self.map().values().cloned().collect()
    }

    /// Replace the value under `id` by `f` of it, atomically: no other
    /// method runs on the store between reading the value and storing the
    /// result. The new value, or `None` (and `f` is not called) when `id`
    /// holds nothing. When `f` panics the old value stays.
    pub fn update(&self, id: i64, f: impl FnOnce(T) -> T) -> Option<T> {
        let mut map = self.map();
        let entry = map.get_mut(&id)?;
        let new = f(entry.clone());
        *entry = new.clone();
        Some(new)
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
    fn get_returns_a_copy() {
        let store = Store::new();
        store.put(1, vec![1]);
        let mut copy = store.get(1).unwrap();
        copy.push(2);
        assert_eq!(store.get(1), Some(vec![1]));
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
