//! Abstract syntax of the Ignite slice, one node per rule of `docs/slice.md`
//! section 3. Every node carries the byte span it was parsed from.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

/// The source file a span or node belongs to: [`FileId::of_path`] of the
/// path [`crate::parse`] was given. [`FileId::NONE`] for tokens lexed
/// without a file and spans made up by a later stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FileId(pub u64);

impl FileId {
    /// No file: [`crate::lexer::lex`] without a path, [`Span::new`].
    pub const NONE: FileId = FileId(0);
    /// Nodes built outside the parser ([`NodeId::fresh`]).
    pub const SYNTHETIC: FileId = FileId(u64::MAX);

    /// The id of the file at `path`. Distinct paths always get distinct
    /// ids within a process, and one path always the same one: the ids
    /// are numbered through a process-wide table of every path seen. A
    /// path's id is the 64-bit FNV-1a hash of its bytes, so it is the same
    /// in every run and can be cached and serialised; only when another
    /// path of the process already holds that value (a collision, which
    /// can be constructed) does the path take the next free value instead.
    /// Never [`FileId::NONE`] or [`FileId::SYNTHETIC`]. A driver passes a
    /// path that names one file one way
    /// ([`crate::SourceFile::with_identity`]).
    pub fn of_path(path: &str) -> FileId {
        let mut files = FILES
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(id) = files.by_path.get(path) {
            return *id;
        }
        let mut value = fnv1a(path);
        while value == FileId::NONE.0
            || value == FileId::SYNTHETIC.0
            || files.taken.contains(&value)
        {
            value = value.wrapping_add(1);
        }
        files.taken.insert(value);
        files.by_path.insert(path.to_string(), FileId(value));
        FileId(value)
    }
}

/// The paths [`FileId::of_path`] has numbered, and the values taken.
#[derive(Default)]
struct FileTable {
    by_path: HashMap<String, FileId>,
    taken: HashSet<u64>,
}

static FILES: OnceLock<Mutex<FileTable>> = OnceLock::new();

/// The 64-bit FNV-1a hash of `path`'s bytes.
fn fnv1a(path: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The identity of an [`Expr`], [`Pattern`], [`Type`], [`Path`] or
/// [`Place`]: the file it was parsed from and its number in that parse.
/// The parser numbers every such node of a file once; a clone keeps the
/// ids, so a typed program answers for a clone of the AST it typed. Not
/// part of a node's equality, which stays structural.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId {
    pub file: FileId,
    pub index: u32,
}

/// Counter of [`NodeId::fresh`].
static SYNTHETIC_NODES: AtomicU32 = AtomicU32::new(0);

impl NodeId {
    /// An id no parse and no [`NodeId::fresh`] gives: for a node that is
    /// never looked up (a constant pattern built in a `static`).
    pub const DETACHED: NodeId = NodeId {
        file: FileId::SYNTHETIC,
        index: u32::MAX,
    };

    /// A new id in [`FileId::SYNTHETIC`], distinct from every other id of
    /// the process, for a node built outside the parser.
    pub fn fresh() -> NodeId {
        let index = SYNTHETIC_NODES.fetch_add(1, Ordering::Relaxed);
        assert!(index != u32::MAX, "synthetic node ids exhausted");
        NodeId {
            file: FileId::SYNTHETIC,
            index,
        }
    }
}

impl std::hash::Hash for NodeId {
    /// One `u64`, so a hasher that mixes one word sees the whole id: the
    /// file xor the index times an odd constant, distinct for every index
    /// of one file.
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.file.0 ^ u64::from(self.index).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    }
}

/// Byte offsets into the source text, `start` inclusive, `end` exclusive,
/// and the file they are offsets into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub file: FileId,
}

impl Span {
    /// A span in no file ([`FileId::NONE`]).
    pub fn new(start: usize, end: usize) -> Span {
        Span {
            start,
            end,
            file: FileId::NONE,
        }
    }

    /// A span in `file`.
    pub fn in_file(start: usize, end: usize, file: FileId) -> Span {
        Span { start, end, file }
    }

