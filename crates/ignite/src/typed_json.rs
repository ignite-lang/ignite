//! `ignite emit --typed-json FILE` (`docs/slice.md` section 24): the
//! parsed and typed program as one JSON document of format
//! `ignite-typed/0.1`.
//!
//! Statements, expressions and patterns are flat tables indexed by number,
//! in pre-order, so a reader's nesting depth does not grow with the
//! program's (`std.json` refuses documents nested deeper than 64). The
//! walk reads the typer's side tables while the `TypedProgram` and the AST
//! it was computed from are alive: they are keyed by node address.

use std::collections::BTreeMap;
use std::path::Path;

use ignite_eval::Json;
use ignite_syntax::SourceFile;
use ignite_syntax::ast::{
    Block, ComponentDecl, Decl, Expr, ExprKind, FieldInit, FnDecl, Ident, Pattern, PatternKind,
    Span, Stmt, StmtKind, Type, UnaryOp,
};
use ignite_types::{Callee, FnSig, ModuleTypes, QualName, Ty, TypedProgram};

use crate::Error;

/// The value of the document's `formatVersion`. A change that removes or
/// reinterprets a key bumps it; an added key does not.
pub const FORMAT_VERSION: &str = "ignite-typed/0.1";

/// The document of the program at `file`, typed against the standard
/// library at `std` (default [`crate::default_std_path`]): compact JSON
/// followed by a newline. Diagnostics fail it as they fail `ignite check`.
pub fn emit(file: &Path, std: Option<&Path>) -> Result<String, Error> {
    let loaded = crate::load(file, std)?;
    let mut text = document(loaded.source(), loaded.program(), loaded.types()).compact();
    text.push('\n');
    Ok(text)
}

/// The document of `program`, parsed from `source` and typed as `types`.
pub fn document(
    source: &SourceFile,
    program: &ignite_syntax::Program,
    types: &TypedProgram,
) -> Json {
    let exports = types.exports();
    let mut writer = Writer {
        source,
        types,
        exports: &exports,
        stmts: Vec::new(),
        exprs: Vec::new(),
        patterns: Vec::new(),
    };
    let imports = program
        .imports
        .iter()
        .map(|import| {
            object([
                ("module", Json::Str(import.module())),
                ("path", Json::Str(import.path.dotted())),
                ("names", idents(&import.names)),
                ("glob", Json::Bool(import.is_glob())),
                ("span", writer.span(import.span)),
            ])
        })
        .collect();
    let decls = program.decls.iter().map(|d| writer.decl(d)).collect();
    object([
        ("formatVersion", Json::Str(FORMAT_VERSION.to_string())),
        ("module", Json::Str(types.module.clone())),
        (
            "package",
            program
                .package
                .as_ref()
                .map_or(Json::Null, |p| Json::Str(p.dotted())),
        ),
        ("files", Json::Array(vec![Json::Str(source.path.clone())])),
        ("imports", Json::Array(imports)),
        ("decls", Json::Array(decls)),
        ("stmts", Json::Array(writer.stmts)),
        ("exprs", Json::Array(writer.exprs)),
        ("patterns", Json::Array(writer.patterns)),
    ])
}

