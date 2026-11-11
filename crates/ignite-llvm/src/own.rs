//! Handles a block owns (`docs/llvm.md` section 4, `docs/slice.md` 30.3).
//! A handle-table entry counts its owners; a binding the block owns is one
//! of them for every handle in its value, and releases them when the block
//! is left (at its end, by `break`, `continue`, `return` or `?`), as the Rust
//! target drops its `Arc`s at the end of the scope. A handle is a
//! listener, a stream, a pipe end, a channel end or a capability; the value
//! may be the handle itself or a list, tuple, record or enum holding some.
//!
//! The block owns a binding when its `let` takes a value no one else owns
//! ([`produces`]: a builtin that creates handles, a copy out of a store or
//! a channel, or a call of a fn of the module that returns only handles it
//! created, [`returns_fresh`]) and no later statement of the block lets it
//! escape. A `match` whose scrutinee is such a value and whose arms keep
//! what they bind is owned the same way ([`owned_scrutinee`], [`owned_match`]),
//! and so is the value a `for` iterates when its body keeps what the loop
//! binds ([`owned_iter`]). Such a value that no binding holds is a
//! temporary, owned by the expression that consumes it while that
//! expression runs ([`kept_temporaries`], a `match` no statement claims, a
//! bare expression statement, [`fresh`]).
//!
//! A mention keeps the handles inside the block when it is the receiver of
//! a builtin method that does not hand the receiver on, a value `put` in a
//! store or sent on a channel (the heap copy is an owner of its own), an
//! argument of `spawn` (the task registers an owner of its own), or a plain
//! argument of a call whose result cannot hold a handle or holds only fresh
//! ones. A field or element of the binding counts as the binding itself
//! there ([`is_path`]), and one of a type that holds no handle keeps it
//! anywhere. A `let` of the binding, or of a tuple or record literal around
//! it, makes an alias that keeps it while the alias is kept in turn. Any
//! other mention (a result, a field of a value that escapes, an element of
//! a list, a capture, an assignment, a `copy()`) leaves the binding unowned,
//! as before: never released, an extra open descriptor and never an early
//! close. Only mentions that denote the binding count: a later `let` of the
//! name ends its scope, and a `for`, `match` arm or closure that binds the
//! name hides it in its body ([`referred`]).

use std::cell::RefCell;

use ignite_syntax::ast::{
    Block, Decl, Expr, ExprKind, Ident, Pattern, PatternKind, Program, Stmt, StmtKind,
};
use ignite_types::{Builtin, Callee, QualName, Ty, TypedProgram};

/// What the ownership rules read: the typed module, its declarations and
/// its module path, and the fns [`returns_fresh`] is deciding.
pub struct Cx<'a> {
    types: &'a TypedProgram,
    program: &'a Program,
    module: &'a str,
    visiting: RefCell<Vec<String>>,
}

