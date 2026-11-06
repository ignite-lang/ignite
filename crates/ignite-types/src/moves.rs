//! Last-use moves (`docs/slice.md` 20.3): which places in move positions
//! take their value without a copy, and which parameters a fn, associated
//! fn or role consumes. Runs once per module after typing and reports no
//! diagnostic; [`TypedProgram::moves`] and [`TypedProgram::consumed`] give
//! the result.
//!
//! The analysis is a backward walk over each body. Its state is the set
//! of places live after the current point and, for consumed parameters,
//! the parameters every path from that point to an exit moves. Loops are
//! iterated to a fixpoint; the consumed sets of a module's fns are the
//! least fixpoint over its bodies, read from the typed modules for
//! imported fns. Names are resolved to bindings first, so a shadowing
//! binding is its own variable. Closure bodies are never entered.
//!
//! Two readings the contract leaves open:
//!
//! * A parameter is consumed by a move only when some path moves it: a
//!   fn whose every path loops forever moves nothing, so its parameters
//!   are not consumed by the vacuous "every path that reaches an exit".
//! * The in-place changes that consume a parameter are counted by name,
//!   closure bodies included, as the backends count them today, so the
//!   set is a superset of both (20.3.3).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use ignite_syntax::ast::{
    BinaryOp, Block, Decl, Expr, ExprKind, Ident, Param, Pattern, PatternKind, Place as Target,
    Program, Span, Stmt, StmtKind,
};

use crate::address::AddressMap;
use crate::{BraceKind, Builtin, Callee, Ty, TypedModules, TypedProgram};

/// Rounds a loop may take before the walk gives up and assumes every
/// place live (no move at all inside). The module's consumed sets have no
/// such bound: they iterate to the least fixpoint (20.3.3).
const ROUNDS: usize = 64;

/// The result of [`analyse`]: the addresses of the expressions that move,
/// and the consumed parameters of every fn, associated fn and role of the
/// module.
pub(crate) struct Moves {
    pub moves: AddressMap<()>,
    pub consumed: BTreeMap<String, Vec<bool>>,
}

