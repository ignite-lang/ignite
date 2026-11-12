---
format: aep.planning-md/3
id: story:data-xml
kind: story
status: draft
title: 'std.xml: a reader and writer for XML 1.0 without DTDs'
relations:
- informed_by: epic:data-formats
revision: 1
---
# Story: std.xml, a reader and writer for XML 1.0 without DTDs

## Outcome

`std.xml` reads a well-formed XML 1.0 document into a tree of elements, attributes and text, and writes one, with namespaces resolved, within stated limits, written in Ignite.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite, beside YAML (`story:data-yaml`) and TOML (`story:data-toml`). `epic:data-formats` names JSON, YAML and TOML; XML is not in it. The compiler has a hand-written RFCXML reader in `crates/ignite/src/rfc/` for `ignite rfc import`; it is Rust and not reachable from a program.

## Acceptance

- `Xml.read(bytes, limits) -> Result<Document, XmlError>`: elements, attributes, character data, CDATA sections, comments and processing instructions (kept or dropped by an option), the five predefined entities and character references, UTF-8 input with or without a byte order mark.
- A document type declaration is refused with an error that names it: no internal subset, no external entity, no entity expansion. A test pins the refusal of a nested-entity document.
- Limits (`XmlLimits`): depth, attributes per element, total nodes, name and value length; each has a test at the bound and one past it.
- Namespaces: `xmlns` and prefixed names resolve to (namespace, local name); an unbound prefix is an error.
- `Xml.write(document) -> Text` escapes what must be escaped; `read(write(d)) == d` on generated documents.
- An error carries line and column. The design step states how a `Document` maps to and from `std.data` `Value`, or that it does not.
- `docs/layering.md` lists the module; `task gate` green.

## Scope

- `std/xml.ignite` (new), `docs/layering.md`, `crates/ignite-check/tests/layering.rs`, `docs/slice.md`, `docs/serde.md`. Inferred.

## Not in this story

- DTD validation, XML Schema, XPath, XSLT, streaming (SAX-style) reading.