impl<'a> Cx<'a> {
    pub fn new(types: &'a TypedProgram, program: &'a Program, module: &'a str) -> Cx<'a> {
        Cx {
            types,
            program,
            module,
            visiting: RefCell::new(Vec::new()),
        }
    }
}

/// The names that `stmts[index]` binds and the block owns, in binding
/// order; empty when it is no `let` of a value no one else owns.
pub fn owned(stmts: &[Stmt], index: usize, cx: &Cx<'_>) -> Vec<String> {
    let bound: Vec<&Ident> = match &stmts[index].kind {
        StmtKind::Let { name, value, .. } if produces(value, cx) => vec![name],
        StmtKind::LetPattern { pattern, value, .. } if produces(value, cx) => match &pattern.kind {
            PatternKind::Binding(id) => vec![id],
            PatternKind::Tuple(items) => items
                .iter()
                .filter_map(|item| match &item.kind {
                    PatternKind::Binding(id) => Some(id),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    bound
        .into_iter()
        .filter(|id| block_keeps(&stmts[index + 1..], &id.name, cx))
        .map(|id| id.name.clone())
        .collect()
}

/// Whether the `match` expression `e`, a statement of its block, owns its
/// scrutinee: the scrutinee is a value no one else owns ([`produces`]) and
/// every arm keeps every name its pattern binds.
pub fn owned_scrutinee(e: &Expr, cx: &Cx<'_>) -> bool {
    let ExprKind::Match { scrutinee, arms } = &e.kind else {
        return false;
    };
    produces(scrutinee, cx)
        && arms.iter().all(|arm| {
            pattern_names(&arm.pattern)
                .iter()
                .all(|name| keeps(&arm.body, name, cx))
        })
}

/// Whether the expression statement `e` makes a value no one else owns
/// ([`produces`]) and drops it: a bare `senders.remove(id)`.
pub fn fresh(e: &Expr, cx: &Cx<'_>) -> bool {
    produces(e, cx)
}

/// The arguments of `e` that are values no one else owns ([`produces`])
/// and that `e` keeps, so the expression around them is their owner until
/// `e` is done: the arguments of a call of a fn or role whose result cannot
/// alias them ([`result_apart`]; the receiver of a fn called by UFCS
/// included), the value arguments of a builtin that stores a heap copy of
/// them ([`copies_arguments`]), and the arguments of a `spawn` that the
/// task gets an owner of its own for ([`spawn_owns`]).
/// `senders.get(id).is_some()`, `spawn ring(senders.get(id), done)`.
pub fn kept_temporaries<'e>(e: &'e Expr, cx: &Cx<'_>) -> Vec<&'e Expr> {
    let unwrap = |arg: &'e Expr| match &arg.kind {
        ExprKind::NamedArg { value, .. } => &**value,
        _ => arg,
    };
    let (args, fn_call): (Vec<&Expr>, bool) = match &e.kind {
        ExprKind::Spawn(call) => (call_args(call, cx.types), false),
        ExprKind::Call { .. } | ExprKind::Method { .. } => match cx.types.callee(e) {
            Some(Callee::Fn(_) | Callee::Role { .. }) => (call_args(e, cx.types), true),
            Some(Callee::Builtin(b)) if copies_arguments(*b) => match &e.kind {
                ExprKind::Method { args, .. } => (args.iter().collect(), false),
                _ => (Vec::new(), false),
            },
            _ => (Vec::new(), false),
        },
        _ => (Vec::new(), false),
    };
    let spawned = matches!(e.kind, ExprKind::Spawn(_));
    let temps: Vec<&Expr> = args
        .into_iter()
        .map(unwrap)
        .filter(|arg| !spawned || spawn_owns(cx.types.type_of(arg)))
        .filter(|arg| produces(arg, cx))
        .collect();
    // The callee's result is looked at only when there is a temporary.
    if fn_call && !temps.is_empty() && !result_apart(e, cx) {
        return Vec::new();
    }
    temps
}

/// Whether `value`, the value of a `let`, holds only handles no one else
/// owns, so that a `spawn` that is the binding's last mention may move
/// them to its task without an owner of the task's own
/// (`region::moved_to_task`): a value [`produces`] makes, the result of a
/// builtin other than `copy()`, `?` on one, a `match` whose every arm is
/// one, a name its pattern binds from a scrutinee that is one, or leaves,
/// and the result of a fn or role call whose every argument that can hold
/// a handle is one as well ([`moves_apart`]). That call may return its
/// argument (`fn same(t: Sender<Int>) -> Sender<Int> = t`), so an argument
/// another owner holds (a parameter, which the caller owns, a binding with
/// another mention) makes the result not fresh: the task then gets an
/// owner of its own, and the caller's binding or temporary keeps its own.
/// A fn argument whose result holds no handle gives none to the call's
/// result. A fn-value call is never fresh: its captures are unknown.
/// `sole` holds the mentions [`sole_mentions`] found so far, by
/// `lower::expr_key`.
pub fn moves_fresh(value: &Expr, sole: &[usize], cx: &Cx<'_>) -> bool {
    if produces(value, cx) {
        return true;
    }
    match &value.kind {
        ExprKind::Try(inner) => moves_fresh(inner, sole, cx),
        ExprKind::Call { .. } | ExprKind::Method { .. } => match cx.types.callee(value) {
            Some(Callee::Builtin(_)) => {
                !matches!(&value.kind, ExprKind::Method { name, .. } if name.name == "copy")
            }
            Some(Callee::Fn(_) | Callee::Role { .. }) => {
                call_args(value, cx.types).iter().all(|arg| {
                    let arg = match &arg.kind {
                        ExprKind::NamedArg { value, .. } => &**value,
                        _ => arg,
                    };
                    let ty = cx.types.type_of(arg);
                    plain(ty) || yields_plain(ty) || moves_apart(arg, sole, cx)
                })
            }
            _ => false,
        },
        ExprKind::Match { scrutinee, arms } => arms.iter().all(|arm| {
            let bound = pattern_names(&arm.pattern);
            arm_moves_fresh(&arm.body, &bound, scrutinee, sole, cx)
        }),
        _ => false,
    }
}

/// Whether the argument `arg` of a call that may return it holds only
/// handles no one else owns: a mention that takes a binding's only owner
/// (`sole`, [`sole_mentions`]), or a temporary [`moves_fresh`] accepts,
/// which no owner scope registers (the call is none [`kept_temporaries`]
/// names).
fn moves_apart(arg: &Expr, sole: &[usize], cx: &Cx<'_>) -> bool {
    match &arg.kind {
        ExprKind::Name(_) => sole.contains(&crate::lower::expr_key(arg)),
        _ => moves_fresh(arg, sole, cx),
    }
}

/// Whether the value of a `match` arm `body`, whose pattern binds `bound`
/// from `scrutinee`, is one [`moves_fresh`] accepts.
fn arm_moves_fresh(
    body: &Expr,
    bound: &[String],
    scrutinee: &Expr,
    sole: &[usize],
    cx: &Cx<'_>,
) -> bool {
    if matches!(cx.types.type_of(body), Ty::Never) {
        return true;
    }
    match &body.kind {
        ExprKind::Name(id) if bound.contains(&id.name) => moves_fresh(scrutinee, sole, cx),
        ExprKind::Block(block) => match block.stmts.split_last() {
            Some((last, inner)) => match &last.kind {
                StmtKind::Expr(tail) => {
                    let still: Vec<String> = bound
                        .iter()
                        .filter(|name| !inner.iter().any(|stmt| rebinds(stmt, name)))
                        .cloned()
                        .collect();
                    arm_moves_fresh(tail, &still, scrutinee, sole, cx)
                }
                _ => false,
            },
            None => false,
        },
        _ => moves_fresh(body, sole, cx),
    }
}

/// The mentions that take the only owner of a binding of the `let` at
/// `stmts[index]` (`docs/llvm.md` section 4): for each name it binds to a
/// value no one else owns ([`moves_fresh`] given `sole`, or [`produces`]
/// for a pattern) and its block does not own ([`owned`]), the one mention
/// of the name in the rest of its scope, when there is exactly one and it
/// runs at most once per run of the `let` (in no loop and no closure). A
/// call that may return its argument passes that owner on to its result.
pub fn sole_mentions<'s>(
    stmts: &'s [Stmt],
    index: usize,
    sole: &[usize],
    cx: &Cx<'_>,
) -> Vec<&'s Expr> {
    let names = match &stmts[index].kind {
        StmtKind::Let { name, value, .. } if moves_fresh(value, sole, cx) => {
            vec![name.name.clone()]
        }
        StmtKind::LetPattern { pattern, value, .. } if produces(value, cx) => {
            pattern_names(pattern)
        }
        _ => return Vec::new(),
    };
    let owned = owned(stmts, index, cx);
    names
        .iter()
        .filter(|name| !owned.contains(name))
        .filter_map(|name| {
            let mut found = Vec::new();
            block_mentions(&stmts[index + 1..], name, false, &mut found);
            match found.as_slice() {
                [(Some(e), false)] => Some(*e),
                _ => None,
            }
        })
        .collect()
}

/// The `match` that the statement `stmt` evaluates as a temporary and its
/// block owns the scrutinee of ([`owned_scrutinee`]): the statement itself
/// or the value of its `let` (`let present = match senders.get(id) {..}`).
/// The scrutinee's handles are released when the block is left.
pub fn owned_match<'s>(stmt: &'s Stmt, cx: &Cx<'_>) -> Option<&'s Expr> {
    let e = match &stmt.kind {
        StmtKind::Expr(e) => e,
        StmtKind::Let { value, .. } | StmtKind::LetPattern { value, .. } => value,
        _ => return None,
    };
    (matches!(e.kind, ExprKind::Match { .. }) && owned_scrutinee(e, cx)).then_some(e)
}

/// The iterated value of the `for` statement `stmt` when its block owns
/// it: the value is one no one else owns ([`produces`]: `senders.values()`)
/// and the body keeps every name the loop binds. The loop binds its
/// elements in place, so the handles are released when the block is left.
pub fn owned_iter<'s>(stmt: &'s Stmt, cx: &Cx<'_>) -> Option<&'s Expr> {
    let (names, iter, body) = match &stmt.kind {
        StmtKind::For { name, iter, body } => (vec![name.name.clone()], iter, body),
        StmtKind::ForPattern {
            pattern,
            iter,
            body,
        } => (pattern_names(pattern), iter, body),
        _ => return None,
    };
    (produces(iter, cx) && names.iter().all(|name| block_keeps(&body.stmts, name, cx)))
        .then_some(iter)
}

/// Whether `value` makes a value whose handles have no other owner.
fn produces(value: &Expr, cx: &Cx<'_>) -> bool {
    if let ExprKind::Try(inner) = &value.kind {
        return produces(inner, cx);
    }
    match cx.types.callee(value) {
        Some(Callee::Builtin(builtin)) => creates(*builtin),
        Some(Callee::Fn(q)) => returns_fresh(q, cx),
        _ => false,
    }
}

/// Whether the builtin returns handles no one else owns: it creates them,
/// or copies them out of a store or a channel, which adds an owner to each
/// (`copy_from_heap`).
fn creates(builtin: Builtin) -> bool {
    use Builtin::*;
    matches!(
        builtin,
        TcpListenerBind
            | TcpListenerAccept
            | TcpStreamConnect
            | TcpStreamConnectWithin
            | TlsClient
            | PipePair
            | ChannelBounded
            | DirectoryTemporary
            | LauncherNew
            | EntropyNew
            | SettingOf
            | CommandLineOf
            | EnvironmentOf
            | StoreGet
            | StoreRemove
            | StoreValues
            | StoreUpdate
            | StoreUpsert
            | ReceiverReceive
            | ReceiverReceiveWithin
    )
}

/// Whether every handle the fn `q` of this module returns is one only its
/// result holds: no `return` with a value that can hold a handle, and a
/// final expression that is a value no one else owns, a binding of the
/// body that owns such a value and lets it escape only there, or a tuple
/// or record literal of those and of values without handles. A fn of
/// another module, or one being decided further up the call chain, does
/// not.
fn returns_fresh(q: &QualName, cx: &Cx<'_>) -> bool {
    if q.module != cx.module || cx.visiting.borrow().contains(&q.name) {
        return false;
    }
    let Some(f) = cx.program.decls.iter().find_map(|decl| match decl {
        Decl::Fn(f) if f.qualified_name() == q.name => Some(f),
        _ => None,
    }) else {
        return false;
    };
    let stmts = &f.body.stmts;
    let Some((last, before)) = stmts.split_last() else {
        return false;
    };
    let StmtKind::Expr(tail) = &last.kind else {
        return false;
    };
    cx.visiting.borrow_mut().push(q.name.clone());
    let mut taken = Vec::new();
    let fresh = !stmts.iter().any(|stmt| stmt_returns_handles(stmt, cx))
        && fresh_value(tail, before, cx, &mut taken);
    cx.visiting.borrow_mut().pop();
    fresh
}

/// Whether `e`, the final expression of a body after `before`, is a value
/// whose handles only it holds (see [`returns_fresh`]); `taken` collects
/// the bindings it uses, each at most once.
fn fresh_value(e: &Expr, before: &[Stmt], cx: &Cx<'_>, taken: &mut Vec<String>) -> bool {
    if plain(cx.types.type_of(e)) || produces(e, cx) {
        return true;
    }
    match &e.kind {
        ExprKind::Name(id) => {
            if taken.contains(&id.name) {
                return false;
            }
            taken.push(id.name.clone());
            fresh_binding(before, &id.name, cx)
        }
        ExprKind::Record { fields, .. } => fields
            .iter()
            .all(|init| fresh_value(&init.value, before, cx, taken)),
        ExprKind::Tuple(items) => items
            .iter()
            .all(|item| fresh_value(item, before, cx, taken)),
        _ => false,
    }
}

/// Whether the last binding of `name` among `stmts`, a body's statements,
/// is a `let` of a value no one else owns that nothing after it lets
/// escape.
fn fresh_binding(stmts: &[Stmt], name: &str, cx: &Cx<'_>) -> bool {
    let Some(at) = stmts.iter().rposition(|stmt| rebinds(stmt, name)) else {
        return false;
    };
    let made = match &stmts[at].kind {
        StmtKind::Let { value, .. } => produces(value, cx),
        StmtKind::LetPattern { pattern, value, .. } => {
            let bound = match &pattern.kind {
                PatternKind::Binding(id) => id.name == name,
                PatternKind::Tuple(items) => items
                    .iter()
                    .any(|item| matches!(&item.kind, PatternKind::Binding(id) if id.name == name)),
                _ => false,
            };
            bound && produces(value, cx)
        }
        _ => false,
    };
    made && block_keeps(&stmts[at + 1..], name, cx)
}

/// Whether `stmt` holds a `return` whose value can hold a handle, outside
/// closures.
fn stmt_returns_handles(stmt: &Stmt, cx: &Cx<'_>) -> bool {
    let block = |b: &Block| b.stmts.iter().any(|s| stmt_returns_handles(s, cx));
    let expr = |e: &Expr| returns_handles(e, cx);
    match &stmt.kind {
        StmtKind::Return(value) => value.as_ref().is_some_and(|v| !plain(cx.types.type_of(v))),
        StmtKind::Let { value, .. } | StmtKind::LetPattern { value, .. } => expr(value),
        StmtKind::Assign { value, .. } => expr(value),
        StmtKind::While { cond, body } => expr(cond) || block(body),
        StmtKind::Loop { body } => block(body),
        StmtKind::For { iter, body, .. } | StmtKind::ForPattern { iter, body, .. } => {
            expr(iter) || block(body)
        }
        StmtKind::Break | StmtKind::Continue => false,
        StmtKind::Expr(e) => expr(e),
        StmtKind::Assert { cond, message } => cond_or(expr(cond), message.as_ref().map(expr)),
    }
}

fn cond_or(a: bool, b: Option<bool>) -> bool {
    a || b.unwrap_or(false)
}

/// Whether `e` holds a `return` whose value can hold a handle, outside
/// closures.
fn returns_handles(e: &Expr, cx: &Cx<'_>) -> bool {
    let any = |items: &mut dyn Iterator<Item = &Expr>| {
        for item in items {
            if returns_handles(item, cx) {
                return true;
            }
        }
        false
    };
    let block = |b: &Block| b.stmts.iter().any(|s| stmt_returns_handles(s, cx));
    match &e.kind {
        ExprKind::Return(value) => value.as_ref().is_some_and(|v| !plain(cx.types.type_of(v))),
        ExprKind::Name(_)
        | ExprKind::Int(_)
        | ExprKind::Str(_)
        | ExprKind::Bool(_)
        | ExprKind::Quantity { .. }
        | ExprKind::Break
        | ExprKind::Continue
        | ExprKind::Closure(_) => false,
        ExprKind::NamedArg { value, .. } => returns_handles(value, cx),
        ExprKind::Brace { entries } => any(&mut entries.iter().map(|entry| &entry.value)),
        ExprKind::RecordUpdate { fields, base, .. } => {
            any(&mut fields.iter().map(|init| &init.value)) || returns_handles(base, cx)
        }
        ExprKind::Record { fields, .. } => any(&mut fields.iter().map(|init| &init.value)),
        ExprKind::TupleField { receiver, .. } | ExprKind::Field { receiver, .. } => {
            returns_handles(receiver, cx)
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => any(&mut items.iter()),
        ExprKind::Call { callee, args } => returns_handles(callee, cx) || any(&mut args.iter()),
        ExprKind::Method { receiver, args, .. } => {
            returns_handles(receiver, cx) || any(&mut args.iter())
        }
        ExprKind::Index { receiver, index } => {
            returns_handles(receiver, cx) || returns_handles(index, cx)
        }
        ExprKind::Try(inner)
        | ExprKind::Await(inner)
        | ExprKind::Spawn(inner)
        | ExprKind::Unary { operand: inner, .. } => returns_handles(inner, cx),
        ExprKind::Binary { .. } => {
            let mut cur = e;
            while let ExprKind::Binary { left, right, .. } = &cur.kind {
                if returns_handles(right, cx) {
                    return true;
                }
                cur = left;
            }
            returns_handles(cur, cx)
        }
        ExprKind::Match { scrutinee, arms } => {
            returns_handles(scrutinee, cx) || arms.iter().any(|arm| returns_handles(&arm.body, cx))
        }
        ExprKind::If {
            cond,
            then,
            otherwise,
        } => {
            returns_handles(cond, cx)
                || block(then)
                || otherwise.as_ref().is_some_and(|o| returns_handles(o, cx))
        }
        ExprKind::Block(body) => block(body),
    }
}

/// The names `pattern` binds ([`Pattern::bindings`]).
fn pattern_names(pattern: &Pattern) -> Vec<String> {
    pattern
        .bindings()
        .into_iter()
        .map(|id| id.name.clone())
        .collect()
}

/// The calls of the `spawn` statements that hand the binding `name` over to
/// their task: `stmts` are the statements after its `let`, and a spawn
/// hands it over where it is the binding's last use. That is a `spawn`
/// statement that takes `name` as an argument and after which nothing in
/// the binding's scope mentions it, in the binding's own block or nested
/// in blocks, `if` branches and `match` arms that are themselves the
/// binding's last use (the other branches keep the binding, which the
/// block releases when it is left). A later binding of the same name ends
/// the scope: mentions after it are of the new binding. A spawn inside a
/// loop runs again, so a loop body is not searched; a shadowing `let`
/// inside a branch or a match arm that binds the name ends the search too.
/// A binding with an alias before that spawn ([`makes_alias`]) is not
/// handed over.
pub fn handovers<'s>(stmts: &'s [Stmt], name: &str) -> Vec<&'s Expr> {
    let mut out = Vec::new();
    block_handovers(stmts, name, &mut out);
    out
}

fn block_handovers<'s>(stmts: &'s [Stmt], name: &str, out: &mut Vec<&'s Expr>) {
    // The rebinding statement itself still reads the binding in its value.
    let (scope, rebinding) = match stmts.iter().position(|stmt| rebinds(stmt, name)) {
        Some(at) => (&stmts[..at], Some(&stmts[at])),
        None => (stmts, None),
    };
    if rebinding.is_some_and(|stmt| match &stmt.kind {
        StmtKind::Let { value, .. } | StmtKind::LetPattern { value, .. } => refers(value, name),
        _ => false,
    }) {
        return;
    }
    let Some(last) = scope
        .iter()
        .rposition(|stmt| referred(std::slice::from_ref(stmt), name))
    else {
        return;
    };
    // An alias (`let r = rx`, `Waiter { wake = rx }`) still reads the
    // handle after the spawn that is the binding's last mention: the block
    // keeps its owner until it is left.
    if scope[..last].iter().any(|stmt| makes_alias(stmt, name)) {
        return;
    }
    if let StmtKind::Expr(e) = &scope[last].kind {
        tail_handovers(e, name, out);
    }
}

