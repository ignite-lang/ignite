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
//! binding is its own variable. A closure body is walked as a body of its
//! own, never as part of the body that creates it: its parameters are its
//! variables, its captures are untracked (20.3.1).
//!
//! Two readings the contract leaves open:
//!
//! * A parameter is consumed by a move only when some path moves it: a
//!   fn whose every path loops forever moves nothing, so its parameters
//!   are not consumed by the vacuous "every path that reaches an exit".
//! * The in-place changes that consume a parameter are counted by name,
//!   closure bodies included (`crate::ownership::changed_in_block`, which
//!   both backends read too), an extra copy and never a missed one (20.3.3).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use ignite_syntax::ast::{
    BinaryOp, Block, Decl, Expr, ExprKind, Ident, NodeId, Param, Pattern, PatternKind,
    Place as Target, Program, Span, Stmt, StmtKind,
};

use crate::ids::NodeMap;
use crate::{BraceKind, Builtin, Callee, Ty, TypedModules, TypedProgram};

/// Rounds a loop may take before the walk gives up and assumes every
/// place live (no move at all inside). The module's consumed sets have no
/// such bound: they iterate to the least fixpoint (20.3.3).
const ROUNDS: usize = 64;

/// The result of [`analyse`]: the ids of the expressions that move,
/// the consumed parameters of every fn, associated fn and role of the
/// module, and those of every closure literal inside a body, by the
/// closure's id.
pub(crate) struct Moves {
    pub moves: NodeMap<()>,
    /// The moves that wait for the later arguments of their call
    /// ([`Walk::late_moves`]).
    pub late: NodeMap<()>,
    pub consumed: BTreeMap<String, Vec<bool>>,
    pub closures: NodeMap<Vec<bool>>,
}

/// Analyse every fn, role, test and closure body of `program`, typed as
/// `types` against the imported `modules`.
pub(crate) fn analyse(program: &Program, types: &TypedProgram, modules: &TypedModules) -> Moves {
    let bodies = bodies(program, types);
    let resolved: Vec<Resolved> = bodies.iter().map(|b| resolve(b, types)).collect();
    let changed_params = |body: &Body<'_>| -> Vec<bool> {
        let names = match body.root {
            Root::Block(block) => crate::ownership::changed_in_block(block, Some(types)),
            Root::Expr(e) => crate::ownership::changed_in(e, Some(types)),
        };
        body.params
            .iter()
            .map(|p| names.contains(p.name.as_str()))
            .collect()
    };
    let mut consumed: BTreeMap<String, Vec<bool>> = BTreeMap::new();
    for body in &bodies {
        if let Some(key) = &body.key {
            consumed.insert(key.clone(), changed_params(body));
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
            let own = consumed[key].clone();
            let (moved, _) = walk(body, res, types, modules, &consumed, &own, false);
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
    let mut marks: NodeMap<(bool, bool)> = NodeMap::default();
    let mut closures: NodeMap<Vec<bool>> = NodeMap::default();
    for (body, res) in bodies.iter().zip(&resolved) {
        let own = match (&body.key, body.closure) {
            (Some(key), _) => consumed[key].clone(),
            // A closure's parameter is consumed as a fn's is (20.3.3):
            // changed in place, or moved on every path to the end of the
            // body. A closure calls nothing of its own, so one walk gives
            // the moved set; it does not depend on `own`.
            (None, Some(closure)) => {
                let mut own = changed_params(body);
                let (moved, _) = walk(body, res, types, modules, &consumed, &own, false);
                for i in moved {
                    own[i] = true;
                }
                closures.insert(key(closure), own.clone());
                own
            }
            (None, None) => Vec::new(),
        };
        let (_, found) = walk(body, res, types, modules, &consumed, &own, true);
        marks.extend(found);
    }
    let mut moves = NodeMap::default();
    let mut late = NodeMap::default();
    for (k, (moved, waits)) in marks {
        if moved {
            moves.insert(k, ());
            if waits {
                late.insert(k, ());
            }
        }
    }
    Moves {
        moves,
        late,
        consumed,
        closures,
    }
}

/// What a body consists of: the block of a fn, role or test, or the
/// expression of a closure.
#[derive(Clone, Copy)]
enum Root<'p> {
    Block(&'p Block),
    Expr(&'p Expr),
}

/// One body the analysis walks: a fn, an associated fn, a role, a test or
/// a closure literal inside one of them.
struct Body<'p> {
    /// The key of [`crate::ModuleTypes::consumed`]; `None` for a test and a
    /// closure.
    key: Option<String>,
    params: Vec<&'p Ident>,
    root: Root<'p>,
    /// Whether the body's value is the result (a return type other than
    /// `Unit`).
    used: bool,
    /// The closure literal, for a closure body.
    closure: Option<&'p Expr>,
}

fn bodies<'p>(program: &'p Program, types: &TypedProgram) -> Vec<Body<'p>> {
    let returns = |ret: &Option<ignite_syntax::ast::Type>| match ret {
        None => false,
        Some(ty) => types.resolved_type(ty) != Some(&Ty::Unit),
    };
    let names = |params: &'p [Param]| params.iter().map(|p| &p.name).collect::<Vec<_>>();
    let mut out = Vec::new();
    for decl in &program.decls {
        match decl {
            Decl::Fn(f) => out.push(Body {
                key: Some(f.qualified_name()),
                params: names(&f.params),
                root: Root::Block(&f.body),
                used: returns(&f.ret),
                closure: None,
            }),
            Decl::Protocol(p) => {
                for f in &p.fns {
                    out.push(Body {
                        key: Some(format!("{}.{}", p.name.name, f.name.name)),
                        params: names(&f.params),
                        root: Root::Block(&f.body),
                        used: returns(&f.ret),
                        closure: None,
                    });
                }
            }
            Decl::Test(t) => out.push(Body {
                key: None,
                params: Vec::new(),
                root: Root::Block(&t.body),
                used: false,
                closure: None,
            }),
            Decl::Record(_)
            | Decl::Enum(_)
            | Decl::System(_)
            | Decl::Const(_)
            | Decl::Alias(_)
            | Decl::Assembly(_) => {}
        }
    }
    let mut stack = Vec::new();
    for body in &out {
        match body.root {
            Root::Block(block) => block_children(block, &mut stack),
            Root::Expr(e) => stack.push(Node::Expr(e)),
        }
    }
    let mut closures = Vec::new();
    while let Some(node) = stack.pop() {
        let Node::Expr(e) = node else { continue };
        if let ExprKind::Closure(closure) = &e.kind {
            closures.push(Body {
                key: None,
                params: closure.params.iter().map(|p| &p.name).collect(),
                root: Root::Expr(&closure.body),
                used: types
                    .closure(e)
                    .is_some_and(|info| !matches!(info.ret, Ty::Unit)),
                closure: Some(e),
            });
        }
        expr_children(e, &mut stack);
    }
    out.extend(closures);
    out
}

