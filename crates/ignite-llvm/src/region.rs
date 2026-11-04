//! Loop regions (`docs/llvm.md` section 4): the variables a loop body
//! modifies, which the loop deep-copies across a release of its region.
//!
//! A name is modified when the body (nested loops and blocks included)
//! assigns to it or to a field of it, or calls a mutating builtin (`push`,
//! `append`, `clear`) on a place rooted at it. The set is by name, so a body
//! that shadows an outer variable makes the outer one count as modified too:
//! an extra copy, never a missed one.

use std::collections::BTreeSet;

use ignite_syntax::ast::{Block, Expr, ExprKind, Pattern, PatternKind, Stmt, StmtKind};
use ignite_types::{Builtin, Callee, TypedProgram};

/// The names `body` modifies, in name order.
pub fn modified(body: &Block, types: &TypedProgram) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    block(body, types, &mut names);
    names
}

/// Whether a builtin changes the value its receiver denotes. The match is
/// exhaustive so that a new builtin needs a decision here.
fn mutates(builtin: Builtin) -> bool {
    use Builtin::*;
    match builtin {
        ListPush | ListAppend | ListClear | MapPut | MapRemove | StoreNextId | StorePut
        | StoreRemove | StoreUpdate | StoreUpsert => true,
        MapEmpty
        | MapGet
        | MapKeys
        | MapLen
        | MapContains
        | DurationToMs
        | DurationToNs
        | ByteSizeToInt
        | RatioPpm
        | IoErrorWithExitCode
        | ModuleRecord
        | ClockMonotonic
        | EndpointOf
        | TextLen
        | TextIsEmpty
        | TextTrim
        | TextStartsWith
        | TextStripPrefix
        | TextConcat
        | TextToBytes
        | BytesToText
        | ListLen
        | ListIsEmpty
        | ListAt
        | ListSlice
        | ListJoin
        | Copy
        | BytesEmpty
        | BytesOf
        | ListEmpty
        | StoreNew
        | IntToText
        | Log
        | TcpListenerBind
        | IntToTextMethod
        | ByteToText
        | BoolToText
        | TextToText
        | TcpListenerAccept
        | TcpListenerLocalPort
        | TcpStreamConnect
        | TcpStreamRead
        | TcpStreamWriteAll
        | TcpStreamClose
        | StdinRead
        | StdinIsTerminal
        | StdinSetRaw
        | StdoutWriteAll
        | ProcessExit
        | IoErrorNew
        | ListMap
        | IntParse
        | ByteToInt
        | IntToByte
        | TcpStreamSetIdleTimeout
        | StoreGet
        | StoreValues
        | PipePair
        | PipeStreamRead
        | PipeStreamWriteAll
        | PipeStreamClose
        | PipeStreamSetIdleTimeout
        | PipeStreamReceivedBatches
        | StreamRead
        | StreamWriteAll
        | StreamClose
        | StreamSetIdleTimeout
        | Debug => false,
    }
}

fn block(body: &Block, types: &TypedProgram, names: &mut BTreeSet<String>) {
    for stmt in &body.stmts {
        match &stmt.kind {
            StmtKind::ForPattern { iter, body, .. } => {
                expr(iter, types, names);
                block(body, types, names);
            }
            StmtKind::Let { value, .. } | StmtKind::LetPattern { value, .. } => {
                expr(value, types, names)
            }
            StmtKind::Assign { target, value } => {
                names.insert(target.base.name.clone());
                expr(value, types, names);
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    expr(value, types, names);
                }
            }
            StmtKind::While { cond, body } => {
                expr(cond, types, names);
                block(body, types, names);
            }
            StmtKind::Loop { body } => block(body, types, names),
            StmtKind::For { iter, body, .. } => {
                expr(iter, types, names);
                block(body, types, names);
            }
            StmtKind::Break | StmtKind::Continue => {}
            StmtKind::Expr(e) => expr(e, types, names),
            StmtKind::Assert { cond, message } => {
                expr(cond, types, names);
                if let Some(message) = message {
                    expr(message, types, names);
                }
            }
        }
    }
}

