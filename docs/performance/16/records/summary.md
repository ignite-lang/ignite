# Performance comparison

unit | baseline | before s | candidate s | ratio | valid/excluded
---|---|---:|---:|---:|---:
cold-fmt | baseline | 0.845470 | 0.928196 | 1.0978 | 1/0
cold-clippy | baseline | 8.665813 | 5.765876 | 0.6654 | 1/0
cold-workspace | baseline | 579.631434 | 629.565257 | 1.0861 | 1/0
cold-inline | baseline | 16.413094 | 28.071288 | 1.7103 | 1/0
gate-cold | baseline | 605.562779 | 664.336732 | 1.0971 | 1/0

Accepted: False
Missing evidence: check, file-ignite_test, file-rustgen, gate-warm, no-run-touch, no-run-warm, plan, todod-llvm-dev, todod-llvm-release, todod-rust-dev, todod-rust-release, warm-clippy, warm-fmt, warm-inline, warm-workspace
Over budget: cold-inline
