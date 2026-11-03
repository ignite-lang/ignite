//! The names of the clauses `conforms`, `satisfies`, `verifies` and the
//! protocol statuses (`docs/slice.md` 19.6), and the literal-data rule of
//! catalogue constants, E091's first trigger (19.4).
//!
//! Only what names alone decide is checked here: a name that names nothing
//! (E003), a requirement its spec's module does not declare (E003) and a
//! spec named twice in one `conforms` (E004). Whether a name is a `Spec` or
//! a `Requirement` constant is the type checker's (E040).

use std::collections::{BTreeMap, BTreeSet};

use ignite_syntax::ast::{BinaryOp, Decl, Expr, ExprKind, Ident, Path, Program, Span, UnaryOp};

use crate::{Checker, Modules, Symbols};

/// The module that declares the clause vocabulary.
const RFC_MODULE: &str = "std.rfc";

/// The enums of `std.rfc` whose variants are literal data.
const LITERAL_ENUMS: &[&str] = &["Level", "Party", "Source"];

impl Checker {
    /// E003 and E004 for every clause of `program`.
    pub(super) fn clauses(&mut self, program: &Program, modules: &Modules) {
        for decl in &program.decls {
            match decl {
                Decl::Fn(f) => self.reqrefs(&f.satisfies, program, modules),
                Decl::Record(record) => {
                    self.reqrefs(&record.satisfies, program, modules);
                    for field in &record.fields {
                        self.reqrefs(&field.satisfies, program, modules);
                    }
                }
                Decl::Test(test) => self.reqrefs(&test.verifies, program, modules),
                Decl::Protocol(protocol) => {
                    let mut seen: BTreeSet<&str> = BTreeSet::new();
                    for spec in &protocol.conforms {
                        if !seen.insert(spec.name.as_str()) {
                            let message = format!(
                                "duplicate spec `{}` in `conforms` of protocol `{}`",
                                spec.name, protocol.name.name
                            );
                            self.error("E004", spec.span, message);
                        } else {
                            self.spec_name(spec);
                        }
                    }
                    for f in &protocol.fns {
                        self.reqrefs(&f.satisfies, program, modules);
                    }
                    for status in &protocol.statuses {
                        self.reqref(&status.requirement, program, modules);
                    }
                }
                Decl::Enum(_)
                | Decl::System(_)
                | Decl::Const(_)
                | Decl::Alias(_)
                | Decl::Assembly(_) => {}
            }
        }
    }

    fn reqrefs(&mut self, refs: &[Path], program: &Program, modules: &Modules) {
        for path in refs {
            self.reqref(path, program, modules);
        }
    }

    /// E003 when the first segment of `path` names nothing, or when it is
    /// a constant whose module declares no constant named by the second.
    fn reqref(&mut self, path: &Path, program: &Program, modules: &Modules) {
        let [spec, id] = path.segments.as_slice() else {
            return;
        };
        if !self.spec_name(spec) || !self.symbols.constants.contains(&spec.name) {
            return;
        }
        let declared = match self.symbols.imported.get(&spec.name) {
            Some(module) => modules.get(module).is_some_and(|m| {
                m.constants.contains(&id.name) && !m.imported.contains_key(&id.name)
            }),
            None => program
                .decls
                .iter()
                .any(|d| matches!(d, Decl::Const(c) if c.name.name == id.name)),
        };
        if !declared {
            let message = format!("`{}` has no requirement `{}`", spec.name, id.name);
            self.error("E003", id.span, message);
        }
    }

    /// E003 when `spec` names nothing in scope; `true` when it names
    /// something.
    fn spec_name(&mut self, spec: &Ident) -> bool {
        let name = spec.name.as_str();
        let s = &self.symbols;
        let known = s.constants.contains(name)
            || s.functions.contains_key(name)
            || s.records.contains(name)
            || s.enums.contains_key(name)
            || s.variants.contains_key(name)
            || s.protocols.contains_key(name)
            || s.aliases.contains_key(name)
            || s.systems.contains_key(name);
        if !known {
            self.error("E003", spec.span, format!("unknown name `{name}`"));
        }
        known
    }

