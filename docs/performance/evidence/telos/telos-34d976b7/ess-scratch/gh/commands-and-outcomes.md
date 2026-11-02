---
title: Commands and outcomes
sidebar_position: 4
description: Wrong-state answers, unknown instances, empty requests, selection by existence or filter, deletion, creation states, outcomes that change nothing, ambient preconditions, and the value sources of events and errors.
---

# Commands and outcomes

## A command says what it answers when invoked in the wrong state

One key and one error name — everything else is derived:

```yaml
- name: issued
  moves: billing.invoice.Invoice.issue      # `issue` runs from [Draft]
  instance: invoice_id
  emits:
    - billing.invoice.InvoiceIssued

- name: wrong-state
  wrong_state: true
  error: billing.invoice.InvoiceStateConflict
```

`wrong_state:` names no state: `issue` already declares it runs from `Draft`, so the refused states
are derived. The `error:` is required — without it a generated scenario could only assert that
*nothing happened*, which also passes against an implementation refusing for the wrong reason.

An **unknown instance** — the input selects a `moves:` or `updates:` branch and its `instance:`
names no record — is answered by the command's **not-found outcome** when it declares one: an
`external:` refusal whose `error:` carries a field of the identity's type, such as
`not-found` reporting `DoorNotFound { door_id }`. The suite checks it
under that outcome's id with an identity no other scenario sends, in place of injecting the
external cause. Only a command declaring no not-found outcome falls back to its `wrong_state`
branch, checked under the branch's own id (`…IssueInvoice/outcome/wrong-state`). Input guards are
decided first, so `PayInvoice` with a non-positive amount still answers `rejected` whatever invoice
it names. Either scenario requires the outcome, its error by name and no error field, and that no
declared event is published. A command that acts on an input-named instance and declares neither
has no declared answer, and one declaring two not-found candidates has an ambiguous one.
`ess verify conform synthesize` prints a `note:` for each, not a refusal.

An error field that describes the current state, such as `InvoiceStateConflict.state`, has no value
for an instance that does not exist. From `ess/23` it can say so: `{subject: state}` fills it with
the state the record holds, the state before the move, and the literal of the move's target state
fills a field naming the state that was asked for (`requested: Published`). Where the `wrong_state` error declares fields, the generated
Rust and Go behaviour seams add a second variant for this answer that carries none of them:
`IssueInvoiceOutcome::WrongStateUnknownInstance` in Rust, `IssueInvoiceOutcomeWrongStateUnknownInstance`
in Go. The served surface answers it with the branch's `409`, the outcome and the error, and no
`payload`. The `WrongState` variant still requires every field, so a realization cannot leave out
the state of an instance it holds.

## An unknown instance can have its own outcome

From `format: ess/15`, an identity the system never held can answer differently from one in a state
no move starts from:

```yaml
- {name: no-such-call, unknown_instance: true, error: example.call.CallNotFound}
- {name: already-ended, wrong_state: true, refuses: false}
```

`unknown_instance:` sits beside `wrong_state:`, at most once per command, on a command with a
`moves:`, `updates:` or `deletes:` branch that reads `instance:` from input. It names an `error:`,
or declares `refuses: false` for an accepted no-op, and takes no other condition and no effect. It
is the first answer for an identity no record carries — before a declared not-found refusal and
before `wrong_state`. The suite checks it under its own outcome id, sending an identity no other
scenario sends and arranging nothing; the `wrong_state` branch keeps its scenarios in the states it
answers. The generated seams carry its variant like any other outcome's, and the served surface
answers a refusing one with `404`.

## A request with no input can have its own outcome

From `format: ess/16`, a command whose implementation answers a request that arrives with no body at
all, before any field is validated, says so without making its fields `Optional`:

```yaml
- {name: body-missing, input_absent: true, error: demo.notes.BodyMissing}
```

`input_absent:` sits beside `wrong_state:` and `unknown_instance:`, at most once per command, on a
command that takes input. It names an `error:` and takes no other condition, no effect and no
`refuses:`. The command's fields keep their types, so every other branch keeps its contract. The
suite checks it under its own outcome id with an `execute_command_without_input` step, which sends
no input document at all and is not `execute_command` with `input: {}`; it requires the outcome,
the error and that no declared event is published. A target that cannot send a request without a
body reports that scenario `unsupported`. The `OpenAPI` projection marks the request body not
required and documents the answer as `400`. Every generated code target (Rust, Go, Web, Clap) and
Entity Runtime lowering refuse the branch by name.

