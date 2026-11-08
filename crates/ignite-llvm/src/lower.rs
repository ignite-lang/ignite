//! Functions, statements, expressions and patterns to IR (`docs/llvm.md`
//! sections 3 and 4).
//!
//! Every local lives in an entry-block slot; an aggregate is handled by a
//! pointer to its memory. An expression evaluates to a [`Val`]; an
//! aggregate result is either *owned* (a fresh temporary nobody else sees)
//! or *borrowed* (a place: a local's slot or a part of one). In a value
//! position of the ownership rule a borrowed aggregate is deep-copied, so a
//! binding, argument, list element or field never shares a buffer with a
//! place.

use std::collections::BTreeSet;

use ignite_check::Symbols;
use ignite_syntax::ast::{
    BinaryOp, Block, BraceEntry, Decl, Expr, ExprKind, Ident, Pattern, PatternKind, Program, Stmt,
    StmtKind, UnaryOp,
};
use ignite_types::ownership;
use ignite_types::{BraceKind, Builtin, Callee, Capability, FnSig, QualName, Ty, TypedProgram};

use crate::ir::{Func, Generic, Glue, Instance, Module, global};
use crate::layout::{
    Layout, TypeEnv, bytes, c_struct, is_aggregate, is_void, map_pair, memory_type, repr,
    scalar_type,
};

/// Operands of a `Text` chain copied per arena block or growth
/// ([`Lower::text_chain`]).
const CONCAT_BATCH: usize = 16;

/// The module a function is lowered in.
pub struct Ctx<'a> {
    /// Module path used in symbol names.
    pub module: String,
    pub program: &'a Program,
    pub symbols: &'a Symbols,
    pub types: &'a TypedProgram,
    consts: BTreeSet<String>,
    fns: BTreeSet<String>,
}

impl<'a> Ctx<'a> {
    pub fn new(
        module: &str,
        program: &'a Program,
        symbols: &'a Symbols,
        types: &'a TypedProgram,
    ) -> Ctx<'a> {
        let mut consts = symbols.constants.clone();
        let mut fns: BTreeSet<String> = symbols.functions.keys().cloned().collect();
        for decl in &program.decls {
            match decl {
                Decl::Const(c) => {
                    consts.insert(c.name.name.clone());
                }
                Decl::Fn(f) => {
                    fns.insert(f.qualified_name());
                }
                _ => {}
            }
        }
        Ctx {
            module: module.to_string(),
            program,
            symbols,
            types,
            consts,
            fns,
        }
    }

    /// The qualified name of a module-level name visible here.
    pub fn qual(&self, name: &str) -> QualName {
        QualName {
            module: self
                .symbols
                .imported
                .get(name)
                .cloned()
                .unwrap_or_else(|| self.module.clone()),
            name: name.to_string(),
        }
    }
}

/// `"<module>.<name>"`.
pub fn symbol(name: &QualName) -> String {
    format!("{}.{}", name.module, name.name)
}

/// The constant holding the default of `field` of `record`:
/// `"<module>.<Record>.<field>"`.
pub fn default_symbol(record: &QualName, field: &str) -> String {
    format!("{}.{field}", symbol(record))
}

/// LLVM return type and parameter types of a function with this signature:
/// aggregates by pointer, an aggregate result through a leading pointer,
/// `Unit` omitted.
pub fn abi(params: &[Ty], ret: &Ty) -> (String, Vec<String>) {
    let mut types = Vec::new();
    let ret = if is_aggregate(ret) {
        types.push("ptr".to_string());
        "void".to_string()
    } else {
        scalar_type(ret).unwrap_or("void").to_string()
    };
    for param in params {
        if is_aggregate(param) {
            types.push("ptr".to_string());
        } else if let Some(t) = scalar_type(param) {
            types.push(t.to_string());
        }
    }
    (ret, types)
}

/// The value of an expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Val {
    /// `Unit`: no value.
    Unit,
    /// The expression does not complete (`return`, `Process.exit`).
    Never,
    /// A scalar operand (`%t3`, `42`, `true`, `@"f"`).
    Scalar(String),
    /// A pointer to an aggregate; `true` when owned.
    Ptr(String, bool),
}

#[derive(Clone, Debug)]
struct Local {
    name: String,
    /// Slot of a scalar or memory of an aggregate; empty for `Unit`.
    ptr: String,
    ty: Ty,
    /// A stream binding its block owns ([`crate::own`]).
    owned: bool,
    /// For a `let` of a closure literal: per parameter, whether the
    /// closure only reads it ([`Lower::known_borrows`]); empty otherwise.
    borrows: Vec<bool>,
}

pub struct Lower<'a, 'm, 'e> {
    pub m: &'m mut Module<'e>,
    pub env: &'e TypeEnv,
    pub cx: &'a Ctx<'a>,
    pub f: Func,
    scopes: Vec<Vec<Local>>,
    /// Per enclosing loop: the label `continue` goes to, the label `break`
    /// goes to, and the length of `own_marks` at the loop's entry.
    loops: Vec<(String, String, usize)>,
    /// Per open block: the owner mark (`ignite_rt_own_mark`) taken at its
    /// first owned stream binding ([`crate::own`]), released when the
    /// block is left.
    own_marks: Vec<Option<String>>,
    ret: Ty,
    sret: Option<String>,
    /// Depth of expressions that may hold a partial value (an operand, an
    /// argument, a field) while a sub-expression runs. A loop lowered at a
    /// depth above 0 gets no region: a release could free memory a pending
    /// value points into.
    nested: u32,
    /// The type parameters of the generic declaration being lowered and the
    /// instance's arguments for them; empty outside generic code.
    subst: (Vec<String>, Vec<Ty>),
    /// The argument names of the `spawn` statement being lowered whose
    /// stream handles move to the task (`region::moved_to_task`); set by
    /// [`Lower::block`] before the statement, taken by [`Lower::spawn`].
    moved_streams: Vec<String>,
    /// The `spawn` statements that hand an owned stream binding over to
    /// their task (`own::handovers`); recorded at the binding's `let`,
    /// taken by [`Lower::spawn`].
    handovers: Vec<Handover>,
}

/// An owned stream binding `name` whose last use is the spawned call with
/// the key `call` ([`expr_key`]).
struct Handover {
    call: usize,
    name: String,
}

/// The identity of an expression of the AST being lowered.
fn expr_key(e: &Expr) -> usize {
    e as *const Expr as usize
}

/// The region of one loop (`docs/llvm.md` section 4): the arena mark taken
/// at its entry and the modified variables it copies across a release.
struct Region {
    mark: String,
    /// Bytes the loop kept above its mark after its last release; only
    /// with carried variables.
    kept: Option<String>,
    /// Each carried variable with the slot of its scratch copy.
    carried: Vec<(Local, String)>,
}

