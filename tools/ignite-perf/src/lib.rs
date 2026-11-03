//! Repository tooling of Ignite: the performance budget harness (`ignite-perf <mode>`) and the
//! LLVM readiness verifier (`ignite-perf verify-llvm-readiness`). Neither is linked into a
//! compiled program or a compiler crate.

pub mod cli;
pub mod json;
pub mod perf;
pub mod readiness;

/// Every source file of this crate. A capture records the SHA256 of [`harness_bundle`] in each
/// row (`harness_sha256`) and keeps the bundle in `evidence/harness/<sha>.rs`, so a change of
/// the criteria is auditable even when the compiler is unchanged.
pub const HARNESS_SOURCE: &[(&str, &str)] = &[
    ("Cargo.toml", include_str!("../Cargo.toml")),
    ("src/cli.rs", include_str!("cli.rs")),
    ("src/json.rs", include_str!("json.rs")),
    ("src/lib.rs", include_str!("lib.rs")),
    ("src/main.rs", include_str!("main.rs")),
    ("src/perf.rs", include_str!("perf.rs")),
    ("src/readiness.rs", include_str!("readiness.rs")),
];

/// The crate's sources as one text: each file after a `// ==> path <==` line.
pub fn harness_bundle() -> String {
    HARNESS_SOURCE
        .iter()
        .map(|(name, text)| format!("// ==> {name} <==\n{text}"))
        .collect()
}
