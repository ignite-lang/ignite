//! Name resolution, effect and protocol checking (`docs/slice.md` sections
//! 4, 5 and 8).
//!
//! [`check_module`] walks one parsed module against the standard-library
//! modules it may import and either returns the [`Symbols`] later stages
//! need or every diagnostic it found. It does not type expressions; that is
//! `ignite-types`, which runs after it.
//!
//! Rules beyond `docs/slice.md`:
//!
//! * Imports are resolved first, then module-level names are collected, so
//!   declaration order does not matter. A declaration named like a builtin
//!   type, builtin function or `Tuple` is `E004`. A record field or enum
//!   variant declared twice is `E004`.
//! * A module exports what it defines; the builtin enums its `Symbols`
//!   carry and the names it imported are not exports. Importing such a name
//!   is `E031`. Importing one name twice is `E004`.
//! * Name lookup in a value position: local bindings (innermost scope
//!   first), the value `Unit`, constants, bare variants, then pure
//!   functions.
//! * A constant's value and a field default are walked as pure contexts
//!   with no local bindings; an effectful operation or `spawn` there is
//!   `E010`. A constant cycle (`E015`) is one through direct references
//!   among the module's own constants; references inside function bodies
//!   are not followed. `E015` is reported at every constant on a cycle.
//! * `Process` is a builtin static owner (`Process.exit`), not a type; a
//!   declaration named `Process` is `E004` like one named after a builtin
//!   type.
//! * Callee lookup for `f(..)`: local binding (a pure call, arity not
//!   checked), user function, builtin function, variant constructor. Arity
//!   is checked for the last three (`E003`, "wrong number of arguments").
//! * An identifier that is not a local binding or a constant and is
//!   uppercase or names a protocol, followed by `.name`, is a path: `Enum.Variant`, a protocol
//!   role `Echo.serve` or a builtin static such as `Bytes.empty`. The
//!   parser may produce `Enum.Variant(x)` either as a call on a field or as
//!   a method call; both are treated as a path call.
//! * A protocol role that is not `serve` or `connect`, a pure role, a role
//!   with the wrong parameter count, a protocol without roles and a call to
//!   a role the protocol does not declare are `E030`; a role declared twice
//!   is `E004`. A `serve` role takes 2 or 3 parameters ([`SERVE_ARITIES`]).
//! * A bound `<S: ByteStream>` on a fn or role makes `S` a receiver of the
//!   four stream methods, resolved as builtins like `TcpStream`'s. A bound
//!   naming anything else or two interfaces, and an interface in a type
//!   position, are `E099` (`docs/slice.md` 18.3, 18.7). Whether a use meets
//!   the bound is the type checker's `E096`.
//! * `spawn` of an effectful substrate operation is `E013`: only user
//!   functions and protocol roles can be spawned.
//! * Receiver types are unknown here. A method named in
//!   [`BUILTIN_METHODS`] is a builtin; one named in [`EFFECTFUL_METHODS`] is
//!   an effectful substrate operation. Any other method name that names a
//!   module-level fn is taken as a UFCS call of it for arity (`E003`,
//!   receiver counted) and effects; otherwise it is a pure call of a
//!   fn-typed field. The type checker confirms both (`docs/slice.md`
//!   section 4.8).
//! * Bare variant names are not resolved to an enum here: an ambiguous one
//!   is the type checker's `E005`. Arity is checked when the owner is
//!   unique. A bare name that is no name in scope but a variant of an enum
//!   some given module defines (imported or not) is not `E003` here: the
//!   type checker resolves it against the expected type or reports `E003`.
//! * A payload-less pattern naming a constant is a constant pattern; with
//!   payload patterns it is `E003`. A name bound inside an or-pattern is
//!   `E049`; it is still bound for the arm body.
//! * `system` and `assembly` declarations take part in the module
//!   namespace; their resource, component, contract, policy and assembly
//!   properties are not resolved here. The type checker owns them (`E020`,
//!   `E104`, `E105`, `E108`, `E109`; `docs/slice.md` section 14).
//! * Effects (`E016`) follow the value of an expression to the parameters
//!   it may come from ([`Checker::roots`]): a `var` assigned from several
//!   parameters carries all of them, whatever the control flow. A fn
//!   without `requires` gets its effectfulness and effect set by a fixed
//!   point: [`check_module`] walks the module until no inferred set grows,
//!   keeping the diagnostics of the last walk only.
//! * A type expression with an unknown name or the wrong number of type
//!   arguments is `E048`; so is a record literal naming an unknown record.
//! * The `package` of a standard-library file is not compared with its
//!   path here; the driver keys `Modules` by module path and owns that
//!   `E031`, and the import-cycle check `E032`.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use ignite_syntax::ast::{
    Block, Closure, Decl, Expr, ExprKind, FieldInit, FnDecl, Ident, Import, NodeId, Path, Pattern,
    PatternKind, Program, ProtocolDecl, Span, Stmt, StmtKind, SystemDecl, TestDecl, Type,
};
use ignite_syntax::diagnostic::Diagnostic;

mod clauses;

/// Builtin enums and their variants, in scope in every program.
pub const BUILTIN_ENUMS: &[(&str, &[&str])] = &[
    ("Option", &["Some", "None"]),
    ("Result", &["Ok", "Err"]),
    ("Input", &["Data", "Eof"]),
    ("Exposure", &["Loopback", "Private", "Public"]),
];

/// Builtin type names.
pub const BUILTIN_TYPES: &[&str] = &[
    "Text",
    "Bytes",
    "Byte",
    "Int",
    "Bool",
    "Unit",
    "List",
    "Option",
    "Result",
    "TcpListener",
    "TcpStream",
    "PipeStream",
    "Stdin",
    "Stdout",
    "Stderr",
    "Input",
    "IoError",
    "Store",
    "Map",
    "Duration",
    "ByteSize",
    "Ratio",
    "Ref",
    "Signal",
    "SystemRef",
    "Protocol",
    "Module",
    "Exposure",
    "Directory",
    "Launcher",
    "ChildProcess",
    "Entropy",
    "Setting",
    "CommandLine",
    "Environment",
];

/// Static builtin calls written `Type.name(...)`.
pub const BUILTIN_STATICS: &[(&str, &str)] = &[
    ("Bytes", "empty"),
    ("Bytes", "of"),
    ("List", "empty"),
    ("Int", "to_text"),
    ("TcpListener", "bind"),
    ("TcpStream", "connect"),
    ("Stdin", "read"),
    ("Stdin", "is_terminal"),
    ("Stdin", "set_raw"),
    ("Stdout", "write_all"),
    ("Stderr", "write_all"),
    ("Process", "exit"),
    ("IoError", "new"),
    ("Int", "parse"),
    ("Store", "new"),
    ("Map", "empty"),
    ("Clock", "monotonic"),
    ("Endpoint", "of"),
    ("Pipe", "pair"),
    ("Directory", "temporary"),
    ("Launcher", "new"),
    ("Entropy", "new"),
    ("Setting", "of"),
    ("CommandLine", "of"),
    ("Environment", "of"),
    ("Clock", "sleep"),
    ("Clock", "now"),
    ("Process", "flush"),
    ("TcpStream", "connect_within"),
];

/// Owners of [`BUILTIN_STATICS`] that are records of the standard library,
/// not builtin types: `std.net.Endpoint` declares the record and the
/// substrate supplies `Endpoint.of` (`docs/slice.md` 16.13), so its
/// declaration is no duplicate of a builtin (E004).
pub const STD_RECORD_STATIC_OWNERS: &[&str] = &["Endpoint"];

/// Substrate operations that are effectful, by method name.
pub const EFFECTFUL_METHODS: &[&str] = &[
    "bind",
    "accept",
    "connect",
    "read",
    "write_all",
    "close",
    "set_idle_timeout",
    "received_batches",
    "write_urgent",
    "urgent_pending",
    "timed_out",
];

/// Every builtin method name (`docs/slice.md` sections 6, 7, 10 and 13).
/// A method call with one of these names is a builtin when the builtin
/// applies to the receiver's type, else a UFCS call of a user fn of that
/// name (`docs/slice.md` section 13, row 5).
pub const BUILTIN_METHODS: &[&str] = &[
    "debug",
    "len",
    "is_empty",
    "trim",
    "starts_with",
    "strip_prefix",
    "concat",
    "to_bytes",
    "to_text",
    "at",
    "slice",
    "push",
    "set",
    "append",
    "clear",
    "join",
    "copy",
    "accept",
    "local_port",
    "read",
    "write_all",
    "close",
    "bind",
    "connect",
    "map",
    "to_int",
    "to_byte",
    "wrapping_add",
    "wrapping_sub",
    "wrapping_mul",
    "checked_add",
    "checked_sub",
    "checked_mul",
    "set_idle_timeout",
    "get",
    "put",
    "remove",
    "keys",
    "contains",
    "to_ms",
    "to_ns",
    "ppm",
    "with_exit_code",
    "received_batches",
    "write_urgent",
    "urgent_pending",
    "peer",
    "timed_out",
];

/// Free builtin functions.
pub const BUILTIN_FUNCTIONS: &[&str] = &["log", "module"];

/// Modules whose names are the builtins: importing from them only checks
/// that the name is a builtin.
pub const SUBSTRATE_MODULES: &[&str] = &["std.net", "std.io", "std.fs", "std.process"];

/// The module that declares the clause vocabulary (`docs/slice.md` 19.3):
/// the checker, the type checker and the evaluator read its records and
/// enums by name.
pub const RFC_MODULE: &str = "std.rfc";

/// The protocol roles and their parameter counts.
pub const PROTOCOL_ROLES: &[(&str, usize)] = &[("serve", 2), ("connect", 1)];

/// The parameter counts a `serve` role may have (`docs/slice.md` 18.6,
/// D12): `(stream, application)` or `(stream, limits, application)`. E030
/// reads this for `serve`; [`PROTOCOL_ROLES`] keeps the two-parameter form.
pub const SERVE_ARITIES: &[usize] = &[2, 3];

/// The interfaces a type parameter of a fn or role may be bounded by
/// (`docs/slice.md` 18.3): compiler-known, not declared in Ignite. Each is
/// a builtin name (E004, imports from `std.net`) and no type (E099).
pub const BUILTIN_INTERFACES: &[&str] = &["ByteStream"];

/// Payload count of each builtin variant, in the order of [`BUILTIN_ENUMS`].
const BUILTIN_VARIANT_PAYLOADS: &[(&str, usize)] = &[
    ("Some", 1),
    ("None", 0),
    ("Ok", 1),
    ("Err", 1),
    ("Data", 1),
    ("Eof", 0),
    ("Loopback", 0),
    ("Private", 0),
    ("Public", 0),
];

/// Arity of each entry of [`BUILTIN_STATICS`].
const BUILTIN_STATIC_ARITIES: &[(&str, &str, usize)] = &[
    ("Bytes", "empty", 0),
    ("Bytes", "of", 1),
    ("List", "empty", 0),
    ("Int", "to_text", 1),
    ("TcpListener", "bind", 2),
    ("TcpStream", "connect", 2),
    ("Stdin", "read", 1),
    ("Stdin", "is_terminal", 0),
    ("Stdin", "set_raw", 1),
    ("Stdout", "write_all", 1),
    ("Stderr", "write_all", 1),
    ("Process", "exit", 1),
    ("IoError", "new", 1),
    ("Int", "parse", 1),
    ("Store", "new", 0),
    ("Map", "empty", 0),
    ("Clock", "monotonic", 0),
    ("Endpoint", "of", 1),
    ("Pipe", "pair", 0),
    ("Directory", "temporary", 0),
    ("Launcher", "new", 0),
    ("Entropy", "new", 0),
    ("Setting", "of", 2),
    ("CommandLine", "of", 1),
    ("Environment", "of", 1),
    ("Clock", "sleep", 1),
    ("Clock", "now", 0),
    ("Process", "flush", 0),
    ("TcpStream", "connect_within", 3),
];

/// Builtin statics that are effectful substrate operations.
const EFFECTFUL_STATICS: &[(&str, &str)] = &[
    ("TcpListener", "bind"),
    ("TcpStream", "connect"),
    ("Stdin", "read"),
    ("Stdin", "is_terminal"),
    ("Stdin", "set_raw"),
    ("Stdout", "write_all"),
    ("Stderr", "write_all"),
    ("Process", "exit"),
    ("Clock", "monotonic"),
    ("Endpoint", "of"),
    ("Clock", "sleep"),
    ("Clock", "now"),
    ("TcpStream", "connect_within"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnInfo {
    pub effectful: bool,
    pub arity: usize,
}

/// What an effectful fn or protocol role declares (`docs/slice.md`
/// section 5): its parameter names in order and its `requires` entries as
/// written (`stream.read`, `stdin.read`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct EffectSig {
    pub params: Vec<String>,
    pub requires: Vec<String>,
}

impl EffectSig {
    fn of(decl: &FnDecl) -> EffectSig {
        EffectSig {
            params: decl.params.iter().map(|p| p.name.name.clone()).collect(),
            requires: decl
                .requires
                .iter()
                .flatten()
                .map(|effect| effect.dotted())
                .collect(),
        }
    }
}

/// The effect a substrate method needs on the value it is called on, by
/// method name: `read`, `received_batches`, `urgent_pending` and `timed_out` need
/// `<root>.read`, `write_all`, `write_urgent`, `set_idle_timeout` and
/// `close` (which shuts down the write half) need `<root>.write`,
/// `accept` needs `<root>.accept`.
pub fn method_effect(method: &str) -> Option<&'static str> {
    match method {
        "read" | "received_batches" | "urgent_pending" | "timed_out" => Some("read"),
        "write_all" | "close" | "set_idle_timeout" | "write_urgent" => Some("write"),
        "accept" => Some("accept"),
        _ => None,
    }
}

/// The effect a method of `Store<T>` needs on its receiver, by method name
/// (`docs/slice.md` section 11.3): `get` and `values` read, the others
/// write. `None` for a name that is no `Store` method.
pub fn store_effect(method: &str) -> Option<&'static str> {
    match method {
        "get" | "values" => Some("read"),
        "next_id" | "put" | "remove" | "update" | "upsert" => Some("write"),
        _ => None,
    }
}

/// The effect a method of a capability type (`docs/slice.md` 21.3) needs on
/// its receiver: `Some(Some(op))` for an effectful method, `Some(None)` for
/// the pure `ChildProcess.id`, `None` for a name that is no method of the
/// type. Checked by the receiver's declared type, as `Store` methods are,
/// so user fns named `start`, `value`, `wait`, `kill` or `execute` stay
/// free (21.11 D3).
pub fn capability_effect(ty: &str, method: &str) -> Option<Option<&'static str>> {
    CAPABILITY_METHODS
        .iter()
        .find(|(t, m, _)| *t == ty && *m == method)
        .map(|(_, _, op)| *op)
}

/// Every method of a capability type (`docs/slice.md` 21.8): its type, its
/// name and the operation of its effect (`None` for the pure
/// `ChildProcess.id`). [`capability_effect`] reads it, and `ignite-types`
/// reads it for the standard-library modules a capability's signatures
/// name.
pub const CAPABILITY_METHODS: &[(&str, &str, Option<&str>)] = &[
    ("Directory", "read_file", Some("read")),
    ("Directory", "read_directory", Some("read")),
    ("Directory", "absolute_path", Some("read")),
    ("Directory", "write_file", Some("write")),
    ("Directory", "create_directory", Some("write")),
    ("Directory", "remove", Some("write")),
    ("Directory", "remove_tree", Some("write")),
    ("Launcher", "start", Some("launch")),
    ("Launcher", "execute", Some("launch")),
    ("ChildProcess", "wait", Some("wait")),
    ("ChildProcess", "kill", Some("kill")),
    ("ChildProcess", "id", None),
    ("Entropy", "random_bytes", Some("draw")),
    ("Setting", "value", Some("argument")),
    ("CommandLine", "arguments", Some("argument")),
    ("Environment", "get", Some("variable")),
];

/// The capability types of `docs/slice.md` 21.3: handles a system declares
/// as resources (all but `ChildProcess`) and passes in values.
pub const CAPABILITY_TYPES: &[&str] = &[
    "Directory",
    "Launcher",
    "ChildProcess",
    "Entropy",
    "Setting",
    "CommandLine",
    "Environment",
];

/// What an in-place builtin changes (`docs/slice.md` 20.3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// The receiver's value: the call changes the variable a place
    /// receiver is rooted at, and nothing else. A closure may not change a
    /// capture this way (E006), and only a place rooted in a `var` or a
    /// parameter may be the receiver.
    Value,
    /// The state behind the receiver's handle (`Store`), which every copy
    /// of the handle shares.
    Handle,
}

/// One row of [`IN_PLACE`]: `receiver.method(..)` on a receiver of the
/// builtin type `receiver` changes it in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InPlace {
    pub receiver: &'static str,
    pub method: &'static str,
    pub change: Change,
}

const fn in_place_row(receiver: &'static str, method: &'static str, change: Change) -> InPlace {
    InPlace {
        receiver,
        method,
        change,
    }
}

/// The one table of builtins that change their receiver in place
/// (`docs/slice.md` 20.3.3). The checker reads it here (E006);
/// `ignite_types::ownership` resolves each row to its `Builtin`, and the
/// moves analysis and both backends read it from there.
pub const IN_PLACE: [InPlace; 15] = [
    in_place_row("List", "push", Change::Value),
    in_place_row("List", "set", Change::Value),
    in_place_row("List", "append", Change::Value),
    in_place_row("List", "clear", Change::Value),
    in_place_row("Bytes", "push", Change::Value),
    in_place_row("Bytes", "set", Change::Value),
    in_place_row("Bytes", "append", Change::Value),
    in_place_row("Bytes", "clear", Change::Value),
    in_place_row("Map", "put", Change::Value),
    in_place_row("Map", "remove", Change::Value),
    in_place_row("Store", "next_id", Change::Handle),
    in_place_row("Store", "put", Change::Handle),
    in_place_row("Store", "remove", Change::Handle),
    in_place_row("Store", "update", Change::Handle),
    in_place_row("Store", "upsert", Change::Handle),
];

/// What the builtin `method` changes on a receiver of the builtin type
/// `receiver` ([`IN_PLACE`]). For a receiver whose type is not known, the
/// change every row of that method name agrees on: `push` is a `Value`
/// change whatever the receiver, `put` is none (`Map` and `Store` differ).
pub fn in_place(receiver: Option<&str>, method: &str) -> Option<Change> {
    let mut rows = IN_PLACE
        .iter()
        .filter(|row| row.method == method && receiver.is_none_or(|r| r == row.receiver));
    let first = rows.next()?.change;
    rows.all(|row| row.change == first).then_some(first)
}

/// The heads of ambient effects (`stdin.read`, `net.connect`): no
/// parameter may take one of these names (E056).
pub const AMBIENT_HEADS: &[&str] = &["stdin", "stdout", "stderr", "process", "net", "clock"];

