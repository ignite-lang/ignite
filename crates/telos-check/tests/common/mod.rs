//! Load a standard-library root the way the driver does: every `.telos`
//! file under `root` is module `std.<relative path with dots>`, its
//! `package` must name that module, and modules are checked in import order
//! with `check_module`, each seeing the modules checked before it.
//!
//! Each test binary uses a different part of this module.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use telos_check::{Modules, SUBSTRATE_MODULES, check_module};
use telos_syntax::ast::Program;
use telos_syntax::{SourceFile, parse};

/// One parsed standard-library file.
pub struct StdFile {
    pub module: String,
    pub source: SourceFile,
    pub program: Program,
}

fn telos_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            telos_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "telos") {
            out.push(path);
        }
    }
}

/// The `std.*` modules a program imports, other than the substrate ones.
pub fn std_imports(program: &Program) -> Vec<String> {
    program
        .imports
        .iter()
        .filter_map(|import| {
            let segments = &import.path.segments;
            let module_len = if import.names.is_empty() {
                segments.len().saturating_sub(1)
            } else {
                segments.len()
            };
            let module = segments[..module_len]
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(".");
            (module.starts_with("std.") && !SUBSTRATE_MODULES.contains(&module.as_str()))
                .then_some(module)
        })
        .collect()
}

/// Parse every file under `root`, check them in import order and return the
/// files in that order with the resulting modules. Any parse or check
/// diagnostic, package mismatch or import cycle is an `Err` with the
/// rendered diagnostics.
pub fn load_std(root: &Path) -> Result<(Vec<StdFile>, Modules), String> {
    let mut paths = Vec::new();
    telos_files(root, &mut paths);
    let mut files: BTreeMap<String, StdFile> = BTreeMap::new();
    for path in paths {
        let relative = path.strip_prefix(root).expect("file under root");
        let module = std::iter::once("std".to_string())
            .chain(
                relative
                    .with_extension("")
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned()),
            )
            .collect::<Vec<_>>()
            .join(".");
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let source = SourceFile::new(path.display().to_string(), text);
        let program = parse(&source).map_err(|d| source.render_all(&d))?;
        let package = program.package.as_ref().map(|p| p.dotted());
        if package.as_deref() != Some(module.as_str()) {
            return Err(format!(
                "{}: package {package:?} does not name module `{module}`",
                path.display()
            ));
        }
        files.insert(
            module.clone(),
            StdFile {
                module,
                source,
                program,
            },
        );
    }

    let mut order: Vec<String> = Vec::new();
    let mut visiting: Vec<String> = Vec::new();
    fn visit(
        module: &str,
        files: &BTreeMap<String, StdFile>,
        order: &mut Vec<String>,
        visiting: &mut Vec<String>,
    ) -> Result<(), String> {
        if order.iter().any(|m| m == module) {
            return Ok(());
        }
        if visiting.iter().any(|m| m == module) {
            return Err(format!("import cycle through `{module}`: {visiting:?}"));
        }
        let Some(file) = files.get(module) else {
            return Ok(());
        };
        visiting.push(module.to_string());
        for dependency in std_imports(&file.program) {
            visit(&dependency, files, order, visiting)?;
        }
        visiting.pop();
        order.push(module.to_string());
        Ok(())
    }
    for module in files.keys() {
        visit(module, &files, &mut order, &mut visiting)?;
    }

    let mut modules = Modules::new();
    let mut ordered = Vec::new();
    for module in order {
        let file = files.remove(&module).expect("ordered module was loaded");
        let symbols =
            check_module(&file.program, &modules).map_err(|d| file.source.render_all(&d))?;
        modules.insert(module, symbols);
        ordered.push(file);
    }
    Ok((ordered, modules))
}
