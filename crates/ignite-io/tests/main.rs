//! Every integration test of this crate in one test executable: one
//! compile and one link instead of one per file (`docs/vision.md`,
//! invariant 9). Each file is a module; harness names are
//! `<file>::<test>`.

mod entropy_adversary;
mod fs_adversary;
mod pipe;
mod stdio;
mod tcp;
mod terminal;
