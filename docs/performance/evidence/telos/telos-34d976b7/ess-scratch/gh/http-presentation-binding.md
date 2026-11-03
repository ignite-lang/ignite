# HTTP presentation binding (`ess-http/1`)

Status: binding for https://github.com/beyond10x/ess/issues/493; stage 1 is built in wave
2026-10-11j (2026-10-11). The scope is the operator's decision on
`decision-blocker:http-surface-scope` (option (b), the staged presentation binding). This page
covers the whole format before any stage is built, so stage 1 fixes no key that stage 2 would have
to bump. Stage 1 is planned for 0.60.0, stage 2 for the release after it, and stage 3 after that
(`decision-blocker:ess-060-wave-scope`). Each section below says which stage it belongs to. Stage
2's open questions are answered on 2026-10-13 ("Decided for stage 2"), and "Stage 2: keys,
precedence and the units that build them" fixes every stage-2 key, refusal and precedence before
any of its units is built; stage 2 is planned for 0.61.0.

## Problem

ESS chooses every HTTP wire shape it projects:

- the route, `POST /{domain wire}/commands/{command wire}` and `GET /{domain wire}/views/{view wire}`
  (`crates/generate/ess-gen/src/http.rs:448-473`);
- two methods only, `GET` and `POST` (`http.rs:320-332`);
- one media type, `application/json` (`crates/generate/ess-gen/src/openapi.rs:223-225`);
- a response envelope: `outcome`, `published` and `response` for a command
  (`openapi.rs:57-70`, `openapi.rs:231-234`), and `{rows: [...]}` for a view (`openapi.rs:1080-1091`);
- one status per kind of outcome (`http.rs:173-239`).

A published HTTP protocol fixes these itself. RFC 8414 §3 fixes
`GET /.well-known/oauth-authorization-server`, OpenID Connect Discovery 1.0 §4 fixes
`/.well-known/openid-configuration`, and both answer with one bare JSON object. RFC 6749 leaves
the token endpoint's path to each server (§3.2), takes a form-encoded request (§4.1.3), and answers
`400 {"error": …}` (§5.2). RFC 6750 §3 answers a refused bearer token with `401` and a
`WWW-Authenticate` challenge whose `error` attribute names the refusal.