    /// E091 (first trigger, `docs/slice.md` 19.4) at the first part of the
    /// value of each `std.rfc.Requirement` or `std.rfc.Spec` constant that
    /// is not literal data.
    pub(super) fn catalogue_literals(&mut self, program: &Program, modules: &Modules) {
        let own_module = program.package.as_ref().is_some_and(|p| {
            p.segments
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                == ["std", "rfc"]
        });
        let symbols = &self.symbols;
        let vocabulary = |name: &str| match symbols.imported.get(name) {
            Some(module) => module == RFC_MODULE,
            None => own_module,
        };
        let kind_of =
            |ty: &ignite_syntax::ast::Type| catalogue_kind(ty, symbols, own_module, modules);
        let variants: BTreeMap<&str, &Vec<String>> = LITERAL_ENUMS
            .iter()
            .filter(|e| vocabulary(e))
            .filter_map(|e| symbols.enums.get(*e).map(|v| (*e, v)))
            .collect();
        let requirements: BTreeSet<&str> = program
            .decls
            .iter()
            .filter_map(|d| match d {
                Decl::Const(c) if kind_of(&c.ty) == Some("Requirement") => {
                    Some(c.name.name.as_str())
                }
                _ => None,
            })
            .collect();
        let literals = Literals {
            variants,
            requirements,
        };
        let mut found = Vec::new();
        for decl in &program.decls {
            let Decl::Const(c) = decl else {
                continue;
            };
            let Some(record) = kind_of(&c.ty) else {
                continue;
            };
            if let Some((span, what)) = literals.record(&c.value, record) {
                let message = format!(
                    "catalogue constant `{}` must be literal data, found {what}",
                    c.name.name
                );
                found.push((span, message));
            }
        }
        for (span, message) in found {
            self.error("E091", span, message);
        }
    }
}

/// The most aliases [`catalogue_kind`] follows; a longer chain is a cycle,
/// which E048 reports.
const ALIAS_DEPTH: usize = 64;

/// `"Requirement"` or `"Spec"` when the declared type `ty` is that record
/// of `std.rfc`, written directly or through type aliases, own or
/// imported; an imported alias's target is read in its own module.
/// `symbols` and `own_module` (whether they are `std.rfc`'s) are the scope
/// `ty` is written in.
fn catalogue_kind(
    ty: &ignite_syntax::ast::Type,
    symbols: &Symbols,
    own_module: bool,
    modules: &Modules,
) -> Option<&'static str> {
    let (mut ty, mut symbols, mut own_module) = (ty, symbols, own_module);
    for _ in 0..ALIAS_DEPTH {
        if !ty.args.is_empty() {
            return None;
        }
        let name = ty.name.name.as_str();
        let vocabulary = match symbols.imported.get(name) {
            Some(module) => module == RFC_MODULE,
            None => own_module,
        };
        if (name == "Requirement" || name == "Spec") && vocabulary && symbols.records.contains(name)
        {
            return Some(if name == "Spec" {
                "Spec"
            } else {
                "Requirement"
            });
        }
        let target = symbols.alias_targets.get(name)?;
        if let Some(module) = symbols.imported.get(name) {
            symbols = modules.get(module)?;
            own_module = module == RFC_MODULE;
        }
        ty = target;
    }
    None
}

/// What is not literal data in a constant's value, at its span.
type Found = Option<(Span, String)>;

/// What literal data may name in one module: the variants of the
/// `std.rfc` enums in scope and the module's own `Requirement` constants.
struct Literals<'a> {
    variants: BTreeMap<&'a str, &'a Vec<String>>,
    requirements: BTreeSet<&'a str>,
}

