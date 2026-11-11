//! The walk behind [`crate::type_module`]: name tables, signatures, then
//! every body, constant and system property.

use std::collections::{BTreeMap, HashMap, HashSet};

use ignite_check::{BUILTIN_METHODS, SUBSTRATE_MODULES, Symbols};
use ignite_syntax::ast::{
    AliasDecl, Arm, BinaryOp, Block, Closure, Decl, Expr, ExprKind, FieldInit, FnDecl, Ident,
    NodeId, Pattern, PatternKind, Place, Program, Span, Stmt, StmtKind, TestDecl, Type, UnaryOp,
};
use ignite_syntax::diagnostic::Diagnostic;

mod clauses;
mod effects;
mod system;

use crate::builtins::{self, bytes, io};
use crate::exhaustive;
use crate::ids::NodeMap;
use crate::{
    Alias, AssemblyInfo, AssertCompare, BraceKind, Builtin, Callee, Capability, Capture,
    ClosureInfo, EXPOSURE_VARIANTS, EffectSet, ExecutableInfo, FailureOp, FailureSite, FnEffects,
    FnSig, ModuleRecord, ModuleTypes, QualName, QuantityKind, Reference, Show, SystemInfo,
    TestInfo, TestTry, Ty, TypedModules, Variants, instantiate, key, variants_of,
};

/// What [`Typer::run`] produces for a module without diagnostics.
pub(crate) struct Typed {
    pub module: String,
    pub types: NodeMap<Ty>,
    pub callees: NodeMap<Callee>,
    pub bindings: HashMap<Span, Ty>,
    pub pattern_consts: NodeMap<QualName>,
    pub clause_consts: HashMap<Span, QualName>,
    pub type_exprs: NodeMap<Ty>,
    pub exports: ModuleTypes,
    pub instances: NodeMap<Vec<Ty>>,
    pub tests: Vec<TestInfo>,
    pub assert_compares: NodeMap<AssertCompare>,
    pub test_tries: NodeMap<TestTry>,
    pub closures: NodeMap<ClosureInfo>,
    pub ranges: NodeMap<(i64, i64)>,
    pub places: HashMap<Span, Ty>,
    pub system: SystemTables,
}

/// The slice-10 tables (`docs/slice.md` section 14.10), filled by the
/// walk and moved into [`crate::TypedProgram`].
#[derive(Default)]
pub(crate) struct SystemTables {
    pub quantities: NodeMap<(QuantityKind, i64)>,
    pub named: NodeMap<Vec<usize>>,
    pub braces: NodeMap<BraceKind>,
    pub references: NodeMap<Reference>,
    pub module_records: NodeMap<ModuleRecord>,
    pub executables: Vec<ExecutableInfo>,
    pub failures: NodeMap<FailureSite>,
    pub systems: Vec<SystemInfo>,
    pub assemblies: Vec<AssemblyInfo>,
    pub effects: NodeMap<EffectSet>,
}

/// How an expression is used. `Value(Some(t))` checks it against `t`,
/// `Value(None)` synthesises its type, `Stmt` is an expression statement
/// whose value is dropped: its `match` and `if` arms are not unified.
#[derive(Clone, Copy)]
enum Mode<'t> {
    Value(Option<&'t Ty>),
    Stmt,
}

/// The effects a fn passed to `List.map` may have where the walk is
/// (`docs/slice.md` 20.4.3): those of the code that calls `map`.
#[derive(Clone, Default)]
enum EffectScope {
    /// A constant, a field default, a property: pure, so `map` takes a
    /// pure fn only.
    #[default]
    Pure,
    /// A test body, a closure body or a fn without `requires`: any effect
    /// on a capture or ambient; `ignite-check` adds it to the inferred set.
    Open,
    /// The body of the fn or role `name` with these `requires` entries.
    Declared { name: String, requires: Vec<String> },
}

/// One arm of a `match` or `if`.
enum Branch<'e> {
    Block(&'e Block),
    Expr(&'e Expr),
    Arm(&'e Arm, Option<Ty>),
}

const BUILTIN_ENUMS: [&str; 4] = ["Option", "Result", "Input", "Exposure"];

/// The builtin enum `Exposure` (`docs/slice.md` section 14.4).
fn exposure() -> Ty {
    Ty::Enum(QualName::builtin("Exposure"))
}

/// Whether a value of type `found` may be used where `expected` is: the
/// same type, `Never`, a pure fn where an effectful fn type of its shape
/// is expected, an effectful fn whose effects the expected type covers by
/// parameter position, a `Ref<K>` where the kind-erased `Ref` is expected
/// (`docs/slice.md` section 14.3), and the same rules inside type
/// arguments.
fn compatible(found: &Ty, expected: &Ty) -> bool {
    if found == expected || *found == Ty::Never {
        return true;
    }
    let all =
        |a: &[Ty], b: &[Ty]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| compatible(x, y));
    match (found, expected) {
        (Ty::Fn(pa, ra), Ty::EffectFn(pb, rb, _)) => pa == pb && ra == rb,
        (Ty::EffectFn(pa, ra, ea), Ty::EffectFn(pb, rb, eb)) => {
            pa == pb && ra == rb && ea.covered_by(eb)
        }
        (Ty::List(a), Ty::List(b))
        | (Ty::Option(a), Ty::Option(b))
        | (Ty::Signal(a), Ty::Signal(b))
        | (Ty::Ref(Some(a)), Ty::Ref(Some(b))) => compatible(a, b),
        (Ty::Ref(Some(_)), Ty::Ref(None)) => true,
        (Ty::Result(a1, a2), Ty::Result(b1, b2)) | (Ty::Map(a1, a2), Ty::Map(b1, b2)) => {
            compatible(a1, b1) && compatible(a2, b2)
        }
        (Ty::Tuple(a), Ty::Tuple(b)) => all(a, b),
        (Ty::GenericRecord(qa, a), Ty::GenericRecord(qb, b))
        | (Ty::GenericEnum(qa, a), Ty::GenericEnum(qb, b)) => qa == qb && all(a, b),
        _ => false,
    }
}

fn idents(params: &[Ident]) -> Vec<String> {
    params.iter().map(|p| p.name.clone()).collect()
}

thread_local! {
    /// The record literals [`Typer::literal_waits`] has looked at on this
    /// thread, read by [`literal_wait_steps`].
    static LITERAL_WAIT_STEPS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// How many record literals the typer has looked at on this thread to
/// decide whether a literal argument waits for the arguments after it: a
/// count of work, so a test bounds that cost without a clock (row 2.6 of
/// `docs/quality.md`).
pub fn literal_wait_steps() -> u64 {
    LITERAL_WAIT_STEPS.with(|steps| steps.get())
}

/// Counts one record literal looked at by [`Typer::literal_waits`].
fn literal_wait_step() {
    LITERAL_WAIT_STEPS.with(|steps| steps.set(steps.get() + 1));
}

/// What binds a type parameter of a record literal from around it, for
/// [`Typer::wait_rules`]: the parameter of the call it is an argument of,
/// or the declared type `ty` of the field it is the value of, whose
/// parameters `tparams` of the literal around it have the facts `facts`.
enum Outside<'a> {
    Param(&'a Ty),
    Field {
        ty: &'a Ty,
        tparams: &'a [String],
        facts: &'a [usize],
    },
}

/// The rules of [`Typer::literal_waits`]: each type parameter of each
/// record literal walked is a fact, bound or not, and a rule binds its
/// conclusions once all of its premises are bound. [`WaitRules::propagate`]
/// looks at each rule once per premise, so the cost is linear in the rules.
#[derive(Default)]
struct WaitRules {
    bound: Vec<bool>,
    /// Per rule, how many of its premises are not bound yet, and its
    /// conclusions.
    rules: Vec<(usize, Vec<usize>)>,
    /// Per fact, the rules it is a premise of.
    watchers: Vec<Vec<usize>>,
    /// Facts bound and not yet propagated.
    ready: Vec<usize>,
}

impl WaitRules {
    fn fact(&mut self) -> usize {
        self.bound.push(false);
        self.watchers.push(Vec::new());
        self.bound.len() - 1
    }

    fn bind(&mut self, fact: usize) {
        if !self.bound[fact] {
            self.bound[fact] = true;
            self.ready.push(fact);
        }
    }

    fn rule(&mut self, mut premises: Vec<usize>, conclusions: Vec<usize>) {
        premises.sort_unstable();
        premises.dedup();
        if premises.is_empty() {
            for fact in conclusions {
                self.bind(fact);
            }
            return;
        }
        let id = self.rules.len();
        for &fact in &premises {
            self.watchers[fact].push(id);
        }
        self.rules.push((premises.len(), conclusions));
    }

    fn propagate(&mut self) {
        while let Some(fact) = self.ready.pop() {
            for id in std::mem::take(&mut self.watchers[fact]) {
                self.rules[id].0 -= 1;
                if self.rules[id].0 == 0 {
                    for bound in std::mem::take(&mut self.rules[id].1) {
                        self.bind(bound);
                    }
                }
            }
        }
    }
}

/// Whether `name` is an inference variable of [`Typer::infer`].
fn is_var(name: &str) -> bool {
    name.starts_with('\'')
}

/// `ty` with the bound inference variables replaced.
fn apply(ty: &Ty, binds: &BTreeMap<String, Ty>) -> Ty {
    ty.map_params(&|name| binds.get(name).cloned())
}

/// Bind the inference variables of `pattern` from the type `found`
/// matched against it. A structural mismatch binds nothing; the check that
/// follows reports it.
fn unify(pattern: &Ty, found: &Ty, binds: &mut BTreeMap<String, Ty>) {
    let each = |a: &[Ty], b: &[Ty], binds: &mut BTreeMap<String, Ty>| {
        if a.len() == b.len() {
            for (x, y) in a.iter().zip(b) {
                unify(x, y, binds);
            }
        }
    };
    match (pattern, found) {
        (_, Ty::Never) => {}
        (Ty::Param(p), _) if is_var(p) => {
            if !binds.contains_key(p) && !found.has_param(&is_var) {
                binds.insert(p.clone(), found.clone());
            }
        }
        (Ty::List(a), Ty::List(b))
        | (Ty::Option(a), Ty::Option(b))
        | (Ty::Store(a), Ty::Store(b))
        | (Ty::Sender(a), Ty::Sender(b))
        | (Ty::Receiver(a), Ty::Receiver(b))
        | (Ty::Signal(a), Ty::Signal(b))
        | (Ty::Ref(Some(a)), Ty::Ref(Some(b))) => unify(a, b, binds),
        (Ty::Result(a1, a2), Ty::Result(b1, b2)) | (Ty::Map(a1, a2), Ty::Map(b1, b2)) => {
            unify(a1, b1, binds);
            unify(a2, b2, binds);
        }
        (Ty::Tuple(a), Ty::Tuple(b)) => each(a, b, binds),
        (Ty::GenericRecord(qa, a), Ty::GenericRecord(qb, b))
        | (Ty::GenericEnum(qa, a), Ty::GenericEnum(qb, b))
            if qa == qb =>
        {
            each(a, b, binds)
        }
        (Ty::Fn(pa, ra) | Ty::EffectFn(pa, ra, _), Ty::Fn(pb, rb) | Ty::EffectFn(pb, rb, _)) => {
            each(pa, pb, binds);
            unify(ra, rb, binds);
        }
        _ => {}
    }
}

fn fn_ty(params: Vec<Ty>, ret: Ty) -> Ty {
    Ty::Fn(params, Box::new(ret))
}

/// An integer literal, possibly negated: typed after the other operand.
fn is_literal(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Int(_) => true,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            operand,
        } => matches!(operand.kind, ExprKind::Int(_)),
        _ => false,
    }
}

/// `[]`: a left operand typed after the right one, so `[] + l` takes the
/// element type of `l` (`docs/slice.md` 25.5).
fn is_empty_list(expr: &Expr) -> bool {
    matches!(&expr.kind, ExprKind::List(items) if items.is_empty())
}

/// Whether the value of `expr` needs an expected type to be typed: `None`,
/// `[]`, `Ok(..)`, `Err(..)`, `List.empty()`, a bare variant `ambiguous`
/// holds for, or a block or `if` ending in one.
fn needs_expected(expr: &Expr, ambiguous: &dyn Fn(&str) -> bool) -> bool {
    let is = |e: &Expr, name: &str| matches!(&e.kind, ExprKind::Name(id) if id.name == name);
    match &expr.kind {
        ExprKind::Name(id) => id.name == "None" || ambiguous(&id.name),
        ExprKind::List(items) => items.is_empty(),
        ExprKind::Call { callee, .. } => {
            is(callee, "Ok")
                || is(callee, "Err")
                || matches!(&callee.kind, ExprKind::Name(id) if ambiguous(&id.name))
                || matches!(&callee.kind, ExprKind::Field { receiver, name } if is(receiver, "List") && matches!(name.name.as_str(), "empty" | "with_capacity"))
                || matches!(&callee.kind, ExprKind::Field { receiver, name } if is(receiver, "Map") && name.name == "empty")
                || matches!(&callee.kind, ExprKind::Field { receiver, name } if is(receiver, "Store") && name.name == "new")
                || matches!(&callee.kind, ExprKind::Field { receiver, name } if is(receiver, "Channel") && name.name == "bounded")
                || matches!(&callee.kind, ExprKind::Name(id) if id.name == "module")
        }
        ExprKind::Method { receiver, name, .. } => {
            is(receiver, "List") && matches!(name.name.as_str(), "empty" | "with_capacity")
                || is(receiver, "Map") && name.name == "empty"
                || is(receiver, "Store") && name.name == "new"
                || is(receiver, "Channel") && name.name == "bounded"
        }
        ExprKind::Brace { .. } => true,
        ExprKind::Block(block) => block_needs_expected(block, ambiguous),
        ExprKind::If { then, .. } => block_needs_expected(then, ambiguous),
        _ => false,
    }
}

fn block_needs_expected(block: &Block, ambiguous: &dyn Fn(&str) -> bool) -> bool {
    matches!(block.stmts.last(), Some(Stmt { kind: StmtKind::Expr(e), .. }) if needs_expected(e, ambiguous))
}

impl Branch<'_> {
    fn needs_expected(&self, ambiguous: &dyn Fn(&str) -> bool) -> bool {
        match self {
            Branch::Block(block) => block_needs_expected(block, ambiguous),
            Branch::Expr(expr) => needs_expected(expr, ambiguous),
            Branch::Arm(arm, _) => needs_expected(&arm.body, ambiguous),
        }
    }
}

/// `ty` with every alias parameter `params[i]` replaced by `args[i]`.
fn substitute(ty: &Ty, params: &[String], args: &[Ty]) -> Ty {
    let sub = |t: &Ty| Box::new(substitute(t, params, args));
    match ty {
        Ty::Record(q) => q
            .alias_parameter_name()
            .and_then(|name| params.iter().position(|p| p == name))
            .and_then(|i| args.get(i).cloned())
            .unwrap_or_else(|| ty.clone()),
        Ty::List(t) => Ty::List(sub(t)),
        Ty::Option(t) => Ty::Option(sub(t)),
        Ty::Result(ok, err) => Ty::Result(sub(ok), sub(err)),
        Ty::Fn(ps, ret) => Ty::Fn(
            ps.iter().map(|p| substitute(p, params, args)).collect(),
            sub(ret),
        ),
        Ty::EffectFn(ps, ret, effects) => Ty::EffectFn(
            ps.iter().map(|p| substitute(p, params, args)).collect(),
            sub(ret),
            effects.clone(),
        ),
        Ty::Store(t) => Ty::Store(sub(t)),
        Ty::Sender(t) => Ty::Sender(sub(t)),
        Ty::Receiver(t) => Ty::Receiver(sub(t)),
        Ty::Signal(t) => Ty::Signal(sub(t)),
        Ty::Ref(Some(t)) => Ty::Ref(Some(sub(t))),
        Ty::Map(k, v) => Ty::Map(sub(k), sub(v)),
        Ty::Tuple(items) => Ty::Tuple(items.iter().map(|t| substitute(t, params, args)).collect()),
        Ty::GenericRecord(q, items) => Ty::GenericRecord(
            q.clone(),
            items.iter().map(|t| substitute(t, params, args)).collect(),
        ),
        Ty::GenericEnum(q, items) => Ty::GenericEnum(
            q.clone(),
            items.iter().map(|t| substitute(t, params, args)).collect(),
        ),
        _ => ty.clone(),
    }
}

pub(crate) struct Typer<'a> {
    module: String,
    symbols: &'a Symbols,
    modules: &'a TypedModules,
    /// This module's signatures, filled before any body is walked.
    own: ModuleTypes,
    records: BTreeMap<String, QualName>,
    enums: BTreeMap<String, QualName>,
    fns: BTreeMap<String, QualName>,
    consts: BTreeMap<String, QualName>,
    protocols: BTreeMap<String, QualName>,
    aliases: BTreeMap<String, QualName>,
    /// This module's alias declarations, resolved on first use.
    alias_decls: BTreeMap<String, &'a AliasDecl>,
    /// Aliases being resolved, innermost last: a cycle is E048.
    resolving: Vec<String>,
    /// Type parameters of the alias whose target is being resolved.
    alias_params: Vec<String>,
    /// Type parameters of the generic declaration being resolved or walked.
    type_params: Vec<String>,
    /// The type parameters of the fn or role being walked that are bounded
    /// by `ByteStream` (`docs/slice.md` 18.3).
    bounded: Vec<String>,
    /// Type arguments of generic uses, by expression id.
    instances: NodeMap<Vec<Ty>>,
    /// Local bindings; `None` when the binding's type is unknown after an
    /// error.
    scopes: Vec<BTreeMap<String, Option<Ty>>>,
    /// Return type of the function being walked; `None` outside one.
    ret: Option<Ty>,
    /// One entry per enclosing loop: whether a `break` targets it.
    loops: Vec<bool>,
    /// Whether the pattern being checked is a `match` arm's: there a bare
    /// name that is a constant is a constant pattern, not a binding.
    in_arm: bool,
    types: NodeMap<Ty>,
    callees: NodeMap<Callee>,
    bindings: HashMap<Span, Ty>,
    pattern_consts: NodeMap<QualName>,
    /// The constant each clause name resolves to, by the span of the
    /// reference (`docs/slice.md` 19.6).
    clause_consts: HashMap<Span, QualName>,
    type_exprs: NodeMap<Ty>,
    diagnostics: Vec<Diagnostic>,
    /// Whether the walk is inside a test body or a test-only declaration,
    /// where test-only items may be used (E051).
    in_test: bool,
    /// Whether the walk is directly inside a test body, where `?` fails the
    /// test instead of returning.
    in_test_body: bool,
    /// The effects of the code being walked, for `List.map`'s fn.
    effect_scope: EffectScope,
    tests: Vec<TestInfo>,
    assert_compares: NodeMap<AssertCompare>,
    /// Complete structural-equality results. Nested coinductive visits must
    /// not be cached, because a parent may still contain an unequal field.
    equatable_cache: HashMap<Ty, bool>,
    test_tries: NodeMap<TestTry>,
    closures: NodeMap<ClosureInfo>,
    ranges: NodeMap<(i64, i64)>,
    /// The type that owns the last field of an assignment place, by
    /// the place's span.
    places: HashMap<Span, Ty>,
    /// The slice-10 tables.
    system: SystemTables,
    /// The span of the name of each binding, by scope index and name.
    binding_spans: HashMap<(usize, String), Span>,
    /// One entry per closure being walked, innermost last: the number of
    /// scopes open outside it and the captures found so far.
    closure_frames: Vec<(usize, Vec<Capture>)>,
    /// Set by a value whose type parameter neither the expected type nor
    /// its arguments decide (`[]`, `None`, `List.empty()`, ..): the body is
    /// walked again to let later uses decide (`docs/slice.md` section 13,
    /// row 3).
    hole_wanted: bool,
    /// During that probe walk: the open holes and what uses bound them.
    holes: Option<Holes>,
    /// `let` bindings whose type the probe walk decided, by name span.
    let_hints: HashMap<Span, Ty>,
    /// `NamedArg` nodes a fn call put in parameter order (`docs/slice.md`
    /// section 14.2); any other `NamedArg` is E103.
    named_ok: HashSet<NodeId>,
    /// The members of every system of this module, by system name, in
    /// declaration order (`docs/slice.md` section 14.4).
    system_members: BTreeMap<String, Vec<system::Member>>,
    /// The system whose declaration is being walked.
    current_system: Option<String>,
    /// While a member's properties are walked: the members it uses through
    /// bare names, each with the type the use has.
    member_uses: Option<Vec<(String, Ty)>>,
    /// The system named by the `system` property of the assembly being
    /// walked, for E108.
    assembly_system: Option<String>,
}

