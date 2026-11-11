//! The one place the substrate changes what a signal does. A disposition
//! is one of [`Disposition`]: the default action, or this crate's handler
//! of a task's stack overflow. No caller can install other handler code
//! through it, so the condition `sigaction` puts on a handler (that it is
//! async-signal-safe) is argued here once, for every caller.

use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};

/// What a signal is set to.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Disposition {
    /// The default action.
    Default,
    /// `crate::sched`'s handler of a task's stack overflow, on the
    /// alternate signal stack.
    Overflow,
}

/// Set `signal` to `disposition`; the action that was installed before.
/// Async-signal-safe itself (no allocation, no lock), so the fault handler
/// may call it.
pub(crate) fn dispose(signal: Signal, disposition: Disposition) -> nix::Result<SigAction> {
    let action = match disposition {
        Disposition::Default => {
            SigAction::new(SigHandler::SigDfl, SaFlags::empty(), SigSet::empty())
        }
        Disposition::Overflow => SigAction::new(
            SigHandler::SigAction(crate::sched::on_fault),
            SaFlags::SA_ONSTACK | SaFlags::SA_SIGINFO,
            SigSet::empty(),
        ),
    };
    // SAFETY: `action` is one of two: the default action installs no
    // handler code, and `sched::on_fault` is async-signal-safe (its docs).
    // `sigaction` reads `action` for the call only.
    unsafe { sigaction(signal, &action) }
}
