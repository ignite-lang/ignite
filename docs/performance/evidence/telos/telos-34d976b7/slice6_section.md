
## 10 Slice 6: surface syntax

Slice 6 shortens Telos source without new runtime behaviour. Sections 2 to 5 and 8 are
rewritten to include it; this section is the contract between the frontend
(`telos-syntax`, `telos-check`, `telos-types`) and the backends (`telos-rustgen`,
`telos-llvm`) and states, per feature, where it is handled. "Desugar" means the parser
produces existing AST forms, so backends see nothing new.

| # | feature | source | handled by |
|---|---|---|---|
| 1 | no `await` | `let input = stream.read(4096)?` | parser: `await` is a syntax error with a hint; checker: an effectful call needs an effectful context (E010 reworded: "effectful call outside an effectful function"); E011 and E012 retired. `ExprKind::Await` is no longer produced |
| 2 | inferred `requires` | a fn without `requires` that performs effects is effectful; its set is inferred (fixed point over the call graph, section 5.1 rules). A fn with `requires` is checked against its list. Protocol roles must declare `requires` (E016 names the missing effect). `FnDecl::is_effectful` is no longer the test: `Symbols.effects` and `FnSig.effectful` carry the inferred answer | checker, typer |
| 3 | expression body | `fn f(x: Int) -> Int = x + 1` | desugar: a block with one `Expr` statement |
| 4 | tuples | type `(A, B)` = `Type { name: "Tuple" }`, value `ExprKind::Tuple`, pattern `PatternKind::Tuple`, `let (a, b) = e` = `StmtKind::LetPattern`; `()` is the Unit value (`ExprKind::Tuple(vec![])`, type `Unit`). `Ty::Tuple(Vec<Ty>)`, two or more elements | all layers. Rust: Rust tuples. LLVM: an anonymous struct with C layout, copied by value with element glue |
| 5 | patterns | `true`/`false` (`PatternKind::Bool`), or-patterns `a \| b` (no bindings inside an or-pattern, E049 otherwise), constant patterns (a payload-less path naming a constant; `TypedProgram::pattern_const`; compared with `==`), tuple patterns, list patterns `[p, q]` (exact length) | parser, checker (E014 retired), typer (exhaustiveness E046 covers Bool, tuples, or; list patterns need a catch-all), both backends |
| 6 | default field values | `type Reply { lines: List<Text>, action: Action = Continue }`; a literal may omit a defaulted field (E047 only for fields without default). A default is lowered like a constant of the record's module named `<Record>.<field>`; an omitting literal calls it | parser, typer, both backends |
| 7 | field punning | `Reply { lines }` = `Reply { lines = lines }`; also `,` as separator on one line | desugar |
| 8 | variant inference | bare `Continue`, `Interpret`, `Some`... resolve against the expected type; an ambiguous bare name with no expected type stays E005 | typer |
| 9 | interpolation | `"too long: {n} bytes"`; parts are expressions of type Text, Int, Byte or Bool; `{{` and `}}` are literal braces | lexer + desugar to `"too long: " + n.to_text() + " bytes"`; new builtin method `to_text` on Int, Byte, Bool, Text (both backends) |
| 10 | `+` on Text | `a + b` | typer (`BinaryOp::Add` on Text is concatenation), both backends |
| 11 | byte literals | `b' '` is an `Int` literal of the byte (typed `Byte` by context, E044 range), `b'\n'`, `b'\\'`, `b'\''`, `b'\xNN'`; `b"\r\n"` is `Bytes.of([13, 10])` | lexer, desugar |
| 12 | `return` expression | `Eof => return Ok(())`; `ExprKind::Return`, type Never. `StmtKind::Return` stays for statements | parser, typer, both backends |
| 13 | discard | `_ = e` and `let _ = e` = `StmtKind::LetPattern` with `PatternKind::Wildcard` | parser |
| 14 | type alias | `type IoResult<T> = Result<T, IoError>`; `Decl::Alias`; exported and importable like records; resolved entirely by the typer (backends see the target type in `Ty`) | parser, checker (names), typer |
| 15 | UFCS | `x.f(a)` where the receiver type has no builtin method or field `f` and a fn `f` is in scope (local or imported) whose first parameter accepts the receiver: `Callee::Fn` on the `Method` node; backends pass the receiver as first argument | typer, both backends |
| 16 | fn-field call | `application.respond(line)` where `respond` is a record field of fn type: `Callee::FieldFn` on the `Method` node; backends call the field's value with `args` | typer, both backends |
| 17 | `for b in bytes` | `for` over `List<T>` including `Bytes`, already in the grammar; tested on both targets | tests |

Rules:
- Backends may assume the typer has accepted the program: every new node has a type, every
  `Method` node a callee, every constant pattern an entry in `pattern_const`.
- `unimplemented!("slice 6: …")` arms left by the skeleton must all be gone at the end.
- Generated Rust and IR goldens change only where a feature is used.
- Std and example sources are rewritten to use these features after the frontend and
  backends land (acceptance: `std/protocol/telnet.telos` at most 400 lines, no line over 100
  characters; `examples/terminal/terminal.telos` without `return`, `await` or
  `Action.Continue`).
