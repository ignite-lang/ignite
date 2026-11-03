EDITS = [
("""| `Symbols.closure_effects: BTreeMap<Span, EffectSig>`, `Symbols.glob_conflicts` | checker results the typer reads |""",
"""| `Symbols.closure_effects: BTreeMap<Span, EffectSig>`, `Symbols.glob_conflicts` | checker results the typer reads |
| `telos_types::{equality, boxed_payload, boxed_field}(.., &TypedModules)` | the same answers as the `TypedProgram` methods, for a layout pass that holds only the modules |
| `telos_check::AMBIENT_HEADS` | `stdin`, `stdout`, `process`, `net`: the heads of ambient effects (E056) |
| `impl Drop for Expr` | drops binary and unary chains from a work list; an owned `Expr` cannot be destructured by move (match on `&e.kind`) |"""),
]
