//! The evaluator's side of the model of the substrate under a test
//! (`docs/slice.md` 33.5): a test's body as task 0 of the model, the
//! spawned tasks, role calls, and the builtins that have a value only
//! under a test (stores, capabilities, pipes, channels, the virtual clock).
//! The model itself is `crate::substrate`.

use ignite_syntax::ast::{Decl, Expr, ExprKind};
use ignite_types::{Builtin, Callee, QualName, QuantityKind};

use super::{Cx, Eval, EvalError, Evaluator, Flow, Frame, bytes_of, internal};
use crate::Value;
use crate::substrate::{Call, HandleKind, Job, Over, Read, Wait, World};

impl<'p> Evaluator<'p> {
    /// Run `body`, a test's, as task 0 of a fresh model of the substrate
    /// (`docs/slice.md` 33.5): its value is `Unit` when the body ended
    /// without failing, and the outcome is the body's, or the reason the
    /// evaluator cannot run a task, or the model's deadlock. Every task
    /// runs on a thread of its own and only the one holding the turn runs;
    /// this thread supervises: it starts the thread of each spawned task
    /// and returns when every task's thread has ended.
    pub(super) fn in_world(
        &mut self,
        body: impl FnOnce(&mut Self) -> Eval<Value> + Send,
    ) -> Eval<Value> {
        let world = World::new(self.units_left);
        let modules = self.modules;
        let fuel = self.fuel;
        let (time_limit, deadline) = (self.time_limit, self.deadline);
        let (outcome, units) = std::thread::scope(|scope| {
            let supervisor = world.clone();
            scope.spawn(move || {
                while let Some(spawned) = supervisor.next_spawns() {
                    for (id, job) in spawned {
                        let task = supervisor.task(id);
                        let started = std::thread::Builder::new()
                            .name("ignite-eval-task".to_string())
                            .stack_size(Evaluator::STACK_BYTES)
                            .spawn_scoped(scope, move || {
                                let mut ev = Evaluator::new(modules, fuel);
                                ev.time_limit = time_limit;
                                ev.deadline = deadline;
                                ev.on_stack = true;
                                let world = task.clone();
                                guarded(&world, move || ev.run_task(task, job));
                            });
                        if let Err(error) = started {
                            supervisor.end_with(EvalError::Failed {
                                report: format!("evaluation failed: a task cannot start: {error}"),
                            });
                        }
                    }
                }
            });
            self.world = Some(world.clone());
            // A panic of the body ends the model first, so the task threads
            // unwind and the scope can join them; then it goes on as before.
            let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(self)))
            {
                Ok(result) => result,
                Err(panic) => {
                    world.end_with(panicked(panic.as_ref()));
                    std::panic::resume_unwind(panic)
                }
            };
            self.world = None;
            let ended = match result {
                Ok(_) | Err(Flow::Return(_)) => Ok(()),
                Err(Flow::Error(error)) => Err(error),
                Err(Flow::Break | Flow::Continue) => Err(EvalError::Failed {
                    report: "evaluation failed: `break` or `continue` outside a loop".to_string(),
                }),
            };
            world.end(ended, self.units_left);
            world.outcome()
        });
        self.units_left = units;
        match outcome {
            Some(Ok(())) => Ok(Value::Unit),
            Some(Err(error)) => Err(Flow::Error(error)),
            None => Err(internal("the test ended without an outcome")),
        }
    }

    /// Run the spawned task of `world` that calls `job`, once it holds the
    /// turn. A failure ends only the task, as on the targets; a limit or
    /// an operation the evaluator has no rule for ends the test.
    fn run_task(&mut self, world: World, job: Job) {
        let Ok(units) = world.begin() else {
            return;
        };
        self.units_left = units;
        self.world = Some(world.clone());
        self.frames.push(Frame::default());
        let result = match job.call {
            Call::Fn(q) => self.call_fn(&q, job.args),
            Call::Role { protocol, role } => self.call_role(&protocol, &role, job.args),
            Call::Value(f) => self.apply(f, job.args),
        };
        self.frames.clear();
        self.stack.clear();
        self.world = None;
        let ended = match result {
            Ok(_) | Err(Flow::Return(_)) => Ok(()),
            Err(Flow::Error(EvalError::Failed { report }))
                if !report.starts_with("evaluation failed: ") =>
            {
                Ok(())
            }
            Err(Flow::Error(error)) => Err(error),
            Err(Flow::Break | Flow::Continue) => Err(EvalError::Failed {
                report: "evaluation failed: `break` or `continue` outside a loop".to_string(),
            }),
        };
        world.end(ended, self.units_left);
    }

    /// Call the role `role` of the protocol `protocol` with `args` by
    /// position: the body of the role's fn in the protocol declaration.
    pub(super) fn call_role(
        &mut self,
        protocol: &QualName,
        role: &str,
        args: Vec<Value>,
    ) -> Eval<Value> {
        let module = self
            .modules
            .get(&protocol.module)
            .ok_or_else(|| internal(format!("module `{}` is not loaded", protocol.module)))?;
        let decl = module
            .program
            .decls
            .iter()
            .find_map(|decl| match decl {
                Decl::Protocol(p) if p.name.name == protocol.name => {
                    p.fns.iter().find(|f| f.name.name == role)
                }
                _ => None,
            })
            .ok_or_else(|| internal(format!("role `{protocol}.{role}` is not declared")))?;
        if decl.params.len() != args.len() {
            return Err(internal(format!(
                "role `{protocol}.{role}` takes {} arguments, {} given",
                decl.params.len(),
                args.len()
            )));
        }
        self.tick()?;
        self.enter()?;
        self.stack.push(QualName::new(
            protocol.module.clone(),
            format!("{}.{role}", protocol.name),
        ));
        let mut frame = Frame::default();
        for (param, value) in decl.params.iter().zip(args) {
            frame.locals.push((param.name.name.clone(), value));
        }
        self.frames.push(frame);
        let cx = Cx {
            module,
            system: None,
        };
        let result = self.block_body(&cx, &decl.body);
        self.frames.pop();
        self.stack.pop();
        match result {
            Ok(value) | Err(Flow::Return(value)) => Ok(value),
            Err(other) => Err(other),
        }
    }

    // ---- the model of the substrate (docs/slice.md 33.5) ----------------

    /// A static builtin of the model under a test.
    pub(super) fn model_static(
        &mut self,
        world: &World,
        builtin: Builtin,
        args: Vec<Value>,
    ) -> Eval<Value> {
        match builtin {
            Builtin::StoreNew => {
                let store = world.store_new().map_err(over)?;
                Ok(Value::Handle(store))
            }
            Builtin::SettingOf => {
                let setting = world
                    .capability(HandleKind::Setting, Value::Tuple(args))
                    .map_err(over)?;
                Ok(Value::Handle(setting))
            }
            Builtin::CommandLineOf | Builtin::EnvironmentOf => {
                let kind = if builtin == Builtin::CommandLineOf {
                    HandleKind::CommandLine
                } else {
                    HandleKind::Environment
                };
                let value = args
                    .into_iter()
                    .next()
                    .ok_or_else(|| internal("missing argument"))?;
                Ok(Value::Handle(world.capability(kind, value).map_err(over)?))
            }
            Builtin::PipePair => {
                let (first, second) = world.pipe_pair().map_err(over)?;
                Ok(Value::Tuple(vec![
                    Value::Handle(first),
                    Value::Handle(second),
                ]))
            }
            Builtin::ProcessFlush => {
                world.flush().map_err(over)?;
                Ok(Value::Unit)
            }
            Builtin::ChannelBounded => {
                let capacity = match args.into_iter().next() {
                    Some(Value::Int(n)) if n >= 1 => n,
                    _ => {
                        return Err(internal(
                            "`Channel.bounded` with a capacity below 1 has no value in the model",
                        ));
                    }
                };
                let capacity = usize::try_from(capacity).unwrap_or(usize::MAX);
                let (sender, receiver) = world.channel(capacity).map_err(over)?;
                Ok(Value::Tuple(vec![
                    Value::Handle(sender),
                    Value::Handle(receiver),
                ]))
            }
            Builtin::ClockMonotonic => Ok(Value::Quantity(
                QuantityKind::Duration,
                world.now().map_err(over)?,
            )),
            Builtin::ClockSleep => {
                let nanoseconds = match args.into_iter().next() {
                    Some(Value::Quantity(_, n)) => n,
                    _ => return Err(internal("`Clock.sleep` of a non-Duration")),
                };
                world.flush().map_err(over)?;
                if nanoseconds > 0 {
                    let deadline = world.now().map_err(over)?.saturating_add(nanoseconds);
                    self.wait_on(world, Wait::Sleep, Some(deadline))?;
                }
                Ok(Value::Unit)
            }
            other => Err(internal(format!(
                "builtin `{other:?}` has no value at compile time"
            ))),
        }
    }

    /// A method of a handle of the model under a test, on the evaluated
    /// `receiver` with the evaluated `args`.
    pub(super) fn model_method(
        &mut self,
        world: &World,
        builtin: Builtin,
        receiver: Value,
        args: Vec<Value>,
    ) -> Eval<Value> {
        let handle = match &receiver {
            Value::Handle(handle) => handle.clone(),
            other => {
                return Err(internal(format!(
                    "builtin `{builtin:?}` on `{}` has no value in the model",
                    other.debug_text()
                )));
            }
        };
        let mut args = args.into_iter();
        let mut next = || args.next().ok_or_else(|| internal("missing argument"));
        let int = |value: Value| match value {
            Value::Int(n) => Ok(n),
            other => Err(internal(format!("`{}` is not an Int", other.debug_text()))),
        };
        let id = handle.id();
        match (builtin, handle.kind()) {
            (Builtin::StoreNextId, HandleKind::Store) => {
                Ok(Value::Int(world.store_next_id(id).map_err(over)?))
            }
            (Builtin::StoreGet, HandleKind::Store) => {
                let key = int(next()?)?;
                let found = world.store_get(id, key).map_err(over)?;
                self.alloc(found.as_ref().map_or(0, Value::nodes) + 1)?;
                Ok(Value::option(found))
            }
            (Builtin::StorePut, HandleKind::Store) => {
                let key = int(next()?)?;
                world.store_put(id, key, next()?).map_err(over)?;
                Ok(Value::Unit)
            }
            (Builtin::StoreRemove, HandleKind::Store) => {
                let key = int(next()?)?;
                Ok(Value::option(world.store_remove(id, key).map_err(over)?))
            }
            (Builtin::StoreValues, HandleKind::Store) => {
                let values = world.store_values(id).map_err(over)?;
                self.alloc(values.iter().map(Value::nodes).sum::<u64>() + 1)?;
                Ok(Value::List(values))
            }
            (Builtin::StoreUpdate, HandleKind::Store) => {
                let key = int(next()?)?;
                let f = next()?;
                match world.store_get(id, key).map_err(over)? {
                    None => Ok(Value::none()),
                    Some(old) => {
                        let new = self.apply(f, vec![old])?;
                        world.store_put(id, key, new.clone()).map_err(over)?;
                        Ok(Value::some(new))
                    }
                }
            }
            (Builtin::StoreUpsert, HandleKind::Store) => {
                let key = int(next()?)?;
                let f = next()?;
                let old = world.store_get(id, key).map_err(over)?;
                let new = self.apply(f, vec![Value::option(old)])?;
                world.store_put(id, key, new.clone()).map_err(over)?;
                Ok(new)
            }
            (Builtin::SettingValue, HandleKind::Setting) => {
                match world.capability_value(id).map_err(over)? {
                    Value::Tuple(mut items) if items.len() == 2 => Ok(items.remove(1)),
                    _ => Err(internal("a setting without its value")),
                }
            }
            (Builtin::CommandLineArguments, HandleKind::CommandLine) => {
                let arguments = world.capability_value(id).map_err(over)?;
                self.alloc(arguments.nodes())?;
                Ok(arguments)
            }
            (Builtin::EnvironmentGet, HandleKind::Environment) => {
                let (Value::List(entries), Value::Text(name)) =
                    (world.capability_value(id).map_err(over)?, next()?)
                else {
                    return Err(internal("an environment without its entries"));
                };
                // `Environment.of`: split at the first `=`, the last entry
                // of a name winning, one without `=` or a name skipped.
                let found = entries.iter().rev().find_map(|entry| match entry {
                    Value::Text(entry) => entry
                        .split_once('=')
                        .filter(|(key, _)| !key.is_empty() && *key == name)
                        .map(|(_, value)| Value::Text(value.to_string())),
                    _ => None,
                });
                Ok(Value::option(found))
            }
            (Builtin::PipeStreamRead | Builtin::StreamRead, HandleKind::PipeStream) => {
                let maximum = int(next()?)?;
                let deadline = match world.read_start(id).map_err(over)? {
                    Ok(deadline) => deadline,
                    Err(message) => return Ok(io_failed(message)),
                };
                loop {
                    match world.read_try(id, maximum, deadline).map_err(over)? {
                        Some(Read::Data(bytes)) => {
                            self.alloc(bytes.len() as u64 + 2)?;
                            return Ok(Value::ok(input("Data", vec![Value::bytes(&bytes)])));
                        }
                        Some(Read::Eof) => return Ok(Value::ok(input("Eof", Vec::new()))),
                        None => self.wait_on(world, Wait::Read(id), deadline)?,
                    }
                }
            }
            (Builtin::PipeStreamWriteAll | Builtin::StreamWriteAll, HandleKind::PipeStream) => {
                let data = data_of(next()?)?;
                Ok(io_unit(world.write(id, &data).map_err(over)?))
            }
            (
                Builtin::PipeStreamWriteUrgent | Builtin::StreamWriteUrgent,
                HandleKind::PipeStream,
            ) => {
                let data = data_of(next()?)?;
                Ok(io_unit(world.write_urgent(id, &data).map_err(over)?))
            }
            (Builtin::PipeStreamClose | Builtin::StreamClose, HandleKind::PipeStream) => {
                Ok(io_unit(world.close(id).map_err(over)?))
            }
            (
                Builtin::PipeStreamSetIdleTimeout | Builtin::StreamSetIdleTimeout,
                HandleKind::PipeStream,
            ) => {
                let ms = int(next()?)?;
                world.set_idle_timeout(id, ms).map_err(over)?;
                Ok(Value::ok(Value::Unit))
            }
            (
                Builtin::PipeStreamUrgentPending | Builtin::StreamUrgentPending,
                HandleKind::PipeStream,
            ) => Ok(Value::ok(Value::Bool(
                world.urgent_pending(id).map_err(over)?,
            ))),
            (Builtin::PipeStreamReceivedBatches, HandleKind::PipeStream) => {
                Ok(Value::Int(world.received_batches(id).map_err(over)?))
            }
            (Builtin::SenderSend, HandleKind::Sender) => {
                let mut value = next()?;
                loop {
                    match world.send(id, value, true).map_err(over)? {
                        Ok(sent) => return Ok(Value::Bool(sent)),
                        Err(back) => {
                            value = back;
                            self.wait_on(world, Wait::Send(id), None)?;
                        }
                    }
                }
            }
            (Builtin::SenderTrySend, HandleKind::Sender) => {
                match world.send(id, next()?, false).map_err(over)? {
                    Ok(sent) => Ok(Value::Bool(sent)),
                    Err(_) => Ok(Value::Bool(false)),
                }
            }
            (Builtin::SenderClose, HandleKind::Sender)
            | (Builtin::ReceiverClose, HandleKind::Receiver) => {
                world.close_channel(id).map_err(over)?;
                Ok(Value::Unit)
            }
            (Builtin::ReceiverClosed, HandleKind::Receiver) => {
                Ok(Value::Bool(world.channel_closed(id).map_err(over)?))
            }
            (Builtin::ReceiverReceive | Builtin::ReceiverReceiveWithin, HandleKind::Receiver) => {
                let deadline = if builtin == Builtin::ReceiverReceiveWithin {
                    let nanoseconds = match next()? {
                        Value::Quantity(_, n) => n,
                        _ => return Err(internal("`receive_within` of a non-Duration")),
                    };
                    Some(
                        world
                            .now()
                            .map_err(over)?
                            .saturating_add(nanoseconds.max(0)),
                    )
                } else {
                    None
                };
                let mut first = true;
                loop {
                    match world.receive(id, deadline, first).map_err(over)? {
                        Some(value) => {
                            let nodes = value.as_ref().map_or(0, Value::nodes);
                            self.alloc(nodes + 1)?;
                            return Ok(Value::option(value));
                        }
                        None => {
                            first = false;
                            self.wait_on(world, Wait::Receive(id), deadline)?;
                        }
                    }
                }
            }
            (builtin, kind) => Err(internal(format!(
                "builtin `{builtin:?}` on a `{}` has no value in the model",
                kind.name()
            ))),
        }
    }

    /// `spawn operand` under a test: the callee and the arguments are
    /// evaluated by the spawning task, as the Rust target does for a named
    /// fn or role, and the task is registered with the model; it runs once
    /// it holds the turn. The target evaluates a fn value's call inside
    /// the task, so that form is taken only when every argument is a name.
    pub(super) fn spawn(&mut self, cx: &Cx<'p>, world: &World, operand: &Expr) -> Eval<Value> {
        let refused = || internal("`spawn` of this operand has no value in the model");
        let names = |args: &[Expr]| args.iter().all(|arg| matches!(arg.kind, ExprKind::Name(_)));
        let callee = cx.types().callee(operand).cloned();
        let (call, args) = match (&operand.kind, callee) {
            (ExprKind::Call { args, .. }, Some(Callee::Fn(q))) => {
                (Call::Fn(q), self.args(cx, operand, args)?)
            }
            (ExprKind::Method { args, .. }, Some(Callee::Fn(q))) if q.name.contains('.') => {
                (Call::Fn(q), self.args(cx, operand, args)?)
            }
            (ExprKind::Method { receiver, args, .. }, Some(Callee::Fn(q))) => {
                let mut values = vec![self.expr(cx, receiver)?];
                values.extend(self.args(cx, operand, args)?);
                (Call::Fn(q), values)
            }
            (
                ExprKind::Call { args, .. } | ExprKind::Method { args, .. },
                Some(Callee::Role { protocol, role }),
            ) => (Call::Role { protocol, role }, self.args(cx, operand, args)?),
            (ExprKind::Call { callee, args }, Some(Callee::Local)) if names(args) => {
                let f = self.expr(cx, callee)?;
                (Call::Value(f), self.exprs(cx, args)?)
            }
            (ExprKind::Method { receiver, args, .. }, Some(Callee::Local)) if names(args) => {
                let f = self.expr(cx, receiver)?;
                (Call::Value(f), self.exprs(cx, args)?)
            }
            (
                ExprKind::Call { args, .. } | ExprKind::Method { args, .. },
                Some(Callee::Const(q)),
            ) if names(args) => {
                let f = self.constant(&q)?;
                (Call::Value(f), self.exprs(cx, args)?)
            }
            (ExprKind::Method { receiver, args, .. }, Some(Callee::FieldFn { field, .. }))
                if names(args) && matches!(receiver.kind, ExprKind::Name(_)) =>
            {
                let record = self.expr(cx, receiver)?;
                let f = self.field_of(cx, record, &field)?;
                (Call::Value(f), self.exprs(cx, args)?)
            }
            _ => return Err(refused()),
        };
        match world.spawn(Job { call, args }).map_err(over)? {
            Ok(()) => Ok(Value::Unit),
            Err(reason) => Err(internal(reason)),
        }
    }

    /// Block this task of `world` on `wait` until it is ready or the
    /// virtual time reaches `deadline`; the fuel left goes with the turn.
    fn wait_on(&mut self, world: &World, wait: Wait, deadline: Option<i64>) -> Eval<()> {
        let mut units = self.units_left;
        world.block(wait, deadline, &mut units).map_err(over)?;
        self.units_left = units;
        Ok(())
    }
}

