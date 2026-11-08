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
| run | 25a14ae7 | 2026-11-08 | ec2:c7i.metal-24xl | 5.3 llvm 105 ms; 6.1 all/check 185 ms; 6.1 all/plan 185 ms; 6.2 llvm 1183 ms; 6.2 rust 3907 ms | 6.1 all/plan 186 ms→185 ms better; 6.2 rust 3897 ms→3907 ms |

## Against plain Rust on the dedicated host

The native benchmark of `ignite-perf quality-remote` (`bench/native/`, `docs/quality.md` row 3.3):
one row per run and cell, each implementation's median over the run's passes against the plain
Rust reference measured in the same pass (above 1 means more replies per second, a longer p99, more
peak RSS or a larger stripped binary than plain Rust).

| run | commit | date | host | cell | Ignite LLVM / Rust | Ignite Rust target / Rust | Go / Rust |
|---|---|---|---|---|---|---|---|
| run | 25a14ae7 | 2026-11-08 | ec2:c7i.metal-24xl | terminal | size 1.423× | size 1.304× | size 5.172× |
| run | 25a14ae7 | 2026-11-08 | ec2:c7i.metal-24xl | terminal/connect-8 | rps 2.343×, p99 0.193×, RSS 2.623× | rps 2.851×, p99 0.183×, RSS 0.973× | rps 2.092×, p99 0.379×, RSS 3.799× |
| run | 25a14ae7 | 2026-11-08 | ec2:c7i.metal-24xl | terminal/rr-1 | rps 0.900×, p99 1.017×, RSS 1.220× | rps 0.874×, p99 1.029×, RSS 1.067× | rps 0.890×, p99 1.059×, RSS 3.955× |
| run | 25a14ae7 | 2026-11-08 | ec2:c7i.metal-24xl | terminal/rr-64 | rps 0.848×, p99 1.495×, RSS 2.295× | rps 0.821×, p99 1.382×, RSS 1.035× | rps 0.965×, p99 1.452×, RSS 2.778× |
| run | 25a14ae7 | 2026-11-08 | ec2:c7i.metal-24xl | todo | size 0.700× | size 0.692× | size 4.852× |
| run | 25a14ae7 | 2026-11-08 | ec2:c7i.metal-24xl | todo/get-1 | rps 0.905×, p99 1.081×, RSS 0.833× | rps 0.786×, p99 1.234×, RSS 0.753× | rps 0.732×, p99 1.589×, RSS 3.425× |
| run | 25a14ae7 | 2026-11-08 | ec2:c7i.metal-24xl | todo/get-64 | rps 0.879×, p99 1.287×, RSS 2.837× | rps 0.694×, p99 1.644×, RSS 2.652× | rps 0.686×, p99 3.538×, RSS 3.081× |
| run | 25a14ae7 | 2026-11-08 | ec2:c7i.metal-24xl | todo/patch-16 | rps 0.578×, p99 2.170×, RSS 1.516× | rps 0.560×, p99 2.313×, RSS 1.185× | rps 0.897×, p99 1.970×, RSS 3.545× |
