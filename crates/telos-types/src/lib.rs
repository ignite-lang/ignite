//! Type checker of the Telos slice (`docs/llvm.md` section 2). Runs after
//! `telos-check` on each standard-library module in import order and on the
//! program, and records a type for every expression and the resolved callee of
//! every call and method call. The LLVM generator reads nothing else about
//! types.
//!
//! Frozen skeleton: add, do not rename.
//!
//! Rules beyond `docs/llvm.md` section 2:
//!
//! * The module of a program without `package` is `"main"`. The builtin
//!   enums `Option`, `Result` and `Input` have the module `""`
//!   ([`QualName::builtin`]); [`Callee::Variant`] uses it for `Some`, `None`,
//!   `Ok`, `Err`, `Data` and `Eof`.
//! * `callee` is also recorded for `Name` and `Field` expressions that are
//!   not calls: a unit variant (`Help`, `Action.Close`) is
//!   [`Callee::Variant`], a constant is [`Callee::Const`] and a pure function
//!   used as a value is [`Callee::Fn`]. A local binding has no callee.
//! * The head of a path (`Command` in `Command.Echo(x)`, `Telnet` in
//!   `Telnet.serve(..)`, `TcpListener` in `TcpListener.bind(..)`) is not a
//!   value and has type `Unit`. The `Field` callee of a path call has the
//!   function type of what it calls, e.g. `fn(Text) -> Command`.
//! * In a `system`, the resource properties `address` (`Text`) and `port`
//!   (`Int`) and the component properties `application` and `executable`
//!   (`Text`) are typed; `application` is checked against the second
//!   parameter of the component protocol's `serve` role. Other properties
//!   (`listener`, `target`, `protocol`) name declarations and have no type.
//! * A `loop` without a `break` of its own has type `Never`, so a function
//!   body may end in one. A block containing a statement of type `Never` has
//!   type `Never`. `e?` on an `e` of type `Never` is `Never`; so is a
//!   `return` expression.
//! * `match` and `if` in statement position (an expression statement that is
//!   not the value of its block) do not unify their arms; their type is
//!   `Unit`, or `Never` when every arm is `Never`.
//! * Without an expected type, the arms of a `match` or `if` whose value is
//!   syntactically `None`, `[]`, `Ok(..)`, `Err(..)`, `List.empty()` or an
//!   ambiguous bare variant are typed after the others, against the type the
//!   others give.
//! * A bare variant takes the enum of the expected type when that enum
//!   declares it, whether or not the enum is in scope, else the one enum in
//!   scope declaring it, else E005. A bare name that only an enum out of
//!   scope declares, where the expected type does not decide it, is E003
//!   (`telos-check` leaves it to the type checker).
//! * `Bytes.of(list)` takes `Bytes`, so a list literal argument is checked
//!   element by element against `Byte` (E044). `x.copy()` is defined for
//!   every type; `join` only for `List<Text>`; `to_text` for `Int`, `Byte`,
//!   `Bool`, `Text` and `Bytes`. `+` on two `Text` values concatenates.
//! * A method call that is no builtin is a call of a fn-typed record field
//!   ([`Callee::FieldFn`]) or a UFCS call of a module-level fn whose first
//!   parameter accepts the receiver ([`Callee::Fn`]); a builtin method name
//!   is never either, and both matching is E041 (`docs/slice.md` 4.8).
//! * Aliases are resolved where they are used, so no [`Ty`] names one;
//!   [`TypedProgram::resolved_type`] gives every type expression's type. An
//!   alias that refers to itself is E048. A field default is typed against
//!   the field like a constant value, and a record literal may omit it.
//! * `error.message` on an `IoError` is `Text`. Indexing `l[i]` needs a
//!   `List` and an `Int`.
//! * A record literal that omits a field, names one the record lacks or
//!   names one twice is E047; an unknown type name and a wrong number of
//!   type arguments (`List<A, B>`) are E048. `telos-check` reports both
//!   first; the type checker reports them again for a program it is given
//!   unchecked. A type with no values (`Stdin`) is E040; a payload-pattern
//!   count mismatch is E043.
//! * Every `match` is checked for exhaustiveness (E046) against the type of
//!   its scrutinee, nested payload, tuple and or-patterns included; the
//!   message lists the uncovered values as patterns (`Command.Quit`,
//!   `Some(Command.Echo(_))`, `(false, None)`).
//!   `Bool` is split into `true` and `false`; any other type without
//!   variants is covered only by a `_` or binding arm (`()` for `Unit`).
//!   A constant pattern must have the scrutinee's type, one with `==`
//!   (E040, E045); [`TypedProgram::pattern_const`] records it.

mod builtins;
mod exhaustive;
mod typer;

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use telos_check::Symbols;
use telos_syntax::ast::{Expr, Pattern, Program, Span, Type};
use telos_syntax::diagnostic::Diagnostic;

