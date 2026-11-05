# Performance comparison

unit | baseline | before s | candidate s | ratio | valid/excluded
---|---|---:|---:|---:|---:
todod-rust-dev | baseline | 7.203409 | 6.304194 | 0.9094 | 6/0
todod-llvm-dev | baseline | 0.512897 | 0.855612 | 1.5252 | 6/0
todod-rust-release | baseline | 10.308131 | 8.203245 | 0.8948 | 6/0
todod-llvm-release | baseline | 0.993518 | 1.499026 | 1.4194 | 6/0
check | baseline | 0.133876 | 0.136190 | 1.0469 | 15/0
plan | baseline | 0.136619 | 0.144428 | 1.0646 | 15/0
apply-cycle-terminal-rust | baseline | 90.536000 | 29.242316 | 0.3230 | 1/0
cold-inline | baseline | 42.656126 | 39.426039 | 0.9243 | 1/0

Accepted: False
Missing evidence: cold-clippy, cold-fmt, cold-workspace, file-ignite_test, file-rustgen, gate-cold, gate-warm, no-run-touch, no-run-warm, warm-clippy, warm-fmt, warm-inline, warm-workspace
Over budget: todod-llvm-dev, todod-llvm-release
