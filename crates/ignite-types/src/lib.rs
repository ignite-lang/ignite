//! Type checker of the Ignite slice (`docs/llvm.md` section 2). Runs after
//! `ignite-check` on each standard-library module in import order and on the
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
//! * The head of a path (`Command` in `Command.Echo(x)`, `Echo` in
//!   `Echo.serve(..)`, `TcpListener` in `TcpListener.bind(..)`) is not a
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
//!   (`ignite-check` leaves it to the type checker).
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
//!   type arguments (`List<A, B>`) are E048. `ignite-check` reports both
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

mod address;
use address::AddressMap;

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use ignite_check::Symbols;
use ignite_syntax::ast::{
    AssemblyDecl, Decl, Expr, FieldInit, Pattern, Place, Program, Span, SystemDecl, Type,
};
use ignite_syntax::diagnostic::Diagnostic;

/// The variants of the builtin enum `Exposure` of a `TcpListener` resource
/// (`docs/slice.md` section 14.4), in declaration order.
pub const EXPOSURE_VARIANTS: [&str; 3] = ["Loopback", "Private", "Public"];

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
    /// One end of an in-memory stream pair, `Pipe.pair()` (`docs/slice.md`
    /// 18.4): a handle like `TcpStream`.
    PipeStream,
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
    /// `Map<K, V>` (`docs/slice.md` section 14.3): value semantics, keys
    /// in order.
    Map(Box<Ty>, Box<Ty>),
    /// `Duration`, `ByteSize` or `Ratio` (`docs/slice.md` section 14.1),
    /// represented as an `Int` of nanoseconds, bytes or parts per million.
    Quantity(QuantityKind),
    /// `Ref<K>` (`Some(K)`, a record type) or the kind-erased `Ref`
    /// (`None`): a reference `System.member` to a component or resource
    /// (`docs/slice.md` section 14.4). Lowered as the `Text` of
    /// [`TypedProgram::reference`].
    Ref(Option<Box<Ty>>),
    /// `Signal<T>`: an output field of a component kind, or the value
    /// `component.field` naming one. No runtime behaviour in slice 10;
    /// lowered as the `Text` of [`TypedProgram::reference`].
    Signal(Box<Ty>),
    /// `SystemRef`: the value of a system name (`system = Terminal`).
    SystemRef,
    /// `Protocol`: the value of a protocol name (`endpoint(Echo)`).
    Protocol,
}

/// The three quantity types of `docs/slice.md` section 14.1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QuantityKind {
    /// Nanoseconds.
    Duration,
    /// Bytes.
    ByteSize,
    /// Parts per million.
    Ratio,
}

