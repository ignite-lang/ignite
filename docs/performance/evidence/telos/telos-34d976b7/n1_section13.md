
## 13 Slice 9: friction fixes

The coordinator's decisions of 2026-11-01 for the 17 rows of `docs/friction.md`, as built.
N1 (frontend: `telos-syntax`, `telos-check`, `telos-types`) settled this section; N2 (Rust
target) and N3 (LLVM target and runtime) build against it. The slice-6 to slice-8 rules stay
unless a row below changes them.

### 13.1 Rows

| row | syntax | typing and checking | lowering obligation |
|---|---|---|---|
| 1 | `"a {f("x")} b"`: an interpolation holds any expression, string literals (themselves interpolated, raw or byte) included; still no line break inside `{ }` | nesting deeper than 32 string literals is E054 | none: the parser desugars as before |
| 2 | `r"…"`, `r#"…"#` (any number of `#`, closed by `"` and as many `#`), `br"…"` | no escapes, no interpolation, line breaks kept; a raw byte string is ASCII (E001) | none: `ExprKind::Str` and the byte-string desugaring |
| 3 | none | `List.empty()`, `[]`, `None`, `Ok`/`Err` and other values with an unbound type parameter take the expected type, else the type later uses in the same body give them; E042 only when neither decides | none: every `Ty` the backends read is resolved |
| 4 | `break` and `continue` in expression position (`None => break`) | `ExprKind::Break`/`Continue`, type `Never`; `break` marks the innermost loop as broken | as the statements |
| 5 | none | `x.f(..)` is the builtin `f` when it applies to `x`'s type, else a UFCS call of the user fn `f` (`Callee::Fn`). A user fn whose first parameter type has a builtin method of the fn's name is E041 at the declaration | none: `callee()` says which |
| 6 | `import std.text.*` | every non-test public item of the module; an explicit import and an own declaration win; a name two globs provide is E041 at its use | `Import::is_glob()`, `Import::module()`; a Rust `use` of every item (or of the names `Symbols::imported` lists from that module) |
| 7 | none | `==`/`!=` on every comparable type (13.5); E045 otherwise | general equality glue, the slice-8 assert glue generalised |
| 8 | `b'0'..=b'9' => …`, `LIMIT..=99` | 13.7 | `range_bounds(pattern)`: `lo <= v && v <= hi` |
| 9, 13 | `\|x, y\| expr`, `\|x\| { … }`, `\|x: Int\| …`, `\|\| e` | 13.3 | 13.3 |
| 9 | `Todo { done = flag, ..t }` | `ExprKind::RecordUpdate`: `base` has the record's type, each field its field type; E047 for an unknown or repeated field | a copy of `base` with the fields replaced; fields evaluated in source order, then `base` |
| 10 | `t.0`, `t.1` | `ExprKind::TupleField`; an index past the tuple is E041 | the element |
| 11 | `x.debug()` | `Callee::Builtin(Builtin::Debug)`, `Text`, on every type but a type parameter | 13.6; a failed `assert a == b` shows both values by `AssertCompare.show`, which is `Show::Debug` where no `to_text` applies |
| 12 | `"\u{1F600}"` | 1 to 6 hex digits, a Unicode scalar value (E001 otherwise); Text literals only | none |
| 14 | a left-deep chain of 100,000 `+` | checker and typer walk the left spine of `Binary` iteratively | every backend pass must too, or prove its depth bound |
| 15 | `enum Tree<T> { Leaf, Node(T, Tree<T>) }`, `type Node { next: Option<Node> }` | E055 only for a record holding itself through records and tuples alone | 13.4 |
| 16 | none | none | generic instances emitted once per program (`linkonce_odr` or one module) |
| 17 | `fn Response.json(status: Int, body: Json) -> Response = …` | 13.8 | 13.8 |

### 13.2 AST and typed API (add-only)

| item | meaning |
|---|---|
| `ExprKind::Closure(Closure { params: Vec<ClosureParam { name, ty: Option<Type>, span }>, body: Box<Expr> })` | a closure literal |
| `ExprKind::RecordUpdate { name, fields, base }` | `Name { f = e, ..base }` |
| `ExprKind::TupleField { receiver, index: usize }` | `receiver.index` |
| `ExprKind::Break`, `ExprKind::Continue` | in expression position; statement position stays `StmtKind::Break`/`Continue` |
| `PatternKind::Range { start, end }` | bounds are `PatternKind::Int` or a payload-less `PatternKind::Variant` naming a constant |
| `FnDecl.owner: Option<Ident>`, `FnDecl::qualified_name()` | `Some(Response)` for `fn Response.json`; `qualified_name()` is `Response.json` |
| `Import::is_glob()`, `Import::module()` | `import a.b.*` is `path` `a.b.*`, `names` empty |
| `TokenKind::DotDot`, `TokenKind::DotDotEq` | `..`, `..=` |
| `TypedProgram::closure(expr) -> Option<&ClosureInfo>` | `ClosureInfo { params: Vec<(String, Ty)>, ret, captures: Vec<Capture>, effects: Option<FnEffects> }` |
| `Capture { name, binding: Span, ty, effects: Vec<String> }` | `binding` is the captured binding's name span (the key of `binding_type`); `effects` are the closure's effects whose head is `name` |
| `TypedProgram::equality(&Ty) -> Option<Equality>` | `Scalar`, `Text`, `Structural`, or `None` when not comparable (13.5) |
| `TypedProgram::boxed_payload(&QualName, variant, index) -> bool`, `boxed_field(&QualName, field) -> bool` | 13.4 |
| `TypedProgram::range_bounds(pattern) -> Option<(i64, i64)>` | both bounds of a range pattern, inclusive, constants evaluated |
| `Builtin::Debug`, `Show::Debug` | 13.6 |
| `ModuleTypes.const_values: BTreeMap<String, i64>` | values of `Int` and `Byte` constants whose value is a literal, a negated literal or such a constant |
| `Symbols.closure_effects: BTreeMap<Span, EffectSig>`, `Symbols.glob_conflicts` | checker results the typer reads |