/// The ambient effect an effectful builtin static needs.
pub fn static_effect(owner: &str, name: &str) -> Option<&'static str> {
    match (owner, name) {
        ("Stdin", "read") => Some("stdin.read"),
        ("Stdin", "is_terminal" | "set_raw") => Some("stdin.mode"),
        ("Stdout", "write_all") => Some("stdout.write"),
        ("Stderr", "write_all") => Some("stderr.write"),
        ("Process", "exit") => Some("process.exit"),
        ("TcpListener", "bind") => Some("net.listen"),
        ("TcpStream", "connect") => Some("net.connect"),
        ("Clock", "monotonic") => Some("clock.read"),
        ("Clock", "sleep") => Some("clock.sleep"),
        ("Clock", "now") => Some("clock.read"),
        ("Endpoint", "of") => Some("process.args"),
        ("TcpStream", "connect_within") => Some("net.connect"),
        _ => None,
    }
}

/// What later stages need to know about a checked program.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Symbols {
    pub records: BTreeSet<String>,
    /// Enum name to its variant names, user enums and builtins.
    pub enums: BTreeMap<String, Vec<String>>,
    /// Variant name to the enums declaring it, user enums and builtins.
    pub variants: BTreeMap<String, Vec<String>>,
    pub functions: BTreeMap<String, FnInfo>,
    /// Protocol name to its roles (`serve`, `connect`), own and imported.
    pub protocols: BTreeMap<String, BTreeMap<String, FnInfo>>,
    /// Constant names (`const N: T = e`), own and imported. A name listed
    /// here is a constant wherever it is not shadowed by a local binding.
    pub constants: BTreeSet<String>,
    /// Every imported name (record, enum, fn, const, protocol) to the module path it
    /// came from, e.g. `"Json" -> "std.json"`. Own definitions
    /// are not listed. The maps above contain imported entries too.
    pub imported: BTreeMap<String, String>,
    /// Record name to its field names in declaration order, own and
    /// imported.
    pub record_fields: BTreeMap<String, Vec<String>>,
    /// Enum name to variant name to payload count, own, imported and
    /// builtin.
    pub payloads: BTreeMap<String, BTreeMap<String, usize>>,
    /// Effect signatures of effectful fns (key: the fn name) and protocol
    /// roles (key: `Protocol.role`), own and imported. For a fn without
    /// `requires` the entries are inferred (`docs/slice.md` section 5.1).
    pub effects: BTreeMap<String, EffectSig>,
    /// Type alias name to its number of type parameters, own and imported.
    pub aliases: BTreeMap<String, usize>,
    /// Record name to the fields that have a default value, own and
    /// imported; records without defaulted fields are absent.
    pub record_defaults: BTreeMap<String, BTreeSet<String>>,
    /// Type parameters of generic records, enums, aliases and fns (key: the
    /// name) and roles (key: `Protocol.role`), own and imported. Absent for
    /// declarations without type parameters.
    pub type_params: BTreeMap<String, Vec<String>>,
    /// Record name to its fields with their declared types, own and
    /// imported. Read for the effects of `Store` methods and of calls of
    /// fn-typed fields (`docs/slice.md` section 11.7).
    pub record_field_types: BTreeMap<String, Vec<(String, Type)>>,
    /// Alias name to the type it names, own and imported.
    pub alias_targets: BTreeMap<String, Type>,
    /// Spans of the `Store` method calls and calls of effectful fn values
    /// whose effects this module's check accounted for. The type checker
    /// reports E042 for such a call that is not listed.
    pub effect_sites: BTreeSet<Span>,
    /// Enum name to the payload types of all its variants, as declared,
    /// own and imported. Read for E055 (a type containing itself).
    pub enum_payload_types: BTreeMap<String, Vec<Type>>,
    /// Own test-only declarations (`test fn`, `test type`, `test enum`,
    /// `test const`) by name. They are not exported; only tests and other
    /// test-only items of the module may use them (E051, `docs/slice.md`
    /// section 12).
    pub test_only: BTreeSet<String>,
    /// Names two glob imports both provide, to the modules providing them
    /// (`docs/slice.md` section 13). Neither is imported; a use is E041.
    pub glob_conflicts: BTreeMap<String, Vec<String>>,
    /// Effect signatures of the effectful closures, by the span of the
    /// closure literal: its parameter names and every effect its body
    /// performs, on its parameters (`p.read`), on the roots of its
    /// captures and ambient ones (`docs/slice.md` section 13.3).
    pub closure_effects: BTreeMap<Span, EffectSig>,
    /// Own contracts (`docs/slice.md` section 14.7), `System.name` to the
    /// kind's name; kept for slice 11.
    pub contracts: BTreeMap<String, String>,
    /// Own policies, `System.name` to the kind's name; kept for slice 11.
    pub policies: BTreeMap<String, String>,
    /// Own assemblies, name to the kind's name; kept for slice 11.
    pub assemblies: BTreeMap<String, String>,
    /// Own systems, name to their member names (resources, components,
    /// contracts, policies) in declaration order.
    pub systems: BTreeMap<String, Vec<String>>,
    /// The declared return type of each non-generic fn, own and imported
    /// by name. Read only by the closure rule of E006 for a local bound to
    /// a call (`docs/slice.md` 20.3.3).
    pub fn_returns: BTreeMap<String, Type>,
}

/// Checked standard-library modules by module path (`"std.console"`), each
/// with the `Symbols` its own `check_module` returned.
pub type Modules = BTreeMap<String, Symbols>;

impl Symbols {
    /// The single enum owning a bare variant name, if unambiguous.
    pub fn enum_of_variant(&self, variant: &str) -> Option<&str> {
        match self.variants.get(variant) {
            Some(enums) if enums.len() == 1 => Some(enums[0].as_str()),
            _ => None,
        }
    }

    /// Whether a module with these symbols exports `name`: a record, enum,
    /// fn, const or protocol it defines itself (not a builtin, not an
    /// import, not test-only).
    pub fn exports(&self, name: &str) -> bool {
        let defined = self.records.contains(name)
            || self.enums.contains_key(name)
            || self.functions.contains_key(name)
            || self.constants.contains(name)
            || self.protocols.contains_key(name)
            || self.aliases.contains_key(name);
        defined
            && !self.imported.contains_key(name)
            && !is_builtin_enum(name)
            && !self.test_only.contains(name)
    }
}

/// Resolve names and check effects of a module that imports nothing beyond
/// the builtins: `check_module(program, &Modules::new())`.
pub fn check(program: &Program) -> Result<Symbols, Vec<Diagnostic>> {
    check_module(program, &Modules::new())
}

/// Resolve names against `modules` and check effects and protocol roles
/// (`docs/slice.md` sections 4 and 5). `check(program)` is
/// `check_module(program, &Modules::new())`.
pub fn check_module(program: &Program, modules: &Modules) -> Result<Symbols, Vec<Diagnostic>> {
    let mut checker = Checker::default();
    checker.builtins();
    checker.imports(program, modules);
    checker.collect(program);
    checker.const_variant_clashes(program);
    checker.recursive_types(program);
    checker.unused_type_params(program);
    let collected = checker.diagnostics.len();
    checker.infer_effects(program);
    checker.diagnostics.truncate(collected);
    loop {
        checker.changed = false;
        checker.check_program(program);
        if !checker.changed {
            break;
        }
        checker.diagnostics.truncate(collected);
    }
    checker.clauses(program, modules);
    checker.catalogue_literals(program, modules);
    if checker.diagnostics.is_empty() {
        Ok(checker.symbols)
    } else {
        let mut diagnostics = checker.diagnostics;
        diagnostics.sort_by_key(|d| (d.span.start, d.code));
        Err(diagnostics)
    }
}

/// How a local name was bound; only `Var` is assignable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Binding {
    Let,
    Var,
    Param,
    Pattern,
    Loop,
}

/// A local name: how it was bound and the parameters of the enclosing
/// function its value may come from (`docs/slice.md` section 5).
#[derive(Clone, Debug)]
struct Local {
    binding: Binding,
    roots: BTreeSet<String>,
    /// The declared type of the value, where the checker can tell it
    /// (`Checker::static_type`); `None` otherwise.
    ty: Option<Type>,
    /// Where `ty` is `None` and the value is a non-generic record literal
    /// or a call of a non-generic fn: that record, or the fn's declared
    /// return type. Read only by the closure rule of E006
    /// (`Checker::in_place_type`), so that no other rule changes.
    result: Option<Type>,
}

/// What the walk is inside, for the effect rules.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Ctx {
    /// Nothing that holds code (types, between declarations).
    #[default]
    None,
    /// A fn or role with a `requires` clause: effects are checked (E016).
    Declared,
    /// A fn without `requires`: effects are collected.
    Inferred,
    /// The value of the named constant: pure (E010).
    Const(String),
    /// The default of the named field `Record.field`: pure (E010).
    Default(String),
}

#[derive(Default)]
struct Checker {
    symbols: Symbols,
    diagnostics: Vec<Diagnostic>,
    scopes: Vec<BTreeMap<String, Local>>,
    ctx: Ctx,
    /// The `requires` entries of the function being walked.
    requires: BTreeSet<String>,
    /// The name of the function being walked, `Protocol.role` for a role.
    fn_name: String,
    /// Effects the inferred fn being walked needs so far.
    inferred: BTreeSet<String>,
    /// Whether the inferred fn being walked performs an effectful operation.
    performs: bool,
    /// Whether this pass changed an inferred effect signature.
    changed: bool,
    /// Set while walking the operand of a `spawn` already reported as E010.
    quiet: bool,
    /// Constants named by the constant value being walked.
    const_refs: BTreeSet<String>,
    /// Parameters of the type alias being checked.
    type_params: BTreeSet<String>,
    /// The type parameters of the fn or role being walked that are bounded
    /// by `ByteStream` (`docs/slice.md` 18.3): a receiver of such a declared
    /// type takes the stream methods as builtins, as a `TcpStream` does.
    bounded: BTreeSet<String>,
    /// Variant names of the enums the given modules define, in scope or
    /// not. A bare one that is not in scope is left to the type checker,
    /// which resolves it against the expected type.
    module_variants: BTreeSet<String>,
    /// Roots of the value of each block and `match` arm body walked so
    /// far (key: its address), taken while its scope was open.
    tail_roots: BTreeMap<usize, BTreeSet<String>>,
    /// Whether the walk is inside a test or a test-only declaration, where
    /// test-only items are in scope.
    in_test: bool,
    /// For each closure being walked, innermost last: the number of scopes
    /// open outside it. A binding of a scope below that is a capture.
    closure_scopes: Vec<usize>,
    /// The span of the closure literal that is the argument of the
    /// `List.map` call being walked, set until that closure is entered,
    /// with the list's element type where `static_type` knows it.
    map_argument: Option<(Span, Option<Type>)>,
}

fn is_static_owner(name: &str) -> bool {
    BUILTIN_STATICS.iter().any(|(owner, _)| *owner == name)
}

fn static_arity(ty: &str, name: &str) -> Option<usize> {
    BUILTIN_STATIC_ARITIES
        .iter()
        .find(|(t, n, _)| *t == ty && *n == name)
        .map(|(_, _, arity)| *arity)
}

/// The number of type arguments a builtin type takes.
fn type_arity(name: &str) -> usize {
    match name {
        "List" | "Option" | "Store" | "Signal" | "Module" => 1,
        "Result" | "Map" => 2,
        _ => 0,
    }
}

fn is_effectful_static(ty: &str, name: &str) -> bool {
    EFFECTFUL_STATICS.contains(&(ty, name))
}

fn is_builtin_variant(name: &str) -> bool {
    BUILTIN_ENUMS
        .iter()
        .any(|(_, variants)| variants.contains(&name))
}

fn is_builtin_enum(name: &str) -> bool {
    BUILTIN_ENUMS.iter().any(|(e, _)| *e == name)
}

/// A name a substrate module (`std.net`, `std.io`) provides.
fn is_builtin_name(name: &str) -> bool {
    BUILTIN_TYPES.contains(&name)
        || is_builtin_variant(name)
        || BUILTIN_FUNCTIONS.contains(&name)
        || BUILTIN_INTERFACES.contains(&name)
        || name == "Pipe"
}

fn plural(count: usize, word: &str) -> String {
    if count == 1 {
        format!("{count} {word}")
    } else {
        format!("{count} {word}s")
    }
}

