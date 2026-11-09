# Reference

The reference describes Ignite as the compiler implements it. Where the
[language design](../design/index.md) says why, these pages say exactly what: the tokens, the
grammar, the statements, the type and effect rules, system declarations, every
standard-library module, every command and every diagnostic.

| page | what it covers |
|---|---|
| [Lexical structure](lexical.md) | comments, identifiers, keywords, literals, quantities with units, punctuation |
| [Grammar](grammar.md) | the whole grammar the parser accepts, protocol clauses, line breaks, operator precedence |
| [Statements and expressions](statements.md) | bindings, assignment, copies and handles, `if`, `match` and patterns, loops, `return` |
| [Types](types.md) | scalars, records, enums, generics, `Option` and `Result`, lists and maps, functions, quantities |
| [Effects](effects.md) | effect names, `requires`, inference, effect roots, pure property expressions, `spawn` |
| [Modules and imports](modules.md) | packages, files and module paths, imports, `module("…")` bindings, method calls |
| [Tests and assertions](tests.md) | `test` blocks, `assert`, test-only declarations, failures |
| [System declarations](systems.md) | `system`, resources, components, contracts, `assembly`, edges, hooks, the system model, the journal |
| [Standard library](std/index.md) | every module of `std/`, its layer and its public items |
| [Command line](cli.md) | every `ignite` command, its options and its exit statuses, and those of compiled executables |
| [Diagnostics](diagnostics.md) | every error code, `E001` to `E151`, most with a program that triggers it |

## Conventions on these pages

- The label on each code block is explained on the [Welcome](../index.html) page.
- A diagnostic is written `file:line:column: error[Ennn]: message`. The code links to its
  entry in [Diagnostics](diagnostics.md).
- Repository paths such as `std/text.ignite` or `docs/slice.md` link to the source on GitHub.
  [`docs/slice.md`](https://github.com/ignite-lang/ignite/blob/main/docs/slice.md) is the
  contract between the compiler's crates and the most detailed account of each rule.