/// A name qualified by the module that defines it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QualName {
    /// Module path, e.g. `"std.console"` or `"examples.terminal"`.
    pub module: String,
    pub name: String,
}

impl QualName {
    pub fn new(module: impl Into<String>, name: impl Into<String>) -> QualName {
        QualName {
            module: module.into(),
            name: name.into(),
        }
    }

    /// A builtin enum (`Option`, `Result`, `Input`): module `""`.
    pub fn builtin(name: impl Into<String>) -> QualName {
        QualName::new("", name)
    }

    pub fn is_builtin(&self) -> bool {
        self.module.is_empty()
    }

    /// The placeholder for parameter `name` inside an [`Alias`] target.
    pub fn alias_parameter(name: impl Into<String>) -> QualName {
        QualName::new(ALIAS_PARAMETER, name)
    }

    /// The parameter name of a [`QualName::alias_parameter`].
    pub fn alias_parameter_name(&self) -> Option<&str> {
        (self.module == ALIAS_PARAMETER).then_some(self.name.as_str())
    }
}

/// Module of [`QualName::alias_parameter`]; no real module has this path.
const ALIAS_PARAMETER: &str = "<alias parameter>";

impl fmt::Display for QualName {
    /// `module.name`, or `name` for a builtin.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.module.is_empty() {
            write!(f, "{}", self.name)
        } else {
            write!(f, "{}.{}", self.module, self.name)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    Int,
    Bool,
    Byte,
    Unit,
    Text,
    /// `Bytes` is `List(Byte)`.
    List(Box<Ty>),
    Option(Box<Ty>),
    Result(Box<Ty>, Box<Ty>),
    Record(QualName),
    Enum(QualName),
    Fn(Vec<Ty>, Box<Ty>),
    /// `(A, B)`: two or more elements; `()` is `Unit`.
    Tuple(Vec<Ty>),
    TcpListener,
    TcpStream,
    IoError,
    Input,
    /// The type of `return`, `break`, `continue` and `Process.exit`.
    Never,
    /// A type parameter, inside the generic declaration that declares it.
    Param(String),
    /// A record that declares type parameters, with its type arguments
    /// (`Param`s inside its own declaration). A record without type
    /// parameters is always [`Ty::Record`].
    GenericRecord(QualName, Vec<Ty>),
    /// An enum that declares type parameters, with its type arguments.
    GenericEnum(QualName, Vec<Ty>),
    /// `Store<T>`.
    Store(Box<Ty>),
    /// An effectful fn type `fn(n: A) -> R requires [n.*]`; lowered like
    /// [`Ty::Fn`].
    EffectFn(Vec<Ty>, Box<Ty>, FnEffects),
}

/// The parameter names and `requires` entries of an effectful fn type.
/// Equality and hashing compare the entries by parameter position, so
/// `fn(a: Int) requires [a.*]` equals `fn(b: Int) requires [b.*]`.
#[derive(Clone, Debug)]
pub struct FnEffects {
    pub names: Vec<String>,
    pub requires: Vec<String>,
}

impl FnEffects {
    /// The entries with a parameter head replaced by `#i`, sorted.
    fn normalised(&self) -> Vec<String> {
        let mut entries: Vec<String> = self
            .requires
            .iter()
            .map(|entry| match entry.split_once('.') {
                Some((head, rest)) => match self.names.iter().position(|n| n == head) {
                    Some(i) => format!("#{i}.{rest}"),
                    None => entry.clone(),
                },
                None => entry.clone(),
            })
            .collect();
        entries.sort();
        entries.dedup();
        entries
    }

    /// Whether every effect of `self` is covered by `other`: an entry on
    /// the `i`-th parameter by `other`'s entry on its `i`-th parameter
    /// (same operation or `*`), any other entry by itself or `head.*`.
    pub fn covered_by(&self, other: &FnEffects) -> bool {
        self.uncovered(other).is_empty()
    }

    /// The entries of `self` that `other` does not cover, as written.
    pub fn uncovered(&self, other: &FnEffects) -> Vec<String> {
        let mapped: Vec<String> = other
            .requires
            .iter()
            .map(|entry| match entry.split_once('.') {
                Some((head, rest)) => match other.names.iter().position(|n| n == head) {
                    Some(i) => format!("#{i}.{rest}"),
                    None => entry.clone(),
                },
                None => entry.clone(),
            })
            .collect();
        self.requires
            .iter()
            .filter(|entry| {
                let normal = match entry.split_once('.') {
                    Some((head, rest)) => match self.names.iter().position(|n| n == head) {
                        Some(i) => format!("#{i}.{rest}"),
                        None => (*entry).clone(),
                    },
                    None => (*entry).clone(),
                };
                !telos_check::covers(&mapped, &normal)
            })
            .cloned()
            .collect()
    }
}

