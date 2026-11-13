//! The storage of a `List<T>` that is the payload of an enum variant or a
//! field of a record on the Rust target (`docs/slice.md` section 6, design
//! D of `docs/value-copies.md`): the elements behind a reference count, so
//! that copying the value that holds the list is an increment instead of a
//! copy of every element.
//!
//! - An empty list holds no block. A non-empty one is an `Arc<Vec<T>>`:
//!   two blocks, the count with the `Vec` and the `Vec`'s buffer. A single
//!   block (count, length, capacity and elements together) needs an
//!   allocation of its own layout, which safe Rust does not give, and the
//!   substrate takes no new `unsafe` site; it would also end the reads as
//!   a `Vec` below, since no `Vec` can point into a block it did not
//!   allocate.
//! - The count's block is built in one function per element type,
//!   `block`, not inlined at each literal: a program pays the
//!   allocation's code and its unwind path once per element type
//!   (`docs/slice.md` section 6, "What a path that never shares pays").
//! - The count is atomic: a value crosses `spawn` and channels, so the
//!   list is `Send` and `Sync` when its elements are.
//! - Reading derefs to the `Vec`, so a payload or field is read where the
//!   generated code reads a `Vec` (`&Vec<T>` parameters, slices, indexes,
//!   `checked::at`, the equality and debug glue).
//! - There is no `DerefMut`: every change goes through [`List::make_mut`],
//!   which copies the elements once when the storage is shared and
//!   changes them in place when it is not. A change the generated code
//!   would make without it does not compile.

use std::ops::Deref;
use std::sync::Arc;

/// A list whose copies share their elements until one of them changes.
pub struct List<T>(Option<Arc<Vec<T>>>);

/// The count's block of `elements`, taken without a copy. Not inlined:
/// inlined at each literal it cost `terminald` 401 bytes in the fn that
/// builds its five replies, out of line 207 bytes once.
#[inline(never)]
fn block<T>(elements: Vec<T>) -> Arc<Vec<T>> {
    Arc::new(elements)
}

impl<T> List<T> {
    /// The empty list, which holds no block.
    #[inline]
    pub const fn new() -> Self {
        List(None)
    }

    /// The list of the elements of `elements`, taken without a copy; an
    /// empty `Vec` gives the empty list.
    #[inline]
    pub fn from_vec(elements: Vec<T>) -> Self {
        if elements.is_empty() {
            List(None)
        } else {
            List(Some(block(elements)))
        }
    }

    /// The list of a non-empty literal's elements, taken without a copy
    /// and without a test: the generator writes the literal `[]` as
    /// [`List::new`] and every other literal as `List::of(vec![..])`.
    #[inline]
    pub fn of(elements: Vec<T>) -> Self {
        debug_assert!(!elements.is_empty(), "the empty literal is List::new");
        List(Some(block(elements)))
    }

    /// Whether the elements are shared with another copy of the list.
    #[inline]
    pub fn is_shared(&self) -> bool {
        self.0.as_ref().is_some_and(|a| Arc::strong_count(a) > 1)
    }

    /// Removes every element: a shared list takes the empty storage and
    /// leaves the elements to the other copies, without copying them; an
    /// unshared one drops them in place and keeps its buffer.
    #[inline]
    pub fn clear(&mut self) {
        if let Some(shared) = &mut self.0 {
            match Arc::get_mut(shared) {
                Some(elements) => elements.clear(),
                None => self.0 = None,
            }
        }
    }
}

impl<T: Clone> List<T> {
    /// The elements to change in place: copied once first when another
    /// copy of the list shares them.
    #[inline]
    pub fn make_mut(&mut self) -> &mut Vec<T> {
        Arc::make_mut(self.0.get_or_insert_with(|| Arc::new(Vec::new())))
    }

    /// The elements as a `Vec`: taken without a copy when no other copy
    /// of the list shares them, else copied.
    #[inline]
    pub fn into_vec(self) -> Vec<T> {
        match self.0 {
            Some(shared) => Arc::unwrap_or_clone(shared),
            None => Vec::new(),
        }
    }
}

impl<T> Deref for List<T> {
    type Target = Vec<T>;

    #[inline]
    fn deref(&self) -> &Vec<T> {
        match &self.0 {
            Some(shared) => shared,
            None => const { &Vec::new() },
        }
    }
}

impl<T> Clone for List<T> {
    /// Another copy of the list: an increment, no element copied.
    #[inline]
    fn clone(&self) -> Self {
        List(self.0.clone())
    }
}

