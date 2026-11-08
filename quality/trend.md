# Quality trend

Generated from `quality/history.jsonl` by `task quality` (`quality::record`); do not edit.
One row per release and the first assessment (`bar`); the changes are against the row before
(`docs/quality.md`). Timing rows (source `perf`) measured on `host: local`, the shared
workstation, are not comparable between runs: they are left out of `worse`, `better` and
`changes` and counted under `timing` (story:quality-timing-host brings a dedicated host).

| run | commit | date | worse | better | timing | changes |
|---|---|---|---|---|---|---|
| bar | 9a775116 | 2026-11-07 | 0 | 0 | 5 local, not comparable | 55 values, the bar |
| v0.14.0 | d87fd798 | 2026-11-08 | 10 | 6 | 5 local, not comparable | 1.2 eval 420→423; 1.2 llvm 614→643; 1.2 rust 614→643; 1.3 eval/builtins 112→120; 1.3 llvm/builtins 12→13; 1.3 rust/builtins 43→44; 1.6 eval 194→220; 2.5 all 319→323; 3.2 llvm/terminald/dev 7563720 B→7780040 B; 3.2 llvm/terminald/release 1468648 B→1505048 B; 3.2 llvm/todod/dev 8230560 B→8452816 B; 3.2 llvm/todod/release 2042752 B→2088624 B; 3.2 rust/terminald/dev 6223520 B→6390088 B; 3.2 rust/terminald/release 681368 B→695480 B; 3.2 rust/todod/dev 7405864 B→7574008 B; 3.2 rust/todod/release 961880 B→980136 B; 3.6 llvm 73→76; 3.6 rust 7→9; 4.2 llvm/rfc9110 1→2; 4.2 rust/rfc9110 1→2; 4.3 llvm 309→322; 4.3 rust 309→322; 7.2 all 21→23; 7.4 all 34→43 |
