# std.rfc

The vocabulary of requirements and the catalogues of the specifications that std's protocols
claim. A catalogue is a module with one `Spec` constant and one `Requirement` constant per
requirement. A protocol names a spec in `conforms`, and its functions and tests name
requirements in `satisfies` and `verifies`. `ignite conformance` prints the matrix of each
claim.

- Layer: leaf, as every `std.rfc.*` module. A catalogue imports only `std.rfc`.
- Source: [`std/rfc.ignite`](https://github.com/ignite-lang/ignite/blob/main/std/rfc.ignite)
  and the catalogues under [`std/rfc/`](https://github.com/ignite-lang/ignite/tree/main/std/rfc).
- Import: `import std.rfc.{Level, Party}`; a catalogue's spec as
  `import std.rfc.rfc9112.{RFC9112}`.

## Types

```ignite,fragment
enum Level { Must, MustNot, Required, Shall, ShallNot, Should, ShouldNot, Recommended,
  NotRecommended, May, Optional }

enum Party { Any, Sender, Recipient, Client, Server, Proxy }

enum Source { Bcp14, Prose }

type Requirement {
  rfc: Int = 0
  section: Text
  ordinal: Int
  level: Level
  party: Party = Any
  source: Source = Bcp14
  minimum: Option<Int> = None
  text: Text
}

type Spec {
  rfc: Int = 0
  title: Text
  url: Text
  digest: Text
  requirements: List<Requirement>
  id: Text = ""
}
```

| item | meaning |
|---|---|
| `Level` | the requirement keywords of BCP 14 (RFC 2119, RFC 8174) |
| `Party` | whom a requirement addresses |
| `Source` | `Bcp14`: extracted from a `<bcp14>` mark; `Prose`: written by hand from an RFC that predates BCP 14, or from a specification that is no RFC |
| `Requirement.section` | the section number; `p<page>` for an RFC that numbers no sections; a dotted heading label where the specification has neither |
| `Requirement.minimum` | the least value a configured limit must have, checked against the default of the field that satisfies it |
| `Spec.digest` | the sha256 of the source text, which `ignite rfc check` compares |
| `Spec.id` | the text id of a specification that is no RFC, whose `rfc` is 0; empty for an RFC |

The levels fall into three classes: MUST (`Must`, `MustNot`, `Required`, `Shall`,
`ShallNot`), SHOULD (`Should`, `ShouldNot`, `Recommended`, `NotRecommended`) and MAY (`May`,
`Optional`). A spec's label in the matrix is its `id`, or `rfc<n>` for an RFC.

## Catalogues

| module | spec constant | specification | requirements | written |
|---|---|---|---|---|
| `std.rfc.rfc854` | `RFC854` | RFC 854, Telnet Protocol Specification | 47 | by hand |
| `std.rfc.rfc855` | `RFC855` | RFC 855, Telnet Option Specifications | 12 | by hand |
| `std.rfc.rfc1143` | `RFC1143` | RFC 1143, The Q Method of Implementing TELNET Option Negotiation | 27 | by hand |
| `std.rfc.rfc9110` | `RFC9110` | RFC 9110, HTTP Semantics, sections 5, 6, 7.2, 9.1 to 9.3 | 73 | `ignite rfc import` |
| `std.rfc.rfc9112` | `RFC9112` | RFC 9112, HTTP/1.1, sections 2 to 9 | 131 | `ignite rfc import` |
| `std.rfc.jsonrpc_2` | `JSONRPC_2` | JSON-RPC 2.0 Specification, sections 4 to 6 (`jsonrpc-2.0`) | 35 | by hand |
| `std.rfc.language_server_3_17` | `LANGUAGE_SERVER_3_17` | Language Server Protocol 3.17, the base protocol and the server lifecycle (`lsp-3.17`) | 37 | by hand |

[std.protocol.telnet](telnet.md) conforms to RFC 854 and RFC 855,
[std.protocol.http](http.md) to RFC 9112 and RFC 9110, and [std.protocol.jsonrpc](jsonrpc.md) to
JSON-RPC 2.0. No protocol module conforms to RFC 1143 or the LSP catalogue yet.

## Catalogue rules

- A requirement's constant name is its id: `R`, its `section` with each `.` as `_` and
  letters upper-cased, `_`, its `ordinal`. Section 3.2's sixth requirement is `R3_2_6`; page
  14's first is `RP14_1`.
- The ordinal counts from 1 within the requirement's innermost section, in document order.
- Every `Requirement` and `Spec` constant is literal data: literals, variants, `None`,
  `Some(n)`, and a `requirements` list of the module's own requirement names. A call, an
  alias or an interpolated text is E091.
- Each requirement is listed exactly once in the spec, and its `rfc` equals the spec's.
  Otherwise it is E091.
- The two catalogues written by `ignite rfc import` hold every sentence with a `<bcp14>` mark
  in the imported sections. Their header names the command that wrote them. See
  [`ignite rfc import`](../cli.md#ignite-rfc-import) and
  [`ignite rfc check`](../cli.md#ignite-rfc-check).

## Claims

```ignite,fragment
import std.rfc.{Party}
import std.rfc.rfc9112.{RFC9112}

protocol Http conforms [RFC9112] scope { parties = [Server, Any] } {
  fn serve<T: ByteStream, S>(stream: T, limits: HttpLimits, application: HttpApp<S, T>) -> IoResult<Unit> requires [stream.read, stream.write, application.*] satisfies [RFC9112.R2_2_3] { .. }
  unsupported RFC9112.R7_1_2 "only Content-Length framing; any Transfer-Encoding is 501"
}

test "bare CR is 400" verifies [RFC9112.R2_2_3] { .. }
```

| clause | where | meaning |
|---|---|---|
| `conforms [S, ..]` | after a protocol's name | the protocol claims every requirement of each spec in scope |
| `scope { parties = [..], sections = [..] }` | after `conforms` | narrows the claim to the requirements of those parties and sections; a missing item admits everything |
| `satisfies [S.Id, ..]` | a fn, role, type or field | the declaration implements the requirements |
| `verifies [S.Id, ..]` | a test | the test is evidence for the requirements |
| `unsupported S.Id "reason"`, `not_applicable S.Id "reason"` | a protocol body | the requirement's status, with its reason |

A requirement reference is `Spec.Id`; the requirement need not be imported. Diagnostics:

- A MUST-class requirement in scope without a status, or one with two statuses, is E090.
- A `satisfies`, `verifies` or status that names a spec no protocol of the module conforms
  to is E092.
- A SHOULD or MAY without a status is no error; its matrix row is `unclassified`.

[`ignite conformance`](../cli.md#ignite-conformance) runs the module's tests and prints, per
protocol and spec, each requirement's status, evidence and verdict, and whether the claim holds.

## Example

```ignite
import std.rfc.{Level, Requirement}
import std.rfc.rfc855.{RFC855, RP2_4}

test "a catalogue is literal data" {
  assert RFC855.rfc == 855
  assert RFC855.requirements.len() == 12
  assert RP2_4.level == May
  assert RP2_4.section == "p2"
}
```