fn object<const N: usize>(entries: [(&str, Json); N]) -> Json {
    Json::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn text(value: &str) -> Json {
    Json::Str(value.to_string())
}

fn int(n: usize) -> Json {
    Json::Int(i64::try_from(n).unwrap_or(i64::MAX))
}

fn idents(names: &[Ident]) -> Json {
    Json::Array(names.iter().map(|n| text(&n.name)).collect())
}

fn paths(paths: &[ignite_syntax::ast::Path]) -> Json {
    Json::Array(paths.iter().map(|p| Json::Str(p.dotted())).collect())
}

fn optional<T>(value: Option<T>, f: impl FnOnce(T) -> Json) -> Json {
    value.map_or(Json::Null, f)
}

/// A name with its module: `{module, name}`; a builtin's module is `""`.
fn qualified(name: &QualName) -> [(&'static str, Json); 2] {
    [
        ("module", Json::Str(name.module.clone())),
        ("name", Json::Str(name.name.clone())),
    ]
}

/// A type: `kind` (the type constructor), `text` (Ignite syntax, records
/// and enums unqualified) and the parts of the constructor.
pub fn ty(t: &Ty) -> Json {
    let mut entries: BTreeMap<String, Json> = BTreeMap::new();
    let mut put = |key: &str, value: Json| {
        entries.insert(key.to_string(), value);
    };
    let list = |items: &[Ty]| Json::Array(items.iter().map(ty).collect());
    let kind = match t {
        Ty::Int => "Int",
        Ty::Bool => "Bool",
        Ty::Byte => "Byte",
        Ty::Unit => "Unit",
        Ty::Text => "Text",
        Ty::TcpListener => "TcpListener",
        Ty::TcpStream => "TcpStream",
        Ty::PipeStream => "PipeStream",
        Ty::IoError => "IoError",
        Ty::Input => "Input",
        Ty::Never => "Never",
        Ty::SystemRef => "SystemRef",
        Ty::Protocol => "Protocol",
        Ty::List(element) => {
            put("args", list(std::slice::from_ref(element)));
            "List"
        }
        Ty::Option(some) => {
            put("args", list(std::slice::from_ref(some)));
            "Option"
        }
        Ty::Result(ok, err) => {
            put("args", Json::Array(vec![ty(ok), ty(err)]));
            "Result"
        }
        Ty::Store(item) => {
            put("args", list(std::slice::from_ref(item)));
            "Store"
        }
        Ty::Sender(item) => {
            put("args", list(std::slice::from_ref(item)));
            "Sender"
        }
        Ty::Receiver(item) => {
            put("args", list(std::slice::from_ref(item)));
            "Receiver"
        }
        Ty::Map(k, v) => {
            put("args", Json::Array(vec![ty(k), ty(v)]));
            "Map"
        }
        Ty::Signal(item) => {
            put("args", list(std::slice::from_ref(item)));
            "Signal"
        }
        Ty::Ref(kind) => {
            put(
                "args",
                list(kind.as_deref().map_or(&[], std::slice::from_ref)),
            );
            "Ref"
        }
        Ty::Tuple(items) => {
            put("args", list(items));
            "Tuple"
        }
        Ty::Record(name) | Ty::Enum(name) => {
            for (key, value) in qualified(name) {
                put(key, value);
            }
            put("args", Json::Array(Vec::new()));
            if matches!(t, Ty::Record(_)) {
                "Record"
            } else {
                "Enum"
            }
        }
        Ty::GenericRecord(name, args) | Ty::GenericEnum(name, args) => {
            for (key, value) in qualified(name) {
                put(key, value);
            }
            put("args", list(args));
            if matches!(t, Ty::GenericRecord(..)) {
                "Record"
            } else {
                "Enum"
            }
        }
        Ty::Fn(params, ret) => {
            put("params", list(params));
            put("ret", ty(ret));
            put("requires", Json::Null);
            "Fn"
        }
        Ty::EffectFn(params, ret, effects) => {
            put("params", list(params));
            put("ret", ty(ret));
            put(
                "requires",
                Json::Array(effects.requires.iter().map(|r| text(r)).collect()),
            );
            put(
                "paramNames",
                Json::Array(effects.names.iter().map(|n| text(n)).collect()),
            );
            "Fn"
        }
        Ty::Param(name) => {
            put("name", text(name));
            "Param"
        }
        Ty::Quantity(kind) => {
            put("name", text(kind.name()));
            "Quantity"
        }
        Ty::Capability(kind) => {
            put("name", text(kind.name()));
            "Capability"
        }
    };
    entries.insert("kind".to_string(), text(kind));
    entries.insert("text".to_string(), Json::Str(t.to_string()));
    Json::Object(entries)
}

/// What a call or method call calls.
fn callee(types: &TypedProgram, callee: &Callee) -> Json {
    match callee {
        Callee::Fn(name) => {
            let [module, name] = qualified(name);
            object([("kind", text("fn")), module, name])
        }
        Callee::Const(name) => {
            let [module, name] = qualified(name);
            object([("kind", text("const")), module, name])
        }
        Callee::Role { protocol, role } => object([
            ("kind", text("role")),
            ("module", Json::Str(protocol.module.clone())),
            ("protocol", Json::Str(protocol.name.clone())),
            ("role", text(role)),
        ]),
        Callee::Variant { enum_, index } => object([
            ("kind", text("variant")),
            ("module", Json::Str(enum_.module.clone())),
            ("enum", Json::Str(enum_.name.clone())),
            ("index", int(*index)),
            (
                "variant",
                optional(
                    types
                        .enum_variants(enum_)
                        .and_then(|variants| variants.get(*index)),
                    |(name, _)| text(name),
                ),
            ),
        ]),
        Callee::Builtin(builtin) => object([
            ("kind", text("builtin")),
            ("name", Json::Str(format!("{builtin:?}"))),
        ]),
        Callee::Local => object([("kind", text("local"))]),
        Callee::FieldFn { record, field } => object([
            ("kind", text("fieldFn")),
            ("module", Json::Str(record.module.clone())),
            ("record", Json::Str(record.name.clone())),
            ("field", text(field)),
        ]),
    }
}

struct Writer<'a> {
    source: &'a SourceFile,
    types: &'a TypedProgram,
    exports: &'a ModuleTypes,
    stmts: Vec<Json>,
    exprs: Vec<Json>,
    patterns: Vec<Json>,
}

impl Writer<'_> {
    /// `{file, line, column, endLine, endColumn, start, end}`: `file` the
    /// index in `files`, lines and columns 1-based (columns in characters),
    /// `start` and `end` byte offsets.
    fn span(&self, span: Span) -> Json {
        let (line, column) = self.source.line_col(span.start);
        let end = span.end.max(span.start);
        let (end_line, end_column) = self.source.line_col(end);
        object([
            ("file", int(0)),
            ("line", int(line)),
            ("column", int(column)),
            ("endLine", int(end_line)),
            ("endColumn", int(end_column)),
            ("start", int(span.start)),
            ("end", int(end)),
        ])
    }

    /// A written type expression: the type it denotes, aliases resolved,
    /// or `fallback` when the typer kept none.
    fn written(&self, written: &Type, fallback: Option<&Ty>) -> Json {
        optional(self.types.resolved_type(written).or(fallback), ty)
    }

    fn decl(&mut self, decl: &Decl) -> Json {
        let span = self.span(decl.span());
        let mut entries = match decl {
            Decl::Fn(f) => {
                let sig = self.exports.fns.get(&f.qualified_name());
                return self.function(f, sig, "fn");
            }
            Decl::Record(r) => {
                let fields = r
                    .fields
                    .iter()
                    .map(|field| {
                        object([
                            ("name", text(&field.name.name)),
                            ("type", self.written(&field.ty, None)),
                            (
                                "default",
                                optional(field.default.as_ref(), |e| self.expr(e)),
                            ),
                            ("satisfies", paths(&field.satisfies)),
                            ("span", self.span(field.span)),
                        ])
                    })
                    .collect();
                vec![
                    ("kind", text("record")),
                    ("typeParams", idents(&r.type_params)),
                    ("fields", Json::Array(fields)),
                    ("satisfies", paths(&r.satisfies)),
                    ("testOnly", Json::Bool(r.test_only)),
                ]
            }
            Decl::Enum(e) => {
                let variants = e
                    .variants
                    .iter()
                    .map(|variant| {
                        let payload = variant
                            .payload
                            .iter()
                            .map(|t| self.written(t, None))
                            .collect();
                        object([
                            ("name", text(&variant.name.name)),
                            ("payload", Json::Array(payload)),
                            ("span", self.span(variant.span)),
                        ])
                    })
                    .collect();
                vec![
                    ("kind", text("enum")),
                    ("typeParams", idents(&e.type_params)),
                    ("variants", Json::Array(variants)),
                    ("testOnly", Json::Bool(e.test_only)),
                ]
            }
            Decl::Const(c) => vec![
                ("kind", text("const")),
                (
                    "type",
                    self.written(&c.ty, self.exports.consts.get(&c.name.name)),
                ),
                ("value", self.expr(&c.value)),
                ("testOnly", Json::Bool(c.test_only)),
            ],
            Decl::Alias(a) => vec![
                ("kind", text("alias")),
                ("typeParams", idents(&a.params)),
                (
                    "type",
                    self.written(
                        &a.ty,
                        self.exports.aliases.get(&a.name.name).map(|a| &a.target),
                    ),
                ),
            ],
            Decl::Test(t) => vec![
                ("kind", text("test")),
                ("body", self.block(&t.body)),
                ("verifies", paths(&t.verifies)),
            ],
            Decl::System(s) => {
                let resources = s
                    .resources
                    .iter()
                    .map(|r| {
                        let args = r.args.iter().map(|t| self.written(t, None)).collect();
                        object([
                            ("name", text(&r.name.name)),
                            ("kind", text(&r.kind.name)),
                            ("args", Json::Array(args)),
                            ("properties", self.properties(&r.properties)),
                            ("span", self.span(r.span)),
                        ])
                    })
                    .collect();
                vec![
                    ("kind", text("system")),
                    ("resources", Json::Array(resources)),
                    ("components", self.members(&s.components)),
                    ("contracts", self.members(&s.contracts)),
                    ("policies", self.members(&s.policies)),
                ]
            }
            Decl::Protocol(p) => {
                let roles_sigs = self.exports.protocols.get(&p.name.name);
                let roles = p
                    .fns
                    .iter()
                    .map(|f| {
                        let sig = roles_sigs.and_then(|r| r.get(&f.name.name));
                        self.function(f, sig, "role")
                    })
                    .collect();
                let scope = optional(p.scope.as_ref(), |scope| {
                    object([
                        (
                            "parties",
                            optional(scope.parties.as_ref(), |parties| idents(parties)),
                        ),
                        (
                            "sections",
                            optional(scope.sections.as_ref(), |sections| {
                                Json::Array(sections.iter().map(|(s, _)| text(s)).collect())
                            }),
                        ),
                        ("span", self.span(scope.span)),
                    ])
                });
                let statuses = p
                    .statuses
                    .iter()
                    .map(|status| {
                        object([
                            ("kind", text(status.kind.word())),
                            ("requirement", Json::Str(status.requirement.dotted())),
                            ("reason", text(&status.reason)),
                            ("span", self.span(status.span)),
                        ])
                    })
                    .collect();
                vec![
                    ("kind", text("protocol")),
                    ("roles", Json::Array(roles)),
                    ("conforms", idents(&p.conforms)),
                    ("scope", scope),
                    ("statuses", Json::Array(statuses)),
                ]
            }
            Decl::Assembly(a) => vec![
                ("kind", text("assembly")),
                ("kindName", text(&a.kind.name)),
                ("properties", self.properties(&a.properties)),
            ],
        };
        entries.push(("name", text(&decl.name().name)));
        entries.push(("span", span));
        Json::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        )
    }

    /// A fn or protocol role (`kind` `fn` or `role`).
    fn function(&mut self, f: &FnDecl, sig: Option<&FnSig>, kind: &str) -> Json {
        let params = f
            .params
            .iter()
            .enumerate()
            .map(|(i, param)| {
                object([
                    ("name", text(&param.name.name)),
                    (
                        "type",
                        self.written(&param.ty, sig.and_then(|s| s.params.get(i))),
                    ),
                    ("span", self.span(param.span)),
                ])
            })
            .collect();
        let ret = match &f.ret {
            Some(ret) => self.written(ret, sig.map(|s| &s.ret)),
            None => ty(sig.map_or(&Ty::Unit, |s| &s.ret)),
        };
        let bounds = f
            .bounds
            .iter()
            .map(|bound| {
                object([
                    ("param", text(&bound.param.name)),
                    ("interfaces", idents(&bound.interfaces)),
                ])
            })
            .collect();
        object([
            ("kind", text(kind)),
            ("name", text(&f.name.name)),
            ("qualifiedName", Json::Str(f.qualified_name())),
            ("owner", optional(f.owner.as_ref(), |o| text(&o.name))),
            ("typeParams", idents(&f.type_params)),
            ("bounds", Json::Array(bounds)),
            ("params", Json::Array(params)),
            ("ret", ret),
            (
                "requires",
                optional(f.requires.as_ref(), |requires| paths(requires)),
            ),
            (
                "effectful",
                optional(sig, |s| Json::Bool(s.effectful || f.requires.is_some())),
            ),
            ("satisfies", paths(&f.satisfies)),
            ("testOnly", Json::Bool(f.test_only)),
            ("body", self.block(&f.body)),
            ("span", self.span(f.span)),
        ])
    }

    fn members(&mut self, members: &[ComponentDecl]) -> Json {
        Json::Array(
            members
                .iter()
                .map(|m| {
                    object([
                        ("name", text(&m.name.name)),
                        ("kind", text(&m.kind.name)),
                        ("properties", self.properties(&m.properties)),
                        ("span", self.span(m.span)),
                    ])
                })
                .collect(),
        )
    }

    /// `name = value` entries: the value an expression index.
    fn properties(&mut self, fields: &[FieldInit]) -> Json {
        Json::Array(
            fields
                .iter()
                .map(|field| {
                    object([
                        ("name", text(&field.name.name)),
                        ("value", self.expr(&field.value)),
                        ("span", self.span(field.span)),
                    ])
                })
                .collect(),
        )
    }

    /// `{span, stmts}`: the statements as indexes.
    fn block(&mut self, block: &Block) -> Json {
        let stmts = block.stmts.iter().map(|s| self.stmt(s)).collect();
        object([
            ("stmts", Json::Array(stmts)),
            ("span", self.span(block.span)),
        ])
    }

    fn binding(&self, name: &Ident) -> Json {
        optional(self.types.binding_type(name.span), ty)
    }

    /// The statement's index in `stmts`, written before its children.
    fn stmt(&mut self, stmt: &Stmt) -> Json {
        let id = self.stmts.len();
        self.stmts.push(Json::Null);
        let mut entries = match &stmt.kind {
            StmtKind::Let {
                name,
                ty: declared,
                value,
                mutable,
            } => vec![
                ("kind", text("let")),
                ("name", text(&name.name)),
                ("mutable", Json::Bool(*mutable)),
                (
                    "declaredType",
                    optional(declared.as_ref(), |t| self.written(t, None)),
                ),
                ("bindingType", self.binding(name)),
                ("value", self.expr(value)),
            ],
            StmtKind::LetPattern {
                pattern,
                value,
                mutable,
            } => vec![
                ("kind", text("letPattern")),
                ("mutable", Json::Bool(*mutable)),
                ("pattern", self.pattern(pattern)),
                ("value", self.expr(value)),
            ],
            StmtKind::Assign { target, value } => vec![
                ("kind", text("assign")),
                (
                    "target",
                    object([
                        ("base", text(&target.base.name)),
                        ("fields", idents(&target.fields)),
                        ("ownerType", optional(self.types.place_owner(target), ty)),
                        ("span", self.span(target.span)),
                    ]),
                ),
                ("value", self.expr(value)),
            ],
            StmtKind::Return(value) => vec![
                ("kind", text("return")),
                ("value", optional(value.as_ref(), |e| self.expr(e))),
            ],
            StmtKind::While { cond, body } => vec![
                ("kind", text("while")),
                ("cond", self.expr(cond)),
                ("body", self.block(body)),
            ],
            StmtKind::Loop { body } => vec![("kind", text("loop")), ("body", self.block(body))],
            StmtKind::For { name, iter, body } => vec![
                ("kind", text("for")),
                ("name", text(&name.name)),
                ("bindingType", self.binding(name)),
                ("iter", self.expr(iter)),
                ("body", self.block(body)),
            ],
            StmtKind::ForPattern {
                pattern,
                iter,
                body,
            } => vec![
                ("kind", text("forPattern")),
                ("pattern", self.pattern(pattern)),
                ("iter", self.expr(iter)),
                ("body", self.block(body)),
            ],
            StmtKind::Break => vec![("kind", text("break"))],
            StmtKind::Continue => vec![("kind", text("continue"))],
            StmtKind::Expr(e) => vec![("kind", text("expr")), ("expr", self.expr(e))],
            StmtKind::Assert { cond, message } => vec![
                ("kind", text("assert")),
                ("cond", self.expr(cond)),
                ("message", optional(message.as_ref(), |e| self.expr(e))),
            ],
        };
        entries.push(("span", self.span(stmt.span)));
        self.stmts[id] = Json::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        );
        int(id)
    }

    fn exprs(&mut self, items: &[Expr]) -> Json {
        Json::Array(items.iter().map(|e| self.expr(e)).collect())
    }

    fn call_parts(&self, expr: &Expr, method: bool) -> Vec<(&'static str, Json)> {
        let resolved = self.types.callee(expr);
        vec![
            ("callee", optional(resolved, |c| callee(self.types, c))),
            (
                "ufcs",
                Json::Bool(method && matches!(resolved, Some(Callee::Fn(_)))),
            ),
            (
                "namedArgs",
                optional(self.types.named_args(expr), |order| {
                    Json::Array(order.iter().map(|&i| int(i)).collect())
                }),
            ),
        ]
    }

    /// The expression's index in `exprs`, written before its children.
    fn expr(&mut self, expr: &Expr) -> Json {
        let id = self.exprs.len();
        self.exprs.push(Json::Null);
        let mut entries: Vec<(&str, Json)> = match &expr.kind {
            ExprKind::Int(n) => vec![("kind", text("int")), ("value", Json::Int(*n))],
            ExprKind::Str(s) => vec![("kind", text("str")), ("value", text(s))],
            ExprKind::Bool(b) => vec![("kind", text("bool")), ("value", Json::Bool(*b))],
            ExprKind::Name(name) => vec![("kind", text("name")), ("name", text(&name.name))],
            ExprKind::Record { name, fields } => vec![
                ("kind", text("record")),
                ("name", text(&name.name)),
                ("fields", self.properties(fields)),
            ],
            ExprKind::List(items) => vec![("kind", text("list")), ("items", self.exprs(items))],
            ExprKind::Tuple(items) => {
                vec![("kind", text("tuple")), ("items", self.exprs(items))]
            }
            ExprKind::Return(value) => vec![
                ("kind", text("return")),
                ("value", optional(value.as_deref(), |e| self.expr(e))),
            ],
            ExprKind::Call { callee, args } => {
                let mut entries = vec![
                    ("kind", text("call")),
                    ("function", self.expr(callee)),
                    ("args", self.exprs(args)),
                ];
                entries.extend(self.call_parts(expr, false));
                entries
            }
            ExprKind::Method {
                receiver,
                name,
                args,
            } => {
                let mut entries = vec![
                    ("kind", text("method")),
                    ("receiver", self.expr(receiver)),
                    ("name", text(&name.name)),
                    ("args", self.exprs(args)),
                ];
                entries.extend(self.call_parts(expr, true));
                entries
            }
            ExprKind::Field { receiver, name } => vec![
                ("kind", text("field")),
                ("receiver", self.expr(receiver)),
                ("name", text(&name.name)),
            ],
            ExprKind::Index { receiver, index } => vec![
                ("kind", text("index")),
                ("receiver", self.expr(receiver)),
                ("index", self.expr(index)),
            ],
            ExprKind::Try(operand) => vec![("kind", text("try")), ("operand", self.expr(operand))],
            ExprKind::Await(operand) => {
                vec![("kind", text("await")), ("operand", self.expr(operand))]
            }
            ExprKind::Spawn(operand) => {
                vec![("kind", text("spawn")), ("operand", self.expr(operand))]
            }
            ExprKind::Unary { op, operand } => vec![
                ("kind", text("unary")),
                (
                    "op",
                    text(match op {
                        UnaryOp::Not => "!",
                        UnaryOp::Neg => "-",
                    }),
                ),
                ("operand", self.expr(operand)),
            ],
            ExprKind::Binary { op, left, right } => vec![
                ("kind", text("binary")),
                ("op", text(op.symbol())),
                ("left", self.expr(left)),
                ("right", self.expr(right)),
            ],
            ExprKind::Match { scrutinee, arms } => {
                let scrutinee = self.expr(scrutinee);
                let arms = arms
                    .iter()
                    .map(|arm| {
                        object([
                            ("pattern", self.pattern(&arm.pattern)),
                            ("body", self.expr(&arm.body)),
                            ("span", self.span(arm.span)),
                        ])
                    })
                    .collect();
                vec![
                    ("kind", text("match")),
                    ("scrutinee", scrutinee),
                    ("arms", Json::Array(arms)),
                ]
            }
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => vec![
                ("kind", text("if")),
                ("cond", self.expr(cond)),
                ("then", self.block(then)),
                ("else", optional(otherwise.as_deref(), |e| self.expr(e))),
            ],
            ExprKind::Block(block) => vec![("kind", text("block")), ("block", self.block(block))],
            ExprKind::Closure(closure) => {
                let info = self.types.closure(expr);
                let params = closure
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, param)| {
                        object([
                            ("name", text(&param.name.name)),
                            (
                                "declaredType",
                                optional(param.ty.as_ref(), |t| self.written(t, None)),
                            ),
                            (
                                "type",
                                optional(info.and_then(|c| c.params.get(i)), |(_, t)| ty(t)),
                            ),
                            ("span", self.span(param.span)),
                        ])
                    })
                    .collect();
                let captures = optional(info, |c| {
                    Json::Array(
                        c.captures
                            .iter()
                            .map(|capture| {
                                object([("name", text(&capture.name)), ("type", ty(&capture.ty))])
                            })
                            .collect(),
                    )
                });
                vec![
                    ("kind", text("closure")),
                    ("params", Json::Array(params)),
                    ("ret", optional(info, |c| ty(&c.ret))),
                    ("captures", captures),
                    ("body", self.expr(&closure.body)),
                ]
            }
            ExprKind::RecordUpdate { name, fields, base } => vec![
                ("kind", text("recordUpdate")),
                ("name", text(&name.name)),
                ("fields", self.properties(fields)),
                ("base", self.expr(base)),
            ],
            ExprKind::TupleField { receiver, index } => vec![
                ("kind", text("tupleField")),
                ("receiver", self.expr(receiver)),
                ("index", int(*index)),
            ],
            ExprKind::Break => vec![("kind", text("break"))],
            ExprKind::Continue => vec![("kind", text("continue"))],
            ExprKind::Quantity { literal, unit } => vec![
                ("kind", text("quantity")),
                ("literal", text(literal)),
                ("unit", text(&unit.name)),
                (
                    "quantity",
                    optional(self.types.quantity(expr), |(kind, value)| {
                        object([("kind", text(kind.name())), ("value", Json::Int(value))])
                    }),
                ),
            ],
            ExprKind::NamedArg { name, value } => vec![
                ("kind", text("namedArg")),
                ("name", text(&name.name)),
                ("value", self.expr(value)),
            ],
            ExprKind::Brace { entries } => {
                let entries = entries
                    .iter()
                    .map(|entry| {
                        object([
                            ("key", Json::Str(entry.key.dotted())),
                            ("value", self.expr(&entry.value)),
                            ("span", self.span(entry.span)),
                        ])
                    })
                    .collect();
                vec![("kind", text("brace")), ("entries", Json::Array(entries))]
            }
        };
        entries.push(("type", optional(self.types.try_type_of(expr), ty)));
        if let Some(args) = self.types.instance(expr) {
            entries.push(("typeArgs", Json::Array(args.iter().map(ty).collect())));
        }
        if let Some(reference) = self.types.reference(expr) {
            entries.push(("reference", Json::Str(reference.text())));
        }
        entries.push(("span", self.span(expr.span)));
        self.exprs[id] = Json::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        );
        int(id)
    }

    /// The pattern's index in `patterns`, written before its children.
    fn pattern(&mut self, pattern: &Pattern) -> Json {
        let id = self.patterns.len();
        self.patterns.push(Json::Null);
        let mut entries: Vec<(&str, Json)> = match &pattern.kind {
            PatternKind::Wildcard => vec![("kind", text("wildcard"))],
            PatternKind::Int(n) => vec![("kind", text("int")), ("value", Json::Int(*n))],
            PatternKind::Str(s) => vec![("kind", text("str")), ("value", text(s))],
            PatternKind::Bool(b) => vec![("kind", text("bool")), ("value", Json::Bool(*b))],
            PatternKind::Or(items) => vec![("kind", text("or")), ("items", self.patterns(items))],
            PatternKind::Tuple(items) => {
                vec![("kind", text("tuple")), ("items", self.patterns(items))]
            }
            PatternKind::List(items) => {
                vec![("kind", text("list")), ("items", self.patterns(items))]
            }
            PatternKind::Rest(None) => vec![("kind", text("rest")), ("name", Json::Null)],
            PatternKind::Rest(Some(name)) => vec![
                ("kind", text("rest")),
                ("name", text(&name.name)),
                ("bindingType", self.binding(name)),
            ],
            PatternKind::Binding(name) => vec![
                ("kind", text("binding")),
                ("name", text(&name.name)),
                ("bindingType", self.binding(name)),
            ],
            PatternKind::Variant { path, args } => {
                let constant = optional(self.types.pattern_const(pattern), |q| {
                    let [module, name] = qualified(q);
                    object([module, name])
                });
                vec![
                    ("kind", text("variant")),
                    ("path", idents(path)),
                    ("args", self.patterns(args)),
                    ("constant", constant),
                ]
            }
            PatternKind::Range { start, end } => {
                let bounds = optional(self.types.range_bounds(pattern), |(lo, hi)| {
                    Json::Array(vec![Json::Int(lo), Json::Int(hi)])
                });
                vec![
                    ("kind", text("range")),
                    ("start", self.pattern(start)),
                    ("end", self.pattern(end)),
                    ("bounds", bounds),
                ]
            }
        };
        entries.push(("span", self.span(pattern.span)));
        self.patterns[id] = Json::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        );
        int(id)
    }

    fn patterns(&mut self, items: &[Pattern]) -> Json {
        Json::Array(items.iter().map(|p| self.pattern(p)).collect())
    }
}
