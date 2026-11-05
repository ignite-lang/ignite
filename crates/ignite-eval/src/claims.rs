//! The claim model (`docs/slice.md` 19.7, 19.13): for each protocol of the
//! checked module that `conforms`, one row per requirement of each spec it
//! names, with scope, status, satisfiers, verifying tests and limits; and
//! the diagnostics of the claim, E090, E092 and E091's second trigger
//! (19.4). The catalogue constants are evaluated by the [`Evaluator`];
//! the clause names are read from the `TypedProgram` resolutions (19.6).
//!
//! The enums [`Level`], [`Party`] and [`Source`] mirror the std vocabulary
//! `std.rfc` by variant name (`docs/layering.md`).

use std::collections::{BTreeMap, HashMap};

use ignite_syntax::ast::{Decl, Expr, ExprKind, Ident, Path, Program, Span, StatusKind};
use ignite_syntax::diagnostic::Diagnostic;
use ignite_types::{QualName, QuantityKind, Ty, TypedProgram};

use crate::model::eval_diagnostic;
use crate::{Evaluator, Module, Value};

/// The module of the clause vocabulary (19.3).
const RFC_MODULE: &str = "std.rfc";

/// The claims of the checked module's protocols, in declaration order.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Claims {
    pub protocols: Vec<ProtocolClaim>,
}

/// One protocol that `conforms`: its specs in `conforms` order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolClaim {
    pub protocol: String,
    pub specs: Vec<SpecClaim>,
}

/// One spec of a protocol's claim, with the protocol's `scope` and one row
/// per requirement in the spec's `requirements` order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecClaim {
    /// The name of the `Spec` constant, e.g. `RFC9112`.
    pub constant: String,
    pub rfc: i64,
    pub title: String,
    pub url: String,
    pub digest: String,
    /// `None` when `scope` has no `parties` (every party is in scope).
    pub parties: Option<Vec<Party>>,
    /// `None` when `scope` has no `sections` (every section is in scope).
    pub sections: Option<Vec<String>>,
    pub rows: Vec<Row>,
    /// The text id of a specification that is no RFC, e.g. `line-echo-1`;
    /// empty for an RFC (19.19).
    pub id: String,
}

impl SpecClaim {
    /// The name the matrix and E092 give the spec: its text id, or
    /// `rfc<n>` when it has none (19.10, 19.19).
    pub fn label(&self) -> String {
        label(&self.id, self.rfc)
    }
}

/// [`SpecClaim::label`] of `id` and `rfc`.
fn label(id: &str, rfc: i64) -> String {
    if id.is_empty() {
        format!("rfc{rfc}")
    } else {
        id.to_string()
    }
}

/// One requirement of a claimed spec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The requirement's constant name, e.g. `R7_1_2`.
    pub id: String,
    pub section: String,
    pub ordinal: i64,
    pub level: Level,
    pub party: Party,
    pub source: Source,
    pub minimum: Option<i64>,
    pub text: String,
    pub in_scope: bool,
    pub status: Status,
    /// The full names (`<module>/<name>`) of the tests that verify it, in
    /// declaration order.
    pub tests: Vec<String>,
}

/// A requirement's status in one protocol. With E090 for two statuses the
/// model is not returned, so a row holds at most one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    None,
    /// By one or more declarations, in declaration order.
    Satisfied(Vec<Satisfier>),
    Unsupported(String),
    NotApplicable(String),
}

/// A declaration that satisfies a requirement: `f`, `Owner.f`, `P.role`,
/// `Name` or `Record.field`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Satisfier {
    pub declaration: String,
    /// For a field satisfying a requirement with a `minimum`.
    pub limit: Option<Limit>,
}

/// A field's configured value against a requirement's `minimum` (19.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limit {
    /// The field's default as an Int (an `Int`, or a `ByteSize` in bytes);
    /// `None` for a field without a default or of another type.
    pub configured: Option<i64>,
    pub minimum: i64,
    /// `configured >= minimum`; `None` without a configured value.
    pub meets: Option<bool>,
}

/// `std.rfc.Level`: the requirement keywords of BCP 14.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Level {
    Must,
    MustNot,
    Required,
    Shall,
    ShallNot,
    Should,
    ShouldNot,
    Recommended,
    NotRecommended,
    May,
    Optional,
}

