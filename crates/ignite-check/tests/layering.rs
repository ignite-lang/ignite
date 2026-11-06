//! The layers of the standard library and of the compiler crates
//! (`docs/layering.md`): a general layer knows no application protocol.
//!
//! Every std module is parsed with the real parser and placed in a layer by
//! [`LAYERS`] or, for `std.protocol.*`, `std.rfc` and `std.rfc.*`, by its
//! path. A module imports only
//! modules of its own layer or a lower one; no module outside
//! `std.protocol.*` imports a protocol module or declares a name that
//! contains a protocol's name, except a catalogue `std.rfc.*` naming a
//! protocol whose module imports it; and no file under `crates/*/src` mentions
//! one, except the crates in [`CRATES_EXEMPT`]. The protocol names are read
//! from std: the `protocol` declarations and file names under
//! `std/protocol/`, so a new protocol is covered without editing this file.
//! No string literal under `crates/*/src` names a std module or an item of
//! one outside the table "Std names in the crates" of `docs/layering.md`,
//! which the test reads from there (rule 6).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use ignite_syntax::ast::{Decl, Ident, Program};
use ignite_syntax::{SourceFile, parse};

/// The layers, lowest first. A module may import modules of its own layer
/// or of a lower one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Layer {
    /// The substrate modules and the data utilities every layer may use.
    Base,
    /// The leaves of O45 (`docs/slice.md` 16.2): the vocabulary the
    /// reducers and the kinds share.
    Leaf,
    /// General modules: reducers, declaration vocabulary, consoles.
    General,
    /// `std.protocol.*`: one application protocol each.
    Protocol,
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Layer::Base => "base",
            Layer::Leaf => "leaf",
            Layer::General => "general",
            Layer::Protocol => "protocol",
        })
    }
}

/// The layer of every std module outside `std.protocol.*`, `std.rfc` and
/// `std.rfc.*`; the table of
/// `docs/layering.md`. A new module fails the test until it is listed.
const LAYERS: &[(&str, Layer)] = &[
    ("std.io", Layer::Base),
    ("std.net", Layer::Base),
    ("std.fs", Layer::Base),
    ("std.process", Layer::Base),
    ("std.option", Layer::Base),
    ("std.result", Layer::Base),
    ("std.text", Layer::Base),
    ("std.json", Layer::Base),
    ("std.sli", Layer::Leaf),
    ("std.time", Layer::Leaf),
    ("std.policy", Layer::Leaf),
    ("std.control", Layer::General),
    ("std.contract", Layer::General),
    ("std.deploy", Layer::General),
    ("std.console", Layer::General),
    ("std.cli", Layer::General),
];

/// Crates under `crates/` that may name a protocol, with the reason.
const CRATES_EXEMPT: &[(&str, &str)] = &[(
    "ignite-load",
    "a load client of the terminal example's server: it speaks the protocol to measure it and is linked into neither the compiler nor a compiled program",
)];

const PROTOCOL_PREFIX: &str = "std.protocol.";

/// `std.rfc` and every catalogue under it are leaf (`docs/slice.md` 19.11).
const RFC_PREFIX: &str = "std.rfc";

/// The layer a module's path decides: protocol for `std.protocol.*`, leaf
/// for `std.rfc` and `std.rfc.*`.
fn prefix_layer(module: &str) -> Option<Layer> {
    if module.starts_with(PROTOCOL_PREFIX) {
        return Some(Layer::Protocol);
    }
    let rfc = module
        .strip_prefix(RFC_PREFIX)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'));
    rfc.then_some(Layer::Leaf)
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn files(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            files(&path, extension, out);
        } else if path.extension().is_some_and(|ext| ext == extension) {
            out.push(path);
        }
    }
}

struct StdModule {
    module: String,
    source: SourceFile,
    program: Program,
}

impl StdModule {
    fn at(&self, ident: &Ident) -> String {
        let (line, column) = self.source.line_col(ident.span.start);
        format!("std/{}:{line}:{column}", self.relative())
    }

    fn relative(&self) -> String {
        let path = self.module.strip_prefix("std.").unwrap_or(&self.module);
        format!("{}.ignite", path.replace('.', "/"))
    }