impl QuantityKind {
    /// The type name: `Duration`, `ByteSize`, `Ratio`.
    pub fn name(self) -> &'static str {
        match self {
            QuantityKind::Duration => "Duration",
            QuantityKind::ByteSize => "ByteSize",
            QuantityKind::Ratio => "Ratio",
        }
    }

    pub fn from_name(name: &str) -> Option<QuantityKind> {
        match name {
            "Duration" => Some(QuantityKind::Duration),
            "ByteSize" => Some(QuantityKind::ByteSize),
            "Ratio" => Some(QuantityKind::Ratio),
            _ => None,
        }
    }

    /// The literal form `debug()` prints for `value` base units: the
    /// largest unit that divides it exactly (`15 minutes`, `4096 bytes`,
    /// `16 KiB`, `99.9%`), the same on every target.
    pub fn debug_text(self, value: i64) -> String {
        let units: &[(&str, &str, i64)] = match self {
            QuantityKind::Duration => &[
                ("day", "days", 86_400_000_000_000),
                ("hour", "hours", 3_600_000_000_000),
                ("minute", "minutes", 60_000_000_000),
                ("second", "seconds", 1_000_000_000),
                ("ms", "ms", 1_000_000),
                ("us", "us", 1_000),
                ("ns", "ns", 1),
            ],
            QuantityKind::ByteSize => &[
                ("GiB", "GiB", 1 << 30),
                ("MiB", "MiB", 1 << 20),
                ("KiB", "KiB", 1 << 10),
                ("byte", "bytes", 1),
            ],
            QuantityKind::Ratio => {
                let sign = if value < 0 { "-" } else { "" };
                let abs = value.unsigned_abs();
                let whole = abs / 10_000;
                let frac = abs % 10_000;
                if frac == 0 {
                    return format!("{sign}{whole}%");
                }
                let digits = format!("{frac:04}");
                return format!("{sign}{whole}.{}%", digits.trim_end_matches('0'));
            }
        };
        for (one, many, factor) in units {
            if value != 0 && value % factor == 0 {
                let n = value / factor;
                let unit = if n == 1 || n == -1 { one } else { many };
                return format!("{n} {unit}");
            }
        }
        let (_, many, _) = units[units.len() - 1];
        format!("{value} {many}")
    }
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
                !ignite_check::covers(&mapped, &normal)
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
            Ty::Map(k, v) => Ty::Map(boxed(k), boxed(v)),
            Ty::Ref(Some(k)) => Ty::Ref(Some(boxed(k))),
            Ty::Signal(t) => Ty::Signal(boxed(t)),
            _ => self.clone(),
        }
    }

    /// Whether `self` mentions a `Param` for which `f` holds.
    pub(crate) fn has_param(&self, f: &dyn Fn(&str) -> bool) -> bool {
        let any = |ts: &[Ty]| ts.iter().any(|t| t.has_param(f));
        match self {
            Ty::Param(name) => f(name),
            Ty::List(t) | Ty::Option(t) | Ty::Store(t) | Ty::Signal(t) => t.has_param(f),
            Ty::Ref(Some(t)) => t.has_param(f),
            Ty::Map(k, v) => k.has_param(f) || v.has_param(f),
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
    /// Ignite syntax: `Bytes` for `List<Byte>`, records and enums by their
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
            Ty::PipeStream => write!(f, "PipeStream"),
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
            Ty::Map(k, v) => write!(f, "Map<{k}, {v}>"),
            Ty::Quantity(kind) => write!(f, "{}", kind.name()),
            Ty::Ref(Some(k)) => write!(f, "Ref<{k}>"),
            Ty::Ref(None) => write!(f, "Ref"),
            Ty::Signal(t) => write!(f, "Signal<{t}>"),
            Ty::SystemRef => write!(f, "SystemRef"),
            Ty::Protocol => write!(f, "Protocol"),
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
    /// `s.upsert(id, f: fn(Option<T>) -> T) -> T`: `f` of the current value or
    /// `None`, stored and returned, atomically.
    StoreUpsert,
    /// `Store.new() -> Store<T>`, `T` from the expected type; tests and
    /// test-only items only (`docs/slice.md` section 12.3).
    StoreNew,
    /// `x.debug() -> Text` on a value of any type but a type parameter:
    /// the debug text of `docs/slice.md` section 13.6.
    Debug,
    /// `Map.empty() -> Map<K, V>`, `K` and `V` from the expected type
    /// (`docs/slice.md` section 14.3).
    MapEmpty,
    /// `m.get(k) -> Option<V>`.
    MapGet,
    /// `m.put(k, v) -> Unit`; `m` is a place, like `push`.
    MapPut,
    /// `m.remove(k) -> Option<V>`; `m` is a place, like `push`.
    MapRemove,
    /// `m.keys() -> List<K>`, in key order.
    MapKeys,
    /// `m.len() -> Int`.
    MapLen,
    /// `m.contains(k) -> Bool`.
    MapContains,
    /// `d.to_ms() -> Int` on a `Duration` (truncating).
    DurationToMs,
    /// `d.to_ns() -> Int` on a `Duration`.
    DurationToNs,
    /// `b.to_int() -> Int` on a `ByteSize`: bytes.
    ByteSizeToInt,
    /// `r.ppm() -> Int` on a `Ratio`.
    RatioPpm,
    /// `e.with_exit_code(n) -> IoError` (`docs/slice.md` section 14.5, O12).
    IoErrorWithExitCode,
    /// `module("p")`: the record of [`TypedProgram::module_record`].
    ModuleRecord,
    /// `Clock.monotonic() -> Duration`: the time since an arbitrary start
    /// of the process, never decreasing (`docs/slice.md` 16.13); effect
    /// `clock.read`.
    ClockMonotonic,
    /// `Endpoint.of(ref: Ref<K>) -> Result<std.net.Endpoint, IoError>`:
    /// the `--ref path=address:port` argument for the reference's text
    /// (`docs/slice.md` 16.9, 16.13); effect `process.args`.
    EndpointOf,
    /// `Pipe.pair() -> (PipeStream, PipeStream)`: two connected in-memory
    /// stream ends; tests and test-only items only (`docs/slice.md` 18.4).
    PipePair,
    /// `p.read(maximum) -> Result<Input, IoError>` on a `PipeStream`.
    PipeStreamRead,
    /// `p.write_all(data) -> Result<Unit, IoError>` on a `PipeStream`.
    PipeStreamWriteAll,
    /// `p.close() -> Result<Unit, IoError>` on a `PipeStream`.
    PipeStreamClose,
    /// `p.set_idle_timeout(ms) -> Result<Unit, IoError>` on a `PipeStream`.
    PipeStreamSetIdleTimeout,
    /// `p.received_batches() -> Int`: the deliveries into this end's queue
    /// since `Pipe.pair()`; effect `<root>.read`.
    PipeStreamReceivedBatches,
    /// `s.read(maximum)` on a value of a `ByteStream`-bounded type
    /// parameter (`docs/slice.md` 18.3). A backend resolves it by the
    /// instance's type argument to the `TcpStream` or `PipeStream` entry.
    StreamRead,
    /// `s.write_all(data)` on a `ByteStream`-bounded receiver.
    StreamWriteAll,
    /// `s.close()` on a `ByteStream`-bounded receiver.
    StreamClose,
    /// `s.set_idle_timeout(ms)` on a `ByteStream`-bounded receiver.
    StreamSetIdleTimeout,
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
    /// Parameter names of every fn (key: its qualified name), for named
    /// arguments (`docs/slice.md` section 14.2).
    pub param_names: BTreeMap<String, Vec<String>>,
    /// The bounded type parameters of fns (key: the qualified name) and
    /// roles (key: `Protocol.role`): parameter to interface
    /// (`docs/slice.md` 18.13). Absent for a fn without bounds.
    pub bounds: BTreeMap<String, BTreeMap<String, String>>,
}

impl ModuleTypes {
    /// Owned metadata for the public queries on `TypedProgram`, after typing.
    /// Keep every record/enum shape, default, generic parameter list and
    /// bound ([`TypedProgram::bound`]).
    /// Function/constant signatures, protocols, aliases, constant values and
    /// parameter names are needed while typing, but no retained query reads
    /// them. A new query that needs one must extend this snapshot and the
    /// ownership regression; these snapshots must never be typing inputs.
    fn query_snapshot(&self) -> ModuleTypes {
        ModuleTypes {
            records: self.records.clone(),
            enums: self.enums.clone(),
            defaults: self.defaults.clone(),
            generics: self.generics.clone(),
            bounds: self.bounds.clone(),
            ..ModuleTypes::default()
        }
    }

    /// Whether the module declares a record, enum, fn, const, protocol or
    /// alias named `name`.
    pub fn exports_name(&self, name: &str) -> bool {
        self.records.contains_key(name)
            || self.enums.contains_key(name)
            || self.fns.contains_key(name)
            || self.consts.contains_key(name)
            || self.protocols.contains_key(name)
            || self.aliases.contains_key(name)
    }
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

/// What a brace literal `{ k = e, .. }` is (`docs/slice.md` section 14.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BraceKind {
    /// A record literal of this record type (`Ty::Record` or
    /// `Ty::GenericRecord`): each entry key is a field name; omitted fields
    /// take their defaults, as in `Name { .. }`.
    Record(Ty),
    /// A map literal with key and value types. An identifier key is the
    /// `Text` of its name; a qualified key `System.member` is the reference
    /// of [`TypedProgram::reference`] on the key path (see
    /// [`TypedProgram::brace_key`]).
    Map(Ty, Ty),
}

/// The constant a reference, signal, system or protocol value denotes
/// (`docs/slice.md` section 14.4). Every target lowers it as the `Text`
/// of [`Reference::text`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reference {
    /// `system = Terminal`: a [`Ty::SystemRef`].
    System(String),
    /// `Terminal.ingress`, or a bare member name where `Ref<K>` is
    /// expected: a [`Ty::Ref`].
    Member { system: String, member: String },
    /// `promptProbe.attempts`: a [`Ty::Signal`].
    Signal {
        system: String,
        member: String,
        field: String,
    },
    /// `Echo` where a `Protocol` is expected.
    Protocol(QualName),
}

