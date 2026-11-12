//! Every integration test of this crate in one test executable: one
//! compile and one link instead of one per file (`docs/vision.md`,
//! invariant 9). Each file is a module; harness names are
//! `<file>::<test>`.

mod entropy_adversary;
mod fs_adversary;
mod list_adversary;
mod pipe;
mod reactor;
mod reply_adversary;
mod scheduler;
mod scheduler_adversary;
mod stdio;
mod tcp;
mod terminal;
mod timer_adversary;
mod tls;
mod tls_adversary;
mod unsafe_adversary;
mod unsafe_adversary_fixes;
mod urgent;
mod waiter;
mod warnings;
