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

Current values: run `v0.16.0` on 2026-11-09, commit `1293ce62`.

## 1 Semantics: one meaning across the three consumers

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 1.1 | programs whose result differs between Rust, LLVM and the evaluator | gate | eval | 4 | 4 | = |
| 1.1 | programs whose result differs between Rust, LLVM and the evaluator | gate | llvm | 4 | 4 | +4 worse |
| 1.2 | differential corpus size | ratchet | eval | 475 | 475 | +23 better |
| 1.2 | differential corpus size | ratchet | llvm | 762 | 762 | +74 better |
| 1.2 | differential corpus size | ratchet | rust | 762 | 762 | +74 better |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | eval/builtins | 112 | 146 | +13 worse |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | eval/patterns | 13 | 13 | = |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | llvm/builtins | 12 | 13 | −1 better |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | llvm/patterns | 30 | 46 | +16 worse |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | rust/builtins | 43 | 55 | +1 worse |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | rust/patterns | 71 | 83 | = |
| 1.4 | generated-code goldens changed without a `CODEGEN_REVISION` bump | gate | all | 0 | 0 | = |
| 1.5 | random programs on which the consumers disagree | gate | | unmeasured: no program generator yet (out of scope of story:quality-ratchet) | | |
| 1.6 | corpus units the evaluator cannot run | ratchet | eval | 287 | 287 | +51 worse |
| 1.7 | known differences marked intermittent | gate | all | 1 | 1 | = |

## 2 Front end: the checker refuses exactly the wrong programs

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 2.1 | diagnostic codes without a test | gate | all | 11 | 11 | = |
| 2.2 | accepted programs that fail in a backend | gate | llvm | 0 | 0 | = |
| 2.2 | accepted programs that fail in a backend | gate | rust | 0 | 0 | = |
| 2.3 | soundness holes open (effects, ownership, exhaustiveness) | ratchet | | unmeasured: the planning store tags no story as a soundness hole; known open: story:map-put-handle-roots, story:immutable-binding-e006 | | |
| 2.4 | compiler panics on any input | gate | | unmeasured: no fuzzer yet (out of scope of story:quality-ratchet) | | |
| 2.5 | diagnostic fixtures (one diagnostic per error site, its span) | ratchet | all | 336 | 336 | +7 better |
| 2.6 | checking time stays linear (limit tests) | gate | all | 5 | 5 | = |

