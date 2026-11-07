//! `Store<T>` (`docs/slice.md` section 11.3): an in-memory map from `Int`
//! ids to values, shared by every task of a process.
//!
//! A clone shares the map, like the other substrate handles. The map is
//! split by a hash of the id into [`SHARDS`] shards, each a `BTreeMap`
//! behind its own `Mutex`; ids come from a separate atomic counter, so
//! `next_id` never waits for a lock. Values have copy semantics: `get` and
//! `values` return clones, `put` stores the value it is given.
//!
//! Every method is atomic for the ids it touches. `update` and `upsert`
//! hold their shard's lock only to take the value out, leaving its slot
//! marked taken with the value shared, and to put the new value back: they
//! copy the old value, apply `f` and drop the old value with no lock held.
//! A task that reaches a taken id (any method on that id) waits until it is
//! put back, so two updates or upserts of one id never lose a write. When
//! `f` panics the old value is put back, and an upsert of an absent id
//! leaves it absent.
//!
//! `len`, `is_empty` and `values` wait for no taken slot: they lock every
//! shard in shard order and see one consistent map, in which an update or
//! upsert in progress has not happened yet (a taken slot counts with the
//! value it held; an absent id that an upsert took is absent), and release
//! the locks when done; `values` is in ascending id order across the
//! shards. So a task waits for another task's `f` only on the same id, and
//! for another task's lock only while that task holds it: on the task's
//! own shard, or on any shard while a `len`, `is_empty` or `values` reads
//! the map. The value of a taken slot is read by two tasks at once, so a
//! store is shared between tasks only when `T` is `Sync`.
//!
//! A task that panics while holding a lock does not make the store
//! unusable: the next caller takes the shard as it is. A `put` that
//! replaces a value, a `remove` that finds one and an `update` or `upsert`
//! that replaces one call [`crate::released_memory`], so the task's end
//! returns the freed heap pages to the kernel.
//!
//! `read`, `read_values`, `replace_with` and `upsert_with` are the borrowing
//! forms of `get`, `values`, `update` and `upsert`: their callbacks see the
//! stored values under the shard's lock (every shard's for `read_values`)
//! and copy only what they need, so a caller that makes its own copy (the
//! LLVM target's runtime, into a task's arena) makes one, not two. No
//! callback, and no `f` of `update` or `upsert`, may call the same store:
//! the locks are not reentrant, and a taken slot waits for its taker.

use std::collections::BTreeMap;
use std::collections::btree_map::{self, Entry};
use std::convert::Infallible;
use std::fmt;
use std::iter::Peekable;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

/// The number of shards of every store, a power of two.
const SHARDS: usize = 16;

const _: () = assert!(SHARDS.is_power_of_two());

/// The shard of `id`: the top bits of a Fibonacci hash, so ids in sequence
/// or in a stride spread over every shard.
fn shard_of(id: i64) -> usize {
    ((id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - SHARDS.trailing_zeros())) as usize
}

pub struct Store<T> {
    inner: Arc<Shared<T>>,
}

struct Shared<T> {
    shards: [Shard<T>; SHARDS],
    last_id: AtomicI64,
}

struct Shard<T> {
    map: Mutex<Map<T>>,
    /// Notified when a taken slot is put back while a task waits.
    returned: Condvar,
}

struct Map<T> {
    slots: BTreeMap<i64, Slot<T>>,
    /// Slots an `upsert` has taken for an absent id: `Slot::Taken(None)`.
    taken_absent: usize,
    /// Tasks waiting in [`Shard::returned`].
    waiting: usize,
}

impl<T> Map<T> {
    /// The number of values held, a taken slot counting with its value.
    fn len(&self) -> usize {
        self.slots.len() - self.taken_absent
    }
}

enum Slot<T> {
    Held(T),
    /// Taken out by an `update` or `upsert` in progress, with the value it
    /// held, shared with the taker so that `len`, `is_empty` and `values`
    /// need not wait for it; `None` for an absent id an `upsert` took.
    Taken(Option<Arc<T>>),
}

impl<T> Slot<T> {
    fn value(&self) -> &T {
        match self {
            Slot::Held(value) => value,
            Slot::Taken(_) => unreachable!("a taken slot seen by a task that waited for it"),
        }
    }

    fn value_mut(&mut self) -> &mut T {
        match self {
            Slot::Held(value) => value,
            Slot::Taken(_) => unreachable!("a taken slot seen by a task that waited for it"),
        }
    }