/// The fn-typed parameters of every fn and associated fn of `program` (key
/// as [`crate::ModuleTypes::consumed`]) that its body only calls
/// (`docs/slice.md` 20.3.7): every use of the name is the callee of a call,
/// none inside a closure or a `spawn`, and nothing assigns it. A backend
/// may then lend the callee a fn that lives only as long as the call.
/// Names are not resolved: a shadowing binding's uses count against the
/// parameter, an extra `Arc` and never a missed one.
pub(crate) fn called_only(program: &Program, types: &TypedProgram) -> BTreeMap<String, Vec<bool>> {
    let mut out = BTreeMap::new();
    for decl in &program.decls {
        let Decl::Fn(f) = decl else { continue };
        let params = f
            .params
            .iter()
            .map(|p| {
                matches!(
                    types.resolved_type(&p.ty),
                    Some(Ty::Fn(..) | Ty::EffectFn(..))
                ) && only_called(&f.body, &p.name.name)
            })
            .collect();
        out.insert(f.qualified_name(), params);
    }
    out
}

/// Whether every use of `name` in `block` is the callee of a call outside
/// any closure or `spawn`, and no assignment targets it.
fn only_called(block: &Block, name: &str) -> bool {
    let mut roots = Vec::new();
    block_children(block, &mut roots);
    let mut stack: Vec<(Node<'_>, bool)> = roots.into_iter().map(|n| (n, false)).collect();
    let mut children = Vec::new();
    while let Some((node, nested)) = stack.pop() {
        let e = match node {
            Node::Target(target) if target.base.name == name => return false,
            Node::Target(_) => continue,
            Node::Expr(e) => e,
        };
        match &e.kind {
            ExprKind::Name(id) if id.name == name => return false,
            ExprKind::Call { callee, args } if matches!(&callee.kind, ExprKind::Name(id) if id.name == name) =>
            {
                if nested {
                    return false;
                }
                stack.extend(args.iter().map(|a| (Node::Expr(a), nested)));
                continue;
            }
            _ => {}
        }
        let inner = nested || matches!(e.kind, ExprKind::Closure(_) | ExprKind::Spawn(_));
        expr_children(e, &mut children);
        stack.extend(children.drain(..).map(|n| (n, inner)));
    }
    true
}

/// The `Bytes` parameters of every fn and associated fn of `program` (key
/// as [`crate::ModuleTypes::consumed`]) that its body only reads as a
/// slice (`docs/slice.md` 20.3.7, [`reads_as_slice`]), with `modules`'
/// sets for imported callees. The greatest fixpoint over the module's fns:
/// every `Bytes` parameter starts lent and loses it when a use is not a
/// slice read, so a recursive call keeps it.
pub(crate) fn lent_bytes(program: &Program, types: &TypedProgram) -> BTreeMap<String, Vec<bool>> {
    let fns: Vec<&ignite_syntax::ast::FnDecl> = program
        .decls
        .iter()
        .filter_map(|d| match d {
            Decl::Fn(f) => Some(f),
            _ => None,
        })
        .collect();
    let mut lent: BTreeMap<String, Vec<bool>> = fns
        .iter()
        .map(|f| {
            let key = f.qualified_name();
            let params = f
                .params
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    types.resolved_type(&p.ty) == Some(&crate::builtins::bytes())
                        && !types.consumed(
                            &Callee::Fn(crate::QualName::new(types.module.clone(), key.clone())),
                            i,
                        )
                })
                .collect();
            (key, params)
        })
        .collect();
    loop {
        let mut changed = false;
        for f in &fns {
            let key = f.qualified_name();
            for (i, p) in f.params.iter().enumerate() {
                if !lent[&key][i] {
                    continue;
                }
                let lends = |callee: &Callee, index: usize| match callee {
                    Callee::Fn(q) if q.module == types.module => {
                        lent.get(&q.name).and_then(|ps| ps.get(index)).copied() == Some(true)
                    }
                    _ => types.lent_bytes(callee, index),
                };
                if !reads_as_slice(&f.body.stmts, &p.name.name, types, &lends) {
                    lent.get_mut(&key).expect("an entry per fn")[i] = false;
                    changed = true;
                }
            }
        }
        if !changed {
            return lent;
        }
    }
}

