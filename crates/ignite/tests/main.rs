//! Every integration test of this crate in one test executable: one
//! compile and one link instead of one per file (`docs/vision.md`,
//! invariant 9). Each file is a module; harness names are
//! `<file>::<test>`.

mod apply;
mod apply_admission;
mod apply_host;
mod apply_integrity;
mod apply_probe_handover;
mod apply_recovery;
mod apply_transition;
mod apply_way_out;
mod build_cache;
mod check_json;
mod clauses;
mod cli;
mod common;
mod conformance;
mod control_trace;
mod decode;
mod default_target;
mod drain;
mod eval;
mod failover;
mod generics;
mod ignite_test;
mod llvm;
mod llvm_build;
mod llvm_memory;
mod lsp;
mod plan;
mod reachability;
mod rfc;
mod run;
mod siblings;
mod site;
mod slice10_llvm;
mod slice16;
mod slice17;
mod slice17_adversary;
mod slice18_adversary;
mod slice9;
mod syntax;
mod terminal;
mod todo;