    /// The value held, or held before an `update` or `upsert` took it.
    fn last(&self) -> Option<&T> {
        match self {
            Slot::Held(value) => Some(value),
            Slot::Taken(old) => old.as_deref(),
        }
    }
}

impl<T> Shard<T> {
    fn new() -> Shard<T> {
        Shard {
            map: Mutex::new(Map {
                slots: BTreeMap::new(),
                taken_absent: 0,
                waiting: 0,
            }),
            returned: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Map<T>> {
        self.map.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The map once `ready` holds of it, waiting for put-backs until then.
    fn lock_when(&self, ready: impl Fn(&Map<T>) -> bool) -> MutexGuard<'_, Map<T>> {
        let mut map = self.lock();
        while !ready(&map) {
            map.waiting += 1;
            map = self
                .returned
                .wait(map)
                .unwrap_or_else(PoisonError::into_inner);
            map.waiting -= 1;
        }
        map
    }

    /// The map once `id`'s slot is not taken.
    fn lock_id(&self, id: i64) -> MutexGuard<'_, Map<T>> {
        self.lock_when(|map| !matches!(map.slots.get(&id), Some(Slot::Taken(_))))
    }

    /// Fill the taken slot of `id` with `value()`, or remove it for `None`,
    /// and wake the waiting tasks. `value` runs once the slot's share of
    /// the taken value is dropped, so the taker's share is the only one.
    fn put_back(&self, id: i64, value: impl FnOnce() -> Option<T>) {
        let mut map = self.lock();
        if let Some(Slot::Taken(None)) = map.slots.remove(&id) {
            map.taken_absent -= 1;
        }
        if let Some(value) = value() {
            map.slots.insert(id, Slot::Held(value));
        }
        if map.waiting > 0 {
            self.returned.notify_all();
        }
    }
}

/// A slot an `update` or `upsert` took out of its shard, with the old
/// value: [`Taken::put_back`] stores the new value, and a taker that
/// panics before that puts the old one back on drop.
struct Taken<'a, T> {
    shard: &'a Shard<T>,
    id: i64,
    old: Option<Arc<T>>,
    done: bool,
}

impl<'a, T> Taken<'a, T> {
    /// Take the value of `id` out of `shard`, waiting while another task
    /// has it; an absent `id` is taken (`old` is `None`) when `absent` is
    /// true and is `None` otherwise.
    fn take(shard: &'a Shard<T>, id: i64, absent: bool) -> Option<Taken<'a, T>> {
        let mut map = shard.lock_id(id);
        let old = match map.slots.entry(id) {
            Entry::Occupied(mut slot) => {
                match std::mem::replace(slot.get_mut(), Slot::Taken(None)) {
                    Slot::Held(old) => {
                        let old = Arc::new(old);
                        *slot.get_mut() = Slot::Taken(Some(Arc::clone(&old)));
                        Some(old)
                    }
                    Slot::Taken(_) => {
                        unreachable!("a taken slot seen by a task that waited for it")
                    }
                }
            }
            Entry::Vacant(slot) if absent => {
                slot.insert(Slot::Taken(None));
                map.taken_absent += 1;
                None
            }
            Entry::Vacant(_) => return None,
        };
        Some(Taken {
            shard,
            id,
            old,
            done: false,
        })
    }

    /// Store `new` in the slot; the old value, for the caller to drop with
    /// no lock held.
    fn put_back(mut self, new: T) -> Option<Arc<T>> {
        self.done = true;
        self.shard.put_back(self.id, || Some(new));
        // The slot's share went with the put-back, under the lock; the
        // taker's is now the last, so the caller's drop of it drops the old
        // value with no lock held.
        debug_assert!(
            self.old
                .as_ref()
                .is_none_or(|old| Arc::strong_count(old) == 1),
            "the slot still shares the old value after its put-back"
        );
        self.old.take()
    }

    /// A copy of the old value, made with no lock held.
    fn copy(&self) -> Option<T>
    where
        T: Clone,
    {
        self.old.as_deref().cloned()
    }
}

impl<T> Drop for Taken<'_, T> {
    fn drop(&mut self) {
        if !self.done {
            let old = self.old.take();
            self.shard.put_back(self.id, || {
                old.map(|old| Arc::into_inner(old).expect("the taker holds the last share"))
            });
        }
    }
}