/// Whether `stmts`, a block's statements from some point on, refer to the
/// binding `name` that is in scope before them (see [`block_mentions`]).
/// Unlike `region::mentioned`, which counts every mention of the name.
fn referred(stmts: &[Stmt], name: &str) -> bool {
    let mut found = Vec::new();
    block_mentions(stmts, name, false, &mut found);
    !found.is_empty()
}

/// Whether `e` refers to the binding `name` in scope around it (see
/// [`block_mentions`]).
fn refers(e: &Expr, name: &str) -> bool {
    let mut found = Vec::new();
    expr_mentions(e, name, false, &mut found);
    !found.is_empty()
}

/// A mention of a binding: the name expression, or `None` for the target
/// of an assignment, and whether it may run more than once per run of the
/// statements searched (inside a loop or a closure).
type Mention<'s> = (Option<&'s Expr>, bool);

/// The mentions of the binding `name` in scope before `stmts`, a block's
/// statements from some point on, in `found`: a mention counts up to a
/// statement that binds the name anew (its value included), and not inside
/// a `for`, a `match` arm or a closure whose own binding is `name`.
/// `again` marks them as inside a loop or a closure.
fn block_mentions<'s>(stmts: &'s [Stmt], name: &str, again: bool, found: &mut Vec<Mention<'s>>) {
    for stmt in stmts {
        if rebinds(stmt, name) {
            if let StmtKind::Let { value, .. } | StmtKind::LetPattern { value, .. } = &stmt.kind {
                expr_mentions(value, name, again, found);
            }
            return;
        }
        stmt_mentions(stmt, name, again, found);
    }
}

