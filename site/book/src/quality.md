# Quality

<!-- Generated from quality/baseline.json and quality/history.jsonl by `task quality`; site::quality_page_is_generated checks it. -->

Ignite keeps a written set of quality metrics. Today's values are the bar: a merge may not make a
gated metric worse, and an improvement is written into the bar in the same change, so the bar only
moves one way. A release measures every row against the release before it; a row that is worse is
tracked: the release notes list it with the story that takes it. The list, its kinds and its
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

The binary-size rows (3.2, 3.9) are the one exception for a release, marked `release_growth` in
`quality/baseline.json`: a release that adds code may leave a cell of them above the release before
it when the cell's bar carries a new decision naming the code that added the bytes, and the release
run lists every such cell. Without that decision, or with one the release before it already held,
the cell is listed as worse. A merge holds these rows to their bars like any other, and bytes are
never bought with time.

Current values: run `v0.16.1` on 2026-11-12, commit `f3b1e159`.

## 1 Semantics: one meaning across the three consumers

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 1.1 | programs whose result differs between Rust, LLVM and the evaluator | gate | eval | 2 | 2 | = |
| 1.1 | programs whose result differs between Rust, LLVM and the evaluator | gate | llvm | 0 | 0 | = |
| 1.2 | differential corpus size | ratchet | eval | 771 | 771 | +1 better |
| 1.2 | differential corpus size | ratchet | llvm | 932 | 932 | +1 better |
| 1.2 | differential corpus size | ratchet | rust | 932 | 932 | +1 better |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | eval/builtins | 112 | 124 | = |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | eval/patterns | 13 | 13 | = |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | llvm/builtins | 12 | 13 | = |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | llvm/patterns | 30 | 23 | = |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | rust/builtins | 43 | 53 | = |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | rust/patterns | 71 | 65 | = |
| 1.4 | generated-code goldens changed without a `CODEGEN_REVISION` bump | gate | all | 0 | 0 | = |
| 1.5 | random programs on which the consumers disagree | gate | | unmeasured: no program generator yet (out of scope of story:quality-ratchet) | | |
| 1.6 | corpus units the evaluator cannot run | ratchet | eval | 161 | 161 | = |
| 1.7 | known differences marked intermittent | gate | all | 1 | 1 | = |

## 2 Front end: the checker refuses exactly the wrong programs

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 2.1 | diagnostic codes without a test | gate | all | 11 | 11 | = |
| 2.2 | accepted programs that fail in a backend | gate | llvm | 0 | 0 | = |
| 2.2 | accepted programs that fail in a backend | gate | rust | 0 | 0 | = |
| 2.3 | soundness holes open (effects, ownership, exhaustiveness) | ratchet | | unmeasured: the planning store tags no story as a soundness hole; known open: story:map-put-handle-roots, story:immutable-binding-e006 | | |
| 2.4 | compiler panics on any input | gate | | unmeasured: no fuzzer yet (out of scope of story:quality-ratchet) | | |
| 2.5 | diagnostic fixtures (one diagnostic per error site, its span) | ratchet | all | 336 | 336 | = |
| 2.6 | checking time stays linear (limit tests) | gate | all | 6 | 6 | = |

