---
format: aep.planning-md/3
id: story:std-url-percent-encoding
kind: story
status: draft
title: Percent-encoding and query strings in std
relations:
- informed_by: story:http-client-role
revision: 1
---
# Story: percent-encoding and query strings in std

## Outcome

`std` encodes and decodes the percent-encoded parts of a URL: a path segment, a query component and `application/x-www-form-urlencoded` pairs, with the character set of each part as RFC 3986 and the URL standard give it, usable without the HTTP client.

## Why

Requested on 2026-11-12 by the author of a command-line tool written in Ignite. `story:http-client-role` brings `Url.parse` (scheme, host, port, path, query) and says nothing about encoding a value into a path or a query, or reading a query into pairs. `std/protocol/http.ignite` has a byte table for the bytes of a `reg-name` (RFC 3986 `unreserved`, `sub-delims` and `%`).

## Acceptance

- `encode_path_segment`, `encode_query_component` and their decoders, each with its own reserved set; a decoder refuses `%` followed by anything but two hex digits and refuses a result that is not UTF-8 when a `Text` is asked for (a `Bytes` form accepts it).
- `query_pairs(text) -> Result<List<(Text, Text)>, UrlError>` and `write_query(pairs) -> Text` round-trip on generated inputs; `+` is a space only in the form-urlencoded functions.
- The RFC 3986 examples that apply are tests; the requirements claimed are listed through `std.rfc` and `ignite conformance` reports them.
- The functions live where `Url` lives after `story:http-client-role`; when that story has not landed, this one takes the module decision and that story follows it.
- `docs/slice.md` section; `task gate` green.

## Scope

- `std/net.ignite` or a new `std/url.ignite`, `std/rfc/` (RFC 3986), `docs/layering.md`, `docs/slice.md`. Inferred.

## Not in this story

- `Url.parse` and the HTTP client (`story:http-client-role`).
- Internationalized host names.
