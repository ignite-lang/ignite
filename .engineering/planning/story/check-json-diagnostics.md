---
format: aep.planning-md/3
id: story:check-json-diagnostics
kind: story
status: active
title: ignite check --json prints machine-readable diagnostics
relations:
- decomposes: epic:ignite-lsp
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-04T23:31:49Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-04T23:31:49Z", actor: "human:timo", revision: 3}
---
# ignite check --json: machine-readable diagnostics

Step 1 of `design:ignite-lsp` (§4). Any editor integration, the Ignite-written LSP and CI read
diagnostics from this instead of parsing text.

## Outcome

`ignite check FILE --json` prints one JSON document on stdout:
`{"diagnostics": [{"path", "code", "message", "start": {"line", "column", "offset"}, "end": {...}}]}`
with `offset` in bytes from the file start, `line`/`column` 1-based (column in chars, as the text
form), and the exit code unchanged from the text form. `--stdin-for PATH` reads the program text
for PATH from stdin instead of disk, so an unsaved editor buffer is checked; siblings and std
still come from disk.

## Acceptance

1. A file with a type error: `--json` prints exactly one diagnostic with its code (e.g. E040),
   message identical to the text form, and start/end byte offsets that slice the offending
   source text.
2. A clean file: `{"diagnostics": []}`, exit 0.
3. A syntax error: E001/E002 reported in the same shape.
4. `--stdin-for PATH` with a buffer that differs from the file on disk reports the buffer's
   diagnostics, and leaves the file untouched.
5. A diagnostic in a sibling module carries that module's path.
6. The text form of `ignite check` is byte-for-byte unchanged (existing tests stay green).

## Out of scope

Diagnostics of every module past the first failing stage (design §3.2 "where the pipeline
allows") is a follow-up story unless it falls out for free; hover/definition data.

## Adversary findings

Adversary pass over 556b19e, 2026-11-05: no confirmed defect. Text form identical to the parent
build over all 263 fixtures; the JSON of all 179 failing fixtures rebuilds the text form exactly
(57 distinct codes, E001 to E129); offsets, escaping, CRLF, BOM, empty file, invalid UTF-8 stdin
and usage errors checked. Edge cases outside the contract, not fixed here:

- `--stdin-for <directory>` checks the buffer and exits 0, where `ignite check <directory>`
  exits 3.
- `--stdin-for dir/x.ignite` with a `package` line and a missing `dir` exits 3, because
  `parse_siblings` reads the directory (`crates/ignite/src/lib.rs:911`); an LSP buffer for a file
  in a not-yet-created directory hits this.
- Any command writing to a full stdout panics with exit 101 (`ignite plan` too); predates this
  change.
- One non-UTF-8 `*.ignite` sibling makes every check of a `package` program in that directory
  exit 3 (`lib.rs:928` reads before `declares_package`); predates this change.