/// Analyse every fn, role and test body of `program`, typed as `types`
/// against the imported `modules`.
pub(crate) fn analyse(program: &Program, types: &TypedProgram, modules: &TypedModules) -> Moves {
    let bodies = bodies(program, types);
    let resolved: Vec<Resolved> = bodies.iter().map(|b| resolve(b, types)).collect();
    let mut consumed: BTreeMap<String, Vec<bool>> = BTreeMap::new();
    for body in &bodies {
        if let Some(key) = &body.key {
            let names = changed_names(body.body, types);
            let changed = body
                .params
                .iter()
                .map(|p| names.contains(p.name.name.as_str()))
                .collect();
            consumed.insert(key.clone(), changed);
        }
    }
    // Each round that changes something sets at least one more entry and
    // none is ever cleared, so the fixpoint is reached within one round
    // per parameter of the module, plus the round that sees no change.
    let rounds = consumed.values().map(Vec::len).sum::<usize>() + 1;
    for _ in 0..rounds {
        let mut changed = false;
        for (body, res) in bodies.iter().zip(&resolved) {
            let Some(key) = &body.key else { continue };
            let (moved, _) = walk(body, res, types, modules, &consumed, false);
            let entry = consumed
                .get_mut(key)
                .expect("every keyed body has an entry");
            for i in moved {
                if !entry[i] {
                    entry[i] = true;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut marks: AddressMap<bool> = AddressMap::default();
    for (body, res) in bodies.iter().zip(&resolved) {
        let (_, found) = walk(body, res, types, modules, &consumed, true);
        marks.extend(found);
    }
    Moves {
        moves: marks
            .into_iter()
            .filter(|(_, moves)| *moves)
            .map(|(k, _)| (k, ()))
            .collect(),
        consumed,
    }
}

/// One body the analysis walks: a fn, an associated fn, a role or a test.
struct Body<'p> {
    /// The key of [`crate::ModuleTypes::consumed`]; `None` for a test.
    key: Option<String>,
    params: &'p [Param],
    body: &'p Block,
    /// Whether the body's value is the result (a return type other than
    /// `Unit`).
    used: bool,
}

fn bodies<'p>(program: &'p Program, types: &TypedProgram) -> Vec<Body<'p>> {
    let returns = |ret: &Option<ignite_syntax::ast::Type>| match ret {
        None => false,
        Some(ty) => types.resolved_type(ty) != Some(&Ty::Unit),
    };
    let mut out = Vec::new();
    for decl in &program.decls {
        match decl {
            Decl::Fn(f) => out.push(Body {
                key: Some(f.qualified_name()),
                params: &f.params,
                body: &f.body,
                used: returns(&f.ret),
            }),
            Decl::Protocol(p) => {
                for f in &p.fns {
                    out.push(Body {
                        key: Some(format!("{}.{}", p.name.name, f.name.name)),
                        params: &f.params,
                        body: &f.body,
                        used: returns(&f.ret),
                    });
                }
            }
            Decl::Test(t) => out.push(Body {
                key: None,
                params: &[],
                body: &t.body,
                used: false,
            }),
            Decl::Record(_)
            | Decl::Enum(_)
            | Decl::System(_)
            | Decl::Const(_)
            | Decl::Alias(_)
            | Decl::Assembly(_) => {}
        }
    }
    out
}

fn key(expr: &Expr) -> usize {
    expr as *const Expr as usize
}

fn target_key(target: &Target) -> usize {
    target as *const Target as usize
}

/// Whether a value of `ty` is an aggregate the analysis tracks: not a
/// scalar, a quantity, a handle (invariant 3) or a fn (20.3.1).
fn aggregate(ty: &Ty) -> bool {
    !matches!(
        ty,
        Ty::Int
            | Ty::Byte
            | Ty::Bool
            | Ty::Unit
            | Ty::Quantity(_)
            | Ty::TcpListener
            | Ty::TcpStream
            | Ty::PipeStream
            | Ty::Store(_)
            | Ty::Capability(_)
            | Ty::Fn(..)
            | Ty::EffectFn(..)
            | Ty::Never
    )
}

// ---- name resolution -------------------------------------------------------

/// A binding of a body: tracked when it is a single-name `let`/`var` or
/// a parameter of an aggregate type; `param` is the parameter's index.
struct Var {
    tracked: bool,
    param: Option<usize>,
}

/// The bindings of one body and what each name use resolves to.
#[derive(Default)]
struct Resolved {
    vars: Vec<Var>,
    /// `Name` expressions that name a binding of the body.
    refs: AddressMap<usize>,
    /// Bindings by the span of their name (the key of
    /// [`TypedProgram::binding_type`] and of a closure capture).
    decls: HashMap<Span, usize>,
    /// Assignment places by address.
    targets: AddressMap<usize>,
}

struct Resolver<'a> {
    types: &'a TypedProgram,
    scopes: Vec<Vec<(&'a str, usize)>>,
    out: Resolved,
}

fn resolve(body: &Body<'_>, types: &TypedProgram) -> Resolved {
    let mut r = Resolver {
        types,
        scopes: vec![Vec::new()],
        out: Resolved::default(),
    };
    for (i, param) in body.params.iter().enumerate() {
        let tracked = types.binding_type(param.name.span).is_some_and(aggregate);
        r.bind(&param.name, tracked, Some(i));
    }
    r.block(body.body);
    r.out
}

impl<'a> Resolver<'a> {
    fn bind(&mut self, name: &'a Ident, tracked: bool, param: Option<usize>) {
        let id = self.out.vars.len();
        self.out.vars.push(Var { tracked, param });
        self.out.decls.insert(name.span, id);
        if let Some(scope) = self.scopes.last_mut() {
            scope.push((name.name.as_str(), id));
        }
    }

    fn lookup(&self, name: &str) -> Option<usize> {
        self.scopes.iter().rev().find_map(|scope| {
            scope
                .iter()
                .rev()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| *v)
        })
    }

    fn block(&mut self, block: &'a Block) {
        self.scopes.push(Vec::new());
        for stmt in &block.stmts {
            self.stmt(stmt);
        }
        self.scopes.pop();
    }

    fn scoped(&mut self, pattern: Option<&'a Pattern>, name: Option<&'a Ident>, body: &'a Block) {
        self.scopes.push(Vec::new());
        if let Some(pattern) = pattern {
            self.pattern(pattern);
        }
        if let Some(name) = name {
            self.bind(name, false, None);
        }
        self.block(body);
        self.scopes.pop();
    }

    fn stmt(&mut self, stmt: &'a Stmt) {
        match &stmt.kind {
            StmtKind::Let { name, value, .. } => {
                self.expr(value);
                let tracked = self.types.binding_type(name.span).is_some_and(aggregate);
                self.bind(name, tracked, None);
            }
            StmtKind::LetPattern { pattern, value, .. } => {
                self.expr(value);
                self.pattern(pattern);
            }
            StmtKind::Assign { target, value } => {
                self.expr(value);
                if let Some(var) = self.lookup(&target.base.name) {
                    self.out.targets.insert(target_key(target), var);
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
                self.scoped(None, Some(name), body);
            }
            StmtKind::ForPattern {
                pattern,
                iter,
                body,
            } => {
                self.expr(iter);
                self.scoped(Some(pattern), None, body);
            }
            StmtKind::Break | StmtKind::Continue => {}
            StmtKind::Expr(e) => self.expr(e),
            StmtKind::Assert { cond, message } => {
                self.expr(cond);
                if let Some(message) = message {
                    self.expr(message);
                }
            }
        }
    }

    fn pattern(&mut self, pattern: &'a Pattern) {
        match &pattern.kind {
            PatternKind::Binding(name) => self.bind(name, false, None),
            PatternKind::Rest(Some(name)) => self.bind(name, false, None),
            PatternKind::Or(items) | PatternKind::Tuple(items) | PatternKind::List(items) => {
                for item in items {
                    self.pattern(item);
                }
            }
            PatternKind::Variant { args, .. } => {
                for arg in args {
                    self.pattern(arg);
                }
            }
            PatternKind::Range { .. }
            | PatternKind::Rest(None)
            | PatternKind::Wildcard
            | PatternKind::Int(_)
            | PatternKind::Str(_)
            | PatternKind::Bool(_) => {}
        }
    }

    fn expr(&mut self, e: &'a Expr) {
        match &e.kind {
            ExprKind::Name(id) => {
                if let Some(var) = self.lookup(&id.name) {
                    self.out.refs.insert(key(e), var);
                }
            }
            ExprKind::Match { scrutinee, arms } => {
                self.expr(scrutinee);
                for arm in arms {
                    self.scopes.push(Vec::new());
                    self.pattern(&arm.pattern);
                    self.expr(&arm.body);
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
            ExprKind::Closure(_) => {}
            ExprKind::Binary { .. } => {
                let mut cur = e;
                while let ExprKind::Binary { left, right, .. } = &cur.kind {
                    self.expr(right);
                    cur = &**left;
                }
                self.expr(cur);
            }
            _ => {
                let mut children = Vec::new();
                expr_children(e, &mut children);
                for child in children {
                    if let Node::Expr(child) = child {
                        self.expr(child);
                    }
                }
            }
        }
    }
}

// ---- traversal helpers -----------------------------------------------------

/// A node of the generic traversal: an expression or an assignment place.
enum Node<'a> {
    Expr(&'a Expr),
    Target(&'a Target),
}

/// The direct sub-expressions of `e` (a block's statements' expressions
/// and assignment places included; a closure's body included).
fn expr_children<'a>(e: &'a Expr, out: &mut Vec<Node<'a>>) {
    let mut push = |e: &'a Expr| out.push(Node::Expr(e));
    match &e.kind {
        ExprKind::Int(_)
        | ExprKind::Str(_)
        | ExprKind::Bool(_)
        | ExprKind::Name(_)
        | ExprKind::Quantity { .. }
        | ExprKind::Break
        | ExprKind::Continue => {}
        ExprKind::Record { fields, .. } => fields.iter().for_each(|f| push(&f.value)),
        ExprKind::List(items) | ExprKind::Tuple(items) => items.iter().for_each(push),
        ExprKind::Return(value) => {
            if let Some(value) = value {
                push(value);
            }
        }
        ExprKind::Call { callee, args } => {
            push(callee);
            args.iter().for_each(push);
        }
        ExprKind::Method { receiver, args, .. } => {
            push(receiver);
            args.iter().for_each(push);
        }
        ExprKind::Field { receiver, .. } | ExprKind::TupleField { receiver, .. } => push(receiver),
        ExprKind::Index { receiver, index } => {
            push(receiver);
            push(index);
        }
        ExprKind::Try(inner) | ExprKind::Await(inner) | ExprKind::Spawn(inner) => push(inner),
        ExprKind::Unary { operand, .. } => push(operand),
        ExprKind::Binary { left, right, .. } => {
            push(left);
            push(right);
        }
        ExprKind::Match { scrutinee, arms } => {
            push(scrutinee);
            arms.iter().for_each(|arm| push(&arm.body));
        }
        ExprKind::If {
            cond,
            then,
            otherwise,
        } => {
            push(cond);
            if let Some(otherwise) = otherwise {
                push(otherwise);
            }
            block_children(then, out);
        }
        ExprKind::Block(block) => block_children(block, out),
        ExprKind::Closure(closure) => push(&closure.body),
        ExprKind::RecordUpdate { fields, base, .. } => {
            fields.iter().for_each(|f| push(&f.value));
            push(base);
        }
        ExprKind::NamedArg { value, .. } => push(value),
        ExprKind::Brace { entries } => entries.iter().for_each(|entry| push(&entry.value)),
    }
}

fn block_children<'a>(block: &'a Block, out: &mut Vec<Node<'a>>) {
    for stmt in &block.stmts {
        match &stmt.kind {
            StmtKind::Let { value, .. } | StmtKind::LetPattern { value, .. } => {
                out.push(Node::Expr(value))
            }
            StmtKind::Assign { target, value } => {
                out.push(Node::Target(target));
                out.push(Node::Expr(value));
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    out.push(Node::Expr(value));
                }
            }
            StmtKind::While { cond, body } => {
                out.push(Node::Expr(cond));
                block_children(body, out);
            }
            StmtKind::Loop { body } => block_children(body, out),
            StmtKind::For { iter, body, .. } | StmtKind::ForPattern { iter, body, .. } => {
                out.push(Node::Expr(iter));
                block_children(body, out);
            }
            StmtKind::Break | StmtKind::Continue => {}
            StmtKind::Expr(e) => out.push(Node::Expr(e)),
            StmtKind::Assert { cond, message } => {
                out.push(Node::Expr(cond));
                if let Some(message) = message {
                    out.push(Node::Expr(message));
                }
            }
        }
    }
}

/// Whether a builtin changes its receiver in place: the set of
/// `region::mutates` (LLVM), which holds `mutates_receiver`'s `Map` entries
/// (Rust).
fn mutates(builtin: Builtin) -> bool {
    matches!(
        builtin,
        Builtin::ListPush
            | Builtin::ListSet
            | Builtin::ListAppend
            | Builtin::ListClear
            | Builtin::MapPut
            | Builtin::MapRemove
            | Builtin::StoreNextId
            | Builtin::StorePut
            | Builtin::StoreRemove
            | Builtin::StoreUpdate
            | Builtin::StoreUpsert
    )
}

/// Method names the Rust target counts as in-place changes whatever the
/// callee (`MUTATING_METHODS` in `ignite-rustgen`).
const MUTATING_METHODS: [&str; 4] = ["push", "set", "append", "clear"];

/// The variable a place expression is rooted at, through every expression
/// `region::root` (LLVM) follows: fields, tuple fields, indexes, `?`,
/// `await`, the builtin `at` and a block's last expression. It is a
/// superset of `place_root` (Rust), so the in-place set is a superset of
/// both backends' (20.3.3).
fn root<'a>(e: &'a Expr, types: &TypedProgram) -> Option<&'a str> {
    let at = || matches!(types.callee(e), Some(Callee::Builtin(Builtin::ListAt)));
    match &e.kind {
        ExprKind::Name(id) => Some(&id.name),
        ExprKind::Field { receiver, .. }
        | ExprKind::TupleField { receiver, .. }
        | ExprKind::Index { receiver, .. }
        | ExprKind::Try(receiver)
        | ExprKind::Await(receiver) => root(receiver, types),
        ExprKind::Method { receiver, .. } if at() => root(receiver, types),
        ExprKind::Call { callee, .. } if at() => match &callee.kind {
            ExprKind::Field { receiver, .. } => root(receiver, types),
            _ => None,
        },
        ExprKind::Block(body) => match body.stmts.last().map(|s| &s.kind) {
            Some(StmtKind::Expr(last)) => root(last, types),
            _ => None,
        },
        _ => None,
    }
}

