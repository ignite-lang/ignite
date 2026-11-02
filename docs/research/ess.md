# Ignite and ESS compared

Date 2026-11-01. Read: Ignite `design.md` (0.2), `docs/slice.md`, `AGENTS.md`, `std/`,
`examples/`, `crates/`; ESS repository https://github.com/beyond10x/ess (tag 0.58.0, 2026-10-10), the
installed `ess` 0.77.0, GitHub `beyond10x/ess` (latest release 0.79.0, published 2026-11-01T05:57Z,
`gh api repos/beyond10x/ess/releases/latest`), adopter specs `a2a/ess`, `aep/ess`.

Snapshot; `std/protocol/http.ignite` and `Store` (`Store<T>` in `crates/ignite-io/src/store.rs`)
exist since later on 2026-11-01. Citations into Ignite name a section or symbol; `E:` citations
keep line numbers, which refer to the ESS checkout at tag 0.58.0.

Paths: `T:` = the root of this repository (this file is `T:docs/research/ess.md`), `E:` = the root of https://github.com/beyond10x/ess,
`S:` = `docs/research/ess-todo/` for the spec sources (`todo/`, `todo-wire/`); generated schemas, OpenAPI and suites are not kept and are reproduced with `ess compile`.
**[inferred]** marks a conclusion I drew; everything else is read or run.

## 1 Comparison

