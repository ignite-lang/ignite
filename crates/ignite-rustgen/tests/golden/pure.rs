#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]

pub fn add(a: &i64, b: &i64) -> i64 {
    let mut a = a.clone();
    let mut b = b.clone();
    ignite_io::checked::add(a, b, "crates/ignite-rustgen/tests/golden/pure.ignite", 4, 3)
}

pub fn clamp(x: &i64, low: &i64, high: &i64) -> i64 {
    let mut x = x.clone();
    let mut low = low.clone();
    let mut high = high.clone();
    if x < low {
        return low.clone();
    };
    if x > high {
        return high.clone();
    };
    x
}

pub fn sum(values: &Vec<i64>) -> i64 {
    let mut total = 0;
    for v in (values.clone()) {
        total = ignite_io::checked::add(total, v, "crates/ignite-rustgen/tests/golden/pure.ignite", 16, 13);
    }
    return total.clone();
}

pub fn greet(name: &String) -> String {
    format!("{}{}", String::from("hello, "), name.clone())
}

pub fn sign(x: &i64) -> i64 {
    let mut x = x.clone();
    if x < 0 {
        ignite_io::checked::neg(1, "crates/ignite-rustgen/tests/golden/pure.ignite", 26, 14)
    } else if x == 0 {
        0
    } else {
        1
    }
}

pub fn countdown(n: &i64) -> Vec<i64> {
    let mut n = n.clone();
    let mut out: Vec<i64> = Vec::new();
    let mut i = n.clone();
    while i > 0 {
        out.push(i.clone());
        i = ignite_io::checked::sub(i, 1, "crates/ignite-rustgen/tests/golden/pure.ignite", 34, 9);
    }
    return out;
}
