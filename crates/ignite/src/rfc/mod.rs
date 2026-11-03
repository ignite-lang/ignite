//! `ignite rfc import` and `ignite rfc check` (`docs/slice.md` 19.8).
//!
//! `import` reads a local RFCXML v3 file, extracts one requirement per sentence with a
//! `<bcp14>` mark and writes a catalogue module of `std.rfc` constants. It never opens a
//! network connection: the URL and the title come from the file or from flags.
//! `check` compares a catalogue's recorded `digest` with the digest of a source file.

use std::fmt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

mod extract;
mod write;
pub mod xml;

/// The flags of `ignite rfc import`, as given; the module's header records them.
#[derive(Clone, Debug, Default)]
pub struct ImportOptions {
    /// Comma-separated section numbers; a requirement is kept when its section equals
    /// one or starts with one followed by `.`.
    pub sections: Option<String>,
    /// Default `std.rfc.rfc<number>`.
    pub package: Option<String>,
    /// Default the `href` of the root's first `<link rel="alternate">`.
    pub url: Option<String>,
}

/// The outcome of a `check` that found no difference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Checked {
    /// The recorded digest is the source's.
    Matches,
    /// No source was given.
    Unchecked,
}

/// Why `import` or `check` failed, with its exit status ([`Error::exit_status`]).
#[derive(Debug)]
pub enum Error {
    /// Exit 2: an invalid flag value.
    Usage(String),
    /// Exit 2: malformed XML, a root without `number`, or an unknown BCP 14 keyword.
    /// Rendered `<file>:<line>:<col>: <message>`.
    Source {
        path: PathBuf,
        line: usize,
        col: usize,
        message: String,
    },
    /// Exit 2: the catalogue does not parse, or has no single `Spec` constant with a
    /// literal `digest`. `rendered` is the whole text for stderr.
    Catalogue { rendered: String },
    /// Exit 1: the recorded digest differs from the source's.
    Mismatch {
        catalogue: PathBuf,
        source: PathBuf,
        recorded: String,
        actual: String,
    },
    /// Exit 3: a file could not be read or written.
    Io {
        path: PathBuf,
        error: std::io::Error,
    },
}

impl Error {
    pub fn exit_status(&self) -> i32 {
        match self {
            Error::Mismatch { .. } => 1,
            Error::Usage(_) | Error::Source { .. } | Error::Catalogue { .. } => 2,
            Error::Io { .. } => 3,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(message) => write!(f, "error: {message}"),
            Error::Source {
                path,
                line,
                col,
                message,
            } => write!(f, "{}:{line}:{col}: {message}", path.display()),
            Error::Catalogue { rendered } => f.write_str(rendered.trim_end()),
            Error::Mismatch {
                catalogue,
                source,
                recorded,
                actual,
            } => write!(
                f,
                "{}: digest {recorded} differs from {actual}, the digest of {}",
                catalogue.display(),
                source.display()
            ),
            Error::Io { path, error } => write!(f, "error: {}: {error}", path.display()),
        }
    }
}

impl std::error::Error for Error {}

/// `"sha256:"` + the lowercase hex sha256 of `bytes`.
pub fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn read(path: &Path) -> Result<Vec<u8>, Error> {
    let io = |error| Error::Io {
        path: path.to_path_buf(),
        error,
    };
    let size = std::fs::metadata(path).map_err(io)?.len();
    if size > xml::LIMIT as u64 {
        return Err(Error::Source {
            path: path.to_path_buf(),
            line: 1,
            col: 1,
            message: "the file is larger than 16 MiB".into(),
        });
    }
    std::fs::read(path).map_err(io)
}

