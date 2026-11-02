//! Text builders for one LLVM IR module and one function in it.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::hash::{BuildHasherDefault, Hasher};

use ignite_check::Symbols;
use ignite_syntax::SourceFile;
use ignite_syntax::ast::Program;
use ignite_types::{QualName, Ty, TypedProgram};

use crate::layout::{Layout, TypeEnv, mangle};

/// The runtime's C ABI (`docs/llvm.md` section 5), declared in every module.
pub const RUNTIME: &[&str] = &[
    "declare void @ignite_rt_init(i32, ptr)",
    "declare ptr @ignite_rt_alloc(i64, i64)",
    "declare i8 @ignite_rt_spawn(ptr, ptr)",
    "declare void @ignite_rt_spawn_ready()",
    "declare void @ignite_rt_task_end()",
    "declare i64 @ignite_rt_arena_mark()",
    "declare void @ignite_rt_arena_release(i64)",
    "declare i8 @ignite_rt_arena_due(i64, i64)",
    "declare void @ignite_rt_arena_scratch(i8)",
    "declare void @ignite_rt_arena_scratch_clear()",
    "declare ptr @ignite_rt_arena_state() nounwind willreturn speculatable memory(none)",
    "declare i32 @ignite_rt_args_port(ptr, ptr, ptr)",
    "declare void @ignite_rt_exit(i32) noreturn",
    "declare void @ignite_rt_fail(ptr) noreturn",
    "declare void @ignite_rt_log(ptr)",
    "declare i32 @ignite_rt_tcp_bind(ptr, i64, ptr, ptr)",
    "declare i32 @ignite_rt_tcp_accept(i64, ptr, ptr)",
    "declare i64 @ignite_rt_tcp_local_port(i64)",
    "declare i32 @ignite_rt_tcp_connect(ptr, i64, ptr, ptr)",
    "declare i32 @ignite_rt_tcp_read(i64, i64, ptr, ptr)",
    "declare i32 @ignite_rt_tcp_write_all(i64, ptr, ptr)",
    "declare i32 @ignite_rt_tcp_close(i64, ptr)",
    "declare void @ignite_rt_tcp_release(i64)",
    "declare i32 @ignite_rt_stdin_read(i64, ptr, ptr)",
    "declare i32 @ignite_rt_stdin_is_terminal(ptr, ptr)",
    "declare i32 @ignite_rt_stdin_set_raw(i8, ptr)",
    "declare i32 @ignite_rt_stdout_write_all(ptr, ptr)",
    "declare void @ignite_rt_io_error_new(ptr, ptr)",
    "declare void @ignite_rt_text_from_bytes_lossy(ptr, ptr)",
    "declare void @ignite_rt_text_trim(ptr, ptr)",
    "declare void @ignite_rt_text_concat(ptr, ptr, ptr)",
    "declare i8 @ignite_rt_text_eq(ptr, ptr)",
    "declare i8 @ignite_rt_text_starts_with(ptr, ptr)",
    "declare void @ignite_rt_text_join(ptr, ptr, ptr)",
    "declare void @ignite_rt_int_to_text(ptr, i64)",
    "declare i8 @ignite_rt_int_parse(ptr, ptr)",
    "declare i32 @ignite_rt_tcp_set_idle_timeout(i64, i64, ptr)",
    "declare ptr @ignite_rt_store_new(ptr, ptr, ptr, i64, i64)",
    "declare i64 @ignite_rt_store_next_id(ptr)",
    "declare void @ignite_rt_store_put(ptr, i64, ptr)",
    "declare i8 @ignite_rt_store_get(ptr, i64, ptr)",
    "declare i8 @ignite_rt_store_remove(ptr, i64, ptr)",
    "declare void @ignite_rt_store_values(ptr, ptr)",
    "declare i8 @ignite_rt_store_update(ptr, i64, ptr, ptr, ptr)",
    "declare ptr @malloc(i64)",
    "declare void @free(ptr)",
    "declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)",
    "declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)",
    "declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64)",
    "declare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64)",
    "declare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64)",
    "declare { i8, i1 } @llvm.uadd.with.overflow.i8(i8, i8)",
    "declare { i8, i1 } @llvm.usub.with.overflow.i8(i8, i8)",
    "declare { i8, i1 } @llvm.umul.with.overflow.i8(i8, i8)",
];