From `format: ess/16`, a guard that cannot hold because it needs a required input to be absent —
`not defined(text)` or `missing(text)` where `text` is not `Optional`, or a conjunction with one —
is refused as a type mismatch, with a hint naming `input_absent:`. A guard that can still hold
another way, such as `any: [text == "x", missing(text)]`, is admitted.

## An outcome can be selected by whether the record exists

From `format: ess/16`, a command that addresses a record by an identity the caller supplies can say
what it does when that record exists and when it does not. Two forms, both built on
`unknown_instance:`.

**Create or update** (`PUT` semantics). Mark the creating branch `unknown_instance: true`, beside the
branch that updates or moves the record the same input names:

```yaml
- name: updated
  updates: demo.items.Item
  instance: item_id
  emits: [demo.items.ItemStored]
  payload:
    demo.items.ItemStored: {item_id: input.item_id, label: input.label}
  sets: {label: input.label}
- name: created
  unknown_instance: true
  creates: demo.items.Item
  instance: item_id
  emits: [demo.items.ItemStored]
  payload:
    demo.items.ItemStored: {item_id: input.item_id, label: input.label}
  sets: {label: input.label}
```

The creation is taken when no row carries the identity, and the update when one does. The creation
must publish its identity from the same input field the updating branch reads as `instance:`, on the
same entity; an identity `{generated: true}` is never one a caller names again, and is refused. The
creating branch names no `error:` and no `refuses:`, and it is the command's one `unknown_instance:`
branch. The pair is exhaustive, so the command is not refused as undetermined by its input.

**Create or refuse.** Declare the refusal for an identity a record already carries beside the
creation:

```yaml
- name: booked
  creates: demo.items.Slot
  instance: slot_id
  emits: [demo.items.SlotBooked]
  payload:
    demo.items.SlotBooked: {slot_id: input.slot_id, label: input.label}
- {name: already-booked, existing_instance: true, error: demo.items.SlotTaken}
```

`existing_instance:` is at most once per command, names an `error:` and takes no other condition, no
effect and no `refuses:`. It needs a `creates:` whose identity comes from an input field — `input.f`,
or an optional id written `{input: f, else: {generated: true}}` — and it
cannot sit beside a branch that acts on the existing record (`moves:`, `updates:`, `deletes:`,
`wrong_state:`) — that command is create-or-update instead.

In both forms an input-guarded refusal (`when:` with an `error:`) is answered first: a request such
a refusal claims is refused whether or not the record exists, and only the rest is answered by
existence.

The suite checks both with two calls that share one identity. For create-or-update, the creating
branch is sent an identity no other scenario sends; the updating branch's scenario creates the record
through the same command, sends the identity again with other values, and requires the update, the
new values in the views and exactly one row for the identity. For create-or-refuse, the scenario
creates the record through each creating branch, snapshots it, sends the identity again with other
values, and requires the declared error, no event and the row unchanged. An input-guarded refusal
beside either form is checked twice: for an identity nothing stored, and for one a record carries. No new suite step is used. The served surface answers
`existing_instance:` with `409`. The generated Rust behaviour looks the identity up in its storage
port before it takes a branch. The Go, Web and Clap targets refuse both forms by name, and Entity
Runtime lowering refuses them with `ExistenceSelectionUnsupported`. Below
`ess/16` both are refused with `unsupported_format_version`.
A system precondition cannot invoke a command of either form, because which branch it takes depends
on a record it cannot observe before it runs.

## An outcome can delete its subject

From `format: ess/15`, a record the implementation removes at the end of its lifecycle is declared
as removed rather than moved to a terminal state no row ever holds:

```yaml
- name: ended
  deletes: example.call.Call
  instance: call_id
  when_subject_state: Connected     # optional, as on any subject outcome
  emits: [example.call.CallEnded]
```

`deletes:` is an effect beside `creates:`, `moves:`, `updates:` and `preserves:`, on an existing
subject named by an input. It may emit events, may not `sets:`, and names no error. Its scenario
requires that no immediate (`read_your_writes`) view of the entity still holds a row with that
identity, then sends the command for it again and requires the unknown-instance answer. A suite
holding the absence check is `ess-conformance/22` (coverage `/23`); Go and TypeScript runners
refuse those majors by version.