fn quoted_list(items: &[String]) -> String {
    items
        .iter()
        .map(|o| format!("`{o}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

impl Checker {
    fn error(&mut self, code: &'static str, span: Span, message: String) {
        self.diagnostics.push(Diagnostic::new(code, span, message));
    }

    // ---- collection -------------------------------------------------------

    fn builtins(&mut self) {
        for (enum_name, variants) in BUILTIN_ENUMS {
            let payloads = variants
                .iter()
                .map(|variant| {
                    let payload = BUILTIN_VARIANT_PAYLOADS
                        .iter()
                        .find(|(v, _)| v == variant)
                        .map_or(0, |(_, n)| *n);
                    (variant.to_string(), payload)
                })
                .collect();
            self.add_enum(
                enum_name,
                variants.iter().map(|v| v.to_string()).collect(),
                payloads,
            );
        }
    }

    fn add_enum(&mut self, name: &str, variants: Vec<String>, payloads: BTreeMap<String, usize>) {
        for variant in &variants {
            self.symbols
                .variants
                .entry(variant.clone())
                .or_default()
                .push(name.to_string());
        }
        self.symbols.enums.insert(name.to_string(), variants);
        self.symbols.payloads.insert(name.to_string(), payloads);
    }

    /// Resolve every import and bring the imported names into scope.
    fn imports(&mut self, program: &Program, modules: &Modules) {
        for symbols in modules.values() {
            for (name, variants) in &symbols.enums {
                if symbols.exports(name) {
                    self.module_variants.extend(variants.iter().cloned());
                }
            }
        }
        for import in program.imports.iter().filter(|i| !i.is_glob()) {
            self.import(import, modules);
        }
        self.glob_imports(program, modules);
    }

    /// `import a.b.*`: every export of the module that is not test-only,
    /// unless the module defines or explicitly imports a name of that name;
    /// a name two globs provide is imported from neither and recorded in
    /// [`Symbols::glob_conflicts`].
    fn glob_imports(&mut self, program: &Program, modules: &Modules) {
        let local: BTreeSet<&str> = program
            .decls
            .iter()
            .filter(|d| !matches!(d, Decl::Test(_)))
            .map(|d| d.name().name.as_str())
            .collect();
        let mut offered: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for import in program.imports.iter().filter(|i| i.is_glob()) {
            let module = import.module();
            if module.is_empty() || !module.starts_with("std") {
                self.error(
                    "E031",
                    import.path.span,
                    format!("cannot import from `{module}`: only standard-library (`std.*`) modules can be imported in this slice"),
                );
                continue;
            }
            if SUBSTRATE_MODULES.contains(&module.as_str()) {
                continue;
            }
            let Some(exports) = modules.get(&module) else {
                self.error(
                    "E031",
                    import.path.span,
                    format!("unknown module `{module}`"),
                );
                continue;
            };
            let names: BTreeSet<&String> = exports
                .records
                .iter()
                .chain(exports.enums.keys())
                .chain(exports.functions.keys())
                .chain(exports.constants.iter())
                .chain(exports.protocols.keys())
                .chain(exports.aliases.keys())
                .filter(|n| !n.contains('.') && exports.exports(n))
                .collect();
            for name in names {
                let modules = offered.entry(name.clone()).or_default();
                if !modules.contains(&module) {
                    modules.push(module.clone());
                }
            }
        }
        for (name, from) in offered {
            if local.contains(name.as_str()) || self.symbols.imported.contains_key(&name) {
                continue;
            }
            if from.len() > 1 {
                self.symbols.glob_conflicts.insert(name, from);
                continue;
            }
            if let Some(exports) = modules.get(&from[0]) {
                self.import_name(&name, &from[0], exports);
            }
        }
    }

    /// E041 for a use of a name two glob imports provide; `false` when
    /// `name` is no such name.
    fn glob_conflict(&mut self, name: &Ident) -> bool {
        let Some(from) = self.symbols.glob_conflicts.get(&name.name) else {
            return false;
        };
        let message = format!(
            "`{}` is provided by the glob imports of {}; import it explicitly",
            name.name,
            quoted_list(from)
        );
        self.error("E041", name.span, message);
        true
    }

    fn import(&mut self, import: &Import, modules: &Modules) {
        let segments = &import.path.segments;
        let (module_segments, names): (&[Ident], Vec<&Ident>) = if import.names.is_empty() {
            (&segments[..segments.len() - 1], vec![import.path.last()])
        } else {
            (&segments[..], import.names.iter().collect())
        };
        let module = module_segments
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(".");
        if module_segments.is_empty() {
            self.error(
                "E031",
                import.path.span,
                format!(
                    "`import {}` names no module; import names from a module, e.g. `import std.net.{{Endpoint}}`",
                    import.path.dotted()
                ),
            );
            return;
        }
        if module_segments[0].name != "std" && !modules.contains_key(&module) {
            self.error(
                "E031",
                import.path.span,
                format!(
                    "cannot import from `{module}`: only standard-library (`std.*`) modules and the sibling modules of the program's package can be imported"
                ),
            );
            return;
        }
        if SUBSTRATE_MODULES.contains(&module.as_str()) {
            for name in names {
                if is_builtin_name(&name.name) {
                    continue;
                }
                match modules.get(&module) {
                    Some(exports) if exports.exports(&name.name) => {
                        if let Some(previous) = self.symbols.imported.get(&name.name) {
                            let message =
                                format!("`{}` is already imported from `{previous}`", name.name);
                            self.error("E004", name.span, message);
                            continue;
                        }
                        self.import_name(&name.name, &module, exports);
                    }
                    _ => self.error(
                        "E031",
                        name.span,
                        format!("`{module}` has no builtin `{}`", name.name),
                    ),
                }
            }
            return;
        }
        let Some(exports) = modules.get(&module) else {
            self.error(
                "E031",
                import.path.span,
                format!("unknown module `{module}`"),
            );
            return;
        };
        for name in names {
            if exports.test_only.contains(&name.name) {
                let message = format!(
                    "`{}` is test-only in `{module}`; test-only items are not exported",
                    name.name
                );
                self.error("E051", name.span, message);
                continue;
            }
            if !exports.exports(&name.name) {
                let message = match exports.imported.get(&name.name) {
                    Some(origin) => format!(
                        "module `{module}` does not export `{}`; imports are not re-exported, import it from `{origin}`",
                        name.name
                    ),
                    None => format!("module `{module}` has no `{}`", name.name),
                };
                self.error("E031", name.span, message);
                continue;
            }
            if let Some(previous) = self.symbols.imported.get(&name.name) {
                let message = format!("`{}` is already imported from `{previous}`", name.name);
                self.error("E004", name.span, message);
                continue;
            }
            self.import_name(&name.name, &module, exports);
        }
    }

    /// Copy one exported definition of `exports` into this module's symbols.
    fn import_name(&mut self, name: &str, module: &str, exports: &Symbols) {
        if exports.records.contains(name) {
            self.symbols.records.insert(name.to_string());
            let fields = exports.record_fields.get(name).cloned().unwrap_or_default();
            self.symbols.record_fields.insert(name.to_string(), fields);
            if let Some(defaults) = exports.record_defaults.get(name) {
                self.symbols
                    .record_defaults
                    .insert(name.to_string(), defaults.clone());
            }
        }
        if let Some(arity) = exports.aliases.get(name) {
            self.symbols.aliases.insert(name.to_string(), *arity);
        }
        if let Some(variants) = exports.enums.get(name) {
            let payloads = exports.payloads.get(name).cloned().unwrap_or_default();
            self.add_enum(name, variants.clone(), payloads);
            if let Some(types) = exports.enum_payload_types.get(name) {
                self.symbols
                    .enum_payload_types
                    .insert(name.to_string(), types.clone());
            }
        }
        if let Some(info) = exports.functions.get(name) {
            self.symbols
                .functions
                .insert(name.to_string(), info.clone());
        }
        if let Some(ret) = exports.fn_returns.get(name) {
            self.symbols
                .fn_returns
                .insert(name.to_string(), ret.clone());
        }
        if let Some(roles) = exports.protocols.get(name) {
            self.symbols
                .protocols
                .insert(name.to_string(), roles.clone());
            for role in roles.keys() {
                let key = format!("{name}.{role}");
                if let Some(sig) = exports.effects.get(&key) {
                    self.symbols.effects.insert(key, sig.clone());
                }
            }
        }
        if exports.functions.contains_key(name)
            && let Some(sig) = exports.effects.get(name)
        {
            self.symbols.effects.insert(name.to_string(), sig.clone());
        }
        let prefix = format!("{name}.");
        for (key, info) in &exports.functions {
            if key.starts_with(&prefix) && !exports.test_only.contains(key) {
                self.symbols.functions.insert(key.clone(), info.clone());
                if let Some(sig) = exports.effects.get(key) {
                    self.symbols.effects.insert(key.clone(), sig.clone());
                }
                if let Some(params) = exports.type_params.get(key) {
                    self.symbols.type_params.insert(key.clone(), params.clone());
                }
            }
        }
        if exports.constants.contains(name) {
            self.symbols.constants.insert(name.to_string());
        }
        if let Some(params) = exports.type_params.get(name) {
            self.symbols
                .type_params
                .insert(name.to_string(), params.clone());
        }
        if let Some(fields) = exports.record_field_types.get(name) {
            self.symbols
                .record_field_types
                .insert(name.to_string(), fields.clone());
        }
        if let Some(target) = exports.alias_targets.get(name) {
            self.symbols
                .alias_targets
                .insert(name.to_string(), target.clone());
        }
        if let Some(roles) = exports.protocols.get(name) {
            for role in roles.keys() {
                let key = format!("{name}.{role}");
                if let Some(params) = exports.type_params.get(&key) {
                    self.symbols.type_params.insert(key, params.clone());
                }
            }
        }
        self.symbols
            .imported
            .insert(name.to_string(), module.to_string());
    }

    fn collect(&mut self, program: &Program) {
        let mut declared: BTreeSet<&str> = BTreeSet::new();
        let mut owned: Vec<&FnDecl> = Vec::new();
        for decl in &program.decls {
            if let Decl::Test(_) = decl {
                continue;
            }
            if let Decl::Fn(f) = decl
                && f.owner.is_some()
            {
                self.associated_fn(f, &mut owned);
                continue;
            }
            let name = decl.name();
            if BUILTIN_TYPES.contains(&name.name.as_str())
                || BUILTIN_FUNCTIONS.contains(&name.name.as_str())
                || BUILTIN_INTERFACES.contains(&name.name.as_str())
                || (is_static_owner(&name.name)
                    && !STD_RECORD_STATIC_OWNERS.contains(&name.name.as_str()))
                || name.name == "Tuple"
            {
                self.error(
                    "E004",
                    name.span,
                    format!("duplicate definition of builtin `{}`", name.name),
                );
                continue;
            }
            if let Some(module) = self.symbols.imported.get(&name.name) {
                let message = format!(
                    "duplicate definition of `{}`, which is imported from `{module}`",
                    name.name
                );
                self.error("E004", name.span, message);
                continue;
            }
            if !declared.insert(name.name.as_str()) {
                self.error(
                    "E004",
                    name.span,
                    format!("duplicate definition of `{}`", name.name),
                );
                continue;
            }
            if decl.is_test_only() {
                self.symbols.test_only.insert(name.name.clone());
            }
            match decl {
                Decl::Alias(alias) => {
                    self.symbols
                        .aliases
                        .insert(name.name.clone(), alias.params.len());
                    self.type_params_of(&name.name, &alias.params);
                    self.symbols
                        .alias_targets
                        .insert(name.name.clone(), alias.ty.clone());
                }
                Decl::Record(record) => {
                    self.symbols.records.insert(name.name.clone());
                    self.type_params_of(&name.name, &record.type_params);
                    self.symbols.record_field_types.insert(
                        name.name.clone(),
                        record
                            .fields
                            .iter()
                            .map(|f| (f.name.name.clone(), f.ty.clone()))
                            .collect(),
                    );
                    let defaults: BTreeSet<String> = record
                        .fields
                        .iter()
                        .filter(|f| f.default.is_some())
                        .map(|f| f.name.name.clone())
                        .collect();
                    if !defaults.is_empty() {
                        self.symbols
                            .record_defaults
                            .insert(name.name.clone(), defaults);
                    }
                    let mut fields: Vec<String> = Vec::new();
                    for field in &record.fields {
                        if fields.contains(&field.name.name) {
                            self.error(
                                "E004",
                                field.name.span,
                                format!(
                                    "duplicate field `{}` in record `{}`",
                                    field.name.name, name.name
                                ),
                            );
                            continue;
                        }
                        fields.push(field.name.name.clone());
                    }
                    self.symbols.record_fields.insert(name.name.clone(), fields);
                }
                Decl::Enum(decl) => {
                    self.type_params_of(&name.name, &decl.type_params);
                    let mut variants: Vec<String> = Vec::new();
                    let mut payloads = BTreeMap::new();
                    for variant in &decl.variants {
                        if variants.contains(&variant.name.name) {
                            self.error(
                                "E004",
                                variant.name.span,
                                format!(
                                    "duplicate variant `{}` in enum `{}`",
                                    variant.name.name, name.name
                                ),
                            );
                            continue;
                        }
                        variants.push(variant.name.name.clone());
                        payloads.insert(variant.name.name.clone(), variant.payload.len());
                    }
                    self.add_enum(&name.name, variants, payloads);
                    self.symbols.enum_payload_types.insert(
                        name.name.clone(),
                        decl.variants
                            .iter()
                            .flat_map(|v| v.payload.iter().cloned())
                            .collect(),
                    );
                }
                Decl::Fn(decl) => {
                    self.type_params_of(&name.name, &decl.type_params);
                    if decl.type_params.is_empty()
                        && let Some(ret) = &decl.ret
                    {
                        self.symbols
                            .fn_returns
                            .insert(name.name.clone(), ret.clone());
                    }
                    self.symbols.functions.insert(
                        name.name.clone(),
                        FnInfo {
                            effectful: decl.is_effectful(),
                            arity: decl.params.len(),
                        },
                    );
                    if decl.requires.is_some() {
                        self.symbols
                            .effects
                            .insert(name.name.clone(), EffectSig::of(decl));
                    }
                }
                Decl::System(system) => self.collect_system(system),
                Decl::Assembly(assembly) => {
                    self.symbols
                        .assemblies
                        .insert(name.name.clone(), assembly.kind.name.clone());
                }
                Decl::Test(_) => {}
                Decl::Const(_) => {
                    self.symbols.constants.insert(name.name.clone());
                }
                Decl::Protocol(protocol) => {
                    let roles = self.protocol_roles(protocol);
                    self.symbols.protocols.insert(name.name.clone(), roles);
                }
            }
        }
        for f in owned {
            self.associated_owner(f, program);
        }
    }

    /// Collect `fn Owner.name(..)` under the key `Owner.name`: E004 when
    /// declared twice.
    fn associated_fn<'p>(&mut self, f: &'p FnDecl, owned: &mut Vec<&'p FnDecl>) {
        let key = f.qualified_name();
        if self.symbols.functions.contains_key(&key) {
            self.error(
                "E004",
                f.name.span,
                format!("duplicate definition of `{key}`"),
            );
            return;
        }
        if f.test_only {
            self.symbols.test_only.insert(key.clone());
        }
        self.type_params_of(&key, &f.type_params);
        self.symbols.functions.insert(
            key.clone(),
            FnInfo {
                effectful: f.is_effectful(),
                arity: f.params.len(),
            },
        );
        if f.requires.is_some() {
            self.symbols.effects.insert(key, EffectSig::of(f));
        }
        owned.push(f);
    }

    /// The owner of an associated fn must be a record or enum this module
    /// declares (E003), without a variant of the fn's name (E004).
    fn associated_owner(&mut self, f: &FnDecl, program: &Program) {
        let Some(owner) = &f.owner else {
            return;
        };
        let declared = program
            .decls
            .iter()
            .any(|d| matches!(d, Decl::Record(_) | Decl::Enum(_)) && d.name().name == owner.name);
        if !declared {
            let message = match self.symbols.imported.get(&owner.name) {
                Some(module) => format!(
                    "associated fn `{}` must be declared in `{module}`, the module that declares `{}`",
                    f.qualified_name(),
                    owner.name
                ),
                None => format!(
                    "`{}` is no record or enum of this module; an associated fn needs one",
                    owner.name
                ),
            };
            self.error("E003", owner.span, message);
            return;
        }
        if self
            .symbols
            .enums
            .get(&owner.name)
            .is_some_and(|variants| variants.contains(&f.name.name))
        {
            self.error(
                "E004",
                f.name.span,
                format!(
                    "`{}` is both a variant and an associated fn of `{}`",
                    f.name.name, owner.name
                ),
            );
        }
    }

    /// `E004` for a constant, own or imported, named like a variant in scope.
    /// Runs after `collect`, so own enums declared later count.
    fn const_variant_clashes(&mut self, program: &Program) {
        let mut names: Vec<&Ident> = program
            .imports
            .iter()
            .flat_map(|import| {
                if import.names.is_empty() {
                    vec![import.path.last()]
                } else {
                    import.names.iter().collect()
                }
            })
            .collect();
        names.extend(program.decls.iter().filter_map(|decl| match decl {
            Decl::Const(c) => Some(&c.name),
            _ => None,
        }));
        for name in names {
            if !self.symbols.constants.contains(&name.name) {
                continue;
            }
            if let Some(owners) = self.symbols.variants.get(&name.name) {
                let message = format!(
                    "constant `{}` has the name of a variant of {}",
                    name.name,
                    quoted_list(owners)
                );
                self.error("E004", name.span, message);
            }
        }
    }

    /// `E057` for a type parameter of a generic record, enum or alias that
    /// no field, payload or aliased type uses: no value fixes it, and the
    /// Rust target rejects it (`docs/slice.md` section 13.11).
    fn unused_type_params(&mut self, program: &Program) {
        fn uses(ty: &Type, param: &str) -> bool {
            ty.name.name == param || ty.args.iter().any(|arg| uses(arg, param))
        }
        for decl in &program.decls {
            let (name, params, types): (&Ident, &[Ident], Vec<&Type>) = match decl {
                Decl::Record(r) => (
                    &r.name,
                    &r.type_params,
                    r.fields.iter().map(|f| &f.ty).collect(),
                ),
                Decl::Enum(e) => (
                    &e.name,
                    &e.type_params,
                    e.variants.iter().flat_map(|v| &v.payload).collect(),
                ),
                Decl::Alias(a) => (&a.name, &a.params, vec![&a.ty]),
                _ => continue,
            };
            for param in params {
                if !types.iter().any(|ty| uses(ty, &param.name)) {
                    self.error(
                        "E057",
                        param.span,
                        format!(
                            "type parameter `{}` of `{}` is never used; remove it or use it in a field",
                            param.name, name.name
                        ),
                    );
                }
            }
        }
    }

    /// `E055` for an own record that holds a value of itself inline through
    /// fields and tuples alone (type arguments substituted), with no enum,
    /// `Option`, `Result`, `List`, `Store` or fn type in between: such a
    /// value would be infinite. Recursion through an enum payload is
    /// allowed and boxed (`docs/slice.md` section 13.4).
    fn recursive_types(&mut self, program: &Program) {
        for decl in &program.decls {
            let (name, params) = match decl {
                Decl::Record(r) => (&r.name, &r.type_params),
                _ => continue,
            };
            let args = params
                .iter()
                .map(|p| Type::named(p.clone(), Vec::new(), p.span))
                .collect();
            let start = Type::named(name.clone(), args, name.span);
            if self.contains_itself(&name.name, &start) {
                let message = format!(
                    "record `{0}` contains itself through fields and tuples alone, so a value would be infinite; put the nested `{0}` in an `Option`, an enum payload or a `List`",
                    name.name
                );
                self.error("E055", name.span, message);
            }
        }
    }

    /// Whether a value of `start` holds a value of the record or enum
    /// `target` inline, at any depth.
    fn contains_itself(&self, target: &str, start: &Type) -> bool {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut queue = self.inline_parts(start);
        while let Some(ty) = queue.pop() {
            if ty.name.name == target {
                return true;
            }
            if seen.len() > 4096 || !seen.insert(type_key(&ty)) {
                continue;
            }
            queue.extend(self.inline_parts(&ty));
        }
        false
    }

    /// The user records and enums a value of the user record or enum `ty`
    /// holds inline in its fields or payloads, with `ty`'s type arguments
    /// substituted.
    fn inline_parts(&self, ty: &Type) -> Vec<Type> {
        let name = &ty.name.name;
        let params = self
            .symbols
            .type_params
            .get(name)
            .cloned()
            .unwrap_or_default();
        let members: Vec<Type> = match self.symbols.record_field_types.get(name) {
            Some(fields) => fields.iter().map(|(_, t)| t.clone()).collect(),
            None => self
                .symbols
                .enum_payload_types
                .get(name)
                .cloned()
                .unwrap_or_default(),
        };
        let mut out = Vec::new();
        for member in &members {
            self.inline_types(&substitute_type(member, &params, &ty.args), &mut out);
        }
        out
    }

    /// The user records `ty` holds inline: itself when it is one, else
    /// those of the elements of a tuple; nothing under an enum (`Option`
    /// and `Result` included), `List`, `Store` and fn types.
    fn inline_types(&self, ty: &Type, out: &mut Vec<Type>) {
        if ty.is_fn() {
            return;
        }
        let ty = self.expand(ty);
        let name = ty.name.name.as_str();
        if ty.is_fn() || !(ty.is_tuple() || self.symbols.records.contains(name)) {
            return;
        }
        if self.symbols.records.contains(name) {
            out.push(ty);
            return;
        }
        for arg in &ty.args {
            self.inline_types(arg, out);
        }
    }

    /// Validate the roles of a protocol (`E030`, `E004`) and return those
    /// with a role name.
    fn protocol_roles(&mut self, protocol: &ProtocolDecl) -> BTreeMap<String, FnInfo> {
        let name = &protocol.name.name;
        if protocol.fns.is_empty() {
            self.error(
                "E030",
                protocol.name.span,
                format!("protocol `{name}` declares no role; declare `serve` or `connect`"),
            );
        }
        let mut roles = BTreeMap::new();
        for f in &protocol.fns {
            let role = &f.name.name;
            let Some((_, expected)) = PROTOCOL_ROLES.iter().find(|(r, _)| r == role) else {
                self.error(
                    "E030",
                    f.name.span,
                    format!(
                        "`{role}` is not a protocol role; the roles are `serve` and `connect`, declare helpers as module-level functions"
                    ),
                );
                continue;
            };
            if roles.contains_key(role) {
                self.error(
                    "E004",
                    f.name.span,
                    format!("duplicate role `{role}` in protocol `{name}`"),
                );
                continue;
            }
            if !f.is_effectful() {
                self.error(
                    "E030",
                    f.name.span,
                    format!("role `{name}.{role}` must be effectful; add a `requires` clause"),
                );
            }
            let found = f.params.len();
            if role == "serve" && !SERVE_ARITIES.contains(&found) {
                self.error(
                    "E030",
                    f.name.span,
                    format!(
                        "role `{name}.serve` takes 2 parameters (stream, application) or 3 (stream, limits, application), found {found}"
                    ),
                );
            } else if role != "serve" && found != *expected {
                self.error(
                    "E030",
                    f.name.span,
                    format!(
                        "role `{name}.{role}` takes {} (stream), found {found}",
                        plural(*expected, "parameter"),
                    ),
                );
            }
            roles.insert(
                role.clone(),
                FnInfo {
                    effectful: true,
                    arity: f.params.len(),
                },
            );
            self.symbols
                .effects
                .insert(format!("{name}.{role}"), EffectSig::of(f));
            self.type_params_of(&format!("{name}.{role}"), &f.type_params);
        }
        roles
    }

    /// Record the type parameters of a generic declaration (`E004` for one
    /// named twice).
    fn type_params_of(&mut self, key: &str, params: &[Ident]) {
        if params.is_empty() {
            return;
        }
        let mut names: Vec<String> = Vec::new();
        for param in params {
            if names.contains(&param.name) {
                self.error(
                    "E004",
                    param.span,
                    format!("duplicate type parameter `{}` of `{key}`", param.name),
                );
                continue;
            }
            names.push(param.name.clone());
        }
        self.symbols.type_params.insert(key.to_string(), names);
    }

    // ---- program ----------------------------------------------------------

    fn check_program(&mut self, program: &Program) {
        let mut const_graph: BTreeMap<String, (Span, BTreeSet<String>)> = BTreeMap::new();
        for decl in &program.decls {
            self.in_test = decl.is_test_only();
            match decl {
                Decl::Test(test) => self.check_test(test),
                Decl::Alias(alias) => {
                    self.type_params = alias.params.iter().map(|p| p.name.clone()).collect();
                    self.check_type(&alias.ty);
                    self.type_params.clear();
                }
                Decl::Record(record) => {
                    self.type_params = record.type_params.iter().map(|p| p.name.clone()).collect();
                    for field in &record.fields {
                        self.check_type(&field.ty);
                    }
                    self.type_params.clear();
                    for field in &record.fields {
                        if let Some(default) = &field.default {
                            let name = format!("{}.{}", record.name.name, field.name.name);
                            self.pure_value(Ctx::Default(name), default);
                        }
                    }
                }
                Decl::Enum(decl) => {
                    self.type_params = decl.type_params.iter().map(|p| p.name.clone()).collect();
                    for variant in &decl.variants {
                        for ty in &variant.payload {
                            self.check_type(ty);
                        }
                    }
                    self.type_params.clear();
                }
                Decl::Fn(decl) => self.check_fn(decl, decl.qualified_name()),
                Decl::Const(decl) => {
                    self.check_type(&decl.ty);
                    self.pure_value(Ctx::Const(decl.name.name.clone()), &decl.value);
                    let refs = std::mem::take(&mut self.const_refs);
                    const_graph
                        .entry(decl.name.name.clone())
                        .or_insert((decl.name.span, refs));
                }
                Decl::Protocol(protocol) => {
                    for f in &protocol.fns {
                        self.check_fn(f, format!("{}.{}", protocol.name.name, f.name.name));
                    }
                }
                Decl::System(_) | Decl::Assembly(_) => {}
            }
        }
        self.in_test = false;
        self.const_cycles(&const_graph);
        self.test_names(program);
    }

    /// `E050` for a test whose name an earlier test of the module has.
    fn test_names(&mut self, program: &Program) {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for decl in &program.decls {
            if let Decl::Test(test) = decl
                && !seen.insert(test.name.name.as_str())
            {
                let message = format!(
                    "duplicate test name \"{}\"; test names are unique within a module",
                    test.name.name
                );
                self.error("E050", test.name.span, message);
            }
        }
    }

    /// A test body: an effect root with an inferred set that nothing
    /// checks (`docs/slice.md` section 12), so no `E016`; `spawn` and IO
    /// are allowed, and test-only items are in scope.
    fn check_test(&mut self, test: &TestDecl) {
        self.ctx = Ctx::Inferred;
        self.requires.clear();
        self.inferred.clear();
        self.performs = false;
        self.fn_name = format!("test \"{}\"", test.name.name);
        self.scopes.push(BTreeMap::new());
        self.block(&test.body);
        self.scopes.pop();
        self.inferred.clear();
        self.performs = false;
        self.ctx = Ctx::None;
    }

    /// `E051` when code that is not a test or test-only item names the
    /// own test-only declaration `name`.
    fn test_only_use(&mut self, name: &Ident) {
        if !self.in_test && self.symbols.test_only.contains(&name.name) {
            let message = format!(
                "`{}` is test-only; only tests and other test-only items can use it",
                name.name
            );
            self.error("E051", name.span, message);
        }
    }

    /// `E051` for a bare variant whose every owning enum is test-only.
    fn test_only_variant(&mut self, name: &Ident) {
        if self.in_test {
            return;
        }
        let owners = self.symbols.variants.get(&name.name);
        if let Some(owners) = owners
            && !owners.is_empty()
            && owners.iter().all(|o| self.symbols.test_only.contains(o))
        {
            let message = format!(
                "variant `{}` belongs to test-only enum `{}`; only tests and other test-only items can use it",
                name.name, owners[0]
            );
            self.error("E051", name.span, message);
        }
    }

    /// Record a system's members, contracts and policies; a member name
    /// declared twice in one system is `E004`.
    fn collect_system(&mut self, system: &SystemDecl) {
        let mut members: Vec<String> = Vec::new();
        let names = system
            .resources
            .iter()
            .map(|r| &r.name)
            .chain(system.components.iter().map(|c| &c.name))
            .chain(system.contracts.iter().map(|c| &c.name))
            .chain(system.policies.iter().map(|c| &c.name));
        for name in names {
            if members.contains(&name.name) {
                let message = format!(
                    "duplicate member `{}` in system `{}`",
                    name.name, system.name.name
                );
                self.error("E004", name.span, message);
                continue;
            }
            members.push(name.name.clone());
        }
        for contract in &system.contracts {
            self.symbols.contracts.insert(
                format!("{}.{}", system.name.name, contract.name.name),
                contract.kind.name.clone(),
            );
        }
        for policy in &system.policies {
            self.symbols.policies.insert(
                format!("{}.{}", system.name.name, policy.name.name),
                policy.kind.name.clone(),
            );
        }
        self.symbols
            .systems
            .insert(system.name.name.clone(), members);
    }
    /// `E015` at every own constant whose value reaches itself through
    /// direct references to other constants. References through function
    /// bodies are not followed: a function may use a constant only on some
    /// paths.
    fn const_cycles(&mut self, graph: &BTreeMap<String, (Span, BTreeSet<String>)>) {
        for (name, (span, _)) in graph {
            let mut parent: BTreeMap<&str, &str> = BTreeMap::new();
            let mut queue = std::collections::VecDeque::from([name.as_str()]);
            let mut found = false;
            while let Some(current) = queue.pop_front() {
                let Some((_, refs)) = graph.get(current) else {
                    continue;
                };
                for next in refs {
                    if next == name {
                        parent.insert(name.as_str(), current);
                        found = true;
                        break;
                    }
                    if !parent.contains_key(next.as_str()) {
                        parent.insert(next.as_str(), current);
                        queue.push_back(next.as_str());
                    }
                }
                if found {
                    break;
                }
            }
            if !found {
                continue;
            }
            let mut between = Vec::new();
            let mut current = parent[name.as_str()];
            while current != name {
                between.push(current);
                current = parent[current];
            }
            between.reverse();
            let mut chain = vec![name.as_str()];
            chain.extend(between);
            chain.push(name.as_str());
            let message = format!(
                "constant `{name}` refers to itself: {}",
                chain
                    .iter()
                    .map(|c| format!("`{c}`"))
                    .collect::<Vec<_>>()
                    .join(" -> ")
            );
            self.error("E015", *span, message);
        }
    }

    fn check_fn(&mut self, decl: &FnDecl, display: String) {
        self.in_test = decl.test_only;
        self.type_params = decl.type_params.iter().map(|p| p.name.clone()).collect();
        self.bounds(decl);
        for param in &decl.params {
            self.ambient_name(&param.name);
            self.check_type(&param.ty);
        }
        if let Some(ret) = &decl.ret {
            self.check_type(ret);
        }
        let role = decl.owner.is_none() && display.contains('.');
        let infer = decl.requires.is_none() && !role;
        self.ctx = if infer { Ctx::Inferred } else { Ctx::Declared };
        self.requires = EffectSig::of(decl).requires.into_iter().collect();
        self.inferred.clear();
        self.performs = false;
        self.fn_name = display;
        self.scopes.push(BTreeMap::new());
        for param in &decl.params {
            let roots = BTreeSet::from([param.name.name.clone()]);
            self.bind_typed(&param.name, Binding::Param, roots, Some(param.ty.clone()));
        }
        self.block(&decl.body);
        self.scopes.pop();
        if infer {
            self.update_inferred(decl);
        }
        self.ctx = Ctx::None;
        self.requires.clear();
        self.type_params.clear();
        self.bounded.clear();
        self.in_test = false;
    }

    /// E099 for a bound that names an interface other than `ByteStream`
    /// or more than one interface (`docs/slice.md` 18.7); sets `bounded`
    /// to the parameters bounded by `ByteStream`.
    fn bounds(&mut self, decl: &FnDecl) {
        self.bounded.clear();
        for bound in &decl.bounds {
            let param = &bound.param.name;
            match bound.interfaces.as_slice() {
                [interface] if BUILTIN_INTERFACES.contains(&interface.name.as_str()) => {
                    self.bounded.insert(param.clone());
                }
                [interface] => self.error(
                    "E099",
                    interface.span,
                    format!(
                        "unknown interface `{}`; the only interface is `ByteStream`",
                        interface.name
                    ),
                ),
                [_, second, ..] => self.error(
                    "E099",
                    second.span,
                    format!("`{param}` has more than one bound; only `ByteStream` exists"),
                ),
                [] => {}
            }
        }
    }

    /// Settle the effect sets of the fns without `requires` before the
    /// checking walk: the strongly connected components of the call graph,
    /// callees first, each walked until its sets stop growing. So every
    /// fn is walked about twice whatever the declaration order.
    fn infer_effects(&mut self, program: &Program) {
        let mut fns: Vec<&FnDecl> = Vec::new();
        let mut index: BTreeMap<String, usize> = BTreeMap::new();
        for decl in &program.decls {
            if let Decl::Fn(f) = decl
                && f.requires.is_none()
                && self.symbols.functions.contains_key(&f.qualified_name())
                && !index.contains_key(&f.qualified_name())
            {
                index.insert(f.qualified_name(), fns.len());
                fns.push(f);
            }
        }
        let edges: Vec<Vec<usize>> = fns
            .iter()
            .map(|f| {
                called_names(&f.body)
                    .iter()
                    .filter_map(|name| index.get(name.as_str()).copied())
                    .collect()
            })
            .collect();
        for component in components(&edges) {
            let cyclic = component.len() > 1 || edges[component[0]].contains(&component[0]);
            loop {
                self.changed = false;
                for &i in &component {
                    self.check_fn(fns[i], fns[i].qualified_name());
                }
                if !cyclic || !self.changed {
                    break;
                }
            }
        }
        self.changed = false;
    }

    /// Record what the walk of a fn without `requires` found: it is
    /// effectful when it performs an effectful operation, and needs the
    /// union of the effects found in this and earlier passes. Sets
    /// `changed` when that differs from the signature so far.
    fn update_inferred(&mut self, decl: &FnDecl) {
        let name = &decl.qualified_name();
        let Some(info) = self.symbols.functions.get(name).cloned() else {
            return;
        };
        let mut sig = self
            .symbols
            .effects
            .get(name)
            .cloned()
            .unwrap_or_else(|| EffectSig::of(decl));
        let before = sig.requires.len();
        let mut requires: BTreeSet<String> = sig.requires.drain(..).collect();
        requires.extend(std::mem::take(&mut self.inferred));
        sig.requires = requires.into_iter().collect();
        let effectful = info.effectful || self.performs;
        if effectful != info.effectful || sig.requires.len() != before {
            self.changed = true;
        }
        if effectful {
            self.symbols.functions.insert(
                name.clone(),
                FnInfo {
                    effectful,
                    arity: info.arity,
                },
            );
            self.symbols.effects.insert(name.clone(), sig);
        }
    }

    /// Walk a pure value (a constant's value or a field default) with no
    /// local bindings.
    fn pure_value(&mut self, ctx: Ctx, value: &Expr) {
        self.ctx = ctx;
        self.scopes.push(BTreeMap::new());
        self.expr(value);
        self.scopes.pop();
        self.ctx = Ctx::None;
    }

    /// Whether the walk is inside a constant's value.
    fn in_const(&self) -> bool {
        matches!(self.ctx, Ctx::Const(_))
    }

    /// `E048` for an unknown type name or a wrong number of type arguments.
    /// E056 for a parameter (of a fn, a role, a fn type or a closure) named
    /// like an ambient capability: its `requires` head would be the
    /// parameter, so the ambient effect would be charged to the argument's
    /// roots instead (`docs/slice.md` section 13.11).
    fn ambient_name(&mut self, name: &Ident) {
        if AMBIENT_HEADS.contains(&name.name.as_str()) {
            self.error(
                "E056",
                name.span,
                format!(
                    "parameter `{}` is named like the ambient capability `{}`; rename it",
                    name.name, name.name
                ),
            );
        }
    }

    fn check_type(&mut self, ty: &Type) {
        for param in &ty.fn_names {
            self.ambient_name(param);
        }
        let name = ty.name.name.as_str();
        if BUILTIN_INTERFACES.contains(&name) {
            self.error(
                "E099",
                ty.name.span,
                format!(
                    "`{name}` is an interface, not a type; declare a type parameter `<S: {name}>`"
                ),
            );
            for arg in &ty.args {
                self.check_type(arg);
            }
            return;
        }
        let known = BUILTIN_TYPES.contains(&name)
            || self.symbols.records.contains(name)
            || self.symbols.enums.contains_key(name)
            || self.symbols.aliases.contains_key(name)
            || self.type_params.contains(name);
        let structural = ty.is_fn() || ty.is_tuple();
        if !self.type_params.contains(name) {
            self.test_only_use(&ty.name);
        }
        if !structural && !known {
            if !self.glob_conflict(&ty.name) {
                self.error("E048", ty.name.span, format!("unknown type `{name}`"));
            }
        } else if !structural {
            let arity = if self.type_params.contains(name) {
                0
            } else if let Some(arity) = self.symbols.aliases.get(name) {
                *arity
            } else if let Some(params) = self.symbols.type_params.get(name) {
                params.len()
            } else {
                type_arity(name)
            };
            let optional = name == "Ref" && arity == 0 && ty.args.len() == 1;
            if ty.args.len() != arity && !optional {
                let takes = match arity {
                    0 if name == "Ref" => "at most one type argument".to_string(),
                    0 => "no type arguments".to_string(),
                    n => plural(n, "type argument"),
                };
                self.error(
                    "E048",
                    ty.span,
                    format!("`{name}` takes {takes}, found {}", ty.args.len()),
                );
            }
        }
        for arg in &ty.args {
            self.check_type(arg);
        }
    }

    // ---- scopes -----------------------------------------------------------

    fn bind_rooted(&mut self, name: &Ident, binding: Binding, roots: BTreeSet<String>) {
        self.bind_typed(name, binding, roots, None);
    }

    fn bind_typed(
        &mut self,
        name: &Ident,
        binding: Binding,
        roots: BTreeSet<String>,
        ty: Option<Type>,
    ) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(
                name.name.clone(),
                Local {
                    binding,
                    roots,
                    ty,
                    result: None,
                },
            );
        }
    }

    fn lookup(&self, name: &str) -> Option<Binding> {
        self.local(name).map(|local| local.binding)
    }

    fn local(&self, name: &str) -> Option<&Local> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    /// The parameters of the enclosing function the value of `expr` may
    /// come from (`docs/slice.md` section 5.1): a parameter, a local bound
    /// or assigned from one, a field, element, `copy()`, `at`, `slice` or
    /// `e?` of one, a tuple, list or record literal holding one, the value
    /// of an `if`, `match` or block that may be one, and the result of any
    /// call of a user fn (pure or effectful, by name or UFCS), a fn value,
    /// a variant constructor, a role or a `Store` method taking one as an
    /// argument whose type can hold a substrate handle. Empty for literals,
    /// operators and the results of substrate operations: the function owns
    /// them.
    fn roots(&self, expr: &Expr) -> BTreeSet<String> {
        match &expr.kind {
            ExprKind::Name(name) => self
                .local(&name.name)
                .map(|local| local.roots.clone())
                .unwrap_or_default(),
            ExprKind::Field { receiver, .. } if self.path_head(receiver).is_some() => {
                BTreeSet::new()
            }
            ExprKind::Field { receiver, .. }
            | ExprKind::Index { receiver, .. }
            | ExprKind::TupleField { receiver, .. }
            | ExprKind::Try(receiver)
            | ExprKind::Await(receiver) => self.roots(receiver),
            ExprKind::RecordUpdate { fields, base, .. } => fields
                .iter()
                .flat_map(|field| self.roots(&field.value))
                .chain(self.roots(base))
                .collect(),
            ExprKind::Tuple(items) | ExprKind::List(items) => {
                items.iter().flat_map(|item| self.roots(item)).collect()
            }
            ExprKind::Record { fields, .. } => fields
                .iter()
                .flat_map(|field| self.roots(&field.value))
                .collect(),
            ExprKind::Block(block) => self.block_roots(block),
            ExprKind::If {
                then, otherwise, ..
            } => {
                let mut roots = self.block_roots(then);
                if let Some(otherwise) = otherwise {
                    roots.extend(self.roots(otherwise));
                }
                roots
            }
            ExprKind::Match { arms, .. } => arms
                .iter()
                .flat_map(|arm| match self.tail_roots.get(&expr_key(&arm.body)) {
                    Some(roots) => roots.clone(),
                    None => self.roots(&arm.body),
                })
                .collect(),
            ExprKind::Method {
                receiver,
                name,
                args,
            } => match self.path_head(receiver) {
                Some(ty) if is_effectful_static(&ty.name, &name.name) => BTreeSet::new(),
                Some(_) => self.arg_roots(args.iter()),
                None => self.method_roots(receiver, name, args),
            },
            ExprKind::Call { callee, args } => match &callee.kind {
                ExprKind::Field { receiver, name } => match self.path_head(receiver) {
                    Some(ty) if is_effectful_static(&ty.name, &name.name) => BTreeSet::new(),
                    _ => self.arg_roots(args.iter()),
                },
                ExprKind::Name(f) if self.lookup(&f.name).is_some() => self
                    .roots(callee)
                    .into_iter()
                    .chain(self.arg_roots(args.iter()))
                    .collect(),
                _ => self.arg_roots(args.iter()),
            },
            ExprKind::Closure(closure) => self.roots(&closure.body),
            _ => BTreeSet::new(),
        }
    }

    /// The roots of `receiver.name(args)` when it is no path call.
    fn method_roots(&self, receiver: &Expr, name: &Ident, args: &[Expr]) -> BTreeSet<String> {
        match name.name.as_str() {
            "copy" | "at" | "slice" => return self.roots(receiver),
            "concat" => return self.arg_roots(std::iter::once(receiver).chain(args)),
            "map" if self.builtin_method(receiver, name) => {
                return self
                    .roots(receiver)
                    .into_iter()
                    .chain(args.iter().flat_map(|f| self.roots(f)))
                    .collect();
            }
            _ => {}
        }
        let receiver_ty = self.static_type(receiver);
        if self.store_op(receiver_ty.as_ref(), &name.name).is_some() {
            return self.arg_roots(std::iter::once(receiver).chain(args));
        }
        if self.map_read(receiver_ty.as_ref(), &name.name) {
            return self.arg_roots(std::iter::once(receiver));
        }
        if self.builtin_method(receiver, name) {
            return BTreeSet::new();
        }
        if self
            .capability_op(receiver_ty.as_ref(), &name.name)
            .is_some()
        {
            return self.arg_roots(args.iter());
        }
        let fn_field = receiver_ty
            .as_ref()
            .and_then(|ty| self.field_type(ty, &name.name))
            .is_some_and(|ty| ty.is_fn());
        if !fn_field && self.ufcs(receiver, name).is_some() {
            self.arg_roots(std::iter::once(receiver).chain(args))
        } else {
            self.arg_roots(args.iter())
        }
    }

    /// The roots of the arguments of a call whose types can hold a
    /// substrate handle; an argument of a declared type that cannot
    /// (`Int`, `Text`, ..) adds none.
    fn arg_roots<'e>(&self, args: impl Iterator<Item = &'e Expr>) -> BTreeSet<String> {
        args.filter(|arg| {
            self.static_type(arg)
                .is_none_or(|ty| self.can_hold_handle(&ty, 0))
        })
        .flat_map(|arg| self.roots(arg))
        .collect()
    }

    /// The roots of the value of `block`: those of its last statement when
    /// that is an expression, recorded while the block's scope was open.
    fn block_roots(&self, block: &Block) -> BTreeSet<String> {
        self.tail_roots
            .get(&(block as *const Block as usize))
            .cloned()
            .unwrap_or_default()
    }

    /// Whether a value of declared type `ty` may hold a `TcpStream`,
    /// `PipeStream`, `TcpListener`, `Store`, `Stdin`, `Stdout`, `Stderr` or a capability
    /// (`Directory`, `Launcher`, `ChildProcess`, `Entropy`, `Setting`, `CommandLine`,
    /// `Environment`),
    /// directly or in a
    /// field, payload, element or type argument. Unknown types may.
    fn can_hold_handle(&self, ty: &Type, depth: usize) -> bool {
        if depth > 16 {
            return true;
        }
        if ty.is_fn() {
            return false;
        }
        let ty = self.expand_ref(ty);
        let name = ty.name.name.as_str();
        match name {
            "Int" | "Text" | "Bool" | "Byte" | "Bytes" | "Unit" | "IoError" | "Input" => {
                return false;
            }
            "TcpStream" | "PipeStream" | "TcpListener" | "Store" | "Stdin" | "Stdout"
            | "Stderr" | "Directory" | "Launcher" | "ChildProcess" | "Entropy" | "Setting"
            | "CommandLine" | "Environment" => {
                return true;
            }
            "List" | "Option" | "Result" | "Tuple" => {
                return ty.args.iter().any(|a| self.can_hold_handle(a, depth + 1));
            }
            _ => {}
        }
        if self.type_params.contains(name) {
            return true;
        }
        let params = self
            .symbols
            .type_params
            .get(name)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let holds = |member: &Type| {
            if params.is_empty() || ty.args.is_empty() {
                self.can_hold_handle(member, depth + 1)
            } else {
                self.can_hold_handle(&substitute_type(member, params, &ty.args), depth + 1)
            }
        };
        if let Some(fields) = self.symbols.record_field_types.get(name) {
            fields.iter().any(|(_, ty)| holds(ty))
        } else if let Some(payloads) = self.symbols.enum_payload_types.get(name) {
            payloads.iter().any(holds)
        } else {
            true
        }
    }

    /// Whether `receiver.name(..)` is a builtin method call: `name` is a
    /// builtin method name and either no module-level fn has that name,
    /// or the receiver's declared type is unknown here, or the builtin
    /// applies to it (`docs/slice.md` section 13, row 5). The type checker
    /// reports E042 where an unknown receiver turns out to take an
    /// effectful user fn.
    fn builtin_method(&self, receiver: &Expr, name: &Ident) -> bool {
        if !BUILTIN_METHODS.contains(&name.name.as_str()) {
            return false;
        }
        if !self.symbols.functions.contains_key(&name.name) {
            return true;
        }
        match self.static_type(receiver) {
            None => true,
            Some(ty) if ty.args.is_empty() && self.bounded.contains(&ty.name.name) => {
                is_stream_method(&name.name) || matches!(name.name.as_str(), "copy" | "debug")
            }
            Some(ty) => builtin_applies(&ty, &name.name),
        }
    }

    /// Whether binding `name` is captured by the closure being walked.
    fn captured(&self, name: &str) -> bool {
        let Some(&outside) = self.closure_scopes.last() else {
            return false;
        };
        self.scopes
            .iter()
            .rposition(|scope| scope.contains_key(name))
            .is_some_and(|index| index < outside)
    }

    /// The user fn a method call `receiver.name(..)` calls by UFCS: a
    /// module-level fn (own or imported) named `name`, when the receiver is
    /// not a path head and the call is no builtin method call
    /// ([`Checker::builtin_method`]). The type checker confirms it against
    /// the receiver type (`docs/slice.md` section 10).
    fn ufcs(&self, receiver: &Expr, name: &Ident) -> Option<FnInfo> {
        if self.path_head(receiver).is_some() || self.builtin_method(receiver, name) {
            return None;
        }
        if !self.in_test && self.symbols.test_only.contains(&name.name) {
            return None;
        }
        self.symbols.functions.get(&name.name).cloned()
    }

    // ---- effects ----------------------------------------------------------

    /// One effectful operation `what` that needs `effects` from the
    /// function being walked: `E016` for each one a declared function
    /// lacks, collected for an inferred one, `E010` in a pure value.
    fn effectful_op(&mut self, effects: Vec<String>, span: Span, what: &str) {
        match self.ctx.clone() {
            Ctx::Declared => {
                for effect in effects {
                    if !covers(&self.requires, &effect) {
                        let message = format!(
                            "missing effect `{effect}` for `{what}`; add it to the `requires` of `{}`",
                            self.fn_name
                        );
                        self.error("E016", span, message);
                    }
                }
            }
            Ctx::Inferred => {
                self.performs = true;
                self.inferred.extend(effects);
            }
            Ctx::Const(_) | Ctx::Default(_) if self.quiet => {}
            Ctx::Const(_) | Ctx::Default(_) => {
                let message = format!(
                    "effectful call `{what}` outside an effectful function: {}",
                    self.pure_reason()
                );
                self.error("E010", span, message);
            }
            Ctx::None => {}
        }
    }

    /// Why the value being walked is pure, for `E010`.
    fn pure_reason(&self) -> String {
        match &self.ctx {
            Ctx::Const(name) => format!("the value of constant `{name}` is pure"),
            Ctx::Default(name) => format!("the default of field `{name}` is pure"),
            _ => "this context is pure".to_string(),
        }
    }

    /// The effects a call to the fn or role `key` needs from the caller:
    /// each requirement `q.op` on a parameter `q` of the callee becomes
    /// `r.op` for every root `r` of the argument passed for `q`; any other
    /// requirement passes through unchanged.
    fn callee_effects(&self, key: &str, args: &[&Expr]) -> Vec<String> {
        let Some(sig) = self.symbols.effects.get(key) else {
            return Vec::new();
        };
        let mut needed = Vec::new();
        for requirement in &sig.requires {
            let on_param = requirement.split_once('.').and_then(|(head, op)| {
                let index = sig.params.iter().position(|p| p == head)?;
                Some((index, op))
            });
            match on_param {
                Some((index, op)) => {
                    let roots = args.get(index).map(|a| self.roots(a)).unwrap_or_default();
                    for root in roots {
                        needed.push(format!("{root}.{op}"));
                    }
                }
                None => needed.push(requirement.clone()),
            }
        }
        needed
    }

    fn block(&mut self, block: &Block) {
        self.scopes.push(BTreeMap::new());
        for stmt in &block.stmts {
            self.stmt(stmt);
        }
        if let Some(Stmt {
            kind: StmtKind::Expr(tail),
            ..
        }) = block.stmts.last()
        {
            let roots = self.roots(tail);
            self.tail_roots
                .insert(block as *const Block as usize, roots);
        }
        self.scopes.pop();
    }

    // ---- statements -------------------------------------------------------

    fn stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::LetPattern {
                pattern,
                value,
                mutable,
            } => {
                self.expr(value);
                let binding = if *mutable { Binding::Var } else { Binding::Let };
                let roots = self.roots(value);
                self.pattern(pattern, &roots, binding);
            }
            StmtKind::Let {
                name,
                ty,
                value,
                mutable,
            } => {
                if let Some(ty) = ty {
                    self.check_type(ty);
                }
                self.expr(value);
                let binding = if *mutable { Binding::Var } else { Binding::Let };
                let roots = self.roots(value);
                let declared = match ty {
                    Some(ty) => Some(self.expand(ty)),
                    None => self.static_type(value),
                };
                let result = match (&declared, &value.kind) {
                    (None, ExprKind::Call { callee, .. }) => match &callee.kind {
                        ExprKind::Name(f) if self.local(&f.name).is_none() => self
                            .symbols
                            .fn_returns
                            .get(&f.name)
                            .cloned()
                            .map(|ret| self.expand(&ret)),
                        _ => None,
                    },
                    (None, ExprKind::Record { name: record, .. })
                        if self.symbols.records.contains(&record.name)
                            && !self.symbols.type_params.contains_key(&record.name) =>
                    {
                        Some(Type::named(record.clone(), Vec::new(), record.span))
                    }
                    _ => None,
                };
                self.bind_typed(name, binding, roots, declared);
                if let Some(local) = self
                    .scopes
                    .last_mut()
                    .and_then(|scope| scope.get_mut(&name.name))
                {
                    local.result = result;
                }
            }
            StmtKind::Assign { target, value } => {
                self.expr(value);
                let base = &target.base;
                if self.captured(&base.name) {
                    self.error(
                        "E006",
                        target.span,
                        format!(
                            "cannot assign to `{}` inside a closure; a closure captures `{}` by copy",
                            base.name, base.name
                        ),
                    );
                }
                let roots = self.roots(value);
                if let Some(local) = self
                    .scopes
                    .iter_mut()
                    .rev()
                    .find_map(|scope| scope.get_mut(&base.name))
                {
                    local.roots.extend(roots);
                }
                match self.lookup(&base.name) {
                    None if self.symbols.constants.contains(&base.name) => self.error(
                        "E006",
                        target.span,
                        format!(
                            "cannot assign to constant `{}`; a constant is a value, not a place",
                            base.name
                        ),
                    ),
                    None => {
                        self.error("E003", base.span, format!("unknown name `{}`", base.name));
                    }
                    Some(Binding::Var) => {}
                    Some(Binding::Param) => self.error(
                        "E006",
                        target.span,
                        format!(
                            "cannot assign to parameter `{}`; parameters are immutable",
                            base.name
                        ),
                    ),
                    Some(Binding::Let) => self.error(
                        "E006",
                        target.span,
                        format!(
                            "cannot assign to immutable binding `{}`; declare it with `var`",
                            base.name
                        ),
                    ),
                    Some(Binding::Pattern | Binding::Loop) => self.error(
                        "E006",
                        target.span,
                        format!("cannot assign to immutable binding `{}`", base.name),
                    ),
                }
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    self.expr(value);
                }
            }
            StmtKind::While { cond, body } => {
                self.expr(cond);
                self.block(body);
            }
            StmtKind::Loop { body } => self.block(body),
            StmtKind::For { name, iter, body } => {
                self.expr(iter);
                self.scopes.push(BTreeMap::new());
                let roots = self.roots(iter);
                let element = self
                    .static_type(iter)
                    .filter(|ty| ty.name.name == "List")
                    .and_then(|ty| ty.args.first().map(|t| self.expand(t)));
                self.bind_typed(name, Binding::Loop, roots, element);
                self.block(body);
                self.scopes.pop();
            }
            StmtKind::ForPattern {
                pattern,
                iter,
                body,
            } => {
                self.expr(iter);
                self.scopes.push(BTreeMap::new());
                let roots = self.roots(iter);
                self.pattern(pattern, &roots, Binding::Loop);
                self.block(body);
                self.scopes.pop();
            }
            StmtKind::Break | StmtKind::Continue => {}
            StmtKind::Expr(expr) => self.expr(expr),
            StmtKind::Assert { cond, message } => {
                self.expr(cond);
                if let Some(message) = message {
                    self.expr(message);
                }
            }
        }
    }

    // ---- expressions ------------------------------------------------------

    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Int(_) | ExprKind::Str(_) | ExprKind::Bool(_) => {}
            ExprKind::Name(name) => self.value_name(name),
            ExprKind::Record { name, fields } => self.record_literal(expr, name, fields),
            ExprKind::List(items) | ExprKind::Tuple(items) => {
                for item in items {
                    self.expr(item);
                }
            }
            ExprKind::Return(value) => {
                if let Some(value) = value {
                    self.expr(value);
                }
            }
            ExprKind::Call { callee, args } => self.call(expr, callee, args),
            ExprKind::Method {
                receiver,
                name,
                args,
            } => self.method(expr, receiver, name, args),
            ExprKind::Field { receiver, name } => {
                if let Some(ty) = self.path_head(receiver) {
                    self.variant_value(ty, name);
                } else {
                    self.expr(receiver);
                }
            }
            ExprKind::Index { receiver, index } => {
                self.expr(receiver);
                self.expr(index);
            }
            ExprKind::Try(inner) | ExprKind::Await(inner) => self.expr(inner),
            ExprKind::Spawn(inner) => self.spawn(expr, inner),
            ExprKind::Unary { operand, .. } => self.expr(operand),
            ExprKind::Binary { .. } => {
                let mut rights = Vec::new();
                let mut bottom = expr;
                while let ExprKind::Binary { left, right, .. } = &bottom.kind {
                    rights.push(&**right);
                    bottom = left;
                }
                self.expr(bottom);
                for right in rights.into_iter().rev() {
                    self.expr(right);
                }
            }
            ExprKind::Match { scrutinee, arms } => {
                self.expr(scrutinee);
                let roots = self.roots(scrutinee);
                for arm in arms {
                    self.scopes.push(BTreeMap::new());
                    self.pattern(&arm.pattern, &roots, Binding::Pattern);
                    self.expr(&arm.body);
                    let tail = self.roots(&arm.body);
                    self.tail_roots.insert(expr_key(&arm.body), tail);
                    self.scopes.pop();
                }
            }
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => {
                self.expr(cond);
                self.block(then);
                if let Some(otherwise) = otherwise {
                    self.expr(otherwise);
                }
            }
            ExprKind::Block(block) => self.block(block),
            ExprKind::Closure(closure) => self.closure(expr, closure),
            ExprKind::RecordUpdate { name, fields, base } => self.record_update(name, fields, base),
            ExprKind::TupleField { receiver, .. } => self.expr(receiver),
            ExprKind::Break | ExprKind::Continue | ExprKind::Quantity { .. } => {}
            ExprKind::NamedArg { value, .. } => self.expr(value),
            ExprKind::Brace { entries } => {
                for entry in entries {
                    self.expr(&entry.value);
                }
            }
        }
    }

    /// `receiver.name(args)`: a path call (`Enum.Variant(x)`, a role, a
    /// builtin static), a builtin method, a UFCS call of a user fn, or a
    /// call of a fn-typed field, which is pure and which the type checker
    /// resolves.
    /// E006 unless the receiver of an in-place builtin (a `Value` row of
    /// [`IN_PLACE`])
    /// is a place rooted in a `var` or a parameter, whose change stays in
    /// the callee (`docs/slice.md` section 13.11): a `let`, a `for` element
    /// or a pattern binding is a value, and `x.to_bytes()` shares `x`'s
    /// buffer.
    fn mutable_receiver(&mut self, expr: &Expr, receiver: &Expr, name: &Ident) {
        let mut current = receiver;
        loop {
            match &current.kind {
                ExprKind::Field { receiver, .. }
                | ExprKind::Index { receiver, .. }
                | ExprKind::TupleField { receiver, .. } => current = receiver,
                ExprKind::Method {
                    name: inner, args, ..
                } if inner.name == "to_bytes" && args.is_empty() => {
                    self.error(
                        "E006",
                        expr.span,
                        format!(
                            "cannot `{}` on the result of `to_bytes()`, which shares the text's buffer; bind it first: `var b = t.to_bytes()`",
                            name.name
                        ),
                    );
                    return;
                }
                _ => break,
            }
        }
        let ExprKind::Name(base) = &current.kind else {
            return;
        };
        if self.captured(&base.name) {
            return;
        }
        let what = match self.lookup(&base.name) {
            Some(Binding::Let) => "the immutable binding",
            Some(Binding::Loop) => "the `for` element",
            Some(Binding::Pattern) => "the pattern binding",
            Some(Binding::Var | Binding::Param) | None => return,
        };
        self.error(
            "E006",
            expr.span,
            format!(
                "cannot `{}` through {what} `{}`: only a place rooted in a `var` or a parameter changes in place; copy it into a `var` first",
                name.name, base.name
            ),
        );
    }

    fn method(&mut self, expr: &Expr, receiver: &Expr, name: &Ident, args: &[Expr]) {
        if let Some(ty) = self.path_head(receiver) {
            self.path_call(expr, ty, name, args);
            return;
        }
        self.expr(receiver);
        let list_map = name.name == "map"
            && self.builtin_method(receiver, name)
            && self
                .static_type(receiver)
                .is_none_or(|ty| matches!(ty.name.name.as_str(), "List" | "Bytes"));
        let element = self
            .static_type(receiver)
            .filter(|ty| ty.name.name == "List")
            .and_then(|ty| ty.args.first().map(|t| self.expand(t)));
        for arg in args {
            if list_map && matches!(arg.kind, ExprKind::Closure(_)) {
                self.map_argument = Some((arg.span, element.clone()));
            }
            self.expr(arg);
            if list_map {
                self.map_fn_value(arg);
            }
        }
        let receiver_type = self.static_type(receiver).map(|ty| ty.name.name);
        let receiver_type = receiver_type
            .as_deref()
            .filter(|ty| IN_PLACE.iter().any(|row| row.receiver == *ty));
        let mutating = in_place(receiver_type, &name.name) == Some(Change::Value)
            && self.builtin_method(receiver, name);
        // Inside a closure only a receiver whose type is known here: a user
        // fn named like a builtin is no change, and the type checker reports
        // the rest once the callee is resolved (`docs/slice.md` 20.3.3).
        let captured_change = self.in_place_type(receiver).is_some_and(|ty| {
            !self.type_params.contains(&ty.name.name)
                && in_place(Some(&ty.name.name), &name.name) == Some(Change::Value)
        });
        if captured_change
            && let Some(base) = place_root(receiver)
            && self.captured(&base.name)
        {
            self.error(
                "E006",
                expr.span,
                format!(
                    "cannot change `{}` inside a closure; a closure captures `{}` by copy",
                    base.name, base.name
                ),
            );
        }
        if mutating {
            self.mutable_receiver(expr, receiver, name);
        }
        if matches!(name.name.as_str(), "push" | "set" | "append")
            && let Some(list) = place_root(receiver)
        {
            let roots = self.arg_roots(args.iter());
            if let Some(local) = self
                .scopes
                .iter_mut()
                .rev()
                .find_map(|scope| scope.get_mut(&list.name))
            {
                local.roots.extend(roots);
            }
        }
        let receiver_ty = self.static_type(receiver);
        if let Some(op) = self.capability_op(receiver_ty.as_ref(), &name.name) {
            let needed = match op {
                Some(op) => self
                    .roots(receiver)
                    .into_iter()
                    .map(|root| format!("{root}.{op}"))
                    .collect(),
                None => Vec::new(),
            };
            let what = match &receiver.kind {
                ExprKind::Name(r) => format!("{}.{}", r.name, name.name),
                _ => name.name.clone(),
            };
            if op.is_some() {
                self.effectful_op(needed, expr.span, &what);
            }
            self.symbols.effect_sites.insert(expr.span);
            return;
        }
        if let Some(op) = self.store_op(receiver_ty.as_ref(), &name.name) {
            let needed = self
                .roots(receiver)
                .into_iter()
                .map(|root| format!("{root}.{op}"))
                .collect();
            let what = match &receiver.kind {
                ExprKind::Name(r) => format!("{}.{}", r.name, name.name),
                _ => name.name.clone(),
            };
            self.effectful_op(needed, expr.span, &what);
            self.symbols.effect_sites.insert(expr.span);
            return;
        }
        if let Some(info) = self.ufcs(receiver, name) {
            self.arity(expr, &name.name, info.arity, args.len() + 1);
            if info.effectful {
                let all: Vec<&Expr> = std::iter::once(receiver).chain(args).collect();
                let needed = self.callee_effects(&name.name, &all);
                self.effectful_op(needed, expr.span, &name.name);
            }
            return;
        }
        if EFFECTFUL_METHODS.contains(&name.name.as_str()) && self.builtin_method(receiver, name) {
            let what = match &receiver.kind {
                ExprKind::Name(r) => format!("{}.{}", r.name, name.name),
                _ => name.name.clone(),
            };
            let needed = match method_effect(&name.name) {
                Some(op) => self
                    .roots(receiver)
                    .into_iter()
                    .map(|root| format!("{root}.{op}"))
                    .collect(),
                None => Vec::new(),
            };
            self.effectful_op(needed, expr.span, &what);
            return;
        }
        if self.builtin_method(receiver, name) {
            return;
        }
        let field = receiver_ty.and_then(|ty| self.field_type(&ty, &name.name));
        let all: Vec<&Expr> = args.iter().collect();
        if let Some(needed) = field.and_then(|ty| self.fn_value_effects(&ty, &all)) {
            self.effectful_op(needed, expr.span, &name.name);
            self.symbols.effect_sites.insert(expr.span);
        }
    }

    /// A closure literal: its body is walked like the body of a fn without
    /// `requires`, its parameters rooted in themselves. Effects on its
    /// parameters belong to its fn type; every other effect (on a capture's
    /// roots or ambient) is also needed where the closure is created, which
    /// is how it carries the authority it captured (`docs/slice.md` section
    /// 13.3).
    ///
    /// The argument of a `List.map` call inside a fn with `requires` is
    /// the exception: the type checker checks its effects against that
    /// `requires` (E017, `docs/slice.md` 20.4.3). Its one parameter, when
    /// unannotated, has the list's element type, as in the type checker,
    /// so a `Store` method on it is an effect on the parameter.
    fn closure(&mut self, expr: &Expr, closure: &Closure) {
        let (map_argument, element) = match self.map_argument.take() {
            Some((span, element)) if span == expr.span => (true, element),
            _ => (false, None),
        };
        let element = element.filter(|_| closure.params.len() == 1);
        let ctx = std::mem::replace(&mut self.ctx, Ctx::Inferred);
        let inferred = std::mem::take(&mut self.inferred);
        let performs = std::mem::replace(&mut self.performs, false);
        self.closure_scopes.push(self.scopes.len());
        self.scopes.push(BTreeMap::new());
        let mut seen: Vec<&str> = Vec::new();
        for param in &closure.params {
            if seen.contains(&param.name.name.as_str()) && param.name.name != "_" {
                self.error(
                    "E004",
                    param.name.span,
                    format!("duplicate closure parameter `{}`", param.name.name),
                );
            }
            seen.push(&param.name.name);
            self.ambient_name(&param.name);
            if let Some(ty) = &param.ty {
                self.check_type(ty);
            }
            let declared = match &param.ty {
                Some(t) => Some(self.expand(t)),
                None => element.clone(),
            };
            let roots = BTreeSet::from([param.name.name.clone()]);
            self.bind_typed(&param.name, Binding::Param, roots, declared);
        }
        self.expr(&closure.body);
        self.scopes.pop();
        self.closure_scopes.pop();
        let effects = std::mem::replace(&mut self.inferred, inferred);
        let effectful = std::mem::replace(&mut self.performs, performs);
        self.ctx = ctx;
        let params: Vec<String> = closure.params.iter().map(|p| p.name.name.clone()).collect();
        if !effectful {
            self.symbols.closure_effects.remove(&expr.span);
            return;
        }
        let outer: Vec<String> = effects
            .iter()
            .filter(|effect| {
                let head = effect.split_once('.').map_or(effect.as_str(), |(h, _)| h);
                !params.iter().any(|p| p == head)
            })
            .cloned()
            .collect();
        if !outer.is_empty() && !(map_argument && matches!(self.ctx, Ctx::Declared)) {
            self.effectful_op(outer, expr.span, "closure");
        }
        self.symbols.closure_effects.insert(
            expr.span,
            EffectSig {
                params,
                requires: effects.into_iter().collect(),
            },
        );
    }

    /// `arg`, the argument of a `List.map` call, when it names an effectful
    /// fn: `map` calls it, so a fn without `requires` performs its effects
    /// that are not on its own parameter (an effect on the parameter is
    /// E017, the type checker's, as is an uncovered one in a fn with
    /// `requires`; `docs/slice.md` 20.4.3). A local names an effectful fn
    /// when its declared type is an effectful fn type (a parameter, a
    /// let-bound closure), as for a call of it; `arg` is then an effect
    /// site in every context, so the type checker reports E042 for a local
    /// of effectful fn type whose type the checker cannot tell (a call's
    /// result, a pattern binding) where it would be charged here.
    fn map_fn_value(&mut self, arg: &Expr) {
        let ExprKind::Name(name) = &arg.kind else {
            return;
        };
        if let Some(local) = self.local(&name.name) {
            let ty = local.ty.as_ref().map(|ty| self.expand(ty));
            if let Some(needed) = ty.and_then(|ty| self.fn_value_effects(&ty, &[])) {
                if matches!(self.ctx, Ctx::Inferred) {
                    self.effectful_op(needed, arg.span, &name.name);
                }
                self.symbols.effect_sites.insert(arg.span);
            }
            return;
        }
        if !matches!(self.ctx, Ctx::Inferred) {
            return;
        }
        if self.lookup(&name.name).is_some()
            || !self
                .symbols
                .functions
                .get(&name.name)
                .is_some_and(|f| f.effectful)
        {
            return;
        }
        let needed: Vec<String> = self
            .symbols
            .effects
            .get(&name.name)
            .map(|sig| {
                sig.requires
                    .iter()
                    .filter(|effect| {
                        let head = effect.split_once('.').map_or(effect.as_str(), |(h, _)| h);
                        !sig.params.iter().any(|p| p == head)
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        self.effectful_op(needed, arg.span, &name.name);
    }

    /// The declared fn type of a closure literal as far as the checker
    /// knows it: parameter names, the annotated parameter types (`_`
    /// where none is written) and `requires` from its effects.
    fn closure_type(&self, expr: &Expr, closure: &Closure) -> Type {
        let unknown = || {
            Type::named(
                Ident {
                    name: "_".to_string(),
                    span: expr.span,
                },
                Vec::new(),
                expr.span,
            )
        };
        let mut args: Vec<Type> = closure
            .params
            .iter()
            .map(|p| p.ty.clone().unwrap_or_else(unknown))
            .collect();
        args.push(unknown());
        let requires = self.symbols.closure_effects.get(&expr.span).map(|sig| {
            sig.requires
                .iter()
                .map(|effect| Path {
                    segments: effect
                        .split('.')
                        .map(|s| Ident {
                            name: s.to_string(),
                            span: expr.span,
                        })
                        .collect(),
                    span: expr.span,
                    id: NodeId::fresh(),
                })
                .collect()
        });
        Type {
            name: Ident {
                name: "fn".to_string(),
                span: expr.span,
            },
            args,
            span: expr.span,
            fn_names: closure.params.iter().map(|p| p.name.clone()).collect(),
            requires,
            id: NodeId::fresh(),
        }
    }

    /// `R { f = e, ..base }`: `E048` for an unknown record, `E047` for a
    /// field the record lacks or one given twice; omitted fields come from
    /// `base`.
    fn record_update(&mut self, name: &Ident, fields: &[FieldInit], base: &Expr) {
        self.test_only_use(name);
        let known = self.symbols.record_fields.get(&name.name).cloned();
        if known.is_none() {
            self.error(
                "E048",
                name.span,
                format!("unknown record type `{}`", name.name),
            );
        }
        let mut given: Vec<&str> = Vec::new();
        for field in fields {
            let field_name = field.name.name.as_str();
            if let Some(known) = &known {
                if !known.iter().any(|f| f == field_name) {
                    self.error(
                        "E047",
                        field.name.span,
                        format!("record `{}` has no field `{field_name}`", name.name),
                    );
                } else if given.contains(&field_name) {
                    self.error(
                        "E047",
                        field.name.span,
                        format!(
                            "field `{field_name}` is given twice in `{}` update",
                            name.name
                        ),
                    );
                }
            }
            given.push(field_name);
            self.expr(&field.value);
        }
        self.expr(base);
    }

    // ---- declared types ---------------------------------------------------

    /// `ty` with aliases at its head expanded, their arguments substituted.
    fn expand(&self, ty: &Type) -> Type {
        self.expand_ref(ty).into_owned()
    }

    /// Most declared types have no alias at their head and can be borrowed.
    fn expand_ref<'t>(&self, ty: &'t Type) -> Cow<'t, Type> {
        let mut ty = Cow::Borrowed(ty);
        for _ in 0..32 {
            if self.type_params.contains(&ty.name.name) {
                break;
            }
            let Some(target) = self.symbols.alias_targets.get(&ty.name.name) else {
                break;
            };
            let params = self
                .symbols
                .type_params
                .get(&ty.name.name)
                .cloned()
                .unwrap_or_default();
            ty = Cow::Owned(substitute_type(target, &params, &ty.args));
        }
        ty
    }

    /// The declared type of field `field` of a value of record type
    /// `record`, the record's type arguments substituted.
    fn field_type(&self, record: &Type, field: &str) -> Option<Type> {
        let record = self.expand_ref(record);
        let fields = self.symbols.record_field_types.get(&record.name.name)?;
        let (_, ty) = fields.iter().find(|(n, _)| n == field)?;
        let params = self
            .symbols
            .type_params
            .get(&record.name.name)
            .cloned()
            .unwrap_or_default();
        Some(self.expand(&substitute_type(ty, &params, &record.args)))
    }

    /// The declared type of the value of `expr`, where it follows from
    /// declarations alone (`docs/slice.md` section 11.7): a local with a
    /// known type, a field of a value of known record type, `copy()`, `e?`,
    /// `l[i]`, the results of `Store` methods and a literal of a record
    /// without type parameters. `None` otherwise.
    fn static_type(&self, expr: &Expr) -> Option<Type> {
        let named = |name: &str, args: Vec<Type>| {
            let ident = Ident {
                name: name.to_string(),
                span: expr.span,
            };
            Type::named(ident, args, expr.span)
        };
        let ty = match &expr.kind {
            ExprKind::Name(name) => self.local(&name.name)?.ty.clone()?,
            ExprKind::Field { receiver, name } if self.path_head(receiver).is_none() => {
                let record = self.static_type(receiver)?;
                return self.field_type(&record, &name.name);
            }
            ExprKind::Method { receiver, name, .. } if self.path_head(receiver).is_none() => {
                let receiver = self.static_type(receiver)?;
                match name.name.as_str() {
                    "copy" => receiver,
                    "get" | "remove" | "update" if receiver.name.name == "Store" => {
                        named("Option", receiver.args.clone())
                    }
                    "values" if receiver.name.name == "Store" => {
                        named("List", receiver.args.clone())
                    }
                    "upsert" if receiver.name.name == "Store" => receiver.args.first()?.clone(),
                    "start"
                        if receiver.name.name == "Launcher"
                            && !self.type_params.contains("Launcher") =>
                    {
                        named(
                            "Result",
                            vec![
                                named("ChildProcess", Vec::new()),
                                named("IoError", Vec::new()),
                            ],
                        )
                    }
                    _ => return None,
                }
            }
            ExprKind::Method { receiver, name, .. } => {
                let owner = self.path_head(receiver)?;
                return self.constructor_type(&owner.name, &name.name, expr.span);
            }
            ExprKind::Call { callee, .. } => {
                let ExprKind::Field { receiver, name } = &callee.kind else {
                    return None;
                };
                let owner = self.path_head(receiver)?;
                return self.constructor_type(&owner.name, &name.name, expr.span);
            }
            ExprKind::Try(inner) => {
                let inner = self.static_type(inner)?;
                if inner.name.name != "Result" {
                    return None;
                }
                inner.args.first()?.clone()
            }
            ExprKind::TupleField { receiver, index } => {
                let tuple = self.static_type(receiver)?;
                if !tuple.is_tuple() {
                    return None;
                }
                tuple.args.get(*index)?.clone()
            }
            ExprKind::Closure(closure) => return Some(self.closure_type(expr, closure)),
            ExprKind::Record { name: record, .. }
                if self.symbols.records.contains(&record.name)
                    && !self.symbols.type_params.contains_key(&record.name)
                    && !self.type_params.contains(&record.name) =>
            {
                Type::named(record.clone(), Vec::new(), record.span)
            }
            ExprKind::Index { receiver, .. } => {
                let list = self.static_type(receiver)?;
                match list.name.name.as_str() {
                    "List" => list.args.first()?.clone(),
                    "Bytes" => named("Byte", Vec::new()),
                    _ => return None,
                }
            }
            _ => return None,
        };
        Some(self.expand(&ty))
    }

    /// [`Checker::static_type`] of a place, where a local bound to a call
    /// of a non-generic fn also has that fn's declared return type
    /// (`Local::result`). Read only by the closure rule of E006.
    fn in_place_type(&self, expr: &Expr) -> Option<Type> {
        match &expr.kind {
            ExprKind::Name(name) => {
                let local = self.local(&name.name)?;
                local.ty.clone().or_else(|| local.result.clone())
            }
            ExprKind::Field { receiver, name } if self.path_head(receiver).is_none() => {
                let record = self.in_place_type(receiver)?;
                self.field_type(&record, &name.name)
            }
            ExprKind::Index { receiver, .. } => {
                let list = self.in_place_type(receiver)?;
                match list.name.name.as_str() {
                    "List" => list.args.first().map(|t| self.expand(t)),
                    _ => None,
                }
            }
            ExprKind::TupleField { receiver, index } => {
                let tuple = self.in_place_type(receiver)?;
                if !tuple.is_tuple() {
                    return None;
                }
                tuple.args.get(*index).map(|t| self.expand(t))
            }
            _ => self.static_type(expr),
        }
    }

    /// Whether `receiver.method(..)` on a receiver of declared type
    /// `receiver` reads elements out of a `Map`: `get`, `remove` and `keys`,
    /// whose results have the roots of the receiver (`docs/slice.md` 5.1).
    fn map_read(&self, receiver: Option<&Type>, method: &str) -> bool {
        matches!(receiver, Some(ty) if ty.name.name == "Map" && !self.type_params.contains("Map"))
            && matches!(method, "get" | "remove" | "keys")
    }

    /// The effect operation of a `Store` method called on a receiver of
    /// declared type `receiver`.
    fn store_op(&self, receiver: Option<&Type>, method: &str) -> Option<&'static str> {
        match receiver {
            Some(ty) if ty.name.name == "Store" && !self.type_params.contains("Store") => {
                store_effect(method)
            }
            _ => None,
        }
    }

    /// The result type of a test-only capability constructor
    /// (`docs/slice.md` 21.3, 21.8): `Directory.temporary()` is a
    /// `Result<Directory, IoError>`, `Launcher.new()`, `Entropy.new()`,
    /// `Setting.of(..)`, `CommandLine.of(..)` (21.19) and `Environment.of(..)`
    /// (21.20) the capability itself. `None` for any other static
    /// or when a declaration shadows the owner.
    fn constructor_type(&self, owner: &str, name: &str, span: Span) -> Option<Type> {
        if self.type_params.contains(owner) || self.symbols.records.contains(owner) {
            return None;
        }
        let named = |name: &str, args: Vec<Type>| {
            let ident = Ident {
                name: name.to_string(),
                span,
            };
            Type::named(ident, args, span)
        };
        match (owner, name) {
            ("Directory", "temporary") => Some(named(
                "Result",
                vec![named("Directory", Vec::new()), named("IoError", Vec::new())],
            )),
            ("Launcher", "new")
            | ("Entropy", "new")
            | ("Setting", "of")
            | ("CommandLine", "of")
            | ("Environment", "of") => Some(named(owner, Vec::new())),
            _ => None,
        }
    }

    /// The effect of a capability method (`docs/slice.md` 21.3) called on a
    /// receiver of declared type `receiver`, as [`capability_effect`].
    fn capability_op(&self, receiver: Option<&Type>, method: &str) -> Option<Option<&'static str>> {
        let ty = receiver?;
        let name = ty.name.name.as_str();
        if self.type_params.contains(name) {
            return None;
        }
        capability_effect(name, method)
    }

    /// The effects a call of a value of fn type `ty` needs: each entry
    /// `n.op` of its `requires` on its parameter `n` becomes `r.op` for each
    /// root `r` of the argument for `n`; any other entry is needed as
    /// written. `None` when `ty` is not an effectful fn type.
    fn fn_value_effects(&self, ty: &Type, args: &[&Expr]) -> Option<Vec<String>> {
        if !ty.is_fn() {
            return None;
        }
        let requires = ty.requires.as_ref()?;
        let mut needed = Vec::new();
        for effect in requires {
            let dotted = effect.dotted();
            let on_param = dotted.split_once('.').and_then(|(head, op)| {
                let index = ty.fn_names.iter().position(|n| n.name == head)?;
                Some((index, op.to_string()))
            });
            match on_param {
                Some((index, op)) => {
                    let roots = args.get(index).map(|a| self.roots(a)).unwrap_or_default();
                    needed.extend(roots.into_iter().map(|root| format!("{root}.{op}")));
                }
                None => needed.push(dotted),
            }
        }
        Some(needed)
    }

    /// `spawn e`: `e` must be a call of an effectful user fn or protocol
    /// role (E013); `spawn` in a pure value is E010.
    fn spawn(&mut self, expr: &Expr, inner: &Expr) {
        let pure = matches!(self.ctx, Ctx::Const(_) | Ctx::Default(_));
        if pure {
            let message = format!(
                "`spawn` outside an effectful function: {}",
                self.pure_reason()
            );
            self.error("E010", expr.span, message);
        }
        if !self.is_spawnable(inner) {
            let message = if self.is_effectful_call(inner) {
                "`spawn` of a substrate operation; spawn a call to an effectful function or protocol role"
            } else {
                "`spawn` needs a call to an effectful function or protocol role"
            };
            self.error("E013", expr.span, message.to_string());
        }
        let quiet = self.quiet;
        self.quiet = pure;
        self.expr(inner);
        self.quiet = quiet;
    }

    /// `R { a = e, .. }`: `E048` for an unknown record, `E047` for a field
    /// the record lacks, a field given twice or a field without default
    /// left out.
    fn record_literal(&mut self, expr: &Expr, name: &Ident, fields: &[FieldInit]) {
        self.test_only_use(name);
        let known = self.symbols.record_fields.get(&name.name).cloned();
        if known.is_none() {
            self.error(
                "E048",
                name.span,
                format!("unknown record type `{}`", name.name),
            );
        }
        let defaults = self
            .symbols
            .record_defaults
            .get(&name.name)
            .cloned()
            .unwrap_or_default();
        let mut given: Vec<&str> = Vec::new();
        for field in fields {
            let field_name = field.name.name.as_str();
            if let Some(known) = &known {
                if !known.iter().any(|f| f == field_name) {
                    self.error(
                        "E047",
                        field.name.span,
                        format!("record `{}` has no field `{field_name}`", name.name),
                    );
                } else if given.contains(&field_name) {
                    self.error(
                        "E047",
                        field.name.span,
                        format!(
                            "field `{field_name}` is given twice in `{}` literal",
                            name.name
                        ),
                    );
                }
            }
            given.push(field_name);
            self.expr(&field.value);
        }
        for field in known.iter().flatten() {
            if !given.contains(&field.as_str()) && !defaults.contains(field) {
                self.error(
                    "E047",
                    expr.span,
                    format!(
                        "missing field `{field}` in `{}` literal; it has no default",
                        name.name
                    ),
                );
            }
        }
    }

    /// The type, enum or protocol name heading a path such as
    /// `Action.Continue`, `Bytes.empty` or `Echo.serve`: an identifier that
    /// is not a local binding or a constant and is uppercase or names a
    /// protocol. `LIMIT.to_text()` on a constant is an ordinary method call.
    fn path_head<'e>(&self, receiver: &'e Expr) -> Option<&'e Ident> {
        match &receiver.kind {
            ExprKind::Name(name)
                if self.lookup(&name.name).is_none()
                    && !self.symbols.constants.contains(&name.name)
                    && (name.is_uppercase() || self.symbols.protocols.contains_key(&name.name)) =>
            {
                Some(name)
            }
            _ => None,
        }
    }

    /// The user function a `f(..)` call names, unless a local shadows it.
    fn user_fn(&self, callee: &Expr) -> Option<&FnInfo> {
        match &callee.kind {
            ExprKind::Name(name) if self.lookup(&name.name).is_none() => {
                self.symbols.functions.get(&name.name)
            }
            _ => None,
        }
    }

    /// Whether `expr` is a `P.role(..)` call on a protocol `P`. A role `P`
    /// does not declare still counts, so the call reports only `E030`.
    fn is_role_call(&self, expr: &Expr) -> bool {
        let receiver = match &expr.kind {
            ExprKind::Call { callee, .. } => match &callee.kind {
                ExprKind::Field { receiver, .. } => receiver,
                _ => return false,
            },
            ExprKind::Method { receiver, .. } => receiver,
            _ => return false,
        };
        self.path_head(receiver)
            .is_some_and(|p| self.symbols.protocols.contains_key(&p.name))
    }

    /// Whether `expr` is an effectful call: of an effectful user function
    /// (also by UFCS), a protocol role, an effectful static or an effectful
    /// substrate method.
    fn is_effectful_call(&self, expr: &Expr) -> bool {
        if self.is_spawnable(expr) {
            return true;
        }
        match &expr.kind {
            ExprKind::Call { callee, .. } => match &callee.kind {
                ExprKind::Field { receiver, name } => self
                    .path_head(receiver)
                    .is_some_and(|ty| is_effectful_static(&ty.name, &name.name)),
                _ => false,
            },
            ExprKind::Method { receiver, name, .. } => match self.path_head(receiver) {
                Some(ty) => is_effectful_static(&ty.name, &name.name),
                None => EFFECTFUL_METHODS.contains(&name.name.as_str()),
            },
            _ => false,
        }
    }

    /// Whether `expr` is a call `spawn` may take: an effectful user function
    /// (also by UFCS) or a protocol role.
    fn is_spawnable(&self, expr: &Expr) -> bool {
        if self.is_role_call(expr) {
            return true;
        }
        match &expr.kind {
            ExprKind::Call { callee, .. } => self.user_fn(callee).is_some_and(|f| f.effectful),
            ExprKind::Method { receiver, name, .. } => {
                self.ufcs(receiver, name).is_some_and(|f| f.effectful)
            }
            _ => false,
        }
    }

    fn value_name(&mut self, name: &Ident) {
        if self.lookup(&name.name).is_some() || name.name == "Unit" {
            return;
        }
        if self.symbols.constants.contains(&name.name) {
            self.test_only_use(name);
            if self.in_const() {
                self.const_refs.insert(name.name.clone());
            }
            return;
        }
        if self.symbols.variants.contains_key(&name.name) {
            self.test_only_variant(name);
            if let Some(owner) = self.symbols.enum_of_variant(&name.name).map(str::to_string) {
                let payload = self.payload(&owner, &name.name);
                if payload > 0 {
                    self.error(
                        "E003",
                        name.span,
                        format!(
                            "variant `{}` takes {}; call it",
                            name.name,
                            plural(payload, "payload value")
                        ),
                    );
                }
            }
            return;
        }
        if self.symbols.functions.contains_key(&name.name) {
            self.test_only_use(name);
            return;
        }
        if BUILTIN_FUNCTIONS.contains(&name.name.as_str()) {
            self.error(
                "E003",
                name.span,
                format!("builtin function `{}` is not a value; call it", name.name),
            );
            return;
        }
        if self.symbols.protocols.contains_key(&name.name) {
            self.error(
                "E003",
                name.span,
                format!(
                    "protocol `{}` is not a value; call a role such as `{}.serve(..)`",
                    name.name, name.name
                ),
            );
            return;
        }
        if self.out_of_scope_variant(&name.name) || self.glob_conflict(name) {
            return;
        }
        self.error("E003", name.span, format!("unknown name `{}`", name.name));
    }

    /// A bare variant of an enum that is not in scope: the type checker
    /// resolves it against the expected type, or reports `E003`.
    fn out_of_scope_variant(&self, name: &str) -> bool {
        !self.symbols.variants.contains_key(name) && self.module_variants.contains(name)
    }

    fn payload(&self, enum_name: &str, variant: &str) -> usize {
        self.symbols
            .payloads
            .get(enum_name)
            .and_then(|variants| variants.get(variant))
            .copied()
            .unwrap_or(0)
    }

    fn arity(&mut self, expr: &Expr, what: &str, expected: usize, found: usize) {
        if expected != found {
            self.error(
                "E003",
                expr.span,
                format!(
                    "wrong number of arguments to `{what}`: expected {expected}, found {found}"
                ),
            );
        }
    }

    fn call(&mut self, expr: &Expr, callee: &Expr, args: &[Expr]) {
        match &callee.kind {
            ExprKind::Name(name) => {
                for arg in args {
                    self.expr(arg);
                }
                if let Some(local) = self.local(&name.name) {
                    let ty = local.ty.as_ref().map(|ty| self.expand(ty));
                    let all: Vec<&Expr> = args.iter().collect();
                    if let Some(needed) = ty.and_then(|ty| self.fn_value_effects(&ty, &all)) {
                        self.effectful_op(needed, expr.span, &name.name);
                        self.symbols.effect_sites.insert(expr.span);
                    }
                    return;
                }
                if let Some(info) = self.symbols.functions.get(&name.name).cloned() {
                    self.test_only_use(name);
                    self.arity(expr, &name.name, info.arity, args.len());
                    if info.effectful {
                        let all: Vec<&Expr> = args.iter().collect();
                        let needed = self.callee_effects(&name.name, &all);
                        self.effectful_op(needed, expr.span, &name.name);
                    }
                } else if self.symbols.constants.contains(&name.name) {
                    self.error(
                        "E003",
                        name.span,
                        format!(
                            "constant `{}` is not callable; bind it first: `let f = {}`",
                            name.name, name.name
                        ),
                    );
                } else if BUILTIN_FUNCTIONS.contains(&name.name.as_str()) {
                    self.arity(expr, &name.name, 1, args.len());
                } else if self.symbols.variants.contains_key(&name.name) {
                    self.test_only_variant(name);
                    if let Some(owner) =
                        self.symbols.enum_of_variant(&name.name).map(str::to_string)
                    {
                        let payload = self.payload(&owner, &name.name);
                        self.arity(expr, &name.name, payload, args.len());
                    }
                } else if !self.out_of_scope_variant(&name.name) && !self.glob_conflict(name) {
                    self.error("E003", name.span, format!("unknown name `{}`", name.name));
                }
            }
            ExprKind::Field { receiver, name } => match self.path_head(receiver) {
                Some(ty) => self.path_call(expr, ty, name, args),
                None => self.expression_call(expr, callee, &name.name, args),
            },
            _ => self.expression_call(expr, callee, "fn value", args),
        }
    }

    /// `e(args)` whose callee is an expression, not a name or a path
    /// (`adder(1)(2)`, `fs[1](10)`, `(h.send)(conn)`; `docs/slice.md`
    /// 20.4.2): the callee's names are resolved, and when `static_type`
    /// gives it an effectful fn type the effects are charged and the call is
    /// an effect site, as for a local of that type. Whether the callee is a
    /// fn at all is the type checker's (E040, and E042 for an effectful
    /// callee without a declared type).
    fn expression_call(&mut self, expr: &Expr, callee: &Expr, what: &str, args: &[Expr]) {
        self.expr(callee);
        for arg in args {
            self.expr(arg);
        }
        let all: Vec<&Expr> = args.iter().collect();
        if let Some(needed) = self
            .static_type(callee)
            .and_then(|ty| self.fn_value_effects(&ty, &all))
        {
            self.effectful_op(needed, expr.span, what);
            self.symbols.effect_sites.insert(expr.span);
        }
    }

    /// `Ty.name(args)`: a variant constructor, a protocol role or a builtin
    /// static.
    fn path_call(&mut self, expr: &Expr, ty: &Ident, name: &Ident, args: &[Expr]) {
        for arg in args {
            self.expr(arg);
        }
        let what = format!("{}.{}", ty.name, name.name);
        self.test_only_use(ty);
        if let Some(info) = self.symbols.functions.get(&what).cloned() {
            if !self.in_test && self.symbols.test_only.contains(&what) {
                self.error(
                    "E051",
                    name.span,
                    format!("`{what}` is test-only; only tests and test-only items may use it"),
                );
            }
            self.arity(expr, &what, info.arity, args.len());
            if info.effectful {
                let all: Vec<&Expr> = args.iter().collect();
                let needed = self.callee_effects(&what, &all);
                self.effectful_op(needed, expr.span, &what);
            }
            return;
        }
        if let Some(variants) = self.symbols.enums.get(&ty.name) {
            if variants.contains(&name.name) {
                let payload = self.payload(&ty.name, &name.name);
                self.arity(expr, &what, payload, args.len());
            } else {
                self.error(
                    "E003",
                    name.span,
                    format!("enum `{}` has no variant `{}`", ty.name, name.name),
                );
            }
        } else if let Some(roles) = self.symbols.protocols.get(&ty.name) {
            match roles.get(&name.name).cloned() {
                Some(role) => {
                    self.arity(expr, &what, role.arity, args.len());
                    let all: Vec<&Expr> = args.iter().collect();
                    let needed = self.callee_effects(&what, &all);
                    self.effectful_op(needed, expr.span, &what);
                }
                None => self.error(
                    "E030",
                    name.span,
                    format!("protocol `{}` has no role `{}`", ty.name, name.name),
                ),
            }
        } else if let Some(arity) = static_arity(&ty.name, &name.name) {
            self.arity(expr, &what, arity, args.len());
            if ty.name == "Store" && name.name == "new" && !self.in_test {
                self.error(
                    "E051",
                    name.span,
                    "`Store.new()` is test-only; a program's stores are `resource`s".to_string(),
                );
            }
            if ty.name == "Pipe" && name.name == "pair" && !self.in_test {
                self.error(
                    "E051",
                    name.span,
                    "`Pipe.pair()` is test-only; a program's streams come from a listener or a connect"
                        .to_string(),
                );
            }
            if self
                .constructor_type(&ty.name, &name.name, name.span)
                .is_some()
                && !self.in_test
            {
                self.error(
                    "E051",
                    name.span,
                    format!(
                        "`{}.{}()` is test-only; a program's capabilities are `resource`s",
                        ty.name, name.name
                    ),
                );
            }
            if is_effectful_static(&ty.name, &name.name) {
                let needed = static_effect(&ty.name, &name.name)
                    .map(str::to_string)
                    .into_iter()
                    .collect();
                self.effectful_op(needed, expr.span, &what);
            }
        } else if self.is_type_name(&ty.name) {
            self.error(
                "E003",
                name.span,
                format!("`{}` has no static function `{}`", ty.name, name.name),
            );
        } else {
            self.error("E003", ty.span, format!("unknown name `{}`", ty.name));
        }
    }

    /// `Enum.Variant` as a value: a unit variant.
    fn variant_value(&mut self, ty: &Ident, name: &Ident) {
        self.test_only_use(ty);
        if self
            .symbols
            .functions
            .contains_key(&format!("{}.{}", ty.name, name.name))
        {
            return;
        }
        match self.symbols.enums.get(&ty.name) {
            Some(variants) if variants.contains(&name.name) => {
                let payload = self.payload(&ty.name, &name.name);
                if payload > 0 {
                    self.error(
                        "E003",
                        name.span,
                        format!(
                            "variant `{}.{}` takes {}; call it",
                            ty.name,
                            name.name,
                            plural(payload, "payload value")
                        ),
                    );
                }
            }
            Some(_) => self.error(
                "E003",
                name.span,
                format!("enum `{}` has no variant `{}`", ty.name, name.name),
            ),
            None if self.symbols.protocols.contains_key(&ty.name) => self.error(
                "E003",
                ty.span,
                format!(
                    "protocol role `{}.{}` is not a value; call it",
                    ty.name, name.name
                ),
            ),
            None if self.is_type_name(&ty.name) => {
                self.error("E003", ty.span, format!("`{}` is not an enum", ty.name))
            }
            None => self.error("E003", ty.span, format!("unknown name `{}`", ty.name)),
        }
    }

    fn is_type_name(&self, name: &str) -> bool {
        BUILTIN_TYPES.contains(&name)
            || is_static_owner(name)
            || self.symbols.records.contains(name)
            || self.symbols.enums.contains_key(name)
    }

    // ---- patterns ---------------------------------------------------------

    /// Bind the names of `pattern` as `binding`; their values come from
    /// `roots`. An or-pattern binding a name is `E049`; the name is still
    /// bound, so the arm body reports nothing more.
    fn pattern(&mut self, pattern: &Pattern, roots: &BTreeSet<String>, binding: Binding) {
        match &pattern.kind {
            PatternKind::Wildcard
            | PatternKind::Int(_)
            | PatternKind::Str(_)
            | PatternKind::Bool(_) => {}
            PatternKind::Binding(name)
                if binding == Binding::Pattern && self.symbols.constants.contains(&name.name) =>
            {
                self.test_only_use(name)
            }
            PatternKind::Binding(name) => self.bind_rooted(name, binding, roots.clone()),
            PatternKind::Rest(Some(name)) => self.bind_rooted(name, binding, roots.clone()),
            PatternKind::Rest(None) => {}
            PatternKind::Range { start, end } => {
                self.pattern(start, roots, binding);
                self.pattern(end, roots, binding);
            }
            PatternKind::Tuple(items) | PatternKind::List(items) => {
                for item in items {
                    self.pattern(item, roots, binding);
                }
            }
            PatternKind::Or(alternatives) => {
                for alternative in alternatives {
                    if let Some(name) = first_binding(alternative) {
                        self.error(
                            "E049",
                            name.span,
                            format!(
                                "`{}` is bound inside an or-pattern; an or-pattern cannot bind names, match the alternatives in separate arms",
                                name.name
                            ),
                        );
                    }
                    self.pattern(alternative, roots, binding);
                }
            }
            PatternKind::Variant { path, args } => {
                match path.as_slice() {
                    [name] if self.symbols.constants.contains(&name.name) => {
                        self.test_only_use(name)
                    }
                    [name] => self.test_only_variant(name),
                    [ty, _] => self.test_only_use(ty),
                    _ => {}
                }
                if let [name] = path.as_slice()
                    && self.symbols.constants.contains(&name.name)
                {
                    if !args.is_empty() {
                        self.error(
                            "E003",
                            pattern.span,
                            format!(
                                "constant `{}` takes no payload patterns; a constant pattern is the name alone",
                                name.name
                            ),
                        );
                    }
                    for arg in args {
                        self.pattern(arg, roots, binding);
                    }
                    return;
                }
                let resolved = match path.as_slice() {
                    [variant] => match self.symbols.variants.get(&variant.name) {
                        None if self.out_of_scope_variant(&variant.name) => None,
                        None => {
                            self.error(
                                "E003",
                                variant.span,
                                format!("unknown variant `{}`", variant.name),
                            );
                            None
                        }
                        Some(owners) if owners.len() == 1 => {
                            Some((owners[0].clone(), variant.name.clone()))
                        }
                        Some(_) => None,
                    },
                    [ty, variant] => match self.symbols.enums.get(&ty.name) {
                        Some(variants) if variants.contains(&variant.name) => {
                            Some((ty.name.clone(), variant.name.clone()))
                        }
                        Some(_) => {
                            self.error(
                                "E003",
                                variant.span,
                                format!("enum `{}` has no variant `{}`", ty.name, variant.name),
                            );
                            None
                        }
                        None => {
                            self.error("E003", ty.span, format!("unknown enum `{}`", ty.name));
                            None
                        }
                    },
                    _ => {
                        let dotted = path
                            .iter()
                            .map(|s| s.name.as_str())
                            .collect::<Vec<_>>()
                            .join(".");
                        let span = path.first().map_or(pattern.span, |s| s.span);
                        self.error("E003", span, format!("`{dotted}` is not a variant"));
                        None
                    }
                };
                if let Some((owner, variant)) = resolved {
                    let payload = self.payload(&owner, &variant);
                    if payload != args.len() {
                        self.error(
                            "E003",
                            pattern.span,
                            format!(
                                "wrong number of payload patterns for `{variant}`: expected {payload}, found {}",
                                args.len()
                            ),
                        );
                    }
                }
                for arg in args {
                    self.pattern(arg, roots, binding);
                }
            }
        }
    }
}