impl PartialEq for FnEffects {
    fn eq(&self, other: &FnEffects) -> bool {
        self.normalised() == other.normalised()
    }
}

impl Eq for FnEffects {}

impl std::hash::Hash for FnEffects {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.normalised().hash(state);
    }
}

impl Ty {
    /// `self` with every `Param(params[i])` replaced by `args[i]`.
    pub fn substitute(&self, params: &[String], args: &[Ty]) -> Ty {
        self.map_params(&|name| {
            params
                .iter()
                .position(|p| p == name)
                .and_then(|i| args.get(i).cloned())
        })
    }

    /// `self` with each `Param(p)` for which `f(p)` is `Some` replaced.
    pub(crate) fn map_params(&self, f: &dyn Fn(&str) -> Option<Ty>) -> Ty {
        let map = |t: &Ty| t.map_params(f);
        let boxed = |t: &Ty| Box::new(t.map_params(f));
        match self {
            Ty::Param(name) => f(name).unwrap_or_else(|| self.clone()),
            Ty::List(t) => Ty::List(boxed(t)),
            Ty::Option(t) => Ty::Option(boxed(t)),
            Ty::Store(t) => Ty::Store(boxed(t)),
            Ty::Result(ok, err) => Ty::Result(boxed(ok), boxed(err)),
            Ty::Fn(ps, ret) => Ty::Fn(ps.iter().map(map).collect(), boxed(ret)),
            Ty::EffectFn(ps, ret, effects) => {
                Ty::EffectFn(ps.iter().map(map).collect(), boxed(ret), effects.clone())
            }
            Ty::Tuple(items) => Ty::Tuple(items.iter().map(map).collect()),
            Ty::GenericRecord(q, args) => {
                Ty::GenericRecord(q.clone(), args.iter().map(map).collect())
            }
            Ty::GenericEnum(q, args) => Ty::GenericEnum(q.clone(), args.iter().map(map).collect()),
            _ => self.clone(),
        }
    }

    /// Whether `self` mentions a `Param` for which `f` holds.
    pub(crate) fn has_param(&self, f: &dyn Fn(&str) -> bool) -> bool {
        let any = |ts: &[Ty]| ts.iter().any(|t| t.has_param(f));
        match self {
            Ty::Param(name) => f(name),
            Ty::List(t) | Ty::Option(t) | Ty::Store(t) => t.has_param(f),
            Ty::Result(ok, err) => ok.has_param(f) || err.has_param(f),
            Ty::Fn(ps, ret) | Ty::EffectFn(ps, ret, _) => any(ps) || ret.has_param(f),
            Ty::Tuple(items) | Ty::GenericRecord(_, items) | Ty::GenericEnum(_, items) => {
                any(items)
            }
            _ => false,
        }
    }
}

