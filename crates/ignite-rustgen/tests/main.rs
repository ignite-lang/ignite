//! Every integration test of this crate in one test executable: one
//! compile and one link instead of one per file (`docs/vision.md`,
//! invariant 9). Each file is a module; harness names are
//! `<file>::<test>`.

mod common;
mod compiles;
mod golden;
mod reachable;
mod slice10;
mod slice16;
mod slice9;
mod streams;
mod units;
