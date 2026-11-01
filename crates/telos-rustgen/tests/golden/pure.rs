#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]

pub fn add(a: &i64, b: &i64) -> i64 {
    let mut a = a.clone();
    let mut b = b.clone();
    a + b
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
        total = total + v;
    }
    return total.clone();
}

pub fn greet(name: &String) -> String {
    format!("{}{}", String::from("hello, "), name.clone())
}

pub fn sign(x: &i64) -> i64 {
    let mut x = x.clone();
    if x < 0 {
        -1
    } else if x == 0 {
        0
    } else {
        1
    }
}

pub fn countdown(n: &i64) -> Vec<i64> {
    let mut n = n.clone();
    let mut out = Vec::new();
    let mut i = n.clone();
    while i > 0 {
        out.push(i.clone());
        i = i - 1;
    }
    return out.clone();
}
