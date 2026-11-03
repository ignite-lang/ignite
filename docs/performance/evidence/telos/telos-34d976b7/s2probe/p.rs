fn a(s: String) -> i64 { match s { ref x if x == "a" => 1, _ => 2 } }
fn b(v: Vec<i64>) -> i64 { match v { x if let [p, q] = x.as_slice() => *p + *q, _ => 0 } }
fn c(s: String) -> i64 { match s { "a" => 1, _ => 2 } }
fn d(v: Vec<i64>) -> i64 { match v { [p, q] => p + q, _ => 0 } }
fn main() { println!("{} {}", a(String::from("a")), b(vec![1,2])); }
