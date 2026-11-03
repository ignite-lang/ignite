EDITS = [
("""| E006 | also: assignment to, or `push`/`append`/`clear` on, a captured binding inside a closure |""",
"""| E006 | also: assignment to, or `push`/`append`/`clear` on, a captured binding inside a closure; `push`/`append`/`clear` through a `let`, a `for` element or a pattern binding, or on `x.to_bytes()` (13.11) |
| E016 | also: a closure's effects on captures and ambient effects, charged where it is created; an effect reached through `map`, `push` or `append` on a field or index, or through a call of a local fn value (13.11) |
| E046 | `Int` and `Byte` columns with a range pattern split into intervals (13.7) |
| E056 | new: a parameter named like an ambient capability (13.11) |
| E057 | new: an unused type parameter of a generic record, enum or alias (13.11) |"""),
]
