//! The interpreter (`docs/slice.md` section 15.3, D5): evaluates a property
//! expression with the fns, constants and field defaults it reaches in any
//! module, under fuel. Semantics are those of the targets: T6 overflow,
//! 14.8 failures, 14.3 map order, 14.1 quantity arithmetic, slice-9
//! structural `==`. Owner: P2.
//!
//! Values have value semantics: a `let` copies, a `var` is a place, and the
//! mutating builtins (`push`, `append`, `clear`, `put`, `remove`) and
//! assignments write through a place path rooted in a local (13.11 a).
//! Evaluations run on a thread with a large stack, so that a
//! deeply recursive fn meets the depth limit ([`Limit::Depth`]) instead of
//! overflowing the compiler's stack.

use std::cmp::Ordering;
use std::collections::HashMap;

use ignite_syntax::SourceFile;
use ignite_syntax::ast::{
    Arm, BinaryOp, Block, BraceEntry, ConstDecl, Decl, EnumDecl, Expr, ExprKind, FnDecl, Pattern,
    PatternKind, Program, RecordDecl, Stmt, StmtKind, UnaryOp,
};
use ignite_types::{
    AssertCompare, BraceKind, Builtin, Callee, EXPOSURE_VARIANTS, FailureOp,
    LAUNCHER_DEFAULT_MAXIMUM_CHILDREN, MemberInfo, PropertySource, QualName, QuantityKind, Show,
    TCP_LISTENER_DEFAULT_BACKLOG, TCP_LISTENER_DEFAULT_EXPOSURE, Ty, TypedProgram, field_default,
};

use crate::{Module, Modules, Value};

/// The limits of one property expression's evaluation (`docs/slice.md`
/// section 15.3, O14): `units` of fuel, one per expression evaluated, per
/// loop iteration and per call; `nodes` the largest value (list items, map
/// entries, record fields and payloads, counted recursively). Both default
/// to 1,000,000; `--fuel N` sets `units`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fuel {
    pub units: u64,
    pub nodes: u64,
}

impl Fuel {
    pub const DEFAULT_LIMIT: u64 = 1_000_000;

    /// The default limits with `units` replaced.
    pub fn with_units(units: u64) -> Fuel {
        Fuel {
            units,
            ..Fuel::default()
        }
    }
}

impl Default for Fuel {
    fn default() -> Fuel {
        Fuel {
            units: Fuel::DEFAULT_LIMIT,
            nodes: Fuel::DEFAULT_LIMIT,
        }
    }
}

/// Which limit an evaluation exceeded (E115).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    /// The fuel limit, in units.
    Fuel(u64),
    /// The value-size limit, in nodes.
    Nodes(u64),
    /// The call-depth limit, in nested fn and closure calls
    /// ([`Evaluator::MAX_DEPTH`]).
    Depth(usize),
}

/// Why an evaluation produced no value. Each maps to one diagnostic at the
/// property expression (`docs/slice.md` section 15.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvalError {
    /// E115: the limit that was exceeded and the fn that was running
    /// (the qualified name, or the property expression's own description
    /// when no fn was).
    Limit { limit: Limit, running: String },
    /// E117: the 14.8 report text, `<op> failed at
    /// <file>:<line>:<column>: <detail>`.
    Failed { report: String },
    /// E118: a value the model cannot encode (a closure, a fn value that
    /// is no named top-level fn, a handle not named by a member), by its
    /// description.
    Unencodable { what: String },
}

/// Non-local exits of the evaluation: an error, or the control flow of
/// `return`, `?`, `break` and `continue` on its way to the construct that
/// catches it.
#[derive(Debug)]
enum Flow {
    Error(EvalError),
    Return(Value),
    Break,
    Continue,
}

impl From<EvalError> for Flow {
    fn from(error: EvalError) -> Flow {
        Flow::Error(error)
    }
}

type Eval<T> = Result<T, Flow>;

/// A failure of the evaluator itself: an expression the type checker
/// accepted but the interpreter has no rule for. Reported as E117 so the
/// compiler never panics on it.
fn internal(what: impl std::fmt::Display) -> Flow {
    Flow::Error(EvalError::Failed {
        report: format!("evaluation failed: {what}"),
    })
}

/// The module an expression belongs to and the system whose members its
/// bare names denote (a property expression inside `system S { .. }`).
#[derive(Clone, Copy)]
struct Cx<'p> {
    module: &'p Module<'p>,
    system: Option<&'p str>,
}

impl<'p> Cx<'p> {
    fn types(&self) -> &'p TypedProgram {
        self.module.types
    }

    fn source(&self) -> &'p SourceFile {
        self.module.source
    }

    fn program(&self) -> &'p Program {
        self.module.program
    }
}

/// The locals of one fn or closure activation, innermost scope last.
#[derive(Debug, Default)]
struct Frame {
    locals: Vec<(String, Value)>,
    scopes: Vec<usize>,
}

/// One step of a place path (`x.items[2].0`), from the local it is rooted in.
enum Step {
    Field(String),
    /// The index and the byte offset of the index expression, for its
    /// `index` failure.
    Index(i64, usize),
    Tuple(usize),
}

/// Evaluates expressions of any module in `modules`.
#[derive(Debug)]
pub struct Evaluator<'p> {
    modules: &'p Modules<'p>,
    fuel: Fuel,
    /// Fuel left in the current evaluation.
    units_left: u64,
    /// Nodes allocated since the last live-size check; when it passes the
    /// node limit the live values are counted exactly.
    allocated: u64,
    frames: Vec<Frame>,
    /// The fns running, outermost first (closures count as their creator).
    stack: Vec<QualName>,
    /// Closure literals by module and byte offset, indexed on first use.
    closures: HashMap<String, HashMap<usize, &'p Expr>>,
    /// A model operation may share its stack across independent evaluations.
    on_stack: bool,
}

impl<'p> Evaluator<'p> {
    /// The most nested fn and closure calls an evaluation may hold.
    pub const MAX_DEPTH: usize = 10_000;

    /// Stack of the evaluation thread: room for [`Evaluator::MAX_DEPTH`]
    /// nested calls of the interpreter's own frames.
    const STACK_BYTES: usize = 512 << 20;