    fn layer(&self) -> Option<Layer> {
        if let Some(layer) = prefix_layer(&self.module) {
            return Some(layer);
        }
        LAYERS
            .iter()
            .find(|(module, _)| *module == self.module)
            .map(|(_, layer)| *layer)
    }
}

fn standard_library() -> Vec<StdModule> {
    let root = repository().join("std");
    let mut paths = Vec::new();
    files(&root, "ignite", &mut paths);
    paths
        .into_iter()
        .map(|path| {
            let relative = path.strip_prefix(&root).expect("file under std/");
            let module = std::iter::once("std".to_string())
                .chain(
                    relative
                        .with_extension("")
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned()),
                )
                .collect::<Vec<_>>()
                .join(".");
            let text = std::fs::read_to_string(&path).unwrap();
            let source = SourceFile::new(path.display().to_string(), text);
            let program = parse(&source).unwrap_or_else(|d| panic!("{}", source.render_all(&d)));
            StdModule {
                module,
                source,
                program,
            }
        })
        .collect()
}

/// The layer of an imported module; `std.io` has no file.
fn layer_of(module: &str, std: &[StdModule]) -> Option<Layer> {
    if let Some(layer) = prefix_layer(module) {
        return Some(layer);
    }
    std.iter()
        .find(|m| m.module == module)
        .and_then(StdModule::layer)
        .or_else(|| {
            LAYERS
                .iter()
                .find(|(m, _)| *m == module)
                .map(|(_, layer)| *layer)
        })
}

/// The protocol names, lowercase: each `protocol` declared under
/// `std/protocol/` and each file name there.
fn protocol_names(std: &[StdModule]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for module in std.iter().filter(|m| m.module.starts_with(PROTOCOL_PREFIX)) {
        names.insert(module.module[PROTOCOL_PREFIX.len()..].to_lowercase());
        for decl in &module.program.decls {
            if let Decl::Protocol(protocol) = decl {
                names.insert(protocol.name.name.to_lowercase());
            }
        }
    }
    names
}

/// The prefix of the catalogue modules, which rule 3's exemption covers.
const CATALOGUE_PREFIX: &str = "std.rfc.";

/// For each catalogue `std.rfc.*` that a protocol module imports, the
/// protocol names of that module (its file name and its `protocol`
/// declarations, lowercase): the protocols whose specification the
/// catalogue quotes.
fn catalogued(std: &[StdModule]) -> BTreeMap<String, BTreeSet<String>> {
    let mut catalogued: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for module in std.iter().filter(|m| m.module.starts_with(PROTOCOL_PREFIX)) {
        let mut names = BTreeSet::from([module.module[PROTOCOL_PREFIX.len()..].to_lowercase()]);
        for decl in &module.program.decls {
            if let Decl::Protocol(protocol) = decl {
                names.insert(protocol.name.name.to_lowercase());
            }
        }
        for import in &module.program.imports {
            let target = import.module();
            if target.starts_with(CATALOGUE_PREFIX) {
                catalogued
                    .entry(target)
                    .or_default()
                    .extend(names.iter().cloned());
            }
        }
    }
    catalogued
}

/// Rule 3's exemption: a catalogue `std.rfc.*` may declare names that
/// contain the name of a protocol whose module imports it. No other module
/// is exempt, `std.rfc` itself included.
fn exempt(module: &str, protocol: &str, catalogued: &BTreeMap<String, BTreeSet<String>>) -> bool {
    module.starts_with(CATALOGUE_PREFIX)
        && catalogued
            .get(module)
            .is_some_and(|protocols| protocols.contains(protocol))
}

#[test]
fn rule_3_exempts_only_catalogues_of_their_own_protocol() {
    let map = BTreeMap::from([
        (
            "std.rfc.rfc9112".to_string(),
            BTreeSet::from(["http".to_string()]),
        ),
        ("std.json".to_string(), BTreeSet::from(["http".to_string()])),
        ("std.rfc".to_string(), BTreeSet::from(["http".to_string()])),
    ]);
    assert!(exempt("std.rfc.rfc9112", "http", &map));
    assert!(!exempt("std.rfc.rfc9112", "telnet", &map));
    assert!(!exempt("std.rfc.rfc854", "http", &map));
    assert!(
        !exempt("std.json", "http", &map),
        "only std.rfc.* is exempt"
    );
    assert!(
        !exempt("std.rfc", "http", &map),
        "the vocabulary is no catalogue"
    );
    assert!(!exempt("std.control", "http", &map));

    let real = catalogued(&standard_library());
    assert!(real["std.rfc.rfc9112"].contains("http"), "{real:?}");
    assert!(real["std.rfc.rfc854"].contains("telnet"), "{real:?}");
    assert!(
        real.keys()
            .all(|module| module.starts_with(CATALOGUE_PREFIX)),
        "{real:?}"
    );
}

