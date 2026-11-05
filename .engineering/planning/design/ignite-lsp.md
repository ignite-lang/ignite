---
format: aep.planning-md/3
id: design:ignite-lsp
kind: design
status: draft
title: 'ignite-lsp design: ESS-generated types, catalogue rules, compiler diagnostics over a child process'
relations:
- designs: epic:ignite-lsp
revision: 2
---
# ignite-lsp: a Language Server written in Ignite, derived from the LSP specification

Status: design proposal, 2026-11-04. Operator direction (Timo, 2026-11-04): "(rfc? ess) -> ignite
== ignite-lsp": the server is Ignite code, and its messages and rules come from a specification
rather than being hand-written.

## 1 Goal

An editor (Helix first, via the `ignite-helix` repository) gets diagnostics and document symbols
for `.ignite` files from `ignite-lsp`, a program written in Ignite and built by `ignite build`.
Its message types are generated from the LSP specification, its protocol rules are a catalogue
that `ignite conformance` reports against, and its diagnostics come from the compiler itself.

Not a goal of the first epic: hover, go-to-definition, completion, formatting, semantic tokens.

## 2 What exists and what is missing (2026-11-04, origin/main 8ec44030)

| need | state | evidence |
|---|---|---|
| stdin bytes, stdout write | present | `crates/ignite-types/src/builtins.rs:169,172`; used at `std/protocol/telnet.ignite:696,819` |
| Content-Length header framing | present as a pattern | `std/protocol/http.ignite:12-26` |
| JSON (escapes, null, nesting) | present, integers only | `std/json.ignite:6-20`; fractions refused `:8-10`; only `Color` uses `decimal` in the LSP meta model (5 uses) |
| process spawn | absent | only `Process.exit` (`builtins.rs:173`); `Launcher`/`ChildProcess` planned in `story:slice-17-frontend` (draft) |
| file read | absent, not needed | `Directory` in the same story; the server forwards buffers to the compiler instead |
| machine-readable diagnostics | absent | `ignite check` has no `--json` (`crates/ignite/src/main.rs:56-61`); text output carries a start position only (`crates/ignite-syntax/src/diagnostic.rs:84-87`) and stops at the first failing module (`crates/ignite/src/lib.rs:560-616`); it reads every file from disk (`lib.rs:488-491`) |
| UTF-16 positions | absent, writable in Ignite | no helper in `std/`; `Text.to_bytes()`, `Bytes.slice` exist (`std/text.ignite:51`) |
| message types from a spec | absent | LSP `metaModel.json` (microsoft/vscode-languageserver-node `protocol/metaModel.json`, 3.18.0 on main): 69 requests, 26 notifications, 387 structures, 137 untagged `or` types, 66 `extends`, 87 `mixins`; ignite-ess lowers newtype/enum/struct, optional, list, map only (`ignite-ess/crates/ignite-ess-ir/src/model.rs:317-355`), no unions, bindings HTTP and CLI only |
| protocol rules as a catalogue | partly | `std.rfc` `Requirement`/`Spec` with `rfc: Int` (`std/rfc.ignite:13-30`); `ignite rfc import` reads RFCXML only; LSP and JSON-RPC 2.0 are not RFCs |

Source: scout report of 2026-11-04 (every row re-cites its file).

## 3 Design

### 3.1 Two sources, one per half

| half | source | route |
|---|---|---|
| message types (structures, enums, notifications, requests) | `metaModel.json`, pinned by digest | importer → ESS `ess/N` domain → ignite-ess lowering → Ignite records with `to_json`/`from_json` |
| behaviour (JSON-RPC 2.0 envelopes, framing, lifecycle, MUST/SHOULD of the base protocol and of each method used) | the prose specifications | hand-written catalogue, `source = Prose`, one `Requirement` per sentence with a BCP 14 keyword |

Decision D1: types go through ESS, rules through a catalogue. Neither route covers both: the meta
model has no envelopes, framing, lifecycle or keywords; a catalogue has no types.

Decision D2: only the subset the server uses is imported (initialize/initialized, shutdown/exit,
didOpen/didChange/didSave/didClose, publishDiagnostics, documentSymbol), computed as the closure
of their references. The importer refuses an untagged `or` it cannot map instead of guessing;
`T | null` lowers to `Option<T>`, `extends`/`mixins` are flattened, `integer | string` ids get one
hand-declared ESS type until ESS has an untagged-union form.

### 3.2 Diagnostics come from the compiler process

