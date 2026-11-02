# Performance comparison

unit | baseline | before s | candidate s | ratio | valid/excluded
---|---|---:|---:|---:|---:
warm-fmt | baseline | 0.487040 | 0.537441 | 1.0704 | 3/1
warm-clippy | baseline | 2.435924 | 3.141346 | 1.1675 | 3/1
warm-workspace | baseline | 11.554199 | 12.374647 | 1.0586 | 3/1
warm-inline | baseline | 0.682590 | 1.203443 | 1.7498 | 3/1
gate-warm | baseline | 15.130241 | 16.960511 | 1.1210 | 3/1
no-run-warm | baseline | 0.025293 | 0.024943 | 0.9862 | 3/1
no-run-touch | baseline | 2.173603 | 2.314760 | 1.0155 | 3/1
file-ignite_test | baseline | 4.779299 | 2.226347 | 0.4369 | 3/1
file-rustgen | baseline | 1.837030 | 2.721556 | 1.3993 | 3/1
check | baseline | 0.063763 | 0.071832 | 1.0741 | 15/0
plan | baseline | 0.064886 | 0.069666 | 1.1067 | 15/0
warm-fmt | comparison | 0.581210 | 0.537441 | 0.9926 | 3/1
warm-clippy | comparison | 2.754476 | 3.141346 | 1.1272 | 3/1
warm-workspace | comparison | 13.297788 | 13.320321 | 0.9330 | 4/0
warm-inline | comparison | 1.181010 | 1.197607 | 1.0088 | 4/0
gate-warm | comparison | 17.509337 | 16.960511 | 0.9687 | 3/1
no-run-warm | comparison | 0.025786 | 0.024928 | 0.9668 | 4/0
no-run-touch | comparison | 2.452599 | 2.343901 | 0.9511 | 4/0
file-ignite_test | comparison | 2.461531 | 2.151630 | 0.8766 | 4/0
file-rustgen | comparison | 3.669466 | 2.633837 | 0.7130 | 4/0
check | comparison | 0.092182 | 0.071832 | 0.7579 | 15/0
plan | comparison | 0.093974 | 0.069666 | 0.7875 | 15/0

Accepted: False
Missing evidence: cold-clippy, cold-fmt, cold-inline, cold-workspace, gate-cold, todod-llvm-dev, todod-llvm-release, todod-rust-dev, todod-rust-release
Over budget: file-rustgen, gate-warm, plan, warm-clippy, warm-inline
