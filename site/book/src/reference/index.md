# Reference

The reference describes Ignite v0.3.0 as the compiler implements it. Where the
[language design](../design/index.md) says why, these pages say exactly what: the tokens, the
grammar, the type and effect rules, every standard-library module, every command and every
diagnostic.

| page | what it covers |
|---|---|
| [Lexical structure](lexical.md) | comments, identifiers, keywords, literals, quantities with units, punctuation |
| [Grammar](grammar.md) | the whole grammar the parser accepts, line breaks, operator precedence |
| [Types](types.md) | scalars, records, enums, generics, `Option` and `Result`, lists and maps, functions, quantities |
| [Effects](effects.md) | effect names, `requires`, inference, effect roots, pure property expressions |
| [Modules and imports](modules.md) | packages, files and module paths, imports, `module("…")` bindings |
| [Tests and assertions](tests.md) | `test` blocks, `assert`, test-only declarations, how `ignite test` runs them |
| [Standard library](std/index.md) | every module of `std/`, its layer and its public items |
| [Command line](cli.md) | every `ignite` command, its options and its exit statuses |
| [Diagnostics](diagnostics.md) | every error code, `E001` to `E129`, with a program that triggers it |

## Conventions on these pages

- Every `ignite` code block carries a label in its corner. **checked**: a complete program;
  the site's own test runs `ignite check` on it, so it compiles with the release it
  describes. **excerpt**: a piece of a larger file, shown for reading. **fails to check, on
  purpose**: a program that must produce the diagnostic it illustrates, and the test checks
  that it does.
- A diagnostic is written `file:line:column: error[Ennn]: message`; the code links to its
  entry in [Diagnostics](diagnostics.md).
- Repository paths such as `std/text.ignite` or `docs/slice.md` link to the source on GitHub.
  [`docs/slice.md`](https://github.com/ignite-lang/ignite/blob/main/docs/slice.md) is the
  contract between the compiler's crates and the most detailed account of each rule.
