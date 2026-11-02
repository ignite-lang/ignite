# Frozen LLVM readiness

`scripts/verify-llvm-readiness.py` passed against candidate `c6b22cf148f1`.
`report.json` records the exact CLI/runtime hashes and source-manifest hash.
All commands, controller output, stderr, journals and replay results are
retained here. Generated executables and disposable build caches are omitted.

- A real terminal connection returned its prompt and echoed a command.
- Manual restart occurred exactly 100 ms after exit; the next backoff was 200 ms.
- Real-clock restart took 118.880 ms, within the unchanged 100–500 ms bound.
- Suspending the owned server produced a failed probe and violated standing.
- Resuming it produced a good probe while retaining the rolling violation.
- Calendar closure emitted exactly one consequence linked to its standing.
- Status agreed with the journal; every scenario stopped and replayed successfully.
- No owned live process remained after stop.

The frozen preparation gate separately passed the Rust run/probe/restart cases
and draining rollouts on both native targets, along with 657 Rust cases, five
ignored and 148 inline cases per backend. Performance acceptance remains
separate and open until the matched comparison passes.
