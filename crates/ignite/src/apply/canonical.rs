//! The canonical form of a loaded program: the implementation projection of
//! apply identities (`docs/slice.md` 17.3, "Canonical implementation form").
//!
//! Each module becomes its sorted imports and one canonical text per
//! declaration. The text is an S-expression written from the syntax tree by
//! hand, never from `Debug`, so it holds no spans, comments or layout, and a
//! new AST field changes it only when this encoder is taught to write it.
//! Names are written by what they denote:
//!
//! - local bindings (`let`, `var`, `for`, pattern bindings, closure
//!   parameters) are numbered per declaration in binding order (`l#0`, ..);
//!   type parameters by position (`t#0`, ..);
//! - private module-level declarations are numbered by first reference
//!   (`#0`, ..), see [`public_names`] for what is public;
//! - everything else stays as written: public and imported names, builtins,
//!   fn parameters (named arguments call them by name), record fields,
//!   variants, methods and associated-fn members.
//!
//! A change of this encoding changes every identity: bump the identity
//! domains in `plan.rs` with it.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ignite_eval::Json;
use ignite_syntax::ast::{
    Block, Decl, Expr, ExprKind, FieldInit, FnDecl, Path, Pattern, PatternKind, Program, StmtKind,
    Type, UnaryOp,
};

/// One loaded module: its path and its parsed program.
pub(crate) struct Module<'p> {
    pub path: &'p str,
    pub program: &'p Program,
}

