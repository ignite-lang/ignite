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

/// P2: a mention of an aggregate constant reads the module's once-made
/// heap copy; the constant's function is called only by its `.once`
/// helper. A scalar constant stays a call LLVM folds.
#[test]
fn constants_are_evaluated_once() {
    let (generated, _) = lower_with("copies", true);
    let program = &generated.program;
    let body = function(program, "tests.copies.count_constants");
    for name in ["GREETING", "TAGS", "NESTED", "HOLDER", "EMPTY"] {
        let constant = format!("@\"tests.copies.{name}\"(");
        assert!(!body.contains(&constant), "{name} is evaluated:\n{body}");
        assert!(
            body.contains(&format!(
                "load atomic ptr, ptr @\"tests.copies.{name}.cache\" acquire"
            )),
            "{name} is not read from its cache:\n{body}"
        );
        let callers = program
            .lines()
            .filter(|l| l.contains(&format!("call void {constant}")))
            .count();
        assert_eq!(callers, 1, "{name} is called outside its .once helper");
    }
}

#[test]
fn every_copies_test_runs() {
    let (generated, tests) = lower_with("copies", true);
    assert!(!tests.is_empty());
    for (i, test) in tests.iter().enumerate() {
        run_test_build(&format!("copies-{i}"), &generated, test);
    }
}