    /// The smallest span covering both, in the file of `self`.
    pub fn to(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            file: self.file,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

impl Ident {
    /// `true` when the first character is an ASCII uppercase letter. The
    /// parser uses this to tell a record literal from a block and a variant
    /// pattern from a binding.
    pub fn is_uppercase(&self) -> bool {
        self.name
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_uppercase())
    }
}

/// `a.b.c`
#[derive(Clone, Debug, Eq)]
pub struct Path {
    pub segments: Vec<Ident>,
    pub span: Span,
    pub id: NodeId,
}

impl PartialEq for Path {
    fn eq(&self, other: &Path) -> bool {
        self.segments == other.segments && self.span == other.span
    }
}

impl Path {
    /// A path built outside the parser, with a [`NodeId::fresh`] id.
    pub fn new(segments: Vec<Ident>, span: Span) -> Path {
        Path {
            segments,
            span,
            id: NodeId::fresh(),
        }
    }

    pub fn last(&self) -> &Ident {
        self.segments
            .last()
            .expect("a path has at least one segment")
    }

    pub fn dotted(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Program {
    pub package: Option<Path>,
    pub imports: Vec<Import>,
    pub decls: Vec<Decl>,
}

/// `import a.b.c` has `names` empty; `import a.b.{X, Y}` lists `X` and `Y`.
/// A glob `import a.b.*` has `names` empty and a last path segment `*`
/// ([`Import::is_glob`]); like `import a.b.c`, its module is the path
/// without the last segment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub path: Path,
    pub names: Vec<Ident>,
    pub span: Span,
}

impl Import {
    /// `import a.b.*` (`docs/slice.md` section 13).
    pub fn is_glob(&self) -> bool {
        self.names.is_empty() && self.path.last().name == "*"
    }

    /// The imported module's dotted path: `a.b` for `import a.b.{X}`,
    /// `import a.b.*` and `import a.b.X`.
    pub fn module(&self) -> String {
        if self.names.is_empty() {
            let n = self.path.segments.len().saturating_sub(1);
            self.path.segments[..n]
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(".")
        } else {
            self.path.dotted()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decl {
    Record(RecordDecl),
    Enum(EnumDecl),
    Fn(FnDecl),
    System(SystemDecl),
    Protocol(ProtocolDecl),
    Const(ConstDecl),
    Alias(AliasDecl),
    /// `test "name" { .. }` (`docs/slice.md` section 12).
    Test(TestDecl),
    /// `assembly name: Kind { .. }` (`docs/slice.md` section 14.7).
    Assembly(AssemblyDecl),
}

/// `assembly name: Kind { property = value ... }`: a top-level
/// declaration whose kind is a record type (`Deployment`). `ignite build`
/// does not read it in slice 10 (`docs/slice.md` section 14.7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssemblyDecl {
    pub name: Ident,
    pub kind: Ident,
    pub properties: Vec<FieldInit>,
    pub span: Span,
}

impl Decl {
    /// The declared name. A test's name is its text, not an identifier,
    /// and takes no part in the module namespace.
    pub fn name(&self) -> &Ident {
        match self {
            Decl::Record(d) => &d.name,
            Decl::Enum(d) => &d.name,
            Decl::Fn(d) => &d.name,
            Decl::System(d) => &d.name,
            Decl::Protocol(d) => &d.name,
            Decl::Const(d) => &d.name,
            Decl::Alias(d) => &d.name,
            Decl::Test(d) => &d.name,
            Decl::Assembly(d) => &d.name,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Decl::Record(d) => d.span,
            Decl::Enum(d) => d.span,
            Decl::Fn(d) => d.span,
            Decl::System(d) => d.span,
            Decl::Protocol(d) => d.span,
            Decl::Const(d) => d.span,
            Decl::Alias(d) => d.span,
            Decl::Test(d) => d.span,
            Decl::Assembly(d) => d.span,
        }
    }

    /// Whether the declaration exists only for tests: a `test "name"`
    /// block or a `test fn`, `test type`, `test enum` or `test const`.
    /// Normal builds drop these (`docs/slice.md` section 12).
    pub fn is_test_only(&self) -> bool {
        match self {
            Decl::Test(_) => true,
            Decl::Fn(d) => d.test_only,
            Decl::Record(d) => d.test_only,
            Decl::Enum(d) => d.test_only,
            Decl::Const(d) => d.test_only,
            Decl::System(_) | Decl::Protocol(_) | Decl::Alias(_) | Decl::Assembly(_) => false,
        }
    }
}

/// `test "name" { body }`: a test, a top-level declaration next to the
/// code it tests (`docs/slice.md` section 12). `name.name` is the text of
/// the string literal (escapes decoded) and `name.span` the literal's span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestDecl {
    pub name: Ident,
    pub body: Block,
    pub span: Span,
    /// `test "name" verifies [Spec.Id, ..] { .. }`: the requirements the
    /// test is evidence for, each a [`Path`] of two segments
    /// (`docs/slice.md` 19.5); empty without the clause.
    pub verifies: Vec<Path>,
}

/// `const NAME: Type = value` (`docs/slice.md` section 4.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstDecl {
    pub name: Ident,
    pub ty: Type,
    pub value: Expr,
    pub span: Span,
    /// `test const`: visible only to tests and other test-only items.
    pub test_only: bool,
}

/// `type Name<T, U> = Type` (`docs/slice.md` section 4.6): a name for
/// another type; `params` are the alias's type parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AliasDecl {
    pub name: Ident,
    pub params: Vec<Ident>,
    pub ty: Type,
    pub span: Span,
}

/// `protocol Name { fn serve(...) ... fn connect(...) ... }`: the roles of a
/// protocol over one TCP stream (`docs/slice.md` section 4.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolDecl {
    pub name: Ident,
    pub fns: Vec<FnDecl>,
    pub span: Span,
    /// `protocol P conforms [RFC9112, ..]`: the names of the `Spec`
    /// constants the protocol claims (`docs/slice.md` 19.5); empty
    /// without the clause.
    pub conforms: Vec<Ident>,
    /// `scope { parties = [..], sections = [..] }` after `conforms`.
    pub scope: Option<Scope>,
    /// The `unsupported` and `not_applicable` members of the body, in
    /// source order.
    pub statuses: Vec<StatusDecl>,
}

