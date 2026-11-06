//! The module text `ignite rfc import` writes (`docs/slice.md` 19.8, "Output"): a header
//! comment that re-runs the import, `package` and the `std.rfc` import, the `Spec`
//! constant, then one `Requirement` constant per requirement. Deterministic: no time and
//! no path beyond the one given.

use std::fmt::Write;
use std::path::Path;

use ignite_check::RFC_MODULE;

use super::ImportOptions;
use super::extract::Requirement;

pub struct Catalogue<'a> {
    pub source: &'a Path,
    pub options: &'a ImportOptions,
    pub number: u32,
    pub title: &'a str,
    pub url: &'a str,
    pub digest: &'a str,
    pub requirements: &'a [Requirement],
}

pub fn module(catalogue: &Catalogue) -> String {
    let Catalogue {
        source,
        options,
        number,
        title,
        url,
        digest,
        requirements,
    } = catalogue;
    let mut out = String::new();
    let mut command = format!(
        "ignite rfc import {}",
        shell_word(&source.display().to_string())
    );
    for (flag, value) in [
        ("--sections", &options.sections),
        ("--package", &options.package),
        ("--url", &options.url),
    ] {
        if let Some(value) = value {
            let _ = write!(command, " {flag} {}", shell_word(value));
        }
    }
    let _ = writeln!(out, "// RFC {number}: {title}");
    let _ = writeln!(out, "// {command}");
    out.push_str("// Every party is guessed from its sentence; review each `party` line.\n\n");
    let package = options
        .package
        .clone()
        .unwrap_or_else(|| format!("{RFC_MODULE}.rfc{number}"));
    let _ = writeln!(out, "package {package}\n");
    let _ = writeln!(
        out,
        "import {RFC_MODULE}.{{Level, Party, Source, Requirement, Spec}}\n"
    );
    let _ = writeln!(out, "const RFC{number}: Spec = Spec {{");
    let _ = writeln!(out, "  rfc = {number}");
    let _ = writeln!(out, "  title = {}", text(title));
    let _ = writeln!(out, "  url = {}", text(url));
    let _ = writeln!(out, "  digest = {}", text(digest));
    if requirements.is_empty() {
        out.push_str("  requirements = []\n");
    } else {
        out.push_str("  requirements = [\n");
        let ids: Vec<String> = requirements
            .iter()
            .map(|requirement| format!("    {}", requirement.id()))
            .collect();
        out.push_str(&ids.join(",\n"));
        out.push_str("\n  ]\n");
    }
    out.push_str("}\n");
    for requirement in requirements.iter() {
        let _ = write!(
            out,
            "\nconst {}: Requirement = Requirement {{\n  rfc = {number}\n  section = {}\n  ordinal = {}\n  level = {}\n  party = {}\n  text = {}\n}}\n",
            requirement.id(),
            text(&requirement.section),
            requirement.ordinal,
            requirement.level,
            requirement.party,
            text(&requirement.text),
        );
    }
    out
}

/// A Text literal: `\` as `\\`, `"` as `\"`, `{` as `{{`, `}` as `}}`, and a control
/// character as `\u{..}`.
fn text(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '{' => out.push_str("{{"),
            '}' => out.push_str("}}"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `word` as one shell word: as written when it holds only characters a shell leaves
/// alone, else single-quoted.
fn shell_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._,/:=+-@%".contains(c));
    if plain {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}
