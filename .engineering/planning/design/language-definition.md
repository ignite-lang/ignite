---
format: aep.planning-md/3
id: design:language-definition
kind: design
status: draft
title: One machine-readable Ignite language definition that docs, highlighters and tools are generated from
revision: 1
---
# Formalizing the Ignite language definition: proposal (2026-11-04)

Read-only survey of `~/projects/ignite-lang/ignite-lang` at `8ec44030` (`git log -1`), sibling
`ignite-ess` (pins `ignite-syntax` rev `9578530`, `ignite-ess/Cargo.toml:19`), `ignite-helix`
(`git log`: "branch 'main' does not have any commits yet", so its hand copies were not readable).
Labels: **V** verified by reading or a command shown; **I** inferred. Code-set comparisons were
made with `grep -rhoE '"E[0-9]{3}"' crates/*/src` (68 codes emitted) against `docs/slice.md`
(78 codes named), every `*.expected` fixture, and `site/book/src/reference/diagnostics.md`.

## 1 Inventory: each language fact and where it is stated

| fact | sources today (file:line) | drift |
|---|---|---|
| keywords (32) | `crates/ignite-syntax/src/lexer.rs:14-45` (enum), `:121-194` (`symbol`), `:197-232` (`keyword`) — three hand lists in one file; `docs/slice.md:71-73`; `docs/slice.md:2030-2031` (adds 3); `site/book/src/reference/lexical.md:48-66`; `site/book/theme/highlight.js:61-64`; `docs/syntax.md:27`; `docs/design.md:136`; `ignite-ess/crates/ignite-ess-print/src/lib.rs:346-349` | **yes (V)**: highlight.js omits `await` (lexer.rs:39 still lexes it; lexical.md:63 lists it as retired). slice.md:71-73 lists 29 words, no `contract policy assembly` (superseded by :2031 but never edited). syntax.md:27 says "29 reserved words today"; lexical.md:50 says 32 |
| top-level (declaration) keywords | `parser.rs:164-178` (`is_top_level_keyword`, 10 kinds); `reference/grammar.md:140-142`; `ignite-ess-print/src/lib.rs:346-349` | no (V): all three list the same 10 |
| punctuation / operators | `lexer.rs:404-440` (lexing), `:155-185` (`symbol`); `slice.md:74`; `lexical.md:274-296`; `highlight.js:97` (operator regex) | **yes (V)**: slice.md:74 lacks `..` and `..=` (lexer.rs:406-410). highlight.js:97 lacks `..`, `..=`, `\|` (cosmetic) |
| operator precedence | parser recursive descent `parser.rs:1798-1984` (`binary_left`, `equality`, `compare`, `additive`, `multiplicative`, `unary`, `postfix`); `slice.md:127-135`; `reference/grammar.md:246-266` (9 levels) | partial (V): slice.md:134 still has `'await' unary`; book is current. Neither copy is checked against the parser |
| newline / separator rules | `slice.md:79-86`; `reference/grammar.md:174`; parser `skip_newlines` sites; `lexer.rs:341-350` (Newline merge) | not checkable today (I): prose only, tested by `crates/ignite-syntax/tests/grammar.rs` cases |
| literal forms (int, hex, string escapes, interpolation, raw, byte) | `lexer.rs:242-264` (doc), `:265-`; `slice.md:49-70`; `lexical.md:96-234`; `highlight.js:74-90` | **yes (V)**: slice.md:54 "No multi-line strings" vs raw strings that "may span lines" (lexer.rs:259). highlight.js string rule `begin: /b?"/` has no raw-string `r#"…"#` mode (I: mis-highlights a raw string containing `"`) |
| quantity units (20 rows) | `crates/ignite-syntax/src/ast.rs:662-683` (`QUANTITY_UNITS`, parse); `crates/ignite-types/src/lib.rs:236-251` (debug text); `crates/ignite-io/src/quantity.rs:19-35` (debug text, substrate); `slice.md:1877-1879`; `lexical.md:241-245`; `highlight.js:56` (UNITS regex); `crates/ignite/src/run/clock.rs:10,94` (reuses ast table) | no today (V: lists agree), but 6 hand copies; types/lib.rs and io/quantity.rs are line-identical functions (I: copy-paste) |
| grammar productions | `slice.md:89-149` (slice-1 grammar + amendments in later sections); `reference/grammar.md:24-133` (current whole grammar); parser.rs (2703 lines) | **yes (V)**: slice.md:89-90 `decl` lacks `assembly`, `contract`, `policy`, closures, `..=` patterns, quantity literals; book grammar.md:30,67-68,100,120,125 has them. `tests/grammar.rs:1` says "one test per grammar rule of `docs/slice.md` section 3" — the stale copy |
| diagnostic codes + meanings | ~370 string literals across `crates/*/src` (`Diagnostic::new` 59×, `.error("E` 42×); `Diagnostic.code: &'static str` (`diagnostic.rs:11`); `slice.md:845-960` §8 plus per-slice tables (§12.8, §13.9, §14.11, …); `reference/diagnostics.md` (one section per code); code ranges only in `docs/vision.md:249-256` (T4) | **yes (V)**: book diagnostics.md:3 says codes `E001 to E129` and has no entry for **E090, E091, E092**, which `crates/ignite-check/src/clauses.rs:167` and `crates/ignite-eval/src/model.rs:1168` emit. Book row 1501 and slice.md:864-872 list retired/reserved E011 E012 E014 E021 E097 (none emitted: consistent). E018 appears only in a range at diagnostics.md:369 and nowhere in slice.md or code |
| messages | inline `format!` at each emitter; quoted in book ```` ```text ```` blocks after each `fails=` example | unchecked (V): `crates/ignite/tests/site.rs:236-243` matches only `error[{code}]`, never the message text |
| code → triggering fixture | `crates/ignite-syntax/tests/fixtures/*.expected`, `crates/ignite-check/tests/fixtures/*.expected` (137 files), `fails=Ennn` blocks in the book | **gap (V)**: E020 and E032 have no `.expected` fixture (they are tested in `ignite-rustgen/tests/units.rs`, `ignite/tests/cli.rs`). No test asserts "every active code has a fixture" though `slice.md:1864` requires it per slice |
| effects / capabilities | `crates/ignite-check/src/lib.rs:158-171` (`EFFECTFUL_METHODS`), `:269` (`EFFECTFUL_STATICS`), `:336` (`AMBIENT_HEADS`), `:338` (`static_effect`); `slice.md:340-433`, `:883`; `reference/effects.md:13-62` | no found (V for ambient heads: lib.rs:336 = effects.md:61 = slice.md:883); method→effect tables in effects.md:31-32 not checked |
| builtin types / enums / statics | `ignite-check/src/lib.rs:91-153, 214-247`; `highlight.js:66-68` (`built_in`); `reference/types.md` | **yes (V)**: highlight `built_in` lacks `Stdin Stdout Input Signal SystemRef Protocol Module` and has non-builtins `module event rolling exponential` (lib.rs:99-127) |
| declaration kinds (system, component, resource, contract, policy, assembly, protocol) | `slice.md:2022-2033` §14.7, §14.4; `reference/grammar.md:60-68`; `site/book/src/design/{systems,components,contracts}.md`; `std/deploy.ignite`, `std/contract.ignite` | see grammar row |
| std vocabulary | `std/*.ignite` (14 + `protocol/` 2 + `rfc/` 5 files); `AGENTS.md:28-45`; `site/book/src/reference/std/*.md` (15 pages, hand-written) | **yes (V)**: no reference page for `std.rfc` (`std/rfc.ignite`); `grep -rln 'std\.rfc' site/book/src` hits only `roadmap.md`. `std.text`: every `fn` of `std/text.ignite` is named in `reference/std/text.md` (loop check) |
| CLI commands | clap derive `crates/ignite/src/main.rs:53-330` (14 commands incl. `rfc import/check`, `conformance`); `AGENTS.md:78-98`; `reference/cli.md` | **yes (V)**: `reference/cli.md` has no section for `rfc import`, `rfc check` or `conformance` (`grep -n 'rfc\|conformance' cli.md` empty) |
| diagnostic code ranges per wave | `docs/vision.md:249-256` only | not machine-checked (V) |

Summary of real drift (V): 1 keyword missing from highlighting; 3 emitted codes missing from the
book; 3 CLI commands missing from the book; 1 std module without a page; slice.md §2/§3 stale
against later sections; `tests/grammar.rs` anchored to the stale grammar. Every drift is a
generated-or-hand copy that no test compares to the compiler.

## 2 Options for one source of truth

Constraints: tooling and checks are Rust, as `#[test]` in the owning crate or a Rust binary
(`AGENTS.md:408`); public compiler APIs: add, do not rename (`AGENTS.md:415`); goldens use
`IGNITE_BLESS` (`crates/{ignite,ignite-rustgen,ignite-llvm}/tests/*.rs`); compiled programs run no
hand-written Rust except `ignite-io`/`ignite-rt` (`AGENTS.md:53-54`).

| | A: Rust crate as data | B: ESS specification | C: declarative file + Rust generator | D: self-description in Ignite (`std.lang`) |
|---|---|---|---|---|
| source | `crates/ignite-lang` (leaf, no deps): `KEYWORDS`, `PUNCT`, `PRECEDENCE`, `UNITS`, `CODES`, grammar as `const` data | `ess/ignite-lang/system.yaml` | `lang/ignite.toml` (+ `grammar.ebnf`) | `std/lang.ignite` constants |
| compiler reads it | directly: `use ignite_lang::KEYWORDS` (const, zero cost) | no: ESS models entities, commands, views, types (`ess/README.md`, `system.yaml` kinds `newtype`…) — no production, token or precedence construct (I from README/schemas; not exhaustively checked) | via `build.rs` or checked-in generated `.rs` (I: build.rs adds a codegen step to every build) | no: the lexer runs before any Ignite exists; bootstrapping circle |
| grammar expressible | yes as Rust data (enum of `Seq/Alt/Opt/Rep/Tok/Rule`) | no (I) | yes, EBNF is native | as data records, verbose |
| generates | book fragments (`{{#include}}`), highlight.js lists, tree-sitter `grammar.js` word lists, Helix `highlights.scm`, a JSON export for other repos, `std/lang.ignite` for the LSP | docs, OpenAPI, conformance (ESS's own projections) — none of the needed artifacts | same as A | docs only via `ignite` reading std |
| drift checks | `#[test]` in each consumer crate + one in `ignite-lang` per artifact | ESS conformance cannot reach a lexer (I) | same tests as A, plus parse-the-file test | `ignite test` |
| cost | S–M: one crate, lexer keyword map becomes a lookup in the table (lexer.rs:197-231 kept as is, a test proves equality first) | L, and circular: `ignite-ess` lowers ESS *to* Ignite | M: a file format, a parser, a generator, and still Rust consts for the lexer | L, and the compiler still needs Rust copies |
| fits rules | yes | language is not an ESS domain; the org "ESS drives every product repository" rule is for beyond10x repos, ignite-lang is `github.com/ignite-lang` (unclear, ask Timo) | yes (generator Rust) | yes, but D cannot be the root |

Verdict: **A**, with D as a *generated output* (the LSP in Ignite gets `std/lang.ignite` written
from the crate, drift-checked) and C rejected because a TOML layer adds a parser and a second
representation over Rust data the compiler needs anyway. B does not express grammars (I) and is
circular for this repo.

### 2.1 Shape of A (all additive)

| item | type | consumers | replaces |
|---|---|---|---|
| `ignite_lang::KEYWORDS: &[Keyword { text, group, status: Active\|Retired, top_level: bool }]` | const | lexer (test first, then lookup), parser `is_top_level_keyword`, book `lexical.md` table, highlight.js, tree-sitter, Helix, ignite-ess `STATEMENT_WORDS` (via pin) | 3 lists in lexer.rs, slice.md:71-73, lexical.md:52-63, highlight.js:61-64 |
| `PUNCT: &[Punct { text, name, uses }]`, `PRECEDENCE: &[Level { ops, assoc }]` | const | lexer test, parser test (parse `a op1 b op2 c` for every pair, compare tree to table), `grammar.md` precedence table | grammar.md:250-260, slice.md:127-135 |
| `UNITS: &[Unit { name, kind, factor, plural_of }]` | const | `ast::QUANTITY_UNITS` re-exported (kept, add-only), `ignite-types` debug text; `ignite-io` keeps its copy (substrate rule) plus a dev-dependency test asserting equality | 6 copies |
| `CODES: &[Code { id, stage, wave_range, status: Active\|Retired{since}\|Reserved, summary, reported_by }]` + `RANGES` (T4) | const | tests below; book diagnostics index; `ignite explain --code`? (later) | vision.md T4 table, slice.md §8 index, diagnostics.md:40-48 |
| `GRAMMAR: &[Rule { name, rhs: Expr, note }]` | const | `reference/grammar.md` block generated; `tests/grammar.rs` iterates rule names (one test per rule) | grammar.md:26-133; slice.md §3 becomes a pointer |
| `ignite lang export --json` (new subcommand) or `ignite-lang` bin | CLI | `ignite-helix`, LSP, ignite-ess pin their consumption to a rev, as ignite-ess pins `ignite-syntax` | hand copies in other repos |

### 2.2 Drift checks (each a `#[test]`, failing with the diff; `IGNITE_BLESS=1` rewrites)

| test | home | catches today |
|---|---|---|
| lexer keyword map == `KEYWORDS` (round-trip each word through `lex`, and `symbol()`) | `ignite-syntax/tests` | future lexer/doc divergence |
| every `"Ennn"` literal in `crates/*/src` (source scan) is an Active code; no Retired code emitted | `ignite-lang/tests` | E018-style phantom codes |
| every Active code has a `.expected` fixture or a `fails=` site block | `ignite/tests/site.rs` (extend) | E020, E032 (no `.expected`; accept book block or add fixture) |
| every Active code has a `{#ennn}` section in `diagnostics.md`; header range generated | `ignite/tests/site.rs` | **E090, E091, E092** |
| book fragments `site/book/src/generated/*.md` == render(data) | `ignite-lang/tests` | lexical/grammar/precedence/units tables |
| `highlight.js` Ignite block between `// BEGIN generated` markers == render | `ignite-lang/tests` | **missing `await`**, built-ins |
| every clap subcommand (`Cli::command().get_subcommands()`) has a `## ignite …` section in `cli.md` | `ignite/tests/site.rs` | **`rfc import`, `rfc check`, `conformance`** |
| every `std/**/*.ignite` module has a `reference/std/*.md` page; every pub `fn` named | `ignite/tests/site.rs` | **`std.rfc`** |
| book code blocks: message text in the ```` ```text ```` after a `fails=` block equals actual output | `ignite/tests/site.rs` | message drift (unknown count) |

## 3 Recommendation and first slice

Ordered; each step is one unit in one wave, smallest valuable first. Effort is S/M (I).

| # | lands | value | effort |
|---|---|---|---|
| 1 | `crates/ignite-lang` with `KEYWORDS`, `PUNCT`, `UNITS`; lexer/ast tests asserting equality (no lexer rewrite yet); `ast::QUANTITY_UNITS` stays, built from or tested against `UNITS` | one place to add a keyword; tests catch any divergence | S |
| 2 | `CODES` + `RANGES` registry; source-scan test, fixture-coverage test, diagnostics.md coverage test; add the three missing E090–E092 sections | fixes V drift; replaces T4 prose with data | S–M |
| 3 | generator `ignite-lang` bin (or `#[test]` with `IGNITE_BLESS`) writing `site/book/src/generated/{keywords,punctuation,units,precedence,codes}.md`, `{{#include}}`d by `lexical.md`, `grammar.md`, `diagnostics.md`; marker block in `highlight.js` | docs and highlighting cannot drift; fixes `await` | S |
| 4 | `ignite lang export --json` (`ignite-lang/1` schema) for `ignite-helix` and the LSP; helix repo adds a Rust test against a pinned rev that compares its `grammar.js` word lists and `highlights.scm` | no hand copies outside the repo | S |
| 5 | CLI and std coverage tests (`cli.md` sections from clap, std page per module); write `rfc`/`conformance` sections and `std/rfc.md` | fixes V drift | S |
| 6 | `PRECEDENCE` + pairwise parser test; generated precedence table | precedence provably matches parser | S |
| 7 | `GRAMMAR` as data, generated `grammar.md` block; `tests/grammar.rs` iterates rule names; slice.md §2/§3 marked historical with a pointer | one grammar | M |
| 8 | generated `std/lang.ignite` (keywords, units, codes as Ignite constants) for the LSP written in Ignite | LSP reads the same facts | S, after LSP design |

Out of scope at first (explicit):

- Generating the parser, lexer or tree-sitter grammar rules from `GRAMMAR`; only word lists and
  token regexes are generated. The hand parser stays authoritative; `GRAMMAR` is checked by tests
  that parse each rule's examples (I: a parser generator would change error recovery and E054).
- Moving diagnostic messages into templates; messages stay at the emitters (`format!` with
  context). Only `summary` lives in the registry.
- Renaming `Diagnostic.code: &'static str` or any public item; `ignite_lang::codes::E040` consts
  may be added and adopted gradually.
- Making `ignite-io` depend on the new crate (substrate rule); it keeps its unit copy under test.
- ESS modelling of the language (option B) and any change to `ignite-ess` beyond consuming the
  export at its next pin bump.
- Rewriting historical `docs/slice.md` sections; they get a pointer, not edits, since slices are
  dated contracts (I).

Open for Timo: whether the beyond10x "ESS drives every product repository" rule applies to
`github.com/ignite-lang` (unclear; this proposal assumes not).
