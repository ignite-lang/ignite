# Ignite compared with Lean 4

Date: 2026-11-01. Ignite state: `design.md` v0.2, `docs/slice.md` (slices 1 to 7),
`docs/llvm.md`, `AGENTS.md`, `std/`, `examples/`. Lean state: the Lean 4 reference manual
(the "latest" manual covers 4.36.0-rc2, per its front page
https://lean-lang.org/doc/reference/latest/), release notes, FRO roadmap Y4-1, and the
system paper. Lean was not installed or run.

Snapshot; `std/protocol/http.ignite` and `Store` (`crates/ignite-io/src/store.rs`) exist since
later on 2026-11-01, and slice 8 has landed since (`docs/slice.md` §12 "Slice 8: tests").

Paths without a directory are under the root of this repository (this file is `docs/research/lean.md`). `Design §n` is
`design.md`.

Tags: **[R]** read in the cited source. **[I]** my inference, not stated by the source.
A table cell with no tag is [R].

Shorthand for Lean sources used several times:

- `[L-paper]` de Moura and Ullrich, "The Lean 4 Theorem Prover and Programming Language
  (System Description)", CADE 2021: https://lean-lang.org/papers/lean4.pdf
- `[L-rc]` reference counting: https://lean-lang.org/doc/reference/latest/Run-Time-Code/Reference-Counting/
- `[L-rec]` recursive definitions: https://lean-lang.org/doc/reference/latest/Definitions/Recursive-Definitions/
- `[L-eval]` interacting with Lean (`#eval`, `#reduce`, `#guard_msgs`): https://lean-lang.org/doc/reference/latest/Interacting-with-Lean/
- `[L-task]` tasks and threads: https://lean-lang.org/doc/reference/latest/IO/Tasks-and-Threads/
- `[L-do]` `do` notation: https://lean-lang.org/doc/reference/latest/Functors___-Monads-and--do--Notation/Syntax/
- `[L-lake]` Lake: https://lean-lang.org/doc/reference/latest/Build-Tools-and-Distribution/Lake/
- `[L-trust]` validating a proof: https://lean-lang.org/doc/reference/latest/ValidatingProofs/
- `[L-boot]` bootstrapping: https://github.com/leanprover/lean4/blob/master/doc/dev/bootstrap.md
- `[L-road]` FRO roadmap Sep 2026 to Feb 2027: https://lean-lang.org/fro/roadmap/y4-1/
- `[L-ffi]` FFI: https://lean-lang.org/doc/reference/latest/Run-Time-Code/Foreign-Function-Interface/

## 1 Comparison table

| dimension | Ignite | Lean 4 |
|---|---|---|
| Purpose and target domain | A language for intent across a whole system: components, resources, connections, contracts (SLI/SLO/SLA), policies and assemblies, plus a small implementation language (Design §1.1, §2, §3.2 "Declaration primitives" table). The deliverable is an operated system: plan, controller, recovery (Design §1.3 PRD01-PRD11). It explicitly "does not infer application correctness" (Design §1.4). Implemented today: protocol servers and clients (Telnet, an HTTP todo backend in progress) compiled to native binaries (slice.md, opening "Direction" paragraph; slice.md §11). | A programming language and proof assistant: "Write software. Prove it correct." (https://lean-lang.org/). The domains are formal mathematics (Mathlib, "over a million lines"), software verification (AWS Cedar), and proof automation written in Lean itself (https://lean-lang.org/; [L-paper] abstract). It does not model deployment or operations. [I] |
| Type system | Nominal records, enums, tuples and aliases. Generics with **no bounds**: a type parameter supports only binding, passing and storing; `==` and methods on it are E045/E041 (slice.md §11.1 "Generics"). Type arguments are inferred at calls and never written (slice.md §11.1; §11.7 "Frontend contract (T1)", "Generic typing"). Bidirectional checking (docs/llvm.md §2 "Types"). `match` exhaustiveness E046 (slice.md §8 "Diagnostics"). Fn types can carry effects (slice.md §11.2 "Effectful fn types"). Not dependent. Design adds quantities with units, money, `Observation<T>`, `Capability<T>`, `Deferred<T>` (Design §3.3), none of them implemented yet: quantity literals are "not in the slice" (slice.md §2 "Lexical structure"). | Dependent type theory: "Lean is based on a version of dependent type theory known as the Calculus of Constructions", types are first-class, with universe polymorphism (https://lean-lang.org/theorem_proving_in_lean4/Dependent-Type-Theory/). Type classes with instances (https://lean-lang.org/doc/reference/latest/Type-Classes/Instance-Declarations/), with tabled resolution ([L-paper] abstract). Type inference through elaboration; annotations can be omitted (same TPIL page). Propositions are types, so specifications live in the type system. |
| Effects and authority | Effects are capability-rooted and checked per value: `stream.read`, `stdin.read`, `net.listen`. A `requires` list is declared or inferred as a fixed point (slice.md §5 "Effects", §5.1 "Effect sets"). A callee's effects on its parameters map to the roots of the caller's arguments (slice.md §5.1, "Roots"). Missing effect E016, effectful fn as a value E017 (slice.md §8 "Diagnostics"). Calls need no marker; `await` was removed (slice.md §5; §10.2 "Deprecated `await`"). `p.*` wildcard on effectful fn types (slice.md §11.2 "Effectful fn types"). Design: pure evaluation cannot do ambient I/O, and authority is passed explicitly (Design §3.1, §3.3 `Capability<T>`, §9.3). | Effects are monadic. An IO action "is just a description of tasks to be carried out", programs are "free of side effects" from Lean's internal view, and the runtime system runs `main : IO Unit` (https://lean-lang.org/functional_programming_in_lean/Hello___-World___/Running-a-Program/#running-a-program). Effects are structured with monads and transformers (FPIL ch. 4-6, https://lean-lang.org/functional_programming_in_lean/). [I] No per-resource authority: the `IO` type does not say which sockets or files an action touches, and I found no capability system in the sources read. |
| Memory model and runtime | Value semantics: every place in a value position is copied (slice.md §6 "Rust lowering", "Ownership rule"). Rust target: `.clone()` (same). LLVM target: deep copy functions per type, borrowed parameters inferred when the callee never modifies them (docs/llvm.md §3 "Representation", calling convention; §4 "Values and memory"), per-task bump arenas with loop regions released at back-edges (docs/llvm.md §4). No GC and no RC. The memory model is an open question (Design §14.2, "Implementation memory model"); reference counting with cycles prohibited is a named candidate (Design §8.5). Runtime: `ignite-io` (Rust, blocking) plus the C ABI adapter `ignite-rt` (AGENTS.md, "Rules of the layout"; docs/llvm.md §5 "Runtime C ABI (`ignite-rt`)"). | Reference counting with destructive updates on unshared values ("functional but in-place", FBIP) and reuse of a dying cell for a new one ("resurrection hypothesis") ([L-paper] §3; [L-rc]). Borrow analysis elides many RC updates; single-threaded counts are cheaper than multi-threaded ones; cycles are not collected, because "the verifiable fragment of Lean cannot create cyclic data" ([L-rc]). C runtime; FFI over the C ABI with owned and `@&`-borrowed parameters ([L-ffi]). |
| Concurrency and async | Today: blocking IO, one OS thread per task, idle-thread reuse, `spawn` keyword, no event loop (slice.md §5 "Effects", lowering; §6.1 "System lowering"; §7 "Substrate `ignite-io`", "Tasks"). Slice 5 (planned): tokio inside `ignite-io`, Rust futures and LLVM coroutines run as tokio tasks, replacing the libuv candidate of Design §8.6 (AGENTS.md, "Rules of the layout"). Design: `await` lowered to resumable state machines, cancellation races, backpressure (Design §8.7). Shared state: `Store<T>` with atomic `update` (slice.md §11.3 "`Store<T>`"). | `Task α` on a thread pool, `Task.spawn` for pure tasks, `IO.asTask` for effectful ones, priorities, dedicated threads, cancellation when the last reference drops, promises, `Std.Channel`, `Std.Mutex` ([L-task]). Async IO on libuv: basic async framework and timers in 4.17 (https://lean-lang.org/doc/reference/latest/releases/v4.17.0/), libuv TCP in 4.19 (https://lean-lang.org/doc/reference/latest/releases/v4.19.0/), `Async` and `Http` moved from `Internal` into `Std` in 4.31 (https://lean-lang.org/doc/reference/latest/releases/v4.31.0/). |
| Compilation targets and bootstrap | Two targets, both from a Rust compiler: generated Rust built by cargo (default), and LLVM IR text compiled and linked by clang against `libignite_rt.a` (slice.md §1 "Pipeline"; docs/llvm.md opening "Direction" paragraph and §1 "Pipeline"). Self-hosting is a roadmap: M5 implementation subset, M6 frontend in Ignite, M7 bootstrap (Design §12.2); stages S0 → C1 → C2 → C3, compare C2 and C3, retain the seed (Design §12.5). The seed's language is independent of its output (Design §8.4). | Self-hosted: "Lean 4 is a reimplementation of the Lean interactive theorem prover (ITP) in Lean itself" ([L-paper] abstract). The compiler emits C; proofs are erased before IR; an IR interpreter switches to native code for precompiled functions ([L-paper] §3). New compiler (LCNF) replaced the old one in 4.22 (https://lean-lang.org/doc/reference/latest/releases/v4.22.0/). Bootstrap: stage0 is "pre-built C files checked into the repository", updated from stage1; "stage 3 should always be identical to stage 2 and only exists as a sanity check" ([L-boot]). |
| Metaprogramming and macros | None. Macros, implicit conversions, unrestricted reflection and user-defined syntax are deferred (Design §3.5). Compile plugins deferred "until ordinary packages cannot express a demonstrated need" (Design §9.2). Sugar is fixed desugaring in the parser (slice.md §10 "Slice 6: surface syntax"; §10.1, "Desugarings"). Compile-time evaluation: only pure functions under fuel and memory limits (Design §3.5); not implemented: constants lower to zero-argument functions evaluated at run time (slice.md §6 "Rust lowering", constant `N` lowers to `N()`). | Core feature. Hygienic macros, `syntax`, `macro_rules`, `notation`, elaborators, and low-level parser extensions (https://lean-lang.org/doc/reference/latest/Notations-and-Macros/). "Lean 4 is fully extensible: users can modify and extend the parser, elaborator, tactics, decision procedures, pretty printer, and code generator" ([L-paper] abstract). |
| Specification and verification | Contracts are measured runtime objectives with `compliant`, `violated` or `insufficient_evidence` status, not proofs (Design §4.1). Verification layers are test layers: language, semantics, control, runtime, adapters, native, product, bootstrap (Design §13.1). Today: fixtures with `.expected` diagnostics, goldens, real `cargo build`, real-socket tests (AGENTS.md, "Conventions"; slice.md §8 "Diagnostics", fixture list). Static checks: types, effects, exhaustiveness. No asserts or proofs in the language. Planned slice 8: inline `test "name" { … }` with `assert` and `ignite test` (from the task brief; not yet in the repository: no hit for `slice 8` in `docs/`). | Theorems checked by a small kernel; `#print axioms` lists what a result depends on ([L-trust]). `decide` synthesizes a `Decidable` instance and reduces it; `decide +native` / `native_decide` trusts the compiler and adds an axiom (https://lean-lang.org/doc/reference/latest/Tactic-Proofs/Tactic-Reference/; [L-trust]). `#eval` compiles and runs a term, IO included, and refuses terms that depend on `sorry` ([L-eval]). `#guard_msgs` checks a command's messages against its docstring ([L-eval]). `example` checks an unnamed statement (https://lean-lang.org/theorem_proving_in_lean4/Propositions-and-Proofs/). `lake test` runs a configured test driver; "Lake itself isn't a test framework" ([L-lake]). Property testing via the community package Plausible (https://reservoir.lean-lang.org/@leanprover-community/plausible). |
| Tooling | `ignite check`, `ignite emit`, `ignite build` (slice.md §1 "Pipeline"; AGENTS.md command table), driven by go-task (`task build`, `task test`, `task gate`). No LSP. No package manager: imports of non-`std` modules are E031 (slice.md §4.1 "Imports and the standard library"); manifests and lockfiles are designed (Design §9.2) but not built. Management CLI `plan`, `run`, `apply`, `explain`, `test` proposed (Design §10.1). | Lake: build system and package manager, `lakefile.toml` or `lakefile.lean`, `lake-manifest.json` lock file, Reservoir registry, remote artifact cache ([L-lake]). `elan` and a `lean-toolchain` pin ([L-lake]). An incremental LSP server built on the `Task` abstraction ([L-paper] §4). The roadmap adds `lake check` for re-checking with the official kernel and with alternative kernels ([L-road] "Trust"). |
| Maturity | "Proposed design, version 0.2" that "does not claim that a compiler, runtime, SDK, or any example is implemented" (Design status line and introduction). Seven slices built. Std is 4 files and about 1050 lines (`std/console.ignite`, `std/text.ignite`, `std/json.ignite`, `std/protocol/telnet.ignite`; counted with `wc -l`). `std/protocol/http.ignite`, named by slice 7 (slice.md §11.6 "Std and example (T3)"), does not exist yet. The slice 7 frontend contract still ends in a placeholder (slice.md §11.7 "Frontend contract (T1)"). Two examples. | Released at the start of 2021 ([L-paper] §1). The manual covers 4.36.0-rc2 (https://lean-lang.org/doc/reference/latest/). Mathlib has more than a million lines; AWS uses it for Cedar (https://lean-lang.org/). The Lean FRO publishes roadmaps, six-monthly since Sep 2026 (https://lean-lang.org/fro/roadmap/). Compiler work in that period: design now, implementation from 2027, covering stack allocation, uniqueness annotations and array code ([L-road] "Compiler"). |

## 2 Overlaps that matter for Ignite's goals

**Intent.** Both separate a statement of what should hold from the code that makes it hold,
at different levels. Ignite states intent as system-level objectives (contracts, policies,
capability requirements: Design §4, §5) and settles it by measurement at run time (Design
§4.1). Lean states intent as propositions about functions and settles it by proof
before run time ([L-trust]). [I] The two are complementary: Ignite has no notion of a
function-level specification, and Lean has no notion of an operational objective, an
observation window or missing evidence.

**Contracts.** [I] Lean has no SLO-like construct. Two parts of Ignite contracts are
mathematical, though, and that is where Lean overlaps:
- The accounting definitions: availability as good over eligible events, the empty
  denominator, a 100% target with zero error budget, and "zero traffic does not
  automatically prove compliance" (Design §4.1; §1.5: "zero contract
  success inferred from missing observations"). These are invariants of pure functions over
  event windows, so they can be stated as theorems.
- Policy arbitration and durable effects (Design §7.1, §7.2), including "do not convert
  uncertainty into a blind duplicate creation" (Design §7.1). These are safety properties of a
  reducer-plus-journal state machine (Design §2).

**Verification layers (Design §13.1).** Every layer is a test layer, and the design says
outright that differential testing "does not prove the seed is correct" (Design §12.4)
and that stage stability "is evidence of bootstrap consistency, not a proof"
(Design §12.5). Lean offers the missing kind of evidence, proof, for the layers whose
subject is pure: Language (typing, bounded evaluation), Semantics (completeness,
eligibility, window boundaries, overflow) and Control (deterministic replay, arbitration).
[I] Runtime, Adapters, Native and Product stay test-only. They involve crashes, providers,
sockets and terminals, which a proof about a model does not reach.

**Self-hosting (Design §12).** Lean is a worked example of the path Ignite plans:
- a compiler written in its own language;
- a textual low-level backend (C for Lean, LLVM IR text for Ignite: docs/llvm.md, opening "Direction" paragraph);
- an IR interpreter next to native code ([L-paper] §3), like Ignite's interpreter-plus-native
  plan (Design §2, §8.10);
- a staged bootstrap with a stage comparison ([L-boot]; Design §12.5).

There is one difference. Lean's seed is generated C refreshed from stage1 ([L-boot]), so the
latest compiler is effectively the only path. Ignite asks for a retained, independent seed
"rather than making the latest compiler the only available path" (Design §12.1).

**Runtime convergence.** [I] Lean's runtime arrived where Design §8.6 first pointed: libuv
for async TCP (releases 4.17, 4.19, 4.31, URLs in row 5). Ignite has since moved to tokio
(AGENTS.md, "Rules of the layout"). Lean's experience is evidence that a libuv substrate works for a
compiled functional language. It is no reason to reverse the tokio decision.

## 3 What Ignite could take from Lean

Costs: small means about one slice-sized change in one or two crates; medium means several
crates or a new subsystem; large means a change to a core representation or a new
toolchain dependency. [I] for all cost estimates.

### 3.1 Inline checks next to code (`#eval`, `example`, `theorem`, `#guard_msgs`)

- **What Lean does.**
  - `#eval` compiles and runs a term inside the file and refuses anything depending on
    `sorry` ([L-eval]).
  - `example` checks an anonymous statement
    (https://lean-lang.org/theorem_proving_in_lean4/Propositions-and-Proofs/).
  - `#guard_msgs` pins expected diagnostics in a docstring next to the command ([L-eval]).
  - All three run while the file is checked, not in a separate test phase ([L-eval]).
- **For Ignite.** Slice 8's `test "name" { … }` with `assert` is the `example` analogue. What
  to take on top of it [I]:
  1. **Pure tests run during `ignite check`.** A test block whose body is pure (the
     effect-inference rule, slice.md §5 "Effects") is evaluated at check time under the fuel
     limits the design already requires (Design §3.5). Effectful tests run under
     `ignite test`. Purity is already inferred, so the split needs no annotation.
  2. **A `#guard_msgs` equivalent for the compiler's own fixtures.** An expected-diagnostic
     block inline in a `.ignite` fixture would replace the separate `.expected` files
     (slice.md §8 "Diagnostics", fixture list; AGENTS.md, "Conventions").
- **Changes.**
  - Grammar and AST: `crates/ignite-syntax`.
  - Checker: `crates/ignite-check`, `crates/ignite-types`.
  - Runner: `crates/ignite`.
  - For item 1, a pure evaluator: the "IR interpreter" of Design §2 / M1 (§12.2), which does
    not exist yet. Today constants are evaluated at run time (slice.md §6 "Rust lowering").
  - Design §10.1 (`ignite test`) and §13.1 (a Language-layer row for inline checks).
- **Cost.** Slice 8 as briefed: medium. Check-time evaluation: medium to large, since it
  needs the pure interpreter. That interpreter is on the roadmap anyway (M1, M5).
- **Conflict.** None. Check-time evaluation is exactly the bounded compile-time evaluation of
  Design §3.1 and §3.5. The keyword budget (Design §1.5)
  absorbs `test` and `assert`.

### 3.2 `decide`: decide a proposition by evaluating it

- **What Lean does.** `decide` proves a goal by synthesizing a `Decidable` instance and
  reducing it in the kernel. `decide +native` uses compiled code and trusts the compiler
  through an added axiom (https://lean-lang.org/doc/reference/latest/Tactic-Proofs/Tactic-Reference/;
  [L-trust]).
- **For Ignite [I].**
  - Ignite has no propositions, but the useful part transfers: an `assert` quantified over a
    finite type, checked by enumeration. Example: every `Command` round-trips through
    `parse(render(c))`; examples/terminal/terminal.ignite has the enum (`Command`) and the
    parser (`parse`).
  - Enumerable types: payload-free enums, `Bool`, `Byte`, and tuples and enums of those.
    `crates/ignite-types/src/exhaustive.rs` already reasons about these finite domains for
    E046.
  - The result is a proof by exhaustion with the evaluator as the trusted base, like
    `decide +native` rather than kernel `decide`.
- **Changes.** `crates/ignite-types` (decide enumerability, extend the E046 domain model), the
  pure evaluator of 3.1, and Design §13.1 (Language and Semantics layers).
- **Cost.** Medium, and small once 3.1's evaluator exists.
- **Conflict.** None, as long as it stays bounded by fuel (Design §3.5). Be explicit
  that its trusted base is the Ignite evaluator, as Lean is explicit about `native` ([L-trust]).

### 3.3 `do` notation

- **What Lean does.** `do` gives monadic code an imperative style: `let mut` that "cannot
  escape the do block", `for`, `while`, `break`, `continue`, early `return`, and the nested
  action `(← e)` ([L-do]).
- **For Ignite.** It already has all of this, without monads:
  - `var`, `while`, `loop`, `for`, `break`, `continue` and `return` expressions (slice.md §3
    "Grammar");
  - `?` (slice.md §3, `postfix`; §6 "Rust lowering");
  - unmarked effectful calls (slice.md §5 "Effects").
  - A function with local `var` mutation and no effectful operation is pure (slice.md §5).

  That is the property Lean gets by desugaring `let mut` to state passing ([L-do]).
- **Take.** Nothing in syntax. [I] One thing to write down: when the proof export of section
  5 exists, Lean's desugaring (local mutation → `StateT`-like pure code, [L-do]) is the
  translation scheme for Ignite `var` and loops.
- **Changes.** None now. Later, the export tool of section 5.
- **Cost.** None.
- **Conflict.** Monadic `do` would conflict with the slice 6 decision that effectful calls need
  no marker (slice.md §5; §10.2 "Deprecated `await`").

### 3.4 `partial`, and totality as a property

- **What Lean does.** Definitions are total by default: structural recursion, or
  well-founded recursion with `termination_by` / `decreasing_by`. `partial` needs only an
  inhabited return type; the kernel treats the function as an opaque constant that is
  "neither unfolded nor reduced", while compiled code uses it "as usual" ([L-rec]). There is
  also `partial_fixpoint`, for monadic or tail-recursive functions that still get unfolding
  theorems ([L-rec]).
- **For Ignite [I].**
  - Every Ignite function is partial today: `while`, `loop` and recursion are unrestricted
    (slice.md §3 "Grammar", §5 "Effects"), and the design wants general recursion (Design §3.1).
  - Lean's lesson is to distinguish the two kinds and to restrict the trusted contexts, not
    to forbid recursion.
  - Ignite needs the distinction in two places. Compile-time evaluation already has the fuel
    answer (Design §3.5). The proof export of section 5 needs a totality fact per
    function.
  - Take it as an **inferred** property, as effects are (slice.md §5), not as a keyword:
    a function is total when it recurses only structurally on enum payloads and loops only
    with `for` over a list. `while` and `loop` make it partial.
- **Changes.** `crates/ignite-check` (a fixed point like effect inference), and Design §3.5
  (state the rule).
- **Cost.** Small to medium.
- **Conflict.** A `partial` keyword would spend the reserved-word budget (Design §1.5)
  on runtime code that the design says may recurse freely (Design §3.1). The
  inferred form conflicts with nothing.

### 3.5 Compiler-in-language self-hosting with a C backend

- **What Lean does.** Compiler written in Lean, emitting C ([L-paper] §1, §3). The IR is
  ordinary Lean data, so other backends can be written in Lean ([L-paper] §3). An IR
  interpreter calls precompiled native code ([L-paper] §3). The stage0 seed is checked-in
  generated C, and stage2 and stage3 are compared ([L-boot]).
- **What transfers to Ignite [I].**
  1. **The IR as plain language data.** Ignite's Code IR and Controller IR, once written in
     Ignite (M6, Design §12.2), should be ordinary Ignite records and enums, so a backend is a
     Ignite package (Design §9.1 "Module" level) and not a compiler change.
  2. **An interpreter that calls compiled code.** It makes check-time evaluation (3.1) cheap
     for std functions. This fits Design §2 (both modes consume the same semantics) and M1.
  3. **A C backend: not now.** Ignite already emits a textual backend (LLVM IR, docs/llvm.md, opening "Direction" paragraph).
     C would buy portability and independence from the LLVM version. Treat it as a fallback
     if the pinned LLVM version becomes a bootstrap problem (Design §14.2, "Backend
     reproducibility").
  4. **A checked-in generated seed: no** (section 4).
- **Changes.** Design §8 (backend choice), §12.2 (M5-M7), §12.5 (stage definitions); crates
  `ignite-llvm` and the future Ignite-written compiler.
- **Cost.** Items 1 and 2: medium, inside work already planned. Item 3: large.
- **Conflict.** Item 3 conflicts with the LLVM-first direction (Design §8.1; §8.4).
  A checked-in generated seed conflicts with AGENTS.md, "Conventions" (generated code is never
  committed) and with the retained independent seed (Design §12.1; §12.5).

### 3.6 Functional-but-in-place reuse (Perceus / Counting Immutable Beans) against Ignite's copy cost

- **What Lean does.**
  - Reference counting with in-place update when a value is unshared, and reuse of a dying
    cell for a same-shaped allocation ([L-paper] §3; [L-rc]).
  - Its example: `map f as` on an unshared list allocates nothing ([L-paper] §3).
  - Borrow inference elides RC updates ([L-rc]).
  - Cycles are not collected because the language cannot create them ([L-rc]).
  - The paper cites Perceus ([L-paper] ref. 10, Reinking, Xie, de Moura, Leijen,
    MSR-TR-2020-42) for FBIP. Lean's own scheme is "Counting Immutable Beans" ([L-rc]).
- **Ignite's cost today.**
  - Every place in a value position is copied, with a deep copy on the LLVM target
    (slice.md §6 "Rust lowering", "Ownership rule"; docs/llvm.md §4 "Values and memory").
  - `Store.get` and `values` return copies, and `put` stores a copy (slice.md §11.3 "`Store<T>`").
  - Mitigations already built: inferred borrowed parameters (docs/llvm.md §3
    "Representation", calling convention), copy-skipping for read-only `match` and `for`
    (docs/llvm.md §4), arenas with loop regions (docs/llvm.md §4).
  - The region scheme itself has to copy carried variables to stay linear
    (docs/llvm.md §4, "Loop regions").
- **Fit [I].**
  - Ignite values cannot form cycles: no reference or pointer type is in the type list
    (slice.md §6 "Rust lowering", type table; §7 "Substrate `ignite-io`"), and values never
    share buffers (docs/llvm.md §4, "Loop regions"). So
    RC without a cycle collector is sound for the same reason as in Lean.
  - Threads: a spawned task deep-copies its arguments into its own arena
    (docs/llvm.md §4, tasks and `spawn`). Most values would therefore stay thread-local, with non-atomic
    counts, which Lean notes are cheaper ([L-rc]).
  - The exceptions are the shared `application` value of a service (docs/llvm.md §4; §6 "Generated mains")
    and `Store` contents (slice.md §11.3). Both need either atomic counts or the deep
    copy they get today.
  - On the Rust target the same semantics is `Rc`/`Arc` plus clone-on-write when unique,
    with a cheaper clone as the result.
- **Changes.**
  - The LLVM target's memory model: `crates/ignite-llvm` (copy glue becomes inc/dec glue;
    regions are dropped or kept only for scratch), `crates/ignite-rt` (allocator, `spawn`,
    `Store`).
  - The ownership rule of slice.md §6, and docs/llvm.md §4.
  - Design §8.5 and §14.2 "Implementation memory model".
- **Cost.** Large for the LLVM target, since it replaces the arena and region design.
  Medium for the Rust target.
- **Conflict.** None with a stated decision: RC is the design's own candidate (Design §8.5),
  and the memory model is open until M5 (Design §14.2). It does replace
  docs/llvm.md §4, a recent slice contract.
- **Recommended path [I].** First run the measurement Design §14.2 asks for, on the HTTP todo
  backend and the Telnet decoder: bytes copied per request. Then prototype unique-owner
  in-place update for `Text`, `Bytes` and `List` buffers only, the large payloads, before
  committing to RC for every aggregate.

### 3.7 Smaller items worth noting [I]

- **`#print axioms`-style trust reporting** ([L-trust]). Once Ignite has check-time evaluation
  (3.1, 3.2), report per test whether it was decided by enumeration, sampled, or skipped.
  This matches Design §1.5's "zero silent constraint relaxation". Small;
  `crates/ignite`.
- **External re-checking** (`lake check`, comparator, independent kernels: [L-road];
  [L-trust]). This mirrors Design §12.5's advice to use "another implementation path"
  against common-mode failures, and supports keeping the Rust and LLVM targets
  as mutual checks (slice.md §1 "Pipeline"). No change; a confirmation of the design.

## 4 What Ignite should not take

- **Dependent types and the Calculus of Constructions.** Ignite aims at a small
  implementation subset with "generics with a limited constraint system" (Design §3.5)
  and a 30-50 reserved-word budget (Design §1.5). [I] Dependent types
  would make type checking, error messages and the M6 self-hosted frontend far larger. The
  proof value can be had by exporting to Lean (section 5) instead of becoming Lean.
- **User-extensible syntax, macros, elaborators and tactics.** They conflict with Design §3.5
  (macros and user-defined syntax deferred) and §9.2 (compile plugins
  deferred). [I] They also work against Design §6.1's provenance requirement ("every
  generated node preserves the source declaration") and against package authors
  extending Ignite through ordinary packages (Design §1.2; §9.1).
- **Monads as the effect system.** Lean's `IO` says that an action has effects, not which
  resource it may touch (row 3 [I]). Ignite's per-capability effects with root mapping
  (slice.md §5.1 "Effect sets") carry authority information that Design §9.3 needs and Lean's
  model lacks. Going monadic would also undo the slice 6 decision against effect markers
  (slice.md §10.2 "Deprecated `await`").
- **Full type-class resolution.** Ignite generics deliberately have no bounds (slice.md §11.1
  "Generics").
  [I] When bounds come, a closed set of builtin constraints (`Eq`, `ToText`) fits the
  "limited constraint system" of Design §3.5 better than open instance search, whose
  performance problems Lean needed tabled resolution to fix ([L-paper] abstract).
- **A checked-in generated seed refreshed from the latest compiler** (Lean's stage0,
  [L-boot]). It conflicts with Design §12.1 and §12.5 (retained independent S0),
  and with AGENTS.md, "Conventions" (generated code is never committed).
- **`native_decide`-style shortcuts presented as proof.** Lean itself warns these can produce
  invalid proofs when native and kernel evaluation disagree ([L-trust]). [I] If Ignite adds
  3.2, label it evaluator-checked, never "proved".
- **Arbitrary-precision `Nat`/`Int` as the default integer** (Lean's `Nat` and `Int` are
  `lean_object *` with small-scalar boxing, [L-ffi]). Design §3.3 and §6.3 want specified
  overflow and a deterministic representation; Ignite `Int` is `i64` (slice.md §6 "Rust lowering", type table;
  docs/llvm.md §3 "Representation"). [I] Boxed bignums would cost the controller determinism nothing, but
  would cost every arithmetic operation a branch and the LLVM layout its simplicity.

## 5 Possible integration

Three options, from most to least feasible. All are [I]: no Ignite-to-Lean tool exists, and
nothing here was run.

### 5.1 Lean as an optional proof layer for pure Ignite code (feasible, large)

- **Mechanism.** A `ignite export-lean` pass over the typed program (`ignite-types`
  `TypedProgram`) translates pure functions to Lean definitions:
  - records → `structure`;
  - enums → `inductive`;
  - `match` → `match`;
  - `List` → `List` or `Array`;
  - `Int` → a 64-bit integer type, to keep Ignite's `i64` semantics;
  - `var` and loops → `do` blocks in `Id` ([L-do]);
  - functions that are not total under 3.4 → `partial` or `partial_fixpoint` ([L-rec]).

  Users write theorems in a sibling `.lean` file and check them with `lake build`.
- **Precedent.** Aeneas translates Rust "into a purely functional representation" in Lean
  and is used to verify SymCrypt (https://lean-lang.org/use-cases/aeneas).
- **Why Ignite is easier.** Ignite is already value-semantic with no borrows (slice.md §6, "Ownership rule"),
  so the translation is closer to identity than Rust's.
- **Targets with a payoff.**
  - The Telnet decoder's state machine (`std/protocol/telnet.ignite`).
  - JSON encode/parse round-trip (`std/json.ignite`).
  - The terminal's `parse`/`respond` (examples/terminal/terminal.ignite, fns `parse`,
    `respond`).
  - Later, contract accounting functions (Design §4.1).
- **Limits.**
  - Effectful functions are out of scope at first: model only their pure cores.
  - Proofs are about the exported model, not the binary. The Ignite compiler, both backends
    and `ignite-io` stay trusted, as Aeneas's translation is.
- **Design change.** Add a "Proof" row to Design §13.1, scoped to the pure subset. Design
  §1.4's "does not infer application correctness" stays true, since proofs are opt-in.

### 5.2 Lean proofs about Controller IR reducers and policies (feasible later, large)

- **Mechanism.** Controller IR is to be canonical JSON with a public schema, holding states,
  guards, effect intents and recovery (Design §6.2). An exporter turns a
  machine into a Lean `inductive` state and step function.
- **Properties to prove.**
  - No dispatch without a persisted intent (Design §7.1).
  - Uncertain outcomes never lead to a blind duplicate create (Design §7.1).
  - The arbiter never lets two policies own the same transition (Design §7.2).
  - Missing telemetry never promotes a canary (Design §4.2; §7.4).
- **Fit.** Finite-state guards over enums are where kernel `decide` works best
  (https://lean-lang.org/doc/reference/latest/Tactic-Proofs/Tactic-Reference/).
- **Blocker.** Controller IR does not exist yet (M1, Design §12.2).

### 5.3 Lean as the implementation language of the Ignite compiler (not feasible as a goal)

It would give a verified-friendly compiler host. But it contradicts compiler self-hosting in
Ignite (Design §12.1, §12.6; PRD11) and the seed decision in favour of Rust (slice.md, opening "Direction" paragraph).
Reject.

### Verdict

Feasible and worth a spike, but not now:

- **5.1** fits once slice 8 and the pure evaluator exist. It is an out-of-band, opt-in
  layer and not part of `task gate`.
- **5.2** follows Controller IR (M1).
- **5.3** is rejected.

The integration adds the Lean toolchain (elan, Lake: [L-lake]) as an external tool, the same
way clang is one for the LLVM target (docs/llvm.md, opening "Direction" paragraph). It does not change the Ignite
language or its trusted runtime. The prerequisite that pays off without Lean anyway is
inferred totality (3.4) plus check-time evaluation (3.1).