## An update can rename its subject

From `format: ess/23`, an `updates:` whose `sets:` writes the entity's identity re-keys the record:
the row its `instance:` names comes to rest under the identity written, every field `sets:` does
not name carried over, and the old identity names nothing afterwards.

```yaml
- name: taken
  when_related: {entity: demo.vault.Secret, where: name == input.new_name, exists: true}
  error: demo.vault.NameTaken
- name: renamed
  updates: demo.vault.Secret
  instance: name
  sets: {name: input.new_name}
- {name: no-such-secret, unknown_instance: true, error: demo.vault.NoSuchSecret}
```

The command must answer a new identity another record carries: a refusal guarded by exactly
`when_related: {entity: <the entity>, where: <identity> == input.<field>, exists: true}` over the
input the identity is written from, or validate refuses the outcome as `missing_declaration`. The
guard reads the rows as they were before the branch, so a rename to the record's own identity is the
collision. The identity write is refused by name beside `compensates:`, in a create-or-update pair,
on an entity an `owns` or `references` relation carries and on a struct identity, and below `ess/23`
it is refused naming `ess/23`. Its scenario requires the row under the new identity with its other
fields, no row under the old one, and the same request again answered as an unknown instance; the
collision's scenario requires every immediate view unchanged, for another record's identity and for the record's own.
The generated Rust behaviour inserts the row under the new identity, then removes the old one,
through its storage port; Go, Web and Clap refuse the write by name, and Entity Runtime lowering
refuses it with `IdentityChangeUnsupported`.

## A creation can land in a declared state

From `format: ess/15`, a record that first appears already past `initial` — a call announced as
ringing — says so:

```yaml
- name: offered
  creates: example.call.Call
  instance: call_id
  into: Ringing
  emits: [example.call.CallOffered]
```

`into:` names a declared state of the created entity and is admitted only beside `creates:`; a
terminal state is allowed. Omitted, creation lands in `initial` as before. The scenario asserts
`Ringing` on the created row wherever a view projects `state`, and an arrangement that needs a
`Ringing` call reaches it through this creation rather than through the moves from `initial`. The
state must still be reachable from `initial` by some transition: the entity's own reachability
check does not yet count creation states.

## An accepted request can change nothing

From `format: ess/15`, a request answered with success and no effect is declared as such:

```yaml
- name: acknowledged
  when: flag == true
  accepts: nothing
```

`accepts: nothing` is admitted under `when:` or as the default, with no subject, no event, no error,
no assignment and no replay. Its scenario requires no error and no direct event, and that every
immediate view without parameters holds exactly the rows it held before the command. A command with
a subject says `preserves:` instead. The whole-view check is `ess-conformance/22` (coverage `/23`).

## A refusal can declare the change it makes before answering

A refused command changes nothing: a branch naming `error:` beside `moves:`, `updates:` or
`emits:` is refused as `refusal_mutated_state`. From `format: ess/22` one exception is declared
explicitly. An `external:` refusal marked `compensates: true` changes the record its `instance:`
names, then answers its error:

```yaml
- name: failed
  external: the upstream refuses the join
  error: shop.order.Refused
  compensates: true
  moves: shop.order.Order.reset
  instance: order_id
  sets: {failure: input.reason}
```

The change is `moves:` or `updates:` with `instance:`, and `sets:` on the same record. A
compensating refusal creates, deletes and publishes nothing, and changes no other record
(`affects:`). Only an `external:` refusal may carry the marker: a refusal the input, a held state
or a related row decides is answered before the record is read, and still changes nothing. The
record must rest in a state the move starts from; otherwise the command's `wrong_state:` answer
applies, as for any move.

The scenario forces the branch, requires the error and no event, then reads the record back
through an immediate view of the entity and requires the target state and the written fields. A
branch whose record no view shows is refused by synthesis rather than checked without its
read-back. Below `ess/22` the marker is refused naming `ess/22`.

## A system can run inside ambient preconditions

From `format: ess/15`, the source carrying the system header can declare commands every command runs
inside — an open session whose user row exists:

```yaml
preconditions:
  - command: example.call.OpenSession
    as: example.call.Agent
    input: {user_id: 00000000-0000-4000-8000-000000000152}
```