/// The model's flow of a test that is over: the task unwinds.
pub(super) fn over(_: Over) -> Flow {
    internal("the test is over")
}

/// `Err(IoError(message))`, exit code 1 as `IoError.new` gives.
fn io_failed(message: String) -> Value {
    Value::err(Value::IoError {
        message,
        exit_code: 1,
    })
}

/// The `Result<Unit, IoError>` of a model operation.
fn io_unit(done: Result<(), String>) -> Value {
    match done {
        Ok(()) => Value::ok(Value::Unit),
        Err(message) => io_failed(message),
    }
}

/// The `Input` variant `variant`.
fn input(variant: &str, fields: Vec<Value>) -> Value {
    Value::Enum {
        name: QualName::builtin("Input"),
        variant: variant.to_string(),
        fields,
    }
}

/// The bytes of a `Bytes` value.
fn data_of(value: Value) -> Eval<Vec<u8>> {
    match value {
        Value::List(items) => bytes_of(&items),
        other => Err(internal(format!("`{}` is not Bytes", other.debug_text()))),
    }
}

/// The statics that have a value in the model of a test (33.5).
pub(super) fn is_model_static(builtin: Builtin) -> bool {
    matches!(
        builtin,
        Builtin::StoreNew
            | Builtin::SettingOf
            | Builtin::CommandLineOf
            | Builtin::EnvironmentOf
            | Builtin::PipePair
            | Builtin::ProcessFlush
            | Builtin::ChannelBounded
            | Builtin::ClockMonotonic
            | Builtin::ClockSleep
    )
}