/// The names an expression modifies (assignments in its blocks and mutating
/// builtins), as [`modified`] for a block.
pub fn modified_by(e: &Expr, types: &TypedProgram) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    expr(e, types, &mut names);
    names
}

fn mutated(types: &TypedProgram, e: &Expr) -> bool {
    matches!(types.callee(e), Some(Callee::Builtin(b)) if mutates(*b))
}

fn expr(e: &Expr, types: &TypedProgram, names: &mut BTreeSet<String>) {
    match &e.kind {
        ExprKind::NamedArg { value, .. } => expr(value, types, names),
        ExprKind::Brace { entries } => {
            for entry in entries {
                expr(&entry.value, types, names);
            }
        }
        ExprKind::Quantity { .. } | ExprKind::Closure(_) | ExprKind::Break | ExprKind::Continue => {
        }
        ExprKind::RecordUpdate { fields, base, .. } => {
            for init in fields {
                expr(&init.value, types, names);
            }
            expr(base, types, names);
        }
        ExprKind::TupleField { receiver, .. } => expr(receiver, types, names),
        ExprKind::Tuple(items) => {
            for item in items {
                expr(item, types, names);
            }
        }
        ExprKind::Return(value) => {
            if let Some(value) = value {
                expr(value, types, names);
            }
        }
        ExprKind::Int(_) | ExprKind::Str(_) | ExprKind::Bool(_) | ExprKind::Name(_) => {}
        ExprKind::Record { fields, .. } => {
            for init in fields {
                expr(&init.value, types, names);
            }
        }
        ExprKind::List(items) => {
            for item in items {
                expr(item, types, names);
            }
        }
        ExprKind::Call { callee, args } => {
            if mutated(types, e)
                && let ExprKind::Field { receiver, .. } = &callee.kind
                && let Some(root) = root(receiver, types)
            {
                names.insert(root);
            }
            expr(callee, types, names);
            for arg in args {
                expr(arg, types, names);
            }
        }
        ExprKind::Method { receiver, args, .. } => {
            if mutated(types, e)
                && let Some(root) = root(receiver, types)
            {
                names.insert(root);
            }
            expr(receiver, types, names);
            for arg in args {
                expr(arg, types, names);
            }
        }
        ExprKind::Field { receiver, .. } => expr(receiver, types, names),
        ExprKind::Index { receiver, index } => {
            expr(receiver, types, names);
            expr(index, types, names);
        }
        ExprKind::Try(inner) | ExprKind::Await(inner) | ExprKind::Spawn(inner) => {
            expr(inner, types, names)
        }
        ExprKind::Unary { operand, .. } => expr(operand, types, names),
        ExprKind::Binary { .. } => {
            let mut cur = e;
            while let ExprKind::Binary { left, right, .. } = &cur.kind {
                expr(right, types, names);
                cur = left;
            }
            expr(cur, types, names);
        }
        ExprKind::Match { scrutinee, arms } => {
            expr(scrutinee, types, names);
            for arm in arms {
                expr(&arm.body, types, names);
            }
        }
        ExprKind::If {
            cond,
            then,
            otherwise,
        } => {
            expr(cond, types, names);
            block(then, types, names);
            if let Some(otherwise) = otherwise {
                expr(otherwise, types, names);
            }
        }
        ExprKind::Block(body) => block(body, types, names),
    }
}

/// Whether any of `stmts` mentions the name `name` anywhere (nested blocks,
/// arms, closures and patterns included). Shadowing is not resolved: a
/// mention of a shadowing binding counts, an extra keep and never a missed
/// one. Used by `spawn` to decide whether a stream handle moves to the task
/// (`docs/llvm.md` section 4).
pub fn mentioned(stmts: &[Stmt], name: &str) -> bool {
    stmts.iter().any(|stmt| stmt_mentions(stmt, name))
}

