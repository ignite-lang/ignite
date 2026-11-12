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

/// P8: a `Map<Text, _>` probe compares keys inline (`@ignite.text_cmp`, a
/// `memcmp` of the common length), not through a runtime call.
#[test]
fn text_keys_compare_inline() {
    let (generated, _) = lower_with("copies", true);
    let program = &generated.program;
    assert!(
        !program.contains("call i32 @ignite_rt_text_cmp("),
        "a runtime comparison is left:\n{program}"
    );
    let find = program
        .split("\ndefine ")
        .find(|f| f.starts_with("internal i64 @\"map.find.") && f.contains("Text"))
        .expect("a map.find over Text keys");
    assert!(find.contains("call i32 @ignite.text_cmp("), "{find}");
}

/// P3 and P5: `list.map` with a closure literal or a named fn that only
/// reads its parameter, and a call through a `let` bound to such a
/// closure, copy no list in the caller. A `spawn` of a borrowed list
/// copies it once: since story spawn-without-handshake the caller copies
/// it into the transfer memory the task owns (between `spawn_begin` and
/// `spawn`), and the task's thunk passes that copy on in place.
#[test]
fn readers_and_spawns_take_places() {
    let (generated, _) = lower_with("copies", true);
    let program = &generated.program;
    for symbol in [
        "tests.copies.lengths",
        "tests.copies.sums",
        "tests.copies.measured",
    ] {
        let body = function(program, symbol);
        assert!(
            !body.contains("call void @\"copy.List"),
            "{symbol}:\n{body}"
        );
    }
    let start = function(program, "tests.copies.start");
    assert_eq!(
        start.matches("call void @\"copy.List").count(),
        1,
        "{start}"
    );
    let (before, after) = start
        .split_once("call void @ignite_rt_spawn_begin()")
        .expect("a transfer begun");
    assert!(!before.contains("call void @\"copy.List"), "{start}");
    let (between, _) = after
        .split_once("call i8 @ignite_rt_spawn(")
        .expect("the spawn after the transfer");
    assert!(between.contains("call void @\"copy.List"), "{start}");
    let thunk = program
        .split("\ndefine ")
        .find(|f| f.starts_with("internal void @\"spawn.tests.copies.report."))
        .expect("the spawn thunk of report");
    assert!(!thunk.contains("call void @\"copy.List"), "{thunk}");
}

/// Story `std-mysql-client`, unit `mysql-cost`: a `match` on an indexed
/// element and a `for` over one read it in place in the list, as they read
/// a place, when their arms or body change nothing it is rooted at
/// (`docs/llvm.md` section 4); the test block shows one that a body
/// changes copied first.
#[test]
fn an_indexed_element_matched_or_iterated_is_read_in_place() {
    let (generated, _) = lower_with("copies", true);
    let program = &generated.program;
    for symbol in ["tests.copies.picked", "tests.copies.summed"] {
        let body = function(program, symbol);
        assert!(
            !body.contains("call void @\"copy.List"),
            "{symbol}:\n{body}"
        );
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