/// The names a body changes in place, by name and closure bodies
/// included: the base of every assignment and the root of every receiver
/// of an in-place builtin (20.3.3).
fn changed_names<'a>(body: &'a Block, types: &TypedProgram) -> BTreeSet<&'a str> {
    let mut names = BTreeSet::new();
    let mut stack = Vec::new();
    block_children(body, &mut stack);
    while let Some(node) = stack.pop() {
        match node {
            Node::Target(target) => {
                names.insert(target.base.name.as_str());
            }
            Node::Expr(e) => {
                let builtin = matches!(types.callee(e), Some(Callee::Builtin(b)) if mutates(*b));
                match &e.kind {
                    ExprKind::Method { receiver, name, .. }
                        if builtin || MUTATING_METHODS.contains(&name.name.as_str()) =>
                    {
                        names.extend(root(receiver, types));
                    }
                    ExprKind::Call { callee, .. } if builtin => {
                        if let ExprKind::Field { receiver, .. } = &callee.kind {
                            names.extend(root(receiver, types));
                        }
                    }
                    _ => {}
                }
                expr_children(e, &mut stack);
            }
        }
    }
    names
}

// ---- places and liveness ---------------------------------------------------

/// A tracked variable or a field path rooted at one.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Place {
    var: usize,
    path: Vec<String>,
}

