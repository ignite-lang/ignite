//! Effect sets of the pure contexts the type checker owns (`docs/slice.md`
//! section 15.2): the property expressions of resources, components,
//! contracts, policies and assemblies, constant values and field defaults.
//! A property expression with a non-empty set is E116, reported at its
//! first effectful operation, as E010 is for a constant.

use ignite_check::EffectSig;
use ignite_syntax::ast::{Block, Expr, ExprKind, Span, StmtKind};

use super::Typer;
use crate::{Builtin, Callee, EffectSet, FnEffects, Ty, key};

/// One effectful operation found in an expression.
pub(super) struct Operation {
    /// The entries it adds to the effect set (never empty).
    pub entries: Vec<String>,
    pub span: Span,
    /// How the diagnostic names it: ``effectful call `noisy` `` or
    /// ``effectful operation `accept` `` or `` `spawn` ``.
    pub what: String,
}

/// What a builtin performs (`docs/slice.md` section 5.1).
enum Effect {
    /// An ambient effect, as written.
    Ambient(&'static str),
    /// An operation on the receiver's roots.
    Receiver(&'static str),
}

/// The effect of an effectful builtin and the name the diagnostic uses;
/// `None` for a pure builtin.
fn builtin_effect(builtin: Builtin) -> Option<(Effect, &'static str)> {
    use Builtin::*;
    Some(match builtin {
        TcpListenerBind => (Effect::Ambient("net.listen"), "TcpListener.bind"),
        TcpStreamConnect => (Effect::Ambient("net.connect"), "TcpStream.connect"),
        StdinRead => (Effect::Ambient("stdin.read"), "Stdin.read"),
        StdinIsTerminal => (Effect::Ambient("stdin.mode"), "Stdin.is_terminal"),
        StdinSetRaw => (Effect::Ambient("stdin.mode"), "Stdin.set_raw"),
        StdoutWriteAll => (Effect::Ambient("stdout.write"), "Stdout.write_all"),
        StderrWriteAll => (Effect::Ambient("stderr.write"), "Stderr.write_all"),
        ProcessExit => (Effect::Ambient("process.exit"), "Process.exit"),
        StoreNew => (Effect::Ambient("store.new"), "Store.new"),
        ClockMonotonic => (Effect::Ambient("clock.read"), "Clock.monotonic"),
        EndpointOf => (Effect::Ambient("process.args"), "Endpoint.of"),
        PipePair => (Effect::Ambient("pipe.pair"), "Pipe.pair"),
        TcpListenerAccept => (Effect::Receiver("accept"), "accept"),
        TcpStreamRead => (Effect::Receiver("read"), "read"),
        TcpStreamWriteAll => (Effect::Receiver("write"), "write_all"),
        TcpStreamClose => (Effect::Receiver("write"), "close"),
        TcpStreamSetIdleTimeout => (Effect::Receiver("write"), "set_idle_timeout"),
        PipeStreamRead | StreamRead => (Effect::Receiver("read"), "read"),
        PipeStreamWriteAll | StreamWriteAll => (Effect::Receiver("write"), "write_all"),
        PipeStreamClose | StreamClose => (Effect::Receiver("write"), "close"),
        PipeStreamSetIdleTimeout | StreamSetIdleTimeout => {
            (Effect::Receiver("write"), "set_idle_timeout")
        }
        PipeStreamReceivedBatches => (Effect::Receiver("read"), "received_batches"),
        TcpStreamWriteUrgent | PipeStreamWriteUrgent | StreamWriteUrgent => {
            (Effect::Receiver("write"), "write_urgent")
        }
        TcpStreamUrgentPending | PipeStreamUrgentPending | StreamUrgentPending => {
            (Effect::Receiver("read"), "urgent_pending")
        }
        StoreGet => (Effect::Receiver("read"), "get"),
        StoreValues => (Effect::Receiver("read"), "values"),
        StoreNextId => (Effect::Receiver("write"), "next_id"),
        StorePut => (Effect::Receiver("write"), "put"),
        StoreRemove => (Effect::Receiver("write"), "remove"),
        StoreUpdate => (Effect::Receiver("write"), "update"),
        StoreUpsert => (Effect::Receiver("write"), "upsert"),
        DirectoryTemporary => (Effect::Ambient("directory.new"), "Directory.temporary"),
        LauncherNew => (Effect::Ambient("launcher.new"), "Launcher.new"),
        EntropyNew => (Effect::Ambient("entropy.new"), "Entropy.new"),
        SettingOf => (Effect::Ambient("setting.new"), "Setting.of"),
        CommandLineOf => (Effect::Ambient("command_line.new"), "CommandLine.of"),
        DirectoryReadFile => (Effect::Receiver("read"), "read_file"),
        DirectoryReadDirectory => (Effect::Receiver("read"), "read_directory"),
        DirectoryAbsolutePath => (Effect::Receiver("read"), "absolute_path"),
        DirectoryWriteFile => (Effect::Receiver("write"), "write_file"),
        DirectoryCreateDirectory => (Effect::Receiver("write"), "create_directory"),
        DirectoryRemove => (Effect::Receiver("write"), "remove"),
        DirectoryRemoveTree => (Effect::Receiver("write"), "remove_tree"),
        LauncherStart => (Effect::Receiver("launch"), "start"),
        LauncherExecute => (Effect::Receiver("launch"), "execute"),
        ChildProcessWait => (Effect::Receiver("wait"), "wait"),
        ChildProcessKill => (Effect::Receiver("kill"), "kill"),
        EntropyRandomBytes => (Effect::Receiver("draw"), "random_bytes"),
        SettingValue => (Effect::Receiver("argument"), "value"),
        CommandLineArguments => (Effect::Receiver("argument"), "arguments"),
        TcpStreamTimedOut => (Effect::Receiver("read"), "timed_out"),
        TcpStreamConnectWithin => (Effect::Ambient("net.connect"), "TcpStream.connect_within"),
        _ => return None,
    })
}

/// The direct sub-expressions of `expr` in source order, the statements
/// of its blocks included and closure bodies excluded.
fn children(expr: &Expr) -> Vec<&Expr> {
    fn block<'e>(b: &'e Block, out: &mut Vec<&'e Expr>) {
        for stmt in &b.stmts {
            match &stmt.kind {
                StmtKind::Let { value, .. }
                | StmtKind::LetPattern { value, .. }
                | StmtKind::Assign { value, .. }
                | StmtKind::Expr(value) => out.push(value),
                StmtKind::Return(value) => out.extend(value.iter()),
                StmtKind::While { cond, body } => {
                    out.push(cond);
                    block(body, out);
                }
                StmtKind::Loop { body } => block(body, out),
                StmtKind::For { iter, body, .. } | StmtKind::ForPattern { iter, body, .. } => {
                    out.push(iter);
                    block(body, out);
                }
                StmtKind::Assert { cond, message } => {
                    out.push(cond);
                    out.extend(message.iter());
                }
                StmtKind::Break | StmtKind::Continue => {}
            }
        }
    }
    let mut out = Vec::new();
    match &expr.kind {
        ExprKind::Int(_)
        | ExprKind::Str(_)
        | ExprKind::Bool(_)
        | ExprKind::Name(_)
        | ExprKind::Break
        | ExprKind::Continue
        | ExprKind::Quantity { .. }
        | ExprKind::Closure(_) => {}
        ExprKind::Record { fields, .. } => out.extend(fields.iter().map(|f| &f.value)),
        ExprKind::RecordUpdate { fields, base, .. } => {
            out.extend(fields.iter().map(|f| &f.value));
            out.push(base);
        }
        ExprKind::List(items) | ExprKind::Tuple(items) => out.extend(items.iter()),
        ExprKind::Return(value) => out.extend(value.iter().map(|v| &**v)),
        ExprKind::Call { callee, args } => {
            out.push(callee);
            out.extend(args.iter());
        }
        ExprKind::Method { receiver, args, .. } => {
            out.push(receiver);
            out.extend(args.iter());
        }
        ExprKind::Field { receiver, .. } | ExprKind::TupleField { receiver, .. } => {
            out.push(receiver)
        }
        ExprKind::Index { receiver, index } => {
            out.push(receiver);
            out.push(index);
        }
        ExprKind::Try(inner) | ExprKind::Await(inner) | ExprKind::Spawn(inner) => out.push(inner),
        ExprKind::Unary { operand, .. } => out.push(operand),
        ExprKind::Binary { left, right, .. } => {
            out.push(left);
            out.push(right);
        }
        ExprKind::Match { scrutinee, arms } => {
            out.push(scrutinee);
            out.extend(arms.iter().map(|arm| &arm.body));
        }
        ExprKind::If {
            cond,
            then,
            otherwise,
        } => {
            out.push(cond);
            block(then, &mut out);
            out.extend(otherwise.iter().map(|o| &**o));
        }
        ExprKind::Block(b) => block(b, &mut out),
        ExprKind::NamedArg { value, .. } => out.push(value),
        ExprKind::Brace { entries } => out.extend(entries.iter().map(|e| &e.value)),
    }
    out
}

/// The entries a callee with the effect signature `sig` needs from the
/// expression: a requirement on a parameter becomes `r.op` for each root
/// `r` of the argument passed for it, or stays as written when the
/// argument names no member; every other requirement passes unchanged.
fn map_requires(
    sig: &EffectSig,
    args: &[&Expr],
    roots: &dyn Fn(&Expr) -> Vec<String>,
) -> Vec<String> {
    let mut entries = Vec::new();
    for requirement in &sig.requires {
        let mapped = requirement.split_once('.').and_then(|(head, op)| {
            let index = sig.params.iter().position(|p| p == head)?;
            let found = roots(args.get(index)?);
            (!found.is_empty()).then(|| found.into_iter().map(|r| format!("{r}.{op}")).collect())
        });
        match mapped {
            Some(mapped) => entries.extend::<Vec<String>>(mapped),
            None => entries.push(requirement.clone()),
        }
    }
    entries
}

fn sig_of_effects(effects: &FnEffects) -> EffectSig {
    EffectSig {
        params: effects.names.clone(),
        requires: effects.requires.clone(),
    }
}

impl Typer<'_> {
    /// Record the effect set of `expr` and of each of its sub-expressions
    /// for [`crate::TypedProgram::effects`], returning the effectful
    /// operations in source order.
    pub(super) fn record_effects(&mut self, expr: &Expr) -> Vec<Operation> {
        let mut operations = Vec::new();
        self.effects_of(expr, &mut operations);
        operations
    }

