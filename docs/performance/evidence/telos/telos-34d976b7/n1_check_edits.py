EDITS = [
# --- imports
("""use telos_syntax::ast::{
    Block, Decl, Expr, ExprKind, FieldInit, FnDecl, Ident, Import, Pattern, PatternKind, Program,
    ProtocolDecl, Span, Stmt, StmtKind, SystemDecl, TestDecl, Type,
};""",
"""use telos_syntax::ast::{
    Block, Closure, Decl, Expr, ExprKind, FieldInit, FnDecl, Ident, Import, Path, Pattern,
    PatternKind, Program, ProtocolDecl, Span, Stmt, StmtKind, SystemDecl, TestDecl, Type,
};"""),
# --- BUILTIN_METHODS doc + debug
("""/// Every builtin method name (`docs/slice.md` sections 6, 7 and 10). A
/// method call with one of these names is never a UFCS call of a user fn.
pub const BUILTIN_METHODS: &[&str] = &[
""",
"""/// Every builtin method name (`docs/slice.md` sections 6, 7, 10 and 13).
/// A method call with one of these names is a builtin when the builtin
/// applies to the receiver's type, else a UFCS call of a user fn of that
/// name (`docs/slice.md` section 13, row 5).
pub const BUILTIN_METHODS: &[&str] = &[
    "debug",
"""),
# --- Symbols fields
("""    pub test_only: BTreeSet<String>,
}
""",
"""    pub test_only: BTreeSet<String>,
    /// Names two glob imports both provide, to the modules providing them
    /// (`docs/slice.md` section 13). Neither is imported; a use is E041.
    pub glob_conflicts: BTreeMap<String, Vec<String>>,
    /// Effect signatures of the effectful closures, by the span of the
    /// closure literal: its parameter names and every effect its body
    /// performs, on its parameters (`p.read`), on the roots of its
    /// captures and ambient ones (`docs/slice.md` section 13.3).
    pub closure_effects: BTreeMap<Span, EffectSig>,
}
"""),
# --- Checker fields
("""    /// Whether the walk is inside a test or a test-only declaration, where
    /// test-only items are in scope.
    in_test: bool,
}
""",
"""    /// Whether the walk is inside a test or a test-only declaration, where
    /// test-only items are in scope.
    in_test: bool,
    /// For each closure being walked, innermost last: the number of scopes
    /// open outside it. A binding of a scope below that is a capture.
    closure_scopes: Vec<usize>,
}
"""),
# --- imports(): explicit first, then globs
("""        for import in &program.imports {
            self.import(import, modules);
        }
    }
""",
"""        for import in program.imports.iter().filter(|i| !i.is_glob()) {
            self.import(import, modules);
        }
        self.glob_imports(program, modules);
    }

    /// `import a.b.*`: every export of the module that is not test-only,
    /// unless the module defines or explicitly imports a name of that name;
    /// a name two globs provide is imported from neither and recorded in
    /// [`Symbols::glob_conflicts`].
    fn glob_imports(&mut self, program: &Program, modules: &Modules) {
        let local: BTreeSet<&str> = program
            .decls
            .iter()
            .filter(|d| !matches!(d, Decl::Test(_)))
            .map(|d| d.name().name.as_str())
            .collect();
        let mut offered: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for import in program.imports.iter().filter(|i| i.is_glob()) {
            let module = import.module();
            if module.is_empty() || !module.starts_with("std") {
                self.error(
                    "E031",
                    import.path.span,
                    format!("cannot import from `{module}`: only standard-library (`std.*`) modules can be imported in this slice"),
                );
                continue;
            }
            if SUBSTRATE_MODULES.contains(&module.as_str()) {
                continue;
            }
            let Some(exports) = modules.get(&module) else {
                self.error(
                    "E031",
                    import.path.span,
                    format!("unknown module `{module}`"),
                );
                continue;
            };
            let names: BTreeSet<&String> = exports
                .records
                .iter()
                .chain(exports.enums.keys())
                .chain(exports.functions.keys())
                .chain(exports.constants.iter())
                .chain(exports.protocols.keys())
                .chain(exports.aliases.keys())
                .filter(|n| !n.contains('.') && exports.exports(n))
                .collect();
            for name in names {
                let modules = offered.entry(name.clone()).or_default();
                if !modules.contains(&module) {
                    modules.push(module.clone());
                }
            }
        }
        for (name, from) in offered {
            if local.contains(name.as_str()) || self.symbols.imported.contains_key(&name) {
                continue;
            }
            if from.len() > 1 {
                self.symbols.glob_conflicts.insert(name, from);
                continue;
            }
            if let Some(exports) = modules.get(&from[0]) {
                self.import_name(&name, &from[0], exports);
            }
        }
    }

    /// E041 for a use of a name two glob imports provide; `false` when
    /// `name` is no such name.
    fn glob_conflict(&mut self, name: &Ident) -> bool {
        let Some(from) = self.symbols.glob_conflicts.get(&name.name) else {
            return false;
        };
        let message = format!(
            "`{}` is provided by the glob imports of {}; import it explicitly",
            name.name,
            quoted_list(from)
        );
        self.error("E041", name.span, message);
        true
    }
"""),
# --- import_name: associated fns travel with their owner
("""        if exports.functions.contains_key(name)
            && let Some(sig) = exports.effects.get(name)
        {
            self.symbols.effects.insert(name.to_string(), sig.clone());
        }
""",
"""        if exports.functions.contains_key(name)
            && let Some(sig) = exports.effects.get(name)
        {
            self.symbols.effects.insert(name.to_string(), sig.clone());
        }
        let prefix = format!("{name}.");
        for (key, info) in &exports.functions {
            if key.starts_with(&prefix) && !exports.test_only.contains(key) {
                self.symbols.functions.insert(key.clone(), info.clone());
                if let Some(sig) = exports.effects.get(key) {
                    self.symbols.effects.insert(key.clone(), sig.clone());
                }
                if let Some(params) = exports.type_params.get(key) {
                    self.symbols.type_params.insert(key.clone(), params.clone());
                }
            }
        }
"""),
# --- collect: associated fns
("""            if let Decl::Test(_) = decl {
                continue;
            }
            let name = decl.name();
""",
"""            if let Decl::Test(_) = decl {
                continue;
            }
            if let Decl::Fn(f) = decl
                && f.owner.is_some()
            {
                self.associated_fn(f, &mut owned);
                continue;
            }
            let name = decl.name();
"""),
("""    fn collect(&mut self, program: &Program) {
        let mut declared: BTreeSet<&str> = BTreeSet::new();
""",
"""    fn collect(&mut self, program: &Program) {
        let mut declared: BTreeSet<&str> = BTreeSet::new();
        let mut owned: Vec<&FnDecl> = Vec::new();
"""),
("""                Decl::Protocol(protocol) => {
                    let roles = self.protocol_roles(protocol);
                    self.symbols.protocols.insert(name.name.clone(), roles);
                }
            }
        }
    }
""",
"""                Decl::Protocol(protocol) => {
                    let roles = self.protocol_roles(protocol);
                    self.symbols.protocols.insert(name.name.clone(), roles);
                }
            }
        }
        for f in owned {
            self.associated_owner(f, program);
        }
    }

    /// Collect `fn Owner.name(..)` under the key `Owner.name`: E004 when
    /// declared twice.
    fn associated_fn<'p>(&mut self, f: &'p FnDecl, owned: &mut Vec<&'p FnDecl>) {
        let key = f.qualified_name();
        if self.symbols.functions.contains_key(&key) {
            self.error(
                "E004",
                f.name.span,
                format!("duplicate definition of `{key}`"),
            );
            return;
        }
        if f.test_only {
            self.symbols.test_only.insert(key.clone());
        }
        self.type_params_of(&key, &f.type_params);
        self.symbols.functions.insert(
            key.clone(),
            FnInfo {
                effectful: f.is_effectful(),
                arity: f.params.len(),
            },
        );
        if f.requires.is_some() {
            self.symbols.effects.insert(key, EffectSig::of(f));
        }
        owned.push(f);
    }

    /// The owner of an associated fn must be a record or enum this module
    /// declares (E003), without a variant of the fn's name (E004).
    fn associated_owner(&mut self, f: &FnDecl, program: &Program) {
        let Some(owner) = &f.owner else {
            return;
        };
        let declared = program.decls.iter().any(|d| {
            matches!(d, Decl::Record(_) | Decl::Enum(_)) && d.name().name == owner.name
        });
        if !declared {
            let message = match self.symbols.imported.get(&owner.name) {
                Some(module) => format!(
                    "associated fn `{}` must be declared in `{module}`, the module that declares `{}`",
                    f.qualified_name(),
                    owner.name
                ),
                None => format!(
                    "`{}` is no record or enum of this module; an associated fn needs one",
                    owner.name
                ),
            };
            self.error("E003", owner.span, message);
            return;
        }
        if self
            .symbols
            .enums
            .get(&owner.name)
            .is_some_and(|variants| variants.contains(&f.name.name))
        {
            self.error(
                "E004",
                f.name.span,
                format!(
                    "`{}` is both a variant and an associated fn of `{}`",
                    f.name.name, owner.name
                ),
            );
        }
    }
"""),
# --- E055 only through records and tuples
("""    /// `E055` for an own record or enum that holds a value of itself inline:
    /// through a field, a payload, a tuple, `Option`, `Result` or another
    /// record or enum (type arguments substituted), not through `List`, a
    /// fn type or `Store`, which hold their values out of line.
    fn recursive_types(&mut self, program: &Program) {
        for decl in &program.decls {
            let (name, params) = match decl {
                Decl::Record(r) => (&r.name, &r.type_params),
                Decl::Enum(e) => (&e.name, &e.type_params),
                _ => continue,
            };""",
"""    /// `E055` for an own record that holds a value of itself inline through
    /// fields and tuples alone (type arguments substituted), with no enum,
    /// `Option`, `Result`, `List`, `Store` or fn type in between: such a
    /// value would be infinite. Recursion through an enum payload is
    /// allowed and boxed (`docs/slice.md` section 13.4).
    fn recursive_types(&mut self, program: &Program) {
        for decl in &program.decls {
            let (name, params) = match decl {
                Decl::Record(r) => (&r.name, &r.type_params),
                _ => continue,
            };"""),
("""                let message = format!(
                    "recursive type `{0}` contains itself without `List` indirection; hold the nested `{0}` values in a `List`",
                    name.name
                );""",
"""                let message = format!(
                    "record `{0}` contains itself through fields and tuples alone, so a value would be infinite; put the nested `{0}` in an `Option`, an enum payload or a `List`",
                    name.name
                );"""),
("""    /// The user records and enums `ty` holds inline: itself when it is one,
    /// else those of its type arguments, except under `List`, `Store` and
    /// fn types.
    fn inline_types(&self, ty: &Type, out: &mut Vec<Type>) {
        if ty.is_fn() {
            return;
        }
        let ty = self.expand(ty);
        let name = ty.name.name.as_str();
        if name == "List" || name == "Store" || ty.is_fn() {
            return;
        }
        let user = self.symbols.records.contains(name)
            || (self.symbols.enums.contains_key(name) && !is_builtin_enum(name));
        if user {""",
"""    /// The user records `ty` holds inline: itself when it is one, else
    /// those of the elements of a tuple; nothing under an enum (`Option`
    /// and `Result` included), `List`, `Store` and fn types.
    fn inline_types(&self, ty: &Type, out: &mut Vec<Type>) {
        if ty.is_fn() {
            return;
        }
        let ty = self.expand(ty);
        let name = ty.name.name.as_str();
        if ty.is_fn() || !(ty.is_tuple() || self.symbols.records.contains(name)) {
            return;
        }
        if self.symbols.records.contains(name) {"""),
# --- infer_effects keyed by qualified name
("""            if let Decl::Fn(f) = decl
                && f.requires.is_none()
                && self.symbols.functions.contains_key(&f.name.name)
                && !index.contains_key(f.name.name.as_str())
            {
                index.insert(&f.name.name, fns.len());
                fns.push(f);
            }
        }""",
"""            if let Decl::Fn(f) = decl
                && f.requires.is_none()
                && self.symbols.functions.contains_key(&f.qualified_name())
                && !index.contains_key(&f.qualified_name())
            {
                index.insert(f.qualified_name(), fns.len());
                fns.push(f);
            }
        }"""),
("""        let mut fns: Vec<&FnDecl> = Vec::new();
        let mut index: BTreeMap<&str, usize> = BTreeMap::new();""",
"""        let mut fns: Vec<&FnDecl> = Vec::new();
        let mut index: BTreeMap<String, usize> = BTreeMap::new();"""),
("""                for &i in &component {
                    self.check_fn(fns[i], fns[i].name.name.clone());
                }""",
"""                for &i in &component {
                    self.check_fn(fns[i], fns[i].qualified_name());
                }"""),
("""    fn update_inferred(&mut self, decl: &FnDecl) {
        let name = &decl.name.name;""",
"""    fn update_inferred(&mut self, decl: &FnDecl) {
        let name = &decl.qualified_name();"""),
("""        let infer = decl.requires.is_none() && !display.contains('.');""",
"""        let role = decl.owner.is_none() && display.contains('.');
        let infer = decl.requires.is_none() && !role;"""),
("""                Decl::Fn(decl) => self.check_fn(decl, decl.name.name.clone()),""",
"""                Decl::Fn(decl) => self.check_fn(decl, decl.qualified_name()),"""),
# --- called_names: Owner.name for path calls
("""            ExprKind::Method {
                receiver,
                name,
                args,
            } => {
                names.insert(name.name.clone());
                stack.push(Node::Expr(receiver));""",
"""            ExprKind::Method {
                receiver,
                name,
                args,
            } => {
                names.insert(name.name.clone());
                if let ExprKind::Name(owner) = &receiver.kind {
                    names.insert(format!("{}.{}", owner.name, name.name));
                }
                stack.push(Node::Expr(receiver));"""),
# --- assignment to a capture
("""            StmtKind::Assign { target, value } => {
                self.expr(value);
                let base = &target.base;""",
"""            StmtKind::Assign { target, value } => {
                self.expr(value);
                let base = &target.base;
                if self.captured(&base.name) {
                    self.error(
                        "E006",
                        target.span,
                        format!(
                            "cannot assign to `{}` inside a closure; a closure captures `{}` by copy",
                            base.name, base.name
                        ),
                    );
                }"""),
# --- binary iterative
("""            ExprKind::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            ExprKind::Match { scrutinee, arms } => {
                self.expr(scrutinee);
                let roots = self.roots(scrutinee);""",
"""            ExprKind::Binary { .. } => {
                let mut rights = Vec::new();
                let mut bottom = expr;
                while let ExprKind::Binary { left, right, .. } = &bottom.kind {
                    rights.push(&**right);
                    bottom = left;
                }
                self.expr(bottom);
                for right in rights.into_iter().rev() {
                    self.expr(right);
                }
            }
            ExprKind::Match { scrutinee, arms } => {
                self.expr(scrutinee);
                let roots = self.roots(scrutinee);"""),
# --- method(): captured mutation, builtin vs ufcs
("""        self.expr(receiver);
        for arg in args {
            self.expr(arg);
        }
        if matches!(name.name.as_str(), "push" | "append")
            && let ExprKind::Name(list) = &receiver.kind
        {""",
"""        self.expr(receiver);
        for arg in args {
            self.expr(arg);
        }
        if matches!(name.name.as_str(), "push" | "append" | "clear")
            && let ExprKind::Name(list) = &receiver.kind
            && self.captured(&list.name)
            && self.builtin_method(receiver, name)
        {
            self.error(
                "E006",
                expr.span,
                format!(
                    "cannot change `{}` inside a closure; a closure captures `{}` by copy",
                    list.name, list.name
                ),
            );
        }
        if matches!(name.name.as_str(), "push" | "append")
            && let ExprKind::Name(list) = &receiver.kind
        {"""),
("""        if EFFECTFUL_METHODS.contains(&name.name.as_str()) {
            let what = match &receiver.kind {""",
"""        if EFFECTFUL_METHODS.contains(&name.name.as_str()) && self.builtin_method(receiver, name) {
            let what = match &receiver.kind {"""),
("""        if BUILTIN_METHODS.contains(&name.name.as_str()) {
            return;
        }
        let field = receiver_ty.and_then(|ty| self.field_type(&ty, &name.name));""",
"""        if self.builtin_method(receiver, name) {
            return;
        }
        let field = receiver_ty.and_then(|ty| self.field_type(&ty, &name.name));"""),
("""        if BUILTIN_METHODS.contains(&name.name.as_str()) {
            return BTreeSet::new();
        }
        let receiver_ty = self.static_type(receiver);""",
"""        if self.builtin_method(receiver, name) {
            return BTreeSet::new();
        }
        let receiver_ty = self.static_type(receiver);"""),
("""    fn ufcs(&self, receiver: &Expr, name: &Ident) -> Option<FnInfo> {
        if self.path_head(receiver).is_some() || BUILTIN_METHODS.contains(&name.name.as_str()) {
            return None;
        }""",
"""    fn ufcs(&self, receiver: &Expr, name: &Ident) -> Option<FnInfo> {
        if self.path_head(receiver).is_some() || self.builtin_method(receiver, name) {
            return None;
        }"""),
("""    /// The user fn a method call `receiver.name(..)` calls by UFCS: a
    /// module-level fn (own or imported) named `name`, when the receiver is
    /// not a path head and `name` is not a builtin method name. The type
    /// checker confirms it against the receiver type (`docs/slice.md`
    /// section 10).""",
"""    /// Whether `receiver.name(..)` is a builtin method call: `name` is a
    /// builtin method name and either no module-level fn has that name,
    /// or the receiver's declared type is unknown here, or the builtin
    /// applies to it (`docs/slice.md` section 13, row 5). The type checker
    /// reports E042 where an unknown receiver turns out to take an
    /// effectful user fn.
    fn builtin_method(&self, receiver: &Expr, name: &Ident) -> bool {
        if !BUILTIN_METHODS.contains(&name.name.as_str()) {
            return false;
        }
        if !self.symbols.functions.contains_key(&name.name) {
            return true;
        }
        match self.static_type(receiver) {
            None => true,
            Some(ty) => builtin_applies(&ty, &name.name),
        }
    }

    /// Whether binding `name` is captured by the closure being walked.
    fn captured(&self, name: &str) -> bool {
        let Some(&outside) = self.closure_scopes.last() else {
            return false;
        };
        self.scopes
            .iter()
            .rposition(|scope| scope.contains_key(name))
            .is_some_and(|index| index < outside)
    }

    /// The user fn a method call `receiver.name(..)` calls by UFCS: a
    /// module-level fn (own or imported) named `name`, when the receiver is
    /// not a path head and the call is no builtin method call
    /// ([`Checker::builtin_method`]). The type checker confirms it against
    /// the receiver type (`docs/slice.md` section 10)."""),
# --- path_call: associated fns
("""    fn path_call(&mut self, expr: &Expr, ty: &Ident, name: &Ident, args: &[Expr]) {
        for arg in args {
            self.expr(arg);
        }
        let what = format!("{}.{}", ty.name, name.name);
        self.test_only_use(ty);
        if let Some(variants) = self.symbols.enums.get(&ty.name) {""",
"""    fn path_call(&mut self, expr: &Expr, ty: &Ident, name: &Ident, args: &[Expr]) {
        for arg in args {
            self.expr(arg);
        }
        let what = format!("{}.{}", ty.name, name.name);
        self.test_only_use(ty);
        if let Some(info) = self.symbols.functions.get(&what).cloned() {
            if !self.in_test && self.symbols.test_only.contains(&what) {
                self.error(
                    "E051",
                    name.span,
                    format!("`{what}` is test-only; only tests and test-only items may use it"),
                );
            }
            self.arity(expr, &what, info.arity, args.len());
            if info.effectful {
                let all: Vec<&Expr> = args.iter().collect();
                let needed = self.callee_effects(&what, &all);
                self.effectful_op(needed, expr.span, &what);
            }
            return;
        }
        if let Some(variants) = self.symbols.enums.get(&ty.name) {"""),
("""    fn variant_value(&mut self, ty: &Ident, name: &Ident) {
        self.test_only_use(ty);
        match self.symbols.enums.get(&ty.name) {""",
"""    fn variant_value(&mut self, ty: &Ident, name: &Ident) {
        self.test_only_use(ty);
        if self
            .symbols
            .functions
            .contains_key(&format!("{}.{}", ty.name, name.name))
        {
            return;
        }
        match self.symbols.enums.get(&ty.name) {"""),
# --- unknown name: glob conflicts
("""        if self.out_of_scope_variant(&name.name) {
            return;
        }
        self.error("E003", name.span, format!("unknown name `{}`", name.name));
    }""",
"""        if self.out_of_scope_variant(&name.name) || self.glob_conflict(name) {
            return;
        }
        self.error("E003", name.span, format!("unknown name `{}`", name.name));
    }"""),
("""                } else if !self.out_of_scope_variant(&name.name) {
                    self.error("E003", name.span, format!("unknown name `{}`", name.name));
                }""",
"""                } else if !self.out_of_scope_variant(&name.name) && !self.glob_conflict(name) {
                    self.error("E003", name.span, format!("unknown name `{}`", name.name));
                }"""),
("""        if !structural && !known {
            self.error("E048", ty.name.span, format!("unknown type `{name}`"));""",
"""        if !structural && !known {
            if !self.glob_conflict(&ty.name) {
                self.error("E048", ty.name.span, format!("unknown type `{name}`"));
            }"""),
# --- roots of new forms
("""            ExprKind::Field { receiver, .. }
            | ExprKind::Index { receiver, .. }
            | ExprKind::Try(receiver)
            | ExprKind::Await(receiver) => self.roots(receiver),""",
"""            ExprKind::Field { receiver, .. }
            | ExprKind::Index { receiver, .. }
            | ExprKind::TupleField { receiver, .. }
            | ExprKind::Try(receiver)
            | ExprKind::Await(receiver) => self.roots(receiver),
            ExprKind::RecordUpdate { fields, base, .. } => fields
                .iter()
                .flat_map(|field| self.roots(&field.value))
                .chain(self.roots(base))
                .collect(),"""),
# --- static types of new forms
("""            ExprKind::Index { receiver, .. } => {
                let list = self.static_type(receiver)?;""",
"""            ExprKind::TupleField { receiver, index } => {
                let tuple = self.static_type(receiver)?;
                if !tuple.is_tuple() {
                    return None;
                }
                tuple.args.get(*index)?.clone()
            }
            ExprKind::Closure(closure) => return Some(self.closure_type(expr, closure)),
            ExprKind::Index { receiver, .. } => {
                let list = self.static_type(receiver)?;"""),
]