impl Place {
    fn whole(var: usize) -> Place {
        Place {
            var,
            path: Vec::new(),
        }
    }

    fn prefix_of(&self, other: &Place) -> bool {
        self.var == other.var && other.path.starts_with(&self.path)
    }

    fn overlaps(&self, other: &Place) -> bool {
        self.prefix_of(other) || other.prefix_of(self)
    }
}

/// A later mention: the value of `place` is needed except for the
/// subtrees at `except` (paths rooted at the same variable), which are
/// reassigned as a whole before they are read. An assignment to `q` is
/// the entry `{ q, except: [q] }`: it mentions every strict prefix of `q`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Entry {
    place: Place,
    except: Vec<Vec<String>>,
}

impl Entry {
    fn conflicts(&self, p: &Place) -> bool {
        self.place.overlaps(p) && !self.except.iter().any(|e| p.path.starts_with(e))
    }
}

/// The places mentioned after a point; `all` when the walk gave up.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Live {
    all: bool,
    entries: BTreeSet<Entry>,
}

impl Live {
    fn dead(&self, p: &Place) -> bool {
        !self.all && !self.entries.iter().any(|e| e.conflicts(p))
    }

    fn read(&mut self, p: Place) {
        if self
            .entries
            .iter()
            .any(|e| e.except.is_empty() && e.place.prefix_of(&p))
        {
            return;
        }
        self.entries.retain(|e| !p.prefix_of(&e.place));
        self.entries.insert(Entry {
            place: p,
            except: Vec::new(),
        });
    }

    /// `q = e` seen backward: `q` and every place under it is reassigned
    /// as a whole, every strict prefix of `q` is mentioned.
    fn assign(&mut self, q: &Place) {
        let entries = std::mem::take(&mut self.entries);
        for mut entry in entries {
            if q.prefix_of(&entry.place) {
                continue;
            }
            if entry.place.prefix_of(q) && !entry.except.iter().any(|e| q.path.starts_with(e)) {
                entry.except.retain(|e| !e.starts_with(&q.path));
                entry.except.push(q.path.clone());
                entry.except.sort();
            }
            self.entries.insert(entry);
        }
        if !q.path.is_empty() {
            self.entries.insert(Entry {
                place: q.clone(),
                except: vec![q.path.clone()],
            });
        }
    }

    fn kill(&mut self, var: usize) {
        self.entries.retain(|e| e.place.var != var);
    }
}

/// The walk's state after a point: the live places and the parameters
/// every path from the point to an exit moves.
#[derive(Clone, Debug, PartialEq, Eq)]
struct State {
    live: Live,
    moved: BTreeSet<usize>,
}

impl State {
    fn exit() -> State {
        State {
            live: Live::default(),
            moved: BTreeSet::new(),
        }
    }

    fn join(&self, other: &State) -> State {
        State {
            live: Live {
                all: self.live.all || other.live.all,
                entries: self
                    .live
                    .entries
                    .union(&other.live.entries)
                    .cloned()
                    .collect(),
            },
            moved: self.moved.intersection(&other.moved).copied().collect(),
        }
    }
}

// ---- the backward walk -----------------------------------------------------

/// How a call treats one of its operands.
#[derive(Clone, Copy)]
enum Operand<'a> {
    Read,
    Move,
    /// The receiver of a fn-field call: the field `name` is read.
    Field(&'a str),
}

