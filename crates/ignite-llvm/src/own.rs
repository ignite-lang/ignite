//! Stream bindings a block owns (`docs/llvm.md` section 4). A handle-table
//! entry counts its owners; a stream binding the block owns is one of them
//! and is released when the block is left (at its end, by `break`,
//! `continue`, `return` or `?`), as the Rust target drops its `Arc` at the
//! end of the scope.
//!
//! The block owns a binding when the `let` creates the stream
//! (`listener.accept()?`, `TcpStream.connect(..)?`, `let (a, b) =
//! Pipe.pair()`) and no later statement of the block lets it escape. A
//! mention keeps the stream inside the block when it is the receiver of a
//! builtin stream method other than `copy`, an argument of `spawn` (the
//! task registers an owner of its own), or a plain argument of a call
//! whose result cannot hold a stream ([`may_hold_stream`]). Any other
//! mention (a result, a field, an element, a capture, an assignment, a
//! `copy()`) leaves the binding unowned, as before: never released, an
//! extra open descriptor and never an early close. Shadowing is not
//! resolved, so a mention of a shadowing binding counts too.
//!
//! A callee that keeps the stream after it returns other than through its
//! result (in a `Store`) would see it closed; no std module or example
//! does.

use ignite_syntax::ast::{Block, Expr, ExprKind, Ident, PatternKind, Stmt, StmtKind};
use ignite_types::{Builtin, Callee, Ty, TypedProgram};

/// The names that `stmts[index]` binds and the block owns, in binding
/// order; empty when it is no `let` that creates a stream.
pub fn owned(stmts: &[Stmt], index: usize, types: &TypedProgram) -> Vec<String> {
    let bound: Vec<&Ident> = match &stmts[index].kind {
        StmtKind::Let { name, value, .. } => {
            let created = matches!(&value.kind, ExprKind::Try(inner) if matches!(
                types.callee(inner),
                Some(Callee::Builtin(Builtin::TcpListenerAccept | Builtin::TcpStreamConnect))
            ));
            if created { vec![name] } else { Vec::new() }
        }
        StmtKind::LetPattern { pattern, value, .. }
            if matches!(
                types.callee(value),
                Some(Callee::Builtin(Builtin::PipePair))
            ) =>
        {
            match &pattern.kind {
                PatternKind::Tuple(items) => items
                    .iter()
                    .filter_map(|item| match &item.kind {
                        PatternKind::Binding(id) => Some(id),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    };
    bound
        .into_iter()
        .filter(|id| {
            stmts[index + 1..]
                .iter()
                .all(|stmt| stmt_keeps(stmt, &id.name, types))
        })
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
        StmtKind::Let { value, .. } | StmtKind::LetPattern { value, .. } => {
            crate::region::mentions(value, name)
        }
        _ => false,
    }) {
        return;
    }
    let Some(last) = scope
        .iter()
        .rposition(|stmt| crate::region::mentioned(std::slice::from_ref(stmt), name))
    else {
        return;
    };
    if let StmtKind::Expr(e) = &scope[last].kind {
        tail_handovers(e, name, out);
    }
}

fn tail_handovers<'s>(e: &'s Expr, name: &str, out: &mut Vec<&'s Expr>) {
    use crate::region::mentions;
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

/// Whether a result of type `ty` can hold a stream handle. Records, enums,
/// fn values and type parameters can; the answer for them is yes without
/// looking inside.
fn may_hold_stream(ty: &Ty) -> bool {
    match ty {
        Ty::Int
        | Ty::Bool
        | Ty::Byte
        | Ty::Unit
        | Ty::Text
        | Ty::IoError
        | Ty::Input
        | Ty::Never
        | Ty::Quantity(_)
        | Ty::TcpListener
        | Ty::Capability(_) => false,
        Ty::List(inner) | Ty::Option(inner) => may_hold_stream(inner),
        Ty::Result(ok, err) | Ty::Map(ok, err) => may_hold_stream(ok) || may_hold_stream(err),
        Ty::Tuple(items) => items.iter().any(may_hold_stream),
        _ => true,
    }
}

/// Whether a builtin method keeps its stream receiver inside the block.
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
            | Debug
    )
}

fn is_name(e: &Expr, name: &str) -> bool {
    match &e.kind {
        ExprKind::Name(id) => id.name == name,
        ExprKind::NamedArg { value, .. } => is_name(value, name),
        _ => false,
    }
}