/// A quoted global symbol: `@"std.console.first_word"`.
pub fn global(symbol: &str) -> String {
    format!("@\"{symbol}\"")
}

/// The bytes of a string as an LLVM `c"..."` constant body.
pub fn c_string(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &b in bytes {
        if b.is_ascii_graphic() && b != b'"' && b != b'\\' || b == b' ' {
            out.push(b as char);
        } else {
            let _ = write!(out, "\\{b:02X}");
        }
    }
    out
}

/// The glue functions of one type (`docs/llvm.md` sections 4 and 5), each
/// internal to the module that uses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glue {
    /// `void copy.<T>(ptr dst, ptr src)`: deep copy into the task's arena.
    Copy,
    /// `void heap_copy.<T>(ptr dst, ptr src)`: deep copy whose buffers come
    /// from `malloc`.
    HeapCopy,
    /// `void heap_free.<T>(ptr value)`: free the `malloc` buffers inside a
    /// heap copy.
    HeapFree,
    /// `ptr copy_to_heap.<T>(ptr value)`: a `malloc`'d heap copy.
    ToHeap,
    /// `void copy_from_heap.<T>(ptr dst, ptr heap)`: a heap copy into the
    /// task's arena.
    FromHeap,
    /// `void free_heap.<T>(ptr heap)`: free a heap copy.
    FreeHeap,
    /// `ptr store_update.<T>(ptr f, ptr old)`: the heap copy of `f` applied
    /// to a copy of the heap value `old` (`Store.update`).
    Update,
    /// `i1 eq.<T>(ptr a, ptr b)`: structural equality of the values at `a`
    /// and `b` (`==`, `!=`, `assert a == b`; `docs/slice.md` sections 12.5
    /// and 13.5).
    Eq,
    /// `void debug.<T>(ptr buf, ptr value)`: append the debug text of the
    /// value at `value` to the `Bytes` at `buf` (`docs/slice.md` section
    /// 13.6).
    Debug,
}

impl Glue {
    pub fn prefix(self) -> &'static str {
        match self {
            Glue::Copy => "copy",
            Glue::HeapCopy => "heap_copy",
            Glue::HeapFree => "heap_free",
            Glue::ToHeap => "copy_to_heap",
            Glue::FromHeap => "copy_from_heap",
            Glue::FreeHeap => "free_heap",
            Glue::Update => "store_update",
            Glue::Eq => "eq",
            Glue::Debug => "debug",
        }
    }
}

/// A module whose declarations instances of generic code are lowered from.
#[derive(Clone, Copy)]
pub struct Source<'e> {
    pub path: &'e str,
    pub program: &'e Program,
    pub symbols: &'e Symbols,
    pub types: &'e TypedProgram,
}

/// A generic declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Generic {
    Fn(QualName),
    Role(QualName, String),
    /// The default of a field of a generic record.
    Default(QualName, String),
}

/// One instance of a generic declaration, defined internal to the module
/// that uses it (`docs/llvm.md` section 3).
#[derive(Clone, Debug)]
pub struct Instance {
    pub symbol: String,
    pub generic: Generic,
    pub args: Vec<Ty>,
}

/// One IR module being built: functions, string constants, external
/// declarations, the glue and the instances of generic code its functions
/// need.
pub struct Module<'e> {
    pub env: &'e TypeEnv,
    name: String,
    functions: Vec<String>,
    defined: BTreeSet<String>,
    declared: BTreeMap<String, String>,
    strings: BTreeMap<Vec<u8>, String>,
    glue_done: BTreeSet<String>,
    glue_pending: Vec<(Glue, Ty)>,
    counter: usize,
    sources: Vec<Source<'e>>,
    instances_done: BTreeSet<String>,
    instances_pending: Vec<Instance>,
    files: BTreeMap<String, &'e SourceFile>,
    globals: Vec<String>,
    target: Option<TargetSpec>,
    debug: Debug,
    /// The symbols the module is emitted for ([`Module::set_roots`]);
    /// `None` keeps every function.
    roots: Option<BTreeSet<String>>,
}

/// The target an IR module is pinned to (`docs/llvm.md` section 1): its
/// `target triple` and `target datalayout` lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetSpec {
    pub triple: String,
    pub datalayout: String,
}

