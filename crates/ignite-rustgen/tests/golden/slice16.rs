#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]

#[derive(Clone)]
pub struct State {
    pub items: ::ignite_io::List<i64>,
    pub count: i64,
}

pub fn fresh() -> State {
    State { items: ::ignite_io::List::new(), count: 0 }
}

pub fn step(state: State, x: &i64) -> State {
    let mut x = x.clone();
    let mut next = state;
    { let __p1 = x.clone(); (*next.items.make_mut()).push(__p1) };
    next.count = ::ignite_io::checked::add(next.count, 1, "crates/ignite-rustgen/tests/golden/slice16.ignite", 16, 16);
    next
}

pub fn run(n: &i64) -> State {
    let mut n = n.clone();
    let mut s = fresh();
    let mut i = 0;
    while i < n {
        s = step(s, &i);
        i = ::ignite_io::checked::add(i, 1, "crates/ignite-rustgen/tests/golden/slice16.ignite", 25, 9);
    }
    s
}

pub fn run_read_after() -> i64 {
    let s = fresh();
    let t = step(s.clone(), &1);
    ::ignite_io::checked::add(t.count, s.count, "crates/ignite-rustgen/tests/golden/slice16.ignite", 33, 3)
}

pub fn total(state: &State) -> i64 {
    ::ignite_io::checked::add(state.count, ((*state.items).len() as i64), "crates/ignite-rustgen/tests/golden/slice16.ignite", 36, 33)
}

pub fn ufcs() -> State {
    let s = fresh();
    step(s, &2)
}

pub struct Counter {}

impl Counter {
    pub fn serve(mut stream: ::ignite_io::TcpStream, mut application: State) -> ::std::result::Result<(), ::ignite_io::IoError> {
        { let __p2 = 1; (*application.items.make_mut()).push(__p2) };
        stream.write_all(((*application.items).len() as i64).to_string().into_bytes())
    }
}

pub fn served(stream: &::ignite_io::TcpStream) -> ::std::result::Result<(), ::ignite_io::IoError> {
    let s = fresh();
    Counter::serve(stream.clone(), s)
}

#[derive(Clone)]
pub struct Out {
    pub log: ::ignite_io::List<i64>,
}

#[derive(Clone)]
pub struct Pair {
    pub state: State,
    pub out: Out,
}

pub fn publish(o: Out) -> Out {
    let mut p = o;
    { let __p3 = 1; (*p.log.make_mut()).push(__p3) };
    p
}

pub fn advance(st: Pair) -> Pair {
    Pair { state: st.state, out: publish(st.out) }
}

#[derive(Clone)]
pub struct Actor {
    pub state: State,
    pub handle: ::std::sync::Arc<dyn Fn(State, i64) -> State + Send + Sync>,
}

pub fn run_actor(n: &i64) -> Actor {
    let mut n = n.clone();
    let mut a = Actor { state: fresh(), handle: (::std::sync::Arc::new(|__a0: State, __a1: i64| step(__a0, &__a1)) as ::std::sync::Arc<dyn Fn(State, i64) -> State + Send + Sync>) };
    let mut i = 0;
    while i < n {
        a.state = (a.handle)(a.state, i.clone());
        i = ::ignite_io::checked::add(i, 1, "crates/ignite-rustgen/tests/golden/slice16.ignite", 82, 9);
    }
    a
}

pub fn adder(n: &i64) -> ::std::sync::Arc<dyn Fn(i64) -> i64 + Send + Sync> {
    let mut n = n.clone();
    (::std::sync::Arc::new({ let n = n.clone(); move |x: i64| -> i64 { ::ignite_io::checked::add(x, n, "crates/ignite-rustgen/tests/golden/slice16.ignite", 87, 42) } }) as ::std::sync::Arc<dyn Fn(i64) -> i64 + Send + Sync>)
}

pub fn added() -> i64 {
    adder(&1)(2)
}
