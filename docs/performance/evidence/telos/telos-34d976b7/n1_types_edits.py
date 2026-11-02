EDITS = [
("""use telos_syntax::ast::{
    AliasDecl, Arm, BinaryOp, Block, ComponentDecl, Decl, Expr, ExprKind, FieldInit, FnDecl, Ident,
    Pattern, PatternKind, Place, Program, Span, Stmt, StmtKind, SystemDecl, TestDecl, Type,
    UnaryOp,
};""",
"""use telos_syntax::ast::{
    AliasDecl, Arm, BinaryOp, Block, Closure, ComponentDecl, Decl, Expr, ExprKind, FieldInit,
    FnDecl, Ident, Pattern, PatternKind, Place, Program, Span, Stmt, StmtKind, SystemDecl,
    TestDecl, Type, UnaryOp,
};"""),
("""    Alias, AssertCompare, Builtin, Callee, ClosureInfo,""",
"""    Alias, AssertCompare, Builtin, Callee, Capture, ClosureInfo,"""),
# fields
("""    closures: HashMap<usize, ClosureInfo>,
    ranges: HashMap<usize, (i64, i64)>,
}

impl<'a> Typer<'a> {""",
"""    closures: HashMap<usize, ClosureInfo>,
    ranges: HashMap<usize, (i64, i64)>,
    /// The span of the name of each binding, by scope index and name.
    binding_spans: HashMap<(usize, String), Span>,
    /// One entry per closure being walked, innermost last: the number of
    /// scopes open outside it and the captures found so far.
    closure_frames: Vec<(usize, Vec<Capture>)>,
}

impl<'a> Typer<'a> {"""),
("""            closures: HashMap::new(),
            ranges: HashMap::new(),
""",
"""            closures: HashMap::new(),
            ranges: HashMap::new(),
            binding_spans: HashMap::new(),
            closure_frames: Vec::new(),
"""),
# bind / lookup
("""    fn bind(&mut self, name: &Ident, ty: Option<Ty>) {
        if let Some(ty) = &ty {
            self.bindings.insert(name.span, ty.clone());
        }
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.name.clone(), ty);
        }
    }""",
"""    fn bind(&mut self, name: &Ident, ty: Option<Ty>) {
        if let Some(ty) = &ty {
            self.bindings.insert(name.span, ty.clone());
        }
        let depth = self.scopes.len().saturating_sub(1);
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.name.clone(), ty);
            self.binding_spans
                .insert((depth, name.name.clone()), name.span);
        }
    }

    /// [`Typer::lookup`] for a use of a local as a value: a binding of a
    /// scope outside a closure being walked is recorded as its capture.
    fn use_local(&mut self, name: &str) -> Option<Option<Ty>> {
        let index = self
            .scopes
            .iter()
            .rposition(|scope| scope.contains_key(name))?;
        let ty = self.scopes[index].get(name).cloned()?;
        let binding = self
            .binding_spans
            .get(&(index, name.to_string()))
            .copied()
            .unwrap_or_default();
        for (outside, captures) in &mut self.closure_frames {
            if index < *outside && !captures.iter().any(|c| c.name == name) {
                captures.push(Capture {
                    name: name.to_string(),
                    binding,
                    ty: ty.clone().unwrap_or(Ty::Unit),
                    effects: Vec::new(),
                });
            }
        }
        Some(ty)
    }"""),
("""    fn name(&mut self, expr: &Expr, name: &Ident, expected: Option<&Ty>) -> Option<Ty> {
        if let Some(local) = self.lookup(&name.name) {
            return local;
        }""",
"""    fn name(&mut self, expr: &Expr, name: &Ident, expected: Option<&Ty>) -> Option<Ty> {
        if let Some(local) = self.use_local(&name.name) {
            return local;
        }"""),
("""            ExprKind::Name(name) => {
                if let Some(local) = self.lookup(&name.name) {
                    let Some(ty) = self.record(callee, local) else {""",
"""            ExprKind::Name(name) => {
                if let Some(local) = self.use_local(&name.name) {
                    let Some(ty) = self.record(callee, local) else {"""),
# expr_kind arms
("""            ExprKind::Block(block) => self.block(block, mode),
        }
    }

    /// `()` is `Unit`;""",
"""            ExprKind::Block(block) => self.block(block, mode),
            ExprKind::Closure(closure) => self.closure(expr, closure, expected),
            ExprKind::RecordUpdate { name, fields, base } => {
                self.record_update(name, fields, base, expected)
            }
            ExprKind::TupleField { receiver, index } => {
                match self.synth(receiver)? {
                    Ty::Tuple(items) if *index < items.len() => Some(items[*index].clone()),
                    Ty::Never => Some(Ty::Never),
                    other => {
                        self.error(
                            "E041",
                            expr.span,
                            format!("`{other}` has no element `{index}`"),
                        );
                        None
                    }
                }
            }
            ExprKind::Break => {
                if let Some(top) = self.loops.last_mut() {
                    *top = true;
                }
                Some(Ty::Never)
            }
            ExprKind::Continue => Some(Ty::Never),
        }
    }

    /// A closure literal (`docs/slice.md` section 13.3): parameter types
    /// from annotations, else from the expected fn type (E042 when
    /// neither gives one); the body checked against the expected return
    /// type or synthesised. `return`, loops and `?` inside it belong to
    /// it. Effects come from `telos-check`; an effectful closure has an
    /// effectful fn type, which a pure fn type does not accept (E017).
    fn closure(&mut self, expr: &Expr, closure: &Closure, expected: Option<&Ty>) -> Option<Ty> {
        let (wanted_params, wanted_ret) = match expected {
            Some(Ty::Fn(ps, r) | Ty::EffectFn(ps, r, _)) if ps.len() == closure.params.len() => {
                (Some(ps.clone()), Some((**r).clone()))
            }
            _ => (None, None),
        };
        let mut params: Vec<(String, Ty)> = Vec::new();
        let mut known = true;
        for (i, param) in closure.params.iter().enumerate() {
            let ty = match &param.ty {
                Some(t) => {
                    let ty = self.resolve(t);
                    if let Some(wanted) = wanted_params.as_ref().map(|ps| &ps[i])
                        && !compatible(wanted, &ty)
                    {
                        self.mismatch(param.span, wanted, &ty);
                    }
                    Some(ty)
                }
                None => wanted_params.as_ref().map(|ps| ps[i].clone()),
            };
            match ty.filter(|t| !t.has_param(&is_var)) {
                Some(ty) => params.push((param.name.name.clone(), ty)),
                None => {
                    self.error(
                        "E042",
                        param.name.span,
                        format!(
                            "cannot infer the type of closure parameter `{}`; annotate it: `|{}: T|`",
                            param.name.name, param.name.name
                        ),
                    );
                    known = false;
                }
            }
        }
        let wanted_ret = wanted_ret.filter(|t| !t.has_param(&is_var));
        let ret_scope = std::mem::replace(&mut self.ret, wanted_ret.clone());
        let loops = std::mem::take(&mut self.loops);
        let in_test_body = std::mem::replace(&mut self.in_test_body, false);
        self.closure_frames.push((self.scopes.len(), Vec::new()));
        self.scopes.push(BTreeMap::new());
        for (param, (_, ty)) in closure.params.iter().zip(&params) {
            self.bind(&param.name, Some(ty.clone()));
        }
        let body = match &wanted_ret {
            Some(Ty::Unit) => self.expr(&closure.body, Mode::Stmt).map(|t| match t {
                Ty::Never => Ty::Never,
                _ => Ty::Unit,
            }),
            Some(ret) => self.check(&closure.body, ret),
            None => self.synth(&closure.body),
        };
        self.scopes.pop();
        let (_, captures) = self.closure_frames.pop().unwrap_or_default();
        self.ret = ret_scope;
        self.loops = loops;
        self.in_test_body = in_test_body;
        if !known {
            return None;
        }
        let ret = match (wanted_ret, body?) {
            (Some(ret), _) => ret,
            (None, Ty::Never) => Ty::Never,
            (None, ty) => ty,
        };
        let effects = self
            .symbols
            .closure_effects
            .get(&expr.span)
            .map(|sig| FnEffects {
                names: sig.params.clone(),
                requires: sig.requires.clone(),
            });
        let captures = captures
            .into_iter()
            .map(|mut c| {
                c.effects = effects
                    .iter()
                    .flat_map(|e| &e.requires)
                    .filter(|e| e.split_once('.').is_some_and(|(head, _)| head == c.name))
                    .cloned()
                    .collect();
                c
            })
            .collect();
        let tys: Vec<Ty> = params.iter().map(|(_, t)| t.clone()).collect();
        let ty = match &effects {
            None => fn_ty(tys, ret.clone()),
            Some(effects) => {
                match expected {
                    Some(Ty::Fn(..)) => {
                        self.error(
                            "E017",
                            expr.span,
                            format!(
                                "effectful closure (requires [{}]) where the pure fn type `{}` is expected",
                                effects.requires.join(", "),
                                expected.map(Ty::to_string).unwrap_or_default()
                            ),
                        );
                        return None;
                    }
                    Some(Ty::EffectFn(_, _, wanted)) => {
                        let missing = effects.uncovered(wanted);
                        if !missing.is_empty() {
                            self.error(
                                "E017",
                                expr.span,
                                format!(
                                    "effectful closure does not fit `{}`: its effects {} are not covered by the type's `requires`",
                                    expected.map(Ty::to_string).unwrap_or_default(),
                                    missing.join(", ")
                                ),
                            );
                            return None;
                        }
                    }
                    _ => {}
                }
                Ty::EffectFn(tys, Box::new(ret.clone()), effects.clone())
            }
        };
        self.closures.insert(
            key(expr),
            ClosureInfo {
                params,
                ret,
                captures,
                effects,
            },
        );
        Some(ty)
    }

    /// `R { f = e, ..base }`: `base` has the record's type (from the
    /// expected type for a generic record, else synthesised); the fields
    /// are checked against that type's fields.
    fn record_update(
        &mut self,
        name: &Ident,
        fields: &[FieldInit],
        base: &Expr,
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        let Some(q) = self.records.get(&name.name).cloned() else {
            for field in fields {
                self.synth(&field.value);
            }
            self.synth(base);
            self.error(
                "E048",
                name.span,
                format!("unknown record type `{}`", name.name),
            );
            return None;
        };
        let generic = !self.generics_of(&q).is_empty();
        let record_ty = match expected {
            Some(t @ Ty::GenericRecord(eq, _)) if generic && *eq == q => Some(t.clone()),
            _ if !generic => Some(Ty::Record(q.clone())),
            _ => None,
        };
        let base_ty = match &record_ty {
            Some(t) => self.check(base, t),
            None => self.synth(base),
        };
        let record_ty = match (record_ty, base_ty) {
            (Some(t), _) => t,
            (None, Some(t @ Ty::GenericRecord(..))) => t,
            (None, Some(other)) => {
                let shown = Ty::Record(q.clone());
                self.mismatch(base.span, &shown, &other);
                return None;
            }
            (None, None) => return None,
        };
        if let Ty::GenericRecord(bq, _) = &record_ty
            && *bq != q
        {
            self.mismatch(base.span, &Ty::Record(q.clone()), &record_ty);
            return None;
        }
        for (i, field) in fields.iter().enumerate() {
            if fields[..i].iter().any(|f| f.name.name == field.name.name) {
                self.error(
                    "E047",
                    field.name.span,
                    format!(
                        "field `{}` is given twice in `{}` update",
                        field.name.name, name.name
                    ),
                );
            }
            match self.field_type(&record_ty, &field.name.name) {
                Some(ty) => {
                    self.check(&field.value, &ty);
                }
                None => {
                    self.error(
                        "E047",
                        field.name.span,
                        format!("record `{}` has no field `{}`", name.name, field.name.name),
                    );
                    self.synth(&field.value);
                }
            }
        }
        Some(record_ty)
    }

    /// `()` is `Unit`;"""),
# range pattern
("""            PatternKind::Wildcard => {}
            PatternKind::Int(value) => match ty {""",
"""            PatternKind::Wildcard => {}
            PatternKind::Range { start, end } => self.range_pattern(pattern, start, end, ty),
            PatternKind::Int(value) => match ty {"""),
("""    /// A constant used as a pattern: it matches a value equal to the""",
"""    /// `lo..=hi` on an `Int` or `Byte` scrutinee: each bound an integer
    /// literal or an `Int`/`Byte` constant whose value is known (E045
    /// otherwise), `lo <= hi` (E043 otherwise).
    fn range_pattern(&mut self, pattern: &Pattern, start: &Pattern, end: &Pattern, ty: Option<&Ty>) {
        let scalar = match ty {
            None | Some(Ty::Never) => None,
            Some(t @ (Ty::Int | Ty::Byte)) => Some(t.clone()),
            Some(other) => {
                self.error(
                    "E040",
                    pattern.span,
                    format!("type mismatch: expected `{other}`, found a range pattern; ranges match `Int` and `Byte`"),
                );
                return;
            }
        };
        let mut bounds = [0i64; 2];
        for (i, bound) in [start, end].into_iter().enumerate() {
            let value = match &bound.kind {
                PatternKind::Int(v) => {
                    if let Some(t) = &scalar {
                        self.int_literal(bound.span, *v, Some(t));
                    }
                    Some(*v)
                }
                PatternKind::Variant { path, args } if args.is_empty() && path.len() == 1 => {
                    let name = &path[0];
                    let q = self.consts.get(&name.name).cloned();
                    self.const_pattern(bound, name, scalar.as_ref());
                    q.and_then(|q| self.const_value(&q))
                }
                _ => None,
            };
            match value {
                Some(v) => bounds[i] = v,
                None => {
                    self.error(
                        "E045",
                        bound.span,
                        "a range bound must be an integer or byte literal or an `Int` or `Byte` constant with a literal value".to_string(),
                    );
                    return;
                }
            }
        }
        if bounds[0] > bounds[1] {
            self.error(
                "E043",
                pattern.span,
                format!("empty range `{}..={}`: the start is above the end", bounds[0], bounds[1]),
            );
            return;
        }
        self.ranges
            .insert(pattern as *const Pattern as usize, (bounds[0], bounds[1]));
    }

    /// The value of the `Int` or `Byte` constant `q`, when its value is a
    /// literal, a negated literal or another such constant.
    fn const_value(&self, q: &QualName) -> Option<i64> {
        if q.module == self.module {
            self.own.const_values.get(&q.name).copied()
        } else {
            self.modules.get(&q.module)?.const_values.get(&q.name).copied()
        }
    }

    /// Record the value of an own `Int` or `Byte` constant for range
    /// patterns (`ModuleTypes::const_values`).
    fn const_eval(&mut self, name: &str, value: &Expr) {
        let v = match &value.kind {
            ExprKind::Int(v) => Some(*v),
            ExprKind::Unary {
                op: UnaryOp::Neg,
                operand,
            } => match operand.kind {
                ExprKind::Int(v) => Some(v.wrapping_neg()),
                _ => None,
            },
            ExprKind::Name(n) => self
                .consts
                .get(&n.name)
                .cloned()
                .and_then(|q| self.const_value(&q)),
            _ => None,
        };
        if let Some(v) = v {
            self.own.const_values.insert(name.to_string(), v);
        }
    }

    /// A constant used as a pattern: it matches a value equal to the"""),
# const values recorded when a const body is typed
("""                Decl::Const(c) => {
                    if let Some(ty) = self.own.consts.get(&c.name.name).cloned() {
                        self.scopes = vec![BTreeMap::new()];
                        self.ret = None;
                        self.check(&c.value, &ty);
                        self.scopes.clear();
                    }
                }""",
"""                Decl::Const(c) => {
                    if let Some(ty) = self.own.consts.get(&c.name.name).cloned() {
                        self.scopes = vec![BTreeMap::new()];
                        self.ret = None;
                        self.check(&c.value, &ty);
                        self.scopes.clear();
                    }
                }
                Decl::Fn(f) if f.owner.is_some() => {
                    if let Some(sig) = self.own.fns.get(&f.qualified_name()).cloned() {
                        self.function(f, &sig);
                    }
                }"""),
("""    fn bodies(&mut self, program: &Program) {
        for (index, decl) in program.decls.iter().enumerate() {""",
"""    fn bodies(&mut self, program: &Program) {
        for _ in 0..2 {
            for decl in &program.decls {
                if let Decl::Const(c) = decl
                    && matches!(
                        self.own.consts.get(&c.name.name),
                        Some(Ty::Int | Ty::Byte)
                    )
                {
                    self.const_eval(&c.name.name, &c.value);
                }
            }
        }
        for (index, decl) in program.decls.iter().enumerate() {"""),
# names(): associated fns and glob imports
("""        for decl in &program.decls {
            let name = decl.name().name.clone();
            let qual = QualName::new(self.module.clone(), name.clone());
            let params = |ps: &[Ident]| ps.iter().map(|p| p.name.clone()).collect::<Vec<_>>();""",
"""        for decl in &program.decls {
            let name = match decl {
                Decl::Fn(f) => f.qualified_name(),
                _ => decl.name().name.clone(),
            };
            let qual = QualName::new(self.module.clone(), name.clone());
            let params = |ps: &[Ident]| ps.iter().map(|p| p.name.clone()).collect::<Vec<_>>();"""),
("""        for import in &program.imports {
            let segments = &import.path.segments;
            let (module_segments, names): (&[Ident], Vec<&Ident>) = if import.names.is_empty() {""",
"""        for import in program.imports.iter().filter(|i| i.is_glob()) {
            let module = import.module();
            let Some(types) = self.modules.get(&module) else {
                continue;
            };
            for (name, from) in &self.symbols.imported {
                if *from != module {
                    continue;
                }
                let qual = QualName::new(module.clone(), name.clone());
                let n = name.clone();
                let prefix = format!("{n}.");
                for f in types.fns.keys().filter(|f| f.starts_with(&prefix)) {
                    self.fns
                        .entry(f.clone())
                        .or_insert_with(|| QualName::new(module.clone(), f.clone()));
                }
                if types.records.contains_key(&n) {
                    self.records.entry(n).or_insert(qual);
                } else if types.enums.contains_key(&n) {
                    self.enums.entry(n).or_insert(qual);
                } else if types.fns.contains_key(&n) {
                    self.fns.entry(n).or_insert(qual);
                } else if types.consts.contains_key(&n) {
                    self.consts.entry(n).or_insert(qual);
                } else if types.protocols.contains_key(&n) {
                    self.protocols.entry(n).or_insert(qual);
                } else if types.aliases.contains_key(&n) {
                    self.aliases.entry(n).or_insert(qual);
                }
            }
        }
        for import in program.imports.iter().filter(|i| !i.is_glob()) {
            let segments = &import.path.segments;
            let (module_segments, names): (&[Ident], Vec<&Ident>) = if import.names.is_empty() {"""),
("""                let qual = QualName::new(module.clone(), name.name.clone());
                let n = name.name.clone();
                if types.records.contains_key(&n) {
                    self.records.insert(n, qual);
                } else if types.enums.contains_key(&n) {""",
"""                let qual = QualName::new(module.clone(), name.name.clone());
                let n = name.name.clone();
                let prefix = format!("{n}.");
                for f in types.fns.keys().filter(|f| f.starts_with(&prefix)) {
                    self.fns
                        .insert(f.clone(), QualName::new(module.clone(), f.clone()));
                }
                if types.records.contains_key(&n) {
                    self.records.insert(n, qual);
                } else if types.enums.contains_key(&n) {"""),
# signatures: associated fn key and E041 for builtin clash
("""                Decl::Fn(decl) => {
                    let sig = self.fn_sig(decl, false);
                    self.own.fns.entry(name).or_insert(sig);
                }""",
"""                Decl::Fn(decl) => {
                    let sig = self.fn_sig(decl, false);
                    if decl.owner.is_none()
                        && let Some(first) = sig.params.first()
                        && builtins::method(first, &decl.name.name).is_some()
                    {
                        self.error(
                            "E041",
                            decl.name.span,
                            format!(
                                "fn `{0}` takes a `{first}` first, and `{first}` has the builtin method `{0}`, so `x.{0}(..)` could never call it; rename it",
                                decl.name.name
                            ),
                        );
                    }
                    self.own.fns.entry(decl.qualified_name()).or_insert(sig);
                }"""),
("""                .functions
                .get(&decl.name.name)
                .map_or(decl.is_effectful(), |f| f.effectful);""",
"""                .functions
                .get(&decl.qualified_name())
                .map_or(decl.is_effectful(), |f| f.effectful);"""),
("""                Decl::Fn(f) => {
                    if let Some(sig) = self.own.fns.get(&f.name.name).cloned() {
                        self.function(f, &sig);
                    }
                }""",
"""                Decl::Fn(f) if f.owner.is_none() => {
                    if let Some(sig) = self.own.fns.get(&f.name.name).cloned() {
                        self.function(f, &sig);
                    }
                }"""),
# path_call: associated fns
("""        self.record(head_expr, Some(Ty::Unit));
        let owner = head.name.as_str();
        let what = format!("{owner}.{}", name.name);
        if self.is_enum(owner) {""",
"""        self.record(head_expr, Some(Ty::Unit));
        let owner = head.name.as_str();
        let what = format!("{owner}.{}", name.name);
        if let Some(q) = self.fns.get(&what).cloned()
            && let Some(sig) = self.sig_of(&q)
        {
            self.callees.insert(key(expr), Callee::Fn(q.clone()));
            let tparams = self.generics_of(&q);
            if !tparams.is_empty() {
                let ret = self.generic_call(
                    expr, &what, &tparams, &sig.params, args, &[], &sig.ret, expected,
                )?;
                let instance = self.instances.get(&key(expr)).cloned().unwrap_or_default();
                let params = sig.params.iter().map(|p| p.substitute(&tparams, &instance));
                if let Some(callee) = callee {
                    self.record(callee, Some(fn_ty(params.collect(), ret.clone())));
                }
                return Some(ret);
            }
            if let Some(callee) = callee {
                self.record(callee, Some(fn_ty(sig.params.clone(), sig.ret.clone())));
            }
            self.arguments(expr.span, &what, &sig.params, args);
            return Some(sig.ret);
        }
        if self.is_enum(owner) {"""),
("""        if let Some(head) = self.path_head(receiver) {
            self.record(receiver, Some(Ty::Unit));
            if self.is_enum(&head.name) {""",
"""        if let Some(head) = self.path_head(receiver) {
            self.record(receiver, Some(Ty::Unit));
            let what = format!("{}.{}", head.name, name.name);
            if let Some(q) = self.fns.get(&what).cloned()
                && let Some(sig) = self.sig_of(&q)
                && self.generics_of(&q).is_empty()
            {
                self.callees.insert(key(expr), Callee::Fn(q));
                let ty = fn_ty(sig.params, sig.ret);
                if sig.effectful {
                    let shown = Ident {
                        name: what,
                        span: name.span,
                    };
                    return self.effectful_value(&shown, ty, expected);
                }
                return Some(ty);
            }
            if self.is_enum(&head.name) {"""),
# method(): user fn of a builtin name when the builtin does not apply
("""        if BUILTIN_METHODS.contains(&name.name.as_str()) {
            let detail = if self.fns.contains_key(&name.name) {
                format!(
                    "; `{0}` is a builtin method name, so call the function as `{0}(x, ..)`",
                    name.name
                )
            } else {
                String::new()
            };
            return no_method(self, detail);
        }""",
"""        if BUILTIN_METHODS.contains(&name.name.as_str()) && !self.fns.contains_key(&name.name) {
            return no_method(self, String::new());
        }"""),
("""                self.callees.insert(key(expr), Callee::Fn(q));
                if !tparams.is_empty() {
                    let pre = [(receiver_ty.clone(), first.clone())];""",
"""                if sig.effectful && BUILTIN_METHODS.contains(&name.name.as_str()) {
                    self.effect_site(expr, &name.name);
                }
                self.callees.insert(key(expr), Callee::Fn(q));
                if !tparams.is_empty() {
                    let pre = [(receiver_ty.clone(), first.clone())];"""),
# show(): debug fallback
("""        if let Some(builtin) = builtin {
            return Some(Show::Builtin(builtin));
        }
        let q = self.fns.get("to_text")?;
        if q.module == self.module && self.symbols.test_only.contains("to_text") && !self.in_test {
            return None;
        }
        if !self.generics_of(q).is_empty() {
            return None;
        }
        let sig = self.sig_of(q)?;
        let fits = sig.params.len() == 1
            && compatible(ty, &sig.params[0])
            && sig.ret == Ty::Text
            && !sig.effectful;
        fits.then(|| Show::Fn(q.clone()))
    }""",
"""        if let Some(builtin) = builtin {
            return Some(Show::Builtin(builtin));
        }
        let debug = (!matches!(ty, Ty::Param(_) | Ty::Never)).then_some(Show::Debug);
        let Some(q) = self.fns.get("to_text") else {
            return debug;
        };
        if q.module == self.module && self.symbols.test_only.contains("to_text") && !self.in_test {
            return debug;
        }
        if !self.generics_of(q).is_empty() {
            return debug;
        }
        let Some(sig) = self.sig_of(q) else {
            return debug;
        };
        let fits = sig.params.len() == 1
            && compatible(ty, &sig.params[0])
            && sig.ret == Ty::Text
            && !sig.effectful;
        if fits { Some(Show::Fn(q.clone())) } else { debug }
    }"""),
# binary: chain and equality
("""            ExprKind::Binary { op, left, right } => self.binary(expr, *op, left, right, expected),""",
"""            ExprKind::Binary { op, left, right }
                if matches!(left.kind, ExprKind::Binary { .. }) =>
            {
                self.binary_chain(expr, *op, left, right, expected)
            }
            ExprKind::Binary { op, left, right } => self.binary(expr, *op, left, right, expected),"""),
("""    fn binary(
        &mut self,
        expr: &Expr,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        expected: Option<&Ty>,
    ) -> Option<Ty> {""",
"""    /// A binary operator whose left operand is another: the left spine is
    /// walked without recursion (`docs/slice.md` section 13, row 14), the
    /// innermost operator typed by [`Typer::binary`], each outer one by
    /// [`Typer::binary_rest`] with its left operand's type.
    fn binary_chain(
        &mut self,
        expr: &Expr,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        let hint_for = |op: BinaryOp, expected: Option<Ty>| -> Option<Ty> {
            match op {
                BinaryOp::And | BinaryOp::Or => Some(Ty::Bool),
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                    expected.filter(|t| matches!(t, Ty::Int | Ty::Byte))
                }
                _ => None,
            }
        };
        let mut spine: Vec<(&Expr, BinaryOp, &Expr, &Expr, Option<Ty>)> =
            vec![(expr, op, left, right, expected.cloned())];
        loop {
            let (_, op, left, _, expected) = spine.last().cloned().expect("non-empty spine");
            let ExprKind::Binary {
                op: inner_op,
                left: inner_left,
                right: inner_right,
            } = &left.kind
            else {
                break;
            };
            let expected = hint_for(op, expected);
            spine.push((left, *inner_op, inner_left, inner_right, expected));
        }
        let (bottom, op, left, right, expected) = spine.pop().expect("non-empty spine");
        let mut ty = self.binary(bottom, op, left, right, expected.as_ref());
        ty = self.record(bottom, ty);
        while let Some((node, op, left, right, expected)) = spine.pop() {
            ty = self.binary_rest(node, op, left, ty, right, expected.as_ref());
            if !spine.is_empty() {
                ty = self.record(node, ty);
            }
        }
        ty
    }

    /// The rest of [`Typer::binary`] once the left operand has been typed:
    /// `left_ty` is its type.
    fn binary_rest(
        &mut self,
        expr: &Expr,
        op: BinaryOp,
        left: &Expr,
        left_ty: Option<Ty>,
        right: &Expr,
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            if let Some(found) = &left_ty
                && !compatible(found, &Ty::Bool)
            {
                self.mismatch(left.span, &Ty::Bool, found);
            }
            self.check(right, &Ty::Bool);
            return Some(Ty::Bool);
        }
        let arithmetic = matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
        );
        let hint = if arithmetic {
            expected.filter(|t| matches!(t, Ty::Int | Ty::Byte))
        } else {
            None
        };
        let right_ty = match &left_ty {
            Some(Ty::Never) => self.expr(right, Mode::Value(hint)),
            Some(t) => self.check(right, t),
            None => self.synth(right),
        };
        self.operator(expr, op, left_ty?, right_ty?)
    }

    fn binary(
        &mut self,
        expr: &Expr,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        expected: Option<&Ty>,
    ) -> Option<Ty> {"""),
("""            (left_ty?, right_ty?)
        };
        let operand = if left_ty == Ty::Never {""",
"""            (left_ty?, right_ty?)
        };
        self.operator(expr, op, left_ty, right_ty)
    }

    /// The type of `left op right` from the operand types: `==` and `!=`
    /// on any comparable type (`docs/slice.md` section 13.5), `+` on
    /// numbers and `Text`, the others on numbers (E045 otherwise).
    fn operator(&mut self, expr: &Expr, op: BinaryOp, left_ty: Ty, right_ty: Ty) -> Option<Ty> {
        let arithmetic = matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
        );
        let operand = if left_ty == Ty::Never {"""),
("""        let defined = match op {
            BinaryOp::Eq | BinaryOp::Ne => {
                matches!(
                    operand,
                    Ty::Int | Ty::Byte | Ty::Bool | Ty::Text | Ty::Never
                )
            }""",
"""        let defined = match op {
            BinaryOp::Eq | BinaryOp::Ne => self.equatable(&operand, &mut Vec::new()),"""),
("""        if !defined {
            self.error(
                "E045",
                expr.span,
                format!("operator `{}` is not defined for `{operand}`", op.symbol()),
            );
            return (!arithmetic).then_some(Ty::Bool);
        }""",
"""        if !defined {
            let detail = if matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
                ": it holds a function, handle, store, `IoError` or type parameter"
            } else {
                ""
            };
            self.error(
                "E045",
                expr.span,
                format!(
                    "operator `{}` is not defined for `{operand}`{detail}",
                    op.symbol()
                ),
            );
            return (!arithmetic).then_some(Ty::Bool);
        }"""),
]