struct Walk<'a> {
    types: &'a TypedProgram,
    modules: &'a TypedModules,
    consumed: &'a BTreeMap<String, Vec<bool>>,
    res: &'a Resolved,
    /// The body's own consumed parameters.
    own: &'a [bool],
    /// Every parameter: the moved set of a loop head before its first
    /// round.
    top: BTreeSet<usize>,
    /// Places of enclosing `match` scrutinees and `for` iterables.
    pins: Vec<Place>,
    /// The operands of the enclosing calls that are still held while the
    /// operand being walked runs (20.3.2).
    held: Vec<Held<'a>>,
    /// The states after the innermost loops: (break, continue).
    loops: Vec<(State, State)>,
    /// Parameters some path moves, from the settled round of every loop.
    any_move: BTreeSet<usize>,
    marks: Option<AddressMap<bool>>,
}

/// An operand of an enclosing call that the call holds while a later
/// operand runs: a borrowed argument, a builtin receiver, or the receiver
/// of a fn-field call (`field` is the field read). `places` caches what it
/// mentions; it is computed only when a move asks.
struct Held<'a> {
    expr: &'a Expr,
    field: Option<&'a str>,
    places: Option<Vec<Place>>,
}

/// Walk one body; the parameters it moves on every path to an exit (and
/// on some path), and when `record`, every move position's decision.
fn walk<'a>(
    body: &Body<'a>,
    res: &'a Resolved,
    types: &'a TypedProgram,
    modules: &'a TypedModules,
    consumed: &'a BTreeMap<String, Vec<bool>>,
    record: bool,
) -> (BTreeSet<usize>, AddressMap<bool>) {
    let own: &[bool] = body
        .key
        .as_ref()
        .and_then(|k| consumed.get(k))
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut w = Walk {
        types,
        modules,
        consumed,
        res,
        own,
        top: (0..body.params.len()).collect(),
        pins: Vec::new(),
        held: Vec::new(),
        loops: Vec::new(),
        any_move: BTreeSet::new(),
        marks: record.then(AddressMap::default),
    };
    let entry = w.block(body.body, body.used, State::exit());
    let moved = entry
        .moved
        .intersection(&w.any_move)
        .copied()
        .filter(|&i| res.vars.iter().any(|v| v.param == Some(i) && v.tracked))
        .collect();
    (moved, w.marks.unwrap_or_default())
}

impl<'a> Walk<'a> {
    fn consumed_by(&self, callee: &Callee, index: usize) -> bool {
        let (module, name) = match callee {
            Callee::Fn(q) => (&q.module, q.name.clone()),
            Callee::Role { protocol, role } => {
                (&protocol.module, format!("{}.{role}", protocol.name))
            }
            _ => return false,
        };
        let table = if *module == self.types.module {
            Some(self.consumed)
        } else {
            self.modules.get(module).map(|m| &m.consumed)
        };
        table
            .and_then(|t| t.get(&name))
            .and_then(|v| v.get(index))
            .copied()
            .unwrap_or(false)
    }

    fn aggregate_expr(&self, e: &Expr) -> bool {
        self.types.try_type_of(e).is_some_and(aggregate)
    }

    fn var_place(&self, e: &Expr) -> Option<Place> {
        let var = *self.res.refs.get(&key(e))?;
        self.res.vars[var].tracked.then(|| Place::whole(var))
    }

    /// The place `e` denotes when it may move: a tracked variable or a
    /// chain of unboxed record fields rooted at one.
    fn movable(&self, e: &Expr) -> Option<Place> {
        match &e.kind {
            ExprKind::Name(_) => self.var_place(e),
            ExprKind::Field { receiver, name } => {
                let mut place = self.movable(receiver)?;
                let record = match self.types.try_type_of(receiver)? {
                    Ty::Record(q) | Ty::GenericRecord(q, _) => q,
                    _ => return None,
                };
                if self.types.boxed_field(record, &name.name) {
                    return None;
                }
                place.path.push(name.name.clone());
                Some(place)
            }
            _ => None,
        }
    }

    /// The place a `Name` or `Field` chain reads, boxed fields included.
    fn read_place(&self, e: &Expr) -> Option<Place> {
        match &e.kind {
            ExprKind::Name(_) => self.var_place(e),
            ExprKind::Field { receiver, name } => {
                let mut place = self.read_place(receiver)?;
                place.path.push(name.name.clone());
                Some(place)
            }
            _ => None,
        }
    }

    /// The place a `match` scrutinee or `for` iterable points into.
    fn pin_place(&self, e: &Expr) -> Option<Place> {
        match &e.kind {
            ExprKind::Name(_) => self.read_place(e),
            ExprKind::Field { receiver, .. } => {
                self.read_place(e).or_else(|| self.pin_place(receiver))
            }
            ExprKind::Index { receiver, .. }
            | ExprKind::TupleField { receiver, .. }
            | ExprKind::Try(receiver)
            | ExprKind::Await(receiver) => self.pin_place(receiver),
            ExprKind::Method { receiver, .. }
                if matches!(self.types.callee(e), Some(Callee::Builtin(_))) =>
            {
                self.pin_place(receiver)
            }
            _ => None,
        }
    }

    fn pinned(&self, p: &Place) -> bool {
        self.pins.iter().any(|q| q.overlaps(p))
    }

    /// Hold `operand`, read in place, while a later operand runs; the depth
    /// of [`Walk::held`] to truncate back to afterwards.
    fn hold(&mut self, operand: &'a Expr) -> usize {
        let depth = self.held.len();
        self.held.push(Held {
            expr: operand,
            field: None,
            places: None,
        });
        depth
    }