/// `scope { parties = [Server, Any], sections = ["2", "3.1"] }`: what a
/// `conforms` claim covers (`docs/slice.md` 19.5). An absent item admits
/// everything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    pub parties: Option<Vec<Ident>>,
    /// Each section as written, with the span of its string literal.
    pub sections: Option<Vec<(String, Span)>>,
    pub span: Span,
}

/// `unsupported Spec.Id "reason"` or `not_applicable Spec.Id "reason"`, a
/// member of a protocol body (`docs/slice.md` 19.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusDecl {
    pub kind: StatusKind,
    /// A path of two segments: the `Spec` constant and the requirement.
    pub requirement: Path,
    /// The reason, a non-empty Text literal without interpolation.
    pub reason: String,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusKind {
    Unsupported,
    NotApplicable,
}

impl StatusKind {
    /// The clause word: `unsupported` or `not_applicable`.
    pub fn word(self) -> &'static str {
        match self {
            StatusKind::Unsupported => "unsupported",
            StatusKind::NotApplicable => "not_applicable",
        }
    }
}

impl ProtocolDecl {
    pub fn role(&self, name: &str) -> Option<&FnDecl> {
        self.fns.iter().find(|f| f.name.name == name)
    }
}

/// `type Name { field: Type ... }`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordDecl {
    pub name: Ident,
    pub fields: Vec<Field>,
    pub span: Span,
    /// `type Pair<A, B> { .. }`: the type parameters; empty when not generic.
    pub type_params: Vec<Ident>,
    /// `test type`: visible only to tests and other test-only items.
    pub test_only: bool,
    /// `type Name satisfies [Spec.Id, ..] { .. }` (`docs/slice.md` 19.5);
    /// empty without the clause.
    pub satisfies: Vec<Path>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: Ident,
    pub ty: Type,
    /// `field: Type = value`: used when a record literal omits the field.
    pub default: Option<Expr>,
    pub span: Span,
    /// `field: Type = value satisfies [Spec.Id, ..]` (`docs/slice.md`
    /// 19.5); empty without the clause.
    pub satisfies: Vec<Path>,
}

/// `enum Name { A, B(T, U) }`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumDecl {
    pub name: Ident,
    pub variants: Vec<Variant>,
    pub span: Span,
    /// `enum Tree<T> { .. }`: the type parameters; empty when not generic.
    pub type_params: Vec<Ident>,
    /// `test enum`: visible only to tests and other test-only items.
    pub test_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    pub name: Ident,
    pub payload: Vec<Type>,
    pub span: Span,
}