fn stmt_keeps(stmt: &Stmt, name: &str, types: &TypedProgram) -> bool {
    let keeps = |e: &Expr| keeps(e, name, types);
    let block = |b: &Block| b.stmts.iter().all(|s| stmt_keeps(s, name, types));
    match &stmt.kind {
        StmtKind::Let { value, .. } | StmtKind::LetPattern { value, .. } => keeps(value),
        StmtKind::Assign { target, value } => target.base.name != name && keeps(value),
        StmtKind::Return(value) => value.as_ref().is_none_or(keeps),
        StmtKind::While { cond, body } => keeps(cond) && block(body),
        StmtKind::Loop { body } => block(body),
        StmtKind::For { iter, body, .. } | StmtKind::ForPattern { iter, body, .. } => {
            keeps(iter) && block(body)
        }
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
fn all_keep<'x>(
    mut items: impl Iterator<Item = &'x Expr>,
    name: &str,
    types: &TypedProgram,
) -> bool {
    items.all(|x| keeps(x, name, types))
}

/// Whether every mention of `name` in `e` keeps the stream inside the
/// block (see the module documentation).
fn keeps(e: &Expr, name: &str, types: &TypedProgram) -> bool {
    let block = |b: &Block| b.stmts.iter().all(|s| stmt_keeps(s, name, types));
    match &e.kind {
        ExprKind::Name(id) => id.name != name,
        ExprKind::Int(_)
        | ExprKind::Str(_)
        | ExprKind::Bool(_)
        | ExprKind::Quantity { .. }
        | ExprKind::Break
        | ExprKind::Continue => true,
        ExprKind::Closure(closure) => !crate::region::mentions(&closure.body, name),
        ExprKind::Spawn(call) => {
            let args = call_args(call, types);
            let callee = match &call.kind {
                ExprKind::Call { callee, .. } => keeps(callee, name, types),
                ExprKind::Method { receiver, .. }
                    if !matches!(types.callee(call), Some(Callee::Fn(_))) =>
                {
                    keeps(receiver, name, types)
                }
                _ => true,
            };
            callee
                && args
                    .iter()
                    .all(|arg| is_name(arg, name) || keeps(arg, name, types))
        }
        ExprKind::Method { receiver, args, .. }
            if is_name(receiver, name)
                && matches!(types.callee(e), Some(Callee::Builtin(b)) if keeps_receiver(*b)) =>
        {
            all_keep(args.iter(), name, types)
        }
        ExprKind::Call { .. } | ExprKind::Method { .. }
            if matches!(types.callee(e), Some(Callee::Fn(_) | Callee::Role { .. }))
                && !may_hold_stream(types.type_of(e)) =>
        {
            let callee = match &e.kind {
                ExprKind::Call { callee, .. } => keeps(callee, name, types),
                ExprKind::Method { receiver, .. }
                    if !matches!(types.callee(e), Some(Callee::Fn(_))) =>
                {
                    keeps(receiver, name, types)
                }
                _ => true,
            };
            callee
                && call_args(e, types)
                    .iter()
                    .all(|arg| is_name(arg, name) || keeps(arg, name, types))
        }
        ExprKind::Call { callee, args } => {
            keeps(callee, name, types) && all_keep(args.iter(), name, types)
        }
        ExprKind::Method { receiver, args, .. } => {
            keeps(receiver, name, types) && all_keep(args.iter(), name, types)
        }
        ExprKind::NamedArg { value, .. } => keeps(value, name, types),
        ExprKind::Brace { entries } => {
            all_keep(entries.iter().map(|entry| &entry.value), name, types)
        }
        ExprKind::RecordUpdate { fields, base, .. } => {
            all_keep(fields.iter().map(|init| &init.value), name, types) && keeps(base, name, types)
        }
        ExprKind::Record { fields, .. } => {
            all_keep(fields.iter().map(|init| &init.value), name, types)
        }
        ExprKind::TupleField { receiver, .. } | ExprKind::Field { receiver, .. } => {
            keeps(receiver, name, types)
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => all_keep(items.iter(), name, types),
        ExprKind::Return(value) => value.as_ref().is_none_or(|v| keeps(v, name, types)),
        ExprKind::Index { receiver, index } => {
            keeps(receiver, name, types) && keeps(index, name, types)
        }
        ExprKind::Try(inner) | ExprKind::Await(inner) => keeps(inner, name, types),
        ExprKind::Unary { operand, .. } => keeps(operand, name, types),
        ExprKind::Binary { .. } => {
            let mut cur = e;
            while let ExprKind::Binary { left, right, .. } = &cur.kind {
                if !keeps(right, name, types) {
                    return false;
                }
                cur = left;
            }
            keeps(cur, name, types)
        }
        ExprKind::Match { scrutinee, arms } => {
            keeps(scrutinee, name, types) && arms.iter().all(|arm| keeps(&arm.body, name, types))
        }
        ExprKind::If {
            cond,
            then,
            otherwise,
        } => {
            keeps(cond, name, types)
                && block(then)
                && otherwise.as_ref().is_none_or(|o| keeps(o, name, types))
        }
        ExprKind::Block(body) => block(body),
    }
}