/// The mentions of `name` in one statement (see [`block_mentions`]); a
/// `let` of `name` mentions it only in its value.
fn stmt_mentions<'s>(stmt: &'s Stmt, name: &str, again: bool, found: &mut Vec<Mention<'s>>) {
    use crate::region::pattern_binds;
    let expr = |e: &'s Expr, again: bool, found: &mut Vec<Mention<'s>>| {
        expr_mentions(e, name, again, found);
    };
    match &stmt.kind {
        StmtKind::Let { value, .. } | StmtKind::LetPattern { value, .. } => {
            expr(value, again, found);
        }
        StmtKind::Assign { target, value } => {
            if target.base.name == name {
                found.push((None, again));
            }
            expr(value, again, found);
        }
        StmtKind::Return(value) => {
            if let Some(v) = value {
                expr(v, again, found);
            }
        }
        StmtKind::While { cond, body } => {
            expr(cond, true, found);
            block_mentions(&body.stmts, name, true, found);
        }
        StmtKind::Loop { body } => block_mentions(&body.stmts, name, true, found),
        StmtKind::For {
            name: bound,
            iter,
            body,
        } => {
            expr(iter, again, found);
            if bound.name != name {
                block_mentions(&body.stmts, name, true, found);
            }
        }
        StmtKind::ForPattern {
            pattern,
            iter,
            body,
        } => {
            expr(iter, again, found);
            if !pattern_binds(pattern, name) {
                block_mentions(&body.stmts, name, true, found);
            }
        }
        StmtKind::Break | StmtKind::Continue => {}
        StmtKind::Expr(e) => expr(e, again, found),
        StmtKind::Assert { cond, message } => {
            expr(cond, again, found);
            if let Some(m) = message {
                expr(m, again, found);
            }
        }
    }
}

