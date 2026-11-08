# Quality

<!-- Generated from quality/baseline.json and quality/history.jsonl by `task quality`; site::quality_page_is_generated checks it. -->

Ignite keeps a written set of quality metrics. Today's values are the bar: a merge may not make a
gated metric worse, a release may not be worse than the release before it, and an improvement is
written into the bar in the same change, so the bar only moves one way. The list, its kinds and its
instruments are in [`docs/quality.md`](https://github.com/ignite-lang/ignite-lang/blob/main/docs/quality.md);
the bars are in `quality/baseline.json` and every measured value in `quality/history.jsonl`.

- **gate**: a count of failures; the bar is never above today, usually 0.
- **ratchet**: a value with a direction and a tolerance; it may not get worse than the bar.
- **hill-climb**: tracked and aimed at a target; not refused when it moves.

Targets: `rust` is the Rust target, `llvm` the LLVM target, `eval` the compile-time evaluator,
`all` a value that does not differ by target. The first of these metrics, one result across the
three, is checked on every merge by running one corpus of programs on all three.

Timing rows (build, check and restart times) are measured on a single-tenant EC2 host made for the
run (`ec2:<type>`, its trend against the run before on that host type); until such a run exists
they show the shared workstation's values (`host: local`), recorded but not comparable.

Throughput, p99 latency, peak RSS and stripped binary size of the todo service and the terminal
server (3.3, 3.7 to 3.9) are ratios against a plain-Rust implementation of the same program,
measured on that host in the same run (`bench/native/`); a Go implementation is shown as a
reference without a bar.

Current values: run `bar` on 2026-11-07, commit `9a775116`.

## 1 Semantics: one meaning across the three consumers

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 1.1 | programs whose result differs between Rust, LLVM and the evaluator | gate | eval | 4 | 4 |  |
| 1.1 | programs whose result differs between Rust, LLVM and the evaluator | gate | llvm | 0 | 0 |  |
| 1.2 | differential corpus size | ratchet | eval | 420 | 420 |  |
| 1.2 | differential corpus size | ratchet | llvm | 614 | 614 |  |
| 1.2 | differential corpus size | ratchet | rust | 614 | 614 |  |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | eval/builtins | 112 | 112 |  |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | eval/patterns | 13 | 13 |  |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | llvm/builtins | 12 | 12 |  |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | llvm/patterns | 30 | 30 |  |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | rust/builtins | 43 | 43 |  |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | rust/patterns | 71 | 71 |  |
| 1.4 | generated-code goldens changed without a `CODEGEN_REVISION` bump | gate | all | 0 | 0 |  |
| 1.5 | random programs on which the consumers disagree | gate | | unmeasured: no program generator yet (out of scope of story:quality-ratchet) | | |
| 1.6 | corpus units the evaluator cannot run | ratchet | eval | 194 | 194 |  |
| 1.7 | known differences marked intermittent | gate | all | 1 | 1 |  |

## 2 Front end: the checker refuses exactly the wrong programs

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 2.1 | diagnostic codes without a test | gate | all | 11 | 11 |  |
| 2.2 | accepted programs that fail in a backend | gate | llvm | 0 | 0 |  |
| 2.2 | accepted programs that fail in a backend | gate | rust | 0 | 0 |  |
| 2.3 | soundness holes open (effects, ownership, exhaustiveness) | ratchet | | unmeasured: the planning store tags no story as a soundness hole; known open: story:map-put-handle-roots, story:immutable-binding-e006 | | |
| 2.4 | compiler panics on any input | gate | | unmeasured: no fuzzer yet (out of scope of story:quality-ratchet) | | |
| 2.5 | diagnostic fixtures (one diagnostic per error site, its span) | ratchet | all | 319 | 319 |  |
| 2.6 | checking time stays linear (limit tests) | gate | all | 5 | 5 |  |

## 3 Generated code and runtime behaviour

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 3.1 | copies in the generated code of the golden corpus | ratchet | llvm | 90 | 90 |  |
| 3.1 | copies in the generated code of the golden corpus | ratchet | rust | 168 | 168 |  |
| 3.2 | binary size | ratchet | llvm/terminald/dev | 7563720 B | 7563720 B |  |
| 3.2 | binary size | ratchet | llvm/terminald/release | 1468648 B | 1468648 B |  |
| 3.2 | binary size | ratchet | llvm/todod/dev | 8230560 B | 8230560 B |  |
| 3.2 | binary size | ratchet | llvm/todod/release | 2042752 B | 2042752 B |  |
| 3.2 | binary size | ratchet | rust/terminald/dev | 6223520 B | 6223520 B |  |
| 3.2 | binary size | ratchet | rust/terminald/release | 681368 B | 681368 B |  |
| 3.2 | binary size | ratchet | rust/todod/dev | 7405864 B | 7405864 B |  |
| 3.2 | binary size | ratchet | rust/todod/release | 961880 B | 961880 B |  |
| 3.3 | throughput against plain Rust | ratchet | llvm/terminal/connect-8 | 2.343× | 2.343× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | llvm/terminal/rr-1 | 0.900× | 0.900× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | llvm/terminal/rr-64 | 0.848× | 0.848× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | llvm/todo/get-1 | 0.905× | 0.905× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | llvm/todo/get-64 | 0.879× | 0.879× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | llvm/todo/patch-16 | 0.578× | 0.578× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | rust/terminal/connect-8 | 2.851× | 2.851× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | rust/terminal/rr-1 | 0.874× | 0.874× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | rust/terminal/rr-64 | 0.821× | 0.821× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | rust/todo/get-1 | 0.786× | 0.786× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | rust/todo/get-64 | 0.694× | 0.694× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | rust/todo/patch-16 | 0.560× | 0.560× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/terminal/connect-8 | reference, no bar | 2.092× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/terminal/rr-1 | reference, no bar | 0.890× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/terminal/rr-64 | reference, no bar | 0.965× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/todo/get-1 | reference, no bar | 0.732× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/todo/get-64 | reference, no bar | 0.686× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/todo/patch-16 | reference, no bar | 0.897× | ec2:c7i.metal-24xl |
| 3.4 | peak RSS, leaked handles and threads after a load run | gate | | unmeasured: no number: the named tests gate it in task gate | | |
| 3.5 | runtime entries that can unwind a panic across `extern "C"` | gate | llvm | 0 | 0 |  |
| 3.6 | `unsafe` sites in the substrate a target links | ratchet | llvm | 73 | 73 |  |
| 3.6 | `unsafe` sites in the substrate a target links | ratchet | rust | 7 | 7 |  |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/terminal/connect-8 | 0.193× | 0.193× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/terminal/rr-1 | 1.017× | 1.017× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/terminal/rr-64 | 1.495× | 1.495× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/todo/get-1 | 1.081× | 1.081× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/todo/get-64 | 1.287× | 1.287× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/todo/patch-16 | 2.170× | 2.170× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | rust/terminal/connect-8 | 0.183× | 0.183× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | rust/terminal/rr-1 | 1.029× | 1.029× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | rust/terminal/rr-64 | 1.382× | 1.382× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | rust/todo/get-1 | 1.234× | 1.234× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | rust/todo/get-64 | 1.644× | 1.644× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | rust/todo/patch-16 | 2.313× | 2.313× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/terminal/connect-8 | reference, no bar | 0.379× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/terminal/rr-1 | reference, no bar | 1.059× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/terminal/rr-64 | reference, no bar | 1.452× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/todo/get-1 | reference, no bar | 1.589× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/todo/get-64 | reference, no bar | 3.538× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/todo/patch-16 | reference, no bar | 1.970× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/terminal/connect-8 | 2.623× | 2.623× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/terminal/rr-1 | 1.220× | 1.220× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/terminal/rr-64 | 2.295× | 2.295× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/todo/get-1 | 0.833× | 0.833× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/todo/get-64 | 2.837× | 2.837× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/todo/patch-16 | 1.516× | 1.516× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/terminal/connect-8 | 0.973× | 0.973× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/terminal/rr-1 | 1.067× | 1.067× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/terminal/rr-64 | 1.035× | 1.035× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/todo/get-1 | 0.753× | 0.753× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/todo/get-64 | 2.652× | 2.652× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/todo/patch-16 | 1.185× | 1.185× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/terminal/connect-8 | reference, no bar | 3.799× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/terminal/rr-1 | reference, no bar | 3.955× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/terminal/rr-64 | reference, no bar | 2.778× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/todo/get-1 | reference, no bar | 3.425× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/todo/get-64 | reference, no bar | 3.081× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/todo/patch-16 | reference, no bar | 3.545× | ec2:c7i.metal-24xl |
| 3.9 | stripped binary size against plain Rust | ratchet | llvm/terminal | 1.423× | 1.423× | ec2:c7i.metal-24xl |
| 3.9 | stripped binary size against plain Rust | ratchet | llvm/todo | 0.700× | 0.700× | ec2:c7i.metal-24xl |
| 3.9 | stripped binary size against plain Rust | ratchet | rust/terminal | 1.304× | 1.304× | ec2:c7i.metal-24xl |
| 3.9 | stripped binary size against plain Rust | ratchet | rust/todo | 0.692× | 0.692× | ec2:c7i.metal-24xl |
| 3.9 | stripped binary size against plain Rust | ratchet | go/terminal | reference, no bar | 5.172× | ec2:c7i.metal-24xl |
| 3.9 | stripped binary size against plain Rust | ratchet | go/todo | reference, no bar | 4.852× | ec2:c7i.metal-24xl |

## 4 Standard library and protocols

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 4.1 | conformance claims that hold | ratchet | llvm | 3 | 3 |  |
| 4.1 | conformance claims that hold | ratchet | rust | 3 | 3 |  |
| 4.2 | gaps per claimed spec | ratchet | llvm/jsonrpc-2.0 | 0 | 0 |  |
| 4.2 | gaps per claimed spec | ratchet | llvm/rfc854 | 0 | 0 |  |
| 4.2 | gaps per claimed spec | ratchet | llvm/rfc855 | 0 | 0 |  |
| 4.2 | gaps per claimed spec | ratchet | llvm/rfc9110 | 1 | 1 |  |
| 4.2 | gaps per claimed spec | ratchet | llvm/rfc9112 | 1 | 1 |  |
| 4.2 | gaps per claimed spec | ratchet | rust/jsonrpc-2.0 | 0 | 0 |  |
| 4.2 | gaps per claimed spec | ratchet | rust/rfc854 | 0 | 0 |  |
| 4.2 | gaps per claimed spec | ratchet | rust/rfc855 | 0 | 0 |  |
| 4.2 | gaps per claimed spec | ratchet | rust/rfc9110 | 1 | 1 |  |
| 4.2 | gaps per claimed spec | ratchet | rust/rfc9112 | 1 | 1 |  |
| 4.3 | inline tests per target | ratchet | eval | 191 | 191 |  |
| 4.3 | inline tests per target | ratchet | llvm | 309 | 309 |  |
| 4.3 | inline tests per target | ratchet | rust | 309 | 309 |  |
| 4.4 | std functions with no inline test | ratchet | | unmeasured: no instrument yet: needs the call graph of the test bodies | | |

## 5 Operations: run, apply, host

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 5.1 | replay mismatches (E127) on recorded journals | gate | | unmeasured: no number: the named suites gate it in task gate | | |
| 5.2 | crash-recovery matrix rows failing (docs/slice.md 17.7) | gate | | unmeasured: no number: the named suites gate it in task gate | | |
| 5.3 | restart readiness, real clock | ratchet | llvm | 384 ms | 105 ms | ec2:c7i.metal-24xl, = |
| 5.4 | apply cycle time of the todo, per target | ratchet | | unmeasured: not in task quality: `ignite-perf apply-cycle` exports and builds the benchmark trees and runs about 15 minutes; it is measured per slice (docs/performance/) | | |

## 6 Compiler speed

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 6.1 | check and plan of the design file | ratchet | all/check | 176 ms | 185 ms | ec2:c7i.metal-24xl, = |
| 6.1 | check and plan of the design file | ratchet | all/plan | 153 ms | 185 ms | ec2:c7i.metal-24xl, −1 better |
| 6.2 | todo build, dev profile | ratchet | llvm | 993 ms | 1183 ms | ec2:c7i.metal-24xl, = |
| 6.2 | todo build, dev profile | ratchet | rust | 3081 ms | 3907 ms | ec2:c7i.metal-24xl, +10 |
| 6.3 | `task gate` wall time | ratchet | | unmeasured: no instrument records the gate's wall time; waves note it in their close (AGENTS.md § Gate time) | | |

## 7 Engineering health

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 7.1 | flaky tests per gate | ratchet | | unmeasured: no instrument reads the gate logs; known: apply_transition::a_failed_start_first_replacement, slice10::executable_component_main | | |
| 7.2 | ignored tests | ratchet | all | 21 | 21 |  |
| 7.3 | source files over 3,000 lines | ratchet | all | 4 | 4 |  |
| 7.4 | language friction rows open | ratchet | all | 34 | 34 |  |
| 7.5 | mutants that survive the suite on the compiler crates | ratchet | | unmeasured: no mutation testing yet (out of scope of story:quality-ratchet) | | |

## 8 Documentation and users

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 8.1 | `ignite` blocks in the book that fail to check | gate | | unmeasured: no number: `site::` gates it in task gate | | |
| 8.2 | website UI ratchet | ratchet | | unmeasured: its own baseline: ux/baseline.json | | |
| 8.3 | public APIs or diagnostics changed without a docs/slice.md change | gate | | unmeasured: a review rule; no test | | |
