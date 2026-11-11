//! The one place the substrate changes what a signal does, and the one
//! handler it installs. A disposition is one of [`Disposition`]: the
//! default action, ignoring the signal, or [`on_fault`], the handler of a
//! task's stack overflow (`crate::sched`'s module docs). The LLVM runtime
//! ignores `SIGPIPE` through [`ignore`]. No caller can install other handler
//! code, and no other module can reach [`on_fault`] to call it with a
//! pointer of its own, so the conditions `sigaction` puts on a handler
//! (that it is async-signal-safe, that the kernel gives it its arguments)
//! are argued here once.

use std::ffi::c_void;
use std::os::fd::AsFd;
use std::sync::{Mutex, Once, OnceLock, PoisonError};

use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};

/// What a signal is set to.
#[derive(Clone, Copy, Debug)]
enum Disposition {
    /// The default action.
    Default,
    /// Ignored.
    Ignore,
    /// [`on_fault`], on the alternate signal stack.
    Overflow,
}

/// Set `signal` to `disposition`; the action that was installed before.
/// Async-signal-safe itself (no allocation, no lock), so the fault handler
/// may call it.
fn dispose(signal: Signal, disposition: Disposition) -> nix::Result<SigAction> {
    let action = match disposition {
        Disposition::Default => {
            SigAction::new(SigHandler::SigDfl, SaFlags::empty(), SigSet::empty())
        }
        Disposition::Ignore => {
            SigAction::new(SigHandler::SigIgn, SaFlags::empty(), SigSet::empty())
        }
        Disposition::Overflow => SigAction::new(
            SigHandler::SigAction(on_fault),
            SaFlags::SA_ONSTACK | SaFlags::SA_SIGINFO,
            SigSet::empty(),
        ),
    };
    // SAFETY: `action` is one of three: the default action and ignoring
    // install no handler code, and `on_fault` is async-signal-safe (its
    // docs).
    // `sigaction` reads `action` for the call only.
    unsafe { sigaction(signal, &action) }
}

/// Set `signal` back to its default action.
pub(crate) fn restore_default(signal: Signal) {
    let _ = dispose(signal, Disposition::Default);
}

/// Ignore `signal`: the LLVM runtime's `SIGPIPE`, so that a write to a
/// closed peer is an error of the write, not the end of the process.
pub fn ignore(signal: Signal) -> nix::Result<()> {
    dispose(signal, Disposition::Ignore).map(|_| ())
}

/// What gives a worker its alternate signal stack, when a runtime gives
/// one: the LLVM runtime's, whose program has no `main` of Rust's, so std
/// gives its threads none. A program of the Rust target has std's on every
/// thread it starts when std installed its own handler for `SIGSEGV` or
/// `SIGBUS`, which it does for each one not ignored at the start: so
/// unless both were ignored. What a task's overflow does in each start is
/// at [`install_overflow_handler`]. A `Mutex`, not a `OnceLock`: the
/// latter's generic setter costs a program of the LLVM target more bytes
/// than a lock, whose code the scheduler's locks share.
static ALTERNATE_STACK: Mutex<Option<fn()>> = Mutex::new(None);

/// The function given to [`give_workers_an_alternate_stack`], if any,
/// after `give` is kept when none was given before. Inlined, as the other
/// small functions of this module: a function of its own costs a program
/// its symbol and unwind entry, more than its few instructions.
#[inline(always)]
fn alternate_stack(give: Option<fn()>) -> Option<fn()> {
    let mut kept = ALTERNATE_STACK
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if kept.is_none() {
        *kept = give;
    }
    *kept
}

/// Have `give` called on each worker thread as it starts, before its first
/// task, to give the thread an alternate signal stack for [`on_fault`].
/// The first call counts; made by the LLVM runtime at its start, before
/// any task.
pub fn give_workers_an_alternate_stack(give: fn()) {
    alternate_stack(Some(give));
}