/// The mentions of `name` in `e` (see [`block_mentions`]).
fn expr_mentions<'s>(e: &'s Expr, name: &str, again: bool, found: &mut Vec<Mention<'s>>) {
    let all = |items: &mut dyn Iterator<Item = &'s Expr>, found: &mut Vec<Mention<'s>>| {
        for item in items {
            expr_mentions(item, name, again, found);
        }
    };
    match &e.kind {
        ExprKind::Name(id) => {
            if id.name == name {
                found.push((Some(e), again));
            }
        }
        ExprKind::Int(_)
        | ExprKind::Str(_)
        | ExprKind::Bool(_)
        | ExprKind::Quantity { .. }
        | ExprKind::Break
        | ExprKind::Continue => {}
        ExprKind::NamedArg { value, .. } => expr_mentions(value, name, again, found),
        ExprKind::Brace { entries } => all(&mut entries.iter().map(|entry| &entry.value), found),
        ExprKind::Closure(closure) => {
            if closure.params.iter().all(|p| p.name.name != name) {
                expr_mentions(&closure.body, name, true, found);
            }
        }
        ExprKind::RecordUpdate { fields, base, .. } => {
            all(&mut fields.iter().map(|init| &init.value), found);
            expr_mentions(base, name, again, found);
        }
        ExprKind::Record { fields, .. } => all(&mut fields.iter().map(|init| &init.value), found),
        ExprKind::TupleField { receiver, .. } | ExprKind::Field { receiver, .. } => {
            expr_mentions(receiver, name, again, found);
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => all(&mut items.iter(), found),
        ExprKind::Return(value) => {
            if let Some(v) = value {
                expr_mentions(v, name, again, found);
            }
        }
        ExprKind::Call { callee, args } => {
            expr_mentions(callee, name, again, found);
            all(&mut args.iter(), found);
        }
        ExprKind::Method { receiver, args, .. } => {
            expr_mentions(receiver, name, again, found);
            all(&mut args.iter(), found);
        }
        ExprKind::Index { receiver, index } => {
            expr_mentions(receiver, name, again, found);
            expr_mentions(index, name, again, found);
        }
        ExprKind::Try(inner)
        | ExprKind::Await(inner)
        | ExprKind::Spawn(inner)
        | ExprKind::Unary { operand: inner, .. } => expr_mentions(inner, name, again, found),
        ExprKind::Binary { .. } => {
            let mut cur = e;
            while let ExprKind::Binary { left, right, .. } = &cur.kind {
                expr_mentions(right, name, again, found);
                cur = left;
            }
            expr_mentions(cur, name, again, found);
        }
        ExprKind::Match { scrutinee, arms } => {
            expr_mentions(scrutinee, name, again, found);
            for arm in arms {
                if !crate::region::pattern_binds(&arm.pattern, name) {
                    expr_mentions(&arm.body, name, again, found);
                }
            }
        }
        ExprKind::If {
            cond,
            then,
            otherwise,
        } => {
            expr_mentions(cond, name, again, found);
            block_mentions(&then.stmts, name, again, found);
            if let Some(o) = otherwise {
                expr_mentions(o, name, again, found);
            }
        }
        ExprKind::Block(body) => block_mentions(&body.stmts, name, again, found),
    }
}

