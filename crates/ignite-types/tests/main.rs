//! Every integration test of this crate in one test executable: one
//! compile and one link instead of one per file (`docs/vision.md`,
//! invariant 9). Each file is a module; harness names are
//! `<file>::<test>`.

mod adversary_bounds;
mod audit;
mod common;
mod fixtures;
mod generics;
mod inline_tests;
mod limits;
mod slice9;
mod surface;
mod terminal;