/// The debug metadata of one module (`docs/llvm.md` section 9): a
/// `DICompileUnit`, one `DIFile` per Ignite module whose source is known,
/// one `DISubprogram` per function lowered from Ignite code. Numbered
/// `!0`, `!1`, … in creation order.
#[derive(Default)]
struct Debug {
    /// The comp directory: the directory relative source paths are read
    /// from.
    directory: String,
    nodes: Vec<String>,
    /// `!N` of the compile unit and the subroutine type, once made.
    unit: Option<(usize, usize)>,
    /// `!N` of each module's file.
    files: BTreeMap<String, usize>,
    /// Start of each line of each module's source text, and whether the
    /// line is ASCII (its column is then a byte difference).
    lines: BTreeMap<String, Vec<(usize, bool)>>,
    /// `!N` of each `DILocation` made, by line, column and scope.
    locations: HashMap<(usize, usize, usize), usize, BuildHasherDefault<LocationHasher>>,
}

/// Only compiler-generated line, column and scope integers use this map.
/// Mix every component and both ends of the hash for bucket indices and tags.
#[derive(Default)]
struct LocationHasher(u64);

impl Hasher for LocationHasher {
    fn finish(&self) -> u64 {
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }

    fn write_usize(&mut self, value: usize) {
        self.0 = self.0.wrapping_mul(0x9e3779b185ebca87) ^ value as u64;
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = self.0.wrapping_mul(0x9e3779b185ebca87) ^ u64::from(*byte);
        }
    }
}

/// The largest column a `DILocation` holds; a later column is clamped.
const MAX_COLUMN: usize = 65535;

/// A string as an LLVM metadata string body.
fn md_string(text: &str) -> String {
    c_string(text.as_bytes())
}

impl<'e> Module<'e> {
    pub fn new(env: &'e TypeEnv, name: &str) -> Module<'e> {
        Module {
            env,
            name: name.to_string(),
            functions: Vec::new(),
            defined: BTreeSet::new(),
            declared: BTreeMap::new(),
            strings: BTreeMap::new(),
            glue_done: BTreeSet::new(),
            glue_pending: Vec::new(),
            counter: 0,
            sources: Vec::new(),
            instances_done: BTreeSet::new(),
            instances_pending: Vec::new(),
            files: BTreeMap::new(),
            globals: Vec::new(),
            target: None,
            debug: Debug {
                directory: ".".to_string(),
                ..Debug::default()
            },
            roots: None,
        }
    }

    /// Emit only the functions reachable from `roots` (`docs/llvm.md`
    /// section 1): a function is kept when a root, a global or a kept
    /// function names it. The standard-library module of a build is
    /// emitted for the symbols the program and its mains reference, so
    /// clang compiles only the std fns the executables can run, and glue
    /// only they use.
    pub fn set_roots(&mut self, roots: BTreeSet<String>) {
        self.roots = Some(roots);
    }

    /// Drop every function no root, global or kept function names.
    fn prune(&mut self) {
        let Some(roots) = &self.roots else {
            return;
        };
        // The graph lives only while function text is immutable. Borrow the
        // symbol slices instead of allocating a name for every IR reference.
        let kept = {
            let functions: Vec<(&str, BTreeSet<&str>)> = self
                .functions
                .iter()
                .map(|text| (defined_symbol(text), symbol_slices(text)))
                .collect();
            let index: BTreeMap<&str, usize> = functions
                .iter()
                .enumerate()
                .map(|(i, (symbol, _))| (*symbol, i))
                .collect();
            let mut pending: Vec<&str> = roots.iter().map(String::as_str).collect();
            let mut global_refs = BTreeSet::new();
            for global in &self.globals {
                global_refs.extend(symbol_slices(global));
            }
            pending.extend(global_refs.iter().copied());
            let mut kept = vec![false; functions.len()];
            while let Some(symbol) = pending.pop() {
                let Some(&i) = index.get(symbol) else {
                    continue;
                };
                if kept[i] {
                    continue;
                }
                kept[i] = true;
                pending.extend(functions[i].1.iter().copied());
            }
            kept
        };
        let mut i = 0;
        self.functions.retain(|_| {
            let keep = kept[i];
            i += 1;
            keep
        });
    }

    /// Pin the module to `target`: its first lines name the triple and
    /// the data layout.
    pub fn set_target(&mut self, target: Option<&TargetSpec>) {
        self.target = target.cloned();
    }

    /// The directory relative source paths are relative to, for the
    /// compile unit of the debug metadata.
    pub fn set_directory(&mut self, directory: Option<&str>) {
        if let Some(directory) = directory {
            self.debug.directory = directory.to_string();
        }
    }

    fn node(&mut self, text: String) -> usize {
        self.debug.nodes.push(text);
        self.debug.nodes.len() - 1
    }

    /// The compile unit and the subroutine type, made on first use.
    fn unit(&mut self) -> (usize, usize) {
        if let Some(unit) = self.debug.unit {
            return unit;
        }
        let file = self.node(format!(
            "!DIFile(filename: \"{}.ll\", directory: \"{}\")",
            md_string(&self.name),
            md_string(&self.debug.directory)
        ));
        let cu = self.node(format!(
            "distinct !DICompileUnit(language: DW_LANG_C, file: !{file}, producer: \"ignite\", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, nameTableKind: None)"
        ));
        let ty = self.node("!DISubroutineType(types: !{})".to_string());
        self.debug.unit = Some((cu, ty));
        (cu, ty)
    }

    /// The `DIFile` of module `path`, when its source is known.
    fn di_file(&mut self, path: &str) -> Option<usize> {
        if let Some(&file) = self.debug.files.get(path) {
            return Some(file);
        }
        let source = self.file(path)?;
        self.unit();
        let full = std::path::Path::new(&source.path);
        let name = full
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| source.path.clone());
        let directory = match full.parent().map(|p| p.to_string_lossy().into_owned()) {
            Some(parent) if full.is_absolute() => parent,
            Some(parent) if !parent.is_empty() => {
                format!("{}/{parent}", self.debug.directory)
            }
            _ => self.debug.directory.clone(),
        };
        let file = self.node(format!(
            "!DIFile(filename: \"{}\", directory: \"{}\")",
            md_string(&name),
            md_string(&directory)
        ));
        self.debug.files.insert(path.to_string(), file);
        let mut lines = vec![(0, true)];
        for (i, b) in source.text.bytes().enumerate() {
            if b == b'\n' {
                lines.push((i + 1, true));
            } else if !b.is_ascii()
                && let Some(last) = lines.last_mut()
            {
                last.1 = false;
            }
        }
        self.debug.lines.insert(path.to_string(), lines);
        Some(file)
    }

