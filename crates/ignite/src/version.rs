//! The version of this compiler, as `ignite --version` prints it.

mod describe;

pub use describe::{Source, describe};

/// `X.Y.Z` for a build of the release tag `vX.Y.Z` from a clean checkout
/// (`task dist`), `X.Y.Z+g<12 hex digits of the commit>` for any other
/// checkout, with `.dirty` when a tracked file is modified, the commit an
/// archive's `COMMIT` file records, or `X.Y.Z+unknown`; set by `build.rs`.
pub const VERSION: &str = env!("IGNITE_VERSION");