impl<T> Default for List<T> {
    #[inline]
    fn default() -> Self {
        List::new()
    }
}

impl<T> From<Vec<T>> for List<T> {
    #[inline]
    fn from(elements: Vec<T>) -> Self {
        List::from_vec(elements)
    }
}

impl<T: Clone> From<List<T>> for Vec<T> {
    #[inline]
    fn from(list: List<T>) -> Self {
        list.into_vec()
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for List<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: PartialEq> PartialEq for List<T> {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl<T: Eq> Eq for List<T> {}

#[cfg(test)]
mod tests {
    use super::List;

    #[test]
    fn an_empty_list_holds_no_block() {
        let list: List<String> = List::from_vec(Vec::new());
        assert!(list.0.is_none());
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert!(List::<String>::new().0.is_none());
    }

    #[test]
    fn a_literal_takes_its_elements_without_a_copy_and_changes_like_any_list() {
        let elements = vec![String::from("x"), String::from("y")];
        let at = elements.as_ptr();
        let a = List::of(elements);
        assert_eq!(a.as_ptr(), at);
        assert!(!a.is_shared());
        let mut b = a.clone();
        b.make_mut().push(String::from("z"));
        assert_eq!(*a, ["x", "y"]);
        assert_eq!(*b, ["x", "y", "z"]);
        assert_eq!(List::of(vec![1]), List::from_vec(vec![1]));
    }

    #[test]
    fn a_copy_shares_the_elements() {
        let a = List::from_vec(vec![String::from("x"), String::from("y")]);
        let b = a.clone();
        assert!(a.is_shared() && b.is_shared());
        assert_eq!(a.as_ptr(), b.as_ptr());
        assert_eq!(*b, ["x", "y"]);
    }

    #[test]
    fn the_first_change_after_a_share_copies_once_and_leaves_the_other_copy() {
        let a = List::from_vec(vec![String::from("x")]);
        let mut b = a.clone();
        b.make_mut().push(String::from("z"));
        assert!(!a.is_shared() && !b.is_shared());
        assert_eq!(*a, ["x"]);
        assert_eq!(*b, ["x", "z"]);
        b.make_mut().push(String::from("w"));
        b.make_mut()[0] = String::from("v");
        assert_eq!(*b, ["v", "z", "w"]);
        assert_eq!(*a, ["x"]);
    }

    #[test]
    fn an_unshared_list_changes_in_place() {
        let mut a = List::from_vec(Vec::with_capacity(4));
        a.make_mut().push(1);
        let at = a.as_ptr();
        a.make_mut().push(2);
        a.make_mut().push(3);
        assert_eq!(a.as_ptr(), at);
        assert_eq!(*a, [1, 2, 3]);
    }

    #[test]
    fn into_vec_takes_an_unshared_list_and_copies_a_shared_one() {
        let a = List::from_vec(vec![1, 2, 3]);
        let at = a.as_ptr();
        let v = a.into_vec();
        assert_eq!(v.as_ptr(), at);
        let b = List::from_vec(vec![4, 5]);
        let c = b.clone();
        let w = c.into_vec();
        assert_ne!(w.as_ptr(), b.as_ptr());
        assert_eq!(w, [4, 5]);
        assert_eq!(*b, [4, 5]);
    }

    #[test]
    fn clearing_a_shared_list_takes_the_empty_storage_and_leaves_the_other_copy() {
        let a = List::from_vec(vec![String::from("x"), String::from("y")]);
        let mut b = a.clone();
        b.clear();
        assert!(b.0.is_none() && b.is_empty());
        assert!(!a.is_shared());
        assert_eq!(*a, ["x", "y"]);
    }

    #[test]
    fn clearing_an_unshared_list_keeps_its_buffer() {
        let mut elements = Vec::with_capacity(8);
        elements.push(1);
        let mut a = List::from_vec(elements);
        let at = a.as_ptr();
        a.clear();
        assert!(a.is_empty());
        assert!(a.capacity() >= 8);
        a.make_mut().push(2);
        assert_eq!(a.as_ptr(), at);
        assert_eq!(*a, [2]);
    }

    #[test]
    fn a_list_crosses_threads() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<List<String>>();
        let a = List::from_vec(vec![String::from("x")]);
        let b = a.clone();
        let joined = std::thread::spawn(move || b.len()).join().expect("joined");
        assert_eq!(joined, 1);
        assert!(!a.is_shared());
    }
}