/// The arguments of the `spawn f(args)` at `stmts[index]` whose stream
/// handle moves to the task (`docs/llvm.md` section 4): a plain name that a
/// `let` earlier in the same block bound to a fresh value (a call, a method
/// call, `?` or `match`, so not a parameter, a field or another variable)
/// and that no later statement of the block mentions. The accept loop's
/// `let stream = listener.accept()?` followed by `spawn serve(stream, ..)`
/// moves; a client role's `spawn forward_input(stream)` on its parameter,
/// which the role keeps reading, does not.
pub fn moved_to_task(stmts: &[Stmt], index: usize, call: &Expr) -> Vec<String> {
    let args: Vec<&Expr> = match &call.kind {
        ExprKind::Call { args, .. } => args.iter().collect(),
        ExprKind::Method { receiver, args, .. } => {
            std::iter::once(&**receiver).chain(args).collect()
        }
        _ => Vec::new(),
    };
    args.iter()
        .filter_map(|arg| match &arg.kind {
            ExprKind::Name(id) => Some(id.name.clone()),
            _ => None,
        })
        .filter(|name| fresh_let(&stmts[..index], name) && !mentioned(&stmts[index + 1..], name))
        .collect()
}

/// Whether the last `let` of `name` among `stmts` binds a fresh value. A
/// `copy()` shares its receiver's handle, so it is not fresh: the task
/// must not release the handle another binding still holds.
fn fresh_let(stmts: &[Stmt], name: &str) -> bool {
    stmts
        .iter()
        .rev()
        .find_map(|stmt| match &stmt.kind {
            StmtKind::Let {
                name: bound, value, ..
            } if bound.name == name => Some(fresh_value(value)),
            _ => None,
        })
        .unwrap_or(false)
}

fn fresh_value(value: &Expr) -> bool {
    match &value.kind {
        ExprKind::Method { name, .. } => name.name != "copy",
        ExprKind::Try(inner) => {
            !matches!(&inner.kind, ExprKind::Method { name, .. } if name.name == "copy")
        }
        ExprKind::Call { .. } | ExprKind::Match { .. } => true,
        _ => false,
    }
}

fn stmt_mentions(stmt: &Stmt, name: &str) -> bool {
    match &stmt.kind {
        StmtKind::Let {
            name: bound, value, ..
        } => bound.name == name || mentions(value, name),
        StmtKind::LetPattern { pattern, value, .. } => {
            pattern_binds(pattern, name) || mentions(value, name)
        }
        StmtKind::Assign { target, value } => target.base.name == name || mentions(value, name),
        StmtKind::Return(value) => value.as_ref().is_some_and(|v| mentions(v, name)),
        StmtKind::While { cond, body } => mentions(cond, name) || mentioned(&body.stmts, name),
        StmtKind::Loop { body } => mentioned(&body.stmts, name),
        StmtKind::For {
            name: bound,
            iter,
            body,
        } => bound.name == name || mentions(iter, name) || mentioned(&body.stmts, name),
        StmtKind::ForPattern {
            pattern,
            iter,
            body,
        } => pattern_binds(pattern, name) || mentions(iter, name) || mentioned(&body.stmts, name),
        StmtKind::Break | StmtKind::Continue => false,
        StmtKind::Expr(e) => mentions(e, name),
        StmtKind::Assert { cond, message } => {
            mentions(cond, name) || message.as_ref().is_some_and(|m| mentions(m, name))
        }
    }
}

fn pattern_binds(pattern: &Pattern, name: &str) -> bool {
    match &pattern.kind {
        PatternKind::Binding(id) => id.name == name,
        PatternKind::Or(items)
        | PatternKind::Tuple(items)
        | PatternKind::List(items)
        | PatternKind::Variant { args: items, .. } => items.iter().any(|p| pattern_binds(p, name)),
        PatternKind::Wildcard
        | PatternKind::Int(_)
        | PatternKind::Str(_)
        | PatternKind::Bool(_)
        | PatternKind::Range { .. } => false,
    }
}

