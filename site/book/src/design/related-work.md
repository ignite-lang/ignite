# Ignite among related work

This page puts six of the projects from the [previous page](inspirations.md) next to Ignite,
then lists what Ignite does not have and what the combination adds. The sources for the cells
are the links on the previous page, and three more:

- Lean's `IO` actions "are just descriptions of tasks to be carried out"
  ([Functional Programming in Lean](https://lean-lang.org/functional_programming_in_lean/Hello___-World___/Running-a-Program/)).
- Lean 4.31 "graduates the Async and Http modules out of Internal into Std", with "a pure
  HTTP/1.1 state machine that incrementally parses incoming byte streams"
  ([release notes](https://lean-lang.org/doc/reference/latest/releases/v4.31.0/)).
- Kubernetes resource limits are written as quantities such as `hugepages-2Mi: 80Mi`
  ([resource management](https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/)).

## Comparison

"Yes" means the project does it as part of what it is. "Partial" is explained in a footnote.
"No" means it is not part of the project, often by choice of scope. Ignite's column describes
v0.15.0.

| capability | Ignite | Lean 4 | Winglang | Pulumi | CUE | Kubernetes controllers | Temporal |
|---|---|---|---|---|---|---|---|
| its own compiler to native executables | yes | yes | no | no | no | no | no |
| application code and the system declaration in one language | yes | no | yes | partial[^pulumi-app] | no | no | no |
| quantities with units in the language | yes | no | partial[^wing-duration] | no | partial[^cue-multiplier] | partial[^k8s-quantity] | no |
| effects checked by the type checker | yes | partial[^lean-io] | partial[^wing-phase] | no | no | no | no |
| network protocols written in the language | yes | yes | no | no | no | no | no |
| objectives measured against the running system | partial[^ignite-contract] | no | no | no | no | partial[^k8s-probe] | no |
| desired-state reconciliation | partial[^ignite-machines] | no | no | partial[^pulumi-drift] | no | yes | no |
| decisions as deterministic code, replayed from a journal | yes | no | no | no | no | no | yes |
| machine-checked proofs | no | yes | no | no | no | no | no |
| manages more than one machine | no | no | yes | yes | no | yes | yes |

[^pulumi-app]: Pulumi programs describe infrastructure in a general-purpose language, and
    JavaScript callbacks can be serialized into deployed functions. The services it deploys are
    otherwise built on their own.

[^wing-duration]: Wing has duration literals (`1m`, `2s`, `3h`). Its language reference shows
    no size or ratio literals.

[^cue-multiplier]: CUE integer literals take SI and IEC multipliers (`1.5G`, `1.3Ki`). They
    scale the number and carry no unit.

[^k8s-quantity]: Resource requests and limits are written with units such as `80Mi` in
    manifests. The controllers themselves are programs in other languages.

[^lean-io]: Effectful code has type `IO`, so pure and effectful code are told apart. The type
    does not say which socket or file an action touches.

[^wing-phase]: The compiler checks whether code is preflight or inflight. It does not track
    which resource a call acts on.

[^ignite-contract]: `Availability`, `ThresholdCompliance` and `SLA` are evaluated over probe
    attempts with missing data as `InsufficientEvidence`. The outcome a probe reports is one
    of two built-in texts, and it all runs on one host.

[^k8s-probe]: Liveness and readiness probes restart a container or take it out of service.
    There is no objective over a window of observations.

[^ignite-machines]: Four built-in machines (supervise, schedule, evaluate, roll out) keep the
    declared processes running on one local host. User-defined machines are a later slice.

[^pulumi-drift]: `pulumi up` moves resources to the declared state when it is run. Pulumi
    Cloud can detect drift on a schedule and remediate it with `pulumi up --refresh`.

## What Ignite does not have

- **No proofs.** Lean and Dafny prove properties of code. Ignite tests them and measures
  objectives. A contract states and measures. It does not guarantee.
- **One machine.** Pulumi, Kubernetes and Temporal run production systems across many
  machines. Ignite runs and applies on one local host, with no cloud provider and no
  distributed state.
- **A young ecosystem.** No package manager, no registry, no language server. Every project in
  the table is older and in wider use.
- **A Rust compiler.** Programs build fast on the LLVM target. The compiler itself is a Rust
  workspace and builds at Rust's speed until it is self-hosted (planned).

## What Ignite adds

Each part of Ignite has a precedent above. None of the projects in the table holds all of them
in one typed program, checked by one compiler:

1. **A compiled native language.** The program builds to native executables on two targets,
   Rust through cargo and LLVM through clang, over one IO substrate written once
   ([One substrate, two targets](substrate.md)).
2. **Systems declared in the same language.** Resources, components, contracts, policies and
   assemblies are declarations whose kinds are ordinary record types in `std/`. The compiler
   evaluates them into a system model that `ignite plan` prints and `ignite explain` traces
   back to source ([Systems and assemblies](systems.md)).
3. **Typed quantities.** `4096 bytes`, `15 minutes` and `99.9%` are values with units that
   the type checker knows ([Intent and properties](intent.md)).
4. **Contracts measured at run time.** A contract names a signal, a target and a window, and
   `ignite run` evaluates it on every probe attempt. Zero traffic is not compliance
   ([Contracts, policies and control](contracts.md)).
5. **Pure reducers for operations, with journal replay.** Supervision, scheduling, evaluation
   and rollout are pure Ignite functions in `std.control`. The journal of a run is enough to
   reproduce every decision with `ignite replay`
   ([The compile-time evaluator](evaluator.md)).
6. **Protocols written in the language.** Telnet and HTTP/1.1 are Ignite code over any byte
   stream, tested in process over a pipe, with their RFC requirements as data.
   `ignite conformance` reports what is claimed and verified
   ([Protocols and roles](protocols.md)).
7. **Fast builds for many agents.** The reason the project exists: a program builds in a
   fraction of a second on the LLVM target, where the same program takes seconds through
   cargo ([Building for Rust and LLVM](../guide/targets.md)).

Because they share one compiler, a change in one part is checked against the others. An
effect in a property expression, a binding an assembly does not admit, a component without
the entry its host needs and a journal that no longer replays are each a diagnostic with a
code ([Diagnostics](../reference/diagnostics.md)).

Ignite is an early prototype. The claim is about the combination, not about any single part
being better than the project that inspired it. Where it goes next is on the
[roadmap](../roadmap.md). The specification-driven loop with ESS is the next test of whether
the combination pays off for agents.
