//! Repository tooling of Ignite: the performance budget harness (`ignite-perf <mode>`) and the
//! LLVM readiness verifier (`ignite-perf verify-llvm-readiness`). Neither is linked into a
//! compiled program or a compiler crate.

pub mod bench;
pub mod cli;
pub mod cycle;
pub mod json;
pub mod perf;
pub mod quality;
pub mod readiness;
pub mod remote;

/// Every source file of this crate. A capture records the SHA256 of [`harness_bundle`] in each
/// row (`harness_sha256`) and keeps the bundle in `evidence/harness/<sha>.rs`, so a change of
/// the criteria is auditable even when the compiler is unchanged.
pub const HARNESS_SOURCE: &[(&str, &str)] = &[
    ("Cargo.toml", include_str!("../Cargo.toml")),
    ("src/bench.rs", include_str!("bench.rs")),
    ("src/cli.rs", include_str!("cli.rs")),
    ("src/cycle.rs", include_str!("cycle.rs")),
    ("src/json.rs", include_str!("json.rs")),
    ("src/lib.rs", include_str!("lib.rs")),
    ("src/main.rs", include_str!("main.rs")),
    ("src/perf.rs", include_str!("perf.rs")),
    ("src/quality.rs", include_str!("quality.rs")),
    ("src/readiness.rs", include_str!("readiness.rs")),
    (
        "src/remote-user-data.sh",
        include_str!("remote-user-data.sh"),
    ),
    ("src/remote.rs", include_str!("remote.rs")),
    (
        "../../crates/ignite-rt/release_std.rs",
        include_str!("../../../crates/ignite-rt/release_std.rs"),
    ),
];

/// The crate's sources as one text: each file after a `// ==> path <==` line.
pub fn harness_bundle() -> String {
    HARNESS_SOURCE
        .iter()
        .map(|(name, text)| format!("// ==> {name} <==\n{text}"))
        .collect()
}