fn tail_handovers<'s>(e: &'s Expr, name: &str, out: &mut Vec<&'s Expr>) {
    let mentions = refers;
    match &e.kind {
        ExprKind::Spawn(call) if spawn_args(call).iter().any(|arg| is_name(arg, name)) => {
            out.push(call);
        }
        ExprKind::Block(body) => block_handovers(&body.stmts, name, out),
        ExprKind::If {
            cond,
            then,
            otherwise,
        } if !mentions(cond, name) => {
            block_handovers(&then.stmts, name, out);
            if let Some(otherwise) = otherwise {
                tail_handovers(otherwise, name, out);
            }
        }
        ExprKind::Match { scrutinee, arms } if !mentions(scrutinee, name) => {
            for arm in arms {
                if !crate::region::pattern_binds(&arm.pattern, name) {
                    tail_handovers(&arm.body, name, out);
                }
            }
        }
        _ => {}
    }
}

/// Whether `stmt` is a `let` whose value may alias the binding `name`: the
/// binding, a field or element of it, or a tuple, record, list or enum
/// literal holding one of those. Wider than [`alias_let`] on purpose: a
/// handover it skips leaves the release at the block's end.
fn makes_alias(stmt: &Stmt, name: &str) -> bool {
    match &stmt.kind {
        StmtKind::Let { value, .. } | StmtKind::LetPattern { value, .. } => holds_path(value, name),
        _ => false,
    }
}

