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

Current values: run `v0.15.0` on 2026-11-08, commit `6e144f76`.

## 1 Semantics: one meaning across the three consumers

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 1.1 | programs whose result differs between Rust, LLVM and the evaluator | gate | eval | 4 | 4 | = |
| 1.1 | programs whose result differs between Rust, LLVM and the evaluator | gate | llvm | 0 | 0 | = |
| 1.2 | differential corpus size | ratchet | eval | 465 | 452 | +29 better |
| 1.2 | differential corpus size | ratchet | llvm | 701 | 688 | +45 better |
| 1.2 | differential corpus size | ratchet | rust | 701 | 688 | +45 better |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | eval/builtins | 112 | 133 | +13 worse |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | eval/patterns | 13 | 13 | = |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | llvm/builtins | 12 | 14 | +1 worse |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | llvm/patterns | 30 | 30 | = |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | rust/builtins | 43 | 54 | +10 worse |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | rust/patterns | 71 | 83 | +12 worse |
| 1.4 | generated-code goldens changed without a `CODEGEN_REVISION` bump | gate | all | 0 | 0 | = |
| 1.5 | random programs on which the consumers disagree | gate | | unmeasured: no program generator yet (out of scope of story:quality-ratchet) | | |
| 1.6 | corpus units the evaluator cannot run | ratchet | eval | 236 | 236 | +16 worse |
| 1.7 | known differences marked intermittent | gate | all | 1 | 1 | = |

## 2 Front end: the checker refuses exactly the wrong programs

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 2.1 | diagnostic codes without a test | gate | all | 11 | 11 | = |
| 2.2 | accepted programs that fail in a backend | gate | llvm | 0 | 0 | = |
| 2.2 | accepted programs that fail in a backend | gate | rust | 0 | 0 | = |
| 2.3 | soundness holes open (effects, ownership, exhaustiveness) | ratchet | | unmeasured: the planning store tags no story as a soundness hole; known open: story:map-put-handle-roots, story:immutable-binding-e006 | | |
| 2.4 | compiler panics on any input | gate | | unmeasured: no fuzzer yet (out of scope of story:quality-ratchet) | | |
| 2.5 | diagnostic fixtures (one diagnostic per error site, its span) | ratchet | all | 329 | 329 | +6 better |
| 2.6 | checking time stays linear (limit tests) | gate | all | 5 | 5 | = |