/// Every identifier a module declares: items, owners of associated fns,
/// record fields, enum variants and protocol roles.
fn declared_names(program: &Program) -> Vec<&Ident> {
    let mut names = Vec::new();
    for decl in &program.decls {
        match decl {
            Decl::Test(_) => continue,
            Decl::Record(record) => names.extend(record.fields.iter().map(|f| &f.name)),
            Decl::Enum(enumeration) => names.extend(enumeration.variants.iter().map(|v| &v.name)),
            Decl::Fn(function) => names.extend(function.owner.iter()),
            Decl::Protocol(protocol) => names.extend(protocol.fns.iter().map(|f| &f.name)),
            _ => {}
        }
        names.push(decl.name());
    }
    names
}

#[test]
fn std_modules_import_only_their_own_layer_or_below() {
    let std = standard_library();
    let protocols = protocol_names(&std);
    let catalogued = catalogued(&std);
    assert!(
        protocols.contains("telnet") && protocols.contains("http") && protocols.contains("jsonrpc"),
        "protocol names read from std/protocol/: {protocols:?}"
    );
    let mut violations = Vec::new();
    for module in &std {
        let layer = module.layer().unwrap_or_else(|| {
            violations.push(format!(
                "std/{}: module `{}` has no layer: list it in LAYERS (crates/ignite-check/tests/layering.rs) and in docs/layering.md; judged as general below",
                module.relative(),
                module.module
            ));
            Layer::General
        });
        for import in &module.program.imports {
            let target = import.module();
            let at = module.at(import.path.segments.first().expect("import path"));
            if layer != Layer::Protocol && target.starts_with(PROTOCOL_PREFIX) {
                violations.push(format!(
                    "{at}: {layer} module `{}` imports `{target}`: only a protocol module may import a protocol module",
                    module.module
                ));
                continue;
            }
            match layer_of(&target, &std) {
                Some(target_layer) if target_layer > layer => violations.push(format!(
                    "{at}: {layer} module `{}` imports {target_layer} module `{target}`: a module imports only its own layer or a lower one",
                    module.module
                )),
                None if target.starts_with("std.") => violations.push(format!(
                    "{at}: `{}` imports `{target}`, which has no layer",
                    module.module
                )),
                _ => {}
            }
        }
        if layer == Layer::Protocol {
            continue;
        }
        for name in declared_names(&module.program) {
            let lower = name.name.to_lowercase();
            if let Some(protocol) = protocols
                .iter()
                .find(|p| lower.contains(p.as_str()) && !exempt(&module.module, p, &catalogued))
            {
                violations.push(format!(
                    "{}: {layer} module `{}` declares `{}`, which names the protocol `{protocol}`: protocol-specific items live in std.protocol.{protocol}",
                    module.at(name),
                    module.module,
                    name.name
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "std layering (docs/layering.md):\n{}",
        violations.join("\n")
    );
}

/// Every `.rs` file under `crates/*/src`, with its crate's name.
fn crate_sources() -> Vec<(String, PathBuf)> {
    let crates = repository().join("crates");
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&crates)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    let mut out = Vec::new();
    for crate_dir in entries {
        let name = crate_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let src = crate_dir.join("src");
        if !src.is_dir() {
            continue;
        }
        let mut sources = Vec::new();
        files(&src, "rs", &mut sources);
        out.extend(sources.into_iter().map(|path| (name.clone(), path)));
    }
    out
}

/// A path under the repository, as the reports and `docs/layering.md` write it.
fn relative(path: &Path) -> String {
    path.strip_prefix(repository())
        .unwrap_or(path)
        .display()
        .to_string()
}

#[test]
fn compiler_crates_name_no_application_protocol() {
    let protocols = protocol_names(&standard_library());
    let mut violations = Vec::new();
    for (name, path) in crate_sources() {
        if CRATES_EXEMPT.iter().any(|(exempt, _)| *exempt == name) {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for (number, line) in text.lines().enumerate() {
            let lower = line.to_lowercase();
            if let Some(protocol) = protocols.iter().find(|p| lower.contains(p.as_str())) {
                violations.push(format!(
                    "{}:{}: names the protocol `{protocol}`: the compiler, the generators and the substrate know no application protocol ({})",
                    relative(&path),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "crates layering (docs/layering.md):\n{}",
        violations.join("\n")
    );
}

/// The heading of the allow-list of rule 6 in `docs/layering.md`.
const STD_NAMES_HEADING: &str = "## Std names in the crates";

/// The allow-list of rule 6, read from `docs/layering.md`: for each file
/// (relative to the repository), the std names its string literals may
/// contain. Every row has a file, at least one name and a reason.
fn std_names_allowed(markdown: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut lines = markdown
        .lines()
        .skip_while(|line| line.trim() != STD_NAMES_HEADING);
    assert!(
        lines.next().is_some(),
        "docs/layering.md has no section `{STD_NAMES_HEADING}`"
    );
    let rows: Vec<&str> = lines
        .skip_while(|line| !line.starts_with('|'))
        .take_while(|line| line.starts_with('|'))
        .skip(2)
        .collect();
    assert!(
        !rows.is_empty(),
        "`{STD_NAMES_HEADING}` in docs/layering.md has no table rows"
    );
    let mut allowed: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in rows {
        let cells: Vec<&str> = row
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        let &[file, names, reason] = cells.as_slice() else {
            panic!("docs/layering.md: a row of `{STD_NAMES_HEADING}` has three cells: {row}");
        };
        assert!(
            !reason.is_empty(),
            "docs/layering.md: row without a reason: {row}"
        );
        let names: Vec<String> = names
            .split(',')
            .map(|name| name.trim().trim_matches('`').to_string())
            .collect();
        assert!(
            names.iter().all(|name| name.starts_with("std.")),
            "docs/layering.md: every name of a row is a std name: {row}"
        );
        allowed
            .entry(file.trim_matches('`').to_string())
            .or_default()
            .extend(names);
    }
    allowed
}

/// The string literals of Rust source `text`, each with the line it starts
/// on; comments are skipped. Handles escapes, raw strings, byte strings,
/// char literals and lifetimes, which is all `crates/*/src` uses.
fn string_literals(text: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = Vec::new();
    let mut line = 1;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\n' => {
                line += 1;
                i += 1;
            }
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if chars.get(i + 1) == Some(&'*') => {
                i += 2;
                while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    i += 1;
                }
                i += 2;
            }
            '\'' => {
                if chars.get(i + 1) == Some(&'\\') {
                    i += 3;
                    while i < chars.len() && chars[i] != '\'' {
                        i += 1;
                    }
                    i += 1;
                } else if chars.get(i + 2) == Some(&'\'') {
                    i += 3;
                } else {
                    i += 1;
                }
            }
            'r' if {
                let before = |k: usize| k.checked_sub(1).map(|k| chars[k]);
                let free = |c: Option<char>| !c.is_some_and(ident);
                free(before(i))
                    || (before(i) == Some('b') && free(i.checked_sub(1).and_then(before)))
            } =>
            {
                let mut j = i + 1;
                while chars.get(j) == Some(&'#') {
                    j += 1;
                }
                if chars.get(j) != Some(&'"') {
                    i += 1;
                    continue;
                }
                let hashes = j - i - 1;
                let start = line;
                let mut content = String::new();
                j += 1;
                while j < chars.len() {
                    if chars[j] == '"' && (1..=hashes).all(|h| chars.get(j + h) == Some(&'#')) {
                        break;
                    }
                    if chars[j] == '\n' {
                        line += 1;
                    }
                    content.push(chars[j]);
                    j += 1;
                }
                out.push((start, content));
                i = j + 1 + hashes;
            }
            '"' => {
                let start = line;
                let mut content = String::new();
                let mut j = i + 1;
                while j < chars.len() && chars[j] != '"' {
                    if chars[j] == '\\' {
                        content.push(chars[j]);
                        j += 1;
                    }
                    if let Some(&c) = chars.get(j) {
                        if c == '\n' {
                            line += 1;
                        }
                        content.push(c);
                    }
                    j += 1;
                }
                out.push((start, content));
                i = j + 1;
            }
            _ => i += 1,
        }
    }
    out
}

/// The std names in `literal`, each with its line offset inside it: `std.`
/// not preceded by an identifier character or a dot, then a lowercase
/// segment and every further `.segment`.
fn std_names(literal: &str) -> Vec<(usize, String)> {
    let chars: Vec<char> = literal.chars().collect();
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let starts = |c: Option<&char>| c.is_some_and(|c| c.is_ascii_lowercase() || *c == '_');
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 < chars.len() {
        let free = i == 0 || !(ident(chars[i - 1]) || chars[i - 1] == '.');
        if free && chars[i..i + 4] == ['s', 't', 'd', '.'] && starts(chars.get(i + 4)) {
            let mut j = i + 4;
            while j < chars.len()
                && (ident(chars[j])
                    || (chars[j] == '.'
                        && chars
                            .get(j + 1)
                            .is_some_and(|c| c.is_alphabetic() || *c == '_')))
            {
                j += 1;
            }
            let offset = chars[..i].iter().filter(|c| **c == '\n').count();
            out.push((offset, chars[i..j].iter().collect()));
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

#[test]
fn rule_6_reads_literals_not_comments() {
    let source = concat!(
        "// \"std.a\" in a comment\n",
        "/* \"std.b\" */ let c = '\"'; let d = '\\'';\n",
        "fn f<'a>(x: &'a str) -> &'a str { \"std.ok.Name and std.two\" }\n",
        "let r = r#\"say \"std.raw\"\"#; let b = b\"std.bytes\";\n",
        "let n = \"line\\\n std.next\"; let s = \"std::fmt, ignite_std.x, a.std.y, std.Upper\";\n",
    );
    let found: Vec<(usize, String)> = string_literals(source)
        .into_iter()
        .flat_map(|(line, literal)| {
            std_names(&literal)
                .into_iter()
                .map(move |(offset, name)| (line + offset, name))
        })
        .collect();
    let expected = [
        (3, "std.ok.Name"),
        (3, "std.two"),
        (4, "std.raw"),
        (4, "std.bytes"),
        (6, "std.next"),
    ];
    assert_eq!(
        found,
        expected.map(|(line, name)| (line, name.to_string())),
        "{source}"
    );

    let table = "x\n\n## Std names in the crates\n\ntext\n\n| file | names | reason |\n|---|---|---|\n| `a.rs` | `std.a`, `std.b` | why |\n| `a.rs` | `std.c` | why |\n\nafter\n";
    let allowed = std_names_allowed(table);
    assert_eq!(
        allowed,
        BTreeMap::from([(
            "a.rs".to_string(),
            BTreeSet::from(["std.a", "std.b", "std.c"].map(str::to_string))
        )])
    );
}

#[test]
fn compiler_crates_name_std_only_from_the_allow_list() {
    let markdown = std::fs::read_to_string(repository().join("docs/layering.md")).unwrap();
    let allowed = std_names_allowed(&markdown);
    let mut used: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut violations = Vec::new();
    for (_, path) in crate_sources() {
        let file = relative(&path);
        let text = std::fs::read_to_string(&path).unwrap();
        for (line, literal) in string_literals(&text) {
            for (offset, name) in std_names(&literal) {
                if allowed
                    .get(&file)
                    .is_some_and(|names| names.contains(&name))
                {
                    used.entry(file.clone()).or_default().insert(name);
                } else {
                    violations.push(format!(
                        "{file}:{}: names `{name}`: the compiler crates name a std module only from its home (docs/layering.md, \"Std names in the crates\"); use the home constant or add a row with the reason",
                        line + offset
                    ));
                }
            }
        }
    }
    for (file, names) in &allowed {
        for name in names {
            if !used.get(file).is_some_and(|used| used.contains(name)) {
                violations.push(format!(
                    "docs/layering.md: the row of `{file}` lists `{name}`, which no string literal of that file names: remove it"
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "std names in the crates (docs/layering.md, rule 6):\n{}",
        violations.join("\n")
    );
}
