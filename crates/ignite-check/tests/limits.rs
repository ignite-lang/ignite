//! Cost bounds of the checker: effect inference over long call chains does
//! not depend on declaration order, and long expressions check.

use std::time::{Duration, Instant};

use ignite_check::{Modules, check_module};
use ignite_syntax::{SourceFile, parse};

/// `f0 -> f1 -> .. -> fn`, where only `fn` writes stdout; callers first
/// or callees first.
fn chain(n: usize, callers_first: bool) -> String {
    let mut fns: Vec<String> = (0..n)
        .map(|i| {
            format!(
                "fn f{i}() -> Result<Unit, IoError> {{\n  f{}()\n}}\n",
                i + 1
            )
        })
        .collect();
    fns.push(format!(
        "fn f{n}() -> Result<Unit, IoError> {{\n  Stdout.write_all(b\"x\")\n}}\n"
    ));
    if !callers_first {
        fns.reverse();
    }
    format!("package chain\n\n{}", fns.concat())
}

fn checked(text: &str) -> Duration {
    let source = SourceFile::new("t.ignite", text);
    let program = parse(&source).unwrap_or_else(|d| panic!("{}", source.render_all(&d)));
    let start = Instant::now();
    let symbols = check_module(&program, &Modules::new())
        .unwrap_or_else(|d| panic!("{}", source.render_all(&d)));
    let elapsed = start.elapsed();
    assert_eq!(
        symbols.effects["f0"].requires,
        ["stdout.write"],
        "the effect reaches the head of the chain"
    );
    elapsed
}

#[test]
fn effect_inference_is_linear_in_either_order() {
    for callers_first in [true, false] {
        let elapsed = checked(&chain(800, callers_first));
        assert!(
            elapsed < Duration::from_millis(500),
            "800 chained fns (callers first: {callers_first}) took {elapsed:?}"
        );
    }
}

#[test]
fn mutual_recursion_reaches_a_fixed_point() {
    let text = "package p\n\nfn a(n: Int) -> Int {\n  if n > 0 {\n    return b(n - 1)\n  }\n  return 0\n}\n\nfn b(n: Int) -> Int {\n  if n > 5 {\n    Stdout.write_all(b\"x\")\n  }\n  return a(n)\n}\n\nfn c() -> Int = a(3)\n";
    let source = SourceFile::new("t.ignite", text);
    let program = parse(&source).expect("parses");
    let symbols = check_module(&program, &Modules::new()).expect("checks");
    for name in ["a", "b", "c"] {
        assert_eq!(symbols.effects[name].requires, ["stdout.write"], "{name}");
        assert!(symbols.functions[name].effectful, "{name}");
    }
}