/// Inference holes of the row-3 probe walk: `'?n` type variables standing
/// for an undecided type parameter, and the types uses bound them to.
#[derive(Default)]
struct Holes {
    next: usize,
    solved: BTreeMap<String, Ty>,
    /// The name span and type of each `let` whose value has a hole.
    lets: Vec<(Span, Ty)>,
}

fn is_hole(name: &str) -> bool {
    name.starts_with("'?")
}

impl<'a> Typer<'a> {
    pub(crate) fn new(
        program: &'a Program,
        symbols: &'a Symbols,
        modules: &'a TypedModules,
    ) -> Typer<'a> {
        let module = program
            .package
            .as_ref()
            .map_or_else(|| "main".to_string(), |p| p.dotted());
        Typer {
            module,
            symbols,
            modules,
            own: ModuleTypes::default(),
            records: BTreeMap::new(),
            enums: BTreeMap::new(),
            fns: BTreeMap::new(),
            consts: BTreeMap::new(),
            protocols: BTreeMap::new(),
            aliases: BTreeMap::new(),
            alias_decls: BTreeMap::new(),
            resolving: Vec::new(),
            alias_params: Vec::new(),
            type_params: Vec::new(),
            bounded: Vec::new(),
            instances: NodeMap::default(),
            scopes: Vec::new(),
            ret: None,
            loops: Vec::new(),
            in_arm: false,
            types: NodeMap::default(),
            callees: NodeMap::default(),
            bindings: HashMap::new(),
            pattern_consts: NodeMap::default(),
            clause_consts: HashMap::new(),
            type_exprs: NodeMap::default(),
            diagnostics: Vec::new(),
            in_test: false,
            in_test_body: false,
            effect_scope: EffectScope::Pure,
            tests: Vec::new(),
            assert_compares: NodeMap::default(),
            equatable_cache: HashMap::new(),
            test_tries: NodeMap::default(),
            closures: NodeMap::default(),
            ranges: NodeMap::default(),
            places: HashMap::new(),
            system: SystemTables::default(),
            binding_spans: HashMap::new(),
            closure_frames: Vec::new(),
            hole_wanted: false,
            holes: None,
            let_hints: HashMap::new(),
            named_ok: HashSet::new(),
            system_members: BTreeMap::new(),
            current_system: None,
            member_uses: None,
            assembly_system: None,
        }
    }

    /// Probe local inference during the first body walk. Bodies without a
    /// hole finish in one pass; otherwise resolve the binding hints and
    /// verify once more without holes. The final walk owns diagnostics.
    fn with_local_inference(&mut self, walk: &dyn Fn(&mut Self)) {
        let diagnostics = self.diagnostics.len();
        let tests = self.tests.len();
        self.hole_wanted = false;
        self.holes = Some(Holes::default());
        walk(self);
        let holes = self.holes.take().unwrap_or_default();
        if holes.next == 0 && !self.hole_wanted {
            return;
        }
        self.diagnostics.truncate(diagnostics);
        self.tests.truncate(tests);
        for (span, ty) in &holes.lets {
            let mut ty = ty.clone();
            for _ in 0..=holes.solved.len() {
                ty = apply(&ty, &holes.solved);
            }
            if !ty.has_param(&is_hole) {
                self.let_hints.insert(*span, ty);
            }
        }
        walk(self);
        self.let_hints.clear();
        self.hole_wanted = false;
    }

    /// A fresh hole during the probe walk; `None` outside it, after noting
    /// that a probe walk is wanted.
    fn hole(&mut self) -> Option<Ty> {
        match &mut self.holes {
            Some(holes) => {
                holes.next += 1;
                Some(Ty::Param(format!("'?{}", holes.next)))
            }
            None => {
                self.hole_wanted = true;
                None
            }
        }
    }

    pub(crate) fn run(mut self, program: &'a Program) -> Result<Typed, Vec<Diagnostic>> {
        self.names(program);
        self.signatures(program);
        self.bodies(program);
        self.clauses(program);
        if self.diagnostics.is_empty() {
            Ok(Typed {
                module: self.module,
                types: self.types,
                callees: self.callees,
                bindings: self.bindings,
                pattern_consts: self.pattern_consts,
                clause_consts: self.clause_consts,
                type_exprs: self.type_exprs,
                exports: self.own,
                instances: self.instances,
                tests: self.tests,
                assert_compares: self.assert_compares,
                test_tries: self.test_tries,
                closures: self.closures,
                ranges: self.ranges,
                places: self.places,
                system: self.system,
            })
        } else {
            let mut diagnostics = self.diagnostics;
            diagnostics.sort_by_key(|d| (d.span.start, d.code));
            diagnostics.dedup();
            Err(diagnostics)
        }
    }

    fn error(&mut self, code: &'static str, span: Span, message: String) {
        self.diagnostics.push(Diagnostic::new(code, span, message));
    }

    fn mismatch(&mut self, span: Span, expected: &Ty, found: &Ty) {
        self.error(
            "E040",
            span,
            format!("type mismatch: expected `{expected}`, found `{found}`"),
        );
    }

    /// A name `ignite-check` should have rejected; reported so that a module
    /// with an untyped expression never types successfully.
    fn unresolved(&mut self, span: Span, what: &str) {
        self.error("E040", span, format!("cannot type `{what}`"));
    }

    // ---- generics and effects ---------------------------------------------

    /// The type arguments of a use of the generic `what` with type
    /// parameters `tparams` (`docs/slice.md` section 11.7). `pairs` are
    /// argument expressions with their declared types, `pre` values typed
    /// already (a UFCS receiver) with theirs, `ret` the declared result
    /// type, matched against `expected` first. Every argument of `pairs` is
    /// typed: one whose declared type is a fn type or which needs an
    /// expected type after the others, a record literal with a type
    /// parameter of its own that nothing binds yet after the plain
    /// arguments. E042 for a parameter left unbound.
    #[allow(clippy::too_many_arguments)]
    fn infer(
        &mut self,
        span: Span,
        what: &str,
        tparams: &[String],
        pairs: &[(&Expr, Ty)],
        pre: &[(Ty, Ty)],
        ret: &Ty,
        expected: Option<&Ty>,
    ) -> Option<Vec<Ty>> {
        let vars: Vec<Ty> = tparams.iter().map(|p| Ty::Param(format!("'{p}"))).collect();
        let fresh = |t: &Ty| t.substitute(tparams, &vars);
        let mut binds = BTreeMap::new();
        if let Some(expected) = expected {
            unify(&fresh(ret), expected, &mut binds);
        }
        for (found, declared) in pre {
            unify(&fresh(declared), found, &mut binds);
        }
        // The pass an argument is typed in: 0 in order, 2 after the others
        // (a fn type or a value that needs an expected type), 1 a record
        // literal that waited in pass 0.
        let mut stage: Vec<u8> = {
            let ambiguous = |n: &str| self.is_ambiguous_variant(n);
            pairs
                .iter()
                .map(|(arg, ty)| {
                    let last = needs_expected(arg, &ambiguous)
                        || matches!(ty, Ty::Fn(..) | Ty::EffectFn(..));
                    if last { 2 } else { 0 }
                })
                .collect()
        };
        for pass in 0..3u8 {
            for (i, (arg, declared)) in pairs.iter().enumerate() {
                if stage[i] != pass {
                    continue;
                }
                let param = apply(&fresh(declared), &binds);
                let open = param.has_param(&is_var);
                let ambiguous = |n: &str| self.is_ambiguous_variant(n);
                if !open || needs_expected(arg, &ambiguous) {
                    self.check(arg, &param);
                    continue;
                }
                // A record literal with a type parameter nothing binds yet
                // is typed after the arguments that follow it, as a closure
                // is: typed now it is E042 whatever they give.
                if pass == 0 && self.literal_waits(arg, &param) {
                    stage[i] = 1;
                    continue;
                }
                // A closure and a record literal are typed against the
                // parameter as far as it is known: a record literal binds
                // a type parameter of its own that only the call gives
                // (a std record's stream type from the call's stream).
                let given = matches!(arg.kind, ExprKind::Closure(_) | ExprKind::Record { .. })
                    .then_some(&param);
                let Some(found) = self.expr(arg, Mode::Value(given)) else {
                    continue;
                };
                unify(&param, &found, &mut binds);
                let param = apply(&param, &binds);
                if !param.has_param(&is_var) && !compatible(&found, &param) {
                    self.mismatch(arg.span, &param, &found);
                }
            }
        }
        let mut instance = Vec::new();
        for (param, var) in tparams.iter().zip(&vars) {
            match binds.get(&var.to_string()) {
                Some(ty) => instance.push(ty.clone()),
                None => {
                    self.error(
                        "E042",
                        span,
                        format!(
                            "cannot infer type parameter `{param}` of `{what}`; annotate the binding"
                        ),
                    );
                    return None;
                }
            }
        }
        Some(instance)
    }

    /// Whether the record literal `arg`, typed now against `param`, leaves
    /// one of its own type parameters unbound: one that `param` leaves open
    /// and that no given field binds. A field binds the parameters its
    /// declared type mentions, unless its value is a record literal that
    /// waits by the same rule, with what `param` and the other fields bind.
    /// One walk of the literal collects these as rules ([`WaitRules`]) and
    /// one pass propagates them, so the cost is linear in the literal.
    fn literal_waits(&self, arg: &Expr, param: &Ty) -> bool {
        let mut rules = WaitRules::default();
        let Some(facts) = self.wait_rules(arg, Outside::Param(param), &mut rules) else {
            return false;
        };
        rules.propagate();
        facts.iter().any(|&fact| !rules.bound[fact])
    }

    /// The facts of the record literal `arg`, one per type parameter of its
    /// record, with the rules of its fields added to `rules`; `None` when
    /// `arg` is no literal of a known record, which waits for nothing.
    /// `outside` says what binds a parameter from around the literal.
    fn wait_rules(
        &self,
        arg: &Expr,
        outside: Outside<'_>,
        rules: &mut WaitRules,
    ) -> Option<Vec<usize>> {
        let ExprKind::Record { name, fields } = &arg.kind else {
            return None;
        };
        let q = self.records.get(&name.name)?;
        literal_wait_step();
        let tparams = self.generics_of(q);
        let facts: Vec<usize> = tparams.iter().map(|_| rules.fact()).collect();
        if tparams.is_empty() {
            return Some(facts);
        }
        match outside {
            Outside::Param(Ty::GenericRecord(pq, args))
                if pq == q && args.len() == tparams.len() =>
            {
                for (fact, arg) in facts.iter().zip(args) {
                    if !arg.has_param(&is_var) {
                        rules.rule(Vec::new(), vec![*fact]);
                    }
                }
            }
            Outside::Field {
                ty: Ty::GenericRecord(pq, args),
                tparams: around,
                facts: around_facts,
            } if pq == q && args.len() == tparams.len() => {
                for (fact, arg) in facts.iter().zip(args) {
                    let premises = around
                        .iter()
                        .zip(around_facts)
                        .filter(|(p, _)| arg.has_param(&|x| x == p.as_str()))
                        .map(|(_, f)| *f)
                        .collect();
                    rules.rule(premises, vec![*fact]);
                }
            }
            _ => {}
        }
        let declared = self.record_fields(q).unwrap_or_default();
        for field in fields {
            let Some((_, ty)) = declared.iter().find(|(n, _)| *n == field.name.name) else {
                continue;
            };
            let binds: Vec<usize> = tparams
                .iter()
                .zip(&facts)
                .filter(|(p, _)| ty.has_param(&|x| x == p.as_str()))
                .map(|(_, f)| *f)
                .collect();
            if binds.is_empty() {
                continue;
            }
            let inner = Outside::Field {
                ty,
                tparams: &tparams,
                facts: &facts,
            };
            let premises = self
                .wait_rules(&field.value, inner, rules)
                .unwrap_or_default();
            rules.rule(premises, binds);
        }
        Some(facts)
    }

    /// Arguments of a call of the generic `what` (E043 on a count
    /// mismatch); the instance is recorded on `expr` and the instantiated
    /// result type returned.
    #[allow(clippy::too_many_arguments)]
    fn generic_call(
        &mut self,
        expr: &Expr,
        what: &str,
        tparams: &[String],
        params: &[Ty],
        args: &[Expr],
        pre: &[(Ty, Ty)],
        ret: &Ty,
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        let refs: Vec<&Expr> = args.iter().collect();
        self.generic_call_refs(expr, what, tparams, params, &refs, pre, ret, expected)
    }

    /// [`Typer::generic_call`] over arguments in parameter order.
    #[allow(clippy::too_many_arguments)]
    fn generic_call_refs(
        &mut self,
        expr: &Expr,
        what: &str,
        tparams: &[String],
        params: &[Ty],
        args: &[&Expr],
        pre: &[(Ty, Ty)],
        ret: &Ty,
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        if params.len() != args.len() {
            self.error(
                "E043",
                expr.span,
                format!(
                    "wrong number of arguments to `{what}`: expected {}, found {}",
                    params.len(),
                    args.len()
                ),
            );
        }
        let pairs: Vec<(&Expr, Ty)> = args.iter().copied().zip(params.iter().cloned()).collect();
        for arg in args.iter().skip(params.len()) {
            self.synth(arg);
        }
        let instance = self.infer(expr.span, what, tparams, &pairs, pre, ret, expected)?;
        self.bounds_met(expr, what, tparams, &instance);
        let ty = ret.substitute(tparams, &instance);
        self.instances.insert(key(expr), instance);
        Some(ty)
    }

    /// Record the bounds of the fn or role `key` of this module
    /// ([`ModuleTypes::bounds`]).
    fn own_bounds(&mut self, key: String, decl: &FnDecl) {
        let bounds: BTreeMap<String, String> = decl
            .bounds
            .iter()
            .filter_map(|b| {
                let interface = b.interfaces.first()?;
                Some((b.param.name.clone(), interface.name.clone()))
            })
            .collect();
        if !bounds.is_empty() {
            self.own.bounds.entry(key).or_insert(bounds);
        }
    }

    /// The bounds of the fn or role `expr` calls or names, from its callee.
    fn callee_bounds(&self, expr: &Expr) -> Option<BTreeMap<String, String>> {
        let (module, name) = match self.callees.get(&key(expr))? {
            Callee::Fn(q) => (&q.module, q.name.clone()),
            Callee::Role { protocol, role } => {
                (&protocol.module, format!("{}.{role}", protocol.name))
            }
            _ => return None,
        };
        self.module_types(module)?.bounds.get(&name).cloned()
    }

    /// Whether a value of `ty` has the methods of `ByteStream`: a
    /// `TcpStream`, a `PipeStream`, a `TlsStream` or a type parameter of
    /// the fn being walked that is bounded by it (`docs/slice.md` 18.3).
    fn implements_stream(&self, ty: &Ty) -> bool {
        match ty {
            Ty::TcpStream | Ty::PipeStream | Ty::TlsStream => true,
            Ty::Param(name) => self.bounded.contains(name),
            _ => false,
        }
    }

    /// E096 for each bounded type parameter of the callee of `expr` (a
    /// call, role call, UFCS call, `spawn` or fn value of `what`) that
    /// `instance` binds to a type not implementing its interface.
    fn bounds_met(&mut self, expr: &Expr, what: &str, tparams: &[String], instance: &[Ty]) {
        let Some(bounds) = self.callee_bounds(expr) else {
            return;
        };
        for (param, ty) in tparams.iter().zip(instance) {
            if let Some(interface) = bounds.get(param)
                && !self.implements_stream(ty)
            {
                self.error(
                    "E096",
                    expr.span,
                    format!(
                        "`{ty}` does not implement `{interface}`, which `{param}` of `{what}` requires; pass a `TcpStream`, a `PipeStream`, a `TlsStream` or a type parameter bounded by `{interface}`"
                    ),
                );
            }
        }
    }

    /// E042 for an effectful operation (a `Store` method, a call of a value
    /// of effectful fn type) whose effects `ignite-check` could not check.
    /// Not inside a `system` or `assembly`, which `ignite-check` does not
    /// walk: there the type checker owns the rule (E116, `docs/slice.md`
    /// section 15.2).
    fn effect_site(&mut self, expr: &Expr, what: &str) {
        if self.unchecked_effects(expr) {
            self.error(
                "E042",
                expr.span,
                format!(
                    "cannot check the effects of `{what}`: the type of its receiver is not declared; annotate the binding"
                ),
            );
        }
    }

    /// E140 for a `Text` literal path argument of a `Directory` method that
    /// is absolute, has a `..` component or a NUL (`docs/slice.md` 21.4,
    /// 21.9). A path from anything but a literal is checked at run time only.
    fn path_literal(&mut self, method: &str, arg: Option<&Expr>) {
        let Some(arg) = arg else {
            return;
        };
        let ExprKind::Str(path) = &arg.kind else {
            return;
        };
        let reason = if path.starts_with('/') {
            "absolute"
        } else if path.split('/').any(|c| c == "..") {
            "leaves its root"
        } else if path.contains('\0') {
            "contains NUL"
        } else {
            return;
        };
        self.error(
            "E140",
            arg.span,
            format!("path `{path}` of `{method}`: {reason}"),
        );
    }

    /// Whether the effectful operation `expr` is one `ignite-check` did not
    /// record as an effect site, outside a `system` or `assembly`.
    fn unchecked_effects(&self, expr: &Expr) -> bool {
        self.current_system.is_none()
            && self.assembly_system.is_none()
            && !self.symbols.effect_sites.contains(&expr.span)
    }

    /// The effectful fn `name` (of type `ty`) used as a value: allowed only
    /// where an effectful fn type covering its effects is expected (E017).
    fn effectful_value(&mut self, name: &Ident, ty: Ty, expected: Option<&Ty>) -> Option<Ty> {
        let effects = self
            .symbols
            .effects
            .get(&name.name)
            .map(|sig| FnEffects {
                names: sig.params.clone(),
                requires: sig.requires.clone(),
            })
            .unwrap_or(FnEffects {
                names: Vec::new(),
                requires: Vec::new(),
            });
        let (Ty::Fn(params, ret), Some(Ty::EffectFn(_, _, wanted))) = (&ty, expected) else {
            self.error(
                "E017",
                name.span,
                format!(
                    "effectful function `{}` is not a value here; only an effectful fn type (`fn(..) -> R requires [..]`) accepts it",
                    name.name
                ),
            );
            return None;
        };
        let missing = effects.uncovered(wanted);
        if !missing.is_empty() {
            let missing = missing
                .iter()
                .map(|e| format!("`{e}`"))
                .collect::<Vec<_>>()
                .join(", ");
            self.error(
                "E017",
                name.span,
                format!(
                    "effectful function `{}` does not fit `{}`: its effect {missing} is not covered by the type's `requires`",
                    name.name,
                    expected.map(Ty::to_string).unwrap_or_default()
                ),
            );
            return None;
        }
        Some(Ty::EffectFn(params.clone(), ret.clone(), effects))
    }

    // ---- names and signatures ---------------------------------------------

    fn names(&mut self, program: &'a Program) {
        for decl in &program.decls {
            let name = match decl {
                Decl::Fn(f) => f.qualified_name(),
                _ => decl.name().name.clone(),
            };
            let qual = QualName::new(self.module.clone(), name.clone());
            let params = |ps: &[Ident]| ps.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
            let generic = match decl {
                Decl::Record(r) => params(&r.type_params),
                Decl::Enum(e) => params(&e.type_params),
                Decl::Fn(f) => params(&f.type_params),
                _ => Vec::new(),
            };
            if !generic.is_empty() {
                self.own.generics.entry(name.clone()).or_insert(generic);
            }
            if let Decl::Fn(f) = decl {
                self.own_bounds(name.clone(), f);
            }
            if let Decl::Protocol(protocol) = decl {
                for role in protocol.fns.iter().filter(|r| !r.type_params.is_empty()) {
                    let key = format!("{name}.{}", role.name.name);
                    self.own_bounds(key.clone(), role);
                    self.own
                        .generics
                        .entry(key)
                        .or_insert(params(&role.type_params));
                }
            }
            let table = match decl {
                Decl::Alias(alias) => {
                    self.alias_decls.entry(name.clone()).or_insert(alias);
                    &mut self.aliases
                }
                Decl::Record(_) => &mut self.records,
                Decl::Enum(_) => &mut self.enums,
                Decl::Fn(_) => &mut self.fns,
                Decl::Const(_) => &mut self.consts,
                Decl::Protocol(_) => &mut self.protocols,
                Decl::System(_) | Decl::Test(_) | Decl::Assembly(_) => continue,
            };
            table.entry(name).or_insert(qual);
        }
        for import in program.imports.iter().filter(|i| i.is_glob()) {
            let module = import.module();
            let Some(types) = self.modules.get(&module) else {
                continue;
            };
            for (name, from) in &self.symbols.imported {
                if *from != module {
                    continue;
                }
                let qual = QualName::new(module.clone(), name.clone());
                let n = name.clone();
                let prefix = format!("{n}.");
                for f in types.fns.keys().filter(|f| f.starts_with(&prefix)) {
                    self.fns
                        .entry(f.clone())
                        .or_insert_with(|| QualName::new(module.clone(), f.clone()));
                }
                if types.records.contains_key(&n) {
                    self.records.entry(n).or_insert(qual);
                } else if types.enums.contains_key(&n) {
                    self.enums.entry(n).or_insert(qual);
                } else if types.fns.contains_key(&n) {
                    self.fns.entry(n).or_insert(qual);
                } else if types.consts.contains_key(&n) {
                    self.consts.entry(n).or_insert(qual);
                } else if types.protocols.contains_key(&n) {
                    self.protocols.entry(n).or_insert(qual);
                } else if types.aliases.contains_key(&n) {
                    self.aliases.entry(n).or_insert(qual);
                }
            }
        }
        for import in program.imports.iter().filter(|i| !i.is_glob()) {
            let segments = &import.path.segments;
            let (module_segments, names): (&[Ident], Vec<&Ident>) = if import.names.is_empty() {
                (
                    &segments[..segments.len().saturating_sub(1)],
                    vec![import.path.last()],
                )
            } else {
                (&segments[..], import.names.iter().collect())
            };
            let module = module_segments
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(".");
            let substrate = SUBSTRATE_MODULES.contains(&module.as_str());
            let Some(types) = self.modules.get(&module) else {
                if !substrate {
                    self.error(
                        "E031",
                        import.path.span,
                        format!("module `{module}` has not been typed"),
                    );
                }
                continue;
            };
            for name in names {
                if substrate && !types.exports_name(&name.name) {
                    continue;
                }
                let qual = QualName::new(module.clone(), name.name.clone());
                let n = name.name.clone();
                let prefix = format!("{n}.");
                for f in types.fns.keys().filter(|f| f.starts_with(&prefix)) {
                    self.fns
                        .insert(f.clone(), QualName::new(module.clone(), f.clone()));
                }
                if types.records.contains_key(&n) {
                    self.records.insert(n, qual);
                } else if types.enums.contains_key(&n) {
                    self.enums.insert(n, qual);
                } else if types.fns.contains_key(&n) {
                    self.fns.insert(n, qual);
                } else if types.consts.contains_key(&n) {
                    self.consts.insert(n, qual);
                } else if types.protocols.contains_key(&n) {
                    self.protocols.insert(n, qual);
                } else if types.aliases.contains_key(&n) {
                    self.aliases.insert(n, qual);
                } else {
                    self.error(
                        "E031",
                        name.span,
                        format!("module `{module}` exports no typed `{n}`"),
                    );
                }
            }
        }
    }

    fn signatures(&mut self, program: &Program) {
        for decl in &program.decls {
            let name = decl.name().name.clone();
            match decl {
                Decl::Alias(_) => {
                    let q = QualName::new(self.module.clone(), name);
                    self.alias(&q, decl.name().span);
                }
                Decl::Record(record) => {
                    self.type_params = idents(&record.type_params);
                    let fields = record
                        .fields
                        .iter()
                        .map(|f| (f.name.name.clone(), self.resolve(&f.ty)))
                        .collect();
                    self.type_params.clear();
                    let defaults: Vec<String> = record
                        .fields
                        .iter()
                        .filter(|f| f.default.is_some())
                        .map(|f| f.name.name.clone())
                        .collect();
                    if !defaults.is_empty() {
                        self.own.defaults.entry(name.clone()).or_insert(defaults);
                    }
                    self.own.records.entry(name).or_insert(fields);
                }
                Decl::Enum(decl) => {
                    self.type_params = idents(&decl.type_params);
                    let variants = decl
                        .variants
                        .iter()
                        .map(|v| {
                            let payload = v.payload.iter().map(|t| self.resolve(t)).collect();
                            (v.name.name.clone(), payload)
                        })
                        .collect();
                    self.type_params.clear();
                    self.own.enums.entry(name).or_insert(variants);
                }
                Decl::Fn(decl) => {
                    let sig = self.fn_sig(decl, false);
                    self.own.fns.entry(decl.qualified_name()).or_insert(sig);
                    self.own
                        .param_names
                        .entry(decl.qualified_name())
                        .or_insert_with(|| {
                            decl.params.iter().map(|p| p.name.name.clone()).collect()
                        });
                }
                Decl::Const(decl) => {
                    let ty = self.resolve(&decl.ty);
                    self.own.consts.entry(name).or_insert(ty);
                }
                Decl::Protocol(protocol) => {
                    let mut roles = BTreeMap::new();
                    for role in &protocol.fns {
                        let sig = self.fn_sig(role, true);
                        roles.entry(role.name.name.clone()).or_insert(sig);
                    }
                    self.own.protocols.entry(name).or_insert(roles);
                }
                Decl::System(_) | Decl::Test(_) | Decl::Assembly(_) => {}
            }
        }
    }

    /// A function's or role's signature. A role is effectful; a function is
    /// effectful when `ignite-check` found it so, declared or inferred.
    fn fn_sig(&mut self, decl: &FnDecl, role: bool) -> FnSig {
        let effectful = role
            || self
                .symbols
                .functions
                .get(&decl.qualified_name())
                .map_or(decl.is_effectful(), |f| f.effectful);
        let outer = std::mem::replace(&mut self.type_params, idents(&decl.type_params));
        let sig = FnSig {
            params: decl.params.iter().map(|p| self.resolve(&p.ty)).collect(),
            ret: decl.ret.as_ref().map_or(Ty::Unit, |t| self.resolve(t)),
            effectful,
        };
        self.type_params = outer;
        sig
    }

    /// The alias `q`, resolving one of this module's on first use. A cycle
    /// through aliases is E048 at `span`.
    fn alias(&mut self, q: &QualName, span: Span) -> Option<Alias> {
        if q.module != self.module {
            return self.modules.get(&q.module)?.aliases.get(&q.name).cloned();
        }
        if let Some(alias) = self.own.aliases.get(&q.name) {
            return Some(alias.clone());
        }
        let decl = *self.alias_decls.get(&q.name)?;
        if self.resolving.contains(&q.name) {
            self.error(
                "E048",
                span,
                format!("type alias `{}` refers to itself", q.name),
            );
            return None;
        }
        self.resolving.push(q.name.clone());
        let params: Vec<String> = decl.params.iter().map(|p| p.name.clone()).collect();
        let outer = std::mem::replace(&mut self.alias_params, params.clone());
        let outer_params = std::mem::take(&mut self.type_params);
        let target = self.resolve(&decl.ty);
        self.type_params = outer_params;
        self.alias_params = outer;
        self.resolving.pop();
        let alias = Alias { params, target };
        self.own
            .aliases
            .entry(q.name.clone())
            .or_insert_with(|| alias.clone());
        Some(alias)
    }

    /// The type a type expression denotes, recorded for
    /// [`crate::TypedProgram::resolved_type`]. After an error the result is
    /// `Unit`; the diagnostic makes the module fail.
    fn resolve(&mut self, ty: &Type) -> Ty {
        let resolved = self.resolve_kind(ty);
        self.type_exprs.insert(ty.id, resolved.clone());
        resolved
    }

    fn resolve_kind(&mut self, ty: &Type) -> Ty {
        if ty.is_fn() {
            let params = ty.fn_params().iter().map(|t| self.resolve(t)).collect();
            let ret = ty.fn_ret().map_or(Ty::Unit, |t| self.resolve(t));
            if let Some(requires) = &ty.requires {
                let effects = FnEffects {
                    names: idents(&ty.fn_names),
                    requires: requires.iter().map(|p| p.dotted()).collect(),
                };
                return Ty::EffectFn(params, Box::new(ret), effects);
            }
            return fn_ty(params, ret);
        }
        if ty.is_tuple() {
            return Ty::Tuple(ty.args.iter().map(|t| self.resolve(t)).collect());
        }
        let name = ty.name.name.as_str();
        if self.type_params.iter().any(|p| p == name)
            && !self.alias_params.iter().any(|p| p == name)
        {
            if !ty.args.is_empty() {
                self.error(
                    "E048",
                    ty.span,
                    format!(
                        "type parameter `{name}` takes no type arguments, found {}",
                        ty.args.len()
                    ),
                );
                return Ty::Unit;
            }
            return Ty::Param(name.to_string());
        }
        let alias = if self.alias_params.iter().any(|p| p == name) {
            None
        } else {
            self.aliases.get(name).cloned()
        };
        let alias = match alias {
            Some(q) => match self.alias(&q, ty.name.span) {
                Some(alias) => Some(alias),
                None => return Ty::Unit,
            },
            None => None,
        };
        let declared = if self.alias_params.iter().any(|p| p == name) {
            None
        } else {
            self.records
                .get(name)
                .or_else(|| self.enums.get(name))
                .cloned()
        };
        let arity = match (&alias, name) {
            (Some(alias), _) => alias.params.len(),
            (None, "List" | "Option" | "Store" | "Signal" | "Module" | "Sender" | "Receiver") => 1,
            (None, "Result" | "Map") => 2,
            (None, "Ref") => usize::from(!ty.args.is_empty()),
            _ => declared.as_ref().map_or(0, |q| self.generics_of(q).len()),
        };
        if ty.args.len() != arity {
            self.error(
                "E048",
                ty.span,
                format!(
                    "`{name}` takes {arity} type argument{}, found {}",
                    if arity == 1 { "" } else { "s" },
                    ty.args.len()
                ),
            );
            return Ty::Unit;
        }
        if let Some(alias) = alias {
            let args: Vec<Ty> = ty.args.iter().map(|t| self.resolve(t)).collect();
            return substitute(&alias.target, &alias.params, &args);
        }
        if self.alias_params.iter().any(|p| p == name) {
            return Ty::Record(QualName::alias_parameter(name));
        }
        let mut arg = |i: usize| Box::new(self.resolve(&ty.args[i]));
        match name {
            "Int" => Ty::Int,
            "Bool" => Ty::Bool,
            "Byte" => Ty::Byte,
            "U32" => Ty::U32,
            "Unit" => Ty::Unit,
            "Text" => Ty::Text,
            "Bytes" => bytes(),
            "TcpListener" => Ty::TcpListener,
            "Directory" => Ty::Capability(Capability::Directory),
            "Launcher" => Ty::Capability(Capability::Launcher),
            "ChildProcess" => Ty::Capability(Capability::ChildProcess),
            "Entropy" => Ty::Capability(Capability::Entropy),
            "Setting" => Ty::Capability(Capability::Setting),
            "CommandLine" => Ty::Capability(Capability::CommandLine),
            "Environment" => Ty::Capability(Capability::Environment),
            "TcpStream" => Ty::TcpStream,
            "PipeStream" => Ty::PipeStream,
            "TlsStream" => Ty::TlsStream,
            "IoError" => Ty::IoError,
            "Input" => Ty::Input,
            "List" => Ty::List(arg(0)),
            "Option" => Ty::Option(arg(0)),
            "Store" => Ty::Store(arg(0)),
            "Sender" => Ty::Sender(arg(0)),
            "Receiver" => Ty::Receiver(arg(0)),
            "Result" => {
                let ok = arg(0);
                Ty::Result(ok, arg(1))
            }
            "Map" => {
                let k = arg(0);
                let v = arg(1);
                self.map_key(&k, ty.args[0].span);
                Ty::Map(k, v)
            }
            "Duration" => Ty::Quantity(QuantityKind::Duration),
            "ByteSize" => Ty::Quantity(QuantityKind::ByteSize),
            "Ratio" => Ty::Quantity(QuantityKind::Ratio),
            "Ref" if ty.args.is_empty() => Ty::Ref(None),
            "Ref" => Ty::Ref(Some(arg(0))),
            "Signal" => Ty::Signal(arg(0)),
            "SystemRef" => Ty::SystemRef,
            "Protocol" => Ty::Protocol,
            "Exposure" => exposure(),
            "Module" => {
                let record = *arg(0);
                if !matches!(record, Ty::Record(_) | Ty::GenericRecord(..)) {
                    self.error(
                        "E048",
                        ty.span,
                        format!("`Module<I>` takes a record type `I` whose fields are fn types, found `{record}`"),
                    );
                }
                record
            }
            "Stdin" | "Stdout" | "Stderr" => {
                self.error(
                    "E040",
                    ty.span,
                    format!("`{name}` has no values; it only owns static operations"),
                );
                Ty::Unit
            }
            _ => {
                let args: Vec<Ty> = (0..ty.args.len()).map(|i| *arg(i)).collect();
                if let Some(q) = self.records.get(name) {
                    if args.is_empty() {
                        Ty::Record(q.clone())
                    } else {
                        Ty::GenericRecord(q.clone(), args)
                    }
                } else if let Some(q) = self.enums.get(name) {
                    if args.is_empty() {
                        Ty::Enum(q.clone())
                    } else {
                        Ty::GenericEnum(q.clone(), args)
                    }
                } else {
                    self.error("E048", ty.name.span, format!("unknown type `{name}`"));
                    Ty::Unit
                }
            }
        }
    }

    // ---- lookups ----------------------------------------------------------

    fn module_types(&self, module: &str) -> Option<&ModuleTypes> {
        if module == self.module {
            Some(&self.own)
        } else {
            self.modules.get(module)
        }
    }

    /// The type parameters of the record, enum or fn `q`; empty when it
    /// declares none.
    fn generics_of(&self, q: &QualName) -> Vec<String> {
        self.module_types(&q.module)
            .and_then(|m| m.generics.get(&q.name).cloned())
            .unwrap_or_default()
    }

    /// The type parameters of role `role` of protocol `q`.
    fn role_generics(&self, q: &QualName, role: &str) -> Vec<String> {
        self.module_types(&q.module)
            .and_then(|m| m.generics.get(&format!("{}.{role}", q.name)).cloned())
            .unwrap_or_default()
    }

    fn sig_of(&self, q: &QualName) -> Option<FnSig> {
        self.module_types(&q.module)?.fns.get(&q.name).cloned()
    }

    fn const_ty(&self, q: &QualName) -> Option<Ty> {
        self.module_types(&q.module)?.consts.get(&q.name).cloned()
    }

    fn record_fields(&self, q: &QualName) -> Option<Vec<(String, Ty)>> {
        self.module_types(&q.module)?.records.get(&q.name).cloned()
    }

    fn role(&self, q: &QualName, role: &str) -> Option<FnSig> {
        self.module_types(&q.module)?
            .protocols
            .get(&q.name)?
            .get(role)
            .cloned()
    }

    /// The enum name and instantiated variants of a type with variants.
    fn variants(&self, ty: &Ty) -> Option<(QualName, Variants)> {
        match ty {
            Ty::Enum(q) if q.module.is_empty() => variants_of(ty, self.modules),
            Ty::Enum(q) => {
                let variants = self.module_types(&q.module)?.enums.get(&q.name)?;
                Some((q.clone(), variants.clone()))
            }
            Ty::GenericEnum(q, args) => Some((
                q.clone(),
                instantiate(self.module_types(&q.module)?, q, args)?,
            )),
            _ => variants_of(ty, self.modules),
        }
    }

    /// Look up one constructor without copying every sibling's payload.
    fn selected_variant(&self, ty: &Ty, name: &str) -> Option<(QualName, usize, Vec<Ty>)> {
        let (q, args) = match ty {
            Ty::Enum(q) if !q.module.is_empty() => (q, None),
            Ty::GenericEnum(q, args) => (q, Some(args)),
            _ => {
                let (q, variants) = self.variants(ty)?;
                let (index, (_, payload)) = variants
                    .into_iter()
                    .enumerate()
                    .find(|(_, (v, _))| v == name)?;
                return Some((q, index, payload));
            }
        };
        let module = self.module_types(&q.module)?;
        let (index, (_, payload)) = module
            .enums
            .get(&q.name)?
            .iter()
            .enumerate()
            .find(|(_, (v, _))| v == name)?;
        let payload = if let Some(args) = args {
            let params = module
                .generics
                .get(&q.name)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            payload
                .iter()
                .map(|ty| ty.substitute(params, args))
                .collect()
        } else {
            payload.clone()
        };
        Some((q.clone(), index, payload))
    }

    /// Payload types of `variant` of the user enum named `owner` in scope.
    fn user_payload(&self, owner: &str, variant: &str) -> Option<Vec<Ty>> {
        let q = self.enums.get(owner)?;
        self.selected_variant(&Ty::Enum(q.clone()), variant)
            .map(|(_, _, payload)| payload)
    }

    fn lookup(&self, name: &str) -> Option<Option<Ty>> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn bind(&mut self, name: &Ident, ty: Option<Ty>) {
        if let Some(ty) = &ty {
            self.bindings.insert(name.span, ty.clone());
        }
        let depth = self.scopes.len().saturating_sub(1);
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.name.clone(), ty);
            self.binding_spans
                .insert((depth, name.name.clone()), name.span);
        }
    }

    /// E006 when `builtin`, called at `expr`, changes the value of a place
    /// rooted at a binding the innermost closure being walked captures: a
    /// closure captures by copy (`docs/slice.md` 13.11, 20.3.3). The
    /// checker reports the same where it knows the receiver's type; here
    /// the callee is resolved, so a capture of any type is covered, and a
    /// UFCS call of a user fn named like a builtin never is.
    fn captured_change(&mut self, expr: &Expr, receiver: &Expr, builtin: Builtin) {
        if crate::ownership::change(builtin) != Some(crate::ownership::Change::Value) {
            return;
        }
        let Some(&(outside, _)) = self.closure_frames.last() else {
            return;
        };
        let Some(base) = crate::ownership::place_root(receiver) else {
            return;
        };
        let captured = self
            .scopes
            .iter()
            .rposition(|scope| scope.contains_key(base))
            .is_some_and(|index| index < outside);
        if captured {
            self.error(
                "E006",
                expr.span,
                format!(
                    "cannot change `{base}` inside a closure; a closure captures `{base}` by copy"
                ),
            );
        }
    }

    /// [`Typer::lookup`] for a use of a local as a value: a binding of a
    /// scope outside a closure being walked is recorded as its capture.
    fn use_local(&mut self, name: &str) -> Option<Option<Ty>> {
        let index = self
            .scopes
            .iter()
            .rposition(|scope| scope.contains_key(name))?;
        let ty = self.scopes[index].get(name).cloned()?;
        let binding = self
            .binding_spans
            .get(&(index, name.to_string()))
            .copied()
            .unwrap_or_default();
        for (outside, captures) in &mut self.closure_frames {
            if index < *outside && !captures.iter().any(|c| c.name == name) {
                captures.push(Capture {
                    name: name.to_string(),
                    binding,
                    ty: ty.clone().unwrap_or(Ty::Unit),
                    effects: Vec::new(),
                });
            }
        }
        Some(ty)
    }

    /// The type, enum or protocol heading a path such as `Action.Continue`,
    /// `Bytes.empty` or `Echo.serve`, by the rule of `ignite-check`.
    fn path_head<'e>(&self, receiver: &'e Expr) -> Option<&'e Ident> {
        match &receiver.kind {
            ExprKind::Name(id)
                if self.lookup(&id.name).is_none()
                    && !self.symbols.constants.contains(&id.name)
                    && (id.is_uppercase() || self.protocols.contains_key(&id.name)) =>
            {
                Some(id)
            }
            _ => None,
        }
    }

    fn is_enum(&self, name: &str) -> bool {
        self.enums.contains_key(name) || BUILTIN_ENUMS.contains(&name)
    }

    // ---- declarations -----------------------------------------------------

    fn bodies(&mut self, program: &Program) {
        for _ in 0..2 {
            for decl in &program.decls {
                if let Decl::Const(c) = decl
                    && matches!(
                        self.own.consts.get(&c.name.name),
                        Some(Ty::Int | Ty::Byte | Ty::U32)
                    )
                {
                    self.const_eval(&c.name.name, &c.value);
                }
            }
        }
        for decl in &program.decls {
            if let Decl::System(system) = decl {
                self.collect_members(system);
            }
        }
        for (index, decl) in program.decls.iter().enumerate() {
            if let Decl::System(system) = decl {
                self.system(index, system);
            }
        }
        for (index, decl) in program.decls.iter().enumerate() {
            self.in_test = decl.is_test_only();
            match decl {
                Decl::Test(test) => self.with_local_inference(&|t| t.test(index, test)),
                Decl::Fn(f) => {
                    if let Some(sig) = self.own.fns.get(&f.qualified_name()).cloned() {
                        self.with_local_inference(&|t| t.function(f, &sig));
                        self.hook_signature(f);
                    }
                }
                Decl::Protocol(protocol) => {
                    for f in &protocol.fns {
                        let sig = self
                            .own
                            .protocols
                            .get(&protocol.name.name)
                            .and_then(|roles| roles.get(&f.name.name))
                            .cloned();
                        if let Some(sig) = sig {
                            self.function(f, &sig);
                        }
                    }
                }
                Decl::Const(c) => {
                    if let Some(ty) = self.own.consts.get(&c.name.name).cloned() {
                        self.scopes = vec![BTreeMap::new()];
                        self.ret = None;
                        self.check(&c.value, &ty);
                        self.scopes.clear();
                        self.record_effects(&c.value);
                    }
                }

                Decl::Record(record) => {
                    let fields = self.own.records.get(&record.name.name).cloned();
                    self.type_params = idents(&record.type_params);
                    for (field, (_, ty)) in record.fields.iter().zip(fields.iter().flatten()) {
                        if let Some(default) = &field.default {
                            self.scopes = vec![BTreeMap::new()];
                            self.ret = None;
                            self.check(default, ty);
                            self.scopes.clear();
                            self.record_effects(default);
                        }
                    }
                    self.type_params.clear();
                }
                Decl::Assembly(assembly) => self.assembly(index, assembly),
                Decl::System(_) | Decl::Alias(_) | Decl::Enum(_) => {}
            }
        }
        self.in_test = false;
    }

    /// A test body: a block of type `Unit` with no parameters, where `?`
    /// takes any `Result` or `Option` and fails the test.
    fn test(&mut self, index: usize, test: &TestDecl) {
        self.tests.push(TestInfo {
            module: self.module.clone(),
            name: test.name.name.clone(),
            full_name: format!("{}/{}", self.module, test.name.name),
            decl: index,
        });
        self.ret = Some(Ty::Unit);
        self.scopes = vec![BTreeMap::new()];
        self.loops.clear();
        self.in_test_body = true;
        self.effect_scope = EffectScope::Open;
        self.block(&test.body, Mode::Stmt);
        self.effect_scope = EffectScope::Pure;
        self.in_test_body = false;
        self.scopes.clear();
        self.ret = None;
    }

    /// `assert cond`: `cond` must be `Bool` (E052). `assert a == b` and
    /// `assert a != b` compare any two values of one type with structural
    /// equality (E045 for a type without it) and record how to show them.
    /// `at` is the offset of the `assert` keyword, the failure site's
    /// position (`docs/slice.md` section 14.8).
    fn assert(&mut self, cond: &Expr, at: usize) {
        self.system.failures.insert(
            key(cond),
            FailureSite {
                op: FailureOp::Assert,
                offset: at,
            },
        );
        if let ExprKind::Binary {
            op: BinaryOp::Eq | BinaryOp::Ne,
            left,
            right,
        } = &cond.kind
        {
            let right_first = {
                let ambiguous = |n: &str| self.is_ambiguous_variant(n);
                (is_literal(left) || needs_expected(left, &ambiguous))
                    && !(is_literal(right) || needs_expected(right, &ambiguous))
            };
            let (first, second) = if right_first {
                (right, left)
            } else {
                (left, right)
            };
            let first_ty = self.synth(first);
            let second_ty = match &first_ty {
                Some(Ty::Never) | None => self.synth(second),
                Some(t) => self.check(second, t),
            };
            let operand = match (first_ty, second_ty) {
                (Some(Ty::Never), Some(t)) | (Some(t), _) => t,
                (None, _) => {
                    self.record(cond, Some(Ty::Bool));
                    return;
                }
            };
            self.record(cond, Some(Ty::Bool));
            if !self.assert_equatable(&operand) {
                let op = if matches!(
                    cond.kind,
                    ExprKind::Binary {
                        op: BinaryOp::Eq,
                        ..
                    }
                ) {
                    "=="
                } else {
                    "!="
                };
                self.error(
                    "E045",
                    cond.span,
                    format!(
                        "operator `{op}` is not defined for `{operand}`, not even in an `assert`: it holds a function, handle, store or type parameter"
                    ),
                );
                return;
            }
            let show = self.show(&operand);
            self.assert_compares
                .insert(key(cond), AssertCompare { operand, show });
            return;
        }
        let found = self.expr(cond, Mode::Value(Some(&Ty::Bool)));
        if let Some(found) = found
            && !compatible(&found, &Ty::Bool)
        {
            self.error(
                "E052",
                cond.span,
                format!("`assert` needs a `Bool` condition, found `{found}`"),
            );
        }
    }

    /// A complete equality query starts with no coinductive assumptions.
    /// Reuse only its final result: caching a nested successful visit could
    /// hide a function field encountered later in a recursive parent.
    fn assert_equatable(&mut self, ty: &Ty) -> bool {
        if let Some(result) = self.equatable_cache.get(ty) {
            return *result;
        }
        let result = self.equatable(ty, &mut Vec::new(), true);
        self.equatable_cache.insert(ty.clone(), result);
        result
    }

    /// Whether values of `ty` can be compared with structural equality: no
    /// fn, handle, store or type parameter anywhere inside, and no `IoError`
    /// unless `in_assert` (`docs/slice.md` 12.3, 13.5). `seen` guards
    /// against records and enums that hold themselves through `List`.
    fn equatable(&self, ty: &Ty, seen: &mut Vec<Ty>, in_assert: bool) -> bool {
        match ty {
            Ty::Int
            | Ty::Bool
            | Ty::Byte
            | Ty::U32
            | Ty::Unit
            | Ty::Text
            | Ty::Input
            | Ty::Never => true,
            Ty::List(t) | Ty::Option(t) => self.equatable(t, seen, in_assert),
            Ty::Result(ok, err) => {
                self.equatable(ok, seen, in_assert) && self.equatable(err, seen, in_assert)
            }
            Ty::Tuple(items) => items.iter().all(|t| self.equatable(t, seen, in_assert)),
            Ty::Record(q) | Ty::GenericRecord(q, _) => {
                if seen.contains(ty) {
                    return true;
                }
                seen.push(ty.clone());
                let args = match ty {
                    Ty::GenericRecord(_, args) => args.clone(),
                    _ => Vec::new(),
                };
                let params = self.generics_of(q);
                let fields = self.record_fields(q).unwrap_or_default();
                fields
                    .iter()
                    .all(|(_, t)| self.equatable(&t.substitute(&params, &args), seen, in_assert))
            }
            Ty::Enum(_) | Ty::GenericEnum(..) => {
                if seen.contains(ty) {
                    return true;
                }
                seen.push(ty.clone());
                let variants = self.variants(ty).map(|(_, v)| v).unwrap_or_default();
                variants
                    .iter()
                    .all(|(_, payload)| payload.iter().all(|t| self.equatable(t, seen, in_assert)))
            }
            Ty::Quantity(_) | Ty::Ref(_) | Ty::Signal(_) | Ty::SystemRef | Ty::Protocol => true,
            // By its message, only in an assert (12.3); E045 for `==` elsewhere.
            Ty::IoError => in_assert,
            Ty::Map(k, v) => {
                self.equatable(k, seen, in_assert) && self.equatable(v, seen, in_assert)
            }
            Ty::Fn(..)
            | Ty::EffectFn(..)
            | Ty::TcpListener
            | Ty::Capability(_)
            | Ty::TcpStream
            | Ty::PipeStream
            | Ty::TlsStream
            | Ty::Store(_)
            | Ty::Sender(_)
            | Ty::Receiver(_)
            | Ty::Param(_) => false,
        }
    }

    /// How a failed `assert` or `?` shows a value of `ty`: the builtin
    /// `to_text`, an `IoError`'s message, or a pure fn `to_text(x: T) ->
    /// Text` in scope (a test-only one only inside tests).
    fn show(&self, ty: &Ty) -> Option<Show> {
        let builtin = match ty {
            Ty::Int => Some(Builtin::IntToTextMethod),
            Ty::Byte => Some(Builtin::ByteToText),
            Ty::U32 => Some(Builtin::U32ToText),
            Ty::Bool => Some(Builtin::BoolToText),
            Ty::Text | Ty::Ref(_) | Ty::Signal(_) => Some(Builtin::TextToText),
            Ty::List(element) if **element == Ty::Byte => Some(Builtin::BytesToText),
            Ty::IoError => return Some(Show::IoErrorMessage),
            _ => None,
        };
        if let Some(builtin) = builtin {
            return Some(Show::Builtin(builtin));
        }
        let debug = (ty != &Ty::Never && !ty.has_param(&|_| true)).then_some(Show::Debug);
        let Some(q) = self.fns.get("to_text") else {
            return debug;
        };
        if q.module == self.module && self.symbols.test_only.contains("to_text") && !self.in_test {
            return debug;
        }
        if !self.generics_of(q).is_empty() {
            return debug;
        }
        let Some(sig) = self.sig_of(q) else {
            return debug;
        };
        let fits = sig.params.len() == 1
            && compatible(ty, &sig.params[0])
            && sig.ret == Ty::Text
            && !sig.effectful;
        if fits {
            Some(Show::Fn(q.clone()))
        } else {
            debug
        }
    }

    fn function(&mut self, f: &FnDecl, sig: &FnSig) {
        self.ret = Some(sig.ret.clone());
        self.scopes = vec![BTreeMap::new()];
        self.loops.clear();
        self.type_params = idents(&f.type_params);
        self.bounded = f.bounds.iter().map(|b| b.param.name.clone()).collect();
        for (param, ty) in f.params.iter().zip(&sig.params) {
            self.bind(&param.name, Some(ty.clone()));
        }
        self.effect_scope = match &f.requires {
            Some(requires) => EffectScope::Declared {
                name: f.name.name.clone(),
                requires: requires.iter().map(|path| path.dotted()).collect(),
            },
            None => EffectScope::Open,
        };
        if sig.ret == Ty::Unit {
            self.block(&f.body, Mode::Stmt);
        } else {
            self.block(&f.body, Mode::Value(Some(&sig.ret)));
        }
        self.effect_scope = EffectScope::Pure;
        self.scopes.clear();
        self.ret = None;
        self.type_params.clear();
        self.bounded.clear();
    }

    // ---- statements -------------------------------------------------------

    /// The type of a block: its last expression's in value position, `Never`
    /// when a statement in it diverges, else `Unit`. Checked against the
    /// expected type of `mode`, which it then returns.
    fn block(&mut self, block: &Block, mode: Mode) -> Option<Ty> {
        self.scopes.push(BTreeMap::new());
        let mut diverges = false;
        let mut value: Option<Option<Ty>> = None;
        let count = block.stmts.len();
        for (i, stmt) in block.stmts.iter().enumerate() {
            if i + 1 == count
                && let (StmtKind::Expr(expr), Mode::Value(expected)) = (&stmt.kind, mode)
            {
                let ty = match expected {
                    Some(t) => self.check(expr, t),
                    None => self.synth(expr),
                };
                diverges |= ty == Some(Ty::Never);
                value = Some(ty);
            } else if self.stmt(stmt) == Some(Ty::Never) {
                diverges = true;
            }
        }
        self.scopes.pop();
        if diverges {
            return Some(Ty::Never);
        }
        match (mode, value) {
            (Mode::Stmt, _) => Some(Ty::Unit),
            (Mode::Value(Some(expected)), Some(ty)) => ty.map(|_| expected.clone()),
            (Mode::Value(None), Some(ty)) => ty,
            (Mode::Value(Some(expected)), None) if *expected != Ty::Unit => {
                self.error(
                    "E040",
                    block.span,
                    format!("type mismatch: expected `{expected}`, the block has no value"),
                );
                None
            }
            (Mode::Value(_), None) => Some(Ty::Unit),
        }
    }

    /// A statement's type: `Never` when it diverges, else `Unit` (an
    /// expression statement: the expression's).
    fn stmt(&mut self, stmt: &Stmt) -> Option<Ty> {
        match &stmt.kind {
            StmtKind::LetPattern { pattern, value, .. } => {
                let ty = match self.let_hints.get(&pattern.span).cloned() {
                    Some(hint) => {
                        self.check(value, &hint);
                        Some(hint)
                    }
                    None => {
                        let ty = self.synth(value);
                        if let (Some(holes), Some(ty)) = (&mut self.holes, &ty)
                            && ty.has_param(&is_hole)
                        {
                            holes.lets.push((pattern.span, ty.clone()));
                        }
                        ty
                    }
                };
                self.pattern(pattern, ty.as_ref());
                Some(if ty == Some(Ty::Never) {
                    Ty::Never
                } else {
                    Ty::Unit
                })
            }
            StmtKind::Let {
                name, ty, value, ..
            } => {
                let ty = match ty {
                    Some(annotation) => {
                        let ty = self.resolve(annotation);
                        self.check(value, &ty);
                        Some(ty)
                    }
                    None => match self.let_hints.get(&name.span).cloned() {
                        Some(hint) => {
                            self.check(value, &hint);
                            Some(hint)
                        }
                        None => {
                            let ty = self.synth(value);
                            if let (Some(holes), Some(ty)) = (&mut self.holes, &ty)
                                && ty.has_param(&is_hole)
                            {
                                holes.lets.push((name.span, ty.clone()));
                            }
                            ty
                        }
                    },
                };
                self.bind(name, ty);
                Some(Ty::Unit)
            }
            StmtKind::Assign { target, value } => {
                match self.place(target) {
                    Some(ty) => {
                        self.check(value, &ty);
                    }
                    None => {
                        self.synth(value);
                    }
                }
                Some(Ty::Unit)
            }
            StmtKind::Return(value) => self.return_(stmt.span, value.as_ref()),
            StmtKind::While { cond, body } => {
                self.check(cond, &Ty::Bool);
                self.loops.push(false);
                self.block(body, Mode::Stmt);
                self.loops.pop();
                Some(Ty::Unit)
            }
            StmtKind::Loop { body } => {
                self.loops.push(false);
                self.block(body, Mode::Stmt);
                let broke = self.loops.pop().unwrap_or(false);
                Some(if broke { Ty::Unit } else { Ty::Never })
            }
            StmtKind::For { name, iter, body } => {
                let element = self.for_element(iter);
                self.loops.push(false);
                self.scopes.push(BTreeMap::new());
                self.bind(name, element);
                self.block(body, Mode::Stmt);
                self.scopes.pop();
                self.loops.pop();
                Some(Ty::Unit)
            }
            StmtKind::ForPattern {
                pattern,
                iter,
                body,
            } => {
                let element = self.for_element(iter);
                self.loops.push(false);
                self.scopes.push(BTreeMap::new());
                self.pattern(pattern, element.as_ref());
                self.block(body, Mode::Stmt);
                self.scopes.pop();
                self.loops.pop();
                Some(Ty::Unit)
            }
            StmtKind::Break => {
                if let Some(top) = self.loops.last_mut() {
                    *top = true;
                }
                Some(Ty::Never)
            }
            StmtKind::Continue => Some(Ty::Never),
            StmtKind::Expr(expr) => self.expr(expr, Mode::Stmt),
            StmtKind::Assert { cond, message } => {
                self.assert(cond, stmt.span.start);
                if let Some(message) = message {
                    self.check(message, &Ty::Text);
                }
                Some(Ty::Unit)
            }
        }
    }

    /// The element type of a `for` iterable: `T` of a `List<T>`, `(K, V)`
    /// of a `Map<K, V>` (`docs/slice.md` section 14.3).
    fn for_element(&mut self, iter: &Expr) -> Option<Ty> {
        match self.synth(iter) {
            Some(Ty::List(element)) => Some(*element),
            Some(Ty::Map(k, v)) => Some(Ty::Tuple(vec![*k, *v])),
            Some(other) => {
                self.error(
                    "E040",
                    iter.span,
                    format!("type mismatch: `for` iterates over a `List`, found `{other}`"),
                );
                None
            }
            None => None,
        }
    }

    /// `return` and `return e`, statement or expression: `Never`.
    fn return_(&mut self, span: Span, value: Option<&Expr>) -> Option<Ty> {
        let ret = self.ret.clone().unwrap_or(Ty::Unit);
        match value {
            Some(value) => {
                self.check(value, &ret);
            }
            None if ret != Ty::Unit => self.error(
                "E040",
                span,
                format!("type mismatch: `return` without a value in a function returning `{ret}`"),
            ),
            None => {}
        }
        Some(Ty::Never)
    }

    /// The type of an assignment target `x.a.b`.
    fn place(&mut self, place: &Place) -> Option<Ty> {
        let mut ty = self.lookup(&place.base.name)??;
        for (i, field) in place.fields.iter().enumerate() {
            if i + 1 == place.fields.len() {
                self.places.insert(place.span, ty.clone());
            }
            ty = self.field_of(&ty, field)?;
        }
        Some(ty)
    }

    // ---- expressions ------------------------------------------------------

    /// Check `expr` against `expected`: E040 unless its type is `expected`
    /// or `Never`. Returns the type found.
    fn check(&mut self, expr: &Expr, expected: &Ty) -> Option<Ty> {
        let found = self.expr(expr, Mode::Value(Some(expected)));
        if let (Some(holes), Some(found)) = (&mut self.holes, &found)
            && holes.next > 0
        {
            let mut binds = BTreeMap::new();
            unify(expected, found, &mut binds);
            unify(found, expected, &mut binds);
            for (name, ty) in binds {
                if is_hole(&name) {
                    holes.solved.entry(name).or_insert(ty);
                }
            }
        }
        if let Some(found) = &found
            && !compatible(found, expected)
        {
            self.mismatch(expr.span, expected, found);
        }
        found
    }

    fn synth(&mut self, expr: &Expr) -> Option<Ty> {
        self.expr(expr, Mode::Value(None))
    }

    fn record(&mut self, expr: &Expr, ty: Option<Ty>) -> Option<Ty> {
        if let Some(ty) = &ty {
            self.types.insert(key(expr), ty.clone());
        }
        ty
    }

    fn expr(&mut self, expr: &Expr, mode: Mode) -> Option<Ty> {
        let ty = self.expr_kind(expr, mode);
        self.record(expr, ty)
    }

    fn expr_kind(&mut self, expr: &Expr, mode: Mode) -> Option<Ty> {
        let expected = match mode {
            Mode::Value(expected) => expected,
            Mode::Stmt => None,
        };
        match &expr.kind {
            ExprKind::Tuple(items) => self.tuple(items, expected),
            ExprKind::Return(value) => self.return_(expr.span, value.as_deref()),
            ExprKind::Int(value) => Some(self.int_literal(expr.span, *value, expected)),
            ExprKind::Str(_) => Some(Ty::Text),
            ExprKind::Bool(_) => Some(Ty::Bool),
            ExprKind::Name(name) => self.name(expr, name, expected),
            ExprKind::Record { name, fields } => self.record_literal(expr, name, fields, expected),
            ExprKind::List(items) => self.list(expr, items, expected),
            ExprKind::Call { callee, args } => self.call(expr, callee, args, expected),
            ExprKind::Method {
                receiver,
                name,
                args,
            } => self.method(expr, receiver, name, args, expected),
            ExprKind::Field { receiver, name } => self.field(expr, receiver, name, expected),
            ExprKind::Index { receiver, index } => {
                let receiver_ty = self.synth(receiver);
                self.check(index, &Ty::Int);
                self.fail_site(expr, FailureOp::Index, expr.span.start);
                match receiver_ty? {
                    Ty::List(element) => Some(*element),
                    other => {
                        self.error(
                            "E041",
                            receiver.span,
                            format!("`{other}` cannot be indexed; only a `List` can"),
                        );
                        None
                    }
                }
            }
            ExprKind::Try(inner) => self.try_(expr, inner),
            ExprKind::Await(inner) => self.expr(inner, Mode::Value(expected)),
            ExprKind::Spawn(inner) => {
                self.check(inner, &io(Ty::Unit));
                Some(Ty::Unit)
            }
            ExprKind::Unary { op, operand } => self.unary(expr, *op, operand, expected),
            ExprKind::Binary { op, left, right }
                if matches!(left.kind, ExprKind::Binary { .. }) =>
            {
                self.binary_chain(expr, *op, left, right, expected)
            }
            ExprKind::Binary { op, left, right } => self.binary(expr, *op, left, right, expected),
            ExprKind::Match { scrutinee, arms } => {
                self.fail_site(expr, FailureOp::Match, expr.span.start);
                let scrutinee = self.synth(scrutinee);
                let branches = arms
                    .iter()
                    .map(|arm| Branch::Arm(arm, scrutinee.clone()))
                    .collect();
                let ty = self.branches(branches, mode);
                if let Some(scrutinee) = &scrutinee {
                    self.exhaustive(expr.span, scrutinee, arms);
                }
                ty
            }
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => {
                self.check(cond, &Ty::Bool);
                match otherwise {
                    None => {
                        self.block(then, Mode::Stmt);
                        Some(Ty::Unit)
                    }
                    Some(otherwise) => {
                        self.branches(vec![Branch::Block(then), Branch::Expr(otherwise)], mode)
                    }
                }
            }
            ExprKind::Block(block) => self.block(block, mode),
            ExprKind::Closure(closure) => self.closure(expr, closure, expected),
            ExprKind::RecordUpdate { name, fields, base } => {
                self.record_update(name, fields, base, expected)
            }
            ExprKind::TupleField { receiver, index } => match self.synth(receiver)? {
                Ty::Tuple(items) if *index < items.len() => Some(items[*index].clone()),
                Ty::Never => Some(Ty::Never),
                other => {
                    self.error(
                        "E041",
                        expr.span,
                        format!("`{other}` has no element `{index}`"),
                    );
                    None
                }
            },
            ExprKind::Break => {
                if let Some(top) = self.loops.last_mut() {
                    *top = true;
                }
                Some(Ty::Never)
            }
            ExprKind::Continue => Some(Ty::Never),
            ExprKind::Quantity { literal, unit } => self.quantity(expr, literal, unit),
            ExprKind::NamedArg { name, value } => {
                if !self.named_ok.contains(&key(expr)) {
                    self.error(
                        "E103",
                        name.span,
                        format!(
                            "named argument `{}` is only allowed in a direct call of a fn declared in Ignite, not through a fn value, closure, role or builtin",
                            name.name
                        ),
                    );
                }
                self.expr(value, Mode::Value(expected))
            }
            ExprKind::Brace { entries } => self.brace(expr, entries, expected),
        }
    }

    /// A closure literal (`docs/slice.md` section 13.3): parameter types
    /// from annotations, else from the expected fn type (E042 when
    /// neither gives one); the body checked against the expected return
    /// type or synthesised. `return`, loops and `?` inside it belong to
    /// it. Effects come from `ignite-check`; an effectful closure has an
    /// effectful fn type, which a pure fn type does not accept (E017).
    fn closure(&mut self, expr: &Expr, closure: &Closure, expected: Option<&Ty>) -> Option<Ty> {
        let (wanted_params, wanted_ret) = match expected {
            Some(Ty::Fn(ps, r) | Ty::EffectFn(ps, r, _)) if ps.len() == closure.params.len() => {
                (Some(ps.clone()), Some((**r).clone()))
            }
            _ => (None, None),
        };
        let mut params: Vec<(String, Ty)> = Vec::new();
        let mut bound: Vec<Option<Ty>> = Vec::new();
        let mut known = true;
        for (i, param) in closure.params.iter().enumerate() {
            let ty = match &param.ty {
                Some(t) => Some(self.resolve(t)),
                None => wanted_params.as_ref().map(|ps| ps[i].clone()),
            };
            let ty = ty.filter(|t| !t.has_param(&is_var));
            bound.push(ty.clone());
            match ty {
                Some(ty) => params.push((param.name.name.clone(), ty)),
                None => {
                    self.error(
                        "E042",
                        param.name.span,
                        format!(
                            "cannot infer the type of closure parameter `{}`; annotate it: `|{}: T|`",
                            param.name.name, param.name.name
                        ),
                    );
                    known = false;
                }
            }
        }
        let wanted_ret = wanted_ret.filter(|t| !t.has_param(&is_var));
        let ret_scope = std::mem::replace(&mut self.ret, wanted_ret.clone());
        let loops = std::mem::take(&mut self.loops);
        let in_test_body = std::mem::replace(&mut self.in_test_body, false);
        let effect_scope = std::mem::replace(&mut self.effect_scope, EffectScope::Open);
        self.closure_frames.push((self.scopes.len(), Vec::new()));
        self.scopes.push(BTreeMap::new());
        for (param, ty) in closure.params.iter().zip(bound) {
            self.bind(&param.name, ty);
        }
        let body = match &wanted_ret {
            Some(Ty::Unit) => self.expr(&closure.body, Mode::Stmt).map(|t| match t {
                Ty::Never => Ty::Never,
                _ => Ty::Unit,
            }),
            Some(ret) => self.check(&closure.body, ret),
            None => self.synth(&closure.body),
        };
        self.scopes.pop();
        let (_, captures) = self.closure_frames.pop().unwrap_or_default();
        self.ret = ret_scope;
        self.loops = loops;
        self.in_test_body = in_test_body;
        self.effect_scope = effect_scope;
        if !known {
            return None;
        }
        let ret = match (wanted_ret, body?) {
            (Some(ret), _) => ret,
            (None, Ty::Never) => Ty::Never,
            (None, ty) => ty,
        };
        let effects = self
            .symbols
            .closure_effects
            .get(&expr.span)
            .map(|sig| FnEffects {
                names: sig.params.clone(),
                requires: sig.requires.clone(),
            });
        let captures = captures
            .into_iter()
            .map(|mut c| {
                c.effects = effects
                    .iter()
                    .flat_map(|e| &e.requires)
                    .filter(|e| e.split_once('.').is_some_and(|(head, _)| head == c.name))
                    .cloned()
                    .collect();
                c
            })
            .collect();
        let tys: Vec<Ty> = params.iter().map(|(_, t)| t.clone()).collect();
        let ty = match &effects {
            None => fn_ty(tys, ret.clone()),
            Some(effects) => {
                match expected {
                    Some(Ty::Fn(..)) => {
                        self.error(
                            "E017",
                            expr.span,
                            format!(
                                "effectful closure (requires [{}]) where the pure fn type `{}` is expected",
                                effects.requires.join(", "),
                                expected.map(Ty::to_string).unwrap_or_default()
                            ),
                        );
                        return None;
                    }
                    Some(Ty::EffectFn(_, _, wanted)) => {
                        let missing = effects.uncovered(wanted);
                        if !missing.is_empty() {
                            self.error(
                                "E017",
                                expr.span,
                                format!(
                                    "effectful closure does not fit `{}`: its effects `{}` are not covered by the type's `requires`",
                                    expected.map(Ty::to_string).unwrap_or_default(),
                                    missing.join("`, `")
                                ),
                            );
                            return None;
                        }
                    }
                    _ => {}
                }
                Ty::EffectFn(tys, Box::new(ret.clone()), effects.clone())
            }
        };
        self.closures.insert(
            key(expr),
            ClosureInfo {
                params,
                ret,
                captures,
                effects,
            },
        );
        Some(ty)
    }

    /// `R { f = e, ..base }`: `base` has the record's type (from the
    /// expected type for a generic record, else synthesised); the fields
    /// are checked against that type's fields.
    fn record_update(
        &mut self,
        name: &Ident,
        fields: &[FieldInit],
        base: &Expr,
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        let Some(q) = self.records.get(&name.name).cloned() else {
            for field in fields {
                self.synth(&field.value);
            }
            self.synth(base);
            self.error(
                "E048",
                name.span,
                format!("unknown record type `{}`", name.name),
            );
            return None;
        };
        let generic = !self.generics_of(&q).is_empty();
        let record_ty = match expected {
            Some(t @ Ty::GenericRecord(eq, _)) if generic && *eq == q => Some(t.clone()),
            _ if !generic => Some(Ty::Record(q.clone())),
            _ => None,
        };
        let base_ty = match &record_ty {
            Some(t) => self.check(base, t),
            None => self.synth(base),
        };
        let record_ty = match (record_ty, base_ty) {
            (Some(t), _) => t,
            (None, Some(t @ Ty::GenericRecord(..))) => t,
            (None, Some(other)) => {
                let shown = Ty::Record(q.clone());
                self.mismatch(base.span, &shown, &other);
                return None;
            }
            (None, None) => return None,
        };
        if let Ty::GenericRecord(bq, _) = &record_ty
            && *bq != q
        {
            self.mismatch(base.span, &Ty::Record(q.clone()), &record_ty);
            return None;
        }
        for (i, field) in fields.iter().enumerate() {
            if fields[..i].iter().any(|f| f.name.name == field.name.name) {
                self.error(
                    "E047",
                    field.name.span,
                    format!(
                        "field `{}` is given twice in `{}` update",
                        field.name.name, name.name
                    ),
                );
            }
            match self.field_type(&record_ty, &field.name.name) {
                Some(ty) => {
                    self.check(&field.value, &ty);
                }
                None => {
                    self.error(
                        "E047",
                        field.name.span,
                        format!("record `{}` has no field `{}`", name.name, field.name.name),
                    );
                    self.synth(&field.value);
                }
            }
        }
        Some(record_ty)
    }

    /// `()` is `Unit`; `(a, b)` is checked element-wise against an expected
    /// tuple of the same length, else synthesised.
    fn tuple(&mut self, items: &[Expr], expected: Option<&Ty>) -> Option<Ty> {
        if items.is_empty() {
            return Some(Ty::Unit);
        }
        if let Some(Ty::Tuple(tys)) = expected
            && tys.len() == items.len()
        {
            let mut ok = true;
            for (item, ty) in items.iter().zip(tys) {
                ok &= self.check(item, ty).is_some();
            }
            return ok.then(|| Ty::Tuple(tys.clone()));
        }
        let tys: Vec<Option<Ty>> = items.iter().map(|item| self.synth(item)).collect();
        tys.into_iter().collect::<Option<Vec<Ty>>>().map(Ty::Tuple)
    }

    fn int_literal(&mut self, span: Span, value: i64, expected: Option<&Ty>) -> Ty {
        if expected == Some(&Ty::Byte) {
            if !(0..=255).contains(&value) {
                self.error(
                    "E044",
                    span,
                    format!("integer literal {value} is out of range for `Byte` (0 to 255)"),
                );
            }
            Ty::Byte
        } else if expected == Some(&Ty::U32) {
            if !(0..=i64::from(u32::MAX)).contains(&value) {
                self.error(
                    "E044",
                    span,
                    format!("integer literal {value} is out of range for `U32` (0 to 4294967295)"),
                );
            }
            Ty::U32
        } else {
            Ty::Int
        }
    }

    fn name(&mut self, expr: &Expr, name: &Ident, expected: Option<&Ty>) -> Option<Ty> {
        if let Some(system) = self.current_system.clone()
            && let Some(ty) = self.member_value(expr, &system, name, expected)
        {
            return ty;
        }
        if let Some(local) = self.use_local(&name.name) {
            return local;
        }
        if name.name == "Unit" {
            return Some(Ty::Unit);
        }
        if let Some(q) = self.consts.get(&name.name).cloned() {
            let ty = self.const_ty(&q);
            self.callees.insert(key(expr), Callee::Const(q));
            return ty;
        }
        if let Some(q) = self.expected_enum(&name.name, expected) {
            let owner = q.name.clone();
            let payload = self
                .selected_variant(&Ty::Enum(q.clone()), &name.name)
                .map_or(0, |(_, _, payload)| payload.len());
            if payload > 0 && self.symbols.enum_of_variant(&name.name) != Some(owner.as_str()) {
                self.error(
                    "E003",
                    name.span,
                    format!(
                        "variant `{}` takes {payload} payload value{}; call it",
                        name.name,
                        if payload == 1 { "" } else { "s" }
                    ),
                );
            }
            return self.enum_variant(expr, &owner, Some(q), name, None, expected);
        }
        if self.symbols.variants.contains_key(&name.name) {
            let owner = self.variant_owner(name, expected)?;
            return self.variant(expr, &owner, name, None, expected);
        }
        if let Some(q) = self.fns.get(&name.name).cloned()
            && let Some(sig) = self.sig_of(&q)
        {
            self.callees.insert(key(expr), Callee::Fn(q.clone()));
            let mut ty = fn_ty(sig.params, sig.ret);
            let tparams = self.generics_of(&q);
            if !tparams.is_empty() {
                let instance =
                    self.infer(expr.span, &name.name, &tparams, &[], &[], &ty, expected)?;
                self.bounds_met(expr, &name.name, &tparams, &instance);
                ty = ty.substitute(&tparams, &instance);
                self.instances.insert(key(expr), instance);
            }
            if sig.effectful {
                return self.effectful_value(name, ty, expected);
            }
            return Some(ty);
        }
        if self.system_members.contains_key(&name.name) {
            self.system
                .references
                .insert(key(expr), Reference::System(name.name.clone()));
            return Some(Ty::SystemRef);
        }
        if let Some(q) = self.protocols.get(&name.name).cloned() {
            self.system
                .references
                .insert(key(expr), Reference::Protocol(q));
            return Some(Ty::Protocol);
        }
        if self.is_out_of_scope_variant(&name.name) {
            self.out_of_scope_variant(name);
            return None;
        }
        if self.current_system.is_some() || self.assembly_system.is_some() {
            self.error("E003", name.span, format!("unknown name `{}`", name.name));
            return None;
        }
        self.unresolved(name.span, &name.name);
        None
    }

    fn record_literal(
        &mut self,
        expr: &Expr,
        name: &Ident,
        fields: &[FieldInit],
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        let Some(q) = self.records.get(&name.name).cloned() else {
            for field in fields {
                self.synth(&field.value);
            }
            self.error(
                "E048",
                name.span,
                format!("unknown record type `{}`", name.name),
            );
            return None;
        };
        let entries: Vec<(&Ident, &Expr)> = fields.iter().map(|f| (&f.name, &f.value)).collect();
        self.fields_literal(
            expr.span,
            Some(expr),
            &q,
            &name.name,
            &entries,
            expected,
            system::Literal::Record,
        )
    }

    fn list(&mut self, expr: &Expr, items: &[Expr], expected: Option<&Ty>) -> Option<Ty> {
        if let Some(Ty::List(element)) = expected {
            for item in items {
                self.check(item, element);
            }
            return expected.cloned();
        }
        if items.is_empty() {
            match expected {
                Some(other) => self.error(
                    "E040",
                    expr.span,
                    format!("type mismatch: expected `{other}`, found a list"),
                ),
                None => {
                    if let Some(hole) = self.hole() {
                        return Some(Ty::List(Box::new(hole)));
                    }
                    self.error(
                        "E042",
                        expr.span,
                        "cannot infer the element type of `[]`; add a type annotation".to_string(),
                    )
                }
            }
            return None;
        }
        let first = items.iter().position(|i| !is_literal(i)).unwrap_or(0);
        let Some(element) = self.synth(&items[first]) else {
            for (i, item) in items.iter().enumerate() {
                if i != first {
                    self.synth(item);
                }
            }
            return None;
        };
        for (i, item) in items.iter().enumerate() {
            if i != first {
                self.check(item, &element);
            }
        }
        Some(Ty::List(Box::new(element)))
    }

    /// Check call arguments against parameter types (E043 on a count
    /// mismatch; every argument is still typed).
    fn arguments(&mut self, span: Span, what: &str, params: &[Ty], args: &[Expr]) {
        let refs: Vec<&Expr> = args.iter().collect();
        self.arguments_refs(span, what, params, &refs);
    }

    /// [`Typer::arguments`] over arguments in parameter order (named
    /// arguments reordered by [`Typer::ordered_args`]).
    fn arguments_refs(&mut self, span: Span, what: &str, params: &[Ty], args: &[&Expr]) {
        if params.len() != args.len() {
            self.error(
                "E043",
                span,
                format!(
                    "wrong number of arguments to `{what}`: expected {}, found {}",
                    params.len(),
                    args.len()
                ),
            );
        }
        for (i, arg) in args.iter().enumerate() {
            match params.get(i) {
                Some(param) => self.check(arg, param),
                None => self.synth(arg),
            };
        }
    }

    fn call(
        &mut self,
        expr: &Expr,
        callee: &Expr,
        args: &[Expr],
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        match &callee.kind {
            ExprKind::Name(name) => {
                if let Some(local) = self.use_local(&name.name) {
                    let Some(ty) = self.record(callee, local) else {
                        for arg in args {
                            self.synth(arg);
                        }
                        return None;
                    };
                    if matches!(ty, Ty::EffectFn(..)) {
                        self.effect_site(expr, &name.name);
                    }
                    let (Ty::Fn(params, ret) | Ty::EffectFn(params, ret, _)) = ty else {
                        self.error(
                            "E040",
                            callee.span,
                            format!(
                                "type mismatch: `{}` has type `{ty}` and cannot be called",
                                name.name
                            ),
                        );
                        for arg in args {
                            self.synth(arg);
                        }
                        return None;
                    };
                    self.callees.insert(key(expr), Callee::Local);
                    self.arguments(expr.span, &name.name, &params, args);
                    return Some(*ret);
                }
                let known = match self.fns.get(&name.name).cloned() {
                    Some(q) => Some(q),
                    None => self.expected_module_fn(&name.name, expected),
                };
                if let Some(q) = known
                    && let Some(sig) = self.sig_of(&q)
                {
                    self.callees.insert(key(expr), Callee::Fn(q.clone()));
                    let ordered = self.ordered_args(expr, &q, 0, args)?;
                    let tparams = self.generics_of(&q);
                    if !tparams.is_empty() {
                        let ret = self.generic_call_refs(
                            expr,
                            &name.name,
                            &tparams,
                            &sig.params,
                            &ordered,
                            &[],
                            &sig.ret,
                            expected,
                        )?;
                        let instance = self.instances.get(&key(expr)).cloned().unwrap_or_default();
                        let params = sig.params.iter().map(|p| p.substitute(&tparams, &instance));
                        self.record(callee, Some(fn_ty(params.collect(), ret.clone())));
                        return Some(ret);
                    }
                    self.record(callee, Some(fn_ty(sig.params.clone(), sig.ret.clone())));
                    self.arguments_refs(expr.span, &name.name, &sig.params, &ordered);
                    return Some(sig.ret);
                }
                if name.name == "module" {
                    return self.module_call(expr, callee, args, expected);
                }
                if name.name == "log" {
                    let sig = builtins::log();
                    self.record(callee, Some(fn_ty(sig.params.clone(), sig.ret.clone())));
                    self.callees.insert(key(expr), Callee::Builtin(sig.builtin));
                    self.arguments(expr.span, "log", &sig.params, args);
                    return Some(sig.ret);
                }
                if let Some(q) = self.expected_enum(&name.name, expected) {
                    let owner = q.name.clone();
                    let ty = self.enum_variant(expr, &owner, Some(q), name, Some(args), expected);
                    self.constructor_type(callee, ty.as_ref(), &name.name);
                    return ty;
                }
                if self.symbols.variants.contains_key(&name.name) {
                    let Some(owner) = self.variant_owner(name, expected) else {
                        for arg in args {
                            self.synth(arg);
                        }
                        return None;
                    };
                    let ty = self.variant(expr, &owner, name, Some(args), expected);
                    self.constructor_type(callee, ty.as_ref(), &name.name);
                    return ty;
                }
                for arg in args {
                    self.synth(arg);
                }
                if self.is_out_of_scope_variant(&name.name) {
                    self.out_of_scope_variant(name);
                } else {
                    self.unresolved(name.span, &name.name);
                }
                None
            }
            ExprKind::Field { receiver, name } => match self.path_head(receiver) {
                Some(head) => {
                    self.path_call(expr, receiver, head, name, args, Some(callee), expected)
                }
                None => self.expression_call(expr, callee, args),
            },
            _ => self.expression_call(expr, callee, args),
        }
    }

    /// `e(args)` whose callee is an expression, not a name or a path
    /// (`docs/slice.md` 20.4.2): a `Callee::Local` call when the callee has
    /// a fn type, E040 otherwise, E043 on arity. A callee of effectful fn
    /// type whose effects `ignite-check` could not charge (no declared type,
    /// such as a call's result) is E042 (20.7).
    fn expression_call(&mut self, expr: &Expr, callee: &Expr, args: &[Expr]) -> Option<Ty> {
        let Some(ty) = self.synth(callee) else {
            for arg in args {
                self.synth(arg);
            }
            return None;
        };
        let (Ty::Fn(params, ret) | Ty::EffectFn(params, ret, _)) = &ty else {
            self.error(
                "E040",
                callee.span,
                format!("type mismatch: this expression has type `{ty}` and cannot be called"),
            );
            for arg in args {
                self.synth(arg);
            }
            return None;
        };
        if matches!(ty, Ty::EffectFn(..)) && self.unchecked_effects(expr) {
            self.error(
                "E042",
                expr.span,
                format!(
                    "cannot check the effects of a call of this expression; bind it first with its type: `let f: {ty} = …`"
                ),
            );
        }
        self.callees.insert(key(expr), Callee::Local);
        if params.len() != args.len() {
            self.error(
                "E043",
                expr.span,
                format!(
                    "wrong number of arguments to this expression: expected {}, found {}",
                    params.len(),
                    args.len()
                ),
            );
        }
        for (i, arg) in args.iter().enumerate() {
            match params.get(i) {
                Some(param) => self.check(arg, param),
                None => self.synth(arg),
            };
        }
        Some((**ret).clone())
    }

    /// Record the function type of a variant constructor's callee node.
    fn constructor_type(&mut self, callee: &Expr, ty: Option<&Ty>, variant: &str) {
        let Some(ty) = ty else { return };
        let payload = self
            .selected_variant(ty, variant)
            .map(|(_, _, payload)| payload);
        if let Some(payload) = payload {
            self.record(callee, Some(fn_ty(payload, ty.clone())));
        }
    }

    /// `Owner.name(args)`: a variant constructor, a protocol role or a
    /// builtin static. `callee` is the `Field` node when the parser produced
    /// a `Call`.
    #[allow(clippy::too_many_arguments)]
    fn path_call(
        &mut self,
        expr: &Expr,
        head_expr: &Expr,
        head: &Ident,
        name: &Ident,
        args: &[Expr],
        callee: Option<&Expr>,
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        self.record(head_expr, Some(Ty::Unit));
        let owner = head.name.as_str();
        let what = format!("{owner}.{}", name.name);
        if let Some(q) = self.fns.get(&what).cloned()
            && let Some(sig) = self.sig_of(&q)
        {
            self.callees.insert(key(expr), Callee::Fn(q.clone()));
            let ordered = self.ordered_args(expr, &q, 0, args)?;
            let tparams = self.generics_of(&q);
            if !tparams.is_empty() {
                let ret = self.generic_call_refs(
                    expr,
                    &what,
                    &tparams,
                    &sig.params,
                    &ordered,
                    &[],
                    &sig.ret,
                    expected,
                )?;
                let instance = self.instances.get(&key(expr)).cloned().unwrap_or_default();
                let params = sig.params.iter().map(|p| p.substitute(&tparams, &instance));
                if let Some(callee) = callee {
                    self.record(callee, Some(fn_ty(params.collect(), ret.clone())));
                }
                return Some(ret);
            }
            if let Some(callee) = callee {
                self.record(callee, Some(fn_ty(sig.params.clone(), sig.ret.clone())));
            }
            self.arguments_refs(expr.span, &what, &sig.params, &ordered);
            return Some(sig.ret);
        }
        if self.is_enum(owner) {
            let ty = self.variant(expr, owner, name, Some(args), expected);
            if let Some(callee) = callee {
                self.constructor_type(callee, ty.as_ref(), &name.name);
            }
            return ty;
        }
        if let Some(q) = self.protocols.get(owner).cloned() {
            let Some(sig) = self.role(&q, &name.name) else {
                for arg in args {
                    self.synth(arg);
                }
                self.unresolved(name.span, &what);
                return None;
            };
            self.callees.insert(
                key(expr),
                Callee::Role {
                    protocol: q.clone(),
                    role: name.name.clone(),
                },
            );
            let tparams = self.role_generics(&q, &name.name);
            if !tparams.is_empty() {
                let ret = self.generic_call(
                    expr,
                    &what,
                    &tparams,
                    &sig.params,
                    args,
                    &[],
                    &sig.ret,
                    expected,
                )?;
                if let Some(callee) = callee {
                    let instance = self.instances.get(&key(expr)).cloned().unwrap_or_default();
                    let params = sig.params.iter().map(|p| p.substitute(&tparams, &instance));
                    self.record(callee, Some(fn_ty(params.collect(), ret.clone())));
                }
                return Some(ret);
            }
            if let Some(callee) = callee {
                self.record(callee, Some(fn_ty(sig.params.clone(), sig.ret.clone())));
            }
            self.arguments(expr.span, &what, &sig.params, args);
            return Some(sig.ret);
        }
        if owner == "List" && matches!(name.name.as_str(), "empty" | "with_capacity") {
            let (builtin, params) = if name.name == "empty" {
                (crate::Builtin::ListEmpty, Vec::new())
            } else {
                (crate::Builtin::ListWithCapacity, vec![Ty::Int])
            };
            self.callees.insert(key(expr), Callee::Builtin(builtin));
            self.arguments(expr.span, &what, &params, args);
            return match expected {
                Some(ty @ Ty::List(_)) => {
                    if let Some(callee) = callee {
                        self.record(callee, Some(fn_ty(params, ty.clone())));
                    }
                    Some(ty.clone())
                }
                Some(other) => {
                    self.error(
                        "E040",
                        expr.span,
                        format!("type mismatch: expected `{other}`, found a list"),
                    );
                    None
                }
                None => {
                    if let Some(hole) = self.hole() {
                        return Some(Ty::List(Box::new(hole)));
                    }
                    self.error(
                        "E042",
                        expr.span,
                        format!(
                            "cannot infer the element type of `List.{}`; add a type annotation",
                            if name.name == "empty" {
                                "empty()"
                            } else {
                                "with_capacity(..)"
                            }
                        ),
                    );
                    None
                }
            };
        }
        if owner == "Store" && name.name == "new" {
            self.callees
                .insert(key(expr), Callee::Builtin(crate::Builtin::StoreNew));
            self.arguments(expr.span, &what, &[], args);
            return match expected {
                Some(ty @ Ty::Store(_)) => {
                    if let Some(callee) = callee {
                        self.record(callee, Some(fn_ty(vec![], ty.clone())));
                    }
                    Some(ty.clone())
                }
                Some(other) => {
                    self.error(
                        "E040",
                        expr.span,
                        format!("type mismatch: expected `{other}`, found a store"),
                    );
                    None
                }
                None => {
                    if let Some(hole) = self.hole() {
                        return Some(Ty::Store(Box::new(hole)));
                    }
                    self.error(
                        "E042",
                        expr.span,
                        "cannot infer the element type of `Store.new()`; add a type annotation"
                            .to_string(),
                    );
                    None
                }
            };
        }
        if owner == "Channel" && name.name == "bounded" {
            self.callees
                .insert(key(expr), Callee::Builtin(crate::Builtin::ChannelBounded));
            self.arguments(expr.span, &what, &[Ty::Int], args);
            let ends = |item: Ty| {
                Ty::Tuple(vec![
                    Ty::Sender(Box::new(item.clone())),
                    Ty::Receiver(Box::new(item)),
                ])
            };
            let ty = match expected {
                Some(Ty::Tuple(items)) => match items.as_slice() {
                    [Ty::Sender(a), Ty::Receiver(b)] if a == b => Some(ends((**a).clone())),
                    _ => None,
                },
                Some(_) => None,
                None => match self.hole() {
                    Some(hole) => return Some(ends(hole)),
                    None => {
                        self.error(
                            "E042",
                            expr.span,
                            "cannot infer the element type of `Channel.bounded(..)`; add a type annotation or send a value"
                                .to_string(),
                        );
                        return None;
                    }
                },
            };
            return match ty {
                Some(ty) => {
                    if let Some(callee) = callee {
                        self.record(callee, Some(fn_ty(vec![Ty::Int], ty.clone())));
                    }
                    Some(ty)
                }
                None => {
                    let other = expected.map(Ty::to_string).unwrap_or_default();
                    self.error(
                        "E040",
                        expr.span,
                        format!(
                            "type mismatch: expected `{other}`, found a channel `(Sender<T>, Receiver<T>)`"
                        ),
                    );
                    None
                }
            };
        }
        if owner == "Map" && name.name == "empty" {
            self.callees
                .insert(key(expr), Callee::Builtin(crate::Builtin::MapEmpty));
            self.arguments(expr.span, &what, &[], args);
            return match expected {
                Some(ty @ Ty::Map(..)) => {
                    if let Some(callee) = callee {
                        self.record(callee, Some(fn_ty(vec![], ty.clone())));
                    }
                    Some(ty.clone())
                }
                Some(other) => {
                    self.error(
                        "E040",
                        expr.span,
                        format!("type mismatch: expected `{other}`, found a map"),
                    );
                    None
                }
                None => {
                    if let Some(k) = self.hole() {
                        let v = self.hole().unwrap_or_else(|| k.clone());
                        return Some(Ty::Map(Box::new(k), Box::new(v)));
                    }
                    self.error(
                        "E042",
                        expr.span,
                        "cannot infer the key and value types of `Map.empty()`; add a type annotation"
                            .to_string(),
                    );
                    None
                }
            };
        }
        if let Some(sig) = builtins::static_fn(owner, &name.name) {
            if let Some(callee) = callee {
                self.record(callee, Some(fn_ty(sig.params.clone(), sig.ret.clone())));
            }
            self.callees.insert(key(expr), Callee::Builtin(sig.builtin));
            self.arguments(expr.span, &what, &sig.params, args);
            return Some(sig.ret);
        }
        for arg in args {
            self.synth(arg);
        }
        self.error(
            "E041",
            name.span,
            format!("`{owner}` has no static function `{}`", name.name),
        );
        None
    }

    /// Whether the bare variant `name` needs the expected type: it has more
    /// than one owner in scope, or none in scope but one in a module.
    fn is_ambiguous_variant(&self, name: &str) -> bool {
        self.lookup(name).is_none()
            && (self
                .symbols
                .variants
                .get(name)
                .is_some_and(|owners| owners.len() > 1)
                || self.is_out_of_scope_variant(name))
    }

    /// Whether `name` is no variant in scope, constant or fn, but a variant
    /// of an enum one of the typed modules defines.
    fn is_out_of_scope_variant(&self, name: &str) -> bool {
        !self.symbols.variants.contains_key(name)
            && !self.consts.contains_key(name)
            && !self.fns.contains_key(name)
            && self.modules.values().any(|m| {
                m.enums
                    .values()
                    .any(|variants| variants.iter().any(|(v, _)| v == name))
            })
    }

    /// The user enum of the expected type when it declares the variant
    /// `name`, whether or not that enum is in scope.
    /// A bare fn name that is not in scope, resolved in the module of the
    /// expected record or enum type, as a bare variant is (`docs/slice.md`
    /// section 14.14 item 17): `window = rolling(30 days)` names
    /// `std.contract.rolling` where a `std.contract.Window` is expected.
    fn expected_module_fn(&self, name: &str, expected: Option<&Ty>) -> Option<QualName> {
        let (Some(Ty::Record(q))
        | Some(Ty::GenericRecord(q, _))
        | Some(Ty::Enum(q))
        | Some(Ty::GenericEnum(q, _))) = expected
        else {
            return None;
        };
        if q.module.is_empty() || q.module == self.module {
            return None;
        }
        self.module_types(&q.module)?.fns.get(name)?;
        Some(QualName {
            module: q.module.clone(),
            name: name.to_string(),
        })
    }

    fn expected_enum(&self, name: &str, expected: Option<&Ty>) -> Option<QualName> {
        let (Some(Ty::Enum(q)) | Some(Ty::GenericEnum(q, _))) = expected else {
            return None;
        };
        if !q.module.is_empty() {
            return self
                .module_types(&q.module)?
                .enums
                .get(&q.name)?
                .iter()
                .any(|(v, _)| v == name)
                .then(|| q.clone());
        }
        self.selected_variant(&Ty::Enum(q.clone()), name)
            .map(|(q, _, _)| q)
    }

    /// `E003` for a bare variant of an enum that is not in scope and not
    /// the expected type.
    fn out_of_scope_variant(&mut self, name: &Ident) {
        self.error(
            "E003",
            name.span,
            format!(
                "unknown name `{}`; a variant of an enum that is not imported is only resolved where that enum is the expected type",
                name.name
            ),
        );
    }

    /// The enum a bare variant `name` belongs to: the expected type's when
    /// it declares `name`, else the only enum in scope declaring it. E005
    /// when neither decides.
    fn variant_owner(&mut self, name: &Ident, expected: Option<&Ty>) -> Option<String> {
        let owners = self.symbols.variants.get(&name.name)?.clone();
        if let Some((q, variants)) = expected.and_then(|t| self.variants(t))
            && variants.iter().any(|(v, _)| *v == name.name)
            && owners.contains(&q.name)
        {
            return Some(q.name);
        }
        if let [owner] = owners.as_slice() {
            return Some(owner.clone());
        }
        let first = owners.first().cloned().unwrap_or_default();
        let listed = owners
            .iter()
            .map(|o| format!("`{o}`"))
            .collect::<Vec<_>>()
            .join(", ");
        self.error(
            "E005",
            name.span,
            format!(
                "ambiguous variant `{}`: declared by {listed}; qualify it, e.g. `{first}.{}`",
                name.name, name.name
            ),
        );
        None
    }

    /// A variant used as a value (`args` is `None`) or a constructor call.
    fn variant(
        &mut self,
        expr: &Expr,
        owner: &str,
        variant: &Ident,
        args: Option<&[Expr]>,
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        if BUILTIN_ENUMS.contains(&owner) && !self.enums.contains_key(owner) {
            return self.builtin_variant(expr, owner, variant, args.unwrap_or(&[]), expected);
        }
        let q = self.enums.get(owner).cloned();
        self.enum_variant(expr, owner, q, variant, args, expected)
    }

    /// Variant `variant` of the user enum `q` (named `owner` in messages),
    /// as a value or a constructor call.
    fn enum_variant(
        &mut self,
        expr: &Expr,
        owner: &str,
        q: Option<QualName>,
        variant: &Ident,
        args: Option<&[Expr]>,
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        let what = format!("{owner}.{}", variant.name);
        let found = q
            .as_ref()
            .and_then(|q| self.selected_variant(&Ty::Enum(q.clone()), &variant.name));
        let (Some(q), Some((_, index, payload))) = (q, found) else {
            for arg in args.unwrap_or(&[]) {
                self.synth(arg);
            }
            self.unresolved(variant.span, &what);
            return None;
        };
        self.callees.insert(
            key(expr),
            Callee::Variant {
                enum_: q.clone(),
                index,
            },
        );
        let tparams = self.generics_of(&q);
        if !tparams.is_empty() {
            let generic = Ty::GenericEnum(
                q.clone(),
                tparams.iter().map(|p| Ty::Param(p.clone())).collect(),
            );
            let params: &[Ty] = if args.is_some() { &payload } else { &[] };
            let args = args.unwrap_or(&[]);
            return self.generic_call(expr, &what, &tparams, params, args, &[], &generic, expected);
        }
        if let Some(args) = args {
            self.arguments(expr.span, &what, &payload, args);
        }
        Some(Ty::Enum(q))
    }

    /// `Some`, `None`, `Ok`, `Err`, `Data`, `Eof`: their type parameters
    /// come from the argument or the expected type.
    fn builtin_variant(
        &mut self,
        expr: &Expr,
        owner: &str,
        variant: &Ident,
        args: &[Expr],
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        let name = variant.name.as_str();
        if let Some(index) = EXPOSURE_VARIANTS.iter().position(|v| *v == name) {
            self.callees.insert(
                key(expr),
                Callee::Variant {
                    enum_: QualName::builtin("Exposure"),
                    index,
                },
            );
            self.arguments(expr.span, name, &[], args);
            return Some(exposure());
        }
        let index = match name {
            "Some" | "Ok" | "Data" => 0,
            "None" | "Err" | "Eof" => 1,
            _ => {
                for arg in args {
                    self.synth(arg);
                }
                self.unresolved(variant.span, name);
                return None;
            }
        };
        self.callees.insert(
            key(expr),
            Callee::Variant {
                enum_: QualName::builtin(owner),
                index,
            },
        );
        match name {
            "Data" | "Eof" => {
                let params = if name == "Data" {
                    vec![bytes()]
                } else {
                    vec![]
                };
                self.arguments(expr.span, name, &params, args);
                Some(Ty::Input)
            }
            "Some" => match expected {
                Some(Ty::Option(some)) => {
                    self.arguments(expr.span, name, std::slice::from_ref(&**some), args);
                    expected.cloned()
                }
                _ => {
                    if args.len() != 1 {
                        self.arguments(expr.span, name, &[Ty::Unit], args);
                        return None;
                    }
                    let some = self.synth(&args[0])?;
                    Some(Ty::Option(Box::new(some)))
                }
            },
            "None" => {
                self.arguments(expr.span, name, &[], args);
                match expected {
                    Some(Ty::Option(_)) => expected.cloned(),
                    Some(other) => {
                        self.error(
                            "E040",
                            expr.span,
                            format!("type mismatch: expected `{other}`, found `None`"),
                        );
                        None
                    }
                    None => {
                        if let Some(hole) = self.hole() {
                            return Some(Ty::Option(Box::new(hole)));
                        }
                        self.error(
                            "E042",
                            expr.span,
                            "cannot infer the type of `None`; add a type annotation".to_string(),
                        );
                        None
                    }
                }
            }
            _ => match expected {
                Some(Ty::Result(ok, err)) => {
                    let half = if name == "Ok" { &**ok } else { &**err };
                    self.arguments(expr.span, name, std::slice::from_ref(half), args);
                    expected.cloned()
                }
                Some(other) => {
                    for arg in args {
                        self.synth(arg);
                    }
                    self.error(
                        "E040",
                        expr.span,
                        format!("type mismatch: expected `{other}`, found `{name}(..)`"),
                    );
                    None
                }
                None => {
                    let mut known = Vec::new();
                    for arg in args {
                        known.push(self.synth(arg));
                    }
                    if self.holes.is_some()
                        && let [Some(known)] = known.as_slice()
                    {
                        let known = known.clone();
                        let hole = self.hole()?;
                        return Some(if name == "Ok" {
                            Ty::Result(Box::new(known), Box::new(hole))
                        } else {
                            Ty::Result(Box::new(hole), Box::new(known))
                        });
                    }
                    self.hole_wanted = true;
                    let other = if name == "Ok" { "error" } else { "success" };
                    self.error(
                        "E042",
                        expr.span,
                        format!(
                            "cannot infer the {other} type of `{name}(..)`; add a type annotation"
                        ),
                    );
                    None
                }
            },
        }
    }

    fn method(
        &mut self,
        expr: &Expr,
        receiver: &Expr,
        name: &Ident,
        args: &[Expr],
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        if let Some(head) = self.path_head(receiver) {
            return self.path_call(expr, receiver, head, name, args, None, expected);
        }
        let Some(receiver_ty) = self.synth(receiver) else {
            for arg in args {
                self.synth(arg);
            }
            return None;
        };
        if let Ty::List(element) = &receiver_ty
            && name.name == "map"
        {
            return self.list_map(expr, element, args, expected);
        }
        let bounded = match &receiver_ty {
            Ty::Param(param) if self.bounded.contains(param) => builtins::stream_method(&name.name),
            _ => None,
        };
        if let Some(sig) = bounded.or_else(|| builtins::method(&receiver_ty, &name.name)) {
            if matches!(receiver_ty, Ty::Store(_) | Ty::Sender(_) | Ty::Receiver(_)) {
                self.effect_site(expr, &name.name);
            }
            if let Ty::Capability(kind) = receiver_ty {
                if sig.builtin != Builtin::ChildProcessId {
                    self.effect_site(expr, &format!("{}.{}", kind.name(), name.name));
                }
                if kind == Capability::Directory {
                    self.path_literal(&name.name, args.first());
                }
            }
            match sig.builtin {
                Builtin::ListAt | Builtin::ListSet => {
                    self.fail_site(expr, FailureOp::Index, expr.span.start)
                }
                Builtin::ListSlice => self.fail_site(expr, FailureOp::Slice, expr.span.start),
                _ => {}
            }
            self.captured_change(expr, receiver, sig.builtin);
            self.callees.insert(key(expr), Callee::Builtin(sig.builtin));
            self.arguments(expr.span, &name.name, &sig.params, args);
            return Some(sig.ret);
        }
        let no_method = |typer: &mut Self, detail: String| {
            typer.error(
                "E041",
                name.span,
                format!("`{receiver_ty}` has no method `{}`{detail}", name.name),
            );
            for arg in args {
                typer.synth(arg);
            }
            None
        };
        if BUILTIN_METHODS.contains(&name.name.as_str()) && !self.fns.contains_key(&name.name) {
            return no_method(self, String::new());
        }
        let field_fn = match (&receiver_ty, self.field_type(&receiver_ty, &name.name)) {
            (
                Ty::Record(q) | Ty::GenericRecord(q, _),
                Some(field @ (Ty::Fn(..) | Ty::EffectFn(..))),
            ) => Some((q.clone(), field)),
            _ => None,
        };
        let function = self
            .fns
            .get(&name.name)
            .cloned()
            .and_then(|q| Some((q.clone(), self.sig_of(&q)?)));
        match (field_fn, function) {
            (Some(_), Some(_)) => no_method(
                self,
                format!(
                    ": ambiguous between the field `{0}` of fn type and the function `{0}`; call `{0}(x, ..)` or bind the field first, `let f = x.{0}`",
                    name.name
                ),
            ),
            (Some((record, field)), None) => {
                if matches!(field, Ty::EffectFn(..)) {
                    self.effect_site(expr, &name.name);
                }
                let (Ty::Fn(params, ret) | Ty::EffectFn(params, ret, _)) = field else {
                    return None;
                };
                self.callees.insert(
                    key(expr),
                    Callee::FieldFn {
                        record,
                        field: name.name.clone(),
                    },
                );
                self.arguments(expr.span, &name.name, &params, args);
                Some(*ret)
            }
            (None, Some((q, sig))) => {
                let Some((first, rest)) = sig.params.split_first() else {
                    return no_method(
                        self,
                        format!("; function `{}` has no parameters", name.name),
                    );
                };
                let tparams = self.generics_of(&q);
                let fits = if tparams.is_empty() {
                    compatible(&receiver_ty, first)
                } else {
                    let vars: Vec<Ty> =
                        tparams.iter().map(|p| Ty::Param(format!("'{p}"))).collect();
                    let fresh = first.substitute(&tparams, &vars);
                    let mut binds = BTreeMap::new();
                    unify(&fresh, &receiver_ty, &mut binds);
                    let applied = apply(&fresh, &binds);
                    !applied.has_param(&is_var) && compatible(&receiver_ty, &applied)
                };
                if !fits {
                    return no_method(
                        self,
                        format!(
                            "; function `{}` takes `{first}` as its first parameter",
                            name.name
                        ),
                    );
                }
                if !self.in_test
                    && q.module == self.module
                    && self.symbols.test_only.contains(&q.name)
                {
                    self.error(
                        "E051",
                        name.span,
                        format!(
                            "`{}` is test-only; only tests and other test-only items can use it",
                            name.name
                        ),
                    );
                }
                if sig.effectful && BUILTIN_METHODS.contains(&name.name.as_str()) {
                    self.effect_site(expr, &name.name);
                }
                self.callees.insert(key(expr), Callee::Fn(q.clone()));
                let ordered = self.ordered_args(expr, &q, 1, args)?;
                if !tparams.is_empty() {
                    let pre = [(receiver_ty.clone(), first.clone())];
                    return self.generic_call_refs(
                        expr, &name.name, &tparams, rest, &ordered, &pre, &sig.ret, expected,
                    );
                }
                self.arguments_refs(expr.span, &name.name, rest, &ordered);
                Some(sig.ret)
            }
            (None, None) => no_method(self, String::new()),
        }
    }

    /// `l.map(f)` on a `List<element>`: `f` is a `fn(element) -> U`, `U`
    /// from the expected `List<U>` or from `f`'s type. Where the caller
    /// has effects, `f` may be an effectful closure or named fn whose
    /// effects the caller's cover (`docs/slice.md` 20.4.3); elsewhere it is
    /// pure.
    fn list_map(
        &mut self,
        expr: &Expr,
        element: &Ty,
        args: &[Expr],
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        self.callees
            .insert(key(expr), Callee::Builtin(crate::Builtin::ListMap));
        let [f] = args else {
            self.arguments(expr.span, "map", &[Ty::Unit], args);
            return None;
        };
        let own = match self.effect_scope {
            EffectScope::Pure => None,
            _ => self.map_fn_effects(f),
        };
        self.map_local_site(f, own.is_some());
        let wanted = |ret: Ty| match &own {
            Some(effects) => Ty::EffectFn(vec![element.clone()], Box::new(ret), effects.clone()),
            None => fn_ty(vec![element.clone()], ret),
        };
        if let Some(Ty::List(item)) = expected {
            if self.check(f, &wanted((**item).clone())).is_some()
                && let Some(effects) = &own
            {
                self.map_effects_fit(f, effects);
            }
            return expected.cloned();
        }
        let open = wanted(Ty::Param("'U".into()));
        let given = (own.is_some() || matches!(f.kind, ExprKind::Closure(_))).then_some(&open);
        match self.expr(f, Mode::Value(given))? {
            Ty::Fn(params, ret) if params.len() == 1 && compatible(element, &params[0]) => {
                Some(Ty::List(ret))
            }
            Ty::EffectFn(params, ret, effects)
                if own.is_some() && params.len() == 1 && compatible(element, &params[0]) =>
            {
                self.map_effects_fit(f, &effects).then_some(Ty::List(ret))
            }
            Ty::Never => None,
            other => {
                self.error(
                    "E040",
                    f.span,
                    format!(
                        "type mismatch: `map` takes a pure `fn({element}) -> U`, found `{other}`"
                    ),
                );
                None
            }
        }
    }

    /// E042 for `map`'s argument `f`, a local of effectful fn type, where
    /// the caller's effects are inferred (no `requires`) and `ignite-check`
    /// could not charge them because it cannot tell the local's type (bound
    /// from a call, a field of a call's result, a pattern; `docs/slice.md`
    /// 20.4.3), as for a call of the local. Under a `requires` the type
    /// checker checks the effects itself (`map_effects_fit`).
    fn map_local_site(&mut self, f: &Expr, effectful: bool) {
        let ExprKind::Name(name) = &f.kind else {
            return;
        };
        if !effectful
            || !matches!(self.effect_scope, EffectScope::Open)
            || !self.unchecked_effects(f)
        {
            return;
        }
        let Some(Some(ty)) = self.lookup(&name.name) else {
            return;
        };
        self.error(
            "E042",
            f.span,
            format!(
                "cannot check the effects of `{}` passed to `map`: its type is not declared; bind it with its type: `let {}: {ty} = …`",
                name.name, name.name
            ),
        );
    }

    /// The effects of `map`'s argument `f` when it is an effectful closure
    /// literal, a local of effectful fn type or the name of an effectful
    /// fn: what `ignite-check` inferred, the local's type or the fn
    /// declares.
    fn map_fn_effects(&self, f: &Expr) -> Option<FnEffects> {
        let sig = match &f.kind {
            ExprKind::Closure(_) => self.symbols.closure_effects.get(&f.span)?,
            ExprKind::Name(name) if self.lookup(&name.name).is_some() => {
                return match self.lookup(&name.name)?? {
                    Ty::EffectFn(_, _, effects) => Some(effects),
                    _ => None,
                };
            }
            ExprKind::Name(name) if self.lookup(&name.name).is_none() => {
                let q = self.fns.get(&name.name)?;
                if !self.sig_of(q)?.effectful {
                    return None;
                }
                let Some(sig) = self.symbols.effects.get(&name.name) else {
                    return Some(FnEffects {
                        names: Vec::new(),
                        requires: Vec::new(),
                    });
                };
                sig
            }
            _ => return None,
        };
        Some(FnEffects {
            names: sig.params.clone(),
            requires: sig.requires.clone(),
        })
    }

    /// Whether the effects of `map`'s argument `f` fit where `map` is
    /// called (`docs/slice.md` 20.4.3): none on `f`'s own parameter, the
    /// element `map` hands it, and every other one covered by the
    /// `requires` of the fn that calls `map`. E017 at `f` otherwise.
    fn map_effects_fit(&mut self, f: &Expr, effects: &FnEffects) -> bool {
        let what = match &f.kind {
            ExprKind::Name(name) if self.lookup(&name.name).is_some() => {
                format!("effectful fn value `{}`", name.name)
            }
            ExprKind::Name(name) => format!("effectful function `{}`", name.name),
            _ => "effectful closure".to_string(),
        };
        let shown = |entries: &[&String]| {
            entries
                .iter()
                .map(|e| format!("`{e}`"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let (on_param, outer): (Vec<&String>, Vec<&String>) =
            effects.requires.iter().partition(|entry| {
                entry
                    .split_once('.')
                    .is_some_and(|(head, _)| effects.names.iter().any(|n| n == head))
            });
        let mut fit = true;
        if let Some(param) = effects.names.first()
            && !on_param.is_empty()
        {
            self.error(
                "E017",
                f.span,
                format!(
                    "{what} passed to `map` needs {} on its parameter `{param}`, the list's element; `map` passes on only its caller's effects",
                    shown(&on_param)
                ),
            );
            fit = false;
        }
        if let EffectScope::Declared { name, requires } = self.effect_scope.clone() {
            let missing: Vec<&String> = outer
                .into_iter()
                .filter(|entry| !ignite_check::covers(&requires, entry))
                .collect();
            if !missing.is_empty() {
                self.error(
                    "E017",
                    f.span,
                    format!(
                        "{what} passed to `map` needs {}, which the `requires` of `{name}` does not cover",
                        shown(&missing)
                    ),
                );
                fit = false;
            }
        }
        fit
    }

    fn field(
        &mut self,
        expr: &Expr,
        receiver: &Expr,
        name: &Ident,
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        if let Some(head) = self.path_head(receiver) {
            self.record(receiver, Some(Ty::Unit));
            if self.system_members.contains_key(&head.name) {
                return self.member_reference(expr, head, name);
            }
            let what = format!("{}.{}", head.name, name.name);
            if let Some(q) = self.fns.get(&what).cloned()
                && let Some(sig) = self.sig_of(&q)
                && self.generics_of(&q).is_empty()
            {
                self.callees.insert(key(expr), Callee::Fn(q));
                let ty = fn_ty(sig.params, sig.ret);
                if sig.effectful {
                    let shown = Ident {
                        name: what,
                        span: name.span,
                    };
                    return self.effectful_value(&shown, ty, expected);
                }
                return Some(ty);
            }
            if self.is_enum(&head.name) {
                return self.variant(expr, &head.name.clone(), name, None, expected);
            }
            self.unresolved(expr.span, &format!("{}.{}", head.name, name.name));
            return None;
        }
        if let Some(signal) = self.member_signal(expr, receiver, name) {
            return signal;
        }
        let receiver_ty = self.synth(receiver)?;
        self.field_of(&receiver_ty, name)
    }

    fn field_type(&self, receiver: &Ty, name: &str) -> Option<Ty> {
        match receiver {
            Ty::Record(q) => self
                .module_types(&q.module)?
                .records
                .get(&q.name)?
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, ty)| ty.clone()),
            Ty::GenericRecord(q, args) => {
                let params = self.generics_of(q);
                self.module_types(&q.module)?
                    .records
                    .get(&q.name)?
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, ty)| ty.substitute(&params, args))
            }
            Ty::IoError if name == "message" => Some(Ty::Text),
            _ => None,
        }
    }

    /// `receiver.name` for a receiver of type `receiver` (E041 if absent).
    fn field_of(&mut self, receiver: &Ty, name: &Ident) -> Option<Ty> {
        let ty = self.field_type(receiver, &name.name);
        if ty.is_none() {
            self.error(
                "E041",
                name.span,
                format!("`{receiver}` has no field `{}`", name.name),
            );
        }
        ty
    }

    fn try_(&mut self, expr: &Expr, inner: &Expr) -> Option<Ty> {
        if self.in_test_body {
            return self.test_try_(expr, inner);
        }
        self.fail_site(expr, FailureOp::Try, expr.span.end.saturating_sub(1));
        match self.synth(inner)? {
            Ty::Never => Some(Ty::Never),
            Ty::Result(ok, err) => {
                match self.ret.clone() {
                    Some(Ty::Result(_, function_err)) if function_err == err => {}
                    Some(Ty::Result(_, function_err)) => self.error(
                        "E040",
                        expr.span,
                        format!(
                            "type mismatch: `?` propagates `{err}`, the function returns errors of type `{function_err}`"
                        ),
                    ),
                    Some(other) => self.error(
                        "E040",
                        expr.span,
                        format!(
                            "type mismatch: `?` needs a function returning `Result`, this one returns `{other}`"
                        ),
                    ),
                    None => self.error(
                        "E040",
                        expr.span,
                        "type mismatch: `?` outside a function".to_string(),
                    ),
                }
                Some(*ok)
            }
            other => {
                self.error(
                    "E040",
                    inner.span,
                    format!("type mismatch: `?` needs a `Result`, found `{other}`"),
                );
                None
            }
        }
    }

    /// `e?` in a test body: any `Result` or `Option`; `Err` or `None` fails
    /// the test (`docs/slice.md` section 12).
    fn test_try_(&mut self, expr: &Expr, inner: &Expr) -> Option<Ty> {
        match self.synth(inner)? {
            Ty::Never => Some(Ty::Never),
            Ty::Result(ok, err) => {
                let show = self.show(&err);
                let error = Some(*err);
                self.test_tries.insert(key(expr), TestTry { error, show });
                Some(*ok)
            }
            Ty::Option(some) => {
                let entry = TestTry {
                    error: None,
                    show: None,
                };
                self.test_tries.insert(key(expr), entry);
                Some(*some)
            }
            other => {
                self.error(
                    "E040",
                    inner.span,
                    format!(
                        "type mismatch: `?` in a test needs a `Result` or `Option`, found `{other}`"
                    ),
                );
                None
            }
        }
    }

    fn unary(
        &mut self,
        expr: &Expr,
        op: UnaryOp,
        operand: &Expr,
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        match op {
            UnaryOp::Not => {
                self.check(operand, &Ty::Bool);
                Some(Ty::Bool)
            }
            UnaryOp::BitNot => {
                // `docs/slice.md` 32.2: `~` on a `U32` is a `U32`; any
                // other operand is checked against `Int` (29.2).
                let hint = expected.filter(|t| matches!(t, Ty::Int | Ty::U32));
                match self.expr(operand, Mode::Value(hint)) {
                    Some(Ty::U32) => Some(Ty::U32),
                    Some(found) if !compatible(&found, &Ty::Int) => {
                        self.mismatch(operand.span, &Ty::Int, &found);
                        Some(Ty::Int)
                    }
                    _ => Some(Ty::Int),
                }
            }
            UnaryOp::Neg => {
                if let ExprKind::Int(value) = operand.kind
                    && let Some(narrow @ (Ty::Byte | Ty::U32)) = expected
                {
                    if value != 0 {
                        let range = if *narrow == Ty::Byte {
                            "0 to 255"
                        } else {
                            "0 to 4294967295"
                        };
                        self.error(
                            "E044",
                            expr.span,
                            format!(
                                "integer literal -{value} is out of range for `{narrow}` ({range})"
                            ),
                        );
                    }
                    return self.record(operand, Some(narrow.clone()));
                }
                match self.synth(operand)? {
                    ty @ (Ty::Int | Ty::Never) => {
                        self.fail_site(expr, FailureOp::Sub, expr.span.start);
                        Some(ty)
                    }
                    // Modulo 2^32, never a failure (32.1).
                    Ty::U32 => Some(Ty::U32),
                    other => {
                        self.error(
                            "E045",
                            expr.span,
                            format!("operator `-` is not defined for `{other}`"),
                        );
                        None
                    }
                }
            }
        }
    }

    /// A binary operator whose left operand is another: the left spine is
    /// walked without recursion (`docs/slice.md` section 13, row 14), the
    /// innermost operator typed by [`Typer::binary`], each outer one by
    /// [`Typer::binary_rest`] with its left operand's type.
    fn binary_chain(
        &mut self,
        expr: &Expr,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        let hint_for = |op: BinaryOp, expected: Option<Ty>| -> Option<Ty> {
            match op {
                BinaryOp::And | BinaryOp::Or => Some(Ty::Bool),
                op if op.is_bitwise() => {
                    Some(expected.filter(|t| *t == Ty::U32).unwrap_or(Ty::Int))
                }
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                    expected.filter(|t| matches!(t, Ty::Int | Ty::Byte | Ty::U32))
                }
                _ => None,
            }
        };
        let mut spine: Vec<(&Expr, BinaryOp, &Expr, &Expr, Option<Ty>)> =
            vec![(expr, op, left, right, expected.cloned())];
        loop {
            let (_, op, left, _, expected) = spine.last().cloned().expect("non-empty spine");
            let ExprKind::Binary {
                op: inner_op,
                left: inner_left,
                right: inner_right,
            } = &left.kind
            else {
                break;
            };
            let expected = hint_for(op, expected);
            spine.push((left, *inner_op, inner_left, inner_right, expected));
        }
        let (bottom, op, left, right, expected) = spine.pop().expect("non-empty spine");
        let mut ty = self.binary(bottom, op, left, right, expected.as_ref());
        ty = self.record(bottom, ty);
        while let Some((node, op, left, right, expected)) = spine.pop() {
            ty = self.binary_rest(node, op, left, ty, right, expected.as_ref());
            if !spine.is_empty() {
                ty = self.record(node, ty);
            }
        }
        ty
    }

    /// The rest of [`Typer::binary`] once the left operand has been typed:
    /// `left_ty` is its type.
    fn binary_rest(
        &mut self,
        expr: &Expr,
        op: BinaryOp,
        left: &Expr,
        left_ty: Option<Ty>,
        right: &Expr,
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            if let Some(found) = &left_ty
                && !compatible(found, &Ty::Bool)
            {
                self.mismatch(left.span, &Ty::Bool, found);
            }
            self.check(right, &Ty::Bool);
            return Some(Ty::Bool);
        }
        if op.is_bitwise() {
            let word = self.word(left, left_ty);
            let amount = if matches!(op, BinaryOp::Shl | BinaryOp::Shr) {
                Ty::Int
            } else {
                word.clone()
            };
            self.check(right, &amount);
            return Some(word);
        }
        let arithmetic = matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
        );
        let hint = if arithmetic {
            expected.filter(|t| matches!(t, Ty::Int | Ty::Byte | Ty::U32))
        } else {
            None
        };
        let right_ty = match &left_ty {
            Some(first) => self.second_operand(op, first, right, hint),
            None => self.synth(right),
        };
        self.operator(expr, op, left_ty?, right_ty?)
    }

    /// The second operand of `op` once the first has type `first`: checked
    /// against `first`, except that a quantity's partner and an `Int`'s
    /// partner under `*` are typed on their own, so that `d * 2`, `2 * d`
    /// and `1 ms + 1 bytes` reach [`Typer::operator`] (`docs/slice.md`
    /// section 14.1).
    fn second_operand(
        &mut self,
        op: BinaryOp,
        first: &Ty,
        second: &Expr,
        hint: Option<&Ty>,
    ) -> Option<Ty> {
        match first {
            Ty::Never => self.expr(second, Mode::Value(hint)),
            Ty::Quantity(_) => {
                let hint = matches!(op, BinaryOp::Mul | BinaryOp::Div).then_some(&Ty::Int);
                self.expr(second, Mode::Value(hint))
            }
            Ty::Int if op == BinaryOp::Mul => {
                let found = self.expr(second, Mode::Value(Some(&Ty::Int)))?;
                if !matches!(found, Ty::Quantity(_)) && !compatible(&found, &Ty::Int) {
                    self.mismatch(second.span, &Ty::Int, &found);
                }
                Some(found)
            }
            first => self.check(second, first),
        }
    }

    fn binary(
        &mut self,
        expr: &Expr,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            self.check(left, &Ty::Bool);
            self.check(right, &Ty::Bool);
            return Some(Ty::Bool);
        }
        if op.is_bitwise() {
            // `docs/slice.md` 32.2: both sides one word type (`Int` or
            // `U32`) for `& | ^`, the literal side taking the other's; a
            // shift's amount is an `Int` whatever its left side.
            let hint = Some(expected.filter(|t| **t == Ty::U32).unwrap_or(&Ty::Int));
            if matches!(op, BinaryOp::Shl | BinaryOp::Shr) {
                let left_ty = self.expr(left, Mode::Value(hint));
                let word = self.word(left, left_ty);
                self.check(right, &Ty::Int);
                return Some(word);
            }
            if is_literal(left) && !is_literal(right) {
                let right_ty = self.expr(right, Mode::Value(hint));
                let word = self.word(right, right_ty);
                self.check(left, &word);
                return Some(word);
            }
            let left_ty = self.expr(left, Mode::Value(hint));
            let word = self.word(left, left_ty);
            self.check(right, &word);
            return Some(word);
        }
        let arithmetic = matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
        );
        let hint = if arithmetic {
            expected.filter(|t| matches!(t, Ty::Int | Ty::Byte | Ty::U32))
        } else {
            None
        };
        let (left_ty, right_ty) = if (is_literal(left) || is_empty_list(left)) && !is_literal(right)
        {
            let right_ty = self.expr(right, Mode::Value(hint));
            let left_ty = match &right_ty {
                Some(first) => self.second_operand(op, first, left, hint),
                None => self.synth(left),
            };
            (left_ty?, right_ty?)
        } else {
            let left_ty = self.expr(left, Mode::Value(hint));
            let right_ty = match &left_ty {
                Some(first) => self.second_operand(op, first, right, hint),
                None => self.synth(right),
            };
            (left_ty?, right_ty?)
        };
        self.operator(expr, op, left_ty, right_ty)
    }

    /// The word type of a bitwise operand `e` of type `found`: `U32` for a
    /// `U32`, else `Int`, with E040 when `found` is neither (29.2, 32.2).
    fn word(&mut self, e: &Expr, found: Option<Ty>) -> Ty {
        match found {
            Some(Ty::U32) => Ty::U32,
            Some(found) if !compatible(&found, &Ty::Int) => {
                self.mismatch(e.span, &Ty::Int, &found);
                Ty::Int
            }
            _ => Ty::Int,
        }
    }

    /// The type of `left op right` from the operand types: `==` and `!=`
    /// on any comparable type (`docs/slice.md` section 13.5), `+` and the
    /// orderings `<`, `>`, `<=`, `>=` on numbers and `Text` (25.1), `+` on
    /// `List` (concatenation, 25.5), the others on numbers (E045 otherwise).
    fn operator(&mut self, expr: &Expr, op: BinaryOp, left_ty: Ty, right_ty: Ty) -> Option<Ty> {
        let arithmetic = matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
        );
        if matches!(left_ty, Ty::Quantity(_)) || matches!(right_ty, Ty::Quantity(_)) {
            return self.quantity_operator(expr, op, &left_ty, &right_ty);
        }
        let operand = if left_ty == Ty::Never {
            right_ty.clone()
        } else {
            left_ty.clone()
        };
        let result = |operand: Ty| if arithmetic { operand } else { Ty::Bool };
        if !compatible(&left_ty, &right_ty) && !compatible(&right_ty, &left_ty) {
            return Some(result(operand));
        }
        let defined = match op {
            BinaryOp::Eq | BinaryOp::Ne => self.equatable(&operand, &mut Vec::new(), false),
            BinaryOp::Add => matches!(
                operand,
                Ty::Int | Ty::Byte | Ty::U32 | Ty::Text | Ty::List(_) | Ty::Never
            ),
            BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => {
                matches!(operand, Ty::Int | Ty::Byte | Ty::U32 | Ty::Text | Ty::Never)
            }
            _ => matches!(operand, Ty::Int | Ty::Byte | Ty::U32 | Ty::Never),
        };
        // `U32`'s `+ - *` wrap (`docs/slice.md` 32.1); only its `/` and `%`
        // can fail, on a zero divisor.
        let fails = match operand {
            Ty::Int | Ty::Byte => true,
            Ty::U32 => matches!(op, BinaryOp::Div | BinaryOp::Rem),
            _ => false,
        };
        if defined && arithmetic && fails {
            self.fail_site(expr, system::failure_op(op), expr.span.start);
        }
        if !defined {
            let detail = if matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
                ": it holds a function, handle, store, `IoError` or type parameter"
            } else {
                ""
            };
            self.error(
                "E045",
                expr.span,
                format!(
                    "operator `{}` is not defined for `{operand}`{detail}",
                    op.symbol()
                ),
            );
            return (!arithmetic).then_some(Ty::Bool);
        }
        Some(result(operand))
    }

    // ---- match and if -----------------------------------------------------

    fn branch(&mut self, branch: &Branch, mode: Mode) -> Option<Ty> {
        match branch {
            Branch::Block(block) => self.block(block, mode),
            Branch::Expr(expr) => self.branch_expr(expr, mode),
            Branch::Arm(arm, scrutinee) => {
                self.scopes.push(BTreeMap::new());
                self.in_arm = true;
                self.pattern(&arm.pattern, scrutinee.as_ref());
                self.in_arm = false;
                let ty = self.branch_expr(&arm.body, mode);
                self.scopes.pop();
                ty
            }
        }
    }

    fn branch_expr(&mut self, expr: &Expr, mode: Mode) -> Option<Ty> {
        match mode {
            Mode::Value(Some(expected)) => self.check(expr, expected),
            _ => self.expr(expr, mode),
        }
    }

    /// Unify the arms of a `match` or `if`: in value position all arms
    /// have one type (`Never` arms excepted).
    fn branches(&mut self, branches: Vec<Branch>, mode: Mode) -> Option<Ty> {
        match mode {
            Mode::Stmt | Mode::Value(Some(_)) => {
                let mut never = !branches.is_empty();
                for branch in &branches {
                    if self.branch(branch, mode) != Some(Ty::Never) {
                        never = false;
                    }
                }
                Some(match mode {
                    _ if never => Ty::Never,
                    Mode::Value(Some(expected)) => expected.clone(),
                    _ => Ty::Unit,
                })
            }
            Mode::Value(None) => {
                let mut order: Vec<usize> = (0..branches.len()).collect();
                let ambiguous = |n: &str| self.is_ambiguous_variant(n);
                order.sort_by_key(|&i| branches[i].needs_expected(&ambiguous));
                let mut found: Option<Ty> = None;
                let mut failed = false;
                for i in order {
                    let ty = match found.clone() {
                        Some(expected) => self.branch(&branches[i], Mode::Value(Some(&expected))),
                        None => self.branch(&branches[i], Mode::Value(None)),
                    };
                    match ty {
                        Some(Ty::Never) => {}
                        Some(ty) => {
                            found.get_or_insert(ty);
                        }
                        None => failed = true,
                    }
                }
                match found {
                    Some(ty) => Some(ty),
                    None if failed => None,
                    None => Some(Ty::Never),
                }
            }
        }
    }

    /// E046 when the arms of a `match` on `scrutinee` leave a value
    /// uncovered.
    fn exhaustive(&mut self, span: Span, scrutinee: &Ty, arms: &[Arm]) {
        let rows: Vec<Vec<&Pattern>> = arms.iter().map(|arm| vec![&arm.pattern]).collect();
        let consts = &self.pattern_consts;
        let interval = |p: &Pattern| match &p.kind {
            PatternKind::Int(v) => Some((*v, *v)),
            PatternKind::Range { .. } => self.ranges.get(&p.id).copied(),
            _ => {
                let v = self.const_value(consts.get(&p.id)?)?;
                Some((v, v))
            }
        };
        let cx = exhaustive::Context {
            variants: &|t| self.variants(t),
            is_const: &|p| consts.contains_key(&p.id),
            interval: &interval,
        };
        let missing = exhaustive::uncovered(&rows, std::slice::from_ref(scrutinee), &cx);
        if let Some(message) = exhaustive::message(scrutinee, &missing) {
            self.error("E046", span, message);
        }
    }

    // ---- patterns ---------------------------------------------------------

    /// Check a pattern against the scrutinee type and bind its names.
    fn pattern(&mut self, pattern: &Pattern, ty: Option<&Ty>) {
        match &pattern.kind {
            PatternKind::Bool(_) => match ty {
                None | Some(Ty::Bool | Ty::Never) => {}
                Some(other) => self.error(
                    "E040",
                    pattern.span,
                    format!("type mismatch: expected `{other}`, found a `Bool` pattern"),
                ),
            },
            PatternKind::Or(alternatives) => {
                for alternative in alternatives {
                    self.pattern(alternative, ty);
                }
            }
            PatternKind::Tuple(items) if items.is_empty() => match ty {
                None | Some(Ty::Unit | Ty::Never) => {}
                Some(other) => self.error(
                    "E040",
                    pattern.span,
                    format!("type mismatch: expected `{other}`, found `()`"),
                ),
            },
            PatternKind::Tuple(items) => match ty {
                Some(Ty::Tuple(tys)) if tys.len() == items.len() => {
                    for (item, ty) in items.iter().zip(tys) {
                        self.pattern(item, Some(ty));
                    }
                }
                None | Some(Ty::Never) => {
                    for item in items {
                        self.pattern(item, None);
                    }
                }
                Some(other) => {
                    self.error(
                        "E040",
                        pattern.span,
                        format!(
                            "type mismatch: expected `{other}`, found a tuple pattern of {} elements",
                            items.len()
                        ),
                    );
                    for item in items {
                        self.pattern(item, None);
                    }
                }
            },
            PatternKind::List(items) => {
                let element = match ty {
                    Some(Ty::List(element)) => Some((**element).clone()),
                    None | Some(Ty::Never) => None,
                    Some(other) => {
                        self.error(
                            "E040",
                            pattern.span,
                            format!("type mismatch: expected `{other}`, found a list pattern"),
                        );
                        None
                    }
                };
                let whole = element.clone().map(|element| Ty::List(Box::new(element)));
                for item in items {
                    match item.kind {
                        PatternKind::Rest(_) => self.pattern(item, whole.as_ref()),
                        _ => self.pattern(item, element.as_ref()),
                    }
                }
            }
            PatternKind::Rest(Some(name)) => self.bind(name, ty.cloned()),
            PatternKind::Rest(None) => {}
            PatternKind::Variant { path, args }
                if args.is_empty()
                    && path.len() == 1
                    && self.consts.contains_key(&path[0].name) =>
            {
                self.const_pattern(pattern, &path[0], ty);
            }
            PatternKind::Wildcard => {}
            PatternKind::Range { start, end } => self.range_pattern(pattern, start, end, ty),
            PatternKind::Int(value) => match ty {
                Some(narrow @ (Ty::Byte | Ty::U32)) => {
                    let narrow = narrow.clone();
                    self.int_literal(pattern.span, *value, Some(&narrow));
                }
                None | Some(Ty::Int | Ty::Never) => {}
                Some(other) => self.error(
                    "E040",
                    pattern.span,
                    format!("type mismatch: expected `{other}`, found an integer pattern"),
                ),
            },
            PatternKind::Str(_) => match ty {
                None | Some(Ty::Text | Ty::Never) => {}
                Some(other) => self.error(
                    "E040",
                    pattern.span,
                    format!("type mismatch: expected `{other}`, found a text pattern"),
                ),
            },
            PatternKind::Binding(name) if self.in_arm && self.consts.contains_key(&name.name) => {
                self.const_pattern(pattern, name, ty);
            }
            PatternKind::Binding(name) => self.bind(name, ty.cloned()),
            PatternKind::Variant { path, args } => self.variant_pattern(pattern, path, args, ty),
        }
    }

    /// `lo..=hi` on an `Int` or `Byte` scrutinee: each bound an integer
    /// literal or an `Int`/`Byte` constant whose value is known (E045
    /// otherwise), `lo <= hi` (E043 otherwise).
    fn range_pattern(
        &mut self,
        pattern: &Pattern,
        start: &Pattern,
        end: &Pattern,
        ty: Option<&Ty>,
    ) {
        let scalar = match ty {
            None | Some(Ty::Never) => None,
            Some(t @ (Ty::Int | Ty::Byte | Ty::U32)) => Some(t.clone()),
            Some(other) => {
                self.error(
                    "E040",
                    pattern.span,
                    format!("type mismatch: expected `{other}`, found a range pattern; ranges match `Int`, `Byte` and `U32`"),
                );
                return;
            }
        };
        let mut bounds = [0i64; 2];
        for (i, bound) in [start, end].into_iter().enumerate() {
            let value = match &bound.kind {
                PatternKind::Int(v) => {
                    if let Some(t) = &scalar {
                        self.int_literal(bound.span, *v, Some(t));
                    }
                    Some(*v)
                }
                PatternKind::Variant { path, args } if args.is_empty() && path.len() == 1 => {
                    let name = &path[0];
                    let q = self.consts.get(&name.name).cloned();
                    self.const_pattern(bound, name, scalar.as_ref());
                    q.and_then(|q| self.const_value(&q))
                }
                _ => None,
            };
            match value {
                Some(v) => bounds[i] = v,
                None => {
                    self.error(
                        "E045",
                        bound.span,
                        "a range bound must be an integer or byte literal or an `Int` or `Byte` constant with a literal value".to_string(),
                    );
                    return;
                }
            }
        }
        if bounds[0] > bounds[1] {
            self.error(
                "E043",
                pattern.span,
                format!(
                    "empty range `{}..={}`: the start is above the end",
                    bounds[0], bounds[1]
                ),
            );
            return;
        }
        self.ranges.insert(pattern.id, (bounds[0], bounds[1]));
    }

    /// The value of the `Int` or `Byte` constant `q`, when its value is a
    /// literal, a negated literal or another such constant.
    fn const_value(&self, q: &QualName) -> Option<i64> {
        if q.module == self.module {
            self.own.const_values.get(&q.name).copied()
        } else {
            self.modules
                .get(&q.module)?
                .const_values
                .get(&q.name)
                .copied()
        }
    }

    /// Record the value of an own `Int` or `Byte` constant for range
    /// patterns (`ModuleTypes::const_values`).
    fn const_eval(&mut self, name: &str, value: &Expr) {
        let v = match &value.kind {
            ExprKind::Int(v) => Some(*v),
            ExprKind::Unary {
                op: UnaryOp::Neg,
                operand,
            } => match operand.kind {
                ExprKind::Int(v) => Some(v.wrapping_neg()),
                _ => None,
            },
            ExprKind::Name(n) => self
                .consts
                .get(&n.name)
                .cloned()
                .and_then(|q| self.const_value(&q)),
            _ => None,
        };
        if let Some(v) = v {
            self.own.const_values.insert(name.to_string(), v);
        }
    }

    /// A constant used as a pattern: it matches a value equal to the
    /// constant, so its type must be the scrutinee's and have `==`.
    fn const_pattern(&mut self, pattern: &Pattern, name: &Ident, ty: Option<&Ty>) {
        let Some(q) = self.consts.get(&name.name).cloned() else {
            return;
        };
        let Some(const_ty) = self.const_ty(&q) else {
            self.unresolved(name.span, &name.name);
            return;
        };
        self.pattern_consts.insert(pattern.id, q);
        if !matches!(const_ty, Ty::Int | Ty::Byte | Ty::U32 | Ty::Bool | Ty::Text) {
            self.error(
                "E045",
                pattern.span,
                format!(
                    "constant `{}` of type `{const_ty}` cannot be a pattern; a constant pattern needs `==` (`Int`, `Byte`, `Bool` or `Text`)",
                    name.name
                ),
            );
            return;
        }
        if let Some(ty) = ty
            && !compatible(ty, &const_ty)
        {
            self.error(
                "E040",
                pattern.span,
                format!(
                    "type mismatch: expected `{ty}`, found constant `{}` of type `{const_ty}`",
                    name.name
                ),
            );
        }
    }

    fn variant_pattern(
        &mut self,
        pattern: &Pattern,
        path: &[Ident],
        args: &[Pattern],
        ty: Option<&Ty>,
    ) {
        let (owner, variant) = match path {
            [variant] => (None, variant),
            [owner, variant] => (Some(owner.name.as_str()), variant),
            _ => {
                self.unresolved(pattern.span, "pattern");
                return;
            }
        };
        let from_scrutinee = ty
            .and_then(|t| self.selected_variant(t, &variant.name))
            .filter(|(q, _, _)| owner.is_none_or(|o| o == q.name))
            .map(|(_, _, payload)| payload);
        let payload = match from_scrutinee {
            Some(payload) => Some(payload),
            None if owner.is_none()
                && ty.is_some_and(|t| *t != Ty::Never)
                && self.is_out_of_scope_variant(&variant.name) =>
            {
                self.error(
                    "E003",
                    variant.span,
                    format!(
                        "unknown variant `{}`; a variant of an enum that is not imported is only resolved against the scrutinee's type",
                        variant.name
                    ),
                );
                None
            }
            None => {
                let owner = owner.map(str::to_string).or_else(|| {
                    self.symbols
                        .enum_of_variant(&variant.name)
                        .map(str::to_string)
                });
                if let Some(ty) = ty
                    && *ty != Ty::Never
                {
                    let shown = match &owner {
                        Some(owner) => format!("{owner}.{}", variant.name),
                        None => variant.name.clone(),
                    };
                    self.error(
                        "E040",
                        pattern.span,
                        format!("type mismatch: expected `{ty}`, found pattern `{shown}`"),
                    );
                }
                owner.and_then(|o| self.user_payload(&o, &variant.name))
            }
        };
        match payload {
            Some(payload) => {
                if payload.len() != args.len() {
                    self.error(
                        "E043",
                        pattern.span,
                        format!(
                            "wrong number of payload patterns for `{}`: expected {}, found {}",
                            variant.name,
                            payload.len(),
                            args.len()
                        ),
                    );
                }
                for (i, arg) in args.iter().enumerate() {
                    self.pattern(arg, payload.get(i));
                }
            }
            None => {
                for arg in args {
                    self.pattern(arg, None);
                }
            }
        }
    }
}