    pub fn new(modules: &'p Modules<'p>, fuel: Fuel) -> Evaluator<'p> {
        Evaluator {
            modules,
            fuel,
            units_left: fuel.units,
            allocated: 0,
            frames: Vec::new(),
            stack: Vec::new(),
            closures: HashMap::new(),
            on_stack: false,
        }
    }

    pub fn modules(&self) -> &'p Modules<'p> {
        self.modules
    }

    pub fn fuel(&self) -> Fuel {
        self.fuel
    }

    /// The units of fuel the last evaluation or call used (invariant 9
    /// measurements, `ignite explain`).
    pub fn units_used(&self) -> u64 {
        self.fuel.units - self.units_left
    }

    /// The value of `expr`, an expression of the module `program` was typed
    /// from (its AST is `modules().get(&program.module)`), evaluated with a
    /// fresh fuel budget. `program.effects(expr)` is asserted empty: the
    /// type checker reports E116 for anything else.
    pub fn evaluate(&mut self, program: &TypedProgram, expr: &Expr) -> Result<Value, EvalError> {
        if let Some(effects) = program.effects(expr)
            && !effects.is_empty()
        {
            debug_assert!(false, "effectful property expression reached the evaluator");
            return Err(EvalError::Failed {
                report: format!(
                    "evaluation failed: effectful property expression ({})",
                    effects.entries().join(", ")
                ),
            });
        }
        let Some(module) = self.modules.get(&program.module) else {
            return Err(EvalError::Failed {
                report: format!(
                    "evaluation failed: module `{}` is not loaded",
                    program.module
                ),
            });
        };
        let system = enclosing_system(module, expr.span.start);
        let cx = Cx { module, system };
        self.run(|ev| ev.expr(&cx, expr))
    }

    /// Call the top-level fn `fn_name` of any loaded module with `args` by
    /// position, with a fresh fuel budget; the errors of
    /// [`Evaluator::evaluate`]. Used by the resolver for `admits` and
    /// `includes` (15.6).
    pub fn call(&mut self, fn_name: &QualName, args: Vec<Value>) -> Result<Value, EvalError> {
        let fn_name = fn_name.clone();
        self.run(move |ev| ev.call_fn(&fn_name, args))
    }

    /// One evaluation: fresh budgets on the evaluation stack, flows
    /// mapped to errors and the result checked for encodability (E118).
    fn run(
        &mut self,
        f: impl FnOnce(&mut Evaluator<'p>) -> Eval<Value> + Send,
    ) -> Result<Value, EvalError> {
        self.units_left = self.fuel.units;
        self.allocated = 0;
        self.frames.clear();
        self.frames.push(Frame::default());
        self.stack.clear();
        let result = self.with_stack(f);
        self.frames.clear();
        self.stack.clear();
        match result {
            Ok(value) | Err(Flow::Return(value)) => {
                value
                    .encodable()
                    .map_err(|what| EvalError::Unencodable { what })?;
                Ok(value)
            }
            Err(Flow::Error(error)) => Err(error),
            Err(Flow::Break | Flow::Continue) => Err(EvalError::Failed {
                report: "evaluation failed: `break` or `continue` outside a loop".to_string(),
            }),
        }
    }

    /// Reuse one scoped worker for the evaluations belonging to an operation.
    /// `run` still resets every expression's fuel, allocations and call frames.
    pub(crate) fn with_stack<R: Send>(&mut self, f: impl FnOnce(&mut Self) -> R + Send) -> R {
        if self.on_stack {
            return f(self);
        }
        std::thread::scope(|scope| {
            let handle = std::thread::Builder::new()
                .name("ignite-eval".to_string())
                .stack_size(Evaluator::STACK_BYTES)
                .spawn_scoped(scope, || {
                    self.on_stack = true;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self)));
                    self.on_stack = false;
                    result.unwrap_or_else(|panic| std::panic::resume_unwind(panic))
                })
                .expect("spawn the evaluation thread");
            handle
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
        })
    }

    // ---- budgets --------------------------------------------------------

    /// One unit of fuel: an expression, a loop iteration or a call.
    fn tick(&mut self) -> Eval<()> {
        if self.units_left == 0 {
            return Err(self.limit(Limit::Fuel(self.fuel.units)));
        }
        self.units_left -= 1;
        Ok(())
    }

    /// `nodes` new nodes were built. Past the node limit the live values are
    /// counted exactly and compared with it.
    fn alloc(&mut self, nodes: u64) -> Eval<()> {
        self.allocated = self.allocated.saturating_add(nodes);
        if self.allocated > self.fuel.nodes {
            let live: u64 = self
                .frames
                .iter()
                .flat_map(|frame| &frame.locals)
                .map(|(_, value)| value.nodes())
                .sum();
            if live > self.fuel.nodes {
                return Err(self.limit(Limit::Nodes(self.fuel.nodes)));
            }
            self.allocated = live;
        }
        Ok(())
    }

    fn limit(&self, limit: Limit) -> Flow {
        Flow::Error(EvalError::Limit {
            limit,
            running: self.running(),
        })
    }

    /// The fn running, for E115.
    fn running(&self) -> String {
        match self.stack.last() {
            Some(name) => name.to_string(),
            None => "the property expression".to_string(),
        }
    }

    // ---- frames and scopes ----------------------------------------------

    fn frame(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("an evaluation has a frame")
    }

    fn bind(&mut self, name: &str, value: Value) {
        self.frame().locals.push((name.to_string(), value));
    }

    fn lookup(&self, name: &str) -> Option<&Value> {
        self.frames
            .last()?
            .locals
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v)
    }

    fn lookup_mut(&mut self, name: &str) -> Option<&mut Value> {
        self.frames
            .last_mut()?
            .locals
            .iter_mut()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v)
    }

    fn push_scope(&mut self) {
        let frame = self.frame();
        frame.scopes.push(frame.locals.len());
    }

    fn pop_scope(&mut self) {
        let frame = self.frame();
        if let Some(len) = frame.scopes.pop() {
            frame.locals.truncate(len);
        }
    }

    /// Whether `name` is a local of the current frame and not a member of
    /// the system `cx` is in (a member name wins over a local, as in the
    /// type checker).
    fn is_local(&self, cx: &Cx<'p>, name: &str) -> bool {
        self.lookup(name).is_some() && member_of(cx, name).is_none()
    }

    // ---- calls ----------------------------------------------------------

    /// Call the fn `q` with `args` by position.
    fn call_fn(&mut self, q: &QualName, args: Vec<Value>) -> Eval<Value> {
        let module = self
            .modules
            .get(&q.module)
            .ok_or_else(|| internal(format!("module `{}` is not loaded", q.module)))?;
        let decl = find_fn(module.program, &q.name)
            .ok_or_else(|| internal(format!("fn `{q}` is not declared")))?;
        if decl.params.len() != args.len() {
            return Err(internal(format!(
                "fn `{q}` takes {} arguments, {} given",
                decl.params.len(),
                args.len()
            )));
        }
        self.tick()?;
        self.enter()?;
        self.stack.push(q.clone());
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

    /// Call the closure created at `offset` in `module` with `captures`.
    fn call_closure(
        &mut self,
        module: &str,
        offset: usize,
        captures: Vec<(String, Value)>,
        args: Vec<Value>,
    ) -> Eval<Value> {
        let m = self
            .modules
            .get(module)
            .ok_or_else(|| internal(format!("module `{module}` is not loaded")))?;
        let expr = self
            .closure_expr(module, offset)
            .ok_or_else(|| internal(format!("no closure at {module}:{offset}")))?;
        let ExprKind::Closure(closure) = &expr.kind else {
            return Err(internal("closure value names no closure literal"));
        };
        if closure.params.len() != args.len() {
            return Err(internal(format!(
                "closure takes {} arguments, {} given",
                closure.params.len(),
                args.len()
            )));
        }
        self.tick()?;
        self.enter()?;
        let mut frame = Frame {
            locals: captures,
            scopes: Vec::new(),
        };
        for (param, value) in closure.params.iter().zip(args) {
            frame.locals.push((param.name.name.clone(), value));
        }
        self.frames.push(frame);
        let cx = Cx {
            module: m,
            system: enclosing_system(m, offset),
        };
        let result = self.expr(&cx, &closure.body);
        self.frames.pop();
        match result {
            Ok(value) | Err(Flow::Return(value)) => Ok(value),
            Err(other) => Err(other),
        }
    }

    fn enter(&self) -> Eval<()> {
        if self.frames.len() >= Evaluator::MAX_DEPTH {
            return Err(self.limit(Limit::Depth(Evaluator::MAX_DEPTH)));
        }
        Ok(())
    }

    /// Apply a fn value.
    fn apply(&mut self, f: Value, args: Vec<Value>) -> Eval<Value> {
        match f {
            Value::Fn(q) => self.call_fn(&q, args),
            Value::Closure {
                module,
                offset,
                captures,
            } => self.call_closure(&module, offset, captures, args),
            other => Err(internal(format!(
                "`{}` is not a fn value",
                other.debug_text()
            ))),
        }
    }

    /// The closure literal at `offset` in `module`.
    fn closure_expr(&mut self, module: &str, offset: usize) -> Option<&'p Expr> {
        if !self.closures.contains_key(module) {
            let index = self
                .modules
                .get(module)
                .map(|m| index_closures(m.program))
                .unwrap_or_default();
            self.closures.insert(module.to_string(), index);
        }
        self.closures.get(module)?.get(&offset).copied()
    }

    /// The value of the constant `q`, evaluated in its module.
    fn constant(&mut self, q: &QualName) -> Eval<Value> {
        let module = self
            .modules
            .get(&q.module)
            .ok_or_else(|| internal(format!("module `{}` is not loaded", q.module)))?;
        let decl = find_const(module.program, &q.name)
            .ok_or_else(|| internal(format!("constant `{q}` is not declared")))?;
        let cx = Cx {
            module,
            system: None,
        };
        self.in_fresh_frame(|ev| ev.expr(&cx, &decl.value))
    }

    /// The default value of field `field` of `record` in `module`.
    fn default_value(&mut self, module: &str, record: &str, field: &str) -> Eval<Value> {
        let m = self
            .modules
            .get(module)
            .ok_or_else(|| internal(format!("module `{module}` is not loaded")))?;
        let expr = field_default(m.program, record, field).ok_or_else(|| {
            internal(format!(
                "field `{record}.{field}` of `{module}` has no default"
            ))
        })?;
        let cx = Cx {
            module: m,
            system: None,
        };
        self.in_fresh_frame(|ev| ev.expr(&cx, expr))
    }

    /// Run `f` with an empty frame of locals (a constant, a field default).
    fn in_fresh_frame(&mut self, f: impl FnOnce(&mut Self) -> Eval<Value>) -> Eval<Value> {
        self.enter()?;
        self.frames.push(Frame::default());
        let result = f(self);
        self.frames.pop();
        match result {
            Ok(value) | Err(Flow::Return(value)) => Ok(value),
            Err(other) => Err(other),
        }
    }

    // ---- expressions ----------------------------------------------------

    fn expr(&mut self, cx: &Cx<'p>, e: &Expr) -> Eval<Value> {
        self.tick()?;
        match &e.kind {
            ExprKind::Int(value) => Ok(match cx.types().try_type_of(e) {
                Some(Ty::Byte) => Value::Byte(*value as u8),
                _ => Value::Int(*value),
            }),
            ExprKind::Str(text) => Ok(Value::Text(text.clone())),
            ExprKind::Bool(value) => Ok(Value::Bool(*value)),
            ExprKind::Name(name) => self.name(cx, e, &name.name),
            ExprKind::Record { fields, .. } => {
                let entries: Vec<(&str, &Expr)> = fields
                    .iter()
                    .map(|f| (f.name.name.as_str(), &f.value))
                    .collect();
                self.record_literal(cx, e, &entries, None)
            }
            ExprKind::List(items) => {
                let values = self.exprs(cx, items)?;
                self.alloc(values.len() as u64 + 1)?;
                Ok(Value::List(values))
            }
            ExprKind::Tuple(items) if items.is_empty() => Ok(Value::Unit),
            ExprKind::Tuple(items) => {
                let values = self.exprs(cx, items)?;
                self.alloc(values.len() as u64 + 1)?;
                Ok(Value::Tuple(values))
            }
            ExprKind::Return(value) => {
                let value = match value {
                    Some(value) => self.expr(cx, value)?,
                    None => Value::Unit,
                };
                Err(Flow::Return(value))
            }
            ExprKind::Call { callee, args } => self.call_expr(cx, e, callee, args),
            ExprKind::Method {
                receiver,
                name,
                args,
            } => self.method(cx, e, receiver, &name.name, args),
            ExprKind::Field { receiver, name } => self.field(cx, e, receiver, &name.name),
            ExprKind::Index { receiver, index } => {
                let index = self.expr(cx, index)?;
                let Value::Int(i) = index else {
                    return Err(internal("index is not an Int"));
                };
                let found = self.with_receiver(cx, receiver, |_, list| match list {
                    Value::List(items) => Ok(usize::try_from(i)
                        .ok()
                        .and_then(|i| items.get(i))
                        .cloned()
                        .ok_or(items.len())),
                    _ => Err(internal("indexed value is not a list")),
                })?;
                match found {
                    Ok(item) => Ok(item),
                    Err(len) => Err(self.fail(
                        cx,
                        e,
                        FailureOp::Index,
                        &format!("index {i} out of range for length {len}"),
                    )),
                }
            }
            ExprKind::Try(inner) => self.try_(cx, e, inner),
            ExprKind::Await(inner) => self.expr(cx, inner),
            ExprKind::Spawn(_) => Err(internal("`spawn` has no value at compile time")),
            ExprKind::Unary { op, operand } => self.unary(cx, e, *op, operand),
            ExprKind::Binary { .. } => self.binary_chain(cx, e),
            ExprKind::Match { scrutinee, arms } => self.match_(cx, e, scrutinee, arms),
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => {
                let cond = self.expr(cx, cond)?;
                match (cond, otherwise) {
                    (Value::Bool(true), _) => self.scoped_block(cx, then),
                    (Value::Bool(false), Some(otherwise)) => self.expr(cx, otherwise),
                    (Value::Bool(false), None) => Ok(Value::Unit),
                    _ => Err(internal("`if` condition is not a Bool")),
                }
            }
            ExprKind::Block(block) => self.scoped_block(cx, block),
            ExprKind::Closure(_) => self.closure(cx, e),
            ExprKind::RecordUpdate { fields, base, .. } => {
                let entries: Vec<(&str, &Expr)> = fields
                    .iter()
                    .map(|f| (f.name.name.as_str(), &f.value))
                    .collect();
                self.record_literal(cx, e, &entries, Some(base))
            }
            ExprKind::TupleField { receiver, index } => {
                self.with_receiver(cx, receiver, |_, tuple| match tuple {
                    Value::Tuple(items) => items
                        .get(*index)
                        .cloned()
                        .ok_or_else(|| internal(format!("tuple has no element {index}"))),
                    _ => Err(internal("tuple field of a value that is no tuple")),
                })
            }
            ExprKind::Break => Err(Flow::Break),
            ExprKind::Continue => Err(Flow::Continue),
            ExprKind::Quantity { .. } => cx
                .types()
                .quantity(e)
                .map(|(kind, value)| Value::Quantity(kind, value))
                .ok_or_else(|| internal("quantity literal without a value")),
            ExprKind::NamedArg { value, .. } => self.expr(cx, value),
            ExprKind::Brace { entries } => self.brace(cx, e, entries),
        }
    }

    fn exprs(&mut self, cx: &Cx<'p>, items: &[Expr]) -> Eval<Vec<Value>> {
        items.iter().map(|item| self.expr(cx, item)).collect()
    }

    /// Run `f` on the value of `receiver`: a local is read in place (no
    /// copy), anything else is evaluated first.
    fn with_receiver<T>(
        &mut self,
        cx: &Cx<'p>,
        receiver: &Expr,
        f: impl FnOnce(&Self, &Value) -> Eval<T>,
    ) -> Eval<T> {
        if let ExprKind::Name(name) = &receiver.kind
            && self.is_local(cx, &name.name)
        {
            self.tick()?;
            let value = self.lookup(&name.name).expect("checked local");
            return f(self, value);
        }
        let value = self.expr(cx, receiver)?;
        f(self, &value)
    }

    fn name(&mut self, cx: &Cx<'p>, e: &Expr, name: &str) -> Eval<Value> {
        let types = cx.types();
        if let Some((system, member)) = member_of(cx, name) {
            if let Some(reference) = types.reference(e) {
                return Ok(Value::Reference(reference.clone()));
            }
            let endpoint = types.try_type_of(e) == Some(&ignite_types::endpoint());
            if endpoint && member.kind == Ty::TcpListener {
                return self.endpoint_of(cx, member);
            }
            return Ok(Value::Member {
                system: system.to_string(),
                member: name.to_string(),
            });
        }
        if let Some(reference) = types.reference(e) {
            return Ok(Value::Reference(reference.clone()));
        }
        if let Some(value) = self.lookup(name) {
            return Ok(value.clone());
        }
        match types.callee(e) {
            Some(Callee::Const(q)) => return self.constant(q),
            Some(Callee::Fn(q)) => return Ok(Value::Fn(q.clone())),
            Some(Callee::Variant { enum_, index }) => {
                return self.variant_value(enum_, *index, Vec::new());
            }
            _ => {}
        }
        if name == "Unit" {
            return Ok(Value::Unit);
        }
        Err(internal(format!("unresolved name `{name}`")))
    }

    /// The `std.net.Endpoint` of a `TcpListener` member (14.14 item 5).
    fn endpoint_of(&mut self, cx: &Cx<'p>, member: &'p MemberInfo) -> Eval<Value> {
        let record = self.member_record(cx, member)?;
        let Value::Record { fields, .. } = record else {
            return Err(internal("member is not a record"));
        };
        let pick = |name: &str| {
            fields
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.clone())
                .ok_or_else(|| internal(format!("listener has no `{name}`")))
        };
        let fields = vec![
            ("address".to_string(), pick("address")?),
            ("port".to_string(), pick("port")?),
        ];
        Ok(Value::Record {
            name: ignite_types::endpoint_name(),
            fields,
        })
    }

    /// The record value of a member: every property of its kind, set in
    /// the declaration or defaulted.
    fn member_record(&mut self, cx: &Cx<'p>, member: &'p MemberInfo) -> Eval<Value> {
        let system = cx
            .types()
            .systems()
            .iter()
            .find(|s| s.members.iter().any(|m| std::ptr::eq(m, member)))
            .ok_or_else(|| internal("member of no system"))?;
        let decl = system
            .decl(cx.program())
            .ok_or_else(|| internal("system declaration not found"))?;
        let mut fields = Vec::with_capacity(member.properties.len());
        for (field, source) in &member.properties {
            let value = match source {
                PropertySource::Set(_) => {
                    let expr = member
                        .property(decl, field)
                        .ok_or_else(|| internal(format!("property `{field}` has no expression")))?;
                    self.in_fresh_frame(|ev| ev.expr(cx, expr))?
                }
                PropertySource::Default(q) if q.is_builtin() => match field.as_str() {
                    "exposure" => Value::Enum {
                        name: QualName::builtin("Exposure"),
                        variant: TCP_LISTENER_DEFAULT_EXPOSURE.to_string(),
                        fields: Vec::new(),
                    },
                    "backlog" => Value::Int(TCP_LISTENER_DEFAULT_BACKLOG),
                    "create" => Value::Bool(false),
                    "flag" => Value::Text(String::new()),
                    "maximum_children" => Value::Int(LAUNCHER_DEFAULT_MAXIMUM_CHILDREN),
                    other => return Err(internal(format!("no builtin default for `{other}`"))),
                },
                PropertySource::Default(q) => {
                    let (record, field) = q
                        .name
                        .split_once('.')
                        .ok_or_else(|| internal(format!("default `{q}` names no field")))?;
                    self.default_value(&q.module, record, field)?
                }
                PropertySource::ComponentName => Value::Text(member.name.clone()),
            };
            fields.push((field.clone(), value));
        }
        let name = match &member.kind {
            Ty::Record(q) | Ty::GenericRecord(q, _) => q.clone(),
            Ty::TcpListener => QualName::builtin("TcpListener"),
            Ty::Store(_) => QualName::builtin("Store"),
            other => QualName::builtin(other.to_string()),
        };
        self.alloc(fields.len() as u64 + 1)?;
        Ok(Value::Record { name, fields })
    }

    /// A record literal, a record update (`base` given, evaluated after the
    /// fields) or a record brace literal: fields in source order, then the
    /// defaults of the omitted ones, the result in declaration order.
    fn record_literal(
        &mut self,
        cx: &Cx<'p>,
        e: &Expr,
        entries: &[(&str, &Expr)],
        base: Option<&Expr>,
    ) -> Eval<Value> {
        let q = match cx.types().try_type_of(e) {
            Some(Ty::Record(q) | Ty::GenericRecord(q, _)) => q.clone(),
            Some(other) => return Err(internal(format!("record literal of type `{other}`"))),
            None => return Err(internal("record literal without a type")),
        };
        let module = self
            .modules
            .get(&q.module)
            .ok_or_else(|| internal(format!("module `{}` is not loaded", q.module)))?;
        let decl = find_record(module.program, &q.name)
            .ok_or_else(|| internal(format!("record `{q}` is not declared")))?;
        let mut given: Vec<(String, Value)> = Vec::with_capacity(entries.len());
        for (name, value) in entries {
            let value = self.expr(cx, value)?;
            given.push((name.to_string(), value));
        }
        let base = match base {
            Some(base) => match self.expr(cx, base)? {
                Value::Record { fields, .. } => Some(fields),
                _ => return Err(internal("record update of a value that is no record")),
            },
            None => None,
        };
        let mut fields = Vec::with_capacity(decl.fields.len());
        for field in &decl.fields {
            let name = field.name.name.as_str();
            let value = if let Some(at) = given.iter().position(|(n, _)| n == name) {
                given.swap_remove(at).1
            } else if let Some(from) = base
                .as_ref()
                .and_then(|fields| fields.iter().find(|(n, _)| n == name))
            {
                from.1.clone()
            } else if field.default.is_some() {
                self.default_value(&q.module, &q.name, name)?
            } else {
                return Err(internal(format!("field `{name}` of `{q}` has no value")));
            };
            fields.push((name.to_string(), value));
        }
        self.alloc(fields.len() as u64 + 1)?;
        Ok(Value::Record { name: q, fields })
    }

    /// `{ k = e, .. }` (14.3): a record or a map by the expected type.
    fn brace(&mut self, cx: &Cx<'p>, e: &Expr, entries: &[BraceEntry]) -> Eval<Value> {
        match cx.types().brace_kind(e) {
            Some(BraceKind::Record(_)) => {
                let keys: Vec<String> = entries.iter().map(|entry| dotted(&entry.key)).collect();
                let entries: Vec<(&str, &Expr)> = keys
                    .iter()
                    .zip(entries)
                    .map(|(key, entry)| (key.as_str(), &entry.value))
                    .collect();
                self.record_literal(cx, e, &entries, None)
            }
            Some(BraceKind::Map(..)) => {
                let mut map = Vec::with_capacity(entries.len());
                for entry in entries {
                    let key = match cx.types().brace_key(&entry.key) {
                        Some(reference) => Value::Reference(reference.clone()),
                        None => Value::Text(dotted(&entry.key)),
                    };
                    let value = self.expr(cx, &entry.value)?;
                    map_put(self.modules, &mut map, key, value);
                }
                self.alloc(map.len() as u64 + 1)?;
                Ok(Value::Map(map))
            }
            None => Err(internal("brace literal without a kind")),
        }
    }

    fn closure(&mut self, cx: &Cx<'p>, e: &Expr) -> Eval<Value> {
        let info = cx
            .types()
            .closure(e)
            .ok_or_else(|| internal("closure literal without its typing"))?;
        let mut captures = Vec::with_capacity(info.captures.len());
        for capture in &info.captures {
            let value = self
                .lookup(&capture.name)
                .cloned()
                .ok_or_else(|| internal(format!("capture `{}` is not bound", capture.name)))?;
            captures.push((capture.name.clone(), value));
        }
        Ok(Value::Closure {
            module: cx.module.path.to_string(),
            offset: e.span.start,
            captures,
        })
    }

    fn variant_value(&mut self, enum_: &QualName, index: usize, fields: Vec<Value>) -> Eval<Value> {
        let variant = self
            .variant_name(enum_, index)
            .ok_or_else(|| internal(format!("enum `{enum_}` has no variant {index}")))?;
        self.alloc(fields.len() as u64 + 1)?;
        Ok(Value::Enum {
            name: enum_.clone(),
            variant,
            fields,
        })
    }

    /// The name of variant `index` of `enum_`.
    fn variant_name(&self, enum_: &QualName, index: usize) -> Option<String> {
        if enum_.is_builtin() {
            return builtin_variants(&enum_.name)
                .get(index)
                .map(|v| v.to_string());
        }
        let module = self.modules.get(&enum_.module)?;
        let decl = find_enum(module.program, &enum_.name)?;
        decl.variants.get(index).map(|v| v.name.name.clone())
    }

    // ---- calls and methods ----------------------------------------------

    /// The arguments of a call in source order, reordered to parameter
    /// order when some are named (14.2).
    fn args(&mut self, cx: &Cx<'p>, e: &Expr, args: &[Expr]) -> Eval<Vec<Value>> {
        let values = self.exprs(cx, args)?;
        match cx.types().named_args(e) {
            Some(order) => order
                .iter()
                .map(|&i| {
                    values
                        .get(i)
                        .cloned()
                        .ok_or_else(|| internal("named argument out of range"))
                })
                .collect(),
            None => Ok(values),
        }
    }

    fn call_expr(&mut self, cx: &Cx<'p>, e: &Expr, callee: &Expr, args: &[Expr]) -> Eval<Value> {
        match cx.types().callee(e).cloned() {
            Some(Callee::Fn(q)) => {
                let values = self.args(cx, e, args)?;
                self.call_fn(&q, values)
            }
            Some(Callee::Variant { enum_, index }) => {
                let values = self.exprs(cx, args)?;
                self.variant_value(&enum_, index, values)
            }
            Some(Callee::Builtin(builtin)) => {
                let values = self.exprs(cx, args)?;
                self.builtin_static(cx, e, builtin, values)
            }
            Some(Callee::Local) => {
                let f = self.expr(cx, callee)?;
                let values = self.exprs(cx, args)?;
                self.apply(f, values)
            }
            Some(Callee::Const(q)) => {
                let f = self.constant(&q)?;
                let values = self.exprs(cx, args)?;
                self.apply(f, values)
            }
            Some(Callee::Role { protocol, role }) => Err(internal(format!(
                "protocol role `{protocol}.{role}` has no value at compile time"
            ))),
            Some(Callee::FieldFn { field, .. }) => Err(internal(format!(
                "call of field `{field}` without a receiver"
            ))),
            None => Err(internal("call without a callee")),
        }
    }

    fn method(
        &mut self,
        cx: &Cx<'p>,
        e: &Expr,
        receiver: &Expr,
        name: &str,
        args: &[Expr],
    ) -> Eval<Value> {
        match cx.types().callee(e).cloned() {
            Some(Callee::Fn(q)) if q.name.contains('.') => {
                let values = self.args(cx, e, args)?;
                self.call_fn(&q, values)
            }
            Some(Callee::Fn(q)) => {
                let mut values = vec![self.expr(cx, receiver)?];
                values.extend(self.args(cx, e, args)?);
                self.call_fn(&q, values)
            }
            Some(Callee::Variant { enum_, index }) => {
                let values = self.exprs(cx, args)?;
                self.variant_value(&enum_, index, values)
            }
            Some(Callee::FieldFn { field, .. }) => {
                let record = self.expr(cx, receiver)?;
                let f = self.field_of(cx, record, &field)?;
                let values = self.exprs(cx, args)?;
                self.apply(f, values)
            }
            Some(Callee::Builtin(builtin)) => self.builtin_method(cx, e, builtin, receiver, args),
            Some(Callee::Local) => {
                let f = self.expr(cx, receiver)?;
                let values = self.exprs(cx, args)?;
                self.apply(f, values)
            }
            Some(Callee::Const(q)) => {
                let f = self.constant(&q)?;
                let values = self.exprs(cx, args)?;
                self.apply(f, values)
            }
            Some(Callee::Role { protocol, role }) => Err(internal(format!(
                "protocol role `{protocol}.{role}` has no value at compile time"
            ))),
            None => Err(internal(format!("method `{name}` without a callee"))),
        }
    }

    /// A builtin static `Owner.name(args)` or free fn (`log`, `module`).
    fn builtin_static(
        &mut self,
        cx: &Cx<'p>,
        e: &Expr,
        builtin: Builtin,
        args: Vec<Value>,
    ) -> Eval<Value> {
        let mut args = args.into_iter();
        let mut next = || args.next().ok_or_else(|| internal("missing argument"));
        match builtin {
            Builtin::BytesEmpty | Builtin::ListEmpty => Ok(Value::List(Vec::new())),
            Builtin::MapEmpty => Ok(Value::Map(Vec::new())),
            Builtin::BytesOf => next(),
            Builtin::IntToText => match next()? {
                Value::Int(n) => Ok(Value::Text(n.to_string())),
                _ => Err(internal("Int.to_text of a non-Int")),
            },
            Builtin::IntParse => match next()? {
                Value::Text(text) => Ok(Value::option(
                    Some(text.as_str())
                        .filter(|t| !t.starts_with('+'))
                        .and_then(|t| t.parse::<i64>().ok())
                        .map(Value::Int),
                )),
                _ => Err(internal("Int.parse of a non-Text")),
            },
            Builtin::IoErrorNew => match next()? {
                Value::Text(message) => Ok(Value::IoError {
                    message,
                    exit_code: 1,
                }),
                _ => Err(internal("IoError.new of a non-Text")),
            },
            Builtin::Log => Ok(Value::Unit),
            Builtin::ModuleRecord => {
                let record = cx
                    .types()
                    .module_record(e)
                    .ok_or_else(|| internal("module(..) without its record"))?;
                Ok(Value::Module {
                    module: record.module.clone(),
                    fns: record.fns.clone(),
                })
            }
            other => Err(internal(format!(
                "builtin `{other:?}` has no value at compile time"
            ))),
        }
    }

    fn builtin_method(
        &mut self,
        cx: &Cx<'p>,
        e: &Expr,
        builtin: Builtin,
        receiver: &Expr,
        args: &[Expr],
    ) -> Eval<Value> {
        if is_static(builtin) {
            let values = self.exprs(cx, args)?;
            return self.builtin_static(cx, e, builtin, values);
        }
        match builtin {
            Builtin::ListPush
            | Builtin::ListSet
            | Builtin::ListAppend
            | Builtin::ListClear
            | Builtin::MapPut
            | Builtin::MapRemove => self.mutating(cx, e, builtin, receiver, args),
            Builtin::ListMap => {
                let list = self.expr(cx, receiver)?;
                let f = self.expr(cx, &args[0])?;
                let Value::List(items) = list else {
                    return Err(internal("map of a non-list"));
                };
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(self.apply(f.clone(), vec![item])?);
                }
                self.alloc(out.len() as u64 + 1)?;
                Ok(Value::List(out))
            }
            Builtin::Debug => {
                let text = self.with_receiver(cx, receiver, |_, v| Ok(v.debug_text()))?;
                self.alloc(text.len() as u64 / 64)?;
                Ok(Value::Text(text))
            }
            Builtin::Copy => {
                let value = self.with_receiver(cx, receiver, |_, v| Ok(v.clone()))?;
                self.alloc(value.nodes())?;
                Ok(value)
            }
            _ => {
                let values = self.exprs(cx, args)?;
                let (value, nodes) = self.with_receiver(cx, receiver, |ev, recv| {
                    ev.builtin_value(cx, e, builtin, recv, values)
                })?;
                self.alloc(nodes)?;
                Ok(value)
            }
        }
    }

    /// A builtin method without side effects on its receiver: the result
    /// and the nodes it allocated.
    fn builtin_value(
        &self,
        cx: &Cx<'p>,
        e: &Expr,
        builtin: Builtin,
        recv: &Value,
        args: Vec<Value>,
    ) -> Eval<(Value, u64)> {
        let mut args = args.into_iter();
        let mut next = || args.next().ok_or_else(|| internal("missing argument"));
        let text = |v: &Value| match v {
            Value::Text(t) => Ok(t.clone()),
            _ => Err(internal(format!("`{builtin:?}` on a non-Text"))),
        };
        let plain = |v: Value| Ok((v, 0));
        match (builtin, recv) {
            (Builtin::TextLen, Value::Text(t)) => plain(Value::Int(t.len() as i64)),
            (Builtin::TextIsEmpty, Value::Text(t)) => plain(Value::Bool(t.is_empty())),
            (Builtin::TextTrim, Value::Text(t)) => plain(Value::Text(t.trim().to_string())),
            (Builtin::TextStartsWith, Value::Text(t)) => {
                let prefix = text(&next()?)?;
                plain(Value::Bool(t.starts_with(&prefix)))
            }
            (Builtin::TextStripPrefix, Value::Text(t)) => {
                let prefix = text(&next()?)?;
                plain(Value::option(
                    t.strip_prefix(prefix.as_str())
                        .map(|rest| Value::Text(rest.to_string())),
                ))
            }
            (Builtin::TextConcat, Value::Text(t)) => {
                let other = text(&next()?)?;
                let joined = format!("{t}{other}");
                let nodes = joined.len() as u64 / 64;
                Ok((Value::Text(joined), nodes))
            }
            (Builtin::TextToBytes, Value::Text(t)) => {
                Ok((Value::bytes(t.as_bytes()), t.len() as u64 + 1))
            }
            (Builtin::TextToText, Value::Text(t)) => plain(Value::Text(t.clone())),
            (Builtin::TextToText, Value::Reference(reference)) => {
                plain(Value::Text(reference.text()))
            }
            (Builtin::TextToText, Value::Member { system, member }) => {
                plain(Value::Text(format!("{system}.{member}")))
            }
            (Builtin::IntToTextMethod, Value::Int(n)) => plain(Value::Text(n.to_string())),
            (Builtin::ByteToText, Value::Byte(b)) => plain(Value::Text(b.to_string())),
            (Builtin::BoolToText, Value::Bool(b)) => plain(Value::Text(b.to_string())),
            (Builtin::BytesToText, Value::List(items)) => {
                let bytes = bytes_of(items)?;
                let text = String::from_utf8_lossy(&bytes).into_owned();
                let nodes = text.len() as u64 / 64;
                Ok((Value::Text(text), nodes))
            }
            (Builtin::ByteToInt, Value::Byte(b)) => plain(Value::Int(i64::from(*b))),
            (Builtin::IntToByte, Value::Int(n)) => {
                plain(Value::option(u8::try_from(*n).ok().map(Value::Byte)))
            }
            (
                Builtin::IntWrappingAdd | Builtin::IntWrappingSub | Builtin::IntWrappingMul,
                Value::Int(a),
            ) => {
                let Value::Int(b) = next()? else {
                    return Err(internal("wrapping arithmetic on a non-Int"));
                };
                plain(Value::Int(match builtin {
                    Builtin::IntWrappingAdd => a.wrapping_add(b),
                    Builtin::IntWrappingSub => a.wrapping_sub(b),
                    _ => a.wrapping_mul(b),
                }))
            }
            (
                Builtin::IntCheckedAdd | Builtin::IntCheckedSub | Builtin::IntCheckedMul,
                Value::Int(a),
            ) => {
                let Value::Int(b) = next()? else {
                    return Err(internal("checked arithmetic on a non-Int"));
                };
                let result = match builtin {
                    Builtin::IntCheckedAdd => a.checked_add(b),
                    Builtin::IntCheckedSub => a.checked_sub(b),
                    _ => a.checked_mul(b),
                };
                plain(Value::option(result.map(Value::Int)))
            }
            (Builtin::ListLen, Value::List(items)) => plain(Value::Int(items.len() as i64)),
            (Builtin::ListIsEmpty, Value::List(items)) => plain(Value::Bool(items.is_empty())),
            (Builtin::ListAt, Value::List(items)) => {
                let Value::Int(i) = next()? else {
                    return Err(internal("at of a non-Int"));
                };
                match usize::try_from(i).ok().and_then(|i| items.get(i)) {
                    Some(item) => Ok((item.clone(), item.nodes())),
                    None => Err(self.fail(
                        cx,
                        e,
                        FailureOp::Index,
                        &format!("index {i} out of range for length {}", items.len()),
                    )),
                }
            }
            (Builtin::ListSlice, Value::List(items)) => {
                let (Value::Int(from), Value::Int(to)) = (next()?, next()?) else {
                    return Err(internal("slice of non-Int bounds"));
                };
                let range = usize::try_from(from)
                    .ok()
                    .zip(usize::try_from(to).ok())
                    .filter(|(start, end)| start <= end && *end <= items.len());
                match range {
                    Some((start, end)) => {
                        let slice = items[start..end].to_vec();
                        let nodes = slice.len() as u64 + 1;
                        Ok((Value::List(slice), nodes))
                    }
                    None => Err(self.fail(
                        cx,
                        e,
                        FailureOp::Slice,
                        &format!("slice {from}..{to} out of range for length {}", items.len()),
                    )),
                }
            }
            (Builtin::ListJoin, Value::List(items)) => {
                let separator = text(&next()?)?;
                let parts: Vec<String> = items.iter().map(text).collect::<Eval<_>>()?;
                let joined = parts.join(&separator);
                let nodes = joined.len() as u64 / 64;
                Ok((Value::Text(joined), nodes))
            }
            (Builtin::IoErrorWithExitCode, Value::IoError { message, .. }) => {
                let Value::Int(code) = next()? else {
                    return Err(internal("with_exit_code of a non-Int"));
                };
                plain(Value::IoError {
                    message: message.clone(),
                    exit_code: code,
                })
            }
            (Builtin::MapGet, Value::Map(entries)) => {
                let key = next()?;
                let found = map_find(self.modules, entries, &key)
                    .ok()
                    .map(|i| entries[i].1.clone());
                let nodes = found.as_ref().map_or(0, Value::nodes);
                Ok((Value::option(found), nodes))
            }
            (Builtin::MapContains, Value::Map(entries)) => {
                let key = next()?;
                plain(Value::Bool(map_find(self.modules, entries, &key).is_ok()))
            }
            (Builtin::MapKeys, Value::Map(entries)) => {
                let keys: Vec<Value> = entries.iter().map(|(k, _)| k.clone()).collect();
                let nodes = keys.len() as u64 + 1;
                Ok((Value::List(keys), nodes))
            }
            (Builtin::MapLen, Value::Map(entries)) => plain(Value::Int(entries.len() as i64)),
            (Builtin::DurationToMs, Value::Quantity(QuantityKind::Duration, ns)) => {
                plain(Value::Int(ns / 1_000_000))
            }
            (Builtin::DurationToNs, Value::Quantity(QuantityKind::Duration, ns)) => {
                plain(Value::Int(*ns))
            }
            (Builtin::ByteSizeToInt, Value::Quantity(QuantityKind::ByteSize, bytes)) => {
                plain(Value::Int(*bytes))
            }
            (Builtin::RatioPpm, Value::Quantity(QuantityKind::Ratio, ppm)) => {
                plain(Value::Int(*ppm))
            }
            (
                Builtin::TcpListenerAccept
                | Builtin::TcpListenerLocalPort
                | Builtin::TcpStreamRead
                | Builtin::TcpStreamWriteAll
                | Builtin::TcpStreamClose
                | Builtin::TcpStreamSetIdleTimeout
                | Builtin::PipeStreamRead
                | Builtin::PipeStreamWriteAll
                | Builtin::PipeStreamClose
                | Builtin::PipeStreamSetIdleTimeout
                | Builtin::PipeStreamReceivedBatches
                | Builtin::StreamRead
                | Builtin::StreamWriteAll
                | Builtin::StreamClose
                | Builtin::StreamSetIdleTimeout
                | Builtin::TcpStreamWriteUrgent
                | Builtin::TcpStreamUrgentPending
                | Builtin::PipeStreamWriteUrgent
                | Builtin::PipeStreamUrgentPending
                | Builtin::StreamWriteUrgent
                | Builtin::StreamUrgentPending
                | Builtin::StoreNextId
                | Builtin::StoreGet
                | Builtin::StorePut
                | Builtin::StoreRemove
                | Builtin::StoreValues
                | Builtin::StoreUpdate
                | Builtin::StoreUpsert
                | Builtin::DirectoryReadFile
                | Builtin::DirectoryWriteFile
                | Builtin::DirectoryReadDirectory
                | Builtin::DirectoryCreateDirectory
                | Builtin::DirectoryRemove
                | Builtin::DirectoryRemoveTree
                | Builtin::DirectoryAbsolutePath
                | Builtin::LauncherStart
                | Builtin::LauncherExecute
                | Builtin::ChildProcessId
                | Builtin::ChildProcessWait
                | Builtin::ChildProcessKill
                | Builtin::EntropyRandomBytes
                | Builtin::SettingValue
                | Builtin::CommandLineArguments
                | Builtin::TcpStreamPeer
                | Builtin::TcpStreamTimedOut,
                _,
            ) => Err(internal(format!(
                "builtin `{builtin:?}` has no value at compile time"
            ))),
            (builtin, recv) => Err(internal(format!(
                "builtin `{builtin:?}` on `{}`",
                recv.debug_text()
            ))),
        }
    }

    /// `push`, `append`, `clear`, `put` and `remove`: write through the
    /// receiver's place; a receiver that is no place (a temporary) is
    /// changed and dropped, as on the targets.
    fn mutating(
        &mut self,
        cx: &Cx<'p>,
        e: &Expr,
        builtin: Builtin,
        receiver: &Expr,
        args: &[Expr],
    ) -> Eval<Value> {
        let path = self.place_path(cx, receiver)?;
        let values = self.exprs(cx, args)?;
        let modules = self.modules;
        let unit_result = matches!(cx.types().try_type_of(e), Some(Ty::Unit));
        let mut out_of_range = None;
        let mut temporary;
        let target: &mut Value = match path {
            Some((base, steps)) => {
                let root = self
                    .lookup_mut(&base)
                    .ok_or_else(|| internal(format!("`{base}` is not a local")))?;
                match navigate(root, &steps) {
                    Ok(target) => target,
                    Err((offset, detail)) => {
                        return Err(self.fail_at(cx, offset, FailureOp::Index, &detail));
                    }
                }
            }
            None => {
                temporary = self.expr(cx, receiver)?;
                &mut temporary
            }
        };
        let mut values = values.into_iter();
        let (result, nodes) = match (builtin, target) {
            (Builtin::ListPush, Value::List(items)) => {
                let item = values.next().ok_or_else(|| internal("missing argument"))?;
                let nodes = item.nodes();
                items.push(item);
                (Value::Unit, nodes)
            }
            (Builtin::ListAppend, Value::List(items)) => {
                let Some(Value::List(more)) = values.next() else {
                    return Err(internal("append of a non-list"));
                };
                let nodes = more.len() as u64;
                items.extend(more);
                (Value::Unit, nodes)
            }
            (Builtin::ListSet, Value::List(items)) => {
                let Some(Value::Int(i)) = values.next() else {
                    return Err(internal("set at a non-Int"));
                };
                let item = values.next().ok_or_else(|| internal("missing argument"))?;
                match usize::try_from(i).ok().filter(|&at| at < items.len()) {
                    Some(at) => {
                        let nodes = item.nodes();
                        items[at] = item;
                        (Value::Unit, nodes)
                    }
                    None => {
                        out_of_range =
                            Some(format!("index {i} out of range for length {}", items.len()));
                        (Value::Unit, 0)
                    }
                }
            }
            (Builtin::ListClear, Value::List(items)) => {
                items.clear();
                (Value::Unit, 0)
            }
            (Builtin::MapPut, Value::Map(entries)) => {
                let key = values.next().ok_or_else(|| internal("missing argument"))?;
                let value = values.next().ok_or_else(|| internal("missing argument"))?;
                let nodes = key.nodes() + value.nodes();
                map_put(modules, entries, key, value);
                (Value::Unit, nodes)
            }
            (Builtin::MapRemove, Value::Map(entries)) => {
                let key = values.next().ok_or_else(|| internal("missing argument"))?;
                let removed = match map_find(modules, entries, &key) {
                    Ok(i) => Some(entries.remove(i).1),
                    Err(_) => None,
                };
                let result = if unit_result {
                    Value::Unit
                } else {
                    Value::option(removed)
                };
                (result, 0)
            }
            (builtin, target) => {
                return Err(internal(format!(
                    "builtin `{builtin:?}` on `{}`",
                    target.debug_text()
                )));
            }
        };
        if let Some(detail) = out_of_range {
            return Err(self.fail(cx, e, FailureOp::Index, &detail));
        }
        self.alloc(nodes)?;
        Ok(result)
    }

    /// The place a receiver expression denotes: the local it is rooted in
    /// and the steps from it, index expressions evaluated; `None` for an
    /// expression that is no place.
    fn place_path(&mut self, cx: &Cx<'p>, e: &Expr) -> Eval<Option<(String, Vec<Step>)>> {
        match &e.kind {
            ExprKind::Name(name) if self.is_local(cx, &name.name) => {
                Ok(Some((name.name.clone(), Vec::new())))
            }
            ExprKind::Field { receiver, name } => {
                Ok(self.place_path(cx, receiver)?.map(|(base, mut steps)| {
                    steps.push(Step::Field(name.name.clone()));
                    (base, steps)
                }))
            }
            ExprKind::Index { receiver, index } => {
                let Some((base, mut steps)) = self.place_path(cx, receiver)? else {
                    return Ok(None);
                };
                let Value::Int(i) = self.expr(cx, index)? else {
                    return Err(internal("index is not an Int"));
                };
                steps.push(Step::Index(i, e.span.start));
                Ok(Some((base, steps)))
            }
            ExprKind::TupleField { receiver, index } => {
                Ok(self.place_path(cx, receiver)?.map(|(base, mut steps)| {
                    steps.push(Step::Tuple(*index));
                    (base, steps)
                }))
            }
            _ => Ok(None),
        }
    }

    // ---- fields, maps, equality -----------------------------------------

    fn field(&mut self, cx: &Cx<'p>, e: &Expr, receiver: &Expr, name: &str) -> Eval<Value> {
        let types = cx.types();
        if let Some(reference) = types.reference(e) {
            return Ok(Value::Reference(reference.clone()));
        }
        match types.callee(e) {
            Some(Callee::Fn(q)) => return Ok(Value::Fn(q.clone())),
            Some(Callee::Variant { enum_, index }) => {
                return self.variant_value(enum_, *index, Vec::new());
            }
            _ => {}
        }
        if let ExprKind::Name(base) = &receiver.kind
            && self.is_local(cx, &base.name)
        {
            self.tick()?;
            let record = self.lookup(&base.name).expect("checked local");
            if let Value::Record { fields, .. } = record {
                return fields
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, v)| v.clone())
                    .ok_or_else(|| internal(format!("no field `{name}`")));
            }
        }
        let record = self.expr(cx, receiver)?;
        self.field_of(cx, record, name)
    }

    /// Field `name` of a record value; a member resolves to its record.
    fn field_of(&mut self, cx: &Cx<'p>, record: Value, name: &str) -> Eval<Value> {
        match record {
            Value::Record { fields, .. } => fields
                .into_iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v)
                .ok_or_else(|| internal(format!("no field `{name}`"))),
            Value::IoError { message, .. } if name == "message" => Ok(Value::Text(message)),
            Value::Member { member, .. } => {
                let info = member_of(cx, &member)
                    .map(|(_, info)| info)
                    .ok_or_else(|| internal(format!("`{member}` is not a member")))?;
                let record = self.member_record(cx, info)?;
                self.field_of(cx, record, name)
            }
            other => Err(internal(format!(
                "`{}` has no field `{name}`",
                other.debug_text()
            ))),
        }
    }

    // ---- operators ------------------------------------------------------

    fn unary(&mut self, cx: &Cx<'p>, e: &Expr, op: UnaryOp, operand: &Expr) -> Eval<Value> {
        let value = self.expr(cx, operand)?;
        match (op, value) {
            (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            (UnaryOp::Neg, Value::Int(n)) => match n.checked_neg() {
                Some(negated) => Ok(Value::Int(negated)),
                None => Err(self.fail(cx, e, FailureOp::Sub, &format!("-({n})"))),
            },
            (UnaryOp::Neg, Value::Byte(0)) => Ok(Value::Byte(0)),
            (op, value) => Err(internal(format!(
                "unary `{op:?}` on `{}`",
                value.debug_text()
            ))),
        }
    }

    /// A binary operator chain, the left spine walked without recursion so
    /// that a 100,000-term chain does not overflow the stack (13, row 14).
    fn binary_chain(&mut self, cx: &Cx<'p>, e: &Expr) -> Eval<Value> {
        let mut spine: Vec<&Expr> = Vec::new();
        let mut current = e;
        while let ExprKind::Binary { left, .. } = &current.kind {
            spine.push(current);
            current = left;
        }
        let mut acc = self.expr(cx, current)?;
        while let Some(node) = spine.pop() {
            let ExprKind::Binary { op, right, .. } = &node.kind else {
                unreachable!("spine holds binary nodes");
            };
            if !std::ptr::eq(node, e) {
                self.tick()?;
            }
            acc = match op {
                BinaryOp::And => match acc {
                    Value::Bool(false) => Value::Bool(false),
                    Value::Bool(true) => self.expr(cx, right)?,
                    other => return Err(internal(format!("`&&` on `{}`", other.debug_text()))),
                },
                BinaryOp::Or => match acc {
                    Value::Bool(true) => Value::Bool(true),
                    Value::Bool(false) => self.expr(cx, right)?,
                    other => return Err(internal(format!("`||` on `{}`", other.debug_text()))),
                },
                op => {
                    let right = self.expr(cx, right)?;
                    self.binary(cx, node, *op, acc, right)?
                }
            };
        }
        Ok(acc)
    }

    fn binary(
        &mut self,
        cx: &Cx<'p>,
        node: &Expr,
        op: BinaryOp,
        l: Value,
        r: Value,
    ) -> Eval<Value> {
        use BinaryOp::*;
        match op {
            Eq => return Ok(Value::Bool(l == r)),
            Ne => return Ok(Value::Bool(l != r)),
            Lt | Gt | Le | Ge => {
                let ordering = match (&l, &r) {
                    (Value::Int(a), Value::Int(b)) => a.cmp(b),
                    (Value::Byte(a), Value::Byte(b)) => a.cmp(b),
                    (Value::Quantity(_, a), Value::Quantity(_, b)) => a.cmp(b),
                    (Value::Text(a), Value::Text(b)) => a.as_bytes().cmp(b.as_bytes()),
                    _ => {
                        return Err(internal(format!(
                            "`{}` on `{}` and `{}`",
                            op.symbol(),
                            l.debug_text(),
                            r.debug_text()
                        )));
                    }
                };
                return Ok(Value::Bool(match op {
                    Lt => ordering.is_lt(),
                    Gt => ordering.is_gt(),
                    Le => ordering.is_le(),
                    _ => ordering.is_ge(),
                }));
            }
            _ => {}
        }
        let failure_op = match op {
            Add => FailureOp::Add,
            Sub => FailureOp::Sub,
            Mul => FailureOp::Mul,
            Div => FailureOp::Div,
            _ => FailureOp::Rem,
        };
        let int = |a: i64, b: i64| match op {
            Add => a.checked_add(b),
            Sub => a.checked_sub(b),
            Mul => a.checked_mul(b),
            Div => a.checked_div(b),
            _ => a.checked_rem(b),
        };
        let byte = |a: u8, b: u8| match op {
            Add => a.checked_add(b),
            Sub => a.checked_sub(b),
            Mul => a.checked_mul(b),
            Div => a.checked_div(b),
            _ => a.checked_rem(b),
        };
        let overflow = |ev: &Self, a: i64, b: i64| {
            ev.fail(cx, node, failure_op, &format!("{a} {} {b}", op.symbol()))
        };
        match (&l, &r) {
            (Value::Int(a), Value::Int(b)) => int(*a, *b)
                .map(Value::Int)
                .ok_or_else(|| overflow(self, *a, *b)),
            (Value::Byte(a), Value::Byte(b)) => byte(*a, *b)
                .map(Value::Byte)
                .ok_or_else(|| overflow(self, i64::from(*a), i64::from(*b))),
            (Value::List(a), Value::List(b)) if op == Add => {
                let joined: Vec<Value> = a.iter().chain(b).cloned().collect();
                self.alloc(joined.len() as u64)?;
                Ok(Value::List(joined))
            }
            (Value::Text(a), Value::Text(b)) if op == Add => {
                let joined = format!("{a}{b}");
                self.alloc(joined.len() as u64 / 64)?;
                Ok(Value::Text(joined))
            }
            (Value::Quantity(k, a), Value::Quantity(_, b)) if matches!(op, Add | Sub) => {
                int(*a, *b)
                    .map(|v| Value::Quantity(*k, v))
                    .ok_or_else(|| overflow(self, *a, *b))
            }
            (Value::Quantity(k, a), Value::Int(b)) if matches!(op, Mul | Div) => int(*a, *b)
                .map(|v| Value::Quantity(*k, v))
                .ok_or_else(|| overflow(self, *a, *b)),
            (Value::Int(a), Value::Quantity(k, b)) if op == Mul => int(*a, *b)
                .map(|v| Value::Quantity(*k, v))
                .ok_or_else(|| overflow(self, *a, *b)),
            _ => Err(internal(format!(
                "`{}` on `{}` and `{}`",
                op.symbol(),
                l.debug_text(),
                r.debug_text()
            ))),
        }
    }

    // ---- match, try, blocks, statements ---------------------------------

    fn match_(&mut self, cx: &Cx<'p>, e: &Expr, scrutinee: &Expr, arms: &[Arm]) -> Eval<Value> {
        let value = self.expr(cx, scrutinee)?;
        for arm in arms {
            let mut bindings = Vec::new();
            if self.matches(cx, &arm.pattern, &value, &mut bindings)? {
                self.push_scope();
                for (name, value) in bindings {
                    self.bind(&name, value);
                }
                let result = self.expr(cx, &arm.body);
                self.pop_scope();
                return result;
            }
        }
        Err(self.fail(cx, e, FailureOp::Match, "no arm matched"))
    }

    /// Whether `pattern` matches `value`, with the bindings it makes.
    fn matches(
        &mut self,
        cx: &Cx<'p>,
        pattern: &Pattern,
        value: &Value,
        bindings: &mut Vec<(String, Value)>,
    ) -> Eval<bool> {
        if let Some(q) = cx.types().pattern_const(pattern) {
            let constant = self.constant(q)?;
            return Ok(constant == *value);
        }
        Ok(match &pattern.kind {
            PatternKind::Wildcard => true,
            PatternKind::Int(n) => match value {
                Value::Int(v) => v == n,
                Value::Byte(v) => i64::from(*v) == *n,
                _ => false,
            },
            PatternKind::Str(s) => matches!(value, Value::Text(t) if t == s),
            PatternKind::Bool(b) => matches!(value, Value::Bool(v) if v == b),
            PatternKind::Or(alternatives) => {
                for alternative in alternatives {
                    let mut found = Vec::new();
                    if self.matches(cx, alternative, value, &mut found)? {
                        bindings.extend(found);
                        return Ok(true);
                    }
                }
                false
            }
            PatternKind::Tuple(items) if items.is_empty() => matches!(value, Value::Unit),
            PatternKind::Tuple(items) => match value {
                Value::Tuple(values) if values.len() == items.len() => {
                    self.match_all(cx, items, values, bindings)?
                }
                _ => false,
            },
            PatternKind::List(items) => {
                let rest = items
                    .iter()
                    .position(|item| matches!(item.kind, PatternKind::Rest(_)));
                match (value, rest) {
                    (Value::List(values), None) if values.len() == items.len() => {
                        self.match_all(cx, items, values, bindings)?
                    }
                    (Value::List(values), Some(at)) if values.len() + 1 >= items.len() => {
                        let suffix = items.len() - at - 1;
                        let end = values.len() - suffix;
                        self.match_all(cx, &items[..at], &values[..at], bindings)?
                            && self.match_all(cx, &items[at + 1..], &values[end..], bindings)?
                            && self.matches(
                                cx,
                                &items[at],
                                &Value::List(values[at..end].to_vec()),
                                bindings,
                            )?
                    }
                    _ => false,
                }
            }
            PatternKind::Rest(name) => {
                if let Some(name) = name {
                    bindings.push((name.name.clone(), value.clone()));
                }
                true
            }
            PatternKind::Binding(name) => {
                bindings.push((name.name.clone(), value.clone()));
                true
            }
            PatternKind::Variant { path, args } => {
                let Some(variant) = path.last() else {
                    return Err(internal("variant pattern without a name"));
                };
                match value {
                    Value::Enum {
                        variant: found,
                        fields,
                        ..
                    } if *found == variant.name && fields.len() == args.len() => {
                        self.match_all(cx, args, fields, bindings)?
                    }
                    _ => false,
                }
            }
            PatternKind::Range { .. } => {
                let (lo, hi) = cx
                    .types()
                    .range_bounds(pattern)
                    .ok_or_else(|| internal("range pattern without bounds"))?;
                match value {
                    Value::Int(v) => lo <= *v && *v <= hi,
                    Value::Byte(v) => lo <= i64::from(*v) && i64::from(*v) <= hi,
                    _ => false,
                }
            }
        })
    }

    fn match_all(
        &mut self,
        cx: &Cx<'p>,
        patterns: &[Pattern],
        values: &[Value],
        bindings: &mut Vec<(String, Value)>,
    ) -> Eval<bool> {
        for (pattern, value) in patterns.iter().zip(values) {
            if !self.matches(cx, pattern, value, bindings)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `e?`: the `Ok` value, or a return of the `Err` from the enclosing fn
    /// or closure (`docs/slice.md` section 13.3). Outside any fn it is a
    /// failure with the 12.5 report.
    fn try_(&mut self, cx: &Cx<'p>, e: &Expr, inner: &Expr) -> Eval<Value> {
        let value = self.expr(cx, inner)?;
        let Value::Enum {
            name,
            variant,
            mut fields,
        } = value
        else {
            return Err(internal("`?` on a value that is no Result or Option"));
        };
        let payload = fields.pop();
        match (name.name.as_str(), variant.as_str(), payload) {
            ("Result", "Ok", Some(value)) | ("Option", "Some", Some(value)) => Ok(value),
            ("Result", "Err", Some(error)) => {
                if self.frames.len() <= 1 {
                    let detail = format!("Err({})", error.debug_text());
                    return Err(self.fail(cx, e, FailureOp::Try, &detail));
                }
                Err(Flow::Return(Value::err(error)))
            }
            ("Option", "None", None) => {
                if self.frames.len() <= 1 {
                    return Err(self.fail(cx, e, FailureOp::Try, "None"));
                }
                Err(Flow::Return(Value::none()))
            }
            _ => Err(internal("`?` on a value that is no Result or Option")),
        }
    }

    /// A block in its own scope.
    fn scoped_block(&mut self, cx: &Cx<'p>, block: &Block) -> Eval<Value> {
        self.push_scope();
        let result = self.block_body(cx, block);
        self.pop_scope();
        result
    }

    /// The statements of a block; its value is that of a last expression
    /// statement, else `Unit`.
    fn block_body(&mut self, cx: &Cx<'p>, block: &Block) -> Eval<Value> {
        let Some((last, init)) = block.stmts.split_last() else {
            return Ok(Value::Unit);
        };
        for stmt in init {
            self.stmt(cx, stmt)?;
        }
        match &last.kind {
            StmtKind::Expr(expr) => self.expr(cx, expr),
            _ => {
                self.stmt(cx, last)?;
                Ok(Value::Unit)
            }
        }
    }

    fn stmt(&mut self, cx: &Cx<'p>, stmt: &Stmt) -> Eval<()> {
        match &stmt.kind {
            StmtKind::Let { name, value, .. } => {
                let value = self.expr(cx, value)?;
                self.bind(&name.name, value);
            }
            StmtKind::LetPattern { pattern, value, .. } => {
                let value = self.expr(cx, value)?;
                let mut bindings = Vec::new();
                if !self.matches(cx, pattern, &value, &mut bindings)? {
                    return Err(internal("irrefutable pattern did not match"));
                }
                for (name, value) in bindings {
                    self.bind(&name, value);
                }
            }
            StmtKind::Assign { target, value } => {
                let value = self.expr(cx, value)?;
                let steps: Vec<Step> = target
                    .fields
                    .iter()
                    .map(|f| Step::Field(f.name.clone()))
                    .collect();
                let root = self
                    .lookup_mut(&target.base.name)
                    .ok_or_else(|| internal(format!("`{}` is not a local", target.base.name)))?;
                match navigate(root, &steps) {
                    Ok(slot) => *slot = value,
                    Err((_, detail)) => return Err(internal(detail)),
                }
            }
            StmtKind::Return(value) => {
                let value = match value {
                    Some(value) => self.expr(cx, value)?,
                    None => Value::Unit,
                };
                return Err(Flow::Return(value));
            }
            StmtKind::While { cond, body } => loop {
                self.tick()?;
                match self.expr(cx, cond)? {
                    Value::Bool(true) => {}
                    Value::Bool(false) => break,
                    _ => return Err(internal("`while` condition is not a Bool")),
                }
                match self.scoped_block(cx, body) {
                    Ok(_) | Err(Flow::Continue) => {}
                    Err(Flow::Break) => break,
                    Err(other) => return Err(other),
                }
            },
            StmtKind::Loop { body } => loop {
                self.tick()?;
                match self.scoped_block(cx, body) {
                    Ok(_) | Err(Flow::Continue) => {}
                    Err(Flow::Break) => break,
                    Err(other) => return Err(other),
                }
            },
            StmtKind::For { name, iter, body } => {
                let items = self.iterable(cx, iter)?;
                for item in items {
                    self.tick()?;
                    self.push_scope();
                    self.bind(&name.name, item);
                    let result = self.block_body(cx, body);
                    self.pop_scope();
                    match result {
                        Ok(_) | Err(Flow::Continue) => {}
                        Err(Flow::Break) => break,
                        Err(other) => return Err(other),
                    }
                }
            }
            StmtKind::ForPattern {
                pattern,
                iter,
                body,
            } => {
                let items = self.iterable(cx, iter)?;
                for item in items {
                    self.tick()?;
                    let mut bindings = Vec::new();
                    if !self.matches(cx, pattern, &item, &mut bindings)? {
                        return Err(internal("irrefutable pattern did not match"));
                    }
                    self.push_scope();
                    for (name, value) in bindings {
                        self.bind(&name, value);
                    }
                    let result = self.block_body(cx, body);
                    self.pop_scope();
                    match result {
                        Ok(_) | Err(Flow::Continue) => {}
                        Err(Flow::Break) => break,
                        Err(other) => return Err(other),
                    }
                }
            }
            StmtKind::Break => return Err(Flow::Break),
            StmtKind::Continue => return Err(Flow::Continue),
            StmtKind::Expr(expr) => {
                self.expr(cx, expr)?;
            }
            StmtKind::Assert { cond, message } => self.assert(cx, stmt, cond, message.as_ref())?,
        }
        Ok(())
    }

    /// The items a `for` iterates: a list's, or a map's `(k, v)` pairs.
    fn iterable(&mut self, cx: &Cx<'p>, iter: &Expr) -> Eval<Vec<Value>> {
        match self.expr(cx, iter)? {
            Value::List(items) => Ok(items),
            Value::Map(entries) => Ok(entries
                .into_iter()
                .map(|(k, v)| Value::Tuple(vec![k, v]))
                .collect()),
            other => Err(internal(format!("`for` over `{}`", other.debug_text()))),
        }
    }

    /// `assert cond` and `assert cond, message` (12.5): a failed one ends
    /// the evaluation with the report of `ignite_io::assert_report`.
    fn assert(
        &mut self,
        cx: &Cx<'p>,
        stmt: &Stmt,
        cond: &Expr,
        message: Option<&Expr>,
    ) -> Eval<()> {
        let types = cx.types();
        let compare = types.assert_compare(cond).cloned();
        let (holds, operands) = match (&compare, &cond.kind) {
            (
                Some(compare),
                ExprKind::Binary {
                    op: op @ (BinaryOp::Eq | BinaryOp::Ne),
                    left,
                    right,
                },
            ) => {
                self.tick()?;
                let l = self.expr(cx, left)?;
                let r = self.expr(cx, right)?;
                let equal = l == r;
                let holds = if *op == BinaryOp::Eq { equal } else { !equal };
                if holds {
                    (true, None)
                } else {
                    let shown = (self.show(compare, &l)?, self.show(compare, &r)?);
                    (false, Some(shown))
                }
            }
            _ => match self.expr(cx, cond)? {
                Value::Bool(holds) => (holds, None),
                _ => return Err(internal("`assert` condition is not a Bool")),
            },
        };
        if holds {
            return Ok(());
        }
        let message = match message {
            Some(message) => match self.expr(cx, message)? {
                Value::Text(text) => Some(text),
                _ => return Err(internal("`assert` message is not a Text")),
            },
            None => None,
        };
        let source = cx.source();
        let (line, col) = source.line_col(stmt.span.start);
        let text = source
            .text
            .get(cond.span.start..cond.span.end)
            .unwrap_or("");
        let mut report = format!("assert failed at {}:{line}:{col}: {text}", source.path);
        if let Some(message) = message {
            report.push_str("\n  message: ");
            report.push_str(&message);
        }
        if let Some((left, right)) = operands {
            report.push_str("\n  left: ");
            report.push_str(&left);
            report.push_str("\n  right: ");
            report.push_str(&right);
        }
        Err(Flow::Error(EvalError::Failed { report }))
    }

    /// How a failed `assert a == b` shows an operand (`ignite_types::Show`).
    fn show(&mut self, compare: &AssertCompare, value: &Value) -> Eval<String> {
        Ok(match (&compare.show, value) {
            (Some(Show::Builtin(Builtin::IntToTextMethod)), Value::Int(n)) => n.to_string(),
            (Some(Show::Builtin(Builtin::ByteToText)), Value::Byte(b)) => b.to_string(),
            (Some(Show::Builtin(Builtin::BoolToText)), Value::Bool(b)) => b.to_string(),
            (Some(Show::Builtin(Builtin::TextToText)), Value::Text(t)) => t.clone(),
            (Some(Show::Builtin(Builtin::TextToText)), Value::Reference(reference)) => {
                reference.text()
            }
            (Some(Show::Builtin(Builtin::TextToText)), Value::Member { system, member }) => {
                format!("{system}.{member}")
            }
            (Some(Show::Builtin(Builtin::BytesToText)), Value::List(items)) => {
                String::from_utf8_lossy(&bytes_of(items)?).into_owned()
            }
            (Some(Show::Fn(q)), value) => match self.call_fn(q, vec![value.clone()])? {
                Value::Text(text) => text,
                _ => return Err(internal("`to_text` returned no Text")),
            },
            (Some(Show::IoErrorMessage), Value::IoError { message, .. }) => message.clone(),
            (Some(Show::Debug), value) | (Some(Show::Builtin(_)), value) => value.debug_text(),
            (Some(Show::IoErrorMessage), value) => value.debug_text(),
            (None, _) => format!("<{}>", compare.operand),
        })
    }

    // ---- failure reports (14.8) -----------------------------------------

    /// The E117 flow of the failure site of `e` (its recorded operation and
    /// offset, else `op` at the expression's start).
    fn fail(&self, cx: &Cx<'p>, e: &Expr, op: FailureOp, detail: &str) -> Flow {
        let (op, offset) = match cx.types().failure_site(e) {
            Some(site) => (site.op, site.offset),
            None => (op, e.span.start),
        };
        self.fail_at(cx, offset, op, detail)
    }

    fn fail_at(&self, cx: &Cx<'p>, offset: usize, op: FailureOp, detail: &str) -> Flow {
        let source = cx.source();
        let (line, col) = source.line_col(offset);
        Flow::Error(EvalError::Failed {
            report: format!(
                "{} failed at {}:{line}:{col}: {detail}",
                op.as_str(),
                source.path
            ),
        })
    }
}

/// The system whose declaration spans `offset` in `module`, by name.
fn enclosing_system<'p>(module: &'p Module<'p>, offset: usize) -> Option<&'p str> {
    module.types.systems().iter().find_map(|system| {
        let decl = system.decl(module.program)?;
        (decl.span.start <= offset && offset < decl.span.end).then_some(system.name.as_str())
    })
}

/// The member `name` of the system `cx` is in.
fn member_of<'p>(cx: &Cx<'p>, name: &str) -> Option<(&'p str, &'p MemberInfo)> {
    let system = cx.system?;
    let info = cx
        .types()
        .systems()
        .iter()
        .find(|s| s.name == system)?
        .member(name)?;
    Some((system, info))
}

/// Builtin statics and free fns: the receiver of such a method call is the
/// owner's name, not a value.
fn is_static(builtin: Builtin) -> bool {
    matches!(
        builtin,
        Builtin::BytesEmpty
            | Builtin::BytesOf
            | Builtin::ListEmpty
            | Builtin::MapEmpty
            | Builtin::IntToText
            | Builtin::IntParse
            | Builtin::IoErrorNew
            | Builtin::StoreNew
            | Builtin::TcpListenerBind
            | Builtin::TcpStreamConnect
            | Builtin::StdinRead
            | Builtin::StdinIsTerminal
            | Builtin::StdinSetRaw
            | Builtin::StdoutWriteAll
            | Builtin::StderrWriteAll
            | Builtin::ProcessExit
            | Builtin::Log
            | Builtin::ModuleRecord
            | Builtin::ClockMonotonic
            | Builtin::EndpointOf
            | Builtin::PipePair
            | Builtin::DirectoryTemporary
            | Builtin::LauncherNew
            | Builtin::EntropyNew
            | Builtin::SettingOf
            | Builtin::CommandLineOf
            | Builtin::TcpStreamConnectWithin
    )
}

/// The variants of a builtin enum, in declaration order.
fn builtin_variants(name: &str) -> &'static [&'static str] {
    match name {
        "Option" => &["Some", "None"],
        "Result" => &["Ok", "Err"],
        "Input" => &["Data", "Eof"],
        "Exposure" => &EXPOSURE_VARIANTS,
        _ => &[],
    }
}

