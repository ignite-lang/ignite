//! Text builders for one LLVM IR module and one function in it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use telos_check::Symbols;
use telos_syntax::SourceFile;
use telos_syntax::ast::Program;
use telos_types::{QualName, Ty, TypedProgram};

use crate::layout::{Layout, TypeEnv, mangle};

/// The runtime's C ABI (`docs/llvm.md` section 5), declared in every module.
pub const RUNTIME: &[&str] = &[
    "declare void @telos_rt_init(i32, ptr)",
    "declare ptr @telos_rt_alloc(i64, i64)",
    "declare void @telos_rt_spawn(ptr, ptr)",
    "declare void @telos_rt_spawn_ready()",
    "declare void @telos_rt_task_end()",
    "declare i64 @telos_rt_arena_mark()",
    "declare void @telos_rt_arena_release(i64)",
    "declare i8 @telos_rt_arena_due(i64, i64)",
    "declare void @telos_rt_arena_scratch(i8)",
    "declare void @telos_rt_arena_scratch_clear()",
    "declare ptr @telos_rt_arena_state() nounwind willreturn speculatable memory(none)",
    "declare i8 @telos_rt_spawn_copy(ptr, ptr, i64)",
    "declare i32 @telos_rt_args_port(ptr, ptr, ptr)",
    "declare void @telos_rt_exit(i32) noreturn",
    "declare void @telos_rt_fail(ptr) noreturn",
    "declare void @telos_rt_panic(ptr) noreturn",
    "declare void @telos_rt_log(ptr)",
    "declare i32 @telos_rt_tcp_bind(ptr, i64, ptr, ptr)",
    "declare i32 @telos_rt_tcp_accept(i64, ptr, ptr)",
    "declare i64 @telos_rt_tcp_local_port(i64)",
    "declare i32 @telos_rt_tcp_connect(ptr, i64, ptr, ptr)",
    "declare i32 @telos_rt_tcp_read(i64, i64, ptr, ptr)",
    "declare i32 @telos_rt_tcp_write_all(i64, ptr, ptr)",
    "declare i32 @telos_rt_tcp_close(i64, ptr)",
    "declare void @telos_rt_tcp_release(i64)",
    "declare i32 @telos_rt_stdin_read(i64, ptr, ptr)",
    "declare i32 @telos_rt_stdin_is_terminal(ptr, ptr)",
    "declare i32 @telos_rt_stdin_set_raw(i8, ptr)",
    "declare i32 @telos_rt_stdout_write_all(ptr, ptr)",
    "declare void @telos_rt_io_error_new(ptr, ptr)",
    "declare void @telos_rt_text_from_bytes_lossy(ptr, ptr)",
    "declare void @telos_rt_text_trim(ptr, ptr)",
    "declare void @telos_rt_text_concat(ptr, ptr, ptr)",
    "declare i8 @telos_rt_text_eq(ptr, ptr)",
    "declare i8 @telos_rt_text_starts_with(ptr, ptr)",
    "declare void @telos_rt_text_join(ptr, ptr, ptr)",
    "declare void @telos_rt_int_to_text(ptr, i64)",
    "declare i8 @telos_rt_int_parse(ptr, ptr)",
    "declare i32 @telos_rt_tcp_set_idle_timeout(i64, i64, ptr)",
    "declare ptr @telos_rt_store_new(ptr, ptr, ptr, i64, i64)",
    "declare i64 @telos_rt_store_next_id(ptr)",
    "declare void @telos_rt_store_put(ptr, i64, ptr)",
    "declare i8 @telos_rt_store_get(ptr, i64, ptr)",
    "declare i8 @telos_rt_store_remove(ptr, i64, ptr)",
    "declare void @telos_rt_store_values(ptr, ptr)",
    "declare i8 @telos_rt_store_update(ptr, i64, ptr, ptr, ptr)",
    "declare ptr @malloc(i64)",
    "declare void @free(ptr)",
    "declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)",
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
    /// and `b` (`assert a == b`, `docs/slice.md` section 12.5).
    Eq,
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
        }
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

    /// Declare an external function, once.
    pub fn declare(&mut self, symbol: &str, ret: &str, params: &[String]) {
        self.declared
            .entry(symbol.to_string())
            .or_insert_with(|| format!("declare {ret} {}({})", global(symbol), params.join(", ")));
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
        let mut out = String::new();
        let _ = writeln!(out, "; ModuleID = '{}'", self.name);
        let _ = writeln!(out, "source_filename = \"{}\"", self.name);
        out.push('\n');
        out.push_str("@telos.empty = private constant [8 x i8] zeroinitializer, align 8\n");
        for (name, text) in crate::glue::MESSAGES {
            let _ = writeln!(
                out,
                "{name} = private unnamed_addr constant [{} x i8] c\"{text}\", align 1",
                text.len()
            );
        }
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
        out
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
        let reg = self.reg();
        let _ = writeln!(
            self.allocas,
            "  {reg} = alloca [{} x i8], align {}",
            layout.size.max(1),
            layout.align.max(1)
        );
        reg
    }

    /// Close the function: `fallback` ends a block that is still open.
    pub fn finish(mut self, fallback: &str) -> String {
        if !self.terminated {
            self.term(fallback);
        }
        format!(
            "{} {{\nentry:\n{}{}}}\n",
            self.header, self.allocas, self.body
        )
    }
}