    /// Line and column (1-based, columns in characters, as
    /// `SourceFile::line_col`) of byte `offset` of module `path`.
    pub fn line_col(&mut self, path: &str, offset: usize) -> Option<(usize, usize)> {
        self.di_file(path)?;
        let source = self.file(path)?;
        let lines = &self.debug.lines[path];
        let mut offset = offset.min(source.text.len());
        while !source.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = lines.partition_point(|&(s, _)| s <= offset);
        let (start, ascii) = lines[line - 1];
        let column = if ascii {
            offset - start + 1
        } else {
            source.text[start..offset].chars().count() + 1
        };
        Some((line, column.min(MAX_COLUMN)))
    }

    /// A `DISubprogram` for the function `symbol` lowered from module
    /// `path`, declared at byte `offset`; `None` when the module's source
    /// is unknown (no debug metadata).
    pub fn subprogram(&mut self, path: &str, symbol: &str, offset: usize) -> Option<usize> {
        let file = self.di_file(path)?;
        let (line, _) = self.line_col(path, offset)?;
        let (cu, ty) = self.unit();
        let name = md_string(symbol);
        Some(self.node(format!(
            "distinct !DISubprogram(name: \"{name}\", linkageName: \"{name}\", scope: !{file}, file: !{file}, line: {line}, type: !{ty}, scopeLine: {line}, spFlags: DISPFlagDefinition, unit: !{cu})"
        )))
    }

    /// The `!N` of the `DILocation` at `line` and `column` in the
    /// subprogram `!scope`, made once per module; instructions refer to it
    /// as `!dbg !N`.
    pub fn location(&mut self, line: usize, column: usize, scope: usize) -> usize {
        if let Some(&node) = self.debug.locations.get(&(line, column, scope)) {
            return node;
        }
        let node = self.node(format!(
            "!DILocation(line: {line}, column: {column}, scope: !{scope})"
        ));
        self.debug.locations.insert((line, column, scope), node);
        node
    }