    /// Whether an operand an enclosing call holds mentions a place
    /// overlapping `p` (20.3.2).
    fn held_overlaps(&mut self, p: &Place) -> bool {
        for i in 0..self.held.len() {
            if self.held[i].places.is_none() {
                let places = self.operand_places(self.held[i].expr, self.held[i].field);
                self.held[i].places = Some(places);
            }
            if self.held[i]
                .places
                .as_ref()
                .is_some_and(|places| places.iter().any(|q| q.overlaps(p)))
            {
                return true;
            }
        }
        false
    }

    /// The places an operand mentions; for the receiver of a fn-field call
    /// that is a place, the field it reads.
    fn operand_places(&self, operand: &Expr, field: Option<&str>) -> Vec<Place> {
        let mut found = Vec::new();
        match (field, self.read_place(operand)) {
            (Some(name), Some(mut place)) => {
                place.path.push(name.to_string());
                found.push(place);
            }
            _ => self.mentions(operand, &mut found),
        }
        found
    }

    /// Every place `e` mentions: reads, assignments, closure captures.
    fn mentions(&self, e: &Expr, out: &mut Vec<Place>) {
        let mut stack = vec![Node::Expr(e)];
        while let Some(node) = stack.pop() {
            match node {
                Node::Target(target) => {
                    if let Some(place) = self.target_place(target) {
                        out.push(place);
                    }
                }
                Node::Expr(e) => {
                    if let Some(place) = self.read_place(e) {
                        out.push(place);
                    } else if matches!(e.kind, ExprKind::Closure(_)) {
                        out.extend(self.captures(e));
                    } else {
                        expr_children(e, &mut stack);
                    }
                }
            }
        }
    }

    fn captures(&self, e: &Expr) -> Vec<Place> {
        let Some(info) = self.types.closure(e) else {
            return Vec::new();
        };
        info.captures
            .iter()
            .filter_map(|c| self.res.decls.get(&c.binding))
            .filter(|&&var| self.res.vars[var].tracked)
            .map(|&var| Place::whole(var))
            .collect()
    }

    fn target_place(&self, target: &Target) -> Option<Place> {
        let var = *self.res.targets.get(&target_key(target))?;
        self.res.vars[var].tracked.then(|| Place {
            var,
            path: target.fields.iter().map(|f| f.name.clone()).collect(),
        })
    }

    fn record(&mut self, e: &Expr, moves: bool) {
        if let Some(marks) = &mut self.marks {
            marks.insert(key(e), moves);
        }
    }

    /// A move position: `e` moves when it is a movable place, dead after,
    /// not pinned, not `excluded` by another operand of its call and not
    /// held by an operand of an enclosing call.
    fn take(&mut self, e: &'a Expr, mut st: State, excluded: bool) -> State {
        let e = match &e.kind {
            ExprKind::NamedArg { value, .. } => &**value,
            _ => e,
        };
        let Some(place) = self.movable(e).filter(|_| self.aggregate_expr(e)) else {
            return self.expr(e, st);
        };
        let would = !excluded
            && !self.pinned(&place)
            && st.live.dead(&place)
            && !self.held_overlaps(&place);
        let param = self.res.vars[place.var].param;
        if would && let Some(i) = param {
            st.moved.insert(i);
            self.any_move.insert(i);
        }
        let own = param.is_none_or(|i| self.own.get(i).copied().unwrap_or(false));
        self.record(e, would && own);
        st.live.read(place);
        st
    }

    /// `e` in a value position: a move position when `used`.
    fn value(&mut self, e: &'a Expr, used: bool, st: State) -> State {
        if used {
            self.take(e, st, false)
        } else {
            self.expr_in(e, false, st)
        }
    }

    fn expr(&mut self, e: &'a Expr, st: State) -> State {
        self.expr_in(e, true, st)
    }