| dimension | Ignite | ESS |
|---|---|---|
| purpose | One typed language to specify, build, deploy and operate systems; a compiler lowers it to executables and a controller (T:design.md preamble, §1.1). | Describe a system as typed data, compile it to deterministic IR, project artifacts, check implementations against it (E:README.md:3-5). "Decisions as data, with evidence" (E:AGENTS.md:11-12). |
| unit of description | Design: package, type, enum, interface, function, system, resource, component, contract, policy, assembly (§3.2). Implemented: `system` with `resource` and `component`, `protocol` with `serve`/`connect` roles, records, enums, fns (T:docs/slice.md §3 Grammar, §4.2 Protocols, §6.1 System lowering). No entity, command or view. | system, types, entities (identity, lifecycle, relations), commands with outcomes, events, errors, views, actors, components, bindings, topology (E:website/docs/concepts/ess.md:20-34). Sidecars: `ess-http/1` routes, `ess-protospec/1` protocol machines, realization, runtime, deployment (E:README.md:113-137). |
| type system | Records, payload enums, tuples, generics without bounds, pure and effectful fn types, structural aliases ("an alias is the type it names") (T:docs/slice.md §4.6 Type aliases, §10.1 Frontend contract for the backends, §11.1 Generics, §11.2 Effectful fn types). Builtins `Text Bytes Byte Int Bool Unit List Option Result` plus IO handles and `Store<T>` (T:docs/slice.md §7 Substrate `ignite-io`, "Builtin names"; §11.3 `Store<T>`). Design adds quantities, Money, Evidence, Identity, Capability (§3.3); quantity literals are not in the slice (T:docs/slice.md §2 Lexical structure). Exhaustive `match` (E046). | Nominal newtypes that refuse crossing without a declared `conversions:` entry (E:examples/billing/components.yaml:4-14), structs with invariants, enums, tagged unions, `Optional`, `List`, `Map`, `Decimal`, `Timestamp`, `Duration`, `Uuid`, `Bytes` (E:examples/billing/domains/invoice.yaml:9-63, 105-132). No functions, no user generics. |
| behaviour | Executable code: functions, loops, pattern matching; protocols are code over a byte stream (T:std/protocol/telnet.ignite, `protocol Telnet` roles `serve` and `connect`). | Declarative: each command lists its outcomes with `when:` guards, `creates/updates/moves/deletes/preserves`, `sets:`, `emits:`, `payload:` (E:examples/billing/domains/invoice.yaml:191-381). What the spec cannot determine becomes an "obligation" for a human (E:website/docs/concepts/ess.md:192-210). "Anything executable is Rust" applies to ESS's own tooling (E:AGENTS.md:23). |
| state and identity | Design: logical identity derived from system and declaration identity, distinct from provider handle (§3.4); durable controller state and journal (§2, §7.1). Implemented: in-memory `Store<T>` keyed by `Int` from `next_id()` (T:docs/slice.md §11.3 `Store<T>`). | Entities with named, typed identity, lifecycle states where illegal transitions are illegal by absence (E:examples/billing/domains/invoice.yaml:97-161); per-subject linearizability promise checked by `check-history` (E:website/docs/concepts/ess.md:212-235). Compiler-minted handles, total lookups (E:AGENTS.md:51-52). |
| effects and authority | Capability effects on code: `requires [stream.read, ...]`, inferred sets, roots through parameters, E016/E017 (T:docs/slice.md §5 Effects, §11.2 Effectful fn types). Design: capabilities passed explicitly, secrets redacted (§3.3, §9.3); policies hold bounded action authority (§4.2, §7.2). | Caller authority: actors with `may:` grants on commands and views (E:examples/billing/domains/invoice.yaml:163-172; E:website/docs/guides/verify/runners.md:150-157). Executor edge: only named commands call external tools (E:website/docs/concepts/overview.md:68-80). No code-level effect system **[inferred: none found in the spec format]**. |
| generated artefacts | Rust crate and `cargo build`, or LLVM IR and clang, into executables `terminald`, `tn`, `todod` (T:docs/slice.md §6.1 System lowering, §6.2 Generated projects; T:crates/ignite/src/main.rs `enum Commands`). Design: canonical JSON executable specification `ignite-ir/0.1` (§6.2). | Markdown, HTML site, JSON Schema, OpenAPI 3.1, AsyncAPI 3.0 (E:website/docs/concepts/ess.md:144-157); synthesized Rust, Go, browser, Clap code with generated/obligation/refused dispositions (same file :280-298); BuildKit, Helm, deployment IR (E:README.md:120-137). Every artifact carries model and contract digests (E:website/docs/concepts/ess.md:154-157). |
| verification | Implemented: syntax/check/type fixtures with `.expected`, generator goldens, real-socket end-to-end tests (T:AGENTS.md "Conventions"; T:crates/ignite/tests/todo.rs module doc comment). Planned slice 8: inline `test` blocks and `ignite test` (task brief; not yet in T:docs/slice.md). Design layers: language, semantics, control, runtime, adapters, native, product, bootstrap (§13.1); terminal acceptance (§11.10). | Suite synthesized from the spec: one scenario per outcome, move, forbidden move, invariant, binding claim; untestable constructs are refused, not omitted (E:website/docs/concepts/ess.md:170-181). Runners in Go, TypeScript, Rust; skipped is not failed, a run with skips is `inconclusive` (E:website/docs/guides/verify/runners.md:45-121). Mutation audit, random sequences, linearizability, semantic diff and impact (E:website/docs/concepts/ess.md:183-235). |
| deployment and operations | Design only: assembly chooses implementations and placement (§5); plan results feasible/infeasible/unresolved (§5); durable effect journal, leases, epochs, unknown outcomes (§7.1-7.2); `ignite plan/apply/inspect/explain` (§10.1); SLI/SLO/SLA contracts with `insufficient_evidence` (§4.1). Implemented: none; `ignite` has `check`, `emit`, `build` (T:crates/ignite/src/main.rs `enum Commands`). | Implemented: realization, runtime, build, release, stack, deployment documents; `deployment reconcile` applies changed Helm release units (E:README.md:113-137). Explicitly "not a running control plane: each is one finite invocation" (E:website/docs/concepts/overview.md:79-80). Topology states replicas, statefulness, required resources (E:examples/billing/topology.yaml:5-20). No SLO construct **[inferred: no match for SLO terms in E:website/docs or E:docs/design]**. |
| tooling | `ignite check/emit/build`, Taskfile, two targets (T:AGENTS.md "Commands"). | `ess` with five areas `specify generate verify infra ui` (`ess --help`), agent plugin `ess@b10x` (E:README.md:14-19), docs site, Go/TS/Rust runners, UI documents, Kubernetes import (E:README.md:92-106). |
| maturity | Design status "proposed", "does not claim that a compiler ... is implemented" (T:design.md 0.2, status line and preamble). Slices 1-7; ~32k lines of Rust in `crates/` (`find ... | wc -l`). Slice 7 half-landed on 2026-11-01: frontend has `Store` and generics (T:crates/ignite-types/src/lib.rs `enum Ty`, variants `Store`, `GenericRecord`, `GenericEnum`), but `crates/ignite-io` has no `Store` and `std/protocol/http.ignite` does not exist (`ls T:std/protocol`), so `todod` cannot build yet **[inferred]**. | Release 0.79.0, source format `ess/32` (`ess specify formats`); ~931k lines of Rust in `crates/` (32 crates); 24 sibling repos carry an `ess/` spec (checkouts of the beyond10x organisation, `ls -d */ess`). Adopters record gaps inline as "UNMAPPED (ESS gap)" (https://github.com/beyond10x/a2a/blob/main/ess/http.yaml lines 8-34). |

## 2 Overlap

| concept | Ignite | ESS | note |
|---|---|---|---|
| todo record and store | `type Todo`, `resource todos: Store<Todo>` (T:examples/todo/todo.ignite `type Todo`, `system TodoService`) | entity `Todo` with identity and lifecycle (S:todo/system.yaml) | ESS requires a `lifecycle` on every entity (validator: `missing field lifecycle`); Ignite has no states. |
| HTTP API | routing as code in `handle` (T:examples/todo/todo.ignite `fn handle`) | commands and a view plus `ess-http/1` routes (S:todo-wire/http.yaml) | ESS can state the same wire: 200/201/204/404, a bare array view, 400 for malformed (run below). |
| system and component | `system { resource; component: Service/Client }`, one executable per component (T:docs/slice.md §6.1 System lowering) | logical `components` owning domains, `reached_by` (E:website/docs/concepts/ess.md:46-70), physical mapping in realization/runtime (E:README.md:113-128) | Ignite fuses logical and physical; ESS keeps them in separate documents. Ignite `assembly` (§3.2, §5) corresponds to ESS realization + runtime **[inferred]**. |
| application CLI | CLI as a component with command tree, generated parsing (§10.2) | `reached_by: command_line` + `cli` block, Clap emitter (E:website/docs/concepts/ess.md:67-70, 102) | Same idea, both generate the parser and leave handlers. |
| protocols | `protocol Telnet` roles as code (T:std/protocol/telnet.ignite, roles `serve` and `connect`) | `ess-protospec/1`: finite machines, channels, timers, flush-before-close property, bounded exploration, trace replay (E:docs/design/protocol-specification.md:7-33) | ESS's own example is a terminal response that must flush before close (E:examples/protocols/terminal-response.yaml); Ignite's substrate sends only when a writer waits, ends or flushes (T:docs/slice.md §7 Substrate `ignite-io`). |
| canonical IR and determinism | canonical JSON, versioned schema, provenance on every node (§6.1-6.3) | ordered collections, compiler-minted handles, canonical bytes, format versions (E:AGENTS.md:49-57) | Same principles. Ignite calls its IR an "executable specification" (§6), which collides with the ESS name. |
| executor boundary | `build` does not deploy; `apply` submits a saved plan (§10.1) | projections never apply; named executor commands only (E:AGENTS.md:20; E:website/docs/concepts/overview.md:68-80) | Same split. Ignite plans a long-running controller (§2, §7); ESS refuses to be one. |
| execution of declarations | compiler emits executables | ESS lowers a component to Entity Runtime definitions (E:website/docs/reference/entity-runtime-lowering.md:9-30), and the `interpreted` target executes the model (E:website/docs/guides/verify/runners.md:24-31) | ESS already has execution paths of its own. |

## 3 Relationship options

| option | cost | verdict |
|---|---|---|
| (a) Ignite emits ESS from a `system` | A Ignite service has no commands, outcomes, events or views to emit. Routing is a `match` in a function (T:examples/todo/todo.ignite `fn handle`), and ESS forbids guessing on import (E:AGENTS.md:20). Emitting records as ESS `types:` is cheap. A full spec needs a declarative service surface in Ignite first. | Later, and only for types. |
| (b) Ignite imports ESS as declarations | Ignite lacks nominal newtypes (aliases are structural, T:docs/slice.md §4.6 Type aliases), `Decimal`, `Timestamp`, `Uuid` and `Map` (T:docs/slice.md §7, "Builtin names"), and non-std modules (E031, T:docs/slice.md §4.1 Imports and the standard library). The mapping: entity → record + `Store<T>`; command → fn returning an outcome enum; view → fn returning `List<Row>`; `ess-http/1` → the HTTP role's route table **[inferred]**. ESS synthesis already sorts every capability into generated, obligation or refused (E:website/docs/concepts/ess.md:192-210), so a "Ignite" synthesis target in `ess-synth` would reuse that plan **[inferred]**. | Promising second step, once Ignite has newtypes and multi-module programs. |
| (c) ESS conformance against Ignite executables | Low. I wrote the todo spec (141 lines) plus a wire binding (45 lines); it validates, synthesizes 11 scenarios and passes 11/11 on the `interpreted` target (section 5). Missing piece: a target adapter. Runners call `ExecuteCommand`/`QueryView`/`ObserveEvents`, not HTTP (E:website/docs/guides/verify/runners.md:57-73), so either a Go/TS target that speaks HTTP to `todod` or a Rust runner in `T:crates/ignite/tests` feeding `ess verify conform report` (`ess verify conform report --help`). Ignite `todod` publishes no events; ESS refuses a changing outcome without one (ESS-COMMAND-007, run below). Without an event log, scenarios that observe events come back `unsupported` and the run is `inconclusive` (E:website/docs/guides/verify/runners.md:107-110). | **Do first.** It gives Ignite an independent oracle for slice 7 and the "Product" layer (§13.1). |
| (d) one replaces the other | Ignite replacing ESS: no entity/command/outcome vocabulary, no conformance synthesis, design not implemented (T:design.md 0.2 preamble), against 24 adopter specs. ESS replacing Ignite: ESS holds no executable behaviour and makes no attempt to; telnet negotiation, effects and LLVM are out of its scope (E:docs/design/protocol-specification.md:19). | No, either way. |
| (e) stay separate | Duplicated concepts drift: system/component, CLI declarations, protocol models, canonical IR, the "executable specification" name. | The default for each core, with (c) as the bridge. |

**Recommendation:** (c) now, then (b) as an ESS synthesis target emitting Ignite once Ignite has
newtypes and multiple modules. ESS stays the contract and oracle; Ignite implements and operates.
Rename Ignite's "executable specification" (§6) to avoid the clash.

## 4 Things to take

### Ignite from ESS

| take | ESS source | Ignite place |
|---|---|---|
| Nominal newtypes; crossings only through a declared conversion | E:examples/billing/components.yaml:4-14 | T:docs/slice.md §4.6; `crates/ignite-types`; design §3.3 |
| Tests derived from declarations, with refused cases listed beside passing ones | E:website/docs/concepts/ess.md:170-181 | slice 8 `ignite test` runner; design §13.1 |
| Three verdicts: skipped is not failed, any skip makes the run inconclusive | E:website/docs/guides/verify/runners.md:107-110 | slice 8 runner output |
| Suite audit by mutants | `ess verify conform mutate` (`ess verify conform --help`) | slice 8 follow-up; design §13.1 |
| Linearizability check for concurrent stores | E:website/docs/concepts/ess.md:212-235 | `Store<T>` test, T:docs/slice.md §11.6 Std and example (16×100 POSTs) |
| Lifecycle machines where an illegal transition is an absent one | E:examples/billing/domains/invoice.yaml:142-161 | controller machines, design §11.8, §6.2 `machines` |
| Model and contract digests in every artifact, plus a `--check` drift mode | header of S:gen-wire/openapi/todo-service.yaml:1-4; `ess generate --help` | `crates/ignite-rustgen` generated project (T:docs/slice.md §6.2); design §6.1 |
| Semantic diff and impact between revisions | E:website/docs/concepts/ess.md:183-190 | plan change classification, design §5, §7.3 |
| Versioned source format (`format: ess/N`) and a toolchain pin (`requires: ess X.Y`) | E:website/docs/reference/formats.md:123-131 | Ignite source has no language version; design §9.2 |
| Protocol machines checked by bounded exploration and trace replay | E:docs/design/protocol-specification.md:21-33 | telnet acceptance, design §11.10; T:docs/slice.md §4.2 "later slice" |

### ESS from Ignite

| take | Ignite source | ESS place |
|---|---|---|
| SLI/SLO/SLA with populations, windows and `insufficient_evidence` | design §4.1, §11.7 | `topology` in `ess-domain` has only replicas, statefulness and resources (E:examples/billing/topology.yaml:5-20) |
| Durable effect intents with an `outcome unknown` result | design §7.1 | `ess generate deployment reconcile` (E:website/docs/concepts/overview.md:76) |
| Policies with bounded action authority and an arbiter | design §4.2, §7.2 | delivery chain, `ess-deployment` |
| Effect sets on obligation code | T:docs/slice.md §5, §11.2 | synthesized ports in `ess-synth`; realization `RuntimeRequirement` declares prerequisites but does not check them (E:website/docs/concepts/ess.md:55) |
| Quantities with units (`Bytes`, `Rate<Event>`) | design §3.3 | `ess-primitives` types |
| Byte-level protocol implementations as `ProtocolTarget`s, where protospec explicitly does not observe bytes | T:std/protocol/telnet.ignite; E:examples/protocols/README.md:20-25 | `ess-conformance::protocol` |

## 5 Worked example: todo

Files: S:todo/system.yaml (semantic spec, 141 lines), S:todo-wire/system.yaml (with `response:`,
160 lines), S:todo-wire/http.yaml (wire binding, 45 lines), projection at
S:gen-wire/openapi/todo-service.yaml. Run with `ess` 0.77.0:

- `ess specify validate` → `todo v1 — 1 file(s), valid`; `ess specify http validate` → `5 route(s), 0 open, valid`.
- `ess verify conform synthesize --target ir` → `11 scenario(s) (0 authored), 0 refusal(s)`.
- `ess verify conform run --target interpreted --report-format 2` → `11 scenarios: 11 passed`.
- The OpenAPI projection has `/todos` GET 200 and POST 201, and `/todos/{id}` GET 200/404, PATCH 200/404, DELETE 204/404, each with 400, plus 501 for an unfinished realization.

Ignite (T:examples/todo/todo.ignite `type Todo`, `fn patch`, `system TodoService`):

```ignite
type Todo { id: Int, title: Text, done: Bool = false }

fn patch(todos: Store<Todo>, id: Int, body: Text) -> Response = match (todos.get(id), parse(body)) {
  (None, _) => status_response(404)
  (Some(todo), Ok(value)) => {
    let changed = Todo { id, title = or(value.text("title"), todo.title),
                         done = or(value.bool("done"), todo.done) }
    todos.put(id, changed)
    json(200, to_json(changed))
  }
  _ => status_response(400)
}

system TodoService {
  resource ingress: TcpListener { address = "127.0.0.1", port = 8080 }
  resource todos: Store<Todo>
  component server: Service { listener = ingress, protocol = Http,
                              application = HttpApp { state = todos, handle }, executable = "todod" }
}
```

ESS (excerpt of S:todo-wire/system.yaml and http.yaml):

```yaml
entities:
  - name: todo.list.Todo
    identity: {name: id, type: todo.list.TodoId}          # newtype of Integer
    fields: [{name: title, type: todo.list.Title}, {name: done, type: Boolean}]
    lifecycle: {initial: Kept, states: [Kept], terminal: [Kept]}   # required
commands:
  - name: todo.list.UpdateTodo
    input:
      - {name: id, type: todo.list.TodoId}
      - {name: title, type: Optional<todo.list.Title>, presence: omitted_when_absent}
      - {name: done, type: Optional<Boolean>, presence: omitted_when_absent}
    outcomes:
      - {name: not-found, unknown_instance: true, error: todo.list.TodoNotFound}
      - name: updated-both
        when: {all: ["defined(title)", "defined(done)"]}
        updates: todo.list.Todo
        instance: id
        returns: true
        sets: {title: input.title, done: input.done}
        emits: [todo.list.TodoChanged]
        payload: {todo.list.TodoChanged: {id: input.id}}
      # updated-title, updated-done, unchanged: same shape, one per presence combination
# http.yaml
  - command: todo.list.UpdateTodo
    method: patch
    path: /todos/{id}
    parameters: {id: input.id}
    alike: [[updated-both, updated-title, updated-done, unchanged]]
    outcomes:
      updated-both: {status: 200, body: response}   # ... and the other three
      not-found: {status: 404, body: none}
```

Mismatches found while writing it:

| Ignite | ESS | evidence |
|---|---|---|
| `or(value.bool("done"), false)` in one line | default-or-keep needs one outcome per presence combination (four for PATCH), because an `Optional` input narrows only in a branch guarded by `defined(...)` | `type_mismatch` ESS-COMMAND-002 from `ess specify validate`; E:website/docs/guides/specify/narrowed-inputs.md:10-15. Exponential in optional fields **[inferred]** |
| outcomes with the same answer | `alike:` group required, otherwise ESS-HTTP-025 | `ess specify http validate` output |
| no events | every changing outcome must emit one (ESS-COMMAND-007) | `ess specify validate` output |
| no lifecycle | `lifecycle` mandatory; a `read_your_writes` view must project `state` or `found`/`unchanged` are refused (ESS-SYNTH-001) | validator and `synthesize` output |
| unknown JSON members ignored (`value.text("title")`) | a `json` request body is closed; `unknown: ignore` only for `form`/`query` (ESS-HTTP-020) | `ess specify http validate` output |
| ids 1, 2, 3 in order (T:docs/slice.md §11.3 `Store<T>`) | `instance: id`, implementation-assigned; the suite checks only presence and type | E:website/docs/start/first-specification.md:131-132 |
| 405, 413, 431, idle timeout, keep-alive (T:docs/slice.md §11.6 Std and example) | 405 for a path that matches with a wrong method is the generated server's; no construct found for size limits or keep-alive **[inferred]** | `gh api …/docs/design/http-presentation-binding.md` lines 443-457, 592 (fetched to S:gh/) |