## 3 Generated code and runtime behaviour

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 3.1 | copies in the generated code of the golden corpus | ratchet | llvm | 90 | 90 | = |
| 3.1 | copies in the generated code of the golden corpus | ratchet | rust | 166 | 166 | = |
| 3.2 | binary size | ratchet | llvm/terminald/dev | 9144568 B | 9144568 B | +1313552 worse |
| 3.2 | binary size | ratchet | llvm/terminald/release | 2242824 B | 2242824 B | +730032 worse |
| 3.2 | binary size | ratchet | llvm/todod/dev | 9849080 B | 9849080 B | +1317608 worse |
| 3.2 | binary size | ratchet | llvm/todod/release | 2837352 B | 2837352 B | +729560 worse |
| 3.2 | binary size | ratchet | rust/terminald/dev | 7620632 B | 7620632 B | +1191360 worse |
| 3.2 | binary size | ratchet | rust/terminald/release | 911336 B | 911336 B | +204896 worse |
| 3.2 | binary size | ratchet | rust/todod/dev | 8924288 B | 8924288 B | +1213496 worse |
| 3.2 | binary size | ratchet | rust/todod/release | 1219336 B | 1219336 B | +208120 worse |
| 3.3 | throughput against plain Rust | ratchet | llvm/terminal/connect-8 | 0.929× | 0.876× | ec2:c7i.metal-24xl, −53 |
| 3.3 | throughput against plain Rust | ratchet | llvm/terminal/rr-1 | 0.987× | 0.987× | ec2:c7i.metal-24xl, +12 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/terminal/rr-64 | 0.859× | 0.859× | ec2:c7i.metal-24xl, −112 worse |
| 3.3 | throughput against plain Rust | ratchet | llvm/todo/get-1 | 1.033× | 1.033× | ec2:c7i.metal-24xl, +18 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/todo/get-64 | 1.210× | 1.210× | ec2:c7i.metal-24xl, +141 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/todo/patch-16 | 1.193× | 1.193× | ec2:c7i.metal-24xl, +181 better |
| 3.3 | throughput against plain Rust | ratchet | rust/terminal/connect-8 | 0.956× | 0.892× | ec2:c7i.metal-24xl, −64 |
| 3.3 | throughput against plain Rust | ratchet | rust/terminal/rr-1 | 0.971× | 0.971× | ec2:c7i.metal-24xl, +9 better |
| 3.3 | throughput against plain Rust | ratchet | rust/terminal/rr-64 | 1.024× | 1.024× | ec2:c7i.metal-24xl, +68 better |
| 3.3 | throughput against plain Rust | ratchet | rust/todo/get-1 | 0.975× | 0.959× | ec2:c7i.metal-24xl, −16 |
| 3.3 | throughput against plain Rust | ratchet | rust/todo/get-64 | 0.826× | 0.826× | ec2:c7i.metal-24xl, −155 worse |
| 3.3 | throughput against plain Rust | ratchet | rust/todo/patch-16 | 1.185× | 1.185× | ec2:c7i.metal-24xl, +191 better |
| 3.3 | throughput against plain Rust | ratchet | go/terminal/connect-8 | reference, no bar | 0.722× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/terminal/rr-1 | reference, no bar | 0.926× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/terminal/rr-64 | reference, no bar | 0.998× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/todo/get-1 | reference, no bar | 0.737× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/todo/get-64 | reference, no bar | 0.679× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/todo/patch-16 | reference, no bar | 0.911× | ec2:c7i.metal-24xl |
| 3.4 | peak RSS, leaked handles and threads after a load run | gate | | unmeasured: no number: the named tests gate it in task gate | | |
| 3.5 | runtime entries that can unwind a panic across `extern "C"` | gate | llvm | 0 | 0 | = |
| 3.6 | `unsafe` sites in the substrate a target links | ratchet | llvm | 95 | 95 | +13 worse |
| 3.6 | `unsafe` sites in the substrate a target links | ratchet | rust | 25 | 25 | +13 worse |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/terminal/connect-8 | 1.470× | 1.470× | ec2:c7i.metal-24xl, +365 worse |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/terminal/rr-1 | 1.000× | 1.000× | ec2:c7i.metal-24xl, = |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/terminal/rr-64 | 0.719× | 0.719× | ec2:c7i.metal-24xl, −313 better |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/todo/get-1 | 0.984× | 0.984× | ec2:c7i.metal-24xl, −8 better |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/todo/get-64 | 1.500× | 1.500× | ec2:c7i.metal-24xl, +397 worse |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/todo/patch-16 | 0.903× | 0.903× | ec2:c7i.metal-24xl, −542 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/terminal/connect-8 | 1.415× | 1.415× | ec2:c7i.metal-24xl, +360 worse |
| 3.7 | p99 latency against plain Rust | ratchet | rust/terminal/rr-1 | 0.980× | 0.980× | ec2:c7i.metal-24xl, −24 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/terminal/rr-64 | 0.526× | 0.526× | ec2:c7i.metal-24xl, −556 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/todo/get-1 | 1.044× | 1.049× | ec2:c7i.metal-24xl, +5 |
| 3.7 | p99 latency against plain Rust | ratchet | rust/todo/get-64 | 1.054× | 1.054× | ec2:c7i.metal-24xl, −198 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/todo/patch-16 | 1.207× | 1.207× | ec2:c7i.metal-24xl, −336 better |
| 3.7 | p99 latency against plain Rust | ratchet | go/terminal/connect-8 | reference, no bar | 2.094× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/terminal/rr-1 | reference, no bar | 1.033× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/terminal/rr-64 | reference, no bar | 1.326× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/todo/get-1 | reference, no bar | 1.560× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/todo/get-64 | reference, no bar | 3.501× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/todo/patch-16 | reference, no bar | 1.978× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/terminal/connect-8 | 1.137× | 1.137× | ec2:c7i.metal-24xl, −1801 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/terminal/rr-1 | 1.203× | 1.289× | ec2:c7i.metal-24xl, +86 |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/terminal/rr-64 | 1.314× | 1.314× | ec2:c7i.metal-24xl, −120 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/todo/get-1 | 0.819× | 0.882× | ec2:c7i.metal-24xl, +63 |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/todo/get-64 | 1.152× | 1.152× | ec2:c7i.metal-24xl, −106 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/todo/patch-16 | 0.913× | 0.941× | ec2:c7i.metal-24xl, +28 |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/terminal/connect-8 | 0.981× | 1.000× | ec2:c7i.metal-24xl, +19 |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/terminal/rr-1 | 1.063× | 1.148× | ec2:c7i.metal-24xl, +85 |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/terminal/rr-64 | 0.858× | 0.858× | ec2:c7i.metal-24xl, −185 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/todo/get-1 | 0.765× | 0.839× | ec2:c7i.metal-24xl, +61 |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/todo/get-64 | 0.837× | 0.837× | ec2:c7i.metal-24xl, −1775 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/todo/patch-16 | 0.878× | 0.878× | ec2:c7i.metal-24xl, −347 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/terminal/connect-8 | reference, no bar | 3.763× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/terminal/rr-1 | reference, no bar | 3.940× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/terminal/rr-64 | reference, no bar | 2.866× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/todo/get-1 | reference, no bar | 3.542× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/todo/get-64 | reference, no bar | 3.161× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/todo/patch-16 | reference, no bar | 3.488× | ec2:c7i.metal-24xl |
| 3.9 | stripped binary size against plain Rust | ratchet | llvm/terminal | 1.820× | 1.820× | ec2:c7i.metal-24xl, +414 worse |
| 3.9 | stripped binary size against plain Rust | ratchet | llvm/todo | 0.869× | 0.869× | ec2:c7i.metal-24xl, +149 worse |
| 3.9 | stripped binary size against plain Rust | ratchet | rust/terminal | 1.669× | 1.669× | ec2:c7i.metal-24xl, +365 worse |
| 3.9 | stripped binary size against plain Rust | ratchet | rust/todo | 0.864× | 0.864× | ec2:c7i.metal-24xl, +141 worse |
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
| 4.3 | inline tests per target | ratchet | eval | 217 | 217 | +8 better |
| 4.3 | inline tests per target | ratchet | llvm | 375 | 375 | +25 better |
| 4.3 | inline tests per target | ratchet | rust | 375 | 375 | +25 better |
| 4.4 | std functions with no inline test | ratchet | | unmeasured: no instrument yet: needs the call graph of the test bodies | | |