impl Reference {
    /// `Terminal`, `Terminal.ingress`, `Terminal.promptProbe.attempts`,
    /// `std.protocol.echo.Echo`.
    pub fn text(&self) -> String {
        match self {
            Reference::System(system) => system.clone(),
            Reference::Member { system, member } => format!("{system}.{member}"),
            Reference::Signal {
                system,
                member,
                field,
            } => format!("{system}.{member}.{field}"),
            Reference::Protocol(q) if q.module.is_empty() => q.name.clone(),
            Reference::Protocol(q) => format!("{}.{}", q.module, q.name),
        }
    }
}

/// `module("p")` (`docs/slice.md` section 14.4): a record of the
/// expected type `record` whose fields are the module's public fns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleRecord {
    /// The module path as written, e.g. `examples.terminal.commands`.
    pub module: String,
    /// The record type `I` of `Module<I>`.
    pub record: Ty,
    /// Field name to the fn that fills it, in field declaration order.
    pub fns: Vec<(String, QualName)>,
}

/// A component that produces an executable (`docs/slice.md` section
/// 14.5): its kind has `fn K.main(self: K) -> Result<(), IoError>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutableInfo {
    pub system: String,
    /// The component's name; its declaration is the component of that
    /// name in `system`.
    pub component: String,
    /// The component's record type `K` (type arguments inferred for a
    /// generic kind).
    pub kind: Ty,
    /// `K.main`.
    pub main: QualName,
    /// The type arguments of a generic `K.main<T, ..>(self: K<T, ..>)`, in
    /// its type parameter order, bound from the component's kind; empty
    /// for a `main` without type parameters.
    pub main_args: Vec<Ty>,
    /// The executable's name: the `executable` property when it is a
    /// string literal, else the component name.
    pub executable: String,
    /// Resources the component uses through bare names, its embedded
    /// components' included, with their types (`Ty::TcpListener`,
    /// `Ty::Store`), in declaration order.
    pub resources: Vec<(String, Ty)>,
    /// Components whose values the component embeds through bare names,
    /// transitively, with their record types; each comes after every
    /// component it embeds, so they can be built in this order.
    pub embedded: Vec<(String, Ty)>,
}