/// The handle methods that have a value in the model of a test (33.5).
pub(super) fn is_model_method(builtin: Builtin) -> bool {
    matches!(
        builtin,
        Builtin::StoreNextId
            | Builtin::StoreGet
            | Builtin::StorePut
            | Builtin::StoreRemove
            | Builtin::StoreValues
            | Builtin::StoreUpdate
            | Builtin::StoreUpsert
            | Builtin::SettingValue
            | Builtin::CommandLineArguments
            | Builtin::EnvironmentGet
            | Builtin::PipeStreamRead
            | Builtin::PipeStreamWriteAll
            | Builtin::PipeStreamWriteUrgent
            | Builtin::PipeStreamClose
            | Builtin::PipeStreamSetIdleTimeout
            | Builtin::PipeStreamUrgentPending
            | Builtin::PipeStreamReceivedBatches
            | Builtin::StreamRead
            | Builtin::StreamWriteAll
            | Builtin::StreamWriteUrgent
            | Builtin::StreamClose
            | Builtin::StreamSetIdleTimeout
            | Builtin::StreamUrgentPending
            | Builtin::SenderSend
            | Builtin::SenderTrySend
            | Builtin::SenderClose
            | Builtin::ReceiverReceive
            | Builtin::ReceiverReceiveWithin
            | Builtin::ReceiverClosed
            | Builtin::ReceiverClose
    )
}