    /// The source files `assert` and test `?` read text and positions
    /// from, by module path.
    pub fn set_files(&mut self, files: &BTreeMap<String, &'e SourceFile>) {
        self.files = files.clone();
    }

    /// The source file of module `path`, when known.
    pub fn file(&self, path: &str) -> Option<&'e SourceFile> {
        self.files.get(path).copied()
    }

    /// Add a global definition (one line), emitted after the string
    /// constants.
    pub fn global_definition(&mut self, text: String) {
        self.globals.push(text);
    }

    /// Make the declarations of `source` available for instances.
    pub fn add_source(&mut self, source: Source<'e>) {
        if !self.sources.iter().any(|s| s.path == source.path) {
            self.sources.push(source);
        }
    }

    /// The module `path` registered with [`Module::add_source`].
    pub fn source(&self, path: &str) -> Option<Source<'e>> {
        self.sources.iter().find(|s| s.path == path).copied()
    }

    /// Request `instance`, defined at the end unless already requested.
    pub fn instance(&mut self, instance: Instance) {
        if self.instances_done.insert(instance.symbol.clone()) {
            self.instances_pending.push(instance);
        }
    }

    /// A number unique in this module (spawn thunks).
    pub fn next_id(&mut self) -> usize {
        self.counter += 1;
        self.counter
    }

    /// Add a function definition. `symbol` is recorded so it is not also
    /// declared.
    pub fn define(&mut self, symbol: &str, text: String) {
        self.defined.insert(symbol.to_string());
        self.functions.push(text);
    }

    /// Define the internal helper `symbol` with the full definition `text`,
    /// once per module.
    pub fn helper(&mut self, symbol: &str, text: &str) {
        if self.defined.insert(symbol.to_string()) {
            self.functions.push(text.to_string());
        }
    }

    /// Whether the internal helper `symbol` is already defined in this module.
    pub fn has(&self, symbol: &str) -> bool {
        self.defined.contains(symbol)
    }

    /// Declare an external function, once.
    pub fn declare(&mut self, symbol: &str, ret: &str, params: &[String]) {
        self.declared
            .entry(symbol.to_string())
            .or_insert_with(|| format!("declare {ret} {}({})", global(symbol), params.join(", ")));
    }

    /// Declare `symbol` with the full declaration `text` (for a declaration
    /// that needs an attribute, such as `_setjmp`'s `returns_twice`), once
    /// per module.
    pub fn declare_raw(&mut self, symbol: &str, text: &str) {
        self.declared
            .entry(symbol.to_string())
            .or_insert_with(|| text.to_string());
    }

    /// The global holding the bytes of a string literal.
    pub fn string(&mut self, bytes: &[u8]) -> String {
        let count = self.strings.len();
        self.strings
            .entry(bytes.to_vec())
            .or_insert_with(|| format!("@.str.{count}"))
            .clone()
    }

    /// The symbol of the copy glue of `ty`, emitted at the end.
    pub fn glue(&mut self, ty: &Ty) -> String {
        self.glue_of(Glue::Copy, ty)
    }

    /// The symbol of glue function `kind` of `ty`, emitted at the end.
    pub fn glue_of(&mut self, kind: Glue, ty: &Ty) -> String {
        let symbol = format!("{}.{}", kind.prefix(), mangle(ty));
        if self.glue_done.insert(symbol.clone()) {
            self.glue_pending.push((kind, ty.clone()));
        }
        global(&symbol)
    }

    pub fn finish(mut self) -> String {
        loop {
            if let Some(instance) = self.instances_pending.pop() {
                crate::lower::define_instance(&mut self, &instance);
            } else if let Some((kind, ty)) = self.glue_pending.pop() {
                let text = crate::glue::function(&mut self, kind, &ty);
                self.functions.push(text);
            } else {
                break;
            }
        }
        self.prune();
        let mut out = String::new();
        if let Some(target) = &self.target {
            let _ = writeln!(out, "target triple = \"{}\"", target.triple);
            let _ = writeln!(out, "target datalayout = \"{}\"", target.datalayout);
        }
        let _ = writeln!(out, "; ModuleID = '{}'", self.name);
        let _ = writeln!(out, "source_filename = \"{}\"", self.name);
        out.push('\n');
        out.push_str("@ignite.empty = private constant [8 x i8] zeroinitializer, align 8\n");
        for (bytes, name) in &self.strings {
            let _ = writeln!(
                out,
                "{name} = private unnamed_addr constant [{} x i8] c\"{}\", align 1",
                bytes.len(),
                c_string(bytes)
            );
        }
        for global in &self.globals {
            out.push_str(global);
            out.push('\n');
        }
        out.push('\n');
        for line in RUNTIME {
            out.push_str(line);
            out.push('\n');
        }
        for (symbol, line) in &self.declared {
            if !self.defined.contains(symbol) {
                out.push_str(line);
                out.push('\n');
            }
        }
        out.push_str(crate::glue::HELPERS);
        for function in &self.functions {
            out.push('\n');
            out.push_str(function);
        }
        if let Some((cu, _)) = self.debug.unit {
            let count = self.debug.nodes.len();
            let _ = writeln!(out, "\n!llvm.dbg.cu = !{{!{cu}}}");
            let _ = writeln!(out, "!llvm.module.flags = !{{!{count}, !{}}}", count + 1);
            for (i, node) in self.debug.nodes.iter().enumerate() {
                let _ = writeln!(out, "!{i} = {node}");
            }
            let _ = writeln!(out, "!{count} = !{{i32 7, !\"Dwarf Version\", i32 5}}");
            let _ = writeln!(
                out,
                "!{} = !{{i32 2, !\"Debug Info Version\", i32 3}}",
                count + 1
            );
        }
        out
    }
}

