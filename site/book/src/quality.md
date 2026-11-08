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

Timing rows (build, check and restart times, sizes) are measured on a shared workstation for now
(`host: local`): they are recorded and shown, but not compared or gated until a dedicated host
measures them.

Current values: run `bar` on 2026-11-07, commit `9a775116`.

## 1 Semantics: one meaning across the three consumers

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 1.1 | programs whose result differs between Rust, LLVM and the evaluator | gate | eval | 4 | 4 |  |
| 1.1 | programs whose result differs between Rust, LLVM and the evaluator | gate | llvm | 0 | 0 |  |
| 1.2 | differential corpus size | ratchet | eval | 423 | 420 |  |
| 1.2 | differential corpus size | ratchet | llvm | 643 | 614 |  |
| 1.2 | differential corpus size | ratchet | rust | 643 | 614 |  |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | eval/builtins | 112 | 112 |  |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | eval/patterns | 13 | 13 |  |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | llvm/builtins | 12 | 12 |  |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | llvm/patterns | 30 | 30 |  |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | rust/builtins | 43 | 43 |  |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | rust/patterns | 71 | 71 |  |
| 1.4 | generated-code goldens changed without a `CODEGEN_REVISION` bump | gate | all | 0 | 0 |  |
| 1.5 | random programs on which the consumers disagree | gate | | unmeasured: no program generator yet (out of scope of story:quality-ratchet) | | |
| 1.6 | corpus units the evaluator cannot run | ratchet | eval | 235 | 194 |  |
| 1.7 | known differences marked intermittent | gate | all | 1 | 1 |  |

## 2 Front end: the checker refuses exactly the wrong programs

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 2.1 | diagnostic codes without a test | gate | all | 11 | 11 |  |
| 2.2 | accepted programs that fail in a backend | gate | llvm | 0 | 0 |  |
| 2.2 | accepted programs that fail in a backend | gate | rust | 0 | 0 |  |
| 2.3 | soundness holes open (effects, ownership, exhaustiveness) | ratchet | | unmeasured: the planning store tags no story as a soundness hole; known open: story:map-put-handle-roots, story:immutable-binding-e006 | | |
| 2.4 | compiler panics on any input | gate | | unmeasured: no fuzzer yet (out of scope of story:quality-ratchet) | | |
| 2.5 | diagnostic fixtures (one diagnostic per error site, its span) | ratchet | all | 323 | 319 |  |
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
| 3.3 | throughput and p99 latency of terminald and todod | ratchet | | unmeasured: no load baseline yet (out of scope of story:quality-ratchet) | | |
| 3.4 | peak RSS, leaked handles and threads after a load run | gate | | unmeasured: no number: the named tests gate it in task gate | | |
| 3.5 | runtime entries that can unwind a panic across `extern "C"` | gate | llvm | 0 | 0 |  |
| 3.6 | `unsafe` sites in the substrate a target links | ratchet | llvm | 79 | 73 |  |
| 3.6 | `unsafe` sites in the substrate a target links | ratchet | rust | 9 | 7 |  |

## 4 Standard library and protocols

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 4.1 | conformance claims that hold | ratchet | llvm | 3 | 3 |  |
| 4.1 | conformance claims that hold | ratchet | rust | 3 | 3 |  |
| 4.2 | gaps per claimed spec | ratchet | llvm/jsonrpc-2.0 | 0 | 0 |  |
| 4.2 | gaps per claimed spec | ratchet | llvm/rfc854 | 0 | 0 |  |
| 4.2 | gaps per claimed spec | ratchet | llvm/rfc855 | 0 | 0 |  |
| 4.2 | gaps per claimed spec | ratchet | llvm/rfc9110 | 2 | 1 |  |
| 4.2 | gaps per claimed spec | ratchet | llvm/rfc9112 | 1 | 1 |  |
| 4.2 | gaps per claimed spec | ratchet | rust/jsonrpc-2.0 | 0 | 0 |  |
| 4.2 | gaps per claimed spec | ratchet | rust/rfc854 | 0 | 0 |  |
| 4.2 | gaps per claimed spec | ratchet | rust/rfc855 | 0 | 0 |  |
| 4.2 | gaps per claimed spec | ratchet | rust/rfc9110 | 2 | 1 |  |
| 4.2 | gaps per claimed spec | ratchet | rust/rfc9112 | 1 | 1 |  |
| 4.3 | inline tests per target | ratchet | eval | 191 | 191 |  |
| 4.3 | inline tests per target | ratchet | llvm | 322 | 309 |  |
| 4.3 | inline tests per target | ratchet | rust | 322 | 309 |  |
| 4.4 | std functions with no inline test | ratchet | | unmeasured: no instrument yet: needs the call graph of the test bodies | | |

## 5 Operations: run, apply, host

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 5.1 | replay mismatches (E127) on recorded journals | gate | | unmeasured: no number: the named suites gate it in task gate | | |
| 5.2 | crash-recovery matrix rows failing (docs/slice.md 17.7) | gate | | unmeasured: no number: the named suites gate it in task gate | | |
| 5.3 | restart readiness, real clock | ratchet | llvm | 384 ms | 133 ms | local, not comparable |
| 5.4 | apply cycle time of the todo, per target | ratchet | | unmeasured: not in task quality: `ignite-perf apply-cycle` exports and builds the benchmark trees and runs about 15 minutes; it is measured per slice (docs/performance/) | | |

## 6 Compiler speed

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 6.1 | check and plan of the design file | ratchet | all/check | 176 ms | 317 ms | local, not comparable |
| 6.1 | check and plan of the design file | ratchet | all/plan | 153 ms | 241 ms | local, not comparable |
| 6.2 | todo build, dev profile | ratchet | llvm | 993 ms | 1203 ms | local, not comparable |
| 6.2 | todo build, dev profile | ratchet | rust | 3081 ms | 2907 ms | local, not comparable |
| 6.3 | `task gate` wall time | ratchet | | unmeasured: no instrument records the gate's wall time; waves note it in their close (AGENTS.md § Gate time) | | |

## 7 Engineering health

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 7.1 | flaky tests per gate | ratchet | | unmeasured: no instrument reads the gate logs; known: apply_transition::a_failed_start_first_replacement, slice10::executable_component_main | | |
| 7.2 | ignored tests | ratchet | all | 24 | 21 |  |
| 7.3 | source files over 3,000 lines | ratchet | all | 4 | 4 |  |
| 7.4 | language friction rows open | ratchet | all | 43 | 34 |  |
| 7.5 | mutants that survive the suite on the compiler crates | ratchet | | unmeasured: no mutation testing yet (out of scope of story:quality-ratchet) | | |

## 8 Documentation and users

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 8.1 | `ignite` blocks in the book that fail to check | gate | | unmeasured: no number: `site::` gates it in task gate | | |
| 8.2 | website UI ratchet | ratchet | | unmeasured: its own baseline: ux/baseline.json | | |
| 8.3 | public APIs or diagnostics changed without a docs/slice.md change | gate | | unmeasured: a review rule; no test | | |
