#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Color {
    Red,
    Green,
    Blue,
}

#[derive(Clone)]
pub struct Box_ {
    pub label: String,
    pub inner: Box<::std::option::Option<Box_>>,
}

#[derive(Clone)]
pub struct Holder {
    pub items: ::ignite_io::List<i64>,
}

pub fn describe(e: &::ignite_io::Exposure) -> String {
    match e {
        ::ignite_io::Exposure::Loopback => String::from("loopback"),
        ::ignite_io::Exposure::Private => String::from("private"),
        ::ignite_io::Exposure::Public => String::from("public"),
    }
}

pub fn pick(open: &bool) -> ::ignite_io::Exposure {
    let mut open = open.clone();
    if open {
        ::ignite_io::Exposure::Public
    } else {
        ::ignite_io::Exposure::Loopback
    }
}

pub fn pushed() -> Vec<Vec<i64>> {
    let mut lists = vec![vec![1], vec![2]];
    { let __p1 = 0; let __p2 = 9; (*::ignite_io::checked::at_mut(&mut lists, __p1, "crates/ignite-rustgen/tests/golden/targets.ignite", 24, 3)).push(__p2) };
    { let __p3 = 1; let __p4 = vec![3]; (*::ignite_io::checked::at_mut(&mut lists, __p3, "crates/ignite-rustgen/tests/golden/targets.ignite", 25, 3)).extend_from_slice(&__p4) };
    lists
}

pub fn held() -> Vec<Holder> {
    let mut holders = vec![Holder { items: ::ignite_io::List::from_vec(vec![]) }];
    { let __p5 = 0; let __p6 = 7; (*(*::ignite_io::checked::at_mut(&mut holders, __p5, "crates/ignite-rustgen/tests/golden/targets.ignite", 31, 3)).items.make_mut()).push(__p6) };
    holders
}

pub fn counted() -> ::std::collections::BTreeMap<Color, i64> {
    let mut m: ::std::collections::BTreeMap<Color, i64> = ::std::collections::BTreeMap::new();
    { m.insert(Color::Blue, 3); };
    { m.insert(Color::Red, 1); };
    m
}

pub fn boxed() -> Box_ {
    Box_ { label: String::from("outer"), inner: Box::new(Some(Box_ { label: String::from("inner"), inner: Box::new(None) })) }
}