impl fmt::Display for Ty {
    /// Telos syntax: `Bytes` for `List<Byte>`, records and enums by their
    /// unqualified name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Int => write!(f, "Int"),
            Ty::Bool => write!(f, "Bool"),
            Ty::Byte => write!(f, "Byte"),
            Ty::Unit => write!(f, "Unit"),
            Ty::Text => write!(f, "Text"),
            Ty::List(element) if **element == Ty::Byte => write!(f, "Bytes"),
            Ty::List(element) => write!(f, "List<{element}>"),
            Ty::Option(some) => write!(f, "Option<{some}>"),
            Ty::Result(ok, err) => write!(f, "Result<{ok}, {err}>"),
            Ty::Record(name) | Ty::Enum(name) => write!(f, "{}", name.name),
            Ty::Fn(params, ret) => {
                let params = params
                    .iter()
                    .map(Ty::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "fn({params}) -> {ret}")
            }
            Ty::TcpListener => write!(f, "TcpListener"),
            Ty::TcpStream => write!(f, "TcpStream"),
            Ty::IoError => write!(f, "IoError"),
            Ty::Input => write!(f, "Input"),
            Ty::Tuple(elements) => {
                let elements = elements
                    .iter()
                    .map(Ty::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "({elements})")
            }
            Ty::Never => write!(f, "Never"),
            Ty::Param(name) => write!(f, "{name}"),
            Ty::GenericRecord(name, args) | Ty::GenericEnum(name, args) => {
                let args = args
                    .iter()
                    .map(Ty::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{}<{args}>", name.name)
            }
            Ty::Store(item) => write!(f, "Store<{item}>"),
            Ty::EffectFn(params, ret, effects) => {
                let params = params
                    .iter()
                    .enumerate()
                    .map(|(i, t)| match effects.names.get(i) {
                        Some(name) => format!("{name}: {t}"),
                        None => t.to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let requires = effects.requires.join(", ");
                write!(f, "fn({params}) -> {ret} requires [{requires}]")
            }
        }
    }
}

/// One row of the builtin table (`docs/slice.md` sections 6 and 7), resolved
/// by receiver type. Element types come from `TypedProgram::type_of` of the
/// receiver or the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Builtin {
    TextLen,
    TextIsEmpty,
    TextTrim,
    TextStartsWith,
    TextStripPrefix,
    TextConcat,
    TextToBytes,
    BytesToText,
    ListLen,
    ListIsEmpty,
    ListAt,
    ListSlice,
    ListPush,
    ListAppend,
    ListClear,
    ListJoin,
    Copy,
    BytesEmpty,
    BytesOf,
    ListEmpty,
    IntToText,
    Log,
    TcpListenerBind,
    TcpListenerAccept,
    TcpListenerLocalPort,
    TcpStreamConnect,
    TcpStreamRead,
    TcpStreamWriteAll,
    TcpStreamClose,
    StdinRead,
    StdinIsTerminal,
    StdinSetRaw,
    StdoutWriteAll,
    ProcessExit,
    IoErrorNew,
    /// `i.to_text()` on an `Int`: decimal (the static `Int.to_text(i)` is `IntToText`).
    IntToTextMethod,
    /// `b.to_text()` on a `Byte`: decimal, `0` to `255`.
    ByteToText,
    /// `b.to_text()` on a `Bool`: `true` or `false`.
    BoolToText,
    /// `t.to_text()` on a `Text`: a copy of `t`.
    TextToText,
    /// `l.map(f)`: `f: fn(T) -> U` pure, result `List<U>`.
    ListMap,
    /// `Int.parse(t) -> Option<Int>`: decimal, optional leading `-`.
    IntParse,
    /// `b.to_int() -> Int`.
    ByteToInt,
    /// `i.to_byte() -> Option<Byte>`, `None` outside 0 to 255.
    IntToByte,
    /// `s.set_idle_timeout(ms) -> Result<Unit, IoError>`.
    TcpStreamSetIdleTimeout,
    /// `s.next_id() -> Int`.
    StoreNextId,
    /// `s.get(id) -> Option<T>`.
    StoreGet,
    /// `s.put(id, value) -> Unit`.
    StorePut,
    /// `s.remove(id) -> Option<T>`.
    StoreRemove,
    /// `s.values() -> List<T>`.
    StoreValues,
    /// `s.update(id, f: fn(T) -> T) -> Option<T>`.
    StoreUpdate,
    /// `Store.new() -> Store<T>`, `T` from the expected type; tests and
    /// test-only items only (`docs/slice.md` section 12.3).
    StoreNew,
    /// `x.debug() -> Text` on a value of any type but a type parameter:
    /// the debug text of `docs/slice.md` section 13.6.
    Debug,
}

/// What a `Call` or `Method` expression calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Callee {
    Fn(QualName),
    Role {
        protocol: QualName,
        role: String,
    },
    Variant {
        enum_: QualName,
        index: usize,
    },
    Builtin(Builtin),
    /// A local, parameter or pattern binding of fn type.
    Local,
    Const(QualName),
    /// `receiver.field(args)` where `field` is a record field of fn type:
    /// the field's value is called with `args` (the receiver is not an argument).
    FieldFn {
        record: QualName,
        field: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnSig {
    pub params: Vec<Ty>,
    pub ret: Ty,
    pub effectful: bool,
}

/// The signatures a checked module exports.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ModuleTypes {
    pub records: BTreeMap<String, Vec<(String, Ty)>>,
    /// Variants in declaration order with their payload types.
    pub enums: BTreeMap<String, Vec<(String, Vec<Ty>)>>,
    pub fns: BTreeMap<String, FnSig>,
    pub consts: BTreeMap<String, Ty>,
    pub protocols: BTreeMap<String, BTreeMap<String, FnSig>>,
    /// Exported type aliases: the alias name to its target. Parameters of
    /// the alias appear in the target as [`Ty::Record`] of
    /// [`QualName::alias_parameter`]; such a type never leaves the typer.
    pub aliases: BTreeMap<String, Alias>,
    /// Record name to the names of its fields that have a default value.
    pub defaults: BTreeMap<String, Vec<String>>,
    /// Type parameters of generic records, enums and fns (key: the name)
    /// and generic protocol roles (key: `Protocol.role`).
    pub generics: BTreeMap<String, Vec<String>>,
    /// Values of the `Int` and `Byte` constants, for range patterns in
    /// importing modules (`docs/slice.md` section 13).
    pub const_values: BTreeMap<String, i64>,
}

/// `type Name<P, Q> = Target`, resolved in the module that declares it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alias {
    pub params: Vec<String>,
    pub target: Ty,
}

/// Typed standard-library modules by module path.
pub type TypedModules = BTreeMap<String, ModuleTypes>;