fn holds_path(e: &Expr, name: &str) -> bool {
    match &e.kind {
        _ if is_path(e, name) => true,
        ExprKind::Record { fields, .. } | ExprKind::RecordUpdate { fields, .. } => {
            fields.iter().any(|init| holds_path(&init.value, name))
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => {
            items.iter().any(|item| holds_path(item, name))
        }
        ExprKind::Call { args, .. } => args.iter().any(|arg| holds_path(arg, name)),
        ExprKind::NamedArg { value, .. } => holds_path(value, name),
        _ => false,
    }
}

/// Whether `stmt` binds `name` anew in its block.
fn rebinds(stmt: &Stmt, name: &str) -> bool {
    match &stmt.kind {
        StmtKind::Let { name: bound, .. } => bound.name == name,
        StmtKind::LetPattern { pattern, .. } => crate::region::pattern_binds(pattern, name),
        _ => false,
    }
}

/// The arguments of a spawned call, the receiver of a method call first,
/// as `Lower::spawn` reads them.
pub fn spawn_args(call: &Expr) -> Vec<&Expr> {
    match &call.kind {
        ExprKind::Call { args, .. } => args.iter().collect(),
        ExprKind::Method { receiver, args, .. } => {
            std::iter::once(&**receiver).chain(args).collect()
        }
        _ => Vec::new(),
    }
}

/// Whether a value of type `ty` holds no handle for certain. Records,
/// enums, fn values, stores and type parameters may; the answer for them
/// is no without looking inside.
fn plain(ty: &Ty) -> bool {
    match ty {
        Ty::Int
        | Ty::Bool
        | Ty::Byte
        | Ty::U32
        | Ty::Unit
        | Ty::Text
        | Ty::IoError
        | Ty::Input
        | Ty::Never
        | Ty::Quantity(_)
        | Ty::Ref(_)
        | Ty::Signal(_)
        | Ty::SystemRef
        | Ty::Protocol => true,
        Ty::List(inner) | Ty::Option(inner) => plain(inner),
        Ty::Result(ok, err) | Ty::Map(ok, err) => plain(ok) && plain(err),
        Ty::Tuple(items) => items.iter().all(plain),
        _ => false,
    }
}

/// Whether a value of type `ty` is a fn whose result holds no handle: a
/// callee given it can get no handle out of it.
fn yields_plain(ty: &Ty) -> bool {
    matches!(ty, Ty::Fn(_, ret) | Ty::EffectFn(_, ret, _) if plain(ret))
}

/// Whether a `spawn` argument of type `ty` gets an owner of its own in the
/// task: a stream, a pipe end or a channel end (`Lower::spawn_with`), or a
/// tuple, record or enum, whose handles the spawn retains. A listener, a
/// capability and a type parameter do not; a value without handles needs
/// none.
fn spawn_owns(ty: &Ty) -> bool {
    match ty {
        Ty::TcpStream | Ty::PipeStream | Ty::TlsStream | Ty::Sender(_) | Ty::Receiver(_) => true,
        Ty::TcpListener | Ty::Capability(_) | Ty::Param(_) => false,
        Ty::List(_)
        | Ty::Option(_)
        | Ty::Result(..)
        | Ty::Map(..)
        | Ty::Tuple(_)
        | Ty::Record(_)
        | Ty::GenericRecord(..)
        | Ty::Enum(_)
        | Ty::GenericEnum(..) => true,
        other => plain(other),
    }
}

/// Whether a builtin method keeps its handle receiver inside the block: it
/// neither returns nor stores it.
fn keeps_receiver(builtin: Builtin) -> bool {
    use Builtin::*;
    matches!(
        builtin,
        TcpStreamRead
            | TcpStreamWriteAll
            | TcpStreamClose
            | TcpStreamSetIdleTimeout
            | PipeStreamRead
            | PipeStreamWriteAll
            | PipeStreamClose
            | PipeStreamSetIdleTimeout
            | PipeStreamReceivedBatches
            | StreamRead
            | StreamWriteAll
            | StreamClose
            | StreamSetIdleTimeout
            | TcpStreamWriteUrgent
            | TcpStreamUrgentPending
            | PipeStreamWriteUrgent
            | PipeStreamUrgentPending
            | StreamWriteUrgent
            | StreamUrgentPending
            | TcpStreamLocal
            | TcpStreamPeer
            | TcpStreamTimedOut
            | TlsStreamAlpn
            | TcpListenerAccept
            | TcpListenerLocalPort
            | SenderSend
            | SenderTrySend
            | SenderClose
            | ReceiverReceive
            | ReceiverReceiveWithin
            | ReceiverClosed
            | ReceiverClose
            | DirectoryReadFile
            | DirectoryWriteFile
            | DirectoryReadDirectory
            | DirectoryCreateDirectory
            | DirectoryRemove
            | DirectoryRemoveTree
            | DirectoryAbsolutePath
            | LauncherStart
            | LauncherExecute
            | EntropyRandomBytes
            | SettingValue
            | CommandLineArguments
            | EnvironmentGet
            | Debug
    )
}

/// Whether a builtin stores a heap copy of its value arguments, which is
/// an owner of their handles of its own (`copy_to_heap`): `put` in a store,
/// `send` and `try_send` on a channel.
fn copies_arguments(builtin: Builtin) -> bool {
    matches!(
        builtin,
        Builtin::StorePut | Builtin::SenderSend | Builtin::SenderTrySend
    )
}

fn is_name(e: &Expr, name: &str) -> bool {
    match &e.kind {
        ExprKind::Name(id) => id.name == name,
        ExprKind::NamedArg { value, .. } => is_name(value, name),
        _ => false,
    }
}

/// Whether `e` is the binding `name` or a field or element of it:
/// `w.wake`, `ends.0`.
fn is_path(e: &Expr, name: &str) -> bool {
    match &e.kind {
        ExprKind::Field { receiver, .. } | ExprKind::TupleField { receiver, .. } => {
            is_path(receiver, name)
        }
        _ => is_name(e, name),
    }
}

/// The names a `let` statement binds as aliases of the binding `name`: its
/// value is the binding, a field or element of it, or a tuple or record
/// literal around those whose other fields keep it.
fn alias_let(stmt: &Stmt, name: &str, cx: &Cx<'_>) -> Option<Vec<String>> {
    let (names, value) = match &stmt.kind {
        StmtKind::Let {
            name: bound, value, ..
        } => (vec![bound.name.clone()], value),
        StmtKind::LetPattern { pattern, value, .. } => (pattern_names(pattern), value),
        _ => return None,
    };
    (refers(value, name) && aliases(value, name, cx)).then_some(names)
}

fn aliases(value: &Expr, name: &str, cx: &Cx<'_>) -> bool {
    let part = |e: &Expr| aliases(e, name, cx) || keeps(e, name, cx);
    match &value.kind {
        _ if is_path(value, name) => true,
        ExprKind::Record { fields, .. } => fields.iter().all(|init| part(&init.value)),
        ExprKind::Tuple(items) => items.iter().all(part),
        _ => false,
    }
}

/// Whether `stmts`, a block's statements from some point on, keep the
/// binding `name` in scope before them inside the block: up to a statement
/// that binds the name anew, whose value still counts (the scope rule of
/// [`referred`]). An alias ([`alias_let`]) keeps it when the rest of the
/// block keeps the alias.
fn block_keeps(stmts: &[Stmt], name: &str, cx: &Cx<'_>) -> bool {
    for (i, stmt) in stmts.iter().enumerate() {
        let rest = &stmts[i + 1..];
        if let Some(aliases) = alias_let(stmt, name, cx) {
            if !aliases.iter().all(|alias| block_keeps(rest, alias, cx)) {
                return false;
            }
            if rebinds(stmt, name) {
                return true;
            }
            continue;
        }
        if rebinds(stmt, name) {
            return match &stmt.kind {
                StmtKind::Let { value, .. } | StmtKind::LetPattern { value, .. } => {
                    keeps(value, name, cx)
                }
                _ => true,
            };
        }
        if !stmt_keeps(stmt, name, cx) {
            return false;
        }
    }
    true
}

fn stmt_keeps(stmt: &Stmt, name: &str, cx: &Cx<'_>) -> bool {
    use crate::region::pattern_binds;
    let keeps = |e: &Expr| keeps(e, name, cx);
    let block = |b: &Block| block_keeps(&b.stmts, name, cx);
    match &stmt.kind {
        StmtKind::Let { value, .. } | StmtKind::LetPattern { value, .. } => keeps(value),
        StmtKind::Assign { target, value } => target.base.name != name && keeps(value),
        StmtKind::Return(value) => value.as_ref().is_none_or(keeps),
        StmtKind::While { cond, body } => keeps(cond) && block(body),
        StmtKind::Loop { body } => block(body),
        StmtKind::For {
            name: bound,
            iter,
            body,
        } => keeps(iter) && (bound.name == name || block(body)),
        StmtKind::ForPattern {
            pattern,
            iter,
            body,
        } => keeps(iter) && (pattern_binds(pattern, name) || block(body)),
        StmtKind::Break | StmtKind::Continue => true,
        StmtKind::Expr(e) => keeps(e),
        StmtKind::Assert { cond, message } => keeps(cond) && message.as_ref().is_none_or(keeps),
    }
}

/// The arguments of a call or method call `e`, the receiver first when
/// the callee takes it as its first parameter.
fn call_args<'e>(e: &'e Expr, types: &TypedProgram) -> Vec<&'e Expr> {
    match &e.kind {
        ExprKind::Call { args, .. } => args.iter().collect(),
        ExprKind::Method { receiver, args, .. } => match types.callee(e) {
            Some(Callee::Fn(_)) => std::iter::once(&**receiver).chain(args).collect(),
            _ => args.iter().collect(),
        },
        _ => Vec::new(),
    }
}

