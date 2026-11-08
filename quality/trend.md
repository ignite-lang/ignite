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
| v0.14.0 | d87fd798 | 2026-11-08 | 10 | 6 | 5 local, not comparable | 1.2 eval 420→423; 1.2 llvm 614→643; 1.2 rust 614→643; 1.3 eval/builtins 112→120; 1.3 llvm/builtins 12→13; 1.3 rust/builtins 43→44; 1.6 eval 194→220; 2.5 all 319→323; 3.2 llvm/terminald/dev 7563720 B→7780040 B; 3.2 llvm/terminald/release 1468648 B→1505048 B; 3.2 llvm/todod/dev 8230560 B→8452816 B; 3.2 llvm/todod/release 2042752 B→2088624 B; 3.2 rust/terminald/dev 6223520 B→6390088 B; 3.2 rust/terminald/release 681368 B→695480 B; 3.2 rust/todod/dev 7405864 B→7574008 B; 3.2 rust/todod/release 961880 B→980136 B; 3.6 llvm 73→76; 3.6 rust 7→9; 4.2 llvm/rfc9110 1→2; 4.2 rust/rfc9110 1→2; 4.3 llvm 309→322; 4.3 rust 309→322; 7.2 all 21→23; 7.4 all 34→43 |

## Timing on the dedicated host

Runs of `ignite-perf quality-remote` (`task quality-remote`): the timing rows only, measured on a
single-tenant EC2 host made for the run, HEAD's median over interleaved passes; the changes are
against the run before on a host of the same type.

| run | commit | date | host | values | changes |
|---|---|---|---|---|---|
| run | 4029b559 | 2026-11-08 | ec2:c7i.metal-24xl | 5.3 llvm 105 ms; 6.1 all/check 185 ms; 6.1 all/plan 186 ms; 6.2 llvm 1183 ms; 6.2 rust 3897 ms | first on this host |
| run | f38aeabd | 2026-11-08 | ec2:c7i.metal-24xl | 5.3 llvm 105 ms; 6.1 all/check 185 ms; 6.1 all/plan 186 ms; 6.2 llvm 1181 ms; 6.2 rust 3905 ms | 6.2 llvm 1183 ms→1181 ms better; 6.2 rust 3897 ms→3905 ms |

## Against plain Rust on the dedicated host

The native benchmark of `ignite-perf quality-remote` (`bench/native/`, `docs/quality.md` row 3.3):
one row per run and cell, each implementation's median over the run's passes against the plain
Rust reference measured in the same pass (above 1 means more replies per second, a longer p99, more
peak RSS or a larger stripped binary than plain Rust).

| run | commit | date | host | cell | Ignite LLVM / Rust | Ignite Rust target / Rust | Go / Rust |
|---|---|---|---|---|---|---|---|
| run | f38aeabd | 2026-11-08 | ec2:c7i.metal-24xl | terminal | size 1.378× | size 1.263× | size 5.009× |
| run | f38aeabd | 2026-11-08 | ec2:c7i.metal-24xl | terminal/connect-8 | rps 0.766×, p99 1.132×, RSS 2.645× | rps 0.946×, p99 1.058×, RSS 0.983× | rps 0.700×, p99 2.163×, RSS 3.732× |
| run | f38aeabd | 2026-11-08 | ec2:c7i.metal-24xl | terminal/rr-1 | rps 0.906×, p99 1.017×, RSS 1.221× | rps 0.886×, p99 1.008×, RSS 1.065× | rps 0.891×, p99 1.059×, RSS 3.904× |
| run | f38aeabd | 2026-11-08 | ec2:c7i.metal-24xl | terminal/rr-64 | rps 0.858×, p99 1.240×, RSS 2.288× | rps 0.824×, p99 1.243×, RSS 1.036× | rps 0.961×, p99 1.371×, RSS 2.857× |
| run | f38aeabd | 2026-11-08 | ec2:c7i.metal-24xl | todo | size 0.700× | size 0.692× | size 4.852× |
| run | f38aeabd | 2026-11-08 | ec2:c7i.metal-24xl | todo/get-1 | rps 0.899×, p99 1.099×, RSS 0.839× | rps 0.809×, p99 1.202×, RSS 0.765× | rps 0.746×, p99 1.553×, RSS 3.537× |
| run | f38aeabd | 2026-11-08 | ec2:c7i.metal-24xl | todo/get-64 | rps 0.869×, p99 1.265×, RSS 2.857× | rps 0.703×, p99 1.551×, RSS 2.654× | rps 0.678×, p99 3.538×, RSS 3.114× |
| run | f38aeabd | 2026-11-08 | ec2:c7i.metal-24xl | todo/patch-16 | rps 0.576×, p99 2.193×, RSS 1.468× | rps 0.567×, p99 2.275×, RSS 1.158× | rps 0.884×, p99 1.994×, RSS 3.405× |