/// One `test "name" { .. }` of a module (`docs/slice.md` section 12).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestInfo {
    /// The module that declares the test, e.g. `"std.text"`.
    pub module: String,
    /// The test's name as written, escapes decoded.
    pub name: String,
    /// `"<module>/<name>"`: what the runner prints, filters on and passes
    /// to `--run-one`.
    pub full_name: String,
    /// Index of the `Decl::Test` in `Program::decls`.
    pub decl: usize,
}

/// How a failed `assert` or `?` shows a value (`docs/slice.md` section
/// 12). A value of a type with no way to show it is printed as `<T>`, `T`
/// being the type's [`Ty`] display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Show {
    /// A builtin `to_text`: `IntToTextMethod`, `ByteToText`, `BoolToText`,
    /// `TextToText` or `BytesToText`.
    Builtin(Builtin),
    /// A pure module-level fn `to_text(value: T) -> Text`, own or imported.
    Fn(QualName),
    /// An `IoError`: its message.
    IoErrorMessage,
    /// Any other type: its debug text (`docs/slice.md` section 13.6), as
    /// `x.debug()` gives it.
    Debug,
}

/// The equality glue `==` and `!=` need for a type (`docs/slice.md`
/// section 13.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Equality {
    /// `Int`, `Byte`, `Bool`, `Unit`: compare the scalar.
    Scalar,
    /// `Text`: compare the bytes.
    Text,
    /// A list, `Option`, `Result`, `Input`, tuple, record or enum whose
    /// parts are all comparable: element-wise, field-wise, or same variant
    /// and payload-wise; a list compares lengths first.
    Structural,
}

/// One binding a closure captures (`docs/slice.md` section 13.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    pub name: String,
    /// The span of the captured binding's name where it is declared: the
    /// key of [`TypedProgram::binding_type`].
    pub binding: Span,
    pub ty: Ty,
    /// The closure's effects on this capture, e.g. `conn.write`; empty when
    /// the closure performs none on it.
    pub effects: Vec<String>,
}

/// What a closure literal is (`docs/slice.md` section 13.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosureInfo {
    /// Parameter names and types.
    pub params: Vec<(String, Ty)>,
    pub ret: Ty,
    /// The bindings of enclosing scopes the body uses, in order of first
    /// use. Module-level fns and constants are not captures.
    pub captures: Vec<Capture>,
    /// `Some` for an effectful closure: its type is then
    /// [`Ty::EffectFn`] with these effects.
    pub effects: Option<FnEffects>,
}

/// `assert left == right` or `assert left != right` (`docs/slice.md`
/// section 12): both operands have type `operand`, which may be any type
/// with structural equality, not only one with the `==` operator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssertCompare {
    pub operand: Ty,
    /// How `left:` and `right:` are shown; `None` prints `<operand>`.
    pub show: Option<Show>,
}

/// A `?` directly inside a test body (`docs/slice.md` section 12): `Err`
/// or `None` fails the test instead of returning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestTry {
    /// The error type for a `Result`; `None` for an `Option`.
    pub error: Option<Ty>,
    /// How the error is shown; `None` prints `<error>` (or nothing for an
    /// `Option`).
    pub show: Option<Show>,
}

/// The result of typing one module. Expression keys are addresses: the
/// `Program` passed to [`type_module`] must be the same object (not a clone)
/// that later stages look expressions up in.
#[derive(Clone, Debug, Default)]
pub struct TypedProgram {
    pub module: String,
    types: HashMap<usize, Ty>,
    callees: HashMap<usize, Callee>,
    bindings: HashMap<Span, Ty>,
    /// Constant patterns, keyed by pattern address.
    pattern_consts: HashMap<usize, QualName>,
    /// Resolved type expressions, keyed by `Type` address.
    type_exprs: HashMap<usize, Ty>,
    exports: ModuleTypes,
    /// The modules this one was typed against, plus itself.
    modules: TypedModules,
    /// Type arguments of generic uses, keyed by expression address.
    instances: HashMap<usize, Vec<Ty>>,
    /// The module's tests in declaration order.
    tests: Vec<TestInfo>,
    /// `==`/`!=` assert conditions, keyed by the condition's address.
    assert_compares: HashMap<usize, AssertCompare>,
    /// `?` inside test bodies, keyed by the `Try` expression's address.
    test_tries: HashMap<usize, TestTry>,
    /// Closure literals, keyed by expression address.
    closures: HashMap<usize, ClosureInfo>,
    /// Bounds of range patterns, keyed by pattern address.
    ranges: HashMap<usize, (i64, i64)>,
}

fn key(expr: &Expr) -> usize {
    expr as *const Expr as usize
}

impl TypedProgram {
    /// The type of `expr`. Panics for an expression the checker never saw.
    pub fn type_of(&self, expr: &Expr) -> &Ty {
        self.types
            .get(&key(expr))
            .expect("expression was typed by type_module")
    }