/// The symbol a function definition text defines: the first `@name` of its
/// `define` line, unquoted.
fn defined_symbol(text: &str) -> &str {
    let header = text.lines().next().unwrap_or("");
    symbol_slices(header).into_iter().next().unwrap_or_default()
}

/// Every symbol `text` names as `@name` or `@"name"`, unquoted. A symbol
/// character is a letter, digit, `-`, `$`, `.` or `_`; a quoted name holds
/// any character but `"`.
pub fn referenced_symbols(text: &str) -> BTreeSet<String> {
    let mut symbols = BTreeSet::new();
    scan_symbols(text, |symbol| {
        symbols.insert(symbol.to_owned());
    });
    symbols
}

/// References borrowed from immutable IR text, for the temporary pruning graph.
fn symbol_slices(text: &str) -> BTreeSet<&str> {
    let mut symbols = BTreeSet::new();
    scan_symbols(text, |symbol| {
        symbols.insert(symbol);
    });
    symbols
}

fn scan_symbols<'a>(text: &'a str, mut emit: impl FnMut(&'a str)) {
    let bytes = text.as_bytes();
    let mut i = 0;
    while let Some(at) = text[i..].find('@') {
        let start = i + at + 1;
        if bytes.get(start) == Some(&b'"') {
            match text[start + 1..].find('"') {
                Some(len) => {
                    emit(&text[start + 1..start + 1 + len]);
                    i = start + 1 + len + 1;
                }
                None => break,
            }
        } else {
            let len = bytes[start..]
                .iter()
                .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'$' | b'.' | b'_'))
                .count();
            if len > 0 {
                emit(&text[start..start + len]);
            }
            i = start + len;
        }
    }
}

/// One function body: registers, labels, entry-block allocas.
pub struct Func {
    header: String,
    allocas: String,
    body: String,
    next: usize,
    terminated: bool,
    current: String,
    /// Allocas made since the function began, as `(size, align, register)`.
    made: Vec<(u64, u64, String)>,
    /// Allocas whose values are dead, for [`Func::alloca`] to hand out again.
    free: Vec<(u64, u64, String)>,
    /// `!N` of the function's `DISubprogram`; without one no instruction
    /// gets a location.
    scope: Option<usize>,
    /// The `!dbg` suffix of every instruction emitted now.
    location: String,
}

impl Func {
    /// `header` is the `define ... {` line without the brace.
    pub fn new(header: String) -> Func {
        Func {
            header,
            allocas: String::new(),
            body: String::new(),
            next: 0,
            terminated: false,
            current: "entry".to_string(),
            made: Vec::new(),
            free: Vec::new(),
            scope: None,
            location: String::new(),
        }
    }