/// The operation that failed at a [`FailureSite`] (`docs/slice.md`
/// section 14.8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FailureOp {
    Assert,
    Try,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Index,
    Match,
    Unwrap,
    Slice,
}

impl FailureOp {
    /// The operation as the report names it: `assert`, `?`, `+`, `-`, `*`,
    /// `/`, `%`, `index`, `match`, `unwrap`, `slice`.
    pub fn as_str(self) -> &'static str {
        match self {
            FailureOp::Assert => "assert",
            FailureOp::Try => "?",
            FailureOp::Add => "+",
            FailureOp::Sub => "-",
            FailureOp::Mul => "*",
            FailureOp::Div => "/",
            FailureOp::Rem => "%",
            FailureOp::Index => "index",
            FailureOp::Match => "match",
            FailureOp::Unwrap => "unwrap",
            FailureOp::Slice => "slice",
        }
    }
}

/// Where an expression that can fail reports its failure (`docs/slice.md`
/// section 14.8). The frontend has no file text; a target turns `offset`
/// into `<file>:<line>:<col>` with the module's `SourceFile`
/// ([`FailureSite::position`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FailureSite {
    pub op: FailureOp,
    /// Byte offset of the reported position in the module's source.
    pub offset: usize,
}

impl FailureSite {
    /// `(path, line, column)` of the site in `source`.
    pub fn position(&self, source: &ignite_syntax::SourceFile) -> (String, usize, usize) {
        let (line, col) = source.line_col(self.offset);
        (source.path.clone(), line, col)
    }
}

/// The default values of the builtin resource kind `TcpListener` that a
/// declaration may leave out (`docs/slice.md` section 14.4): the variant
/// of `exposure` and the value of `backlog`. [`PropertySource::Default`]
/// names them `TcpListener.exposure` and `TcpListener.backlog` in the
/// builtin module `""`.
pub const TCP_LISTENER_DEFAULT_EXPOSURE: &str = "Loopback";
pub const TCP_LISTENER_DEFAULT_BACKLOG: i64 = 128;

/// Where a property of a member or assembly declaration takes its value
/// (`docs/slice.md` section 15.9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropertySource {
    /// Set in the declaration: the index into its `properties`
    /// (`FieldInit`s, source order). [`MemberInfo::property`] and
    /// [`AssemblyInfo::property`] give the expression.
    Set(usize),
    /// Left out; the kind's field default `<Record>.<field>` in the
    /// kind's module ([`field_default`] finds its expression), or
    /// `TcpListener.exposure` and `TcpListener.backlog` in the builtin
    /// module `""` ([`TCP_LISTENER_DEFAULT_EXPOSURE`],
    /// [`TCP_LISTENER_DEFAULT_BACKLOG`]).
    Default(QualName),
    /// The `executable` field left out: its value is the component's
    /// name (`docs/slice.md` section 14.5).
    ComponentName,
}