## 3 Generated code and runtime behaviour

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 3.1 | copies in the generated code of the golden corpus | ratchet | llvm | 90 | 90 | = |
| 3.1 | copies in the generated code of the golden corpus | ratchet | rust | 166 | 166 | −2 better |
| 3.2 | binary size | ratchet | llvm/terminald/dev | 7563720 B | 7831016 B | +50976 |
| 3.2 | binary size | ratchet | llvm/terminald/release | 1468648 B | 1512792 B | +7744 |
| 3.2 | binary size | ratchet | llvm/todod/dev | 8230560 B | 8531472 B | +78656 |
| 3.2 | binary size | ratchet | llvm/todod/release | 2042752 B | 2107792 B | +19168 |
| 3.2 | binary size | ratchet | rust/terminald/dev | 6223520 B | 6429272 B | +39184 |
| 3.2 | binary size | ratchet | rust/terminald/release | 681368 B | 706440 B | +10960 |
| 3.2 | binary size | ratchet | rust/todod/dev | 7405864 B | 7710792 B | +136784 |
| 3.2 | binary size | ratchet | rust/todod/release | 1011216 B | 1011216 B | +31080 |
| 3.3 | throughput against plain Rust | ratchet | llvm/terminal/connect-8 | 0.929× | 0.929× | ec2:c7i.metal-24xl, +163 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/terminal/rr-1 | 0.975× | 0.975× | ec2:c7i.metal-24xl, +69 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/terminal/rr-64 | 0.971× | 0.971× | ec2:c7i.metal-24xl, +113 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/todo/get-1 | 1.015× | 1.015× | ec2:c7i.metal-24xl, +116 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/todo/get-64 | 1.069× | 1.069× | ec2:c7i.metal-24xl, +200 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/todo/patch-16 | 1.012× | 1.012× | ec2:c7i.metal-24xl, +436 better |
| 3.3 | throughput against plain Rust | ratchet | rust/terminal/connect-8 | 0.956× | 0.956× | ec2:c7i.metal-24xl, +10 better |
| 3.3 | throughput against plain Rust | ratchet | rust/terminal/rr-1 | 0.962× | 0.962× | ec2:c7i.metal-24xl, +76 better |
| 3.3 | throughput against plain Rust | ratchet | rust/terminal/rr-64 | 0.956× | 0.956× | ec2:c7i.metal-24xl, +132 better |
| 3.3 | throughput against plain Rust | ratchet | rust/todo/get-1 | 0.975× | 0.975× | ec2:c7i.metal-24xl, +166 better |
| 3.3 | throughput against plain Rust | ratchet | rust/todo/get-64 | 0.981× | 0.981× | ec2:c7i.metal-24xl, +278 better |
| 3.3 | throughput against plain Rust | ratchet | rust/todo/patch-16 | 0.994× | 0.994× | ec2:c7i.metal-24xl, +427 better |
| 3.3 | throughput against plain Rust | ratchet | go/terminal/connect-8 | reference, no bar | 0.730× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/terminal/rr-1 | reference, no bar | 0.904× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/terminal/rr-64 | reference, no bar | 1.014× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/todo/get-1 | reference, no bar | 0.728× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/todo/get-64 | reference, no bar | 0.680× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/todo/patch-16 | reference, no bar | 0.908× | ec2:c7i.metal-24xl |
| 3.4 | peak RSS, leaked handles and threads after a load run | gate | | unmeasured: no number: the named tests gate it in task gate | | |
| 3.5 | runtime entries that can unwind a panic across `extern "C"` | gate | llvm | 0 | 0 | = |
| 3.6 | `unsafe` sites in the substrate a target links | ratchet | llvm | 82 | 82 | +6 worse |
| 3.6 | `unsafe` sites in the substrate a target links | ratchet | rust | 12 | 12 | +3 worse |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/terminal/connect-8 | 1.105× | 1.105× | ec2:c7i.metal-24xl, −27 better |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/terminal/rr-1 | 1.000× | 1.000× | ec2:c7i.metal-24xl, −17 better |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/terminal/rr-64 | 1.032× | 1.032× | ec2:c7i.metal-24xl, −208 better |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/todo/get-1 | 0.992× | 0.992× | ec2:c7i.metal-24xl, −107 better |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/todo/get-64 | 1.103× | 1.103× | ec2:c7i.metal-24xl, −162 better |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/todo/patch-16 | 1.445× | 1.445× | ec2:c7i.metal-24xl, −748 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/terminal/connect-8 | 1.055× | 1.055× | ec2:c7i.metal-24xl, −3 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/terminal/rr-1 | 1.004× | 1.004× | ec2:c7i.metal-24xl, −4 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/terminal/rr-64 | 1.082× | 1.082× | ec2:c7i.metal-24xl, −161 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/todo/get-1 | 1.044× | 1.044× | ec2:c7i.metal-24xl, −158 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/todo/get-64 | 1.252× | 1.252× | ec2:c7i.metal-24xl, −299 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/todo/patch-16 | 1.543× | 1.543× | ec2:c7i.metal-24xl, −732 better |
| 3.7 | p99 latency against plain Rust | ratchet | go/terminal/connect-8 | reference, no bar | 2.073× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/terminal/rr-1 | reference, no bar | 1.053× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/terminal/rr-64 | reference, no bar | 1.374× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/todo/get-1 | reference, no bar | 1.564× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/todo/get-64 | reference, no bar | 3.549× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/todo/patch-16 | reference, no bar | 1.955× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/terminal/connect-8 | 2.938× | 2.938× | ec2:c7i.metal-24xl, +293 worse |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/terminal/rr-1 | 1.203× | 1.203× | ec2:c7i.metal-24xl, −18 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/terminal/rr-64 | 1.434× | 1.434× | ec2:c7i.metal-24xl, −854 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/todo/get-1 | 0.819× | 0.819× | ec2:c7i.metal-24xl, −20 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/todo/get-64 | 1.258× | 1.258× | ec2:c7i.metal-24xl, −1599 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/todo/patch-16 | 0.913× | 0.913× | ec2:c7i.metal-24xl, −555 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/terminal/connect-8 | 0.981× | 0.981× | ec2:c7i.metal-24xl, −2 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/terminal/rr-1 | 1.063× | 1.063× | ec2:c7i.metal-24xl, −2 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/terminal/rr-64 | 1.036× | 1.043× | ec2:c7i.metal-24xl, +7 |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/todo/get-1 | 0.765× | 0.778× | ec2:c7i.metal-24xl, +13 |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/todo/get-64 | 2.612× | 2.612× | ec2:c7i.metal-24xl, −42 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/todo/patch-16 | 1.158× | 1.225× | ec2:c7i.metal-24xl, +67 |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/terminal/connect-8 | reference, no bar | 3.750× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/terminal/rr-1 | reference, no bar | 3.775× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/terminal/rr-64 | reference, no bar | 2.804× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/todo/get-1 | reference, no bar | 3.572× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/todo/get-64 | reference, no bar | 3.214× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/todo/patch-16 | reference, no bar | 3.417× | ec2:c7i.metal-24xl |
| 3.9 | stripped binary size against plain Rust | ratchet | llvm/terminal | 1.378× | 1.406× | ec2:c7i.metal-24xl, +28 |
| 3.9 | stripped binary size against plain Rust | ratchet | llvm/todo | 0.700× | 0.720× | ec2:c7i.metal-24xl, +20 |
| 3.9 | stripped binary size against plain Rust | ratchet | rust/terminal | 1.263× | 1.304× | ec2:c7i.metal-24xl, +41 |
| 3.9 | stripped binary size against plain Rust | ratchet | rust/todo | 0.692× | 0.723× | ec2:c7i.metal-24xl, +31 |
| 3.9 | stripped binary size against plain Rust | ratchet | go/terminal | reference, no bar | 4.998× | ec2:c7i.metal-24xl |
| 3.9 | stripped binary size against plain Rust | ratchet | go/todo | reference, no bar | 4.852× | ec2:c7i.metal-24xl |

