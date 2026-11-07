# Ignite quality metrics, proposal (2026-11-07)

Status quo values come from a survey of `main` at `ab1e597a` (`survey.sh` next to this file) or from
AGENTS.md; "unmeasured" means no instrument exists yet.

Kinds:
- **G** hard gate: a count of failures or violations; the bar is "never above today" (usually 0).
- **R** ratchet: a lower- or higher-is-better value with a tolerance, kept in a baseline file; an improvement
  must be written into the baseline in the same merge (as `ux/baseline.json`).
- **H** hill-climb target: tracked per wave, not gating.

## 1. Semantics: one meaning across the 3 consumers (the mid-level IR's main gate)

| # | metric | kind | how | status quo |
|---|---|---|---|---|
| 1.1 | programs whose output differs between Rust, LLVM and the evaluator | G = 0 | differential corpus: every fixture and inline test run on all three, stdout/exit/failure text compared | 71 `on_both_targets` call sites; evaluator agreement only in `eval_equivalence.ignite`, `control_trace` |
| 1.2 | differential corpus size (programs x consumers) | R up | count | unmeasured |
| 1.3 | rules implemented more than once (patterns, each builtin, failure text, mains, copy/move, mutation) | H down, then G | a table of rule -> implementing functions, checked by a test | patterns 3, builtins 96 x 3 (review 2026-11-05); ownership 1 since wave 4 |
| 1.4 | generated-code goldens changed without a `CODEGEN_REVISION` bump | G = 0 | test over goldens + revision | rule exists, no test |
| 1.5 | random programs (generator) on which consumers disagree | G = 0 | a typed program generator, N seeds per gate | no generator |

## 2. Front end: the checker refuses exactly the wrong programs

| # | metric | kind | how | status quo |
|---|---|---|---|---|
| 2.1 | diagnostic codes in `docs/slice.md` 8 with no triggering fixture | G | grep codes vs tests | 14 of 88 without one: E011 E012 E014 E021 E070 E079 E093 E095 E097 E098 E130 E139 E149 E159 (retired or missing: not checked) |
| 2.2 | accepted programs that fail in a backend (rustc error, clang error, LLVM verifier) | G = 0 | every build in the suite + generator | known open: `story:immutable-binding-e006` (E0596 on Rust) |
| 2.3 | soundness holes open (effects, ownership, exhaustiveness) | R down | count of drafts tagged soundness | `map-put-handle-roots`, `immutable-binding-e006` |
| 2.4 | compiler panics on any input | G = 0 | fuzz the parser and checker on mutated corpus | no fuzzer |
| 2.5 | diagnostics per error site (duplicates), wrong spans | G = 0 | `.expected` fixtures | 318 `.expected` files |
| 2.6 | time to check scales linearly | G | `limits::` tests | exists (`limits.rs` in 2 crates) |

## 3. Generated code and runtime behaviour

| # | metric | kind | how | status quo |
|---|---|---|---|---|
| 3.1 | copies/clones per program (LLVM `copy` calls, Rust `.clone()`) on a fixed corpus | R down | count in goldens or IR | unmeasured |
| 3.2 | binary size of `todod`, `terminald` per target and profile | R down | `ignite-perf programs` | sizes not recorded |
| 3.3 | throughput and p99 latency of `terminald` / `todod` | R | `ignite-load matrix` (`task bench-throughput`) | exists, no baseline file |
| 3.4 | peak RSS, leaked handles, leaked threads after a load run | G | `llvm_memory`, thread-count tests | partial (`idle_terminald_runs_one_thread`) |
| 3.5 | runtime entry points that can unwind a Rust panic across `extern "C"` | G = 0 | test over `ignite-rt` | 91 entries, 4 `catch_unwind`/`C-unwind` hits |
| 3.6 | `unsafe` sites in `ignite-rt` + `ignite-io` | R down | grep | 73 |

## 4. Standard library and protocols

| # | metric | kind | how | status quo |
|---|---|---|---|---|
| 4.1 | conformance claims that hold | R up | `ignite conformance` | Telnet rfc854 yes, rfc855 yes, JSON-RPC yes; Http rfc9112 no (1 gap), rfc9110 no (1 gap) |
| 4.2 | gaps per claimed RFC | R down | same | 9112: 1, 9110: 1 |
| 4.3 | inline tests per target | R up | `task test-ignite` | 309 per target (wave 3 gate) |
| 4.4 | std functions with no inline test | R down | typed program vs test bodies | unmeasured |

## 5. Operations: run, apply, host

| # | metric | kind | how | status quo |
|---|---|---|---|---|
| 5.1 | replay mismatches (E127) on recorded journals | G = 0 | `replay` suites | exists |
| 5.2 | crash-recovery matrix rows failing (`docs/slice.md` 17.7) | G = 0 | `apply*` suites | 0 (slice 13 close) |
| 5.3 | restart readiness, real clock | R | `ignite-perf readiness` | 104 ms, bound 100-500 ms |
| 5.4 | apply cycle time, todo, per target | R down | `ignite-perf apply-cycle` | Rust 31.5 s, LLVM 30.8 s (slice 18) |

## 6. Compiler speed

| # | metric | kind | how | status quo |
|---|---|---|---|---|
| 6.1 | `ignite check` / `plan` of the design file | R, 10 % | `ignite-perf cli`, 15 pairs | 0.136 / 0.144 s (slice 18) |
| 6.2 | todo build, LLVM dev / Rust dev | R, 10 % | `ignite-perf programs` | 0.856 / 6.304 s; ratio 0.136, bound 0.15 |
| 6.3 | `task gate` wall time | R | gate script summary | 25-45 min at load 9-14 (waves 3, 4) |

## 7. Engineering health

| # | metric | kind | how | status quo |
|---|---|---|---|---|
| 7.1 | flaky tests (failed once, passed 3 of 3 alone) per gate | R down | gate logs | `apply_transition::a_failed_start_first_replacement`, `slice10::executable_component_main` known |
| 7.2 | `#[ignore]` tests | R down | grep | 16 |
| 7.3 | source files over 3,000 lines | R down | wc | `ignite-check/src/lib.rs`, `apply/plan.rs` among them (wave 6) |
| 7.4 | language friction rows open | R down | `docs/friction.md` | 59 rows |
| 7.5 | mutants that survive the suite on the compiler crates | R down | `cargo mutants` sample | unmeasured |

## 8. Documentation and users

| # | metric | kind | how | status quo |
|---|---|---|---|---|
| 8.1 | `ignite` blocks in the book that fail to check | G = 0 | `site::` | 0 |
| 8.2 | website UI ratchet | R | `ux/baseline.json` | exists |
| 8.3 | public APIs or diagnostics changed without a `docs/slice.md` change | G = 0 | review rule | no test |

## Instruments missing today

1. Differential runner over the 3 consumers (1.1, 1.2): needed before `mid-level-ir`.
2. Program generator / fuzzer (1.5, 2.4).
3. One `quality/baseline.json` + ratchet test, as `ux/baseline.json` (all R rows).
4. Copy counter on goldens (3.1), binary sizes (3.2), load baseline (3.3).
5. Panic-boundary test for `ignite-rt` (3.5).
