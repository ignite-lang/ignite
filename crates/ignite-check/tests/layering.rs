//! The layers of the standard library and of the compiler crates
//! (`docs/layering.md`): a general layer knows no application protocol.
//!
//! Every std module is parsed with the real parser and placed in a layer by
//! [`LAYERS`] or, for `std.protocol.*`, `std.rfc` and `std.rfc.*`, by its
//! path. A module imports only
//! modules of its own layer or a lower one; no module outside
//! `std.protocol.*` imports a protocol module or declares a name that
//! contains a protocol's name; and no file under `crates/*/src` mentions
//! one, except the crates in [`CRATES_EXEMPT`]. The protocol names are read
//! from std: the `protocol` declarations and file names under
//! `std/protocol/`, so a new protocol is covered without editing this file.

use std::collections::BTreeSet;
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
    assert!(
        protocols.contains("telnet") && protocols.contains("http"),
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
            if let Some(protocol) = protocols.iter().find(|p| lower.contains(p.as_str())) {
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

#[test]
fn compiler_crates_name_no_application_protocol() {
    let protocols = protocol_names(&standard_library());
    let crates = repository().join("crates");
    let mut violations = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&crates)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for crate_dir in entries {
        let name = crate_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        if CRATES_EXEMPT.iter().any(|(exempt, _)| *exempt == name) {
            continue;
        }
        let src = crate_dir.join("src");
        if !src.is_dir() {
            continue;
        }
        let mut sources = Vec::new();
        files(&src, "rs", &mut sources);
        for path in sources {
            let text = std::fs::read_to_string(&path).unwrap();
            for (number, line) in text.lines().enumerate() {
                let lower = line.to_lowercase();
                if let Some(protocol) = protocols.iter().find(|p| lower.contains(p.as_str())) {
                    violations.push(format!(
                        "{}:{}: names the protocol `{protocol}`: the compiler, the generators and the substrate know no application protocol ({})",
                        path.strip_prefix(repository()).unwrap_or(&path).display(),
                        number + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "crates layering (docs/layering.md):\n{}",
        violations.join("\n")
    );
}