fn bytes_of(items: &[Value]) -> Eval<Vec<u8>> {
    items
        .iter()
        .map(|item| match item {
            Value::Byte(b) => Ok(*b),
            _ => Err(internal("a Bytes value holds a non-Byte")),
        })
        .collect()
}

/// Follow `steps` from `root`; an index out of range gives the offset of the
/// index expression and the `index` failure detail.
fn navigate<'v>(
    mut value: &'v mut Value,
    steps: &[Step],
) -> Result<&'v mut Value, (usize, String)> {
    for step in steps {
        value = match step {
            Step::Field(name) => match value {
                Value::Record { fields, .. } => match fields.iter_mut().find(|(n, _)| n == name) {
                    Some((_, field)) => field,
                    None => return Err((0, format!("no field `{name}`"))),
                },
                _ => return Err((0, "field of a value that is no record".to_string())),
            },
            Step::Index(index, offset) => match value {
                Value::List(items) => {
                    let len = items.len();
                    match usize::try_from(*index).ok().and_then(|i| items.get_mut(i)) {
                        Some(item) => item,
                        None => {
                            return Err((
                                *offset,
                                format!("index {index} out of range for length {len}"),
                            ));
                        }
                    }
                }
                _ => return Err((*offset, "index of a value that is no list".to_string())),
            },
            Step::Tuple(index) => match value {
                Value::Tuple(items) => match items.get_mut(*index) {
                    Some(item) => item,
                    None => return Err((0, format!("tuple has no element {index}"))),
                },
                _ => return Err((0, "element of a value that is no tuple".to_string())),
            },
        };
    }
    Ok(value)
}

