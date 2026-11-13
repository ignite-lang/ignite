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

Current values: run `v0.16.4` on 2026-11-13, commit `3f221dfb`.

## 1 Semantics: one meaning across the three consumers

| # | metric | kind | target | bar | current | trend |
|---|---|---|---|---|---|---|
| 1.1 | programs whose result differs between Rust, LLVM and the evaluator | gate | eval | 2 | 2 | = |
| 1.1 | programs whose result differs between Rust, LLVM and the evaluator | gate | llvm | 0 | 0 | = |
| 1.2 | differential corpus size | ratchet | eval | 792 | 792 | = |
| 1.2 | differential corpus size | ratchet | llvm | 953 | 953 | = |
| 1.2 | differential corpus size | ratchet | rust | 953 | 953 | = |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | eval/builtins | 112 | 126 | = |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | eval/patterns | 13 | 13 | = |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | llvm/builtins | 12 | 13 | = |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | llvm/patterns | 30 | 23 | = |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | rust/builtins | 43 | 56 | = |
| 1.3 | rules implemented more than once (patterns, builtins) | hill-climb | rust/patterns | 71 | 68 | = |
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
| 3.1 | copies in the generated code of the golden corpus | ratchet | rust | 155 | 155 | −1 better |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | all/design/check/instructions | 853115818 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | all/design/check/syscalls | 75 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | all/design/plan/instructions | 858815028 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | all/design/plan/syscalls | 75 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/design/build/instructions | 2236436054 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/design/build/syscalls | 14977 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/terminal/session/instructions | 245002 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/terminal/session/syscalls | 126 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/terminal/start/instructions | 304802 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/terminal/start/syscalls | 76 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/todo/connection/instructions | 469917 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/todo/connection/syscalls | 246 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/todo/delete/instructions | 636367 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/todo/delete/syscalls | 380 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/todo/get/instructions | 450236 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/todo/get/syscalls | 40 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/todo/patch/instructions | 930218 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/todo/patch/syscalls | 380 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/todo/start/instructions | 321791 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | llvm/todo/start/syscalls | 80 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/design/build/instructions | 1165423497 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/design/build/syscalls | 74 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/terminal/session/instructions | 131722 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/terminal/session/syscalls | 120 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/terminal/start/instructions | 369121 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/terminal/start/syscalls | 100 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/todo/connection/instructions | 225714 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/todo/connection/syscalls | 227 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/todo/delete/instructions | 530585 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/todo/delete/syscalls | 300 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/todo/get/instructions | 303778 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/todo/get/syscalls | 40 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/todo/patch/instructions | 789639 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/todo/patch/syscalls | 300 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/todo/start/instructions | 386833 |  |  |
| 3.10 | cost of a frozen path: instructions and system calls | ratchet | rust/todo/start/syscalls | 104 |  |  |
| 3.2 | binary size | ratchet | llvm/terminald/dev | 7264152 B | 7270968 B | +768 |
| 3.2 | binary size | ratchet | llvm/terminald/release | 1394120 B | 1394120 B | −328 better |
| 3.2 | binary size | ratchet | llvm/todod/dev | 7969480 B | 7976544 B | +840 |
| 3.2 | binary size | ratchet | llvm/todod/release | 1967072 B | 1967072 B | −312 better |
| 3.2 | binary size | ratchet | rust/terminald/dev | 6344576 B | 6354712 B | −800 better |
| 3.2 | binary size | ratchet | rust/terminald/release | 743384 B | 744408 B | +16 |
| 3.2 | binary size | ratchet | rust/todod/dev | 7586840 B | 7635104 B | +3088 |
| 3.2 | binary size | ratchet | rust/todod/release | 1038712 B | 1066544 B | +64 |
| 3.3 | throughput against plain Rust | ratchet | llvm/terminal/connect-8 | 0.929× | 0.919× | ec2:c7i.metal-24xl, +9 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/terminal/rr-1 | 0.995× | 0.990× | ec2:c7i.metal-24xl, +8 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/terminal/rr-64 | 1.254× | 1.245× | ec2:c7i.metal-24xl, +6 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/todo/get-1 | 1.035× | 1.035× | ec2:c7i.metal-24xl, +4 better |
| 3.3 | throughput against plain Rust | ratchet | llvm/todo/get-64 | 1.322× | 1.322× | ec2:c7i.metal-24xl, = |
| 3.3 | throughput against plain Rust | ratchet | llvm/todo/patch-16 | 1.203× | 1.203× | ec2:c7i.metal-24xl, +14 better |
| 3.3 | throughput against plain Rust | ratchet | rust/terminal/connect-8 | 0.974× | 0.974× | ec2:c7i.metal-24xl, +10 better |
| 3.3 | throughput against plain Rust | ratchet | rust/terminal/rr-1 | 0.974× | 0.966× | ec2:c7i.metal-24xl, −8 |
| 3.3 | throughput against plain Rust | ratchet | rust/terminal/rr-64 | 1.236× | 1.236× | ec2:c7i.metal-24xl, +2 better |
| 3.3 | throughput against plain Rust | ratchet | rust/todo/get-1 | 0.990× | 0.988× | ec2:c7i.metal-24xl, +5 better |
| 3.3 | throughput against plain Rust | ratchet | rust/todo/get-64 | 1.178× | 1.167× | ec2:c7i.metal-24xl, −11 |
| 3.3 | throughput against plain Rust | ratchet | rust/todo/patch-16 | 1.198× | 1.198× | ec2:c7i.metal-24xl, +14 better |
| 3.3 | throughput against plain Rust | ratchet | go/terminal/connect-8 | reference, no bar | 0.725× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/terminal/rr-1 | reference, no bar | 0.921× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/terminal/rr-64 | reference, no bar | 1.013× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/todo/get-1 | reference, no bar | 0.737× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/todo/get-64 | reference, no bar | 0.687× | ec2:c7i.metal-24xl |
| 3.3 | throughput against plain Rust | ratchet | go/todo/patch-16 | reference, no bar | 0.901× | ec2:c7i.metal-24xl |
| 3.4 | peak RSS, leaked handles and threads after a load run | gate | | unmeasured: no number: the named tests gate it in task gate | | |
| 3.5 | runtime entries that can unwind a panic across `extern "C"` | gate | llvm | 0 | 0 | = |
| 3.6 | `unsafe` sites in the substrate a target links | ratchet | llvm | 79 | 79 | = |
| 3.6 | `unsafe` sites in the substrate a target links | ratchet | rust | 12 | 12 | = |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/terminal/connect-8 | 1.105× | 1.292× | ec2:c7i.metal-24xl, −6 better |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/terminal/rr-1 | 0.992× | 0.992× | ec2:c7i.metal-24xl, −16 better |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/terminal/rr-64 | 0.383× | 0.383× | ec2:c7i.metal-24xl, −14 better |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/todo/get-1 | 0.961× | 0.992× | ec2:c7i.metal-24xl, +31 |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/todo/get-64 | 0.481× | 0.484× | ec2:c7i.metal-24xl, +3 |
| 3.7 | p99 latency against plain Rust | ratchet | llvm/todo/patch-16 | 0.566× | 0.573× | ec2:c7i.metal-24xl, −5 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/terminal/connect-8 | 1.055× | 1.191× | ec2:c7i.metal-24xl, −10 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/terminal/rr-1 | 0.980× | 1.020× | ec2:c7i.metal-24xl, −5 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/terminal/rr-64 | 0.388× | 0.388× | ec2:c7i.metal-24xl, −12 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/todo/get-1 | 0.992× | 0.992× | ec2:c7i.metal-24xl, −12 better |
| 3.7 | p99 latency against plain Rust | ratchet | rust/todo/get-64 | 0.541× | 0.558× | ec2:c7i.metal-24xl, +17 |
| 3.7 | p99 latency against plain Rust | ratchet | rust/todo/patch-16 | 0.740× | 0.794× | ec2:c7i.metal-24xl, +12 |
| 3.7 | p99 latency against plain Rust | ratchet | go/terminal/connect-8 | reference, no bar | 2.059× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/terminal/rr-1 | reference, no bar | 1.079× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/terminal/rr-64 | reference, no bar | 1.324× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/todo/get-1 | reference, no bar | 1.555× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/todo/get-64 | reference, no bar | 3.455× | ec2:c7i.metal-24xl |
| 3.7 | p99 latency against plain Rust | ratchet | go/todo/patch-16 | reference, no bar | 1.973× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/terminal/connect-8 | 0.934× | 0.958× | ec2:c7i.metal-24xl, +10 |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/terminal/rr-1 | 1.061× | 1.067× | ec2:c7i.metal-24xl, +2 |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/terminal/rr-64 | 1.168× | 1.172× | ec2:c7i.metal-24xl, −6 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/todo/get-1 | 0.722× | 0.722× | ec2:c7i.metal-24xl, −3 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/todo/get-64 | 1.083× | 1.093× | ec2:c7i.metal-24xl, +7 |
| 3.8 | peak RSS under load against plain Rust | ratchet | llvm/todo/patch-16 | 0.789× | 0.789× | ec2:c7i.metal-24xl, −11 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/terminal/connect-8 | 0.912× | 0.927× | ec2:c7i.metal-24xl, −12 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/terminal/rr-1 | 1.058× | 1.066× | ec2:c7i.metal-24xl, +8 |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/terminal/rr-64 | 0.798× | 0.798× | ec2:c7i.metal-24xl, −5 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/todo/get-1 | 0.748× | 0.748× | ec2:c7i.metal-24xl, −2 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/todo/get-64 | 0.770× | 0.775× | ec2:c7i.metal-24xl, +3 |
| 3.8 | peak RSS under load against plain Rust | ratchet | rust/todo/patch-16 | 0.757× | 0.757× | ec2:c7i.metal-24xl, −9 better |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/terminal/connect-8 | reference, no bar | 3.860× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/terminal/rr-1 | reference, no bar | 3.863× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/terminal/rr-64 | reference, no bar | 2.769× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/todo/get-1 | reference, no bar | 3.537× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/todo/get-64 | reference, no bar | 3.163× | ec2:c7i.metal-24xl |
| 3.8 | peak RSS under load against plain Rust | ratchet | go/todo/patch-16 | reference, no bar | 3.379× | ec2:c7i.metal-24xl |
| 3.9 | stripped binary size against plain Rust | ratchet | llvm/terminal | 1.308× | 1.315× | ec2:c7i.metal-24xl, = |
| 3.9 | stripped binary size against plain Rust | ratchet | llvm/todo | 0.706× | 0.708× | ec2:c7i.metal-24xl, = |
| 3.9 | stripped binary size against plain Rust | ratchet | rust/terminal | 1.507× | 1.509× | ec2:c7i.metal-24xl, = |
| 3.9 | stripped binary size against plain Rust | ratchet | rust/todo | 0.839× | 0.839× | ec2:c7i.metal-24xl, = |
| 3.9 | stripped binary size against plain Rust | ratchet | go/terminal | reference, no bar | 4.983× | ec2:c7i.metal-24xl |
| 3.9 | stripped binary size against plain Rust | ratchet | go/todo | reference, no bar | 4.837× | ec2:c7i.metal-24xl |

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
| 6.1 | check and plan of the design file | ratchet | all/check | 176 ms | 195 ms | ec2:c7i.metal-24xl, −2 better |
| 6.1 | check and plan of the design file | ratchet | all/plan | 153 ms | 197 ms | ec2:c7i.metal-24xl, = |
| 6.2 | todo build, dev profile | ratchet | llvm | 993 ms | 929 ms | ec2:c7i.metal-24xl, +2 |
| 6.2 | todo build, dev profile | ratchet | rust | 3081 ms | 4088 ms | ec2:c7i.metal-24xl, −18 better |
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