impl Literals<'_> {
    /// The first part of `value` that is not a literal `record { .. }`
    /// with literal fields; the `requirements` field of a `Spec` is a list
    /// literal of names of `Requirement` constants of the module.
    fn record(&self, value: &Expr, record: &str) -> Found {
        match &value.kind {
            ExprKind::Record { name, fields } if name.name == record => {
                fields.iter().find_map(|field| {
                    if record == "Spec" && field.name.name == "requirements" {
                        self.requirement_list(&field.value)
                    } else {
                        self.scalar(&field.value)
                    }
                })
            }
            _ => Some((value.span, describe(value))),
        }
    }

    fn requirement_list(&self, value: &Expr) -> Found {
        let ExprKind::List(items) = &value.kind else {
            return Some((value.span, describe(value)));
        };
        items.iter().find_map(|item| match &item.kind {
            ExprKind::Name(name) if self.requirements.contains(name.name.as_str()) => None,
            ExprKind::Name(name) => Some((
                item.span,
                format!(
                    "`{}`, which is no `Requirement` constant of this module",
                    name.name
                ),
            )),
            _ => Some((item.span, describe(item))),
        })
    }

    /// The first part of a field value that is not a scalar literal: an
    /// Int literal (also negated), a Text literal or a `+` chain of them,
    /// a variant of a `std.rfc` enum, bare or qualified, `None` or
    /// `Some(` Int literal `)`.
    fn scalar(&self, value: &Expr) -> Found {
        match &value.kind {
            ExprKind::Int(_) | ExprKind::Str(_) => None,
            _ if is_negative_int(value) => None,
            ExprKind::Binary {
                op: BinaryOp::Add, ..
            } => text_chain(value),
            ExprKind::Name(name)
                if name.name == "None"
                    || self.variants.values().any(|v| v.contains(&name.name)) =>
            {
                None
            }
            ExprKind::Field { receiver, name }
                if matches!(&receiver.kind, ExprKind::Name(owner)
                    if self.variants.get(owner.name.as_str()).is_some_and(|v| v.contains(&name.name))) =>
            {
                None
            }
            ExprKind::Call { callee, args }
                if matches!(&callee.kind, ExprKind::Name(n) if n.name == "Some")
                    && args.len() == 1 =>
            {
                let arg = &args[0];
                if matches!(arg.kind, ExprKind::Int(_)) || is_negative_int(arg) {
                    None
                } else {
                    Some((arg.span, describe(arg)))
                }
            }
            _ => Some((value.span, describe(value))),
        }
    }
}

fn is_negative_int(value: &Expr) -> bool {
    matches!(&value.kind, ExprKind::Unary { op: UnaryOp::Neg, operand }
        if matches!(operand.kind, ExprKind::Int(_)))
}

/// The first part of a `+` chain that is not a Text literal.
fn text_chain(value: &Expr) -> Found {
    match &value.kind {
        ExprKind::Str(_) => None,
        ExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } if !is_template(left) && !is_template(right) => {
            text_chain(left).or_else(|| text_chain(right))
        }
        _ => Some((value.span, describe(value))),
    }
}

/// An interpolated piece of a text template: the parser lowers `{e}` to
/// `e.to_text()` with the method name at the span of `e`, which no
/// written method call has.
fn is_template(piece: &Expr) -> bool {
    matches!(&piece.kind, ExprKind::Method { receiver, name, args }
        if name.name == "to_text" && args.is_empty() && name.span == receiver.span)
}

/// What a non-literal expression is, for E091's message.
fn describe(value: &Expr) -> String {
    let what = match &value.kind {
        _ if is_template(value) => "an interpolated text",
        ExprKind::Call { .. } => "a call",
        ExprKind::Method { .. } => "a method call",
        ExprKind::Name(name) => return format!("the name `{}`", name.name),
        ExprKind::List(_) => "a list",
        ExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } if is_template(left) || is_template(right) => "an interpolated text",
        ExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } if matches!(left.kind, ExprKind::List(_) | ExprKind::Binary { .. })
            || matches!(right.kind, ExprKind::List(_)) =>
        {
            "a list built with `+`"
        }
        ExprKind::Binary { .. } | ExprKind::Unary { .. } => "an operator",
        ExprKind::If { .. } => "an `if`",
        ExprKind::Match { .. } => "a `match`",
        ExprKind::Block(_) => "a block",
        ExprKind::Brace { .. } => "a brace literal without its type name",
        ExprKind::Record { name, .. } => return format!("a literal of `{}`", name.name),
        ExprKind::RecordUpdate { .. } => "a record update",
        ExprKind::Field { .. } => "a field access",
        ExprKind::Quantity { .. } => "a quantity",
        ExprKind::Bool(_) => "a Bool literal",
        ExprKind::Closure(_) => "a closure",
        _ => "an expression",
    };
    what.to_string()
}