    /// The callee of a `Call` or `Method` expression.
    pub fn callee(&self, expr: &Expr) -> Option<&Callee> {
        self.callees.get(&key(expr))
    }

    /// The constant a `PatternKind::Variant` pattern names, when it names a
    /// constant rather than a variant (`docs/slice.md` section 3).
    pub fn pattern_const(&self, pattern: &Pattern) -> Option<&QualName> {
        self.pattern_consts
            .get(&(pattern as *const Pattern as usize))
    }

    /// The type of a `let`/`var`/`for`/pattern binding, by the span of its name.
    pub fn binding_type(&self, span: Span) -> Option<&Ty> {
        self.bindings.get(&span)
    }

    pub fn exports(&self) -> ModuleTypes {
        self.exports.clone()
    }

    /// The type a type expression of this module denotes, with aliases
    /// resolved (`docs/slice.md` section 10.1). `None` for a type
    /// expression the checker never saw.
    pub fn resolved_type(&self, ty: &Type) -> Option<&Ty> {
        self.type_exprs.get(&(ty as *const Type as usize))
    }

    /// Whether field `field` of record `record` (of this module or a module
    /// it was typed against) has a default value.
    pub fn field_has_default(&self, record: &QualName, field: &str) -> bool {
        self.modules
            .get(&record.module)
            .and_then(|m| m.defaults.get(&record.name))
            .is_some_and(|fields| fields.iter().any(|f| f == field))
    }

    /// The type of `expr`, or `None` for an expression the checker never saw
    /// (a `system` property that names a declaration).
    pub fn try_type_of(&self, expr: &Expr) -> Option<&Ty> {
        self.types.get(&key(expr))
    }

    /// The fields of a record of this module or of a module it was typed
    /// against, in declaration order.
    pub fn record_fields(&self, record: &QualName) -> Option<&[(String, Ty)]> {
        self.modules
            .get(&record.module)?
            .records
            .get(&record.name)
            .map(Vec::as_slice)
    }

    /// The variants of a user enum of this module or of a module it was
    /// typed against, in declaration order. `None` for the builtin enums;
    /// use [`TypedProgram::variant`] for those.
    pub fn enum_variants(&self, enum_: &QualName) -> Option<&[(String, Vec<Ty>)]> {
        self.modules
            .get(&enum_.module)?
            .enums
            .get(&enum_.name)
            .map(Vec::as_slice)
    }

    /// The type arguments, in the order of the declaration's type
    /// parameters, of a generic use (`docs/slice.md` section 11.7): a call
    /// or method call of a generic fn or role, a generic fn used as a
    /// value, a variant of a generic enum, a record literal of a generic
    /// record, and a component's `protocol` property value.
    pub fn instance(&self, expr: &Expr) -> Option<&[Ty]> {
        self.instances.get(&key(expr)).map(Vec::as_slice)
    }

    /// The type parameters of a generic record, enum or fn of this module
    /// or a module it was typed against; `None` when it declares none.
    pub fn type_params(&self, name: &QualName) -> Option<&[String]> {
        self.modules
            .get(&name.module)?
            .generics
            .get(&name.name)
            .map(Vec::as_slice)
    }

    /// The type parameters of role `role` of protocol `protocol`; `None`
    /// when it declares none.
    pub fn role_type_params(&self, protocol: &QualName, role: &str) -> Option<&[String]> {
        self.modules
            .get(&protocol.module)?
            .generics
            .get(&format!("{}.{role}", protocol.name))
            .map(Vec::as_slice)
    }

    /// The tests this module declares, in declaration order.
    pub fn tests(&self) -> &[TestInfo] {
        &self.tests
    }

    /// For the condition of `assert a == b` or `assert a != b`: the operand
    /// type and how to show it. `None` for any other condition.
    pub fn assert_compare(&self, cond: &Expr) -> Option<&AssertCompare> {
        self.assert_compares.get(&key(cond))
    }

    /// For a `?` (`ExprKind::Try`) directly inside a test body: what fails
    /// the test and how its error is shown. `None` for a `?` in a fn.
    pub fn test_try(&self, expr: &Expr) -> Option<&TestTry> {
        self.test_tries.get(&key(expr))
    }

    /// The type, captures and effects of a closure literal
    /// (`ExprKind::Closure`); `None` for any other expression.
    pub fn closure(&self, expr: &Expr) -> Option<&ClosureInfo> {
        self.closures.get(&key(expr))
    }

    /// Both inclusive bounds of a range pattern (`PatternKind::Range`),
    /// constants evaluated; `None` for any other pattern.
    pub fn range_bounds(&self, pattern: &Pattern) -> Option<(i64, i64)> {
        self.ranges
            .get(&(pattern as *const Pattern as usize))
            .copied()
    }

