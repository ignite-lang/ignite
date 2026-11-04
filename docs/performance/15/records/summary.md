# Performance comparison

unit | baseline | before s | candidate s | ratio | valid/excluded
---|---|---:|---:|---:|---:
cold-inline | baseline | 14.288645 | 12.186941 | 0.9798 | 2/0
warm-fmt | baseline | 0.967226 | 0.798175 | 0.8202 | 3/2
warm-clippy | baseline | 4.383968 | 4.035284 | 1.0030 | 3/2
warm-workspace | baseline | 403.407447 | 401.098180 | 0.9922 | 4/1
warm-inline | baseline | 1.256956 | 1.424583 | 1.1643 | 5/0
gate-warm | baseline | 395.386813 | 393.148413 | 1.0164 | 3/2
no-run-warm | baseline | 0.032214 | 0.030167 | 0.9294 | 5/0
no-run-touch | baseline | 2.987166 | 4.025379 | 1.1319 | 5/0
file-ignite_test | baseline | 2.134548 | 2.362730 | 1.1221 | 5/0
file-rustgen | baseline | 1.825403 | 2.428537 | 1.3252 | 5/0
todod-rust-dev | baseline | 3.468008 | 3.449111 | 0.9698 | 3/0
todod-llvm-dev | baseline | 0.292595 | 0.340317 | 1.1377 | 3/0
todod-rust-release | baseline | 4.537091 | 4.581610 | 0.9770 | 3/0
todod-llvm-release | baseline | 0.752435 | 0.866156 | 1.1511 | 3/0
check | baseline | 0.078758 | 0.088584 | 1.1166 | 15/0
plan | baseline | 0.077245 | 0.084409 | 1.0921 | 15/0

Accepted: False
Missing evidence: cold-clippy, cold-fmt, cold-workspace, gate-cold
Over budget: check, file-ignite_test, file-rustgen, no-run-touch, todod-llvm-dev, todod-llvm-release, warm-inline
