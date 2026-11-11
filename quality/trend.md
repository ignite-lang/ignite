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
| v0.15.0 | 6e144f76 | 2026-11-08 | 8 | 9 | 5 local, not comparable | 1.2 eval 423→452; 1.2 llvm 643→688; 1.2 rust 643→688; 1.3 eval/builtins 120→133; 1.3 llvm/builtins 13→14; 1.3 rust/builtins 44→54; 1.3 rust/patterns 71→83; 1.6 eval 220→236; 2.5 all 323→329; 3.1 rust 168→166; 3.2 llvm/terminald/dev 7780040 B→7831016 B; 3.2 llvm/terminald/release 1505048 B→1512792 B; 3.2 llvm/todod/dev 8452816 B→8531472 B; 3.2 llvm/todod/release 2088624 B→2107792 B; 3.2 rust/terminald/dev 6390088 B→6429272 B; 3.2 rust/terminald/release 695480 B→706440 B; 3.2 rust/todod/dev 7574008 B→7710792 B; 3.2 rust/todod/release 980136 B→1011216 B; 3.6 llvm 76→82; 3.6 rust 9→12; 4.3 eval 191→209; 4.3 llvm 322→350; 4.3 rust 322→350; 7.2 all 23→24; 7.4 all 43→38 |

## Timing on the dedicated host

Runs of `ignite-perf quality-remote` (`task quality-remote`): the timing rows only, measured on a
single-tenant EC2 host made for the run, HEAD's median over interleaved passes; the changes are
against the run before on a host of the same type.