    /// The equality glue `==` on values of type `ty` needs, or `None` when
    /// `ty` is not comparable (`docs/slice.md` section 13.5).
    pub fn equality(&self, ty: &Ty) -> Option<Equality> {
        equality(ty, &self.modules)
    }

    /// Whether payload `index` of variant `variant` (both counted from 0)
    /// of enum `enum_` is stored behind a pointer (`docs/slice.md` section
    /// 13.4). For a generic enum the answer holds for every instance.
    pub fn boxed_payload(&self, enum_: &QualName, variant: usize, index: usize) -> bool {
        boxed_payload(enum_, variant, index, &self.modules)
    }

    /// Whether field `field` of record `record` is stored behind a pointer
    /// (`docs/slice.md` section 13.4).
    pub fn boxed_field(&self, record: &QualName, field: &str) -> bool {
        boxed_field(record, field, &self.modules)
    }

    /// Index and payload types of variant `name` of a value of type `ty`
    /// (an enum, `Option`, `Result` or `Input`), as a pattern on it resolves.
    pub fn variant(&self, ty: &Ty, name: &str) -> Option<(usize, Vec<Ty>)> {
        let (_, variants) = variants_of(ty, &self.modules)?;
        variants
            .into_iter()
            .enumerate()
            .find(|(_, (n, _))| n == name)
            .map(|(index, (_, payload))| (index, payload))
    }
}

/// Variants in declaration order with their payload types.
pub(crate) type Variants = Vec<(String, Vec<Ty>)>;

/// See [`TypedProgram::equality`]. A record or enum is comparable when
/// every field or payload is; a recursive one is assumed comparable where
/// it recurs.
pub(crate) fn equality(ty: &Ty, modules: &TypedModules) -> Option<Equality> {
    fn parts_ok(ty: &Ty, modules: &TypedModules, seen: &mut Vec<Ty>) -> bool {
        match ty {
            Ty::Int | Ty::Byte | Ty::Bool | Ty::Unit | Ty::Text | Ty::Input => true,
            Ty::List(t) | Ty::Option(t) => parts_ok(t, modules, seen),
            Ty::Result(ok, err) => parts_ok(ok, modules, seen) && parts_ok(err, modules, seen),
            Ty::Tuple(items) => items.iter().all(|t| parts_ok(t, modules, seen)),
            Ty::Record(_) | Ty::GenericRecord(..) | Ty::Enum(_) | Ty::GenericEnum(..) => {
                if seen.contains(ty) {
                    return true;
                }
                seen.push(ty.clone());
                let parts: Vec<Ty> = match ty {
                    Ty::Enum(_) | Ty::GenericEnum(..) => match variants_of(ty, modules) {
                        Some((_, variants)) => variants.into_iter().flat_map(|(_, p)| p).collect(),
                        None => return false,
                    },
                    _ => match record_fields_of(ty, modules) {
                        Some(fields) => fields.into_iter().map(|(_, t)| t).collect(),
                        None => return false,
                    },
                };
                parts.iter().all(|t| parts_ok(t, modules, seen))
            }
            Ty::Fn(..)
            | Ty::EffectFn(..)
            | Ty::TcpListener
            | Ty::TcpStream
            | Ty::IoError
            | Ty::Store(_)
            | Ty::Param(_)
            | Ty::Never => false,
        }
    }
    match ty {
        Ty::Int | Ty::Byte | Ty::Bool | Ty::Unit => Some(Equality::Scalar),
        Ty::Text => Some(Equality::Text),
        _ if parts_ok(ty, modules, &mut Vec::new()) => Some(Equality::Structural),
        _ => None,
    }
}

/// The fields of a record type, type arguments substituted.
pub(crate) fn record_fields_of(ty: &Ty, modules: &TypedModules) -> Option<Vec<(String, Ty)>> {
    let (name, args) = match ty {
        Ty::Record(name) => (name, &[][..]),
        Ty::GenericRecord(name, args) => (name, args.as_slice()),
        _ => return None,
    };
    let module = modules.get(&name.module)?;
    let fields = module.records.get(&name.name)?;
    let params = module.generics.get(&name.name).cloned().unwrap_or_default();
    Some(
        fields
            .iter()
            .map(|(f, t)| (f.clone(), t.substitute(&params, args)))
            .collect(),
    )
}