NEW_METHODS = '''
    /// A closure literal: its body is walked like the body of a fn without
    /// `requires`, its parameters rooted in themselves. Effects on its
    /// parameters belong to its fn type; every other effect (on a capture's
    /// roots or ambient) is also needed where the closure is created, which
    /// is how it carries the authority it captured (`docs/slice.md` section
    /// 13.3).
    fn closure(&mut self, expr: &Expr, closure: &Closure) {
        let ctx = std::mem::replace(&mut self.ctx, Ctx::Inferred);
        let inferred = std::mem::take(&mut self.inferred);
        let performs = std::mem::replace(&mut self.performs, false);
        self.closure_scopes.push(self.scopes.len());
        self.scopes.push(BTreeMap::new());
        let mut seen: Vec<&str> = Vec::new();
        for param in &closure.params {
            if seen.contains(&param.name.name.as_str()) && param.name.name != "_" {
                self.error(
                    "E004",
                    param.name.span,
                    format!("duplicate closure parameter `{}`", param.name.name),
                );
            }
            seen.push(&param.name.name);
            if let Some(ty) = &param.ty {
                self.check_type(ty);
            }
            let declared = param.ty.as_ref().map(|t| self.expand(t));
            let roots = BTreeSet::from([param.name.name.clone()]);
            self.bind_typed(&param.name, Binding::Param, roots, declared);
        }
        self.expr(&closure.body);
        self.scopes.pop();
        self.closure_scopes.pop();
        let effects = std::mem::replace(&mut self.inferred, inferred);
        let effectful = std::mem::replace(&mut self.performs, performs);
        self.ctx = ctx;
        let params: Vec<String> = closure.params.iter().map(|p| p.name.name.clone()).collect();
        if !effectful {
            self.symbols.closure_effects.remove(&expr.span);
            return;
        }
        let outer: Vec<String> = effects
            .iter()
            .filter(|effect| {
                let head = effect.split_once('.').map_or(effect.as_str(), |(h, _)| h);
                !params.iter().any(|p| p == head)
            })
            .cloned()
            .collect();
        if !outer.is_empty() {
            self.effectful_op(outer, expr.span, "closure");
        }
        self.symbols.closure_effects.insert(
            expr.span,
            EffectSig {
                params,
                requires: effects.into_iter().collect(),
            },
        );
    }

    /// The declared fn type of a closure literal as far as the checker
    /// knows it: parameter names, the annotated parameter types (`_`
    /// where none is written) and `requires` from its effects.
    fn closure_type(&self, expr: &Expr, closure: &Closure) -> Type {
        let unknown = || {
            Type::named(
                Ident {
                    name: "_".to_string(),
                    span: expr.span,
                },
                Vec::new(),
                expr.span,
            )
        };
        let mut args: Vec<Type> = closure
            .params
            .iter()
            .map(|p| p.ty.clone().unwrap_or_else(unknown))
            .collect();
        args.push(unknown());
        let requires = self.symbols.closure_effects.get(&expr.span).map(|sig| {
            sig.requires
                .iter()
                .map(|effect| Path {
                    segments: effect
                        .split('.')
                        .map(|s| Ident {
                            name: s.to_string(),
                            span: expr.span,
                        })
                        .collect(),
                    span: expr.span,
                })
                .collect()
        });
        Type {
            name: Ident {
                name: "fn".to_string(),
                span: expr.span,
            },
            args,
            span: expr.span,
            fn_names: closure.params.iter().map(|p| p.name.clone()).collect(),
            requires,
        }
    }

    /// `R { f = e, ..base }`: `E048` for an unknown record, `E047` for a
    /// field the record lacks or one given twice; omitted fields come from
    /// `base`.
    fn record_update(&mut self, name: &Ident, fields: &[FieldInit], base: &Expr) {
        self.test_only_use(name);
        let known = self.symbols.record_fields.get(&name.name).cloned();
        if known.is_none() {
            self.error(
                "E048",
                name.span,
                format!("unknown record type `{}`", name.name),
            );
        }
        let mut given: Vec<&str> = Vec::new();
        for field in fields {
            let field_name = field.name.name.as_str();
            if let Some(known) = &known {
                if !known.iter().any(|f| f == field_name) {
                    self.error(
                        "E047",
                        field.name.span,
                        format!("record `{}` has no field `{field_name}`", name.name),
                    );
                } else if given.contains(&field_name) {
                    self.error(
                        "E047",
                        field.name.span,
                        format!("field `{field_name}` is given twice in `{}` update", name.name),
                    );
                }
            }
            given.push(field_name);
            self.expr(&field.value);
        }
        self.expr(base);
    }

    // ---- declared types ---------------------------------------------------
'''