/// Called by a worker thread as it starts (`crate::sched`).
pub(crate) fn worker_started() {
    if let Some(give) = alternate_stack(None) {
        give();
    }
}

/// The handlers of `SIGSEGV` and `SIGBUS` before [`on_fault`].
static PREVIOUS: OnceLock<[SigAction; 2]> = OnceLock::new();

/// Install [`on_fault`] for `SIGSEGV` and `SIGBUS` once, keeping the
/// handlers before it for every fault outside a task's guard page. A
/// signal found ignored keeps its ignore when no runtime gives the workers
/// an alternate stack (the Rust target), since without one its handler
/// would run on the stack that overflowed and fault again. An overflow
/// faults with `SIGSEGV`, so by the start on the Rust target: nothing or
/// only `SIGBUS` ignored, it is reported and the process aborts; `SIGSEGV`
/// ignored, alone or with `SIGBUS`, it ends the process by `SIGSEGV` with
/// no report, as a program of v0.15.0 did, even where std gave the thread
/// a stack (only `SIGSEGV` ignored). The LLVM target reports it in all
/// four starts.
pub(crate) fn install_overflow_handler() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(install);
}

/// The one run of [`install_overflow_handler`]. Not inlined: `call_once`
/// would copy it into each of the two closures it makes of it.
#[inline(never)]
fn install() {
    let stacks_given = alternate_stack(None).is_some();
    let segv = overflow_handler_for(Signal::SIGSEGV, stacks_given);
    let bus = overflow_handler_for(Signal::SIGBUS, stacks_given);
    if let (Ok(segv), Ok(bus)) = (segv, bus) {
        let _ = PREVIOUS.set([segv, bus]);
    }
}

/// Install [`on_fault`] for `signal`, or keep the signal ignored when it
/// was and `stacks_given` is false ([`install_overflow_handler`]); the
/// action installed before.
#[inline(always)]
fn overflow_handler_for(signal: Signal, stacks_given: bool) -> nix::Result<SigAction> {
    let previous = dispose(signal, Disposition::Overflow)?;
    if matches!(previous.handler(), SigHandler::SigIgn) && !stacks_given {
        let _ = dispose(signal, Disposition::Ignore);
    }
    Ok(previous)
}

/// The `SIGSEGV` and `SIGBUS` handler: a fault in the running task's guard
/// page (`crate::sched::guard_page`) is a stack overflow, reported, and
/// the process aborts; any other fault goes to the handler installed
/// before. Async-signal-safe, which [`dispose`] relies on: it reads a
/// `const` thread-local cell, writes to stderr with `write`, aborts, hands
/// the signal on, or restores the default action with [`dispose`]. Private
/// to this module: only the kernel calls it, with the arguments of a
/// handler installed with `SA_SIGINFO`.
extern "C" fn on_fault(signal: i32, info: *mut nix::libc::siginfo_t, context: *mut c_void) {
    // SAFETY: the kernel passes a valid `siginfo_t` to a handler installed
    // with `SA_SIGINFO`, and nothing else calls this function (it is
    // private and only `dispose` names it); `si_addr` is defined for
    // SIGSEGV and SIGBUS.
    let address = unsafe { (*info).si_addr() } as usize;
    let (start, end) = crate::sched::guard_page();
    if start <= address && address < end {
        let message = b"\nthread 'worker' has overflowed its stack\nfatal runtime error: stack overflow, aborting\n";
        let _ = nix::unistd::write(std::io::stderr().as_fd(), message);
        std::process::abort();
    }
    let which = usize::from(signal != Signal::SIGSEGV as i32);
    match PREVIOUS.get().map(|previous| previous[which].handler()) {
        Some(SigHandler::SigAction(previous)) => previous(signal, info, context),
        Some(SigHandler::Handler(previous)) => previous(signal),
        _ => {
            if let Ok(signal) = Signal::try_from(signal) {
                // The fault repeats on return and ends the process with
                // the default action.
                let _ = dispose(signal, Disposition::Default);
            }
        }
    }
}
