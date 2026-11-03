import sys

path = sys.argv[1]
text = open(path, encoding="utf-8").read()


def replace(old, new):
    global text
    if text.count(old) != 1:
        sys.exit(f"expected one of: {old[:70]!r} (found {text.count(old)})")
    text = text.replace(old, new)


replace(
    "    Protocol(ProtocolDecl),\n    Const(ConstDecl),\n}\n",
    "    Protocol(ProtocolDecl),\n    Const(ConstDecl),\n    Alias(AliasDecl),\n}\n",
)
replace(
    "            Decl::Const(d) => &d.name,\n",
    "            Decl::Const(d) => &d.name,\n            Decl::Alias(d) => &d.name,\n",
)
replace(
    "            Decl::Const(d) => d.span,\n",
    "            Decl::Const(d) => d.span,\n            Decl::Alias(d) => d.span,\n",
)
replace(
    "/// `protocol Name {",
    "/// `type Name<T, U> = Type` (`docs/slice.md` section 4.6): a name for\n"
    "/// another type; `params` are the alias's type parameters.\n"
    "#[derive(Clone, Debug, PartialEq, Eq)]\n"
    "pub struct AliasDecl {\n"
    "    pub name: Ident,\n"
    "    pub params: Vec<Ident>,\n"
    "    pub ty: Type,\n"
    "    pub span: Span,\n"
    "}\n\n"
    "/// `protocol Name {",
)
replace(
    "pub struct Field {\n    pub name: Ident,\n    pub ty: Type,\n    pub span: Span,\n}",
    "pub struct Field {\n    pub name: Ident,\n    pub ty: Type,\n"
    "    /// `field: Type = value`: used when a record literal omits the field.\n"
    "    pub default: Option<Expr>,\n    pub span: Span,\n}",
)
replace(
    "    pub fn is_fn(&self) -> bool {\n        self.name.name == \"fn\"\n    }\n",
    "    pub fn is_fn(&self) -> bool {\n        self.name.name == \"fn\"\n    }\n\n"
    "    /// A tuple type `(A, B)` is `Type { name: \"Tuple\", args: [A, B] }`;\n"
    "    /// `()` is `Unit`, never an empty tuple.\n"
    "    pub fn is_tuple(&self) -> bool {\n        self.name.name == \"Tuple\"\n    }\n",
)
replace(
    "    /// `place = e`\n",
    "    /// `let (a, b) = e`, `let _ = e`, `_ = e`: an irrefutable pattern\n"
    "    /// (binding, wildcard or tuple of those).\n"
    "    LetPattern {\n        pattern: Pattern,\n        value: Expr,\n        mutable: bool,\n    },\n"
    "    /// `place = e`\n",
)
replace(
    "    List(Vec<Expr>),\n    /// `callee(args)`.",
    "    List(Vec<Expr>),\n"
    "    /// `(a, b)`; at least two elements. `()` is `Tuple(vec![])`, the Unit value.\n"
    "    Tuple(Vec<Expr>),\n"
    "    /// `return e` in expression position (a match arm, an `if` branch); type Never.\n"
    "    Return(Option<Box<Expr>>),\n"
    "    /// `callee(args)`.",
)
replace(
    "    /// `await e`\n",
    "    /// `await e`: no longer produced by the parser (slice 6); kept so the\n"
    "    /// public AST does not lose a variant.\n",
)
replace(
    "    Wildcard,\n    Int(i64),\n    Str(String),\n",
    "    Wildcard,\n    Int(i64),\n    Str(String),\n    Bool(bool),\n"
    "    /// `p | q`\n    Or(Vec<Pattern>),\n"
    "    /// `(p, q)`\n    Tuple(Vec<Pattern>),\n"
    "    /// `[p, q]`: a list of exactly that length.\n    List(Vec<Pattern>),\n",
)
replace(
    "    /// `Variant`, `Variant(p, q)`, or `Enum.Variant(...)`: an uppercase\n"
    "    /// identifier, with an optional qualifying path and payload patterns.\n",
    "    /// `Variant`, `Variant(p, q)`, or `Enum.Variant(...)`: an uppercase\n"
    "    /// identifier, with an optional qualifying path and payload patterns.\n"
    "    /// A path without payload that names a constant is a constant pattern;\n"
    "    /// the type checker resolves it (`TypedProgram::pattern_const`).\n",
)
open(path, "w", encoding="utf-8").write(text)
print("ok")