Each names a declared command, optionally the actor it runs as (who must be granted it), and a
literal for every required input. Its literal input must select exactly one branch that reports no
error — a precondition whose `when:` guards pick a refusal, several branches, none, or cannot be
decided from the literals is refused. An input the command declares under `fixture_inputs:` may be left
out or written `{fixture: name}`, and is resolved like any fixture input. Synthesis prepends the
list, in order, to every scenario and requires each to take its success branch; the generated Go
and TypeScript explorers run it before every sequence, so the model starts from the state it leaves.
A precondition the target, or the explorer's model, does not answer with that branch fails the
scenario, or the exploration, as setup. A scenario that itself sends the precondition's command for
the identity it creates runs without the prelude, so it does not create that record twice. The explorers do
not resolve fixture inputs and refuse a precondition that needs one.

## An event's values need a declared source

`emits:` declares which facts a branch announces; `payload:` declares what fills their fields:

```yaml
- name: accepted
  when: amount.amount > 0
  creates: billing.invoice.Invoice
  instance: invoice_id
  emits:
    - billing.invoice.InvoiceCreated
  payload:
    billing.invoice.InvoiceCreated:
      account_id: input.account_id
      customer_email: input.customer_email
      amount: input.amount
```

Without this, an implementation announcing an amount nobody submitted contradicts nothing. In source formats 1–3, the block
is optional per field: `invoice_id` has no line because the identity is the implementation's to assign.

Source `ess/4`, introduced in 0.23.0, requires every field of each emitted event to have a mapping. Use
`invoice_id: {generated: true}` for an explicitly implementation-generated identity.
A minted `Optional` value an invariant requires is declared `{generated: true, present: true}` (from `ess/27`), in `sets:` as in `payload:`; `{generated: true}` alone lets the implementation leave it absent.
Events with no emitting outcome remain valid; they may have an external producer.

A command may declare a closed typed `response` record and map one of its fields with
`item: {response: item}`. This reads the returned response; the string `response.item`
retains its historical literal meaning. Input mappings keep their existing spelling.
Missing fields, unknown response members and incompatible types are refused. Conformance
checks compare mapped values with the actual response from the same command invocation.

## An error's fields can have a declared source

From `format: ess/19`, an outcome that reports an error may say what fills the error's fields, in a
`payload:` block keyed by the error, the same way it does for an event:

```yaml
- name: too-many
  when: quantity > 10
  error: demo.order.TooMany
  payload:
    demo.order.TooMany: {requested: input.quantity, limit: 10, reason: Quantity}
- name: already-closed
  wrong_state: true
  error: demo.order.AlreadyClosed
  payload:
    demo.order.AlreadyClosed: {order_id: input.order_id, quantity: {subject: quantity}}
```

A field takes the sources an event field takes: an input field, a literal, `{subject: …}`,
`{caller: …}` or `{generated: true}`. `{subject: …}` reads the row the refusal is answered for, so
it is refused on a refusal that reads no row, such as an input-guarded one on a command that
creates. A field the error does not declare, and a source of another type than the field, are
refused as they are on an event; `{cleared}` and a response field are refused on an error. A field
needs no line: without one, the implementation fills it, as before `ess/19`. The conformance suite
compares each field that has a source, except a generated one, which is the implementation's to
choose. Earlier formats refuse the block as `unsupported_format_version`. The
[fixture](https://github.com/beyond10x/ess/blob/main/crates/verify/ess-conformance/tests/fixtures/error-payload-sources.yaml)
covers every refusal position.

From `ess/23`, `{subject: state}` reads the lifecycle state the row held, the state before the move:
in an error payload, an event payload and `sets:`, wherever `{subject: …}` is admitted, typed as the
entity's own `State`. `current: {subject: state}` on a `wrong_state:` refusal says which state the
record is in, and each `<entity>/state/<S>/refuses/<command>` scenario requires `S` there. Below
`ess/23` it is refused naming `ess/23`. The generated Rust and Go behaviours read the held row's
state; Entity Runtime lowering refuses it with the other `{subject: …}` values.

A sourced error field is also what lets synthesis generate a refusal; see
[Synthesize code from a specification](../synthesize.md#generated-behaviour-over-ports-you-provide).