### 13.3 Closures

**Typing.** A parameter's type is its annotation, else the expected fn type's parameter type;
neither is E042. The body is checked against the expected return type, else synthesised (a
`return e` inside a closure without an expected return type is checked against `Unit`).
`return`, `?`, `break` and `continue` inside the body belong to the closure: `return` leaves
the closure, a loop of the enclosing fn is not visible, and a `?` in a closure inside a test
body returns from the closure. The closure's type is `Ty::Fn(params, ret)`, or
`Ty::EffectFn(params, ret, effects)` when its body performs an effectful operation.

**Captures.** A closure captures every binding of an enclosing scope its body reads (a local,
a parameter, a pattern or loop binding; never a module-level fn or constant), in order of
first use, including those only a nested closure reads. Captures are **by copy**: the value at
creation; a later assignment to the outer binding does not change the closure. Assigning to a
captured binding, or `push`/`append`/`clear` on one, inside the closure is E006.

**Effects.** `telos-check` walks the body as a fn without `requires`, parameters rooted in
themselves. Each effect is `p.op` on a parameter `p`, `r.op` on a root `r` of a captured
value (the enclosing fn's parameter it comes from), or ambient (`stdout.write`). Every effect
that is not on a parameter is also needed where the closure is created: a closure carries the
authority it captured, and its creator must hold it. `ClosureInfo.effects` lists all of them
(`FnEffects.names` are the closure's parameter names). An effectful closure where a pure fn
type is expected, or where an effectful fn type does not cover its effects, is E017. A
`let f = |..| ..` binding of an effectful closure may be called; the call needs the effects on
the arguments' roots plus the others (`Symbols::effect_sites` lists it).

**Rust target (N2).** Every fn value, closure or named fn, is
`std::sync::Arc<dyn Fn(P1, ..) -> R + Send + Sync>` (generics where cheaper and equivalent). A
closure lowers to `Arc::new({ let c1 = c1.clone(); .. move |p1, ..| body })`, cloning each
capture at creation. A named fn used as a value is wrapped the same way.

**LLVM target (N3).** A fn value is a pair `{ ptr code, ptr env }`. `code` takes `env` first,
then the parameters. `env` points to a struct of the captures in `ClosureInfo.captures` order,
copied at creation; copying a fn value runs the env's copy glue (each capture's copy glue). A
named fn used as a value has `env` null and a thunk that ignores it.

### 13.4 Boxing

A record field or an enum payload position is **boxed** when its declared type holds a value
of the record or enum that owns the position inline: through records, tuples, `Option`,
`Result` and enums (type arguments substituted), not through `List`, `Store` or a fn type.
`boxed_field`/`boxed_payload` answer per position, for every instance of a generic
declaration alike. Examples: `Tree.Node` payload 1 in `enum Tree<T> { Leaf, Node(T, Tree<T>) }`;
field `next` in `type Node { value: Int, next: Option<Node> }` (the whole `Option<Node>` is
boxed); both `A.b` and `B.X` payload 0 for `type A { b: B }`, `enum B { X(A), Y }`.

Rust: the position's type is `Box<T>`; constructing boxes, matching and reading deref.
LLVM: the position holds a pointer to a heap copy; copy glue copies the pointee, equality and
debug glue follow the pointer. A record that holds itself through records and tuples alone is
E055 and never reaches a backend.

### 13.5 Equality

`==` and `!=` take two operands of one type; the type is **comparable** when it holds no fn,
`TcpStream`, `TcpListener`, `IoError`, `Store` or type parameter anywhere inside (fields,
payloads, elements, type arguments; a recursive type is assumed comparable where it recurs).
Otherwise E045. The operand type is `type_of(left)`, or `type_of(right)` when the left is
`Never`. Both operands are evaluated, left first, each once.

| `equality(ty)` | types | equal when |
|---|---|---|
| `Scalar` | `Int`, `Byte`, `Bool`, `Unit` | same value |
| `Text` | `Text` | same bytes |
| `Structural` | `List` (`Bytes` too), `Option`, `Result`, `Input`, tuples, records, enums | lists: same length and elements pairwise; tuples and records: every element or field in order; enums: same variant and payloads pairwise; boxed positions compare the pointees |

`!=` is the negation. Constant patterns stay limited to `Int`, `Byte`, `Bool` and `Text`.

### 13.6 Debug text

`x.debug()` and a failed `assert` with `Show::Debug` produce this text, the same on both
targets, nested values formatted by the same rules:

| type | text |
|---|---|
| `Int`, `Byte` | decimal, `-` for a negative `Int`: `-3`, `255` |
| `Bool` | `true`, `false` |
| `Unit` | `()` |
| `Text` | in `"`; `"` as `\"`, `\` as `\\`, newline `\n`, carriage return `\r`, tab `\t`, NUL `\0`, any other character below U+0020 and U+007F as `\u{hex}` (lowercase, no padding: `\u{1b}`); every other character as itself |
| `List<T>`, `Bytes` | `[a, b]`, `[]` |
| tuple | `(a, b)` |
| `Option` | `Some(x)`, `None` |
| `Result` | `Ok(x)`, `Err(e)` |
| `Input` | `Data([104, 105])`, `Eof` |
| record | `Todo { id: 1, title: "milk", done: false }`: the record's unqualified name, no type arguments, fields in declaration order; `Empty {}` without fields |
| user enum | `Command.Quit`, `Command.Echo("x")`: unqualified enum name, `.`, variant, payloads in `( )` |
| `IoError` | `IoError("connection refused")`, the message as `Text` |
| fn value | `<fn>` |
| `TcpStream`, `TcpListener`, `Store<T>` | `<TcpStream>`, `<TcpListener>`, `<Store>` |

### 13.7 Range patterns

`lo..=hi` matches a value `v` of type `Int` or `Byte` with `lo <= v <= hi`. Each bound is an
integer literal (`-5`, `0x7F`), a byte literal (`b'a'`, an integer) or a constant of the
scrutinee's type whose value is a literal, a negated literal or such a constant
(`ModuleTypes.const_values`); anything else is E045. A range on another type is E040, a bound
out of `Byte` range E044, `lo > hi` E043. Exhaustiveness splits `Int` and `Byte` columns into
intervals: a `match` on `Byte` covering `0..=127` and `128..=255` needs no `_`; an uncovered
interval is shown as `lo..=hi`.

### 13.8 Associated fns

`fn Owner.name(..)` declares a fn of the record or enum `Owner`, which the same module must
declare (E003 otherwise); a second `Owner.name` or a variant `Owner.name` is E004. It is called
`Owner.name(..)` (a path call) and may be used as a value `Owner.name`. Its `QualName` is
`{ module, name: "Owner.name" }` and its key in `ModuleTypes.fns`, `Symbols.functions` and
`Symbols.effects` is `Owner.name`; the call's callee is `Callee::Fn` of that `QualName`.
Importing `Owner` (explicitly or by glob) brings its associated fns. Backends turn the `.` into
a valid symbol of their own (Rust: an inherent `impl Owner { fn name }` or a mangled free fn;
LLVM: a mangled symbol); a free fn `name` of the same module is a different fn.

### 13.9 Diagnostics changed in slice 9

| code | change |
|---|---|
| E006 | also: assignment to, or `push`/`append`/`clear` on, a captured binding inside a closure |
| E017 | also: an effectful closure where a pure fn type, or an effectful fn type not covering its effects, is expected |
| E041 | also: a name two glob imports provide; a fn whose first parameter type has a builtin method of the fn's name; a tuple index past the tuple |
| E042 | also: a closure parameter without annotation or expected type; no longer for a generic value later uses decide (row 3) |
| E043 | also: a range pattern whose start is above its end |
| E045 | `==`/`!=` on a type that is not comparable; a range bound that is neither a literal nor a constant with a literal value |
| E054 | also: string literals nested deeper than 32 interpolations |
| E055 | narrowed: only a record holding itself through records and tuples alone |
| E003, E004 | also: an associated fn whose owner is no record or enum of the module; one declared twice or named like a variant of its owner |

### 13.10 Placeholders for N2 and N3

Every `unimplemented!("slice 9: …")` arm N1 added so the workspace builds:

- `crates/telos-rustgen/src/lib.rs`: three `match &expr.kind` (closures, record update, tuple
  fields, `break`/`continue` expressions), four `match &pattern.kind` (range patterns), one
  `match show` (`Show::Debug`).
- `crates/telos-llvm/src/lower.rs`: `Lower::expr` (`match &e.kind`), `Lower::pattern` (range
  patterns), the show match (`Show::Debug`), the builtin match (`Builtin::Debug`).
- `crates/telos-llvm/src/region.rs`: the builtin match (`Builtin::Debug`), the expression
  walk (`match &expr.kind`).

`grep -rn 'slice 9:' crates/telos-rustgen crates/telos-llvm` lists them. Not placeholders but
gaps every backend has to close: `FnDecl.owner` (13.8), glob imports (row 6), boxing (13.4),
equality on every comparable type (13.5), and `Builtin::Debug` in the Rust target's builtin
lowering if it matches without a wildcard. `crates/telos-rustgen/tests/units.rs` builds
`FnDecl` with `owner: None`.