    /// The state before `e` from the state `st` after it; `used` says
    /// whether the value of a block, `if` or `match` is used.
    fn expr_in(&mut self, e: &'a Expr, used: bool, mut st: State) -> State {
        match &e.kind {
            ExprKind::Int(_) | ExprKind::Str(_) | ExprKind::Bool(_) | ExprKind::Quantity { .. } => {
                st
            }
            ExprKind::Name(_) => {
                if let Some(place) = self.var_place(e) {
                    st.live.read(place);
                }
                st
            }
            ExprKind::Field { receiver, .. } => match self.read_place(e) {
                Some(place) => {
                    st.live.read(place);
                    st
                }
                None => self.expr(receiver, st),
            },
            ExprKind::TupleField { receiver, .. } => self.expr(receiver, st),
            ExprKind::Index { receiver, index } => {
                // The receiver is read in place after the index ran: a
                // move under the index must not change it (20.3.2).
                let depth = self.hold(receiver);
                let st = self.expr(index, st);
                self.held.truncate(depth);
                self.expr(receiver, st)
            }
            ExprKind::Record { fields, .. } => fields
                .iter()
                .rev()
                .fold(st, |st, f| self.take(&f.value, st, false)),
            ExprKind::Brace { entries } => {
                let record = matches!(self.types.brace_kind(e), Some(BraceKind::Record(_)));
                entries.iter().rev().fold(st, |st, entry| {
                    if record {
                        self.take(&entry.value, st, false)
                    } else {
                        self.expr(&entry.value, st)
                    }
                })
            }
            ExprKind::List(items) | ExprKind::Tuple(items) => items
                .iter()
                .rev()
                .fold(st, |st, item| self.take(item, st, false)),
            ExprKind::RecordUpdate { fields, base, .. } => {
                let st = self.take(base, st, false);
                fields
                    .iter()
                    .rev()
                    .fold(st, |st, f| self.take(&f.value, st, false))
            }
            ExprKind::Return(value) => match value {
                Some(value) => self.take(value, State::exit(), false),
                None => State::exit(),
            },
            ExprKind::Call { .. } | ExprKind::Method { .. } => self.call(e, st, false),
            ExprKind::Spawn(inner) => match &inner.kind {
                ExprKind::Call { .. } | ExprKind::Method { .. } => self.call(inner, st, true),
                _ => self.expr(inner, st),
            },
            ExprKind::Try(inner) => {
                st.moved.clear();
                self.expr(inner, st)
            }
            ExprKind::Await(inner) => self.expr(inner, st),
            ExprKind::Unary { operand, .. } => self.expr(operand, st),
            ExprKind::Binary { .. } => {
                let mut cur = e;
                while let ExprKind::Binary { op, left, right } = &cur.kind {
                    // An aggregate left operand is read in place after the
                    // right one ran (20.3.2); a scalar one, the `Bool` of
                    // `&&` and `||` included, is a value by then.
                    let depth = self.held.len();
                    if self.aggregate_expr(left) {
                        self.hold(left);
                    }
                    if matches!(op, BinaryOp::And | BinaryOp::Or) {
                        let skipped = st.clone();
                        let evaluated = self.expr(right, st);
                        st = skipped.join(&evaluated);
                    } else {
                        st = self.expr(right, st);
                    }
                    self.held.truncate(depth);
                    cur = &**left;
                }
                self.expr(cur, st)
            }
            ExprKind::Match { scrutinee, arms } => {
                let pin = self.pin_place(scrutinee);
                let mut joined: Option<State> = None;
                for arm in arms {
                    if let Some(pin) = &pin {
                        self.pins.push(pin.clone());
                    }
                    let before = self.value(&arm.body, used, st.clone());
                    if pin.is_some() {
                        self.pins.pop();
                    }
                    joined = Some(match joined {
                        None => before,
                        Some(j) => j.join(&before),
                    });
                }
                self.expr(scrutinee, joined.unwrap_or(st))
            }
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => {
                let then = self.block(then, used, st.clone());
                let otherwise = match otherwise {
                    Some(otherwise) => self.value(otherwise, used, st),
                    None => st,
                };
                self.expr(cond, then.join(&otherwise))
            }
            ExprKind::Block(block) => self.block(block, used, st),
            ExprKind::Closure(_) => {
                for place in self.captures(e) {
                    st.live.read(place);
                }
                st
            }
            ExprKind::Break => self.loops.last().map_or(st, |(brk, _)| brk.clone()),
            ExprKind::Continue => self.loops.last().map_or(st, |(_, cont)| cont.clone()),
            ExprKind::NamedArg { value, .. } => self.expr(value, st),
        }
    }

    fn block(&mut self, block: &'a Block, used: bool, mut st: State) -> State {
        let count = block.stmts.len();
        for (i, stmt) in block.stmts.iter().enumerate().rev() {
            st = self.stmt(stmt, used && i + 1 == count, st);
        }
        st
    }

    fn stmt(&mut self, stmt: &'a Stmt, last_used: bool, mut st: State) -> State {
        match &stmt.kind {
            StmtKind::Expr(e) => self.value(e, last_used, st),
            StmtKind::Let { name, value, .. } => {
                if let Some(&var) = self.res.decls.get(&name.span) {
                    st.live.kill(var);
                }
                self.take(value, st, false)
            }
            StmtKind::LetPattern { value, .. } => self.expr(value, st),
            StmtKind::Assign { target, value } => {
                if let Some(place) = self.target_place(target) {
                    st.live.assign(&place);
                }
                self.take(value, st, false)
            }
            StmtKind::Return(value) => match value {
                Some(value) => self.take(value, State::exit(), false),
                None => State::exit(),
            },
            StmtKind::While { cond, body } => {
                let exit = st;
                self.cycle(&exit, &mut |w, head| {
                    let body = w.block(body, false, head.clone());
                    w.expr(cond, exit.join(&body))
                })
            }
            StmtKind::Loop { body } => {
                let exit = st;
                self.cycle(&exit, &mut |w, head| w.block(body, false, head.clone()))
            }
            StmtKind::For { iter, body, .. } | StmtKind::ForPattern { iter, body, .. } => {
                let exit = st;
                let pin = self.pin_place(iter);
                let head = self.cycle(&exit, &mut |w, head| {
                    if let Some(pin) = &pin {
                        w.pins.push(pin.clone());
                    }
                    let body = w.block(body, false, head.clone());
                    if pin.is_some() {
                        w.pins.pop();
                    }
                    exit.join(&body)
                });
                self.expr(iter, head)
            }
            StmtKind::Break => self.loops.last().map_or(st, |(brk, _)| brk.clone()),
            StmtKind::Continue => self.loops.last().map_or(st, |(_, cont)| cont.clone()),
            StmtKind::Assert { cond, message } => {
                if let Some(message) = message {
                    st = self.expr(message, st);
                }
                self.expr(cond, st)
            }
        }
    }

