---
format: aep.planning-md/3
id: story:web-markdown
kind: story
status: draft
title: File literals and Markdown pages checked against the routes
relations:
- decomposes: epic:x-web
- depends_on: story:markup-views
- depends_on: story:web-live
scope:
- confidence: inferred
  path: crates/ignite-check/src
- confidence: inferred
  path: crates/ignite-eval/src
- confidence: inferred
  path: crates/ignite-types/src
- confidence: inferred
  path: crates/ignite/src/apply/plan.rs
- confidence: inferred
  path: crates/ignite/src/lib.rs
- confidence: inferred
  path: docs/commonmark
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: x/web.ignite
- confidence: inferred
  path: x/web/markdown.ignite
revision: 4
---
# Story: web-markdown

## Outcome

`embed("help.md")` is the text of a file beside the module and `embed_all("notes/*.md")` the files a pattern names, both part of the program's source set (X15); `x.web.markdown` parses CommonMark in the evaluator at check time into `Html`, decodes the front matter with a declared type, checks every site-relative link against the route table, and `doc<T>` and `docs<T>` serve the documents with no parsing per request (`docs/web.md` §6.13).

## Context

- web §5 row X15: the file joins the source set, so `ignite check` reads it and a saved plan's source digest covers it (slice §17).
- web §6.13: front matter is a `key: value` subset (one scalar or `[a, b]` list per line) decoded with the type's derived codec (`story:data-derived-codec`); Q14 default: CommonMark 0.31.2 plus tables, raw HTML escaped; holes `{field}` and calls of pure module functions that take no capability; external links not checked (`ignite check` opens no connection); `layout` is a view taking a `Doc<T>` (`story:markup-views`).
- Measured: the prose page 146 → 116 tokens (web §2.8).

## Acceptance

- `embed("help.md")` of a missing file is a check error naming the path; `embed_all` returns files in name order; a pattern escaping the module's directory (`../x`) is refused (fixtures).
- Changing `help.md` changes the source digest of `ignite plan --for-apply` (test in `crates/ignite/tests/`); `ignite check` of an unchanged program with a changed `.md` reports its new diagnostics.
- `x.web.markdown` and the CommonMark 0.31.2 spec examples, vendored as data under `docs/commonmark/` with the licence note: the claimed sections are exactly ATX headings, setext headings, thematic breaks, indented code blocks, fenced code blocks, paragraphs, blank lines, block quotes, list items, lists, backslash escapes, entity and numeric character references, code spans, emphasis and strong emphasis, links, images, autolinks, hard line breaks, soft line breaks, textual content, plus every example of the GFM tables extension. Every example of a claimed section must pass; the test fails on any failing one. Examples of the unclaimed sections (HTML blocks, raw HTML, link reference definitions, tabs) are skipped by section name only, and since raw HTML is escaped their expected output differs by design; the passed and skipped counts are printed and recorded in `docs/slice.md`. Raw HTML in a document is escaped (inline test).
- `doc<Help>(embed("help.md"), layout = article)` answers `GET /help` with the rendered page on both targets (inline test); a missing, extra or mistyped front-matter key is a check error at `help.md:<line>`; `[contact](/contact)` with no such route is E120 at `help.md:<line>`; an external link is not checked (fixtures).
- Holes: `{title}` in the body renders the front-matter field; a call of a pure module fn with no capability renders its result; a hole calling a fn that takes a `Store` is a check error at `help.md:<line>` (inline test and fixture).
- `docs<Note>(embed_all("notes/*.md"), layout = article)` serves `/notes/{name}` per file and the index `/notes` (inline test).
- The built program contains no Markdown parser: `ignite emit` of the example reaches no fn of `x.web.markdown` (test reads the emitted IR's defines).
- `docs/slice.md` section; `task gate` green.

## Scope

- `crates/ignite/src/lib.rs` (loader, source set), `crates/ignite/src/apply/plan.rs` (source identity), `crates/ignite-eval/src` (`embed`, check-time rendering), `crates/ignite-check/src` and `crates/ignite-types/src` (the `embed` builtins).
- `x/web/markdown.ignite` (new), `x/web.ignite` (`doc`, `docs`), `docs/commonmark/` (new, vendored examples).
- `crates/ignite/tests/`, `crates/ignite-check/tests/` fixtures, `docs/slice.md`. Inferred from web §8 row 19.
- Ordering: after `story:web-live`, before `story:web-service-calls`; all three edit `x/web.ignite` and `crates/ignite-eval/src`, and this story shares the loader in `crates/ignite/src/lib.rs` with `story:web-core`.
