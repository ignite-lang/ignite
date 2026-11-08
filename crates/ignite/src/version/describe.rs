//! What `ignite --version` says about the source a compiler was built from.
//! Shared with `build.rs`, which reads the source and sets
//! `IGNITE_VERSION` to [`describe`]'s answer; this file needs nothing of the
//! crate.

/// Where a build's source came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A git checkout whose top level is the workspace: the full `HEAD`
    /// commit, whether a tracked file differs from it, and the tags at `HEAD`.
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
/// tracked file, or `+unknown` when the commit is not known.
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
