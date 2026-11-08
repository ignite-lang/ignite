//! Every integration test of this crate in one test executable: one
//! compile and one link instead of one per file (`docs/vision.md`,
//! invariant 9). Each file is a module; harness names are
//! `<file>::<test>`.

mod adversary_bounds;
mod audit;
mod builtin_tables;
mod clone;
mod common;
mod effectful_map_adversary;
mod fixtures;
mod generics;
mod infer_adversary;
mod inline_tests;
mod lent;
mod limits;
mod moves;
mod moves_adversary;
mod ownership;
mod ownership_adversary2;
mod slice9;
mod surface;
mod terminal;