/// `fn name(params) -> Ret requires [effects] { body }`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    /// `Some` (possibly empty) when `requires [..]` is written; `None` when
    /// the effects are inferred (`docs/slice.md` section 5).
    pub requires: Option<Vec<Path>>,
    pub body: Block,
    pub span: Span,
    /// `fn swap<A, B>(..)`: the type parameters; empty when not generic.
    pub type_params: Vec<Ident>,
    /// `test fn`: visible only to tests and other test-only items.
    pub test_only: bool,
    /// `fn Response.json(..)`: an associated fn of the record or enum
    /// `Response` of the same module, called `Response.json(..)`
    /// (`docs/slice.md` section 13); `None` for a free fn.
    pub owner: Option<Ident>,
    /// `fn relay<S: ByteStream>(..)`: one entry per bounded type parameter,
    /// in the order of `type_params` (`docs/slice.md` 18.3).
    pub bounds: Vec<Bound>,
    /// `.. requires [..] satisfies [Spec.Id, ..] { .. }`: the requirements
    /// the fn or role implements (`docs/slice.md` 19.5); empty without the
    /// clause.
    pub satisfies: Vec<Path>,
}

/// `S: A + B` in the type parameters of a fn or role: the parameter and
/// the interfaces it names, as written. `ignite-check` accepts exactly one,
/// `ByteStream` (E099).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bound {
    pub param: Ident,
    pub interfaces: Vec<Ident>,
    /// From the parameter to the last interface.
    pub span: Span,
}

impl FnDecl {
    /// `Owner.name` for an associated fn, `name` otherwise: the key of the
    /// fn in [`ignite-types`]' tables and the name of its `QualName`.
    pub fn qualified_name(&self) -> String {
        match &self.owner {
            Some(owner) => format!("{}.{}", owner.name, self.name.name),
            None => self.name.name.clone(),
        }
    }

