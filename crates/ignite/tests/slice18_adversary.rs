//! Adversarial cases of slice 18 wave 2 (`docs/slice.md` section 22): the
//! check-time rules E150 and E151 (22.3) against the subscriptions the
//! model records, `ModelMachine::subscribes`.

use std::path::{Path, PathBuf};
use std::process::Output;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/slice18_adversary")
}

fn check(name: &str) -> Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_ignite"))
        .args(["check", name])
        .current_dir(fixtures())
        .output()
        .expect("run ignite check")
}

/// 22.3 E150: "a member whose machine subscribes to a signal, and whose
/// kind declares no `classify` hook". `S.w`'s machine subscribes to
/// `S.q.counts` through a list property (`ignite plan` lists it in the
/// machine's `subscribes`), and `Watch` declares no `classify`, so every
/// emission routed to it is E128 at run time ("declares no `classify`",
/// `crates/ignite/src/run/engine.rs`). The check must report it.
#[test]
fn a_subscription_inside_a_list_without_classify_is_e150() {
    let output = check("nested_signals.ignite");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error[E150]: contract `S.w` of kind `nested_signals.Watch` subscribes to `S.q.counts` of `Int` but declares no `fn Watch.classify(self: Watch, sample: Int) -> Bool`"),
        "exit {:?}, stderr:\n{stderr}",
        output.status.code()
    );
}

/// 22.3 E151: a signal the assembly runs and a member with a machine
/// subscribes to, whose element type is no outcome type. `S.v` subscribes
/// to `S.p.beats` (a `Signal<fn() -> Int>`, `S.p` run by the assembly)
/// through a list property.
#[test]
fn a_subscription_inside_a_list_to_a_fn_signal_is_e151() {
    let output = check("nested_signals.ignite");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error[E151]: signal `S.p.beats` carries `fn() -> Int`; a probe outcome holds no fn, handle, reference or signal"),
        "exit {:?}, stderr:\n{stderr}",
        output.status.code()
    );
}