fn find_fn<'a>(program: &'a Program, name: &str) -> Option<&'a FnDecl> {
    program.decls.iter().find_map(|decl| match decl {
        Decl::Fn(f) if f.qualified_name() == name => Some(f),
        _ => None,
    })
}

fn find_const<'a>(program: &'a Program, name: &str) -> Option<&'a ConstDecl> {
    program.decls.iter().find_map(|decl| match decl {
        Decl::Const(c) if c.name.name == name => Some(c),
        _ => None,
    })
}

fn find_record<'a>(program: &'a Program, name: &str) -> Option<&'a RecordDecl> {
    program.decls.iter().find_map(|decl| match decl {
        Decl::Record(r) if r.name.name == name => Some(r),
        _ => None,
    })
}

fn find_enum<'a>(program: &'a Program, name: &str) -> Option<&'a EnumDecl> {
    program.decls.iter().find_map(|decl| match decl {
        Decl::Enum(e) if e.name.name == name => Some(e),
        _ => None,
    })
}

/// Every closure literal of a module by the byte offset of its start, found
/// by a work-list walk over every expression (no recursion, so a
/// 100,000-term chain is fine).
fn index_closures(program: &Program) -> HashMap<usize, &Expr> {
    let mut pending: Vec<&Expr> = Vec::new();
    let mut blocks: Vec<&Block> = Vec::new();
    for decl in &program.decls {
        match decl {
            Decl::Fn(f) => blocks.push(&f.body),
            Decl::Const(c) => pending.push(&c.value),
            Decl::Record(r) => pending.extend(r.fields.iter().filter_map(|f| f.default.as_ref())),
            Decl::Protocol(p) => blocks.extend(p.fns.iter().map(|f| &f.body)),
            Decl::Test(t) => blocks.push(&t.body),
            Decl::System(s) => {
                pending.extend(
                    s.resources
                        .iter()
                        .flat_map(|r| r.properties.iter().map(|p| &p.value)),
                );
                for component in s.components.iter().chain(&s.contracts).chain(&s.policies) {
                    pending.extend(component.properties.iter().map(|p| &p.value));
                }
            }
            Decl::Assembly(a) => pending.extend(a.properties.iter().map(|p| &p.value)),
            Decl::Enum(_) | Decl::Alias(_) => {}
        }
    }
    let mut index = HashMap::new();
    loop {
        while let Some(block) = blocks.pop() {
            for stmt in &block.stmts {
                match &stmt.kind {
                    StmtKind::Let { value, .. }
                    | StmtKind::LetPattern { value, .. }
                    | StmtKind::Assign { value, .. }
                    | StmtKind::Expr(value) => pending.push(value),
                    StmtKind::Return(value) => pending.extend(value.as_ref()),
                    StmtKind::While { cond, body } => {
                        pending.push(cond);
                        blocks.push(body);
                    }
                    StmtKind::Loop { body } => blocks.push(body),
                    StmtKind::For { iter, body, .. } | StmtKind::ForPattern { iter, body, .. } => {
                        pending.push(iter);
                        blocks.push(body);
                    }
                    StmtKind::Assert { cond, message } => {
                        pending.push(cond);
                        pending.extend(message.as_ref());
                    }
                    StmtKind::Break | StmtKind::Continue => {}
                }
            }
        }
        let Some(expr) = pending.pop() else {
            break;
        };
        match &expr.kind {
            ExprKind::Closure(closure) => {
                index.insert(expr.span.start, expr);
                pending.push(&closure.body);
            }
            ExprKind::Record { fields, .. } => pending.extend(fields.iter().map(|f| &f.value)),
            ExprKind::RecordUpdate { fields, base, .. } => {
                pending.extend(fields.iter().map(|f| &f.value));
                pending.push(base);
            }
            ExprKind::List(items) | ExprKind::Tuple(items) => pending.extend(items),
            ExprKind::Return(value) => pending.extend(value.as_deref()),
            ExprKind::Call { callee, args } => {
                pending.push(callee);
                pending.extend(args);
            }
            ExprKind::Method { receiver, args, .. } => {
                pending.push(receiver);
                pending.extend(args);
            }
            ExprKind::Field { receiver, .. } | ExprKind::TupleField { receiver, .. } => {
                pending.push(receiver);
            }
            ExprKind::Index { receiver, index } => {
                pending.push(receiver);
                pending.push(index);
            }
            ExprKind::Try(inner)
            | ExprKind::Await(inner)
            | ExprKind::Spawn(inner)
            | ExprKind::Unary { operand: inner, .. }
            | ExprKind::NamedArg { value: inner, .. } => pending.push(inner),
            ExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ExprKind::Match { scrutinee, arms } => {
                pending.push(scrutinee);
                pending.extend(arms.iter().map(|arm| &arm.body));
            }
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => {
                pending.push(cond);
                blocks.push(then);
                pending.extend(otherwise.as_deref());
            }
            ExprKind::Block(block) => blocks.push(block),
            ExprKind::Brace { entries } => pending.extend(entries.iter().map(|e| &e.value)),
            ExprKind::Int(_)
            | ExprKind::Str(_)
            | ExprKind::Bool(_)
            | ExprKind::Name(_)
            | ExprKind::Break
            | ExprKind::Continue
            | ExprKind::Quantity { .. } => {}
        }
        if pending.is_empty() && blocks.is_empty() {
            break;
        }
    }
    index
}