## 4 Standard library and protocols

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 4.1 | conformance claims that hold | ratchet | llvm | 3 | 3 | = |
| 4.1 | conformance claims that hold | ratchet | rust | 3 | 3 | = |
| 4.2 | gaps per claimed spec | ratchet | llvm/jsonrpc-2.0 | 0 | 0 | = |
| 4.2 | gaps per claimed spec | ratchet | llvm/rfc854 | 0 | 0 | = |
| 4.2 | gaps per claimed spec | ratchet | llvm/rfc855 | 0 | 0 | = |
| 4.2 | gaps per claimed spec | ratchet | llvm/rfc9110 | 2 | 2 | = |
| 4.2 | gaps per claimed spec | ratchet | llvm/rfc9112 | 1 | 1 | = |
| 4.2 | gaps per claimed spec | ratchet | rust/jsonrpc-2.0 | 0 | 0 | = |
| 4.2 | gaps per claimed spec | ratchet | rust/rfc854 | 0 | 0 | = |
| 4.2 | gaps per claimed spec | ratchet | rust/rfc855 | 0 | 0 | = |
| 4.2 | gaps per claimed spec | ratchet | rust/rfc9110 | 2 | 2 | = |
| 4.2 | gaps per claimed spec | ratchet | rust/rfc9112 | 1 | 1 | = |
| 4.3 | inline tests per target | ratchet | eval | 209 | 209 | +18 better |
| 4.3 | inline tests per target | ratchet | llvm | 350 | 350 | +28 better |
| 4.3 | inline tests per target | ratchet | rust | 350 | 350 | +28 better |
| 4.4 | std functions with no inline test | ratchet | | unmeasured: no instrument yet: needs the call graph of the test bodies | | |

## 5 Operations: run, apply, host

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 5.1 | replay mismatches (E127) on recorded journals | gate | | unmeasured: no number: the named suites gate it in task gate | | |
| 5.2 | crash-recovery matrix rows failing (docs/slice.md 17.7) | gate | | unmeasured: no number: the named suites gate it in task gate | | |
| 5.3 | restart readiness, real clock | ratchet | llvm | 384 ms | 106 ms | ec2:c7i.metal-24xl, +1 |
| 5.4 | apply cycle time of the todo, per target | ratchet | | unmeasured: not in task quality: `ignite-perf apply-cycle` exports and builds the benchmark trees and runs about 15 minutes; it is measured per slice (docs/performance/) | | |

## 6 Compiler speed

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 6.1 | check and plan of the design file | ratchet | all/check | 176 ms | 200 ms | ec2:c7i.metal-24xl, +15 |
| 6.1 | check and plan of the design file | ratchet | all/plan | 153 ms | 201 ms | ec2:c7i.metal-24xl, +15 |
| 6.2 | todo build, dev profile | ratchet | llvm | 993 ms | 1298 ms | ec2:c7i.metal-24xl, +117 |
| 6.2 | todo build, dev profile | ratchet | rust | 3081 ms | 4120 ms | ec2:c7i.metal-24xl, +215 |
| 6.3 | `task gate` wall time | ratchet | | unmeasured: no instrument records the gate's wall time; waves note it in their close (AGENTS.md § Gate time) | | |

## 7 Engineering health

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 7.1 | flaky tests per gate | ratchet | | unmeasured: no instrument reads the gate logs; known: apply_transition::a_failed_start_first_replacement, slice10::executable_component_main | | |
| 7.2 | ignored tests | ratchet | all | 24 | 24 | +1 worse |
| 7.3 | source files over 3,000 lines | ratchet | all | 4 | 4 | = |
| 7.4 | language friction rows open | ratchet | all | 38 | 38 | −5 better |
| 7.5 | mutants that survive the suite on the compiler crates | ratchet | | unmeasured: no mutation testing yet (out of scope of story:quality-ratchet) | | |

## 8 Documentation and users

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 8.1 | `ignite` blocks in the book that fail to check | gate | | unmeasured: no number: `site::` gates it in task gate | | |
| 8.2 | website UI ratchet | ratchet | | unmeasured: its own baseline: ux/baseline.json | | |
| 8.3 | public APIs or diagnostics changed without a docs/slice.md change | gate | | unmeasured: a review rule; no test | | |