/// Whether `stmts` read the `Bytes` variable `name` only as a slice: every
/// use is the receiver of `len`, `is_empty`, `at`, `slice`, `to_text` or
/// `index_of`, or
/// an argument by position to a parameter `lends` says is a lent `Bytes`
/// parameter, outside any closure or `spawn`, and nothing assigns it. A
/// backend may then hold the bytes as a borrowed slice. Names are not
/// resolved: a shadowing binding's uses count against `name`.
pub fn reads_as_slice(
    stmts: &[Stmt],
    name: &str,
    types: &TypedProgram,
    lends: &dyn Fn(&Callee, usize) -> bool,
) -> bool {
    let is_name = |e: &Expr| matches!(&e.kind, ExprKind::Name(id) if id.name == name);
    let mut roots = Vec::new();
    stmts_children(stmts, &mut roots);
    let mut stack: Vec<(Node<'_>, bool)> = roots.into_iter().map(|n| (n, false)).collect();
    let mut children = Vec::new();
    while let Some((node, nested)) = stack.pop() {
        let e = match node {
            Node::Target(target) if target.base.name == name => return false,
            Node::Target(_) => continue,
            Node::Expr(e) => e,
        };
        let callee = types.callee(e);
        match &e.kind {
            ExprKind::Name(id) if id.name == name => return false,
            ExprKind::Method { receiver, args, .. } if !nested && is_name(receiver) => {
                let read = match callee {
                    Some(Callee::Builtin(b)) => matches!(
                        b,
                        Builtin::ListLen
                            | Builtin::ListIsEmpty
                            | Builtin::ListAt
                            | Builtin::ListSlice
                            | Builtin::BytesToText
                            | Builtin::BytesIndexOf
                    ),
                    Some(c @ Callee::Fn(_)) => types.named_args(e).is_none() && lends(c, 0),
                    _ => false,
                };
                if !read {
                    return false;
                }
                stack.extend(args.iter().map(|a| (Node::Expr(a), nested)));
                continue;
            }
            ExprKind::Method { receiver, args, .. }
                if !nested
                    && types.named_args(e).is_none()
                    && matches!(callee, Some(Callee::Fn(_))) =>
            {
                let callee = callee.expect("a fn callee");
                stack.push((Node::Expr(receiver), nested));
                for (i, a) in args.iter().enumerate() {
                    if !(is_name(a) && lends(callee, i + 1)) {
                        stack.push((Node::Expr(a), nested));
                    }
                }
                continue;
            }
            ExprKind::Call {
                callee: target,
                args,
            } if !nested
                && types.named_args(e).is_none()
                && matches!(callee, Some(Callee::Fn(_))) =>
            {
                let callee = callee.expect("a fn callee");
                stack.push((Node::Expr(target), nested));
                for (i, a) in args.iter().enumerate() {
                    if !(is_name(a) && lends(callee, i)) {
                        stack.push((Node::Expr(a), nested));
                    }
                }
                continue;
            }
            _ => {}
        }
        let inner = nested || matches!(e.kind, ExprKind::Closure(_) | ExprKind::Spawn(_));
        expr_children(e, &mut children);
        stack.extend(children.drain(..).map(|n| (n, inner)));
    }
    true
}

fn key(expr: &Expr) -> NodeId {
    expr.id
}

/// The value of a named argument, any other operand as it is.
fn unnamed(e: &Expr) -> &Expr {
    match &e.kind {
        ExprKind::NamedArg { value, .. } => value,
        _ => e,
    }
}

/// Whether an operand of a call may move late ([`Walk::late_moves`]): one
/// in a move position that is a name or a field, as a movable place is,
/// with an operand after it. The first test of the late moves, the only
/// one a call without a late move pays inline.
#[inline]
fn may_move_late(operands: &[(&Expr, Operand<'_>)]) -> bool {
    operands.split_last().is_some_and(|(_, before)| {
        before.iter().any(|&(operand, kind)| {
            matches!(kind, Operand::Move)
                && matches!(
                    unnamed(operand).kind,
                    ExprKind::Name(_) | ExprKind::Field { .. }
                )
        })
    })
}

fn target_key(target: &Target) -> NodeId {
    target.id
}

/// Whether a value of `ty` is an aggregate the analysis tracks: not a
/// scalar, a quantity, a handle (invariant 3) or a fn (20.3.1).
fn aggregate(ty: &Ty) -> bool {
    !matches!(
        ty,
        Ty::Int
            | Ty::Byte
            | Ty::U32
            | Ty::Bool
            | Ty::Unit
            | Ty::Quantity(_)
            | Ty::TcpListener
            | Ty::TcpStream
            | Ty::PipeStream
            | Ty::TlsStream
            | Ty::Store(_)
            | Ty::Sender(_)
            | Ty::Receiver(_)
            | Ty::Capability(_)
            | Ty::Fn(..)
            | Ty::EffectFn(..)
            | Ty::Never
    )
}

/// Whether a value of `ty` is a handle (invariant 3): a resource shared by
/// copying 8 bytes. A handle binding moves only at the tail of its own
/// scope ([`Resolved::tails`]).
fn handle(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::TcpListener
            | Ty::TcpStream
            | Ty::PipeStream
            | Ty::TlsStream
            | Ty::Store(_)
            | Ty::Sender(_)
            | Ty::Receiver(_)
            | Ty::Capability(_)
    )
}

// ---- name resolution -------------------------------------------------------

/// A binding of a body: tracked when it is a single-name `let`/`var` or a
/// parameter of an aggregate type, or an owned binding (20.3.1) of an
/// aggregate type; `param` is the parameter's index. `handle` marks a
/// tracked `let`/`var` or owned binding of a handle type.
struct Var {
    tracked: bool,
    param: Option<usize>,
    handle: bool,
}

/// The bindings of one body and what each name use resolves to.
#[derive(Default)]
struct Resolved {
    vars: Vec<Var>,
    /// `Name` expressions that name a binding of the body.
    refs: NodeMap<usize>,
    /// Bindings by the span of their name (the key of
    /// [`TypedProgram::binding_type`] and of a closure capture).
    decls: HashMap<Span, usize>,
    /// Assignment places by id.
    targets: NodeMap<usize>,
    /// `Name` expressions that are the value of the block or `match` arm
    /// that binds the name (directly, or through the branches of an `if`
    /// or the arms of a `match` that is that value): the binding's scope
    /// ends there.
    tails: NodeMap<()>,
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
    let closure = body.closure.and_then(|c| types.closure(c));
    for (i, name) in body.params.iter().enumerate() {
        let ty = types
            .binding_type(name.span)
            .or_else(|| closure.and_then(|c| c.params.get(i)).map(|(_, ty)| ty));
        r.bind(name, ty.is_some_and(aggregate), Some(i), false);
    }
    match body.root {
        Root::Block(block) => r.block(block),
        Root::Expr(e) => r.expr(e),
    }
    r.out
}

impl<'a> Resolver<'a> {
    fn bind(&mut self, name: &'a Ident, tracked: bool, param: Option<usize>, handle: bool) {
        let id = self.out.vars.len();
        self.out.vars.push(Var {
            tracked,
            param,
            handle,
        });
        self.out.decls.insert(name.span, id);
        if let Some(scope) = self.scopes.last_mut() {
            scope.push((name.name.as_str(), id));
        }
    }

    /// Bind a `let`/`var` name or an owned binding: tracked when its type
    /// is an aggregate or a handle.
    fn bind_owned(&mut self, name: &'a Ident) {
        let ty = self.types.binding_type(name.span);
        let handle = ty.is_some_and(handle);
        self.bind(name, ty.is_some_and(aggregate) || handle, None, handle);
    }

    /// Whether the value of `e` may point into a binding or a constant:
    /// `e` is a name with a value, or reaches one through the expressions
    /// [`root`] follows or the receiver of a builtin method (not the type
    /// of a static one, `TcpStream.connect`). Any other
    /// value (a call of a fn, a role or a variant, a literal, an `if` or
    /// `match`, whose results are copies or moves) is a fresh value its
    /// pattern bindings or elements own (20.3.1).
    fn borrowed(&self, e: &Expr) -> bool {
        let builtin = matches!(self.types.callee(e), Some(Callee::Builtin(_)));
        match &e.kind {
            ExprKind::Name(_) => {
                self.out.refs.contains_key(&key(e)) || self.types.try_type_of(e).is_some()
            }
            ExprKind::Field { receiver, .. }
            | ExprKind::TupleField { receiver, .. }
            | ExprKind::Index { receiver, .. }
            | ExprKind::Try(receiver)
            | ExprKind::Await(receiver) => self.borrowed(receiver),
            ExprKind::Method { receiver, name, .. } if builtin => {
                !self.static_receiver(receiver, &name.name) && self.borrowed(receiver)
            }
            ExprKind::Call { callee, .. } if builtin => match &callee.kind {
                ExprKind::Field { receiver, name } => {
                    !self.static_receiver(receiver, &name.name) && self.borrowed(receiver)
                }
                _ => false,
            },
            ExprKind::NamedArg { value, .. } => self.borrowed(value),
            ExprKind::Block(body) => match body.stmts.last().map(|s| &s.kind) {
                Some(StmtKind::Expr(last)) => self.borrowed(last),
                _ => false,
            },
            _ => false,
        }
    }

    /// Whether `receiver.name(..)` is a static builtin (`TcpStream.connect`,
    /// `Int.parse`): the receiver names a type, not a value.
    fn static_receiver(&self, receiver: &Expr, name: &str) -> bool {
        match &receiver.kind {
            ExprKind::Name(owner) => {
                !self.out.refs.contains_key(&key(receiver))
                    && (crate::builtins::static_fn(&owner.name, name).is_some()
                        || (owner.name == "Channel" && name == "bounded")
                        || (owner.name == "List" && name == "with_capacity"))
            }
            _ => false,
        }
    }

    /// Record `e`, the value of a scope whose bindings are numbered from
    /// `start`, as a tail of a binding of that scope.
    fn tail(&mut self, e: &Expr, start: usize) {
        match &e.kind {
            ExprKind::Name(_) => {
                if self.out.refs.get(&key(e)).is_some_and(|&v| v >= start) {
                    self.out.tails.insert(key(e), ());
                }
            }
            ExprKind::Block(block) => {
                if let Some(StmtKind::Expr(last)) = block.stmts.last().map(|s| &s.kind) {
                    self.tail(last, start);
                }
            }
            ExprKind::If {
                then, otherwise, ..
            } => {
                if let Some(StmtKind::Expr(last)) = then.stmts.last().map(|s| &s.kind) {
                    self.tail(last, start);
                }
                if let Some(otherwise) = otherwise {
                    self.tail(otherwise, start);
                }
            }
            ExprKind::Match { arms, .. } => {
                for arm in arms {
                    self.tail(&arm.body, start);
                }
            }
            _ => {}
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
        let start = self.out.vars.len();
        for stmt in &block.stmts {
            self.stmt(stmt);
        }
        if let Some(StmtKind::Expr(last)) = block.stmts.last().map(|s| &s.kind) {
            self.tail(last, start);
        }
        self.scopes.pop();
    }

    /// The body of a `for`: its element is owned when the iterable is.
    fn scoped(
        &mut self,
        pattern: Option<&'a Pattern>,
        name: Option<&'a Ident>,
        owned: bool,
        body: &'a Block,
    ) {
        self.scopes.push(Vec::new());
        if let Some(pattern) = pattern {
            self.pattern(pattern, owned);
        }
        if let Some(name) = name {
            if owned {
                self.bind_owned(name);
            } else {
                self.bind(name, false, None, false);
            }
        }
        self.block(body);
        self.scopes.pop();
    }

    fn stmt(&mut self, stmt: &'a Stmt) {
        match &stmt.kind {
            StmtKind::Let { name, value, .. } => {
                self.expr(value);
                self.bind_owned(name);
            }
            StmtKind::LetPattern { pattern, value, .. } => {
                self.expr(value);
                let owned = !self.borrowed(value);
                self.pattern(pattern, owned);
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
                let owned = !self.borrowed(iter);
                self.scoped(None, Some(name), owned, body);
            }
            StmtKind::ForPattern {
                pattern,
                iter,
                body,
            } => {
                self.expr(iter);
                let owned = !self.borrowed(iter);
                self.scoped(Some(pattern), None, owned, body);
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

    /// Bind the names of `pattern`; `owned` when the matched value is a
    /// fresh value ([`Resolver::borrowed`]), so its bindings own their
    /// parts. Bindings under a list pattern are never owned: a list pattern
    /// takes its elements out of a slice.
    fn pattern(&mut self, pattern: &'a Pattern, owned: bool) {
        match &pattern.kind {
            PatternKind::Binding(name) if owned => self.bind_owned(name),
            PatternKind::Binding(name) | PatternKind::Rest(Some(name)) => {
                self.bind(name, false, None, false)
            }
            PatternKind::Or(items) | PatternKind::Tuple(items) => {
                for item in items {
                    self.pattern(item, owned);
                }
            }
            PatternKind::List(items) => {
                for item in items {
                    self.pattern(item, false);
                }
            }
            PatternKind::Variant { args, .. } => {
                for arg in args {
                    self.pattern(arg, owned);
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
                let owned = !self.borrowed(scrutinee);
                for arm in arms {
                    self.scopes.push(Vec::new());
                    let start = self.out.vars.len();
                    self.pattern(&arm.pattern, owned);
                    self.expr(&arm.body);
                    self.tail(&arm.body, start);
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
pub(crate) enum Node<'a> {
    Expr(&'a Expr),
    Target(&'a Target),
}

/// The direct sub-expressions of `e` (a block's statements' expressions
/// and assignment places included; a closure's body included).
pub(crate) fn expr_children<'a>(e: &'a Expr, out: &mut Vec<Node<'a>>) {
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

pub(crate) fn block_children<'a>(block: &'a Block, out: &mut Vec<Node<'a>>) {
    stmts_children(&block.stmts, out);
}

/// [`block_children`] of a run of statements.
pub(crate) fn stmts_children<'a>(stmts: &'a [Stmt], out: &mut Vec<Node<'a>>) {
    for stmt in stmts {
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
    /// An argument of a `spawn`: taken like [`Operand::Move`] (a handle
    /// binding too, a place rooted at a parameter never), held like
    /// [`Operand::Read`] (20.3.1).
    Spawned,
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
    marks: Option<NodeMap<(bool, bool)>>,
    /// The stack [`Walk::names`] reuses, so the test allocates nothing.
    stack: Vec<Node<'a>>,
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

/// Walk one body whose consumed parameters are `own`; the parameters it
/// moves on every path to an exit (and on some path), and when `record`,
/// every move position's decision.
fn walk<'a>(
    body: &Body<'a>,
    res: &'a Resolved,
    types: &'a TypedProgram,
    modules: &'a TypedModules,
    consumed: &'a BTreeMap<String, Vec<bool>>,
    own: &'a [bool],
    record: bool,
) -> (BTreeSet<usize>, NodeMap<(bool, bool)>) {
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
        marks: record.then(NodeMap::default),
        stack: Vec::new(),
    };
    let entry = match body.root {
        Root::Block(block) => w.block(block, body.used, State::exit()),
        Root::Expr(e) => w.value(e, body.used, State::exit()),
    };
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

    /// Forget every place of the bindings `pattern` makes: seen backward,
    /// the pattern binds them afresh, so a later mention (in the next loop
    /// iteration) is of another value.
    fn unbind(&self, pattern: &Pattern, st: &mut State) {
        // A walk of its own rather than `Pattern::bindings`, which collects
        // the names into a vector: this runs for every arm in every round.
        match &pattern.kind {
            PatternKind::Binding(name) | PatternKind::Rest(Some(name)) => {
                self.unbind_name(name, st)
            }
            PatternKind::Or(items)
            | PatternKind::Tuple(items)
            | PatternKind::List(items)
            | PatternKind::Variant { args: items, .. } => {
                items.iter().for_each(|item| self.unbind(item, st))
            }
            PatternKind::Range { .. }
            | PatternKind::Rest(None)
            | PatternKind::Wildcard
            | PatternKind::Int(_)
            | PatternKind::Str(_)
            | PatternKind::Bool(_) => {}
        }
    }

    fn unbind_name(&self, name: &Ident, st: &mut State) {
        if let Some(&var) = self.res.decls.get(&name.span) {
            st.live.kill(var);
        }
    }

    fn record(&mut self, e: &Expr, moves: bool) {
        if let Some(marks) = &mut self.marks {
            marks.insert(key(e), (moves, false));
        }
    }

    /// [`record`](Self::record) a move that waits for the later arguments
    /// of its call ([`late_moves`](Self::late_moves)).
    fn record_late(&mut self, e: &Expr, moves: bool) {
        if let Some(marks) = &mut self.marks {
            marks.insert(key(e), (moves, true));
        }
    }

    /// A move position: `e` moves when it is a movable place, dead after,
    /// not pinned, not `excluded` by another operand of its call and not
    /// held by an operand of an enclosing call.
    fn take(&mut self, e: &'a Expr, st: State, excluded: bool) -> State {
        self.take_as(e, st, excluded, false)
    }

    /// [`take`](Self::take), or with `spawned` an argument of a `spawn`
    /// (20.3.1): the task takes every argument, so a handle binding moves
    /// at its last use there too, handing its share to the task, and a
    /// place rooted at a parameter is copied, so no `spawn` makes a
    /// parameter consumed.
    fn take_as(&mut self, e: &'a Expr, mut st: State, excluded: bool, spawned: bool) -> State {
        let e = match &e.kind {
            ExprKind::NamedArg { value, .. } => &**value,
            _ => e,
        };
        // A handle moves only as a whole binding at the tail of the scope
        // that binds it: the scope ends there, so the move hands on the
        // one share the binding would have dropped (20.3.1).
        let handle_binding = |w: &Self, place: &Place| {
            place.path.is_empty()
                && w.res.vars[place.var].handle
                && (spawned || w.res.tails.contains_key(&key(e)))
                && w.types.try_type_of(e).is_some_and(handle)
        };
        let Some(place) = self
            .movable(e)
            .filter(|place| self.aggregate_expr(e) || handle_binding(self, place))
        else {
            return self.expr(e, st);
        };
        let param = self.res.vars[place.var].param;
        let would = !excluded
            && !(spawned && param.is_some())
            && !self.pinned(&place)
            && st.live.dead(&place)
            && !self.held_overlaps(&place);
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
                    let mut before = self.value(&arm.body, used, st.clone());
                    if pin.is_some() {
                        self.pins.pop();
                    }
                    self.unbind(&arm.pattern, &mut before);
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
            StmtKind::LetPattern { pattern, value, .. } => {
                self.unbind(pattern, &mut st);
                self.expr(value, st)
            }
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
                    let mut body = w.block(body, false, head.clone());
                    if pin.is_some() {
                        w.pins.pop();
                    }
                    match &stmt.kind {
                        StmtKind::For { name, .. } => w.unbind_name(name, &mut body),
                        StmtKind::ForPattern { pattern, .. } => w.unbind(pattern, &mut body),
                        _ => {}
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
    /// arguments are [`Operand::Spawned`] move positions (20.3.1).
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
            // The task takes every argument (the receiver of a fn call
            // too, its argument 0): each is a move position of its own
            // kind, held like a borrowed one, because LLVM's spawn reads a
            // copied aggregate in place after the later arguments ran.
            let receiver_taken =
                matches!(e.kind, ExprKind::Method { .. }) && matches!(&callee, Some(Callee::Fn(_)));
            for (k, operand) in operands.iter_mut().enumerate() {
                if k > 0 || receiver_taken {
                    operand.1 = Operand::Spawned;
                }
            }
        }
        // Only a fn call binds its later arguments first (the Rust target's
        // `fn_call`, the evaluator's `late_args`); a `spawn` has no
        // `Operand::Move` left to move late.
        // Empty for a call with no late move, the common case: no
        // allocation, every operand read as not late. The walk runs here
        // for every call in every round, so the common path holds one
        // cheap test; the rest is out of line ([`late_call`](Self::late_call)).
        let late = if matches!(callee, Some(Callee::Fn(_))) && may_move_late(&operands) {
            self.late_call(&operands, &mut st)
        } else {
            Vec::new()
        };
        for (i, &(operand, kind)) in operands.iter().enumerate().rev() {
            // While operand `i` runs, the call holds the operands before
            // it that it reads in place (borrowed arguments, builtin
            // receivers, a fn-field receiver) and the places it moves
            // late; a move under operand `i` must not change them
            // (20.3.2). Any other operand in a move position is a value
            // (moved or copied) once evaluated, and an operand after `i`
            // reads what liveness already keeps.
            let depth = self.held.len();
            for (j, (other, other_kind)) in operands[..i].iter().copied().enumerate() {
                let field = match other_kind {
                    Operand::Move if !late.is_empty() && late[j] => None,
                    Operand::Move => continue,
                    Operand::Read | Operand::Spawned => None,
                    Operand::Field(name) => Some(name),
                };
                self.held.push(Held {
                    expr: unnamed(other),
                    field,
                    places: None,
                });
            }
            st = match kind {
                Operand::Read => self.expr(operand, st),
                Operand::Move if !late.is_empty() && late[i] => self.take_late(operand, st),
                Operand::Move | Operand::Spawned => {
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
                    self.take_as(operand, st, excluded, matches!(kind, Operand::Spawned))
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
    /// constructor, and for the element of `push` and `put` and the value of
    /// `send` and `try_send` (`docs/slice.md` 30). `offset` is
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
                    | (Builtin::SenderSend, 0)
                    | (Builtin::SenderTrySend, 0)
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
                Operand::Read | Operand::Move | Operand::Spawned => None,
            };
            self.operand_places(operand, field)
                .iter()
                .any(|q| q.overlaps(place))
        })
    }

    /// The late moves of a fn call that [`may_move_late`] lets through
    /// ([`late_moves`](Self::late_moves)), out of line so the walk of a
    /// call without one stays as small as before unit `call-moves`. A late
    /// move happens at the call, after every later operand ran, so a `?`
    /// in one of them leaves the parameter unmoved: its parameter goes into
    /// the moved sets of `st` here.
    #[cold]
    #[inline(never)]
    fn late_call(&mut self, operands: &[(&'a Expr, Operand<'a>)], st: &mut State) -> Vec<bool> {
        let late = self.late_moves(operands, st);
        for (&(operand, _), _) in operands.iter().zip(&late).filter(|(_, late)| **late) {
            if let Some(i) = self
                .movable(unnamed(operand))
                .and_then(|place| self.res.vars[place.var].param)
            {
                st.moved.insert(i);
                self.any_move.insert(i);
            }
        }
        late
    }

    /// Per operand of a fn call, whether it moves late (unit
    /// `call-moves`): a movable place in a move position that is dead after
    /// the call (`after`, the state after it), not pinned or held by an
    /// enclosing call, mentioned by no operand before it and read by one
    /// after it, where no later operand changes its variable (by name, as
    /// [`crate::ownership::changed_in`] counts) and none passed by
    /// reference [`lends`](Self::lends) from it. The place then moves into
    /// the call once the later operands have run: they read it as it was,
    /// and it is held while they run, so none of them moves it. Empty when
    /// no operand moves late. The walk runs this for every fn call in every
    /// round of every check, so the tests that allocate nothing come first,
    /// the most selective before the rest: an operand after it, a place,
    /// whose variable a later operand names ([`names`](Self::names)), then
    /// dead after the call and not pinned.
    fn late_moves(&mut self, operands: &[(&'a Expr, Operand<'a>)], after: &State) -> Vec<bool> {
        let mut late = Vec::new();
        for i in 0..operands.len().saturating_sub(1) {
            let (operand, kind) = operands[i];
            if !matches!(kind, Operand::Move) {
                continue;
            }
            let inner = unnamed(operand);
            let Some(root) = crate::ownership::place_root(inner) else {
                continue;
            };
            if !operands[i + 1..]
                .iter()
                .any(|&(other, _)| self.names(other, root))
            {
                continue;
            }
            let Some(place) = self.movable(inner) else {
                continue;
            };
            if !after.live.dead(&place)
                || self.pinned(&place)
                || !self.aggregate_expr(inner)
                || self.held_overlaps(&place)
            {
                continue;
            }
            let mentions = |w: &Self, (other, kind): &(&Expr, Operand<'_>)| {
                let field = match kind {
                    Operand::Field(name) => Some(*name),
                    _ => None,
                };
                w.operand_places(other, field)
                    .iter()
                    .any(|q| q.overlaps(&place))
            };
            let later = &operands[i + 1..];
            if operands[..i].iter().any(|o| mentions(self, o))
                || !later.iter().any(|o| mentions(self, o))
            {
                continue;
            }
            if later.iter().all(|&(other, kind)| {
                !crate::ownership::changed_in(other, Some(self.types)).contains(root)
                    && (matches!(kind, Operand::Move) || !self.lends(other, &place))
            }) {
                if late.is_empty() {
                    late = vec![false; operands.len()];
                }
                late[i] = true;
            }
        }
        late
    }

    /// Whether `e` names `root` anywhere, as [`crate::ownership::mentions`]
    /// does, on the reused [`Walk::stack`].
    fn names(&mut self, e: &'a Expr, root: &str) -> bool {
        // Most operands are a name or a literal: no stack for those.
        match &e.kind {
            ExprKind::Name(id) => return id.name == root,
            ExprKind::Int(_) | ExprKind::Str(_) | ExprKind::Bool(_) => return false,
            _ => {}
        }
        let mut stack = std::mem::take(&mut self.stack);
        stack.push(Node::Expr(e));
        let mut found = false;
        while let Some(node) = stack.pop() {
            match node {
                Node::Target(target) => found = target.base.name == root,
                Node::Expr(e) => match &e.kind {
                    ExprKind::Name(id) => found = id.name == root,
                    ExprKind::Int(_) | ExprKind::Str(_) | ExprKind::Bool(_) => {}
                    _ => expr_children(e, &mut stack),
                },
            }
            if found {
                break;
            }
        }
        stack.clear();
        self.stack = stack;
        found
    }

    /// Whether the value of `operand`, passed by reference, may borrow from
    /// `place`: it mentions the place and is not [`fresh`](Self::fresh).
    /// A field or element of the place (an `Int` field too: the Rust target
    /// passes it as `&i64`), a `slice` or `to_bytes` of one, a closure that
    /// reads it, a block or a branch whose value is one, are taken to
    /// borrow.
    fn lends(&self, operand: &Expr, place: &Place) -> bool {
        let operand = unnamed(operand);
        self.operand_places(operand, None)
            .iter()
            .any(|q| q.overlaps(place))
            && !self.fresh(operand)
    }

    /// Whether `e` is a value of its own on both targets, under any `?` or
    /// `await`: the result of a fn or role call, of an operator, or a
    /// scalar result of a builtin.
    fn fresh(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Try(inner) | ExprKind::Await(inner) => self.fresh(inner),
            ExprKind::Binary { .. } | ExprKind::Unary { .. } => true,
            ExprKind::Call { .. } | ExprKind::Method { .. } => match self.types.callee(e) {
                Some(Callee::Fn(_) | Callee::Role { .. }) => true,
                Some(Callee::Builtin(_)) => self.types.try_type_of(e).is_some_and(|ty| {
                    matches!(
                        ty,
                        Ty::Int | Ty::Byte | Ty::U32 | Ty::Bool | Ty::Unit | Ty::Quantity(_)
                    )
                }),
                _ => false,
            },
            _ => false,
        }
    }

    /// A late move ([`late_moves`](Self::late_moves)) of `operand`: its
    /// parameter is already in the moved set (the call moves it), the place
    /// is read here.
    #[cold]
    #[inline(never)]
    fn take_late(&mut self, operand: &'a Expr, mut st: State) -> State {
        let e = unnamed(operand);
        let place = self.movable(e).expect("a late move is a movable place");
        let param = self.res.vars[place.var].param;
        let own = param.is_none_or(|i| self.own.get(i).copied().unwrap_or(false));
        self.record_late(e, own);
        st.live.read(place);
        st
    }
}