    /// The state at a loop head: `round` maps a head state to the next
    /// one (the continue target is the head, the break target `exit`),
    /// iterated from no live place and every parameter moved until it
    /// settles. A loop that does not settle within [`ROUNDS`] is walked
    /// once more with every place live, so nothing inside it moves. Only
    /// the round walked from the settled head adds to `any_move`: the
    /// earlier rounds start from fewer live places than any execution has.
    fn cycle(
        &mut self,
        exit: &State,
        round: &mut dyn FnMut(&mut Walk<'a>, &State) -> State,
    ) -> State {
        let mut head = State {
            live: Live::default(),
            moved: self.top.clone(),
        };
        let before = self.any_move.clone();
        for _ in 0..ROUNDS {
            self.any_move.clone_from(&before);
            self.loops.push((exit.clone(), head.clone()));
            let next = round(self, &head);
            self.loops.pop();
            if next == head {
                return head;
            }
            head = next;
        }
        self.any_move = before;
        let head = State {
            live: Live {
                all: true,
                entries: BTreeSet::new(),
            },
            moved: BTreeSet::new(),
        };
        self.loops.push((exit.clone(), head.clone()));
        round(self, &head);
        self.loops.pop();
        head
    }

    /// A call or method call; `spawned` for the call of a `spawn`, whose
    /// arguments are no move positions.
    fn call(&mut self, e: &'a Expr, mut st: State, spawned: bool) -> State {
        let callee = self.types.callee(e).cloned();
        let mut operands: Vec<(&'a Expr, Operand<'a>)> = Vec::new();
        match &e.kind {
            ExprKind::Call { callee: head, args } => {
                operands.push((&**head, Operand::Read));
                for (j, arg) in args.iter().enumerate() {
                    operands.push((arg, self.argument(callee.as_ref(), e, j, 0)));
                }
            }
            ExprKind::Method {
                receiver,
                name,
                args,
            } => {
                let ufcs = matches!(&callee, Some(Callee::Fn(q)) if !q.name.contains('.'));
                let first = match &callee {
                    Some(c @ Callee::Fn(_)) if ufcs && self.consumed_by(c, 0) => Operand::Move,
                    Some(Callee::FieldFn { .. }) => Operand::Field(&name.name),
                    _ => Operand::Read,
                };
                operands.push((&**receiver, first));
                for (j, arg) in args.iter().enumerate() {
                    operands.push((arg, self.argument(callee.as_ref(), e, j, usize::from(ufcs))));
                }
            }
            _ => return self.expr(e, st),
        }
        if spawned {
            for operand in &mut operands {
                if matches!(operand.1, Operand::Move) {
                    operand.1 = Operand::Read;
                }
            }
        }
        for (i, &(operand, kind)) in operands.iter().enumerate().rev() {
            // While operand `i` runs, the call holds the operands before
            // it that it reads in place (borrowed arguments, builtin
            // receivers, a fn-field receiver); a move under operand `i`
            // must not change them (20.3.2). An operand in a move
            // position is a value (moved or copied) once evaluated, and
            // an operand after `i` reads what liveness already keeps.
            let depth = self.held.len();
            for (other, other_kind) in operands[..i].iter().copied() {
                let field = match other_kind {
                    Operand::Move => continue,
                    Operand::Read => None,
                    Operand::Field(name) => Some(name),
                };
                self.held.push(Held {
                    expr: other,
                    field,
                    places: None,
                });
            }
            st = match kind {
                Operand::Read => self.expr(operand, st),
                Operand::Move => {
                    let inner = match &operand.kind {
                        ExprKind::NamedArg { value, .. } => &**value,
                        _ => operand,
                    };
                    let excluded = match self.movable(inner) {
                        Some(place) if st.live.dead(&place) => {
                            self.mentioned_by_others(&operands, i, &place)
                        }
                        _ => false,
                    };
                    self.take(operand, st, excluded)
                }
                Operand::Field(name) => match self.read_place(operand) {
                    Some(mut place) => {
                        place.path.push(name.to_string());
                        st.live.read(place);
                        st
                    }
                    None => self.expr(operand, st),
                },
            };
            self.held.truncate(depth);
        }
        st
    }

    /// How argument `j` of `call` is passed: a move position when the
    /// callee consumes its parameter, for a call through a fn value or a
    /// constructor, and for the element of `push` and `put`. `offset` is
    /// 1 for a UFCS call, whose receiver is parameter 0.
    fn argument(
        &self,
        callee: Option<&Callee>,
        call: &Expr,
        j: usize,
        offset: usize,
    ) -> Operand<'a> {
        let moves = match callee {
            Some(c @ (Callee::Fn(_) | Callee::Role { .. })) => {
                let param = match self.types.named_args(call) {
                    Some(perm) => perm.iter().position(|&a| a == j).unwrap_or(j),
                    None => j,
                };
                self.consumed_by(c, param + offset)
            }
            Some(
                Callee::Variant { .. } | Callee::Local | Callee::Const(_) | Callee::FieldFn { .. },
            ) => true,
            Some(Callee::Builtin(b)) => matches!(
                (b, j),
                (Builtin::ListPush, 0)
                    | (Builtin::ListSet, 1)
                    | (Builtin::MapPut, 1)
                    | (Builtin::StorePut, 1)
            ),
            None => false,
        };
        if moves { Operand::Move } else { Operand::Read }
    }

    /// Whether an operand of the call other than `i` mentions a place
    /// overlapping `place` (20.3.2).
    fn mentioned_by_others(
        &self,
        operands: &[(&Expr, Operand<'_>)],
        i: usize,
        place: &Place,
    ) -> bool {
        operands.iter().enumerate().any(|(k, (operand, kind))| {
            if k == i {
                return false;
            }
            let field = match kind {
                Operand::Field(name) => Some(*name),
                Operand::Read | Operand::Move => None,
            };
            self.operand_places(operand, field)
                .iter()
                .any(|q| q.overlaps(place))
        })
    }
}
