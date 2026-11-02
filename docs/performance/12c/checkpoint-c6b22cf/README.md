# Checkpoint c6b22cf: frozen comparison stopped

Candidate `c6b22cf148f1402499a4279975b52494106bd68a` is published on
`perf/restore-budget`. It includes full-scope parallel formatting, six empty
Cargo harness settings, identical-output JSON writes, bounded ETXTBSY retry,
streamed runtime inspection and the requested `--target=all` spelling. `all`
enumerates the supported native CLI target variants.

Smoke checks and full preparation passed. The frozen candidate ran 657 Rust
cases, the existing five ignored cases, and all 148 inline cases per backend.
The separate frozen LLVM readiness record is in `../readiness-c6b22cf/`.

Three matched program pairs pass every build guard: Rust dev 1.0714, Rust
release 1.0374, LLVM dev 0.8139 and LLVM release 0.9244. Fifteen check/plan pairs
still miss narrowly: check 1.103534 and plan 1.109699 against the unchanged
1.10 limit. These samples are retained; no favourable extra CLI pairs were run.

Two bounded 180-second low-load checks expired while unrelated host workloads
were active. Warm measurements therefore cannot establish the absolute
10.8-second goal. All samples and preflight records are retained. The third
warm workspace run failed `run::killing_terminald_restarts_it_in_real_time`:
599,617,298 ns restart latency exceeded the unchanged 500 ms bound. The same
frozen candidate had passed readiness at 118,879,906 ns and both preparations.
Load was about 20–22 at the failed run. This is a correlation, not an established
cause; CPU scheduling versus durable-write latency needs diagnosis.

The harness stopped immediately after the candidate failure. No replacement
greens or cold measurements were collected around it. This candidate is not
accepted. Next work profiles the actual successful design-check path for the
narrow frontend misses and investigates the recurrent real-clock latency.
Neither the baseline nor a correctness/performance bound is changed.
