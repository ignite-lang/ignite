---
format: aep.planning-md/3
id: story:std-html-escape
kind: story
status: draft
title: std escapes text for HTML
relations:
- informed_by: story:markup-views
revision: 1
---
# Story: std escapes text for HTML

## Outcome

`std` has one function that makes a `Text` safe to place in HTML element content and in a quoted attribute value, so a program that writes a page by concatenation does not write its own.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite, found while it built an HTTP server on `std.protocol.http`: its pages are rendered by a hand-written escape function and concatenation. `std` has no HTML escape (`std/text.ignite` and `std/encoding.ignite` hold none). `story:markup-views` escapes text inside a `view` and is a draft that depends on three other stories; a program that serves two or three pages needs the function long before it needs views.

## Acceptance

- `escape_html(text) -> Text` replaces `&`, `<`, `>`, `"` and `'` by `&amp;`, `&lt;`, `&gt;`, `&quot;` and `&#39;`, and nothing else.
- A property test on generated texts: the result holds no `<`, `>`, `"` or `'`, every `&` in it starts one of the five references, and replacing the five references back gives the input.
- A text without any of the five characters comes back equal; the allocations of that case on the Rust target are counted and written down.
- The doc comment names the two places the result is safe in (element content, a quoted attribute value) and the places it is not (an unquoted attribute, a URL, a `script` or `style` element).
- An example or an inline test serves one page through `std.protocol.http` whose body holds an escaped value; `ignite test` passes it on both targets.
- `docs/slice.md` lists the function; `task gate` green.

## Scope

- `std/text.ignite` or a new `std/html.ignite` (the module decision is part of this story), `docs/layering.md` when a module is added, `docs/slice.md`. Inferred.

## Not in this story

- Templating and typed element functions (`story:markup-views`).
- Percent-encoding of a URL placed in an attribute (`story:std-url-percent-encoding`).
- Unescaping, and parsing HTML.
