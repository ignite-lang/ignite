use std::path::Path;

use ignite_perf::cycle::{System, replace_once, self_check, units};

fn example(system: System, file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(system.directory())
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn the_apply_cycle_self_check_passes() {
    self_check().unwrap();
}

/// The examples hold each text the cycle replaces exactly once, so `apply-cycle` does not fail
/// on drift after its first build.
#[test]
fn the_examples_hold_the_edited_literals_once() {
    for system in [System::Terminal, System::Todo] {
        let source = example(system, system.source());
        replace_once(&source, system.port_text(), "port = 1").unwrap();
        let edited = example(system, system.edited());
        let changed = replace_once(
            &edited,
            &system.literal(system.original()),
            &system.literal(&system.text(1)),
        )
        .unwrap();
        replace_once(
            &changed,
            &system.literal(&system.text(1)),
            &system.literal(&system.text(2)),
        )
        .unwrap();
    }
}

#[test]
fn the_todo_is_cycled_on_the_candidate_only() {
    assert!(units("baseline").iter().all(|u| !u.contains("todo")));
    assert!(units("comparison").iter().all(|u| !u.contains("todo")));
    assert_eq!(
        units("candidate")
            .iter()
            .filter(|u| u.starts_with("apply-cycle-todo-"))
            .count(),
        2
    );
}