/// The segments of a path joined with `.`.
fn dotted(path: &ignite_syntax::ast::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.name.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

/// Where `key` is in `entries`, or where it would be inserted.
fn map_find(
    modules: &Modules<'_>,
    entries: &[(Value, Value)],
    key: &Value,
) -> Result<usize, usize> {
    entries.binary_search_by(|(k, _)| key_cmp(modules, k, key))
}

fn map_put(modules: &Modules<'_>, entries: &mut Vec<(Value, Value)>, key: Value, value: Value) {
    match map_find(modules, entries, &key) {
        Ok(i) => entries[i].1 = value,
        Err(i) => entries.insert(i, (key, value)),
    }
}

/// The order of map keys (14.3): `Int`, `Byte` and `Bool` by value, `Text`
/// and references by bytes, enums without payload by declaration order.
fn key_cmp(modules: &Modules<'_>, a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Byte(x), Value::Byte(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (Value::Text(x), Value::Text(y)) => x.as_bytes().cmp(y.as_bytes()),
        (Value::Reference(x), Value::Reference(y)) => x.text().cmp(&y.text()),
        (Value::Quantity(_, x), Value::Quantity(_, y)) => x.cmp(y),
        (
            Value::Enum {
                name, variant: x, ..
            },
            Value::Enum { variant: y, .. },
        ) => variant_index(modules, name, x).cmp(&variant_index(modules, name, y)),
        _ => Ordering::Equal,
    }
}

/// The index of variant `variant` of `enum_`.
fn variant_index(modules: &Modules<'_>, enum_: &QualName, variant: &str) -> Option<usize> {
    if enum_.is_builtin() {
        return builtin_variants(&enum_.name)
            .iter()
            .position(|v| *v == variant);
    }
    let module = modules.get(&enum_.module)?;
    let decl = find_enum(module.program, &enum_.name)?;
    decl.variants.iter().position(|v| v.name.name == variant)
}
