#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]

#[derive(Clone, Debug)]
pub enum Shape {
    Circle(i64),
    Rect(i64, i64),
    Empty,
}

#[derive(Clone, Debug)]
pub struct Labelled {
    pub label: String,
    pub shape: Shape,
}

pub fn area(shape: &Shape) -> i64 {
    match shape.clone() {
        Shape::Circle(r) => (3 * r) * r,
        Shape::Rect(w, h) => w * h,
        Shape::Empty => 0,
    }
}

pub fn describe(item: &Labelled) -> String {
    let size = area(&item.shape);
    match item.shape.clone() {
        Shape::Empty => format!("{}{}", item.label, String::from(" is empty")),
        _ => format!("{}{}", format!("{}{}", item.label, String::from(": ")), size.clone().to_string()),
    }
}

pub fn unit_square() -> Labelled {
    Labelled { label: String::from("unit"), shape: Shape::Rect(1, 1) }
}

pub fn largest(items: &Vec<Labelled>) -> Option<Labelled> {
    let mut best = None;
    let mut best_area = 0 - 1;
    for item in (items.clone()) {
        let a = area(&item.shape);
        if a > best_area {
            best_area = a.clone();
            best = Some(item.clone());
        };
    }
    return best.clone();
}