    /// Attach the function to the `DISubprogram` `!scope`.
    pub fn set_scope(&mut self, scope: Option<usize>) {
        self.scope = scope;
        self.location.clear();
    }

    /// `!N` of the function's `DISubprogram`, if it has one.
    pub fn scope(&self) -> Option<usize> {
        self.scope
    }

    /// The source position of the instructions emitted from now on, the
    /// `DILocation` `!node` ([`Module::location`]); returns the previous
    /// one for [`Func::restore_location`].
    pub fn set_location(&mut self, node: usize) -> String {
        if self.scope.is_none() {
            return String::new();
        }
        std::mem::replace(&mut self.location, format!(", !dbg !{node}"))
    }

    pub fn restore_location(&mut self, previous: String) {
        if self.scope.is_some() {
            self.location = previous;
        }
    }

    pub fn reg(&mut self) -> String {
        self.next += 1;
        format!("%t{}", self.next)
    }

    pub fn label(&mut self) -> String {
        self.next += 1;
        format!("L{}", self.next)
    }

    /// The label of the block instructions are emitted into.
    pub fn current(&self) -> &str {
        &self.current
    }

    pub fn is_terminated(&self) -> bool {
        self.terminated
    }

    /// One instruction. After a terminator a fresh (dead) block is opened.
    pub fn ins(&mut self, text: impl AsRef<str>) {
        if self.terminated {
            let dead = self.label();
            self.place(&dead);
        }
        self.body.push_str("  ");
        self.body.push_str(text.as_ref());
        self.body.push_str(&self.location);
        self.body.push('\n');
    }

    /// `%r = <text>`; returns `%r`.
    pub fn val(&mut self, text: impl AsRef<str>) -> String {
        let reg = self.reg();
        self.ins(format!("{reg} = {}", text.as_ref()));
        reg
    }

    pub fn term(&mut self, text: impl AsRef<str>) {
        self.ins(text);
        self.terminated = true;
    }

    pub fn br(&mut self, label: &str) {
        self.term(format!("br label %{label}"));
    }

    pub fn cond_br(&mut self, cond: &str, then: &str, otherwise: &str) {
        self.term(format!("br i1 {cond}, label %{then}, label %{otherwise}"));
    }

    /// Start block `label`, falling through from the current block.
    pub fn place(&mut self, label: &str) {
        if !self.terminated {
            self.body.push_str(&format!("  br label %{label}\n"));
        }
        self.body.push_str(label);
        self.body.push_str(":\n");
        self.terminated = false;
        self.current = label.to_string();
    }

    /// A stack slot in the entry block.
    pub fn alloca(&mut self, layout: Layout) -> String {
        let (size, align) = (layout.size.max(1), layout.align.max(1));
        let reg = match self
            .free
            .iter()
            .rposition(|(s, a, _)| *s == size && *a == align)
        {
            Some(at) => self.free.swap_remove(at).2,
            None => {
                let reg = self.reg();
                let _ = writeln!(
                    self.allocas,
                    "  {reg} = alloca [{size} x i8], align {align}"
                );
                reg
            }
        };
        self.made.push((size, align, reg.clone()));
        reg
    }

    /// The number of allocas handed out so far, for [`Func::reuse_allocas`].
    pub fn alloca_mark(&self) -> usize {
        self.made.len()
    }

    /// Declare the allocas handed out since `mark` dead, except `keep`:
    /// later calls of [`Func::alloca`] may return them again. The caller
    /// guarantees that no live value points into them. A chain of binary
    /// operators uses this after each step, so a 100,000-term chain needs
    /// a constant number of stack slots.
    pub fn reuse_allocas(&mut self, mark: usize, keep: &[&str]) {
        let made: Vec<_> = self.made.drain(mark..).collect();
        for slot in made {
            if keep.contains(&slot.2.as_str()) {
                self.made.push(slot);
            } else {
                self.free.push(slot);
            }
        }
    }

    /// Close the function: `fallback` ends a block that is still open.
    pub fn finish(mut self, fallback: &str) -> String {
        if !self.terminated {
            self.term(fallback);
        }
        let dbg = self
            .scope
            .map(|scope| format!(" !dbg !{scope}"))
            .unwrap_or_default();
        format!(
            "{}{dbg} {{\nentry:\n{}{}}}\n",
            self.header, self.allocas, self.body
        )
    }
}
