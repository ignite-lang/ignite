//! Every integration test of this crate in one test executable: one
//! compile and one link instead of one per file (`docs/vision.md`,
//! invariant 9). Each file is a module; harness names are
//! `<file>::<test>`.

mod fixtures;
mod grammar;
mod node_ids;
mod node_ids_adversary;
mod parse_terminal;
mod robustness;