EDITS.append(("""
    // ---- declared types ---------------------------------------------------
""", NEW_METHODS))

EDITS.append(("""            ExprKind::RecordUpdate { name, fields, base } => {
                self.record_update(expr, name, fields, base)
            }""", """            ExprKind::RecordUpdate { name, fields, base } => {
                self.record_update(name, fields, base)
            }"""))

EDITS.append(("""/// The first name `pattern` binds, if any.""",
"""/// Whether the builtin method `method` applies to a receiver of declared
/// type `ty` (`docs/slice.md` sections 6, 7 and 13): the table of
/// `telos-types`' builtins by type name. A type parameter or a fn type
/// takes none but `copy`.
fn builtin_applies(ty: &Type, method: &str) -> bool {
    let name = ty.name.name.as_str();
    if method == "copy" || method == "debug" {
        return true;
    }
    let text = ty
        .args
        .first()
        .is_some_and(|a| a.name.name == "Text");
    match name {
        "Text" => matches!(
            method,
            "len" | "is_empty" | "trim" | "starts_with" | "strip_prefix" | "concat" | "to_bytes"
                | "to_text"
        ),
        "Int" => matches!(method, "to_text" | "to_byte"),
        "Byte" => matches!(method, "to_text" | "to_int"),
        "Bool" => method == "to_text",
        "Bytes" => matches!(
            method,
            "len" | "is_empty" | "at" | "slice" | "push" | "append" | "clear" | "map" | "to_text"
        ),
        "List" => {
            matches!(
                method,
                "len" | "is_empty" | "at" | "slice" | "push" | "append" | "clear" | "map"
            ) || (method == "join" && text)
        }
        "Store" => matches!(
            method,
            "next_id" | "get" | "put" | "remove" | "values" | "update"
        ),
        "TcpListener" => matches!(method, "accept" | "local_port"),
        "TcpStream" => matches!(method, "read" | "write_all" | "close" | "set_idle_timeout"),
        _ => false,
    }
}

/// The first name `pattern` binds, if any."""))
