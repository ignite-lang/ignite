#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, clippy::all)]

#[derive(Clone)]
pub enum Shape {
    Circle(i64),
    Rect(i64, i64),
    Empty,
}

#[derive(Clone)]
pub struct Labelled {
    pub label: String,
    pub shape: Shape,
}

pub fn area(shape: &Shape) -> i64 {
    match shape {
        Shape::Circle(r) => { let r = (r).clone(); ignite_io::checked::mul((ignite_io::checked::mul(3, r, "crates/ignite-rustgen/tests/golden/shapes.ignite", 16, 18)), r, "crates/ignite-rustgen/tests/golden/shapes.ignite", 16, 18) },
        Shape::Rect(w, h) => { let w = (w).clone(); let h = (h).clone(); ignite_io::checked::mul(w, h, "crates/ignite-rustgen/tests/golden/shapes.ignite", 17, 19) },
        Shape::Empty => 0,
    }
}

pub fn describe(item: &Labelled) -> String {
    let size = area(&item.shape);
    match &item.shape {
        Shape::Empty => format!("{}{}", item.label, String::from(" is empty")),
        _ => format!("{}{}", format!("{}{}", item.label, String::from(": ")), size.clone().to_string()),
    }
}

pub fn unit_square() -> Labelled {
    Labelled { label: String::from("unit"), shape: Shape::Rect(1, 1) }
}

pub fn largest(items: &Vec<Labelled>) -> Option<Labelled> {
    let mut best = None;
    let mut best_area = ignite_io::checked::sub(0, 1, "crates/ignite-rustgen/tests/golden/shapes.ignite", 36, 19);
    for item in (items).iter() {
        let a = area(&item.shape);
        if a > best_area {
            best_area = a.clone();
            best = Some(item.clone());
        };
    }
    return best.clone();
}
