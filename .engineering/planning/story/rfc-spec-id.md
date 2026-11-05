---
format: aep.planning-md/3
id: story:rfc-spec-id
kind: story
status: active
title: Non-RFC specifications in std.rfc, and the JSON-RPC 2.0 and LSP base-protocol catalogues
relations:
- decomposes: epic:ignite-lsp
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T08:03:38Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-05T08:03:38Z", actor: "human:timo", revision: 3}
---
# Non-RFC specifications in std.rfc, and the JSON-RPC 2.0 and LSP base-protocol catalogues

Step 3b of `design:ignite-lsp` (§3.4 D5, §3.5, §4 row 3), decided by sys-mon on 2026-11-05 under
Timo's mandate. `std.rfc.Spec` and `Requirement` carry `rfc: Int` (`std/rfc.ignite:13-30`);
JSON-RPC 2.0 and the Language Server Protocol are not RFCs, so a catalogue of them has no honest
`rfc` value.

## Outcome

- D5, add-only: a specification has a text identifier (`"jsonrpc-2.0"`, `"lsp-3.17"`) beside
  `rfc: Int`. Every existing catalogue, protocol claim, `ignite conformance` matrix and
  `ignite rfc import`/`check` output is unchanged.
- Two hand-written catalogues (`source = Prose`): JSON-RPC 2.0 (sections 4 to 6: request,
  notification, parameters, response, error object; batch marked) and the LSP 3.17 base
  protocol (header part, content part, request/notification/response messages, cancellation,
  lifecycle: initialize first, shutdown then exit), one `Requirement` per sentence with a
  BCP 14 keyword, quoted verbatim, URL and section per requirement.
- `protocol JsonRpc` declares `conforms` against the JSON-RPC 2.0 catalogue with
  `unsupported`/`not_applicable` lines (batch is unsupported), and `ignite conformance
  std/protocol/jsonrpc.ignite` prints its matrix with every claim holding or stated.

## Acceptance

1. The existing golden outputs (conformance matrices of Telnet and Http, `ignite rfc check`
   over the vendored RFCs, model goldens) are byte-identical.
2. A catalogue with the text id and no RFC number checks; E090 to E092 still fire for their
   fixtures, and a new fixture shows the diagnostic for a non-RFC claim that does not hold.
3. `ignite conformance std/protocol/jsonrpc.ignite` exits 0 on both targets and lists the
   JSON-RPC 2.0 requirements with their verdicts.
4. The layering test passes with the catalogues' module names.
5. Stop condition: if any step needs a rename, a removed field or a changed type, the change is
   not add-only; stop and report instead.

## Out of scope

The LSP message-type import (step 4, ignite-ess), the LSP server (step 5), decimals
(`story:json-decimals`).

## Adversary findings

Adversary pass over acbbc658 and 1c7107e7, 2026-11-05 (failing tests in 9c3520cf). Add-only held:
Telnet and Http matrices, `ignite rfc check`, `ignite plan` and `ignite check --json` outputs are
byte-identical to the parent build; all 35 JSON-RPC and 30 LSP texts verbatim; digests match.
Findings and what became of them (831b72d4, 2c747e00):

- fixed: a batch of only notifications was answered; now it gets no reply.
- fixed: two specs could share one label (`id = "rfc1"` beside RFC 1, two equal text ids); E091
  now rejects an `rfc<digits>` id, a label used twice and a Spec with neither `rfc` nor `id`.
- fixed: E091 said "is RFC 0" for a non-RFC spec; it now names the label.
- fixed: the LSP catalogue missed Register Capability, setTrace and logTrace; now 37
  requirements.
- deferred: decimals in params or id are answered with -32700 under a null id; R4_1_1, R4_6 and
  R5_9 are `unsupported` naming `story:json-decimals`, so the claim reads no (3 gaps). The three
  tests are kept in that story.
- kept: `"id": null` requests are refused (LSP forbids them); R4_4 is a Client row, not claimed.
- rule change, not add-only: layering rule 3 exempts a `std.rfc.*` catalogue that the protocol
  it quotes imports (2c747e00, its own commit); `JSON_RPC_2` renamed `JSONRPC_2`.

Gate: two runs failed on a race in `apply::host::recovery` tests (ETXTBSY copying and spawning
`/bin/sleep`, `recovery.rs:719,806,873`), untouched by this branch; the ignite session fixes it
in `fix/llvm-runtime-embedded`. The third run passed.
