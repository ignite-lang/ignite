# Quality trend

Generated from `quality/history.jsonl` by `task quality` (`quality::record`); do not edit.
One row per release and the first assessment (`bar`); the changes are against the row before
(`docs/quality.md`). Timing rows (source `perf`) measured on `host: local`, the shared
workstation, are not comparable between runs: they are left out of `worse`, `better` and
`changes` and counted under `timing`; the
dedicated host's runs have their own table below the main one.

| run | commit | date | worse | better | timing | changes |
|---|---|---|---|---|---|---|
| bar | 9a775116 | 2026-11-07 | 0 | 0 | 5 local, not comparable | 55 values, the bar |

## Timing on the dedicated host

Runs of `ignite-perf quality-remote` (`task quality-remote`): the timing rows only, measured on a
single-tenant EC2 host made for the run, HEAD's median over interleaved passes; the changes are
against the run before on a host of the same type.

| run | commit | date | host | values | changes |
|---|---|---|---|---|---|
| run | 4029b559 | 2026-11-08 | ec2:c7i.metal-24xl | 5.3 llvm 105 ms; 6.1 all/check 185 ms; 6.1 all/plan 186 ms; 6.2 llvm 1183 ms; 6.2 rust 3897 ms | first on this host |
