//! Slice 10 (`docs/slice.md` section 14): quantities, named arguments,
//! brace literals, maps, references, `module("p")`, component kinds,
//! executables, contracts, policies, assemblies and failure sites.

use std::collections::{BTreeMap, BTreeSet};

use ignite_syntax::ast::{
    AssemblyDecl, BinaryOp, BraceEntry, ComponentDecl, Expr, ExprKind, FieldInit, Ident, Path,
    ResourceDecl, Span, SystemDecl,
};

use super::{Typer, compatible, exposure, fn_ty};
use crate::{
    AssemblyInfo, BraceKind, Builtin, Callee, ExecutableInfo, FailureOp, FailureSite,
    MemberDeclaration, MemberInfo, ModuleRecord, PropertySource, QualName, QuantityKind, Reference,
    SystemInfo, Ty, key,
};

/// One member of a `system` (`docs/slice.md` section 14.4).
#[derive(Clone, Debug)]
pub(super) struct Member {
    pub name: String,
    pub kind: MemberKind,
    /// The kind as written: `TcpListener`, `Store`, `TelnetServer`.
    pub kind_name: String,
    /// `TcpListener`, `Store<T>` or the kind's record type. `None` for an
    /// unknown kind, and for a generic kind until its declaration has been
    /// typed.
    pub ty: Option<Ty>,
    /// The span of the member's name.
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MemberKind {
    Resource,
    Component,
    Contract,
    Policy,
}

impl MemberKind {
    fn word(self) -> &'static str {
        self.declaration().as_str()
    }

    fn declaration(self) -> MemberDeclaration {
        match self {
            MemberKind::Resource => MemberDeclaration::Resource,
            MemberKind::Component => MemberDeclaration::Component,
            MemberKind::Contract => MemberDeclaration::Contract,
            MemberKind::Policy => MemberDeclaration::Policy,
        }
    }
}

/// The properties of a declaration with the source of each value, and its
/// unset `Signal<T>` outputs with their element types (`docs/slice.md`
/// section 15.9).
type Properties = Vec<(String, PropertySource)>;
type Outputs = Vec<(String, Ty)>;

/// The compiler-known properties of a `TcpListener` resource
/// (`docs/slice.md` section 14.4) in field order, with whether each has a
/// default.
const TCP_LISTENER_FIELDS: [(&str, bool); 4] = [
    ("address", false),
    ("port", false),
    ("exposure", true),
    ("backlog", true),
];

/// What a list of `field = value` entries builds, for the diagnostics of
/// [`Typer::fields_literal`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Literal {
    /// `Name { .. }`.
    Record,
    /// `{ .. }` with an expected record type.
    Brace,
    /// The properties of a component, contract, policy or assembly: a
    /// missing field is E104 and an output field cannot be set.
    Declaration,
}

/// The failure operation of an arithmetic operator (`docs/slice.md`
/// section 14.8).
pub(super) fn failure_op(op: BinaryOp) -> FailureOp {
    match op {
        BinaryOp::Add => FailureOp::Add,
        BinaryOp::Sub => FailureOp::Sub,
        BinaryOp::Mul => FailureOp::Mul,
        BinaryOp::Div => FailureOp::Div,
        _ => FailureOp::Rem,
    }
}

/// Whether `address` is a loopback address, for the `exposure = Loopback`
/// rule of `docs/slice.md` section 14.4.
fn is_loopback(address: &str) -> bool {
    address == "localhost" || address == "::1" || address.starts_with("127.")
}

