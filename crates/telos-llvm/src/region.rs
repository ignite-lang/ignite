//! Loop regions (`docs/llvm.md` section 4): the variables a loop body
//! modifies, which the loop deep-copies across a release of its region.
//!
//! A name is modified when the body (nested loops and blocks included)
//! assigns to it or to a field of it, or calls a mutating builtin (`push`,
//! `append`, `clear`) on a place rooted at it. The set is by name, so a body
//! that shadows an outer variable makes the outer one count as modified too:
//! an extra copy, never a missed one.

use std::collections::BTreeSet;

use telos_syntax::ast::{Block, Expr, ExprKind, StmtKind};
use telos_types::{Builtin, Callee, TypedProgram};

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
        telos_types::Builtin::Debug => unimplemented!("slice 9: x.debug()"),
        ListPush | ListAppend | ListClear | StoreNextId | StorePut | StoreRemove | StoreUpdate => {
            true
        }
        TextLen
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
        | StoreValues => false,
    }
}

fn block(body: &Block, types: &TypedProgram, names: &mut BTreeSet<String>) {
    for stmt in &body.stmts {
        match &stmt.kind {
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
        telos_syntax::ast::ExprKind::Closure(_) | telos_syntax::ast::ExprKind::RecordUpdate { .. } | telos_syntax::ast::ExprKind::TupleField { .. } | telos_syntax::ast::ExprKind::Break | telos_syntax::ast::ExprKind::Continue => unimplemented!("slice 9: closures, record update, tuple fields, break and continue expressions"),
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
        ExprKind::Binary { left, right, .. } => {
            expr(left, types, names);
            expr(right, types, names);
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

/// The variable whose memory the place expression `e` denotes, following
/// every expression the generator lowers to a pointer into its operand
/// (`docs/llvm.md` section 4): fields, elements, `?`, `Await` and the last
/// expression of a block.
fn root(e: &Expr, types: &TypedProgram) -> Option<String> {
    match &e.kind {
        ExprKind::Name(id) => Some(id.name.clone()),
        ExprKind::Field { receiver, .. }
        | ExprKind::Index { receiver, .. }
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