Decision D3: the server spawns `ignite check --json --stdin-for PATH` per change (through slice
17's `Launcher`) and passes the buffer on stdin. It does not link or reimplement the front end.
`--json` prints one record per diagnostic: `{path, code, message, start, end}` with byte offsets,
and continues past a failing module where the pipeline allows. Self-hosting the front end is out
of scope.

### 3.3 Positions

Decision D4: the server offers `positionEncoding: "utf-8"` first and falls back to UTF-16 with a
pure-Ignite byte-to-UTF-16 mapper in `std.text`. Whether Helix 25.07 offers `utf-8` is not
verified; the fallback is required either way.

### 3.4 Layers and homes

| piece | home | layer (`docs/layering.md`) |
|---|---|---|
| `ignite check --json --stdin-for` | ignite, `crates/ignite` | CLI |
| `Launcher` with piped child stdin/stdout | ignite, slice 17 | substrate builtin in `ignite-io` |
| `std.protocol.jsonrpc`: framing over `ByteStream`/stdin, envelopes, id type | ignite, `std/protocol/jsonrpc.ignite` | protocol |
| catalogue of JSON-RPC 2.0 and LSP base-protocol requirements | ignite, beside `std/rfc/`, after D5 | leaf data |
| `metaModel.json` subset importer, union lowering | ignite-ess | tooling |
| `LspServer` kind, handlers, `conforms [..]` | new repository `ignite-lsp`; until it builds on its own, `examples/lsp/` in ignite | application |

Decision D5: `std.rfc.Spec` and `Requirement` gain an add-only identifier `id: Text` beside
`rfc: Int` so a non-RFC specification (`"jsonrpc-2.0"`, `"lsp-3.17"`) has an honest name; the
public shape is added to, not renamed (AGENTS.md rule on public APIs).

### 3.5 Conformance

`ignite-lsp` declares `conforms [JSONRPC2, LSPBase]` with `unsupported`/`not_applicable` lines, so
`ignite conformance` prints its claim matrix like Telnet and Http do. Session tests run the server
over `Pipe.pair()` with a scripted client.

## 4 Order

| # | step | repository | depends on |
|---|---|---|---|
| 1 | `ignite check --json --stdin-for PATH`, diagnostics of every module | ignite | none |
| 2 | `Launcher`/`ChildProcess` with piped stdin/stdout | ignite | `story:slice-17-frontend` |
| 3 | D5, then `std.protocol.jsonrpc` and the two catalogues | ignite | none |
| 4 | metaModel subset importer, union lowering, `to_json`/`from_json` | ignite-ess | ESS untagged-union request (or the D2 stopgap) |
| 5 | `LspServer`: lifecycle, text sync, diagnostics via step 1-2, documentSymbol, UTF-16 | ignite-lsp | 1, 2, 3, 4 |
| 6 | Helix wiring: `[language-server.ignite-lsp]` in `ignite-helix` | ignite-helix | 5 |

Step 1 is useful alone: any editor integration and CI can read `--json`.

## 5 Out of scope, recorded

- hover, definition, completion: need types and spans from the compiler (a `--json` symbol dump
  or self-hosting); a later epic.
- the lifecycle as an `ess-protospec/1` machine: ignite-ess does not lower protospec yet.
- decimals in JSON: only `Color` needs them; a later `std.json` change.
- the beyond10x `rfcspec` pipeline (catalogue → protospec) is related and unimplemented; not a
  dependency.

## 6 Open questions

- Does Helix 25.07 offer `positionEncoding: "utf-8"`? Answer by logging its `initialize`
  request once step 5 runs.
- Pin LSP 3.17 (a release tag) or 3.18 (main)? Default: the 3.17 release tag, since clients
  implement it.

## Decision 2026-11-05: generated types

Operator decision (Timo, 2026-11-05, "B"; hand-written types refused: "no, not handwritten"):
the LSP message types are generated straight from `metaModel.json` by a Rust generator in the
ignite repository, not through ESS and ignite-ess. This replaces the types row of §3.1 and step 4
of §4; D2 (subset closure, refuse what cannot be mapped) stays.

- Source: `metaModel.json` of the LSP 3.17 release, vendored unmodified with its licence and
  pinned by digest, as `docs/rfc/` vendors the RFCXML sources.
- Generator: a Rust binary (clap derive) in the workspace; input the vendored file and a root
  list of requests and notifications; output one Ignite module of records, enums, `Option`,
  `List`, `Map` and shape-discriminated unions, with `to_json`/`from_json` over `std.json`
  (decimals as `Decimal(Text)`).
- Drift check: a `#[test]` regenerates the module and compares it byte for byte with the
  committed file (`IGNITE_BLESS=1` rewrites it), as the goldens do.
- The ESS route stays possible later; nothing in the server depends on how the types were made.