/// Whether a value of type `ty` holds a value of the record or enum
/// `owner` inline: through records, tuples, `Option`, `Result` and enums,
/// with type arguments substituted, but not through a list, a `Store` or a
/// fn (`docs/slice.md` section 13.4).
fn reaches(ty: &Ty, owner: &QualName, modules: &TypedModules, seen: &mut Vec<Ty>) -> bool {
    match ty {
        Ty::Option(t) => reaches(t, owner, modules, seen),
        Ty::Result(ok, err) => {
            reaches(ok, owner, modules, seen) || reaches(err, owner, modules, seen)
        }
        Ty::Tuple(items) => items.iter().any(|t| reaches(t, owner, modules, seen)),
        Ty::Record(name) | Ty::GenericRecord(name, _) | Ty::Enum(name) | Ty::GenericEnum(name, _) => {
            if name == owner {
                return true;
            }
            if seen.contains(ty) || seen.len() > 64 {
                return false;
            }
            seen.push(ty.clone());
            let parts: Vec<Ty> = match ty {
                Ty::Enum(_) | Ty::GenericEnum(..) => variants_of(ty, modules)
                    .map(|(_, v)| v.into_iter().flat_map(|(_, p)| p).collect())
                    .unwrap_or_default(),
                _ => record_fields_of(ty, modules)
                    .map(|f| f.into_iter().map(|(_, t)| t).collect())
                    .unwrap_or_default(),
            };
            parts.iter().any(|t| reaches(t, owner, modules, seen))
        }
        _ => false,
    }
}

/// See [`TypedProgram::boxed_payload`].
pub(crate) fn boxed_payload(
    enum_: &QualName,
    variant: usize,
    index: usize,
    modules: &TypedModules,
) -> bool {
    let Some(ty) = modules
        .get(&enum_.module)
        .and_then(|m| m.enums.get(&enum_.name))
        .and_then(|vs| vs.get(variant))
        .and_then(|(_, payload)| payload.get(index))
    else {
        return false;
    };
    reaches(ty, enum_, modules, &mut Vec::new())
}

/// See [`TypedProgram::boxed_field`].
pub(crate) fn boxed_field(record: &QualName, field: &str, modules: &TypedModules) -> bool {
    let Some(ty) = modules
        .get(&record.module)
        .and_then(|m| m.records.get(&record.name))
        .and_then(|fs| fs.iter().find(|(f, _)| f == field))
        .map(|(_, t)| t)
    else {
        return false;
    };
    reaches(ty, record, modules, &mut Vec::new())
}

/// The enum name and the variants, with payload types instantiated, of a
/// type that has variants.
pub(crate) fn variants_of(ty: &Ty, modules: &TypedModules) -> Option<(QualName, Variants)> {
    let builtin = |name: &str, variants: Vec<(&str, Vec<Ty>)>| {
        Some((
            QualName::builtin(name),
            variants
                .into_iter()
                .map(|(v, p)| (v.to_string(), p))
                .collect(),
        ))
    };
    match ty {
        Ty::Option(some) => builtin(
            "Option",
            vec![("Some", vec![(**some).clone()]), ("None", vec![])],
        ),
        Ty::Result(ok, err) => builtin(
            "Result",
            vec![("Ok", vec![(**ok).clone()]), ("Err", vec![(**err).clone()])],
        ),
        Ty::Input => builtin(
            "Input",
            vec![("Data", vec![builtins::bytes()]), ("Eof", vec![])],
        ),
        Ty::Enum(name) => {
            let variants = modules.get(&name.module)?.enums.get(&name.name)?;
            Some((name.clone(), variants.clone()))
        }
        Ty::GenericEnum(name, args) => {
            let module = modules.get(&name.module)?;
            Some((name.clone(), instantiate(module, name, args)?))
        }
        _ => None,
    }
}

/// The variants of the generic enum `name` of `module` with `args`
/// substituted for its type parameters.
pub(crate) fn instantiate(module: &ModuleTypes, name: &QualName, args: &[Ty]) -> Option<Variants> {
    let variants = module.enums.get(&name.name)?;
    let params = module.generics.get(&name.name).cloned().unwrap_or_default();
    Some(
        variants
            .iter()
            .map(|(v, payload)| {
                let payload = payload
                    .iter()
                    .map(|t| t.substitute(&params, args))
                    .collect();
                (v.clone(), payload)
            })
            .collect(),
    )
}

/// Type one module (`docs/llvm.md` section 2). `symbols` is what
/// `telos_check::check_module` returned for it; `modules` holds the typed
/// standard-library modules it may import.
pub fn type_module(
    program: &Program,
    symbols: &Symbols,
    modules: &TypedModules,
) -> Result<TypedProgram, Vec<Diagnostic>> {
    let typed = typer::Typer::new(program, symbols, modules).run(program)?;
    let mut all = modules.clone();
    all.insert(typed.module.clone(), typed.exports.clone());
    Ok(TypedProgram {
        module: typed.module,
        types: typed.types,
        callees: typed.callees,
        bindings: typed.bindings,
        pattern_consts: typed.pattern_consts,
        type_exprs: typed.type_exprs,
        exports: typed.exports,
        modules: all,
        instances: typed.instances,
        tests: typed.tests,
        assert_compares: typed.assert_compares,
        test_tries: typed.test_tries,
        closures: typed.closures,
        ranges: typed.ranges,
    })
}