/// Whether every expression of `items` keeps `name` inside the block.
fn all_keep<'x>(mut items: impl Iterator<Item = &'x Expr>, name: &str, cx: &Cx<'_>) -> bool {
    items.all(|x| keeps(x, name, cx))
}

/// Whether the callee of the user fn or role call `e` keeps `name`: the
/// callee expression of a call, the receiver of a role call.
fn callee_keeps(e: &Expr, name: &str, cx: &Cx<'_>) -> bool {
    match &e.kind {
        ExprKind::Call { callee, .. } => keeps(callee, name, cx),
        ExprKind::Method { receiver, .. } if !matches!(cx.types.callee(e), Some(Callee::Fn(_))) => {
            keeps(receiver, name, cx)
        }
        _ => true,
    }
}

/// Whether a call `e` of a fn or role returns nothing that can alias its
/// arguments' handles: its result holds none, or only fresh ones.
fn result_apart(e: &Expr, cx: &Cx<'_>) -> bool {
    plain(cx.types.type_of(e))
        || matches!(cx.types.callee(e), Some(Callee::Fn(q)) if returns_fresh(q, cx))
}

/// Whether every mention of `name` in `e` keeps its handles inside the
/// block (see the module documentation).
fn keeps(e: &Expr, name: &str, cx: &Cx<'_>) -> bool {
    if !refers(e, name) {
        return true;
    }
    if is_path(e, name) && !is_name(e, name) && plain(cx.types.type_of(e)) {
        return true;
    }
    let types = cx.types;
    let block = |b: &Block| block_keeps(&b.stmts, name, cx);
    let arg_keeps = |arg: &Expr| is_path(arg, name) || keeps(arg, name, cx);
    // The value argument of a builtin that stores a heap copy: the copy is
    // an owner of its own, so a tuple or record literal around the binding
    // (a temporary that owns nothing) keeps it as the binding itself does.
    let copied_keeps = |arg: &Expr| aliases(arg, name, cx) || keeps(arg, name, cx);
    match &e.kind {
        ExprKind::Name(id) => id.name != name,
        ExprKind::Int(_)
        | ExprKind::Str(_)
        | ExprKind::Bool(_)
        | ExprKind::Quantity { .. }
        | ExprKind::Break
        | ExprKind::Continue => true,
        ExprKind::Closure(closure) => {
            closure.params.iter().any(|p| p.name.name == name) || !refers(&closure.body, name)
        }
        ExprKind::Spawn(call) => {
            let args = call_args(call, types);
            callee_keeps(call, name, cx)
                && args.iter().all(|arg| {
                    (is_path(arg, name) && spawn_owns(types.type_of(arg))) || keeps(arg, name, cx)
                })
        }
        ExprKind::Method { receiver, args, .. }
            if is_path(receiver, name)
                && matches!(types.callee(e), Some(Callee::Builtin(b)) if keeps_receiver(*b)) =>
        {
            if matches!(types.callee(e), Some(Callee::Builtin(b)) if copies_arguments(*b)) {
                args.iter().all(copied_keeps)
            } else {
                all_keep(args.iter(), name, cx)
            }
        }
        ExprKind::Method { receiver, args, .. } if matches!(types.callee(e), Some(Callee::Builtin(b)) if copies_arguments(*b)) => {
            keeps(receiver, name, cx) && args.iter().all(copied_keeps)
        }
        ExprKind::Call { .. } | ExprKind::Method { .. }
            if matches!(types.callee(e), Some(Callee::Fn(_) | Callee::Role { .. }))
                && result_apart(e, cx) =>
        {
            callee_keeps(e, name, cx) && call_args(e, types).iter().all(|arg| arg_keeps(arg))
        }
        ExprKind::Call { callee, args } => {
            keeps(callee, name, cx) && all_keep(args.iter(), name, cx)
        }
        ExprKind::Method { receiver, args, .. } => {
            keeps(receiver, name, cx) && all_keep(args.iter(), name, cx)
        }
        ExprKind::NamedArg { value, .. } => keeps(value, name, cx),
        ExprKind::Brace { entries } => all_keep(entries.iter().map(|entry| &entry.value), name, cx),
        ExprKind::RecordUpdate { fields, base, .. } => {
            all_keep(fields.iter().map(|init| &init.value), name, cx) && keeps(base, name, cx)
        }
        ExprKind::Record { fields, .. } => {
            all_keep(fields.iter().map(|init| &init.value), name, cx)
        }
        ExprKind::TupleField { receiver, .. } | ExprKind::Field { receiver, .. } => {
            keeps(receiver, name, cx)
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => all_keep(items.iter(), name, cx),
        ExprKind::Return(value) => value.as_ref().is_none_or(|v| keeps(v, name, cx)),
        ExprKind::Index { receiver, index } => keeps(receiver, name, cx) && keeps(index, name, cx),
        ExprKind::Try(inner) | ExprKind::Await(inner) => keeps(inner, name, cx),
        ExprKind::Unary { operand, .. } => keeps(operand, name, cx),
        ExprKind::Binary { .. } => {
            let mut cur = e;
            while let ExprKind::Binary { left, right, .. } = &cur.kind {
                if !keeps(right, name, cx) {
                    return false;
                }
                cur = left;
            }
            keeps(cur, name, cx)
        }
        ExprKind::Match { scrutinee, arms } => {
            keeps(scrutinee, name, cx)
                && arms.iter().all(|arm| {
                    crate::region::pattern_binds(&arm.pattern, name) || keeps(&arm.body, name, cx)
                })
        }
        ExprKind::If {
            cond,
            then,
            otherwise,
        } => {
            keeps(cond, name, cx)
                && block(then)
                && otherwise.as_ref().is_none_or(|o| keeps(o, name, cx))
        }
        ExprKind::Block(body) => block(body),
    }
}