/// Module path to `{"imports": [..], "declarations": {key: text}}`; a
/// declaration's key is its name when public, its number when private, and
/// `Owner.member` for an associated fn. Test-only declarations, systems and
/// assemblies are left out (the model carries the latter two).
pub(crate) fn projection(modules: &[Module<'_>]) -> Json {
    let public = public_names(modules);
    let empty = BTreeSet::new();
    Json::Object(
        modules
            .iter()
            .map(|module| {
                let public = public.get(module.path).unwrap_or(&empty);
                let consts = visible_consts(module, modules);
                (
                    module.path.to_string(),
                    module_form(module.program, public, &consts),
                )
            })
            .collect(),
    )
}

fn is_std(path: &str) -> bool {
    path == "std" || path.starts_with("std.")
}

/// The module-level name a declaration takes; `None` for an associated fn
/// (`Owner.member` is no module-level name) and for a test.
fn free_name(decl: &Decl) -> Option<&str> {
    match decl {
        Decl::Fn(function) if function.owner.is_some() => None,
        Decl::Test(_) => None,
        _ => Some(&decl.name().name),
    }
}

fn projected(decl: &Decl) -> bool {
    !decl.is_test_only() && !matches!(decl, Decl::System(_) | Decl::Assembly(_))
}

fn module_decls<'m>(modules: &'m [Module<'_>], path: &str) -> &'m [Decl] {
    modules
        .iter()
        .find(|module| module.path == path)
        .map_or(&[], |module| &module.program.decls)
}

/// The free names of each module whose spelling is significant. Ignite has
/// no visibility keyword: a module exports every declaration (`docs/slice.md`
/// 4.1), so "private" is decided over the loaded program. A name is public
/// when
///
/// - its module is a std module (the compiler, the evaluator and every
///   program refer to std names);
/// - another module imports it, by name or by a glob of its module;
/// - its module is named by a `module("p")` literal and it is a fn whose name
///   is a field of some record (the literal fills the interface record's
///   fields by name, `docs/slice.md` 14.4);
/// - a system or assembly of its module names it (the model binds code by
///   name);
/// - it is a free fn `main` (the program entry point).
///
/// Every other non-test declaration is private: nothing reaches it by name
/// from outside its module.
fn public_names(modules: &[Module<'_>]) -> BTreeMap<String, BTreeSet<String>> {
    let mut public: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut literals = BTreeSet::new();
    let mut fields = BTreeSet::new();
    for module in modules {
        let entry = public.entry(module.path.to_string()).or_default();
        for decl in &module.program.decls {
            let Some(name) = free_name(decl) else {
                continue;
            };
            if is_std(module.path) || (matches!(decl, Decl::Fn(_)) && name == "main") {
                entry.insert(name.to_string());
            }
            if let Decl::Record(record) = decl {
                fields.extend(record.fields.iter().map(|field| field.name.name.clone()));
            }
        }
        let empty = BTreeSet::new();
        let mut encoder = Encoder::new(&empty, &empty);
        for decl in &module.program.decls {
            encoder.top(decl);
            if matches!(decl, Decl::System(_) | Decl::Assembly(_)) {
                public
                    .entry(module.path.to_string())
                    .or_default()
                    .append(&mut encoder.lookups);
            }
            encoder.lookups.clear();
        }
        literals.append(&mut encoder.literals);
    }
    for module in modules {
        for import in &module.program.imports {
            let target = import.module();
            let names: Vec<String> = if import.is_glob() {
                module_decls(modules, &target)
                    .iter()
                    .filter_map(free_name)
                    .map(str::to_string)
                    .collect()
            } else if import.names.is_empty() {
                vec![import.path.last().name.clone()]
            } else {
                import.names.iter().map(|name| name.name.clone()).collect()
            };
            public.entry(target).or_default().extend(names);
        }
    }
    for literal in literals {
        let interface: Vec<String> = module_decls(modules, &literal)
            .iter()
            .filter(|decl| matches!(decl, Decl::Fn(_)))
            .filter_map(free_name)
            .filter(|name| fields.contains(*name))
            .map(str::to_string)
            .collect();
        public.entry(literal).or_default().extend(interface);
    }
    public
}

/// The names a payload-less match pattern may take as a constant: the
/// module's own constants and the constants it imports.
fn visible_consts(module: &Module<'_>, modules: &[Module<'_>]) -> BTreeSet<String> {
    let consts = |decls: &[Decl]| -> BTreeSet<String> {
        decls
            .iter()
            .filter(|decl| matches!(decl, Decl::Const(_)))
            .map(|decl| decl.name().name.clone())
            .collect()
    };
    let mut visible = consts(&module.program.decls);
    for import in &module.program.imports {
        let target = consts(module_decls(modules, &import.module()));
        if import.is_glob() {
            visible.extend(target);
        } else if import.names.is_empty() {
            let name = &import.path.last().name;
            if target.contains(name) {
                visible.insert(name.clone());
            }
        } else {
            for name in &import.names {
                if target.contains(&name.name) {
                    visible.insert(name.name.clone());
                }
            }
        }
    }
    visible
}

fn module_form(program: &Program, public: &BTreeSet<String>, consts: &BTreeSet<String>) -> Json {
    let mut imports: Vec<String> = program
        .imports
        .iter()
        .map(|import| {
            let mut names: Vec<&str> = import.names.iter().map(|name| name.name.as_str()).collect();
            names.sort_unstable();
            format!("{}{{{}}}", import.path.dotted(), names.join(","))
        })
        .collect();
    imports.sort();
    let decls: Vec<&Decl> = program
        .decls
        .iter()
        .filter(|decl| projected(decl))
        .collect();
    let by_name: BTreeMap<&str, &Decl> = decls
        .iter()
        .filter_map(|decl| free_name(decl).map(|name| (name, *decl)))
        .collect();
    let privates: BTreeSet<String> = by_name
        .keys()
        .filter(|name| !public.contains(**name))
        .map(|name| name.to_string())
        .collect();
    let mut attached: BTreeMap<&str, Vec<&FnDecl>> = BTreeMap::new();
    let mut roots: Vec<(String, &Decl)> = Vec::new();
    for decl in &decls {
        match (free_name(decl), decl) {
            (Some(name), _) if privates.contains(name) => {}
            (None, Decl::Fn(function)) => {
                let owner = &function.owner.as_ref().expect("associated fn").name;
                if privates.contains(owner) {
                    attached.entry(owner.as_str()).or_default().push(function);
                } else {
                    roots.push((function.qualified_name(), *decl));
                }
            }
            _ => roots.push((decl.name().name.clone(), *decl)),
        }
    }
    for functions in attached.values_mut() {
        functions.sort_by(|a, b| a.name.name.cmp(&b.name.name));
    }
    roots.sort_by(|a, b| a.0.cmp(&b.0));
    let mut encoder = Encoder::new(&privates, consts);
    let mut out = BTreeMap::new();
    let drain = |encoder: &mut Encoder<'_>, out: &mut BTreeMap<String, Json>| {
        while let Some(name) = encoder.queue.pop_front() {
            let token = encoder.tokens[&name].clone();
            let decl = by_name[name.as_str()];
            out.insert(token.clone(), Json::Str(encoder.top(decl)));
            for function in attached.get(name.as_str()).into_iter().flatten() {
                let text = encoder.function(function);
                out.insert(format!("{token}.{}", function.name.name), Json::Str(text));
            }
        }
    };
    for (key, decl) in roots {
        out.insert(key, Json::Str(encoder.top(decl)));
        drain(&mut encoder, &mut out);
    }
    // Private declarations no public one reaches: unreferenced code, or
    // code only a system or test names. Each next root is the one whose
    // text, with the still unnumbered privates masked, sorts first; only
    // structurally identical ones fall back to their names.
    loop {
        let remaining: Vec<&str> = privates
            .iter()
            .filter(|name| !encoder.tokens.contains_key(*name))
            .map(String::as_str)
            .collect();
        let Some(next) = remaining
            .into_iter()
            .map(|name| (encoder.masked(by_name[name]), name))
            .min()
            .map(|(_, name)| name.to_string())
        else {
            break;
        };
        encoder.number(&next);
        drain(&mut encoder, &mut out);
    }
    let mut module = BTreeMap::new();
    module.insert(
        "imports".to_string(),
        Json::Array(imports.into_iter().map(Json::Str).collect()),
    );
    module.insert("declarations".to_string(), Json::Object(out));
    Json::Object(module)
}

struct Encoder<'e> {
    out: String,
    privates: &'e BTreeSet<String>,
    consts: &'e BTreeSet<String>,
    /// Private name to its number, in numbering order through `queue`.
    tokens: BTreeMap<String, String>,
    queue: VecDeque<String>,
    /// Write an unnumbered private as `#?` instead of numbering it.
    mask: bool,
    /// Local bindings in scope, innermost last; `frames` marks block starts.
    values: Vec<(String, String)>,
    frames: Vec<usize>,
    types: Vec<(String, String)>,
    next_local: usize,
    /// Every module-level name resolved, and every `module("p")` path.
    lookups: BTreeSet<String>,
    literals: BTreeSet<String>,
}