/// Whether `e` mentions the name `name` (see [`mentioned`]).
pub fn mentions(e: &Expr, name: &str) -> bool {
    match &e.kind {
        ExprKind::Name(id) => id.name == name,
        ExprKind::Int(_)
        | ExprKind::Str(_)
        | ExprKind::Bool(_)
        | ExprKind::Quantity { .. }
        | ExprKind::Break
        | ExprKind::Continue => false,
        ExprKind::NamedArg { value, .. } => mentions(value, name),
        ExprKind::Brace { entries } => entries.iter().any(|entry| mentions(&entry.value, name)),
        ExprKind::Closure(closure) => {
            closure.params.iter().any(|p| p.name.name == name) || mentions(&closure.body, name)
        }
        ExprKind::RecordUpdate { fields, base, .. } => {
            fields.iter().any(|init| mentions(&init.value, name)) || mentions(base, name)
        }
        ExprKind::Record { fields, .. } => fields.iter().any(|init| mentions(&init.value, name)),
        ExprKind::TupleField { receiver, .. } | ExprKind::Field { receiver, .. } => {
            mentions(receiver, name)
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => {
            items.iter().any(|item| mentions(item, name))
        }
        ExprKind::Return(value) => value.as_ref().is_some_and(|v| mentions(v, name)),
        ExprKind::Call { callee, args } => {
            mentions(callee, name) || args.iter().any(|arg| mentions(arg, name))
        }
        ExprKind::Method { receiver, args, .. } => {
            mentions(receiver, name) || args.iter().any(|arg| mentions(arg, name))
        }
        ExprKind::Index { receiver, index } => mentions(receiver, name) || mentions(index, name),
        ExprKind::Try(inner) | ExprKind::Await(inner) | ExprKind::Spawn(inner) => {
            mentions(inner, name)
        }
        ExprKind::Unary { operand, .. } => mentions(operand, name),
        ExprKind::Binary { .. } => {
            let mut cur = e;
            while let ExprKind::Binary { left, right, .. } = &cur.kind {
                if mentions(right, name) {
                    return true;
                }
                cur = left;
            }
            mentions(cur, name)
        }
        ExprKind::Match { scrutinee, arms } => {
            mentions(scrutinee, name)
                || arms
                    .iter()
                    .any(|arm| pattern_binds(&arm.pattern, name) || mentions(&arm.body, name))
        }
        ExprKind::If {
            cond,
            then,
            otherwise,
        } => {
            mentions(cond, name)
                || mentioned(&then.stmts, name)
                || otherwise.as_ref().is_some_and(|o| mentions(o, name))
        }
        ExprKind::Block(body) => mentioned(&body.stmts, name),
    }
}

/// The variable whose memory the place expression `e` denotes, following
/// every expression the generator lowers to a pointer into its operand
/// (`docs/llvm.md` section 4): fields, elements, `?`, `Await` and the last
/// expression of a block.
pub fn root(e: &Expr, types: &TypedProgram) -> Option<String> {
    match &e.kind {
        ExprKind::Name(id) => Some(id.name.clone()),
        ExprKind::Field { receiver, .. }
        | ExprKind::Index { receiver, .. }
        | ExprKind::TupleField { receiver, .. }
        | ExprKind::Try(receiver)
        | ExprKind::Await(receiver) => root(receiver, types),
        ExprKind::Method { receiver, .. }
            if matches!(types.callee(e), Some(Callee::Builtin(Builtin::ListAt))) =>
        {
            root(receiver, types)
        }
        ExprKind::Call { callee, .. }
            if matches!(types.callee(e), Some(Callee::Builtin(Builtin::ListAt))) =>
        {
            match &callee.kind {
                ExprKind::Field { receiver, .. } => root(receiver, types),
                _ => None,
            }
        }
        ExprKind::Block(body) => match body.stmts.last().map(|s| &s.kind) {
            Some(StmtKind::Expr(last)) => root(last, types),
            _ => None,
        },
        _ => None,
    }
}