/// Whether a `requires` set covers `effect`: it lists it, or lists `x.*`
/// for an effect `x.op` (`docs/slice.md` section 11.7). A needed `x.*` is
/// covered only by `x.*`.
pub fn covers<'a>(requires: impl IntoIterator<Item = &'a String> + Clone, effect: &str) -> bool {
    let head = effect.split_once('.').map(|(head, _)| head);
    requires.into_iter().any(|r| {
        r == effect || (head.is_some() && r.strip_suffix(".*").is_some_and(|h| Some(h) == head))
    })
}

/// Every name a block may call a module-level fn by: identifiers in value
/// position and method names (UFCS). Iterative, so long chains cost no
/// stack.
fn called_names(block: &Block) -> BTreeSet<String> {
    enum Node<'a> {
        Expr(&'a Expr),
        Block(&'a Block),
    }
    let mut names = BTreeSet::new();
    let mut stack = vec![Node::Block(block)];
    while let Some(node) = stack.pop() {
        let expr = match node {
            Node::Block(block) => {
                for stmt in &block.stmts {
                    match &stmt.kind {
                        StmtKind::Let { value, .. }
                        | StmtKind::LetPattern { value, .. }
                        | StmtKind::Assign { value, .. }
                        | StmtKind::Expr(value) => stack.push(Node::Expr(value)),
                        StmtKind::Return(value) => stack.extend(value.iter().map(Node::Expr)),
                        StmtKind::While { cond, body } => {
                            stack.push(Node::Expr(cond));
                            stack.push(Node::Block(body));
                        }
                        StmtKind::Loop { body } => stack.push(Node::Block(body)),
                        StmtKind::For { iter, body, .. }
                        | StmtKind::ForPattern { iter, body, .. } => {
                            stack.push(Node::Expr(iter));
                            stack.push(Node::Block(body));
                        }
                        StmtKind::Break | StmtKind::Continue => {}
                        StmtKind::Assert { cond, message } => {
                            stack.push(Node::Expr(cond));
                            stack.extend(message.iter().map(Node::Expr));
                        }
                    }
                }
                continue;
            }
            Node::Expr(expr) => expr,
        };
        match &expr.kind {
            ExprKind::Int(_) | ExprKind::Str(_) | ExprKind::Bool(_) => {}
            ExprKind::Name(name) => {
                names.insert(name.name.clone());
            }
            ExprKind::Record { fields, .. } => {
                stack.extend(fields.iter().map(|f| Node::Expr(&f.value)));
            }
            ExprKind::List(items) | ExprKind::Tuple(items) => {
                stack.extend(items.iter().map(Node::Expr));
            }
            ExprKind::Return(value) => stack.extend(value.iter().map(|v| Node::Expr(v))),
            ExprKind::Call { callee, args } => {
                stack.push(Node::Expr(callee));
                stack.extend(args.iter().map(Node::Expr));
            }
            ExprKind::Method {
                receiver,
                name,
                args,
            } => {
                names.insert(name.name.clone());
                if let ExprKind::Name(owner) = &receiver.kind {
                    names.insert(format!("{}.{}", owner.name, name.name));
                }
                stack.push(Node::Expr(receiver));
                stack.extend(args.iter().map(Node::Expr));
            }
            ExprKind::Field { receiver, .. }
            | ExprKind::Try(receiver)
            | ExprKind::Await(receiver)
            | ExprKind::Spawn(receiver)
            | ExprKind::Unary {
                operand: receiver, ..
            } => stack.push(Node::Expr(receiver)),
            ExprKind::Index { receiver, index } => {
                stack.push(Node::Expr(receiver));
                stack.push(Node::Expr(index));
            }
            ExprKind::Binary { left, right, .. } => {
                stack.push(Node::Expr(left));
                stack.push(Node::Expr(right));
            }
            ExprKind::Match { scrutinee, arms } => {
                stack.push(Node::Expr(scrutinee));
                stack.extend(arms.iter().map(|arm| Node::Expr(&arm.body)));
            }
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => {
                stack.push(Node::Expr(cond));
                stack.push(Node::Block(then));
                stack.extend(otherwise.iter().map(|o| Node::Expr(o)));
            }
            ExprKind::Block(block) => stack.push(Node::Block(block)),
            ExprKind::Closure(closure) => stack.push(Node::Expr(&closure.body)),
            ExprKind::RecordUpdate { fields, base, .. } => {
                stack.extend(fields.iter().map(|f| Node::Expr(&f.value)));
                stack.push(Node::Expr(base));
            }
            ExprKind::TupleField { receiver, .. } => stack.push(Node::Expr(receiver)),
            ExprKind::Break | ExprKind::Continue | ExprKind::Quantity { .. } => {}
            ExprKind::NamedArg { value, .. } => stack.push(Node::Expr(value)),
            ExprKind::Brace { entries } => {
                stack.extend(entries.iter().map(|e| Node::Expr(&e.value)));
            }
        }
    }
    names
}

/// The strongly connected components of the graph `edges` (node to its
/// successors), each listed after every component it reaches: callees
/// first. Tarjan's algorithm, iterative.
fn components(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = edges.len();
    let mut index = vec![usize::MAX; n];
    let mut low = vec![0; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut out = Vec::new();
    let mut next = 0;
    for root in 0..n {
        if index[root] != usize::MAX {
            continue;
        }
        let mut work: Vec<(usize, usize)> = vec![(root, 0)];
        index[root] = next;
        low[root] = next;
        next += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(&mut (v, ref mut child)) = work.last_mut() {
            if let Some(&w) = edges[v].get(*child) {
                *child += 1;
                if index[w] == usize::MAX {
                    index[w] = next;
                    low[w] = next;
                    next += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    work.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(index[w]);
                }
                continue;
            }
            work.pop();
            if let Some(&(parent, _)) = work.last() {
                low[parent] = low[parent].min(low[v]);
            }
            if low[v] == index[v] {
                let mut component = Vec::new();
                while let Some(w) = stack.pop() {
                    on_stack[w] = false;
                    component.push(w);
                    if w == v {
                        break;
                    }
                }
                out.push(component);
            }
        }
    }
    out
}

/// The address of an expression, a key for [`Checker::tail_roots`].
fn expr_key(expr: &Expr) -> usize {
    expr as *const Expr as usize
}

/// `Name<A, B>` written out, to tell type instances apart.
fn type_key(ty: &Type) -> String {
    if ty.args.is_empty() {
        return ty.name.name.clone();
    }
    let args: Vec<String> = ty.args.iter().map(type_key).collect();
    format!("{}<{}>", ty.name.name, args.join(","))
}

/// `ty` with every type named `params[i]` (without arguments) replaced by
/// `args[i]`.
pub fn substitute_type(ty: &Type, params: &[String], args: &[Type]) -> Type {
    if ty.args.is_empty()
        && let Some(i) = params.iter().position(|p| *p == ty.name.name)
        && let Some(arg) = args.get(i)
    {
        return arg.clone();
    }
    Type {
        args: ty
            .args
            .iter()
            .map(|a| substitute_type(a, params, args))
            .collect(),
        ..ty.clone()
    }
}

/// The name a place `x`, `x.f`, `x[i]` or `x.0` (nested) is rooted in.
fn place_root(expr: &Expr) -> Option<&Ident> {
    let mut current = expr;
    loop {
        match &current.kind {
            ExprKind::Name(name) => return Some(name),
            ExprKind::Field { receiver, .. }
            | ExprKind::Index { receiver, .. }
            | ExprKind::TupleField { receiver, .. } => current = receiver,
            _ => return None,
        }
    }
}

/// The methods of `ByteStream` (`docs/slice.md` 18.3), which `TcpStream`,
/// `PipeStream` and a `ByteStream`-bounded type parameter have.
fn is_stream_method(method: &str) -> bool {
    matches!(
        method,
        "read" | "write_all" | "close" | "set_idle_timeout" | "write_urgent" | "urgent_pending"
    )
}

/// Whether the builtin method `method` applies to a receiver of declared
/// type `ty` (`docs/slice.md` sections 6, 7 and 13): the table of
/// `ignite-types`' builtins by type name. A type parameter or a fn type
/// takes none but `copy`; [`Checker::builtin_method`] gives a
/// `ByteStream`-bounded one the stream methods. Public so that
/// `ignite-types` can compare it with its own table
/// (`builtins::tests::checker_table_matches_the_type_checker`).
pub fn builtin_applies(ty: &Type, method: &str) -> bool {
    let name = ty.name.name.as_str();
    if method == "copy" || method == "debug" {
        return true;
    }
    let text = ty.args.first().is_some_and(|a| a.name.name == "Text");
    let byte = ty.args.first().is_some_and(|a| a.name.name == "Byte");
    if CAPABILITY_METHODS
        .iter()
        .any(|(owner, m, _)| *owner == name && *m == method)
    {
        return true;
    }
    match name {
        "Text" => matches!(
            method,
            "len"
                | "is_empty"
                | "trim"
                | "starts_with"
                | "strip_prefix"
                | "concat"
                | "to_bytes"
                | "to_text"
        ),
        "Int" => matches!(
            method,
            "to_text"
                | "to_byte"
                | "wrapping_add"
                | "wrapping_sub"
                | "wrapping_mul"
                | "checked_add"
                | "checked_sub"
                | "checked_mul"
        ),
        "Byte" => matches!(method, "to_text" | "to_int"),
        "Bool" => method == "to_text",
        "Bytes" => matches!(
            method,
            "len"
                | "is_empty"
                | "at"
                | "slice"
                | "push"
                | "set"
                | "append"
                | "clear"
                | "map"
                | "to_text"
        ),
        "List" => {
            matches!(
                method,
                "len" | "is_empty" | "at" | "slice" | "push" | "set" | "append" | "clear" | "map"
            ) || (method == "join" && text)
                || (method == "to_text" && byte)
        }
        "Ref" | "Signal" => method == "to_text",
        "Store" => matches!(
            method,
            "next_id" | "get" | "put" | "remove" | "values" | "update" | "upsert"
        ),
        "TcpListener" => matches!(method, "accept" | "local_port"),
        "TcpStream" => is_stream_method(method) || matches!(method, "peer" | "timed_out"),
        "PipeStream" => is_stream_method(method) || method == "received_batches",
        "Map" => matches!(
            method,
            "get" | "put" | "remove" | "keys" | "len" | "contains"
        ),
        "Duration" => matches!(method, "to_ms" | "to_ns"),
        "ByteSize" => method == "to_int",
        "Ratio" => method == "ppm",
        "IoError" => method == "with_exit_code",
        _ => false,
    }
}

/// The first name `pattern` binds, if any.
fn first_binding(pattern: &Pattern) -> Option<&Ident> {
    match &pattern.kind {
        PatternKind::Binding(name) => Some(name),
        PatternKind::Rest(name) => name.as_ref(),
        PatternKind::Tuple(items) | PatternKind::List(items) | PatternKind::Or(items) => {
            items.iter().find_map(first_binding)
        }
        PatternKind::Variant { args, .. } => args.iter().find_map(first_binding),
        PatternKind::Wildcard
        | PatternKind::Range { .. }
        | PatternKind::Int(_)
        | PatternKind::Str(_)
        | PatternKind::Bool(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ignite_syntax::ast::{
        Arm, BinaryOp, ConstDecl, EnumDecl, FnDecl, Param, Place, Span, Variant,
    };

    fn sp() -> Span {
        Span::default()
    }

    fn id(name: &str) -> Ident {
        Ident {
            name: name.to_string(),
            span: sp(),
        }
    }

    fn ty(name: &str) -> Type {
        Type::named(id(name), Vec::new(), sp())
    }

    fn ex(kind: ExprKind) -> Expr {
        Expr::new(kind, sp())
    }

    fn name(n: &str) -> Expr {
        ex(ExprKind::Name(id(n)))
    }

    fn int(i: i64) -> Expr {
        ex(ExprKind::Int(i))
    }

    fn call(f: &str, args: Vec<Expr>) -> Expr {
        ex(ExprKind::Call {
            callee: Box::new(name(f)),
            args,
        })
    }

    fn method(receiver: Expr, m: &str, args: Vec<Expr>) -> Expr {
        ex(ExprKind::Method {
            receiver: Box::new(receiver),
            name: id(m),
            args,
        })
    }

    fn st(kind: StmtKind) -> Stmt {
        Stmt { kind, span: sp() }
    }

    fn let_(n: &str, value: Expr) -> Stmt {
        st(StmtKind::Let {
            name: id(n),
            ty: None,
            value,
            mutable: false,
        })
    }

    fn var(n: &str, value: Expr) -> Stmt {
        st(StmtKind::Let {
            name: id(n),
            ty: None,
            value,
            mutable: true,
        })
    }

    fn assign(n: &str, value: Expr) -> Stmt {
        st(StmtKind::Assign {
            target: Place::new(id(n), Vec::new(), sp()),
            value,
        })
    }

    fn expr_stmt(e: Expr) -> Stmt {
        st(StmtKind::Expr(e))
    }

    fn block(stmts: Vec<Stmt>) -> Block {
        Block { stmts, span: sp() }
    }

    fn func(n: &str, params: &[&str], effectful: bool, stmts: Vec<Stmt>) -> Decl {
        Decl::Fn(FnDecl {
            satisfies: Vec::new(),
            name: id(n),
            params: params
                .iter()
                .map(|p| Param {
                    name: id(p),
                    ty: ty("Int"),
                    span: sp(),
                })
                .collect(),
            ret: None,
            requires: effectful.then(Vec::new),
            body: block(stmts),
            span: sp(),
            type_params: Vec::new(),
            test_only: false,
            owner: None,
            bounds: Vec::new(),
        })
    }

    fn enum_(n: &str, variants: &[(&str, usize)]) -> Decl {
        Decl::Enum(EnumDecl {
            name: id(n),
            variants: variants
                .iter()
                .map(|(v, payload)| Variant {
                    name: id(v),
                    payload: (0..*payload).map(|_| ty("Int")).collect(),
                    span: sp(),
                })
                .collect(),
            type_params: Vec::new(),
            test_only: false,
            span: sp(),
        })
    }

    fn program(decls: Vec<Decl>) -> Program {
        Program {
            package: None,
            imports: Vec::new(),
            decls,
        }
    }

    fn codes(decls: Vec<Decl>) -> Vec<&'static str> {
        match check(&program(decls)) {
            Ok(_) => Vec::new(),
            Err(diagnostics) => diagnostics.iter().map(|d| d.code).collect(),
        }
    }

    #[test]
    fn let_shadows_and_inner_block_does_not_leak() {
        let inner = ex(ExprKind::Block(block(vec![let_("y", int(1))])));
        let decls = vec![func(
            "f",
            &["x"],
            false,
            vec![
                let_("x", name("x")),
                let_("x", name("x")),
                expr_stmt(inner),
                expr_stmt(name("y")),
            ],
        )];
        assert_eq!(codes(decls), vec!["E003"]);
    }

    #[test]
    fn let_value_is_checked_before_binding() {
        let decls = vec![func("f", &[], false, vec![let_("x", name("x"))])];
        assert_eq!(codes(decls), vec!["E003"]);
    }

    #[test]
    fn for_variable_is_scoped_to_its_body() {
        let for_stmt = st(StmtKind::For {
            name: id("item"),
            iter: ex(ExprKind::List(vec![int(1)])),
            body: block(vec![expr_stmt(name("item"))]),
        });
        let decls = vec![func(
            "f",
            &[],
            false,
            vec![for_stmt, expr_stmt(name("item"))],
        )];
        assert_eq!(codes(decls), vec!["E003"]);
    }

    #[test]
    fn arm_bindings_are_scoped_to_their_arm() {
        let arm = |pattern: PatternKind, body: Expr| Arm {
            pattern: Pattern::new(pattern, sp()),
            body,
            span: sp(),
        };
        let some = PatternKind::Variant {
            path: vec![id("Some")],
            args: vec![Pattern::new(PatternKind::Binding(id("v")), sp())],
        };
        let none = PatternKind::Variant {
            path: vec![id("None")],
            args: Vec::new(),
        };
        let matched = ex(ExprKind::Match {
            scrutinee: Box::new(call("Some", vec![int(1)])),
            arms: vec![arm(some, name("v")), arm(none, name("v"))],
        });
        let decls = vec![func(
            "f",
            &[],
            false,
            vec![expr_stmt(matched), expr_stmt(name("v"))],
        )];
        assert_eq!(codes(decls), vec!["E003", "E003"]);
    }

    #[test]
    fn assignment_needs_a_var() {
        let decls = vec![func(
            "f",
            &["p"],
            false,
            vec![
                let_("a", int(1)),
                var("b", int(1)),
                assign("a", int(2)),
                assign("b", int(2)),
                assign("p", int(2)),
                assign("missing", int(2)),
            ],
        )];
        assert_eq!(codes(decls), vec!["E003", "E006", "E006"]);
    }

    #[test]
    fn inner_let_shadows_outer_var_for_assignment() {
        let inner = ex(ExprKind::Block(block(vec![
            let_("x", int(1)),
            assign("x", int(2)),
        ])));
        let decls = vec![func(
            "f",
            &[],
            false,
            vec![var("x", int(0)), expr_stmt(inner), assign("x", int(3))],
        )];
        assert_eq!(codes(decls), vec!["E006"]);
    }

    #[test]
    fn effect_rules() {
        let decls = vec![
            func("io", &[], true, vec![]),
            func("inferred", &[], false, vec![expr_stmt(call("io", vec![]))]),
            func(
                "caller",
                &["s"],
                true,
                vec![
                    expr_stmt(call("io", vec![])),
                    expr_stmt(call("inferred", vec![])),
                    expr_stmt(method(name("s"), "read", vec![int(1)])),
                ],
            ),
            Decl::Const(ConstDecl {
                name: id("C"),
                ty: ty("Int"),
                value: call("inferred", vec![]),
                span: sp(),
                test_only: false,
            }),
            func("g", &[], false, vec![let_("h", name("inferred"))]),
        ];
        let mut found = codes(decls);
        found.sort();
        assert_eq!(
            found,
            vec!["E010", "E016"],
            "E017 is the type checker's since slice 7"
        );
    }

    #[test]
    fn ambiguous_variants_are_left_to_the_type_checker() {
        let decls = vec![
            enum_("A", &[("Go", 0), ("Go", 0)]),
            enum_("B", &[("Go", 0)]),
            func("f", &[], false, vec![expr_stmt(name("Go"))]),
        ];
        assert_eq!(codes(decls), vec!["E004"]);
    }

    #[test]
    fn path_calls_resolve_variants_and_statics() {
        let path = |t: &str, n: &str, args: Vec<Expr>| {
            ex(ExprKind::Call {
                callee: Box::new(ex(ExprKind::Field {
                    receiver: Box::new(name(t)),
                    name: id(n),
                })),
                args,
            })
        };
        let decls = vec![
            enum_("Cmd", &[("Echo", 1), ("Quit", 0)]),
            func(
                "f",
                &[],
                false,
                vec![
                    expr_stmt(path("Cmd", "Echo", vec![int(1)])),
                    expr_stmt(method(name("Cmd"), "Echo", vec![int(1)])),
                    expr_stmt(path("Bytes", "empty", vec![])),
                    expr_stmt(method(name("Int"), "to_text", vec![int(1)])),
                    expr_stmt(ex(ExprKind::Field {
                        receiver: Box::new(name("Cmd")),
                        name: id("Quit"),
                    })),
                    expr_stmt(path("Cmd", "Nope", vec![])),
                    expr_stmt(ex(ExprKind::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(call("Some", vec![])),
                        right: Box::new(name("Unit")),
                    })),
                ],
            ),
        ];
        assert_eq!(codes(decls), vec!["E003", "E003"]);
    }

    #[test]
    fn duplicate_top_level_names() {
        let decls = vec![
            func("f", &[], false, vec![]),
            func("f", &[], false, vec![]),
            enum_("Option", &[("X", 0)]),
        ];
        assert_eq!(codes(decls), vec!["E004", "E004"]);
    }
}
