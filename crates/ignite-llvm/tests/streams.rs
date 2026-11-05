//! Slice 14 row 11 (`docs/slice.md` 18.0, 18.11, D11): the repository's
//! standard library, lowered as the driver does, gives each instance of a
//! `ByteStream`-bounded fn or role the entries of its own stream kind. The
//! terminal's std IR defines `Telnet.serve<TcpStream>` over the
//! `ignite_rt_tcp_*` entries; the test build of `std/protocol/telnet.ignite`
//! defines `session<PipeStream>` over the `ignite_rt_pipe_*` entries; and
//! no instance over one kind calls a stream entry of the other.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ignite_check::{Modules, Symbols, check_module};
use ignite_syntax::SourceFile;
use ignite_syntax::ast::Program;
use ignite_types::{Ty, TypedModules, TypedProgram, type_module};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One checked and typed module.
struct Module {
    path: String,
    source: SourceFile,
    program: Program,
    symbols: Symbols,
    types: TypedProgram,
}

impl Module {
    fn input(&self) -> ignite_llvm::ModuleInput<'_> {
        ignite_llvm::ModuleInput {
            path: &self.path,
            program: &self.program,
            symbols: &self.symbols,
            types: &self.types,
        }
    }
}

fn ignite_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            ignite_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "ignite") {
            out.push(path);
        }
    }
}

fn imported_modules(program: &Program) -> Vec<String> {
    program
        .imports
        .iter()
        .map(|import| {
            let segments = &import.path.segments;
            let count = if import.names.is_empty() {
                segments.len() - 1
            } else {
                segments.len()
            };
            segments[..count]
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(".")
        })
        .collect()
}

/// Check and type `files` (module path, file) in import order against
/// `modules` and `typed`, which gain each module.
fn load(
    files: Vec<(String, PathBuf)>,
    modules: &mut Modules,
    typed: &mut TypedModules,
) -> Vec<Module> {
    let mut pending: Vec<(String, SourceFile, Program)> = files
        .into_iter()
        .map(|(path, file)| {
            let text = std::fs::read_to_string(&file).unwrap();
            let source = SourceFile::new(file.display().to_string(), text);
            let program = ignite_syntax::parse(&source)
                .unwrap_or_else(|d| panic!("{}", source.render_all(&d)));
            (path, source, program)
        })
        .collect();
    let names: BTreeSet<String> = pending.iter().map(|(m, _, _)| m.clone()).collect();
    let mut out = Vec::new();
    while !pending.is_empty() {
        let ready = pending
            .iter()
            .position(|(_, _, program)| {
                imported_modules(program)
                    .iter()
                    .all(|m| !names.contains(m) || modules.contains_key(m))
            })
            .expect("imports form a cycle");
        let (path, source, program) = pending.remove(ready);
        let symbols =
            check_module(&program, modules).unwrap_or_else(|d| panic!("{}", source.render_all(&d)));
        let types = type_module(&program, &symbols, typed)
            .unwrap_or_else(|d| panic!("{}", source.render_all(&d)));
        modules.insert(path.clone(), symbols.clone());
        typed.insert(path.clone(), types.exports());
        out.push(Module {
            path,
            source,
            program,
            symbols,
            types,
        });
    }
    out
}

/// The repository's standard library, every module, and the tables the
/// programs are checked and typed against.
fn standard_library() -> (Vec<Module>, Modules, TypedModules) {
    let root = workspace().join("std");
    let mut files = Vec::new();
    ignite_files(&root, &mut files);
    let files = files
        .into_iter()
        .map(|file| {
            let relative = file.strip_prefix(&root).unwrap().with_extension("");
            let path = std::iter::once("std".to_string())
                .chain(
                    relative
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned()),
                )
                .collect::<Vec<_>>()
                .join(".");
            (path, file)
        })
        .collect();
    let mut modules = Modules::new();
    let mut typed = TypedModules::new();
    let library = load(files, &mut modules, &mut typed);
    (library, modules, typed)
}

/// The text of the function defined as `symbol` in `ir`, from its `define`
/// line to its closing brace.
fn function<'a>(ir: &'a str, symbol: &str) -> &'a str {
    let quoted = format!("@\"{symbol}\"(");
    let start = ir
        .match_indices("define ")
        .map(|(i, _)| i)
        .find(|&i| ir[i..].lines().next().is_some_and(|l| l.contains(&quoted)))
        .unwrap_or_else(|| panic!("{symbol} is not defined"));
    let end = ir[start..].find("\n}\n").expect("end of the function") + start;
    &ir[start..end]
}

/// Every function of `ir` that is an instance over a stream type: its
/// symbol and its text.
fn stream_instances(ir: &str) -> Vec<(String, &str)> {
    ir.lines()
        .filter(|line| line.starts_with("define "))
        .filter_map(|line| line.split('"').nth(1))
        .filter(|symbol| {
            symbol
                .split_once('<')
                .is_some_and(|(_, args)| args.contains("TcpStream") || args.contains("PipeStream"))
        })
        .map(|symbol| (symbol.to_string(), function(ir, symbol)))
        .collect()
}

