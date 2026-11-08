---
format: aep.planning-md/3
id: story:markup-views
kind: story
status: draft
title: 'view blocks: HTML as typed element functions with checked forms'
relations:
- decomposes: epic:x-web
- depends_on: story:web-collection
- depends_on: story:given-and-defaults
- depends_on: story:web-openapi-client
scope:
- confidence: inferred
  path: crates/ignite-check/src
- confidence: inferred
  path: crates/ignite-eval/src
- confidence: inferred
  path: crates/ignite-syntax/src
- confidence: inferred
  path: crates/ignite-types/src
- confidence: inferred
  path: docs/html
- confidence: inferred
  path: docs/slice.md
- confidence: inferred
  path: docs/syntax.md
- confidence: inferred
  path: x/web.ignite
- confidence: inferred
  path: x/web/html.ignite
revision: 5
---
# Story: markup-views

## Outcome

A program writes HTML as a `view`: `view board(todos: Store<Todo>) =` followed by an indented block of element lines, `for`, `if`, `match`, `let` and calls of other views, where each element is a typed function of the markup provider `x.web.html`, text is escaped, and a form's route and input names are checked against the route table at `ignite check`; `page(view)` serves it (X10 of `docs/web.md` §5, §6.13).

## Context

- web §6.13 table of line forms; an element lowers to nested calls of the provider (`fn form(action: Route, method: Method = get, …) -> Html`), so optional attributes need parameter defaults (`story:given-and-defaults`). The `view` block is the only indentation-sensitive syntax; it ends at the first line indented no deeper than `view`.
- web §6.2: a form whose `action`/`method` names no route, or whose input names are not fields of that route's input type, is E120 (facts recorded for the evaluator). A URL attribute takes `Route` or `Url`, so `javascript:` cannot be written.
- Q4 default: elements and global attributes of the HTML Living Standard, from a vendored data file as `docs/rfc/` vendors RFCs.
- The compiler crates name no HTML (layering rule 5): element names resolve as functions of whatever provider the file imports.
- The input type of a route is that of `collection` (`New<T>`, `story:web-collection`) or of a hand route (`story:web-core`).

## Acceptance

- The `board` view of web §1 checks and `page(board)` answers `GET /` with `text/html; charset=utf-8`; the inline test compares the body with the expected HTML for two todos, one done (`class="done"`), on both targets.
- Through the test client of `story:web-openapi-client`: `client(TodoService.server).get("/").body` is the rendered page (web §6.8; inline test).
- Escaping: a title `<script>&"` appears as `&lt;script&gt;&amp;&quot;` in text and attribute positions (inline test).
- Each line form of the web §6.13 table has an inline test: static class and id (`li.done#first`), positional and named arguments, a bare `Bool` attribute, `| text`, `{expr}` with `Html`, `Text` and a list, `for`, `if`/`else`, `match`, `let`, a nested view with children.
- Fixtures with `.expected`: an unknown element name; an attribute of the wrong type; `form("/todo", post)` with no such route (E120); `input("titel")` that is no field of `New<Todo>` (E120); `a(href = "javascript:x")` refused by type; a line indented inconsistently (syntax error with a range).
- Evaluator, Rust and LLVM render one view to equal bytes (test in `crates/ignite/tests/`).
- `docs/slice.md` section, `docs/syntax.md` entry; `task gate` green.

## Scope

- `crates/ignite-syntax/src` (the `view` block), `crates/ignite-check/src` (elements resolve through the provider, form facts), `crates/ignite-types/src`, `crates/ignite-eval/src` (facts checked against the route table).
- `x/web/html.ignite` (new: provider and vocabulary), `x/web.ignite` (`page`), `docs/html/` (new: vendored element data and its licence note).
- `crates/ignite/tests/`, `crates/ignite-check/tests/` fixtures.
- Ordering: after `story:web-openapi-client` and `story:given-and-defaults`, which share `crates/ignite-eval/src`, `crates/ignite-check/src`, `crates/ignite-types/src` and `docs/syntax.md` with this story.
- `docs/slice.md`, `docs/syntax.md`. Inferred from web §8 row 6.
