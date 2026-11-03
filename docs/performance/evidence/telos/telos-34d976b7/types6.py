import sys

path = sys.argv[1]
text = open(path, encoding="utf-8").read()


def replace(old, new):
    global text
    if text.count(old) != 1:
        sys.exit(f"expected one of: {old[:70]!r} (found {text.count(old)})")
    text = text.replace(old, new)


replace(
    "    Fn(Vec<Ty>, Box<Ty>),\n    TcpListener,",
    "    Fn(Vec<Ty>, Box<Ty>),\n    /// `(A, B)`: two or more elements; `()` is `Unit`.\n    Tuple(Vec<Ty>),\n    TcpListener,",
)
replace(
    "    /// A local, parameter or pattern binding of fn type.\n    Local,\n    Const(QualName),\n}",
    "    /// A local, parameter or pattern binding of fn type.\n    Local,\n    Const(QualName),\n"
    "    /// `receiver.field(args)` where `field` is a record field of fn type:\n"
    "    /// the field's value is called with `args` (the receiver is not an argument).\n"
    "    FieldFn {\n        record: QualName,\n        field: String,\n    },\n}",
)
replace(
    "    bindings: HashMap<Span, Ty>,\n    exports: ModuleTypes,",
    "    bindings: HashMap<Span, Ty>,\n"
    "    /// Constant patterns, keyed by pattern address.\n"
    "    pattern_consts: HashMap<usize, QualName>,\n"
    "    exports: ModuleTypes,",
)
replace(
    "    /// The type of a `let`/`var`/`for`/pattern binding, by the span of its name.\n",
    "    /// The constant a `PatternKind::Variant` pattern names, when it names a\n"
    "    /// constant rather than a variant (`docs/slice.md` section 3).\n"
    "    pub fn pattern_const(&self, pattern: &Pattern) -> Option<&QualName> {\n"
    "        self.pattern_consts\n"
    "            .get(&(pattern as *const Pattern as usize))\n"
    "    }\n\n"
    "    /// The type of a `let`/`var`/`for`/pattern binding, by the span of its name.\n",
)
open(path, "w", encoding="utf-8").write(text)
print("ok")
