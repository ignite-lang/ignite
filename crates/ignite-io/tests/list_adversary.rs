//! Adversary (unit `rust-shared-payload-lists`): `ignite_io::List` against
//! the claims of `docs/slice.md` section 6 where its own tests do not
//! reach: a list of values that hold lists of their own, copies changed on
//! two threads, and the traits it shares with `Vec`.

use ignite_io::List;

#[derive(Clone, Debug, PartialEq)]
struct Row {
    name: String,
    cells: List<String>,
}

fn row(name: &str, cells: &[&str]) -> Row {
    Row {
        name: name.to_string(),
        cells: List::from_vec(cells.iter().map(|c| c.to_string()).collect()),
    }
}

/// The first change of a shared list of rows copies the rows once, each
/// row's own list by an increment, and leaves the other holder as it was,
/// also when the change goes on inside one of the rows.
#[test]
fn a_change_at_depth_two_leaves_the_other_holder_and_shares_the_untouched_rows() {
    let a = List::from_vec(vec![row("r0", &["x", "y"]), row("r1", &["z"])]);
    let mut b = a.clone();
    b.make_mut()[0].cells.make_mut().push("w".to_string());
    assert_eq!(*a[0].cells, ["x", "y"]);
    assert_eq!(*b[0].cells, ["x", "y", "w"]);
    assert!(!a.is_shared() && !b.is_shared());
    assert_eq!(a[1].cells.as_ptr(), b[1].cells.as_ptr());
    assert!(a[1].cells.is_shared() && b[1].cells.is_shared());
    assert_ne!(a[0].cells.as_ptr(), b[0].cells.as_ptr());
    b.make_mut()[1] = row("r1", &[]);
    assert!(!a[1].cells.is_shared());
    assert_eq!(*a[1].cells, ["z"]);
    assert!(b[1].cells.is_empty());
}

/// `into_vec` of a shared list copies and leaves the other holder; of an
/// empty one gives an empty `Vec`; `from_vec` of an empty `Vec` is the
/// empty list, equal to an emptied one.
#[test]
fn into_vec_and_from_vec_at_the_boundaries() {
    let a = List::from_vec(vec![row("r", &["c"])]);
    let b = a.clone();
    let mut v = b.into_vec();
    v[0].cells.make_mut().clear();
    assert_eq!(*a[0].cells, ["c"]);
    let e: List<Row> = List::from_vec(Vec::new());
    assert!(e.clone().into_vec().is_empty());
    let mut f = List::from_vec(vec![row("r", &[])]);
    f.make_mut().clear();
    assert_eq!(e, f);
    assert_eq!(e, List::default());
    assert_eq!(format!("{e:?}"), format!("{:?}", Vec::<Row>::new()));
}

/// Two threads each change their copy of one list 1,000 times; at the end
/// each holds its own changes and the original is as it was.
#[test]
fn two_threads_change_their_copies_a_thousand_times() {
    let a = List::from_vec(vec![row("r", &["0"])]);
    let spawn = |tag: &'static str, mut copy: List<Row>| {
        std::thread::spawn(move || {
            for i in 0..1000 {
                let keep = copy.clone();
                copy.make_mut()[0]
                    .cells
                    .make_mut()
                    .push(format!("{tag}{i}"));
                copy.make_mut().push(row(tag, &[]));
                assert_eq!(keep.len() + 1, copy.len());
            }
            copy
        })
    };
    let one = spawn("a", a.clone());
    let two = spawn("b", a.clone());
    let one = one.join().expect("thread one");
    let two = two.join().expect("thread two");
    assert_eq!(*a[0].cells, ["0"]);
    assert_eq!(a.len(), 1);
    assert_eq!(one.len(), 1001);
    assert_eq!(two.len(), 1001);
    assert_eq!(one[0].cells.len(), 1001);
    assert_eq!(one[0].cells[1000], "a999");
    assert_eq!(two[0].cells[1], "b0");
    assert!(one[1..].iter().all(|r| r.name == "a"));
}

/// Equality and debug text are the `Vec`'s for the same elements, shared
/// or not.
#[test]
fn the_traits_agree_with_vec() {
    let v = vec![row("a", &["1"]), row("b", &[])];
    let l = List::from_vec(v.clone());
    let c = l.clone();
    assert_eq!(format!("{l:?}"), format!("{v:?}"));
    assert_eq!(format!("{c:#?}"), format!("{v:#?}"));
    assert_eq!(*c, v);
    assert!(l == c);
    let mut d = c.clone();
    d.make_mut().pop();
    assert!(l != d);
    assert_eq!(*List::<Row>::default(), Vec::<Row>::new());
}
