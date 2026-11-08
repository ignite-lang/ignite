//! Every integration test of this crate in one test executable
//! (`docs/vision.md`, invariant 9). Each file is a module; harness names
//! are `<file>::<test>`.

mod args;
mod bench;
mod common;
mod compare;
mod cycle;
mod export;
mod json;
mod readiness;
mod records;
mod remote;
mod source;
