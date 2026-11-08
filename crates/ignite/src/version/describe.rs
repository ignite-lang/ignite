//! What `ignite --version` says about the source a compiler was built from.
//! Shared with `build.rs`, which reads the source and sets
//! `IGNITE_VERSION` to [`describe`]'s answer; this file needs nothing of the
//! crate.

/// The workspace paths the driver is built from: a tracked change under one
/// of them makes a checkout's build `.dirty`, and `build.rs` reruns when one
/// changes. Edits elsewhere (`std/`, which the driver reads at run time,
/// docs, tests) change no driver and do not make it `.dirty`.
pub const SOURCES: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "crates/ignite/Cargo.toml",
    "crates/ignite/build.rs",
    "crates/ignite/src",
    "crates/ignite-syntax/Cargo.toml",
    "crates/ignite-syntax/src",
    "crates/ignite-check/Cargo.toml",
    "crates/ignite-check/src",
    "crates/ignite-types/Cargo.toml",
    "crates/ignite-types/src",
    "crates/ignite-rustgen/Cargo.toml",
    "crates/ignite-rustgen/src",
    "crates/ignite-llvm/Cargo.toml",
    "crates/ignite-llvm/src",
    "crates/ignite-eval/Cargo.toml",
    "crates/ignite-eval/src",
    "crates/ignite-io/Cargo.toml",
    "crates/ignite-io/src",
    "crates/ignite-rt/Cargo.toml",
    "crates/ignite-rt/src",
];

/// Where a build's source came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A git checkout whose top level is the workspace: the full `HEAD`
    /// commit, whether a tracked file under [`SOURCES`] differs from it, and
    /// the tags at `HEAD`.
    Git {
        commit: String,
        dirty: bool,
        tags: Vec<String>,
    },
    /// No checkout, but the workspace's `COMMIT` file names the full commit
    /// the source archive was made from.
    Recorded(String),
    /// Neither.
    Unknown,
}

impl Source {
    /// The source an archive's `COMMIT` text names: 40 hexadecimal digits
    /// on one line, else [`Source::Unknown`].
    pub fn recorded(text: &str) -> Source {
        let commit = text.trim();
        if commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            Source::Recorded(commit.to_ascii_lowercase())
        } else {
            Source::Unknown
        }
    }
}

/// `version` alone for a clean checkout at the tag `v<version>`, the release
/// build; otherwise `version` with the semver build metadata `+g<the first 12
/// digits of the commit>`, `.dirty` appended for a checkout that modifies a
/// tracked file under [`SOURCES`], or `+unknown` when the commit is not known.
pub fn describe(version: &str, source: &Source) -> String {
    match source {
        Source::Git {
            commit,
            dirty,
            tags,
        } => {
            let release = format!("v{version}");
            if !dirty && tags.contains(&release) {
                version.to_string()
            } else {
                let short = &commit[..commit.len().min(12)];
                let dirty = if *dirty { ".dirty" } else { "" };
                format!("{version}+g{short}{dirty}")
            }
        }
        Source::Recorded(commit) => format!("{version}+g{}", &commit[..12]),
        Source::Unknown => format!("{version}+unknown"),
    }
}
