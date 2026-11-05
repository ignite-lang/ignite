//! `story:llvm-lowering-copies`: `tests/golden/copies.ignite` holds one
//! test per removed copy or allocation, each one that a wrong result of the
//! new lowering fails. Every test runs in a test build linked with
//! `libignite_rt.a` (the debug runtime, which overwrites released arena
//! memory), and the IR of the program module shows the copies gone.

use crate::golden::{function, lower_with, run_test_build};

/// P1: a `Text` chain, `+` and `.concat` alike, is one arena block of the
/// summed length and one `memcpy` per operand, with no runtime call.
#[test]
fn text_chains_copy_each_operand_once() {
    let (generated, _) = lower_with("copies", true);
    let program = &generated.program;
    assert!(
        !program.contains("call void @ignite_rt_text_concat("),
        "a runtime concatenation is left:\n{program}"
    );
    for symbol in ["tests.copies.four", "tests.copies.shout"] {
        let body = function(program, symbol);
        assert_eq!(body.matches("call ptr @ignite.alloc(").count(), 1, "{body}");
        assert!(!body.contains("@ignite.append"), "{body}");
        assert!(!body.contains("@ignite.reserve"), "{body}");
    }
    let four = function(program, "tests.copies.four");
    assert_eq!(four.matches("call void @llvm.memcpy").count(), 5, "{four}");
}

#[test]
fn every_copies_test_runs() {
    let (generated, tests) = lower_with("copies", true);
    assert!(!tests.is_empty());
    for (i, test) in tests.iter().enumerate() {
        run_test_build(&format!("copies-{i}"), &generated, test);
    }
}
