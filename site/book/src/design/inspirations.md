# Inspirations

Ignite borrows most of its ideas. This page names where they come from: for each project, what
Ignite takes from it and where Ignite does something different. The next page,
[Ignite among related work](related-work.md), puts them side by side and says what the
combination adds.

Two rules hold for both pages. Every statement about another project is taken from that
project's own documentation, linked where it is made, as read on 2026-11-04. Every statement
about Ignite describes release v0.4.0, an early prototype: one local host, two native targets,
and "planned" wherever something is designed and not built.

## Why Ignite exists

Two problems started the project.

- **Build time with many agents.** Ignite started as a replacement for Rust. In the words of
  the [README](https://github.com/ignite-lang/ignite#why-ignite), rustc's compile times "bring
  a machine to a halt when several coding agents build at the same time, and caches did not
  fix that." The LLVM target is the fast path: at the v0.3.0 close the todo HTTP service built
  in about 0.29 s (LLVM, dev) against 3.1 s (Rust target through cargo, dev), from a fresh
  output directory ([Building for Rust and LLVM](../guide/targets.md#build-times-as-measured)).
  The compiler itself is still Rust, so changing the compiler is no faster; a self-hosted
  compiler is planned ([roadmap](../roadmap.md)).
- **Intent spread over many files.** What a system is for usually lives in application code,
  infrastructure definitions, deployment configuration, monitoring rules and runbooks, and
  those copies drift apart ([The idea](index.md)). Ignite's answer is one typed program from
  which the executables, the system model and the control behaviour are derived.

The projects below are grouped by the part of Ignite they shaped.

## Languages and proofs

### Lean 4

Lean is "an open-source programming language and proof assistant" that "compiles to native
code" ([lean-lang.org](https://lean-lang.org/)). Its system description says Lean 4 "is a
reimplementation of the Lean interactive theorem prover (ITP) in Lean itself" and that "users
can modify and extend the parser, elaborator, tactics, decision procedures, pretty printer,
and code generator"
([de Moura and Ullrich, CADE 2021](https://lean-lang.org/papers/lean4.pdf)).

- **What Ignite takes.** A language that is its own tool. Ignite's compiler calls Ignite code
  while it checks a program: component kinds are record types in `std/`, their `validate`
  hooks refuse declarations ([E120](../reference/diagnostics.md#e120)), and `std.deploy`'s
  `admits` and `includes` decide how an assembly resolves
  ([The compile-time evaluator](evaluator.md)). The vocabulary of systems, contracts and
  policies is library code, not syntax. Lean is also the worked example for Ignite's plan of a
  compiler written in its own language with a staged bootstrap
  ([`docs/design.md`](https://github.com/ignite-lang/ignite/blob/main/docs/design.md) section
  12; planned).
- **Where Ignite differs.** Ignite has no dependent types, no theorems and no proofs. It has no
  macros or user-defined syntax either; the design defers them (section 3.5) and puts domain
  nouns in the library instead. An Ignite contract is measured while the system runs; a Lean
  theorem is checked before it runs. The full comparison, dated 2026-11-01, is
  [`docs/research/lean.md`](https://github.com/ignite-lang/ignite/blob/main/docs/research/lean.md).

### Dafny and TLA+

Dafny is a "verification-aware programming language that has native support for recording
specifications", "equipped with a static program verifier", with pre- and post-conditions among
its tools; it compiles to C#, Java, JavaScript, Go and Python ([dafny.org](https://dafny.org/)).
TLA+ is "a high-level language for modeling programs and systems--especially concurrent and
distributed ones" ([TLA+ home](https://lamport.azurewebsites.net/tla/tla.html)); it "is used to
model systems above the code level" and "does not resemble a programming language"
([a high-level view](https://lamport.azurewebsites.net/tla/high-level-view.html)).

- **What Ignite takes.** A statement of what must hold, kept apart from the code that makes it
  hold, and checked before anything acts: the compiler rejects an incompatible binding, and
  `ignite plan` evaluates every property before `ignite run` or `ignite apply` starts a process.
- **Where Ignite differs.** Ignite proves nothing. Its checks are types, effects, exhaustive
  `match`, evaluation of the declaration and tests; its contracts end in a standing of
  `Compliant`, `Violated` or `InsufficientEvidence`, not a proof
  ([Contracts, policies and control](contracts.md)). Where TLA+ models a system above the
  code, Ignite's model is derived from the code that runs. The protocol design rejects a
  separate model in TLA+ because "it drifts from the code" and plans to explore the protocol
  code itself instead
  ([`docs/protocols.md`](https://github.com/ignite-lang/ignite/blob/main/docs/protocols.md)
  section 3.5; planned).

## Infrastructure as code and configuration

### Winglang

Wing calls itself "a unified programming model, combining infrastructure and runtime code into
one language" ([GitHub](https://github.com/winglang/wing)). Preflight is "code that runs once,
at compile time, and generates the infrastructure configuration of your cloud application";
inflight is "code that runs at runtime and implements your application's behavior", and
"inflights are typically compiled into JavaScript"
([preflight and inflight](https://www.winglang.io/docs/concepts/inflights)). Wing ships a local
simulator and has duration literals such as `2s` and `3h`
([language reference](https://www.winglang.io/docs/api/language-reference)). Its README calls
it "a pre-release".

- **What Ignite takes.** Infrastructure and application in one language, with a phase that is
  evaluated at compile time. Ignite's property expressions are pure and the compiler evaluates
  them into a system model ([The compile-time evaluator](evaluator.md)); functions run at run
  time.
- **Where Ignite differs.** Ignite compiles to native executables, not to JavaScript plus
  configuration for a provisioning engine. It has no cloud provider and no simulator: it runs
  the real executables on one local machine under `ignite run` or `ignite host`. Wing deploys
  to clouds such as AWS; Ignite does not deploy anywhere else yet.

### Pulumi

"Pulumi programs, written in general-purpose programming languages, describe how your cloud
infrastructure should be composed", and the engine "uses the last deployed state to compute the
minimal set of changes needed"; `pulumi preview` shows "the operations it will perform on each
resource" ([how Pulumi works](https://www.pulumi.com/docs/iac/concepts/how-pulumi-works/)).
Pulumi Cloud can run scheduled drift detection and, with auto-remediation, "runs `pulumi up
--refresh` after a detection run finds drift"
([drift](https://www.pulumi.com/docs/deployments/concepts/drift/)). JavaScript callbacks can be
serialized into deployed functions
([function serialization](https://www.pulumi.com/docs/iac/concepts/functions/function-serialization/)).

- **What Ignite takes.** A typed, programmable description of the deployment, a plan computed
  from the last applied state and shown before anything changes, and apply as a separate step:
  `ignite plan --for-apply` saves a plan bound to its source, and `ignite apply` carries it out
  with durable receipts ([Local apply](../guide/apply.md)).
- **Where Ignite differs.** Pulumi borrows existing languages and drives cloud providers; it is
  in production use. Ignite is its own language, and the same program holds the application
  logic that becomes the executables. Ignite has no providers: apply manages processes on one
  local host.

### CUE

"CUE is an open-source data validation language and inference engine with its roots in logic
programming"; "it merges types and values into a single concept", and "the language is not a
general-purpose programming language" ([introduction](https://cuelang.org/docs/introduction/)).
Values form "a lattice", and unification is its foundation
([the logic of CUE](https://cuelang.org/docs/concept/the-logic-of-cue/)).

- **What Ignite takes.** Configuration is checked as a whole before anything runs, and the
  checks live next to the values. A component kind is a record type with defaults, and its
  `validate` hook can refuse a declaration ([Components](components.md)).
- **Where Ignite differs.** CUE unifies constraints; Ignite evaluates ordinary functions with a
  fuel limit. Ignite is a general-purpose language that produces programs; CUE validates and
  generates data.

### Nix

The Nix language is "pure", "lazy" and "dynamically typed", and derivations are "precise
descriptions of how contents of existing files are used to derive new files"
([Nix language](https://nix.dev/manual/nix/stable/language/)). Nix "builds packages in isolation
from each other", which "ensures that they are reproducible", and it "allows you to roll back
to previous versions" ([nixos.org](https://nixos.org/)).

- **What Ignite takes.** A pure evaluation of the description, and outputs identified by their
  inputs: a saved plan pins its source, its inputs and the artifacts it will run, and is
  refused when they change ([Local apply](../guide/apply.md)).
- **Where Ignite differs.** Ignite is statically typed, and its plans cover running processes
  and their objectives, not packages and their builds. Ignite has no package manager yet.

### Dhall and Nickel

Dhall is "JSON + functions + types + imports" and "a total programming language that forbids
arbitrary side effects" ([dhall-lang.org](https://dhall-lang.org/)). Nickel uses "contracts to
validate your data and ensure they conform to a given schema" and "(opt-in) static typing"
([nickel-lang.org](https://nickel-lang.org/)).

- **What Ignite takes.** Configuration is typed and cannot act on the world: an effect in a
  property expression is [E116](../reference/diagnostics.md#e116).
- **Where Ignite differs.** Dhall rules out non-termination by construction; Ignite allows
  recursion and loops and stops evaluation at a fuel limit
  ([E115](../reference/diagnostics.md#e115)). The word "contract" means different things:
  Nickel's validates data, Ignite's is an objective measured against a running system.

## Operations and reconciliation

### Kubernetes controllers and operators

"In Kubernetes, controllers are control loops that watch the state of your cluster, then make
or request changes where needed. Each controller tries to move the current cluster state closer
to the desired state" ([controllers](https://kubernetes.io/docs/concepts/architecture/controller/)).
Operators "are clients of the Kubernetes API that act as controllers for a Custom Resource" and
"aim to capture the key aim of a human operator"
([operator pattern](https://kubernetes.io/docs/concepts/extend-kubernetes/operator/)). Liveness
probes "determine when to restart a container"
([probes](https://kubernetes.io/docs/concepts/configuration/liveness-readiness-startup-probes/)).
Kubernetes "does not deploy source code and does not build your application"
([overview](https://kubernetes.io/docs/concepts/overview/)).

- **What Ignite takes.** Declared desired state and machines that act to keep it: a supervisor
  starts and restarts a server under its `Restart` policy, a scheduler runs probes, a rollout
  replaces a server and drains it ([Contracts, policies and control](contracts.md)).
- **Where Ignite differs.** Ignite's machines are pure functions in
  [`std.control`](../reference/std/control.md), and the same program declares the system and
  holds the application it builds. The set of machines is closed (four, user-defined machines
  are a later slice), and they run on one host. Kubernetes runs across a cluster and is
  extended by any program that speaks its API.

### Erlang/OTP supervision

"A supervisor is responsible for starting, stopping, and monitoring its child processes", and
"if more than `MaxR` number of restarts occur in the last `MaxT` seconds", it "terminates all
the child processes and then itself"; the defaults are `1` and `5`
([supervision principles](https://www.erlang.org/doc/system/sup_princ.html)).

- **What Ignite takes.** A failure ends its task and is reported, and the service goes on: a
  failed `assert` or an overflow ends one session of `terminald`, not the server
  ([FAQ](../faq.md)). Restarts are bounded: `Restart` has a backoff, `maximum_attempts` and an
  `interval`. Typed actors with OTP's supervision strategies are designed
  ([`docs/actors.md`](https://github.com/ignite-lang/ignite/blob/main/docs/actors.md); planned).
- **Where Ignite differs.** Ignite has no lightweight processes yet: a task is an OS thread, and
  the supervisor restarts whole executables, not processes inside one.

### Temporal

Temporal promises "the Durable Execution of your code in the face of any failure"
([temporal.io](https://temporal.io/)). A workflow's event history is "a complete, ordered log
of everything that has already happened in a Workflow"; on recovery Temporal "replays the Event
History step by step", so workflow code "has to make the same decisions when given the same
history" ([workflows](https://docs.temporal.io/workflows)). Workers run "on hosts external to
the Temporal Service", as many as needed ([workers](https://docs.temporal.io/workers)).

- **What Ignite takes.** Decisions as deterministic code over an ordered journal. `ignite run`
  journals every event before the step and every effect before dispatch, and `ignite replay`
  re-steps the journal and compares every record, byte for byte
  ([E127](../reference/diagnostics.md#e127)).
- **Where Ignite differs.** In Temporal the rule is a discipline for workflow code in
  general-purpose languages; in Ignite the reducers are pure functions, and the evaluator that
  steps them does no IO. Temporal replays to resume a workflow on many hosts; Ignite's replay
  checks that a run's decisions are reproducible, on one host.

## Language design

### Effects and capabilities: Koka and Austral

"Koka tracks the (side) effects of every function in its type, where pure and effectful
computations are distinguished", and can "compile directly to C code without needing a garbage
collector or runtime system" ([Koka](https://koka-lang.github.io/koka/doc/index.html)). In
Austral, "the root of the capability hierarchy is a value of type `RootCapability`", the "first
argument to the entrypoint function", motivated by the observation that "code is overwhelmingly
permissionless" ([Austral specification](https://austral-lang.org/spec/spec.html)).

- **What Ignite takes.** Effects in function types, inferred where they are not declared
  (Koka), and authority passed as values rather than held by all code (Austral): a function that
  writes to `stream` needs `stream.write` on that parameter
  ([Effects and capabilities](effects.md)).
- **Where Ignite differs.** Ignite has no effect handlers and no user-defined effects, and no
  linear types; values are copied. A few process-wide operations (`stdin.read`, `net.listen`,
  `clock.read` and others) are named directly rather than derived from a root capability.

### The Elm architecture

An Elm program is a model, a view and an update, with
`update : Msg -> Model -> (Model, Cmd Msg)` ([architecture](https://guide.elm-lang.org/architecture/),
[HTTP](https://guide.elm-lang.org/effects/http.html)); programs "send `Cmd` and `Sub` values to
the runtime system" ([effects](https://guide.elm-lang.org/effects/)).

- **What Ignite takes.** The shape of every control machine:
  `step(machine, event) -> Step`, where a `Step` is the next machine and the list of effects to
  carry out. The driver's adapters carry the effects out and feed what they observe back as
  events ([`std.control`](../reference/std/control.md)).
- **Where Ignite differs.** There is no view. The effects start processes, run probes and set
  timers, and every event and effect is journaled so the run can be replayed.

### Ballerina

Ballerina is "optimized for integration"; "network primitives in the language make it simpler
to write services" ([ballerina.io](https://ballerina.io/)). Services work "with a listener
object", and "client objects allow Ballerina developers to communicate with a remote process
that follows a given protocol"
([cloud native](https://ballerina.io/why-ballerina/cloud-native/)).

- **What Ignite takes.** The network belongs in the program: a `TcpListener` is a typed
  resource, and a `protocol` declares its `serve` and `connect` roles
  ([Protocols and roles](protocols.md)).
- **Where Ignite differs.** No protocol is built in. Telnet and HTTP/1.1 are Ignite code in
  `std/`, generic over any `ByteStream`, and the compiler and substrate know only byte streams.
  Since v0.4.0, the requirements of five RFCs are catalogued as Ignite data, and
  `ignite conformance` prints which requirements the code claims and which tests verify them;
  the Telnet matrix claims RFC 855, and the HTTP matrix lists its gaps and claims nothing
  ([`PLAN.md`](https://github.com/ignite-lang/ignite/blob/main/PLAN.md), slice 15).

## Compile speed

### Go

Go's own account: "The origin myth for Go states that it was during one of those 45 minute
builds that Go was conceived", and "a factor of fifty less to do turns minutes into seconds,
coffee breaks into interactive builds"
([Go at Google, 2012](https://go.dev/talks/2012/splash.article)).

- **What Ignite takes.** Build time as a requirement of the language, not an afterthought. The
  repository measures program builds and its own test gate at every slice and records them in
  [`AGENTS.md`](https://github.com/ignite-lang/ignite/blob/main/AGENTS.md).
- **Where Ignite differs.** The article names Go's handling of dependencies as "the single
  biggest reason why Go compilations are faster than C or C++ compilations".
  Ignite's fast path emits LLVM IR text for the standard library, the program and one main per
  executable, and hands it to clang, with no cargo in between
  ([One substrate, two targets](substrate.md)).

### Zig

"Zig 0.15.x enables Zig's self-hosted x86_64 code generation backend by default in Debug
mode", and "Debug compilation is 5 times faster"; incremental compilation "is still
experimental" ([0.15.1 release notes](https://ziglang.org/download/0.15.1/release-notes.html)).
Zig "can also evaluate functions and blocks of code at compile-time"
([overview](https://ziglang.org/learn/overview/)).

- **What Ignite takes.** The debug build is the one that matters in the edit-build-test loop,
  and ordinary functions can run at compile time (Ignite's evaluator, bounded by fuel).
- **Where Ignite differs.** Zig wrote its own code generator; Ignite relies on clang for both
  the dev and the release profile.

### Caching: sccache

sccache "is a ccache-like compiler caching tool" with Rust support. Its README lists the
limits: "Crates that invoke the system linker cannot be cached. This includes `bin`, `dylib`,
`cdylib`, and `proc-macro` crates", "Incrementally compiled crates cannot be cached", and "by
default, absolute paths to files must match to get a cache hit"
([sccache](https://github.com/mozilla/sccache)).

- **What Ignite took from it.** sccache was the approach tried first: the repository's
  build-time measurements run with it on, and in the README's words "caches did not fix" the
  machine-wide slowdown under concurrent agents.
- **Where Ignite differs.** Instead of caching the work, Ignite reduces it. An inference, not a
  measurement: the caveats above bite exactly in the agent setting, where each agent builds in
  its own worktree (paths differ) and every build ends in a linked binary.

## Specification-driven agents: ESS

[ESS](https://github.com/beyond10x/ess) describes "systems as typed data, compiling those
descriptions into deterministic IR, projecting concrete artifacts, and checking implementations
or infrastructure against what was declared." Its conformance suite derives scenarios from the
specification, and a construct it cannot test "is refused, not omitted"; each synthesized piece
is generated, an obligation for a human, or refused
([concepts](https://github.com/beyond10x/ess/blob/main/website/docs/concepts/ess.md)).

- **What Ignite takes.** Ignite is meant as a native realization target for ESS: intent becomes
  an ESS specification, an agent writes the Ignite program, it builds to a native executable,
  and ESS's conformance suite checks that executable like any other. That loop is planned, not
  built
  ([`docs/research/review-2026-11-04-ecosystem.md`](https://github.com/ignite-lang/ignite/blob/main/docs/research/review-2026-11-04-ecosystem.md));
  `epic:agent-delivery-benchmark` will measure it against writing Rust directly. ESS's habit of
  saying what has no evidence shows up in Ignite too: a contract without enough data is
  `InsufficientEvidence`, and `ignite conformance` marks a requirement that is claimed but
  has no passing test `inconclusive`.
- **Where Ignite differs.** ESS is declarative and language-independent, and holds no
  executable behaviour; Ignite is the code. Neither depends on the other
  ([`docs/research/ess.md`](https://github.com/ignite-lang/ignite/blob/main/docs/research/ess.md)).

Next: [Ignite among related work](related-work.md).