/// The members of `start`'s use graph in an order where each comes after
/// the components it uses, `start` excluded; `None` when `start` uses
/// itself.
fn embedded_order(
    start: &str,
    edges: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<String>, Vec<String>> {
    fn visit(
        node: &str,
        edges: &BTreeMap<String, Vec<String>>,
        done: &mut Vec<String>,
        path: &mut Vec<String>,
    ) -> Result<(), Vec<String>> {
        if let Some(at) = path.iter().position(|p| p == node) {
            let mut cycle = path[at..].to_vec();
            cycle.push(node.to_string());
            return Err(cycle);
        }
        if done.iter().any(|d| d == node) {
            return Ok(());
        }
        path.push(node.to_string());
        for next in edges.get(node).into_iter().flatten() {
            visit(next, edges, done, path)?;
        }
        path.pop();
        done.push(node.to_string());
        Ok(())
    }
    let mut done = Vec::new();
    visit(start, edges, &mut done, &mut Vec::new())?;
    done.retain(|d| d != start);
    Ok(done)
}

impl Typer<'_> {
    /// Record where `expr` reports its failure (`docs/slice.md` section
    /// 14.8).
    pub(super) fn fail_site(&mut self, expr: &Expr, op: FailureOp, offset: usize) {
        self.system
            .failures
            .insert(key(expr), FailureSite { op, offset });
    }

    // ---- quantities (14.1) --------------------------------------------------

    /// A quantity literal (`docs/slice.md` section 14.1): its type from
    /// the unit, its value in base units recorded; E107 when the value
    /// does not fit in an `Int`, has more than 4 decimals or is not a
    /// whole number of base units.
    pub(super) fn quantity(&mut self, expr: &Expr, literal: &str, unit: &Ident) -> Option<Ty> {
        let Some((kind, factor)) = ignite_syntax::ast::quantity_unit(&unit.name) else {
            self.error("E101", unit.span, format!("unknown unit `{}`", unit.name));
            return None;
        };
        let kind = QuantityKind::from_name(kind)?;
        let (whole, fraction) = literal.split_once('.').unwrap_or((literal, ""));
        let value = if fraction.len() > 4 {
            None
        } else {
            let scale = 10i128.pow(fraction.len() as u32);
            let digits = format!("{whole}{fraction}");
            digits
                .parse::<i128>()
                .ok()
                .map(|n| n * i128::from(factor))
                .filter(|n| n % scale == 0)
                .map(|n| n / scale)
                .and_then(|n| i64::try_from(n).ok())
        };
        let Some(value) = value else {
            let reason = if fraction.len() > 4 {
                "it has more than 4 decimals".to_string()
            } else {
                format!(
                    "it is not a whole number of {} or does not fit in an `Int`",
                    match kind {
                        QuantityKind::Duration => "nanoseconds",
                        QuantityKind::ByteSize => "bytes",
                        QuantityKind::Ratio => "parts per million",
                    }
                )
            };
            let unit_text = if unit.name == "%" {
                "%".to_string()
            } else {
                format!(" {}", unit.name)
            };
            self.error(
                "E107",
                expr.span,
                format!(
                    "quantity literal `{literal}{unit_text}` is out of range for `{}`: {reason}",
                    kind.name()
                ),
            );
            return Some(Ty::Quantity(kind));
        };
        self.system.quantities.insert(key(expr), (kind, value));
        Some(Ty::Quantity(kind))
    }

    /// `left op right` with a quantity operand (`docs/slice.md` section
    /// 14.1): `+`, `-` and comparisons take two values of one quantity
    /// type, `*` a quantity and an `Int` either way round, `/` a quantity
    /// by an `Int`; anything else is E101, `%` E045.
    pub(super) fn quantity_operator(
        &mut self,
        expr: &Expr,
        op: BinaryOp,
        left: &Ty,
        right: &Ty,
    ) -> Option<Ty> {
        use BinaryOp::*;
        let kind = |t: &Ty| match t {
            Ty::Quantity(kind) => Some(*kind),
            _ => None,
        };
        let never = |t: &Ty| *t == Ty::Never;
        let int = |t: &Ty| *t == Ty::Int || never(t);
        let (lk, rk) = (kind(left), kind(right));
        let same = lk == rk || never(left) || never(right);
        let quantity = lk.or(rk).map(Ty::Quantity);
        let result = match op {
            Eq | Ne | Lt | Gt | Le | Ge => same.then_some(Ty::Bool),
            Add | Sub => same.then(|| quantity.clone()).flatten(),
            Mul => match (lk, rk) {
                (Some(k), None) if int(right) => Some(Ty::Quantity(k)),
                (None, Some(k)) if int(left) => Some(Ty::Quantity(k)),
                _ => None,
            },
            Div => match (lk, rk) {
                (Some(k), None) if int(right) => Some(Ty::Quantity(k)),
                _ => None,
            },
            Rem | And | Or => None,
        };
        let comparison = matches!(op, Eq | Ne | Lt | Gt | Le | Ge);
        match result {
            Some(ty) => {
                if !comparison {
                    self.fail_site(expr, failure_op(op), expr.span.start);
                }
                Some(ty)
            }
            None => {
                if op == Rem {
                    let shown = quantity.unwrap_or(Ty::Int);
                    self.error(
                        "E045",
                        expr.span,
                        format!("operator `%` is not defined for `{shown}`"),
                    );
                } else {
                    self.error(
                        "E101",
                        expr.span,
                        format!(
                            "operator `{}` mixes `{left}` and `{right}`: quantity arithmetic takes two values of one quantity type (`+`, `-`, comparisons) or a quantity and an `Int` (`*`, `/`)",
                            op.symbol()
                        ),
                    );
                }
                comparison.then_some(Ty::Bool)
            }
        }
    }

    // ---- named arguments (14.2) ---------------------------------------------

    /// The arguments of a call of the fn `q` in parameter order
    /// (`docs/slice.md` section 14.2). `skip` parameters are covered
    /// before `args` (a UFCS receiver). With a named argument the mapping
    /// is recorded for [`crate::TypedProgram::named_args`]; an unknown
    /// name, a repeated parameter or a positional argument after a named
    /// one is E103, too many arguments E043, and then every argument is
    /// typed here and the result is `None`.
    pub(super) fn ordered_args<'e>(
        &mut self,
        expr: &Expr,
        q: &QualName,
        skip: usize,
        args: &'e [Expr],
    ) -> Option<Vec<&'e Expr>> {
        if !args
            .iter()
            .any(|a| matches!(a.kind, ExprKind::NamedArg { .. }))
        {
            return Some(args.iter().collect());
        }
        let names: Vec<String> = self
            .module_types(&q.module)
            .and_then(|m| m.param_names.get(&q.name).cloned())
            .unwrap_or_default()
            .into_iter()
            .skip(skip)
            .collect();
        let what = q.name.clone();
        let mut slots: Vec<Option<(usize, &Expr)>> = vec![None; names.len()];
        let mut named_seen = false;
        let mut extra: Vec<&Expr> = Vec::new();
        let mut bad = false;
        for (i, arg) in args.iter().enumerate() {
            match &arg.kind {
                ExprKind::NamedArg { name, .. } => {
                    named_seen = true;
                    self.named_ok.insert(key(arg));
                    match names.iter().position(|n| *n == name.name) {
                        None => {
                            let listed = names
                                .iter()
                                .map(|n| format!("`{n}`"))
                                .collect::<Vec<_>>()
                                .join(", ");
                            self.error(
                                "E103",
                                name.span,
                                format!(
                                    "`{what}` has no parameter `{}`; its parameters are {listed}",
                                    name.name
                                ),
                            );
                            bad = true;
                            extra.push(arg);
                        }
                        Some(p) if slots[p].is_some() => {
                            self.error(
                                "E103",
                                name.span,
                                format!("argument `{}` of `{what}` is given twice", name.name),
                            );
                            bad = true;
                            extra.push(arg);
                        }
                        Some(p) => slots[p] = Some((i, arg)),
                    }
                }
                _ if named_seen => {
                    self.error(
                        "E103",
                        arg.span,
                        format!(
                            "positional argument after a named argument in the call of `{what}`; named arguments come last"
                        ),
                    );
                    bad = true;
                    extra.push(arg);
                }
                _ if i < names.len() => slots[i] = Some((i, arg)),
                _ => extra.push(arg),
            }
        }
        if !bad && !extra.is_empty() {
            self.error(
                "E043",
                expr.span,
                format!(
                    "wrong number of arguments to `{what}`: expected {}, found {}",
                    names.len(),
                    args.len()
                ),
            );
            bad = true;
        }
        if bad {
            for arg in slots.iter().flatten().map(|(_, a)| *a).chain(extra) {
                self.synth(arg);
            }
            return None;
        }
        let ordered: Vec<&Expr> = slots.iter().flatten().map(|(_, a)| *a).collect();
        if ordered.len() == names.len() {
            let mapping = slots.iter().flatten().map(|(i, _)| *i).collect();
            self.system.named.insert(key(expr), mapping);
        }
        Some(ordered)
    }

    // ---- brace literals and maps (14.3) ------------------------------------

    /// E106 unless values of `key` can be `Map` keys: `Int`, `Byte`,
    /// `Text`, `Bool`, an enum without payloads or `Ref`.
    pub(super) fn map_key(&mut self, key: &Ty, span: Span) -> bool {
        let orderable = match key {
            Ty::Int | Ty::Byte | Ty::Text | Ty::Bool | Ty::Ref(_) | Ty::Never => true,
            Ty::Enum(_) | Ty::GenericEnum(..) => self.variants(key).is_some_and(|(_, variants)| {
                variants.iter().all(|(_, payload)| payload.is_empty())
            }),
            _ => false,
        };
        if !orderable {
            self.error(
                "E106",
                span,
                format!(
                    "`{key}` cannot be a `Map` key; keys are `Int`, `Byte`, `Text`, `Bool`, an enum without payloads or `Ref`"
                ),
            );
        }
        orderable
    }

    /// A brace literal (`docs/slice.md` section 14.3): a record literal of
    /// the expected record type, a map literal of the expected `Map`;
    /// E100 without an expected type.
    pub(super) fn brace(
        &mut self,
        expr: &Expr,
        entries: &[BraceEntry],
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        let Some(expected) = expected else {
            for entry in entries {
                self.synth(&entry.value);
            }
            self.error(
                "E100",
                expr.span,
                "a brace literal `{ .. }` needs an expected type (a record or a `Map`); write the type name before `{` or annotate the binding".to_string(),
            );
            return None;
        };
        match expected {
            Ty::Record(q) | Ty::GenericRecord(q, _) => {
                let q = q.clone();
                let mut pairs: Vec<(&Ident, &Expr)> = Vec::new();
                for entry in entries {
                    match entry.key.segments.as_slice() {
                        [name] => pairs.push((name, &entry.value)),
                        _ => {
                            self.error(
                                "E047",
                                entry.key.span,
                                format!(
                                    "record `{}` has no field `{}`",
                                    q.name,
                                    entry.key.dotted()
                                ),
                            );
                            self.synth(&entry.value);
                        }
                    }
                }
                self.system
                    .braces
                    .insert(key(expr), BraceKind::Record(expected.clone()));
                let shown = q.name.clone();
                self.fields_literal(
                    expr.span,
                    Some(expr),
                    &q,
                    &shown,
                    &pairs,
                    Some(expected),
                    Literal::Brace,
                )
            }
            Ty::Map(k, v) => self.map_literal(expr, entries, k, v, expected),
            other => {
                for entry in entries {
                    self.synth(&entry.value);
                }
                self.error(
                    "E040",
                    expr.span,
                    format!(
                        "type mismatch: expected `{other}`, found a brace literal `{{ .. }}`, which is a record or a `Map`"
                    ),
                );
                None
            }
        }
    }

    /// `{ k = v, .. }` where a `Map<k, v>` is expected: an identifier key
    /// is a `Text`, a qualified key `System.member` a `Ref` of the member.
    fn map_literal(
        &mut self,
        expr: &Expr,
        entries: &[BraceEntry],
        k: &Ty,
        v: &Ty,
        expected: &Ty,
    ) -> Option<Ty> {
        let mut seen: Vec<String> = Vec::new();
        for entry in entries {
            let shown = entry.key.dotted();
            if seen.contains(&shown) {
                self.error(
                    "E047",
                    entry.key.span,
                    format!("key `{shown}` is given twice in the map literal"),
                );
            }
            seen.push(shown.clone());
            match entry.key.segments.as_slice() {
                [name] => {
                    if !compatible(&Ty::Text, k) {
                        self.error(
                            "E040",
                            name.span,
                            format!(
                                "type mismatch: the key `{}` is a `Text`, the map's keys are `{k}`",
                                name.name
                            ),
                        );
                    }
                }
                [system, member] => {
                    if let Some(found) = self.brace_reference(&entry.key, system, member)
                        && !compatible(&found, k)
                    {
                        self.mismatch(entry.key.span, k, &found);
                    }
                }
                _ => self.error(
                    "E108",
                    entry.key.span,
                    format!(
                        "`{shown}` is not a map key: a key is a name or a reference `System.member`"
                    ),
                ),
            }
            self.check(&entry.value, v);
        }
        self.system
            .braces
            .insert(key(expr), BraceKind::Map(k.clone(), v.clone()));
        Some(expected.clone())
    }

    /// The reference a qualified map key `System.member` denotes, recorded
    /// for [`crate::TypedProgram::brace_key`]; E108 when the system or the
    /// member does not exist, or when the key is not a member of the
    /// system an assembly names (`docs/slice.md` section 14.7).
    fn brace_reference(&mut self, path: &Path, system: &Ident, member: &Ident) -> Option<Ty> {
        let shown = path.dotted();
        if let Some(named) = self.assembly_system.clone()
            && !named.is_empty()
            && named != system.name
        {
            self.error(
                "E108",
                path.span,
                format!(
                    "`{shown}` is not a member of system `{named}`, the system of this assembly"
                ),
            );
            return None;
        }
        let Some(members) = self.system_members.get(&system.name) else {
            self.error(
                "E108",
                path.span,
                format!("unknown system `{}` in key `{shown}`", system.name),
            );
            return None;
        };
        let Some(found) = members.iter().find(|m| m.name == member.name) else {
            self.error(
                "E108",
                path.span,
                format!("`{shown}` is not a member of system `{}`", system.name),
            );
            return None;
        };
        let ty = Ty::Ref(found.ty.clone().map(Box::new));
        self.system.references.insert(
            path as *const Path as usize,
            Reference::Member {
                system: system.name.clone(),
                member: member.name.clone(),
            },
        );
        Some(ty)
    }

    /// Whether `value` is `member.field` for a member of the system being
    /// walked and a `Signal` field of its kind: the one value an output
    /// field of a declaration (a contract's `signal`) may be set to.
    fn is_signal_reference(&self, value: &Expr) -> bool {
        let Some(system) = &self.current_system else {
            return false;
        };
        let ExprKind::Field { receiver, name } = &value.kind else {
            return false;
        };
        let ExprKind::Name(member) = &receiver.kind else {
            return false;
        };
        let Some(ty) = self
            .system_members
            .get(system)
            .and_then(|members| members.iter().find(|m| m.name == member.name))
            .and_then(|m| m.ty.clone())
        else {
            return false;
        };
        matches!(self.field_type(&ty, &name.name), Some(Ty::Signal(_)))
    }

    /// The fields of a record literal, brace literal or declaration of the
    /// record `q` (shown as `shown`): E047 for a repeated or unknown field
    /// and, in a literal, a missing one without default; in a declaration a
    /// missing field is E104 and so is a `Signal` field given a value other
    /// than another member's signal (`docs/slice.md` section 14.4). A
    /// generic record's type arguments are inferred from the fields and
    /// `expected`, recorded on `record_expr`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fields_literal(
        &mut self,
        span: Span,
        record_expr: Option<&Expr>,
        q: &QualName,
        shown: &str,
        entries: &[(&Ident, &Expr)],
        expected: Option<&Ty>,
        literal: Literal,
    ) -> Option<Ty> {
        let declaration = literal == Literal::Declaration;
        let declared = self.record_fields(q).unwrap_or_default();
        let tparams = self.generics_of(q);
        let mut pairs: Vec<(&Expr, Ty)> = Vec::new();
        for (i, (name, value)) in entries.iter().enumerate() {
            if entries[..i].iter().any(|(n, _)| n.name == name.name) {
                self.error(
                    "E047",
                    name.span,
                    format!("field `{}` is given twice in `{shown}` literal", name.name),
                );
            }
            match declared.iter().find(|(n, _)| *n == name.name) {
                Some((_, ty))
                    if declaration
                        && matches!(ty, Ty::Signal(_))
                        && !self.is_signal_reference(value) =>
                {
                    self.error(
                        "E104",
                        name.span,
                        format!("output field `{}` of `{shown}` cannot be set", name.name),
                    );
                    self.synth(value);
                }
                Some((_, ty)) if !tparams.is_empty() => pairs.push((value, ty.clone())),
                Some((_, ty)) => {
                    self.check(value, ty);
                }
                None => {
                    self.error(
                        "E047",
                        name.span,
                        format!("record `{shown}` has no field `{}`", name.name),
                    );
                    self.synth(value);
                }
            }
        }
        let defaults = self
            .module_types(&q.module)
            .and_then(|m| m.defaults.get(&q.name).cloned())
            .unwrap_or_default();
        for (field, ty) in &declared {
            let given = entries.iter().any(|(n, _)| n.name == *field);
            if given || defaults.contains(field) {
                continue;
            }
            if declaration {
                if matches!(ty, Ty::Signal(_)) || (field == "executable" && *ty == Ty::Text) {
                    continue;
                }
                self.error(
                    "E104",
                    span,
                    format!("required field `{field}` of `{shown}` is missing"),
                );
            } else {
                self.error(
                    "E047",
                    span,
                    format!("missing field `{field}` in `{shown}` literal; it has no default"),
                );
            }
        }
        if tparams.is_empty() {
            return Some(Ty::Record(q.clone()));
        }
        let generic = Ty::GenericRecord(
            q.clone(),
            tparams.iter().map(|p| Ty::Param(p.clone())).collect(),
        );
        let instance = self.infer(span, shown, &tparams, &pairs, &[], &generic, expected)?;
        if let Some(expr) = record_expr {
            self.instances.insert(key(expr), instance.clone());
        }
        Some(Ty::GenericRecord(q.clone(), instance))
    }

    // ---- references and module records (14.4) ------------------------------

    /// A bare member name of the system being walked (`docs/slice.md`
    /// section 14.4, O2): a reference where `Ref` is expected, the
    /// `Endpoint` of a `TcpListener` where `Endpoint` is expected (14.14
    /// item 5), else the member's value, recorded as a use of the member.
    /// `None` when `name` is no member of `system`.
    pub(super) fn member_value(
        &mut self,
        expr: &Expr,
        system: &str,
        name: &Ident,
        expected: Option<&Ty>,
    ) -> Option<Option<Ty>> {
        let member = self
            .system_members
            .get(system)?
            .iter()
            .find(|m| m.name == name.name)?
            .clone();
        if let Some(Ty::Ref(_)) = expected {
            self.system.references.insert(
                key(expr),
                Reference::Member {
                    system: system.to_string(),
                    member: name.name.clone(),
                },
            );
            return Some(Some(Ty::Ref(member.ty.map(Box::new))));
        }
        let Some(ty) = member.ty else {
            if member.kind == MemberKind::Component && self.records.contains_key(&member.kind_name)
            {
                self.error(
                    "E040",
                    name.span,
                    format!(
                        "the type of `{}` is not known yet: a component of a generic kind must be declared before the members that use it",
                        name.name
                    ),
                );
            }
            return Some(None);
        };
        if let Some(endpoint @ Ty::Record(q)) = expected
            && q.module == "std.net"
            && q.name == "Endpoint"
            && ty == Ty::TcpListener
        {
            self.use_member(&name.name, endpoint.clone());
            return Some(Some(endpoint.clone()));
        }
        self.use_member(&name.name, ty.clone());
        Some(Some(ty))
    }

    fn use_member(&mut self, name: &str, ty: Ty) {
        if let Some(uses) = &mut self.member_uses
            && !uses.iter().any(|(n, _)| n == name)
        {
            uses.push((name.to_string(), ty));
        }
    }

    /// `System.member`: a `Ref` of the member (E041 when the system has no
    /// such member).
    pub(super) fn member_reference(
        &mut self,
        expr: &Expr,
        system: &Ident,
        member: &Ident,
    ) -> Option<Ty> {
        let found = self
            .system_members
            .get(&system.name)
            .and_then(|members| members.iter().find(|m| m.name == member.name))
            .cloned();
        let Some(found) = found else {
            self.error(
                "E041",
                member.span,
                format!("system `{}` has no member `{}`", system.name, member.name),
            );
            return None;
        };
        self.system.references.insert(
            key(expr),
            Reference::Member {
                system: system.name.clone(),
                member: member.name.clone(),
            },
        );
        Some(Ty::Ref(found.ty.map(Box::new)))
    }

    /// `member.field` inside a system where `field` is a `Signal<T>` of the
    /// member's kind (`docs/slice.md` section 14.4, O3): the signal value,
    /// recorded as a reference; not a use of the member. `None` when the
    /// expression is something else.
    pub(super) fn member_signal(
        &mut self,
        expr: &Expr,
        receiver: &Expr,
        field: &Ident,
    ) -> Option<Option<Ty>> {
        let system = self.current_system.clone()?;
        let ExprKind::Name(member) = &receiver.kind else {
            return None;
        };
        let ty = self
            .system_members
            .get(&system)?
            .iter()
            .find(|m| m.name == member.name)?
            .ty
            .clone()?;
        let signal = self.field_type(&ty, &field.name)?;
        if !matches!(signal, Ty::Signal(_)) {
            return None;
        }
        self.record(receiver, Some(ty));
        self.system.references.insert(
            key(expr),
            Reference::Signal {
                system,
                member: member.name.clone(),
                field: field.name.clone(),
            },
        );
        Some(Some(signal))
    }

    /// `module("p")` (`docs/slice.md` section 14.4, O4): a record of the
    /// expected type `I` (or `Option<I>`) whose fn-typed fields are filled
    /// by name from the public fns of module `p`, a standard-library module
    /// or a sibling of the program's package. A module that is not found,
    /// a missing fn or one of another type is E102.
    pub(super) fn module_call(
        &mut self,
        expr: &Expr,
        callee: &Expr,
        args: &[Expr],
        expected: Option<&Ty>,
    ) -> Option<Ty> {
        self.callees
            .insert(key(expr), Callee::Builtin(Builtin::ModuleRecord));
        let mut path = None;
        for arg in args {
            match &arg.kind {
                ExprKind::Str(text) if args.len() == 1 => {
                    path = Some(text.clone());
                    self.record(arg, Some(Ty::Text));
                }
                _ => {
                    self.synth(arg);
                }
            }
        }
        let Some(path) = path else {
            self.error(
                "E102",
                expr.span,
                "`module(..)` takes one string literal naming a module, e.g. `module(\"examples.terminal.commands\")`".to_string(),
            );
            return None;
        };
        let (record, optional) = match expected {
            Some(ty @ (Ty::Record(_) | Ty::GenericRecord(..))) => (ty.clone(), false),
            Some(Ty::Option(inner)) if matches!(**inner, Ty::Record(_) | Ty::GenericRecord(..)) => {
                ((**inner).clone(), true)
            }
            Some(other) => {
                self.error(
                    "E040",
                    expr.span,
                    format!(
                        "type mismatch: expected `{other}`, found `module(..)`, a record of fns (`Module<I>`)"
                    ),
                );
                return None;
            }
            None => {
                self.error(
                    "E042",
                    expr.span,
                    "cannot infer the record type of `module(..)`; it takes it from the expected type `Module<I>`".to_string(),
                );
                return None;
            }
        };
        let (q, targs) = match &record {
            Ty::Record(q) => (q.clone(), Vec::new()),
            Ty::GenericRecord(q, args) => (q.clone(), args.clone()),
            _ => return None,
        };
        self.record(callee, Some(fn_ty(vec![Ty::Text], record.clone())));
        let Some(types) = self.module_types(&path).cloned() else {
            self.error(
                "E102",
                args[0].span,
                format!(
                    "module `{path}` not found; `module(..)` names a standard-library module or a sibling file `<name>.ignite` of the program's package"
                ),
            );
            return None;
        };
        let shown = q.name.clone();
        let fields = self.record_fields(&q).unwrap_or_default();
        let tparams = self.generics_of(&q);
        let mut fns = Vec::new();
        let mut ok = true;
        for (field, field_ty) in fields {
            let field_ty = field_ty.substitute(&tparams, &targs);
            let (params, ret, effectful_ok) = match &field_ty {
                Ty::Fn(params, ret) => (params, ret, false),
                Ty::EffectFn(params, ret, _) => (params, ret, true),
                other => {
                    self.error(
                        "E102",
                        expr.span,
                        format!(
                            "field `{field}` of `{shown}` has type `{other}`; `module(..)` fills only fields of fn type"
                        ),
                    );
                    ok = false;
                    continue;
                }
            };
            let Some(sig) = types.fns.get(&field) else {
                self.error(
                    "E102",
                    expr.span,
                    format!("module `{path}` has no fn `{field}` for field `{field}` of `{shown}`"),
                );
                ok = false;
                continue;
            };
            let generic = types.generics.contains_key(&field);
            let fits = !generic
                && sig.params == *params
                && sig.ret == **ret
                && (effectful_ok || !sig.effectful);
            if !fits {
                let found = fn_ty(sig.params.clone(), sig.ret.clone());
                let detail = if sig.effectful { " (effectful)" } else { "" };
                self.error(
                    "E102",
                    expr.span,
                    format!(
                        "fn `{field}` of module `{path}` has type `{found}`{detail}, field `{field}` of `{shown}` needs `{field_ty}`"
                    ),
                );
                ok = false;
                continue;
            }
            fns.push((field.clone(), QualName::new(path.clone(), field)));
        }
        if !ok {
            return None;
        }
        self.system.module_records.insert(
            key(expr),
            ModuleRecord {
                module: path,
                record: record.clone(),
                fns,
            },
        );
        Some(if optional {
            Ty::Option(Box::new(record))
        } else {
            record
        })
    }

    // ---- systems (14.4, 14.5, 14.7) -----------------------------------------

    /// The members of `system` with the types that are known before its
    /// declarations are typed, for references from anywhere in the module.
    pub(super) fn collect_members(&mut self, system: &SystemDecl) {
        let mut members = Vec::new();
        for resource in &system.resources {
            let ty = match resource.kind.name.as_str() {
                "TcpListener" => Some(Ty::TcpListener),
                "Store" => match resource.args.as_slice() {
                    [item] => Some(Ty::Store(Box::new(self.resolve(item)))),
                    args => {
                        self.error(
                            "E048",
                            resource.kind.span,
                            format!("`Store` takes 1 type argument, found {}", args.len()),
                        );
                        None
                    }
                },
                _ => None,
            };
            members.push(Member {
                name: resource.name.name.clone(),
                kind: MemberKind::Resource,
                kind_name: resource.kind.name.clone(),
                ty,
                span: resource.name.span,
            });
        }
        let lists = [
            (&system.components, MemberKind::Component),
            (&system.contracts, MemberKind::Contract),
            (&system.policies, MemberKind::Policy),
        ];
        for (list, kind) in lists {
            for decl in list.iter() {
                let ty = self
                    .records
                    .get(decl.kind.name.as_str())
                    .cloned()
                    .filter(|q| self.generics_of(q).is_empty())
                    .map(Ty::Record);
                members.push(Member {
                    name: decl.name.name.clone(),
                    kind,
                    kind_name: decl.kind.name.clone(),
                    ty,
                    span: decl.name.span,
                });
            }
        }
        self.system_members
            .insert(system.name.name.clone(), members);
    }

    /// Where each property of a declaration of the record kind `ty` with
    /// the properties `given` takes its value, and the `Signal<T>` outputs
    /// it leaves unset (`docs/slice.md` section 15.9), in the kind's field
    /// order. A required field that is missing (E104) appears in neither.
    fn property_sources(&self, ty: &Ty, given: &[FieldInit]) -> (Properties, Outputs) {
        let (q, args) = match ty {
            Ty::Record(q) => (q, Vec::new()),
            Ty::GenericRecord(q, args) => (q, args.clone()),
            _ => return (Vec::new(), Vec::new()),
        };
        let params = self.generics_of(q);
        let defaults = self
            .module_types(&q.module)
            .and_then(|m| m.defaults.get(&q.name).cloned())
            .unwrap_or_default();
        let mut properties = Vec::new();
        let mut outputs = Vec::new();
        for (field, field_ty) in self.record_fields(q).unwrap_or_default() {
            let field_ty = field_ty.substitute(&params, &args);
            if let Some(i) = given.iter().position(|p| p.name.name == field) {
                properties.push((field, PropertySource::Set(i)));
            } else if let Ty::Signal(element) = &field_ty {
                outputs.push((field, (**element).clone()));
            } else if defaults.contains(&field) {
                let default = QualName::new(q.module.clone(), format!("{}.{field}", q.name));
                properties.push((field, PropertySource::Default(default)));
            } else if field == "executable" && field_ty == Ty::Text {
                properties.push((field, PropertySource::ComponentName));
            }
        }
        (properties, outputs)
    }

    /// The [`MemberInfo`] of member `index` of `kind` in `system`, whose
    /// typed kind is `ty`; `None` for a kind that did not type (E020).
    #[allow(clippy::too_many_arguments)]
    fn member_info(
        &self,
        system: &str,
        name: &str,
        kind: MemberKind,
        index: usize,
        ty: Option<Ty>,
        properties: &[FieldInit],
        offset: usize,
    ) -> Option<MemberInfo> {
        let ty = ty?;
        let (properties, outputs) = match &ty {
            Ty::TcpListener => {
                let sources = TCP_LISTENER_FIELDS
                    .iter()
                    .filter_map(|(field, defaulted)| {
                        let source = match properties.iter().position(|p| p.name.name == *field) {
                            Some(i) => PropertySource::Set(i),
                            None if *defaulted => PropertySource::Default(QualName::builtin(
                                format!("TcpListener.{field}"),
                            )),
                            None => return None,
                        };
                        Some((field.to_string(), source))
                    })
                    .collect();
                (sources, Vec::new())
            }
            Ty::Store(_) => (Vec::new(), Vec::new()),
            other => self.property_sources(other, properties),
        };
        Some(MemberInfo {
            id: format!("{system}.{name}"),
            name: name.to_string(),
            declaration: kind.declaration(),
            index,
            kind: ty,
            properties,
            outputs,
            offset,
        })
    }

    /// A `system` declaration (`docs/slice.md` sections 14.4, 14.5 and
    /// 14.7): its resources, then its components, contracts and policies
    /// as declarations of their kinds; the components whose kinds have a
    /// `main` become executables; a cycle of bare-name uses among
    /// components is E105. `decl` is the declaration's index in the
    /// module, for [`SystemInfo`].
    pub(super) fn system(&mut self, decl: usize, system: &SystemDecl) {
        let name = system.name.name.clone();
        if self.module == "std" || self.module.starts_with("std.") {
            self.error(
                "E020",
                system.name.span,
                "a `system` cannot be declared in a standard-library module".to_string(),
            );
        }
        self.current_system = Some(name.clone());
        self.scopes = vec![BTreeMap::new()];
        self.ret = None;
        let members = self.system_members.get(&name).cloned().unwrap_or_default();
        for member in &members {
            let ident = Ident {
                name: member.name.clone(),
                span: member.span,
            };
            self.bind(&ident, member.ty.clone());
        }
        for resource in &system.resources {
            self.resource(resource);
        }
        let mut uses: BTreeMap<String, Vec<(String, Ty)>> = BTreeMap::new();
        let mut typed: Vec<(&ComponentDecl, Ty)> = Vec::new();
        let lists = [
            (&system.components, MemberKind::Component),
            (&system.contracts, MemberKind::Contract),
            (&system.policies, MemberKind::Policy),
        ];
        for (list, kind) in lists {
            for decl in list.iter() {
                let (ty, used) = self.member_decl(&name, decl, kind);
                uses.insert(decl.name.name.clone(), used);
                if let (Some(ty), MemberKind::Component) = (ty, kind) {
                    typed.push((decl, ty));
                }
            }
        }
        let members = self.system_members.get(&name).cloned().unwrap_or_default();
        let component_edges: BTreeMap<String, Vec<String>> = uses
            .iter()
            .map(|(component, used)| {
                let components = used
                    .iter()
                    .filter(|(n, _)| {
                        members
                            .iter()
                            .any(|m| m.name == *n && m.kind == MemberKind::Component)
                    })
                    .map(|(n, _)| n.clone())
                    .collect();
                (component.clone(), components)
            })
            .collect();
        let mut reported: BTreeSet<String> = BTreeSet::new();
        let mut executables: Vec<(String, Span)> = Vec::new();
        for (decl, ty) in typed {
            let order = match embedded_order(&decl.name.name, &component_edges) {
                Ok(order) => order,
                Err(cycle) => {
                    if !reported.contains(&decl.name.name) {
                        reported.extend(cycle.iter().cloned());
                        self.error(
                            "E105",
                            decl.name.span,
                            format!(
                                "components use each other through bare names: {}; a component embeds the values it names, so the chain must end",
                                cycle.join(" -> ")
                            ),
                        );
                    }
                    continue;
                }
            };
            if let Some(executable) = self.executable(&name, decl, &ty, &order, &uses, &members) {
                if let Some((other, _)) = executables.iter().find(|(e, _)| *e == executable) {
                    self.error(
                        "E020",
                        decl.name.span,
                        format!(
                            "two components produce the executable `{other}`; give each its own `executable`"
                        ),
                    );
                }
                executables.push((executable, decl.name.span));
            }
        }
        let mut infos = Vec::new();
        for (index, resource) in system.resources.iter().enumerate() {
            let ty = members
                .iter()
                .find(|m| m.name == resource.name.name)
                .and_then(|m| m.ty.clone());
            infos.extend(self.member_info(
                &name,
                &resource.name.name,
                MemberKind::Resource,
                index,
                ty,
                &resource.properties,
                resource.span.start,
            ));
        }
        for (list, kind) in lists {
            for (index, decl) in list.iter().enumerate() {
                let ty = members
                    .iter()
                    .find(|m| m.name == decl.name.name)
                    .and_then(|m| m.ty.clone());
                infos.extend(self.member_info(
                    &name,
                    &decl.name.name,
                    kind,
                    index,
                    ty,
                    &decl.properties,
                    decl.span.start,
                ));
            }
        }
        self.system.systems.push(SystemInfo {
            name,
            decl,
            members: infos,
            offset: system.span.start,
        });
        self.current_system = None;
        self.scopes.clear();
    }

    /// A resource declaration: `TcpListener` with `address`, `port`,
    /// `exposure` and `backlog` (the last two defaulted), or `Store<T>`
    /// with no properties.
    fn resource(&mut self, resource: &ResourceDecl) {
        match resource.kind.name.as_str() {
            "TcpListener" => {
                let known = [
                    ("address", Ty::Text),
                    ("port", Ty::Int),
                    ("exposure", exposure()),
                    ("backlog", Ty::Int),
                ];
                let mut seen: Vec<&str> = Vec::new();
                let mut address: Option<&Expr> = None;
                let mut loopback = false;
                for property in &resource.properties {
                    let name = property.name.name.as_str();
                    if seen.contains(&name) {
                        self.error(
                            "E047",
                            property.name.span,
                            format!(
                                "property `{name}` is given twice in resource `{}`",
                                resource.name.name
                            ),
                        );
                    }
                    seen.push(name);
                    match known.iter().find(|(n, _)| *n == name) {
                        Some((_, ty)) => {
                            self.check(&property.value, ty);
                            self.property_effects(
                                &property.value,
                                name,
                                &format!("resource `{}`", resource.name.name),
                            );
                            if name == "address" {
                                address = Some(&property.value);
                            }
                            if name == "exposure" {
                                loopback = match &property.value.kind {
                                    ExprKind::Name(id) => id.name == "Loopback",
                                    ExprKind::Field { receiver, name } => {
                                        name.name == "Loopback"
                                            && matches!(&receiver.kind, ExprKind::Name(id) if id.name == "Exposure")
                                    }
                                    _ => false,
                                };
                            }
                        }
                        None => {
                            self.error(
                                "E047",
                                property.name.span,
                                format!(
                                    "`TcpListener` has no property `{name}`; its properties are `address`, `port`, `exposure` and `backlog`"
                                ),
                            );
                            self.synth(&property.value);
                        }
                    }
                }
                for required in ["address", "port"] {
                    if !seen.contains(&required) {
                        self.error(
                            "E104",
                            resource.span,
                            format!("required field `{required}` of `TcpListener` is missing"),
                        );
                    }
                }
                if loopback
                    && let Some(address) = address
                    && let ExprKind::Str(text) = &address.kind
                    && !is_loopback(text)
                {
                    self.error(
                        "E104",
                        address.span,
                        format!("exposure Loopback needs a loopback address, got {text}"),
                    );
                }
            }
            "Store" => {
                for property in &resource.properties {
                    self.error(
                        "E047",
                        property.name.span,
                        format!(
                            "`Store` has no property `{}`; a store takes no properties",
                            property.name.name
                        ),
                    );
                    self.synth(&property.value);
                }
            }
            other => {
                for property in &resource.properties {
                    self.synth(&property.value);
                }
                self.error(
                    "E020",
                    resource.kind.span,
                    format!(
                        "unsupported resource kind `{other}`; expected `TcpListener` or `Store`"
                    ),
                );
            }
        }
    }

    /// A component, contract or policy declaration: a declaration literal
    /// of its kind, a record type (E020 for anything else). Returns the
    /// record type and the members the properties use through bare names.
    fn member_decl(
        &mut self,
        system: &str,
        decl: &ComponentDecl,
        kind: MemberKind,
    ) -> (Option<Ty>, Vec<(String, Ty)>) {
        let kind_name = decl.kind.name.as_str();
        let Some(q) = self.records.get(kind_name).cloned() else {
            for property in &decl.properties {
                self.synth(&property.value);
            }
            self.error(
                "E020",
                decl.kind.span,
                format!(
                    "unsupported {} kind `{kind_name}`: a kind is a record type declared in the standard library or the program",
                    kind.word()
                ),
            );
            return (None, Vec::new());
        };
        self.member_uses = Some(Vec::new());
        let entries: Vec<(&Ident, &Expr)> = decl
            .properties
            .iter()
            .map(|p| (&p.name, &p.value))
            .collect();
        let ty = self.fields_literal(
            decl.span,
            None,
            &q,
            kind_name,
            &entries,
            None,
            Literal::Declaration,
        );
        let uses = self.member_uses.take().unwrap_or_default();
        let owner = format!("{} `{}`", kind.word(), decl.name.name);
        for property in &decl.properties {
            self.property_effects(&property.value, &property.name.name, &owner);
        }
        if let Some(ty) = &ty {
            if let Some(member) = self
                .system_members
                .get_mut(system)
                .and_then(|members| members.iter_mut().find(|m| m.name == decl.name.name))
            {
                member.ty = Some(ty.clone());
            }
            self.bind(&decl.name, Some(ty.clone()));
        }
        (ty, uses)
    }

    /// The [`ExecutableInfo`] of a component whose kind has `K.main`
    /// (`docs/slice.md` section 14.5), with the executable's name; `None`
    /// for a component without one. A `main` of another signature is E109.
    fn executable(
        &mut self,
        system: &str,
        decl: &ComponentDecl,
        ty: &Ty,
        embedded: &[String],
        uses: &BTreeMap<String, Vec<(String, Ty)>>,
        members: &[Member],
    ) -> Option<String> {
        let (q, args) = match ty {
            Ty::Record(q) => (q.clone(), Vec::new()),
            Ty::GenericRecord(q, args) => (q.clone(), args.clone()),
            _ => return None,
        };
        let main_name = format!("{}.main", q.name);
        let sig = self
            .module_types(&q.module)
            .and_then(|m| m.fns.get(&main_name).cloned())?;
        let main = QualName::new(q.module.clone(), main_name.clone());
        let tparams = self.generics_of(&q);
        let main_generics = self.generics_of(&main);
        let main_args: Vec<Ty> = match sig.params.first() {
            Some(Ty::GenericRecord(owner, params)) if *owner == q && params.len() == args.len() => {
                main_generics
                    .iter()
                    .map(|p| {
                        params
                            .iter()
                            .zip(&args)
                            .find(|(param, _)| matches!(param, Ty::Param(name) if name == p))
                            .map_or(Ty::Param(p.clone()), |(_, arg)| arg.clone())
                    })
                    .collect()
            }
            _ => main_generics.iter().map(|p| Ty::Param(p.clone())).collect(),
        };
        let fits = sig.params.len() == 1
            && sig.params[0]
                .substitute(&main_generics, &main_args)
                .substitute(&tparams, &args)
                == *ty
            && sig.ret == Ty::Result(Box::new(Ty::Unit), Box::new(Ty::IoError));
        if !fits {
            let found = fn_ty(sig.params.clone(), sig.ret.clone());
            self.error(
                "E109",
                decl.kind.span,
                format!(
                    "`{main_name}` must have the signature `fn {main_name}(self: {ty}) -> Result<Unit, IoError>`, found `{found}`"
                ),
            );
            return None;
        }
        let executable = decl
            .properties
            .iter()
            .find(|p| p.name.name == "executable")
            .and_then(|p| match &p.value.kind {
                ExprKind::Str(text) => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_else(|| decl.name.name.clone());
        let mut resources: Vec<(String, Ty)> = Vec::new();
        let chain = embedded.iter().chain(std::iter::once(&decl.name.name));
        for component in chain {
            for (used, ty) in uses.get(component).into_iter().flatten() {
                let is_resource = members
                    .iter()
                    .any(|m| m.name == *used && m.kind == MemberKind::Resource);
                if is_resource && !resources.iter().any(|(n, _)| n == used) {
                    resources.push((used.clone(), ty.clone()));
                }
            }
        }
        let embedded: Vec<(String, Ty)> = embedded
            .iter()
            .filter_map(|name| {
                let member = members.iter().find(|m| m.name == *name)?;
                Some((name.clone(), member.ty.clone()?))
            })
            .collect();
        self.system.executables.push(ExecutableInfo {
            system: system.to_string(),
            component: decl.name.name.clone(),
            kind: ty.clone(),
            main,
            main_args,
            executable: executable.clone(),
            resources,
            embedded,
        });
        Some(executable)
    }

    /// An `assembly` declaration (`docs/slice.md` section 14.7): a
    /// declaration literal of its kind; the system its `system` property
    /// names is the one every `bindings` key must belong to (E108). `decl`
    /// is the declaration's index in the module, for [`AssemblyInfo`].
    pub(super) fn assembly(&mut self, decl: usize, assembly: &AssemblyDecl) {
        self.scopes = vec![BTreeMap::new()];
        self.ret = None;
        let system = assembly
            .properties
            .iter()
            .find(|p| p.name.name == "system")
            .and_then(|p| match &p.value.kind {
                ExprKind::Name(name) if self.system_members.contains_key(&name.name) => {
                    Some(name.name.clone())
                }
                _ => None,
            });
        self.assembly_system = Some(system.clone().unwrap_or_default());
        let kind_name = assembly.kind.name.as_str();
        match self.records.get(kind_name).cloned() {
            Some(q) => {
                let entries: Vec<(&Ident, &Expr)> = assembly
                    .properties
                    .iter()
                    .map(|p| (&p.name, &p.value))
                    .collect();
                let ty = self.fields_literal(
                    assembly.span,
                    None,
                    &q,
                    kind_name,
                    &entries,
                    None,
                    Literal::Declaration,
                );
                let owner = format!("assembly `{}`", assembly.name.name);
                for property in &assembly.properties {
                    self.property_effects(&property.value, &property.name.name, &owner);
                }
                if let Some(ty) = ty {
                    let (properties, _) = self.property_sources(&ty, &assembly.properties);
                    self.system.assemblies.push(AssemblyInfo {
                        name: assembly.name.name.clone(),
                        decl,
                        kind: ty,
                        system,
                        properties,
                        offset: assembly.span.start,
                    });
                }
            }
            None => {
                for property in &assembly.properties {
                    self.synth(&property.value);
                }
                self.error(
                    "E020",
                    assembly.kind.span,
                    format!(
                        "unsupported assembly kind `{kind_name}`: a kind is a record type declared in the standard library or the program"
                    ),
                );
            }
        }
        self.assembly_system = None;
        self.scopes.clear();
    }
}