/// Which declaration a member of a `system` is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MemberDeclaration {
    Resource,
    Component,
    Contract,
    Policy,
}

impl MemberDeclaration {
    /// The keyword: `resource`, `component`, `contract`, `policy`.
    pub fn as_str(self) -> &'static str {
        match self {
            MemberDeclaration::Resource => "resource",
            MemberDeclaration::Component => "component",
            MemberDeclaration::Contract => "contract",
            MemberDeclaration::Policy => "policy",
        }
    }
}

impl fmt::Display for MemberDeclaration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One member of a `system` (`docs/slice.md` section 15.9): a resource,
/// component, contract or policy with its kind and where each property of
/// the kind takes its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberInfo {
    /// `<System>.<name>`, the member's stable identity.
    pub id: String,
    /// The member's name inside the system.
    pub name: String,
    pub declaration: MemberDeclaration,
    /// Index of the declaration in the system's list for its
    /// [`MemberDeclaration`]: `SystemDecl::resources`, `components`,
    /// `contracts` or `policies`.
    pub index: usize,
    /// `TcpListener`, `Store<T>` or the kind's record type, type
    /// arguments inferred for a generic kind.
    pub kind: Ty,
    /// Every field of the kind that is not an unset `Signal<T>` output,
    /// in the kind's field order (`address`, `port`, `exposure`,
    /// `backlog` for a `TcpListener`; none for a `Store<T>`), with the
    /// source of its value.
    pub properties: Vec<(String, PropertySource)>,
    /// The `Signal<T>` fields the declaration does not set, in field
    /// order, each with its element type `T`.
    pub outputs: Vec<(String, Ty)>,
    /// Byte offset of the declaration's start in the module's source.
    pub offset: usize,
}

impl MemberInfo {
    /// The declaration's `properties` in `system`, the `SystemDecl` of
    /// [`SystemInfo::decl`].
    pub fn properties_of<'p>(&self, system: &'p SystemDecl) -> &'p [FieldInit] {
        let list: Option<&'p [FieldInit]> = match self.declaration {
            MemberDeclaration::Resource => system
                .resources
                .get(self.index)
                .map(|r| r.properties.as_slice()),
            MemberDeclaration::Component => system
                .components
                .get(self.index)
                .map(|c| c.properties.as_slice()),
            MemberDeclaration::Contract => system
                .contracts
                .get(self.index)
                .map(|c| c.properties.as_slice()),
            MemberDeclaration::Policy => system
                .policies
                .get(self.index)
                .map(|c| c.properties.as_slice()),
        };
        list.unwrap_or(&[])
    }

    /// The expression property `field` is set to in the declaration;
    /// `None` for a defaulted, implicit or unknown property.
    pub fn property<'p>(&self, system: &'p SystemDecl, field: &str) -> Option<&'p Expr> {
        let (_, source) = self.properties.iter().find(|(f, _)| f == field)?;
        match source {
            PropertySource::Set(i) => self.properties_of(system).get(*i).map(|p| &p.value),
            PropertySource::Default(_) | PropertySource::ComponentName => None,
        }
    }
}

/// One `system` of a module (`docs/slice.md` section 15.9) with its
/// members in declaration order: resources, components, contracts,
/// policies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemInfo {
    pub name: String,
    /// Index of the `Decl::System` in `Program::decls`.
    pub decl: usize,
    pub members: Vec<MemberInfo>,
    /// Byte offset of the declaration's start.
    pub offset: usize,
}

impl SystemInfo {
    /// The declaration in `program`, the `Program` this module was typed
    /// from.
    pub fn decl<'p>(&self, program: &'p Program) -> Option<&'p SystemDecl> {
        match program.decls.get(self.decl)? {
            Decl::System(system) => Some(system),
            _ => None,
        }
    }

    pub fn member(&self, name: &str) -> Option<&MemberInfo> {
        self.members.iter().find(|m| m.name == name)
    }
}