/// The catalogue module of the RFCXML file `file`.
pub fn import(file: &Path, options: &ImportOptions) -> Result<String, Error> {
    let sections = match &options.sections {
        Some(list) => {
            let entries: Vec<String> = list.split(',').map(|s| s.trim().to_string()).collect();
            if entries.iter().any(String::is_empty) {
                return Err(Error::Usage(format!(
                    "--sections `{list}` needs comma-separated section numbers"
                )));
            }
            Some(entries)
        }
        None => None,
    };
    if let Some(package) = &options.package
        && !is_module_path(package)
    {
        return Err(Error::Usage(format!(
            "--package `{package}` is no module path (dotted identifiers, `std.rfc.rfc9112`)"
        )));
    }
    let bytes = read(file)?;
    let at = |offset: usize, message: String| {
        let text = String::from_utf8_lossy(&bytes[..offset.min(bytes.len())]);
        let (line, col) = xml::line_col(&text, text.len());
        Error::Source {
            path: file.to_path_buf(),
            line,
            col,
            message,
        }
    };
    let root = xml::parse(&bytes).map_err(|error| at(error.offset, error.message))?;
    let number = match root.attribute("number").map(str::parse::<u32>) {
        Some(Ok(number)) => number,
        Some(Err(_)) => {
            return Err(at(
                root.offset,
                "the root's `number` attribute is no RFC number".into(),
            ));
        }
        None => return Err(at(root.offset, "the root has no `number` attribute".into())),
    };
    let url = match &options.url {
        Some(url) => url.clone(),
        None => match root
            .elements()
            .find(|e| e.name == "link" && e.attribute("rel") == Some("alternate"))
            .and_then(|link| link.attribute("href"))
        {
            Some(href) => href.to_string(),
            None => {
                return Err(at(
                    root.offset,
                    "the root has no `<link rel=\"alternate\">`; give --url".into(),
                ));
            }
        },
    };
    let title = root
        .child("front")
        .and_then(|front| front.child("title"))
        .map(extract::plain_text)
        .unwrap_or_default();
    let mut requirements = extract::requirements(&root).map_err(|unknown| {
        at(
            unknown.offset,
            format!("`{}` is no BCP 14 keyword", unknown.keyword),
        )
    })?;
    if let Some(sections) = &sections {
        requirements.retain(|requirement| {
            sections.iter().any(|entry| {
                requirement.section == *entry
                    || requirement
                        .section
                        .strip_prefix(entry.as_str())
                        .is_some_and(|rest| rest.starts_with('.'))
            })
        });
    }
    Ok(write::module(&write::Catalogue {
        source: file,
        options,
        number,
        title: &title,
        url: &url,
        digest: &digest(&bytes),
        requirements: &requirements,
    }))
}

/// Whether `path` is a dotted module path: identifiers (an ASCII letter or `_`, then ASCII
/// letters, digits or `_`) joined by `.`.
fn is_module_path(path: &str) -> bool {
    path.split('.').all(|segment| {
        let mut chars = segment.chars();
        chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// Compares the `digest` of `catalogue`'s single `Spec` constant with the digest of
/// `source`'s bytes; without `source`, only reads the digest.
pub fn check(catalogue: &Path, source: Option<&Path>) -> Result<Checked, Error> {
    let text = std::fs::read_to_string(catalogue).map_err(|error| Error::Io {
        path: catalogue.to_path_buf(),
        error,
    })?;
    let file = ignite_syntax::SourceFile::new(catalogue.display().to_string(), text);
    let program = ignite_syntax::parse(&file).map_err(|diagnostics| Error::Catalogue {
        rendered: file.render_all(&diagnostics),
    })?;
    let recorded = spec_digest(&program).map_err(|message| Error::Catalogue {
        rendered: format!("{}: {message}", catalogue.display()),
    })?;
    let Some(source) = source else {
        return Ok(Checked::Unchecked);
    };
    let actual = digest(&std::fs::read(source).map_err(|error| Error::Io {
        path: source.to_path_buf(),
        error,
    })?);
    if actual == recorded {
        Ok(Checked::Matches)
    } else {
        Err(Error::Mismatch {
            catalogue: catalogue.to_path_buf(),
            source: source.to_path_buf(),
            recorded,
            actual,
        })
    }
}

/// The `digest` Text literal of the program's single `Spec` constant.
fn spec_digest(program: &ignite_syntax::ast::Program) -> Result<String, String> {
    use ignite_syntax::ast::{Decl, ExprKind};
    let specs: Vec<_> = program
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Const(constant)
                if constant.ty.name.name.rsplit('.').next() == Some("Spec")
                    && constant.ty.args.is_empty() =>
            {
                Some(constant)
            }
            _ => None,
        })
        .collect();
    let [spec] = specs.as_slice() else {
        return Err(format!(
            "expected exactly one `Spec` constant, found {}",
            specs.len()
        ));
    };
    let ExprKind::Record { fields, .. } = &spec.value.kind else {
        return Err(format!(
            "the `Spec` constant `{}` is no `Spec {{ .. }}` literal",
            spec.name.name
        ));
    };
    match fields.iter().find(|field| field.name.name == "digest") {
        Some(field) => match &field.value.kind {
            ExprKind::Str(digest) => Ok(digest.clone()),
            _ => Err(format!(
                "the `digest` of `{}` is no Text literal",
                spec.name.name
            )),
        },
        None => Err(format!(
            "the `Spec` constant `{}` has no `digest`",
            spec.name.name
        )),
    }
}
