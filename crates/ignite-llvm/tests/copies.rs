//! `story:llvm-lowering-copies`: `tests/golden/copies.ignite` holds one
//! test per removed copy or allocation, each one that a wrong result of the
//! new lowering fails. Every test runs in a test build linked with
//! `libignite_rt.a` (the debug runtime, which overwrites released arena
//! memory), and the IR of the program module shows the copies gone.

use crate::golden::{lower_with, run_test_build};

#[test]
fn every_copies_test_runs() {
    let (generated, tests) = lower_with("copies", true);
    assert!(!tests.is_empty());
    for (i, test) in tests.iter().enumerate() {
        run_test_build(&format!("copies-{i}"), &generated, test);
    }
}