/// Every value of a store in ascending id order, borrowed while
/// [`Store::read_values`] holds every shard's lock: a merge of the shards.
pub struct Values<'a, T> {
    shards: [Peekable<btree_map::Iter<'a, i64, Slot<T>>>; SHARDS],
    left: usize,
}

impl<'a, T> Values<'a, T> {
    fn new(maps: &'a [MutexGuard<'_, Map<T>>; SHARDS]) -> Values<'a, T> {
        Values {
            shards: std::array::from_fn(|i| maps[i].slots.iter().peekable()),
            left: maps.iter().map(|map| map.len()).sum(),
        }
    }
}

impl<'a, T> Iterator for Values<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        let mut least: Option<(usize, i64)> = None;
        for (i, shard) in self.shards.iter_mut().enumerate() {
            while shard.next_if(|(_, slot)| slot.last().is_none()).is_some() {}
            if let Some(&(&id, _)) = shard.peek()
                && least.is_none_or(|(_, least)| id < least)
            {
                least = Some((i, id));
            }
        }
        let (i, _) = least?;
        self.left -= 1;
        self.shards[i].next().and_then(|(_, slot)| slot.last())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left, Some(self.left))
    }
}

impl<T> ExactSizeIterator for Values<'_, T> {}

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
                shards: std::array::from_fn(|_| Shard::new()),
                last_id: AtomicI64::new(0),
            }),
        }
    }

    fn shard(&self, id: i64) -> &Shard<T> {
        &self.inner.shards[shard_of(id)]
    }

    /// Every shard's map, locked in shard order; taken slots are read with
    /// the value they held.
    fn lock_every(&self) -> [MutexGuard<'_, Map<T>>; SHARDS] {
        std::array::from_fn(|i| self.inner.shards[i].lock())
    }

    /// 1, 2, 3, … per store; never the same id twice, from any task.
    pub fn next_id(&self) -> i64 {
        self.inner.last_id.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Insert `value` under `id`, replacing what was there.
    pub fn put(&self, id: i64, value: T) {
        let old = self
            .shard(id)
            .lock_id(id)
            .slots
            .insert(id, Slot::Held(value));
        if old.is_some() {
            crate::released_memory();
        }
    }

    /// The value under `id`, taken out of the store.
    pub fn remove(&self, id: i64) -> Option<T> {
        match self.shard(id).lock_id(id).slots.remove(&id) {
            Some(Slot::Held(old)) => {
                crate::released_memory();
                Some(old)
            }
            Some(Slot::Taken(_)) => unreachable!("a taken slot seen by a task that waited for it"),
            None => None,
        }
    }

    /// The number of values held.
    pub fn len(&self) -> usize {
        self.lock_every().iter().map(|map| map.len()).sum()
    }

    /// Whether the store holds no value.
    pub fn is_empty(&self) -> bool {
        self.lock_every().iter().all(|map| map.len() == 0)
    }

    /// `read` of the value under `id`, borrowed under the lock, or `None`
    /// (and `read` is not called) when `id` holds nothing.
    pub fn read<R>(&self, id: i64, read: impl FnOnce(&T) -> R) -> Option<R> {
        self.shard(id)
            .lock_id(id)
            .slots
            .get(&id)
            .map(|slot| read(slot.value()))
    }

    /// `read` of every value in ascending id order, borrowed under every
    /// shard's lock.
    pub fn read_values<R>(&self, read: impl FnOnce(Values<'_, T>) -> R) -> R {
        let maps = self.lock_every();
        read(Values::new(&maps))
    }

    /// Replace the value under `id` by `f` of a borrow of it and return
    /// `read` of the new value, atomically: no other method runs on `id`
    /// between reading the old value and `read` of the new one. `None`
    /// (and neither is called) when `id` holds nothing. When `f` panics
    /// the old value stays.
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
        let mut map = self.shard(id).lock_id(id);
        let slot = map.slots.get_mut(&id)?.value_mut();
        let replaced = match f(slot) {
            Ok(new) => {
                let old = std::mem::replace(slot, new);
                Ok((read(slot), old))
            }
            Err(error) => Err(error),
        };
        drop(map);
        Some(replaced.map(|(read, old)| {
            drop(old);
            crate::released_memory();
            read
        }))
    }

    /// Store `f` of a borrow of the value under `id`, or of `None` when it
    /// holds nothing, under `id` and return `read` of it, atomically: no
    /// other method runs on `id` between reading the old value and `read`
    /// of the new one. On `Err` from `f` the store is unchanged, `read` is
    /// not called and the lock is released before the error is returned;
    /// when `f` panics the store is unchanged too.
    pub fn upsert_with<R, E>(
        &self,
        id: i64,
        f: impl FnOnce(Option<&T>) -> Result<T, E>,
        read: impl FnOnce(&T) -> R,
    ) -> Result<R, E> {
        let mut map = self.shard(id).lock_id(id);
        let (read, old) = match map.slots.entry(id) {
            Entry::Occupied(mut slot) => {
                let new = f(Some(slot.get().value()))?;
                let old = std::mem::replace(slot.get_mut().value_mut(), new);
                (read(slot.get().value()), Some(old))
            }
            Entry::Vacant(slot) => {
                let new = f(None)?;
                (read(slot.insert(Slot::Held(new)).value()), None)
            }
        };
        drop(map);
        if old.is_some() {
            drop(old);
            crate::released_memory();
        }
        Ok(read)
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
    /// method runs on `id` between reading the value and storing the
    /// result. The new value, or `None` (and `f` is not called) when `id`
    /// holds nothing. When `f` panics the old value stays.
    pub fn update(&self, id: i64, f: impl FnOnce(T) -> T) -> Option<T> {
        self.update_read(id, f, T::clone)
    }

    /// [`Store::update`] that returns `read` of the new value instead of a
    /// copy of it: the Rust target's form of an `update` whose result is
    /// discarded (`read` is `|_| ()`). The value is taken out of its slot
    /// under the shard's lock; `f`, the copy it gets and `read` run with no
    /// lock held, while other tasks that reach `id` wait.
    pub fn update_read<R>(
        &self,
        id: i64,
        f: impl FnOnce(T) -> T,
        read: impl FnOnce(&T) -> R,
    ) -> Option<R> {
        let taken = Taken::take(self.shard(id), id, false)?;
        let new = f(taken.copy().expect("an update takes a held value"));
        let read = read(&new);
        drop(taken.put_back(new));
        crate::released_memory();
        Some(read)
    }

    /// Store `f` of a copy of the value under `id`, or of `None` when it
    /// holds nothing, under `id`, atomically: no other method runs on `id`
    /// between reading the value and storing the result, so two tasks that
    /// upsert one new id lose neither write. The stored value. When `f`
    /// panics the store is unchanged.
    pub fn upsert(&self, id: i64, f: impl FnOnce(Option<T>) -> T) -> T {
        self.upsert_read(id, f, T::clone)
    }

    /// [`Store::upsert`] that returns `read` of the stored value instead
    /// of a copy of it: the Rust target's form of an `upsert` whose result
    /// is discarded (`read` is `|_| ()`) or read for one field (`read`
    /// copies the field). Taken out of its slot as for
    /// [`Store::update_read`].
    pub fn upsert_read<R>(
        &self,
        id: i64,
        f: impl FnOnce(Option<T>) -> T,
        read: impl FnOnce(&T) -> R,
    ) -> R {
        let taken = Taken::take(self.shard(id), id, true).expect("an upsert takes an absent id");
        let new = f(taken.copy());
        let read = read(&new);
        if taken.put_back(new).is_some() {
            crate::released_memory();
        }
        read
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
    fn upsert_inserts_then_replaces_and_marks_only_a_replacement() {
        let releases = || crate::task::RELEASES.with(std::cell::Cell::get);
        let store: Store<String> = Store::new();
        let start = releases();
        let append = |old: Option<String>| old.map_or("a".to_string(), |v| v + "b");
        assert_eq!(store.upsert(1, append), "a");
        assert_eq!(releases(), start, "an insert drops nothing");
        assert_eq!(store.upsert(1, append), "ab");
        assert_eq!(releases(), start + 1, "a replacement drops the old value");
        assert_eq!(store.get(1), Some("ab".to_string()));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn a_failed_or_panicking_upsert_leaves_the_store_unchanged() {
        let store: Store<i64> = Store::new();
        store.put(1, 5);
        let failed: Result<i64, &str> =
            store.upsert_with(1, |_| Err("failed"), |_| panic!("read called"));
        assert_eq!(failed, Err("failed"));
        let failed: Result<i64, &str> =
            store.upsert_with(2, |_| Err("failed"), |_| panic!("read called"));
        assert_eq!(failed, Err("failed"));
        let clone = store.clone();
        let result = std::thread::spawn(move || clone.upsert(3, |_| panic!("boom"))).join();
        assert!(result.is_err());
        assert_eq!(store.values(), [5], "neither id 2 nor id 3 was inserted");
        assert_eq!(store.upsert(1, |v| v.unwrap_or(0) * 2), 10);
    }

    #[test]
    fn concurrent_upserts_of_one_new_id_lose_nothing() {
        let store: Store<i64> = Store::new();
        let threads: Vec<_> = (0..16)
            .map(|_| {
                let store = store.clone();
                std::thread::spawn(move || {
                    for _ in 0..100 {
                        store.upsert(9, |v| {
                            std::thread::yield_now();
                            v.unwrap_or(0) + 1
                        });
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(store.get(9), Some(1600));
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

    std::thread_local! {
        static PROBE: std::cell::RefCell<Option<Box<dyn Fn() -> bool>>> =
            const { std::cell::RefCell::new(None) };
        static PROBED: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    }

    /// A value whose clone asks another thread to read the store and
    /// counts whether that read got through: a clone made while the
    /// cloning task holds the store's lock counts as blocked.
    #[derive(Debug, PartialEq)]
    struct Probed(i64);

    impl Clone for Probed {
        fn clone(&self) -> Probed {
            let free = PROBE.with(|probe| probe.borrow().as_ref().map(|probe| probe()));
            if let Some(free) = free {
                PROBED.with(|n| {
                    let (passed, blocked) = n.get();
                    n.set(if free {
                        (passed + 1, blocked)
                    } else {
                        (passed, blocked + 1)
                    });
                });
            }
            Probed(self.0)
        }
    }

    /// The load of `docs/performance/store-lock/README.md`: 8 threads,
    /// 100,000 upserts each, over 1,000 ids.
    #[test]
    #[ignore]
    fn contended_upserts_8_threads_1000_ids() {
        const THREADS: i64 = 8;
        const UPSERTS: i64 = 100_000;
        const IDS: i64 = 1_000;
        let store: Store<Vec<i64>> = Store::new();
        let start = std::time::Instant::now();
        let threads: Vec<_> = (0..THREADS)
            .map(|t| {
                let store = store.clone();
                std::thread::spawn(move || {
                    for i in 0..UPSERTS {
                        let id = (i * 7 + t * 131) % IDS;
                        store.upsert_read(
                            id,
                            |old| {
                                let mut v = old.unwrap_or_else(|| vec![0; 8]);
                                v[0] += 1;
                                v
                            },
                            |_| (),
                        );
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let elapsed = start.elapsed();
        let total: i64 = store.read_values(|values| values.map(|v| v[0]).sum());
        assert_eq!(total, THREADS * UPSERTS);
        assert_eq!(store.len(), IDS as usize);
        eprintln!(
            "contended_upserts: {THREADS} threads x {UPSERTS} upserts on {IDS} ids in {:.3} s",
            elapsed.as_secs_f64()
        );
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

    #[test]
    fn values_are_in_ascending_id_order_across_every_shard() {
        let mut ids: Vec<i64> = (0..400i64).map(|i| (i * 7919) % 1000 - 500).collect();
        ids.extend([i64::MIN, i64::MAX, -1, 0, 1]);
        ids.sort_unstable();
        ids.dedup();
        let shards: std::collections::BTreeSet<usize> =
            ids.iter().map(|&id| super::shard_of(id)).collect();
        assert_eq!(shards.len(), super::SHARDS, "the ids miss a shard");
        let store: Store<i64> = Store::new();
        for &id in ids.iter().rev() {
            store.put(id, id);
        }
        assert_eq!(store.values(), ids);
        let (len, read) = store.read_values(|values| (values.len(), values.copied().collect()));
        assert_eq!(len, ids.len());
        let read: Vec<i64> = read;
        assert_eq!(read, ids);
        assert_eq!(store.len(), ids.len());
        assert!(!store.is_empty());
        for &id in &ids {
            assert_eq!(store.remove(id), Some(id));
        }
        assert!(store.is_empty());
        assert_eq!(store.values(), Vec::<i64>::new());
    }

    fn ids_in_shard(shard: usize, count: usize) -> Vec<i64> {
        (1..)
            .filter(|&id| super::shard_of(id) == shard)
            .take(count)
            .collect()
    }

    fn overlapping_updates(
        store: &Store<i64>,
        ids: &[i64],
        until: std::time::Instant,
    ) -> Vec<std::thread::JoinHandle<()>> {
        ids.iter()
            .enumerate()
            .map(|(i, &id)| {
                let store = store.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(10 * i as u64));
                    while std::time::Instant::now() < until {
                        store.update(id, |v| {
                            std::thread::sleep(std::time::Duration::from_millis(20));
                            v + 1
                        });
                    }
                })
            })
            .collect()
    }

    #[test]
    fn values_returns_while_two_updates_of_one_shard_overlap() {
        let store: Store<i64> = Store::new();
        let hot = ids_in_shard(super::SHARDS - 1, 2);
        for &id in &hot {
            store.put(id, 0);
        }
        let until = std::time::Instant::now() + std::time::Duration::from_millis(2000);
        let updaters = overlapping_updates(&store, &hot, until);
        std::thread::sleep(std::time::Duration::from_millis(100));
        let started = std::time::Instant::now();
        let values = store.values();
        let waited = started.elapsed();
        for updater in updaters {
            updater.join().unwrap();
        }
        assert_eq!(values.len(), 2);
        assert!(
            waited < std::time::Duration::from_millis(500),
            "values waited {waited:?}: it returned only when the updates of its shard stopped"
        );
    }

    #[test]
    fn a_pending_values_does_not_stop_a_read_of_an_idle_shard() {
        let store: Store<i64> = Store::new();
        let hot = ids_in_shard(super::SHARDS - 1, 2);
        let idle = ids_in_shard(0, 1)[0];
        for &id in hot.iter().chain([&idle]) {
            store.put(id, 0);
        }
        let until = std::time::Instant::now() + std::time::Duration::from_millis(2000);
        let updaters = overlapping_updates(&store, &hot, until);
        std::thread::sleep(std::time::Duration::from_millis(100));
        let lister = store.clone();
        let values = std::thread::spawn(move || lister.values().len());
        std::thread::sleep(std::time::Duration::from_millis(100));
        let started = std::time::Instant::now();
        let read = store.get(idle);
        let waited = started.elapsed();
        for updater in updaters {
            updater.join().unwrap();
        }
        assert_eq!(values.join().unwrap(), 3);
        assert_eq!(read, Some(0));
        assert!(
            waited < std::time::Duration::from_millis(300),
            "a get of shard 0 waited {waited:?} behind a values call waiting for shard {}",
            super::SHARDS - 1
        );
    }

    #[test]
    fn update_and_upsert_copy_no_value_while_holding_the_lock_of_their_own_shard() {
        let [id, neighbour] = ids_in_shard(super::shard_of(1), 2)[..] else {
            unreachable!()
        };
        assert_ne!(super::shard_of(1), super::shard_of(2));
        let store: Store<Probed> = Store::new();
        store.put(id, Probed(1));
        store.put(neighbour, Probed(2));
        let prober = store.clone();
        PROBE.with(|probe| {
            *probe.borrow_mut() = Some(Box::new(move || {
                let store = prober.clone();
                let (done, finished) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let _ = done.send(store.read(neighbour, |v| v.0));
                });
                finished
                    .recv_timeout(std::time::Duration::from_millis(500))
                    .is_ok()
            }));
        });
        PROBED.with(|n| n.set((0, 0)));
        assert_eq!(store.update(id, |v| Probed(v.0 + 1)), Some(Probed(2)));
        assert_eq!(
            store.upsert(id, |v| Probed(v.map_or(0, |v| v.0) * 10)),
            Probed(20)
        );
        PROBE.with(|probe| *probe.borrow_mut() = None);
        assert_eq!(
            PROBED.with(std::cell::Cell::get),
            (4, 0),
            "clones made while another task could read the shard, and made while it could not"
        );
    }

    #[test]
    fn a_put_back_wakes_the_task_waiting_for_its_id_when_another_id_of_the_shard_has_a_waiter() {
        let store: Store<i64> = Store::new();
        let [first, second] = ids_in_shard(3, 2)[..] else {
            unreachable!()
        };
        let take = |id: i64| {
            let (taken, started) = std::sync::mpsc::channel();
            let (go, wait) = std::sync::mpsc::channel::<()>();
            let taker = store.clone();
            let upsert = std::thread::spawn(move || {
                taker.upsert(id, |v| {
                    taken.send(()).unwrap();
                    wait.recv().unwrap();
                    v.unwrap_or(0) + 1
                })
            });
            started.recv().unwrap();
            (go, upsert)
        };
        let get = |id: i64| {
            let (got, result) = std::sync::mpsc::channel();
            let reader = store.clone();
            std::thread::spawn(move || got.send(reader.get(id)));
            std::thread::sleep(std::time::Duration::from_millis(100));
            result
        };
        let (go_second, second_upsert) = take(second);
        let (go_first, first_upsert) = take(first);
        let got_second = get(second);
        let got_first = get(first);
        go_first.send(()).unwrap();
        assert_eq!(first_upsert.join().unwrap(), 1);
        let woken = got_first.recv_timeout(std::time::Duration::from_millis(1000));
        go_second.send(()).unwrap();
        assert_eq!(second_upsert.join().unwrap(), 1);
        let _ = got_second.recv_timeout(std::time::Duration::from_millis(1000));
        assert_eq!(
            woken,
            Ok(Some(1)),
            "the get of the put-back id stayed asleep while the other id was taken"
        );
    }

    #[test]
    fn values_len_and_is_empty_see_the_map_before_the_updates_in_progress() {
        let store: Store<i64> = Store::new();
        let [absent, held] = ids_in_shard(5, 2)[..] else {
            unreachable!()
        };
        store.put(held, 7);
        let hold = |id: i64| {
            let (taken, started) = std::sync::mpsc::channel();
            let (go, wait) = std::sync::mpsc::channel::<()>();
            let taker = store.clone();
            let upsert = std::thread::spawn(move || {
                taker.upsert(id, |v| {
                    taken.send(()).unwrap();
                    wait.recv().unwrap();
                    v.unwrap_or(0) + 1
                })
            });
            started.recv().unwrap();
            (go, upsert)
        };
        let (go_absent, absent_upsert) = hold(absent);
        let (go_held, held_upsert) = hold(held);
        let (seen, snapshot) = std::sync::mpsc::channel();
        let reader = store.clone();
        std::thread::spawn(move || seen.send((reader.values(), reader.len(), reader.is_empty())));
        let during = snapshot.recv_timeout(std::time::Duration::from_millis(1000));
        for go in [go_absent, go_held] {
            go.send(()).unwrap();
        }
        assert_eq!(
            (absent_upsert.join().unwrap(), held_upsert.join().unwrap()),
            (1, 8)
        );
        assert_eq!(during, Ok((vec![7], 1, false)));
        assert_eq!((store.values(), store.len()), (vec![1, 8], 2));
    }

    #[test]
    fn a_task_waiting_on_a_panicking_upsert_sees_the_old_value() {
        let store: Store<i64> = Store::new();
        store.put(1, 5);
        for id in [1, 2] {
            let (taken, started) = std::sync::mpsc::channel();
            let (go, wait) = std::sync::mpsc::channel::<()>();
            let taker = store.clone();
            let upsert = std::thread::spawn(move || {
                taker.upsert(id, |_| {
                    taken.send(()).unwrap();
                    wait.recv().unwrap();
                    panic!("boom")
                })
            });
            started.recv().unwrap();
            let reader = store.clone();
            let get = std::thread::spawn(move || (reader.get(id), reader.len()));
            let writer = store.clone();
            let update = std::thread::spawn(move || writer.update(id, |v| v + 1));
            go.send(()).unwrap();
            assert!(upsert.join().is_err());
            let (got, len) = get.join().unwrap();
            let updated = update.join().unwrap();
            if id == 1 {
                assert!(got == Some(5) || got == Some(6), "{got:?}");
                assert_eq!(updated, Some(6));
                assert_eq!(store.get(1), Some(6));
            } else {
                assert_eq!(got, None);
                assert_eq!(updated, None);
                assert_eq!(store.get(2), None);
            }
            assert_eq!(len, 1);
        }
        assert_eq!(store.upsert(1, |v| v.unwrap_or(0) * 2), 12);
        assert_eq!(store.len(), 1);
    }

    std::thread_local! {
        static DROP_PROBE: std::cell::RefCell<Option<Box<dyn Fn() -> bool>>> =
            const { std::cell::RefCell::new(None) };
        static DROP_PROBED: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    }

    /// A value whose drop asks another thread to read the store and counts
    /// whether that read got through: a drop made while the dropping task
    /// holds the shard's lock counts as blocked.
    #[derive(Clone, Debug, PartialEq)]
    struct DropProbed(i64);

    impl Drop for DropProbed {
        fn drop(&mut self) {
            let free = DROP_PROBE
                .try_with(|probe| probe.borrow().as_ref().map(|probe| probe()))
                .ok()
                .flatten();
            if let Some(free) = free {
                DROP_PROBED.with(|n| {
                    let (passed, blocked) = n.get();
                    n.set(if free {
                        (passed + 1, blocked)
                    } else {
                        (passed, blocked + 1)
                    });
                });
            }
        }
    }

    /// The module doc: `update` and `upsert` drop the old value with no
    /// lock held. Red against a `Taken::put_back` that drops `old` before
    /// it puts the new value back (the slot's share is then the last one
    /// and the value is dropped under the shard's lock).
    #[test]
    fn update_and_upsert_drop_the_old_value_with_no_lock_held_of_their_own_shard() {
        let [id, neighbour] = ids_in_shard(super::shard_of(1), 2)[..] else {
            unreachable!()
        };
        let store: Store<DropProbed> = Store::new();
        store.put(id, DropProbed(1));
        store.put(neighbour, DropProbed(2));
        let prober = store.clone();
        DROP_PROBE.with(|probe| {
            *probe.borrow_mut() = Some(Box::new(move || {
                let store = prober.clone();
                let (done, finished) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let _ = done.send(store.read(neighbour, |v| v.0));
                });
                finished
                    .recv_timeout(std::time::Duration::from_millis(500))
                    .is_ok()
            }));
        });
        DROP_PROBED.with(|n| n.set((0, 0)));
        let updated = store.update(id, |v| DropProbed(v.0 + 1)).map(|v| v.0);
        let upserted = store
            .upsert(id, |v| DropProbed(v.map_or(0, |v| v.0) * 10))
            .0;
        DROP_PROBE.with(|probe| *probe.borrow_mut() = None);
        let (passed, blocked) = DROP_PROBED.with(std::cell::Cell::get);
        assert_eq!((updated, upserted), (Some(2), 20));
        assert!(passed >= 4, "the probe ran {passed} times");
        assert_eq!(
            blocked, 0,
            "values dropped while the task held its shard's lock ({passed} without)"
        );
    }

    /// `read_values` during an upsert of an absent id: the iterator's
    /// length counts what it yields. Red against `Values::new` counting
    /// `slots.len()` (the taken absent id), which `values` (a `collect`)
    /// does not notice.
    #[test]
    fn read_values_len_counts_no_absent_id_an_upsert_took() {
        let store: Store<i64> = Store::new();
        let [absent, held] = ids_in_shard(7, 2)[..] else {
            unreachable!()
        };
        store.put(held, 7);
        let (taken, started) = std::sync::mpsc::channel();
        let (go, wait) = std::sync::mpsc::channel::<()>();
        let taker = store.clone();
        let upsert = std::thread::spawn(move || {
            taker.upsert(absent, |v| {
                taken.send(()).unwrap();
                wait.recv().unwrap();
                v.unwrap_or(0) + 1
            })
        });
        started.recv().unwrap();
        let (len, yielded) = store.read_values(|values| {
            let len = values.len();
            (len, values.count())
        });
        go.send(()).unwrap();
        assert_eq!(upsert.join().unwrap(), 1);
        assert_eq!((len, yielded), (1, 1));
    }

    /// The module doc: an absent id that an upsert took is absent, so a
    /// store whose only slot is such an id is empty. Red against an
    /// `is_empty` that asks the shard's `slots` instead of `Map::len`.
    #[test]
    fn is_empty_while_an_upsert_holds_only_an_absent_id() {
        let store: Store<i64> = Store::new();
        let (taken, started) = std::sync::mpsc::channel();
        let (go, wait) = std::sync::mpsc::channel::<()>();
        let taker = store.clone();
        let upsert = std::thread::spawn(move || {
            taker.upsert(9, |v| {
                taken.send(()).unwrap();
                wait.recv().unwrap();
                v.unwrap_or(0) + 1
            })
        });
        started.recv().unwrap();
        let during = (store.is_empty(), store.len(), store.values());
        go.send(()).unwrap();
        assert_eq!(upsert.join().unwrap(), 1);
        assert_eq!(during, (true, 0, Vec::<i64>::new()));
        assert!(!store.is_empty());
    }
}