/// The level classes of 19.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LevelClass {
    Must,
    Should,
    May,
}

impl Level {
    pub fn class(self) -> LevelClass {
        match self {
            Level::Must | Level::MustNot | Level::Required | Level::Shall | Level::ShallNot => {
                LevelClass::Must
            }
            Level::Should | Level::ShouldNot | Level::Recommended | Level::NotRecommended => {
                LevelClass::Should
            }
            Level::May | Level::Optional => LevelClass::May,
        }
    }

    /// The keyword as BCP 14 writes it: `MUST`, `MUST NOT`, `REQUIRED`, ….
    pub fn keyword(self) -> &'static str {
        match self {
            Level::Must => "MUST",
            Level::MustNot => "MUST NOT",
            Level::Required => "REQUIRED",
            Level::Shall => "SHALL",
            Level::ShallNot => "SHALL NOT",
            Level::Should => "SHOULD",
            Level::ShouldNot => "SHOULD NOT",
            Level::Recommended => "RECOMMENDED",
            Level::NotRecommended => "NOT RECOMMENDED",
            Level::May => "MAY",
            Level::Optional => "OPTIONAL",
        }
    }

    fn from_variant(variant: &str) -> Option<Level> {
        Some(match variant {
            "Must" => Level::Must,
            "MustNot" => Level::MustNot,
            "Required" => Level::Required,
            "Shall" => Level::Shall,
            "ShallNot" => Level::ShallNot,
            "Should" => Level::Should,
            "ShouldNot" => Level::ShouldNot,
            "Recommended" => Level::Recommended,
            "NotRecommended" => Level::NotRecommended,
            "May" => Level::May,
            "Optional" => Level::Optional,
            _ => return None,
        })
    }
}

/// `std.rfc.Party`: whom a requirement addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Party {
    Any,
    Sender,
    Recipient,
    Client,
    Server,
    Proxy,
}

impl Party {
    /// The variant name, as `scope { parties = [..] }` writes it.
    pub fn name(self) -> &'static str {
        match self {
            Party::Any => "Any",
            Party::Sender => "Sender",
            Party::Recipient => "Recipient",
            Party::Client => "Client",
            Party::Server => "Server",
            Party::Proxy => "Proxy",
        }
    }

    fn from_variant(variant: &str) -> Option<Party> {
        Some(match variant {
            "Any" => Party::Any,
            "Sender" => Party::Sender,
            "Recipient" => Party::Recipient,
            "Client" => Party::Client,
            "Server" => Party::Server,
            "Proxy" => Party::Proxy,
            _ => return None,
        })
    }
}

/// `std.rfc.Source`: a BCP 14 mark or prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    Bcp14,
    Prose,
}

impl Source {
    fn from_variant(variant: &str) -> Option<Source> {
        match variant {
            "Bcp14" => Some(Source::Bcp14),
            "Prose" => Some(Source::Prose),
            _ => None,
        }
    }
}

impl Claims {
    /// The claim model of `module`'s protocols and the diagnostics of
    /// 19.7: E090 (a MUST-class requirement in scope with no status, two
    /// statuses), E092 (a clause naming a requirement of a spec outside
    /// the claim) and E091's second trigger for the `Spec` and
    /// `Requirement` constants `module` declares (19.4).
    pub fn of(
        evaluator: &mut Evaluator<'_>,
        module: Module<'_>,
    ) -> Result<Claims, Vec<Diagnostic>> {
        if !has_claim_material(module.program, module.types) {
            return Ok(Claims::default());
        }
        evaluator.with_stack(|evaluator| Builder::new(evaluator, module).run())
    }
}

/// Whether `program` declares anything the claim model reads.
fn has_claim_material(program: &Program, types: &TypedProgram) -> bool {
    program.decls.iter().any(|decl| match decl {
        Decl::Protocol(p) => {
            !p.conforms.is_empty()
                || !p.statuses.is_empty()
                || p.fns.iter().any(|f| !f.satisfies.is_empty())
        }
        Decl::Fn(f) => !f.satisfies.is_empty(),
        Decl::Record(r) => {
            !r.satisfies.is_empty() || r.fields.iter().any(|f| !f.satisfies.is_empty())
        }
        Decl::Test(t) => !t.verifies.is_empty(),
        Decl::Const(c) => vocabulary(types.try_type_of(&c.value)).is_some(),
        Decl::Enum(_) | Decl::System(_) | Decl::Alias(_) | Decl::Assembly(_) => false,
    })
}