/// The stream entries of the other kind that `text` calls. `tcp_own`,
/// `tcp_retain` and `tcp_release` take any handle (18.9), so they belong
/// to neither kind.
fn foreign_calls(text: &str, other: &str) -> Vec<String> {
    text.match_indices(other)
        .map(|(i, _)| {
            let rest = &text[i..];
            rest[..rest.find('(').unwrap_or(rest.len())].to_string()
        })
        .filter(|call| {
            ![
                "@ignite_rt_tcp_own",
                "@ignite_rt_tcp_retain",
                "@ignite_rt_tcp_release",
            ]
            .contains(&call.as_str())
        })
        .collect()
}

/// No instance over `TcpStream` calls a pipe entry, and none over
/// `PipeStream` a TCP stream entry.
fn assert_own_kinds(ir: &str) -> usize {
    let instances = stream_instances(ir);
    for (symbol, text) in &instances {
        let (_, args) = symbol.split_once('<').unwrap();
        if args.contains("TcpStream") && !args.contains("PipeStream") {
            let foreign = foreign_calls(text, "@ignite_rt_pipe_");
            assert!(foreign.is_empty(), "{symbol} calls {foreign:?}");
        }
        if args.contains("PipeStream") && !args.contains("TcpStream") {
            let foreign = foreign_calls(text, "@ignite_rt_tcp_");
            assert!(foreign.is_empty(), "{symbol} calls {foreign:?}");
        }
    }
    instances.len()
}

#[test]
fn the_terminal_serves_through_the_tcp_instance_of_telnet_serve() {
    let (library, mut modules, mut typed) = standard_library();
    let dir = workspace().join("examples/terminal");
    let siblings = load(
        vec![
            (
                "examples.terminal.commands".to_string(),
                dir.join("commands.ignite"),
            ),
            (
                "examples.terminal.command_schema".to_string(),
                dir.join("command_schema.ignite"),
            ),
        ],
        &mut modules,
        &mut typed,
    );
    let terminal = load(
        vec![("examples.terminal".to_string(), dir.join("terminal.ignite"))],
        &mut modules,
        &mut typed,
    );
    let std_inputs: Vec<_> = library.iter().map(Module::input).collect();
    let options = ignite_llvm::Options {
        siblings: siblings.iter().map(Module::input).collect(),
        sources: library
            .iter()
            .chain(&siblings)
            .chain(&terminal)
            .map(|m| (m.path.clone(), &m.source))
            .collect(),
        ..Default::default()
    };
    let generated = ignite_llvm::generate_with(&std_inputs, &terminal[0].input(), &options);
    let serve = function(
        &generated.std,
        &ignite_llvm::instance_symbol("std.protocol.telnet.Telnet.serve", &[Ty::TcpStream]),
    );
    for entry in ["@ignite_rt_tcp_set_idle_timeout(", "@ignite_rt_tcp_close("] {
        assert!(
            serve.contains(entry),
            "Telnet.serve<TcpStream> has no {entry}:\n{serve}"
        );
    }
    assert!(!serve.contains("@ignite_rt_pipe_"), "{serve}");
    let all = format!("{}\n{}", generated.std, generated.program);
    assert!(assert_own_kinds(&all) > 0, "no stream instance in:\n{all}");
}

#[test]
fn the_telnet_tests_run_the_session_over_the_pipe_instance() {
    let (library, _, _) = standard_library();
    let telnet = library
        .iter()
        .find(|m| m.path == "std.protocol.telnet")
        .expect("std.protocol.telnet");
    let std_inputs: Vec<_> = library.iter().map(Module::input).collect();
    let options = ignite_llvm::Options {
        test_modules: vec![telnet.path.clone()],
        sources: library
            .iter()
            .map(|m| (m.path.clone(), &m.source))
            .collect(),
        ..Default::default()
    };
    let generated = ignite_llvm::generate_tests(&std_inputs, &[], &options);
    let all = format!("{}\n{}", generated.std, generated.program);
    let session = function(
        &all,
        &ignite_llvm::instance_symbol("std.protocol.telnet.session", &[Ty::PipeStream]),
    );
    assert!(
        session.contains("@ignite_rt_pipe_read("),
        "session<PipeStream> has no @ignite_rt_pipe_read:\n{session}"
    );
    assert!(
        foreign_calls(session, "@ignite_rt_tcp_").is_empty(),
        "{session}"
    );
    let tcp_serve = function(
        &all,
        &ignite_llvm::instance_symbol("std.protocol.telnet.Telnet.serve", &[Ty::TcpStream]),
    );
    assert!(tcp_serve.contains("@ignite_rt_tcp_close("), "{tcp_serve}");
    assert!(assert_own_kinds(&all) > 1, "{all}");
}
