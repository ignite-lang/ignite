EDITS = [
("""# Telos slice 1: language subset, standard library, substrate and Rust lowering""",
"""# Telos slices 1 to 9: language subset, standard library, substrate and lowering"""),
("""Slice 0 (server only, Telnet in the example, hand-written reference client and server) is
superseded. Its reference crates are archived in `target/archive/terminal-reference.tar.gz`.
""",
"""Slice 0 (server only, Telnet in the example, hand-written reference client and server) is
superseded.
"""),
("""  backends land (acceptance: `std/protocol/telnet.telos` at most 400 lines, no line over 100
  characters;""",
"""  backends land (acceptance: `std/protocol/telnet.telos` at most 400 lines excluding tests, no
  line over 100 characters;"""),
# section 8 table
("""| E006 | assignment to an immutable binding or a constant |""",
"""| E006 | assignment to an immutable binding or a constant; since slice 9 also `push`, `append` or `clear` through a `let`, a `for` element or a pattern binding, on `x.to_bytes()`, or on a binding a closure captured (section 13.11) |"""),
("""| E003 | unknown name; also wrong arity of a call (a UFCS call counts the receiver), `log` used as a value, a constant with payload patterns |
| E004 | duplicate definition, including a declaration named like a builtin or `Tuple`, and a role or record field declared twice |""",
"""| E003 | unknown name; also wrong arity of a call (a UFCS call counts the receiver), `log` used as a value, a constant with payload patterns, an associated fn whose owner is no record or enum of the module (section 13.8) |
| E004 | duplicate definition, including a declaration named like a builtin or `Tuple`, a role or record field declared twice, an associated fn declared twice or named like a variant of its owner, and a closure parameter repeated |"""),
("""| E016 | effect missing from `requires` (section 5.1) |""",
"""| E016 | effect missing from `requires` (section 5.1); a closure's effects not on its own parameters are charged where it is created (section 13.3) |"""),
("""| E054 | nesting deeper than 256 levels of brackets, type arguments, prefix operators or `return` (section 3) |
| E055 | a record or enum that contains itself other than through `List`: by a field, a payload, a tuple, `Option`, `Result` or another record or enum. Recursion through an enum payload or `Option` (finite values, needs boxing in both backends) is a later extension. |""",
"""| E041 | (slice 9) a name two glob imports provide, used without an explicit import (section 13.1 row 6) |
| E054 | nesting deeper than 256 levels of brackets, type arguments, prefix operators or `return` (section 3); string literals nested deeper than 32 interpolations (section 13.1 row 1) |
| E055 | a record that contains itself through fields and tuples alone, so a value would be infinite; recursion through an enum payload, `Option`, `Result` or `List` is allowed and boxed (section 13.4) |
| E056 | a parameter of a fn, role, fn type or closure named like an ambient capability: `stdin`, `stdout`, `process`, `net` (section 13.11) |
| E057 | a type parameter of a generic record, enum or alias that no field, payload or aliased type uses (section 13.11) |"""),
("""E041 unknown method or field for the receiver type (also an ambiguous or inapplicable method call, section 4.8), E042 type cannot
be inferred (also a type parameter a generic use leaves unbound, and an effectful operation
whose effects `telos-check` could not check, section 11.7), E043 wrong number of arguments, E044 integer literal out of range for `Byte`,
E045 operator not defined for the operand type (also a constant pattern of a type without
`==`), E046 `match` not exhaustive,""",
"""E041 unknown method or field for the receiver type (also an ambiguous or inapplicable method call, section 4.8; a tuple index past the tuple; a fn named like a builtin method of its first parameter's type, section 13.1 row 5), E042 type cannot
be inferred (also a type parameter neither the expected type nor later uses in the body decide, a closure parameter without type, and an effectful operation
whose effects `telos-check` could not check, section 11.7), E043 wrong number of arguments (also a range pattern whose start is above its end), E044 integer literal out of range for `Byte`,
E045 operator not defined for the operand type (also `==` on a type that is not comparable, section 13.5, a constant pattern of a type without
`==`, and a range bound that is no literal or constant with a literal value), E046 `match` not exhaustive (ranges split `Int` and `Byte` into intervals, section 13.7),"""),
("""Fixtures: E001, E002, E053 and E054 under `crates/telos-syntax/tests/fixtures`; E003, E004,
E006, E010, E013, E015, E016, E030, E031, E047, E048, E049, E050, E051 and E055 under
`crates/telos-check/tests/fixtures`;""",
"""Fixtures: E001, E002, E053 and E054 under `crates/telos-syntax/tests/fixtures`; E003, E004,
E006, E010, E013, E015, E016, E030, E031, E041 (glob conflict), E047, E048, E049, E050, E051,
E055, E056 and E057 under `crates/telos-check/tests/fixtures`;"""),
# section 13 fixes
("""| 11 | `x.debug()` | `Callee::Builtin(Builtin::Debug)`, `Text`, on every type without a type parameter anywhere inside (`List<T>` in a generic fn has none: E040) |""",
"""| 11 | `x.debug()` | `Callee::Builtin(Builtin::Debug)`, `Text`, on every type without a type parameter anywhere inside (`List<T>` in a generic fn has none: E041); `Show::Debug` likewise |"""),
("""| 3 | none | `List.empty()`, `[]`, `None`, `Ok`/`Err` and other values with an unbound type parameter take the expected type, else the type later uses in the same body give them; E042 only when neither decides | none: every `Ty` the backends read is resolved |""",
"""| 3 | none | `[]`, `List.empty()`, `Store.new()`, `None`, `Ok(x)` and `Err(e)` bound by a `let`/`var` without annotation take the type later uses in the same body give them: an argument, a `return`, an assignment, a pushed element, any position with an expected type. The typer walks such a body once more with inference holes and then types the `let` as if annotated; E042 only when neither the expected type nor a use decides | none: every `Ty` the backends read is resolved |"""),
]