/// Run `task`, the body of a task's thread. A panic in it ends the test
/// with a failure (`evaluation failed: ...`), so the other tasks unwind
/// instead of waiting for a turn that never passes.
fn guarded(world: &World, task: impl FnOnce()) {
    if let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(task)) {
        world.end_with(panicked(panic.as_ref()));
    }
}

/// The failure of a panic with the payload `panic`.
fn panicked(panic: &(dyn std::any::Any + Send)) -> EvalError {
    let message = panic
        .downcast_ref::<&str>()
        .map(|text| text.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic without a message".to_string());
    EvalError::Failed {
        report: format!("evaluation failed: a task of the model panicked: {message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A task thread that panics while it holds the turn ends the test
    /// with a failure; a task waiting for the turn then unwinds.
    #[test]
    fn a_panicking_task_thread_ends_the_test_with_a_failure() {
        let world = World::new(1_000);
        let job = Job {
            call: Call::Value(Value::Unit),
            args: Vec::new(),
        };
        assert!(matches!(world.spawn(job), Ok(Ok(()))));
        let task = world.task(1);
        let thread = std::thread::spawn(move || guarded(&task, || panic!("boom")));
        let _ = thread.join();
        match world.outcome().0 {
            Some(Err(EvalError::Failed { report })) => assert_eq!(
                report,
                "evaluation failed: a task of the model panicked: boom"
            ),
            other => panic!("expected the failure of the panic, found {other:?}"),
        }
        let mut units = 1_000;
        assert!(world.block(Wait::Sleep, None, &mut units).is_err());
    }
}
