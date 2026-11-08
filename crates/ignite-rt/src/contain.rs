//! Panic containment at the C ABI (`docs/llvm.md` section 4). Every
//! exported entry but `init` runs its body through [`entry`], which catches
//! a Rust panic of the body with `catch_unwind` and turns it into an Ignite
//! failure of the current task ([`crate::fail::fail`]): the task's handles
//! are released, `task ended with error: panic at <file>:<line>:<col>:
//! <message>` is logged and the task wrapper's `longjmp` ends the task; on
//! the main task or in a test the report is logged as `error: <report>`
//! and the process exits 101 (`ignite_io::FAILURE_EXIT_CODE`). A failure
//! the body raises through `ignite_io::fail_with` is caught the same way
//! and keeps its own report.
//!
//! Why `catch_unwind` and not `extern "C-unwind"`: the generator's frames
//! carry no landing pads, so an unwind through them would skip their own
//! cleanup (the guard registered by the task wrapper, the callback guard of
//! a store thunk, the task's handles and arena) and leave the task's state
//! to `ignite_io::spawn`'s catch, which knows none of it; on the main
//! thread nothing would catch it. Ignite failures already end a task by
//! `longjmp` to its wrapper, so a caught panic takes that same path.
//!
//! The jump is sound because it never leaves a `catch_unwind`: [`entry`]
//! calls `fail` after `catch_unwind` returned, when every Rust frame of the
//! body has been unwound and dropped and the entry's own frame holds
//! nothing that needs a destructor. For the same reason the entries that
//! end the task themselves (`fail_*`, `assert_failed`, `try_failed`) guard
//! only the building of their report and call `fail` after, and
//! `store_update` and `store_upsert` `resume` a callback's failure after
//! their guarded body returned.
//!
//! [`ignite_io::catch`] marks the thread as catching, so the panic hook
//! records the report instead of printing it and exiting. `init` has no
//! guard: it installs the runtime's panic hook (exit 101 for a panic no
//! entry catches) and then `ignite-io`'s, which wraps it; a guard around
//! `init` would install `ignite-io`'s hook first and let `init`'s replace
//! it. `arena_state` has none either: it cannot panic, and it is the
//! hottest entry (its docs; the test lists both).

use std::convert::Infallible;

use crate::fail::fail;

/// The value of `body`, or, when it panics, the end of the current task
/// with the panic's report (module docs).
#[inline]
pub(crate) fn entry<T>(body: impl FnOnce() -> T) -> T {
    match ignite_io::catch(body) {
        Ok(value) => value,
        Err(report) => fail(report),
    }
}

/// [`entry`] of a body that does not return (`exit`, `panic`,
/// `test_main`).
pub(crate) fn entry_never(body: impl FnOnce() -> Infallible) -> ! {
    match entry(body) {}
}

/// Entries without a guard: `init` (module docs), and the entries whose
/// whole body is the given expression, which cannot panic (see each
/// entry's docs). An edit to such a body fails
/// `contain::tests::every_entry_is_guarded` (and quality row 3.5, which
/// reads this list) until the entry is guarded again or the new body is
/// shown not to panic.
pub const UNGUARDED_ENTRIES: [(&str, Option<&str>); 2] = [
    ("ignite_rt_init", None),
    ("ignite_rt_arena_state", Some("state()")),
];

#[cfg(test)]
mod tests {
    use super::UNGUARDED_ENTRIES as UNGUARDED;

    /// Every `extern "C"` entry of the crate runs its body through
    /// [`super::entry`] or [`super::entry_never`], or builds its report in
    /// [`super::entry`] before it fails; the exceptions are [`UNGUARDED`].
    #[test]
    fn every_entry_is_guarded() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut entries = 0;
        let mut unguarded = Vec::new();
        for file in std::fs::read_dir(&dir).unwrap() {
            let path = file.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap();
            for (at, _) in text.match_indices("\npub extern \"C\" fn ") {
                let rest = &text[at + 1..];
                let body = &rest[..rest.find("\n}\n").expect("end of the entry")];
                let name = body["pub extern \"C\" fn ".len()..]
                    .split('(')
                    .next()
                    .unwrap();
                entries += 1;
                let guarded =
                    body.contains("crate::entry(") || body.contains("crate::entry_never(");
                let excepted = UNGUARDED.iter().any(|(entry, whole)| {
                    *entry == name
                        && whole.is_none_or(|whole| {
                            body.split_once("{\n").map(|(_, inside)| inside.trim()) == Some(whole)
                        })
                });
                if !guarded && !excepted {
                    unguarded.push(format!("{}: {name}", path.display()));
                }
            }
        }
        assert!(entries > 90, "{entries} entries found");
        assert!(unguarded.is_empty(), "unguarded entries: {unguarded:#?}");
    }
}
