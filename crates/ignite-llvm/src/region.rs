//! Mentions of a name, the stream handles a `spawn` moves to its task and
//! whether emitted code can allocate (`docs/llvm.md` section 4). The names
//! a body changes in place, which a
//! loop deep-copies across a release of its region, and the variable a
//! place is rooted at are `ignite_types::ownership`'s.

use ignite_syntax::ast::{Expr, ExprKind, Pattern, Stmt, StmtKind};

/// Whether the IR `text` (a function's definition, or what a loop emitted
/// from its head to its back-edge) holds a call that can allocate in the
/// arena (`docs/llvm.md` section 4): an indirect call, an `invoke`, or a
/// direct call of anything but an intrinsic, the inline helpers that only
/// read the bump state, a runtime entry that never returns or only
/// compares, and a function `quiet` names (one of the module already found
/// to allocate nothing). A loop whose body allocates nothing has nothing
/// to release at its back-edge.
pub fn allocates(text: &str, quiet: impl Fn(&str) -> bool) -> bool {
    text.lines().any(|line| {
        if line.contains(" invoke ") {
            return true;
        }
        let Some(at) = line.find(" call ") else {
            return false;
        };
        let rest = &line[at + " call ".len()..];
        let Some(start) = rest.find(['@', '%']) else {
            return true;
        };
        let Some(name) = rest[start..].strip_prefix('@') else {
            return true;
        };
        let name = match name.strip_prefix('"') {
            Some(quoted) => quoted.split('"').next().unwrap_or(""),
            None => {
                let end = name
                    .find(|c: char| !(c.is_ascii_alphanumeric() || "._$-".contains(c)))
                    .unwrap_or(name.len());
                &name[..end]
            }
        };
        !(quiet_entry(name) || quiet(name))
    })
}

/// The callees every module may call without allocating: intrinsics, the
/// inline helpers that read the bump state, the runtime entries that end
/// the task or the process, and the text comparisons.
fn quiet_entry(name: &str) -> bool {
    name.starts_with("llvm.")
        || name.starts_with("ignite_rt_fail")
        || matches!(
            name,
            "ignite.arena_mark"
                | "ignite.arena_due"
                | "ignite_rt_arena_state"
                | "ignite_rt_exit"
                | "ignite_rt_process_exit"
                | "ignite_rt_text_eq"
                | "ignite_rt_text_starts_with"
        )
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

/// Whether `pattern` binds `name` ([`Pattern::binds`]).
pub fn pattern_binds(pattern: &Pattern, name: &str) -> bool {
    pattern.binds(name)
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