/// `Requirement` or `Spec` when `ty` is that record of `std.rfc`.
fn vocabulary(ty: Option<&Ty>) -> Option<&'static str> {
    match ty {
        Some(Ty::Record(q)) if q.module == RFC_MODULE && q.name == "Requirement" => {
            Some("Requirement")
        }
        Some(Ty::Record(q)) if q.module == RFC_MODULE && q.name == "Spec" => Some("Spec"),
        _ => None,
    }
}

/// A requirement constant's value.
#[derive(Clone, Debug)]
struct Requirement {
    id: String,
    rfc: i64,
    section: String,
    ordinal: i64,
    level: Level,
    party: Party,
    source: Source,
    minimum: Option<i64>,
    text: String,
}

/// A spec constant's value, its requirements in `requirements` order.
#[derive(Clone, Debug)]
struct Spec {
    constant: String,
    rfc: i64,
    title: String,
    url: String,
    digest: String,
    requirements: Vec<Requirement>,
    id: String,
}

/// What a clause says about one requirement.
#[derive(Clone, Debug)]
enum Said {
    /// `field` is `Some(configured)` for a record field, `None` otherwise.
    Satisfied {
        declaration: String,
        field: Option<Option<i64>>,
    },
    Status {
        kind: StatusKind,
        reason: String,
    },
}

/// One clause naming a requirement: where it is and what it says.
#[derive(Clone, Debug)]
struct Clause {
    span: Span,
    said: Said,
}

struct Builder<'e, 'p, 'm> {
    evaluator: &'e mut Evaluator<'p>,
    module: Module<'m>,
    specs: HashMap<QualName, Option<Spec>>,
    diagnostics: Vec<Diagnostic>,
}

impl<'e, 'p, 'm> Builder<'e, 'p, 'm> {
    fn new(evaluator: &'e mut Evaluator<'p>, module: Module<'m>) -> Self {
        Builder {
            evaluator,
            module,
            specs: HashMap::new(),
            diagnostics: Vec::new(),
        }
    }