impl<'e> Encoder<'e> {
    fn new(privates: &'e BTreeSet<String>, consts: &'e BTreeSet<String>) -> Encoder<'e> {
        Encoder {
            out: String::new(),
            privates,
            consts,
            tokens: BTreeMap::new(),
            queue: VecDeque::new(),
            mask: false,
            values: Vec::new(),
            frames: Vec::new(),
            types: Vec::new(),
            next_local: 0,
            lookups: BTreeSet::new(),
            literals: BTreeSet::new(),
        }
    }

    fn number(&mut self, name: &str) -> String {
        let token = format!("#{}", self.tokens.len());
        self.tokens.insert(name.to_string(), token.clone());
        self.queue.push_back(name.to_string());
        token
    }

    /// The text of one top-level declaration.
    fn top(&mut self, decl: &Decl) -> String {
        self.reset();
        self.decl(decl);
        std::mem::take(&mut self.out)
    }

    /// The text of an associated fn of a private owner.
    fn function(&mut self, function: &FnDecl) -> String {
        self.reset();
        self.fn_decl(function, None);
        std::mem::take(&mut self.out)
    }

    fn masked(&mut self, decl: &Decl) -> String {
        self.mask = true;
        let text = self.top(decl);
        self.mask = false;
        text
    }

    fn reset(&mut self) {
        self.out.clear();
        self.values.clear();
        self.frames.clear();
        self.types.clear();
        self.next_local = 0;
    }

    fn open(&mut self, tag: &str) {
        self.out.push('(');
        self.out.push_str(tag);
    }

    fn close(&mut self) {
        self.out.push(')');
    }

    fn atom(&mut self, atom: &str) {
        self.out.push(' ');
        self.out.push_str(atom);
    }

    fn literal(&mut self, text: &str) {
        self.out.push(' ');
        self.out.push_str(&format!("{text:?}"));
    }

    /// A module-level name: a private one by its number, any other as
    /// written.
    fn global(&mut self, name: &str) -> String {
        self.lookups.insert(name.to_string());
        if !self.privates.contains(name) {
            return name.to_string();
        }
        if let Some(token) = self.tokens.get(name) {
            return token.clone();
        }
        if self.mask {
            return "#?".into();
        }
        self.number(name)
    }

    fn value(&mut self, name: &str) -> String {
        match self.values.iter().rev().find(|(bound, _)| bound == name) {
            Some((_, token)) => token.clone(),
            None => self.global(name),
        }
    }

    fn type_name(&mut self, name: &str) -> String {
        match self.types.iter().rev().find(|(bound, _)| bound == name) {
            Some((_, token)) => token.clone(),
            None => self.global(name),
        }
    }

    fn frame(&mut self) {
        self.frames.push(self.values.len());
    }

    fn unframe(&mut self) {
        let start = self.frames.pop().expect("an open frame");
        self.values.truncate(start);
    }

    fn fresh(&mut self) -> String {
        let token = format!("l#{}", self.next_local);
        self.next_local += 1;
        token
    }

    fn bind(&mut self, name: &str) -> String {
        let token = self.fresh();
        self.values.push((name.to_string(), token.clone()));
        token
    }

    fn type_params(&mut self, params: &[ignite_syntax::ast::Ident]) {
        let start = self.types.len();
        for (index, param) in params.iter().enumerate() {
            self.types
                .push((param.name.clone(), format!("t#{}", start + index)));
        }
        self.atom(&format!("{}", params.len()));
    }

    fn decl(&mut self, decl: &Decl) {
        match decl {
            Decl::Fn(function) => self.fn_decl(function, None),
            Decl::Record(record) => {
                self.open("record");
                self.type_params(&record.type_params);
                for field in &record.fields {
                    self.open("field");
                    self.atom(&field.name.name);
                    self.ty(&field.ty);
                    if let Some(default) = &field.default {
                        self.open("default");
                        self.expr(default);
                        self.close();
                    }
                    self.paths("satisfies", &field.satisfies);
                    self.close();
                }
                self.paths("satisfies", &record.satisfies);
                self.close();
            }
            Decl::Enum(enumeration) => {
                self.open("enum");
                self.type_params(&enumeration.type_params);
                for variant in &enumeration.variants {
                    self.open("variant");
                    self.atom(&variant.name.name);
                    for payload in &variant.payload {
                        self.ty(payload);
                    }
                    self.close();
                }
                self.close();
            }
            Decl::Alias(alias) => {
                self.open("alias");
                self.type_params(&alias.params);
                self.ty(&alias.ty);
                self.close();
            }
            Decl::Const(constant) => {
                self.open("const");
                self.ty(&constant.ty);
                self.expr(&constant.value);
                self.close();
            }
            Decl::Protocol(protocol) => {
                self.open("protocol");
                self.open("conforms");
                for spec in &protocol.conforms {
                    let spec = self.global(&spec.name);
                    self.atom(&spec);
                }
                self.close();
                if let Some(scope) = &protocol.scope {
                    self.open("scope");
                    if let Some(parties) = &scope.parties {
                        self.open("parties");
                        for party in parties {
                            self.atom(&party.name);
                        }
                        self.close();
                    }
                    if let Some(sections) = &scope.sections {
                        self.open("sections");
                        for (section, _) in sections {
                            self.literal(section);
                        }
                        self.close();
                    }
                    self.close();
                }
                for status in &protocol.statuses {
                    self.open(status.kind.word());
                    self.path(&status.requirement);
                    self.literal(&status.reason);
                    self.close();
                }
                for role in &protocol.fns {
                    self.fn_decl(role, Some(&role.name.name));
                }
                self.close();
            }
            Decl::Test(test) => {
                self.open("test");
                self.paths("verifies", &test.verifies);
                self.block(&test.body);
                self.close();
            }
            Decl::System(system) => {
                self.open("system");
                for resource in &system.resources {
                    self.open("resource");
                    self.atom(&resource.name.name);
                    let kind = self.type_name(&resource.kind.name);
                    self.atom(&kind);
                    for arg in &resource.args {
                        self.ty(arg);
                    }
                    self.inits(&resource.properties);
                    self.close();
                }
                for (tag, members) in [
                    ("component", &system.components),
                    ("contract", &system.contracts),
                    ("policy", &system.policies),
                ] {
                    for member in members {
                        self.open(tag);
                        self.atom(&member.name.name);
                        let kind = self.type_name(&member.kind.name);
                        self.atom(&kind);
                        self.inits(&member.properties);
                        self.close();
                    }
                }
                self.close();
            }
            Decl::Assembly(assembly) => {
                self.open("assembly");
                let kind = self.type_name(&assembly.kind.name);
                self.atom(&kind);
                self.inits(&assembly.properties);
                self.close();
            }
        }
    }

    /// `member` is written for a protocol role; a module-level fn's name is
    /// its key.
    fn fn_decl(&mut self, function: &FnDecl, member: Option<&str>) {
        self.open("fn");
        if let Some(member) = member {
            self.atom(member);
        }
        let types = self.types.len();
        self.type_params(&function.type_params);
        for bound in &function.bounds {
            self.open("bound");
            let param = self.type_name(&bound.param.name);
            self.atom(&param);
            for interface in &bound.interfaces {
                let interface = self.type_name(&interface.name);
                self.atom(&interface);
            }
            self.close();
        }
        self.frame();
        self.open("params");
        for param in &function.params {
            self.open("param");
            self.atom(&param.name.name);
            self.ty(&param.ty);
            self.close();
            self.values
                .push((param.name.name.clone(), format!("a:{}", param.name.name)));
        }
        self.close();
        match &function.ret {
            Some(ret) => self.ty(ret),
            None => self.atom("-"),
        }
        match &function.requires {
            Some(requires) => self.paths("requires", requires),
            None => self.atom("-"),
        }
        self.paths("satisfies", &function.satisfies);
        self.block(&function.body);
        self.unframe();
        self.types.truncate(types);
        self.close();
    }

    fn inits(&mut self, inits: &[FieldInit]) {
        for init in inits {
            self.open("=");
            self.atom(&init.name.name);
            self.expr(&init.value);
            self.close();
        }
    }

    /// `Spec.Id`, an effect or a capability: the first segment resolves like
    /// a module-level name, the rest are members.
    fn path(&mut self, path: &Path) {
        self.open("path");
        for (index, segment) in path.segments.iter().enumerate() {
            if index == 0 {
                let first = self.global(&segment.name);
                self.atom(&first);
            } else {
                self.atom(&segment.name);
            }
        }
        self.close();
    }

    fn paths(&mut self, tag: &str, paths: &[Path]) {
        self.open(tag);
        for path in paths {
            self.path(path);
        }
        self.close();
    }

    fn ty(&mut self, ty: &Type) {
        self.open("T");
        let name = self.type_name(&ty.name.name);
        self.atom(&name);
        for arg in &ty.args {
            self.ty(arg);
        }
        if !ty.fn_names.is_empty() {
            self.open("names");
            for name in &ty.fn_names {
                self.atom(&name.name);
            }
            self.close();
        }
        if let Some(requires) = &ty.requires {
            self.paths("requires", requires);
        }
        self.close();
    }

    fn opt_ty(&mut self, ty: Option<&Type>) {
        match ty {
            Some(ty) => self.ty(ty),
            None => self.atom("-"),
        }
    }

    fn block(&mut self, block: &Block) {
        self.open("block");
        self.frame();
        for stmt in &block.stmts {
            self.stmt(&stmt.kind);
        }
        self.unframe();
        self.close();
    }

    fn stmt(&mut self, stmt: &StmtKind) {
        match stmt {
            StmtKind::Let {
                name,
                ty,
                value,
                mutable,
            } => {
                self.open(if *mutable { "var" } else { "let" });
                self.opt_ty(ty.as_ref());
                self.expr(value);
                let token = self.bind(&name.name);
                self.atom(&token);
                self.close();
            }
            StmtKind::LetPattern {
                pattern,
                value,
                mutable,
            } => {
                self.open(if *mutable {
                    "var-pattern"
                } else {
                    "let-pattern"
                });
                self.expr(value);
                self.bound_pattern(pattern, false);
                self.close();
            }
            StmtKind::Assign { target, value } => {
                self.open("assign");
                let base = self.value(&target.base.name);
                self.atom(&base);
                for field in &target.fields {
                    self.atom(&field.name);
                }
                self.expr(value);
                self.close();
            }
            StmtKind::Return(value) => {
                self.open("return");
                if let Some(value) = value {
                    self.expr(value);
                }
                self.close();
            }
            StmtKind::While { cond, body } => {
                self.open("while");
                self.expr(cond);
                self.block(body);
                self.close();
            }
            StmtKind::Loop { body } => {
                self.open("loop");
                self.block(body);
                self.close();
            }
            StmtKind::For { name, iter, body } => {
                self.open("for");
                self.expr(iter);
                self.frame();
                let token = self.bind(&name.name);
                self.atom(&token);
                self.block(body);
                self.unframe();
                self.close();
            }
            StmtKind::ForPattern {
                pattern,
                iter,
                body,
            } => {
                self.open("for-pattern");
                self.expr(iter);
                self.frame();
                self.bound_pattern(pattern, false);
                self.block(body);
                self.unframe();
                self.close();
            }
            StmtKind::Break => self.atom("(break)"),
            StmtKind::Continue => self.atom("(continue)"),
            StmtKind::Expr(expr) => self.expr(expr),
            StmtKind::Assert { cond, message } => {
                self.open("assert");
                self.expr(cond);
                if let Some(message) = message {
                    self.expr(message);
                }
                self.close();
            }
        }
    }

    /// Write `pattern` and bring its bindings into the current frame. `arm`:
    /// a match arm, where a payload-less name of a constant is a constant
    /// pattern (`docs/slice.md` 4.3).
    fn bound_pattern(&mut self, pattern: &Pattern, arm: bool) {
        let mut bound = Vec::new();
        self.pattern(pattern, arm, &mut bound);
        self.values.extend(bound);
    }

    fn pattern(&mut self, pattern: &Pattern, arm: bool, bound: &mut Vec<(String, String)>) {
        let mut bind = |encoder: &mut Encoder<'_>, name: &str| {
            // The alternatives of an or-pattern bind the same names.
            let token = match bound.iter().find(|(bound, _)| bound == name) {
                Some((_, token)) => token.clone(),
                None => {
                    let token = encoder.fresh();
                    bound.push((name.to_string(), token.clone()));
                    token
                }
            };
            encoder.open("bind");
            encoder.atom(&token);
            encoder.close();
        };
        match &pattern.kind {
            PatternKind::Wildcard => self.atom("_"),
            PatternKind::Int(value) => self.atom(&format!("(i {value})")),
            PatternKind::Str(value) => {
                self.open("s");
                self.literal(value);
                self.close();
            }
            PatternKind::Bool(value) => self.atom(&format!("(b {value})")),
            PatternKind::Or(alternatives) => {
                self.open("or");
                for alternative in alternatives {
                    self.pattern(alternative, arm, bound);
                }
                self.close();
            }
            PatternKind::Tuple(elements) => {
                self.open("tuple");
                for element in elements {
                    self.pattern(element, arm, bound);
                }
                self.close();
            }
            PatternKind::List(elements) => {
                self.open("list");
                for element in elements {
                    self.pattern(element, arm, bound);
                }
                self.close();
            }
            PatternKind::Rest(name) => {
                self.open("rest");
                if let Some(name) = name {
                    bind(self, &name.name);
                }
                self.close();
            }
            PatternKind::Binding(name) if arm && self.consts.contains(&name.name) => {
                self.open("const");
                let constant = self.global(&name.name);
                self.atom(&constant);
                self.close();
            }
            PatternKind::Binding(name) => bind(self, &name.name),
            PatternKind::Variant { path, args } => {
                self.open("variant");
                let last = path.len().saturating_sub(1);
                for (index, segment) in path.iter().enumerate() {
                    if index == 0 && (index < last || self.privates.contains(&segment.name)) {
                        let first = self.global(&segment.name);
                        self.atom(&first);
                    } else {
                        self.atom(&segment.name);
                    }
                }
                for arg in args {
                    self.pattern(arg, arm, bound);
                }
                self.close();
            }
            PatternKind::Range { start, end } => {
                self.open("range");
                self.pattern(start, arm, bound);
                self.pattern(end, arm, bound);
                self.close();
            }
        }
    }

    fn exprs(&mut self, exprs: &[Expr]) {
        for expr in exprs {
            self.expr(expr);
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Int(value) => self.atom(&format!("(i {value})")),
            ExprKind::Str(value) => {
                self.open("s");
                self.literal(value);
                self.close();
            }
            ExprKind::Bool(value) => self.atom(&format!("(b {value})")),
            ExprKind::Name(name) => {
                self.open("n");
                let token = self.value(&name.name);
                self.atom(&token);
                self.close();
            }
            ExprKind::Record { name, fields } => {
                self.open("record");
                let name = self.type_name(&name.name);
                self.atom(&name);
                self.inits(fields);
                self.close();
            }
            ExprKind::List(elements) => {
                self.open("list");
                self.exprs(elements);
                self.close();
            }
            ExprKind::Tuple(elements) => {
                self.open("tuple");
                self.exprs(elements);
                self.close();
            }
            ExprKind::Return(value) => {
                self.open("return");
                if let Some(value) = value {
                    self.expr(value);
                }
                self.close();
            }
            ExprKind::Call { callee, args } => {
                if let (ExprKind::Name(name), [argument]) = (&callee.kind, args.as_slice())
                    && name.name == "module"
                    && let ExprKind::Str(path) = &argument.kind
                {
                    self.literals.insert(path.clone());
                }
                self.open("call");
                self.expr(callee);
                self.exprs(args);
                self.close();
            }
            ExprKind::Method {
                receiver,
                name,
                args,
            } => {
                self.open("method");
                self.expr(receiver);
                // A private fn called as a method (UFCS) takes its number.
                let method = if self.privates.contains(&name.name) {
                    self.global(&name.name)
                } else {
                    self.lookups.insert(name.name.clone());
                    name.name.clone()
                };
                self.atom(&method);
                self.exprs(args);
                self.close();
            }
            ExprKind::Field { receiver, name } => {
                self.open(".");
                self.expr(receiver);
                self.atom(&name.name);
                self.close();
            }
            ExprKind::Index { receiver, index } => {
                self.open("index");
                self.expr(receiver);
                self.expr(index);
                self.close();
            }
            ExprKind::Try(inner) => self.wrapped("try", inner),
            ExprKind::Await(inner) => self.wrapped("await", inner),
            ExprKind::Spawn(inner) => self.wrapped("spawn", inner),
            ExprKind::Unary { op, operand } => {
                let tag = match op {
                    UnaryOp::Not => "not",
                    UnaryOp::Neg => "neg",
                    UnaryOp::BitNot => "bitnot",
                };
                self.wrapped(tag, operand);
            }
            ExprKind::Binary { .. } => {
                // The left spine iteratively: an interpolation or a long
                // `+` chain is a deep left-folded tree.
                let mut spine = Vec::new();
                let mut leftmost = expr;
                while let ExprKind::Binary { op, left, right } = &leftmost.kind {
                    spine.push((op.symbol(), &**right));
                    leftmost = left;
                }
                self.open("chain");
                self.expr(leftmost);
                for (symbol, right) in spine.into_iter().rev() {
                    self.open(symbol);
                    self.expr(right);
                    self.close();
                }
                self.close();
            }
            ExprKind::Match { scrutinee, arms } => {
                self.open("match");
                self.expr(scrutinee);
                for arm in arms {
                    self.open("arm");
                    self.frame();
                    self.bound_pattern(&arm.pattern, true);
                    self.expr(&arm.body);
                    self.unframe();
                    self.close();
                }
                self.close();
            }
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => {
                self.open("if");
                self.expr(cond);
                self.block(then);
                if let Some(otherwise) = otherwise {
                    self.expr(otherwise);
                }
                self.close();
            }
            ExprKind::Block(block) => self.block(block),
            ExprKind::Closure(closure) => {
                self.open("closure");
                self.frame();
                for param in &closure.params {
                    self.open("param");
                    self.opt_ty(param.ty.as_ref());
                    let token = self.bind(&param.name.name);
                    self.atom(&token);
                    self.close();
                }
                self.expr(&closure.body);
                self.unframe();
                self.close();
            }
            ExprKind::RecordUpdate { name, fields, base } => {
                self.open("update");
                let name = self.type_name(&name.name);
                self.atom(&name);
                self.inits(fields);
                self.expr(base);
                self.close();
            }
            ExprKind::TupleField { receiver, index } => {
                self.open("element");
                self.expr(receiver);
                self.atom(&index.to_string());
                self.close();
            }
            ExprKind::Break => self.atom("(break)"),
            ExprKind::Continue => self.atom("(continue)"),
            ExprKind::Quantity { literal, unit } => {
                self.open("quantity");
                self.literal(literal);
                self.atom(&unit.name);
                self.close();
            }
            ExprKind::NamedArg { name, value } => {
                self.open("named");
                self.atom(&name.name);
                self.expr(value);
                self.close();
            }
            ExprKind::Brace { entries } => {
                self.open("brace");
                for entry in entries {
                    self.open("=");
                    for segment in &entry.key.segments {
                        self.atom(&segment.name);
                    }
                    self.expr(&entry.value);
                    self.close();
                }
                self.close();
            }
        }
    }

    fn wrapped(&mut self, tag: &str, inner: &Expr) {
        self.open(tag);
        self.expr(inner);
        self.close();
    }
}
