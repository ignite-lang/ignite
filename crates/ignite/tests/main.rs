//! Every integration test of this crate in one test executable: one
//! compile and one link instead of one per file (`docs/vision.md`,
//! invariant 9). Each file is a module; harness names are
//! `<file>::<test>`.

mod cli;
mod common;
mod failover;
mod generics;
mod ignite_test;
mod llvm;
mod llvm_build;
mod llvm_memory;
mod siblings;
mod slice10_llvm;
mod slice9;
mod syntax;
mod terminal;
mod todo;