    fn run(mut self) -> Result<Claims, Vec<Diagnostic>> {
        self.catalogue();
        let program = self.module.program;
        let types = self.module.types;
        let protocols: Vec<_> = program
            .decls
            .iter()
            .filter_map(|decl| match decl {
                Decl::Protocol(p) => Some(p),
                _ => None,
            })
            .collect();
        let conformed: Vec<(String, Vec<QualName>)> = protocols
            .iter()
            .map(|p| {
                let specs = p
                    .conforms
                    .iter()
                    .filter_map(|entry| types.clause_constant(entry.span).cloned())
                    .collect();
                (p.name.name.clone(), specs)
            })
            .collect();
        let any_conforms = |spec: &QualName| conformed.iter().any(|(_, s)| s.contains(spec));

        // Satisfies and verifies, module-wide; E092 when no protocol
        // conforms to the reference's spec.
        let mut satisfied: BTreeMap<QualName, Vec<Clause>> = BTreeMap::new();
        let mut verified: BTreeMap<QualName, Vec<String>> = BTreeMap::new();
        let tests: HashMap<usize, &str> = types
            .tests()
            .iter()
            .map(|t| (t.decl, t.full_name.as_str()))
            .collect();
        let mut satisfies = Vec::new();
        for (index, decl) in program.decls.iter().enumerate() {
            match decl {
                Decl::Fn(f) => {
                    for path in &f.satisfies {
                        satisfies.push((path, f.qualified_name(), None));
                    }
                }
                Decl::Record(r) => {
                    for path in &r.satisfies {
                        satisfies.push((path, r.name.name.clone(), None));
                    }
                    for field in &r.fields {
                        for path in &field.satisfies {
                            let declaration = format!("{}.{}", r.name.name, field.name.name);
                            satisfies.push((path, declaration, Some(field.default.as_ref())));
                        }
                    }
                }
                Decl::Protocol(p) => {
                    for f in &p.fns {
                        for path in &f.satisfies {
                            satisfies.push((
                                path,
                                format!("{}.{}", p.name.name, f.name.name),
                                None,
                            ));
                        }
                    }
                }
                Decl::Test(t) => {
                    for path in &t.verifies {
                        let Some((spec, requirement)) = self.reference(path) else {
                            continue;
                        };
                        if !any_conforms(&spec) {
                            self.foreign(path, &spec, None);
                            continue;
                        }
                        let name = tests.get(&index).copied().unwrap_or(t.name.name.as_str());
                        let names = verified.entry(requirement).or_default();
                        if !names.iter().any(|n| n == name) {
                            names.push(name.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
        for (path, declaration, default) in satisfies {
            let Some((spec, requirement)) = self.reference(path) else {
                continue;
            };
            if !any_conforms(&spec) {
                self.foreign(path, &spec, None);
                continue;
            }
            let field = default.map(|expr| expr.and_then(|expr| self.configured(expr)));
            satisfied.entry(requirement).or_default().push(Clause {
                span: path.span,
                said: Said::Satisfied { declaration, field },
            });
        }

        let mut claims = Claims::default();
        for (protocol, (name, specs)) in protocols.iter().zip(&conformed) {
            // Statuses of this protocol; E092 for a spec it does not
            // conform to.
            let mut own: BTreeMap<QualName, Vec<Clause>> = BTreeMap::new();
            for status in &protocol.statuses {
                let Some((spec, requirement)) = self.reference(&status.requirement) else {
                    continue;
                };
                if !specs.contains(&spec) {
                    self.foreign(&status.requirement, &spec, Some(name));
                    continue;
                }
                own.entry(requirement).or_default().push(Clause {
                    span: status.span,
                    said: Said::Status {
                        kind: status.kind,
                        reason: status.reason.clone(),
                    },
                });
            }
            if protocol.conforms.is_empty() {
                continue;
            }
            let scope = protocol.scope.as_ref();
            let parties: Option<Vec<Party>> = scope.and_then(|s| s.parties.as_ref()).map(|ps| {
                ps.iter()
                    .filter_map(|p| Party::from_variant(&p.name))
                    .collect()
            });
            let sections: Option<Vec<String>> = scope
                .and_then(|s| s.sections.as_ref())
                .map(|ss| ss.iter().map(|(s, _)| s.clone()).collect());
            let mut claim = ProtocolClaim {
                protocol: name.clone(),
                specs: Vec::new(),
            };
            for entry in &protocol.conforms {
                let Some(q) = types.clause_constant(entry.span).cloned() else {
                    continue;
                };
                let Some(spec) = self.spec(&q, entry.span) else {
                    continue;
                };
                let mut rows = Vec::new();
                for requirement in &spec.requirements {
                    let rq = QualName::new(q.module.clone(), requirement.id.clone());
                    let mut clauses: Vec<Clause> = satisfied.get(&rq).cloned().unwrap_or_default();
                    clauses.extend(own.get(&rq).cloned().unwrap_or_default());
                    clauses.sort_by_key(|c| c.span.start);
                    let status = self.status(&spec, requirement, name, &clauses);
                    let in_scope = in_scope(requirement, parties.as_deref(), sections.as_deref());
                    if in_scope
                        && requirement.level.class() == LevelClass::Must
                        && clauses.is_empty()
                    {
                        self.diagnostics.push(Diagnostic::new(
                            "E090",
                            entry.span,
                            format!(
                                "requirement `{}.{}` ({}, section {}) is in scope of `{name}` \
                                 and has no status; add `satisfies`, `unsupported` or \
                                 `not_applicable`",
                                spec.constant,
                                requirement.id,
                                requirement.level.keyword(),
                                requirement.section
                            ),
                        ));
                    }
                    rows.push(Row {
                        id: requirement.id.clone(),
                        section: requirement.section.clone(),
                        ordinal: requirement.ordinal,
                        level: requirement.level,
                        party: requirement.party,
                        source: requirement.source,
                        minimum: requirement.minimum,
                        text: requirement.text.clone(),
                        in_scope,
                        status,
                        tests: verified.get(&rq).cloned().unwrap_or_default(),
                    });
                }
                claim.specs.push(SpecClaim {
                    constant: spec.constant.clone(),
                    rfc: spec.rfc,
                    title: spec.title.clone(),
                    url: spec.url.clone(),
                    digest: spec.digest.clone(),
                    parties: parties.clone(),
                    sections: sections.clone(),
                    rows,
                    id: spec.id.clone(),
                });
            }
            claims.protocols.push(claim);
        }
        if self.diagnostics.is_empty() {
            Ok(claims)
        } else {
            Err(self.diagnostics)
        }
    }

    /// The status of `requirement` in `protocol` from its clauses in source
    /// order; E090 at each clause that adds a second status.
    fn status(
        &mut self,
        spec: &Spec,
        requirement: &Requirement,
        protocol: &str,
        clauses: &[Clause],
    ) -> Status {
        // One satisfier per declaration: a declaration that names the
        // requirement twice is still one of "the declarations" (19.7).
        let mut satisfiers: Vec<Satisfier> = Vec::new();
        for satisfier in clauses.iter().filter_map(|c| match &c.said {
            Said::Satisfied { declaration, field } => Some(Satisfier {
                declaration: declaration.clone(),
                limit: requirement
                    .minimum
                    .zip(*field)
                    .map(|(minimum, configured)| Limit {
                        configured,
                        minimum,
                        meets: configured.map(|c| c >= minimum),
                    }),
            }),
            Said::Status { .. } => None,
        }) {
            if !satisfiers
                .iter()
                .any(|s| s.declaration == satisfier.declaration)
            {
                satisfiers.push(satisfier);
            }
        }
        let describe = |said: &Said| match said {
            Said::Satisfied { .. } => format!(
                "satisfied by {}",
                satisfiers
                    .iter()
                    .map(|s| format!("`{}`", s.declaration))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Said::Status { kind, .. } => kind.word().to_string(),
        };
        let mut first: Option<&Said> = None;
        let mut seen_satisfied = false;
        for clause in clauses {
            if matches!(clause.said, Said::Satisfied { .. }) {
                if seen_satisfied {
                    continue;
                }
                seen_satisfied = true;
            }
            match first {
                None => first = Some(&clause.said),
                Some(earlier) => self.diagnostics.push(Diagnostic::new(
                    "E090",
                    clause.span,
                    format!(
                        "requirement `{}.{}` has two statuses in `{protocol}`: {} and {}",
                        spec.constant,
                        requirement.id,
                        describe(earlier),
                        describe(&clause.said)
                    ),
                )),
            }
        }
        match first {
            None => Status::None,
            Some(Said::Satisfied { .. }) => Status::Satisfied(satisfiers),
            Some(Said::Status {
                kind: StatusKind::Unsupported,
                reason,
            }) => Status::Unsupported(reason.clone()),
            Some(Said::Status {
                kind: StatusKind::NotApplicable,
                reason,
            }) => Status::NotApplicable(reason.clone()),
        }
    }

    /// The spec and requirement constants a `Spec.Id` reference names.
    fn reference(&self, path: &Path) -> Option<(QualName, QualName)> {
        let types = self.module.types;
        let spec = types.clause_constant(path.segments.first()?.span)?.clone();
        let requirement = types.clause_constant(path.span)?.clone();
        Some((spec, requirement))
    }

    /// E092 at `path`: its spec is outside `protocol`'s claim, or outside
    /// every claim of the module when `protocol` is `None`.
    fn foreign(&mut self, path: &Path, spec: &QualName, protocol: Option<&String>) {
        // An RFC is `RFC <n>`; a specification that is no RFC, its id (19.19).
        let rfc = self.spec(spec, path.span).map_or_else(
            || spec.name.clone(),
            |s| {
                if s.id.is_empty() {
                    format!("RFC {}", s.rfc)
                } else {
                    s.id
                }
            },
        );
        let whom = match protocol {
            Some(p) => format!("which `{p}` does not conform to"),
            None => "which no protocol of this module conforms to".to_string(),
        };
        self.diagnostics.push(Diagnostic::new(
            "E092",
            path.span,
            format!("`{}` belongs to {rfc}, {whom}", path.dotted()),
        ));
    }

    /// The value of the `Spec` constant `q`, its requirements in
    /// `requirements` order; an evaluation failure is reported at `at`.
    fn spec(&mut self, q: &QualName, at: Span) -> Option<Spec> {
        if let Some(found) = self.specs.get(q) {
            return found.clone();
        }
        let found = self.load_spec(q, at);
        self.specs.insert(q.clone(), found.clone());
        found
    }

    fn load_spec(&mut self, q: &QualName, at: Span) -> Option<Spec> {
        let modules = self.evaluator.modules();
        let module = modules.get(&q.module)?;
        let decl = find_const(module.program, &q.name)?;
        let at = if q.module == self.module.path {
            decl.name.span
        } else {
            at
        };
        let value = match self.evaluator.evaluate(module.types, &decl.value) {
            Ok(value) => value,
            Err(error) => {
                self.diagnostics.push(eval_diagnostic(
                    &error,
                    &format!("constant `{}`", q.name),
                    at,
                ));
                return None;
            }
        };
        let names = listed(&decl.value);
        let values = match field(&value, "requirements") {
            Some(Value::List(items)) => items.clone(),
            _ => Vec::new(),
        };
        // One row per requirement (19.7): a name listed twice, which is
        // E091 in its own module, keeps its first place only.
        let mut requirements: Vec<Requirement> = Vec::new();
        for (name, value) in names.iter().zip(&values) {
            if requirements.iter().any(|r| r.id == name.name) {
                continue;
            }
            requirements.extend(requirement(&name.name, value));
        }
        Some(Spec {
            constant: q.name.clone(),
            rfc: int(&value, "rfc")?,
            title: text(&value, "title")?,
            url: text(&value, "url")?,
            digest: text(&value, "digest")?,
            requirements,
            id: text(&value, "id").unwrap_or_default(),
        })
    }

    /// A field default as an Int: an `Int`, or a `ByteSize` in bytes.
    fn configured(&mut self, expr: &Expr) -> Option<i64> {
        match self.evaluator.evaluate(self.module.types, expr) {
            Ok(Value::Int(n)) => Some(n),
            Ok(Value::Quantity(QuantityKind::ByteSize, n)) => Some(n),
            _ => None,
        }
    }

    /// E091, second trigger (19.4): the `Spec` and `Requirement` constants
    /// of the checked module, one diagnostic per constant, the first rule
    /// that applies.
    fn catalogue(&mut self) {
        let program = self.module.program;
        let types = self.module.types;
        let mut spec: Option<(&Ident, i64, Vec<&Ident>)> = None;
        let mut requirements = Vec::new();
        for decl in &program.decls {
            let Decl::Const(c) = decl else { continue };
            match vocabulary(types.try_type_of(&c.value)) {
                Some("Spec") => {
                    if let Some((first, _, _)) = &spec {
                        let message = format!(
                            "a module declares one `Spec` constant; `{}` is a second beside `{}`",
                            c.name.name, first.name
                        );
                        self.diagnostics
                            .push(Diagnostic::new("E091", c.name.span, message));
                        continue;
                    }
                    let Ok(value) = self.evaluate_own(c) else {
                        continue;
                    };
                    let rfc = int(&value, "rfc").unwrap_or_default();
                    spec = Some((&c.name, rfc, listed(&c.value)));
                }
                Some("Requirement") => requirements.push(c),
                _ => {}
            }
        }
        for c in requirements {
            let Ok(value) = self.evaluate_own(c) else {
                continue;
            };
            let Some(requirement) = requirement(&c.name.name, &value) else {
                continue;
            };
            let name = &c.name.name;
            let id = id_of(&requirement.section, requirement.ordinal);
            let message = if requirement.ordinal < 1 {
                Some(format!(
                    "requirement `{name}` has ordinal {}; an ordinal is 1 or more",
                    requirement.ordinal
                ))
            } else if *name != id {
                Some(format!(
                    "requirement `{name}` must be named `{id}`, the id of section {} and \
                     ordinal {}",
                    requirement.section, requirement.ordinal
                ))
            } else if let Some((spec_name, spec_rfc, listed)) = &spec {
                let times = listed.iter().filter(|l| l.name == *name).count();
                if times == 0 {
                    Some(format!(
                        "requirement `{name}` is not listed in the requirements of `{}`",
                        spec_name.name
                    ))
                } else if times > 1 {
                    Some(format!(
                        "requirement `{name}` is listed {times} times in the requirements of `{}`",
                        spec_name.name
                    ))
                } else if requirement.rfc != *spec_rfc {
                    Some(format!(
                        "requirement `{name}` has rfc {}, and `{}` is RFC {spec_rfc}",
                        requirement.rfc, spec_name.name
                    ))
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(message) = message {
                self.diagnostics
                    .push(Diagnostic::new("E091", c.name.span, message));
            }
        }
    }

    /// A constant of the checked module, evaluated; a failure is reported
    /// at its name.
    fn evaluate_own(&mut self, c: &ignite_syntax::ast::ConstDecl) -> Result<Value, ()> {
        match self.evaluator.evaluate(self.module.types, &c.value) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.diagnostics.push(eval_diagnostic(
                    &error,
                    &format!("constant `{}`", c.name.name),
                    c.name.span,
                ));
                Err(())
            }
        }
    }
}

/// 19.5's scope rule.
fn in_scope(
    requirement: &Requirement,
    parties: Option<&[Party]>,
    sections: Option<&[String]>,
) -> bool {
    let party = parties.is_none_or(|ps| ps.contains(&requirement.party));
    let section = sections.is_none_or(|ss| {
        ss.iter().any(|s| {
            requirement.section == *s
                || requirement
                    .section
                    .strip_prefix(s.as_str())
                    .is_some_and(|rest| rest.starts_with('.'))
        })
    });
    party && section
}

/// The id of 19.4: `R` + the section, `.` as `_` and letters upper-cased,
/// + `_` + the ordinal.
fn id_of(section: &str, ordinal: i64) -> String {
    let section: String = section
        .chars()
        .map(|c| {
            if c == '.' {
                '_'
            } else {
                c.to_ascii_uppercase()
            }
        })
        .collect();
    format!("R{section}_{ordinal}")
}

/// The names a `Spec` literal lists in `requirements`.
fn listed(value: &Expr) -> Vec<&Ident> {
    let ExprKind::Record { fields, .. } = &value.kind else {
        return Vec::new();
    };
    let Some(list) = fields.iter().find(|f| f.name.name == "requirements") else {
        return Vec::new();
    };
    let ExprKind::List(items) = &list.value.kind else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| match &item.kind {
            ExprKind::Name(name) => Some(name),
            _ => None,
        })
        .collect()
}

fn find_const<'a>(program: &'a Program, name: &str) -> Option<&'a ignite_syntax::ast::ConstDecl> {
    program.decls.iter().find_map(|decl| match decl {
        Decl::Const(c) if c.name.name == name => Some(c),
        _ => None,
    })
}

/// A `Requirement` value named `id`.
fn requirement(id: &str, value: &Value) -> Option<Requirement> {
    Some(Requirement {
        id: id.to_string(),
        rfc: int(value, "rfc")?,
        section: text(value, "section")?,
        ordinal: int(value, "ordinal")?,
        level: Level::from_variant(variant(value, "level")?)?,
        party: Party::from_variant(variant(value, "party")?)?,
        source: Source::from_variant(variant(value, "source")?)?,
        minimum: match field(value, "minimum")? {
            Value::Enum {
                variant, fields, ..
            } if variant == "Some" => match fields.as_slice() {
                [Value::Int(n)] => Some(*n),
                _ => return None,
            },
            _ => None,
        },
        text: text(value, "text")?,
    })
}

fn field<'v>(value: &'v Value, name: &str) -> Option<&'v Value> {
    match value {
        Value::Record { fields, .. } => fields.iter().find(|(f, _)| f == name).map(|(_, v)| v),
        _ => None,
    }
}

fn int(value: &Value, name: &str) -> Option<i64> {
    match field(value, name)? {
        Value::Int(n) => Some(*n),
        _ => None,
    }
}

fn text(value: &Value, name: &str) -> Option<String> {
    match field(value, name)? {
        Value::Text(t) => Some(t.clone()),
        _ => None,
    }
}

fn variant<'v>(value: &'v Value, name: &str) -> Option<&'v str> {
    match field(value, name)? {
        Value::Enum { variant, .. } => Some(variant),
        _ => None,
    }
}