| run | commit | date | host | values | changes |
|---|---|---|---|---|---|
| run | 4029b559 | 2026-11-08 | ec2:c7i.metal-24xl | 5.3 llvm 105 ms; 6.1 all/check 185 ms; 6.1 all/plan 186 ms; 6.2 llvm 1183 ms; 6.2 rust 3897 ms | first on this host |
| run | f38aeabd | 2026-11-08 | ec2:c7i.metal-24xl | 5.3 llvm 105 ms; 6.1 all/check 185 ms; 6.1 all/plan 186 ms; 6.2 llvm 1181 ms; 6.2 rust 3905 ms | 6.2 llvm 1183 ms→1181 ms better; 6.2 rust 3897 ms→3905 ms |
| v0.15.0 | 798033f2 | 2026-11-08 | ec2:c7i.metal-24xl | 5.3 llvm 106 ms; 6.1 all/check 200 ms; 6.1 all/plan 201 ms; 6.2 llvm 1298 ms; 6.2 rust 4120 ms | 5.3 llvm 105 ms→106 ms; 6.1 all/check 185 ms→200 ms; 6.1 all/plan 186 ms→201 ms; 6.2 llvm 1181 ms→1298 ms; 6.2 rust 3905 ms→4120 ms |
| run | 66a3d5b9 | 2026-11-09 | ec2:c7i.metal-24xl | 5.3 llvm 105 ms; 6.1 all/check 206 ms; 6.1 all/plan 206 ms; 6.2 llvm 1430 ms; 6.2 rust 4879 ms | 5.3 llvm 106 ms→105 ms better; 6.1 all/check 200 ms→206 ms; 6.1 all/plan 201 ms→206 ms; 6.2 llvm 1298 ms→1430 ms worse; 6.2 rust 4120 ms→4879 ms worse |
| run | 4520b8cf | 2026-11-10 | ec2:c7i.metal-24xl | 5.3 llvm 106 ms; 6.1 all/check 207 ms; 6.1 all/plan 207 ms; 6.2 llvm 1875 ms; 6.2 rust 4957 ms | 5.3 llvm 105 ms→106 ms; 6.1 all/check 206 ms→207 ms; 6.1 all/plan 206 ms→207 ms; 6.2 llvm 1430 ms→1875 ms worse; 6.2 rust 4879 ms→4957 ms |
| v0.16.0 | a45d5cd3 | 2026-11-11 | ec2:c7i.metal-24xl | 5.3 llvm 105 ms; 6.1 all/check 198 ms; 6.1 all/plan 199 ms; 6.2 llvm 933 ms; 6.2 rust 4110 ms | 5.3 llvm 106 ms→105 ms better; 6.1 all/check 207 ms→198 ms better; 6.1 all/plan 207 ms→199 ms better; 6.2 llvm 1875 ms→933 ms better; 6.2 rust 4957 ms→4110 ms better |

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
| v0.15.0 | 798033f2 | 2026-11-08 | ec2:c7i.metal-24xl | terminal | size 1.406× | size 1.304× | size 4.998× |
| v0.15.0 | 798033f2 | 2026-11-08 | ec2:c7i.metal-24xl | terminal/connect-8 | rps 0.929×, p99 1.105×, RSS 2.938× | rps 0.956×, p99 1.055×, RSS 0.981× | rps 0.730×, p99 2.073×, RSS 3.750× |
| v0.15.0 | 798033f2 | 2026-11-08 | ec2:c7i.metal-24xl | terminal/rr-1 | rps 0.975×, p99 1.000×, RSS 1.203× | rps 0.962×, p99 1.004×, RSS 1.063× | rps 0.904×, p99 1.053×, RSS 3.775× |
| v0.15.0 | 798033f2 | 2026-11-08 | ec2:c7i.metal-24xl | terminal/rr-64 | rps 0.971×, p99 1.032×, RSS 1.434× | rps 0.956×, p99 1.082×, RSS 1.043× | rps 1.014×, p99 1.374×, RSS 2.804× |
| v0.15.0 | 798033f2 | 2026-11-08 | ec2:c7i.metal-24xl | todo | size 0.720× | size 0.723× | size 4.852× |
| v0.15.0 | 798033f2 | 2026-11-08 | ec2:c7i.metal-24xl | todo/get-1 | rps 1.015×, p99 0.992×, RSS 0.819× | rps 0.975×, p99 1.044×, RSS 0.778× | rps 0.728×, p99 1.564×, RSS 3.572× |
| v0.15.0 | 798033f2 | 2026-11-08 | ec2:c7i.metal-24xl | todo/get-64 | rps 1.069×, p99 1.103×, RSS 1.258× | rps 0.981×, p99 1.252×, RSS 2.612× | rps 0.680×, p99 3.549×, RSS 3.214× |
| v0.15.0 | 798033f2 | 2026-11-08 | ec2:c7i.metal-24xl | todo/patch-16 | rps 1.012×, p99 1.445×, RSS 0.913× | rps 0.994×, p99 1.543×, RSS 1.225× | rps 0.908×, p99 1.955×, RSS 3.417× |
| run | 66a3d5b9 | 2026-11-09 | ec2:c7i.metal-24xl | terminal | size 1.820× | size 1.669× | size 4.998× |
| run | 66a3d5b9 | 2026-11-09 | ec2:c7i.metal-24xl | terminal/connect-8 | rps 0.876×, p99 1.470×, RSS 1.137× | rps 0.892×, p99 1.415×, RSS 1.000× | rps 0.722×, p99 2.094×, RSS 3.763× |
| run | 66a3d5b9 | 2026-11-09 | ec2:c7i.metal-24xl | terminal/rr-1 | rps 0.987×, p99 1.000×, RSS 1.289× | rps 0.971×, p99 0.980×, RSS 1.148× | rps 0.926×, p99 1.033×, RSS 3.940× |
| run | 66a3d5b9 | 2026-11-09 | ec2:c7i.metal-24xl | terminal/rr-64 | rps 0.859×, p99 0.719×, RSS 1.314× | rps 1.024×, p99 0.526×, RSS 0.858× | rps 0.998×, p99 1.326×, RSS 2.866× |
| run | 66a3d5b9 | 2026-11-09 | ec2:c7i.metal-24xl | todo | size 0.869× | size 0.864× | size 4.852× |
| run | 66a3d5b9 | 2026-11-09 | ec2:c7i.metal-24xl | todo/get-1 | rps 1.033×, p99 0.984×, RSS 0.882× | rps 0.959×, p99 1.049×, RSS 0.839× | rps 0.737×, p99 1.560×, RSS 3.542× |
| run | 66a3d5b9 | 2026-11-09 | ec2:c7i.metal-24xl | todo/get-64 | rps 1.210×, p99 1.500×, RSS 1.152× | rps 0.826×, p99 1.054×, RSS 0.837× | rps 0.679×, p99 3.501×, RSS 3.161× |
| run | 66a3d5b9 | 2026-11-09 | ec2:c7i.metal-24xl | todo/patch-16 | rps 1.193×, p99 0.903×, RSS 0.941× | rps 1.185×, p99 1.207×, RSS 0.878× | rps 0.911×, p99 1.978×, RSS 3.488× |
| run | 4520b8cf | 2026-11-10 | ec2:c7i.metal-24xl | terminal | size 1.830× | size 1.547× | size 4.998× |
| run | 4520b8cf | 2026-11-10 | ec2:c7i.metal-24xl | terminal/connect-8 | rps 0.874×, p99 1.467×, RSS 1.136× | rps 0.899×, p99 1.387×, RSS 0.967× | rps 0.721×, p99 2.080×, RSS 3.827× |
| run | 4520b8cf | 2026-11-10 | ec2:c7i.metal-24xl | terminal/rr-1 | rps 0.981×, p99 0.992×, RSS 1.299× | rps 0.965×, p99 0.996×, RSS 1.123× | rps 0.915×, p99 1.041×, RSS 3.938× |
| run | 4520b8cf | 2026-11-10 | ec2:c7i.metal-24xl | terminal/rr-64 | rps 0.871×, p99 0.700×, RSS 1.313× | rps 1.045×, p99 0.510×, RSS 0.841× | rps 1.009×, p99 1.371×, RSS 2.816× |
| run | 4520b8cf | 2026-11-10 | ec2:c7i.metal-24xl | todo | size 0.901× | size 0.850× | size 4.852× |
| run | 4520b8cf | 2026-11-10 | ec2:c7i.metal-24xl | todo/get-1 | rps 1.011×, p99 0.992×, RSS 0.885× | rps 0.941×, p99 1.048×, RSS 0.827× | rps 0.733×, p99 1.577×, RSS 3.511× |
| run | 4520b8cf | 2026-11-10 | ec2:c7i.metal-24xl | todo/get-64 | rps 1.127×, p99 0.650×, RSS 1.212× | rps 1.057×, p99 0.856×, RSS 0.894× | rps 0.687×, p99 3.463×, RSS 3.128× |
| run | 4520b8cf | 2026-11-10 | ec2:c7i.metal-24xl | todo/patch-16 | rps 1.167×, p99 0.688×, RSS 0.941× | rps 1.165×, p99 0.984×, RSS 0.877× | rps 0.899×, p99 1.978×, RSS 3.437× |
| v0.16.0 | a45d5cd3 | 2026-11-11 | ec2:c7i.metal-24xl | terminal | size 1.308× | size 1.510× | size 4.996× |
| v0.16.0 | a45d5cd3 | 2026-11-11 | ec2:c7i.metal-24xl | terminal/connect-8 | rps 0.919×, p99 1.281×, RSS 0.934× | rps 0.966×, p99 1.194×, RSS 0.923× | rps 0.723×, p99 2.057×, RSS 3.771× |
| v0.16.0 | a45d5cd3 | 2026-11-11 | ec2:c7i.metal-24xl | terminal/rr-1 | rps 0.994×, p99 1.018×, RSS 1.061× | rps 0.972×, p99 1.015×, RSS 1.066× | rps 0.933×, p99 1.045×, RSS 3.720× |
| v0.16.0 | a45d5cd3 | 2026-11-11 | ec2:c7i.metal-24xl | terminal/rr-64 | rps 1.254×, p99 0.400×, RSS 1.171× | rps 1.222×, p99 0.399×, RSS 0.810× | rps 1.004×, p99 1.369×, RSS 2.771× |
| v0.16.0 | a45d5cd3 | 2026-11-11 | ec2:c7i.metal-24xl | todo | size 0.706× | size 0.839× | size 4.849× |
| v0.16.0 | a45d5cd3 | 2026-11-11 | ec2:c7i.metal-24xl | todo/get-1 | rps 1.023×, p99 1.000×, RSS 0.741× | rps 0.990×, p99 1.011×, RSS 0.768× | rps 0.737×, p99 1.442×, RSS 3.607× |
| v0.16.0 | a45d5cd3 | 2026-11-11 | ec2:c7i.metal-24xl | todo/get-64 | rps 1.282×, p99 0.500×, RSS 1.083× | rps 1.176×, p99 0.555×, RSS 0.770× | rps 0.680×, p99 3.544×, RSS 3.100× |
| v0.16.0 | a45d5cd3 | 2026-11-11 | ec2:c7i.metal-24xl | todo/patch-16 | rps 1.203×, p99 0.566×, RSS 0.808× | rps 1.193×, p99 0.740×, RSS 0.770× | rps 0.886×, p99 1.983×, RSS 3.431× |