None of this can be stated today. A command refuses an `http:` key
(`` unknown field `http` ``, reproduced in the issue). The model holds no path or verb
(`crates/specify/ess-domain/src/component.rs:259-261`, "No variant names a wire, a port, a path or a
verb"). A realization entrypoint carries one base URL, not a route per operation
(`crates/specify/ess-realization/src/lib.rs:350-364`). So an external implementation's conformance
adapter holds the operation-to-URL mapping in code that nothing checks against the model.

## Decision

A sibling document, `ess-http/1`, binds the commands and views of **one exact ESS** to an HTTP
surface. It is to a command or view what `ess-transport/1` is to an event: authored separately,
pinned to the specification's source digest, compiled against the resolved IR into a deterministic
`ess-http-ir/1`, and never part of `ess/N` (`docs/design/event-transport-binding.md:17-23`). The
model does not change when the same command is served at another path; the binding changes
instead. It follows `ess-cli/1` in owning the whole presentation of an operation, not only its
location (`docs/design/cli-presentation-binding.md:276-294`), and differs from it in pinning the
specification, which `ess-cli/1` does not (`crates/specify/ess-cli-contract/src/lib.rs:12-22`).

Without a binding, every artifact keeps its bytes. That is the rule `--transport` already keeps for
AsyncAPI (`event-transport-binding.md:100-102`).

| stage | adds | release |
|---|---|---|
| 1 | the document, its envelope, the method and path of a command or view, path parameters, refinement, the OpenAPI projection | 0.60.0 |
| 2 | request encoding, response body per outcome, status per outcome, response headers including `WWW-Authenticate`, redirects, overrides for the standard refusals, the Rust and Go servers' template dispatch (both servers built, units k5 and k6) | the release after 0.60.0 |
| 3 | an HTTP conformance target that reaches an implementation through the binding | after stage 2 |

## Stage 1: the document and the route

### Envelope

```yaml
type: ess-http/1
specification:
  system: catalog
  version: v1
  source_digest: sha256:<hex>
refines: sha256:<hex>          # optional; the digest of the binding this one fills in
routes:
  - view: catalog.discovery.ServerMetadata
    method: get
    path: /.well-known/oauth-authorization-server
  - command: catalog.orders.PlaceOrder
    method: post               # no path: each server chooses it
  - view: catalog.items.ItemById
    method: get
    path: /items/{item_id}
    parameters:
      item_id: param.item_id
```

| key | meaning |
|---|---|
| `type` | exactly `ess-http/1` |
| `specification` | `system`, `version` and `source_digest` of the IR, the same closed object `ess-transport` reads (`crates/specify/ess-transport/src/lib.rs:34-44`) and checks (`lib.rs:514-533`) |
| `refines` | `sha256:<hex>` of the bytes of the binding this one refines. Absent, the document refines nothing |
| `routes[].command` / `routes[].view` | exactly one of the two: a command's or a view's qualified name |
| `routes[].method` | the closed lower-case set the OpenAPI import already reads: `get`, `put`, `post`, `delete`, `options`, `head`, `patch`, `trace` (`crates/generate/ess-openapi/src/lib.rs:132-152`) |
| `routes[].path` | a path template; absent, the route is open (below) |
| `routes[].parameters` | template variable to source: `input.<field>` for a command, `param.<name>` for a view |

Every object rejects unknown fields. The parameter spelling is the binding-mapping source spelling
that `ess-transport/2` reused for subject tokens (`docs/design/parameterized-event-channel-addresses.md:56-58`),
and `param.<name>` is what a view's `filter:` already reads. There is no expression language: a
source names one field.

### Two levels in one format

The transport format already splits what a contract fixes from what each deployment chooses:
`brokers[].host` is declared "when the contract fixes it" (`event-transport-binding.md:61`). Routes
split the same way:

- A **protocol binding** is published with the specification. It declares a path where the protocol
  fixes one, and leaves `path` out where the protocol leaves it to the server. A route without a
  path is **open**.
- A **server binding** names the protocol binding in `refines:` and fills in every open path. It
  restates each route of the refined binding. It may not change a method or a path the refined
  binding fixed. It may add routes for operations the refined binding does not name.

```yaml
type: ess-http/1
specification: {system: catalog, version: v1, source_digest: sha256:<hex>}
refines: sha256:<digest of the protocol binding above>
routes:
  - {view: catalog.discovery.ServerMetadata, method: get, path: /.well-known/oauth-authorization-server}
  - {command: catalog.orders.PlaceOrder, method: post, path: /api/orders}
  - {view: catalog.items.ItemById, method: get, path: '/items/{item_id}', parameters: {item_id: param.item_id}}
```

The server binding belongs to the implementation, not to an environment: the path a server's code
serves is the same everywhere, and the base URL is what an environment changes
(`crates/generate/ess-deployment/src/runtime.rs` lowers ports and base endpoints; the realization
entrypoint carries the base `url`, `ess-realization/src/lib.rs:358-363`). A refinement refines one
document; a refinement of a refinement is refused in `/1`.

### Path templates

- starts with `/`; the root `/` alone is a path;
- `/`-separated segments, none empty, so no trailing `/` except the root;
- no `?`, no `#`, no percent-encoding, and no segment that is `.` or `..`. That is the same reason the
  specification already refuses such wire names (`crates/specify/ess-domain/src/wire.rs:95-116`): a
  client that removes dot-segments (RFC 3986 §5.2.4) asks for another route;
- a literal segment is RFC 3986 `pchar` text without `%`;
- a variable is a **whole** segment, `{name}`, where `name` is an identifier. `/items/{id}.json` is
  refused: a partial-segment template needs an escaping rule this format does not state.

Each variable is mapped exactly once in `parameters`, and each mapping names a variable of the path.
A command's mapped input field leaves the request body. A view's mapped parameter leaves the query
string; its other parameters stay query parameters exactly as today (`openapi.rs:1770-1815`). A
mapped source is a scalar a path segment can carry: `String`, `Integer`, an enum, or an unconstrained
newtype of one of them. It is never `Optional`, `List`, a struct or `Json`, because a path segment is
always present and holds one value.

A view parameter that stays in the query string is named there by its flat `wire:`, as a command's
field is: the OpenAPI `in: query` parameter, the Rust and Go servers' query readers and the `rows`
or `single` they write all use the wire name, and the model name is an unknown key. A view field
takes its wire name the same way. `naming: {wire}` is not a view field's key and is refused as an
unknown field.

### Which operations a binding may name

- A command a component `accepts`. Every such command has an OpenAPI route today (`http.rs:392-411`).
- A view of a domain owned by a `reached_by: network` component. A view has no route anywhere else
  (`http.rs:413`), and binding one would invent a query surface the specification did not declare.
- A view is bound to `get` only. A command is bound to `post`, `put`, `patch` or `delete`, or to
  `get` only when every input field is mapped to the path or the route is `request.encoding: query`.
  `head`, `options` and `trace` are refused for both in `/1`.

An operation the binding does not name keeps its derived route. Collision checks run over declared
and derived routes together, per component, because each component is one surface and one OpenAPI
document (`openapi.rs:500-510`).

### What compiling checks (stage 1)

Every refusal is returned at once, with a stable code and the path of the offending key, as
`ess-transport` does (`ess-transport/src/lib.rs:465-512`).

| code | refused | names |
|---|---|---|
| `ESS-HTTP-001` | `type` is not `ess-http/1` | `type` and the value found |
| `ESS-HTTP-002` | `specification` disagrees with the IR | `specification.<key>`, the value found and the IR's |
| `ESS-HTTP-003` | `refines` is not `sha256:` and 64 lower-case hex digits, `refines` given and the refined file not supplied, its digest differs, or the refined file itself refines; or a refined file supplied to a document without `refines` | `refines`, both digests. The refined file is checked against the same specification, and its own refusals are keyed under `refined.` |
| `ESS-HTTP-004` | a route names no command or view, names both, or names one that does not exist | `routes[i]` and the name |
| `ESS-HTTP-005` | one operation routed twice in one document | both `routes[i]` paths and the operation |
| `ESS-HTTP-006` | a method outside the closed set, or not admitted for that kind of operation | `routes[i].method`, the value, the kind |
| `ESS-HTTP-007` | a path that breaks the template grammar | `routes[i].path` and the offending segment |
| `ESS-HTTP-008` | a variable not mapped, mapped twice, reading a source another variable of the route already reads (a request path would give it two values), or a mapping for no variable | `routes[i].parameters.<name>`; for a source read twice, the second variable of the path that reads it |
| `ESS-HTTP-009` | a source in the wrong namespace, naming no field, or of a type a segment cannot carry | `routes[i].parameters.<name>`, the source, its type |
| `ESS-HTTP-010` | two operations of one component on one method and template (variable names ignored), declared or derived | both operations; the message reuses the import's wording, "two operations claim the same method and path" (`ess-openapi/src/lib.rs:76-80`) |
| `ESS-HTTP-011` | two templates of one component and one method that overlap: of equal length, equal up to variable names before the first position they differ, a literal beside a variable at that position, and no later position holding two different literals (`/items/current` beside `/items/{item_id}`; `/items/{item_id}/a` beside `/items/current/b` does not overlap) | both operations and the segment |
| `ESS-HTTP-012` | a refinement that changes a fixed method or path, leaves a fixed path open, or drops a route of the refined binding | `routes[i].method` or `routes[i].path`, the refined value and the new one; `routes` and the refined `routes[j]` for a dropped route |
| `ESS-HTTP-013` | an operation that has no HTTP surface: a command no component accepts, a view no network component serves | the operation |
| `ESS-HTTP-014` | a command on `get` with an input field that is neither mapped to the path nor (stage 2) query-encoded | the command and the field |

A stage-1 reader refuses every stage-2 key as an unknown field naming it. `ess-cli/1` took the same
route when the `trailing` kind arrived in 0.54.0 without a format change: an older reader refuses it
by name (`cli-presentation-binding.md:256-260`).

Validation does not refuse an open route. A document with an open route is valid and compiles; the
artifacts that need a path refuse it (below).

### Command surface (stage 1)

Mirroring `ess specify transport` (`crates/edge/ess-cli/src/main.rs:211-215`):

```console
ess specify http validate --path MODEL --binding catalog-http.yaml [--refines protocol-http.yaml]
ess specify http compile  --path MODEL --binding catalog-http.yaml [--refines …] --out FILE
ess generate --path MODEL --kind openapi --http catalog-http.yaml [--refines …] --out DIR
```

`--http` is refused with any other `--kind`, as `--transport` is refused without `--kind asyncapi`
(`main.rs:1626-1627`). `ess generate synthesize` accepts `--http` for the Rust and Go targets only,
from stage 2: 0.60.0 ships stage 1 without it, so the servers' template dispatch below lands with
stage 2 and `ess generate synthesize` has no `--http` flag until then.

### What generated artifacts change (stage 1)

`ess_gen::http::routes` is the one route table every HTTP artifact reads (`http.rs:1-12`,
`http.rs:387`). Stage 1 gives it an optional compiled binding as `ess_gen::http::bound_routes`, which
takes `Option<&ess_gen::http::Binding>`. Without one it returns exactly what `routes` returns, and
`routes` keeps its signature, so every reader below keeps its bytes and its call.

| artifact | today | with a binding |
|---|---|---|
| OpenAPI | `routes()` becomes `paths` (`openapi.rs:505`) | declared methods and paths; path parameters as `in: path`, `required: true`; a mapped command field leaves `requestBody`. Each bound operation carries `x-ess-route-source: binding`, as a bound channel carries `x-ess-address-source: transport` (`event-transport-binding.md:91`). A binding with an open route is refused, naming the operation |
| Rust server | exact `match request.path.as_str()` and one method per path (`crates/generate/ess-synth/src/rust/http.rs:824-828`) | template dispatch and several methods on one path; `/openapi.json` and `/docs` (`http.rs:42-45`, served at `rust/http.rs:831-834`) stay reserved, and a route claiming either is refused at synthesis |
| Go server | `switch request.URL.Path` (`crates/generate/ess-synth/src/go/http.rs:1132`, `go/http.rs:1300`) | the same as Rust |
| view queries, view-grant synthesis | read `routes()` (`crates/generate/ess-synth/src/view_query.rs:187`, `crates/verify/ess-conformance/src/synthesize/view_grant.rs:64`) | follow `routes()` |
| UI binding and React adapter | build from `routes()` (`crates/ui/ess-ui-check/src/model.rs:1605`, `crates/ui/ess-ui/src/binding.rs:7`) | `ess ui` refuses `--http` by name until a story admits it |
| emitted conformance runners, web bridge, interpreter | no route (`crates/generate/ess-synth/src/web/bridge.rs:1-10`) | untouched |
| generated documentation | prints no route | untouched |

### Formats and determinism (stage 1)

- New: `ess-http/1` (authored) and `ess-http-ir/1` (compiled, canonical JSON).
- The IR orders routes by path, then method, the order `routes()` already uses (`http.rs:440-444`),
  and holds every map as a `BTreeMap`. It records the refined binding's digest and the derived routes
  it was checked against, so a reader sees the whole surface without recomputing it.
- Unchanged: `ess/N` (the `http:` key on a command or view stays an unknown field), `ess-realization/1`
  and `/2`, `ess-transport/1` and `/2`, `ess-cli/1` and `/2` with their plans, every conformance
  document, and every artifact generated without `--http`.

## Stage 2: encoding, bodies, statuses, headers and redirects

Stage 2 adds keys to a route. A route with none of them keeps ESS's own wire shape: JSON request,
the envelope, the derived statuses.

```yaml
type: ess-http/1
specification: {system: catalog, version: v1, source_digest: sha256:<hex>}
refines: sha256:<hex>
refusals:
  malformed: {status: 400, body: {error: {literal: invalid_request}}}
routes:
  - command: catalog.orders.PlaceOrder
    method: post
    path: /api/orders
    request:
      encoding: form            # json | form | query
      unknown: ignore           # refuse | ignore
    outcomes:
      placed:
        status: 200
        body: response
        headers:
          Cache-Control: {literal: no-store}
      out-of-stock:
        status: 400
        body: {error: error}    # {"error": <error wire name>, …the error's own fields}
  - view: catalog.items.ItemById
    method: get
    path: /items/{item_id}
    parameters: {item_id: param.item_id}
    body: single
    empty: {status: 404}
  - command: catalog.items.ReadPrivateItem
    method: get
    path: /private/items/{item_id}
    parameters: {item_id: input.item_id}
    outcomes:
      token-refused:
        status: 401
        body: {error: error}
        headers:
          WWW-Authenticate:
            challenge:
              scheme: Bearer
              params:
                realm: {literal: catalog}
                error: {source: error.wire}
  - command: catalog.orders.ApproveOrder
    method: get
    path: /approve
    request: {encoding: query}
    outcomes:
      approved:
        status: 302
        redirect:
          location: input.return_to
          mode: query           # query | fragment
          parameters:
            code: response.code
            state: input.state
```

### Request encoding

| `encoding` | request |
|---|---|
| `json` (default) | today's body, `application/json` (`openapi.rs:223-225`), less any path-mapped field |
| `form` | `application/x-www-form-urlencoded`, UTF-8 (RFC 6749 §4.1.3, Appendix B) |
| `query` | no body; every unmapped input field in the query string. The only encoding a `get` command may use, and one a `delete` command may use |

A `form` or `query` field is a scalar, an `Optional` scalar or a `List` of scalars. A `List` repeats
its key once per element, the rule view parameters already follow (`openapi.rs:1776-1778`). A key a
scalar field receives twice is a malformed request (RFC 6749 §3.1). Booleans are `true` and `false`,
integers exact decimal text. Keys are the fields' wire names. `unknown: ignore` admits a request
parameter no field names (RFC 6749 §3.1, "MUST ignore unrecognized request parameters"); the
default, `refuse`, keeps ESS's closed bodies. A key whose percent-decoding fails names no field,
because every declared name is UTF-8, so it is unknown: ignored with its value under `ignore`, and
malformed under `refuse`.

### Response body per outcome

| `body` | for | wire |
|---|---|---|
| `envelope` (default) | any outcome | today's body: `outcome`, `error`, `payload`, `published`, `response` (`openapi.rs:57-70`) |
| `response` | a `returns: true` outcome without `error` | the command's declared response object, bare |
| `response.<field>` | a `returns: true` outcome without `error`, naming a response field that is not `Optional` | that field's value alone, bare: a JSON string, number, boolean, list or object, and `null` only from a `Json` field that holds it |
| `{<member>: error}` | an outcome with `error` | an object whose `<member>` is the error's wire name (`naming.wire`, falling back to the local name, `crates/specify/ess-domain/src/name.rs:240-242`), beside the error's own fields under their wire names, unchanged (decided for stage 2, answer 9) |
| `{<member>: {literal: <text>}}` | a standard refusal (below) | an object with one fixed member |
| `none` | any outcome | no body; required with `204` and every redirect |
| `rows` (default) | a view | today's `{rows: [...]}` |
| `single` | a view selected by its source entity's identity (decided for stage 2, answer 7) | the one row, bare; with `empty: {status, body}` answering a view that holds none, `body` being `none` (the default) or `{<member>: {literal: <text>}}` |
| `array` | a view that is not paged | the rows, a bare JSON array, `[]` when the view holds none |

A bare body carries no `outcome` and no `published`. A caller of such a route sees the branch only
through status, body shape and headers, and sees no published event. That is what the protocol
says, and stage 3 counts what it costs.

An outcome's `encoding` says how its body is written:

| `encoding` | for | wire |
|---|---|---|
| `json` (default) | any body | the body as JSON, `application/json` |
| `text` | `response.<field>` naming a `String` field (a newtype of one counts) | the string alone, its UTF-8 bytes, `Content-Type: text/plain; charset=utf-8`; `OpenAPI` writes the field's schema under `text/plain` |

`text` beside any other body, on a field of another type, or under `errors` is refused
(`ESS-HTTP-024`), as is any word but `json` and `text`. Compiled IR writes `encoding` only when it
is `text`. An adopter asked for a token endpoint that answers a bare string
(`story:ess-http-admits-text-bodies-route-refusals-empty-bodies-and-array-views`).

### Status per outcome

`status` is an integer. An outcome the binding does not list keeps `outcome_status`
(`http.rs:233-239`). Refused:

- `1xx` for anything;
- `2xx` for an outcome with `error`, and `4xx` or `5xx` for one without. `3xx` is admitted for both,
  because RFC 6749 §4.1.2.1 answers an authorization error with a redirect, but only the five
  redirect statuses `301`, `302`, `303`, `307` and `308`: `300`, `304`, `305` and `306` are refused
  in stage 2 (decided for stage 2, answer 15);
- two outcomes of one operation on one status unless the bodies tell them apart: both `envelope`
  (the `outcome` member does), or both `{<member>: error}` on one member with distinct error wire
  names; or, on a redirect status, unless both declare a `redirect` and one of them always writes
  a parameter the other never writes. A parameter is always written when its source is not
  `Optional`. An OAuth authorization endpoint answers success and error both with `302`, the
  success always writing `code` and the error always writing `error` (RFC 6749 §4.1.2,
  §4.1.2.1), and compiles. Two redirects are also told apart when both write one parameter from
  `error.wire` and their error wire names differ, as two `{<member>: error}` bodies are: every
  authorization error redirects with its own `error` (RFC 6749 §4.1.2.1), so `access_denied` and
  `invalid_scope` on one `302`, both `parameters: {error: error.wire}`, compile, and two outcomes
  reporting one error do not. Two redirects whose always-written names are equal, or where each
  always-written name of one is one the other may write, are otherwise refused. Otherwise a reader,
  and the stage-3 target, cannot say which branch was taken. Two outcomes the route declares
  `alike` (below) are not such a pair.

A command route may declare outcomes the service answers the same way `alike`: a list of groups,
each of two or more of the command's outcome names, such as `alike: [[fetched, inline]]`. Each
member keeps its own declared answer and states its own status (answer 16); the group says the
caller is not meant to tell them apart. Every member needs a declared answer, listed under
`outcomes:` or covered by `errors:`, and the members' answers must be equal: `status`, `body`,
`encoding`, `headers` and `redirect`. A group whose answers differ is refused, naming the first
key that differs, and so is one the wire already tells apart: `envelope` bodies (the `outcome`
member names the branch), `{<member>: error}` bodies under distinct error wire names, or
redirects one of which always writes a parameter the other never does (`ESS-HTTP-025`). An
unknown name, a name in two groups or twice in one, a group of one, a name with no declared
answer, and `alike` on a view are refused (`ESS-HTTP-022`). Compiled IR writes `alike` only when
present, each group's names in name order and the groups in the order of their first names.
`OpenAPI` writes one response for the status, `x-ess-outcomes` naming every member, and one
schema for the group where a `oneOf` would hold equal ones. An upsert whose `created` and
`replaced` both answer `200` with the stored resource is the case an adopter asked for
(`story:two-outcomes-answered-alike-may-share-a-status-and-body`); a success and an error may be
alike only on a status both may answer, a redirect.

A command route may also carry `errors:`, one answer (`status`, `body`, `headers`, `redirect`, the
keys of a listed outcome) for every outcome with `error` it does not list under `outcomes:`. It
means exactly what listing that answer under each such outcome would mean, so an `external:`
refusal leaves its derived `502` and a `when:` refusal its derived `422` the same way. A listed
outcome wins over `errors`. Every check of a listed outcome runs once per outcome `errors` covers
and is reported at `routes[i].errors`, saying which outcome it was checked for; `errors` is
further refused with a status outside `300..=599`, with a `response` or `response.<field>` body, on
a view, and on a route whose command has no unlisted error outcome (`ESS-HTTP-022`). An adopter
asked for both forms (`story:an-outcome-may-answer-a-response-field-bare-and-errors-share-a-default-status`).

### Response headers

`headers:` maps a header name (an RFC 9110 token) to one value:

| value | wire |
|---|---|
| `{literal: <text>}` | the text, which must be a valid field value |
| `{source: <path>}` | `response.<field>`, `error.<field>`, `error.wire` or `input.<field>`, a scalar, written as text; an absent optional omits the header |
| `{challenge: {scheme, params}}` | one RFC 9110 §11.6.1 challenge: `scheme` a token, each param a `literal` or a `source`, serialised as a quoted string by the generator |

`WWW-Authenticate` with its `error` attribute (RFC 6750 §3) is the third form with
`error: {source: error.wire}`. The generator writes the quoting, so an error wire name a quoted
string carries cannot break out of the challenge. The wire name is known when the binding
compiles, so `error.wire` is held to what a literal holding the same text is: a header reading it
is refused when the wire name is not a field value (`ESS-HTTP-026`), and a challenge param reading
it when no quoted string carries the wire name (`ESS-HTTP-027`). `Content-Type`,
`Content-Encoding`, `Content-Length` and `Transfer-Encoding` belong to the encoding, `Connection`
to the server, and `Location` to `redirect`. Declaring any of them is refused.

This states a header. It does not add authentication to the model: `security` and `securitySchemes`
stay omitted for the reason `openapi.rs:164` and `openapi.rs:171-191` give (decided for stage 2,
answer 8). A `401` with a challenge is the answer a declared outcome gives; how a caller proves who
it is remains the realization's.

### Redirects

`redirect` belongs to a `3xx` status (`301`, `302`, `303`, `307`, `308`), and each of those requires
it. `location` names a `String` source holding an absolute URI, never `error.wire`, which is a
wire name. `mode: query` appends `parameters`
to its query, keeping a query it already has (RFC 6749 §3.1.2); `mode: fragment` writes them as the
fragment. Parameter values are scalar sources; an absent optional is omitted, which is how RFC 6749
§4.1.2 returns `state` only when the request carried it. A parameter name is one or more RFC 3986
`unreserved` characters (`A-Z a-z 0-9 - . _ ~`), which no encoding of the query or fragment
changes; any other name is refused (`ESS-HTTP-028`). The body is `none`.

In `mode: query` the location's existing query is kept. A location that already carries a
parameter name a redirect declares therefore writes that name twice, and a reader may take the
location's value: it can defeat the parameter-based distinction of two redirects on one status
(`ESS-HTTP-025`) at run time. The generator does not check it; the location's value is the
specification's to constrain.

A redirect to a URI taken from input is an open redirect unless the specification's guards compare
that input with something stored. ESS does not add that check; the outcome that redirects must be
guarded by the specification.

### Standard refusals

The served surface refuses some requests without a declared outcome: an actor no grant admits
(`403`, `http.rs:88-109`), an unfinished realization (`501`, `http.rs:141-148`), a request with no
input where the command declares that answer (`400`, `http.rs:83-86`), and a malformed request or a
path nothing declares (`rust/http.rs:1566-1579`). A protocol states its own answer for some of them
(RFC 6749 §5.2, `invalid_request`, `invalid_client`). Top-level `refusals:` takes `not_granted`,
`unfinished` and `malformed`, each with `status`, a literal body and headers. A refusal not listed
keeps today's answer.

A route may state its own: `routes[i].refusals:` takes the same keys and the same checks, and on
that route only stands in place of the top-level entry of the same key. One endpoint of a protocol
can answer a malformed request with its own error while the binding's other routes keep theirs
(`story:ess-http-admits-text-bodies-route-refusals-empty-bodies-and-array-views`).

### What compiling checks (stage 2)

| code | refused | names |
|---|---|---|
| `ESS-HTTP-020` | an encoding outside the set; `form` or `json` on `get`; `query` on anything but `get` or `delete`; `request:` on a view; `unknown: ignore` with `json`, or an `unknown` value outside `refuse` and `ignore` | `routes[i].request.encoding` or `routes[i].request.unknown` |
| `ESS-HTTP-021` | a `form` or `query` field whose type has no flat spelling (struct, nested list, map, `Json`) | the field and its type |
| `ESS-HTTP-022` | an `outcomes:` key that is not an outcome of the command, or `outcomes:` on a view; `errors:` on a view, or on a route whose command has no outcome with `error` that `outcomes:` leaves unlisted; `alike:` on a view, or a group of one, a name that is not an outcome of the command, one in two groups or twice in one, or one with no declared answer | `routes[i].outcomes.<name>`, `routes[i].errors` or `routes[i].alike` |
| `ESS-HTTP-023` | a status outside `100..=599`, in a class the outcome may not use, or a `3xx` other than `301`, `302`, `303`, `307` and `308` | the outcome, the status, the class |
| `ESS-HTTP-024` | a body form not admitted for the outcome or operation: `response` or `response.<field>` on an outcome without `returns: true` or with `error`, and so under `errors`; `response.<field>` naming no response field or an `Optional` one, or a source other than `response.<field>` as a body; `{…: error}` without `error`; `{…: {literal: …}}` on an outcome; a body other than `none` on `204` or beside a `redirect`; `single`, `array` or `rows` on a command, or an outcome form on a view; `single` or `array` on a paged view; a `<member>` that is empty or equals the wire name of one of the error's fields; an `encoding` other than `json` and `text`, or `text` beside any body but `response.<field>`, on a field that is not a `String`, or under `errors` | the outcome (or `routes[i].body`) and the form, or `….encoding` |
| `ESS-HTTP-025` | two outcomes on one status that the bodies cannot tell apart, an unlisted outcome counted at its derived status with `envelope`, unless both declare a `redirect` and one always writes (its source not `Optional`) a parameter name the other never writes, or both write one parameter name from `error.wire` with distinct error wire names; a `refusals:` override on a route the binding names, on the status of an outcome of its command, with a body that does not tell the two apart (answer 24); an `alike:` group whose answers differ, or that the wire tells apart (`envelope` bodies, error objects under distinct wire names or over errors whose fields differ in wire name, type or order, a header or challenge param reading `error.wire` under distinct wire names, redirects told apart as above); two outcomes in one group are never a pair | both outcomes and the status (for an override, `refusals.<key>` or the route's own `routes[i].refusals.<key>`, the route and the outcomes; for a group, `routes[i].alike`, the group and the first key that differs) |
| `ESS-HTTP-026` | a header name that is not a token, a header the server (`Connection`), the encoding (`Content-Type`, `Content-Encoding`, `Content-Length`, `Transfer-Encoding`) or `redirect` (`Location`) owns, a literal that is not a field value, `error.wire` whose wire name is not a field value, a header declared twice ignoring case, a source that names no field or names a non-scalar, a source the outcome cannot supply (`response.` without `returns: true`, `error.` without `error`), `error.wire` on an outcome whose error declares a field named `wire`, or any `source` in a `refusals:` header | the header and the source |
| `ESS-HTTP-027` | a challenge whose scheme or param name is not a token, a param declared twice, a literal param or a param reading `error.wire` whose text no quoted string carries, a challenge under any header but `WWW-Authenticate`, or a declared `401` (outcome or refusal) without a `WWW-Authenticate` challenge | the header, the param |
| `ESS-HTTP-028` | `redirect` without a `3xx` status, a `3xx` without `redirect`, a `location` that is not a `String` or a newtype of `String`, or is `Optional`, or that reads `error.wire`; a `mode` absent or outside the set, a parameter name that is not one or more RFC 3986 `unreserved` characters, a parameter declared twice, a parameter source that names no field, names a non-scalar or that the outcome cannot supply | the outcome and the key |
| `ESS-HTTP-029` | a `refusals:` key outside the set; a status in the wrong class for it (`4xx` for `malformed` and `not_granted`, `5xx` for `unfinished`); a body other than `{<member>: {literal: …}}` or `none`; the same under a route's own `refusals:` | `refusals.<key>` or `routes[i].refusals.<key>` |
| `ESS-HTTP-030` | `body: single` without `empty`, `empty` without `single`, an `empty` status outside `4xx`, or an `empty` body other than `none` and `{<member>: {literal: …}}` | `routes[i]`, or `routes[i].empty.body` |
| `ESS-HTTP-031` | `body: single` on a view whose filter does not select by its source entity's identity: neither the filter nor a member of a top-level `all` (nested `all` included) is `<identity field> == param.<name>`, compared by value, in either operand order | `routes[i].body`, the view, its source entity and the entity's identity field |

A refinement may not change a refined route's stage-2 keys either; that is `ESS-HTTP-012`. A
missing `status` in a listed outcome or refusal, and an unknown key anywhere, are parse refusals
naming the key, as a missing `method` is in stage 1.

### What generated artifacts change (stage 2)

- **OpenAPI.** `requestBody` takes the declared media type. Each declared status gets its own
  response: the bare schema, the error object, or no content; headers as response `headers`, with a
  redirect's `Location`. The `outcome` discriminator is gone from a bare body, so each response
  carries `x-ess-outcomes` naming the outcomes it answers.
- **Rust and Go servers.** Form and query decoders; status, header, challenge and redirect writers.
  The status a server writes today comes from `http::outcome_status` (`rust/http.rs:1244`,
  `go/http.rs:1546-1557`); a binding's status replaces it in the same place, so the document and the
  servers still read one answer.
- Untouched: the transport-free entry points, emitted conformance runners, the web bridge and the
  interpreter, none of which speaks HTTP.

### Formats and determinism (stage 2)

The format stays `ess-http/1` and `ess-http-ir/1`. A document that uses no stage-2 key compiles to
the same IR bytes under a stage-2 reader: absent keys are omitted, never written as defaults. An
OpenAPI document generated from such a binding keeps its stage-1 bytes. Header and parameter maps
are `BTreeMap`s, so the emitted order is the name order, not the authored order.

### Stage 2: keys, precedence and the units that build them

Stage 2 is built by units k1 to k6 of wave k, in that order; k1 to k3 change one file,
`crates/specify/ess-http/src/lib.rs`, so they run one after another. Each unit adds the keys and
codes below and nothing else, appends its codes to `CODES` in code order and to `compile`'s table
of refusals, and runs `task test-xtask`, because a new code moves pins outside the crate (AGENTS.md).

#### Keys

Every key is optional unless the table says so. The IR spells each key and value exactly as the
authored document does (canonical JSON of the same value), and omits a key whose value is its
default, whether or not the author wrote it, so two documents that mean the same compile to the
same bytes.

| authored key | values | default, and what the IR omits | unit |
|---|---|---|---|
| `routes[].request.encoding` | `json`, `form`, `query` | `json`, omitted | k1 |
| `routes[].request.unknown` | `refuse`, `ignore` | `refuse`, omitted; `request` itself is omitted when both are defaults | k1 |
| `routes[].outcomes.<outcome>` | an object per listed outcome of the command, keyed by its local name | absent: the outcome keeps `outcome_status` and `envelope` | k2 |
| `routes[].outcomes.<outcome>.status` | an integer; **required** in a listed outcome | always written | k2 |
| `routes[].outcomes.<outcome>.body` | `envelope`, `response`, `response.<field>`, `none`, `{<member>: error}` | `envelope`, omitted | k2; `response.<field>` 0.69.0 |
| `routes[].outcomes.<outcome>.encoding` | `json`, `text` (with `response.<field>` naming a `String` field) | `json`, omitted | 0.77.0 |
| `routes[].errors` | one answer with the keys of a listed outcome, for every outcome with `error` the route does not list | absent, omitted | 0.69.0 |
| `routes[].alike` | a list of groups, each two or more outcome names of the command whose declared answers are equal ("Status per outcome") | absent, omitted; each group in name order, the groups in the order of their first names | 0.77.0 |
| `routes[].body` (view route) | `rows`, `single`, `array` (a view that is not paged) | `rows`, omitted | k2; `array` 0.77.0 |
| `routes[].empty.status` (view route) | an integer, `4xx`; **required** with `single` | written with `single` | k2 |
| `routes[].empty.body` (view route) | `none`, `{<member>: {literal: <text>}}` | `none`, omitted | 0.77.0 |
| `routes[].refusals.<key>` | as `refusals.<key>` with its `status`, `body` and `headers`; on that route in place of the top-level entry | absent, omitted when the route states none | 0.77.0 |
| `refusals.<key>` | `malformed`, `not_granted`, `unfinished` | absent: today's answer | k2 |
| `refusals.<key>.status` | an integer; **required** | always written | k2 |
| `refusals.<key>.body` | `{<member>: {literal: <text>}}`, `none`; **required** | always written | k2 |
| `routes[].outcomes.<outcome>.headers.<Name>` | `{literal: <text>}`, `{source: <path>}`, `{challenge: {scheme, params}}` | absent, omitted when empty; ordered by name | k3 |
| `routes[].outcomes.<outcome>.headers.<Name>.challenge.scheme` | a token; **required** | written as authored | k3 |
| `routes[].outcomes.<outcome>.headers.<Name>.challenge.params.<param>` | `{literal: <text>}` or `{source: <path>}` | omitted when empty; ordered by name | k3 |
| `refusals.<key>.headers.<Name>` | `{literal}` or `{challenge}` whose params are all literals | omitted when empty | k3 |
| `routes[].outcomes.<outcome>.redirect.location` | a source text (`input.<field>`, `response.<field>`); **required** | written | k3 |
| `routes[].outcomes.<outcome>.redirect.mode` | `query`, `fragment`; **required** | written | k3 |
| `routes[].outcomes.<outcome>.redirect.parameters.<name>` | a source text | omitted when empty; ordered by name | k3 |

A header source is one of `input.<field>`, `response.<field>`, `error.<field>` or `error.wire`, and
names a top-level field: there is no expression language and no nested path, as in stage 1. Header
names, challenge params and redirect parameters are read as ordered entries, the way `Parameters`
keeps a key written twice (`crates/specify/ess-http/src/lib.rs:153-156`), so that a duplicate is
refused with its code rather than one entry silently replacing the other. The IR keeps a header
name as authored.

k4 to k6 add no binding key:

| unit | adds |
|---|---|
| k4 | the OpenAPI projection of every key above (below); `security` and `securitySchemes` stay omitted (answer 8) |
| k5 | `ess generate synthesize --target rust --http FILE [--refines FILE]`, modelled first in `models/toolchain/domains/generate.yaml`; `--http` with any other target is refused by name, `go` included until k6; built |
| k6 | `--http` admitted for `--target go`, with the Rust server's behaviour, byte for byte where both write the same answer; built |

#### Codes per unit

| unit | adds | widens |
|---|---|---|
| k1 | `ESS-HTTP-020`, `ESS-HTTP-021` | `ESS-HTTP-014` admits a `get` command's input field that is query-encoded; `ESS-HTTP-012` covers `request` |
| k2 | `ESS-HTTP-022`, `-023`, `-024` (except its `redirect` clause), `-025`, `-029`, `-030`, `-031` | `ESS-HTTP-012` covers `outcomes` (status, body), `body`, `empty` and `refusals` |
| k3 | `ESS-HTTP-026`, `-027`, `-028`, and `-024`'s clause for a body beside a `redirect` | `ESS-HTTP-012` covers headers and redirects |
| k4, k5, k6 | no code | — |

k3 refuses a `3xx` outcome without a `redirect` (`ESS-HTTP-028`) and a `401` without a challenge
(`ESS-HTTP-027`), which k2 alone compiled; no release falls between the two units. k3 also widens
`ESS-HTTP-025`: two redirects of one operation on one status, both `body: none`, are told apart by
the parameters they always write, or by distinct error wire names both write under one
parameter from `error.wire` ("Status per outcome"). The
synthesis refusals of k5 and k6 carry no `ESS-HTTP` code, as the OpenAPI
projection's refusal of an open route carries none (`crates/edge/ess-cli/src/http.rs:100-126`):
the document is valid, and the artifact cannot honour it. They are a binding with an open route,
naming the operation, and a route whose template a request for `/openapi.json` or `/docs` would
match: the literal path itself, or a one-segment template such as `/{name}`, by the overlap rule of
`ESS-HTTP-011`. `ess specify http validate` accepts both.

#### Refinement

A refining document restates each stage-2 key of every route of the refined binding exactly: one it
adds, changes or drops on such a route is `ESS-HTTP-012`, naming the key, the refined value and the
new one. A route's `errors` is compared as one value, at `routes[i].errors`, and its `empty` as
status and body. Its top-level `refusals:` equals the refined binding's, both absent or both the
same, and so does each refined route's own `refusals:`, compared at `routes[i].refusals.<key>`. A
route the refining document adds may carry any stage-2 key. The comparison is over the compiled
values, so writing a default explicitly is not a change.

#### Precedence

A served request on a bound route is answered by the first of these that applies, the order the
servers already follow (`crates/generate/ess-synth/src/rust/http.rs:824-868`):

1. A request the HTTP layer cannot read (framing, header lines): today's `400`, never overridden.
2. No route matches: today's `404`. `/openapi.json` and `/docs` keep their answers; no bound route
   can match them (above).
3. The path matches and the method does not: today's `405`.
4. The caller is not granted the operation: `refusals.not_granted`, else today's `403`.
5. The request carries no input at all and the command declares `input_absent:`: that declared
   answer (`http.rs:83-86`), before `malformed`, as today.
6. The input cannot be read per `request` (not the declared media type, not UTF-8, a scalar key
   twice, an unknown key under `refuse`, a missing required field, a value its type refuses):
   `refusals.malformed`, else today's `400`. A path variable's value wins over a body or query key
   for the same field, and that key counts as unknown.
7. The command runs and takes an outcome: the route's `outcomes.<outcome>` if listed, else its
   `errors` for an outcome with `error`, else `outcome_status` and `envelope`. A view answers `rows`, `array`, or `single` with the row or `empty`.
8. The realization is unfinished: `refusals.unfinished`, else today's `501`.

Where steps 4, 6 and 8 read `refusals.<key>`, a route's own `routes[i].refusals.<key>` stands in
its place on that route. `refusals:` applies to the routes the binding names and to no derived
route, and never replaces a declared outcome. A listed outcome replaces status and body as a whole; its headers are added to
the ones the encoding and `redirect` write, which they cannot name (`ESS-HTTP-026`). At compile
time there is no precedence: every refusal is reported at once, and a view route with `body:
single` and no identity selection and no `empty` earns both `ESS-HTTP-030` and `ESS-HTTP-031`.

#### What the servers write (k5, k6)

- **Form and query.** UTF-8, `+` as a space, percent-decoding, keys the fields' wire names. A
  `form` request whose `Content-Type` is not `application/x-www-form-urlencoded` (parameters
  ignored) is malformed. An absent key is an absent optional; an empty value is the empty text for a
  `String` and malformed for any other scalar.
- **Headers.** A literal is checked at compile time. A sourced value or challenge param that is not
  a valid field value at run time (a control character other than horizontal tab, or a leading or
  trailing space for a bare value) is never written: the answer is `unfinished` with `committed`
  `true`, because the effect stands and its answer could not be delivered. A challenge param is
  always a quoted string; `"` and `\` are escaped with `\`, so a value cannot leave it. An absent
  optional omits the header, or the param.
- **Redirects.** `location` must hold an absolute URI without a fragment at run time, else the same
  `unfinished` answer. `query` appends the parameters to any query the URI already has, `fragment`
  writes them as the fragment, both `application/x-www-form-urlencoded` (RFC 6749 Appendix B), in
  name order; an absent optional is omitted.
- **Go and Rust alike (k6).** The Go server answers a bound request with the Rust server's status,
  headers and body bytes wherever the binding writes the answer
  (`crates/generate/ess-synth/tests/http_binding/go.rs`,
  `the_go_and_rust_servers_answer_one_binding_alike`). Both match routes on the path as the
  request line spells it, read a `json` body with one reader (the Rust server's `json::parse`,
  ported: one document, nesting at most 64, the first of a repeated member), and write a body the
  binding declares with members in declaration order and the Rust server's string escaping. Two
  differences stay: `net/http` refuses a
  request line holding a malformed percent-escape with its own `400` before the surface reads it
  (step 1 of "Precedence"), where the Rust server refuses the path variable as `malformed`; and an
  answer the binding does not write (an envelope, a derived refusal, `rows`) keeps each server's
  member order, as it does without a binding.

#### What OpenAPI writes (k4)

`requestBody` content is `application/x-www-form-urlencoded` for `form`, with the flat field
schema; `query` has no `requestBody`, and each unmapped field is an `in: query` parameter, a `List`
with `style: form`, `explode: true` and `required: false`, its absent key the empty list. Each
declared status is one response: the bare response schema, the one field's schema as
`{command}.{outcome}.Response` for `response.<field>`, the error object (`<member>` a `const` of
the wire name beside the error's fields), or no content; an outcome `errors` answers is described
as if listed with that answer; headers as response `headers`, a challenge as a `string` header, and a redirect's
`Location`. Every response of a bound operation carries `x-ess-outcomes`, the outcomes it answers in
the order the command declares them. A view route with `single` answers the row schema at `200` and,
at its `empty` status, no content or the literal object its `empty.body` names; one with `array`
answers an array of the row schema. A `refusals:` override replaces the standard refusal's response
on every bound operation that can answer it, and a route's own override on that route only.

## Operation ids and servers

A binding may name the `operationId` a protocol or a generated client uses for a route, and the base
URLs a component's paths resolve against. Both keys are optional additions inside `ess-http/1` and
`ess-http-ir/1`, the way stage 2 added keys: a binding without them compiles to the same IR bytes
and projects the same OpenAPI bytes, and an older reader refuses an IR that uses them by name
("unknown field") rather than misreading it.

```yaml
servers:
  orders-api:
    - {url: 'https://api.example.test/v2', description: Production}
    - {url: /v2}
routes:
  - command: shop.orders.ListOrders
    operation_id: listOrders
    method: get
    path: /orders
    request: {encoding: query}
```

- **`routes[i].operation_id`**, on a command or a view route. Its grammar is
  `[A-Za-z_][A-Za-z0-9_.-]*`. It is unique across the binding and never the qualified name of
  another operation of the specification, which is that operation's derived `operationId`. OpenAPI
  publishes it as the operation's `operationId`; a route without one, and every derived route, keeps
  its qualified name.
- **`servers: {<component>: [{url, description}]}`**, keyed by component, because a binding spans
  every component and one list would give every document the same URLs, documentation-only ones
  included. A component listed is declared `reached_by: network`. Its list is not empty, keeps the
  order written (the first entry is OpenAPI's default server) and names each URL once. A URL is
  absolute `http://` or `https://` with a host, or `/`-rooted, and holds no whitespace, no `{` or
  `}` (no server variables), no query, no fragment and no userinfo, the rule the realization's
  URL follows. OpenAPI publishes the list as that component's document `servers`, after `info`; no
  other document gets any.
- **Refinement.** An `operation_id` the refined binding fixes cannot change or be left out
  (`ESS-HTTP-012` at `routes[i].operation_id`); one may be added where the refined route has none.
  `servers` is not compared: each server binding states its own deployment.
- **IR.** `Route.operation_id` follows the operation's key, and `servers` follows `refusals`,
  ordered by component; each is omitted when absent. The reader refuses an id outside the grammar,
  one given to two routes or equal to another routed or derived operation's name, components out of
  order, an empty list and a URL outside the rule or given twice.
- Not offered: per-path `servers`, server variables, and an `operationId` derived from `naming.wire`
  in `ess/N`, which would move every document's bytes.

| code | refused | names |
|---|---|---|
| `ESS-HTTP-032` | an `operation_id` outside the grammar; one an earlier route gives; one equal to another operation's qualified name | `routes[i].operation_id`, and both routes for a duplicate |
| `ESS-HTTP-033` | a `servers` component the specification does not declare, one not `reached_by: network`, one written twice, or an empty list; a URL outside the rule, or given twice for one component | `servers.<component>` or `servers.<component>[j].url` |

## Stage 3: an HTTP conformance target

Today the runner is semantic only. `ConformanceTarget::execute_command` takes a
`SemanticCommandRequest` (`crates/verify/ess-conformance/src/target.rs:183-186`) and `query_view` a
`SemanticViewRequest` (`target.rs:225`). `--target` takes `billing`, `oracle-fixture` and
`interpreted` (`main.rs:1089-1097`). The out-of-process adapter is "the named open gap"
(`docs/design/running-a-suite.md:71-80`).

Stage 3 adds a target that implements that trait over HTTP:

```console
ess verify conform run --path MODEL --target http --http server-http.yaml \
  [--refines protocol-http.yaml] --base-url https://catalog.example
```

- **Command.** Encode the input per the route, send it to the declared method and path, and map
  status, body and headers back to the outcome. Stage 2's distinguishability rule
  (`ESS-HTTP-025`) is what makes that mapping total. An answer a route declares `alike` maps to
  the set of its group's outcomes, not to one: a scenario expecting any outcome of the set passes,
  and the report says which outcomes the wire did not tell apart. No HTTP conformance target
  exists yet, so this is the design, and the two generated servers' tests carry the proof that
  both outcomes are answered alike.
- **View.** Send the parameters as path segments and a query string, and read `rows` or `single`.
- **What a bare body cannot show.** Events a branch published, and a response the protocol does
  not return. A scenario that asserts one of these is counted as unobservable on this target, not as
  passed.
- **Refusals by name.** An open route; a binding whose specification digest differs from the
  suite's; an operation the suite exercises that the binding cannot reach.

Formats: the `ess-conformance/*` documents do not change. `--target` gains one value, and
`ess-http-ir/1` is the target's only new input.

## Still refused

- The `http:` key on a command or view in `ess/N`. The binding replaces it.
- Any key in `ess/N`, `ess-realization` or `ess-transport` that carries a route.
- Non-JSON response bodies in `ess-http/1`, except one: a `String` response field answered
  `encoding: text` ("Response body per outcome"). Partial-segment path templates.
- `--http` with an artifact that cannot honour it: `ess ui`, the clap, web and TypeScript targets,
  and any `--kind` other than `openapi`. Each refuses it by name rather than ignoring it.
- Authentication: the binding states a status and a challenge, not how a caller proves who it is.

## Decided for stage 1 (2026-10-11, `decision-blocker:ess-060-wave-scope`)

1. The OpenAPI projection refuses a binding with an open route, naming the operation. Emitting a
   marked server-chosen path stays possible later as an addition.
2. A binding cannot require that it routes every operation in stage 1; an unnamed operation keeps
   its derived route.
3. A literal segment does not take precedence over a variable: `ESS-HTTP-011` refuses the overlap.
4. `refines:` is the digest of the refined file's bytes, as the specification's source pin is.
   Reformatting a refined file is a deliberate re-pin.
5. `ess verify diff` does not compare two bindings in stage 1; that is one unit of stage 2.
6. A view is bound to `get` only in stage 1.

## Decided for stage 2 (2026-10-13, `story:http-stage-2-design-answers`)

Answers 7 to 9 were the open stage-2 questions of this page, each settled on the recommended
option. Answers 14 to 24 settle what the stage-2 sections above left unstated; each takes the
option that is narrower, so a later release can relax it as an addition without a format change,
and none can be tightened later without refusing a document that compiled.

7. **A `single` view with more than one row cannot exist.** Compile refuses `body: single` unless
   the view's filter selects by its source entity's identity (`<identity field> == param.<name>`,
   the filter itself or a member of a top-level `all`), so a `single` route never holds two rows at
   run time: `ESS-HTTP-031`, naming `routes[i].body`, the view, the entity and its identity field.
   The identity is the entity's `identity` field (`crates/specify/ess-domain/src/entity.rs:935`),
   and the filter reads a parameter as `param.<name>` (`crates/specify/ess-domain/src/view.rs:942-946`),
   so the check reads the compiled view and needs nothing new in the model. Rejected: answering the
   first row by key order, which hides a model error behind an answer; answering `500`, a run-time
   failure that a compile can prevent.
8. **No `securitySchemes` in stage 2.** A challenge is a response header only, until the model has
   an authentication construct. `security` and `securitySchemes` stay omitted for the reason the
   OpenAPI projection gives: `may:` is authorization, and a scheme says how a caller proves who it
   is, which the model does not state (`crates/generate/ess-gen/src/openapi.rs:164`,
   `openapi.rs:171-191`). Rejected: deriving a scheme from each declared challenge, which claims an
   authentication mechanism the model does not declare, and which every generated client would
   then implement.
9. **`{<member>: error}` writes the error's own fields under their wire names, unchanged.** A
   protocol field such as `error_description` (RFC 6749 §5.2) is a field of that name on the
   model's error (`crates/specify/ess-domain/src/command.rs:4880-4881`, `fields`), so the model
   owns every name on the wire. A field whose wire name equals `<member>` is refused
   (`ESS-HTTP-024`), because the object would hold that member twice. Rejected: a per-field rename
   map in the binding, a second mapping of names the model already owns, and one that two bindings
   of the same specification could spell differently.
14. `request.unknown: ignore` is admitted with `form` and `query` only, and `request:` is refused on
    a view (`ESS-HTTP-020`). A JSON body stays closed, as ESS's bodies are today; RFC 6749 §3.1's
    "MUST ignore unrecognized request parameters" is about form and query parameters.
15. A `3xx` status is one of `301`, `302`, `303`, `307` and `308`, each with a `redirect`
    (`ESS-HTTP-023`, `ESS-HTTP-028`). `304` answers a conditional request, which the binding cannot
    state, and `300`, `305` and `306` name no answer an outcome gives.
16. A listed outcome requires `status`, and a refusal override requires `status` and `body`, so the
    IR states every answer it changes. Admitting a header on the derived status without restating
    it is an addition for later.
17. `refusals:` applies to the routes the binding names, never to a derived route, a declared
    outcome, an unmatched path (`404`) or method (`405`). `malformed` and `not_granted` take a `4xx`
    status, `unfinished` a `5xx` (`ESS-HTTP-029`). An unmatched path is a property of the whole
    surface, which a binding of some routes does not own. Since 0.77.0 a route the binding names
    may state its own `refusals:`, which stand in place of the top-level ones on that route alone.
18. `{challenge}` is admitted under `WWW-Authenticate` only, and a declared `401` (an outcome or a
    refusal) requires a `WWW-Authenticate` challenge, as RFC 9110 §15.5.2 requires of every `401`
    (`ESS-HTTP-027`). `Proxy-Authenticate` and other `401` answers wait for a protocol that needs
    them.
19. `error.wire` always names the error's wire name. A source `error.wire` on an outcome whose error
    declares a field named `wire` is refused as ambiguous (`ESS-HTTP-026`), rather than one reading
    shadowing the other.
20. `single` is refused on a paged view (`ESS-HTTP-024`), whose answer carries a page, and `empty`
    holds a `4xx` status and nothing else: the empty answer has no body (`ESS-HTTP-030`). Since
    0.77.0 `empty` also takes `body`, `none` or a literal object, and `array` answers a view that
    is not paged with its rows bare; `array` is refused on a paged view for the same reason.
21. A refining document restates every stage-2 key of each refined route, and `refusals:`, exactly
    (`ESS-HTTP-012`). A protocol fixes its encoding, statuses and challenges as it fixes a path; a
    server binding may later be allowed to fill in what a protocol binding leaves out.
22. A sourced header value, challenge param or redirect location that cannot be written at run time
    is answered as `unfinished` with `committed` `true`; it is never written, truncated or
    re-encoded, because a header value from input must not inject a header.
23. `redirect.location` is a `String`, or a newtype of `String`, never `Optional`; `redirect.mode`
    is required. A redirect without a location has nowhere to send the caller, and a default mode
    would choose between query and fragment for the author.
24. A `refusals:` override that shares a status with an outcome of a route the binding names, the
    outcome listed or counted at its derived status with `envelope`, is refused (`ESS-HTTP-025`, at
    `refusals.<key>`) unless its body tells the two apart, which only
    `{<member>: {literal: <text>}}` beside `{<member>: error}` on one member does, the text
    differing from the error's wire name; a client reading the status and body could not otherwise
    tell a refusal from an outcome.

Stage 1's answer 5 places a comparison of two bindings in `ess verify diff` in stage 2. Wave k has
no unit for it, so it stays a stage-2 item without a unit and is not part of 0.61.0.

## Open questions

These could not be settled from the sources. Each needs a decision before the stage it names.
Questions 7 to 9 are answered above.

10. **Stage 3.** How does the HTTP target begin each scenario on an isolated state
    (`target.rs:175-180` leaves "how" to the target) and seed a scenario's initial records, against
    a server it does not control?
11. **Stage 3.** How does the target present itself as a given actor, so grant and `caller.`
    scenarios run? The credentials belong to the realization, which this format does not read.
12. **Stage 3.** Does the conformance run report name its target in a way that admits `http`
    without a format change?
13. **All stages.** The fit review reopens `story:a-resource-has-an-http-shape-of-its-own` and
    `story:a-conformance-suite-runs-against-any-realization`, excluded on 2026-09-15. Which of their
    acceptance items this page now owns, and which stay archived, is not settled here.