## 5 Operations: run, apply, host

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 5.1 | replay mismatches (E127) on recorded journals | gate | | unmeasured: no number: the named suites gate it in task gate | | |
| 5.2 | crash-recovery matrix rows failing (docs/slice.md 17.7) | gate | | unmeasured: no number: the named suites gate it in task gate | | |
| 5.3 | restart readiness, real clock | ratchet | llvm | 384 ms | 105 ms | ec2:c7i.metal-24xl, −1 better |
| 5.4 | apply cycle time of the todo, per target | ratchet | | unmeasured: not in task quality: `ignite-perf apply-cycle` exports and builds the benchmark trees and runs about 15 minutes; it is measured per slice (docs/performance/) | | |

## 6 Compiler speed

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 6.1 | check and plan of the design file | ratchet | all/check | 176 ms | 206 ms | ec2:c7i.metal-24xl, +6 |
| 6.1 | check and plan of the design file | ratchet | all/plan | 153 ms | 206 ms | ec2:c7i.metal-24xl, +5 |
| 6.2 | todo build, dev profile | ratchet | llvm | 993 ms | 1430 ms | ec2:c7i.metal-24xl, +132 worse |
| 6.2 | todo build, dev profile | ratchet | rust | 3081 ms | 4879 ms | ec2:c7i.metal-24xl, +759 worse |
| 6.3 | `task gate` wall time | ratchet | | unmeasured: no instrument records the gate's wall time; waves note it in their close (AGENTS.md § Gate time) | | |

## 7 Engineering health

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 7.1 | flaky tests per gate | ratchet | | unmeasured: no instrument reads the gate logs; known: apply_transition::a_failed_start_first_replacement, slice10::executable_component_main | | |
| 7.2 | ignored tests | ratchet | all | 24 | 24 | = |
| 7.3 | source files over 3,000 lines | ratchet | all | 4 | 4 | = |
| 7.4 | language friction rows open | ratchet | all | 38 | 38 | = |
| 7.5 | mutants that survive the suite on the compiler crates | ratchet | | unmeasured: no mutation testing yet (out of scope of story:quality-ratchet) | | |

## 8 Documentation and users

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 8.1 | `ignite` blocks in the book that fail to check | gate | | unmeasured: no number: `site::` gates it in task gate | | |
| 8.2 | website UI ratchet | ratchet | | unmeasured: its own baseline: ux/baseline.json | | |
| 8.3 | public APIs or diagnostics changed without a docs/slice.md change | gate | | unmeasured: a review rule; no test | | |