    /// E116 when the property expression `expr` of `property` in the
    /// declaration `owner` (``component `svc` ``) performs an effect,
    /// reported at its first effectful operation; the effect sets are
    /// recorded either way.
    pub(super) fn property_effects(&mut self, expr: &Expr, property: &str, owner: &str) {
        let operations = self.record_effects(expr);
        if let Some(first) = operations.first() {
            self.error(
                "E116",
                first.span,
                format!(
                    "{} in the property `{property}` of {owner}: a property expression is pure",
                    first.what
                ),
            );
        }
    }

    /// The members of the system being walked that `expr` names, in
    /// source order: the roots of its value.
    fn member_roots(&self, expr: &Expr) -> Vec<String> {
        let Some(system) = &self.current_system else {
            return Vec::new();
        };
        let Some(members) = self.system_members.get(system) else {
            return Vec::new();
        };
        let mut roots = Vec::new();
        let mut pending = vec![expr];
        while let Some(e) = pending.pop() {
            if let ExprKind::Name(id) = &e.kind
                && members.iter().any(|m| m.name == id.name)
                && !roots.contains(&id.name)
            {
                roots.push(id.name.clone());
            }
            let mut next = children(e);
            next.reverse();
            pending.extend(next);
        }
        roots
    }

    /// The entries of `expr`'s own operation, if it performs one.
    fn operation(&self, expr: &Expr) -> Option<Operation> {
        let roots = |e: &Expr| self.member_roots(e);
        let (receiver, args, callee): (Option<&Expr>, Vec<&Expr>, Option<&Expr>) = match &expr.kind
        {
            ExprKind::Spawn(_) => {
                return Some(Operation {
                    entries: vec!["spawn".to_string()],
                    span: expr.span,
                    what: "`spawn`".to_string(),
                });
            }
            ExprKind::Call { callee, args } => (None, args.iter().collect(), Some(callee)),
            ExprKind::Method { receiver, args, .. } => {
                (Some(&**receiver), args.iter().collect(), None)
            }
            _ => return None,
        };
        let found = self.callees.get(&key(expr))?;
        let (entries, what) = match found {
            Callee::Fn(q) => {
                let sig = self.sig_of(q)?;
                if !sig.effectful {
                    return None;
                }
                let mut all: Vec<&Expr> = receiver.into_iter().collect();
                all.extend(args.iter().copied());
                let entries = match self.symbols.effects.get(&q.name) {
                    Some(sig) => map_requires(sig, &all, &roots),
                    None => vec![q.name.clone()],
                };
                (entries, format!("effectful call `{}`", q.name))
            }
            Callee::Role { protocol, role } => {
                let name = format!("{}.{role}", protocol.name);
                let entries = match self.symbols.effects.get(&name) {
                    Some(sig) => map_requires(sig, &args, &roots),
                    None => vec![name.clone()],
                };
                (entries, format!("effectful call `{name}`"))
            }
            Callee::Builtin(builtin) => {
                let (effect, name) = builtin_effect(*builtin)?;
                let entries = match effect {
                    Effect::Ambient(entry) => vec![entry.to_string()],
                    Effect::Receiver(op) => {
                        let found = receiver.map(roots).unwrap_or_default();
                        if found.is_empty() {
                            vec![op.to_string()]
                        } else {
                            found.into_iter().map(|r| format!("{r}.{op}")).collect()
                        }
                    }
                };
                let what = match effect {
                    Effect::Ambient(_) => format!("effectful call `{name}`"),
                    Effect::Receiver(_) => format!("effectful operation `{name}`"),
                };
                (entries, what)
            }
            Callee::Local => {
                let effects = match callee.and_then(|c| self.types.get(&key(c))) {
                    Some(Ty::EffectFn(_, _, effects)) => effects,
                    _ => return None,
                };
                let name = match callee.map(|c| &c.kind) {
                    Some(ExprKind::Name(id)) => id.name.clone(),
                    _ => "a fn value".to_string(),
                };
                let entries = map_requires(&sig_of_effects(effects), &args, &roots);
                (entries, format!("effectful call `{name}`"))
            }
            Callee::FieldFn { field, .. } => {
                let receiver_ty = receiver.and_then(|r| self.types.get(&key(r)))?;
                let effects = match self.field_type(receiver_ty, field) {
                    Some(Ty::EffectFn(_, _, effects)) => effects,
                    _ => return None,
                };
                let entries = map_requires(&sig_of_effects(&effects), &args, &roots);
                (entries, format!("effectful call `{field}`"))
            }
            Callee::Variant { .. } | Callee::Const(_) => return None,
        };
        if entries.is_empty() {
            return None;
        }
        Some(Operation {
            entries,
            span: expr.span,
            what,
        })
    }

    /// The effect set of `expr`: its own operation's entries plus its
    /// sub-expressions', recorded for every node.
    fn effects_of(&mut self, expr: &Expr, operations: &mut Vec<Operation>) -> Vec<String> {
        let mut entries = Vec::new();
        for child in children(expr) {
            entries.extend(self.effects_of(child, operations));
        }
        if let Some(operation) = self.operation(expr) {
            entries.extend(operation.entries.iter().cloned());
            operations.push(operation);
        }
        operations.sort_by_key(|o| o.span.start);
        let set = EffectSet::new(entries.clone());
        self.system.effects.insert(key(expr), set);
        entries
    }
}