/// One `assembly` of a module (`docs/slice.md` section 15.9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssemblyInfo {
    pub name: String,
    /// Index of the `Decl::Assembly` in `Program::decls`.
    pub decl: usize,
    /// The kind's record type.
    pub kind: Ty,
    /// The system its `system` property names; `None` when the property
    /// is not a bare system name.
    pub system: Option<String>,
    /// Every field of the kind in field order with the source of its
    /// value, as [`MemberInfo::properties`].
    pub properties: Vec<(String, PropertySource)>,
    /// Byte offset of the declaration's start.
    pub offset: usize,
}

impl AssemblyInfo {
    pub fn decl<'p>(&self, program: &'p Program) -> Option<&'p AssemblyDecl> {
        match program.decls.get(self.decl)? {
            Decl::Assembly(assembly) => Some(assembly),
            _ => None,
        }
    }

    /// The expression property `field` is set to; `None` for a defaulted
    /// or unknown property.
    pub fn property<'p>(&self, program: &'p Program, field: &str) -> Option<&'p Expr> {
        let (_, source) = self.properties.iter().find(|(f, _)| f == field)?;
        match source {
            PropertySource::Set(i) => self.decl(program)?.properties.get(*i).map(|p| &p.value),
            PropertySource::Default(_) | PropertySource::ComponentName => None,
        }
    }
}

/// The default expression of field `field` of the record `record`
/// declared in `program` (`docs/slice.md` section 4.7), for a
/// [`PropertySource::Default`] whose module's AST is `program`.
pub fn field_default<'p>(program: &'p Program, record: &str, field: &str) -> Option<&'p Expr> {
    program.decls.iter().find_map(|decl| match decl {
        Decl::Record(r) if r.name.name == record => r
            .fields
            .iter()
            .find(|f| f.name.name == field)
            .and_then(|f| f.default.as_ref()),
        _ => None,
    })
}

/// The effects an expression performs (`docs/slice.md` section 15.2),
/// sorted and deduplicated, in the form `requires` writes them: `r.op` on
/// a system member `r` the expression names (`ingress.accept`), an
/// ambient effect (`stdin.read`, `net.listen`), a requirement of a callee
/// kept as written when the argument for its parameter names no member,
/// the bare operation (`accept`, `read`) on a handle that is neither, and
/// `spawn`. Empty for a pure expression: a bare member or resource name
/// is a value, not an operation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EffectSet {
    entries: Vec<String>,
}

impl EffectSet {
    pub fn new(mut entries: Vec<String>) -> EffectSet {
        entries.sort();
        entries.dedup();
        EffectSet { entries }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }
}

/// The result of typing one module. Expression keys are addresses: the
/// `Program` passed to [`type_module`] must be the same object (not a clone)
/// that later stages look expressions up in.
#[derive(Clone, Debug, Default)]
pub struct TypedProgram {
    pub module: String,
    types: AddressMap<Ty>,
    callees: AddressMap<Callee>,
    bindings: HashMap<Span, Ty>,
    /// Constant patterns, keyed by pattern address.
    pattern_consts: AddressMap<QualName>,
    /// The constant each `conforms` entry and requirement reference names,
    /// by the span of the reference (`docs/slice.md` 19.6).
    clause_consts: HashMap<Span, QualName>,
    /// Resolved type expressions, keyed by `Type` address.
    type_exprs: AddressMap<Ty>,
    exports: ModuleTypes,
    /// Owned query metadata of the imported modules and this one: only
    /// records, enums, defaults and generics. See `ModuleTypes::query_snapshot`;
    /// full signatures remain in `exports` and in the caller's typing inputs.
    modules: TypedModules,
    /// Type arguments of generic uses, keyed by expression address.
    instances: AddressMap<Vec<Ty>>,
    /// The module's tests in declaration order.
    tests: Vec<TestInfo>,
    /// `==`/`!=` assert conditions, keyed by the condition's address.
    assert_compares: AddressMap<AssertCompare>,
    /// `?` inside test bodies, keyed by the `Try` expression's address.
    test_tries: AddressMap<TestTry>,
    /// Closure literals, keyed by expression address.
    closures: AddressMap<ClosureInfo>,
    /// Bounds of range patterns, keyed by pattern address.
    ranges: AddressMap<(i64, i64)>,
    /// Owners of the last field of assignment places, by place span.
    places: HashMap<Span, Ty>,
    /// Quantity literals, keyed by expression address.
    quantities: AddressMap<(QuantityKind, i64)>,
    /// Calls with named arguments: parameter index to argument index.
    named: AddressMap<Vec<usize>>,
    /// Brace literals, keyed by expression address.
    braces: AddressMap<BraceKind>,
    /// References, signals, system and protocol values, keyed by
    /// expression address (or by brace-key path address).
    references: AddressMap<Reference>,
    /// `module("p")` calls, keyed by expression address.
    module_records: AddressMap<ModuleRecord>,
    executables: Vec<ExecutableInfo>,
    /// Failure sites, keyed by expression address.
    failures: AddressMap<FailureSite>,
    /// The module's systems in declaration order (slice 11).
    systems: Vec<SystemInfo>,
    /// The module's assemblies in declaration order (slice 11).
    assemblies: Vec<AssemblyInfo>,
    /// Effect sets of property expressions, constant values and field
    /// defaults and their sub-expressions, keyed by expression address.
    effects: AddressMap<EffectSet>,
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