    /// Whether `requires` is written. A function without it may still be
    /// effectful by inference; `ignite-check` and `ignite-types` have the answer.
    pub fn is_effectful(&self) -> bool {
        self.requires.is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

/// `Name` or `Name<A, B>`, or the function type `fn(A, B) -> R`, which is
/// `Type { name: "fn", args: [A, B, R] }`: parameters first, return type last
/// (`Unit` when the source omits `-> R`).
#[derive(Clone, Debug, Eq)]
pub struct Type {
    pub name: Ident,
    pub args: Vec<Type>,
    pub span: Span,
    /// Parameter names of a fn type `fn(state: S, request: R) -> T`: empty,
    /// or one per parameter. Empty for every other type.
    pub fn_names: Vec<Ident>,
    /// `requires [..]` of an effectful fn type; `None` for a pure fn type
    /// and every other type.
    pub requires: Option<Vec<Path>>,
    pub id: NodeId,
}

impl PartialEq for Type {
    fn eq(&self, other: &Type) -> bool {
        self.name == other.name
            && self.args == other.args
            && self.span == other.span
            && self.fn_names == other.fn_names
            && self.requires == other.requires
    }
}

impl Type {
    /// A type `name<args>` without fn-type parts, with a
    /// [`NodeId::fresh`] id.
    pub fn named(name: Ident, args: Vec<Type>, span: Span) -> Type {
        Type {
            name,
            args,
            span,
            fn_names: Vec::new(),
            requires: None,
            id: NodeId::fresh(),
        }
    }

    pub fn is_fn(&self) -> bool {
        self.name.name == "fn"
    }

    /// A tuple type `(A, B)` is `Type { name: "Tuple", args: [A, B] }`;
    /// `()` is `Unit`, never an empty tuple.
    pub fn is_tuple(&self) -> bool {
        self.name.name == "Tuple"
    }

    /// Parameter types of a function type; empty for any other type.
    pub fn fn_params(&self) -> &[Type] {
        match self.args.split_last() {
            Some((_, params)) if self.is_fn() => params,
            _ => &[],
        }
    }

    /// Return type of a function type.
    pub fn fn_ret(&self) -> Option<&Type> {
        if self.is_fn() { self.args.last() } else { None }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StmtKind {
    /// `let x = e` (`mutable: false`) or `var x = e` (`mutable: true`).
    Let {
        name: Ident,
        ty: Option<Type>,
        value: Expr,
        mutable: bool,
    },
    /// `let (a, b) = e`, `let _ = e`, `_ = e`: an irrefutable pattern
    /// (binding, wildcard or tuple of those).
    LetPattern {
        pattern: Pattern,
        value: Expr,
        mutable: bool,
    },
    /// `place = e`
    Assign {
        target: Place,
        value: Expr,
    },
    Return(Option<Expr>),
    While {
        cond: Expr,
        body: Block,
    },
    Loop {
        body: Block,
    },
    For {
        name: Ident,
        iter: Expr,
        body: Block,
    },
    Break,
    Continue,
    /// An expression in statement position. When it is the last statement of
    /// a block, it is the block's value.
    Expr(Expr),
    /// `assert cond` or `assert cond, message` (`docs/slice.md` section 12):
    /// `cond` is `Bool`, `message` is `Text`. Allowed in every block.
    Assert {
        cond: Expr,
        message: Option<Expr>,
    },
    /// `for (k, v) in m { .. }`: a `for` whose binding is a tuple pattern
    /// (irrefutable: bindings, `_` and tuples of those). A `for` over a
    /// single name stays [`StmtKind::For`] (`docs/slice.md` section 14.3).
    ForPattern {
        pattern: Pattern,
        iter: Expr,
        body: Block,
    },
}

/// `x` or `x.a.b`
#[derive(Clone, Debug, Eq)]
pub struct Place {
    pub base: Ident,
    pub fields: Vec<Ident>,
    pub span: Span,
    pub id: NodeId,
}

impl PartialEq for Place {
    fn eq(&self, other: &Place) -> bool {
        self.base == other.base && self.fields == other.fields && self.span == other.span
    }
}

impl Place {
    /// A place built outside the parser, with a [`NodeId::fresh`] id.
    pub fn new(base: Ident, fields: Vec<Ident>, span: Span) -> Place {
        Place {
            base,
            fields,
            span,
            id: NodeId::fresh(),
        }
    }
}

#[derive(Clone, Debug, Eq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
    pub id: NodeId,
}

impl PartialEq for Expr {
    fn eq(&self, other: &Expr) -> bool {
        self.kind == other.kind && self.span == other.span
    }
}

impl Expr {
    /// An expression built outside the parser, with a [`NodeId::fresh`] id.
    pub fn new(kind: ExprKind, span: Span) -> Expr {
        Expr {
            kind,
            span,
            id: NodeId::fresh(),
        }
    }
}

/// Drops the operands of a binary or unary operator chain from a work
/// list instead of recursively, so that a 100,000-term `+` chain does not
/// overflow the stack (`docs/slice.md` section 13, row 14).
impl Drop for Expr {
    fn drop(&mut self) {
        if !matches!(self.kind, ExprKind::Binary { .. } | ExprKind::Unary { .. }) {
            return;
        }
        let mut pending = Vec::new();
        take_operands(&mut self.kind, &mut pending);
        while let Some(mut expr) = pending.pop() {
            take_operands(&mut expr.kind, &mut pending);
        }
    }
}

fn take_operands(kind: &mut ExprKind, pending: &mut Vec<Expr>) {
    if !matches!(kind, ExprKind::Binary { .. } | ExprKind::Unary { .. }) {
        return;
    }
    match std::mem::replace(kind, ExprKind::Break) {
        ExprKind::Binary { left, right, .. } => {
            pending.push(*left);
            pending.push(*right);
        }
        ExprKind::Unary { operand, .. } => pending.push(*operand),
        _ => {}
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprKind {
    Int(i64),
    Str(String),
    Bool(bool),
    Name(Ident),
    /// `Name { field = value, ... }`; `Name` starts uppercase.
    Record {
        name: Ident,
        fields: Vec<FieldInit>,
    },
    List(Vec<Expr>),
    /// `(a, b)`; at least two elements. `()` is `Tuple(vec![])`, the Unit value.
    Tuple(Vec<Expr>),
    /// `return e` in expression position (a match arm, an `if` branch); type Never.
    Return(Option<Box<Expr>>),
    /// `callee(args)`. `callee` is a `Name` for a function or builtin
    /// constructor, or a `Field` for a path call such as `Command.Echo(x)`
    /// and `TcpListener.bind(a, p)`.
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    /// `receiver.name(args)`
    Method {
        receiver: Box<Expr>,
        name: Ident,
        args: Vec<Expr>,
    },
    /// `receiver.name`; also how `Action.Continue` parses.
    Field {
        receiver: Box<Expr>,
        name: Ident,
    },
    /// `receiver[index]`
    Index {
        receiver: Box<Expr>,
        index: Box<Expr>,
    },
    /// `e?`
    Try(Box<Expr>),
    /// `Await(e)`: the deprecated marker, no longer produced by the parser
    /// (slice 6); kept so the public AST does not lose a variant.
    Await(Box<Expr>),
    /// `spawn f(args)`: runs an effectful call as a concurrent task.
    Spawn(Box<Expr>),
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<Arm>,
    },
    /// `if cond { then } else otherwise`; `otherwise` is a `Block` expression
    /// or another `If`, or absent.
    If {
        cond: Box<Expr>,
        then: Block,
        otherwise: Option<Box<Expr>>,
    },
    Block(Block),
    /// `|x, y| body` or `|x: Int| { .. }` (`docs/slice.md` section 13).
    Closure(Closure),
    /// `Name { field = value, ..base }`: a copy of `base` with the listed
    /// fields replaced. The fields are evaluated in source order, then
    /// `base`.
    RecordUpdate {
        name: Ident,
        fields: Vec<FieldInit>,
        base: Box<Expr>,
    },
    /// `receiver.0`: element `index` of a tuple.
    TupleField {
        receiver: Box<Expr>,
        index: usize,
    },
    /// `break` in expression position (a match arm, an `if` branch); type
    /// Never. Statement-position `break` stays [`StmtKind::Break`].
    Break,
    /// `continue` in expression position; type Never.
    Continue,
    /// `4096 bytes`, `15 minutes`, `99.9%` (`docs/slice.md` section 14.1):
    /// `literal` is the number as written without `_` separators (decimal
    /// digits, at most one `.`), `unit` the unit name (`%` for a ratio).
    /// `ignite_types::TypedProgram::quantity` has the kind and the value.
    Quantity {
        literal: String,
        unit: Ident,
    },
    /// `name = value` as an argument of a call or method call
    /// (`docs/slice.md` section 14.2). Only appears in `args`;
    /// `ignite_types::TypedProgram::named_args` maps the call's arguments to
    /// parameters. Its type is the type of `value`.
    NamedArg {
        name: Ident,
        value: Box<Expr>,
    },
    /// `{ k = e, .. }` without a type name (`docs/slice.md` section 14.3):
    /// a record or map literal by the expected type,
    /// `ignite_types::TypedProgram::brace_kind`.
    Brace {
        entries: Vec<BraceEntry>,
    },
}

/// The unit names of quantity literals (`docs/slice.md` section 14.1):
/// name, quantity type and how many base units one of it is (nanoseconds,
/// bytes, parts per million).
pub const QUANTITY_UNITS: &[(&str, &str, i64)] = &[
    ("ns", "Duration", 1),
    ("us", "Duration", 1_000),
    ("ms", "Duration", 1_000_000),
    ("second", "Duration", 1_000_000_000),
    ("seconds", "Duration", 1_000_000_000),
    ("minute", "Duration", 60_000_000_000),
    ("minutes", "Duration", 60_000_000_000),
    ("hour", "Duration", 3_600_000_000_000),
    ("hours", "Duration", 3_600_000_000_000),
    ("day", "Duration", 86_400_000_000_000),
    ("days", "Duration", 86_400_000_000_000),
    ("byte", "ByteSize", 1),
    ("bytes", "ByteSize", 1),
    ("KiB", "ByteSize", 1 << 10),
    ("MiB", "ByteSize", 1 << 20),
    ("GiB", "ByteSize", 1 << 30),
    ("KB", "ByteSize", 1_000),
    ("MB", "ByteSize", 1_000_000),
    ("GB", "ByteSize", 1_000_000_000),
    ("%", "Ratio", 10_000),
];

/// The row of [`QUANTITY_UNITS`] for a unit name.
pub fn quantity_unit(name: &str) -> Option<(&'static str, i64)> {
    QUANTITY_UNITS
        .iter()
        .find(|(unit, _, _)| *unit == name)
        .map(|(_, kind, factor)| (*kind, *factor))
}

/// One `key = value` of a brace literal. `key` is one name (a field or a
/// `Text` map key) or a qualified reference `System.member`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BraceEntry {
    pub key: Path,
    pub value: Expr,
    pub span: Span,
}

/// A closure literal. Its captures, effects and type are in
/// `ignite_types::TypedProgram::closure`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Closure {
    pub params: Vec<ClosureParam>,
    pub body: Box<Expr>,
}

/// One parameter of a closure, with an optional type `|x: Int|`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosureParam {
    pub name: Ident,
    pub ty: Option<Type>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldInit {
    pub name: Ident,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
    /// `~`, bitwise not of an `Int` (`docs/slice.md` 29).
    BitNot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    /// `&`, `|`, `^`, `<<`, `>>` on `Int` (`docs/slice.md` 29).
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

impl BinaryOp {
    /// `&`, `|`, `^`, `<<` and `>>`: both operands and the result are
    /// `Int`, and none fails (`docs/slice.md` 29).
    pub fn is_bitwise(self) -> bool {
        matches!(
            self,
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr
        )
    }

    pub fn symbol(self) -> &'static str {
        match self {
            BinaryOp::Or => "||",
            BinaryOp::And => "&&",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Gt => ">",
            BinaryOp::Le => "<=",
            BinaryOp::Ge => ">=",
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Rem => "%",
            BinaryOp::BitAnd => "&",
            BinaryOp::BitOr => "|",
            BinaryOp::BitXor => "^",
            BinaryOp::Shl => "<<",
            BinaryOp::Shr => ">>",
        }
    }
}

/// `pattern => body`; a block body is an `ExprKind::Block`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, Eq)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
    pub id: NodeId,
}

impl PartialEq for Pattern {
    fn eq(&self, other: &Pattern) -> bool {
        self.kind == other.kind && self.span == other.span
    }
}

impl Pattern {
    /// A pattern built outside the parser, with a [`NodeId::fresh`] id.
    pub fn new(kind: PatternKind, span: Span) -> Pattern {
        Pattern {
            kind,
            span,
            id: NodeId::fresh(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternKind {
    Wildcard,
    Int(i64),
    Str(String),
    Bool(bool),
    /// `p | q`
    Or(Vec<Pattern>),
    /// `(p, q)`
    Tuple(Vec<Pattern>),
    /// `[p, q]`: a list of exactly that length; with a [`PatternKind::Rest`]
    /// element, a list of at least the other elements' count
    /// (`docs/slice.md` 20.5).
    List(Vec<Pattern>),
    /// `..` or `..name`, only as an element of [`PatternKind::List`], at
    /// most one per list and `..name` only last (E076): the elements
    /// between the prefix and the suffix, bound as a `List<T>` or `Bytes`.
    Rest(Option<Ident>),
    /// A lowercase identifier with no payload binds the value.
    Binding(Ident),
    /// `Variant`, `Variant(p, q)`, or `Enum.Variant(...)`: an uppercase
    /// identifier, with an optional qualifying path and payload patterns.
    /// A path without payload that names a constant is a constant pattern;
    /// the type checker resolves it (`TypedProgram::pattern_const`).
    Variant {
        path: Vec<Ident>,
        args: Vec<Pattern>,
    },
    /// `lo..=hi` on `Int` or `Byte` (`docs/slice.md` section 13): both
    /// bounds inclusive, each an [`PatternKind::Int`] (a byte literal
    /// `b'0'` is one) or a constant ([`PatternKind::Variant`] without
    /// payload, resolved by `ignite_types::TypedProgram::pattern_const`).
    /// `ignite_types::TypedProgram::range_bounds` has both values.
    Range {
        start: Box<Pattern>,
        end: Box<Pattern>,
    },
}

/// `system Name { resources and components }`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemDecl {
    pub name: Ident,
    pub resources: Vec<ResourceDecl>,
    pub components: Vec<ComponentDecl>,
    pub span: Span,
    /// `contract name: Kind { .. }` (`docs/slice.md` section 14.7): like a
    /// component, never executable.
    pub contracts: Vec<ComponentDecl>,
    /// `policy name: Kind { .. }`: like a component, never executable.
    pub policies: Vec<ComponentDecl>,
}

/// `resource name: Kind { property = value ... }`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceDecl {
    pub name: Ident,
    pub kind: Ident,
    pub properties: Vec<FieldInit>,
    pub span: Span,
    /// Type arguments of the kind: `resource todos: Store<Todo>` has `[Todo]`.
    pub args: Vec<Type>,
}

/// `component name: Kind { property = value ... }`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentDecl {
    pub name: Ident,
    pub kind: Ident,
    pub properties: Vec<FieldInit>,
    pub span: Span,
}