/// How a `for` binds its element: a single name, or a pattern destructured
/// against the element (`docs/slice.md` section 14.3, `for (k, v) in map`).
enum ForBind<'b> {
    Name(&'b Ident),
    Pattern(&'b Pattern),
}

impl<'a, 'm, 'e> Lower<'a, 'm, 'e> {
    pub fn new(m: &'m mut Module<'e>, cx: &'a Ctx<'a>, f: Func, ret: Ty) -> Lower<'a, 'm, 'e> {
        let env = m.env;
        let sret = is_aggregate(&ret).then(|| "%sret".to_string());
        Lower {
            m,
            env,
            cx,
            f,
            scopes: vec![Vec::new()],
            loops: Vec::new(),
            own_marks: Vec::new(),
            ret,
            sret,
            nested: 0,
            subst: (Vec::new(), Vec::new()),
            moved_streams: Vec::new(),
            handovers: Vec::new(),
        }
    }

    // ---- debug metadata --------------------------------------------------

    /// Give the function being lowered the `DISubprogram` of `symbol`,
    /// declared at byte `offset` of the current module, and start its
    /// instructions there (`docs/llvm.md` section 9). Nothing when the
    /// module's source is unknown.
    pub fn debug_scope(&mut self, symbol: &str, offset: usize) {
        let scope = self.m.subprogram(&self.cx.module, symbol, offset);
        self.f.set_scope(scope);
        self.locate(offset);
    }

    /// Emit the following instructions at byte `offset` of the current
    /// module; the previous location, for [`Func::restore_location`].
    fn locate(&mut self, offset: usize) -> Option<String> {
        let scope = self.f.scope()?;
        let (line, column) = self.m.line_col(&self.cx.module, offset)?;
        let node = self.m.location(line, column, scope);
        Some(self.f.set_location(node))
    }

    // ---- functions -------------------------------------------------------

    /// Define `symbol` with `params` (name, type), result `ret` and `body`.
    pub fn define_fn(
        m: &mut Module<'_>,
        cx: &Ctx<'_>,
        symbol: &str,
        params: &[(Ident, Ty)],
        ret: &Ty,
        body: &Block,
    ) {
        let subst = (Vec::new(), Vec::new());
        Self::define_fn_in(m, cx, symbol, params, ret, body, subst, false);
    }

    /// [`Lower::define_fn`] for an instance of generic code: `subst` maps
    /// the declaration's type parameters to the instance's arguments
    /// (`params` and `ret` are already instantiated), and `instance`
    /// defines the symbol `linkonce_odr` in its own comdat.
    #[allow(clippy::too_many_arguments)]
    pub fn define_fn_in(
        m: &mut Module<'_>,
        cx: &Ctx<'_>,
        symbol: &str,
        params: &[(Ident, Ty)],
        ret: &Ty,
        body: &Block,
        subst: (Vec<String>, Vec<Ty>),
        instance: bool,
    ) {
        let (ret_abi, _) = abi(&[], ret);
        let mut list = Vec::new();
        if is_aggregate(ret) {
            list.push("ptr %sret".to_string());
        }
        let mut incoming = Vec::new();
        for (i, (_, ty)) in params.iter().enumerate() {
            if is_aggregate(ty) {
                list.push(format!("ptr %p{i}"));
            } else if let Some(t) = scalar_type(ty) {
                list.push(format!("{t} %p{i}"));
            }
            incoming.push(format!("%p{i}"));
        }
        let (linkage, comdat) = instance_linkage(m, symbol, instance);
        let header = format!(
            "define {linkage}{ret_abi} {}({}){comdat}",
            global(symbol),
            list.join(", ")
        );
        let mut lower = Lower::new(m, cx, Func::new(header), ret.clone());
        lower.debug_scope(symbol, body.span.start);
        lower.subst = subst;
        for ((name, ty), value) in params.iter().zip(incoming) {
            if is_aggregate(ty) {
                lower.bind(&name.name, value, ty.clone());
            } else if is_void(ty) {
                lower.bind(&name.name, String::new(), ty.clone());
            } else {
                let slot = lower.f.alloca(lower.env.layout(ty));
                lower.store_scalar(ty, &slot, &value);
                lower.bind(&name.name, slot, ty.clone());
            }
        }
        let want = !is_void(ret);
        let value = lower.block(body, want);
        if !lower.f.is_terminated() {
            lower.ret_value(value);
        }
        let text = lower.f.finish("unreachable");
        m.define(symbol, text);
    }

    /// Define a constant as a function without arguments.
    pub fn define_const(m: &mut Module<'_>, cx: &Ctx<'_>, symbol: &str, ty: &Ty, value: &Expr) {
        let subst = (Vec::new(), Vec::new());
        Self::define_const_in(m, cx, symbol, ty, value, subst, false);
    }

    /// [`Lower::define_const`] for the default of a field of a generic
    /// record at one instance (see [`Lower::define_fn_in`]).
    pub fn define_const_in(
        m: &mut Module<'_>,
        cx: &Ctx<'_>,
        symbol: &str,
        ty: &Ty,
        value: &Expr,
        subst: (Vec<String>, Vec<Ty>),
        instance: bool,
    ) {
        let (ret_abi, _) = abi(&[], ty);
        let param = if is_aggregate(ty) { "ptr %sret" } else { "" };
        let (linkage, comdat) = instance_linkage(m, symbol, instance);
        let header = format!(
            "define {linkage}{ret_abi} {}({param}){comdat}",
            global(symbol)
        );
        let mut lower = Lower::new(m, cx, Func::new(header), ty.clone());
        lower.debug_scope(symbol, value.span.start);
        lower.subst = subst;
        let v = lower.expr(value, true);
        lower.ret_value(v);
        let text = lower.f.finish("unreachable");
        m.define(symbol, text);
    }

    // ---- generic code ----------------------------------------------------

    /// `ty` with the type parameters of the instance being lowered
    /// replaced by its arguments.
    /// `ty` with this instance's type arguments substituted and references
    /// lowered to `Text` ([`crate::layout::repr`]).
    fn sub(&self, ty: &Ty) -> Ty {
        if self.subst.0.is_empty() {
            repr(ty)
        } else {
            repr(&ty.substitute(&self.subst.0, &self.subst.1))
        }
    }

    /// The type arguments of the generic use `e`, instantiated.
    fn instance_args(&self, e: &Expr, what: &str) -> Vec<Ty> {
        self.cx
            .types
            .instance(e)
            .unwrap_or_else(|| panic!("no instance of generic {what} at {:?}", e.span))
            .iter()
            .map(|t| self.sub(t))
            .collect()
    }

    /// The symbol and signature of fn `q` as used by `e`: the instance of a
    /// generic fn, requested in this module.
    fn fn_instance(&mut self, q: &QualName, e: &Expr) -> Option<(String, FnSig)> {
        let sig = self.env.fn_sig(q)?.clone();
        let params = self.env.generics(q).to_vec();
        if params.is_empty() {
            return Some((symbol(q), repr_sig(&sig)));
        }
        let args = self.instance_args(e, &symbol(q));
        let name = instance_symbol(&symbol(q), &args);
        self.m.instance(Instance {
            symbol: name.clone(),
            generic: Generic::Fn(q.clone()),
            args: args.clone(),
        });
        Some((name, substitute_sig(&sig, &params, &args)))
    }

    /// The symbol and signature of fn `q` at the type arguments `args`
    /// (empty for a fn without type parameters), requested in this module
    /// when generic: the `K.main` of an executable component
    /// (`docs/slice.md` section 14.5).
    pub fn fn_instance_with(&mut self, q: &QualName, args: &[Ty]) -> Option<(String, FnSig)> {
        let sig = self.env.fn_sig(q)?.clone();
        let params = self.env.generics(q).to_vec();
        if params.is_empty() {
            return Some((symbol(q), repr_sig(&sig)));
        }
        let args: Vec<Ty> = args.iter().map(|t| self.sub(t)).collect();
        let name = instance_symbol(&symbol(q), &args);
        self.m.instance(Instance {
            symbol: name.clone(),
            generic: Generic::Fn(q.clone()),
            args: args.clone(),
        });
        Some((name, substitute_sig(&sig, &params, &args)))
    }

    /// The symbol and signature of role `role` of `protocol` at `args`
    /// (empty for a role without type parameters), requested in this
    /// module when generic.
    pub fn role_instance(
        &mut self,
        protocol: &QualName,
        role: &str,
        args: &[Ty],
    ) -> Option<(String, FnSig)> {
        let sig = self.env.role_sig(protocol, role)?.clone();
        let base = format!("{}.{role}", symbol(protocol));
        let params = self.env.role_generics(protocol, role).to_vec();
        if params.is_empty() {
            return Some((base, repr_sig(&sig)));
        }
        let args: Vec<Ty> = args.iter().map(|t| self.sub(t)).collect();
        let name = instance_symbol(&base, &args);
        self.m.instance(Instance {
            symbol: name.clone(),
            generic: Generic::Role(protocol.clone(), role.to_string()),
            args: args.clone(),
        });
        Some((name, substitute_sig(&sig, &params, &args)))
    }

    /// Bind `name` to `value` of type `ty` in the current scope (a scalar
    /// gets a slot); for the resources of a generated `main`.
    pub fn bind_local(&mut self, name: &str, ty: &Ty, value: Val) {
        self.bind_value(name, ty, value, false);
    }

    /// A new store of `element`s bound to `name` (a `Store` resource of a
    /// generated `main`, `docs/llvm.md` section 6).
    pub fn create_store(&mut self, name: &str, element: &Ty) {
        let store = self.new_store(element);
        let ty = Ty::Store(Box::new(element.clone()));
        self.bind_local(name, &ty, store);
    }

    /// A new, empty store of `element`s; never freed.
    fn new_store(&mut self, element: &Ty) -> Val {
        let to = self.m.glue_of(Glue::ToHeap, element);
        let from = self.m.glue_of(Glue::FromHeap, element);
        let free = self.m.glue_of(Glue::FreeHeap, element);
        let layout = self.layout(element);
        Val::Scalar(self.f.val(format!(
            "call ptr @ignite_rt_store_new(ptr {to}, ptr {from}, ptr {free}, i64 {}, i64 {})",
            layout.size,
            layout.align.max(1)
        )))
    }

    /// `Store.new()` in a test: the element type comes from the call's type.
    fn store_new(&mut self, e: &Expr) -> Val {
        match self.ty(e) {
            Ty::Store(element) => self.new_store(&element),
            other => panic!("`Store.new()` of type {other:?}"),
        }
    }

    fn ret_value(&mut self, value: Val) {
        if value == Val::Never {
            self.f.term("unreachable");
            return;
        }
        let ret = self.ret.clone();
        if let Some(sret) = self.sret.clone() {
            self.store_value(&ret, &sret, value);
            self.release_owned(0);
            self.f.term("ret void");
        } else if let Some(t) = scalar_type(&ret) {
            let op = self.scalar(&value);
            self.release_owned(0);
            self.f.term(format!("ret {t} {op}"));
        } else {
            self.release_owned(0);
            self.f.term("ret void");
        }
    }

    // ---- scopes ----------------------------------------------------------

    fn bind(&mut self, name: &str, ptr: String, ty: Ty) {
        self.scopes
            .last_mut()
            .expect("a scope is open")
            .push(Local {
                name: name.to_string(),
                ptr,
                ty,
                owned: false,
                borrows: Vec::new(),
            });
    }

    // ---- stream ownership ------------------------------------------------

    /// Make the stream bindings `names` of the current block owners of
    /// their handles (`docs/llvm.md` section 4): registered with
    /// `tcp_own`, so a failure releases them, and released when the block
    /// is left. The block's mark is taken at its first owned binding.
    fn own_streams(&mut self, names: &[String]) {
        self.m.declare("ignite_rt_own_mark", "i64", &[]);
        self.m
            .declare("ignite_rt_own_release", "void", &["i64".into()]);
        self.m.declare("ignite_rt_tcp_own", "void", &["i64".into()]);
        if matches!(self.own_marks.last(), Some(None)) {
            let mark = self.f.val("call i64 @ignite_rt_own_mark()");
            *self.own_marks.last_mut().expect("a block is open") = Some(mark);
        }
        for name in names {
            let Some(local) = self
                .scopes
                .last_mut()
                .and_then(|scope| scope.iter_mut().rev().find(|l| l.name == *name))
            else {
                continue;
            };
            local.owned = true;
            let local = local.clone();
            let handle = self.load_scalar(&local.ty, &local.ptr);
            self.f
                .ins(format!("call void @ignite_rt_tcp_own(i64 {handle})"));
        }
    }

    /// Release the owned stream bindings of the open blocks from `depth`
    /// on: those of every block when leaving the function, those inside a
    /// loop for its `break` and `continue`.
    fn release_owned(&mut self, depth: usize) {
        let mark = self
            .own_marks
            .get(depth..)
            .and_then(|marks| marks.iter().flatten().next())
            .cloned();
        if let Some(mark) = mark {
            self.f
                .ins(format!("call void @ignite_rt_own_release(i64 {mark})"));
        }
    }

    /// The `continue` (`next`) or `break` target of the innermost loop,
    /// after releasing the owned streams bound inside it.
    fn leave_loop(&mut self, next: bool) -> String {
        let (latch, exit, depth) = self.loops.last().cloned().expect("inside a loop");
        self.release_owned(depth);
        if next { latch } else { exit }
    }

    fn lookup(&self, name: &str) -> Option<Local> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .find(|local| local.name == name)
            .cloned()
    }

    // ---- memory helpers --------------------------------------------------

    pub fn ty(&self, e: &Expr) -> Ty {
        self.sub(self.cx.types.type_of(e))
    }

    fn layout(&self, ty: &Ty) -> Layout {
        self.env.layout(ty)
    }

    pub fn gep(&mut self, ptr: &str, offset: u64) -> String {
        if offset == 0 {
            ptr.to_string()
        } else {
            self.f.val(format!(
                "getelementptr inbounds i8, ptr {ptr}, i64 {offset}"
            ))
        }
    }

    pub fn load_scalar(&mut self, ty: &Ty, ptr: &str) -> String {
        let mt = memory_type(ty).expect("a scalar type");
        let align = self.layout(ty).align;
        let v = self.f.val(format!("load {mt}, ptr {ptr}, align {align}"));
        if *ty == Ty::Bool {
            self.f.val(format!("trunc i8 {v} to i1"))
        } else {
            v
        }
    }

    pub fn store_scalar(&mut self, ty: &Ty, ptr: &str, op: &str) {
        let align = self.layout(ty).align;
        if *ty == Ty::Bool {
            let z = self.f.val(format!("zext i1 {op} to i8"));
            self.f.ins(format!("store i8 {z}, ptr {ptr}, align 1"));
        } else {
            let mt = memory_type(ty).expect("a scalar type");
            self.f
                .ins(format!("store {mt} {op}, ptr {ptr}, align {align}"));
        }
    }

    pub fn memcpy(&mut self, dst: &str, src: &str, size: u64) {
        if size > 0 {
            self.f.ins(format!(
                "call void @llvm.memcpy.p0.p0.i64(ptr {dst}, ptr {src}, i64 {size}, i1 false)"
            ));
        }
    }

    /// Deep copy of the `ty` at `src` into `dst`.
    pub fn copy(&mut self, ty: &Ty, dst: &str, src: &str) {
        if self.env.needs_glue(ty) {
            let glue = self.m.glue(ty);
            self.f
                .ins(format!("call void {glue}(ptr {dst}, ptr {src})"));
        } else {
            let size = self.layout(ty).size;
            self.memcpy(dst, src, size);
        }
    }

    pub fn temp(&mut self, ty: &Ty) -> String {
        let layout = self.layout(ty);
        self.f.alloca(layout)
    }

    /// Store `value` (of type `ty`) into the memory at `dst`: owned values
    /// are moved, borrowed ones copied.
    pub fn store_value(&mut self, ty: &Ty, dst: &str, value: Val) {
        match value {
            Val::Unit | Val::Never => {}
            Val::Scalar(op) => {
                if !is_void(ty) {
                    self.store_scalar(ty, dst, &op)
                }
            }
            Val::Ptr(src, true) => {
                let size = self.layout(ty).size;
                self.memcpy(dst, &src, size);
            }
            Val::Ptr(src, false) => self.copy(ty, dst, &src),
        }
    }

    /// `value` as an owned value.
    pub fn owned(&mut self, ty: &Ty, value: Val) -> Val {
        match value {
            Val::Ptr(src, false) => {
                let t = self.temp(ty);
                self.copy(ty, &t, &src);
                Val::Ptr(t, true)
            }
            other => other,
        }
    }

    pub fn scalar(&self, value: &Val) -> String {
        match value {
            Val::Scalar(op) => op.clone(),
            Val::Never => "poison".to_string(),
            other => panic!("expected a scalar, got {other:?}"),
        }
    }

    pub fn ptr(&self, value: &Val) -> String {
        match value {
            Val::Ptr(p, _) => p.clone(),
            Val::Never => "poison".to_string(),
            other => panic!("expected an aggregate, got {other:?}"),
        }
    }

    /// The value of type `ty` stored at `ptr`: loaded when scalar.
    fn at(&mut self, ty: &Ty, ptr: String, owned: bool) -> Val {
        if is_aggregate(ty) {
            Val::Ptr(ptr, owned)
        } else if is_void(ty) {
            Val::Unit
        } else {
            Val::Scalar(self.load_scalar(ty, &ptr))
        }
    }

    /// The result in `slot` of an `if` or `match` of type `ty`.
    fn slot_value(&mut self, ty: &Ty, slot: Option<String>) -> Val {
        match slot {
            Some(slot) => self.at(ty, slot, true),
            None if *ty == Ty::Never => Val::Never,
            None => Val::Unit,
        }
    }

    /// A `Text` pointing at a literal's global bytes, not copied: for
    /// reading only (comparisons, prefixes, messages).
    pub fn literal_ref(&mut self, text: &[u8]) -> String {
        let t = self.temp(&Ty::Text);
        let data = if text.is_empty() {
            "@ignite.empty".to_string()
        } else {
            self.m.string(text)
        };
        self.f.ins(format!("store ptr {data}, ptr {t}, align 8"));
        let len = text.len();
        let lenp = self.gep(&t, 8);
        self.f.ins(format!("store i64 {len}, ptr {lenp}, align 8"));
        let capp = self.gep(&t, 16);
        self.f.ins(format!("store i64 {len}, ptr {capp}, align 8"));
        t
    }

    /// A string literal as a value: its bytes copied into the arena.
    pub fn text_literal(&mut self, text: &[u8]) -> Val {
        let view = self.literal_ref(text);
        let t = self.temp(&Ty::Text);
        self.copy(&Ty::Text, &t, &view);
        Val::Ptr(t, true)
    }

    /// An empty list or text.
    fn empty_list(&mut self) -> Val {
        let t = self.temp(&Ty::Text);
        self.f
            .ins(format!("store ptr @ignite.empty, ptr {t}, align 8"));
        let lenp = self.gep(&t, 8);
        self.f.ins(format!("store i64 0, ptr {lenp}, align 8"));
        let capp = self.gep(&t, 16);
        self.f.ins(format!("store i64 0, ptr {capp}, align 8"));
        Val::Ptr(t, true)
    }

    fn list_len(&mut self, list: &str) -> String {
        let p = self.gep(list, 8);
        self.f.val(format!("load i64, ptr {p}, align 8"))
    }

    // ---- calls -----------------------------------------------------------

    /// An argument in a value position: the typed operand, or `None` for
    /// `Unit`.
    fn arg(&mut self, ty: &Ty, value: Val) -> Option<String> {
        if is_aggregate(ty) {
            let v = self.owned(ty, value);
            Some(format!("ptr {}", self.ptr(&v)))
        } else {
            scalar_type(ty).map(|t| format!("{t} {}", self.scalar(&value)))
        }
    }

    /// Call `callee` (a global or a register) with argument operands.
    pub fn call(&mut self, callee: &str, ret: &Ty, args: Vec<Option<String>>) -> Val {
        self.call_with_env(callee, None, ret, args)
    }

    /// The code and environment pointers of the fn value at `fv`, loaded
    /// now, so that a later argument that reassigns the fn value's place
    /// does not change the callee.
    fn fn_parts(&mut self, fv: &str) -> (String, String) {
        let code = self.f.val(format!("load ptr, ptr {fv}, align 8"));
        let at = self.gep(fv, 8);
        let env = self.f.val(format!("load ptr, ptr {at}, align 8"));
        (code, env)
    }

    /// Per parameter of the fn value `f` evaluates to, whether its callee is
    /// known to only read it (`docs/llvm.md` section 3.1): a closure literal
    /// (or a `let` bound to one) reads every parameter its body neither
    /// changes in place (`ownership::changed_in`) nor consumes
    /// (`TypedProgram::closure_consumed`: only a consumed closure parameter
    /// moves inside the body), and a named fn reads the parameters it does
    /// not consume. Empty when the callee is not known.
    fn known_borrows(&mut self, f: &Expr) -> Vec<bool> {
        match &f.kind {
            ExprKind::Closure(closure) => {
                // A parameter the body consumes (`docs/slice.md` 20.3.1) may
                // be moved into a callee that changes it in place, so the
                // caller passes a copy, as it does to a modified one.
                let modified = ownership::changed_in(&closure.body, Some(self.cx.types));
                closure
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        !modified.contains(p.name.name.as_str())
                            && !self.cx.types.closure_consumed(f, i)
                    })
                    .collect()
            }
            ExprKind::Name(id) => {
                if let Some(local) = self.lookup(&id.name) {
                    return local.borrows;
                }
                match self.cx.types.callee(f).cloned() {
                    Some(Callee::Fn(q)) => match self.fn_instance(&q, f) {
                        Some((symbol, sig)) => (0..sig.params.len())
                            .map(|i| self.env.reads_only(&symbol, i))
                            .collect(),
                        None => Vec::new(),
                    },
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }

    /// The operands of a call through a fn value, `None` when an argument
    /// does not return. A borrowed place goes as its own pointer to a
    /// parameter `borrows` marks (the callee only reads it), unless a later
    /// argument modifies a variable or mentions the place's root (it could
    /// move the place into a callee that changes it); every other
    /// aggregate is copied when it is evaluated, or moved.
    fn value_operands(
        &mut self,
        args: &[Expr],
        params: &[Ty],
        borrows: &[bool],
    ) -> Option<Vec<Option<String>>> {
        let mut operands = Vec::new();
        for (i, (arg, ty)) in args.iter().zip(params).enumerate() {
            let v = self.expr(arg, true);
            if v == Val::Never {
                return None;
            }
            let root = ownership::memory_root(arg, Some(self.cx.types));
            let borrow = is_aggregate(ty)
                && borrows.get(i).copied().unwrap_or(false)
                && matches!(v, Val::Ptr(_, false))
                && args[i + 1..].iter().all(|later| {
                    ownership::changed_in(later, Some(self.cx.types)).is_empty()
                        && root
                            .as_ref()
                            .is_none_or(|n| !crate::region::mentions(later, n))
                });
            if borrow {
                operands.push(Some(format!("ptr {}", self.ptr(&v))));
            } else {
                operands.push(self.arg(ty, v));
            }
        }
        Some(operands)
    }

    /// Call a fn value (`docs/llvm.md` section 3): its code with its
    /// environment first.
    fn call_value(&mut self, parts: &(String, String), ret: &Ty, args: Vec<Option<String>>) -> Val {
        let (code, env) = parts;
        self.call_with_env(code, Some(format!("ptr {env}")), ret, args)
    }

    /// A fn value of the function `symbol` with `sig`: a thunk that ignores
    /// the environment, and a null environment.
    fn fn_value(&mut self, symbol: &str, sig: &FnSig) -> Val {
        let callee = self.function(symbol, &sig.params, &sig.ret);
        let thunk = format!("fnval.{symbol}");
        let (ret, params) = abi(&sig.params, &sig.ret);
        let typed: Vec<String> = params
            .iter()
            .enumerate()
            .map(|(i, t)| format!("{t} %a{i}"))
            .collect();
        let mut header = vec!["ptr %env".to_string()];
        header.extend(typed.iter().cloned());
        let call = format!("call {ret} {callee}({})", typed.join(", "));
        let body = if ret == "void" {
            format!("  {call}\n  ret void\n")
        } else {
            format!("  %r = {call}\n  ret {ret} %r\n")
        };
        let text = format!(
            "define internal {ret} {}({}) {{\nentry:\n{body}}}\n",
            global(&thunk),
            header.join(", ")
        );
        self.m.helper(&thunk, &text);
        let ty = Ty::Fn(sig.params.clone(), Box::new(sig.ret.clone()));
        let t = self.temp(&ty);
        self.f
            .ins(format!("store ptr {}, ptr {t}, align 8", global(&thunk)));
        let at = self.gep(&t, 8);
        self.f.ins(format!("store ptr null, ptr {at}, align 8"));
        Val::Ptr(t, true)
    }

    /// [`Lower::call`] with an optional environment operand before the
    /// result pointer and the arguments.
    fn call_with_env(
        &mut self,
        callee: &str,
        env: Option<String>,
        ret: &Ty,
        args: Vec<Option<String>>,
    ) -> Val {
        let mut list: Vec<String> = env.into_iter().collect();
        let result = if is_aggregate(ret) {
            let t = self.temp(ret);
            list.push(format!("ptr {t}"));
            Some(t)
        } else {
            None
        };
        list.extend(args.into_iter().flatten());
        let args = list.join(", ");
        if let Some(t) = result {
            self.f.ins(format!("call void {callee}({args})"));
            Val::Ptr(t, true)
        } else if let Some(t) = scalar_type(ret) {
            Val::Scalar(self.f.val(format!("call {t} {callee}({args})")))
        } else {
            self.f.ins(format!("call void {callee}({args})"));
            if *ret == Ty::Never {
                self.f.term("unreachable");
                Val::Never
            } else {
                Val::Unit
            }
        }
    }

    /// The global of a function with this signature, declared when it is
    /// defined elsewhere.
    pub fn function(&mut self, symbol: &str, params: &[Ty], ret: &Ty) -> String {
        let (r, p) = abi(params, ret);
        self.m.declare(symbol, &r, &p);
        global(symbol)
    }

    /// Call a function with argument expressions.
    fn call_exprs(&mut self, e: &Expr, symbol: &str, sig: &FnSig, args: &[Expr]) -> Val {
        let args: Vec<&Expr> = args.iter().collect();
        self.call_with(Some(e), symbol, sig, &args)
    }

    /// [`Lower::call_exprs`] with the argument expressions listed, for a
    /// UFCS call whose receiver is the first argument. `e` is the call
    /// expression, for named arguments (`docs/slice.md` section 14.2).
    ///
    /// A borrowed aggregate goes to a parameter the callee does not consume
    /// without a copy (`docs/llvm.md` section 3), unless a later argument
    /// modifies a variable: then it could change the place before the call.
    /// A moved place ([`Lower::moved`]) is owned and goes to a consumed
    /// parameter as its own pointer.
    fn call_with(&mut self, e: Option<&Expr>, symbol: &str, sig: &FnSig, args: &[&Expr]) -> Val {
        if let Some(perm) = e
            .and_then(|e| self.cx.types.named_args(e))
            .map(<[usize]>::to_vec)
        {
            return self.call_named(symbol, sig, args, &perm);
        }
        let callee = self.function(symbol, &sig.params, &sig.ret);
        let mut operands = Vec::new();
        for (i, (arg, ty)) in args.iter().zip(&sig.params).enumerate() {
            let v = self.expr(arg, true);
            if v == Val::Never {
                return Val::Never;
            }
            let borrow = matches!(v, Val::Ptr(_, false))
                && self.env.reads_only(symbol, i)
                && args[i + 1..]
                    .iter()
                    .all(|later| ownership::changed_in(later, Some(self.cx.types)).is_empty());
            if borrow {
                operands.push(Some(format!("ptr {}", self.ptr(&v))));
            } else {
                operands.push(self.arg(ty, v));
            }
        }
        self.call(&callee, &sig.ret, operands)
    }

    /// A call with named arguments (`docs/slice.md` section 14.2). `args` is
    /// in source order (a UFCS receiver first); `perm[i]` is the index among
    /// the written arguments of the one for the parameter after any
    /// receiver. The arguments are evaluated in source order into values,
    /// then passed in parameter order, so the reorder is invisible to their
    /// effects. The borrow shortcut is skipped: a copy of every aggregate is
    /// correct whatever the order. Each borrowed value is copied when it is
    /// evaluated, as on the positional path: a later argument may move the
    /// same place into a callee that changes it (`docs/slice.md` 20.3.5).
    fn call_named(&mut self, symbol: &str, sig: &FnSig, args: &[&Expr], perm: &[usize]) -> Val {
        let callee = self.function(symbol, &sig.params, &sig.ret);
        let offset = args.len() - perm.len();
        let source = |i: usize| {
            if i < offset {
                i
            } else {
                offset + perm[i - offset]
            }
        };
        let mut param_of = vec![None; args.len()];
        for i in 0..sig.params.len() {
            param_of[source(i)] = Some(i);
        }
        let mut values = Vec::with_capacity(args.len());
        for (arg, param) in args.iter().zip(&param_of) {
            let v = self.expr(arg, true);
            if v == Val::Never {
                return Val::Never;
            }
            values.push(match param {
                Some(i) => self.owned(&sig.params[*i], v),
                None => v,
            });
        }
        let mut operands = Vec::with_capacity(sig.params.len());
        for (i, ty) in sig.params.iter().enumerate() {
            operands.push(self.arg(ty, values[source(i)].clone()));
        }
        self.call(&callee, &sig.ret, operands)
    }

    /// The variable a place expression (`x`, `x.a.b`) is rooted at.
    fn root(e: &Expr) -> Option<&str> {
        match &e.kind {
            ExprKind::Name(id) => Some(&id.name),
            ExprKind::Field { receiver, .. } => Self::root(receiver),
            _ => None,
        }
    }

    /// Whether `place` is a place none of `bodies` modifies, so that a
    /// borrowed value of it stays valid and unchanged while they run.
    fn unchanged_in<'x>(&self, place: &Expr, bodies: impl Iterator<Item = &'x Expr>) -> bool {
        let Some(root) = Self::root(place) else {
            return false;
        };
        let mut bodies = bodies;
        bodies.all(|body| !ownership::changed_in(body, Some(self.cx.types)).contains(root))
    }

    /// The symbol and signature `callee` of the call `e` calls: for generic
    /// code, the instance `e` uses.
    fn sig_of(&mut self, callee: &Callee, e: &Expr) -> Option<(String, FnSig)> {
        match callee {
            Callee::Fn(q) => self.fn_instance(q, e),
            Callee::Role { protocol, role } => {
                let args = if self.env.role_generics(protocol, role).is_empty() {
                    Vec::new()
                } else {
                    self.instance_args(e, role)
                };
                self.role_instance(protocol, role, &args)
            }
            _ => None,
        }
    }

    // ---- statements ------------------------------------------------------

    /// A block; its value when `want` and the last statement is an
    /// expression.
    pub fn block(&mut self, block: &Block, want: bool) -> Val {
        self.scopes.push(Vec::new());
        self.own_marks.push(None);
        let mut value = Val::Unit;
        for (i, stmt) in block.stmts.iter().enumerate() {
            let last = i + 1 == block.stmts.len();
            let previous = self.locate(stmt.span.start);
            if let StmtKind::Expr(e) = &stmt.kind
                && let ExprKind::Spawn(call) = &e.kind
            {
                self.moved_streams = crate::region::moved_to_task(&block.stmts, i, call);
            }
            match &stmt.kind {
                StmtKind::Expr(e) if last && want => value = self.expr(e, true),
                _ => self.stmt(stmt),
            }
            if matches!(
                stmt.kind,
                StmtKind::Let { .. } | StmtKind::LetPattern { .. }
            ) && !self.f.is_terminated()
            {
                let owned = crate::own::owned(&block.stmts, i, self.cx.types);
                if !owned.is_empty() {
                    self.own_streams(&owned);
                    for name in owned {
                        for call in crate::own::handovers(&block.stmts[i + 1..], &name) {
                            self.handovers.push(Handover {
                                call: expr_key(call),
                                name: name.clone(),
                            });
                        }
                    }
                }
            }
            if let Some(previous) = previous {
                self.f.restore_location(previous);
            }
        }
        if let Some(Some(mark)) = self.own_marks.pop()
            && !self.f.is_terminated()
        {
            self.f
                .ins(format!("call void @ignite_rt_own_release(i64 {mark})"));
        }
        self.scopes.pop();
        if self.f.is_terminated() && value == Val::Unit {
            Val::Never
        } else {
            value
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::ForPattern {
                pattern,
                iter,
                body,
            } => self.for_each(iter, body, ForBind::Pattern(pattern)),
            StmtKind::LetPattern { pattern, value, .. } => {
                let ty = self.ty(value);
                let v = self.expr(value, true);
                if v == Val::Never {
                    return;
                }
                let v = self.owned(&ty, v);
                let (bound, fail) = (self.f.label(), self.f.label());
                self.pattern(pattern, &v, &ty, &fail);
                self.f.br(&bound);
                self.f.place(&fail);
                self.fail_match_at(pattern.span.start);
                self.f.term("unreachable");
                self.f.place(&bound);
            }
            StmtKind::Let {
                name,
                value,
                mutable,
                ..
            } => {
                let ty = self
                    .cx
                    .types
                    .binding_type(name.span)
                    .map(|t| self.sub(t))
                    .unwrap_or_else(|| self.ty(value));
                let v = self.expr(value, true);
                if is_void(&ty) {
                    self.bind(&name.name, String::new(), ty);
                } else {
                    let slot = self.temp(&ty);
                    self.store_value(&ty, &slot, v);
                    self.bind(&name.name, slot, ty);
                    if !mutable && matches!(value.kind, ExprKind::Closure(_)) {
                        let borrows = self.known_borrows(value);
                        if let Some(local) = self.scopes.last_mut().and_then(|s| s.last_mut()) {
                            local.borrows = borrows;
                        }
                    }
                }
            }
            StmtKind::Assign { target, value } => {
                let local = self
                    .lookup(&target.base.name)
                    .unwrap_or_else(|| panic!("unbound `{}`", target.base.name));
                let mut ty = local.ty.clone();
                let v = self.expr(value, true);
                let v = self.owned(&ty_of_place(self.env, &ty, &target.fields), v);
                let mut ptr = local.ptr.clone();
                for field in &target.fields {
                    let (offset, field_ty) = self
                        .env
                        .field(&ty, &field.name)
                        .unwrap_or_else(|| panic!("no field `{}`", field.name));
                    let boxed = self.env.field_boxed(&ty, &field.name);
                    ptr = self.gep(&ptr, offset);
                    ptr = self.deref_position(ptr, boxed);
                    ty = field_ty;
                }
                if !is_void(&ty) {
                    self.store_value(&ty, &ptr, v);
                }
            }
            StmtKind::Return(value) => {
                let v = match value {
                    Some(e) => self.expr(e, true),
                    None => Val::Unit,
                };
                if !self.f.is_terminated() || v != Val::Never {
                    self.ret_value(v);
                }
            }
            StmtKind::While { cond, body } => {
                let region = self.region_enter(body);
                let (head, inside, latch, exit) = (
                    self.f.label(),
                    self.f.label(),
                    self.f.label(),
                    self.f.label(),
                );
                self.f.place(&head);
                let c = self.expr(cond, true);
                let c = self.scalar(&c);
                self.f.cond_br(&c, &inside, &exit);
                self.f.place(&inside);
                self.loops
                    .push((latch.clone(), exit.clone(), self.own_marks.len()));
                self.block(body, false);
                self.loops.pop();
                self.f.place(&latch);
                self.region_back_edge(region.as_ref(), &head);
                self.f.place(&exit);
            }
            StmtKind::Loop { body } => {
                let region = self.region_enter(body);
                let (head, latch, exit) = (self.f.label(), self.f.label(), self.f.label());
                self.f.place(&head);
                self.loops
                    .push((latch.clone(), exit.clone(), self.own_marks.len()));
                self.block(body, false);
                self.loops.pop();
                self.f.place(&latch);
                self.region_back_edge(region.as_ref(), &head);
                self.f.place(&exit);
            }
            StmtKind::For { name, iter, body } => self.for_loop(name, iter, body),
            StmtKind::Break => {
                let exit = self.leave_loop(false);
                self.f.br(&exit);
            }
            StmtKind::Continue => {
                let next = self.leave_loop(true);
                self.f.br(&next);
            }
            StmtKind::Expr(e) => {
                self.expr(e, false);
            }
            StmtKind::Assert { cond, message } => {
                self.assert(stmt.span.start, cond, message.as_ref())
            }
        }
    }

    fn for_loop(&mut self, name: &Ident, iter: &Expr, body: &Block) {
        self.for_each(iter, body, ForBind::Name(name));
    }

    /// Iterate `iter` in order (a list by element, a map by pair in key
    /// order, text by byte) and run `body` with the element bound by `bind`.
    fn for_each(&mut self, iter: &Expr, body: &Block, bind: ForBind<'_>) {
        let list_ty = self.ty(iter);
        let element = match &list_ty {
            Ty::List(t) => (**t).clone(),
            Ty::Map(key, value) => map_pair(key, value),
            Ty::Text => Ty::Byte,
            other => panic!("for over {other:?}"),
        };
        let v = self.expr(iter, true);
        let names = ownership::changed_in_block(body, Some(self.cx.types));
        let keep = Self::root(iter).is_some_and(|root| !names.contains(root));
        let v = if keep { v } else { self.owned(&list_ty, v) };
        let list = self.ptr(&v);
        let counter = self.f.alloca(Layout::scalar(8));
        self.f.ins(format!("store i64 0, ptr {counter}, align 8"));
        let region = self.region_enter(body);
        let (head, inside, step, exit) = (
            self.f.label(),
            self.f.label(),
            self.f.label(),
            self.f.label(),
        );
        self.f.place(&head);
        let i = self.f.val(format!("load i64, ptr {counter}, align 8"));
        let len = self.list_len(&list);
        let more = self.f.val(format!("icmp slt i64 {i}, {len}"));
        self.f.cond_br(&more, &inside, &exit);
        self.f.place(&inside);
        let size = self.layout(&element).size;
        let data = self.f.val(format!("load ptr, ptr {list}, align 8"));
        let at = self.f.val(format!("mul i64 {i}, {size}"));
        let elem = self
            .f
            .val(format!("getelementptr inbounds i8, ptr {data}, i64 {at}"));
        self.scopes.push(Vec::new());
        match bind {
            ForBind::Name(name) => {
                self.bind_value(&name.name, &element, Val::Ptr(elem.clone(), false), true);
            }
            ForBind::Pattern(pattern) => {
                let (bound, fail) = (self.f.label(), self.f.label());
                self.pattern(pattern, &Val::Ptr(elem.clone(), false), &element, &fail);
                self.f.br(&bound);
                self.f.place(&fail);
                self.fail_match_at(pattern.span.start);
                self.f.term("unreachable");
                self.f.place(&bound);
            }
        }
        self.loops
            .push((step.clone(), exit.clone(), self.own_marks.len()));
        self.block(body, false);
        self.loops.pop();
        self.scopes.pop();
        self.f.place(&step);
        let i = self.f.val(format!("load i64, ptr {counter}, align 8"));
        let next = self.f.val(format!("add i64 {i}, 1"));
        self.f
            .ins(format!("store i64 {next}, ptr {counter}, align 8"));
        self.region_back_edge(region.as_ref(), &head);
        self.f.place(&exit);
    }

    /// At a loop's entry: the region of a loop with `body`, or none when
    /// the loop runs inside an expression with pending values (`nested`).
    /// A carried variable is one declared outside the loop that `body`
    /// modifies and whose type holds arena pointers.
    fn region_enter(&mut self, body: &Block) -> Option<Region> {
        if self.nested > 0 {
            return None;
        }
        let mut carried = Vec::new();
        for name in ownership::changed_in_block(body, Some(self.cx.types)) {
            if let Some(local) = self.lookup(name)
                && self.env.needs_glue(&local.ty)
            {
                let copy = self.temp(&local.ty);
                carried.push((local, copy));
            }
        }
        let mark = self.f.val("call i64 @ignite.arena_mark()");
        let kept = (!carried.is_empty()).then(|| {
            let kept = self.f.alloca(Layout::scalar(8));
            self.f.ins(format!("store i64 0, ptr {kept}, align 8"));
            kept
        });
        Some(Region {
            mark,
            kept,
            carried,
        })
    }

    /// A loop's back-edge to `head`. Without carried variables, release the
    /// region. With them, when a release is due (`ignite.arena_due`):
    /// copy them into the scratch arena, release, copy them back, clear the
    /// scratch arena and record what was kept.
    fn region_back_edge(&mut self, region: Option<&Region>, head: &str) {
        let Some(region) = region else {
            self.f.br(head);
            return;
        };
        let mark = &region.mark;
        let Some(kept) = &region.kept else {
            self.f
                .ins(format!("call void @ignite.arena_release(i64 {mark})"));
            self.f.br(head);
            return;
        };
        let before = self.f.val(format!("load i64, ptr {kept}, align 8"));
        let due = self.f.val(format!(
            "call i1 @ignite.arena_due(i64 {mark}, i64 {before})"
        ));
        let compact = self.f.label();
        self.f.cond_br(&due, &compact, head);
        self.f.place(&compact);
        self.f.ins("call void @ignite_rt_arena_scratch(i8 1)");
        for (local, copy) in &region.carried {
            self.copy(&local.ty, copy, &local.ptr);
        }
        self.f.ins("call void @ignite_rt_arena_scratch(i8 0)");
        self.f
            .ins(format!("call void @ignite_rt_arena_release(i64 {mark})"));
        for (local, copy) in &region.carried {
            self.copy(&local.ty, &local.ptr, copy);
        }
        self.f.ins("call void @ignite_rt_arena_scratch_clear()");
        let top = self.f.val("call i64 @ignite.arena_mark()");
        let now = self.f.val(format!("sub i64 {top}, {mark}"));
        self.f.ins(format!("store i64 {now}, ptr {kept}, align 8"));
        self.f.br(head);
    }

    /// Bind an immutable name to a value. An aggregate is bound in place
    /// (`in_memory`: `value` points at memory of type `ty`); a scalar gets
    /// a slot.
    fn bind_value(&mut self, name: &str, ty: &Ty, value: Val, in_memory: bool) {
        if is_void(ty) {
            self.bind(name, String::new(), ty.clone());
        } else if is_aggregate(ty) {
            let p = self.ptr(&value);
            self.bind(name, p, ty.clone());
        } else {
            let op = if in_memory {
                let p = self.ptr(&value);
                self.load_scalar(ty, &p)
            } else {
                self.scalar(&value)
            };
            let slot = self.temp(ty);
            self.store_scalar(ty, &slot, &op);
            self.bind(name, slot, ty.clone());
        }
    }

    // ---- expressions -----------------------------------------------------

    /// Lower `e`. `want` is false when the value of an `if`, `match` or
    /// block is not used.
    pub fn expr(&mut self, e: &Expr, want: bool) -> Val {
        let pending = !matches!(
            e.kind,
            ExprKind::Block(_)
                | ExprKind::If { .. }
                | ExprKind::Match { .. }
                | ExprKind::Try(_)
                | ExprKind::Await(_)
        );
        if pending {
            self.nested += 1;
        }
        let previous = self.locate(e.span.start);
        let value = self.expr_kind(e, want);
        if let Some(previous) = previous {
            self.f.restore_location(previous);
        }
        if pending {
            self.nested -= 1;
        }
        self.moved(e, value)
    }

    /// A place in a move position that is dead after it
    /// (`TypedProgram::moves`, `docs/slice.md` 20.3.5) as an owned value:
    /// every value position then takes it as it takes a temporary, passed
    /// as the place's own pointer to an owned parameter or through a fn
    /// value, or stored with a shallow `memcpy`, without `copy.` glue.
    /// Nothing reads the place before it is reassigned or goes out of
    /// scope, so the taker may change it in place.
    fn moved(&self, e: &Expr, value: Val) -> Val {
        match value {
            Val::Ptr(p, false) if self.cx.types.moves(e) => Val::Ptr(p, true),
            other => other,
        }
    }

    fn expr_kind(&mut self, e: &Expr, want: bool) -> Val {
        if let Some(reference) = self.cx.types.reference(e) {
            return self.text_literal(reference.text().as_bytes());
        }
        match &e.kind {
            ExprKind::Quantity { .. } => {
                let (_, value) = self
                    .cx
                    .types
                    .quantity(e)
                    .unwrap_or_else(|| panic!("untyped quantity literal at {:?}", e.span));
                Val::Scalar(value.to_string())
            }
            ExprKind::NamedArg { value, .. } => self.expr(value, want),
            ExprKind::Brace { entries } => self.brace(e, entries),
            ExprKind::Closure(closure) => self.closure(e, closure),
            ExprKind::RecordUpdate { fields, base, .. } => self.record_update(e, fields, base),
            ExprKind::TupleField { receiver, index } => {
                let elements = match self.ty(receiver) {
                    Ty::Tuple(elements) => elements,
                    other => panic!("tuple field of {other:?}"),
                };
                let base = self.expr(receiver, true);
                let (p, owned) = match &base {
                    Val::Ptr(p, owned) => (p.clone(), *owned),
                    _ => return Val::Never,
                };
                let (offsets, _) = self.env.tuple_layout(&elements);
                let at = self.gep(&p, offsets[*index]);
                self.at(&elements[*index], at, owned)
            }
            ExprKind::Break => {
                let exit = self.leave_loop(false);
                self.f.br(&exit);
                Val::Never
            }
            ExprKind::Continue => {
                let next = self.leave_loop(true);
                self.f.br(&next);
                Val::Never
            }
            ExprKind::Tuple(items) if items.is_empty() => Val::Unit,
            ExprKind::Tuple(items) => self.tuple(e, items),
            ExprKind::Return(value) => {
                let v = match value {
                    Some(inner) => self.expr(inner, true),
                    None => Val::Unit,
                };
                if !self.f.is_terminated() || v != Val::Never {
                    self.ret_value(v);
                }
                Val::Never
            }
            ExprKind::Int(v) => match self.ty(e) {
                Ty::Byte => Val::Scalar(((*v as u8) as i8).to_string()),
                _ => Val::Scalar(v.to_string()),
            },
            ExprKind::Str(s) => self.text_literal(s.as_bytes()),
            ExprKind::Bool(b) => Val::Scalar(b.to_string()),
            ExprKind::Name(id) => self.name(id, e),
            ExprKind::Record { fields, .. } => {
                let ty = self.ty(e);
                let inits: Vec<(&str, &Expr)> = fields
                    .iter()
                    .map(|init| (init.name.name.as_str(), &init.value))
                    .collect();
                self.record_literal(&ty, &inits)
            }
            ExprKind::List(items) => {
                let ty = self.ty(e);
                let element = match &ty {
                    Ty::List(t) => (**t).clone(),
                    other => panic!("list literal of type {other:?}"),
                };
                self.list_literal(&element, items)
            }
            ExprKind::Call { callee, args } => self.call_expr(e, callee, args),
            ExprKind::Method {
                receiver,
                name,
                args,
            } => self.method(e, receiver, name, args),
            ExprKind::Field { receiver, name } => self.field(e, receiver, name),
            ExprKind::Index { receiver, index } => {
                self.list_at(e, receiver, std::slice::from_ref(&**index))
            }
            ExprKind::Try(inner) => self.try_expr(e, inner),
            ExprKind::Await(inner) => self.expr(inner, want),
            ExprKind::Spawn(inner) => {
                self.spawn(inner);
                Val::Unit
            }
            ExprKind::Unary { op, operand } => {
                let v = self.expr(operand, true);
                let x = self.scalar(&v);
                match op {
                    UnaryOp::Not => Val::Scalar(self.f.val(format!("xor i1 {x}, true"))),
                    UnaryOp::BitNot => Val::Scalar(self.f.val(format!("xor i64 {x}, -1"))),
                    UnaryOp::Neg => {
                        let t = scalar_type(&self.ty(operand)).unwrap_or("i64");
                        if t == "i64" && !matches!(operand.kind, ExprKind::Int(_)) {
                            let min = self.f.val(format!("icmp eq i64 {x}, {}", i64::MIN));
                            self.fail_neg_if(&min, e, &x);
                        }
                        Val::Scalar(self.f.val(format!("sub {t} 0, {x}")))
                    }
                }
            }
            ExprKind::Binary { .. } => self.binary(e),
            ExprKind::Match { scrutinee, arms } => {
                let ty = self.ty(e);
                let sty = self.ty(scrutinee);
                let s = self.expr(scrutinee, true);
                let keep = self.unchanged_in(scrutinee, arms.iter().map(|arm| &arm.body));
                let s = if keep { s } else { self.owned(&sty, s) };
                if s == Val::Never {
                    return Val::Never;
                }
                let slot = (want && !is_void(&ty)).then(|| self.temp(&ty));
                let end = self.f.label();
                for arm in arms {
                    let next = self.f.label();
                    self.scopes.push(Vec::new());
                    self.pattern(&arm.pattern, &s, &sty, &next);
                    let v = self.expr(&arm.body, want);
                    if let Some(slot) = &slot {
                        self.store_value(&ty, slot, v);
                    }
                    self.scopes.pop();
                    self.f.br(&end);
                    self.f.place(&next);
                }
                self.fail_match(e);
                self.f.term("unreachable");
                self.f.place(&end);
                self.slot_value(&ty, slot)
            }
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => {
                let ty = self.ty(e);
                let c = self.expr(cond, true);
                let c = self.scalar(&c);
                let slot = (want && !is_void(&ty)).then(|| self.temp(&ty));
                let (yes, no, end) = (self.f.label(), self.f.label(), self.f.label());
                self.f.cond_br(&c, &yes, &no);
                self.f.place(&yes);
                let v = self.block(then, want);
                if let Some(slot) = &slot {
                    self.store_value(&ty, slot, v);
                }
                self.f.br(&end);
                self.f.place(&no);
                if let Some(other) = otherwise {
                    let v = self.expr(other, want);
                    if let Some(slot) = &slot {
                        self.store_value(&ty, slot, v);
                    }
                }
                self.f.br(&end);
                self.f.place(&end);
                self.slot_value(&ty, slot)
            }
            ExprKind::Block(block) => self.block(block, want),
        }
    }

    /// `(a, b)`: each element in a value position, stored at its offset in
    /// the tuple's C struct.
    /// A record literal of type `ty` with the fields `inits` given, in
    /// source order; every other field takes its default (`docs/slice.md`
    /// section 13.3).
    fn record_literal(&mut self, ty: &Ty, inits: &[(&str, &Expr)]) -> Val {
        self.record_with(ty, inits, &[])
    }

    /// A record of `ty` from the field expressions `inits`, the values
    /// `fixed` (which replace an init of the same field; for the generated
    /// `main` of `docs/slice.md` section 14.5: the `Endpoint` of a listener,
    /// an omitted `executable`, a `Signal` output) and the kind's defaults
    /// for every other field.
    pub fn record_with(
        &mut self,
        ty: &Ty,
        inits: &[(&str, &Expr)],
        fixed: &[(String, Val)],
    ) -> Val {
        let t = self.temp(ty);
        for (field, value) in inits {
            if fixed.iter().any(|(f, _)| f == field) {
                continue;
            }
            let (offset, field_ty) = self
                .env
                .field(ty, field)
                .unwrap_or_else(|| panic!("no field `{field}`"));
            let v = self.expr(value, true);
            let dst = self.gep(&t, offset);
            let boxed = self.env.field_boxed(ty, field);
            self.store_position(&field_ty, &dst, boxed, v);
        }
        for (field, v) in fixed {
            let (offset, field_ty) = self
                .env
                .field(ty, field)
                .unwrap_or_else(|| panic!("no field `{field}`"));
            let dst = self.gep(&t, offset);
            let boxed = self.env.field_boxed(ty, field);
            self.store_position(&field_ty, &dst, boxed, v.clone());
        }
        if let Ty::Record(record) | Ty::GenericRecord(record, _) = ty {
            let (offsets, _) = self.env.fields_layout(ty);
            let declared = self.env.fields_of(ty);
            for ((field, field_ty), offset) in declared.iter().zip(offsets) {
                if inits.iter().any(|(given, _)| given == field)
                    || fixed.iter().any(|(given, _)| given == field)
                {
                    continue;
                }
                let symbol = match ty {
                    Ty::GenericRecord(_, args) => {
                        let name = format!("{}.{field}", instance_symbol(&symbol(record), args));
                        self.m.instance(Instance {
                            symbol: name.clone(),
                            generic: Generic::Default(record.clone(), field.clone()),
                            args: args.clone(),
                        });
                        name
                    }
                    _ => default_symbol(record, field),
                };
                let callee = self.function(&symbol, &[], field_ty);
                let v = self.call(&callee, field_ty, Vec::new());
                let dst = self.gep(&t, offset);
                let boxed = self.env.field_boxed(ty, field);
                self.store_position(field_ty, &dst, boxed, v);
            }
        }
        Val::Ptr(t, true)
    }

    /// A brace literal (`docs/slice.md` section 14.3): a record literal of
    /// the expected record type, or a map literal whose entries are put in
    /// source order (a later duplicate key replaces the value). An
    /// identifier key is its name as `Text`; a qualified key is the text of
    /// the reference it denotes.
    fn brace(&mut self, e: &Expr, entries: &[BraceEntry]) -> Val {
        let kind = self
            .cx
            .types
            .brace_kind(e)
            .cloned()
            .unwrap_or_else(|| panic!("untyped brace literal at {:?}", e.span));
        match kind {
            BraceKind::Record(ty) => {
                let ty = self.sub(&ty);
                let inits: Vec<(&str, &Expr)> = entries
                    .iter()
                    .map(|entry| (entry.key.last().name.as_str(), &entry.value))
                    .collect();
                self.record_literal(&ty, &inits)
            }
            BraceKind::Map(key, value) => {
                let (key, value) = (self.sub(&key), self.sub(&value));
                let map = self.empty_list();
                let m = self.ptr(&map);
                for entry in entries {
                    let k = match self.cx.types.brace_key(&entry.key) {
                        Some(reference) => self.text_literal(reference.text().as_bytes()),
                        None => self.text_literal(entry.key.last().name.as_bytes()),
                    };
                    let v = self.expr(&entry.value, true);
                    if v == Val::Never {
                        return Val::Never;
                    }
                    let have = self.ty(&entry.value);
                    let v = self.coerce(&value, &have, v);
                    self.map_put(&key, &value, &m, k, v);
                }
                map
            }
        }
    }

    // ---- maps (docs/llvm.md section 3) -------------------------------------

    /// The key and value types of a map receiver.
    fn map_kv(&self, receiver: &Expr) -> (Ty, Ty) {
        match self.ty(receiver) {
            Ty::Map(key, value) => (*key, *value),
            other => panic!("map builtin on {other:?}"),
        }
    }

    /// Evaluate `arg` and coerce it to the map's key type.
    fn map_key_arg(&mut self, key: &Ty, arg: &Expr) -> Val {
        let v = self.expr(arg, true);
        let have = self.ty(arg);
        self.coerce(key, &have, v)
    }

    /// The record `I` of `module("p")` (`docs/slice.md` section 14.3): a
    /// value of the record type whose every field is a fn value of the
    /// function the typer bound it to.
    fn module_record(&mut self, e: &Expr) -> Val {
        let mr = self
            .cx
            .types
            .module_record(e)
            .cloned()
            .unwrap_or_else(|| panic!("no module record at {:?}", e.span));
        let ty = self.sub(&mr.record);
        let t = self.temp(&ty);
        for (field, q) in &mr.fns {
            let (offset, field_ty) = self
                .env
                .field(&ty, field)
                .unwrap_or_else(|| panic!("module record has no field `{field}`"));
            let sig = self
                .env
                .fn_sig(q)
                .map(repr_sig)
                .unwrap_or_else(|| panic!("no signature for {}", symbol(q)));
            let fv = self.fn_value(&symbol(q), &sig);
            let dst = self.gep(&t, offset);
            let boxed = self.env.field_boxed(&ty, field);
            self.store_position(&field_ty, &dst, boxed, fv);
        }
        let record = Val::Ptr(t, true);
        // `module("p")` where an `Option<record>` is expected types as
        // `Option`, so wrap the record in `Some` (P1, frontend fact 2).
        match self.ty(e) {
            option @ Ty::Option(_) => self.variant(&option, "Some", vec![record]),
            _ => record,
        }
    }

    /// A pointer to the memory of the key `k` of type `key`: an aggregate's
    /// own, a scalar's in a fresh slot.
    fn key_ptr(&mut self, key: &Ty, k: &Val) -> String {
        if is_aggregate(key) {
            self.ptr(k)
        } else {
            let slot = self.temp(key);
            let x = self.scalar(k);
            self.store_scalar(key, &slot, &x);
            slot
        }
    }

    /// `i64 map.find.<Map<K,V>>(ptr map, ptr key)`, defined once per
    /// module: a binary search over the pairs, giving the index of the pair
    /// holding `key`, or `-(insertion point) - 1` when there is none.
    fn map_find_fn(&mut self, key: &Ty, value: &Ty) -> String {
        let map_ty = Ty::Map(Box::new(key.clone()), Box::new(value.clone()));
        let symbol = format!("map.find.{}", crate::layout::mangle(&map_ty));
        if self.m.has(&symbol) {
            return global(&symbol);
        }
        let pair = map_pair(key, value);
        let size = self.layout(&pair).size.max(1);
        let mut f = Func::new(format!(
            "define internal i64 {}(ptr %map, ptr %key)",
            global(&symbol)
        ));
        let lenp = f.val("getelementptr inbounds i8, ptr %map, i64 8");
        let len = f.val(format!("load i64, ptr {lenp}, align 8"));
        let data = f.val("load ptr, ptr %map, align 8");
        let lo = f.alloca(Layout::scalar(8));
        let hi = f.alloca(Layout::scalar(8));
        f.ins(format!("store i64 0, ptr {lo}, align 8"));
        f.ins(format!("store i64 {len}, ptr {hi}, align 8"));
        let (head, probe, less, after, more, found, missing) = (
            f.label(),
            f.label(),
            f.label(),
            f.label(),
            f.label(),
            f.label(),
            f.label(),
        );
        f.br(&head);
        f.place(&head);
        let l = f.val(format!("load i64, ptr {lo}, align 8"));
        let h = f.val(format!("load i64, ptr {hi}, align 8"));
        let open = f.val(format!("icmp slt i64 {l}, {h}"));
        f.cond_br(&open, &probe, &missing);
        f.place(&probe);
        let sum = f.val(format!("add i64 {l}, {h}"));
        let mid = f.val(format!("lshr i64 {sum}, 1"));
        let at = f.val(format!("mul i64 {mid}, {size}"));
        let p = f.val(format!("getelementptr inbounds i8, ptr {data}, i64 {at}"));
        let c = crate::glue::compare(self.m, &mut f, key, &p, "%key");
        let neg = f.val(format!("icmp slt i32 {c}, 0"));
        f.cond_br(&neg, &less, &after);
        f.place(&less);
        let next = f.val(format!("add i64 {mid}, 1"));
        f.ins(format!("store i64 {next}, ptr {lo}, align 8"));
        f.br(&head);
        f.place(&after);
        let pos = f.val(format!("icmp sgt i32 {c}, 0"));
        f.cond_br(&pos, &more, &found);
        f.place(&more);
        f.ins(format!("store i64 {mid}, ptr {hi}, align 8"));
        f.br(&head);
        f.place(&found);
        f.term(format!("ret i64 {mid}"));
        f.place(&missing);
        let l = f.val(format!("load i64, ptr {lo}, align 8"));
        let r = f.val(format!("xor i64 {l}, -1"));
        f.term(format!("ret i64 {r}"));
        let text = f.finish("unreachable");
        self.m.helper(&symbol, &text);
        global(&symbol)
    }

    /// The index of the pair holding `k` in the map at `m`, or
    /// `-(insertion point) - 1`.
    fn map_find(&mut self, key: &Ty, value: &Ty, m: &str, k: &Val) -> String {
        let kp = self.key_ptr(key, k);
        let find = self.map_find_fn(key, value);
        self.f.val(format!("call i64 {find}(ptr {m}, ptr {kp})"))
    }

    /// The pointer to pair `index` of the map at `m` and the offset of the
    /// value inside a pair.
    fn map_pair_at(&mut self, key: &Ty, value: &Ty, m: &str, index: &str) -> (String, u64) {
        let pair = map_pair(key, value);
        let size = self.layout(&pair).size;
        let (offsets, _) = self.env.tuple_layout(&[key.clone(), value.clone()]);
        let data = self.f.val(format!("load ptr, ptr {m}, align 8"));
        let at = self.f.val(format!("mul i64 {index}, {size}"));
        let p = self
            .f
            .val(format!("getelementptr inbounds i8, ptr {data}, i64 {at}"));
        (p, offsets[1])
    }

    /// `m.put(k, v)` on the map at `m` (`docs/slice.md` section 14.3):
    /// replace the value under `k`, or insert the pair at its sorted
    /// position, moving the pairs after it up by one.
    fn map_put(&mut self, key: &Ty, value: &Ty, m: &str, k: Val, v: Val) {
        let k = self.owned(key, k);
        let v = self.owned(value, v);
        let index = self.map_find(key, value, m, &k);
        let found = self.f.val(format!("icmp sge i64 {index}, 0"));
        let (replace, insert, done) = (self.f.label(), self.f.label(), self.f.label());
        self.f.cond_br(&found, &replace, &insert);
        self.f.place(&replace);
        let (p, value_at) = self.map_pair_at(key, value, m, &index);
        let vp = self.gep(&p, value_at);
        self.store_value(value, &vp, v.clone());
        self.f.br(&done);
        self.f.place(&insert);
        let pair = map_pair(key, value);
        let layout = self.layout(&pair);
        let size = layout.size.max(1);
        let at = self.f.val(format!("xor i64 {index}, -1"));
        self.f.ins(format!(
            "call void @ignite.reserve(ptr {m}, i64 1, i64 {size}, i64 {})",
            layout.align
        ));
        let len = self.list_len(m);
        let (p, value_at) = self.map_pair_at(key, value, m, &at);
        let after = self
            .f
            .val(format!("getelementptr inbounds i8, ptr {p}, i64 {size}"));
        let moved = self.f.val(format!("sub i64 {len}, {at}"));
        let bytes = self.f.val(format!("mul i64 {moved}, {size}"));
        self.f.ins(format!(
            "call void @llvm.memmove.p0.p0.i64(ptr {after}, ptr {p}, i64 {bytes}, i1 false)"
        ));
        self.store_value(key, &p, k);
        let vp = self.gep(&p, value_at);
        self.store_value(value, &vp, v);
        let next = self.f.val(format!("add i64 {len}, 1"));
        let lenp = self.gep(m, 8);
        self.f.ins(format!("store i64 {next}, ptr {lenp}, align 8"));
        self.f.br(&done);
        self.f.place(&done);
    }

    /// `m.get(k) -> Option<V>`: a copy of the value under `k`.
    fn map_get(&mut self, key: &Ty, value: &Ty, m: &str, k: &Val) -> Val {
        let option = Ty::Option(Box::new(value.clone()));
        let layout = self.env.enum_layout(&option);
        let result = self.temp(&option);
        let index = self.map_find(key, value, m, k);
        let found = self.f.val(format!("icmp sge i64 {index}, 0"));
        let (some, none, done) = (self.f.label(), self.f.label(), self.f.label());
        self.f.cond_br(&found, &some, &none);
        self.f.place(&some);
        self.f.ins(format!("store i32 0, ptr {result}, align 4"));
        if let Some((_, payload)) = layout.variants[0].1.first() {
            let (p, value_at) = self.map_pair_at(key, value, m, &index);
            let vp = self.gep(&p, value_at);
            let dst = self.gep(&result, *payload);
            self.copy(value, &dst, &vp);
        }
        self.f.br(&done);
        self.f.place(&none);
        self.f.ins(format!("store i32 1, ptr {result}, align 4"));
        self.f.br(&done);
        self.f.place(&done);
        Val::Ptr(result, true)
    }

    /// `m.remove(k) -> Option<V>`: drop the pair under `k`, moving the pairs
    /// after it down by one, and return `Some` of its value (copied out
    /// before the move); `None` when there is none.
    fn map_remove(&mut self, key: &Ty, value: &Ty, m: &str, k: &Val) -> Val {
        let option = Ty::Option(Box::new(value.clone()));
        let layout = self.env.enum_layout(&option);
        let result = self.temp(&option);
        let index = self.map_find(key, value, m, k);
        let found = self.f.val(format!("icmp sge i64 {index}, 0"));
        let (drop, none, done) = (self.f.label(), self.f.label(), self.f.label());
        self.f.cond_br(&found, &drop, &none);
        self.f.place(&drop);
        self.f.ins(format!("store i32 0, ptr {result}, align 4"));
        if let Some((_, payload)) = layout.variants[0].1.first() {
            let (p, value_at) = self.map_pair_at(key, value, m, &index);
            let vp = self.gep(&p, value_at);
            let dst = self.gep(&result, *payload);
            self.copy(value, &dst, &vp);
        }
        let pair = map_pair(key, value);
        let size = self.layout(&pair).size.max(1);
        let len = self.list_len(m);
        let (p, _) = self.map_pair_at(key, value, m, &index);
        let after = self
            .f
            .val(format!("getelementptr inbounds i8, ptr {p}, i64 {size}"));
        let last = self.f.val(format!("sub i64 {len}, 1"));
        let moved = self.f.val(format!("sub i64 {last}, {index}"));
        let bytes = self.f.val(format!("mul i64 {moved}, {size}"));
        self.f.ins(format!(
            "call void @llvm.memmove.p0.p0.i64(ptr {p}, ptr {after}, i64 {bytes}, i1 false)"
        ));
        let lenp = self.gep(m, 8);
        self.f.ins(format!("store i64 {last}, ptr {lenp}, align 8"));
        self.f.br(&done);
        self.f.place(&none);
        self.f.ins(format!("store i32 1, ptr {result}, align 4"));
        self.f.br(&done);
        self.f.place(&done);
        Val::Ptr(result, true)
    }

    /// `m.keys() -> List<K>`: copies of the keys in key order.
    fn map_keys(&mut self, key: &Ty, value: &Ty, m: &str) -> Val {
        let pair = map_pair(key, value);
        let pair_size = self.layout(&pair).size.max(1);
        let layout = self.layout(key);
        let len = self.list_len(m);
        let bytes = self.f.val(format!("mul i64 {len}, {}", layout.size));
        let data = self.f.val(format!(
            "call ptr @ignite.alloc(i64 {bytes}, i64 {})",
            layout.align
        ));
        let pairs = self.f.val(format!("load ptr, ptr {m}, align 8"));
        let counter = self.f.alloca(Layout::scalar(8));
        self.f.ins(format!("store i64 0, ptr {counter}, align 8"));
        let (head, inside, exit) = (self.f.label(), self.f.label(), self.f.label());
        self.f.br(&head);
        self.f.place(&head);
        let i = self.f.val(format!("load i64, ptr {counter}, align 8"));
        let more = self.f.val(format!("icmp slt i64 {i}, {len}"));
        self.f.cond_br(&more, &inside, &exit);
        self.f.place(&inside);
        let from = self.f.val(format!("mul i64 {i}, {pair_size}"));
        let src = self.f.val(format!(
            "getelementptr inbounds i8, ptr {pairs}, i64 {from}"
        ));
        let to = self.f.val(format!("mul i64 {i}, {}", layout.size));
        let dst = self
            .f
            .val(format!("getelementptr inbounds i8, ptr {data}, i64 {to}"));
        self.copy(key, &dst, &src);
        let next = self.f.val(format!("add i64 {i}, 1"));
        self.f
            .ins(format!("store i64 {next}, ptr {counter}, align 8"));
        self.f.br(&head);
        self.f.place(&exit);
        let t = self.temp(&Ty::Text);
        self.f.ins(format!("store ptr {data}, ptr {t}, align 8"));
        let lenp = self.gep(&t, 8);
        self.f.ins(format!("store i64 {len}, ptr {lenp}, align 8"));
        let capp = self.gep(&t, 16);
        self.f.ins(format!("store i64 {len}, ptr {capp}, align 8"));
        Val::Ptr(t, true)
    }

    fn tuple(&mut self, e: &Expr, items: &[Expr]) -> Val {
        let ty = self.ty(e);
        let elements = match &ty {
            Ty::Tuple(elements) => elements.clone(),
            other => panic!("tuple literal of type {other:?}"),
        };
        let (offsets, _) = self.env.tuple_layout(&elements);
        let t = self.temp(&ty);
        for ((item, element), offset) in items.iter().zip(&elements).zip(offsets) {
            let v = self.expr(item, true);
            if v == Val::Never {
                return Val::Never;
            }
            let v = self.coerce(element, &self.ty(item), v);
            let dst = self.gep(&t, offset);
            self.store_value(element, &dst, v);
        }
        Val::Ptr(t, true)
    }

    /// `Name { f = e, .., ..base }`: the field values in source order, then
    /// `base`; the result is a copy of `base` with those fields replaced.
    /// Only the fields kept from `base` are deep-copied.
    fn record_update(
        &mut self,
        e: &Expr,
        fields: &[ignite_syntax::ast::FieldInit],
        base: &Expr,
    ) -> Val {
        let ty = self.ty(e);
        let mut values = Vec::new();
        for init in fields {
            let (_, field_ty) = self
                .env
                .field(&ty, &init.name.name)
                .unwrap_or_else(|| panic!("no field `{}`", init.name.name));
            let v = self.expr(&init.value, true);
            if v == Val::Never {
                return Val::Never;
            }
            let have = self.ty(&init.value);
            let v = self.coerce(&field_ty, &have, v);
            values.push(self.owned(&field_ty, v));
        }
        let b = self.expr(base, true);
        let (src, owned) = match &b {
            Val::Ptr(p, owned) => (p.clone(), *owned),
            _ => return Val::Never,
        };
        let t = self.temp(&ty);
        let size = self.layout(&ty).size;
        self.memcpy(&t, &src, size);
        let (offsets, _) = self.env.fields_layout(&ty);
        let declared = self.env.fields_of(&ty);
        for ((name, field_ty), offset) in declared.iter().zip(offsets) {
            let boxed = self.env.field_boxed(&ty, name);
            let at = self.gep(&t, offset);
            match fields.iter().position(|init| init.name.name == *name) {
                Some(i) => {
                    let v = values[i].clone();
                    self.store_position(field_ty, &at, boxed, v);
                }
                None if owned => {}
                None if boxed => {
                    let from = self.gep(&src, offset);
                    let inner = self.deref_position(from, true);
                    self.store_position(field_ty, &at, true, Val::Ptr(inner, false));
                }
                None => {
                    let from = self.gep(&src, offset);
                    self.copy(field_ty, &at, &from);
                }
            }
        }
        Val::Ptr(t, true)
    }

    /// A closure literal (`docs/slice.md` section 13.3, `docs/llvm.md`
    /// section 3): its body becomes the internal function
    /// `"closure.<module>.<n>"(ptr env, [ptr result,] params..)`; its
    /// value is that function and an environment in the arena holding the
    /// address of the environment glue, then a copy of each capture in
    /// `ClosureInfo::captures` order. Without captures the environment is
    /// null.
    fn closure(&mut self, e: &Expr, closure: &ignite_syntax::ast::Closure) -> Val {
        let info = self
            .cx
            .types
            .closure(e)
            .cloned()
            .unwrap_or_else(|| panic!("untyped closure at {:?}", e.span));
        let params: Vec<(String, Ty)> = info
            .params
            .iter()
            .map(|(n, t)| (n.clone(), self.sub(t)))
            .collect();
        let ret = self.sub(&info.ret);
        let captures: Vec<(String, Ty)> = info
            .captures
            .iter()
            .map(|c| (c.name.clone(), self.sub(&c.ty)))
            .collect();
        let mut layouts = vec![Layout::scalar(8)];
        layouts.extend(captures.iter().map(|(_, t)| self.layout(t)));
        let (offsets, env_layout) = c_struct(&layouts);
        let symbol = format!("closure.{}.{}", self.cx.module, self.m.next_id());

        let (ret_abi, _) = abi(&[], &ret);
        let mut list = vec!["ptr %env".to_string()];
        if is_aggregate(&ret) {
            list.push("ptr %sret".to_string());
        }
        for (i, (_, ty)) in params.iter().enumerate() {
            if is_aggregate(ty) {
                list.push(format!("ptr %p{i}"));
            } else if let Some(t) = scalar_type(ty) {
                list.push(format!("{t} %p{i}"));
            }
        }
        let header = format!(
            "define internal {ret_abi} {}({})",
            global(&symbol),
            list.join(", ")
        );
        let mut body = Lower::new(self.m, self.cx, Func::new(header), ret.clone());
        body.debug_scope(&symbol, e.span.start);
        body.subst = self.subst.clone();
        for ((name, ty), offset) in captures.iter().zip(&offsets[1..]) {
            let at = body.gep("%env", *offset);
            body.bind_value(name, ty, Val::Ptr(at, false), true);
        }
        for (i, (name, ty)) in params.iter().enumerate() {
            let value = format!("%p{i}");
            if is_aggregate(ty) {
                body.bind(name, value, ty.clone());
            } else if is_void(ty) {
                body.bind(name, String::new(), ty.clone());
            } else {
                let slot = body.f.alloca(body.env.layout(ty));
                body.store_scalar(ty, &slot, &value);
                body.bind(name, slot, ty.clone());
            }
        }
        let value = body.expr(&closure.body, !is_void(&ret));
        if !body.f.is_terminated() {
            body.ret_value(value);
        }
        let text = body.f.finish("unreachable");
        self.m.define(&symbol, text);

        let fn_ty = Ty::Fn(
            params.iter().map(|(_, t)| t.clone()).collect(),
            Box::new(ret),
        );
        let t = self.temp(&fn_ty);
        self.f
            .ins(format!("store ptr {}, ptr {t}, align 8", global(&symbol)));
        let env = if captures.is_empty() {
            "null".to_string()
        } else {
            let glue_symbol = format!("{symbol}.env");
            let glue_captures: Vec<(Ty, u64)> = captures
                .iter()
                .map(|(_, t)| t.clone())
                .zip(offsets[1..].iter().copied())
                .collect();
            let glue = crate::glue::env_glue(self.m, &glue_symbol, &glue_captures, env_layout.size);
            self.m.define(&glue_symbol, glue);
            let env = self.f.val(format!(
                "call ptr @ignite.alloc(i64 {}, i64 8)",
                env_layout.size
            ));
            self.f.ins(format!(
                "store ptr {}, ptr {env}, align 8",
                global(&glue_symbol)
            ));
            for ((name, ty), offset) in captures.iter().zip(&offsets[1..]) {
                if is_void(ty) {
                    continue;
                }
                let local = self
                    .lookup(name)
                    .unwrap_or_else(|| panic!("capture `{name}` is not in scope"));
                let v = self.at(&local.ty, local.ptr, false);
                let dst = self.gep(&env, *offset);
                self.store_value(ty, &dst, v);
            }
            env
        };
        let at = self.gep(&t, 8);
        self.f.ins(format!("store ptr {env}, ptr {at}, align 8"));
        Val::Ptr(t, true)
    }

    fn list_literal(&mut self, element: &Ty, items: &[Expr]) -> Val {
        if items.is_empty() {
            return self.empty_list();
        }
        let layout = self.layout(element);
        let size = layout.size;
        let count = items.len() as u64;
        let data = self.f.val(format!(
            "call ptr @ignite.alloc(i64 {}, i64 {})",
            (size * count).max(1),
            layout.align
        ));
        for (i, item) in items.iter().enumerate() {
            let v = self.expr(item, true);
            let v = self.coerce(element, &self.ty(item), v);
            let dst = self.gep(&data, size * i as u64);
            self.store_value(element, &dst, v);
        }
        let t = self.temp(&Ty::Text);
        self.f.ins(format!("store ptr {data}, ptr {t}, align 8"));
        let lenp = self.gep(&t, 8);
        self.f
            .ins(format!("store i64 {count}, ptr {lenp}, align 8"));
        let capp = self.gep(&t, 16);
        self.f
            .ins(format!("store i64 {count}, ptr {capp}, align 8"));
        Val::Ptr(t, true)
    }

    /// An `Int` scalar where a `Byte` is expected (and back).
    fn coerce(&mut self, want: &Ty, have: &Ty, v: Val) -> Val {
        match (want, have, &v) {
            (Ty::Byte, Ty::Int, Val::Scalar(op)) => {
                Val::Scalar(self.f.val(format!("trunc i64 {op} to i8")))
            }
            (Ty::Int, Ty::Byte, Val::Scalar(op)) => {
                Val::Scalar(self.f.val(format!("zext i8 {op} to i64")))
            }
            _ => v,
        }
    }

    /// A mention of the constant `q` of type `ty`. A scalar constant is a
    /// call of its function, which LLVM folds. An aggregate constant is
    /// evaluated once per process and IR module instead of at every
    /// mention: the first mention calls `"<q>.once"`, which evaluates it,
    /// copies the value to the heap (`copy_to_heap`) and publishes the
    /// copy in the module's `"<q>.cache"` with a compare-and-swap (a task
    /// that loses a race frees its copy and takes the winner's); every
    /// mention after it is an acquire load. The mention is borrowed, so
    /// every value position copies it into the task's arena as it copies
    /// a variable, and an in-place builtin on it changes a copy
    /// ([`Lower::mutable_receiver`]). A failure while the constant is
    /// evaluated publishes nothing. The `.once` helper is pruned as a
    /// constant ([`crate::ir::Module::mark_const`]): kept only when kept
    /// code mentions the constant.
    fn constant(&mut self, q: &QualName, ty: &Ty) -> Val {
        let name = symbol(q);
        let callee = self.function(&name, &[], ty);
        if !is_aggregate(ty) {
            return self.call(&callee, ty, Vec::new());
        }
        let once = format!("{name}.once");
        let cache = global(&format!("{name}.cache"));
        if !self.m.has(&once) {
            let layout = self.layout(ty);
            let to_heap = self.m.glue_of(Glue::ToHeap, ty);
            let free = self.m.glue_of(Glue::FreeHeap, ty);
            self.m
                .global_definition(format!("{cache} = internal global ptr null, align 8"));
            let text = format!(
                "define internal ptr {once_global}() noinline {{\n\
                 entry:\n  \
                 %value = alloca [{size} x i8], align {align}\n  \
                 call void {callee}(ptr %value)\n  \
                 %heap = call ptr {to_heap}(ptr %value)\n  \
                 %swap = cmpxchg ptr {cache}, ptr null, ptr %heap acq_rel acquire, align 8\n  \
                 %old = extractvalue {{ ptr, i1 }} %swap, 0\n  \
                 %won = extractvalue {{ ptr, i1 }} %swap, 1\n  \
                 br i1 %won, label %mine, label %theirs\n\
                 mine:\n  \
                 ret ptr %heap\n\
                 theirs:\n  \
                 call void {free}(ptr %heap)\n  \
                 ret ptr %old\n\
                 }}\n",
                once_global = global(&once),
                size = layout.size.max(1),
                align = layout.align.max(1),
            );
            self.m.helper(&once, &text);
            self.m.mark_const(&once);
        }
        let have = self
            .f
            .val(format!("load atomic ptr, ptr {cache} acquire, align 8"));
        let missing = self.f.val(format!("icmp eq ptr {have}, null"));
        let from = self.f.current().to_string();
        let (make, done) = (self.f.label(), self.f.label());
        self.f.cond_br(&missing, &make, &done);
        self.f.place(&make);
        let made = self.f.val(format!("call ptr {}()", global(&once)));
        self.f.br(&done);
        self.f.place(&done);
        let p = self
            .f
            .val(format!("phi ptr [ {have}, %{from} ], [ {made}, %{make} ]"));
        Val::Ptr(p, false)
    }

    fn is_const(&self, name: &str) -> bool {
        self.cx.consts.contains(name)
    }

    fn name(&mut self, id: &Ident, e: &Expr) -> Val {
        if let Some(local) = self.lookup(&id.name) {
            return self.at(&local.ty, local.ptr, false);
        }
        if id.name == "Unit" {
            return Val::Unit;
        }
        match self.cx.types.callee(e).cloned() {
            Some(Callee::Fn(q)) => {
                if let Some((name, sig)) = self.fn_instance(&q, e) {
                    return self.fn_value(&name, &sig);
                }
            }
            Some(Callee::Const(q)) => {
                if let Some(ty) = self.env.const_ty(&q).map(repr) {
                    return self.constant(&q, &ty);
                }
            }
            Some(Callee::Variant { .. }) => {
                let ty = self.ty(e);
                return self.variant(&ty, &id.name, Vec::new());
            }
            _ => {}
        }
        if self.is_const(&id.name) {
            let q = self.cx.qual(&id.name);
            let ty = self
                .env
                .const_ty(&q)
                .map(repr)
                .unwrap_or_else(|| self.ty(e));
            return self.constant(&q, &ty);
        }
        if self.cx.fns.contains(&id.name) {
            let q = self.cx.qual(&id.name);
            if let Some(sig) = self.env.fn_sig(&q).map(repr_sig) {
                return self.fn_value(&symbol(&q), &sig);
            }
        }
        let ty = self.ty(e);
        self.variant(&ty, &id.name, Vec::new())
    }

    /// A value of enum-like `ty`: variant `name` with payload values.
    fn variant(&mut self, ty: &Ty, name: &str, args: Vec<Val>) -> Val {
        let layout = self.env.enum_layout(ty);
        let index = layout
            .index(name)
            .unwrap_or_else(|| panic!("no variant `{name}` in {ty:?}"));
        let t = self.temp(ty);
        self.f.ins(format!("store i32 {index}, ptr {t}, align 4"));
        let fields = layout.variants[index].1.clone();
        for (i, ((field_ty, offset), v)) in fields.iter().zip(args).enumerate() {
            let dst = self.gep(&t, *offset);
            let boxed = self.env.payload_boxed(ty, index, i);
            self.store_position(field_ty, &dst, boxed, v);
        }
        Val::Ptr(t, true)
    }

    /// Store `v` of type `ty` into the position at `dst`: in place, or,
    /// when `boxed` (`docs/slice.md` section 13.4), into a new box in the
    /// arena whose address the position holds.
    fn store_position(&mut self, ty: &Ty, dst: &str, boxed: bool, v: Val) {
        if !boxed {
            self.store_value(ty, dst, v);
            return;
        }
        let layout = self.layout(ty);
        let b = self.f.val(format!(
            "call ptr @ignite.alloc(i64 {}, i64 {})",
            layout.size.max(1),
            layout.align.max(1)
        ));
        self.store_value(ty, &b, v);
        self.f.ins(format!("store ptr {b}, ptr {dst}, align 8"));
    }

    /// The address of the value of a position at `at`: `at` itself, or
    /// the box it points to.
    fn deref_position(&mut self, at: String, boxed: bool) -> String {
        if boxed {
            self.f.val(format!("load ptr, ptr {at}, align 8"))
        } else {
            at
        }
    }

    /// The variant name a constructor expression names: `Some`, `Command.Echo`.
    fn constructor_name(callee: &Expr) -> Option<&str> {
        match &callee.kind {
            ExprKind::Name(id) => Some(&id.name),
            ExprKind::Field { name, .. } => Some(&name.name),
            _ => None,
        }
    }

    fn construct(&mut self, e: &Expr, name: &str, index: usize, args: &[Expr]) -> Val {
        let ty = self.ty(e);
        let layout = self.env.enum_layout(&ty);
        let index = layout.index(name).unwrap_or(index);
        let name = layout.variants[index].0.clone();
        let fields = layout.variants[index].1.clone();
        // The payloads are stored once every one is evaluated, so each but
        // the last borrowed one is copied when it is evaluated: a later
        // payload may move the same place into a callee that changes it
        // (`docs/slice.md` 20.3.5).
        if let Some(perm) = self.cx.types.named_args(e).map(<[usize]>::to_vec) {
            let mut field_of = vec![None; args.len()];
            for (field_i, &src) in perm.iter().enumerate().take(fields.len()) {
                field_of[src] = Some(field_i);
            }
            let mut evaluated = Vec::with_capacity(args.len());
            for (k, (arg, field)) in args.iter().zip(&field_of).enumerate() {
                let v = self.expr(arg, true);
                if v == Val::Never {
                    return Val::Never;
                }
                evaluated.push(match field {
                    Some(f) if k + 1 < args.len() => self.owned(&fields[*f].0, v),
                    _ => v,
                });
            }
            let mut vals = Vec::new();
            for (field_i, (field_ty, _)) in fields.iter().enumerate() {
                let src = perm[field_i];
                let have = self.ty(&args[src]);
                vals.push(self.coerce(field_ty, &have, evaluated[src].clone()));
            }
            return self.variant(&ty, &name, vals);
        }
        let mut vals = Vec::new();
        let count = args.len().min(fields.len());
        for (k, (arg, (field_ty, _))) in args.iter().zip(&fields).enumerate() {
            let v = self.expr(arg, true);
            if v == Val::Never {
                return Val::Never;
            }
            let have = self.ty(arg);
            let v = self.coerce(field_ty, &have, v);
            vals.push(if k + 1 < count {
                self.owned(field_ty, v)
            } else {
                v
            });
        }
        self.variant(&ty, &name, vals)
    }

    fn call_expr(&mut self, e: &Expr, callee: &Expr, args: &[Expr]) -> Val {
        let resolved = self.cx.types.callee(e).cloned();
        match resolved {
            Some(Callee::Variant { index, .. }) => {
                let name = Self::constructor_name(callee).unwrap_or("").to_string();
                self.construct(e, &name, index, args)
            }
            Some(Callee::Builtin(Builtin::StoreNew)) => self.store_new(e),
            Some(Callee::Builtin(b)) => {
                let receiver = match &callee.kind {
                    ExprKind::Field { receiver, .. } => Some(&**receiver),
                    _ => None,
                };
                self.builtin(e, b, receiver, args)
            }
            Some(Callee::Local) => {
                let fn_ty = self.ty(callee);
                let (params, ret) = match fn_ty {
                    Ty::Fn(params, ret) | Ty::EffectFn(params, ret, _) => (params, *ret),
                    other => panic!("call of a {other:?}"),
                };
                let f = self.expr(callee, true);
                if f == Val::Never {
                    return Val::Never;
                }
                let fp = self.ptr(&f);
                let parts = self.fn_parts(&fp);
                let borrows = self.known_borrows(callee);
                let Some(operands) = self.value_operands(args, &params, &borrows) else {
                    return Val::Never;
                };
                self.call_value(&parts, &ret, operands)
            }
            Some(other) => match self.sig_of(&other, e) {
                Some((symbol, sig)) => self.call_exprs(e, &symbol, &sig, args),
                None => panic!("call of {other:?}"),
            },
            None => panic!("untyped call at {:?}", e.span),
        }
    }

    fn method(&mut self, e: &Expr, receiver: &Expr, name: &Ident, args: &[Expr]) -> Val {
        let resolved = self.cx.types.callee(e).cloned();
        match resolved {
            Some(Callee::Variant { index, .. }) => self.construct(e, &name.name, index, args),
            Some(Callee::Builtin(Builtin::StoreNew)) => self.store_new(e),
            Some(Callee::Builtin(b)) => self.builtin(e, b, Some(receiver), args),
            Some(Callee::Fn(q)) => {
                let (name, sig) = self
                    .fn_instance(&q, e)
                    .unwrap_or_else(|| panic!("no signature for {}", symbol(&q)));
                let all: Vec<&Expr> = if q.name.contains('.') {
                    args.iter().collect()
                } else {
                    std::iter::once(receiver).chain(args).collect()
                };
                self.call_with(Some(e), &name, &sig, &all)
            }
            Some(Callee::FieldFn { .. }) => {
                let fn_ty = match self.ty(receiver) {
                    rty @ (Ty::Record(_) | Ty::GenericRecord(..)) => self
                        .env
                        .field(&rty, &name.name)
                        .map(|(_, t)| t)
                        .unwrap_or_else(|| panic!("no field `{}` in {rty:?}", name.name)),
                    other => panic!("fn field `{}` of {other:?}", name.name),
                };
                let (params, ret) = match fn_ty {
                    Ty::Fn(params, ret) | Ty::EffectFn(params, ret, _) => (params, *ret),
                    other => panic!("call of a {other:?}"),
                };
                let f = self.field(receiver, receiver, name);
                if f == Val::Never {
                    return Val::Never;
                }
                let fp = self.ptr(&f);
                let parts = self.fn_parts(&fp);
                let Some(operands) = self.value_operands(args, &params, &[]) else {
                    return Val::Never;
                };
                self.call_value(&parts, &ret, operands)
            }
            Some(other) => match self.sig_of(&other, e) {
                Some((symbol, sig)) => self.call_exprs(e, &symbol, &sig, args),
                None => panic!("method call of {other:?}"),
            },
            None => panic!("untyped method call `{}` at {:?}", name.name, e.span),
        }
    }

    /// `Enum.Variant` when the receiver names an enum, else a field.
    fn field(&mut self, e: &Expr, receiver: &Expr, name: &Ident) -> Val {
        match self.cx.types.callee(e).cloned() {
            Some(Callee::Variant { .. }) => {
                let ty = self.ty(e);
                return self.variant(&ty, &name.name, Vec::new());
            }
            Some(Callee::Fn(q)) if !std::ptr::eq(e, receiver) => {
                if let Some((symbol, sig)) = self.fn_instance(&q, e) {
                    return self.fn_value(&symbol, &sig);
                }
            }
            _ => {}
        }
        if let ExprKind::Name(id) = &receiver.kind
            && self.lookup(&id.name).is_none()
            && !self.is_const(&id.name)
            && (self.cx.symbols.enums.contains_key(&id.name) || id.name == "Input")
        {
            let ty = self.ty(e);
            return self.variant(&ty, &name.name, Vec::new());
        }
        let rty = self.ty(receiver);
        let base = self.expr(receiver, true);
        if base == Val::Never {
            return Val::Never;
        }
        let (owned, ptr) = match &base {
            Val::Ptr(p, owned) => (*owned, p.clone()),
            other => panic!("field `{}` of {other:?}", name.name),
        };
        let (offset, ty) = self
            .env
            .field(&rty, &name.name)
            .unwrap_or_else(|| panic!("no field `{}` in {rty:?}", name.name));
        let p = self.gep(&ptr, offset);
        let boxed = self.env.field_boxed(&rty, &name.name);
        let p = self.deref_position(p, boxed);
        self.at(&ty, p, owned)
    }

    fn try_expr(&mut self, e: &Expr, inner: &Expr) -> Val {
        let v = self.expr(inner, true);
        if v == Val::Never {
            return Val::Never;
        }
        let ty = self.ty(inner);
        let layout = self.env.enum_layout(&ty);
        let (owned, p) = match &v {
            Val::Ptr(p, owned) => (*owned, p.clone()),
            other => panic!("`?` on {other:?}"),
        };
        let tag = self.f.val(format!("load i32, ptr {p}, align 4"));
        let failed = self.f.val(format!("icmp eq i32 {tag}, 1"));
        let (bad, good) = (self.f.label(), self.f.label());
        self.f.cond_br(&failed, &bad, &good);
        self.f.place(&bad);
        if let Some(test_try) = self.cx.types.test_try(e).cloned() {
            self.try_failed(e, &test_try, &layout.variants[1].1, &p);
            self.f.place(&good);
            return match layout.variants[0].1.first().cloned() {
                Some((ok_ty, offset)) => {
                    let src = self.gep(&p, offset);
                    self.at(&ok_ty, src, owned)
                }
                None => Val::Unit,
            };
        }
        let ret = self.ret.clone();
        let sret = self
            .sret
            .clone()
            .expect("`?` in a function returning Result or Option");
        let ret_layout = self.env.enum_layout(&ret);
        self.f.ins(format!("store i32 1, ptr {sret}, align 4"));
        if let (Some((err_ty, from)), Some((_, to))) = (
            layout.variants[1].1.first().cloned(),
            ret_layout.variants[1].1.first().cloned(),
        ) {
            let src = self.gep(&p, from);
            let dst = self.gep(&sret, to);
            self.store_value(&err_ty, &dst, Val::Ptr(src, owned));
        }
        self.release_owned(0);
        self.f.term("ret void");
        self.f.place(&good);
        match layout.variants[0].1.first().cloned() {
            Some((ok_ty, offset)) => {
                let src = self.gep(&p, offset);
                self.at(&ok_ty, src, owned)
            }
            None => Val::Unit,
        }
    }

    // ---- assert and `?` in tests (docs/slice.md section 12.5) ---------

    /// `<path>:<line>:<col>` of byte `offset` in the module being lowered;
    /// `<module>:?:?` when the driver gave no source file for it.
    fn location(&self, offset: usize) -> String {
        match self.m.file(&self.cx.module) {
            Some(file) => {
                let (line, col) = file.line_col(offset);
                format!("{}:{line}:{col}", file.path)
            }
            None => format!("{}:?:?", self.cx.module),
        }
    }

    /// The source text of `e`, or `<condition>` without a source file.
    fn source_text(&self, e: &Expr) -> String {
        self.m
            .file(&self.cx.module)
            .and_then(|file| file.text.get(e.span.start..e.span.end))
            .unwrap_or("<condition>")
            .to_string()
    }

    /// Declare a runtime function that only tests and asserts use, so a
    /// module without them declares nothing new.
    fn runtime(&mut self, name: &str, params: usize) -> String {
        let symbol = format!("ignite_rt_{name}");
        self.m
            .declare(&symbol, "void", &vec!["ptr".to_string(); params]);
        global(&symbol)
    }

    /// `assert cond` / `assert cond, message` at byte `start` (the
    /// keyword). For `a == b` and `a != b` with an [`AssertCompare`]
    /// (`ignite_types`), `a` then `b` are evaluated once each, compared
    /// structurally and shown on failure.
    fn assert(&mut self, start: usize, cond: &Expr, message: Option<&Expr>) {
        let compare = self.cx.types.assert_compare(cond).cloned();
        let operands = match (&compare, &cond.kind) {
            (
                Some(compare),
                ExprKind::Binary {
                    op: op @ (BinaryOp::Eq | BinaryOp::Ne),
                    left,
                    right,
                },
            ) => Some((compare, *op, left, right)),
            _ => None,
        };
        let (ok, shown) = match operands {
            Some((compare, op, left, right)) => {
                let ty = self.sub(&compare.operand);
                let l = self.expr(left, true);
                if l == Val::Never {
                    return;
                }
                let l = match l {
                    Val::Ptr(_, false)
                        if !ownership::changed_in(right, Some(self.cx.types)).is_empty() =>
                    {
                        self.owned(&ty, l)
                    }
                    other => other,
                };
                let r = self.expr(right, true);
                if r == Val::Never {
                    return;
                }
                let same = self.equal(&ty, &l, &r);
                let ok = if op == BinaryOp::Eq {
                    same
                } else {
                    self.f.val(format!("xor i1 {same}, true"))
                };
                (ok, Some((ty, compare.show.clone(), l, r)))
            }
            None => {
                let c = self.expr(cond, true);
                if c == Val::Never {
                    return;
                }
                (self.scalar(&c), None)
            }
        };
        let (bad, good) = (self.f.label(), self.f.label());
        self.f.cond_br(&ok, &good, &bad);
        self.f.place(&bad);
        let text = self.source_text(cond);
        let cond_text = self.literal_ref(text.as_bytes());
        let message = match message {
            Some(m) => {
                let v = self.expr(m, true);
                if v == Val::Never {
                    self.f.place(&good);
                    return;
                }
                self.ptr(&v)
            }
            None => "null".to_string(),
        };
        let (left, right) = match shown {
            Some((ty, show, l, r)) => {
                let left = self.show(show.as_ref(), &ty, &l);
                let right = self.show(show.as_ref(), &ty, &r);
                (left, right)
            }
            None => ("null".to_string(), "null".to_string()),
        };
        let at = self.location(start);
        let at = self.literal_ref(at.as_bytes());
        let failed = self.runtime("assert_failed", 5);
        self.f.ins(format!(
            "call void {failed}(ptr {cond_text}, ptr {message}, ptr {left}, ptr {right}, ptr {at})"
        ));
        self.f.term("unreachable");
        self.f.place(&good);
    }

    /// The failing branch of a `?` in a test (`e`): show the error of an
    /// `Err` (the payload `fields` of variant 1 of the value at `value`),
    /// or nothing for `None`, and end the process.
    fn try_failed(
        &mut self,
        e: &Expr,
        test_try: &ignite_types::TestTry,
        fields: &[(Ty, u64)],
        value: &str,
    ) {
        let error = match (&test_try.error, fields.first()) {
            (Some(_), Some((ty, offset))) => {
                let ty = ty.clone();
                let p = self.gep(value, *offset);
                let v = self.at(&ty, p, false);
                self.show(test_try.show.as_ref(), &ty, &v)
            }
            (Some(ty), None) => {
                let ty = self.sub(ty);
                self.literal_ref(format!("<{ty}>").as_bytes())
            }
            (None, _) => "null".to_string(),
        };
        let at = self.location(e.span.end.saturating_sub(1));
        let at = self.literal_ref(at.as_bytes());
        let failed = self.runtime("try_failed", 2);
        self.f
            .ins(format!("call void {failed}(ptr {error}, ptr {at})"));
        self.f.term("unreachable");
    }

    /// An `i1` saying whether `a` and `b` (of type `ty`) are structurally
    /// equal.
    fn equal(&mut self, ty: &Ty, a: &Val, b: &Val) -> String {
        if is_void(ty) {
            return "true".to_string();
        }
        if is_aggregate(ty) {
            let (pa, pb) = (self.ptr(a), self.ptr(b));
            return crate::glue::value_eq(self.m, &mut self.f, ty, &pa, &pb);
        }
        let t = scalar_type(ty).unwrap_or("i64");
        let (x, y) = (self.scalar(a), self.scalar(b));
        self.f.val(format!("icmp eq {t} {x}, {y}"))
    }

    /// A pointer to a `Text` showing `v` of type `ty` (`docs/slice.md`
    /// section 12.3): its builtin `to_text`, a `to_text` fn, an `IoError`'s
    /// message, or `<T>`.
    fn show(&mut self, show: Option<&ignite_types::Show>, ty: &Ty, v: &Val) -> String {
        use ignite_types::Show;
        match show {
            Some(Show::Debug) => {
                let t = self.debug_text(ty, v);
                self.ptr(&t)
            }
            Some(Show::Builtin(b)) => match (b, ty) {
                (Builtin::IntToTextMethod | Builtin::IntToText, Ty::Int) => {
                    let i = self.scalar(v);
                    let t = self.rt_out("int_to_text", &Ty::Text, &[format!("i64 {i}")]);
                    self.ptr(&t)
                }
                (Builtin::ByteToText, Ty::Byte) => {
                    let b = self.scalar(v);
                    let i = self.f.val(format!("zext i8 {b} to i64"));
                    let t = self.rt_out("int_to_text", &Ty::Text, &[format!("i64 {i}")]);
                    self.ptr(&t)
                }
                (Builtin::BoolToText, Ty::Bool) => {
                    let b = self.scalar(v);
                    let yes = self.literal_ref(b"true");
                    let no = self.literal_ref(b"false");
                    self.f.val(format!("select i1 {b}, ptr {yes}, ptr {no}"))
                }
                (Builtin::TextToText, Ty::Text | Ty::Ref(_) | Ty::Signal(_)) => self.ptr(v),
                (Builtin::BytesToText, Ty::List(_)) => {
                    let p = self.ptr(v);
                    let t = self.rt_out("text_from_bytes_lossy", &Ty::Text, &[format!("ptr {p}")]);
                    self.ptr(&t)
                }
                _ => self.literal_ref(format!("<{ty}>").as_bytes()),
            },
            Some(Show::Fn(q)) => {
                let Some(sig) = self.env.fn_sig(q).map(repr_sig) else {
                    return self.literal_ref(format!("<{ty}>").as_bytes());
                };
                let callee = self.function(&symbol(q), &sig.params, &sig.ret);
                let arg = self.arg(ty, v.clone());
                let t = self.call(&callee, &sig.ret, vec![arg]);
                self.ptr(&t)
            }
            Some(Show::IoErrorMessage) => self.ptr(v),
            None => self.literal_ref(format!("<{ty}>").as_bytes()),
        }
    }

    /// A binary expression `e`. A left-deep chain (`a + b + c ..`) is
    /// lowered from its innermost left operand outwards in a loop, so a
    /// chain of 100,000 terms needs no deep stack (`docs/slice.md` section
    /// 13.1, row 14).
    fn binary(&mut self, e: &Expr) -> Val {
        let mut spine = Vec::new();
        let mut cur = e;
        while let ExprKind::Binary { op, left, right } = &cur.kind {
            spine.push((*op, &**left, &**right, cur));
            cur = left;
        }
        let mark = self.f.alloca_mark();
        // The `Text` `+` steps come first from the inside (no other
        // operator yields a `Text`); [`Lower::text_chain`] lowers them all
        // at once, `.concat` calls on the spine included.
        let concats = spine
            .iter()
            .rev()
            .take_while(|(op, left, ..)| *op == BinaryOp::Add && self.ty(left) == Ty::Text)
            .count();
        let mut value = if concats > 0 {
            let rest = spine.len() - concats;
            let node = spine[rest].3;
            spine.truncate(rest);
            self.text_chain(node)
        } else {
            self.expr(cur, true)
        };
        for (op, left, right, node) in spine.into_iter().rev() {
            if value == Val::Never {
                return Val::Never;
            }
            value = self.binary_step(op, left, value, right, node);
            let keep = match &value {
                Val::Ptr(p, _) => vec![p.as_str()],
                _ => Vec::new(),
            };
            self.f.reuse_allocas(mark, &keep);
        }
        value
    }

    /// The operands of `e` when it is one `Text` concatenation, `a + b`
    /// or `a.concat(b)`: `(a, b)`.
    fn concat_parts<'x>(&self, e: &'x Expr) -> Option<(&'x Expr, &'x Expr)> {
        match &e.kind {
            ExprKind::Binary {
                op: BinaryOp::Add,
                left,
                right,
            } if self.ty(left) == Ty::Text => Some((left, right)),
            ExprKind::Method { receiver, args, .. }
                if args.len() == 1
                    && matches!(
                        self.cx.types.callee(e),
                        Some(Callee::Builtin(Builtin::TextConcat))
                    ) =>
            {
                Some((receiver, &args[0]))
            }
            ExprKind::Call { callee, args }
                if args.len() == 1
                    && matches!(
                        self.cx.types.callee(e),
                        Some(Callee::Builtin(Builtin::TextConcat))
                    ) =>
            {
                match &callee.kind {
                    ExprKind::Field { receiver, .. } => Some((receiver, &args[0])),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The `Text` concatenation `e` with every concatenation on its left
    /// spine (`a + b + c`, `a.concat(b).concat(c)`, mixed): the operands
    /// are evaluated left to right and their pointer and length loaded at
    /// once, then one arena block of the summed length takes one `memcpy`
    /// per operand, a literal straight from its constant (adjacent literals
    /// joined into one). Operands go in
    /// batches of [`CONCAT_BATCH`]: the first makes the text with an exact
    /// capacity, each later one grows it with `@ignite.reserve` (doubling)
    /// and copies into place, so a 100,000-term chain stays linear and
    /// holds a bounded number of values and stack slots.
    fn text_chain(&mut self, e: &Expr) -> Val {
        let mut operands = Vec::new();
        let mut cur = e;
        while let Some((left, right)) = self.concat_parts(cur) {
            operands.push(right);
            cur = left;
        }
        operands.push(cur);
        operands.reverse();
        let out = self.temp(&Ty::Text);
        let mark = self.f.alloca_mark();
        let mut pieces = Vec::new();
        // Adjacent literals, joined into one constant.
        let mut literal: Vec<u8> = Vec::new();
        let mut made = false;
        for operand in operands {
            if let ExprKind::Str(s) = &operand.kind {
                literal.extend_from_slice(s.as_bytes());
                continue;
            }
            let v = self.expr(operand, true);
            if v == Val::Never {
                return Val::Never;
            }
            let p = self.ptr(&v);
            let piece = self.text_piece(&p);
            self.f.reuse_allocas(mark, &[]);
            if !literal.is_empty() {
                pieces.push(self.literal_piece(&std::mem::take(&mut literal)));
            }
            pieces.push(piece);
            if pieces.len() >= CONCAT_BATCH {
                self.concat_into(&out, &pieces, made);
                pieces.clear();
                made = true;
            }
        }
        if !literal.is_empty() {
            pieces.push(self.literal_piece(&literal));
        }
        if !made || !pieces.is_empty() {
            self.concat_into(&out, &pieces, made);
        }
        Val::Ptr(out, true)
    }

    /// The bytes of a literal, for [`Lower::concat_into`].
    fn literal_piece(&mut self, text: &[u8]) -> (String, String) {
        let data = if text.is_empty() {
            "@ignite.empty".to_string()
        } else {
            self.m.string(text)
        };
        (data, text.len().to_string())
    }

    /// The data pointer and length of the `Text` at `p`, loaded now, for
    /// [`Lower::concat_into`].
    fn text_piece(&mut self, p: &str) -> (String, String) {
        let data = self.f.val(format!("load ptr, ptr {p}, align 8"));
        let lenp = self.gep(p, 8);
        let len = self.f.val(format!("load i64, ptr {lenp}, align 8"));
        (data, len)
    }

    /// Copy `pieces` (data pointer, length) one after the other into the
    /// `Text` at `out`: a new block of their summed length, or, when
    /// `grow`, appended to the text `out` already holds.
    fn concat_into(&mut self, out: &str, pieces: &[(String, String)], grow: bool) {
        let mut fixed: u64 = 0;
        let mut total: Option<String> = None;
        for (_, len) in pieces {
            match len.parse::<u64>() {
                Ok(n) => fixed += n,
                Err(_) => {
                    total = Some(match total {
                        None => len.clone(),
                        Some(t) => self.f.val(format!("add i64 {t}, {len}")),
                    })
                }
            }
        }
        let total = match total {
            None => fixed.to_string(),
            Some(t) if fixed == 0 => t,
            Some(t) => self.f.val(format!("add i64 {t}, {fixed}")),
        };
        let (mut at, old) = if grow {
            self.f.ins(format!(
                "call void @ignite.reserve(ptr {out}, i64 {total}, i64 1, i64 1)"
            ));
            let lenp = self.gep(out, 8);
            let old = self.f.val(format!("load i64, ptr {lenp}, align 8"));
            let data = self.f.val(format!("load ptr, ptr {out}, align 8"));
            let at = self
                .f
                .val(format!("getelementptr inbounds i8, ptr {data}, i64 {old}"));
            (at, Some(old))
        } else {
            let data = self
                .f
                .val(format!("call ptr @ignite.alloc(i64 {total}, i64 1)"));
            self.f.ins(format!("store ptr {data}, ptr {out}, align 8"));
            (data, None)
        };
        let last = pieces.len().saturating_sub(1);
        for (i, (data, len)) in pieces.iter().enumerate() {
            if len == "0" {
                continue;
            }
            self.f.ins(format!(
                "call void @llvm.memcpy.p0.p0.i64(ptr {at}, ptr {data}, i64 {len}, i1 false)"
            ));
            if i < last {
                at = self
                    .f
                    .val(format!("getelementptr inbounds i8, ptr {at}, i64 {len}"));
            }
        }
        let lenp = self.gep(out, 8);
        match old {
            Some(old) => {
                let new = self.f.val(format!("add i64 {old}, {total}"));
                self.f.ins(format!("store i64 {new}, ptr {lenp}, align 8"));
            }
            None => {
                self.f
                    .ins(format!("store i64 {total}, ptr {lenp}, align 8"));
                let capp = self.gep(out, 16);
                self.f
                    .ins(format!("store i64 {total}, ptr {capp}, align 8"));
            }
        }
    }

    /// `left op right` with `left` already evaluated to `l`; `node` is the
    /// binary expression itself, for positioning an overflow or divide-by-zero
    /// failure.
    fn binary_step(&mut self, op: BinaryOp, left: &Expr, l: Val, right: &Expr, node: &Expr) -> Val {
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            let l = self.scalar(&l);
            let from = self.f.current().to_string();
            let (rhs, end) = (self.f.label(), self.f.label());
            let short = if op == BinaryOp::And {
                self.f.cond_br(&l, &rhs, &end);
                "false"
            } else {
                self.f.cond_br(&l, &end, &rhs);
                "true"
            };
            self.f.place(&rhs);
            let r = self.expr(right, true);
            let r = self.scalar(&r);
            let rhs_end = self.f.current().to_string();
            self.f.br(&end);
            self.f.place(&end);
            return Val::Scalar(
                self.f
                    .val(format!("phi i1 [ {short}, %{from} ], [ {r}, %{rhs_end} ]")),
            );
        }
        let rty = self.ty(right);
        let ty = match self.ty(left) {
            Ty::Never => rty.clone(),
            ty => ty,
        };
        let l = match l {
            Val::Ptr(_, false) if !ownership::changed_in(right, Some(self.cx.types)).is_empty() => {
                self.owned(&ty, l)
            }
            other => other,
        };
        // A literal compared with `==` or `!=` is read in place, not
        // copied into the arena, so the comparison sees its bytes.
        let r = match &right.kind {
            ExprKind::Str(s) if matches!(op, BinaryOp::Eq | BinaryOp::Ne) => {
                Val::Ptr(self.literal_ref(s.as_bytes()), false)
            }
            _ => self.expr(right, true),
        };
        if l == Val::Never || r == Val::Never {
            return Val::Never;
        }
        if matches!(op, BinaryOp::Eq | BinaryOp::Ne)
            && ty != Ty::Text
            && (is_aggregate(&ty) || is_void(&ty))
        {
            let same = self.equal(&ty, &l, &r);
            return Val::Scalar(if op == BinaryOp::Eq {
                same
            } else {
                self.f.val(format!("xor i1 {same}, true"))
            });
        }
        if let (BinaryOp::Add, Ty::List(element)) = (op, &ty) {
            return self.list_concat(&ty, element, l, r);
        }
        if ty == Ty::Text && op == BinaryOp::Add {
            let (a, b) = (self.ptr(&l), self.ptr(&r));
            return self.rt_out(
                "text_concat",
                &Ty::Text,
                &[format!("ptr {a}"), format!("ptr {b}")],
            );
        }
        if ty == Ty::Text
            && matches!(
                op,
                BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge
            )
        {
            let (a, b) = (self.ptr(&l), self.ptr(&r));
            let order = crate::glue::compare(self.m, &mut self.f, &Ty::Text, &a, &b);
            let predicate = match op {
                BinaryOp::Lt => "slt",
                BinaryOp::Gt => "sgt",
                BinaryOp::Le => "sle",
                _ => "sge",
            };
            return Val::Scalar(self.f.val(format!("icmp {predicate} i32 {order}, 0")));
        }
        if ty == Ty::Text {
            let (a, b) = (self.ptr(&l), self.ptr(&r));
            let eq = crate::glue::text_eq(self.m, &mut self.f, &a, &b);
            return Val::Scalar(if op == BinaryOp::Ne {
                self.f.val(format!("xor i1 {eq}, true"))
            } else {
                eq
            });
        }
        let r = self.coerce(&ty, &rty, r);
        let t = scalar_type(&ty).unwrap_or("i64");
        let (a, b) = (self.scalar(&l), self.scalar(&r));
        let signed = ty != Ty::Byte;
        if op.is_bitwise() {
            return Val::Scalar(self.bitwise(op, &a, &b));
        }
        let instr = match op {
            BinaryOp::Eq => "icmp eq",
            BinaryOp::Ne => "icmp ne",
            BinaryOp::Lt => {
                if signed {
                    "icmp slt"
                } else {
                    "icmp ult"
                }
            }
            BinaryOp::Gt => {
                if signed {
                    "icmp sgt"
                } else {
                    "icmp ugt"
                }
            }
            BinaryOp::Le => {
                if signed {
                    "icmp sle"
                } else {
                    "icmp ule"
                }
            }
            BinaryOp::Ge => {
                if signed {
                    "icmp sge"
                } else {
                    "icmp uge"
                }
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => {
                return Val::Scalar(self.overflowing(op, node, t, signed, &a, &b));
            }
            BinaryOp::Div | BinaryOp::Rem => {
                let rem = op == BinaryOp::Rem;
                let detail = if rem { "%" } else { "/" };
                let zero = self.f.val(format!("icmp eq {t} {b}, 0"));
                self.fail_arith_if(&zero, node, detail, t, signed, &a, &b);
                if signed {
                    let minus_one = self.f.val(format!("icmp eq i64 {b}, -1"));
                    let min = self.f.val(format!("icmp eq i64 {a}, {}", i64::MIN));
                    let over = self.f.val(format!("and i1 {minus_one}, {min}"));
                    self.fail_arith_if(&over, node, detail, t, signed, &a, &b);
                }
                match (op, signed) {
                    (BinaryOp::Div, true) => "sdiv",
                    (BinaryOp::Div, false) => "udiv",
                    (_, true) => "srem",
                    (_, false) => "urem",
                }
            }
            BinaryOp::And
            | BinaryOp::Or
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr => unreachable!("handled above"),
        };
        Val::Scalar(self.f.val(format!("{instr} {t} {a}, {b}")))
    }

    /// `l + r` on `List<element>` (`docs/slice.md` 25.5): an owned copy of
    /// `l` with the elements of `r` appended, copied as `append` copies them.
    fn list_concat(&mut self, ty: &Ty, element: &Ty, l: Val, r: Val) -> Val {
        let out = self.owned(ty, l);
        let r = if self.env.needs_glue(element) {
            self.owned(ty, r)
        } else {
            r
        };
        let (p, rp) = (self.ptr(&out), self.ptr(&r));
        let layout = self.layout(element);
        let rlen = self.list_len(&rp);
        self.f.ins(format!(
            "call void @ignite.reserve(ptr {p}, i64 {rlen}, i64 {}, i64 {})",
            layout.size, layout.align
        ));
        let len = self.list_len(&p);
        let data = self.f.val(format!("load ptr, ptr {p}, align 8"));
        let at = self.f.val(format!("mul i64 {len}, {}", layout.size));
        let dst = self
            .f
            .val(format!("getelementptr inbounds i8, ptr {data}, i64 {at}"));
        let src = self.f.val(format!("load ptr, ptr {rp}, align 8"));
        let bytes = self.f.val(format!("mul i64 {rlen}, {}", layout.size));
        self.f.ins(format!(
            "call void @llvm.memcpy.p0.p0.i64(ptr {dst}, ptr {src}, i64 {bytes}, i1 false)"
        ));
        let next = self.f.val(format!("add i64 {len}, {rlen}"));
        let lenp = self.gep(&p, 8);
        self.f.ins(format!("store i64 {next}, ptr {lenp}, align 8"));
        out
    }

    /// `a op b` on `i64` for a bitwise `op` (`docs/slice.md` 29.2). LLVM's
    /// `shl` and `ashr` are poison for an amount outside `0..64`, so `<<`
    /// selects 0 for one (the unselected poison does not propagate) and
    /// `>>` shifts by at most 63, the sign fill; an amount is compared
    /// unsigned, so a negative one counts as too large.
    fn bitwise(&mut self, op: BinaryOp, a: &str, b: &str) -> String {
        match op {
            BinaryOp::BitAnd => self.f.val(format!("and i64 {a}, {b}")),
            BinaryOp::BitOr => self.f.val(format!("or i64 {a}, {b}")),
            BinaryOp::BitXor => self.f.val(format!("xor i64 {a}, {b}")),
            BinaryOp::Shl => {
                let fits = self.f.val(format!("icmp ult i64 {b}, 64"));
                let shifted = self.f.val(format!("shl i64 {a}, {b}"));
                self.f
                    .val(format!("select i1 {fits}, i64 {shifted}, i64 0"))
            }
            _ => {
                let fits = self.f.val(format!("icmp ult i64 {b}, 64"));
                let amount = self.f.val(format!("select i1 {fits}, i64 {b}, i64 63"));
                self.f.val(format!("ashr i64 {a}, {amount}"))
            }
        }
    }

    /// `a op b` for `+`, `-` and `*` through `llvm.*.with.overflow`: an
    /// overflow fails with the `docs/llvm.md` section 4 report, signed on
    /// `Int`, unsigned on `Byte`.
    fn overflowing(
        &mut self,
        op: BinaryOp,
        node: &Expr,
        t: &str,
        signed: bool,
        a: &str,
        b: &str,
    ) -> String {
        let (name, detail) = match op {
            BinaryOp::Add => ("add", "+"),
            BinaryOp::Sub => ("sub", "-"),
            _ => ("mul", "*"),
        };
        let sign = if signed { 's' } else { 'u' };
        let pair = self.f.val(format!(
            "call {{ {t}, i1 }} @llvm.{sign}{name}.with.overflow.{t}({t} {a}, {t} {b})"
        ));
        let over = self.f.val(format!("extractvalue {{ {t}, i1 }} {pair}, 1"));
        self.fail_arith_if(&over, node, detail, t, signed, a, b);
        self.f.val(format!("extractvalue {{ {t}, i1 }} {pair}, 0"))
    }

    /// The source position of a failure at `e`: a `Text` view of the module
    /// path (a literal store into the current block) and the 1-based line and
    /// column, from the typed failure site when one was recorded, else `e`'s
    /// own start.
    fn fail_pos(&mut self, e: &Expr) -> (String, usize, usize) {
        let offset = self
            .cx
            .types
            .failure_site(e)
            .map(|site| site.offset)
            .unwrap_or(e.span.start);
        self.position_parts(offset)
    }

    /// A `Text` view of the module path and the line and column of `offset`;
    /// the module name and `0:0` when the driver gave no source file.
    fn position_parts(&mut self, offset: usize) -> (String, usize, usize) {
        let (path, line, col) = match self.m.file(&self.cx.module) {
            Some(file) => {
                let (line, col) = file.line_col(offset);
                (file.path.to_string(), line, col)
            }
            None => (self.cx.module.clone(), 0, 0),
        };
        let file = self.literal_ref(path.as_bytes());
        (file, line, col)
    }

    /// Widen a scalar of IR type `t` to `i64` for a failure detail: a no-op
    /// for `i64`, sign- or zero-extended for a narrower type.
    fn widen64(&mut self, t: &str, signed: bool, x: &str) -> String {
        if t == "i64" {
            x.to_string()
        } else if signed {
            self.f.val(format!("sext {t} {x} to i64"))
        } else {
            self.f.val(format!("zext {t} {x} to i64"))
        }
    }

    /// Fail the arithmetic operation `op` of `node` when `cond` holds, with
    /// the operands `a` and `b` (IR type `t`) widened into the detail. A
    /// per-site cold block keeps the common path free of landing pads.
    #[allow(clippy::too_many_arguments)]
    fn fail_arith_if(
        &mut self,
        cond: &str,
        node: &Expr,
        op: &str,
        t: &str,
        signed: bool,
        a: &str,
        b: &str,
    ) {
        self.m.declare(
            "ignite_rt_fail_arith",
            "void",
            &[
                "ptr".into(),
                "ptr".into(),
                "i64".into(),
                "i64".into(),
                "i64".into(),
                "i64".into(),
            ],
        );
        let (bad, good) = (self.f.label(), self.f.label());
        self.f.cond_br(cond, &bad, &good);
        self.f.place(&bad);
        let a = self.widen64(t, signed, a);
        let b = self.widen64(t, signed, b);
        let (file, line, col) = self.fail_pos(node);
        let op = self.literal_ref(op.as_bytes());
        self.f.ins(format!(
            "call void @ignite_rt_fail_arith(ptr {op}, ptr {file}, i64 {line}, i64 {col}, i64 {a}, i64 {b})"
        ));
        self.f.term("unreachable");
        self.f.place(&good);
    }

    /// Fail the negation of `node` when `cond` holds, with `value` (an `i64`)
    /// in the detail.
    fn fail_neg_if(&mut self, cond: &str, node: &Expr, value: &str) {
        self.m.declare(
            "ignite_rt_fail_neg",
            "void",
            &["ptr".into(), "i64".into(), "i64".into(), "i64".into()],
        );
        let (bad, good) = (self.f.label(), self.f.label());
        self.f.cond_br(cond, &bad, &good);
        self.f.place(&bad);
        let (file, line, col) = self.fail_pos(node);
        self.f.ins(format!(
            "call void @ignite_rt_fail_neg(ptr {file}, i64 {line}, i64 {col}, i64 {value})"
        ));
        self.f.term("unreachable");
        self.f.place(&good);
    }

    /// Fail an index of `node` when `cond` holds: `index` out of range for a
    /// list of length `len`.
    fn fail_index_if(&mut self, cond: &str, node: &Expr, index: &str, len: &str) {
        self.m.declare(
            "ignite_rt_fail_index",
            "void",
            &[
                "ptr".into(),
                "i64".into(),
                "i64".into(),
                "i64".into(),
                "i64".into(),
            ],
        );
        let (bad, good) = (self.f.label(), self.f.label());
        self.f.cond_br(cond, &bad, &good);
        self.f.place(&bad);
        let (file, line, col) = self.fail_pos(node);
        self.f.ins(format!(
            "call void @ignite_rt_fail_index(ptr {file}, i64 {line}, i64 {col}, i64 {index}, i64 {len})"
        ));
        self.f.term("unreachable");
        self.f.place(&good);
    }

    /// Fail a slice of `node` when `cond` holds: `from..to` out of range for
    /// a list of length `len`.
    fn fail_slice_if(&mut self, cond: &str, node: &Expr, from: &str, to: &str, len: &str) {
        self.m.declare(
            "ignite_rt_fail_slice",
            "void",
            &[
                "ptr".into(),
                "i64".into(),
                "i64".into(),
                "i64".into(),
                "i64".into(),
                "i64".into(),
            ],
        );
        let (bad, good) = (self.f.label(), self.f.label());
        self.f.cond_br(cond, &bad, &good);
        self.f.place(&bad);
        let (file, line, col) = self.fail_pos(node);
        self.f.ins(format!(
            "call void @ignite_rt_fail_slice(ptr {file}, i64 {line}, i64 {col}, i64 {from}, i64 {to}, i64 {len})"
        ));
        self.f.term("unreachable");
        self.f.place(&good);
    }

    /// Fail a `match` of `e` with no arm matched; the caller terminates the
    /// block.
    fn fail_match(&mut self, e: &Expr) {
        let offset = self
            .cx
            .types
            .failure_site(e)
            .map(|site| site.offset)
            .unwrap_or(e.span.start);
        self.fail_match_at(offset);
    }

    /// Fail a destructuring that did not match at `offset` (no arm matched);
    /// the caller terminates the block.
    fn fail_match_at(&mut self, offset: usize) {
        self.m.declare(
            "ignite_rt_fail_match",
            "void",
            &["ptr".into(), "i64".into(), "i64".into()],
        );
        let (file, line, col) = self.position_parts(offset);
        self.f.ins(format!(
            "call void @ignite_rt_fail_match(ptr {file}, i64 {line}, i64 {col})"
        ));
    }

    // ---- patterns --------------------------------------------------------

    /// Test `pattern` against `v` of type `ty`: falls through on a match
    /// (with bindings in the current scope), branches to `fail` otherwise.
    fn pattern(&mut self, pattern: &Pattern, v: &Val, ty: &Ty, fail: &str) {
        match &pattern.kind {
            PatternKind::Range { .. } => {
                let (lo, hi) = self
                    .cx
                    .types
                    .range_bounds(pattern)
                    .expect("a typed range pattern");
                let x = self.scalar(v);
                let (t, lo, hi, ge, le) = match ty {
                    Ty::Byte => (
                        "i8",
                        ((lo as u8) as i8).to_string(),
                        ((hi as u8) as i8).to_string(),
                        "uge",
                        "ule",
                    ),
                    _ => ("i64", lo.to_string(), hi.to_string(), "sge", "sle"),
                };
                let above = self.f.val(format!("icmp {ge} {t} {x}, {lo}"));
                let below = self.f.val(format!("icmp {le} {t} {x}, {hi}"));
                let ok = self.f.val(format!("and i1 {above}, {below}"));
                self.branch_or_fail(&ok, fail);
            }
            _ if self.cx.types.pattern_const(pattern).is_some() => {
                let q = self
                    .cx
                    .types
                    .pattern_const(pattern)
                    .cloned()
                    .expect("checked above");
                self.const_pattern(&q, v, ty, fail);
            }
            PatternKind::Bool(b) => {
                let x = self.scalar(v);
                let ok = self.f.val(format!("icmp eq i1 {x}, {b}"));
                self.branch_or_fail(&ok, fail);
            }
            PatternKind::Or(alternatives) => {
                let matched = self.f.label();
                for (i, alternative) in alternatives.iter().enumerate() {
                    if i + 1 == alternatives.len() {
                        self.pattern(alternative, v, ty, fail);
                    } else {
                        let next = self.f.label();
                        self.pattern(alternative, v, ty, &next);
                        self.f.br(&matched);
                        self.f.place(&next);
                    }
                }
                self.f.br(&matched);
                self.f.place(&matched);
            }
            PatternKind::Tuple(items) if items.is_empty() => {}
            PatternKind::Tuple(items) => {
                let elements = match ty {
                    Ty::Tuple(elements) => elements.clone(),
                    other => panic!("tuple pattern on {other:?}"),
                };
                let (offsets, _) = self.env.tuple_layout(&elements);
                let p = self.ptr(v);
                for ((item, element), offset) in items.iter().zip(&elements).zip(offsets) {
                    let at = self.gep(&p, offset);
                    let sub = self.part(element, at);
                    self.pattern(item, &sub, element, fail);
                }
            }
            PatternKind::List(items) => {
                let element = match ty {
                    Ty::List(t) => (**t).clone(),
                    Ty::Text => Ty::Byte,
                    other => panic!("list pattern on {other:?}"),
                };
                let p = self.ptr(v);
                let len = self.list_len(&p);
                if let Some(at) = items
                    .iter()
                    .position(|item| matches!(item.kind, PatternKind::Rest(_)))
                {
                    self.rest_list_pattern(items, at, &p, &len, ty, &element, fail);
                    return;
                }
                let ok = self.f.val(format!("icmp eq i64 {len}, {}", items.len()));
                self.branch_or_fail(&ok, fail);
                if !items.is_empty() {
                    let size = self.layout(&element).size;
                    let data = self.f.val(format!("load ptr, ptr {p}, align 8"));
                    for (i, item) in items.iter().enumerate() {
                        let at = self.gep(&data, size * i as u64);
                        let sub = self.part(&element, at);
                        self.pattern(item, &sub, &element, fail);
                    }
                }
            }
            PatternKind::Wildcard | PatternKind::Rest(None) => {}
            PatternKind::Rest(Some(id)) => {
                let owned = self.owned(ty, v.clone());
                self.bind_value(&id.name, ty, owned, true);
            }
            PatternKind::Int(n) => {
                let x = self.scalar(v);
                let (t, n) = match ty {
                    Ty::Byte => ("i8", ((*n as u8) as i8).to_string()),
                    _ => ("i64", n.to_string()),
                };
                let ok = self.f.val(format!("icmp eq {t} {x}, {n}"));
                self.branch_or_fail(&ok, fail);
            }
            PatternKind::Str(text) => {
                let p = self.ptr(v);
                let lit = self.literal_ref(text.as_bytes());
                let ok = crate::glue::text_eq(self.m, &mut self.f, &p, &lit);
                self.branch_or_fail(&ok, fail);
            }
            PatternKind::Binding(id) => {
                if id.is_uppercase() && self.is_variant_of(ty, &id.name) {
                    self.variant_pattern(&id.name, &[], v, ty, fail);
                } else {
                    let in_memory = matches!(v, Val::Ptr(..));
                    self.bind_value(&id.name, ty, v.clone(), in_memory);
                }
            }
            PatternKind::Variant { path, args } => {
                let name = path.last().map(|i| i.name.as_str()).unwrap_or("");
                self.variant_pattern(name, args, v, ty, fail);
            }
        }
    }

    /// A list pattern whose element `at` is a rest (`docs/slice.md` 20.5):
    /// `len >= k` for the `k` other elements, prefix element `i` at index
    /// `i`, suffix element `j` of `s` at `len - s + j`; a `..name` binds a
    /// copy of elements `at` to `len - s`, made after every element test.
    #[allow(clippy::too_many_arguments)]
    fn rest_list_pattern(
        &mut self,
        items: &[Pattern],
        at: usize,
        p: &str,
        len: &str,
        ty: &Ty,
        element: &Ty,
        fail: &str,
    ) {
        let fixed = items.len() - 1;
        let suffix = fixed - at;
        let ok = self.f.val(format!("icmp sge i64 {len}, {fixed}"));
        self.branch_or_fail(&ok, fail);
        let wild =
            |item: &Pattern| matches!(item.kind, PatternKind::Wildcard | PatternKind::Rest(None));
        if items.iter().all(wild) {
            return;
        }
        let size = self.layout(element).size;
        let data = self.f.val(format!("load ptr, ptr {p}, align 8"));
        for (i, item) in items[..at].iter().enumerate() {
            if wild(item) {
                continue;
            }
            let place = self.gep(&data, size * i as u64);
            let sub = self.part(element, place);
            self.pattern(item, &sub, element, fail);
        }
        let named = matches!(items[at].kind, PatternKind::Rest(Some(_)));
        let end = if suffix == 0 {
            len.to_string()
        } else {
            self.f.val(format!("sub i64 {len}, {suffix}"))
        };
        if !items[at + 1..].iter().all(wild) {
            let end_bytes = self.f.val(format!("mul i64 {end}, {size}"));
            let tail = self.f.val(format!(
                "getelementptr inbounds i8, ptr {data}, i64 {end_bytes}"
            ));
            for (j, item) in items[at + 1..].iter().enumerate() {
                if wild(item) {
                    continue;
                }
                let place = self.gep(&tail, size * j as u64);
                let sub = self.part(element, place);
                self.pattern(item, &sub, element, fail);
            }
        }
        if named {
            let start = self.gep(&data, size * at as u64);
            let count = self.f.val(format!("sub i64 {end}, {at}"));
            let view = self.temp(ty);
            self.f
                .ins(format!("store ptr {start}, ptr {view}, align 8"));
            let lenp = self.gep(&view, 8);
            self.f
                .ins(format!("store i64 {count}, ptr {lenp}, align 8"));
            let capp = self.gep(&view, 16);
            self.f
                .ins(format!("store i64 {count}, ptr {capp}, align 8"));
            self.pattern(&items[at], &Val::Ptr(view, false), ty, fail);
        }
    }

    fn is_variant_of(&self, ty: &Ty, name: &str) -> bool {
        matches!(
            ty,
            Ty::Option(_) | Ty::Result(..) | Ty::Input | Ty::Enum(_) | Ty::GenericEnum(..)
        ) && self.env.variants(ty).iter().any(|(n, _)| n == name)
    }

    fn variant_pattern(&mut self, name: &str, args: &[Pattern], v: &Val, ty: &Ty, fail: &str) {
        let layout = self.env.enum_layout(ty);
        let index = layout
            .index(name)
            .unwrap_or_else(|| panic!("no variant `{name}` in {ty:?}"));
        let p = self.ptr(v);
        let tag = self.f.val(format!("load i32, ptr {p}, align 4"));
        let ok = self.f.val(format!("icmp eq i32 {tag}, {index}"));
        self.branch_or_fail(&ok, fail);
        let fields = layout.variants[index].1.clone();
        for (i, (arg, (field_ty, offset))) in args.iter().zip(fields).enumerate() {
            let fp = self.gep(&p, offset);
            let boxed = self.env.payload_boxed(ty, index, i);
            let fp = self.deref_position(fp, boxed);
            let sub = self.part(&field_ty, fp);
            self.pattern(arg, &sub, &field_ty, fail);
        }
    }

    /// The part of type `ty` at `ptr` inside a scrutinee the match owns: an
    /// aggregate in place, a scalar loaded.
    fn part(&mut self, ty: &Ty, ptr: String) -> Val {
        if is_aggregate(ty) {
            Val::Ptr(ptr, true)
        } else if is_void(ty) {
            Val::Unit
        } else {
            Val::Scalar(self.load_scalar(ty, &ptr))
        }
    }

    /// A constant pattern: the value `==` the constant `q`.
    fn const_pattern(&mut self, q: &QualName, v: &Val, ty: &Ty, fail: &str) {
        let cty = self.env.const_ty(q).map(repr).unwrap_or_else(|| ty.clone());
        let c = self.constant(q, &cty);
        let ok = if *ty == Ty::Text {
            let (a, b) = (self.ptr(v), self.ptr(&c));
            crate::glue::text_eq(self.m, &mut self.f, &a, &b)
        } else {
            let c = self.coerce(ty, &cty, c);
            let t = scalar_type(ty).expect("a constant pattern on a scalar or Text");
            let (a, b) = (self.scalar(v), self.scalar(&c));
            self.f.val(format!("icmp eq {t} {a}, {b}"))
        };
        self.branch_or_fail(&ok, fail);
    }

    fn branch_or_fail(&mut self, ok: &str, fail: &str) {
        let next = self.f.label();
        self.f.cond_br(ok, &next, fail);
        self.f.place(&next);
    }

    // ---- builtins --------------------------------------------------------

    fn list_at(&mut self, e: &Expr, receiver: &Expr, args: &[Expr]) -> Val {
        let element = match self.ty(receiver) {
            Ty::List(t) => *t,
            Ty::Text => Ty::Byte,
            other => panic!("index into {other:?}"),
        };
        let r = self.expr(receiver, true);
        let (rp, owned) = match &r {
            Val::Ptr(p, owned) => (p.clone(), *owned),
            _ => return Val::Never,
        };
        let i = self.expr(&args[0], true);
        let i = self.scalar(&i);
        let len = self.list_len(&rp);
        let oob = self.f.val(format!("icmp uge i64 {i}, {len}"));
        self.fail_index_if(&oob, e, &i, &len);
        let size = self.layout(&element).size;
        let data = self.f.val(format!("load ptr, ptr {rp}, align 8"));
        let at = self.f.val(format!("mul i64 {i}, {size}"));
        let p = self
            .f
            .val(format!("getelementptr inbounds i8, ptr {data}, i64 {at}"));
        self.at(&element, p, owned)
    }

    /// Call a runtime function `ignite_rt_<name>(args.., [ok out,] err)` and
    /// wrap its status as a `Result<ok, IoError>`.
    fn rt_result(&mut self, name: &str, args: &[String], ok: Ty) -> Val {
        let ty = Ty::Result(Box::new(ok.clone()), Box::new(Ty::IoError));
        let layout = self.env.enum_layout(&ty);
        let result = self.temp(&ty);
        let mut operands: Vec<String> = args.to_vec();
        let out = (!is_void(&ok)).then(|| self.temp(&ok));
        if let Some(out) = &out {
            operands.push(format!("ptr {out}"));
        }
        let err = self.temp(&Ty::IoError);
        operands.push(format!("ptr {err}"));
        let status = self.f.val(format!(
            "call i32 @ignite_rt_{name}({})",
            operands.join(", ")
        ));
        let failed = self.f.val(format!("icmp ne i32 {status}, 0"));
        let (bad, good, end) = (self.f.label(), self.f.label(), self.f.label());
        self.f.cond_br(&failed, &bad, &good);
        self.f.place(&bad);
        self.f.ins(format!("store i32 1, ptr {result}, align 4"));
        let dst = self.gep(&result, layout.variants[1].1[0].1);
        self.memcpy(&dst, &err, 32);
        self.f.br(&end);
        self.f.place(&good);
        self.f.ins(format!("store i32 0, ptr {result}, align 4"));
        if let Some(out) = out {
            let dst = self.gep(&result, layout.variants[0].1[0].1);
            let size = self.layout(&ok).size;
            self.memcpy(&dst, &out, size);
        }
        self.f.br(&end);
        self.f.place(&end);
        Val::Ptr(result, true)
    }

    /// The runtime entry (without `ignite_rt_`) of the stream method `op`
    /// that builtin `b` calls (`docs/slice.md` 18.11, D11): `tcp_<op>` for
    /// a `TcpStream`, `pipe_<op>` for a `PipeStream`. A `Stream*` builtin
    /// of a `ByteStream`-bounded receiver takes the kind of the receiver's
    /// substituted type, so each instance calls its own entries and no
    /// dispatch is left for run time.
    fn stream_entry(&mut self, b: Builtin, receiver: Option<&Expr>, op: &str) -> String {
        use Builtin::*;
        let pipe = match b {
            TcpStreamRead
            | TcpStreamWriteAll
            | TcpStreamClose
            | TcpStreamSetIdleTimeout
            | TcpStreamWriteUrgent
            | TcpStreamUrgentPending => false,
            PipeStreamRead
            | PipeStreamWriteAll
            | PipeStreamClose
            | PipeStreamSetIdleTimeout
            | PipeStreamWriteUrgent
            | PipeStreamUrgentPending => true,
            _ => match self.ty(receiver.expect("a stream method has a receiver")) {
                Ty::TcpStream => false,
                Ty::PipeStream => true,
                other => panic!(
                    "`{b:?}` on a receiver of type {other:?}: an instance substitutes \
                     `TcpStream` or `PipeStream` for a `ByteStream` bound"
                ),
            },
        };
        if pipe {
            let entry = format!("pipe_{op}");
            self.m.declare_pipe(&format!("ignite_rt_{entry}"));
            entry
        } else {
            format!("tcp_{op}")
        }
    }

    /// A runtime call `void ignite_rt_<name>(ptr out, args..)` producing `ty`.
    fn rt_out(&mut self, name: &str, ty: &Ty, args: &[String]) -> Val {
        let out = self.temp(ty);
        let mut operands = vec![format!("ptr {out}")];
        operands.extend(args.iter().cloned());
        self.f.ins(format!(
            "call void @ignite_rt_{name}({})",
            operands.join(", ")
        ));
        Val::Ptr(out, true)
    }

    /// Pointer to an aggregate argument that is only read.
    fn read_arg(&mut self, e: &Expr) -> String {
        let v = self.expr(e, true);
        self.ptr(&v)
    }

    /// The receiver of a builtin that changes it in place (a `Value` row
    /// of `ignite_types::ownership::table`). A place
    /// ([`ownership::place_root`]: a name, field, index or tuple field)
    /// rooted at a local is changed in place; one rooted at a constant
    /// ([`Lower::constant`]), and any other value, such as `x.to_bytes()`,
    /// which is `x`'s own memory, or `xs.at(0)` and `o?`, which point into
    /// their operand, is copied first, so the change goes to a temporary,
    /// as on the Rust target, and never to a variable the analysis counts
    /// as unmodified (`docs/vision.md` section 10.1, M1).
    fn mutable_receiver(&mut self, receiver: &Expr) -> Val {
        let r = self.expr(receiver, true);
        if ownership::place_root(receiver).is_some_and(|n| self.lookup(n).is_some()) {
            return r;
        }
        let ty = self.ty(receiver);
        self.owned(&ty, r)
    }

    /// `receiver.slice(from, to)` (`e`) as a borrowed view: a list header
    /// with the address of element `from` of the receiver and the length
    /// `to - from`, the elements not copied (`docs/llvm.md` section 4). The
    /// caller reads it before it evaluates anything else, or copies it.
    /// When neither bound modifies a variable, a `slice` receiver is a view
    /// too ([`Self::slice_view`]): nothing evaluated between the two can
    /// change the elements it points at. When one does, a borrowed receiver
    /// is copied before the bounds run, so the slice is of the value the
    /// receiver had (`b.slice(0, if c { b = ..; 1 } else { 2 })`).
    fn list_slice(&mut self, e: &Expr, receiver: &Expr, args: &[Expr]) -> Val {
        let ty = self.ty(receiver);
        let element = match &ty {
            Ty::List(t) => (**t).clone(),
            _ => Ty::Byte,
        };
        let r = if args
            .iter()
            .all(|arg| ownership::changed_in(arg, Some(self.cx.types)).is_empty())
        {
            self.slice_view(receiver)
        } else {
            let r = self.expr(receiver, true);
            self.owned(&ty, r)
        };
        let p = self.ptr(&r);
        let i = self.scalar_arg(&args[0]);
        let j = self.scalar_arg(&args[1]);
        let len = self.list_len(&p);
        let neg = self.f.val(format!("icmp slt i64 {i}, 0"));
        let order = self.f.val(format!("icmp sgt i64 {i}, {j}"));
        let past = self.f.val(format!("icmp sgt i64 {j}, {len}"));
        let bad = self.f.val(format!("or i1 {neg}, {order}"));
        let bad = self.f.val(format!("or i1 {bad}, {past}"));
        self.fail_slice_if(&bad, e, &i, &j, &len);
        let size = self.layout(&element).size;
        let data = self.f.val(format!("load ptr, ptr {p}, align 8"));
        let at = self.f.val(format!("mul i64 {i}, {size}"));
        let start = self
            .f
            .val(format!("getelementptr inbounds i8, ptr {data}, i64 {at}"));
        let count = self.f.val(format!("sub i64 {j}, {i}"));
        let view = self.temp(&ty);
        self.f
            .ins(format!("store ptr {start}, ptr {view}, align 8"));
        let lenp = self.gep(&view, 8);
        self.f
            .ins(format!("store i64 {count}, ptr {lenp}, align 8"));
        let capp = self.gep(&view, 16);
        self.f
            .ins(format!("store i64 {count}, ptr {capp}, align 8"));
        Val::Ptr(view, false)
    }

    /// The value of `e` where the caller only reads it before it evaluates
    /// anything else (the receiver of `to_text`, the bytes of
    /// `Int.parse(bytes.to_text())`): a `slice` call as the view of
    /// [`Self::list_slice`], with no copy; any other expression as
    /// [`Self::expr`].
    fn slice_view(&mut self, e: &Expr) -> Val {
        let ExprKind::Method { receiver, args, .. } = &e.kind else {
            return self.expr(e, true);
        };
        if !matches!(
            self.cx.types.callee(e),
            Some(Callee::Builtin(Builtin::ListSlice))
        ) {
            return self.expr(e, true);
        }
        self.nested += 1;
        let previous = self.locate(e.span.start);
        let view = self.list_slice(e, receiver, args);
        if let Some(previous) = previous {
            self.f.restore_location(previous);
        }
        self.nested -= 1;
        view
    }

    /// The debug text (`docs/slice.md` section 13.6) of `v` of type `ty`,
    /// as an owned `Text`.
    fn debug_text(&mut self, ty: &Ty, v: &Val) -> Val {
        let buf = self.empty_list();
        let b = self.ptr(&buf);
        let value = match v {
            Val::Ptr(p, _) => p.clone(),
            Val::Scalar(op) => {
                let t = self.temp(ty);
                self.store_scalar(ty, &t, op);
                t
            }
            Val::Unit | Val::Never => "null".to_string(),
        };
        crate::glue::append_debug(self.m, &mut self.f, &b, ty, &value);
        buf
    }

    fn scalar_arg(&mut self, e: &Expr) -> String {
        let v = self.expr(e, true);
        self.scalar(&v)
    }

    /// A capability builtin (`docs/slice.md` 21.3 to 21.8): a call of its
    /// runtime entry, declared on first use so a module without
    /// capabilities keeps its text. A capability is an `i64` handle of the
    /// runtime's table; `Invocation` and `ChildExit` cross as their record
    /// layouts (`IgniteInvocation`, `IgniteChildExit`).
    fn capability(&mut self, b: Builtin, receiver: Option<&Expr>, args: &[Expr]) -> Val {
        use Builtin::*;
        let ptr = || "ptr".to_string();
        let i64 = || "i64".to_string();
        let handle = |this: &mut Self| -> String {
            let r = this.expr(receiver.expect("a method has a receiver"), true);
            this.scalar(&r)
        };
        let child_exit = ignite_types::child_exit();
        let texts = Ty::List(Box::new(Ty::Text));
        match b {
            DirectoryTemporary => {
                self.m
                    .declare("ignite_rt_directory_temporary", "i32", &[ptr(), ptr()]);
                self.rt_result(
                    "directory_temporary",
                    &[],
                    Ty::Capability(Capability::Directory),
                )
            }
            DirectoryReadFile | DirectoryReadDirectory => {
                let (entry, ok) = if b == DirectoryReadFile {
                    ("directory_read_file", bytes())
                } else {
                    ("directory_read_directory", texts)
                };
                self.m.declare(
                    &format!("ignite_rt_{entry}"),
                    "i32",
                    &[i64(), ptr(), ptr(), ptr()],
                );
                let d = handle(self);
                let path = self.read_arg(&args[0]);
                self.rt_result(entry, &[format!("i64 {d}"), format!("ptr {path}")], ok)
            }
            DirectoryWriteFile => {
                self.m.declare(
                    "ignite_rt_directory_write_file",
                    "i32",
                    &[i64(), ptr(), ptr(), ptr()],
                );
                let d = handle(self);
                let path = self.read_arg(&args[0]);
                let data = self.read_arg(&args[1]);
                self.rt_result(
                    "directory_write_file",
                    &[
                        format!("i64 {d}"),
                        format!("ptr {path}"),
                        format!("ptr {data}"),
                    ],
                    Ty::Unit,
                )
            }
            DirectoryCreateDirectory | DirectoryRemove | DirectoryRemoveTree => {
                let entry = match b {
                    DirectoryCreateDirectory => "directory_create_directory",
                    DirectoryRemove => "directory_remove",
                    _ => "directory_remove_tree",
                };
                self.m
                    .declare(&format!("ignite_rt_{entry}"), "i32", &[i64(), ptr(), ptr()]);
                let d = handle(self);
                let path = self.read_arg(&args[0]);
                self.rt_result(
                    entry,
                    &[format!("i64 {d}"), format!("ptr {path}")],
                    Ty::Unit,
                )
            }
            DirectoryAbsolutePath => {
                self.m.declare(
                    "ignite_rt_directory_absolute_path",
                    "i32",
                    &[i64(), ptr(), ptr()],
                );
                let d = handle(self);
                self.rt_result("directory_absolute_path", &[format!("i64 {d}")], Ty::Text)
            }
            LauncherNew => {
                self.m.declare("ignite_rt_launcher_new", "i64", &[i64()]);
                Val::Scalar(self.f.val(format!(
                    "call i64 @ignite_rt_launcher_new(i64 {})",
                    ignite_types::LAUNCHER_DEFAULT_MAXIMUM_CHILDREN
                )))
            }
            LauncherStart | LauncherExecute => {
                let (entry, ok) = if b == LauncherStart {
                    ("launcher_start", Ty::Capability(Capability::ChildProcess))
                } else {
                    ("launcher_execute", child_exit)
                };
                self.m.declare(
                    &format!("ignite_rt_{entry}"),
                    "i32",
                    &[i64(), ptr(), ptr(), ptr()],
                );
                let l = handle(self);
                let invocation = self.read_arg(&args[0]);
                self.rt_result(
                    entry,
                    &[format!("i64 {l}"), format!("ptr {invocation}")],
                    ok,
                )
            }
            ChildProcessId => {
                self.m.declare("ignite_rt_child_id", "i64", &[i64()]);
                let c = handle(self);
                Val::Scalar(self.f.val(format!("call i64 @ignite_rt_child_id(i64 {c})")))
            }
            ChildProcessWait => {
                self.m
                    .declare("ignite_rt_child_wait", "i32", &[i64(), ptr(), ptr()]);
                let c = handle(self);
                self.rt_result("child_wait", &[format!("i64 {c}")], child_exit)
            }
            ChildProcessKill => {
                self.m
                    .declare("ignite_rt_child_kill", "i32", &[i64(), ptr()]);
                let c = handle(self);
                self.rt_result("child_kill", &[format!("i64 {c}")], Ty::Unit)
            }
            EntropyNew => {
                self.m.declare("ignite_rt_entropy_new", "i64", &[]);
                Val::Scalar(self.f.val("call i64 @ignite_rt_entropy_new()"))
            }
            EntropyRandomBytes => {
                self.m.declare(
                    "ignite_rt_entropy_random_bytes",
                    "i32",
                    &[i64(), i64(), ptr(), ptr()],
                );
                let e = handle(self);
                let count = self.scalar_arg(&args[0]);
                self.rt_result(
                    "entropy_random_bytes",
                    &[format!("i64 {e}"), format!("i64 {count}")],
                    bytes(),
                )
            }
            SettingOf => {
                self.m
                    .declare("ignite_rt_setting_of", "i64", &[ptr(), ptr()]);
                let flag = self.read_arg(&args[0]);
                let value = self.read_arg(&args[1]);
                Val::Scalar(self.f.val(format!(
                    "call i64 @ignite_rt_setting_of(ptr {flag}, ptr {value})"
                )))
            }
            SettingValue => {
                self.m
                    .declare("ignite_rt_setting_value", "void", &[ptr(), i64()]);
                let s = handle(self);
                self.rt_out("setting_value", &Ty::Text, &[format!("i64 {s}")])
            }
            CommandLineOf => {
                self.m.declare("ignite_rt_command_line_of", "i64", &[ptr()]);
                let arguments = self.read_arg(&args[0]);
                Val::Scalar(self.f.val(format!(
                    "call i64 @ignite_rt_command_line_of(ptr {arguments})"
                )))
            }
            CommandLineArguments => {
                self.m
                    .declare("ignite_rt_command_line_arguments", "void", &[ptr(), i64()]);
                let c = handle(self);
                self.rt_out("command_line_arguments", &texts, &[format!("i64 {c}")])
            }
            EnvironmentOf => {
                self.m.declare("ignite_rt_environment_of", "i64", &[ptr()]);
                let entries = self.read_arg(&args[0]);
                Val::Scalar(
                    self.f
                        .val(format!("call i64 @ignite_rt_environment_of(ptr {entries})")),
                )
            }
            EnvironmentGet => {
                self.m
                    .declare("ignite_rt_environment_get", "i8", &[i64(), ptr(), ptr()]);
                let e = handle(self);
                let name = self.read_arg(&args[0]);
                self.option_from("environment_get", &Ty::Text, |payload| {
                    format!("i64 {e}, ptr {name}, ptr {payload}")
                })
            }
            other => unreachable!("`{other:?}` is no capability builtin"),
        }
    }

    /// A channel builtin (`docs/slice.md` 30): each end is an `i64` handle
    /// of the runtime's table, a value crosses as a heap copy made and
    /// freed by the element's `Store` glue.
    fn channel(&mut self, e: &Expr, b: Builtin, receiver: Option<&Expr>, args: &[Expr]) -> Val {
        use Builtin::*;
        let ptr = || "ptr".to_string();
        let i64 = || "i64".to_string();
        let element = match receiver.filter(|_| b != ChannelBounded) {
            None => match self.ty(e) {
                Ty::Tuple(ends) => match ends.first() {
                    Some(Ty::Sender(t)) => (**t).clone(),
                    other => panic!("`Channel.bounded` of ends {other:?}"),
                },
                other => panic!("`Channel.bounded` of type {other:?}"),
            },
            Some(r) => match self.ty(r) {
                Ty::Sender(t) | Ty::Receiver(t) => *t,
                other => panic!("channel method on {other:?}"),
            },
        };
        if b == ChannelBounded {
            self.m.declare(
                "ignite_rt_channel_bounded",
                "void",
                &[i64(), ptr(), ptr(), ptr(), i64(), i64(), ptr(), ptr()],
            );
            let capacity = self.scalar_arg(&args[0]);
            let to = self.m.glue_of(Glue::ToHeap, &element);
            let from = self.m.glue_of(Glue::FromHeap, &element);
            let free = self.m.glue_of(Glue::FreeHeap, &element);
            let layout = self.layout(&element);
            let ends = [
                Ty::Sender(Box::new(element.clone())),
                Ty::Receiver(Box::new(element.clone())),
            ];
            let (offsets, _) = self.env.tuple_layout(&ends);
            let pair = self.temp(&Ty::Tuple(ends.to_vec()));
            let first = self.gep(&pair, offsets[0]);
            let second = self.gep(&pair, offsets[1]);
            self.f.ins(format!(
                "call void @ignite_rt_channel_bounded(i64 {capacity}, ptr {to}, ptr {from}, ptr {free}, i64 {}, i64 {}, ptr {first}, ptr {second})",
                layout.size,
                layout.align.max(1)
            ));
            return Val::Ptr(pair, true);
        }
        let r = self.expr(receiver.expect("a channel method has a receiver"), true);
        if r == Val::Never {
            return Val::Never;
        }
        let handle = self.scalar(&r);
        match b {
            SenderSend | SenderTrySend => {
                let name = if b == SenderSend {
                    "ignite_rt_channel_send"
                } else {
                    "ignite_rt_channel_try_send"
                };
                self.m.declare(name, "i8", &[i64(), ptr()]);
                let v = self.expr(&args[0], true);
                if v == Val::Never {
                    return Val::Never;
                }
                let have = self.ty(&args[0]);
                let v = self.coerce(&element, &have, v);
                let value = match v {
                    Val::Ptr(p, _) => p,
                    other => {
                        let t = self.temp(&element);
                        self.store_value(&element, &t, other);
                        t
                    }
                };
                let sent = self
                    .f
                    .val(format!("call i8 @{name}(i64 {handle}, ptr {value})"));
                Val::Scalar(self.f.val(format!("icmp ne i8 {sent}, 0")))
            }
            SenderClose | ReceiverClose => {
                self.m.declare("ignite_rt_channel_close", "void", &[i64()]);
                self.f
                    .ins(format!("call void @ignite_rt_channel_close(i64 {handle})"));
                Val::Unit
            }
            ReceiverReceive => {
                self.m
                    .declare("ignite_rt_channel_receive", "i8", &[i64(), ptr()]);
                self.option_from("channel_receive", &element, |payload| {
                    format!("i64 {handle}, ptr {payload}")
                })
            }
            ReceiverReceiveWithin => {
                self.m.declare(
                    "ignite_rt_channel_receive_within",
                    "i8",
                    &[i64(), i64(), ptr()],
                );
                let duration = self.scalar_arg(&args[0]);
                self.option_from("channel_receive_within", &element, |payload| {
                    format!("i64 {handle}, i64 {duration}, ptr {payload}")
                })
            }
            ReceiverClosed => {
                self.m.declare("ignite_rt_channel_closed", "i8", &[i64()]);
                let closed = self
                    .f
                    .val(format!("call i8 @ignite_rt_channel_closed(i64 {handle})"));
                Val::Scalar(self.f.val(format!("icmp ne i8 {closed}, 0")))
            }
            other => unreachable!("`{other:?}` is no channel builtin"),
        }
    }

    fn builtin(&mut self, e: &Expr, b: Builtin, receiver: Option<&Expr>, args: &[Expr]) -> Val {
        use Builtin::*;
        let recv = |this: &mut Self| -> Val {
            let r = receiver.expect("a method has a receiver");
            this.expr(r, true)
        };
        match b {
            MapEmpty => self.empty_list(),
            MapLen => {
                let r = recv(self);
                let p = self.ptr(&r);
                Val::Scalar(self.list_len(&p))
            }
            MapGet => {
                let (key, value) = self.map_kv(receiver.expect("receiver"));
                let k = self.map_key_arg(&key, &args[0]);
                let r = recv(self);
                let m = self.ptr(&r);
                self.map_get(&key, &value, &m, &k)
            }
            MapContains => {
                let (key, value) = self.map_kv(receiver.expect("receiver"));
                let k = self.map_key_arg(&key, &args[0]);
                let r = recv(self);
                let m = self.ptr(&r);
                let index = self.map_find(&key, &value, &m, &k);
                Val::Scalar(self.f.val(format!("icmp sge i64 {index}, 0")))
            }
            MapKeys => {
                let (key, value) = self.map_kv(receiver.expect("receiver"));
                let r = recv(self);
                let m = self.ptr(&r);
                self.map_keys(&key, &value, &m)
            }
            MapPut => {
                let (key, value) = self.map_kv(receiver.expect("receiver"));
                let k = self.map_key_arg(&key, &args[0]);
                let v = self.expr(&args[1], true);
                if v == Val::Never {
                    return Val::Never;
                }
                let have = self.ty(&args[1]);
                let v = self.coerce(&value, &have, v);
                let r = self.mutable_receiver(receiver.expect("receiver"));
                let m = self.ptr(&r);
                self.map_put(&key, &value, &m, k, v);
                Val::Unit
            }
            MapRemove => {
                let (key, value) = self.map_kv(receiver.expect("receiver"));
                let k = self.map_key_arg(&key, &args[0]);
                let r = self.mutable_receiver(receiver.expect("receiver"));
                let m = self.ptr(&r);
                self.map_remove(&key, &value, &m, &k)
            }
            DurationToMs => {
                let r = recv(self);
                let x = self.scalar(&r);
                Val::Scalar(self.f.val(format!("sdiv i64 {x}, 1000000")))
            }
            DurationToNs | ByteSizeToInt | RatioPpm => {
                let r = recv(self);
                Val::Scalar(self.scalar(&r))
            }
            IoErrorWithExitCode => {
                let ty = self.ty(receiver.expect("receiver"));
                let r = recv(self);
                if r == Val::Never {
                    return Val::Never;
                }
                let owned = self.owned(&ty, r);
                let p = self.ptr(&owned);
                let code = self.scalar_arg(&args[0]);
                let code32 = self.f.val(format!("trunc i64 {code} to i32"));
                let exit = self.gep(&p, 24);
                self.f
                    .ins(format!("store i32 {code32}, ptr {exit}, align 4"));
                Val::Ptr(p, true)
            }
            ModuleRecord => self.module_record(e),
            Debug => {
                let ty = self.ty(receiver.expect("receiver"));
                let r = recv(self);
                if r == Val::Never {
                    return Val::Never;
                }
                self.debug_text(&ty, &r)
            }
            TextLen | ListLen => {
                let r = recv(self);
                let p = self.ptr(&r);
                Val::Scalar(self.list_len(&p))
            }
            TextIsEmpty | ListIsEmpty => {
                let r = recv(self);
                let p = self.ptr(&r);
                let len = self.list_len(&p);
                Val::Scalar(self.f.val(format!("icmp eq i64 {len}, 0")))
            }
            TextTrim => {
                let r = recv(self);
                let p = self.ptr(&r);
                self.rt_out("text_trim", &Ty::Text, &[format!("ptr {p}")])
            }
            TextStartsWith => {
                let r = recv(self);
                let p = self.ptr(&r);
                let prefix = self.read_arg(&args[0]);
                let v = self.f.val(format!(
                    "call i8 @ignite_rt_text_starts_with(ptr {p}, ptr {prefix})"
                ));
                Val::Scalar(self.f.val(format!("icmp ne i8 {v}, 0")))
            }
            TextStripPrefix => self.strip_prefix(receiver.expect("receiver"), &args[0]),
            TextConcat if self.concat_parts(e).is_some() => self.text_chain(e),
            TextConcat => {
                let r = recv(self);
                let p = self.ptr(&r);
                let a = self.text_piece(&p);
                let other = self.read_arg(&args[0]);
                let b = self.text_piece(&other);
                let out = self.temp(&Ty::Text);
                self.concat_into(&out, &[a, b], false);
                Val::Ptr(out, true)
            }
            TextToBytes => self.expr(receiver.expect("receiver"), true),
            Copy => {
                let r = recv(self);
                let ty = self.ty(receiver.expect("receiver"));
                match r {
                    Val::Ptr(p, _) => {
                        let t = self.temp(&ty);
                        self.copy(&ty, &t, &p);
                        Val::Ptr(t, true)
                    }
                    other => other,
                }
            }
            BytesToText => {
                let r = self.slice_view(receiver.expect("receiver"));
                let p = self.ptr(&r);
                self.rt_out("text_from_bytes_lossy", &Ty::Text, &[format!("ptr {p}")])
            }
            ListAt => self.list_at(e, receiver.expect("receiver"), args),
            ListSlice => {
                let rexpr = receiver.expect("receiver");
                let view = self.list_slice(e, rexpr, args);
                let ty = self.ty(rexpr);
                self.owned(&ty, view)
            }
            ListPush => {
                let rexpr = receiver.expect("receiver");
                let element = match self.ty(rexpr) {
                    Ty::List(t) => *t,
                    _ => Ty::Byte,
                };
                let x = self.expr(&args[0], true);
                let have = self.ty(&args[0]);
                let x = self.coerce(&element, &have, x);
                let x = self.owned(&element, x);
                let r = self.mutable_receiver(receiver.expect("receiver"));
                let p = self.ptr(&r);
                let layout = self.layout(&element);
                self.f.ins(format!(
                    "call void @ignite.reserve(ptr {p}, i64 1, i64 {}, i64 {})",
                    layout.size, layout.align
                ));
                let len = self.list_len(&p);
                let data = self.f.val(format!("load ptr, ptr {p}, align 8"));
                let at = self.f.val(format!("mul i64 {len}, {}", layout.size));
                let dst = self
                    .f
                    .val(format!("getelementptr inbounds i8, ptr {data}, i64 {at}"));
                self.store_value(&element, &dst, x);
                let next = self.f.val(format!("add i64 {len}, 1"));
                let lenp = self.gep(&p, 8);
                self.f.ins(format!("store i64 {next}, ptr {lenp}, align 8"));
                Val::Unit
            }
            ListSet => {
                let rexpr = receiver.expect("receiver");
                let element = match self.ty(rexpr) {
                    Ty::List(t) => *t,
                    _ => Ty::Byte,
                };
                let i = self.scalar_arg(&args[0]);
                let x = self.expr(&args[1], true);
                let have = self.ty(&args[1]);
                let x = self.coerce(&element, &have, x);
                let x = self.owned(&element, x);
                let r = self.mutable_receiver(rexpr);
                let p = self.ptr(&r);
                let len = self.list_len(&p);
                let oob = self.f.val(format!("icmp uge i64 {i}, {len}"));
                self.fail_index_if(&oob, e, &i, &len);
                let size = self.layout(&element).size;
                let data = self.f.val(format!("load ptr, ptr {p}, align 8"));
                let at = self.f.val(format!("mul i64 {i}, {size}"));
                let dst = self
                    .f
                    .val(format!("getelementptr inbounds i8, ptr {data}, i64 {at}"));
                self.store_value(&element, &dst, x);
                Val::Unit
            }
            ListAppend => {
                let rexpr = receiver.expect("receiver");
                let ty = self.ty(rexpr);
                let element = match &ty {
                    Ty::List(t) => (**t).clone(),
                    _ => Ty::Byte,
                };
                let c = self.expr(&args[0], true);
                let c = if self.env.needs_glue(&element) {
                    self.owned(&ty, c)
                } else {
                    c
                };
                let cp = self.ptr(&c);
                let r = self.mutable_receiver(receiver.expect("receiver"));
                let p = self.ptr(&r);
                let layout = self.layout(&element);
                let clen = self.list_len(&cp);
                self.f.ins(format!(
                    "call void @ignite.reserve(ptr {p}, i64 {clen}, i64 {}, i64 {})",
                    layout.size, layout.align
                ));
                let len = self.list_len(&p);
                let data = self.f.val(format!("load ptr, ptr {p}, align 8"));
                let at = self.f.val(format!("mul i64 {len}, {}", layout.size));
                let dst = self
                    .f
                    .val(format!("getelementptr inbounds i8, ptr {data}, i64 {at}"));
                let src = self.f.val(format!("load ptr, ptr {cp}, align 8"));
                let bytes = self.f.val(format!("mul i64 {clen}, {}", layout.size));
                self.f.ins(format!(
                    "call void @llvm.memcpy.p0.p0.i64(ptr {dst}, ptr {src}, i64 {bytes}, i1 false)"
                ));
                let next = self.f.val(format!("add i64 {len}, {clen}"));
                let lenp = self.gep(&p, 8);
                self.f.ins(format!("store i64 {next}, ptr {lenp}, align 8"));
                Val::Unit
            }
            ListClear => {
                let r = self.mutable_receiver(receiver.expect("receiver"));
                let p = self.ptr(&r);
                let lenp = self.gep(&p, 8);
                self.f.ins(format!("store i64 0, ptr {lenp}, align 8"));
                Val::Unit
            }
            ListJoin => {
                let r = recv(self);
                let p = self.ptr(&r);
                let sep = self.read_arg(&args[0]);
                self.rt_out(
                    "text_join",
                    &Ty::Text,
                    &[format!("ptr {p}"), format!("ptr {sep}")],
                )
            }
            BytesEmpty | ListEmpty => self.empty_list(),
            BytesWithCapacity | ListWithCapacity => {
                let element = match self.ty(e) {
                    Ty::List(t) => *t,
                    _ => Ty::Byte,
                };
                let n = self.scalar_arg(&args[0]);
                let list = self.empty_list();
                let p = self.ptr(&list);
                let layout = self.layout(&element);
                // `@ignite.reserve` allocates once for `n > 0` (at least 4
                // elements) and leaves the empty list alone for `n <= 0`.
                self.f.ins(format!(
                    "call void @ignite.reserve(ptr {p}, i64 {n}, i64 {}, i64 {})",
                    layout.size, layout.align
                ));
                list
            }
            StoreNew => unreachable!("`Store.new()` is lowered by `store_new`"),
            DirectoryTemporary
            | DirectoryReadFile
            | DirectoryWriteFile
            | DirectoryReadDirectory
            | DirectoryCreateDirectory
            | DirectoryRemove
            | DirectoryRemoveTree
            | DirectoryAbsolutePath
            | LauncherNew
            | LauncherStart
            | LauncherExecute
            | ChildProcessId
            | ChildProcessWait
            | ChildProcessKill
            | EntropyNew
            | EntropyRandomBytes
            | SettingOf
            | SettingValue
            | CommandLineOf
            | CommandLineArguments
            | EnvironmentOf
            | EnvironmentGet => self.capability(b, receiver, args),
            ChannelBounded
            | SenderSend
            | SenderTrySend
            | SenderClose
            | ReceiverReceive
            | ReceiverReceiveWithin
            | ReceiverClosed
            | ReceiverClose => self.channel(e, b, receiver, args),
            ClockSleep => {
                self.m
                    .declare("ignite_rt_clock_sleep", "void", &["i64".to_string()]);
                let duration = self.scalar_arg(&args[0]);
                self.f
                    .ins(format!("call void @ignite_rt_clock_sleep(i64 {duration})"));
                Val::Unit
            }
            ClockNow => {
                self.m.declare("ignite_rt_clock_now", "i64", &[]);
                Val::Scalar(self.f.val("call i64 @ignite_rt_clock_now()"))
            }
            ProcessOpenFilesLimit => {
                self.m.declare("ignite_rt_open_files_limit", "i64", &[]);
                Val::Scalar(self.f.val("call i64 @ignite_rt_open_files_limit()"))
            }
            ProcessFlush => {
                self.m.declare("ignite_rt_flush", "void", &[]);
                self.f.ins("call void @ignite_rt_flush()");
                Val::Unit
            }
            ClockMonotonic => {
                self.m.declare("ignite_rt_clock_monotonic", "i64", &[]);
                Val::Scalar(self.f.val("call i64 @ignite_rt_clock_monotonic()"))
            }
            EndpointOf => {
                self.m.declare(
                    "ignite_rt_endpoint_of",
                    "i32",
                    &["ptr".to_string(), "ptr".to_string(), "ptr".to_string()],
                );
                let reference = self.read_arg(&args[0]);
                let endpoint = ignite_types::endpoint();
                self.rt_result("endpoint_of", &[format!("ptr {reference}")], endpoint)
            }
            BytesOf => {
                let aty = self.ty(&args[0]);
                if aty == bytes() {
                    let v = self.expr(&args[0], true);
                    return self.owned(&aty, v);
                }
                if let ExprKind::List(items) = &args[0].kind {
                    return self.list_literal(&Ty::Byte, items);
                }
                let v = self.expr(&args[0], true);
                let src = self.ptr(&v);
                self.ints_to_bytes(&src)
            }
            IntToText => {
                let i = self.scalar_arg(&args[0]);
                self.rt_out("int_to_text", &Ty::Text, &[format!("i64 {i}")])
            }
            IntToTextMethod => {
                let r = recv(self);
                let i = self.scalar(&r);
                self.rt_out("int_to_text", &Ty::Text, &[format!("i64 {i}")])
            }
            ByteToText => {
                let r = recv(self);
                let b = self.scalar(&r);
                let i = self.f.val(format!("zext i8 {b} to i64"));
                self.rt_out("int_to_text", &Ty::Text, &[format!("i64 {i}")])
            }
            BoolToText => {
                let r = recv(self);
                let b = self.scalar(&r);
                let yes = self.literal_ref(b"true");
                let no = self.literal_ref(b"false");
                let view = self.f.val(format!("select i1 {b}, ptr {yes}, ptr {no}"));
                let t = self.temp(&Ty::Text);
                self.copy(&Ty::Text, &t, &view);
                Val::Ptr(t, true)
            }
            TextToText => {
                let r = recv(self);
                self.owned(&Ty::Text, r)
            }
            Log => {
                let t = self.read_arg(&args[0]);
                self.f.ins(format!("call void @ignite_rt_log(ptr {t})"));
                Val::Unit
            }
            TcpListenerBind | TcpStreamConnect => {
                let address = self.read_arg(&args[0]);
                let port = self.scalar_arg(&args[1]);
                let (name, ok) = if b == TcpListenerBind {
                    ("tcp_bind", Ty::TcpListener)
                } else {
                    ("tcp_connect", Ty::TcpStream)
                };
                self.rt_result(name, &[format!("ptr {address}"), format!("i64 {port}")], ok)
            }
            TcpListenerAccept => {
                let r = recv(self);
                let l = self.scalar(&r);
                self.rt_result("tcp_accept", &[format!("i64 {l}")], Ty::TcpStream)
            }
            TcpStreamConnectWithin => {
                let ptr = "ptr".to_string();
                let i64 = "i64".to_string();
                self.m.declare(
                    "ignite_rt_tcp_connect_within",
                    "i32",
                    &[ptr.clone(), i64.clone(), i64, ptr.clone(), ptr],
                );
                let address = self.read_arg(&args[0]);
                let port = self.scalar_arg(&args[1]);
                let timeout = self.scalar_arg(&args[2]);
                self.rt_result(
                    "tcp_connect_within",
                    &[
                        format!("ptr {address}"),
                        format!("i64 {port}"),
                        format!("i64 {timeout}"),
                    ],
                    Ty::TcpStream,
                )
            }
            TcpStreamPeer => {
                self.m.declare(
                    "ignite_rt_tcp_peer",
                    "void",
                    &["ptr".to_string(), "i64".to_string()],
                );
                let r = recv(self);
                let s = self.scalar(&r);
                self.rt_out("tcp_peer", &ignite_types::endpoint(), &[format!("i64 {s}")])
            }
            TcpStreamTimedOut => {
                self.m
                    .declare("ignite_rt_tcp_timed_out", "i8", &["i64".to_string()]);
                let r = recv(self);
                let s = self.scalar(&r);
                let flag = self
                    .f
                    .val(format!("call i8 @ignite_rt_tcp_timed_out(i64 {s})"));
                Val::Scalar(self.f.val(format!("icmp ne i8 {flag}, 0")))
            }
            TcpListenerLocalPort => {
                let r = recv(self);
                let l = self.scalar(&r);
                Val::Scalar(
                    self.f
                        .val(format!("call i64 @ignite_rt_tcp_local_port(i64 {l})")),
                )
            }
            TcpStreamRead | PipeStreamRead | StreamRead => {
                let entry = self.stream_entry(b, receiver, "read");
                let r = recv(self);
                let s = self.scalar(&r);
                let max = self.scalar_arg(&args[0]);
                self.rt_result(
                    &entry,
                    &[format!("i64 {s}"), format!("i64 {max}")],
                    Ty::Input,
                )
            }
            TcpStreamWriteAll | PipeStreamWriteAll | StreamWriteAll => {
                let entry = self.stream_entry(b, receiver, "write_all");
                let r = recv(self);
                let s = self.scalar(&r);
                let data = self.read_arg(&args[0]);
                self.rt_result(
                    &entry,
                    &[format!("i64 {s}"), format!("ptr {data}")],
                    Ty::Unit,
                )
            }
            TcpStreamClose | PipeStreamClose | StreamClose => {
                let entry = self.stream_entry(b, receiver, "close");
                let r = recv(self);
                let s = self.scalar(&r);
                self.rt_result(&entry, &[format!("i64 {s}")], Ty::Unit)
            }
            StdinRead => {
                let max = self.scalar_arg(&args[0]);
                self.rt_result("stdin_read", &[format!("i64 {max}")], Ty::Input)
            }
            StdinIsTerminal => self.rt_result("stdin_is_terminal", &[], Ty::Bool),
            StdinSetRaw => {
                let on = self.scalar_arg(&args[0]);
                let byte = self.f.val(format!("zext i1 {on} to i8"));
                self.rt_result("stdin_set_raw", &[format!("i8 {byte}")], Ty::Unit)
            }
            StdoutWriteAll => {
                let data = self.read_arg(&args[0]);
                self.rt_result("stdout_write_all", &[format!("ptr {data}")], Ty::Unit)
            }
            StderrWriteAll => {
                let data = self.read_arg(&args[0]);
                self.rt_result("stderr_write_all", &[format!("ptr {data}")], Ty::Unit)
            }
            ProcessExit => {
                let code = self.scalar_arg(&args[0]);
                let c = self.f.val(format!("trunc i64 {code} to i32"));
                self.m.declare_raw(
                    "ignite_rt_process_exit",
                    "declare void @ignite_rt_process_exit(i32) noreturn",
                );
                self.f
                    .ins(format!("call void @ignite_rt_process_exit(i32 {c})"));
                self.f.term("unreachable");
                Val::Never
            }
            IoErrorNew => {
                let message = self.read_arg(&args[0]);
                self.rt_out("io_error_new", &Ty::IoError, &[format!("ptr {message}")])
            }
            ListMap => self.list_map(receiver.expect("receiver"), &args[0]),
            IntParse => {
                let text = match &args[0].kind {
                    ExprKind::Method { receiver, .. }
                        if matches!(
                            self.cx.types.callee(&args[0]),
                            Some(Callee::Builtin(BytesToText))
                        ) =>
                    {
                        let bytes = self.slice_view(receiver);
                        self.ptr(&bytes)
                    }
                    _ => self.read_arg(&args[0]),
                };
                self.option_from("int_parse", &Ty::Int, |payload| {
                    format!("ptr {text}, ptr {payload}")
                })
            }
            ByteToInt => {
                let r = recv(self);
                let b = self.scalar(&r);
                Val::Scalar(self.f.val(format!("zext i8 {b} to i64")))
            }
            IntToByte => {
                let r = recv(self);
                let i = self.scalar(&r);
                let ty = Ty::Option(Box::new(Ty::Byte));
                let layout = self.env.enum_layout(&ty);
                let result = self.temp(&ty);
                let fits = self.f.val(format!("icmp ult i64 {i}, 256"));
                let tag = self.f.val(format!("select i1 {fits}, i32 0, i32 1"));
                self.f
                    .ins(format!("store i32 {tag}, ptr {result}, align 4"));
                let byte = self.f.val(format!("trunc i64 {i} to i8"));
                let payload = self.gep(&result, layout.variants[0].1[0].1);
                self.f
                    .ins(format!("store i8 {byte}, ptr {payload}, align 1"));
                Val::Ptr(result, true)
            }
            IntWrappingAdd | IntWrappingSub | IntWrappingMul => {
                let r = recv(self);
                let x = self.scalar(&r);
                let y = self.scalar_arg(&args[0]);
                let instr = match b {
                    IntWrappingAdd => "add",
                    IntWrappingSub => "sub",
                    _ => "mul",
                };
                Val::Scalar(self.f.val(format!("{instr} i64 {x}, {y}")))
            }
            IntCheckedAdd | IntCheckedSub | IntCheckedMul => {
                let r = recv(self);
                let x = self.scalar(&r);
                let y = self.scalar_arg(&args[0]);
                let name = match b {
                    IntCheckedAdd => "sadd",
                    IntCheckedSub => "ssub",
                    _ => "smul",
                };
                let pair = self.f.val(format!(
                    "call {{ i64, i1 }} @llvm.{name}.with.overflow.i64(i64 {x}, i64 {y})"
                ));
                let over = self.f.val(format!("extractvalue {{ i64, i1 }} {pair}, 1"));
                let value = self.f.val(format!("extractvalue {{ i64, i1 }} {pair}, 0"));
                let ty = Ty::Option(Box::new(Ty::Int));
                let layout = self.env.enum_layout(&ty);
                let result = self.temp(&ty);
                let tag = self.f.val(format!("select i1 {over}, i32 1, i32 0"));
                self.f
                    .ins(format!("store i32 {tag}, ptr {result}, align 4"));
                let payload = self.gep(&result, layout.variants[0].1[0].1);
                self.f
                    .ins(format!("store i64 {value}, ptr {payload}, align 8"));
                Val::Ptr(result, true)
            }
            TcpStreamSetIdleTimeout | PipeStreamSetIdleTimeout | StreamSetIdleTimeout => {
                let entry = self.stream_entry(b, receiver, "set_idle_timeout");
                let r = recv(self);
                let s = self.scalar(&r);
                let ms = self.scalar_arg(&args[0]);
                self.rt_result(&entry, &[format!("i64 {s}"), format!("i64 {ms}")], Ty::Unit)
            }
            TcpStreamWriteUrgent | PipeStreamWriteUrgent | StreamWriteUrgent => {
                let entry = self.stream_entry(b, receiver, "write_urgent");
                let ptr = "ptr".to_string();
                self.m.declare(
                    &format!("ignite_rt_{entry}"),
                    "i32",
                    &["i64".to_string(), ptr.clone(), ptr],
                );
                let r = recv(self);
                let s = self.scalar(&r);
                let data = self.read_arg(&args[0]);
                self.rt_result(
                    &entry,
                    &[format!("i64 {s}"), format!("ptr {data}")],
                    Ty::Unit,
                )
            }
            TcpStreamUrgentPending | PipeStreamUrgentPending | StreamUrgentPending => {
                let entry = self.stream_entry(b, receiver, "urgent_pending");
                let ptr = "ptr".to_string();
                self.m.declare(
                    &format!("ignite_rt_{entry}"),
                    "i32",
                    &["i64".to_string(), ptr.clone(), ptr],
                );
                let r = recv(self);
                let s = self.scalar(&r);
                self.rt_result(&entry, &[format!("i64 {s}")], Ty::Bool)
            }
            PipeStreamReceivedBatches => {
                self.m.declare_pipe("ignite_rt_pipe_received_batches");
                let r = recv(self);
                let s = self.scalar(&r);
                Val::Scalar(self.f.val(format!(
                    "call i64 @ignite_rt_pipe_received_batches(i64 {s})"
                )))
            }
            PipePair => {
                self.m.declare_pipe("ignite_rt_pipe_pair");
                let ends = [Ty::PipeStream, Ty::PipeStream];
                let (offsets, _) = self.env.tuple_layout(&ends);
                let pair = self.temp(&Ty::Tuple(ends.to_vec()));
                let first = self.gep(&pair, offsets[0]);
                let second = self.gep(&pair, offsets[1]);
                self.f.ins(format!(
                    "call void @ignite_rt_pipe_pair(ptr {first}, ptr {second})"
                ));
                Val::Ptr(pair, true)
            }
            StoreNextId => {
                let r = recv(self);
                let s = self.scalar(&r);
                Val::Scalar(
                    self.f
                        .val(format!("call i64 @ignite_rt_store_next_id(ptr {s})")),
                )
            }
            StoreGet | StoreRemove => {
                let element = self.store_element(receiver);
                let r = recv(self);
                let s = self.scalar(&r);
                let id = self.scalar_arg(&args[0]);
                let name = if b == StoreGet {
                    "store_get"
                } else {
                    "store_remove"
                };
                self.option_from(name, &element, |payload| {
                    format!("ptr {s}, i64 {id}, ptr {payload}")
                })
            }
            StorePut => {
                let element = self.store_element(receiver);
                let r = recv(self);
                let s = self.scalar(&r);
                let id = self.scalar_arg(&args[0]);
                let v = self.expr(&args[1], true);
                if v == Val::Never {
                    return Val::Never;
                }
                let have = self.ty(&args[1]);
                let v = self.coerce(&element, &have, v);
                let value = match v {
                    Val::Ptr(p, _) => p,
                    other => {
                        let t = self.temp(&element);
                        self.store_value(&element, &t, other);
                        t
                    }
                };
                self.f.ins(format!(
                    "call void @ignite_rt_store_put(ptr {s}, i64 {id}, ptr {value})"
                ));
                Val::Unit
            }
            StoreValues => {
                let element = self.store_element(receiver);
                let r = recv(self);
                let s = self.scalar(&r);
                let out = self.temp(&Ty::List(Box::new(element)));
                self.f.ins(format!(
                    "call void @ignite_rt_store_values(ptr {s}, ptr {out})"
                ));
                Val::Ptr(out, true)
            }
            StoreUpdate => {
                let element = self.store_element(receiver);
                let r = recv(self);
                let s = self.scalar(&r);
                let id = self.scalar_arg(&args[0]);
                let fv = self.expr(&args[1], true);
                let f = self.ptr(&fv);
                let thunk = self.m.glue_of(Glue::Update, &element);
                self.option_from("store_update", &element, |payload| {
                    format!("ptr {s}, i64 {id}, ptr {f}, ptr {thunk}, ptr {payload}")
                })
            }
            StoreUpsert => {
                let element = self.store_element(receiver);
                let r = recv(self);
                let s = self.scalar(&r);
                let id = self.scalar_arg(&args[0]);
                let fv = self.expr(&args[1], true);
                let f = self.ptr(&fv);
                let thunk = self.m.glue_of(Glue::Upsert, &element);
                self.m.declare(
                    "ignite_rt_store_upsert",
                    "void",
                    &[
                        "ptr".into(),
                        "i64".into(),
                        "ptr".into(),
                        "ptr".into(),
                        "ptr".into(),
                    ],
                );
                let out = self.temp(&element);
                self.f.ins(format!(
                    "call void @ignite_rt_store_upsert(ptr {s}, i64 {id}, ptr {f}, ptr {thunk}, ptr {out})"
                ));
                if is_aggregate(&element) {
                    Val::Ptr(out, true)
                } else if scalar_type(&element).is_some() {
                    Val::Scalar(self.load_scalar(&element, &out))
                } else {
                    Val::Unit
                }
            }
        }
    }

    /// The element type of the `Store` receiver of a builtin.
    fn store_element(&self, receiver: Option<&Expr>) -> Ty {
        match self.ty(receiver.expect("a store method has a receiver")) {
            Ty::Store(t) => *t,
            other => panic!("store method on {other:?}"),
        }
    }

    /// `Option<ty>` from `i8 ignite_rt_<name>(args)`, where `args` gets the
    /// pointer to the `Some` payload, which the call fills when it returns
    /// 1.
    fn option_from(&mut self, name: &str, ty: &Ty, args: impl FnOnce(&str) -> String) -> Val {
        let option = Ty::Option(Box::new(ty.clone()));
        let layout = self.env.enum_layout(&option);
        let result = self.temp(&option);
        let payload = self.gep(&result, layout.variants[0].1[0].1);
        let found = self
            .f
            .val(format!("call i8 @ignite_rt_{name}({})", args(&payload)));
        let some = self.f.val(format!("icmp ne i8 {found}, 0"));
        let tag = self.f.val(format!("select i1 {some}, i32 0, i32 1"));
        self.f
            .ins(format!("store i32 {tag}, ptr {result}, align 4"));
        Val::Ptr(result, true)
    }

    /// `list.map(f)`: a new list of `f` applied to each element, in order;
    /// an aggregate element goes as its own pointer when `f` only reads it
    /// ([`Lower::known_borrows`]), else as a copy.
    fn list_map(&mut self, receiver: &Expr, f: &Expr) -> Val {
        let element = match self.ty(receiver) {
            Ty::List(t) => *t,
            other => panic!("map on {other:?}"),
        };
        let out = match self.ty(f) {
            Ty::Fn(_, ret) | Ty::EffectFn(_, ret, _) => *ret,
            other => panic!("map with a {other:?}"),
        };
        let r = self.expr(receiver, true);
        if r == Val::Never {
            return Val::Never;
        }
        let list = self.ptr(&r);
        let fv = self.expr(f, true);
        let borrow = self.known_borrows(f).first().copied().unwrap_or(false);
        let fp = self.ptr(&fv);
        let parts = self.fn_parts(&fp);
        let len = self.list_len(&list);
        let in_size = self.layout(&element).size;
        let out_layout = self.layout(&out);
        let bytes = self
            .f
            .val(format!("mul i64 {len}, {}", out_layout.size.max(1)));
        let buf = self.f.val(format!(
            "call ptr @ignite.alloc(i64 {bytes}, i64 {})",
            out_layout.align.max(1)
        ));
        let data = self.f.val(format!("load ptr, ptr {list}, align 8"));
        let counter = self.f.alloca(Layout::scalar(8));
        self.f.ins(format!("store i64 0, ptr {counter}, align 8"));
        let (head, body, exit) = (self.f.label(), self.f.label(), self.f.label());
        self.f.place(&head);
        let i = self.f.val(format!("load i64, ptr {counter}, align 8"));
        let more = self.f.val(format!("icmp slt i64 {i}, {len}"));
        self.f.cond_br(&more, &body, &exit);
        self.f.place(&body);
        let at = self.f.val(format!("mul i64 {i}, {in_size}"));
        let src = self
            .f
            .val(format!("getelementptr inbounds i8, ptr {data}, i64 {at}"));
        let operand = if is_aggregate(&element) && borrow {
            Some(format!("ptr {src}"))
        } else {
            let item = self.at(&element, src, false);
            self.arg(&element, item)
        };
        let result = self.call_value(&parts, &out, vec![operand]);
        let to = self.f.val(format!("mul i64 {i}, {}", out_layout.size));
        let dst = self
            .f
            .val(format!("getelementptr inbounds i8, ptr {buf}, i64 {to}"));
        self.store_value(&out, &dst, result);
        let next = self.f.val(format!("add i64 {i}, 1"));
        self.f
            .ins(format!("store i64 {next}, ptr {counter}, align 8"));
        self.f.br(&head);
        self.f.place(&exit);
        let t = self.temp(&Ty::List(Box::new(out)));
        self.f.ins(format!("store ptr {buf}, ptr {t}, align 8"));
        let lenp = self.gep(&t, 8);
        self.f.ins(format!("store i64 {len}, ptr {lenp}, align 8"));
        let capp = self.gep(&t, 16);
        self.f.ins(format!("store i64 {len}, ptr {capp}, align 8"));
        Val::Ptr(t, true)
    }

    fn strip_prefix(&mut self, receiver: &Expr, prefix: &Expr) -> Val {
        let ty = Ty::Option(Box::new(Ty::Text));
        let layout = self.env.enum_layout(&ty);
        let r = self.expr(receiver, true);
        let p = self.ptr(&r);
        let pre = self.read_arg(prefix);
        let result = self.temp(&ty);
        let starts = self.f.val(format!(
            "call i8 @ignite_rt_text_starts_with(ptr {p}, ptr {pre})"
        ));
        let yes = self.f.val(format!("icmp ne i8 {starts}, 0"));
        let (some, none, end) = (self.f.label(), self.f.label(), self.f.label());
        self.f.cond_br(&yes, &some, &none);
        self.f.place(&some);
        let plen = self.list_len(&pre);
        let len = self.list_len(&p);
        let data = self.f.val(format!("load ptr, ptr {p}, align 8"));
        let start = self
            .f
            .val(format!("getelementptr inbounds i8, ptr {data}, i64 {plen}"));
        let rest = self.f.val(format!("sub i64 {len}, {plen}"));
        let view = self.temp(&Ty::Text);
        self.f
            .ins(format!("store ptr {start}, ptr {view}, align 8"));
        let lenp = self.gep(&view, 8);
        self.f.ins(format!("store i64 {rest}, ptr {lenp}, align 8"));
        let capp = self.gep(&view, 16);
        self.f.ins(format!("store i64 {rest}, ptr {capp}, align 8"));
        self.f.ins(format!("store i32 0, ptr {result}, align 4"));
        let dst = self.gep(&result, layout.variants[0].1[0].1);
        self.copy(&Ty::Text, &dst, &view);
        self.f.br(&end);
        self.f.place(&none);
        self.f.ins(format!("store i32 1, ptr {result}, align 4"));
        self.f.br(&end);
        self.f.place(&end);
        Val::Ptr(result, true)
    }

    /// `Bytes.of(list)` for a `List<Int>` at `src`: each element truncated.
    fn ints_to_bytes(&mut self, src: &str) -> Val {
        let len = self.list_len(src);
        let data = self.f.val(format!("load ptr, ptr {src}, align 8"));
        let size = self.f.val(format!("add i64 {len}, 1"));
        let buf = self
            .f
            .val(format!("call ptr @ignite.alloc(i64 {size}, i64 1)"));
        let counter = self.f.alloca(Layout::scalar(8));
        self.f.ins(format!("store i64 0, ptr {counter}, align 8"));
        let (head, body, exit) = (self.f.label(), self.f.label(), self.f.label());
        self.f.place(&head);
        let i = self.f.val(format!("load i64, ptr {counter}, align 8"));
        let more = self.f.val(format!("icmp slt i64 {i}, {len}"));
        self.f.cond_br(&more, &body, &exit);
        self.f.place(&body);
        let s = self
            .f
            .val(format!("getelementptr inbounds i64, ptr {data}, i64 {i}"));
        let x = self.f.val(format!("load i64, ptr {s}, align 8"));
        let byte = self.f.val(format!("trunc i64 {x} to i8"));
        let d = self
            .f
            .val(format!("getelementptr inbounds i8, ptr {buf}, i64 {i}"));
        self.f.ins(format!("store i8 {byte}, ptr {d}, align 1"));
        let next = self.f.val(format!("add i64 {i}, 1"));
        self.f
            .ins(format!("store i64 {next}, ptr {counter}, align 8"));
        self.f.br(&head);
        self.f.place(&exit);
        let t = self.temp(&Ty::Text);
        self.f.ins(format!("store ptr {buf}, ptr {t}, align 8"));
        let lenp = self.gep(&t, 8);
        self.f.ins(format!("store i64 {len}, ptr {lenp}, align 8"));
        let capp = self.gep(&t, 16);
        self.f.ins(format!("store i64 {len}, ptr {capp}, align 8"));
        Val::Ptr(t, true)
    }

    // ---- tasks -----------------------------------------------------------

    /// `spawn f(args)`: the arguments are evaluated into an environment in
    /// this frame (a borrowed one as its place's pointer); a per-site thunk
    /// copies them into the new task's arena, calls `spawn_ready` and then `f`.
    fn spawn(&mut self, call: &Expr) {
        let callee = self.cx.types.callee(call).cloned();
        let args: Vec<&Expr> = match &call.kind {
            ExprKind::Method { receiver, args, .. } if matches!(callee, Some(Callee::Fn(_))) => {
                std::iter::once(&**receiver).chain(args).collect()
            }
            ExprKind::Call { args, .. } | ExprKind::Method { args, .. } => args.iter().collect(),
            _ => Vec::new(),
        };
        let (symbol, sig) = callee
            .as_ref()
            .and_then(|c| self.sig_of(c, call))
            .unwrap_or_else(|| panic!("spawn of an unresolved call at {:?}", call.span));
        // A stream binding a block owns ([`crate::own`]) whose last use this
        // spawn is (`own::handovers`: in the binding's block or in a branch
        // nested in it) is handed over: the task becomes an owner like for
        // any other argument, and the block releases its own owner right
        // after the spawn, so the task holds the last one. The release
        // leaves a tombstone in the task's owner list (`ignite-rt`'s
        // `release`), so the mark a block between the binding's and this
        // one took before the spawn still releases that block's own
        // streams. An unowned binding that `region::moved_to_task` names
        // moves its handle to the task as before.
        let moved = std::mem::take(&mut self.moved_streams);
        let name_of = |arg: &Expr| match &arg.kind {
            ExprKind::Name(n) => Some(n.name.clone()),
            ExprKind::NamedArg { value, .. } => match &value.kind {
                ExprKind::Name(n) => Some(n.name.clone()),
                _ => None,
            },
            _ => None,
        };
        let key = expr_key(call);
        let mut owned: Vec<String> = Vec::new();
        for handover in self.handovers.iter().filter(|h| h.call == key) {
            if self.lookup(&handover.name).is_some_and(|l| l.owned)
                && !owned.contains(&handover.name)
            {
                owned.push(handover.name.clone());
            }
        }
        self.handovers.retain(|h| h.call != key);
        let moves =
            |arg: &Expr| name_of(arg).is_some_and(|n| moved.contains(&n) && !owned.contains(&n));
        let hands_over = |arg: &Expr| name_of(arg).is_some_and(|n| owned.contains(&n));
        let mut handed: Vec<(String, Val)> = Vec::new();
        let mut values = Vec::with_capacity(sig.params.len());
        if let Some(perm) = self.cx.types.named_args(call).map(<[usize]>::to_vec) {
            let offset = args.len() - perm.len();
            let evaluated: Vec<Val> = args.iter().map(|arg| self.expr(arg, true)).collect();
            for (i, ty) in sig.params.iter().enumerate() {
                let src = if i < offset {
                    i
                } else {
                    offset + perm[i - offset]
                };
                if hands_over(args[src]) {
                    handed.push((
                        name_of(args[src]).unwrap_or_default(),
                        evaluated[src].clone(),
                    ));
                }
                values.push((ty.clone(), evaluated[src].clone(), moves(args[src])));
            }
        } else {
            for (arg, ty) in args.iter().zip(&sig.params) {
                let v = self.expr(arg, true);
                if hands_over(arg) {
                    handed.push((name_of(arg).unwrap_or_default(), v.clone()));
                }
                values.push((ty.clone(), v, moves(arg)));
            }
        }
        let thunk = format!("spawn.{symbol}.{}", self.m.next_id());
        self.spawn_with(&thunk, &symbol, &sig, values, "task ended with error: ");
        let mut released: Vec<String> = Vec::new();
        for (name, handle) in handed {
            if released.contains(&name) {
                continue;
            }
            released.push(name);
            let handle = self.scalar(&handle);
            self.f
                .ins(format!("call void @ignite_rt_tcp_release(i64 {handle})"));
        }
    }

    /// Store `values` into an environment, define the thunk `thunk` that
    /// runs `symbol` with copies of them, and start it (`docs/llvm.md`
    /// section 4). An owned value is moved into the environment and a
    /// borrowed aggregate goes in as its place's pointer, not copied: the
    /// parent waits in `ignite_rt_spawn` until the thunk has deep-copied
    /// every value into the task's own arena (`spawn_ready`), and nothing
    /// changes the place meanwhile. The task is an owner of every stream argument
    /// ([`is_stream`]; a handle-table entry counts its owners): the thunk
    /// registers it with `tcp_own`, so a failure releases it, and releases
    /// it when `symbol` returns; the stream closes when its last owner
    /// releases it. A stream not flagged as moved stays the parent's too:
    /// the parent adds the task's owner with `tcp_retain` before the start
    /// (a client role keeps reading the stream `forward_input` writes to;
    /// a task that hands its parameter on to a second task and returns
    /// leaves it open). A stream flagged as moved passes the parent's
    /// owner to the task (`docs/slice.md` 18.11 for pipe ends). When no
    /// thread can start, the task's owners are released at once instead.
    pub fn spawn_with(
        &mut self,
        thunk: &str,
        symbol: &str,
        sig: &FnSig,
        values: Vec<(Ty, Val, bool)>,
        prefix: &str,
    ) {
        let by_place: Vec<bool> = values
            .iter()
            .map(|(ty, v, _)| is_aggregate(ty) && matches!(v, Val::Ptr(_, false)))
            .collect();
        let layouts: Vec<Layout> = values
            .iter()
            .zip(&by_place)
            .map(|((ty, _, _), &place)| {
                if place {
                    Layout::scalar(8)
                } else {
                    self.layout(ty)
                }
            })
            .collect();
        let (offsets, env_layout) = c_struct(&layouts);
        let env = self.f.alloca(env_layout);
        for (((ty, v, _), offset), &place) in values.iter().zip(&offsets).zip(&by_place) {
            let dst = self.gep(&env, *offset);
            if place {
                let p = self.ptr(v);
                self.f.ins(format!("store ptr {p}, ptr {dst}, align 8"));
            } else {
                self.store_value(ty, &dst, v.clone());
            }
        }
        let callee = self.function(symbol, &sig.params, &sig.ret);
        let mut t = Lower::new(
            self.m,
            self.cx,
            Func::new(format!("define internal void {}(ptr %env)", global(thunk))),
            Ty::Unit,
        );
        // Failure containment (`docs/llvm.md` section 4). The task body runs
        // behind a `setjmp` landing site no Rust frame can see: a failure
        // inside it reaches `ignite_rt_*` in the runtime, which releases this
        // task's owned handles and `longjmp`s back here. The return lands in
        // `recover`, which ends the task without unwinding, so no landing
        // pads are emitted and the arena and handles never leak.
        t.m.declare_raw("_setjmp", "declare i32 @_setjmp(ptr) returns_twice");
        t.m.declare("ignite_rt_guard_push", "void", &["ptr".into()]);
        t.m.declare("ignite_rt_guard_pop", "void", &[]);
        let jb = t.f.alloca(Layout {
            size: 256,
            align: 16,
        });
        let landed = t.f.val(format!("call i32 @_setjmp(ptr {jb})"));
        let fresh = t.f.val(format!("icmp eq i32 {landed}, 0"));
        let (run, recover) = (t.f.label(), t.f.label());
        t.f.cond_br(&fresh, &run, &recover);
        t.f.place(&recover);
        t.f.ins("call void @ignite_rt_task_end()");
        t.f.term("ret void");
        t.f.place(&run);
        t.f.ins(format!("call void @ignite_rt_guard_push(ptr {jb})"));
        let mut operands = Vec::new();
        let mut streams = Vec::new();
        for (((ty, _, _), offset), &place) in values.iter().zip(&offsets).zip(&by_place) {
            let slot = t.gep("%env", *offset);
            let src = if place {
                t.f.val(format!("load ptr, ptr {slot}, align 8"))
            } else {
                slot
            };
            if is_aggregate(ty) {
                let own = t.temp(ty);
                t.copy(ty, &own, &src);
                operands.push(Some(format!("ptr {own}")));
            } else if let Some(llty) = scalar_type(ty) {
                let x = t.load_scalar(ty, &src);
                if is_stream(ty) {
                    streams.push(x.clone());
                }
                operands.push(Some(format!("{llty} {x}")));
            }
        }
        // The task owns the streams moved to it for its length: a failure
        // closes them through `release_task_handles`, the normal path
        // through the explicit releases below (which also drop them from
        // the owned set).
        if !streams.is_empty() {
            t.m.declare("ignite_rt_tcp_own", "void", &["i64".into()]);
        }
        for stream in &streams {
            t.f.ins(format!("call void @ignite_rt_tcp_own(i64 {stream})"));
        }
        t.f.ins("call void @ignite_rt_spawn_ready()");
        let result = t.call(&callee, &sig.ret, operands);
        if let (Ty::Result(_, err), Val::Ptr(p, _)) = (&sig.ret, &result)
            && **err == Ty::IoError
        {
            t.log_error(&sig.ret, p, prefix);
        }
        t.f.ins("call void @ignite_rt_guard_pop()");
        for stream in &streams {
            t.f.ins(format!("call void @ignite_rt_tcp_release(i64 {stream})"));
        }
        t.f.ins("call void @ignite_rt_task_end()");
        let text = t.f.finish("ret void");
        self.m.define(thunk, text);
        let parents: Vec<String> = values
            .iter()
            .filter_map(|(ty, v, _)| match v {
                Val::Scalar(stream) if is_stream(ty) => Some(stream.clone()),
                _ => None,
            })
            .collect();
        for (ty, v, moved) in &values {
            if let Val::Scalar(stream) = v
                && is_stream(ty)
                && !moved
            {
                self.m
                    .declare("ignite_rt_tcp_retain", "void", &["i64".into()]);
                self.f
                    .ins(format!("call void @ignite_rt_tcp_retain(i64 {stream})"));
            }
        }
        let started = self.f.val(format!(
            "call i8 @ignite_rt_spawn(ptr {}, ptr {env})",
            global(thunk)
        ));
        if !parents.is_empty() {
            let failed = self.f.val(format!("icmp eq i8 {started}, 0"));
            let (close, done) = (self.f.label(), self.f.label());
            self.f.cond_br(&failed, &close, &done);
            self.f.place(&close);
            for stream in &parents {
                self.f
                    .ins(format!("call void @ignite_rt_tcp_release(i64 {stream})"));
            }
            self.f.br(&done);
            self.f.place(&done);
        }
    }

    /// When the `Result<_, IoError>` at `result` is `Err`, log
    /// `<prefix><message>`.
    pub fn log_error(&mut self, ty: &Ty, result: &str, prefix: &str) {
        let layout = self.env.enum_layout(ty);
        let tag = self.f.val(format!("load i32, ptr {result}, align 4"));
        let failed = self.f.val(format!("icmp eq i32 {tag}, 1"));
        let (bad, done) = (self.f.label(), self.f.label());
        self.f.cond_br(&failed, &bad, &done);
        self.f.place(&bad);
        let error = self.gep(result, layout.variants[1].1[0].1);
        let pieces = [
            self.literal_piece(prefix.as_bytes()),
            self.text_piece(&error),
        ];
        let line = self.temp(&Ty::Text);
        self.concat_into(&line, &pieces, false);
        self.f.ins(format!("call void @ignite_rt_log(ptr {line})"));
        self.f.br(&done);
        self.f.place(&done);
    }
}

/// The type of `base.fields...`.
fn ty_of_place(env: &TypeEnv, base: &Ty, fields: &[Ident]) -> Ty {
    let mut ty = base.clone();
    for field in fields {
        ty = env
            .field(&ty, &field.name)
            .map(|(_, t)| t)
            .unwrap_or(Ty::Unit);
    }
    ty
}

/// The symbol of an instance of the generic declaration `base`:
/// `"<module>.<name><Int,Text>"` (`docs/llvm.md` section 3).
pub fn instance_symbol(base: &str, args: &[Ty]) -> String {
    let args: Vec<String> = args.iter().map(crate::layout::mangle).collect();
    format!("{base}<{}>", args.join(","))
}

/// The linkage of a definition and its trailing comdat clause: an instance
/// of generic code (`instance`) is `linkonce_odr` in a comdat of its own
/// name, so the linker keeps one copy per executable however many IR
/// modules define it (`docs/slice.md` section 13.1, row 16); anything else
/// is external.
fn instance_linkage(
    m: &mut Module<'_>,
    symbol: &str,
    instance: bool,
) -> (&'static str, &'static str) {
    if instance {
        m.global_definition(format!(
            "${} = comdat any",
            global(symbol).trim_start_matches('@')
        ));
        ("linkonce_odr ", " comdat")
    } else {
        ("", "")
    }
}

/// Whether `ty` is a stream handle that a task owns when it is moved to
/// it: a `TcpStream`, a `PipeStream` end or a channel end (`ignite_rt_tcp_own` and
/// `ignite_rt_tcp_release` take any handle, `docs/slice.md` 18.9).
fn is_stream(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::TcpStream | Ty::PipeStream | Ty::Sender(_) | Ty::Receiver(_)
    )
}

/// `sig` with `params` replaced by `args`.
fn substitute_sig(sig: &FnSig, params: &[String], args: &[Ty]) -> FnSig {
    FnSig {
        params: sig
            .params
            .iter()
            .map(|t| repr(&t.substitute(params, args)))
            .collect(),
        ret: repr(&sig.ret.substitute(params, args)),
        effectful: sig.effectful,
    }
}

/// `sig` as the target lowers it: references as `Text`
/// ([`crate::layout::repr`]).
pub fn repr_sig(sig: &FnSig) -> FnSig {
    FnSig {
        params: sig.params.iter().map(repr).collect(),
        ret: repr(&sig.ret),
        effectful: sig.effectful,
    }
}

/// Define `instance` in `m`, lowering its generic declaration from the
/// declaring module's source with the instance's type arguments.
pub fn define_instance(m: &mut Module<'_>, instance: &Instance) {
    let (Generic::Fn(q) | Generic::Role(q, _) | Generic::Default(q, _)) = &instance.generic;
    let source = m.source(&q.module).unwrap_or_else(|| {
        panic!(
            "instance {} needs the source of module {}",
            instance.symbol, q.module
        )
    });
    let env = m.env;
    let cx = Ctx::new(source.path, source.program, source.symbols, source.types);
    let args = instance.args.clone();
    match &instance.generic {
        Generic::Fn(q) => {
            let decl = source
                .program
                .decls
                .iter()
                .find_map(|d| match d {
                    Decl::Fn(f) if f.qualified_name() == q.name => Some(f),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no fn {}", symbol(q)));
            let params = env.generics(q).to_vec();
            let sig = env
                .fn_sig(q)
                .unwrap_or_else(|| panic!("no signature for {}", symbol(q)));
            let sig = substitute_sig(sig, &params, &args);
            define_body(m, &cx, instance, decl, &sig, (params, args));
        }
        Generic::Role(q, role) => {
            let decl = source
                .program
                .decls
                .iter()
                .find_map(|d| match d {
                    Decl::Protocol(p) if p.name.name == q.name => {
                        p.fns.iter().find(|f| f.name.name == *role)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no role {}.{role}", symbol(q)));
            let params = env.role_generics(q, role).to_vec();
            let sig = env
                .role_sig(q, role)
                .unwrap_or_else(|| panic!("no signature for role {}.{role}", symbol(q)));
            let sig = substitute_sig(sig, &params, &args);
            define_body(m, &cx, instance, decl, &sig, (params, args));
        }
        Generic::Default(q, field) => {
            let value = source
                .program
                .decls
                .iter()
                .find_map(|d| match d {
                    Decl::Record(r) if r.name.name == q.name => r
                        .fields
                        .iter()
                        .find(|f| f.name.name == *field)
                        .and_then(|f| f.default.as_ref()),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no default of {}.{field}", symbol(q)));
            let ty = Ty::GenericRecord(q.clone(), args.clone());
            let field_ty = env
                .fields_of(&ty)
                .into_iter()
                .find(|(n, _)| n == field)
                .map(|(_, t)| t)
                .unwrap_or_else(|| panic!("no field {}.{field}", symbol(q)));
            let params = env.generics(q).to_vec();
            Lower::define_const_in(
                m,
                &cx,
                &instance.symbol,
                &field_ty,
                value,
                (params, args),
                true,
            );
        }
    }
}

fn define_body(
    m: &mut Module<'_>,
    cx: &Ctx<'_>,
    instance: &Instance,
    decl: &ignite_syntax::ast::FnDecl,
    sig: &FnSig,
    subst: (Vec<String>, Vec<Ty>),
) {
    let params: Vec<(Ident, Ty)> = decl
        .params
        .iter()
        .zip(&sig.params)
        .map(|(p, ty)| (p.name.clone(), ty.clone()))
        .collect();
    Lower::define_fn_in(
        m,
        cx,
        &instance.symbol,
        &params,
        &sig.ret,
        &decl.body,
        subst,
        true,
    );
}