    /// The constant a clause name resolves to (`docs/slice.md` 19.6), by
    /// the span of the reference: a `conforms` entry and the first segment
    /// of a requirement reference give the `Spec` constant
    /// (`std.rfc.rfc9112.RFC9112`), the whole reference `Spec.Id` gives the
    /// `Requirement` constant (`std.rfc.rfc9112.R7_1_2`).
    pub fn clause_constant(&self, span: Span) -> Option<&QualName> {
        self.clause_consts.get(&span)
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

    /// The interface that bounds type parameter `param` of the fn
    /// `fn_name` (for a role, the name `Protocol.role`) of this module or a
    /// module it was typed against: `Some("ByteStream")`, or `None` for an
    /// unbounded parameter (`docs/slice.md` 18.3, 18.13).
    pub fn bound(&self, fn_name: &QualName, param: &str) -> Option<&str> {
        self.modules
            .get(&fn_name.module)?
            .bounds
            .get(&fn_name.name)?
            .get(param)
            .map(String::as_str)
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

    /// The type of the record whose field the assignment place `a.b.c`
    /// writes (the type of `a.b`); `None` for a place without fields.
    pub fn place_owner(&self, place: &Place) -> Option<&Ty> {
        self.places.get(&place.span)
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

    /// The kind and value in base units (ns, bytes, ppm) of a quantity
    /// literal (`ExprKind::Quantity`, `docs/slice.md` section 14.1).
    pub fn quantity(&self, expr: &Expr) -> Option<(QuantityKind, i64)> {
        self.quantities.get(&key(expr)).copied()
    }

    /// For a call or method call with named arguments (`docs/slice.md`
    /// section 14.2): element `i` is the index in `args` of the argument
    /// for parameter `i` (a method call's receiver is not counted, so for
    /// a UFCS call parameter 0 is the first parameter after the receiver).
    /// Arguments are evaluated in source order; `None` for a call without
    /// named arguments.
    pub fn named_args(&self, call: &Expr) -> Option<&[usize]> {
        self.named.get(&key(call)).map(Vec::as_slice)
    }

    /// Whether a brace literal (`ExprKind::Brace`) is a record or a map
    /// literal (`docs/slice.md` section 14.3).
    pub fn brace_kind(&self, expr: &Expr) -> Option<&BraceKind> {
        self.braces.get(&key(expr))
    }

    /// The reference a qualified key of a map brace literal denotes
    /// (`Terminal.ingress`); `None` for an identifier key, which is `Text`.
    pub fn brace_key(&self, key_path: &ignite_syntax::ast::Path) -> Option<&Reference> {
        self.references
            .get(&(key_path as *const ignite_syntax::ast::Path as usize))
    }

    /// The constant an expression of type `Ref`, `Signal`, `SystemRef` or
    /// `Protocol` denotes (`docs/slice.md` section 14.4).
    pub fn reference(&self, expr: &Expr) -> Option<&Reference> {
        self.references.get(&key(expr))
    }

    /// The record a `module("p")` call builds (`docs/slice.md` section
    /// 14.4).
    pub fn module_record(&self, expr: &Expr) -> Option<&ModuleRecord> {
        self.module_records.get(&key(expr))
    }

    /// The components of this module's systems that produce executables
    /// (`docs/slice.md` section 14.5), in declaration order.
    pub fn executables(&self) -> &[ExecutableInfo] {
        &self.executables
    }

    /// The failure site of an expression that can fail (`docs/slice.md`
    /// section 14.8): `+ - *` on `Int` and quantities, `/ %` (also the
    /// division by zero), a prefix `-` (key: the `Unary`), an index, a
    /// `match`, `?` (key: the `Try`), `slice` and `unwrap` calls, and an
    /// `assert` (key: its condition; offset: the `assert` keyword). The
    /// offset is the expression's start, except for `?` (the `?`
    /// character) and `assert`.
    pub fn failure_site(&self, expr: &Expr) -> Option<FailureSite> {
        self.failures.get(&key(expr)).copied()
    }

    /// The systems this module declares with their members
    /// (`docs/slice.md` section 15.9), in declaration order.
    pub fn systems(&self) -> &[SystemInfo] {
        &self.systems
    }

    /// The assemblies this module declares (`docs/slice.md` section
    /// 15.9), in declaration order.
    pub fn assemblies(&self) -> &[AssemblyInfo] {
        &self.assemblies
    }

    /// The effects `expr` performs (`docs/slice.md` section 15.2), for a
    /// property expression of a resource, component, contract, policy or
    /// assembly, a constant's value, a field default, and every
    /// sub-expression of those (closure bodies excluded: creating a
    /// closure performs nothing). `None` for an expression inside a fn,
    /// role or test body, whose effects `ignite-check` accounts to the
    /// fn. E116 reports a non-empty set on a property expression, so a
    /// typed program's property expressions all have empty sets.
    pub fn effects(&self, expr: &Expr) -> Option<&EffectSet> {
        self.effects.get(&key(expr))
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
pub fn equality(ty: &Ty, modules: &TypedModules) -> Option<Equality> {
    fn parts_ok(ty: &Ty, modules: &TypedModules, seen: &mut Vec<Ty>) -> bool {
        match ty {
            Ty::Int
            | Ty::Byte
            | Ty::Bool
            | Ty::Unit
            | Ty::Text
            | Ty::Input
            | Ty::Quantity(_)
            | Ty::Ref(_)
            | Ty::Signal(_)
            | Ty::SystemRef
            | Ty::Protocol => true,
            Ty::List(t) | Ty::Option(t) => parts_ok(t, modules, seen),
            Ty::Map(k, v) => parts_ok(k, modules, seen) && parts_ok(v, modules, seen),
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
            | Ty::PipeStream
            | Ty::IoError
            | Ty::Store(_)
            | Ty::Param(_)
            | Ty::Never => false,
        }
    }
    match ty {
        Ty::Int | Ty::Byte | Ty::Bool | Ty::Unit | Ty::Quantity(_) => Some(Equality::Scalar),
        Ty::Text | Ty::Ref(_) | Ty::Signal(_) | Ty::SystemRef | Ty::Protocol => {
            Some(Equality::Text)
        }
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
        Ty::Record(name)
        | Ty::GenericRecord(name, _)
        | Ty::Enum(name)
        | Ty::GenericEnum(name, _) => {
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
pub fn boxed_payload(
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
pub fn boxed_field(record: &QualName, field: &str, modules: &TypedModules) -> bool {
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
        Ty::Enum(name) if name.module.is_empty() && name.name == "Exposure" => builtin(
            "Exposure",
            EXPOSURE_VARIANTS.iter().map(|v| (*v, vec![])).collect(),
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
/// `ignite_check::check_module` returned for it; `modules` holds the typed
/// standard-library modules it may import.
pub fn type_module(
    program: &Program,
    symbols: &Symbols,
    modules: &TypedModules,
) -> Result<TypedProgram, Vec<Diagnostic>> {
    let typed = typer::Typer::new(program, symbols, modules).run(program)?;
    let mut all: TypedModules = modules
        .iter()
        .map(|(path, types)| (path.clone(), types.query_snapshot()))
        .collect();
    all.insert(typed.module.clone(), typed.exports.query_snapshot());
    Ok(TypedProgram {
        module: typed.module,
        types: typed.types,
        callees: typed.callees,
        bindings: typed.bindings,
        pattern_consts: typed.pattern_consts,
        clause_consts: typed.clause_consts,
        type_exprs: typed.type_exprs,
        exports: typed.exports,
        modules: all,
        instances: typed.instances,
        tests: typed.tests,
        assert_compares: typed.assert_compares,
        test_tries: typed.test_tries,
        closures: typed.closures,
        ranges: typed.ranges,
        places: typed.places,
        quantities: typed.system.quantities,
        named: typed.system.named,
        braces: typed.system.braces,
        references: typed.system.references,
        module_records: typed.system.module_records,
        executables: typed.system.executables,
        failures: typed.system.failures,
        systems: typed.system.systems,
        assemblies: typed.system.assemblies,
        effects: typed.system.effects,
    })
}