## 3 Generated code and runtime behaviour

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 3.1 | copies in the generated code of the golden corpus | ratchet | llvm | 90 | 90 | = |
| 3.1 | copies in the generated code of the golden corpus | ratchet | rust | 163 | 163 | −3 better |
| 3.2 | binary size | ratchet | llvm/terminald/dev | 7264448 B | 7264456 B | +8 |
| 3.2 | binary size | ratchet | llvm/terminald/release | 1394536 B | 1394536 B | −224 better |
| 3.2 | binary size | ratchet | llvm/todod/dev | 7969688 B | 7969760 B | +72 |
| 3.2 | binary size | ratchet | llvm/todod/release | 1967944 B | 1967944 B | −240 better |
| 3.2 | binary size | ratchet | rust/terminald/dev | 6344576 B | 6344576 B | −2904 better |
| 3.2 | binary size | ratchet | rust/terminald/release | 743384 B | 743384 B | −912 better |
| 3.2 | binary size | ratchet | rust/todod/dev | 7586840 B | 7586840 B | −3224 better |
| 3.2 | binary size | ratchet | rust/todod/release | 1038712 B | 1062560 B | −400 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/terminal/connect-8 | 0.929× | 0.919× | ec2:c7i.metal-24xl, +45 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/terminal/rr-1 | 0.994× | 0.994× | ec2:c7i.metal-24xl, +13 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/terminal/rr-64 | 1.254× | 1.254× | ec2:c7i.metal-24xl, +383 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/todo/get-1 | 1.033× | 1.023× | ec2:c7i.metal-24xl, +12 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/todo/get-64 | 1.282× | 1.282× | ec2:c7i.metal-24xl, +155 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/todo/patch-16 | 1.203× | 1.203× | ec2:c7i.metal-24xl, +36 better |
| 3.3 | throughput against plain Rust | ratchet | rust/terminal/connect-8 | 0.966× | 0.966× | ec2:c7i.metal-24xl, +67 better |
| 3.3 | throughput against plain Rust | ratchet | rust/terminal/rr-1 | 0.972× | 0.972× | ec2:c7i.metal-24xl, +7 better |
| 3.3 | throughput against plain Rust | ratchet | rust/terminal/rr-64 | 1.222× | 1.222× | ec2:c7i.metal-24xl, +177 better |
| 3.3 | throughput against plain Rust | ratchet | rust/todo/get-1 | 0.990× | 0.990× | ec2:c7i.metal-24xl, +49 better |
| 3.3 | throughput against plain Rust | ratchet | rust/todo/get-64 | 1.176× | 1.176× | ec2:c7i.metal-24xl, +119 better |
| 3.3 | throughput against plain Rust | ratchet | rust/todo/patch-16 | 1.193× | 1.193× | ec2:c7i.metal-24xl, +28 better |
| 3.3 | throughput against plain Rust | ratchet | go/terminal/connect-8 | reference, no bar | 0.723× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/terminal/rr-1 | reference, no bar | 0.933× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/terminal/rr-64 | reference, no bar | 1.004× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/todo/get-1 | reference, no bar | 0.737× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/todo/get-64 | reference, no bar | 0.680× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/todo/patch-16 | reference, no bar | 0.886× | ec2:c7i.metal-24xl |
| 3.4 | peak RSS, leaked handles and threads after a load run | gate | | unmeasured: no number: the named tests gate it in task gate | | |
| 3.5 | runtime entries that can unwind a panic across `extern "C"` | gate | llvm | 0 | 0 | = |
| 3.6 | `unsafe` sites in the substrate a target links | ratchet | llvm | 79 | 79 | = |
| 3.6 | `unsafe` sites in the substrate a target links | ratchet | rust | 12 | 12 | = |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/terminal/connect-8 | 1.105× | 1.281× | ec2:c7i.metal-24xl, −186 better |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/terminal/rr-1 | 0.992× | 1.018× | ec2:c7i.metal-24xl, +26 |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/terminal/rr-64 | 0.400× | 0.400× | ec2:c7i.metal-24xl, −300 better |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/todo/get-1 | 0.984× | 1.000× | ec2:c7i.metal-24xl, +8 |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/todo/get-64 | 0.500× | 0.500× | ec2:c7i.metal-24xl, −150 better |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/todo/patch-16 | 0.566× | 0.566× | ec2:c7i.metal-24xl, −122 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/terminal/connect-8 | 1.055× | 1.194× | ec2:c7i.metal-24xl, −193 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/terminal/rr-1 | 0.980× | 1.015× | ec2:c7i.metal-24xl, +19 |
| 3.7 | p99 latency against plain Rust | ratchet | rust/terminal/rr-64 | 0.399× | 0.399× | ec2:c7i.metal-24xl, −111 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/todo/get-1 | 1.011× | 1.011× | ec2:c7i.metal-24xl, −37 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/todo/get-64 | 0.555× | 0.555× | ec2:c7i.metal-24xl, −301 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/todo/patch-16 | 0.740× | 0.740× | ec2:c7i.metal-24xl, −244 better |
| 3.7 | p99 latency against plain Rust | ratchet | go/terminal/connect-8 | reference, no bar | 2.057× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/terminal/rr-1 | reference, no bar | 1.045× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/terminal/rr-64 | reference, no bar | 1.369× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/todo/get-1 | reference, no bar | 1.442× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/todo/get-64 | reference, no bar | 3.544× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/todo/patch-16 | reference, no bar | 1.983× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/terminal/connect-8 | 0.934× | 0.934× | ec2:c7i.metal-24xl, −202 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/terminal/rr-1 | 1.061× | 1.061× | ec2:c7i.metal-24xl, −238 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/terminal/rr-64 | 1.171× | 1.171× | ec2:c7i.metal-24xl, −142 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/todo/get-1 | 0.741× | 0.741× | ec2:c7i.metal-24xl, −144 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/todo/get-64 | 1.083× | 1.083× | ec2:c7i.metal-24xl, −129 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/todo/patch-16 | 0.808× | 0.808× | ec2:c7i.metal-24xl, −133 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/terminal/connect-8 | 0.923× | 0.923× | ec2:c7i.metal-24xl, −44 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/terminal/rr-1 | 1.063× | 1.066× | ec2:c7i.metal-24xl, −57 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/terminal/rr-64 | 0.810× | 0.810× | ec2:c7i.metal-24xl, −31 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/todo/get-1 | 0.765× | 0.768× | ec2:c7i.metal-24xl, −59 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/todo/get-64 | 0.770× | 0.770× | ec2:c7i.metal-24xl, −124 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/todo/patch-16 | 0.770× | 0.770× | ec2:c7i.metal-24xl, −107 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/terminal/connect-8 | reference, no bar | 3.771× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/terminal/rr-1 | reference, no bar | 3.720× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/terminal/rr-64 | reference, no bar | 2.771× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/todo/get-1 | reference, no bar | 3.607× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/todo/get-64 | reference, no bar | 3.100× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/todo/patch-16 | reference, no bar | 3.431× | ec2:c7i.metal-24xl |
| 3.9 | stripped binary size against plain Rust | ratchet | llvm/terminal | 1.308× | 1.308× | ec2:c7i.metal-24xl, −522 better |
| 3.9 | stripped binary size against plain Rust | ratchet | llvm/todo | 0.706× | 0.706× | ec2:c7i.metal-24xl, −195 better |
| 3.9 | stripped binary size against plain Rust | ratchet | rust/terminal | 1.510× | 1.510× | ec2:c7i.metal-24xl, −37 better |
| 3.9 | stripped binary size against plain Rust | ratchet | rust/todo | 0.839× | 0.839× | ec2:c7i.metal-24xl, −11 better |
| 3.9 | stripped binary size against plain Rust | ratchet | go/terminal | reference, no bar | 4.996× | ec2:c7i.metal-24xl |
| 3.9 | stripped binary size against plain Rust | ratchet | go/todo | reference, no bar | 4.849× | ec2:c7i.metal-24xl |

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
| 4.3 | inline tests per target | ratchet | eval | 427 | 427 | = |
| 4.3 | inline tests per target | ratchet | llvm | 517 | 517 | = |
| 4.3 | inline tests per target | ratchet | rust | 517 | 517 | = |
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
| 6.1 | check and plan of the design file | ratchet | all/check | 176 ms | 198 ms | ec2:c7i.metal-24xl, −9 better |
| 6.1 | check and plan of the design file | ratchet | all/plan | 153 ms | 199 ms | ec2:c7i.metal-24xl, −8 better |
| 6.2 | todo build, dev profile | ratchet | llvm | 993 ms | 933 ms | ec2:c7i.metal-24xl, −942 better |
| 6.2 | todo build, dev profile | ratchet | rust | 3081 ms | 4110 ms | ec2:c7i.metal-24xl, −847 better |
| 6.3 | `task gate` wall time | ratchet | | unmeasured: no instrument records the gate's wall time; waves note it in their close (AGENTS.md § Gate time) | | |

## 7 Engineering health

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 7.1 | flaky tests per gate | ratchet | | unmeasured: no instrument reads the gate logs; known: apply_transition::a_failed_start_first_replacement, slice10::executable_component_main | | |
| 7.2 | ignored tests | ratchet | all | 23 | 23 | = |
| 7.3 | source files over 3,000 lines | ratchet | all | 4 | 4 | = |
| 7.4 | language friction rows open | ratchet | all | 38 | 38 | = |
| 7.5 | mutants that survive the suite on the compiler crates | ratchet | | unmeasured: no mutation testing yet (out of scope of story:quality-ratchet) | | |

## 8 Documentation and users

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 8.1 | `ignite` blocks in the book that fail to check | gate | | unmeasured: no number: `site::` gates it in task gate | | |
| 8.2 | website UI ratchet | ratchet | | unmeasured: its own baseline: ux/baseline.json | | |
| 8.3 | public APIs or diagnostics changed without a docs/slice.md change | gate | | unmeasured: a review rule; no test | | |
